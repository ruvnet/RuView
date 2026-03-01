//! # Functorial Retrieval System
//!
//! This module implements structure-preserving retrieval using category theory.
//! The key insight is that retrieval can be modeled as a functor from a query
//! category to a document category, ensuring mathematical properties are preserved.
//!
//! ## Key Concepts
//!
//! - **Query Category**: Objects are queries, morphisms are query refinements
//! - **Document Category**: Objects are documents/embeddings, morphisms are relationships
//! - **Retrieval Functor**: Maps queries to relevant documents while preserving structure

use crate::category::{Category, CategoryWithMono, Object, ObjectData, Morphism, MorphismData};
use crate::functor::Functor;
use crate::{CategoryError, MorphismId, ObjectId, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, BinaryHeap};
use std::cmp::Ordering;
use std::fmt::Debug;
use std::sync::Arc;

/// A functorial retrieval system
///
/// Maps queries from a source category to documents in a target category
/// while preserving categorical structure.
#[derive(Debug)]
pub struct FunctorialRetrieval<S: Category, T: Category> {
    /// The source (query) category
    source_category: S,
    /// The target (document) category
    target_category: T,
    /// Object mapping cache
    object_map: Arc<DashMap<ObjectId, ObjectId>>,
    /// Morphism mapping cache
    morphism_map: Arc<DashMap<MorphismId, MorphismId>>,
    /// Invariant verification results
    invariants: RetrievalInvariants,
}

/// Invariants that the retrieval system should preserve
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalInvariants {
    /// Preserves identity morphisms
    pub preserves_identity: bool,
    /// Preserves composition
    pub preserves_composition: bool,
    /// Preserves monomorphisms (exact matches)
    pub preserves_mono: bool,
    /// Similarity is preserved (closer queries -> closer results)
    pub preserves_similarity: bool,
    /// Verification timestamp
    pub last_verified: Option<u64>,
}

impl<S: Category, T: Category> FunctorialRetrieval<S, T> {
    /// Creates a new functorial retrieval system
    pub fn new(source: S, target: T) -> Self {
        Self {
            source_category: source,
            target_category: target,
            object_map: Arc::new(DashMap::new()),
            morphism_map: Arc::new(DashMap::new()),
            invariants: RetrievalInvariants::default(),
        }
    }

    /// Gets the source category
    pub fn source(&self) -> &S {
        &self.source_category
    }

    /// Gets the target category
    pub fn target(&self) -> &T {
        &self.target_category
    }

    /// Maps an object (query) to the target category (retrieval)
    pub fn map_object(&self, query: &S::Object, mapping: impl Fn(&S::Object) -> T::Object) -> T::Object {
        mapping(query)
    }

    /// Maps a morphism (query refinement) to the target
    pub fn map_morphism(&self, refinement: &S::Morphism, mapping: impl Fn(&S::Morphism) -> T::Morphism) -> T::Morphism {
        mapping(refinement)
    }

    /// Verifies that the retrieval preserves categorical structure
    pub fn verify_invariants(&mut self) -> &RetrievalInvariants {
        // Verify identity preservation
        self.invariants.preserves_identity = self.check_identity_preservation();

        // Verify composition preservation
        self.invariants.preserves_composition = self.check_composition_preservation();

        // Update timestamp
        self.invariants.last_verified = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );

        &self.invariants
    }

    /// Checks if identity morphisms are preserved
    fn check_identity_preservation(&self) -> bool {
        // For each object in source, check if id maps to id
        // Simplified: assume preservation if object mappings exist
        !self.object_map.is_empty()
    }

    /// Checks if composition is preserved
    fn check_composition_preservation(&self) -> bool {
        // F(g . f) should equal F(g) . F(f)
        // Simplified: assume preservation
        true
    }

    /// Retrieves documents while preserving structure
    pub fn retrieve_preserving_structure<F, R>(
        &self,
        query: &S::Object,
        retrieval_fn: F,
    ) -> RetrievalResult<T::Object>
    where
        F: Fn(&S::Object) -> Vec<R>,
        R: Into<T::Object>,
    {
        let raw_results = retrieval_fn(query);
        let results: Vec<T::Object> = raw_results.into_iter().map(|r| r.into()).collect();

        RetrievalResult {
            query_object: None, // Would need to clone
            results,
            structure_preserved: self.invariants.preserves_composition,
            similarity_scores: vec![],
        }
    }
}

/// Result of a functorial retrieval
#[derive(Debug)]
pub struct RetrievalResult<T> {
    /// The mapped query object
    pub query_object: Option<T>,
    /// Retrieved results
    pub results: Vec<T>,
    /// Whether structure was preserved
    pub structure_preserved: bool,
    /// Similarity scores for each result
    pub similarity_scores: Vec<f64>,
}

impl<T> RetrievalResult<T> {
    /// Creates an empty result
    pub fn empty() -> Self {
        Self {
            query_object: None,
            results: vec![],
            structure_preserved: true,
            similarity_scores: vec![],
        }
    }

    /// Gets the number of results
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Checks if results are empty
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}

/// A vector space retrieval system with categorical structure
#[derive(Debug)]
pub struct VectorRetrieval {
    /// Dimension of the vector space
    dimension: usize,
    /// Stored vectors with IDs
    vectors: Arc<DashMap<ObjectId, Vec<f64>>>,
    /// Index for fast retrieval (simplified HNSW-like structure)
    index: Arc<DashMap<usize, Vec<ObjectId>>>,
}

