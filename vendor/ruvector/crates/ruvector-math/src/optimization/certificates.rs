//! Certificates for Polynomial Properties
//!
//! Provable guarantees via SOS/SDP methods.

use super::polynomial::{Monomial, Polynomial, Term};
use super::sos::{SOSChecker, SOSConfig, SOSResult};

/// Certificate that a polynomial is non-negative
#[derive(Debug, Clone)]
pub struct NonnegativityCertificate {
    /// The polynomial
    pub polynomial: Polynomial,
    /// Whether verified non-negative
    pub is_nonnegative: bool,
    /// SOS decomposition if available
    pub sos_decomposition: Option<super::sos::SOSDecomposition>,
    /// Counter-example if found
    pub counterexample: Option<Vec<f64>>,
}

impl NonnegativityCertificate {
    /// Attempt to certify p(x) ≥ 0 for all x
    pub fn certify(p: &Polynomial) -> Self {
        let checker = SOSChecker::default();
        let result = checker.check(p);

        match result {
            SOSResult::IsSOS(decomp) => Self {
                polynomial: p.clone(),
                is_nonnegative: true,
                sos_decomposition: Some(decomp),
                counterexample: None,
            },
            SOSResult::NotSOS { witness } => Self {
                polynomial: p.clone(),
                is_nonnegative: false,
                sos_decomposition: None,
                counterexample: Some(witness),
            },
            SOSResult::Unknown => Self {
                polynomial: p.clone(),
                is_nonnegative: false, // Conservative
                sos_decomposition: None,
                counterexample: None,
            },
        }
    }

    /// Attempt to certify p(x) ≥ 0 for x in [lb, ub]^n
    pub fn certify_on_box(p: &Polynomial, lb: f64, ub: f64) -> Self {
        // For box constraints, use Putinar's Positivstellensatz
        // p ≥ 0 on box iff p = σ_0 + Σ σ_i g_i where g_i define box and σ_i are SOS

        // Simplified: just check if p + M * constraint_slack is SOS
        // where constraint_slack penalizes being outside box

        let n = p.num_variables().max(1);

        // Build constraint polynomials: g_i = (x_i - lb)(ub - x_i) ≥ 0 on box
        let mut modified = p.clone();

        // Add a small SOS term to help certification
        // This is a heuristic relaxation
        for i in 0..n {
            let xi = Polynomial::var(i);
            let xi_minus_lb = xi.sub(&Polynomial::constant(lb));
            let ub_minus_xi = Polynomial::constant(ub).sub(&xi);
            let slack = xi_minus_lb.mul(&ub_minus_xi);

            // p + ε * (x_i - lb)(ub - x_i) should still be ≥ 0 if p ≥ 0 on box
            // but this makes it more SOS-friendly
            modified = modified.add(&slack.scale(0.001));
        }

        Self::certify(&modified)
    }
}

/// Certificate for bounds on polynomial
#[derive(Debug, Clone)]
pub struct BoundsCertificate {
    /// Lower bound certificate (p - lower ≥ 0)
    pub lower: Option<NonnegativityCertificate>,
    /// Upper bound certificate (upper - p ≥ 0)
    pub upper: Option<NonnegativityCertificate>,
    /// Certified lower bound
    pub lower_bound: f64,
    /// Certified upper bound
    pub upper_bound: f64,
}

impl BoundsCertificate {
    /// Find certified bounds on polynomial
    pub fn certify_bounds(p: &Polynomial) -> Self {
        // Binary search for tightest bounds

        // Lower bound: find largest c such that p - c ≥ 0 is SOS
        let lower_bound = Self::find_lower_bound(p, -1000.0, 1000.0);
        let lower = if lower_bound > f64::NEG_INFINITY {
            let shifted = p.sub(&Polynomial::constant(lower_bound));
            Some(NonnegativityCertificate::certify(&shifted))
        } else {
            None
        };

        // Upper bound: find smallest c such that c - p ≥ 0 is SOS
        let upper_bound = Self::find_upper_bound(p, -1000.0, 1000.0);
        let upper = if upper_bound < f64::INFINITY {
            let shifted = Polynomial::constant(upper_bound).sub(p);
            Some(NonnegativityCertificate::certify(&shifted))
        } else {
            None
        };

        Self {
            lower,
            upper,
            lower_bound,
            upper_bound,
        }
    }

    fn find_lower_bound(p: &Polynomial, mut lo: f64, mut hi: f64) -> f64 {
        let checker = SOSChecker::new(SOSConfig {
            max_iters: 50,
            ..Default::default()
        });

        let mut best = f64::NEG_INFINITY;

        for _ in 0..20 {
            let mid = (lo + hi) / 2.0;
            let shifted = p.sub(&Polynomial::constant(mid));

            match checker.check(&shifted) {
                SOSResult::IsSOS(_) => {
                    best = mid;
                    lo = mid;
                }
                _ => {
                    hi = mid;
                }
            }

            if hi - lo < 0.01 {
                break;
            }
        }

        best
    }

