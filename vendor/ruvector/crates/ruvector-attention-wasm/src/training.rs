use ruvector_attention::training::{Adam, AdamW, InfoNCELoss, Loss, Optimizer, SGD};
use wasm_bindgen::prelude::*;

/// InfoNCE contrastive loss for training
#[wasm_bindgen]
pub struct WasmInfoNCELoss {
    inner: InfoNCELoss,
}

#[wasm_bindgen]
impl WasmInfoNCELoss {
    /// Create a new InfoNCE loss instance
    ///
    /// # Arguments
    /// * `temperature` - Temperature parameter for softmax
    #[wasm_bindgen(constructor)]
    pub fn new(temperature: f32) -> WasmInfoNCELoss {
        Self {
            inner: InfoNCELoss::new(temperature),
        }
    }

    /// Compute InfoNCE loss
    ///
    /// # Arguments
    /// * `anchor` - Anchor embedding
    /// * `positive` - Positive example embedding
    /// * `negatives` - Array of negative example embeddings
    pub fn compute(
        &self,
        anchor: &[f32],
        positive: &[f32],
        negatives: JsValue,
    ) -> Result<f32, JsError> {
        let negatives_vec: Vec<Vec<f32>> = serde_wasm_bindgen::from_value(negatives)?;
        let negatives_refs: Vec<&[f32]> = negatives_vec.iter().map(|n| n.as_slice()).collect();

        Ok(self.inner.compute(anchor, positive, &negatives_refs))
    }
}

/// Adam optimizer
#[wasm_bindgen]
pub struct WasmAdam {
    inner: Adam,
}

#[wasm_bindgen]
impl WasmAdam {
    /// Create a new Adam optimizer
    ///
    /// # Arguments
    /// * `param_count` - Number of parameters
    /// * `learning_rate` - Learning rate
    #[wasm_bindgen(constructor)]
    pub fn new(param_count: usize, learning_rate: f32) -> WasmAdam {
        Self {
            inner: Adam::new(param_count, learning_rate),
        }
    }

    /// Perform optimization step
    ///
    /// # Arguments
    /// * `params` - Current parameter values (will be updated in-place)
    /// * `gradients` - Gradient values
    pub fn step(&mut self, params: &mut [f32], gradients: &[f32]) {
        self.inner.step(params, gradients);
    }

    /// Reset optimizer state
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get current learning rate
    #[wasm_bindgen(getter)]
    pub fn learning_rate(&self) -> f32 {
        self.inner.learning_rate()
    }

    /// Set learning rate
    #[wasm_bindgen(setter)]
    pub fn set_learning_rate(&mut self, lr: f32) {
        self.inner.set_learning_rate(lr);
    }
}

/// AdamW optimizer (Adam with decoupled weight decay)
#[wasm_bindgen]
pub struct WasmAdamW {
    inner: AdamW,
    wd: f32,
}

#[wasm_bindgen]
impl WasmAdamW {
    /// Create a new AdamW optimizer
    ///
    /// # Arguments
    /// * `param_count` - Number of parameters
    /// * `learning_rate` - Learning rate
    /// * `weight_decay` - Weight decay coefficient
    #[wasm_bindgen(constructor)]
    pub fn new(param_count: usize, learning_rate: f32, weight_decay: f32) -> WasmAdamW {
        let optimizer = AdamW::new(param_count, learning_rate).with_weight_decay(weight_decay);
        Self {
            inner: optimizer,
            wd: weight_decay,
        }
    }

    /// Perform optimization step with weight decay
    pub fn step(&mut self, params: &mut [f32], gradients: &[f32]) {
        self.inner.step(params, gradients);
    }

    /// Reset optimizer state
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get current learning rate
    #[wasm_bindgen(getter)]
    pub fn learning_rate(&self) -> f32 {
        self.inner.learning_rate()
    }

    /// Set learning rate
    #[wasm_bindgen(setter)]
    pub fn set_learning_rate(&mut self, lr: f32) {
        self.inner.set_learning_rate(lr);
    }

    /// Get weight decay
    #[wasm_bindgen(getter)]
    pub fn weight_decay(&self) -> f32 {
        self.wd
    }
}

/// SGD optimizer with momentum
#[wasm_bindgen]
pub struct WasmSGD {
    inner: SGD,
}

#[wasm_bindgen]
impl WasmSGD {
    /// Create a new SGD optimizer
    ///
    /// # Arguments
    /// * `param_count` - Number of parameters
    /// * `learning_rate` - Learning rate
    /// * `momentum` - Momentum coefficient (default: 0)
    #[wasm_bindgen(constructor)]
    pub fn new(param_count: usize, learning_rate: f32, momentum: Option<f32>) -> WasmSGD {
        let mut optimizer = SGD::new(param_count, learning_rate);
        if let Some(m) = momentum {
            optimizer = optimizer.with_momentum(m);
        }
        Self { inner: optimizer }
    }

    /// Perform optimization step
    pub fn step(&mut self, params: &mut [f32], gradients: &[f32]) {
        self.inner.step(params, gradients);
    }

    /// Reset optimizer state
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Get current learning rate
    #[wasm_bindgen(getter)]
    pub fn learning_rate(&self) -> f32 {
        self.inner.learning_rate()
    }

    /// Set learning rate
    #[wasm_bindgen(setter)]
    pub fn set_learning_rate(&mut self, lr: f32) {
        self.inner.set_learning_rate(lr);
    }
}

/// Learning rate scheduler
#[wasm_bindgen]
pub struct WasmLRScheduler {
    initial_lr: f32,
    current_step: usize,
    warmup_steps: usize,
    total_steps: usize,
}

#[wasm_bindgen]
impl WasmLRScheduler {
    /// Create a new learning rate scheduler with warmup and cosine decay
    ///
    /// # Arguments
    /// * `initial_lr` - Initial learning rate
    /// * `warmup_steps` - Number of warmup steps
    /// * `total_steps` - Total training steps
    #[wasm_bindgen(constructor)]
    pub fn new(initial_lr: f32, warmup_steps: usize, total_steps: usize) -> WasmLRScheduler {
        Self {
            initial_lr,
            current_step: 0,
            warmup_steps,
            total_steps,
        }
    }

    /// Get learning rate for current step
    pub fn get_lr(&self) -> f32 {
        if self.current_step < self.warmup_steps {
            // Linear warmup
            self.initial_lr * (self.current_step as f32 / self.warmup_steps as f32)
        } else {
            // Cosine decay
            let progress = (self.current_step - self.warmup_steps) as f32
                / (self.total_steps - self.warmup_steps) as f32;
            let cosine = 0.5 * (1.0 + (std::f32::consts::PI * progress).cos());
            self.initial_lr * cosine
        }
    }

    /// Advance to next step
    pub fn step(&mut self) {
        self.current_step += 1;
    }

    /// Reset scheduler
    pub fn reset(&mut self) {
        self.current_step = 0;
    }
}
