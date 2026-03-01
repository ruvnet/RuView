//! Simplicial Complex and Chain Complex Types
//!
//! This module provides the foundational types for simplicial complexes
//! and chain complexes used in cohomology computations.

use crate::substrate::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// Unique identifier for a simplex
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SimplexId(pub u64);

impl SimplexId {
    /// Create a new simplex ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Compute ID from vertex set (deterministic)
    pub fn from_vertices(vertices: &BTreeSet<NodeId>) -> Self {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        for v in vertices {
            v.hash(&mut hasher);
        }
        Self(hasher.finish())
    }
}

/// A simplex in a simplicial complex
///
/// An n-simplex is a set of n+1 vertices. For example:
/// - 0-simplex: a single vertex (node)
/// - 1-simplex: an edge (pair of nodes)
/// - 2-simplex: a triangle (triple of nodes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Simplex {
    /// Unique identifier
    pub id: SimplexId,
    /// Ordered set of vertices (using BTreeSet for canonical ordering)
    pub vertices: BTreeSet<NodeId>,
    /// Dimension of the simplex (number of vertices - 1)
    pub dimension: usize,
    /// Optional weight for weighted computations
    pub weight: f64,
}

impl Simplex {
    /// Create a new simplex from vertices
    pub fn new(vertices: impl IntoIterator<Item = NodeId>) -> Self {
        let vertices: BTreeSet<NodeId> = vertices.into_iter().collect();
        let dimension = if vertices.is_empty() {
            0
        } else {
            vertices.len() - 1
        };
        let id = SimplexId::from_vertices(&vertices);
        Self {
            id,
            vertices,
            dimension,
            weight: 1.0,
        }
    }

    /// Create a simplex with a specific weight
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    /// Get the boundary of this simplex (faces of dimension n-1)
    ///
    /// The boundary of an n-simplex [v0, v1, ..., vn] is the alternating sum:
    /// sum_{i=0}^n (-1)^i [v0, ..., v_{i-1}, v_{i+1}, ..., vn]
    pub fn boundary(&self) -> Vec<(Simplex, i8)> {
        if self.dimension == 0 {
            return Vec::new();
        }

        let vertices: Vec<NodeId> = self.vertices.iter().copied().collect();
        let mut faces = Vec::with_capacity(vertices.len());

        for (i, _) in vertices.iter().enumerate() {
            let mut face_vertices = BTreeSet::new();
            for (j, &v) in vertices.iter().enumerate() {
                if i != j {
                    face_vertices.insert(v);
                }
            }
            let face = Simplex {
                id: SimplexId::from_vertices(&face_vertices),
                vertices: face_vertices,
                dimension: self.dimension - 1,
                weight: self.weight,
            };
            let sign = if i % 2 == 0 { 1i8 } else { -1i8 };
            faces.push((face, sign));
        }

        faces
    }

    /// Check if this simplex contains a given vertex
    pub fn contains_vertex(&self, vertex: NodeId) -> bool {
        self.vertices.contains(&vertex)
    }

    /// Check if this simplex is a face of another simplex
    pub fn is_face_of(&self, other: &Simplex) -> bool {
        self.dimension < other.dimension && self.vertices.is_subset(&other.vertices)
    }

    /// Get the coboundary (simplices that have this as a face)
    /// Note: This requires the containing simplicial complex to compute
    pub fn vertices_as_vec(&self) -> Vec<NodeId> {
        self.vertices.iter().copied().collect()
    }
}

impl PartialEq for Simplex {
    fn eq(&self, other: &Self) -> bool {
        self.vertices == other.vertices
    }
}

impl Eq for Simplex {}

impl Hash for Simplex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use ordered iteration for consistent hashing
        for v in &self.vertices {
            v.hash(state);
        }
    }
}

/// A simplicial complex built from a graph
///
/// Contains simplices of various dimensions and tracks the incidence relations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplicialComplex {
    /// Simplices organized by dimension
    pub simplices: HashMap<usize, HashMap<SimplexId, Simplex>>,
    /// Maximum dimension
    pub max_dimension: usize,
    /// Face relations: simplex -> its faces
    face_map: HashMap<SimplexId, Vec<SimplexId>>,
    /// Coface relations: simplex -> simplices it is a face of
    coface_map: HashMap<SimplexId, Vec<SimplexId>>,
}

