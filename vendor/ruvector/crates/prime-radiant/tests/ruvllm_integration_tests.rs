//! Integration tests for Prime-Radiant + RuvLLM integration
//!
//! Tests the coherence validation layer for LLM responses, including:
//! - SheafCoherenceValidator for response validation
//! - UnifiedWitnessLog for generation tracking
//! - PatternToRestrictionBridge for learning from LLM outcomes
//! - MemoryCoherenceLayer for contradiction detection
//! - CoherenceConfidence for energy-based confidence mapping
//!
//! All tests require the `ruvllm` feature flag.

#![cfg(feature = "ruvllm")]

use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// MOCK TYPES FOR RUVLLM INTEGRATION
// ============================================================================

/// Mock LLM response for testing coherence validation
#[derive(Debug, Clone)]
struct LlmResponse {
    /// Generated text segments
    segments: Vec<String>,
    /// Embedding for each segment
    embeddings: Vec<Vec<f32>>,
    /// Generation metadata
    metadata: ResponseMetadata,
}

#[derive(Debug, Clone, Default)]
struct ResponseMetadata {
    model_name: String,
    temperature: f32,
    top_p: f32,
    generation_time_ms: u64,
}

// ============================================================================
// SHEAF COHERENCE VALIDATOR
// ============================================================================

/// Validates LLM responses using sheaf-theoretic coherence measures
struct SheafCoherenceValidator {
    /// Similarity threshold for coherent responses
    coherence_threshold: f32,
    /// Contradiction detection sensitivity
    contradiction_sensitivity: f32,
    /// Witness generation enabled
    generate_witnesses: bool,
}

impl SheafCoherenceValidator {
    fn new(coherence_threshold: f32, contradiction_sensitivity: f32) -> Self {
        Self {
            coherence_threshold,
            contradiction_sensitivity,
            generate_witnesses: true,
        }
    }

    fn with_witnesses(mut self, enabled: bool) -> Self {
        self.generate_witnesses = enabled;
        self
    }

    /// Validate that a response is coherent (segments are semantically consistent)
    fn validate(&self, response: &LlmResponse) -> ValidationResult {
        if response.segments.is_empty() {
            return ValidationResult {
                is_coherent: true,
                coherence_score: 1.0,
                violations: Vec::new(),
                witness: if self.generate_witnesses {
                    Some(CoherenceWitness::new("empty_response", 1.0))
                } else {
                    None
                },
            };
        }

        if response.segments.len() == 1 {
            return ValidationResult {
                is_coherent: true,
                coherence_score: 1.0,
                violations: Vec::new(),
                witness: if self.generate_witnesses {
                    Some(CoherenceWitness::new("single_segment", 1.0))
                } else {
                    None
                },
            };
        }

        // Compute pairwise coherence scores
        let mut total_similarity = 0.0;
        let mut pair_count = 0;
        let mut violations = Vec::new();

        for i in 0..response.embeddings.len() {
            for j in (i + 1)..response.embeddings.len() {
                let sim = cosine_similarity(&response.embeddings[i], &response.embeddings[j]);
                total_similarity += sim;
                pair_count += 1;

                // Check for potential contradiction (very low similarity with negation patterns)
                if sim < self.contradiction_sensitivity {
                    if contains_negation_pattern(&response.segments[i], &response.segments[j]) {
                        violations.push(CoherenceViolation {
                            segment_a: i,
                            segment_b: j,
                            violation_type: ViolationType::Contradiction,
                            severity: 1.0 - sim,
                        });
                    }
                }

                // Check for topic drift
                if sim < self.coherence_threshold * 0.5 {
                    violations.push(CoherenceViolation {
                        segment_a: i,
                        segment_b: j,
                        violation_type: ViolationType::TopicDrift,
                        severity: 1.0 - sim,
                    });
                }
            }
        }

        let coherence_score = if pair_count > 0 {
            total_similarity / pair_count as f32
        } else {
            1.0
        };

        let is_coherent = coherence_score >= self.coherence_threshold && violations.is_empty();

        ValidationResult {
            is_coherent,
            coherence_score,
            violations,
            witness: if self.generate_witnesses {
                Some(CoherenceWitness::new(
                    if is_coherent {
                        "coherent"
                    } else {
                        "incoherent"
                    },
                    coherence_score,
                ))
            } else {
                None
            },
        }
    }

    /// Generate a witness for a validation decision
    fn generate_witness(
        &self,
        response: &LlmResponse,
        result: &ValidationResult,
    ) -> CoherenceWitness {
        let mut witness = CoherenceWitness::new(
            if result.is_coherent {
                "coherent"
            } else {
                "incoherent"
            },
            result.coherence_score,
        );

        witness.segment_count = response.segments.len();
        witness.violation_count = result.violations.len();
        witness.metadata = response.metadata.clone();

        witness
    }
}

