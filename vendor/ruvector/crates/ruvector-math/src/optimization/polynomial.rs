//! Multivariate Polynomials
//!
//! Representation and operations for multivariate polynomials.

use std::collections::HashMap;

/// A monomial: product of variables with powers
/// Represented as sorted list of (variable_index, power)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Monomial {
    /// (variable_index, power) pairs, sorted by variable index
    pub powers: Vec<(usize, usize)>,
}

impl Monomial {
    /// Create constant monomial (1)
    pub fn one() -> Self {
        Self { powers: vec![] }
    }

    /// Create single variable monomial x_i
    pub fn var(i: usize) -> Self {
        Self {
            powers: vec![(i, 1)],
        }
    }

    /// Create from powers (will be sorted)
    pub fn new(mut powers: Vec<(usize, usize)>) -> Self {
        // Sort and merge
        powers.sort_by_key(|&(i, _)| i);

        // Merge duplicate variables
        let mut merged = Vec::new();
        for (i, p) in powers {
            if p == 0 {
                continue;
            }
            if let Some(&mut (last_i, ref mut last_p)) = merged.last_mut() {
                if last_i == i {
                    *last_p += p;
                    continue;
                }
            }
            merged.push((i, p));
        }

        Self { powers: merged }
    }

    /// Total degree
    pub fn degree(&self) -> usize {
        self.powers.iter().map(|&(_, p)| p).sum()
    }

    /// Is this the constant monomial?
    pub fn is_constant(&self) -> bool {
        self.powers.is_empty()
    }

    /// Maximum variable index (or None if constant)
    pub fn max_var(&self) -> Option<usize> {
        self.powers.last().map(|&(i, _)| i)
    }

    /// Multiply two monomials
    pub fn mul(&self, other: &Monomial) -> Monomial {
        let mut combined = self.powers.clone();
        combined.extend(other.powers.iter().copied());
        Monomial::new(combined)
    }

    /// Evaluate at point
    pub fn eval(&self, x: &[f64]) -> f64 {
        let mut result = 1.0;
        for &(i, p) in &self.powers {
            if i < x.len() {
                result *= x[i].powi(p as i32);
            }
        }
        result
    }

    /// Check divisibility: does self divide other?
    pub fn divides(&self, other: &Monomial) -> bool {
        let mut j = 0;
        for &(i, p) in &self.powers {
            // Find matching variable in other
            while j < other.powers.len() && other.powers[j].0 < i {
                j += 1;
            }
            if j >= other.powers.len() || other.powers[j].0 != i || other.powers[j].1 < p {
                return false;
            }
            j += 1;
        }
        true
    }
}

impl std::fmt::Display for Monomial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.powers.is_empty() {
            write!(f, "1")
        } else {
            let parts: Vec<String> = self
                .powers
                .iter()
                .map(|&(i, p)| {
                    if p == 1 {
                        format!("x{}", i)
                    } else {
                        format!("x{}^{}", i, p)
                    }
                })
                .collect();
            write!(f, "{}", parts.join("*"))
        }
    }
}

/// A term: coefficient times monomial
#[derive(Debug, Clone)]
pub struct Term {
    /// Coefficient
    pub coeff: f64,
    /// Monomial
    pub monomial: Monomial,
}

impl Term {
    /// Create term from coefficient and powers
    pub fn new(coeff: f64, powers: Vec<(usize, usize)>) -> Self {
        Self {
            coeff,
            monomial: Monomial::new(powers),
        }
    }

    /// Create constant term
    pub fn constant(c: f64) -> Self {
        Self {
            coeff: c,
            monomial: Monomial::one(),
        }
    }

    /// Degree
    pub fn degree(&self) -> usize {
        self.monomial.degree()
    }
}

/// Multivariate polynomial
#[derive(Debug, Clone)]
pub struct Polynomial {
    /// Terms indexed by monomial
    terms: HashMap<Monomial, f64>,
    /// Cached degree
    degree: usize,
    /// Number of variables
    num_vars: usize,
}

impl Polynomial {
    /// Create zero polynomial
    pub fn zero() -> Self {
        Self {
            terms: HashMap::new(),
            degree: 0,
            num_vars: 0,
        }
    }

