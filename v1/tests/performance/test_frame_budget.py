"""Frame budget benchmark for CSI processing pipeline.

Verifies that per-frame CSI processing stays within the 50 ms budget
required for real-time sensing at 20 FPS.
"""

import time
import statistics
import pytest
import numpy as np

from src.core.csi_processor import CSIProcessor


def _make_config():
    return {
        "sampling_rate": 1000,
        "window_size": 256,
        "overlap": 0.5,
        "noise_threshold": -60,
        "human_detection_threshold": 0.8,
        "smoothing_factor": 0.9,
        "max_history_size": 500,
        "num_subcarriers": 256,
        "num_antennas": 3,
        "doppler_window": 64,
    }


def _make_csi_data(n_subcarriers=256, n_antennas=3, seed=None):
    """Generate a synthetic CSI frame with complex-valued subcarriers."""
    rng = np.random.default_rng(seed)
    from unittest.mock import MagicMock
    csi = MagicMock()
    csi.amplitude = rng.random((n_antennas, n_subcarriers)).astype(np.float64) * 20.0
    csi.phase = (rng.random((n_antennas, n_subcarriers)).astype(np.float64) - 0.5) * np.pi * 2
    csi.frequency = 5.0e9
    csi.bandwidth = 80e6
    csi.num_subcarriers = n_subcarriers
    csi.num_antennas = n_antennas
    csi.snr = 25.0
    csi.timestamp = time.time()
    csi.metadata = {}
    return csi


class TestSingleFrameBudget:
    """Single-frame processing must complete in < 50 ms."""

    def test_single_frame_under_50ms(self):
        proc = CSIProcessor(config=_make_config())
        frame = _make_csi_data(seed=42)

        # Warm up
        proc.preprocess_csi_data(frame)

        start = time.perf_counter()
        proc.preprocess_csi_data(frame)
        features = proc.extract_features(frame)
        if features:
            proc.detect_human_presence(features)
        elapsed_ms = (time.perf_counter() - start) * 1000

        assert elapsed_ms < 50, f"Single frame took {elapsed_ms:.1f} ms (budget: 50 ms)"


class TestSustainedFrameBudget:
    """Sustained 100-frame processing p95 must be < 50 ms per frame."""

    def test_sustained_100_frames_p95(self):
        proc = CSIProcessor(config=_make_config())
        rng = np.random.default_rng(123)
        n_frames = 100
        latencies = []

        for i in range(n_frames):
            frame = _make_csi_data(seed=i)
            start = time.perf_counter()
            preprocessed = proc.preprocess_csi_data(frame)
            features = proc.extract_features(preprocessed)
            if features:
                proc.detect_human_presence(features)
            proc.add_to_history(frame)
            elapsed_ms = (time.perf_counter() - start) * 1000
            latencies.append(elapsed_ms)

        p50 = statistics.median(latencies)
        p95 = sorted(latencies)[int(0.95 * len(latencies))]
        p99 = sorted(latencies)[int(0.99 * len(latencies))]

        print(f"\n--- Sustained {n_frames}-frame benchmark ---")
        print(f"  p50: {p50:.2f} ms")
        print(f"  p95: {p95:.2f} ms")
        print(f"  p99: {p99:.2f} ms")
        print(f"  min: {min(latencies):.2f} ms")
        print(f"  max: {max(latencies):.2f} ms")

        assert p95 < 50, f"p95 latency {p95:.1f} ms exceeds 50 ms budget"


class TestPipelineWithDoppler:
    """Full pipeline including Doppler estimation must stay within budget."""

    def test_doppler_pipeline(self):
        proc = CSIProcessor(config=_make_config())
        n_frames = 100
        latencies = []

        # Fill history first
        for i in range(20):
            frame = _make_csi_data(seed=i + 1000)
            proc.add_to_history(frame)

        for i in range(n_frames):
            frame = _make_csi_data(seed=i + 2000)
            start = time.perf_counter()
            preprocessed = proc.preprocess_csi_data(frame)
            features = proc.extract_features(preprocessed)
            if features:
                proc.detect_human_presence(features)
            proc.add_to_history(frame)
            elapsed_ms = (time.perf_counter() - start) * 1000
            latencies.append(elapsed_ms)

        p50 = statistics.median(latencies)
        p95 = sorted(latencies)[int(0.95 * len(latencies))]
        p99 = sorted(latencies)[int(0.99 * len(latencies))]

        print(f"\n--- Doppler pipeline benchmark ({n_frames} frames, 20 warmup) ---")
        print(f"  p50: {p50:.2f} ms")
        print(f"  p95: {p95:.2f} ms")
        print(f"  p99: {p99:.2f} ms")

        # Doppler adds overhead but should still be within budget
        assert p95 < 50, f"Doppler pipeline p95 {p95:.1f} ms exceeds 50 ms budget"