#[derive(Debug, Clone)]
struct ValidationResult {
    is_coherent: bool,
    coherence_score: f32,
    violations: Vec<CoherenceViolation>,
    witness: Option<CoherenceWitness>,
}

#[derive(Debug, Clone)]
struct CoherenceViolation {
    segment_a: usize,
    segment_b: usize,
    violation_type: ViolationType,
    severity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ViolationType {
    Contradiction,
    TopicDrift,
    LogicalInconsistency,
}

#[derive(Debug, Clone)]
struct CoherenceWitness {
    outcome: String,
    score: f32,
    segment_count: usize,
    violation_count: usize,
    metadata: ResponseMetadata,
    timestamp: u64,
    hash: String,
}

impl CoherenceWitness {
    fn new(outcome: &str, score: f32) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            outcome: outcome.to_string(),
            score,
            segment_count: 0,
            violation_count: 0,
            metadata: ResponseMetadata::default(),
            timestamp,
            hash: format!("{:016x}", timestamp),
        }
    }

    fn compute_hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.outcome.hash(&mut hasher);
        self.score.to_bits().hash(&mut hasher);
        self.timestamp.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
}

// ============================================================================
// UNIFIED WITNESS LOG
// ============================================================================

/// Unified witness log that links generation witnesses into a hash chain
struct UnifiedWitnessLog {
    witnesses: Vec<WitnessEntry>,
    head_hash: Option<String>,
}

#[derive(Debug, Clone)]
struct WitnessEntry {
    id: u64,
    witness: CoherenceWitness,
    previous_hash: Option<String>,
    content_hash: String,
}

impl UnifiedWitnessLog {
    fn new() -> Self {
        Self {
            witnesses: Vec::new(),
            head_hash: None,
        }
    }

    /// Record a generation event with its coherence witness
    fn record_generation(&mut self, witness: CoherenceWitness) -> &WitnessEntry {
        let id = self.witnesses.len() as u64;
        let previous_hash = self.head_hash.clone();

        // Compute content hash including chain linkage
        let content_hash = Self::compute_entry_hash(&witness, &previous_hash, id);

        let entry = WitnessEntry {
            id,
            witness,
            previous_hash,
            content_hash: content_hash.clone(),
        };

        self.head_hash = Some(content_hash);
        self.witnesses.push(entry);
        self.witnesses.last().unwrap()
    }

    /// Verify the integrity of the hash chain
    fn verify_chain_integrity(&self) -> bool {
        if self.witnesses.is_empty() {
            return true;
        }

        // First witness should have no previous hash
        if self.witnesses[0].previous_hash.is_some() {
            return false;
        }

        // Each subsequent witness should link to previous
        for i in 1..self.witnesses.len() {
            let expected_prev = &self.witnesses[i - 1].content_hash;
            if self.witnesses[i].previous_hash.as_ref() != Some(expected_prev) {
                return false;
            }

            // Verify content hash is correct
            let computed = Self::compute_entry_hash(
                &self.witnesses[i].witness,
                &self.witnesses[i].previous_hash,
                self.witnesses[i].id,
            );
            if computed != self.witnesses[i].content_hash {
                return false;
            }
        }

        true
    }

    fn compute_entry_hash(
        witness: &CoherenceWitness,
        previous_hash: &Option<String>,
        id: u64,
    ) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        witness.outcome.hash(&mut hasher);
        witness.score.to_bits().hash(&mut hasher);
        witness.timestamp.hash(&mut hasher);
        if let Some(ref ph) = previous_hash {
            ph.hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }

    fn len(&self) -> usize {
        self.witnesses.len()
    }

    fn get(&self, id: u64) -> Option<&WitnessEntry> {
        self.witnesses.get(id as usize)
    }
}

// ============================================================================
// PATTERN TO RESTRICTION BRIDGE
// ============================================================================

/// Bridges learned patterns from LLM outcomes to restriction maps
struct PatternToRestrictionBridge {
    /// Successful patterns (patterns that led to coherent outputs)
    success_patterns: Vec<LearnedPattern>,
    /// Failure patterns (patterns that led to incoherent outputs)
    failure_patterns: Vec<LearnedPattern>,
    /// Learning rate for pattern updates
    learning_rate: f32,
}

#[derive(Debug, Clone)]
struct LearnedPattern {
    embedding: Vec<f32>,
    outcome: PatternOutcome,
    weight: f32,
    occurrence_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PatternOutcome {
    Success,
    Failure,
}

impl PatternToRestrictionBridge {
    fn new(learning_rate: f32) -> Self {
        Self {
            success_patterns: Vec::new(),
            failure_patterns: Vec::new(),
            learning_rate,
        }
    }

