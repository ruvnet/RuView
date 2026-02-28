"""
Router interface for WiFi CSI data collection
"""

import logging
import asyncio
import time
from typing import Dict, List, Optional, Any
from datetime import datetime

import numpy as np

logger = logging.getLogger(__name__)


class RouterInterface:
    """Interface for connecting to WiFi routers and collecting CSI data."""
    
    def __init__(
        self,
        router_id: str,
        host: str,
        port: int = 22,
        username: str = "admin",
        password: str = "",
        interface: str = "wlan0",
        mock_mode: bool = False
    ):
        """Initialize router interface.
        
        Args:
            router_id: Unique identifier for the router
            host: Router IP address or hostname
            port: SSH port for connection
            username: SSH username
            password: SSH password
            interface: WiFi interface name
            mock_mode: Whether to use mock data instead of real connection
        """
        self.router_id = router_id
        self.host = host
        self.port = port
        self.username = username
        self.password = password
        self.interface = interface
        self.mock_mode = mock_mode
        
        self.logger = logging.getLogger(f"{__name__}.{router_id}")
        
        # Connection state
        self.is_connected = False
        self.connection = None
        self.last_error = None
        
        # Data collection state
        self.last_data_time = None
        self.error_count = 0
        self.sample_count = 0
        
        # Mock data generation (delegated to testing module)
        self._mock_csi_generator = None
        if mock_mode:
            self._initialize_mock_generator()

    def _initialize_mock_generator(self):
        """Initialize mock data generator from the testing module."""
        from src.testing.mock_csi_generator import MockCSIGenerator
        self._mock_csi_generator = MockCSIGenerator()
        self._mock_csi_generator.show_banner()
    
    async def connect(self):
        """Connect to the router."""
        if self.mock_mode:
            self.is_connected = True
            self.logger.info(f"Mock connection established to router {self.router_id}")
            return
        
        try:
            self.logger.info(f"Connecting to router {self.router_id} at {self.host}:{self.port}")
            
            # In a real implementation, this would establish SSH connection
            # For now, we'll simulate the connection
            await asyncio.sleep(0.1)  # Simulate connection delay
            
            self.is_connected = True
            self.error_count = 0
            self.logger.info(f"Connected to router {self.router_id}")
            
        except Exception as e:
            self.last_error = str(e)
            self.error_count += 1
            self.logger.error(f"Failed to connect to router {self.router_id}: {e}")
            raise
    
    async def disconnect(self):
        """Disconnect from the router."""
        try:
            if self.connection:
                # Close SSH connection
                self.connection = None
            
            self.is_connected = False
            self.logger.info(f"Disconnected from router {self.router_id}")
            
        except Exception as e:
            self.logger.error(f"Error disconnecting from router {self.router_id}: {e}")
    
    async def reconnect(self):
        """Reconnect to the router."""
        await self.disconnect()
        await asyncio.sleep(1)  # Wait before reconnecting
        await self.connect()
    
    async def get_csi_data(self) -> Optional[np.ndarray]:
        """Get CSI data from the router.
        
        Returns:
            CSI data as numpy array, or None if no data available
        """
        if not self.is_connected:
            raise RuntimeError(f"Router {self.router_id} is not connected")
        
        try:
            if self.mock_mode:
                csi_data = self._generate_mock_csi_data()
            else:
                csi_data = await self._collect_real_csi_data()
            
            if csi_data is not None:
                self.last_data_time = datetime.now()
                self.sample_count += 1
                self.error_count = 0
            
            return csi_data
            
        except Exception as e:
            self.last_error = str(e)
            self.error_count += 1
            self.logger.error(f"Error getting CSI data from router {self.router_id}: {e}")
            return None
    
    def _generate_mock_csi_data(self) -> np.ndarray:
        """Generate mock CSI data for testing.

        Delegates to the MockCSIGenerator in the testing module.
        This method is only callable when mock_mode is True.
        """
        if self._mock_csi_generator is None:
            self._initialize_mock_generator()
        return self._mock_csi_generator.generate()
    
    async def _collect_real_csi_data(self) -> Optional[np.ndarray]:
        """Collect real CSI data from the router.

        Raises:
            RuntimeError: Always in the current state, because real CSI
                data collection requires hardware setup that has not been
                configured. This method must never silently return random
                or placeholder data.
        """
        raise RuntimeError(
            f"Real CSI data collection from router '{self.router_id}' requires "
            "hardware setup that is not configured. You must: "
            "(1) install CSI-capable firmware (e.g., Atheros CSI Tool, Nexmon CSI) on the router, "
            "(2) configure the SSH connection to the router, and "
            "(3) implement the CSI extraction command for your specific firmware. "
            "For development/testing, use mock_mode=True. "
            "See docs/hardware-setup.md for complete setup instructions."
        )
    
    async def check_health(self) -> bool:
        """Check if the router connection is healthy.
        
        Returns:
            True if healthy, False otherwise
        """
        if not self.is_connected:
            return False
        
        try:
            # In mock mode, always healthy
            if self.mock_mode:
                return True
            
            # For real connections, we could ping the router or check SSH connection
            # For now, consider healthy if error count is low
            return self.error_count < 5
            
        except Exception as e:
            self.logger.error(f"Error checking health of router {self.router_id}: {e}")
            return False
    
    async def get_status(self) -> Dict[str, Any]:
        """Get router status information.
        
        Returns:
            Dictionary containing router status
        """
        return {
            "router_id": self.router_id,
            "connected": self.is_connected,
            "mock_mode": self.mock_mode,
            "last_data_time": self.last_data_time.isoformat() if self.last_data_time else None,
            "error_count": self.error_count,
            "sample_count": self.sample_count,
            "last_error": self.last_error,
            "configuration": {
                "host": self.host,
                "port": self.port,
                "username": self.username,
                "interface": self.interface
            }
        }
    
    async def get_router_info(self) -> Dict[str, Any]:
        """Get router hardware information.
        
        Returns:
            Dictionary containing router information
        """
        if self.mock_mode:
            if self._mock_csi_generator is None:
                self._initialize_mock_generator()
            return self._mock_csi_generator.get_router_info()
        
        # For real routers, this would query the actual hardware
        return {
            "model": "Unknown",
            "firmware": "Unknown",
            "wifi_standard": "Unknown",
            "antennas": 1,
            "supported_bands": ["Unknown"],
            "csi_capabilities": {
                "max_subcarriers": 64,
                "max_antennas": 1,
                "sampling_rate": 100
            }
        }
    
    async def configure_csi_collection(self, config: Dict[str, Any]) -> bool:
        """Configure CSI data collection parameters.
        
        Args:
            config: Configuration dictionary
            
        Returns:
            True if configuration successful, False otherwise
        """
        try:
            if self.mock_mode:
                if self._mock_csi_generator is None:
                    self._initialize_mock_generator()
                self._mock_csi_generator.configure(config)
                self.logger.info(f"Mock CSI collection configured for router {self.router_id}")
                return True
            
            # For real routers, this would send configuration commands
            self.logger.warning("Real CSI configuration not implemented")
            return False
            
        except Exception as e:
            self.logger.error(f"Error configuring CSI collection for router {self.router_id}: {e}")
            return False
    
    def get_metrics(self) -> Dict[str, Any]:
        """Get router interface metrics.
        
        Returns:
            Dictionary containing metrics
        """
        uptime = 0
        if self.last_data_time:
            uptime = (datetime.now() - self.last_data_time).total_seconds()
        
        success_rate = 0
        if self.sample_count > 0:
            success_rate = (self.sample_count - self.error_count) / self.sample_count
        
        return {
            "router_id": self.router_id,
            "sample_count": self.sample_count,
            "error_count": self.error_count,
            "success_rate": success_rate,
            "uptime_seconds": uptime,
            "is_connected": self.is_connected,
            "mock_mode": self.mock_mode
        }
    
    def reset_stats(self):
        """Reset statistics counters."""
        self.error_count = 0
        self.sample_count = 0
        self.last_error = None
        self.logger.info(f"Statistics reset for router {self.router_id}")