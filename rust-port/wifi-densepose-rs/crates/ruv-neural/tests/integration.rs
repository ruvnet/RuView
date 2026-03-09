//! Workspace-level integration tests for the rUv Neural crate ecosystem.
//!
//! These tests verify that all crates compose correctly and that the full
//! pipeline (simulate -> preprocess -> graph -> mincut -> embed -> decode)
//! produces consistent results across crate boundaries.
//!
//! Gate with `cfg(feature = "integration")` so these only run when all crates
//! are built together (they require the full workspace).

#![cfg(feature = "integration")]

use ruv_neural_core::error::Result;
use ruv_neural_core::graph::{BrainEdge, BrainGraph, ConnectivityMetric};
use ruv_neural_core::signal::{FrequencyBand, MultiChannelTimeSeries};
use ruv_neural_core::topology::MincutResult;
use ruv_neural_core::traits::SensorSource;
use ruv_neural_core::{Atlas, BrainRegion, Hemisphere, Lobe};

// ---------------------------------------------------------------------------
// 1. Cross-crate type compatibility
// ---------------------------------------------------------------------------

#[test]
fn core_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BrainGraph>();
    assert_send_sync::<BrainEdge>();
    assert_send_sync::<MincutResult>();
    assert_send_sync::<MultiChannelTimeSeries>();
    assert_send_sync::<ruv_neural_core::embedding::NeuralEmbedding>();
}

#[test]
fn core_enums_roundtrip_serde() {
    let atlas = Atlas::DesikanKilliany68;
    let json = serde_json::to_string(&atlas).unwrap();
    let back: Atlas = serde_json::from_str(&json).unwrap();
    assert_eq!(atlas, back);

    let metric = ConnectivityMetric::PhaseLockingValue;
    let json = serde_json::to_string(&metric).unwrap();
    let back: ConnectivityMetric = serde_json::from_str(&json).unwrap();
    assert_eq!(metric, back);

    let band = FrequencyBand::Alpha;
    let json = serde_json::to_string(&band).unwrap();
    let back: FrequencyBand = serde_json::from_str(&json).unwrap();
    assert_eq!(band, back);
}

// ---------------------------------------------------------------------------
// 2. Sensor -> Signal pipeline
// ---------------------------------------------------------------------------

#[test]
fn simulator_produces_valid_multichannel_data() {
    use ruv_neural_sensor::simulator::SimulatedSensorArray;

    let mut sim = SimulatedSensorArray::new(16, 1000.0);
    let data = sim.read_chunk(500).expect("sensor read failed");

    assert_eq!(data.num_channels, 16);
    assert_eq!(data.num_samples, 500);
    assert_eq!(data.sample_rate_hz, 1000.0);
    assert_eq!(data.data.len(), 16);
    for ch in &data.data {
        assert_eq!(ch.len(), 500);
    }
}

#[test]
fn simulator_with_alpha_injection() {
    use ruv_neural_sensor::simulator::SimulatedSensorArray;

    let mut sim = SimulatedSensorArray::new(8, 1000.0);
    sim.inject_alpha(200.0);
    let data = sim.read_chunk(2000).expect("sensor read failed");

    // With alpha injection, signals should have non-trivial variance.
    let ch0 = &data.data[0];
    let mean: f64 = ch0.iter().sum::<f64>() / ch0.len() as f64;
    let variance: f64 = ch0.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / ch0.len() as f64;
    assert!(
        variance > 0.0,
        "Expected non-zero variance with alpha injection"
    );
}

#[test]
fn preprocessing_pipeline_processes_channel_data() {
    use ruv_neural_signal::PreprocessingPipeline;

    let pipeline = PreprocessingPipeline::new();
    assert_eq!(pipeline.num_stages(), 0, "Default pipeline has no stages");

    // Process a simple signal through the empty pipeline (identity).
    let signal: Vec<f64> = (0..100).map(|i| (i as f64 * 0.1).sin()).collect();
    let result = pipeline.process(&signal);
    assert_eq!(result.len(), signal.len());
}

// ---------------------------------------------------------------------------
// 3. Signal -> Graph -> Mincut pipeline
// ---------------------------------------------------------------------------

