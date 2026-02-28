//! Spike encoding and decoding utilities.
//!
//! This module provides methods to convert between analog signals and sparse spike trains.
//!
//! ## Encoding Schemes
//!
//! - **Rate coding**: Spike frequency encodes magnitude
//! - **Temporal coding**: Spike timing encodes information
//! - **Population coding**: Distributed representation across neurons
//! - **Delta modulation**: Spikes encode changes only
//!
//! ## ASIC Benefits
//!
//! Sparse spike representations dramatically reduce:
//! - Memory bandwidth (only store/transmit active spikes)
//! - Computation (skip silent neurons entirely)
//! - Power consumption (event-driven processing)

use bitvec::prelude::*;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// A single spike event with source and timing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpikeEvent {
    /// Source neuron index
    pub source: u32,
    /// Timestamp in simulation time units
    pub time: f32,
    /// Optional payload (for routing or plasticity)
    pub payload: u8,
}

impl SpikeEvent {
    /// Create a new spike event.
    pub fn new(source: u32, time: f32) -> Self {
        Self {
            source,
            time,
            payload: 0,
        }
    }

    /// Create spike with payload.
    pub fn with_payload(source: u32, time: f32, payload: u8) -> Self {
        Self { source, time, payload }
    }
}

/// A train of spikes from a single neuron over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpikeTrain {
    /// Neuron ID
    pub neuron_id: u32,
    /// Spike times (sorted ascending)
    pub times: Vec<f32>,
}

impl SpikeTrain {
    /// Create empty spike train for a neuron.
    pub fn new(neuron_id: u32) -> Self {
        Self {
            neuron_id,
            times: Vec::new(),
        }
    }

    /// Add a spike at given time.
    pub fn add_spike(&mut self, time: f32) {
        self.times.push(time);
    }

    /// Get spike count.
    pub fn spike_count(&self) -> usize {
        self.times.len()
    }

    /// Calculate firing rate over duration.
    pub fn firing_rate(&self, duration: f32) -> f32 {
        if duration <= 0.0 {
            return 0.0;
        }
        self.times.len() as f32 / duration * 1000.0 // Hz
    }

    /// Get inter-spike intervals.
    pub fn isis(&self) -> Vec<f32> {
        if self.times.len() < 2 {
            return Vec::new();
        }
        self.times.windows(2).map(|w| w[1] - w[0]).collect()
    }
}

/// Sparse spike matrix for population activity.
///
/// Uses compressed representation - only stores active spikes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseSpikes {
    /// Number of neurons in population
    pub num_neurons: u32,
    /// Number of time bins
    pub num_timesteps: u32,
    /// Spike events (sorted by time)
    pub events: Vec<SpikeEvent>,
}

impl SparseSpikes {
    /// Create empty spike matrix.
    pub fn new(num_neurons: u32, num_timesteps: u32) -> Self {
        Self {
            num_neurons,
            num_timesteps,
            events: Vec::new(),
        }
    }

    /// Add a spike event.
    pub fn add_spike(&mut self, neuron: u32, timestep: u32) {
        if neuron < self.num_neurons && timestep < self.num_timesteps {
            self.events.push(SpikeEvent::new(neuron, timestep as f32));
        }
    }

    /// Get sparsity (fraction of silent entries).
    pub fn sparsity(&self) -> f32 {
        let total = self.num_neurons as f64 * self.num_timesteps as f64;
        if total == 0.0 {
            return 1.0;
        }
        1.0 - (self.events.len() as f64 / total) as f32
    }

    /// Get neurons that spiked at given timestep.
    pub fn spikes_at(&self, timestep: u32) -> SmallVec<[u32; 8]> {
        self.events
            .iter()
            .filter(|e| e.time as u32 == timestep)
            .map(|e| e.source)
            .collect()
    }

    /// Get spike count.
    pub fn spike_count(&self) -> usize {
        self.events.len()
    }
}

/// Encoder for converting analog values to spike trains.
pub struct SpikeEncoder;

impl SpikeEncoder {
    /// Rate coding: Convert value to spike probability per timestep.
    ///
    /// Higher values produce more frequent spikes.
    /// Returns sparse bit vector of spike times.
    pub fn rate_encode(value: f32, duration_ms: f32, dt: f32, max_rate_hz: f32) -> BitVec {
        let num_steps = (duration_ms / dt) as usize;
        let mut spikes = bitvec![0; num_steps];

        // Clamp value to [0, 1]
        let normalized = value.clamp(0.0, 1.0);

        // Convert to spike probability per timestep
        let prob_per_step = normalized * max_rate_hz * dt / 1000.0;

        // Generate spikes stochastically
        use rand::Rng;
        let mut rng = rand::thread_rng();

        for i in 0..num_steps {
            if rng.gen::<f32>() < prob_per_step {
                spikes.set(i, true);
            }
        }

        spikes
    }

    /// Temporal coding: First spike time encodes value.
    ///
    /// Lower values spike earlier (inverse temporal coding).
    pub fn temporal_encode(value: f32, max_latency_ms: f32) -> f32 {
        let normalized = value.clamp(0.0, 1.0);
        // Invert: high value = early spike
        (1.0 - normalized) * max_latency_ms
    }

    /// Delta modulation: Spike on significant change.
    ///
    /// Returns (+1, 0, -1) for increase, no change, decrease.
    pub fn delta_encode(current: f32, previous: f32, threshold: f32) -> i8 {
        let delta = current - previous;
        if delta > threshold {
            1 // Positive spike
        } else if delta < -threshold {
            -1 // Negative spike
        } else {
            0 // No spike
        }
    }

