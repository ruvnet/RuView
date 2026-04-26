//! Graph-theoretic metrics for brain connectivity analysis.
//!
//! Provides standard network neuroscience metrics: efficiency, clustering,
//! centrality, modularity, and small-world properties.

use ruv_neural_core::graph::BrainGraph;


/// Compute global efficiency of a brain graph.
///
/// Global efficiency is the average inverse shortest path length between all
/// pairs of nodes. For disconnected pairs, the contribution is 0.
///
/// E_global = (1 / N(N-1)) * sum_{i != j} 1/d(i,j)
pub fn global_efficiency(graph: &BrainGraph) -> f64 {
    let n = graph.num_nodes;
    if n < 2 {
        return 0.0;
    }

    let dist = all_pairs_shortest_paths(graph);
    let mut sum = 0.0;

    for i in 0..n {
        for j in 0..n {
            if i != j && dist[i][j] < f64::INFINITY {
                sum += 1.0 / dist[i][j];
            }
        }
    }

    sum / (n * (n - 1)) as f64
}

/// Compute local efficiency of a brain graph.
///
/// Average of each node's subgraph efficiency (efficiency among its neighbors).
pub fn local_efficiency(graph: &BrainGraph) -> f64 {
    let n = graph.num_nodes;
    if n < 2 {
        return 0.0;
    }

    let adj = graph.adjacency_matrix();
    let mut total = 0.0;

    for i in 0..n {
        let neighbors: Vec<usize> = (0..n)
            .filter(|&j| j != i && adj[i][j] > 0.0)
            .collect();

        let k = neighbors.len();
        if k < 2 {
            continue;
        }

        // Build subgraph of neighbors and compute its efficiency
        let mut sub_sum = 0.0;
        for &ni in &neighbors {
            for &nj in &neighbors {
                if ni != nj && adj[ni][nj] > 0.0 {
                    // Use direct weight as inverse distance proxy
                    sub_sum += adj[ni][nj];
                }
            }
        }

        total += sub_sum / (k * (k - 1)) as f64;
    }

    total / n as f64
}

/// Compute global clustering coefficient.
///
/// C = (3 * number_of_triangles) / number_of_connected_triples
/// For weighted graphs, uses the geometric mean of edge weights in triangles.
pub fn clustering_coefficient(graph: &BrainGraph) -> f64 {
    let n = graph.num_nodes;
    if n < 3 {
        return 0.0;
    }

    let adj = graph.adjacency_matrix();
    let mut triangles = 0.0;
    let mut triples = 0.0;

    for i in 0..n {
        let neighbors_i: Vec<usize> = (0..n)
            .filter(|&j| j != i && adj[i][j] > 0.0)
            .collect();
        let k = neighbors_i.len();
        if k < 2 {
            continue;
        }

        triples += (k * (k - 1)) as f64 / 2.0;

        for a in 0..neighbors_i.len() {
            for b in (a + 1)..neighbors_i.len() {
                let ni = neighbors_i[a];
                let nj = neighbors_i[b];
                if adj[ni][nj] > 0.0 {
                    // Weighted triangle: geometric mean of the three edges
                    let w = (adj[i][ni] * adj[i][nj] * adj[ni][nj]).cbrt();
                    triangles += w;
                }
            }
        }
    }

    if triples == 0.0 {
        return 0.0;
    }

    triangles / triples
}

/// Weighted degree of a single node.
pub fn node_degree(graph: &BrainGraph, node: usize) -> f64 {
    graph.node_degree(node)
}

/// Degree distribution: weighted degree for every node.
pub fn degree_distribution(graph: &BrainGraph) -> Vec<f64> {
    (0..graph.num_nodes)
        .map(|i| graph.node_degree(i))
        .collect()
}

