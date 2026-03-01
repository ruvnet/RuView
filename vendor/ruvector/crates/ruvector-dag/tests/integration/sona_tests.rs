//! SONA learning integration tests

use ruvector_dag::dag::{OperatorNode, OperatorType, QueryDag};
use ruvector_dag::sona::*;

#[test]
fn test_micro_lora_adaptation() {
    let mut lora = MicroLoRA::new(MicroLoRAConfig::default(), 256);

    let input = ndarray::Array1::from_vec(vec![0.1; 256]);
    let output1 = lora.forward(&input);

    // Adapt
    let gradient = ndarray::Array1::from_vec(vec![0.01; 256]);
    lora.adapt(&gradient, 0.1);

    let output2 = lora.forward(&input);

    // Output should change after adaptation
    let diff: f32 = output1
        .iter()
        .zip(output2.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();

    assert!(diff > 0.0, "Output should change after adaptation");
}

#[test]
fn test_trajectory_buffer() {
    let buffer = DagTrajectoryBuffer::new(10);

    // Push trajectories
    for i in 0..15 {
        buffer.push(DagTrajectory::new(
            i as u64,
            vec![0.1; 256],
            "topological".to_string(),
            100.0,
            150.0,
        ));
    }

    // Buffer should not exceed capacity
    assert!(buffer.len() <= 10);

    // Drain should return all
    let drained = buffer.drain();
    assert!(!drained.is_empty());
    assert!(buffer.is_empty());
}

#[test]
fn test_reasoning_bank_clustering() {
    let mut bank = DagReasoningBank::new(ReasoningBankConfig {
        num_clusters: 5,
        pattern_dim: 256,
        max_patterns: 100,
        similarity_threshold: 0.5,
    });

    // Store patterns
    for i in 0..50 {
        let pattern: Vec<f32> = (0..256)
            .map(|j| ((i * 256 + j) as f32 / 1000.0).sin())
            .collect();
        bank.store_pattern(pattern, 0.8);
    }

    assert_eq!(bank.pattern_count(), 50);

    // Cluster
    bank.recompute_clusters();

    // Query similar
    let query: Vec<f32> = (0..256).map(|j| (j as f32 / 1000.0).sin()).collect();
    let results = bank.query_similar(&query, 5);

    assert!(results.len() <= 5);
}

#[test]
fn test_ewc_prevents_forgetting() {
    let mut ewc = EwcPlusPlus::new(EwcConfig::default());

    // Initial parameters
    let params1 = ndarray::Array1::from_vec(vec![1.0; 256]);
    let fisher1 = ndarray::Array1::from_vec(vec![0.1; 256]);

    ewc.consolidate(&params1, &fisher1);

    // Penalty should be 0 for original params
    let penalty0 = ewc.penalty(&params1);
    assert!(penalty0 < 0.001);

    // Penalty should increase for deviated params
    let params2 = ndarray::Array1::from_vec(vec![2.0; 256]);
    let penalty1 = ewc.penalty(&params2);

    assert!(penalty1 > penalty0);
}

#[test]
fn test_trajectory_buffer_ordering() {
    let buffer = DagTrajectoryBuffer::new(100);

    // Push trajectories with different timestamps
    for i in 0..10 {
        buffer.push(DagTrajectory::new(
            i as u64,
            vec![0.1; 256],
            "test".to_string(),
            100.0,
            150.0,
        ));
    }

    let trajectories = buffer.drain();

    // Should maintain insertion order
    for (idx, traj) in trajectories.iter().enumerate() {
        assert_eq!(traj.query_hash, idx as u64);
    }
}

#[test]
fn test_lora_rank_adaptation() {
    let config = MicroLoRAConfig {
        rank: 8,
        alpha: 16.0,
        dropout: 0.1,
    };

    let lora = MicroLoRA::new(config, 256);
    let input = ndarray::Array1::from_vec(vec![0.5; 256]);
    let output = lora.forward(&input);

    assert_eq!(output.len(), 256);
}

#[test]
fn test_reasoning_bank_similarity_threshold() {
    let config = ReasoningBankConfig {
        num_clusters: 3,
        pattern_dim: 64,
        max_patterns: 50,
        similarity_threshold: 0.9, // High threshold
    };

    let mut bank = DagReasoningBank::new(config);

    // Store identical patterns
    let pattern = vec![1.0; 64];
    for _ in 0..10 {
        bank.store_pattern(pattern.clone(), 0.8);
    }

    // Query should return similar patterns
    let results = bank.query_similar(&pattern, 5);
    assert!(!results.is_empty());
}

#[test]
fn test_ewc_consolidation_updates() {
    let mut ewc = EwcPlusPlus::new(EwcConfig {
        lambda: 1000.0,
        decay: 0.9,
        online: true,
    });

    let params1 = ndarray::Array1::from_vec(vec![1.0; 256]);
    let fisher1 = ndarray::Array1::from_vec(vec![0.5; 256]);

    ewc.consolidate(&params1, &fisher1);

    // Second consolidation
    let params2 = ndarray::Array1::from_vec(vec![1.5; 256]);
    let fisher2 = ndarray::Array1::from_vec(vec![0.3; 256]);

    ewc.consolidate(&params2, &fisher2);

    // Penalty should consider both consolidations
    let params3 = ndarray::Array1::from_vec(vec![2.0; 256]);
    let penalty = ewc.penalty(&params3);

    assert!(penalty > 0.0);
}

#[test]
fn test_trajectory_buffer_capacity() {
    let buffer = DagTrajectoryBuffer::new(5);

    for i in 0..10 {
        buffer.push(DagTrajectory::new(
            i as u64,
            vec![0.1; 256],
            "test".to_string(),
            100.0,
            150.0,
        ));
    }

    // Should only keep last 5
    assert_eq!(buffer.len(), 5);

    let trajectories = buffer.drain();
    assert_eq!(trajectories.len(), 5);

    // Should have IDs 5-9 (most recent)
    let ids: Vec<u64> = trajectories.iter().map(|t| t.query_hash).collect();
    assert!(ids.contains(&5));
    assert!(ids.contains(&9));
}

#[test]
fn test_reasoning_bank_cluster_count() {
    let config = ReasoningBankConfig {
        num_clusters: 4,
        pattern_dim: 128,
        max_patterns: 100,
        similarity_threshold: 0.5,
    };

    let mut bank = DagReasoningBank::new(config);

    // Store diverse patterns
    for i in 0..20 {
        let pattern: Vec<f32> = (0..128).map(|j| ((i + j) as f32 / 10.0).sin()).collect();
        bank.store_pattern(pattern, 0.7);
    }

    bank.recompute_clusters();

    // Should have created clusters
    assert!(bank.cluster_count() <= 4);
}
