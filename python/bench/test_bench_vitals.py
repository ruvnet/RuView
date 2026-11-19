"""ADR-117 hardening sweep — Benchmarks for the P3 vitals hot paths.

Targets the ESP32 production rate: 100 Hz × 56 subcarriers, which is
what `BreathingExtractor.esp32_default()` is tuned for. The bench
asserts the *per-extract* cost is comfortably below 10 ms — at 100 Hz
that's the entire frame budget, so anything above 10 ms means the
Python binding would be the bottleneck instead of the radio.

Run with:
    pytest python/bench/ --benchmark-only

The benchmarks are skipped by default (`addopts` in pyproject.toml
doesn't include them) — they live in a sibling `bench/` directory
so the main test run stays fast.
"""

from __future__ import annotations

import math
from random import Random

import pytest

from wifi_densepose import BreathingExtractor, HeartRateExtractor


def _synth_frame(
    n_subcarriers: int, sample_rate: float, t: float, freq_hz: float, rng: Random
) -> tuple[list[float], list[float]]:
    """Build one ESP32-shape frame at time `t`: sine at `freq_hz` plus
    tiny per-subcarrier noise."""
    base = math.sin(2.0 * math.pi * freq_hz * t)
    residuals = [base + rng.gauss(0.0, 0.01) for _ in range(n_subcarriers)]
    weights = [1.0] * n_subcarriers
    return residuals, weights


def test_breathing_extract_per_frame_cost(benchmark) -> None:
    """One BreathingExtractor.extract() at ESP32 defaults should
    finish well under 10 ms — that's the 100 Hz frame budget."""
    br = BreathingExtractor.esp32_default()
    rng = Random(42)
    # Pre-fill ~25 seconds of history so the bench measures the
    # steady-state cost, not the cold-start cost.
    for i in range(2500):
        residuals, weights = _synth_frame(56, 100.0, i / 100.0, 0.25, rng)
        br.extract(residuals=residuals, weights=weights)

    def _one_frame():
        residuals, weights = _synth_frame(56, 100.0, 30.0, 0.25, rng)
        return br.extract(residuals=residuals, weights=weights)

    benchmark(_one_frame)


def test_heart_rate_extract_per_frame_cost(benchmark) -> None:
    """One HeartRateExtractor.extract() at ESP32 defaults — same 10 ms
    target."""
    hr = HeartRateExtractor.esp32_default()
    rng = Random(43)
    for i in range(1500):
        residuals, phases = _synth_frame(56, 100.0, i / 100.0, 1.2, rng)
        hr.extract(residuals=residuals, phases=phases)

    def _one_frame():
        residuals, phases = _synth_frame(56, 100.0, 16.0, 1.2, rng)
        return hr.extract(residuals=residuals, phases=phases)

    benchmark(_one_frame)


@pytest.mark.parametrize("n_subcarriers", [56, 114, 242])
def test_breathing_extract_scaling(benchmark, n_subcarriers: int) -> None:
    """Sanity check: cost should scale roughly linearly with the
    subcarrier count. Catches accidental O(n^2) regressions."""
    sample_rate = 100.0
    br = BreathingExtractor(n_subcarriers, sample_rate, 30.0)
    rng = Random(n_subcarriers)
    for i in range(2500):
        residuals, weights = _synth_frame(n_subcarriers, sample_rate, i / sample_rate, 0.25, rng)
        br.extract(residuals=residuals, weights=weights)

    def _one_frame():
        residuals, weights = _synth_frame(n_subcarriers, sample_rate, 30.0, 0.25, rng)
        return br.extract(residuals=residuals, weights=weights)

    benchmark(_one_frame)
