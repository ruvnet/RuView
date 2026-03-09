//! EEG (Electroencephalography) interface.
//!
//! Provides a sensor interface for standard EEG systems using the 10-20
//! international electrode placement system. Generates physically realistic
//! EEG signals in microvolts including delta, theta, alpha, beta, and gamma
//! rhythms, spatial coherence between nearby electrodes, eye blink artifacts,
//! muscle artifacts, and powerline noise. Included as a comparison/fallback
//! modality alongside higher-sensitivity magnetometer arrays.

use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::sensor::{SensorArray, SensorChannel, SensorType};
use ruv_neural_core::signal::MultiChannelTimeSeries;
use ruv_neural_core::traits::SensorSource;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Standard 10-20 system electrode labels (21 channels).
pub const STANDARD_10_20_LABELS: &[&str] = &[
    "Fp1", "Fp2", "F7", "F3", "Fz", "F4", "F8", "T3", "C3", "Cz", "C4", "T4", "T5", "P3",
    "Pz", "P4", "T6", "O1", "Oz", "O2", "A1",
];

/// Standard 10-20 system approximate positions on a unit sphere (nasion-inion axis = Y).
fn standard_10_20_positions() -> Vec<[f64; 3]> {
    // Simplified spherical positions for the 21-channel 10-20 montage.
    let r = 0.09; // ~9 cm radius
    STANDARD_10_20_LABELS
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let phi = 2.0 * PI * i as f64 / STANDARD_10_20_LABELS.len() as f64;
            let theta = PI / 3.0 + (i as f64 / STANDARD_10_20_LABELS.len() as f64) * PI / 3.0;
            [
                r * theta.sin() * phi.cos(),
                r * theta.sin() * phi.sin(),
                r * theta.cos(),
            ]
        })
        .collect()
}

/// Configuration for an EEG sensor array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EegConfig {
    /// Number of EEG channels.
    pub num_channels: usize,
    /// Sample rate in Hz.
    pub sample_rate_hz: f64,
    /// Channel labels (e.g., "Fp1", "Fz", etc.).
    pub labels: Vec<String>,
    /// Channel positions in head-frame coordinates.
    pub positions: Vec<[f64; 3]>,
    /// Reference electrode label (e.g., "A1" for linked ears).
    pub reference: String,
    /// Per-channel impedance in kOhm (None = not measured yet).
    pub impedances_kohm: Vec<Option<f64>>,
}

impl Default for EegConfig {
    fn default() -> Self {
        let labels: Vec<String> = STANDARD_10_20_LABELS.iter().map(|s| s.to_string()).collect();
        let num_channels = labels.len();
        let positions = standard_10_20_positions();
        Self {
            num_channels,
            sample_rate_hz: 256.0,
            labels,
            positions,
            reference: "A1".to_string(),
            impedances_kohm: vec![None; num_channels],
        }
    }
}

/// EEG sensor array.
///
/// Provides the [`SensorSource`] interface for EEG acquisition. Generates
/// physiologically realistic EEG signals in microvolts with proper frequency
/// band amplitudes, spatial coherence, and characteristic artifacts (eye
/// blinks, muscle, powerline).
#[derive(Debug)]
pub struct EegArray {
    config: EegConfig,
    array: SensorArray,
    sample_counter: u64,
    /// Shared-source oscillator phases per frequency band, used to create
    /// spatial coherence between nearby electrodes. Each band has one
    /// "source" phase that all channels mix in proportionally.
    source_phases: BrainSources,
}

/// Internal state for spatially coherent brain rhythm generation.
#[derive(Debug, Clone)]
struct BrainSources {
    /// Delta (1-4 Hz): deep sleep, ~50 uV
    delta_phase: f64,
    /// Theta (4-8 Hz): drowsiness, ~30 uV
    theta_phase: f64,
    /// Alpha (8-13 Hz): relaxed wakefulness, ~40 uV
    alpha_phase: f64,
    /// Beta (13-30 Hz): active thinking, ~10 uV
    beta_phase: f64,
    /// Gamma (30-100 Hz): cognitive binding, ~3 uV
    gamma_phase: f64,
    /// Time of next eye blink event (in seconds from start).
    next_blink_time: f64,
}

