//! Integration tests for full tick cycle
//!
//! Tests cover:
//! - Complete WorkerTileState lifecycle
//! - Delta processing sequences
//! - Tick report generation
//! - Multiple tile coordination scenarios

use cognitum_gate_kernel::{
    Delta, DeltaError, WorkerTileState,
    shard::{Edge, EdgeId, VertexId, Weight},
    report::{TileReport, TileStatus},
};

#[cfg(test)]
mod worker_tile_lifecycle {
    use super::*;

    #[test]
    fn test_tile_creation() {
        let tile = WorkerTileState::new(42);
        assert_eq!(tile.tile_id, 42);
        assert_eq!(tile.coherence, 0);
        assert_eq!(tile.tick, 0);
    }

    #[test]
    fn test_initial_report() {
        let mut tile = WorkerTileState::new(5);
        let report = tile.tick(1000);
        assert_eq!(report.tile_id, 5);
        assert_eq!(report.status, TileStatus::Active);
    }
}

#[cfg(test)]
mod delta_processing {
    use super::*;

    #[test]
    fn test_edge_add_delta() {
        let mut tile = WorkerTileState::new(0);
        let edge = Edge::new(VertexId(0), VertexId(1));
        let delta = Delta::EdgeAdd { edge, weight: Weight(100) };

        assert!(tile.ingest_delta(&delta).is_ok());
        assert_eq!(tile.graph_shard.edge_count(), 1);
    }

    #[test]
    fn test_edge_remove_delta() {
        let mut tile = WorkerTileState::new(0);
        let edge = Edge::new(VertexId(0), VertexId(1));

        tile.ingest_delta(&Delta::EdgeAdd { edge, weight: Weight(100) }).unwrap();
        tile.ingest_delta(&Delta::EdgeRemove { edge: EdgeId(0) }).unwrap();

        assert_eq!(tile.graph_shard.edge_count(), 0);
    }

    #[test]
    fn test_weight_update_delta() {
        let mut tile = WorkerTileState::new(0);
        let edge = Edge::new(VertexId(0), VertexId(1));

        tile.ingest_delta(&Delta::EdgeAdd { edge, weight: Weight(100) }).unwrap();
        tile.ingest_delta(&Delta::WeightUpdate { edge: EdgeId(0), weight: Weight(200) }).unwrap();

        assert_eq!(tile.graph_shard.get_weight(EdgeId(0)), Some(Weight(200)));
    }

    #[test]
    fn test_observation_delta() {
        let mut tile = WorkerTileState::new(0);
        tile.ingest_delta(&Delta::Observation { score: 0.8 }).unwrap();
        assert_eq!(tile.e_accumulator.observation_count(), 1);
    }

    #[test]
    fn test_self_loop_rejected() {
        let mut tile = WorkerTileState::new(0);
        let edge = Edge::new(VertexId(5), VertexId(5));
        let delta = Delta::EdgeAdd { edge, weight: Weight(100) };
        assert_eq!(tile.ingest_delta(&delta), Err(DeltaError::InvalidEdge));
    }
}

#[cfg(test)]
mod tick_cycle {
    use super::*;

    #[test]
    fn test_single_tick() {
        let mut tile = WorkerTileState::new(10);
        let report = tile.tick(1000);
        assert_eq!(report.tile_id, 10);
        assert_eq!(tile.tick, 1000);
    }

    #[test]
    fn test_tick_updates_timestamp() {
        let mut tile = WorkerTileState::new(0);
        tile.tick(1000);
        assert_eq!(tile.tick, 1000);
        tile.tick(2000);
        assert_eq!(tile.tick, 2000);
    }

    #[test]
    fn test_tick_after_deltas() {
        let mut tile = WorkerTileState::new(0);

        tile.ingest_delta(&Delta::EdgeAdd {
            edge: Edge::new(VertexId(0), VertexId(1)),
            weight: Weight(100),
        }).unwrap();
        tile.ingest_delta(&Delta::Observation { score: 0.9 }).unwrap();

        let report = tile.tick(1000);
        assert!(report.is_healthy());
    }

    #[test]
    fn test_multiple_tick_cycles() {
        let mut tile = WorkerTileState::new(0);

        for i in 0..10 {
            tile.ingest_delta(&Delta::EdgeAdd {
                edge: Edge::new(VertexId(i as u8), VertexId((i + 1) as u8)),
                weight: Weight(100),
            }).unwrap();
            tile.ingest_delta(&Delta::Observation { score: 0.8 }).unwrap();
            let report = tile.tick((i + 1) * 1000);
            assert!(report.is_healthy());
        }

        assert_eq!(tile.graph_shard.edge_count(), 10);
    }
}

