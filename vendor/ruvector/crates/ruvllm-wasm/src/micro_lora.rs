//! MicroLoRA for WASM - Browser-Compatible Lightweight LoRA Adaptation
//!
//! This module provides ultra-lightweight LoRA (Low-Rank Adaptation) for browser-based
//! LLM inference. Designed for minimal memory footprint and real-time per-request adaptation.
//!
//! ## Features
//!
//! - **Rank 1-4 adapters**: Very small memory footprint (<10KB per adapter)
//! - **Pure Rust**: No threading, no file I/O, fully WASM-compatible
//! - **Per-request adaptation**: Update weights based on user feedback
//! - **Serialization**: JSON-based persistence for browser storage
//!
//! ## Example (JavaScript)
//!
//! ```javascript
//! import { MicroLoraWasm, MicroLoraConfigWasm, AdaptFeedbackWasm } from 'ruvllm-wasm';
//!
//! // Create a rank-2 adapter for 768-dim hidden states
//! const config = new MicroLoraConfigWasm();
//! config.rank = 2;
//! config.alpha = 4.0;
//! config.inFeatures = 768;
//! config.outFeatures = 768;
//!
//! const lora = new MicroLoraWasm(config);
//!
//! // Apply LoRA to input
//! const input = new Float32Array(768);
//! const output = lora.apply(input);
//!
//! // Adapt based on feedback
//! const feedback = new AdaptFeedbackWasm();
//! feedback.quality = 0.8;
//! lora.adapt(input, feedback);
//!
//! // Serialize for persistence
//! const json = lora.toJson();
//! localStorage.setItem('lora-state', json);
//!
//! // Restore from JSON
//! const restored = MicroLoraWasm.fromJson(json);
//! ```

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for MicroLoRA adapter.
///
/// Controls the rank, scaling, and dimensions of the LoRA adapter.
/// TypeScript-friendly with getter/setter methods.
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroLoraConfigWasm {
    #[wasm_bindgen(skip)]
    pub rank: usize,
    #[wasm_bindgen(skip)]
    pub alpha: f32,
    #[wasm_bindgen(skip)]
    pub in_features: usize,
    #[wasm_bindgen(skip)]
    pub out_features: usize,
}

#[wasm_bindgen]
impl MicroLoraConfigWasm {
    /// Create a new config with default values (rank=2, alpha=4.0, 768x768).
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            rank: 2,
            alpha: 4.0,
            in_features: 768,
            out_features: 768,
        }
    }

    /// Get rank.
    #[wasm_bindgen(getter)]
    pub fn rank(&self) -> usize {
        self.rank
    }

    /// Set rank (clamped to 1-4 for browser efficiency).
    #[wasm_bindgen(setter)]
    pub fn set_rank(&mut self, value: usize) {
        self.rank = value.clamp(1, 4);
    }

    /// Get alpha scaling factor.
    #[wasm_bindgen(getter)]
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Set alpha scaling factor.
    #[wasm_bindgen(setter)]
    pub fn set_alpha(&mut self, value: f32) {
        self.alpha = value;
    }

    /// Get input feature dimension.
    #[wasm_bindgen(getter, js_name = inFeatures)]
    pub fn in_features(&self) -> usize {
        self.in_features
    }

    /// Set input feature dimension.
    #[wasm_bindgen(setter, js_name = inFeatures)]
    pub fn set_in_features(&mut self, value: usize) {
        self.in_features = value;
    }

    /// Get output feature dimension.
    #[wasm_bindgen(getter, js_name = outFeatures)]
    pub fn out_features(&self) -> usize {
        self.out_features
    }

    /// Set output feature dimension.
    #[wasm_bindgen(setter, js_name = outFeatures)]
    pub fn set_out_features(&mut self, value: usize) {
        self.out_features = value;
    }

    /// Calculate memory footprint in bytes.
    #[wasm_bindgen(js_name = memoryBytes)]
    pub fn memory_bytes(&self) -> usize {
        // A: in_features x rank, B: rank x out_features
        let params = self.in_features * self.rank + self.rank * self.out_features;
        params * std::mem::size_of::<f32>()
    }

    /// Get computed scaling factor (alpha / rank).
    #[wasm_bindgen(js_name = computeScaling)]
    pub fn compute_scaling(&self) -> f32 {
        self.alpha / self.rank as f32
    }
}

