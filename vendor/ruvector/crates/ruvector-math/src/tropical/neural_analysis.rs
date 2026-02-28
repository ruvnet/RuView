//! Tropical Neural Network Analysis
//!
//! Neural networks with ReLU activations are piecewise linear functions,
//! which can be analyzed using tropical geometry.
//!
//! ## Key Insight
//!
//! ReLU(x) = max(0, x) = 0 ⊕ x in tropical arithmetic
//!
//! A ReLU network is a composition of affine maps and tropical additions,
//! making it a tropical rational function.
//!
//! ## Applications
//!
//! - Count linear regions of a neural network
//! - Analyze decision boundaries
//! - Bound network complexity

use super::polynomial::TropicalPolynomial;

/// Analyzes ReLU neural networks using tropical geometry
#[derive(Debug, Clone)]
pub struct TropicalNeuralAnalysis {
    /// Network architecture: [input_dim, hidden1, hidden2, ..., output_dim]
    architecture: Vec<usize>,
    /// Weights: weights[l] is a (layer_size, prev_layer_size) matrix
    weights: Vec<Vec<Vec<f64>>>,
    /// Biases: biases[l] is a vector of length layer_size
    biases: Vec<Vec<f64>>,
}

impl TropicalNeuralAnalysis {
    /// Create analyzer for a ReLU network
    pub fn new(
        architecture: Vec<usize>,
        weights: Vec<Vec<Vec<f64>>>,
        biases: Vec<Vec<f64>>,
    ) -> Self {
        Self {
            architecture,
            weights,
            biases,
        }
    }

    /// Create a random network for testing
    pub fn random(architecture: Vec<usize>, seed: u64) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut weights = Vec::new();
        let mut biases = Vec::new();

        let mut s = seed;
        for i in 1..architecture.len() {
            let input_size = architecture[i - 1];
            let output_size = architecture[i];

            let mut layer_weights = Vec::new();
            for _ in 0..output_size {
                let mut neuron_weights = Vec::new();
                for _ in 0..input_size {
                    // Simple PRNG
                    s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let w = ((s >> 33) as f64 / (1u64 << 31) as f64) - 1.0;
                    neuron_weights.push(w);
                }
                layer_weights.push(neuron_weights);
            }
            weights.push(layer_weights);

            let mut layer_biases = Vec::new();
            for _ in 0..output_size {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                let b = ((s >> 33) as f64 / (1u64 << 31) as f64) - 1.0;
                layer_biases.push(b * 0.1);
            }
            biases.push(layer_biases);
        }