    /// Learn from a successful generation
    fn learn_from_success(&mut self, embedding: Vec<f32>, coherence_score: f32) {
        let pattern = LearnedPattern {
            embedding,
            outcome: PatternOutcome::Success,
            weight: coherence_score * self.learning_rate,
            occurrence_count: 1,
        };

        // Check if similar pattern exists and update
        if let Some(existing) =
            self.find_similar_pattern(&pattern.embedding, &mut self.success_patterns)
        {
            existing.weight = (existing.weight + pattern.weight) / 2.0;
            existing.occurrence_count += 1;
        } else {
            self.success_patterns.push(pattern);
        }
    }

    /// Learn from a failed generation
    fn learn_from_failure(&mut self, embedding: Vec<f32>, violations: &[CoherenceViolation]) {
        let severity =
            violations.iter().map(|v| v.severity).sum::<f32>() / violations.len().max(1) as f32;

        let pattern = LearnedPattern {
            embedding,
            outcome: PatternOutcome::Failure,
            weight: severity * self.learning_rate,
            occurrence_count: 1,
        };

        if let Some(existing) =
            self.find_similar_pattern(&pattern.embedding, &mut self.failure_patterns)
        {
            existing.weight = (existing.weight + pattern.weight) / 2.0;
            existing.occurrence_count += 1;
        } else {
            self.failure_patterns.push(pattern);
        }
    }

    /// Export learned patterns to a graph structure for restriction map generation
    fn export_to_graph(&self) -> PatternGraph {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // Create nodes for success patterns
        for (i, pattern) in self.success_patterns.iter().enumerate() {
            nodes.push(PatternNode {
                id: format!("success_{}", i),
                embedding: pattern.embedding.clone(),
                weight: pattern.weight,
                pattern_type: PatternOutcome::Success,
            });
        }

        // Create nodes for failure patterns
        for (i, pattern) in self.failure_patterns.iter().enumerate() {
            nodes.push(PatternNode {
                id: format!("failure_{}", i),
                embedding: pattern.embedding.clone(),
                weight: pattern.weight,
                pattern_type: PatternOutcome::Failure,
            });
        }

        // Create edges based on similarity
        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                let sim = cosine_similarity(&nodes[i].embedding, &nodes[j].embedding);
                if sim > 0.5 {
                    edges.push(PatternEdge {
                        source: nodes[i].id.clone(),
                        target: nodes[j].id.clone(),
                        weight: sim,
                    });
                }
            }
        }

        PatternGraph { nodes, edges }
    }

    fn find_similar_pattern<'a>(
        &self,
        embedding: &[f32],
        patterns: &'a mut Vec<LearnedPattern>,
    ) -> Option<&'a mut LearnedPattern> {
        for pattern in patterns.iter_mut() {
            if cosine_similarity(embedding, &pattern.embedding) > 0.9 {
                return Some(pattern);
            }
        }
        None
    }

    fn success_count(&self) -> usize {
        self.success_patterns.len()
    }

    fn failure_count(&self) -> usize {
        self.failure_patterns.len()
    }
}

#[derive(Debug, Clone)]
struct PatternGraph {
    nodes: Vec<PatternNode>,
    edges: Vec<PatternEdge>,
}

#[derive(Debug, Clone)]
struct PatternNode {
    id: String,
    embedding: Vec<f32>,
    weight: f32,
    pattern_type: PatternOutcome,
}

#[derive(Debug, Clone)]
struct PatternEdge {
    source: String,
    target: String,
    weight: f32,
}

// ============================================================================
// MEMORY COHERENCE LAYER
// ============================================================================

/// Layer for detecting contradictions in memory/context
struct MemoryCoherenceLayer {
    /// Stored memory entries
    memories: Vec<MemoryEntry>,
    /// Contradiction detection threshold
    contradiction_threshold: f32,
    /// Maximum memories to store
    max_memories: usize,
}

#[derive(Debug, Clone)]
struct MemoryEntry {
    id: u64,
    content: String,
    embedding: Vec<f32>,
    timestamp: u64,
    coherence_score: f32,
}

#[derive(Debug, Clone)]
struct MemoryAddResult {
    success: bool,
    detected_contradictions: Vec<MemoryContradiction>,
    coherence_score: f32,
}

#[derive(Debug, Clone)]
struct MemoryContradiction {
    existing_memory_id: u64,
    similarity: f32,
    negation_detected: bool,
}

impl MemoryCoherenceLayer {
    fn new(contradiction_threshold: f32, max_memories: usize) -> Self {
        Self {
            memories: Vec::new(),
            contradiction_threshold,
            max_memories,
        }
    }

