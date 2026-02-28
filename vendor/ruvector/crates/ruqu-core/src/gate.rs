//! Quantum gate definitions and matrix representations

use crate::types::{Complex, QubitIndex};
use std::f64::consts::FRAC_1_SQRT_2;

/// Quantum gate operations
#[derive(Debug, Clone)]
pub enum Gate {
    // ----- Single-qubit gates -----
    H(QubitIndex),
    X(QubitIndex),
    Y(QubitIndex),
    Z(QubitIndex),
    S(QubitIndex),
    Sdg(QubitIndex),
    T(QubitIndex),
    Tdg(QubitIndex),
    Rx(QubitIndex, f64),
    Ry(QubitIndex, f64),
    Rz(QubitIndex, f64),
    Phase(QubitIndex, f64),

    // ----- Two-qubit gates -----
    CNOT(QubitIndex, QubitIndex),
    CZ(QubitIndex, QubitIndex),
    SWAP(QubitIndex, QubitIndex),
    Rzz(QubitIndex, QubitIndex, f64),

    // ----- Special operations -----
    Measure(QubitIndex),
    Reset(QubitIndex),
    Barrier,

    // ----- Fused / custom single-qubit unitary (produced by optimizer) -----
    Unitary1Q(QubitIndex, [[Complex; 2]; 2]),
}

impl Gate {
    /// Return the qubit indices this gate acts on.
    pub fn qubits(&self) -> Vec<QubitIndex> {
        match self {
            Gate::H(q)
            | Gate::X(q)
            | Gate::Y(q)
            | Gate::Z(q)
            | Gate::S(q)
            | Gate::Sdg(q)
            | Gate::T(q)
            | Gate::Tdg(q)
            | Gate::Rx(q, _)
            | Gate::Ry(q, _)
            | Gate::Rz(q, _)
            | Gate::Phase(q, _)
            | Gate::Measure(q)
            | Gate::Reset(q)
            | Gate::Unitary1Q(q, _) => vec![*q],

            Gate::CNOT(q1, q2) | Gate::CZ(q1, q2) | Gate::SWAP(q1, q2) | Gate::Rzz(q1, q2, _) => {
                vec![*q1, *q2]
            }

            Gate::Barrier => vec![],
        }
    }

    /// Returns `true` for non-unitary operations (measurement, reset, barrier).
    pub fn is_non_unitary(&self) -> bool {
        matches!(self, Gate::Measure(_) | Gate::Reset(_) | Gate::Barrier)
    }

    /// Return the 2x2 unitary matrix for single-qubit gates; `None` otherwise.
    pub fn matrix_1q(&self) -> Option<[[Complex; 2]; 2]> {
        let c0 = Complex::ZERO;
        let c1 = Complex::ONE;
        let ci = Complex::I;

        match self {
            // H = (1/sqrt2) [[1, 1], [1, -1]]
            Gate::H(_) => {
                let h = Complex::new(FRAC_1_SQRT_2, 0.0);
                Some([[h, h], [h, -h]])
            }

            // X = [[0, 1], [1, 0]]
            Gate::X(_) => Some([[c0, c1], [c1, c0]]),

            // Y = [[0, -i], [i, 0]]
            Gate::Y(_) => Some([[c0, -ci], [ci, c0]]),

            // Z = [[1, 0], [0, -1]]
            Gate::Z(_) => Some([[c1, c0], [c0, -c1]]),

            // S = [[1, 0], [0, i]]
            Gate::S(_) => Some([[c1, c0], [c0, ci]]),

            // Sdg = [[1, 0], [0, -i]]
            Gate::Sdg(_) => Some([[c1, c0], [c0, -ci]]),

            // T = [[1, 0], [0, e^(i*pi/4)]]
            Gate::T(_) => {
                let t = Complex::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2);
                Some([[c1, c0], [c0, t]])
            }

            // Tdg = [[1, 0], [0, e^(-i*pi/4)]]
            Gate::Tdg(_) => {
                let t = Complex::new(FRAC_1_SQRT_2, -FRAC_1_SQRT_2);
                Some([[c1, c0], [c0, t]])
            }

            // Rx(theta) = [[cos(t/2), -i*sin(t/2)], [-i*sin(t/2), cos(t/2)]]
            Gate::Rx(_, theta) => {
                let half = *theta / 2.0;
                let c = Complex::new(half.cos(), 0.0);
                let s = Complex::new(0.0, -half.sin());
                Some([[c, s], [s, c]])
            }

            // Ry(theta) = [[cos(t/2), -sin(t/2)], [sin(t/2), cos(t/2)]]
            Gate::Ry(_, theta) => {
                let half = *theta / 2.0;
                let cos_h = half.cos();
                let sin_h = half.sin();
                Some([
                    [Complex::new(cos_h, 0.0), Complex::new(-sin_h, 0.0)],
                    [Complex::new(sin_h, 0.0), Complex::new(cos_h, 0.0)],
                ])
            }

            // Rz(theta) = [[e^(-i*t/2), 0], [0, e^(i*t/2)]]
            Gate::Rz(_, theta) => {
                let half = *theta / 2.0;
                Some([
                    [Complex::from_polar(1.0, -half), c0],
                    [c0, Complex::from_polar(1.0, half)],
                ])
            }

            // Phase(theta) = [[1, 0], [0, e^(i*theta)]]
            Gate::Phase(_, theta) => Some([[c1, c0], [c0, Complex::from_polar(1.0, *theta)]]),

            // Custom fused unitary
            Gate::Unitary1Q(_, m) => Some(*m),

            // Not a single-qubit gate
            _ => None,
        }
    }

    /// Return the 4x4 unitary matrix for two-qubit gates; `None` otherwise.
    ///
    /// Row / column ordering: index = q1_bit * 2 + q2_bit
    /// where q1 is the first qubit argument and q2 the second.
    pub fn matrix_2q(&self) -> Option<[[Complex; 4]; 4]> {
        let c0 = Complex::ZERO;
        let c1 = Complex::ONE;

        match self {
            // CNOT(control, target): |c,t> -> |c, t XOR c>
            // Rows: |00>, |01>, |10>, |11>  (control, target)
            Gate::CNOT(_, _) => Some([
                [c1, c0, c0, c0],
                [c0, c1, c0, c0],
                [c0, c0, c0, c1],
                [c0, c0, c1, c0],
            ]),

            // CZ: diag(1, 1, 1, -1)
            Gate::CZ(_, _) => Some([
                [c1, c0, c0, c0],
                [c0, c1, c0, c0],
                [c0, c0, c1, c0],
                [c0, c0, c0, -c1],
            ]),

            // SWAP: identity with rows 1 and 2 exchanged
            Gate::SWAP(_, _) => Some([
                [c1, c0, c0, c0],
                [c0, c0, c1, c0],
                [c0, c1, c0, c0],
                [c0, c0, c0, c1],
            ]),

            // Rzz(theta): diag(e^{-it/2}, e^{it/2}, e^{it/2}, e^{-it/2})
            Gate::Rzz(_, _, theta) => {
                let half = *theta / 2.0;
                let en = Complex::from_polar(1.0, -half);
                let ep = Complex::from_polar(1.0, half);
                Some([
                    [en, c0, c0, c0],
                    [c0, ep, c0, c0],
                    [c0, c0, ep, c0],
                    [c0, c0, c0, en],
                ])
            }

            _ => None,
        }
    }
}
