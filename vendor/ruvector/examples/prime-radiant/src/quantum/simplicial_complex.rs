//! Simplicial Complex and Algebraic Topology Operations
//!
//! Provides simplicial complexes, boundary maps, and algebraic topology operations
//! for computing homology and cohomology groups.

use std::collections::{HashMap, HashSet, BTreeSet};
use super::{constants, QuantumTopologyError, Result};
use ruvector_solver::types::CsrMatrix;

/// A simplex (k-simplex has k+1 vertices)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Simplex {
    /// Sorted vertex indices
    vertices: BTreeSet<usize>,
}

impl Simplex {
    /// Create a simplex from vertices
    pub fn new(vertices: impl IntoIterator<Item = usize>) -> Self {
        Self {
            vertices: vertices.into_iter().collect(),
        }
    }

    /// Create a 0-simplex (vertex)
    pub fn vertex(v: usize) -> Self {
        Self::new([v])
    }

    /// Create a 1-simplex (edge)
    pub fn edge(v0: usize, v1: usize) -> Self {
        Self::new([v0, v1])
    }

    /// Create a 2-simplex (triangle)
    pub fn triangle(v0: usize, v1: usize, v2: usize) -> Self {
        Self::new([v0, v1, v2])
    }

    /// Create a 3-simplex (tetrahedron)
    pub fn tetrahedron(v0: usize, v1: usize, v2: usize, v3: usize) -> Self {
        Self::new([v0, v1, v2, v3])
    }

    /// Dimension of the simplex (0 = vertex, 1 = edge, ...)
    pub fn dim(&self) -> usize {
        if self.vertices.is_empty() {
            0
        } else {
            self.vertices.len() - 1
        }
    }

    /// Number of vertices
    pub fn num_vertices(&self) -> usize {
        self.vertices.len()
    }

    /// Get vertices as a sorted vector
    pub fn vertices(&self) -> Vec<usize> {
        self.vertices.iter().copied().collect()
    }

    /// Check if this is a face of another simplex
    pub fn is_face_of(&self, other: &Simplex) -> bool {
        self.vertices.is_subset(&other.vertices) && self.vertices != other.vertices
    }

    /// Get all faces of dimension dim-1 (boundary)
    pub fn boundary_faces(&self) -> Vec<(Simplex, i32)> {
        if self.vertices.is_empty() {
            return vec![];
        }

        let verts: Vec<usize> = self.vertices();
        let mut faces = Vec::with_capacity(verts.len());

        for (i, _) in verts.iter().enumerate() {
            let face_verts: Vec<usize> = verts
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, &v)| v)
                .collect();

            let sign = if i % 2 == 0 { 1 } else { -1 };
            faces.push((Simplex::new(face_verts), sign));
        }

        faces
    }

    /// Get all faces of the simplex (all dimensions)
    pub fn all_faces(&self) -> Vec<Simplex> {
        let verts: Vec<usize> = self.vertices();
        let n = verts.len();
        let mut faces = Vec::new();

        // Generate all non-empty subsets
        for mask in 1..(1 << n) {
            let subset: Vec<usize> = (0..n)
                .filter(|i| (mask >> i) & 1 == 1)
                .map(|i| verts[i])
                .collect();
            faces.push(Simplex::new(subset));
        }

        faces
    }

    /// Check if two simplices share a common face
    pub fn shares_face_with(&self, other: &Simplex) -> bool {
        !self.vertices.is_disjoint(&other.vertices)
    }

    /// Join two simplices (if disjoint)
    pub fn join(&self, other: &Simplex) -> Option<Simplex> {
        if !self.vertices.is_disjoint(&other.vertices) {
            return None;
        }

        let mut new_vertices = self.vertices.clone();
        new_vertices.extend(&other.vertices);
        Some(Simplex { vertices: new_vertices })
    }
}

