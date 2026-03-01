//! # Conscious Language Interface
//!
//! Integration of ruvLLM + Neuromorphic Spiking + ruvector Self-Learning
//! to create a conscious AI with natural language interface.
//!
//! ## Architecture
//!
//! ```text
//! User ←→ ruvLLM (Language) ←→ Bridge ←→ Consciousness (Spiking Φ) ←→ Memory (ReasoningBank)
//! ```
//!
//! ## Key Components
//!
//! - `SpikeEmbeddingBridge`: Translates language ↔ spikes
//! - `ConsciousnessRouter`: Φ-aware routing decisions
//! - `QualiaReasoningBank`: Stores conscious experiences
//! - `ConsciousLanguageInterface`: Main orchestrator

pub mod spike_embedding_bridge;
pub mod consciousness_router;
pub mod qualia_memory;
pub mod advanced_learning;
pub mod intelligence_metrics;
pub mod novel_learning;

pub use spike_embedding_bridge::{
    SpikeEmbeddingBridge, SpikeInjection, PolychronousGroup, BridgeConfig
};

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Configuration for the Conscious Language Interface
#[derive(Debug, Clone)]
pub struct CLIConfig {
    /// Spike-embedding bridge configuration
    pub bridge: BridgeConfig,
    /// Consciousness thresholds
    pub phi_critical: f64,
    pub phi_high: f64,
    pub phi_low: f64,
    /// Maximum consciousness processing steps
    pub max_consciousness_steps: usize,
    /// Memory consolidation interval
    pub consolidation_interval: Duration,
    /// Enable introspection
    pub enable_introspection: bool,
}

impl Default for CLIConfig {
    fn default() -> Self {
        Self {
            bridge: BridgeConfig::default(),
            phi_critical: 100_000.0,
            phi_high: 50_000.0,
            phi_low: 10_000.0,
            max_consciousness_steps: 10_000,
            consolidation_interval: Duration::from_secs(3600), // 1 hour
            enable_introspection: true,
        }
    }
}

/// Conscious experience record
#[derive(Debug, Clone)]
pub struct ConsciousExperience {
    /// Unique experience ID
    pub id: u64,
    /// Original query
    pub query: String,
    /// Query embedding
    pub query_embedding: Vec<f32>,
    /// Extracted qualia (polychronous groups)
    pub qualia: Vec<PolychronousGroup>,
    /// Integrated information level
    pub phi: f64,
    /// Generated response
    pub response: String,
    /// Emotional valence [-1.0, 1.0]
    pub emotional_valence: f32,
    /// Arousal level [0.0, 1.0]
    pub arousal: f32,
    /// Associated concepts
    pub language_associations: Vec<String>,
    /// Feedback score (updated later)
    pub feedback_score: f32,
    /// Timestamp
    pub timestamp: Instant,
}

/// Response from conscious processing
#[derive(Debug, Clone)]
pub struct ConsciousResponse {
    /// Generated text response
    pub text: String,
    /// Φ level during processing
    pub phi_level: f64,
    /// Number of qualia detected
    pub qualia_count: usize,
    /// Consciousness mode used
    pub consciousness_mode: ConsciousnessMode,
    /// Number of recalled experiences
    pub recalled_experiences: usize,
    /// Experience ID for feedback
    pub experience_id: u64,
    /// Processing latency
    pub latency_ms: u64,
}

/// Consciousness processing modes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConsciousnessMode {
    /// High Φ: Full conscious attention
    Full,
    /// Medium Φ: Background processing
    Background,
    /// Low Φ: Reflexive response
    Reflex,
}

impl ConsciousnessMode {
    pub fn from_phi(phi: f64, config: &CLIConfig) -> Self {
        if phi > config.phi_high {
            ConsciousnessMode::Full
        } else if phi > config.phi_low {
            ConsciousnessMode::Background
        } else {
            ConsciousnessMode::Reflex
        }
    }
}

/// Introspection data about current conscious state
#[derive(Debug, Clone)]
pub struct Introspection {
    /// Current Φ level
    pub phi_level: f64,
    /// Current consciousness mode
    pub consciousness_mode: ConsciousnessMode,
    /// Number of active qualia
    pub active_qualia_count: usize,
    /// Current emotional state
    pub emotional_state: EmotionalState,
    /// What the system is "thinking about"
    pub thinking_about: Vec<String>,
    /// Recent experience count
    pub recent_experience_count: usize,
    /// Dominant learned patterns
    pub dominant_patterns: Vec<String>,
}