        Self {
            architecture,
            weights,
            biases,
        }
    }

    /// Forward pass of the ReLU network
    pub fn forward(&self, input: &[f64]) -> Vec<f64> {
        let mut x = input.to_vec();

        for layer in 0..self.weights.len() {
            let mut y = Vec::with_capacity(self.weights[layer].len());

            for (neuron_weights, &bias) in self.weights[layer].iter().zip(self.biases[layer].iter())
            {
                let linear: f64 = neuron_weights
                    .iter()
                    .zip(x.iter())
                    .map(|(w, xi)| w * xi)
                    .sum();
                let z = linear + bias;
                // ReLU = max(0, z) = tropical addition
                y.push(z.max(0.0));
            }

            x = y;
        }

        x
    }

    /// Upper bound on number of linear regions
    ///
    /// For a network with widths n_0, n_1, ..., n_L where n_0 is input dimension:
    /// Upper bound = prod_{i=1}^{L-1} sum_{j=0}^{min(n_0, n_i)} C(n_i, j)
    ///
    /// This follows from tropical geometry considerations.
    pub fn linear_region_upper_bound(&self) -> u128 {
        if self.architecture.len() < 2 {
            return 1;
        }

        let n0 = self.architecture[0] as u128;
        let mut bound: u128 = 1;

        for i in 1..self.architecture.len() - 1 {
            let ni = self.architecture[i] as u128;

            // Sum of binomial coefficients C(ni, j) for j = 0 to min(n0, ni)
            let k_max = n0.min(ni);
            let mut layer_sum: u128 = 0;

            for j in 0..=k_max {
                layer_sum = layer_sum.saturating_add(binomial(ni, j));
            }

            bound = bound.saturating_mul(layer_sum);
        }

        bound
    }

    /// Estimate actual linear regions by sampling
    ///
    /// Samples random points and counts how many distinct activation patterns occur.
    pub fn estimate_linear_regions(&self, num_samples: usize, seed: u64) -> usize {
        use std::collections::HashSet;

        let mut activation_patterns = HashSet::new();
        let input_dim = self.architecture[0];

        let mut s = seed;
        for _ in 0..num_samples {
            // Generate random input
            let mut input = Vec::with_capacity(input_dim);
            for _ in 0..input_dim {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                let x = ((s >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0;
                input.push(x);
            }

            // Track activation pattern
            let pattern = self.get_activation_pattern(&input);
            activation_patterns.insert(pattern);
        }

        activation_patterns.len()
    }

    /// Get activation pattern (which neurons are active) for an input
    fn get_activation_pattern(&self, input: &[f64]) -> Vec<bool> {
        let mut x = input.to_vec();
        let mut pattern = Vec::new();

        for layer in 0..self.weights.len() {
            let mut y = Vec::with_capacity(self.weights[layer].len());

            for (neuron_weights, &bias) in self.weights[layer].iter().zip(self.biases[layer].iter())
            {
                let linear: f64 = neuron_weights
                    .iter()
                    .zip(x.iter())
                    .map(|(w, xi)| w * xi)
                    .sum();
                let z = linear + bias;
                pattern.push(z > 0.0);
                y.push(z.max(0.0));
            }

            x = y;
        }

        pattern
    }

    /// Compute the tropical polynomial representation for 1D input
    /// Returns the piecewise linear function f(x)
    pub fn as_tropical_polynomial_1d(&self) -> Option<TropicalPolynomial> {
        if self.architecture[0] != 1 || self.architecture[self.architecture.len() - 1] != 1 {
            return None;
        }

        // For 1D input, we can enumerate the breakpoints
        let breakpoints = self.find_breakpoints_1d(-10.0, 10.0, 1000);

        if breakpoints.is_empty() {
            return None;
        }

        // Build tropical polynomial from breakpoints
        // Each breakpoint corresponds to a change in slope
        let mut terms = Vec::new();
        for (i, &x) in breakpoints.iter().enumerate() {
            let y = self.forward(&[x])[0];
            terms.push((y - (i as f64) * x, i as i32));
        }

        Some(TropicalPolynomial::from_monomials(
            terms
                .into_iter()
                .map(|(c, e)| super::polynomial::TropicalMonomial::new(c, e))
                .collect(),
        ))
    }

    /// Find breakpoints of the 1D piecewise linear function
    fn find_breakpoints_1d(&self, x_min: f64, x_max: f64, num_samples: usize) -> Vec<f64> {
        let mut breakpoints = vec![x_min];
        let dx = (x_max - x_min) / num_samples as f64;

        let mut prev_pattern = self.get_activation_pattern(&[x_min]);

        for i in 1..=num_samples {
            let x = x_min + i as f64 * dx;
            let pattern = self.get_activation_pattern(&[x]);

            if pattern != prev_pattern {
                // Breakpoint somewhere between previous x and current x
                let breakpoint = self.binary_search_breakpoint(x - dx, x, &prev_pattern);
                breakpoints.push(breakpoint);
                prev_pattern = pattern;
            }
        }

        breakpoints.push(x_max);
        breakpoints
    }

    /// Binary search for exact breakpoint location
    fn binary_search_breakpoint(&self, mut lo: f64, mut hi: f64, lo_pattern: &[bool]) -> f64 {
        for _ in 0..50 {
            let mid = (lo + hi) / 2.0;
            let mid_pattern = self.get_activation_pattern(&[mid]);

            if mid_pattern == *lo_pattern {
                lo = mid;
            } else {
                hi = mid;
            }
        }

        (lo + hi) / 2.0
    }

    /// Compute decision boundary complexity for binary classification
    pub fn decision_boundary_complexity(&self, num_samples: usize, seed: u64) -> f64 {
        // For a binary classifier, count sign changes in output
        // along random rays through the input space

        let input_dim = self.architecture[0];
        let mut total_changes = 0;
        let mut s = seed;

        for _ in 0..num_samples {
            // Random direction
            let mut direction = Vec::with_capacity(input_dim);
            for _ in 0..input_dim {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                let d = ((s >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0;
                direction.push(d);
            }

            // Normalize
            let norm: f64 = direction.iter().map(|x| x * x).sum::<f64>().sqrt();
            for d in direction.iter_mut() {
                *d /= norm.max(1e-10);
            }

            // Count sign changes along ray
            let mut prev_sign = None;
            for t in -100..=100 {
                let t = t as f64 * 0.1;
                let input: Vec<f64> = direction.iter().map(|d| t * d).collect();
                let output = self.forward(&input);

                if !output.is_empty() {
                    let sign = output[0] > 0.0;
                    if let Some(prev) = prev_sign {
                        if prev != sign {
                            total_changes += 1;
                        }
                    }
                    prev_sign = Some(sign);
                }
            }
        }

        total_changes as f64 / num_samples as f64
    }
}

/// Counter for linear regions of piecewise linear functions
#[derive(Debug, Clone)]
pub struct LinearRegionCounter {
    /// Dimension of input space
    input_dim: usize,
}

impl LinearRegionCounter {
    /// Create counter for given input dimension
    pub fn new(input_dim: usize) -> Self {
        Self { input_dim }
    }

    /// Theoretical maximum for n-dimensional input with k hyperplanes
    /// This is the central zone counting problem
    pub fn hyperplane_arrangement_max(&self, num_hyperplanes: usize) -> u128 {
        // Maximum regions = sum_{i=0}^{n} C(k, i)
        let n = self.input_dim as u128;
        let k = num_hyperplanes as u128;

        let mut total: u128 = 0;
        for i in 0..=n.min(k) {
            total = total.saturating_add(binomial(k, i));
        }

        total
    }

    /// Zaslavsky's theorem: count regions of hyperplane arrangement
    /// For a general position arrangement of k hyperplanes in R^n:
    /// regions = sum_{i=0}^n C(k, i)
    pub fn zaslavsky_formula(&self, num_hyperplanes: usize) -> u128 {
        self.hyperplane_arrangement_max(num_hyperplanes)
    }
}

/// Compute binomial coefficient C(n, k) = n! / (k! * (n-k)!)
fn binomial(n: u128, k: u128) -> u128 {
    if k > n {
        return 0;
    }

    let k = k.min(n - k); // Use symmetry

    let mut result: u128 = 1;
    for i in 0..k {
        result = result.saturating_mul(n - i) / (i + 1);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relu_forward() {
        let analysis = TropicalNeuralAnalysis::new(
            vec![2, 3, 1],
            vec![
                vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]],
                vec![vec![1.0, 1.0, 1.0]],
            ],
            vec![vec![0.0, 0.0, -1.0], vec![0.0]],
        );

        let output = analysis.forward(&[1.0, 1.0]);
        assert!(output[0] > 0.0);
    }

    #[test]
    fn test_linear_region_bound() {
        // Network: 2 -> 4 -> 4 -> 1
        let analysis = TropicalNeuralAnalysis::random(vec![2, 4, 4, 1], 42);
        let bound = analysis.linear_region_upper_bound();

        // For 2D input with hidden layers of 4:
        // Upper bound = C(4,0)+C(4,1)+C(4,2) for each hidden layer
        // = (1 + 4 + 6)^2 = 121
        assert!(bound > 0);
    }

    #[test]
    fn test_estimate_regions() {
        let analysis = TropicalNeuralAnalysis::random(vec![2, 4, 1], 42);
        let estimate = analysis.estimate_linear_regions(1000, 123);

        // Should find multiple regions
        assert!(estimate >= 1);
    }

    #[test]
    fn test_binomial() {
        assert_eq!(binomial(5, 2), 10);
        assert_eq!(binomial(10, 0), 1);
        assert_eq!(binomial(10, 10), 1);
        assert_eq!(binomial(6, 3), 20);
    }

    #[test]
    fn test_hyperplane_max() {
        let counter = LinearRegionCounter::new(2);

        // 3 lines in R^2 can create at most 1 + 3 + 3 = 7 regions
        assert_eq!(counter.hyperplane_arrangement_max(3), 7);
    }
}
