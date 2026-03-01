//! Hybrid classical-quantum circuit decomposition engine.
//!
//! Performs structural decomposition of quantum circuits across simulation
//! paradigms using graph-based partitioning. Most quantum simulation systems
//! commit to a single backend for an entire circuit. This engine partitions
//! a circuit into segments that are independently routed to the optimal
//! backend (StateVector, Stabilizer, or TensorNetwork), yielding significant
//! performance gains for heterogeneous circuits.
//!
//! # Decomposition strategies
//!
//! | Strategy | Description |
//! |----------|-------------|
//! | `Temporal` | Split by time slices (barrier gates or natural idle boundaries) |
//! | `Spatial` | Split by qubit subsets (connected components or min-cut partitioning) |
//! | `Hybrid` | Both temporal and spatial decomposition applied in sequence |
//! | `None` | No decomposition; the whole circuit is a single segment |
//!
//! # Example
//!
//! ```
//! use ruqu_core::circuit::QuantumCircuit;
//! use ruqu_core::decomposition::decompose;
//!
//! // Two independent Bell pairs on disjoint qubits.
//! let mut circ = QuantumCircuit::new(4);
//! circ.h(0).cnot(0, 1);   // Bell pair on qubits 0-1
//! circ.h(2).cnot(2, 3);   // Bell pair on qubits 2-3
//!
//! let partition = decompose(&circ, 25);
//! assert_eq!(partition.segments.len(), 2);
//! ```

use std::collections::{HashMap, HashSet, VecDeque};

use crate::backend::BackendType;
use crate::circuit::QuantumCircuit;
use crate::gate::Gate;
use crate::stabilizer::StabilizerState;

// ---------------------------------------------------------------------------
// Public data structures
// ---------------------------------------------------------------------------

/// The result of decomposing a circuit into independently-simulable segments.
#[derive(Debug, Clone)]
pub struct CircuitPartition {
    /// Ordered list of circuit segments to simulate.
    pub segments: Vec<CircuitSegment>,
    /// Total qubit count of the original circuit.
    pub total_qubits: u32,
    /// Strategy that was used for decomposition.
    pub strategy: DecompositionStrategy,
}

/// A single segment of a decomposed circuit, ready for backend dispatch.
#[derive(Debug, Clone)]
pub struct CircuitSegment {
    /// The sub-circuit to simulate.
    pub circuit: QuantumCircuit,
    /// The backend selected for this segment.
    pub backend: BackendType,
    /// Inclusive range of original qubit indices covered by this segment.
    pub qubit_range: (u32, u32),
    /// Start and end gate indices in the original circuit (end is exclusive).
    pub gate_range: (usize, usize),
    /// Estimated simulation cost of this segment.
    pub estimated_cost: SegmentCost,
}

/// Estimated resource consumption for simulating a circuit segment.
#[derive(Debug, Clone)]
pub struct SegmentCost {
    /// Estimated memory consumption in bytes.
    pub memory_bytes: u64,
    /// Estimated floating-point operations.
    pub estimated_flops: u64,
    /// Number of qubits in this segment.
    pub qubit_count: u32,
}

/// Strategy used for circuit decomposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecompositionStrategy {
    /// Split by time slices (gate layers / barriers).
    Temporal,
    /// Split by qubit subsets (connected components / partitioning).
    Spatial,
    /// Both temporal and spatial decomposition applied.
    Hybrid,
    /// No decomposition; the circuit is a single segment.
    None,
}

// ---------------------------------------------------------------------------
// Interaction graph
// ---------------------------------------------------------------------------

/// Qubit interaction graph extracted from a quantum circuit.
///
/// Nodes are qubits. Edges are two-qubit gates, weighted by the number of
/// such gates between each pair.
#[derive(Debug, Clone)]
pub struct InteractionGraph {
    /// Number of qubits (nodes) in the graph.
    pub num_qubits: u32,
    /// Edges as `(qubit_a, qubit_b, gate_count)`.
    pub edges: Vec<(u32, u32, usize)>,
    /// Adjacency list: `adjacency[q]` contains the neighbours of qubit `q`.
    pub adjacency: Vec<Vec<u32>>,
}

/// Build the qubit interaction graph for a circuit.
///
/// Every two-qubit gate contributes an edge (or increments the weight of an
/// existing edge) between the two qubits it acts on.
pub fn build_interaction_graph(circuit: &QuantumCircuit) -> InteractionGraph {
    let n = circuit.num_qubits();
    let mut edge_counts: HashMap<(u32, u32), usize> = HashMap::new();

    for gate in circuit.gates() {
        let qubits = gate.qubits();
        if qubits.len() == 2 {
            let (a, b) = if qubits[0] <= qubits[1] {
                (qubits[0], qubits[1])
            } else {
                (qubits[1], qubits[0])
            };
            *edge_counts.entry((a, b)).or_insert(0) += 1;
        }
    }

    let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); n as usize];
    let mut edges: Vec<(u32, u32, usize)> = Vec::with_capacity(edge_counts.len());

    for (&(a, b), &count) in &edge_counts {
        edges.push((a, b, count));
        if !adjacency[a as usize].contains(&b) {
            adjacency[a as usize].push(b);
        }
        if !adjacency[b as usize].contains(&a) {
            adjacency[b as usize].push(a);
        }
    }

    // Sort adjacency lists for deterministic traversal.
    for adj in &mut adjacency {
        adj.sort_unstable();
    }

    InteractionGraph {
        num_qubits: n,
        edges,
        adjacency,
    }
}

// ---------------------------------------------------------------------------
// Connected components (BFS)
// ---------------------------------------------------------------------------

/// Find connected components of the qubit interaction graph using BFS.
///
/// Returns a list of components, each being a sorted list of qubit indices.
/// Isolated qubits (those with no two-qubit gate interactions) are each
/// returned as their own singleton component.
pub fn find_connected_components(graph: &InteractionGraph) -> Vec<Vec<u32>> {
    let n = graph.num_qubits as usize;
    let mut visited = vec![false; n];
    let mut components: Vec<Vec<u32>> = Vec::new();

    for start in 0..n {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut component = vec![start as u32];
        let mut queue = VecDeque::new();
        queue.push_back(start as u32);

        while let Some(node) = queue.pop_front() {
            for &neighbor in &graph.adjacency[node as usize] {
                if !visited[neighbor as usize] {
                    visited[neighbor as usize] = true;
                    component.push(neighbor);
                    queue.push_back(neighbor);
                }
            }
        }

        component.sort_unstable();
        components.push(component);
    }

    components
}

// ---------------------------------------------------------------------------
// Temporal decomposition
// ---------------------------------------------------------------------------

/// Split a circuit at `Barrier` gates or at natural breakpoints where no
/// qubit is active across the boundary.
///
/// A natural breakpoint occurs when all qubits that have been touched in the
/// current slice have been measured or reset, making them logically idle.
///
/// Returns a list of sub-circuits. Each sub-circuit preserves the original
/// qubit count so that qubit indices remain valid.
pub fn temporal_decomposition(circuit: &QuantumCircuit) -> Vec<QuantumCircuit> {
    let gates = circuit.gates();
    if gates.is_empty() {
        return vec![QuantumCircuit::new(circuit.num_qubits())];
    }

    let n = circuit.num_qubits();
    let mut slices: Vec<QuantumCircuit> = Vec::new();
    let mut current = QuantumCircuit::new(n);
    let mut current_has_gates = false;

    // Track which qubits have been used (touched) in the current slice
    // and which of those have been subsequently measured/reset.
    let mut active_qubits: HashSet<u32> = HashSet::new();
    let mut measured_qubits: HashSet<u32> = HashSet::new();

    for gate in gates {
        match gate {
            Gate::Barrier => {
                // Barrier always forces a slice boundary.
                if current_has_gates {
                    slices.push(current);
                    current = QuantumCircuit::new(n);
                    current_has_gates = false;
                    active_qubits.clear();
                    measured_qubits.clear();
                }
            }
            _ => {
                let qubits = gate.qubits();

                // Before adding this gate, check if we have a natural breakpoint:
                // All previously-active qubits have been measured/reset, and this
                // gate touches at least one qubit not yet in the active set.
                if current_has_gates
                    && !active_qubits.is_empty()
                    && active_qubits.iter().all(|q| measured_qubits.contains(q))
                {
                    // All active qubits are measured/reset -- natural boundary.
                    slices.push(current);
                    current = QuantumCircuit::new(n);
                    active_qubits.clear();
                    measured_qubits.clear();
                }

                // Track measurement/reset operations.
                match gate {
                    Gate::Measure(q) => {
                        measured_qubits.insert(*q);
                    }
                    Gate::Reset(q) => {
                        measured_qubits.insert(*q);
                    }
                    _ => {}
                }

                // Mark touched qubits as active.
                for &q in &qubits {
                    active_qubits.insert(q);
                }

                current.add_gate(gate.clone());
                current_has_gates = true;
            }
        }
    }

    // Push the final slice if it has any gates.
    if current_has_gates {
        slices.push(current);
    }

    // Guarantee at least one circuit is returned.
    if slices.is_empty() {
        slices.push(QuantumCircuit::new(n));
    }

    slices
}