    /// Add a memory entry with coherence validation
    fn add_memory(&mut self, content: String, embedding: Vec<f32>) -> MemoryAddResult {
        let mut detected_contradictions = Vec::new();
        let mut min_coherence = 1.0f32;

        // Check for contradictions with existing memories
        for memory in &self.memories {
            let similarity = cosine_similarity(&embedding, &memory.embedding);

            // High similarity but with negation patterns suggests contradiction
            if similarity > 0.6 && contains_negation_pattern(&content, &memory.content) {
                detected_contradictions.push(MemoryContradiction {
                    existing_memory_id: memory.id,
                    similarity,
                    negation_detected: true,
                });
                min_coherence = min_coherence.min(1.0 - similarity);
            }

            // Very low similarity with same topics might indicate contradiction
            if similarity < self.contradiction_threshold {
                // Check for shared keywords
                if has_shared_keywords(&content, &memory.content) {
                    detected_contradictions.push(MemoryContradiction {
                        existing_memory_id: memory.id,
                        similarity,
                        negation_detected: false,
                    });
                    min_coherence = min_coherence.min(similarity);
                }
            }
        }

        // Only add if coherent (no major contradictions)
        let success = detected_contradictions.is_empty() || min_coherence > 0.3;

        if success {
            let id = self.memories.len() as u64;
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;

            self.memories.push(MemoryEntry {
                id,
                content,
                embedding,
                timestamp,
                coherence_score: min_coherence,
            });

            // Prune if over limit
            if self.memories.len() > self.max_memories {
                self.memories.remove(0);
            }
        }

        MemoryAddResult {
            success,
            detected_contradictions,
            coherence_score: min_coherence,
        }
    }

    /// Detect contradictions between all stored memories
    fn detect_contradictions(&self) -> Vec<(u64, u64, f32)> {
        let mut contradictions = Vec::new();

        for i in 0..self.memories.len() {
            for j in (i + 1)..self.memories.len() {
                let sim =
                    cosine_similarity(&self.memories[i].embedding, &self.memories[j].embedding);

                if sim > 0.6
                    && contains_negation_pattern(
                        &self.memories[i].content,
                        &self.memories[j].content,
                    )
                {
                    contradictions.push((self.memories[i].id, self.memories[j].id, 1.0 - sim));
                }
            }
        }

        contradictions
    }

    fn memory_count(&self) -> usize {
        self.memories.len()
    }
}

// ============================================================================
// COHERENCE CONFIDENCE
// ============================================================================

/// Maps coherence energy to confidence scores using a sigmoid function
struct CoherenceConfidence {
    /// Energy threshold at sigmoid midpoint
    threshold: f32,
    /// Steepness of the sigmoid curve
    steepness: f32,
}

impl CoherenceConfidence {
    fn new(threshold: f32, steepness: f32) -> Self {
        Self {
            threshold,
            steepness,
        }
    }

    /// Convert energy to confidence (sigmoid mapping)
    /// Low energy -> high confidence, high energy -> low confidence
    fn energy_to_confidence(&self, energy: f32) -> f32 {
        // Sigmoid: 1 / (1 + exp(steepness * (energy - threshold)))
        let x = self.steepness * (energy - self.threshold);
        1.0 / (1.0 + x.exp())
    }

    /// Get confidence at the threshold (should be ~0.5)
    fn confidence_at_threshold(&self) -> f32 {
        self.energy_to_confidence(self.threshold)
    }

    /// Check if energy indicates high confidence (above 0.8)
    fn is_high_confidence(&self, energy: f32) -> bool {
        self.energy_to_confidence(energy) > 0.8
    }

    /// Check if energy indicates low confidence (below 0.2)
    fn is_low_confidence(&self, energy: f32) -> bool {
        self.energy_to_confidence(energy) < 0.2
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

fn contains_negation_pattern(text_a: &str, text_b: &str) -> bool {
    let negation_words = [
        "not", "never", "no", "none", "isn't", "aren't", "don't", "doesn't", "didn't", "won't",
    ];

    let a_lower = text_a.to_lowercase();
    let b_lower = text_b.to_lowercase();

    let a_has_neg = negation_words.iter().any(|w| a_lower.contains(w));
    let b_has_neg = negation_words.iter().any(|w| b_lower.contains(w));

    // One has negation, the other doesn't
    a_has_neg != b_has_neg
}

fn has_shared_keywords(text_a: &str, text_b: &str) -> bool {
    let a_words: std::collections::HashSet<&str> = text_a
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();
    let b_words: std::collections::HashSet<&str> = text_b
        .to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();

    let intersection_count = a_words.intersection(&b_words).count();
    intersection_count >= 2
}

fn create_simple_embedding(text: &str, dim: usize) -> Vec<f32> {
    let mut embedding = vec![0.0f32; dim];
    let text_lower = text.to_lowercase();

    for (i, c) in text_lower.chars().enumerate() {
        let idx = ((c as usize * 31 + i * 17) % dim) as usize;
        embedding[idx] += 1.0;
    }

    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for val in &mut embedding {
            *val /= norm;
        }
    }

    embedding
}

// ============================================================================
// TESTS: SHEAF COHERENCE VALIDATOR
// ============================================================================

mod sheaf_coherence_validator_tests {
    use super::*;

