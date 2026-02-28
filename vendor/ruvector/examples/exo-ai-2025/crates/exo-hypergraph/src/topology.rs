//! Topological Data Analysis (TDA) structures
//!
//! Implements simplicial complexes, persistent homology computation,
//! and Betti number calculations.

use exo_core::EntityId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A simplex (generalization of triangle to arbitrary dimensions)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Simplex {
    /// Vertices of the simplex
    pub vertices: Vec<EntityId>,
}

impl Simplex {
    /// Create a new simplex from vertices
    pub fn new(mut vertices: Vec<EntityId>) -> Self {
        vertices.sort_by_key(|v| v.0);
        vertices.dedup();
        Self { vertices }
    }

    /// Get the dimension of this simplex (0 for point, 1 for edge, 2 for triangle, etc.)
    pub fn dimension(&self) -> usize {
        self.vertices.len().saturating_sub(1)
    }

    /// Get all faces (sub-simplices) of this simplex
    pub fn faces(&self) -> Vec<Simplex> {
        if self.vertices.is_empty() {
            return vec![];
        }

        let mut faces = Vec::new();

        // Generate all non-empty subsets
        for i in 0..self.vertices.len() {
            let mut face_vertices = self.vertices.clone();
            face_vertices.remove(i);
            if !face_vertices.is_empty() {
                faces.push(Simplex::new(face_vertices));
            }
        }

        faces
    }
}

/// Simplicial complex for topological data analysis
///
/// A simplicial complex is a collection of simplices (points, edges, triangles, etc.)
/// that are "glued together" in a consistent way.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimplicialComplex {
    /// All simplices in the complex, organized by dimension
    simplices: HashMap<usize, HashSet<Simplex>>,
    /// Maximum dimension
    max_dimension: usize,
}

impl SimplicialComplex {
    /// Create a new empty simplicial complex
    pub fn new() -> Self {
        Self {
            simplices: HashMap::new(),
            max_dimension: 0,
        }
    }

    /// Add a simplex and all its faces to the complex
    pub fn add_simplex(&mut self, vertices: &[EntityId]) {
        if vertices.is_empty() {
            return;
        }

        let simplex = Simplex::new(vertices.to_vec());
        let dim = simplex.dimension();

        // Add the simplex itself
        self.simplices
            .entry(dim)
            .or_insert_with(HashSet::new)
            .insert(simplex.clone());

        if dim > self.max_dimension {
            self.max_dimension = dim;
        }

        // Add all faces recursively
        for face in simplex.faces() {
            self.add_simplex(&face.vertices);
        }
    }

    /// Get all simplices of a given dimension
    pub fn get_simplices(&self, dimension: usize) -> Vec<Simplex> {
        self.simplices
            .get(&dimension)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the number of simplices of a given dimension
    pub fn count_simplices(&self, dimension: usize) -> usize {
        self.simplices
            .get(&dimension)
            .map(|set| set.len())
            .unwrap_or(0)
    }

    /// Compute Betti number for a given dimension
    ///
    /// Betti numbers are topological invariants:
    /// - β₀ = number of connected components
    /// - β₁ = number of 1-dimensional holes (loops)
    /// - β₂ = number of 2-dimensional holes (voids)
    ///
    /// This is a simplified stub implementation.
    pub fn betti_number(&self, dimension: usize) -> usize {
        if dimension == 0 {
            // β₀ = number of connected components
            self.count_connected_components()
        } else {
            // For higher dimensions, return 0 (stub - full implementation requires
            // boundary matrix computation and Smith normal form)
            0
        }
    }

    /// Count connected components (β₀)
    fn count_connected_components(&self) -> usize {
        let vertices = self.get_simplices(0);
        if vertices.is_empty() {
            return 0;
        }

        // Union-find to count components
        let mut parent: HashMap<EntityId, EntityId> = HashMap::new();

        // Initialize each vertex as its own component
        for simplex in &vertices {
            if let Some(v) = simplex.vertices.first() {
                parent.insert(*v, *v);
            }
        }

        // Process edges to merge components
        let edges = self.get_simplices(1);
        for edge in edges {
            if edge.vertices.len() == 2 {
                let v1 = edge.vertices[0];
                let v2 = edge.vertices[1];
                self.union(&mut parent, v1, v2);
            }
        }

        // Count unique roots
        let mut roots = HashSet::new();
        for v in parent.keys() {
            roots.insert(self.find(&parent, *v));
        }

        roots.len()
    }

    /// Union-find: find root
    fn find(&self, parent: &HashMap<EntityId, EntityId>, mut x: EntityId) -> EntityId {
        while parent.get(&x) != Some(&x) {
            if let Some(&p) = parent.get(&x) {
                x = p;
            } else {
                break;
            }
        }
        x
    }

    /// Union-find: merge components
    fn union(&self, parent: &mut HashMap<EntityId, EntityId>, x: EntityId, y: EntityId) {
        let root_x = self.find(parent, x);
        let root_y = self.find(parent, y);
        if root_x != root_y {
            parent.insert(root_x, root_y);
        }
    }

    /// Build filtration (nested sequence of complexes) for persistent homology
    ///
    /// This is a stub - a full implementation would assign filtration values
    /// to simplices based on some metric (e.g., edge weights, distances).
    pub fn filtration(&self, _epsilon_range: (f32, f32)) -> Filtration {
        Filtration {
            complexes: vec![],
            epsilon_values: vec![],
        }
    }

    /// Compute persistent homology (stub implementation)
    ///
    /// Returns a persistence diagram showing birth and death of topological features.
    /// This is a placeholder - full implementation requires:
    /// - Building a filtration
    /// - Constructing boundary matrices
    /// - Column reduction algorithm
    pub fn persistent_homology(
        &self,
        _dimension: usize,
        _epsilon_range: (f32, f32),
    ) -> PersistenceDiagram {
        // Stub: return empty diagram
        PersistenceDiagram { pairs: vec![] }
    }
}

impl Default for SimplicialComplex {
    fn default() -> Self {
        Self::new()
    }
}

/// Filtration: nested sequence of simplicial complexes
///
/// Used for persistent homology computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filtration {
    /// Sequence of complexes
    pub complexes: Vec<SimplicialComplex>,
    /// Epsilon values at which complexes change
    pub epsilon_values: Vec<f32>,
}

