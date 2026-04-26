import pytest
import numpy as np
import torch
from unittest.mock import Mock, patch, MagicMock
from src.hardware.csi_extractor import CSIExtractor, CSIExtractionError


class TestCSIExtractor:
    """Test suite for CSI Extractor following London School TDD principles"""
    
    @pytest.fixture
    def mock_config(self):
        """Configuration for CSI extractor"""
        return {
            'interface': 'wlan0',
            'channel': 6,
            'bandwidth': 20,
            'sample_rate': 1000,
            'buffer_size': 1024,
            'extraction_timeout': 5.0
        }
    
    @pytest.fixture
    def mock_router_interface(self):
        """Mock router interface for testing"""
        mock_router = Mock()
        mock_router.is_connected = True
        mock_router.execute_command = Mock()
        return mock_router
    
    @pytest.fixture
    def csi_extractor(self, mock_config, mock_router_interface):
        """Create CSI extractor instance for testing"""
        return CSIExtractor(mock_config, mock_router_interface)
    
    @pytest.fixture
    def mock_csi_data(self):
        """Generate synthetic CSI data for testing"""
        # Simulate CSI data: complex values for multiple subcarriers
        num_subcarriers = 56
        num_antennas = 3
        amplitude = np.random.uniform(0.1, 2.0, (num_antennas, num_subcarriers))
        phase = np.random.uniform(-np.pi, np.pi, (num_antennas, num_subcarriers))
        return amplitude * np.exp(1j * phase)
    
    def test_extractor_initialization_creates_correct_configuration(self, mock_config, mock_router_interface):
        """Test that CSI extractor initializes with correct configuration"""
        # Act
        extractor = CSIExtractor(mock_config, mock_router_interface)
        
        # Assert
        assert extractor is not None
        assert extractor.interface == mock_config['interface']
        assert extractor.channel == mock_config['channel']
        assert extractor.bandwidth == mock_config['bandwidth']
        assert extractor.sample_rate == mock_config['sample_rate']
        assert extractor.buffer_size == mock_config['buffer_size']
        assert extractor.extraction_timeout == mock_config['extraction_timeout']
        assert extractor.router_interface == mock_router_interface
        assert not extractor.is_extracting
    
    def test_start_extraction_configures_monitor_mode(self, csi_extractor, mock_router_interface):
        """Test that start_extraction configures monitor mode"""
        # Arrange
        mock_router_interface.enable_monitor_mode.return_value = True
        mock_router_interface.execute_command.return_value = "CSI extraction started"
        
        # Act
        result = csi_extractor.start_extraction()
        
        # Assert
        assert result is True
        assert csi_extractor.is_extracting is True
        mock_router_interface.enable_monitor_mode.assert_called_once_with(csi_extractor.interface)
    
    def test_start_extraction_handles_monitor_mode_failure(self, csi_extractor, mock_router_interface):
        """Test that start_extraction handles monitor mode configuration failure"""
        # Arrange
        mock_router_interface.enable_monitor_mode.return_value = False
        
        # Act & Assert
        with pytest.raises(CSIExtractionError):
            csi_extractor.start_extraction()
        
        assert csi_extractor.is_extracting is False
    
    def test_stop_extraction_disables_monitor_mode(self, csi_extractor, mock_router_interface):
        """Test that stop_extraction disables monitor mode"""
        # Arrange
        mock_router_interface.enable_monitor_mode.return_value = True
        mock_router_interface.disable_monitor_mode.return_value = True
        mock_router_interface.execute_command.return_value = "CSI extraction started"
        
        csi_extractor.start_extraction()
        
        # Act
        result = csi_extractor.stop_extraction()
        
        # Assert
        assert result is True
        assert csi_extractor.is_extracting is False
        mock_router_interface.disable_monitor_mode.assert_called_once_with(csi_extractor.interface)
    
    def test_extract_csi_data_returns_valid_format(self, csi_extractor, mock_router_interface, mock_csi_data):
        """Test that extract_csi_data returns data in valid format"""
        # Arrange
        mock_router_interface.enable_monitor_mode.return_value = True
        mock_router_interface.execute_command.return_value = "CSI extraction started"
        
        # Mock the CSI data extraction
        with patch.object(csi_extractor, '_parse_csi_output', return_value=mock_csi_data):
            csi_extractor.start_extraction()
            
            # Act
            csi_data = csi_extractor.extract_csi_data()
        
        # Assert
        assert csi_data is not None
        assert isinstance(csi_data, np.ndarray)
        assert csi_data.dtype == np.complex128
        assert csi_data.shape == mock_csi_data.shape
    
    def test_extract_csi_data_requires_active_extraction(self, csi_extractor):
        """Test that extract_csi_data requires active extraction"""
        # Act & Assert
        with pytest.raises(CSIExtractionError):
            csi_extractor.extract_csi_data()
    
    def test_extract_csi_data_handles_timeout(self, csi_extractor, mock_router_interface):
        """Test that extract_csi_data handles extraction timeout"""
        # Arrange
        mock_router_interface.enable_monitor_mode.return_value = True
        mock_router_interface.execute_command.side_effect = [
            "CSI extraction started",
            Exception("Timeout")
        ]
        
        csi_extractor.start_extraction()
        
        # Act & Assert
        with pytest.raises(CSIExtractionError):
            csi_extractor.extract_csi_data()
    
    def test_convert_to_tensor_produces_correct_format(self, csi_extractor, mock_csi_data):
        """Test that convert_to_tensor produces correctly formatted tensor"""
        # Act
        tensor = csi_extractor.convert_to_tensor(mock_csi_data)
        
        # Assert
        assert isinstance(tensor, torch.Tensor)
        assert tensor.dtype == torch.float32
        assert tensor.shape[0] == mock_csi_data.shape[0] * 2  # Real and imaginary parts
        assert tensor.shape[1] == mock_csi_data.shape[1]
    
    def test_convert_to_tensor_handles_invalid_input(self, csi_extractor):
        """Test that convert_to_tensor handles invalid input"""
        # Arrange
        invalid_data = "not an array"
        
        # Act & Assert
        with pytest.raises(ValueError):
            csi_extractor.convert_to_tensor(invalid_data)
    
    def test_get_extraction_stats_returns_valid_statistics(self, csi_extractor, mock_router_interface):
        """Test that get_extraction_stats returns valid statistics"""
        # Arrange
        mock_router_interface.enable_monitor_mode.return_value = True
        mock_router_interface.execute_command.return_value = "CSI extraction started"
        
        csi_extractor.start_extraction()
        
        # Act
        stats = csi_extractor.get_extraction_stats()
        
        # Assert
        assert stats is not None
        assert isinstance(stats, dict)
        assert 'samples_extracted' in stats
        assert 'extraction_rate' in stats
        assert 'buffer_utilization' in stats
        assert 'last_extraction_time' in stats
    
    def test_set_channel_configures_wifi_channel(self, csi_extractor, mock_router_interface):
        """Test that set_channel configures WiFi channel"""
        # Arrange
        new_channel = 11
        mock_router_interface.execute_command.return_value = f"Channel set to {new_channel}"
        
        # Act
        result = csi_extractor.set_channel(new_channel)
        
        # Assert
        assert result is True
        assert csi_extractor.channel == new_channel
        mock_router_interface.execute_command.assert_called()
    
    def test_set_channel_validates_channel_range(self, csi_extractor):
        """Test that set_channel validates channel range"""
        # Act & Assert
        with pytest.raises(ValueError):
            csi_extractor.set_channel(0)  # Invalid channel
        
        with pytest.raises(ValueError):
            csi_extractor.set_channel(15)  # Invalid channel
    
    def test_extractor_supports_context_manager(self, csi_extractor, mock_router_interface):
        """Test that CSI extractor supports context manager protocol"""
        # Arrange
        mock_router_interface.enable_monitor_mode.return_value = True
        mock_router_interface.disable_monitor_mode.return_value = True
        mock_router_interface.execute_command.return_value = "CSI extraction started"
        
        # Act
        with csi_extractor as extractor:
            # Assert
            assert extractor.is_extracting is True
        
        # Assert - extraction should be stopped after context
        assert csi_extractor.is_extracting is False
    
    def test_extractor_validates_configuration(self, mock_router_interface):
        """Test that CSI extractor validates configuration parameters"""
        # Arrange
        invalid_config = {
            'interface': '',  # Invalid interface
            'channel': 6,
            'bandwidth': 20
        }
        
        # Act & Assert
        with pytest.raises(ValueError):
            CSIExtractor(invalid_config, mock_router_interface)
    
    def test_parse_csi_output_processes_raw_data(self, csi_extractor):
        """Test that _parse_csi_output processes raw CSI data correctly"""
        # Arrange
        raw_output = "CSI_DATA: 1.5+0.5j,2.0-1.0j,0.8+1.2j"
        
        # Act
        parsed_data = csi_extractor._parse_csi_output(raw_output)
        
        # Assert
        assert parsed_data is not None
        assert isinstance(parsed_data, np.ndarray)
        assert parsed_data.dtype == np.complex128
    
    def test_buffer_management_handles_overflow(self, csi_extractor, mock_router_interface, mock_csi_data):
        """Test that buffer management handles overflow correctly"""
        # Arrange
        mock_router_interface.enable_monitor_mode.return_value = True
        mock_router_interface.execute_command.return_value = "CSI extraction started"
        
        with patch.object(csi_extractor, '_parse_csi_output', return_value=mock_csi_data):
            csi_extractor.start_extraction()
            
            # Fill buffer beyond capacity
            for _ in range(csi_extractor.buffer_size + 10):
                csi_extractor._add_to_buffer(mock_csi_data)
            
            # Act
            stats = csi_extractor.get_extraction_stats()
        
        # Assert
        assert stats['buffer_utilization'] <= 1.0  # Should not exceed 100%