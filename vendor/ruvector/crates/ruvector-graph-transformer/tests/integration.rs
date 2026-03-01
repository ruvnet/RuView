//! Integration tests for ruvector-graph-transformer.
//!
//! Tests the composition of all modules through proof-gated operations.

use ruvector_graph_transformer::{
    AttestationChain, GraphTransformer, GraphTransformerConfig, ProofGate,
};
use ruvector_verified::{
    gated::{ProofKind, ProofTier},
    proof_store::create_attestation,
    ProofEnvironment,
};

// ---- Proof-gated tests ----

#[test]
fn test_proof_gate_create_and_read() {
    let gate = ProofGate::new(42u32);
    assert_eq!(*gate.read(), 42);
    assert!(gate.attestation_chain().is_empty());
}

#[test]
fn test_proof_gate_dim_mutation_succeeds() {
    let mut gate = ProofGate::new(vec![0.0f32; 128]);
    let result = gate.mutate_with_dim_proof(128, 128, |v| {
        v[0] = 42.0;
    });
    assert!(result.is_ok());
    assert_eq!(gate.read()[0], 42.0);
    assert_eq!(gate.attestation_chain().len(), 1);

    // Verify attestation
    let entry = gate.attestation_chain().latest().unwrap();
    assert_eq!(entry.sequence, 0);
    assert!(entry.attestation.verification_timestamp_ns > 0);
}

#[test]
fn test_proof_gate_dim_mutation_fails_on_mismatch() {
    let mut gate = ProofGate::new(vec![0.0f32; 64]);
    let result = gate.mutate_with_dim_proof(128, 64, |v| {
        v[0] = 1.0; // should not execute
    });
    assert!(result.is_err());
    assert_eq!(gate.read()[0], 0.0); // unchanged
    assert!(gate.attestation_chain().is_empty());
}

#[test]
fn test_proof_gate_routed_mutation() {
    let mut gate = ProofGate::new(100i32);
    let result = gate.mutate_with_routed_proof(ProofKind::Reflexivity, 5, 5, |v| *v += 50);
    assert!(result.is_ok());
    let (decision, attestation) = result.unwrap();
    assert_eq!(decision.tier, ProofTier::Reflex);
    assert_eq!(*gate.read(), 150);
    assert!(attestation.verification_timestamp_ns > 0);
}

#[test]
fn test_proof_gate_pipeline_mutation() {
    let mut gate = ProofGate::new(String::from("initial"));
    let stages = vec![
        ("embed".into(), 1u32, 2u32),
        ("align".into(), 2, 3),
        ("call".into(), 3, 4),
    ];
    let result = gate.mutate_with_pipeline_proof(&stages, |s| {
        *s = String::from("transformed");
    });
    assert!(result.is_ok());
    assert_eq!(gate.read().as_str(), "transformed");
}

#[test]
fn test_attestation_chain_integrity() {
    let mut chain = AttestationChain::new();
    let env = ProofEnvironment::new();
    for i in 0..10 {
        let att = create_attestation(&env, i);
        chain.append(att);
    }
    assert_eq!(chain.len(), 10);
    assert!(chain.verify_integrity());
    assert!(!chain.is_empty());
    assert_ne!(chain.chain_hash(), 0);
}

// ---- Sublinear attention tests ----

#[cfg(feature = "sublinear")]
mod sublinear_tests {
    use ruvector_graph_transformer::config::SublinearConfig;
    use ruvector_graph_transformer::SublinearGraphAttention;