impl SimplicialComplex {
    /// Create a new empty simplicial complex
    pub fn new() -> Self {
        Self {
            simplices: HashMap::new(),
            max_dimension: 0,
            face_map: HashMap::new(),
            coface_map: HashMap::new(),
        }
    }

    /// Build a simplicial complex from a graph (flag complex / clique complex)
    ///
    /// The flag complex has an n-simplex for every clique of n+1 vertices
    pub fn from_graph_cliques(
        nodes: &[NodeId],
        edges: &[(NodeId, NodeId)],
        max_dim: usize,
    ) -> Self {
        let mut complex = Self::new();

        // Build adjacency for clique detection
        let mut adjacency: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();
        for &node in nodes {
            adjacency.insert(node, HashSet::new());
        }
        for &(u, v) in edges {
            adjacency.entry(u).or_default().insert(v);
            adjacency.entry(v).or_default().insert(u);
        }

        // Add 0-simplices (vertices)
        for &node in nodes {
            let simplex = Simplex::new([node]);
            complex.add_simplex(simplex);
        }

        // Add 1-simplices (edges)
        for &(u, v) in edges {
            let simplex = Simplex::new([u, v]);
            complex.add_simplex(simplex);
        }

        // Find higher-dimensional cliques using Bron-Kerbosch algorithm
        if max_dim >= 2 {
            let all_nodes: HashSet<NodeId> = nodes.iter().copied().collect();
            Self::find_cliques_recursive(
                &mut complex,
                &adjacency,
                BTreeSet::new(),
                all_nodes,
                HashSet::new(),
                max_dim,
            );
        }

        complex.build_incidence_maps();
        complex
    }

    /// Bron-Kerbosch algorithm for finding cliques
    fn find_cliques_recursive(
        complex: &mut SimplicialComplex,
        adjacency: &HashMap<NodeId, HashSet<NodeId>>,
        r: BTreeSet<NodeId>,
        mut p: HashSet<NodeId>,
        mut x: HashSet<NodeId>,
        max_dim: usize,
    ) {
        if p.is_empty() && x.is_empty() {
            if r.len() >= 3 && r.len() <= max_dim + 1 {
                let simplex = Simplex::new(r.iter().copied());
                complex.add_simplex(simplex);
            }
            return;
        }

        let pivot = p.iter().chain(x.iter()).next().copied();
        if let Some(pivot_node) = pivot {
            let pivot_neighbors = adjacency.get(&pivot_node).cloned().unwrap_or_default();
            let candidates: Vec<NodeId> = p.difference(&pivot_neighbors).copied().collect();

            for v in candidates {
                let v_neighbors = adjacency.get(&v).cloned().unwrap_or_default();
                let mut new_r = r.clone();
                new_r.insert(v);
                let new_p: HashSet<NodeId> = p.intersection(&v_neighbors).copied().collect();
                let new_x: HashSet<NodeId> = x.intersection(&v_neighbors).copied().collect();

                Self::find_cliques_recursive(complex, adjacency, new_r, new_p, new_x, max_dim);

                p.remove(&v);
                x.insert(v);
            }
        }
    }

    /// Add a simplex to the complex
    pub fn add_simplex(&mut self, simplex: Simplex) {
        let dim = simplex.dimension;
        self.max_dimension = self.max_dimension.max(dim);
        self.simplices
            .entry(dim)
            .or_default()
            .insert(simplex.id, simplex);
    }

    /// Build the face and coface incidence maps
    fn build_incidence_maps(&mut self) {
        self.face_map.clear();
        self.coface_map.clear();

        // For each simplex, compute its faces
        for dim in 1..=self.max_dimension {
            if let Some(simplices) = self.simplices.get(&dim) {
                for (id, simplex) in simplices {
                    let faces = simplex.boundary();
                    let face_ids: Vec<SimplexId> = faces.iter().map(|(f, _)| f.id).collect();
                    self.face_map.insert(*id, face_ids.clone());

                    // Update coface map
                    for face_id in face_ids {
                        self.coface_map.entry(face_id).or_default().push(*id);
                    }
                }
            }
        }
    }

    /// Get simplices of a specific dimension
    pub fn simplices_of_dim(&self, dim: usize) -> impl Iterator<Item = &Simplex> {
        self.simplices
            .get(&dim)
            .into_iter()
            .flat_map(|s| s.values())
    }

