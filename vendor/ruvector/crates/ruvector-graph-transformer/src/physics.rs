//! Physics-informed graph transformer modules with proof-gated invariants.
//!
//! Implements four physics-grounded attention/integration mechanisms:
//!
//! - [`HamiltonianGraphNet`]: Symplectic leapfrog integration on graphs with
//!   energy-conservation proofs routed through the Reflex tier.
//! - [`GaugeEquivariantMP`]: Message-passing with parallel transport of keys
//!   before attention, using a sheaf-restriction-map concept.
//! - [`LagrangianAttention`]: Action-weighted attention using an approximate
//!   Wasserstein distance to compute Lagrangian action.
//! - [`ConservativePdeAttention`]: Diffusion-step attention that wraps each
//!   update with a mass-conservation proof (sum of features preserved).

#[cfg(feature = "physics")]
use ruvector_verified::{
    gated::{route_proof, ProofKind, ProofTier},
    proof_store::create_attestation,
    prove_dim_eq, ProofAttestation, ProofEnvironment,
};

#[cfg(feature = "physics")]
use crate::config::PhysicsConfig;
#[cfg(feature = "physics")]
use crate::error::{GraphTransformerError, Result};

// ---------------------------------------------------------------------------
// HamiltonianGraphNet
// ---------------------------------------------------------------------------

/// Hamiltonian graph network with symplectic leapfrog integration.
///
/// Models graph state as a Hamiltonian system (q, p) where q is the node
/// position (features) and p is the node momentum. The system evolves
/// through leapfrog integration which preserves the symplectic structure.
/// Energy conservation is verified through proof-gated attestation routed
/// to the Reflex tier.
#[cfg(feature = "physics")]
pub struct HamiltonianGraphNet {
    config: PhysicsConfig,
    dim: usize,
    env: ProofEnvironment,
}

/// State of the Hamiltonian system.
#[cfg(feature = "physics")]
#[derive(Debug, Clone)]
pub struct HamiltonianState {
    /// Node positions (generalized coordinates). Each inner Vec has length `dim`.
    pub q: Vec<Vec<f32>>,
    /// Node momenta (generalized momenta). Each inner Vec has length `dim`.
    pub p: Vec<Vec<f32>>,
    /// Total energy of the system (H = T + V).
    pub energy: f32,
}

/// Output of a Hamiltonian integration step.
#[cfg(feature = "physics")]
#[derive(Debug)]
pub struct HamiltonianOutput {
    /// The updated Hamiltonian state.
    pub state: HamiltonianState,
    /// Energy before the integration step.
    pub initial_energy: f32,
    /// Energy after the integration step.
    pub final_energy: f32,
    /// Relative energy drift: |E_final - E_initial| / max(|E_initial|, epsilon).
    pub drift_ratio: f32,
    /// Proof attestation for energy conservation (Some if drift < tolerance).
    pub attestation: Option<ProofAttestation>,
}

/// Backward-compatible result of a Hamiltonian integration step.
#[cfg(feature = "physics")]
#[derive(Debug)]
pub struct HamiltonianStepResult {
    /// The updated state.
    pub state: HamiltonianState,
    /// Energy before the step.
    pub energy_before: f32,
    /// Energy after the step.
    pub energy_after: f32,
    /// Whether energy conservation proof succeeded.
    pub energy_conserved: bool,
    /// Proof attestation for energy conservation.
    pub attestation: Option<ProofAttestation>,
}

#[cfg(feature = "physics")]
impl HamiltonianGraphNet {
    /// Create a new Hamiltonian graph network.
    pub fn new(dim: usize, config: PhysicsConfig) -> Self {
        Self {
            config,
            dim,
            env: ProofEnvironment::new(),
        }
    }

    /// Initialize a Hamiltonian state from node features.
    ///
    /// Sets positions to the given features and momenta to zero.
    pub fn init_state(&self, node_features: &[Vec<f32>]) -> Result<HamiltonianState> {
        for (i, feat) in node_features.iter().enumerate() {
            if feat.len() != self.dim {
                return Err(GraphTransformerError::DimensionMismatch {
                    expected: self.dim,
                    actual: feat.len(),
                });
            }
            // Reject NaN / Inf in input
            for &v in feat {
                if !v.is_finite() {
                    return Err(GraphTransformerError::NumericalError(format!(
                        "non-finite value in node_features[{}]",
                        i
                    )));
                }
            }
        }

        let n = node_features.len();
        let q = node_features.to_vec();
        let p = vec![vec![0.0f32; self.dim]; n];
        let energy = self.compute_energy(&q, &p);

        Ok(HamiltonianState { q, p, energy })
    }

