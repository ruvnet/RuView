"""Synthetic protocol fixtures only. Run: python -m unittest discover -s tools/bfi -v."""
import io
import copy
import math
import struct
import unittest
import zlib

import bfi_decode as decoder

AP = bytes.fromhex("020000000001")
STA = bytes.fromhex("020000000002")


def vht_body(control=0x8289, snr=b"\x80\x7f"):
    # Literal control: 2 columns, 2 rows, 80 MHz, Ng=4, SU codebook 0,
    # first and only feedback segment. 62 tones * 6 bits = 47 bytes.
    # Golden independent first pairs (phi,psi)=(5,2),(9,1) => 65 06.
    return b"\x15\x00" + control.to_bytes(3, "little") + snr + b"\x65\x06" + bytes(45)


def he_body():
    # 5-byte HE control: 2x2, 80 MHz, Ng=4, SU codebook 0, first,
    # RU start=0/end=36. 250*6 bits=188 bytes.
    return b"\x1e\x00\x89\x80\x00\x12\x00\x00\x00\x65\x06" + bytes(186)


def he20_body(nc=2, codebook=0):
    # Full 20 MHz RU0..8, Ng4, two rows; exactly 64 angle pairs.
    control = (0x04008008 + nc - 1) | (codebook << 9)
    prefix, length = (b"\x65\x06", 48) if codebook == 0 else (b"\x85\x24\x01", 80)
    return (b"\x1e\x00" + control.to_bytes(5, "little") + b"\x80\x7f"[:nc]
            + prefix + bytes(length - len(prefix)))


def he40_body(nc=2, codebook=0):
    # Full 40 MHz RU0..17, Ng4, two rows; 122 pairs leave four zero pad bits.
    control = (0x08808048 + nc - 1) | (codebook << 9)
    prefix, length = (b"\x65\x06", 92) if codebook == 0 else (b"\x85\x24\x01", 153)
    return (b"\x1e\x00" + control.to_bytes(5, "little") + b"\x80\x7f"[:nc]
            + prefix + bytes(length - len(prefix)))


def action(body=None, retry=False, sequence=7):
    fc = 0x00e0 | (0x0800 if retry else 0)
    return struct.pack("<HH", fc, 0) + AP + STA + AP + struct.pack("<H", sequence << 4) + (body or vht_body())


def radio(frame, flags=None):
    if flags is None:
        return b"\x00\x00\x08\x00\x00\x00\x00\x00" + frame
    return b"\x00\x00\x09\x00\x02\x00\x00\x00" + bytes([flags]) + frame


def pcap(records, endian="<", nano=False):
    magic = 0xa1b23c4d if nano else 0xa1b2c3d4
    data = struct.pack(endian + "IHHIIII", magic, 2, 4, 0, 0, 65535, 127)
    for sec, frac, packet, extra in records:
        data += struct.pack(endian + "IIII", sec, frac, len(packet), len(packet) + extra) + packet
    return io.BytesIO(data)


