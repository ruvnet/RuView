//! # ruv-neural-core
//!
//! Core types, traits, and error types for the ruv-neural brain topology
//! analysis system.
//!
//! This crate is the foundation of the ruv-neural workspace. It has **zero**
//! internal dependencies — all other ruv-neural crates depend on this one.
//!
//! ## Modules
//!
//! | Module      | Contents                                          |
//! |-------------|---------------------------------------------------|
//! | `error`     | `RuvNeuralError` enum, `Result<T>` alias           |
//! | `sensor`    | `SensorType`, `SensorChannel`, `SensorArray`       |
//! | `signal`    | `MultiChannelTimeSeries`, `FrequencyBand`, spectra |
//! | `brain`     | `Atlas`, `BrainRegion`, `Parcellation`             |
//! | `graph`     | `BrainGraph`, `BrainEdge`, `ConnectivityMetric`    |
//! | `topology`  | `MincutResult`, `CognitiveState`, `TopologyMetrics`|
//! | `embedding` | `NeuralEmbedding`, `EmbeddingTrajectory`           |
//! | `rvf`       | RuVector File format header and I/O                |
//! | `traits`    | Pipeline trait definitions for all crates          |

pub mod brain;
pub mod embedding;
pub mod error;
pub mod graph;
pub mod rvf;
pub mod sensor;
pub mod signal;
pub mod topology;
pub mod traits;
pub mod witness;

// Re-export the most commonly used types at crate root.
pub use brain::{Atlas, BrainRegion, Hemisphere, Lobe, Parcellation};
pub use embedding::{EmbeddingMetadata, EmbeddingTrajectory, NeuralEmbedding};
pub use error::{Result, RuvNeuralError};
pub use graph::{BrainEdge, BrainGraph, BrainGraphSequence, ConnectivityMetric};
pub use rvf::{RvfDataType, RvfFile, RvfHeader};
pub use sensor::{SensorArray, SensorChannel, SensorType};
pub use signal::{FrequencyBand, MultiChannelTimeSeries, SpectralFeatures, TimeFrequencyMap};
pub use topology::{
    CognitiveState, MincutResult, MultiPartition, SleepStage, TopologyMetrics,
};
pub use traits::{
    EmbeddingGenerator, GraphConstructor, NeuralMemory, RvfSerializable, SensorSource,
    SignalProcessor, StateDecoder, TopologyAnalyzer,
};

#[cfg(test)]
mod tests {
    use super::*;

    // ── Error tests ─────────────────────────────────────────────────

    #[test]
    fn error_display_formatting() {
        let err = RuvNeuralError::Sensor("calibration failed".into());
        assert!(err.to_string().contains("Sensor error"));
        assert!(err.to_string().contains("calibration failed"));

        let err = RuvNeuralError::DimensionMismatch {
            expected: 68,
            got: 100,
        };
        assert!(err.to_string().contains("68"));
        assert!(err.to_string().contains("100"));

        let err = RuvNeuralError::ChannelOutOfRange {
            channel: 5,
            max: 3,
        };
        assert!(err.to_string().contains("5"));
        assert!(err.to_string().contains("3"));

        let err = RuvNeuralError::InsufficientData {
            needed: 1000,
            have: 500,
        };
        assert!(err.to_string().contains("1000"));
        assert!(err.to_string().contains("500"));
    }

    // ── Sensor tests ────────────────────────────────────────────────

    #[test]
    fn sensor_type_sensitivity() {
        assert!(SensorType::SquidMeg.typical_sensitivity_ft_sqrt_hz() < 5.0);
        assert!(SensorType::Eeg.typical_sensitivity_ft_sqrt_hz() > 100.0);
    }