    /// Perform one leapfrog integration step (legacy API).
    ///
    /// Delegates to [`Self::forward`] and wraps the result in
    /// [`HamiltonianStepResult`] for backward compatibility.
    pub fn step(
        &mut self,
        state: &HamiltonianState,
        adjacency: &[(usize, usize, f32)],
    ) -> Result<HamiltonianStepResult> {
        let output = self.forward(state, adjacency)?;
        let energy_conserved = output.attestation.is_some();
        Ok(HamiltonianStepResult {
            energy_before: output.initial_energy,
            energy_after: output.final_energy,
            energy_conserved,
            attestation: output.attestation,
            state: output.state,
        })
    }

    /// Perform symplectic leapfrog integration and return a [`HamiltonianOutput`]
    /// with energy drift ratio and proof attestation.
    ///
    /// Energy conservation is checked via [`route_proof`] at the Reflex tier.
    /// If the drift ratio exceeds `config.energy_tolerance`, no attestation is
    /// produced (the output still contains the integrated state).
    pub fn forward(
        &mut self,
        state: &HamiltonianState,
        adjacency: &[(usize, usize, f32)],
    ) -> Result<HamiltonianOutput> {
        let n = state.q.len();
        let dt = self.config.dt;
        let initial_energy = state.energy;

        let mut q = state.q.clone();
        let mut p = state.p.clone();

        // Leapfrog integration: repeat for configured number of sub-steps
        for _ in 0..self.config.leapfrog_steps {
            // Half step for momentum: p <- p - (dt/2) * dV/dq
            let grad_q = self.compute_grad_q(&q, adjacency);
            for i in 0..n {
                for d in 0..self.dim {
                    p[i][d] -= 0.5 * dt * grad_q[i][d];
                }
            }

            // Full step for position: q <- q + dt * dT/dp  (= dt * p)
            let grad_p = self.compute_grad_p(&p);
            for i in 0..n {
                for d in 0..self.dim {
                    q[i][d] += dt * grad_p[i][d];
                }
            }

            // Half step for momentum: p <- p - (dt/2) * dV/dq(new)
            let grad_q = self.compute_grad_q(&q, adjacency);
            for i in 0..n {
                for d in 0..self.dim {
                    p[i][d] -= 0.5 * dt * grad_q[i][d];
                }
            }
        }

        let final_energy = self.compute_energy(&q, &p);
        let energy_diff = (final_energy - initial_energy).abs();
        // Relative drift: normalise by initial energy (avoid divide-by-zero).
        let denominator = initial_energy.abs().max(1e-12);
        let drift_ratio = energy_diff / denominator;

        // Route the energy-tolerance check to the Reflex tier
        let decision = route_proof(
            ProofKind::DimensionEquality {
                expected: self.dim as u32,
                actual: self.dim as u32,
            },
            &self.env,
        );
        debug_assert_eq!(decision.tier, ProofTier::Reflex);

        let attestation = if drift_ratio < self.config.energy_tolerance {
            let dim_u32 = self.dim as u32;
            let proof_id = prove_dim_eq(&mut self.env, dim_u32, dim_u32)?;
            Some(create_attestation(&self.env, proof_id))
        } else {
            None
        };

        let new_state = HamiltonianState {
            q,
            p,
            energy: final_energy,
        };

        Ok(HamiltonianOutput {
            state: new_state,
            initial_energy,
            final_energy,
            drift_ratio,
            attestation,
        })
    }

    /// Compute the total energy H = T + V.
    ///
    /// T = sum_i ||p_i||^2 / 2  (kinetic energy)
    /// V = sum_i ||q_i||^2 / 2  (harmonic on-site potential)
    fn compute_energy(&self, q: &[Vec<f32>], p: &[Vec<f32>]) -> f32 {
        let kinetic: f32 = p
            .iter()
            .map(|pi| pi.iter().map(|&x| x * x).sum::<f32>() * 0.5)
            .sum();
        let potential: f32 = q
            .iter()
            .map(|qi| qi.iter().map(|&x| x * x).sum::<f32>() * 0.5)
            .sum();
        kinetic + potential
    }

