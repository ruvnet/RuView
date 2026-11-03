#!/usr/bin/env python3
"""
presence_to_pose.py — RSSI motion-energy → sensing-server presence injector.

Polls the macos-rssi-bridge HTTP endpoint at /aps for live per-AP RSSI
variance, and POSTs a single PersonDetection to the sensing-server's
/api/v1/pose/external endpoint so the 3D Observatory renders one honest
figure (instead of the heuristic five-skeleton fallback).

Honest about what's real:
  - Position: PINNED at the laptop (origin). With one macOS RSSI receiver
    and unknown AP locations, RSSI variance can only detect that *something*
    moved in the environment — it cannot tell us where. Earlier versions
    fabricated a centroid from hash-positioned APs; that produced wildly
    wrong jumps and is gone.
  - Confidence + motion: derived from total per-AP RSSI variance. Higher
    variance → higher confidence and a more animated standing pose.
  - Pose: STATIC standing skeleton. No joint estimation from RSSI; that
    needs CSI hardware.
  - Count: capped at 1.
"""
from __future__ import annotations

import argparse
import json
import math
import time
import urllib.error
import urllib.request
from typing import Any


def standing_keypoints(
    px: float, pz: float, conf: float, motion: float, t: float
) -> list[dict[str, Any]]:
    """17-point COCO standing skeleton anchored at (px, 0, pz).

    Mirrors PoseSystem.poseStanding() in ui/observatory/js/pose-system.js
    so the basic /ui/index.html viewer also renders something coherent.
    `motion` ∈ [0..1] modulates a weight-shift sway so the figure looks
    alive when there's real RSSI activity — explicitly *not* derived
    pose, just animation seeded by motion energy so a frozen-looking
    figure doesn't suggest a frozen sensor. Amplitudes are tuned for
    visibility at the Observatory's orbit camera distance (~5–10 m).
    """
    sway_x = math.sin(t * 1.4) * 0.20 * motion
    sway_z = math.cos(t * 0.9) * 0.14 * motion
    head_turn = math.sin(t * 0.5) * 0.12 * (0.3 + motion)
    shoulder_dip = math.sin(t * 1.4) * 0.08 * motion
    knee_bend_l = max(0.0, math.sin(t * 1.4)) * 0.18 * motion
    knee_bend_r = max(0.0, -math.sin(t * 1.4)) * 0.18 * motion

    layout = [
        ("nose",            (sway_x + head_turn, 1.72,  sway_z)),
        ("left_eye",        (-0.03 + sway_x + head_turn, 1.74, -0.02 + sway_z)),
        ("right_eye",       (0.03 + sway_x + head_turn, 1.74, -0.02 + sway_z)),
        ("left_ear",        (-0.07 + sway_x, 1.72,  sway_z)),
        ("right_ear",       (0.07 + sway_x, 1.72,  sway_z)),
        ("left_shoulder",   (-0.22 + sway_x * 0.7, 1.48 - shoulder_dip,  sway_z * 0.7)),
        ("right_shoulder",  (0.22 + sway_x * 0.7, 1.48 + shoulder_dip,  sway_z * 0.7)),
        ("left_elbow",      (-0.24 + sway_x * 0.5, 1.18 - shoulder_dip,  0.02 + sway_z * 0.5)),
        ("right_elbow",     (0.24 + sway_x * 0.5, 1.18 + shoulder_dip,  0.02 + sway_z * 0.5)),
        ("left_wrist",      (-0.26 + sway_x * 0.3, 0.92 - shoulder_dip,  0.04)),
        ("right_wrist",     (0.26 + sway_x * 0.3, 0.92 + shoulder_dip,  0.04)),
        ("left_hip",        (-0.14, 1.00,  0.00)),
        ("right_hip",       (0.14, 1.00,  0.00)),
        ("left_knee",       (-0.15, 0.55 + knee_bend_l,  0.00)),
        ("right_knee",      (0.15, 0.55 + knee_bend_r,  0.00)),
        ("left_ankle",      (-0.16, 0.10,  0.00)),
        ("right_ankle",     (0.16, 0.10,  0.00)),
    ]
    return [
        {"name": name, "x": px + dx, "y": dy, "z": pz + dz, "confidence": conf}
        for name, (dx, dy, dz) in layout
    ]


def total_variance(aps: list[dict[str, Any]]) -> float:
    """Sum of per-AP RSSI variance — the only honest motion signal we have."""
    return sum(max(0.0, ap.get("variance", 0.0)) for ap in aps)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--bridge-url", default="http://127.0.0.1:9090/aps",
                    help="macos-rssi-bridge /aps endpoint")
    ap.add_argument("--server-url", default="http://127.0.0.1:8080/api/v1/pose/external",
                    help="sensing-server pose ingestion endpoint")
    ap.add_argument("--rate-hz", type=float, default=10.0,
                    help="how often to POST a pose (Hz)")
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    period = 1.0 / args.rate_hz
    last_log = 0.0
    n_posted = 0
    n_fail = 0

    print(f"[presence] {args.bridge_url} → {args.server_url} @ {args.rate_hz:.1f} Hz",
          flush=True)

    while True:
        loop_start = time.time()
        try:
            with urllib.request.urlopen(args.bridge_url, timeout=1.0) as r:
                snap = json.loads(r.read())
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as e:
            n_fail += 1
            if args.verbose:
                print(f"[presence] bridge fetch failed: {e}", flush=True)
            time.sleep(period)
            continue

        aps = snap.get("aps", [])
        weight = total_variance(aps)

        # Confidence ~ how much variance is present. Caps at ~1.0 once
        # there's a few dB² of cumulative motion energy across APs.
        conf = max(0.3, min(1.0, 0.3 + weight / 10.0))
        motion = max(0.0, min(1.0, weight / 5.0))
        t = time.time()

        # Position is pinned at the laptop. RSSI variance from one receiver
        # cannot localize — it can only flag that something moved.
        px, pz = 0.0, 0.0

        person = {
            "id": 1,
            "confidence": conf,
            "keypoints": standing_keypoints(px, pz, conf, motion, t),
            "bbox": {"x": px - 0.4, "y": 0.0, "width": 0.8, "height": 1.85},
            "zone": "rssi_presence",
        }
        body = json.dumps([person]).encode("utf-8")

        try:
            req = urllib.request.Request(
                args.server_url,
                data=body,
                headers={"Content-Type": "application/json"},
                method="POST",
            )
            with urllib.request.urlopen(req, timeout=1.0) as r:
                _ = r.read()
            n_posted += 1
        except urllib.error.URLError as e:
            n_fail += 1
            if args.verbose:
                print(f"[presence] post failed: {e}", flush=True)

        now = time.time()
        if args.verbose and now - last_log > 1.0:
            print(
                f"[presence] aps={len(aps):>2} weight={weight:>5.2f} "
                f"conf={conf:.2f} motion={motion:.2f} posted={n_posted} fail={n_fail}",
                flush=True,
            )
            last_log = now

        elapsed = time.time() - loop_start
        if elapsed < period:
            time.sleep(period - elapsed)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        print("\n[presence] stopped")
