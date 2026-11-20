"""Synthetic coverage/identity/metadata fixtures; no network or hardware access."""
import copy
import contextlib
import datetime
import hashlib
import io
import json
from pathlib import Path
import struct
import tempfile
import unittest

import bfi_decode as decoder
import quality_report as quality

AP, CLIENT, OTHER = (bytes.fromhex(value) for value in
                     ("020000000001", "020000000002", "020000000003"))
START = 1_000_000_000_000


def radio(frame, metadata=True, extended=False):
    if not metadata:
        return struct.pack("<BBHI", 0, 0, 8, 0) + frame
    # Channel (bit3), signed first antenna signal (bit5); alignment is 2 bytes.
    present = 0x28 | (0x80000000 if extended else 0)
    offset = 12 if extended else 8
    prefix = struct.pack("<BBHI", 0, 0, offset + 5, present)
    if extended:
        prefix += bytes(4)
    return prefix + struct.pack("<HHb", 5180, 0x140, -82) + frame


def report(sequence=1, retry=False, receiver=AP, transmitter=CLIENT):
    fc = 0xe0 | (0x800 if retry else 0)
    header = struct.pack("<HH", fc, 0) + receiver + transmitter + AP + struct.pack("<H", sequence << 4)
    # HE SU 2x2, full80, Ng4, codebook0, complete report; 250 pairs.
    body = bytes.fromhex("1e00898000120000006506") + bytes(186)
    return header + body


def beacon():
    return struct.pack("<HH", 0x80, 0) + b"\xff" * 6 + AP + AP + bytes(14)


