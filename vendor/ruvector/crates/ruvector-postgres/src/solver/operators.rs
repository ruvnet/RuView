//! PostgreSQL operator functions for solver integration.

use pgrx::prelude::*;
use pgrx::JsonB;

use ruvector_solver::forward_push::ForwardPushSolver;
use ruvector_solver::traits::{SolverEngine, SublinearPageRank};
use ruvector_solver::types::{ComputeBudget, CsrMatrix};

use super::{edges_json_to_csr, matrix_json_to_csr};

/// Compute PageRank on an edge list using Forward Push.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_pagerank(
    edges_json: JsonB,
    alpha: default!(f32, 0.85),
    epsilon: default!(f32, 1e-6),
) -> JsonB {
    let csr = match edges_json_to_csr(&edges_json.0) {
        Ok(m) => m,
        Err(e) => {
            pgrx::error!("PageRank: {}", e);
        }
    };

    let n = csr.rows;
    let solver = ForwardPushSolver::new(alpha as f64, epsilon as f64);

    // Compute PPR from each node and accumulate
    let mut scores = vec![0.0f64; n];
    for source in 0..n {
        match solver.ppr(&csr, source, alpha as f64, epsilon as f64) {
            Ok(ppr) => {
                for (node, val) in ppr {
                    if node < n {
                        scores[node] += val;
                    }
                }
            }
            Err(_) => {} // skip failed nodes
        }
    }

    // Normalize
    let total: f64 = scores.iter().sum();
    if total > 0.0 {
        for s in &mut scores {
            *s /= total;
        }
    }

    let result: Vec<serde_json::Value> = scores
        .iter()
        .enumerate()
        .map(|(i, &s)| serde_json::json!({"node": i, "rank": s}))
        .collect();

    JsonB(serde_json::json!(result))
}

/// Compute Personalized PageRank from a single source.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_pagerank_personalized(
    edges_json: JsonB,
    source: i32,
    alpha: default!(f32, 0.85),
    epsilon: default!(f32, 1e-6),
) -> JsonB {
    let csr = match edges_json_to_csr(&edges_json.0) {
        Ok(m) => m,
        Err(e) => pgrx::error!("PPR: {}", e),
    };

    let solver = ForwardPushSolver::new(alpha as f64, epsilon as f64);

    match solver.ppr(&csr, source as usize, alpha as f64, epsilon as f64) {
        Ok(ppr) => {
            let result: Vec<serde_json::Value> = ppr
                .iter()
                .map(|&(node, val)| serde_json::json!({"node": node, "rank": val}))
                .collect();
            JsonB(serde_json::json!(result))
        }
        Err(e) => pgrx::error!("PPR failed: {}", e),
    }
}

/// Compute multi-seed Personalized PageRank.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_pagerank_multi_seed(
    edges_json: JsonB,
    seeds_json: JsonB,
    alpha: default!(f32, 0.85),
    epsilon: default!(f32, 1e-6),
) -> JsonB {
    let csr = match edges_json_to_csr(&edges_json.0) {
        Ok(m) => m,
        Err(e) => pgrx::error!("Multi-seed PPR: {}", e),
    };

    let seeds: Vec<(usize, f64)> = match seeds_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                let a = v.as_array()?;
                Some((a[0].as_u64()? as usize, a[1].as_f64().unwrap_or(1.0)))
            })
            .collect(),
        None => pgrx::error!("Seeds must be array of [node, weight] pairs"),
    };

    let solver = ForwardPushSolver::new(alpha as f64, epsilon as f64);

    match solver.ppr_multi_seed(&csr, &seeds, alpha as f64, epsilon as f64) {
        Ok(ppr) => {
            let result: Vec<serde_json::Value> = ppr
                .iter()
                .map(|&(node, val)| serde_json::json!({"node": node, "rank": val}))
                .collect();
            JsonB(serde_json::json!(result))
        }
        Err(e) => pgrx::error!("Multi-seed PPR failed: {}", e),
    }
}

