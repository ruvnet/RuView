//! Matrix operations and data structures for sparse linear algebra.
//!
//! This module provides efficient implementations of sparse matrix formats
//! optimized for asymmetric diagonally dominant systems, with support for
//! both traditional linear algebra operations and graph-based algorithms.

use crate::types::{Precision, DimensionType, IndexType, SparsityInfo, ConditioningInfo};
use crate::error::{SolverError, Result};
use alloc::{vec::Vec, string::String};
use core::fmt;

pub mod sparse;
pub mod optimized;

use sparse::*;

// Re-export optimized types for convenience
pub use optimized::{OptimizedCSRStorage, BufferPool, StreamingMatrix};

/// Trait defining the interface for matrix operations.
/// 
/// This trait abstracts over different matrix storage formats,
/// allowing algorithms to work with CSR, CSC, or graph adjacency
/// representations transparently.
pub trait Matrix: Send + Sync {
    /// Get the number of rows in the matrix.
    fn rows(&self) -> DimensionType;
    
    /// Get the number of columns in the matrix.
    fn cols(&self) -> DimensionType;
    
    /// Get a specific matrix element, returning None if it's zero or out of bounds.
    fn get(&self, row: usize, col: usize) -> Option<Precision>;
    
    /// Get an iterator over non-zero elements in a specific row.
    /// Returns (column_index, value) pairs.
    fn row_iter(&self, row: usize) -> Box<dyn Iterator<Item = (IndexType, Precision)> + '_>;
    
    /// Get an iterator over non-zero elements in a specific column.
    /// Returns (row_index, value) pairs.
    fn col_iter(&self, col: usize) -> Box<dyn Iterator<Item = (IndexType, Precision)> + '_>;
    
    /// Perform matrix-vector multiplication: result = A * x
    fn multiply_vector(&self, x: &[Precision], result: &mut [Precision]) -> Result<()>;
    
    /// Perform matrix-vector multiplication with accumulation: result += A * x
    fn multiply_vector_add(&self, x: &[Precision], result: &mut [Precision]) -> Result<()>;
    
    /// Check if the matrix is diagonally dominant.
    /// A matrix is diagonally dominant if |a_ii| >= Σ_{j≠i} |a_ij| for all i.
    fn is_diagonally_dominant(&self) -> bool;
    
    /// Get the diagonal dominance factor (minimum ratio of diagonal to off-diagonal).
    fn diagonal_dominance_factor(&self) -> Option<Precision>;
    
    /// Get the number of non-zero elements.
    fn nnz(&self) -> usize;
    
    /// Get sparsity pattern information.
    fn sparsity_info(&self) -> SparsityInfo;
    
    /// Get matrix conditioning information.
    fn conditioning_info(&self) -> ConditioningInfo;
    
    /// Get the storage format name.
    fn format_name(&self) -> &'static str;
    
    /// Check if the matrix is square.
    fn is_square(&self) -> bool {
        self.rows() == self.cols()
    }
    
    /// Get the Frobenius norm of the matrix.
    fn frobenius_norm(&self) -> Precision {
        let mut norm_sq = 0.0;
        for row in 0..self.rows() {
            for (_, value) in self.row_iter(row) {
                norm_sq += value * value;
            }
        }
        norm_sq.sqrt()
    }
    
    /// Estimate the spectral radius (largest eigenvalue magnitude).
    /// Uses Gershgorin circle theorem for a conservative estimate.
    fn spectral_radius_estimate(&self) -> Precision {
        let mut max_radius: Precision = 0.0;
        for row in 0..self.rows() {
            let mut diagonal = 0.0;
            let mut off_diagonal_sum = 0.0;
            
            for (col, value) in self.row_iter(row) {
                if col as usize == row {
                    diagonal = value.abs();
                } else {
                    off_diagonal_sum += value.abs();
                }
            }
            
            max_radius = max_radius.max(diagonal + off_diagonal_sum);
        }
        max_radius
    }
}

/// Sparse matrix storage formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SparseFormat {
    /// Compressed Sparse Row format - efficient for row-wise operations
    CSR,
    /// Compressed Sparse Column format - efficient for column-wise operations  
    CSC,
    /// Coordinate format - efficient for construction and random access
    COO,
    /// Graph adjacency list - efficient for graph algorithms
    GraphAdjacency,
}

