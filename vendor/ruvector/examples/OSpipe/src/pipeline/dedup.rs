//! Frame deduplication using cosine similarity.
//!
//! Maintains a sliding window of recent embeddings and checks new
//! frames against them to avoid storing near-duplicate content
//! (e.g., consecutive screen captures of the same static page).

use std::collections::VecDeque;

use crate::storage::embedding::cosine_similarity;
use uuid::Uuid;

/// Deduplicator that checks new embeddings against a sliding window
/// of recently stored embeddings.
pub struct FrameDeduplicator {
    /// Cosine similarity threshold above which a frame is considered duplicate.
    threshold: f32,
    /// Sliding window of recent embeddings (id, vector).
    recent_embeddings: VecDeque<(Uuid, Vec<f32>)>,
    /// Maximum number of recent embeddings to keep.
    window_size: usize,
}

impl FrameDeduplicator {
    /// Create a new deduplicator.
    ///
    /// - `threshold`: Cosine similarity threshold for duplicate detection (e.g., 0.95).
    /// - `window_size`: Number of recent embeddings to keep for comparison.
    pub fn new(threshold: f32, window_size: usize) -> Self {
        Self {
            threshold,
            recent_embeddings: VecDeque::with_capacity(window_size),
            window_size,
        }
    }

    /// Check if the given embedding is a duplicate of a recent entry.
    ///
    /// Returns `Some((id, similarity))` if a duplicate is found, where
    /// `id` is the ID of the matching recent embedding and `similarity`
    /// is the cosine similarity score.
    pub fn is_duplicate(&self, embedding: &[f32]) -> Option<(Uuid, f32)> {
        let mut best_match: Option<(Uuid, f32)> = None;

        for (id, stored_emb) in &self.recent_embeddings {
            if stored_emb.len() != embedding.len() {
                continue;
            }
            let sim = cosine_similarity(embedding, stored_emb);
            if sim >= self.threshold {
                match best_match {
                    Some((_, best_sim)) if sim > best_sim => {
                        best_match = Some((*id, sim));
                    }
                    None => {
                        best_match = Some((*id, sim));
                    }
                    _ => {}
                }
            }
        }

        best_match
    }

    /// Add an embedding to the sliding window.
    ///
    /// If the window is full, the oldest entry is evicted.
    pub fn add(&mut self, id: Uuid, embedding: Vec<f32>) {
        if self.recent_embeddings.len() >= self.window_size {
            self.recent_embeddings.pop_front();
        }
        self.recent_embeddings.push_back((id, embedding));
    }

    /// Return the current number of embeddings in the window.
    pub fn window_len(&self) -> usize {
        self.recent_embeddings.len()
    }

    /// Return the configured similarity threshold.
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Clear all entries from the sliding window.
    pub fn clear(&mut self) {
        self.recent_embeddings.clear();
    }
}
