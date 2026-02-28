//! Utility helpers for the WASM solver bindings.
//!
//! Provides panic hook installation for better error messages in the browser
//! console, and conversion routines that bridge JavaScript typed arrays to
//! the solver's internal [`CsrMatrix`] representation.

use ruvector_solver::types::CsrMatrix;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Console logging
// ---------------------------------------------------------------------------

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

/// Log a message to the browser console.
pub fn console_log(msg: &str) {
    log(msg);
}

/// Log an error to the browser console.
#[allow(dead_code)]
pub fn console_error(msg: &str) {
    error(msg);
}

// ---------------------------------------------------------------------------
// Panic hook
// ---------------------------------------------------------------------------

/// Install a custom panic hook that forwards Rust panics to `console.error`.
///
/// Call this once at module initialisation (via `#[wasm_bindgen(start)]`).
/// Subsequent calls are no-ops.
pub fn set_panic_hook() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };

            let location = info
                .location()
                .map(|loc| format!(" at {}:{}:{}", loc.file(), loc.line(), loc.column()))
                .unwrap_or_default();

            error(&format!(
                "[ruvector-solver-wasm] panic{}: {}",
                location, msg
            ));
        }));
    });
}

// ---------------------------------------------------------------------------
// CsrMatrix construction from JS arrays
// ---------------------------------------------------------------------------

/// Build a [`CsrMatrix<f32>`] from flat JS-compatible arrays.
///
/// # Arguments
///
/// * `values`      - Non-zero values (Float32Array).
/// * `col_indices` - Column index for each non-zero (Uint32Array).
/// * `row_ptrs`    - Row pointer array of length `rows + 1` (Uint32Array).
/// * `rows`        - Number of rows.
/// * `cols`        - Number of columns.
///
/// # Errors
///
/// Returns a human-readable error string when the inputs are structurally
/// invalid (mismatched lengths, out-of-bounds indices, non-finite values).
pub fn csr_from_js_arrays(
    values: &[f32],
    col_indices: &[u32],
    row_ptrs: &[u32],
    rows: usize,
    cols: usize,
) -> Result<CsrMatrix<f32>, String> {
    // row_ptrs length check.
    if row_ptrs.len() != rows + 1 {
        return Err(format!(
            "row_ptrs length {} does not equal rows + 1 = {}",
            row_ptrs.len(),
            rows + 1,
        ));
    }

    // Monotonicity check.
    for i in 1..row_ptrs.len() {
        if row_ptrs[i] < row_ptrs[i - 1] {
            return Err(format!(
                "row_ptrs is not monotonically non-decreasing at position {}",
                i,
            ));
        }
    }

    let expected_nnz = row_ptrs[rows] as usize;

    if values.len() != expected_nnz {
        return Err(format!(
            "values length {} does not match row_ptrs[rows] = {}",
            values.len(),
            expected_nnz,
        ));
    }
    if col_indices.len() != expected_nnz {
        return Err(format!(
            "col_indices length {} does not match row_ptrs[rows] = {}",
            col_indices.len(),
            expected_nnz,
        ));
    }

    // Column bounds and value finiteness.
    for row in 0..rows {
        let start = row_ptrs[row] as usize;
        let end = row_ptrs[row + 1] as usize;
        for idx in start..end {
            if col_indices[idx] as usize >= cols {
                return Err(format!(
                    "column index {} out of bounds for {} columns (row {})",
                    col_indices[idx], cols, row,
                ));
            }
            if !values[idx].is_finite() {
                return Err(format!(
                    "non-finite value at matrix[{}, {}] = {}",
                    row, col_indices[idx], values[idx],
                ));
            }
        }
    }

    // Convert to the solver's internal representation.
    // `CsrMatrix<f32>` from `types` uses `row_ptr: Vec<usize>` and
    // `col_indices: Vec<usize>`.
    let row_ptr: Vec<usize> = row_ptrs.iter().map(|&r| r as usize).collect();
    let col_idx: Vec<usize> = col_indices.iter().map(|&c| c as usize).collect();

    Ok(CsrMatrix {
        row_ptr,
        col_indices: col_idx,
        values: values.to_vec(),
        rows,
        cols,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csr_from_js_arrays_valid() {
        // 2x3 matrix: [[1, 0, 2], [0, 3, 0]]
        let values = [1.0f32, 2.0, 3.0];
        let col_indices = [0u32, 2, 1];
        let row_ptrs = [0u32, 2, 3];

        let csr = csr_from_js_arrays(&values, &col_indices, &row_ptrs, 2, 3).unwrap();
        assert_eq!(csr.rows, 2);
        assert_eq!(csr.cols, 3);
        assert_eq!(csr.values.len(), 3);
    }

    #[test]
    fn test_csr_from_js_arrays_row_ptrs_length() {
        let err = csr_from_js_arrays(&[], &[], &[0], 2, 2).unwrap_err();
        assert!(err.contains("row_ptrs length"));
    }

    #[test]
    fn test_csr_from_js_arrays_non_monotonic() {
        let err = csr_from_js_arrays(&[1.0], &[0], &[0, 1, 0], 2, 2).unwrap_err();
        assert!(err.contains("not monotonically"));
    }

    #[test]
    fn test_csr_from_js_arrays_col_out_of_bounds() {
        let err = csr_from_js_arrays(&[1.0], &[5], &[0, 1], 1, 3).unwrap_err();
        assert!(err.contains("out of bounds"));
    }

    #[test]
    fn test_csr_from_js_arrays_nan_rejected() {
        let err = csr_from_js_arrays(&[f32::NAN], &[0], &[0, 1], 1, 2).unwrap_err();
        assert!(err.contains("non-finite"));
    }
}