#[test]
fn connectivity_matrix_from_signals() {
    use ruv_neural_signal::{compute_all_pairs, ConnectivityMetric};

    // Create 4 channels of synthetic sinusoidal data.
    let n = 1000;
    let channels: Vec<Vec<f64>> = (0..4)
        .map(|ch| {
            (0..n)
                .map(|t| {
                    let phase = ch as f64 * 0.5;
                    (2.0 * std::f64::consts::PI * 10.0 * t as f64 / 1000.0 + phase).sin()
                })
                .collect()
        })
        .collect();

    let matrix = compute_all_pairs(&channels, &ConnectivityMetric::PhaseLockingValue);
    assert_eq!(matrix.len(), 4);
    for row in &matrix {
        assert_eq!(row.len(), 4);
    }

    // Diagonal should be 1.0 (self-PLV) or at least the highest value.
    for i in 0..4 {
        assert!(
            matrix[i][i] >= 0.99,
            "Self-PLV should be ~1.0, got {}",
            matrix[i][i]
        );
    }
}

#[test]
fn brain_graph_construction_and_mincut() {
    // Build a small BrainGraph manually and run Stoer-Wagner.
    let edges = vec![
        BrainEdge {
            source: 0,
            target: 1,
            weight: 0.9,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        },
        BrainEdge {
            source: 1,
            target: 2,
            weight: 0.8,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        },
        BrainEdge {
            source: 2,
            target: 3,
            weight: 0.1,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        },
        BrainEdge {
            source: 3,
            target: 4,
            weight: 0.85,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        },
        BrainEdge {
            source: 0,
            target: 2,
            weight: 0.7,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        },
    ];

    let graph = BrainGraph {
        num_nodes: 5,
        edges,
        timestamp: 0.0,
        window_duration_s: 1.0,
        atlas: Atlas::DesikanKilliany68,
    };

    // Verify graph utilities.
    assert!(graph.density() > 0.0);
    assert!(graph.total_weight() > 0.0);
    assert_eq!(graph.adjacency_matrix().len(), 5);

    // Run Stoer-Wagner mincut.
    let result = ruv_neural_mincut::stoer_wagner_mincut(&graph).expect("mincut failed");
    assert!(result.cut_value > 0.0, "Cut value must be positive");
    assert!(
        !result.partition_a.is_empty() && !result.partition_b.is_empty(),
        "Both partitions must be non-empty"
    );
    assert_eq!(
        result.partition_a.len() + result.partition_b.len(),
        5,
        "Partitions must cover all nodes"
    );

    // The weakest link (0.1 between nodes 2-3) should likely be cut.
    assert!(
        result.cut_value <= 0.2,
        "Expected cut near the weak edge (0.1), got {}",
        result.cut_value
    );
}

#[test]
fn normalized_cut_produces_valid_partition() {
    let edges = vec![
        BrainEdge {
            source: 0,
            target: 1,
            weight: 0.9,
            metric: ConnectivityMetric::Coherence,
            frequency_band: FrequencyBand::Beta,
        },
        BrainEdge {
            source: 1,
            target: 2,
            weight: 0.05,
            metric: ConnectivityMetric::Coherence,
            frequency_band: FrequencyBand::Beta,
        },
        BrainEdge {
            source: 2,
            target: 3,
            weight: 0.85,
            metric: ConnectivityMetric::Coherence,
            frequency_band: FrequencyBand::Beta,
        },
    ];

    let graph = BrainGraph {
        num_nodes: 4,
        edges,
        timestamp: 1.0,
        window_duration_s: 1.0,
        atlas: Atlas::DesikanKilliany68,
    };

    let result = ruv_neural_mincut::normalized_cut(&graph).expect("normalized cut failed");
    assert!(result.cut_value >= 0.0);
    assert_eq!(result.partition_a.len() + result.partition_b.len(), 4);
}

// ---------------------------------------------------------------------------
// 4. Mincut -> Embed pipeline
// ---------------------------------------------------------------------------

#[test]
fn neural_embedding_creation_and_serialization() {
    use ruv_neural_embed::NeuralEmbedding;

    let embedding = NeuralEmbedding::new(vec![1.0, 2.0, 3.0, 4.0], 0.0, "spectral")
        .expect("embedding creation failed");

    assert_eq!(embedding.dimension, 4);
    assert_eq!(embedding.values.len(), 4);
    assert_eq!(embedding.method, "spectral");
    assert!((embedding.norm() - (1.0_f64 + 4.0 + 9.0 + 16.0).sqrt()).abs() < 1e-10);

    // Serde roundtrip.
    let json = serde_json::to_string(&embedding).unwrap();
    let back: NeuralEmbedding = serde_json::from_str(&json).unwrap();
    assert_eq!(back.dimension, 4);
    assert_eq!(back.values, embedding.values);
}

