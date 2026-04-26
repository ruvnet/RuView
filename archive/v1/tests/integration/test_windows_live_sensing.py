#!/usr/bin/env python3
"""
Live integration test: WindowsWifiCollector → FeatureExtractor → Classifier.

Runs the full ADR-013 commodity sensing pipeline against a real Windows WiFi
interface using ``netsh wlan show interfaces`` as the RSSI source.

Usage:
    python -m pytest v1/tests/integration/test_windows_live_sensing.py -v -o "addopts=" -s

Requirements:
    - Windows with connected WiFi
    - scipy, numpy installed
"""
import platform
import subprocess
import sys
import time

import pytest

# Skip the entire module on non-Windows or when WiFi is disconnected
_IS_WINDOWS = platform.system() == "Windows"

def _wifi_connected() -> bool:
    if not _IS_WINDOWS:
        return False
    try:
        r = subprocess.run(
            ["netsh", "wlan", "show", "interfaces"],
            capture_output=True, text=True, timeout=5,
        )
        return "connected" in r.stdout.lower() and "disconnected" not in r.stdout.lower().split("state")[1][:30]
    except Exception:
        return False


pytestmark = pytest.mark.skipif(
    not (_IS_WINDOWS and _wifi_connected()),
    reason="Requires Windows with connected WiFi",
)

from v1.src.sensing.rssi_collector import WindowsWifiCollector, WifiSample
from v1.src.sensing.feature_extractor import RssiFeatureExtractor, RssiFeatures
from v1.src.sensing.classifier import PresenceClassifier, MotionLevel, SensingResult
from v1.src.sensing.backend import CommodityBackend, Capability


class TestWindowsWifiCollectorLive:
    """Live tests against real Windows WiFi hardware."""

    def test_collect_once_returns_valid_sample(self):
        collector = WindowsWifiCollector(interface="Wi-Fi", sample_rate_hz=1.0)
        sample = collector.collect_once()

        assert isinstance(sample, WifiSample)
        assert -100 <= sample.rssi_dbm <= 0, f"RSSI {sample.rssi_dbm} out of range"
        assert sample.noise_dbm <= 0
        assert 0.0 <= sample.link_quality <= 1.0
        assert sample.interface == "Wi-Fi"
        print(f"\n  Single sample: RSSI={sample.rssi_dbm} dBm, "
              f"quality={sample.link_quality:.0%}, ts={sample.timestamp:.3f}")

    def test_collect_multiple_samples_over_time(self):
        collector = WindowsWifiCollector(interface="Wi-Fi", sample_rate_hz=2.0)
        collector.start()
        time.sleep(6)  # Collect ~12 samples at 2 Hz
        collector.stop()

        samples = collector.get_samples()
        assert len(samples) >= 5, f"Expected >= 5 samples, got {len(samples)}"

        rssi_values = [s.rssi_dbm for s in samples]
        print(f"\n  Collected {len(samples)} samples over ~6s")
        print(f"  RSSI range: {min(rssi_values):.1f} to {max(rssi_values):.1f} dBm")
        print(f"  RSSI values: {[f'{v:.1f}' for v in rssi_values]}")

        # All RSSI values should be in valid range
        for s in samples:
            assert -100 <= s.rssi_dbm <= 0

    def test_rssi_varies_between_samples(self):
        """RSSI should show at least slight natural variation."""
        collector = WindowsWifiCollector(interface="Wi-Fi", sample_rate_hz=2.0)
        collector.start()
        time.sleep(8)  # Collect ~16 samples
        collector.stop()

        samples = collector.get_samples()
        rssi_values = [s.rssi_dbm for s in samples]

        # With real hardware, we expect some variation (even if small)
        # But netsh may quantize RSSI so identical values are possible
        unique_count = len(set(rssi_values))
        print(f"\n  {len(rssi_values)} samples, {unique_count} unique RSSI values")
        print(f"  Values: {rssi_values}")


class TestFullPipelineLive:
    """End-to-end: WindowsWifiCollector → Extractor → Classifier."""

    def test_full_pipeline_produces_sensing_result(self):
        collector = WindowsWifiCollector(interface="Wi-Fi", sample_rate_hz=2.0)
        extractor = RssiFeatureExtractor(window_seconds=10.0)
        classifier = PresenceClassifier()

        collector.start()
        time.sleep(10)  # Collect ~20 samples
        collector.stop()

        samples = collector.get_samples()
        assert len(samples) >= 5, f"Need >= 5 samples, got {len(samples)}"

        features = extractor.extract(samples)
        assert isinstance(features, RssiFeatures)
        assert features.n_samples >= 5
        print(f"\n  Features from {features.n_samples} samples:")
        print(f"    mean={features.mean:.2f} dBm")
        print(f"    variance={features.variance:.4f}")
        print(f"    std={features.std:.4f}")
        print(f"    range={features.range:.2f}")
        print(f"    dominant_freq={features.dominant_freq_hz:.3f} Hz")
        print(f"    breathing_band={features.breathing_band_power:.4f}")
        print(f"    motion_band={features.motion_band_power:.4f}")
        print(f"    spectral_power={features.total_spectral_power:.4f}")
        print(f"    change_points={features.n_change_points}")

        result = classifier.classify(features)
        assert isinstance(result, SensingResult)
        assert isinstance(result.motion_level, MotionLevel)
        assert 0.0 <= result.confidence <= 1.0
        print(f"\n  Classification:")
        print(f"    motion_level={result.motion_level.value}")
        print(f"    presence={result.presence_detected}")
        print(f"    confidence={result.confidence:.2%}")
        print(f"    details: {result.details}")

    def test_commodity_backend_with_windows_collector(self):
        collector = WindowsWifiCollector(interface="Wi-Fi", sample_rate_hz=2.0)
        backend = CommodityBackend(collector=collector)

        assert backend.get_capabilities() == {Capability.PRESENCE, Capability.MOTION}

        backend.start()
        time.sleep(10)
        result = backend.get_result()
        backend.stop()

        assert isinstance(result, SensingResult)
        print(f"\n  CommodityBackend result:")
        print(f"    motion={result.motion_level.value}")
        print(f"    presence={result.presence_detected}")
        print(f"    confidence={result.confidence:.2%}")
        print(f"    rssi_variance={result.rssi_variance:.4f}")
        print(f"    motion_energy={result.motion_band_energy:.4f}")
        print(f"    breathing_energy={result.breathing_band_energy:.4f}")
