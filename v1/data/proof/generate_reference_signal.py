#!/usr/bin/env python3
"""
Deterministic Reference CSI Signal Generator for WiFi-DensePose Proof Bundle.

This script generates a SYNTHETIC, DETERMINISTIC CSI (Channel State Information)
reference signal for pipeline verification. It is NOT a real WiFi capture.

The signal models a 3-antenna, 56-subcarrier WiFi system with:
  - Human breathing modulation at 0.3 Hz
  - Walking motion modulation at 1.2 Hz
  - Structured (deterministic) multipath propagation with known delays
  - 10 seconds of data at 100 Hz sampling rate (1000 frames total)

Generation Formula
==================

For each frame t (t = 0..999) at time s = t / 100.0:

  CSI[antenna_a, subcarrier_k] = sum over P paths of:
      A_p * exp(j * (2*pi*f_k*tau_p + phi_p,a))
      * (1 + alpha_breathe * sin(2*pi * 0.3 * s + psi_breathe_a))
      * (1 + alpha_walk   * sin(2*pi * 1.2 * s + psi_walk_a))

Where:
  - f_k = center_freq + (k - 28) * subcarrier_spacing  [subcarrier frequency]
  - tau_p = deterministic path delay for path p
  - A_p = deterministic path amplitude for path p
  - phi_p,a = deterministic phase offset per path per antenna
  - alpha_breathe = 0.02 (breathing modulation depth)
  - alpha_walk = 0.08 (walking modulation depth)
  - psi_breathe_a, psi_walk_a = deterministic per-antenna phase offsets

All parameters are computed from numpy with seed=42. No randomness is used
at generation time -- the seed is used ONLY to select fixed parameter values
once, which are then documented in the metadata file.

Output:
  - sample_csi_data.json: All 1000 CSI frames with amplitude and phase arrays
  - sample_csi_meta.json: Complete parameter documentation

Author: WiFi-DensePose Project (synthetic test data)
"""

import json
import os
import sys

import numpy as np


def generate_deterministic_parameters():
    """Generate all fixed parameters using seed=42.

    These parameters define the multipath channel model and human motion
    modulation. Once generated, they are constants -- no further randomness
    is used.

    Returns:
        dict: All channel and motion parameters.
    """
    rng = np.random.RandomState(42)

    # System parameters (fixed by design, not random)
    num_antennas = 3
    num_subcarriers = 56
    sampling_rate_hz = 100
    duration_s = 10.0
    center_freq_hz = 5.21e9  # WiFi 5 GHz channel 42
    subcarrier_spacing_hz = 312.5e3  # Standard 802.11n/ac

    # Multipath channel: 5 deterministic paths
    num_paths = 5
    # Path delays in nanoseconds (typical indoor)
    path_delays_ns = np.array([0.0, 15.0, 42.0, 78.0, 120.0])
    # Path amplitudes (linear scale, decreasing with delay)
    path_amplitudes = np.array([1.0, 0.6, 0.35, 0.18, 0.08])
    # Phase offsets per path per antenna (from seed=42, then fixed)
    path_phase_offsets = rng.uniform(-np.pi, np.pi, size=(num_paths, num_antennas))

    # Human motion modulation parameters
    breathing_freq_hz = 0.3
    walking_freq_hz = 1.2
    breathing_depth = 0.02  # 2% amplitude modulation
    walking_depth = 0.08    # 8% amplitude modulation

    # Per-antenna phase offsets for motion signals (from seed=42, then fixed)
    breathing_phase_offsets = rng.uniform(0, 2 * np.pi, size=num_antennas)
    walking_phase_offsets = rng.uniform(0, 2 * np.pi, size=num_antennas)

    return {
        "num_antennas": num_antennas,
        "num_subcarriers": num_subcarriers,
        "sampling_rate_hz": sampling_rate_hz,
        "duration_s": duration_s,
        "center_freq_hz": center_freq_hz,
        "subcarrier_spacing_hz": subcarrier_spacing_hz,
        "num_paths": num_paths,
        "path_delays_ns": path_delays_ns,
        "path_amplitudes": path_amplitudes,
        "path_phase_offsets": path_phase_offsets,
        "breathing_freq_hz": breathing_freq_hz,
        "walking_freq_hz": walking_freq_hz,
        "breathing_depth": breathing_depth,
        "walking_depth": walking_depth,
        "breathing_phase_offsets": breathing_phase_offsets,
        "walking_phase_offsets": walking_phase_offsets,
    }


