//! Chain complex implementation

use super::Homology;
use crate::{Error, Result};
use nalgebra::DMatrix;

/// A chain complex for computing homology
///
/// A chain complex is a sequence of abelian groups (vector spaces) connected
/// by boundary maps: ... -> C_{n+1} -d_{n+1}-> C_n -d_n-> C_{n-1} -> ...
///
/// The key property is d_n ∘ d_{n+1} = 0 (boundary of boundary is zero).
#[derive(Debug, Clone)]
pub struct ChainComplex {
    /// Boundary maps d_n: C_n -> C_{n-1}
    boundary_maps: Vec<DMatrix<f64>>,
}

impl ChainComplex {
    /// Create a new chain complex from boundary maps
    pub fn new(boundary_maps: Vec<DMatrix<f64>>) -> Self {
        Self { boundary_maps }
    }

    /// Create a chain complex from dimensions and explicit maps
    pub fn from_dimensions(dimensions: &[usize]) -> Self {
        let mut maps = Vec::new();
        for i in 1..dimensions.len() {
            maps.push(DMatrix::zeros(dimensions[i - 1], dimensions[i]));
        }
        Self::new(maps)
    }

    /// Get the number of chain groups
    pub fn length(&self) -> usize {
        self.boundary_maps.len() + 1
    }

    /// Get the n-th boundary map
    pub fn boundary(&self, n: usize) -> Option<&DMatrix<f64>> {
        self.boundary_maps.get(n)
    }

    /// Set the n-th boundary map
    pub fn set_boundary(&mut self, n: usize, map: DMatrix<f64>) -> Result<()> {
        if n >= self.boundary_maps.len() {
            return Err(Error::InvalidTopology(format!(
                "Boundary index {} out of range",
                n
            )));
        }
        self.boundary_maps[n] = map;
        Ok(())
    }

    /// Check the chain complex property: d ∘ d = 0
    pub fn verify(&self, epsilon: f64) -> Result<bool> {
        for i in 0..self.boundary_maps.len().saturating_sub(1) {
            let d_i = &self.boundary_maps[i];
            let d_i1 = &self.boundary_maps[i + 1];

            // Check dimensions are compatible
            if d_i.ncols() != d_i1.nrows() {
                return Ok(false);
            }

            // Check d_i ∘ d_{i+1} = 0
            let composition = d_i * d_i1;
            if composition.norm() > epsilon {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Compute the n-th homology group H_n = ker(d_n) / im(d_{n+1})
    pub fn homology(&self, n: usize) -> Result<Homology> {
        // Get the relevant boundary maps
        let d_n = self.boundary_maps.get(n);
        let d_n1 = if n + 1 < self.boundary_maps.len() {
            Some(&self.boundary_maps[n + 1])
        } else {
            None
        };

        // Compute kernel of d_n
        let kernel_dim = if let Some(d) = d_n {
            compute_kernel_dimension(d)
        } else {
            // If no outgoing boundary, kernel is everything
            if n > 0 && n - 1 < self.boundary_maps.len() {
                self.boundary_maps[n - 1].ncols()
            } else {
                0
            }
        };

        // Compute image of d_{n+1}
        let image_dim = if let Some(d) = d_n1 {
            compute_image_dimension(d)
        } else {
            0
        };

        // Homology dimension = dim(ker) - dim(im)
        let homology_dim = kernel_dim.saturating_sub(image_dim);

        Ok(Homology::new(n, homology_dim))
    }

    /// Compute all homology groups
    pub fn all_homology(&self) -> Result<Vec<Homology>> {
        let mut result = Vec::new();
        for n in 0..self.length() {
            result.push(self.homology(n)?);
        }
        Ok(result)
    }

    /// Get the Betti numbers
    pub fn betti_numbers(&self) -> Result<Vec<usize>> {
        let homology = self.all_homology()?;
        Ok(homology.iter().map(|h| h.dimension()).collect())
    }
}

/// Compute the dimension of the kernel of a matrix
fn compute_kernel_dimension(matrix: &DMatrix<f64>) -> usize {
    // Use SVD to compute rank, kernel dimension = ncols - rank
    let svd = matrix.clone().svd(false, false);
    let singular_values = svd.singular_values;

    let threshold = 1e-10;
    let rank = singular_values.iter().filter(|&&s| s > threshold).count();

    matrix.ncols().saturating_sub(rank)
}

/// Compute the dimension of the image of a matrix
fn compute_image_dimension(matrix: &DMatrix<f64>) -> usize {
    // Image dimension = rank
    let svd = matrix.clone().svd(false, false);
    let singular_values = svd.singular_values;

    let threshold = 1e-10;
    singular_values.iter().filter(|&&s| s > threshold).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_complex_creation() {
        let d0 = DMatrix::from_row_slice(2, 3, &[1.0, -1.0, 0.0, 0.0, 1.0, -1.0]);

        let complex = ChainComplex::new(vec![d0]);
        assert_eq!(complex.length(), 2);
    }

    #[test]
    fn test_kernel_dimension() {
        // Identity matrix has trivial kernel
        let identity = DMatrix::identity(3, 3);
        assert_eq!(compute_kernel_dimension(&identity), 0);

        // Zero matrix has full kernel
        let zero = DMatrix::zeros(2, 3);
        assert_eq!(compute_kernel_dimension(&zero), 3);
    }

    #[test]
    fn test_image_dimension() {
        // Identity matrix has full image
        let identity = DMatrix::identity(3, 3);
        assert_eq!(compute_image_dimension(&identity), 3);

        // Zero matrix has trivial image
        let zero = DMatrix::zeros(2, 3);
        assert_eq!(compute_image_dimension(&zero), 0);
    }
}
