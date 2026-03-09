//! WASM-compatible lightweight graph algorithms.
//!
//! These implementations avoid heavy dependencies (ndarray-linalg, petgraph) and work
//! within the constraints of the wasm32-unknown-unknown target. All algorithms operate
//! on the `BrainGraph` type from `ruv-neural-core`.

use ruv_neural_core::embedding::{EmbeddingMetadata, NeuralEmbedding};
use ruv_neural_core::graph::BrainGraph;
use ruv_neural_core::topology::{CognitiveState, MincutResult, TopologyMetrics};

/// Error type for WASM graph operations.
#[derive(Debug)]
pub struct WasmGraphError(pub String);

impl std::fmt::Display for WasmGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WasmGraphError: {}", self.0)
    }
}

impl std::error::Error for WasmGraphError {}

/// Simplified Stoer-Wagner minimum cut for small graphs (<500 nodes).
///
/// This is a direct implementation of the Stoer-Wagner algorithm that finds
/// the global minimum cut in an undirected weighted graph. The algorithm runs
/// in O(V^3) time which is acceptable for brain graphs up to ~500 nodes.
pub fn wasm_mincut(graph: &BrainGraph) -> Result<MincutResult, WasmGraphError> {
    let n = graph.num_nodes;
    if n == 0 {
        return Err(WasmGraphError("Graph has no nodes".into()));
    }
    if n > 500 {
        return Err(WasmGraphError(format!(
            "Graph too large for WASM mincut: {} nodes (max 500)",
            n
        )));
    }
    if n == 1 {
        return Ok(MincutResult {
            cut_value: 0.0,
            partition_a: vec![0],
            partition_b: vec![],
            cut_edges: vec![],
            timestamp: graph.timestamp,
        });
    }

    let mut adj = graph.adjacency_matrix();

    // Track which original nodes are merged into each super-node.
    let mut merged: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    // Track which super-nodes are still active.
    let mut active: Vec<bool> = vec![true; n];

    let mut best_cut = f64::INFINITY;
    let mut best_partition_a: Vec<usize> = Vec::new();

    // Stoer-Wagner: perform n-1 minimum cut phases.
    for _ in 0..n - 1 {
        let active_nodes: Vec<usize> = (0..n).filter(|&i| active[i]).collect();
        if active_nodes.len() < 2 {
            break;
        }

        // Maximum adjacency ordering.
        let mut in_set = vec![false; n];
        let mut w = vec![0.0f64; n]; // key values
        let mut order: Vec<usize> = Vec::with_capacity(active_nodes.len());

        for _ in 0..active_nodes.len() {
            // Find the active node not in set with maximum key.
            let next = active_nodes
                .iter()
                .filter(|&&v| !in_set[v])
                .max_by(|&&a, &&b| w[a].partial_cmp(&w[b]).unwrap_or(std::cmp::Ordering::Equal))
                .copied()
                .unwrap();

            in_set[next] = true;
            order.push(next);

            // Update keys for neighbours.
            for &v in &active_nodes {
                if !in_set[v] {
                    w[v] += adj[next][v];
                }
            }
        }

        // The last two nodes in the ordering.
        let t = *order.last().unwrap();
        let s = order[order.len() - 2];

        // Cut of the phase = key of the last added node.
        let cut_of_phase = w[t];

        if cut_of_phase < best_cut {
            best_cut = cut_of_phase;
            best_partition_a = merged[t].clone();
        }

        // Merge t into s.
        let t_nodes = merged[t].clone();
        merged[s].extend(t_nodes);
        active[t] = false;

        // Update adjacency: merge t into s.
        for i in 0..n {
            adj[s][i] += adj[t][i];
            adj[i][s] += adj[i][t];
        }
        adj[s][s] = 0.0;
    }

    // Build partition B from nodes not in partition A.
    let partition_a_set: std::collections::HashSet<usize> =
        best_partition_a.iter().copied().collect();
    let partition_b: Vec<usize> = (0..n).filter(|i| !partition_a_set.contains(i)).collect();

    // Find cut edges.
    let cut_edges: Vec<(usize, usize, f64)> = graph
        .edges
        .iter()
        .filter(|e| {
            (partition_a_set.contains(&e.source) && !partition_a_set.contains(&e.target))
                || (!partition_a_set.contains(&e.source) && partition_a_set.contains(&e.target))
        })
        .map(|e| (e.source, e.target, e.weight))
        .collect();

    Ok(MincutResult {
        cut_value: best_cut,
        partition_a: best_partition_a,
        partition_b,
        cut_edges,
        timestamp: graph.timestamp,
    })
}