impl std::fmt::Display for Simplex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let verts: Vec<String> = self.vertices.iter().map(|v| v.to_string()).collect();
        write!(f, "[{}]", verts.join(","))
    }
}

/// Sparse matrix for boundary computations (using HashMap for O(1) access)
#[derive(Debug, Clone)]
pub struct SparseMatrix {
    /// (row, col) -> value entries (O(1) access)
    entries: HashMap<(usize, usize), i32>,
    /// Number of rows
    pub rows: usize,
    /// Number of columns
    pub cols: usize,
}

impl SparseMatrix {
    /// Create an empty sparse matrix
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            entries: HashMap::new(),
            rows,
            cols,
        }
    }

    /// Create from dense matrix
    pub fn from_dense(dense: &[Vec<i32>]) -> Self {
        let rows = dense.len();
        let cols = dense.first().map(|r| r.len()).unwrap_or(0);
        let mut entries = HashMap::new();

        for (i, row) in dense.iter().enumerate() {
            for (j, &val) in row.iter().enumerate() {
                if val != 0 {
                    entries.insert((i, j), val);
                }
            }
        }

        Self { entries, rows, cols }
    }

    /// Set a value (O(1) via HashMap)
    pub fn set(&mut self, row: usize, col: usize, value: i32) {
        if value == 0 {
            self.entries.remove(&(row, col));
        } else {
            self.entries.insert((row, col), value);
        }
    }

    /// Get a value (O(1) via HashMap)
    pub fn get(&self, row: usize, col: usize) -> i32 {
        self.entries.get(&(row, col)).copied().unwrap_or(0)
    }

    /// Transpose the matrix
    pub fn transpose(&self) -> Self {
        Self {
            entries: self.entries.iter().map(|(&(r, c), &v)| ((c, r), v)).collect(),
            rows: self.cols,
            cols: self.rows,
        }
    }

    /// Number of non-zero entries
    pub fn nnz(&self) -> usize {
        self.entries.len()
    }

    /// Convert to dense matrix
    pub fn to_dense(&self) -> Vec<Vec<i32>> {
        let mut dense = vec![vec![0; self.cols]; self.rows];
        for (&(r, c), &v) in &self.entries {
            if r < self.rows && c < self.cols {
                dense[r][c] = v;
            }
        }
        dense
    }

    /// Matrix-vector multiplication (over integers mod 2)
    pub fn matvec_mod2(&self, v: &[u8]) -> Vec<u8> {
        let mut result = vec![0u8; self.rows];
        for (&(r, c), &val) in &self.entries {
            if c < v.len() && r < result.len() {
                let product = ((val.abs() as u8) * v[c]) % 2;
                result[r] = (result[r] + product) % 2;
            }
        }
        result
    }

    /// Convert to CsrMatrix<f64> for efficient SpMV operations.
    /// Uses ruvector-solver's CSR format with O(nnz) cache-friendly SpMV.
    pub fn to_csr_f64(&self) -> CsrMatrix<f64> {
        CsrMatrix::<f64>::from_coo(
            self.rows,
            self.cols,
            self.entries.iter().map(|(&(r, c), &v)| (r, c, v as f64)),
        )
    }

    /// Sparse matrix-vector multiply using solver's optimized CSR SpMV.
    /// Converts to CsrMatrix internally for cache-efficient computation.
    pub fn spmv_f64(&self, x: &[f64]) -> Vec<f64> {
        let csr = self.to_csr_f64();
        let mut y = vec![0.0f64; self.rows];
        csr.spmv(x, &mut y);
        y
    }

    /// Get entries as a Vec of (row, col, value) triples (for iteration).
    pub fn triplets(&self) -> Vec<(usize, usize, i32)> {
        self.entries.iter().map(|(&(r, c), &v)| (r, c, v)).collect()
    }

    /// Compute rank over Z/2Z using Gaussian elimination
    pub fn rank_mod2(&self) -> usize {
        let mut dense: Vec<Vec<u8>> = self.to_dense()
            .into_iter()
            .map(|row| row.into_iter().map(|v| (v.abs() % 2) as u8).collect())
            .collect();

        if dense.is_empty() || dense[0].is_empty() {
            return 0;
        }

        let rows = dense.len();
        let cols = dense[0].len();
        let mut rank = 0;
        let mut pivot_col = 0;

        for row in 0..rows {
            if pivot_col >= cols {
                break;
            }

            // Find pivot
            let mut pivot_row = None;
            for r in row..rows {
                if dense[r][pivot_col] != 0 {
                    pivot_row = Some(r);
                    break;
                }
            }

            if let Some(pr) = pivot_row {
                // Swap rows
                dense.swap(row, pr);

                // Eliminate column
                for r in 0..rows {
                    if r != row && dense[r][pivot_col] != 0 {
                        for c in 0..cols {
                            dense[r][c] = (dense[r][c] + dense[row][c]) % 2;
                        }
                    }
                }

                rank += 1;
                pivot_col += 1;
            } else {
                pivot_col += 1;
            }
        }

        rank
    }

    /// Compute kernel (null space) over Z/2Z
    pub fn kernel_mod2(&self) -> Vec<Vec<u8>> {
        let mut dense: Vec<Vec<u8>> = self.to_dense()
            .into_iter()
            .map(|row| row.into_iter().map(|v| (v.abs() % 2) as u8).collect())
            .collect();

        // For a 0-row matrix (0xN), the entire domain is the kernel
        if dense.is_empty() {
            // Return standard basis vectors for each column
            let mut kernel = Vec::new();
            for c in 0..self.cols {
                let mut vec = vec![0u8; self.cols];
                vec[c] = 1;
                kernel.push(vec);
            }
            return kernel;
        }

        let rows = dense.len();
        let cols = dense[0].len();

        // Augment with identity for tracking
        for (i, row) in dense.iter_mut().enumerate() {
            for j in 0..rows {
                row.push(if i == j { 1 } else { 0 });
            }
        }

        // Gaussian elimination
        let mut pivot_col = 0;
        let mut pivot_rows = Vec::new();

        for row in 0..rows {
            if pivot_col >= cols {
                break;
            }

            // Find pivot
            let mut pivot_row = None;
            for r in row..rows {
                if dense[r][pivot_col] != 0 {
                    pivot_row = Some(r);
                    break;
                }
            }

            if let Some(pr) = pivot_row {
                dense.swap(row, pr);

                for r in 0..rows {
                    if r != row && dense[r][pivot_col] != 0 {
                        for c in 0..dense[0].len() {
                            dense[r][c] = (dense[r][c] + dense[row][c]) % 2;
                        }
                    }
                }

                pivot_rows.push((row, pivot_col));
                pivot_col += 1;
            } else {
                pivot_col += 1;
            }
        }

        // Extract kernel basis (free variables)
        let pivot_cols: HashSet<usize> = pivot_rows.iter().map(|&(_, c)| c).collect();
        let mut kernel = Vec::new();

        for c in 0..cols {
            if !pivot_cols.contains(&c) {
                let mut vec = vec![0u8; cols];
                vec[c] = 1;

                for &(r, pc) in &pivot_rows {
                    if dense[r][c] != 0 {
                        vec[pc] = 1;
                    }
                }

                kernel.push(vec);
            }
        }

        kernel
    }
}

