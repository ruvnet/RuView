//! Topological Invariants
//!
//! Computes topological invariants including Betti numbers, Euler characteristic,
//! and homology/cohomology groups.

use super::simplicial_complex::{Simplex, SimplicialComplex, SparseMatrix};
use super::{constants, QuantumTopologyError, Result};
use std::collections::HashMap;

/// A cycle (representative element of homology)
#[derive(Debug, Clone)]
pub struct Cycle {
    /// Simplices in the cycle with their coefficients
    pub simplices: Vec<(Simplex, i32)>,
    /// Dimension of the cycle
    pub dimension: usize,
}

impl Cycle {
    /// Create a new cycle
    pub fn new(simplices: Vec<(Simplex, i32)>, dimension: usize) -> Self {
        Self { simplices, dimension }
    }

    /// Check if the cycle is trivial (empty)
    pub fn is_trivial(&self) -> bool {
        self.simplices.is_empty()
    }

    /// Number of simplices in the cycle
    pub fn size(&self) -> usize {
        self.simplices.len()
    }
}

/// Homology group H_k(X; R) for some coefficient ring R
#[derive(Debug, Clone)]
pub struct HomologyGroup {
    /// Dimension k
    pub dimension: usize,
    /// Rank (free part) - equals Betti number for field coefficients
    pub rank: usize,
    /// Torsion coefficients (for integer homology)
    pub torsion: Vec<usize>,
    /// Representative cycles (generators)
    pub generators: Vec<Cycle>,
}

impl HomologyGroup {
    /// Create a trivial homology group
    pub fn trivial(dimension: usize) -> Self {
        Self {
            dimension,
            rank: 0,
            torsion: vec![],
            generators: vec![],
        }
    }

    /// Create a free homology group of given rank
    pub fn free(dimension: usize, rank: usize) -> Self {
        Self {
            dimension,
            rank,
            torsion: vec![],
            generators: vec![],
        }
    }

    /// Check if the group is trivial
    pub fn is_trivial(&self) -> bool {
        self.rank == 0 && self.torsion.is_empty()
    }

    /// Total rank including torsion
    pub fn total_rank(&self) -> usize {
        self.rank + self.torsion.len()
    }
}

/// A cocycle (representative element of cohomology)
#[derive(Debug, Clone)]
pub struct Cocycle {
    /// Values on simplices
    pub values: HashMap<Simplex, f64>,
    /// Dimension of the cocycle
    pub dimension: usize,
}

impl Cocycle {
    /// Create a new cocycle
    pub fn new(values: HashMap<Simplex, f64>, dimension: usize) -> Self {
        Self { values, dimension }
    }

    /// Create zero cocycle
    pub fn zero(dimension: usize) -> Self {
        Self {
            values: HashMap::new(),
            dimension,
        }
    }

    /// Evaluate on a simplex
    pub fn evaluate(&self, simplex: &Simplex) -> f64 {
        *self.values.get(simplex).unwrap_or(&0.0)
    }

    /// Add two cocycles
    pub fn add(&self, other: &Cocycle) -> Result<Cocycle> {
        if self.dimension != other.dimension {
            return Err(QuantumTopologyError::DimensionMismatch {
                expected: self.dimension,
                got: other.dimension,
            });
        }

        let mut values = self.values.clone();
        for (simplex, value) in &other.values {
            *values.entry(simplex.clone()).or_insert(0.0) += value;
        }

        // Remove zeros
        values.retain(|_, v| v.abs() > constants::EPSILON);

        Ok(Cocycle {
            values,
            dimension: self.dimension,
        })
    }

    /// Scale the cocycle
    pub fn scale(&self, factor: f64) -> Cocycle {
        Cocycle {
            values: self
                .values
                .iter()
                .map(|(s, v)| (s.clone(), v * factor))
                .collect(),
            dimension: self.dimension,
        }
    }

