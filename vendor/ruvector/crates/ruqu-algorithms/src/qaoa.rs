//! Quantum Approximate Optimization Algorithm (QAOA) for MaxCut
//!
//! QAOA is a hybrid classical-quantum algorithm for combinatorial optimization.
//! This module implements the **MaxCut** variant: given an undirected weighted
//! graph, find a partition of vertices into two sets that maximizes the total
//! weight of edges crossing the partition.
//!
//! # Circuit structure
//!
//! A depth-p QAOA circuit has the form:
//!
//! ```text
//! |+>^n --[C(gamma_1)][B(beta_1)]--...--[C(gamma_p)][B(beta_p)]-- measure
//! ```
//!
//! where:
//! - **Phase separator** C(gamma) = prod_{(i,j) in E} exp(-i * gamma * w_ij * Z_i Z_j)
//!   is implemented with Rzz gates.
//! - **Mixer** B(beta) = prod_i exp(-i * beta * X_i) is implemented with Rx gates.
//!
//! The 2p parameters (gamma_1..gamma_p, beta_1..beta_p) are optimized
//! classically to maximize the expected cut value.

use ruqu_core::circuit::QuantumCircuit;
use ruqu_core::simulator::{SimConfig, Simulator};
use ruqu_core::types::{PauliOp, PauliString};

// ---------------------------------------------------------------------------
// Graph representation
// ---------------------------------------------------------------------------

/// Simple undirected weighted graph for MaxCut problems.
#[derive(Debug, Clone)]
pub struct Graph {
    /// Number of vertices (each mapped to one qubit).
    pub num_nodes: u32,
    /// Edges as `(node_i, node_j, weight)` triples. Both directions are
    /// represented by a single entry (undirected).
    pub edges: Vec<(u32, u32, f64)>,
}

impl Graph {
    /// Create an empty graph with the given number of nodes.
    pub fn new(num_nodes: u32) -> Self {
        Self {
            num_nodes,
            edges: Vec::new(),
        }
    }

    /// Add an undirected weighted edge between nodes `i` and `j`.
    ///
    /// # Panics
    ///
    /// Panics if `i` or `j` is out of range.
    pub fn add_edge(&mut self, i: u32, j: u32, weight: f64) {
        assert!(i < self.num_nodes, "node index {} out of range", i);
        assert!(j < self.num_nodes, "node index {} out of range", j);
        self.edges.push((i, j, weight));
    }

    /// Convenience constructor for an unweighted graph (all weights = 1.0).
    pub fn unweighted(num_nodes: u32, edges: Vec<(u32, u32)>) -> Self {
        let weighted: Vec<(u32, u32, f64)> = edges.into_iter().map(|(i, j)| (i, j, 1.0)).collect();
        Self {
            num_nodes,
            edges: weighted,
        }
    }

    /// Return the total number of edges.
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }
}

// ---------------------------------------------------------------------------
// Configuration and result types
// ---------------------------------------------------------------------------

/// Configuration for a QAOA MaxCut run.
pub struct QaoaConfig {
    /// The graph instance to solve MaxCut on.
    pub graph: Graph,
    /// QAOA depth (number of alternating phase-separation / mixing layers).
    pub p: u32,
    /// Maximum number of classical optimizer iterations.
    pub max_iterations: u32,
    /// Step size for gradient ascent.
    pub learning_rate: f64,
    /// Optional RNG seed for reproducible simulation.
    pub seed: Option<u64>,
}

/// Result of a QAOA MaxCut run.
pub struct QaoaResult {
    /// Highest expected cut value found.
    pub best_cut_value: f64,
    /// Bitstring that achieves (or approximates) `best_cut_value`.
    /// `best_bitstring[v]` is `true` when vertex `v` belongs to partition S1.
    pub best_bitstring: Vec<bool>,
    /// Optimized gamma parameters (phase-separation angles).
    pub optimal_gammas: Vec<f64>,
    /// Optimized beta parameters (mixer angles).
    pub optimal_betas: Vec<f64>,
    /// Expected cut value at each iteration.
    pub energy_history: Vec<f64>,
    /// Whether the optimizer converged.
    pub converged: bool,
}

// ---------------------------------------------------------------------------
// Circuit construction
// ---------------------------------------------------------------------------