def capture(records, duration=4.5):
    raw = struct.pack("<IHHIIII", 0xa1b2c3d4, 2, 4, 0, 0, 65535, 127)
    for offset, packet in records:
        timestamp = START + round(offset * quality.NS)
        seconds, nanoseconds = divmod(timestamp, quality.NS)
        raw += struct.pack("<IIII", seconds, nanoseconds // 1000, len(packet), len(packet)) + packet
    def iso(timestamp):
        return datetime.datetime.fromtimestamp(timestamp / quality.NS, datetime.timezone.utc).isoformat()
    manifest = {"schema": 1, "capture_started_at": iso(START),
                "capture_ended_at": iso(START + round(duration * quality.NS)),
                "capture_seconds": duration, "capture_exit": 124,
                "sha256": hashlib.sha256(raw).hexdigest()}
    return raw, manifest


def analyze(records, duration=4.5):
    raw, manifest = capture(records, duration)
    return quality.analyze(io.BytesIO(raw), manifest, AP, CLIENT)


class CoverageTests(unittest.TestCase):
    def test_edges_partial_bin_and_blackouts_use_capture_window(self):
        result = analyze([(0.1, radio(beacon())), (1.2, radio(report())),
                          (1.4, radio(report(2))), (4.3, radio(beacon()))])
        value = result["feedback"]
        self.assertEqual(value["one_second_bin_counts"], [0, 2, 0, 0, 0])
        self.assertEqual(value["final_bin_seconds"], .5)
        self.assertAlmostEqual(value["leading_no_report_seconds"], 1.2)
        self.assertAlmostEqual(value["trailing_no_report_seconds"], 3.1)
        self.assertAlmostEqual(value["maximum_no_report_gap_seconds"], 3.1)
        self.assertAlmostEqual(value["maximum_empty_bin_run_seconds"], 2.5)
        self.assertAlmostEqual(value["reports_per_second"], 2 / 4.5)
        self.assertAlmostEqual(value["occupied_bin_fraction"], 1 / 5)
        self.assertAlmostEqual(value["occupied_bin_duration_fraction"], 1 / 4.5)

    def test_no_reports_has_full_window_blackout_even_without_packets(self):
        for records in ([], [(1, radio(beacon()))]):
            result = analyze(records)
            value = result["feedback"]
            self.assertEqual(result["status"], "NO_EXPECTED_CLIENT_REPORTS")
            self.assertEqual(value["unique_reports"], 0)
            self.assertEqual(value["leading_no_report_seconds"], 4.5)
            self.assertEqual(value["trailing_no_report_seconds"], 4.5)
            self.assertEqual(value["maximum_no_report_gap_seconds"], 4.5)
            self.assertEqual(value["maximum_empty_bin_run_seconds"], 4.5)

    def test_bin_boundaries_and_exact_end_have_declared_semantics(self):
        result = analyze([(0, radio(report(1))), (1, radio(report(2))),
                          (2, radio(report(3)))], duration=2)
        self.assertEqual(result["feedback"]["one_second_bin_counts"], [1, 2])
        self.assertEqual(result["feedback"]["leading_no_report_seconds"], 0)
        self.assertEqual(result["feedback"]["trailing_no_report_seconds"], 0)

    def test_only_expected_direction_counts_and_retries_do_not_inflate_coverage(self):
        result = analyze([(0, radio(report(1))), (0.1, radio(report(1, retry=True))),
                          (1, radio(report(2, receiver=CLIENT, transmitter=AP))),
                          (2, radio(report(3, transmitter=OTHER))),
                          (3, radio(report(4)))])
        self.assertEqual(result["feedback"]["unique_reports"], 2)
        self.assertEqual(result["expected_retry_duplicates"], 1)
        self.assertEqual(result["supported_reports_by_direction"], {
            "expected_client_to_ap": 3, "ap_to_expected_client": 1, "other_link": 1})
        serialized = json.dumps(result)
        for forbidden in ("02:00:00:00:00:01", "02:00:00:00:00:02", '"angles"', '"matrices"'):
            self.assertNotIn(forbidden, serialized)


class MetadataTests(unittest.TestCase):
    def test_all_prefix_fields_align_after_extended_presence(self):
        prefix = (struct.pack("<BBHII", 0, 0, 33, 0x8000003f, 0) + bytes(4)
                  + bytes(8) + b"\x00\x0c" + struct.pack("<HH", 5180, 0x140)
                  + bytes(2) + struct.pack("b", -82))
        result = analyze([(0, prefix + beacon())])
        self.assertEqual(result["beacons"]["frequency_counts_mhz"], {5180: 1})
        self.assertEqual(result["beacons"]["signal_dbm_median"], -82)

    def test_known_prefix_and_extended_presence_match(self):
        result = analyze([(0, radio(beacon())), (1, radio(beacon(), extended=True)),
                          (2, radio(beacon(), metadata=False))])
        self.assertEqual(result["beacons"]["frequency_counts_mhz"], {5180: 2})
        self.assertEqual(result["beacons"]["frequency_unavailable"], 1)
        self.assertEqual(result["beacons"]["signal_dbm_median"], -82)
        self.assertEqual(result["beacons"]["signal_unavailable"], 1)

    def test_metadata_truncation_and_bad_fcs_are_rejected(self):
        invalid = struct.pack("<BBHI", 0, 0, 8, 0x28) + beacon()
        with self.assertRaisesRegex(decoder.DecodeError, "metadata_truncated"):
            analyze([(0, invalid)])
        bad_fcs = struct.pack("<BBHI", 0, 0, 9, 2) + b"\x40" + beacon()
        with self.assertRaisesRegex(decoder.DecodeError, "bad_fcs"):
            analyze([(0, bad_fcs)])


class IntegrityTests(unittest.TestCase):
    def test_huge_json_duration_returns_bounded_cli_error(self):
        raw, manifest = capture([])
        manifest["capture_seconds"] = 10 ** 400
        with tempfile.TemporaryDirectory() as directory:
            packet_path = Path(directory) / "capture.pcap"
            manifest_path = Path(directory) / "manifest.json"
            packet_path.write_bytes(raw)
            manifest_path.write_text(json.dumps(manifest))
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                status = quality.main([str(packet_path), "--manifest", str(manifest_path),
                                       "--ap", "02:00:00:00:00:01",
                                       "--client", "02:00:00:00:00:02"])
            self.assertEqual(status, 1)
            self.assertEqual(json.loads(output.getvalue()),
                             {"status": "INPUT_ERROR", "reason": "invalid_manifest_duration"})

    def test_hash_missing_time_clock_mismatch_invalid_exit_fail(self):
        raw, manifest = capture([(0, radio(beacon()))])
        cases = [("sha256", "0" * 64), ("capture_started_at", None),
                 ("capture_started_at", "1970-01-01T00:16:40"),
                 ("capture_seconds", 1), ("capture_seconds", float("nan")),
                 ("capture_seconds", True), ("capture_seconds", 10 ** 400),
                 ("capture_exit", 1), ("capture_exit", False),
                 ("schema", True)]
        for key, value in cases:
            changed = copy.deepcopy(manifest)
            changed[key] = value
            with self.subTest(key=key, value=value):
                with self.assertRaises(decoder.DecodeError):
                    quality.analyze(io.BytesIO(raw), changed, AP, CLIENT)

    def test_outside_window_and_clock_regression_rejected(self):
        for records, reason in ([(-.01, radio(beacon()))], "outside_manifest_window"), \
                               ([(4.6, radio(beacon()))], "outside_manifest_window"), \
                               ([(2, radio(beacon())), (1, radio(beacon()))], "timestamp_regression"):
            with self.assertRaisesRegex(decoder.DecodeError, reason):
                analyze(records)

    def test_malformed_supported_report_fails_without_partial_metrics(self):
        with self.assertRaisesRegex(decoder.DecodeError, "payload_length_mismatch"):
            analyze([(0, radio(report())), (1, radio(report(2)[:-1]))])

    def test_unsupported_report_is_explicit_and_not_counted(self):
        frame = report()[:24] + b"\x24\x00" + bytes(5)
        result = analyze([(0, radio(frame))])
        self.assertEqual(result["unsupported_actions"], {"unsupported_eht_feedback": 1})
        self.assertEqual(result["feedback"]["unique_reports"], 0)

    def test_manifest_duplicate_keys_nan_and_deep_json_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "manifest.json"
            for value in ('{"schema":1,"schema":1}', '{"value":NaN}', '[' * 1100 + '0' + ']' * 1100):
                path.write_text(value)
                with self.assertRaises(decoder.DecodeError):
                    quality.read_manifest(path)


if __name__ == "__main__":
    unittest.main()