/// Emotional state derived from qualia
#[derive(Debug, Clone)]
pub struct EmotionalState {
    /// Primary emotion
    pub primary: Emotion,
    /// Valence [-1.0, 1.0]
    pub valence: f32,
    /// Arousal [0.0, 1.0]
    pub arousal: f32,
    /// Confidence in this assessment
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Emotion {
    Neutral,
    Curious,
    Pleased,
    Concerned,
    Excited,
    Calm,
    Confused,
    Confident,
}

impl EmotionalState {
    pub fn from_valence_arousal(valence: f32, arousal: f32) -> Self {
        let primary = match (valence > 0.0, arousal > 0.5) {
            (true, true) => Emotion::Excited,
            (true, false) => Emotion::Calm,
            (false, true) => Emotion::Concerned,
            (false, false) => Emotion::Neutral,
        };

        Self {
            primary,
            valence,
            arousal,
            confidence: 0.7,
        }
    }

    pub fn neutral() -> Self {
        Self {
            primary: Emotion::Neutral,
            valence: 0.0,
            arousal: 0.5,
            confidence: 1.0,
        }
    }
}

/// Main Conscious Language Interface
pub struct ConsciousLanguageInterface {
    /// Configuration
    config: CLIConfig,

    /// Spike-embedding bridge
    bridge: SpikeEmbeddingBridge,

    /// Experience storage (simplified - would integrate with ReasoningBank)
    experiences: HashMap<u64, ConsciousExperience>,
    next_experience_id: u64,

    /// Current consciousness state (mock - would be full spiking network)
    current_phi: f64,
    current_qualia: Vec<PolychronousGroup>,

    /// Statistics
    query_count: u64,
    total_phi: f64,
    last_consolidation: Instant,
}

impl ConsciousLanguageInterface {
    pub fn new(config: CLIConfig) -> Self {
        Self {
            bridge: SpikeEmbeddingBridge::new(config.bridge.clone()),
            config,
            experiences: HashMap::new(),
            next_experience_id: 0,
            current_phi: 0.0,
            current_qualia: Vec::new(),
            query_count: 0,
            total_phi: 0.0,
            last_consolidation: Instant::now(),
        }
    }

    /// Process a natural language query with consciousness
    ///
    /// This is the main entry point for the conscious language interface.
    pub fn process(&mut self, query: &str) -> ConsciousResponse {
        let start = Instant::now();

        // Phase 1: Generate embedding (mock - would use ruvLLM)
        let embedding = self.mock_embed(query);

        // Phase 2: Recall similar experiences (get count only to avoid borrow issues)
        let recalled_count = self.recall_similar(&embedding, 5).len();

        // Phase 3: Inject into consciousness engine
        let injection = self.bridge.encode(&embedding);

        // Phase 4: Run consciousness processing (mock)
        let (phi, qualia) = self.mock_consciousness_processing(&injection);
        self.current_phi = phi;
        self.current_qualia = qualia.clone();

        // Phase 5: Extract emotional state
        let (valence, arousal) = self.estimate_emotion(&qualia);

        // Phase 6: Decode qualia to embedding
        let qualia_embedding = self.bridge.decode(&qualia);

        // Phase 7: Generate response (mock - would use ruvLLM)
        let response_text = self.mock_generate(query, &qualia_embedding, phi);

        // Phase 8: Determine consciousness mode
        let mode = ConsciousnessMode::from_phi(phi, &self.config);

        // Phase 9: Store experience
        let experience = ConsciousExperience {
            id: self.next_experience_id,
            query: query.to_string(),
            query_embedding: embedding,
            qualia: qualia.clone(),
            phi,
            response: response_text.clone(),
            emotional_valence: valence,
            arousal,
            language_associations: self.extract_concepts(query),
            feedback_score: 0.0,
            timestamp: Instant::now(),
        };

        let experience_id = experience.id;
        self.experiences.insert(experience_id, experience);
        self.next_experience_id += 1;

        // Update statistics
        self.query_count += 1;
        self.total_phi += phi;

        // Check for consolidation
        if self.last_consolidation.elapsed() > self.config.consolidation_interval {
            self.consolidate_memory();
        }

        let latency = start.elapsed().as_millis() as u64;

        ConsciousResponse {
            text: response_text,
            phi_level: phi,
            qualia_count: qualia.len(),
            consciousness_mode: mode,
            recalled_experiences: recalled_count,
            experience_id,
            latency_ms: latency,
        }
    }