/// Compute basic topology metrics without heavy linear algebra dependencies.
///
/// Computes density, degree statistics, clustering coefficient, and graph entropy.
/// Fiedler value and global efficiency use simplified approximations suitable for WASM.
pub fn wasm_topology_metrics(graph: &BrainGraph) -> Result<TopologyMetrics, WasmGraphError> {
    let n = graph.num_nodes;
    if n == 0 {
        return Err(WasmGraphError("Graph has no nodes".into()));
    }

    let adj = graph.adjacency_matrix();

    // Density.
    let _density = graph.density();

    // Degree statistics.
    let degrees: Vec<f64> = (0..n).map(|i| graph.node_degree(i)).collect();
    let _mean_degree = degrees.iter().sum::<f64>() / n as f64;

    // Graph entropy from edge weight distribution.
    let total_weight = graph.total_weight();
    let graph_entropy = if total_weight > 0.0 {
        graph
            .edges
            .iter()
            .map(|e| {
                let p = e.weight / total_weight;
                if p > 0.0 {
                    -p * p.ln()
                } else {
                    0.0
                }
            })
            .sum::<f64>()
    } else {
        0.0
    };

    // Approximate global efficiency using shortest paths (Floyd-Warshall for small graphs).
    let global_efficiency = compute_global_efficiency(&adj, n);

    // Approximate Fiedler value using power iteration on the Laplacian.
    let fiedler_value = approximate_fiedler(&adj, n);

    // Modularity estimate from mincut (simplified).
    let mincut_result = wasm_mincut(graph).ok();
    let (modularity, global_mincut) = if let Some(ref mc) = mincut_result {
        let q = estimate_modularity(graph, &mc.partition_a, &mc.partition_b);
        (q, mc.cut_value)
    } else {
        (0.0, 0.0)
    };

    // Local efficiency (average local clustering).
    let local_efficiency = compute_local_efficiency(&adj, n);

    // Number of modules (using simple threshold-based detection).
    let num_modules = if modularity > 0.3 { 2 } else { 1 };

    Ok(TopologyMetrics {
        global_mincut,
        modularity,
        global_efficiency,
        local_efficiency,
        graph_entropy,
        fiedler_value,
        num_modules,
        timestamp: graph.timestamp,
    })
}

/// Spectral embedding using power iteration on the graph Laplacian.
///
/// Computes the `dimension` smallest non-trivial eigenvectors of the normalized
/// Laplacian using repeated power iteration with deflation. This avoids any
/// dependency on LAPACK/BLAS.
pub fn wasm_embed(
    graph: &BrainGraph,
    dimension: usize,
) -> Result<NeuralEmbedding, WasmGraphError> {
    let n = graph.num_nodes;
    if n == 0 {
        return Err(WasmGraphError("Graph has no nodes".into()));
    }
    if dimension == 0 {
        return Err(WasmGraphError("Embedding dimension must be > 0".into()));
    }
    if dimension >= n {
        return Err(WasmGraphError(format!(
            "Embedding dimension {} must be < num_nodes {}",
            dimension, n
        )));
    }

    let adj = graph.adjacency_matrix();

    // Build normalized Laplacian: L = D^(-1/2) * (D - A) * D^(-1/2)
    let degrees: Vec<f64> = (0..n).map(|i| adj[i].iter().sum::<f64>()).collect();
    let d_inv_sqrt: Vec<f64> = degrees
        .iter()
        .map(|&d| if d > 0.0 { 1.0 / d.sqrt() } else { 0.0 })
        .collect();

    let mut laplacian = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in 0..n {
            if i == j {
                laplacian[i][j] = if degrees[i] > 0.0 { 1.0 } else { 0.0 };
            } else {
                laplacian[i][j] = -adj[i][j] * d_inv_sqrt[i] * d_inv_sqrt[j];
            }
        }
    }

    // Power iteration with deflation to find smallest eigenvectors.
    // We invert the problem: find largest eigenvectors of (I - L).
    let mut inv_l = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in 0..n {
            inv_l[i][j] = if i == j {
                1.0 - laplacian[i][j]
            } else {
                -laplacian[i][j]
            };
        }
    }

    let mut eigenvectors: Vec<Vec<f64>> = Vec::new();
    let max_iter = 100;

    // Skip the first (trivial) eigenvector, compute `dimension` more.
    for _ in 0..dimension + 1 {
        let mut v = vec![0.0f64; n];
        // Initialize with pseudo-random values based on index.
        for i in 0..n {
            v[i] = ((i as f64 + 1.0) * 0.618033988749895).fract() - 0.5;
        }

        // Orthogonalize against previously found eigenvectors.
        for ev in &eigenvectors {
            let dot: f64 = v.iter().zip(ev.iter()).map(|(a, b)| a * b).sum();
            for i in 0..n {
                v[i] -= dot * ev[i];
            }
        }

        for _ in 0..max_iter {
            // Multiply: w = inv_l * v
            let mut w = vec![0.0f64; n];
            for i in 0..n {
                for j in 0..n {
                    w[i] += inv_l[i][j] * v[j];
                }
            }

            // Orthogonalize against previously found eigenvectors.
            for ev in &eigenvectors {
                let dot: f64 = w.iter().zip(ev.iter()).map(|(a, b)| a * b).sum();
                for i in 0..n {
                    w[i] -= dot * ev[i];
                }
            }

            // Normalize.
            let norm: f64 = w.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 1e-12 {
                for x in w.iter_mut() {
                    *x /= norm;
                }
            }

            v = w;
        }

        eigenvectors.push(v);
    }

    // Skip the first eigenvector (trivial constant vector), take the next `dimension`.
    let embedding_vectors: Vec<&Vec<f64>> = eigenvectors.iter().skip(1).take(dimension).collect();

    // Build embedding: each node gets a `dimension`-dimensional vector.
    // We flatten into a single vector of length n * dimension for the NeuralEmbedding.
    let mut flat_embedding = Vec::with_capacity(n * dimension);
    for node in 0..n {
        for ev in &embedding_vectors {
            flat_embedding.push(ev[node]);
        }
    }

    let metadata = EmbeddingMetadata {
        subject_id: None,
        session_id: None,
        cognitive_state: None,
        source_atlas: graph.atlas,
        embedding_method: "spectral-power-iteration".to_string(),
    };

    NeuralEmbedding::new(flat_embedding, graph.timestamp, metadata)
        .map_err(|e| WasmGraphError(e.to_string()))
}

