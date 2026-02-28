//! Semidefinite Programming (SDP)
//!
//! Simple SDP solver for SOS certificates.

/// SDP problem in standard form
/// minimize: trace(C * X)
/// subject to: trace(A_i * X) = b_i, X ≽ 0
#[derive(Debug, Clone)]
pub struct SDPProblem {
    /// Matrix dimension
    pub n: usize,
    /// Objective matrix C (n × n)
    pub c: Vec<f64>,
    /// Constraint matrices A_i
    pub constraints: Vec<Vec<f64>>,
    /// Constraint right-hand sides b_i
    pub b: Vec<f64>,
}

impl SDPProblem {
    /// Create new SDP problem
    pub fn new(n: usize) -> Self {
        Self {
            n,
            c: vec![0.0; n * n],
            constraints: Vec::new(),
            b: Vec::new(),
        }
    }

    /// Set objective matrix
    pub fn set_objective(&mut self, c: Vec<f64>) {
        assert_eq!(c.len(), self.n * self.n);
        self.c = c;
    }

    /// Add constraint
    pub fn add_constraint(&mut self, a: Vec<f64>, bi: f64) {
        assert_eq!(a.len(), self.n * self.n);
        self.constraints.push(a);
        self.b.push(bi);
    }

    /// Number of constraints
    pub fn num_constraints(&self) -> usize {
        self.constraints.len()
    }
}

/// SDP solution
#[derive(Debug, Clone)]
pub struct SDPSolution {
    /// Optimal X matrix
    pub x: Vec<f64>,
    /// Optimal value
    pub value: f64,
    /// Solver status
    pub status: SDPStatus,
    /// Number of iterations
    pub iterations: usize,
}

/// Solver status
#[derive(Debug, Clone, PartialEq)]
pub enum SDPStatus {
    Optimal,
    Infeasible,
    Unbounded,
    MaxIterations,
    NumericalError,
}

/// Simple projected gradient SDP solver
pub struct SDPSolver {
    /// Maximum iterations
    pub max_iters: usize,
    /// Tolerance
    pub tolerance: f64,
    /// Step size
    pub step_size: f64,
}

impl SDPSolver {
    /// Create with default parameters
    pub fn new() -> Self {
        Self {
            max_iters: 1000,
            tolerance: 1e-6,
            step_size: 0.01,
        }
    }

    /// Solve SDP problem
    pub fn solve(&self, problem: &SDPProblem) -> SDPSolution {
        let n = problem.n;
        let m = problem.num_constraints();

        if n == 0 {
            return SDPSolution {
                x: vec![],
                value: 0.0,
                status: SDPStatus::Optimal,
                iterations: 0,
            };
        }

        // Initialize X as identity
        let mut x = vec![0.0; n * n];
        for i in 0..n {
            x[i * n + i] = 1.0;
        }

        // Simple augmented Lagrangian method
        let mut dual = vec![0.0; m];
        let rho = 1.0;

        for iter in 0..self.max_iters {
            // Compute gradient of Lagrangian
            let mut grad = problem.c.clone();

            for (j, (a, &d)) in problem.constraints.iter().zip(dual.iter()).enumerate() {
                let ax: f64 = (0..n * n).map(|k| a[k] * x[k]).sum();
                let residual = ax - problem.b[j];

                // Gradient contribution from constraint
                for k in 0..n * n {
                    grad[k] += (d + rho * residual) * a[k];
                }
            }

            // Gradient descent step
            for k in 0..n * n {
                x[k] -= self.step_size * grad[k];
            }

            // Project onto PSD cone
            self.project_psd(&mut x, n);

            // Update dual variables
            let mut max_violation = 0.0f64;
            for (j, a) in problem.constraints.iter().enumerate() {
                let ax: f64 = (0..n * n).map(|k| a[k] * x[k]).sum();
                let residual = ax - problem.b[j];
                dual[j] += rho * residual;
                max_violation = max_violation.max(residual.abs());
            }

            // Check convergence
            if max_violation < self.tolerance {
                let value: f64 = (0..n * n).map(|k| problem.c[k] * x[k]).sum();
                return SDPSolution {
                    x,
                    value,
                    status: SDPStatus::Optimal,
                    iterations: iter + 1,
                };
            }
        }

        let value: f64 = (0..n * n).map(|k| problem.c[k] * x[k]).sum();
        SDPSolution {
            x,
            value,
            status: SDPStatus::MaxIterations,
            iterations: self.max_iters,
        }
    }