/// Solve a sparse linear system Ax=b.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_solve_sparse(
    matrix_json: JsonB,
    rhs: Vec<f32>,
    method: default!(&str, "'neumann'"),
) -> JsonB {
    let csr = match matrix_json_to_csr(&matrix_json.0) {
        Ok(m) => m,
        Err(e) => pgrx::error!("Sparse solve: {}", e),
    };

    let rhs_f64: Vec<f64> = rhs.iter().map(|&x| x as f64).collect();
    let budget = ComputeBudget::default();

    // Select solver based on method
    let result = match method.to_lowercase().as_str() {
        "cg" | "conjugate_gradient" => {
            let solver = ruvector_solver::cg::ConjugateGradientSolver::new(1e-6, 1000, true);
            solver.solve(&csr, &rhs_f64, &budget)
        }
        _ => {
            // Default to Neumann — use trait method explicitly for f64 interface
            let solver = ruvector_solver::neumann::NeumannSolver::new(1e-6, 1000);
            SolverEngine::solve(&solver, &csr, &rhs_f64, &budget)
        }
    };

    match result {
        Ok(res) => JsonB(serde_json::json!({
            "solution": res.solution,
            "iterations": res.iterations,
            "residual_norm": res.residual_norm,
            "algorithm": format!("{:?}", res.algorithm),
            "wall_time_ms": res.wall_time.as_millis(),
        })),
        Err(e) => pgrx::error!("Solver failed: {}", e),
    }
}

/// Solve a graph Laplacian system Lx=b.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_solve_laplacian(laplacian_json: JsonB, rhs: Vec<f32>) -> JsonB {
    let csr = match matrix_json_to_csr(&laplacian_json.0) {
        Ok(m) => m,
        Err(e) => pgrx::error!("Laplacian solve: {}", e),
    };

    let rhs_f64: Vec<f64> = rhs.iter().map(|&x| x as f64).collect();
    let budget = ComputeBudget::default();

    let solver = ruvector_solver::cg::ConjugateGradientSolver::new(1e-6, 1000, true);

    match solver.solve(&csr, &rhs_f64, &budget) {
        Ok(res) => JsonB(serde_json::json!({
            "solution": res.solution,
            "iterations": res.iterations,
            "residual_norm": res.residual_norm,
            "algorithm": format!("{:?}", res.algorithm),
        })),
        Err(e) => pgrx::error!("Laplacian solve failed: {}", e),
    }
}

/// Compute effective resistance between two nodes.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_effective_resistance(laplacian_json: JsonB, source: i32, target: i32) -> f32 {
    let csr = match matrix_json_to_csr(&laplacian_json.0) {
        Ok(m) => m,
        Err(e) => pgrx::error!("Effective resistance: {}", e),
    };

    let n = csr.rows;
    let budget = ComputeBudget::default();

    // Solve L * x = e_s - e_t
    let mut rhs = vec![0.0f64; n];
    if (source as usize) < n {
        rhs[source as usize] = 1.0;
    }
    if (target as usize) < n {
        rhs[target as usize] = -1.0;
    }

    let solver = ruvector_solver::cg::ConjugateGradientSolver::new(1e-8, 2000, true);
    match solver.solve(&csr, &rhs, &budget) {
        Ok(res) => {
            let s = source as usize;
            let t = target as usize;
            let x_s = if s < res.solution.len() {
                res.solution[s] as f64
            } else {
                0.0
            };
            let x_t = if t < res.solution.len() {
                res.solution[t] as f64
            } else {
                0.0
            };
            (x_s - x_t) as f32
        }
        Err(e) => pgrx::error!("Effective resistance failed: {}", e),
    }
}

