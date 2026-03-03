use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use crate::solver_core::{ConjugateGradientSolver, SolverConfig};
use crate::optimized_solver::OptimizedConjugateGradientSolver;
use crate::math_wasm::{Matrix, Vector};
use std::collections::HashMap;

// Configuration structure for the solver
#[derive(Serialize, Deserialize, Clone)]
pub struct WasmSolverConfig {
    pub max_iterations: usize,
    pub tolerance: f64,
    pub simd_enabled: bool,
    pub stream_chunk_size: usize,
}

impl Default for WasmSolverConfig {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-10,
            simd_enabled: cfg!(target_feature = "simd128"),
            stream_chunk_size: 100,
        }
    }
}

// Solution step for streaming interface
#[derive(Serialize, Deserialize, Clone)]
pub struct SolutionStep {
    pub iteration: usize,
    pub residual: f64,
    pub timestamp: f64,
    pub convergence: bool,
}

// Memory usage information
#[derive(Serialize, Deserialize)]
pub struct MemoryUsage {
    pub used: usize,
    pub capacity: usize,
}

// Main WASM solver interface
#[wasm_bindgen]
pub struct WasmSublinearSolver {
    config: WasmSolverConfig,
    solver: OptimizedConjugateGradientSolver,
    callbacks: HashMap<String, js_sys::Function>,
    memory_usage: usize,
}

#[wasm_bindgen]
impl WasmSublinearSolver {
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<WasmSublinearSolver, JsValue> {
        crate::set_panic_hook();

        let config: WasmSolverConfig = if config.is_undefined() {
            WasmSolverConfig::default()
        } else {
            serde_wasm_bindgen::from_value(config)
                .map_err(|e| JsValue::from_str(&format!("Invalid config: {}", e)))?
        };

        let solver_config = SolverConfig {
            max_iterations: config.max_iterations,
            tolerance: config.tolerance,
        };

        #[cfg(feature = "std")]
        let solver = if config.simd_enabled {
            OptimizedConjugateGradientSolver::new_parallel(solver_config)
        } else {
            OptimizedConjugateGradientSolver::new(solver_config)
        };
        #[cfg(not(feature = "std"))]
        let solver = OptimizedConjugateGradientSolver::new(solver_config);

        Ok(WasmSublinearSolver {
            config,
            solver,
            callbacks: HashMap::new(),
            memory_usage: 0,
        })
    }

    #[wasm_bindgen]
    pub fn solve(
        &mut self,
        matrix_data: &[f64],
        matrix_rows: usize,
        matrix_cols: usize,
        vector_data: &[f64],
    ) -> Result<Vec<f64>, JsValue> {
        // Validate input dimensions
        if matrix_data.len() != matrix_rows * matrix_cols {
            return Err(JsValue::from_str("Matrix dimensions mismatch"));
        }

        if vector_data.len() != matrix_rows {
            return Err(JsValue::from_str("Vector size mismatch"));
        }

        // Create matrix and vector views
        let matrix = Matrix::from_slice(matrix_data, matrix_rows, matrix_cols);
        let vector = Vector::from_slice(vector_data);

        // Update memory usage tracking
        self.memory_usage = matrix_data.len() * 8 + vector_data.len() * 8;

        // Solve system
        match self.solver.solve(&matrix, &vector) {
            Ok(solution) => Ok(solution.data().to_vec()),
            Err(e) => Err(JsValue::from_str(&format!("Solver error: {}", e))),
        }
    }

    #[wasm_bindgen]
    pub fn solve_stream(
        &mut self,
        matrix_data: &[f64],
        matrix_rows: usize,
        matrix_cols: usize,
        vector_data: &[f64],
        progress_callback: &js_sys::Function,
    ) -> Result<Vec<f64>, JsValue> {
        // Validate input
        if matrix_data.len() != matrix_rows * matrix_cols {
            return Err(JsValue::from_str("Matrix dimensions mismatch"));
        }

        if vector_data.len() != matrix_rows {
            return Err(JsValue::from_str("Vector size mismatch"));
        }

        let matrix = Matrix::from_slice(matrix_data, matrix_rows, matrix_cols);
        let vector = Vector::from_slice(vector_data);

        self.memory_usage = matrix_data.len() * 8 + vector_data.len() * 8;

        // Solve with streaming callback
        let mut solution = None;
        let chunk_size = self.config.stream_chunk_size;

        let result = self.solver.solve_with_callback(&matrix, &vector, chunk_size, |step_data| {
            let timestamp = js_sys::Date::now();
            let step = SolutionStep {
                iteration: step_data.iteration,
                residual: step_data.residual,
                timestamp,
                convergence: step_data.converged,
            };

            let step_js = serde_wasm_bindgen::to_value(&step).unwrap();
            let _ = progress_callback.call1(&JsValue::NULL, &step_js);

            if step_data.converged {
                solution = Some(step_data.solution.clone());
            }
        });

        match result {
            Ok(final_solution) => Ok(final_solution.data().to_vec()),
            Err(e) => Err(JsValue::from_str(&format!("Streaming solver error: {}", e))),
        }
    }

