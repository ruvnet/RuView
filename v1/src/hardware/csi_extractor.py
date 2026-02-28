"""CSI data extraction from WiFi hardware using Test-Driven Development approach."""

import asyncio
import numpy as np
from datetime import datetime, timezone
from typing import Dict, Any, Optional, Callable, Protocol
from dataclasses import dataclass
import logging


class CSIParseError(Exception):
    """Exception raised for CSI parsing errors."""
    pass


class CSIValidationError(Exception):
    """Exception raised for CSI validation errors."""
    pass


class CSIExtractionError(Exception):
    """Exception raised when CSI data extraction fails.

    This error is raised instead of silently returning random/placeholder data.
    Callers should handle this to inform users that real hardware data is required.
    """
    pass


@dataclass
class CSIData:
    """Data structure for CSI measurements."""
    timestamp: datetime
    amplitude: np.ndarray
    phase: np.ndarray
    frequency: float
    bandwidth: float
    num_subcarriers: int
    num_antennas: int
    snr: float
    metadata: Dict[str, Any]


class CSIParser(Protocol):
    """Protocol for CSI data parsers."""
    
    def parse(self, raw_data: bytes) -> CSIData:
        """Parse raw CSI data into structured format."""
        ...


class ESP32CSIParser:
    """Parser for ESP32 CSI data format."""
    
    def parse(self, raw_data: bytes) -> CSIData:
        """Parse ESP32 CSI data format.
        
        Args:
            raw_data: Raw bytes from ESP32
            
        Returns:
            Parsed CSI data
            
        Raises:
            CSIParseError: If data format is invalid
        """
        if not raw_data:
            raise CSIParseError("Empty data received")
        
        try:
            data_str = raw_data.decode('utf-8')
            if not data_str.startswith('CSI_DATA:'):
                raise CSIParseError("Invalid ESP32 CSI data format")
            
            # Parse ESP32 format: CSI_DATA:timestamp,antennas,subcarriers,freq,bw,snr,[amp],[phase]
            parts = data_str[9:].split(',')  # Remove 'CSI_DATA:' prefix
            
            timestamp_ms = int(parts[0])
            num_antennas = int(parts[1])
            num_subcarriers = int(parts[2])
            frequency_mhz = float(parts[3])
            bandwidth_mhz = float(parts[4])
            snr = float(parts[5])
            
            # Convert to proper units
            frequency = frequency_mhz * 1e6  # MHz to Hz
            bandwidth = bandwidth_mhz * 1e6  # MHz to Hz
            
            # Parse amplitude and phase arrays from the remaining CSV fields.
            # Expected format after the header fields: comma-separated float values
            # representing interleaved amplitude and phase per antenna per subcarrier.
            data_values = parts[6:]
            expected_values = num_antennas * num_subcarriers * 2  # amplitude + phase

            if len(data_values) < expected_values:
                raise CSIExtractionError(
                    f"ESP32 CSI data incomplete: expected {expected_values} values "
                    f"(amplitude + phase for {num_antennas} antennas x {num_subcarriers} subcarriers), "
                    f"but received {len(data_values)} values. "
                    "Ensure the ESP32 firmware is configured to output full CSI matrix data. "
                    "See docs/hardware-setup.md for ESP32 CSI configuration."
                )

            try:
                float_values = [float(v) for v in data_values[:expected_values]]
            except ValueError as ve:
                raise CSIExtractionError(
                    f"ESP32 CSI data contains non-numeric values: {ve}. "
                    "Raw CSI fields must be numeric float values."
                )

            all_values = np.array(float_values)
            amplitude = all_values[:num_antennas * num_subcarriers].reshape(num_antennas, num_subcarriers)
            phase = all_values[num_antennas * num_subcarriers:].reshape(num_antennas, num_subcarriers)
            
            return CSIData(
                timestamp=datetime.fromtimestamp(timestamp_ms / 1000, tz=timezone.utc),
                amplitude=amplitude,
                phase=phase,
                frequency=frequency,
                bandwidth=bandwidth,
                num_subcarriers=num_subcarriers,
                num_antennas=num_antennas,
                snr=snr,
                metadata={'source': 'esp32', 'raw_length': len(raw_data)}
            )
            
        except (ValueError, IndexError) as e:
            raise CSIParseError(f"Failed to parse ESP32 data: {e}")


class RouterCSIParser:
    """Parser for router CSI data format."""
    
    def parse(self, raw_data: bytes) -> CSIData:
        """Parse router CSI data format.
        
        Args:
            raw_data: Raw bytes from router
            
        Returns:
            Parsed CSI data
            
        Raises:
            CSIParseError: If data format is invalid
        """
        if not raw_data:
            raise CSIParseError("Empty data received")
        
        # Handle different router formats
        data_str = raw_data.decode('utf-8')
        
        if data_str.startswith('ATHEROS_CSI:'):
            return self._parse_atheros_format(raw_data)
        else:
            raise CSIParseError("Unknown router CSI format")
    
    def _parse_atheros_format(self, raw_data: bytes) -> CSIData:
        """Parse Atheros CSI format.

        Raises:
            CSIExtractionError: Always, because Atheros CSI parsing requires
                the Atheros CSI Tool binary format parser which has not been
                implemented yet. Use the ESP32 parser or contribute an
                Atheros implementation.
        """
        raise CSIExtractionError(
            "Atheros CSI format parsing is not yet implemented. "
            "The Atheros CSI Tool outputs a binary format that requires a dedicated parser. "
            "To collect real CSI data from Atheros-based routers, you must implement "
            "the binary format parser following the Atheros CSI Tool specification. "
            "See docs/hardware-setup.md for supported hardware and data formats."
        )


