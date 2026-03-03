use crate::math_wasm::{Matrix, Vector};
use std::fmt;

#[derive(Debug, Clone)]
pub struct SolverConfig {
    pub max_iterations: usize,
    pub tolerance: f64,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-10,
        }
    }
}

#[derive(Debug)]
pub struct SolverError {
    pub message: String,
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Solver error: {}", self.message)
    }
}

impl std::error::Error for SolverError {}

pub struct StepData {
    pub iteration: usize,
    pub residual: f64,
    pub converged: bool,
    pub solution: Vector,
}

pub struct ConjugateGradientSolver {
    config: SolverConfig,
    last_iteration_count: usize,
}

impl ConjugateGradientSolver {
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            last_iteration_count: 0,
        }
    }

    pub fn solve(&mut self, a: &Matrix, b: &Vector) -> Result<Vector, SolverError> {
        self.validate_input(a, b)?;

        let n = b.len();
        let mut x = Vector::zeros(n);
        let mut r = b.subtract(&a.multiply_vector(&x).unwrap());
        let mut p = r.clone();
        let mut rsold = r.dot(&r);

        for iteration in 0..self.config.max_iterations {
            let ap = a.multiply_vector(&p).unwrap();
            let alpha = rsold / p.dot(&ap);

            x.axpy(alpha, &p);
            r.axpy(-alpha, &ap);

            let rsnew = r.dot(&r);
            let residual = rsnew.sqrt();

            self.last_iteration_count = iteration + 1;

            if residual < self.config.tolerance {
                return Ok(x);
            }

            let beta = rsnew / rsold;
            p = r.add(&p.scale(beta));
            rsold = rsnew;
        }

        Err(SolverError {
            message: format!(
                "Failed to converge after {} iterations. Final residual: {}",
                self.config.max_iterations,
                rsold.sqrt()
            ),
        })
    }

    pub fn solve_with_callback<F>(
        &mut self,
        a: &Matrix,
        b: &Vector,
        chunk_size: usize,
        mut callback: F,
    ) -> Result<Vector, SolverError>
    where
        F: FnMut(StepData),
    {
        self.validate_input(a, b)?;

        let n = b.len();
        let mut x = Vector::zeros(n);
        let mut r = b.subtract(&a.multiply_vector(&x).unwrap());
        let mut p = r.clone();
        let mut rsold = r.dot(&r);

        for iteration in 0..self.config.max_iterations {
            let ap = a.multiply_vector(&p).unwrap();
            let alpha = rsold / p.dot(&ap);

            x.axpy(alpha, &p);
            r.axpy(-alpha, &ap);

            let rsnew = r.dot(&r);
            let residual = rsnew.sqrt();

            let converged = residual < self.config.tolerance;

            // Call callback every chunk_size iterations or on convergence
            if iteration % chunk_size == 0 || converged {
                callback(StepData {
                    iteration: iteration + 1,
                    residual,
                    converged,
                    solution: x.clone(),
                });
            }

            self.last_iteration_count = iteration + 1;

            if converged {
                return Ok(x);
            }

            let beta = rsnew / rsold;
            p = r.add(&p.scale(beta));
            rsold = rsnew;
        }

        Err(SolverError {
            message: format!(
                "Failed to converge after {} iterations. Final residual: {}",
                self.config.max_iterations,
                rsold.sqrt()
            ),
        })
    }

    pub fn get_last_iteration_count(&self) -> usize {
        self.last_iteration_count
    }

    fn validate_input(&self, a: &Matrix, b: &Vector) -> Result<(), SolverError> {
        if a.rows() != a.cols() {
            return Err(SolverError {
                message: "Matrix must be square".to_string(),
            });
        }

        if a.rows() != b.len() {
            return Err(SolverError {
                message: "Matrix rows must match vector length".to_string(),
            });
        }

        if !a.is_symmetric() {
            return Err(SolverError {
                message: "Matrix must be symmetric for conjugate gradient".to_string(),
            });
        }

        if !a.is_positive_definite() {
            return Err(SolverError {
                message: "Matrix must be positive definite for conjugate gradient".to_string(),
            });
        }

        Ok(())
    }
}

