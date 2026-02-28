//! LLM coherence gate for Prime-Radiant.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

use crate::coherence::{CoherenceEnergy, CoherenceEngine};
use crate::execution::ComputeLane;
use crate::governance::PolicyBundle;

use super::config::LlmCoherenceConfig;
use super::error::{Result, RuvLlmIntegrationError};

/// Coherence gate for LLM responses.
///
/// Evaluates LLM outputs against the sheaf graph to detect
/// potential hallucinations and coherence violations.
pub struct LlmCoherenceGate {
    /// Coherence engine (shared reference)
    engine: Arc<CoherenceEngine>,

    /// Policy bundle
    policy: PolicyBundle,

    /// Configuration
    config: LlmCoherenceConfig,
}

impl std::fmt::Debug for LlmCoherenceGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmCoherenceGate")
            .field("policy", &self.policy)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Clone for LlmCoherenceGate {
    fn clone(&self) -> Self {
        Self {
            engine: Arc::clone(&self.engine),
            policy: self.policy.clone(),
            config: self.config.clone(),
        }
    }
}

/// Decision from the LLM coherence gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmGateDecision {
    /// Whether the response is allowed
    pub allowed: bool,

    /// Computed coherence energy
    pub energy: f64,

    /// Assigned compute lane
    pub lane: ComputeLane,

    /// Reason for the decision
    pub reason: LlmGateReason,

    /// Coherence analysis details
    pub analysis: CoherenceAnalysis,

    /// Processing time in microseconds
    pub processing_time_us: u64,

    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Reason for the gate decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmGateReason {
    /// Response is coherent with knowledge graph
    Coherent,

    /// Response energy below threshold
    BelowThreshold {
        /// Computed energy
        energy: f64,
        /// Required threshold
        threshold: f64,
    },

    /// Potential hallucination detected
    HallucinationDetected {
        /// Confidence score
        confidence: f64,
        /// Description of the issue
        description: String,
    },

    /// Semantic inconsistency found
    SemanticInconsistency {
        /// Description of the inconsistency
        description: String,
    },

    /// Citation verification failed
    CitationFailure {
        /// Missing or invalid citations
        citations: Vec<String>,
    },

    /// Escalated to human review
    HumanEscalation {
        /// Reason for escalation
        reason: String,
    },

    /// Response too long
    LengthExceeded {
        /// Actual length
        actual: usize,
        /// Maximum allowed
        maximum: usize,
    },
}

/// Analysis of response coherence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceAnalysis {
    /// Semantic consistency score (0.0-1.0)
    pub semantic_score: f64,

    /// Factual grounding score (0.0-1.0)
    pub factual_score: f64,

    /// Citation validity score (0.0-1.0)
    pub citation_score: f64,

    /// Hallucination probability (0.0-1.0)
    pub hallucination_prob: f64,

    /// Number of nodes affected
    pub affected_nodes: usize,

    /// Maximum residual in affected subgraph
    pub max_residual: f64,

    /// Total energy in affected subgraph
    pub subgraph_energy: f64,
}

impl Default for CoherenceAnalysis {
    fn default() -> Self {
        Self {
            semantic_score: 1.0,
            factual_score: 1.0,
            citation_score: 1.0,
            hallucination_prob: 0.0,
            affected_nodes: 0,
            max_residual: 0.0,
            subgraph_energy: 0.0,
        }
    }
}

/// Coherence check for a response.
#[derive(Debug, Clone)]
pub struct ResponseCoherence {
    /// Response text
    pub response: String,

    /// Context embedding
    pub context_embedding: Vec<f32>,

    /// Response embedding
    pub response_embedding: Vec<f32>,

    /// Related node IDs in the knowledge graph
    pub related_nodes: Vec<crate::NodeId>,

    /// Session ID (if applicable)
    pub session_id: Option<String>,
}

impl LlmCoherenceGate {
    /// Create a new LLM coherence gate.
    /// Create a new LLM coherence gate with an Arc-wrapped engine.
    pub fn new(
        engine: Arc<CoherenceEngine>,
        policy: PolicyBundle,
        config: LlmCoherenceConfig,
    ) -> Result<Self> {
        Ok(Self {
            engine,
            policy,
            config,
        })
    }

    /// Create a new LLM coherence gate, wrapping the engine in an Arc.
    pub fn from_engine(
        engine: CoherenceEngine,
        policy: PolicyBundle,
        config: LlmCoherenceConfig,
    ) -> Result<Self> {
        Self::new(Arc::new(engine), policy, config)
    }

    /// Evaluate a response for coherence.
    pub fn evaluate(&self, response: &ResponseCoherence) -> Result<LlmGateDecision> {
        let start = Instant::now();

        // Check response length
        if response.response.len() > self.config.max_response_length {
            return Ok(self.create_decision(
                false,
                0.0,
                ComputeLane::Human,
                LlmGateReason::LengthExceeded {
                    actual: response.response.len(),
                    maximum: self.config.max_response_length,
                },
                CoherenceAnalysis::default(),
                start.elapsed().as_micros() as u64,
            ));
        }

        // Compute coherence analysis
        let analysis = self.analyze_coherence(response)?;

        // Determine decision based on analysis
        let (allowed, lane, reason) = self.determine_decision(&analysis);

        Ok(self.create_decision(
            allowed,
            analysis.subgraph_energy,
            lane,
            reason,
            analysis,
            start.elapsed().as_micros() as u64,
        ))
    }

