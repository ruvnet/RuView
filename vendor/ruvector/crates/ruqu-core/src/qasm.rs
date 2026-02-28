//! OpenQASM 3.0 export bridge for `QuantumCircuit`.
//!
//! Converts a circuit into a valid OpenQASM 3.0 program string using the
//! `stdgates.inc` naming conventions. Arbitrary single-qubit unitaries
//! (`Unitary1Q`) are decomposed into ZYZ Euler angles and emitted as
//! `U(theta, phi, lambda)` gates.

use std::fmt::Write;

use crate::circuit::QuantumCircuit;
use crate::gate::Gate;
use crate::types::Complex;

// ---------------------------------------------------------------------------
// ZYZ Euler decomposition
// ---------------------------------------------------------------------------

/// Euler angles in the ZYZ convention: `Rz(phi) * Ry(theta) * Rz(lambda)`.
///
/// The overall unitary (up to a global phase) is:
///
/// ```text
/// U(theta, phi, lambda) = Rz(phi) * Ry(theta) * Rz(lambda)
/// ```
///
/// This matches the OpenQASM 3.0 `U(theta, phi, lambda)` gate definition.
struct ZyzAngles {
    theta: f64,
    phi: f64,
    lambda: f64,
}

/// Decompose an arbitrary 2x2 unitary matrix into ZYZ Euler angles.
///
/// Given a unitary U, we find (theta, phi, lambda) such that
///
/// ```text
/// U = e^{i*alpha} * Rz(phi) * Ry(theta) * Rz(lambda)
/// ```
///
/// where alpha is a discarded global phase.
///
/// The parametrisation expands to:
///
/// ```text
/// U[0][0] = e^{ia} * cos(t/2) * e^{-i(p+l)/2}
/// U[0][1] = e^{ia} * (-sin(t/2)) * e^{-i(p-l)/2}
/// U[1][0] = e^{ia} * sin(t/2) * e^{i(p-l)/2}
/// U[1][1] = e^{ia} * cos(t/2) * e^{i(p+l)/2}
/// ```
///
/// We extract phi and lambda independently using products that isolate
/// each angle, avoiding the half-sum/half-difference 2*pi ambiguity.
fn decompose_zyz(u: &[[Complex; 2]; 2]) -> ZyzAngles {
    let abs00 = u[0][0].norm();
    let abs10 = u[1][0].norm();

    // Clamp for numerical safety before acos
    let cos_half_theta = abs00.clamp(0.0, 1.0);
    let theta = 2.0 * cos_half_theta.acos();

    let eps = 1e-12;

    if abs00 > eps && abs10 > eps {
        // General case: both cos(t/2) and sin(t/2) are nonzero.
        //
        // We extract phi and lambda directly from pairwise products of
        // matrix elements that isolate each angle individually.
        //
        // From the parametrisation (global phase e^{ia} cancels in products
        // of an element with the conjugate of another):
        //
        //   conj(U[0][0]) * U[1][0] = cos(t/2) * sin(t/2) * e^{i*phi}
        //   => phi = arg(conj(U[0][0]) * U[1][0])
        //
        //   U[1][1] * conj(U[1][0]) = cos(t/2) * sin(t/2) * e^{i*lambda}
        //   => lambda = arg(U[1][1] * conj(U[1][0]))
        //
        // These formulas give phi and lambda each in (-pi, pi] without
        // the half-angle ambiguity that plagues the (sum, diff) approach.
        let phi_complex = u[0][0].conj() * u[1][0];
        let lambda_complex = u[1][1] * u[1][0].conj();

        ZyzAngles {
            theta,
            phi: phi_complex.arg(),
            lambda: lambda_complex.arg(),
        }
    } else if abs10 < eps {
        // theta ~ 0: U is nearly diagonal (up to global phase).
        //   U[0][0] = e^{ia} * e^{-i(p+l)/2}
        //   U[1][1] = e^{ia} * e^{i(p+l)/2}
        //   => U[1][1] * conj(U[0][0]) = e^{i(p+l)}
        // We only need phi + lambda. Set lambda = 0.
        let diag_product = u[1][1] * u[0][0].conj();
        ZyzAngles {
            theta: 0.0,
            phi: diag_product.arg(),
            lambda: 0.0,
        }
    } else {
        // theta ~ pi: U[0][0] ~ 0 and U[1][1] ~ 0.
        // Only the off-diagonal elements carry useful phase info.
        //   U[1][0] = e^{ia} * sin(t/2) * e^{i(p-l)/2}
        //   U[0][1] = e^{ia} * (-sin(t/2)) * e^{-i(p-l)/2}
        //
        //   U[1][0] * conj(-U[0][1]) = sin^2(t/2) * e^{i(p-l)}
        //
        // Set lambda = 0, phi = phi - lambda = arg of that product.
        let neg_01 = -u[0][1];
        let anti_product = u[1][0] * neg_01.conj();
        ZyzAngles {
            theta: std::f64::consts::PI,
            phi: anti_product.arg(),
            lambda: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Angle formatting helper
// ---------------------------------------------------------------------------

/// Format a floating-point angle for QASM output.
/// Uses enough precision to be lossless for common multiples of pi,
/// and trims unnecessary trailing zeros for readability.
fn fmt_angle(angle: f64) -> String {
    // Use 15 significant digits (full f64 precision), then trim trailing zeros.
    let s = format!("{:.15e}", angle);

    // For angles that are "nice" decimals, prefer fixed notation.
    // If the absolute value is in [1e-4, 1e6] use fixed, else scientific.
    let abs = angle.abs();
    if abs == 0.0 {
        return "0".to_string();
    }

    if abs >= 1e-4 && abs < 1e6 {
        // Fixed notation with enough precision
        let s = format!("{:.15}", angle);
        // Trim trailing zeros after the decimal point
        let trimmed = s.trim_end_matches('0');
        let trimmed = trimmed.trim_end_matches('.');
        trimmed.to_string()
    } else {
        // Scientific notation
        s
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Convert a `QuantumCircuit` into a valid OpenQASM 3.0 program string.
///
/// The output uses `stdgates.inc` gate names and follows the OpenQASM 3.0
/// specification for qubit/bit declarations, measurements, resets, and
/// barriers.
///
/// # Example
///
/// ```
/// use ruqu_core::circuit::QuantumCircuit;
/// use ruqu_core::qasm::to_qasm3;
///
/// let mut circuit = QuantumCircuit::new(2);
/// circuit.h(0).cnot(0, 1);
/// let qasm = to_qasm3(&circuit);
/// assert!(qasm.starts_with("OPENQASM 3.0;"));
/// ```
pub fn to_qasm3(circuit: &QuantumCircuit) -> String {
    let n = circuit.num_qubits();

    // Pre-allocate a reasonable buffer size
    let mut out = String::with_capacity(256 + circuit.gates().len() * 30);

    // Header
    out.push_str("OPENQASM 3.0;\n");
    out.push_str("include \"stdgates.inc\";\n");

    // Register declarations
    let _ = writeln!(out, "qubit[{}] q;", n);
    let _ = writeln!(out, "bit[{}] c;", n);

    // Gate body
    for gate in circuit.gates() {
        emit_gate(&mut out, gate);
    }

    out
}

/// Emit a single gate as one or more QASM lines.
fn emit_gate(out: &mut String, gate: &Gate) {
    match gate {
        // --- Single-qubit standard gates ---
        Gate::H(q) => {
            let _ = writeln!(out, "h q[{}];", q);
        }
        Gate::X(q) => {
            let _ = writeln!(out, "x q[{}];", q);
        }
        Gate::Y(q) => {
            let _ = writeln!(out, "y q[{}];", q);
        }
        Gate::Z(q) => {
            let _ = writeln!(out, "z q[{}];", q);
        }
        Gate::S(q) => {
            let _ = writeln!(out, "s q[{}];", q);
        }
        Gate::Sdg(q) => {
            let _ = writeln!(out, "sdg q[{}];", q);
        }
        Gate::T(q) => {
            let _ = writeln!(out, "t q[{}];", q);
        }
        Gate::Tdg(q) => {
            let _ = writeln!(out, "tdg q[{}];", q);
        }

        // --- Parametric single-qubit gates ---
        Gate::Rx(q, angle) => {
            let _ = writeln!(out, "rx({}) q[{}];", fmt_angle(*angle), q);
        }
        Gate::Ry(q, angle) => {
            let _ = writeln!(out, "ry({}) q[{}];", fmt_angle(*angle), q);
        }
        Gate::Rz(q, angle) => {
            let _ = writeln!(out, "rz({}) q[{}];", fmt_angle(*angle), q);
        }
        Gate::Phase(q, angle) => {
            let _ = writeln!(out, "p({}) q[{}];", fmt_angle(*angle), q);
        }

        // --- Two-qubit gates ---
        Gate::CNOT(ctrl, tgt) => {
            let _ = writeln!(out, "cx q[{}], q[{}];", ctrl, tgt);
        }
        Gate::CZ(q1, q2) => {
            let _ = writeln!(out, "cz q[{}], q[{}];", q1, q2);
        }
        Gate::SWAP(q1, q2) => {
            let _ = writeln!(out, "swap q[{}], q[{}];", q1, q2);
        }
        Gate::Rzz(q1, q2, angle) => {
            let _ = writeln!(out, "rzz({}) q[{}], q[{}];", fmt_angle(*angle), q1, q2);
        }

        // --- Special operations ---
        Gate::Measure(q) => {
            let _ = writeln!(out, "c[{}] = measure q[{}];", q, q);
        }
        Gate::Reset(q) => {
            let _ = writeln!(out, "reset q[{}];", q);
        }
        Gate::Barrier => {
            out.push_str("barrier q;\n");
        }

        // --- Arbitrary single-qubit unitary (ZYZ decomposition) ---
        Gate::Unitary1Q(q, matrix) => {
            let angles = decompose_zyz(matrix);
            let _ = writeln!(
                out,
                "U({}, {}, {}) q[{}];",
                fmt_angle(angles.theta),
                fmt_angle(angles.phi),
                fmt_angle(angles.lambda),
                q,
            );
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::QuantumCircuit;
    use crate::gate::Gate;
    use crate::types::Complex;
    use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, FRAC_PI_4, PI};

    /// Helper: verify the QASM header is present and well-formed.
    fn assert_valid_header(qasm: &str) {
        let lines: Vec<&str> = qasm.lines().collect();
        assert!(lines.len() >= 4, "QASM output should have at least 4 lines");
        assert_eq!(lines[0], "OPENQASM 3.0;");
        assert_eq!(lines[1], "include \"stdgates.inc\";");
        assert!(lines[2].starts_with("qubit["));
        assert!(lines[3].starts_with("bit["));
    }

    /// Collect only the gate lines (skip the 4-line header).
    fn gate_lines(qasm: &str) -> Vec<String> {
        qasm.lines()
            .skip(4)
            .map(|l| l.to_string())
            .filter(|l| !l.is_empty())
            .collect()
    }

    // ----- Bell State -----

    #[test]
    fn test_bell_state() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0).cnot(0, 1);

        let qasm = to_qasm3(&circuit);
        assert_valid_header(&qasm);

        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "h q[0];");
        assert_eq!(lines[1], "cx q[0], q[1];");

        // Verify register sizes
        assert!(qasm.contains("qubit[2] q;"));
        assert!(qasm.contains("bit[2] c;"));
    }

    #[test]
    fn test_bell_state_with_measurement() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0).cnot(0, 1).measure(0).measure(1);

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "h q[0];");
        assert_eq!(lines[1], "cx q[0], q[1];");
        assert_eq!(lines[2], "c[0] = measure q[0];");
        assert_eq!(lines[3], "c[1] = measure q[1];");
    }

    // ----- GHZ State -----

    #[test]
    fn test_ghz_3_qubit() {
        let mut circuit = QuantumCircuit::new(3);
        circuit.h(0).cnot(0, 1).cnot(0, 2);

        let qasm = to_qasm3(&circuit);
        assert_valid_header(&qasm);
        assert!(qasm.contains("qubit[3] q;"));
        assert!(qasm.contains("bit[3] c;"));

        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "h q[0];");
        assert_eq!(lines[1], "cx q[0], q[1];");
        assert_eq!(lines[2], "cx q[0], q[2];");
    }

    #[test]
    fn test_ghz_5_qubit() {
        let mut circuit = QuantumCircuit::new(5);
        circuit.h(0);
        for i in 1..5 {
            circuit.cnot(0, i);
        }

        let qasm = to_qasm3(&circuit);
        assert!(qasm.contains("qubit[5] q;"));

        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "h q[0];");
        for i in 1..5u32 {
            assert_eq!(lines[i as usize], format!("cx q[0], q[{}];", i));
        }
    }

    // ----- Parametric Gates -----

    #[test]
    fn test_parametric_rx_ry_rz() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.rx(0, PI).ry(0, FRAC_PI_2).rz(0, FRAC_PI_4);

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 3);

        // Verify the gate names are correct
        assert!(lines[0].starts_with("rx("));
        assert!(lines[0].ends_with(") q[0];"));
        assert!(lines[1].starts_with("ry("));
        assert!(lines[2].starts_with("rz("));

        // Verify angles parse back to original values within tolerance
        let rx_angle: f64 = extract_angle(&lines[0]);
        let ry_angle: f64 = extract_angle(&lines[1]);
        let rz_angle: f64 = extract_angle(&lines[2]);

        assert!((rx_angle - PI).abs() < 1e-10, "rx angle mismatch");
        assert!((ry_angle - FRAC_PI_2).abs() < 1e-10, "ry angle mismatch");
        assert!((rz_angle - FRAC_PI_4).abs() < 1e-10, "rz angle mismatch");
    }

    #[test]
    fn test_phase_gate() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.phase(0, PI / 3.0);

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("p("));
        assert!(lines[0].ends_with(") q[0];"));

        let angle = extract_angle(&lines[0]);
        assert!((angle - PI / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_rzz_gate() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.rzz(0, 1, PI / 6.0);

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("rzz("));
        assert!(lines[0].contains("q[0], q[1]"));

        let angle = extract_angle(&lines[0]);
        assert!((angle - PI / 6.0).abs() < 1e-10);
    }

    // ----- All Standard Gates -----

    #[test]
    fn test_all_single_qubit_standard_gates() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.h(0);
        circuit.x(0);
        circuit.y(0);
        circuit.z(0);
        circuit.s(0);
        circuit.add_gate(Gate::Sdg(0));
        circuit.t(0);
        circuit.add_gate(Gate::Tdg(0));

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 8);
        assert_eq!(lines[0], "h q[0];");
        assert_eq!(lines[1], "x q[0];");
        assert_eq!(lines[2], "y q[0];");
        assert_eq!(lines[3], "z q[0];");
        assert_eq!(lines[4], "s q[0];");
        assert_eq!(lines[5], "sdg q[0];");
        assert_eq!(lines[6], "t q[0];");
        assert_eq!(lines[7], "tdg q[0];");
    }

    #[test]
    fn test_two_qubit_gates() {
        let mut circuit = QuantumCircuit::new(3);
        circuit.cnot(0, 1);
        circuit.cz(1, 2);
        circuit.swap(0, 2);

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "cx q[0], q[1];");
        assert_eq!(lines[1], "cz q[1], q[2];");
        assert_eq!(lines[2], "swap q[0], q[2];");
    }

    // ----- Special Operations -----

    #[test]
    fn test_reset() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.reset(0);

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "reset q[0];");
    }

    #[test]
    fn test_barrier() {
        let mut circuit = QuantumCircuit::new(3);
        circuit.h(0).barrier().cnot(0, 1);

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "h q[0];");
        assert_eq!(lines[1], "barrier q;");
        assert_eq!(lines[2], "cx q[0], q[1];");
    }

    #[test]
    fn test_measure_all() {
        let mut circuit = QuantumCircuit::new(3);
        circuit.h(0).measure_all();

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "h q[0];");
        assert_eq!(lines[1], "c[0] = measure q[0];");
        assert_eq!(lines[2], "c[1] = measure q[1];");
        assert_eq!(lines[3], "c[2] = measure q[2];");
    }

    // ----- Unitary1Q Decomposition -----

    #[test]
    fn test_unitary1q_identity() {
        // Identity matrix should decompose to U(0, 0, 0) (or near-zero angles)
        let identity = [
            [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
            [Complex::new(0.0, 0.0), Complex::new(1.0, 0.0)],
        ];

        let mut circuit = QuantumCircuit::new(1);
        circuit.add_gate(Gate::Unitary1Q(0, identity));

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("U("));
        assert!(lines[0].ends_with(") q[0];"));

        // Extract the three angles from U(theta, phi, lambda)
        let (theta, phi, lambda) = extract_u_angles(&lines[0]);
        assert!(
            theta.abs() < 1e-10,
            "Identity theta should be ~0, got {}",
            theta
        );
        // For identity, phi + lambda should be ~0 (mod 2*pi)
        let sum = phi + lambda;
        let sum_mod = ((sum % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);
        assert!(
            sum_mod.abs() < 1e-10 || (sum_mod - 2.0 * PI).abs() < 1e-10,
            "Identity phi+lambda should be ~0 mod 2pi, got {}",
            sum
        );
    }

    #[test]
    fn test_unitary1q_hadamard() {
        // Hadamard matrix: (1/sqrt2) * [[1, 1], [1, -1]]
        let h = FRAC_1_SQRT_2;
        let hadamard = [
            [Complex::new(h, 0.0), Complex::new(h, 0.0)],
            [Complex::new(h, 0.0), Complex::new(-h, 0.0)],
        ];

        let mut circuit = QuantumCircuit::new(1);
        circuit.add_gate(Gate::Unitary1Q(0, hadamard));

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("U("));

        // Hadamard is Rz(pi) * Ry(pi/2) * Rz(0) or equivalent.
        // We verify the decomposition reconstructs the correct unitary.
        let (theta, phi, lambda) = extract_u_angles(&lines[0]);
        let reconstructed = reconstruct_zyz(theta, phi, lambda);
        assert_unitaries_equal_up_to_phase(&hadamard, &reconstructed);
    }

    #[test]
    fn test_unitary1q_x_gate() {
        // X gate: [[0, 1], [1, 0]]
        let x_matrix = [
            [Complex::new(0.0, 0.0), Complex::new(1.0, 0.0)],
            [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
        ];

        let mut circuit = QuantumCircuit::new(1);
        circuit.add_gate(Gate::Unitary1Q(0, x_matrix));

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        let (theta, phi, lambda) = extract_u_angles(&lines[0]);
        let reconstructed = reconstruct_zyz(theta, phi, lambda);
        assert_unitaries_equal_up_to_phase(&x_matrix, &reconstructed);
    }

    #[test]
    fn test_unitary1q_s_gate() {
        // S gate: [[1, 0], [0, i]]
        let s_matrix = [
            [Complex::new(1.0, 0.0), Complex::new(0.0, 0.0)],
            [Complex::new(0.0, 0.0), Complex::new(0.0, 1.0)],
        ];

        let mut circuit = QuantumCircuit::new(1);
        circuit.add_gate(Gate::Unitary1Q(0, s_matrix));

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        let (theta, phi, lambda) = extract_u_angles(&lines[0]);

        // S is diagonal, so theta should be ~0
        assert!(
            theta.abs() < 1e-10,
            "S gate theta should be ~0, got {}",
            theta
        );

        let reconstructed = reconstruct_zyz(theta, phi, lambda);
        assert_unitaries_equal_up_to_phase(&s_matrix, &reconstructed);
    }

    #[test]
    fn test_unitary1q_arbitrary() {
        // An arbitrary unitary: Rx(pi/3) in matrix form
        let half = PI / 6.0;
        let cos_h = half.cos();
        let sin_h = half.sin();
        let arb_matrix = [
            [Complex::new(cos_h, 0.0), Complex::new(0.0, -sin_h)],
            [Complex::new(0.0, -sin_h), Complex::new(cos_h, 0.0)],
        ];

        let mut circuit = QuantumCircuit::new(1);
        circuit.add_gate(Gate::Unitary1Q(0, arb_matrix));

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        let (theta, phi, lambda) = extract_u_angles(&lines[0]);
        let reconstructed = reconstruct_zyz(theta, phi, lambda);
        assert_unitaries_equal_up_to_phase(&arb_matrix, &reconstructed);
    }

    #[test]
    fn test_unitary1q_y_gate() {
        // Y gate: [[0, -i], [i, 0]]
        let y_matrix = [
            [Complex::new(0.0, 0.0), Complex::new(0.0, -1.0)],
            [Complex::new(0.0, 1.0), Complex::new(0.0, 0.0)],
        ];

        let mut circuit = QuantumCircuit::new(1);
        circuit.add_gate(Gate::Unitary1Q(0, y_matrix));

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        let (theta, phi, lambda) = extract_u_angles(&lines[0]);
        let reconstructed = reconstruct_zyz(theta, phi, lambda);
        assert_unitaries_equal_up_to_phase(&y_matrix, &reconstructed);
    }

    // ----- Round-trip QASM text validation -----

    #[test]
    fn test_round_trip_text_validity() {
        // Build a complex circuit with many gate types
        let mut circuit = QuantumCircuit::new(4);
        circuit
            .h(0)
            .x(1)
            .y(2)
            .z(3)
            .s(0)
            .t(1)
            .rx(2, 1.234)
            .ry(3, 2.345)
            .rz(0, 0.567)
            .phase(1, PI / 5.0)
            .cnot(0, 1)
            .cz(2, 3)
            .swap(0, 3)
            .rzz(1, 2, PI / 7.0)
            .barrier()
            .reset(0)
            .measure(0)
            .measure(1)
            .measure(2)
            .measure(3);

        let qasm = to_qasm3(&circuit);

        // Structural checks
        assert_valid_header(&qasm);
        assert!(qasm.contains("qubit[4] q;"));
        assert!(qasm.contains("bit[4] c;"));

        // Every line after the header should be a valid QASM statement
        for line in qasm.lines().skip(4) {
            if line.is_empty() {
                continue;
            }
            assert!(
                line.ends_with(';'),
                "Line should end with semicolon: '{}'",
                line
            );
            // Check it uses valid gate/operation keywords
            let valid_starts = [
                "h ", "x ", "y ", "z ", "s ", "sdg ", "t ", "tdg ", "rx(", "ry(", "rz(", "p(",
                "rzz(", "cx ", "cz ", "swap ", "c[", "reset ", "barrier ", "U(",
            ];
            assert!(
                valid_starts.iter().any(|prefix| line.starts_with(prefix)),
                "Line has unexpected format: '{}'",
                line
            );
        }
    }

    #[test]
    fn test_round_trip_gate_count() {
        let mut circuit = QuantumCircuit::new(2);
        circuit.h(0).cnot(0, 1).measure(0).measure(1);

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);

        // Number of QASM gate lines should match circuit gate count
        assert_eq!(
            lines.len(),
            circuit.gate_count(),
            "Gate line count should match circuit gate count"
        );
    }

    #[test]
    fn test_empty_circuit() {
        let circuit = QuantumCircuit::new(1);
        let qasm = to_qasm3(&circuit);
        assert_valid_header(&qasm);
        assert!(qasm.contains("qubit[1] q;"));
        assert!(qasm.contains("bit[1] c;"));
        let lines = gate_lines(&qasm);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_qubit_indices_in_bounds() {
        // Verify that qubit indices in the output never exceed the register size
        let mut circuit = QuantumCircuit::new(4);
        circuit.h(0).cnot(0, 3).swap(1, 2).measure(3);

        let qasm = to_qasm3(&circuit);
        // Extract all qubit references q[N] and verify N < 4
        for line in qasm.lines().skip(4) {
            let mut remaining = line;
            while let Some(start) = remaining.find("q[") {
                let after_q = &remaining[start + 2..];
                if let Some(end) = after_q.find(']') {
                    let idx_str = &after_q[..end];
                    let idx: u32 = idx_str
                        .parse()
                        .unwrap_or_else(|_| panic!("Invalid qubit index in: '{}'", line));
                    assert!(idx < 4, "Qubit index {} out of bounds in: '{}'", idx, line);
                    remaining = &after_q[end + 1..];
                } else {
                    break;
                }
            }
        }
    }

    #[test]
    fn test_negative_angle() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.rx(0, -PI / 4.0);

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 1);

        let angle = extract_angle(&lines[0]);
        assert!((angle - (-PI / 4.0)).abs() < 1e-10);
    }

    #[test]
    fn test_zero_angle() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.rx(0, 0.0);

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("rx("));
    }

    #[test]
    fn test_sdg_and_tdg_gates() {
        let mut circuit = QuantumCircuit::new(1);
        circuit.add_gate(Gate::Sdg(0));
        circuit.add_gate(Gate::Tdg(0));

        let qasm = to_qasm3(&circuit);
        let lines = gate_lines(&qasm);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "sdg q[0];");
        assert_eq!(lines[1], "tdg q[0];");
    }

    #[test]
    fn test_large_circuit_structure() {
        // A more realistic circuit: QFT-like pattern on 4 qubits
        let mut circuit = QuantumCircuit::new(4);
        for i in 0..4u32 {
            circuit.h(i);
            for j in (i + 1)..4 {
                let angle = PI / (1u32 << (j - i)) as f64;
                circuit.phase(j, angle);
                circuit.cnot(j, i);
            }
        }
        circuit.measure_all();

        let qasm = to_qasm3(&circuit);
        assert_valid_header(&qasm);
        assert!(qasm.contains("qubit[4] q;"));

        // Verify it has at least the H gates and measurements
        let lines = gate_lines(&qasm);
        let h_count = lines.iter().filter(|l| l.starts_with("h ")).count();
        let measure_count = lines.iter().filter(|l| l.contains("measure")).count();
        assert_eq!(h_count, 4);
        assert_eq!(measure_count, 4);
    }

    // ----- Test helpers -----

    /// Extract a single angle from a gate line like `rx(1.234) q[0];`
    fn extract_angle(line: &str) -> f64 {
        let open = line.find('(').expect("No opening parenthesis");
        let close = line.find(')').expect("No closing parenthesis");
        let angle_str = &line[open + 1..close];
        // Handle the case where there are multiple comma-separated angles (take the first)
        let first = angle_str.split(',').next().unwrap().trim();
        first
            .parse::<f64>()
            .unwrap_or_else(|e| panic!("Failed to parse angle '{}': {}", first, e))
    }

    /// Extract (theta, phi, lambda) from a U gate line like `U(t, p, l) q[0];`
    fn extract_u_angles(line: &str) -> (f64, f64, f64) {
        let open = line.find('(').expect("No opening parenthesis");
        let close = line.find(')').expect("No closing parenthesis");
        let inside = &line[open + 1..close];
        let parts: Vec<&str> = inside.split(',').map(|s| s.trim()).collect();
        assert_eq!(
            parts.len(),
            3,
            "U gate should have 3 angles, got: {:?}",
            parts
        );
        let theta: f64 = parts[0].parse().unwrap();
        let phi: f64 = parts[1].parse().unwrap();
        let lambda: f64 = parts[2].parse().unwrap();
        (theta, phi, lambda)
    }

    /// Reconstruct the 2x2 unitary from ZYZ Euler angles:
    /// U = Rz(phi) * Ry(theta) * Rz(lambda)
    fn reconstruct_zyz(theta: f64, phi: f64, lambda: f64) -> [[Complex; 2]; 2] {
        // Rz(a) = [[e^{-ia/2}, 0], [0, e^{ia/2}]]
        // Ry(a) = [[cos(a/2), -sin(a/2)], [sin(a/2), cos(a/2)]]

        let rz = |a: f64| -> [[Complex; 2]; 2] {
            [
                [Complex::from_polar(1.0, -a / 2.0), Complex::ZERO],
                [Complex::ZERO, Complex::from_polar(1.0, a / 2.0)],
            ]
        };

        let ct = (theta / 2.0).cos();
        let st = (theta / 2.0).sin();
        let ry_theta: [[Complex; 2]; 2] = [
            [Complex::new(ct, 0.0), Complex::new(-st, 0.0)],
            [Complex::new(st, 0.0), Complex::new(ct, 0.0)],
        ];

        let rz_phi = rz(phi);
        let rz_lambda = rz(lambda);

        // Multiply: Rz(phi) * Ry(theta)
        let temp = mat_mul(&rz_phi, &ry_theta);
        // Then: temp * Rz(lambda)
        mat_mul(&temp, &rz_lambda)
    }

    /// Multiply two 2x2 complex matrices.
    fn mat_mul(a: &[[Complex; 2]; 2], b: &[[Complex; 2]; 2]) -> [[Complex; 2]; 2] {
        [
            [
                a[0][0] * b[0][0] + a[0][1] * b[1][0],
                a[0][0] * b[0][1] + a[0][1] * b[1][1],
            ],
            [
                a[1][0] * b[0][0] + a[1][1] * b[1][0],
                a[1][0] * b[0][1] + a[1][1] * b[1][1],
            ],
        ]
    }

    /// Assert that two 2x2 unitaries are equal up to a global phase factor.
    ///
    /// Two unitaries U and V are equal up to global phase if there exists
    /// some phase factor e^{i*alpha} such that U = e^{i*alpha} * V.
    ///
    /// We find the phase by looking at the first non-zero element.
    fn assert_unitaries_equal_up_to_phase(
        expected: &[[Complex; 2]; 2],
        actual: &[[Complex; 2]; 2],
    ) {
        let eps = 1e-8;

        // Find the first element with significant magnitude in `expected`
        let mut phase = Complex::ZERO;
        let mut found = false;

        for i in 0..2 {
            for j in 0..2 {
                if expected[i][j].norm() > eps {
                    // phase = actual[i][j] / expected[i][j]
                    // = actual * conj(expected) / |expected|^2
                    let denom = expected[i][j].norm_sq();
                    phase = actual[i][j] * expected[i][j].conj() * (1.0 / denom);
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }

        assert!(found, "Expected matrix is all zeros");

        // Verify the phase has unit magnitude
        assert!(
            (phase.norm() - 1.0).abs() < eps,
            "Phase factor should have unit magnitude, got {}",
            phase.norm()
        );

        // Verify all elements match up to the global phase
        for i in 0..2 {
            for j in 0..2 {
                let scaled = expected[i][j] * phase;
                let diff = (actual[i][j] - scaled).norm();
                assert!(
                    diff < eps,
                    "Mismatch at [{},{}]: expected {} (scaled), got {}. diff={}",
                    i,
                    j,
                    scaled,
                    actual[i][j],
                    diff,
                );
            }
        }
    }
}
