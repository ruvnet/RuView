"""CSI (Channel State Information) processor for WiFi-DensePose system."""

import numpy as np
import torch
from typing import Dict, Any, Optional, List
from datetime import datetime
from collections import deque


class CSIProcessor:
    """Processes raw CSI data for neural network input."""
    
    def __init__(self, config: Optional[Dict[str, Any]] = None):
        """Initialize CSI processor with configuration.
        
        Args:
            config: Configuration dictionary with processing parameters
        """
        self.config = config or {}
        self.sample_rate = self.config.get('sample_rate', 1000)
        self.num_subcarriers = self.config.get('num_subcarriers', 56)
        self.num_antennas = self.config.get('num_antennas', 3)
        self.buffer_size = self.config.get('buffer_size', 1000)
        
        # Data buffer for temporal processing
        self.data_buffer = deque(maxlen=self.buffer_size)
        self.last_processed_data = None
    
    def process_raw_csi(self, raw_data: np.ndarray) -> np.ndarray:
        """Process raw CSI data into normalized format.
        
        Args:
            raw_data: Raw CSI data array
            
        Returns:
            Processed CSI data ready for neural network input
        """
        if raw_data.size == 0:
            raise ValueError("Raw CSI data cannot be empty")
        
        # Basic processing: normalize and reshape
        processed = raw_data.astype(np.float32)
        
        # Handle NaN values by replacing with mean of non-NaN values
        if np.isnan(processed).any():
            nan_mask = np.isnan(processed)
            non_nan_mean = np.nanmean(processed)
            processed[nan_mask] = non_nan_mean
        
        # Simple normalization
        if processed.std() > 0:
            processed = (processed - processed.mean()) / processed.std()
        
        return processed
    
    def process_csi_batch(self, csi_data: np.ndarray) -> torch.Tensor:
        """Process a batch of CSI data for neural network input.
        
        Args:
            csi_data: Complex CSI data array of shape (batch, antennas, subcarriers, time)
            
        Returns:
            Processed CSI tensor ready for neural network input
        """
        if csi_data.ndim != 4:
            raise ValueError(f"Expected 4D input (batch, antennas, subcarriers, time), got {csi_data.ndim}D")
        
        batch_size, num_antennas, num_subcarriers, time_samples = csi_data.shape
        
        # Extract amplitude and phase
        amplitude = np.abs(csi_data)
        phase = np.angle(csi_data)
        
        # Process each component
        processed_amplitude = self.process_raw_csi(amplitude)
        processed_phase = self.process_raw_csi(phase)
        
        # Stack amplitude and phase as separate channels
        processed_data = np.stack([processed_amplitude, processed_phase], axis=1)
        
        # Reshape to (batch, channels, antennas, subcarriers, time)
        # Then flatten spatial dimensions for CNN input
        processed_data = processed_data.reshape(batch_size, 2 * num_antennas, num_subcarriers, time_samples)
        
        # Convert to tensor
        return torch.from_numpy(processed_data).float()
    
    def add_data(self, csi_data: np.ndarray, timestamp: datetime):
        """Add CSI data to the processing buffer.
        
        Args:
            csi_data: Raw CSI data array
            timestamp: Timestamp of the data sample
        """
        sample = {
            'data': csi_data,
            'timestamp': timestamp,
            'processed': False
        }
        self.data_buffer.append(sample)
    
    def get_processed_data(self) -> Optional[np.ndarray]:
        """Get the most recent processed CSI data.
        
        Returns:
            Processed CSI data array or None if no data available
        """
        if not self.data_buffer:
            return None
        
        # Get the most recent unprocessed sample
        recent_sample = None
        for sample in reversed(self.data_buffer):
            if not sample['processed']:
                recent_sample = sample
                break
        
        if recent_sample is None:
            return self.last_processed_data
        
        # Process the data
        try:
            processed_data = self.process_raw_csi(recent_sample['data'])
            recent_sample['processed'] = True
            self.last_processed_data = processed_data
            return processed_data
        except Exception as e:
            # Return last known good data if processing fails
            return self.last_processed_data