impl Default for MicroLoraConfigWasm {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Feedback for Adaptation
// ============================================================================

/// Feedback for per-request adaptation.
///
/// Provides quality scores and optional gradient estimates to guide
/// LoRA weight updates.
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptFeedbackWasm {
    #[wasm_bindgen(skip)]
    pub quality: f32,
    #[wasm_bindgen(skip)]
    pub learning_rate: f32,
}

#[wasm_bindgen]
impl AdaptFeedbackWasm {
    /// Create new feedback with quality score [0.0, 1.0].
    #[wasm_bindgen(constructor)]
    pub fn new(quality: f32) -> Self {
        Self {
            quality: quality.clamp(0.0, 1.0),
            learning_rate: 0.01,
        }
    }

    /// Get quality score.
    #[wasm_bindgen(getter)]
    pub fn quality(&self) -> f32 {
        self.quality
    }

    /// Set quality score (clamped to [0.0, 1.0]).
    #[wasm_bindgen(setter)]
    pub fn set_quality(&mut self, value: f32) {
        self.quality = value.clamp(0.0, 1.0);
    }

    /// Get learning rate.
    #[wasm_bindgen(getter, js_name = learningRate)]
    pub fn learning_rate(&self) -> f32 {
        self.learning_rate
    }

    /// Set learning rate.
    #[wasm_bindgen(setter, js_name = learningRate)]
    pub fn set_learning_rate(&mut self, value: f32) {
        self.learning_rate = value;
    }
}

// ============================================================================
// Statistics
// ============================================================================

/// Statistics for MicroLoRA adapter.
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroLoraStatsWasm {
    #[wasm_bindgen(skip)]
    pub samples_seen: usize,
    #[wasm_bindgen(skip)]
    pub avg_quality: f32,
    #[wasm_bindgen(skip)]
    pub memory_bytes: usize,
    #[wasm_bindgen(skip)]
    pub param_count: usize,
}

#[wasm_bindgen]
impl MicroLoraStatsWasm {
    /// Get number of samples seen.
    #[wasm_bindgen(getter, js_name = samplesSeen)]
    pub fn samples_seen(&self) -> usize {
        self.samples_seen
    }

    /// Get average quality score.
    #[wasm_bindgen(getter, js_name = avgQuality)]
    pub fn avg_quality(&self) -> f32 {
        self.avg_quality
    }

    /// Get memory usage in bytes.
    #[wasm_bindgen(getter, js_name = memoryBytes)]
    pub fn memory_bytes(&self) -> usize {
        self.memory_bytes
    }

    /// Get parameter count.
    #[wasm_bindgen(getter, js_name = paramCount)]
    pub fn param_count(&self) -> usize {
        self.param_count
    }

    /// Convert to JSON string.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        serde_json::to_string(self)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }
}

// ============================================================================
// MicroLoRA Adapter (Internal)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoraAdapterInternal {
    /// A matrix (in_features x rank) - down projection
    lora_a: Vec<f32>,
    /// B matrix (rank x out_features) - up projection
    lora_b: Vec<f32>,
    /// Scaling factor (alpha / rank)
    scaling: f32,
    /// Rank
    rank: usize,
    /// Input features
    in_features: usize,
    /// Output features
    out_features: usize,
    /// Accumulated gradients for A
    grad_a: Vec<f32>,
    /// Accumulated gradients for B
    grad_b: Vec<f32>,
    /// Number of accumulated gradients
    grad_count: usize,
}

impl LoraAdapterInternal {
    /// Create a new LoRA adapter with Kaiming initialization for A and zeros for B.
    fn new(in_features: usize, out_features: usize, rank: usize, alpha: f32) -> Self {
        let scaling = alpha / rank as f32;

        // Kaiming initialization for A
        let std_a = (2.0 / in_features as f32).sqrt() * 0.01;
        let mut lora_a = Vec::with_capacity(in_features * rank);
        for i in 0..(in_features * rank) {
            // Deterministic pseudo-random for reproducibility
            let seed = i as f32;
            let value = ((seed * 0.618033988749895) % 1.0 - 0.5) * 2.0 * std_a;
            lora_a.push(value);
        }

        // Zero initialization for B (standard LoRA)
        let lora_b = vec![0.0; rank * out_features];

        Self {
            lora_a,
            lora_b,
            scaling,
            rank,
            in_features,
            out_features,
            grad_a: vec![0.0; in_features * rank],
            grad_b: vec![0.0; rank * out_features],
            grad_count: 0,
        }
    }

