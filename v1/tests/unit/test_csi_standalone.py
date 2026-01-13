"""Standalone tests for CSI extractor module."""

import pytest
import numpy as np
import sys
import os
from unittest.mock import Mock, patch, AsyncMock
import asyncio
from datetime import datetime, timezone
import importlib.util

# Import the module directly to avoid circular imports
spec = importlib.util.spec_from_file_location(
    'csi_extractor', 
    '/workspaces/wifi-densepose/src/hardware/csi_extractor.py'
)
csi_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(csi_module)

# Get classes from the module
CSIExtractor = csi_module.CSIExtractor
CSIParseError = csi_module.CSIParseError
CSIData = csi_module.CSIData
ESP32CSIParser = csi_module.ESP32CSIParser
RouterCSIParser = csi_module.RouterCSIParser
CSIValidationError = csi_module.CSIValidationError


@pytest.mark.unit
@pytest.mark.tdd
@pytest.mark.london
class TestCSIExtractorStandalone:
    """Standalone tests for CSI extractor with 100% coverage."""

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

    # Test all initialization paths
    def test_init_esp32_config(self, esp32_config, mock_logger):
        """Should initialize with ESP32 configuration."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        assert extractor.config == esp32_config
        assert extractor.logger == mock_logger
        assert extractor.is_connected == False
        assert extractor.hardware_type == 'esp32'
        assert isinstance(extractor.parser, ESP32CSIParser)

    def test_init_router_config(self, router_config, mock_logger):
        """Should initialize with router configuration."""
        extractor = CSIExtractor(config=router_config, logger=mock_logger)
        
        assert isinstance(extractor.parser, RouterCSIParser)
        assert extractor.hardware_type == 'router'

    def test_init_unsupported_hardware(self, mock_logger):
        """Should raise error for unsupported hardware type."""
        invalid_config = {
            'hardware_type': 'unsupported',
            'sampling_rate': 100,
            'buffer_size': 1024,
            'timeout': 5.0
        }
        
        with pytest.raises(ValueError, match="Unsupported hardware type: unsupported"):
            CSIExtractor(config=invalid_config, logger=mock_logger)

    def test_init_without_logger(self, esp32_config):
        """Should initialize without logger."""
        extractor = CSIExtractor(config=esp32_config)
        
        assert extractor.logger is not None  # Should create default logger

    # Test all validation paths
    def test_validation_missing_fields(self, mock_logger):
        """Should validate missing required fields."""
        for missing_field in ['hardware_type', 'sampling_rate', 'buffer_size', 'timeout']:
            config = {
                'hardware_type': 'esp32',
                'sampling_rate': 100,
                'buffer_size': 1024,
                'timeout': 5.0
            }
            del config[missing_field]
            
            with pytest.raises(ValueError, match="Missing required configuration"):
                CSIExtractor(config=config, logger=mock_logger)

    def test_validation_negative_sampling_rate(self, mock_logger):
        """Should validate sampling_rate is positive."""
        config = {
            'hardware_type': 'esp32',
            'sampling_rate': -1,
            'buffer_size': 1024,
            'timeout': 5.0
        }
        
        with pytest.raises(ValueError, match="sampling_rate must be positive"):
            CSIExtractor(config=config, logger=mock_logger)

    def test_validation_zero_buffer_size(self, mock_logger):
        """Should validate buffer_size is positive."""
        config = {
            'hardware_type': 'esp32',
            'sampling_rate': 100,
            'buffer_size': 0,
            'timeout': 5.0
        }
        
        with pytest.raises(ValueError, match="buffer_size must be positive"):
            CSIExtractor(config=config, logger=mock_logger)

    def test_validation_negative_timeout(self, mock_logger):
        """Should validate timeout is positive."""
        config = {
            'hardware_type': 'esp32',
            'sampling_rate': 100,
            'buffer_size': 1024,
            'timeout': -1.0
        }
        
        with pytest.raises(ValueError, match="timeout must be positive"):
            CSIExtractor(config=config, logger=mock_logger)

    # Test connection management
    @pytest.mark.asyncio
    async def test_connect_success(self, esp32_config, mock_logger):
        """Should connect successfully."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        with patch.object(extractor, '_establish_hardware_connection', new_callable=AsyncMock) as mock_conn:
            mock_conn.return_value = True
            
            result = await extractor.connect()
            
            assert result == True
            assert extractor.is_connected == True

    @pytest.mark.asyncio
    async def test_connect_failure(self, esp32_config, mock_logger):
        """Should handle connection failure."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        with patch.object(extractor, '_establish_hardware_connection', new_callable=AsyncMock) as mock_conn:
            mock_conn.side_effect = ConnectionError("Failed")
            
            result = await extractor.connect()
            
            assert result == False
            assert extractor.is_connected == False

    @pytest.mark.asyncio
    async def test_disconnect_when_connected(self, esp32_config, mock_logger):
        """Should disconnect when connected."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        
        with patch.object(extractor, '_close_hardware_connection', new_callable=AsyncMock) as mock_close:
            await extractor.disconnect()
            
            assert extractor.is_connected == False
            mock_close.assert_called_once()

    @pytest.mark.asyncio
    async def test_disconnect_when_not_connected(self, esp32_config, mock_logger):
        """Should not disconnect when not connected."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = False
        
        with patch.object(extractor, '_close_hardware_connection', new_callable=AsyncMock) as mock_close:
            await extractor.disconnect()
            
            mock_close.assert_not_called()

    # Test extraction
    @pytest.mark.asyncio
    async def test_extract_not_connected(self, esp32_config, mock_logger):
        """Should raise error when not connected."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = False
        
        with pytest.raises(CSIParseError, match="Not connected to hardware"):
            await extractor.extract_csi()

    @pytest.mark.asyncio
    async def test_extract_success_with_validation(self, esp32_config, mock_logger, sample_csi_data):
        """Should extract successfully with validation."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        
        with patch.object(extractor, '_read_raw_data', new_callable=AsyncMock) as mock_read:
            with patch.object(extractor.parser, 'parse', return_value=sample_csi_data):
                with patch.object(extractor, 'validate_csi_data', return_value=True) as mock_validate:
                    mock_read.return_value = b"raw_data"
                    
                    result = await extractor.extract_csi()
                    
                    assert result == sample_csi_data
                    mock_validate.assert_called_once()

    @pytest.mark.asyncio
    async def test_extract_success_without_validation(self, esp32_config, mock_logger, sample_csi_data):
        """Should extract successfully without validation."""
        esp32_config['validation_enabled'] = False
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        
        with patch.object(extractor, '_read_raw_data', new_callable=AsyncMock) as mock_read:
            with patch.object(extractor.parser, 'parse', return_value=sample_csi_data):
                with patch.object(extractor, 'validate_csi_data') as mock_validate:
                    mock_read.return_value = b"raw_data"
                    
                    result = await extractor.extract_csi()
                    
                    assert result == sample_csi_data
                    mock_validate.assert_not_called()

    @pytest.mark.asyncio
    async def test_extract_retry_success(self, esp32_config, mock_logger, sample_csi_data):
        """Should retry and succeed."""
        esp32_config['retry_attempts'] = 3
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        
        with patch.object(extractor, '_read_raw_data', new_callable=AsyncMock) as mock_read:
            with patch.object(extractor.parser, 'parse', return_value=sample_csi_data):
                # Fail first two attempts, succeed on third
                mock_read.side_effect = [ConnectionError(), ConnectionError(), b"raw_data"]
                
                result = await extractor.extract_csi()
                
                assert result == sample_csi_data
                assert mock_read.call_count == 3

    @pytest.mark.asyncio
    async def test_extract_retry_failure(self, esp32_config, mock_logger):
        """Should fail after max retries."""
        esp32_config['retry_attempts'] = 2
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        
        with patch.object(extractor, '_read_raw_data', new_callable=AsyncMock) as mock_read:
            mock_read.side_effect = ConnectionError("Failed")
            
            with pytest.raises(CSIParseError, match="Extraction failed after 2 attempts"):
                await extractor.extract_csi()

    # Test validation
    def test_validate_success(self, esp32_config, mock_logger, sample_csi_data):
        """Should validate successfully."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        result = extractor.validate_csi_data(sample_csi_data)
        
        assert result == True

    def test_validate_empty_amplitude(self, esp32_config, mock_logger):
        """Should reject empty amplitude."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        data = CSIData(
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
            extractor.validate_csi_data(data)

    def test_validate_empty_phase(self, esp32_config, mock_logger):
        """Should reject empty phase."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        data = CSIData(
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
            extractor.validate_csi_data(data)

    def test_validate_invalid_frequency(self, esp32_config, mock_logger):
        """Should reject invalid frequency."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        data = CSIData(
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
            extractor.validate_csi_data(data)

    def test_validate_invalid_bandwidth(self, esp32_config, mock_logger):
        """Should reject invalid bandwidth."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        data = CSIData(
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
            extractor.validate_csi_data(data)

    def test_validate_invalid_subcarriers(self, esp32_config, mock_logger):
        """Should reject invalid subcarriers."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        data = CSIData(
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
            extractor.validate_csi_data(data)

    def test_validate_invalid_antennas(self, esp32_config, mock_logger):
        """Should reject invalid antennas."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        data = CSIData(
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
            extractor.validate_csi_data(data)

    def test_validate_snr_too_low(self, esp32_config, mock_logger):
        """Should reject SNR too low."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        data = CSIData(
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
            extractor.validate_csi_data(data)

    def test_validate_snr_too_high(self, esp32_config, mock_logger):
        """Should reject SNR too high."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        data = CSIData(
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
            extractor.validate_csi_data(data)

    # Test streaming
    @pytest.mark.asyncio
    async def test_streaming_success(self, esp32_config, mock_logger, sample_csi_data):
        """Should stream successfully."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        callback = Mock()
        
        with patch.object(extractor, 'extract_csi', new_callable=AsyncMock) as mock_extract:
            mock_extract.return_value = sample_csi_data
            
            # Start streaming task
            task = asyncio.create_task(extractor.start_streaming(callback))
            await asyncio.sleep(0.1)  # Let it run briefly
            extractor.stop_streaming()
            await task
            
            callback.assert_called()

    @pytest.mark.asyncio
    async def test_streaming_exception(self, esp32_config, mock_logger):
        """Should handle streaming exceptions."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_connected = True
        callback = Mock()
        
        with patch.object(extractor, 'extract_csi', new_callable=AsyncMock) as mock_extract:
            mock_extract.side_effect = Exception("Test error")
            
            # Start streaming and let it handle exception
            task = asyncio.create_task(extractor.start_streaming(callback))
            await task  # This should complete due to exception
            
            assert extractor.is_streaming == False

    def test_stop_streaming(self, esp32_config, mock_logger):
        """Should stop streaming."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        extractor.is_streaming = True
        
        extractor.stop_streaming()
        
        assert extractor.is_streaming == False

    # Test placeholder implementations for 100% coverage
    @pytest.mark.asyncio
    async def test_establish_hardware_connection_placeholder(self, esp32_config, mock_logger):
        """Should test placeholder hardware connection."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        result = await extractor._establish_hardware_connection()
        
        assert result == True

    @pytest.mark.asyncio
    async def test_close_hardware_connection_placeholder(self, esp32_config, mock_logger):
        """Should test placeholder hardware disconnection."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        # Should not raise any exception
        await extractor._close_hardware_connection()

    @pytest.mark.asyncio
    async def test_read_raw_data_placeholder(self, esp32_config, mock_logger):
        """Should test placeholder raw data reading."""
        extractor = CSIExtractor(config=esp32_config, logger=mock_logger)
        
        result = await extractor._read_raw_data()
        
        assert result == b"CSI_DATA:1234567890,3,56,2400,20,15.5,[1.0,2.0,3.0],[0.5,1.5,2.5]"


@pytest.mark.unit
@pytest.mark.tdd
class TestESP32CSIParserStandalone:
    """Standalone tests for ESP32 CSI parser."""

    @pytest.fixture
    def parser(self):
        """Create parser instance."""
        return ESP32CSIParser()

    def test_parse_valid_data(self, parser):
        """Should parse valid ESP32 data."""
        data = b"CSI_DATA:1234567890,3,56,2400,20,15.5,[1.0,2.0,3.0],[0.5,1.5,2.5]"
        
        result = parser.parse(data)
        
        assert isinstance(result, CSIData)
        assert result.num_antennas == 3
        assert result.num_subcarriers == 56
        assert result.frequency == 2400000000
        assert result.bandwidth == 20000000
        assert result.snr == 15.5

    def test_parse_empty_data(self, parser):
        """Should reject empty data."""
        with pytest.raises(CSIParseError, match="Empty data received"):
            parser.parse(b"")

    def test_parse_invalid_format(self, parser):
        """Should reject invalid format."""
        with pytest.raises(CSIParseError, match="Invalid ESP32 CSI data format"):
            parser.parse(b"INVALID_DATA")

    def test_parse_value_error(self, parser):
        """Should handle ValueError."""
        data = b"CSI_DATA:invalid_number,3,56,2400,20,15.5"
        
        with pytest.raises(CSIParseError, match="Failed to parse ESP32 data"):
            parser.parse(data)

    def test_parse_index_error(self, parser):
        """Should handle IndexError."""
        data = b"CSI_DATA:1234567890"  # Missing fields
        
        with pytest.raises(CSIParseError, match="Failed to parse ESP32 data"):
            parser.parse(data)


@pytest.mark.unit
@pytest.mark.tdd
class TestRouterCSIParserStandalone:
    """Standalone tests for Router CSI parser."""

    @pytest.fixture
    def parser(self):
        """Create parser instance."""
        return RouterCSIParser()

    def test_parse_empty_data(self, parser):
        """Should reject empty data."""
        with pytest.raises(CSIParseError, match="Empty data received"):
            parser.parse(b"")

    def test_parse_atheros_format(self, parser):
        """Should parse Atheros format."""
        data = b"ATHEROS_CSI:mock_data"
        
        result = parser.parse(data)
        
        assert isinstance(result, CSIData)
        assert result.metadata['source'] == 'atheros_router'

    def test_parse_unknown_format(self, parser):
        """Should reject unknown format."""
        data = b"UNKNOWN_FORMAT:data"
        
        with pytest.raises(CSIParseError, match="Unknown router CSI format"):
            parser.parse(data)