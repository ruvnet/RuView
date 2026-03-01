//! QEC scheduling engine that minimizes classical round trips.
//!
//! The scheduler generates surface code syndrome extraction schedules and
//! optimizes them to minimize feed-forward latency -- the critical bottleneck
//! in fault-tolerant quantum computing where classical decoding results must
//! be fed back to the quantum processor before decoherence accumulates.
//!
//! # Key optimizations
//!
//! - **Deferred corrections**: Pauli frame tracking allows many corrections
//!   to be tracked classically rather than applied physically, eliminating
//!   the associated feed-forward latency.
//! - **Batch merging**: Consecutive correction rounds that share no
//!   data dependencies are merged into single rounds.
//! - **Critical path minimization**: The dependency graph is analyzed
//!   to push feed-forward decisions as late as possible.
//!
//! # Architecture
//!
//! ```text
//! Syndrome Extraction -> Decoder -> Correction Scheduler
//!       |                   |              |
//!       v                   v              v
//!   QecRound          DependencyGraph  Optimized Schedule
//! ```

use crate::decoder::PauliType;

// ---------------------------------------------------------------------------
// Schedule data types
// ---------------------------------------------------------------------------

/// Type of stabilizer being measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StabilizerType {
    /// X-type stabilizer (detects Z errors).
    XStabilizer,
    /// Z-type stabilizer (detects X errors).
    ZStabilizer,
}

/// A single syndrome extraction operation.
///
/// Measures one stabilizer by entangling an ancilla qubit with
/// the data qubits in the stabilizer's support.
#[derive(Debug, Clone)]
pub struct SyndromeExtraction {
    /// Type of stabilizer being measured.
    pub stabilizer_type: StabilizerType,
    /// Data qubit indices in the stabilizer's support.
    pub data_qubits: Vec<u32>,
    /// Ancilla qubit used for indirect measurement.
    pub ancilla_qubit: u32,
}

/// A correction scheduled for application.
#[derive(Debug, Clone)]
pub struct ScheduledCorrection {
    /// Target data qubit to correct.
    pub target_qubit: u32,
    /// Type of Pauli correction.
    pub correction_type: PauliType,
    /// If `Some(round)`, this correction depends on the decoder result
    /// from the given round (feed-forward). If `None`, the correction
    /// can be deferred to the Pauli frame.
    pub depends_on_round: Option<usize>,
}

/// A single round of QEC operations.
#[derive(Debug, Clone)]
pub struct QecRound {
    /// Syndrome extraction operations in this round.
    pub syndrome_extractions: Vec<SyndromeExtraction>,
    /// Corrections to apply after this round.
    pub corrections: Vec<ScheduledCorrection>,
    /// Whether this round requires a feed-forward decision
    /// (classical decode result needed before continuing).
    pub is_feed_forward: bool,
}

/// A complete QEC schedule.
#[derive(Debug, Clone)]
pub struct QecSchedule {
    /// Ordered list of QEC rounds.
    pub rounds: Vec<QecRound>,
    /// Total classical processing depth (number of decode cycles).
    pub total_classical_depth: u32,
    /// Total quantum circuit depth.
    pub total_quantum_depth: u32,
    /// Indices of rounds that are feed-forward points
    /// (where classical results must be available).
    pub feed_forward_points: Vec<usize>,
}

// ---------------------------------------------------------------------------
// Schedule generation
// ---------------------------------------------------------------------------

/// Generate the standard surface code syndrome extraction schedule.
///
/// For a distance-d surface code:
/// - The data qubits are arranged in a d x d grid.
/// - X stabilizers act on plaquettes (4 data qubits each, except at boundaries).
/// - Z stabilizers act on vertices (4 data qubits each, except at boundaries).
/// - Ancilla qubits are interleaved between data qubits.
///
/// The schedule interleaves X and Z stabilizer extractions to minimize
/// ancilla reuse conflicts.
///
/// Each round consists of:
/// 1. X-stabilizer syndrome extraction
/// 2. Z-stabilizer syndrome extraction
/// 3. A placeholder correction round (populated during optimization)
pub fn generate_surface_code_schedule(distance: u32, num_rounds: u32) -> QecSchedule {
    let d = distance;
    let mut rounds = Vec::with_capacity(num_rounds as usize);
    let mut feed_forward_points = Vec::new();

    // Ancilla numbering: data qubits [0, d*d), ancillas [d*d, ...).
    let data_qubit_count = d * d;
    let mut next_ancilla = data_qubit_count;

    // Pre-compute stabilizer definitions.
    let x_stabilizers = generate_x_stabilizers(d, &mut next_ancilla);
    let z_stabilizers = generate_z_stabilizers(d, &mut next_ancilla);

    for round_idx in 0..num_rounds {
        // Syndrome extraction round: interleave X and Z.
        let mut extractions = Vec::new();

        // X stabilizers first (CNOT fan-out pattern).
        for stab in &x_stabilizers {
            extractions.push(stab.clone());
        }

        // Z stabilizers second (CNOT fan-in pattern).
        for stab in &z_stabilizers {
            extractions.push(stab.clone());
        }

        // Each round is initially marked as feed-forward.
        // The optimizer will remove unnecessary feed-forward points.
        let is_ff = true;
        if is_ff {
            feed_forward_points.push(round_idx as usize);
        }

        rounds.push(QecRound {
            syndrome_extractions: extractions,
            corrections: Vec::new(),
            is_feed_forward: is_ff,
        });
    }

    // Add placeholder corrections to each round.
    // In a real system, these would be populated by the decoder.
    for (i, round) in rounds.iter_mut().enumerate() {
        // Each round can potentially correct any data qubit.
        // We add dependency metadata for the optimizer.
        for q in 0..data_qubit_count {
            round.corrections.push(ScheduledCorrection {
                target_qubit: q,
                correction_type: PauliType::X,
                depends_on_round: Some(i),
            });
        }
    }

    let total_classical_depth = num_rounds;
    let total_quantum_depth = compute_quantum_depth(&rounds, d);

    QecSchedule {
        rounds,
        total_classical_depth,
        total_quantum_depth,
        feed_forward_points,
    }
}

