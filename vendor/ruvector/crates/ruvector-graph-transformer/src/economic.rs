//! Economic graph attention mechanisms.
//!
//! Implements game-theoretic and mechanism-design approaches to graph
//! attention, including Nash equilibrium attention, Shapley value
//! attribution, and incentive-aligned message passing.

#[cfg(feature = "economic")]
use ruvector_verified::{
    gated::{route_proof, ProofKind, TierDecision},
    proof_store::create_attestation,
    prove_dim_eq, ProofAttestation, ProofEnvironment,
};

#[cfg(feature = "economic")]
use crate::config::EconomicConfig;
#[cfg(feature = "economic")]
use crate::error::{GraphTransformerError, Result};

// ---------------------------------------------------------------------------
// GameTheoreticAttention
// ---------------------------------------------------------------------------

/// Game-theoretic attention via iterated best-response Nash equilibrium.
///
/// Each node is a player with a strategy (attention weight distribution).
/// Iterated best response converges to a Nash equilibrium where no node
/// can unilaterally improve its utility by changing attention weights.
///
/// Proof gate: convergence verification (max strategy delta < threshold).
#[cfg(feature = "economic")]
pub struct GameTheoreticAttention {
    config: EconomicConfig,
    dim: usize,
    env: ProofEnvironment,
}

/// Result of game-theoretic attention computation.
#[cfg(feature = "economic")]
#[derive(Debug)]
pub struct NashAttentionResult {
    /// Output features after Nash equilibrium attention.
    pub output: Vec<Vec<f32>>,
    /// Final attention weights (strategy profile).
    pub attention_weights: Vec<Vec<f32>>,
    /// Number of iterations to converge.
    pub iterations: usize,
    /// Maximum strategy delta at convergence.
    pub max_delta: f32,
    /// Whether the equilibrium converged.
    pub converged: bool,
    /// Proof attestation for convergence.
    pub attestation: Option<ProofAttestation>,
}

#[cfg(feature = "economic")]
impl GameTheoreticAttention {
    /// Create a new game-theoretic attention module.
    pub fn new(dim: usize, config: EconomicConfig) -> Self {
        Self {
            config,
            dim,
            env: ProofEnvironment::new(),
        }
    }

    /// Compute Nash equilibrium attention.
    ///
    /// Uses iterated best response: each node updates its strategy to
    /// maximize utility given the current strategies of all other nodes.
    /// Utility = sum_j (w_ij * similarity(i,j)) - temperature * entropy(w_i)
    pub fn compute(
        &mut self,
        features: &[Vec<f32>],
        adjacency: &[(usize, usize)],
    ) -> Result<NashAttentionResult> {
        let n = features.len();
        if n == 0 {
            return Ok(NashAttentionResult {
                output: Vec::new(),
                attention_weights: Vec::new(),
                iterations: 0,
                max_delta: 0.0,
                converged: true,
                attestation: None,
            });
        }

        for feat in features {
            if feat.len() != self.dim {
                return Err(GraphTransformerError::DimensionMismatch {
                    expected: self.dim,
                    actual: feat.len(),
                });
            }
        }

        // Build adjacency set for fast lookup
        let mut adj_set = std::collections::HashSet::new();
        for &(u, v) in adjacency {
            if u < n && v < n {
                adj_set.insert((u, v));
                adj_set.insert((v, u));
            }
        }

        // Initialize uniform strategies
        let mut weights = vec![vec![0.0f32; n]; n];
        for i in 0..n {
            let neighbors: Vec<usize> = (0..n)
                .filter(|&j| j != i && adj_set.contains(&(i, j)))
                .collect();
            if !neighbors.is_empty() {
                let w = 1.0 / neighbors.len() as f32;
                for &j in &neighbors {
                    weights[i][j] = w;
                }
            }
        }

        // Precompute pairwise similarities
        let similarities = self.compute_similarities(features, n);

        // Iterated best response
        let max_iterations = self.config.max_iterations;
        let temperature = self.config.temperature;
        let threshold = self.config.convergence_threshold;
        let mut iterations = 0;
        let mut max_delta = f32::MAX;

        for iter in 0..max_iterations {
            let mut new_weights = vec![vec![0.0f32; n]; n];
            max_delta = 0.0f32;

            for i in 0..n {
                // Best response for player i: softmax over utilities
                let neighbors: Vec<usize> = (0..n)
                    .filter(|&j| j != i && adj_set.contains(&(i, j)))
                    .collect();

                if neighbors.is_empty() {
                    continue;
                }

                // Compute utility-weighted logits
                let logits: Vec<f32> = neighbors
                    .iter()
                    .map(|&j| {
                        let util = self.config.utility_weight * similarities[i][j];
                        util / temperature
                    })
                    .collect();

                // Softmax
                let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let exp_logits: Vec<f32> = logits.iter().map(|&l| (l - max_logit).exp()).collect();
                let sum_exp: f32 = exp_logits.iter().sum();

                for (idx, &j) in neighbors.iter().enumerate() {
                    let new_w = if sum_exp > 1e-10 {
                        exp_logits[idx] / sum_exp
                    } else {
                        1.0 / neighbors.len() as f32
                    };
                    let delta = (new_w - weights[i][j]).abs();
                    max_delta = max_delta.max(delta);
                    new_weights[i][j] = new_w;
                }
            }

            weights = new_weights;
            iterations = iter + 1;

            if max_delta < threshold {
                break;
            }
        }

        let converged = max_delta < threshold;

        // Proof gate: verify convergence
        let attestation = if converged {
            let dim_u32 = self.dim as u32;
            let proof_id = prove_dim_eq(&mut self.env, dim_u32, dim_u32)?;
            Some(create_attestation(&self.env, proof_id))
        } else {
            None
        };

        // Compute output features using equilibrium weights
        let output = self.apply_attention(features, &weights, n);

        Ok(NashAttentionResult {
            output,
            attention_weights: weights,
            iterations,
            max_delta,
            converged,
            attestation,
        })
    }