    /// L2 norm squared
    pub fn norm_squared(&self) -> f64 {
        self.values.values().map(|v| v * v).sum()
    }
}

/// Cohomology group H^k(X; R)
#[derive(Debug, Clone)]
pub struct CohomologyGroup {
    /// Dimension k
    pub dimension: usize,
    /// Rank
    pub rank: usize,
    /// Torsion coefficients
    pub torsion: Vec<usize>,
    /// Representative cocycles (generators)
    pub generators: Vec<Cocycle>,
}

impl CohomologyGroup {
    /// Create a trivial cohomology group
    pub fn trivial(dimension: usize) -> Self {
        Self {
            dimension,
            rank: 0,
            torsion: vec![],
            generators: vec![],
        }
    }

    /// Create a free cohomology group
    pub fn free(dimension: usize, rank: usize) -> Self {
        Self {
            dimension,
            rank,
            torsion: vec![],
            generators: vec![],
        }
    }

    /// Check if trivial
    pub fn is_trivial(&self) -> bool {
        self.rank == 0 && self.torsion.is_empty()
    }
}

/// Topological invariant collection for a space
#[derive(Debug, Clone)]
pub struct TopologicalInvariant {
    /// Betti numbers β_0, β_1, β_2, ...
    pub betti_numbers: Vec<usize>,
    /// Euler characteristic χ = Σ (-1)^k β_k
    pub euler_characteristic: i64,
    /// Homology groups H_k
    pub homology_groups: Vec<HomologyGroup>,
    /// Cohomology groups H^k (optional)
    pub cohomology_groups: Vec<CohomologyGroup>,
}

impl TopologicalInvariant {
    /// Compute invariants from a simplicial complex
    pub fn from_complex(complex: &SimplicialComplex) -> Self {
        let betti_numbers = complex.betti_numbers();
        let euler_characteristic = complex.euler_characteristic();

        // Compute homology groups
        let mut homology_groups = Vec::new();
        for (k, &betti) in betti_numbers.iter().enumerate() {
            let generators = complex.homology_generators(k);
            let cycles: Vec<Cycle> = generators
                .into_iter()
                .map(|simplices| {
                    let with_coeffs: Vec<(Simplex, i32)> =
                        simplices.into_iter().map(|s| (s, 1)).collect();
                    Cycle::new(with_coeffs, k)
                })
                .collect();

            homology_groups.push(HomologyGroup {
                dimension: k,
                rank: betti,
                torsion: vec![], // Computing torsion requires more work
                generators: cycles,
            });
        }

        // Cohomology is dual to homology (for field coefficients)
        let cohomology_groups = homology_groups
            .iter()
            .map(|h| CohomologyGroup {
                dimension: h.dimension,
                rank: h.rank,
                torsion: h.torsion.clone(),
                generators: vec![],
            })
            .collect();

        Self {
            betti_numbers,
            euler_characteristic,
            homology_groups,
            cohomology_groups,
        }
    }

    /// Create from pre-computed Betti numbers
    pub fn from_betti(betti_numbers: Vec<usize>) -> Self {
        let euler_characteristic: i64 = betti_numbers
            .iter()
            .enumerate()
            .map(|(k, &b)| {
                let sign = if k % 2 == 0 { 1 } else { -1 };
                sign * b as i64
            })
            .sum();

        let homology_groups = betti_numbers
            .iter()
            .enumerate()
            .map(|(k, &b)| HomologyGroup::free(k, b))
            .collect();

        let cohomology_groups = betti_numbers
            .iter()
            .enumerate()
            .map(|(k, &b)| CohomologyGroup::free(k, b))
            .collect();

        Self {
            betti_numbers,
            euler_characteristic,
            homology_groups,
            cohomology_groups,
        }
    }

    /// Get β_k
    pub fn betti(&self, k: usize) -> usize {
        *self.betti_numbers.get(k).unwrap_or(&0)
    }

