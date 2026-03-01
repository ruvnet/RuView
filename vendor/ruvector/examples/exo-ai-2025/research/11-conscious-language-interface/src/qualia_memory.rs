//! # Qualia Memory (ReasoningBank Integration)
//!
//! Extended ReasoningBank that stores conscious experiences (qualia)
//! for recall, learning, and memory consolidation.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::spike_embedding_bridge::PolychronousGroup;

/// A stored conscious experience with full qualia
#[derive(Debug, Clone)]
pub struct QualiaPattern {
    /// Unique pattern ID
    pub id: u64,
    /// Associated polychronous groups (spike patterns)
    pub spike_patterns: Vec<PolychronousGroup>,
    /// Semantic embedding of this qualia
    pub embedding: Vec<f32>,
    /// Φ level when this qualia occurred
    pub phi_level: f64,
    /// Emotional valence [-1.0, 1.0]
    pub valence: f32,
    /// Arousal level [0.0, 1.0]
    pub arousal: f32,
    /// Associated concepts (from language model)
    pub concepts: Vec<String>,
    /// Quality score from feedback
    pub quality: f32,
    /// Times this qualia has been re-experienced
    pub occurrence_count: u32,
    /// Creation timestamp
    pub created_at: Instant,
    /// Last access timestamp
    pub last_accessed: Instant,
}

impl QualiaPattern {
    pub fn new(
        id: u64,
        spike_patterns: Vec<PolychronousGroup>,
        embedding: Vec<f32>,
        phi_level: f64,
    ) -> Self {
        let now = Instant::now();
        Self {
            id,
            spike_patterns,
            embedding,
            phi_level,
            valence: 0.0,
            arousal: 0.5,
            concepts: Vec::new(),
            quality: 0.5,
            occurrence_count: 1,
            created_at: now,
            last_accessed: now,
        }
    }

    /// Compute similarity to another qualia pattern
    pub fn similarity(&self, other: &QualiaPattern) -> f32 {
        cosine_similarity(&self.embedding, &other.embedding)
    }

    /// Compute similarity to an embedding
    pub fn similarity_to_embedding(&self, embedding: &[f32]) -> f32 {
        cosine_similarity(&self.embedding, embedding)
    }

    /// Age in seconds
    pub fn age_secs(&self) -> u64 {
        self.created_at.elapsed().as_secs()
    }

    /// Time since last access
    pub fn idle_secs(&self) -> u64 {
        self.last_accessed.elapsed().as_secs()
    }
}

/// Valence-based memory organization
#[derive(Debug, Clone, Default)]
pub struct ValenceMemory {
    /// Positive valence patterns
    positive: Vec<u64>,
    /// Negative valence patterns
    negative: Vec<u64>,
    /// Neutral patterns
    neutral: Vec<u64>,
    /// Valence history for trend analysis
    valence_history: Vec<(Instant, f32)>,
}

impl ValenceMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Categorize pattern by valence
    pub fn add(&mut self, pattern_id: u64, valence: f32) {
        if valence > 0.3 {
            self.positive.push(pattern_id);
        } else if valence < -0.3 {
            self.negative.push(pattern_id);
        } else {
            self.neutral.push(pattern_id);
        }
        self.valence_history.push((Instant::now(), valence));
    }

    /// Get average valence over recent history
    pub fn average_valence(&self, window: Duration) -> f32 {
        let cutoff = Instant::now() - window;
        let recent: Vec<_> = self.valence_history
            .iter()
            .filter(|(t, _)| *t > cutoff)
            .map(|(_, v)| *v)
            .collect();

        if recent.is_empty() {
            0.0
        } else {
            recent.iter().sum::<f32>() / recent.len() as f32
        }
    }
}

/// Φ history tracking
#[derive(Debug, Clone, Default)]
pub struct PhiHistory {
    /// (timestamp, phi) pairs
    history: Vec<(Instant, f64)>,
    /// Running statistics
    total: f64,
    count: u64,
    max: f64,
    min: f64,
}

