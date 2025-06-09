"""Direct tests for CSI extractor avoiding import issues."""

import pytest
import numpy as np
import sys
import os
from unittest.mock import Mock, patch, AsyncMock, MagicMock
from typing import Dict, Any, Optional
import asyncio
from datetime import datetime, timezone

# Add src to path for direct import
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../'))

# Import the CSI extractor module directly
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
class TestCSIExtractorDirect:
    """Test CSI extractor with direct imports."""

    @pytest.fixture
    def mock_logger(self):
        """Mock logger for testing."""
        return Mock()

    @pytest.fixture
    def esp32_config(self):
        """ESP32 configuration for testing."""
        return {
            'hardware_type': 'esp32',
            'sampling_rate': 100,
            'buffer_size': 1024,
            'timeout': 5.0,
            'validation_enabled': True,
            'retry_attempts': 3
        }

    @pytest.fixture
    def router_config(self):
        """Router configuration for testing."""
        return {
            'hardware_type': 'router',
            'sampling_rate': 50,
            'buffer_size': 512,
            'timeout': 10.0,
            'validation_enabled': False,
            'retry_attempts': 1
        }

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

    # Initialization tests
    def test_should_initialize_with_valid_config(self, esp32_config, mock_logger):
        """Should initialize CSI extractor with valid configuration."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        assert extractor.config == esp32_config
        assert extractor.logger == mock_logger
        assert extractor.is_connected == False
        assert extractor.hardware_type == 'esp32'

    def test_should_create_esp32_parser(self, esp32_config, mock_logger):
        """Should create ESP32 parser when hardware_type is esp32."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        assert isinstance(extractor.parser, ESP32CSIParser)

    def test_should_create_router_parser(self, router_config, mock_logger):
        """Should create router parser when hardware_type is router."""
        extractor = CSIExtractor(config=router_config, logger=mock_logger)
        
        assert isinstance(extractor.parser, RouterCSIParser)
        assert extractor.hardware_type == 'router'

    def test_should_raise_error_for_unsupported_hardware(self, mock_logger):
        """Should raise error for unsupported hardware type."""
        invalid_config = {
            'hardware_type': 'unsupported',
            'sampling_rate': 100,
            'buffer_size': 1024,
            'timeout': 5.0
        }
        
        with pytest.raises(ValueError, match="Unsupported hardware type: unsupported"):
            CSIExtractor(config=invalid_config, logger=mock_logger)

    # Configuration validation tests
    def test_config_validation_missing_fields(self, mock_logger):
        """Should validate required configuration fields."""
        invalid_config = {'invalid': 'config'}
        
        with pytest.raises(ValueError, match="Missing required configuration"):
            CSIExtractor(config=invalid_config, logger=mock_logger)

    def test_config_validation_negative_sampling_rate(self, mock_logger):
        """Should validate sampling_rate is positive."""
        invalid_config = {
            'hardware_type': 'esp32',
            'sampling_rate': -1,
            'buffer_size': 1024,
            'timeout': 5.0
        }
        
        with pytest.raises(ValueError, match="sampling_rate must be positive"):
            CSIExtractor(config=invalid_config, logger=mock_logger)

    def test_config_validation_zero_buffer_size(self, mock_logger):
        """Should validate buffer_size is positive."""
        invalid_config = {
            'hardware_type': 'esp32',
            'sampling_rate': 100,
            'buffer_size': 0,
            'timeout': 5.0
        }
        
        with pytest.raises(ValueError, match="buffer_size must be positive"):
            CSIExtractor(config=invalid_config, logger=mock_logger)

    def test_config_validation_negative_timeout(self, mock_logger):
        """Should validate timeout is positive."""
        invalid_config = {
            'hardware_type': 'esp32',
            'sampling_rate': 100,
            'buffer_size': 1024,
            'timeout': -1.0
        }
        
        with pytest.raises(ValueError, match="timeout must be positive"):
            CSIExtractor(config=invalid_config, logger=mock_logger)

    # Connection tests
    @pytest.mark.asyncio
    async def test_should_establish_connection_successfully(self, esp32_config, mock_logger):
        """Should establish connection to hardware successfully."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        with patch.object(extractor, '_establish_hardware_connection', new_callable=AsyncMock) as mock_connect:
            mock_connect.return_value = True
            
            result = await extractor.connect()
            
            assert result == True
            assert extractor.is_connected == True
            mock_connect.assert_called_once()

    @pytest.mark.asyncio
    async def test_should_handle_connection_failure(self, esp32_config, mock_logger):
        """Should handle connection failure gracefully."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        with patch.object(extractor, '_establish_hardware_connection', new_callable=AsyncMock) as mock_connect:
            mock_connect.side_effect = ConnectionError("Hardware not found")
            
            result = await extractor.connect()
            
            assert result == False
            assert extractor.is_connected == False
            extractor.logger.error.assert_called()

    @pytest.mark.asyncio
    async def test_should_disconnect_properly(self, esp32_config, mock_logger):
        """Should disconnect from hardware properly."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        
        with patch.object(extractor, '_close_hardware_connection', new_callable=AsyncMock) as mock_disconnect:
            await extractor.disconnect()
            
            assert extractor.is_connected == False
            mock_disconnect.assert_called_once()

    @pytest.mark.asyncio
    async def test_disconnect_when_not_connected(self, esp32_config, mock_logger):
        """Should handle disconnect when not connected."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = False
        
        with patch.object(extractor, '_close_hardware_connection', new_callable=AsyncMock) as mock_close:
            await extractor.disconnect()
            
            # Should not call close when not connected
            mock_close.assert_not_called()
            assert extractor.is_connected == False

    # Data extraction tests
    @pytest.mark.asyncio
    async def test_should_extract_csi_data_successfully(self, esp32_config, mock_logger, sample_csi_data):
        """Should extract CSI data successfully from hardware."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        
        with patch.object(extractor, '_read_raw_data', new_callable=AsyncMock) as mock_read:
            with patch.object(extractor.parser, 'parse', return_value=sample_csi_data) as mock_parse:
                mock_read.return_value = b"raw_csi_data"
                
                result = await extractor.extract_csi()
                
                assert result == sample_csi_data
                mock_read.assert_called_once()
                mock_parse.assert_called_once_with(b"raw_csi_data")

    @pytest.mark.asyncio
    async def test_should_handle_extraction_failure_when_not_connected(self, esp32_config, mock_logger):
        """Should handle extraction failure when not connected."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = False
        
        with pytest.raises(CSIParseError, match="Not connected to hardware"):
            await extractor.extract_csi()

    @pytest.mark.asyncio
    async def test_should_retry_on_temporary_failure(self, esp32_config, mock_logger, sample_csi_data):
        """Should retry extraction on temporary failure."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        
        with patch.object(extractor, '_read_raw_data', new_callable=AsyncMock) as mock_read:
            with patch.object(extractor.parser, 'parse') as mock_parse:
                # First two calls fail, third succeeds
                mock_read.side_effect = [ConnectionError(), ConnectionError(), b"raw_data"]
                mock_parse.return_value = sample_csi_data
                
                result = await extractor.extract_csi()
                
                assert result == sample_csi_data
                assert mock_read.call_count == 3

    @pytest.mark.asyncio
    async def test_extract_with_validation_disabled(self, esp32_config, mock_logger, sample_csi_data):
        """Should skip validation when disabled."""
        esp32_config['validation_enabled'] = False
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        
        with patch.object(extractor, '_read_raw_data', new_callable=AsyncMock) as mock_read:
            with patch.object(extractor.parser, 'parse', return_value=sample_csi_data) as mock_parse:
                with patch.object(extractor, 'validate_csi_data') as mock_validate:
                    mock_read.return_value = b"raw_data"
                    
                    result = await extractor.extract_csi()
                    
                    assert result == sample_csi_data
                    mock_validate.assert_not_called()

    @pytest.mark.asyncio
    async def test_extract_max_retries_exceeded(self, esp32_config, mock_logger):
        """Should raise error after max retries exceeded."""
        esp32_config['retry_attempts'] = 2
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        
        with patch.object(extractor, '_read_raw_data', new_callable=AsyncMock) as mock_read:
            mock_read.side_effect = ConnectionError("Connection failed")
            
            with pytest.raises(CSIParseError, match="Extraction failed after 2 attempts"):
                await extractor.extract_csi()
            
            assert mock_read.call_count == 2

    # Validation tests
    def test_should_validate_csi_data_successfully(self, esp32_config, mock_logger, sample_csi_data):
        """Should validate CSI data successfully."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        result = extractor.validate_csi_data(sample_csi_data)
        
        assert result == True

    def test_validation_empty_amplitude(self, esp32_config, mock_logger):
        """Should raise validation error for empty amplitude."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        invalid_data = CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.array([]),
            phase=np.random.rand(3, 56),
            frequency=2.4e9,
            bandwidth=20e6,
            num_subcarriers=56,
            num_antennas=3,
            snr=15.5,
            metadata={}
        )
        
        with pytest.raises(CSIValidationError, match="Empty amplitude data"):
            extractor.validate_csi_data(invalid_data)

    def test_validation_empty_phase(self, esp32_config, mock_logger):
        """Should raise validation error for empty phase."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        invalid_data = CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.random.rand(3, 56),
            phase=np.array([]),
            frequency=2.4e9,
            bandwidth=20e6,
            num_subcarriers=56,
            num_antennas=3,
            snr=15.5,
            metadata={}
        )
        
        with pytest.raises(CSIValidationError, match="Empty phase data"):
            extractor.validate_csi_data(invalid_data)

    def test_validation_invalid_frequency(self, esp32_config, mock_logger):
        """Should raise validation error for invalid frequency."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        invalid_data = CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.random.rand(3, 56),
            phase=np.random.rand(3, 56),
            frequency=0,
            bandwidth=20e6,
            num_subcarriers=56,
            num_antennas=3,
            snr=15.5,
            metadata={}
        )
        
        with pytest.raises(CSIValidationError, match="Invalid frequency"):
            extractor.validate_csi_data(invalid_data)

    def test_validation_invalid_bandwidth(self, esp32_config, mock_logger):
        """Should raise validation error for invalid bandwidth."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        invalid_data = CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.random.rand(3, 56),
            phase=np.random.rand(3, 56),
            frequency=2.4e9,
            bandwidth=0,
            num_subcarriers=56,
            num_antennas=3,
            snr=15.5,
            metadata={}
        )
        
        with pytest.raises(CSIValidationError, match="Invalid bandwidth"):
            extractor.validate_csi_data(invalid_data)

    def test_validation_invalid_subcarriers(self, esp32_config, mock_logger):
        """Should raise validation error for invalid subcarriers."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        invalid_data = CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.random.rand(3, 56),
            phase=np.random.rand(3, 56),
            frequency=2.4e9,
            bandwidth=20e6,
            num_subcarriers=0,
            num_antennas=3,
            snr=15.5,
            metadata={}
        )
        
        with pytest.raises(CSIValidationError, match="Invalid number of subcarriers"):
            extractor.validate_csi_data(invalid_data)

    def test_validation_invalid_antennas(self, esp32_config, mock_logger):
        """Should raise validation error for invalid antennas."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        invalid_data = CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.random.rand(3, 56),
            phase=np.random.rand(3, 56),
            frequency=2.4e9,
            bandwidth=20e6,
            num_subcarriers=56,
            num_antennas=0,
            snr=15.5,
            metadata={}
        )
        
        with pytest.raises(CSIValidationError, match="Invalid number of antennas"):
            extractor.validate_csi_data(invalid_data)

    def test_validation_snr_too_low(self, esp32_config, mock_logger):
        """Should raise validation error for SNR too low."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        invalid_data = CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.random.rand(3, 56),
            phase=np.random.rand(3, 56),
            frequency=2.4e9,
            bandwidth=20e6,
            num_subcarriers=56,
            num_antennas=3,
            snr=-100,
            metadata={}
        )
        
        with pytest.raises(CSIValidationError, match="Invalid SNR value"):
            extractor.validate_csi_data(invalid_data)

    def test_validation_snr_too_high(self, esp32_config, mock_logger):
        """Should raise validation error for SNR too high."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        invalid_data = CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.random.rand(3, 56),
            phase=np.random.rand(3, 56),
            frequency=2.4e9,
            bandwidth=20e6,
            num_subcarriers=56,
            num_antennas=3,
            snr=100,
            metadata={}
        )
        
        with pytest.raises(CSIValidationError, match="Invalid SNR value"):
            extractor.validate_csi_data(invalid_data)

    # Streaming tests
    @pytest.mark.asyncio
    async def test_should_start_streaming_successfully(self, esp32_config, mock_logger, sample_csi_data):
        """Should start CSI data streaming successfully."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        callback = Mock()
        
        with patch.object(extractor, 'extract_csi', new_callable=AsyncMock) as mock_extract:
            mock_extract.return_value = sample_csi_data
            
            # Start streaming with limited iterations to avoid infinite loop
            streaming_task = asyncio.create_task(extractor.start_streaming(callback))
            await asyncio.sleep(0.1)  # Let it run briefly
            extractor.stop_streaming()
            await streaming_task
            
            callback.assert_called()

    @pytest.mark.asyncio
    async def test_should_stop_streaming_gracefully(self, esp32_config, mock_logger):
        """Should stop streaming gracefully."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_streaming = True
        
        extractor.stop_streaming()
        
        assert extractor.is_streaming == False

    @pytest.mark.asyncio
    async def test_streaming_with_exception(self, esp32_config, mock_logger):
        """Should handle exceptions during streaming."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        callback = Mock()
        
        with patch.object(extractor, 'extract_csi', new_callable=AsyncMock) as mock_extract:
            mock_extract.side_effect = Exception("Extraction error")
            
            # Start streaming and let it handle the exception
            streaming_task = asyncio.create_task(extractor.start_streaming(callback))
            await asyncio.sleep(0.1)  # Let it run briefly and hit the exception
            await streaming_task
            
            # Should log error and stop streaming
            assert extractor.is_streaming == False
            extractor.logger.error.assert_called()


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestESP32CSIParserDirect:
    """Test ESP32 CSI parser with direct imports."""

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

    def test_parse_with_value_error(self, parser):
        """Should handle ValueError during parsing."""
        invalid_data = b"CSI_DATA:invalid_timestamp,3,56,2400,20,15.5"
        
        with pytest.raises(CSIParseError, match="Failed to parse ESP32 data"):
            parser.parse(invalid_data)

    def test_parse_with_index_error(self, parser):
        """Should handle IndexError during parsing."""
        invalid_data = b"CSI_DATA:1234567890"  # Missing fields
        
        with pytest.raises(CSIParseError, match="Failed to parse ESP32 data"):
            parser.parse(invalid_data)


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestRouterCSIParserDirect:
    """Test Router CSI parser with direct imports."""

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

    def test_parse_atheros_format_directly(self, parser):
        """Should parse Atheros format directly."""
        raw_data = b"ATHEROS_CSI:mock_data"
        
        result = parser.parse(raw_data)
        
        assert isinstance(result, CSIData)
        assert result.metadata['source'] == 'atheros_router'

    def test_should_handle_empty_data_router(self, parser):
        """Should handle empty data gracefully."""
        with pytest.raises(CSIParseError, match="Empty data received"):
            parser.parse(b"")