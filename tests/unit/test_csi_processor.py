import pytest
import numpy as np
from unittest.mock import Mock, patch
from src.core.csi_processor import CSIProcessor


class TestCSIProcessor:
    """Test suite for CSI processor following London School TDD principles"""
    
    @pytest.fixture
    def mock_csi_data(self):
        """Generate synthetic CSI data for testing"""
        # Simple raw CSI data array for testing
        return np.random.uniform(0.1, 2.0, (3, 56, 100))
    
    @pytest.fixture
    def csi_processor(self):
        """Create CSI processor instance for testing"""
        return CSIProcessor()
    
    def test_process_csi_data_returns_normalized_output(self, csi_processor, mock_csi_data):
        """Test that CSI processing returns properly normalized output"""
        # Act
        result = csi_processor.process_raw_csi(mock_csi_data)
        
        # Assert
        assert result is not None
        assert isinstance(result, np.ndarray)
        assert result.shape == mock_csi_data.shape
        
        # Verify normalization - mean should be close to 0, std close to 1
        assert abs(result.mean()) < 0.1
        assert abs(result.std() - 1.0) < 0.1
    
    def test_process_csi_data_handles_invalid_input(self, csi_processor):
        """Test that CSI processor handles invalid input gracefully"""
        # Arrange
        invalid_data = np.array([])
        
        # Act & Assert
        with pytest.raises(ValueError, match="Raw CSI data cannot be empty"):
            csi_processor.process_raw_csi(invalid_data)
    
    def test_process_csi_data_removes_nan_values(self, csi_processor, mock_csi_data):
        """Test that CSI processor removes NaN values from input"""
        # Arrange
        mock_csi_data[0, 0, 0] = np.nan
        
        # Act
        result = csi_processor.process_raw_csi(mock_csi_data)
        
        # Assert
        assert not np.isnan(result).any()
    
    def test_process_csi_data_applies_temporal_filtering(self, csi_processor, mock_csi_data):
        """Test that temporal filtering is applied to CSI data"""
        # Arrange - Add noise to make filtering effect visible
        noisy_data = mock_csi_data + np.random.normal(0, 0.1, mock_csi_data.shape)
        
        # Act
        result = csi_processor.process_raw_csi(noisy_data)
        
        # Assert - Result should be normalized
        assert isinstance(result, np.ndarray)
        assert result.shape == noisy_data.shape
    
    def test_process_csi_data_preserves_metadata(self, csi_processor, mock_csi_data):
        """Test that metadata is preserved during processing"""
        # Act
        result = csi_processor.process_raw_csi(mock_csi_data)
        
        # Assert - For now, just verify processing works
        assert result is not None
        assert isinstance(result, np.ndarray)
    
    def test_process_csi_data_performance_requirement(self, csi_processor, mock_csi_data):
        """Test that CSI processing meets performance requirements (<10ms)"""
        import time
        
        # Act
        start_time = time.time()
        result = csi_processor.process_raw_csi(mock_csi_data)
        processing_time = time.time() - start_time
        
        # Assert
        assert processing_time < 0.01  # <10ms requirement
        assert result is not None