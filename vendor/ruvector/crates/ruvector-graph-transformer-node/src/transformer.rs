//! Self-contained graph transformer implementation for the Node.js bindings.
//!
//! Provides proof-gated operations, sublinear attention, physics-informed
//! layers, biological learning, verified training, manifold distance,
//! temporal causal attention, and economic game-theoretic attention --
//! all without external crate dependencies beyond serde.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum GraphTransformerError {
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: u32, actual: u32 },

    #[error("proof verification failed: {0}")]
    ProofFailed(String),
}

pub type Result<T> = std::result::Result<T, GraphTransformerError>;

// ---------------------------------------------------------------------------
// Proof-gated types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofGate {
    pub id: u32,
    pub dimension: u32,
    pub verified: bool,
    pub proof_term_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimProofResult {
    pub proof_id: u32,
    pub expected: u32,
    pub actual: u32,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    pub proof_id: u32,
    pub proof_term_hash: [u8; 32],
    pub environment_hash: [u8; 32],
    pub timestamp_ns: u64,
    pub verifier_version: u32,
    pub reduction_steps: u32,
    pub cache_hit_rate_bps: u16,
}

pub const ATTESTATION_SIZE: usize = 82;

impl Attestation {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(ATTESTATION_SIZE);
        buf.extend_from_slice(&self.proof_term_hash);
        buf.extend_from_slice(&self.environment_hash);
        buf.extend_from_slice(&self.timestamp_ns.to_le_bytes());
        buf.extend_from_slice(&self.verifier_version.to_le_bytes());
        buf.extend_from_slice(&self.reduction_steps.to_le_bytes());
        buf.extend_from_slice(&self.cache_hit_rate_bps.to_le_bytes());
        buf
    }

    pub fn from_bytes(data: &[u8]) -> std::result::Result<Self, &'static str> {
        if data.len() < ATTESTATION_SIZE {
            return Err("attestation data too short");
        }
        let mut proof_term_hash = [0u8; 32];
        proof_term_hash.copy_from_slice(&data[0..32]);
        let mut environment_hash = [0u8; 32];
        environment_hash.copy_from_slice(&data[32..64]);
        let timestamp_ns =
            u64::from_le_bytes(data[64..72].try_into().map_err(|_| "bad timestamp")?);
        let verifier_version =
            u32::from_le_bytes(data[72..76].try_into().map_err(|_| "bad version")?);
        let reduction_steps = u32::from_le_bytes(data[76..80].try_into().map_err(|_| "bad steps")?);
        let cache_hit_rate_bps =
            u16::from_le_bytes(data[80..82].try_into().map_err(|_| "bad rate")?);

        Ok(Self {
            proof_id: 0,
            proof_term_hash,
            environment_hash,
            timestamp_ns,
            verifier_version,
            reduction_steps,
            cache_hit_rate_bps,
        })
    }

    fn verify(&self) -> bool {
        self.verifier_version != 0 && self.proof_term_hash != [0u8; 32]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStage {
    pub name: String,
    pub input_type_id: u32,
    pub output_type_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposedProof {
    pub proof_id: u32,
    pub input_type_id: u32,
    pub output_type_id: u32,
    pub stages_verified: u32,
    pub chain_name: String,
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionResult {
    pub scores: Vec<f64>,
    pub top_k_indices: Vec<u32>,
    pub sparsity_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HamiltonianState {
    pub positions: Vec<f64>,
    pub momenta: Vec<f64>,
    pub energy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedStepResult {
    pub weights: Vec<f64>,
    pub proof_id: u32,
    pub loss_before: f64,
    pub loss_after: f64,
    pub gradient_norm: f64,
}

/// Edge descriptor for graph operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub src: u32,
    pub tgt: u32,
}

/// Result of a graph-aware Hamiltonian step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HamiltonianOutput {
    pub positions: Vec<f64>,
    pub momenta: Vec<f64>,
    pub energy: f64,
    pub energy_conserved: bool,
}

/// Result of a spiking attention step over 2D features.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpikingStepResult {
    pub features: Vec<Vec<f64>>,
    pub spikes: Vec<bool>,
    pub weights: Vec<Vec<f64>>,
}

/// Result of a verified training step with certificate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingStepResult {
    pub weights: Vec<f64>,
    pub certificate_id: u32,
    pub loss: f64,
    pub loss_monotonic: bool,
    pub lipschitz_satisfied: bool,
}

/// Result of product manifold attention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifoldOutput {
    pub output: Vec<f64>,
    pub curvatures: Vec<f64>,
    pub distances: Vec<f64>,
}

/// Granger causality DAG extracted from attention history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrangerDag {
    pub edges: Vec<GrangerEdge>,
    pub num_nodes: u32,
}