/// Generate X-type stabilizer definitions for a distance-d code.
///
/// X stabilizers are plaquette operators on the surface code lattice.
/// For a d x d lattice, X stabilizers cover the "faces" of the lattice.
/// There are (d-1) * (d-1) / 2 + boundary stabilizers.
fn generate_x_stabilizers(d: u32, next_ancilla: &mut u32) -> Vec<SyndromeExtraction> {
    let mut stabilizers = Vec::new();

    if d < 2 {
        return stabilizers;
    }

    // X stabilizers are on the "even" plaquettes of a checkerboard pattern.
    // Each X stabilizer measures the product of X operators on neighboring
    // data qubits (typically 4 in the bulk, 2 at boundaries).
    for row in 0..(d - 1) {
        for col in 0..(d - 1) {
            // Checkerboard: X stabilizers on even (row+col) parity.
            if (row + col) % 2 != 0 {
                continue;
            }

            let mut data_qubits = Vec::new();

            // Top-left data qubit.
            data_qubits.push(row * d + col);
            // Top-right data qubit.
            data_qubits.push(row * d + col + 1);
            // Bottom-left data qubit.
            data_qubits.push((row + 1) * d + col);
            // Bottom-right data qubit.
            data_qubits.push((row + 1) * d + col + 1);

            let ancilla = *next_ancilla;
            *next_ancilla += 1;

            stabilizers.push(SyndromeExtraction {
                stabilizer_type: StabilizerType::XStabilizer,
                data_qubits,
                ancilla_qubit: ancilla,
            });
        }
    }

    // Boundary stabilizers (weight-2) on the left and right edges.
    // Left boundary: column 0, odd rows.
    for row in (0..(d - 1)).step_by(2) {
        if (row) % 2 == 0 && d > 2 {
            // Only add if not already covered by bulk stabilizers.
            // Skip: these are covered by the checkerboard above if d > 2.
            continue;
        }
        let mut data_qubits = Vec::new();
        data_qubits.push(row * d);
        data_qubits.push((row + 1) * d);

        let ancilla = *next_ancilla;
        *next_ancilla += 1;

        stabilizers.push(SyndromeExtraction {
            stabilizer_type: StabilizerType::XStabilizer,
            data_qubits,
            ancilla_qubit: ancilla,
        });
    }

    stabilizers
}

/// Generate Z-type stabilizer definitions for a distance-d code.
///
/// Z stabilizers are vertex operators on the surface code lattice.
/// For a d x d lattice, Z stabilizers cover the "vertices" between faces.
fn generate_z_stabilizers(d: u32, next_ancilla: &mut u32) -> Vec<SyndromeExtraction> {
    let mut stabilizers = Vec::new();

    if d < 2 {
        return stabilizers;
    }

    // Z stabilizers are on the "odd" plaquettes of a checkerboard pattern.
    for row in 0..(d - 1) {
        for col in 0..(d - 1) {
            if (row + col) % 2 != 1 {
                continue;
            }

            let mut data_qubits = Vec::new();

            data_qubits.push(row * d + col);
            data_qubits.push(row * d + col + 1);
            data_qubits.push((row + 1) * d + col);
            data_qubits.push((row + 1) * d + col + 1);

            let ancilla = *next_ancilla;
            *next_ancilla += 1;

            stabilizers.push(SyndromeExtraction {
                stabilizer_type: StabilizerType::ZStabilizer,
                data_qubits,
                ancilla_qubit: ancilla,
            });
        }
    }

    // Top boundary Z stabilizers (weight-2).
    for col in (1..(d - 1)).step_by(2) {
        if d <= 2 {
            break;
        }
        let mut data_qubits = Vec::new();
        data_qubits.push(col);
        data_qubits.push(col + 1);

        let ancilla = *next_ancilla;
        *next_ancilla += 1;

        stabilizers.push(SyndromeExtraction {
            stabilizer_type: StabilizerType::ZStabilizer,
            data_qubits,
            ancilla_qubit: ancilla,
        });
    }

    stabilizers
}

/// Compute the quantum circuit depth for the given schedule rounds.
///
/// Each stabilizer extraction requires 4 CNOT gates (or 2 for boundary
/// stabilizers) plus preparation and measurement, contributing ~6 gate
/// layers per extraction. Extractions that share no data qubits can
/// run in parallel.
fn compute_quantum_depth(rounds: &[QecRound], distance: u32) -> u32 {
    let mut depth = 0u32;

    for round in rounds {
        if round.syndrome_extractions.is_empty() {
            continue;
        }

        // Count parallel layers by checking qubit conflicts.
        let mut layers = 0u32;
        let mut scheduled = vec![false; round.syndrome_extractions.len()];
        let total = round.syndrome_extractions.len();
        let mut done = 0;

        while done < total {
            let mut used_qubits: Vec<u32> = Vec::new();
            for (i, ext) in round.syndrome_extractions.iter().enumerate() {
                if scheduled[i] {
                    continue;
                }
                let conflicts = ext.data_qubits.iter().any(|q| used_qubits.contains(q))
                    || used_qubits.contains(&ext.ancilla_qubit);

                if !conflicts {
                    used_qubits.extend(&ext.data_qubits);
                    used_qubits.push(ext.ancilla_qubit);
                    scheduled[i] = true;
                    done += 1;
                }
            }
            // Each extraction takes ~6 gate layers (prep, 4 CNOTs, measure).
            layers += 6;
        }

        depth += layers;

        // Corrections add 1 layer each (single Pauli gates in parallel).
        if !round.corrections.is_empty() {
            depth += 1;
        }
    }

    depth.max(distance) // At minimum, depth equals the code distance.
}

