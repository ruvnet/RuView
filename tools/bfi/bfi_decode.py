#!/usr/bin/env python3
"""Offline BFI acceptance decoder. Python standard library only; never captures.

Supported: radiotap classic PCAP; complete VHT SU 2x1/2x2, 20/40/80 MHz;
HE SU 2x1/2x2, full 20/40/80 MHz, Ng=4, RU indices 0..8/0..17/0..36.
Other variants fail closed.
The reconstructed V is quantized steering information, NOT recovered full CSI.

Field references: IEEE 802.11ac-2013 sections 8.4.1.47-48, 8.5.23.2;
Wireshark packet-ieee80211.c (9209acd8): HE MIMO masks / next_he_scidx;
https://arxiv.org/abs/2309.04408 (quantization and steering reconstruction).
No external implementation code is incorporated. Tests use synthetic bytes.
"""

import argparse
import cmath
import collections
import functools
import hashlib
import json
import math
import pathlib
import re
import statistics
import struct
import sys
import zlib

MAX_FILE = 128 * 1024 * 1024
MAX_PACKET = 65535
MAX_PACKETS = 200000
MAX_LINKS = 256
# SU codebooks contain 16*4 and 64*16 angle pairs, for each of Nc=1 and Nc=2.
MAX_STEERING_CACHE = 2 * (16 * 4 + 64 * 16)
# Number of feedback tones; never infer a tone count from payload length.
VHT_TONES = {(20, 1): 52, (20, 2): 30, (20, 4): 16,
             (40, 1): 108, (40, 2): 58, (40, 4): 30,
             (80, 1): 234, (80, 2): 122, (80, 4): 62}
# Wireshark v4.2.2 scidx_{20,40,80}MHz_Ng4 and next_he_scidx.
# 20: [-122, -120:4:-4, -2, 2, 4:4:120, 122]; 40: [-244:4:-4, 4:4:244];
# 80: [-500:4:-4, 4:4:500]. Only these complete ranges are accepted.
HE_FULL_BAND_TONES = {(20, 4, 0, 8): 64, (40, 4, 0, 17): 122, (80, 4, 0, 36): 250}


class DecodeError(ValueError):
    def __init__(self, reason):
        super().__init__(reason)
        self.reason = reason


def require(condition, reason):
    if not condition:
        raise DecodeError(reason)


def mac(value):
    require(bool(re.fullmatch(r"[0-9a-fA-F]{2}(?::[0-9a-fA-F]{2}){5}", value)),
            "invalid_mac")
    return bytes.fromhex(value.replace(":", ""))


