#!/usr/bin/env python3
"""Compare local decoded integer angles against independently executed tshark 4.2.2.

No capture or network access. Raw frames go only to the local tshark stdin.
Default output contains aggregate comparison results, not addresses or angles.
VHT frequency-axis labels are deliberately not validated: tshark 4.2.2 uses
sequential SCIDX labels in the tested Ng=4 report. This checks angle order,
all angle integers, MIMO controls, and signed SNR, not frequency-axis fidelity.
"""
import argparse
import collections
import io
import json
import os
import pathlib
import re
import shutil
import struct
import subprocess
import sys
import threading
import xml.etree.ElementTree as ET

import bfi_decode as decoder

ORACLE_VERSION = "4.2.2"
MAX_OUTPUT = 4 * 1024 * 1024
MAX_REPORTS = 128


def run_bounded(command, data=b"", timeout=15):
    """Bound time and both output pipes; no shell, temp capture, or live interface."""
    env = dict(os.environ, LC_ALL="C", LANG="C")
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, env=env)
    output = {}
    failures = []

    def read_pipe(name, pipe, limit):
        output[name] = pipe.read(limit + 1)
        if len(output[name]) > limit:
            failures.append("oracle_output_limit_exceeded")
            process.kill()

    def feed():
        try:
            process.stdin.write(data)
            process.stdin.close()
        except (BrokenPipeError, OSError):
            pass

    threads = [threading.Thread(target=read_pipe, args=("stdout", process.stdout, MAX_OUTPUT), daemon=True),
               threading.Thread(target=read_pipe, args=("stderr", process.stderr, 65536), daemon=True),
               threading.Thread(target=feed, daemon=True)]
    for thread in threads:
        thread.start()
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        failures.append("oracle_timeout")
        process.kill()
        process.wait(timeout=5)
    finally:
        for thread in threads:
            thread.join(timeout=2)
        for pipe in (process.stdin, process.stdout, process.stderr):
            pipe.close()
    decoder.require(not failures, failures[0] if failures else "oracle_failed")
    decoder.require(process.returncode == 0, "oracle_nonzero_exit")
    return output["stdout"]


def pcap_for_packet(packet):
    decoder.require(len(packet) <= decoder.MAX_PACKET, "packet_limit_exceeded")
    return (struct.pack("<IHHIIII", 0xa1b2c3d4, 2, 4, 0, 0, 65535, 127)
            + struct.pack("<IIII", 1, 0, len(packet), len(packet)) + packet)


def pdml_values(data, report):
    decoder.require(len(data) <= MAX_OUTPUT, "oracle_output_limit_exceeded")
    decoder.require(b"<!DOCTYPE" not in data.upper() and b"<!ENTITY" not in data.upper(),
                    "unsafe_oracle_xml")
    try:
        root = ET.fromstring(data)
    except ET.ParseError as error:
        raise decoder.DecodeError("invalid_oracle_xml") from error
    decoder.require(len(root.findall("packet")) == 1, "oracle_expected_one_packet")
    decoder.require(not any(x.get("name") == "_ws.malformed" for x in root.iter()), "oracle_malformed_packet")
    fields = collections.defaultdict(list)
    for field in root.iter("field"):
        fields[field.get("name", "")].append(field.get("show", ""))
    if report["standard"] == "VHT":
        prefix = "wlan.vht.mimo_control."
        names = ("ncindex", "nrindex", "chanwidth", "grouping", "codebookinfo", "feedbacktype", "sounding_dialog_token_nbr")
        angle_name = "wlan.vht.compressed_beamforming_report.scidx"
        snr_name = "wlan.vht.compressed_beamforming_report.snr"
        expected_group = {1: 0, 2: 1, 4: 2}[report["grouping"]]
    else:
        prefix = "wlan.he.mimo."
        names = ("nc_index", "nr_index", "bw", "grouping", "codebook_info", "feedback_type", "sounding_dialog_token_num")
        angle_name = "wlan.he.action.he_mimo_control.scidx"
        snr_name = "wlan.he.mimo.beamforming_report.avgsnr"
        expected_group = 0
    expected = [report["nc"] - 1, report["nr"] - 1,
                {20: 0, 40: 1, 80: 2}[report["bandwidth_mhz"]],
                expected_group, report["codebook"], 0, report["token"]]
    actual = []
    try:
        for name in names:
            values = fields[prefix + name]
            decoder.require(len(values) == 1, "oracle_control_field_missing_or_ambiguous")
            actual.append(int(values[0], 0))
        snr = [int(value, 0) for value in fields[snr_name]]
    except ValueError as error:
        if isinstance(error, decoder.DecodeError):
            raise
        raise decoder.DecodeError("invalid_oracle_numeric_field") from error
    angles = []
    for value in fields[angle_name]:
        match = re.fullmatch(r"-?\d+,\s*φ11:(\d+),\s*ψ21:(\d+)", value)
        decoder.require(match is not None, "unsupported_oracle_angle_display")
        angles.append((int(match[1]), int(match[2])))
    return {"controls_match": actual == expected, "snr_match": snr == report["snr_codes"],
            "angles_match": angles == [tuple(angle[:2]) for angle in report["angles"]],
            "oracle_angle_count": len(angles), "decoder_angle_count": report["tone_count"]}