impl Filtration {
    /// Get birth time of a simplex (stub)
    pub fn birth_time(&self, _simplex_index: usize) -> f32 {
        0.0
    }
}

/// Persistence diagram showing birth and death of topological features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceDiagram {
    /// Birth-death pairs (birth_time, death_time)
    /// death_time = infinity (f32::INFINITY) for features that never die
    pub pairs: Vec<(f32, f32)>,
}

impl PersistenceDiagram {
    /// Get persistent features (those with significant lifetime)
    pub fn significant_features(&self, min_persistence: f32) -> Vec<(f32, f32)> {
        self.pairs
            .iter()
            .filter(|(birth, death)| {
                if death.is_infinite() {
                    true
                } else {
                    death - birth >= min_persistence
                }
            })
            .copied()
            .collect()
    }
}

/// Column reduction for persistent homology (from pseudocode)
///
/// This is the standard algorithm from computational topology.
/// Currently a stub - full implementation requires boundary matrix representation.
#[allow(dead_code)]
fn column_reduction(_matrix: &BoundaryMatrix) -> BoundaryMatrix {
    // Stub implementation
    BoundaryMatrix { columns: vec![] }
}

/// Boundary matrix for homology computation
#[derive(Debug, Clone)]
struct BoundaryMatrix {
    columns: Vec<Vec<usize>>,
}

impl BoundaryMatrix {
    #[allow(dead_code)]
    fn low(&self, _col: usize) -> Option<usize> {
        None
    }

    #[allow(dead_code)]
    fn column(&self, _index: usize) -> Vec<usize> {
        vec![]
    }

    #[allow(dead_code)]
    fn num_cols(&self) -> usize {
        self.columns.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplex_dimension() {
        let e1 = EntityId::new();
        let e2 = EntityId::new();
        let e3 = EntityId::new();

        // 0-simplex (point)
        let s0 = Simplex::new(vec![e1]);
        assert_eq!(s0.dimension(), 0);

        // 1-simplex (edge)
        let s1 = Simplex::new(vec![e1, e2]);
        assert_eq!(s1.dimension(), 1);

        // 2-simplex (triangle)
        let s2 = Simplex::new(vec![e1, e2, e3]);
        assert_eq!(s2.dimension(), 2);
    }

    #[test]
    fn test_simplex_faces() {
        let e1 = EntityId::new();
        let e2 = EntityId::new();
        let e3 = EntityId::new();

        // Triangle has 3 edges as faces
        let triangle = Simplex::new(vec![e1, e2, e3]);
        let faces = triangle.faces();
        assert_eq!(faces.len(), 3);
        assert!(faces.iter().all(|f| f.dimension() == 1));
    }

    #[test]
    fn test_simplicial_complex() {
        let mut complex = SimplicialComplex::new();

        let e1 = EntityId::new();
        let e2 = EntityId::new();
        let e3 = EntityId::new();

        // Add a triangle
        complex.add_simplex(&[e1, e2, e3]);

        // Should have 3 vertices, 3 edges, 1 triangle
        assert_eq!(complex.count_simplices(0), 3);
        assert_eq!(complex.count_simplices(1), 3);
        assert_eq!(complex.count_simplices(2), 1);

        // Connected, so β₀ = 1
        assert_eq!(complex.betti_number(0), 1);
    }

    #[test]
    fn test_betti_number_disconnected() {
        let mut complex = SimplicialComplex::new();

        let e1 = EntityId::new();
        let e2 = EntityId::new();
        let e3 = EntityId::new();
        let e4 = EntityId::new();

        // Add two separate edges (2 components)
        complex.add_simplex(&[e1, e2]);
        complex.add_simplex(&[e3, e4]);

        // Two connected components
        assert_eq!(complex.betti_number(0), 2);
    }
}