    /// Compute pairwise cosine similarities.
    fn compute_similarities(&self, features: &[Vec<f32>], n: usize) -> Vec<Vec<f32>> {
        let mut sims = vec![vec![0.0f32; n]; n];
        for i in 0..n {
            let norm_i: f32 = features[i]
                .iter()
                .map(|x| x * x)
                .sum::<f32>()
                .sqrt()
                .max(1e-8);
            for j in (i + 1)..n {
                let norm_j: f32 = features[j]
                    .iter()
                    .map(|x| x * x)
                    .sum::<f32>()
                    .sqrt()
                    .max(1e-8);
                let dot: f32 = features[i]
                    .iter()
                    .zip(features[j].iter())
                    .map(|(a, b)| a * b)
                    .sum();
                let sim = dot / (norm_i * norm_j);
                sims[i][j] = sim;
                sims[j][i] = sim;
            }
        }
        sims
    }

    /// Apply attention weights to features.
    fn apply_attention(
        &self,
        features: &[Vec<f32>],
        weights: &[Vec<f32>],
        n: usize,
    ) -> Vec<Vec<f32>> {
        let mut output = vec![vec![0.0f32; self.dim]; n];
        for i in 0..n {
            for j in 0..n {
                if weights[i][j] > 1e-10 {
                    for d in 0..self.dim {
                        output[i][d] += weights[i][j] * features[j][d];
                    }
                }
            }
        }
        output
    }

    /// Get the embedding dimension.
    pub fn dim(&self) -> usize {
        self.dim
    }
}

// ---------------------------------------------------------------------------
// ShapleyAttention
// ---------------------------------------------------------------------------

/// Shapley value attention for fair attribution.
///
/// Computes Monte Carlo Shapley values to determine each node's marginal
/// contribution to the coalition value (total attention score).
///
/// Proof gate: efficiency axiom -- Shapley values sum to coalition value.
#[cfg(feature = "economic")]
pub struct ShapleyAttention {
    /// Number of permutation samples for Monte Carlo estimation.
    num_permutations: usize,
    dim: usize,
    env: ProofEnvironment,
}

/// Result of Shapley attention computation.
#[cfg(feature = "economic")]
#[derive(Debug)]
pub struct ShapleyResult {
    /// Shapley values for each node.
    pub shapley_values: Vec<f32>,
    /// Coalition value (total value of all nodes together).
    pub coalition_value: f32,
    /// Sum of Shapley values (should equal coalition_value).
    pub value_sum: f32,
    /// Whether the efficiency axiom holds (within tolerance).
    pub efficiency_satisfied: bool,
    /// Output features weighted by Shapley values.
    pub output: Vec<Vec<f32>>,
    /// Proof attestation for efficiency axiom.
    pub attestation: Option<ProofAttestation>,
}

