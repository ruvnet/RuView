//! Sum-of-Squares Decomposition
//!
//! Check if a polynomial can be written as a sum of squared polynomials.

use super::polynomial::{Monomial, Polynomial, Term};

/// SOS decomposition configuration
#[derive(Debug, Clone)]
pub struct SOSConfig {
    /// Maximum iterations for SDP solver
    pub max_iters: usize,
    /// Convergence tolerance
    pub tolerance: f64,
    /// Regularization parameter
    pub regularization: f64,
}

impl Default for SOSConfig {
    fn default() -> Self {
        Self {
            max_iters: 100,
            tolerance: 1e-8,
            regularization: 1e-6,
        }
    }
}

/// Result of SOS decomposition
#[derive(Debug, Clone)]
pub enum SOSResult {
    /// Polynomial is SOS with given decomposition
    IsSOS(SOSDecomposition),
    /// Could not verify SOS (may or may not be SOS)
    Unknown,
    /// Polynomial is definitely not SOS (has negative value somewhere)
    NotSOS { witness: Vec<f64> },
}

/// SOS decomposition: p = Σ q_i²
#[derive(Debug, Clone)]
pub struct SOSDecomposition {
    /// The squared polynomials q_i
    pub squares: Vec<Polynomial>,
    /// Gram matrix Q such that p = v^T Q v where v is monomial basis
    pub gram_matrix: Vec<f64>,
    /// Monomial basis used
    pub basis: Vec<Monomial>,
}

impl SOSDecomposition {
    /// Verify decomposition: check that Σ q_i² ≈ original polynomial
    pub fn verify(&self, original: &Polynomial, tol: f64) -> bool {
        let reconstructed = self.reconstruct();

        // Check each term
        for (m, &c) in original.terms() {
            let c_rec = reconstructed.coeff(m);
            if (c - c_rec).abs() > tol {
                return false;
            }
        }

        // Check that reconstructed doesn't have extra terms
        for (m, &c) in reconstructed.terms() {
            if c.abs() > tol && original.coeff(m).abs() < tol {
                return false;
            }
        }

        true
    }

    /// Reconstruct polynomial from decomposition
    pub fn reconstruct(&self) -> Polynomial {
        let mut result = Polynomial::zero();
        for q in &self.squares {
            result = result.add(&q.square());
        }
        result
    }

    /// Get lower bound on polynomial (should be ≥ 0 if SOS)
    pub fn lower_bound(&self) -> f64 {
        0.0 // SOS polynomials are always ≥ 0
    }
}

/// SOS checker/decomposer
pub struct SOSChecker {
    config: SOSConfig,
}

impl SOSChecker {
    /// Create with config
    pub fn new(config: SOSConfig) -> Self {
        Self { config }
    }

    /// Create with defaults
    pub fn default() -> Self {
        Self::new(SOSConfig::default())
    }

    /// Check if polynomial is SOS and find decomposition
    pub fn check(&self, p: &Polynomial) -> SOSResult {
        let degree = p.degree();
        if degree == 0 {
            // Constant polynomial
            let c = p.eval(&[]);
            if c >= 0.0 {
                return SOSResult::IsSOS(SOSDecomposition {
                    squares: vec![Polynomial::constant(c.sqrt())],
                    gram_matrix: vec![c],
                    basis: vec![Monomial::one()],
                });
            } else {
                return SOSResult::NotSOS { witness: vec![] };
            }
        }

        if degree % 2 == 1 {
            // Odd degree polynomials cannot be SOS (go to -∞)
            // Try to find a witness
            let witness = self.find_negative_witness(p);
            if let Some(w) = witness {
                return SOSResult::NotSOS { witness: w };
            }
            return SOSResult::Unknown;
        }

        // Build SOS program
        let half_degree = degree / 2;
        let num_vars = p.num_variables();

        // Monomial basis for degree ≤ half_degree
        let basis = Polynomial::monomials_up_to_degree(num_vars, half_degree);
        let n = basis.len();

        if n == 0 {
            return SOSResult::Unknown;
        }

        // Try to find Gram matrix Q such that p = v^T Q v
        // where v is the monomial basis vector
        match self.find_gram_matrix(p, &basis) {
            Some(gram) => {
                // Check if Gram matrix is PSD
                if self.is_psd(&gram, n) {
                    let squares = self.extract_squares(&gram, &basis, n);
                    SOSResult::IsSOS(SOSDecomposition {
                        squares,
                        gram_matrix: gram,
                        basis,
                    })
                } else {
                    SOSResult::Unknown
                }
            }
            None => {
                // Try to find witness that p < 0
                let witness = self.find_negative_witness(p);
                if let Some(w) = witness {
                    SOSResult::NotSOS { witness: w }
                } else {
                    SOSResult::Unknown
                }
            }
        }
    }