// Alternative solver for comparison and benchmarking
pub struct JacobiSolver {
    config: SolverConfig,
    last_iteration_count: usize,
}

impl JacobiSolver {
    pub fn new(config: SolverConfig) -> Self {
        Self {
            config,
            last_iteration_count: 0,
        }
    }

    pub fn solve(&mut self, a: &Matrix, b: &Vector) -> Result<Vector, SolverError> {
        if a.rows() != a.cols() {
            return Err(SolverError {
                message: "Matrix must be square".to_string(),
            });
        }

        if a.rows() != b.len() {
            return Err(SolverError {
                message: "Matrix rows must match vector length".to_string(),
            });
        }

        let n = b.len();
        let mut x = Vector::zeros(n);
        let mut x_new = Vector::zeros(n);

        for iteration in 0..self.config.max_iterations {
            for i in 0..n {
                let mut sum = 0.0;
                for j in 0..n {
                    if i != j {
                        sum += a.get(i, j) * x.get(j);
                    }
                }
                x_new.set(i, (b.get(i) - sum) / a.get(i, i));
            }

            // Check convergence
            let diff = x_new.subtract(&x);
            let residual = diff.norm();

            self.last_iteration_count = iteration + 1;

            if residual < self.config.tolerance {
                return Ok(x_new);
            }

            x = x_new.clone();
        }

        Err(SolverError {
            message: format!(
                "Jacobi method failed to converge after {} iterations",
                self.config.max_iterations
            ),
        })
    }

    pub fn get_last_iteration_count(&self) -> usize {
        self.last_iteration_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conjugate_gradient_simple() {
        let config = SolverConfig {
            max_iterations: 100,
            tolerance: 1e-10,
        };

        let mut solver = ConjugateGradientSolver::new(config);

        // Simple 2x2 positive definite system
        // [4 1] [x1]   [1]
        // [1 3] [x2] = [2]
        let a = Matrix::from_slice(&[4.0, 1.0, 1.0, 3.0], 2, 2);
        let b = Vector::from_slice(&[1.0, 2.0]);

        let solution = solver.solve(&a, &b).unwrap();

        // Verify solution by substituting back
        let result = a.multiply_vector(&solution).unwrap();
        let error = result.subtract(&b).norm();

        assert!(error < 1e-10, "Solution error too large: {}", error);
    }

    #[test]
    fn test_jacobi_simple() {
        let config = SolverConfig {
            max_iterations: 1000,
            tolerance: 1e-6,
        };

        let mut solver = JacobiSolver::new(config);

        // Diagonally dominant system for Jacobi convergence
        // [4 1] [x1]   [1]
        // [1 4] [x2] = [2]
        let a = Matrix::from_slice(&[4.0, 1.0, 1.0, 4.0], 2, 2);
        let b = Vector::from_slice(&[1.0, 2.0]);

        let solution = solver.solve(&a, &b).unwrap();

        // Verify solution
        let result = a.multiply_vector(&solution).unwrap();
        let error = result.subtract(&b).norm();

        assert!(error < 1e-6, "Solution error too large: {}", error);
    }

    #[test]
    fn test_solver_with_callback() {
        let config = SolverConfig {
            max_iterations: 100,
            tolerance: 1e-10,
        };

        let mut solver = ConjugateGradientSolver::new(config);
        let a = Matrix::from_slice(&[4.0, 1.0, 1.0, 3.0], 2, 2);
        let b = Vector::from_slice(&[1.0, 2.0]);

        let mut callback_count = 0;
        let _solution = solver.solve_with_callback(&a, &b, 1, |_step| {
            callback_count += 1;
        }).unwrap();

        assert!(callback_count > 0, "Callback should have been called");
    }
}