/// Build a QAOA circuit for the MaxCut problem on `graph`.
///
/// The circuit starts with Hadamard on every qubit (equal superposition),
/// then applies `p` alternating layers:
///
/// 1. **Phase separation**: `Rzz(2 * gamma * w)` on each edge `(i, j, w)`.
/// 2. **Mixing**: `Rx(2 * beta)` on each qubit.
///
/// `gammas` and `betas` must each have length `p`.
pub fn build_qaoa_circuit(graph: &Graph, gammas: &[f64], betas: &[f64]) -> QuantumCircuit {
    assert_eq!(
        gammas.len(),
        betas.len(),
        "gammas and betas must have equal length"
    );
    let n = graph.num_nodes;
    let p = gammas.len();
    let mut circuit = QuantumCircuit::new(n);

    // Initial equal superposition
    for q in 0..n {
        circuit.h(q);
    }

    // QAOA layers
    for layer in 0..p {
        // Phase separator: Rzz for each edge
        for &(i, j, w) in &graph.edges {
            circuit.rzz(i, j, 2.0 * gammas[layer] * w);
        }
        // Mixer: Rx on each qubit
        for q in 0..n {
            circuit.rx(q, 2.0 * betas[layer]);
        }
    }

    circuit
}

// ---------------------------------------------------------------------------
// Cost evaluation
// ---------------------------------------------------------------------------

/// Compute the classical MaxCut value for a given bitstring.
///
/// An edge (i, j, w) contributes `w` to the cut if `bitstring[i] != bitstring[j]`.
pub fn cut_value(graph: &Graph, bitstring: &[bool]) -> f64 {
    graph
        .edges
        .iter()
        .filter(|(i, j, _)| bitstring[*i as usize] != bitstring[*j as usize])
        .map(|(_, _, w)| w)
        .sum()
}

/// Evaluate the expected MaxCut cost from a QAOA state.
///
/// For each edge (i, j) with weight w:
/// ```text
/// C_{ij} = w * 0.5 * (1 - <Z_i Z_j>)
/// ```
///
/// The total expected cost is the sum over all edges.
pub fn evaluate_qaoa_cost(
    graph: &Graph,
    gammas: &[f64],
    betas: &[f64],
    seed: Option<u64>,
) -> ruqu_core::error::Result<f64> {
    let circuit = build_qaoa_circuit(graph, gammas, betas);
    let sim_config = SimConfig {
        seed,
        noise: None,
        shots: None,
    };
    let result = Simulator::run_with_config(&circuit, &sim_config)?;

    let mut cost = 0.0;
    for &(i, j, w) in &graph.edges {
        let zz = result.state.expectation_value(&PauliString {
            ops: vec![(i, PauliOp::Z), (j, PauliOp::Z)],
        });
        cost += w * 0.5 * (1.0 - zz);
    }
    Ok(cost)
}

// ---------------------------------------------------------------------------
// QAOA optimizer
// ---------------------------------------------------------------------------

/// Run QAOA optimization for MaxCut using gradient ascent with the
/// parameter-shift rule.
///
/// The optimizer maximizes the expected cut value by adjusting gamma and beta
/// parameters. Convergence is declared when the absolute change in cost
/// between successive iterations drops below 1e-6.
///
/// # Errors
///
/// Returns a [`ruqu_core::error::QuantumError`] on simulator failures.
pub fn run_qaoa(config: &QaoaConfig) -> ruqu_core::error::Result<QaoaResult> {
    let p = config.p as usize;

    // Initialize parameters at reasonable starting values.
    let mut gammas = vec![0.5_f64; p];
    let mut betas = vec![0.5_f64; p];
    let mut energy_history: Vec<f64> = Vec::with_capacity(config.max_iterations as usize);
    let mut best_cost = f64::NEG_INFINITY;
    let mut best_bitstring = vec![false; config.graph.num_nodes as usize];
    let mut converged = false;

    for iter in 0..config.max_iterations {
        // ------------------------------------------------------------------
        // Evaluate current expected cost
        // ------------------------------------------------------------------
        let cost = evaluate_qaoa_cost(&config.graph, &gammas, &betas, config.seed)?;
        energy_history.push(cost);

        // ------------------------------------------------------------------
        // Track best solution: sample the most probable bitstring
        // ------------------------------------------------------------------
        if cost > best_cost {
            best_cost = cost;
            let circuit = build_qaoa_circuit(&config.graph, &gammas, &betas);
            let sim_result = Simulator::run_with_config(
                &circuit,
                &SimConfig {
                    seed: config.seed,
                    noise: None,
                    shots: None,
                },
            )?;
            let probs = sim_result.state.probabilities();
            let best_idx = probs
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0);
            best_bitstring = (0..config.graph.num_nodes)
                .map(|q| (best_idx >> q) & 1 == 1)
                .collect();
        }

        // ------------------------------------------------------------------
        // Convergence check
        // ------------------------------------------------------------------
        if iter > 0 {
            let prev = energy_history[iter as usize - 1];
            if (cost - prev).abs() < 1e-6 {
                converged = true;
                break;
            }
        }

        // ------------------------------------------------------------------
        // Gradient ascent via parameter-shift rule
        // ------------------------------------------------------------------
        let shift = std::f64::consts::FRAC_PI_2;

        // Update gamma parameters
        for i in 0..p {
            let mut gp = gammas.clone();
            gp[i] += shift;
            let mut gm = gammas.clone();
            gm[i] -= shift;
            let cp = evaluate_qaoa_cost(&config.graph, &gp, &betas, config.seed)?;
            let cm = evaluate_qaoa_cost(&config.graph, &gm, &betas, config.seed)?;
            gammas[i] += config.learning_rate * (cp - cm) / 2.0;
        }

        // Update beta parameters
        for i in 0..p {
            let mut bp = betas.clone();
            bp[i] += shift;
            let mut bm = betas.clone();
            bm[i] -= shift;
            let cp = evaluate_qaoa_cost(&config.graph, &gammas, &bp, config.seed)?;
            let cm = evaluate_qaoa_cost(&config.graph, &gammas, &bm, config.seed)?;
            betas[i] += config.learning_rate * (cp - cm) / 2.0;
        }
    }

    Ok(QaoaResult {
        best_cut_value: best_cost,
        best_bitstring,
        optimal_gammas: gammas,
        optimal_betas: betas,
        energy_history,
        converged,
    })
}

