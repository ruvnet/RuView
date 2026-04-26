//! Simulated sensor array for testing and development.
//!
//! Generates realistic synthetic neural magnetic field data with configurable
//! channels, sample rate, noise floor, and injectable events.

use rand::Rng;
use ruv_neural_core::error::Result;
use ruv_neural_core::sensor::{SensorArray, SensorChannel, SensorType};
use ruv_neural_core::signal::MultiChannelTimeSeries;
use ruv_neural_core::traits::SensorSource;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// An injectable event that modifies the simulated signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorEvent {
    /// A sharp spike at a specific sample offset.
    Spike {
        /// Channel to inject the spike into.
        channel: usize,
        /// Amplitude in femtotesla.
        amplitude_ft: f64,
        /// Sample offset from the start of the next acquisition.
        sample_offset: usize,
    },
    /// A burst of oscillatory activity.
    OscillationBurst {
        /// Channel to inject the burst into.
        channel: usize,
        /// Frequency of oscillation in Hz.
        frequency_hz: f64,
        /// Amplitude in femtotesla.
        amplitude_ft: f64,
        /// Start sample offset.
        start_sample: usize,
        /// Duration in samples.
        duration_samples: usize,
    },
    /// A DC level shift.
    DcShift {
        /// Channel to inject the shift into.
        channel: usize,
        /// Shift magnitude in femtotesla.
        shift_ft: f64,
        /// Sample offset at which the shift begins.
        start_sample: usize,
    },
}

/// Configuration for an oscillation component injected into the simulator.
#[derive(Debug, Clone)]
struct OscillationComponent {
    /// Frequency in Hz.
    frequency_hz: f64,
    /// Amplitude in femtotesla.
    amplitude_ft: f64,
}

/// Simulated sensor array that generates synthetic neural magnetic field data.
///
/// The simulator produces multi-channel time series with configurable noise,
/// background oscillations (alpha, beta, etc.), and injectable transient events.
#[derive(Debug)]
pub struct SimulatedSensorArray {
    /// Number of channels (4-256).
    num_channels: usize,
    /// Sample rate in Hz (100-10000).
    sample_rate_hz: f64,
    /// Noise floor density in fT/sqrt(Hz).
    noise_density_ft: f64,
    /// Background oscillation components active on all channels.
    oscillations: Vec<OscillationComponent>,
    /// Pending events to inject on the next acquisition.
    pending_events: Vec<SensorEvent>,
    /// Current phase accumulator (sample counter).
    sample_counter: u64,
    /// Sensor array metadata.
    array: SensorArray,
    /// Random number generator.
    rng: rand::rngs::ThreadRng,
}

impl SimulatedSensorArray {
    /// Create a new simulated sensor array.
    ///
    /// # Arguments
    /// * `num_channels` - Number of channels (clamped to 4..=256).
    /// * `sample_rate_hz` - Sample rate in Hz (clamped to 100..=10000).
    pub fn new(num_channels: usize, sample_rate_hz: f64) -> Self {
        let num_channels = num_channels.clamp(4, 256);
        let sample_rate_hz = sample_rate_hz.clamp(100.0, 10000.0);

        let channels = (0..num_channels)
            .map(|i| {
                let angle = 2.0 * PI * i as f64 / num_channels as f64;
                let radius = 0.1; // 10 cm from center
                SensorChannel {
                    id: i,
                    sensor_type: SensorType::NvDiamond,
                    position: [radius * angle.cos(), radius * angle.sin(), 0.0],
                    orientation: [0.0, 0.0, 1.0],
                    sensitivity_ft_sqrt_hz: 10.0,
                    sample_rate_hz,
                    label: format!("SIM-{:03}", i),
                }
            })
            .collect();

        let array = SensorArray {
            channels,
            sensor_type: SensorType::NvDiamond,
            name: "SimulatedSensorArray".to_string(),
        };

        Self {
            num_channels,
            sample_rate_hz,
            noise_density_ft: 10.0,
            oscillations: Vec::new(),
            pending_events: Vec::new(),
            sample_counter: 0,
            array,
            rng: rand::thread_rng(),
        }
    }

    /// Set the noise floor density in fT/sqrt(Hz).
    ///
    /// Returns self for builder-pattern chaining.
    pub fn with_noise(mut self, noise_density_ft: f64) -> Self {
        self.noise_density_ft = noise_density_ft;
        self
    }

