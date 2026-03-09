//! NV Diamond magnetometer interface.
//!
//! Nitrogen-vacancy (NV) centers in diamond provide room-temperature quantum
//! magnetometry with ~10 fT/sqrt(Hz) sensitivity. This module implements the
//! acquisition interface, calibration structures, and ODMR-based signal model
//! for NV diamond arrays.

use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::sensor::{SensorArray, SensorChannel, SensorType};
use ruv_neural_core::signal::MultiChannelTimeSeries;
use ruv_neural_core::traits::SensorSource;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// NV center gyromagnetic ratio in GHz/T.
const GAMMA_NV_GHZ_PER_T: f64 = 28.024;

/// Configuration for an NV diamond magnetometer array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvDiamondConfig {
    /// Number of diamond sensor chips.
    pub num_channels: usize,
    /// Sample rate in Hz.
    pub sample_rate_hz: f64,
    /// Laser power in mW per chip.
    pub laser_power_mw: f64,
    /// Microwave drive frequency in GHz (near 2.87 GHz zero-field splitting).
    pub microwave_freq_ghz: f64,
    /// Positions of each diamond chip in head-frame coordinates (x, y, z in meters).
    pub chip_positions: Vec<[f64; 3]>,
}

impl Default for NvDiamondConfig {
    fn default() -> Self {
        let num_channels = 16;
        let positions: Vec<[f64; 3]> = (0..num_channels)
            .map(|i| {
                let angle = 2.0 * PI * i as f64 / num_channels as f64;
                let r = 0.09;
                [r * angle.cos(), r * angle.sin(), 0.0]
            })
            .collect();
        Self {
            num_channels,
            sample_rate_hz: 1000.0,
            laser_power_mw: 100.0,
            microwave_freq_ghz: 2.87,
            chip_positions: positions,
        }
    }
}

/// Per-channel calibration data for NV diamond sensors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvCalibration {
    /// Sensitivity in fT per fluorescence count, per channel.
    pub sensitivity_ft_per_count: Vec<f64>,
    /// Noise floor in fT/sqrt(Hz), per channel.
    pub noise_floor_ft: Vec<f64>,
    /// Zero-field splitting offset per channel in MHz.
    pub zfs_offset_mhz: Vec<f64>,
}

impl NvCalibration {
    /// Create default calibration for `n` channels.
    pub fn default_for(n: usize) -> Self {
        Self {
            sensitivity_ft_per_count: vec![0.1; n],
            noise_floor_ft: vec![10.0; n],
            zfs_offset_mhz: vec![0.0; n],
        }
    }
}

/// NV Diamond magnetometer array.
///
/// Provides the [`SensorSource`] interface for NV diamond magnetometry.
/// Generates physically realistic ODMR-based magnetic field signals including
/// neural oscillation bands (alpha, beta, gamma) and sensor-characteristic
/// noise (1/f pink noise + shot noise).
#[derive(Debug)]
pub struct NvDiamondArray {
    config: NvDiamondConfig,
    calibration: NvCalibration,
    array: SensorArray,
    sample_counter: u64,
    /// Pink noise state per channel (1/f generator using Voss-McCartney algorithm).
    pink_state: Vec<PinkNoiseGen>,
}

/// Voss-McCartney pink noise generator (8 octaves).
#[derive(Debug, Clone)]
struct PinkNoiseGen {
    octaves: [f64; 8],
    counter: u32,
}

impl PinkNoiseGen {
    fn new() -> Self {
        Self {
            octaves: [0.0; 8],
            counter: 0,
        }
    }

    /// Generate the next pink noise sample using the Voss-McCartney algorithm.
    /// Returns a value with approximate unit variance when averaged.
    fn next(&mut self, rng: &mut impl rand::Rng) -> f64 {
        self.counter = self.counter.wrapping_add(1);
        let changed = self.counter;
        // Update octave i when bit i flips from 0 to 1
        for i in 0..8u32 {
            if changed & (1 << i) != 0 {
                self.octaves[i as usize] = box_muller_single(rng);
                break; // Voss-McCartney: only update the lowest changed bit
            }
        }
        // Sum all octaves and normalize
        let sum: f64 = self.octaves.iter().sum();
        sum / (8.0_f64).sqrt()
    }
}