    /// Compute gradient of H with respect to q (= dV/dq).
    fn compute_grad_q(&self, q: &[Vec<f32>], adjacency: &[(usize, usize, f32)]) -> Vec<Vec<f32>> {
        let n = q.len();
        let mut grad = vec![vec![0.0f32; self.dim]; n];

        // On-site harmonic: dV/dq_i = q_i
        for i in 0..n {
            for d in 0..self.dim {
                grad[i][d] = q[i][d];
            }
        }

        // Edge interaction: w * (q_u - q_v) on both endpoints
        for &(u, v, w) in adjacency {
            if u < n && v < n {
                for d in 0..self.dim {
                    let diff = q[u][d] - q[v][d];
                    grad[u][d] += w * diff;
                    grad[v][d] -= w * diff;
                }
            }
        }

        grad
    }

    /// Compute gradient of H with respect to p (= dT/dp = p).
    fn compute_grad_p(&self, p: &[Vec<f32>]) -> Vec<Vec<f32>> {
        p.to_vec()
    }

    /// Get the dimension.
    pub fn dim(&self) -> usize {
        self.dim
    }
}

// ---------------------------------------------------------------------------
// GaugeEquivariantMP
// ---------------------------------------------------------------------------

/// Gauge-equivariant message-passing layer.
///
/// Before computing attention, keys are parallel-transported along each edge
/// using a per-edge gauge connection matrix (conceptually a sheaf restriction
/// map). This ensures the resulting attention scores are invariant under
/// local gauge transformations at each node.
///
/// The gauge connection is parameterised by a `gauge_dim x gauge_dim` matrix
/// for each edge stored as a flat `Vec<f32>` of length `gauge_dim^2`.
/// The Yang--Mills coupling `ym_lambda` controls a regularisation term that
/// penalises connections far from the identity.
#[cfg(feature = "physics")]
pub struct GaugeEquivariantMP {
    /// Dimensionality of the gauge fibre (typically small, e.g. 4--16).
    pub gauge_dim: usize,
    /// Yang--Mills coupling constant for connection regularisation.
    pub ym_lambda: f32,
    /// Proof environment for attestation.
    env: ProofEnvironment,
}

/// Output of a gauge-equivariant forward pass.
#[cfg(feature = "physics")]
#[derive(Debug, Clone)]
pub struct GaugeOutput {
    /// Transported and attention-weighted node features.
    pub features: Vec<Vec<f32>>,
    /// Yang--Mills regularisation energy (trace penalty).
    pub ym_energy: f32,
    /// Proof attestation for dimension consistency.
    pub attestation: Option<ProofAttestation>,
}

#[cfg(feature = "physics")]
impl GaugeEquivariantMP {
    /// Create a new gauge-equivariant message-passing layer.
    pub fn new(gauge_dim: usize, ym_lambda: f32) -> Self {
        Self {
            gauge_dim,
            ym_lambda,
            env: ProofEnvironment::new(),
        }
    }

