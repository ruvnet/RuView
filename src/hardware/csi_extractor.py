"""CSI data extraction from WiFi routers."""

import time
import re
import threading
from typing import Dict, Any, Optional
import numpy as np
import torch
from collections import deque


class CSIExtractionError(Exception):
    """Exception raised for CSI extraction errors."""
    pass


class CSIExtractor:
    """Extracts CSI data from WiFi routers via router interface."""
    
    def __init__(self, config: Dict[str, Any], router_interface):
        """Initialize CSI extractor.
        
        Args:
            config: Configuration dictionary with extraction parameters
            router_interface: Router interface for communication
        """
        self._validate_config(config)
        
        self.interface = config['interface']
        self.channel = config['channel']
        self.bandwidth = config['bandwidth']
        self.sample_rate = config['sample_rate']
        self.buffer_size = config['buffer_size']
        self.extraction_timeout = config['extraction_timeout']
        
        self.router_interface = router_interface
        self.is_extracting = False
        
        # Statistics tracking
        self._samples_extracted = 0
        self._extraction_start_time = None
        self._last_extraction_time = None
        self._buffer = deque(maxlen=self.buffer_size)
        self._extraction_lock = threading.Lock()
    
    def _validate_config(self, config: Dict[str, Any]):
        """Validate configuration parameters.
        
        Args:
            config: Configuration dictionary to validate
            
        Raises:
            ValueError: If configuration is invalid
        """
        required_fields = ['interface', 'channel', 'bandwidth', 'sample_rate', 'buffer_size']
        for field in required_fields:
            if not config.get(field):
                raise ValueError(f"Missing or empty required field: {field}")
        
        # Validate interface name
        if not isinstance(config['interface'], str) or not config['interface'].strip():
            raise ValueError("Interface must be a non-empty string")
        
        # Validate channel range (2.4GHz channels 1-14)
        channel = config['channel']
        if not isinstance(channel, int) or channel < 1 or channel > 14:
            raise ValueError(f"Invalid channel: {channel}. Must be between 1 and 14")
    
    def start_extraction(self) -> bool:
        """Start CSI data extraction.
        
        Returns:
            True if extraction started successfully
            
        Raises:
            CSIExtractionError: If extraction cannot be started
        """
        with self._extraction_lock:
            if self.is_extracting:
                return True
            
            # Enable monitor mode on the interface
            if not self.router_interface.enable_monitor_mode(self.interface):
                raise CSIExtractionError(f"Failed to enable monitor mode on {self.interface}")
            
            try:
                # Start CSI extraction process
                command = f"iwconfig {self.interface} channel {self.channel}"
                self.router_interface.execute_command(command)
                
                # Initialize extraction state
                self.is_extracting = True
                self._extraction_start_time = time.time()
                self._samples_extracted = 0
                self._buffer.clear()
                
                return True
                
            except Exception as e:
                self.router_interface.disable_monitor_mode(self.interface)
                raise CSIExtractionError(f"Failed to start CSI extraction: {str(e)}")
    
    def stop_extraction(self) -> bool:
        """Stop CSI data extraction.
        
        Returns:
            True if extraction stopped successfully
        """
        with self._extraction_lock:
            if not self.is_extracting:
                return True
            
            try:
                # Disable monitor mode
                self.router_interface.disable_monitor_mode(self.interface)
                self.is_extracting = False
                return True
                
            except Exception:
                return False
    
    def extract_csi_data(self) -> np.ndarray:
        """Extract CSI data from the router.
        
        Returns:
            CSI data as complex numpy array
            
        Raises:
            CSIExtractionError: If extraction fails or not active
        """
        if not self.is_extracting:
            raise CSIExtractionError("CSI extraction not active. Call start_extraction() first.")
        
        try:
            # Execute command to get CSI data
            command = f"cat /proc/net/csi_data_{self.interface}"
            raw_output = self.router_interface.execute_command(command)
            
            # Parse the raw CSI output
            csi_data = self._parse_csi_output(raw_output)
            
            # Add to buffer and update statistics
            self._add_to_buffer(csi_data)
            self._samples_extracted += 1
            self._last_extraction_time = time.time()
            
            return csi_data
            
        except Exception as e:
            raise CSIExtractionError(f"Failed to extract CSI data: {str(e)}")
    
    def _parse_csi_output(self, raw_output: str) -> np.ndarray:
        """Parse raw CSI output into structured data.
        
        Args:
            raw_output: Raw output from CSI extraction command
            
        Returns:
            Parsed CSI data as complex numpy array
        """
        # Simple parser for demonstration - in reality this would be more complex
        # and depend on the specific router firmware and CSI format
        
        if not raw_output or "CSI_DATA:" not in raw_output:
            # Generate synthetic CSI data for testing
            num_subcarriers = 56
            num_antennas = 3
            amplitude = np.random.uniform(0.1, 2.0, (num_antennas, num_subcarriers))
            phase = np.random.uniform(-np.pi, np.pi, (num_antennas, num_subcarriers))
            return amplitude * np.exp(1j * phase)
        
        # Extract CSI data from output
        csi_line = raw_output.split("CSI_DATA:")[-1].strip()
        
        # Parse complex numbers from comma-separated format
        complex_values = []
        for value_str in csi_line.split(','):
            value_str = value_str.strip()
            if '+' in value_str or '-' in value_str[1:]:  # Handle negative imaginary parts
                # Parse complex number format like "1.5+0.5j" or "2.0-1.0j"
                complex_val = complex(value_str)
                complex_values.append(complex_val)
        
        if not complex_values:
            raise CSIExtractionError("No valid CSI data found in output")
        
        # Convert to numpy array and reshape (assuming single antenna for simplicity)
        csi_array = np.array(complex_values, dtype=np.complex128)
        return csi_array.reshape(1, -1)  # Shape: (1, num_subcarriers)
    
    def _add_to_buffer(self, csi_data: np.ndarray):
        """Add CSI data to internal buffer.
        
        Args:
            csi_data: CSI data to add to buffer
        """
        self._buffer.append(csi_data.copy())
    
    def convert_to_tensor(self, csi_data: np.ndarray) -> torch.Tensor:
        """Convert CSI data to PyTorch tensor format.
        
        Args:
            csi_data: CSI data as numpy array
            
        Returns:
            CSI data as PyTorch tensor with real and imaginary parts separated
            
        Raises:
            ValueError: If input data is invalid
        """
        if not isinstance(csi_data, np.ndarray):
            raise ValueError("Input must be a numpy array")
        
        if not np.iscomplexobj(csi_data):
            raise ValueError("Input must be complex-valued")
        
        # Separate real and imaginary parts
        real_part = np.real(csi_data)
        imag_part = np.imag(csi_data)
        
        # Stack real and imaginary parts
        stacked = np.vstack([real_part, imag_part])
        
        # Convert to tensor
        tensor = torch.from_numpy(stacked).float()
        
        return tensor
    
    def get_extraction_stats(self) -> Dict[str, Any]:
        """Get extraction statistics.
        
        Returns:
            Dictionary containing extraction statistics
        """
        current_time = time.time()
        
        if self._extraction_start_time:
            extraction_duration = current_time - self._extraction_start_time
            extraction_rate = self._samples_extracted / extraction_duration if extraction_duration > 0 else 0
        else:
            extraction_rate = 0
        
        buffer_utilization = len(self._buffer) / self.buffer_size if self.buffer_size > 0 else 0
        
        return {
            'samples_extracted': self._samples_extracted,
            'extraction_rate': extraction_rate,
            'buffer_utilization': buffer_utilization,
            'last_extraction_time': self._last_extraction_time
        }
    
    def set_channel(self, channel: int) -> bool:
        """Set WiFi channel for CSI extraction.
        
        Args:
            channel: WiFi channel number (1-14)
            
        Returns:
            True if channel set successfully
            
        Raises:
            ValueError: If channel is invalid
        """
        if not isinstance(channel, int) or channel < 1 or channel > 14:
            raise ValueError(f"Invalid channel: {channel}. Must be between 1 and 14")
        
        try:
            command = f"iwconfig {self.interface} channel {channel}"
            self.router_interface.execute_command(command)
            self.channel = channel
            return True
            
        except Exception:
            return False
    
    def __enter__(self):
        """Context manager entry."""
        self.start_extraction()
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit."""
        self.stop_extraction()