// ---------------------------------------------------------------------------
// Stoer-Wagner minimum cut
// ---------------------------------------------------------------------------

/// Result of a Stoer-Wagner minimum cut computation.
#[derive(Debug, Clone)]
pub struct MinCutResult {
    /// The minimum cut value (sum of edge weights crossing the cut).
    pub cut_value: usize,
    /// One side of the partition (qubit indices).
    pub partition_a: Vec<u32>,
    /// Other side of the partition.
    pub partition_b: Vec<u32>,
}

/// Compute the minimum cut of an interaction graph using Stoer-Wagner.
///
/// Time complexity: O(V * E + V^2 * log V) which is O(V^3) for dense graphs.
/// This is optimal for finding a global minimum cut without specifying s and t.
///
/// Returns `None` if the graph has 0 or 1 nodes.
pub fn stoer_wagner_mincut(graph: &InteractionGraph) -> Option<MinCutResult> {
    let n = graph.num_qubits as usize;
    if n <= 1 {
        return None;
    }

    // Build a weighted adjacency matrix.
    let mut adj = vec![vec![0usize; n]; n];
    for &(a, b, w) in &graph.edges {
        let (a, b) = (a as usize, b as usize);
        adj[a][b] += w;
        adj[b][a] += w;
    }

    // Track which original vertices are merged into each super-vertex.
    let mut merged: Vec<Vec<u32>> = (0..n).map(|i| vec![i as u32]).collect();
    let mut active: Vec<bool> = vec![true; n];

    let mut best_cut_value = usize::MAX;
    let mut best_partition: Vec<u32> = Vec::new();

    for _ in 0..(n - 1) {
        // Stoer-Wagner phase: find the most tightly connected vertex ordering.
        let active_nodes: Vec<usize> = (0..n).filter(|&i| active[i]).collect();
        if active_nodes.len() < 2 {
            break;
        }

        let mut in_a = vec![false; n];
        let mut weight_to_a = vec![0usize; n];

        // Start with the first active node.
        let start = active_nodes[0];
        in_a[start] = true;

        // Update weights for neighbors of start.
        for &node in &active_nodes {
            if node != start {
                weight_to_a[node] = adj[start][node];
            }
        }

        let mut prev = start;
        let mut last = start;

        for _ in 1..active_nodes.len() {
            // Find the most tightly connected vertex not yet in A.
            let next = active_nodes
                .iter()
                .filter(|&&v| !in_a[v])
                .max_by_key(|&&v| weight_to_a[v])
                .copied()
                .unwrap();

            prev = last;
            last = next;
            in_a[next] = true;

            // Update weights.
            for &node in &active_nodes {
                if !in_a[node] {
                    weight_to_a[node] += adj[next][node];
                }
            }
        }

        // The cut-of-the-phase is the weight of last vertex added.
        let cut_of_phase = weight_to_a[last];

        if cut_of_phase < best_cut_value {
            best_cut_value = cut_of_phase;
            best_partition = merged[last].clone();
        }

        // Merge last into prev.
        for &node in &active_nodes {
            if node != last && node != prev {
                adj[prev][node] += adj[last][node];
                adj[node][prev] += adj[node][last];
            }
        }
        active[last] = false;
        let last_merged = std::mem::take(&mut merged[last]);
        merged[prev].extend(last_merged);
    }

    let partition_a_set: HashSet<u32> = best_partition.iter().copied().collect();
    let mut partition_a: Vec<u32> = best_partition;
    partition_a.sort_unstable();
    let mut partition_b: Vec<u32> = (0..n as u32)
        .filter(|q| !partition_a_set.contains(q))
        .collect();
    partition_b.sort_unstable();

    Some(MinCutResult {
        cut_value: best_cut_value,
        partition_a,
        partition_b,
    })
}

/// Spatial decomposition using Stoer-Wagner minimum cut.
///
/// Recursively bisects the circuit along minimum cuts until all segments
/// have at most `max_qubits` qubits. Produces better partitions than the
/// greedy approach by minimizing the number of cross-partition entangling
/// gates.
pub fn spatial_decomposition_mincut(
    circuit: &QuantumCircuit,
    graph: &InteractionGraph,
    max_qubits: u32,
) -> Vec<(Vec<u32>, QuantumCircuit)> {
    let n = graph.num_qubits;
    if n == 0 || max_qubits == 0 {
        return Vec::new();
    }
    if n <= max_qubits {
        let all_qubits: Vec<u32> = (0..n).collect();
        return vec![(all_qubits, circuit.clone())];
    }

    // Recursively bisect using Stoer-Wagner.
    let mut result = Vec::new();
    recursive_mincut_partition(circuit, graph, max_qubits, &mut result);
    result
}

/// Recursively partition using min-cut bisection.
fn recursive_mincut_partition(
    circuit: &QuantumCircuit,
    graph: &InteractionGraph,
    max_qubits: u32,
    result: &mut Vec<(Vec<u32>, QuantumCircuit)>,
) {
    let n = graph.num_qubits;
    if n <= max_qubits {
        let all_qubits: Vec<u32> = (0..n).collect();
        result.push((all_qubits, circuit.clone()));
        return;
    }

    match stoer_wagner_mincut(graph) {
        Some(cut) => {
            // Extract subcircuits for each partition.
            let set_a: HashSet<u32> = cut.partition_a.iter().copied().collect();
            let set_b: HashSet<u32> = cut.partition_b.iter().copied().collect();

            let circ_a = extract_component_circuit(circuit, &set_a);
            let circ_b = extract_component_circuit(circuit, &set_b);

            let graph_a = build_interaction_graph(&circ_a);
            let graph_b = build_interaction_graph(&circ_b);

            // Recurse on each half.
            if cut.partition_a.len() as u32 > max_qubits {
                recursive_mincut_partition(&circ_a, &graph_a, max_qubits, result);
            } else {
                result.push((cut.partition_a, circ_a));
            }

            if cut.partition_b.len() as u32 > max_qubits {
                recursive_mincut_partition(&circ_b, &graph_b, max_qubits, result);
            } else {
                result.push((cut.partition_b, circ_b));
            }
        }
        None => {
            // Cannot partition further.
            let all_qubits: Vec<u32> = (0..n).collect();
            result.push((all_qubits, circuit.clone()));
        }
    }
}

// ---------------------------------------------------------------------------
// Spatial decomposition (greedy heuristic)
// ---------------------------------------------------------------------------

