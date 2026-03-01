//! Cocycle and Coboundary Operations
//!
//! Cocycles are the building blocks of cohomology. A cocycle is a cochain
//! that is in the kernel of the coboundary operator.

use super::sheaf::{Sheaf, SheafSection};
use super::simplex::{Cochain, SimplexId, SimplicialComplex};
use crate::substrate::NodeId;
use ndarray::Array1;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A cocycle representing a cohomology class
///
/// A cocycle is a cochain f such that delta(f) = 0
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cocycle {
    /// Degree (dimension) of the cocycle
    pub degree: usize,
    /// Values on simplices
    pub values: HashMap<SimplexId, f64>,
    /// Whether this is a coboundary (trivial cocycle)
    pub is_coboundary: bool,
    /// Norm of the cocycle
    norm: f64,
}

impl Cocycle {
    /// Create a new cocycle
    pub fn new(degree: usize, values: HashMap<SimplexId, f64>) -> Self {
        let norm = values.values().map(|v| v * v).sum::<f64>().sqrt();
        Self {
            degree,
            values,
            is_coboundary: false,
            norm,
        }
    }

    /// Create a zero cocycle
    pub fn zero(degree: usize) -> Self {
        Self {
            degree,
            values: HashMap::new(),
            is_coboundary: false,
            norm: 0.0,
        }
    }

    /// Create a cocycle from a cochain
    pub fn from_cochain(cochain: &Cochain) -> Self {
        Self::new(cochain.dimension, cochain.values.clone())
    }

    /// Get the value on a simplex
    pub fn get(&self, id: SimplexId) -> f64 {
        self.values.get(&id).copied().unwrap_or(0.0)
    }

    /// Set the value on a simplex
    pub fn set(&mut self, id: SimplexId, value: f64) {
        if value.abs() > 1e-10 {
            self.values.insert(id, value);
        } else {
            self.values.remove(&id);
        }
        self.update_norm();
    }

    /// Update the cached norm
    fn update_norm(&mut self) {
        self.norm = self.values.values().map(|v| v * v).sum::<f64>().sqrt();
    }

    /// Get the L2 norm
    pub fn norm(&self) -> f64 {
        self.norm
    }

    /// Normalize the cocycle to unit norm
    pub fn normalize(&mut self) {
        if self.norm > 1e-10 {
            let scale = 1.0 / self.norm;
            for v in self.values.values_mut() {
                *v *= scale;
            }
            self.norm = 1.0;
        }
    }

    /// Add another cocycle
    pub fn add(&mut self, other: &Cocycle) {
        assert_eq!(self.degree, other.degree, "Cocycle degrees must match");
        for (&id, &value) in &other.values {
            let new_val = self.get(id) + value;
            self.set(id, new_val);
        }
    }

    /// Scale the cocycle
    pub fn scale(&mut self, factor: f64) {
        for v in self.values.values_mut() {
            *v *= factor;
        }
        self.norm *= factor.abs();
    }

    /// Check if this is a zero cocycle
    pub fn is_zero(&self, tolerance: f64) -> bool {
        self.norm < tolerance
    }

    /// Inner product with another cocycle
    pub fn inner_product(&self, other: &Cocycle) -> f64 {
        assert_eq!(self.degree, other.degree, "Cocycle degrees must match");
        let mut sum = 0.0;
        for (&id, &v) in &self.values {
            sum += v * other.get(id);
        }
        sum
    }

    /// Convert to cochain
    pub fn to_cochain(&self) -> Cochain {
        Cochain::from_values(self.degree, self.values.clone())
    }
}

/// Coboundary operator delta: C^n -> C^{n+1}
///
/// For a cochain f on n-simplices, delta(f) evaluated on an (n+1)-simplex sigma is:
/// delta(f)(sigma) = sum_{i=0}^{n+1} (-1)^i f(d_i sigma)
/// where d_i sigma is the i-th face of sigma
pub struct Coboundary {
    /// The simplicial complex
    complex: SimplicialComplex,
}

impl Coboundary {
    /// Create a coboundary operator for a simplicial complex
    pub fn new(complex: SimplicialComplex) -> Self {
        Self { complex }
    }

    /// Get reference to the complex
    pub fn complex(&self) -> &SimplicialComplex {
        &self.complex
    }

    /// Apply the coboundary operator to a cochain
    ///
    /// delta: C^n -> C^{n+1}
    pub fn apply(&self, cochain: &Cochain) -> Cochain {
        let target_dim = cochain.dimension + 1;
        let mut result = Cochain::zero(target_dim);

        // For each (n+1)-simplex sigma
        if let Some(target_simplices) = self.complex.simplices.get(&target_dim) {
            for (sigma_id, sigma) in target_simplices {
                // Compute delta(f)(sigma) = sum(-1)^i f(d_i sigma)
                let boundary = sigma.boundary();
                let mut value = 0.0;

                for (face, sign) in &boundary {
                    value += (*sign as f64) * cochain.get(face.id);
                }

                if value.abs() > 1e-10 {
                    result.set(*sigma_id, value);
                }
            }
        }

        result
    }