    /// Create constant polynomial
    pub fn constant(c: f64) -> Self {
        if c == 0.0 {
            return Self::zero();
        }
        let mut terms = HashMap::new();
        terms.insert(Monomial::one(), c);
        Self {
            terms,
            degree: 0,
            num_vars: 0,
        }
    }

    /// Create single variable polynomial x_i
    pub fn var(i: usize) -> Self {
        let mut terms = HashMap::new();
        terms.insert(Monomial::var(i), 1.0);
        Self {
            terms,
            degree: 1,
            num_vars: i + 1,
        }
    }

    /// Create from terms
    pub fn from_terms(term_list: Vec<Term>) -> Self {
        let mut terms = HashMap::new();
        let mut degree = 0;
        let mut num_vars = 0;

        for term in term_list {
            if term.coeff.abs() < 1e-15 {
                continue;
            }

            degree = degree.max(term.degree());
            if let Some(max_v) = term.monomial.max_var() {
                num_vars = num_vars.max(max_v + 1);
            }

            *terms.entry(term.monomial).or_insert(0.0) += term.coeff;
        }

        // Remove zero terms
        terms.retain(|_, &mut c| c.abs() >= 1e-15);

        Self {
            terms,
            degree,
            num_vars,
        }
    }

    /// Total degree
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// Number of variables (max variable index + 1)
    pub fn num_variables(&self) -> usize {
        self.num_vars
    }

    /// Number of terms
    pub fn num_terms(&self) -> usize {
        self.terms.len()
    }

    /// Is zero polynomial?
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// Get coefficient of monomial
    pub fn coeff(&self, m: &Monomial) -> f64 {
        *self.terms.get(m).unwrap_or(&0.0)
    }

    /// Get all terms
    pub fn terms(&self) -> impl Iterator<Item = (&Monomial, &f64)> {
        self.terms.iter()
    }

    /// Evaluate at point
    pub fn eval(&self, x: &[f64]) -> f64 {
        self.terms.iter().map(|(m, &c)| c * m.eval(x)).sum()
    }

    /// Add two polynomials
    pub fn add(&self, other: &Polynomial) -> Polynomial {
        let mut terms = self.terms.clone();

        for (m, &c) in &other.terms {
            *terms.entry(m.clone()).or_insert(0.0) += c;
        }

        terms.retain(|_, &mut c| c.abs() >= 1e-15);

        let degree = terms.keys().map(|m| m.degree()).max().unwrap_or(0);
        let num_vars = terms
            .keys()
            .filter_map(|m| m.max_var())
            .max()
            .map(|v| v + 1)
            .unwrap_or(0);

        Polynomial {
            terms,
            degree,
            num_vars,
        }
    }

    /// Subtract polynomials
    pub fn sub(&self, other: &Polynomial) -> Polynomial {
        self.add(&other.neg())
    }

    /// Negate polynomial
    pub fn neg(&self) -> Polynomial {
        Polynomial {
            terms: self.terms.iter().map(|(m, &c)| (m.clone(), -c)).collect(),
            degree: self.degree,
            num_vars: self.num_vars,
        }
    }

    /// Multiply by scalar
    pub fn scale(&self, s: f64) -> Polynomial {
        if s.abs() < 1e-15 {
            return Polynomial::zero();
        }

        Polynomial {
            terms: self
                .terms
                .iter()
                .map(|(m, &c)| (m.clone(), s * c))
                .collect(),
            degree: self.degree,
            num_vars: self.num_vars,
        }
    }

    /// Multiply two polynomials
    pub fn mul(&self, other: &Polynomial) -> Polynomial {
        let mut terms = HashMap::new();

        for (m1, &c1) in &self.terms {
            for (m2, &c2) in &other.terms {
                let m = m1.mul(m2);
                *terms.entry(m).or_insert(0.0) += c1 * c2;
            }
        }

        terms.retain(|_, &mut c| c.abs() >= 1e-15);

        let degree = terms.keys().map(|m| m.degree()).max().unwrap_or(0);
        let num_vars = terms
            .keys()
            .filter_map(|m| m.max_var())
            .max()
            .map(|v| v + 1)
            .unwrap_or(0);

        Polynomial {
            terms,
            degree,
            num_vars,
        }
    }

    /// Square polynomial
    pub fn square(&self) -> Polynomial {
        self.mul(self)
    }

