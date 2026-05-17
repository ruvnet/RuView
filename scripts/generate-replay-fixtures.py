#!/usr/bin/env python3
"""ADR-114: generate 1000 idle + 1000 motion CSI replay fixtures.

Two files are written under
`v2/crates/wifi-densepose-sensing-server/tests/fixtures/`:

* `replay_idle.jsonl`   — 1000 frames of empty-room baseline +
                          per-frame Gaussian noise (low CV).
* `replay_motion.jsonl` — 1000 frames of the same baseline + 1.5 Hz
                          coherent modulation + per-frame Gaussian
                          noise (high CV).

Format: one JSON object per line:
    {"node_id": <u8>, "amplitude": [<f64>; 56]}

These are *synthetic but parameter-matched to live data* (baseline
mean = 27.04 / 14.72 from data/baseline.json, CV ≈ 2.6 / 3.6 %).
They exist to provide deterministic regression coverage of the
amp_presence_override classifier. Real captured-from-sensor fixtures
can replace them in-place (same filename, same line format) without
changing the test code.

Deterministic by seed so the test result is reproducible across
machines. Re-run only when you want to regenerate.
"""

from __future__ import annotations

import json
import math
import random
from pathlib import Path

OUT_DIR = (
    Path(__file__).resolve().parent.parent
    / "v2"
    / "crates"
    / "wifi-densepose-sensing-server"
    / "tests"
    / "fixtures"
)

# Per-node baseline mean amplitude pulled from a real recording of
# this deployment (data/baseline.json). Holding them in code keeps
# the fixture script self-contained.
NODE_BASELINES = {1: 27.04, 2: 14.72}
N_SUB = 56
FRAMES_PER_NODE = 500  # 500 × 2 nodes = 1000 per fixture file


def gen_subcarrier_profile(rng: random.Random, mean: float) -> list[float]:
    """Static per-subcarrier mean profile — same for the whole capture."""
    return [max(1.0, mean * rng.uniform(0.7, 1.3)) for _ in range(N_SUB)]


def write_fixture(path: Path, motion: bool, seed: int) -> int:
    rng = random.Random(seed)
    profiles = {
        nid: gen_subcarrier_profile(rng, mean) for nid, mean in NODE_BASELINES.items()
    }
    count = 0
    with path.open("w") as f:
        # Interleave nodes round-robin so the test driver gets per-node
        # streams of the same length, like a real WS feed.
        for i in range(FRAMES_PER_NODE):
            for nid, profile in profiles.items():
                t = i / 20.0  # 20 Hz tick
                # AMP_SHORT_WIN in the server is 90 frames = 4.5 s.
                # Idle: small per-frame noise → rolling-window CV stays
                # well below the universal threshold.
                # Motion: a slow ~0.15 Hz coherent envelope (6.7 s cycle,
                # longer than the 4.5 s averaging window) drives the
                # broadband mean up/down by ±40 %, producing a high
                # rolling CV. Mimics body position changes during
                # walking — the channel response shifts slowly relative
                # to the classifier window.
                if motion:
                    envelope = 1.0 + 0.40 * math.sin(2 * math.pi * 0.15 * t)
                else:
                    envelope = 1.0
                amps: list[float] = []
                for mu in profile:
                    noise_sigma = mu * (0.05 if motion else 0.018)
                    n = rng.gauss(0.0, noise_sigma)
                    amps.append(round(mu * envelope + n, 3))
                f.write(json.dumps({"node_id": nid, "amplitude": amps}) + "\n")
                count += 1
    return count


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    idle_path = OUT_DIR / "replay_idle.jsonl"
    motion_path = OUT_DIR / "replay_motion.jsonl"
    n_idle = write_fixture(idle_path, motion=False, seed=42)
    n_motion = write_fixture(motion_path, motion=True, seed=43)
    print(f"wrote {n_idle} idle frames    → {idle_path}")
    print(f"wrote {n_motion} motion frames → {motion_path}")
    print()
    print("These fixtures are SYNTHETIC parameter-matched to live data —")
    print("the cargo test that consumes them measures classifier")
    print("consistency, not real-world accuracy. Replace with live")
    print("captures (same line format, same filenames) when operator")
    print("time allows for a true empty-vs-walking ground-truth pair.")


if __name__ == "__main__":
    main()