    /// Apply the adjoint coboundary (negative boundary transpose)
    ///
    /// delta^*: C^{n+1} -> C^n
    pub fn apply_adjoint(&self, cochain: &Cochain) -> Cochain {
        if cochain.dimension == 0 {
            return Cochain::zero(0);
        }

        let target_dim = cochain.dimension - 1;
        let mut result = Cochain::zero(target_dim);

        // For each n-simplex tau, compute sum over (n+1)-simplices containing tau
        if let Some(simplices) = self.complex.simplices.get(&cochain.dimension) {
            for (sigma_id, sigma) in simplices {
                let boundary = sigma.boundary();
                let sigma_value = cochain.get(*sigma_id);

                if sigma_value.abs() > 1e-10 {
                    for (face, sign) in &boundary {
                        let current = result.get(face.id);
                        result.set(face.id, current + (*sign as f64) * sigma_value);
                    }
                }
            }
        }

        result
    }

    /// Check if a cochain is a cocycle (in kernel of delta)
    pub fn is_cocycle(&self, cochain: &Cochain, tolerance: f64) -> bool {
        let delta_f = self.apply(cochain);
        delta_f.norm() < tolerance
    }

    /// Check if a cocycle is a coboundary (in image of delta)
    pub fn is_coboundary(&self, cocycle: &Cocycle, tolerance: f64) -> bool {
        // A cocycle is a coboundary if it's in the image of delta
        // This requires solving delta(g) = f, which is more complex
        // For now, we use a simple check based on dimension
        if cocycle.degree == 0 {
            // 0-cocycles are coboundaries iff they're constant
            let values: Vec<f64> = cocycle.values.values().copied().collect();
            if values.is_empty() {
                return true;
            }
            let first = values[0];
            values.iter().all(|&v| (v - first).abs() < tolerance)
        } else {
            // For higher degrees, we'd need to solve a linear system
            // Returning false as a conservative estimate
            false
        }
    }

    /// Compute the Laplacian L = delta^* delta + delta delta^*
    pub fn laplacian(&self, cochain: &Cochain) -> Cochain {
        // L = delta^* delta + delta delta^*
        let delta_f = self.apply(cochain);
        let delta_star_delta_f = self.apply_adjoint(&delta_f);

        let delta_star_f = self.apply_adjoint(cochain);
        let delta_delta_star_f = self.apply(&delta_star_f);

        let mut result = delta_star_delta_f;
        result.add(&delta_delta_star_f);
        result
    }
}

/// Builder for constructing cocycles
pub struct CocycleBuilder {
    degree: usize,
    values: HashMap<SimplexId, f64>,
}

impl CocycleBuilder {
    /// Create a new builder
    pub fn new(degree: usize) -> Self {
        Self {
            degree,
            values: HashMap::new(),
        }
    }

    /// Set value on a simplex
    pub fn value(mut self, id: SimplexId, value: f64) -> Self {
        self.values.insert(id, value);
        self
    }

    /// Set values from iterator
    pub fn values(mut self, values: impl IntoIterator<Item = (SimplexId, f64)>) -> Self {
        for (id, value) in values {
            self.values.insert(id, value);
        }
        self
    }

    /// Build the cocycle
    pub fn build(self) -> Cocycle {
        Cocycle::new(self.degree, self.values)
    }
}

/// Sheaf-valued cocycle for sheaf cohomology
///
/// Instead of real-valued, this assigns vectors from stalks
#[derive(Debug, Clone)]
pub struct SheafCocycle {
    /// Degree of the cocycle
    pub degree: usize,
    /// Values on simplices (simplex -> vector value)
    pub values: HashMap<SimplexId, Array1<f64>>,
}

impl SheafCocycle {
    /// Create a new sheaf-valued cocycle
    pub fn new(degree: usize) -> Self {
        Self {
            degree,
            values: HashMap::new(),
        }
    }

    /// Set value on a simplex
    pub fn set(&mut self, id: SimplexId, value: Array1<f64>) {
        self.values.insert(id, value);
    }

    /// Get value on a simplex
    pub fn get(&self, id: SimplexId) -> Option<&Array1<f64>> {
        self.values.get(&id)
    }

    /// Compute norm squared
    pub fn norm_squared(&self) -> f64 {
        self.values
            .values()
            .map(|v| v.iter().map(|x| x * x).sum::<f64>())
            .sum()
    }