/// Partition qubits into groups of at most `max_qubits` using a greedy
/// min-cut heuristic, then extract subcircuits for each group.
///
/// Algorithm:
/// 1. Pick the highest-degree unassigned qubit as a seed.
/// 2. Greedily add adjacent qubits (preferring those with more edges into
///    the current group) until the group reaches `max_qubits` or no more
///    connected qubits remain.
/// 3. Repeat until all qubits in the interaction graph are assigned.
/// 4. For each group, extract the gates that operate exclusively within
///    the group. Cross-group gates (whose qubits span multiple groups)
///    are included in the group that contains the majority of their qubits,
///    with the remote qubit added to the subcircuit.
///
/// Returns `(qubit_group, subcircuit)` pairs.
pub fn spatial_decomposition(
    circuit: &QuantumCircuit,
    graph: &InteractionGraph,
    max_qubits: u32,
) -> Vec<(Vec<u32>, QuantumCircuit)> {
    let n = graph.num_qubits;
    if n == 0 || max_qubits == 0 {
        return Vec::new();
    }

    // If the circuit fits within max_qubits, return it as a single group.
    if n <= max_qubits {
        let all_qubits: Vec<u32> = (0..n).collect();
        return vec![(all_qubits, circuit.clone())];
    }

    // Compute degree for each qubit.
    let mut degree: Vec<usize> = vec![0; n as usize];
    for &(a, b, count) in &graph.edges {
        degree[a as usize] += count;
        degree[b as usize] += count;
    }

    let mut assigned = vec![false; n as usize];
    let mut groups: Vec<Vec<u32>> = Vec::new();

    while assigned.iter().any(|&a| !a) {
        // Pick the highest-degree unassigned qubit as seed.
        let seed = (0..n as usize)
            .filter(|&q| !assigned[q])
            .max_by_key(|&q| degree[q])
            .unwrap() as u32;

        let mut group = vec![seed];
        assigned[seed as usize] = true;

        // Greedily expand the group.
        while (group.len() as u32) < max_qubits {
            // Find the unassigned neighbor with the most connections into group.
            let mut best_candidate: Option<u32> = Option::None;
            let mut best_score: usize = 0;

            for &member in &group {
                for &neighbor in &graph.adjacency[member as usize] {
                    if assigned[neighbor as usize] {
                        continue;
                    }
                    // Score = number of edges from this neighbor into group members.
                    let score: usize = graph.adjacency[neighbor as usize]
                        .iter()
                        .filter(|&&adj| group.contains(&adj))
                        .count();
                    if score > best_score
                        || (score == best_score && best_candidate.map_or(true, |bc| neighbor < bc))
                    {
                        best_score = score;
                        best_candidate = Some(neighbor);
                    }
                }
            }

            match best_candidate {
                Some(candidate) => {
                    assigned[candidate as usize] = true;
                    group.push(candidate);
                }
                Option::None => break, // No more connected unassigned neighbors.
            }
        }

        group.sort_unstable();
        groups.push(group);
    }

    // For each group, build a subcircuit with remapped qubit indices.
    let mut result: Vec<(Vec<u32>, QuantumCircuit)> = Vec::new();

    // Build a lookup: original qubit -> group index.
    let mut qubit_to_group: Vec<usize> = vec![0; n as usize];
    for (gi, group) in groups.iter().enumerate() {
        for &q in group {
            qubit_to_group[q as usize] = gi;
        }
    }

    for group in &groups {
        let group_set: HashSet<u32> = group.iter().copied().collect();

        // Build the qubit remapping: original index -> local index.
        // We may need to include extra qubits for cross-group gates.
        let mut local_qubits: Vec<u32> = group.clone();

        // First pass: identify any extra qubits needed for cross-group gates
        // that have at least one qubit in this group.
        for gate in circuit.gates() {
            let gate_qubits = gate.qubits();
            if gate_qubits.is_empty() {
                continue;
            }
            let in_group = gate_qubits.iter().filter(|q| group_set.contains(q)).count();
            let out_group = gate_qubits.len() - in_group;
            if in_group > 0 && out_group > 0 {
                // This is a cross-group gate. If the majority of qubits are in
                // this group, include the remote qubits.
                if in_group >= out_group {
                    for &q in &gate_qubits {
                        if !local_qubits.contains(&q) {
                            local_qubits.push(q);
                        }
                    }
                }
            }
        }

        local_qubits.sort_unstable();
        let num_local = local_qubits.len() as u32;
        let remap: HashMap<u32, u32> = local_qubits
            .iter()
            .enumerate()
            .map(|(i, &q)| (q, i as u32))
            .collect();

        let mut sub_circuit = QuantumCircuit::new(num_local);

        // Second pass: add gates that belong to this group.
        for gate in circuit.gates() {
            let gate_qubits = gate.qubits();

            // Barrier: include in every sub-circuit.
            if matches!(gate, Gate::Barrier) {
                sub_circuit.add_gate(Gate::Barrier);
                continue;
            }

            if gate_qubits.is_empty() {
                continue;
            }

            let in_group = gate_qubits.iter().filter(|q| group_set.contains(q)).count();
            if in_group == 0 {
                continue; // Gate does not touch this group at all.
            }

            let out_group = gate_qubits.len() - in_group;
            if out_group > 0 && in_group < out_group {
                continue; // Gate is majority in another group.
            }

            // All qubits must be in our local remap.
            if gate_qubits.iter().all(|q| remap.contains_key(q)) {
                let remapped = remap_gate(gate, &remap);
                sub_circuit.add_gate(remapped);
            }
        }

        result.push((group.clone(), sub_circuit));
    }

    result
}

/// Remap qubit indices in a gate according to the given mapping.
fn remap_gate(gate: &Gate, remap: &HashMap<u32, u32>) -> Gate {
    match gate {
        Gate::H(q) => Gate::H(remap[q]),
        Gate::X(q) => Gate::X(remap[q]),
        Gate::Y(q) => Gate::Y(remap[q]),
        Gate::Z(q) => Gate::Z(remap[q]),
        Gate::S(q) => Gate::S(remap[q]),
        Gate::Sdg(q) => Gate::Sdg(remap[q]),
        Gate::T(q) => Gate::T(remap[q]),
        Gate::Tdg(q) => Gate::Tdg(remap[q]),
        Gate::Rx(q, a) => Gate::Rx(remap[q], *a),
        Gate::Ry(q, a) => Gate::Ry(remap[q], *a),
        Gate::Rz(q, a) => Gate::Rz(remap[q], *a),
        Gate::Phase(q, a) => Gate::Phase(remap[q], *a),
        Gate::CNOT(c, t) => Gate::CNOT(remap[c], remap[t]),
        Gate::CZ(a, b) => Gate::CZ(remap[a], remap[b]),
        Gate::SWAP(a, b) => Gate::SWAP(remap[a], remap[b]),
        Gate::Rzz(a, b, angle) => Gate::Rzz(remap[a], remap[b], *angle),
        Gate::Measure(q) => Gate::Measure(remap[q]),
        Gate::Reset(q) => Gate::Reset(remap[q]),
        Gate::Barrier => Gate::Barrier,
        Gate::Unitary1Q(q, m) => Gate::Unitary1Q(remap[q], *m),
    }
}

// ---------------------------------------------------------------------------
// Backend classification
// ---------------------------------------------------------------------------

/// Determine the best backend for a circuit segment based on its gate composition.
///
/// Decision rules:
/// 1. If all gates are Clifford (or non-unitary) -> `Stabilizer`
/// 2. If `num_qubits <= 25` -> `StateVector`
/// 3. If `num_qubits > 25` and T-count <= 40 -> `CliffordT`
/// 4. If `num_qubits > 25` and T-count > 40 -> `TensorNetwork`
/// 5. Otherwise -> `StateVector`
pub fn classify_segment(segment: &QuantumCircuit) -> BackendType {
    let mut has_non_clifford = false;
    let mut t_count: usize = 0;

    for gate in segment.gates() {
        if gate.is_non_unitary() {
            continue;
        }
        if !StabilizerState::is_clifford_gate(gate) {
            has_non_clifford = true;
            t_count += 1;
        }
    }

    if !has_non_clifford {
        return BackendType::Stabilizer;
    }

    if segment.num_qubits() <= 25 {
        return BackendType::StateVector;
    }

    // Moderate T-count on large circuits -> CliffordT (Bravyi-Gosset).
    // 2^t stabilizer terms; practical up to ~40 T-gates.
    if t_count <= 40 {
        return BackendType::CliffordT;
    }

    // High T-count with > 25 qubits -> TensorNetwork
    BackendType::TensorNetwork
}