    #[test]
    fn sensor_array_operations() {
        let array = SensorArray {
            channels: vec![
                SensorChannel {
                    id: 0,
                    sensor_type: SensorType::Opm,
                    position: [0.0, 0.0, 0.1],
                    orientation: [0.0, 0.0, 1.0],
                    sensitivity_ft_sqrt_hz: 7.0,
                    sample_rate_hz: 1000.0,
                    label: "OPM-001".into(),
                },
                SensorChannel {
                    id: 1,
                    sensor_type: SensorType::Opm,
                    position: [0.05, 0.0, 0.12],
                    orientation: [0.0, 0.0, 1.0],
                    sensitivity_ft_sqrt_hz: 7.0,
                    sample_rate_hz: 1000.0,
                    label: "OPM-002".into(),
                },
            ],
            sensor_type: SensorType::Opm,
            name: "OPM array".into(),
        };

        assert_eq!(array.num_channels(), 2);
        assert!(!array.is_empty());
        assert_eq!(array.get_channel(0).unwrap().label, "OPM-001");
        assert!(array.get_channel(5).is_none());

        let (min, max) = array.bounding_box().unwrap();
        assert_eq!(min[0], 0.0);
        assert_eq!(max[0], 0.05);
    }

    #[test]
    fn sensor_serialize_roundtrip() {
        let ch = SensorChannel {
            id: 0,
            sensor_type: SensorType::NvDiamond,
            position: [1.0, 2.0, 3.0],
            orientation: [0.0, 0.0, 1.0],
            sensitivity_ft_sqrt_hz: 10.0,
            sample_rate_hz: 2000.0,
            label: "NV-001".into(),
        };
        let json = serde_json::to_string(&ch).unwrap();
        let ch2: SensorChannel = serde_json::from_str(&json).unwrap();
        assert_eq!(ch2.id, 0);
        assert_eq!(ch2.sensor_type, SensorType::NvDiamond);
    }

    // ── Signal tests ────────────────────────────────────────────────

    #[test]
    fn frequency_band_ranges() {
        assert_eq!(FrequencyBand::Delta.range_hz(), (1.0, 4.0));
        assert_eq!(FrequencyBand::Alpha.range_hz(), (8.0, 13.0));
        assert_eq!(FrequencyBand::Gamma.range_hz(), (30.0, 100.0));
        assert_eq!(
            FrequencyBand::Custom {
                low_hz: 50.0,
                high_hz: 70.0
            }
            .range_hz(),
            (50.0, 70.0)
        );
    }

