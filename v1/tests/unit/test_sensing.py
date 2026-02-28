"""
Unit tests for the commodity sensing module (ADR-013).

Tests cover:
    - Feature extraction from known sinusoidal RSSI input
    - Classifier producing correct presence/motion from known features
    - SimulatedCollector determinism (same seed = same output)
    - CUSUM change-point detection catching step changes
    - Band power extraction isolating correct frequencies
    - Backend capabilities and pipeline integration
"""

from __future__ import annotations

import math

import numpy as np
import pytest
from numpy.typing import NDArray

from v1.src.sensing.rssi_collector import (
    RingBuffer,
    SimulatedCollector,
    WifiSample,
)
from v1.src.sensing.feature_extractor import (
    RssiFeatureExtractor,
    RssiFeatures,
    cusum_detect,
    _band_power,
)
from v1.src.sensing.classifier import (
    MotionLevel,
    PresenceClassifier,
    SensingResult,
)
from v1.src.sensing.backend import (
    Capability,
    CommodityBackend,
    SensingBackend,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def make_sinusoidal_rssi(
    freq_hz: float,
    amplitude: float,
    baseline: float,
    duration_s: float,
    sample_rate: float,
) -> NDArray[np.float64]:
    """Generate a clean sinusoidal RSSI signal (no noise)."""
    n = int(duration_s * sample_rate)
    t = np.arange(n) / sample_rate
    return baseline + amplitude * np.sin(2 * np.pi * freq_hz * t)


def make_step_signal(
    baseline: float,
    step_value: float,
    step_at_sample: int,
    n_samples: int,
) -> NDArray[np.float64]:
    """Generate a signal with a step change at a specific sample."""
    signal = np.full(n_samples, baseline, dtype=np.float64)
    signal[step_at_sample:] = step_value
    return signal


# ===========================================================================
# RingBuffer tests
# ===========================================================================

class TestRingBuffer:
    def test_append_and_get_all(self):
        buf = RingBuffer(max_size=5)
        for i in range(3):
            buf.append(WifiSample(
                timestamp=float(i), rssi_dbm=-50.0 + i, noise_dbm=-95.0,
                link_quality=0.8, tx_bytes=0, rx_bytes=0, retry_count=0,
                interface="test0",
            ))
        assert len(buf) == 3
        samples = buf.get_all()
        assert len(samples) == 3
        assert samples[0].rssi_dbm == -50.0
        assert samples[2].rssi_dbm == -48.0

    def test_ring_buffer_overflow(self):
        buf = RingBuffer(max_size=3)
        for i in range(5):
            buf.append(WifiSample(
                timestamp=float(i), rssi_dbm=float(i), noise_dbm=-95.0,
                link_quality=0.8, tx_bytes=0, rx_bytes=0, retry_count=0,
                interface="test0",
            ))
        assert len(buf) == 3
        samples = buf.get_all()
        # Oldest two should have been evicted; remaining: 2, 3, 4
        assert samples[0].rssi_dbm == 2.0
        assert samples[2].rssi_dbm == 4.0

    def test_get_last_n(self):
        buf = RingBuffer(max_size=10)
        for i in range(7):
            buf.append(WifiSample(
                timestamp=float(i), rssi_dbm=float(i), noise_dbm=-95.0,
                link_quality=0.8, tx_bytes=0, rx_bytes=0, retry_count=0,
                interface="test0",
            ))
        last_3 = buf.get_last_n(3)
        assert len(last_3) == 3
        assert last_3[0].rssi_dbm == 4.0
        assert last_3[2].rssi_dbm == 6.0

    def test_clear(self):
        buf = RingBuffer(max_size=10)
        buf.append(WifiSample(
            timestamp=0.0, rssi_dbm=-50.0, noise_dbm=-95.0,
            link_quality=0.8, tx_bytes=0, rx_bytes=0, retry_count=0,
            interface="test0",
        ))
        buf.clear()
        assert len(buf) == 0


# ===========================================================================
# SimulatedCollector tests
# ===========================================================================

class TestSimulatedCollector:
    def test_deterministic_output_same_seed(self):
        """Same seed must produce identical samples."""
        c1 = SimulatedCollector(seed=123, sample_rate_hz=10.0)
        c2 = SimulatedCollector(seed=123, sample_rate_hz=10.0)

        s1 = c1.generate_samples(5.0)
        s2 = c2.generate_samples(5.0)

        assert len(s1) == len(s2) == 50
        for a, b in zip(s1, s2):
            assert a.rssi_dbm == b.rssi_dbm, (
                f"RSSI mismatch at same seed: {a.rssi_dbm} != {b.rssi_dbm}"
            )
            assert a.noise_dbm == b.noise_dbm
            assert a.link_quality == b.link_quality

    def test_different_seeds_differ(self):
        """Different seeds must produce different samples."""
        c1 = SimulatedCollector(seed=1, sample_rate_hz=10.0)
        c2 = SimulatedCollector(seed=999, sample_rate_hz=10.0)

        s1 = c1.generate_samples(2.0)
        s2 = c2.generate_samples(2.0)

        rssi1 = [s.rssi_dbm for s in s1]
        rssi2 = [s.rssi_dbm for s in s2]
        # Not all values should match
        assert rssi1 != rssi2

    def test_sinusoidal_component(self):
        """With zero noise, should see a clean sinusoid."""
        c = SimulatedCollector(
            seed=0,
            sample_rate_hz=100.0,
            baseline_dbm=-50.0,
            sine_freq_hz=1.0,
            sine_amplitude_dbm=5.0,
            noise_std_dbm=0.0,  # no noise
        )
        samples = c.generate_samples(2.0)
        rssi = np.array([s.rssi_dbm for s in samples])

        # Mean should be very close to baseline
        assert abs(np.mean(rssi) - (-50.0)) < 0.5

        # Amplitude should be close to 5 dBm (peak-to-peak ~10)
        assert np.ptp(rssi) > 9.0
        assert np.ptp(rssi) < 11.0

    def test_step_change_injection(self):
        """Step change should shift the signal at the specified time."""
        c = SimulatedCollector(
            seed=42,
            sample_rate_hz=10.0,
            baseline_dbm=-50.0,
            sine_amplitude_dbm=0.0,
            noise_std_dbm=0.0,
            step_change_at=2.0,
            step_change_dbm=-10.0,
        )
        samples = c.generate_samples(4.0)
        rssi = np.array([s.rssi_dbm for s in samples])

        # Before step (first 20 samples at 10 Hz = 2 seconds)
        mean_before = np.mean(rssi[:20])
        # After step (samples 20-39)
        mean_after = np.mean(rssi[20:])

        assert abs(mean_before - (-50.0)) < 0.1
        assert abs(mean_after - (-60.0)) < 0.1

    def test_sample_count(self):
        """generate_samples should produce exactly rate * duration samples."""
        c = SimulatedCollector(seed=0, sample_rate_hz=20.0)
        samples = c.generate_samples(3.0)
        assert len(samples) == 60


# ===========================================================================
# Feature extraction tests
# ===========================================================================

class TestFeatureExtractor:
    def test_time_domain_from_known_sine(self):
        """
        A pure sinusoid at -50 dBm baseline with 2 dBm amplitude should
        produce known statistical properties.
        """
        sample_rate = 100.0
        rssi = make_sinusoidal_rssi(
            freq_hz=1.0, amplitude=2.0, baseline=-50.0,
            duration_s=10.0, sample_rate=sample_rate,
        )

        ext = RssiFeatureExtractor(window_seconds=30.0)
        features = ext.extract_from_array(rssi, sample_rate)

        # Mean should be close to -50
        assert abs(features.mean - (-50.0)) < 0.1

        # Variance of A*sin(x) is A^2/2
        expected_var = 2.0**2 / 2.0  # = 2.0
        assert abs(features.variance - expected_var) < 0.2

        # Skewness of a pure sinusoid is ~0
        assert abs(features.skewness) < 0.2

        # Range should be close to 2*amplitude = 4.0
        assert abs(features.range - 4.0) < 0.2

    def test_frequency_domain_dominant_frequency(self):
        """
        A 0.3 Hz sinusoid should produce a dominant frequency near 0.3 Hz.
        """
        sample_rate = 10.0
        rssi = make_sinusoidal_rssi(
            freq_hz=0.3, amplitude=3.0, baseline=-50.0,
            duration_s=30.0, sample_rate=sample_rate,
        )

        ext = RssiFeatureExtractor(window_seconds=60.0)
        features = ext.extract_from_array(rssi, sample_rate)

        # Dominant frequency should be close to 0.3 Hz
        assert abs(features.dominant_freq_hz - 0.3) < 0.1, (
            f"Dominant freq {features.dominant_freq_hz} != ~0.3 Hz"
        )

    def test_breathing_band_power(self):
        """
        A 0.3 Hz signal should produce significant power in the breathing
        band (0.1-0.5 Hz) and negligible power in the motion band (0.5-3 Hz).
        """
        sample_rate = 10.0
        rssi = make_sinusoidal_rssi(
            freq_hz=0.3, amplitude=3.0, baseline=-50.0,
            duration_s=30.0, sample_rate=sample_rate,
        )

        ext = RssiFeatureExtractor(window_seconds=60.0)
        features = ext.extract_from_array(rssi, sample_rate)

        assert features.breathing_band_power > 0.1, (
            f"Breathing band power too low: {features.breathing_band_power}"
        )
        # Motion band should have much less power than breathing band
        assert features.motion_band_power < features.breathing_band_power, (
            f"Motion band ({features.motion_band_power}) should be less than "
            f"breathing band ({features.breathing_band_power})"
        )

    def test_motion_band_power(self):
        """
        A 1.5 Hz signal should produce significant power in the motion
        band (0.5-3.0 Hz) and negligible power in the breathing band.
        """
        sample_rate = 10.0
        rssi = make_sinusoidal_rssi(
            freq_hz=1.5, amplitude=3.0, baseline=-50.0,
            duration_s=30.0, sample_rate=sample_rate,
        )

        ext = RssiFeatureExtractor(window_seconds=60.0)
        features = ext.extract_from_array(rssi, sample_rate)

        assert features.motion_band_power > 0.1, (
            f"Motion band power too low: {features.motion_band_power}"
        )
        assert features.motion_band_power > features.breathing_band_power, (
            f"Motion band ({features.motion_band_power}) should dominate over "
            f"breathing band ({features.breathing_band_power})"
        )

    def test_band_isolation_multi_frequency(self):
        """
        A signal with components at 0.2 Hz AND 2.0 Hz should produce power
        in both bands, each dominated by the correct component.
        """
        sample_rate = 10.0
        n = int(30.0 * sample_rate)
        t = np.arange(n) / sample_rate
        # 0.2 Hz component (breathing) + 2.0 Hz component (motion)
        rssi = -50.0 + 3.0 * np.sin(2 * np.pi * 0.2 * t) + 2.0 * np.sin(2 * np.pi * 2.0 * t)

        ext = RssiFeatureExtractor(window_seconds=60.0)
        features = ext.extract_from_array(rssi, sample_rate)

        # Both bands should have significant power
        assert features.breathing_band_power > 0.05
        assert features.motion_band_power > 0.05

    def test_constant_signal_features(self):
        """A constant signal should have zero variance and no spectral content."""
        rssi = np.full(200, -50.0)
        ext = RssiFeatureExtractor()
        features = ext.extract_from_array(rssi, 10.0)

        assert features.variance == 0.0
        assert features.std == 0.0
        assert features.range == 0.0
        assert features.iqr == 0.0
        assert features.total_spectral_power < 1e-10

    def test_too_few_samples(self):
        """Fewer than 4 samples should return empty features."""
        rssi = np.array([-50.0, -51.0])
        ext = RssiFeatureExtractor()
        features = ext.extract_from_array(rssi, 10.0)
        assert features.n_samples == 2
        assert features.variance == 0.0

    def test_extract_from_wifi_samples(self):
        """Test extraction from WifiSample objects (the normal path)."""
        collector = SimulatedCollector(
            seed=42, sample_rate_hz=10.0,
            baseline_dbm=-50.0, sine_freq_hz=0.3,
            sine_amplitude_dbm=2.0, noise_std_dbm=0.1,
        )
        samples = collector.generate_samples(10.0)

        ext = RssiFeatureExtractor(window_seconds=30.0)
        features = ext.extract(samples)

        assert features.n_samples == 100
        assert abs(features.mean - (-50.0)) < 1.0
        assert features.variance > 0.0


# ===========================================================================
# CUSUM change-point detection tests
# ===========================================================================

class TestCusum:
    def test_step_change_detected(self):
        """CUSUM should detect a step change in the signal."""
        signal = make_step_signal(
            baseline=0.0, step_value=5.0,
            step_at_sample=100, n_samples=200,
        )
        target = float(np.mean(signal))
        std = float(np.std(signal, ddof=1))
        threshold = 3.0 * std
        drift = 0.5 * std

        change_points = cusum_detect(signal, target, threshold, drift)

        assert len(change_points) > 0, "No change points detected for step change"
        # At least one change point should be near the step (sample 100)
        nearest = min(change_points, key=lambda x: abs(x - 100))
        assert abs(nearest - 100) < 20, (
            f"Nearest change point at {nearest}, expected near 100"
        )

    def test_no_change_point_in_constant(self):
        """A constant signal should produce no change points."""
        signal = np.full(200, 0.0)
        change_points = cusum_detect(signal, 0.0, 1.0, 0.1)
        assert len(change_points) == 0

    def test_multiple_step_changes(self):
        """CUSUM should detect multiple step changes."""
        n = 300
        signal = np.zeros(n, dtype=np.float64)
        signal[100:200] = 5.0
        signal[200:] = 0.0

        target = float(np.mean(signal))
        std = float(np.std(signal, ddof=1))
        threshold = 2.0 * std
        drift = 0.3 * std

        change_points = cusum_detect(signal, target, threshold, drift)
        # Should detect at least the step up and the step down
        assert len(change_points) >= 2, (
            f"Expected >= 2 change points, got {len(change_points)}"
        )

    def test_cusum_with_feature_extractor(self):
        """Feature extractor should detect step change via CUSUM."""
        signal = make_step_signal(
            baseline=-50.0, step_value=-60.0,
            step_at_sample=150, n_samples=300,
        )

        ext = RssiFeatureExtractor(cusum_threshold=2.0, cusum_drift=0.3)
        features = ext.extract_from_array(signal, 10.0)

        assert features.n_change_points > 0, (
            f"Expected change points but got {features.n_change_points}"
        )


# ===========================================================================
# Classifier tests
# ===========================================================================

class TestPresenceClassifier:
    def test_absent_when_low_variance(self):
        """Low variance should classify as ABSENT."""
        features = RssiFeatures(
            variance=0.1,
            motion_band_power=0.0,
            breathing_band_power=0.0,
            n_samples=100,
        )
        clf = PresenceClassifier(presence_variance_threshold=0.5)
        result = clf.classify(features)

        assert result.motion_level == MotionLevel.ABSENT
        assert result.presence_detected is False

    def test_present_still_when_high_variance_low_motion(self):
        """High variance but low motion energy should classify as PRESENT_STILL."""
        features = RssiFeatures(
            variance=2.0,
            motion_band_power=0.05,
            breathing_band_power=0.3,
            n_samples=100,
        )
        clf = PresenceClassifier(
            presence_variance_threshold=0.5,
            motion_energy_threshold=0.1,
        )
        result = clf.classify(features)

        assert result.motion_level == MotionLevel.PRESENT_STILL
        assert result.presence_detected is True

    def test_active_when_high_variance_high_motion(self):
        """High variance and high motion energy should classify as ACTIVE."""
        features = RssiFeatures(
            variance=3.0,
            motion_band_power=0.5,
            breathing_band_power=0.1,
            n_samples=100,
        )
        clf = PresenceClassifier(
            presence_variance_threshold=0.5,
            motion_energy_threshold=0.1,
        )
        result = clf.classify(features)

        assert result.motion_level == MotionLevel.ACTIVE
        assert result.presence_detected is True

    def test_confidence_for_absent_decreases_with_rising_variance(self):
        """
        When classified as ABSENT, confidence should decrease as variance
        approaches the presence threshold (less certain about absence).
        """
        clf = PresenceClassifier(presence_variance_threshold=10.0)

        clearly_absent = clf.classify(RssiFeatures(
            variance=0.5, motion_band_power=0.0, n_samples=100
        ))
        borderline_absent = clf.classify(RssiFeatures(
            variance=9.0, motion_band_power=0.0, n_samples=100
        ))

        assert clearly_absent.motion_level == MotionLevel.ABSENT
        assert borderline_absent.motion_level == MotionLevel.ABSENT
        assert clearly_absent.confidence > borderline_absent.confidence, (
            f"Clearly absent ({clearly_absent.confidence}) should have higher "
            f"confidence than borderline absent ({borderline_absent.confidence})"
        )

    def test_confidence_bounded_0_to_1(self):
        """Confidence should always be in [0, 1]."""
        clf = PresenceClassifier()

        for var in [0.0, 0.1, 1.0, 10.0, 100.0]:
            result = clf.classify(
                RssiFeatures(variance=var, motion_band_power=var, n_samples=100)
            )
            assert 0.0 <= result.confidence <= 1.0, (
                f"Confidence {result.confidence} out of bounds for var={var}"
            )

    def test_cross_receiver_agreement_boosts_confidence(self):
        """Matching results from other receivers should boost confidence."""
        clf = PresenceClassifier(presence_variance_threshold=0.5)
        features = RssiFeatures(variance=2.0, motion_band_power=0.0, n_samples=100)

        result_solo = clf.classify(features)

        # Other receivers also report PRESENT_STILL
        other = [
            SensingResult(
                motion_level=MotionLevel.PRESENT_STILL,
                confidence=0.8,
                presence_detected=True,
                rssi_variance=1.5,
                motion_band_energy=0.0,
                breathing_band_energy=0.0,
                n_change_points=0,
            )
        ]
        result_agreed = clf.classify(features, other_receiver_results=other)

        assert result_agreed.confidence >= result_solo.confidence

    def test_result_dataclass_fields(self):
        """SensingResult should contain all expected fields."""
        clf = PresenceClassifier()
        features = RssiFeatures(
            variance=1.0,
            motion_band_power=0.2,
            breathing_band_power=0.3,
            n_change_points=2,
            n_samples=100,
        )
        result = clf.classify(features)

        assert hasattr(result, "motion_level")
        assert hasattr(result, "confidence")
        assert hasattr(result, "presence_detected")
        assert hasattr(result, "rssi_variance")
        assert hasattr(result, "motion_band_energy")
        assert hasattr(result, "breathing_band_energy")
        assert hasattr(result, "n_change_points")
        assert hasattr(result, "details")
        assert isinstance(result.details, str)
        assert len(result.details) > 0


# ===========================================================================
# Backend tests
# ===========================================================================

class TestCommodityBackend:
    def test_capabilities(self):
        """CommodityBackend should only report PRESENCE and MOTION."""
        collector = SimulatedCollector(seed=0)
        backend = CommodityBackend(collector=collector)

        caps = backend.get_capabilities()
        assert Capability.PRESENCE in caps
        assert Capability.MOTION in caps
        assert Capability.RESPIRATION not in caps
        assert Capability.LOCATION not in caps
        assert Capability.POSE not in caps

    def test_is_capable(self):
        collector = SimulatedCollector(seed=0)
        backend = CommodityBackend(collector=collector)

        assert backend.is_capable(Capability.PRESENCE) is True
        assert backend.is_capable(Capability.MOTION) is True
        assert backend.is_capable(Capability.RESPIRATION) is False
        assert backend.is_capable(Capability.POSE) is False

    def test_protocol_conformance(self):
        """CommodityBackend should satisfy the SensingBackend protocol."""
        collector = SimulatedCollector(seed=0)
        backend = CommodityBackend(collector=collector)
        assert isinstance(backend, SensingBackend)

    def test_full_pipeline(self):
        """
        End-to-end: SimulatedCollector -> features -> classification.

        With a 0.3 Hz sine and some noise, the pipeline should detect
        presence (variance > threshold).
        """
        collector = SimulatedCollector(
            seed=42,
            sample_rate_hz=10.0,
            baseline_dbm=-50.0,
            sine_freq_hz=0.3,
            sine_amplitude_dbm=3.0,
            noise_std_dbm=0.3,
        )
        backend = CommodityBackend(
            collector=collector,
            extractor=RssiFeatureExtractor(window_seconds=10.0),
            classifier=PresenceClassifier(
                presence_variance_threshold=0.5,
                motion_energy_threshold=0.1,
            ),
        )

        # Pre-fill the collector buffer with generated samples
        samples = collector.generate_samples(10.0)
        for s in samples:
            collector._buffer.append(s)

        result = backend.get_result()
        features = backend.get_features()

        # With amplitude 3 dBm, variance should be about 4.5
        assert features.variance > 0.5, (
            f"Expected variance > 0.5, got {features.variance}"
        )
        assert result.presence_detected is True
        assert result.motion_level in (MotionLevel.PRESENT_STILL, MotionLevel.ACTIVE)

    def test_absent_with_constant_signal(self):
        """
        A collector producing a near-constant signal should result in ABSENT.
        """
        collector = SimulatedCollector(
            seed=0,
            sample_rate_hz=10.0,
            baseline_dbm=-50.0,
            sine_amplitude_dbm=0.0,
            noise_std_dbm=0.05,  # very low noise
        )
        backend = CommodityBackend(
            collector=collector,
            extractor=RssiFeatureExtractor(window_seconds=10.0),
            classifier=PresenceClassifier(presence_variance_threshold=0.5),
        )

        samples = collector.generate_samples(10.0)
        for s in samples:
            collector._buffer.append(s)

        result = backend.get_result()
        assert result.motion_level == MotionLevel.ABSENT
        assert result.presence_detected is False

    def test_repr(self):
        collector = SimulatedCollector(seed=0)
        backend = CommodityBackend(collector=collector)
        r = repr(backend)
        assert "CommodityBackend" in r
        assert "PRESENCE" in r
        assert "MOTION" in r


# ===========================================================================
# Band power helper tests
# ===========================================================================

class TestBandPower:
    def test_band_power_single_frequency(self):
        """Power of a single frequency should concentrate in the correct band."""
        sample_rate = 10.0
        n = 300
        t = np.arange(n) / sample_rate
        signal = 5.0 * np.sin(2 * np.pi * 0.3 * t)

        # Apply window and compute FFT
        window = np.hanning(n)
        windowed = signal * window
        from scipy import fft as scipy_fft
        fft_vals = scipy_fft.rfft(windowed)
        freqs = scipy_fft.rfftfreq(n, d=1.0 / sample_rate)
        psd = (np.abs(fft_vals) ** 2) / n

        # Skip DC
        freqs_no_dc = freqs[1:]
        psd_no_dc = psd[1:]

        breathing = _band_power(freqs_no_dc, psd_no_dc, 0.1, 0.5)
        motion = _band_power(freqs_no_dc, psd_no_dc, 0.5, 3.0)

        assert breathing > motion, (
            f"0.3 Hz signal should have more breathing band power ({breathing}) "
            f"than motion band power ({motion})"
        )

    def test_band_power_zero_for_empty_band(self):
        """Band with no frequency content should return ~0 power."""
        freqs = np.array([0.1, 0.2, 0.3, 0.4, 0.5])
        psd = np.array([1.0, 0.0, 0.0, 0.0, 1.0])

        # Band 0.21-0.39 has no power
        p = _band_power(freqs, psd, 0.21, 0.39)
        assert p == 0.0