    /// Power
    pub fn pow(&self, n: usize) -> Polynomial {
        if n == 0 {
            return Polynomial::constant(1.0);
        }
        if n == 1 {
            return self.clone();
        }

        let mut result = self.clone();
        for _ in 1..n {
            result = result.mul(self);
        }
        result
    }

    /// Generate all monomials up to given degree
    pub fn monomials_up_to_degree(num_vars: usize, max_degree: usize) -> Vec<Monomial> {
        let mut result = vec![Monomial::one()];

        if max_degree == 0 || num_vars == 0 {
            return result;
        }

        // Generate systematically using recursion
        fn generate(
            var: usize,
            num_vars: usize,
            remaining_degree: usize,
            current: Vec<(usize, usize)>,
            result: &mut Vec<Monomial>,
        ) {
            if var >= num_vars {
                result.push(Monomial::new(current));
                return;
            }

            for p in 0..=remaining_degree {
                let mut next = current.clone();
                if p > 0 {
                    next.push((var, p));
                }
                generate(var + 1, num_vars, remaining_degree - p, next, result);
            }
        }

        for d in 1..=max_degree {
            generate(0, num_vars, d, vec![], &mut result);
        }

        // Deduplicate
        result.sort_by(|a, b| {
            a.degree()
                .cmp(&b.degree())
                .then_with(|| a.powers.cmp(&b.powers))
        });
        result.dedup();

        // Ensure only one constant monomial
        let const_count = result.iter().filter(|m| m.is_constant()).count();
        if const_count > 1 {
            let mut seen_const = false;
            result.retain(|m| {
                if m.is_constant() {
                    if seen_const {
                        return false;
                    }
                    seen_const = true;
                }
                true
            });
        }

        result
    }
}

impl std::fmt::Display for Polynomial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.terms.is_empty() {
            return write!(f, "0");
        }

        let mut sorted: Vec<_> = self.terms.iter().collect();
        sorted.sort_by(|a, b| {
            a.0.degree()
                .cmp(&b.0.degree())
                .then_with(|| a.0.powers.cmp(&b.0.powers))
        });

        let parts: Vec<String> = sorted
            .iter()
            .map(|(m, &c)| {
                if m.is_constant() {
                    format!("{:.4}", c)
                } else if (c - 1.0).abs() < 1e-10 {
                    format!("{}", m)
                } else if (c + 1.0).abs() < 1e-10 {
                    format!("-{}", m)
                } else {
                    format!("{:.4}*{}", c, m)
                }
            })
            .collect();

        write!(f, "{}", parts.join(" + "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monomial() {
        let m1 = Monomial::var(0);
        let m2 = Monomial::var(1);
        let m3 = m1.mul(&m2);

        assert_eq!(m3.degree(), 2);
        assert_eq!(m3.powers, vec![(0, 1), (1, 1)]);
    }

    #[test]
    fn test_polynomial_eval() {
        // p = x² + 2xy + y²
        let p = Polynomial::from_terms(vec![
            Term::new(1.0, vec![(0, 2)]),
            Term::new(2.0, vec![(0, 1), (1, 1)]),
            Term::new(1.0, vec![(1, 2)]),
        ]);

        // At (1, 1): 1 + 2 + 1 = 4
        assert!((p.eval(&[1.0, 1.0]) - 4.0).abs() < 1e-10);

        // At (2, 3): 4 + 12 + 9 = 25 = (2+3)²
        assert!((p.eval(&[2.0, 3.0]) - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_polynomial_mul() {
        // (x + y)²  = x² + 2xy + y²
        let x = Polynomial::var(0);
        let y = Polynomial::var(1);
        let sum = x.add(&y);
        let squared = sum.square();

        assert!((squared.coeff(&Monomial::new(vec![(0, 2)])) - 1.0).abs() < 1e-10);
        assert!((squared.coeff(&Monomial::new(vec![(0, 1), (1, 1)])) - 2.0).abs() < 1e-10);
        assert!((squared.coeff(&Monomial::new(vec![(1, 2)])) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_monomials_generation() {
        let monoms = Polynomial::monomials_up_to_degree(2, 2);

        // Should have: 1, x0, x1, x0², x0*x1, x1²
        assert!(monoms.len() >= 6);
    }
}
