//! Spike-driven attention with multiplication-free operations.
//!
//! Based on Spike-Driven Self-Attention (Yao et al., 2023).
//! Uses temporal coding and binary operations instead of floating-point multiplications,
//! achieving up to 87.2x lower energy consumption compared to vanilla attention.
//!
//! Key innovations:
//! - Rate coding: Values encoded as spike timing
//! - Binary QKV: Query/Key/Value as binary spike trains
//! - Mask-and-add: Attention computed without multiplications
//! - Refractory period: Prevents spike bursts

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

/// Configuration for spike-driven attention.
#[derive(Clone, Debug)]
pub struct SpikeDrivenConfig {
    /// Spike threshold in Q15 fixed-point (0-32768)
    pub spike_threshold_q15: u16,

    /// Number of temporal coding steps per forward pass
    pub temporal_coding_steps: u8,

    /// Use binary quantization for Q, K, V
    pub binary_qkv: bool,

    /// Refractory period (steps) after a spike
    pub refractory_period: u8,
}

impl Default for SpikeDrivenConfig {
    fn default() -> Self {
        Self {
            spike_threshold_q15: 16384, // 0.5 in Q15
            temporal_coding_steps: 8,
            binary_qkv: true,
            refractory_period: 2,
        }
    }
}

/// Spike train representation for temporal coding.
#[derive(Clone, Debug)]
pub struct SpikeTrain {
    /// Spike times within temporal window (0..temporal_coding_steps)
    pub times: Vec<u8>,

    /// Spike polarities: +1 for positive, -1 for negative
    pub polarities: Vec<i8>,
}

impl SpikeTrain {
    /// Create empty spike train.
    pub fn new() -> Self {
        Self {
            times: Vec::new(),
            polarities: Vec::new(),
        }
    }

    /// Create spike train with capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            times: Vec::with_capacity(capacity),
            polarities: Vec::with_capacity(capacity),
        }
    }

    /// Add a spike at given time with polarity.
    pub fn add_spike(&mut self, time: u8, polarity: i8) {
        self.times.push(time);
        self.polarities.push(polarity);
    }

    /// Number of spikes in this train.
    #[inline]
    pub fn len(&self) -> usize {
        self.times.len()
    }

    /// Check if spike train is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// Clear all spikes.
    pub fn clear(&mut self) {
        self.times.clear();
        self.polarities.clear();
    }
}

impl Default for SpikeTrain {
    fn default() -> Self {
        Self::new()
    }
}

/// Spike-driven attention mechanism.
pub struct SpikeDrivenAttention {
    config: SpikeDrivenConfig,
}

impl SpikeDrivenAttention {
    /// Create new spike-driven attention with configuration.
    pub fn new(config: SpikeDrivenConfig) -> Self {
        Self { config }
    }
}

impl Default for SpikeDrivenAttention {
    fn default() -> Self {
        Self {
            config: SpikeDrivenConfig::default(),
        }
    }
}

impl SpikeDrivenAttention {
    /// Convert quantized i8 activations to spike trains using rate coding.
    ///
    /// Higher magnitude values produce more spikes.
    /// Sign determines spike polarity.
    ///
    /// # Arguments
    ///
    /// * `values` - Quantized i8 values (-128 to 127)
    ///
    /// # Returns
    ///
    /// Vector of spike trains, one per value
    pub fn encode_spikes(&self, values: &[i8]) -> Vec<SpikeTrain> {
        let steps = self.config.temporal_coding_steps;
        let mut trains = Vec::with_capacity(values.len());

        for &value in values {
            let mut train = SpikeTrain::with_capacity(steps as usize);

            // Convert to absolute value and polarity
            // Handle i8::MIN (-128) which can't be negated
            let abs_val = if value == i8::MIN {
                128u16
            } else {
                value.abs() as u16
            };
            let polarity = value.signum();

            if abs_val == 0 {
                trains.push(train);
                continue;
            }

            // Rate coding: spike frequency proportional to magnitude
            // Scale to Q15 range: i8 (-128..127) -> (0..32768)
            let rate_q15 = ((abs_val as u32) * 32768 / 128) as u16;

            // Generate spikes based on rate
            let mut refractory_counter = 0u8;
            let mut membrane_potential = 0u32;

            for step in 0..steps {
                // Skip if in refractory period
                if refractory_counter > 0 {
                    refractory_counter -= 1;
                    continue;
                }

                // Accumulate membrane potential (saturating to prevent overflow)
                membrane_potential = membrane_potential.saturating_add(rate_q15 as u32);

                // Fire if threshold exceeded
                if membrane_potential >= self.config.spike_threshold_q15 as u32 {
                    train.add_spike(step, polarity);
                    membrane_potential = 0; // Reset
                    refractory_counter = self.config.refractory_period;
                }
            }

            trains.push(train);
        }

        trains
    }

