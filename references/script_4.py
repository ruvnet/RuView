# WiFi DensePose Implementation - Core Architecture (NumPy-based prototype)
# Based on "DensePose From WiFi" by Carnegie Mellon University

import numpy as np
import math
from typing import Dict, List, Tuple, Optional
from collections import OrderedDict
import json

class CSIPhaseProcessor:
    """
    Processes raw CSI phase data through unwrapping, filtering, and linear fitting
    Based on the phase sanitization methodology from the paper
    """
    
    def __init__(self, num_subcarriers: int = 30):
        self.num_subcarriers = num_subcarriers
        print(f"Initialized CSI Phase Processor with {num_subcarriers} subcarriers")
    
    def unwrap_phase(self, phase_data: np.ndarray) -> np.ndarray:
        """
        Unwrap phase values to handle discontinuities
        """
        unwrapped = np.copy(phase_data)
        
        for i in range(1, phase_data.shape[1]):  # Along frequency dimension
            diff = unwrapped[:, i] - unwrapped[:, i-1]
            
            # Apply unwrapping logic
            unwrapped[:, i] = np.where(diff > np.pi, 
                                     unwrapped[:, i-1] + diff - 2*np.pi,
                                     unwrapped[:, i])
            unwrapped[:, i] = np.where(diff < -np.pi,
                                     unwrapped[:, i-1] + diff + 2*np.pi,
                                     unwrapped[:, i])
        
        return unwrapped
    
    def apply_filters(self, phase_data: np.ndarray) -> np.ndarray:
        """
        Apply median and uniform filters to eliminate outliers
        """
        filtered = np.copy(phase_data)
        
        # Apply simple smoothing in time dimension
        for i in range(1, phase_data.shape[0]-1):
            filtered[i] = (phase_data[i-1] + phase_data[i] + phase_data[i+1]) / 3
        
        # Apply smoothing in frequency dimension
        for i in range(1, phase_data.shape[1]-1):
            filtered[:, i] = (filtered[:, i-1] + filtered[:, i] + filtered[:, i+1]) / 3
        
        return filtered
    
    def linear_fitting(self, phase_data: np.ndarray) -> np.ndarray:
        """
        Apply linear fitting to remove systematic phase drift
        """
        fitted_data = np.copy(phase_data)
        F = self.num_subcarriers
        
        for sample_idx in range(phase_data.shape[0]):
            for ant_i in range(phase_data.shape[2]):
                for ant_j in range(phase_data.shape[3]):
                    phase_seq = phase_data[sample_idx, :, ant_i, ant_j]
                    
                    # Calculate linear coefficients
                    alpha1 = (phase_seq[-1] - phase_seq[0]) / (2 * np.pi * F)
                    alpha0 = np.mean(phase_seq)
                    
                    # Apply linear fitting
                    frequencies = np.arange(1, F + 1)
                    linear_trend = alpha1 * frequencies + alpha0
                    fitted_data[sample_idx, :, ant_i, ant_j] = phase_seq - linear_trend
        
        return fitted_data
    
    def sanitize_phase(self, raw_phase: np.ndarray) -> np.ndarray:
        """
        Complete phase sanitization pipeline
        """
        print("Sanitizing CSI phase data...")
        print(f"Input shape: {raw_phase.shape}")
        
        # Step 1: Unwrap phase
        unwrapped = self.unwrap_phase(raw_phase)
        print("✓ Phase unwrapping completed")
        
        # Step 2: Apply filters
        filtered = self.apply_filters(unwrapped)
        print("✓ Filtering completed")
        
        # Step 3: Linear fitting
        sanitized = self.linear_fitting(filtered)
        print("✓ Linear fitting completed")
        
        return sanitized

class WiFiDensePoseConfig:
    """
    Configuration class for WiFi DensePose system
    """
    def __init__(self):
        # Hardware configuration
        self.num_transmitters = 3
        self.num_receivers = 3
        self.num_subcarriers = 30
        self.sampling_rate = 100  # Hz
        self.consecutive_samples = 5
        
        # Network configuration
        self.input_amplitude_shape = (150, 3, 3)  # 5 samples * 30 frequencies, 3x3 antennas
        self.input_phase_shape = (150, 3, 3)
        self.output_feature_shape = (3, 720, 1280)  # Image-like feature map
        
        # DensePose configuration
        self.num_body_parts = 24
        self.num_keypoints = 17
        self.keypoint_heatmap_size = (56, 56)
        self.uv_map_size = (112, 112)
        
        # Training configuration
        self.learning_rate = 1e-3
        self.batch_size = 16
        self.num_epochs = 145000
        self.lambda_dp = 0.6  # DensePose loss weight
        self.lambda_kp = 0.3  # Keypoint loss weight
        self.lambda_tr = 0.1  # Transfer learning loss weight