/// Main sparse matrix implementation supporting multiple storage formats.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SparseMatrix {
    /// Current storage format
    format: SparseFormat,
    /// Matrix dimensions
    rows: DimensionType,
    cols: DimensionType,
    /// Storage implementation
    storage: SparseStorage,
}

/// Internal storage implementation for different sparse formats.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum SparseStorage {
    CSR(CSRStorage),
    CSC(CSCStorage),
    COO(COOStorage),
    Graph(GraphStorage),
}

impl SparseMatrix {
    /// Create a new sparse matrix from coordinate (triplet) format.
    /// 
    /// # Arguments
    /// * `triplets` - Vector of (row, col, value) triplets
    /// * `rows` - Number of rows
    /// * `cols` - Number of columns
    /// 
    /// # Example
    /// ```
    /// use sublinear_solver::SparseMatrix;
    /// 
    /// let matrix = SparseMatrix::from_triplets(
    ///     vec![(0, 0, 4.0), (0, 1, 1.0), (1, 0, 2.0), (1, 1, 5.0)],
    ///     2, 2
    /// ).unwrap();
    /// ```
    pub fn from_triplets(
        triplets: Vec<(usize, usize, Precision)>,
        rows: DimensionType,
        cols: DimensionType,
    ) -> Result<Self> {
        // Validate input
        for &(r, c, v) in &triplets {
            if r >= rows {
                return Err(SolverError::IndexOutOfBounds {
                    index: r,
                    max_index: rows - 1,
                    context: "row index in triplet".to_string(),
                });
            }
            if c >= cols {
                return Err(SolverError::IndexOutOfBounds {
                    index: c,
                    max_index: cols - 1,
                    context: "column index in triplet".to_string(),
                });
            }
            if !v.is_finite() {
                return Err(SolverError::InvalidInput {
                    message: format!("Non-finite value {} at ({}, {})", v, r, c),
                    parameter: Some("matrix_element".to_string()),
                });
            }
        }
        
        // Create COO storage first, then convert to CSR for efficiency
        let coo_storage = COOStorage::from_triplets(triplets)?;
        let csr_storage = CSRStorage::from_coo(&coo_storage, rows, cols)?;
        
        Ok(Self {
            format: SparseFormat::CSR,
            rows,
            cols,
            storage: SparseStorage::CSR(csr_storage),
        })
    }
    
    /// Create a sparse matrix from dense row-major data.
    /// 
    /// Zero elements are automatically filtered out.
    pub fn from_dense(data: &[Precision], rows: DimensionType, cols: DimensionType) -> Result<Self> {
        if data.len() != rows * cols {
            return Err(SolverError::DimensionMismatch {
                expected: rows * cols,
                actual: data.len(),
                operation: "dense_to_sparse_conversion".to_string(),
            });
        }
        
        let mut triplets = Vec::new();
        for (i, &value) in data.iter().enumerate() {
            if value != 0.0 {
                let row = i / cols;
                let col = i % cols;
                triplets.push((row, col, value));
            }
        }
        
        Self::from_triplets(triplets, rows, cols)
    }
    
    /// Create an identity matrix of the given size.
    pub fn identity(size: DimensionType) -> Result<Self> {
        let triplets: Vec<_> = (0..size).map(|i| (i, i, 1.0)).collect();
        Self::from_triplets(triplets, size, size)
    }
    
    /// Create a diagonal matrix from the given diagonal values.
    pub fn diagonal(diag: &[Precision]) -> Result<Self> {
        let size = diag.len();
        let triplets: Vec<_> = diag.iter().enumerate()
            .filter(|(_, &v)| v != 0.0)
            .map(|(i, &v)| (i, i, v))
            .collect();
        Self::from_triplets(triplets, size, size)
    }
    
