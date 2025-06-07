"""CSI (Channel State Information) processor for WiFi-DensePose system."""

import numpy as np
from typing import Dict, Any, Optional


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