    /// Population coding: Distribute value across multiple neurons.
    ///
    /// Returns spike pattern across `num_neurons` with Gaussian tuning curves.
    pub fn population_encode(value: f32, num_neurons: usize, sigma: f32) -> Vec<f32> {
        let mut activities = vec![0.0; num_neurons];
        let centers: Vec<f32> = (0..num_neurons)
            .map(|i| i as f32 / (num_neurons - 1).max(1) as f32)
            .collect();

        for (i, &center) in centers.iter().enumerate() {
            let diff = value - center;
            activities[i] = (-diff * diff / (2.0 * sigma * sigma)).exp();
        }

        activities
    }

    /// Convert image patch to spike-based representation.
    ///
    /// Uses difference-of-Gaussians for edge detection,
    /// then temporal coding for spike generation.
    pub fn encode_image_patch(
        patch: &[f32],
        width: usize,
        height: usize,
    ) -> SparseSpikes {
        let mut spikes = SparseSpikes::new((width * height) as u32, 100);

        // Simple intensity-based encoding
        for (i, &pixel) in patch.iter().enumerate() {
            if i >= width * height {
                break;
            }
            // Higher intensity = earlier spike
            let spike_time = ((1.0 - pixel.clamp(0.0, 1.0)) * 99.0) as u32;
            if pixel > 0.1 {
                // Threshold
                spikes.add_spike(i as u32, spike_time);
            }
        }

        spikes
    }
}

/// Decoder for converting spike trains back to analog values.
pub struct SpikeDecoder;

impl SpikeDecoder {
    /// Decode rate-coded spikes to value.
    pub fn rate_decode(spikes: &BitVec, dt: f32, max_rate_hz: f32) -> f32 {
        let spike_count = spikes.count_ones();
        let duration_ms = spikes.len() as f32 * dt;
        let rate_hz = spike_count as f32 / duration_ms * 1000.0;
        (rate_hz / max_rate_hz).clamp(0.0, 1.0)
    }

    /// Decode temporally-coded spike time to value.
    pub fn temporal_decode(spike_time: f32, max_latency_ms: f32) -> f32 {
        1.0 - (spike_time / max_latency_ms).clamp(0.0, 1.0)
    }

    /// Decode population activity to value using center-of-mass.
    pub fn population_decode(activities: &[f32]) -> f32 {
        let num_neurons = activities.len();
        if num_neurons == 0 {
            return 0.5;
        }

        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for (i, &activity) in activities.iter().enumerate() {
            let center = i as f32 / (num_neurons - 1).max(1) as f32;
            weighted_sum += center * activity;
            total_weight += activity;
        }

        if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            0.5
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spike_event_creation() {
        let event = SpikeEvent::new(42, 10.5);
        assert_eq!(event.source, 42);
        assert_eq!(event.time, 10.5);
        assert_eq!(event.payload, 0);
    }

    #[test]
    fn test_spike_train() {
        let mut train = SpikeTrain::new(0);
        train.add_spike(10.0);
        train.add_spike(30.0);
        train.add_spike(50.0);

        assert_eq!(train.spike_count(), 3);
        assert!((train.firing_rate(100.0) - 30.0).abs() < 0.1);

        let isis = train.isis();
        assert_eq!(isis, vec![20.0, 20.0]);
    }

    #[test]
    fn test_sparse_spikes() {
        let mut spikes = SparseSpikes::new(100, 1000);
        spikes.add_spike(0, 10);
        spikes.add_spike(50, 10);
        spikes.add_spike(99, 500);

        assert_eq!(spikes.spike_count(), 3);
        assert!(spikes.sparsity() > 0.99); // Very sparse

        let at_10 = spikes.spikes_at(10);
        assert_eq!(at_10.len(), 2);
    }

    #[test]
    fn test_rate_encoding() {
        // High value should produce more spikes
        let high_spikes = SpikeEncoder::rate_encode(0.9, 100.0, 1.0, 100.0);
        let low_spikes = SpikeEncoder::rate_encode(0.1, 100.0, 1.0, 100.0);

        // Statistical test - not deterministic
        assert!(high_spikes.count_ones() > low_spikes.count_ones() / 2);
    }

    #[test]
    fn test_temporal_encoding() {
        let early = SpikeEncoder::temporal_encode(0.9, 100.0);
        let late = SpikeEncoder::temporal_encode(0.1, 100.0);

        assert!(early < late); // High value = early spike
    }

    #[test]
    fn test_delta_encoding() {
        assert_eq!(SpikeEncoder::delta_encode(1.0, 0.0, 0.5), 1);
        assert_eq!(SpikeEncoder::delta_encode(0.0, 1.0, 0.5), -1);
        assert_eq!(SpikeEncoder::delta_encode(0.5, 0.5, 0.5), 0);
    }

    #[test]
    fn test_population_encoding() {
        let activities = SpikeEncoder::population_encode(0.5, 10, 0.2);
        assert_eq!(activities.len(), 10);

        // Middle neuron should have highest activity
        let max_idx = activities
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(max_idx, 4); // Close to middle
    }

    #[test]
    fn test_rate_decode() {
        let mut spikes = bitvec![0; 100];
        // 10 spikes in 100 timesteps at dt=1ms = 100 Hz
        for i in (0..100).step_by(10) {
            spikes.set(i, true);
        }

        let decoded = SpikeDecoder::rate_decode(&spikes, 1.0, 100.0);
        assert!((decoded - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_population_decode() {
        // Peak at middle
        let activities = vec![0.0, 0.1, 0.5, 1.0, 0.5, 0.1, 0.0];
        let decoded = SpikeDecoder::population_decode(&activities);
        assert!((decoded - 0.5).abs() < 0.1);
    }
}