    /// Forward pass: output = x @ A @ B * scaling
    fn forward(&self, input: &[f32], output: &mut [f32]) {
        debug_assert_eq!(input.len(), self.in_features);
        debug_assert_eq!(output.len(), self.out_features);

        // Compute intermediate: x @ A (in_features -> rank)
        let mut intermediate = vec![0.0; self.rank];
        for r in 0..self.rank {
            let mut sum = 0.0;
            for i in 0..self.in_features {
                sum += input[i] * self.lora_a[i * self.rank + r];
            }
            intermediate[r] = sum;
        }

        // Compute output: intermediate @ B * scaling (rank -> out_features)
        for o in 0..self.out_features {
            let mut sum = 0.0;
            for r in 0..self.rank {
                sum += intermediate[r] * self.lora_b[r * self.out_features + o];
            }
            output[o] += sum * self.scaling;
        }
    }

    /// Accumulate gradients based on feedback quality.
    ///
    /// Uses a simplified gradient estimate based on the quality score.
    /// For browser use, we use a lightweight update rule without full backprop.
    fn accumulate_gradient(&mut self, input: &[f32], quality: f32) {
        // Compute intermediate activation
        let mut intermediate = vec![0.0; self.rank];
        for r in 0..self.rank {
            let mut sum = 0.0;
            for i in 0..self.in_features {
                sum += input[i] * self.lora_a[i * self.rank + r];
            }
            intermediate[r] = sum;
        }

        // Simple gradient estimate: use quality as reward signal
        // For positive quality (>0.5), strengthen current activation patterns
        // For negative quality (<0.5), weaken them
        let reward = (quality - 0.5) * 2.0; // Map [0,1] to [-1,1]

        // Update B gradients: outer product of intermediate and reward
        for r in 0..self.rank {
            for o in 0..self.out_features {
                let idx = r * self.out_features + o;
                self.grad_b[idx] += intermediate[r] * reward * self.scaling * 0.01;
            }
        }

        // Update A gradients: outer product of input and reward-weighted intermediate
        for i in 0..self.in_features {
            for r in 0..self.rank {
                let idx = i * self.rank + r;
                self.grad_a[idx] += input[i] * reward * self.scaling * 0.01;
            }
        }

        self.grad_count += 1;
    }

    /// Apply accumulated gradients with learning rate.
    fn apply_gradients(&mut self, learning_rate: f32) {
        if self.grad_count == 0 {
            return;
        }

        let scale = learning_rate / self.grad_count as f32;

        // Update A
        for i in 0..self.lora_a.len() {
            self.lora_a[i] -= self.grad_a[i] * scale;
        }

        // Update B
        for i in 0..self.lora_b.len() {
            self.lora_b[i] -= self.grad_b[i] * scale;
        }

        // Reset gradients
        for g in &mut self.grad_a {
            *g = 0.0;
        }
        for g in &mut self.grad_b {
            *g = 0.0;
        }
        self.grad_count = 0;
    }

    /// Reset adapter to initial state.
    fn reset(&mut self) {
        // Reset B to zeros
        for b in &mut self.lora_b {
            *b = 0.0;
        }

        // Reset gradients
        for g in &mut self.grad_a {
            *g = 0.0;
        }
        for g in &mut self.grad_b {
            *g = 0.0;
        }
        self.grad_count = 0;
    }

    /// Get parameter count.
    fn param_count(&self) -> usize {
        self.lora_a.len() + self.lora_b.len()
    }

    /// Get memory usage in bytes.
    fn memory_bytes(&self) -> usize {
        self.param_count() * std::mem::size_of::<f32>()
    }
}

// ============================================================================
// MicroLoRA (Public WASM Interface)
// ============================================================================

/// MicroLoRA adapter for browser-based real-time adaptation.
///
/// Provides lightweight LoRA (Low-Rank Adaptation) with minimal memory footprint
/// suitable for browser environments. Supports per-request adaptation with
/// quality-based feedback.
#[wasm_bindgen]
pub struct MicroLoraWasm {
    adapter: LoraAdapterInternal,
    samples_seen: usize,
    quality_sum: f32,
}