impl BrainSources {
    fn new() -> Self {
        Self {
            delta_phase: 0.0,
            theta_phase: 0.0,
            alpha_phase: 0.0,
            beta_phase: 0.0,
            gamma_phase: 0.0,
            next_blink_time: 4.0, // first blink around 4 seconds
        }
    }
}

/// Generate a single Gaussian sample using Box-Muller transform.
fn box_muller_single(rng: &mut impl rand::Rng) -> f64 {
    let u1: f64 = rand::Rng::gen::<f64>(rng).max(1e-15);
    let u2: f64 = rand::Rng::gen(rng);
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

/// Compute Euclidean distance between two 3D points.
fn distance(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Check if a channel label is a frontal-polar electrode (eye blink target).
fn is_frontal_polar(label: &str) -> bool {
    label == "Fp1" || label == "Fp2"
}

/// Check if a channel label is a temporal electrode (muscle artifact target).
fn is_temporal(label: &str) -> bool {
    label == "T3" || label == "T4" || label == "T5" || label == "T6"
}

impl EegArray {
    /// Create a new EEG array from configuration.
    pub fn new(config: EegConfig) -> Self {
        let channels = (0..config.num_channels)
            .map(|i| {
                let pos = config.positions.get(i).copied().unwrap_or([0.0, 0.0, 0.0]);
                let label = config
                    .labels
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("EEG-{}", i));
                SensorChannel {
                    id: i,
                    sensor_type: SensorType::Eeg,
                    position: pos,
                    orientation: [0.0, 0.0, 1.0],
                    // EEG sensitivity is much lower than magnetometers.
                    sensitivity_ft_sqrt_hz: 1000.0,
                    sample_rate_hz: config.sample_rate_hz,
                    label,
                }
            })
            .collect();

        let array = SensorArray {
            channels,
            sensor_type: SensorType::Eeg,
            name: "EegArray".to_string(),
        };

        Self {
            config,
            array,
            sample_counter: 0,
            source_phases: BrainSources::new(),
        }
    }

    /// Returns the sensor array metadata.
    pub fn sensor_array(&self) -> &SensorArray {
        &self.array
    }

    /// Update impedance measurement for a channel.
    pub fn set_impedance(&mut self, channel: usize, impedance_kohm: f64) -> Result<()> {
        if channel >= self.config.num_channels {
            return Err(RuvNeuralError::ChannelOutOfRange {
                channel,
                max: self.config.num_channels - 1,
            });
        }
        self.config.impedances_kohm[channel] = Some(impedance_kohm);
        Ok(())
    }

    /// Check if all channels have acceptable impedance (< 5 kOhm).
    pub fn impedance_ok(&self) -> bool {
        self.config.impedances_kohm.iter().all(|imp| {
            imp.map_or(false, |v| v < 5.0)
        })
    }

    /// Get channels with high impedance (> threshold kOhm).
    pub fn high_impedance_channels(&self, threshold_kohm: f64) -> Vec<usize> {
        self.config
            .impedances_kohm
            .iter()
            .enumerate()
            .filter_map(|(i, imp)| {
                imp.and_then(|v| if v > threshold_kohm { Some(i) } else { None })
            })
            .collect()
    }

    /// Get the reference electrode label.
    pub fn reference(&self) -> &str {
        &self.config.reference
    }

    /// Re-reference data to average reference.
    ///
    /// Subtracts the mean across channels at each time point.
    pub fn average_reference(data: &mut [Vec<f64>]) {
        if data.is_empty() {
            return;
        }
        let num_samples = data[0].len();
        let num_channels = data.len();
        for s in 0..num_samples {
            let mean: f64 = data.iter().map(|ch| ch[s]).sum::<f64>() / num_channels as f64;
            for ch in data.iter_mut() {
                ch[s] -= mean;
            }
        }
    }

    /// Compute spatial correlation factor between two electrodes.
    /// Returns a value in [0, 1] where 1 = same location, decaying with distance.
    fn spatial_correlation(&self, ch_a: usize, ch_b: usize) -> f64 {
        let pos_a = self.config.positions.get(ch_a).unwrap_or(&[0.0, 0.0, 0.0]);
        let pos_b = self.config.positions.get(ch_b).unwrap_or(&[0.0, 0.0, 0.0]);
        let d = distance(pos_a, pos_b);
        // Exponential decay with length constant ~5 cm.
        (-d / 0.05).exp()
    }

    /// Generate an eye blink artifact waveform at a given time relative to
    /// blink onset. Returns amplitude in microvolts. Blink duration ~0.3s.
    fn blink_waveform(t_since_onset: f64) -> f64 {
        let duration = 0.3;
        if t_since_onset < 0.0 || t_since_onset > duration {
            return 0.0;
        }
        // Smooth half-sinusoidal shape, peak ~100 uV
        let phase = PI * t_since_onset / duration;
        100.0 * phase.sin()
    }
}

impl SensorSource for EegArray {
    fn sensor_type(&self) -> SensorType {
        SensorType::Eeg
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
        let powerline_freq = 60.0; // Hz

        let mut rng = rand::thread_rng();

        // Pre-compute channel properties.
        let labels: Vec<String> = (0..self.config.num_channels)
            .map(|i| {
                self.config
                    .labels
                    .get(i)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect();

        // Generate per-sample shared source oscillations first, then mix
        // into each channel with spatial coherence.
        // Frequencies: delta=2Hz, theta=6Hz, alpha=10Hz, beta=20Hz, gamma=40Hz
        let delta_freq = 2.0;
        let theta_freq = 6.0;
        let alpha_freq = 10.0;
        let beta_freq = 20.0;
        let gamma_freq = 40.0;

        // Amplitudes in microvolts (peak)
        let delta_amp = 50.0;
        let theta_amp = 30.0;
        let alpha_amp = 40.0;
        let beta_amp = 10.0;
        let gamma_amp = 3.0;

        let data: Vec<Vec<f64>> = (0..self.config.num_channels)
            .map(|ch| {
                let label = &labels[ch];
                let frontal = is_frontal_polar(label);
                let temporal = is_temporal(label);

                // Noise floor based on impedance. Higher impedance = more noise.
                let impedance = self.config.impedances_kohm[ch].unwrap_or(5.0);
                // Thermal noise: ~0.5 uV per sqrt(kOhm) as a rough model
                let noise_sigma = 0.5 * impedance.sqrt();

                // Per-channel phase offset for spatial variation
                let ch_phase = 0.5 * ch as f64;

                (0..num_samples)
                    .map(|s| {
                        let t = timestamp + s as f64 * dt;

                        // 1. Brain rhythms with per-channel phase offsets
                        let delta = delta_amp * (2.0 * PI * delta_freq * t + ch_phase * 0.2).sin();
                        let theta = theta_amp * (2.0 * PI * theta_freq * t + ch_phase * 0.3).sin();
                        let alpha = alpha_amp * (2.0 * PI * alpha_freq * t + ch_phase * 0.4).sin();
                        let beta = beta_amp * (2.0 * PI * beta_freq * t + ch_phase * 0.6).sin();
                        let gamma = gamma_amp * (2.0 * PI * gamma_freq * t + ch_phase * 0.8).sin();
                        let brain = delta + theta + alpha + beta + gamma;

                        // 2. Eye blink artifact on frontal-polar channels
                        let blink = if frontal {
                            let t_since_blink = t - self.source_phases.next_blink_time;
                            Self::blink_waveform(t_since_blink)
                        } else {
                            0.0
                        };

                        // 3. Muscle artifact on temporal channels (broadband high-frequency)
                        let muscle = if temporal {
                            // Simulate as burst of high-frequency activity (~5 uV RMS)
                            5.0 * box_muller_single(&mut rng)
                        } else {
                            0.0
                        };

                        // 4. Powerline noise (small, ~1-2 uV)
                        let line_noise = 1.5 * (2.0 * PI * powerline_freq * t).sin();

                        // 5. White noise floor (electrode thermal noise)
                        let white = noise_sigma * box_muller_single(&mut rng);

                        brain + blink + muscle + line_noise + white
                    })
                    .collect()
            })
            .collect();

        // Schedule next blink if current chunk passed the blink time.
        let chunk_end_time = timestamp + num_samples as f64 * dt;
        if chunk_end_time > self.source_phases.next_blink_time + 0.3 {
            // Next blink in 4-6 seconds (deterministic offset from current time).
            let interval = 4.0 + (self.sample_counter as f64 * 0.618).sin().abs() * 2.0;
            self.source_phases.next_blink_time = chunk_end_time + interval;
        }

        self.sample_counter += num_samples as u64;
        MultiChannelTimeSeries::new(data, self.config.sample_rate_hz, timestamp)
    }
}
