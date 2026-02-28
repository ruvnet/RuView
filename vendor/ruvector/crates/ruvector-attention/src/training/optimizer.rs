//! Optimizers for attention training
//!
//! Provides standard optimizers with momentum and adaptive learning rates.

/// Optimizer trait for parameter updates
pub trait Optimizer: Send + Sync {
    /// Update parameters using gradients
    fn step(&mut self, params: &mut [f32], gradients: &[f32]);

    /// Reset optimizer state
    fn reset(&mut self);

    /// Get current learning rate
    fn learning_rate(&self) -> f32;

    /// Set learning rate
    fn set_learning_rate(&mut self, lr: f32);
}

/// Stochastic Gradient Descent with momentum
pub struct SGD {
    lr: f32,
    momentum: f32,
    weight_decay: f32,
    velocity: Vec<f32>,
    nesterov: bool,
}

impl SGD {
    pub fn new(dim: usize, lr: f32) -> Self {
        Self {
            lr,
            momentum: 0.0,
            weight_decay: 0.0,
            velocity: vec![0.0; dim],
            nesterov: false,
        }
    }

    pub fn with_momentum(mut self, momentum: f32) -> Self {
        self.momentum = momentum;
        self
    }

    pub fn with_weight_decay(mut self, wd: f32) -> Self {
        self.weight_decay = wd;
        self
    }

    pub fn with_nesterov(mut self, nesterov: bool) -> Self {
        self.nesterov = nesterov;
        self
    }
}

impl Optimizer for SGD {
    fn step(&mut self, params: &mut [f32], gradients: &[f32]) {
        if self.velocity.len() != params.len() {
            self.velocity = vec![0.0; params.len()];
        }

        for i in 0..params.len() {
            let mut g = gradients[i];

            // Weight decay
            if self.weight_decay > 0.0 {
                g += self.weight_decay * params[i];
            }

            // Update velocity
            self.velocity[i] = self.momentum * self.velocity[i] + g;

            // Update parameters
            if self.nesterov {
                params[i] -= self.lr * (g + self.momentum * self.velocity[i]);
            } else {
                params[i] -= self.lr * self.velocity[i];
            }
        }
    }

    fn reset(&mut self) {
        self.velocity.fill(0.0);
    }

    fn learning_rate(&self) -> f32 {
        self.lr
    }

    fn set_learning_rate(&mut self, lr: f32) {
        self.lr = lr;
    }
}

/// Adam optimizer with bias correction
pub struct Adam {
    lr: f32,
    beta1: f32,
    beta2: f32,
    epsilon: f32,
    weight_decay: f32,
    m: Vec<f32>, // First moment
    v: Vec<f32>, // Second moment
    t: usize,    // Timestep
}

impl Adam {
    pub fn new(dim: usize, lr: f32) -> Self {
        Self {
            lr,
            beta1: 0.9,
            beta2: 0.999,
            epsilon: 1e-8,
            weight_decay: 0.0,
            m: vec![0.0; dim],
            v: vec![0.0; dim],
            t: 0,
        }
    }

    pub fn with_betas(mut self, beta1: f32, beta2: f32) -> Self {
        self.beta1 = beta1;
        self.beta2 = beta2;
        self
    }

    pub fn with_epsilon(mut self, eps: f32) -> Self {
        self.epsilon = eps;
        self
    }

    pub fn with_weight_decay(mut self, wd: f32) -> Self {
        self.weight_decay = wd;
        self
    }
}

impl Optimizer for Adam {
    fn step(&mut self, params: &mut [f32], gradients: &[f32]) {
        if self.m.len() != params.len() {
            self.m = vec![0.0; params.len()];
            self.v = vec![0.0; params.len()];
        }

        self.t += 1;
        let bias_correction1 = 1.0 - self.beta1.powi(self.t as i32);
        let bias_correction2 = 1.0 - self.beta2.powi(self.t as i32);

        for i in 0..params.len() {
            let g = gradients[i];

            // Update moments
            self.m[i] = self.beta1 * self.m[i] + (1.0 - self.beta1) * g;
            self.v[i] = self.beta2 * self.v[i] + (1.0 - self.beta2) * g * g;

            // Bias-corrected estimates
            let m_hat = self.m[i] / bias_correction1;
            let v_hat = self.v[i] / bias_correction2;

            // Update with optional weight decay
            let update = m_hat / (v_hat.sqrt() + self.epsilon);
            params[i] -= self.lr * (update + self.weight_decay * params[i]);
        }
    }

    fn reset(&mut self) {
        self.m.fill(0.0);
        self.v.fill(0.0);
        self.t = 0;
    }

    fn learning_rate(&self) -> f32 {
        self.lr
    }

    fn set_learning_rate(&mut self, lr: f32) {
        self.lr = lr;
    }
}

/// AdamW optimizer (decoupled weight decay)
pub struct AdamW {
    inner: Adam,
    weight_decay: f32,
}

impl AdamW {
    pub fn new(dim: usize, lr: f32) -> Self {
        Self {
            inner: Adam::new(dim, lr),
            weight_decay: 0.01,
        }
    }

    pub fn with_weight_decay(mut self, wd: f32) -> Self {
        self.weight_decay = wd;
        self
    }

    pub fn with_betas(mut self, beta1: f32, beta2: f32) -> Self {
        self.inner = self.inner.with_betas(beta1, beta2);
        self
    }
}

