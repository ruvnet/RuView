//! Simplicial Complexes
//!
//! Basic building blocks for topological data analysis.

use std::collections::{HashMap, HashSet};

/// A simplex (k-simplex has k+1 vertices)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Simplex {
    /// Sorted vertex indices
    pub vertices: Vec<usize>,
}

impl Simplex {
    /// Create simplex from vertices (will be sorted)
    pub fn new(mut vertices: Vec<usize>) -> Self {
        vertices.sort_unstable();
        vertices.dedup();
        Self { vertices }
    }

    /// Create 0-simplex (vertex)
    pub fn vertex(v: usize) -> Self {
        Self { vertices: vec![v] }
    }

    /// Create 1-simplex (edge)
    pub fn edge(v0: usize, v1: usize) -> Self {
        Self::new(vec![v0, v1])
    }

    /// Create 2-simplex (triangle)
    pub fn triangle(v0: usize, v1: usize, v2: usize) -> Self {
        Self::new(vec![v0, v1, v2])
    }

    /// Dimension of simplex (0 = vertex, 1 = edge, 2 = triangle, ...)
    pub fn dim(&self) -> usize {
        if self.vertices.is_empty() {
            0
        } else {
            self.vertices.len() - 1
        }
    }

    /// Is this a vertex (0-simplex)?
    pub fn is_vertex(&self) -> bool {
        self.vertices.len() == 1
    }

    /// Is this an edge (1-simplex)?
    pub fn is_edge(&self) -> bool {
        self.vertices.len() == 2
    }

    /// Get all faces (boundary simplices)
    pub fn faces(&self) -> Vec<Simplex> {
        if self.vertices.len() <= 1 {
            return vec![];
        }

        (0..self.vertices.len())
            .map(|i| {
                let face_verts: Vec<usize> = self
                    .vertices
                    .iter()
                    .enumerate()
                    .filter(|&(j, _)| j != i)
                    .map(|(_, &v)| v)
                    .collect();
                Simplex::new(face_verts)
            })
            .collect()
    }

    /// Check if this simplex is a face of another
    pub fn is_face_of(&self, other: &Simplex) -> bool {
        if self.vertices.len() >= other.vertices.len() {
            return false;
        }
        self.vertices.iter().all(|v| other.vertices.contains(v))
    }

    /// Check if two simplices share a face
    pub fn shares_face_with(&self, other: &Simplex) -> bool {
        let intersection: Vec<usize> = self
            .vertices
            .iter()
            .filter(|v| other.vertices.contains(v))
            .copied()
            .collect();
        !intersection.is_empty()
    }
}

/// Simplicial complex (collection of simplices)
#[derive(Debug, Clone)]
pub struct SimplicialComplex {
    /// Simplices organized by dimension
    simplices: Vec<HashSet<Simplex>>,
    /// Maximum dimension
    max_dim: usize,
}

impl SimplicialComplex {
    /// Create empty complex
    pub fn new() -> Self {
        Self {
            simplices: vec![HashSet::new()],
            max_dim: 0,
        }
    }

    /// Create from list of simplices (automatically adds faces)
    pub fn from_simplices(simplices: Vec<Simplex>) -> Self {
        let mut complex = Self::new();
        for s in simplices {
            complex.add(s);
        }
        complex
    }

    /// Add simplex and all its faces
    pub fn add(&mut self, simplex: Simplex) {
        let dim = simplex.dim();

        // Ensure we have enough dimension levels
        while self.simplices.len() <= dim {
            self.simplices.push(HashSet::new());
        }
        self.max_dim = self.max_dim.max(dim);

        // Add all faces recursively
        self.add_with_faces(simplex);
    }

    fn add_with_faces(&mut self, simplex: Simplex) {
        let dim = simplex.dim();

        if self.simplices[dim].contains(&simplex) {
            return; // Already present
        }

        // Add faces first
        for face in simplex.faces() {
            self.add_with_faces(face);
        }

        // Add this simplex
        self.simplices[dim].insert(simplex);
    }