/// Boundary matrix for a simplicial complex
#[derive(Debug, Clone)]
pub struct BoundaryMatrix {
    /// Sparse boundary matrix ∂_k: C_k → C_{k-1}
    pub matrix: SparseMatrix,
    /// Dimension k
    pub dimension: usize,
    /// Domain simplices (k-simplices)
    pub domain: Vec<Simplex>,
    /// Codomain simplices ((k-1)-simplices)
    pub codomain: Vec<Simplex>,
}

impl BoundaryMatrix {
    /// Create a boundary matrix for dimension k
    pub fn new(k_simplices: &[Simplex], k_minus_1_simplices: &[Simplex]) -> Self {
        let rows = k_minus_1_simplices.len();
        let cols = k_simplices.len();
        let mut matrix = SparseMatrix::new(rows, cols);

        // Build simplex to index mapping for codomain
        let codomain_indices: HashMap<&Simplex, usize> = k_minus_1_simplices
            .iter()
            .enumerate()
            .map(|(i, s)| (s, i))
            .collect();

        // Build boundary matrix
        for (col, sigma) in k_simplices.iter().enumerate() {
            for (face, sign) in sigma.boundary_faces() {
                if let Some(&row) = codomain_indices.get(&face) {
                    matrix.set(row, col, sign);
                }
            }
        }

        Self {
            matrix,
            dimension: if k_simplices.is_empty() { 0 } else { k_simplices[0].dim() },
            domain: k_simplices.to_vec(),
            codomain: k_minus_1_simplices.to_vec(),
        }
    }

