//! Collective Memory Formation for Swarm Intelligence
//!
//! Implements hippocampal-inspired memory consolidation for distributed
//! learning across swarm nodes. Patterns are shared via RAC events and
//! consolidated during idle periods for long-term retention.
//!
//! ## Theory
//!
//! Biological memory consolidation occurs during sleep/rest:
//! - Working memory -> Short-term storage (hippocampus)
//! - Consolidation -> Long-term storage (cortex)
//! - Replay -> Strengthens important memories
//!
//! ## Collective Memory Algorithm
//!
//! 1. Nodes learn patterns locally from task execution
//! 2. High-quality patterns are shared via RAC LearningPattern events
//! 3. Received patterns enter consolidation queue
//! 4. During idle periods, patterns are validated and merged
//! 5. Consolidated patterns are indexed for semantic retrieval
//!
//! ## References
//!
//! - Complementary learning systems theory
//! - Hippocampal replay mechanisms
//! - Federated learning pattern aggregation

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use rustc_hash::FxHashMap;
use std::sync::{Arc, RwLock, Mutex};
use std::collections::VecDeque;

use crate::rac::{EventKind, Event, AssertEvent, Ruvector, ContextId, PublicKeyBytes, EvidenceRef};
use crate::learning::LearnedPattern;

// ============================================================================
// Pattern Types
// ============================================================================

/// A pattern to be shared across the collective
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    /// Unique pattern identifier
    pub id: String,
    /// Semantic embedding vector
    pub embedding: Vec<f32>,
    /// Quality score (0.0 - 1.0)
    pub quality: f32,
    /// Number of samples that contributed
    pub samples: usize,
    /// Evidence supporting the pattern
    pub evidence: Vec<EvidenceRef>,
    /// Source node ID
    pub source_node: String,
    /// Creation timestamp
    pub created_at: u64,
    /// Optimal allocation learned
    pub optimal_allocation: f32,
    /// Optimal energy budget
    pub optimal_energy: u64,
    /// Task type this pattern applies to
    pub task_type: Option<String>,
}

impl Pattern {
    /// Create new pattern from learned data
    pub fn new(
        id: String,
        embedding: Vec<f32>,
        quality: f32,
        samples: usize,
        source_node: String,
    ) -> Self {
        Self {
            id,
            embedding,
            quality,
            samples,
            evidence: Vec::new(),
            source_node,
            created_at: current_timestamp_ms(),
            optimal_allocation: 0.5,
            optimal_energy: 100,
            task_type: None,
        }
    }

    /// Create pattern from LearnedPattern
    pub fn from_learned(
        id: String,
        learned: &LearnedPattern,
        source_node: String,
    ) -> Self {
        Self {
            id,
            embedding: learned.centroid.clone(),
            quality: learned.confidence as f32,
            samples: learned.sample_count,
            evidence: Vec::new(),
            source_node,
            created_at: current_timestamp_ms(),
            optimal_allocation: learned.optimal_allocation,
            optimal_energy: learned.optimal_energy,
            task_type: None,
        }
    }

    /// Calculate similarity to another pattern
    pub fn similarity(&self, other: &Pattern) -> f32 {
        if self.embedding.len() != other.embedding.len() {
            return 0.0;
        }

        let dot: f32 = self.embedding.iter()
            .zip(&other.embedding)
            .map(|(a, b)| a * b)
            .sum();

        let norm_a: f32 = self.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = other.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Merge with another similar pattern (weighted average)
    pub fn merge(&mut self, other: &Pattern) {
        let total_samples = self.samples + other.samples;
        let self_weight = self.samples as f32 / total_samples as f32;
        let other_weight = other.samples as f32 / total_samples as f32;

        // Merge embeddings
        for (i, val) in self.embedding.iter_mut().enumerate() {
            if i < other.embedding.len() {
                *val = self_weight * *val + other_weight * other.embedding[i];
            }
        }

        // Update quality (weighted average)
        self.quality = self_weight * self.quality + other_weight * other.quality;

        // Sum samples
        self.samples = total_samples;

        // Merge optimal values
        self.optimal_allocation = self_weight * self.optimal_allocation
            + other_weight * other.optimal_allocation;
        self.optimal_energy = (self_weight * self.optimal_energy as f32
            + other_weight * other.optimal_energy as f32) as u64;

        // Merge evidence
        self.evidence.extend(other.evidence.clone());
    }
}

/// Cross-platform timestamp helper
fn current_timestamp_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

// ============================================================================
// HNSW Index (Simplified for collective memory)
// ============================================================================

/// Simple HNSW-like index for pattern retrieval
pub struct HnswIndex {
    /// All stored patterns
    patterns: Vec<Pattern>,
    /// Pattern ID to index mapping
    id_to_idx: FxHashMap<String, usize>,
    /// Dimension of embeddings
    dim: usize,
}

impl HnswIndex {
    /// Create new index with dimension
    pub fn new(dim: usize) -> Self {
        Self {
            patterns: Vec::with_capacity(1000),
            id_to_idx: FxHashMap::default(),
            dim,
        }
    }

