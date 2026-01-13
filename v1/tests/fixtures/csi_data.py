"""
Test data generation utilities for CSI data.

Provides realistic CSI data samples for testing pose estimation pipeline.
"""

import numpy as np
from datetime import datetime, timedelta
from typing import Dict, Any, List, Optional, Tuple
import json
import random


class CSIDataGenerator:
    """Generate realistic CSI data for testing."""
    
    def __init__(self, 
                 frequency: float = 5.8e9,
                 bandwidth: float = 80e6,
                 num_antennas: int = 4,
                 num_subcarriers: int = 64):
        self.frequency = frequency
        self.bandwidth = bandwidth
        self.num_antennas = num_antennas
        self.num_subcarriers = num_subcarriers
        self.sample_rate = 1000  # Hz
        self.noise_level = 0.1
        
        # Pre-computed patterns for different scenarios
        self._initialize_patterns()
    
    def _initialize_patterns(self):
        """Initialize CSI patterns for different scenarios."""
        # Empty room pattern (baseline)
        self.empty_room_pattern = {
            "amplitude_mean": 0.3,
            "amplitude_std": 0.05,
            "phase_variance": 0.1,
            "temporal_stability": 0.95
        }
        
        # Single person patterns
        self.single_person_patterns = {
            "standing": {
                "amplitude_mean": 0.5,
                "amplitude_std": 0.08,
                "phase_variance": 0.2,
                "temporal_stability": 0.85,
                "movement_frequency": 0.1
            },
            "walking": {
                "amplitude_mean": 0.6,
                "amplitude_std": 0.15,
                "phase_variance": 0.4,
                "temporal_stability": 0.6,
                "movement_frequency": 2.0
            },
            "sitting": {
                "amplitude_mean": 0.4,
                "amplitude_std": 0.06,
                "phase_variance": 0.15,
                "temporal_stability": 0.9,
                "movement_frequency": 0.05
            },
            "fallen": {
                "amplitude_mean": 0.35,
                "amplitude_std": 0.04,
                "phase_variance": 0.08,
                "temporal_stability": 0.95,
                "movement_frequency": 0.02
            }
        }
        
        # Multi-person patterns
        self.multi_person_patterns = {
            2: {"amplitude_multiplier": 1.4, "phase_complexity": 1.6},
            3: {"amplitude_multiplier": 1.7, "phase_complexity": 2.1},
            4: {"amplitude_multiplier": 2.0, "phase_complexity": 2.8}
        }
    
    def generate_empty_room_sample(self, timestamp: Optional[datetime] = None) -> Dict[str, Any]:
        """Generate CSI sample for empty room."""
        if timestamp is None:
            timestamp = datetime.utcnow()
        
        pattern = self.empty_room_pattern
        
        # Generate amplitude matrix
        amplitude = np.random.normal(
            pattern["amplitude_mean"],
            pattern["amplitude_std"],
            (self.num_antennas, self.num_subcarriers)
        )
        amplitude = np.clip(amplitude, 0, 1)
        
        # Generate phase matrix
        phase = np.random.uniform(
            -np.pi, np.pi,
            (self.num_antennas, self.num_subcarriers)
        )
        
        # Add temporal stability
        if hasattr(self, '_last_empty_sample'):
            stability = pattern["temporal_stability"]
            amplitude = stability * self._last_empty_sample["amplitude"] + (1 - stability) * amplitude
            phase = stability * self._last_empty_sample["phase"] + (1 - stability) * phase
        
        sample = {
            "timestamp": timestamp.isoformat(),
            "router_id": "router_001",
            "amplitude": amplitude.tolist(),
            "phase": phase.tolist(),
            "frequency": self.frequency,
            "bandwidth": self.bandwidth,
            "num_antennas": self.num_antennas,
            "num_subcarriers": self.num_subcarriers,
            "sample_rate": self.sample_rate,
            "scenario": "empty_room",
            "signal_quality": np.random.uniform(0.85, 0.95)
        }
        
        self._last_empty_sample = {
            "amplitude": amplitude,
            "phase": phase
        }
        
        return sample
    
    def generate_single_person_sample(self, 
                                    activity: str = "standing",
                                    timestamp: Optional[datetime] = None) -> Dict[str, Any]:
        """Generate CSI sample for single person activity."""
        if timestamp is None:
            timestamp = datetime.utcnow()
        
        if activity not in self.single_person_patterns:
            raise ValueError(f"Unknown activity: {activity}")
        
        pattern = self.single_person_patterns[activity]
        
        # Generate base amplitude
        amplitude = np.random.normal(
            pattern["amplitude_mean"],
            pattern["amplitude_std"],
            (self.num_antennas, self.num_subcarriers)
        )
        
        # Add movement-induced variations
        movement_freq = pattern["movement_frequency"]
        time_factor = timestamp.timestamp()
        movement_modulation = 0.1 * np.sin(2 * np.pi * movement_freq * time_factor)
        amplitude += movement_modulation
        amplitude = np.clip(amplitude, 0, 1)
        
        # Generate phase with activity-specific variance
        phase_base = np.random.uniform(-np.pi, np.pi, (self.num_antennas, self.num_subcarriers))
        phase_variance = pattern["phase_variance"]
        phase_noise = np.random.normal(0, phase_variance, (self.num_antennas, self.num_subcarriers))
        phase = phase_base + phase_noise
        phase = np.mod(phase + np.pi, 2 * np.pi) - np.pi  # Wrap to [-π, π]
        
        # Add temporal correlation
        if hasattr(self, f'_last_{activity}_sample'):
            stability = pattern["temporal_stability"]
            last_sample = getattr(self, f'_last_{activity}_sample')
            amplitude = stability * last_sample["amplitude"] + (1 - stability) * amplitude
            phase = stability * last_sample["phase"] + (1 - stability) * phase
        
        sample = {
            "timestamp": timestamp.isoformat(),
            "router_id": "router_001",
            "amplitude": amplitude.tolist(),
            "phase": phase.tolist(),
            "frequency": self.frequency,
            "bandwidth": self.bandwidth,
            "num_antennas": self.num_antennas,
            "num_subcarriers": self.num_subcarriers,
            "sample_rate": self.sample_rate,
            "scenario": f"single_person_{activity}",
            "signal_quality": np.random.uniform(0.7, 0.9),
            "activity": activity
        }
        
        setattr(self, f'_last_{activity}_sample', {
            "amplitude": amplitude,
            "phase": phase
        })
        
        return sample
    
    def generate_multi_person_sample(self, 
                                   num_persons: int = 2,
                                   activities: Optional[List[str]] = None,
                                   timestamp: Optional[datetime] = None) -> Dict[str, Any]:
        """Generate CSI sample for multiple persons."""
        if timestamp is None:
            timestamp = datetime.utcnow()
        
        if num_persons < 2 or num_persons > 4:
            raise ValueError("Number of persons must be between 2 and 4")
        
        if activities is None:
            activities = random.choices(list(self.single_person_patterns.keys()), k=num_persons)
        
        if len(activities) != num_persons:
            raise ValueError("Number of activities must match number of persons")
        
        # Start with empty room baseline
        amplitude = np.random.normal(
            self.empty_room_pattern["amplitude_mean"],
            self.empty_room_pattern["amplitude_std"],
            (self.num_antennas, self.num_subcarriers)
        )
        
        phase = np.random.uniform(
            -np.pi, np.pi,
            (self.num_antennas, self.num_subcarriers)
        )
        
        # Add contribution from each person
        for i, activity in enumerate(activities):
            person_pattern = self.single_person_patterns[activity]
            
            # Generate person-specific contribution
            person_amplitude = np.random.normal(
                person_pattern["amplitude_mean"] * 0.7,  # Reduced for multi-person
                person_pattern["amplitude_std"],
                (self.num_antennas, self.num_subcarriers)
            )
            
            # Add spatial variation (different persons at different locations)
            spatial_offset = i * self.num_subcarriers // num_persons
            person_amplitude = np.roll(person_amplitude, spatial_offset, axis=1)
            
            # Add movement modulation
            movement_freq = person_pattern["movement_frequency"]
            time_factor = timestamp.timestamp() + i * 0.5  # Phase offset between persons
            movement_modulation = 0.05 * np.sin(2 * np.pi * movement_freq * time_factor)
            person_amplitude += movement_modulation
            
            amplitude += person_amplitude
            
            # Add phase contribution
            person_phase = np.random.normal(0, person_pattern["phase_variance"], 
                                         (self.num_antennas, self.num_subcarriers))
            person_phase = np.roll(person_phase, spatial_offset, axis=1)
            phase += person_phase
        
        # Apply multi-person complexity
        pattern = self.multi_person_patterns[num_persons]
        amplitude *= pattern["amplitude_multiplier"]
        phase *= pattern["phase_complexity"]
        
        # Clip and normalize
        amplitude = np.clip(amplitude, 0, 1)
        phase = np.mod(phase + np.pi, 2 * np.pi) - np.pi
        
        sample = {
            "timestamp": timestamp.isoformat(),
            "router_id": "router_001",
            "amplitude": amplitude.tolist(),
            "phase": phase.tolist(),
            "frequency": self.frequency,
            "bandwidth": self.bandwidth,
            "num_antennas": self.num_antennas,
            "num_subcarriers": self.num_subcarriers,
            "sample_rate": self.sample_rate,
            "scenario": f"multi_person_{num_persons}",
            "signal_quality": np.random.uniform(0.6, 0.8),
            "num_persons": num_persons,
            "activities": activities
        }
        
        return sample
    
    def generate_time_series(self, 
                           duration_seconds: int = 10,
                           scenario: str = "single_person_walking",
                           **kwargs) -> List[Dict[str, Any]]:
        """Generate time series of CSI samples."""
        samples = []
        start_time = datetime.utcnow()
        
        for i in range(duration_seconds * self.sample_rate):
            timestamp = start_time + timedelta(seconds=i / self.sample_rate)
            
            if scenario == "empty_room":
                sample = self.generate_empty_room_sample(timestamp)
            elif scenario.startswith("single_person_"):
                activity = scenario.replace("single_person_", "")
                sample = self.generate_single_person_sample(activity, timestamp)
            elif scenario.startswith("multi_person_"):
                num_persons = int(scenario.split("_")[-1])
                sample = self.generate_multi_person_sample(num_persons, timestamp=timestamp, **kwargs)
            else:
                raise ValueError(f"Unknown scenario: {scenario}")
            
            samples.append(sample)
        
        return samples
    
    def add_noise(self, sample: Dict[str, Any], noise_level: Optional[float] = None) -> Dict[str, Any]:
        """Add noise to CSI sample."""
        if noise_level is None:
            noise_level = self.noise_level
        
        noisy_sample = sample.copy()
        
        # Add amplitude noise
        amplitude = np.array(sample["amplitude"])
        amplitude_noise = np.random.normal(0, noise_level, amplitude.shape)
        noisy_amplitude = amplitude + amplitude_noise
        noisy_amplitude = np.clip(noisy_amplitude, 0, 1)
        noisy_sample["amplitude"] = noisy_amplitude.tolist()
        
        # Add phase noise
        phase = np.array(sample["phase"])
        phase_noise = np.random.normal(0, noise_level * np.pi, phase.shape)
        noisy_phase = phase + phase_noise
        noisy_phase = np.mod(noisy_phase + np.pi, 2 * np.pi) - np.pi
        noisy_sample["phase"] = noisy_phase.tolist()
        
        # Reduce signal quality
        noisy_sample["signal_quality"] *= (1 - noise_level)
        
        return noisy_sample
    
    def simulate_hardware_artifacts(self, sample: Dict[str, Any]) -> Dict[str, Any]:
        """Simulate hardware-specific artifacts."""
        artifact_sample = sample.copy()
        
        amplitude = np.array(sample["amplitude"])
        phase = np.array(sample["phase"])
        
        # Simulate antenna coupling
        coupling_matrix = np.random.uniform(0.95, 1.05, (self.num_antennas, self.num_antennas))
        amplitude = coupling_matrix @ amplitude
        
        # Simulate frequency-dependent gain variations
        freq_response = 1 + 0.1 * np.sin(np.linspace(0, 2*np.pi, self.num_subcarriers))
        amplitude *= freq_response[np.newaxis, :]
        
        # Simulate phase drift
        phase_drift = np.random.uniform(-0.1, 0.1) * np.arange(self.num_subcarriers)
        phase += phase_drift[np.newaxis, :]
        
        # Clip and wrap
        amplitude = np.clip(amplitude, 0, 1)
        phase = np.mod(phase + np.pi, 2 * np.pi) - np.pi
        
        artifact_sample["amplitude"] = amplitude.tolist()
        artifact_sample["phase"] = phase.tolist()
        
        return artifact_sample


