import pytest
import numpy as np
from unittest.mock import Mock, patch
from src.core.phase_sanitizer import PhaseSanitizer


class TestPhaseSanitizer:
    """Test suite for Phase Sanitizer following London School TDD principles"""
    
    @pytest.fixture
    def mock_phase_data(self):
        """Generate synthetic phase data for testing"""
        # Phase data with unwrapping issues and outliers
        return np.array([
            [0.1, 0.2, 6.0, 0.4, 0.5],  # Contains phase jump at index 2
            [-3.0, -0.1, 0.0, 0.1, 0.2],  # Contains wrapped phase at index 0
            [0.0, 0.1, 0.2, 0.3, 0.4]   # Clean phase data
        ])
    
    @pytest.fixture
    def phase_sanitizer(self):
        """Create Phase Sanitizer instance for testing"""
        return PhaseSanitizer()
    
    def test_unwrap_phase_removes_discontinuities(self, phase_sanitizer, mock_phase_data):
        """Test that phase unwrapping removes 2π discontinuities"""
        # Act
        result = phase_sanitizer.unwrap_phase(mock_phase_data)
        
        # Assert
        assert result is not None
        assert isinstance(result, np.ndarray)
        assert result.shape == mock_phase_data.shape
        
        # Check that large jumps are reduced
        for i in range(result.shape[0]):
            phase_diffs = np.abs(np.diff(result[i]))
            assert np.all(phase_diffs < np.pi)  # No jumps larger than π
    
    def test_remove_outliers_filters_anomalous_values(self, phase_sanitizer, mock_phase_data):
        """Test that outlier removal filters anomalous phase values"""
        # Arrange - Add clear outliers
        outlier_data = mock_phase_data.copy()
        outlier_data[0, 2] = 100.0  # Clear outlier
        
        # Act
        result = phase_sanitizer.remove_outliers(outlier_data)
        
        # Assert
        assert result is not None
        assert isinstance(result, np.ndarray)
        assert result.shape == outlier_data.shape
        assert np.abs(result[0, 2]) < 10.0  # Outlier should be corrected
    
    def test_smooth_phase_reduces_noise(self, phase_sanitizer, mock_phase_data):
        """Test that phase smoothing reduces noise while preserving trends"""
        # Arrange - Add noise
        noisy_data = mock_phase_data + np.random.normal(0, 0.1, mock_phase_data.shape)
        
        # Act
        result = phase_sanitizer.smooth_phase(noisy_data)
        
        # Assert
        assert result is not None
        assert isinstance(result, np.ndarray)
        assert result.shape == noisy_data.shape
        
        # Smoothed data should have lower variance
        original_variance = np.var(noisy_data)
        smoothed_variance = np.var(result)
        assert smoothed_variance <= original_variance
    
    def test_sanitize_handles_empty_input(self, phase_sanitizer):
        """Test that sanitizer handles empty input gracefully"""
        # Arrange
        empty_data = np.array([])
        
        # Act & Assert
        with pytest.raises(ValueError, match="Phase data cannot be empty"):
            phase_sanitizer.sanitize(empty_data)
    
    def test_sanitize_full_pipeline_integration(self, phase_sanitizer, mock_phase_data):
        """Test that full sanitization pipeline works correctly"""
        # Act
        result = phase_sanitizer.sanitize(mock_phase_data)
        
        # Assert
        assert result is not None
        assert isinstance(result, np.ndarray)
        assert result.shape == mock_phase_data.shape
        
        # Result should be within reasonable phase bounds
        assert np.all(result >= -2*np.pi)
        assert np.all(result <= 2*np.pi)
    
    def test_sanitize_performance_requirement(self, phase_sanitizer, mock_phase_data):
        """Test that phase sanitization meets performance requirements (<5ms)"""
        import time
        
        # Act
        start_time = time.time()
        result = phase_sanitizer.sanitize(mock_phase_data)
        processing_time = time.time() - start_time
        
        # Assert
        assert processing_time < 0.005  # <5ms requirement
        assert result is not None