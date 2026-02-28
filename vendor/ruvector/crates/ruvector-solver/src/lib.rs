//! Iterative sparse linear solvers for the ruvector ecosystem.
//!
//! This crate provides iterative methods for solving `Ax = b` where `A` is a
//! sparse matrix stored in CSR format.
//!
//! # Available Solvers
//!
//! | Solver | Feature gate | Method |
//! |--------|-------------|--------|
//! | [`NeumannSolver`](neumann::NeumannSolver) | `neumann` | Neumann series x = sum (I-A)^k b |
//!
//! # Example
//!
//! ```rust
//! use ruvector_solver::types::{ComputeBudget, CsrMatrix};
//! use ruvector_solver::neumann::NeumannSolver;
//! use ruvector_solver::traits::SolverEngine;
//!
//! // Build a diagonally dominant 3x3 matrix (f32)
//! let matrix = CsrMatrix::<f32>::from_coo(3, 3, vec![
//!     (0, 0, 2.0_f32), (0, 1, -0.5_f32),
//!     (1, 0, -0.5_f32), (1, 1, 2.0_f32), (1, 2, -0.5_f32),
//!     (2, 1, -0.5_f32), (2, 2, 2.0_f32),
//! ]);
//! let rhs = vec![1.0_f32, 0.0, 1.0];
//!
//! let solver = NeumannSolver::new(1e-6, 500);
//! let result = solver.solve(&matrix, &rhs).unwrap();
//! assert!(result.residual_norm < 1e-4);
//! ```

pub mod arena;
pub mod audit;
pub mod budget;
pub mod error;
pub mod events;
pub mod simd;
pub mod traits;
pub mod types;
pub mod validation;

#[cfg(feature = "neumann")]
pub mod neumann;

#[cfg(feature = "cg")]
pub mod cg;

#[cfg(feature = "forward-push")]
pub mod forward_push;

#[cfg(feature = "backward-push")]
pub mod backward_push;

#[cfg(feature = "hybrid-random-walk")]
pub mod random_walk;

#[cfg(feature = "bmssp")]
pub mod bmssp;

#[cfg(feature = "true-solver")]
pub mod true_solver;

pub mod router;