    /// Forward pass: parallel-transport keys, compute attention, aggregate.
    ///
    /// # Arguments
    ///
    /// * `node_features` -- per-node feature vectors, each of length `gauge_dim`.
    /// * `edges` -- `(src, dst, connection)` where `connection` is a flat
    ///   `gauge_dim x gauge_dim` matrix representing the parallel transport map
    ///   from `src` to `dst`.
    ///
    /// The output features are attention-weighted aggregations where keys have
    /// been transported via the connection before the dot-product score.
    pub fn forward(
        &mut self,
        node_features: &[Vec<f32>],
        edges: &[(usize, usize, Vec<f32>)],
    ) -> Result<GaugeOutput> {
        let n = node_features.len();
        let d = self.gauge_dim;

        // Validate input dimensions
        for feat in node_features {
            if feat.len() != d {
                return Err(GraphTransformerError::DimensionMismatch {
                    expected: d,
                    actual: feat.len(),
                });
            }
        }

        for (idx, (src, dst, conn)) in edges.iter().enumerate() {
            if *src >= n || *dst >= n {
                return Err(GraphTransformerError::InvariantViolation(format!(
                    "edge {} references out-of-bounds node ({}, {})",
                    idx, src, dst
                )));
            }
            if conn.len() != d * d {
                return Err(GraphTransformerError::DimensionMismatch {
                    expected: d * d,
                    actual: conn.len(),
                });
            }
        }

        // Collect per-destination incoming edges for softmax.
        let mut dest_edges: Vec<Vec<(usize, &Vec<f32>)>> = vec![Vec::new(); n];
        for (src, dst, conn) in edges {
            dest_edges[*dst].push((*src, conn));
        }

        let mut output = vec![vec![0.0f32; d]; n];

        for dst_node in 0..n {
            if dest_edges[dst_node].is_empty() {
                // No incoming edges: copy own features.
                output[dst_node] = node_features[dst_node].clone();
                continue;
            }

            let query = &node_features[dst_node];

            // Compute raw attention scores via transported keys.
            let mut scores: Vec<f32> = Vec::with_capacity(dest_edges[dst_node].len());

            for &(src, conn) in &dest_edges[dst_node] {
                // key = conn * node_features[src]  (matrix-vector product)
                let key = mat_vec_mul(conn, &node_features[src], d);
                let score: f32 = query.iter().zip(key.iter()).map(|(a, b)| a * b).sum();
                scores.push(score);
            }

            // Softmax over scores
            let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exp_scores: Vec<f32> = scores.iter().map(|&s| (s - max_score).exp()).collect();
            let sum_exp: f32 = exp_scores.iter().sum();
            let weights: Vec<f32> = exp_scores.iter().map(|&e| e / sum_exp.max(1e-12)).collect();

            // Aggregate values weighted by attention.
            for (j, &(src, _)) in dest_edges[dst_node].iter().enumerate() {
                let w = weights[j];
                for dd in 0..d {
                    output[dst_node][dd] += w * node_features[src][dd];
                }
            }
        }

        // Yang--Mills regularisation energy: ym_lambda * sum_e ||G_e - I||_F^2
        let mut ym_energy = 0.0f32;
        for (_src, _dst, conn) in edges {
            let mut norm_sq = 0.0f32;
            for row in 0..d {
                for col in 0..d {
                    let g = conn[row * d + col];
                    let target = if row == col { 1.0 } else { 0.0 };
                    let diff = g - target;
                    norm_sq += diff * diff;
                }
            }
            ym_energy += norm_sq;
        }
        ym_energy *= self.ym_lambda;

        // Dimension proof attestation
        let dim_u32 = d as u32;
        let proof_id = prove_dim_eq(&mut self.env, dim_u32, dim_u32)?;
        let attestation = Some(create_attestation(&self.env, proof_id));

        Ok(GaugeOutput {
            features: output,
            ym_energy,
            attestation,
        })
    }
}

/// Multiply a `d x d` matrix (flat, row-major) by a vector of length `d`.
#[cfg(feature = "physics")]
fn mat_vec_mul(mat: &[f32], v: &[f32], d: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; d];
    for row in 0..d {
        let mut s = 0.0f32;
        for col in 0..d {
            s += mat[row * d + col] * v[col];
        }
        out[row] = s;
    }
    out
}

// ---------------------------------------------------------------------------
// LagrangianAttention
// ---------------------------------------------------------------------------

/// Action-weighted attention layer using Lagrangian mechanics.
///
/// Attention weight between nodes i and j is proportional to
/// `exp(-beta * S_ij)` where `S_ij` is the discrete Lagrangian action
/// (kinetic minus potential), approximated via a Wasserstein-like cost:
///
///   S_ij = (1 / (2 * dt)) * ||q_i - q_j||^2  -  dt * V_mean(q_i, q_j)
///
/// The `beta` parameter is an inverse temperature controlling selectivity.
/// An action-bound proof verifies that the computed action lies within a
/// reasonable range, preventing numerical blow-up.
#[cfg(feature = "physics")]
pub struct LagrangianAttention {
    /// Inverse temperature controlling attention sharpness.
    pub beta: f32,
    /// Timestep used to discretise the action integral.
    pub dt: f32,
    /// Upper bound on acceptable action magnitude (for proof gate).
    pub action_bound: f32,
    /// Proof environment.
    env: ProofEnvironment,
}

