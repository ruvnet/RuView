"""TDD tests for CSI processor following London School approach."""

import pytest
import numpy as np
import sys
import os
from unittest.mock import Mock, patch, AsyncMock, MagicMock
from datetime import datetime, timezone
import importlib.util
from typing import Dict, List, Any

# Import the CSI processor module directly
spec = importlib.util.spec_from_file_location(
    'csi_processor', 
    '/workspaces/wifi-densepose/src/core/csi_processor.py'
)
csi_processor_module = importlib.util.module_from_spec(spec)

# Import CSI extractor for dependencies
csi_spec = importlib.util.spec_from_file_location(
    'csi_extractor', 
    '/workspaces/wifi-densepose/src/hardware/csi_extractor.py'
)
csi_module = importlib.util.module_from_spec(csi_spec)
csi_spec.loader.exec_module(csi_module)

# Make dependencies available and load the processor
csi_processor_module.CSIData = csi_module.CSIData
spec.loader.exec_module(csi_processor_module)

# Get classes from modules
CSIProcessor = csi_processor_module.CSIProcessor
CSIProcessingError = csi_processor_module.CSIProcessingError
HumanDetectionResult = csi_processor_module.HumanDetectionResult
CSIFeatures = csi_processor_module.CSIFeatures
CSIData = csi_module.CSIData


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestCSIProcessor:
    """Test CSI processor using London School TDD."""

    @pytest.fixture
    def mock_logger(self):
        """Mock logger for testing."""
        return Mock()

    @pytest.fixture
    def processor_config(self):
        """CSI processor configuration for testing."""
        return {
            'sampling_rate': 100,
            'window_size': 256,
            'overlap': 0.5,
            'noise_threshold': -60.0,
            'human_detection_threshold': 0.7,
            'smoothing_factor': 0.8,
            'max_history_size': 1000,
            'enable_preprocessing': True,
            'enable_feature_extraction': True,
            'enable_human_detection': True
        }

    @pytest.fixture
    def csi_processor(self, processor_config, mock_logger):
        """Create CSI processor for testing."""
        return CSIProcessor(config=processor_config, logger=mock_logger)

    @pytest.fixture
    def sample_csi_data(self):
        """Sample CSI data for testing."""
        return CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.random.rand(3, 56) + 1.0,  # Ensure positive amplitude
            phase=np.random.uniform(-np.pi, np.pi, (3, 56)),
            frequency=2.4e9,
            bandwidth=20e6,
            num_subcarriers=56,
            num_antennas=3,
            snr=15.5,
            metadata={'source': 'test'}
        )

    @pytest.fixture
    def sample_features(self):
        """Sample CSI features for testing."""
        return CSIFeatures(
            amplitude_mean=np.random.rand(56),
            amplitude_variance=np.random.rand(56),
            phase_difference=np.random.rand(56),
            correlation_matrix=np.random.rand(3, 3),
            doppler_shift=np.random.rand(10),
            power_spectral_density=np.random.rand(128),
            timestamp=datetime.now(timezone.utc),
            metadata={'processing_params': {}}
        )

    # Initialization tests
    def test_should_initialize_with_valid_config(self, processor_config, mock_logger):
        """Should initialize CSI processor with valid configuration."""
        processor = CSIProcessor(config=processor_config, logger=mock_logger)
        
        assert processor.config == processor_config
        assert processor.logger == mock_logger
        assert processor.sampling_rate == 100
        assert processor.window_size == 256
        assert processor.overlap == 0.5
        assert processor.noise_threshold == -60.0
        assert processor.human_detection_threshold == 0.7
        assert processor.smoothing_factor == 0.8
        assert processor.max_history_size == 1000
        assert len(processor.csi_history) == 0

    def test_should_raise_error_with_invalid_config(self, mock_logger):
        """Should raise error when initialized with invalid configuration."""
        invalid_config = {'invalid': 'config'}
        
        with pytest.raises(ValueError, match="Missing required configuration"):
            CSIProcessor(config=invalid_config, logger=mock_logger)

    def test_should_validate_required_fields(self, mock_logger):
        """Should validate all required configuration fields."""
        required_fields = ['sampling_rate', 'window_size', 'overlap', 'noise_threshold']
        base_config = {
            'sampling_rate': 100,
            'window_size': 256,
            'overlap': 0.5,
            'noise_threshold': -60.0
        }
        
        for field in required_fields:
            config = base_config.copy()
            del config[field]
            
            with pytest.raises(ValueError, match="Missing required configuration"):
                CSIProcessor(config=config, logger=mock_logger)

    def test_should_use_default_values(self, mock_logger):
        """Should use default values for optional parameters."""
        minimal_config = {
            'sampling_rate': 100,
            'window_size': 256,
            'overlap': 0.5,
            'noise_threshold': -60.0
        }
        
        processor = CSIProcessor(config=minimal_config, logger=mock_logger)
        
        assert processor.human_detection_threshold == 0.8  # default
        assert processor.smoothing_factor == 0.9  # default
        assert processor.max_history_size == 500  # default

    def test_should_initialize_without_logger(self, processor_config):
        """Should initialize without logger provided."""
        processor = CSIProcessor(config=processor_config)
        
        assert processor.logger is not None  # Should create default logger

    # Preprocessing tests
    def test_should_preprocess_csi_data_successfully(self, csi_processor, sample_csi_data):
        """Should preprocess CSI data successfully."""
        with patch.object(csi_processor, '_remove_noise') as mock_noise:
            with patch.object(csi_processor, '_apply_windowing') as mock_window:
                with patch.object(csi_processor, '_normalize_amplitude') as mock_normalize:
                    mock_noise.return_value = sample_csi_data
                    mock_window.return_value = sample_csi_data
                    mock_normalize.return_value = sample_csi_data
                    
                    result = csi_processor.preprocess_csi_data(sample_csi_data)
                    
                    assert result == sample_csi_data
                    mock_noise.assert_called_once_with(sample_csi_data)
                    mock_window.assert_called_once()
                    mock_normalize.assert_called_once()

    def test_should_skip_preprocessing_when_disabled(self, processor_config, mock_logger, sample_csi_data):
        """Should skip preprocessing when disabled."""
        processor_config['enable_preprocessing'] = False
        processor = CSIProcessor(config=processor_config, logger=mock_logger)
        
        result = processor.preprocess_csi_data(sample_csi_data)
        
        assert result == sample_csi_data

    def test_should_handle_preprocessing_error(self, csi_processor, sample_csi_data):
        """Should handle preprocessing errors gracefully."""
        with patch.object(csi_processor, '_remove_noise') as mock_noise:
            mock_noise.side_effect = Exception("Preprocessing error")
            
            with pytest.raises(CSIProcessingError, match="Failed to preprocess CSI data"):
                csi_processor.preprocess_csi_data(sample_csi_data)

    # Feature extraction tests
    def test_should_extract_features_successfully(self, csi_processor, sample_csi_data, sample_features):
        """Should extract features from CSI data successfully."""
        with patch.object(csi_processor, '_extract_amplitude_features') as mock_amp:
            with patch.object(csi_processor, '_extract_phase_features') as mock_phase:
                with patch.object(csi_processor, '_extract_correlation_features') as mock_corr:
                    with patch.object(csi_processor, '_extract_doppler_features') as mock_doppler:
                        mock_amp.return_value = (sample_features.amplitude_mean, sample_features.amplitude_variance)
                        mock_phase.return_value = sample_features.phase_difference
                        mock_corr.return_value = sample_features.correlation_matrix
                        mock_doppler.return_value = (sample_features.doppler_shift, sample_features.power_spectral_density)
                        
                        result = csi_processor.extract_features(sample_csi_data)
                        
                        assert isinstance(result, CSIFeatures)
                        assert np.array_equal(result.amplitude_mean, sample_features.amplitude_mean)
                        assert np.array_equal(result.amplitude_variance, sample_features.amplitude_variance)
                        mock_amp.assert_called_once()
                        mock_phase.assert_called_once()
                        mock_corr.assert_called_once()
                        mock_doppler.assert_called_once()

    def test_should_skip_feature_extraction_when_disabled(self, processor_config, mock_logger, sample_csi_data):
        """Should skip feature extraction when disabled."""
        processor_config['enable_feature_extraction'] = False
        processor = CSIProcessor(config=processor_config, logger=mock_logger)
        
        result = processor.extract_features(sample_csi_data)
        
        assert result is None

    def test_should_handle_feature_extraction_error(self, csi_processor, sample_csi_data):
        """Should handle feature extraction errors gracefully."""
        with patch.object(csi_processor, '_extract_amplitude_features') as mock_amp:
            mock_amp.side_effect = Exception("Feature extraction error")
            
            with pytest.raises(CSIProcessingError, match="Failed to extract features"):
                csi_processor.extract_features(sample_csi_data)

    # Human detection tests
    def test_should_detect_human_presence_successfully(self, csi_processor, sample_features):
        """Should detect human presence successfully."""
        with patch.object(csi_processor, '_analyze_motion_patterns') as mock_motion:
            with patch.object(csi_processor, '_calculate_detection_confidence') as mock_confidence:
                with patch.object(csi_processor, '_apply_temporal_smoothing') as mock_smooth:
                    mock_motion.return_value = 0.9
                    mock_confidence.return_value = 0.85
                    mock_smooth.return_value = 0.88
                    
                    result = csi_processor.detect_human_presence(sample_features)
                    
                    assert isinstance(result, HumanDetectionResult)
                    assert result.human_detected == True
                    assert result.confidence == 0.88
                    assert result.motion_score == 0.9
                    mock_motion.assert_called_once()
                    mock_confidence.assert_called_once()
                    mock_smooth.assert_called_once()

    def test_should_detect_no_human_presence(self, csi_processor, sample_features):
        """Should detect no human presence when confidence is low."""
        with patch.object(csi_processor, '_analyze_motion_patterns') as mock_motion:
            with patch.object(csi_processor, '_calculate_detection_confidence') as mock_confidence:
                with patch.object(csi_processor, '_apply_temporal_smoothing') as mock_smooth:
                    mock_motion.return_value = 0.3
                    mock_confidence.return_value = 0.2
                    mock_smooth.return_value = 0.25
                    
                    result = csi_processor.detect_human_presence(sample_features)
                    
                    assert result.human_detected == False
                    assert result.confidence == 0.25
                    assert result.motion_score == 0.3

    def test_should_skip_human_detection_when_disabled(self, processor_config, mock_logger, sample_features):
        """Should skip human detection when disabled."""
        processor_config['enable_human_detection'] = False
        processor = CSIProcessor(config=processor_config, logger=mock_logger)
        
        result = processor.detect_human_presence(sample_features)
        
        assert result is None

    def test_should_handle_human_detection_error(self, csi_processor, sample_features):
        """Should handle human detection errors gracefully."""
        with patch.object(csi_processor, '_analyze_motion_patterns') as mock_motion:
            mock_motion.side_effect = Exception("Detection error")
            
            with pytest.raises(CSIProcessingError, match="Failed to detect human presence"):
                csi_processor.detect_human_presence(sample_features)

    # Processing pipeline tests
    @pytest.mark.asyncio
    async def test_should_process_csi_data_pipeline_successfully(self, csi_processor, sample_csi_data, sample_features):
        """Should process CSI data through full pipeline successfully."""
        expected_detection = HumanDetectionResult(
            human_detected=True,
            confidence=0.85,
            motion_score=0.9,
            timestamp=datetime.now(timezone.utc),
            features=sample_features,
            metadata={}
        )
        
        with patch.object(csi_processor, 'preprocess_csi_data', return_value=sample_csi_data) as mock_preprocess:
            with patch.object(csi_processor, 'extract_features', return_value=sample_features) as mock_features:
                with patch.object(csi_processor, 'detect_human_presence', return_value=expected_detection) as mock_detect:
                    
                    result = await csi_processor.process_csi_data(sample_csi_data)
                    
                    assert result == expected_detection
                    mock_preprocess.assert_called_once_with(sample_csi_data)
                    mock_features.assert_called_once_with(sample_csi_data)
                    mock_detect.assert_called_once_with(sample_features)

    @pytest.mark.asyncio
    async def test_should_handle_pipeline_processing_error(self, csi_processor, sample_csi_data):
        """Should handle pipeline processing errors gracefully."""
        with patch.object(csi_processor, 'preprocess_csi_data') as mock_preprocess:
            mock_preprocess.side_effect = CSIProcessingError("Pipeline error")
            
            with pytest.raises(CSIProcessingError):
                await csi_processor.process_csi_data(sample_csi_data)

    # History management tests
    def test_should_add_csi_data_to_history(self, csi_processor, sample_csi_data):
        """Should add CSI data to history successfully."""
        csi_processor.add_to_history(sample_csi_data)
        
        assert len(csi_processor.csi_history) == 1
        assert csi_processor.csi_history[0] == sample_csi_data

    def test_should_maintain_history_size_limit(self, processor_config, mock_logger):
        """Should maintain history size within limits."""
        processor_config['max_history_size'] = 2
        processor = CSIProcessor(config=processor_config, logger=mock_logger)
        
        # Add 3 items to history of size 2
        for i in range(3):
            csi_data = CSIData(
                timestamp=datetime.now(timezone.utc),
                amplitude=np.random.rand(3, 56),
                phase=np.random.rand(3, 56),
                frequency=2.4e9,
                bandwidth=20e6,
                num_subcarriers=56,
                num_antennas=3,
                snr=15.5,
                metadata={'index': i}
            )
            processor.add_to_history(csi_data)
        
        assert len(processor.csi_history) == 2
        assert processor.csi_history[0].metadata['index'] == 1  # First item removed
        assert processor.csi_history[1].metadata['index'] == 2

    def test_should_clear_history(self, csi_processor, sample_csi_data):
        """Should clear history successfully."""
        csi_processor.add_to_history(sample_csi_data)
        assert len(csi_processor.csi_history) > 0
        
        csi_processor.clear_history()
        
        assert len(csi_processor.csi_history) == 0

    def test_should_get_recent_history(self, csi_processor):
        """Should get recent history entries."""
        # Add 5 items to history
        for i in range(5):
            csi_data = CSIData(
                timestamp=datetime.now(timezone.utc),
                amplitude=np.random.rand(3, 56),
                phase=np.random.rand(3, 56),
                frequency=2.4e9,
                bandwidth=20e6,
                num_subcarriers=56,
                num_antennas=3,
                snr=15.5,
                metadata={'index': i}
            )
            csi_processor.add_to_history(csi_data)
        
        recent = csi_processor.get_recent_history(3)
        
        assert len(recent) == 3
        assert recent[0].metadata['index'] == 2  # Most recent first
        assert recent[1].metadata['index'] == 3
        assert recent[2].metadata['index'] == 4

    # Statistics and monitoring tests
    def test_should_get_processing_statistics(self, csi_processor):
        """Should get processing statistics."""
        # Simulate some processing
        csi_processor._total_processed = 100
        csi_processor._processing_errors = 5
        csi_processor._human_detections = 25
        
        stats = csi_processor.get_processing_statistics()
        
        assert isinstance(stats, dict)
        assert stats['total_processed'] == 100
        assert stats['processing_errors'] == 5
        assert stats['human_detections'] == 25
        assert stats['error_rate'] == 0.05
        assert stats['detection_rate'] == 0.25

    def test_should_reset_statistics(self, csi_processor):
        """Should reset processing statistics."""
        csi_processor._total_processed = 100
        csi_processor._processing_errors = 5
        csi_processor._human_detections = 25
        
        csi_processor.reset_statistics()
        
        assert csi_processor._total_processed == 0
        assert csi_processor._processing_errors == 0
        assert csi_processor._human_detections == 0


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestCSIFeatures:
    """Test CSI features data structure."""

    def test_should_create_csi_features(self):
        """Should create CSI features successfully."""
        features = CSIFeatures(
            amplitude_mean=np.random.rand(56),
            amplitude_variance=np.random.rand(56),
            phase_difference=np.random.rand(56),
            correlation_matrix=np.random.rand(3, 3),
            doppler_shift=np.random.rand(10),
            power_spectral_density=np.random.rand(128),
            timestamp=datetime.now(timezone.utc),
            metadata={'test': 'data'}
        )
        
        assert features.amplitude_mean.shape == (56,)
        assert features.amplitude_variance.shape == (56,)
        assert features.phase_difference.shape == (56,)
        assert features.correlation_matrix.shape == (3, 3)
        assert features.doppler_shift.shape == (10,)
        assert features.power_spectral_density.shape == (128,)
        assert isinstance(features.timestamp, datetime)
        assert features.metadata['test'] == 'data'


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestHumanDetectionResult:
    """Test human detection result data structure."""

    @pytest.fixture
    def sample_features(self):
        """Sample features for testing."""
        return CSIFeatures(
            amplitude_mean=np.random.rand(56),
            amplitude_variance=np.random.rand(56),
            phase_difference=np.random.rand(56),
            correlation_matrix=np.random.rand(3, 3),
            doppler_shift=np.random.rand(10),
            power_spectral_density=np.random.rand(128),
            timestamp=datetime.now(timezone.utc),
            metadata={}
        )

    def test_should_create_detection_result(self, sample_features):
        """Should create human detection result successfully."""
        result = HumanDetectionResult(
            human_detected=True,
            confidence=0.85,
            motion_score=0.92,
            timestamp=datetime.now(timezone.utc),
            features=sample_features,
            metadata={'test': 'data'}
        )
        
        assert result.human_detected == True
        assert result.confidence == 0.85
        assert result.motion_score == 0.92
        assert isinstance(result.timestamp, datetime)
        assert result.features == sample_features
        assert result.metadata['test'] == 'data'