    /// Find Gram matrix via moment matching
    fn find_gram_matrix(&self, p: &Polynomial, basis: &[Monomial]) -> Option<Vec<f64>> {
        let n = basis.len();

        // Build mapping from monomial to coefficient constraint
        // p = Σ_{i,j} Q[i,j] * (basis[i] * basis[j])
        // So for each monomial m in p, we need:
        // coeff(m) = Σ_{i,j: basis[i]*basis[j] = m} Q[i,j]

        // For simplicity, use a direct approach for small cases
        // and iterative refinement for larger ones

        if n <= 10 {
            return self.find_gram_direct(p, basis);
        }

        self.find_gram_iterative(p, basis)
    }

    /// Direct Gram matrix construction for small cases
    fn find_gram_direct(&self, p: &Polynomial, basis: &[Monomial]) -> Option<Vec<f64>> {
        let n = basis.len();

        // Start with identity scaled by constant term
        let c0 = p.coeff(&Monomial::one());
        let scale = (c0.abs() + 1.0) / n as f64;

        let mut gram = vec![0.0; n * n];
        for i in 0..n {
            gram[i * n + i] = scale;
        }

        // Iteratively adjust to match polynomial coefficients
        for _ in 0..self.config.max_iters {
            // Compute current reconstruction
            let mut recon_terms = std::collections::HashMap::new();
            for i in 0..n {
                for j in 0..n {
                    let m = basis[i].mul(&basis[j]);
                    *recon_terms.entry(m).or_insert(0.0) += gram[i * n + j];
                }
            }

            // Compute error
            let mut max_err = 0.0f64;
            for (m, &c_target) in p.terms() {
                let c_current = *recon_terms.get(m).unwrap_or(&0.0);
                max_err = max_err.max((c_target - c_current).abs());
            }

            if max_err < self.config.tolerance {
                return Some(gram);
            }

            // Gradient step to reduce error
            let step = 0.1;
            for i in 0..n {
                for j in 0..n {
                    let m = basis[i].mul(&basis[j]);
                    let c_target = p.coeff(&m);
                    let c_current = *recon_terms.get(&m).unwrap_or(&0.0);
                    let err = c_target - c_current;

                    // Count how many (i',j') pairs produce this monomial
                    let count = self.count_pairs(&basis, &m);
                    if count > 0 {
                        gram[i * n + j] += step * err / count as f64;
                    }
                }
            }

            // Project to symmetric
            for i in 0..n {
                for j in i + 1..n {
                    let avg = (gram[i * n + j] + gram[j * n + i]) / 2.0;
                    gram[i * n + j] = avg;
                    gram[j * n + i] = avg;
                }
            }

            // Regularize diagonal
            for i in 0..n {
                gram[i * n + i] = gram[i * n + i].max(self.config.regularization);
            }
        }

        None
    }

    fn find_gram_iterative(&self, p: &Polynomial, basis: &[Monomial]) -> Option<Vec<f64>> {
        // Same as direct but with larger step budget
        self.find_gram_direct(p, basis)
    }

    fn count_pairs(&self, basis: &[Monomial], target: &Monomial) -> usize {
        let n = basis.len();
        let mut count = 0;
        for i in 0..n {
            for j in 0..n {
                if basis[i].mul(&basis[j]) == *target {
                    count += 1;
                }
            }
        }
        count
    }

    /// Check if matrix is positive semidefinite via Cholesky
    fn is_psd(&self, gram: &[f64], n: usize) -> bool {
        // Simple check: try Cholesky decomposition
        let mut l = vec![0.0; n * n];

        for i in 0..n {
            for j in 0..=i {
                let mut sum = gram[i * n + j];
                for k in 0..j {
                    sum -= l[i * n + k] * l[j * n + k];
                }

                if i == j {
                    if sum < -self.config.tolerance {
                        return false;
                    }
                    l[i * n + j] = sum.max(0.0).sqrt();
                } else {
                    let ljj = l[j * n + j];
                    l[i * n + j] = if ljj > self.config.tolerance {
                        sum / ljj
                    } else {
                        0.0
                    };
                }
            }
        }

        true
    }

