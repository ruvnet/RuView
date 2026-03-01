//! Per-Operator-Type Scoped LoRA
//!
//! Maintains separate LoRA adapters for different operator types,
//! enabling specialized learning for each query operator.

use crate::lora::{LoRAConfig, LoRAPair};
use wasm_bindgen::prelude::*;

/// Operator types for scoping (matches ruvector-dag OperatorType)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OperatorScope {
    // Scan operators (0-3)
    SeqScan = 0,
    IndexScan = 1,
    HnswScan = 2,
    IvfFlatScan = 3,

    // Join operators (4-6)
    NestedLoopJoin = 4,
    HashJoin = 5,
    MergeJoin = 6,

    // Aggregation (7-8)
    Aggregate = 7,
    GroupBy = 8,

    // Filter/Project (9-10)
    Filter = 9,
    Project = 10,

    // Sort/Limit (11-12)
    Sort = 11,
    Limit = 12,

    // Vector operations (13-14)
    VectorDistance = 13,
    Rerank = 14,

    // Utility (15-16)
    Materialize = 15,
    Result = 16,
}

impl OperatorScope {
    /// Convert from u8
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::SeqScan),
            1 => Some(Self::IndexScan),
            2 => Some(Self::HnswScan),
            3 => Some(Self::IvfFlatScan),
            4 => Some(Self::NestedLoopJoin),
            5 => Some(Self::HashJoin),
            6 => Some(Self::MergeJoin),
            7 => Some(Self::Aggregate),
            8 => Some(Self::GroupBy),
            9 => Some(Self::Filter),
            10 => Some(Self::Project),
            11 => Some(Self::Sort),
            12 => Some(Self::Limit),
            13 => Some(Self::VectorDistance),
            14 => Some(Self::Rerank),
            15 => Some(Self::Materialize),
            16 => Some(Self::Result),
            _ => None,
        }
    }

    /// Get category for grouped learning
    pub fn category(&self) -> OperatorCategory {
        match self {
            Self::SeqScan | Self::IndexScan | Self::HnswScan | Self::IvfFlatScan => {
                OperatorCategory::Scan
            }
            Self::NestedLoopJoin | Self::HashJoin | Self::MergeJoin => OperatorCategory::Join,
            Self::Aggregate | Self::GroupBy => OperatorCategory::Aggregation,
            Self::Filter | Self::Project => OperatorCategory::Transform,
            Self::Sort | Self::Limit => OperatorCategory::Order,
            Self::VectorDistance | Self::Rerank => OperatorCategory::Vector,
            Self::Materialize | Self::Result => OperatorCategory::Utility,
        }
    }
}

/// High-level operator categories for shared learning
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OperatorCategory {
    Scan = 0,
    Join = 1,
    Aggregation = 2,
    Transform = 3,
    Order = 4,
    Vector = 5,
    Utility = 6,
}

/// Scoped LoRA manager with per-operator-type adapters
///
/// Maintains 17 separate LoRA pairs (one per OperatorScope) for
/// specialized learning based on query operator type.
pub struct ScopedLoRA {
    /// Per-operator-type LoRA pairs (17 total)
    adapters: [LoRAPair; 17],
    /// Per-category LoRA pairs for fallback (7 total)
    category_adapters: [LoRAPair; 7],
    /// Configuration (kept for potential future use)
    #[allow(dead_code)]
    config: LoRAConfig,
    /// Whether to use category fallback when operator has no history
    use_category_fallback: bool,
    /// Per-operator forward counts
    forward_counts: [u64; 17],
}

impl ScopedLoRA {
    /// Create a new scoped LoRA manager
    pub fn new(config: LoRAConfig) -> Self {
        // Initialize all 17 operator adapters
        let adapters = std::array::from_fn(|_| LoRAPair::new(&config));
        let category_adapters = std::array::from_fn(|_| LoRAPair::new(&config));

        Self {
            adapters,
            category_adapters,
            config,
            use_category_fallback: true,
            forward_counts: [0; 17],
        }
    }