    /// Analyze the coherence of a response.
    fn analyze_coherence(&self, response: &ResponseCoherence) -> Result<CoherenceAnalysis> {
        let mut analysis = CoherenceAnalysis::default();

        // Check if we have related nodes to evaluate against
        if response.related_nodes.is_empty() {
            // No related nodes - can't compute coherence, assume coherent
            return Ok(analysis);
        }

        // Compute semantic consistency if enabled
        if self.config.semantic_consistency {
            analysis.semantic_score = self.compute_semantic_score(response);
        }

        // Compute factual grounding if enabled
        if self.config.factual_grounding {
            analysis.factual_score = self.compute_factual_score(response);
        }

        // Compute citation validity if enabled
        if self.config.citation_verification {
            analysis.citation_score = self.compute_citation_score(response);
        }

        // Estimate hallucination probability
        analysis.hallucination_prob = self.estimate_hallucination_prob(&analysis);

        // Compute subgraph metrics
        analysis.affected_nodes = response.related_nodes.len();

        Ok(analysis)
    }

    /// Compute semantic consistency score.
    fn compute_semantic_score(&self, response: &ResponseCoherence) -> f64 {
        // Compute cosine similarity between context and response embeddings
        if response.context_embedding.is_empty() || response.response_embedding.is_empty() {
            return 1.0;
        }

        let dot: f32 = response
            .context_embedding
            .iter()
            .zip(&response.response_embedding)
            .map(|(a, b)| a * b)
            .sum();

        let mag_a: f32 = response
            .context_embedding
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        let mag_b: f32 = response
            .response_embedding
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();

        if mag_a == 0.0 || mag_b == 0.0 {
            return 1.0;
        }

        (dot / (mag_a * mag_b)).max(0.0) as f64
    }

    /// Compute factual grounding score.
    fn compute_factual_score(&self, _response: &ResponseCoherence) -> f64 {
        // Placeholder - would require access to knowledge base
        1.0
    }

    /// Compute citation validity score.
    fn compute_citation_score(&self, _response: &ResponseCoherence) -> f64 {
        // Placeholder - would require citation parsing and verification
        1.0
    }

    /// Estimate hallucination probability.
    fn estimate_hallucination_prob(&self, analysis: &CoherenceAnalysis) -> f64 {
        // Combine scores to estimate hallucination probability
        let combined =
            (analysis.semantic_score + analysis.factual_score + analysis.citation_score) / 3.0;

        // Higher combined score = lower hallucination probability
        (1.0 - combined) * self.config.hallucination_sensitivity
    }

    /// Determine the gate decision based on analysis.
    fn determine_decision(
        &self,
        analysis: &CoherenceAnalysis,
    ) -> (bool, ComputeLane, LlmGateReason) {
        // Check for hallucination
        if analysis.hallucination_prob > self.config.hallucination_sensitivity {
            return (
                false,
                ComputeLane::Human,
                LlmGateReason::HallucinationDetected {
                    confidence: analysis.hallucination_prob,
                    description: "Response may contain hallucinated content".to_string(),
                },
            );
        }

        // Check semantic consistency
        if analysis.semantic_score < self.config.coherence_threshold {
            return (
                false,
                ComputeLane::Heavy,
                LlmGateReason::SemanticInconsistency {
                    description: format!(
                        "Semantic score {:.2} below threshold {:.2}",
                        analysis.semantic_score, self.config.coherence_threshold
                    ),
                },
            );
        }

        // Determine lane based on energy
        let lane = self.determine_lane(analysis.subgraph_energy);

        (true, lane, LlmGateReason::Coherent)
    }

    /// Determine the compute lane based on energy.
    fn determine_lane(&self, energy: f64) -> ComputeLane {
        if energy < self.config.lane_thresholds.reflex {
            ComputeLane::Reflex
        } else if energy < self.config.lane_thresholds.retrieval {
            ComputeLane::Retrieval
        } else if energy < self.config.lane_thresholds.heavy {
            ComputeLane::Heavy
        } else {
            ComputeLane::Human
        }
    }

    /// Create a gate decision.
    fn create_decision(
        &self,
        allowed: bool,
        energy: f64,
        lane: ComputeLane,
        reason: LlmGateReason,
        analysis: CoherenceAnalysis,
        processing_time_us: u64,
    ) -> LlmGateDecision {
        LlmGateDecision {
            allowed,
            energy,
            lane,
            reason,
            analysis,
            processing_time_us,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &LlmCoherenceConfig {
        &self.config
    }

    /// Get the policy bundle.
    pub fn policy(&self) -> &PolicyBundle {
        &self.policy
    }

    /// Get the coherence engine.
    pub fn engine(&self) -> &CoherenceEngine {
        &self.engine
    }
}

impl LlmGateDecision {
    /// Check if the response is allowed.
    pub fn is_allowed(&self) -> bool {
        self.allowed
    }

    /// Check if escalation is required.
    pub fn requires_escalation(&self) -> bool {
        matches!(self.lane, ComputeLane::Human)
    }
}