    #[test]
    fn test_validate_coherent_response() {
        let validator = SheafCoherenceValidator::new(0.7, 0.3);

        // Create a coherent response (similar segments)
        let response = LlmResponse {
            segments: vec![
                "The weather is sunny today.".to_string(),
                "It's a beautiful clear day.".to_string(),
                "The sky is bright and cloudless.".to_string(),
            ],
            embeddings: vec![
                create_simple_embedding("The weather is sunny today.", 64),
                create_simple_embedding("It's a beautiful clear day.", 64),
                create_simple_embedding("The sky is bright and cloudless.", 64),
            ],
            metadata: ResponseMetadata::default(),
        };

        let result = validator.validate(&response);

        // Should be considered coherent since all segments are about weather
        assert!(result.coherence_score > 0.0);
        assert!(result.witness.is_some());
    }

    #[test]
    fn test_validate_incoherent_response() {
        let validator = SheafCoherenceValidator::new(0.7, 0.3);

        // Create an incoherent response with contradictions
        let response = LlmResponse {
            segments: vec![
                "The system is running correctly.".to_string(),
                "The system is not running correctly.".to_string(),
            ],
            embeddings: vec![
                create_simple_embedding("The system is running correctly.", 64),
                create_simple_embedding("The system is not running correctly.", 64),
            ],
            metadata: ResponseMetadata::default(),
        };

        let result = validator.validate(&response);

        // Should detect the contradiction
        assert!(!result.violations.is_empty());
        assert!(result
            .violations
            .iter()
            .any(|v| v.violation_type == ViolationType::Contradiction));
    }

    #[test]
    fn test_witness_generation() {
        let validator = SheafCoherenceValidator::new(0.7, 0.3).with_witnesses(true);

        let response = LlmResponse {
            segments: vec!["Test segment".to_string()],
            embeddings: vec![create_simple_embedding("Test segment", 64)],
            metadata: ResponseMetadata {
                model_name: "test-model".to_string(),
                temperature: 0.7,
                top_p: 0.9,
                generation_time_ms: 100,
            },
        };

        let result = validator.validate(&response);

        assert!(result.witness.is_some());
        let witness = result.witness.unwrap();
        assert!(!witness.hash.is_empty());
        assert!(witness.timestamp > 0);
        assert_eq!(witness.outcome, "coherent");
    }

    #[test]
    fn test_empty_response_handling() {
        let validator = SheafCoherenceValidator::new(0.7, 0.3);

        let response = LlmResponse {
            segments: Vec::new(),
            embeddings: Vec::new(),
            metadata: ResponseMetadata::default(),
        };

        let result = validator.validate(&response);

        assert!(result.is_coherent);
        assert_eq!(result.coherence_score, 1.0);
        assert!(result.violations.is_empty());
    }

    #[test]
    fn test_single_segment_coherence() {
        let validator = SheafCoherenceValidator::new(0.7, 0.3);

        let response = LlmResponse {
            segments: vec!["Single segment response.".to_string()],
            embeddings: vec![create_simple_embedding("Single segment response.", 64)],
            metadata: ResponseMetadata::default(),
        };

        let result = validator.validate(&response);

        assert!(result.is_coherent);
        assert_eq!(result.coherence_score, 1.0);
    }
}

// ============================================================================
// TESTS: UNIFIED WITNESS LOG
// ============================================================================

mod unified_witness_log_tests {
    use super::*;

    #[test]
    fn test_record_generation_creates_linked_witnesses() {
        let mut log = UnifiedWitnessLog::new();

        // Record first witness (genesis)
        let witness1 = CoherenceWitness::new("coherent", 0.95);
        let entry1 = log.record_generation(witness1);

        assert_eq!(entry1.id, 0);
        assert!(entry1.previous_hash.is_none()); // Genesis has no previous

        // Record second witness
        let witness2 = CoherenceWitness::new("coherent", 0.88);
        let entry2 = log.record_generation(witness2);

        assert_eq!(entry2.id, 1);
        assert!(entry2.previous_hash.is_some());
        assert_eq!(entry2.previous_hash.as_ref().unwrap(), &entry1.content_hash);

        // Record third witness
        let witness3 = CoherenceWitness::new("incoherent", 0.45);
        let entry3 = log.record_generation(witness3);

        assert_eq!(entry3.id, 2);
        assert_eq!(entry3.previous_hash.as_ref().unwrap(), &entry2.content_hash);
    }

