//! Standalone vector database integration for ONNX embeddings
//!
//! This module provides a lightweight vector database built on top of the
//! embedding system, demonstrating how to integrate with RuVector or use
//! as a standalone semantic search engine.

use crate::{Embedder, EmbeddingError, Result};
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, instrument};
use uuid::Uuid;

/// Vector ID type (using String for compatibility with RuVector)
pub type VectorId = String;

/// Distance metric for similarity calculation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Distance {
    /// Cosine similarity (default, best for normalized embeddings)
    #[default]
    Cosine,
    /// Euclidean (L2) distance
    Euclidean,
    /// Dot product
    DotProduct,
}

/// Search result with text and score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Vector ID
    pub id: VectorId,
    /// Original text
    pub text: String,
    /// Similarity score (higher is better for cosine, lower for euclidean)
    pub score: f32,
    /// Optional metadata
    pub metadata: Option<serde_json::Value>,
}

/// Stored vector entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredEntry {
    id: VectorId,
    text: String,
    vector: Vec<f32>,
    metadata: Option<serde_json::Value>,
}

/// Configuration for creating a vector index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    /// Distance metric
    pub distance: Distance,
    /// Maximum number of elements (for pre-allocation)
    pub max_elements: usize,
    /// Number of results to over-fetch for filtering
    pub ef_search: usize,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            distance: Distance::Cosine,
            max_elements: 100_000,
            ef_search: 100,
        }
    }
}

/// RuVector-compatible embeddings index
///
/// A lightweight in-memory vector database that integrates ONNX embeddings
/// with similarity search. Compatible with RuVector's API patterns.
pub struct RuVectorEmbeddings {
    /// The embedder for generating vectors (wrapped in RwLock for mutable access)
    embedder: Arc<RwLock<Embedder>>,
    /// Stored vectors and metadata
    entries: RwLock<Vec<StoredEntry>>,
    /// Index name
    name: String,
    /// Configuration
    config: IndexConfig,
}

impl RuVectorEmbeddings {
    /// Create a new RuVector index with the given embedder
    #[instrument(skip_all)]
    pub fn new(
        name: impl Into<String>,
        embedder: Embedder,
        config: IndexConfig,
    ) -> Result<Self> {
        let name = name.into();
        let dimension = embedder.dimension();

        info!(
            "Creating RuVector index '{}' with dimension {} and {:?} distance",
            name, dimension, config.distance
        );

        Ok(Self {
            embedder: Arc::new(RwLock::new(embedder)),
            entries: RwLock::new(Vec::with_capacity(config.max_elements.min(10_000))),
            name,
            config,
        })
    }

    /// Create with default configuration
    pub fn new_default(name: impl Into<String>, embedder: Embedder) -> Result<Self> {
        Self::new(name, embedder, IndexConfig::default())
    }