// ---------------------------------------------------------------------------
// Cost estimation
// ---------------------------------------------------------------------------

/// Estimate the simulation cost of a circuit segment on a given backend.
///
/// The estimates are order-of-magnitude correct and intended for comparing
/// relative costs between decomposition options, not for precise prediction.
pub fn estimate_segment_cost(segment: &QuantumCircuit, backend: BackendType) -> SegmentCost {
    let n = segment.num_qubits();
    let gate_count = segment.gate_count() as u64;

    match backend {
        BackendType::StateVector => {
            // Memory: 2^n complex amplitudes * 16 bytes each.
            let state_size = if n <= 63 { 1u64 << n } else { u64::MAX / 16 };
            let memory_bytes = state_size.saturating_mul(16);
            // FLOPs: each gate touches O(2^n) amplitudes with a few ops each.
            // Single-qubit: ~4 * 2^(n-1) FLOPs; two-qubit: ~8 * 2^(n-2).
            // Simplified to 8 * 2^n per gate.
            let flops_per_gate = if n <= 60 {
                8u64.saturating_mul(1u64 << n)
            } else {
                u64::MAX / gate_count.max(1)
            };
            let estimated_flops = gate_count.saturating_mul(flops_per_gate);
            SegmentCost {
                memory_bytes,
                estimated_flops,
                qubit_count: n,
            }
        }
        BackendType::Stabilizer => {
            // Memory: tableau of 2n rows x (2n+1) bits, stored as bools.
            let tableau_size = 2 * (n as u64) * (2 * (n as u64) + 1);
            let memory_bytes = tableau_size; // 1 byte per bool in practice
                                             // FLOPs: O(n^2) per gate (row operations over 2n rows of width 2n+1).
            let flops_per_gate = 4 * (n as u64) * (n as u64);
            let estimated_flops = gate_count.saturating_mul(flops_per_gate);
            SegmentCost {
                memory_bytes,
                estimated_flops,
                qubit_count: n,
            }
        }
        BackendType::TensorNetwork => {
            // Memory: n tensors, each of dimension up to chi^2 * 4 (bond dim).
            // Default chi ~ 64 for moderate entanglement.
            let chi: u64 = 64;
            let tensor_bytes = (n as u64) * chi * chi * 16; // complex entries
            let memory_bytes = tensor_bytes;
            // FLOPs: each gate requires SVD truncation ~ O(chi^3).
            let flops_per_gate = chi * chi * chi;
            let estimated_flops = gate_count.saturating_mul(flops_per_gate);
            SegmentCost {
                memory_bytes,
                estimated_flops,
                qubit_count: n,
            }
        }
        BackendType::CliffordT => {
            // Memory: 2^t stabiliser tableaux, each n^2 / 4 bytes.
            let analysis = crate::backend::analyze_circuit(segment);
            let t = analysis.non_clifford_gates as u32;
            let terms: u64 = 1u64.checked_shl(t).unwrap_or(u64::MAX);
            let tableau_bytes = (n as u64).saturating_mul(n as u64) / 4;
            let memory_bytes = terms.saturating_mul(tableau_bytes).max(1);
            // FLOPs: each of 2^t terms processes every gate at O(n^2).
            let flops_per_gate = 4 * (n as u64) * (n as u64);
            let estimated_flops = terms
                .saturating_mul(gate_count)
                .saturating_mul(flops_per_gate);
            SegmentCost {
                memory_bytes,
                estimated_flops,
                qubit_count: n,
            }
        }
        BackendType::Auto => {
            // For Auto, classify first, then estimate with the resolved backend.
            let resolved = classify_segment(segment);
            estimate_segment_cost(segment, resolved)
        }
    }
}

// ---------------------------------------------------------------------------
// Result stitching
// ---------------------------------------------------------------------------

/// Probabilistically combine measurement results from independent circuit
/// segments.
///
/// For independent segments, the probability of a combined bitstring is the
/// product of the individual segment probabilities:
///
/// ```text
/// P(combined) = P(segment_0) * P(segment_1) * ...
/// ```
///
/// Each input element is `(bitstring, probability)` from one segment's
/// simulation. The output maps combined bitstrings to their joint
/// probabilities.
pub fn stitch_results(partitions: &[(Vec<bool>, f64)]) -> HashMap<Vec<bool>, f64> {
    if partitions.is_empty() {
        return HashMap::new();
    }

    // Group entries by segment: consecutive entries form a segment until the
    // bitstring length changes. For simplicity, if all bitstrings have the
    // same length, we treat them as a single segment and return as-is.
    //
    // The more general approach: the caller provides results as a flat list
    // of (bitstring, probability) pairs from multiple independent segments.
    // We combine by taking the Cartesian product.
    //
    // We use a simple iterative approach: start with an empty combined result,
    // and for each new segment result, concatenate bitstrings and multiply
    // probabilities.

    // To differentiate segments, we group by consecutive runs of equal-length
    // bitstrings. This is a pragmatic heuristic -- callers should provide
    // segment results in order, with each segment having a distinct length.

    let mut segments: Vec<Vec<(Vec<bool>, f64)>> = Vec::new();
    let mut current_segment: Vec<(Vec<bool>, f64)> = Vec::new();
    let mut current_len: Option<usize> = Option::None;

    for (bits, prob) in partitions {
        match current_len {
            Some(l) if l == bits.len() => {
                current_segment.push((bits.clone(), *prob));
            }
            _ => {
                if !current_segment.is_empty() {
                    segments.push(current_segment);
                    current_segment = Vec::new();
                }
                current_len = Some(bits.len());
                current_segment.push((bits.clone(), *prob));
            }
        }
    }
    if !current_segment.is_empty() {
        segments.push(current_segment);
    }

    // Iteratively compute the Cartesian product.
    let mut combined: Vec<(Vec<bool>, f64)> = vec![(Vec::new(), 1.0)];

    for segment in &segments {
        let mut next_combined: Vec<(Vec<bool>, f64)> = Vec::new();
        for (base_bits, base_prob) in &combined {
            for (seg_bits, seg_prob) in segment {
                let mut merged = base_bits.clone();
                merged.extend_from_slice(seg_bits);
                next_combined.push((merged, base_prob * seg_prob));
            }
        }
        combined = next_combined;
    }

    let mut result: HashMap<Vec<bool>, f64> = HashMap::new();
    for (bits, prob) in combined {
        *result.entry(bits).or_insert(0.0) += prob;
    }

    result
}

// ---------------------------------------------------------------------------
// Fidelity-aware stitching
// ---------------------------------------------------------------------------

/// Fidelity estimate for a partition boundary.
///
/// Models the information loss when a quantum circuit is split across
/// a partition boundary where entangling gates were cut. Each cut
/// entangling gate reduces the fidelity by a factor related to the
/// Schmidt decomposition rank at the cut.
#[derive(Debug, Clone)]
pub struct StitchFidelity {
    /// Overall fidelity estimate (product of per-cut fidelities).
    pub fidelity: f64,
    /// Number of entangling gates that were cut.
    pub cut_gates: usize,
    /// Per-cut fidelity values.
    pub per_cut_fidelity: Vec<f64>,
}

/// Stitch results with fidelity estimation.
///
/// Like [`stitch_results`], but also estimates the fidelity loss from
/// partitioning. Each entangling gate that crosses a partition boundary
/// contributes a fidelity penalty:
///
/// ```text
/// F_cut = 1 / sqrt(2^k)
/// ```
///
/// where k is the number of entangling gates crossing that particular
/// boundary. This is a conservative upper bound derived from the fact
/// that each maximally entangling gate can create at most 1 ebit of
/// entanglement, and cutting it loses at most 1 bit of mutual information.
///
/// # Arguments
///
/// * `partitions` - Flat list of (bitstring, probability) pairs from all segments.
/// * `partition_info` - The `CircuitPartition` used to understand cut structure.
/// * `original_circuit` - The original (undecomposed) circuit for cut analysis.
pub fn stitch_with_fidelity(
    partitions: &[(Vec<bool>, f64)],
    partition_info: &CircuitPartition,
    original_circuit: &QuantumCircuit,
) -> (HashMap<Vec<bool>, f64>, StitchFidelity) {
    // Get the basic stitched distribution.
    let distribution = stitch_results(partitions);

    // Compute fidelity from the partition structure.
    let fidelity = estimate_stitch_fidelity(partition_info, original_circuit);

    (distribution, fidelity)
}

