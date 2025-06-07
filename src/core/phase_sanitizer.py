"""Phase sanitizer for WiFi-DensePose CSI phase data processing."""

import numpy as np
import torch
from typing import Optional
from scipy import signal


class PhaseSanitizer:
    """Sanitizes phase data by unwrapping, removing outliers, and smoothing."""
    
    def __init__(self, outlier_threshold: float = 3.0, smoothing_window: int = 5):
        """Initialize phase sanitizer with configuration.
        
        Args:
            outlier_threshold: Standard deviations for outlier detection
            smoothing_window: Window size for smoothing filter
        """
        self.outlier_threshold = outlier_threshold
        self.smoothing_window = smoothing_window
    
    def unwrap_phase(self, phase_data: np.ndarray) -> np.ndarray:
        """Unwrap phase data to remove 2π discontinuities.
        
        Args:
            phase_data: Raw phase data array
            
        Returns:
            Unwrapped phase data
        """
        if phase_data.size == 0:
            raise ValueError("Phase data cannot be empty")
        
        # Apply unwrapping along the last axis (temporal dimension)
        unwrapped = np.unwrap(phase_data, axis=-1)
        return unwrapped.astype(np.float32)
    
    def remove_outliers(self, phase_data: np.ndarray) -> np.ndarray:
        """Remove outliers from phase data using statistical thresholding.
        
        Args:
            phase_data: Phase data array
            
        Returns:
            Phase data with outliers replaced
        """
        if phase_data.size == 0:
            raise ValueError("Phase data cannot be empty")
        
        result = phase_data.copy().astype(np.float32)
        
        # Calculate statistics for outlier detection
        mean_val = np.mean(result)
        std_val = np.std(result)
        
        # Identify outliers
        outlier_mask = np.abs(result - mean_val) > (self.outlier_threshold * std_val)
        
        # Replace outliers with mean value
        result[outlier_mask] = mean_val
        
        return result
    
    def sanitize_phase_batch(self, processed_csi: torch.Tensor) -> torch.Tensor:
        """Sanitize phase information in a batch of processed CSI data.
        
        Args:
            processed_csi: Processed CSI tensor from CSI processor
            
        Returns:
            CSI tensor with sanitized phase information
        """
        if not isinstance(processed_csi, torch.Tensor):
            raise ValueError("Input must be a torch.Tensor")
        
        # Convert to numpy for processing
        csi_numpy = processed_csi.detach().cpu().numpy()
        
        # The processed CSI has shape (batch, channels, subcarriers, time)
        # where channels = 2 * antennas (amplitude and phase interleaved)
        batch_size, channels, subcarriers, time_samples = csi_numpy.shape
        
        # Process phase channels (odd indices contain phase information)
        for batch_idx in range(batch_size):
            for ch_idx in range(1, channels, 2):  # Phase channels are at odd indices
                phase_data = csi_numpy[batch_idx, ch_idx, :, :]
                sanitized_phase = self.sanitize(phase_data)
                csi_numpy[batch_idx, ch_idx, :, :] = sanitized_phase
        
        # Convert back to tensor
        return torch.from_numpy(csi_numpy).float()
    
    def smooth_phase(self, phase_data: np.ndarray) -> np.ndarray:
        """Apply smoothing filter to reduce noise in phase data.
        
        Args:
            phase_data: Phase data array
            
        Returns:
            Smoothed phase data
        """
        if phase_data.size == 0:
            raise ValueError("Phase data cannot be empty")
        
        result = phase_data.copy().astype(np.float32)
        
        # Apply simple moving average filter along temporal dimension
        if result.ndim > 1:
            for i in range(result.shape[0]):
                if result.shape[-1] >= self.smoothing_window:
                    # Apply 1D smoothing along the last axis
                    kernel = np.ones(self.smoothing_window) / self.smoothing_window
                    result[i] = np.convolve(result[i], kernel, mode='same')
        else:
            if result.shape[0] >= self.smoothing_window:
                kernel = np.ones(self.smoothing_window) / self.smoothing_window
                result = np.convolve(result, kernel, mode='same')
        
        return result
    
    def sanitize(self, phase_data: np.ndarray) -> np.ndarray:
        """Apply full sanitization pipeline to phase data.
        
        Args:
            phase_data: Raw phase data array
            
        Returns:
            Fully sanitized phase data
        """
        if phase_data.size == 0:
            raise ValueError("Phase data cannot be empty")
        
        # Apply sanitization pipeline
        result = self.unwrap_phase(phase_data)
        result = self.remove_outliers(result)
        result = self.smooth_phase(result)
        
        return result