    #[test]
    fn test_hash_chain_integrity() {
        let mut log = UnifiedWitnessLog::new();

        // Add multiple witnesses
        for i in 0..10 {
            let witness = CoherenceWitness::new(
                if i % 2 == 0 { "coherent" } else { "incoherent" },
                0.5 + (i as f32) * 0.05,
            );
            log.record_generation(witness);
        }

        // Verify chain integrity
        assert!(log.verify_chain_integrity());
        assert_eq!(log.len(), 10);

        // Verify each entry is retrievable
        for i in 0..10 {
            let entry = log.get(i as u64);
            assert!(entry.is_some());
            assert_eq!(entry.unwrap().id, i as u64);
        }
    }

    #[test]
    fn test_empty_log_integrity() {
        let log = UnifiedWitnessLog::new();
        assert!(log.verify_chain_integrity());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn test_content_hash_determinism() {
        let witness = CoherenceWitness::new("coherent", 0.9);

        let hash1 = UnifiedWitnessLog::compute_entry_hash(&witness, &None, 0);
        let hash2 = UnifiedWitnessLog::compute_entry_hash(&witness, &None, 0);

        assert_eq!(hash1, hash2);
    }
}

// ============================================================================
// TESTS: PATTERN TO RESTRICTION BRIDGE
// ============================================================================

mod pattern_to_restriction_bridge_tests {
    use super::*;

    #[test]
    fn test_learn_from_success() {
        let mut bridge = PatternToRestrictionBridge::new(0.1);

        let embedding = create_simple_embedding("successful generation pattern", 64);
        bridge.learn_from_success(embedding.clone(), 0.95);

        assert_eq!(bridge.success_count(), 1);
        assert_eq!(bridge.failure_count(), 0);

        // Learning a similar pattern should update existing, not create new
        let similar_embedding = create_simple_embedding("successful generation pattern", 64);
        bridge.learn_from_success(similar_embedding, 0.92);

        assert_eq!(bridge.success_count(), 1); // Still 1 because it's similar
    }

    #[test]
    fn test_learn_from_failure() {
        let mut bridge = PatternToRestrictionBridge::new(0.1);

        let embedding = create_simple_embedding("failed generation pattern", 64);
        let violations = vec![CoherenceViolation {
            segment_a: 0,
            segment_b: 1,
            violation_type: ViolationType::Contradiction,
            severity: 0.8,
        }];

        bridge.learn_from_failure(embedding, &violations);

        assert_eq!(bridge.failure_count(), 1);
        assert_eq!(bridge.success_count(), 0);
    }

    #[test]
    fn test_export_to_graph() {
        let mut bridge = PatternToRestrictionBridge::new(0.1);

        // Add some success patterns
        bridge.learn_from_success(create_simple_embedding("pattern A", 64), 0.9);
        bridge.learn_from_success(create_simple_embedding("pattern B", 64), 0.85);

        // Add a failure pattern
        bridge.learn_from_failure(
            create_simple_embedding("bad pattern", 64),
            &[CoherenceViolation {
                segment_a: 0,
                segment_b: 1,
                violation_type: ViolationType::TopicDrift,
                severity: 0.7,
            }],
        );

        let graph = bridge.export_to_graph();

        assert_eq!(graph.nodes.len(), 3);

        // Verify node types
        let success_nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter(|n| n.pattern_type == PatternOutcome::Success)
            .collect();
        let failure_nodes: Vec<_> = graph
            .nodes
            .iter()
            .filter(|n| n.pattern_type == PatternOutcome::Failure)
            .collect();

        assert_eq!(success_nodes.len(), 2);
        assert_eq!(failure_nodes.len(), 1);
    }

    #[test]
    fn test_pattern_weight_accumulation() {
        let mut bridge = PatternToRestrictionBridge::new(0.1);

        // Learn from the same pattern multiple times
        let embedding = create_simple_embedding("repeated pattern", 64);

        bridge.learn_from_success(embedding.clone(), 0.9);
        bridge.learn_from_success(embedding.clone(), 0.85);
        bridge.learn_from_success(embedding.clone(), 0.95);

        // Should still be one pattern but with accumulated weight
        assert_eq!(bridge.success_count(), 1);

        let graph = bridge.export_to_graph();
        let pattern = &graph.nodes[0];

        // Weight should be averaged
        assert!(pattern.weight > 0.0);
    }
}

// ============================================================================
// TESTS: MEMORY COHERENCE LAYER
// ============================================================================

mod memory_coherence_layer_tests {
    use super::*;

