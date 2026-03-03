// WASM bindings for the fast sublinear-time solver
#![cfg(feature = "wasm")]

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use crate::solver::{SolverOptions, ConvergenceMode};
use crate::fast_solver::{FastCSRMatrix, FastConjugateGradient};
use crate::utils::Precision;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
pub fn init_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
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

#[wasm_bindgen]
pub struct WasmSolver {
    tolerance: f64,
    max_iterations: usize,
}

#[wasm_bindgen]
impl WasmSolver {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        init_panic_hook();
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
        let start = js_sys::Date::now();

        // Parse input
        let matrix_data: MatrixData = serde_json::from_str(matrix_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse matrix: {}", e)))?;

        let vector: Vec<f64> = serde_json::from_str(vector_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse vector: {}", e)))?;

        // Create FastCSRMatrix
        let matrix = FastCSRMatrix {
            values: matrix_data.values,
            col_indices: matrix_data.col_indices,
            row_ptr: matrix_data.row_ptr,
            rows: matrix_data.rows,
            cols: matrix_data.cols,
        };

        // Check matrix dimensions
        if matrix.rows != vector.len() {
            return Err(JsValue::from_str(&format!(
                "Dimension mismatch: matrix has {} rows but vector has {} elements",
                matrix.rows,
                vector.len()
            )));
        }

        // Create solver
        let mut solver = FastConjugateGradient::new(&matrix, &vector);

        // Solve
        let solution = solver.solve_with_tolerance(self.tolerance, self.max_iterations);

        let compute_time_ms = js_sys::Date::now() - start;

        // Compute residual
        let mut residual_vec = vec![0.0; matrix.rows];
        matrix.multiply_vector(&solution, &mut residual_vec);

        let mut residual = 0.0;
        for i in 0..matrix.rows {
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
        let start = js_sys::Date::now();

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

        // Create solver
        let mut solver = FastConjugateGradient::new(&matrix, &vector);

        // Solve
        let solution = solver.solve_with_tolerance(self.tolerance, self.max_iterations);

        let compute_time_ms = js_sys::Date::now() - start;

        // Compute residual
        let mut residual_vec = vec![0.0; matrix.rows];
        matrix.multiply_vector(&solution, &mut residual_vec);

        let mut residual = 0.0;
        for i in 0..matrix.rows {
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
        let start = js_sys::Date::now();

        // Parse input
        let matrix_data: MatrixData = serde_json::from_str(matrix_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse matrix: {}", e)))?;

        let vector: Vec<f64> = serde_json::from_str(vector_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse vector: {}", e)))?;

        // Create FastCSRMatrix
        let matrix = FastCSRMatrix {
            values: matrix_data.values,
            col_indices: matrix_data.col_indices,
            row_ptr: matrix_data.row_ptr,
            rows: matrix_data.rows,
            cols: matrix_data.cols,
        };

        // Use Neumann series solver
        let options = SolverOptions {
            tolerance: self.tolerance,
            max_iterations: self.max_iterations,
            convergence_mode: ConvergenceMode::Residual,
            enable_caching: true,
            cache_size: 100,
            enable_simd: true,
            enable_parallel: false,
            thread_count: 1,
        };

        // Solve using Neumann series
        let mut solution = vector.clone();
        let mut residual = vec![0.0; matrix.rows];
        let mut iterations = 0;

        for iter in 0..self.max_iterations {
            // Compute residual: r = b - Ax
            matrix.multiply_vector(&solution, &mut residual);
            for i in 0..matrix.rows {
                residual[i] = vector[i] - residual[i];
            }

            // Check convergence
            let residual_norm: f64 = residual.iter().map(|x| x * x).sum::<f64>().sqrt();
            if residual_norm < self.tolerance {
                iterations = iter + 1;
                break;
            }

            // Update solution: x = x + r
            for i in 0..matrix.rows {
                solution[i] += residual[i];
            }

            iterations = iter + 1;
        }

        let compute_time_ms = js_sys::Date::now() - start;

        // Final residual computation
        matrix.multiply_vector(&solution, &mut residual);
        let mut final_residual = 0.0;
        for i in 0..matrix.rows {
            let diff = residual[i] - vector[i];
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
    env!("CARGO_PKG_VERSION").to_string()
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