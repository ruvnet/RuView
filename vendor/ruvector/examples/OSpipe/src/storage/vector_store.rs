//! Vector storage with cosine similarity search.
//!
//! This module provides two implementations:
//!
//! - [`VectorStore`] -- brute-force O(n) linear scan (cross-platform,
//!   works on WASM).
//! - [`HnswVectorStore`] (native only) -- wraps ruvector-core's HNSW
//!   index for O(log n) approximate nearest-neighbor search.
//!
//! Both implementations support insert, search, filtered search, delete,
//! and metadata update.

use crate::capture::CapturedFrame;
use crate::config::StorageConfig;
use crate::error::{OsPipeError, Result};
use crate::storage::embedding::cosine_similarity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A vector embedding stored with its metadata.
#[derive(Debug, Clone)]
pub struct StoredEmbedding {
    /// Unique identifier matching the source frame.
    pub id: Uuid,
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// JSON metadata about the source frame.
    pub metadata: serde_json::Value,
    /// When the source frame was captured.
    pub timestamp: DateTime<Utc>,
}

/// A search result returned from the vector store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// ID of the matched embedding.
    pub id: Uuid,
    /// Cosine similarity score (higher is more similar).
    pub score: f32,
    /// Metadata of the matched embedding.
    pub metadata: serde_json::Value,
}

/// Filter criteria for narrowing search results.
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    /// Filter by application name.
    pub app: Option<String>,
    /// Filter by start time (inclusive).
    pub time_start: Option<DateTime<Utc>>,
    /// Filter by end time (inclusive).
    pub time_end: Option<DateTime<Utc>>,
    /// Filter by content type (e.g., "ocr", "transcription", "ui_event").
    pub content_type: Option<String>,
    /// Filter by monitor index.
    pub monitor: Option<u32>,
}

// ===========================================================================
// VectorStore -- brute-force fallback (cross-platform)
// ===========================================================================

/// In-memory vector store with brute-force cosine similarity search.
///
/// This is the cross-platform fallback that also works on WASM targets.
/// On native targets, prefer [`HnswVectorStore`] for large datasets.
pub struct VectorStore {
    config: StorageConfig,
    embeddings: Vec<StoredEmbedding>,
    dimension: usize,
}

impl VectorStore {
    /// Create a new vector store with the given configuration.
    pub fn new(config: StorageConfig) -> Result<Self> {
        let dimension = config.embedding_dim;
        if dimension == 0 {
            return Err(OsPipeError::Storage(
                "embedding_dim must be greater than 0".to_string(),
            ));
        }
        Ok(Self {
            config,
            embeddings: Vec::new(),
            dimension,
        })
    }

    /// Insert a captured frame with its pre-computed embedding.
    pub fn insert(&mut self, frame: &CapturedFrame, embedding: &[f32]) -> Result<()> {
        if embedding.len() != self.dimension {
            return Err(OsPipeError::Storage(format!(
                "Expected embedding dimension {}, got {}",
                self.dimension,
                embedding.len()
            )));
        }

        let metadata = serde_json::json!({
            "text": frame.text_content(),
            "content_type": frame.content_type(),
            "app_name": frame.metadata.app_name,
            "window_title": frame.metadata.window_title,
            "monitor_id": frame.metadata.monitor_id,
            "confidence": frame.metadata.confidence,
        });

        self.embeddings.push(StoredEmbedding {
            id: frame.id,
            vector: embedding.to_vec(),
            metadata,
            timestamp: frame.timestamp,
        });

        Ok(())
    }

