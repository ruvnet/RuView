#!/usr/bin/env python3
"""
ESP32 (S3/C6) live CSI throughput and continuity benchmark.

The ESP32 counterpart of benchmark-rtl8721dx-csi.py (RuView#1943): listens on
the sensing-server's UDP port for real ADR-018 CSI frames (magic 0xC5110001)
from esp32-csi-node firmware on real hardware and reports MEASURED frame rate,
sequence loss, and -- for RuView#1941 -- capture continuity: every interval
with no CSI frame longer than --outage-ms, with its start time and length.

Other packet types on the same port (vitals 0xC5110002, ADR-110 sync
0xC511A110, feature vectors, adaptive-controller/mesh frames) are counted
separately. They keep flowing during a capture stall, which is why a node can
look alive while delivering no CSI.

Run it while the firmware is streaming to this host; it does not reset the
board. Stop any sensing-server bound to the same port first.

Usage:
    python scripts/benchmark-esp32-csi.py --duration 60
    python scripts/benchmark-esp32-csi.py --duration 300 --source-ip 192.168.1.86 --output c6.json
"""

from __future__ import annotations

import argparse
import json
import socket
import struct
import sys
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path

CSI_MAGIC = 0xC5110001
VITALS_MAGIC = 0xC5110002
SYNC_MAGIC = 0xC511A110
CSI_HEADER_LEN = 20


@dataclass
class BenchmarkResult:
    duration_s: float
    source_ip: str | None
    total_datagrams: int
    csi_frames: int
    vitals_packets: int
    sync_packets: int
    other_packets: int
    csi_fps: float
    csi_bytes_per_sec: float
    sequence_gaps: int
    frames_lost: int
    loss_pct: float
    node_ids: list[int]
    rssi_mean_dbm: float | None
    inter_frame_ms_p50: float | None
    inter_frame_ms_p95: float | None
    inter_frame_ms_p99: float | None
    max_csi_gap_s: float | None
    outages: list[dict] = field(default_factory=list)
    sequence_gap_events: list[dict] = field(default_factory=list)
    evidence: str = "MEASURED"


def percentile(values: list[float], p: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(len(ordered) * p))]


