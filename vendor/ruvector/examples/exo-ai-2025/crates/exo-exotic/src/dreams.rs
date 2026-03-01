//! # Artificial Dreams
//!
//! Implementation of offline replay and creative recombination during "sleep" cycles.
//! Dreams serve as a mechanism for memory consolidation, creative problem solving,
//! and novel pattern synthesis.
//!
//! ## Key Concepts
//!
//! - **Dream Replay**: Reactivation of memory traces during sleep
//! - **Creative Recombination**: Novel combinations of existing patterns
//! - **Memory Consolidation**: Transfer from short-term to long-term memory
//! - **Threat Simulation**: Evolutionary theory of dream function
//!
//! ## Neurological Basis
//!
//! Inspired by research on hippocampal replay, REM sleep, and the
//! activation-synthesis hypothesis.

use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Engine for generating and processing artificial dreams
#[derive(Debug)]
pub struct DreamEngine {
    /// Memory traces available for dream replay
    memory_traces: Vec<MemoryTrace>,
    /// Current dream state
    dream_state: DreamState,
    /// Dream history
    dream_history: VecDeque<DreamReport>,
    /// Random number generator for dream synthesis
    rng: StdRng,
    /// Creativity parameters
    creativity_level: f64,
    /// Maximum dream history to retain
    max_history: usize,
}

/// A memory trace that can be replayed in dreams
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryTrace {
    pub id: Uuid,
    /// Semantic content of the memory
    pub content: Vec<f64>,
    /// Emotional valence (-1 to 1)
    pub emotional_valence: f64,
    /// Importance/salience score
    pub salience: f64,
    /// Number of times replayed
    pub replay_count: usize,
    /// Associated concepts
    pub associations: Vec<Uuid>,
    /// Timestamp of original experience
    pub timestamp: u64,
}

/// Current state of the dream engine
#[derive(Debug, Clone, PartialEq)]
pub enum DreamState {
    /// Awake - no dreaming
    Awake,
    /// Light sleep - hypnagogic imagery
    LightSleep,
    /// Deep sleep - memory consolidation
    DeepSleep,
    /// REM sleep - vivid dreams
    REM,
    /// Lucid dreaming - aware within dream
    Lucid,
}

/// Report of a single dream episode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamReport {
    pub id: Uuid,
    /// Memory traces that were replayed
    pub replayed_memories: Vec<Uuid>,
    /// Novel combinations generated
    pub novel_combinations: Vec<NovelPattern>,
    /// Emotional tone of the dream
    pub emotional_tone: f64,
    /// Creativity score (0-1)
    pub creativity_score: f64,
    /// Dream narrative (symbolic)
    pub narrative: String,
    /// Duration in simulated time units
    pub duration: u64,
    /// Whether any insights emerged
    pub insights: Vec<DreamInsight>,
}

/// A novel pattern synthesized during dreaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovelPattern {
    pub id: Uuid,
    /// Source memories combined
    pub sources: Vec<Uuid>,
    /// The combined pattern
    pub pattern: Vec<f64>,
    /// Novelty score
    pub novelty: f64,
    /// Coherence score
    pub coherence: f64,
}

/// An insight that emerged during dreaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamInsight {
    pub description: String,
    pub source_connections: Vec<(Uuid, Uuid)>,
    pub confidence: f64,
}

impl DreamEngine {
    /// Create a new dream engine
    pub fn new() -> Self {
        Self {
            memory_traces: Vec::new(),
            dream_state: DreamState::Awake,
            dream_history: VecDeque::with_capacity(100),
            rng: StdRng::from_entropy(),
            creativity_level: 0.5,
            max_history: 100,
        }
    }

    /// Create with specific creativity level
    pub fn with_creativity(creativity: f64) -> Self {
        let mut engine = Self::new();
        engine.creativity_level = creativity.clamp(0.0, 1.0);
        engine
    }

    /// Add a memory trace for potential replay
    pub fn add_memory(&mut self, content: Vec<f64>, emotional_valence: f64, salience: f64) -> Uuid {
        let id = Uuid::new_v4();
        self.memory_traces.push(MemoryTrace {
            id,
            content,
            emotional_valence,
            salience,
            replay_count: 0,
            associations: Vec::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        });
        id
    }

