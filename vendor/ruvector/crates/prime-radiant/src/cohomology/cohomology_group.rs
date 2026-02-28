//! Cohomology Group Computation
//!
//! Computes the cohomology groups H^n(K, F) using linear algebra methods.

use super::cocycle::{Coboundary, Cocycle};
use super::simplex::{Cochain, SimplexId, SimplicialComplex};
use ndarray::{Array1, Array2, Axis};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for cohomology computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohomologyConfig {
    /// Maximum dimension to compute
    pub max_dimension: usize,
    /// Tolerance for numerical zero
    pub tolerance: f64,
    /// Whether to compute explicit generators
    pub compute_generators: bool,
    /// Whether to use sparse methods for large complexes
    pub use_sparse: bool,
}

impl Default for CohomologyConfig {
    fn default() -> Self {
        Self {
            max_dimension: 2,
            tolerance: 1e-10,
            compute_generators: true,
            use_sparse: false,
        }
    }
}

/// Betti numbers of a space
///
/// The n-th Betti number b_n = dim(H^n) counts "n-dimensional holes"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BettiNumbers {
    /// Betti numbers indexed by dimension
    pub numbers: Vec<usize>,
    /// Euler characteristic (alternating sum)
    pub euler_characteristic: i64,
}

impl BettiNumbers {
    /// Create from vector of Betti numbers
    pub fn from_vec(numbers: Vec<usize>) -> Self {
        let euler_characteristic = numbers
            .iter()
            .enumerate()
            .map(|(i, &b)| if i % 2 == 0 { b as i64 } else { -(b as i64) })
            .sum();

        Self {
            numbers,
            euler_characteristic,
        }
    }

    /// Get Betti number for dimension n
    pub fn b(&self, n: usize) -> usize {
        self.numbers.get(n).copied().unwrap_or(0)
    }

    /// Total number of holes
    pub fn total_rank(&self) -> usize {
        self.numbers.iter().sum()
    }
}

/// A cohomology group H^n(K, F)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohomologyGroup {
    /// Dimension of the cohomology group
    pub dimension: usize,
    /// Generators (representatives of cohomology classes)
    pub generators: Vec<Cocycle>,
    /// Betti number (dimension of the group as vector space)
    pub betti_number: usize,
    /// Whether generators are normalized
    pub normalized: bool,
}

impl CohomologyGroup {
    /// Create a trivial cohomology group
    pub fn trivial(dimension: usize) -> Self {
        Self {
            dimension,
            generators: Vec::new(),
            betti_number: 0,
            normalized: true,
        }
    }

    /// Create a cohomology group from generators
    pub fn from_generators(dimension: usize, generators: Vec<Cocycle>) -> Self {
        let betti_number = generators.len();
        Self {
            dimension,
            generators,
            betti_number,
            normalized: false,
        }
    }

    /// Normalize the generators to be orthonormal
    pub fn normalize(&mut self) {
        if self.generators.is_empty() {
            self.normalized = true;
            return;
        }

        // Gram-Schmidt orthonormalization
        let mut orthonormal = Vec::new();

        for gen in &self.generators {
            let mut v = gen.clone();

            // Subtract projections onto previous vectors
            for u in &orthonormal {
                let proj_coeff = v.inner_product(u) / u.inner_product(u);
                let mut proj = u.clone();
                proj.scale(proj_coeff);
                v.scale(1.0);
                for (&id, &val) in &proj.values {
                    let current = v.get(id);
                    v.set(id, current - val);
                }
            }

            // Normalize
            v.normalize();
            if v.norm() > 1e-10 {
                orthonormal.push(v);
            }
        }

        self.generators = orthonormal;
        self.betti_number = self.generators.len();
        self.normalized = true;
    }

    /// Check if a cocycle represents the zero class
    pub fn is_trivial_class(&self, cocycle: &Cocycle) -> bool {
        // A cocycle represents the zero class if it's a coboundary
        // This is checked during computation
        cocycle.is_coboundary
    }

    /// Project a cocycle onto this cohomology group
    pub fn project(&self, cocycle: &Cocycle) -> Cocycle {
        if self.generators.is_empty() {
            return Cocycle::zero(self.dimension);
        }

        let mut result = Cocycle::zero(self.dimension);

        for gen in &self.generators {
            let coeff = cocycle.inner_product(gen);
            let mut contrib = gen.clone();
            contrib.scale(coeff);

            for (&id, &val) in &contrib.values {
                let current = result.get(id);
                result.set(id, current + val);
            }
        }

        result
    }
}