    /// Total Betti number sum
    pub fn total_betti(&self) -> usize {
        self.betti_numbers.iter().sum()
    }

    /// Maximum dimension with non-trivial homology
    pub fn homological_dimension(&self) -> usize {
        self.betti_numbers
            .iter()
            .enumerate()
            .rev()
            .find(|(_, &b)| b > 0)
            .map(|(k, _)| k)
            .unwrap_or(0)
    }

    /// Check if simply connected (β_1 = 0)
    pub fn is_simply_connected(&self) -> bool {
        self.betti(1) == 0
    }

    /// Check if connected (β_0 = 1)
    pub fn is_connected(&self) -> bool {
        self.betti(0) == 1
    }

    /// Compute cup product (at cohomology level)
    pub fn cup_product(&self, alpha: &Cocycle, beta: &Cocycle) -> Cocycle {
        // Cup product α ∪ β has dimension dim(α) + dim(β)
        // Simplified implementation - returns empty cocycle
        Cocycle::zero(alpha.dimension + beta.dimension)
    }

    /// Compare with another topological invariant
    pub fn distance(&self, other: &TopologicalInvariant) -> f64 {
        // Sum of absolute differences in Betti numbers
        let max_len = self.betti_numbers.len().max(other.betti_numbers.len());
        let mut dist = 0.0;

        for k in 0..max_len {
            let b1 = *self.betti_numbers.get(k).unwrap_or(&0) as f64;
            let b2 = *other.betti_numbers.get(k).unwrap_or(&0) as f64;
            dist += (b1 - b2).abs();
        }

        // Add Euler characteristic difference
        dist += (self.euler_characteristic - other.euler_characteristic).abs() as f64;

        dist
    }
}

/// Compute topological invariants from a point cloud
pub fn compute_topological_invariants(
    points: &[Vec<f64>],
    max_dimension: usize,
    max_radius: f64,
) -> TopologicalInvariant {
    // Build Vietoris-Rips complex
    let complex = build_vietoris_rips(points, max_dimension, max_radius);
    TopologicalInvariant::from_complex(&complex)
}

/// Build a Vietoris-Rips complex from a point cloud
fn build_vietoris_rips(
    points: &[Vec<f64>],
    max_dimension: usize,
    max_radius: f64,
) -> SimplicialComplex {
    let n = points.len();
    let mut complex = SimplicialComplex::new();

    // Add vertices
    for i in 0..n {
        complex.add_simplex(Simplex::vertex(i));
    }

    // Compute pairwise distances and add edges
    let mut edges = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            let dist = euclidean_distance(&points[i], &points[j]);
            if dist <= 2.0 * max_radius {
                edges.push((i, j));
                complex.add_simplex(Simplex::edge(i, j));
            }
        }
    }

    // Build higher simplices using clique enumeration
    if max_dimension >= 2 {
        // Build adjacency list
        let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
        for &(i, j) in &edges {
            adj[i].push(j);
            adj[j].push(i);
        }

        // Find triangles
        for &(i, j) in &edges {
            let common: Vec<usize> = adj[i]
                .iter()
                .filter(|&&k| k > j && adj[j].contains(&k))
                .copied()
                .collect();

            for k in common {
                complex.add_simplex(Simplex::triangle(i, j, k));

                // Find tetrahedra (if max_dimension >= 3)
                if max_dimension >= 3 {
                    let common_3: Vec<usize> = adj[i]
                        .iter()
                        .filter(|&&l| l > k && adj[j].contains(&l) && adj[k].contains(&l))
                        .copied()
                        .collect();

                    for l in common_3 {
                        complex.add_simplex(Simplex::tetrahedron(i, j, k, l));
                    }
                }
            }
        }
    }

    complex
}

/// Euclidean distance between two points
fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// Compute Alexander polynomial (for knots - simplified)
#[derive(Debug, Clone)]
pub struct AlexanderPolynomial {
    /// Coefficients (a_0 + a_1*t + a_2*t^2 + ...)
    pub coefficients: Vec<i32>,
}

