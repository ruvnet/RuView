//! Tropical Polynomials
//!
//! A tropical polynomial p(x) = ⊕_i (a_i ⊗ x^i) = max_i(a_i + i*x)
//! represents a piecewise linear function.
//!
//! Key property: The number of linear pieces = number of "bends" in the graph.

use super::semiring::Tropical;

/// A monomial in tropical arithmetic: a ⊗ x^k = a + k*x
#[derive(Debug, Clone, Copy)]
pub struct TropicalMonomial {
    /// Coefficient (tropical)
    pub coeff: f64,
    /// Exponent
    pub exp: i32,
}

impl TropicalMonomial {
    /// Create new monomial
    pub fn new(coeff: f64, exp: i32) -> Self {
        Self { coeff, exp }
    }

    /// Evaluate at point x: coeff + exp * x
    #[inline]
    pub fn eval(&self, x: f64) -> f64 {
        if self.coeff == f64::NEG_INFINITY {
            f64::NEG_INFINITY
        } else {
            self.coeff + self.exp as f64 * x
        }
    }

    /// Multiply monomials (add coefficients, add exponents)
    pub fn mul(&self, other: &Self) -> Self {
        Self {
            coeff: self.coeff + other.coeff,
            exp: self.exp + other.exp,
        }
    }
}

/// Tropical polynomial: max_i(a_i + i*x)
///
/// Represents a piecewise linear convex function.
#[derive(Debug, Clone)]
pub struct TropicalPolynomial {
    /// Monomials (sorted by exponent)
    terms: Vec<TropicalMonomial>,
}

impl TropicalPolynomial {
    /// Create polynomial from coefficients (index = exponent)
    pub fn from_coeffs(coeffs: &[f64]) -> Self {
        let terms: Vec<TropicalMonomial> = coeffs
            .iter()
            .enumerate()
            .filter(|(_, &c)| c != f64::NEG_INFINITY)
            .map(|(i, &c)| TropicalMonomial::new(c, i as i32))
            .collect();

        Self { terms }
    }

    /// Create from explicit monomials
    pub fn from_monomials(terms: Vec<TropicalMonomial>) -> Self {
        let mut sorted = terms;
        sorted.sort_by_key(|m| m.exp);
        Self { terms: sorted }
    }

    /// Number of terms
    pub fn num_terms(&self) -> usize {
        self.terms.len()
    }

    /// Evaluate polynomial at x: max_i(a_i + i*x)
    pub fn eval(&self, x: f64) -> f64 {
        self.terms
            .iter()
            .map(|m| m.eval(x))
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Find roots (bend points) of the tropical polynomial
    /// These are x values where two linear pieces meet
    pub fn roots(&self) -> Vec<f64> {
        if self.terms.len() < 2 {
            return vec![];
        }

        let mut roots = Vec::new();

        // Find intersections between consecutive dominant pieces
        for i in 0..self.terms.len() - 1 {
            for j in i + 1..self.terms.len() {
                let m1 = &self.terms[i];
                let m2 = &self.terms[j];

                // Solve: a1 + e1*x = a2 + e2*x
                // x = (a1 - a2) / (e2 - e1)
                if m1.exp != m2.exp {
                    let x = (m1.coeff - m2.coeff) / (m2.exp - m1.exp) as f64;

                    // Check if this is actually a root (both pieces achieve max here)
                    let val = m1.eval(x);
                    let max_val = self.eval(x);

                    if (val - max_val).abs() < 1e-10 {
                        roots.push(x);
                    }
                }
            }
        }

        roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
        roots.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        roots
    }

    /// Count linear regions (pieces) of the tropical polynomial
    /// This equals 1 + number of roots
    pub fn num_linear_regions(&self) -> usize {
        1 + self.roots().len()
    }

    /// Tropical multiplication: (⊕_i a_i x^i) ⊗ (⊕_j b_j x^j) = ⊕_{i,j} (a_i + b_j) x^{i+j}
    pub fn mul(&self, other: &Self) -> Self {
        let mut new_terms = Vec::new();

        for m1 in &self.terms {
            for m2 in &other.terms {
                new_terms.push(m1.mul(m2));
            }
        }

        // Simplify: keep only dominant terms for each exponent
        new_terms.sort_by_key(|m| m.exp);

        let mut simplified = Vec::new();
        let mut i = 0;
        while i < new_terms.len() {
            let exp = new_terms[i].exp;
            let mut max_coeff = new_terms[i].coeff;

            while i < new_terms.len() && new_terms[i].exp == exp {
                max_coeff = max_coeff.max(new_terms[i].coeff);
                i += 1;
            }

            simplified.push(TropicalMonomial::new(max_coeff, exp));
        }

        Self { terms: simplified }
    }

    /// Tropical addition: max of two polynomials
    pub fn add(&self, other: &Self) -> Self {
        let mut combined: Vec<TropicalMonomial> = Vec::new();
        combined.extend(self.terms.iter().cloned());
        combined.extend(other.terms.iter().cloned());

        combined.sort_by_key(|m| m.exp);

        // Keep max coefficient for each exponent
        let mut simplified = Vec::new();
        let mut i = 0;
        while i < combined.len() {
            let exp = combined[i].exp;
            let mut max_coeff = combined[i].coeff;

            while i < combined.len() && combined[i].exp == exp {
                max_coeff = max_coeff.max(combined[i].coeff);
                i += 1;
            }

            simplified.push(TropicalMonomial::new(max_coeff, exp));
        }

        Self { terms: simplified }
    }
}

/// Multivariate tropical polynomial
/// Represents piecewise linear functions in multiple variables
#[derive(Debug, Clone)]
pub struct MultivariateTropicalPolynomial {
    /// Number of variables
    nvars: usize,
    /// Terms: (coefficient, exponent vector)
    terms: Vec<(f64, Vec<i32>)>,
}

impl MultivariateTropicalPolynomial {
    /// Create from terms
    pub fn new(nvars: usize, terms: Vec<(f64, Vec<i32>)>) -> Self {
        Self { nvars, terms }
    }

