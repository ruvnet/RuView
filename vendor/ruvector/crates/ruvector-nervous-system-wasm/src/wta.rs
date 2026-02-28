//! Winner-Take-All (WTA) WASM bindings
//!
//! Instant decisions via neural competition:
//! - Single winner: <1us for 1000 neurons
//! - K-WTA: <10us for k=50

use wasm_bindgen::prelude::*;

/// Winner-Take-All competition layer
///
/// Implements neural competition where the highest-activation neuron
/// wins and suppresses others through lateral inhibition.
///
/// # Performance
/// - <1us winner selection for 1000 neurons
#[wasm_bindgen]
pub struct WTALayer {
    membranes: Vec<f32>,
    threshold: f32,
    inhibition_strength: f32,
    refractory_period: u32,
    refractory_counters: Vec<u32>,
}

#[wasm_bindgen]
impl WTALayer {
    /// Create a new WTA layer
    ///
    /// # Arguments
    /// * `size` - Number of competing neurons
    /// * `threshold` - Activation threshold for firing
    /// * `inhibition` - Lateral inhibition strength (0.0-1.0)
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize, threshold: f32, inhibition: f32) -> Result<WTALayer, JsValue> {
        if size == 0 {
            return Err(JsValue::from_str("Size must be > 0"));
        }

        Ok(Self {
            membranes: vec![0.0; size],
            threshold,
            inhibition_strength: inhibition.clamp(0.0, 1.0),
            refractory_period: 10,
            refractory_counters: vec![0; size],
        })
    }

    /// Run winner-take-all competition
    ///
    /// Returns the index of the winning neuron, or -1 if no neuron exceeds threshold.
    #[wasm_bindgen]
    pub fn compete(&mut self, inputs: &[f32]) -> Result<i32, JsValue> {
        if inputs.len() != self.membranes.len() {
            return Err(JsValue::from_str(&format!(
                "Input size mismatch: expected {}, got {}",
                self.membranes.len(),
                inputs.len()
            )));
        }

        // Single-pass: update membrane potentials and find max
        let mut best_idx: Option<usize> = None;
        let mut best_val = f32::NEG_INFINITY;

        for (i, &input) in inputs.iter().enumerate() {
            if self.refractory_counters[i] == 0 {
                self.membranes[i] = input;
                if input > best_val {
                    best_val = input;
                    best_idx = Some(i);
                }
            } else {
                self.refractory_counters[i] = self.refractory_counters[i].saturating_sub(1);
            }
        }

        let winner_idx = match best_idx {
            Some(idx) => idx,
            None => return Ok(-1),
        };

        // Check if winner exceeds threshold
        if best_val < self.threshold {
            return Ok(-1);
        }

        // Apply lateral inhibition
        for (i, membrane) in self.membranes.iter_mut().enumerate() {
            if i != winner_idx {
                *membrane *= 1.0 - self.inhibition_strength;
            }
        }

        // Set refractory period for winner
        self.refractory_counters[winner_idx] = self.refractory_period;

        Ok(winner_idx as i32)
    }

    /// Soft competition with normalized activations
    ///
    /// Returns activation levels for all neurons after softmax-like normalization.
    #[wasm_bindgen]
    pub fn compete_soft(&mut self, inputs: &[f32]) -> Result<js_sys::Float32Array, JsValue> {
        if inputs.len() != self.membranes.len() {
            return Err(JsValue::from_str(&format!(
                "Input size mismatch: expected {}, got {}",
                self.membranes.len(),
                inputs.len()
            )));
        }

        // Update membrane potentials
        self.membranes.copy_from_slice(inputs);

        // Find max for numerical stability
        let max_val = self
            .membranes
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        // Softmax with temperature
        let temperature = 1.0 / (1.0 + self.inhibition_strength);
        let mut activations: Vec<f32> = self
            .membranes
            .iter()
            .map(|&x| ((x - max_val) / temperature).exp())
            .collect();

        // Normalize
        let sum: f32 = activations.iter().sum();
        if sum > 0.0 {
            for a in &mut activations {
                *a /= sum;
            }
        }

        Ok(js_sys::Float32Array::from(activations.as_slice()))
    }

    /// Reset layer state
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.membranes.fill(0.0);
        self.refractory_counters.fill(0);
    }

    /// Get current membrane potentials
    #[wasm_bindgen]
    pub fn get_membranes(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.membranes.as_slice())
    }

    /// Set refractory period
    #[wasm_bindgen]
    pub fn set_refractory_period(&mut self, period: u32) {
        self.refractory_period = period;
    }

    /// Get layer size
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> usize {
        self.membranes.len()
    }
}

