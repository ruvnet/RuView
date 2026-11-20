#!/usr/bin/env python3
"""Offline, aggregate-only feedback coverage for a verified capture window.

Requires classic radiotap PCAP, its capture_linux.py manifest, and explicit
own-network AP/client addresses. This tool never captures or sends traffic.
It measures captured reports, not the AP's sounding rate or radio loss.
"""
import argparse
import collections
import datetime
import hashlib
import json
import math
from pathlib import Path
import re
import statistics
import struct
import sys

import bfi_decode as decoder

NS = 1_000_000_000
MAX_MANIFEST = 1024 * 1024
MAX_SECONDS = 86400
CLOCK_TOLERANCE_NS = 1_000_000


def timestamp_ns(value):
    decoder.require(isinstance(value, str) and re.fullmatch(
        r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,6})?(?:Z|[+-]\d{2}:\d{2})", value) is not None,
                    "invalid_manifest_timestamp")
    try:
        parsed = datetime.datetime.fromisoformat(value)
        decoder.require(parsed.tzinfo is not None, "manifest_timestamp_requires_timezone")
        delta = parsed.astimezone(datetime.timezone.utc) - datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
        return (delta.days * 86400 + delta.seconds) * NS + delta.microseconds * 1000
    except (ValueError, OverflowError) as error:
        if isinstance(error, decoder.DecodeError):
            raise
        raise decoder.DecodeError("invalid_manifest_timestamp") from error


def manifest_window(manifest):
    decoder.require(isinstance(manifest, dict) and type(manifest.get("schema")) is int
                    and manifest.get("schema") == 1,
                    "unsupported_manifest_schema")
    start = timestamp_ns(manifest.get("capture_started_at"))
    end = timestamp_ns(manifest.get("capture_ended_at"))
    duration = manifest.get("capture_seconds")
    decoder.require(isinstance(duration, (int, float)) and not isinstance(duration, bool)
                    and 0 < duration <= MAX_SECONDS and math.isfinite(duration),
                    "invalid_manifest_duration")
    decoder.require(0 < end - start <= MAX_SECONDS * NS,
                    "invalid_manifest_window")
    decoder.require(abs(end - start - round(duration * NS)) <= CLOCK_TOLERANCE_NS,
                    "manifest_wall_monotonic_duration_mismatch")
    digest = manifest.get("sha256")
    decoder.require(isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest) is not None,
                    "invalid_manifest_sha256")
    decoder.require(type(manifest.get("capture_exit")) is int and manifest.get("capture_exit") in (0, 124)
                    and not manifest.get("error"), "manifest_capture_not_successful")
    return start, end, duration


def radiotap_metadata(packet):
    """Read only standard fields 0..5; later/vendor fields are never guessed.

    Signal is the first reported dBm antenna signal, not an average of chains.
    decoder.strip_radiotap validates the envelope and any supplied FCS first.
    """
    _, _, length, first = struct.unpack_from("<BBHI", packet)
    offset, present = 8, first
    while present & (1 << 31):
        present = struct.unpack_from("<I", packet, offset)[0]
        offset += 4
    result = {"frequency_mhz": None, "signal_dbm": None}
    for bit, alignment, size in ((0, 8, 8), (1, 1, 1), (2, 1, 1),
                                  (3, 2, 4), (4, 2, 2), (5, 1, 1)):
        if not first & (1 << bit):
            continue
        offset = (offset + alignment - 1) & ~(alignment - 1)
        decoder.require(offset + size <= length, "radiotap_metadata_truncated")
        if bit == 3:
            frequency = struct.unpack_from("<H", packet, offset)[0]
            result["frequency_mhz"] = frequency or None
        elif bit == 5:
            result["signal_dbm"] = struct.unpack_from("b", packet, offset)[0]
        offset += size
    return result


def direction(a1, a2, a3, ap, client):
    if (a1, a2, a3) == (ap, client, ap):
        return "expected_client_to_ap"
    if (a1, a2, a3) == (client, ap, ap):
        return "ap_to_expected_client"
    return "other_link"