    /// Search for the k most similar embeddings to the query vector.
    pub fn search(&self, query_embedding: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        if query_embedding.len() != self.dimension {
            return Err(OsPipeError::Search(format!(
                "Expected query dimension {}, got {}",
                self.dimension,
                query_embedding.len()
            )));
        }

        let mut scored: Vec<(usize, f32)> = self
            .embeddings
            .iter()
            .enumerate()
            .map(|(i, stored)| {
                let score = cosine_similarity(query_embedding, &stored.vector);
                (i, score)
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);

        Ok(scored
            .into_iter()
            .map(|(i, score)| {
                let stored = &self.embeddings[i];
                SearchResult {
                    id: stored.id,
                    score,
                    metadata: stored.metadata.clone(),
                }
            })
            .collect())
    }

    /// Search with metadata filtering applied before scoring.
    pub fn search_filtered(
        &self,
        query: &[f32],
        k: usize,
        filter: &SearchFilter,
    ) -> Result<Vec<SearchResult>> {
        if query.len() != self.dimension {
            return Err(OsPipeError::Search(format!(
                "Expected query dimension {}, got {}",
                self.dimension,
                query.len()
            )));
        }

        let mut scored: Vec<(usize, f32)> = self
            .embeddings
            .iter()
            .enumerate()
            .filter(|(_, stored)| matches_filter(stored, filter))
            .map(|(i, stored)| {
                let score = cosine_similarity(query, &stored.vector);
                (i, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);

        Ok(scored
            .into_iter()
            .map(|(i, score)| {
                let stored = &self.embeddings[i];
                SearchResult {
                    id: stored.id,
                    score,
                    metadata: stored.metadata.clone(),
                }
            })
            .collect())
    }

    /// Delete a stored embedding by its ID.
    ///
    /// Returns `true` if the embedding was found and removed, `false`
    /// if no embedding with the given ID existed.
    pub fn delete(&mut self, id: &Uuid) -> Result<bool> {
        let before = self.embeddings.len();
        self.embeddings.retain(|e| e.id != *id);
        Ok(self.embeddings.len() < before)
    }

    /// Update the metadata of a stored embedding.
    ///
    /// The provided `metadata` value completely replaces the old metadata
    /// for the entry identified by `id`. Returns an error if the ID is
    /// not found.
    pub fn update_metadata(&mut self, id: &Uuid, metadata: serde_json::Value) -> Result<()> {
        match self.embeddings.iter_mut().find(|e| e.id == *id) {
            Some(entry) => {
                entry.metadata = metadata;
                Ok(())
            }
            None => Err(OsPipeError::Storage(format!(
                "No embedding found with id {}",
                id
            ))),
        }
    }

    /// Return the number of stored embeddings.
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Return true if the store contains no embeddings.
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Return the configured embedding dimension.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Return a reference to the storage configuration.
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// Get a stored embedding by its ID.
    pub fn get(&self, id: &Uuid) -> Option<&StoredEmbedding> {
        self.embeddings.iter().find(|e| e.id == *id)
    }
}

// ===========================================================================
// HnswVectorStore -- native-only HNSW-backed store
// ===========================================================================

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use ruvector_core::index::hnsw::HnswIndex;
    use ruvector_core::index::VectorIndex;
    use ruvector_core::types::{DistanceMetric, HnswConfig};
    use std::collections::HashMap;

    /// HNSW-backed vector store using ruvector-core.
    ///
    /// Uses approximate nearest-neighbor search for O(log n) query time.
    /// Metadata and timestamps are stored in a side-car `HashMap`
    /// alongside the HNSW index.
    pub struct HnswVectorStore {
        index: HnswIndex,
        /// Side-car storage: id -> (metadata, timestamp, vector)
        entries: HashMap<Uuid, StoredEmbedding>,
        dimension: usize,
        config: StorageConfig,
        ef_search: usize,
    }

    impl HnswVectorStore {
        /// Create a new HNSW-backed vector store.
        pub fn new(config: StorageConfig) -> Result<Self> {
            let dimension = config.embedding_dim;
            if dimension == 0 {
                return Err(OsPipeError::Storage(
                    "embedding_dim must be greater than 0".to_string(),
                ));
            }

            let hnsw_config = HnswConfig {
                m: config.hnsw_m,
                ef_construction: config.hnsw_ef_construction,
                ef_search: config.hnsw_ef_search,
                max_elements: 10_000_000,
            };

            let index = HnswIndex::new(dimension, DistanceMetric::Cosine, hnsw_config)
                .map_err(|e| OsPipeError::Storage(format!("Failed to create HNSW index: {}", e)))?;

            let ef_search = config.hnsw_ef_search;

            Ok(Self {
                index,
                entries: HashMap::new(),
                dimension,
                config,
                ef_search,
            })
        }

        /// Insert a captured frame with its pre-computed embedding.
        pub fn insert(&mut self, frame: &CapturedFrame, embedding: &[f32]) -> Result<()> {
            if embedding.len() != self.dimension {
                return Err(OsPipeError::Storage(format!(
                    "Expected embedding dimension {}, got {}",
                    self.dimension,
                    embedding.len()
                )));
            }

            let metadata = serde_json::json!({
                "text": frame.text_content(),
                "content_type": frame.content_type(),
                "app_name": frame.metadata.app_name,
                "window_title": frame.metadata.window_title,
                "monitor_id": frame.metadata.monitor_id,
                "confidence": frame.metadata.confidence,
            });

            let id_str = frame.id.to_string();

            // Insert into HNSW index
            self.index
                .add(id_str, embedding.to_vec())
                .map_err(|e| OsPipeError::Storage(format!("HNSW insert failed: {}", e)))?;

            // Store side-car data
            self.entries.insert(
                frame.id,
                StoredEmbedding {
                    id: frame.id,
                    vector: embedding.to_vec(),
                    metadata,
                    timestamp: frame.timestamp,
                },
            );

            Ok(())
        }

        /// Search for the k most similar embeddings using HNSW ANN search.
        pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
            if query.len() != self.dimension {
                return Err(OsPipeError::Search(format!(
                    "Expected query dimension {}, got {}",
                    self.dimension,
                    query.len()
                )));
            }

            let hnsw_results = self
                .index
                .search_with_ef(query, k, self.ef_search)
                .map_err(|e| OsPipeError::Search(format!("HNSW search failed: {}", e)))?;

            let mut results = Vec::with_capacity(hnsw_results.len());
            for hr in hnsw_results {
                // hr.id is a String representation of the Uuid
                if let Ok(uuid) = Uuid::parse_str(&hr.id) {
                    if let Some(stored) = self.entries.get(&uuid) {
                        // ruvector-core HNSW returns distance (lower = closer
                        // for cosine). Convert to similarity: 1.0 - distance.
                        let similarity = 1.0 - hr.score;
                        results.push(SearchResult {
                            id: uuid,
                            score: similarity,
                            metadata: stored.metadata.clone(),
                        });
                    }
                }
            }

            // Sort descending by similarity score
            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            Ok(results)
        }

        /// Search with post-filtering on metadata.
        ///
        /// HNSW does not natively support metadata filters, so we
        /// over-fetch and filter after the ANN search.
        pub fn search_filtered(
            &self,
            query: &[f32],
            k: usize,
            filter: &SearchFilter,
        ) -> Result<Vec<SearchResult>> {
            // Over-fetch to account for filtering
            let over_k = (k * 4).max(k + 20);
            let candidates = self.search(query, over_k)?;

            let mut filtered: Vec<SearchResult> = candidates
                .into_iter()
                .filter(|r| {
                    if let Some(stored) = self.entries.get(&r.id) {
                        matches_filter(stored, filter)
                    } else {
                        false
                    }
                })
                .take(k)
                .collect();

            filtered.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            Ok(filtered)
        }

        /// Delete a stored embedding by its ID.
        ///
        /// Returns `true` if the embedding was found and removed, `false`
        /// otherwise. The HNSW graph link is removed via soft-delete (the
        /// underlying `hnsw_rs` does not support hard deletion).
        pub fn delete(&mut self, id: &Uuid) -> Result<bool> {
            let id_str = id.to_string();
            let removed_from_index = self
                .index
                .remove(&id_str)
                .map_err(|e| OsPipeError::Storage(format!("HNSW delete failed: {}", e)))?;

            let removed_from_entries = self.entries.remove(id).is_some();

            Ok(removed_from_index || removed_from_entries)
        }

        /// Update the metadata of a stored embedding.
        ///
        /// Returns an error if no embedding with the given ID exists.
        pub fn update_metadata(&mut self, id: &Uuid, metadata: serde_json::Value) -> Result<()> {
            match self.entries.get_mut(id) {
                Some(entry) => {
                    entry.metadata = metadata;
                    Ok(())
                }
                None => Err(OsPipeError::Storage(format!(
                    "No embedding found with id {}",
                    id
                ))),
            }
        }

        /// Return the number of stored embeddings.
        pub fn len(&self) -> usize {
            self.entries.len()
        }

        /// Return true if the store is empty.
        pub fn is_empty(&self) -> bool {
            self.entries.is_empty()
        }

        /// Return the configured embedding dimension.
        pub fn dimension(&self) -> usize {
            self.dimension
        }

        /// Return a reference to the storage configuration.
        pub fn config(&self) -> &StorageConfig {
            &self.config
        }

        /// Get a stored embedding by its ID.
        pub fn get(&self, id: &Uuid) -> Option<&StoredEmbedding> {
            self.entries.get(id)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::HnswVectorStore;

// ===========================================================================
// Shared helpers
// ===========================================================================

/// Check whether a stored embedding matches the given filter.
fn matches_filter(stored: &StoredEmbedding, filter: &SearchFilter) -> bool {
    if let Some(ref app) = filter.app {
        let stored_app = stored
            .metadata
            .get("app_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if stored_app != app {
            return false;
        }
    }

    if let Some(start) = filter.time_start {
        if stored.timestamp < start {
            return false;
        }
    }

    if let Some(end) = filter.time_end {
        if stored.timestamp > end {
            return false;
        }
    }

    if let Some(ref ct) = filter.content_type {
        let stored_ct = stored
            .metadata
            .get("content_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if stored_ct != ct {
            return false;
        }
    }

    if let Some(monitor) = filter.monitor {
        let stored_monitor = stored
            .metadata
            .get("monitor_id")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        if stored_monitor != Some(monitor) {
            return false;
        }
    }

    true
}
