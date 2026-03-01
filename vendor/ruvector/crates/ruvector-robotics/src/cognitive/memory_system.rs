//! Three-tier memory system: working, episodic, and semantic.
//!
//! - **Working memory**: bounded short-term buffer for active items.
//! - **Episodic memory**: stores temporally ordered episodes for experience replay.
//! - **Semantic memory**: long-term concept storage with similarity search.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Working memory
// ---------------------------------------------------------------------------

/// A single item held in working memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryItem {
    pub key: String,
    pub data: Vec<f64>,
    pub importance: f64,
    pub timestamp: i64,
    pub access_count: u64,
}

/// Bounded short-term buffer. Evicts the least-important item when full.
#[derive(Debug, Clone)]
pub struct WorkingMemory {
    items: Vec<MemoryItem>,
    max_size: usize,
}

impl WorkingMemory {
    /// Create a working memory with the given capacity.
    pub fn new(max_size: usize) -> Self {
        Self {
            items: Vec::new(),
            max_size,
        }
    }

    /// Insert an item. If the buffer is full the item with the lowest
    /// importance is evicted first.
    pub fn add(&mut self, item: MemoryItem) {
        if self.items.len() >= self.max_size {
            // Evict least important.
            if let Some((idx, _)) = self
                .items
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.importance
                        .partial_cmp(&b.importance)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            {
                self.items.remove(idx);
            }
        }
        self.items.push(item);
    }

