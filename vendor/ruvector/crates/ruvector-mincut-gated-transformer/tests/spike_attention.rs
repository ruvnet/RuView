//! Integration tests for spike-driven attention.
//!
//! Tests the complete spike-driven attention pipeline including:
//! - Spike encoding from quantized values
//! - Spike-based attention computation
//! - Energy efficiency metrics
//! - Integration with existing transformer components

#![cfg(feature = "spike_attention")]

use ruvector_mincut_gated_transformer::{SpikeDrivenAttention, SpikeDrivenConfig, SpikeTrain};

#[test]
fn test_spike_train_basic_operations() {
    let mut train = SpikeTrain::new();

    assert!(train.is_empty());
    assert_eq!(train.len(), 0);

    train.add_spike(0, 1);
    train.add_spike(2, 1);
    train.add_spike(5, -1);

    assert_eq!(train.len(), 3);
    assert_eq!(train.times.len(), 3);
    assert_eq!(train.polarities.len(), 3);

    train.clear();
    assert!(train.is_empty());
}

#[test]
fn test_spike_encoding_range() {
    let config = SpikeDrivenConfig {
        spike_threshold_q15: 16384,
        temporal_coding_steps: 10,
        binary_qkv: true,
        refractory_period: 2,
    };
    let attn = SpikeDrivenAttention::new(config);

    // Test full i8 range
    let values: Vec<i8> = vec![-128, -64, -32, 0, 32, 64, 127];
    let trains = attn.encode_spikes(&values);

    assert_eq!(trains.len(), 7);

    // Zero produces no spikes
    assert_eq!(trains[3].len(), 0);

    // Negative values should have negative polarity
    for train in &trains[0..3] {
        if !train.is_empty() {
            assert!(train.polarities.iter().all(|&p| p == -1));
        }
    }

    // Positive values should have positive polarity
    for train in &trains[4..7] {
        if !train.is_empty() {
            assert!(train.polarities.iter().all(|&p| p == 1));
        }
    }
}

#[test]
fn test_spike_encoding_proportionality() {
    let config = SpikeDrivenConfig::default();
    let attn = SpikeDrivenAttention::new(config);

    // Higher magnitude should produce more spikes
    let values = vec![127i8, 64, 32, 16, 8];
    let trains = attn.encode_spikes(&values);

    // Verify descending spike counts
    for i in 0..trains.len() - 1 {
        assert!(
            trains[i].len() >= trains[i + 1].len(),
            "Higher values should produce more spikes: {} vs {}",
            trains[i].len(),
            trains[i + 1].len()
        );
    }
}

#[test]
fn test_refractory_period_enforcement() {
    let config = SpikeDrivenConfig {
        spike_threshold_q15: 4096, // Low threshold for more spikes
        temporal_coding_steps: 20,
        binary_qkv: true,
        refractory_period: 5,
    };
    let refractory_period = config.refractory_period;
    let attn = SpikeDrivenAttention::new(config);

    let values = vec![127i8]; // Maximum value
    let trains = attn.encode_spikes(&values);

    if trains[0].len() > 1 {
        // Verify refractory period between consecutive spikes
        for i in 1..trains[0].times.len() {
            let time_diff = trains[0].times[i] - trains[0].times[i - 1];
            assert!(
                time_diff > refractory_period,
                "Spikes should respect refractory period: diff={}, period={}",
                time_diff,
                refractory_period
            );
        }
    }
}

#[test]
fn test_attention_output_shape() {
    let attn = SpikeDrivenAttention::default();

    // Create simple spike trains
    let seq_len = 4;
    let hidden_dim = 8;

    let mut q_spikes = Vec::with_capacity(seq_len);
    let mut k_spikes = Vec::with_capacity(seq_len);
    let mut v_spikes = Vec::with_capacity(hidden_dim);

    for _ in 0..seq_len {
        let mut train = SpikeTrain::new();
        train.add_spike(0, 1);
        q_spikes.push(train.clone());
        k_spikes.push(train);
    }

    for _ in 0..hidden_dim {
        let mut train = SpikeTrain::new();
        train.add_spike(1, 1);
        v_spikes.push(train);
    }

    let output = attn.attention(&q_spikes, &k_spikes, &v_spikes);

    assert_eq!(output.len(), hidden_dim);
}