    /// Get a simplex by ID
    pub fn get_simplex(&self, id: SimplexId) -> Option<&Simplex> {
        for simplices in self.simplices.values() {
            if let Some(s) = simplices.get(&id) {
                return Some(s);
            }
        }
        None
    }

    /// Get the faces of a simplex
    pub fn faces(&self, id: SimplexId) -> Option<&[SimplexId]> {
        self.face_map.get(&id).map(|v| v.as_slice())
    }

    /// Get the cofaces (simplices that have this as a face)
    pub fn cofaces(&self, id: SimplexId) -> Option<&[SimplexId]> {
        self.coface_map.get(&id).map(|v| v.as_slice())
    }

    /// Count simplices of each dimension
    pub fn simplex_counts(&self) -> Vec<usize> {
        (0..=self.max_dimension)
            .map(|d| self.simplices.get(&d).map(|s| s.len()).unwrap_or(0))
            .collect()
    }

    /// Total number of simplices
    pub fn total_simplices(&self) -> usize {
        self.simplices.values().map(|s| s.len()).sum()
    }

    /// Euler characteristic: sum(-1)^n * |K_n|
    pub fn euler_characteristic(&self) -> i64 {
        let mut chi = 0i64;
        for (dim, simplices) in &self.simplices {
            let count = simplices.len() as i64;
            if dim % 2 == 0 {
                chi += count;
            } else {
                chi -= count;
            }
        }
        chi
    }
}

impl Default for SimplicialComplex {
    fn default() -> Self {
        Self::new()
    }
}

/// A chain in the chain complex C_n(K)
///
/// Represents a formal sum of n-simplices with coefficients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chain {
    /// Dimension of the chain
    pub dimension: usize,
    /// Simplex coefficients (simplex ID -> coefficient)
    pub coefficients: HashMap<SimplexId, f64>,
}

impl Chain {
    /// Create a zero chain of given dimension
    pub fn zero(dimension: usize) -> Self {
        Self {
            dimension,
            coefficients: HashMap::new(),
        }
    }

    /// Create a chain from a single simplex
    pub fn from_simplex(simplex: &Simplex, coefficient: f64) -> Self {
        let mut coefficients = HashMap::new();
        if coefficient.abs() > 1e-10 {
            coefficients.insert(simplex.id, coefficient);
        }
        Self {
            dimension: simplex.dimension,
            coefficients,
        }
    }

    /// Add a simplex to the chain
    pub fn add_simplex(&mut self, id: SimplexId, coefficient: f64) {
        if coefficient.abs() > 1e-10 {
            *self.coefficients.entry(id).or_insert(0.0) += coefficient;
            // Remove if coefficient is now essentially zero
            if self
                .coefficients
                .get(&id)
                .map(|c| c.abs() < 1e-10)
                .unwrap_or(false)
            {
                self.coefficients.remove(&id);
            }
        }
    }

    /// Scale the chain by a constant
    pub fn scale(&mut self, factor: f64) {
        for coeff in self.coefficients.values_mut() {
            *coeff *= factor;
        }
    }

    /// Add another chain to this one
    pub fn add(&mut self, other: &Chain) {
        assert_eq!(
            self.dimension, other.dimension,
            "Chain dimensions must match"
        );
        for (&id, &coeff) in &other.coefficients {
            self.add_simplex(id, coeff);
        }
    }

    /// Check if chain is zero
    pub fn is_zero(&self) -> bool {
        self.coefficients.is_empty()
    }

    /// L2 norm of the chain
    pub fn norm(&self) -> f64 {
        self.coefficients
            .values()
            .map(|c| c * c)
            .sum::<f64>()
            .sqrt()
    }
}

/// A cochain in the cochain complex C^n(K)
///
/// Represents a function from n-simplices to R
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cochain {
    /// Dimension of the cochain
    pub dimension: usize,
    /// Values on simplices (simplex ID -> value)
    pub values: HashMap<SimplexId, f64>,
}

impl Cochain {
    /// Create a zero cochain of given dimension
    pub fn zero(dimension: usize) -> Self {
        Self {
            dimension,
            values: HashMap::new(),
        }
    }

    /// Create a cochain from values
    pub fn from_values(dimension: usize, values: HashMap<SimplexId, f64>) -> Self {
        Self { dimension, values }
    }