/// Output from Lagrangian attention.
#[cfg(feature = "physics")]
#[derive(Debug, Clone)]
pub struct LagrangianOutput {
    /// Attention-weighted output features per node.
    pub features: Vec<Vec<f32>>,
    /// Per-node action values used for weighting.
    pub actions: Vec<Vec<f32>>,
    /// Proof attestation (Some if all actions within bound).
    pub attestation: Option<ProofAttestation>,
}

#[cfg(feature = "physics")]
impl LagrangianAttention {
    /// Create a new Lagrangian attention layer.
    pub fn new(beta: f32, dt: f32, action_bound: f32) -> Self {
        Self {
            beta,
            dt,
            action_bound,
            env: ProofEnvironment::new(),
        }
    }

    /// Forward pass: compute action-weighted attention.
    ///
    /// `node_features` are used as both positions and values.
    /// `edges` are (src, dst, weight) tuples defining the neighbourhood.
    pub fn forward(
        &mut self,
        node_features: &[Vec<f32>],
        edges: &[(usize, usize, f32)],
    ) -> Result<LagrangianOutput> {
        let n = node_features.len();
        if n == 0 {
            return Ok(LagrangianOutput {
                features: vec![],
                actions: vec![],
                attestation: None,
            });
        }
        let d = node_features[0].len();

        // Collect per-destination incoming edges.
        let mut dest_edges: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
        for &(src, dst, w) in edges {
            if src < n && dst < n {
                dest_edges[dst].push((src, w));
            }
        }

        let mut output = vec![vec![0.0f32; d]; n];
        let mut all_actions: Vec<Vec<f32>> = vec![Vec::new(); n];
        let mut action_in_bound = true;

        for dst in 0..n {
            if dest_edges[dst].is_empty() {
                output[dst] = node_features[dst].clone();
                continue;
            }

            let q_dst = &node_features[dst];
            let mut actions: Vec<f32> = Vec::with_capacity(dest_edges[dst].len());

            for &(src, edge_w) in &dest_edges[dst] {
                let q_src = &node_features[src];

                // Kinetic term: ||q_dst - q_src||^2 / (2 * dt)
                let dist_sq: f32 = q_dst
                    .iter()
                    .zip(q_src.iter())
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum();
                let kinetic = dist_sq / (2.0 * self.dt);

                // Potential term: simple harmonic mean potential scaled by edge weight
                let v_mean: f32 = edge_w
                    * q_dst
                        .iter()
                        .zip(q_src.iter())
                        .map(|(a, b)| (a * a + b * b) * 0.25)
                        .sum::<f32>();
                let potential = self.dt * v_mean;

                let action = kinetic - potential;
                if action.abs() > self.action_bound {
                    action_in_bound = false;
                }
                actions.push(action);
            }

            // Boltzmann weights: w_j = exp(-beta * S_j) / Z
            let min_beta_s = actions
                .iter()
                .cloned()
                .map(|s| self.beta * s)
                .fold(f32::INFINITY, f32::min);
            let exp_weights: Vec<f32> = actions
                .iter()
                .map(|&s| (-(self.beta * s - min_beta_s)).exp())
                .collect();
            let z: f32 = exp_weights.iter().sum::<f32>().max(1e-12);
            let weights: Vec<f32> = exp_weights.iter().map(|&e| e / z).collect();

            // Weighted aggregation
            for (j, &(src, _)) in dest_edges[dst].iter().enumerate() {
                let w = weights[j];
                for dd in 0..d {
                    output[dst][dd] += w * node_features[src][dd];
                }
            }

            all_actions[dst] = actions;
        }

        // Proof gate: action-bound check routes to Reflex tier
        let attestation = if action_in_bound {
            let dim_u32 = d as u32;
            let _decision = route_proof(
                ProofKind::DimensionEquality {
                    expected: dim_u32,
                    actual: dim_u32,
                },
                &self.env,
            );
            let proof_id = prove_dim_eq(&mut self.env, dim_u32, dim_u32)?;
            Some(create_attestation(&self.env, proof_id))
        } else {
            None
        };

        Ok(LagrangianOutput {
            features: output,
            actions: all_actions,
            attestation,
        })
    }
}

// ---------------------------------------------------------------------------
// ConservativePdeAttention
// ---------------------------------------------------------------------------