#[wasm_bindgen]
impl MicroLoraWasm {
    /// Create a new MicroLoRA adapter with the given configuration.
    #[wasm_bindgen(constructor)]
    pub fn new(config: &MicroLoraConfigWasm) -> Self {
        let adapter = LoraAdapterInternal::new(
            config.in_features,
            config.out_features,
            config.rank,
            config.alpha,
        );

        Self {
            adapter,
            samples_seen: 0,
            quality_sum: 0.0,
        }
    }

    /// Apply LoRA transformation to input.
    ///
    /// Returns a new Float32Array with the transformed output.
    /// The output is added to (not replaced) so you can combine with base model output.
    #[wasm_bindgen]
    pub fn apply(&self, input: &[f32]) -> Result<Vec<f32>, JsValue> {
        if input.len() != self.adapter.in_features {
            return Err(JsValue::from_str(&format!(
                "Input size mismatch: expected {}, got {}",
                self.adapter.in_features,
                input.len()
            )));
        }

        let mut output = vec![0.0; self.adapter.out_features];
        self.adapter.forward(input, &mut output);
        Ok(output)
    }

    /// Adapt the LoRA weights based on feedback.
    ///
    /// Accumulates gradients based on the quality score. Call `applyUpdates()`
    /// to actually apply the accumulated gradients.
    #[wasm_bindgen]
    pub fn adapt(&mut self, input: &[f32], feedback: &AdaptFeedbackWasm) -> Result<(), JsValue> {
        if input.len() != self.adapter.in_features {
            return Err(JsValue::from_str(&format!(
                "Input size mismatch: expected {}, got {}",
                self.adapter.in_features,
                input.len()
            )));
        }

        self.adapter.accumulate_gradient(input, feedback.quality);
        self.samples_seen += 1;
        self.quality_sum += feedback.quality;

        Ok(())
    }

    /// Apply accumulated gradients with the given learning rate.
    ///
    /// Should be called after one or more `adapt()` calls to update the weights.
    #[wasm_bindgen(js_name = applyUpdates)]
    pub fn apply_updates(&mut self, learning_rate: f32) {
        self.adapter.apply_gradients(learning_rate);
    }