def generate_csi_frames(params):
    """Generate all CSI frames deterministically from the given parameters.

    Args:
        params: Dictionary of channel/motion parameters.

    Returns:
        list: List of dicts, each containing amplitude and phase arrays
              for one frame, plus timestamp.
    """
    num_antennas = params["num_antennas"]
    num_subcarriers = params["num_subcarriers"]
    sampling_rate = params["sampling_rate_hz"]
    duration = params["duration_s"]
    center_freq = params["center_freq_hz"]
    subcarrier_spacing = params["subcarrier_spacing_hz"]
    num_paths = params["num_paths"]
    path_delays_ns = params["path_delays_ns"]
    path_amplitudes = params["path_amplitudes"]
    path_phase_offsets = params["path_phase_offsets"]
    breathing_freq = params["breathing_freq_hz"]
    walking_freq = params["walking_freq_hz"]
    breathing_depth = params["breathing_depth"]
    walking_depth = params["walking_depth"]
    breathing_phase = params["breathing_phase_offsets"]
    walking_phase = params["walking_phase_offsets"]

    num_frames = int(duration * sampling_rate)

    # Precompute subcarrier frequencies relative to center
    k_indices = np.arange(num_subcarriers) - num_subcarriers // 2
    subcarrier_freqs = center_freq + k_indices * subcarrier_spacing

    # Convert path delays to seconds
    path_delays_s = path_delays_ns * 1e-9

    frames = []
    for frame_idx in range(num_frames):
        t = frame_idx / sampling_rate

        # Build complex CSI matrix: (num_antennas, num_subcarriers)
        csi_complex = np.zeros((num_antennas, num_subcarriers), dtype=complex)

        for a in range(num_antennas):
            # Human motion modulation for this antenna at this time
            breathing_mod = 1.0 + breathing_depth * np.sin(
                2.0 * np.pi * breathing_freq * t + breathing_phase[a]
            )
            walking_mod = 1.0 + walking_depth * np.sin(
                2.0 * np.pi * walking_freq * t + walking_phase[a]
            )
            motion_factor = breathing_mod * walking_mod

            for p in range(num_paths):
                # Phase shift from path delay across subcarriers
                phase_from_delay = 2.0 * np.pi * subcarrier_freqs * path_delays_s[p]
                # Add per-path per-antenna offset
                total_phase = phase_from_delay + path_phase_offsets[p, a]
                # Accumulate path contribution
                csi_complex[a, :] += (
                    path_amplitudes[p] * motion_factor * np.exp(1j * total_phase)
                )

        amplitude = np.abs(csi_complex)
        phase = np.angle(csi_complex)  # in [-pi, pi]

        frames.append({
            "frame_index": frame_idx,
            "timestamp_s": round(t, 4),
            "amplitude": amplitude.tolist(),
            "phase": phase.tolist(),
        })

    return frames


