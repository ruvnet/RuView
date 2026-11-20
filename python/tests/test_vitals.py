"""ADR-117 P3 — Tests for vital-sign extraction bindings.

Covers:

- VitalStatus enum (eq, eq_int, hash, frozen)
- VitalEstimate construction + getters + immutability
- VitalReading composite + getters
- BreathingExtractor + HeartRateExtractor — esp32_default, explicit
  ctor, extract() return type, validation behaviour

The Rust pipeline is unit-tested in `v2/crates/wifi-densepose-vitals/`.
These tests are deliberately scoped to the *binding* layer — does the
Python surface return the right shapes, raise the right errors, and
release the GIL safely.
"""

from __future__ import annotations

import math
from random import Random

import numpy as np
import pytest

import wifi_densepose
from wifi_densepose import (
    BreathingExtractor,
    HeartRateExtractor,
    VitalEstimate,
    VitalReading,
    VitalStatus,
)


# ─── VitalStatus enum ────────────────────────────────────────────────


def test_vital_status_variants_present() -> None:
    assert VitalStatus.Valid != VitalStatus.Degraded
    assert VitalStatus.Unreliable != VitalStatus.Unavailable


def test_vital_status_equality_against_int() -> None:
    # eq_int → enum can be compared to int (PyO3 0.22 surface)
    assert VitalStatus.Valid == 0
    assert VitalStatus.Unavailable == 3


def test_vital_status_is_hashable() -> None:
    # frozen + hash → can be used as dict key / set member
    s = {VitalStatus.Valid, VitalStatus.Valid, VitalStatus.Degraded}
    assert len(s) == 2


def test_vital_status_repr_contains_variant_name() -> None:
    r = repr(VitalStatus.Valid)
    assert "VitalStatus" in r and "Valid" in r


# ─── VitalEstimate ───────────────────────────────────────────────────


def test_vital_estimate_construction_and_getters() -> None:
    est = VitalEstimate(value_bpm=72.4, confidence=0.85, status=VitalStatus.Valid)
    assert math.isclose(est.value_bpm, 72.4)
    assert math.isclose(est.confidence, 0.85)
    assert est.status == VitalStatus.Valid


def test_vital_estimate_is_frozen() -> None:
    est = VitalEstimate(value_bpm=72.0, confidence=0.9, status=VitalStatus.Valid)
    with pytest.raises(AttributeError):
        est.value_bpm = 100.0  # type: ignore[misc]


def test_vital_estimate_repr_is_readable() -> None:
    est = VitalEstimate(value_bpm=72.0, confidence=0.9, status=VitalStatus.Valid)
    r = repr(est)
    assert "VitalEstimate" in r
    assert "72" in r


# ─── VitalReading ────────────────────────────────────────────────────


def test_vital_reading_construction_and_getters() -> None:
    br = VitalEstimate(value_bpm=14.0, confidence=0.9, status=VitalStatus.Valid)
    hr = VitalEstimate(value_bpm=72.0, confidence=0.8, status=VitalStatus.Degraded)
    reading = VitalReading(
        respiratory_rate=br,
        heart_rate=hr,
        subcarrier_count=56,
        signal_quality=0.77,
        timestamp_secs=1700000000.5,
    )
    assert reading.respiratory_rate.value_bpm == 14.0
    assert reading.heart_rate.status == VitalStatus.Degraded
    assert reading.subcarrier_count == 56
    assert math.isclose(reading.signal_quality, 0.77)
    assert math.isclose(reading.timestamp_secs, 1700000000.5)


# ─── BreathingExtractor ──────────────────────────────────────────────


def test_breathing_esp32_default_constructs() -> None:
    br = BreathingExtractor.esp32_default()
    assert br is not None
    assert "BreathingExtractor" in repr(br)


def test_breathing_explicit_ctor() -> None:
    br = BreathingExtractor(n_subcarriers=64, sample_rate=200.0, window_secs=20.0)
    assert br is not None


def test_breathing_extract_returns_none_with_too_few_samples() -> None:
    """One frame can't produce a 30-second window — must return None.

    Verifies the binding propagates Rust's `Option<VitalEstimate>` →
    Python None correctly (vs raising or returning a default).
    """
    br = BreathingExtractor.esp32_default()
    out = br.extract(residuals=[0.0] * 56, weights=[])
    assert out is None


def test_breathing_extract_accepts_empty_weights() -> None:
    """Empty weights vector means "equal weight per subcarrier" by
    convention (per breathing.rs)."""
    br = BreathingExtractor.esp32_default()
    out = br.extract(residuals=[0.01] * 56, weights=[])
    # Even with synthetic input it may return None until enough history
    # accumulates — what matters is that the call doesn't panic.
    assert out is None or isinstance(out, VitalEstimate)


def test_breathing_extract_with_synthetic_signal() -> None:
    """Drive the extractor with a synthetic 0.25 Hz sine (15 BPM) for
    enough samples to fill the 30-second window. Don't assert the exact
    BPM — just that the extractor *eventually* produces a result (rather
    than returning None forever)."""
    br = BreathingExtractor.esp32_default()
    sample_rate = 100.0
    target_freq = 0.25  # 15 BPM
    # Run 40 seconds of synthetic data — comfortably past the 30s window.
    n_samples = int(40 * sample_rate)
    weights = [1.0] * 56

    produced_estimate = False
    rng = Random(42)
    for i in range(n_samples):
        t = i / sample_rate
        base = math.sin(2.0 * math.pi * target_freq * t)
        # Per-subcarrier residual: same signal + small per-carrier noise
        residuals = [base + rng.gauss(0.0, 0.01) for _ in range(56)]
        est = br.extract(residuals=residuals, weights=weights)
        if est is not None:
            produced_estimate = True
            assert isinstance(est.value_bpm, float)
            assert 0.0 <= est.confidence <= 1.0
            assert est.status in (
                VitalStatus.Valid,
                VitalStatus.Degraded,
                VitalStatus.Unreliable,
                VitalStatus.Unavailable,
            )
            break

    assert produced_estimate, "BreathingExtractor never produced an estimate after 40s of synthetic data"