/// Betweenness centrality for each node.
///
/// Computes the fraction of shortest paths passing through each node.
/// Uses Brandes' algorithm adapted for weighted graphs.
pub fn betweenness_centrality(graph: &BrainGraph) -> Vec<f64> {
    let n = graph.num_nodes;
    let mut centrality = vec![0.0; n];

    if n < 3 {
        return centrality;
    }

    let adj = graph.adjacency_matrix();

    // For each source node, run Dijkstra and accumulate betweenness
    for s in 0..n {
        let mut dist = vec![f64::INFINITY; n];
        let mut sigma = vec![0.0_f64; n]; // number of shortest paths
        let mut delta = vec![0.0_f64; n];
        let mut pred: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut visited = vec![false; n];
        let mut order = Vec::with_capacity(n);

        dist[s] = 0.0;
        sigma[s] = 1.0;

        // Simple Dijkstra (priority queue not needed for correctness)
        for _ in 0..n {
            // Find unvisited node with minimum distance
            let mut u = None;
            let mut min_dist = f64::INFINITY;
            for v in 0..n {
                if !visited[v] && dist[v] < min_dist {
                    min_dist = dist[v];
                    u = Some(v);
                }
            }

            let u = match u {
                Some(u) => u,
                None => break,
            };

            visited[u] = true;
            order.push(u);

            for v in 0..n {
                if adj[u][v] <= 0.0 || u == v {
                    continue;
                }
                // Convert weight to distance (stronger connection = shorter distance)
                let edge_dist = 1.0 / adj[u][v];
                let new_dist = dist[u] + edge_dist;

                if new_dist < dist[v] - 1e-12 {
                    dist[v] = new_dist;
                    sigma[v] = sigma[u];
                    pred[v] = vec![u];
                } else if (new_dist - dist[v]).abs() < 1e-12 {
                    sigma[v] += sigma[u];
                    pred[v].push(u);
                }
            }
        }

        // Back-propagation of dependencies
        for &w in order.iter().rev() {
            for &v in &pred[w] {
                if sigma[w] > 0.0 {
                    delta[v] += (sigma[v] / sigma[w]) * (1.0 + delta[w]);
                }
            }
            if w != s {
                centrality[w] += delta[w];
            }
        }
    }

    // Normalize for undirected graph
    let norm = if n > 2 {
        2.0 / ((n - 1) * (n - 2)) as f64
    } else {
        1.0
    };
    for c in &mut centrality {
        *c *= norm;
    }

    centrality
}

/// Graph density: fraction of possible edges that exist.
pub fn graph_density(graph: &BrainGraph) -> f64 {
    graph.density()
}

/// Small-world index sigma = (C/C_rand) / (L/L_rand).
///
/// Uses lattice-equivalent approximations:
/// - C_rand ~ k / N (for Erdos-Renyi)
/// - L_rand ~ ln(N) / ln(k) (for Erdos-Renyi)
///
/// where k is the mean degree and N is the number of nodes.
pub fn small_world_index(graph: &BrainGraph) -> f64 {
    let n = graph.num_nodes as f64;
    if n < 4.0 {
        return 0.0;
    }

    let c = clustering_coefficient(graph);
    let eff = global_efficiency(graph);

    // Mean binary degree
    let adj = graph.adjacency_matrix();
    let total_edges: f64 = adj
        .iter()
        .flat_map(|row| row.iter())
        .filter(|&&w| w > 0.0)
        .count() as f64
        / 2.0;
    let k = 2.0 * total_edges / n;

    if k < 1.0 || c <= 0.0 || eff <= 0.0 {
        return 0.0;
    }

    // Random graph approximations
    let c_rand = k / n;
    let l_rand = n.ln() / k.ln();
    let l = if eff > 0.0 { 1.0 / eff } else { f64::INFINITY };

    if c_rand <= 0.0 || l_rand <= 0.0 || l.is_infinite() {
        return 0.0;
    }

    (c / c_rand) / (l / l_rand)
}

/// Newman modularity Q for a given partition.
///
/// Q = (1/2m) * sum_{ij} [A_ij - k_i*k_j/(2m)] * delta(c_i, c_j)
///
/// where m is total edge weight, k_i is weighted degree of node i,
/// and delta(c_i, c_j) = 1 if nodes i and j are in the same community.
pub fn modularity(graph: &BrainGraph, partition: &[Vec<usize>]) -> f64 {
    let adj = graph.adjacency_matrix();
    let n = graph.num_nodes;

    // Build community assignment map
    let mut community = vec![0usize; n];
    for (c, members) in partition.iter().enumerate() {
        for &node in members {
            if node < n {
                community[node] = c;
            }
        }
    }

    // Total edge weight (each edge counted once in adjacency, so sum / 2)
    let m: f64 = adj.iter().flat_map(|row| row.iter()).sum::<f64>() / 2.0;
    if m == 0.0 {
        return 0.0;
    }

    // Weighted degree
    let degrees: Vec<f64> = (0..n)
        .map(|i| adj[i].iter().sum::<f64>())
        .collect();

    let mut q = 0.0;
    for i in 0..n {
        for j in 0..n {
            if community[i] == community[j] {
                q += adj[i][j] - degrees[i] * degrees[j] / (2.0 * m);
            }
        }
    }

    q / (2.0 * m)
}

