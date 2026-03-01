//! Quantum circuit: a fluent builder for ordered gate sequences

use crate::gate::Gate;
use crate::types::QubitIndex;

/// A quantum circuit consisting of an ordered sequence of gates on a qubit register.
#[derive(Debug, Clone)]
pub struct QuantumCircuit {
    gates: Vec<Gate>,
    num_qubits: u32,
}

impl QuantumCircuit {
    /// Create a new empty circuit for the given number of qubits.
    pub fn new(num_qubits: u32) -> Self {
        Self {
            gates: Vec::new(),
            num_qubits,
        }
    }

    // -------------------------------------------------------------------
    // Fluent single-qubit gate methods
    // -------------------------------------------------------------------

    pub fn h(&mut self, q: QubitIndex) -> &mut Self {
        self.gates.push(Gate::H(q));
        self
    }

    pub fn x(&mut self, q: QubitIndex) -> &mut Self {
        self.gates.push(Gate::X(q));
        self
    }

    pub fn y(&mut self, q: QubitIndex) -> &mut Self {
        self.gates.push(Gate::Y(q));
        self
    }

    pub fn z(&mut self, q: QubitIndex) -> &mut Self {
        self.gates.push(Gate::Z(q));
        self
    }

    pub fn s(&mut self, q: QubitIndex) -> &mut Self {
        self.gates.push(Gate::S(q));
        self
    }

    pub fn t(&mut self, q: QubitIndex) -> &mut Self {
        self.gates.push(Gate::T(q));
        self
    }

    pub fn rx(&mut self, q: QubitIndex, angle: f64) -> &mut Self {
        self.gates.push(Gate::Rx(q, angle));
        self
    }

    pub fn ry(&mut self, q: QubitIndex, angle: f64) -> &mut Self {
        self.gates.push(Gate::Ry(q, angle));
        self
    }

    pub fn rz(&mut self, q: QubitIndex, angle: f64) -> &mut Self {
        self.gates.push(Gate::Rz(q, angle));
        self
    }

    pub fn phase(&mut self, q: QubitIndex, angle: f64) -> &mut Self {
        self.gates.push(Gate::Phase(q, angle));
        self
    }

    // -------------------------------------------------------------------
    // Fluent two-qubit gate methods
    // -------------------------------------------------------------------

    pub fn cnot(&mut self, control: QubitIndex, target: QubitIndex) -> &mut Self {
        self.gates.push(Gate::CNOT(control, target));
        self
    }

    pub fn cz(&mut self, q1: QubitIndex, q2: QubitIndex) -> &mut Self {
        self.gates.push(Gate::CZ(q1, q2));
        self
    }

    pub fn swap(&mut self, q1: QubitIndex, q2: QubitIndex) -> &mut Self {
        self.gates.push(Gate::SWAP(q1, q2));
        self
    }

    pub fn rzz(&mut self, q1: QubitIndex, q2: QubitIndex, angle: f64) -> &mut Self {
        self.gates.push(Gate::Rzz(q1, q2, angle));
        self
    }

    // -------------------------------------------------------------------
    // Special operations
    // -------------------------------------------------------------------

    pub fn measure(&mut self, q: QubitIndex) -> &mut Self {
        self.gates.push(Gate::Measure(q));
        self
    }

    /// Add a measurement gate to every qubit.
    pub fn measure_all(&mut self) -> &mut Self {
        for q in 0..self.num_qubits {
            self.gates.push(Gate::Measure(q));
        }
        self
    }

    pub fn reset(&mut self, q: QubitIndex) -> &mut Self {
        self.gates.push(Gate::Reset(q));
        self
    }

    pub fn barrier(&mut self) -> &mut Self {
        self.gates.push(Gate::Barrier);
        self
    }

    /// Push an arbitrary gate onto the circuit.
    pub fn add_gate(&mut self, gate: Gate) -> &mut Self {
        self.gates.push(gate);
        self
    }

    // -------------------------------------------------------------------
    // Accessors
    // -------------------------------------------------------------------

    pub fn gates(&self) -> &[Gate] {
        &self.gates
    }

    pub fn num_qubits(&self) -> u32 {
        self.num_qubits
    }

    pub fn gate_count(&self) -> usize {
        self.gates.len()
    }

    /// Compute the circuit depth: the longest path through the circuit
    /// taking qubit dependencies into account.
    ///
    /// A `Barrier` synchronises all qubits to the current maximum depth.
    pub fn depth(&self) -> u32 {
        let mut qubit_depth = vec![0u32; self.num_qubits as usize];

        for gate in &self.gates {
            match gate {
                Gate::Barrier => {
                    let max_d = qubit_depth.iter().copied().max().unwrap_or(0);
                    for d in qubit_depth.iter_mut() {
                        *d = max_d;
                    }
                }
                other => {
                    let qubits = other.qubits();
                    if qubits.is_empty() {
                        continue;
                    }
                    let max_d = qubits
                        .iter()
                        .map(|&q| qubit_depth.get(q as usize).copied().unwrap_or(0))
                        .max()
                        .unwrap_or(0);
                    for &q in &qubits {
                        if (q as usize) < qubit_depth.len() {
                            qubit_depth[q as usize] = max_d + 1;
                        }
                    }
                }
            }
        }

        qubit_depth.into_iter().max().unwrap_or(0)
    }
}