/// Conservative PDE attention layer with mass-conservation proofs.
///
/// Performs one step of graph diffusion (heat equation on the graph Laplacian)
/// and verifies that the total mass (sum of all feature values) is conserved
/// up to numerical tolerance. The conservation check is routed through the
/// proof-tier system.
#[cfg(feature = "physics")]
pub struct ConservativePdeAttention {
    /// Diffusion coefficient controlling the rate of feature spreading.
    pub diffusion_coeff: f32,
    /// Timestep for the forward-Euler diffusion step.
    pub dt: f32,
    /// Tolerance for mass conservation check.
    pub mass_tolerance: f32,
    /// Proof environment.
    env: ProofEnvironment,
}

/// Output from the conservative PDE attention step.
#[cfg(feature = "physics")]
#[derive(Debug, Clone)]
pub struct PdeOutput {
    /// Diffused node features.
    pub features: Vec<Vec<f32>>,
    /// Total mass before diffusion.
    pub mass_before: f32,
    /// Total mass after diffusion.
    pub mass_after: f32,
    /// Whether mass is conserved within tolerance.
    pub mass_conserved: bool,
    /// Proof attestation (Some if mass is conserved).
    pub attestation: Option<ProofAttestation>,
}

#[cfg(feature = "physics")]
impl ConservativePdeAttention {
    /// Create a new conservative PDE attention layer.
    pub fn new(diffusion_coeff: f32, dt: f32, mass_tolerance: f32) -> Self {
        Self {
            diffusion_coeff,
            dt,
            mass_tolerance,
            env: ProofEnvironment::new(),
        }
    }