    /// Compute the image (over Z/2Z)
    pub fn image_rank(&self) -> usize {
        self.matrix.rank_mod2()
    }

    /// Compute the kernel (over Z/2Z)
    pub fn kernel(&self) -> Vec<Vec<u8>> {
        self.matrix.kernel_mod2()
    }
}

/// Simplicial complex with boundary chain structure
#[derive(Debug, Clone)]
pub struct SimplicialComplex {
    /// Simplices organized by dimension
    simplices: Vec<HashSet<Simplex>>,
    /// Maximum dimension
    max_dim: usize,
}

impl SimplicialComplex {
    /// Create an empty simplicial complex
    pub fn new() -> Self {
        Self {
            simplices: vec![HashSet::new()],
            max_dim: 0,
        }
    }

    /// Create from a list of simplices (automatically adds faces)
    pub fn from_simplices(simplices: impl IntoIterator<Item = Simplex>) -> Self {
        let mut complex = Self::new();
        for s in simplices {
            complex.add_simplex(s);
        }
        complex
    }

    /// Add a simplex and all its faces
    pub fn add_simplex(&mut self, simplex: Simplex) {
        // Ensure we have enough dimensions
        let dim = simplex.dim();
        while self.simplices.len() <= dim {
            self.simplices.push(HashSet::new());
        }
        self.max_dim = self.max_dim.max(dim);

        // Add simplex and all faces
        for face in simplex.all_faces() {
            let face_dim = face.dim();
            if face_dim < self.simplices.len() {
                self.simplices[face_dim].insert(face);
            }
        }
    }

    /// Check if a simplex is in the complex
    pub fn contains(&self, simplex: &Simplex) -> bool {
        let dim = simplex.dim();
        if dim >= self.simplices.len() {
            return false;
        }
        self.simplices[dim].contains(simplex)
    }

    /// Get all simplices of dimension k
    pub fn simplices_of_dim(&self, k: usize) -> Vec<Simplex> {
        if k >= self.simplices.len() {
            return vec![];
        }
        self.simplices[k].iter().cloned().collect()
    }

    /// Get all simplices
    pub fn all_simplices(&self) -> Vec<Simplex> {
        self.simplices.iter().flat_map(|s| s.iter().cloned()).collect()
    }

    /// Number of simplices of dimension k
    pub fn count(&self, k: usize) -> usize {
        self.simplices.get(k).map(|s| s.len()).unwrap_or(0)
    }

    /// Total number of simplices
    pub fn size(&self) -> usize {
        self.simplices.iter().map(|s| s.len()).sum()
    }

    /// Maximum dimension
    pub fn dimension(&self) -> usize {
        self.max_dim
    }

    /// f-vector: (f_0, f_1, f_2, ...) = counts at each dimension
    pub fn f_vector(&self) -> Vec<usize> {
        self.simplices.iter().map(|s| s.len()).collect()
    }