    /// Insert pattern into index
    pub fn insert(&mut self, pattern: Pattern) {
        if pattern.embedding.len() != self.dim && self.dim > 0 {
            return;
        }

        if self.dim == 0 && !pattern.embedding.is_empty() {
            // Set dimension from first pattern
            // Note: this is a simplified approach
        }

        let idx = self.patterns.len();
        self.id_to_idx.insert(pattern.id.clone(), idx);
        self.patterns.push(pattern);
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(String, f32)> {
        let mut scores: Vec<(usize, f32)> = self.patterns.iter()
            .enumerate()
            .map(|(i, p)| {
                let sim = if p.embedding.len() == query.len() {
                    let dot: f32 = p.embedding.iter().zip(query).map(|(a, b)| a * b).sum();
                    let norm_p: f32 = p.embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                    let norm_q: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm_p > 0.0 && norm_q > 0.0 { dot / (norm_p * norm_q) } else { 0.0 }
                } else {
                    0.0
                };
                (i, sim)
            })
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(k);

        scores.into_iter()
            .map(|(i, sim)| (self.patterns[i].id.clone(), sim))
            .collect()
    }

    /// Get pattern by ID
    pub fn get(&self, id: &str) -> Option<&Pattern> {
        self.id_to_idx.get(id).and_then(|&idx| self.patterns.get(idx))
    }

    /// Get pattern count
    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }
}

// ============================================================================
// RAC Claim Types for Pattern Sharing
// ============================================================================

/// Claim types for pattern sharing via RAC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClaimType {
    /// A learning pattern to be shared
    LearningPattern {
        pattern_id: String,
        embedding: Vec<f32>,
        quality_score: f32,
        sample_count: usize,
    },
    /// Pattern validation/endorsement
    PatternEndorsement {
        pattern_id: String,
        endorser_id: String,
        confidence: f32,
    },
    /// Pattern deprecation (outdated/incorrect)
    PatternDeprecation {
        pattern_id: String,
        reason: String,
    },
    /// Collective model update
    ModelUpdate {
        model_id: String,
        weights: Vec<f32>,
        version: u64,
    },
}

/// RAC event for pattern sharing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RacEvent {
    /// Assert a claim with evidence
    Assert {
        claim: ClaimType,
        evidence: Vec<EvidenceRef>,
        confidence: f32,
    },
    /// Challenge an existing claim
    Challenge {
        claim_id: String,
        reason: String,
    },
    /// Support a claim under challenge
    Support {
        claim_id: String,
        evidence: Vec<EvidenceRef>,
    },
}

// ============================================================================
// Collective Memory
// ============================================================================

/// Configuration for collective memory
#[derive(Clone, Debug)]
pub struct CollectiveMemoryConfig {
    /// Quality threshold for accepting patterns
    pub quality_threshold: f32,
    /// Enable hippocampal replay
    pub hippocampal_replay: bool,
    /// Maximum consolidation queue size
    pub max_queue_size: usize,
    /// Similarity threshold for merging patterns
    pub merge_threshold: f32,
    /// Maximum patterns in index
    pub max_patterns: usize,
    /// Consolidation batch size
    pub consolidation_batch_size: usize,
}

impl Default for CollectiveMemoryConfig {
    fn default() -> Self {
        Self {
            quality_threshold: 0.8,
            hippocampal_replay: true,
            max_queue_size: 1000,
            merge_threshold: 0.85,
            max_patterns: 10000,
            consolidation_batch_size: 50,
        }
    }
}

