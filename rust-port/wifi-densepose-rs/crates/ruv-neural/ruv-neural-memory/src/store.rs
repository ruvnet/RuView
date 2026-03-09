//! In-memory embedding store with brute-force nearest neighbor search.

use std::collections::HashMap;
use std::collections::VecDeque;

use ruv_neural_core::embedding::NeuralEmbedding;
use ruv_neural_core::error::Result;
use ruv_neural_core::topology::CognitiveState;
use ruv_neural_core::traits::NeuralMemory;

/// In-memory store for neural embeddings with index-based retrieval.
///
/// Uses a VecDeque for O(1) front eviction instead of Vec::remove(0) which is O(n).
#[derive(Debug, Clone)]
pub struct NeuralMemoryStore {
    /// All stored embeddings in insertion order.
    embeddings: VecDeque<NeuralEmbedding>,
    /// Maps subject_id to the indices of their embeddings.
    index: HashMap<String, Vec<usize>>,
    /// Maximum number of embeddings to store.
    capacity: usize,
    /// Running offset: total number of embeddings ever evicted.
    /// Logical index = physical index + evicted_count.
    evicted_count: usize,
}

impl NeuralMemoryStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            embeddings: VecDeque::with_capacity(capacity.min(1024)),
            index: HashMap::new(),
            capacity,
            evicted_count: 0,
        }
    }

    /// Store an embedding, returning its physical index within the deque.
    ///
    /// If the store is at capacity, the oldest embedding is evicted.
    /// Returns an error if the embedding dimension is inconsistent with
    /// previously stored embeddings.
    pub fn store(&mut self, embedding: NeuralEmbedding) -> Result<usize> {
        // Check dimension consistency with existing embeddings
        if let Some(first) = self.embeddings.front() {
            if embedding.dimension != first.dimension {
                return Err(ruv_neural_core::error::RuvNeuralError::DimensionMismatch {
                    expected: first.dimension,
                    got: embedding.dimension,
                });
            }
        }

        if self.embeddings.len() >= self.capacity {
            self.evict_oldest();
        }

        let idx = self.embeddings.len();

        if let Some(ref subject_id) = embedding.metadata.subject_id {
            self.index
                .entry(subject_id.clone())
                .or_default()
                .push(idx);
        }

        self.embeddings.push_back(embedding);
        Ok(idx)
    }

    /// Get an embedding by its index.
    pub fn get(&self, id: usize) -> Option<&NeuralEmbedding> {
        self.embeddings.get(id)
    }

    /// Number of embeddings currently stored.
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Returns true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Find the k nearest neighbors using brute-force Euclidean distance.
    ///
    /// Returns pairs of (index, distance), sorted by ascending distance.
    pub fn query_nearest(&self, query: &NeuralEmbedding, k: usize) -> Vec<(usize, f64)> {
        let mut distances: Vec<(usize, f64)> = self
            .embeddings
            .iter()
            .enumerate()
            .filter_map(|(i, emb)| {
                emb.euclidean_distance(query).ok().map(|d| (i, d))
            })
            .collect();

        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        distances.truncate(k);
        distances
    }

    /// Query all embeddings matching a given cognitive state.
    pub fn query_by_state(&self, state: CognitiveState) -> Vec<&NeuralEmbedding> {
        self.embeddings
            .iter()
            .filter(|e| e.metadata.cognitive_state == Some(state))
            .collect()
    }

    /// Query all embeddings for a given subject.
    pub fn query_by_subject(&self, subject_id: &str) -> Vec<&NeuralEmbedding> {
        match self.index.get(subject_id) {
            Some(indices) => indices
                .iter()
                .filter_map(|&i| self.embeddings.get(i))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Query embeddings within a timestamp range [start, end].
    pub fn query_time_range(&self, start: f64, end: f64) -> Vec<&NeuralEmbedding> {
        self.embeddings
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect()
    }

    /// Access all embeddings (for serialization).
    ///
    /// Returns the two slices of the VecDeque as a pair. For contiguous access,
    /// callers can use `make_contiguous()` on a mutable reference, or iterate.
    pub fn embeddings_iter(&self) -> impl Iterator<Item = &NeuralEmbedding> {
        self.embeddings.iter()
    }

    /// Access all embeddings as a slice pair (VecDeque may be non-contiguous).
    pub fn embeddings(&self) -> Vec<&NeuralEmbedding> {
        self.embeddings.iter().collect()
    }

    /// Get the capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Evict the oldest embedding with O(1) pop and incremental index update.
    ///
    /// Instead of rebuilding the entire index, we remove the evicted entry
    /// from the subject index and decrement all remaining indices by 1.
    fn evict_oldest(&mut self) {
        if self.embeddings.is_empty() {
            return;
        }

        let evicted = self.embeddings.pop_front().unwrap();
        self.evicted_count += 1;

        // Remove index 0 from the evicted embedding's subject entry.
        if let Some(ref subject_id) = evicted.metadata.subject_id {
            if let Some(indices) = self.index.get_mut(subject_id) {
                indices.retain(|&i| i != 0);
            }
        }

        // Decrement all indices by 1 since front was removed.
        for indices in self.index.values_mut() {
            for idx in indices.iter_mut() {
                *idx -= 1;
            }
        }

        // Clean up empty entries.
        self.index.retain(|_, v| !v.is_empty());
    }
}

impl NeuralMemory for NeuralMemoryStore {
    fn store(&mut self, embedding: &NeuralEmbedding) -> Result<()> {
        NeuralMemoryStore::store(self, embedding.clone())?;
        Ok(())
    }

    fn query_nearest(
        &self,
        embedding: &NeuralEmbedding,
        k: usize,
    ) -> Result<Vec<NeuralEmbedding>> {
        let results = NeuralMemoryStore::query_nearest(self, embedding, k);
        Ok(results
            .into_iter()
            .filter_map(|(i, _)| self.get(i).cloned())
            .collect())
    }

    fn query_by_state(&self, state: CognitiveState) -> Result<Vec<NeuralEmbedding>> {
        Ok(NeuralMemoryStore::query_by_state(self, state)
            .into_iter()
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::embedding::EmbeddingMetadata;

    fn make_embedding(vector: Vec<f64>, subject: &str, timestamp: f64) -> NeuralEmbedding {
        NeuralEmbedding::new(
            vector,
            timestamp,
            EmbeddingMetadata {
                subject_id: Some(subject.to_string()),
                session_id: None,
                cognitive_state: Some(CognitiveState::Rest),
                source_atlas: Atlas::Schaefer100,
                embedding_method: "test".to_string(),
            },
        )
        .unwrap()
    }

    fn make_embedding_with_state(
        vector: Vec<f64>,
        state: CognitiveState,
        timestamp: f64,
    ) -> NeuralEmbedding {
        NeuralEmbedding::new(
            vector,
            timestamp,
            EmbeddingMetadata {
                subject_id: Some("subj1".to_string()),
                session_id: None,
                cognitive_state: Some(state),
                source_atlas: Atlas::Schaefer100,
                embedding_method: "test".to_string(),
            },
        )
        .unwrap()
    }

    #[test]
    fn store_and_retrieve() {
        let mut store = NeuralMemoryStore::new(100);
        let emb = make_embedding(vec![1.0, 2.0, 3.0], "subj1", 0.0);
        let idx = store.store(emb.clone()).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(store.len(), 1);

        let retrieved = store.get(0).unwrap();
        assert_eq!(retrieved.vector, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn nearest_neighbor_returns_correct_results() {
        let mut store = NeuralMemoryStore::new(100);
        store
            .store(make_embedding(vec![0.0, 0.0, 0.0], "a", 0.0))
            .unwrap();
        store
            .store(make_embedding(vec![1.0, 0.0, 0.0], "b", 1.0))
            .unwrap();
        store
            .store(make_embedding(vec![10.0, 10.0, 10.0], "c", 2.0))
            .unwrap();

        let query = make_embedding(vec![0.5, 0.0, 0.0], "q", 3.0);
        let results = store.query_nearest(&query, 2);

        assert_eq!(results.len(), 2);
        // Closest should be [0,0,0] (dist=0.5) then [1,0,0] (dist=0.5)
        assert!(results[0].1 <= results[1].1);
    }

    #[test]
    fn query_by_state_filters_correctly() {
        let mut store = NeuralMemoryStore::new(100);
        store
            .store(make_embedding_with_state(
                vec![1.0, 0.0],
                CognitiveState::Rest,
                0.0,
            ))
            .unwrap();
        store
            .store(make_embedding_with_state(
                vec![0.0, 1.0],
                CognitiveState::Focused,
                1.0,
            ))
            .unwrap();
        store
            .store(make_embedding_with_state(
                vec![1.0, 1.0],
                CognitiveState::Rest,
                2.0,
            ))
            .unwrap();

        let resting = store.query_by_state(CognitiveState::Rest);
        assert_eq!(resting.len(), 2);

        let focused = store.query_by_state(CognitiveState::Focused);
        assert_eq!(focused.len(), 1);
    }

    #[test]
    fn query_by_subject() {
        let mut store = NeuralMemoryStore::new(100);
        store
            .store(make_embedding(vec![1.0, 0.0], "alice", 0.0))
            .unwrap();
        store
            .store(make_embedding(vec![0.0, 1.0], "bob", 1.0))
            .unwrap();
        store
            .store(make_embedding(vec![1.0, 1.0], "alice", 2.0))
            .unwrap();

        let alice = store.query_by_subject("alice");
        assert_eq!(alice.len(), 2);

        let bob = store.query_by_subject("bob");
        assert_eq!(bob.len(), 1);

        let unknown = store.query_by_subject("charlie");
        assert_eq!(unknown.len(), 0);
    }

    #[test]
    fn query_time_range() {
        let mut store = NeuralMemoryStore::new(100);
        store
            .store(make_embedding(vec![1.0], "a", 1.0))
            .unwrap();
        store
            .store(make_embedding(vec![2.0], "a", 5.0))
            .unwrap();
        store
            .store(make_embedding(vec![3.0], "a", 10.0))
            .unwrap();

        let in_range = store.query_time_range(2.0, 8.0);
        assert_eq!(in_range.len(), 1);
        assert_eq!(in_range[0].vector, vec![2.0]);

        let all = store.query_time_range(0.0, 20.0);
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn capacity_eviction() {
        let mut store = NeuralMemoryStore::new(2);
        store
            .store(make_embedding(vec![1.0], "a", 0.0))
            .unwrap();
        store
            .store(make_embedding(vec![2.0], "b", 1.0))
            .unwrap();
        assert_eq!(store.len(), 2);

        // This should evict the oldest
        store
            .store(make_embedding(vec![3.0], "c", 2.0))
            .unwrap();
        assert_eq!(store.len(), 2);
        // First element should now be [2.0]
        assert_eq!(store.get(0).unwrap().vector, vec![2.0]);
    }
}