def compare_packet(tshark, packet, report):
    pdml = run_bounded([tshark, "-n", "-r", "-", "-T", "pdml"], pcap_for_packet(packet))
    result = pdml_values(pdml, report)
    result["standard"] = report["standard"]
    result["matches"] = all(result[key] for key in ("controls_match", "snr_match", "angles_match"))
    return result


def supported_reports(stream, ap, limit):
    selected = []
    rejected = collections.Counter()
    for number, (_, packet, truncated) in enumerate(decoder.pcap_packets(stream), 1):
        try:
            decoder.require(not truncated, "snaplen_truncated")
            frame = decoder.strip_radiotap(packet)
            decoder.require(len(frame) >= 24, "short_frame")
            fc = int.from_bytes(frame[:2], "little")
            if (fc & 15) != 0 or ((fc >> 4) & 15) not in (13, 14):
                continue
            if ap not in (frame[4:10], frame[10:16], frame[16:22]):
                continue
            decoder.require(not fc & 0xc700 and frame[22] & 15 == 0, "unsupported_action_flags")
            if frame[24:26] not in (b"\x15\x00", b"\x1e\x00", b"\x24\x00"):
                continue
            report = decoder.decode_report(frame[24:])
            selected.append((number, packet, report))
            if len(selected) == limit:
                break
        except decoder.DecodeError as error:
            rejected[error.reason] += 1
    return selected, dict(rejected)


def golden_packets():
    """Literal SYNTHETIC wire bytes, independent of decoder encode logic."""
    ap, sta = bytes.fromhex("020000000001"), bytes.fromhex("020000000002")
    # First two phi/psi pairs are (5,2),(9,1). Remaining pairs are zero.
    cases = [("VHT-small", "1500898200807f6506", 45),
             ("HE-small", "1e008980001200807f6506", 186),
             ("VHT-large", "1500898600807f852401", 75),
             ("HE-large", "1e008982001200807f852401", 310)]
    for name, prefix, tail_length in cases:
        body = bytes.fromhex(prefix) + bytes(tail_length)
        frame = struct.pack("<HH", 0x00e0, 0) + ap + sta + ap + bytes(2) + body
        packet = bytes.fromhex("0000080000000000") + frame
        yield name, packet, decoder.decode_report(body)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pcap", type=pathlib.Path, nargs="?")
    parser.add_argument("--ap", help="Own-network AP BSSID for a real capture")
    parser.add_argument("--tshark", default="tshark", help="Local executable; required version 4.2.2")
    parser.add_argument("--max-reports", type=int, default=8,
                        help=f"Maximum supported reports to compare (1..{MAX_REPORTS}; default: 8)")
    parser.add_argument("--self-test", action="store_true", help="Compare four SYNTHETIC literal fixtures")
    args = parser.parse_args(argv)
    try:
        decoder.require(1 <= args.max_reports <= MAX_REPORTS, "invalid_report_limit")
        decoder.require(args.self_test != (args.pcap is not None), "choose_self_test_or_pcap")
        tshark = shutil.which(args.tshark)
        decoder.require(tshark is not None, "tshark_not_found")
        version = run_bounded([tshark, "--version"]).decode("utf-8", errors="replace").splitlines()[0]
        decoder.require(bool(re.search(r"TShark \(Wireshark\) 4\.2\.2\b", version)), "unsupported_oracle_version")
        if args.self_test:
            selected, rejected = list(golden_packets()), {}
        else:
            decoder.require(args.ap is not None, "ap_required")
            ap = decoder.mac(args.ap)
            decoder.require(args.pcap.is_file() and args.pcap.stat().st_size <= decoder.MAX_FILE,
                            "input_missing_or_too_large")
            with args.pcap.open("rb") as stream:
                selected, rejected = supported_reports(stream, ap, args.max_reports)
        results = []
        for frame_id, packet, report in selected:
            result = compare_packet(tshark, packet, report)
            result["fixture" if args.self_test else "frame_number"] = frame_id
            results.append(result)
        matched = bool(results) and all(result["matches"] for result in results)
        status = "MATCH" if matched else "MISMATCH" if results else "NO_SUPPORTED_REPORTS"
        print(json.dumps({"status": status, "evidence": "SYNTHETIC" if args.self_test else "CAPTURED_BYTES",
                          "oracle_version": version, "reports_compared": len(results),
                          "results": results, "rejections": rejected,
                          "frequency_axis_validated": False, "matrix_or_motion_validated": False,
                          "scope": "Every integer angle, selected MIMO controls and signed SNR in selected reports"}, indent=2))
        return 0 if matched else 2
    except (decoder.DecodeError, OSError, subprocess.SubprocessError) as error:
        print(json.dumps({"status": "ORACLE_ERROR", "reason": str(error)}))
        return 1


if __name__ == "__main__":
    sys.exit(main())
