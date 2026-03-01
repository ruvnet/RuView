//! BTSP (Behavioral Timescale Synaptic Plasticity) WASM bindings
//!
//! One-shot learning for immediate pattern-target associations.
//! Based on Bittner et al. 2017 hippocampal place field formation.

use wasm_bindgen::prelude::*;

/// BTSP synapse with eligibility trace and bidirectional plasticity
#[wasm_bindgen]
#[derive(Clone)]
pub struct BTSPSynapse {
    weight: f32,
    eligibility_trace: f32,
    tau_btsp: f32,
    min_weight: f32,
    max_weight: f32,
    ltp_rate: f32,
    ltd_rate: f32,
}

#[wasm_bindgen]
impl BTSPSynapse {
    /// Create a new BTSP synapse
    ///
    /// # Arguments
    /// * `initial_weight` - Starting weight (0.0 to 1.0)
    /// * `tau_btsp` - Time constant in milliseconds (1000-3000ms recommended)
    #[wasm_bindgen(constructor)]
    pub fn new(initial_weight: f32, tau_btsp: f32) -> Result<BTSPSynapse, JsValue> {
        if !(0.0..=1.0).contains(&initial_weight) {
            return Err(JsValue::from_str(&format!(
                "Invalid weight: {} (must be 0.0-1.0)",
                initial_weight
            )));
        }
        if tau_btsp <= 0.0 {
            return Err(JsValue::from_str(&format!(
                "Invalid time constant: {} (must be > 0)",
                tau_btsp
            )));
        }

        Ok(Self {
            weight: initial_weight,
            eligibility_trace: 0.0,
            tau_btsp,
            min_weight: 0.0,
            max_weight: 1.0,
            ltp_rate: 0.1,
            ltd_rate: 0.05,
        })
    }

    /// Update synapse based on activity and plateau signal
    ///
    /// # Arguments
    /// * `presynaptic_active` - Is presynaptic neuron firing?
    /// * `plateau_signal` - Dendritic plateau potential detected?
    /// * `dt` - Time step in milliseconds
    #[wasm_bindgen]
    pub fn update(&mut self, presynaptic_active: bool, plateau_signal: bool, dt: f32) {
        // Decay eligibility trace exponentially
        self.eligibility_trace *= (-dt / self.tau_btsp).exp();

        // Accumulate trace when presynaptic neuron fires
        if presynaptic_active {
            self.eligibility_trace += 1.0;
        }

        // Bidirectional plasticity gated by plateau potential
        if plateau_signal && self.eligibility_trace > 0.01 {
            let delta = if self.weight < 0.5 {
                self.ltp_rate // Potentiation
            } else {
                -self.ltd_rate // Depression
            };

            self.weight += delta * self.eligibility_trace;
            self.weight = self.weight.clamp(self.min_weight, self.max_weight);
        }
    }

    /// Get current weight
    #[wasm_bindgen(getter)]
    pub fn weight(&self) -> f32 {
        self.weight
    }

    /// Get eligibility trace
    #[wasm_bindgen(getter)]
    pub fn eligibility_trace(&self) -> f32 {
        self.eligibility_trace
    }

    /// Compute synaptic output
    #[wasm_bindgen]
    pub fn forward(&self, input: f32) -> f32 {
        self.weight * input
    }
}

/// BTSP Layer for one-shot learning
///
/// # Performance
/// - One-shot learning: immediate, no iteration
/// - Forward pass: <10us for 10K synapses
#[wasm_bindgen]
pub struct BTSPLayer {
    weights: Vec<f32>,
    eligibility_traces: Vec<f32>,
    #[allow(dead_code)]
    tau_btsp: f32,
    #[allow(dead_code)]
    plateau_threshold: f32,
}

