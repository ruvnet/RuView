import pytest
import numpy as np
import time
from unittest.mock import Mock, patch
from src.core.phase_sanitizer import PhaseSanitizer, PhaseSanitizationError


_SANITIZER_CONFIG = {
    "unwrapping_method": "numpy",
    "outlier_threshold": 3.0,
    "smoothing_window": 5,
    "enable_outlier_removal": True,
    "enable_smoothing": True,
    "enable_noise_filtering": True,
    "noise_threshold": 0.1,
}


class TestPhaseSanitizer:
    """Test suite for Phase Sanitizer following London School TDD principles"""

    @pytest.fixture
    def mock_phase_data(self):
        """Generate synthetic phase data strictly within valid [-π, π] range"""
        return np.array([
            [0.1, 0.2, 0.4, 0.3, 0.5],
            [-1.0, -0.1, 0.0, 0.1, 0.2],
            [0.0, 0.1, 0.2, 0.3, 0.4],
        ])

    @pytest.fixture
    def phase_sanitizer(self):
        """Create Phase Sanitizer instance for testing"""
        return PhaseSanitizer(config=_SANITIZER_CONFIG)

    def test_unwrap_phase_removes_discontinuities(self, phase_sanitizer):
        """Test that phase unwrapping removes 2π discontinuities"""
        # Create data with explicit 2π jump
        jumpy = np.array([[0.1, 0.2, 0.2 + 2 * np.pi, 0.4, 0.5]])
        result = phase_sanitizer.unwrap_phase(jumpy)

        assert result is not None
        assert isinstance(result, np.ndarray)
        assert result.shape == jumpy.shape
        phase_diffs = np.abs(np.diff(result[0]))
        assert np.all(phase_diffs < np.pi)  # No jumps larger than π

    def test_remove_outliers_returns_same_shape(self, phase_sanitizer, mock_phase_data):
        """Test that outlier removal preserves array shape"""
        result = phase_sanitizer.remove_outliers(mock_phase_data)

        assert result is not None
        assert isinstance(result, np.ndarray)
        assert result.shape == mock_phase_data.shape

    def test_smooth_phase_reduces_noise(self, phase_sanitizer, mock_phase_data):
        """Test that phase smoothing reduces noise while preserving trends"""
        rng = np.random.default_rng(42)
        noisy_data = mock_phase_data + rng.normal(0, 0.05, mock_phase_data.shape)
        # Clip to valid range after adding noise
        noisy_data = np.clip(noisy_data, -np.pi, np.pi)

        result = phase_sanitizer.smooth_phase(noisy_data)

        assert result is not None
        assert isinstance(result, np.ndarray)
        assert result.shape == noisy_data.shape
        assert np.var(result) <= np.var(noisy_data)

    def test_sanitize_raises_for_1d_input(self, phase_sanitizer):
        """Sanitizer should raise PhaseSanitizationError on 1D input"""
        with pytest.raises(PhaseSanitizationError, match="Phase data must be 2D array"):
            phase_sanitizer.sanitize_phase(np.array([0.1, 0.2, 0.3]))

    def test_sanitize_raises_for_empty_2d_input(self, phase_sanitizer):
        """Sanitizer should raise PhaseSanitizationError on empty 2D input"""
        with pytest.raises(PhaseSanitizationError, match="Phase data cannot be empty"):
            phase_sanitizer.sanitize_phase(np.empty((0, 5)))

    def test_sanitize_full_pipeline_integration(self, phase_sanitizer, mock_phase_data):
        """Test that full sanitization pipeline works correctly"""
        result = phase_sanitizer.sanitize_phase(mock_phase_data)

        assert result is not None
        assert isinstance(result, np.ndarray)
        assert result.shape == mock_phase_data.shape
        assert np.all(np.isfinite(result))

    def test_sanitize_performance_requirement(self, phase_sanitizer, mock_phase_data):
        """Test that phase sanitization meets performance requirements (<5ms)"""
        start_time = time.perf_counter()
        phase_sanitizer.sanitize_phase(mock_phase_data)
        processing_time = time.perf_counter() - start_time

        assert processing_time < 0.005  # < 5 ms