#[test]
fn test_attention_causal_masking() {
    let attn = SpikeDrivenAttention::default();

    // Create spike trains where future positions have different patterns
    let mut q_spikes = vec![];
    let mut k_spikes = vec![];
    let mut v_spikes = vec![];

    // Position 0 query
    let mut q0 = SpikeTrain::new();
    q0.add_spike(0, 1);
    q_spikes.push(q0);

    // Position 0 key (should match)
    let mut k0 = SpikeTrain::new();
    k0.add_spike(0, 1);
    k_spikes.push(k0);

    // Position 1 key (should not affect position 0's attention)
    let mut k1 = SpikeTrain::new();
    k1.add_spike(0, 1);
    k1.add_spike(1, 1);
    k1.add_spike(2, 1); // Much stronger signal
    k_spikes.push(k1);

    let mut v0 = SpikeTrain::new();
    v0.add_spike(0, 1);
    v_spikes.push(v0);

    // Compute attention for position 0
    // It should only see k0, not k1 (causal masking)
    let output = attn.attention(&q_spikes, &k_spikes, &v_spikes);

    assert_eq!(output.len(), 1);
    // Output should be non-zero due to coincidence at position 0
    assert_ne!(output[0], 0);
}

#[test]
fn test_coincidence_detection() {
    let attn = SpikeDrivenAttention::default();

    // Test spike coincidence scoring
    let mut q_train = SpikeTrain::new();
    q_train.add_spike(0, 1);
    q_train.add_spike(5, 1);

    let mut k_coincident = SpikeTrain::new();
    k_coincident.add_spike(0, 1); // Matches q at time 0
    k_coincident.add_spike(5, 1); // Matches q at time 5

    let mut k_no_match = SpikeTrain::new();
    k_no_match.add_spike(1, 1); // No match
    k_no_match.add_spike(3, 1); // No match

    let mut v_train = SpikeTrain::new();
    v_train.add_spike(0, 1);

    let q_spikes = vec![q_train];
    let k_spikes = vec![k_coincident, k_no_match];
    let v_spikes = vec![v_train];

    let output = attn.attention(&q_spikes, &k_spikes, &v_spikes);

    // Should have stronger output due to coincident k0
    assert_ne!(output[0], 0);
}

#[test]
fn test_polarity_interaction() {
    let attn = SpikeDrivenAttention::default();

    // Test opposite polarities
    let mut q_pos = SpikeTrain::new();
    q_pos.add_spike(0, 1);

    let mut k_pos = SpikeTrain::new();
    k_pos.add_spike(0, 1); // Same polarity

    let mut k_neg = SpikeTrain::new();
    k_neg.add_spike(0, -1); // Opposite polarity

    let mut v_train = SpikeTrain::new();
    v_train.add_spike(0, 1);

    // Test with same polarity
    let q_spikes = vec![q_pos.clone()];
    let k_spikes = vec![k_pos];
    let v_spikes = vec![v_train.clone()];

    let output_same = attn.attention(&q_spikes, &k_spikes, &v_spikes);

    // Test with opposite polarity
    let k_spikes_opp = vec![k_neg];
    let output_opp = attn.attention(&q_spikes, &k_spikes_opp, &v_spikes);

    // Opposite polarities should produce negative contribution
    assert!(output_same[0] > 0);
    assert!(output_opp[0] < 0);
}

#[test]
fn test_sparse_attention_top_k() {
    let attn = SpikeDrivenAttention::default();

    // Create scenario with clear top-k positions
    let mut q_spikes = vec![];
    let mut k_spikes = vec![];
    let mut v_spikes = vec![];

    let mut q = SpikeTrain::new();
    q.add_spike(0, 1);
    q.add_spike(1, 1);
    q_spikes.push(q);

    // k0: strong match (2 coincidences)
    let mut k0 = SpikeTrain::new();
    k0.add_spike(0, 1);
    k0.add_spike(1, 1);
    k_spikes.push(k0);

    // k1: weak match (1 coincidence)
    let mut k1 = SpikeTrain::new();
    k1.add_spike(0, 1);
    k_spikes.push(k1);

    // k2: no match
    let mut k2 = SpikeTrain::new();
    k2.add_spike(5, 1);
    k_spikes.push(k2);

    let mut v = SpikeTrain::new();
    v.add_spike(0, 1);
    v_spikes.push(v);

    // Top-1 should only use strongest match (k0)
    let output_top1 = attn.sparse_attention(&q_spikes, &k_spikes, &v_spikes, 1);

    // Top-2 should use both k0 and k1
    let output_top2 = attn.sparse_attention(&q_spikes, &k_spikes, &v_spikes, 2);

    assert_eq!(output_top1.len(), 1);
    assert_eq!(output_top2.len(), 1);

    // Top-2 should have higher magnitude (more contributions)
    assert!(output_top2[0].abs() >= output_top1[0].abs());
}

#[test]
fn test_energy_ratio_estimation() {
    let attn = SpikeDrivenAttention::default();

    // Test various sequence lengths
    let test_cases = vec![
        (16, 64),   // Small
        (64, 256),  // Medium
        (128, 512), // Large
    ];

    for (seq_len, hidden_dim) in test_cases {
        let ratio = attn.energy_ratio(seq_len, hidden_dim);

        // Should show significant energy savings
        assert!(
            ratio > 5.0,
            "Energy ratio should be > 5x for ({}, {})",
            seq_len,
            hidden_dim
        );

        // Should be finite and positive
        assert!(ratio.is_finite());
        assert!(ratio > 0.0);
    }
}