/// Generate a single Gaussian sample using Box-Muller transform.
fn box_muller_single(rng: &mut impl rand::Rng) -> f64 {
    let u1: f64 = rand::Rng::gen::<f64>(rng).max(1e-15);
    let u2: f64 = rand::Rng::gen(rng);
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

impl NvDiamondArray {
    /// Create a new NV diamond array from configuration.
    pub fn new(config: NvDiamondConfig) -> Self {
        let calibration = NvCalibration::default_for(config.num_channels);
        let channels = (0..config.num_channels)
            .map(|i| {
                let pos = config
                    .chip_positions
                    .get(i)
                    .copied()
                    .unwrap_or([0.0, 0.0, 0.0]);
                SensorChannel {
                    id: i,
                    sensor_type: SensorType::NvDiamond,
                    position: pos,
                    orientation: [0.0, 0.0, 1.0],
                    sensitivity_ft_sqrt_hz: calibration.noise_floor_ft[i],
                    sample_rate_hz: config.sample_rate_hz,
                    label: format!("NV-{:03}", i),
                }
            })
            .collect();

        let array = SensorArray {
            channels,
            sensor_type: SensorType::NvDiamond,
            name: "NvDiamondArray".to_string(),
        };

        let pink_state = (0..config.num_channels)
            .map(|_| PinkNoiseGen::new())
            .collect();

        Self {
            config,
            calibration,
            array,
            sample_counter: 0,
            pink_state,
        }
    }

    /// Returns the sensor array metadata.
    pub fn sensor_array(&self) -> &SensorArray {
        &self.array
    }

    /// Set custom calibration data.
    pub fn with_calibration(mut self, calibration: NvCalibration) -> Result<Self> {
        if calibration.sensitivity_ft_per_count.len() != self.config.num_channels {
            return Err(RuvNeuralError::DimensionMismatch {
                expected: self.config.num_channels,
                got: calibration.sensitivity_ft_per_count.len(),
            });
        }
        self.calibration = calibration;
        Ok(self)
    }

    /// Get the current calibration data.
    pub fn calibration(&self) -> &NvCalibration {
        &self.calibration
    }

    /// Convert raw fluorescence counts to magnetic field (fT) via ODMR analysis.
    ///
    /// Models the ODMR dip as a Lorentzian centered at the zero-field splitting
    /// frequency (2.87 GHz + channel offset). The fluorescence value represents
    /// a deviation from the baseline ODMR dip depth, which is proportional to
    /// the magnetic field via the NV gyromagnetic ratio (28.024 GHz/T).
    ///
    /// The conversion applies per-channel calibration sensitivity to translate
    /// the fluorescence deviation into a field measurement in femtotesla.
    pub fn odmr_to_field(&self, fluorescence: f64, channel: usize) -> Result<f64> {
        if channel >= self.config.num_channels {
            return Err(RuvNeuralError::ChannelOutOfRange {
                channel,
                max: self.config.num_channels - 1,
            });
        }
        // The fluorescence deviation from baseline is proportional to the
        // resonance frequency shift. Convert via calibrated sensitivity.
        // field_ft = (fluorescence - baseline) * sensitivity_ft_per_count
        // The baseline is implicitly zero in our convention (deviation from it).
        let field_ft = fluorescence * self.calibration.sensitivity_ft_per_count[channel];
        Ok(field_ft)
    }

    /// Generate the brain signal component at a given time (in seconds) for
    /// a given channel, returning the value in femtotesla.
    ///
    /// Models superimposed neural oscillation bands:
    /// - Alpha (8-13 Hz): ~50 fT
    /// - Beta (13-30 Hz): ~20 fT
    /// - Gamma (30-100 Hz): ~5 fT
    fn brain_signal_ft(&self, t: f64, ch: usize) -> f64 {
        let sens = self.calibration.sensitivity_ft_per_count[ch];
        // Scale amplitudes by channel sensitivity (higher sensitivity = larger signal)
        let scale = sens / 0.1; // normalized to default sensitivity

        // Alpha band: 10 Hz representative frequency
        let alpha = 50.0 * scale * (2.0 * PI * 10.0 * t + 0.3 * ch as f64).sin();
        // Beta band: 20 Hz representative frequency
        let beta = 20.0 * scale * (2.0 * PI * 20.0 * t + 0.7 * ch as f64).sin();
        // Gamma band: 40 Hz representative frequency
        let gamma = 5.0 * scale * (2.0 * PI * 40.0 * t + 1.1 * ch as f64).sin();

        alpha + beta + gamma
    }
}

impl SensorSource for NvDiamondArray {
    fn sensor_type(&self) -> SensorType {
        SensorType::NvDiamond
    }

    fn num_channels(&self) -> usize {
        self.config.num_channels
    }

    fn sample_rate_hz(&self) -> f64 {
        self.config.sample_rate_hz
    }

    fn read_chunk(&mut self, num_samples: usize) -> Result<MultiChannelTimeSeries> {
        let timestamp = self.sample_counter as f64 / self.config.sample_rate_hz;
        let dt = 1.0 / self.config.sample_rate_hz;

        let mut rng = rand::thread_rng();
        let data: Vec<Vec<f64>> = (0..self.config.num_channels)
            .map(|ch| {
                let noise_floor = self.calibration.noise_floor_ft[ch];
                // White noise (shot noise) scaled to noise floor.
                // noise_floor is in fT/sqrt(Hz), convert to per-sample sigma.
                let white_sigma = noise_floor * (self.config.sample_rate_hz / 2.0).sqrt();

                // 1/f (pink) noise amplitude: comparable to white noise floor
                // but spectrally shaped to dominate at low frequencies.
                let pink_amplitude = noise_floor * 2.0;

                (0..num_samples)
                    .map(|s| {
                        let t = timestamp + s as f64 * dt;

                        // 1. Brain signal: alpha + beta + gamma oscillations
                        let brain = self.brain_signal_ft(t, ch);

                        // 2. 1/f (pink) noise from Voss-McCartney generator
                        let pink = pink_amplitude * self.pink_state[ch].next(&mut rng);

                        // 3. White (shot) noise floor
                        let white = white_sigma * box_muller_single(&mut rng);

                        // Sum all components
                        brain + pink + white
                    })
                    .collect()
            })
            .collect();

        self.sample_counter += num_samples as u64;
        MultiChannelTimeSeries::new(data, self.config.sample_rate_hz, timestamp)
    }
}