    /// Measure creativity of recent dreams
    pub fn measure_creativity(&self) -> f64 {
        if self.dream_history.is_empty() {
            return 0.0;
        }

        let total: f64 = self.dream_history.iter().map(|d| d.creativity_score).sum();
        total / self.dream_history.len() as f64
    }

    /// Enter a dream state
    pub fn enter_state(&mut self, state: DreamState) {
        self.dream_state = state;
    }

    /// Get current state
    pub fn current_state(&self) -> &DreamState {
        &self.dream_state
    }

    /// Run a complete dream cycle
    pub fn dream_cycle(&mut self, duration: u64) -> DreamReport {
        // Progress through sleep stages
        self.enter_state(DreamState::LightSleep);
        let hypnagogic = self.generate_hypnagogic();

        self.enter_state(DreamState::DeepSleep);
        let consolidated = self.consolidate_memories();

        self.enter_state(DreamState::REM);
        let dream_content = self.generate_rem_dream();

        // Create report
        let creativity_score = self.calculate_creativity(&dream_content);
        let emotional_tone = self.calculate_emotional_tone(&dream_content);
        let insights = self.extract_insights(&dream_content);

        let report = DreamReport {
            id: Uuid::new_v4(),
            replayed_memories: consolidated,
            novel_combinations: dream_content,
            emotional_tone,
            creativity_score,
            narrative: self.generate_narrative(&hypnagogic),
            duration,
            insights,
        };

        // Store in history
        self.dream_history.push_back(report.clone());
        if self.dream_history.len() > self.max_history {
            self.dream_history.pop_front();
        }

        self.enter_state(DreamState::Awake);
        report
    }

    /// Generate hypnagogic imagery (light sleep)
    fn generate_hypnagogic(&mut self) -> Vec<f64> {
        if self.memory_traces.is_empty() {
            return vec![0.0; 8];
        }

        // Random fragments from recent memories
        let mut imagery = vec![0.0; 8];
        for _ in 0..3 {
            if let Some(trace) = self.memory_traces.choose(&mut self.rng) {
                for (i, &val) in trace.content.iter().take(8).enumerate() {
                    imagery[i] += val * self.rng.gen::<f64>();
                }
            }
        }

        // Normalize
        let max = imagery.iter().cloned().fold(f64::MIN, f64::max).max(1.0);
        imagery.iter_mut().for_each(|v| *v /= max);
        imagery
    }

