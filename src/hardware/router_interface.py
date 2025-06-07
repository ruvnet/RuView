"""Router interface for WiFi-DensePose system."""

import paramiko
import time
import re
from typing import Dict, Any, Optional
from contextlib import contextmanager


class RouterConnectionError(Exception):
    """Exception raised for router connection errors."""
    pass


class RouterInterface:
    """Interface for communicating with WiFi routers via SSH."""
    
    def __init__(self, config: Dict[str, Any]):
        """Initialize router interface.
        
        Args:
            config: Configuration dictionary with connection parameters
        """
        self._validate_config(config)
        
        self.router_ip = config['router_ip']
        self.username = config['username']
        self.password = config['password']
        self.ssh_port = config.get('ssh_port', 22)
        self.timeout = config.get('timeout', 30)
        self.max_retries = config.get('max_retries', 3)
        
        self._ssh_client = None
        self.is_connected = False
    
    def _validate_config(self, config: Dict[str, Any]):
        """Validate configuration parameters.
        
        Args:
            config: Configuration dictionary to validate
            
        Raises:
            ValueError: If configuration is invalid
        """
        required_fields = ['router_ip', 'username', 'password']
        for field in required_fields:
            if not config.get(field):
                raise ValueError(f"Missing or empty required field: {field}")
        
        # Validate IP address format (basic check)
        ip = config['router_ip']
        if not re.match(r'^(\d{1,3}\.){3}\d{1,3}$', ip):
            raise ValueError(f"Invalid IP address format: {ip}")
    
    def connect(self) -> bool:
        """Establish SSH connection to router.
        
        Returns:
            True if connection successful, False otherwise
            
        Raises:
            RouterConnectionError: If connection fails after retries
        """
        for attempt in range(self.max_retries):
            try:
                self._ssh_client = paramiko.SSHClient()
                self._ssh_client.set_missing_host_key_policy(paramiko.AutoAddPolicy())
                
                self._ssh_client.connect(
                    hostname=self.router_ip,
                    port=self.ssh_port,
                    username=self.username,
                    password=self.password,
                    timeout=self.timeout
                )
                
                self.is_connected = True
                return True
                
            except Exception as e:
                if attempt == self.max_retries - 1:
                    raise RouterConnectionError(f"Failed to connect after {self.max_retries} attempts: {str(e)}")
                time.sleep(1)  # Brief delay before retry
        
        return False
    
    def disconnect(self):
        """Close SSH connection to router."""
        if self._ssh_client:
            self._ssh_client.close()
            self._ssh_client = None
        self.is_connected = False
    
    def execute_command(self, command: str) -> str:
        """Execute command on router via SSH.
        
        Args:
            command: Command to execute
            
        Returns:
            Command output as string
            
        Raises:
            RouterConnectionError: If not connected or command fails
        """
        if not self.is_connected or not self._ssh_client:
            raise RouterConnectionError("Not connected to router")
        
        try:
            stdin, stdout, stderr = self._ssh_client.exec_command(command)
            
            output = stdout.read().decode('utf-8').strip()
            error = stderr.read().decode('utf-8').strip()
            
            if error:
                raise RouterConnectionError(f"Command failed: {error}")
            
            return output
            
        except Exception as e:
            raise RouterConnectionError(f"Failed to execute command: {str(e)}")
    
    def get_router_info(self) -> Dict[str, str]:
        """Get router system information.
        
        Returns:
            Dictionary containing router information
        """
        # Try common commands to get router info
        info = {}
        
        try:
            # Try to get model information
            model_output = self.execute_command("cat /proc/cpuinfo | grep 'model name' | head -1")
            if model_output:
                info['model'] = model_output.split(':')[-1].strip()
            else:
                info['model'] = "Unknown"
        except:
            info['model'] = "Unknown"
        
        try:
            # Try to get firmware version
            firmware_output = self.execute_command("cat /etc/openwrt_release | grep DISTRIB_RELEASE")
            if firmware_output:
                info['firmware'] = firmware_output.split('=')[-1].strip().strip("'\"")
            else:
                info['firmware'] = "Unknown"
        except:
            info['firmware'] = "Unknown"
        
        return info
    
    def enable_monitor_mode(self, interface: str) -> bool:
        """Enable monitor mode on WiFi interface.
        
        Args:
            interface: WiFi interface name (e.g., 'wlan0')
            
        Returns:
            True if successful, False otherwise
        """
        try:
            # Bring interface down
            self.execute_command(f"ifconfig {interface} down")
            
            # Set monitor mode
            self.execute_command(f"iwconfig {interface} mode monitor")
            
            # Bring interface up
            self.execute_command(f"ifconfig {interface} up")
            
            return True
            
        except RouterConnectionError:
            return False
    
    def disable_monitor_mode(self, interface: str) -> bool:
        """Disable monitor mode on WiFi interface.
        
        Args:
            interface: WiFi interface name (e.g., 'wlan0')
            
        Returns:
            True if successful, False otherwise
        """
        try:
            # Bring interface down
            self.execute_command(f"ifconfig {interface} down")
            
            # Set managed mode
            self.execute_command(f"iwconfig {interface} mode managed")
            
            # Bring interface up
            self.execute_command(f"ifconfig {interface} up")
            
            return True
            
        except RouterConnectionError:
            return False
    
    def __enter__(self):
        """Context manager entry."""
        self.connect()
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self.disconnect()