/// Decode cognitive state from topology metrics using threshold-based rules.
///
/// This is a simplified heuristic decoder that maps topology metric patterns
/// to cognitive states without requiring a trained ML model.
pub fn wasm_decode(metrics: &TopologyMetrics) -> Result<CognitiveState, WasmGraphError> {
    // Simple threshold-based classification based on topology patterns.
    // In a production system, this would be replaced by the trained decoder
    // from ruv-neural-decoder.

    let modularity = metrics.modularity;
    let efficiency = metrics.global_efficiency;
    let fiedler = metrics.fiedler_value;
    let entropy = metrics.graph_entropy;

    // High modularity + low efficiency => segregated processing (rest, sleep).
    if modularity > 0.5 && efficiency < 0.3 {
        if entropy < 1.0 {
            return Ok(CognitiveState::Sleep(
                ruv_neural_core::topology::SleepStage::N3,
            ));
        }
        return Ok(CognitiveState::Rest);
    }

    // Low modularity + high efficiency => integrated processing (focused, creative).
    if modularity < 0.3 && efficiency > 0.6 {
        if fiedler > 0.5 {
            return Ok(CognitiveState::Focused);
        }
        return Ok(CognitiveState::Creative);
    }

    // High entropy => complex distributed processing.
    if entropy > 3.0 {
        if efficiency > 0.5 {
            return Ok(CognitiveState::MemoryRetrieval);
        }
        return Ok(CognitiveState::MemoryEncoding);
    }

    // Medium modularity => motor or speech.
    if modularity > 0.3 && modularity < 0.5 {
        if efficiency > 0.5 {
            return Ok(CognitiveState::MotorPlanning);
        }
        return Ok(CognitiveState::SpeechProcessing);
    }

    // High fiedler + low entropy => stressed/fatigued.
    if fiedler > 0.7 && entropy < 1.5 {
        return Ok(CognitiveState::Stressed);
    }
    if fiedler < 0.2 && entropy < 1.5 {
        return Ok(CognitiveState::Fatigued);
    }

    Ok(CognitiveState::Unknown)
}

// --- Internal helper functions ---