    /// Convert the matrix to a different storage format.
    /// 
    /// This operation may be expensive for large matrices.
    pub fn convert_to_format(&mut self, new_format: SparseFormat) -> Result<()> {
        if self.format == new_format {
            return Ok(());
        }
        
        match (self.format, new_format) {
            (SparseFormat::CSR, SparseFormat::CSC) => {
                if let SparseStorage::CSR(ref csr) = self.storage {
                    let csc = CSCStorage::from_csr(csr, self.rows, self.cols)?;
                    self.storage = SparseStorage::CSC(csc);
                    self.format = SparseFormat::CSC;
                }
            },
            (SparseFormat::CSC, SparseFormat::CSR) => {
                if let SparseStorage::CSC(ref csc) = self.storage {
                    let csr = CSRStorage::from_csc(csc, self.rows, self.cols)?;
                    self.storage = SparseStorage::CSR(csr);
                    self.format = SparseFormat::CSR;
                }
            },
            (_, SparseFormat::GraphAdjacency) => {
                // Convert any format to graph adjacency
                let triplets = self.to_triplets()?;
                let graph = GraphStorage::from_triplets(triplets, self.rows)?;
                self.storage = SparseStorage::Graph(graph);
                self.format = SparseFormat::GraphAdjacency;
            },
            _ => {
                // For other conversions, go through COO format
                let triplets = self.to_triplets()?;
                let coo = COOStorage::from_triplets(triplets)?;
                
                match new_format {
                    SparseFormat::CSR => {
                        let csr = CSRStorage::from_coo(&coo, self.rows, self.cols)?;
                        self.storage = SparseStorage::CSR(csr);
                    },
                    SparseFormat::CSC => {
                        let csc = CSCStorage::from_coo(&coo, self.rows, self.cols)?;
                        self.storage = SparseStorage::CSC(csc);
                    },
                    SparseFormat::COO => {
                        self.storage = SparseStorage::COO(coo);
                    },
                    _ => unreachable!(),
                }
                self.format = new_format;
            }
        }
        
        Ok(())
    }
    
    /// Extract the matrix as coordinate triplets.
    pub fn to_triplets(&self) -> Result<Vec<(usize, usize, Precision)>> {
        match &self.storage {
            SparseStorage::CSR(csr) => csr.to_triplets(),
            SparseStorage::CSC(csc) => csc.to_triplets(),
            SparseStorage::COO(coo) => Ok(coo.to_triplets()),
            SparseStorage::Graph(graph) => graph.to_triplets(),
        }
    }
    
    /// Get the current storage format.
    pub fn format(&self) -> SparseFormat {
        self.format
    }
    
    /// Get a reference to the underlying CSR storage.
    /// 
    /// Converts to CSR format if necessary.
    pub fn as_csr(&mut self) -> Result<&CSRStorage> {
        self.convert_to_format(SparseFormat::CSR)?;
        match &self.storage {
            SparseStorage::CSR(csr) => Ok(csr),
            _ => unreachable!(),
        }
    }
    
    /// Get a reference to the underlying CSC storage.
    /// 
    /// Converts to CSC format if necessary.
    pub fn as_csc(&mut self) -> Result<&CSCStorage> {
        self.convert_to_format(SparseFormat::CSC)?;
        match &self.storage {
            SparseStorage::CSC(csc) => Ok(csc),
            _ => unreachable!(),
        }
    }
    
    /// Get a reference to the underlying graph storage.
    /// 
    /// Converts to graph format if necessary.
    pub fn as_graph(&mut self) -> Result<&GraphStorage> {
        self.convert_to_format(SparseFormat::GraphAdjacency)?;
        match &self.storage {
            SparseStorage::Graph(graph) => Ok(graph),
            _ => unreachable!(),
        }
    }
    
    /// Scale the matrix by a scalar value.
    pub fn scale(&mut self, factor: Precision) {
        match &mut self.storage {
            SparseStorage::CSR(csr) => csr.scale(factor),
            SparseStorage::CSC(csc) => csc.scale(factor),
            SparseStorage::COO(coo) => coo.scale(factor),
            SparseStorage::Graph(graph) => graph.scale(factor),
        }
    }
    
    /// Add a scalar multiple of the identity matrix: A = A + alpha * I
    pub fn add_diagonal(&mut self, alpha: Precision) -> Result<()> {
        if !self.is_square() {
            return Err(SolverError::InvalidInput {
                message: "Cannot add diagonal to non-square matrix".to_string(),
                parameter: Some("matrix_dimensions".to_string()),
            });
        }
        
        match &mut self.storage {
            SparseStorage::CSR(csr) => csr.add_diagonal(alpha),
            SparseStorage::CSC(csc) => csc.add_diagonal(alpha),
            SparseStorage::COO(coo) => coo.add_diagonal(alpha, self.rows),
            SparseStorage::Graph(graph) => graph.add_diagonal(alpha),
        }
        
        Ok(())
    }
}

impl Matrix for SparseMatrix {
    fn rows(&self) -> DimensionType {
        self.rows
    }
    
    fn cols(&self) -> DimensionType {
        self.cols
    }
    
