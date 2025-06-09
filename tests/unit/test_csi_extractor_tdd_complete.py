"""Complete TDD tests for CSI extractor with 100% coverage."""

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
class TestCSIExtractorComplete:
    """Complete CSI extractor tests for 100% coverage."""

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
class TestESP32CSIParserComplete:
    """Complete ESP32 CSI parser tests for 100% coverage."""

    @pytest.fixture
    def parser(self):
        """Create ESP32 CSI parser for testing."""
        return ESP32CSIParser()

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
class TestRouterCSIParserComplete:
    """Complete Router CSI parser tests for 100% coverage."""

    @pytest.fixture
    def parser(self):
        """Create Router CSI parser for testing."""
        return RouterCSIParser()

    def test_parse_atheros_format_directly(self, parser):
        """Should parse Atheros format directly."""
        raw_data = b"ATHEROS_CSI:mock_data"
        
        result = parser.parse(raw_data)
        
        assert isinstance(result, CSIData)
        assert result.metadata['source'] == 'atheros_router'