class DecodeTests(unittest.TestCase):
    def assertReason(self, reason, call):
        with self.assertRaisesRegex(decoder.DecodeError, "^" + reason + "$"):
            call()

    def test_independent_nonzero_angle_golden(self):
        angles = decoder.decode_angles(bytes.fromhex("6506"), 2, 4, 2)
        self.assertEqual([pair[:2] for pair in angles], [(5, 2), (9, 1)])
        for (_, _, phi, psi), expected_phi, expected_psi in zip(
                angles, [11 * math.pi / 16, 19 * math.pi / 16],
                [5 * math.pi / 16, 3 * math.pi / 16]):
            self.assertAlmostEqual(phi, expected_phi)
            self.assertAlmostEqual(psi, expected_psi)
        # Independent numerical oracle for tone 1; catches omitted phi/sign errors.
        matrix = decoder.steering_matrix(angles[0][2], angles[0][3], 2)
        self.assertAlmostEqual(matrix[0][0].real, -0.3086582838174551)
        self.assertAlmostEqual(matrix[0][0].imag, 0.4619397662556434)
        self.assertAlmostEqual(matrix[0][1].real, 0.4619397662556434)
        self.assertAlmostEqual(matrix[0][1].imag, -0.6913417161825449)
        self.assertAlmostEqual(matrix[1][0].real, 0.8314696123025452)
        self.assertAlmostEqual(matrix[1][1].real, 0.5555702330196023)

    def test_vht_complete_and_snr(self):
        result = decoder.decode_report(vht_body())
        self.assertEqual(result["tone_count"], 62)
        self.assertEqual(result["snr_db"], [-10.0, 53.75])
        self.assertEqual(result["snr_endpoint_saturated"], [True, True])
        self.assertEqual(result["angles"][0][:2], (5, 2))
        self.assertLess(result["orthonormality_max_residual"], 1e-12)
        one = decoder.decode_report(vht_body(0x8288, b"\x00"))
        self.assertEqual(one["nc"], 1)
        self.assertEqual(len(one["matrices"][0][0]), 1)

    def test_he_full80(self):
        result = decoder.decode_report(he_body())
        self.assertEqual(result["standard"], "HE")
        self.assertEqual(result["tone_count"], 250)
        self.assertEqual(result["angles"][1][:2], (9, 1))
        self.assertLess(result["orthonormality_max_residual"], 1e-12)

    def test_cached_matrices_do_not_alias_reports_or_repeated_tones(self):
        original = decoder.decode_report(he_body())
        expected = copy.deepcopy(original)
        # Tones 2 and 3 share the same all-zero integer angles in this fixture.
        original["matrices"][2][0][0] = complex(99, 99)
        original["matrices"][2][1].clear()
        original["angles"][2] = (99, 99, 99.0, 99.0)
        self.assertEqual(original["matrices"][3], expected["matrices"][3])
        self.assertEqual(decoder.decode_report(he_body()), expected)

    def test_he_full20_both_dimensions_and_codebooks_have_exact_budget(self):
        for nc in (1, 2):
            for codebook in (0, 1):
                body = he20_body(nc, codebook)
                result = decoder.decode_report(body)
                self.assertEqual(result["bandwidth_mhz"], 20)
                self.assertEqual(result["tone_count"], 64)
                self.assertEqual(result["nc"], nc)
                self.assertEqual(result["codebook"], codebook)
                self.assertEqual([pair[:2] for pair in result["angles"][:2]], [(5, 2), (9, 1)])
                self.assertTrue(all(len(row) == nc for matrix in result["matrices"] for row in matrix))
                for malformed in (body[:-1], body + b"\0"):
                    self.assertReason("report_payload_length_mismatch",
                                      lambda: decoder.decode_report(malformed))

    def test_he_full40_both_dimensions_codebooks_and_zero_padding(self):
        for nc in (1, 2):
            for codebook in (0, 1):
                body = he40_body(nc, codebook)
                result = decoder.decode_report(body)
                self.assertEqual(result["bandwidth_mhz"], 40)
                self.assertEqual(result["tone_count"], 122)
                self.assertEqual(result["nc"], nc)
                self.assertEqual(result["codebook"], codebook)
                self.assertEqual([pair[:2] for pair in result["angles"][:2]], [(5, 2), (9, 1)])
                self.assertTrue(all(len(row) == nc for matrix in result["matrices"] for row in matrix))
                for malformed in (body[:-1], body + b"\0"):
                    self.assertReason("report_payload_length_mismatch",
                                      lambda: decoder.decode_report(malformed))
                self.assertReason("nonzero_angle_padding",
                                  lambda: decoder.decode_report(body[:-1] + b"\x80"))

    def test_he_partial_ru_invalid_end_and_ng16_remain_unsupported(self):
        full20 = int.from_bytes(he20_body()[2:7], "little")
        controls = (full20 | (1 << 8), full20 | (1 << 16),
                    (full20 & ~(127 << 23)) | (7 << 23),
                    (full20 & ~(127 << 23)) | (18 << 23) | (1 << 6))
        for control in controls:
            body = he20_body()[:2] + control.to_bytes(5, "little") + he20_body()[7:]
            self.assertReason("unsupported_he_bandwidth_grouping_or_partial_ru",
                              lambda: decoder.decode_report(body))
        full40 = int.from_bytes(he40_body()[2:7], "little")
        for control in (full40 | (1 << 8), full40 | (1 << 16),
                        (full40 & ~(127 << 23)) | (16 << 23),
                        (full40 & ~(127 << 23)) | (18 << 23)):
            body = he40_body()[:2] + control.to_bytes(5, "little") + he40_body()[7:]
            self.assertReason("unsupported_he_bandwidth_grouping_or_partial_ru",
                              lambda: decoder.decode_report(body))

    def test_all_supported_vht_lengths_and_codebooks(self):
        # Independent table: rows=20,40,80 MHz; columns=Ng1,2,4.
        for width_index, counts in enumerate(((52, 30, 16), (108, 58, 30), (234, 122, 62))):
            for group_index, tones in enumerate(counts):
                for codebook, bits in ((0, 6), (1, 10)):
                    control = 0x8009 | (width_index << 6) | (group_index << 8) | (codebook << 10)
                    body = b"\x15\x00" + control.to_bytes(3, "little") + bytes(2 + (tones * bits + 7) // 8)
                    result = decoder.decode_report(body)
                    self.assertEqual(result["tone_count"], tones)
                    self.assertEqual(result["phi_bits"] + result["psi_bits"], bits)

    def test_exact_payload_and_padding(self):
        for body in (vht_body()[:-1], vht_body() + b"\x00", he_body()[:-1], he_body() + b"\x00"):
            self.assertReason("report_payload_length_mismatch", lambda: decoder.decode_report(body))
        body = vht_body()[:-1] + b"\x80"
        self.assertReason("nonzero_angle_padding", lambda: decoder.decode_report(body))

    def test_unsupported_is_explicit(self):
        for bits, reason in [(0x1000, "unsupported_segmented_or_empty_feedback"),
                             (0x8000, "unsupported_segmented_or_empty_feedback"),
                             (0x0800, "unsupported_vht_mu_exclusive_report"),
                             (0x10000, "unsupported_vht_reserved_bits")]:
            self.assertReason(reason, lambda: decoder.decode_report(vht_body(0x8289 ^ bits)))
        control = bytearray(he_body())
        control[3] |= 1  # Ng=16.
        self.assertReason("unsupported_he_bandwidth_grouping_or_partial_ru",
                          lambda: decoder.decode_report(bytes(control)))

    def test_fcs_and_bad_flags(self):
        frame = action()
        checksum = struct.pack("<I", zlib.crc32(frame))
        self.assertEqual(decoder.strip_radiotap(radio(frame + checksum, 0x10)), frame)
        self.assertReason("fcs_mismatch", lambda: decoder.strip_radiotap(radio(frame + bytes(4), 0x10)))
        self.assertReason("radiotap_bad_fcs", lambda: decoder.strip_radiotap(radio(frame, 0x40)))

    def test_extended_radiotap_alignment(self):
        prefix = struct.pack("<BBHII", 0, 0, 25, 0x80000003, 0) + bytes(12) + b"\x00"
        self.assertEqual(len(prefix), 25)
        self.assertEqual(decoder.strip_radiotap(prefix + action()), action())
        self.assertReason("invalid_radiotap_presence", lambda: decoder.strip_radiotap(
            b"\x00\x00\x08\x00\x03\x00\x00\x80"))

    def test_endian_and_timestamp_resolution(self):
        for endian in ("<", ">"):
            for nano in (True, False):
                record = list(decoder.pcap_packets(pcap([(2, 123, radio(action()), 0)], endian, nano)))[0]
                self.assertEqual(record[0], 2_000_000_000 + 123 * (1 if nano else 1000))

    def test_pcap_corruption_rejected(self):
        capture = pcap([(0, 0, radio(action()), 0)]).getvalue()
        self.assertReason("pcap_record_data_truncated", lambda: list(decoder.pcap_packets(io.BytesIO(capture[:-1]))))
        self.assertReason("pcap_record_header_truncated", lambda: list(decoder.pcap_packets(io.BytesIO(capture + b"\x00"))))
        self.assertReason("unsupported_pcap_format_pcapng_not_supported",
                          lambda: list(decoder.pcap_packets(io.BytesIO(b"\x0a\x0d\x0d\x0a" + bytes(20)))))

    def test_retry_dedup_not_equal_payload_dedup(self):
        records = [(1, 0, radio(action()), 0), (1, 100000, radio(action(retry=True)), 0),
                   (2, 0, radio(action(sequence=8)), 0), (4, 0, radio(action(retry=True)), 0)]
        result = decoder.analyze(pcap(records), AP, duration=5)
        self.assertEqual(result["counts"]["decoded_reports"], 4)
        self.assertEqual(result["status"], "DECODED_REPORTS")
        self.assertEqual(result["decoder_validation"],
                         "requires_independent_capture_verification")
        self.assertEqual(result["counts"]["retry_duplicates"], 1)
        self.assertEqual(result["counts"]["unique_reports"], 3)
        self.assertEqual(result["unique_reports_per_second"], .6)
        self.assertEqual(result["links"][0]["max_gap_seconds"], 2)

    def test_truncation_and_no_reports(self):
        result = decoder.analyze(pcap([(1, 0, radio(action()), 10)]), AP, duration=30)
        self.assertEqual(result["rejections"], {"snaplen_truncated": 1})
        self.assertEqual(result["status"], "NO_TARGET_BEACONS")
        empty = decoder.analyze(pcap([]), AP, duration=30)
        self.assertEqual(empty["unique_reports_per_second"], 0)
        self.assertIsNone(empty["kernel_drops"])
        self.assertFalse(empty["motion_tested"])
        self.assertEqual(empty["counts"]["packets"], 0)

    def test_eht_is_detected_but_explicitly_unsupported(self):
        result = decoder.analyze(pcap([(0, 0, radio(action(b"\x24\x00" + bytes(5))), 0)]), AP)
        self.assertEqual(result["counts"]["feedback_candidates"], 1)
        self.assertEqual(result["rejections"], {"unsupported_eht_feedback": 1})
        self.assertEqual(result["status"], "UNDECODED_REPORTS")

    def test_invalid_duration_and_protected(self):
        for duration in (0, -1, float("nan"), float("inf")):
            self.assertReason("invalid_capture_duration", lambda: decoder.analyze(pcap([]), AP, duration))
        frame = bytearray(action())
        frame[1] |= 0x40
        result = decoder.analyze(pcap([(0, 0, radio(frame), 0)]), AP)
        self.assertEqual(result["rejections"], {"protected_action": 1})

    def test_packet_clock_regression_invalidates_rates(self):
        capture = pcap([(2, 0, radio(action()), 0), (1, 0, radio(action(sequence=8)), 0)])
        result = decoder.analyze(capture, AP, duration=30)
        self.assertEqual(result["packet_timestamp_regressions"], 1)
        self.assertEqual(result["rate_invalid_reason"], "packet_timestamp_regression")
        self.assertIsNone(result["unique_reports_per_second"])
        self.assertIsNone(result["raw_decoded_reports_per_second"])
        self.assertFalse(result["timing_valid"])

    def test_all_prefixes_fail_safely(self):
        for fixture in (vht_body(), he_body(), he20_body(1, 1), he20_body(2, 0),
                        he40_body(1, 0), he40_body(2, 1)):
            for size in range(len(fixture)):
                with self.assertRaises(decoder.DecodeError):
                    decoder.decode_report(fixture[:size])


if __name__ == "__main__":
    unittest.main()