def pcap_packets(stream):
    """Yield integer nanosecond timestamp, bytes, snaplen-truncated marker."""
    header = stream.read(24)
    require(len(header) == 24, "pcap_header_truncated")
    formats = {b"\xd4\xc3\xb2\xa1": ("<", 1000), b"\xa1\xb2\xc3\xd4": (">", 1000),
               b"\x4d\x3c\xb2\xa1": ("<", 1), b"\xa1\xb2\x3c\x4d": (">", 1)}
    require(header[:4] in formats, "unsupported_pcap_format_pcapng_not_supported")
    endian, multiplier = formats[header[:4]]
    major, minor, _, _, snaplen, linktype = struct.unpack(endian + "HHIIII", header[4:])
    require((major, minor) == (2, 4), "unsupported_pcap_version")
    # Reject additional linktype flags rather than silently guessing FCS handling.
    require(linktype == 127, "unsupported_linktype_expected_radiotap_127")
    require(0 < snaplen <= MAX_PACKET, "invalid_snaplen")
    size = 24
    count = 0
    while True:
        record = stream.read(16)
        if not record:
            return
        count += 1
        require(count <= MAX_PACKETS, "packet_limit_exceeded")
        require(len(record) == 16, "pcap_record_header_truncated")
        seconds, fraction, captured, original = struct.unpack(endian + "IIII", record)
        require(fraction < 1_000_000_000 // multiplier, "invalid_timestamp_fraction")
        require(captured <= snaplen and captured <= original, "invalid_record_lengths")
        size += 16 + captured
        require(size <= MAX_FILE, "file_limit_exceeded")
        packet = stream.read(captured)
        require(len(packet) == captured, "pcap_record_data_truncated")
        yield seconds * 1_000_000_000 + fraction * multiplier, packet, captured < original


def strip_radiotap(packet):
    require(len(packet) >= 8, "radiotap_truncated")
    version, _, length, present = struct.unpack_from("<BBHI", packet)
    require(version == 0 and 8 <= length <= len(packet), "invalid_radiotap_header")
    first_present = present
    offset = 8
    words = 1
    while present & (1 << 31):
        require(words < 8 and offset + 4 <= length, "invalid_radiotap_presence")
        present = struct.unpack_from("<I", packet, offset)[0]
        offset += 4
        words += 1
    # TSFT and Flags precede every other radiotap field; no guesses about later fields.
    if first_present & 1:
        offset = (offset + 7) & ~7
        require(offset + 8 <= length, "radiotap_tsft_truncated")
        offset += 8
    flags = 0
    if first_present & 2:
        require(offset < length, "radiotap_flags_truncated")
        flags = packet[offset]
    require(not flags & 0x40, "radiotap_bad_fcs")
    frame = packet[length:]
    if flags & 0x10:
        require(len(frame) >= 4, "fcs_truncated")
        expected = struct.unpack_from("<I", frame, len(frame) - 4)[0]
        frame = frame[:-4]
        require(zlib.crc32(frame) == expected, "fcs_mismatch")
    return frame


def decode_angles(data, tone_count, phi_bits, psi_bits):
    """2-row matrices have one phi/psi pair, including Nc=2 (no extra column)."""
    bit_count = tone_count * (phi_bits + psi_bits)
    require(len(data) == (bit_count + 7) // 8, "angle_payload_length_mismatch")
    packed = int.from_bytes(data, "little")
    require(packed >> bit_count == 0, "nonzero_angle_padding")
    values = []
    for _ in range(tone_count):
        phi_index = packed & ((1 << phi_bits) - 1)
        packed >>= phi_bits
        psi_index = packed & ((1 << psi_bits) - 1)
        packed >>= psi_bits
        phi = (2 * phi_index + 1) * math.pi / (1 << phi_bits)
        psi = (2 * psi_index + 1) * math.pi / (1 << (psi_bits + 2))
        values.append((phi_index, psi_index, phi, psi))
    return values


def steering_matrix(phi, psi, nc):
    phase = cmath.exp(1j * phi)
    c, s = math.cos(psi), math.sin(psi)
    return [[phase * c, -phase * s][:nc], [complex(s), complex(c)][:nc]]


@functools.lru_cache(maxsize=MAX_STEERING_CACHE)
def _steering_with_residual(phi, psi, nc):
    """Cache only immutable quantized matrices and their fully computed residual."""
    matrix = steering_matrix(phi, psi, nc)
    residual = max(abs(sum(row[i].conjugate() * row[j] for row in matrix)
                       - (1 if i == j else 0))
                   for i in range(nc) for j in range(nc))
    return tuple(tuple(row) for row in matrix), residual


def decode_report(body):
    require(len(body) >= 2, "action_header_truncated")
    category, action = body[:2]
    require((category, action) != (36, 0), "unsupported_eht_feedback")
    require((category, action) in ((21, 0), (30, 0)), "unsupported_action")
    standard = "VHT" if category == 21 else "HE"
    control_length = 3 if standard == "VHT" else 5
    require(len(body) >= 2 + control_length, "mimo_control_truncated")
    control = int.from_bytes(body[2:2 + control_length], "little")
    nc, nr = (control & 7) + 1, ((control >> 3) & 7) + 1
    width = (20, 40, 80, 160)[(control >> 6) & 3]
    require(nc <= nr, "invalid_matrix_dimensions")
    require(nr == 2 and nc in (1, 2), "unsupported_matrix_dimensions")
    require((control >> 12) & 7 == 0 and (control >> 15) & 1 == 1,
            "unsupported_segmented_or_empty_feedback")
    if standard == "VHT":
        require(control & (3 << 16) == 0, "unsupported_vht_reserved_bits")
        group_index = (control >> 8) & 3
        require(group_index < 3, "invalid_vht_grouping")
        ng = (1, 2, 4)[group_index]
        feedback, codebook = (control >> 11) & 1, (control >> 10) & 1
        token = (control >> 18) & 63
        require(feedback == 0, "unsupported_vht_mu_exclusive_report")
        require((width, ng) in VHT_TONES, "unsupported_vht_bandwidth")
        tones = VHT_TONES[(width, ng)]
    else:
        require(control >> 36 == 0, "unsupported_he_extension_or_reserved_bits")
        ng = (4, 16)[(control >> 8) & 1]
        feedback, codebook = (control >> 10) & 3, (control >> 9) & 1
        token = (control >> 30) & 63
        require(feedback == 0, "unsupported_he_mu_cqi_or_reserved_feedback")
        ru_start, ru_end = (control >> 16) & 127, (control >> 23) & 127
        require(ru_start <= ru_end, "invalid_he_ru_range")
        require((width, ng, ru_start, ru_end) in HE_FULL_BAND_TONES,
                "unsupported_he_bandwidth_grouping_or_partial_ru")
        tones = HE_FULL_BAND_TONES[(width, ng, ru_start, ru_end)]
    phi_bits, psi_bits = (4, 2) if codebook == 0 else (6, 4)
    payload = body[2 + control_length:]
    expected = nc + (tones * (phi_bits + psi_bits) + 7) // 8
    require(len(payload) == expected, "report_payload_length_mismatch")
    snr_codes = [x if x < 128 else x - 256 for x in payload[:nc]]
    angles = decode_angles(payload[nc:], tones, phi_bits, psi_bits)
    matrices = []
    residual = 0.0
    for _, _, phi, psi in angles:
        matrix, tone_residual = _steering_with_residual(phi, psi, nc)
        # Reports own their mutable rows: callers cannot mutate a cached matrix
        # or a different tone/report that happened to use the same angle pair.
        matrices.append([list(row) for row in matrix])
        residual = max(residual, tone_residual)
    return {"standard": standard, "nr": nr, "nc": nc, "bandwidth_mhz": width,
            "grouping": ng, "feedback": "SU", "codebook": codebook, "token": token,
            "tone_count": tones, "phi_bits": phi_bits, "psi_bits": psi_bits,
            "snr_codes": snr_codes, "snr_db": [22 + x / 4 for x in snr_codes],
            "snr_endpoint_saturated": [x in (-128, 127) for x in snr_codes],
            "angles": angles, "matrices": matrices,
            "orthonormality_max_residual": residual}


def gap_summary(timestamps):
    # Never hide clock regressions by sorting samples.
    gaps = [(b - a) / 1e9 for a, b in zip(timestamps, timestamps[1:]) if b >= a]
    ordered = sorted(gaps)
    return {"gap_count": len(gaps),
            "median_gap_seconds": statistics.median(gaps) if gaps else None,
            "p95_gap_seconds": ordered[math.ceil(.95 * len(ordered)) - 1] if gaps else None,
            "max_gap_seconds": max(gaps) if gaps else None,
            "timestamp_regressions": sum(b < a for a, b in zip(timestamps, timestamps[1:]))}


def analyze(stream, ap, duration=None, kernel_drops=None):
    counts = collections.Counter({name: 0 for name in (
        "packets", "non_management", "other_ap_management", "target_management",
        "target_beacons", "target_actions", "other_actions", "feedback_candidates",
        "decoded_reports", "retry_duplicates", "unique_reports")})
    rejected = collections.Counter()
    links = {}
    seen = collections.OrderedDict()
    first = last = previous_packet_timestamp = None
    packet_timestamp_regressions = 0
    max_residual = 0.0
    for timestamp, packet, truncated in pcap_packets(stream):
        counts["packets"] += 1
        if previous_packet_timestamp is not None and timestamp < previous_packet_timestamp:
            packet_timestamp_regressions += 1
        previous_packet_timestamp = timestamp
        first = timestamp if first is None else min(first, timestamp)
        last = timestamp if last is None else max(last, timestamp)
        try:
            require(not truncated, "snaplen_truncated")
            frame = strip_radiotap(packet)
            require(len(frame) >= 2, "frame_control_truncated")
            fc = int.from_bytes(frame[:2], "little")
            require(fc & 3 == 0, "unsupported_80211_version")
            if (fc >> 2) & 3 != 0:
                counts["non_management"] += 1
                continue
            require(len(frame) >= 24, "management_header_truncated")
            a1, a2, a3 = frame[4:10], frame[10:16], frame[16:22]
            if ap not in (a1, a2, a3):
                counts["other_ap_management"] += 1
                continue
            counts["target_management"] += 1
            subtype = (fc >> 4) & 15
            if subtype == 8:
                counts["target_beacons"] += 1
            if subtype not in (13, 14):
                continue
            counts["target_actions"] += 1
            require(not fc & 0x4000, "protected_action")
            require(not fc & 0x8000, "unsupported_ordered_action")
            require(not fc & 0x0300, "invalid_management_ds_bits")
            require(not fc & 0x0400 and frame[22] & 15 == 0, "unsupported_fragmented_action")
            body = frame[24:]
            require(len(body) >= 2, "action_header_truncated")
            if (body[0], body[1]) not in ((21, 0), (30, 0), (36, 0)):
                counts["other_actions"] += 1
                continue
            counts["feedback_candidates"] += 1
            report = decode_report(body)
            counts["decoded_reports"] += 1
            key = (a1, a2, frame[22:24], hashlib.sha256(body).digest())
            previous = seen.get(key)
            duplicate = bool(fc & 0x0800 and previous is not None
                             and 0 <= timestamp - previous <= 1_000_000_000)
            seen[key] = timestamp
            seen.move_to_end(key)
            if len(seen) > 4096:
                seen.popitem(last=False)
            if duplicate:
                counts["retry_duplicates"] += 1
                continue
            link_key = (a1, a2, report["standard"], report["bandwidth_mhz"],
                        report["nc"], report["grouping"], report["codebook"])
            require(link_key in links or len(links) < MAX_LINKS, "link_limit_exceeded")
            counts["unique_reports"] += 1
            if link_key not in links:
                links[link_key] = {"id": "link-" + str(len(links) + 1),
                                   "standard": report["standard"],
                                   "bandwidth_mhz": report["bandwidth_mhz"],
                                   "nc": report["nc"], "grouping": report["grouping"],
                                   "tone_count": report["tone_count"], "timestamps": []}
            links[link_key]["timestamps"].append(timestamp)
            max_residual = max(max_residual, report["orthonormality_max_residual"])
        except DecodeError as error:
            if error.reason == "link_limit_exceeded":
                raise  # A resource limit must not produce a partial acceptance summary.
            rejected[error.reason] += 1
    span = (last - first) / 1e9 if first is not None else None
    if duration is not None:
        require(math.isfinite(duration) and 0 < duration <= 86400, "invalid_capture_duration")
        require(span is None or span <= duration + .001, "capture_duration_shorter_than_packet_span")
    denominator = duration if duration is not None else span
    rate_invalid_reason = None
    if packet_timestamp_regressions:
        rate_invalid_reason = "packet_timestamp_regression"
        denominator = None
    elif not denominator:
        rate_invalid_reason = "capture_duration_or_nonzero_packet_span_required"
    link_summaries = []
    for link in links.values():
        timestamps = link.pop("timestamps")
        link.update(gap_summary(timestamps))
        link["unique_reports"] = len(timestamps)
        link["reports_per_second"] = len(timestamps) / denominator if denominator else None
        link_summaries.append(link)
    status = ("DECODED_REPORTS" if counts["unique_reports"] else
              "UNDECODED_REPORTS" if counts["feedback_candidates"] else
              "NO_FEEDBACK_REPORTS" if counts["target_beacons"] else
              "NO_TARGET_BEACONS")
    return {"schema_version": 1, "status": status, "input_provenance": "user_supplied_pcap",
            # Successful decoding alone does not independently verify this capture.
            "decoder_validation": "requires_independent_capture_verification",
            "capture_duration_seconds": duration, "packet_timestamp_span_seconds": span,
            "packet_timestamp_regressions": packet_timestamp_regressions,
            "timing_valid": packet_timestamp_regressions == 0,
            "rate_invalid_reason": rate_invalid_reason,
            "rate_basis": "capture_duration" if duration is not None else "packet_timestamp_span",
            "counts": dict(counts), "rejections": dict(rejected), "links": link_summaries,
            "kernel_drops": kernel_drops,
            "unique_reports_per_second": counts["unique_reports"] / denominator if denominator else None,
            "raw_decoded_reports_per_second": counts["decoded_reports"] / denominator if denominator else None,
            "orthonormality_max_residual": max_residual if counts["unique_reports"] else None,
            "motion_tested": False,
            "limitations": ["No full CSI recovery or motion inference", "Kernel drops do not measure radio loss",
                            "Unsupported MU, segmentation, HE partial RU/Ng16 and EHT are not decoded",
                            "Repeated non-retry frames are retained; no payload-only deduplication"]}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pcap", type=pathlib.Path)
    parser.add_argument("--ap", required=True, help="Own-network AP BSSID; not printed in summary")
    parser.add_argument("--duration", type=float, help="Actual capture wall duration in seconds")
    parser.add_argument("--kernel-drops", type=int, help="From capture stderr; absent means unknown")
    args = parser.parse_args(argv)
    try:
        ap = mac(args.ap)
        require(args.pcap.is_file(), "input_is_not_regular_file")
        require(args.pcap.stat().st_size <= MAX_FILE, "file_limit_exceeded")
        require(args.kernel_drops is None or args.kernel_drops >= 0, "invalid_kernel_drop_count")
        with args.pcap.open("rb") as stream:
            result = analyze(stream, ap, args.duration, args.kernel_drops)
        print(json.dumps(result, indent=2, allow_nan=False))
        return 0 if result["status"] == "DECODED_REPORTS" else 2
    except (DecodeError, OSError) as error:
        print(json.dumps({"status": "INPUT_ERROR", "reason": str(error)}))
        return 1


if __name__ == "__main__":
    sys.exit(main())
