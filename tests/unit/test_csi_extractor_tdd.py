"""Test-Driven Development tests for CSI extractor using London School approach."""

import pytest
import numpy as np
from unittest.mock import Mock, patch, AsyncMock, MagicMock
from typing import Dict, Any, Optional
import asyncio
from datetime import datetime, timezone

from src.hardware.csi_extractor import (
    CSIExtractor,
    CSIParseError,
    CSIData,
    ESP32CSIParser,
    RouterCSIParser,
    CSIValidationError
)


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestCSIExtractor:
    """Test CSI extractor using London School TDD - focus on interactions and behavior."""

    @pytest.fixture
    def mock_logger(self):
        """Mock logger for testing."""
        return Mock()

    @pytest.fixture
    def mock_config(self):
        """Mock configuration for CSI extractor."""
        return {
            'hardware_type': 'esp32',
            'sampling_rate': 100,
            'buffer_size': 1024,
            'timeout': 5.0,
            'validation_enabled': True,
            'retry_attempts': 3
        }

    @pytest.fixture
    def csi_extractor(self, mock_config, mock_logger):
        """Create CSI extractor instance for testing."""
        return CSIExtractor(config=mock_config, logger=mock_logger)

    @pytest.fixture
    def sample_csi_data(self):
        """Sample CSI data for testing."""
        return CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.random.rand(3, 56),
            phase=np.random.rand(3, 56),
            frequency=2.4e9,
            bandwidth=20e6,
            num_subcarriers=56,
            num_antennas=3,
            snr=15.5,
            metadata={'source': 'esp32', 'channel': 6}
        )

    def test_should_initialize_with_valid_config(self, mock_config, mock_logger):
        """Should initialize CSI extractor with valid configuration."""
        extractor = CSIExtractor(config=mock_config, logger=mock_logger)
        
        assert extractor.config == mock_config
        assert extractor.logger == mock_logger
        assert extractor.is_connected == False
        assert extractor.hardware_type == 'esp32'

    def test_should_raise_error_with_invalid_config(self, mock_logger):
        """Should raise error when initialized with invalid configuration."""
        invalid_config = {'invalid': 'config'}
        
        with pytest.raises(ValueError, match="Missing required configuration"):
            CSIExtractor(config=invalid_config, logger=mock_logger)

    def test_should_create_appropriate_parser(self, mock_config, mock_logger):
        """Should create appropriate parser based on hardware type."""
        extractor = CSIExtractor(config=mock_config, logger=mock_logger)
        
        assert isinstance(extractor.parser, ESP32CSIParser)

    @pytest.mark.asyncio
    async def test_should_establish_connection_successfully(self, csi_extractor):
        """Should establish connection to hardware successfully."""
        with patch.object(csi_extractor, '_establish_hardware_connection', new_callable=AsyncMock) as mock_connect:
            mock_connect.return_value = True
            
            result = await csi_extractor.connect()
            
            assert result == True
            assert csi_extractor.is_connected == True
            mock_connect.assert_called_once()

    @pytest.mark.asyncio
    async def test_should_handle_connection_failure(self, csi_extractor):
        """Should handle connection failure gracefully."""
        with patch.object(csi_extractor, '_establish_hardware_connection', new_callable=AsyncMock) as mock_connect:
            mock_connect.side_effect = ConnectionError("Hardware not found")
            
            result = await csi_extractor.connect()
            
            assert result == False
            assert csi_extractor.is_connected == False
            csi_extractor.logger.error.assert_called()

    @pytest.mark.asyncio
    async def test_should_disconnect_properly(self, csi_extractor):
        """Should disconnect from hardware properly."""
        csi_extractor.is_connected = True
        
        with patch.object(csi_extractor, '_close_hardware_connection', new_callable=AsyncMock) as mock_disconnect:
            await csi_extractor.disconnect()
            
            assert csi_extractor.is_connected == False
            mock_disconnect.assert_called_once()

    @pytest.mark.asyncio
    async def test_should_extract_csi_data_successfully(self, csi_extractor, sample_csi_data):
        """Should extract CSI data successfully from hardware."""
        csi_extractor.is_connected = True
        
        with patch.object(csi_extractor, '_read_raw_data', new_callable=AsyncMock) as mock_read:
            with patch.object(csi_extractor.parser, 'parse', return_value=sample_csi_data) as mock_parse:
                mock_read.return_value = b"raw_csi_data"
                
                result = await csi_extractor.extract_csi()
                
                assert result == sample_csi_data
                mock_read.assert_called_once()
                mock_parse.assert_called_once_with(b"raw_csi_data")

    @pytest.mark.asyncio
    async def test_should_handle_extraction_failure_when_not_connected(self, csi_extractor):
        """Should handle extraction failure when not connected."""
        csi_extractor.is_connected = False
        
        with pytest.raises(CSIParseError, match="Not connected to hardware"):
            await csi_extractor.extract_csi()

    @pytest.mark.asyncio
    async def test_should_retry_on_temporary_failure(self, csi_extractor, sample_csi_data):
        """Should retry extraction on temporary failure."""
        csi_extractor.is_connected = True
        
        with patch.object(csi_extractor, '_read_raw_data', new_callable=AsyncMock) as mock_read:
            with patch.object(csi_extractor.parser, 'parse') as mock_parse:
                # First two calls fail, third succeeds
                mock_read.side_effect = [ConnectionError(), ConnectionError(), b"raw_data"]
                mock_parse.return_value = sample_csi_data
                
                result = await csi_extractor.extract_csi()
                
                assert result == sample_csi_data
                assert mock_read.call_count == 3

    def test_should_validate_csi_data_successfully(self, csi_extractor, sample_csi_data):
        """Should validate CSI data successfully."""
        result = csi_extractor.validate_csi_data(sample_csi_data)
        
        assert result == True

    def test_should_reject_invalid_csi_data(self, csi_extractor):
        """Should reject CSI data with invalid structure."""
        invalid_data = CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.array([]),  # Empty array
            phase=np.array([]),
            frequency=0,  # Invalid frequency
            bandwidth=0,
            num_subcarriers=0,
            num_antennas=0,
            snr=-100,  # Invalid SNR
            metadata={}
        )
        
        with pytest.raises(CSIValidationError):
            csi_extractor.validate_csi_data(invalid_data)

    @pytest.mark.asyncio
    async def test_should_start_streaming_successfully(self, csi_extractor, sample_csi_data):
        """Should start CSI data streaming successfully."""
        csi_extractor.is_connected = True
        callback = Mock()
        
        with patch.object(csi_extractor, 'extract_csi', new_callable=AsyncMock) as mock_extract:
            mock_extract.return_value = sample_csi_data
            
            # Start streaming with limited iterations to avoid infinite loop
            streaming_task = asyncio.create_task(csi_extractor.start_streaming(callback))
            await asyncio.sleep(0.1)  # Let it run briefly
            csi_extractor.stop_streaming()
            await streaming_task
            
            callback.assert_called()

    @pytest.mark.asyncio
    async def test_should_stop_streaming_gracefully(self, csi_extractor):
        """Should stop streaming gracefully."""
        csi_extractor.is_streaming = True
        
        csi_extractor.stop_streaming()
        
        assert csi_extractor.is_streaming == False


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestESP32CSIParser:
    """Test ESP32 CSI parser using London School TDD."""

    @pytest.fixture
    def parser(self):
        """Create ESP32 CSI parser for testing."""
        return ESP32CSIParser()

    @pytest.fixture
    def raw_esp32_data(self):
        """Sample raw ESP32 CSI data."""
        return b"CSI_DATA:1234567890,3,56,2400,20,15.5,[1.0,2.0,3.0],[0.5,1.5,2.5]"

    def test_should_parse_valid_esp32_data(self, parser, raw_esp32_data):
        """Should parse valid ESP32 CSI data successfully."""
        result = parser.parse(raw_esp32_data)
        
        assert isinstance(result, CSIData)
        assert result.num_antennas == 3
        assert result.num_subcarriers == 56
        assert result.frequency == 2400000000  # 2.4 GHz
        assert result.bandwidth == 20000000    # 20 MHz
        assert result.snr == 15.5

    def test_should_handle_malformed_data(self, parser):
        """Should handle malformed ESP32 data gracefully."""
        malformed_data = b"INVALID_DATA"
        
        with pytest.raises(CSIParseError, match="Invalid ESP32 CSI data format"):
            parser.parse(malformed_data)

    def test_should_handle_empty_data(self, parser):
        """Should handle empty data gracefully."""
        with pytest.raises(CSIParseError, match="Empty data received"):
            parser.parse(b"")


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestRouterCSIParser:
    """Test Router CSI parser using London School TDD."""

    @pytest.fixture
    def parser(self):
        """Create Router CSI parser for testing."""
        return RouterCSIParser()

    def test_should_parse_atheros_format(self, parser):
        """Should parse Atheros CSI format successfully."""
        raw_data = b"ATHEROS_CSI:mock_data"
        
        with patch.object(parser, '_parse_atheros_format', return_value=Mock(spec=CSIData)) as mock_parse:
            result = parser.parse(raw_data)
            
            mock_parse.assert_called_once()
            assert result is not None

    def test_should_handle_unknown_format(self, parser):
        """Should handle unknown router format gracefully."""
        unknown_data = b"UNKNOWN_FORMAT:data"
        
        with pytest.raises(CSIParseError, match="Unknown router CSI format"):
            parser.parse(unknown_data)