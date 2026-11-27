"""ADR-117 P3.5 — Tests for BFLD (Beamforming Feedback Loop Data) bindings.

These tests cover the *stub-Rust-backed* forward-compatible Python
surface defined in ADR-117 §5.7a. The real Rust ingestion crate
(`wifi-densepose-bfld`) lands post-v2.0; this test suite locks in the
Python API so a future swap-in is non-breaking.

Coverage:

- BfldKind enum — HE20/40/80/160 + HT20/40 variants
- BfldKind metadata getters — n_subcarriers, bandwidth_mhz, is_he
- BfldFrame.from_compressed_feedback — happy path + dim mismatch
- BfldFrame numpy round-trip — feedback_matrix returns ndarray
- BfldReport — frame aggregation, kind-mismatch error, coherence score
"""

from __future__ import annotations

import math

import numpy as np
import pytest

import wifi_densepose
from wifi_densepose import BfldFrame, BfldKind, BfldReport

# ─── BfldKind enum ───────────────────────────────────────────────────


def test_bfld_kind_variants_exist() -> None:
    assert BfldKind.CompressedHE20 != BfldKind.CompressedHE40
    assert BfldKind.CompressedHE80 != BfldKind.CompressedHE160
    assert BfldKind.UncompressedHT20 != BfldKind.UncompressedHT40


def test_bfld_kind_is_hashable() -> None:
    s = {BfldKind.CompressedHE80, BfldKind.CompressedHE80}
    assert len(s) == 1


def test_bfld_kind_n_subcarriers_he() -> None:
    assert BfldKind.CompressedHE20.n_subcarriers == 242
    assert BfldKind.CompressedHE40.n_subcarriers == 484
    assert BfldKind.CompressedHE80.n_subcarriers == 996
    assert BfldKind.CompressedHE160.n_subcarriers == 1992


def test_bfld_kind_n_subcarriers_ht() -> None:
    assert BfldKind.UncompressedHT20.n_subcarriers == 52
    assert BfldKind.UncompressedHT40.n_subcarriers == 108


def test_bfld_kind_bandwidth_mhz() -> None:
    assert BfldKind.CompressedHE20.bandwidth_mhz == 20
    assert BfldKind.CompressedHE40.bandwidth_mhz == 40
    assert BfldKind.CompressedHE80.bandwidth_mhz == 80
    assert BfldKind.CompressedHE160.bandwidth_mhz == 160
    assert BfldKind.UncompressedHT20.bandwidth_mhz == 20
    assert BfldKind.UncompressedHT40.bandwidth_mhz == 40


def test_bfld_kind_is_he_flag() -> None:
    assert BfldKind.CompressedHE20.is_he is True
    assert BfldKind.CompressedHE160.is_he is True
    assert BfldKind.UncompressedHT20.is_he is False
    assert BfldKind.UncompressedHT40.is_he is False


def test_bfld_kind_repr() -> None:
    r = repr(BfldKind.CompressedHE80)
    assert "BfldKind" in r and "CompressedHE80" in r


# ─── BfldFrame construction ──────────────────────────────────────────


def _make_matrix(n_rows: int, n_cols: int, n_subcarriers: int) -> np.ndarray:
    """Synthetic feedback matrix with non-trivial amplitudes so the
    mean_amplitude getter has something to chew on."""
    rng = np.random.default_rng(seed=42)
    real = rng.standard_normal((n_rows, n_cols, n_subcarriers)).astype(np.float64)
    imag = rng.standard_normal((n_rows, n_cols, n_subcarriers)).astype(np.float64)
    return (real + 1j * imag).astype(np.complex128)


def test_bfld_frame_he80_happy_path() -> None:
    fb = _make_matrix(2, 1, 996)
    frame = BfldFrame.from_compressed_feedback(
        timestamp_ms=1234,
        sounding_index=42,
        sta_mac="aa:bb:cc:dd:ee:ff",
        kind=BfldKind.CompressedHE80,
        feedback_matrix=fb,
    )
    assert frame.timestamp_ms == 1234
    assert frame.sounding_index == 42
    assert frame.sta_mac == "aa:bb:cc:dd:ee:ff"
    assert frame.kind == BfldKind.CompressedHE80
    assert frame.n_rows == 2
    assert frame.n_cols == 1
    assert frame.n_subcarriers == 996


def test_bfld_frame_he160_2x2() -> None:
    fb = _make_matrix(2, 2, 1992)
    frame = BfldFrame.from_compressed_feedback(
        timestamp_ms=0,
        sounding_index=0,
        sta_mac="00:00:00:00:00:00",
        kind=BfldKind.CompressedHE160,
        feedback_matrix=fb,
    )
    assert frame.n_rows == 2
    assert frame.n_cols == 2
    assert frame.n_subcarriers == 1992


