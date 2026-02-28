//! Tests for ruqu_core::types — Complex arithmetic, PauliString, Hamiltonian.

use ruqu_core::types::*;

const EPSILON: f64 = 1e-10;

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON
}

// ---------------------------------------------------------------------------
// Complex – basic construction
// ---------------------------------------------------------------------------

#[test]
fn test_complex_zero() {
    let z = Complex { re: 0.0, im: 0.0 };
    assert!(approx_eq(z.re, 0.0));
    assert!(approx_eq(z.im, 0.0));
}

#[test]
fn test_complex_real_only() {
    let z = Complex { re: 3.0, im: 0.0 };
    assert!(approx_eq(z.re, 3.0));
    assert!(approx_eq(z.im, 0.0));
}

#[test]
fn test_complex_imaginary_only() {
    let z = Complex { re: 0.0, im: 4.0 };
    assert!(approx_eq(z.re, 0.0));
    assert!(approx_eq(z.im, 4.0));
}

// ---------------------------------------------------------------------------
// Complex – arithmetic operations
// ---------------------------------------------------------------------------

#[test]
fn test_complex_addition() {
    let a = Complex { re: 1.0, im: 2.0 };
    let b = Complex { re: 3.0, im: -1.0 };
    let c = a + b;
    assert!(approx_eq(c.re, 4.0));
    assert!(approx_eq(c.im, 1.0));
}

#[test]
fn test_complex_subtraction() {
    let a = Complex { re: 5.0, im: 3.0 };
    let b = Complex { re: 2.0, im: 7.0 };
    let c = a - b;
    assert!(approx_eq(c.re, 3.0));
    assert!(approx_eq(c.im, -4.0));
}

#[test]
fn test_complex_multiplication() {
    // (1+2i)*(3+4i) = 3+4i+6i+8i^2 = (3-8)+(4+6)i = -5+10i
    let a = Complex { re: 1.0, im: 2.0 };
    let b = Complex { re: 3.0, im: 4.0 };
    let c = a * b;
    assert!(approx_eq(c.re, -5.0));
    assert!(approx_eq(c.im, 10.0));
}

#[test]
fn test_complex_multiplication_real() {
    // (2+3i) * (4+0i) = 8+12i
    let a = Complex { re: 2.0, im: 3.0 };
    let b = Complex { re: 4.0, im: 0.0 };
    let c = a * b;
    assert!(approx_eq(c.re, 8.0));
    assert!(approx_eq(c.im, 12.0));
}

#[test]
fn test_complex_multiplication_imaginary() {
    // (0+1i) * (0+1i) = -1+0i
    let a = Complex { re: 0.0, im: 1.0 };
    let c = a * a;
    assert!(approx_eq(c.re, -1.0));
    assert!(approx_eq(c.im, 0.0));
}

#[test]
fn test_complex_negation() {
    let a = Complex { re: 3.0, im: -4.0 };
    let b = -a;
    assert!(approx_eq(b.re, -3.0));
    assert!(approx_eq(b.im, 4.0));
}

#[test]
fn test_complex_conjugate() {
    let a = Complex { re: 3.0, im: 4.0 };
    let c = a.conj();
    assert!(approx_eq(c.re, 3.0));
    assert!(approx_eq(c.im, -4.0));
}

#[test]
fn test_complex_conjugate_real() {
    let a = Complex { re: 5.0, im: 0.0 };
    let c = a.conj();
    assert!(approx_eq(c.re, 5.0));
    assert!(approx_eq(c.im, 0.0));
}

// ---------------------------------------------------------------------------
// Complex – norm / magnitude
// ---------------------------------------------------------------------------

#[test]
fn test_complex_norm_sq() {
    // |3+4i|^2 = 9+16 = 25
    let a = Complex { re: 3.0, im: 4.0 };
    assert!(approx_eq(a.norm_sq(), 25.0));
}

#[test]
fn test_complex_norm() {
    // |3+4i| = 5
    let a = Complex { re: 3.0, im: 4.0 };
    assert!(approx_eq(a.norm(), 5.0));
}

