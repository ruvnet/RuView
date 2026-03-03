use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[derive(Serialize, Deserialize)]
pub struct MatrixData {
    pub values: Vec<f64>,
    pub col_indices: Vec<u32>,
    pub row_ptr: Vec<u32>,
    pub rows: usize,
    pub cols: usize,
}

#[derive(Serialize, Deserialize)]
pub struct SolverResult {
    pub solution: Vec<f64>,
    pub iterations: usize,
    pub residual: f64,
    pub converged: bool,
    pub compute_time_ms: f64,
}

// Fast CSR Matrix implementation
pub struct FastCSRMatrix {
    values: Vec<f64>,
    col_indices: Vec<u32>,
    row_ptr: Vec<u32>,
    rows: usize,
    cols: usize,
}

impl FastCSRMatrix {
    pub fn new(data: MatrixData) -> Self {
        Self {
            values: data.values,
            col_indices: data.col_indices,
            row_ptr: data.row_ptr,
            rows: data.rows,
            cols: data.cols,
        }
    }

    pub fn multiply_vector(&self, x: &[f64], result: &mut [f64]) {
        for i in 0..self.rows {
            let start = self.row_ptr[i] as usize;
            let end = self.row_ptr[i + 1] as usize;
            let mut sum = 0.0;

            for k in start..end {
                let j = self.col_indices[k] as usize;
                sum += self.values[k] * x[j];
            }

            result[i] = sum;
        }
    }

    pub fn get_diagonal(&self) -> Vec<f64> {
        let mut diagonal = vec![0.0; self.rows];

        for i in 0..self.rows {
            let start = self.row_ptr[i] as usize;
            let end = self.row_ptr[i + 1] as usize;

            for k in start..end {
                if self.col_indices[k] as usize == i {
                    diagonal[i] = self.values[k];
                    break;
                }
            }
        }

        diagonal
    }
}

// Fast Conjugate Gradient solver
pub struct FastConjugateGradient {
    matrix: FastCSRMatrix,
    b: Vec<f64>,
    x: Vec<f64>,
    r: Vec<f64>,
    p: Vec<f64>,
    iterations: usize,
}

impl FastConjugateGradient {
    pub fn new(matrix: FastCSRMatrix, b: Vec<f64>) -> Self {
        let n = matrix.rows;
        Self {
            matrix,
            b: b.clone(),
            x: vec![0.0; n],
            r: b,
            p: vec![0.0; n],
            iterations: 0,
        }
    }

    pub fn solve(&mut self, tolerance: f64, max_iterations: usize) -> Vec<f64> {
        let n = self.matrix.rows;
        let mut ap = vec![0.0; n];

        // Initialize p = r
        self.p.copy_from_slice(&self.r);

        let mut rsold = self.dot_product(&self.r, &self.r);

        for iter in 0..max_iterations {
            // ap = A * p
            self.matrix.multiply_vector(&self.p, &mut ap);

            // alpha = rsold / (p' * ap)
            let pap = self.dot_product(&self.p, &ap);
            if pap.abs() < 1e-15 {
                break;
            }
            let alpha = rsold / pap;

            // x = x + alpha * p
            for i in 0..n {
                self.x[i] += alpha * self.p[i];
            }

            // r = r - alpha * ap
            for i in 0..n {
                self.r[i] -= alpha * ap[i];
            }

            // Check convergence
            let rsnew = self.dot_product(&self.r, &self.r);
            if rsnew.sqrt() < tolerance {
                self.iterations = iter + 1;
                break;
            }

            // beta = rsnew / rsold
            let beta = rsnew / rsold;

            // p = r + beta * p
            for i in 0..n {
                self.p[i] = self.r[i] + beta * self.p[i];
            }

            rsold = rsnew;
            self.iterations = iter + 1;
        }

        self.x.clone()
    }

    fn dot_product(&self, a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }
}

#[wasm_bindgen]
pub struct WasmSolver {
    tolerance: f64,
    max_iterations: usize,
}