#[cfg(test)]
mod e_value_accumulation {
    use super::*;

    #[test]
    fn test_e_value_in_report() {
        let mut tile = WorkerTileState::new(0);

        for _ in 0..5 {
            tile.ingest_delta(&Delta::Observation { score: 0.9 }).unwrap();
        }

        let report = tile.tick(1000);
        assert!(report.e_value > 0.0);
    }
}

#[cfg(test)]
mod multi_tile_scenario {
    use super::*;

    #[test]
    fn test_deterministic_across_tiles() {
        let deltas = [
            Delta::EdgeAdd { edge: Edge::new(VertexId(0), VertexId(1)), weight: Weight(100) },
            Delta::EdgeAdd { edge: Edge::new(VertexId(1), VertexId(2)), weight: Weight(150) },
            Delta::Observation { score: 0.9 },
        ];

        let mut tile1 = WorkerTileState::new(0);
        let mut tile2 = WorkerTileState::new(0);

        for delta in &deltas {
            tile1.ingest_delta(delta).unwrap();
            tile2.ingest_delta(delta).unwrap();
        }

        let report1 = tile1.tick(1000);
        let report2 = tile2.tick(1000);

        assert_eq!(report1.coherence, report2.coherence);
        assert!((report1.e_value - report2.e_value).abs() < 0.001);
    }

    #[test]
    fn test_tile_network() {
        let mut tiles: Vec<WorkerTileState> = (0..10)
            .map(|id| WorkerTileState::new(id))
            .collect();

        for (tile_idx, tile) in tiles.iter_mut().enumerate() {
            let base = (tile_idx * 10) as u8;
            for i in 0..5u8 {
                let _ = tile.ingest_delta(&Delta::EdgeAdd {
                    edge: Edge::new(VertexId(base + i), VertexId(base + i + 1)),
                    weight: Weight(100),
                });
            }
        }

        let reports: Vec<TileReport> = tiles
            .iter_mut()
            .enumerate()
            .map(|(idx, tile)| tile.tick((idx as u64) * 100))
            .collect();

        for report in &reports {
            assert!(report.is_healthy());
        }
    }
}

#[cfg(test)]
mod edge_cases {
    use super::*;

    #[test]
    fn test_empty_tile_tick() {
        let mut tile = WorkerTileState::new(0);
        let report = tile.tick(1000);
        assert!(report.is_healthy());
    }

    #[test]
    fn test_tile_with_only_observations() {
        let mut tile = WorkerTileState::new(0);

        for _ in 0..100 {
            tile.ingest_delta(&Delta::Observation { score: 0.5 }).unwrap();
        }

        let report = tile.tick(1000);
        assert!(report.is_healthy());
        assert_eq!(tile.graph_shard.edge_count(), 0);
    }

    #[test]
    fn test_tick_at_max() {
        let mut tile = WorkerTileState::new(0);
        let report = tile.tick(u64::MAX);
        assert_eq!(tile.tick, u64::MAX);
        assert!(report.is_healthy());
    }

    #[test]
    fn test_alternating_add_remove() {
        let mut tile = WorkerTileState::new(0);

        for _ in 0..100 {
            tile.ingest_delta(&Delta::EdgeAdd {
                edge: Edge::new(VertexId(0), VertexId(1)),
                weight: Weight(100),
            }).unwrap();
            tile.ingest_delta(&Delta::EdgeRemove { edge: EdgeId(0) }).unwrap();
        }

        assert!(tile.tick(1000).is_healthy());
        assert_eq!(tile.graph_shard.edge_count(), 0);
    }
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    #[test]
    fn test_high_volume_deltas() {
        let mut tile = WorkerTileState::new(0);

        for i in 0..1000 {
            let src = (i % 200) as u8;
            let dst = ((i + 1) % 200) as u8;

            if src != dst {
                let _ = tile.ingest_delta(&Delta::EdgeAdd {
                    edge: Edge::new(VertexId(src), VertexId(dst)),
                    weight: Weight(100),
                });
            }

            if i % 10 == 0 {
                let _ = tile.ingest_delta(&Delta::Observation { score: 0.8 });
            }
        }

        assert!(tile.tick(10000).is_healthy());
    }

    #[test]
    fn test_rapid_tick_cycles() {
        let mut tile = WorkerTileState::new(0);

        tile.ingest_delta(&Delta::EdgeAdd {
            edge: Edge::new(VertexId(0), VertexId(1)),
            weight: Weight(100),
        }).unwrap();

        for i in 0..1000u64 {
            assert!(tile.tick(i).is_healthy());
        }
    }
}