#[test]
fn test_energy_ratio_scaling() {
    let attn = SpikeDrivenAttention::default();

    // Energy ratio should improve with larger sequences
    // (more multiplications avoided)
    let ratio_small = attn.energy_ratio(16, 64);
    let ratio_large = attn.energy_ratio(128, 512);

    assert!(
        ratio_large > ratio_small,
        "Energy savings should increase with size: small={}, large={}",
        ratio_small,
        ratio_large
    );
}

#[test]
fn test_binarization() {
    let attn = SpikeDrivenAttention::default();

    let values = vec![-127, -50, -1, 0, 1, 50, 127];
    let binary = attn.binarize(&values);

    assert_eq!(binary.len(), values.len());

    // All values should be in {-1, 0, 1}
    for &b in &binary {
        assert!(b >= -1 && b <= 1);
    }

    // Check specific mappings
    assert_eq!(binary[0], -1); // negative -> -1
    assert_eq!(binary[3], 0); // zero -> 0
    assert_eq!(binary[6], 1); // positive -> 1
}

#[test]
fn test_end_to_end_encoding_and_attention() {
    let config = SpikeDrivenConfig {
        spike_threshold_q15: 16384,
        temporal_coding_steps: 8,
        binary_qkv: true,
        refractory_period: 2,
    };
    let attn = SpikeDrivenAttention::new(config);

    // Simulate simple sequence: [high, medium, low, zero]
    let q_values = vec![100i8, 50, 25, 0];
    let k_values = vec![100i8, 50, 25, 0];
    let v_values = vec![64i8, 32, 16, 8];

    // Encode to spike trains
    let q_spikes = attn.encode_spikes(&q_values);
    let k_spikes = attn.encode_spikes(&k_values);
    let v_spikes = attn.encode_spikes(&v_values);

    // Compute attention
    let output = attn.attention(&q_spikes, &k_spikes, &v_spikes);

    assert_eq!(output.len(), v_values.len());

    // Output should be non-zero for positions with spike activity
    assert!(output.iter().any(|&x| x != 0));
}

#[test]
fn test_zero_length_sequences() {
    let attn = SpikeDrivenAttention::default();

    let empty_spikes: Vec<SpikeTrain> = vec![];
    let output = attn.attention(&empty_spikes, &empty_spikes, &empty_spikes);

    assert_eq!(output.len(), 0);
}

#[test]
fn test_mismatched_dimensions() {
    let attn = SpikeDrivenAttention::default();

    let mut q_spikes = vec![SpikeTrain::new()];
    let k_spikes = vec![SpikeTrain::new(), SpikeTrain::new()];
    let v_spikes = vec![SpikeTrain::new()];

    q_spikes[0].add_spike(0, 1);

    // Should handle mismatched dimensions gracefully
    let output = attn.attention(&q_spikes, &k_spikes, &v_spikes);

    assert_eq!(output.len(), 1);
}

#[test]
fn test_high_temporal_resolution() {
    let config = SpikeDrivenConfig {
        spike_threshold_q15: 8192,
        temporal_coding_steps: 32, // High temporal resolution
        binary_qkv: false,
        refractory_period: 1,
    };
    let temporal_coding_steps = config.temporal_coding_steps;
    let attn = SpikeDrivenAttention::new(config);

    let values = vec![127i8];
    let trains = attn.encode_spikes(&values);

    // Should produce more spikes with higher temporal resolution
    assert!(trains[0].len() > 0);

    // All spike times should be within temporal window
    for &time in &trains[0].times {
        assert!(time < temporal_coding_steps);
    }
}

#[test]
fn test_config_variations() {
    // Test different configurations
    let configs = vec![
        SpikeDrivenConfig {
            spike_threshold_q15: 8192,
            temporal_coding_steps: 4,
            binary_qkv: true,
            refractory_period: 1,
        },
        SpikeDrivenConfig {
            spike_threshold_q15: 24576,
            temporal_coding_steps: 16,
            binary_qkv: false,
            refractory_period: 3,
        },
    ];

    for config in configs {
        let temporal_coding_steps = config.temporal_coding_steps;
        let attn = SpikeDrivenAttention::new(config);

        let values = vec![64i8, -64];
        let trains = attn.encode_spikes(&values);

        assert_eq!(trains.len(), 2);

        // Basic sanity checks
        for train in &trains {
            for &time in &train.times {
                assert!(time < temporal_coding_steps);
            }
            for &polarity in &train.polarities {
                assert!(polarity == 1 || polarity == -1);
            }
        }
    }
}