    #[test]
    fn test_lsh_attention_basic() {
        let config = SublinearConfig {
            lsh_buckets: 4,
            ppr_samples: 8,
            sparsification_factor: 0.5,
        };
        let attn = SublinearGraphAttention::new(8, config);

        let features: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32 * 0.1; 8]).collect();

        let result = attn.lsh_attention(&features);
        assert!(result.is_ok());
        let outputs = result.unwrap();
        assert_eq!(outputs.len(), 10);
        for out in &outputs {
            assert_eq!(out.len(), 8);
        }
    }

    #[test]
    fn test_ppr_attention_on_small_graph() {
        let config = SublinearConfig {
            lsh_buckets: 4,
            ppr_samples: 3,
            sparsification_factor: 0.5,
        };
        let attn = SublinearGraphAttention::new(4, config);

        let features = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
            vec![0.0, 0.0, 0.0, 1.0],
            vec![0.5, 0.5, 0.0, 0.0],
        ];
        let edges = vec![
            (0, 1, 1.0),
            (1, 2, 1.0),
            (2, 3, 1.0),
            (3, 4, 1.0),
            (4, 0, 1.0),
        ];

        let result = attn.ppr_attention(&features, &edges);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 5);
    }

    #[test]
    fn test_spectral_attention_on_small_graph() {
        let config = SublinearConfig {
            lsh_buckets: 4,
            ppr_samples: 4,
            sparsification_factor: 0.5,
        };
        let attn = SublinearGraphAttention::new(4, config);

        let features = vec![
            vec![1.0, 0.5, 0.3, 0.1],
            vec![0.5, 1.0, 0.4, 0.2],
            vec![0.3, 0.4, 1.0, 0.5],
        ];
        let edges = vec![(0, 1, 2.0), (1, 2, 1.0), (0, 2, 0.5)];

        let result = attn.spectral_attention(&features, &edges);
        assert!(result.is_ok());
    }
}

// ---- Physics tests ----

#[cfg(feature = "physics")]
mod physics_tests {
    use ruvector_graph_transformer::config::PhysicsConfig;
    use ruvector_graph_transformer::HamiltonianGraphNet;

    #[test]
    fn test_hamiltonian_step_energy_conservation() {
        let config = PhysicsConfig {
            dt: 0.001,
            leapfrog_steps: 1,
            energy_tolerance: 0.1,
        };
        let mut hgn = HamiltonianGraphNet::new(4, config);

        let features = vec![vec![0.1, 0.2, 0.3, 0.4], vec![0.4, 0.3, 0.2, 0.1]];
        let state = hgn.init_state(&features).unwrap();
        let edges = vec![(0, 1, 0.1)];

        let result = hgn.step(&state, &edges).unwrap();
        let energy_diff = (result.energy_after - result.energy_before).abs();
        assert!(
            energy_diff < 0.1,
            "energy not conserved: diff={}",
            energy_diff
        );
        assert!(result.energy_conserved);
        assert!(result.attestation.is_some());
    }
}

// ---- Biological tests ----

#[cfg(feature = "biological")]
mod biological_tests {
    use ruvector_graph_transformer::config::BiologicalConfig;
    use ruvector_graph_transformer::{HebbianLayer, SpikingGraphAttention};

    #[test]
    fn test_spiking_attention_update() {
        let config = BiologicalConfig {
            tau_membrane: 10.0,
            threshold: 0.3,
            stdp_rate: 0.01,
            max_weight: 5.0,
        };
        let mut sga = SpikingGraphAttention::new(3, 4, config);

        let features = vec![
            vec![0.8, 0.6, 0.4, 0.2],
            vec![0.1, 0.2, 0.3, 0.4],
            vec![0.9, 0.7, 0.5, 0.3],
        ];
        let weights = vec![
            vec![0.0, 0.5, 0.3],
            vec![0.5, 0.0, 0.2],
            vec![0.3, 0.2, 0.0],
        ];
        let adjacency = vec![(0, 1), (1, 2), (0, 2)];

        let result = sga.step(&features, &weights, &adjacency).unwrap();
        assert_eq!(result.features.len(), 3);

        // Verify weight bounds
        for row in &result.weights {
            for &w in row {
                assert!(w.abs() <= 5.0, "weight {} exceeds bound", w);
            }
        }
    }

    #[test]
    fn test_hebbian_weight_bounds() {
        let hebb = HebbianLayer::new(4, 1.0, 2.0);
        let pre = vec![1.0, 1.0, 1.0, 1.0];
        let post = vec![1.0, 1.0, 1.0, 1.0];
        let mut weights = vec![0.0; 4];

        for _ in 0..100 {
            hebb.update(&pre, &post, &mut weights).unwrap();
        }
        assert!(hebb.verify_bounds(&weights));
    }
}

// ---- Self-organizing tests ----

