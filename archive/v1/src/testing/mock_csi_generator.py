"""
Mock CSI data generator for testing and development.

This module provides synthetic CSI (Channel State Information) data generation
for use in development and testing environments ONLY. The generated data mimics
realistic WiFi CSI patterns including multipath effects, human motion signatures,
and noise characteristics.

WARNING: This module uses np.random intentionally for test data generation.
Do NOT use this module in production data paths.
"""

import logging
import numpy as np
from typing import Dict, Any, Optional

logger = logging.getLogger(__name__)

# Banner displayed when mock mode is active
MOCK_MODE_BANNER = """
================================================================================
  WARNING: MOCK MODE ACTIVE - Using synthetic CSI data

  All CSI data is randomly generated and does NOT represent real WiFi signals.
  For real pose estimation, configure hardware per docs/hardware-setup.md.
================================================================================
"""


class MockCSIGenerator:
    """Generator for synthetic CSI data used in testing and development.

    This class produces complex-valued CSI matrices that simulate realistic
    WiFi channel characteristics including:
    - Per-antenna and per-subcarrier amplitude/phase variation
    - Simulated human movement signatures
    - Configurable noise levels
    - Temporal coherence across consecutive frames

    This is ONLY for testing. Production code must use real hardware data.
    """

    def __init__(
        self,
        num_subcarriers: int = 64,
        num_antennas: int = 4,
        num_samples: int = 100,
        noise_level: float = 0.1,
        movement_freq: float = 0.5,
        movement_amplitude: float = 0.3,
    ):
        """Initialize mock CSI generator.

        Args:
            num_subcarriers: Number of OFDM subcarriers to simulate
            num_antennas: Number of antenna elements
            num_samples: Number of temporal samples per frame
            noise_level: Standard deviation of additive Gaussian noise
            movement_freq: Frequency of simulated human movement (Hz)
            movement_amplitude: Amplitude of movement-induced CSI variation
        """
        self.num_subcarriers = num_subcarriers
        self.num_antennas = num_antennas
        self.num_samples = num_samples
        self.noise_level = noise_level
        self.movement_freq = movement_freq
        self.movement_amplitude = movement_amplitude

        # Internal state for temporal coherence
        self._phase = 0.0
        self._frequency = 0.1
        self._amplitude_base = 1.0

        self._banner_shown = False

    def show_banner(self) -> None:
        """Display the mock mode warning banner (once per session)."""
        if not self._banner_shown:
            logger.warning(MOCK_MODE_BANNER)
            self._banner_shown = True

    def generate(self) -> np.ndarray:
        """Generate a single frame of mock CSI data.

        Returns:
            Complex-valued numpy array of shape
            (num_antennas, num_subcarriers, num_samples).
        """
        self.show_banner()

        # Advance internal phase for temporal coherence
        self._phase += self._frequency

        time_axis = np.linspace(0, 1, self.num_samples)

        csi_data = np.zeros(
            (self.num_antennas, self.num_subcarriers, self.num_samples),
            dtype=complex,
        )

        for antenna in range(self.num_antennas):
            for subcarrier in range(self.num_subcarriers):
                # Base amplitude varies with antenna and subcarrier
                amplitude = (
                    self._amplitude_base
                    * (1 + 0.2 * np.sin(2 * np.pi * subcarrier / self.num_subcarriers))
                    * (1 + 0.1 * antenna)
                )

                # Phase with spatial and frequency variation
                phase_offset = (
                    self._phase
                    + 2 * np.pi * subcarrier / self.num_subcarriers
                    + np.pi * antenna / self.num_antennas
                )

                # Simulated human movement
                movement = self.movement_amplitude * np.sin(
                    2 * np.pi * self.movement_freq * time_axis
                )

                signal_amplitude = amplitude * (1 + movement)
                signal_phase = phase_offset + movement * 0.5

                # Additive complex Gaussian noise
                noise = np.random.normal(0, self.noise_level, self.num_samples) + 1j * np.random.normal(
                    0, self.noise_level, self.num_samples
                )

                csi_data[antenna, subcarrier, :] = (
                    signal_amplitude * np.exp(1j * signal_phase) + noise
                )

        return csi_data

    def configure(self, config: Dict[str, Any]) -> None:
        """Update generator parameters.

        Args:
            config: Dictionary with optional keys:
                - sampling_rate: Adjusts internal frequency
                - noise_level: Sets noise standard deviation
                - num_subcarriers: Updates subcarrier count
                - num_antennas: Updates antenna count
                - movement_freq: Updates simulated movement frequency
                - movement_amplitude: Updates movement amplitude
        """
        if "sampling_rate" in config:
            self._frequency = config["sampling_rate"] / 1000.0
        if "noise_level" in config:
            self.noise_level = config["noise_level"]
        if "num_subcarriers" in config:
            self.num_subcarriers = config["num_subcarriers"]
        if "num_antennas" in config:
            self.num_antennas = config["num_antennas"]
        if "movement_freq" in config:
            self.movement_freq = config["movement_freq"]
        if "movement_amplitude" in config:
            self.movement_amplitude = config["movement_amplitude"]

    def get_router_info(self) -> Dict[str, Any]:
        """Return mock router hardware information.

        Returns:
            Dictionary mimicking router hardware info for testing.
        """
        return {
            "model": "Mock Router",
            "firmware": "1.0.0-mock",
            "wifi_standard": "802.11ac",
            "antennas": self.num_antennas,
            "supported_bands": ["2.4GHz", "5GHz"],
            "csi_capabilities": {
                "max_subcarriers": self.num_subcarriers,
                "max_antennas": self.num_antennas,
                "sampling_rate": 1000,
            },
        }