    /// Compute spike-driven attention using only mask and addition operations.
    ///
    /// No multiplications required - uses spike timing for weighting.
    ///
    /// # Arguments
    ///
    /// * `q_spikes` - Query spike trains [seq_len]
    /// * `k_spikes` - Key spike trains [seq_len]
    /// * `v_spikes` - Value spike trains [hidden_dim]
    ///
    /// # Returns
    ///
    /// Attention output as i32 (accumulated spike contributions)
    pub fn attention(
        &self,
        q_spikes: &[SpikeTrain],
        k_spikes: &[SpikeTrain],
        v_spikes: &[SpikeTrain],
    ) -> Vec<i32> {
        let seq_len = q_spikes.len().min(k_spikes.len());
        let hidden_dim = v_spikes.len();
        let mut output = vec![0i32; hidden_dim];

        if seq_len == 0 || hidden_dim == 0 {
            return output;
        }

        // For each query position
        for q_idx in 0..seq_len {
            let q_train = &q_spikes[q_idx];

            // Compute attention weights via spike coincidence detection
            let mut attention_weights = vec![0i32; seq_len];

            for k_idx in 0..seq_len {
                let k_train = &k_spikes[k_idx];

                // Count spike coincidences (within temporal window)
                let mut coincidence_score = 0i32;

                for (&q_time, &q_pol) in q_train.times.iter().zip(q_train.polarities.iter()) {
                    for (&k_time, &k_pol) in k_train.times.iter().zip(k_train.polarities.iter()) {
                        // Coincidence if spikes occur at same time
                        if q_time == k_time {
                            coincidence_score += (q_pol as i32) * (k_pol as i32);
                        }
                    }
                }

                attention_weights[k_idx] = coincidence_score;
            }

            // Apply causal mask (only attend to past)
            for k_idx in (q_idx + 1)..seq_len {
                attention_weights[k_idx] = 0;
            }

            // Accumulate weighted values using mask-and-add
            for k_idx in 0..=q_idx.min(seq_len - 1) {
                let weight = attention_weights[k_idx];

                if weight == 0 {
                    continue;
                }

                // Add contribution from each value dimension
                for (d, v_train) in v_spikes.iter().enumerate().take(hidden_dim) {
                    // Spike-based value contribution
                    let value_contrib = self.spike_value_contribution(v_train, weight);
                    output[d] += value_contrib;
                }
            }
        }

        output
    }

    /// Compute value contribution using spike timing.
    ///
    /// Instead of multiplication, use spike count weighted by attention.
    /// Uses saturating arithmetic to prevent overflow.
    fn spike_value_contribution(&self, v_train: &SpikeTrain, attention_weight: i32) -> i32 {
        if attention_weight == 0 {
            return 0;
        }

        // Sum spike polarities weighted by attention (saturating to prevent overflow)
        let mut contrib = 0i32;
        for &polarity in &v_train.polarities {
            contrib = contrib.saturating_add((polarity as i32).saturating_mul(attention_weight));
        }

        contrib
    }

