//! Tests for ruqu_core::gate — gate matrix correctness and unitarity.

use ruqu_core::gate::Gate;
use ruqu_core::types::Complex;

const EPSILON: f64 = 1e-10;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON
}

fn complex_approx_eq(a: &Complex, b: &Complex) -> bool {
    approx_eq(a.re, b.re) && approx_eq(a.im, b.im)
}

/// Convenience: build a Complex value.
fn c(re: f64, im: f64) -> Complex {
    Complex { re, im }
}

/// Check that a 2x2 matrix satisfies U^dag * U = I (unitarity).
fn assert_unitary_2x2(m: &[[Complex; 2]; 2]) {
    // Flatten to make indexing easier: flat[i*2+j] = m[i][j]
    let flat = [m[0][0], m[0][1], m[1][0], m[1][1]];
    // U^dag * U should equal identity
    let udu = [
        // (0,0)
        flat[0].conj() * flat[0] + flat[2].conj() * flat[2],
        // (0,1)
        flat[0].conj() * flat[1] + flat[2].conj() * flat[3],
        // (1,0)
        flat[1].conj() * flat[0] + flat[3].conj() * flat[2],
        // (1,1)
        flat[1].conj() * flat[1] + flat[3].conj() * flat[3],
    ];
    assert!(
        complex_approx_eq(&udu[0], &c(1.0, 0.0)),
        "U^dag U [0,0] = {:?}, expected 1",
        udu[0]
    );
    assert!(
        complex_approx_eq(&udu[1], &c(0.0, 0.0)),
        "U^dag U [0,1] = {:?}, expected 0",
        udu[1]
    );
    assert!(
        complex_approx_eq(&udu[2], &c(0.0, 0.0)),
        "U^dag U [1,0] = {:?}, expected 0",
        udu[2]
    );
    assert!(
        complex_approx_eq(&udu[3], &c(1.0, 0.0)),
        "U^dag U [1,1] = {:?}, expected 1",
        udu[3]
    );
}