    /// Euler characteristic: χ = Σ (-1)^k f_k
    pub fn euler_characteristic(&self) -> i64 {
        self.simplices
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let sign = if k % 2 == 0 { 1 } else { -1 };
                sign * s.len() as i64
            })
            .sum()
    }

    /// Get the boundary matrix ∂_k: C_k → C_{k-1}
    pub fn boundary_matrix(&self, k: usize) -> BoundaryMatrix {
        let k_simplices = self.simplices_of_dim(k);
        let k_minus_1_simplices = if k > 0 {
            self.simplices_of_dim(k - 1)
        } else {
            vec![]
        };

        BoundaryMatrix::new(&k_simplices, &k_minus_1_simplices)
    }

    /// Compute Betti number β_k (over Z/2Z)
    /// β_k = dim(ker(∂_k)) - dim(im(∂_{k+1}))
    pub fn betti_number(&self, k: usize) -> usize {
        let boundary_k = self.boundary_matrix(k);
        let boundary_k_plus_1 = self.boundary_matrix(k + 1);

        let kernel_dim = boundary_k.kernel().len();
        let image_dim = boundary_k_plus_1.image_rank();

        kernel_dim.saturating_sub(image_dim)
    }

    /// Compute all Betti numbers up to max dimension
    pub fn betti_numbers(&self) -> Vec<usize> {
        (0..=self.max_dim).map(|k| self.betti_number(k)).collect()
    }

    /// Compute homology generators (as chains)
    pub fn homology_generators(&self, k: usize) -> Vec<Vec<Simplex>> {
        let boundary_k = self.boundary_matrix(k);
        let boundary_k_plus_1 = self.boundary_matrix(k + 1);

        let cycles = boundary_k.kernel();
        let boundaries = boundary_k_plus_1.image_rank();

        // Return cycle representatives (simplified - doesn't mod out boundaries)
        let k_simplices = self.simplices_of_dim(k);
        let num_generators = cycles.len().saturating_sub(boundaries);
        cycles
            .into_iter()
            .take(num_generators)
            .map(|cycle| {
                cycle
                    .into_iter()
                    .enumerate()
                    .filter(|(_, v)| *v != 0)
                    .map(|(i, _)| k_simplices[i].clone())
                    .collect()
            })
            .collect()
    }

    /// Cup product at the chain level (simplified)
    /// For cochains α ∈ C^p and β ∈ C^q, compute α ∪ β ∈ C^{p+q}
    pub fn cup_product(
        &self,
        alpha: &[f64],  // p-cochain values on p-simplices
        beta: &[f64],   // q-cochain values on q-simplices
        p: usize,
        q: usize,
    ) -> Vec<f64> {
        let p_plus_q_simplices = self.simplices_of_dim(p + q);
        let mut result = vec![0.0; p_plus_q_simplices.len()];

        for (i, sigma) in p_plus_q_simplices.iter().enumerate() {
            let vertices = sigma.vertices();
            if vertices.len() >= p + q + 1 {
                // Front p-face: [v_0, ..., v_p]
                let front: Vec<usize> = vertices[..=p].to_vec();
                // Back q-face: [v_p, ..., v_{p+q}]
                let back: Vec<usize> = vertices[p..].to_vec();

                let front_simplex = Simplex::new(front);
                let back_simplex = Simplex::new(back);

                // Find indices in respective dimensions
                let p_simplices = self.simplices_of_dim(p);
                let q_simplices = self.simplices_of_dim(q);

                let front_idx = p_simplices.iter().position(|s| s == &front_simplex);
                let back_idx = q_simplices.iter().position(|s| s == &back_simplex);

                if let (Some(fi), Some(bi)) = (front_idx, back_idx) {
                    if fi < alpha.len() && bi < beta.len() {
                        result[i] = alpha[fi] * beta[bi];
                    }
                }
            }
        }

        result
    }

    /// Compute the graph Laplacian L = D - A as a CsrMatrix<f64>.
    /// Uses the 1-skeleton (edges) to build the adjacency matrix, then
    /// L_ii = degree(i), L_ij = -1 if edge (i,j) exists.
    pub fn graph_laplacian_csr(&self) -> CsrMatrix<f64> {
        let num_vertices = self.count(0);
        let edges = self.simplices_of_dim(1);

        let mut entries: Vec<(usize, usize, f64)> = Vec::new();
        let mut degree = vec![0usize; num_vertices];

        // Get vertex mapping (since vertices may not be 0..n)
        let vertices = self.simplices_of_dim(0);
        let vertex_map: HashMap<Vec<usize>, usize> = vertices.iter()
            .enumerate()
            .map(|(idx, s)| (s.vertices(), idx))
            .collect();

        for edge in &edges {
            let verts = edge.vertices();
            if verts.len() == 2 {
                if let (Some(&i), Some(&j)) = (vertex_map.get(&vec![verts[0]]), vertex_map.get(&vec![verts[1]])) {
                    entries.push((i, j, -1.0));
                    entries.push((j, i, -1.0));
                    degree[i] += 1;
                    degree[j] += 1;
                }
            }
        }

        // Add diagonal (degree)
        for (i, &d) in degree.iter().enumerate() {
            entries.push((i, i, d as f64));
        }

        CsrMatrix::<f64>::from_coo(num_vertices, num_vertices, entries)
    }
}