    /// Insert a single text with optional metadata
    #[instrument(skip(self, text, metadata), fields(text_len = text.len()))]
    pub fn insert(
        &self,
        text: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<VectorId> {
        let embedding = self.embedder.write().embed_one(text)?;
        self.insert_with_embedding(text, embedding, metadata)
    }

    /// Insert with pre-computed embedding
    pub fn insert_with_embedding(
        &self,
        text: &str,
        embedding: Vec<f32>,
        metadata: Option<serde_json::Value>,
    ) -> Result<VectorId> {
        let id = Uuid::new_v4().to_string();

        let entry = StoredEntry {
            id: id.clone(),
            text: text.to_string(),
            vector: embedding,
            metadata,
        };

        self.entries.write().push(entry);

        debug!("Inserted text with ID {}", id);
        Ok(id)
    }

    /// Insert multiple texts
    #[instrument(skip(self, texts), fields(count = texts.len()))]
    pub fn insert_batch<S: AsRef<str>>(&self, texts: &[S]) -> Result<Vec<VectorId>> {
        let embeddings = self.embedder.write().embed(texts)?;
        self.insert_batch_with_embeddings(texts, embeddings.embeddings)
    }

    /// Insert batch with pre-computed embeddings
    pub fn insert_batch_with_embeddings<S: AsRef<str>>(
        &self,
        texts: &[S],
        embeddings: Vec<Vec<f32>>,
    ) -> Result<Vec<VectorId>> {
        if texts.len() != embeddings.len() {
            return Err(EmbeddingError::dimension_mismatch(
                texts.len(),
                embeddings.len(),
            ));
        }

        let entries: Vec<StoredEntry> = texts
            .iter()
            .zip(embeddings)
            .map(|(text, vector)| StoredEntry {
                id: Uuid::new_v4().to_string(),
                text: text.as_ref().to_string(),
                vector,
                metadata: None,
            })
            .collect();

        let ids: Vec<VectorId> = entries.iter().map(|e| e.id.clone()).collect();

        self.entries.write().extend(entries);

        info!("Inserted {} vectors", ids.len());
        Ok(ids)
    }

    /// Search for similar texts
    #[instrument(skip(self, query), fields(k))]
    pub fn search(&self, query: &str, k: usize) -> Result<Vec<SearchResult>> {
        let query_embedding = self.embedder.write().embed_one(query)?;
        self.search_with_embedding(&query_embedding, k)
    }

    /// Search with pre-computed query embedding
    pub fn search_with_embedding(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> Result<Vec<SearchResult>> {
        let entries = self.entries.read();

        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate similarities in parallel
        let mut scored: Vec<(usize, f32)> = entries
            .par_iter()
            .enumerate()
            .map(|(i, entry)| {
                let score = self.compute_similarity(query_embedding, &entry.vector);
                (i, score)
            })
            .collect();

        // Sort by score (descending for cosine/dot, ascending for euclidean)
        match self.config.distance {
            Distance::Cosine | Distance::DotProduct => {
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
            Distance::Euclidean => {
                scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

        // Take top k
        let results: Vec<SearchResult> = scored
            .into_iter()
            .take(k)
            .map(|(i, score)| {
                let entry = &entries[i];
                SearchResult {
                    id: entry.id.clone(),
                    text: entry.text.clone(),
                    score,
                    metadata: entry.metadata.clone(),
                }
            })
            .collect();

        debug!("Search returned {} results", results.len());
        Ok(results)
    }

    /// Compute similarity/distance between two vectors
    fn compute_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        match self.config.distance {
            Distance::Cosine => Self::cosine_similarity(a, b),
            Distance::Euclidean => Self::euclidean_distance(a, b),
            Distance::DotProduct => Self::dot_product(a, b),
        }
    }

    /// Cosine similarity between two vectors
    #[inline]
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a > 1e-10 && norm_b > 1e-10 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        }
    }

    /// Euclidean (L2) distance
    #[inline]
    fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Dot product
    #[inline]
    fn dot_product(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Search with metadata filter
    #[instrument(skip(self, query, filter), fields(k))]
    pub fn search_filtered<F>(&self, query: &str, k: usize, filter: F) -> Result<Vec<SearchResult>>
    where
        F: Fn(&serde_json::Value) -> bool + Sync,
    {
        let query_embedding = self.embedder.write().embed_one(query)?;
        let entries = self.entries.read();

        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate similarities with filtering
        let mut scored: Vec<(usize, f32)> = entries
            .par_iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                // Apply filter
                if let Some(ref meta) = entry.metadata {
                    if !filter(meta) {
                        return None;
                    }
                }
                let score = self.compute_similarity(&query_embedding, &entry.vector);
                Some((i, score))
            })
            .collect();

        // Sort
        match self.config.distance {
            Distance::Cosine | Distance::DotProduct => {
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
            Distance::Euclidean => {
                scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

        let results: Vec<SearchResult> = scored
            .into_iter()
            .take(k)
            .map(|(i, score)| {
                let entry = &entries[i];
                SearchResult {
                    id: entry.id.clone(),
                    text: entry.text.clone(),
                    score,
                    metadata: entry.metadata.clone(),
                }
            })
            .collect();

        Ok(results)
    }

    /// Get a vector by ID
    pub fn get(&self, id: &str) -> Option<(String, Vec<f32>)> {
        let entries = self.entries.read();
        entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| (e.text.clone(), e.vector.clone()))
    }

    /// Delete a vector by ID
    pub fn delete(&self, id: &str) -> bool {
        let mut entries = self.entries.write();
        let len_before = entries.len();
        entries.retain(|e| e.id != id);
        entries.len() < len_before
    }

    /// Get the number of vectors in the index
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Get index name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> usize {
        self.embedder.read().dimension()
    }

    /// Get reference to the embedder (wrapped in Arc<RwLock>)
    pub fn embedder(&self) -> &Arc<RwLock<Embedder>> {
        &self.embedder
    }

    /// Clear all vectors
    pub fn clear(&self) {
        self.entries.write().clear();
    }

    /// Export all entries for persistence
    pub fn export(&self) -> Vec<(VectorId, String, Vec<f32>, Option<serde_json::Value>)> {
        self.entries
            .read()
            .iter()
            .map(|e| (e.id.clone(), e.text.clone(), e.vector.clone(), e.metadata.clone()))
            .collect()
    }

    /// Import entries (for loading from persistence)
    pub fn import(
        &self,
        entries: Vec<(VectorId, String, Vec<f32>, Option<serde_json::Value>)>,
    ) {
        let stored: Vec<StoredEntry> = entries
            .into_iter()
            .map(|(id, text, vector, metadata)| StoredEntry {
                id,
                text,
                vector,
                metadata,
            })
            .collect();

        *self.entries.write() = stored;
    }
}

/// Builder for creating RuVector indexes
pub struct RuVectorBuilder {
    name: String,
    embedder: Option<Embedder>,
    config: IndexConfig,
}

impl RuVectorBuilder {
    /// Create a new builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            embedder: None,
            config: IndexConfig::default(),
        }
    }

    /// Set the embedder
    pub fn embedder(mut self, embedder: Embedder) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Set distance metric
    pub fn distance(mut self, distance: Distance) -> Self {
        self.config.distance = distance;
        self
    }

    /// Set max elements
    pub fn max_elements(mut self, max: usize) -> Self {
        self.config.max_elements = max;
        self
    }

    /// Set ef_search parameter
    pub fn ef_search(mut self, ef: usize) -> Self {
        self.config.ef_search = ef;
        self
    }

    /// Build the index
    pub fn build(self) -> Result<RuVectorEmbeddings> {
        let embedder = self
            .embedder
            .ok_or_else(|| EmbeddingError::invalid_config("Embedder is required"))?;

        RuVectorEmbeddings::new(self.name, embedder, self.config)
    }
}

/// RAG (Retrieval-Augmented Generation) helper
pub struct RagPipeline {
    index: RuVectorEmbeddings,
    top_k: usize,
}

impl RagPipeline {
    /// Create a new RAG pipeline
    pub fn new(index: RuVectorEmbeddings, top_k: usize) -> Self {
        Self { index, top_k }
    }

