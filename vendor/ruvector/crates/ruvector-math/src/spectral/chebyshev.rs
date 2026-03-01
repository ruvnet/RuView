//! Chebyshev Polynomials
//!
//! Efficient polynomial approximation using Chebyshev basis.
//! Key for matrix function approximation without eigendecomposition.

use std::f64::consts::PI;

/// Chebyshev polynomial of the first kind
#[derive(Debug, Clone)]
pub struct ChebyshevPolynomial {
    /// Polynomial degree
    pub degree: usize,
}

impl ChebyshevPolynomial {
    /// Create Chebyshev polynomial T_n
    pub fn new(degree: usize) -> Self {
        Self { degree }
    }

    /// Evaluate T_n(x) using recurrence
    /// T_0(x) = 1, T_1(x) = x, T_{n+1}(x) = 2x·T_n(x) - T_{n-1}(x)
    pub fn eval(&self, x: f64) -> f64 {
        if self.degree == 0 {
            return 1.0;
        }
        if self.degree == 1 {
            return x;
        }

        let mut t_prev = 1.0;
        let mut t_curr = x;

        for _ in 2..=self.degree {
            let t_next = 2.0 * x * t_curr - t_prev;
            t_prev = t_curr;
            t_curr = t_next;
        }

        t_curr
    }

    /// Evaluate all Chebyshev polynomials T_0(x) through T_n(x)
    pub fn eval_all(x: f64, max_degree: usize) -> Vec<f64> {
        if max_degree == 0 {
            return vec![1.0];
        }

        let mut result = Vec::with_capacity(max_degree + 1);
        result.push(1.0);
        result.push(x);

        for k in 2..=max_degree {
            let t_k = 2.0 * x * result[k - 1] - result[k - 2];
            result.push(t_k);
        }

        result
    }

    /// Chebyshev nodes for interpolation: x_k = cos((2k+1)π/(2n))
    pub fn nodes(n: usize) -> Vec<f64> {
        (0..n)
            .map(|k| ((2 * k + 1) as f64 * PI / (2 * n) as f64).cos())
            .collect()
    }

    /// Derivative: T'_n(x) = n * U_{n-1}(x) where U is Chebyshev of second kind
    pub fn derivative(&self, x: f64) -> f64 {
        if self.degree == 0 {
            return 0.0;
        }
        if self.degree == 1 {
            return 1.0;
        }

        // Use: T'_n(x) = n * U_{n-1}(x)
        // where U_0 = 1, U_1 = 2x, U_{n+1} = 2x*U_n - U_{n-1}
        let n = self.degree;
        let mut u_prev = 1.0;
        let mut u_curr = 2.0 * x;

        for _ in 2..n {
            let u_next = 2.0 * x * u_curr - u_prev;
            u_prev = u_curr;
            u_curr = u_next;
        }

        n as f64 * if n == 1 { u_prev } else { u_curr }
    }
}

/// Chebyshev expansion of a function
/// f(x) ≈ Σ c_k T_k(x)
#[derive(Debug, Clone)]
pub struct ChebyshevExpansion {
    /// Chebyshev coefficients c_k
    pub coefficients: Vec<f64>,
}

impl ChebyshevExpansion {
    /// Create from coefficients
    pub fn new(coefficients: Vec<f64>) -> Self {
        Self { coefficients }
    }

    /// Approximate function on [-1, 1] using n+1 Chebyshev nodes
    pub fn from_function<F: Fn(f64) -> f64>(f: F, degree: usize) -> Self {
        let n = degree + 1;
        let nodes = ChebyshevPolynomial::nodes(n);

        // Evaluate function at nodes
        let f_values: Vec<f64> = nodes.iter().map(|&x| f(x)).collect();

        // Compute coefficients via DCT-like formula
        let mut coefficients = Vec::with_capacity(n);

        for k in 0..n {
            let mut c_k = 0.0;
            for (j, &f_j) in f_values.iter().enumerate() {
                let t_k_at_node = ChebyshevPolynomial::new(k).eval(nodes[j]);
                c_k += f_j * t_k_at_node;
            }
            c_k *= 2.0 / n as f64;
            if k == 0 {
                c_k *= 0.5;
            }
            coefficients.push(c_k);
        }

        Self { coefficients }
    }

    /// Approximate exp(-t*x) for heat kernel (x in [0, 2])
    /// Maps [0, 2] to [-1, 1] via x' = x - 1
    pub fn heat_kernel(t: f64, degree: usize) -> Self {
        Self::from_function(
            |x| {
                let exponent = -t * (x + 1.0);
                // Clamp to prevent overflow (exp(709) ≈ max f64, exp(-745) ≈ 0)
                let clamped = exponent.clamp(-700.0, 700.0);
                clamped.exp()
            },
            degree,
        )
    }

    /// Approximate low-pass filter: 1 if λ < cutoff, 0 otherwise
    /// Smooth transition via sigmoid-like function
    pub fn low_pass(cutoff: f64, degree: usize) -> Self {
        let steepness = 10.0 / cutoff.max(0.1);
        Self::from_function(
            |x| {
                let lambda = (x + 1.0) / 2.0 * 2.0; // Map [-1,1] to [0,2]
                let exponent = steepness * (lambda - cutoff);
                // Clamp to prevent overflow
                let clamped = exponent.clamp(-700.0, 700.0);
                1.0 / (1.0 + clamped.exp())
            },
            degree,
        )
    }

