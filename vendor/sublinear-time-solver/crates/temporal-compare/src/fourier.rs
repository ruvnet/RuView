use ndarray::{Array1, Array2};
use rand::{thread_rng, Rng, distributions::Uniform};
use std::f32::consts::PI;

/// Random Fourier Features for kernel approximation
/// Maps input to high-dimensional space where linear methods work well
pub struct FourierFeatures {
    // Random projection parameters
    omega: Array2<f32>,     // Random frequencies (D x d)
    b: Array1<f32>,         // Random phase shifts
    input_dim: usize,
    feature_dim: usize,
    sigma: f32,             // RBF kernel bandwidth

    // Linear model on top
    weights: Array1<f32>,
    bias: f32,
}

impl FourierFeatures {
    pub fn new(input_dim: usize, feature_dim: usize, sigma: f32) -> Self {
        let mut rng = thread_rng();
        let normal = Uniform::new(0.0, 2.0 * PI);

        // Sample random frequencies from Gaussian (for RBF kernel)
        let scale = 1.0 / sigma;
        let omega = Array2::from_shape_fn((feature_dim, input_dim), |_|
            rng.gen::<f32>() * scale);

        // Random phase shifts
        let b = Array1::from_shape_fn(feature_dim, |_| rng.sample(normal));

        Self {
            omega,
            b,
            input_dim,
            feature_dim,
            sigma,
            weights: Array1::zeros(feature_dim),
            bias: 0.0,
        }
    }

    /// Transform input using random Fourier features
    pub fn transform(&self, x: &[f32]) -> Array1<f32> {
        let x_arr = Array1::from_vec(x.to_vec());
        let projections = self.omega.dot(&x_arr) + &self.b;

        // Apply cosine to get Fourier features
        let scale = (2.0 / self.feature_dim as f32).sqrt();
        projections.mapv(|p| (p.cos() * scale))
    }

    /// Batch transform
    pub fn transform_batch(&self, x: &[Vec<f32>]) -> Array2<f32> {
        let n_samples = x.len();
        let mut features = Array2::zeros((n_samples, self.feature_dim));

        for (i, xi) in x.iter().enumerate() {
            let feat = self.transform(xi);
            features.row_mut(i).assign(&feat);
        }

        features
    }

    /// Train using closed-form ridge regression
    pub fn train(&mut self, x: &[Vec<f32>], y: &[f32], lambda: f32) {
        let features = self.transform_batch(x);
        let n = features.nrows();

        // Closed-form solution: w = (X^T X + λI)^{-1} X^T y
        let xtx = features.t().dot(&features);
        let xty = features.t().dot(&Array1::from_vec(y.to_vec()));

        // Add regularization
        let mut reg_xtx = xtx + Array2::<f32>::eye(self.feature_dim) * lambda * n as f32;

        // Solve (simplified - production would use LAPACK)
        self.weights = self.solve_regularized(&reg_xtx, &xty);

        // Compute bias
        let predictions = features.dot(&self.weights);
        let mean_pred = predictions.mean().unwrap();
        let mean_y = y.iter().sum::<f32>() / y.len() as f32;
        self.bias = mean_y - mean_pred;
    }

    fn solve_regularized(&self, a: &Array2<f32>, b: &Array1<f32>) -> Array1<f32> {
        // Simplified solver using gradient descent
        let mut x = Array1::zeros(self.feature_dim);
        let lr = 0.01;

        for _ in 0..100 {
            let grad = a.dot(&x) - b;
            x = x - &grad * lr;
        }

        x
    }

    pub fn predict(&self, x: &[Vec<f32>]) -> Vec<f32> {
        x.iter().map(|xi| {
            let features = self.transform(xi);
            self.weights.dot(&features) + self.bias
        }).collect()
    }

    pub fn predict_class(&self, x: &[Vec<f32>]) -> Vec<usize> {
        self.predict(x).iter().map(|&y| {
            if y < -0.25 { 0 }
            else if y > 0.25 { 2 }
            else { 1 }
        }).collect()
    }
}

/// Adaptive Fourier Features with frequency learning
pub struct AdaptiveFourierFeatures {
    base: FourierFeatures,
    frequency_lr: f32,
    adapt_frequencies: bool,
}

impl AdaptiveFourierFeatures {
    pub fn new(input_dim: usize, feature_dim: usize, sigma: f32) -> Self {
        Self {
            base: FourierFeatures::new(input_dim, feature_dim, sigma),
            frequency_lr: 0.001,
            adapt_frequencies: true,
        }
    }

    /// Train with frequency adaptation
    pub fn train_adaptive(&mut self, x: &[Vec<f32>], y: &[f32], epochs: usize) {
        for epoch in 0..epochs {
            // Standard training
            self.base.train(x, y, 0.01);

            if self.adapt_frequencies && epoch % 10 == 0 {
                // Adapt frequencies based on gradient
                self.adapt_frequencies_step(x, y);
            }
        }
    }

    fn adapt_frequencies_step(&mut self, x: &[Vec<f32>], y: &[f32]) {
        // Compute gradient w.r.t frequencies
        let mut grad_omega = Array2::zeros((self.base.feature_dim, self.base.input_dim));

        for (xi, &yi) in x.iter().zip(y.iter()) {
            let x_arr = Array1::from_vec(xi.clone());
            let projections = self.base.omega.dot(&x_arr) + &self.base.b;
            let features = projections.mapv(|p| p.cos());

            let pred = self.base.weights.dot(&features) + self.base.bias;
            let error = pred - yi;

            // Gradient through cosine
            for j in 0..self.base.feature_dim {
                let grad_cos = -projections[j].sin();
                let grad_j = error * self.base.weights[j] * grad_cos;

                for k in 0..self.base.input_dim {
                    grad_omega[[j, k]] += grad_j * xi[k];
                }
            }
        }

        // Update frequencies
        self.base.omega = &self.base.omega - &grad_omega * self.frequency_lr;
    }

    pub fn predict(&self, x: &[Vec<f32>]) -> Vec<f32> {
        self.base.predict(x)
    }

    pub fn predict_class(&self, x: &[Vec<f32>]) -> Vec<usize> {
        self.base.predict_class(x)
    }
}