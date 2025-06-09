"""Phase sanitization module for WiFi-DensePose system using TDD approach."""

import numpy as np
import logging
from typing import Dict, Any, Optional, Tuple
from datetime import datetime, timezone
from scipy import signal


class PhaseSanitizationError(Exception):
    """Exception raised for phase sanitization errors."""
    pass


class PhaseSanitizer:
    """Sanitizes phase data from CSI signals for reliable processing."""
    
    def __init__(self, config: Dict[str, Any], logger: Optional[logging.Logger] = None):
        """Initialize phase sanitizer.
        
        Args:
            config: Configuration dictionary
            logger: Optional logger instance
            
        Raises:
            ValueError: If configuration is invalid
        """
        self._validate_config(config)
        
        self.config = config
        self.logger = logger or logging.getLogger(__name__)
        
        # Processing parameters
        self.unwrapping_method = config['unwrapping_method']
        self.outlier_threshold = config['outlier_threshold']
        self.smoothing_window = config['smoothing_window']
        
        # Optional parameters with defaults
        self.enable_outlier_removal = config.get('enable_outlier_removal', True)
        self.enable_smoothing = config.get('enable_smoothing', True)
        self.enable_noise_filtering = config.get('enable_noise_filtering', False)
        self.noise_threshold = config.get('noise_threshold', 0.05)
        self.phase_range = config.get('phase_range', (-np.pi, np.pi))
        
        # Statistics tracking
        self._total_processed = 0
        self._outliers_removed = 0
        self._sanitization_errors = 0
    
    def _validate_config(self, config: Dict[str, Any]) -> None:
        """Validate configuration parameters.
        
        Args:
            config: Configuration to validate
            
        Raises:
            ValueError: If configuration is invalid
        """
        required_fields = ['unwrapping_method', 'outlier_threshold', 'smoothing_window']
        missing_fields = [field for field in required_fields if field not in config]
        
        if missing_fields:
            raise ValueError(f"Missing required configuration: {missing_fields}")
        
        # Validate unwrapping method
        valid_methods = ['numpy', 'scipy', 'custom']
        if config['unwrapping_method'] not in valid_methods:
            raise ValueError(f"Invalid unwrapping method: {config['unwrapping_method']}. Must be one of {valid_methods}")
        
        # Validate thresholds
        if config['outlier_threshold'] <= 0:
            raise ValueError("outlier_threshold must be positive")
        
        if config['smoothing_window'] <= 0:
            raise ValueError("smoothing_window must be positive")
    
    def unwrap_phase(self, phase_data: np.ndarray) -> np.ndarray:
        """Unwrap phase data to remove discontinuities.
        
        Args:
            phase_data: Wrapped phase data (2D array)
            
        Returns:
            Unwrapped phase data
            
        Raises:
            PhaseSanitizationError: If unwrapping fails
        """
        try:
            if self.unwrapping_method == 'numpy':
                return self._unwrap_numpy(phase_data)
            elif self.unwrapping_method == 'scipy':
                return self._unwrap_scipy(phase_data)
            elif self.unwrapping_method == 'custom':
                return self._unwrap_custom(phase_data)
            else:
                raise ValueError(f"Unknown unwrapping method: {self.unwrapping_method}")
                
        except Exception as e:
            raise PhaseSanitizationError(f"Failed to unwrap phase: {e}")
    
    def _unwrap_numpy(self, phase_data: np.ndarray) -> np.ndarray:
        """Unwrap phase using numpy's unwrap function."""
        if phase_data.size == 0:
            raise ValueError("Cannot unwrap empty phase data")
        return np.unwrap(phase_data, axis=1)
    
    def _unwrap_scipy(self, phase_data: np.ndarray) -> np.ndarray:
        """Unwrap phase using scipy's unwrap function."""
        if phase_data.size == 0:
            raise ValueError("Cannot unwrap empty phase data")
        return np.unwrap(phase_data, axis=1)
    
    def _unwrap_custom(self, phase_data: np.ndarray) -> np.ndarray:
        """Unwrap phase using custom algorithm."""
        if phase_data.size == 0:
            raise ValueError("Cannot unwrap empty phase data")
        # Simple custom unwrapping algorithm
        unwrapped = phase_data.copy()
        for i in range(phase_data.shape[0]):
            unwrapped[i, :] = np.unwrap(phase_data[i, :])
        return unwrapped
    
    def remove_outliers(self, phase_data: np.ndarray) -> np.ndarray:
        """Remove outliers from phase data.
        
        Args:
            phase_data: Phase data (2D array)
            
        Returns:
            Phase data with outliers removed
            
        Raises:
            PhaseSanitizationError: If outlier removal fails
        """
        if not self.enable_outlier_removal:
            return phase_data
        
        try:
            # Detect outliers
            outlier_mask = self._detect_outliers(phase_data)
            
            # Interpolate outliers
            clean_data = self._interpolate_outliers(phase_data, outlier_mask)
            
            return clean_data
            
        except Exception as e:
            raise PhaseSanitizationError(f"Failed to remove outliers: {e}")
    
    def _detect_outliers(self, phase_data: np.ndarray) -> np.ndarray:
        """Detect outliers using statistical methods."""
        # Use Z-score method to detect outliers
        z_scores = np.abs((phase_data - np.mean(phase_data, axis=1, keepdims=True)) / 
                         (np.std(phase_data, axis=1, keepdims=True) + 1e-8))
        outlier_mask = z_scores > self.outlier_threshold
        
        # Update statistics
        self._outliers_removed += np.sum(outlier_mask)
        
        return outlier_mask
    
    def _interpolate_outliers(self, phase_data: np.ndarray, outlier_mask: np.ndarray) -> np.ndarray:
        """Interpolate outlier values."""
        clean_data = phase_data.copy()
        
        for i in range(phase_data.shape[0]):
            outliers = outlier_mask[i, :]
            if np.any(outliers):
                # Linear interpolation for outliers
                valid_indices = np.where(~outliers)[0]
                outlier_indices = np.where(outliers)[0]
                
                if len(valid_indices) > 1:
                    clean_data[i, outlier_indices] = np.interp(
                        outlier_indices, valid_indices, phase_data[i, valid_indices]
                    )
        
        return clean_data
    
    def smooth_phase(self, phase_data: np.ndarray) -> np.ndarray:
        """Smooth phase data to reduce noise.
        
        Args:
            phase_data: Phase data (2D array)
            
        Returns:
            Smoothed phase data
            
        Raises:
            PhaseSanitizationError: If smoothing fails
        """
        if not self.enable_smoothing:
            return phase_data
        
        try:
            smoothed_data = self._apply_moving_average(phase_data, self.smoothing_window)
            return smoothed_data
            
        except Exception as e:
            raise PhaseSanitizationError(f"Failed to smooth phase: {e}")
    
    def _apply_moving_average(self, phase_data: np.ndarray, window_size: int) -> np.ndarray:
        """Apply moving average smoothing."""
        smoothed_data = phase_data.copy()
        
        # Ensure window size is odd
        if window_size % 2 == 0:
            window_size += 1
        
        half_window = window_size // 2
        
        for i in range(phase_data.shape[0]):
            for j in range(half_window, phase_data.shape[1] - half_window):
                start_idx = j - half_window
                end_idx = j + half_window + 1
                smoothed_data[i, j] = np.mean(phase_data[i, start_idx:end_idx])
        
        return smoothed_data
    
    def filter_noise(self, phase_data: np.ndarray) -> np.ndarray:
        """Filter noise from phase data.
        
        Args:
            phase_data: Phase data (2D array)
            
        Returns:
            Filtered phase data
            
        Raises:
            PhaseSanitizationError: If noise filtering fails
        """
        if not self.enable_noise_filtering:
            return phase_data
        
        try:
            filtered_data = self._apply_low_pass_filter(phase_data, self.noise_threshold)
            return filtered_data
            
        except Exception as e:
            raise PhaseSanitizationError(f"Failed to filter noise: {e}")
    
    def _apply_low_pass_filter(self, phase_data: np.ndarray, threshold: float) -> np.ndarray:
        """Apply low-pass filter to remove high-frequency noise."""
        filtered_data = phase_data.copy()
        
        # Check if data is large enough for filtering
        min_filter_length = 18  # Minimum length required for filtfilt with order 4
        if phase_data.shape[1] < min_filter_length:
            # Skip filtering for small arrays
            return filtered_data
        
        # Apply Butterworth low-pass filter
        nyquist = 0.5
        cutoff = threshold * nyquist
        
        # Design filter
        b, a = signal.butter(4, cutoff, btype='low')
        
        # Apply filter to each antenna
        for i in range(phase_data.shape[0]):
            filtered_data[i, :] = signal.filtfilt(b, a, phase_data[i, :])
        
        return filtered_data
    
    def sanitize_phase(self, phase_data: np.ndarray) -> np.ndarray:
        """Sanitize phase data through complete pipeline.
        
        Args:
            phase_data: Raw phase data (2D array)
            
        Returns:
            Sanitized phase data
            
        Raises:
            PhaseSanitizationError: If sanitization fails
        """
        try:
            self._total_processed += 1
            
            # Validate input data
            self.validate_phase_data(phase_data)
            
            # Apply complete sanitization pipeline
            sanitized_data = self.unwrap_phase(phase_data)
            sanitized_data = self.remove_outliers(sanitized_data)
            sanitized_data = self.smooth_phase(sanitized_data)
            sanitized_data = self.filter_noise(sanitized_data)
            
            return sanitized_data
            
        except PhaseSanitizationError:
            self._sanitization_errors += 1
            raise
        except Exception as e:
            self._sanitization_errors += 1
            raise PhaseSanitizationError(f"Sanitization pipeline failed: {e}")
    
    def validate_phase_data(self, phase_data: np.ndarray) -> bool:
        """Validate phase data format and values.
        
        Args:
            phase_data: Phase data to validate
            
        Returns:
            True if valid
            
        Raises:
            PhaseSanitizationError: If validation fails
        """
        # Check if data is 2D
        if phase_data.ndim != 2:
            raise PhaseSanitizationError("Phase data must be 2D array")
        
        # Check if data is not empty
        if phase_data.size == 0:
            raise PhaseSanitizationError("Phase data cannot be empty")
        
        # Check if values are within valid range
        min_val, max_val = self.phase_range
        if np.any(phase_data < min_val) or np.any(phase_data > max_val):
            raise PhaseSanitizationError(f"Phase values outside valid range [{min_val}, {max_val}]")
        
        return True
    
    def get_sanitization_statistics(self) -> Dict[str, Any]:
        """Get sanitization statistics.
        
        Returns:
            Dictionary containing sanitization statistics
        """
        outlier_rate = self._outliers_removed / self._total_processed if self._total_processed > 0 else 0
        error_rate = self._sanitization_errors / self._total_processed if self._total_processed > 0 else 0
        
        return {
            'total_processed': self._total_processed,
            'outliers_removed': self._outliers_removed,
            'sanitization_errors': self._sanitization_errors,
            'outlier_rate': outlier_rate,
            'error_rate': error_rate
        }
    
    def reset_statistics(self) -> None:
        """Reset sanitization statistics."""
        self._total_processed = 0
        self._outliers_removed = 0
        self._sanitization_errors = 0