#[cfg(feature = "self-organizing")]
mod self_organizing_tests {
    use ruvector_graph_transformer::config::SelfOrganizingConfig;
    use ruvector_graph_transformer::self_organizing::{GrowthRule, GrowthRuleKind};
    use ruvector_graph_transformer::{DevelopmentalProgram, MorphogeneticField};

    #[test]
    fn test_morphogenetic_step_topology_invariants() {
        let config = SelfOrganizingConfig {
            diffusion_rate: 0.05,
            reaction_rate: 0.04,
            max_growth_steps: 100,
            coherence_threshold: 0.0,
        };
        let mut field = MorphogeneticField::new(5, config);

        let edges = vec![(0, 1), (1, 2), (2, 3), (3, 4), (4, 0)];

        for _ in 0..5 {
            let result = field.step(&edges).unwrap();
            // Concentrations must remain bounded [0.0, 2.0]
            for &a in &result.activator {
                assert!(a >= 0.0 && a <= 2.0);
            }
            for &b in &result.inhibitor {
                assert!(b >= 0.0 && b <= 2.0);
            }
            // Bounds-passing step should produce attestation
            assert!(result.attestation.is_some());
        }
    }

    #[test]
    fn test_developmental_growth_rules() {
        let rules = vec![GrowthRule {
            activator_threshold: 0.5,
            max_degree: 3,
            connection_weight: 1.0,
            kind: GrowthRuleKind::Branch,
        }];
        let mut program = DevelopmentalProgram::new(rules, 10);

        let activator = vec![0.8, 0.6, 0.2, 0.9];
        let degrees = vec![1, 1, 1, 1];
        let edges = vec![(0, 1), (2, 3)];

        let result = program.grow_step(&activator, &degrees, &edges).unwrap();
        assert!(result.edges_added > 0);
        assert!(result.attestation.is_some());
    }
}

// ---- Verified training tests ----

#[cfg(feature = "verified-training")]
mod verified_training_tests {
    use ruvector_gnn::RuvectorLayer;
    use ruvector_graph_transformer::config::VerifiedTrainingConfig;
    use ruvector_graph_transformer::{RollbackStrategy, TrainingInvariant, VerifiedTrainer};

    #[test]
    fn test_verified_training_single_step_certificate() {
        let config = VerifiedTrainingConfig {
            lipschitz_bound: 100.0,
            verify_monotonicity: true,
            learning_rate: 0.001,
            ..Default::default()
        };
        let invariants = vec![TrainingInvariant::WeightNormBound {
            max_norm: 1000.0,
            rollback_strategy: RollbackStrategy::DeltaApply,
        }];
        let mut trainer = VerifiedTrainer::new(4, 8, config, invariants);

        let layer = RuvectorLayer::new(4, 8, 2, 0.0).unwrap();
        let features = vec![vec![1.0, 0.0, 0.0, 0.0]];
        let neighbors = vec![vec![]];
        let weights = vec![vec![]];
        let targets = vec![vec![0.0; 8]];

        let result = trainer.train_step(&features, &neighbors, &weights, &targets, &layer);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.step, 1);
        assert!(result.loss >= 0.0);
        assert!(result.weights_committed);
        assert!(result.attestation.verification_timestamp_ns > 0);
    }

    #[test]
    fn test_verified_training_multiple_steps() {
        let config = VerifiedTrainingConfig {
            lipschitz_bound: 100.0,
            verify_monotonicity: false,
            learning_rate: 0.001,
            ..Default::default()
        };
        let invariants = vec![TrainingInvariant::WeightNormBound {
            max_norm: 1000.0,
            rollback_strategy: RollbackStrategy::DeltaApply,
        }];
        let mut trainer = VerifiedTrainer::new(4, 8, config, invariants);

        let layer = RuvectorLayer::new(4, 8, 2, 0.0).unwrap();

        for _ in 0..3 {
            let result = trainer
                .train_step(
                    &[vec![1.0; 4]],
                    &[vec![]],
                    &[vec![]],
                    &[vec![0.0; 8]],
                    &layer,
                )
                .unwrap();
            assert!(result.weights_committed);
        }

        assert_eq!(trainer.step_count(), 3);
        assert_eq!(trainer.step_results().len(), 3);
    }
}