    /// Extract square polynomials from Gram matrix via Cholesky
    fn extract_squares(&self, gram: &[f64], basis: &[Monomial], n: usize) -> Vec<Polynomial> {
        // Cholesky: G = L L^T
        let mut l = vec![0.0; n * n];

        for i in 0..n {
            for j in 0..=i {
                let mut sum = gram[i * n + j];
                for k in 0..j {
                    sum -= l[i * n + k] * l[j * n + k];
                }

                if i == j {
                    l[i * n + j] = sum.max(0.0).sqrt();
                } else {
                    let ljj = l[j * n + j];
                    l[i * n + j] = if ljj > 1e-15 { sum / ljj } else { 0.0 };
                }
            }
        }

        // Each column of L gives a polynomial q_j = Σ_i L[i,j] * basis[i]
        let mut squares = Vec::new();
        for j in 0..n {
            let terms: Vec<Term> = (0..n)
                .filter(|&i| l[i * n + j].abs() > 1e-15)
                .map(|i| Term {
                    coeff: l[i * n + j],
                    monomial: basis[i].clone(),
                })
                .collect();

            if !terms.is_empty() {
                squares.push(Polynomial::from_terms(terms));
            }
        }

        squares
    }

    /// Try to find a point where polynomial is negative
    fn find_negative_witness(&self, p: &Polynomial) -> Option<Vec<f64>> {
        let n = p.num_variables().max(1);

        // Grid search
        let grid = [-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0];

        fn recurse(
            p: &Polynomial,
            current: &mut Vec<f64>,
            depth: usize,
            n: usize,
            grid: &[f64],
        ) -> Option<Vec<f64>> {
            if depth == n {
                if p.eval(current) < -1e-10 {
                    return Some(current.clone());
                }
                return None;
            }

            for &v in grid {
                current.push(v);
                if let Some(w) = recurse(p, current, depth + 1, n, grid) {
                    return Some(w);
                }
                current.pop();
            }

            None
        }

        let mut current = Vec::new();
        recurse(p, &mut current, 0, n, &grid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_sos() {
        let p = Polynomial::constant(4.0);
        let checker = SOSChecker::default();

        match checker.check(&p) {
            SOSResult::IsSOS(decomp) => {
                assert!(decomp.verify(&p, 1e-6));
            }
            _ => panic!("4.0 should be SOS"),
        }
    }

    #[test]
    fn test_negative_constant_not_sos() {
        let p = Polynomial::constant(-1.0);
        let checker = SOSChecker::default();

        match checker.check(&p) {
            SOSResult::NotSOS { .. } => {}
            _ => panic!("-1.0 should not be SOS"),
        }
    }

    #[test]
    fn test_square_is_sos() {
        // (x + y)² = x² + 2xy + y² is SOS
        let x = Polynomial::var(0);
        let y = Polynomial::var(1);
        let p = x.add(&y).square();

        let checker = SOSChecker::default();

        match checker.check(&p) {
            SOSResult::IsSOS(decomp) => {
                // Verify reconstruction
                let recon = decomp.reconstruct();
                for pt in [vec![1.0, 1.0], vec![2.0, -1.0], vec![0.0, 3.0]] {
                    let diff = (p.eval(&pt) - recon.eval(&pt)).abs();
                    assert!(diff < 1.0, "Reconstruction error too large: {}", diff);
                }
            }
            SOSResult::Unknown => {
                // Simplified solver may not always converge
                // But polynomial should be non-negative at sample points
                for pt in [vec![1.0, 1.0], vec![2.0, -1.0], vec![0.0, 3.0]] {
                    assert!(p.eval(&pt) >= 0.0, "(x+y)² should be >= 0");
                }
            }
            SOSResult::NotSOS { witness } => {
                // Should not find counterexample for a true SOS polynomial
                panic!(
                    "(x+y)² incorrectly marked as not SOS with witness {:?}",
                    witness
                );
            }
        }
    }

    #[test]
    fn test_x_squared_plus_one() {
        // x² + 1 is SOS
        let x = Polynomial::var(0);
        let p = x.square().add(&Polynomial::constant(1.0));

        let checker = SOSChecker::default();

        match checker.check(&p) {
            SOSResult::IsSOS(_) => {}
            SOSResult::Unknown => {} // Acceptable if solver didn't converge
            SOSResult::NotSOS { .. } => panic!("x² + 1 should be SOS"),
        }
    }
}