    /// Provide feedback on a response (for learning)
    pub fn feedback(&mut self, experience_id: u64, score: f32, comment: Option<&str>) {
        // First, extract data we need for learning
        let learning_data = self.experiences.get(&experience_id).map(|exp| {
            (exp.query_embedding.clone(), exp.qualia.clone())
        });

        // Update the feedback score
        if let Some(exp) = self.experiences.get_mut(&experience_id) {
            exp.feedback_score = score;
        }

        // Learn from this experience
        if let Some((query_embedding, qualia)) = learning_data {
            self.bridge.learn(&query_embedding, &qualia, score);

            // If comment provided, use as correction signal
            if let Some(comment) = comment {
                let correction_embedding = self.mock_embed(comment);
                self.bridge.add_correction(&query_embedding, &correction_embedding, score);
            }
        }
    }

    /// Introspect on current conscious state
    pub fn introspect(&self) -> Introspection {
        let emotional_state = if self.current_qualia.is_empty() {
            EmotionalState::neutral()
        } else {
            let (valence, arousal) = self.estimate_emotion(&self.current_qualia);
            EmotionalState::from_valence_arousal(valence, arousal)
        };

        let thinking_about: Vec<String> = self.current_qualia
            .iter()
            .filter_map(|q| q.label.clone())
            .take(3)
            .collect();

        Introspection {
            phi_level: self.current_phi,
            consciousness_mode: ConsciousnessMode::from_phi(self.current_phi, &self.config),
            active_qualia_count: self.current_qualia.len(),
            emotional_state,
            thinking_about,
            recent_experience_count: self.experiences.len(),
            dominant_patterns: vec!["general".to_string()], // Would come from ReasoningBank
        }
    }

    /// Describe current conscious state in natural language
    pub fn describe_self(&self) -> String {
        let intro = self.introspect();

        format!(
            "My current conscious state has Φ = {:.0}, operating in {:?} mode. \
             I'm aware of {} distinct qualia. My emotional state is {:?} \
             (valence: {:.2}, arousal: {:.2}). I have {} recent experiences in memory.",
            intro.phi_level,
            intro.consciousness_mode,
            intro.active_qualia_count,
            intro.emotional_state.primary,
            intro.emotional_state.valence,
            intro.emotional_state.arousal,
            intro.recent_experience_count
        )
    }

    /// Memory consolidation (like sleep)
    fn consolidate_memory(&mut self) {
        // Get high-quality experiences
        let high_quality: Vec<_> = self.experiences
            .values()
            .filter(|e| e.feedback_score > 0.7)
            .filter(|e| e.phi > self.config.phi_low)
            .collect();

        // Would cluster and consolidate to ReasoningBank
        // For now, just update timestamp
        self.last_consolidation = Instant::now();

        println!("Consolidated {} high-quality experiences", high_quality.len());
    }

    // Mock implementations (would integrate with actual ruvLLM and spiking network)

    fn mock_embed(&self, text: &str) -> Vec<f32> {
        // Simple hash-based embedding (mock)
        let mut embedding = vec![0.0; self.config.bridge.embedding_dim];
        for (i, c) in text.chars().enumerate() {
            let idx = i % self.config.bridge.embedding_dim;
            embedding[idx] += (c as u32 as f32) / 1000.0;
        }
        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }
        embedding
    }

    fn mock_consciousness_processing(&self, _injection: &SpikeInjection) -> (f64, Vec<PolychronousGroup>) {
        // Mock consciousness processing
        // Would actually run the spiking network
        let phi = 50_000.0 + (rand_float() * 100_000.0) as f64;

        let qualia = vec![
            PolychronousGroup {
                pattern: vec![(0, 0), (1, 100), (2, 200)],
                phi: phi * 0.3,
                occurrences: 1,
                label: Some("contemplation".to_string()),
            },
            PolychronousGroup {
                pattern: vec![(100, 50), (101, 150), (102, 250)],
                phi: phi * 0.2,
                occurrences: 1,
                label: Some("understanding".to_string()),
            },
        ];

        (phi, qualia)
    }