    fn get(&self, row: usize, col: usize) -> Option<Precision> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        
        match &self.storage {
            SparseStorage::CSR(csr) => csr.get(row, col),
            SparseStorage::CSC(csc) => csc.get(row, col),
            SparseStorage::COO(coo) => coo.get(row, col),
            SparseStorage::Graph(graph) => graph.get(row, col),
        }
    }
    
    fn row_iter(&self, row: usize) -> Box<dyn Iterator<Item = (IndexType, Precision)> + '_> {
        match &self.storage {
            SparseStorage::CSR(csr) => Box::new(csr.row_iter(row)),
            SparseStorage::CSC(csc) => Box::new(csc.row_iter(row)),
            SparseStorage::COO(coo) => Box::new(coo.row_iter(row)),
            SparseStorage::Graph(graph) => Box::new(graph.row_iter(row)),
        }
    }
    
    fn col_iter(&self, col: usize) -> Box<dyn Iterator<Item = (IndexType, Precision)> + '_> {
        match &self.storage {
            SparseStorage::CSR(csr) => Box::new(csr.col_iter(col)),
            SparseStorage::CSC(csc) => Box::new(csc.col_iter(col)),
            SparseStorage::COO(coo) => Box::new(coo.col_iter(col)),
            SparseStorage::Graph(graph) => Box::new(graph.col_iter(col)),
        }
    }
    
    fn multiply_vector(&self, x: &[Precision], result: &mut [Precision]) -> Result<()> {
        if x.len() != self.cols {
            return Err(SolverError::DimensionMismatch {
                expected: self.cols,
                actual: x.len(),
                operation: "matrix_vector_multiply".to_string(),
            });
        }
        if result.len() != self.rows {
            return Err(SolverError::DimensionMismatch {
                expected: self.rows,
                actual: result.len(),
                operation: "matrix_vector_multiply".to_string(),
            });
        }
        
        match &self.storage {
            SparseStorage::CSR(csr) => csr.multiply_vector(x, result),
            SparseStorage::CSC(csc) => csc.multiply_vector(x, result),
            SparseStorage::COO(coo) => coo.multiply_vector(x, result),
            SparseStorage::Graph(graph) => graph.multiply_vector(x, result),
        }
        
        Ok(())
    }
    
    fn multiply_vector_add(&self, x: &[Precision], result: &mut [Precision]) -> Result<()> {
        if x.len() != self.cols {
            return Err(SolverError::DimensionMismatch {
                expected: self.cols,
                actual: x.len(),
                operation: "matrix_vector_multiply_add".to_string(),
            });
        }
        if result.len() != self.rows {
            return Err(SolverError::DimensionMismatch {
                expected: self.rows,
                actual: result.len(),
                operation: "matrix_vector_multiply_add".to_string(),
            });
        }
        
        match &self.storage {
            SparseStorage::CSR(csr) => csr.multiply_vector_add(x, result),
            SparseStorage::CSC(csc) => csc.multiply_vector_add(x, result),
            SparseStorage::COO(coo) => coo.multiply_vector_add(x, result),
            SparseStorage::Graph(graph) => graph.multiply_vector_add(x, result),
        }
        
        Ok(())
    }
    
    fn is_diagonally_dominant(&self) -> bool {
        for row in 0..self.rows {
            let mut diagonal = 0.0;
            let mut off_diagonal_sum = 0.0;
            
            for (col, value) in self.row_iter(row) {
                if col as usize == row {
                    diagonal = value.abs();
                } else {
                    off_diagonal_sum += value.abs();
                }
            }
            
            if diagonal < off_diagonal_sum {
                return false;
            }
        }
        true
    }
    
    fn diagonal_dominance_factor(&self) -> Option<Precision> {
        let mut min_factor = Precision::INFINITY;
        
        for row in 0..self.rows {
            let mut diagonal = 0.0;
            let mut off_diagonal_sum = 0.0;
            
            for (col, value) in self.row_iter(row) {
                if col as usize == row {
                    diagonal = value.abs();
                } else {
                    off_diagonal_sum += value.abs();
                }
            }
            
            if off_diagonal_sum > 0.0 {
                let factor = diagonal / off_diagonal_sum;
                min_factor = min_factor.min(factor);
            }
        }
        
        if min_factor.is_finite() {
            Some(min_factor)
        } else {
            None
        }
    }
    
    fn nnz(&self) -> usize {
        match &self.storage {
            SparseStorage::CSR(csr) => csr.nnz(),
            SparseStorage::CSC(csc) => csc.nnz(),
            SparseStorage::COO(coo) => coo.nnz(),
            SparseStorage::Graph(graph) => graph.nnz(),
        }
    }
    
    fn sparsity_info(&self) -> SparsityInfo {
        let mut info = SparsityInfo::new(self.nnz(), self.rows, self.cols);
        
        // Compute additional statistics
        let mut max_nnz_per_row = 0;
        for row in 0..self.rows {
            let row_nnz = self.row_iter(row).count();
            max_nnz_per_row = max_nnz_per_row.max(row_nnz);
        }
        info.max_nnz_per_row = max_nnz_per_row;
        
        // Check for banded structure (simple heuristic)
        let mut max_bandwidth = 0;
        for (r, c, _) in self.to_triplets().unwrap_or_default() {
            let bandwidth = if r > c { r - c } else { c - r };
            max_bandwidth = max_bandwidth.max(bandwidth);
        }
        info.bandwidth = Some(max_bandwidth);
        info.is_banded = max_bandwidth < self.rows / 4; // Heuristic: banded if bandwidth < 25% of size
        
        info
    }
    
    fn conditioning_info(&self) -> ConditioningInfo {
        ConditioningInfo {
            condition_number: None, // Expensive to compute exactly
            is_diagonally_dominant: self.is_diagonally_dominant(),
            diagonal_dominance_factor: self.diagonal_dominance_factor(),
            spectral_radius: Some(self.spectral_radius_estimate()),
            is_positive_definite: None, // Expensive to determine
        }
    }
    
    fn format_name(&self) -> &'static str {
        match self.format {
            SparseFormat::CSR => "CSR",
            SparseFormat::CSC => "CSC",
            SparseFormat::COO => "COO",
            SparseFormat::GraphAdjacency => "GraphAdjacency",
        }
    }
}