    /// Evaluate at point x
    pub fn eval(&self, x: &[f64]) -> f64 {
        assert_eq!(x.len(), self.nvars);

        self.terms
            .iter()
            .map(|(coeff, exp)| {
                if *coeff == f64::NEG_INFINITY {
                    f64::NEG_INFINITY
                } else {
                    let linear: f64 = exp
                        .iter()
                        .zip(x.iter())
                        .map(|(&e, &xi)| e as f64 * xi)
                        .sum();
                    coeff + linear
                }
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Number of terms
    pub fn num_terms(&self) -> usize {
        self.terms.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tropical_polynomial_eval() {
        // p(x) = max(2 + 0x, 1 + 1x, -1 + 2x) = max(2, 1+x, -1+2x)
        let p = TropicalPolynomial::from_coeffs(&[2.0, 1.0, -1.0]);

        assert!((p.eval(0.0) - 2.0).abs() < 1e-10); // max(2, 1, -1) = 2
        assert!((p.eval(1.0) - 2.0).abs() < 1e-10); // max(2, 2, 1) = 2
        assert!((p.eval(3.0) - 5.0).abs() < 1e-10); // max(2, 4, 5) = 5
    }

    #[test]
    fn test_tropical_roots() {
        // p(x) = max(0, x) has root at x=0
        let p = TropicalPolynomial::from_coeffs(&[0.0, 0.0]);
        let roots = p.roots();

        assert_eq!(roots.len(), 1);
        assert!(roots[0].abs() < 1e-10);
    }

    #[test]
    fn test_tropical_mul() {
        let p = TropicalPolynomial::from_coeffs(&[1.0, 2.0]); // max(1, 2+x)
        let q = TropicalPolynomial::from_coeffs(&[0.0, 1.0]); // max(0, 1+x)

        let pq = p.mul(&q);

        // At x=0: p(0)=2, q(0)=1, pq(0) should be max of products
        // We expect max(1+0, 2+1, 1+1, 2+0) for appropriate exponents
        assert!(pq.num_terms() > 0);
    }

    #[test]
    fn test_multivariate() {
        // p(x,y) = max(0, x, y)
        let p = MultivariateTropicalPolynomial::new(
            2,
            vec![(0.0, vec![0, 0]), (0.0, vec![1, 0]), (0.0, vec![0, 1])],
        );

        assert!((p.eval(&[1.0, 2.0]) - 2.0).abs() < 1e-10);
        assert!((p.eval(&[3.0, 1.0]) - 3.0).abs() < 1e-10);
    }
}
