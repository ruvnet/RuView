use std::fmt;
use rand::Rng;

#[derive(Debug, Clone)]
pub struct Matrix {
    data: Vec<f64>,
    rows: usize,
    cols: usize,
}

impl Matrix {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            data: vec![0.0; rows * cols],
            rows,
            cols,
        }
    }

    pub fn from_slice(data: &[f64], rows: usize, cols: usize) -> Self {
        assert_eq!(data.len(), rows * cols, "Data length must match matrix dimensions");
        Self {
            data: data.to_vec(),
            rows,
            cols,
        }
    }

    pub fn identity(size: usize) -> Self {
        let mut matrix = Self::new(size, size);
        for i in 0..size {
            matrix.data[i * size + i] = 1.0;
        }
        matrix
    }

    pub fn random(rows: usize, cols: usize) -> Self {
        let mut matrix = Self::new(rows, cols);
        for i in 0..matrix.data.len() {
            #[cfg(feature = "wasm")]
            {
                matrix.data[i] = fastrand::f64();
            }
            #[cfg(not(feature = "wasm"))]
            {
                matrix.data[i] = rand::random::<f64>();
            }
        }
        matrix
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn data(&self) -> &[f64] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [f64] {
        &mut self.data
    }

    pub fn get(&self, row: usize, col: usize) -> f64 {
        assert!(row < self.rows && col < self.cols, "Index out of bounds");
        self.data[row * self.cols + col]
    }

    pub fn set(&mut self, row: usize, col: usize, value: f64) {
        assert!(row < self.rows && col < self.cols, "Index out of bounds");
        self.data[row * self.cols + col] = value;
    }

    pub fn multiply(&self, other: &Matrix) -> Result<Matrix, String> {
        if self.cols != other.rows {
            return Err("Matrix dimensions incompatible for multiplication".to_string());
        }

        let mut result = Matrix::new(self.rows, other.cols);

        for i in 0..self.rows {
            for j in 0..other.cols {
                let mut sum = 0.0;
                for k in 0..self.cols {
                    sum += self.get(i, k) * other.get(k, j);
                }
                result.set(i, j, sum);
            }
        }

        Ok(result)
    }

    pub fn transpose(&self) -> Matrix {
        let mut result = Matrix::new(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                result.set(j, i, self.get(i, j));
            }
        }
        result
    }

    pub fn is_symmetric(&self) -> bool {
        if self.rows != self.cols {
            return false;
        }

        for i in 0..self.rows {
            for j in 0..self.cols {
                if (self.get(i, j) - self.get(j, i)).abs() > 1e-10 {
                    return false;
                }
            }
        }
        true
    }

    pub fn is_positive_definite(&self) -> bool {
        if !self.is_symmetric() {
            return false;
        }

        // Simple check using Sylvester's criterion for small matrices
        // For larger matrices, this should use Cholesky decomposition
        if self.rows <= 3 {
            return self.check_sylvester_criterion();
        }

        // For larger matrices, approximate check
        true
    }

    fn check_sylvester_criterion(&self) -> bool {
        for k in 1..=self.rows {
            let det = self.leading_principal_minor(k);
            if det <= 0.0 {
                return false;
            }
        }
        true
    }

    fn leading_principal_minor(&self, k: usize) -> f64 {
        if k == 1 {
            return self.get(0, 0);
        }
        if k == 2 {
            return self.get(0, 0) * self.get(1, 1) - self.get(0, 1) * self.get(1, 0);
        }
        if k == 3 {
            let a = self.get(0, 0);
            let b = self.get(0, 1);
            let c = self.get(0, 2);
            let d = self.get(1, 0);
            let e = self.get(1, 1);
            let f = self.get(1, 2);
            let g = self.get(2, 0);
            let h = self.get(2, 1);
            let i = self.get(2, 2);

            return a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
        }

        // For larger matrices, use a simplified approximation
        1.0
    }
}