# ─── HeartRateExtractor ──────────────────────────────────────────────


def test_heart_rate_esp32_default_constructs() -> None:
    hr = HeartRateExtractor.esp32_default()
    assert hr is not None
    assert "HeartRateExtractor" in repr(hr)


def test_heart_rate_explicit_ctor() -> None:
    hr = HeartRateExtractor(n_subcarriers=64, sample_rate=200.0, window_secs=10.0)
    assert hr is not None


def test_heart_rate_extract_returns_none_with_too_few_samples() -> None:
    hr = HeartRateExtractor.esp32_default()
    out = hr.extract(residuals=[0.0] * 56, phases=[0.0] * 56)
    assert out is None


def test_heart_rate_extract_rejects_old_weights_keyword() -> None:
    hr = HeartRateExtractor.esp32_default()
    with pytest.raises(TypeError):
        hr.extract(residuals=[0.0] * 56, weights=[0.0] * 56)


def test_heart_rate_extract_with_synthetic_signal_and_phases() -> None:
    """Drive the extractor with a synthetic 1.2 Hz sine (72 BPM) plus
    same-length phase data. This proves the binding feeds Rust's required
    `phases` slice instead of an empty vector that would keep returning None."""
    hr = HeartRateExtractor.esp32_default()
    sample_rate = 100.0
    target_freq = 1.2  # 72 BPM
    n_samples = int(60 * sample_rate)
    phases = [i * 0.01 for i in range(56)]

    produced_estimate = False
    rng = Random(43)
    for i in range(n_samples):
        t = i / sample_rate
        base = math.sin(2.0 * math.pi * target_freq * t)
        residuals = [base + rng.gauss(0.0, 0.01) for _ in range(56)]
        est = hr.extract(residuals=residuals, phases=phases)
        if est is not None:
            produced_estimate = True
            assert math.isfinite(est.value_bpm)
            assert 0.0 <= est.confidence <= 1.0
            break

    assert produced_estimate, (
        "HeartRateExtractor never produced an estimate after 60s of synthetic data"
    )


def test_heart_rate_extract_with_empty_phases_produces_estimates() -> None:
    """Issue #1423 regression: `phases=[]` must fall back to equal
    weighting (mirroring `BreathingExtractor`'s `weights=[]`), not silently
    return `None` for every frame.

    Reproduces the GH-issue repro: a noiseless 1.2 Hz (72 BPM) sine
    identical across all 56 subcarriers, fed frame-by-frame with an empty
    `phases` list. Before the fix this produced 0/4000 estimates; the same
    signal with `phases=[1.0] * 56` already produced thousands.
    """
    hr = HeartRateExtractor.esp32_default()
    sample_rate = 100.0
    target_freq = 1.2  # 72 BPM
    n_samples = 4000

    produced = 0
    for i in range(n_samples):
        t = i / sample_rate
        base = math.sin(2.0 * math.pi * target_freq * t)
        residuals = [base] * 56
        est = hr.extract(residuals=residuals, phases=[])
        if est is not None:
            produced += 1
            assert math.isfinite(est.value_bpm)
            assert 0.0 <= est.confidence <= 1.0

    assert produced > 0, (
        "HeartRateExtractor.extract(residuals=..., phases=[]) must not silently "
        "return None for every frame of a clean 72 BPM signal (issue #1423)"
    )


@pytest.mark.parametrize("heart_amplitude", [0.0, 0.01])
def test_issue_2057_breathing_leakage(heart_amplitude: float) -> None:
    """Replay the reported synthetic input through the installed Rust wheel."""
    hr = HeartRateExtractor(n_subcarriers=56, sample_rate=100.0, window_secs=15.0)
    rng = np.random.default_rng(0)
    last = None
    for i in range(6000):
        t = i / 100.0
        base = 0.1 * math.sin(2.0 * math.pi * 0.25 * t)
        base += heart_amplitude * math.sin(2.0 * math.pi * 1.2 * t)
        residuals = (base + 0.01 * rng.standard_normal(56)).tolist()
        last = hr.extract(residuals=residuals, phases=[])
    if heart_amplitude == 0.0:
        assert last is None or last.confidence < 0.3
    else:
        assert last is not None
        assert abs(last.value_bpm - 72.0) < 3.0


def test_issue_2057_filtering_does_not_promote_noise() -> None:
    hr = HeartRateExtractor(n_subcarriers=56, sample_rate=50.0, window_secs=15.0)
    rng = np.random.default_rng(79)
    last = None
    for frame in 0.01 * rng.standard_normal((250, 56)):
        last = hr.extract(residuals=frame.tolist(), phases=[])
    assert last is None or last.confidence < 0.6
    assert last is None or last.status != VitalStatus.Valid


# ─── Build feature flag ──────────────────────────────────────────────


def test_p3_vitals_in_build_features() -> None:
    assert "p3-vitals-bindings" in wifi_densepose.__build_features__