    /// Check if simplex is in complex
    pub fn contains(&self, simplex: &Simplex) -> bool {
        let dim = simplex.dim();
        if dim >= self.simplices.len() {
            return false;
        }
        self.simplices[dim].contains(simplex)
    }

    /// Get all simplices of dimension d
    pub fn simplices_of_dim(&self, d: usize) -> impl Iterator<Item = &Simplex> {
        self.simplices.get(d).into_iter().flat_map(|s| s.iter())
    }

    /// Get all simplices
    pub fn all_simplices(&self) -> impl Iterator<Item = &Simplex> {
        self.simplices.iter().flat_map(|s| s.iter())
    }

    /// Number of simplices of dimension d
    pub fn count_dim(&self, d: usize) -> usize {
        self.simplices.get(d).map(|s| s.len()).unwrap_or(0)
    }

    /// Total number of simplices
    pub fn size(&self) -> usize {
        self.simplices.iter().map(|s| s.len()).sum()
    }

    /// Maximum dimension
    pub fn dimension(&self) -> usize {
        self.max_dim
    }

    /// f-vector: (f_0, f_1, f_2, ...) = counts of each dimension
    pub fn f_vector(&self) -> Vec<usize> {
        self.simplices.iter().map(|s| s.len()).collect()
    }

    /// Euler characteristic via f-vector
    pub fn euler_characteristic(&self) -> i64 {
        self.simplices
            .iter()
            .enumerate()
            .map(|(d, s)| {
                let sign = if d % 2 == 0 { 1 } else { -1 };
                sign * s.len() as i64
            })
            .sum()
    }

    /// Get vertex set
    pub fn vertices(&self) -> HashSet<usize> {
        self.simplices_of_dim(0)
            .flat_map(|s| s.vertices.iter().copied())
            .collect()
    }

    /// Get edges as pairs
    pub fn edges(&self) -> Vec<(usize, usize)> {
        self.simplices_of_dim(1)
            .filter_map(|s| {
                if s.vertices.len() == 2 {
                    Some((s.vertices[0], s.vertices[1]))
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for SimplicialComplex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplex_creation() {
        let vertex = Simplex::vertex(0);
        assert_eq!(vertex.dim(), 0);

        let edge = Simplex::edge(0, 1);
        assert_eq!(edge.dim(), 1);

        let triangle = Simplex::triangle(0, 1, 2);
        assert_eq!(triangle.dim(), 2);
    }

    #[test]
    fn test_simplex_faces() {
        let triangle = Simplex::triangle(0, 1, 2);
        let faces = triangle.faces();

        assert_eq!(faces.len(), 3);
        assert!(faces.contains(&Simplex::edge(0, 1)));
        assert!(faces.contains(&Simplex::edge(0, 2)));
        assert!(faces.contains(&Simplex::edge(1, 2)));
    }

    #[test]
    fn test_simplicial_complex() {
        let mut complex = SimplicialComplex::new();
        complex.add(Simplex::triangle(0, 1, 2));

        // Should have 1 triangle, 3 edges, 3 vertices
        assert_eq!(complex.count_dim(0), 3);
        assert_eq!(complex.count_dim(1), 3);
        assert_eq!(complex.count_dim(2), 1);

        assert_eq!(complex.euler_characteristic(), 1); // 3 - 3 + 1 = 1
    }

    #[test]
    fn test_f_vector() {
        let complex = SimplicialComplex::from_simplices(vec![
            Simplex::triangle(0, 1, 2),
            Simplex::triangle(1, 2, 3),
        ]);

        let f = complex.f_vector();
        assert_eq!(f[0], 4); // 4 vertices
        assert_eq!(f[1], 5); // 5 edges (shared edge 1-2)
        assert_eq!(f[2], 2); // 2 triangles
    }

    #[test]
    fn test_is_face_of() {
        let edge = Simplex::edge(0, 1);
        let triangle = Simplex::triangle(0, 1, 2);

        assert!(edge.is_face_of(&triangle));
        assert!(!triangle.is_face_of(&edge));
    }
}
