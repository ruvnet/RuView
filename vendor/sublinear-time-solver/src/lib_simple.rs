//! Simplified lib.rs for BMSSP and ultra-fast solver integration

pub mod ultra_fast;
pub mod bmssp;

// Re-export main types
pub use ultra_fast::{UltraFastCSR, UltraFastCG, generate_test_matrix, benchmark_rust_performance};
pub use bmssp::{BMSSPSolver, BMSSPConfig};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// WASM entry point for solving linear systems
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn solve_linear_system(
    matrix_data: &[f64],
    rows: usize,
    cols: usize,
    b: &[f64],
    use_bmssp: bool
) -> Vec<f64> {
    // Convert dense matrix to CSR format
    let mut triplets = Vec::new();
    for i in 0..rows {
        for j in 0..cols {
            let val = matrix_data[i * cols + j];
            if val.abs() > 1e-10 {
                triplets.push((i, j, val));
            }
        }
    }

    let matrix = UltraFastCSR::from_triplets(triplets, rows, cols);

    if use_bmssp {
        let config = BMSSPConfig::default();
        let mut solver = BMSSPSolver::new(config);
        solver.solve(&matrix, b)
    } else {
        let solver = UltraFastCG::new(1000, 1e-10);
        solver.solve(&matrix, b)
    }
}

/// Initialize WASM module
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn init() {
    // Set panic hook for better error messages
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}