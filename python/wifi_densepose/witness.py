"""ADR-117 §11.3 / ADR-028 — v2 witness: a deterministic proof run of the
compiled Rust vitals pipeline.

The v1 proof (``archive/v1/data/proof/verify.py``) hashes the output of the
legacy pure-Python CSI processor. This module is its v2 counterpart: it feeds
the same synthetic reference signal (``sample_csi_data.json``) through the
Rust pipeline bound in ``wifi_densepose._native`` and hashes the output.

Pipeline (all three stages are Rust, called through the PyO3 bindings):

1. ``CsiVitalPreprocessor.esp32_default()``  -> per-subcarrier residuals
2. ``BreathingExtractor.esp32_default().extract(residuals, [])``
3. ``HeartRateExtractor.esp32_default().extract(residuals, phases)``

Canonical input: antenna 0 of each frame (56 amplitudes, 56 phases).

Canonical bytes, in frame order, after the ``SCHEMA`` header:

* frame index as little-endian ``int64``
* residual count as ``uint32``, then each residual as ``float64``
* for breathing, then heart rate: one byte (0 = no estimate, 1 = estimate),
  followed, when present, by ``value_bpm`` and ``confidence`` as ``float64``
  and the status code as ``uint8``

Every float is rounded to ``DECIMALS`` places and ``-0.0`` is folded into
``0.0`` before packing. The heart-rate stage calls libm ``exp``/``cos``,
whose last bit may differ across platforms; rounding keeps the hash stable
across builds of the same code (the same reason v1 quantizes, RuView#560).
The committed ``expected_features_v2.sha256`` is produced on Linux x86_64.

Usage::

    python -I -m wifi_densepose.witness --data sample_csi_data.json
    python -I -m wifi_densepose.witness --data sample_csi_data.json \\
        --expected expected_features_v2.sha256

Run with ``-I`` (or from outside the RuView checkout): the repository root
contains a legacy ``wifi_densepose/`` directory that would shadow the wheel.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import struct
import sys
from dataclasses import dataclass
from typing import Any

from wifi_densepose._native import (
    BreathingExtractor,
    CsiVitalPreprocessor,
    HeartRateExtractor,
    VitalStatus,
)

SCHEMA = b"wifi-densepose/witness-v2/1\n"
"""Header hashed before any frame. Bump it if the canonical form changes."""

DECIMALS = 6
"""Decimal places every float is rounded to before hashing."""

_STATUS_CODE = {
    VitalStatus.Valid: 0,
    VitalStatus.Degraded: 1,
    VitalStatus.Unreliable: 2,
    VitalStatus.Unavailable: 3,
}


@dataclass(frozen=True)
class WitnessResult:
    """Outcome of one witness run."""

    sha256: str
    frames: int
    breathing_estimates: int
    heart_rate_estimates: int


def _q(x: float) -> bytes:
    if not math.isfinite(x):
        raise ValueError(f"non-finite value in pipeline output: {x!r}")
    # `+ 0.0` folds -0.0 into 0.0 so the sign of zero never changes the hash.
    return struct.pack("<d", round(x, DECIMALS) + 0.0)


def _estimate_bytes(est: Any) -> bytes:
    if est is None:
        return b"\x00"
    return (
        b"\x01"
        + _q(est.value_bpm)
        + _q(est.confidence)
        + struct.pack("<B", _STATUS_CODE[est.status])
    )


def compute_witness(data_path: str | os.PathLike[str]) -> WitnessResult:
    """Run the reference signal through the Rust pipeline and hash the output."""
    with open(data_path, encoding="utf-8") as f:
        data = json.load(f)
    frames = data["frames"]
    if not frames:
        raise ValueError(f"{data_path}: no frames")

    pre = CsiVitalPreprocessor.esp32_default()
    br = BreathingExtractor.esp32_default()
    hr = HeartRateExtractor.esp32_default()

    hasher = hashlib.sha256(SCHEMA)
    n_br = n_hr = 0
    for frame in frames:
        amplitudes = [float(v) for v in frame["amplitude"][0]]
        phases = [float(v) for v in frame["phase"][0]]
        residuals = pre.process(amplitudes)
        if residuals is None:
            raise ValueError(f"frame {frame['frame_index']}: empty amplitude vector")
        br_est = br.extract(residuals, [])
        hr_est = hr.extract(residuals, phases)
        n_br += br_est is not None
        n_hr += hr_est is not None

        hasher.update(struct.pack("<q", int(frame["frame_index"])))
        hasher.update(struct.pack("<I", len(residuals)))
        for r in residuals:
            hasher.update(_q(r))
        hasher.update(_estimate_bytes(br_est))
        hasher.update(_estimate_bytes(hr_est))

    return WitnessResult(hasher.hexdigest(), len(frames), n_br, n_hr)


def _read_expected(expected: str | os.PathLike[str]) -> str:
    text = str(expected)
    if len(text) == 64 and all(c in "0123456789abcdef" for c in text.lower()):
        return text.lower()
    with open(expected, encoding="utf-8") as f:
        token = f.read().split()
    if not token:
        raise ValueError(f"{expected}: empty hash file")
    return token[0].lower()


def verify_witness(
    data_path: str | os.PathLike[str], expected: str | os.PathLike[str]
) -> bool:
    """Return True if the pipeline output hash matches ``expected``.

    ``expected`` is either a 64-character hex digest or the path of a file
    whose first token is one (e.g. ``expected_features_v2.sha256``).
    """
    return compute_witness(data_path).sha256 == _read_expected(expected)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python -m wifi_densepose.witness",
        description="ADR-117 §11.3 v2 witness: hash the Rust vitals pipeline output.",
    )
    parser.add_argument("--data", required=True, help="path to sample_csi_data.json")
    parser.add_argument(
        "--expected", help="expected hash file (or hex digest); omit to just print the hash"
    )
    args = parser.parse_args(argv)

    result = compute_witness(args.data)
    print(f"frames:               {result.frames}")
    print(f"breathing estimates:  {result.breathing_estimates}")
    print(f"heart-rate estimates: {result.heart_rate_estimates}")
    print(f"sha256:               {result.sha256}")
    if args.expected is None:
        return 0
    want = _read_expected(args.expected)
    if result.sha256 == want:
        print("VERDICT: PASS")
        return 0
    print(f"expected:             {want}")
    print("VERDICT: FAIL")
    return 1


if __name__ == "__main__":
    sys.exit(main())