# Convenience functions for common test scenarios
def generate_fall_detection_sequence() -> List[Dict[str, Any]]:
    """Generate CSI sequence showing fall detection scenario."""
    generator = CSIDataGenerator()
    
    sequence = []
    
    # Normal standing (5 seconds)
    sequence.extend(generator.generate_time_series(5, "single_person_standing"))
    
    # Walking (3 seconds)
    sequence.extend(generator.generate_time_series(3, "single_person_walking"))
    
    # Fall event (1 second transition)
    sequence.extend(generator.generate_time_series(1, "single_person_fallen"))
    
    # Fallen state (3 seconds)
    sequence.extend(generator.generate_time_series(3, "single_person_fallen"))
    
    return sequence


def generate_multi_person_scenario() -> List[Dict[str, Any]]:
    """Generate CSI sequence for multi-person scenario."""
    generator = CSIDataGenerator()
    
    sequence = []
    
    # Start with empty room
    sequence.extend(generator.generate_time_series(2, "empty_room"))
    
    # One person enters
    sequence.extend(generator.generate_time_series(3, "single_person_walking"))
    
    # Second person enters
    sequence.extend(generator.generate_time_series(5, "multi_person_2", 
                                                 activities=["standing", "walking"]))
    
    # Third person enters
    sequence.extend(generator.generate_time_series(4, "multi_person_3",
                                                 activities=["standing", "walking", "sitting"]))
    
    return sequence