// ---- Manifold tests ----

#[cfg(feature = "manifold")]
mod manifold_tests {
    use ruvector_graph_transformer::config::ManifoldConfig;
    use ruvector_graph_transformer::manifold::{hyperbolic_geodesic, spherical_geodesic};
    use ruvector_graph_transformer::ProductManifoldAttention;

    #[test]
    fn test_product_manifold_attention_curvature() {
        let config = ManifoldConfig {
            spherical_dim: 4,
            hyperbolic_dim: 4,
            euclidean_dim: 4,
            curvature: -1.0,
        };
        let mut attn = ProductManifoldAttention::new(config);
        assert_eq!(attn.total_dim(), 12);

        let query = vec![0.5; 12];
        let keys = vec![vec![0.3; 12], vec![0.7; 12]];
        let values = vec![vec![1.0; 12], vec![2.0; 12]];

        let result = attn.compute(&query, &keys, &values).unwrap();
        assert_eq!(result.output.len(), 12);

        // Verify curvatures
        assert!(result.curvatures.spherical > 0.0);
        assert!(result.curvatures.hyperbolic < 0.0);
        assert!((result.curvatures.euclidean).abs() < 1e-6);
    }

    #[test]
    fn test_spherical_geodesic_distance() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let dist = spherical_geodesic(&a, &b);
        assert!((dist - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    #[test]
    fn test_hyperbolic_geodesic_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![0.1, 0.0];
        let dist = hyperbolic_geodesic(&a, &b, -1.0);
        assert!(dist > 0.0);
        assert!(dist.is_finite());
    }
}

// ---- Temporal tests ----

#[cfg(feature = "temporal")]
mod temporal_tests {
    use ruvector_graph_transformer::config::TemporalConfig;
    use ruvector_graph_transformer::CausalGraphTransformer;

    #[test]
    fn test_causal_attention_ordering() {
        let config = TemporalConfig {
            decay_rate: 0.9,
            max_lag: 10,
            granger_lags: 3,
        };
        let transformer = CausalGraphTransformer::new(4, config);

        let sequence = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
            vec![0.0, 0.0, 0.0, 1.0],
            vec![0.5, 0.5, 0.0, 0.0],
        ];

        let result = transformer.temporal_attention(&sequence).unwrap();
        assert_eq!(result.output.len(), 5);
        assert_eq!(result.attention_weights.len(), 5);

        // Verify causal ordering: no future attention
        assert!(transformer.verify_causal_ordering(&result.attention_weights));
    }

    #[test]
    fn test_granger_causality_extraction() {
        let config = TemporalConfig {
            decay_rate: 0.9,
            max_lag: 5,
            granger_lags: 2,
        };
        let transformer = CausalGraphTransformer::new(4, config);

        let mut series = Vec::new();
        for t in 0..30 {
            let x = (t as f32 * 0.1).sin();
            let y = (t as f32 * 0.2).cos();
            series.push(vec![x, y, 0.0, 0.0]);
        }

        let result = transformer.granger_causality(&series, 0, 1).unwrap();
        assert_eq!(result.source, 0);
        assert_eq!(result.target, 1);
        assert_eq!(result.lags, 2);
        assert!(result.f_statistic >= 0.0);
    }
}

// ---- Integration: Composing multiple modules ----

#[test]
fn test_graph_transformer_unified_entry() {
    let config = GraphTransformerConfig::default();
    let gt = GraphTransformer::new(config);
    assert_eq!(gt.embed_dim(), 64);

    let gate = gt.create_gate(vec![1.0, 2.0, 3.0]);
    assert_eq!(gate.read().len(), 3);
}

#[test]
fn test_proof_gate_multiple_mutations() {
    let mut gate = ProofGate::new(0u64);

    for i in 1..=5u32 {
        let result = gate.mutate_with_dim_proof(i, i, |v| *v += 1);
        assert!(result.is_ok());
    }

    assert_eq!(*gate.read(), 5);
    assert_eq!(gate.attestation_chain().len(), 5);
    assert!(gate.attestation_chain().verify_integrity());
}