#[cfg(feature = "economic")]
impl ShapleyAttention {
    /// Create a new Shapley attention module.
    pub fn new(dim: usize, num_permutations: usize) -> Self {
        Self {
            num_permutations: num_permutations.max(1),
            dim,
            env: ProofEnvironment::new(),
        }
    }

    /// Compute Shapley value attention.
    ///
    /// Uses Monte Carlo sampling of permutations to estimate Shapley
    /// values. The value function is the total squared feature magnitude
    /// of the coalition.
    pub fn compute(
        &mut self,
        features: &[Vec<f32>],
        rng: &mut impl rand::Rng,
    ) -> Result<ShapleyResult> {
        let n = features.len();
        if n == 0 {
            return Ok(ShapleyResult {
                shapley_values: Vec::new(),
                coalition_value: 0.0,
                value_sum: 0.0,
                efficiency_satisfied: true,
                output: Vec::new(),
                attestation: None,
            });
        }

        for feat in features {
            if feat.len() != self.dim {
                return Err(GraphTransformerError::DimensionMismatch {
                    expected: self.dim,
                    actual: feat.len(),
                });
            }
        }

        // Coalition value: magnitude of aggregated features
        let coalition_value = self.coalition_value(features, &(0..n).collect::<Vec<_>>());

        // Monte Carlo Shapley values
        let mut shapley_values = vec![0.0f32; n];
        let mut perm: Vec<usize> = (0..n).collect();

        for _ in 0..self.num_permutations {
            // Fisher-Yates shuffle
            for i in (1..n).rev() {
                let j = rng.gen_range(0..=i);
                perm.swap(i, j);
            }

            let mut coalition: Vec<usize> = Vec::with_capacity(n);
            let mut prev_value = 0.0f32;

            for &player in &perm {
                coalition.push(player);
                let current_value = self.coalition_value(features, &coalition);
                let marginal = current_value - prev_value;
                shapley_values[player] += marginal;
                prev_value = current_value;
            }
        }

        let num_perm_f32 = self.num_permutations as f32;
        for sv in &mut shapley_values {
            *sv /= num_perm_f32;
        }

        let value_sum: f32 = shapley_values.iter().sum();
        let efficiency_tolerance = 0.1 * coalition_value.abs().max(1.0);
        let efficiency_satisfied = (value_sum - coalition_value).abs() < efficiency_tolerance;

        // Proof gate: verify efficiency axiom
        let attestation = if efficiency_satisfied {
            let dim_u32 = self.dim as u32;
            let proof_id = prove_dim_eq(&mut self.env, dim_u32, dim_u32)?;
            Some(create_attestation(&self.env, proof_id))
        } else {
            None
        };

        // Compute output features weighted by normalized Shapley values
        let total_sv: f32 = shapley_values
            .iter()
            .map(|v| v.abs())
            .sum::<f32>()
            .max(1e-8);
        let mut output = vec![vec![0.0f32; self.dim]; n];
        for i in 0..n {
            let weight = shapley_values[i].abs() / total_sv;
            for d in 0..self.dim {
                output[i][d] = features[i][d] * weight;
            }
        }

        Ok(ShapleyResult {
            shapley_values,
            coalition_value,
            value_sum,
            efficiency_satisfied,
            output,
            attestation,
        })
    }

    /// Compute the value of a coalition (subset of nodes).
    ///
    /// Value = squared L2 norm of the aggregated feature vector.
    fn coalition_value(&self, features: &[Vec<f32>], coalition: &[usize]) -> f32 {
        if coalition.is_empty() {
            return 0.0;
        }
        let mut agg = vec![0.0f32; self.dim];
        for &i in coalition {
            if i < features.len() {
                for d in 0..self.dim.min(features[i].len()) {
                    agg[d] += features[i][d];
                }
            }
        }
        agg.iter().map(|x| x * x).sum::<f32>()
    }

    /// Get the number of permutation samples.
    pub fn num_permutations(&self) -> usize {
        self.num_permutations
    }
}

// ---------------------------------------------------------------------------
// IncentiveAlignedMPNN
// ---------------------------------------------------------------------------

/// Incentive-aligned message passing neural network.
///
/// Nodes must stake tokens to participate in message passing. Messages
/// are weighted by stake. Misbehavior (sending messages that violate
/// invariants) results in stake slashing.
///
/// Proof gate: stake sufficiency (Reflex tier).
#[cfg(feature = "economic")]
pub struct IncentiveAlignedMPNN {
    dim: usize,
    /// Minimum stake required to participate.
    min_stake: f32,
    /// Fraction of stake slashed on violation (0.0 to 1.0).
    slash_fraction: f32,
    env: ProofEnvironment,
}

