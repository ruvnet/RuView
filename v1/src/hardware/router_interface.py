"""Router interface for WiFi-DensePose system using TDD approach."""

import asyncio
import base64
import logging
import struct
from typing import Dict, Any, Optional
import asyncssh
from datetime import datetime, timezone
import numpy as np

try:
    from .csi_extractor import CSIData
except ImportError:
    # Handle import for testing
    from src.hardware.csi_extractor import CSIData


class RouterConnectionError(Exception):
    """Exception raised for router connection errors."""
    pass


class RouterCSIParser:
    """Parser for router CSI matrices (Atheros / Nexmon)."""

    class AtherosCSIFormat:
        HEADER_SIZE = 25

        @staticmethod
        def parse_header(data: bytes) -> Dict[str, Any]:
            if len(data) < RouterCSIParser.AtherosCSIFormat.HEADER_SIZE:
                raise CSIParseError("Atheros header too short")
            timestamp = struct.unpack('<Q', data[0:8])[0]
            channel, rate = struct.unpack('<HH', data[8:12])
            rssi = struct.unpack('<b', data[12:13])[0]
            noise = struct.unpack('<b', data[13:14])[0]
            antenna_config = data[14]
            csi_length = struct.unpack('<H', data[15:17])[0]
            mac_addr = struct.unpack('<Q', data[17:25])[0]
            return {
                'timestamp': timestamp,
                'channel': channel,
                'rate': rate,
                'rssi': rssi,
                'noise': noise,
                'antenna_config': antenna_config,
                'csi_length': csi_length,
                'mac_address': mac_addr
            }

        @staticmethod
        def _extract_10bit(data: bytes, bit_offset: int) -> int:
            byte_offset = bit_offset // 8
            bit_shift = bit_offset % 8
            if byte_offset + 1 >= len(data):
                return 0
            window = (data[byte_offset] << 8) | data[byte_offset + 1]
            return (window >> (6 - bit_shift)) & 0x3FF

        @staticmethod
        def parse_csi_data(data: bytes, header: Dict[str, Any]) -> np.ndarray:
            start = RouterCSIParser.AtherosCSIFormat.HEADER_SIZE
            length = header['csi_length']
            if len(data) < start + length:
                raise CSIParseError("Atheros CSI payload truncated")
            payload = data[start:start + length]
            samples = []
            bit_offset = 0
            while bit_offset + 20 <= len(payload) * 8:
                real = RouterCSIParser.AtherosCSIFormat._extract_10bit(payload, bit_offset)
                imag = RouterCSIParser.AtherosCSIFormat._extract_10bit(payload, bit_offset + 10)
                real = real - 512 if real > 511 else real
                imag = imag - 512 if imag > 511 else imag
                samples.append(complex(real, imag))
                bit_offset += 20
            if not samples:
                raise CSIParseError("No complex samples recovered from Atheros payload")
            tx = 3 if header['antenna_config'] == 0x07 else 2
            rx = 3
            num_subcarriers = len(samples) // (tx * rx)
            if num_subcarriers == 0:
                raise CSIParseError("Subcarrier count is zero")
            matrix = np.array(samples[:tx * rx * num_subcarriers])
            matrix = matrix.reshape((tx * rx, num_subcarriers))
            return matrix

    def parse(self, raw_data: bytes) -> CSIData:
        data = raw_data.strip()
        if data.startswith(b'CSI_HEX:'):
            data = bytes.fromhex(data.split(b':', 1)[1].strip().decode('utf-8'))
        elif data.startswith(b'CSI_BASE64:'):
            data = base64.b64decode(data.split(b':', 1)[1].strip())
        elif data.startswith(b'0x'):
            data = bytes.fromhex(data[2:].decode('utf-8'))

        header = self.AtherosCSIFormat.parse_header(data)
        matrix = self.AtherosCSIFormat.parse_csi_data(data, header)
        amplitude = np.abs(matrix)
        phase = np.angle(matrix)
        return CSIData(
            timestamp=datetime.now(tz=timezone.utc),
            amplitude=amplitude,
            phase=phase,
            frequency=header['channel'] * 1e6,
            bandwidth=20e6,
            num_subcarriers=amplitude.shape[1],
            num_antennas=amplitude.shape[0],
            snr=float(header['rssi'] - header['noise']),
            metadata={
                'source': 'router',
                'router_channel': header['channel'],
                'mac_address': header['mac_address']
            }
        )


