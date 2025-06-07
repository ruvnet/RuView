import pytest
import numpy as np
import asyncio
from unittest.mock import Mock, AsyncMock, patch
from src.core.csi_processor import CSIProcessor


class TestCSIProcessor:
    """Test suite for CSI processor following London School TDD principles"""
    
    @pytest.fixture
    def mock_csi_data(self):
        """Generate synthetic CSI data for testing"""
        # 3x3 MIMO, 56 subcarriers, 100 temporal samples
        amplitude = np.random.uniform(0.1, 2.0, (3, 3, 56, 100))
        phase = np.random.uniform(-np.pi, np.pi, (3, 3, 56, 100))
        return {
            'amplitude': amplitude,
            'phase': phase,
            'timestamp': 1234567890.0,
            'rssi': -45,
            'channel': 6
        }
    
    @pytest.fixture
    def csi_processor(self):
        """Create CSI processor instance for testing"""
        return CSIProcessor()
    
    async def test_process_csi_data_returns_normalized_output(self, csi_processor, mock_csi_data):
        """Test that CSI processing returns properly normalized output"""
        # Act
        result = await csi_processor.process(mock_csi_data)
        
        # Assert
        assert result is not None
        assert 'processed_amplitude' in result
        assert 'processed_phase' in result
        assert result['processed_amplitude'].shape == (3, 3, 56, 100)
        assert result['processed_phase'].shape == (3, 3, 56, 100)
        
        # Verify normalization - values should be in reasonable range
        assert np.all(result['processed_amplitude'] >= 0)
        assert np.all(result['processed_amplitude'] <= 1)
        assert np.all(result['processed_phase'] >= -np.pi)
        assert np.all(result['processed_phase'] <= np.pi)
    
    async def test_process_csi_data_handles_invalid_input(self, csi_processor):
        """Test that CSI processor handles invalid input gracefully"""
        # Arrange
        invalid_data = {'invalid': 'data'}
        
        # Act & Assert
        with pytest.raises(ValueError, match="Invalid CSI data format"):
            await csi_processor.process(invalid_data)
    
    async def test_process_csi_data_removes_nan_values(self, csi_processor, mock_csi_data):
        """Test that CSI processor removes NaN values from input"""
        # Arrange
        mock_csi_data['amplitude'][0, 0, 0, 0] = np.nan
        mock_csi_data['phase'][0, 0, 0, 0] = np.nan
        
        # Act
        result = await csi_processor.process(mock_csi_data)
        
        # Assert
        assert not np.isnan(result['processed_amplitude']).any()
        assert not np.isnan(result['processed_phase']).any()
    
    async def test_process_csi_data_applies_temporal_filtering(self, csi_processor, mock_csi_data):
        """Test that temporal filtering is applied to CSI data"""
        # Arrange - Add noise to make filtering effect visible
        noisy_amplitude = mock_csi_data['amplitude'] + np.random.normal(0, 0.1, mock_csi_data['amplitude'].shape)
        mock_csi_data['amplitude'] = noisy_amplitude
        
        # Act
        result = await csi_processor.process(mock_csi_data)
        
        # Assert - Filtered data should be smoother (lower variance)
        original_variance = np.var(mock_csi_data['amplitude'])
        filtered_variance = np.var(result['processed_amplitude'])
        assert filtered_variance < original_variance
    
    async def test_process_csi_data_preserves_metadata(self, csi_processor, mock_csi_data):
        """Test that metadata is preserved during processing"""
        # Act
        result = await csi_processor.process(mock_csi_data)
        
        # Assert
        assert result['timestamp'] == mock_csi_data['timestamp']
        assert result['rssi'] == mock_csi_data['rssi']
        assert result['channel'] == mock_csi_data['channel']
    
    async def test_process_csi_data_performance_requirement(self, csi_processor, mock_csi_data):
        """Test that CSI processing meets performance requirements (<10ms)"""
        import time
        
        # Act
        start_time = time.time()
        result = await csi_processor.process(mock_csi_data)
        processing_time = time.time() - start_time
        
        # Assert
        assert processing_time < 0.01  # <10ms requirement
        assert result is not None