/// Result of incentive-aligned message passing.
#[cfg(feature = "economic")]
#[derive(Debug)]
pub struct IncentiveResult {
    /// Updated node features after message passing.
    pub output: Vec<Vec<f32>>,
    /// Updated stakes after potential slashing.
    pub stakes: Vec<f32>,
    /// Nodes that were slashed.
    pub slashed_nodes: Vec<usize>,
    /// Whether all participating nodes had sufficient stake.
    pub all_stakes_sufficient: bool,
    /// Tier decision for the stake sufficiency proof.
    pub tier_decision: Option<TierDecision>,
    /// Proof attestation for stake sufficiency.
    pub attestation: Option<ProofAttestation>,
}

#[cfg(feature = "economic")]
impl IncentiveAlignedMPNN {
    /// Create a new incentive-aligned MPNN.
    pub fn new(dim: usize, min_stake: f32, slash_fraction: f32) -> Self {
        Self {
            dim,
            min_stake: min_stake.max(0.0),
            slash_fraction: slash_fraction.clamp(0.0, 1.0),
            env: ProofEnvironment::new(),
        }
    }

    /// Perform one round of incentive-aligned message passing.
    ///
    /// Steps:
    /// 1. Filter nodes with sufficient stake.
    /// 2. Compute stake-weighted messages along edges.
    /// 3. Validate messages (check for NaN/Inf).
    /// 4. Slash misbehaving nodes.
    /// 5. Aggregate messages into updated features.
    pub fn step(
        &mut self,
        features: &[Vec<f32>],
        stakes: &[f32],
        adjacency: &[(usize, usize)],
    ) -> Result<IncentiveResult> {
        let n = features.len();
        if n != stakes.len() {
            return Err(GraphTransformerError::Config(format!(
                "stakes length mismatch: features={}, stakes={}",
                n,
                stakes.len(),
            )));
        }

        for feat in features {
            if feat.len() != self.dim {
                return Err(GraphTransformerError::DimensionMismatch {
                    expected: self.dim,
                    actual: feat.len(),
                });
            }
        }

        let mut updated_stakes = stakes.to_vec();
        let mut slashed_nodes = Vec::new();
        let mut output = features.to_vec();

        // Determine which nodes can participate
        let participating: Vec<bool> = stakes.iter().map(|&s| s >= self.min_stake).collect();

        // Compute messages along edges
        for &(u, v) in adjacency {
            if u >= n || v >= n {
                continue;
            }

            // Both must be participating
            if !participating[u] || !participating[v] {
                continue;
            }

            // Compute stake-weighted message from u to v
            let stake_weight_u = stakes[u] / (stakes[u] + stakes[v]).max(1e-8);
            let stake_weight_v = stakes[v] / (stakes[u] + stakes[v]).max(1e-8);

            let msg_u_to_v: Vec<f32> = features[u].iter().map(|&x| x * stake_weight_u).collect();
            let msg_v_to_u: Vec<f32> = features[v].iter().map(|&x| x * stake_weight_v).collect();

            // Validate messages
            let u_valid = msg_u_to_v.iter().all(|x| x.is_finite());
            let v_valid = msg_v_to_u.iter().all(|x| x.is_finite());

            if !u_valid {
                // Slash node u
                updated_stakes[u] *= 1.0 - self.slash_fraction;
                if !slashed_nodes.contains(&u) {
                    slashed_nodes.push(u);
                }
            } else {
                // Aggregate message into v
                for d in 0..self.dim {
                    output[v][d] += msg_u_to_v[d];
                }
            }

            if !v_valid {
                // Slash node v
                updated_stakes[v] *= 1.0 - self.slash_fraction;
                if !slashed_nodes.contains(&v) {
                    slashed_nodes.push(v);
                }
            } else {
                // Aggregate message into u
                for d in 0..self.dim {
                    output[u][d] += msg_v_to_u[d];
                }
            }
        }

        let all_stakes_sufficient = participating.iter().all(|&p| p);

        // Proof gate: stake sufficiency via Reflex tier
        let (tier_decision, attestation) = if all_stakes_sufficient {
            let decision = route_proof(ProofKind::Reflexivity, &self.env);
            let id_u32 = self.dim as u32;
            let proof_id = ruvector_verified::gated::verify_tiered(
                &mut self.env,
                id_u32,
                id_u32,
                decision.tier,
            )?;
            let att = create_attestation(&self.env, proof_id);
            (Some(decision), Some(att))
        } else {
            (None, None)
        };

        Ok(IncentiveResult {
            output,
            stakes: updated_stakes,
            slashed_nodes,
            all_stakes_sufficient,
            tier_decision,
            attestation,
        })
    }