/// K-Winner-Take-All layer for sparse distributed coding
///
/// Selects top-k neurons with highest activations.
///
/// # Performance
/// - O(n + k log k) using partial sorting
/// - <10us for 1000 neurons, k=50
#[wasm_bindgen]
pub struct KWTALayer {
    size: usize,
    k: usize,
    threshold: Option<f32>,
}

#[wasm_bindgen]
impl KWTALayer {
    /// Create a new K-WTA layer
    ///
    /// # Arguments
    /// * `size` - Total number of neurons
    /// * `k` - Number of winners to select
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize, k: usize) -> Result<KWTALayer, JsValue> {
        if k == 0 {
            return Err(JsValue::from_str("k must be > 0"));
        }
        if k > size {
            return Err(JsValue::from_str("k cannot exceed layer size"));
        }

        Ok(Self {
            size,
            k,
            threshold: None,
        })
    }

    /// Set activation threshold
    #[wasm_bindgen]
    pub fn with_threshold(&mut self, threshold: f32) {
        self.threshold = Some(threshold);
    }

    /// Select top-k neurons
    ///
    /// Returns indices of k neurons with highest activations, sorted descending.
    #[wasm_bindgen]
    pub fn select(&self, inputs: &[f32]) -> Result<js_sys::Uint32Array, JsValue> {
        if inputs.len() != self.size {
            return Err(JsValue::from_str(&format!(
                "Input size mismatch: expected {}, got {}",
                self.size,
                inputs.len()
            )));
        }

        // Create (index, value) pairs
        let mut indexed: Vec<(usize, f32)> =
            inputs.iter().enumerate().map(|(i, &v)| (i, v)).collect();

        // Filter by threshold if set
        if let Some(threshold) = self.threshold {
            indexed.retain(|(_, v)| *v >= threshold);
        }

        if indexed.is_empty() {
            return Ok(js_sys::Uint32Array::new_with_length(0));
        }

        // Partial sort to get top-k
        let k_actual = self.k.min(indexed.len());
        indexed.select_nth_unstable_by(k_actual - 1, |a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Take top k and sort descending
        let mut winners: Vec<(usize, f32)> = indexed[..k_actual].to_vec();
        winners.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Return only indices as u32
        let indices: Vec<u32> = winners.into_iter().map(|(i, _)| i as u32).collect();
        Ok(js_sys::Uint32Array::from(indices.as_slice()))
    }

    /// Select top-k neurons with their activation values
    ///
    /// Returns array of [index, value] pairs.
    #[wasm_bindgen]
    pub fn select_with_values(&self, inputs: &[f32]) -> Result<JsValue, JsValue> {
        if inputs.len() != self.size {
            return Err(JsValue::from_str(&format!(
                "Input size mismatch: expected {}, got {}",
                self.size,
                inputs.len()
            )));
        }

        let mut indexed: Vec<(usize, f32)> =
            inputs.iter().enumerate().map(|(i, &v)| (i, v)).collect();

        if let Some(threshold) = self.threshold {
            indexed.retain(|(_, v)| *v >= threshold);
        }

        if indexed.is_empty() {
            return serde_wasm_bindgen::to_value(&Vec::<(usize, f32)>::new())
                .map_err(|e| JsValue::from_str(&e.to_string()));
        }

        let k_actual = self.k.min(indexed.len());
        indexed.select_nth_unstable_by(k_actual - 1, |a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut winners: Vec<(usize, f32)> = indexed[..k_actual].to_vec();
        winners.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        serde_wasm_bindgen::to_value(&winners).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Create sparse activation vector (only top-k preserved)
    #[wasm_bindgen]
    pub fn sparse_activations(&self, inputs: &[f32]) -> Result<js_sys::Float32Array, JsValue> {
        if inputs.len() != self.size {
            return Err(JsValue::from_str(&format!(
                "Input size mismatch: expected {}, got {}",
                self.size,
                inputs.len()
            )));
        }

        let mut indexed: Vec<(usize, f32)> =
            inputs.iter().enumerate().map(|(i, &v)| (i, v)).collect();

        if let Some(threshold) = self.threshold {
            indexed.retain(|(_, v)| *v >= threshold);
        }

        let mut sparse = vec![0.0; self.size];

        if !indexed.is_empty() {
            let k_actual = self.k.min(indexed.len());
            indexed.select_nth_unstable_by(k_actual - 1, |a, b| {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            });

            for (idx, value) in &indexed[..k_actual] {
                sparse[*idx] = *value;
            }
        }

        Ok(js_sys::Float32Array::from(sparse.as_slice()))
    }

    /// Get number of winners
    #[wasm_bindgen(getter)]
    pub fn k(&self) -> usize {
        self.k
    }

    /// Get layer size
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> usize {
        self.size
    }
}
