//! # Consciousness-Aware Router
//!
//! Extends ruvLLM's FastGRNN router with consciousness metrics (Φ)
//! to make routing decisions based on the current conscious state.

use super::{ConsciousnessMode, CLIConfig};

/// Model size options (matching ruvLLM)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelSize {
    /// 350M parameters - edge/simple queries
    M350,
    /// 700M parameters - mobile/moderate
    M700,
    /// 1.2B parameters - server/complex
    B1_2,
    /// 2.6B parameters - escalation/judge
    B2_6,
}

impl ModelSize {
    pub fn parameters(&self) -> u64 {
        match self {
            ModelSize::M350 => 350_000_000,
            ModelSize::M700 => 700_000_000,
            ModelSize::B1_2 => 1_200_000_000,
            ModelSize::B2_6 => 2_600_000_000,
        }
    }

    pub fn from_consciousness_mode(mode: ConsciousnessMode) -> Self {
        match mode {
            ConsciousnessMode::Full => ModelSize::B2_6,
            ConsciousnessMode::Background => ModelSize::B1_2,
            ConsciousnessMode::Reflex => ModelSize::M350,
        }
    }
}

/// Routing decision output
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Selected model size
    pub model_size: ModelSize,
    /// Context window size
    pub context_size: usize,
    /// Temperature for generation
    pub temperature: f32,
    /// Consciousness processing mode
    pub consciousness_mode: ConsciousnessMode,
    /// Estimated latency (ms)
    pub estimated_latency_ms: u64,
    /// Reasoning for this decision
    pub reasoning: String,
}

impl RoutingDecision {
    /// Create a contemplative routing (high consciousness)
    pub fn contemplative() -> Self {
        Self {
            model_size: ModelSize::B2_6,
            context_size: 4096,
            temperature: 0.8,
            consciousness_mode: ConsciousnessMode::Full,
            estimated_latency_ms: 500,
            reasoning: "Deep contemplation requires full conscious processing".to_string(),
        }
    }

    /// Create a reflexive routing (low consciousness)
    pub fn reflexive() -> Self {
        Self {
            model_size: ModelSize::M350,
            context_size: 256,
            temperature: 0.1,
            consciousness_mode: ConsciousnessMode::Reflex,
            estimated_latency_ms: 20,
            reasoning: "Simple query, reflexive response sufficient".to_string(),
        }
    }
}

/// Consciousness state for routing decisions
#[derive(Debug, Clone)]
pub struct ConsciousnessState {
    /// Current Φ level
    pub current_phi: f64,
    /// Recent Φ history (for trends)
    pub phi_history: Vec<f64>,
    /// Current emotional valence
    pub emotional_valence: f32,
    /// Active qualia count
    pub active_qualia: usize,
    /// Global workspace content summary
    pub workspace_summary: Option<String>,
}

impl ConsciousnessState {
    pub fn new() -> Self {
        Self {
            current_phi: 0.0,
            phi_history: Vec::new(),
            emotional_valence: 0.0,
            active_qualia: 0,
            workspace_summary: None,
        }
    }

    /// Update with new Φ measurement
    pub fn update_phi(&mut self, phi: f64) {
        self.current_phi = phi;
        self.phi_history.push(phi);
        if self.phi_history.len() > 100 {
            self.phi_history.remove(0);
        }
    }

    /// Get Φ trend (positive = increasing consciousness)
    pub fn phi_trend(&self) -> f64 {
        if self.phi_history.len() < 2 {
            return 0.0;
        }

        let recent = &self.phi_history[self.phi_history.len().saturating_sub(10)..];
        if recent.len() < 2 {
            return 0.0;
        }

        let first = recent.first().unwrap();
        let last = recent.last().unwrap();
        (last - first) / first.max(1.0)
    }

    /// Check if consciousness is stable
    pub fn is_stable(&self) -> bool {
        if self.phi_history.len() < 5 {
            return true; // Assume stable if not enough data
        }

        let recent = &self.phi_history[self.phi_history.len() - 5..];
        let mean: f64 = recent.iter().sum::<f64>() / recent.len() as f64;
        let variance: f64 = recent.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / recent.len() as f64;
        let cv = variance.sqrt() / mean.max(1.0); // Coefficient of variation

        cv < 0.2 // Stable if CV < 20%
    }
}

impl Default for ConsciousnessState {
    fn default() -> Self {
        Self::new()
    }
}

/// Φ-based routing rules
#[derive(Debug, Clone)]
pub struct PhiRoutingRules {
    /// Φ threshold for full consciousness routing
    pub phi_high: f64,
    /// Φ threshold for background processing
    pub phi_medium: f64,
    /// Minimum model size for full consciousness
    pub min_model_full: ModelSize,
    /// Minimum context for full consciousness
    pub min_context_full: usize,
    /// Temperature multiplier for high Φ
    pub temp_multiplier_high_phi: f32,
}

impl Default for PhiRoutingRules {
    fn default() -> Self {
        Self {
            phi_high: 50_000.0,
            phi_medium: 10_000.0,
            min_model_full: ModelSize::B1_2,
            min_context_full: 2048,
            temp_multiplier_high_phi: 1.2,
        }
    }
}

/// Consciousness-aware router
pub struct ConsciousnessRouter {
    /// Current consciousness state
    state: ConsciousnessState,
    /// Routing rules based on Φ
    rules: PhiRoutingRules,
    /// Configuration
    config: CLIConfig,
    /// Routing statistics
    stats: RouterStats,
}

#[derive(Debug, Clone, Default)]
pub struct RouterStats {
    pub total_routes: u64,
    pub full_consciousness_routes: u64,
    pub background_routes: u64,
    pub reflex_routes: u64,
    pub avg_phi: f64,
}