#[test]
fn test_complex_norm_zero() {
    let z = Complex { re: 0.0, im: 0.0 };
    assert!(approx_eq(z.norm(), 0.0));
}

#[test]
fn test_complex_unit_norm() {
    // e^{i*pi/4} has norm 1
    let angle = std::f64::consts::FRAC_PI_4;
    let z = Complex {
        re: angle.cos(),
        im: angle.sin(),
    };
    assert!(approx_eq(z.norm(), 1.0));
}

// ---------------------------------------------------------------------------
// Complex – from_polar
// ---------------------------------------------------------------------------

#[test]
fn test_complex_from_polar_zero_angle() {
    let z = Complex::from_polar(2.0, 0.0);
    assert!(approx_eq(z.re, 2.0));
    assert!(approx_eq(z.im, 0.0));
}

#[test]
fn test_complex_from_polar_pi_half() {
    let z = Complex::from_polar(1.0, std::f64::consts::FRAC_PI_2);
    assert!(approx_eq(z.re, 0.0));
    assert!(approx_eq(z.im, 1.0));
}

#[test]
fn test_complex_from_polar_pi() {
    let z = Complex::from_polar(1.0, std::f64::consts::PI);
    assert!(approx_eq(z.re, -1.0));
    assert!(approx_eq(z.im, 0.0));
}

#[test]
fn test_complex_from_polar_three_pi_half() {
    let z = Complex::from_polar(1.0, 3.0 * std::f64::consts::FRAC_PI_2);
    assert!(approx_eq(z.re, 0.0));
    assert!(approx_eq(z.im, -1.0));
}

#[test]
fn test_complex_from_polar_roundtrip() {
    let r = 3.5;
    let theta = 1.23;
    let z = Complex::from_polar(r, theta);
    assert!(approx_eq(z.norm(), r));
}

// ---------------------------------------------------------------------------
// Complex – algebraic identities
// ---------------------------------------------------------------------------

#[test]
fn test_complex_mul_conjugate_is_norm_sq() {
    // z * conj(z) = |z|^2 (real)
    let z = Complex { re: 2.0, im: -7.0 };
    let product = z * z.conj();
    assert!(approx_eq(product.re, z.norm_sq()));
    assert!(approx_eq(product.im, 0.0));
}

#[test]
fn test_complex_addition_commutativity() {
    let a = Complex { re: 1.5, im: -2.3 };
    let b = Complex { re: -0.7, im: 4.1 };
    let ab = a + b;
    let ba = b + a;
    assert!(approx_eq(ab.re, ba.re));
    assert!(approx_eq(ab.im, ba.im));
}

#[test]
fn test_complex_multiplication_commutativity() {
    let a = Complex { re: 1.5, im: -2.3 };
    let b = Complex { re: -0.7, im: 4.1 };
    let ab = a * b;
    let ba = b * a;
    assert!(approx_eq(ab.re, ba.re));
    assert!(approx_eq(ab.im, ba.im));
}

#[test]
fn test_complex_distributivity() {
    // a*(b+c) = a*b + a*c
    let a = Complex { re: 1.0, im: 2.0 };
    let b = Complex { re: 3.0, im: -1.0 };
    let c = Complex { re: -2.0, im: 0.5 };
    let lhs = a * (b + c);
    let rhs = a * b + a * c;
    assert!(approx_eq(lhs.re, rhs.re));
    assert!(approx_eq(lhs.im, rhs.im));
}

// ---------------------------------------------------------------------------
// PauliOp
// ---------------------------------------------------------------------------

#[test]
fn test_pauli_op_variants_exist() {
    // Ensure all four Pauli operators can be constructed.
    let _i = PauliOp::I;
    let _x = PauliOp::X;
    let _y = PauliOp::Y;
    let _z = PauliOp::Z;
}

// ---------------------------------------------------------------------------
// PauliString
// ---------------------------------------------------------------------------

#[test]
fn test_pauli_string_single_z() {
    let ps = PauliString {
        ops: vec![(0, PauliOp::Z)],
    };
    assert_eq!(ps.ops.len(), 1);
    assert_eq!(ps.ops[0].0, 0);
}