class WiFiDataSimulator:
    """
    Simulates WiFi CSI data for demonstration purposes
    """
    
    def __init__(self, config: WiFiDensePoseConfig):
        self.config = config
        np.random.seed(42)  # For reproducibility
    
    def generate_csi_sample(self, num_people: int = 1, movement_intensity: float = 1.0) -> Tuple[np.ndarray, np.ndarray]:
        """
        Generate simulated CSI amplitude and phase data
        """
        # Base CSI signal (environment)
        amplitude = np.ones(self.config.input_amplitude_shape) * 50  # Base signal strength
        phase = np.zeros(self.config.input_phase_shape)
        
        # Add noise
        amplitude += np.random.normal(0, 5, self.config.input_amplitude_shape)
        phase += np.random.normal(0, 0.1, self.config.input_phase_shape)
        
        # Simulate human presence effects
        for person in range(num_people):
            # Random position effects
            pos_x = np.random.uniform(0.2, 0.8)
            pos_y = np.random.uniform(0.2, 0.8)
            
            # Create interference patterns
            for tx in range(3):
                for rx in range(3):
                    # Distance-based attenuation
                    distance = np.sqrt((tx/2 - pos_x)**2 + (rx/2 - pos_y)**2)
                    attenuation = movement_intensity * np.exp(-distance * 2)
                    
                    # Frequency-dependent effects
                    for freq in range(30):
                        freq_effect = np.sin(2 * np.pi * freq / 30 + person * np.pi/2)
                        
                        # Amplitude effects
                        for sample in range(5):
                            sample_idx = sample * 30 + freq
                            amplitude[sample_idx, tx, rx] *= (1 - attenuation * 0.3 * freq_effect)
                        
                        # Phase effects
                        for sample in range(5):
                            sample_idx = sample * 30 + freq
                            phase[sample_idx, tx, rx] += attenuation * freq_effect * movement_intensity
        
        return amplitude, phase
    
    def generate_ground_truth_poses(self, num_people: int = 1) -> Dict:
        """
        Generate simulated ground truth pose data
        """
        poses = []
        for person in range(num_people):
            # Simulate a person's bounding box
            x = np.random.uniform(100, 620)  # Within 720px width
            y = np.random.uniform(100, 1180)  # Within 1280px height
            w = np.random.uniform(80, 200)
            h = np.random.uniform(150, 400)
            
            # Simulate keypoints (17 COCO keypoints)
            keypoints = []
            for kp in range(17):
                kp_x = x + np.random.uniform(-w/4, w/4)
                kp_y = y + np.random.uniform(-h/4, h/4)
                confidence = np.random.uniform(0.7, 1.0)
                keypoints.extend([kp_x, kp_y, confidence])
            
            poses.append({
                'bbox': [x, y, w, h],
                'keypoints': keypoints,
                'person_id': person
            })
        
        return {'poses': poses, 'num_people': num_people}

# Initialize the system
config = WiFiDensePoseConfig()
phase_processor = CSIPhaseProcessor(config.num_subcarriers)
data_simulator = WiFiDataSimulator(config)

print("WiFi DensePose System Initialized!")
print(f"Configuration:")
print(f"  - Hardware: {config.num_transmitters}x{config.num_receivers} antenna array")
print(f"  - Frequencies: {config.num_subcarriers} subcarriers at 2.4GHz")
print(f"  - Sampling: {config.sampling_rate}Hz")
print(f"  - Body parts: {config.num_body_parts}")
print(f"  - Keypoints: {config.num_keypoints}")

# Demonstrate CSI data processing
print("\n" + "="*60)
print("DEMONSTRATING CSI DATA PROCESSING")
print("="*60)

# Generate sample CSI data
amplitude_data, phase_data = data_simulator.generate_csi_sample(num_people=2, movement_intensity=1.5)
print(f"Generated CSI data:")
print(f"  Amplitude shape: {amplitude_data.shape}")
print(f"  Phase shape: {phase_data.shape}")
print(f"  Amplitude range: [{amplitude_data.min():.2f}, {amplitude_data.max():.2f}]")
print(f"  Phase range: [{phase_data.min():.2f}, {phase_data.max():.2f}]")

# Process phase data
sanitized_phase = phase_processor.sanitize_phase(phase_data)
print(f"Sanitized phase range: [{sanitized_phase.min():.2f}, {sanitized_phase.max():.2f}]")

# Generate ground truth
ground_truth = data_simulator.generate_ground_truth_poses(num_people=2)
print(f"\nGenerated ground truth for {ground_truth['num_people']} people")
for i, pose in enumerate(ground_truth['poses']):
    bbox = pose['bbox']
    print(f"  Person {i+1}: bbox=[{bbox[0]:.1f}, {bbox[1]:.1f}, {bbox[2]:.1f}, {bbox[3]:.1f}]")

print("\nCSI processing demonstration completed!")