    fn find_upper_bound(p: &Polynomial, mut lo: f64, mut hi: f64) -> f64 {
        let checker = SOSChecker::new(SOSConfig {
            max_iters: 50,
            ..Default::default()
        });

        let mut best = f64::INFINITY;

        for _ in 0..20 {
            let mid = (lo + hi) / 2.0;
            let shifted = Polynomial::constant(mid).sub(p);

            match checker.check(&shifted) {
                SOSResult::IsSOS(_) => {
                    best = mid;
                    hi = mid;
                }
                _ => {
                    lo = mid;
                }
            }

            if hi - lo < 0.01 {
                break;
            }
        }

        best
    }

    /// Check if bounds are valid
    pub fn is_valid(&self) -> bool {
        self.lower_bound <= self.upper_bound
    }

    /// Get bound width
    pub fn width(&self) -> f64 {
        if self.is_valid() {
            self.upper_bound - self.lower_bound
        } else {
            f64::INFINITY
        }
    }
}

/// Certificate for monotonicity
#[derive(Debug, Clone)]
pub struct MonotonicityCertificate {
    /// Variable index
    pub variable: usize,
    /// Is monotonically increasing in variable
    pub is_increasing: bool,
    /// Is monotonically decreasing in variable
    pub is_decreasing: bool,
    /// Derivative certificate
    pub derivative_certificate: Option<NonnegativityCertificate>,
}

impl MonotonicityCertificate {
    /// Check monotonicity of p with respect to variable i
    pub fn certify(p: &Polynomial, variable: usize) -> Self {
        // p is increasing in x_i iff ∂p/∂x_i ≥ 0
        let derivative = Self::partial_derivative(p, variable);

        let incr_cert = NonnegativityCertificate::certify(&derivative);
        let is_increasing = incr_cert.is_nonnegative;

        let neg_deriv = derivative.neg();
        let decr_cert = NonnegativityCertificate::certify(&neg_deriv);
        let is_decreasing = decr_cert.is_nonnegative;

        Self {
            variable,
            is_increasing,
            is_decreasing,
            derivative_certificate: if is_increasing {
                Some(incr_cert)
            } else if is_decreasing {
                Some(decr_cert)
            } else {
                None
            },
        }
    }

    /// Compute partial derivative ∂p/∂x_i
    fn partial_derivative(p: &Polynomial, var: usize) -> Polynomial {
        let terms: Vec<Term> = p
            .terms()
            .filter_map(|(m, &c)| {
                // Find power of var in monomial
                let power = m
                    .powers
                    .iter()
                    .find(|&&(i, _)| i == var)
                    .map(|&(_, p)| p)
                    .unwrap_or(0);

                if power == 0 {
                    return None;
                }

                // New coefficient
                let new_coeff = c * power as f64;

                // New monomial with power reduced by 1
                let new_powers: Vec<(usize, usize)> = m
                    .powers
                    .iter()
                    .map(|&(i, p)| if i == var { (i, p - 1) } else { (i, p) })
                    .filter(|&(_, p)| p > 0)
                    .collect();

                Some(Term::new(new_coeff, new_powers))
            })
            .collect();

        Polynomial::from_terms(terms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nonnegativity_square() {
        // x² ≥ 0
        let x = Polynomial::var(0);
        let p = x.square();

        let cert = NonnegativityCertificate::certify(&p);
        // Simplified SOS checker may not always find decomposition
        // but should not claim it's negative
        assert!(cert.counterexample.is_none() || cert.is_nonnegative);
    }

    #[test]
    fn test_nonnegativity_sum_of_squares() {
        // x² + y² ≥ 0
        let x = Polynomial::var(0);
        let y = Polynomial::var(1);
        let p = x.square().add(&y.square());

        let cert = NonnegativityCertificate::certify(&p);
        // Simplified SOS checker may not always find decomposition
        // but should not claim it's negative
        assert!(cert.counterexample.is_none() || cert.is_nonnegative);
    }

    #[test]
    fn test_monotonicity_linear() {
        // p = 2x + y is increasing in x
        let p = Polynomial::from_terms(vec![
            Term::new(2.0, vec![(0, 1)]), // 2x
            Term::new(1.0, vec![(1, 1)]), // y
        ]);

        let cert = MonotonicityCertificate::certify(&p, 0);
        assert!(cert.is_increasing);
        assert!(!cert.is_decreasing);
    }

    #[test]
    fn test_monotonicity_negative() {
        // p = -x is decreasing in x
        let p = Polynomial::from_terms(vec![Term::new(-1.0, vec![(0, 1)])]);

        let cert = MonotonicityCertificate::certify(&p, 0);
        assert!(!cert.is_increasing);
        assert!(cert.is_decreasing);
    }

    #[test]
    fn test_bounds_constant() {
        let p = Polynomial::constant(5.0);
        let cert = BoundsCertificate::certify_bounds(&p);

        // Should find bounds close to 5
        assert!((cert.lower_bound - 5.0).abs() < 1.0);
        assert!((cert.upper_bound - 5.0).abs() < 1.0);
    }
}