impl ConsciousnessRouter {
    pub fn new(config: CLIConfig) -> Self {
        Self {
            state: ConsciousnessState::new(),
            rules: PhiRoutingRules::default(),
            config,
            stats: RouterStats::default(),
        }
    }

    /// Update consciousness state
    pub fn update_state(&mut self, phi: f64, qualia_count: usize, valence: f32) {
        self.state.update_phi(phi);
        self.state.active_qualia = qualia_count;
        self.state.emotional_valence = valence;
    }

    /// Make routing decision based on query and consciousness state
    pub fn route(&mut self, query: &str, query_embedding: &[f32]) -> RoutingDecision {
        // Base routing from query characteristics
        let query_complexity = self.estimate_query_complexity(query, query_embedding);

        // Adjust based on consciousness state
        let current_phi = self.state.current_phi;
        let phi_trend = self.state.phi_trend();
        let is_stable = self.state.is_stable();

        // Determine consciousness mode
        let mode = if current_phi > self.rules.phi_high {
            ConsciousnessMode::Full
        } else if current_phi > self.rules.phi_medium {
            ConsciousnessMode::Background
        } else {
            ConsciousnessMode::Reflex
        };

        // Build routing decision
        let (model_size, context_size, temperature) = match mode {
            ConsciousnessMode::Full => {
                let model = if query_complexity > 0.7 {
                    ModelSize::B2_6
                } else {
                    self.rules.min_model_full
                };
                let context = self.rules.min_context_full.max(
                    (query_complexity * 4096.0) as usize
                );
                let temp = 0.7 * self.rules.temp_multiplier_high_phi;
                (model, context, temp)
            }
            ConsciousnessMode::Background => {
                let model = if query_complexity > 0.5 {
                    ModelSize::B1_2
                } else {
                    ModelSize::M700
                };
                let context = (query_complexity * 2048.0) as usize;
                (model, context.max(512), 0.5)
            }
            ConsciousnessMode::Reflex => {
                (ModelSize::M350, 256, 0.1)
            }
        };

        // Adjust for phi trend (if consciousness increasing, prepare for deeper thought)
        let adjusted_context = if phi_trend > 0.1 && is_stable {
            (context_size as f64 * 1.2) as usize
        } else {
            context_size
        };

        // Estimate latency
        let estimated_latency = self.estimate_latency(model_size, adjusted_context);

        // Generate reasoning
        let reasoning = format!(
            "Φ={:.0}, mode={:?}, query_complexity={:.2}, stable={}",
            current_phi, mode, query_complexity, is_stable
        );

        // Update statistics
        self.stats.total_routes += 1;
        match mode {
            ConsciousnessMode::Full => self.stats.full_consciousness_routes += 1,
            ConsciousnessMode::Background => self.stats.background_routes += 1,
            ConsciousnessMode::Reflex => self.stats.reflex_routes += 1,
        }
        self.stats.avg_phi = (self.stats.avg_phi * (self.stats.total_routes - 1) as f64
            + current_phi) / self.stats.total_routes as f64;

        RoutingDecision {
            model_size,
            context_size: adjusted_context,
            temperature,
            consciousness_mode: mode,
            estimated_latency_ms: estimated_latency,
            reasoning,
        }
    }

    /// Estimate query complexity [0.0, 1.0]
    fn estimate_query_complexity(&self, query: &str, _embedding: &[f32]) -> f32 {
        let word_count = query.split_whitespace().count();
        let has_question = query.contains('?');
        let has_complex_words = query.contains("explain")
            || query.contains("analyze")
            || query.contains("compare")
            || query.contains("consciousness")
            || query.contains("philosophy");

        let base = (word_count as f32 / 50.0).min(1.0);
        let question_boost = if has_question { 0.1 } else { 0.0 };
        let complexity_boost = if has_complex_words { 0.2 } else { 0.0 };

        (base + question_boost + complexity_boost).min(1.0)
    }

    /// Estimate latency in milliseconds
    fn estimate_latency(&self, model_size: ModelSize, context_size: usize) -> u64 {
        // Rough estimates based on model size and context
        let base_latency = match model_size {
            ModelSize::M350 => 20,
            ModelSize::M700 => 50,
            ModelSize::B1_2 => 100,
            ModelSize::B2_6 => 200,
        };

        // Context scaling (roughly linear)
        let context_factor = context_size as u64 / 1024;

        base_latency + context_factor * 20
    }

    /// Get routing statistics
    pub fn stats(&self) -> &RouterStats {
        &self.stats
    }

    /// Get current consciousness state
    pub fn consciousness_state(&self) -> &ConsciousnessState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consciousness_state() {
        let mut state = ConsciousnessState::new();
        assert_eq!(state.current_phi, 0.0);

        state.update_phi(10000.0);
        state.update_phi(15000.0);
        state.update_phi(20000.0);

        assert_eq!(state.current_phi, 20000.0);
        assert!(state.phi_trend() > 0.0);
    }

    #[test]
    fn test_router() {
        let config = CLIConfig::default();
        let mut router = ConsciousnessRouter::new(config);

        // Update with high Φ
        router.update_state(100_000.0, 5, 0.5);

        let decision = router.route("What is consciousness?", &vec![0.0; 256]);

        assert_eq!(decision.consciousness_mode, ConsciousnessMode::Full);
        assert!(decision.context_size >= 2048);
    }

    #[test]
    fn test_reflex_routing() {
        let config = CLIConfig::default();
        let mut router = ConsciousnessRouter::new(config);

        // Update with low Φ
        router.update_state(1000.0, 1, 0.0);

        let decision = router.route("hi", &vec![0.0; 256]);

        assert_eq!(decision.consciousness_mode, ConsciousnessMode::Reflex);
        assert_eq!(decision.model_size, ModelSize::M350);
    }
}