/// Check unitarity for a 4x4 matrix.
fn assert_unitary_4x4(m: &[[Complex; 4]; 4]) {
    for i in 0..4 {
        for j in 0..4 {
            let mut sum = c(0.0, 0.0);
            for k in 0..4 {
                // (U^dag)_{ik} = conj(U_{ki})
                let u_dag_ik = m[k][i].conj();
                let u_kj = m[k][j];
                sum = sum + u_dag_ik * u_kj;
            }
            let expected = if i == j { c(1.0, 0.0) } else { c(0.0, 0.0) };
            assert!(
                complex_approx_eq(&sum, &expected),
                "U^dag U [{},{}] = ({}, {}), expected ({}, {})",
                i,
                j,
                sum.re,
                sum.im,
                expected.re,
                expected.im
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Hadamard gate: H = 1/sqrt(2) * [[1, 1], [1, -1]]
// ---------------------------------------------------------------------------

#[test]
fn test_hadamard_matrix() {
    let matrix = Gate::H(0).matrix_1q().expect("H should have a 2x2 matrix");
    let s = std::f64::consts::FRAC_1_SQRT_2;

    assert!(complex_approx_eq(&matrix[0][0], &c(s, 0.0))); // [0,0]
    assert!(complex_approx_eq(&matrix[0][1], &c(s, 0.0))); // [0,1]
    assert!(complex_approx_eq(&matrix[1][0], &c(s, 0.0))); // [1,0]
    assert!(complex_approx_eq(&matrix[1][1], &c(-s, 0.0))); // [1,1]
}

#[test]
fn test_hadamard_is_self_inverse() {
    // H * H = I
    let m = Gate::H(0).matrix_1q().unwrap();
    // Multiply m * m
    let prod = [
        m[0][0] * m[0][0] + m[0][1] * m[1][0],
        m[0][0] * m[0][1] + m[0][1] * m[1][1],
        m[1][0] * m[0][0] + m[1][1] * m[1][0],
        m[1][0] * m[0][1] + m[1][1] * m[1][1],
    ];
    assert!(complex_approx_eq(&prod[0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&prod[1], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&prod[2], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&prod[3], &c(1.0, 0.0)));
}

#[test]
fn test_hadamard_unitarity() {
    let m = Gate::H(0).matrix_1q().unwrap();
    assert_unitary_2x2(&m);
}

// ---------------------------------------------------------------------------
// Pauli-X gate: [[0, 1], [1, 0]]
// ---------------------------------------------------------------------------

#[test]
fn test_pauli_x_matrix() {
    let m = Gate::X(0).matrix_1q().expect("X should have a 2x2 matrix");
    assert!(complex_approx_eq(&m[0][0], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[0][1], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&m[1][0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&m[1][1], &c(0.0, 0.0)));
}

#[test]
fn test_pauli_x_is_self_inverse() {
    let m = Gate::X(0).matrix_1q().unwrap();
    let prod = [
        m[0][0] * m[0][0] + m[0][1] * m[1][0],
        m[0][0] * m[0][1] + m[0][1] * m[1][1],
        m[1][0] * m[0][0] + m[1][1] * m[1][0],
        m[1][0] * m[0][1] + m[1][1] * m[1][1],
    ];
    assert!(complex_approx_eq(&prod[0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&prod[3], &c(1.0, 0.0)));
}

#[test]
fn test_pauli_x_unitarity() {
    let m = Gate::X(0).matrix_1q().unwrap();
    assert_unitary_2x2(&m);
}

// ---------------------------------------------------------------------------
// Pauli-Y gate: [[0, -i], [i, 0]]
// ---------------------------------------------------------------------------

#[test]
fn test_pauli_y_matrix() {
    let m = Gate::Y(0).matrix_1q().expect("Y should have a 2x2 matrix");
    assert!(complex_approx_eq(&m[0][0], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[0][1], &c(0.0, -1.0)));
    assert!(complex_approx_eq(&m[1][0], &c(0.0, 1.0)));
    assert!(complex_approx_eq(&m[1][1], &c(0.0, 0.0)));
}

#[test]
fn test_pauli_y_is_self_inverse() {
    let m = Gate::Y(0).matrix_1q().unwrap();
    let prod = [
        m[0][0] * m[0][0] + m[0][1] * m[1][0],
        m[0][0] * m[0][1] + m[0][1] * m[1][1],
        m[1][0] * m[0][0] + m[1][1] * m[1][0],
        m[1][0] * m[0][1] + m[1][1] * m[1][1],
    ];
    assert!(complex_approx_eq(&prod[0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&prod[3], &c(1.0, 0.0)));
}

#[test]
fn test_pauli_y_unitarity() {
    let m = Gate::Y(0).matrix_1q().unwrap();
    assert_unitary_2x2(&m);
}

// ---------------------------------------------------------------------------
// Pauli-Z gate: [[1, 0], [0, -1]]
// ---------------------------------------------------------------------------

#[test]
fn test_pauli_z_matrix() {
    let m = Gate::Z(0).matrix_1q().expect("Z should have a 2x2 matrix");
    assert!(complex_approx_eq(&m[0][0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&m[0][1], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[1][0], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[1][1], &c(-1.0, 0.0)));
}

#[test]
fn test_pauli_z_is_self_inverse() {
    let m = Gate::Z(0).matrix_1q().unwrap();
    let prod = [
        m[0][0] * m[0][0] + m[0][1] * m[1][0],
        m[0][0] * m[0][1] + m[0][1] * m[1][1],
        m[1][0] * m[0][0] + m[1][1] * m[1][0],
        m[1][0] * m[0][1] + m[1][1] * m[1][1],
    ];
    assert!(complex_approx_eq(&prod[0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&prod[3], &c(1.0, 0.0)));
}

#[test]
fn test_pauli_z_unitarity() {
    let m = Gate::Z(0).matrix_1q().unwrap();
    assert_unitary_2x2(&m);
}

// ---------------------------------------------------------------------------
// Pauli algebra: X*Y = iZ, Y*Z = iX, Z*X = iY
// ---------------------------------------------------------------------------

#[test]
fn test_pauli_xy_equals_iz() {
    let x = Gate::X(0).matrix_1q().unwrap();
    let y = Gate::Y(0).matrix_1q().unwrap();
    let z = Gate::Z(0).matrix_1q().unwrap();

    // X * Y (2x2 matrix multiply)
    let xy = [
        [
            x[0][0] * y[0][0] + x[0][1] * y[1][0],
            x[0][0] * y[0][1] + x[0][1] * y[1][1],
        ],
        [
            x[1][0] * y[0][0] + x[1][1] * y[1][0],
            x[1][0] * y[0][1] + x[1][1] * y[1][1],
        ],
    ];
    // i * Z
    let iz = [
        [c(0.0, 1.0) * z[0][0], c(0.0, 1.0) * z[0][1]],
        [c(0.0, 1.0) * z[1][0], c(0.0, 1.0) * z[1][1]],
    ];
    for i in 0..2 {
        for j in 0..2 {
            assert!(
                complex_approx_eq(&xy[i][j], &iz[i][j]),
                "XY[{},{}] = ({}, {}), iZ[{},{}] = ({}, {})",
                i,
                j,
                xy[i][j].re,
                xy[i][j].im,
                i,
                j,
                iz[i][j].re,
                iz[i][j].im
            );
        }
    }
}

// ---------------------------------------------------------------------------
// S gate: [[1, 0], [0, i]]
// ---------------------------------------------------------------------------

#[test]
fn test_s_gate_matrix() {
    let m = Gate::S(0).matrix_1q().expect("S should have a 2x2 matrix");
    assert!(complex_approx_eq(&m[0][0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&m[0][1], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[1][0], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[1][1], &c(0.0, 1.0)));
}

#[test]
fn test_s_gate_unitarity() {
    let m = Gate::S(0).matrix_1q().unwrap();
    assert_unitary_2x2(&m);
}

#[test]
fn test_s_squared_is_z() {
    // S^2 = Z
    let s = Gate::S(0).matrix_1q().unwrap();
    let z = Gate::Z(0).matrix_1q().unwrap();
    let s2 = [
        [
            s[0][0] * s[0][0] + s[0][1] * s[1][0],
            s[0][0] * s[0][1] + s[0][1] * s[1][1],
        ],
        [
            s[1][0] * s[0][0] + s[1][1] * s[1][0],
            s[1][0] * s[0][1] + s[1][1] * s[1][1],
        ],
    ];
    for i in 0..2 {
        for j in 0..2 {
            assert!(
                complex_approx_eq(&s2[i][j], &z[i][j]),
                "S^2[{},{}] != Z[{},{}]",
                i,
                j,
                i,
                j
            );
        }
    }
}

// ---------------------------------------------------------------------------
// T gate: [[1, 0], [0, e^{i*pi/4}]]
// ---------------------------------------------------------------------------

#[test]
fn test_t_gate_matrix() {
    let m = Gate::T(0).matrix_1q().expect("T should have a 2x2 matrix");
    let phase = std::f64::consts::FRAC_PI_4;
    assert!(complex_approx_eq(&m[0][0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&m[0][1], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[1][0], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[1][1], &c(phase.cos(), phase.sin())));
}

#[test]
fn test_t_gate_unitarity() {
    let m = Gate::T(0).matrix_1q().unwrap();
    assert_unitary_2x2(&m);
}

#[test]
fn test_t_squared_is_s() {
    // T^2 = S
    let t = Gate::T(0).matrix_1q().unwrap();
    let s = Gate::S(0).matrix_1q().unwrap();
    let t2 = [
        [
            t[0][0] * t[0][0] + t[0][1] * t[1][0],
            t[0][0] * t[0][1] + t[0][1] * t[1][1],
        ],
        [
            t[1][0] * t[0][0] + t[1][1] * t[1][0],
            t[1][0] * t[0][1] + t[1][1] * t[1][1],
        ],
    ];
    for i in 0..2 {
        for j in 0..2 {
            assert!(
                complex_approx_eq(&t2[i][j], &s[i][j]),
                "T^2[{},{}] != S[{},{}]",
                i,
                j,
                i,
                j
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rotation gates: Rx, Ry, Rz
// ---------------------------------------------------------------------------

#[test]
fn test_rx_zero_is_identity() {
    let m = Gate::Rx(0, 0.0).matrix_1q().unwrap();
    assert!(complex_approx_eq(&m[0][0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&m[0][1], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[1][0], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[1][1], &c(1.0, 0.0)));
}

#[test]
fn test_rx_pi_matrix() {
    // Rx(pi) = [[cos(pi/2), -i*sin(pi/2)], [-i*sin(pi/2), cos(pi/2)]]
    //        = [[0, -i], [-i, 0]]
    let m = Gate::Rx(0, std::f64::consts::PI).matrix_1q().unwrap();
    assert!(complex_approx_eq(&m[0][0], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[0][1], &c(0.0, -1.0)));
    assert!(complex_approx_eq(&m[1][0], &c(0.0, -1.0)));
    assert!(complex_approx_eq(&m[1][1], &c(0.0, 0.0)));
}

#[test]
fn test_rx_unitarity() {
    let m = Gate::Rx(0, 1.234).matrix_1q().unwrap();
    assert_unitary_2x2(&m);
}

#[test]
fn test_ry_zero_is_identity() {
    let m = Gate::Ry(0, 0.0).matrix_1q().unwrap();
    assert!(complex_approx_eq(&m[0][0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&m[1][1], &c(1.0, 0.0)));
}

#[test]
fn test_ry_pi_matrix() {
    // Ry(pi) = [[cos(pi/2), -sin(pi/2)], [sin(pi/2), cos(pi/2)]]
    //        = [[0, -1], [1, 0]]
    let m = Gate::Ry(0, std::f64::consts::PI).matrix_1q().unwrap();
    assert!(complex_approx_eq(&m[0][0], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[0][1], &c(-1.0, 0.0)));
    assert!(complex_approx_eq(&m[1][0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&m[1][1], &c(0.0, 0.0)));
}

#[test]
fn test_ry_unitarity() {
    let m = Gate::Ry(0, 2.718).matrix_1q().unwrap();
    assert_unitary_2x2(&m);
}

#[test]
fn test_rz_zero_is_identity() {
    let m = Gate::Rz(0, 0.0).matrix_1q().unwrap();
    assert!(complex_approx_eq(&m[0][0], &c(1.0, 0.0)));
    assert!(complex_approx_eq(&m[1][1], &c(1.0, 0.0)));
}

#[test]
fn test_rz_pi_matrix() {
    // Rz(pi) = [[e^{-i*pi/2}, 0], [0, e^{i*pi/2}]] = [[-i, 0], [0, i]]
    let m = Gate::Rz(0, std::f64::consts::PI).matrix_1q().unwrap();
    assert!(complex_approx_eq(&m[0][0], &c(0.0, -1.0)));
    assert!(complex_approx_eq(&m[0][1], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[1][0], &c(0.0, 0.0)));
    assert!(complex_approx_eq(&m[1][1], &c(0.0, 1.0)));
}

#[test]
fn test_rz_unitarity() {
    let m = Gate::Rz(0, 0.789).matrix_1q().unwrap();
    assert_unitary_2x2(&m);
}

#[test]
fn test_rotation_gates_various_angles_unitary() {
    let angles = [
        0.0,
        0.1,
        0.5,
        1.0,
        std::f64::consts::PI,
        2.0 * std::f64::consts::PI,
        -0.7,
    ];
    for &theta in &angles {
        let rx = Gate::Rx(0, theta).matrix_1q().unwrap();
        assert_unitary_2x2(&rx);

        let ry = Gate::Ry(0, theta).matrix_1q().unwrap();
        assert_unitary_2x2(&ry);

        let rz = Gate::Rz(0, theta).matrix_1q().unwrap();
        assert_unitary_2x2(&rz);
    }
}

// ---------------------------------------------------------------------------
// CNOT gate: 4x4 matrix
// ---------------------------------------------------------------------------

#[test]
fn test_cnot_matrix() {
    // CNOT = [[1,0,0,0],[0,1,0,0],[0,0,0,1],[0,0,1,0]]
    let m = Gate::CNOT(0, 1)
        .matrix_2q()
        .expect("CNOT should have a 4x4 matrix");

    let expected: [[Complex; 4]; 4] = [
        [c(1.0, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(0.0, 0.0)], // row 0
        [c(0.0, 0.0), c(1.0, 0.0), c(0.0, 0.0), c(0.0, 0.0)], // row 1
        [c(0.0, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(1.0, 0.0)], // row 2
        [c(0.0, 0.0), c(0.0, 0.0), c(1.0, 0.0), c(0.0, 0.0)], // row 3
    ];

    for i in 0..4 {
        for j in 0..4 {
            assert!(
                complex_approx_eq(&m[i][j], &expected[i][j]),
                "CNOT matrix[{}][{}]: got ({}, {}), expected ({}, {})",
                i,
                j,
                m[i][j].re,
                m[i][j].im,
                expected[i][j].re,
                expected[i][j].im
            );
        }
    }
}

#[test]
fn test_cnot_unitarity() {
    let m = Gate::CNOT(0, 1).matrix_2q().unwrap();
    assert_unitary_4x4(&m);
}

#[test]
fn test_cnot_is_self_inverse() {
    // CNOT * CNOT = I
    let m = Gate::CNOT(0, 1).matrix_2q().unwrap();
    for i in 0..4 {
        for j in 0..4 {
            let mut sum = c(0.0, 0.0);
            for k in 0..4 {
                sum = sum + m[i][k] * m[k][j];
            }
            let expected = if i == j { c(1.0, 0.0) } else { c(0.0, 0.0) };
            assert!(
                complex_approx_eq(&sum, &expected),
                "CNOT^2 [{},{}] = ({}, {}), expected ({}, {})",
                i,
                j,
                sum.re,
                sum.im,
                expected.re,
                expected.im
            );
        }
    }
}

// ---------------------------------------------------------------------------
// CZ gate: 4x4 matrix  diag(1, 1, 1, -1)
// ---------------------------------------------------------------------------

#[test]
fn test_cz_matrix() {
    let m = Gate::CZ(0, 1)
        .matrix_2q()
        .expect("CZ should have a 4x4 matrix");

    // CZ = diag(1, 1, 1, -1)
    for i in 0..4 {
        for j in 0..4 {
            let expected = if i == j {
                if i == 3 {
                    c(-1.0, 0.0)
                } else {
                    c(1.0, 0.0)
                }
            } else {
                c(0.0, 0.0)
            };
            assert!(
                complex_approx_eq(&m[i][j], &expected),
                "CZ[{},{}] mismatch",
                i,
                j
            );
        }
    }
}

#[test]
fn test_cz_unitarity() {
    let m = Gate::CZ(0, 1).matrix_2q().unwrap();
    assert_unitary_4x4(&m);
}

#[test]
fn test_cz_is_symmetric() {
    // CZ(0,1) should equal CZ(1,0) — the gate is symmetric in control/target
    let m01 = Gate::CZ(0, 1).matrix_2q().unwrap();
    let m10 = Gate::CZ(1, 0).matrix_2q().unwrap();
    for i in 0..4 {
        for j in 0..4 {
            assert!(
                complex_approx_eq(&m01[i][j], &m10[i][j]),
                "CZ symmetry mismatch at [{},{}]",
                i,
                j
            );
        }
    }
}

// ---------------------------------------------------------------------------
// SWAP gate: 4x4 matrix
// ---------------------------------------------------------------------------

#[test]
fn test_swap_matrix() {
    // SWAP = [[1,0,0,0],[0,0,1,0],[0,1,0,0],[0,0,0,1]]
    let m = Gate::SWAP(0, 1)
        .matrix_2q()
        .expect("SWAP should have a 4x4 matrix");

    let expected: [[Complex; 4]; 4] = [
        [c(1.0, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(0.0, 0.0)], // row 0
        [c(0.0, 0.0), c(0.0, 0.0), c(1.0, 0.0), c(0.0, 0.0)], // row 1
        [c(0.0, 0.0), c(1.0, 0.0), c(0.0, 0.0), c(0.0, 0.0)], // row 2
        [c(0.0, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(1.0, 0.0)], // row 3
    ];

    for i in 0..4 {
        for j in 0..4 {
            assert!(
                complex_approx_eq(&m[i][j], &expected[i][j]),
                "SWAP matrix[{}][{}] mismatch",
                i,
                j
            );
        }
    }
}

#[test]
fn test_swap_unitarity() {
    let m = Gate::SWAP(0, 1).matrix_2q().unwrap();
    assert_unitary_4x4(&m);
}

#[test]
fn test_swap_is_self_inverse() {
    // SWAP * SWAP = I
    let m = Gate::SWAP(0, 1).matrix_2q().unwrap();
    for i in 0..4 {
        for j in 0..4 {
            let mut sum = c(0.0, 0.0);
            for k in 0..4 {
                sum = sum + m[i][k] * m[k][j];
            }
            let expected = if i == j { c(1.0, 0.0) } else { c(0.0, 0.0) };
            assert!(
                complex_approx_eq(&sum, &expected),
                "SWAP^2 [{},{}] mismatch",
                i,
                j
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Gate qubit index extraction
// ---------------------------------------------------------------------------

#[test]
fn test_gate_qubit_indices() {
    // Single-qubit gates should report their target qubit via qubits().
    assert_eq!(Gate::H(3).qubits(), vec![3]);
    assert_eq!(Gate::X(0).qubits(), vec![0]);
    assert_eq!(Gate::Y(7).qubits(), vec![7]);
    assert_eq!(Gate::Z(15).qubits(), vec![15]);
    assert_eq!(Gate::Rx(2, 0.5).qubits(), vec![2]);
    assert_eq!(Gate::Ry(4, 1.0).qubits(), vec![4]);
    assert_eq!(Gate::Rz(6, 0.3).qubits(), vec![6]);
}

#[test]
fn test_two_qubit_gate_indices() {
    // Two-qubit gates should report both qubits.
    let qubits = Gate::CNOT(0, 1).qubits();
    assert_eq!(qubits.len(), 2);
    assert_eq!(qubits[0], 0);
    assert_eq!(qubits[1], 1);

    let qubits2 = Gate::CNOT(5, 3).qubits();
    assert_eq!(qubits2[0], 5);
    assert_eq!(qubits2[1], 3);
}