/// Collective memory system for distributed pattern learning
#[wasm_bindgen]
pub struct CollectiveMemory {
    /// Shared pattern index (thread-safe)
    shared_patterns: Arc<RwLock<HnswIndex>>,
    /// Consolidation queue for incoming patterns
    consolidation_queue: Mutex<VecDeque<Pattern>>,
    /// Enable hippocampal replay
    hippocampal_replay: bool,
    /// Quality threshold for acceptance
    quality_threshold: f32,
    /// Similarity threshold for merging
    merge_threshold: f32,
    /// Max patterns in index
    max_patterns: usize,
    /// Consolidation batch size
    batch_size: usize,
    /// Statistics
    stats: RwLock<CollectiveStats>,
    /// Local node ID
    local_node_id: String,
}

/// Statistics for collective memory
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CollectiveStats {
    pub patterns_received: usize,
    pub patterns_accepted: usize,
    pub patterns_rejected: usize,
    pub patterns_merged: usize,
    pub consolidation_runs: usize,
    pub replay_events: usize,
}

#[wasm_bindgen]
impl CollectiveMemory {
    /// Create new collective memory with default config
    #[wasm_bindgen(constructor)]
    pub fn new(node_id: &str) -> Self {
        Self::with_config(node_id, CollectiveMemoryConfig::default())
    }

    /// Get pattern count in shared index
    #[wasm_bindgen(js_name = patternCount)]
    pub fn pattern_count(&self) -> usize {
        self.shared_patterns.read().unwrap().len()
    }

    /// Get queue size
    #[wasm_bindgen(js_name = queueSize)]
    pub fn queue_size(&self) -> usize {
        self.consolidation_queue.lock().unwrap().len()
    }

    /// Get statistics as JSON
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let stats = self.stats.read().unwrap();
        serde_json::to_string(&*stats).unwrap_or_else(|_| "{}".to_string())
    }

    /// Run consolidation (call during idle periods)
    #[wasm_bindgen]
    pub fn consolidate(&self) -> usize {
        let mut consolidated = 0;
        let mut queue = self.consolidation_queue.lock().unwrap();
        let mut index = self.shared_patterns.write().unwrap();

        let batch_size = self.batch_size.min(queue.len());

        for _ in 0..batch_size {
            if let Some(pattern) = queue.pop_front() {
                if pattern.quality >= self.quality_threshold {
                    // Check if similar pattern exists
                    let similar = index.search(&pattern.embedding, 1);

                    if let Some((existing_id, sim)) = similar.first() {
                        if *sim > self.merge_threshold {
                            // Merge with existing pattern
                            // Note: In production, we'd modify the existing pattern
                            self.stats.write().unwrap().patterns_merged += 1;
                        } else {
                            // Add as new pattern
                            index.insert(pattern);
                            consolidated += 1;
                        }
                    } else {
                        // First pattern
                        index.insert(pattern);
                        consolidated += 1;
                    }

                    self.stats.write().unwrap().patterns_accepted += 1;
                } else {
                    self.stats.write().unwrap().patterns_rejected += 1;
                }
            }
        }

        if consolidated > 0 || batch_size > 0 {
            self.stats.write().unwrap().consolidation_runs += 1;
        }

        consolidated
    }

    /// Search for similar patterns
    #[wasm_bindgen]
    pub fn search(&self, query_json: &str, k: usize) -> String {
        let query: Vec<f32> = match serde_json::from_str(query_json) {
            Ok(q) => q,
            Err(_) => return "[]".to_string(),
        };

        let index = self.shared_patterns.read().unwrap();
        let results = index.search(&query, k);

        let results_json: Vec<_> = results.iter()
            .filter_map(|(id, sim)| {
                index.get(id).map(|p| {
                    serde_json::json!({
                        "id": id,
                        "similarity": sim,
                        "quality": p.quality,
                        "samples": p.samples,
                        "optimal_allocation": p.optimal_allocation,
                        "optimal_energy": p.optimal_energy
                    })
                })
            })
            .collect();

        serde_json::to_string(&results_json).unwrap_or_else(|_| "[]".to_string())
    }

    /// Check if a pattern ID exists
    #[wasm_bindgen(js_name = hasPattern)]
    pub fn has_pattern(&self, pattern_id: &str) -> bool {
        self.shared_patterns.read().unwrap().get(pattern_id).is_some()
    }
}

