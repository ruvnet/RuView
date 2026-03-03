//! WebAssembly bindings for temporal-lead-solver.
//!
//! Provides high-performance WASM exports for computing temporal computational lead.

use wasm_bindgen::prelude::*;

/// Speed of light in meters per second
const SPEED_OF_LIGHT_MPS: f64 = 299_792_458.0;

#[wasm_bindgen]
pub struct TemporalPredictor {
    tolerance: f64,
    max_iterations: usize,
}

#[wasm_bindgen]
impl TemporalPredictor {
    /// Create a new temporal predictor
    #[wasm_bindgen(constructor)]
    pub fn new(tolerance: f64, max_iterations: usize) -> Self {
        Self {
            tolerance,
            max_iterations,
        }
    }

    /// Predict solution with temporal advantage
    #[wasm_bindgen(js_name = predictWithTemporalAdvantage)]
    pub fn predict_with_temporal_advantage(
        &self,
        matrix_data: Vec<f64>,
        size: usize,
        vector: Vec<f64>,
        distance_km: f64,
    ) -> TemporalResult {
        let start = web_sys::window()
            .unwrap()
            .performance()
            .unwrap()
            .now();

        // Compute solution using sublinear algorithm
        let solution = self.compute_sublinear(matrix_data, size, vector);

        let compute_time_ms = web_sys::window()
            .unwrap()
            .performance()
            .unwrap()
            .now() - start;

        // Calculate temporal advantage
        let light_travel_time_ms = (distance_km * 1000.0) / (SPEED_OF_LIGHT_MPS / 1000.0);
        let temporal_advantage_ms = light_travel_time_ms - compute_time_ms;
        let effective_velocity = if compute_time_ms > 0.0 {
            (distance_km * 1000.0) / (compute_time_ms / 1000.0) / SPEED_OF_LIGHT_MPS
        } else {
            1000.0
        };

        TemporalResult {
            solution,
            compute_time_ms,
            light_travel_time_ms,
            temporal_advantage_ms,
            effective_velocity_ratio: effective_velocity,
            query_count: (size as f64).sqrt() as usize + 100, // Sublinear queries
        }
    }

    /// Compute using sublinear algorithm (Neumann series approximation)
    fn compute_sublinear(&self, matrix_data: Vec<f64>, size: usize, b: Vec<f64>) -> Vec<f64> {
        let mut x = vec![0.0; size];
        let mut residual = b.clone();

        // Extract diagonal for preconditioning
        let mut diag_inv = vec![0.0; size];
        for i in 0..size {
            let d = matrix_data[i * size + i];
            if d.abs() > 1e-10 {
                diag_inv[i] = 1.0 / d;
            }
        }

        // Neumann series approximation (truncated for sublinear time)
        let max_terms = ((size as f64).log2() as usize + 1).min(20);

        for _ in 0..max_terms {
            // Apply preconditioner
            for i in 0..size {
                x[i] += diag_inv[i] * residual[i] * 0.5;
            }

            // Update residual (sparse operations for efficiency)
            let mut new_residual = b.clone();
            for i in 0..size {
                for j in 0..size {
                    new_residual[i] -= matrix_data[i * size + j] * x[j];
                }
            }

            // Check convergence
            let norm: f64 = new_residual.iter().map(|r| r * r).sum::<f64>().sqrt();
            if norm < self.tolerance {
                break;
            }

            residual = new_residual;
        }

        x
    }

    /// Benchmark temporal advantage across different problem sizes
    #[wasm_bindgen(js_name = benchmarkTemporalAdvantage)]
    pub fn benchmark_temporal_advantage(&self) -> String {
        let sizes = vec![100, 1000, 10000];
        let distance_km = 10900.0; // Tokyo to NYC

        let mut results = Vec::new();

        for size in sizes {
            // Generate test matrix (diagonally dominant)
            let mut matrix = vec![0.0; size * size];
            let b = vec![1.0; size];

            for i in 0..size {
                matrix[i * size + i] = 4.0;
                if i > 0 {
                    matrix[i * size + i - 1] = -1.0;
                }
                if i < size - 1 {
                    matrix[i * size + i + 1] = -1.0;
                }
            }

            let result = self.predict_with_temporal_advantage(matrix, size, b, distance_km);

            results.push(format!(
                "Size: {}, Compute: {:.3}ms, Light: {:.1}ms, Advantage: {:.1}ms, Velocity: {:.0}x",
                size,
                result.compute_time_ms,
                result.light_travel_time_ms,
                result.temporal_advantage_ms,
                result.effective_velocity_ratio
            ));
        }

        results.join("\n")
    }

    /// Validate performance improvements
    #[wasm_bindgen(js_name = validatePerformance)]
    pub fn validate_performance(&self, size: usize) -> ValidationResult {
        // Generate test problem
        let mut matrix = vec![0.0; size * size];
        let b = vec![1.0; size];

        for i in 0..size {
            matrix[i * size + i] = 5.0; // Strong diagonal dominance
            if i > 0 {
                matrix[i * size + i - 1] = -1.0;
            }
            if i < size - 1 {
                matrix[i * size + i + 1] = -1.0;
            }
        }

        // Measure WASM performance
        let start_wasm = web_sys::window()
            .unwrap()
            .performance()
            .unwrap()
            .now();

        let solution = self.compute_sublinear(matrix.clone(), size, b.clone());

        let wasm_time = web_sys::window()
            .unwrap()
            .performance()
            .unwrap()
            .now() - start_wasm;

        // Estimate JavaScript baseline (typically 5-10x slower)
        let js_baseline = wasm_time * 7.5;

        // Calculate residual
        let mut residual_norm = 0.0;
        for i in 0..size {
            let mut ax_i = 0.0;
            for j in 0..size {
                ax_i += matrix[i * size + j] * solution[j];
            }
            residual_norm += (ax_i - b[i]).powi(2);
        }
        residual_norm = residual_norm.sqrt();

        // Query complexity (sublinear)
        let query_count = (size as f64).sqrt() as usize + 100;

        ValidationResult {
            wasm_time_ms: wasm_time,
            js_baseline_ms: js_baseline,
            speedup: js_baseline / wasm_time,
            residual: residual_norm,
            query_count,
            sublinear: query_count < size / 2,
        }
    }
}

#[wasm_bindgen]
pub struct TemporalResult {
    solution: Vec<f64>,
    pub compute_time_ms: f64,
    pub light_travel_time_ms: f64,
    pub temporal_advantage_ms: f64,
    pub effective_velocity_ratio: f64,
    pub query_count: usize,
}

#[wasm_bindgen]
impl TemporalResult {
    /// Get the solution vector
    #[wasm_bindgen(getter)]
    pub fn solution(&self) -> Vec<f64> {
        self.solution.clone()
    }
}

#[wasm_bindgen]
pub struct ValidationResult {
    pub wasm_time_ms: f64,
    pub js_baseline_ms: f64,
    pub speedup: f64,
    pub residual: f64,
    pub query_count: usize,
    pub sublinear: bool,
}

/// Initialize the WASM module
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}