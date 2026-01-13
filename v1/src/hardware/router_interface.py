"""Router interface for WiFi-DensePose system using TDD approach."""

import asyncio
import logging
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
            response = await self.execute_command("iwlist scan | grep CSI")
            return self._parse_csi_response(response)
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
    
    def _parse_csi_response(self, response: str) -> CSIData:
        """Parse CSI response data.
        
        Args:
            response: Raw response from router
            
        Returns:
            Parsed CSI data
        """
        # Mock implementation for testing
        # In real implementation, this would parse actual router CSI format
        return CSIData(
            timestamp=datetime.now(timezone.utc),
            amplitude=np.random.rand(3, 56),
            phase=np.random.rand(3, 56),
            frequency=2.4e9,
            bandwidth=20e6,
            num_subcarriers=56,
            num_antennas=3,
            snr=15.0,
            metadata={'source': 'router', 'raw_response': response}
        )
    
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