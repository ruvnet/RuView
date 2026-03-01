//! Noise-aware transpiler for quantum circuits.
//!
//! Decomposes arbitrary gates into hardware-native basis gate sets, routes
//! two-qubit gates onto constrained coupling topologies via SWAP insertion,
//! and applies peephole gate-cancellation optimizations.

use std::collections::VecDeque;

use crate::circuit::QuantumCircuit;
use crate::gate::Gate;

use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Hardware-native basis gate sets supported by the transpiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasisGateSet {
    /// IBM Eagle: CX, ID, RZ, SX (= Rx(pi/2)), X
    IbmEagle,
    /// IonQ Aria: GPI, GPI2, MS -- mapped to Rx, Ry, Rzz
    IonQAria,
    /// Rigetti Aspen: CZ, RX, RZ
    RigettiAspen,
    /// Universal: any gate passes through without decomposition.
    Universal,
}

/// Transpiler configuration.
#[derive(Debug, Clone)]
pub struct TranspilerConfig {
    /// Target basis gate set.
    pub basis: BasisGateSet,
    /// Optional coupling map describing which qubit pairs support two-qubit
    /// gates.  Edges are undirected -- `(a, b)` implies `(b, a)`.
    pub coupling_map: Option<Vec<(u32, u32)>>,
    /// Optimization level: 0 = none, 1 = inverse-pair cancellation,
    /// 2 = also merge adjacent Rz rotations.
    pub optimization_level: u8,
}

// ---------------------------------------------------------------------------
// Top-level entry point
// ---------------------------------------------------------------------------

/// Transpile a circuit through the full pipeline:
/// decompose -> route -> optimize.
pub fn transpile(circuit: &QuantumCircuit, config: &TranspilerConfig) -> QuantumCircuit {
    // Step 1: decompose to basis gate set
    let decomposed = decompose(circuit, config.basis);

    // Step 2: route onto coupling map (if provided)
    let routed = match &config.coupling_map {
        Some(map) => route_circuit(&decomposed, map),
        None => decomposed,
    };

    // Step 3: optimize
    optimize_gates(&routed, config.optimization_level)
}

// ---------------------------------------------------------------------------
// Decomposition dispatcher
// ---------------------------------------------------------------------------