    /// Forward pass for a specific operator type
    #[inline]
    pub fn forward(&mut self, scope: OperatorScope, input: &[f32]) -> Vec<f32> {
        let idx = scope as usize;
        self.forward_counts[idx] += 1;

        // Use operator-specific adapter
        let output = self.adapters[idx].forward(input);

        // If using fallback and this operator has little history,
        // blend with category adapter
        if self.use_category_fallback && self.adapters[idx].adapt_count() < 10 {
            let cat_idx = scope.category() as usize;
            let cat_output = self.category_adapters[cat_idx].forward(input);

            // Blend based on relative experience
            let op_exp = self.adapters[idx].adapt_count() as f32;
            let weight = (op_exp / 10.0).min(1.0);

            let mut blended = output;
            for i in 0..blended.len().min(cat_output.len()) {
                blended[i] = blended[i] * weight + cat_output[i] * (1.0 - weight);
            }
            return blended;
        }

        output
    }

    /// Adapt the adapter for a specific operator type
    #[inline]
    pub fn adapt(&mut self, scope: OperatorScope, gradient: &[f32]) {
        let idx = scope as usize;
        self.adapters[idx].adapt(gradient);

        // Also update category adapter for transfer learning
        let cat_idx = scope.category() as usize;
        self.category_adapters[cat_idx].adapt(gradient);
    }

    /// Adapt with improvement reward
    #[inline]
    pub fn adapt_with_reward(&mut self, scope: OperatorScope, gradient: &[f32], improvement: f32) {
        let idx = scope as usize;
        self.adapters[idx].adapt_with_reward(gradient, improvement);

        // Also update category adapter
        let cat_idx = scope.category() as usize;
        self.category_adapters[cat_idx].adapt_with_reward(gradient, improvement);
    }

    /// Reset a specific operator's adapter
    pub fn reset_scope(&mut self, scope: OperatorScope) {
        let idx = scope as usize;
        self.adapters[idx].reset();
        self.forward_counts[idx] = 0;
    }

    /// Reset all adapters
    pub fn reset_all(&mut self) {
        for adapter in &mut self.adapters {
            adapter.reset();
        }
        for adapter in &mut self.category_adapters {
            adapter.reset();
        }
        self.forward_counts = [0; 17];
    }

    /// Get statistics for a specific operator
    pub fn stats(&self, scope: OperatorScope) -> (u64, u64, f32) {
        let idx = scope as usize;
        (
            self.forward_counts[idx],
            self.adapters[idx].adapt_count(),
            self.adapters[idx].delta_norm(),
        )
    }

    /// Get total statistics across all operators
    pub fn total_stats(&self) -> (u64, u64, f32) {
        let total_forwards: u64 = self.forward_counts.iter().sum();
        let total_adapts: u64 = self.adapters.iter().map(|a| a.adapt_count()).sum();
        let max_delta: f32 = self
            .adapters
            .iter()
            .map(|a| a.delta_norm())
            .fold(0.0, f32::max);

        (total_forwards, total_adapts, max_delta)
    }

    /// Get the most active operator scopes
    pub fn most_active(&self, top_n: usize) -> Vec<(OperatorScope, u64)> {
        let mut counts: Vec<(usize, u64)> = self
            .forward_counts
            .iter()
            .enumerate()
            .map(|(i, &c)| (i, c))
            .collect();

        counts.sort_by(|a, b| b.1.cmp(&a.1));

        counts
            .into_iter()
            .take(top_n)
            .filter_map(|(idx, count)| {
                OperatorScope::from_u8(idx as u8).map(|scope| (scope, count))
            })
            .collect()
    }

    /// Set category fallback mode
    pub fn set_category_fallback(&mut self, enabled: bool) {
        self.use_category_fallback = enabled;
    }

    /// Get reference to operator adapter
    pub fn adapter(&self, scope: OperatorScope) -> &LoRAPair {
        &self.adapters[scope as usize]
    }

    /// Get mutable reference to operator adapter
    pub fn adapter_mut(&mut self, scope: OperatorScope) -> &mut LoRAPair {
        &mut self.adapters[scope as usize]
    }
}