/// Compute global efficiency using Floyd-Warshall shortest paths.
fn compute_global_efficiency(adj: &[Vec<f64>], n: usize) -> f64 {
    if n < 2 {
        return 0.0;
    }

    // Initialize distance matrix with inverse weights (higher weight = shorter distance).
    let mut dist = vec![vec![f64::INFINITY; n]; n];
    for i in 0..n {
        dist[i][i] = 0.0;
        for j in 0..n {
            if i != j && adj[i][j] > 0.0 {
                dist[i][j] = 1.0 / adj[i][j];
            }
        }
    }

    // Floyd-Warshall.
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                let via_k = dist[i][k] + dist[k][j];
                if via_k < dist[i][j] {
                    dist[i][j] = via_k;
                }
            }
        }
    }

    // Global efficiency = mean of (1/d_ij) for all i != j.
    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..n {
        for j in 0..n {
            if i != j && dist[i][j].is_finite() && dist[i][j] > 0.0 {
                sum += 1.0 / dist[i][j];
                count += 1;
            }
        }
    }

    if count > 0 {
        sum / count as f64
    } else {
        0.0
    }
}

/// Approximate the Fiedler value (algebraic connectivity) using power iteration
/// on the graph Laplacian.
fn approximate_fiedler(adj: &[Vec<f64>], n: usize) -> f64 {
    if n < 2 {
        return 0.0;
    }

    // Build Laplacian: L = D - A
    let mut laplacian = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        let degree: f64 = adj[i].iter().sum();
        laplacian[i][i] = degree;
        for j in 0..n {
            if i != j {
                laplacian[i][j] = -adj[i][j];
            }
        }
    }

    // Find second-smallest eigenvalue using inverse power iteration.
    // First, find the largest eigenvalue to shift the matrix.
    let mut v = vec![0.0f64; n];
    for i in 0..n {
        v[i] = ((i as f64 + 1.0) * 0.618033988749895).fract() - 0.5;
    }

    // Orthogonalize against the trivial eigenvector (constant vector).
    let trivial: Vec<f64> = vec![1.0 / (n as f64).sqrt(); n];

    let max_iter = 50;
    for _ in 0..max_iter {
        // Multiply: w = L * v
        let mut w = vec![0.0f64; n];
        for i in 0..n {
            for j in 0..n {
                w[i] += laplacian[i][j] * v[j];
            }
        }

        // Orthogonalize against trivial eigenvector.
        let dot: f64 = w.iter().zip(trivial.iter()).map(|(a, b)| a * b).sum();
        for i in 0..n {
            w[i] -= dot * trivial[i];
        }

        // Normalize.
        let norm: f64 = w.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 1e-12 {
            for x in w.iter_mut() {
                *x /= norm;
            }
        }

        v = w;
    }

    // Rayleigh quotient: lambda = v^T L v / v^T v
    let mut vlv = 0.0;
    for i in 0..n {
        let mut lv_i = 0.0;
        for j in 0..n {
            lv_i += laplacian[i][j] * v[j];
        }
        vlv += v[i] * lv_i;
    }
    let vtv: f64 = v.iter().map(|x| x * x).sum();

    if vtv > 1e-12 {
        vlv / vtv
    } else {
        0.0
    }
}

/// Estimate Newman-Girvan modularity for a two-way partition.
fn estimate_modularity(
    graph: &BrainGraph,
    partition_a: &[usize],
    partition_b: &[usize],
) -> f64 {
    let total_weight = graph.total_weight();
    if total_weight == 0.0 {
        return 0.0;
    }
    let m = total_weight; // sum of all edge weights

    let _a_set: std::collections::HashSet<usize> = partition_a.iter().copied().collect();

    let mut q = 0.0;
    for &i in partition_a {
        for &j in partition_a {
            if i != j {
                let a_ij = graph.edge_weight(i, j).unwrap_or(0.0);
                let k_i = graph.node_degree(i);
                let k_j = graph.node_degree(j);
                q += a_ij - (k_i * k_j) / (2.0 * m);
            }
        }
    }
    for &i in partition_b {
        for &j in partition_b {
            if i != j {
                let a_ij = graph.edge_weight(i, j).unwrap_or(0.0);
                let k_i = graph.node_degree(i);
                let k_j = graph.node_degree(j);
                q += a_ij - (k_i * k_j) / (2.0 * m);
            }
        }
    }

    q / (2.0 * m)
}

