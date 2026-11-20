#!/usr/bin/env python3
"""
RTL8721Dx (Realtek AmebaDplus) live CSI throughput/latency benchmark.

Listens on the wifi-densepose-sensing-server's CSI UDP port for real RAC1
frames (ADR-323, docs/adr/ADR-323-rtl8721dx-ameba-csi-wire-protocol.md) sent
by the ruview_csi_node firmware (branch feat/rtl8721dx-csi-bringup) running
on real RTL8721Dx hardware, and reports MEASURED — not simulated — throughput
and frame-rate statistics.

This does not itself trigger a board reset; run it while the firmware is
already connected and streaming (see CLAUDE.local.md for the reset+capture
pattern used to bring a fresh boot up during firmware iteration).

Usage:
    python scripts/benchmark-rtl8721dx-csi.py --duration 40
    python scripts/benchmark-rtl8721dx-csi.py --duration 40 --port 5005 --output results.json
"""

from __future__ import annotations

import argparse
import json
import socket
import struct
import sys
import time
from dataclasses import dataclass, asdict, field
from pathlib import Path

RAC1_MAGIC = bytes.fromhex("52414331")  # "RAC1" little-endian, see ADR-323
RHB1_MAGIC = bytes.fromhex("52484231")  # heartbeat marker (not a CSI frame)
RAC1_HEADER_LEN = 49


@dataclass
class Rac1Frame:
    recv_time: float
    length: int
    sequence: int
    timestamp_us: int
    channel: int
    bandwidth: int
    num_sub_carrier: int
    rssi_dbm: int
    csi_valid: int


def parse_rac1_header(data: bytes, recv_time: float) -> Rac1Frame | None:
    if len(data) < RAC1_HEADER_LEN or data[:4] != RAC1_MAGIC:
        return None
    sequence = struct.unpack_from("<I", data, 13)[0]
    timestamp_us = struct.unpack_from("<I", data, 17)[0]
    channel = data[33]
    bandwidth = data[34]
    num_sub_carrier = struct.unpack_from("<H", data, 37)[0]
    rssi_dbm = struct.unpack_from("<b", data, 41)[0]
    csi_valid = data[43]
    return Rac1Frame(
        recv_time=recv_time,
        length=len(data),
        sequence=sequence,
        timestamp_us=timestamp_us,
        channel=channel,
        bandwidth=bandwidth,
        num_sub_carrier=num_sub_carrier,
        rssi_dbm=rssi_dbm,
        csi_valid=csi_valid,
    )


@dataclass
class BenchmarkResult:
    duration_s: float
    total_datagrams: int
    rac1_frames: int
    heartbeat_packets: int
    other_packets: int
    rac1_bytes: int
    rac1_fps: float
    rac1_bytes_per_sec: float
    sequence_gaps: int
    first_sequence: int | None
    last_sequence: int | None
    rssi_min_dbm: int | None
    rssi_max_dbm: int | None
    rssi_mean_dbm: float | None
    inter_frame_interval_ms_mean: float | None
    inter_frame_interval_ms_p95: float | None
    channels_seen: list[int] = field(default_factory=list)
    evidence: str = "MEASURED"


def run_benchmark(port: int, duration: float, bind_ip: str = "0.0.0.0") -> BenchmarkResult:
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind((bind_ip, port))
    sock.settimeout(0.5)

    frames: list[Rac1Frame] = []
    total = 0
    heartbeats = 0
    other = 0

    deadline = time.time() + duration
    print(f"Listening on {bind_ip}:{port} for {duration}s ...", file=sys.stderr)
    while time.time() < deadline:
        try:
            data, _addr = sock.recvfrom(4096)
        except socket.timeout:
            continue
        recv_time = time.time()
        total += 1
        if data[:4] == RHB1_MAGIC:
            heartbeats += 1
            continue
        frame = parse_rac1_header(data, recv_time)
        if frame is not None:
            frames.append(frame)
        else:
            other += 1
    sock.close()

    rac1_bytes = sum(f.length for f in frames)
    seqs = [f.sequence for f in frames]
    rssis = [f.rssi_dbm for f in frames]
    channels = sorted({f.channel for f in frames})

    gaps = 0
    for a, b in zip(seqs, seqs[1:]):
        if b != a + 1:
            gaps += 1

    intervals_ms = [
        (b.recv_time - a.recv_time) * 1000.0 for a, b in zip(frames, frames[1:])
    ]
    intervals_ms.sort()

    def percentile(values: list[float], p: float) -> float | None:
        if not values:
            return None
        idx = min(len(values) - 1, int(len(values) * p))
        return values[idx]

    return BenchmarkResult(
        duration_s=duration,
        total_datagrams=total,
        rac1_frames=len(frames),
        heartbeat_packets=heartbeats,
        other_packets=other,
        rac1_bytes=rac1_bytes,
        rac1_fps=len(frames) / duration if duration > 0 else 0.0,
        rac1_bytes_per_sec=rac1_bytes / duration if duration > 0 else 0.0,
        sequence_gaps=gaps,
        first_sequence=seqs[0] if seqs else None,
        last_sequence=seqs[-1] if seqs else None,
        rssi_min_dbm=min(rssis) if rssis else None,
        rssi_max_dbm=max(rssis) if rssis else None,
        rssi_mean_dbm=(sum(rssis) / len(rssis)) if rssis else None,
        inter_frame_interval_ms_mean=(
            sum(intervals_ms) / len(intervals_ms) if intervals_ms else None
        ),
        inter_frame_interval_ms_p95=percentile(intervals_ms, 0.95),
        channels_seen=channels,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--port", type=int, default=5005, help="UDP port to listen on (default: 5005, matches wifi-densepose-sensing-server)")
    parser.add_argument("--bind-ip", type=str, default="0.0.0.0")
    parser.add_argument("--duration", type=float, default=40.0, help="capture window in seconds")
    parser.add_argument("--output", type=Path, default=None, help="write results as JSON to this path")
    args = parser.parse_args()

    result = run_benchmark(args.port, args.duration, args.bind_ip)

    print(f"\n=== RTL8721Dx RAC1 CSI benchmark ({result.evidence}) ===")
    print(f"duration:              {result.duration_s:.1f}s")
    print(f"total UDP datagrams:   {result.total_datagrams}")
    print(f"RAC1 CSI frames:       {result.rac1_frames}")
    print(f"heartbeat packets:     {result.heartbeat_packets}")
    print(f"other/unrecognized:    {result.other_packets}")
    print(f"RAC1 throughput:       {result.rac1_fps:.2f} fps, {result.rac1_bytes_per_sec:.1f} bytes/s")
    print(f"sequence gaps:         {result.sequence_gaps} (dropped/reordered frames)")
    if result.first_sequence is not None:
        print(f"sequence range:        {result.first_sequence} .. {result.last_sequence}")
    if result.rssi_mean_dbm is not None:
        print(f"RSSI:                  min={result.rssi_min_dbm} max={result.rssi_max_dbm} mean={result.rssi_mean_dbm:.1f} dBm")
    if result.inter_frame_interval_ms_mean is not None:
        print(f"inter-frame interval:  mean={result.inter_frame_interval_ms_mean:.1f}ms p95={result.inter_frame_interval_ms_p95:.1f}ms")
    print(f"channels seen:         {result.channels_seen}")

    if args.output:
        args.output.write_text(json.dumps(asdict(result), indent=2))
        print(f"\nwrote {args.output}", file=sys.stderr)

    return 0


if __name__ == "__main__":
    sys.exit(main())
