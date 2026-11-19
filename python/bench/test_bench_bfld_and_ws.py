"""ADR-117 hardening sweep — Benchmarks for the P3.5 numpy bridge
and the P4 WS decoder.

The numpy bridge is the most-likely candidate for a hidden allocation
hot-spot: every `BfldFrame.from_compressed_feedback()` call copies the
ndarray into a Vec<Complex64>. Confirm the per-frame cost is
acceptable for the BFR cadence the AP emits (typically a few
hundred per second, not thousands).

The WS decoder runs once per frame the sensing-server emits. At
worst-case ~100 Hz × number-of-subscribers, the decoder budget is
tight; make sure dataclass construction doesn't dominate.
"""

from __future__ import annotations

import json

import numpy as np
import pytest

from wifi_densepose import BfldFrame, BfldKind


@pytest.mark.parametrize("kind,shape", [
    (BfldKind.UncompressedHT20, (1, 1, 52)),
    (BfldKind.CompressedHE20, (2, 1, 242)),
    (BfldKind.CompressedHE80, (2, 1, 996)),
    (BfldKind.CompressedHE160, (2, 2, 1992)),
])
def test_bfld_from_compressed_feedback(
    benchmark, kind: BfldKind, shape: tuple[int, int, int]
) -> None:
    rng = np.random.default_rng(seed=42)
    fb = (rng.standard_normal(shape) + 1j * rng.standard_normal(shape)).astype(np.complex128)

    def _build():
        return BfldFrame.from_compressed_feedback(
            timestamp_ms=0,
            sounding_index=0,
            sta_mac="aa:bb:cc:dd:ee:ff",
            kind=kind,
            feedback_matrix=fb,
        )

    benchmark(_build)


def test_bfld_feedback_matrix_roundtrip(benchmark) -> None:
    """How expensive is the numpy-out round-trip? Used by clients
    that want to do further analysis in numpy after constructing
    the frame."""
    rng = np.random.default_rng(seed=42)
    shape = (2, 1, 996)
    fb = (rng.standard_normal(shape) + 1j * rng.standard_normal(shape)).astype(np.complex128)
    frame = BfldFrame.from_compressed_feedback(
        timestamp_ms=0,
        sounding_index=0,
        sta_mac="aa:bb:cc:dd:ee:ff",
        kind=BfldKind.CompressedHE80,
        feedback_matrix=fb,
    )
    benchmark(frame.feedback_matrix)


# ─── WS decoder ──────────────────────────────────────────────────────


_EDGE_VITALS_FRAME = json.dumps({
    "type": "edge_vitals",
    "node_id": "bench-node",
    "presence": True,
    "fall_detected": False,
    "motion": 0.34,
    "breathing_rate_bpm": 14.2,
    "heartrate_bpm": 72.5,
    "n_persons": 1,
    "motion_energy": 0.04,
    "presence_score": 0.91,
    "rssi": -42.0,
})


def test_ws_decoder_edge_vitals(benchmark) -> None:
    from wifi_densepose.client.ws import _decode

    def _decode_one():
        return _decode(_EDGE_VITALS_FRAME)

    benchmark(_decode_one)


_POSE_FRAME = json.dumps({
    "type": "pose_data",
    "node_id": "bench-node",
    "timestamp": 1700000000.5,
    "persons": [
        {"id": i, "keypoints": [[0.5, 0.5, 0.9] for _ in range(17)]}
        for i in range(3)
    ],
    "confidence": 0.85,
})


def test_ws_decoder_pose_data(benchmark) -> None:
    """The pose_data frame is typically the largest one the server
    emits — bench it separately so a future blob-size regression
    in the persons array is visible."""
    from wifi_densepose.client.ws import _decode

    def _decode_one():
        return _decode(_POSE_FRAME)

    benchmark(_decode_one)