impl Default for ScopedLoRA {
    fn default() -> Self {
        Self::new(LoRAConfig::default())
    }
}

// ============ WASM Bindings ============

pub mod wasm_exports {
    use super::*;
    #[allow(unused_imports)]
    use wasm_bindgen::prelude::*;

    /// WASM-exposed Scoped LoRA manager
    #[wasm_bindgen]
    pub struct WasmScopedLoRA {
        inner: ScopedLoRA,
        input_buffer: Vec<f32>,
        output_buffer: Vec<f32>,
    }

    #[wasm_bindgen]
    impl WasmScopedLoRA {
        /// Create a new scoped LoRA manager
        ///
        /// @param dim - Embedding dimension (max 256)
        /// @param alpha - Scaling factor (default 0.1)
        /// @param learning_rate - Learning rate (default 0.01)
        #[wasm_bindgen(constructor)]
        pub fn new(dim: Option<usize>, alpha: Option<f32>, learning_rate: Option<f32>) -> Self {
            let config = LoRAConfig {
                dim: dim.unwrap_or(256).min(256),
                rank: 2,
                alpha: alpha.unwrap_or(0.1),
                learning_rate: learning_rate.unwrap_or(0.01),
                dropout: 0.0,
            };

            let actual_dim = config.dim;
            Self {
                inner: ScopedLoRA::new(config),
                input_buffer: vec![0.0; actual_dim],
                output_buffer: vec![0.0; actual_dim],
            }
        }

        /// Get input buffer pointer
        #[wasm_bindgen]
        pub fn get_input_ptr(&mut self) -> *mut f32 {
            self.input_buffer.as_mut_ptr()
        }

        /// Get output buffer pointer
        #[wasm_bindgen]
        pub fn get_output_ptr(&self) -> *const f32 {
            self.output_buffer.as_ptr()
        }

        /// Forward pass for operator type (uses internal buffers)
        ///
        /// @param op_type - Operator type (0-16)
        #[wasm_bindgen]
        pub fn forward(&mut self, op_type: u8) {
            if let Some(scope) = OperatorScope::from_u8(op_type) {
                let output = self.inner.forward(scope, &self.input_buffer);
                let n = output.len().min(self.output_buffer.len());
                self.output_buffer[..n].copy_from_slice(&output[..n]);
            }
        }

        /// Forward pass with typed array
        #[wasm_bindgen]
        pub fn forward_array(&mut self, op_type: u8, input: &[f32]) -> Vec<f32> {
            if let Some(scope) = OperatorScope::from_u8(op_type) {
                self.inner.forward(scope, input)
            } else {
                input.to_vec()
            }
        }

        /// Adapt for operator type using input buffer as gradient
        #[wasm_bindgen]
        pub fn adapt(&mut self, op_type: u8) {
            if let Some(scope) = OperatorScope::from_u8(op_type) {
                self.inner.adapt(scope, &self.input_buffer.clone());
            }
        }

        /// Adapt with typed array
        #[wasm_bindgen]
        pub fn adapt_array(&mut self, op_type: u8, gradient: &[f32]) {
            if let Some(scope) = OperatorScope::from_u8(op_type) {
                self.inner.adapt(scope, gradient);
            }
        }

        /// Adapt with improvement reward
        #[wasm_bindgen]
        pub fn adapt_with_reward(&mut self, op_type: u8, improvement: f32) {
            if let Some(scope) = OperatorScope::from_u8(op_type) {
                self.inner
                    .adapt_with_reward(scope, &self.input_buffer.clone(), improvement);
            }
        }

        /// Reset specific operator adapter
        #[wasm_bindgen]
        pub fn reset_scope(&mut self, op_type: u8) {
            if let Some(scope) = OperatorScope::from_u8(op_type) {
                self.inner.reset_scope(scope);
            }
        }

        /// Reset all adapters
        #[wasm_bindgen]
        pub fn reset_all(&mut self) {
            self.inner.reset_all();
        }

        /// Get forward count for operator
        #[wasm_bindgen]
        pub fn forward_count(&self, op_type: u8) -> u64 {
            if let Some(scope) = OperatorScope::from_u8(op_type) {
                self.inner.stats(scope).0
            } else {
                0
            }
        }