impl PhiHistory {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            total: 0.0,
            count: 0,
            max: 0.0,
            min: f64::MAX,
        }
    }

    pub fn record(&mut self, phi: f64) {
        self.history.push((Instant::now(), phi));
        self.total += phi;
        self.count += 1;
        self.max = self.max.max(phi);
        self.min = self.min.min(phi);

        // Keep history bounded
        if self.history.len() > 10000 {
            self.history.remove(0);
        }
    }

    pub fn average(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total / self.count as f64
        }
    }

    pub fn max(&self) -> f64 {
        self.max
    }

    pub fn min(&self) -> f64 {
        if self.min == f64::MAX { 0.0 } else { self.min }
    }

    pub fn recent_average(&self, window: Duration) -> f64 {
        let cutoff = Instant::now() - window;
        let recent: Vec<_> = self.history
            .iter()
            .filter(|(t, _)| *t > cutoff)
            .map(|(_, p)| *p)
            .collect();

        if recent.is_empty() {
            0.0
        } else {
            recent.iter().sum::<f64>() / recent.len() as f64
        }
    }
}

/// Qualia-enhanced ReasoningBank
pub struct QualiaReasoningBank {
    /// Stored qualia patterns
    patterns: HashMap<u64, QualiaPattern>,
    /// Next pattern ID
    next_id: u64,
    /// Valence-based organization
    valence_memory: ValenceMemory,
    /// Φ history
    phi_history: PhiHistory,
    /// Configuration
    max_patterns: usize,
    /// Similarity threshold for merging
    merge_threshold: f32,
}

impl QualiaReasoningBank {
    pub fn new(max_patterns: usize) -> Self {
        Self {
            patterns: HashMap::new(),
            next_id: 0,
            valence_memory: ValenceMemory::new(),
            phi_history: PhiHistory::new(),
            max_patterns,
            merge_threshold: 0.9,
        }
    }

    /// Store a new conscious experience
    pub fn store(&mut self, qualia: QualiaPattern) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        // Record in valence memory
        self.valence_memory.add(id, qualia.valence);

        // Record Φ
        self.phi_history.record(qualia.phi_level);

        // Check if similar pattern exists - collect IDs to avoid borrow issues
        let existing_id = {
            let similar = self.find_similar(&qualia.embedding, 1);
            similar.first()
                .filter(|p| p.similarity_to_embedding(&qualia.embedding) > self.merge_threshold)
                .map(|p| p.id)
        };

        if let Some(existing_id) = existing_id {
            // Merge with existing pattern
            if let Some(pattern) = self.patterns.get_mut(&existing_id) {
                pattern.occurrence_count += 1;
                pattern.quality = (pattern.quality + qualia.quality) / 2.0;
                pattern.last_accessed = Instant::now();
                return existing_id;
            }
        }

        // Store new pattern
        let mut pattern = qualia;
        pattern.id = id;
        self.patterns.insert(id, pattern);

        // Prune if over capacity
        if self.patterns.len() > self.max_patterns {
            self.prune_oldest();
        }

