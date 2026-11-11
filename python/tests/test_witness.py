"""ADR-117 §11.3 — v2 witness: the Rust vitals pipeline is deterministic and
matches the committed ``expected_features_v2.sha256``."""

from __future__ import annotations

import pathlib

import pytest

from wifi_densepose import CsiVitalPreprocessor, verify_witness
from wifi_densepose.witness import compute_witness

PROOF = pathlib.Path(__file__).resolve().parents[2] / "archive" / "v1" / "data" / "proof"
DATA = PROOF / "sample_csi_data.json"
EXPECTED = PROOF / "expected_features_v2.sha256"

needs_data = pytest.mark.skipif(not DATA.exists(), reason="reference signal not in this checkout")


def test_preprocessor_seeds_then_tracks() -> None:
    pre = CsiVitalPreprocessor(4, alpha=0.5)
    assert pre.process([1.0, 2.0, 3.0, 4.0]) == [0.0, 0.0, 0.0, 0.0]
    assert pre.process([3.0, 2.0, 3.0, 4.0]) == [2.0, 0.0, 0.0, 0.0]
    assert pre.process([]) is None


@needs_data
def test_witness_is_deterministic() -> None:
    a = compute_witness(DATA)
    b = compute_witness(DATA)
    assert a == b
    assert a.frames == 1000
    assert a.heart_rate_estimates > 0


@needs_data
@pytest.mark.skipif(
    not EXPECTED.exists() or not EXPECTED.read_text().strip(),
    reason="expected_features_v2.sha256 not committed yet",
)
def test_witness_matches_committed_hash() -> None:
    assert verify_witness(str(DATA), str(EXPECTED))