        /// Get adapt count for operator
        #[wasm_bindgen]
        pub fn adapt_count(&self, op_type: u8) -> u64 {
            if let Some(scope) = OperatorScope::from_u8(op_type) {
                self.inner.stats(scope).1
            } else {
                0
            }
        }

        /// Get delta norm for operator
        #[wasm_bindgen]
        pub fn delta_norm(&self, op_type: u8) -> f32 {
            if let Some(scope) = OperatorScope::from_u8(op_type) {
                self.inner.stats(scope).2
            } else {
                0.0
            }
        }

        /// Get total forward count
        #[wasm_bindgen]
        pub fn total_forward_count(&self) -> u64 {
            self.inner.total_stats().0
        }

        /// Get total adapt count
        #[wasm_bindgen]
        pub fn total_adapt_count(&self) -> u64 {
            self.inner.total_stats().1
        }

        /// Enable/disable category fallback
        #[wasm_bindgen]
        pub fn set_category_fallback(&mut self, enabled: bool) {
            self.inner.set_category_fallback(enabled);
        }

        /// Get operator scope name
        #[wasm_bindgen]
        pub fn scope_name(op_type: u8) -> String {
            match op_type {
                0 => "SeqScan".to_string(),
                1 => "IndexScan".to_string(),
                2 => "HnswScan".to_string(),
                3 => "IvfFlatScan".to_string(),
                4 => "NestedLoopJoin".to_string(),
                5 => "HashJoin".to_string(),
                6 => "MergeJoin".to_string(),
                7 => "Aggregate".to_string(),
                8 => "GroupBy".to_string(),
                9 => "Filter".to_string(),
                10 => "Project".to_string(),
                11 => "Sort".to_string(),
                12 => "Limit".to_string(),
                13 => "VectorDistance".to_string(),
                14 => "Rerank".to_string(),
                15 => "Materialize".to_string(),
                16 => "Result".to_string(),
                _ => "Unknown".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scoped_lora_creation() {
        let lora = ScopedLoRA::default();
        let (forwards, adapts, delta) = lora.total_stats();
        assert_eq!(forwards, 0);
        assert_eq!(adapts, 0);
        assert_eq!(delta, 0.0);
    }

    #[test]
    fn test_scoped_forward() {
        let mut lora = ScopedLoRA::default();
        let input = vec![1.0; 256];

        let output = lora.forward(OperatorScope::HnswScan, &input);
        assert_eq!(output.len(), 256);

        let (forwards, _, _) = lora.stats(OperatorScope::HnswScan);
        assert_eq!(forwards, 1);
    }

    #[test]
    fn test_scoped_adapt() {
        let mut lora = ScopedLoRA::default();
        let gradient = vec![0.1; 256];

        lora.adapt(OperatorScope::Filter, &gradient);

        let (_, adapts, delta) = lora.stats(OperatorScope::Filter);
        assert_eq!(adapts, 1);
        assert!(delta > 0.0);
    }

    #[test]
    fn test_category_transfer() {
        let mut lora = ScopedLoRA::default();
        let gradient = vec![0.1; 256];

        // Adapt HnswScan (category: Scan)
        lora.adapt(OperatorScope::HnswScan, &gradient);

        // SeqScan should benefit from category adapter via fallback
        let input = vec![1.0; 256];
        let output = lora.forward(OperatorScope::SeqScan, &input);

        // With fallback enabled and SeqScan having no history,
        // it should use the category adapter which was updated
        // This is a behavioral test - output should differ from input
        let mut diff = 0.0f32;
        for i in 0..256 {
            diff += (output[i] - input[i]).abs();
        }
        // Due to category transfer, there should be some difference
        assert!(diff > 0.0, "Category transfer should affect output");
    }

    #[test]
    fn test_operator_scope_conversion() {
        for i in 0..=16u8 {
            let scope = OperatorScope::from_u8(i);
            assert!(scope.is_some(), "Scope {} should be valid", i);
        }
        assert!(OperatorScope::from_u8(17).is_none());
    }
}