impl Optimizer for AdamW {
    fn step(&mut self, params: &mut [f32], gradients: &[f32]) {
        if self.inner.m.len() != params.len() {
            self.inner.m = vec![0.0; params.len()];
            self.inner.v = vec![0.0; params.len()];
        }

        self.inner.t += 1;
        let bias_correction1 = 1.0 - self.inner.beta1.powi(self.inner.t as i32);
        let bias_correction2 = 1.0 - self.inner.beta2.powi(self.inner.t as i32);

        for i in 0..params.len() {
            let g = gradients[i];

            // Update moments
            self.inner.m[i] = self.inner.beta1 * self.inner.m[i] + (1.0 - self.inner.beta1) * g;
            self.inner.v[i] = self.inner.beta2 * self.inner.v[i] + (1.0 - self.inner.beta2) * g * g;

            // Bias-corrected estimates
            let m_hat = self.inner.m[i] / bias_correction1;
            let v_hat = self.inner.v[i] / bias_correction2;

            // Decoupled weight decay (applied to params directly, not through gradient)
            params[i] *= 1.0 - self.inner.lr * self.weight_decay;

            // Adam update
            params[i] -= self.inner.lr * m_hat / (v_hat.sqrt() + self.inner.epsilon);
        }
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn learning_rate(&self) -> f32 {
        self.inner.lr
    }

    fn set_learning_rate(&mut self, lr: f32) {
        self.inner.lr = lr;
    }
}

/// Learning rate scheduler
pub struct LearningRateScheduler {
    initial_lr: f32,
    warmup_steps: usize,
    decay_steps: usize,
    min_lr: f32,
    current_step: usize,
}

impl LearningRateScheduler {
    pub fn new(initial_lr: f32) -> Self {
        Self {
            initial_lr,
            warmup_steps: 0,
            decay_steps: 100000,
            min_lr: 1e-7,
            current_step: 0,
        }
    }

    pub fn with_warmup(mut self, steps: usize) -> Self {
        self.warmup_steps = steps;
        self
    }

    pub fn with_decay(mut self, steps: usize) -> Self {
        self.decay_steps = steps;
        self
    }

    pub fn with_min_lr(mut self, min_lr: f32) -> Self {
        self.min_lr = min_lr;
        self
    }

    /// Get current learning rate and advance step
    pub fn step(&mut self) -> f32 {
        let lr = self.get_lr();
        self.current_step += 1;
        lr
    }

    /// Get learning rate without advancing
    pub fn get_lr(&self) -> f32 {
        if self.current_step < self.warmup_steps {
            // Linear warmup
            self.initial_lr * (self.current_step + 1) as f32 / self.warmup_steps as f32
        } else {
            // Cosine decay
            let progress = (self.current_step - self.warmup_steps) as f32 / self.decay_steps as f32;
            let decay = 0.5 * (1.0 + (std::f32::consts::PI * progress.min(1.0)).cos());
            self.min_lr + (self.initial_lr - self.min_lr) * decay
        }
    }

    /// Reset scheduler
    pub fn reset(&mut self) {
        self.current_step = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sgd() {
        let mut opt = SGD::new(4, 0.1);
        let mut params = vec![1.0, 2.0, 3.0, 4.0];
        let gradients = vec![0.1, 0.2, 0.3, 0.4];

        opt.step(&mut params, &gradients);

        assert!(params[0] < 1.0);
        assert!(params[1] < 2.0);
    }

    #[test]
    fn test_sgd_momentum() {
        let mut opt = SGD::new(4, 0.1).with_momentum(0.9);
        let mut params = vec![1.0; 4];
        let gradients = vec![1.0; 4];

        // Multiple steps should accumulate momentum
        for _ in 0..5 {
            opt.step(&mut params, &gradients);
        }

        assert!(params[0] < 0.0);
    }

    #[test]
    fn test_adam() {
        let mut opt = Adam::new(64, 0.001);
        let mut params = vec![0.5; 64];
        let gradients = vec![0.1; 64];

        for _ in 0..100 {
            opt.step(&mut params, &gradients);
        }

        // Should have moved toward 0
        assert!(params[0] < 0.5);
    }

    #[test]
    fn test_adamw() {
        let mut opt = AdamW::new(32, 0.001).with_weight_decay(0.01);
        let mut params = vec![1.0; 32];
        let gradients = vec![0.0; 32]; // No gradient, only weight decay

        for _ in 0..100 {
            opt.step(&mut params, &gradients);
        }

        // Weight decay should shrink params
        assert!(params[0] < 1.0);
    }

    #[test]
    fn test_lr_scheduler_warmup() {
        let mut scheduler = LearningRateScheduler::new(0.001).with_warmup(100);

        let lr_start = scheduler.step();
        assert!(lr_start < 0.001); // Still warming up

        for _ in 0..99 {
            scheduler.step();
        }

        let lr_end_warmup = scheduler.get_lr();
        assert!((lr_end_warmup - 0.001).abs() < 1e-5);
    }

    #[test]
    fn test_lr_scheduler_decay() {
        let mut scheduler = LearningRateScheduler::new(0.001)
            .with_warmup(0)
            .with_decay(100)
            .with_min_lr(0.0001);

        let lr_start = scheduler.step();
        assert!((lr_start - 0.001).abs() < 1e-5);

        for _ in 0..100 {
            scheduler.step();
        }

        let lr_end = scheduler.get_lr();
        assert!((lr_end - 0.0001).abs() < 1e-5);
    }
}
