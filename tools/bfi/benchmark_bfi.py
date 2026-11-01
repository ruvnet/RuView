#!/usr/bin/env python3
"""Offline CPU benchmark against a trusted saved decoder source file.

Imports the supplied baseline as Python code. Never captures or uses the network.
Real PCAPs stay on the machine running this command; output is aggregate only.
"""
import argparse
import hashlib
import importlib.util
import io
import json
import pathlib
import platform
import random
import statistics
import struct
import sys
import time
import tracemalloc

import bfi_decode as candidate


def load_baseline(path):
    candidate.require(path.is_file() and path.stat().st_size <= 1024 * 1024,
                      "baseline_missing_or_too_large")
    spec = importlib.util.spec_from_file_location("bfi_benchmark_baseline", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def exact_value(value):
    """Preserve float bits, signed zero, complex components and sequence types."""
    if isinstance(value, float):
        return {"float64": struct.pack("<d", value).hex()}
    if isinstance(value, complex):
        return {"complex128": struct.pack("<dd", value.real, value.imag).hex()}
    if isinstance(value, (tuple, list)):
        return {type(value).__name__: [exact_value(item) for item in value]}
    if isinstance(value, dict):
        return {key: exact_value(item) for key, item in value.items()}
    return value


def digest(value):
    encoded = json.dumps(exact_value(value), sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(encoded.encode()).hexdigest()


def synthetic_bodies():
    """256 deterministic varied reports; both codebooks, standards and Nc values."""
    rng = random.Random(0xBF1)
    bodies = []
    for he in (False, True):
        for nc in (1, 2):
            for codebook, bits in ((0, 6), (1, 10)):
                tones = 250 if he else 62
                for token in range(32):
                    control = (0x12008088 if he else 0x8288) + nc - 1
                    control |= codebook << (9 if he else 10)
                    control |= token << (30 if he else 18)
                    angles = rng.getrandbits(tones * bits).to_bytes((tones * bits + 7) // 8, "little")
                    bodies.append(bytes((30 if he else 21, 0))
                                  + control.to_bytes(5 if he else 3, "little")
                                  + bytes((128, 127))[:nc] + angles)
    return bodies


def captured_bodies(module, data, ap):
    bodies = []
    for _, packet, truncated in module.pcap_packets(io.BytesIO(data)):
        try:
            if truncated:
                continue
            frame = module.strip_radiotap(packet)
            if len(frame) < 26 or ap not in (frame[4:10], frame[10:16], frame[16:22]):
                continue
            fc = int.from_bytes(frame[:2], "little")
            if fc & 15 or ((fc >> 4) & 15) not in (13, 14):
                continue
            if frame[24:26] not in (b"\x15\x00", b"\x1e\x00", b"\x24\x00"):
                continue
            if fc & 0xc700 or frame[22] & 15:
                continue
            bodies.append(frame[24:])
        except module.DecodeError:
            continue
    return bodies


def decode_outcome(module, body):
    try:
        return module.decode_report(body)
    except module.DecodeError as error:
        return {"rejection": error.reason}


def verify_reports(baseline, bodies):
    result_hash = hashlib.sha256()
    for body in bodies:
        before = digest(decode_outcome(baseline, body))
        after = digest(decode_outcome(candidate, body))
        candidate.require(before == after, "report_output_mismatch")
        result_hash.update(before.encode())
    return result_hash.hexdigest()


def clear_cache(module):
    cached = getattr(module, "_steering_with_residual", None)
    if cached is not None:
        cached.cache_clear()


def run_reports(module, bodies):
    for body in bodies:
        decode_outcome(module, body)


def benchmark_pair(baseline, task, repeats, iterations):
    modules = (baseline, candidate)
    cold = [[], []]
    warm = [[], []]
    # Alternate order to reduce bias from machine load or CPU frequency changes.
    for repeat in range(repeats):
        for index in ((0, 1) if repeat % 2 == 0 else (1, 0)):
            module = modules[index]
            clear_cache(module)
            start = time.perf_counter_ns()
            task(module)
            cold[index].append((time.perf_counter_ns() - start) / 1e6)
            start = time.perf_counter_ns()
            for _ in range(iterations):
                task(module)
            warm[index].append((time.perf_counter_ns() - start) / 1e6 / iterations)
    return {"cold_median_ms": [statistics.median(values) for values in cold],
            "warm_median_ms": [statistics.median(values) for values in warm],
            "warm_min_max_ms": [[min(values), max(values)] for values in warm],
            "warm_speedup": statistics.median(warm[0]) / statistics.median(warm[1]),
            "order": ["baseline", "candidate"]}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=pathlib.Path, required=True,
                        help="Trusted baseline Python source; executed by this benchmark")
    parser.add_argument("--pcap", type=pathlib.Path)
    parser.add_argument("--ap", help="Own-network BSSID; not printed")
    parser.add_argument("--repeats", type=int, default=7)
    parser.add_argument("--iterations", type=int, default=3)
    args = parser.parse_args(argv)
    sys.dont_write_bytecode = True
    try:
        candidate.require(1 <= args.repeats <= 20 and 1 <= args.iterations <= 100,
                          "invalid_benchmark_limits")
        candidate.require((args.pcap is None) == (args.ap is None), "pcap_and_ap_required_together")
        baseline = load_baseline(args.baseline)
        bodies = synthetic_bodies()
        verified = verify_reports(baseline, bodies)
        # Every truncated prefix of representative formats plus trailing bytes.
        malformed = [body[:size] for body in bodies[::32] for size in range(len(body))]
        malformed += [body + b"\0" for body in bodies[::32]]
        rejection_digest = verify_reports(baseline, malformed)
        result = {"evidence": "MEASURED_LOCAL_CPU", "python": platform.python_version(),
                  "platform": platform.platform(), "repeats": args.repeats,
                  "iterations_per_warm_sample": args.iterations,
                  "baseline_sha256": hashlib.sha256(args.baseline.read_bytes()).hexdigest(),
                  "candidate_sha256": hashlib.sha256(pathlib.Path(candidate.__file__).read_bytes()).hexdigest(),
                  "synthetic": {"reports": len(bodies), "bit_exact_digest": verified,
                                "rejection_cases": len(malformed), "rejection_digest": rejection_digest,
                                **benchmark_pair(baseline, lambda m: run_reports(m, bodies),
                                                 args.repeats, args.iterations)}}
        if args.pcap is not None:
            candidate.require(args.pcap.is_file() and args.pcap.stat().st_size <= candidate.MAX_FILE,
                              "pcap_missing_or_too_large")
            data, ap = args.pcap.read_bytes(), candidate.mac(args.ap)
            reports = captured_bodies(baseline, data, ap)
            before = baseline.analyze(io.BytesIO(data), ap)
            after = candidate.analyze(io.BytesIO(data), ap)
            candidate.require(digest(before) == digest(after), "capture_summary_mismatch")
            result["captured"] = {"sha256": hashlib.sha256(data).hexdigest(),
                                  "reports": len(reports), "bit_exact_digest": verify_reports(baseline, reports),
                                  "summary_bit_exact": True,
                                  "decode_reports": benchmark_pair(baseline, lambda m: run_reports(m, reports),
                                                                   args.repeats, args.iterations),
                                  "complete_analysis": benchmark_pair(baseline, lambda m: m.analyze(io.BytesIO(data), ap),
                                                                      args.repeats, args.iterations)}
        clear_cache(candidate)
        tracemalloc.start()
        run_reports(candidate, bodies)
        retained, peak = tracemalloc.get_traced_memory()
        tracemalloc.stop()
        result["cache"] = {**candidate._steering_with_residual.cache_info()._asdict(),
                           "retained_traced_bytes": retained, "peak_traced_bytes_including_report": peak}
        print(json.dumps(result, indent=2))
        return 0
    except (candidate.DecodeError, OSError, ValueError) as error:
        print(json.dumps({"status": "BENCHMARK_ERROR", "reason": str(error)}))
        return 1


if __name__ == "__main__":
    sys.exit(main())
