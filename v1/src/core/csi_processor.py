"""CSI data processor for WiFi-DensePose system using TDD approach."""

import asyncio
import logging
import numpy as np
from datetime import datetime, timezone
from typing import Dict, Any, Optional, List
from dataclasses import dataclass
from collections import deque
import scipy.signal
import scipy.fft

try:
    from ..hardware.csi_extractor import CSIData
except ImportError:
    # Handle import for testing
    from src.hardware.csi_extractor import CSIData


class CSIProcessingError(Exception):
    """Exception raised for CSI processing errors."""
    pass


@dataclass
class CSIFeatures:
    """Data structure for extracted CSI features."""
    amplitude_mean: np.ndarray
    amplitude_variance: np.ndarray
    phase_difference: np.ndarray
    correlation_matrix: np.ndarray
    doppler_shift: np.ndarray
    power_spectral_density: np.ndarray
    timestamp: datetime
    metadata: Dict[str, Any]


@dataclass
class HumanDetectionResult:
    """Data structure for human detection results."""
    human_detected: bool
    confidence: float
    motion_score: float
    timestamp: datetime
    features: CSIFeatures
    metadata: Dict[str, Any]


class CSIProcessor:
    """Processes CSI data for human detection and pose estimation."""
    
    def __init__(self, config: Dict[str, Any], logger: Optional[logging.Logger] = None):
        """Initialize CSI processor.
        
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
        self.sampling_rate = config['sampling_rate']
        self.window_size = config['window_size']
        self.overlap = config['overlap']
        self.noise_threshold = config['noise_threshold']
        self.human_detection_threshold = config.get('human_detection_threshold', 0.8)
        self.smoothing_factor = config.get('smoothing_factor', 0.9)
        self.max_history_size = config.get('max_history_size', 500)
        
        # Feature extraction flags
        self.enable_preprocessing = config.get('enable_preprocessing', True)
        self.enable_feature_extraction = config.get('enable_feature_extraction', True)
        self.enable_human_detection = config.get('enable_human_detection', True)
        
        # Processing state
        self.csi_history = deque(maxlen=self.max_history_size)
        self.previous_detection_confidence = 0.0
        
        # Statistics tracking
        self._total_processed = 0
        self._processing_errors = 0
        self._human_detections = 0
    
    def _validate_config(self, config: Dict[str, Any]) -> None:
        """Validate configuration parameters.
        
        Args:
            config: Configuration to validate
            
        Raises:
            ValueError: If configuration is invalid
        """
        required_fields = ['sampling_rate', 'window_size', 'overlap', 'noise_threshold']
        missing_fields = [field for field in required_fields if field not in config]
        
        if missing_fields:
            raise ValueError(f"Missing required configuration: {missing_fields}")
        
        if config['sampling_rate'] <= 0:
            raise ValueError("sampling_rate must be positive")
        
        if config['window_size'] <= 0:
            raise ValueError("window_size must be positive")
        
        if not 0 <= config['overlap'] < 1:
            raise ValueError("overlap must be between 0 and 1")
    
    def preprocess_csi_data(self, csi_data: CSIData) -> CSIData:
        """Preprocess CSI data for feature extraction.
        
        Args:
            csi_data: Raw CSI data
            
        Returns:
            Preprocessed CSI data
            
        Raises:
            CSIProcessingError: If preprocessing fails
        """
        if not self.enable_preprocessing:
            return csi_data
        
        try:
            # Remove noise from the signal
            cleaned_data = self._remove_noise(csi_data)
            
            # Apply windowing function
            windowed_data = self._apply_windowing(cleaned_data)
            
            # Normalize amplitude values
            normalized_data = self._normalize_amplitude(windowed_data)
            
            return normalized_data
            
        except Exception as e:
            raise CSIProcessingError(f"Failed to preprocess CSI data: {e}")
    
    def extract_features(self, csi_data: CSIData) -> Optional[CSIFeatures]:
        """Extract features from CSI data.
        
        Args:
            csi_data: Preprocessed CSI data
            
        Returns:
            Extracted features or None if disabled
            
        Raises:
            CSIProcessingError: If feature extraction fails
        """
        if not self.enable_feature_extraction:
            return None
        
        try:
            # Extract amplitude-based features
            amplitude_mean, amplitude_variance = self._extract_amplitude_features(csi_data)
            
            # Extract phase-based features
            phase_difference = self._extract_phase_features(csi_data)
            
            # Extract correlation features
            correlation_matrix = self._extract_correlation_features(csi_data)
            
            # Extract Doppler and frequency features
            doppler_shift, power_spectral_density = self._extract_doppler_features(csi_data)
            
            return CSIFeatures(
                amplitude_mean=amplitude_mean,
                amplitude_variance=amplitude_variance,
                phase_difference=phase_difference,
                correlation_matrix=correlation_matrix,
                doppler_shift=doppler_shift,
                power_spectral_density=power_spectral_density,
                timestamp=datetime.now(timezone.utc),
                metadata={'processing_params': self.config}
            )
            
        except Exception as e:
            raise CSIProcessingError(f"Failed to extract features: {e}")
    
    def detect_human_presence(self, features: CSIFeatures) -> Optional[HumanDetectionResult]:
        """Detect human presence from CSI features.
        
        Args:
            features: Extracted CSI features
            
        Returns:
            Detection result or None if disabled
            
        Raises:
            CSIProcessingError: If detection fails
        """
        if not self.enable_human_detection:
            return None
        
        try:
            # Analyze motion patterns
            motion_score = self._analyze_motion_patterns(features)
            
            # Calculate detection confidence
            raw_confidence = self._calculate_detection_confidence(features, motion_score)
            
            # Apply temporal smoothing
            smoothed_confidence = self._apply_temporal_smoothing(raw_confidence)
            
            # Determine if human is detected
            human_detected = smoothed_confidence >= self.human_detection_threshold
            
            if human_detected:
                self._human_detections += 1
            
            return HumanDetectionResult(
                human_detected=human_detected,
                confidence=smoothed_confidence,
                motion_score=motion_score,
                timestamp=datetime.now(timezone.utc),
                features=features,
                metadata={'threshold': self.human_detection_threshold}
            )
            
        except Exception as e:
            raise CSIProcessingError(f"Failed to detect human presence: {e}")
    
    async def process_csi_data(self, csi_data: CSIData) -> HumanDetectionResult:
        """Process CSI data through the complete pipeline.
        
        Args:
            csi_data: Raw CSI data
            
        Returns:
            Human detection result
            
        Raises:
            CSIProcessingError: If processing fails
        """
        try:
            self._total_processed += 1
            
            # Preprocess the data
            preprocessed_data = self.preprocess_csi_data(csi_data)
            
            # Extract features
            features = self.extract_features(preprocessed_data)
            
            # Detect human presence
            detection_result = self.detect_human_presence(features)
            
            # Add to history
            self.add_to_history(csi_data)
            
            return detection_result
            
        except Exception as e:
            self._processing_errors += 1
            raise CSIProcessingError(f"Pipeline processing failed: {e}")
    
    def add_to_history(self, csi_data: CSIData) -> None:
        """Add CSI data to processing history.
        
        Args:
            csi_data: CSI data to add to history
        """
        self.csi_history.append(csi_data)
    
    def clear_history(self) -> None:
        """Clear the CSI data history."""
        self.csi_history.clear()
    
    def get_recent_history(self, count: int) -> List[CSIData]:
        """Get recent CSI data from history.
        
        Args:
            count: Number of recent entries to return
            
        Returns:
            List of recent CSI data entries
        """
        if count >= len(self.csi_history):
            return list(self.csi_history)
        else:
            return list(self.csi_history)[-count:]
    
    def get_processing_statistics(self) -> Dict[str, Any]:
        """Get processing statistics.
        
        Returns:
            Dictionary containing processing statistics
        """
        error_rate = self._processing_errors / self._total_processed if self._total_processed > 0 else 0
        detection_rate = self._human_detections / self._total_processed if self._total_processed > 0 else 0
        
        return {
            'total_processed': self._total_processed,
            'processing_errors': self._processing_errors,
            'human_detections': self._human_detections,
            'error_rate': error_rate,
            'detection_rate': detection_rate,
            'history_size': len(self.csi_history)
        }
    
    def reset_statistics(self) -> None:
        """Reset processing statistics."""
        self._total_processed = 0
        self._processing_errors = 0
        self._human_detections = 0
    
    # Private processing methods
    def _remove_noise(self, csi_data: CSIData) -> CSIData:
        """Remove noise from CSI data."""
        # Apply noise filtering based on threshold
        amplitude_db = 20 * np.log10(np.abs(csi_data.amplitude) + 1e-12)
        noise_mask = amplitude_db > self.noise_threshold
        
        filtered_amplitude = csi_data.amplitude.copy()
        filtered_amplitude[~noise_mask] = 0
        
        return CSIData(
            timestamp=csi_data.timestamp,
            amplitude=filtered_amplitude,
            phase=csi_data.phase,
            frequency=csi_data.frequency,
            bandwidth=csi_data.bandwidth,
            num_subcarriers=csi_data.num_subcarriers,
            num_antennas=csi_data.num_antennas,
            snr=csi_data.snr,
            metadata={**csi_data.metadata, 'noise_filtered': True}
        )
    
    def _apply_windowing(self, csi_data: CSIData) -> CSIData:
        """Apply windowing function to CSI data."""
        # Apply Hamming window to reduce spectral leakage
        window = scipy.signal.windows.hamming(csi_data.num_subcarriers)
        windowed_amplitude = csi_data.amplitude * window[np.newaxis, :]
        
        return CSIData(
            timestamp=csi_data.timestamp,
            amplitude=windowed_amplitude,
            phase=csi_data.phase,
            frequency=csi_data.frequency,
            bandwidth=csi_data.bandwidth,
            num_subcarriers=csi_data.num_subcarriers,
            num_antennas=csi_data.num_antennas,
            snr=csi_data.snr,
            metadata={**csi_data.metadata, 'windowed': True}
        )
    
    def _normalize_amplitude(self, csi_data: CSIData) -> CSIData:
        """Normalize amplitude values."""
        # Normalize to unit variance
        normalized_amplitude = csi_data.amplitude / (np.std(csi_data.amplitude) + 1e-12)
        
        return CSIData(
            timestamp=csi_data.timestamp,
            amplitude=normalized_amplitude,
            phase=csi_data.phase,
            frequency=csi_data.frequency,
            bandwidth=csi_data.bandwidth,
            num_subcarriers=csi_data.num_subcarriers,
            num_antennas=csi_data.num_antennas,
            snr=csi_data.snr,
            metadata={**csi_data.metadata, 'normalized': True}
        )
    
    def _extract_amplitude_features(self, csi_data: CSIData) -> tuple:
        """Extract amplitude-based features."""
        amplitude_mean = np.mean(csi_data.amplitude, axis=0)
        amplitude_variance = np.var(csi_data.amplitude, axis=0)
        return amplitude_mean, amplitude_variance
    
    def _extract_phase_features(self, csi_data: CSIData) -> np.ndarray:
        """Extract phase-based features."""
        # Calculate phase differences between adjacent subcarriers
        phase_diff = np.diff(csi_data.phase, axis=1)
        return np.mean(phase_diff, axis=0)
    
    def _extract_correlation_features(self, csi_data: CSIData) -> np.ndarray:
        """Extract correlation features between antennas."""
        # Calculate correlation matrix between antennas
        correlation_matrix = np.corrcoef(csi_data.amplitude)
        return correlation_matrix
    
    def _extract_doppler_features(self, csi_data: CSIData) -> tuple:
        """Extract Doppler and frequency domain features."""
        # Simple Doppler estimation (would use history in real implementation)
        doppler_shift = np.random.rand(10)  # Placeholder
        
        # Power spectral density
        psd = np.abs(scipy.fft.fft(csi_data.amplitude.flatten(), n=128))**2
        
        return doppler_shift, psd
    
    def _analyze_motion_patterns(self, features: CSIFeatures) -> float:
        """Analyze motion patterns from features."""
        # Analyze variance and correlation patterns to detect motion
        variance_score = np.mean(features.amplitude_variance)
        correlation_score = np.mean(np.abs(features.correlation_matrix - np.eye(features.correlation_matrix.shape[0])))
        
        # Combine scores (simplified approach)
        motion_score = 0.6 * variance_score + 0.4 * correlation_score
        return np.clip(motion_score, 0.0, 1.0)
    
    def _calculate_detection_confidence(self, features: CSIFeatures, motion_score: float) -> float:
        """Calculate detection confidence based on features."""
        # Combine multiple feature indicators
        amplitude_indicator = np.mean(features.amplitude_mean) > 0.1
        phase_indicator = np.std(features.phase_difference) > 0.05
        motion_indicator = motion_score > 0.3
        
        # Weight the indicators
        confidence = (0.4 * amplitude_indicator + 0.3 * phase_indicator + 0.3 * motion_indicator)
        return np.clip(confidence, 0.0, 1.0)
    
    def _apply_temporal_smoothing(self, raw_confidence: float) -> float:
        """Apply temporal smoothing to detection confidence."""
        # Exponential moving average
        smoothed_confidence = (self.smoothing_factor * self.previous_detection_confidence + 
                             (1 - self.smoothing_factor) * raw_confidence)
        
        self.previous_detection_confidence = smoothed_confidence
        return smoothed_confidence