impl fmt::Display for SparseMatrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}x{} sparse matrix ({} format, {} nnz)", 
               self.rows, self.cols, self.format_name(), self.nnz())
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    
    #[test]
    fn test_matrix_creation() {
        let triplets = vec![(0, 0, 4.0), (0, 1, 1.0), (1, 0, 2.0), (1, 1, 5.0)];
        let matrix = SparseMatrix::from_triplets(triplets, 2, 2).unwrap();
        
        assert_eq!(matrix.rows(), 2);
        assert_eq!(matrix.cols(), 2);
        assert_eq!(matrix.nnz(), 4);
        assert!(matrix.is_diagonally_dominant());
    }
    
    #[test]
    fn test_matrix_vector_multiply() {
        let triplets = vec![(0, 0, 2.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 3.0)];
        let matrix = SparseMatrix::from_triplets(triplets, 2, 2).unwrap();
        
        let x = vec![1.0, 2.0];
        let mut result = vec![0.0; 2];
        
        matrix.multiply_vector(&x, &mut result).unwrap();
        
        assert_eq!(result, vec![4.0, 7.0]); // [2*1 + 1*2, 1*1 + 3*2]
    }
    
    #[test]
    fn test_diagonal_dominance() {
        // Diagonally dominant matrix
        let triplets = vec![(0, 0, 5.0), (0, 1, 1.0), (1, 0, 2.0), (1, 1, 7.0)];
        let matrix = SparseMatrix::from_triplets(triplets, 2, 2).unwrap();
        assert!(matrix.is_diagonally_dominant());
        
        // Not diagonally dominant
        let triplets = vec![(0, 0, 1.0), (0, 1, 3.0), (1, 0, 2.0), (1, 1, 2.0)];
        let matrix = SparseMatrix::from_triplets(triplets, 2, 2).unwrap();
        assert!(!matrix.is_diagonally_dominant());
    }
    
    #[test]
    fn test_format_conversion() {
        let triplets = vec![(0, 0, 1.0), (0, 2, 2.0), (1, 1, 3.0)];
        let mut matrix = SparseMatrix::from_triplets(triplets, 2, 3).unwrap();
        
        assert_eq!(matrix.format(), SparseFormat::CSR);
        
        matrix.convert_to_format(SparseFormat::CSC).unwrap();
        assert_eq!(matrix.format(), SparseFormat::CSC);
        
        matrix.convert_to_format(SparseFormat::GraphAdjacency).unwrap();
        assert_eq!(matrix.format(), SparseFormat::GraphAdjacency);
    }
}