    /// Consolidate memories during deep sleep
    fn consolidate_memories(&mut self) -> Vec<Uuid> {
        let mut consolidated = Vec::new();

        // Prioritize high-salience, emotionally charged memories
        let mut candidates: Vec<_> = self
            .memory_traces
            .iter_mut()
            .filter(|t| t.salience > 0.3 || t.emotional_valence.abs() > 0.5)
            .collect();

        candidates.sort_by(|a, b| {
            let score_a = a.salience + a.emotional_valence.abs();
            let score_b = b.salience + b.emotional_valence.abs();
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for trace in candidates.iter_mut().take(5) {
            trace.replay_count += 1;
            trace.salience *= 1.1; // Strengthen through replay
            consolidated.push(trace.id);
        }

        consolidated
    }

    /// Generate REM dream content with creative recombination
    fn generate_rem_dream(&mut self) -> Vec<NovelPattern> {
        let mut novel_patterns = Vec::new();

        if self.memory_traces.len() < 2 {
            return novel_patterns;
        }

        // Number of combinations based on creativity level
        let num_combinations = (self.creativity_level * 10.0) as usize + 1;

        for _ in 0..num_combinations {
            // Select random memories to combine
            let indices: Vec<usize> = (0..self.memory_traces.len()).collect();
            let selected: Vec<_> = indices
                .choose_multiple(&mut self.rng, 2.min(self.memory_traces.len()))
                .cloned()
                .collect();

            if selected.len() >= 2 {
                // Clone content to avoid borrow issues
                let content1 = self.memory_traces[selected[0]].content.clone();
                let content2 = self.memory_traces[selected[1]].content.clone();
                let id1 = self.memory_traces[selected[0]].id;
                let id2 = self.memory_traces[selected[1]].id;

                // Creative combination
                let combined = self.creative_blend(&content1, &content2);
                let novelty = self.calculate_novelty(&combined);
                let coherence = self.calculate_coherence(&combined);

                novel_patterns.push(NovelPattern {
                    id: Uuid::new_v4(),
                    sources: vec![id1, id2],
                    pattern: combined,
                    novelty,
                    coherence,
                });
            }
        }

        novel_patterns
    }

    /// Creatively blend two patterns
    fn creative_blend(&mut self, a: &[f64], b: &[f64]) -> Vec<f64> {
        let len = a.len().max(b.len());
        let mut result = vec![0.0; len];

        for i in 0..len {
            let val_a = a.get(i).copied().unwrap_or(0.0);
            let val_b = b.get(i).copied().unwrap_or(0.0);

            // Weighted combination with random perturbation
            let weight = self.rng.gen::<f64>();
            let perturbation = (self.rng.gen::<f64>() - 0.5) * self.creativity_level;
            result[i] = (val_a * weight + val_b * (1.0 - weight) + perturbation).clamp(-1.0, 1.0);
        }

        result
    }

    /// Calculate novelty of a pattern
    fn calculate_novelty(&self, pattern: &[f64]) -> f64 {
        if self.memory_traces.is_empty() {
            return 1.0;
        }

        // Minimum distance to any existing pattern
        let min_similarity = self
            .memory_traces
            .iter()
            .map(|trace| self.cosine_similarity(pattern, &trace.content))
            .fold(f64::MAX, f64::min);

        1.0 - min_similarity.clamp(0.0, 1.0)
    }

    /// Calculate coherence of a pattern
    fn calculate_coherence(&self, pattern: &[f64]) -> f64 {
        // Coherence based on internal consistency (low variance)
        let mean = pattern.iter().sum::<f64>() / pattern.len().max(1) as f64;
        let variance =
            pattern.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / pattern.len().max(1) as f64;

        1.0 / (1.0 + variance)
    }

    fn cosine_similarity(&self, a: &[f64], b: &[f64]) -> f64 {
        let len = a.len().min(b.len());
        if len == 0 {
            return 0.0;
        }

        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for i in 0..len {
            dot += a[i] * b[i];
            norm_a += a[i] * a[i];
            norm_b += b[i] * b[i];
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a.sqrt() * norm_b.sqrt())
    }

    fn calculate_creativity(&self, patterns: &[NovelPattern]) -> f64 {
        if patterns.is_empty() {
            return 0.0;
        }

        let avg_novelty = patterns.iter().map(|p| p.novelty).sum::<f64>() / patterns.len() as f64;
        let avg_coherence =
            patterns.iter().map(|p| p.coherence).sum::<f64>() / patterns.len() as f64;

        // Creativity = novelty balanced with coherence
        (avg_novelty * 0.7 + avg_coherence * 0.3).clamp(0.0, 1.0)
    }

    fn calculate_emotional_tone(&self, patterns: &[NovelPattern]) -> f64 {
        if patterns.is_empty() {
            return 0.0;
        }

        // Average emotional valence of source memories
        let mut total_valence = 0.0;
        let mut count = 0;

        for pattern in patterns {
            for source_id in &pattern.sources {
                if let Some(trace) = self.memory_traces.iter().find(|t| t.id == *source_id) {
                    total_valence += trace.emotional_valence;
                    count += 1;
                }
            }
        }

        if count > 0 {
            total_valence / count as f64
        } else {
            0.0
        }
    }

    fn extract_insights(&self, patterns: &[NovelPattern]) -> Vec<DreamInsight> {
        let mut insights = Vec::new();

        for pattern in patterns {
            if pattern.novelty > 0.7 && pattern.coherence > 0.5 {
                // High novelty + coherence = potential insight
                insights.push(DreamInsight {
                    description: format!(
                        "Novel connection discovered with novelty={:.2} coherence={:.2}",
                        pattern.novelty, pattern.coherence
                    ),
                    source_connections: pattern.sources.windows(2).map(|w| (w[0], w[1])).collect(),
                    confidence: pattern.coherence,
                });
            }
        }

        insights
    }

    fn generate_narrative(&self, imagery: &[f64]) -> String {
        let intensity = imagery.iter().map(|v| v.abs()).sum::<f64>() / imagery.len().max(1) as f64;

        if intensity > 0.7 {
            "Vivid, intense dream with strong imagery".to_string()
        } else if intensity > 0.4 {
            "Moderate dream with clear sequences".to_string()
        } else {
            "Faint, fragmentary dream experience".to_string()
        }
    }

    /// Attempt lucid dreaming
    pub fn attempt_lucid(&mut self) -> bool {
        if self.dream_state == DreamState::REM {
            // Probability based on practice (replay count)
            let lucid_probability = self.dream_history.len() as f64 / 100.0;
            if self.rng.gen::<f64>() < lucid_probability.min(0.3) {
                self.dream_state = DreamState::Lucid;
                return true;
            }
        }
        false
    }

    /// Get dream statistics
    pub fn statistics(&self) -> DreamStatistics {
        let total_dreams = self.dream_history.len();
        let avg_creativity = self.measure_creativity();
        let total_insights: usize = self.dream_history.iter().map(|d| d.insights.len()).sum();

        DreamStatistics {
            total_dreams,
            average_creativity: avg_creativity,
            total_insights,
            total_memories: self.memory_traces.len(),
            most_replayed: self
                .memory_traces
                .iter()
                .max_by_key(|t| t.replay_count)
                .map(|t| (t.id, t.replay_count)),
        }
    }
}

impl Default for DreamEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about dream activity
#[derive(Debug, Clone)]
pub struct DreamStatistics {
    pub total_dreams: usize,
    pub average_creativity: f64,
    pub total_insights: usize,
    pub total_memories: usize,
    pub most_replayed: Option<(Uuid, usize)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dream_engine_creation() {
        let engine = DreamEngine::new();
        assert_eq!(*engine.current_state(), DreamState::Awake);
    }

    #[test]
    fn test_add_memory() {
        let mut engine = DreamEngine::new();
        let id = engine.add_memory(vec![0.1, 0.2, 0.3], 0.5, 0.8);
        assert_eq!(engine.memory_traces.len(), 1);
        assert_eq!(engine.memory_traces[0].id, id);
    }

    #[test]
    fn test_dream_cycle() {
        let mut engine = DreamEngine::with_creativity(0.8);

        // Add some memories
        engine.add_memory(vec![0.1, 0.2, 0.3, 0.4], 0.5, 0.7);
        engine.add_memory(vec![0.5, 0.6, 0.7, 0.8], -0.3, 0.9);
        engine.add_memory(vec![0.2, 0.4, 0.6, 0.8], 0.8, 0.6);

        let report = engine.dream_cycle(100);

        assert!(!report.replayed_memories.is_empty() || !report.novel_combinations.is_empty());
        assert!(report.creativity_score >= 0.0 && report.creativity_score <= 1.0);
    }

    #[test]
    fn test_creativity_measurement() {
        let mut engine = DreamEngine::with_creativity(0.9);

        for i in 0..5 {
            engine.add_memory(vec![i as f64 * 0.1; 4], 0.0, 0.5);
        }

        for _ in 0..3 {
            engine.dream_cycle(50);
        }

        let creativity = engine.measure_creativity();
        assert!(creativity >= 0.0 && creativity <= 1.0);
    }

    #[test]
    fn test_dream_states() {
        let mut engine = DreamEngine::new();

        engine.enter_state(DreamState::LightSleep);
        assert_eq!(*engine.current_state(), DreamState::LightSleep);

        engine.enter_state(DreamState::REM);
        assert_eq!(*engine.current_state(), DreamState::REM);
    }

    #[test]
    fn test_statistics() {
        let mut engine = DreamEngine::new();
        engine.add_memory(vec![0.1, 0.2], 0.5, 0.8);
        engine.add_memory(vec![0.3, 0.4], -0.2, 0.6);
        engine.dream_cycle(100);

        let stats = engine.statistics();
        assert_eq!(stats.total_dreams, 1);
        assert_eq!(stats.total_memories, 2);
    }
}