#[wasm_bindgen]
impl WasmSolver {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        WasmSolver {
            tolerance: 1e-6,
            max_iterations: 1000,
        }
    }

    #[wasm_bindgen]
    pub fn set_tolerance(&mut self, tolerance: f64) {
        self.tolerance = tolerance;
    }

    #[wasm_bindgen]
    pub fn set_max_iterations(&mut self, max_iterations: usize) {
        self.max_iterations = max_iterations;
    }

    #[wasm_bindgen]
    pub fn solve_csr(&self, matrix_json: &str, vector_json: &str) -> Result<String, JsValue> {
        // Use a simple timestamp for Node.js compatibility
        let start = 0.0;

        // Parse input
        let matrix_data: MatrixData = serde_json::from_str(matrix_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse matrix: {}", e)))?;

        let vector: Vec<f64> = serde_json::from_str(vector_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse vector: {}", e)))?;

        // Create solver
        let matrix = FastCSRMatrix::new(matrix_data);
        let mut solver = FastConjugateGradient::new(matrix, vector.clone());

        // Solve
        let solution = solver.solve(self.tolerance, self.max_iterations);

        let compute_time_ms = 0.0; // Timing disabled for Node.js

        // Compute residual
        let mut residual_vec = vec![0.0; solver.matrix.rows];
        solver.matrix.multiply_vector(&solution, &mut residual_vec);

        let mut residual = 0.0;
        for i in 0..solver.matrix.rows {
            let diff = residual_vec[i] - vector[i];
            residual += diff * diff;
        }
        residual = residual.sqrt();

        let result = SolverResult {
            solution,
            iterations: solver.iterations,
            residual,
            converged: residual < self.tolerance,
            compute_time_ms,
        };

        serde_json::to_string(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
    }

    #[wasm_bindgen]
    pub fn solve_dense(&self, matrix_json: &str, vector_json: &str) -> Result<String, JsValue> {
        let start = 0.0;

        // Parse dense matrix
        let matrix_data: Vec<Vec<f64>> = serde_json::from_str(matrix_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse matrix: {}", e)))?;

        let vector: Vec<f64> = serde_json::from_str(vector_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse vector: {}", e)))?;

        let n = matrix_data.len();
        if n == 0 {
            return Err(JsValue::from_str("Empty matrix"));
        }

        // Convert dense to CSR
        let mut values = Vec::new();
        let mut col_indices = Vec::new();
        let mut row_ptr = vec![0];

        for row in &matrix_data {
            if row.len() != n {
                return Err(JsValue::from_str("Matrix must be square"));
            }
            for (j, &val) in row.iter().enumerate() {
                if val.abs() > 1e-10 {
                    values.push(val);
                    col_indices.push(j as u32);
                }
            }
            row_ptr.push(values.len() as u32);
        }

        let matrix = FastCSRMatrix {
            values,
            col_indices,
            row_ptr,
            rows: n,
            cols: n,
        };

        let mut solver = FastConjugateGradient::new(matrix, vector.clone());
        let solution = solver.solve(self.tolerance, self.max_iterations);

        let compute_time_ms = 0.0; // Timing disabled for Node.js

        // Compute residual
        let mut residual_vec = vec![0.0; n];
        solver.matrix.multiply_vector(&solution, &mut residual_vec);

        let mut residual = 0.0;
        for i in 0..n {
            let diff = residual_vec[i] - vector[i];
            residual += diff * diff;
        }
        residual = residual.sqrt();

        let result = SolverResult {
            solution,
            iterations: solver.iterations,
            residual,
            converged: residual < self.tolerance,
            compute_time_ms,
        };

        serde_json::to_string(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
    }

    #[wasm_bindgen]
    pub fn solve_neumann(&self, matrix_json: &str, vector_json: &str) -> Result<String, JsValue> {
        let start = 0.0;

        // Parse input
        let matrix_data: MatrixData = serde_json::from_str(matrix_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse matrix: {}", e)))?;

        let vector: Vec<f64> = serde_json::from_str(vector_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse vector: {}", e)))?;

        let matrix = FastCSRMatrix::new(matrix_data);
        let n = matrix.rows;

        // Get diagonal
        let diagonal = matrix.get_diagonal();

        // Initialize solution: x₀ = D⁻¹b
        let mut solution = vec![0.0; n];
        for i in 0..n {
            if diagonal[i].abs() < 1e-15 {
                return Err(JsValue::from_str(&format!("Zero diagonal at position {}", i)));
            }
            solution[i] = vector[i] / diagonal[i];
        }

        let mut temp = vec![0.0; n];
        let mut iterations = 0;

        for iter in 0..self.max_iterations {
            // Compute Ax
            matrix.multiply_vector(&solution, &mut temp);

            // Compute residual: r = b - Ax
            let mut residual = 0.0;
            for i in 0..n {
                let diff = vector[i] - temp[i];
                residual += diff * diff;
                // Update with diagonal preconditioning
                solution[i] += diff / diagonal[i];
            }

            residual = residual.sqrt();
            iterations = iter + 1;

            if residual < self.tolerance {
                break;
            }
        }

        let compute_time_ms = 0.0; // Timing disabled for Node.js

        // Final residual
        matrix.multiply_vector(&solution, &mut temp);
        let mut final_residual = 0.0;
        for i in 0..n {
            let diff = temp[i] - vector[i];
            final_residual += diff * diff;
        }
        final_residual = final_residual.sqrt();

        let result = SolverResult {
            solution,
            iterations,
            residual: final_residual,
            converged: final_residual < self.tolerance,
            compute_time_ms,
        };

        serde_json::to_string(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize result: {}", e)))
    }
}

#[wasm_bindgen]
pub fn version() -> String {
    "0.1.0".to_string()
}

#[wasm_bindgen]
pub fn create_test_matrix(n: usize) -> String {
    // Create a diagonally dominant test matrix
    let mut values = Vec::new();
    let mut col_indices = Vec::new();
    let mut row_ptr = vec![0];

    for i in 0..n {
        // Add off-diagonal elements
        if i > 0 {
            values.push(-1.0);
            col_indices.push((i - 1) as u32);
        }

        // Add diagonal element (make it dominant)
        values.push(4.0);
        col_indices.push(i as u32);

        if i < n - 1 {
            values.push(-1.0);
            col_indices.push((i + 1) as u32);
        }

        row_ptr.push(values.len() as u32);
    }

    let matrix_data = MatrixData {
        values,
        col_indices,
        row_ptr,
        rows: n,
        cols: n,
    };

    serde_json::to_string(&matrix_data).unwrap_or_else(|_| "{}".to_string())
}

#[wasm_bindgen]
pub fn create_test_vector(n: usize) -> String {
    let vector: Vec<f64> = (0..n).map(|i| (i + 1) as f64).collect();
    serde_json::to_string(&vector).unwrap_or_else(|_| "[]".to_string())
}