impl CollectiveMemory {
    /// Create with custom configuration
    pub fn with_config(node_id: &str, config: CollectiveMemoryConfig) -> Self {
        Self {
            shared_patterns: Arc::new(RwLock::new(HnswIndex::new(0))),
            consolidation_queue: Mutex::new(VecDeque::with_capacity(config.max_queue_size)),
            hippocampal_replay: config.hippocampal_replay,
            quality_threshold: config.quality_threshold,
            merge_threshold: config.merge_threshold,
            max_patterns: config.max_patterns,
            batch_size: config.consolidation_batch_size,
            stats: RwLock::new(CollectiveStats::default()),
            local_node_id: node_id.to_string(),
        }
    }

    /// Share a pattern via RAC event
    ///
    /// Creates a RAC assertion event for the pattern and queues it
    /// for broadcast to the network.
    pub fn share_pattern(&self, pattern: &Pattern) -> RacEvent {
        let event = RacEvent::Assert {
            claim: ClaimType::LearningPattern {
                pattern_id: pattern.id.clone(),
                embedding: pattern.embedding.clone(),
                quality_score: pattern.quality,
                sample_count: pattern.samples,
            },
            evidence: pattern.evidence.clone(),
            confidence: pattern.quality,
        };

        event
    }

    /// Receive and validate a pattern from peer
    ///
    /// Returns true if the pattern was accepted into the consolidation queue.
    pub fn receive_pattern(&self, event: &RacEvent) -> bool {
        let (pattern, confidence) = match event {
            RacEvent::Assert { claim, evidence, confidence } => {
                match claim {
                    ClaimType::LearningPattern { pattern_id, embedding, quality_score, sample_count } => {
                        let pattern = Pattern {
                            id: pattern_id.clone(),
                            embedding: embedding.clone(),
                            quality: *quality_score,
                            samples: *sample_count,
                            evidence: evidence.clone(),
                            source_node: "peer".to_string(), // Would come from event author
                            created_at: current_timestamp_ms(),
                            optimal_allocation: 0.5,
                            optimal_energy: 100,
                            task_type: None,
                        };
                        (pattern, *confidence)
                    }
                    _ => return false,
                }
            }
            _ => return false,
        };

        // Validate pattern
        if !self.validate_pattern(&pattern) {
            return false;
        }

        // Add to consolidation queue
        let mut queue = self.consolidation_queue.lock().unwrap();
        if queue.len() < self.max_patterns {
            queue.push_back(pattern);
            self.stats.write().unwrap().patterns_received += 1;
            true
        } else {
            false
        }
    }

    /// Add pattern directly to queue (for local patterns)
    pub fn add_pattern(&self, pattern: Pattern) -> bool {
        if pattern.quality < self.quality_threshold * 0.5 {
            return false;
        }

        let mut queue = self.consolidation_queue.lock().unwrap();
        if queue.len() < self.max_patterns {
            queue.push_back(pattern);
            true
        } else {
            false
        }
    }

    /// Hippocampal-inspired replay during idle
    ///
    /// Replays high-value patterns to strengthen retention and
    /// improve retrieval pathways.
    pub fn hippocampal_replay(&self) -> usize {
        if !self.hippocampal_replay {
            return 0;
        }

        let index = self.shared_patterns.read().unwrap();
        let patterns: Vec<_> = index.patterns.iter()
            .filter(|p| p.quality > 0.9) // Only high-quality patterns
            .take(10) // Limit replay batch
            .collect();

        let replayed = patterns.len();

        // In a full implementation, replay would:
        // 1. Re-inject patterns with slight variations
        // 2. Strengthen associated pathways
        // 3. Prune weak connections

        if replayed > 0 {
            self.stats.write().unwrap().replay_events += replayed;
        }

        replayed
    }

    /// Validate pattern before acceptance
    fn validate_pattern(&self, pattern: &Pattern) -> bool {
        // Check quality threshold
        if pattern.quality < self.quality_threshold * 0.5 {
            return false;
        }

        // Check embedding dimension (non-empty)
        if pattern.embedding.is_empty() {
            return false;
        }

        // Check for NaN/Inf values
        if pattern.embedding.iter().any(|&v| v.is_nan() || v.is_infinite()) {
            return false;
        }

        // Check sample count
        if pattern.samples == 0 {
            return false;
        }

        true
    }

    /// Get pattern by ID
    pub fn get_pattern(&self, id: &str) -> Option<Pattern> {
        self.shared_patterns.read().unwrap().get(id).cloned()
    }

