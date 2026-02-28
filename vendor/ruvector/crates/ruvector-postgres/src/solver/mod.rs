//! Solver integration module — exposes ruvector-solver as SQL functions.

pub mod operators;

use ruvector_solver::types::CsrMatrix;

/// Convert a JSON edge list `[[src, dst], ...]` or `[[src, dst, weight], ...]`
/// into a CsrMatrix<f64> adjacency matrix.
pub fn edges_json_to_csr(json: &serde_json::Value) -> Result<CsrMatrix<f64>, String> {
    let edges = json
        .get("edges")
        .and_then(|e| e.as_array())
        .or_else(|| json.as_array())
        .ok_or_else(|| {
            "Expected JSON object with 'edges' array or a JSON array of edges".to_string()
        })?;

    if edges.is_empty() {
        return Err("Edge list is empty".to_string());
    }

    // Collect edges and determine node count
    let mut coo: Vec<(usize, usize, f64)> = Vec::with_capacity(edges.len() * 2);
    let mut max_node: usize = 0;

    for edge in edges {
        let arr = edge
            .as_array()
            .ok_or_else(|| "Each edge must be an array".to_string())?;
        if arr.len() < 2 {
            return Err("Each edge must have at least [src, dst]".to_string());
        }
        let src = arr[0].as_u64().ok_or("Edge source must be integer")? as usize;
        let dst = arr[1].as_u64().ok_or("Edge target must be integer")? as usize;
        let weight = arr.get(2).and_then(|w| w.as_f64()).unwrap_or(1.0);

        max_node = max_node.max(src).max(dst);
        coo.push((src, dst, weight));
        coo.push((dst, src, weight)); // undirected
    }

    let n = max_node + 1;
    Ok(CsrMatrix::<f64>::from_coo(n, n, coo))
}

/// Convert a JSON sparse matrix representation to CsrMatrix<f64>.
/// Accepts format: `{"rows": N, "cols": M, "entries": [[r, c, val], ...]}`
/// or a flat array `[[r, c, val], ...]` (square matrix inferred).
pub fn matrix_json_to_csr(json: &serde_json::Value) -> Result<CsrMatrix<f64>, String> {
    // Structured format with rows/cols
    if let Some(entries) = json.get("entries").and_then(|e| e.as_array()) {
        let rows = json
            .get("rows")
            .and_then(|r| r.as_u64())
            .ok_or("Missing 'rows'")? as usize;
        let cols = json
            .get("cols")
            .and_then(|c| c.as_u64())
            .ok_or("Missing 'cols'")? as usize;

        let coo: Vec<(usize, usize, f64)> = entries
            .iter()
            .filter_map(|e| {
                let a = e.as_array()?;
                Some((
                    a[0].as_u64()? as usize,
                    a[1].as_u64()? as usize,
                    a[2].as_f64()?,
                ))
            })
            .collect();

        return Ok(CsrMatrix::<f64>::from_coo(rows, cols, coo));
    }

    // Flat array format
    if let Some(entries) = json.as_array() {
        let mut max_r = 0usize;
        let mut max_c = 0usize;
        let coo: Vec<(usize, usize, f64)> = entries
            .iter()
            .filter_map(|e| {
                let a = e.as_array()?;
                let r = a[0].as_u64()? as usize;
                let c = a[1].as_u64()? as usize;
                let v = a[2].as_f64()?;
                Some((r, c, v))
            })
            .inspect(|(r, c, _)| {
                max_r = max_r.max(*r);
                max_c = max_c.max(*c);
            })
            .collect();

        let n = max_r.max(max_c) + 1;
        return Ok(CsrMatrix::<f64>::from_coo(n, n, coo));
    }

    Err("Invalid matrix JSON format".to_string())
}