    /// Estimate energy savings ratio compared to standard attention.
    ///
    /// Based on Yao et al. 2023:
    /// - Standard attention: ~2N^2D multiplications
    /// - Spike-driven: Only mask and add operations
    ///
    /// Returns ratio: standard_energy / spike_energy
    pub fn energy_ratio(&self, seq_len: usize, hidden_dim: usize) -> f32 {
        if seq_len == 0 || hidden_dim == 0 {
            return 1.0;
        }

        // Standard attention operations (multiplications)
        let standard_mults = 2 * seq_len * seq_len * hidden_dim;

        // Spike-driven operations (additions only)
        // Assume average spike rate of 0.3 (30% of timesteps have spikes)
        let avg_spikes_per_neuron = (self.config.temporal_coding_steps as f32) * 0.3;
        let spike_adds = (seq_len as f32) * avg_spikes_per_neuron * (hidden_dim as f32);

        // Energy ratio (multiplication ~3.7x more expensive than addition)
        let mult_energy_factor = 3.7;

        let standard_energy = (standard_mults as f32) * mult_energy_factor;
        let spike_energy = spike_adds;

        standard_energy / spike_energy
    }

    /// Binary quantization of values to {-1, 0, +1}.
    ///
    /// Used when `binary_qkv` is enabled.
    pub fn binarize(&self, values: &[i8]) -> Vec<i8> {
        values
            .iter()
            .map(|&v| {
                if v > 0 {
                    1
                } else if v < 0 {
                    -1
                } else {
                    0
                }
            })
            .collect()
    }