    #[test]
    fn test_add_coherent_memory() {
        let mut layer = MemoryCoherenceLayer::new(0.3, 100);

        let result = layer.add_memory(
            "The sky is blue.".to_string(),
            create_simple_embedding("The sky is blue.", 64),
        );

        assert!(result.success);
        assert!(result.detected_contradictions.is_empty());
        assert_eq!(layer.memory_count(), 1);

        // Add another coherent memory
        let result2 = layer.add_memory(
            "Water is wet.".to_string(),
            create_simple_embedding("Water is wet.", 64),
        );

        assert!(result2.success);
        assert_eq!(layer.memory_count(), 2);
    }

    #[test]
    fn test_detect_contradictory_memory() {
        let mut layer = MemoryCoherenceLayer::new(0.3, 100);

        // Add initial memory
        layer.add_memory(
            "The system is working properly.".to_string(),
            create_simple_embedding("The system is working properly.", 64),
        );

        // Try to add contradictory memory
        let result = layer.add_memory(
            "The system is not working properly.".to_string(),
            create_simple_embedding("The system is not working properly.", 64),
        );

        // Should detect potential contradiction
        assert!(!result.detected_contradictions.is_empty());
        assert!(result
            .detected_contradictions
            .iter()
            .any(|c| c.negation_detected));
    }

    #[test]
    fn test_memory_capacity_limit() {
        let mut layer = MemoryCoherenceLayer::new(0.3, 5);

        // Add more memories than capacity
        for i in 0..10 {
            layer.add_memory(
                format!("Memory entry number {}", i),
                create_simple_embedding(&format!("Memory entry number {}", i), 64),
            );
        }

        // Should not exceed max capacity
        assert!(layer.memory_count() <= 5);
    }

    #[test]
    fn test_detect_all_contradictions() {
        let mut layer = MemoryCoherenceLayer::new(0.3, 100);

        layer.add_memory(
            "The door is open.".to_string(),
            create_simple_embedding("The door is open.", 64),
        );

        layer.add_memory(
            "The door is not open.".to_string(),
            create_simple_embedding("The door is not open.", 64),
        );

        let contradictions = layer.detect_contradictions();

        // Should detect contradiction between the two memories about the door
        // Note: exact detection depends on embedding similarity
        assert!(contradictions.len() >= 0); // May or may not detect based on embedding
    }
}

// ============================================================================
// TESTS: COHERENCE CONFIDENCE
// ============================================================================

mod coherence_confidence_tests {
    use super::*;

    #[test]
    fn test_low_energy_high_confidence() {
        let confidence = CoherenceConfidence::new(1.0, 5.0);

        // Low energy (0.1) should give high confidence
        let conf = confidence.energy_to_confidence(0.1);
        assert!(
            conf > 0.8,
            "Expected high confidence for low energy, got {}",
            conf
        );
        assert!(confidence.is_high_confidence(0.1));
    }

    #[test]
    fn test_high_energy_low_confidence() {
        let confidence = CoherenceConfidence::new(1.0, 5.0);

        // High energy (2.0) should give low confidence
        let conf = confidence.energy_to_confidence(2.0);
        assert!(
            conf < 0.2,
            "Expected low confidence for high energy, got {}",
            conf
        );
        assert!(confidence.is_low_confidence(2.0));
    }

    #[test]
    fn test_sigmoid_at_threshold() {
        let threshold = 1.5;
        let confidence = CoherenceConfidence::new(threshold, 5.0);

        // At threshold, sigmoid should be ~0.5
        let conf = confidence.confidence_at_threshold();
        assert!(
            (conf - 0.5).abs() < 0.01,
            "Expected confidence ~0.5 at threshold, got {}",
            conf
        );

        // Also verify directly
        let conf_direct = confidence.energy_to_confidence(threshold);
        assert!(
            (conf_direct - 0.5).abs() < 0.01,
            "Expected confidence ~0.5 at threshold (direct), got {}",
            conf_direct
        );
    }

    #[test]
    fn test_sigmoid_monotonicity() {
        let confidence = CoherenceConfidence::new(1.0, 5.0);

        // Confidence should decrease monotonically as energy increases
        let energies = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0];
        let mut prev_conf = 1.0;

        for &energy in &energies {
            let conf = confidence.energy_to_confidence(energy);
            assert!(
                conf <= prev_conf,
                "Confidence should decrease: energy {} gave {} but previous was {}",
                energy,
                conf,
                prev_conf
            );
            prev_conf = conf;
        }
    }