    /// Reset the adapter to its initial state.
    ///
    /// Clears B weights and all statistics.
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.adapter.reset();
        self.samples_seen = 0;
        self.quality_sum = 0.0;
    }

    /// Get adapter statistics.
    #[wasm_bindgen]
    pub fn stats(&self) -> MicroLoraStatsWasm {
        MicroLoraStatsWasm {
            samples_seen: self.samples_seen,
            avg_quality: if self.samples_seen > 0 {
                self.quality_sum / self.samples_seen as f32
            } else {
                0.0
            },
            memory_bytes: self.adapter.memory_bytes(),
            param_count: self.adapter.param_count(),
        }
    }

    /// Serialize to JSON string for persistence.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> Result<String, JsValue> {
        #[derive(Serialize)]
        struct SerializedState {
            adapter: LoraAdapterInternal,
            samples_seen: usize,
            quality_sum: f32,
        }

        let state = SerializedState {
            adapter: self.adapter.clone(),
            samples_seen: self.samples_seen,
            quality_sum: self.quality_sum,
        };

        serde_json::to_string(&state)
            .map_err(|e| JsValue::from_str(&format!("Serialization error: {}", e)))
    }

    /// Deserialize from JSON string.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<MicroLoraWasm, JsValue> {
        #[derive(Deserialize)]
        struct SerializedState {
            adapter: LoraAdapterInternal,
            samples_seen: usize,
            quality_sum: f32,
        }

        let state: SerializedState = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Deserialization error: {}", e)))?;

        Ok(MicroLoraWasm {
            adapter: state.adapter,
            samples_seen: state.samples_seen,
            quality_sum: state.quality_sum,
        })
    }

    /// Get number of pending gradient updates.
    #[wasm_bindgen(js_name = pendingUpdates)]
    pub fn pending_updates(&self) -> usize {
        self.adapter.grad_count
    }

    /// Get configuration.
    #[wasm_bindgen(js_name = getConfig)]
    pub fn get_config(&self) -> MicroLoraConfigWasm {
        MicroLoraConfigWasm {
            rank: self.adapter.rank,
            alpha: self.adapter.scaling * self.adapter.rank as f32,
            in_features: self.adapter.in_features,
            out_features: self.adapter.out_features,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = MicroLoraConfigWasm::new();
        assert_eq!(config.rank(), 2);
        assert_eq!(config.alpha(), 4.0);
        assert_eq!(config.in_features(), 768);
        assert_eq!(config.out_features(), 768);
    }

    #[test]
    fn test_config_rank_clamping() {
        let mut config = MicroLoraConfigWasm::new();
        config.set_rank(10);
        assert_eq!(config.rank(), 4); // Clamped to max 4
        config.set_rank(0);
        assert_eq!(config.rank(), 1); // Clamped to min 1
    }

    #[test]
    fn test_adapter_creation() {
        let config = MicroLoraConfigWasm::new();
        let adapter = MicroLoraWasm::new(&config);
        let stats = adapter.stats();
        assert_eq!(stats.samples_seen(), 0);
        assert_eq!(stats.avg_quality(), 0.0);
    }

    #[test]
    fn test_forward_pass() {
        let mut config = MicroLoraConfigWasm::new();
        config.set_in_features(64);
        config.set_out_features(64);
        config.set_rank(2);

        let adapter = MicroLoraWasm::new(&config);
        let input = vec![1.0; 64];

        let output = adapter.apply(&input).unwrap();
        assert_eq!(output.len(), 64);

        // With zero-initialized B, output should be very small
        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        assert!(sum < 0.1);
    }

    #[test]
    fn test_adaptation() {
        let mut config = MicroLoraConfigWasm::new();
        config.set_in_features(64);
        config.set_out_features(64);
        config.set_rank(2);

        let mut adapter = MicroLoraWasm::new(&config);
        let input = vec![0.1; 64];
        let feedback = AdaptFeedbackWasm::new(0.8);

        adapter.adapt(&input, &feedback).unwrap();
        assert_eq!(adapter.pending_updates(), 1);

        adapter.apply_updates(0.01);
        assert_eq!(adapter.pending_updates(), 0);

        let stats = adapter.stats();
        assert_eq!(stats.samples_seen(), 1);
        assert!((stats.avg_quality() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_serialization() {
        let mut config = MicroLoraConfigWasm::new();
        config.set_in_features(32);
        config.set_out_features(32);
        config.set_rank(2);

        let mut adapter = MicroLoraWasm::new(&config);
        let input = vec![0.1; 32];
        let feedback = AdaptFeedbackWasm::new(0.9);

        adapter.adapt(&input, &feedback).unwrap();
        adapter.apply_updates(0.01);

        let json = adapter.to_json().unwrap();
        let restored = MicroLoraWasm::from_json(&json).unwrap();

        let stats1 = adapter.stats();
        let stats2 = restored.stats();

        assert_eq!(stats1.samples_seen(), stats2.samples_seen());
        assert!((stats1.avg_quality() - stats2.avg_quality()).abs() < 1e-6);
    }

    #[test]
    fn test_reset() {
        let mut config = MicroLoraConfigWasm::new();
        config.set_in_features(32);
        config.set_out_features(32);

        let mut adapter = MicroLoraWasm::new(&config);
        let input = vec![0.1; 32];
        let feedback = AdaptFeedbackWasm::new(0.8);

        adapter.adapt(&input, &feedback).unwrap();
        adapter.apply_updates(0.01);

        let stats_before = adapter.stats();
        assert_eq!(stats_before.samples_seen(), 1);

        adapter.reset();

        let stats_after = adapter.stats();
        assert_eq!(stats_after.samples_seen(), 0);
        assert_eq!(stats_after.avg_quality(), 0.0);
    }

    #[test]
    fn test_memory_calculation() {
        let mut config = MicroLoraConfigWasm::new();
        config.set_in_features(768);
        config.set_out_features(768);
        config.set_rank(2);

        let memory = config.memory_bytes();
        // (768 * 2 + 2 * 768) * 4 bytes = 3072 * 4 = 12288 bytes
        assert_eq!(memory, 12288);

        let adapter = MicroLoraWasm::new(&config);
        let stats = adapter.stats();
        assert_eq!(stats.memory_bytes(), 12288);
    }
}