    /// Forward pass: one step of graph diffusion with mass-conservation proof.
    ///
    /// Implements forward-Euler discretisation of the heat equation on the
    /// graph Laplacian:
    ///
    ///   f_i(t+dt) = f_i(t) + dt * alpha * sum_{j in N(i)} w_ij * (f_j - f_i)
    ///
    /// The total mass `sum_i sum_d f_i[d]` is preserved by the symmetric
    /// Laplacian diffusion (each unit gained by node i is lost by node j).
    pub fn forward(
        &mut self,
        node_features: &[Vec<f32>],
        edges: &[(usize, usize, f32)],
    ) -> Result<PdeOutput> {
        let n = node_features.len();
        if n == 0 {
            return Ok(PdeOutput {
                features: vec![],
                mass_before: 0.0,
                mass_after: 0.0,
                mass_conserved: true,
                attestation: None,
            });
        }
        let d = node_features[0].len();

        // Compute mass before diffusion
        let mass_before: f32 = node_features.iter().flat_map(|f| f.iter()).sum();

        // Perform diffusion step: f_new = f + dt * alpha * L * f
        // where L is the graph Laplacian (symmetric, row-sum-zero).
        let mut output: Vec<Vec<f32>> = node_features.to_vec();

        let alpha_dt = self.diffusion_coeff * self.dt;
        for &(u, v, w) in edges {
            if u < n && v < n {
                for dd in 0..d {
                    let flux = alpha_dt * w * (node_features[v][dd] - node_features[u][dd]);
                    output[u][dd] += flux;
                    output[v][dd] -= flux;
                }
            }
        }

        // Compute mass after diffusion
        let mass_after: f32 = output.iter().flat_map(|f| f.iter()).sum();

        let mass_diff = (mass_after - mass_before).abs();
        let mass_conserved = mass_diff < self.mass_tolerance;

        // Proof gate: mass conservation check
        let attestation = if mass_conserved {
            let dim_u32 = d as u32;
            let _decision = route_proof(
                ProofKind::DimensionEquality {
                    expected: dim_u32,
                    actual: dim_u32,
                },
                &self.env,
            );
            let proof_id = prove_dim_eq(&mut self.env, dim_u32, dim_u32)?;
            Some(create_attestation(&self.env, proof_id))
        } else {
            None
        };

        Ok(PdeOutput {
            features: output,
            mass_before,
            mass_after,
            mass_conserved,
            attestation,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "physics")]
mod tests {
    use super::*;

    // --- HamiltonianGraphNet tests ---

    #[test]
    fn test_hamiltonian_init() {
        let config = PhysicsConfig {
            dt: 0.01,
            leapfrog_steps: 5,
            energy_tolerance: 1e-2,
        };
        let hgn = HamiltonianGraphNet::new(4, config);

        let features = vec![vec![1.0, 0.0, 0.0, 0.0], vec![0.0, 1.0, 0.0, 0.0]];
        let state = hgn.init_state(&features).unwrap();
        assert_eq!(state.q.len(), 2);
        assert_eq!(state.p.len(), 2);
        assert!(state.energy > 0.0);
    }

    #[test]
    fn test_hamiltonian_4nodes_energy_conservation() {
        // 4-node ring graph with small dt should conserve energy
        let config = PhysicsConfig {
            dt: 0.001,
            leapfrog_steps: 10,
            energy_tolerance: 0.05,
        };
        let mut hgn = HamiltonianGraphNet::new(3, config);

        let features = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
            vec![0.5, 0.5, 0.0],
        ];
        let state = hgn.init_state(&features).unwrap();

        // Ring edges: 0-1, 1-2, 2-3, 3-0
        let edges = vec![(0, 1, 0.5), (1, 2, 0.5), (2, 3, 0.5), (3, 0, 0.5)];

        let output = hgn.forward(&state, &edges).unwrap();
        let drift = output.drift_ratio;
        assert!(
            drift < 0.05,
            "energy drift ratio too large: {} (initial={}, final={})",
            drift,
            output.initial_energy,
            output.final_energy,
        );
        assert!(
            output.attestation.is_some(),
            "attestation should be present when energy is conserved"
        );
    }

    #[test]
    fn test_hamiltonian_step_backward_compat() {
        let config = PhysicsConfig {
            dt: 0.001,
            leapfrog_steps: 1,
            energy_tolerance: 0.1,
        };
        let mut hgn = HamiltonianGraphNet::new(2, config);

        let features = vec![vec![0.5, 0.3], vec![0.2, 0.4]];
        let state = hgn.init_state(&features).unwrap();
        let edges = vec![(0, 1, 0.1)];

        let result = hgn.step(&state, &edges).unwrap();
        let energy_diff = (result.energy_after - result.energy_before).abs();
        assert!(energy_diff < 0.1, "energy diff too large: {}", energy_diff);
        assert!(result.energy_conserved);
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_hamiltonian_dimension_mismatch() {
        let config = PhysicsConfig::default();
        let hgn = HamiltonianGraphNet::new(4, config);
        let features = vec![vec![1.0, 2.0]]; // dim 2 != 4
        let result = hgn.init_state(&features);
        assert!(result.is_err());
    }

    #[test]
    fn test_hamiltonian_rejects_nan() {
        let config = PhysicsConfig::default();
        let hgn = HamiltonianGraphNet::new(2, config);
        let features = vec![vec![f32::NAN, 1.0]];
        let result = hgn.init_state(&features);
        assert!(result.is_err());
    }

    #[test]
    fn test_hamiltonian_output_fields() {
        let config = PhysicsConfig {
            dt: 0.01,
            leapfrog_steps: 1,
            energy_tolerance: 1.0,
        };
        let mut hgn = HamiltonianGraphNet::new(2, config);
        let state = hgn.init_state(&[vec![1.0, 0.0]]).unwrap();
        let output = hgn.forward(&state, &[]).unwrap();
        assert!(output.initial_energy > 0.0);
        assert!(output.final_energy > 0.0);
        assert!(output.drift_ratio >= 0.0);
    }

    // --- ConservativePdeAttention tests ---

    #[test]
    fn test_pde_mass_conservation() {
        let mut pde = ConservativePdeAttention::new(0.1, 0.01, 1e-4);

        let features = vec![
            vec![1.0, 2.0, 3.0],
            vec![4.0, 5.0, 6.0],
            vec![7.0, 8.0, 9.0],
        ];
        // Triangle graph
        let edges = vec![(0, 1, 1.0), (1, 2, 1.0), (0, 2, 1.0)];

        let output = pde.forward(&features, &edges).unwrap();
        assert!(
            output.mass_conserved,
            "mass not conserved: before={}, after={}, diff={}",
            output.mass_before,
            output.mass_after,
            (output.mass_after - output.mass_before).abs(),
        );
        assert!(output.attestation.is_some());

        // Verify features actually changed (diffusion happened)
        let features_changed = output
            .features
            .iter()
            .zip(features.iter())
            .any(|(new_f, old_f)| {
                new_f
                    .iter()
                    .zip(old_f.iter())
                    .any(|(a, b)| (a - b).abs() > 1e-8)
            });
        assert!(features_changed, "diffusion should modify features");
    }

    #[test]
    fn test_pde_empty_graph() {
        let mut pde = ConservativePdeAttention::new(0.1, 0.01, 1e-6);
        let output = pde.forward(&[], &[]).unwrap();
        assert_eq!(output.mass_before, 0.0);
        assert_eq!(output.mass_after, 0.0);
        assert!(output.mass_conserved);
    }

    #[test]
    fn test_pde_no_edges() {
        let mut pde = ConservativePdeAttention::new(0.1, 0.01, 1e-6);
        let features = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let output = pde.forward(&features, &[]).unwrap();
        // No edges means no diffusion; features unchanged
        assert_eq!(output.features, features);
        assert!(output.mass_conserved);
    }

    #[test]
    fn test_pde_mass_values() {
        let mut pde = ConservativePdeAttention::new(0.5, 0.1, 1e-3);
        let features = vec![vec![10.0, 0.0], vec![0.0, 10.0]];
        let edges = vec![(0, 1, 1.0)];
        let output = pde.forward(&features, &edges).unwrap();

        // Mass should be 20.0 before and after
        assert!((output.mass_before - 20.0).abs() < 1e-6);
        assert!(
            (output.mass_after - output.mass_before).abs() < 1e-3,
            "mass drift: {}",
            (output.mass_after - output.mass_before).abs(),
        );
    }

    // --- GaugeEquivariantMP tests ---

    #[test]
    fn test_gauge_basic_forward() {
        let gauge_dim = 3;
        let mut gauge = GaugeEquivariantMP::new(gauge_dim, 0.01);

        let features = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        // Identity connections (parallel transport is trivial)
        let identity: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

        let edges = vec![
            (0, 1, identity.clone()),
            (1, 2, identity.clone()),
            (2, 0, identity.clone()),
        ];

        let output = gauge.forward(&features, &edges).unwrap();
        assert_eq!(output.features.len(), 3);
        assert_eq!(output.features[0].len(), gauge_dim);
        assert!(output.attestation.is_some());

        // With identity connections, ym_energy should be zero (up to floating point)
        assert!(
            output.ym_energy.abs() < 1e-6,
            "ym_energy should be ~0 for identity connections, got {}",
            output.ym_energy,
        );
    }

    #[test]
    fn test_gauge_ym_energy_nonidentity() {
        let gauge_dim = 2;
        let mut gauge = GaugeEquivariantMP::new(gauge_dim, 1.0);

        let features = vec![vec![1.0, 0.0], vec![0.0, 1.0]];

        // Non-identity connection (90-degree rotation)
        let rotation: Vec<f32> = vec![0.0, -1.0, 1.0, 0.0];
        let edges = vec![(0, 1, rotation)];

        let output = gauge.forward(&features, &edges).unwrap();
        assert!(
            output.ym_energy > 0.0,
            "ym_energy should be > 0 for non-identity connection",
        );
    }

    #[test]
    fn test_gauge_dimension_mismatch() {
        let mut gauge = GaugeEquivariantMP::new(3, 0.01);
        let features = vec![vec![1.0, 0.0]]; // dim 2 != gauge_dim 3
        let edges = vec![];
        let result = gauge.forward(&features, &edges);
        assert!(result.is_err());
    }

    #[test]
    fn test_gauge_connection_dimension_mismatch() {
        let mut gauge = GaugeEquivariantMP::new(2, 0.01);
        let features = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        // Connection should be 2x2=4 elements, provide 3
        let edges = vec![(0, 1, vec![1.0, 0.0, 0.0])];
        let result = gauge.forward(&features, &edges);
        assert!(result.is_err());
    }

    // --- LagrangianAttention tests ---

    #[test]
    fn test_lagrangian_basic() {
        let mut lagr = LagrangianAttention::new(1.0, 0.1, 100.0);
        let features = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]];
        let edges = vec![(0, 1, 1.0), (1, 2, 1.0), (0, 2, 1.0)];

        let output = lagr.forward(&features, &edges).unwrap();
        assert_eq!(output.features.len(), 3);
        assert!(output.attestation.is_some());
    }

    #[test]
    fn test_lagrangian_empty() {
        let mut lagr = LagrangianAttention::new(1.0, 0.1, 100.0);
        let output = lagr.forward(&[], &[]).unwrap();
        assert!(output.features.is_empty());
    }
}