/// Estimate fidelity loss from circuit partitioning.
///
/// Analyzes the original circuit to count how many entangling gates
/// cross each partition boundary.
fn estimate_stitch_fidelity(
    partition_info: &CircuitPartition,
    original_circuit: &QuantumCircuit,
) -> StitchFidelity {
    if partition_info.segments.len() <= 1 {
        return StitchFidelity {
            fidelity: 1.0,
            cut_gates: 0,
            per_cut_fidelity: Vec::new(),
        };
    }

    // Build a map: original qubit -> segment index.
    let mut qubit_to_segment: HashMap<u32, usize> = HashMap::new();
    for (seg_idx, segment) in partition_info.segments.iter().enumerate() {
        let (lo, hi) = segment.qubit_range;
        for q in lo..=hi {
            qubit_to_segment.entry(q).or_insert(seg_idx);
        }
    }

    // Count entangling gates that cross segment boundaries.
    // Group by boundary pair (seg_a, seg_b) to compute per-boundary fidelity.
    let mut boundary_cuts: HashMap<(usize, usize), usize> = HashMap::new();
    let mut total_cut_gates = 0usize;

    for gate in original_circuit.gates() {
        let qubits = gate.qubits();
        if qubits.len() != 2 {
            continue;
        }
        let seg_a = qubit_to_segment.get(&qubits[0]).copied();
        let seg_b = qubit_to_segment.get(&qubits[1]).copied();

        if let (Some(a), Some(b)) = (seg_a, seg_b) {
            if a != b {
                let key = if a < b { (a, b) } else { (b, a) };
                *boundary_cuts.entry(key).or_insert(0) += 1;
                total_cut_gates += 1;
            }
        }
    }

    // Compute per-boundary fidelity: F = 1/sqrt(2^k) where k is cut gate count.
    // This is conservative -- assumes each cut gate creates maximal entanglement.
    let per_cut_fidelity: Vec<f64> = boundary_cuts
        .values()
        .map(|&k| {
            if k == 0 {
                1.0
            } else {
                // F = 2^(-k/2)
                2.0_f64.powf(-(k as f64) / 2.0)
            }
        })
        .collect();

    let overall_fidelity = per_cut_fidelity.iter().product::<f64>();

    StitchFidelity {
        fidelity: overall_fidelity,
        cut_gates: total_cut_gates,
        per_cut_fidelity,
    }
}

// ---------------------------------------------------------------------------
// Main decomposition entry point
// ---------------------------------------------------------------------------