// ---------------------------------------------------------------------------
// Graph construction helpers
// ---------------------------------------------------------------------------

/// Create a triangle graph (3 nodes, 3 edges, all weight 1).
///
/// The optimal MaxCut is 2 (any partition has exactly one edge within a
/// group and two edges crossing).
pub fn triangle_graph() -> Graph {
    Graph::unweighted(3, vec![(0, 1), (1, 2), (0, 2)])
}

/// Create a 4-node ring graph (cycle C4, all weight 1).
///
/// The optimal MaxCut is 4 (bipartition {0,2} vs {1,3} cuts all edges).
pub fn ring4_graph() -> Graph {
    Graph::unweighted(4, vec![(0, 1), (1, 2), (2, 3), (3, 0)])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_construction() {
        let g = triangle_graph();
        assert_eq!(g.num_nodes, 3);
        assert_eq!(g.num_edges(), 3);
    }

    #[test]
    fn test_graph_add_edge() {
        let mut g = Graph::new(4);
        g.add_edge(0, 1, 2.5);
        g.add_edge(2, 3, 1.0);
        assert_eq!(g.num_edges(), 2);
    }

    #[test]
    #[should_panic(expected = "node index 5 out of range")]
    fn test_graph_add_edge_out_of_range() {
        let mut g = Graph::new(4);
        g.add_edge(0, 5, 1.0);
    }

    #[test]
    fn test_cut_value_triangle() {
        let g = triangle_graph();
        // Partition {0} vs {1,2}: edges (0,1) and (0,2) are cut, (1,2) is not.
        assert_eq!(cut_value(&g, &[true, false, false]), 2.0);
        // All same partition: no cut.
        assert_eq!(cut_value(&g, &[false, false, false]), 0.0);
    }

    #[test]
    fn test_cut_value_ring4() {
        let g = ring4_graph();
        // Optimal: alternate partitions {0,2} vs {1,3} -> cut all 4 edges.
        assert_eq!(cut_value(&g, &[true, false, true, false]), 4.0);
    }

    #[test]
    fn test_build_qaoa_circuit_gate_count() {
        let g = triangle_graph();
        let gammas = vec![0.5];
        let betas = vec![0.3];
        let circuit = build_qaoa_circuit(&g, &gammas, &betas);
        assert_eq!(circuit.num_qubits(), 3);
        // 3 H + 3 Rzz + 3 Rx = 9 gates
        assert_eq!(circuit.gates().len(), 9);
    }

    #[test]
    fn test_cut_value_weighted() {
        let mut g = Graph::new(3);
        g.add_edge(0, 1, 2.0);
        g.add_edge(1, 2, 3.0);
        // Partition {0,2} vs {1}: cuts both edges -> 2.0 + 3.0 = 5.0
        assert_eq!(cut_value(&g, &[true, false, true]), 5.0);
    }
}