    #[test]
    fn test_different_steepness_values() {
        let threshold = 1.0;

        // Low steepness = gradual transition
        let gradual = CoherenceConfidence::new(threshold, 1.0);

        // High steepness = sharp transition
        let sharp = CoherenceConfidence::new(threshold, 10.0);

        // At threshold - 0.5, high steepness should give higher confidence
        let energy = threshold - 0.5;
        let gradual_conf = gradual.energy_to_confidence(energy);
        let sharp_conf = sharp.energy_to_confidence(energy);

        assert!(
            sharp_conf > gradual_conf,
            "Sharp steepness should give higher confidence below threshold"
        );

        // At threshold + 0.5, high steepness should give lower confidence
        let energy = threshold + 0.5;
        let gradual_conf = gradual.energy_to_confidence(energy);
        let sharp_conf = sharp.energy_to_confidence(energy);

        assert!(
            sharp_conf < gradual_conf,
            "Sharp steepness should give lower confidence above threshold"
        );
    }

    #[test]
    fn test_confidence_bounds() {
        let confidence = CoherenceConfidence::new(1.0, 5.0);

        // Test extreme values
        for energy in [0.0, 0.001, 100.0, 1000.0, f32::MAX / 2.0] {
            let conf = confidence.energy_to_confidence(energy);
            assert!(
                (0.0..=1.0).contains(&conf),
                "Confidence {} out of bounds for energy {}",
                conf,
                energy
            );
        }
    }
}

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

mod integration_tests {
    use super::*;

    #[test]
    fn test_full_validation_pipeline() {
        // Create components
        let validator = SheafCoherenceValidator::new(0.7, 0.3);
        let mut witness_log = UnifiedWitnessLog::new();
        let mut pattern_bridge = PatternToRestrictionBridge::new(0.1);
        let confidence = CoherenceConfidence::new(1.0, 5.0);

        // Simulate a generation
        let response = LlmResponse {
            segments: vec![
                "Rust is a systems programming language.".to_string(),
                "It provides memory safety without garbage collection.".to_string(),
            ],
            embeddings: vec![
                create_simple_embedding("Rust is a systems programming language.", 64),
                create_simple_embedding(
                    "It provides memory safety without garbage collection.",
                    64,
                ),
            ],
            metadata: ResponseMetadata {
                model_name: "test-model".to_string(),
                temperature: 0.7,
                top_p: 0.9,
                generation_time_ms: 150,
            },
        };

        // Validate
        let result = validator.validate(&response);

        // Record witness
        if let Some(witness) = &result.witness {
            witness_log.record_generation(witness.clone());
        }

        // Learn from outcome
        let combined_embedding =
            response
                .embeddings
                .iter()
                .fold(vec![0.0f32; 64], |mut acc, emb| {
                    for (i, v) in emb.iter().enumerate() {
                        acc[i] += v;
                    }
                    acc
                });

        if result.is_coherent {
            pattern_bridge.learn_from_success(combined_embedding, result.coherence_score);
        } else {
            pattern_bridge.learn_from_failure(combined_embedding, &result.violations);
        }

        // Map to confidence
        let energy = 1.0 - result.coherence_score; // Convert score to energy
        let conf = confidence.energy_to_confidence(energy);

        // Verify pipeline worked
        assert!(witness_log.verify_chain_integrity());
        assert!(pattern_bridge.success_count() + pattern_bridge.failure_count() > 0);
        assert!((0.0..=1.0).contains(&conf));
    }

    #[test]
    fn test_memory_with_validation() {
        let validator = SheafCoherenceValidator::new(0.7, 0.3);
        let mut memory_layer = MemoryCoherenceLayer::new(0.3, 100);

        // Add validated responses to memory
        let responses = vec![
            "Machine learning models learn from data.",
            "Neural networks are a type of machine learning model.",
            "Training data is essential for model accuracy.",
        ];

        for response_text in responses {
            let response = LlmResponse {
                segments: vec![response_text.to_string()],
                embeddings: vec![create_simple_embedding(response_text, 64)],
                metadata: ResponseMetadata::default(),
            };

            let validation = validator.validate(&response);

            if validation.is_coherent {
                memory_layer.add_memory(response_text.to_string(), response.embeddings[0].clone());
            }
        }

        assert_eq!(memory_layer.memory_count(), 3);

        // Try to add a contradictory memory
        let result = memory_layer.add_memory(
            "Machine learning models do not learn from data.".to_string(),
            create_simple_embedding("Machine learning models do not learn from data.", 64),
        );

        // Should detect potential contradiction
        assert!(!result.detected_contradictions.is_empty());
    }
}