/// Decompose a quantum circuit into segments for multi-backend simulation.
///
/// This is the primary entry point for the decomposition engine. The
/// algorithm proceeds as follows:
///
/// 1. Build the qubit interaction graph (nodes = qubits, edges = two-qubit
///    gates).
/// 2. Identify connected components. Disconnected components become separate
///    spatial segments immediately.
/// 3. For each connected component, attempt temporal decomposition at
///    barriers and natural breakpoints.
/// 4. Classify each resulting segment to select the optimal backend.
/// 5. If any segment exceeds `max_segment_qubits`, attempt further spatial
///    decomposition using a greedy min-cut heuristic.
/// 6. Estimate costs for every final segment.
///
/// # Arguments
///
/// * `circuit` - The circuit to decompose.
/// * `max_segment_qubits` - Maximum number of qubits allowed per segment.
///   Segments exceeding this limit are spatially subdivided.
pub fn decompose(circuit: &QuantumCircuit, max_segment_qubits: u32) -> CircuitPartition {
    let n = circuit.num_qubits();
    let gates = circuit.gates();

    // Trivial case: empty circuit or single qubit.
    if gates.is_empty() || n <= 1 {
        let backend = classify_segment(circuit);
        let cost = estimate_segment_cost(circuit, backend);
        return CircuitPartition {
            segments: vec![CircuitSegment {
                circuit: circuit.clone(),
                backend,
                qubit_range: (0, n.saturating_sub(1)),
                gate_range: (0, gates.len()),
                estimated_cost: cost,
            }],
            total_qubits: n,
            strategy: DecompositionStrategy::None,
        };
    }

    // Step 1: Build the interaction graph.
    let graph = build_interaction_graph(circuit);

    // Step 2: Find connected components.
    let components = find_connected_components(&graph);

    let mut used_spatial = false;
    let mut used_temporal = false;
    let mut final_segments: Vec<CircuitSegment> = Vec::new();

    if components.len() > 1 {
        used_spatial = true;
    }

    // Step 3: For each connected component, extract its subcircuit and
    // attempt temporal decomposition.
    for component in &components {
        let comp_set: HashSet<u32> = component.iter().copied().collect();

        // Extract the subcircuit for this component.
        let comp_circuit = extract_component_circuit(circuit, &comp_set);

        // Find the gate index range in the original circuit for this component.
        let gate_indices = gate_indices_for_component(circuit, &comp_set);
        let gate_range_start = gate_indices.first().copied().unwrap_or(0);
        let _gate_range_end = gate_indices.last().map(|&i| i + 1).unwrap_or(0);

        // Temporal decomposition within the component.
        let time_slices = temporal_decomposition(&comp_circuit);

        if time_slices.len() > 1 {
            used_temporal = true;
        }

        // Track cumulative gate offset for slices.
        let mut slice_gate_offset = gate_range_start;

        for slice_circuit in &time_slices {
            let slice_gate_count = slice_circuit.gate_count();

            // Step 4: Classify the segment.
            let backend = classify_segment(slice_circuit);

            // Step 5: If the segment is too large, attempt spatial decomposition.
            if slice_circuit.num_qubits() > max_segment_qubits
                && active_qubit_count(slice_circuit) > max_segment_qubits
            {
                used_spatial = true;
                let sub_graph = build_interaction_graph(slice_circuit);
                let sub_parts =
                    spatial_decomposition(slice_circuit, &sub_graph, max_segment_qubits);

                for (qubit_group, sub_circ) in &sub_parts {
                    let sub_backend = classify_segment(sub_circ);
                    let cost = estimate_segment_cost(sub_circ, sub_backend);
                    let qmin = qubit_group.iter().copied().min().unwrap_or(0);
                    let qmax = qubit_group.iter().copied().max().unwrap_or(0);

                    final_segments.push(CircuitSegment {
                        circuit: sub_circ.clone(),
                        backend: sub_backend,
                        qubit_range: (qmin, qmax),
                        gate_range: (slice_gate_offset, slice_gate_offset + slice_gate_count),
                        estimated_cost: cost,
                    });
                }
            } else {
                let cost = estimate_segment_cost(slice_circuit, backend);
                let qmin = component.iter().copied().min().unwrap_or(0);
                let qmax = component.iter().copied().max().unwrap_or(0);

                final_segments.push(CircuitSegment {
                    circuit: slice_circuit.clone(),
                    backend,
                    qubit_range: (qmin, qmax),
                    gate_range: (slice_gate_offset, slice_gate_offset + slice_gate_count),
                    estimated_cost: cost,
                });
            }

            slice_gate_offset += slice_gate_count;
        }
    }

    // Determine the overall strategy.
    let strategy = match (used_temporal, used_spatial) {
        (true, true) => DecompositionStrategy::Hybrid,
        (true, false) => DecompositionStrategy::Temporal,
        (false, true) => DecompositionStrategy::Spatial,
        (false, false) => DecompositionStrategy::None,
    };

    CircuitPartition {
        segments: final_segments,
        total_qubits: n,
        strategy,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Count the number of qubits that are actually used (touched by at least one
/// gate) in a circuit.
fn active_qubit_count(circuit: &QuantumCircuit) -> u32 {
    let mut active: HashSet<u32> = HashSet::new();
    for gate in circuit.gates() {
        for &q in &gate.qubits() {
            active.insert(q);
        }
    }
    active.len() as u32
}

/// Extract a subcircuit containing only the gates that act on qubits in the
/// given component set. The subcircuit has `num_qubits` equal to the size of
/// the component, with qubit indices remapped to `0..component.len()`.
fn extract_component_circuit(circuit: &QuantumCircuit, component: &HashSet<u32>) -> QuantumCircuit {
    // Build a sorted list for deterministic remapping.
    let mut sorted_qubits: Vec<u32> = component.iter().copied().collect();
    sorted_qubits.sort_unstable();
    let remap: HashMap<u32, u32> = sorted_qubits
        .iter()
        .enumerate()
        .map(|(i, &q)| (q, i as u32))
        .collect();

    let num_local = sorted_qubits.len() as u32;
    let mut sub_circuit = QuantumCircuit::new(num_local);

    for gate in circuit.gates() {
        match gate {
            Gate::Barrier => {
                // Include barriers in every component subcircuit.
                sub_circuit.add_gate(Gate::Barrier);
            }
            _ => {
                let qubits = gate.qubits();
                if qubits.is_empty() {
                    continue;
                }
                // Include the gate only if all its qubits are in this component.
                if qubits.iter().all(|q| component.contains(q)) {
                    sub_circuit.add_gate(remap_gate(gate, &remap));
                }
            }
        }
    }

    sub_circuit
}

/// Find the gate indices in the original circuit that belong to a given
/// qubit component.
fn gate_indices_for_component(circuit: &QuantumCircuit, component: &HashSet<u32>) -> Vec<usize> {
    circuit
        .gates()
        .iter()
        .enumerate()
        .filter_map(|(i, gate)| {
            let qubits = gate.qubits();
            if qubits.is_empty() {
                return Some(i); // Barrier belongs to all components.
            }
            if qubits.iter().any(|q| component.contains(q)) {
                Some(i)
            } else {
                Option::None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create two independent Bell pairs on qubits (0,1) and (2,3).
    fn two_bell_pairs() -> QuantumCircuit {
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).cnot(0, 1); // Bell pair on 0,1
        circ.h(2).cnot(2, 3); // Bell pair on 2,3
        circ
    }

    // ----- Test 1: Two independent Bell states decompose into 2 spatial segments -----

    #[test]
    fn two_independent_bell_states_decompose_into_two_segments() {
        let circ = two_bell_pairs();
        let partition = decompose(&circ, 25);

        assert_eq!(
            partition.segments.len(),
            2,
            "expected 2 segments for two independent Bell pairs, got {}",
            partition.segments.len()
        );
        assert_eq!(partition.strategy, DecompositionStrategy::Spatial);

        // Each segment should have 2 qubits.
        for seg in &partition.segments {
            assert_eq!(
                seg.circuit.num_qubits(),
                2,
                "each Bell pair segment should have 2 qubits"
            );
        }
    }

    // ----- Test 2: Pure Clifford segment is classified as Stabilizer -----

    #[test]
    fn pure_clifford_classified_as_stabilizer() {
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).cnot(0, 1).s(2).cz(2, 3).x(1).y(3).z(0);

        let backend = classify_segment(&circ);
        assert_eq!(
            backend,
            BackendType::Stabilizer,
            "all-Clifford circuit should be classified as Stabilizer"
        );
    }

    // ----- Test 3: Temporal decomposition splits at barriers -----

    #[test]
    fn temporal_decomposition_splits_at_barriers() {
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).cnot(0, 1);
        circ.barrier();
        circ.x(0).z(1);

        let slices = temporal_decomposition(&circ);
        assert_eq!(
            slices.len(),
            2,
            "expected 2 time slices around barrier, got {}",
            slices.len()
        );

        // First slice: H + CNOT = 2 gates.
        assert_eq!(slices[0].gate_count(), 2);
        // Second slice: X + Z = 2 gates.
        assert_eq!(slices[1].gate_count(), 2);
    }

    // ----- Test 4: Connected circuit stays as single segment -----

    #[test]
    fn connected_circuit_stays_as_single_segment() {
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).cnot(0, 1).cnot(1, 2).cnot(2, 3);

        let partition = decompose(&circ, 25);
        assert_eq!(
            partition.segments.len(),
            1,
            "fully connected circuit should remain a single segment"
        );
        assert_eq!(partition.strategy, DecompositionStrategy::None);
    }

    // ----- Test 5: Interaction graph correctly counts two-qubit gate edges -----

    #[test]
    fn interaction_graph_counts_edges() {
        let mut circ = QuantumCircuit::new(3);
        circ.cnot(0, 1); // edge (0,1)
        circ.cnot(0, 1); // edge (0,1) again
        circ.cz(1, 2); // edge (1,2)

        let graph = build_interaction_graph(&circ);

        assert_eq!(graph.num_qubits, 3);
        assert_eq!(graph.edges.len(), 2, "should have 2 distinct edges");

        // Find the (0,1) edge and check its count.
        let edge_01 = graph.edges.iter().find(|&&(a, b, _)| a == 0 && b == 1);
        assert!(edge_01.is_some(), "edge (0,1) should exist");
        assert_eq!(edge_01.unwrap().2, 2, "edge (0,1) should have count 2");

        // Find the (1,2) edge.
        let edge_12 = graph.edges.iter().find(|&&(a, b, _)| a == 1 && b == 2);
        assert!(edge_12.is_some(), "edge (1,2) should exist");
        assert_eq!(edge_12.unwrap().2, 1, "edge (1,2) should have count 1");

        // Check adjacency.
        assert!(graph.adjacency[0].contains(&1));
        assert!(graph.adjacency[1].contains(&0));
        assert!(graph.adjacency[1].contains(&2));
        assert!(graph.adjacency[2].contains(&1));
    }

    // ----- Test 6: Spatial decomposition respects max_qubits limit -----

    #[test]
    fn spatial_decomposition_respects_max_qubits() {
        // Create a 6-qubit circuit with a chain of CNOT gates.
        let mut circ = QuantumCircuit::new(6);
        for q in 0..5 {
            circ.cnot(q, q + 1);
        }

        let graph = build_interaction_graph(&circ);
        let parts = spatial_decomposition(&circ, &graph, 3);

        // Every group should have at most 3 qubits.
        for (group, _sub_circ) in &parts {
            assert!(
                group.len() <= 3,
                "group {:?} has {} qubits, expected at most 3",
                group,
                group.len()
            );
        }

        // All 6 qubits should be covered.
        let mut all_qubits: Vec<u32> = parts
            .iter()
            .flat_map(|(group, _)| group.iter().copied())
            .collect();
        all_qubits.sort_unstable();
        all_qubits.dedup();
        assert_eq!(all_qubits.len(), 6, "all 6 qubits should be covered");
    }

    // ----- Test 7: Segment cost estimation produces reasonable values -----

    #[test]
    fn segment_cost_estimation_reasonable() {
        let mut circ = QuantumCircuit::new(10);
        circ.h(0).cnot(0, 1).t(2);

        // StateVector cost.
        let sv_cost = estimate_segment_cost(&circ, BackendType::StateVector);
        assert_eq!(sv_cost.qubit_count, 10);
        // 2^10 * 16 = 16384 bytes.
        assert_eq!(sv_cost.memory_bytes, 16384);
        assert!(sv_cost.estimated_flops > 0);

        // Stabilizer cost.
        let stab_cost = estimate_segment_cost(&circ, BackendType::Stabilizer);
        assert_eq!(stab_cost.qubit_count, 10);
        // Tableau: 2*10*(2*10+1) = 420 bytes.
        assert_eq!(stab_cost.memory_bytes, 420);
        assert!(stab_cost.estimated_flops > 0);

        // TensorNetwork cost.
        let tn_cost = estimate_segment_cost(&circ, BackendType::TensorNetwork);
        assert_eq!(tn_cost.qubit_count, 10);
        // 10 * 64 * 64 * 16 = 655360.
        assert_eq!(tn_cost.memory_bytes, 655_360);
        assert!(tn_cost.estimated_flops > 0);

        // StateVector memory should be much less than TN for small qubit counts,
        // and stabilizer should be the smallest.
        assert!(stab_cost.memory_bytes < sv_cost.memory_bytes);
    }

    // ----- Test 8: 10-qubit GHZ circuit stays as one segment (fully connected) -----

    #[test]
    fn ghz_10_qubit_single_segment() {
        let mut circ = QuantumCircuit::new(10);
        circ.h(0);
        for q in 0..9 {
            circ.cnot(q, q + 1);
        }

        let partition = decompose(&circ, 25);
        assert_eq!(
            partition.segments.len(),
            1,
            "10-qubit GHZ circuit should stay as one segment"
        );

        // The GHZ circuit is all Clifford, so backend should be Stabilizer.
        assert_eq!(partition.segments[0].backend, BackendType::Stabilizer);
    }

    // ----- Test 9: Disconnected 20-qubit circuit decomposes -----

    #[test]
    fn disconnected_20_qubit_circuit_decomposes() {
        let mut circ = QuantumCircuit::new(20);

        // Block A: qubits 0..9 (GHZ-like).
        circ.h(0);
        for q in 0..9 {
            circ.cnot(q, q + 1);
        }

        // Block B: qubits 10..19 (GHZ-like).
        circ.h(10);
        for q in 10..19 {
            circ.cnot(q, q + 1);
        }

        let partition = decompose(&circ, 25);
        assert_eq!(
            partition.segments.len(),
            2,
            "two disconnected 10-qubit blocks should yield 2 segments, got {}",
            partition.segments.len()
        );
        assert_eq!(partition.total_qubits, 20);
        assert_eq!(partition.strategy, DecompositionStrategy::Spatial);

        // Each segment should have 10 qubits.
        for seg in &partition.segments {
            assert_eq!(seg.circuit.num_qubits(), 10);
        }
    }

    // ----- Additional tests for edge cases and coverage -----

    #[test]
    fn empty_circuit_produces_single_segment() {
        let circ = QuantumCircuit::new(4);
        let partition = decompose(&circ, 25);
        assert_eq!(partition.segments.len(), 1);
        assert_eq!(partition.strategy, DecompositionStrategy::None);
    }

    #[test]
    fn single_qubit_circuit() {
        let mut circ = QuantumCircuit::new(1);
        circ.h(0).t(0);
        let partition = decompose(&circ, 25);
        assert_eq!(partition.segments.len(), 1);
        assert_eq!(partition.segments[0].backend, BackendType::StateVector);
    }

    #[test]
    fn mixed_clifford_non_clifford_classification() {
        // Circuit with one T gate among Cliffords.
        let mut circ = QuantumCircuit::new(5);
        circ.h(0).cnot(0, 1).t(2).s(3);

        let backend = classify_segment(&circ);
        assert_eq!(
            backend,
            BackendType::StateVector,
            "mixed circuit with <= 25 qubits should use StateVector"
        );
    }

    #[test]
    fn connected_components_isolated_qubits() {
        // Circuit where qubit 2 has no two-qubit gates.
        let mut circ = QuantumCircuit::new(3);
        circ.cnot(0, 1).h(2);

        let graph = build_interaction_graph(&circ);
        let components = find_connected_components(&graph);

        assert_eq!(
            components.len(),
            2,
            "qubit 2 is isolated, should form its own component"
        );

        // One component should be {0, 1}, the other {2}.
        let has_pair = components.iter().any(|c| c == &vec![0, 1]);
        let has_single = components.iter().any(|c| c == &vec![2]);
        assert!(has_pair, "component {{0, 1}} should exist");
        assert!(has_single, "component {{2}} should exist");
    }

    #[test]
    fn stitch_results_independent_segments() {
        // Segment 1: 1-qubit outcomes.
        // Segment 2: 1-qubit outcomes.
        let partitions = vec![
            (vec![false], 0.5),
            (vec![true], 0.5),
            (vec![false, false], 0.25),
            (vec![true, true], 0.75),
        ];

        let combined = stitch_results(&partitions);

        // Combined bitstrings: 1-bit x 2-bit.
        // (false, false, false) = 0.5 * 0.25 = 0.125
        // (false, true, true)   = 0.5 * 0.75 = 0.375
        // (true, false, false)  = 0.5 * 0.25 = 0.125
        // (true, true, true)    = 0.5 * 0.75 = 0.375
        assert_eq!(combined.len(), 4);

        let prob_fff = combined
            .get(&vec![false, false, false])
            .copied()
            .unwrap_or(0.0);
        let prob_ftt = combined
            .get(&vec![false, true, true])
            .copied()
            .unwrap_or(0.0);
        let prob_tff = combined
            .get(&vec![true, false, false])
            .copied()
            .unwrap_or(0.0);
        let prob_ttt = combined
            .get(&vec![true, true, true])
            .copied()
            .unwrap_or(0.0);

        assert!((prob_fff - 0.125).abs() < 1e-10);
        assert!((prob_ftt - 0.375).abs() < 1e-10);
        assert!((prob_tff - 0.125).abs() < 1e-10);
        assert!((prob_ttt - 0.375).abs() < 1e-10);
    }

    #[test]
    fn stitch_results_empty() {
        let combined = stitch_results(&[]);
        assert!(combined.is_empty());
    }

    #[test]
    fn classify_large_moderate_t_as_clifford_t() {
        // 30 qubits with 1 T-gate -> CliffordT (moderate T-count, large circuit).
        let mut circ = QuantumCircuit::new(30);
        circ.h(0);
        circ.t(1); // non-Clifford
        for q in 0..29 {
            circ.cnot(q, q + 1);
        }

        let backend = classify_segment(&circ);
        assert_eq!(
            backend,
            BackendType::CliffordT,
            "moderate T-count on > 25 qubits should use CliffordT"
        );
    }

    #[test]
    fn classify_large_high_t_as_tensor_network() {
        // 30 qubits with 50 T-gates -> TensorNetwork (too many for CliffordT).
        let mut circ = QuantumCircuit::new(30);
        for q in 0..29 {
            circ.cnot(q, q + 1);
        }
        for _ in 0..50 {
            circ.rx(0, 1.0); // non-Clifford
        }

        let backend = classify_segment(&circ);
        assert_eq!(
            backend,
            BackendType::TensorNetwork,
            "high T-count on > 25 qubits should use TensorNetwork"
        );
    }

    #[test]
    fn temporal_decomposition_no_barriers_single_slice() {
        let mut circ = QuantumCircuit::new(2);
        circ.h(0).cnot(0, 1);

        let slices = temporal_decomposition(&circ);
        assert_eq!(
            slices.len(),
            1,
            "circuit without barriers should produce a single time slice"
        );
        assert_eq!(slices[0].gate_count(), 2);
    }

    #[test]
    fn temporal_decomposition_multiple_barriers() {
        let mut circ = QuantumCircuit::new(2);
        circ.h(0);
        circ.barrier();
        circ.cnot(0, 1);
        circ.barrier();
        circ.x(0);

        let slices = temporal_decomposition(&circ);
        assert_eq!(
            slices.len(),
            3,
            "two barriers should produce three time slices"
        );
    }

    #[test]
    fn cost_auto_backend_resolves() {
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).cnot(0, 1);

        let cost = estimate_segment_cost(&circ, BackendType::Auto);
        // Auto should resolve to Stabilizer for this all-Clifford circuit.
        let stab_cost = estimate_segment_cost(&circ, BackendType::Stabilizer);
        assert_eq!(cost.memory_bytes, stab_cost.memory_bytes);
        assert_eq!(cost.estimated_flops, stab_cost.estimated_flops);
    }

    #[test]
    fn decompose_with_measurements() {
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).cnot(0, 1).measure(0).measure(1);
        circ.h(2).cnot(2, 3).measure(2).measure(3);

        let partition = decompose(&circ, 25);
        // Qubits (0,1) and (2,3) are disconnected.
        assert_eq!(partition.segments.len(), 2);
    }

    #[test]
    fn interaction_graph_empty_circuit() {
        let circ = QuantumCircuit::new(5);
        let graph = build_interaction_graph(&circ);

        assert_eq!(graph.num_qubits, 5);
        assert!(graph.edges.is_empty());
        for adj in &graph.adjacency {
            assert!(adj.is_empty());
        }
    }

    #[test]
    fn connected_components_fully_connected() {
        let mut circ = QuantumCircuit::new(4);
        circ.cnot(0, 1).cnot(1, 2).cnot(2, 3);

        let graph = build_interaction_graph(&circ);
        let components = find_connected_components(&graph);

        assert_eq!(
            components.len(),
            1,
            "fully connected chain should be one component"
        );
        assert_eq!(components[0], vec![0, 1, 2, 3]);
    }

    #[test]
    fn spatial_decomposition_returns_single_group_if_fits() {
        let mut circ = QuantumCircuit::new(4);
        circ.cnot(0, 1).cnot(2, 3);

        let graph = build_interaction_graph(&circ);
        let parts = spatial_decomposition(&circ, &graph, 10);

        // 4 qubits <= 10, so should return a single group.
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].0, vec![0, 1, 2, 3]);
    }

    #[test]
    fn segment_qubit_ranges_are_valid() {
        let circ = two_bell_pairs();
        let partition = decompose(&circ, 25);

        for seg in &partition.segments {
            let (qmin, qmax) = seg.qubit_range;
            assert!(qmin <= qmax, "qubit_range should be non-inverted");
            assert!(
                qmax < partition.total_qubits,
                "qubit_range max should be within total_qubits"
            );
        }
    }

    #[test]
    fn classify_segment_measure_only() {
        // A circuit with only measurements should be classified as Stabilizer
        // (all gates are non-unitary, so has_non_clifford stays false).
        let mut circ = QuantumCircuit::new(3);
        circ.measure(0).measure(1).measure(2);

        let backend = classify_segment(&circ);
        assert_eq!(backend, BackendType::Stabilizer);
    }

    #[test]
    fn classify_segment_empty_circuit() {
        let circ = QuantumCircuit::new(5);
        let backend = classify_segment(&circ);
        assert_eq!(
            backend,
            BackendType::Stabilizer,
            "empty circuit has no non-Clifford gates"
        );
    }

    // ----- Stoer-Wagner min-cut tests -----

    #[test]
    fn test_stoer_wagner_mincut_linear() {
        // Linear chain: 0-1-2-3-4
        // Min cut should be 1 (cutting any single edge).
        let mut circ = QuantumCircuit::new(5);
        circ.cnot(0, 1).cnot(1, 2).cnot(2, 3).cnot(3, 4);
        let graph = build_interaction_graph(&circ);
        let cut = stoer_wagner_mincut(&graph).unwrap();
        assert_eq!(cut.cut_value, 1);
        assert!(!cut.partition_a.is_empty());
        assert!(!cut.partition_b.is_empty());
    }

    #[test]
    fn test_stoer_wagner_mincut_triangle() {
        // Triangle: 0-1, 1-2, 0-2 (each with weight 1).
        // Min cut = 2 (cutting any vertex out cuts 2 edges).
        let mut circ = QuantumCircuit::new(3);
        circ.cnot(0, 1).cnot(1, 2).cnot(0, 2);
        let graph = build_interaction_graph(&circ);
        let cut = stoer_wagner_mincut(&graph).unwrap();
        assert_eq!(cut.cut_value, 2);
    }

    #[test]
    fn test_stoer_wagner_mincut_barbell() {
        // Barbell: clique(0,1,2) - bridge(2,3) - clique(3,4,5)
        // Min cut should be 1 (cutting the bridge).
        let mut circ = QuantumCircuit::new(6);
        // Left clique.
        circ.cnot(0, 1).cnot(1, 2).cnot(0, 2);
        // Bridge.
        circ.cnot(2, 3);
        // Right clique.
        circ.cnot(3, 4).cnot(4, 5).cnot(3, 5);
        let graph = build_interaction_graph(&circ);
        let cut = stoer_wagner_mincut(&graph).unwrap();
        assert_eq!(cut.cut_value, 1);
    }

    #[test]
    fn test_spatial_decomposition_mincut() {
        // 6-qubit barbell, max 3 qubits per segment.
        let mut circ = QuantumCircuit::new(6);
        circ.cnot(0, 1).cnot(1, 2).cnot(0, 2);
        circ.cnot(2, 3);
        circ.cnot(3, 4).cnot(4, 5).cnot(3, 5);
        let graph = build_interaction_graph(&circ);
        let parts = spatial_decomposition_mincut(&circ, &graph, 3);
        assert!(parts.len() >= 2, "Should partition into at least 2 groups");
        for (qubits, _sub_circ) in &parts {
            assert!(
                qubits.len() as u32 <= 3,
                "Each group should have at most 3 qubits"
            );
        }
    }

    // ----- Fidelity-aware stitching tests -----

    #[test]
    fn test_stitch_with_fidelity_single_segment() {
        let circ = QuantumCircuit::new(2);
        let partition = CircuitPartition {
            segments: vec![CircuitSegment {
                circuit: circ.clone(),
                backend: BackendType::Stabilizer,
                qubit_range: (0, 1),
                gate_range: (0, 0),
                estimated_cost: SegmentCost {
                    memory_bytes: 0,
                    estimated_flops: 0,
                    qubit_count: 2,
                },
            }],
            total_qubits: 2,
            strategy: DecompositionStrategy::None,
        };
        let partitions = vec![(vec![false, false], 1.0)];
        let (dist, fidelity) = stitch_with_fidelity(&partitions, &partition, &circ);
        assert_eq!(fidelity.fidelity, 1.0);
        assert_eq!(fidelity.cut_gates, 0);
        assert!(!dist.is_empty());
    }

    #[test]
    fn test_stitch_with_fidelity_cut_circuit() {
        // Circuit with a CNOT crossing a partition boundary.
        let mut circ = QuantumCircuit::new(4);
        circ.h(0).cnot(0, 1); // Bell pair 0-1
        circ.h(2).cnot(2, 3); // Bell pair 2-3
        circ.cnot(1, 2); // Cross-partition gate

        let partition = CircuitPartition {
            segments: vec![
                CircuitSegment {
                    circuit: {
                        let mut c = QuantumCircuit::new(2);
                        c.h(0).cnot(0, 1);
                        c
                    },
                    backend: BackendType::Stabilizer,
                    qubit_range: (0, 1),
                    gate_range: (0, 2),
                    estimated_cost: SegmentCost {
                        memory_bytes: 0,
                        estimated_flops: 0,
                        qubit_count: 2,
                    },
                },
                CircuitSegment {
                    circuit: {
                        let mut c = QuantumCircuit::new(2);
                        c.h(0).cnot(0, 1);
                        c
                    },
                    backend: BackendType::Stabilizer,
                    qubit_range: (2, 3),
                    gate_range: (2, 4),
                    estimated_cost: SegmentCost {
                        memory_bytes: 0,
                        estimated_flops: 0,
                        qubit_count: 2,
                    },
                },
            ],
            total_qubits: 4,
            strategy: DecompositionStrategy::Spatial,
        };

        let partitions = vec![
            (vec![false, false], 0.5),
            (vec![true, true], 0.5),
            (vec![false, false], 0.5),
            (vec![true, true], 0.5),
        ];
        let (_dist, fidelity) = stitch_with_fidelity(&partitions, &partition, &circ);
        assert!(
            fidelity.fidelity < 1.0,
            "Cut circuit should have fidelity < 1.0"
        );
        assert!(fidelity.cut_gates >= 1, "Should detect at least 1 cut gate");
    }
}