impl AlexanderPolynomial {
    /// Create from coefficients
    pub fn new(coefficients: Vec<i32>) -> Self {
        Self { coefficients }
    }

    /// Trivial polynomial (unknot)
    pub fn trivial() -> Self {
        Self {
            coefficients: vec![1],
        }
    }

    /// Trefoil knot
    pub fn trefoil() -> Self {
        Self {
            coefficients: vec![1, -1, 1],
        }
    }

    /// Figure-8 knot
    pub fn figure_eight() -> Self {
        Self {
            coefficients: vec![-1, 3, -1],
        }
    }

    /// Evaluate at t
    pub fn evaluate(&self, t: f64) -> f64 {
        let mut result = 0.0;
        let mut t_power = 1.0;

        for &coef in &self.coefficients {
            result += coef as f64 * t_power;
            t_power *= t;
        }

        result
    }

    /// Degree of the polynomial
    pub fn degree(&self) -> usize {
        if self.coefficients.is_empty() {
            0
        } else {
            self.coefficients.len() - 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topological_invariant_triangle() {
        let complex = SimplicialComplex::from_simplices([Simplex::triangle(0, 1, 2)]);
        let invariant = TopologicalInvariant::from_complex(&complex);

        // Filled triangle: β_0 = 1 (connected), β_1 = 0 (no holes)
        assert_eq!(invariant.betti(0), 1);
        assert_eq!(invariant.betti(1), 0);
        assert_eq!(invariant.euler_characteristic, 1);
    }

    #[test]
    fn test_topological_invariant_circle() {
        let mut complex = SimplicialComplex::new();
        complex.add_simplex(Simplex::edge(0, 1));
        complex.add_simplex(Simplex::edge(1, 2));
        complex.add_simplex(Simplex::edge(0, 2));

        let invariant = TopologicalInvariant::from_complex(&complex);

        // Circle: β_0 = 1, β_1 = 1 (one hole)
        assert_eq!(invariant.betti(0), 1);
        assert_eq!(invariant.betti(1), 1);
        assert_eq!(invariant.euler_characteristic, 0);
    }

    #[test]
    fn test_homology_group() {
        let h = HomologyGroup::free(1, 2);
        assert_eq!(h.rank, 2);
        assert!(!h.is_trivial());

        let trivial = HomologyGroup::trivial(0);
        assert!(trivial.is_trivial());
    }

    #[test]
    fn test_cocycle_operations() {
        let mut values1 = HashMap::new();
        values1.insert(Simplex::edge(0, 1), 1.0);
        let alpha = Cocycle::new(values1, 1);

        let mut values2 = HashMap::new();
        values2.insert(Simplex::edge(0, 1), 2.0);
        let beta = Cocycle::new(values2, 1);

        let sum = alpha.add(&beta).unwrap();
        assert!((sum.evaluate(&Simplex::edge(0, 1)) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_invariant_distance() {
        let inv1 = TopologicalInvariant::from_betti(vec![1, 0, 0]);
        let inv2 = TopologicalInvariant::from_betti(vec![1, 1, 0]);

        let dist = inv1.distance(&inv2);
        assert!((dist - 2.0).abs() < 1e-10); // β_1 differs by 1, χ differs by 1
    }

    #[test]
    fn test_alexander_polynomial() {
        let trefoil = AlexanderPolynomial::trefoil();
        assert_eq!(trefoil.degree(), 2);

        // Δ(1) = 1 - 1 + 1 = 1 for trefoil
        assert!((trefoil.evaluate(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_vietoris_rips() {
        // Three points forming a triangle
        let points = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.5, 0.866],
        ];

        let invariant = compute_topological_invariants(&points, 2, 0.6);

        // With radius 0.6, should form complete triangle
        assert_eq!(invariant.betti(0), 1);
    }
}