    #[wasm_bindgen]
    pub fn solve_batch(
        &mut self,
        batch_data: JsValue,
    ) -> Result<JsValue, JsValue> {
        #[derive(Deserialize)]
        struct BatchRequest {
            id: String,
            matrix_data: Vec<f64>,
            matrix_rows: usize,
            matrix_cols: usize,
            vector_data: Vec<f64>,
        }

        #[derive(Serialize)]
        struct BatchResult {
            id: String,
            solution: Vec<f64>,
            iterations: usize,
            error: Option<String>,
        }

        let requests: Vec<BatchRequest> = serde_wasm_bindgen::from_value(batch_data)
            .map_err(|e| JsValue::from_str(&format!("Invalid batch data: {}", e)))?;

        let mut results = Vec::new();

        for request in requests {
            let result = match self.solve(
                &request.matrix_data,
                request.matrix_rows,
                request.matrix_cols,
                &request.vector_data,
            ) {
                Ok(solution) => BatchResult {
                    id: request.id,
                    solution,
                    iterations: self.solver.get_last_iteration_count(),
                    error: None,
                },
                Err(e) => BatchResult {
                    id: request.id,
                    solution: Vec::new(),
                    iterations: 0,
                    error: Some(format!("{:?}", e)),
                },
            };
            results.push(result);
        }

        serde_wasm_bindgen::to_value(&results)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize results: {}", e)))
    }

    #[wasm_bindgen(getter)]
    pub fn memory_usage(&self) -> JsValue {
        let usage = MemoryUsage {
            used: self.memory_usage,
            capacity: self.memory_usage * 2, // Rough estimate
        };

        serde_wasm_bindgen::to_value(&usage).unwrap()
    }

    #[wasm_bindgen]
    pub fn get_config(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.config).unwrap()
    }

    #[wasm_bindgen]
    pub fn dispose(&mut self) {
        self.callbacks.clear();
        self.memory_usage = 0;
    }
}

// Zero-copy matrix view for efficient data transfer
#[wasm_bindgen]
pub struct MatrixView {
    data: Vec<f64>,
    rows: usize,
    cols: usize,
}

#[wasm_bindgen]
impl MatrixView {
    #[wasm_bindgen(constructor)]
    pub fn new(rows: usize, cols: usize) -> MatrixView {
        MatrixView {
            data: vec![0.0; rows * cols],
            rows,
            cols,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn data(&self) -> *const f64 {
        self.data.as_ptr()
    }

    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.data.len()
    }

    #[wasm_bindgen(getter)]
    pub fn rows(&self) -> usize {
        self.rows
    }

    #[wasm_bindgen(getter)]
    pub fn cols(&self) -> usize {
        self.cols
    }

    // Zero-copy access to data
    #[wasm_bindgen]
    pub fn data_view(&self) -> js_sys::Float64Array {
        unsafe { js_sys::Float64Array::view(&self.data) }
    }

    // Set data without copying
    #[wasm_bindgen]
    pub fn set_data(&mut self, data: &[f64]) -> Result<(), JsValue> {
        if data.len() != self.data.len() {
            return Err(JsValue::from_str("Data length mismatch"));
        }
        self.data.copy_from_slice(data);
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_element(&self, row: usize, col: usize) -> Result<f64, JsValue> {
        if row >= self.rows || col >= self.cols {
            return Err(JsValue::from_str("Index out of bounds"));
        }
        Ok(self.data[row * self.cols + col])
    }

    #[wasm_bindgen]
    pub fn set_element(&mut self, row: usize, col: usize, value: f64) -> Result<(), JsValue> {
        if row >= self.rows || col >= self.cols {
            return Err(JsValue::from_str("Index out of bounds"));
        }
        self.data[row * self.cols + col] = value;
        Ok(())
    }
}

// Memory management utilities
#[wasm_bindgen]
pub fn allocate_matrix(rows: usize, cols: usize) -> *mut f64 {
    let size = rows * cols;
    let layout = std::alloc::Layout::array::<f64>(size).unwrap();
    unsafe { std::alloc::alloc(layout) as *mut f64 }
}

#[wasm_bindgen]
pub fn deallocate_matrix(ptr: *mut f64, rows: usize, cols: usize) {
    let size = rows * cols;
    let layout = std::alloc::Layout::array::<f64>(size).unwrap();
    unsafe { std::alloc::dealloc(ptr as *mut u8, layout) }
}

// Utility functions for performance benchmarking
#[wasm_bindgen]
pub fn benchmark_matrix_multiply(size: usize) -> f64 {
    let start = js_sys::Date::now();

    let matrix_a = Matrix::identity(size);
    let matrix_b = Matrix::identity(size);
    let _result = matrix_a.multiply(&matrix_b);

    js_sys::Date::now() - start
}

#[wasm_bindgen]
pub fn get_wasm_memory_usage() -> usize {
    // Return current memory usage in bytes
    #[cfg(target_arch = "wasm32")]
    {
        use core::arch::wasm32;
        unsafe {
            wasm32::memory_size(0) * 65536 // Pages to bytes
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

/// Get available features in this WASM build
#[wasm_bindgen]
pub fn get_features() -> JsValue {
    let mut features = Vec::new();

    #[cfg(feature = "simd")]
    features.push("simd");

    #[cfg(feature = "wasm")]
    features.push("wasm");

    #[cfg(feature = "std")]
    features.push("std");

    serde_wasm_bindgen::to_value(&features).unwrap()
}

/// Check if SIMD is enabled and supported
#[wasm_bindgen]
pub fn enable_simd() -> bool {
    #[cfg(feature = "simd")]
    {
        #[cfg(target_arch = "wasm32")]
        {
            // Check for WASM SIMD support
            cfg!(target_feature = "simd128")
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            crate::has_simd_support()
        }
    }
    #[cfg(not(feature = "simd"))]
    {
        false
    }
}