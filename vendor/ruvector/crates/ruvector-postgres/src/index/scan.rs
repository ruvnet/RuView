//! Index scan operators for PostgreSQL
//!
//! Implements the access method interface for HNSW and IVFFlat indexes.

use pgrx::prelude::*;

use super::hnsw::HnswConfig;
use super::ivfflat::IvfFlatConfig;
use crate::distance::DistanceMetric;

/// Parse distance metric from operator name
pub fn parse_distance_metric(op_name: &str) -> DistanceMetric {
    match op_name {
        "ruvector_l2_ops" | "<->" => DistanceMetric::Euclidean,
        "ruvector_ip_ops" | "<#>" => DistanceMetric::InnerProduct,
        "ruvector_cosine_ops" | "<=>" => DistanceMetric::Cosine,
        "ruvector_l1_ops" | "<+>" => DistanceMetric::Manhattan,
        _ => DistanceMetric::Euclidean, // Default
    }
}

/// Parse HNSW config from reloptions
pub fn parse_hnsw_config(reloptions: Option<&str>) -> HnswConfig {
    let mut config = HnswConfig::default();

    if let Some(opts) = reloptions {
        for opt in opts.split(',') {
            let parts: Vec<&str> = opt.split('=').collect();
            if parts.len() == 2 {
                let key = parts[0].trim().to_lowercase();
                let value = parts[1].trim();

                match key.as_str() {
                    "m" => {
                        if let Ok(v) = value.parse() {
                            config.m = v;
                            config.m0 = v * 2;
                        }
                    }
                    "ef_construction" => {
                        if let Ok(v) = value.parse() {
                            config.ef_construction = v;
                        }
                    }
                    "ef_search" => {
                        if let Ok(v) = value.parse() {
                            config.ef_search = v;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    config
}

/// Parse IVFFlat config from reloptions
pub fn parse_ivfflat_config(reloptions: Option<&str>) -> IvfFlatConfig {
    let mut config = IvfFlatConfig::default();

    if let Some(opts) = reloptions {
        for opt in opts.split(',') {
            let parts: Vec<&str> = opt.split('=').collect();
            if parts.len() == 2 {
                let key = parts[0].trim().to_lowercase();
                let value = parts[1].trim();

                match key.as_str() {
                    "lists" => {
                        if let Ok(v) = value.parse() {
                            config.lists = v;
                        }
                    }
                    "probes" => {
                        if let Ok(v) = value.parse() {
                            config.probes = v;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    config
}

/// Index scan state
pub struct IndexScanState {
    pub results: Vec<(u64, f32)>,
    pub current_pos: usize,
    pub metric: DistanceMetric,
}

impl IndexScanState {
    pub fn new(results: Vec<(u64, f32)>, metric: DistanceMetric) -> Self {
        Self {
            results,
            current_pos: 0,
            metric,
        }
    }

    pub fn next(&mut self) -> Option<(u64, f32)> {
        if self.current_pos < self.results.len() {
            let result = self.results[self.current_pos];
            self.current_pos += 1;
            Some(result)
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        self.current_pos = 0;
    }
}

// ============================================================================
// SQL Interface for Index Options
// ============================================================================

/// Get HNSW index info as JSON
#[pg_extern]
fn ruhnsw_index_info(index_name: &str) -> pgrx::JsonB {
    // Would query pg_class and parse reloptions
    let info = serde_json::json!({
        "name": index_name,
        "type": "ruhnsw",
        "parameters": {
            "m": 16,
            "ef_construction": 64,
            "ef_search": 40
        }
    });
    pgrx::JsonB(info)
}

/// Get IVFFlat index info as JSON
#[pg_extern]
fn ruivfflat_index_info(index_name: &str) -> pgrx::JsonB {
    // Would query pg_class and parse reloptions
    let info = serde_json::json!({
        "name": index_name,
        "type": "ruivfflat",
        "parameters": {
            "lists": 100,
            "probes": 1
        }
    });
    pgrx::JsonB(info)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hnsw_config() {
        let config = parse_hnsw_config(Some("m=32, ef_construction=200"));
        assert_eq!(config.m, 32);
        assert_eq!(config.m0, 64);
        assert_eq!(config.ef_construction, 200);
    }

    #[test]
    fn test_parse_ivfflat_config() {
        let config = parse_ivfflat_config(Some("lists=500, probes=10"));
        assert_eq!(config.lists, 500);
        assert_eq!(config.probes, 10);
    }

    #[test]
    fn test_parse_distance_metric() {
        assert_eq!(parse_distance_metric("<->"), DistanceMetric::Euclidean);
        assert_eq!(parse_distance_metric("<#>"), DistanceMetric::InnerProduct);
        assert_eq!(parse_distance_metric("<=>"), DistanceMetric::Cosine);
        assert_eq!(parse_distance_metric("<+>"), DistanceMetric::Manhattan);
    }

    #[test]
    fn test_scan_state() {
        let results = vec![(1, 0.1), (2, 0.2), (3, 0.3)];
        let mut state = IndexScanState::new(results, DistanceMetric::Euclidean);

        assert_eq!(state.next(), Some((1, 0.1)));
        assert_eq!(state.next(), Some((2, 0.2)));
        assert_eq!(state.next(), Some((3, 0.3)));
        assert_eq!(state.next(), None);

        state.reset();
        assert_eq!(state.next(), Some((1, 0.1)));
    }
}