def run_benchmark(port: int, duration: float, bind_ip: str, source_ip: str | None,
                  outage_ms: float) -> BenchmarkResult:
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 1 << 20)
    sock.bind((bind_ip, port))
    sock.settimeout(0.25)

    start = time.monotonic()
    deadline = start + duration
    total = vitals = sync = other = 0
    csi_times: list[float] = []
    csi_bytes = 0
    seqs: list[int] = []
    rssis: list[int] = []
    node_ids: set[int] = set()

    print(f"Listening on {bind_ip}:{port} for {duration:.0f}s"
          f"{' from ' + source_ip if source_ip else ''} ...", file=sys.stderr)
    while time.monotonic() < deadline:
        try:
            data, addr = sock.recvfrom(4096)
        except socket.timeout:
            continue
        now = time.monotonic() - start
        if source_ip and addr[0] != source_ip:
            continue
        total += 1
        if len(data) < 4:
            other += 1
            continue
        magic = struct.unpack_from("<I", data, 0)[0]
        if magic == CSI_MAGIC and len(data) >= CSI_HEADER_LEN:
            csi_times.append(now)
            csi_bytes += len(data)
            node_ids.add(data[4])
            seqs.append(struct.unpack_from("<I", data, 12)[0])
            rssis.append(struct.unpack_from("<b", data, 16)[0])
        elif magic == VITALS_MAGIC:
            vitals += 1
        elif magic == SYNC_MAGIC:
            sync += 1
        else:
            other += 1
    sock.close()

    gaps = lost = 0
    gap_events: list[dict] = []
    for i, (a, b) in enumerate(zip(seqs, seqs[1:])):
        step = (b - a) & 0xFFFFFFFF
        if step != 1:
            gaps += 1
            if 1 < step < 1_000_000:        # a reboot resets seq; not "lost"
                lost += step - 1
            if len(gap_events) < 50:
                gap_events.append({"at_s": round(csi_times[i + 1], 3), "from_seq": a,
                                   "to_seq": b, "wall_gap_s": round(csi_times[i + 1] - csi_times[i], 3)})

    # Include the capture window edges so a stall at the start or end counts.
    edges = [0.0] + csi_times + [duration]
    intervals = [(b - a) for a, b in zip(csi_times, csi_times[1:])]
    outages = [
        {"start_s": round(a, 3), "length_s": round(b - a, 3)}
        for a, b in zip(edges, edges[1:]) if (b - a) * 1000.0 >= outage_ms
    ]
    received = len(seqs)
    return BenchmarkResult(
        duration_s=duration,
        source_ip=source_ip,
        total_datagrams=total,
        csi_frames=received,
        vitals_packets=vitals,
        sync_packets=sync,
        other_packets=other,
        csi_fps=received / duration if duration > 0 else 0.0,
        csi_bytes_per_sec=csi_bytes / duration if duration > 0 else 0.0,
        sequence_gaps=gaps,
        frames_lost=lost,
        loss_pct=(100.0 * lost / (received + lost)) if (received + lost) else 0.0,
        node_ids=sorted(node_ids),
        rssi_mean_dbm=(sum(rssis) / len(rssis)) if rssis else None,
        inter_frame_ms_p50=(lambda v: v * 1000 if v is not None else None)(percentile(intervals, 0.50)),
        inter_frame_ms_p95=(lambda v: v * 1000 if v is not None else None)(percentile(intervals, 0.95)),
        inter_frame_ms_p99=(lambda v: v * 1000 if v is not None else None)(percentile(intervals, 0.99)),
        max_csi_gap_s=max((b - a) for a, b in zip(edges, edges[1:])),
        outages=outages,
        sequence_gap_events=gap_events,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--port", type=int, default=5005)
    parser.add_argument("--bind-ip", default="0.0.0.0")
    parser.add_argument("--source-ip", default=None, help="only count datagrams from this node")
    parser.add_argument("--duration", type=float, default=60.0, help="capture window in seconds")
    parser.add_argument("--outage-ms", type=float, default=2000.0,
                        help="report CSI gaps at least this long (default 2000)")
    parser.add_argument("--output", type=Path, default=None, help="write results as JSON")
    args = parser.parse_args()

    r = run_benchmark(args.port, args.duration, args.bind_ip, args.source_ip, args.outage_ms)

    print(f"\n=== ESP32 ADR-018 CSI benchmark ({r.evidence}) ===")
    print(f"duration:            {r.duration_s:.1f}s  source={r.source_ip or 'any'}  nodes={r.node_ids}")
    print(f"datagrams:           {r.total_datagrams} (csi={r.csi_frames} vitals={r.vitals_packets} "
          f"sync={r.sync_packets} other={r.other_packets})")
    print(f"CSI throughput:      {r.csi_fps:.2f} fps, {r.csi_bytes_per_sec:.0f} bytes/s")
    print(f"sequence:            {r.sequence_gaps} gaps, {r.frames_lost} lost ({r.loss_pct:.2f}%)")
    if r.rssi_mean_dbm is not None:
        print(f"RSSI mean:           {r.rssi_mean_dbm:.1f} dBm")
    if r.inter_frame_ms_p50 is not None:
        print(f"inter-frame:         p50={r.inter_frame_ms_p50:.1f}ms p95={r.inter_frame_ms_p95:.1f}ms "
              f"p99={r.inter_frame_ms_p99:.1f}ms")
    if r.max_csi_gap_s is not None:
        print(f"longest CSI gap:     {r.max_csi_gap_s:.2f}s")
    print(f"outages >= {args.outage_ms:.0f}ms:   {len(r.outages)}")
    for o in r.outages:
        print(f"  at {o['start_s']:8.2f}s  for {o['length_s']:.2f}s")
    for g in r.sequence_gap_events[:10]:
        print(f"  seq gap at {g['at_s']:8.2f}s: {g['from_seq']} -> {g['to_seq']} "
              f"(wall gap {g['wall_gap_s']:.2f}s)")

    if args.output:
        args.output.write_text(json.dumps(asdict(r), indent=2))
        print(f"\nwrote {args.output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