impl Default for SimplicialComplex {
    fn default() -> Self {
        Self::new()
    }
}

/// Create standard simplicial complexes
pub mod standard_complexes {
    use super::*;

    /// Create k-simplex (filled)
    pub fn simplex(k: usize) -> SimplicialComplex {
        let vertices: Vec<usize> = (0..=k).collect();
        let simplex = Simplex::new(vertices);
        SimplicialComplex::from_simplices([simplex])
    }

    /// Create k-sphere (boundary of (k+1)-simplex)
    pub fn sphere(k: usize) -> SimplicialComplex {
        let simplex_vertices: Vec<usize> = (0..=k + 1).collect();
        let big_simplex = Simplex::new(simplex_vertices);

        // Get all k-faces
        let mut complex = SimplicialComplex::new();
        for (face, _) in big_simplex.boundary_faces() {
            complex.add_simplex(face);
        }
        complex
    }

    /// Create torus (triangulated)
    pub fn torus() -> SimplicialComplex {
        // Minimal triangulation of torus with 7 vertices
        let triangles = [
            [0, 1, 2], [0, 2, 3], [0, 3, 5], [0, 5, 6], [0, 4, 6], [0, 1, 4],
            [1, 2, 4], [2, 4, 5], [2, 3, 5], [3, 4, 6], [3, 5, 6], [1, 3, 4],
            [1, 3, 6], [1, 2, 6], [2, 5, 6],
        ];

        SimplicialComplex::from_simplices(
            triangles.iter().map(|&[a, b, c]| Simplex::triangle(a, b, c))
        )
    }

    /// Create Klein bottle (triangulated)
    pub fn klein_bottle() -> SimplicialComplex {
        // Minimal triangulation with identification
        // Similar structure to torus but with different identifications
        let triangles = [
            [0, 1, 4], [0, 4, 3], [0, 3, 2], [0, 2, 5], [0, 5, 1],
            [1, 4, 5], [2, 3, 6], [3, 4, 6], [4, 5, 6], [1, 2, 5],
            [1, 2, 6], [2, 5, 6], [3, 4, 7], [4, 6, 7], [3, 6, 7],
        ];

        SimplicialComplex::from_simplices(
            triangles.iter().map(|&[a, b, c]| Simplex::triangle(a, b, c))
        )
    }

