"""TDD tests for phase sanitizer following London School approach."""

import pytest
import numpy as np
import sys
import os
from unittest.mock import Mock, patch, AsyncMock
from datetime import datetime, timezone
import importlib.util

# Import the phase sanitizer module directly
spec = importlib.util.spec_from_file_location(
    'phase_sanitizer', 
    '/workspaces/wifi-densepose/src/core/phase_sanitizer.py'
)
phase_sanitizer_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(phase_sanitizer_module)

# Get classes from the module
PhaseSanitizer = phase_sanitizer_module.PhaseSanitizer
PhaseSanitizationError = phase_sanitizer_module.PhaseSanitizationError


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestPhaseSanitizer:
    """Test phase sanitizer using London School TDD."""

    @pytest.fixture
    def mock_logger(self):
        """Mock logger for testing."""
        return Mock()

    @pytest.fixture
    def sanitizer_config(self):
        """Phase sanitizer configuration for testing."""
        return {
            'unwrapping_method': 'numpy',
            'outlier_threshold': 3.0,
            'smoothing_window': 5,
            'enable_outlier_removal': True,
            'enable_smoothing': True,
            'enable_noise_filtering': True,
            'noise_threshold': 0.1,
            'phase_range': (-np.pi, np.pi)
        }

    @pytest.fixture
    def phase_sanitizer(self, sanitizer_config, mock_logger):
        """Create phase sanitizer for testing."""
        return PhaseSanitizer(config=sanitizer_config, logger=mock_logger)

    @pytest.fixture
    def sample_wrapped_phase(self):
        """Sample wrapped phase data with discontinuities."""
        # Create phase data with wrapping
        phase = np.linspace(0, 4*np.pi, 100)
        wrapped_phase = np.angle(np.exp(1j * phase))  # Wrap to [-π, π]
        return wrapped_phase.reshape(1, -1)  # Shape: (1, 100)

    @pytest.fixture
    def sample_noisy_phase(self):
        """Sample phase data with noise and outliers."""
        clean_phase = np.linspace(-np.pi, np.pi, 50)
        noise = np.random.normal(0, 0.05, 50)
        # Add some outliers
        outliers = np.random.choice(50, 5, replace=False)
        noisy_phase = clean_phase + noise
        noisy_phase[outliers] += np.random.uniform(-2, 2, 5)  # Add outliers
        return noisy_phase.reshape(1, -1)

    # Initialization tests
    def test_should_initialize_with_valid_config(self, sanitizer_config, mock_logger):
        """Should initialize phase sanitizer with valid configuration."""
        sanitizer = PhaseSanitizer(config=sanitizer_config, logger=mock_logger)
        
        assert sanitizer.config == sanitizer_config
        assert sanitizer.logger == mock_logger
        assert sanitizer.unwrapping_method == 'numpy'
        assert sanitizer.outlier_threshold == 3.0
        assert sanitizer.smoothing_window == 5
        assert sanitizer.enable_outlier_removal == True
        assert sanitizer.enable_smoothing == True
        assert sanitizer.enable_noise_filtering == True
        assert sanitizer.noise_threshold == 0.1
        assert sanitizer.phase_range == (-np.pi, np.pi)

    def test_should_raise_error_with_invalid_config(self, mock_logger):
        """Should raise error when initialized with invalid configuration."""
        invalid_config = {'invalid': 'config'}
        
        with pytest.raises(ValueError, match="Missing required configuration"):
            PhaseSanitizer(config=invalid_config, logger=mock_logger)

    def test_should_validate_required_fields(self, mock_logger):
        """Should validate required configuration fields."""
        required_fields = ['unwrapping_method', 'outlier_threshold', 'smoothing_window']
        base_config = {
            'unwrapping_method': 'numpy',
            'outlier_threshold': 3.0,
            'smoothing_window': 5
        }
        
        for field in required_fields:
            config = base_config.copy()
            del config[field]
            
            with pytest.raises(ValueError, match="Missing required configuration"):
                PhaseSanitizer(config=config, logger=mock_logger)

    def test_should_use_default_values(self, mock_logger):
        """Should use default values for optional parameters."""
        minimal_config = {
            'unwrapping_method': 'numpy',
            'outlier_threshold': 3.0,
            'smoothing_window': 5
        }
        
        sanitizer = PhaseSanitizer(config=minimal_config, logger=mock_logger)
        
        assert sanitizer.enable_outlier_removal == True  # default
        assert sanitizer.enable_smoothing == True  # default
        assert sanitizer.enable_noise_filtering == False  # default
        assert sanitizer.noise_threshold == 0.05  # default
        assert sanitizer.phase_range == (-np.pi, np.pi)  # default

    def test_should_initialize_without_logger(self, sanitizer_config):
        """Should initialize without logger provided."""
        sanitizer = PhaseSanitizer(config=sanitizer_config)
        
        assert sanitizer.logger is not None  # Should create default logger

    # Phase unwrapping tests
    def test_should_unwrap_phase_successfully(self, phase_sanitizer, sample_wrapped_phase):
        """Should unwrap phase data successfully."""
        result = phase_sanitizer.unwrap_phase(sample_wrapped_phase)
        
        # Check that result has same shape
        assert result.shape == sample_wrapped_phase.shape
        
        # Check that unwrapping removed discontinuities
        phase_diff = np.diff(result.flatten())
        large_jumps = np.abs(phase_diff) > np.pi
        assert np.sum(large_jumps) < np.sum(np.abs(np.diff(sample_wrapped_phase.flatten())) > np.pi)

    def test_should_handle_different_unwrapping_methods(self, sanitizer_config, mock_logger):
        """Should handle different unwrapping methods."""
        for method in ['numpy', 'scipy', 'custom']:
            sanitizer_config['unwrapping_method'] = method
            sanitizer = PhaseSanitizer(config=sanitizer_config, logger=mock_logger)
            
            phase_data = np.random.uniform(-np.pi, np.pi, (2, 50))
            
            with patch.object(sanitizer, f'_unwrap_{method}', return_value=phase_data) as mock_unwrap:
                result = sanitizer.unwrap_phase(phase_data)
                
                assert result.shape == phase_data.shape
                mock_unwrap.assert_called_once()

    def test_should_handle_unwrapping_error(self, phase_sanitizer):
        """Should handle phase unwrapping errors gracefully."""
        invalid_phase = np.array([[]])  # Empty array
        
        with pytest.raises(PhaseSanitizationError, match="Failed to unwrap phase"):
            phase_sanitizer.unwrap_phase(invalid_phase)

    # Outlier removal tests
    def test_should_remove_outliers_successfully(self, phase_sanitizer, sample_noisy_phase):
        """Should remove outliers from phase data successfully."""
        with patch.object(phase_sanitizer, '_detect_outliers') as mock_detect:
            with patch.object(phase_sanitizer, '_interpolate_outliers') as mock_interpolate:
                outlier_mask = np.zeros(sample_noisy_phase.shape, dtype=bool)
                outlier_mask[0, [10, 20, 30]] = True  # Mark some outliers
                clean_phase = sample_noisy_phase.copy()
                
                mock_detect.return_value = outlier_mask
                mock_interpolate.return_value = clean_phase
                
                result = phase_sanitizer.remove_outliers(sample_noisy_phase)
                
                assert result.shape == sample_noisy_phase.shape
                mock_detect.assert_called_once_with(sample_noisy_phase)
                mock_interpolate.assert_called_once()

    def test_should_skip_outlier_removal_when_disabled(self, sanitizer_config, mock_logger, sample_noisy_phase):
        """Should skip outlier removal when disabled."""
        sanitizer_config['enable_outlier_removal'] = False
        sanitizer = PhaseSanitizer(config=sanitizer_config, logger=mock_logger)
        
        result = sanitizer.remove_outliers(sample_noisy_phase)
        
        assert np.array_equal(result, sample_noisy_phase)

    def test_should_handle_outlier_removal_error(self, phase_sanitizer):
        """Should handle outlier removal errors gracefully."""
        with patch.object(phase_sanitizer, '_detect_outliers') as mock_detect:
            mock_detect.side_effect = Exception("Detection error")
            
            phase_data = np.random.uniform(-np.pi, np.pi, (2, 50))
            
            with pytest.raises(PhaseSanitizationError, match="Failed to remove outliers"):
                phase_sanitizer.remove_outliers(phase_data)

    # Smoothing tests
    def test_should_smooth_phase_successfully(self, phase_sanitizer, sample_noisy_phase):
        """Should smooth phase data successfully."""
        with patch.object(phase_sanitizer, '_apply_moving_average') as mock_smooth:
            smoothed_phase = sample_noisy_phase * 0.9  # Simulate smoothing
            mock_smooth.return_value = smoothed_phase
            
            result = phase_sanitizer.smooth_phase(sample_noisy_phase)
            
            assert result.shape == sample_noisy_phase.shape
            mock_smooth.assert_called_once_with(sample_noisy_phase, phase_sanitizer.smoothing_window)

    def test_should_skip_smoothing_when_disabled(self, sanitizer_config, mock_logger, sample_noisy_phase):
        """Should skip smoothing when disabled."""
        sanitizer_config['enable_smoothing'] = False
        sanitizer = PhaseSanitizer(config=sanitizer_config, logger=mock_logger)
        
        result = sanitizer.smooth_phase(sample_noisy_phase)
        
        assert np.array_equal(result, sample_noisy_phase)

    def test_should_handle_smoothing_error(self, phase_sanitizer):
        """Should handle smoothing errors gracefully."""
        with patch.object(phase_sanitizer, '_apply_moving_average') as mock_smooth:
            mock_smooth.side_effect = Exception("Smoothing error")
            
            phase_data = np.random.uniform(-np.pi, np.pi, (2, 50))
            
            with pytest.raises(PhaseSanitizationError, match="Failed to smooth phase"):
                phase_sanitizer.smooth_phase(phase_data)

    # Noise filtering tests
    def test_should_filter_noise_successfully(self, phase_sanitizer, sample_noisy_phase):
        """Should filter noise from phase data successfully."""
        with patch.object(phase_sanitizer, '_apply_low_pass_filter') as mock_filter:
            filtered_phase = sample_noisy_phase * 0.95  # Simulate filtering
            mock_filter.return_value = filtered_phase
            
            result = phase_sanitizer.filter_noise(sample_noisy_phase)
            
            assert result.shape == sample_noisy_phase.shape
            mock_filter.assert_called_once_with(sample_noisy_phase, phase_sanitizer.noise_threshold)

    def test_should_skip_noise_filtering_when_disabled(self, sanitizer_config, mock_logger, sample_noisy_phase):
        """Should skip noise filtering when disabled."""
        sanitizer_config['enable_noise_filtering'] = False
        sanitizer = PhaseSanitizer(config=sanitizer_config, logger=mock_logger)
        
        result = sanitizer.filter_noise(sample_noisy_phase)
        
        assert np.array_equal(result, sample_noisy_phase)

    def test_should_handle_noise_filtering_error(self, phase_sanitizer):
        """Should handle noise filtering errors gracefully."""
        with patch.object(phase_sanitizer, '_apply_low_pass_filter') as mock_filter:
            mock_filter.side_effect = Exception("Filtering error")
            
            phase_data = np.random.uniform(-np.pi, np.pi, (2, 50))
            
            with pytest.raises(PhaseSanitizationError, match="Failed to filter noise"):
                phase_sanitizer.filter_noise(phase_data)

    # Complete sanitization pipeline tests
    def test_should_sanitize_phase_pipeline_successfully(self, phase_sanitizer, sample_wrapped_phase):
        """Should sanitize phase through complete pipeline successfully."""
        with patch.object(phase_sanitizer, 'unwrap_phase', return_value=sample_wrapped_phase) as mock_unwrap:
            with patch.object(phase_sanitizer, 'remove_outliers', return_value=sample_wrapped_phase) as mock_outliers:
                with patch.object(phase_sanitizer, 'smooth_phase', return_value=sample_wrapped_phase) as mock_smooth:
                    with patch.object(phase_sanitizer, 'filter_noise', return_value=sample_wrapped_phase) as mock_filter:
                        
                        result = phase_sanitizer.sanitize_phase(sample_wrapped_phase)
                        
                        assert result.shape == sample_wrapped_phase.shape
                        mock_unwrap.assert_called_once_with(sample_wrapped_phase)
                        mock_outliers.assert_called_once()
                        mock_smooth.assert_called_once()
                        mock_filter.assert_called_once()

    def test_should_handle_sanitization_pipeline_error(self, phase_sanitizer, sample_wrapped_phase):
        """Should handle sanitization pipeline errors gracefully."""
        with patch.object(phase_sanitizer, 'unwrap_phase') as mock_unwrap:
            mock_unwrap.side_effect = PhaseSanitizationError("Unwrapping failed")
            
            with pytest.raises(PhaseSanitizationError):
                phase_sanitizer.sanitize_phase(sample_wrapped_phase)

    # Phase validation tests
    def test_should_validate_phase_data_successfully(self, phase_sanitizer):
        """Should validate phase data successfully."""
        valid_phase = np.random.uniform(-np.pi, np.pi, (3, 56))
        
        result = phase_sanitizer.validate_phase_data(valid_phase)
        
        assert result == True

    def test_should_reject_invalid_phase_shape(self, phase_sanitizer):
        """Should reject phase data with invalid shape."""
        invalid_phase = np.array([1, 2, 3])  # 1D array
        
        with pytest.raises(PhaseSanitizationError, match="Phase data must be 2D"):
            phase_sanitizer.validate_phase_data(invalid_phase)

    def test_should_reject_empty_phase_data(self, phase_sanitizer):
        """Should reject empty phase data."""
        empty_phase = np.array([]).reshape(0, 0)
        
        with pytest.raises(PhaseSanitizationError, match="Phase data cannot be empty"):
            phase_sanitizer.validate_phase_data(empty_phase)

    def test_should_reject_phase_out_of_range(self, phase_sanitizer):
        """Should reject phase data outside valid range."""
        invalid_phase = np.array([[10.0, -10.0, 5.0, -5.0]])  # Outside [-π, π]
        
        with pytest.raises(PhaseSanitizationError, match="Phase values outside valid range"):
            phase_sanitizer.validate_phase_data(invalid_phase)

    # Statistics and monitoring tests
    def test_should_get_sanitization_statistics(self, phase_sanitizer):
        """Should get sanitization statistics."""
        # Simulate some processing
        phase_sanitizer._total_processed = 50
        phase_sanitizer._outliers_removed = 5
        phase_sanitizer._sanitization_errors = 2
        
        stats = phase_sanitizer.get_sanitization_statistics()
        
        assert isinstance(stats, dict)
        assert stats['total_processed'] == 50
        assert stats['outliers_removed'] == 5
        assert stats['sanitization_errors'] == 2
        assert stats['outlier_rate'] == 0.1
        assert stats['error_rate'] == 0.04

    def test_should_reset_statistics(self, phase_sanitizer):
        """Should reset sanitization statistics."""
        phase_sanitizer._total_processed = 50
        phase_sanitizer._outliers_removed = 5
        phase_sanitizer._sanitization_errors = 2
        
        phase_sanitizer.reset_statistics()
        
        assert phase_sanitizer._total_processed == 0
        assert phase_sanitizer._outliers_removed == 0
        assert phase_sanitizer._sanitization_errors == 0

    # Configuration validation tests
    def test_should_validate_unwrapping_method(self, mock_logger):
        """Should validate unwrapping method."""
        invalid_config = {
            'unwrapping_method': 'invalid_method',
            'outlier_threshold': 3.0,
            'smoothing_window': 5
        }
        
        with pytest.raises(ValueError, match="Invalid unwrapping method"):
            PhaseSanitizer(config=invalid_config, logger=mock_logger)

    def test_should_validate_outlier_threshold(self, mock_logger):
        """Should validate outlier threshold."""
        invalid_config = {
            'unwrapping_method': 'numpy',
            'outlier_threshold': -1.0,  # Negative threshold
            'smoothing_window': 5
        }
        
        with pytest.raises(ValueError, match="outlier_threshold must be positive"):
            PhaseSanitizer(config=invalid_config, logger=mock_logger)

    def test_should_validate_smoothing_window(self, mock_logger):
        """Should validate smoothing window."""
        invalid_config = {
            'unwrapping_method': 'numpy',
            'outlier_threshold': 3.0,
            'smoothing_window': 0  # Invalid window size
        }
        
        with pytest.raises(ValueError, match="smoothing_window must be positive"):
            PhaseSanitizer(config=invalid_config, logger=mock_logger)

    # Edge case tests
    def test_should_handle_single_antenna_data(self, phase_sanitizer):
        """Should handle single antenna phase data."""
        single_antenna_phase = np.random.uniform(-np.pi, np.pi, (1, 56))
        
        result = phase_sanitizer.sanitize_phase(single_antenna_phase)
        
        assert result.shape == single_antenna_phase.shape

    def test_should_handle_small_phase_arrays(self, phase_sanitizer):
        """Should handle small phase arrays."""
        small_phase = np.random.uniform(-np.pi, np.pi, (2, 5))
        
        result = phase_sanitizer.sanitize_phase(small_phase)
        
        assert result.shape == small_phase.shape

    def test_should_handle_constant_phase_data(self, phase_sanitizer):
        """Should handle constant phase data."""
        constant_phase = np.full((3, 20), 0.5)
        
        result = phase_sanitizer.sanitize_phase(constant_phase)
        
        assert result.shape == constant_phase.shape