class RouterInterface:
    """Interface for communicating with WiFi routers via SSH."""
    
    def __init__(self, config: Dict[str, Any], logger: Optional[logging.Logger] = None):
        """Initialize router interface.
        
        Args:
            config: Configuration dictionary with connection parameters
            logger: Optional logger instance
            
        Raises:
            ValueError: If configuration is invalid
        """
        self._validate_config(config)
        
        self.config = config
        self.logger = logger or logging.getLogger(__name__)
        
        # Connection parameters
        self.host = config['host']
        self.port = config['port']
        self.username = config['username']
        self.password = config['password']
        self.command_timeout = config.get('command_timeout', 30)
        self.connection_timeout = config.get('connection_timeout', 10)
        self.max_retries = config.get('max_retries', 3)
        self.retry_delay = config.get('retry_delay', 1.0)
        
        # Connection state
        self.is_connected = False
        self.ssh_client = None
        self.parser = RouterCSIParser()
        self.csi_command = config.get('csi_command', 'cat /tmp/csi.bin')
        self.output_encoding = config.get('output_encoding', 'binary')
    
    def _validate_config(self, config: Dict[str, Any]) -> None:
        """Validate configuration parameters.
        
        Args:
            config: Configuration to validate
            
        Raises:
            ValueError: If configuration is invalid
        """
        required_fields = ['host', 'port', 'username', 'password']
        missing_fields = [field for field in required_fields if field not in config]
        
        if missing_fields:
            raise ValueError(f"Missing required configuration: {missing_fields}")
        
        if not isinstance(config['port'], int) or config['port'] <= 0:
            raise ValueError("Port must be a positive integer")
    
    async def connect(self) -> bool:
        """Establish SSH connection to router.
        
        Returns:
            True if connection successful, False otherwise
        """
        try:
            self.ssh_client = await asyncssh.connect(
                self.host,
                port=self.port,
                username=self.username,
                password=self.password,
                connect_timeout=self.connection_timeout
            )
            self.is_connected = True
            self.logger.info(f"Connected to router at {self.host}:{self.port}")
            return True
        except Exception as e:
            self.logger.error(f"Failed to connect to router: {e}")
            self.is_connected = False
            self.ssh_client = None
            return False
    
    async def disconnect(self) -> None:
        """Disconnect from router."""
        if self.is_connected and self.ssh_client:
            self.ssh_client.close()
            self.is_connected = False
            self.ssh_client = None
            self.logger.info("Disconnected from router")
    
    async def execute_command(self, command: str) -> str:
        """Execute command on router via SSH.
        
        Args:
            command: Command to execute
            
        Returns:
            Command output
            
        Raises:
            RouterConnectionError: If not connected or command fails
        """
        if not self.is_connected:
            raise RouterConnectionError("Not connected to router")
        
        # Retry mechanism for temporary failures
        for attempt in range(self.max_retries):
            try:
                result = await self.ssh_client.run(command, timeout=self.command_timeout)
                
                if result.returncode != 0:
                    raise RouterConnectionError(f"Command failed: {result.stderr}")
                
                return result.stdout
                
            except ConnectionError as e:
                if attempt < self.max_retries - 1:
                    self.logger.warning(f"Command attempt {attempt + 1} failed, retrying: {e}")
                    await asyncio.sleep(self.retry_delay)
                else:
                    raise RouterConnectionError(f"Command execution failed after {self.max_retries} retries: {e}")
            except Exception as e:
                raise RouterConnectionError(f"Command execution error: {e}")
    
    async def get_csi_data(self) -> CSIData:
        """Retrieve CSI data from router.

        Returns:
            CSI data structure

        Raises:
            RouterConnectionError: If data retrieval fails
        """
        try:
            response = await self.execute_command(self.csi_command)
            raw_bytes = self._decode_csi_output(response)
            return self.parser.parse(raw_bytes)
        except Exception as e:
            raise RouterConnectionError(f"Failed to retrieve CSI data: {e}")
    
    async def get_router_status(self) -> Dict[str, Any]:
        """Get router system status.
        
        Returns:
            Dictionary containing router status information
            
        Raises:
            RouterConnectionError: If status retrieval fails
        """
        try:
            response = await self.execute_command("cat /proc/stat && free && iwconfig")
            return self._parse_status_response(response)
        except Exception as e:
            raise RouterConnectionError(f"Failed to retrieve router status: {e}")
    
    async def configure_csi_monitoring(self, config: Dict[str, Any]) -> bool:
        """Configure CSI monitoring on router.
        
        Args:
            config: CSI monitoring configuration
            
        Returns:
            True if configuration successful, False otherwise
        """
        try:
            channel = config.get('channel', 6)
            # Validate channel is an integer in a safe range to prevent command injection
            if not isinstance(channel, int) or not (1 <= channel <= 196):
                raise ValueError(f"Invalid WiFi channel: {channel}. Must be an integer between 1 and 196.")
            command = f"iwconfig wlan0 channel {channel} && echo 'CSI monitoring configured'"
            await self.execute_command(command)
            return True
        except Exception as e:
            self.logger.error(f"Failed to configure CSI monitoring: {e}")
            return False
    
    async def health_check(self) -> bool:
        """Perform health check on router.
        
        Returns:
            True if router is healthy, False otherwise
        """
        try:
            response = await self.execute_command("echo 'ping' && echo 'pong'")
            return "pong" in response
        except Exception as e:
            self.logger.error(f"Health check failed: {e}")
            return False
    
    def _decode_csi_output(self, response: str) -> bytes:
        payload = response.strip()
        if self.output_encoding == 'hex' or payload.startswith('0x'):
            normalized = payload[2:] if payload.startswith('0x') else payload
            return bytes.fromhex(normalized)
        if self.output_encoding == 'base64' or payload.startswith('CSI_BASE64:'):
            _, encoded = payload.split(':', 1) if ':' in payload else ('', payload)
            return base64.b64decode(encoded.strip())
        if payload.startswith('CSI_HEX:'):
            _, encoded = payload.split(':', 1)
            return bytes.fromhex(encoded.strip())
        return payload.encode('latin-1')
    
    def _parse_status_response(self, response: str) -> Dict[str, Any]:
        """Parse router status response.
        
        Args:
            response: Raw response from router
            
        Returns:
            Parsed status information
        """
        # Mock implementation for testing
        # In real implementation, this would parse actual system status
        return {
            'cpu_usage': 25.5,
            'memory_usage': 60.2,
            'wifi_status': 'active',
            'uptime': '5 days, 3 hours',
            'raw_response': response
        }