/// Computes cohomology groups for a simplicial complex
pub struct CohomologyComputer {
    /// The simplicial complex
    complex: SimplicialComplex,
    /// Configuration
    config: CohomologyConfig,
    /// Coboundary operator
    coboundary: Coboundary,
    /// Cached boundary matrices
    boundary_matrices: HashMap<usize, Array2<f64>>,
}

impl CohomologyComputer {
    /// Create a new cohomology computer
    pub fn new(complex: SimplicialComplex, config: CohomologyConfig) -> Self {
        let coboundary = Coboundary::new(complex.clone());
        Self {
            complex,
            config,
            coboundary,
            boundary_matrices: HashMap::new(),
        }
    }

    /// Create with default configuration
    pub fn with_default_config(complex: SimplicialComplex) -> Self {
        Self::new(complex, CohomologyConfig::default())
    }

    /// Build the boundary matrix for dimension n
    ///
    /// The boundary matrix d_n: C_n -> C_{n-1} has entry (i,j) equal to
    /// the coefficient of simplex i in the boundary of simplex j
    fn build_boundary_matrix(&mut self, n: usize) -> Array2<f64> {
        if let Some(matrix) = self.boundary_matrices.get(&n) {
            return matrix.clone();
        }

        let n_simplices: Vec<_> = self.complex.simplices_of_dim(n).collect();
        let n_minus_1_simplices: Vec<_> = if n > 0 {
            self.complex.simplices_of_dim(n - 1).collect()
        } else {
            Vec::new()
        };

        if n == 0 || n_minus_1_simplices.is_empty() {
            let matrix = Array2::zeros((0, n_simplices.len()));
            self.boundary_matrices.insert(n, matrix.clone());
            return matrix;
        }

        // Create index maps
        let simplex_to_idx: HashMap<SimplexId, usize> = n_minus_1_simplices
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id, i))
            .collect();

        let rows = n_minus_1_simplices.len();
        let cols = n_simplices.len();
        let mut matrix = Array2::zeros((rows, cols));

        for (j, simplex) in n_simplices.iter().enumerate() {
            let boundary = simplex.boundary();
            for (face, sign) in boundary {
                if let Some(&i) = simplex_to_idx.get(&face.id) {
                    matrix[[i, j]] = sign as f64;
                }
            }
        }

        self.boundary_matrices.insert(n, matrix.clone());
        matrix
    }

    /// Compute the coboundary matrix (transpose of boundary matrix)
    fn build_coboundary_matrix(&mut self, n: usize) -> Array2<f64> {
        let boundary = self.build_boundary_matrix(n + 1);
        boundary.t().to_owned()
    }

    /// Compute the kernel of a matrix using SVD
    fn compute_kernel(&self, matrix: &Array2<f64>) -> Vec<Array1<f64>> {
        if matrix.is_empty() || matrix.ncols() == 0 {
            return Vec::new();
        }

        // Use simple Gaussian elimination for kernel computation
        // For production, should use proper SVD
        let mut kernel_basis = Vec::new();

        // Find null space using reduced row echelon form
        let (rref, pivot_cols) = self.row_reduce(matrix);

        let n_cols = matrix.ncols();
        let free_vars: Vec<usize> = (0..n_cols).filter(|c| !pivot_cols.contains(c)).collect();

        for &free_var in &free_vars {
            let mut kernel_vec = Array1::zeros(n_cols);
            kernel_vec[free_var] = 1.0;

            // Back-substitute to find other components
            for (row_idx, &pivot_col) in pivot_cols.iter().enumerate() {
                if row_idx < rref.nrows() {
                    kernel_vec[pivot_col] = -rref[[row_idx, free_var]];
                }
            }

            if kernel_vec.iter().map(|x| x * x).sum::<f64>().sqrt() > self.config.tolerance {
                kernel_basis.push(kernel_vec);
            }
        }

        kernel_basis
    }

    /// Compute the image of a matrix
    fn compute_image(&self, matrix: &Array2<f64>) -> Vec<Array1<f64>> {
        if matrix.is_empty() || matrix.ncols() == 0 {
            return Vec::new();
        }

        let (_, pivot_cols) = self.row_reduce(matrix);

        pivot_cols
            .into_iter()
            .map(|col| matrix.column(col).to_owned())
            .collect()
    }

    /// Row reduce to RREF
    fn row_reduce(&self, matrix: &Array2<f64>) -> (Array2<f64>, Vec<usize>) {
        let mut a = matrix.clone();
        let m = a.nrows();
        let n = a.ncols();
        let mut pivot_cols = Vec::new();

        let mut pivot_row = 0;
        for col in 0..n {
            if pivot_row >= m {
                break;
            }

            // Find pivot
            let mut max_row = pivot_row;
            let mut max_val = a[[pivot_row, col]].abs();
            for row in (pivot_row + 1)..m {
                if a[[row, col]].abs() > max_val {
                    max_val = a[[row, col]].abs();
                    max_row = row;
                }
            }

            if max_val < self.config.tolerance {
                continue;
            }

            // Swap rows
            for c in 0..n {
                let tmp = a[[pivot_row, c]];
                a[[pivot_row, c]] = a[[max_row, c]];
                a[[max_row, c]] = tmp;
            }

            // Scale pivot row
            let pivot_val = a[[pivot_row, col]];
            for c in 0..n {
                a[[pivot_row, c]] /= pivot_val;
            }

            // Eliminate other rows
            for row in 0..m {
                if row != pivot_row {
                    let factor = a[[row, col]];
                    for c in 0..n {
                        a[[row, c]] -= factor * a[[pivot_row, c]];
                    }
                }
            }

            pivot_cols.push(col);
            pivot_row += 1;
        }

        (a, pivot_cols)
    }

    /// Compute cohomology in dimension n: H^n = ker(delta_n) / im(delta_{n-1})
    pub fn compute_cohomology(&mut self, n: usize) -> CohomologyGroup {
        // Get simplices for this dimension
        let n_simplices: Vec<_> = self.complex.simplices_of_dim(n).collect();
        if n_simplices.is_empty() {
            return CohomologyGroup::trivial(n);
        }

        // Build simplex ID to index map
        let simplex_to_idx: HashMap<SimplexId, usize> = n_simplices
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id, i))
            .collect();

        // Compute ker(delta_n): cochains f such that delta(f) = 0
        let delta_n = self.build_coboundary_matrix(n);
        let kernel_basis = self.compute_kernel(&delta_n);

        if kernel_basis.is_empty() {
            return CohomologyGroup::trivial(n);
        }

        // Compute im(delta_{n-1}): cochains that are coboundaries
        let image_basis = if n > 0 {
            let delta_n_minus_1 = self.build_coboundary_matrix(n - 1);
            self.compute_image(&delta_n_minus_1)
        } else {
            Vec::new()
        };

        // Quotient: find kernel vectors not in image
        // Use orthogonal projection to remove image component
        let generators = self.quotient_space(&kernel_basis, &image_basis, &simplex_to_idx, n);

        CohomologyGroup::from_generators(n, generators)
    }

    /// Compute quotient space ker/im
    fn quotient_space(
        &self,
        kernel: &[Array1<f64>],
        image: &[Array1<f64>],
        simplex_to_idx: &HashMap<SimplexId, usize>,
        dimension: usize,
    ) -> Vec<Cocycle> {
        if kernel.is_empty() {
            return Vec::new();
        }

        // Build index to simplex ID map
        let idx_to_simplex: HashMap<usize, SimplexId> =
            simplex_to_idx.iter().map(|(&id, &idx)| (idx, id)).collect();

        // If no image, all kernel elements are generators
        if image.is_empty() {
            return kernel
                .iter()
                .map(|v| self.array_to_cocycle(v, &idx_to_simplex, dimension, false))
                .collect();
        }

        // Orthogonalize kernel against image
        let mut quotient_basis: Vec<Array1<f64>> = Vec::new();

        for kernel_vec in kernel {
            let mut v = kernel_vec.clone();

            // Project out image components
            for img_vec in image {
                let norm_sq = img_vec.iter().map(|x| x * x).sum::<f64>();
                if norm_sq > self.config.tolerance {
                    let dot: f64 = v.iter().zip(img_vec.iter()).map(|(a, b)| a * b).sum();
                    let proj_coeff = dot / norm_sq;
                    v = &v - &(img_vec * proj_coeff);
                }
            }

            // Project out previous quotient vectors
            for prev in &quotient_basis {
                let norm_sq: f64 = prev.iter().map(|x| x * x).sum();
                if norm_sq > self.config.tolerance {
                    let dot: f64 = v.iter().zip(prev.iter()).map(|(a, b)| a * b).sum();
                    let proj_coeff = dot / norm_sq;
                    v = &v - &(prev * proj_coeff);
                }
            }

            let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > self.config.tolerance {
                quotient_basis.push(v);
            }
        }

        quotient_basis
            .iter()
            .map(|v| self.array_to_cocycle(v, &idx_to_simplex, dimension, false))
            .collect()
    }

    /// Convert array to cocycle
    fn array_to_cocycle(
        &self,
        arr: &Array1<f64>,
        idx_to_simplex: &HashMap<usize, SimplexId>,
        dimension: usize,
        is_coboundary: bool,
    ) -> Cocycle {
        let mut values = HashMap::new();
        for (idx, &val) in arr.iter().enumerate() {
            if val.abs() > self.config.tolerance {
                if let Some(&simplex_id) = idx_to_simplex.get(&idx) {
                    values.insert(simplex_id, val);
                }
            }
        }
        let mut cocycle = Cocycle::new(dimension, values);
        cocycle.is_coboundary = is_coboundary;
        cocycle
    }

    /// Compute all cohomology groups up to max_dimension
    pub fn compute_all(&mut self) -> Vec<CohomologyGroup> {
        let max_dim = self.config.max_dimension.min(self.complex.max_dimension);
        (0..=max_dim).map(|n| self.compute_cohomology(n)).collect()
    }

    /// Compute Betti numbers
    pub fn compute_betti_numbers(&mut self) -> BettiNumbers {
        let groups = self.compute_all();
        let numbers: Vec<usize> = groups.iter().map(|g| g.betti_number).collect();
        BettiNumbers::from_vec(numbers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::substrate::NodeId;
    use uuid::Uuid;

    fn make_node_id() -> NodeId {
        Uuid::new_v4()
    }

    #[test]
    fn test_point_cohomology() {
        // Single point: H^0 = R, H^n = 0 for n > 0
        let v0 = make_node_id();
        let complex = SimplicialComplex::from_graph_cliques(&[v0], &[], 0);

        let mut computer = CohomologyComputer::with_default_config(complex);
        let betti = computer.compute_betti_numbers();

        assert_eq!(betti.b(0), 1);
    }

    #[test]
    fn test_two_points_cohomology() {
        // Two disconnected points: H^0 = R^2
        let v0 = make_node_id();
        let v1 = make_node_id();
        let complex = SimplicialComplex::from_graph_cliques(&[v0, v1], &[], 0);

        let mut computer = CohomologyComputer::with_default_config(complex);
        let betti = computer.compute_betti_numbers();

        assert_eq!(betti.b(0), 2);
    }

    #[test]
    fn test_edge_cohomology() {
        // Single edge: H^0 = R (connected), H^n = 0 for n > 0
        let v0 = make_node_id();
        let v1 = make_node_id();
        let complex = SimplicialComplex::from_graph_cliques(&[v0, v1], &[(v0, v1)], 1);

        let mut computer = CohomologyComputer::with_default_config(complex);
        let betti = computer.compute_betti_numbers();

        assert_eq!(betti.b(0), 1);
    }

    #[test]
    fn test_circle_cohomology() {
        // Triangle boundary (circle): H^0 = R, H^1 = R
        let v0 = make_node_id();
        let v1 = make_node_id();
        let v2 = make_node_id();

        // Only edges, no filled triangle
        let nodes = vec![v0, v1, v2];
        let edges = vec![(v0, v1), (v1, v2), (v0, v2)];
        let complex = SimplicialComplex::from_graph_cliques(&nodes, &edges, 1);

        let mut computer = CohomologyComputer::with_default_config(complex);
        let betti = computer.compute_betti_numbers();

        assert_eq!(betti.b(0), 1); // Connected
        assert_eq!(betti.b(1), 1); // One hole
    }

    #[test]
    fn test_filled_triangle_cohomology() {
        // Filled triangle (disk): H^0 = R, H^n = 0 for n > 0
        let v0 = make_node_id();
        let v1 = make_node_id();
        let v2 = make_node_id();

        let nodes = vec![v0, v1, v2];
        let edges = vec![(v0, v1), (v1, v2), (v0, v2)];
        let complex = SimplicialComplex::from_graph_cliques(&nodes, &edges, 2);

        let mut computer = CohomologyComputer::with_default_config(complex);
        let betti = computer.compute_betti_numbers();

        assert_eq!(betti.b(0), 1); // Connected
        assert_eq!(betti.b(1), 0); // No hole (filled)
    }

    #[test]
    fn test_euler_characteristic() {
        let v0 = make_node_id();
        let v1 = make_node_id();
        let v2 = make_node_id();

        let nodes = vec![v0, v1, v2];
        let edges = vec![(v0, v1), (v1, v2), (v0, v2)];
        let complex = SimplicialComplex::from_graph_cliques(&nodes, &edges, 2);

        // Euler characteristic from simplices: 3 - 3 + 1 = 1
        assert_eq!(complex.euler_characteristic(), 1);

        let mut computer = CohomologyComputer::with_default_config(complex);
        let betti = computer.compute_betti_numbers();

        // Euler characteristic from Betti: b0 - b1 + b2 = 1 - 0 + 0 = 1
        assert_eq!(betti.euler_characteristic, 1);
    }
}