// ---------------------------------------------------------------------------
// Feed-forward optimization
// ---------------------------------------------------------------------------

/// Optimize a QEC schedule to minimize feed-forward latency.
///
/// This optimization pass performs three transformations:
///
/// 1. **Pauli frame deferral**: Corrections that commute with subsequent
///    operations are deferred to the Pauli frame (tracked classically)
///    and removed from the physical schedule.
///
/// 2. **Round merging**: Consecutive rounds whose corrections have no
///    inter-round data dependencies are merged into single rounds.
///
/// 3. **Feed-forward postponement**: Feed-forward decision points are
///    pushed as late as possible in the schedule, maximizing the time
///    available for classical decoding.
pub fn optimize_feed_forward(schedule: &QecSchedule) -> QecSchedule {
    let mut optimized_rounds = schedule.rounds.clone();

    // Pass 1: Defer corrections that do not block subsequent rounds.
    // A correction is deferrable if no later syndrome extraction
    // in the same or next round acts on the same data qubit with
    // a non-commuting stabilizer type.
    defer_corrections(&mut optimized_rounds);

    // Pass 2: Merge consecutive non-feed-forward rounds.
    let merged_rounds = merge_rounds(&optimized_rounds);

    // Pass 3: Minimize feed-forward points.
    let (final_rounds, ff_points) = minimize_feed_forward(&merged_rounds);

    let total_classical_depth = ff_points.len() as u32;
    let total_quantum_depth = compute_quantum_depth(&final_rounds, 0);

    QecSchedule {
        rounds: final_rounds,
        total_classical_depth,
        total_quantum_depth,
        feed_forward_points: ff_points,
    }
}

/// Defer corrections to the Pauli frame where possible.
///
/// A Pauli correction commutes with Clifford gates, so we can track
/// it classically (in the Pauli frame) instead of applying it physically.
/// The only corrections that must be applied physically are those that
/// affect a non-Clifford gate or a measurement in a later round.
///
/// For the surface code (which is all-Clifford), almost all corrections
/// can be deferred except those immediately before a logical measurement.
fn defer_corrections(rounds: &mut [QecRound]) {
    let num_rounds = rounds.len();
    if num_rounds == 0 {
        return;
    }

    for i in 0..num_rounds {
        let mut deferred_indices = Vec::new();

        // Check each correction in round i.
        for (ci, corr) in rounds[i].corrections.iter().enumerate() {
            let qubit = corr.target_qubit;

            // Check if the next round uses this qubit in a syndrome extraction.
            let blocks_next_round = if i + 1 < num_rounds {
                rounds[i + 1].syndrome_extractions.iter().any(|ext| {
                    ext.data_qubits.contains(&qubit)
                        && !commutes_with_correction(&ext.stabilizer_type, &corr.correction_type)
                })
            } else {
                // Last round: correction must be applied for logical readout.
                true
            };

            if !blocks_next_round {
                deferred_indices.push(ci);
            }
        }

        // Mark deferred corrections by removing their round dependency.
        for &ci in deferred_indices.iter().rev() {
            rounds[i].corrections[ci].depends_on_round = None;
        }

        // Remove fully deferred corrections from the physical schedule.
        let mut kept = Vec::new();
        for (ci, corr) in rounds[i].corrections.iter().enumerate() {
            if !deferred_indices.contains(&ci) {
                kept.push(corr.clone());
            }
        }
        rounds[i].corrections = kept;
    }
}

/// Check whether a stabilizer type commutes with a Pauli correction type.
///
/// X stabilizers commute with X corrections; Z stabilizers commute with Z corrections.
/// Other combinations anticommute and require physical application.
fn commutes_with_correction(stab: &StabilizerType, pauli: &PauliType) -> bool {
    match (stab, pauli) {
        (StabilizerType::XStabilizer, PauliType::X) => true,
        (StabilizerType::ZStabilizer, PauliType::Z) => true,
        _ => false,
    }
}

/// Merge consecutive rounds that have no inter-round dependencies.
///
/// Two rounds can be merged if:
/// - The second round has no feed-forward corrections.
/// - No data qubit is used by both a correction in the first round
///   and a syndrome extraction in the second round.
fn merge_rounds(rounds: &[QecRound]) -> Vec<QecRound> {
    if rounds.is_empty() {
        return Vec::new();
    }

    let mut merged = Vec::new();
    let mut current = rounds[0].clone();

    for next in rounds.iter().skip(1) {
        let can_merge = can_merge_rounds(&current, next);

        if can_merge {
            // Merge next into current.
            current
                .syndrome_extractions
                .extend(next.syndrome_extractions.iter().cloned());
            current.corrections.extend(next.corrections.iter().cloned());
            current.is_feed_forward = current.is_feed_forward || next.is_feed_forward;
        } else {
            merged.push(current);
            current = next.clone();
        }
    }
    merged.push(current);

    merged
}

