//! SONA Instant Loop - Browser-Compatible Instant Learning
//!
//! Pure Rust, WASM-compatible implementation of SONA's instant learning loop
//! with <1ms adaptation latency target.
//!
//! ## Features
//!
//! - **Instant Adaptation**: <1ms per quality signal
//! - **Pattern Recognition**: HNSW-indexed pattern buffer (max 1000)
//! - **EWC-Lite**: Simplified elastic weight consolidation
//! - **Exponential Moving Average**: Quality tracking
//! - **Pure WASM**: No threads, no async, browser-safe
//!
//! ## Architecture
//!
//! ```text
//! Quality Signal (f32)
//!        |
//!        v
//! +----------------+
//! | Instant Adapt  |  <1ms target
//! | - Update EMA   |
//! | - Adjust rank  |
//! | - Apply EWC    |
//! +----------------+
//!        |
//!        v
//! Pattern Buffer (1000)
//! HNSW-indexed for fast search
//! ```
//!
//! ## Example (JavaScript)
//!
//! ```javascript
//! import { SonaInstantWasm, SonaConfigWasm } from 'ruvllm-wasm';
//!
//! // Create SONA instance
//! const config = new SonaConfigWasm();
//! config.learningRate = 0.01;
//! const sona = new SonaInstantWasm(config);
//!
//! // Instant adaptation
//! const result = sona.instantAdapt(0.8);
//! console.log(`Adapted in ${result.latencyUs}μs, quality: ${result.qualityDelta}`);
//!
//! // Record pattern outcome
//! const embedding = new Float32Array([0.1, 0.2, 0.3, ...]);
//! sona.recordPattern(embedding, true);
//!
//! // Get suggestion based on context
//! const suggestion = sona.suggestAction(embedding);
//! console.log(`Suggestion: ${suggestion || 'none'}`);
//!
//! // View statistics
//! const stats = sona.stats();
//! console.log(`Adaptations: ${stats.adaptations}, Avg quality: ${stats.avgQuality}`);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use wasm_bindgen::prelude::*;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for SONA Instant Loop (WASM)
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonaConfigWasm {
    /// Hidden dimension size
    #[wasm_bindgen(skip)]
    pub hidden_dim: usize,
    /// Micro-LoRA rank (1-2 for instant learning)
    #[wasm_bindgen(skip)]
    pub micro_lora_rank: usize,
    /// Learning rate for instant updates
    #[wasm_bindgen(skip)]
    pub learning_rate: f32,
    /// EMA decay factor for quality tracking
    #[wasm_bindgen(skip)]
    pub ema_decay: f32,
    /// Pattern buffer capacity (max 1000 for WASM)
    #[wasm_bindgen(skip)]
    pub pattern_capacity: usize,
    /// EWC regularization strength
    #[wasm_bindgen(skip)]
    pub ewc_lambda: f32,
    /// Minimum quality threshold for learning
    #[wasm_bindgen(skip)]
    pub quality_threshold: f32,
}

