//! Core types for the ruQu quantum simulation engine

use std::fmt;
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Complex number for quantum amplitudes (f64 precision)
#[derive(Clone, Copy, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
    pub const ONE: Self = Self { re: 1.0, im: 0.0 };
    pub const I: Self = Self { re: 0.0, im: 1.0 };

    #[inline]
    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    #[inline]
    pub fn from_polar(r: f64, theta: f64) -> Self {
        Self {
            re: r * theta.cos(),
            im: r * theta.sin(),
        }
    }

    #[inline]
    pub fn norm_sq(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    #[inline]
    pub fn norm(&self) -> f64 {
        self.norm_sq().sqrt()
    }

    #[inline]
    pub fn conj(&self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }

    #[inline]
    pub fn arg(&self) -> f64 {
        self.im.atan2(self.re)
    }
}

// ---------------------------------------------------------------------------
// Arithmetic trait implementations
// ---------------------------------------------------------------------------

impl Add for Complex {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl Sub for Complex {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl Mul for Complex {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl Neg for Complex {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            re: -self.re,
            im: -self.im,
        }
    }
}

impl AddAssign for Complex {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.re += rhs.re;
        self.im += rhs.im;
    }
}

impl SubAssign for Complex {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.re -= rhs.re;
        self.im -= rhs.im;
    }
}

impl MulAssign for Complex {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        let re = self.re * rhs.re - self.im * rhs.im;
        let im = self.re * rhs.im + self.im * rhs.re;
        self.re = re;
        self.im = im;
    }
}

impl Mul<f64> for Complex {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f64) -> Self {
        Self {
            re: self.re * rhs,
            im: self.im * rhs,
        }
    }
}

impl Mul<Complex> for f64 {
    type Output = Complex;
    #[inline]
    fn mul(self, rhs: Complex) -> Complex {
        Complex {
            re: self * rhs.re,
            im: self * rhs.im,
        }
    }
}

impl From<f64> for Complex {
    #[inline]
    fn from(re: f64) -> Self {
        Self { re, im: 0.0 }
    }
}

impl From<(f64, f64)> for Complex {
    #[inline]
    fn from((re, im): (f64, f64)) -> Self {
        Self { re, im }
    }
}

impl fmt::Debug for Complex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.re, self.im)
    }
}

impl fmt::Display for Complex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.im >= 0.0 {
            write!(f, "{}+{}i", self.re, self.im)
        } else {
            write!(f, "{}{}i", self.re, self.im)
        }
    }
}

// ---------------------------------------------------------------------------
// Quantum-domain types
// ---------------------------------------------------------------------------

/// Index of a qubit in a register
pub type QubitIndex = u32;

/// Single Pauli operator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PauliOp {
    I,
    X,
    Y,
    Z,
}

/// Pauli string: sparse representation of a tensor product of Pauli operators.
///
/// Only non-identity factors are stored.
#[derive(Debug, Clone, PartialEq)]
pub struct PauliString {
    pub ops: Vec<(QubitIndex, PauliOp)>,
}

impl PauliString {
    pub fn new(ops: Vec<(QubitIndex, PauliOp)>) -> Self {
        Self { ops }
    }

    pub fn identity() -> Self {
        Self { ops: vec![] }
    }
}

/// Hamiltonian expressed as a weighted sum of Pauli strings
#[derive(Debug, Clone)]
pub struct Hamiltonian {
    pub terms: Vec<(f64, PauliString)>,
    pub num_qubits: u32,
}

impl Hamiltonian {
    pub fn new(terms: Vec<(f64, PauliString)>, num_qubits: u32) -> Self {
        Self { terms, num_qubits }
    }
}

/// Result of measuring a single qubit
#[derive(Debug, Clone)]
pub struct MeasurementOutcome {
    pub qubit: QubitIndex,
    pub result: bool,
    pub probability: f64,
}

/// Aggregate metrics collected during simulation
#[derive(Debug, Clone, Default)]
pub struct SimulationMetrics {
    pub num_qubits: u32,
    pub gate_count: usize,
    pub execution_time_ns: u64,
    pub peak_memory_bytes: usize,
    pub gates_per_second: f64,
    pub gates_fused: usize,
}

/// Noise model for realistic simulation
#[derive(Debug, Clone)]
pub struct NoiseModel {
    pub depolarizing_rate: f64,
    pub bit_flip_rate: f64,
    pub phase_flip_rate: f64,
}

impl Default for NoiseModel {
    fn default() -> Self {
        Self {
            depolarizing_rate: 0.0,
            bit_flip_rate: 0.0,
            phase_flip_rate: 0.0,
        }
    }
}