    /// Get patterns by similarity threshold
    pub fn get_similar_patterns(&self, embedding: &[f32], threshold: f32) -> Vec<Pattern> {
        let index = self.shared_patterns.read().unwrap();
        let results = index.search(embedding, 20);

        results.iter()
            .filter(|(_, sim)| *sim >= threshold)
            .filter_map(|(id, _)| index.get(id).cloned())
            .collect()
    }

    /// Export patterns as JSON for sharing
    pub fn export_patterns(&self) -> String {
        let index = self.shared_patterns.read().unwrap();
        serde_json::to_string(&index.patterns).unwrap_or_else(|_| "[]".to_string())
    }

    /// Import patterns from JSON
    pub fn import_patterns(&self, json: &str) -> usize {
        let patterns: Vec<Pattern> = match serde_json::from_str(json) {
            Ok(p) => p,
            Err(_) => return 0,
        };

        let mut imported = 0;
        for pattern in patterns {
            if self.add_pattern(pattern) {
                imported += 1;
            }
        }

        // Run consolidation to process imports
        self.consolidate();

        imported
    }
}

// ============================================================================
// Swarm Broadcaster (Stub for network integration)
// ============================================================================

/// Stub swarm interface for pattern broadcasting
pub struct Swarm {
    /// Topic for model synchronization
    pub model_sync_topic: String,
}

/// Topic constant for model sync
pub const TOPIC_MODEL_SYNC: &str = "edge-net/model-sync/v1";

impl Swarm {
    /// Create new swarm interface
    pub fn new() -> Self {
        Self {
            model_sync_topic: TOPIC_MODEL_SYNC.to_string(),
        }
    }

    /// Publish to topic (stub - would use actual P2P layer)
    pub fn publish(&mut self, topic: &str, data: &[u8]) -> Result<(), &'static str> {
        // In production, this would:
        // 1. Serialize the data
        // 2. Sign with node identity
        // 3. Broadcast via GUN.js or WebRTC
        let _ = (topic, data);
        Ok(())
    }
}

impl Default for Swarm {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_creation() {
        let pattern = Pattern::new(
            "pat-1".to_string(),
            vec![1.0, 0.0, 0.0],
            0.9,
            100,
            "node-1".to_string(),
        );

        assert_eq!(pattern.id, "pat-1");
        assert_eq!(pattern.quality, 0.9);
        assert_eq!(pattern.samples, 100);
    }