        id
    }

    /// Find similar qualia patterns
    pub fn find_similar(&self, embedding: &[f32], k: usize) -> Vec<&QualiaPattern> {
        let mut scored: Vec<_> = self.patterns
            .values()
            .map(|p| (p, p.similarity_to_embedding(embedding)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().take(k).map(|(p, _)| p).collect()
    }

    /// Recall a specific pattern (marks as accessed)
    pub fn recall(&mut self, pattern_id: u64) -> Option<&QualiaPattern> {
        if let Some(pattern) = self.patterns.get_mut(&pattern_id) {
            pattern.last_accessed = Instant::now();
            pattern.occurrence_count += 1;
        }
        self.patterns.get(&pattern_id)
    }

    /// Get patterns by valence
    pub fn get_positive_patterns(&self, k: usize) -> Vec<&QualiaPattern> {
        self.valence_memory.positive
            .iter()
            .filter_map(|id| self.patterns.get(id))
            .take(k)
            .collect()
    }

    pub fn get_negative_patterns(&self, k: usize) -> Vec<&QualiaPattern> {
        self.valence_memory.negative
            .iter()
            .filter_map(|id| self.patterns.get(id))
            .take(k)
            .collect()
    }

    /// Prune low-quality or old patterns
    pub fn prune(&mut self, min_quality: f32, min_occurrences: u32, max_age_secs: u64) {
        let to_remove: Vec<u64> = self.patterns
            .iter()
            .filter(|(_, p)| {
                p.quality < min_quality
                    || p.occurrence_count < min_occurrences
                    || p.age_secs() > max_age_secs
            })
            .map(|(id, _)| *id)
            .collect();

        for id in to_remove {
            self.patterns.remove(&id);
        }
    }

    /// Prune oldest patterns to make room
    fn prune_oldest(&mut self) {
        // Find oldest 10%
        let mut by_age: Vec<_> = self.patterns
            .iter()
            .map(|(id, p)| (*id, p.last_accessed))
            .collect();

        by_age.sort_by(|a, b| a.1.cmp(&b.1));

        let to_remove = by_age.len() / 10;
        for (id, _) in by_age.into_iter().take(to_remove) {
            self.patterns.remove(&id);
        }
    }

    /// Consolidate similar patterns (like memory consolidation during sleep)
    pub fn consolidate(&mut self) {
        let pattern_ids: Vec<u64> = self.patterns.keys().cloned().collect();
        let mut merged = Vec::new();
        let mut merge_actions: Vec<(u64, u64, u32, f32, Vec<f32>)> = Vec::new();

        // First pass: identify patterns to merge
        for i in 0..pattern_ids.len() {
            for j in (i + 1)..pattern_ids.len() {
                let id1 = pattern_ids[i];
                let id2 = pattern_ids[j];

                if merged.contains(&id1) || merged.contains(&id2) {
                    continue;
                }

                let p1 = self.patterns.get(&id1);
                let p2 = self.patterns.get(&id2);

                if let (Some(p1), Some(p2)) = (p1, p2) {
                    if p1.similarity(p2) > self.merge_threshold {
                        // Record merge action
                        merge_actions.push((
                            id1,
                            id2,
                            p2.occurrence_count,
                            p2.quality,
                            p2.embedding.clone(),
                        ));
                        merged.push(id2);
                    }
                }
            }
        }

        // Second pass: apply merges
        for (id1, _id2, occ_count, quality, embedding) in merge_actions {
            if let Some(pattern) = self.patterns.get_mut(&id1) {
                pattern.occurrence_count += occ_count;
                pattern.quality = (pattern.quality + quality) / 2.0;

                // Merge embeddings (weighted average)
                for (i, e) in pattern.embedding.iter_mut().enumerate() {
                    if i < embedding.len() {
                        *e = (*e + embedding[i]) / 2.0;
                    }
                }
            }
        }

        // Remove merged patterns
        for id in merged {
            self.patterns.remove(&id);
        }
    }

    /// Get statistics
    pub fn stats(&self) -> QualiaMemoryStats {
        QualiaMemoryStats {
            pattern_count: self.patterns.len(),
            avg_phi: self.phi_history.average(),
            max_phi: self.phi_history.max(),
            avg_valence: self.valence_memory.average_valence(Duration::from_secs(3600)),
            positive_count: self.valence_memory.positive.len(),
            negative_count: self.valence_memory.negative.len(),
            neutral_count: self.valence_memory.neutral.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QualiaMemoryStats {
    pub pattern_count: usize,
    pub avg_phi: f64,
    pub max_phi: f64,
    pub avg_valence: f32,
    pub positive_count: usize,
    pub negative_count: usize,
    pub neutral_count: usize,
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < 1e-6 || norm_b < 1e-6 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qualia_bank() {
        let mut bank = QualiaReasoningBank::new(100);

        let pattern = QualiaPattern {
            id: 0,
            spike_patterns: vec![],
            embedding: vec![1.0, 0.0, 0.0],
            phi_level: 50000.0,
            valence: 0.5,
            arousal: 0.6,
            concepts: vec!["test".to_string()],
            quality: 0.8,
            occurrence_count: 1,
            created_at: Instant::now(),
            last_accessed: Instant::now(),
        };

        let id = bank.store(pattern);
        assert!(bank.patterns.contains_key(&id));
    }

    #[test]
    fn test_similarity_search() {
        let mut bank = QualiaReasoningBank::new(100);

        // Store some patterns
        for i in 0..5 {
            let embedding = vec![i as f32, 0.0, 0.0];
            let pattern = QualiaPattern::new(0, vec![], embedding, 10000.0);
            bank.store(pattern);
        }

        // Search
        let query = vec![2.5, 0.0, 0.0];
        let similar = bank.find_similar(&query, 2);

        assert_eq!(similar.len(), 2);
    }

    #[test]
    fn test_valence_memory() {
        let mut valence = ValenceMemory::new();

        valence.add(1, 0.8);  // Positive
        valence.add(2, -0.6); // Negative
        valence.add(3, 0.1);  // Neutral

        assert_eq!(valence.positive.len(), 1);
        assert_eq!(valence.negative.len(), 1);
        assert_eq!(valence.neutral.len(), 1);
    }

    #[test]
    fn test_phi_history() {
        let mut history = PhiHistory::new();

        history.record(10000.0);
        history.record(20000.0);
        history.record(30000.0);

        assert_eq!(history.average(), 20000.0);
        assert_eq!(history.max(), 30000.0);
        assert_eq!(history.min(), 10000.0);
    }
}