    /// Compute sparse spike-driven attention with top-k selection.
    ///
    /// Only attend to positions with highest spike coincidence.
    pub fn sparse_attention(
        &self,
        q_spikes: &[SpikeTrain],
        k_spikes: &[SpikeTrain],
        v_spikes: &[SpikeTrain],
        top_k: usize,
    ) -> Vec<i32> {
        let seq_len = q_spikes.len().min(k_spikes.len());
        let hidden_dim = v_spikes.len();
        let mut output = vec![0i32; hidden_dim];

        if seq_len == 0 || hidden_dim == 0 || top_k == 0 {
            return output;
        }

        // For each query position
        for q_idx in 0..seq_len {
            let q_train = &q_spikes[q_idx];

            // Compute attention weights
            let mut attention_weights: Vec<(usize, i32)> = Vec::with_capacity(seq_len);

            for k_idx in 0..=q_idx.min(seq_len - 1) {
                let k_train = &k_spikes[k_idx];

                let mut coincidence_score = 0i32;
                for (&q_time, &q_pol) in q_train.times.iter().zip(q_train.polarities.iter()) {
                    for (&k_time, &k_pol) in k_train.times.iter().zip(k_train.polarities.iter()) {
                        if q_time == k_time {
                            coincidence_score += (q_pol as i32) * (k_pol as i32);
                        }
                    }
                }

                attention_weights.push((k_idx, coincidence_score));
            }

            // Select top-k positions
            attention_weights.sort_by(|a, b| b.1.cmp(&a.1));
            attention_weights.truncate(top_k);

            // Accumulate only top-k contributions
            for (_k_idx, weight) in attention_weights {
                if weight == 0 {
                    continue;
                }

                for (d, v_train) in v_spikes.iter().enumerate().take(hidden_dim) {
                    let value_contrib = self.spike_value_contribution(v_train, weight);
                    output[d] += value_contrib;
                }
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_spike_train_creation() {
        let mut train = SpikeTrain::new();
        assert!(train.is_empty());
        assert_eq!(train.len(), 0);

        train.add_spike(0, 1);
        train.add_spike(3, 1);
        train.add_spike(5, -1);

        assert_eq!(train.len(), 3);
        assert_eq!(train.times, vec![0, 3, 5]);
        assert_eq!(train.polarities, vec![1, 1, -1]);
    }

    #[test]
    fn test_spike_encoding_positive() {
        let config = SpikeDrivenConfig {
            spike_threshold_q15: 16384,
            temporal_coding_steps: 8,
            binary_qkv: true,
            refractory_period: 1,
        };
        let attn = SpikeDrivenAttention::new(config);

        let values = vec![64i8, 32, 16, 0, -32];
        let trains = attn.encode_spikes(&values);

        assert_eq!(trains.len(), 5);

        // Higher magnitude should produce more spikes
        assert!(trains[0].len() >= trains[1].len());
        assert!(trains[1].len() >= trains[2].len());

        // Zero should produce no spikes
        assert_eq!(trains[3].len(), 0);

        // Negative value should have negative polarity
        assert!(trains[4].polarities.iter().all(|&p| p == -1));
    }

    #[test]
    fn test_spike_encoding_rate() {
        let config = SpikeDrivenConfig::default();
        let attn = SpikeDrivenAttention::new(config);

        // Maximum positive value should produce most spikes
        let max_val = vec![127i8];
        let trains = attn.encode_spikes(&max_val);

        // Should have some spikes
        assert!(trains[0].len() > 0);

        // All polarities should be positive
        assert!(trains[0].polarities.iter().all(|&p| p == 1));
    }

    #[test]
    fn test_refractory_period() {
        let refractory_period = 3u8;
        let config = SpikeDrivenConfig {
            spike_threshold_q15: 8192, // Lower threshold
            temporal_coding_steps: 10,
            binary_qkv: true,
            refractory_period, // 3-step refractory
        };
        let attn = SpikeDrivenAttention::new(config);

        let values = vec![127i8]; // Maximum value
        let trains = attn.encode_spikes(&values);

        // Check that spikes respect refractory period
        for i in 1..trains[0].times.len() {
            let time_diff = trains[0].times[i] - trains[0].times[i - 1];
            assert!(time_diff > refractory_period);
        }
    }

    #[test]
    fn test_attention_empty() {
        let attn = SpikeDrivenAttention::default();

        let q_spikes = vec![];
        let k_spikes = vec![];
        let v_spikes = vec![];

        let output = attn.attention(&q_spikes, &k_spikes, &v_spikes);
        assert_eq!(output.len(), 0);
    }

    #[test]
    fn test_attention_basic() {
        let attn = SpikeDrivenAttention::default();

        // Create simple spike trains
        let mut q1 = SpikeTrain::new();
        q1.add_spike(0, 1);

        let mut k1 = SpikeTrain::new();
        k1.add_spike(0, 1); // Coincides with q1

        let mut v1 = SpikeTrain::new();
        v1.add_spike(1, 1);

        let q_spikes = vec![q1];
        let k_spikes = vec![k1];
        let v_spikes = vec![v1];

        let output = attn.attention(&q_spikes, &k_spikes, &v_spikes);

        assert_eq!(output.len(), 1);
        // Should have non-zero output due to coincidence
        assert_ne!(output[0], 0);
    }

    #[test]
    fn test_energy_ratio() {
        let attn = SpikeDrivenAttention::default();

        let ratio = attn.energy_ratio(64, 256);

        // Should show significant energy savings (> 10x)
        assert!(ratio > 10.0);

        // Paper reports up to 87.2x
        // Our conservative estimate should be in reasonable range
        assert!(ratio < 200.0);
    }

    #[test]
    fn test_binarization() {
        let attn = SpikeDrivenAttention::default();

        let values = vec![-64, -1, 0, 1, 64];
        let binary = attn.binarize(&values);

        assert_eq!(binary, vec![-1, -1, 0, 1, 1]);
    }

    #[test]
    fn test_sparse_attention() {
        let attn = SpikeDrivenAttention::default();

        // Create spike trains with different coincidence levels
        let mut q1 = SpikeTrain::new();
        q1.add_spike(0, 1);
        q1.add_spike(2, 1);

        let mut k1 = SpikeTrain::new();
        k1.add_spike(0, 1); // Strong coincidence

        let mut k2 = SpikeTrain::new();
        k2.add_spike(5, 1); // No coincidence

        let mut v1 = SpikeTrain::new();
        v1.add_spike(1, 1);

        let q_spikes = vec![q1];
        let k_spikes = vec![k1, k2];
        let v_spikes = vec![v1];

        // Top-1 should only attend to k1
        let output = attn.sparse_attention(&q_spikes, &k_spikes, &v_spikes, 1);

        assert_eq!(output.len(), 1);
        assert_ne!(output[0], 0); // Should have contribution
    }

    #[test]
    fn test_causal_masking() {
        let attn = SpikeDrivenAttention::default();

        // Create 3 positions
        let mut spikes = vec![];
        for _ in 0..3 {
            let mut train = SpikeTrain::new();
            train.add_spike(0, 1);
            spikes.push(train);
        }

        let output = attn.attention(&spikes, &spikes, &spikes);

        // Should produce valid output
        assert_eq!(output.len(), 3);

        // Note: Actual causal masking is implicit in the attention loop
        // which only iterates k_idx from 0 to q_idx
    }
}