fn decompose(circuit: &QuantumCircuit, basis: BasisGateSet) -> QuantumCircuit {
    if basis == BasisGateSet::Universal {
        return circuit.clone();
    }
    let mut result = QuantumCircuit::new(circuit.num_qubits());
    for gate in circuit.gates() {
        let decomposed = match basis {
            BasisGateSet::IbmEagle => decompose_to_ibm(gate),
            BasisGateSet::IonQAria => decompose_to_ionq(gate),
            BasisGateSet::RigettiAspen => decompose_to_rigetti(gate),
            BasisGateSet::Universal => unreachable!(),
        };
        for g in decomposed {
            result.add_gate(g);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// IBM Eagle decomposition: basis = {CNOT, Rz, SX (Rx(pi/2)), X}
// ---------------------------------------------------------------------------
//
// SX = Rx(pi/2).  The IBM ID gate is a no-op and never needs to be emitted.

/// Decompose a single gate into the IBM Eagle basis {CNOT, Rz, Rx(pi/2), X}.
///
/// The SX gate is represented as `Rx(q, PI/2)`.
pub fn decompose_to_ibm(gate: &Gate) -> Vec<Gate> {
    match gate {
        // --- already in basis ---
        Gate::CNOT(c, t) => vec![Gate::CNOT(*c, *t)],
        Gate::X(q) => vec![Gate::X(*q)],
        Gate::Rz(q, theta) => vec![Gate::Rz(*q, *theta)],

        // --- single-qubit Cliffords ---
        // H = Rz(pi) SX Rz(pi)
        Gate::H(q) => vec![
            Gate::Rz(*q, PI),
            Gate::Rx(*q, FRAC_PI_2), // SX
            Gate::Rz(*q, PI),
        ],

        // S = Rz(pi/2)
        Gate::S(q) => vec![Gate::Rz(*q, FRAC_PI_2)],

        // Sdg = Rz(-pi/2)
        Gate::Sdg(q) => vec![Gate::Rz(*q, -FRAC_PI_2)],

        // T = Rz(pi/4)
        Gate::T(q) => vec![Gate::Rz(*q, FRAC_PI_4)],

        // Tdg = Rz(-pi/4)
        Gate::Tdg(q) => vec![Gate::Rz(*q, -FRAC_PI_4)],

        // Y = X Rz(pi)  (global phase ignored)
        Gate::Y(q) => vec![Gate::X(*q), Gate::Rz(*q, PI)],

        // Z = Rz(pi)
        Gate::Z(q) => vec![Gate::Rz(*q, PI)],

        // Phase(theta) = Rz(theta)  (differs by global phase only)
        Gate::Phase(q, theta) => vec![Gate::Rz(*q, *theta)],

        // Rx(theta): Rz(-pi/2) SX Rz(pi - theta) SX Rz(-pi/2)
        // Simplified: for arbitrary Rx we use Rz(-pi/2) SX Rz(pi) Rz(-theta) SX Rz(-pi/2)
        // But a simpler exact decomposition is:
        //   Rx(theta) = Rz(-pi/2) Rx(pi/2) Rz(theta) Rx(pi/2) Rz(-pi/2)
        // keeping only basis gates
        Gate::Rx(q, theta) => {
            if (*theta - FRAC_PI_2).abs() < 1e-12 {
                // Already SX
                vec![Gate::Rx(*q, FRAC_PI_2)]
            } else {
                // Rx(theta) = Rz(-pi/2) SX Rz(PI - theta) SX Rz(-pi/2)
                vec![
                    Gate::Rz(*q, -FRAC_PI_2),
                    Gate::Rx(*q, FRAC_PI_2),
                    Gate::Rz(*q, PI - theta),
                    Gate::Rx(*q, FRAC_PI_2),
                    Gate::Rz(*q, -FRAC_PI_2),
                ]
            }
        }

        // Ry(theta) = Rz(-pi/2) SX Rz(theta) SX^dag Rz(pi/2)
        // SX^dag = Rx(-pi/2) but that is not in basis, so use X SX = Rx(-pi/2)
        // Actually: Ry(theta) = SX Rz(theta) SX^dag
        //   where SX^dag = Rz(pi) SX Rz(pi)  (since Rx(-pi/2) = Rz(pi) Rx(pi/2) Rz(pi))
        // Simpler: Ry(theta) = Rz(pi/2) Rx(pi/2) Rz(theta) Rx(pi/2) Rz(-pi/2)
        // We map to: Rz(-pi/2) SX Rz(theta + pi) SX Rz(pi/2)
        Gate::Ry(q, theta) => vec![
            Gate::Rz(*q, -FRAC_PI_2),
            Gate::Rx(*q, FRAC_PI_2),
            Gate::Rz(*q, theta + PI),
            Gate::Rx(*q, FRAC_PI_2),
            Gate::Rz(*q, FRAC_PI_2),
        ],

        // --- two-qubit gates ---
        // CZ = H(target) CNOT H(target)
        Gate::CZ(q1, q2) => {
            let mut gates = Vec::new();
            gates.extend(decompose_to_ibm(&Gate::H(*q2)));
            gates.push(Gate::CNOT(*q1, *q2));
            gates.extend(decompose_to_ibm(&Gate::H(*q2)));
            gates
        }

        // SWAP = CNOT(a,b) CNOT(b,a) CNOT(a,b)
        Gate::SWAP(a, b) => vec![Gate::CNOT(*a, *b), Gate::CNOT(*b, *a), Gate::CNOT(*a, *b)],

        // Rzz(theta) = CNOT(a,b) Rz(b, theta) CNOT(a,b)
        Gate::Rzz(a, b, theta) => {
            vec![Gate::CNOT(*a, *b), Gate::Rz(*b, *theta), Gate::CNOT(*a, *b)]
        }

        // --- non-unitary / pass-through ---
        Gate::Measure(q) => vec![Gate::Measure(*q)],
        Gate::Reset(q) => vec![Gate::Reset(*q)],
        Gate::Barrier => vec![Gate::Barrier],

        // Unitary1Q: decompose via ZYZ Euler angles and then map Ry
        // For simplicity, keep as-is since custom unitaries are an edge case
        // and the user can re-synthesize them.
        Gate::Unitary1Q(q, m) => vec![Gate::Unitary1Q(*q, *m)],
    }
}

// ---------------------------------------------------------------------------
// Rigetti Aspen decomposition: basis = {CZ, Rx, Rz}
// ---------------------------------------------------------------------------

/// Decompose a single gate into the Rigetti Aspen basis {CZ, Rx, Rz}.
pub fn decompose_to_rigetti(gate: &Gate) -> Vec<Gate> {
    match gate {
        // --- already in basis ---
        Gate::CZ(q1, q2) => vec![Gate::CZ(*q1, *q2)],
        Gate::Rx(q, theta) => vec![Gate::Rx(*q, *theta)],
        Gate::Rz(q, theta) => vec![Gate::Rz(*q, *theta)],

        // --- single-qubit Cliffords ---
        // H = Rz(pi) Rx(pi/2)  (up to global phase)
        Gate::H(q) => vec![Gate::Rz(*q, PI), Gate::Rx(*q, FRAC_PI_2)],

        Gate::X(q) => vec![Gate::Rx(*q, PI)],
        Gate::Y(q) => vec![Gate::Rx(*q, PI), Gate::Rz(*q, PI)],
        Gate::Z(q) => vec![Gate::Rz(*q, PI)],
        Gate::S(q) => vec![Gate::Rz(*q, FRAC_PI_2)],
        Gate::Sdg(q) => vec![Gate::Rz(*q, -FRAC_PI_2)],
        Gate::T(q) => vec![Gate::Rz(*q, FRAC_PI_4)],
        Gate::Tdg(q) => vec![Gate::Rz(*q, -FRAC_PI_4)],
        Gate::Phase(q, theta) => vec![Gate::Rz(*q, *theta)],

        // Ry(theta) = Rz(-pi/2) Rx(theta) Rz(pi/2)
        Gate::Ry(q, theta) => vec![
            Gate::Rz(*q, -FRAC_PI_2),
            Gate::Rx(*q, *theta),
            Gate::Rz(*q, FRAC_PI_2),
        ],

        // --- two-qubit gates ---
        // CNOT = H(target) CZ H(target)
        //      = [Rz(pi) Rx(pi/2)] CZ [Rz(pi) Rx(pi/2)]  on target
        Gate::CNOT(c, t) => {
            let mut gates = Vec::new();
            gates.extend(decompose_to_rigetti(&Gate::H(*t)));
            gates.push(Gate::CZ(*c, *t));
            gates.extend(decompose_to_rigetti(&Gate::H(*t)));
            gates
        }

        // SWAP = CNOT(a,b) CNOT(b,a) CNOT(a,b) -- each CNOT further decomposed
        Gate::SWAP(a, b) => {
            let mut gates = Vec::new();
            gates.extend(decompose_to_rigetti(&Gate::CNOT(*a, *b)));
            gates.extend(decompose_to_rigetti(&Gate::CNOT(*b, *a)));
            gates.extend(decompose_to_rigetti(&Gate::CNOT(*a, *b)));
            gates
        }

        // Rzz(theta) = CNOT(a,b) Rz(b, theta) CNOT(a,b)
        Gate::Rzz(a, b, theta) => {
            let mut gates = Vec::new();
            gates.extend(decompose_to_rigetti(&Gate::CNOT(*a, *b)));
            gates.push(Gate::Rz(*b, *theta));
            gates.extend(decompose_to_rigetti(&Gate::CNOT(*a, *b)));
            gates
        }

        // --- non-unitary / pass-through ---
        Gate::Measure(q) => vec![Gate::Measure(*q)],
        Gate::Reset(q) => vec![Gate::Reset(*q)],
        Gate::Barrier => vec![Gate::Barrier],
        Gate::Unitary1Q(q, m) => vec![Gate::Unitary1Q(*q, *m)],
    }
}

// ---------------------------------------------------------------------------
// IonQ Aria decomposition: basis = {Rx, Ry, Rzz}
// ---------------------------------------------------------------------------

/// Decompose a single gate into the IonQ Aria basis {Rx, Ry, Rzz}.
///
/// IonQ native gates are GPI, GPI2, and MS, which map naturally to rotations
/// in the {Rx, Ry, Rzz} family.
pub fn decompose_to_ionq(gate: &Gate) -> Vec<Gate> {
    match gate {
        // --- already in basis ---
        Gate::Rx(q, theta) => vec![Gate::Rx(*q, *theta)],
        Gate::Ry(q, theta) => vec![Gate::Ry(*q, *theta)],
        Gate::Rzz(a, b, theta) => vec![Gate::Rzz(*a, *b, *theta)],

        // --- single-qubit Cliffords (decomposed via Rx / Ry) ---
        // H = Ry(pi/2) Rx(pi)  (= Y^{1/2} X up to global phase)
        Gate::H(q) => vec![Gate::Ry(*q, FRAC_PI_2), Gate::Rx(*q, PI)],

        Gate::X(q) => vec![Gate::Rx(*q, PI)],
        Gate::Y(q) => vec![Gate::Ry(*q, PI)],

        // Z = Rx(pi) Ry(pi)  (up to global phase)
        Gate::Z(q) => vec![Gate::Rx(*q, PI), Gate::Ry(*q, PI)],

        // S = Rz(pi/2) = Rx(-pi/2) Ry(pi/2) Rx(pi/2)
        Gate::S(q) => vec![
            Gate::Rx(*q, -FRAC_PI_2),
            Gate::Ry(*q, FRAC_PI_2),
            Gate::Rx(*q, FRAC_PI_2),
        ],

        // Sdg = Rz(-pi/2) = Rx(-pi/2) Ry(-pi/2) Rx(pi/2)
        Gate::Sdg(q) => vec![
            Gate::Rx(*q, -FRAC_PI_2),
            Gate::Ry(*q, -FRAC_PI_2),
            Gate::Rx(*q, FRAC_PI_2),
        ],

        // T = Rz(pi/4) = Rx(-pi/2) Ry(pi/4) Rx(pi/2)
        Gate::T(q) => vec![
            Gate::Rx(*q, -FRAC_PI_2),
            Gate::Ry(*q, FRAC_PI_4),
            Gate::Rx(*q, FRAC_PI_2),
        ],

        // Tdg = Rz(-pi/4)
        Gate::Tdg(q) => vec![
            Gate::Rx(*q, -FRAC_PI_2),
            Gate::Ry(*q, -FRAC_PI_4),
            Gate::Rx(*q, FRAC_PI_2),
        ],

        // Rz(theta) = Rx(-pi/2) Ry(theta) Rx(pi/2)
        Gate::Rz(q, theta) => vec![
            Gate::Rx(*q, -FRAC_PI_2),
            Gate::Ry(*q, *theta),
            Gate::Rx(*q, FRAC_PI_2),
        ],

        // Phase(theta) maps to Rz(theta)
        Gate::Phase(q, theta) => decompose_to_ionq(&Gate::Rz(*q, *theta)),

        // --- two-qubit gates ---
        // CNOT via Rzz + single-qubit rotations:
        //   CNOT(c, t) = Ry(t, -pi/2) Rzz(c, t, pi/2) Rx(c, -pi/2) Rx(t, -pi/2) Ry(t, pi/2)
        // This is the standard MS-based CNOT decomposition.
        Gate::CNOT(c, t) => vec![
            Gate::Ry(*t, -FRAC_PI_2),
            Gate::Rzz(*c, *t, FRAC_PI_2),
            Gate::Rx(*c, -FRAC_PI_2),
            Gate::Rx(*t, -FRAC_PI_2),
            Gate::Ry(*t, FRAC_PI_2),
        ],

        // CZ = H(target) CNOT H(target) -- decompose recursively
        Gate::CZ(q1, q2) => {
            let mut gates = Vec::new();
            gates.extend(decompose_to_ionq(&Gate::H(*q2)));
            gates.extend(decompose_to_ionq(&Gate::CNOT(*q1, *q2)));
            gates.extend(decompose_to_ionq(&Gate::H(*q2)));
            gates
        }

        // SWAP = 3 CNOTs -- decompose recursively
        Gate::SWAP(a, b) => {
            let mut gates = Vec::new();
            gates.extend(decompose_to_ionq(&Gate::CNOT(*a, *b)));
            gates.extend(decompose_to_ionq(&Gate::CNOT(*b, *a)));
            gates.extend(decompose_to_ionq(&Gate::CNOT(*a, *b)));
            gates
        }

        // --- non-unitary / pass-through ---
        Gate::Measure(q) => vec![Gate::Measure(*q)],
        Gate::Reset(q) => vec![Gate::Reset(*q)],
        Gate::Barrier => vec![Gate::Barrier],
        Gate::Unitary1Q(q, m) => vec![Gate::Unitary1Q(*q, *m)],
    }
}

// ---------------------------------------------------------------------------
// Qubit routing via SWAP insertion
// ---------------------------------------------------------------------------

/// Route a circuit onto the given coupling map by inserting SWAP gates so that
/// every two-qubit gate operates on adjacent (coupled) qubits.
///
/// The coupling map is treated as undirected: `(a, b)` implies `(b, a)`.
///
/// Uses a simple greedy strategy: for each two-qubit gate on non-adjacent
/// qubits, find the shortest path via BFS and insert SWAPs along the path,
/// updating the logical-to-physical qubit mapping.
pub fn route_circuit(circuit: &QuantumCircuit, coupling_map: &[(u32, u32)]) -> QuantumCircuit {
    let n = circuit.num_qubits() as usize;

    // Build adjacency list (undirected).
    let adj = build_adjacency_list(coupling_map, n);

    // logical -> physical mapping (starts as identity)
    let mut log2phys: Vec<u32> = (0..n as u32).collect();
    // physical -> logical mapping (inverse)
    let mut phys2log: Vec<u32> = (0..n as u32).collect();

    let mut result = QuantumCircuit::new(circuit.num_qubits());

    for gate in circuit.gates() {
        let qubits = gate.qubits();
        if qubits.len() == 2 {
            let logical_a = qubits[0];
            let logical_b = qubits[1];
            let mut phys_a = log2phys[logical_a as usize];
            let mut phys_b = log2phys[logical_b as usize];

            // Check if already adjacent.
            if !are_adjacent(&adj, phys_a, phys_b) {
                // BFS to find shortest path from phys_a to phys_b.
                let path = bfs_shortest_path(&adj, phys_a, phys_b, n);

                // Insert SWAPs along the path to bring phys_a next to phys_b.
                // We move qubit A along the path towards B.
                // After swapping along path[0..path.len()-2], the logical qubit
                // that was at phys_a ends up adjacent to phys_b.
                for i in 0..path.len() - 2 {
                    let p1 = path[i];
                    let p2 = path[i + 1];

                    // Insert physical SWAP
                    result.add_gate(Gate::SWAP(p1, p2));

                    // Update mappings
                    let log1 = phys2log[p1 as usize];
                    let log2 = phys2log[p2 as usize];
                    log2phys[log1 as usize] = p2;
                    log2phys[log2 as usize] = p1;
                    phys2log[p1 as usize] = log2;
                    phys2log[p2 as usize] = log1;
                }

                // Recompute physical positions after routing.
                phys_a = log2phys[logical_a as usize];
                phys_b = log2phys[logical_b as usize];
            }

            // Emit the two-qubit gate on the (now adjacent) physical qubits.
            result.add_gate(remap_gate(gate, &log2phys));

            // Sanity check: the physical qubits should now be adjacent.
            debug_assert!(
                are_adjacent(&adj, phys_a, phys_b),
                "routing failed: qubits {} and {} are not adjacent after SWAP insertion",
                phys_a,
                phys_b
            );
        } else if qubits.len() == 1 {
            // Single-qubit gate: remap to physical qubit.
            result.add_gate(remap_gate(gate, &log2phys));
        } else {
            // Barrier, etc.
            result.add_gate(gate.clone());
        }
    }

    result
}

/// Build an adjacency list from a coupling map.
fn build_adjacency_list(coupling_map: &[(u32, u32)], n: usize) -> Vec<Vec<u32>> {
    let mut adj: Vec<Vec<u32>> = vec![Vec::new(); n];
    for &(a, b) in coupling_map {
        if (a as usize) < n && (b as usize) < n {
            if !adj[a as usize].contains(&b) {
                adj[a as usize].push(b);
            }
            if !adj[b as usize].contains(&a) {
                adj[b as usize].push(a);
            }
        }
    }
    adj
}

/// Check whether two physical qubits are directly connected.
fn are_adjacent(adj: &[Vec<u32>], a: u32, b: u32) -> bool {
    adj.get(a as usize)
        .map(|neighbors| neighbors.contains(&b))
        .unwrap_or(false)
}

/// BFS shortest path between two nodes in the coupling graph.
/// Returns the sequence of physical qubit indices from `start` to `end`
/// (inclusive of both endpoints).
fn bfs_shortest_path(adj: &[Vec<u32>], start: u32, end: u32, n: usize) -> Vec<u32> {
    if start == end {
        return vec![start];
    }

    let mut visited = vec![false; n];
    let mut parent: Vec<Option<u32>> = vec![None; n];
    let mut queue = VecDeque::new();

    visited[start as usize] = true;
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        if current == end {
            break;
        }
        for &neighbor in &adj[current as usize] {
            if !visited[neighbor as usize] {
                visited[neighbor as usize] = true;
                parent[neighbor as usize] = Some(current);
                queue.push_back(neighbor);
            }
        }
    }

    // Reconstruct path from end to start.
    let mut path = Vec::new();
    let mut current = end;
    path.push(current);
    while let Some(p) = parent[current as usize] {
        path.push(p);
        current = p;
        if current == start {
            break;
        }
    }
    path.reverse();
    path
}

/// Remap a gate's qubit indices using the logical-to-physical mapping.
fn remap_gate(gate: &Gate, log2phys: &[u32]) -> Gate {
    match gate {
        Gate::H(q) => Gate::H(log2phys[*q as usize]),
        Gate::X(q) => Gate::X(log2phys[*q as usize]),
        Gate::Y(q) => Gate::Y(log2phys[*q as usize]),
        Gate::Z(q) => Gate::Z(log2phys[*q as usize]),
        Gate::S(q) => Gate::S(log2phys[*q as usize]),
        Gate::Sdg(q) => Gate::Sdg(log2phys[*q as usize]),
        Gate::T(q) => Gate::T(log2phys[*q as usize]),
        Gate::Tdg(q) => Gate::Tdg(log2phys[*q as usize]),
        Gate::Rx(q, theta) => Gate::Rx(log2phys[*q as usize], *theta),
        Gate::Ry(q, theta) => Gate::Ry(log2phys[*q as usize], *theta),
        Gate::Rz(q, theta) => Gate::Rz(log2phys[*q as usize], *theta),
        Gate::Phase(q, theta) => Gate::Phase(log2phys[*q as usize], *theta),
        Gate::CNOT(c, t) => Gate::CNOT(log2phys[*c as usize], log2phys[*t as usize]),
        Gate::CZ(a, b) => Gate::CZ(log2phys[*a as usize], log2phys[*b as usize]),
        Gate::SWAP(a, b) => Gate::SWAP(log2phys[*a as usize], log2phys[*b as usize]),
        Gate::Rzz(a, b, theta) => Gate::Rzz(log2phys[*a as usize], log2phys[*b as usize], *theta),
        Gate::Measure(q) => Gate::Measure(log2phys[*q as usize]),
        Gate::Reset(q) => Gate::Reset(log2phys[*q as usize]),
        Gate::Barrier => Gate::Barrier,
        Gate::Unitary1Q(q, m) => Gate::Unitary1Q(log2phys[*q as usize], *m),
    }
}

// ---------------------------------------------------------------------------
// Gate cancellation / optimization
// ---------------------------------------------------------------------------

/// Optimize a circuit by cancelling and merging gates.
///
/// * Level 0: no optimization (pass-through).
/// * Level 1: cancel adjacent self-inverse pairs
///   (H-H, X-X, Y-Y, Z-Z, S-Sdg, T-Tdg, CNOT-CNOT on same qubits).
/// * Level 2: level 1 plus merge adjacent Rz gates on the same qubit
///   (Rz(a) Rz(b) -> Rz(a+b)).
pub fn optimize_gates(circuit: &QuantumCircuit, level: u8) -> QuantumCircuit {
    if level == 0 {
        return circuit.clone();
    }

    let mut gates: Vec<Gate> = circuit.gates().to_vec();

    // Apply cancellation passes iteratively until no more changes occur.
    let mut changed = true;
    while changed {
        changed = false;

        // Level 1: cancel inverse pairs
        let (new_gates, did_cancel) = cancel_inverse_pairs(&gates);
        if did_cancel {
            gates = new_gates;
            changed = true;
        }

        // Level 2: merge adjacent Rz
        if level >= 2 {
            let (new_gates, did_merge) = merge_adjacent_rz(&gates);
            if did_merge {
                gates = new_gates;
                changed = true;
            }
        }
    }

    let mut result = QuantumCircuit::new(circuit.num_qubits());
    for g in gates {
        result.add_gate(g);
    }
    result
}

/// Cancel adjacent self-inverse gate pairs.
///
/// Returns the new gate list and whether any cancellation occurred.
fn cancel_inverse_pairs(gates: &[Gate]) -> (Vec<Gate>, bool) {
    let mut result: Vec<Gate> = Vec::with_capacity(gates.len());
    let mut changed = false;
    let mut i = 0;

    while i < gates.len() {
        if i + 1 < gates.len() && is_inverse_pair(&gates[i], &gates[i + 1]) {
            // Skip both gates -- they cancel.
            changed = true;
            i += 2;
        } else {
            result.push(gates[i].clone());
            i += 1;
        }
    }

    (result, changed)
}

/// Check whether two gates form an inverse pair that cancels to identity.
fn is_inverse_pair(a: &Gate, b: &Gate) -> bool {
    match (a, b) {
        // Self-inverse single-qubit gates
        (Gate::H(q1), Gate::H(q2)) if q1 == q2 => true,
        (Gate::X(q1), Gate::X(q2)) if q1 == q2 => true,
        (Gate::Y(q1), Gate::Y(q2)) if q1 == q2 => true,
        (Gate::Z(q1), Gate::Z(q2)) if q1 == q2 => true,

        // Adjoint pairs
        (Gate::S(q1), Gate::Sdg(q2)) if q1 == q2 => true,
        (Gate::Sdg(q1), Gate::S(q2)) if q1 == q2 => true,
        (Gate::T(q1), Gate::Tdg(q2)) if q1 == q2 => true,
        (Gate::Tdg(q1), Gate::T(q2)) if q1 == q2 => true,

        // Self-inverse two-qubit gates (same qubit order)
        (Gate::CNOT(c1, t1), Gate::CNOT(c2, t2)) if c1 == c2 && t1 == t2 => true,
        (Gate::CZ(a1, b1), Gate::CZ(a2, b2))
            if (a1 == a2 && b1 == b2) || (a1 == b2 && b1 == a2) =>
        {
            true
        }
        (Gate::SWAP(a1, b1), Gate::SWAP(a2, b2))
            if (a1 == a2 && b1 == b2) || (a1 == b2 && b1 == a2) =>
        {
            true
        }

        _ => false,
    }
}

/// Merge adjacent Rz gates on the same qubit: Rz(a) Rz(b) -> Rz(a+b).
///
/// If the merged angle is effectively zero (|a+b| < epsilon), the gate is
/// dropped entirely.
///
/// Returns the new gate list and whether any merge occurred.
fn merge_adjacent_rz(gates: &[Gate]) -> (Vec<Gate>, bool) {
    let mut result: Vec<Gate> = Vec::with_capacity(gates.len());
    let mut changed = false;
    let mut i = 0;
    let epsilon = 1e-12;

    while i < gates.len() {
        if let Gate::Rz(q1, a) = &gates[i] {
            // Accumulate consecutive Rz on the same qubit.
            let mut total_angle = *a;
            let qubit = *q1;
            let mut count = 1;

            while i + count < gates.len() {
                if let Gate::Rz(q2, b) = &gates[i + count] {
                    if *q2 == qubit {
                        total_angle += b;
                        count += 1;
                        continue;
                    }
                }
                break;
            }

            if count > 1 {
                changed = true;
                if total_angle.abs() > epsilon {
                    result.push(Gate::Rz(qubit, total_angle));
                }
                // else: angle is zero, drop entirely
            } else {
                result.push(gates[i].clone());
            }
            i += count;
        } else {
            result.push(gates[i].clone());
            i += 1;
        }
    }

    (result, changed)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    // -- Decomposition tests --

    #[test]
    fn test_decompose_h_to_ibm() {
        let gates = decompose_to_ibm(&Gate::H(0));
        // H -> Rz(pi) SX Rz(pi) = 3 gates
        assert_eq!(gates.len(), 3);
        assert!(matches!(gates[0], Gate::Rz(0, _)));
        assert!(matches!(gates[1], Gate::Rx(0, _)));
        assert!(matches!(gates[2], Gate::Rz(0, _)));

        // The Rx should be pi/2 (SX)
        if let Gate::Rx(_, theta) = &gates[1] {
            assert!((theta - FRAC_PI_2).abs() < 1e-12);
        } else {
            panic!("expected Rx");
        }
    }

    #[test]
    fn test_decompose_s_to_ibm() {
        let gates = decompose_to_ibm(&Gate::S(0));
        assert_eq!(gates.len(), 1);
        if let Gate::Rz(0, theta) = &gates[0] {
            assert!((theta - FRAC_PI_2).abs() < 1e-12);
        } else {
            panic!("expected Rz(pi/2)");
        }
    }

    #[test]
    fn test_decompose_t_to_ibm() {
        let gates = decompose_to_ibm(&Gate::T(0));
        assert_eq!(gates.len(), 1);
        if let Gate::Rz(0, theta) = &gates[0] {
            assert!((theta - FRAC_PI_4).abs() < 1e-12);
        } else {
            panic!("expected Rz(pi/4)");
        }
    }

    #[test]
    fn test_decompose_swap_to_ibm() {
        let gates = decompose_to_ibm(&Gate::SWAP(0, 1));
        // SWAP -> 3 CNOTs
        assert_eq!(gates.len(), 3);
        assert!(gates.iter().all(|g| matches!(g, Gate::CNOT(_, _))));
    }

    #[test]
    fn test_decompose_cz_to_ibm() {
        let gates = decompose_to_ibm(&Gate::CZ(0, 1));
        // CZ -> H(1) CNOT H(1) = 3 + 1 + 3 = 7 gates
        assert_eq!(gates.len(), 7);
        // The middle gate should be CNOT
        assert!(matches!(gates[3], Gate::CNOT(0, 1)));
    }

    #[test]
    fn test_decompose_cnot_to_rigetti_produces_cz() {
        let gates = decompose_to_rigetti(&Gate::CNOT(0, 1));
        // CNOT -> H(target) CZ H(target)
        // H(target) = Rz(pi) Rx(pi/2) = 2 gates
        // So total = 2 + 1 + 2 = 5 gates
        assert_eq!(gates.len(), 5);
        // There should be exactly one CZ
        let cz_count = gates.iter().filter(|g| matches!(g, Gate::CZ(_, _))).count();
        assert_eq!(cz_count, 1);
        assert!(matches!(gates[2], Gate::CZ(0, 1)));
    }

    #[test]
    fn test_decompose_h_to_rigetti() {
        let gates = decompose_to_rigetti(&Gate::H(0));
        // H -> Rz(pi) Rx(pi/2)
        assert_eq!(gates.len(), 2);
        assert!(matches!(gates[0], Gate::Rz(0, _)));
        assert!(matches!(gates[1], Gate::Rx(0, _)));
    }

    #[test]
    fn test_decompose_cnot_to_ionq() {
        let gates = decompose_to_ionq(&Gate::CNOT(0, 1));
        // Should contain exactly one Rzz gate
        let rzz_count = gates
            .iter()
            .filter(|g| matches!(g, Gate::Rzz(_, _, _)))
            .count();
        assert_eq!(rzz_count, 1);
        // Total: Ry(-pi/2) Rzz(pi/2) Rx(-pi/2) Rx(-pi/2) Ry(pi/2) = 5 gates
        assert_eq!(gates.len(), 5);
    }

    #[test]
    fn test_decompose_preserves_non_unitary() {
        let measure_ibm = decompose_to_ibm(&Gate::Measure(0));
        assert_eq!(measure_ibm.len(), 1);
        assert!(matches!(measure_ibm[0], Gate::Measure(0)));

        let barrier_rigetti = decompose_to_rigetti(&Gate::Barrier);
        assert_eq!(barrier_rigetti.len(), 1);
        assert!(matches!(barrier_rigetti[0], Gate::Barrier));

        let reset_ionq = decompose_to_ionq(&Gate::Reset(2));
        assert_eq!(reset_ionq.len(), 1);
        assert!(matches!(reset_ionq[0], Gate::Reset(2)));
    }

    // -- Routing tests --

    #[test]
    fn test_route_adjacent_cnot_no_swaps() {
        // Linear chain: 0-1-2
        let coupling = vec![(0, 1), (1, 2)];
        let mut circuit = QuantumCircuit::new(3);
        circuit.cnot(0, 1);

        let routed = route_circuit(&circuit, &coupling);
        // Already adjacent -- no SWAPs needed.
        let swap_count = routed
            .gates()
            .iter()
            .filter(|g| matches!(g, Gate::SWAP(_, _)))
            .count();
        assert_eq!(swap_count, 0);
        assert_eq!(routed.gates().len(), 1);
    }

    #[test]
    fn test_route_non_adjacent_cnot_inserts_swaps() {
        // Linear chain: 0-1-2
        let coupling = vec![(0, 1), (1, 2)];
        let mut circuit = QuantumCircuit::new(3);
        circuit.cnot(0, 2); // not adjacent

        let routed = route_circuit(&circuit, &coupling);
        // Should have inserted at least one SWAP.
        let swap_count = routed
            .gates()
            .iter()
            .filter(|g| matches!(g, Gate::SWAP(_, _)))
            .count();
        assert!(
            swap_count >= 1,
            "expected at least 1 SWAP, got {}",
            swap_count
        );
    }

    #[test]
    fn test_route_single_qubit_gate_remapped() {
        // Linear chain: 0-1-2
        let coupling = vec![(0, 1), (1, 2)];
        let mut circuit = QuantumCircuit::new(3);
        circuit.h(0);

        let routed = route_circuit(&circuit, &coupling);
        // Single-qubit gate should pass through (mapped to physical qubit 0
        // since no SWAPs happened).
        assert_eq!(routed.gates().len(), 1);
        assert!(matches!(routed.gates()[0], Gate::H(0)));
    }

    #[test]
    fn test_bfs_shortest_path_linear() {
        // 0 - 1 - 2 - 3
        let coupling = vec![(0, 1), (1, 2), (2, 3)];
        let adj = build_adjacency_list(&coupling, 4);
        let path = bfs_shortest_path(&adj, 0, 3, 4);
        assert_eq!(path, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_bfs_shortest_path_branching() {
        // Star topology: 0-1, 0-2, 0-3
        let coupling = vec![(0, 1), (0, 2), (0, 3)];
        let adj = build_adjacency_list(&coupling, 4);
        let path = bfs_shortest_path(&adj, 1, 3, 4);
        // Shortest path: 1 -> 0 -> 3 (length 3 nodes)
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], 1);
        assert_eq!(*path.last().unwrap(), 3);
    }

    #[test]
    fn test_bfs_same_node() {
        let coupling = vec![(0, 1)];
        let adj = build_adjacency_list(&coupling, 2);
        let path = bfs_shortest_path(&adj, 0, 0, 2);
        assert_eq!(path, vec![0]);
    }

    // -- Optimization tests --

    #[test]
    fn test_cancel_hh_produces_empty() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.h(0);
        circuit.h(0);

        let optimized = optimize_gates(&circuit, 1);
        assert_eq!(optimized.gate_count(), 0);
    }

    #[test]
    fn test_cancel_xx() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.x(0);
        circuit.x(0);

        let optimized = optimize_gates(&circuit, 1);
        assert_eq!(optimized.gate_count(), 0);
    }

    #[test]
    fn test_cancel_zz() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.z(0);
        circuit.z(0);

        let optimized = optimize_gates(&circuit, 1);
        assert_eq!(optimized.gate_count(), 0);
    }

    #[test]
    fn test_cancel_s_sdg() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.s(0);
        circuit.add_gate(Gate::Sdg(0));

        let optimized = optimize_gates(&circuit, 1);
        assert_eq!(optimized.gate_count(), 0);
    }

    #[test]
    fn test_cancel_t_tdg() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.t(0);
        circuit.add_gate(Gate::Tdg(0));

        let optimized = optimize_gates(&circuit, 1);
        assert_eq!(optimized.gate_count(), 0);
    }

    #[test]
    fn test_cancel_cnot_cnot() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.cnot(0, 1);
        circuit.cnot(0, 1);

        let optimized = optimize_gates(&circuit, 1);
        assert_eq!(optimized.gate_count(), 0);
    }

    #[test]
    fn test_no_cancel_different_qubits() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0);
        circuit.h(1);

        let optimized = optimize_gates(&circuit, 1);
        assert_eq!(optimized.gate_count(), 2);
    }

    #[test]
    fn test_merge_rz_level2() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.rz(0, FRAC_PI_4);
        circuit.rz(0, FRAC_PI_4);

        let optimized = optimize_gates(&circuit, 2);
        assert_eq!(optimized.gate_count(), 1);
        if let Gate::Rz(0, theta) = &optimized.gates()[0] {
            assert!((theta - FRAC_PI_2).abs() < 1e-12);
        } else {
            panic!("expected merged Rz(pi/2)");
        }
    }

    #[test]
    fn test_merge_rz_to_zero_eliminates() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.rz(0, PI);
        circuit.rz(0, -PI);

        let optimized = optimize_gates(&circuit, 2);
        assert_eq!(optimized.gate_count(), 0);
    }

    #[test]
    fn test_merge_three_rz() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.rz(0, FRAC_PI_4);
        circuit.rz(0, FRAC_PI_4);
        circuit.rz(0, FRAC_PI_4);

        let optimized = optimize_gates(&circuit, 2);
        assert_eq!(optimized.gate_count(), 1);
        if let Gate::Rz(0, theta) = &optimized.gates()[0] {
            assert!((theta - 3.0 * FRAC_PI_4).abs() < 1e-12);
        } else {
            panic!("expected merged Rz(3*pi/4)");
        }
    }

    #[test]
    fn test_level0_no_optimization() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.h(0);
        circuit.h(0);

        let optimized = optimize_gates(&circuit, 0);
        assert_eq!(optimized.gate_count(), 2);
    }

    #[test]
    fn test_level1_does_not_merge_rz() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.rz(0, FRAC_PI_4);
        circuit.rz(0, FRAC_PI_4);

        let optimized = optimize_gates(&circuit, 1);
        // Level 1 only cancels inverses, not merges.
        assert_eq!(optimized.gate_count(), 2);
    }

    // -- Full pipeline tests --

    #[test]
    fn test_transpile_universal_passthrough() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0);
        circuit.cnot(0, 1);

        let config = TranspilerConfig {
            basis: BasisGateSet::Universal,
            coupling_map: None,
            optimization_level: 0,
        };

        let result = transpile(&circuit, &config);
        assert_eq!(result.gate_count(), 2);
    }

    #[test]
    fn test_transpile_ibm_decomposes_then_optimizes() {
        let mut circuit = QuantumCircuit::new(1);
        // H H should decompose to 6 gates then cancel to 0
        circuit.h(0);
        circuit.h(0);

        let config = TranspilerConfig {
            basis: BasisGateSet::IbmEagle,
            coupling_map: None,
            optimization_level: 2,
        };

        let result = transpile(&circuit, &config);
        // After decomposition: Rz(pi) Rx(pi/2) Rz(pi) Rz(pi) Rx(pi/2) Rz(pi)
        // Level 2 merges adjacent Rz: Rz(pi) Rx(pi/2) Rz(2*pi) Rx(pi/2) Rz(pi)
        // Rz(2*pi) is not zero so it stays (it is 2*pi, not 0).
        // This tests that the pipeline runs without error.
        assert!(result.gate_count() < 6, "expected some optimization");
    }

    #[test]
    fn test_transpile_with_routing() {
        // 3-qubit linear chain, CNOT(0,2) should get routed
        let mut circuit = QuantumCircuit::new(3);
        circuit.cnot(0, 2);

        let config = TranspilerConfig {
            basis: BasisGateSet::Universal,
            coupling_map: Some(vec![(0, 1), (1, 2)]),
            optimization_level: 0,
        };

        let result = transpile(&circuit, &config);
        // Should have inserted SWAPs
        let swap_count = result
            .gates()
            .iter()
            .filter(|g| matches!(g, Gate::SWAP(_, _)))
            .count();
        assert!(swap_count >= 1);
    }

    #[test]
    fn test_transpile_rigetti_bell_state() {
        // Bell state: H(0), CNOT(0,1)
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0);
        circuit.cnot(0, 1);

        let config = TranspilerConfig {
            basis: BasisGateSet::RigettiAspen,
            coupling_map: None,
            optimization_level: 0,
        };

        let result = transpile(&circuit, &config);
        // All gates should be in {CZ, Rx, Rz}
        for gate in result.gates() {
            match gate {
                Gate::CZ(_, _) | Gate::Rx(_, _) | Gate::Rz(_, _) => {}
                Gate::Measure(_) | Gate::Reset(_) | Gate::Barrier => {}
                other => panic!("gate {:?} not in Rigetti basis", other),
            }
        }
    }

    #[test]
    fn test_transpile_ionq_single_qubit() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.h(0);

        let config = TranspilerConfig {
            basis: BasisGateSet::IonQAria,
            coupling_map: None,
            optimization_level: 0,
        };

        let result = transpile(&circuit, &config);
        // All gates should be in {Rx, Ry, Rzz}
        for gate in result.gates() {
            match gate {
                Gate::Rx(_, _) | Gate::Ry(_, _) | Gate::Rzz(_, _, _) => {}
                Gate::Measure(_) | Gate::Reset(_) | Gate::Barrier => {}
                other => panic!("gate {:?} not in IonQ basis", other),
            }
        }
    }

    #[test]
    fn test_iterative_cancellation() {
        // After cancelling the inner pair, the outer pair should also cancel.
        // X H H X -> X (cancel) X -> (cancel) -> empty
        let mut circuit = QuantumCircuit::new(1);
        circuit.x(0);
        circuit.h(0);
        circuit.h(0);
        circuit.x(0);

        let optimized = optimize_gates(&circuit, 1);
        assert_eq!(optimized.gate_count(), 0);
    }

    #[test]
    fn test_routing_updates_mapping_correctly() {
        // Linear chain: 0-1-2-3
        // Two CNOTs: CNOT(0,3) then CNOT(0,1)
        // After routing CNOT(0,3), the mapping changes due to SWAPs.
        let coupling = vec![(0, 1), (1, 2), (2, 3)];
        let mut circuit = QuantumCircuit::new(4);
        circuit.cnot(0, 3);
        circuit.h(0);

        let routed = route_circuit(&circuit, &coupling);
        // The circuit should compile without panicking and contain SWAPs.
        let swap_count = routed
            .gates()
            .iter()
            .filter(|g| matches!(g, Gate::SWAP(_, _)))
            .count();
        assert!(swap_count >= 1);
        // The H gate should also be present (on the remapped physical qubit).
        let h_count = routed
            .gates()
            .iter()
            .filter(|g| matches!(g, Gate::H(_)))
            .count();
        assert_eq!(h_count, 1);
    }

    #[test]
    fn test_decompose_rzz_to_ibm() {
        let gates = decompose_to_ibm(&Gate::Rzz(0, 1, FRAC_PI_4));
        // Rzz -> CNOT Rz CNOT = 3 gates
        assert_eq!(gates.len(), 3);
        assert!(matches!(gates[0], Gate::CNOT(0, 1)));
        assert!(matches!(gates[1], Gate::Rz(1, _)));
        assert!(matches!(gates[2], Gate::CNOT(0, 1)));
    }

    #[test]
    fn test_basis_gate_set_variants() {
        // Ensure all variants are distinct and constructible.
        let variants = [
            BasisGateSet::IbmEagle,
            BasisGateSet::IonQAria,
            BasisGateSet::RigettiAspen,
            BasisGateSet::Universal,
        ];
        for (i, a) in variants.iter().enumerate() {
            for (j, b) in variants.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }
}