def generate_noisy_environment_data() -> List[Dict[str, Any]]:
    """Generate CSI data with various noise levels."""
    generator = CSIDataGenerator()
    
    # Generate clean data
    clean_samples = generator.generate_time_series(5, "single_person_walking")
    
    # Add different noise levels
    noisy_samples = []
    noise_levels = [0.05, 0.1, 0.2, 0.3]
    
    for noise_level in noise_levels:
        for sample in clean_samples[:10]:  # Take first 10 samples
            noisy_sample = generator.add_noise(sample, noise_level)
            noisy_samples.append(noisy_sample)
    
    return noisy_samples


def generate_hardware_test_data() -> List[Dict[str, Any]]:
    """Generate CSI data with hardware artifacts."""
    generator = CSIDataGenerator()
    
    # Generate base samples
    base_samples = generator.generate_time_series(3, "single_person_standing")
    
    # Add hardware artifacts
    artifact_samples = []
    for sample in base_samples:
        artifact_sample = generator.simulate_hardware_artifacts(sample)
        artifact_samples.append(artifact_sample)
    
    return artifact_samples


# Test data validation utilities
def validate_csi_sample(sample: Dict[str, Any]) -> bool:
    """Validate CSI sample structure and data ranges."""
    required_fields = [
        "timestamp", "router_id", "amplitude", "phase",
        "frequency", "bandwidth", "num_antennas", "num_subcarriers"
    ]
    
    # Check required fields
    for field in required_fields:
        if field not in sample:
            return False
    
    # Validate data types and ranges
    amplitude = np.array(sample["amplitude"])
    phase = np.array(sample["phase"])
    
    # Check shapes
    expected_shape = (sample["num_antennas"], sample["num_subcarriers"])
    if amplitude.shape != expected_shape or phase.shape != expected_shape:
        return False
    
    # Check value ranges
    if not (0 <= amplitude.min() and amplitude.max() <= 1):
        return False
    
    if not (-np.pi <= phase.min() and phase.max() <= np.pi):
        return False
    
    return True


def extract_features_from_csi(sample: Dict[str, Any]) -> Dict[str, Any]:
    """Extract features from CSI sample for testing."""
    amplitude = np.array(sample["amplitude"])
    phase = np.array(sample["phase"])
    
    features = {
        "amplitude_mean": float(np.mean(amplitude)),
        "amplitude_std": float(np.std(amplitude)),
        "amplitude_max": float(np.max(amplitude)),
        "amplitude_min": float(np.min(amplitude)),
        "phase_variance": float(np.var(phase)),
        "phase_range": float(np.max(phase) - np.min(phase)),
        "signal_energy": float(np.sum(amplitude ** 2)),
        "phase_coherence": float(np.abs(np.mean(np.exp(1j * phase)))),
        "spatial_correlation": float(np.mean(np.corrcoef(amplitude))),
        "frequency_diversity": float(np.std(np.mean(amplitude, axis=0)))
    }
    
    return features