    /// Set the value on a simplex
    pub fn set(&mut self, id: SimplexId, value: f64) {
        if value.abs() > 1e-10 {
            self.values.insert(id, value);
        } else {
            self.values.remove(&id);
        }
    }

    /// Get the value on a simplex
    pub fn get(&self, id: SimplexId) -> f64 {
        self.values.get(&id).copied().unwrap_or(0.0)
    }

    /// Evaluate the cochain on a chain (inner product)
    pub fn evaluate(&self, chain: &Chain) -> f64 {
        assert_eq!(self.dimension, chain.dimension, "Dimensions must match");
        let mut sum = 0.0;
        for (&id, &coeff) in &chain.coefficients {
            sum += coeff * self.get(id);
        }
        sum
    }

    /// Add another cochain to this one
    pub fn add(&mut self, other: &Cochain) {
        assert_eq!(
            self.dimension, other.dimension,
            "Cochain dimensions must match"
        );
        for (&id, &value) in &other.values {
            let new_val = self.get(id) + value;
            self.set(id, new_val);
        }
    }

    /// Scale the cochain
    pub fn scale(&mut self, factor: f64) {
        for value in self.values.values_mut() {
            *value *= factor;
        }
    }

    /// L2 norm of the cochain
    pub fn norm(&self) -> f64 {
        self.values.values().map(|v| v * v).sum::<f64>().sqrt()
    }

    /// Check if cochain is zero
    pub fn is_zero(&self) -> bool {
        self.values.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_node_id() -> NodeId {
        Uuid::new_v4()
    }

    #[test]
    fn test_simplex_creation() {
        let v0 = make_node_id();
        let v1 = make_node_id();
        let v2 = make_node_id();

        let vertex = Simplex::new([v0]);
        assert_eq!(vertex.dimension, 0);

        let edge = Simplex::new([v0, v1]);
        assert_eq!(edge.dimension, 1);

        let triangle = Simplex::new([v0, v1, v2]);
        assert_eq!(triangle.dimension, 2);
    }

    #[test]
    fn test_simplex_boundary() {
        let v0 = make_node_id();
        let v1 = make_node_id();
        let v2 = make_node_id();

        // Boundary of edge [v0, v1] = v1 - v0
        let edge = Simplex::new([v0, v1]);
        let boundary = edge.boundary();
        assert_eq!(boundary.len(), 2);

        // Boundary of triangle [v0, v1, v2] = [v1,v2] - [v0,v2] + [v0,v1]
        let triangle = Simplex::new([v0, v1, v2]);
        let boundary = triangle.boundary();
        assert_eq!(boundary.len(), 3);
    }

    #[test]
    fn test_simplicial_complex() {
        let v0 = make_node_id();
        let v1 = make_node_id();
        let v2 = make_node_id();

        let nodes = vec![v0, v1, v2];
        let edges = vec![(v0, v1), (v1, v2), (v0, v2)];

        let complex = SimplicialComplex::from_graph_cliques(&nodes, &edges, 2);

        // Should have 3 vertices, 3 edges, 1 triangle
        let counts = complex.simplex_counts();
        assert_eq!(counts[0], 3);
        assert_eq!(counts[1], 3);
        assert_eq!(counts[2], 1);

        // Euler characteristic: 3 - 3 + 1 = 1
        assert_eq!(complex.euler_characteristic(), 1);
    }

    #[test]
    fn test_chain_operations() {
        let v0 = make_node_id();
        let v1 = make_node_id();

        let simplex = Simplex::new([v0, v1]);
        let mut chain = Chain::from_simplex(&simplex, 2.0);

        assert_eq!(chain.dimension, 1);
        assert!(!chain.is_zero());

        chain.scale(0.5);
        assert_eq!(chain.coefficients.get(&simplex.id), Some(&1.0));
    }

    #[test]
    fn test_cochain_evaluation() {
        let v0 = make_node_id();
        let v1 = make_node_id();

        let simplex = Simplex::new([v0, v1]);
        let chain = Chain::from_simplex(&simplex, 3.0);

        let mut cochain = Cochain::zero(1);
        cochain.set(simplex.id, 2.0);

        // Inner product: 3.0 * 2.0 = 6.0
        assert!((cochain.evaluate(&chain) - 6.0).abs() < 1e-10);
    }
}