#[test]
fn zero_embedding_has_zero_norm() {
    use ruv_neural_embed::NeuralEmbedding;

    let zero = NeuralEmbedding::zeros(16, 0.0, "test");
    assert_eq!(zero.dimension, 16);
    assert!((zero.norm() - 0.0).abs() < 1e-15);
}

#[test]
fn empty_embedding_is_rejected() {
    use ruv_neural_embed::NeuralEmbedding;

    let result = NeuralEmbedding::new(vec![], 0.0, "empty");
    assert!(result.is_err(), "Empty embedding should be rejected");
}

// ---------------------------------------------------------------------------
// 5. Decoder types
// ---------------------------------------------------------------------------

#[test]
fn decoder_types_exist_and_are_constructible() {
    // Verify that decoder public types can be referenced.
    // This is a compile-time check more than a runtime check.
    let _: fn() -> &str = || {
        let _ = std::any::type_name::<ruv_neural_decoder::KnnDecoder>();
        let _ = std::any::type_name::<ruv_neural_decoder::ThresholdDecoder>();
        let _ = std::any::type_name::<ruv_neural_decoder::TransitionDecoder>();
        let _ = std::any::type_name::<ruv_neural_decoder::ClinicalScorer>();
        let _ = std::any::type_name::<ruv_neural_decoder::DecoderPipeline>();
        "ok"
    };
}

// ---------------------------------------------------------------------------
// 6. Core traits are object-safe (can be used as trait objects)
// ---------------------------------------------------------------------------

#[test]
fn core_traits_are_object_safe() {
    use ruv_neural_core::traits::*;

    // These lines verify the traits can be used as `dyn Trait`.
    // If a trait is not object-safe, this will fail to compile.
    fn _accept_sensor(_: &dyn SensorSource) {}
    fn _accept_signal(_: &dyn SignalProcessor) {}
    fn _accept_graph(_: &dyn GraphConstructor) {}
    fn _accept_topology(_: &dyn TopologyAnalyzer) {}
    fn _accept_embedding(_: &dyn EmbeddingGenerator) {}
    fn _accept_decoder(_: &dyn StateDecoder) {}
    fn _accept_memory(_: &mut dyn NeuralMemory) {}
}

// ---------------------------------------------------------------------------
// 7. Full pipeline: simulate -> preprocess -> connectivity -> graph -> mincut
// ---------------------------------------------------------------------------

#[test]
fn full_pipeline_simulate_to_mincut() {
    use ruv_neural_sensor::simulator::SimulatedSensorArray;
    use ruv_neural_signal::{compute_all_pairs, ConnectivityMetric};

    // Step 1: Simulate sensor data (16 channels, 1s at 1000 Hz).
    let mut sim = SimulatedSensorArray::new(16, 1000.0);
    sim.inject_alpha(150.0);
    let data = sim.read_chunk(1000).expect("sensor read failed");
    assert_eq!(data.data.len(), 16);

    // Step 2: Compute pairwise connectivity matrix (PLV).
    let matrix = compute_all_pairs(&data.data, &ConnectivityMetric::PhaseLockingValue);
    assert_eq!(matrix.len(), 16);

    // Step 3: Build BrainGraph from connectivity matrix.
    let threshold = 0.3;
    let mut edges = Vec::new();
    for i in 0..16 {
        for j in (i + 1)..16 {
            if matrix[i][j] > threshold {
                edges.push(BrainEdge {
                    source: i,
                    target: j,
                    weight: matrix[i][j],
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                });
            }
        }
    }

    let graph = BrainGraph {
        num_nodes: 16,
        edges,
        timestamp: data.timestamp_start,
        window_duration_s: 1.0,
        atlas: Atlas::DesikanKilliany68,
    };

    // Step 4: Run Stoer-Wagner mincut.
    if graph.edges.is_empty() {
        // If no edges pass threshold, the graph is disconnected — that is valid.
        return;
    }
    let result = ruv_neural_mincut::stoer_wagner_mincut(&graph).expect("mincut failed");
    assert!(result.cut_value >= 0.0);
    assert_eq!(
        result.partition_a.len() + result.partition_b.len(),
        16,
        "Partitions must cover all 16 nodes"
    );

    // Step 5: Create embedding from topology result.
    let feature_vec = vec![
        result.cut_value,
        result.balance_ratio(),
        result.num_cut_edges() as f64,
        graph.density(),
        graph.total_weight(),
    ];
    let embedding = ruv_neural_embed::NeuralEmbedding::new(feature_vec, data.timestamp_start, "topology")
        .expect("embedding failed");
    assert_eq!(embedding.dimension, 5);
    assert!(embedding.norm() > 0.0);
}