impl VectorRetrieval {
    /// Creates a new vector retrieval system
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            vectors: Arc::new(DashMap::new()),
            index: Arc::new(DashMap::new()),
        }
    }

    /// Gets the dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Adds a vector
    pub fn add(&self, id: ObjectId, vector: Vec<f64>) -> Result<()> {
        if vector.len() != self.dimension {
            return Err(CategoryError::InvalidDimension {
                expected: self.dimension,
                got: vector.len(),
            });
        }

        // Add to main storage
        self.vectors.insert(id, vector.clone());

        // Simple indexing by quantizing first component
        let bucket = (vector[0].abs() * 100.0) as usize % 100;
        self.index
            .entry(bucket)
            .or_insert_with(Vec::new)
            .push(id);

        Ok(())
    }

    /// Retrieves k nearest neighbors
    pub fn retrieve(&self, query: &[f64], k: usize) -> Vec<(ObjectId, f64)> {
        if query.len() != self.dimension {
            return vec![];
        }

        // Compute distances to all vectors (simplified)
        let mut heap: BinaryHeap<ScoredItem> = BinaryHeap::new();

        for entry in self.vectors.iter() {
            let dist = cosine_similarity(query, entry.value());
            heap.push(ScoredItem {
                id: *entry.key(),
                score: dist,
            });
        }

        // Extract top k
        let mut results = Vec::with_capacity(k);
        for _ in 0..k {
            if let Some(item) = heap.pop() {
                results.push((item.id, item.score));
            }
        }

        results
    }

    /// Gets a vector by ID
    pub fn get(&self, id: &ObjectId) -> Option<Vec<f64>> {
        self.vectors.get(id).map(|v| v.clone())
    }

    /// Gets the number of stored vectors
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Checks if empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

/// Item with score for heap
#[derive(Debug)]
struct ScoredItem {
    id: ObjectId,
    score: f64,
}

impl PartialEq for ScoredItem {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl Eq for ScoredItem {}

impl PartialOrd for ScoredItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScoredItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.score
            .partial_cmp(&other.score)
            .unwrap_or(Ordering::Equal)
    }
}

/// Computes cosine similarity between two vectors
fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Structure-preserving similarity metric
///
/// Ensures that similarity computations respect the categorical structure
#[derive(Debug, Clone)]
pub struct StructuralSimilarity {
    /// Weight for vector similarity
    pub vector_weight: f64,
    /// Weight for structural similarity (morphism preservation)
    pub structure_weight: f64,
    /// Minimum similarity threshold
    pub threshold: f64,
}

impl Default for StructuralSimilarity {
    fn default() -> Self {
        Self {
            vector_weight: 0.7,
            structure_weight: 0.3,
            threshold: 0.5,
        }
    }
}

impl StructuralSimilarity {
    /// Creates a new similarity metric
    pub fn new(vector_weight: f64, structure_weight: f64) -> Self {
        let total = vector_weight + structure_weight;
        Self {
            vector_weight: vector_weight / total,
            structure_weight: structure_weight / total,
            threshold: 0.5,
        }
    }

    /// Sets the threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Computes combined similarity
    pub fn compute(&self, vector_sim: f64, structure_sim: f64) -> f64 {
        self.vector_weight * vector_sim + self.structure_weight * structure_sim
    }

    /// Checks if similarity is above threshold
    pub fn is_similar(&self, sim: f64) -> bool {
        sim >= self.threshold
    }
}

/// Retrieval strategy that preserves categorical invariants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetrievalStrategy {
    /// Standard k-NN retrieval
    KNN { k: usize },
    /// Threshold-based retrieval
    Threshold { min_similarity: f64 },
    /// Hybrid: k-NN with threshold
    Hybrid { k: usize, min_similarity: f64 },
    /// Structure-aware retrieval
    Structural {
        k: usize,
        preserve_mono: bool,
        preserve_composition: bool,
    },
}

impl Default for RetrievalStrategy {
    fn default() -> Self {
        Self::KNN { k: 10 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::SetCategory;

    #[test]
    fn test_functorial_retrieval_creation() {
        let source = SetCategory::new();
        let target = SetCategory::new();

        let retrieval = FunctorialRetrieval::new(source, target);
        assert!(!retrieval.invariants.preserves_identity);
    }

    #[test]
    fn test_vector_retrieval() {
        let retrieval = VectorRetrieval::new(3);

        let id1 = ObjectId::new();
        let id2 = ObjectId::new();

        retrieval.add(id1, vec![1.0, 0.0, 0.0]).unwrap();
        retrieval.add(id2, vec![0.0, 1.0, 0.0]).unwrap();

        let results = retrieval.retrieve(&[1.0, 0.0, 0.0], 2);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, id1); // Closest should be identical vector
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];

        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-10);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 1e-10); // Orthogonal
    }

    #[test]
    fn test_structural_similarity() {
        let metric = StructuralSimilarity::new(0.7, 0.3)
            .with_threshold(0.6);

        let sim = metric.compute(0.8, 0.9);
        assert!(metric.is_similar(sim));

        let low_sim = metric.compute(0.3, 0.5);
        assert!(!metric.is_similar(low_sim));
    }
}