/// Check whether two rounds can be safely merged.
fn can_merge_rounds(first: &QecRound, second: &QecRound) -> bool {
    // Cannot merge if second round has feed-forward dependencies.
    if second
        .corrections
        .iter()
        .any(|c| c.depends_on_round.is_some())
    {
        return false;
    }

    // Check for data qubit conflicts between first's corrections
    // and second's syndrome extractions.
    let corrected_qubits: Vec<u32> = first.corrections.iter().map(|c| c.target_qubit).collect();

    let extraction_qubits: Vec<u32> = second
        .syndrome_extractions
        .iter()
        .flat_map(|ext| ext.data_qubits.iter().copied())
        .collect();

    !corrected_qubits
        .iter()
        .any(|q| extraction_qubits.contains(q))
}

/// Minimize feed-forward points by pushing decisions as late as possible.
///
/// Returns the optimized rounds and the indices of remaining feed-forward points.
fn minimize_feed_forward(rounds: &[QecRound]) -> (Vec<QecRound>, Vec<usize>) {
    let mut result = rounds.to_vec();
    let mut ff_points = Vec::new();

    for (i, round) in result.iter_mut().enumerate() {
        // A round is only a true feed-forward point if it has corrections
        // that depend on decoder results AND the next operation requires them.
        let has_dependent_corrections = round
            .corrections
            .iter()
            .any(|c| c.depends_on_round.is_some());

        if has_dependent_corrections {
            round.is_feed_forward = true;
            ff_points.push(i);
        } else {
            round.is_feed_forward = false;
        }
    }

    (result, ff_points)
}

// ---------------------------------------------------------------------------
// Latency estimation
// ---------------------------------------------------------------------------

/// Estimate the total schedule latency in nanoseconds.
///
/// - `gate_time_ns`: Time for a single quantum gate (typically 20-100ns).
/// - `classical_time_ns`: Time for one classical decode cycle (typically 500-1000ns).
///
/// The total latency is:
///   sum over rounds of (extraction_depth * gate_time + correction_time)
///   + feed_forward_points * classical_time
pub fn schedule_latency(schedule: &QecSchedule, gate_time_ns: u64, classical_time_ns: u64) -> u64 {
    let quantum_latency = schedule.total_quantum_depth as u64 * gate_time_ns;
    let classical_latency = schedule.feed_forward_points.len() as u64 * classical_time_ns;

    quantum_latency + classical_latency
}

// ---------------------------------------------------------------------------
// Dependency graph
// ---------------------------------------------------------------------------

/// Type of operation in the dependency graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    /// Syndrome extraction (quantum operation).
    SyndromeExtract,
    /// Classical decoding.
    Decode,
    /// Correction application.
    Correct,
}

/// A node in the dependency graph.
#[derive(Debug, Clone)]
pub struct DependencyNode {
    /// The QEC round this node belongs to.
    pub round: usize,
    /// The type of operation.
    pub operation: OperationType,
}

/// Directed acyclic dependency graph for a QEC schedule.
///
/// Edges represent "must happen before" relationships.
/// The critical path through this graph determines the minimum
/// possible latency.
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// Nodes in the dependency graph.
    pub nodes: Vec<DependencyNode>,
    /// Directed edges: (from, to) meaning `from` must complete before `to`.
    pub edges: Vec<(usize, usize)>,
}

/// Build the dependency graph for a QEC schedule.
///
/// For each round, the graph contains three nodes:
/// 1. SyndromeExtract -- depends on previous round's Correct (if any)
/// 2. Decode -- depends on SyndromeExtract
/// 3. Correct -- depends on Decode (if feed-forward) or can be deferred
///
/// Cross-round dependencies exist when corrections must be applied
/// before the next round's syndrome extraction.
pub fn build_dependency_graph(schedule: &QecSchedule) -> DependencyGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let num_rounds = schedule.rounds.len();

    for (i, round) in schedule.rounds.iter().enumerate() {
        let base = i * 3;

        // Node 0: Syndrome extraction.
        nodes.push(DependencyNode {
            round: i,
            operation: OperationType::SyndromeExtract,
        });

        // Node 1: Decode.
        nodes.push(DependencyNode {
            round: i,
            operation: OperationType::Decode,
        });

        // Node 2: Correct.
        nodes.push(DependencyNode {
            round: i,
            operation: OperationType::Correct,
        });

        // Intra-round edges.
        // Extract -> Decode (always).
        edges.push((base, base + 1));

        // Decode -> Correct (if feed-forward).
        if round.is_feed_forward {
            edges.push((base + 1, base + 2));
        }

        // Cross-round edges.
        if i > 0 {
            let prev_base = (i - 1) * 3;
            // Previous Correct -> Current Extract.
            edges.push((prev_base + 2, base));
        }
    }

    // Final round: add dependency from last Correct to ensure
    // it's on the critical path if feed-forward.
    if num_rounds > 0 {
        let last = (num_rounds - 1) * 3;
        // Ensure Decode -> Correct for the last round always.
        if !edges.contains(&(last + 1, last + 2)) {
            edges.push((last + 1, last + 2));
        }
    }

    DependencyGraph { nodes, edges }
}