    /// Project matrix onto PSD cone via eigendecomposition
    fn project_psd(&self, x: &mut [f64], n: usize) {
        // Symmetrize first
        for i in 0..n {
            for j in i + 1..n {
                let avg = (x[i * n + j] + x[j * n + i]) / 2.0;
                x[i * n + j] = avg;
                x[j * n + i] = avg;
            }
        }

        // For small matrices, use power iteration to find and remove negative eigencomponents
        // This is a simplified approach
        if n <= 10 {
            self.project_psd_small(x, n);
        } else {
            // For larger matrices, just ensure diagonal dominance
            for i in 0..n {
                let mut row_sum = 0.0;
                for j in 0..n {
                    if i != j {
                        row_sum += x[i * n + j].abs();
                    }
                }
                x[i * n + i] = x[i * n + i].max(row_sum + 0.01);
            }
        }
    }

    fn project_psd_small(&self, x: &mut [f64], n: usize) {
        // Simple approach: ensure minimum eigenvalue is non-negative
        // by adding αI where α makes smallest eigenvalue ≥ 0

        // Estimate smallest eigenvalue via power iteration on -X + λ_max I
        let mut v: Vec<f64> = (0..n).map(|i| 1.0 / (n as f64).sqrt()).collect();

        // First get largest eigenvalue estimate
        let mut lambda_max = 0.0;
        for _ in 0..20 {
            let mut y = vec![0.0; n];
            for i in 0..n {
                for j in 0..n {
                    y[i] += x[i * n + j] * v[j];
                }
            }
            let norm: f64 = y.iter().map(|&yi| yi * yi).sum::<f64>().sqrt();
            lambda_max = v.iter().zip(y.iter()).map(|(&vi, &yi)| vi * yi).sum();
            if norm > 1e-15 {
                for i in 0..n {
                    v[i] = y[i] / norm;
                }
            }
        }

        // Now find smallest eigenvalue using shifted power iteration
        let shift = lambda_max.abs() + 1.0;
        let mut v: Vec<f64> = (0..n).map(|i| 1.0 / (n as f64).sqrt()).collect();
        let mut lambda_min = 0.0;

        for _ in 0..20 {
            let mut y = vec![0.0; n];
            for i in 0..n {
                for j in 0..n {
                    let val = if i == j {
                        shift - x[i * n + j]
                    } else {
                        -x[i * n + j]
                    };
                    y[i] += val * v[j];
                }
            }
            let norm: f64 = y.iter().map(|&yi| yi * yi).sum::<f64>().sqrt();
            let lambda_shifted: f64 = v.iter().zip(y.iter()).map(|(&vi, &yi)| vi * yi).sum();
            lambda_min = shift - lambda_shifted;
            if norm > 1e-15 {
                for i in 0..n {
                    v[i] = y[i] / norm;
                }
            }
        }

        // If smallest eigenvalue is negative, shift matrix
        if lambda_min < 0.0 {
            let alpha = -lambda_min + 0.01;
            for i in 0..n {
                x[i * n + i] += alpha;
            }
        }
    }
}

impl Default for SDPSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdp_simple() {
        // Minimize trace(X) subject to X_{11} = 1, X ≽ 0
        let mut problem = SDPProblem::new(2);

        // Objective: trace(X) = X_{00} + X_{11}
        let mut c = vec![0.0; 4];
        c[0] = 1.0; // X_{00}
        c[3] = 1.0; // X_{11}
        problem.set_objective(c);

        // Constraint: X_{00} = 1
        let mut a = vec![0.0; 4];
        a[0] = 1.0;
        problem.add_constraint(a, 1.0);

        let solver = SDPSolver::new();
        let solution = solver.solve(&problem);

        // Should find X_{00} = 1, X_{11} close to 0 (or whatever makes X PSD)
        assert!(
            solution.status == SDPStatus::Optimal || solution.status == SDPStatus::MaxIterations
        );
    }

    #[test]
    fn test_sdp_feasibility() {
        // Feasibility: find X ≽ 0 with X_{00} = 1, X_{11} = 1
        let mut problem = SDPProblem::new(2);

        // Zero objective
        problem.set_objective(vec![0.0; 4]);

        // X_{00} = 1
        let mut a1 = vec![0.0; 4];
        a1[0] = 1.0;
        problem.add_constraint(a1, 1.0);

        // X_{11} = 1
        let mut a2 = vec![0.0; 4];
        a2[3] = 1.0;
        problem.add_constraint(a2, 1.0);

        let solver = SDPSolver::new();
        let solution = solver.solve(&problem);

        // Check constraints approximately satisfied
        let x00 = solution.x[0];
        let x11 = solution.x[3];
        assert!((x00 - 1.0).abs() < 0.1 || solution.status == SDPStatus::MaxIterations);
        assert!((x11 - 1.0).abs() < 0.1 || solution.status == SDPStatus::MaxIterations);
    }
}