    /// Retrieve context for a query
    pub fn retrieve(&self, query: &str) -> Result<Vec<String>> {
        let results = self.index.search(query, self.top_k)?;
        Ok(results.into_iter().map(|r| r.text).collect())
    }

    /// Retrieve with scores
    pub fn retrieve_with_scores(&self, query: &str) -> Result<Vec<(String, f32)>> {
        let results = self.index.search(query, self.top_k)?;
        Ok(results.into_iter().map(|r| (r.text, r.score)).collect())
    }

    /// Format retrieved context as a prompt
    pub fn format_context(&self, query: &str) -> Result<String> {
        let contexts = self.retrieve(query)?;

        let mut prompt = String::from("Context:\n");
        for (i, ctx) in contexts.iter().enumerate() {
            prompt.push_str(&format!("[{}] {}\n", i + 1, ctx));
        }
        prompt.push_str(&format!("\nQuestion: {}", query));

        Ok(prompt)
    }

    /// Format context with scores
    pub fn format_context_with_scores(&self, query: &str) -> Result<String> {
        let results = self.retrieve_with_scores(query)?;

        let mut prompt = String::from("Context (with relevance scores):\n");
        for (i, (ctx, score)) in results.iter().enumerate() {
            prompt.push_str(&format!("[{} - {:.3}] {}\n", i + 1, score, ctx));
        }
        prompt.push_str(&format!("\nQuestion: {}", query));

        Ok(prompt)
    }

    /// Add documents to the index
    pub fn add_documents<S: AsRef<str>>(&self, documents: &[S]) -> Result<Vec<VectorId>> {
        self.index.insert_batch(documents)
    }

    /// Get reference to the underlying index
    pub fn index(&self) -> &RuVectorEmbeddings {
        &self.index
    }

    /// Get mutable reference to set top_k
    pub fn set_top_k(&mut self, k: usize) {
        self.top_k = k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];

        assert!((RuVectorEmbeddings::cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
        assert!(RuVectorEmbeddings::cosine_similarity(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];

        let dist = RuVectorEmbeddings::euclidean_distance(&a, &b);
        assert!((dist - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];

        let dot = RuVectorEmbeddings::dot_product(&a, &b);
        assert!((dot - 32.0).abs() < 1e-6);
    }
}