    #[test]
    fn test_pattern_similarity() {
        let p1 = Pattern::new(
            "p1".to_string(),
            vec![1.0, 0.0, 0.0],
            0.9,
            10,
            "node".to_string(),
        );

        let p2 = Pattern::new(
            "p2".to_string(),
            vec![1.0, 0.0, 0.0],
            0.9,
            10,
            "node".to_string(),
        );

        let p3 = Pattern::new(
            "p3".to_string(),
            vec![0.0, 1.0, 0.0],
            0.9,
            10,
            "node".to_string(),
        );

        assert!((p1.similarity(&p2) - 1.0).abs() < 0.001);
        assert!((p1.similarity(&p3) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_pattern_merge() {
        let mut p1 = Pattern::new(
            "p1".to_string(),
            vec![1.0, 0.0],
            0.8,
            100,
            "node".to_string(),
        );

        let p2 = Pattern::new(
            "p2".to_string(),
            vec![0.0, 1.0],
            0.9,
            100,
            "node".to_string(),
        );

        p1.merge(&p2);

        // Should be weighted average
        assert_eq!(p1.samples, 200);
        assert!((p1.embedding[0] - 0.5).abs() < 0.001);
        assert!((p1.embedding[1] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_hnsw_index() {
        let mut index = HnswIndex::new(3);

        index.insert(Pattern::new(
            "p1".to_string(),
            vec![1.0, 0.0, 0.0],
            0.9,
            10,
            "node".to_string(),
        ));

        index.insert(Pattern::new(
            "p2".to_string(),
            vec![0.0, 1.0, 0.0],
            0.8,
            10,
            "node".to_string(),
        ));

        assert_eq!(index.len(), 2);

        let results = index.search(&[0.9, 0.1, 0.0], 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "p1"); // Most similar
    }

    #[test]
    fn test_collective_memory_add() {
        let memory = CollectiveMemory::new("node-1");

        let pattern = Pattern::new(
            "test".to_string(),
            vec![1.0, 2.0, 3.0],
            0.9,
            50,
            "node-1".to_string(),
        );

        assert!(memory.add_pattern(pattern));
        assert_eq!(memory.queue_size(), 1);
    }

    #[test]
    fn test_collective_memory_consolidate() {
        let config = CollectiveMemoryConfig {
            quality_threshold: 0.5,
            ..Default::default()
        };
        let memory = CollectiveMemory::with_config("node-1", config);

        // Add patterns
        for i in 0..5 {
            let pattern = Pattern::new(
                format!("pat-{}", i),
                vec![i as f32, 0.0, 0.0],
                0.9,
                10,
                "node-1".to_string(),
            );
            memory.add_pattern(pattern);
        }

        assert_eq!(memory.queue_size(), 5);

        // Consolidate
        let consolidated = memory.consolidate();
        assert!(consolidated > 0);
        assert!(memory.pattern_count() > 0);
    }

    #[test]
    fn test_receive_pattern_from_rac() {
        let memory = CollectiveMemory::new("node-1");

        let event = RacEvent::Assert {
            claim: ClaimType::LearningPattern {
                pattern_id: "test-rac".to_string(),
                embedding: vec![1.0, 2.0, 3.0],
                quality_score: 0.95,
                sample_count: 100,
            },
            evidence: vec![],
            confidence: 0.95,
        };

        let accepted = memory.receive_pattern(&event);
        assert!(accepted);
        assert_eq!(memory.queue_size(), 1);
    }

    #[test]
    fn test_share_pattern() {
        let memory = CollectiveMemory::new("node-1");

        let pattern = Pattern::new(
            "share-test".to_string(),
            vec![1.0, 0.0, 0.0],
            0.95,
            50,
            "node-1".to_string(),
        );

        let event = memory.share_pattern(&pattern);

        match event {
            RacEvent::Assert { claim, confidence, .. } => {
                assert!((confidence - 0.95).abs() < 0.001);
                match claim {
                    ClaimType::LearningPattern { pattern_id, .. } => {
                        assert_eq!(pattern_id, "share-test");
                    }
                    _ => panic!("Wrong claim type"),
                }
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_validate_pattern() {
        let memory = CollectiveMemory::new("node-1");

        // Valid pattern
        let valid = Pattern::new(
            "valid".to_string(),
            vec![1.0, 2.0],
            0.9,
            10,
            "node".to_string(),
        );
        assert!(memory.validate_pattern(&valid));

        // Empty embedding
        let empty = Pattern::new(
            "empty".to_string(),
            vec![],
            0.9,
            10,
            "node".to_string(),
        );
        assert!(!memory.validate_pattern(&empty));

        // Zero samples
        let zero_samples = Pattern::new(
            "zero".to_string(),
            vec![1.0],
            0.9,
            0,
            "node".to_string(),
        );
        assert!(!memory.validate_pattern(&zero_samples));
    }

    #[test]
    fn test_hippocampal_replay() {
        let config = CollectiveMemoryConfig {
            quality_threshold: 0.5,
            hippocampal_replay: true,
            ..Default::default()
        };
        let memory = CollectiveMemory::with_config("node-1", config);

        // Add high-quality patterns
        for i in 0..5 {
            let pattern = Pattern::new(
                format!("hq-{}", i),
                vec![i as f32, 1.0, 2.0],
                0.95, // High quality
                100,
                "node-1".to_string(),
            );
            memory.add_pattern(pattern);
        }

        memory.consolidate();

        // Replay should process high-quality patterns
        let replayed = memory.hippocampal_replay();
        assert!(replayed > 0);
    }

    #[test]
    fn test_import_export() {
        let config = CollectiveMemoryConfig {
            quality_threshold: 0.5,
            ..Default::default()
        };
        let memory1 = CollectiveMemory::with_config("node-1", config.clone());

        // Add and consolidate patterns
        for i in 0..3 {
            memory1.add_pattern(Pattern::new(
                format!("exp-{}", i),
                vec![i as f32, 0.0],
                0.9,
                10,
                "node-1".to_string(),
            ));
        }
        memory1.consolidate();

        // Export
        let json = memory1.export_patterns();
        assert!(!json.is_empty());

        // Import to new memory
        let memory2 = CollectiveMemory::with_config("node-2", config);
        let imported = memory2.import_patterns(&json);
        assert!(imported > 0);
    }
}
