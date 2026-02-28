//! Differential Privacy for Pattern Sharing

use rand::Rng;

#[derive(Debug, Clone)]
pub struct DpConfig {
    pub epsilon: f64,     // Privacy budget
    pub delta: f64,       // Failure probability
    pub sensitivity: f64, // Query sensitivity
}

impl Default for DpConfig {
    fn default() -> Self {
        Self {
            epsilon: 1.0,
            delta: 1e-5,
            sensitivity: 1.0,
        }
    }
}

pub struct DifferentialPrivacy {
    config: DpConfig,
}

impl DifferentialPrivacy {
    pub fn new(config: DpConfig) -> Self {
        Self { config }
    }

    /// Add Laplace noise for (epsilon, 0)-differential privacy
    pub fn laplace_noise(&self, value: f64) -> f64 {
        let scale = self.config.sensitivity / self.config.epsilon;
        let noise = self.sample_laplace(scale);
        value + noise
    }

    /// Add Laplace noise to a vector
    pub fn add_noise_to_vector(&self, vector: &mut [f32]) {
        let scale = self.config.sensitivity / self.config.epsilon;
        for v in vector.iter_mut() {
            let noise = self.sample_laplace(scale);
            *v += noise as f32;
        }
    }

    /// Add Gaussian noise for (epsilon, delta)-differential privacy
    pub fn gaussian_noise(&self, value: f64) -> f64 {
        let sigma = self.gaussian_sigma();
        let noise = self.sample_gaussian(sigma);
        value + noise
    }

    fn gaussian_sigma(&self) -> f64 {
        // Compute sigma for (epsilon, delta)-DP
        let c = (2.0 * (1.25 / self.config.delta).ln()).sqrt();
        c * self.config.sensitivity / self.config.epsilon
    }

    fn sample_laplace(&self, scale: f64) -> f64 {
        let mut rng = rand::thread_rng();
        // Clamp to avoid ln(0) - use small epsilon for numerical stability
        let u: f64 = rng.gen::<f64>() - 0.5;
        let clamped = (1.0 - 2.0 * u.abs()).clamp(f64::EPSILON, 1.0);
        -scale * u.signum() * clamped.ln()
    }

    fn sample_gaussian(&self, sigma: f64) -> f64 {
        let mut rng = rand::thread_rng();
        // Box-Muller transform with numerical stability
        // Clamp u1 to avoid ln(0)
        let u1: f64 = rng.gen::<f64>().clamp(f64::EPSILON, 1.0 - f64::EPSILON);
        let u2: f64 = rng.gen();
        sigma * (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// Compute privacy loss for a composition of queries
    pub fn privacy_loss(&self, num_queries: usize) -> f64 {
        // Basic composition theorem
        self.config.epsilon * (num_queries as f64)
    }

    /// Compute privacy loss with advanced composition
    pub fn advanced_privacy_loss(&self, num_queries: usize) -> f64 {
        let k = num_queries as f64;
        // Advanced composition theorem
        (2.0 * k * (1.0 / self.config.delta).ln()).sqrt() * self.config.epsilon
            + k * self.config.epsilon * (self.config.epsilon.exp() - 1.0)
    }
}