    /// Compute norm
    pub fn norm(&self) -> f64 {
        self.norm_squared().sqrt()
    }
}

/// Sheaf coboundary operator
///
/// For a sheaf F on a graph, the coboundary uses restriction maps:
/// (delta f)(e) = rho_t(f(t)) - rho_s(f(s))
pub struct SheafCoboundary<'a> {
    /// The sheaf
    sheaf: &'a Sheaf,
    /// Edge list as (source, target) pairs
    edges: Vec<(NodeId, NodeId)>,
}

impl<'a> SheafCoboundary<'a> {
    /// Create a sheaf coboundary operator
    pub fn new(sheaf: &'a Sheaf, edges: Vec<(NodeId, NodeId)>) -> Self {
        Self { sheaf, edges }
    }

    /// Apply sheaf coboundary to a section
    ///
    /// Returns the residual vector at each edge
    pub fn apply(&self, section: &SheafSection) -> SheafCocycle {
        let mut result = SheafCocycle::new(1);

        for (i, &(source, target)) in self.edges.iter().enumerate() {
            if let Some(residual) = self.sheaf.edge_residual(source, target, section) {
                result.set(SimplexId::new(i as u64), residual);
            }
        }

        result
    }

    /// Compute the sheaf Laplacian energy
    pub fn laplacian_energy(&self, section: &SheafSection) -> f64 {
        let delta_s = self.apply(section);
        delta_s.norm_squared()
    }

    /// Check if section is a global section (delta s = 0)
    pub fn is_global_section(&self, section: &SheafSection, tolerance: f64) -> bool {
        let delta_s = self.apply(section);
        delta_s.norm() < tolerance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cohomology::simplex::Simplex;
    use uuid::Uuid;

    fn make_node_id() -> NodeId {
        Uuid::new_v4()
    }

    #[test]
    fn test_cocycle_creation() {
        let mut values = HashMap::new();
        values.insert(SimplexId::new(0), 1.0);
        values.insert(SimplexId::new(1), 2.0);

        let cocycle = Cocycle::new(1, values);
        assert_eq!(cocycle.degree, 1);
        assert!((cocycle.norm() - (5.0_f64).sqrt()).abs() < 1e-10);
    }

    #[test]
    fn test_cocycle_builder() {
        let cocycle = CocycleBuilder::new(1)
            .value(SimplexId::new(0), 3.0)
            .value(SimplexId::new(1), 4.0)
            .build();

        assert!((cocycle.norm() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_coboundary_on_path() {
        // Create a path graph: v0 -- v1 -- v2
        let v0 = make_node_id();
        let v1 = make_node_id();
        let v2 = make_node_id();

        let nodes = vec![v0, v1, v2];
        let edges = vec![(v0, v1), (v1, v2)];

        let complex = SimplicialComplex::from_graph_cliques(&nodes, &edges, 1);
        let coboundary = Coboundary::new(complex);

        // Create a 0-cochain that assigns different values to vertices
        let mut f = Cochain::zero(0);
        for (i, simplex) in coboundary.complex().simplices_of_dim(0).enumerate() {
            f.set(simplex.id, i as f64);
        }

        // Apply coboundary
        let delta_f = coboundary.apply(&f);
        assert_eq!(delta_f.dimension, 1);

        // delta(f) should be non-zero since f is not constant
        assert!(!delta_f.is_zero());
    }

    #[test]
    fn test_constant_cochain_is_cocycle() {
        // Create a triangle
        let v0 = make_node_id();
        let v1 = make_node_id();
        let v2 = make_node_id();

        let nodes = vec![v0, v1, v2];
        let edges = vec![(v0, v1), (v1, v2), (v0, v2)];

        let complex = SimplicialComplex::from_graph_cliques(&nodes, &edges, 2);
        let coboundary = Coboundary::new(complex);

        // Create a constant 0-cochain
        let mut f = Cochain::zero(0);
        for simplex in coboundary.complex().simplices_of_dim(0) {
            f.set(simplex.id, 1.0);
        }

        // Constant function should be a cocycle
        assert!(coboundary.is_cocycle(&f, 1e-10));
    }

    #[test]
    fn test_cocycle_inner_product() {
        let c1 = CocycleBuilder::new(1)
            .value(SimplexId::new(0), 1.0)
            .value(SimplexId::new(1), 0.0)
            .build();

        let c2 = CocycleBuilder::new(1)
            .value(SimplexId::new(0), 0.0)
            .value(SimplexId::new(1), 1.0)
            .build();

        // Orthogonal cocycles
        assert!((c1.inner_product(&c2)).abs() < 1e-10);

        // Self inner product equals norm squared
        assert!((c1.inner_product(&c1) - c1.norm() * c1.norm()).abs() < 1e-10);
    }
}
