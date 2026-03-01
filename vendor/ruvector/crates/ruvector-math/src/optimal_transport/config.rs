//! Configuration for optimal transport algorithms

/// Configuration for Wasserstein distance computation
#[derive(Debug, Clone)]
pub struct WassersteinConfig {
    /// Number of random projections for Sliced Wasserstein
    pub num_projections: usize,
    /// Regularization parameter for Sinkhorn (epsilon)
    pub regularization: f64,
    /// Maximum iterations for Sinkhorn
    pub max_iterations: usize,
    /// Convergence threshold for Sinkhorn
    pub threshold: f64,
    /// Power p for Wasserstein-p distance
    pub p: f64,
    /// Random seed for reproducibility
    pub seed: Option<u64>,
}

impl Default for WassersteinConfig {
    fn default() -> Self {
        Self {
            num_projections: 100,
            regularization: 0.1,
            max_iterations: 100,
            threshold: 1e-6,
            p: 2.0,
            seed: None,
        }
    }
}

impl WassersteinConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of random projections
    pub fn with_projections(mut self, n: usize) -> Self {
        self.num_projections = n;
        self
    }

    /// Set the regularization parameter
    pub fn with_regularization(mut self, eps: f64) -> Self {
        self.regularization = eps;
        self
    }

    /// Set the maximum iterations
    pub fn with_max_iterations(mut self, max_iter: usize) -> Self {
        self.max_iterations = max_iter;
        self
    }

    /// Set the convergence threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Set the Wasserstein power
    pub fn with_power(mut self, p: f64) -> Self {
        self.p = p;
        self
    }

    /// Set the random seed
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> crate::Result<()> {
        if self.num_projections == 0 {
            return Err(crate::MathError::invalid_parameter(
                "num_projections",
                "must be > 0",
            ));
        }
        if self.regularization <= 0.0 {
            return Err(crate::MathError::invalid_parameter(
                "regularization",
                "must be > 0",
            ));
        }
        if self.p <= 0.0 {
            return Err(crate::MathError::invalid_parameter("p", "must be > 0"));
        }
        Ok(())
    }
}