/// Run PageRank on an existing property graph stored via ruvector graph module.
#[cfg(feature = "graph")]
#[pg_extern]
pub fn ruvector_graph_pagerank(
    graph_name: &str,
    alpha: default!(f32, 0.85),
    epsilon: default!(f32, 1e-6),
) -> TableIterator<'static, (name!(node_id, i64), name!(rank, f64))> {
    let graph = match crate::graph::get_graph(graph_name) {
        Some(g) => g,
        None => pgrx::error!("Graph '{}' not found", graph_name),
    };

    // Extract edges and nodes
    let all_nodes = graph.nodes.all_nodes();
    let all_edges = graph.edges.all_edges();

    if all_nodes.is_empty() {
        return TableIterator::new(std::iter::empty());
    }

    // Build node id mapping
    let mut node_ids: Vec<u64> = all_nodes.iter().map(|n| n.id).collect();
    node_ids.sort();
    let node_idx: std::collections::HashMap<u64, usize> = node_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();

    let n = node_ids.len();
    let mut coo = Vec::new();
    for edge in &all_edges {
        if let (Some(&si), Some(&di)) = (node_idx.get(&edge.source), node_idx.get(&edge.target)) {
            coo.push((si, di, 1.0f64));
            coo.push((di, si, 1.0f64));
        }
    }

    let csr = CsrMatrix::<f64>::from_coo(n, n, coo);
    let solver = ForwardPushSolver::new(alpha as f64, epsilon as f64);

    let mut scores = vec![0.0f64; n];
    for source in 0..n {
        if let Ok(ppr) = solver.ppr(&csr, source, alpha as f64, epsilon as f64) {
            for (node, val) in ppr {
                if node < n {
                    scores[node] += val;
                }
            }
        }
    }

    let total: f64 = scores.iter().sum();
    if total > 0.0 {
        for s in &mut scores {
            *s /= total;
        }
    }

    let results: Vec<(i64, f64)> = node_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id as i64, scores[i]))
        .collect();

    TableIterator::new(results.into_iter())
}

/// List available solver algorithms.
#[pg_extern]
pub fn ruvector_solver_info() -> TableIterator<
    'static,
    (
        name!(algorithm, String),
        name!(description, String),
        name!(complexity, String),
    ),
> {
    let algos = vec![
        (
            "neumann",
            "Jacobi-preconditioned Neumann series",
            "O(nnz * log(1/eps))",
        ),
        (
            "cg",
            "Conjugate Gradient for SPD systems",
            "O(n * sqrt(kappa))",
        ),
        (
            "forward-push",
            "Andersen-Chung-Lang PageRank",
            "O(1/epsilon)",
        ),
        (
            "backward-push",
            "Backward Push for target PPR",
            "O(1/epsilon)",
        ),
        (
            "hybrid-random-walk",
            "Push + Monte Carlo sampling",
            "O(sqrt(n/epsilon))",
        ),
        (
            "bmssp",
            "Block MSS preconditioned solver",
            "O(n * nnz_per_row)",
        ),
        (
            "true-solver",
            "Topology-aware batch solver",
            "O(batch * nnz)",
        ),
    ];

    TableIterator::new(
        algos
            .into_iter()
            .map(|(a, d, c)| (a.to_string(), d.to_string(), c.to_string())),
    )
}

/// Analyze matrix sparsity profile.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_matrix_analyze(matrix_json: JsonB) -> JsonB {
    let csr = match matrix_json_to_csr(&matrix_json.0) {
        Ok(m) => m,
        Err(e) => pgrx::error!("Matrix analyze: {}", e),
    };

    let nnz = csr.nnz();
    let density = if csr.rows > 0 && csr.cols > 0 {
        nnz as f64 / (csr.rows as f64 * csr.cols as f64)
    } else {
        0.0
    };

    let mut max_nnz_per_row = 0usize;
    let mut min_nnz_per_row = usize::MAX;
    for i in 0..csr.rows {
        let row_nnz = csr.row_degree(i);
        max_nnz_per_row = max_nnz_per_row.max(row_nnz);
        min_nnz_per_row = min_nnz_per_row.min(row_nnz);
    }
    if csr.rows == 0 {
        min_nnz_per_row = 0;
    }

    let avg_nnz_per_row = if csr.rows > 0 {
        nnz as f64 / csr.rows as f64
    } else {
        0.0
    };

    JsonB(serde_json::json!({
        "rows": csr.rows,
        "cols": csr.cols,
        "nnz": nnz,
        "density": density,
        "avg_nnz_per_row": avg_nnz_per_row,
        "max_nnz_per_row": max_nnz_per_row,
        "min_nnz_per_row": min_nnz_per_row,
    }))
}