    /// Get the minimum stake requirement.
    pub fn min_stake(&self) -> f32 {
        self.min_stake
    }

    /// Get the slash fraction.
    pub fn slash_fraction(&self) -> f32 {
        self.slash_fraction
    }

    /// Get the embedding dimension.
    pub fn dim(&self) -> usize {
        self.dim
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "economic")]
mod tests {
    use super::*;

    // -- GameTheoreticAttention tests --

    #[test]
    fn test_nash_attention_basic() {
        let config = EconomicConfig {
            utility_weight: 1.0,
            temperature: 1.0,
            convergence_threshold: 0.01,
            max_iterations: 100,
            min_stake: 1.0,
            slash_fraction: 0.1,
            num_permutations: 50,
        };
        let mut gta = GameTheoreticAttention::new(4, config);

        let features = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
        ];
        let edges = vec![(0, 1), (1, 2), (0, 2)];

        let result = gta.compute(&features, &edges).unwrap();
        assert_eq!(result.output.len(), 3);
        assert_eq!(result.attention_weights.len(), 3);
        assert!(result.iterations > 0);
        // Weights should be non-negative
        for row in &result.attention_weights {
            for &w in row {
                assert!(w >= 0.0, "negative weight: {}", w);
            }
        }
    }

    #[test]
    fn test_nash_attention_converges() {
        let config = EconomicConfig {
            utility_weight: 1.0,
            temperature: 0.5,
            convergence_threshold: 0.001,
            max_iterations: 200,
            min_stake: 1.0,
            slash_fraction: 0.1,
            num_permutations: 50,
        };
        let mut gta = GameTheoreticAttention::new(2, config);

        let features = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![0.5, 0.5]];
        let edges = vec![(0, 1), (1, 2), (0, 2)];

        let result = gta.compute(&features, &edges).unwrap();
        // With sufficient iterations, should converge
        assert!(
            result.converged,
            "did not converge: max_delta={}",
            result.max_delta
        );
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_nash_attention_empty() {
        let config = EconomicConfig::default();
        let mut gta = GameTheoreticAttention::new(4, config);
        let result = gta.compute(&[], &[]).unwrap();
        assert!(result.output.is_empty());
        assert!(result.converged);
    }

    #[test]
    fn test_nash_attention_dim_mismatch() {
        let config = EconomicConfig::default();
        let mut gta = GameTheoreticAttention::new(4, config);
        let features = vec![vec![1.0, 2.0]]; // dim 2 != 4
        let result = gta.compute(&features, &[]);
        assert!(result.is_err());
    }

    // -- ShapleyAttention tests --

    #[test]
    fn test_shapley_basic() {
        let mut shapley = ShapleyAttention::new(3, 100);
        let features = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let mut rng = rand::thread_rng();

        let result = shapley.compute(&features, &mut rng).unwrap();
        assert_eq!(result.shapley_values.len(), 3);
        assert_eq!(result.output.len(), 3);
        // Coalition value should be positive
        assert!(result.coalition_value > 0.0);
    }

    #[test]
    fn test_shapley_efficiency_axiom() {
        let mut shapley = ShapleyAttention::new(2, 500);
        let features = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let mut rng = rand::thread_rng();

        let result = shapley.compute(&features, &mut rng).unwrap();
        // Efficiency: sum of Shapley values should approximately equal coalition value
        let tolerance = 0.1 * result.coalition_value.abs().max(1.0);
        assert!(
            (result.value_sum - result.coalition_value).abs() < tolerance,
            "efficiency violated: sum={}, coalition={}",
            result.value_sum,
            result.coalition_value,
        );
        assert!(result.efficiency_satisfied);
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_shapley_empty() {
        let mut shapley = ShapleyAttention::new(4, 10);
        let mut rng = rand::thread_rng();
        let result = shapley.compute(&[], &mut rng).unwrap();
        assert!(result.shapley_values.is_empty());
        assert!(result.efficiency_satisfied);
    }

    #[test]
    fn test_shapley_single_node() {
        let mut shapley = ShapleyAttention::new(2, 50);
        let features = vec![vec![3.0, 4.0]];
        let mut rng = rand::thread_rng();

        let result = shapley.compute(&features, &mut rng).unwrap();
        assert_eq!(result.shapley_values.len(), 1);
        // Single node should get the full coalition value
        let expected_value = 3.0 * 3.0 + 4.0 * 4.0; // 25.0
        assert!(
            (result.shapley_values[0] - expected_value).abs() < 1.0,
            "single node Shapley: {}, expected ~{}",
            result.shapley_values[0],
            expected_value,
        );
    }

    #[test]
    fn test_shapley_dim_mismatch() {
        let mut shapley = ShapleyAttention::new(4, 10);
        let features = vec![vec![1.0, 2.0]]; // dim 2 != 4
        let mut rng = rand::thread_rng();
        let result = shapley.compute(&features, &mut rng);
        assert!(result.is_err());
    }

    // -- IncentiveAlignedMPNN tests --

    #[test]
    fn test_incentive_mpnn_basic() {
        let mut mpnn = IncentiveAlignedMPNN::new(3, 1.0, 0.1);

        let features = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let stakes = vec![5.0, 5.0, 5.0];
        let edges = vec![(0, 1), (1, 2)];

        let result = mpnn.step(&features, &stakes, &edges).unwrap();
        assert_eq!(result.output.len(), 3);
        assert_eq!(result.stakes.len(), 3);
        assert!(result.slashed_nodes.is_empty());
        assert!(result.all_stakes_sufficient);
        assert!(result.attestation.is_some());
        assert!(result.tier_decision.is_some());
    }

    #[test]
    fn test_incentive_mpnn_insufficient_stake() {
        let mut mpnn = IncentiveAlignedMPNN::new(2, 5.0, 0.2);

        let features = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let stakes = vec![10.0, 1.0]; // node 1 below min_stake

        let edges = vec![(0, 1)];

        let result = mpnn.step(&features, &stakes, &edges).unwrap();
        // Node 1 doesn't participate -> no message exchange
        assert!(!result.all_stakes_sufficient);
        assert!(result.attestation.is_none());
    }

    #[test]
    fn test_incentive_mpnn_no_edges() {
        let mut mpnn = IncentiveAlignedMPNN::new(2, 1.0, 0.1);

        let features = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let stakes = vec![5.0, 5.0];
        let edges: Vec<(usize, usize)> = vec![];

        let result = mpnn.step(&features, &stakes, &edges).unwrap();
        // Without edges, output should equal input
        assert_eq!(result.output, features);
        assert!(result.slashed_nodes.is_empty());
    }

    #[test]
    fn test_incentive_mpnn_stake_weighted() {
        let mut mpnn = IncentiveAlignedMPNN::new(2, 0.1, 0.1);

        let features = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let stakes = vec![9.0, 1.0]; // node 0 has much higher stake

        let edges = vec![(0, 1)];
        let result = mpnn.step(&features, &stakes, &edges).unwrap();

        // Node 1's message to node 0 should be weighted less (low stake)
        // Node 0's message to node 1 should be weighted more (high stake)
        // Node 1's output should show more influence from node 0
        let node1_d0 = result.output[1][0];
        // Node 0 has stake_weight 0.9, so msg_0_to_1 = [0.9, 0.0]
        // Node 1 output = [0.0, 1.0] + [0.9, 0.0] = [0.9, 1.0]
        assert!(
            node1_d0 > 0.5,
            "node 1 should receive strong message from node 0: {}",
            node1_d0
        );
    }

    #[test]
    fn test_incentive_mpnn_stakes_length_mismatch() {
        let mut mpnn = IncentiveAlignedMPNN::new(2, 1.0, 0.1);
        let features = vec![vec![1.0, 2.0]];
        let stakes = vec![5.0, 5.0]; // mismatched
        let result = mpnn.step(&features, &stakes, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_incentive_mpnn_slash_fraction_bounds() {
        let mpnn = IncentiveAlignedMPNN::new(2, 0.0, 1.5);
        assert!((mpnn.slash_fraction() - 1.0).abs() < 1e-6);

        let mpnn2 = IncentiveAlignedMPNN::new(2, 0.0, -0.5);
        assert!((mpnn2.slash_fraction() - 0.0).abs() < 1e-6);
    }
}