/// Compute the critical path length through the dependency graph.
///
/// Uses topological sort followed by longest-path computation
/// (DAG longest path in O(V + E)).
///
/// Returns the number of nodes on the critical path.
pub fn critical_path_length(graph: &DependencyGraph) -> usize {
    let n = graph.nodes.len();
    if n == 0 {
        return 0;
    }

    // Build adjacency list.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut in_degree = vec![0usize; n];

    for &(from, to) in &graph.edges {
        if from < n && to < n {
            adj[from].push(to);
            in_degree[to] += 1;
        }
    }

    // Topological sort using Kahn's algorithm.
    let mut queue: Vec<usize> = Vec::new();
    for i in 0..n {
        if in_degree[i] == 0 {
            queue.push(i);
        }
    }

    let mut topo_order = Vec::with_capacity(n);
    let mut head = 0;

    while head < queue.len() {
        let u = queue[head];
        head += 1;
        topo_order.push(u);

        for &v in &adj[u] {
            in_degree[v] -= 1;
            if in_degree[v] == 0 {
                queue.push(v);
            }
        }
    }

    // Longest path in the DAG.
    let mut dist = vec![1usize; n]; // Each node has weight 1.

    for &u in &topo_order {
        for &v in &adj[u] {
            if dist[v] < dist[u] + 1 {
                dist[v] = dist[u] + 1;
            }
        }
    }

    dist.into_iter().max().unwrap_or(0)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- StabilizerType --

    #[test]
    fn test_stabilizer_type_equality() {
        assert_eq!(StabilizerType::XStabilizer, StabilizerType::XStabilizer);
        assert_ne!(StabilizerType::XStabilizer, StabilizerType::ZStabilizer);
    }

    // -- SyndromeExtraction --

    #[test]
    fn test_syndrome_extraction_creation() {
        let ext = SyndromeExtraction {
            stabilizer_type: StabilizerType::XStabilizer,
            data_qubits: vec![0, 1, 3, 4],
            ancilla_qubit: 25,
        };
        assert_eq!(ext.stabilizer_type, StabilizerType::XStabilizer);
        assert_eq!(ext.data_qubits.len(), 4);
        assert_eq!(ext.ancilla_qubit, 25);
    }

    // -- ScheduledCorrection --

    #[test]
    fn test_scheduled_correction_with_dependency() {
        let corr = ScheduledCorrection {
            target_qubit: 5,
            correction_type: PauliType::X,
            depends_on_round: Some(2),
        };
        assert_eq!(corr.target_qubit, 5);
        assert_eq!(corr.correction_type, PauliType::X);
        assert_eq!(corr.depends_on_round, Some(2));
    }

    #[test]
    fn test_scheduled_correction_deferred() {
        let corr = ScheduledCorrection {
            target_qubit: 3,
            correction_type: PauliType::Z,
            depends_on_round: None,
        };
        assert!(corr.depends_on_round.is_none());
    }

    // -- QecRound --

    #[test]
    fn test_qec_round_empty() {
        let round = QecRound {
            syndrome_extractions: Vec::new(),
            corrections: Vec::new(),
            is_feed_forward: false,
        };
        assert!(round.syndrome_extractions.is_empty());
        assert!(!round.is_feed_forward);
    }

    // -- QecSchedule --

    #[test]
    fn test_qec_schedule_creation() {
        let schedule = QecSchedule {
            rounds: Vec::new(),
            total_classical_depth: 0,
            total_quantum_depth: 0,
            feed_forward_points: Vec::new(),
        };
        assert!(schedule.rounds.is_empty());
        assert_eq!(schedule.total_classical_depth, 0);
    }

    // -- generate_surface_code_schedule --

    #[test]
    fn test_generate_schedule_d3() {
        let schedule = generate_surface_code_schedule(3, 5);
        assert_eq!(schedule.rounds.len(), 5);
        assert_eq!(schedule.total_classical_depth, 5);

        // Each round should have syndrome extractions.
        for round in &schedule.rounds {
            assert!(
                !round.syndrome_extractions.is_empty(),
                "Each round should have syndrome extractions"
            );
        }
    }

    #[test]
    fn test_generate_schedule_d5() {
        let schedule = generate_surface_code_schedule(5, 3);
        assert_eq!(schedule.rounds.len(), 3);

        // d=5: should have both X and Z stabilizers.
        let first_round = &schedule.rounds[0];
        let has_x = first_round
            .syndrome_extractions
            .iter()
            .any(|e| e.stabilizer_type == StabilizerType::XStabilizer);
        let has_z = first_round
            .syndrome_extractions
            .iter()
            .any(|e| e.stabilizer_type == StabilizerType::ZStabilizer);
        assert!(has_x, "Should have X stabilizers");
        assert!(has_z, "Should have Z stabilizers");
    }

    #[test]
    fn test_generate_schedule_single_round() {
        let schedule = generate_surface_code_schedule(3, 1);
        assert_eq!(schedule.rounds.len(), 1);
        assert_eq!(schedule.feed_forward_points.len(), 1);
    }

    #[test]
    fn test_generate_schedule_zero_rounds() {
        let schedule = generate_surface_code_schedule(3, 0);
        assert!(schedule.rounds.is_empty());
        assert!(schedule.feed_forward_points.is_empty());
    }

    #[test]
    fn test_generate_schedule_d1() {
        // Distance 1 is degenerate but should not panic.
        let schedule = generate_surface_code_schedule(1, 2);
        assert_eq!(schedule.rounds.len(), 2);
    }

    #[test]
    fn test_generate_schedule_stabilizer_coverage() {
        // For d=3, data qubits are [0..9], ancillas start at 9.
        let schedule = generate_surface_code_schedule(3, 1);
        let round = &schedule.rounds[0];

        for ext in &round.syndrome_extractions {
            // Ancilla should be >= d*d.
            assert!(
                ext.ancilla_qubit >= 9,
                "Ancilla {} should be >= 9",
                ext.ancilla_qubit
            );
            // Data qubits should be < d*d.
            for &q in &ext.data_qubits {
                assert!(q < 9, "Data qubit {} should be < 9", q);
            }
        }
    }

    #[test]
    fn test_generate_schedule_all_rounds_have_corrections() {
        let schedule = generate_surface_code_schedule(3, 3);
        for round in &schedule.rounds {
            assert!(
                !round.corrections.is_empty(),
                "Each round should have correction placeholders"
            );
        }
    }

    // -- Stabilizer generators --

    #[test]
    fn test_x_stabilizers_d3() {
        let mut next = 9;
        let x_stabs = generate_x_stabilizers(3, &mut next);
        assert!(!x_stabs.is_empty());
        for s in &x_stabs {
            assert_eq!(s.stabilizer_type, StabilizerType::XStabilizer);
            assert!(!s.data_qubits.is_empty());
        }
    }

    #[test]
    fn test_z_stabilizers_d3() {
        let mut next = 9;
        let z_stabs = generate_z_stabilizers(3, &mut next);
        assert!(!z_stabs.is_empty());
        for s in &z_stabs {
            assert_eq!(s.stabilizer_type, StabilizerType::ZStabilizer);
        }
    }

    #[test]
    fn test_x_stabilizers_d1() {
        let mut next = 1;
        let x_stabs = generate_x_stabilizers(1, &mut next);
        assert!(x_stabs.is_empty(), "d=1 should have no X stabilizers");
    }

    #[test]
    fn test_z_stabilizers_d1() {
        let mut next = 1;
        let z_stabs = generate_z_stabilizers(1, &mut next);
        assert!(z_stabs.is_empty(), "d=1 should have no Z stabilizers");
    }

    #[test]
    fn test_stabilizer_ancillas_unique() {
        let mut next = 25; // d=5
        let x_stabs = generate_x_stabilizers(5, &mut next);
        let z_stabs = generate_z_stabilizers(5, &mut next);

        let all_ancillas: Vec<u32> = x_stabs
            .iter()
            .chain(z_stabs.iter())
            .map(|s| s.ancilla_qubit)
            .collect();

        // All ancilla qubits should be unique.
        let mut unique = all_ancillas.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            all_ancillas.len(),
            unique.len(),
            "Ancilla qubits must be unique"
        );
    }

    // -- commutes_with_correction --

    #[test]
    fn test_commutation() {
        assert!(commutes_with_correction(
            &StabilizerType::XStabilizer,
            &PauliType::X
        ));
        assert!(commutes_with_correction(
            &StabilizerType::ZStabilizer,
            &PauliType::Z
        ));
        assert!(!commutes_with_correction(
            &StabilizerType::XStabilizer,
            &PauliType::Z
        ));
        assert!(!commutes_with_correction(
            &StabilizerType::ZStabilizer,
            &PauliType::X
        ));
    }

    // -- optimize_feed_forward --

    #[test]
    fn test_optimize_reduces_feed_forward() {
        let schedule = generate_surface_code_schedule(3, 5);
        let original_ff = schedule.feed_forward_points.len();

        let optimized = optimize_feed_forward(&schedule);

        // Optimization should not increase feed-forward points.
        assert!(
            optimized.feed_forward_points.len() <= original_ff,
            "Optimization should reduce or maintain FF points: {} <= {}",
            optimized.feed_forward_points.len(),
            original_ff
        );
    }

    #[test]
    fn test_optimize_preserves_round_structure() {
        let schedule = generate_surface_code_schedule(3, 3);
        let optimized = optimize_feed_forward(&schedule);

        // Optimized schedule should still have rounds.
        assert!(!optimized.rounds.is_empty());
    }

    #[test]
    fn test_optimize_empty_schedule() {
        let schedule = QecSchedule {
            rounds: Vec::new(),
            total_classical_depth: 0,
            total_quantum_depth: 0,
            feed_forward_points: Vec::new(),
        };
        let optimized = optimize_feed_forward(&schedule);
        assert!(optimized.rounds.is_empty());
        assert!(optimized.feed_forward_points.is_empty());
    }

    #[test]
    fn test_optimize_single_round() {
        let schedule = generate_surface_code_schedule(3, 1);
        let optimized = optimize_feed_forward(&schedule);
        assert!(!optimized.rounds.is_empty());
    }

    #[test]
    fn test_optimize_classical_depth_decreases() {
        let schedule = generate_surface_code_schedule(5, 10);
        let optimized = optimize_feed_forward(&schedule);
        assert!(
            optimized.total_classical_depth <= schedule.total_classical_depth,
            "Classical depth should decrease: {} <= {}",
            optimized.total_classical_depth,
            schedule.total_classical_depth
        );
    }

    // -- merge_rounds --

    #[test]
    fn test_merge_rounds_no_conflicts() {
        let rounds = vec![
            QecRound {
                syndrome_extractions: vec![SyndromeExtraction {
                    stabilizer_type: StabilizerType::XStabilizer,
                    data_qubits: vec![0, 1],
                    ancilla_qubit: 100,
                }],
                corrections: Vec::new(),
                is_feed_forward: false,
            },
            QecRound {
                syndrome_extractions: vec![SyndromeExtraction {
                    stabilizer_type: StabilizerType::ZStabilizer,
                    data_qubits: vec![2, 3],
                    ancilla_qubit: 101,
                }],
                corrections: Vec::new(),
                is_feed_forward: false,
            },
        ];
        let merged = merge_rounds(&rounds);
        // Rounds with no conflicts and no dependent corrections should merge.
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].syndrome_extractions.len(), 2);
    }

    #[test]
    fn test_merge_rounds_with_conflicts() {
        let rounds = vec![
            QecRound {
                syndrome_extractions: vec![SyndromeExtraction {
                    stabilizer_type: StabilizerType::XStabilizer,
                    data_qubits: vec![0, 1],
                    ancilla_qubit: 100,
                }],
                corrections: vec![ScheduledCorrection {
                    target_qubit: 2,
                    correction_type: PauliType::X,
                    depends_on_round: Some(0),
                }],
                is_feed_forward: true,
            },
            QecRound {
                syndrome_extractions: vec![SyndromeExtraction {
                    stabilizer_type: StabilizerType::ZStabilizer,
                    data_qubits: vec![2, 3],
                    ancilla_qubit: 101,
                }],
                corrections: vec![ScheduledCorrection {
                    target_qubit: 3,
                    correction_type: PauliType::Z,
                    depends_on_round: Some(1),
                }],
                is_feed_forward: true,
            },
        ];
        let merged = merge_rounds(&rounds);
        // Rounds with conflicting dependencies should not merge.
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_merge_empty_rounds() {
        let merged = merge_rounds(&[]);
        assert!(merged.is_empty());
    }

    // -- schedule_latency --

    #[test]
    fn test_schedule_latency_basic() {
        let schedule = QecSchedule {
            rounds: Vec::new(),
            total_classical_depth: 2,
            total_quantum_depth: 10,
            feed_forward_points: vec![0, 1],
        };
        let latency = schedule_latency(&schedule, 50, 1000);
        // 10 * 50 + 2 * 1000 = 500 + 2000 = 2500
        assert_eq!(latency, 2500);
    }

    #[test]
    fn test_schedule_latency_no_feed_forward() {
        let schedule = QecSchedule {
            rounds: Vec::new(),
            total_classical_depth: 0,
            total_quantum_depth: 20,
            feed_forward_points: Vec::new(),
        };
        let latency = schedule_latency(&schedule, 100, 500);
        assert_eq!(latency, 2000);
    }

    #[test]
    fn test_schedule_latency_zero_times() {
        let schedule = QecSchedule {
            rounds: Vec::new(),
            total_classical_depth: 5,
            total_quantum_depth: 100,
            feed_forward_points: vec![0, 1, 2, 3, 4],
        };
        let latency = schedule_latency(&schedule, 0, 0);
        assert_eq!(latency, 0);
    }

    #[test]
    fn test_schedule_latency_optimized_is_less() {
        let schedule = generate_surface_code_schedule(3, 5);
        let optimized = optimize_feed_forward(&schedule);

        let lat_orig = schedule_latency(&schedule, 50, 1000);
        let lat_opt = schedule_latency(&optimized, 50, 1000);

        assert!(
            lat_opt <= lat_orig,
            "Optimized latency should be <= original: {} <= {}",
            lat_opt,
            lat_orig
        );
    }

    // -- DependencyGraph --

    #[test]
    fn test_build_dependency_graph_empty() {
        let schedule = QecSchedule {
            rounds: Vec::new(),
            total_classical_depth: 0,
            total_quantum_depth: 0,
            feed_forward_points: Vec::new(),
        };
        let graph = build_dependency_graph(&schedule);
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_build_dependency_graph_single_round() {
        let schedule = QecSchedule {
            rounds: vec![QecRound {
                syndrome_extractions: vec![SyndromeExtraction {
                    stabilizer_type: StabilizerType::XStabilizer,
                    data_qubits: vec![0, 1],
                    ancilla_qubit: 10,
                }],
                corrections: Vec::new(),
                is_feed_forward: true,
            }],
            total_classical_depth: 1,
            total_quantum_depth: 6,
            feed_forward_points: vec![0],
        };
        let graph = build_dependency_graph(&schedule);
        assert_eq!(graph.nodes.len(), 3);
        // Extract -> Decode, Decode -> Correct.
        assert!(graph.edges.contains(&(0, 1)));
        assert!(graph.edges.contains(&(1, 2)));
    }

    #[test]
    fn test_build_dependency_graph_two_rounds() {
        let schedule = generate_surface_code_schedule(3, 2);
        let graph = build_dependency_graph(&schedule);
        assert_eq!(graph.nodes.len(), 6); // 2 rounds * 3 nodes
                                          // Cross-round edge: round 0 Correct -> round 1 Extract.
        assert!(graph.edges.contains(&(2, 3)));
    }

    #[test]
    fn test_dependency_node_types() {
        let schedule = generate_surface_code_schedule(3, 1);
        let graph = build_dependency_graph(&schedule);
        assert_eq!(graph.nodes[0].operation, OperationType::SyndromeExtract);
        assert_eq!(graph.nodes[1].operation, OperationType::Decode);
        assert_eq!(graph.nodes[2].operation, OperationType::Correct);
    }

    // -- critical_path_length --

    #[test]
    fn test_critical_path_empty_graph() {
        let graph = DependencyGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        assert_eq!(critical_path_length(&graph), 0);
    }

    #[test]
    fn test_critical_path_single_node() {
        let graph = DependencyGraph {
            nodes: vec![DependencyNode {
                round: 0,
                operation: OperationType::SyndromeExtract,
            }],
            edges: Vec::new(),
        };
        assert_eq!(critical_path_length(&graph), 1);
    }

    #[test]
    fn test_critical_path_linear_chain() {
        let graph = DependencyGraph {
            nodes: vec![
                DependencyNode {
                    round: 0,
                    operation: OperationType::SyndromeExtract,
                },
                DependencyNode {
                    round: 0,
                    operation: OperationType::Decode,
                },
                DependencyNode {
                    round: 0,
                    operation: OperationType::Correct,
                },
            ],
            edges: vec![(0, 1), (1, 2)],
        };
        assert_eq!(critical_path_length(&graph), 3);
    }

    #[test]
    fn test_critical_path_parallel() {
        // Two independent chains of length 2.
        let graph = DependencyGraph {
            nodes: vec![
                DependencyNode {
                    round: 0,
                    operation: OperationType::SyndromeExtract,
                },
                DependencyNode {
                    round: 0,
                    operation: OperationType::Decode,
                },
                DependencyNode {
                    round: 1,
                    operation: OperationType::SyndromeExtract,
                },
                DependencyNode {
                    round: 1,
                    operation: OperationType::Decode,
                },
            ],
            edges: vec![(0, 1), (2, 3)],
        };
        assert_eq!(critical_path_length(&graph), 2);
    }

    #[test]
    fn test_critical_path_two_round_schedule() {
        let schedule = generate_surface_code_schedule(3, 2);
        let graph = build_dependency_graph(&schedule);
        let cp = critical_path_length(&graph);
        // 2 rounds with full dependency chain: should be 6
        // (Extract->Decode->Correct -> Extract->Decode->Correct).
        assert_eq!(cp, 6);
    }

    #[test]
    fn test_critical_path_five_round_schedule() {
        let schedule = generate_surface_code_schedule(3, 5);
        let graph = build_dependency_graph(&schedule);
        let cp = critical_path_length(&graph);
        // 5 rounds with full dependency chain: 15 nodes on critical path.
        assert_eq!(cp, 15);
    }

    // -- Integration tests --

    #[test]
    fn test_full_pipeline_d3() {
        // Generate -> optimize -> build graph -> measure.
        let schedule = generate_surface_code_schedule(3, 4);
        let optimized = optimize_feed_forward(&schedule);
        let graph = build_dependency_graph(&optimized);
        let cp = critical_path_length(&graph);

        assert!(cp > 0);
        assert!(optimized.total_classical_depth <= schedule.total_classical_depth);

        let lat = schedule_latency(&optimized, 50, 1000);
        assert!(lat > 0);
    }

    #[test]
    fn test_full_pipeline_d5() {
        let schedule = generate_surface_code_schedule(5, 10);
        let optimized = optimize_feed_forward(&schedule);
        let graph = build_dependency_graph(&optimized);
        let cp = critical_path_length(&graph);

        assert!(cp > 0);

        let lat_orig = schedule_latency(&schedule, 50, 1000);
        let lat_opt = schedule_latency(&optimized, 50, 1000);
        assert!(lat_opt <= lat_orig);
    }

    #[test]
    fn test_latency_scales_with_distance() {
        let lat_d3 = schedule_latency(&generate_surface_code_schedule(3, 5), 50, 1000);
        let lat_d5 = schedule_latency(&generate_surface_code_schedule(5, 5), 50, 1000);
        // Larger distance -> more stabilizers -> more quantum depth -> more latency.
        assert!(
            lat_d5 >= lat_d3,
            "Larger distance should have >= latency: d5={} >= d3={}",
            lat_d5,
            lat_d3
        );
    }

    #[test]
    fn test_latency_scales_with_rounds() {
        let lat_5 = schedule_latency(&generate_surface_code_schedule(3, 5), 50, 1000);
        let lat_10 = schedule_latency(&generate_surface_code_schedule(3, 10), 50, 1000);
        assert!(
            lat_10 >= lat_5,
            "More rounds should have >= latency: {} >= {}",
            lat_10,
            lat_5
        );
    }

    #[test]
    fn test_optimization_idempotent() {
        let schedule = generate_surface_code_schedule(3, 4);
        let opt1 = optimize_feed_forward(&schedule);
        let opt2 = optimize_feed_forward(&opt1);
        // Re-optimizing should not change the result significantly.
        assert_eq!(
            opt1.feed_forward_points.len(),
            opt2.feed_forward_points.len()
        );
    }

    #[test]
    fn test_dependency_graph_node_count() {
        for num_rounds in 1..=5 {
            let schedule = generate_surface_code_schedule(3, num_rounds);
            let graph = build_dependency_graph(&schedule);
            assert_eq!(
                graph.nodes.len(),
                (num_rounds as usize) * 3,
                "Should have 3 nodes per round"
            );
        }
    }

    #[test]
    fn test_can_merge_no_corrections() {
        let a = QecRound {
            syndrome_extractions: vec![],
            corrections: vec![],
            is_feed_forward: false,
        };
        let b = QecRound {
            syndrome_extractions: vec![],
            corrections: vec![],
            is_feed_forward: false,
        };
        assert!(can_merge_rounds(&a, &b));
    }

    #[test]
    fn test_cannot_merge_with_dependency() {
        let a = QecRound {
            syndrome_extractions: vec![],
            corrections: vec![],
            is_feed_forward: false,
        };
        let b = QecRound {
            syndrome_extractions: vec![],
            corrections: vec![ScheduledCorrection {
                target_qubit: 0,
                correction_type: PauliType::X,
                depends_on_round: Some(1),
            }],
            is_feed_forward: true,
        };
        assert!(!can_merge_rounds(&a, &b));
    }
}