/// Compute all-pairs shortest path distances using Floyd-Warshall.
///
/// Edge weights are converted to distances as 1/weight (stronger = closer).
fn all_pairs_shortest_paths(graph: &BrainGraph) -> Vec<Vec<f64>> {
    let n = graph.num_nodes;
    let adj = graph.adjacency_matrix();

    let mut dist = vec![vec![f64::INFINITY; n]; n];

    for i in 0..n {
        dist[i][i] = 0.0;
        for j in 0..n {
            if i != j && adj[i][j] > 0.0 {
                dist[i][j] = 1.0 / adj[i][j];
            }
        }
    }

    // Floyd-Warshall
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                let through_k = dist[i][k] + dist[k][j];
                if through_k < dist[i][j] {
                    dist[i][j] = through_k;
                }
            }
        }
    }

    dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::graph::{BrainEdge, BrainGraph, ConnectivityMetric};
    use ruv_neural_core::signal::FrequencyBand;

    /// Build a complete graph with n nodes, all edges weight 1.0.
    fn complete_graph(n: usize) -> BrainGraph {
        let mut edges = Vec::new();
        for i in 0..n {
            for j in (i + 1)..n {
                edges.push(BrainEdge {
                    source: i,
                    target: j,
                    weight: 1.0,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                });
            }
        }
        BrainGraph {
            num_nodes: n,
            edges,
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(n),
        }
    }

    /// Build a path graph: 0-1-2-..-(n-1).
    fn path_graph(n: usize) -> BrainGraph {
        let edges: Vec<BrainEdge> = (0..n.saturating_sub(1))
            .map(|i| BrainEdge {
                source: i,
                target: i + 1,
                weight: 1.0,
                metric: ConnectivityMetric::PhaseLockingValue,
                frequency_band: FrequencyBand::Alpha,
            })
            .collect();
        BrainGraph {
            num_nodes: n,
            edges,
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(n),
        }
    }

    #[test]
    fn global_efficiency_complete_graph() {
        // In a complete graph with weight 1, all shortest paths have length 1,
        // so efficiency = 1.0.
        let g = complete_graph(10);
        let eff = global_efficiency(&g);
        assert!((eff - 1.0).abs() < 1e-10, "Expected ~1.0, got {}", eff);
    }

    #[test]
    fn global_efficiency_empty_graph() {
        let g = BrainGraph {
            num_nodes: 5,
            edges: Vec::new(),
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(5),
        };
        let eff = global_efficiency(&g);
        assert_eq!(eff, 0.0);
    }

    #[test]
    fn clustering_coefficient_complete_graph() {
        let g = complete_graph(8);
        let cc = clustering_coefficient(&g);
        assert!(cc > 0.9, "Complete graph should have clustering ~1.0, got {}", cc);
    }

    #[test]
    fn clustering_coefficient_path_graph() {
        // A path graph has no triangles, so clustering = 0.
        let g = path_graph(5);
        let cc = clustering_coefficient(&g);
        assert!(cc.abs() < 1e-10, "Path graph should have CC=0, got {}", cc);
    }

    #[test]
    fn density_complete_graph() {
        let g = complete_graph(10);
        let d = graph_density(&g);
        assert!((d - 1.0).abs() < 1e-10, "Complete graph density should be 1.0, got {}", d);
    }

    #[test]
    fn degree_distribution_uniform() {
        let g = complete_graph(5);
        let dd = degree_distribution(&g);
        // Each node in K5 has degree 4 (4 edges * weight 1.0 = 4.0)
        for &d in &dd {
            assert!((d - 4.0).abs() < 1e-10);
        }
    }

    #[test]
    fn betweenness_centrality_path() {
        // In a path 0-1-2-3-4, middle nodes should have higher betweenness.
        let g = path_graph(5);
        let bc = betweenness_centrality(&g);
        // Node 2 (center) should have highest betweenness
        assert!(bc[2] >= bc[0], "Center node should have >= betweenness than endpoints");
        assert!(bc[2] >= bc[4], "Center node should have >= betweenness than endpoints");
    }

    #[test]
    fn modularity_single_community() {
        let g = complete_graph(6);
        let all_in_one = vec![vec![0, 1, 2, 3, 4, 5]];
        let q = modularity(&g, &all_in_one);
        // All in one community, modularity should be 0
        assert!(q.abs() < 1e-10, "Single community Q should be ~0, got {}", q);
    }

    #[test]
    fn modularity_good_partition() {
        // Two cliques connected by a weak edge
        let mut edges = Vec::new();
        // Clique 1: nodes 0,1,2
        for i in 0..3 {
            for j in (i + 1)..3 {
                edges.push(BrainEdge {
                    source: i,
                    target: j,
                    weight: 1.0,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                });
            }
        }
        // Clique 2: nodes 3,4,5
        for i in 3..6 {
            for j in (i + 1)..6 {
                edges.push(BrainEdge {
                    source: i,
                    target: j,
                    weight: 1.0,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                });
            }
        }
        // Weak bridge
        edges.push(BrainEdge {
            source: 2,
            target: 3,
            weight: 0.1,
            metric: ConnectivityMetric::PhaseLockingValue,
            frequency_band: FrequencyBand::Alpha,
        });

        let g = BrainGraph {
            num_nodes: 6,
            edges,
            timestamp: 0.0,
            window_duration_s: 1.0,
            atlas: Atlas::Custom(6),
        };

        let good = vec![vec![0, 1, 2], vec![3, 4, 5]];
        let q = modularity(&g, &good);
        assert!(q > 0.0, "Good partition should have positive modularity, got {}", q);
    }
}