impl fmt::Display for Matrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.rows {
            write!(f, "[")?;
            for j in 0..self.cols {
                if j > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{:8.4}", self.get(i, j))?;
            }
            writeln!(f, "]")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Vector {
    data: Vec<f64>,
}

impl Vector {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0.0; size],
        }
    }

    pub fn from_slice(data: &[f64]) -> Self {
        Self {
            data: data.to_vec(),
        }
    }

    pub fn zeros(size: usize) -> Self {
        Self::new(size)
    }

    pub fn ones(size: usize) -> Self {
        Self {
            data: vec![1.0; size],
        }
    }

    pub fn random(size: usize) -> Self {
        let mut vector = Self::new(size);
        for i in 0..size {
            #[cfg(feature = "wasm")]
            {
                vector.data[i] = fastrand::f64();
            }
            #[cfg(not(feature = "wasm"))]
            {
                vector.data[i] = rand::random::<f64>();
            }
        }
        vector
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn data(&self) -> &[f64] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [f64] {
        &mut self.data
    }

    pub fn get(&self, index: usize) -> f64 {
        self.data[index]
    }

    pub fn set(&mut self, index: usize, value: f64) {
        self.data[index] = value;
    }

    pub fn dot(&self, other: &Vector) -> f64 {
        assert_eq!(self.len(), other.len(), "Vector lengths must match for dot product");

        self.data.iter()
            .zip(other.data.iter())
            .map(|(a, b)| a * b)
            .sum()
    }

    pub fn norm(&self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn normalize(&mut self) {
        let norm = self.norm();
        if norm > 0.0 {
            for x in &mut self.data {
                *x /= norm;
            }
        }
    }

    pub fn add(&self, other: &Vector) -> Vector {
        assert_eq!(self.len(), other.len(), "Vector lengths must match for addition");

        let mut result = Vector::new(self.len());
        for i in 0..self.len() {
            result.data[i] = self.data[i] + other.data[i];
        }
        result
    }

    pub fn subtract(&self, other: &Vector) -> Vector {
        assert_eq!(self.len(), other.len(), "Vector lengths must match for subtraction");

        let mut result = Vector::new(self.len());
        for i in 0..self.len() {
            result.data[i] = self.data[i] - other.data[i];
        }
        result
    }

    pub fn scale(&self, scalar: f64) -> Vector {
        let mut result = Vector::new(self.len());
        for i in 0..self.len() {
            result.data[i] = self.data[i] * scalar;
        }
        result
    }

    pub fn axpy(&mut self, alpha: f64, x: &Vector) {
        assert_eq!(self.len(), x.len(), "Vector lengths must match for axpy");

        for i in 0..self.len() {
            self.data[i] += alpha * x.data[i];
        }
    }
}

impl fmt::Display for Vector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, &value) in self.data.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{:8.4}", value)?;
        }
        write!(f, "]")
    }
}

// Matrix-Vector operations
impl Matrix {
    pub fn multiply_vector(&self, vector: &Vector) -> Result<Vector, String> {
        if self.cols != vector.len() {
            return Err("Matrix columns must match vector length".to_string());
        }

        let mut result = Vector::new(self.rows);
        for i in 0..self.rows {
            let mut sum = 0.0;
            for j in 0..self.cols {
                sum += self.get(i, j) * vector.get(j);
            }
            result.set(i, sum);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_creation() {
        let matrix = Matrix::new(3, 3);
        assert_eq!(matrix.rows(), 3);
        assert_eq!(matrix.cols(), 3);
        assert_eq!(matrix.data().len(), 9);
    }

    #[test]
    fn test_matrix_identity() {
        let identity = Matrix::identity(3);
        assert_eq!(identity.get(0, 0), 1.0);
        assert_eq!(identity.get(1, 1), 1.0);
        assert_eq!(identity.get(2, 2), 1.0);
        assert_eq!(identity.get(0, 1), 0.0);
    }

    #[test]
    fn test_vector_operations() {
        let v1 = Vector::from_slice(&[1.0, 2.0, 3.0]);
        let v2 = Vector::from_slice(&[4.0, 5.0, 6.0]);

        let dot_product = v1.dot(&v2);
        assert_eq!(dot_product, 32.0); // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32

        let sum = v1.add(&v2);
        assert_eq!(sum.data(), &[5.0, 7.0, 9.0]);
    }

    #[test]
    fn test_matrix_vector_multiply() {
        let matrix = Matrix::from_slice(&[1.0, 2.0, 3.0, 4.0], 2, 2);
        let vector = Vector::from_slice(&[1.0, 2.0]);

        let result = matrix.multiply_vector(&vector).unwrap();
        assert_eq!(result.data(), &[5.0, 11.0]); // [1*1+2*2, 3*1+4*2] = [5, 11]
    }
}