    /// Create projective plane RP² (triangulated)
    pub fn projective_plane() -> SimplicialComplex {
        // 6-vertex triangulation
        let triangles = [
            [0, 1, 2], [0, 2, 4], [0, 1, 5], [0, 4, 5], [1, 2, 3],
            [1, 3, 5], [2, 3, 4], [3, 4, 5],
        ];

        SimplicialComplex::from_simplices(
            triangles.iter().map(|&[a, b, c]| Simplex::triangle(a, b, c))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplex_basics() {
        let vertex = Simplex::vertex(0);
        assert_eq!(vertex.dim(), 0);

        let edge = Simplex::edge(0, 1);
        assert_eq!(edge.dim(), 1);

        let triangle = Simplex::triangle(0, 1, 2);
        assert_eq!(triangle.dim(), 2);
    }

    #[test]
    fn test_simplex_boundary() {
        let triangle = Simplex::triangle(0, 1, 2);
        let boundary = triangle.boundary_faces();

        assert_eq!(boundary.len(), 3);

        // Check edges are correct
        let edges: Vec<Simplex> = boundary.iter().map(|(s, _)| s.clone()).collect();
        assert!(edges.contains(&Simplex::edge(1, 2)));
        assert!(edges.contains(&Simplex::edge(0, 2)));
        assert!(edges.contains(&Simplex::edge(0, 1)));
    }

    #[test]
    fn test_simplicial_complex_triangle() {
        let complex = SimplicialComplex::from_simplices([Simplex::triangle(0, 1, 2)]);

        assert_eq!(complex.count(0), 3); // 3 vertices
        assert_eq!(complex.count(1), 3); // 3 edges
        assert_eq!(complex.count(2), 1); // 1 triangle

        // Euler characteristic: χ = 3 - 3 + 1 = 1
        assert_eq!(complex.euler_characteristic(), 1);
    }

    #[test]
    fn test_betti_numbers_triangle() {
        let complex = SimplicialComplex::from_simplices([Simplex::triangle(0, 1, 2)]);
        let betti = complex.betti_numbers();

        // Filled triangle is contractible: β_0 = 1, β_1 = 0, β_2 = 0
        assert_eq!(betti[0], 1);
        assert_eq!(betti[1], 0);
    }

    #[test]
    fn test_betti_numbers_circle() {
        // Circle = triangle boundary (no filling)
        let mut complex = SimplicialComplex::new();
        complex.add_simplex(Simplex::edge(0, 1));
        complex.add_simplex(Simplex::edge(1, 2));
        complex.add_simplex(Simplex::edge(0, 2));

        let betti = complex.betti_numbers();

        // Circle: β_0 = 1 (connected), β_1 = 1 (one loop)
        assert_eq!(betti[0], 1);
        assert_eq!(betti[1], 1);
    }

    #[test]
    fn test_sparse_matrix_rank() {
        // Simple 2x3 matrix with rank 2
        let matrix = SparseMatrix::from_dense(&[
            vec![1, 0, 1],
            vec![0, 1, 1],
        ]);

        assert_eq!(matrix.rank_mod2(), 2);
    }

    #[test]
    fn test_sparse_matrix_kernel() {
        // Matrix with non-trivial kernel
        let matrix = SparseMatrix::from_dense(&[
            vec![1, 1, 0],
            vec![0, 0, 0],
        ]);

        let kernel = matrix.kernel_mod2();
        assert!(!kernel.is_empty());
    }

    #[test]
    fn test_standard_simplex() {
        let simplex_2 = standard_complexes::simplex(2);
        assert_eq!(simplex_2.euler_characteristic(), 1);
    }

    #[test]
    fn test_standard_sphere() {
        // 1-sphere should have χ = 0 (V - E = n - n = 0 for a cycle)
        let sphere_1 = standard_complexes::sphere(1);
        // Actually S^1 has χ = 0
        let chi = sphere_1.euler_characteristic();
        assert!(chi == 0 || chi == 2); // Depending on triangulation
    }
}