    /// Inject an alpha rhythm (~10 Hz) into all channels.
    ///
    /// # Arguments
    /// * `amplitude_ft` - Peak amplitude in femtotesla (typical: ~100 fT).
    pub fn inject_alpha(&mut self, amplitude_ft: f64) {
        self.oscillations.push(OscillationComponent {
            frequency_hz: 10.0,
            amplitude_ft,
        });
    }

    /// Inject a transient event to appear in the next acquisition.
    pub fn inject_event(&mut self, event: SensorEvent) {
        self.pending_events.push(event);
    }

    /// Returns the sensor array metadata.
    pub fn sensor_array(&self) -> &SensorArray {
        &self.array
    }

    /// Add a custom oscillation component to all channels.
    pub fn add_oscillation(&mut self, frequency_hz: f64, amplitude_ft: f64) {
        self.oscillations.push(OscillationComponent {
            frequency_hz,
            amplitude_ft,
        });
    }

    /// Generate samples for one channel.
    fn generate_channel(&mut self, channel_idx: usize, num_samples: usize) -> Vec<f64> {
        let dt = 1.0 / self.sample_rate_hz;
        // Noise standard deviation: density * sqrt(bandwidth).
        // For white noise sampled at fs, the per-sample sigma = density * sqrt(fs / 2).
        let noise_sigma = self.noise_density_ft * (self.sample_rate_hz / 2.0).sqrt();

        let mut samples = Vec::with_capacity(num_samples);

        for s in 0..num_samples {
            let t = (self.sample_counter + s as u64) as f64 * dt;
            let mut value = 0.0;

            // Add oscillation components with slight per-channel phase offset.
            let phase_offset = channel_idx as f64 * 0.1;
            for osc in &self.oscillations {
                value +=
                    osc.amplitude_ft * (2.0 * PI * osc.frequency_hz * t + phase_offset).sin();
            }

            // Add Gaussian noise.
            if noise_sigma > 0.0 {
                let noise: f64 = self.rng.gen::<f64>() * 2.0 - 1.0;
                let noise2: f64 = self.rng.gen::<f64>() * 2.0 - 1.0;
                // Box-Muller transform for Gaussian noise.
                let u1 = self.rng.gen::<f64>().max(1e-15);
                let u2 = self.rng.gen::<f64>();
                let gaussian = (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos();
                value += noise_sigma * gaussian;
                let _ = (noise, noise2); // suppress unused
            }

            samples.push(value);
        }

        // Apply pending events for this channel.
        for event in &self.pending_events {
            match event {
                SensorEvent::Spike {
                    channel,
                    amplitude_ft,
                    sample_offset,
                } => {
                    if *channel == channel_idx && *sample_offset < num_samples {
                        samples[*sample_offset] += amplitude_ft;
                    }
                }
                SensorEvent::OscillationBurst {
                    channel,
                    frequency_hz,
                    amplitude_ft,
                    start_sample,
                    duration_samples,
                } => {
                    if *channel == channel_idx {
                        let end = (*start_sample + *duration_samples).min(num_samples);
                        for s in *start_sample..end {
                            let t = s as f64 / self.sample_rate_hz;
                            samples[s] += amplitude_ft * (2.0 * PI * frequency_hz * t).sin();
                        }
                    }
                }
                SensorEvent::DcShift {
                    channel,
                    shift_ft,
                    start_sample,
                } => {
                    if *channel == channel_idx {
                        for s in *start_sample..num_samples {
                            samples[s] += shift_ft;
                        }
                    }
                }
            }
        }

        samples
    }
}

impl SensorSource for SimulatedSensorArray {
    fn sensor_type(&self) -> SensorType {
        SensorType::NvDiamond
    }

    fn num_channels(&self) -> usize {
        self.num_channels
    }

    fn sample_rate_hz(&self) -> f64 {
        self.sample_rate_hz
    }

    fn read_chunk(&mut self, num_samples: usize) -> Result<MultiChannelTimeSeries> {
        let timestamp = self.sample_counter as f64 / self.sample_rate_hz;

        let mut data = Vec::with_capacity(self.num_channels);
        for ch in 0..self.num_channels {
            data.push(self.generate_channel(ch, num_samples));
        }

        self.sample_counter += num_samples as u64;
        self.pending_events.clear();

        MultiChannelTimeSeries::new(data, self.sample_rate_hz, timestamp)
    }
}