class CSIExtractor:
    """Main CSI data extractor supporting multiple hardware types."""
    
    def __init__(self, config: Dict[str, Any], logger: Optional[logging.Logger] = None):
        """Initialize CSI extractor.
        
        Args:
            config: Configuration dictionary
            logger: Optional logger instance
            
        Raises:
            ValueError: If configuration is invalid
        """
        self._validate_config(config)
        
        self.config = config
        self.logger = logger or logging.getLogger(__name__)
        self.hardware_type = config['hardware_type']
        self.sampling_rate = config['sampling_rate']
        self.buffer_size = config['buffer_size']
        self.timeout = config['timeout']
        self.validation_enabled = config.get('validation_enabled', True)
        self.retry_attempts = config.get('retry_attempts', 3)
        
        # State management
        self.is_connected = False
        self.is_streaming = False
        
        # Create appropriate parser
        if self.hardware_type == 'esp32':
            self.parser = ESP32CSIParser()
        elif self.hardware_type == 'router':
            self.parser = RouterCSIParser()
        else:
            raise ValueError(f"Unsupported hardware type: {self.hardware_type}")
    
    def _validate_config(self, config: Dict[str, Any]) -> None:
        """Validate configuration parameters.
        
        Args:
            config: Configuration to validate
            
        Raises:
            ValueError: If configuration is invalid
        """
        required_fields = ['hardware_type', 'sampling_rate', 'buffer_size', 'timeout']
        missing_fields = [field for field in required_fields if field not in config]
        
        if missing_fields:
            raise ValueError(f"Missing required configuration: {missing_fields}")
        
        if config['sampling_rate'] <= 0:
            raise ValueError("sampling_rate must be positive")
        
        if config['buffer_size'] <= 0:
            raise ValueError("buffer_size must be positive")
        
        if config['timeout'] <= 0:
            raise ValueError("timeout must be positive")
    
    async def connect(self) -> bool:
        """Establish connection to CSI hardware.
        
        Returns:
            True if connection successful, False otherwise
        """
        try:
            success = await self._establish_hardware_connection()
            self.is_connected = success
            return success
        except Exception as e:
            self.logger.error(f"Failed to connect to hardware: {e}")
            self.is_connected = False
            return False
    
    async def disconnect(self) -> None:
        """Disconnect from CSI hardware."""
        if self.is_connected:
            await self._close_hardware_connection()
            self.is_connected = False
    
    async def extract_csi(self) -> CSIData:
        """Extract CSI data from hardware.
        
        Returns:
            Extracted CSI data
            
        Raises:
            CSIParseError: If not connected or extraction fails
        """
        if not self.is_connected:
            raise CSIParseError("Not connected to hardware")
        
        # Retry mechanism for temporary failures
        for attempt in range(self.retry_attempts):
            try:
                raw_data = await self._read_raw_data()
                csi_data = self.parser.parse(raw_data)
                
                if self.validation_enabled:
                    self.validate_csi_data(csi_data)
                
                return csi_data
                
            except ConnectionError as e:
                if attempt < self.retry_attempts - 1:
                    self.logger.warning(f"Extraction attempt {attempt + 1} failed, retrying: {e}")
                    await asyncio.sleep(0.1)  # Brief delay before retry
                else:
                    raise CSIParseError(f"Extraction failed after {self.retry_attempts} attempts: {e}")
    
    def validate_csi_data(self, csi_data: CSIData) -> bool:
        """Validate CSI data structure and values.
        
        Args:
            csi_data: CSI data to validate
            
        Returns:
            True if valid
            
        Raises:
            CSIValidationError: If data is invalid
        """
        if csi_data.amplitude.size == 0:
            raise CSIValidationError("Empty amplitude data")
        
        if csi_data.phase.size == 0:
            raise CSIValidationError("Empty phase data")
        
        if csi_data.frequency <= 0:
            raise CSIValidationError("Invalid frequency")
        
        if csi_data.bandwidth <= 0:
            raise CSIValidationError("Invalid bandwidth")
        
        if csi_data.num_subcarriers <= 0:
            raise CSIValidationError("Invalid number of subcarriers")
        
        if csi_data.num_antennas <= 0:
            raise CSIValidationError("Invalid number of antennas")
        
        if csi_data.snr < -50 or csi_data.snr > 50:  # Reasonable SNR range
            raise CSIValidationError("Invalid SNR value")
        
        return True
    
    async def start_streaming(self, callback: Callable[[CSIData], None]) -> None:
        """Start streaming CSI data.
        
        Args:
            callback: Function to call with each CSI sample
        """
        self.is_streaming = True
        
        try:
            while self.is_streaming:
                csi_data = await self.extract_csi()
                callback(csi_data)
                await asyncio.sleep(1.0 / self.sampling_rate)
        except Exception as e:
            self.logger.error(f"Streaming error: {e}")
        finally:
            self.is_streaming = False
    
    def stop_streaming(self) -> None:
        """Stop streaming CSI data."""
        self.is_streaming = False
    
    async def _establish_hardware_connection(self) -> bool:
        """Establish connection to hardware (to be implemented by subclasses)."""
        # Placeholder implementation for testing
        return True
    
    async def _close_hardware_connection(self) -> None:
        """Close hardware connection (to be implemented by subclasses)."""
        # Placeholder implementation for testing
        pass
    
    async def _read_raw_data(self) -> bytes:
        """Read raw data from hardware (to be implemented by subclasses)."""
        # Placeholder implementation for testing
        return b"CSI_DATA:1234567890,3,56,2400,20,15.5,[1.0,2.0,3.0],[0.5,1.5,2.5]"