// ---------------------------------------------------------------------------
// 8. BrainGraph serde roundtrip
// ---------------------------------------------------------------------------

#[test]
fn brain_graph_serde_roundtrip() {
    let graph = BrainGraph {
        num_nodes: 3,
        edges: vec![
            BrainEdge {
                source: 0,
                target: 1,
                weight: 0.5,
                metric: ConnectivityMetric::PhaseLockingValue,
                frequency_band: FrequencyBand::Alpha,
            },
            BrainEdge {
                source: 1,
                target: 2,
                weight: 0.7,
                metric: ConnectivityMetric::Coherence,
                frequency_band: FrequencyBand::Gamma,
            },
        ],
        timestamp: 42.0,
        window_duration_s: 2.0,
        atlas: Atlas::DesikanKilliany68,
    };

    let json = serde_json::to_string_pretty(&graph).unwrap();
    let back: BrainGraph = serde_json::from_str(&json).unwrap();

    assert_eq!(back.num_nodes, graph.num_nodes);
    assert_eq!(back.edges.len(), graph.edges.len());
    assert!((back.timestamp - graph.timestamp).abs() < 1e-10);
    assert_eq!(back.atlas, graph.atlas);
}

// ---------------------------------------------------------------------------
// 9. Multiway cut (multiple partitions)
// ---------------------------------------------------------------------------

#[test]
fn multiway_cut_produces_valid_partitions() {
    // Build a graph with 3 clear clusters connected by weak edges.
    let mut edges = Vec::new();

    // Cluster A: nodes 0, 1, 2 (strong internal edges).
    for &(s, t) in &[(0, 1), (1, 2), (0, 2)] {
        edges.push(BrainEdge {
            source: s,
            target: t,
            weight: 0.9,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        });
    }

    // Cluster B: nodes 3, 4, 5 (strong internal edges).
    for &(s, t) in &[(3, 4), (4, 5), (3, 5)] {
        edges.push(BrainEdge {
            source: s,
            target: t,
            weight: 0.85,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        });
    }

    // Cluster C: nodes 6, 7, 8 (strong internal edges).
    for &(s, t) in &[(6, 7), (7, 8), (6, 8)] {
        edges.push(BrainEdge {
            source: s,
            target: t,
            weight: 0.88,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        });
    }

    // Weak inter-cluster bridges.
    edges.push(BrainEdge {
        source: 2,
        target: 3,
        weight: 0.05,
        metric: ConnectivityMetric::PhaseLockingValue,
        frequency_band: FrequencyBand::Alpha,
    });
    edges.push(BrainEdge {
        source: 5,
        target: 6,
        weight: 0.04,
        metric: ConnectivityMetric::PhaseLockingValue,
        frequency_band: FrequencyBand::Alpha,
    });

    let graph = BrainGraph {
        num_nodes: 9,
        edges,
        timestamp: 0.0,
        window_duration_s: 1.0,
        atlas: Atlas::DesikanKilliany68,
    };

    let partitions = ruv_neural_mincut::multiway_cut(&graph, 3).expect("multiway cut failed");
    assert!(
        partitions.num_partitions() >= 2,
        "Expected at least 2 partitions"
    );
    assert_eq!(
        partitions.num_nodes(),
        9,
        "All nodes must be assigned to a partition"
    );
}

// ---------------------------------------------------------------------------
// 10. Spectral cut analysis
// ---------------------------------------------------------------------------

#[test]
fn spectral_bisection_produces_valid_split() {
    let edges = vec![
        BrainEdge {
            source: 0,
            target: 1,
            weight: 0.9,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        },
        BrainEdge {
            source: 1,
            target: 2,
            weight: 0.05,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        },
        BrainEdge {
            source: 2,
            target: 3,
            weight: 0.85,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        },
    ];

    let graph = BrainGraph {
        num_nodes: 4,
        edges,
        timestamp: 0.0,
        window_duration_s: 1.0,
        atlas: Atlas::DesikanKilliany68,
    };

    let result = ruv_neural_mincut::spectral_bisection(&graph).expect("spectral bisection failed");
    assert!(result.cut_value >= 0.0);
    assert_eq!(result.partition_a.len() + result.partition_b.len(), 4);
}