def coverage(timestamps, start, end, duration):
    width = end - start
    bins = [0] * ((width + NS - 1) // NS)
    for timestamp in timestamps:
        # The exact end instant belongs to the final, right-closed bin.
        bins[min((timestamp - start) // NS, len(bins) - 1)] += 1
    bin_widths = [min(NS, width - i * NS) / NS for i in range(len(bins))]
    leading = (timestamps[0] - start) / NS if timestamps else width / NS
    trailing = (end - timestamps[-1]) / NS if timestamps else width / NS
    gaps = [(b - a) / NS for a, b in zip(timestamps, timestamps[1:])]
    longest_empty = current_empty = 0.0
    for count, bin_width in zip(bins, bin_widths):
        current_empty = 0.0 if count else current_empty + bin_width
        longest_empty = max(longest_empty, current_empty)
    occupied = sum(count > 0 for count in bins)
    return {"unique_reports": len(timestamps),
            "reports_per_second": len(timestamps) / duration,
            "report_span_seconds": (timestamps[-1] - timestamps[0]) / NS if timestamps else None,
            "leading_no_report_seconds": leading, "trailing_no_report_seconds": trailing,
            "maximum_no_report_gap_seconds": max([leading, trailing] + gaps),
            "inter_report_gaps": decoder.gap_summary(timestamps),
            "one_second_bin_counts": bins,
            "bin_origin": "manifest_capture_start",
            "bin_endpoint_rule": "left_closed_right_open_except_final_right_closed",
            "final_bin_seconds": bin_widths[-1],
            "occupied_bins": occupied, "empty_bins": len(bins) - occupied,
            "occupied_bin_fraction": occupied / len(bins),
            "occupied_bin_duration_fraction": sum(w for w, n in zip(bin_widths, bins) if n) / (width / NS),
            "maximum_empty_bin_run_seconds": longest_empty}


def analyze(stream, manifest, ap, client):
    start, end, duration = manifest_window(manifest)
    decoder.require(isinstance(ap, bytes) and isinstance(client, bytes)
                    and len(ap) == len(client) == 6 and ap != client
                    and not ap[0] & 1 and not client[0] & 1
                    and ap != bytes(6) and client != bytes(6), "invalid_ap_client_pair")
    digest = hashlib.sha256()
    stream.seek(0)
    total_bytes = 0
    while chunk := stream.read(65536):
        total_bytes += len(chunk)
        decoder.require(total_bytes <= decoder.MAX_FILE, "file_limit_exceeded")
        digest.update(chunk)
    decoder.require(digest.hexdigest() == manifest["sha256"], "manifest_capture_hash_mismatch")
    stream.seek(0)
    packet_count = beacon_count = retry_duplicates = 0
    previous = None
    reports, beacon_timestamps, signals = [], [], []
    frequencies = collections.Counter()
    directions = collections.Counter()
    unsupported = collections.Counter()
    formats = collections.Counter()
    seen = collections.OrderedDict()
    for timestamp, packet, truncated in decoder.pcap_packets(stream):
        packet_count += 1
        decoder.require(start <= timestamp <= end, "packet_outside_manifest_window")
        decoder.require(previous is None or timestamp >= previous, "packet_timestamp_regression")
        previous = timestamp
        decoder.require(not truncated, "snaplen_truncated")
        frame = decoder.strip_radiotap(packet)
        metadata = radiotap_metadata(packet)
        decoder.require(len(frame) >= 2, "frame_control_truncated")
        fc = int.from_bytes(frame[:2], "little")
        decoder.require(fc & 3 == 0, "unsupported_80211_version")
        if (fc >> 2) & 3 != 0:
            continue
        decoder.require(len(frame) >= 24, "management_header_truncated")
        a1, a2, a3 = frame[4:10], frame[10:16], frame[16:22]
        subtype = (fc >> 4) & 15
        if subtype == 8 and a2 == ap and a3 == ap:
            beacon_count += 1
            beacon_timestamps.append(timestamp)
            if metadata["frequency_mhz"] is not None:
                frequencies[metadata["frequency_mhz"]] += 1
            if metadata["signal_dbm"] is not None:
                signals.append(metadata["signal_dbm"])
        if subtype not in (13, 14) or ap not in (a1, a2, a3):
            continue
        # Encrypted payloads cannot be classified from their body bytes.
        if fc & 0x4000:
            unsupported["protected_action"] += 1
            continue
        decoder.require(not fc & 0x0300, "invalid_management_ds_bits")
        if fc & 0x8400 or frame[22] & 15:
            unsupported["ordered_or_fragmented_action"] += 1
            continue
        body = frame[24:]
        decoder.require(len(body) >= 2, "action_header_truncated")
        if body[:2] not in (b"\x15\x00", b"\x1e\x00", b"\x24\x00"):
            continue
        try:
            report = decoder.decode_report(body)
        except decoder.DecodeError as error:
            if error.reason.startswith("unsupported_"):
                unsupported[error.reason] += 1
                continue
            raise
        report_direction = direction(a1, a2, a3, ap, client)
        directions[report_direction] += 1
        if report_direction != "expected_client_to_ap":
            continue
        key = (frame[22:24], hashlib.sha256(body).digest())
        prior = seen.get(key)
        duplicate = bool(fc & 0x0800 and prior is not None and timestamp - prior <= NS)
        seen[key] = timestamp
        seen.move_to_end(key)
        if len(seen) > 4096:
            seen.popitem(last=False)
        if duplicate:
            retry_duplicates += 1
            continue
        reports.append(timestamp)
        formats[(report["standard"], report["nr"], report["nc"], report["bandwidth_mhz"],
                 report["grouping"], report["codebook"])] += 1
    return {"schema_version": 1,
            "status": "REPORTS_CAPTURED" if reports else "NO_EXPECTED_CLIENT_REPORTS",
            "capture_sha256": manifest["sha256"], "capture_duration_seconds": duration,
            "window_seconds": (end - start) / NS, "window_validated": True,
            "packets": packet_count, "supported_reports_by_direction": dict(directions),
            "expected_retry_duplicates": retry_duplicates, "unsupported_actions": dict(unsupported),
            "feedback": coverage(reports, start, end, duration),
            "report_formats": [dict(zip(("standard", "nr", "nc", "bandwidth_mhz", "grouping", "codebook"), key), reports=count)
                               for key, count in sorted(formats.items())],
            "beacons": {"count": beacon_count, "per_second": beacon_count / duration,
                        "frequency_counts_mhz": dict(sorted(frequencies.items())),
                        "frequency_unavailable": beacon_count - sum(frequencies.values()),
                        "signal_samples": len(signals), "signal_unavailable": beacon_count - len(signals),
                        "signal_dbm_min": min(signals) if signals else None,
                        "signal_dbm_median": statistics.median(signals) if signals else None,
                        "signal_dbm_max": max(signals) if signals else None,
                        "signal_basis": "first_reported_radiotap_dbm_antenna_signal",
                        "inter_beacon_gaps": decoder.gap_summary(beacon_timestamps)},
            "limitations": ["Captured report coverage does not measure AP sounding rate or RF loss",
                            "No-report leading and trailing windows overlap when no reports exist",
                            "Occupied bins do not imply uniform sampling within a bin",
                            "Unsupported feedback is not counted as a decoded report",
                            "No motion inference or raw angles are emitted"]}


def read_manifest(path):
    def pairs(items):
        result = {}
        for key, value in items:
            decoder.require(key not in result, "duplicate_manifest_key")
            result[key] = value
        return result
    decoder.require(path.is_file() and path.stat().st_size <= MAX_MANIFEST,
                    "manifest_missing_or_too_large")
    try:
        result = json.loads(path.read_bytes(), object_pairs_hook=pairs,
                            parse_constant=lambda _: decoder.require(False, "invalid_manifest_constant"))
        decoder.require(isinstance(result, dict), "invalid_manifest_json")
        return result
    except (UnicodeError, ValueError, RecursionError) as error:
        if isinstance(error, decoder.DecodeError):
            raise
        raise decoder.DecodeError("invalid_manifest_json") from error


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pcap", type=Path)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--ap", required=True)
    parser.add_argument("--client", required=True)
    args = parser.parse_args(argv)
    try:
        manifest = read_manifest(args.manifest)
        decoder.require(args.pcap.is_file() and args.pcap.stat().st_size <= decoder.MAX_FILE,
                        "capture_missing_or_too_large")
        with args.pcap.open("rb") as stream:
            result = analyze(stream, manifest, decoder.mac(args.ap), decoder.mac(args.client))
        print(json.dumps(result, indent=2, allow_nan=False))
        return 0  # A valid zero-report capture is an informative quality result.
    except (decoder.DecodeError, OSError) as error:
        print(json.dumps({"status": "INPUT_ERROR", "reason": str(error) if isinstance(error, decoder.DecodeError) else "file_io_error"}))
        return 1


if __name__ == "__main__":
    sys.exit(main())
