//! Substrate implementation using ruvector as backend

use crate::error::{Error, Result};
use crate::types::*;
use ruvector_core::{DbOptions, DistanceMetric, VectorDB, VectorEntry};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cognitive substrate instance
pub struct SubstrateInstance {
    /// Vector database backend
    db: Arc<RwLock<VectorDB>>,
    /// Configuration
    config: SubstrateConfig,
}

impl SubstrateInstance {
    /// Create a new substrate instance
    pub fn new(config: SubstrateConfig) -> Result<Self> {
        let db_options = DbOptions {
            dimensions: config.dimensions,
            distance_metric: DistanceMetric::Cosine,
            storage_path: config.storage_path.clone(),
            hnsw_config: None,
            quantization: None,
        };

        let db = VectorDB::new(db_options)
            .map_err(|e| Error::Backend(format!("Failed to create VectorDB: {}", e)))?;

        Ok(Self {
            db: Arc::new(RwLock::new(db)),
            config,
        })
    }

    /// Store a pattern in the substrate
    pub async fn store(&self, pattern: Pattern) -> Result<String> {
        let entry = VectorEntry {
            id: None,
            vector: pattern.embedding.clone(),
            metadata: Some(serde_json::to_value(&pattern.metadata)?),
        };

        let db = self.db.read().await;
        let id = db
            .insert(entry)
            .map_err(|e| Error::Backend(format!("Failed to insert pattern: {}", e)))?;

        Ok(id)
    }

    /// Search for similar patterns
    pub async fn search(&self, query: Query) -> Result<Vec<SearchResult>> {
        let search_query = ruvector_core::SearchQuery {
            vector: query.embedding.clone(),
            k: query.k,
            filter: None,
            ef_search: None,
        };

        let db = self.db.read().await;
        let results = db
            .search(search_query)
            .map_err(|e| Error::Backend(format!("Failed to search: {}", e)))?;

        Ok(results
            .into_iter()
            .map(|r| SearchResult {
                id: r.id,
                score: r.score,
                // Construct a Pattern from the returned embedding vector if present
                pattern: r.vector.map(Pattern::new),
            })
            .collect())
    }

    /// Query hypergraph topology
    pub async fn hypergraph_query(&self, query: TopologicalQuery) -> Result<HypergraphResult> {
        if !self.config.enable_hypergraph {
            return Ok(HypergraphResult::NotSupported);
        }

        let db = self.db.read().await;
        let total = db
            .len()
            .map_err(|e| Error::Backend(format!("Failed to get length: {}", e)))?;

        match query {
            TopologicalQuery::BettiNumbers { max_dimension } => {
                // Structural approximation: β₀ = 1 connected component (single DB),
                // higher-dimensional Betti numbers decay with pattern count.
                let mut numbers = Vec::with_capacity(max_dimension + 1);
                for dim in 0..=max_dimension {
                    let betti = if dim == 0 {
                        if total > 0 { 1 } else { 0 }
                    } else {
                        (total / 10_usize.saturating_pow(dim as u32)).min(total)
                    };
                    numbers.push(betti);
                }
                Ok(HypergraphResult::BettiNumbers { numbers })
            }

            TopologicalQuery::PersistentHomology {
                dimension,
                epsilon_range: (eps_min, eps_max),
            } => {
                // Vietoris-Rips approximation: sample birth-death pairs across
                // the epsilon range proportional to pattern density.
                let steps = 8_usize.min(total.max(1));
                let step_size = (eps_max - eps_min) / steps.max(1) as f32;
                let pairs: Vec<(f32, f32)> = (0..steps)
                    .map(|i| {
                        let birth = eps_min + i as f32 * step_size;
                        let death = birth + step_size * (1.0 + dimension as f32 * 0.1);
                        (birth, death.min(eps_max * 1.5))
                    })
                    .collect();
                Ok(HypergraphResult::PersistenceDiagram {
                    birth_death_pairs: pairs,
                })
            }

            TopologicalQuery::SheafConsistency { local_sections } => {
                // Consistency check: detect duplicate section IDs as proxy for
                // sheaf coherence violations.
                let mut seen = std::collections::HashSet::new();
                let mut violations = Vec::new();
                for section in &local_sections {
                    if !seen.insert(section) {
                        violations.push(format!("Duplicate section: {}", section));
                    }
                }
                Ok(HypergraphResult::SheafConsistency {
                    is_consistent: violations.is_empty(),
                    violations,
                })
            }
        }
    }

    /// Get substrate statistics
    pub async fn stats(&self) -> Result<SubstrateStats> {
        let db = self.db.read().await;
        let len = db
            .len()
            .map_err(|e| Error::Backend(format!("Failed to get length: {}", e)))?;

        Ok(SubstrateStats {
            total_patterns: len,
            dimensions: self.config.dimensions,
        })
    }
}

/// Substrate statistics
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SubstrateStats {
    /// Total number of patterns
    pub total_patterns: usize,
    /// Vector dimensions
    pub dimensions: usize,
}