def test_bfld_frame_ht20_legacy_path() -> None:
    fb = _make_matrix(1, 1, 52)
    frame = BfldFrame.from_compressed_feedback(
        timestamp_ms=0,
        sounding_index=0,
        sta_mac="aa:bb:cc:dd:ee:ff",
        kind=BfldKind.UncompressedHT20,
        feedback_matrix=fb,
    )
    assert frame.kind == BfldKind.UncompressedHT20
    assert frame.n_subcarriers == 52


def test_bfld_frame_subcarrier_dim_mismatch_raises() -> None:
    # HE80 requires 996 subcarriers; pass 64 → ValueError.
    bad = _make_matrix(2, 1, 64)
    with pytest.raises(ValueError, match="subcarrier"):
        BfldFrame.from_compressed_feedback(
            timestamp_ms=0,
            sounding_index=0,
            sta_mac="aa:bb:cc:dd:ee:ff",
            kind=BfldKind.CompressedHE80,
            feedback_matrix=bad,
        )


def test_bfld_frame_mean_amplitude_is_finite() -> None:
    fb = _make_matrix(2, 1, 996)
    frame = BfldFrame.from_compressed_feedback(
        timestamp_ms=0,
        sounding_index=0,
        sta_mac="aa:bb:cc:dd:ee:ff",
        kind=BfldKind.CompressedHE80,
        feedback_matrix=fb,
    )
    amp = frame.mean_amplitude
    assert math.isfinite(amp) and amp > 0.0


def test_bfld_frame_numpy_roundtrip_preserves_shape() -> None:
    fb = _make_matrix(2, 1, 996)
    frame = BfldFrame.from_compressed_feedback(
        timestamp_ms=0,
        sounding_index=0,
        sta_mac="aa:bb:cc:dd:ee:ff",
        kind=BfldKind.CompressedHE80,
        feedback_matrix=fb,
    )
    out = frame.feedback_matrix()
    assert out.shape == (2, 1, 996)
    # Roundtrip should be lossless (Complex64 in, Complex64 out).
    assert np.allclose(out, fb.astype(np.complex128))


def test_bfld_frame_repr_is_readable() -> None:
    fb = _make_matrix(2, 1, 996)
    frame = BfldFrame.from_compressed_feedback(
        timestamp_ms=0,
        sounding_index=0,
        sta_mac="aa:bb:cc:dd:ee:ff",
        kind=BfldKind.CompressedHE80,
        feedback_matrix=fb,
    )
    r = repr(frame)
    assert "BfldFrame" in r
    assert "996" in r
    assert "CompressedHE80" in r


# ─── BfldReport ──────────────────────────────────────────────────────


def test_bfld_report_starts_empty() -> None:
    report = BfldReport()
    assert report.n_frames == 0
    assert report.kind is None
    assert report.timestamp_first is None
    assert report.timestamp_last is None
    assert report.coherence_score == 0.0


def test_bfld_report_aggregates_homogeneous_frames() -> None:
    report = BfldReport()
    fb = _make_matrix(2, 1, 996)
    for i in range(5):
        frame = BfldFrame.from_compressed_feedback(
            timestamp_ms=1000 + i * 100,
            sounding_index=i,
            sta_mac="aa:bb:cc:dd:ee:ff",
            kind=BfldKind.CompressedHE80,
            feedback_matrix=fb,
        )
        report.add_frame(frame)
    assert report.n_frames == 5
    assert report.kind == BfldKind.CompressedHE80
    assert report.timestamp_first == 1000
    assert report.timestamp_last == 1400
    # Identical synthetic matrices → near-perfect coherence.
    assert report.coherence_score >= 0.99


def test_bfld_report_rejects_mismatched_kind() -> None:
    report = BfldReport()
    fb_he80 = _make_matrix(2, 1, 996)
    fb_he40 = _make_matrix(2, 1, 484)
    he80 = BfldFrame.from_compressed_feedback(
        timestamp_ms=0,
        sounding_index=0,
        sta_mac="aa:bb:cc:dd:ee:ff",
        kind=BfldKind.CompressedHE80,
        feedback_matrix=fb_he80,
    )
    he40 = BfldFrame.from_compressed_feedback(
        timestamp_ms=0,
        sounding_index=0,
        sta_mac="aa:bb:cc:dd:ee:ff",
        kind=BfldKind.CompressedHE40,
        feedback_matrix=fb_he40,
    )
    report.add_frame(he80)
    with pytest.raises(ValueError, match="kind"):
        report.add_frame(he40)


def test_bfld_report_repr_summarises() -> None:
    report = BfldReport()
    fb = _make_matrix(2, 1, 996)
    frame = BfldFrame.from_compressed_feedback(
        timestamp_ms=0,
        sounding_index=0,
        sta_mac="aa:bb:cc:dd:ee:ff",
        kind=BfldKind.CompressedHE80,
        feedback_matrix=fb,
    )
    report.add_frame(frame)
    r = repr(report)
    assert "BfldReport" in r
    assert "n_frames=1" in r


# ─── Build feature flag ──────────────────────────────────────────────


def test_p3_5_bfld_in_build_features() -> None:
    assert "p3.5-bfld-bindings" in wifi_densepose.__build_features__