def save_data(frames, params, output_dir):
    """Save CSI frames and metadata to JSON files.

    Args:
        frames: List of CSI frame dicts.
        params: Generation parameters.
        output_dir: Directory to write output files.
    """
    # Save CSI data
    csi_data = {
        "description": (
            "SYNTHETIC deterministic CSI reference signal for pipeline verification. "
            "This is NOT a real WiFi capture. Generated mathematically with known "
            "parameters for reproducibility testing."
        ),
        "generator": "generate_reference_signal.py",
        "generator_version": "1.0.0",
        "numpy_seed": 42,
        "num_frames": len(frames),
        "num_antennas": params["num_antennas"],
        "num_subcarriers": params["num_subcarriers"],
        "sampling_rate_hz": params["sampling_rate_hz"],
        "frequency_hz": params["center_freq_hz"],
        "bandwidth_hz": params["subcarrier_spacing_hz"] * params["num_subcarriers"],
        "frames": frames,
    }

    data_path = os.path.join(output_dir, "sample_csi_data.json")
    with open(data_path, "w") as f:
        json.dump(csi_data, f, indent=2)
    print(f"Wrote {len(frames)} frames to {data_path}")

    # Save metadata
    meta = {
        "description": (
            "Metadata for the SYNTHETIC deterministic CSI reference signal. "
            "Documents all generation parameters so the signal can be independently "
            "reproduced and verified."
        ),
        "is_synthetic": True,
        "is_real_capture": False,
        "generator_script": "generate_reference_signal.py",
        "numpy_seed": 42,
        "system_parameters": {
            "num_antennas": params["num_antennas"],
            "num_subcarriers": params["num_subcarriers"],
            "sampling_rate_hz": params["sampling_rate_hz"],
            "duration_s": params["duration_s"],
            "center_frequency_hz": params["center_freq_hz"],
            "subcarrier_spacing_hz": params["subcarrier_spacing_hz"],
            "total_frames": int(params["duration_s"] * params["sampling_rate_hz"]),
        },
        "multipath_channel": {
            "num_paths": params["num_paths"],
            "path_delays_ns": params["path_delays_ns"].tolist(),
            "path_amplitudes": params["path_amplitudes"].tolist(),
            "path_phase_offsets_rad": params["path_phase_offsets"].tolist(),
            "description": (
                "5-path indoor multipath model with deterministic delays and "
                "amplitudes. Path amplitudes decrease with delay (typical indoor)."
            ),
        },
        "human_motion_signals": {
            "breathing": {
                "frequency_hz": params["breathing_freq_hz"],
                "modulation_depth": params["breathing_depth"],
                "per_antenna_phase_offsets_rad": params["breathing_phase_offsets"].tolist(),
                "description": (
                    "Sinusoidal amplitude modulation at 0.3 Hz modeling human "
                    "breathing (typical adult resting rate: 12-20 breaths/min = 0.2-0.33 Hz)."
                ),
            },
            "walking": {
                "frequency_hz": params["walking_freq_hz"],
                "modulation_depth": params["walking_depth"],
                "per_antenna_phase_offsets_rad": params["walking_phase_offsets"].tolist(),
                "description": (
                    "Sinusoidal amplitude modulation at 1.2 Hz modeling human "
                    "walking motion (typical stride rate: ~1.0-1.4 Hz)."
                ),
            },
        },
        "generation_formula": (
            "CSI[a,k,t] = sum_p { A_p * exp(j*(2*pi*f_k*tau_p + phi_{p,a})) "
            "* (1 + d_breathe * sin(2*pi*0.3*t + psi_breathe_a)) "
            "* (1 + d_walk * sin(2*pi*1.2*t + psi_walk_a)) }"
        ),
        "determinism_guarantee": (
            "All parameters are derived from numpy.random.RandomState(42) at "
            "script initialization. The generation loop itself uses NO randomness. "
            "Running this script on any platform with the same numpy version will "
            "produce bit-identical output."
        ),
    }

    meta_path = os.path.join(output_dir, "sample_csi_meta.json")
    with open(meta_path, "w") as f:
        json.dump(meta, f, indent=2)
    print(f"Wrote metadata to {meta_path}")


def main():
    """Main entry point."""
    # Determine output directory
    output_dir = os.path.dirname(os.path.abspath(__file__))

    print("=" * 70)
    print("WiFi-DensePose: Deterministic Reference CSI Signal Generator")
    print("=" * 70)
    print(f"Output directory: {output_dir}")
    print()

    # Step 1: Generate deterministic parameters
    print("[1/3] Generating deterministic channel parameters (seed=42)...")
    params = generate_deterministic_parameters()
    print(f"  - {params['num_paths']} multipath paths")
    print(f"  - {params['num_antennas']} antennas, {params['num_subcarriers']} subcarriers")
    print(f"  - Breathing: {params['breathing_freq_hz']} Hz, depth={params['breathing_depth']}")
    print(f"  - Walking: {params['walking_freq_hz']} Hz, depth={params['walking_depth']}")
    print()

    # Step 2: Generate all frames
    num_frames = int(params["duration_s"] * params["sampling_rate_hz"])
    print(f"[2/3] Generating {num_frames} CSI frames...")
    print(f"  - Duration: {params['duration_s']}s at {params['sampling_rate_hz']} Hz")
    frames = generate_csi_frames(params)
    print(f"  - Generated {len(frames)} frames")
    print()

    # Step 3: Save output
    print("[3/3] Saving output files...")
    save_data(frames, params, output_dir)
    print()
    print("Done. Reference signal generated successfully.")
    print("=" * 70)


if __name__ == "__main__":
    main()