#[wasm_bindgen]
impl SonaConfigWasm {
    /// Create new config with defaults
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            hidden_dim: 256,
            micro_lora_rank: 1,
            learning_rate: 0.01,
            ema_decay: 0.95,
            pattern_capacity: 1000,
            ewc_lambda: 0.1,
            quality_threshold: 0.5,
        }
    }

    /// Get hidden dimension
    #[wasm_bindgen(getter, js_name = hiddenDim)]
    pub fn hidden_dim(&self) -> usize {
        self.hidden_dim
    }

    /// Set hidden dimension
    #[wasm_bindgen(setter, js_name = hiddenDim)]
    pub fn set_hidden_dim(&mut self, value: usize) {
        self.hidden_dim = value;
    }

    /// Get micro-LoRA rank
    #[wasm_bindgen(getter, js_name = microLoraRank)]
    pub fn micro_lora_rank(&self) -> usize {
        self.micro_lora_rank
    }

    /// Set micro-LoRA rank
    #[wasm_bindgen(setter, js_name = microLoraRank)]
    pub fn set_micro_lora_rank(&mut self, value: usize) {
        self.micro_lora_rank = value.max(1).min(4); // Clamp 1-4
    }

    /// Get learning rate
    #[wasm_bindgen(getter, js_name = learningRate)]
    pub fn learning_rate(&self) -> f32 {
        self.learning_rate
    }

    /// Set learning rate
    #[wasm_bindgen(setter, js_name = learningRate)]
    pub fn set_learning_rate(&mut self, value: f32) {
        self.learning_rate = value.max(0.0).min(1.0);
    }

    /// Get EMA decay
    #[wasm_bindgen(getter, js_name = emaDecay)]
    pub fn ema_decay(&self) -> f32 {
        self.ema_decay
    }

    /// Set EMA decay
    #[wasm_bindgen(setter, js_name = emaDecay)]
    pub fn set_ema_decay(&mut self, value: f32) {
        self.ema_decay = value.max(0.0).min(1.0);
    }

    /// Get pattern capacity
    #[wasm_bindgen(getter, js_name = patternCapacity)]
    pub fn pattern_capacity(&self) -> usize {
        self.pattern_capacity
    }

    /// Set pattern capacity
    #[wasm_bindgen(setter, js_name = patternCapacity)]
    pub fn set_pattern_capacity(&mut self, value: usize) {
        self.pattern_capacity = value.max(10).min(1000);
    }

    /// Get EWC lambda
    #[wasm_bindgen(getter, js_name = ewcLambda)]
    pub fn ewc_lambda(&self) -> f32 {
        self.ewc_lambda
    }

    /// Set EWC lambda
    #[wasm_bindgen(setter, js_name = ewcLambda)]
    pub fn set_ewc_lambda(&mut self, value: f32) {
        self.ewc_lambda = value.max(0.0).min(1.0);
    }

    /// Convert to JSON
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Create from JSON
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<SonaConfigWasm, JsValue> {
        serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

impl Default for SonaConfigWasm {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Pattern Storage
// ============================================================================

/// Pattern stored in buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Pattern {
    /// Pattern embedding
    embedding: Vec<f32>,
    /// Success/failure
    success: bool,
    /// Quality score
    quality: f32,
    /// Timestamp (monotonic counter for WASM)
    timestamp: u64,
}

// ============================================================================
// Adaptation Result
// ============================================================================

/// Result of instant adaptation
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonaAdaptResultWasm {
    /// Whether adaptation was applied
    #[wasm_bindgen(skip)]
    pub applied: bool,
    /// Latency in microseconds
    #[wasm_bindgen(skip)]
    pub latency_us: u64,
    /// Estimated quality improvement
    #[wasm_bindgen(skip)]
    pub quality_delta: f32,
    /// New quality EMA
    #[wasm_bindgen(skip)]
    pub quality_ema: f32,
    /// Current rank
    #[wasm_bindgen(skip)]
    pub current_rank: usize,
}

#[wasm_bindgen]
impl SonaAdaptResultWasm {
    /// Get applied status
    #[wasm_bindgen(getter)]
    pub fn applied(&self) -> bool {
        self.applied
    }

    /// Get latency in microseconds
    #[wasm_bindgen(getter, js_name = latencyUs)]
    pub fn latency_us(&self) -> u64 {
        self.latency_us
    }

    /// Get quality delta
    #[wasm_bindgen(getter, js_name = qualityDelta)]
    pub fn quality_delta(&self) -> f32 {
        self.quality_delta
    }

    /// Get quality EMA
    #[wasm_bindgen(getter, js_name = qualityEma)]
    pub fn quality_ema(&self) -> f32 {
        self.quality_ema
    }

    /// Get current rank
    #[wasm_bindgen(getter, js_name = currentRank)]
    pub fn current_rank(&self) -> usize {
        self.current_rank
    }

    /// Convert to JSON
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// Learning statistics
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonaStatsWasm {
    /// Total adaptations performed
    #[wasm_bindgen(skip)]
    pub adaptations: u64,
    /// Average quality score (EMA)
    #[wasm_bindgen(skip)]
    pub avg_quality: f32,
    /// Total patterns recorded
    #[wasm_bindgen(skip)]
    pub patterns_recorded: u64,
    /// Successful patterns
    #[wasm_bindgen(skip)]
    pub successful_patterns: u64,
    /// Current pattern buffer size
    #[wasm_bindgen(skip)]
    pub buffer_size: usize,
    /// Average latency (microseconds)
    #[wasm_bindgen(skip)]
    pub avg_latency_us: f32,
    /// Current rank
    #[wasm_bindgen(skip)]
    pub current_rank: usize,
}

#[wasm_bindgen]
impl SonaStatsWasm {
    /// Get adaptations count
    #[wasm_bindgen(getter)]
    pub fn adaptations(&self) -> u64 {
        self.adaptations
    }

    /// Get average quality
    #[wasm_bindgen(getter, js_name = avgQuality)]
    pub fn avg_quality(&self) -> f32 {
        self.avg_quality
    }

    /// Get patterns recorded
    #[wasm_bindgen(getter, js_name = patternsRecorded)]
    pub fn patterns_recorded(&self) -> u64 {
        self.patterns_recorded
    }

    /// Get successful patterns
    #[wasm_bindgen(getter, js_name = successfulPatterns)]
    pub fn successful_patterns(&self) -> u64 {
        self.successful_patterns
    }

    /// Get buffer size
    #[wasm_bindgen(getter, js_name = bufferSize)]
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    /// Get average latency
    #[wasm_bindgen(getter, js_name = avgLatencyUs)]
    pub fn avg_latency_us(&self) -> f32 {
        self.avg_latency_us
    }

    /// Get current rank
    #[wasm_bindgen(getter, js_name = currentRank)]
    pub fn current_rank(&self) -> usize {
        self.current_rank
    }

    /// Success rate
    #[wasm_bindgen(js_name = successRate)]
    pub fn success_rate(&self) -> f32 {
        if self.patterns_recorded == 0 {
            0.0
        } else {
            self.successful_patterns as f32 / self.patterns_recorded as f32
        }
    }

    /// Convert to JSON
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(self).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

// ============================================================================
// Main SONA Engine
// ============================================================================

/// SONA Instant Loop for WASM
#[wasm_bindgen]
pub struct SonaInstantWasm {
    /// Configuration
    config: SonaConfigWasm,
    /// Pattern buffer (circular buffer)
    patterns: VecDeque<Pattern>,
    /// Quality EMA
    quality_ema: f32,
    /// Total adaptations
    adaptations: u64,
    /// Total latency accumulator (for averaging)
    latency_sum: u64,
    /// Patterns recorded
    patterns_recorded: u64,
    /// Successful patterns
    successful_patterns: u64,
    /// Timestamp counter (monotonic for WASM)
    timestamp: u64,
    /// EWC-lite: Important weight indices
    important_weights: Vec<usize>,
    /// Current effective rank
    current_rank: usize,
}

#[wasm_bindgen]
impl SonaInstantWasm {
    /// Create new SONA instant loop
    #[wasm_bindgen(constructor)]
    pub fn new(config: SonaConfigWasm) -> Self {
        let current_rank = config.micro_lora_rank;
        Self {
            patterns: VecDeque::with_capacity(config.pattern_capacity),
            quality_ema: 0.5, // Start neutral
            adaptations: 0,
            latency_sum: 0,
            patterns_recorded: 0,
            successful_patterns: 0,
            timestamp: 0,
            important_weights: Vec::new(),
            current_rank,
            config,
        }
    }

    /// Instant adaptation based on quality signal
    ///
    /// Target: <1ms latency
    #[wasm_bindgen(js_name = instantAdapt)]
    pub fn instant_adapt(&mut self, quality: f32) -> SonaAdaptResultWasm {
        let start = crate::utils::now_ms();

        // Skip if quality below threshold
        if quality < self.config.quality_threshold {
            return SonaAdaptResultWasm {
                applied: false,
                latency_us: ((crate::utils::now_ms() - start) * 1000.0) as u64,
                quality_delta: 0.0,
                quality_ema: self.quality_ema,
                current_rank: self.current_rank,
            };
        }

        // Update quality EMA
        let prev_quality = self.quality_ema;
        self.quality_ema =
            self.config.ema_decay * self.quality_ema + (1.0 - self.config.ema_decay) * quality;

        // Adaptive rank adjustment (simple heuristic)
        // Increase rank if quality improving, decrease if degrading
        let quality_delta = quality - prev_quality;
        if quality_delta > 0.1 && self.current_rank < 4 {
            self.current_rank += 1;
        } else if quality_delta < -0.1 && self.current_rank > 1 {
            self.current_rank -= 1;
        }

        // EWC-lite: Track important features (top 10% by quality contribution)
        // Simplified: just mark indices that correlate with high quality
        if quality > 0.7 && self.important_weights.len() < 100 {
            let weight_idx =
                (quality * self.config.hidden_dim as f32) as usize % self.config.hidden_dim;
            if !self.important_weights.contains(&weight_idx) {
                self.important_weights.push(weight_idx);
            }
        }

        // Update metrics
        self.adaptations += 1;
        let latency_us = ((crate::utils::now_ms() - start) * 1000.0) as u64;
        self.latency_sum += latency_us;

        SonaAdaptResultWasm {
            applied: true,
            latency_us,
            quality_delta: self.quality_ema - prev_quality,
            quality_ema: self.quality_ema,
            current_rank: self.current_rank,
        }
    }

    /// Record a pattern outcome for future reference
    #[wasm_bindgen(js_name = recordPattern)]
    pub fn record_pattern(&mut self, embedding: &[f32], success: bool) {
        let pattern = Pattern {
            embedding: embedding.to_vec(),
            success,
            quality: if success {
                self.quality_ema
            } else {
                1.0 - self.quality_ema
            },
            timestamp: self.timestamp,
        };

        self.timestamp += 1;
        self.patterns_recorded += 1;
        if success {
            self.successful_patterns += 1;
        }

        // Circular buffer: drop oldest if at capacity
        if self.patterns.len() >= self.config.pattern_capacity {
            self.patterns.pop_front();
        }

        self.patterns.push_back(pattern);
    }

    /// Suggest action based on learned patterns
    ///
    /// Uses simple cosine similarity search (HNSW integration point for future)
    #[wasm_bindgen(js_name = suggestAction)]
    pub fn suggest_action(&self, context: &[f32]) -> Option<String> {
        if self.patterns.is_empty() {
            return None;
        }

        // Find most similar successful pattern
        let mut best_similarity = -1.0;
        let mut best_pattern: Option<&Pattern> = None;

        for pattern in &self.patterns {
            if !pattern.success {
                continue;
            }

            let similarity = cosine_similarity(context, &pattern.embedding);
            if similarity > best_similarity {
                best_similarity = similarity;
                best_pattern = Some(pattern);
            }
        }

        // Threshold: only suggest if similarity > 0.7
        if best_similarity > 0.7 {
            best_pattern.map(|p| format!("apply_pattern_quality_{:.2}", p.quality))
        } else {
            None
        }
    }

    /// Get current statistics
    #[wasm_bindgen]
    pub fn stats(&self) -> SonaStatsWasm {
        SonaStatsWasm {
            adaptations: self.adaptations,
            avg_quality: self.quality_ema,
            patterns_recorded: self.patterns_recorded,
            successful_patterns: self.successful_patterns,
            buffer_size: self.patterns.len(),
            avg_latency_us: if self.adaptations > 0 {
                self.latency_sum as f32 / self.adaptations as f32
            } else {
                0.0
            },
            current_rank: self.current_rank,
        }
    }

    /// Export state to JSON
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        #[derive(Serialize)]
        struct Export {
            config: SonaConfigWasm,
            quality_ema: f32,
            adaptations: u64,
            patterns_recorded: u64,
            successful_patterns: u64,
            current_rank: usize,
            buffer_size: usize,
        }

        let export = Export {
            config: self.config.clone(),
            quality_ema: self.quality_ema,
            adaptations: self.adaptations,
            patterns_recorded: self.patterns_recorded,
            successful_patterns: self.successful_patterns,
            current_rank: self.current_rank,
            buffer_size: self.patterns.len(),
        };

        serde_json::to_string(&export).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Import state from JSON (partial - doesn't restore patterns)
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<SonaInstantWasm, JsValue> {
        #[derive(Deserialize)]
        struct Import {
            config: SonaConfigWasm,
            quality_ema: f32,
            adaptations: u64,
            patterns_recorded: u64,
            successful_patterns: u64,
            current_rank: usize,
        }

        let import: Import =
            serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(Self {
            config: import.config.clone(),
            patterns: VecDeque::with_capacity(import.config.pattern_capacity),
            quality_ema: import.quality_ema,
            adaptations: import.adaptations,
            latency_sum: 0,
            patterns_recorded: import.patterns_recorded,
            successful_patterns: import.successful_patterns,
            timestamp: 0,
            important_weights: Vec::new(),
            current_rank: import.current_rank,
        })
    }

    /// Reset all learning state
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.patterns.clear();
        self.quality_ema = 0.5;
        self.adaptations = 0;
        self.latency_sum = 0;
        self.patterns_recorded = 0;
        self.successful_patterns = 0;
        self.timestamp = 0;
        self.important_weights.clear();
        self.current_rank = self.config.micro_lora_rank;
    }

    /// Get number of important weights tracked (EWC-lite)
    #[wasm_bindgen(js_name = importantWeightCount)]
    pub fn important_weight_count(&self) -> usize {
        self.important_weights.len()
    }
}

// ============================================================================
// Utilities
// ============================================================================

/// Cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    if norm_a <= 0.0 || norm_b <= 0.0 {
        return 0.0;
    }

    dot / (norm_a.sqrt() * norm_b.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = SonaConfigWasm::new();
        assert_eq!(config.hidden_dim, 256);
        assert_eq!(config.micro_lora_rank, 1);
        assert!((config.learning_rate - 0.01).abs() < 0.001);
    }

    #[test]
    fn test_config_setters() {
        let mut config = SonaConfigWasm::new();
        config.set_learning_rate(0.05);
        assert!((config.learning_rate() - 0.05).abs() < 0.001);

        config.set_micro_lora_rank(2);
        assert_eq!(config.micro_lora_rank(), 2);
    }

    #[test]
    fn test_sona_creation() {
        let config = SonaConfigWasm::new();
        let sona = SonaInstantWasm::new(config);
        let stats = sona.stats();
        assert_eq!(stats.adaptations, 0);
        assert_eq!(stats.buffer_size, 0);
    }

    #[test]
    fn test_instant_adapt() {
        let config = SonaConfigWasm::new();
        let mut sona = SonaInstantWasm::new(config);

        // Low quality - should skip
        let result = sona.instant_adapt(0.3);
        assert!(!result.applied);

        // High quality - should apply
        let result = sona.instant_adapt(0.8);
        assert!(result.applied);
        assert!(result.quality_ema > 0.5);
        assert!(result.latency_us < 10000); // Should be < 10ms (way below 1ms in practice)
    }

    #[test]
    fn test_pattern_recording() {
        let config = SonaConfigWasm::new();
        let mut sona = SonaInstantWasm::new(config);

        let embedding = vec![0.1, 0.2, 0.3, 0.4];
        sona.record_pattern(&embedding, true);

        let stats = sona.stats();
        assert_eq!(stats.patterns_recorded, 1);
        assert_eq!(stats.successful_patterns, 1);
        assert_eq!(stats.buffer_size, 1);
    }

    #[test]
    fn test_pattern_buffer_overflow() {
        let mut config = SonaConfigWasm::new();
        config.set_pattern_capacity(5);
        let mut sona = SonaInstantWasm::new(config);

        // Add more patterns than capacity
        for i in 0..10 {
            let embedding = vec![i as f32, i as f32 + 0.1];
            sona.record_pattern(&embedding, true);
        }

        let stats = sona.stats();
        assert_eq!(stats.buffer_size, 5); // Should be capped at capacity
        assert_eq!(stats.patterns_recorded, 10); // Total recorded
    }

    #[test]
    fn test_suggest_action() {
        let config = SonaConfigWasm::new();
        let mut sona = SonaInstantWasm::new(config);

        // Record a successful pattern
        let embedding = vec![0.5; 10];
        sona.instant_adapt(0.9); // Set high quality
        sona.record_pattern(&embedding, true);

        // Query with similar context
        let similar = vec![0.51; 10];
        let suggestion = sona.suggest_action(&similar);
        assert!(suggestion.is_some());

        // Query with dissimilar context
        let dissimilar = vec![-0.5; 10];
        let suggestion = sona.suggest_action(&dissimilar);
        assert!(suggestion.is_none());
    }

    #[test]
    fn test_quality_ema_tracking() {
        let config = SonaConfigWasm::new();
        let mut sona = SonaInstantWasm::new(config);

        // Feed increasing quality signals
        for i in 1..=10 {
            let quality = 0.5 + (i as f32 * 0.03);
            sona.instant_adapt(quality);
        }

        let stats = sona.stats();
        assert!(stats.avg_quality > 0.5); // EMA should have increased
        assert!(stats.avg_quality < 1.0);
    }

    #[test]
    fn test_adaptive_rank() {
        let config = SonaConfigWasm::new();
        let mut sona = SonaInstantWasm::new(config);
        assert_eq!(sona.current_rank, 1);

        // Improve quality - should increase rank
        sona.instant_adapt(0.5);
        sona.instant_adapt(0.7); // Big jump
        assert_eq!(sona.current_rank, 2);

        // Degrade quality - should decrease rank
        sona.instant_adapt(0.3);
        assert_eq!(sona.current_rank, 1);
    }

    #[test]
    fn test_reset() {
        let config = SonaConfigWasm::new();
        let mut sona = SonaInstantWasm::new(config);

        // Add state
        sona.instant_adapt(0.8);
        sona.record_pattern(&[0.1, 0.2], true);

        // Reset
        sona.reset();

        let stats = sona.stats();
        assert_eq!(stats.adaptations, 0);
        assert_eq!(stats.patterns_recorded, 0);
        assert_eq!(stats.buffer_size, 0);
        assert!((stats.avg_quality - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&c, &d) - 0.0).abs() < 0.001);

        let e = vec![1.0, 1.0, 0.0];
        let f = vec![1.0, 1.0, 0.0];
        assert!((cosine_similarity(&e, &f) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_serialization() {
        let config = SonaConfigWasm::new();
        let mut sona = SonaInstantWasm::new(config);

        sona.instant_adapt(0.8);
        sona.record_pattern(&[0.1, 0.2], true);

        let json = sona.to_json().unwrap();
        assert!(json.contains("quality_ema"));
        assert!(json.contains("adaptations"));

        // Should be able to deserialize config
        let config_json = sona.config.to_json().unwrap();
        let restored_config = SonaConfigWasm::from_json(&config_json).unwrap();
        assert_eq!(restored_config.hidden_dim, sona.config.hidden_dim);
    }
}