    /// Evaluate expansion at point x using Clenshaw recurrence
    /// More numerically stable than direct summation
    pub fn eval(&self, x: f64) -> f64 {
        if self.coefficients.is_empty() {
            return 0.0;
        }
        if self.coefficients.len() == 1 {
            return self.coefficients[0];
        }

        // Clenshaw recurrence
        let n = self.coefficients.len();
        let mut b_next = 0.0;
        let mut b_curr = 0.0;

        for k in (1..n).rev() {
            let b_prev = 2.0 * x * b_curr - b_next + self.coefficients[k];
            b_next = b_curr;
            b_curr = b_prev;
        }

        self.coefficients[0] + x * b_curr - b_next
    }

    /// Evaluate expansion on vector: apply filter to each component
    pub fn eval_vector(&self, x: &[f64]) -> Vec<f64> {
        x.iter().map(|&xi| self.eval(xi)).collect()
    }

    /// Degree of expansion
    pub fn degree(&self) -> usize {
        self.coefficients.len().saturating_sub(1)
    }

    /// Truncate to lower degree
    pub fn truncate(&self, new_degree: usize) -> Self {
        let n = (new_degree + 1).min(self.coefficients.len());
        Self {
            coefficients: self.coefficients[..n].to_vec(),
        }
    }

    /// Add two expansions
    pub fn add(&self, other: &Self) -> Self {
        let max_len = self.coefficients.len().max(other.coefficients.len());
        let mut coefficients = vec![0.0; max_len];

        for (i, &c) in self.coefficients.iter().enumerate() {
            coefficients[i] += c;
        }
        for (i, &c) in other.coefficients.iter().enumerate() {
            coefficients[i] += c;
        }

        Self { coefficients }
    }

    /// Scale by constant
    pub fn scale(&self, s: f64) -> Self {
        Self {
            coefficients: self.coefficients.iter().map(|&c| c * s).collect(),
        }
    }

    /// Derivative expansion
    /// d/dx Σ c_k T_k(x) = Σ c'_k T_k(x)
    pub fn derivative(&self) -> Self {
        let n = self.coefficients.len();
        if n <= 1 {
            return Self::new(vec![0.0]);
        }

        let mut d_coeffs = vec![0.0; n - 1];

        // Backward recurrence for derivative coefficients
        for k in (0..n - 1).rev() {
            d_coeffs[k] = 2.0 * (k + 1) as f64 * self.coefficients[k + 1];
            if k + 2 < n {
                d_coeffs[k] += if k == 0 { 0.0 } else { d_coeffs[k + 2] };
            }
        }

        // First coefficient needs halving
        if !d_coeffs.is_empty() {
            d_coeffs[0] *= 0.5;
        }

        Self {
            coefficients: d_coeffs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chebyshev_polynomial() {
        // T_0(x) = 1
        assert!((ChebyshevPolynomial::new(0).eval(0.5) - 1.0).abs() < 1e-10);

        // T_1(x) = x
        assert!((ChebyshevPolynomial::new(1).eval(0.5) - 0.5).abs() < 1e-10);

        // T_2(x) = 2x² - 1
        let t2_at_half = 2.0 * 0.5 * 0.5 - 1.0;
        assert!((ChebyshevPolynomial::new(2).eval(0.5) - t2_at_half).abs() < 1e-10);

        // T_3(x) = 4x³ - 3x
        let t3_at_half = 4.0 * 0.5_f64.powi(3) - 3.0 * 0.5;
        assert!((ChebyshevPolynomial::new(3).eval(0.5) - t3_at_half).abs() < 1e-10);
    }

    #[test]
    fn test_eval_all() {
        let x = 0.5;
        let all = ChebyshevPolynomial::eval_all(x, 5);

        assert_eq!(all.len(), 6);
        for (k, &t_k) in all.iter().enumerate() {
            let expected = ChebyshevPolynomial::new(k).eval(x);
            assert!((t_k - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn test_chebyshev_nodes() {
        let nodes = ChebyshevPolynomial::nodes(4);
        assert_eq!(nodes.len(), 4);

        // All nodes should be in [-1, 1]
        for &x in &nodes {
            assert!(x >= -1.0 && x <= 1.0);
        }
    }

    #[test]
    fn test_expansion_constant() {
        let expansion = ChebyshevExpansion::from_function(|_| 5.0, 3);

        // Should approximate 5.0 everywhere
        for x in [-0.9, -0.5, 0.0, 0.5, 0.9] {
            assert!((expansion.eval(x) - 5.0).abs() < 0.1);
        }
    }

    #[test]
    fn test_expansion_linear() {
        let expansion = ChebyshevExpansion::from_function(|x| 2.0 * x + 1.0, 5);

        for x in [-0.8, -0.3, 0.0, 0.4, 0.7] {
            let expected = 2.0 * x + 1.0;
            assert!(
                (expansion.eval(x) - expected).abs() < 0.1,
                "x={}, expected={}, got={}",
                x,
                expected,
                expansion.eval(x)
            );
        }
    }

    #[test]
    fn test_heat_kernel() {
        let heat = ChebyshevExpansion::heat_kernel(1.0, 10);

        // At x = -1 (λ = 0): exp(0) = 1
        let at_zero = heat.eval(-1.0);
        assert!((at_zero - 1.0).abs() < 0.1);

        // At x = 1 (λ = 2): exp(-2) ≈ 0.135
        let at_two = heat.eval(1.0);
        assert!((at_two - (-2.0_f64).exp()).abs() < 0.1);
    }

    #[test]
    fn test_clenshaw_stability() {
        // High degree expansion should still be numerically stable
        let expansion = ChebyshevExpansion::from_function(|x| x.sin(), 20);

        for x in [-0.9, 0.0, 0.9] {
            let approx = expansion.eval(x);
            let exact = x.sin();
            assert!(
                (approx - exact).abs() < 0.01,
                "x={}, approx={}, exact={}",
                x,
                approx,
                exact
            );
        }
    }
}
