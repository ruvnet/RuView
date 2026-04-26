import pytest
import numpy as np
import time
from datetime import datetime, timezone
from unittest.mock import Mock, patch
from src.core.csi_processor import CSIProcessor, CSIFeatures
from src.hardware.csi_extractor import CSIData


def make_csi_data(amplitude=None, phase=None, n_ant=3, n_sub=56):
    """Build a CSIData test fixture."""
    if amplitude is None:
        amplitude = np.random.uniform(0.1, 2.0, (n_ant, n_sub))
    if phase is None:
        phase = np.random.uniform(-np.pi, np.pi, (n_ant, n_sub))
    return CSIData(
        timestamp=datetime.now(timezone.utc),
        amplitude=amplitude,
        phase=phase,
        frequency=5.21e9,
        bandwidth=17.5e6,
        num_subcarriers=n_sub,
        num_antennas=n_ant,
        snr=15.0,
        metadata={"source": "test"},
    )


_PROCESSOR_CONFIG = {
    "sampling_rate": 100,
    "window_size": 56,
    "overlap": 0.5,
    "noise_threshold": -60,
    "human_detection_threshold": 0.8,
    "smoothing_factor": 0.9,
    "max_history_size": 500,
    "enable_preprocessing": True,
    "enable_feature_extraction": True,
    "enable_human_detection": True,
}


class TestCSIProcessor:
    """Test suite for CSI processor following London School TDD principles"""

    @pytest.fixture
    def csi_processor(self):
        """Create CSI processor instance for testing"""
        return CSIProcessor(config=_PROCESSOR_CONFIG)

    @pytest.fixture
    def sample_csi(self):
        """Generate synthetic CSIData for testing"""
        return make_csi_data()

    def test_preprocess_returns_csi_data(self, csi_processor, sample_csi):
        """Preprocess should return a CSIData instance"""
        result = csi_processor.preprocess_csi_data(sample_csi)
        assert isinstance(result, CSIData)
        assert result.num_antennas == sample_csi.num_antennas
        assert result.num_subcarriers == sample_csi.num_subcarriers

    def test_preprocess_normalises_amplitude(self, csi_processor, sample_csi):
        """Preprocess should produce finite, non-negative amplitude with unit-variance normalisation"""
        result = csi_processor.preprocess_csi_data(sample_csi)
        assert np.all(np.isfinite(result.amplitude))
        assert result.amplitude.min() >= 0.0
        # Normalised to unit variance: std ≈ 1.0 (may differ due to Hamming window)
        std = np.std(result.amplitude)
        assert 0.5 < std < 5.0  # within reasonable bounds of unit-variance normalisation

    def test_preprocess_removes_nan(self, csi_processor):
        """Preprocess should replace NaN amplitude with 0"""
        amp = np.ones((3, 56))
        amp[0, 0] = np.nan
        csi = make_csi_data(amplitude=amp)
        result = csi_processor.preprocess_csi_data(csi)
        assert not np.isnan(result.amplitude).any()

    def test_extract_features_returns_csi_features(self, csi_processor, sample_csi):
        """extract_features should return a CSIFeatures instance"""
        preprocessed = csi_processor.preprocess_csi_data(sample_csi)
        features = csi_processor.extract_features(preprocessed)
        assert isinstance(features, CSIFeatures)

    def test_extract_features_has_correct_shapes(self, csi_processor, sample_csi):
        """Feature arrays should have expected shapes"""
        preprocessed = csi_processor.preprocess_csi_data(sample_csi)
        features = csi_processor.extract_features(preprocessed)
        assert features.amplitude_mean.shape == (56,)
        assert features.amplitude_variance.shape == (56,)

    def test_preprocess_performance(self, csi_processor, sample_csi):
        """Preprocessing a single frame must complete in < 10 ms"""
        start = time.perf_counter()
        csi_processor.preprocess_csi_data(sample_csi)
        elapsed = time.perf_counter() - start
        assert elapsed < 0.010  # < 10 ms