/// Solve using Conjugate Gradient directly.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_conjugate_gradient(
    matrix_json: JsonB,
    rhs: Vec<f32>,
    tol: default!(f32, 1e-6),
    max_iter: default!(i32, 1000),
) -> JsonB {
    let csr = match matrix_json_to_csr(&matrix_json.0) {
        Ok(m) => m,
        Err(e) => pgrx::error!("CG solve: {}", e),
    };

    let rhs_f64: Vec<f64> = rhs.iter().map(|&x| x as f64).collect();
    let budget = ComputeBudget {
        tolerance: tol as f64,
        max_iterations: max_iter as usize,
        ..Default::default()
    };

    let solver =
        ruvector_solver::cg::ConjugateGradientSolver::new(tol as f64, max_iter as usize, true);

    match solver.solve(&csr, &rhs_f64, &budget) {
        Ok(res) => JsonB(serde_json::json!({
            "solution": res.solution,
            "iterations": res.iterations,
            "residual_norm": res.residual_norm,
            "converged": res.residual_norm < tol as f64,
            "wall_time_ms": res.wall_time.as_millis(),
        })),
        Err(e) => pgrx::error!("CG solve failed: {}", e),
    }
}

/// Compute node centrality using solver-based methods.
#[cfg(feature = "graph")]
#[pg_extern]
pub fn ruvector_graph_centrality(
    graph_name: &str,
    method: default!(&str, "'pagerank'"),
) -> TableIterator<'static, (name!(node_id, i64), name!(centrality, f64))> {
    let graph = match crate::graph::get_graph(graph_name) {
        Some(g) => g,
        None => pgrx::error!("Graph '{}' not found", graph_name),
    };

    let all_nodes = graph.nodes.all_nodes();
    let all_edges = graph.edges.all_edges();

    if all_nodes.is_empty() {
        return TableIterator::new(std::iter::empty());
    }

    let mut node_ids: Vec<u64> = all_nodes.iter().map(|n| n.id).collect();
    node_ids.sort();
    let node_idx: std::collections::HashMap<u64, usize> = node_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();

    let n = node_ids.len();
    let mut coo = Vec::new();
    for edge in &all_edges {
        if let (Some(&si), Some(&di)) = (node_idx.get(&edge.source), node_idx.get(&edge.target)) {
            coo.push((si, di, 1.0f64));
            coo.push((di, si, 1.0f64));
        }
    }

    let csr = CsrMatrix::<f64>::from_coo(n, n, coo);

    let scores = match method.to_lowercase().as_str() {
        "degree" => {
            // Degree centrality
            (0..n).map(|i| csr.row_degree(i) as f64).collect::<Vec<_>>()
        }
        _ => {
            // Default: PageRank centrality
            let solver = ForwardPushSolver::new(0.85, 1e-6);
            let mut scores = vec![0.0f64; n];
            for source in 0..n {
                if let Ok(ppr) = solver.ppr(&csr, source, 0.85, 1e-6) {
                    for (node, val) in ppr {
                        if node < n {
                            scores[node] += val;
                        }
                    }
                }
            }
            let total: f64 = scores.iter().sum();
            if total > 0.0 {
                for s in &mut scores {
                    *s /= total;
                }
            }
            scores
        }
    };

    let results: Vec<(i64, f64)> = node_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id as i64, scores[i]))
        .collect();

    TableIterator::new(results.into_iter())
}