/// Compute mean local efficiency (average clustering coefficient approximation).
fn compute_local_efficiency(adj: &[Vec<f64>], n: usize) -> f64 {
    if n < 3 {
        return 0.0;
    }

    let mut total_cc = 0.0;
    for i in 0..n {
        let neighbors: Vec<usize> = (0..n).filter(|&j| j != i && adj[i][j] > 0.0).collect();
        let k = neighbors.len();
        if k < 2 {
            continue;
        }

        // Count weighted triangles.
        let mut triangle_weight = 0.0;
        for &u in &neighbors {
            for &v in &neighbors {
                if u < v && adj[u][v] > 0.0 {
                    // Weighted triangle contribution.
                    triangle_weight +=
                        (adj[i][u] * adj[i][v] * adj[u][v]).cbrt();
                }
            }
        }

        let max_triangles = (k * (k - 1)) as f64 / 2.0;
        if max_triangles > 0.0 {
            // Normalize by the maximum possible strength.
            let max_weight = adj[i]
                .iter()
                .filter(|&&w| w > 0.0)
                .cloned()
                .fold(0.0f64, f64::max);
            let denom = max_triangles * max_weight;
            if denom > 0.0 {
                total_cc += triangle_weight / denom;
            }
        }
    }

    total_cc / n as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph};
    use ruv_neural_core::signal::FrequencyBand;

    fn make_test_graph() -> BrainGraph {
        // Simple 4-node graph with a clear 2-way cut:
        // 0 -- 1 (weight 5.0)
        // 2 -- 3 (weight 5.0)
        // 1 -- 2 (weight 0.1) <-- this is the cut edge
        BrainGraph {
            num_nodes: 4,
            edges: vec![
                BrainEdge {
                    source: 0,
                    target: 1,
                    weight: 5.0,
                    metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 2,
                    target: 3,
                    weight: 5.0,
                    metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
                BrainEdge {
                    source: 1,
                    target: 2,
                    weight: 0.1,
                    metric: ruv_neural_core::graph::ConnectivityMetric::Coherence,
                    frequency_band: FrequencyBand::Alpha,
                },
            ],
            timestamp: 1000.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(4),
        }
    }

    #[test]
    fn test_wasm_mincut_finds_cut() {
        let graph = make_test_graph();
        let result = wasm_mincut(&graph).unwrap();
        // The minimum cut should separate {0,1} from {2,3} with value 0.1.
        assert!((result.cut_value - 0.1).abs() < 1e-6);
        assert_eq!(result.num_cut_edges(), 1);
    }

    #[test]
    fn test_wasm_mincut_single_node() {
        let graph = BrainGraph {
            num_nodes: 1,
            edges: vec![],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(1),
        };
        let result = wasm_mincut(&graph).unwrap();
        assert_eq!(result.cut_value, 0.0);
    }

    #[test]
    fn test_wasm_topology_metrics() {
        let graph = make_test_graph();
        let metrics = wasm_topology_metrics(&graph).unwrap();
        assert!(metrics.global_mincut >= 0.0);
        assert!(metrics.graph_entropy >= 0.0);
        assert!(metrics.fiedler_value >= 0.0);
    }

    #[test]
    fn test_wasm_embed() {
        let graph = make_test_graph();
        let embedding = wasm_embed(&graph, 2).unwrap();
        // 4 nodes x 2 dimensions = 8 values.
        assert_eq!(embedding.vector.len(), 8);
    }

    #[test]
    fn test_wasm_decode_sleep() {
        let metrics = TopologyMetrics {
            global_mincut: 0.1,
            modularity: 0.6,
            global_efficiency: 0.2,
            local_efficiency: 0.3,
            graph_entropy: 0.5,
            fiedler_value: 0.3,
            num_modules: 2,
            timestamp: 0.0,
        };
        let state = wasm_decode(&metrics).unwrap();
        // High modularity + low efficiency + low entropy => deep sleep.
        assert_eq!(
            state,
            CognitiveState::Sleep(ruv_neural_core::topology::SleepStage::N3)
        );
    }

    #[test]
    fn test_wasm_decode_rest() {
        let metrics = TopologyMetrics {
            global_mincut: 0.1,
            modularity: 0.6,
            global_efficiency: 0.2,
            local_efficiency: 0.3,
            graph_entropy: 1.5,
            fiedler_value: 0.3,
            num_modules: 2,
            timestamp: 0.0,
        };
        let state = wasm_decode(&metrics).unwrap();
        // High modularity + low efficiency + moderate entropy => rest.
        assert_eq!(state, CognitiveState::Rest);
    }

    #[test]
    fn test_wasm_mincut_empty_graph() {
        let graph = BrainGraph {
            num_nodes: 0,
            edges: vec![],
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(0),
        };
        assert!(wasm_mincut(&graph).is_err());
    }
}