#[test]
fn test_pauli_string_zz() {
    let ps = PauliString {
        ops: vec![(0, PauliOp::Z), (1, PauliOp::Z)],
    };
    assert_eq!(ps.ops.len(), 2);
}

#[test]
fn test_pauli_string_xx() {
    let ps = PauliString {
        ops: vec![(0, PauliOp::X), (1, PauliOp::X)],
    };
    assert_eq!(ps.ops.len(), 2);
}

#[test]
fn test_pauli_string_mixed() {
    let ps = PauliString {
        ops: vec![
            (0, PauliOp::X),
            (1, PauliOp::Y),
            (2, PauliOp::Z),
            (3, PauliOp::I),
        ],
    };
    assert_eq!(ps.ops.len(), 4);
}

#[test]
fn test_pauli_string_empty() {
    // An empty Pauli string acts as the identity on all qubits.
    let ps = PauliString { ops: vec![] };
    assert_eq!(ps.ops.len(), 0);
}

#[test]
fn test_pauli_string_high_qubit_index() {
    let ps = PauliString {
        ops: vec![(15, PauliOp::Z)],
    };
    assert_eq!(ps.ops[0].0, 15);
}

// ---------------------------------------------------------------------------
// Hamiltonian
// ---------------------------------------------------------------------------

#[test]
fn test_hamiltonian_single_term() {
    let h = Hamiltonian {
        terms: vec![(
            1.0,
            PauliString {
                ops: vec![(0, PauliOp::Z)],
            },
        )],
        num_qubits: 1,
    };
    assert_eq!(h.terms.len(), 1);
    assert_eq!(h.num_qubits, 1);
}

#[test]
fn test_hamiltonian_multiple_terms() {
    // H = 0.5*Z0 + 0.3*Z1 + 0.2*Z0Z1
    let h = Hamiltonian {
        terms: vec![
            (
                0.5,
                PauliString {
                    ops: vec![(0, PauliOp::Z)],
                },
            ),
            (
                0.3,
                PauliString {
                    ops: vec![(1, PauliOp::Z)],
                },
            ),
            (
                0.2,
                PauliString {
                    ops: vec![(0, PauliOp::Z), (1, PauliOp::Z)],
                },
            ),
        ],
        num_qubits: 2,
    };
    assert_eq!(h.terms.len(), 3);
    assert_eq!(h.num_qubits, 2);
}

#[test]
fn test_hamiltonian_with_negative_coefficients() {
    let h = Hamiltonian {
        terms: vec![
            (
                -0.5,
                PauliString {
                    ops: vec![(0, PauliOp::Z)],
                },
            ),
            (
                1.2,
                PauliString {
                    ops: vec![(0, PauliOp::X), (1, PauliOp::X)],
                },
            ),
        ],
        num_qubits: 2,
    };
    assert!(approx_eq(h.terms[0].0, -0.5));
    assert!(approx_eq(h.terms[1].0, 1.2));
}

#[test]
fn test_hamiltonian_empty() {
    let h = Hamiltonian {
        terms: vec![],
        num_qubits: 0,
    };
    assert_eq!(h.terms.len(), 0);
}

#[test]
fn test_hamiltonian_ising_model() {
    // Simple 3-qubit Ising: H = -J * sum(Z_i Z_{i+1}) - h * sum(X_i)
    let j = 1.0_f64;
    let field = 0.5_f64;
    let h = Hamiltonian {
        terms: vec![
            (
                -j,
                PauliString {
                    ops: vec![(0, PauliOp::Z), (1, PauliOp::Z)],
                },
            ),
            (
                -j,
                PauliString {
                    ops: vec![(1, PauliOp::Z), (2, PauliOp::Z)],
                },
            ),
            (
                -field,
                PauliString {
                    ops: vec![(0, PauliOp::X)],
                },
            ),
            (
                -field,
                PauliString {
                    ops: vec![(1, PauliOp::X)],
                },
            ),
            (
                -field,
                PauliString {
                    ops: vec![(2, PauliOp::X)],
                },
            ),
        ],
        num_qubits: 3,
    };
    assert_eq!(h.terms.len(), 5);
    assert_eq!(h.num_qubits, 3);
}
