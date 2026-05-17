#!/usr/bin/env python3
"""
Record an empty-room baseline for the RuView sensing-server.

ADR-103 v2 — persistent baseline override that's stable across NBVI
re-selection between server restarts. Computes baseline from the FULL
amplitude vector (all non-zero subcarriers), not from the dynamic NBVI
top-K subset.

Usage:
    1. Operator steps out of the room.
    2. Run:  scripts/record-baseline.py [--duration 90] [--server localhost]
    3. Wait for the "saved" message. Operator can come back.
    4. Restart sensing-server to pick up the new baseline.

The script connects to the live WebSocket stream, records `duration`
seconds of per-node amplitudes, trims the first and last 15 seconds
(catches door-opening transients), then for each node finds the most
stable 30-second sub-window (lowest broadband CV) and writes per-node
full-broadband mean / median / p95 to data/baseline.json.
"""

import argparse
import asyncio
import json
import statistics
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

try:
    import websockets
except ImportError:
    print("error: pip install websockets", file=sys.stderr)
    sys.exit(2)


def full_broadband_mean(amps):
    """Mean over all non-zero subcarriers (skips guard tones)."""
    valid = [v for v in amps if v > 0]
    return (sum(valid) / len(valid)) if valid else 0.0


async def record(server: str, duration: float, port: int):
    by_node: dict[int, list[tuple[float, list[float], float]]] = {}
    url = f"ws://{server}:{port}/ws/sensing"
    start = time.time()
    print(f"connecting to {url} — recording {duration:.0f}s …", flush=True)
    async with websockets.connect(url) as ws:
        async for msg in ws:
            d = json.loads(msg)
            if d.get("type") != "sensing_update":
                continue
            t = time.time() - start
            for n in d.get("nodes") or []:
                a = n.get("amplitude") or []
                if not a:
                    continue
                by_node.setdefault(n["node_id"], []).append((t, a, n.get("rssi_dbm", 0.0)))
            if time.time() - start >= duration:
                break
    return by_node


def trim_and_clean(frames, trim_head_sec=15.0, trim_tail_sec=15.0, clean_window_sec=30.0):
    """Trim head/tail transients, then scan for the cleanest sub-window."""
    if not frames:
        return None
    t0 = frames[0][0]
    t1 = frames[-1][0]
    dur = t1 - t0
    if dur < trim_head_sec + trim_tail_sec + clean_window_sec / 2:
        head = dur / 6
        tail = dur / 6
    else:
        head = trim_head_sec
        tail = trim_tail_sec
    trimmed = [f for f in frames if t0 + head <= f[0] <= t1 - tail]
    if not trimmed:
        return None

    win = clean_window_sec
    if (trimmed[-1][0] - trimmed[0][0]) <= win:
        chunk = trimmed
    else:
        best = None  # (cv, frames)
        step = 5.0
        cursor = trimmed[0][0]
        while cursor + win <= trimmed[-1][0]:
            window = [f for f in trimmed if cursor <= f[0] <= cursor + win]
            if len(window) >= 5:
                bms = [full_broadband_mean(a) for _, a, _ in window]
                mu = statistics.mean(bms)
                if mu > 0:
                    sd = statistics.pstdev(bms)
                    cv = sd / mu
                    if best is None or cv < best[0]:
                        best = (cv, window)
            cursor += step
        if best is None or not best[1]:
            return None
        chunk = best[1]

    # ── Compute per-node stats on the clean window ───────────────
    full_means = [full_broadband_mean(a) for _, a, _ in chunk]
    rssis = [r for _, _, r in chunk if r != 0]
    sorted_full = sorted(full_means)

    # Per-subcarrier mean across the clean window (for diagnostic + future
    # subcarrier-level comparison if the server gets that capability).
    n_sub = min(len(a) for _, a, _ in chunk)
    per_sub_means = []
    for k in range(n_sub):
        vs = [a[k] for _, a, _ in chunk if k < len(a) and a[k] > 0]
        per_sub_means.append(statistics.mean(vs) if vs else 0.0)

    return {
        # Persistent fields the server reads:
        "full_broadband_mean": statistics.mean(full_means),
        "full_broadband_p50":  sorted_full[len(sorted_full)//2],
        "full_broadband_p95":  sorted_full[int(len(sorted_full)*0.95)],
        "full_broadband_std":  statistics.pstdev(full_means),
        "full_broadband_cv_pct": 100*statistics.pstdev(full_means)/statistics.mean(full_means)
                                if statistics.mean(full_means) else 0.0,
        # Reference:
        "rssi_dbm": statistics.mean(rssis) if rssis else 0.0,
        "n_samples": len(full_means),
        "window_start_sec": chunk[0][0],
        "window_end_sec": chunk[-1][0],
        # Per-subcarrier diagnostic (kept so future server versions can do
        # subcarrier-level comparison without re-recording):
        "per_subcarrier_mean": [round(v, 3) for v in per_sub_means],
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[1])
    ap.add_argument("--duration", type=float, default=90.0, help="seconds to record (default 90)")
    ap.add_argument("--server", default="localhost", help="sensing-server host")
    ap.add_argument("--port", type=int, default=8765, help="ws port (default 8765)")
    ap.add_argument("--out", type=Path, default=Path("v2/data/baseline.json"))
    ap.add_argument("--trim-head", type=float, default=15.0)
    ap.add_argument("--trim-tail", type=float, default=15.0)
    ap.add_argument("--clean-window", type=float, default=30.0)
    args = ap.parse_args()

    by_node = asyncio.run(record(args.server, args.duration, args.port))
    if not by_node:
        print("no data received from server", file=sys.stderr)
        sys.exit(1)

    out = {
        "version": 2,
        "captured_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "duration_sec": args.duration,
        "trim_head_sec": args.trim_head,
        "trim_tail_sec": args.trim_tail,
        "clean_window_sec": args.clean_window,
        "method": "record → trim head/tail → find lowest-CV sub-window → FULL-broadband stats per node",
        "nodes": {},
    }
    print()
    for nid, frames in sorted(by_node.items()):
        result = trim_and_clean(frames, args.trim_head, args.trim_tail, args.clean_window)
        if not result:
            print(f"node {nid}: not enough data for cleaning (skipped)")
            continue
        out["nodes"][str(nid)] = result
        print(f"node {nid}: {len(frames)} raw frames, kept cleanest {result['n_samples']}-sample window")
        print(f"  FULL broadband: mean={result['full_broadband_mean']:.2f}  std={result['full_broadband_std']:.2f}  CV={result['full_broadband_cv_pct']:.2f}%")
        print(f"  full p50={result['full_broadband_p50']:.2f}  p95={result['full_broadband_p95']:.2f}  rssi={result['rssi_dbm']:.1f}")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(out, indent=2))
    print(f"\nsaved → {args.out}")
    print("restart sensing-server to load the new baseline.")


if __name__ == "__main__":
    main()