    fn mock_generate(&self, query: &str, _qualia_embedding: &[f32], phi: f64) -> String {
        // Mock response generation
        let mode = ConsciousnessMode::from_phi(phi, &self.config);

        match mode {
            ConsciousnessMode::Full => {
                format!(
                    "After deep contemplation of '{}', I experience a sense of \
                     integrated understanding. The question evokes patterns of \
                     thought that feel coherent and meaningful.",
                    query
                )
            }
            ConsciousnessMode::Background => {
                format!(
                    "Regarding '{}': I process this with moderate attention, \
                     drawing on learned patterns while remaining open to new insights.",
                    query
                )
            }
            ConsciousnessMode::Reflex => {
                format!("Quick response to '{}': processed reflexively.", query)
            }
        }
    }

    fn recall_similar(&self, embedding: &[f32], k: usize) -> Vec<&ConsciousExperience> {
        // Simple cosine similarity search
        let mut scored: Vec<_> = self.experiences
            .values()
            .map(|exp| {
                let sim = cosine_similarity(embedding, &exp.query_embedding);
                (exp, sim)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().take(k).map(|(exp, _)| exp).collect()
    }

    fn estimate_emotion(&self, qualia: &[PolychronousGroup]) -> (f32, f32) {
        if qualia.is_empty() {
            return (0.0, 0.5);
        }

        // Derive emotion from qualia characteristics
        let avg_phi: f64 = qualia.iter().map(|q| q.phi).sum::<f64>() / qualia.len() as f64;
        let complexity = qualia.iter().map(|q| q.pattern.len()).sum::<usize>() as f32;

        // Higher phi → more positive valence (engaged, interested)
        let valence = ((avg_phi / self.config.phi_high) as f32 - 0.5).clamp(-1.0, 1.0);

        // More complexity → higher arousal
        let arousal = (complexity / 20.0).clamp(0.0, 1.0);

        (valence, arousal)
    }

    fn extract_concepts(&self, text: &str) -> Vec<String> {
        // Simple word extraction (would use NLP)
        text.split_whitespace()
            .filter(|w| w.len() > 4)
            .take(5)
            .map(|s| s.to_lowercase())
            .collect()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < 1e-6 || norm_b < 1e-6 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

fn rand_float() -> f32 {
    use std::cell::Cell;
    thread_local! {
        static SEED: Cell<u64> = Cell::new(0xCAFEBABE);
    }

    SEED.with(|seed| {
        let mut s = seed.get();
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        seed.set(s);
        (s as f32) / (u64::MAX as f32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conscious_interface() {
        let config = CLIConfig::default();
        let mut cli = ConsciousLanguageInterface::new(config);

        let response = cli.process("What is consciousness?");

        assert!(!response.text.is_empty());
        assert!(response.phi_level > 0.0);
        assert!(response.qualia_count > 0);
    }

    #[test]
    fn test_feedback() {
        let config = CLIConfig::default();
        let mut cli = ConsciousLanguageInterface::new(config);

        let response = cli.process("Hello");
        cli.feedback(response.experience_id, 0.9, Some("Great response!"));

        // Check experience was updated
        let exp = cli.experiences.get(&response.experience_id).unwrap();
        assert_eq!(exp.feedback_score, 0.9);
    }

    #[test]
    fn test_introspection() {
        let config = CLIConfig::default();
        let mut cli = ConsciousLanguageInterface::new(config);

        // Process something first
        cli.process("Think about this");

        let intro = cli.introspect();
        assert!(intro.phi_level > 0.0);
        assert!(intro.active_qualia_count > 0);
    }

    #[test]
    fn test_self_description() {
        let config = CLIConfig::default();
        let mut cli = ConsciousLanguageInterface::new(config);

        cli.process("Initialize");

        let description = cli.describe_self();
        assert!(description.contains("Φ"));
        assert!(description.contains("conscious"));
    }
}