    /// Retrieve an item by key, incrementing its access count.
    pub fn get(&mut self, key: &str) -> Option<&MemoryItem> {
        if let Some(item) = self.items.iter_mut().find(|i| i.key == key) {
            item.access_count += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Remove all items.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Current number of items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Episodic memory
// ---------------------------------------------------------------------------

/// A single episode consisting of percepts, actions, and a scalar reward.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub percepts: Vec<Vec<f64>>,
    pub actions: Vec<String>,
    pub reward: f64,
    pub timestamp: i64,
}

/// Stores temporally ordered episodes and supports similarity recall.
#[derive(Debug, Clone, Default)]
pub struct EpisodicMemory {
    episodes: Vec<Episode>,
}

impl EpisodicMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new episode.
    pub fn store(&mut self, episode: Episode) {
        self.episodes.push(episode);
    }

    /// Recall the `k` most similar episodes to `query` using dot-product
    /// similarity on the flattened percept vectors.
    pub fn recall_similar(&self, query: &[f64], k: usize) -> Vec<&Episode> {
        let mut scored: Vec<(f64, &Episode)> = self
            .episodes
            .iter()
            .map(|ep| {
                let flat: Vec<f64> = ep.percepts.iter().flatten().copied().collect();
                let sim = dot_product(query, &flat);
                (sim, ep)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(k).map(|(_, ep)| ep).collect()
    }

    /// Number of stored episodes.
    pub fn len(&self) -> usize {
        self.episodes.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.episodes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Semantic memory
// ---------------------------------------------------------------------------

/// Long-term concept storage mapping names to embedding vectors.
#[derive(Debug, Clone, Default)]
pub struct SemanticMemory {
    concepts: HashMap<String, Vec<f64>>,
}

impl SemanticMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a concept with the given name and embedding.
    pub fn store(&mut self, name: &str, embedding: Vec<f64>) {
        self.concepts.insert(name.to_string(), embedding);
    }

    /// Retrieve the embedding for a concept.
    pub fn retrieve(&self, name: &str) -> Option<&Vec<f64>> {
        self.concepts.get(name)
    }

    /// Find the `k` concepts most similar to `query` (dot-product).
    pub fn find_similar(&self, query: &[f64], k: usize) -> Vec<(&str, f64)> {
        let mut scored: Vec<(&str, f64)> = self
            .concepts
            .iter()
            .map(|(name, emb)| (name.as_str(), dot_product(query, emb)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(k).collect()
    }

    /// Number of stored concepts.
    pub fn len(&self) -> usize {
        self.concepts.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.concepts.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Dot product of two slices (truncated to the shorter length).
fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Working memory ---------------------------------------------------

    #[test]
    fn test_working_memory_add_get() {
        let mut wm = WorkingMemory::new(5);
        wm.add(MemoryItem {
            key: "obj1".into(),
            data: vec![1.0, 2.0],
            importance: 0.8,
            timestamp: 100,
            access_count: 0,
        });
        let item = wm.get("obj1").unwrap();
        assert_eq!(item.access_count, 1);
    }

    #[test]
    fn test_working_memory_eviction() {
        let mut wm = WorkingMemory::new(2);
        wm.add(MemoryItem {
            key: "a".into(),
            data: vec![],
            importance: 0.1,
            timestamp: 1,
            access_count: 0,
        });
        wm.add(MemoryItem {
            key: "b".into(),
            data: vec![],
            importance: 0.9,
            timestamp: 2,
            access_count: 0,
        });
        wm.add(MemoryItem {
            key: "c".into(),
            data: vec![],
            importance: 0.5,
            timestamp: 3,
            access_count: 0,
        });
        assert_eq!(wm.len(), 2);
        // "a" (importance 0.1) should have been evicted.
        assert!(wm.get("a").is_none());
        assert!(wm.get("b").is_some());
    }

    #[test]
    fn test_working_memory_clear() {
        let mut wm = WorkingMemory::new(5);
        wm.add(MemoryItem {
            key: "x".into(),
            data: vec![],
            importance: 1.0,
            timestamp: 0,
            access_count: 0,
        });
        assert!(!wm.is_empty());
        wm.clear();
        assert!(wm.is_empty());
    }

    #[test]
    fn test_working_memory_get_missing() {
        let mut wm = WorkingMemory::new(5);
        assert!(wm.get("nonexistent").is_none());
    }

    // -- Episodic memory --------------------------------------------------

    #[test]
    fn test_episodic_store_recall() {
        let mut em = EpisodicMemory::new();
        em.store(Episode {
            percepts: vec![vec![1.0, 0.0, 0.0]],
            actions: vec!["move".into()],
            reward: 1.0,
            timestamp: 100,
        });
        em.store(Episode {
            percepts: vec![vec![0.0, 1.0, 0.0]],
            actions: vec!["turn".into()],
            reward: 0.5,
            timestamp: 200,
        });
        let results = em.recall_similar(&[1.0, 0.0, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].actions[0], "move");
    }

    #[test]
    fn test_episodic_empty_recall() {
        let em = EpisodicMemory::new();
        let results = em.recall_similar(&[1.0], 5);
        assert!(results.is_empty());
    }

    #[test]
    fn test_episodic_len() {
        let mut em = EpisodicMemory::new();
        assert!(em.is_empty());
        em.store(Episode {
            percepts: vec![],
            actions: vec![],
            reward: 0.0,
            timestamp: 0,
        });
        assert_eq!(em.len(), 1);
    }

    // -- Semantic memory --------------------------------------------------

    #[test]
    fn test_semantic_store_retrieve() {
        let mut sm = SemanticMemory::new();
        sm.store("cup", vec![1.0, 0.0, 0.0]);
        let emb = sm.retrieve("cup").unwrap();
        assert_eq!(emb, &vec![1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_semantic_find_similar() {
        let mut sm = SemanticMemory::new();
        sm.store("cup", vec![1.0, 0.0, 0.0]);
        sm.store("plate", vec![0.9, 0.1, 0.0]);
        sm.store("ball", vec![0.0, 0.0, 1.0]);
        let results = sm.find_similar(&[1.0, 0.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "cup");
    }

    #[test]
    fn test_semantic_retrieve_missing() {
        let sm = SemanticMemory::new();
        assert!(sm.retrieve("nothing").is_none());
    }

    #[test]
    fn test_semantic_len() {
        let mut sm = SemanticMemory::new();
        assert!(sm.is_empty());
        sm.store("a", vec![]);
        assert_eq!(sm.len(), 1);
    }

    // -- Helpers ----------------------------------------------------------

    #[test]
    fn test_dot_product() {
        assert!((dot_product(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]) - 32.0).abs() < 1e-9);
    }
}