    #[test]
    fn frequency_band_center_and_bandwidth() {
        assert!((FrequencyBand::Alpha.center_hz() - 10.5).abs() < 1e-10);
        assert!((FrequencyBand::Alpha.bandwidth_hz() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn time_series_creation_valid() {
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let ts = MultiChannelTimeSeries::new(data, 100.0, 1000.0).unwrap();
        assert_eq!(ts.num_channels, 2);
        assert_eq!(ts.num_samples, 3);
        assert!((ts.duration_s() - 0.03).abs() < 1e-10);
    }

    #[test]
    fn time_series_dimension_mismatch() {
        let data = vec![vec![1.0, 2.0], vec![3.0]];
        let result = MultiChannelTimeSeries::new(data, 100.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn time_series_channel_access() {
        let data = vec![vec![10.0, 20.0], vec![30.0, 40.0]];
        let ts = MultiChannelTimeSeries::new(data, 100.0, 0.0).unwrap();
        assert_eq!(ts.channel(0).unwrap(), &[10.0, 20.0]);
        assert!(ts.channel(5).is_err());
    }

    // ── Brain / Atlas tests ─────────────────────────────────────────

    #[test]
    fn atlas_region_counts() {
        assert_eq!(Atlas::DesikanKilliany68.num_regions(), 68);
        assert_eq!(Atlas::Destrieux148.num_regions(), 148);
        assert_eq!(Atlas::Schaefer100.num_regions(), 100);
        assert_eq!(Atlas::Schaefer200.num_regions(), 200);
        assert_eq!(Atlas::Schaefer400.num_regions(), 400);
        assert_eq!(Atlas::Custom(42).num_regions(), 42);
    }

    #[test]
    fn parcellation_query() {
        let parcellation = Parcellation {
            atlas: Atlas::Custom(3),
            regions: vec![
                BrainRegion {
                    id: 0,
                    name: "left_frontal".into(),
                    hemisphere: Hemisphere::Left,
                    lobe: Lobe::Frontal,
                    centroid: [-30.0, 20.0, 40.0],
                },
                BrainRegion {
                    id: 1,
                    name: "right_frontal".into(),
                    hemisphere: Hemisphere::Right,
                    lobe: Lobe::Frontal,
                    centroid: [30.0, 20.0, 40.0],
                },
                BrainRegion {
                    id: 2,
                    name: "left_temporal".into(),
                    hemisphere: Hemisphere::Left,
                    lobe: Lobe::Temporal,
                    centroid: [-50.0, -10.0, 0.0],
                },
            ],
        };

        assert_eq!(parcellation.num_regions(), 3);
        assert_eq!(
            parcellation.regions_in_hemisphere(Hemisphere::Left).len(),
            2
        );
        assert_eq!(parcellation.regions_in_lobe(Lobe::Frontal).len(), 2);
        assert_eq!(parcellation.regions_in_lobe(Lobe::Temporal).len(), 1);
        assert!(parcellation.get_region(1).is_some());
        assert!(parcellation.get_region(99).is_none());
    }

    #[test]
    fn brain_region_serialize_roundtrip() {
        let region = BrainRegion {
            id: 42,
            name: "postcentral".into(),
            hemisphere: Hemisphere::Left,
            lobe: Lobe::Parietal,
            centroid: [-40.0, -25.0, 55.0],
        };
        let json = serde_json::to_string(&region).unwrap();
        let r2: BrainRegion = serde_json::from_str(&json).unwrap();
        assert_eq!(r2.id, 42);
        assert_eq!(r2.hemisphere, Hemisphere::Left);
    }

    // ── Graph tests ─────────────────────────────────────────────────

    #[test]
    fn brain_graph_adjacency_matrix() {
        let graph = BrainGraph {
            num_nodes: 3,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 0.8,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 1,
                    target: 2,
                    weight: 0.5,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Beta,
                },
            ],
            timestamp: 100.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(3),
        };

        let mat = graph.adjacency_matrix();
        assert_eq!(mat.len(), 3);
        assert!((mat[0][1] - 0.8).abs() < 1e-10);
        assert!((mat[1][0] - 0.8).abs() < 1e-10);
        assert!((mat[1][2] - 0.5).abs() < 1e-10);
        assert!((mat[0][2] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn brain_graph_edge_weight_lookup() {
        let graph = BrainGraph {
            num_nodes: 2,
            edges: vec![BrainEdge {
                source: 0,
                target: 1,
                weight: 0.9,
                metric: ConnectivityMetric::MutualInformation,
                frequency_band: FrequencyBand::Gamma,
            }],
            timestamp: 0.0,
            window_duration_s: 0.5,
            atlas: Atlas::Custom(2),
        };

        assert!((graph.edge_weight(0, 1).unwrap() - 0.9).abs() < 1e-10);
        assert!((graph.edge_weight(1, 0).unwrap() - 0.9).abs() < 1e-10);
        assert!(graph.edge_weight(0, 0).is_none());
    }

    #[test]
    fn brain_graph_node_degree() {
        let graph = BrainGraph {
            num_nodes: 3,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 0.3,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 0,
                    target: 2,
                    weight: 0.7,
                    metric: ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(3),
        };

        assert!((graph.node_degree(0) - 1.0).abs() < 1e-10);
        assert!((graph.node_degree(1) - 0.3).abs() < 1e-10);
        assert!((graph.node_degree(2) - 0.7).abs() < 1e-10);
    }

    #[test]
    fn brain_graph_density() {
        let graph = BrainGraph {
            num_nodes: 4,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 1.0,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 2,
                    target: 3,
                    weight: 1.0,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 0,
                    target: 3,
                    weight: 1.0,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                },
            ],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        };

        assert!((graph.density() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn graph_sequence_duration() {
        let seq = BrainGraphSequence {
            graphs: vec![
                BrainGraph {
                    num_nodes: 2,
                    edges: vec![],
                    timestamp: 0.0,
                    window_duration_s: 1.0,
                    atlas: Atlas::Custom(2),
                },
                BrainGraph {
                    num_nodes: 2,
                    edges: vec![],
                    timestamp: 0.5,
                    window_duration_s: 1.0,
                    atlas: Atlas::Custom(2),
                },
                BrainGraph {
                    num_nodes: 2,
                    edges: vec![],
                    timestamp: 1.0,
                    window_duration_s: 1.0,
                    atlas: Atlas::Custom(2),
                },
            ],
            window_step_s: 0.5,
        };

        assert_eq!(seq.len(), 3);
        assert!(!seq.is_empty());
        assert!((seq.duration_s() - 2.0).abs() < 1e-10);
    }

    // ── Topology tests ──────────────────────────────────────────────

    #[test]
    fn mincut_result_properties() {
        let result = MincutResult {
            cut_value: 1.5,
            partition_a: vec![0, 1],
            partition_b: vec![2, 3, 4],
            cut_edges: vec![(1, 2, 0.8), (0, 3, 0.7)],
            timestamp: 100.0,
        };

        assert_eq!(result.num_nodes(), 5);
        assert_eq!(result.num_cut_edges(), 2);
        assert!((result.balance_ratio() - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn multi_partition_properties() {
        let mp = MultiPartition {
            partitions: vec![vec![0, 1], vec![2, 3], vec![4]],
            cut_value: 2.0,
            modularity: 0.4,
        };
        assert_eq!(mp.num_partitions(), 3);
        assert_eq!(mp.num_nodes(), 5);
    }

    #[test]
    fn cognitive_state_serialize_roundtrip() {
        let states = vec![
            CognitiveState::Rest,
            CognitiveState::Focused,
            CognitiveState::Sleep(SleepStage::Rem),
            CognitiveState::Unknown,
        ];
        let json = serde_json::to_string(&states).unwrap();
        let deserialized: Vec<CognitiveState> = serde_json::from_str(&json).unwrap();
        assert_eq!(states, deserialized);
    }

    // ── Embedding tests ─────────────────────────────────────────────

    #[test]
    fn embedding_creation_and_norm() {
        let meta = EmbeddingMetadata {
            subject_id: Some("sub-01".into()),
            session_id: Some("ses-01".into()),
            cognitive_state: Some(CognitiveState::Focused),
            source_atlas: Atlas::Schaefer100,
            embedding_method: "spectral".into(),
        };
        let emb = NeuralEmbedding::new(vec![3.0, 4.0], 1000.0, meta).unwrap();
        assert_eq!(emb.dimension, 2);
        assert!((emb.norm() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn embedding_cosine_similarity() {
        let meta = || EmbeddingMetadata {
            subject_id: None,
            session_id: None,
            cognitive_state: None,
            source_atlas: Atlas::Custom(2),
            embedding_method: "test".into(),
        };

        let a = NeuralEmbedding::new(vec![1.0, 0.0], 0.0, meta()).unwrap();
        let b = NeuralEmbedding::new(vec![1.0, 0.0], 0.0, meta()).unwrap();
        let c = NeuralEmbedding::new(vec![0.0, 1.0], 0.0, meta()).unwrap();

        assert!((a.cosine_similarity(&b).unwrap() - 1.0).abs() < 1e-10);
        assert!((a.cosine_similarity(&c).unwrap() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn embedding_euclidean_distance() {
        let meta = || EmbeddingMetadata {
            subject_id: None,
            session_id: None,
            cognitive_state: None,
            source_atlas: Atlas::Custom(2),
            embedding_method: "test".into(),
        };

        let a = NeuralEmbedding::new(vec![0.0, 0.0], 0.0, meta()).unwrap();
        let b = NeuralEmbedding::new(vec![3.0, 4.0], 0.0, meta()).unwrap();
        assert!((a.euclidean_distance(&b).unwrap() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn embedding_dimension_mismatch() {
        let meta = || EmbeddingMetadata {
            subject_id: None,
            session_id: None,
            cognitive_state: None,
            source_atlas: Atlas::Custom(2),
            embedding_method: "test".into(),
        };

        let a = NeuralEmbedding::new(vec![1.0, 2.0], 0.0, meta()).unwrap();
        let b = NeuralEmbedding::new(vec![1.0, 2.0, 3.0], 0.0, meta()).unwrap();
        assert!(a.cosine_similarity(&b).is_err());
        assert!(a.euclidean_distance(&b).is_err());
    }

    #[test]
    fn embedding_trajectory() {
        let meta = || EmbeddingMetadata {
            subject_id: None,
            session_id: None,
            cognitive_state: None,
            source_atlas: Atlas::Custom(2),
            embedding_method: "test".into(),
        };

        let traj = EmbeddingTrajectory {
            embeddings: vec![
                NeuralEmbedding::new(vec![1.0], 0.0, meta()).unwrap(),
                NeuralEmbedding::new(vec![2.0], 1.0, meta()).unwrap(),
                NeuralEmbedding::new(vec![3.0], 2.0, meta()).unwrap(),
            ],
            timestamps: vec![0.0, 1.0, 2.0],
        };

        assert_eq!(traj.len(), 3);
        assert!(!traj.is_empty());
        assert!((traj.duration_s() - 2.0).abs() < 1e-10);
    }

    // ── RVF tests ───────────────────────────────────────────────────

    #[test]
    fn rvf_data_type_tag_roundtrip() {
        for dt in [
            RvfDataType::BrainGraph,
            RvfDataType::NeuralEmbedding,
            RvfDataType::TopologyMetrics,
            RvfDataType::MincutResult,
            RvfDataType::TimeSeriesChunk,
        ] {
            let tag = dt.to_tag();
            let recovered = RvfDataType::from_tag(tag).unwrap();
            assert_eq!(dt, recovered);
        }
        assert!(RvfDataType::from_tag(255).is_err());
    }

    #[test]
    fn rvf_header_encode_decode() {
        let header = RvfHeader::new(RvfDataType::NeuralEmbedding, 42, 128);
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), 22);

        let decoded = RvfHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.magic, rvf::RVF_MAGIC);
        assert_eq!(decoded.version, rvf::RVF_VERSION);
        assert_eq!(decoded.data_type, RvfDataType::NeuralEmbedding);
        assert_eq!(decoded.num_entries, 42);
        assert_eq!(decoded.embedding_dim, 128);
    }

    #[test]
    fn rvf_header_validation() {
        let mut header = RvfHeader::new(RvfDataType::BrainGraph, 1, 0);
        assert!(header.validate().is_ok());

        header.magic = [0, 0, 0, 0];
        assert!(header.validate().is_err());
    }

    #[test]
    fn rvf_file_write_read_roundtrip() {
        let mut file = RvfFile::new(RvfDataType::TopologyMetrics);
        file.header.num_entries = 1;
        file.metadata = serde_json::json!({ "subject": "sub-01" });
        file.data = vec![1, 2, 3, 4, 5];

        let mut buf = Vec::new();
        file.write_to(&mut buf).unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let recovered = RvfFile::read_from(&mut cursor).unwrap();

        assert_eq!(recovered.header.data_type, RvfDataType::TopologyMetrics);
        assert_eq!(recovered.header.num_entries, 1);
        assert_eq!(recovered.metadata["subject"], "sub-01");
        assert_eq!(recovered.data, vec![1, 2, 3, 4, 5]);
    }

    // ── Serialization roundtrip tests ───────────────────────────────

    #[test]
    fn graph_serialize_roundtrip() {
        let graph = BrainGraph {
            num_nodes: 2,
            edges: vec![BrainEdge {
                source: 0,
                target: 1,
                weight: 0.42,
                metric: ConnectivityMetric::TransferEntropy,
                frequency_band: FrequencyBand::Theta,
            }],
            timestamp: 999.0,
            window_duration_s: 2.0,
            atlas: Atlas::Schaefer200,
        };
        let json = serde_json::to_string(&graph).unwrap();
        let g2: BrainGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(g2.num_nodes, 2);
        assert_eq!(g2.edges.len(), 1);
        assert!((g2.edges[0].weight - 0.42).abs() < 1e-10);
    }

    #[test]
    fn topology_metrics_serialize_roundtrip() {
        let metrics = TopologyMetrics {
            global_mincut: 3.14,
            modularity: 0.55,
            global_efficiency: 0.72,
            local_efficiency: 0.68,
            graph_entropy: 2.3,
            fiedler_value: 0.12,
            num_modules: 4,
            timestamp: 500.0,
        };
        let json = serde_json::to_string(&metrics).unwrap();
        let m2: TopologyMetrics = serde_json::from_str(&json).unwrap();
        assert!((m2.global_mincut - 3.14).abs() < 1e-10);
        assert_eq!(m2.num_modules, 4);
    }
}