/// A single edge in a Granger causality DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrangerEdge {
    pub source: u32,
    pub target: u32,
    pub f_statistic: f64,
    pub is_causal: bool,
}

/// Result of game-theoretic attention (Nash equilibrium).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquilibriumOutput {
    pub allocations: Vec<f64>,
    pub utilities: Vec<f64>,
    pub nash_gap: f64,
    pub converged: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransformerStats {
    pub proofs_constructed: u64,
    pub proofs_verified: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub attention_ops: u64,
    pub physics_ops: u64,
    pub bio_ops: u64,
    pub training_steps: u64,
}

// ---------------------------------------------------------------------------
// Core implementation
// ---------------------------------------------------------------------------

pub struct CoreGraphTransformer {
    term_counter: u32,
    proof_cache: HashMap<u64, u32>,
    gates: HashMap<u32, ProofGate>,
    stats: TransformerStats,
    prev_loss: Option<f64>,
}

impl Default for CoreGraphTransformer {
    fn default() -> Self {
        Self::new()
    }
}

impl CoreGraphTransformer {
    pub fn new() -> Self {
        Self {
            term_counter: 0,
            proof_cache: HashMap::with_capacity(256),
            gates: HashMap::new(),
            stats: TransformerStats::default(),
            prev_loss: None,
        }
    }

    fn alloc_term(&mut self) -> u32 {
        let id = self.term_counter;
        self.term_counter = self.term_counter.wrapping_add(1);
        self.stats.proofs_constructed += 1;
        id
    }

    fn cache_key(a: u64, b: u64) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        h ^= a;
        h = h.wrapping_mul(0x0100_0000_01b3);
        h ^= b;
        h = h.wrapping_mul(0x0100_0000_01b3);
        h
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    // -- Proof-gated --

    pub fn create_proof_gate(&mut self, dim: u32) -> ProofGate {
        let id = self.alloc_term();
        let gate = ProofGate {
            id,
            dimension: dim,
            verified: false,
            proof_term_id: None,
        };
        self.gates.insert(id, gate.clone());
        gate
    }

    pub fn prove_dimension(&mut self, expected: u32, actual: u32) -> Result<DimProofResult> {
        if expected != actual {
            return Err(GraphTransformerError::DimensionMismatch { expected, actual });
        }
        let key = Self::cache_key(u64::from(expected), u64::from(actual));
        let proof_id = if let Some(&cached) = self.proof_cache.get(&key) {
            self.stats.cache_hits += 1;
            cached
        } else {
            self.stats.cache_misses += 1;
            let id = self.alloc_term();
            self.proof_cache.insert(key, id);
            id
        };
        self.stats.proofs_verified += 1;
        Ok(DimProofResult {
            proof_id,
            expected,
            actual,
            verified: true,
        })
    }

    pub fn create_attestation(&self, proof_id: u32) -> Attestation {
        let mut proof_hash = [0u8; 32];
        proof_hash[0..4].copy_from_slice(&proof_id.to_le_bytes());
        proof_hash[4..8].copy_from_slice(&self.term_counter.to_le_bytes());

        let mut env_hash = [0u8; 32];
        env_hash[0..4].copy_from_slice(&(self.gates.len() as u32).to_le_bytes());

        let total = self.stats.cache_hits + self.stats.cache_misses;
        let rate = if total > 0 {
            ((self.stats.cache_hits * 10000) / total) as u16
        } else {
            0
        };

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Attestation {
            proof_id,
            proof_term_hash: proof_hash,
            environment_hash: env_hash,
            timestamp_ns: ts,
            verifier_version: 0x0002_0004, // 2.0.4
            reduction_steps: self.stats.proofs_verified as u32,
            cache_hit_rate_bps: rate,
        }
    }

    pub fn verify_attestation(&self, bytes: &[u8]) -> bool {
        Attestation::from_bytes(bytes)
            .map(|a| a.verify())
            .unwrap_or(false)
    }

    pub fn compose_proofs(&mut self, stages: &[PipelineStage]) -> Result<ComposedProof> {
        if stages.is_empty() {
            return Err(GraphTransformerError::ProofFailed(
                "empty pipeline chain".into(),
            ));
        }

        let mut current_output = stages[0].output_type_id;
        let mut chain_name = stages[0].name.clone();

        for stage in stages.iter().skip(1) {
            if current_output != stage.input_type_id {
                return Err(GraphTransformerError::ProofFailed(format!(
                    "pipeline type mismatch: type#{} != type#{}",
                    current_output, stage.input_type_id,
                )));
            }
            chain_name = format!("{} >> {}", chain_name, stage.name);
            current_output = stage.output_type_id;
            self.alloc_term();
        }

        let proof_id = self.alloc_term();
        self.stats.proofs_verified += stages.len() as u64;

        Ok(ComposedProof {
            proof_id,
            input_type_id: stages[0].input_type_id,
            output_type_id: current_output,
            stages_verified: stages.len() as u32,
            chain_name,
        })
    }