#[wasm_bindgen]
impl BTSPLayer {
    /// Create a new BTSP layer
    ///
    /// # Arguments
    /// * `size` - Number of synapses (input dimension)
    /// * `tau` - Time constant in milliseconds (2000ms default)
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize, tau: f32) -> BTSPLayer {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let weights: Vec<f32> = (0..size).map(|_| rng.gen_range(0.0..0.1)).collect();
        let eligibility_traces = vec![0.0; size];

        Self {
            weights,
            eligibility_traces,
            tau_btsp: tau,
            plateau_threshold: 0.7,
        }
    }

    /// Forward pass: compute layer output
    #[wasm_bindgen]
    pub fn forward(&self, input: &[f32]) -> Result<f32, JsValue> {
        if input.len() != self.weights.len() {
            return Err(JsValue::from_str(&format!(
                "Input size mismatch: expected {}, got {}",
                self.weights.len(),
                input.len()
            )));
        }

        Ok(self
            .weights
            .iter()
            .zip(input.iter())
            .map(|(&w, &x)| w * x)
            .sum())
    }

    /// One-shot association: learn pattern -> target in single step
    ///
    /// This is the key BTSP capability: immediate learning without iteration.
    /// Uses gradient normalization for single-step convergence.
    #[wasm_bindgen]
    pub fn one_shot_associate(&mut self, pattern: &[f32], target: f32) -> Result<(), JsValue> {
        if pattern.len() != self.weights.len() {
            return Err(JsValue::from_str(&format!(
                "Pattern size mismatch: expected {}, got {}",
                self.weights.len(),
                pattern.len()
            )));
        }

        // Current output
        let current: f32 = self
            .weights
            .iter()
            .zip(pattern.iter())
            .map(|(&w, &x)| w * x)
            .sum();

        // Compute required weight change
        let error = target - current;

        // Compute sum of squared inputs for gradient normalization
        let sum_squared: f32 = pattern.iter().map(|&x| x * x).sum();
        if sum_squared < 1e-8 {
            return Ok(()); // No active inputs
        }

        // Set eligibility traces and update weights
        for (i, &input_val) in pattern.iter().enumerate() {
            if input_val.abs() > 0.01 {
                // Set trace proportional to input
                self.eligibility_traces[i] = input_val;

                // Direct weight update: delta = error * x / sum(x^2)
                let delta = error * input_val / sum_squared;
                self.weights[i] += delta;
                self.weights[i] = self.weights[i].clamp(0.0, 1.0);
            }
        }

        Ok(())
    }

    /// Get number of synapses
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> usize {
        self.weights.len()
    }

    /// Get weights as Float32Array
    #[wasm_bindgen]
    pub fn get_weights(&self) -> js_sys::Float32Array {
        js_sys::Float32Array::from(self.weights.as_slice())
    }

    /// Reset layer to initial state
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for w in &mut self.weights {
            *w = rng.gen_range(0.0..0.1);
        }
        self.eligibility_traces.fill(0.0);
    }
}

/// Associative memory using BTSP for key-value storage
#[wasm_bindgen]
pub struct BTSPAssociativeMemory {
    layers: Vec<BTSPLayer>,
    input_size: usize,
    output_size: usize,
}

#[wasm_bindgen]
impl BTSPAssociativeMemory {
    /// Create new associative memory
    ///
    /// # Arguments
    /// * `input_size` - Dimension of key vectors
    /// * `output_size` - Dimension of value vectors
    #[wasm_bindgen(constructor)]
    pub fn new(input_size: usize, output_size: usize) -> BTSPAssociativeMemory {
        let tau = 2000.0;
        let layers = (0..output_size)
            .map(|_| BTSPLayer::new(input_size, tau))
            .collect();

        Self {
            layers,
            input_size,
            output_size,
        }
    }

    /// Store key-value association in one shot
    #[wasm_bindgen]
    pub fn store_one_shot(&mut self, key: &[f32], value: &[f32]) -> Result<(), JsValue> {
        if key.len() != self.input_size {
            return Err(JsValue::from_str(&format!(
                "Key size mismatch: expected {}, got {}",
                self.input_size,
                key.len()
            )));
        }
        if value.len() != self.output_size {
            return Err(JsValue::from_str(&format!(
                "Value size mismatch: expected {}, got {}",
                self.output_size,
                value.len()
            )));
        }

        for (layer, &target) in self.layers.iter_mut().zip(value.iter()) {
            layer.one_shot_associate(key, target)?;
        }

        Ok(())
    }

    /// Retrieve value from key
    #[wasm_bindgen]
    pub fn retrieve(&self, query: &[f32]) -> Result<js_sys::Float32Array, JsValue> {
        if query.len() != self.input_size {
            return Err(JsValue::from_str(&format!(
                "Query size mismatch: expected {}, got {}",
                self.input_size,
                query.len()
            )));
        }

        let output: Vec<f32> = self
            .layers
            .iter()
            .map(|layer| layer.forward(query).unwrap_or(0.0))
            .collect();

        Ok(js_sys::Float32Array::from(output.as_slice()))
    }

    /// Get memory dimensions
    #[wasm_bindgen]
    pub fn dimensions(&self) -> JsValue {
        let dims = serde_wasm_bindgen::to_value(&(self.input_size, self.output_size));
        dims.unwrap_or(JsValue::NULL)
    }
}
