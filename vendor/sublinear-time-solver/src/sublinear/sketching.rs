//! Matrix sketching algorithms for sublinear solvers

use crate::types::Precision;
use crate::error::{SolverError, Result};
use alloc::{vec::Vec, string::String};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

/// Sketching method
#[derive(Debug, Clone, PartialEq)]
pub enum SketchingMethod {
    /// Count-Sketch
    CountSketch,
    /// Sparse embedding
    SparseEmbedding,
    /// Fast Johnson-Lindenstrauss
    FastJL,
}

/// Matrix sketching engine
#[derive(Debug)]
pub struct MatrixSketch {
    method: SketchingMethod,
    sketch_size: usize,
    original_size: usize,
    hash_functions: Vec<usize>,
    sign_functions: Vec<i8>,
    rng: StdRng,
}

impl MatrixSketch {
    /// Create new matrix sketch
    pub fn new(
        method: SketchingMethod,
        original_size: usize,
        sketch_size: usize,
        seed: Option<u64>,
    ) -> Result<Self> {
        if sketch_size > original_size {
            return Err(SolverError::InvalidInput {
                message: "Sketch size must be <= original size".to_string(),
                parameter: Some("sketch_size".to_string()),
            });
        }

        let mut rng = match seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };

        // Generate hash and sign functions for Count-Sketch
        let mut hash_functions = Vec::with_capacity(original_size);
        let mut sign_functions = Vec::with_capacity(original_size);

        for _ in 0..original_size {
            hash_functions.push(rng.gen_range(0..sketch_size));
            sign_functions.push(if rng.gen::<bool>() { 1 } else { -1 });
        }

        Ok(Self {
            method,
            sketch_size,
            original_size,
            hash_functions,
            sign_functions,
            rng,
        })
    }

    /// Sketch a vector
    pub fn sketch_vector(&self, vector: &[Precision]) -> Result<Vec<Precision>> {
        if vector.len() != self.original_size {
            return Err(SolverError::DimensionMismatch {
                expected: self.original_size,
                actual: vector.len(),
                operation: "sketch_vector".to_string(),
            });
        }

        match self.method {
            SketchingMethod::CountSketch => self.count_sketch_vector(vector),
            SketchingMethod::SparseEmbedding => self.sparse_embed_vector(vector),
            SketchingMethod::FastJL => self.fast_jl_vector(vector),
        }
    }

    /// Count-Sketch implementation
    fn count_sketch_vector(&self, vector: &[Precision]) -> Result<Vec<Precision>> {
        let mut sketch = vec![0.0; self.sketch_size];

        for (i, &value) in vector.iter().enumerate() {
            let hash_idx = self.hash_functions[i];
            let sign = self.sign_functions[i] as Precision;
            sketch[hash_idx] += sign * value;
        }

        Ok(sketch)
    }

    /// Sparse embedding implementation
    fn sparse_embed_vector(&self, vector: &[Precision]) -> Result<Vec<Precision>> {
        let sparsity = 0.1; // 10% non-zero entries
        let mut sketch = vec![0.0; self.sketch_size];
        let scale = (1.0_f64 / sparsity).sqrt();

        for (i, &value) in vector.iter().enumerate() {
            if (i * 31) % self.sketch_size < (self.sketch_size as f64 * sparsity) as usize {
                let sketch_idx = (i * 17) % self.sketch_size;
                let sign = if (i * 13) % 2 == 0 { 1.0 } else { -1.0 };
                sketch[sketch_idx] += sign * scale * value;
            }
        }

        Ok(sketch)
    }

    /// Fast Johnson-Lindenstrauss implementation
    fn fast_jl_vector(&self, vector: &[Precision]) -> Result<Vec<Precision>> {
        // Simplified Fast JL using random signs and subsampling
        let mut sketch = vec![0.0; self.sketch_size];
        let scale = (self.original_size as f64 / self.sketch_size as f64).sqrt();

        for i in 0..self.sketch_size {
            let start_idx = (i * self.original_size) / self.sketch_size;
            let end_idx = ((i + 1) * self.original_size) / self.sketch_size;

            let mut sum = 0.0;
            for j in start_idx..end_idx {
                let sign = self.sign_functions[j % self.sign_functions.len()] as f64;
                sum += sign * vector[j];
            }

            sketch[i] = sum / scale;
        }

        Ok(sketch)
    }

    /// Reconstruct approximate vector (for methods that support it)
    pub fn reconstruct_vector(&self, sketch: &[Precision]) -> Result<Vec<Precision>> {
        if sketch.len() != self.sketch_size {
            return Err(SolverError::DimensionMismatch {
                expected: self.sketch_size,
                actual: sketch.len(),
                operation: "reconstruct_vector".to_string(),
            });
        }

        match self.method {
            SketchingMethod::CountSketch => self.count_sketch_reconstruct(sketch),
            _ => {
                // Simple upsampling for other methods
                let mut reconstructed = vec![0.0; self.original_size];
                let ratio = self.original_size / self.sketch_size;

                for (i, &value) in sketch.iter().enumerate() {
                    for j in 0..ratio {
                        let idx = i * ratio + j;
                        if idx < self.original_size {
                            reconstructed[idx] = value;
                        }
                    }
                }

                Ok(reconstructed)
            }
        }
    }

    /// Reconstruct from Count-Sketch
    fn count_sketch_reconstruct(&self, sketch: &[Precision]) -> Result<Vec<Precision>> {
        let mut reconstructed = vec![0.0; self.original_size];

        // Simple reconstruction: use sketch values at hash positions
        for i in 0..self.original_size {
            let hash_idx = self.hash_functions[i];
            let sign = self.sign_functions[i] as Precision;
            reconstructed[i] = sign * sketch[hash_idx];
        }

        Ok(reconstructed)
    }

    /// Get compression ratio
    pub fn compression_ratio(&self) -> Precision {
        self.sketch_size as Precision / self.original_size as Precision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_sketch_creation() {
        let sketch = MatrixSketch::new(
            SketchingMethod::CountSketch,
            100,
            50,
            Some(42),
        ).unwrap();

        assert_eq!(sketch.original_size, 100);
        assert_eq!(sketch.sketch_size, 50);
        assert_eq!(sketch.compression_ratio(), 0.5);
    }

    #[test]
    fn test_count_sketch() {
        let sketch = MatrixSketch::new(
            SketchingMethod::CountSketch,
            10,
            5,
            Some(123),
        ).unwrap();

        let vector = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let sketched = sketch.sketch_vector(&vector).unwrap();

        assert_eq!(sketched.len(), 5);

        let reconstructed = sketch.reconstruct_vector(&sketched).unwrap();
        assert_eq!(reconstructed.len(), 10);
    }

    #[test]
    fn test_sparse_embedding() {
        let sketch = MatrixSketch::new(
            SketchingMethod::SparseEmbedding,
            20,
            10,
            Some(456),
        ).unwrap();

        let vector = vec![1.0; 20];
        let sketched = sketch.sketch_vector(&vector).unwrap();

        assert_eq!(sketched.len(), 10);
    }

    #[test]
    fn test_fast_jl() {
        let sketch = MatrixSketch::new(
            SketchingMethod::FastJL,
            16,
            8,
            Some(789),
        ).unwrap();

        let vector = (1..=16).map(|x| x as f64).collect::<Vec<_>>();
        let sketched = sketch.sketch_vector(&vector).unwrap();

        assert_eq!(sketched.len(), 8);
    }
}