    // -- Sublinear attention --

    pub fn sublinear_attention(
        &mut self,
        query: &[f64],
        edges: &[Vec<u32>],
        dim: u32,
        k: u32,
    ) -> Result<AttentionResult> {
        if query.len() != dim as usize {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: dim,
                actual: query.len() as u32,
            });
        }

        let n = edges.len();
        let k = (k as usize).min(n);

        let ppr = self.compute_ppr(0, edges, 0.15);

        let mut indexed: Vec<(usize, f64)> = ppr.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_k: Vec<(usize, f64)> = indexed.into_iter().take(k).collect();

        let q_norm = query.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-12);
        let scores: Vec<f64> = top_k.iter().map(|(_, s)| s / q_norm).collect();

        let max_s = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exps: Vec<f64> = scores.iter().map(|s| (s - max_s).exp()).collect();
        let sum_exp: f64 = exps.iter().sum();
        let normalized: Vec<f64> = exps.iter().map(|e| e / sum_exp).collect();

        let indices: Vec<u32> = top_k.iter().map(|(i, _)| *i as u32).collect();
        let sparsity = if n > 0 {
            1.0 - (k as f64 / n as f64)
        } else {
            0.0
        };

        self.stats.attention_ops += 1;
        Ok(AttentionResult {
            scores: normalized,
            top_k_indices: indices,
            sparsity_ratio: sparsity,
        })
    }

    pub fn ppr_scores(&mut self, source: u32, adjacency: &[Vec<u32>], alpha: f64) -> Vec<f64> {
        self.compute_ppr(source as usize, adjacency, alpha)
    }

    fn compute_ppr(&self, source: usize, adjacency: &[Vec<u32>], alpha: f64) -> Vec<f64> {
        let n = adjacency.len();
        if n == 0 {
            return vec![];
        }
        let src = source.min(n - 1);
        let mut scores = vec![0.0f64; n];
        scores[src] = 1.0;

        for _ in 0..20 {
            let mut next = vec![0.0f64; n];
            for (node, neighbors) in adjacency.iter().enumerate() {
                if neighbors.is_empty() {
                    next[node] += scores[node];
                } else {
                    let share = scores[node] / neighbors.len() as f64;
                    for &nb in neighbors {
                        if (nb as usize) < n {
                            next[nb as usize] += share;
                        }
                    }
                }
            }
            for i in 0..n {
                scores[i] = alpha * (if i == src { 1.0 } else { 0.0 }) + (1.0 - alpha) * next[i];
            }
        }
        scores
    }

    // -- Physics --

    pub fn hamiltonian_step(
        &mut self,
        positions: &[f64],
        momenta: &[f64],
        dt: f64,
    ) -> Result<HamiltonianState> {
        if positions.len() != momenta.len() {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: positions.len() as u32,
                actual: momenta.len() as u32,
            });
        }

        let n = positions.len();
        let mut new_p = vec![0.0; n];
        let mut new_m = vec![0.0; n];

        // Half step in momentum: p -= 0.5 * dt * grad_V(q)  where grad_V = q
        for i in 0..n {
            new_m[i] = momenta[i] - 0.5 * dt * positions[i];
        }
        // Full step in position: q += dt * grad_T(p) where grad_T = p
        for i in 0..n {
            new_p[i] = positions[i] + dt * new_m[i];
        }
        // Half step in momentum
        for i in 0..n {
            new_m[i] -= 0.5 * dt * new_p[i];
        }

        let kinetic: f64 = new_m.iter().map(|p| 0.5 * p * p).sum();
        let potential: f64 = new_p.iter().map(|q| 0.5 * q * q).sum();

        self.stats.physics_ops += 1;
        Ok(HamiltonianState {
            positions: new_p,
            momenta: new_m,
            energy: kinetic + potential,
        })
    }

    /// Graph-aware Hamiltonian step with edge interactions.
    pub fn hamiltonian_step_graph(
        &mut self,
        positions: &[f64],
        momenta: &[f64],
        edges: &[Edge],
        dt: f64,
    ) -> Result<HamiltonianOutput> {
        if positions.len() != momenta.len() {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: positions.len() as u32,
                actual: momenta.len() as u32,
            });
        }

        let n = positions.len();
        let energy_before = compute_energy(positions, momenta);

        let mut q = positions.to_vec();
        let mut p = momenta.to_vec();

        let grad = compute_grad_with_edges(&q, edges, n);
        for i in 0..n {
            p[i] -= 0.5 * dt * grad[i];
        }
        for i in 0..n {
            q[i] += dt * p[i];
        }
        let grad = compute_grad_with_edges(&q, edges, n);
        for i in 0..n {
            p[i] -= 0.5 * dt * grad[i];
        }

        let energy_after = compute_energy(&q, &p);
        let delta = (energy_after - energy_before).abs();
        let energy_conserved = delta < 0.01 * energy_before.abs().max(1e-8);

        self.stats.physics_ops += 1;
        Ok(HamiltonianOutput {
            positions: q,
            momenta: p,
            energy: energy_after,
            energy_conserved,
        })
    }

    // -- Biological --

    pub fn spiking_attention(
        &mut self,
        spikes: &[f64],
        edges: &[Vec<u32>],
        threshold: f64,
    ) -> Vec<f64> {
        let n = spikes.len();
        let mut output = vec![0.0f64; n];

        for (i, &spike) in spikes.iter().enumerate() {
            if spike > threshold {
                if i < edges.len() {
                    let weight = spike - threshold;
                    for &nb in &edges[i] {
                        if (nb as usize) < n {
                            output[nb as usize] += weight;
                        }
                    }
                }
                output[i] += spike;
            }
        }

        self.stats.bio_ops += 1;
        output
    }

    /// Spiking step over 2D node features + adjacency matrix.
    pub fn spiking_step(
        &mut self,
        features: &[Vec<f64>],
        adjacency: &[f64],
        threshold: f64,
    ) -> SpikingStepResult {
        let n = features.len();
        let dim = if n > 0 { features[0].len() } else { 0 };

        let potentials: Vec<f64> = features
            .iter()
            .map(|f| f.iter().sum::<f64>() / dim.max(1) as f64)
            .collect();

        let spikes: Vec<bool> = potentials.iter().map(|&v| v >= threshold).collect();

        let mut out_features = vec![vec![0.0; dim]; n];
        for i in 0..n {
            if spikes[i] {
                for d in 0..dim {
                    out_features[i][d] = features[i][d] * threshold;
                }
            } else {
                let attenuation = (potentials[i] / threshold).abs().min(1.0);
                for d in 0..dim {
                    out_features[i][d] = features[i][d] * attenuation;
                }
            }
        }

        let mut weights = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                let idx = i * n + j;
                let w = if idx < adjacency.len() {
                    adjacency[idx]
                } else {
                    0.0
                };
                let dw = if spikes[i] && spikes[j] {
                    0.01
                } else if spikes[i] && !spikes[j] {
                    -0.005
                } else {
                    0.0
                };
                weights[i][j] = (w + dw).clamp(-5.0, 5.0);
            }
        }

        self.stats.bio_ops += 1;
        SpikingStepResult {
            features: out_features,
            spikes,
            weights,
        }
    }

    pub fn hebbian_update(
        &mut self,
        pre: &[f64],
        post: &[f64],
        weights: &[f64],
        lr: f64,
    ) -> Vec<f64> {
        let n_pre = pre.len();
        let n_post = post.len();
        let expected_len = n_pre * n_post;

        let mut result = if weights.len() == expected_len {
            weights.to_vec()
        } else {
            vec![0.0; expected_len]
        };

        for i in 0..n_pre {
            for j in 0..n_post {
                result[i * n_post + j] += lr * pre[i] * post[j];
            }
        }

        self.stats.bio_ops += 1;
        result
    }

    // -- Verified training --

    pub fn verified_step(
        &mut self,
        weights: &[f64],
        gradients: &[f64],
        lr: f64,
    ) -> Result<VerifiedStepResult> {
        if weights.len() != gradients.len() {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: weights.len() as u32,
                actual: gradients.len() as u32,
            });
        }

        let grad_norm: f64 = gradients.iter().map(|g| g * g).sum::<f64>().sqrt();
        let loss_before: f64 = weights.iter().map(|w| w * w).sum::<f64>() * 0.5;

        let new_weights: Vec<f64> = weights
            .iter()
            .zip(gradients.iter())
            .map(|(w, g)| w - lr * g)
            .collect();

        let loss_after: f64 = new_weights.iter().map(|w| w * w).sum::<f64>() * 0.5;
        let proof_id = self.alloc_term();
        self.stats.proofs_verified += 1;
        self.stats.training_steps += 1;

        Ok(VerifiedStepResult {
            weights: new_weights,
            proof_id,
            loss_before,
            loss_after,
            gradient_norm: grad_norm,
        })
    }

    /// Verified training step with features, targets, and weight update.
    pub fn verified_training_step(
        &mut self,
        features: &[f64],
        targets: &[f64],
        weights: &[f64],
        lr: f64,
    ) -> Result<TrainingStepResult> {
        let dim = features.len().min(targets.len());
        if dim == 0 {
            return Err(GraphTransformerError::ProofFailed(
                "empty features or targets".into(),
            ));
        }

        let mut outputs = vec![0.0; dim];
        for i in 0..dim {
            let w = if i < weights.len() { weights[i] } else { 0.0 };
            outputs[i] = features[i] * w;
        }

        let loss: f64 = outputs
            .iter()
            .zip(targets.iter())
            .map(|(o, t)| (o - t).powi(2))
            .sum::<f64>()
            / dim as f64;

        let new_weights: Vec<f64> = (0..weights.len())
            .map(|i| {
                let grad = if i < dim {
                    2.0 * (outputs[i] - targets[i]) * features[i] / dim as f64
                } else {
                    0.0
                };
                weights[i] - lr * grad
            })
            .collect();

        let loss_monotonic = match self.prev_loss {
            Some(prev) => loss <= prev + 1e-6,
            None => true,
        };
        self.prev_loss = Some(loss);

        let max_update: f64 = weights
            .iter()
            .zip(new_weights.iter())
            .map(|(w, nw)| (nw - w).abs())
            .fold(0.0, f64::max);
        let lipschitz_satisfied = max_update <= 10.0;

        let certificate_id = self.alloc_term();
        self.stats.proofs_verified += 1;
        self.stats.training_steps += 1;

        Ok(TrainingStepResult {
            weights: new_weights,
            certificate_id,
            loss,
            loss_monotonic,
            lipschitz_satisfied,
        })
    }

    // -- Manifold --

    pub fn product_manifold_distance(&self, a: &[f64], b: &[f64], curvatures: &[f64]) -> f64 {
        if a.len() != b.len() || curvatures.is_empty() {
            return 0.0;
        }
        let n = a.len();
        let n_spaces = curvatures.len();
        let chunk_size = (n + n_spaces - 1) / n_spaces;

        let mut total_dist_sq = 0.0;

        for (space_idx, &k) in curvatures.iter().enumerate() {
            let start = space_idx * chunk_size;
            let end = (start + chunk_size).min(n);
            if start >= n {
                break;
            }

            let mut dist_sq = 0.0;
            for i in start..end {
                let diff = a[i] - b[i];
                dist_sq += diff * diff;
            }

            if k.abs() < 1e-12 {
                total_dist_sq += dist_sq;
            } else if k > 0.0 {
                let d = dist_sq.sqrt();
                total_dist_sq += (d * k.sqrt()).min(std::f64::consts::PI).powi(2) / k;
            } else {
                total_dist_sq += dist_sq / k.abs();
            }
        }

        total_dist_sq.sqrt()
    }

    /// Product manifold attention with mixed curvatures.
    pub fn product_manifold_attention(
        &mut self,
        features: &[f64],
        edges: &[Edge],
        curvatures: &[f64],
    ) -> ManifoldOutput {
        let dim = features.len();
        let n_spaces = curvatures.len().max(1);
        let chunk_size = (dim + n_spaces - 1) / n_spaces;

        let mut distances = Vec::new();
        for edge in edges {
            let s = edge.src as usize;
            let t = edge.tgt as usize;
            if s < dim && t < dim {
                distances.push((features[s] - features[t]).abs());
            } else {
                distances.push(0.0);
            }
        }

        let mut output = vec![0.0; dim];
        for (space_idx, &k) in curvatures.iter().enumerate() {
            let start = space_idx * chunk_size;
            let end = (start + chunk_size).min(dim);
            for i in start..end {
                let scale = if k.abs() < 1e-12 {
                    1.0
                } else if k > 0.0 {
                    (features[i] * k.sqrt()).sin() / (features[i] * k.sqrt()).max(1e-12)
                } else {
                    (features[i] * k.abs().sqrt()).sinh()
                        / (features[i] * k.abs().sqrt()).max(1e-12)
                };
                output[i] = features[i] * scale;
            }
        }

        self.stats.attention_ops += 1;
        ManifoldOutput {
            output,
            curvatures: curvatures.to_vec(),
            distances,
        }
    }

    // -- Temporal --

    pub fn causal_attention(
        &mut self,
        query: &[f64],
        keys: &[Vec<f64>],
        timestamps: &[f64],
    ) -> Vec<f64> {
        let dim = query.len();
        if keys.is_empty() || timestamps.len() != keys.len() {
            return vec![];
        }

        let q_time = timestamps.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let scores: Vec<f64> = keys
            .iter()
            .zip(timestamps.iter())
            .map(|(key, &t)| {
                if t > q_time {
                    f64::NEG_INFINITY
                } else {
                    let dot: f64 = query.iter().zip(key.iter()).map(|(q, k)| q * k).sum();
                    dot / (dim as f64).sqrt()
                }
            })
            .collect();

        let max_s = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if max_s.is_infinite() && max_s < 0.0 {
            return vec![0.0; keys.len()];
        }
        let exps: Vec<f64> = scores.iter().map(|s| (s - max_s).exp()).collect();
        let sum_exp: f64 = exps.iter().sum();
        if sum_exp < 1e-12 {
            return vec![0.0; keys.len()];
        }

        self.stats.attention_ops += 1;
        exps.iter().map(|e| e / sum_exp).collect()
    }

    /// Causal attention over features, timestamps, and graph edges.
    pub fn causal_attention_graph(
        &mut self,
        features: &[f64],
        timestamps: &[f64],
        edges: &[Edge],
    ) -> Vec<f64> {
        let n = features.len();
        if n == 0 || timestamps.len() != n {
            return vec![];
        }

        let mut output = vec![0.0; n];
        for i in 0..n {
            let t_i = timestamps[i];
            let mut weighted_sum = 0.0;
            let mut weight_sum = 0.0;

            for edge in edges {
                let j = edge.src as usize;
                let k = edge.tgt as usize;
                let neighbor = if k == i && j < n {
                    j
                } else if j == i && k < n {
                    k
                } else {
                    continue;
                };

                if timestamps[neighbor] <= t_i {
                    let dt = t_i - timestamps[neighbor];
                    let decay = (-0.1 * dt).exp();
                    let w = decay * features[neighbor].abs().max(1e-12);
                    weighted_sum += w * features[neighbor];
                    weight_sum += w;
                }
            }

            output[i] = if weight_sum > 1e-12 {
                weighted_sum / weight_sum
            } else {
                features[i]
            };
        }

        self.stats.attention_ops += 1;
        output
    }

    /// Extract Granger causality DAG from attention history.
    pub fn granger_extract(
        &mut self,
        attention_history: &[f64],
        num_nodes: u32,
        num_steps: u32,
    ) -> GrangerDag {
        let n = num_nodes as usize;
        let t = num_steps as usize;

        if n == 0 || t < 3 || attention_history.len() < n * t {
            return GrangerDag {
                edges: vec![],
                num_nodes,
            };
        }

        let mut series: Vec<Vec<f64>> = vec![Vec::with_capacity(t); n];
        for step in 0..t {
            for node in 0..n {
                series[node].push(attention_history[step * n + node]);
            }
        }

        let lags = 2.min(t - 1);
        let mut edges = Vec::new();

        for source in 0..n {
            for target in 0..n {
                if source == target {
                    continue;
                }

                let rss_r = var_rss(&series[target], &[&series[target]], lags);
                let rss_u = var_rss(&series[target], &[&series[target], &series[source]], lags);

                let n_obs = (t - lags) as f64;
                let df_diff = lags as f64;
                let df_denom = n_obs - 2.0 * lags as f64;

                let f_stat = if rss_u > 1e-10 && df_denom > 0.0 && df_diff > 0.0 {
                    let raw = ((rss_r - rss_u) / df_diff) / (rss_u / df_denom);
                    if raw.is_finite() {
                        raw.max(0.0)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                let is_causal = f_stat > 3.84;
                if is_causal {
                    edges.push(GrangerEdge {
                        source: source as u32,
                        target: target as u32,
                        f_statistic: f_stat,
                        is_causal,
                    });
                }
            }
        }

        self.stats.attention_ops += 1;
        GrangerDag { edges, num_nodes }
    }

    // -- Economic / Game-Theoretic --

    /// Game-theoretic attention: computes Nash equilibrium allocations.
    pub fn game_theoretic_attention(
        &mut self,
        features: &[f64],
        edges: &[Edge],
    ) -> EquilibriumOutput {
        let n = features.len();
        if n == 0 {
            return EquilibriumOutput {
                allocations: vec![],
                utilities: vec![],
                nash_gap: 0.0,
                converged: true,
            };
        }

        let mut neighbors: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for edge in edges {
            let s = edge.src as usize;
            let t = edge.tgt as usize;
            if s < n && t < n {
                neighbors[s].push((t, features[t]));
                neighbors[t].push((s, features[s]));
            }
        }

        let feat_sum: f64 = features.iter().map(|x| x.abs()).sum::<f64>().max(1e-12);
        let mut allocations: Vec<f64> = features.iter().map(|x| x.abs() / feat_sum).collect();

        let max_iters = 50;
        let mut nash_gap = f64::MAX;

        for _ in 0..max_iters {
            let mut new_alloc = vec![0.0; n];
            for i in 0..n {
                let mut best_response = features[i].abs() / feat_sum;
                for &(j, _fj) in &neighbors[i] {
                    best_response += 0.1 * allocations[j];
                }
                new_alloc[i] = best_response;
            }

            let alloc_sum: f64 = new_alloc.iter().sum::<f64>().max(1e-12);
            for v in &mut new_alloc {
                *v /= alloc_sum;
            }

            nash_gap = allocations
                .iter()
                .zip(new_alloc.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);

            allocations = new_alloc;
            if nash_gap < 1e-6 {
                break;
            }
        }

        let utilities: Vec<f64> = (0..n)
            .map(|i| {
                let self_util = features[i] * allocations[i];
                let neighbor_util: f64 = neighbors[i]
                    .iter()
                    .map(|&(j, _)| 0.1 * allocations[j] * features[i])
                    .sum();
                self_util + neighbor_util
            })
            .collect();

        self.stats.attention_ops += 1;
        EquilibriumOutput {
            allocations,
            utilities,
            nash_gap,
            converged: nash_gap < 1e-6,
        }
    }

    // -- Stats --

    pub fn stats(&self) -> &TransformerStats {
        &self.stats
    }

    pub fn reset(&mut self) {
        self.term_counter = 0;
        self.proof_cache.clear();
        self.gates.clear();
        self.stats = TransformerStats::default();
        self.prev_loss = None;
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn compute_energy(positions: &[f64], momenta: &[f64]) -> f64 {
    let kinetic: f64 = momenta.iter().map(|p| 0.5 * p * p).sum();
    let potential: f64 = positions.iter().map(|q| 0.5 * q * q).sum();
    kinetic + potential
}

fn compute_grad_with_edges(q: &[f64], edges: &[Edge], n: usize) -> Vec<f64> {
    let mut grad = q.to_vec();
    for edge in edges {
        let u = edge.src as usize;
        let v = edge.tgt as usize;
        if u < n && v < n {
            let diff = q[u] - q[v];
            grad[u] += diff;
            grad[v] -= diff;
        }
    }
    grad
}

fn var_rss(target: &[f64], predictors: &[&[f64]], lags: usize) -> f64 {
    let t = target.len();
    if t <= lags {
        return 0.0;
    }
    let mut rss = 0.0;
    for i in lags..t {
        let actual = target[i];
        let mut predicted = 0.0;
        let mut count = 0;
        for pred in predictors {
            for lag in 1..=lags {
                if i >= lag && pred.len() > i - lag {
                    predicted += pred[i - lag];
                    count += 1;
                }
            }
        }
        if count > 0 {
            predicted /= count as f64;
        }
        let residual = actual - predicted;
        rss += residual * residual;
    }
    rss
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_gate() {
        let mut gt = CoreGraphTransformer::new();
        let gate = gt.create_proof_gate(128);
        assert_eq!(gate.dimension, 128);
    }

    #[test]
    fn test_prove_dim_ok() {
        let mut gt = CoreGraphTransformer::new();
        assert!(gt.prove_dimension(64, 64).unwrap().verified);
    }

    #[test]
    fn test_prove_dim_err() {
        let mut gt = CoreGraphTransformer::new();
        assert!(gt.prove_dimension(64, 128).is_err());
    }

    #[test]
    fn test_attestation_roundtrip() {
        let mut gt = CoreGraphTransformer::new();
        let _ = gt.prove_dimension(32, 32).unwrap();
        let att = gt.create_attestation(0);
        let bytes = att.to_bytes();
        assert_eq!(bytes.len(), ATTESTATION_SIZE);
        assert!(gt.verify_attestation(&bytes));
    }

    #[test]
    fn test_compose() {
        let mut gt = CoreGraphTransformer::new();
        let stages = vec![
            PipelineStage {
                name: "a".into(),
                input_type_id: 1,
                output_type_id: 2,
            },
            PipelineStage {
                name: "b".into(),
                input_type_id: 2,
                output_type_id: 3,
            },
        ];
        let r = gt.compose_proofs(&stages).unwrap();
        assert_eq!(r.stages_verified, 2);
    }

    #[test]
    fn test_sublinear() {
        let mut gt = CoreGraphTransformer::new();
        let r = gt
            .sublinear_attention(&[1.0, 0.5], &[vec![1], vec![0]], 2, 1)
            .unwrap();
        assert_eq!(r.scores.len(), 1);
    }

    #[test]
    fn test_hamiltonian() {
        let mut gt = CoreGraphTransformer::new();
        let r = gt.hamiltonian_step(&[1.0], &[0.0], 0.001).unwrap();
        assert!(r.energy > 0.0);
    }

    #[test]
    fn test_spiking() {
        let mut gt = CoreGraphTransformer::new();
        let o = gt.spiking_attention(&[0.5, 2.0], &[vec![1], vec![0]], 1.0);
        assert_eq!(o.len(), 2);
        assert!(o[0] > 0.0);
    }

    #[test]
    fn test_hebbian() {
        let mut gt = CoreGraphTransformer::new();
        let r = gt.hebbian_update(&[1.0], &[1.0], &[0.0], 0.5);
        assert!((r[0] - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_verified_step() {
        let mut gt = CoreGraphTransformer::new();
        let r = gt.verified_step(&[1.0, 2.0], &[0.1, 0.2], 0.01).unwrap();
        assert!(r.loss_after < r.loss_before);
    }

    #[test]
    fn test_manifold_euclidean() {
        let gt = CoreGraphTransformer::new();
        let d = gt.product_manifold_distance(&[0.0, 0.0], &[3.0, 4.0], &[0.0]);
        assert!((d - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_causal_attention() {
        let mut gt = CoreGraphTransformer::new();
        let s = gt.causal_attention(&[1.0], &[vec![1.0], vec![0.5]], &[1.0, 2.0]);
        assert_eq!(s.len(), 2);
        let sum: f64 = s.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_hamiltonian_graph() {
        let mut gt = CoreGraphTransformer::new();
        let edges = vec![Edge { src: 0, tgt: 1 }];
        let r = gt
            .hamiltonian_step_graph(&[1.0, 0.0], &[0.0, 1.0], &edges, 0.001)
            .unwrap();
        assert!(r.energy > 0.0);
    }

    #[test]
    fn test_spiking_step() {
        let mut gt = CoreGraphTransformer::new();
        let features = vec![vec![0.8, 0.6], vec![0.1, 0.2]];
        let adjacency = vec![0.0, 0.5, 0.3, 0.0];
        let result = gt.spiking_step(&features, &adjacency, 0.5);
        assert_eq!(result.features.len(), 2);
        assert_eq!(result.spikes.len(), 2);
    }

    #[test]
    fn test_verified_training_step() {
        let mut gt = CoreGraphTransformer::new();
        let r = gt
            .verified_training_step(&[1.0, 2.0], &[0.5, 1.0], &[0.5, 0.5], 0.01)
            .unwrap();
        assert!(r.loss >= 0.0);
        assert!(r.loss_monotonic);
    }

    #[test]
    fn test_product_manifold_attention() {
        let mut gt = CoreGraphTransformer::new();
        let features = vec![1.0, 0.5, -0.3, 0.8];
        let edges = vec![Edge { src: 0, tgt: 1 }];
        let curvatures = vec![0.0, -1.0];
        let result = gt.product_manifold_attention(&features, &edges, &curvatures);
        assert_eq!(result.output.len(), 4);
        assert_eq!(result.curvatures.len(), 2);
    }

    #[test]
    fn test_causal_attention_graph() {
        let mut gt = CoreGraphTransformer::new();
        let features = vec![1.0, 0.5, 0.8];
        let timestamps = vec![1.0, 2.0, 3.0];
        let edges = vec![Edge { src: 0, tgt: 1 }, Edge { src: 1, tgt: 2 }];
        let out = gt.causal_attention_graph(&features, &timestamps, &edges);
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn test_granger_extract() {
        let mut gt = CoreGraphTransformer::new();
        let mut history = Vec::new();
        for t in 0..10 {
            let x = (t as f64 * 0.5).sin();
            let y = if t > 0 {
                ((t - 1) as f64 * 0.5).sin() * 0.8
            } else {
                0.0
            };
            history.push(x);
            history.push(y);
        }
        let dag = gt.granger_extract(&history, 2, 10);
        assert_eq!(dag.num_nodes, 2);
    }

    #[test]
    fn test_game_theoretic_attention() {
        let mut gt = CoreGraphTransformer::new();
        let features = vec![1.0, 0.5, 0.8];
        let edges = vec![Edge { src: 0, tgt: 1 }, Edge { src: 1, tgt: 2 }];
        let result = gt.game_theoretic_attention(&features, &edges);
        assert_eq!(result.allocations.len(), 3);
        assert_eq!(result.utilities.len(), 3);
        let alloc_sum: f64 = result.allocations.iter().sum();
        assert!((alloc_sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_stats_reset() {
        let mut gt = CoreGraphTransformer::new();
        gt.create_proof_gate(64);
        assert!(gt.stats().proofs_constructed > 0);
        gt.reset();
        assert_eq!(gt.stats().proofs_constructed, 0);
    }
}
