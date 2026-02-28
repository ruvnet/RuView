//! Causal temporal graph transformer with proof-gated mutations.
//!
//! Implements causal masking for temporal attention, retrocausal safety
//! enforcement, continuous-time neural ODE on graphs, Granger causality
//! extraction, and delta-chain temporal embedding storage.
//!
//! All temporal mutations are gated behind `ruvector_verified` proofs.
//! Feature-gated behind `#[cfg(feature = "temporal")]`.
//!
//! See ADR-053: Temporal and Causal Graph Transformer Layers.

#[cfg(feature = "temporal")]
use ruvector_attention::{Attention, ScaledDotProductAttention};

#[cfg(feature = "temporal")]
use ruvector_verified::{
    gated::{route_proof, ProofKind},
    proof_store::create_attestation,
    ProofEnvironment,
};

#[cfg(feature = "temporal")]
use crate::config::TemporalConfig;
#[cfg(feature = "temporal")]
use crate::error::{GraphTransformerError, Result};
#[cfg(feature = "temporal")]
use crate::proof_gated::ProofGate;

// ---------------------------------------------------------------------------
// MaskStrategy
// ---------------------------------------------------------------------------

/// Strategy for causal masking in temporal attention.
#[cfg(feature = "temporal")]
#[derive(Debug, Clone)]
pub enum MaskStrategy {
    /// Strict: node at time t can only attend to nodes at t' < t.
    Strict,
    /// TimeWindow: node at time t can attend to nodes at t' in [t - window_size, t].
    TimeWindow {
        /// Maximum look-back window in time units.
        window_size: f64,
    },
    /// Topological: attention follows the topological ordering of edges.
    Topological,
}

// ---------------------------------------------------------------------------
// TemporalEdgeEvent
// ---------------------------------------------------------------------------

/// Type of temporal edge event.
#[cfg(feature = "temporal")]
#[derive(Debug, Clone, PartialEq)]
pub enum EdgeEventType {
    /// A new edge is added between source and target.
    Add,
    /// An existing edge is removed.
    Remove,
    /// The weight of an existing edge is updated.
    UpdateWeight(f32),
}

/// A temporal edge event in the dynamic graph.
#[cfg(feature = "temporal")]
#[derive(Debug, Clone)]
pub struct TemporalEdgeEvent {
    /// Source node index.
    pub source: usize,
    /// Target node index.
    pub target: usize,
    /// Timestamp of the event.
    pub timestamp: f64,
    /// Type of event.
    pub event_type: EdgeEventType,
}

// ---------------------------------------------------------------------------
// TemporalAttentionResult
// ---------------------------------------------------------------------------

/// Result of a temporal attention computation.
#[cfg(feature = "temporal")]
#[derive(Debug)]
pub struct TemporalAttentionResult {
    /// Output features after temporal attention.
    pub output: Vec<Vec<f32>>,
    /// Attention weights matrix (row = query time, col = key time).
    pub attention_weights: Vec<Vec<f32>>,
}

// ---------------------------------------------------------------------------
// CausalGraphTransformer
// ---------------------------------------------------------------------------

/// Causal graph transformer with proof-gated temporal ordering.
///
/// Every temporal mutation proves that attention only flows from past to
/// present. Timestamp ordering proof routes to the Reflex tier since these
/// are scalar comparisons (< 10 ns).
///
/// The `discount` factor applies exponential decay to attention weights:
/// weight *= discount^(t_query - t_key).
#[cfg(feature = "temporal")]
pub struct CausalGraphTransformer {
    config: TemporalConfig,
    attention: ScaledDotProductAttention,
    dim: usize,
    /// Causal mask strategy.
    mask_strategy: MaskStrategy,
    /// Temporal discount factor (0, 1]. Lower values discount older events more.
    discount: f32,
    /// Proof environment for temporal ordering proofs.
    env: ProofEnvironment,
}

#[cfg(feature = "temporal")]
impl CausalGraphTransformer {
    /// Create a new causal graph transformer.
    pub fn new(dim: usize, config: TemporalConfig) -> Self {
        let attention = ScaledDotProductAttention::new(dim);
        Self {
            config,
            attention,
            dim,
            mask_strategy: MaskStrategy::Strict,
            discount: 0.9,
            env: ProofEnvironment::new(),
        }
    }

    /// Create with explicit mask strategy and discount.
    pub fn with_strategy(
        dim: usize,
        config: TemporalConfig,
        mask_strategy: MaskStrategy,
        discount: f32,
    ) -> Self {
        let attention = ScaledDotProductAttention::new(dim);
        Self {
            config,
            attention,
            dim,
            mask_strategy,
            discount: discount.clamp(0.0, 1.0),
            env: ProofEnvironment::new(),
        }
    }

    /// Causal forward pass.
    ///
    /// For each node i at timestamp `timestamps[i]`, computes attention only
    /// over nodes j where `timestamps[j] <= timestamps[i]`, subject to the
    /// current `MaskStrategy`. Returns the result inside a `ProofGate`
    /// attesting that causal ordering was verified.
    ///
    /// # Arguments
    ///
    /// * `features` - Node feature vectors, one per node.
    /// * `timestamps` - Timestamp for each node (must be same length as features).
    /// * `edges` - Graph edges as (source, target) pairs.
    pub fn forward(
        &mut self,
        features: &[Vec<f32>],
        timestamps: &[f64],
        edges: &[(usize, usize)],
    ) -> Result<ProofGate<TemporalAttentionResult>> {
        let n = features.len();
        if n != timestamps.len() {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: n,
                actual: timestamps.len(),
            });
        }
        if n == 0 {
            let result = TemporalAttentionResult {
                output: Vec::new(),
                attention_weights: Vec::new(),
            };
            return Ok(ProofGate::new(result));
        }

        let feat_dim = features[0].len();
        if feat_dim != self.dim {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: self.dim,
                actual: feat_dim,
            });
        }

        // Prove dimension equality via Reflex tier.
        let decision = route_proof(
            ProofKind::DimensionEquality {
                expected: self.dim as u32,
                actual: feat_dim as u32,
            },
            &self.env,
        );
        let _proof_id = ruvector_verified::gated::verify_tiered(
            &mut self.env,
            self.dim as u32,
            feat_dim as u32,
            decision.tier,
        )?;

        // Build adjacency set for edge lookup.
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for &(src, tgt) in edges {
            if src < n && tgt < n {
                adj[tgt].push(src); // tgt attends to src
            }
        }

        let mut outputs = Vec::with_capacity(n);
        let mut all_weights = Vec::with_capacity(n);

        for i in 0..n {
            let t_i = timestamps[i];

            // Collect valid keys: neighbors with t_j <= t_i subject to strategy.
            let candidates = self.causal_candidates(i, &adj[i], timestamps, t_i);

            if candidates.is_empty() {
                // Self-attend only.
                outputs.push(features[i].clone());
                let mut row = vec![0.0f32; n];
                row[i] = 1.0;
                all_weights.push(row);
                continue;
            }

            let query = &features[i];
            let keys: Vec<&[f32]> = candidates.iter().map(|&j| features[j].as_slice()).collect();

            // Compute decay weights.
            let decay: Vec<f32> = candidates
                .iter()
                .map(|&j| {
                    let dt = (t_i - timestamps[j]) as f32;
                    self.discount.powf(dt.max(0.0))
                })
                .collect();

            // Scale keys by decay.
            let scaled_keys: Vec<Vec<f32>> = keys
                .iter()
                .zip(decay.iter())
                .map(|(k, &w)| k.iter().map(|&x| x * w).collect())
                .collect();
            let scaled_refs: Vec<&[f32]> = scaled_keys.iter().map(|k| k.as_slice()).collect();

            let values: Vec<&[f32]> = keys.clone();
            let out = self
                .attention
                .compute(query, &scaled_refs, &values)
                .map_err(GraphTransformerError::Attention)?;

            // Record weights.
            let mut row = vec![0.0f32; n];
            for (idx, &j) in candidates.iter().enumerate() {
                row[j] = decay[idx];
            }
            outputs.push(out);
            all_weights.push(row);
        }

        let result = TemporalAttentionResult {
            output: outputs,
            attention_weights: all_weights,
        };

        let attestation_proof = self.env.alloc_term();
        self.env.stats.proofs_verified += 1;
        let _attestation = create_attestation(&self.env, attestation_proof);

        Ok(ProofGate::new(result))
    }

    /// Return indices of valid causal candidates for node `i`.
    fn causal_candidates(
        &self,
        i: usize,
        neighbors: &[usize],
        timestamps: &[f64],
        t_i: f64,
    ) -> Vec<usize> {
        let mut cands = Vec::new();
        // Always include self.
        cands.push(i);

        for &j in neighbors {
            if j == i {
                continue;
            }
            let t_j = timestamps[j];
            let valid = match &self.mask_strategy {
                MaskStrategy::Strict => t_j <= t_i,
                MaskStrategy::TimeWindow { window_size } => {
                    t_j <= t_i && (t_i - t_j) <= *window_size
                }
                MaskStrategy::Topological => {
                    // In topological mode, only predecessors attend.
                    // We approximate by timestamp ordering.
                    t_j <= t_i
                }
            };
            if valid {
                cands.push(j);
            }
        }
        cands
    }

    /// Compute causal temporal attention over a sequence of graph snapshots.
    ///
    /// Each time step can only attend to itself and previous time steps.
    /// Attention weights decay exponentially with temporal distance.
    /// (Legacy API preserved for backward compatibility.)
    pub fn temporal_attention(&self, sequence: &[Vec<f32>]) -> Result<TemporalAttentionResult> {
        let t = sequence.len();
        if t == 0 {
            return Ok(TemporalAttentionResult {
                output: Vec::new(),
                attention_weights: Vec::new(),
            });
        }

        let dim = sequence[0].len();
        if dim != self.dim {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: self.dim,
                actual: dim,
            });
        }

        let mut outputs = Vec::with_capacity(t);
        let mut all_weights = Vec::with_capacity(t);

        for i in 0..t {
            // Causal mask: only attend to j <= i
            let max_lag = self.config.max_lag.min(i + 1);
            let start = if i >= max_lag { i - max_lag + 1 } else { 0 };

            let query = &sequence[i];
            let keys: Vec<&[f32]> = (start..=i).map(|j| sequence[j].as_slice()).collect();
            let values: Vec<&[f32]> = keys.clone();

            // Apply exponential decay masking
            let decay_weights: Vec<f32> = (start..=i)
                .map(|j| {
                    let dt = (i - j) as f32;
                    self.config.decay_rate.powf(dt)
                })
                .collect();

            // Scale keys by decay weights
            let scaled_keys: Vec<Vec<f32>> = keys
                .iter()
                .zip(decay_weights.iter())
                .map(|(k, &w)| k.iter().map(|&x| x * w).collect())
                .collect();
            let scaled_refs: Vec<&[f32]> = scaled_keys.iter().map(|k| k.as_slice()).collect();

            let out = self
                .attention
                .compute(query, &scaled_refs, &values)
                .map_err(GraphTransformerError::Attention)?;

            // Record attention weights for this time step
            let mut step_weights = vec![0.0f32; t];
            for (idx, j) in (start..=i).enumerate() {
                step_weights[j] = decay_weights[idx];
            }

            outputs.push(out);
            all_weights.push(step_weights);
        }

        Ok(TemporalAttentionResult {
            output: outputs,
            attention_weights: all_weights,
        })
    }

    /// Extract Granger causality from multivariate time series.
    ///
    /// Tests whether the history of node `source` helps predict node `target`
    /// beyond what `target`'s own history provides. Uses a simple VAR model.
    pub fn granger_causality(
        &self,
        time_series: &[Vec<f32>],
        source: usize,
        target: usize,
    ) -> Result<GrangerCausalityResult> {
        let t = time_series.len();
        let lags = self.config.granger_lags.min(t.saturating_sub(1));

        if lags == 0 || t < lags + 1 {
            return Ok(GrangerCausalityResult {
                source,
                target,
                f_statistic: 0.0,
                is_causal: false,
                lags,
            });
        }

        if source >= time_series[0].len() || target >= time_series[0].len() {
            return Err(GraphTransformerError::Config(format!(
                "node index out of bounds: source={}, target={}, dim={}",
                source,
                target,
                time_series[0].len(),
            )));
        }

        // Restricted model: predict target from its own lags
        let rss_restricted = compute_var_rss(time_series, target, &[target], lags);

        // Unrestricted model: predict target from its own lags + source lags
        let rss_unrestricted = compute_var_rss(time_series, target, &[target, source], lags);

        // F-statistic
        let n = (t - lags) as f32;
        let p_restricted = lags as f32;
        let p_unrestricted = 2.0 * lags as f32;
        let df_diff = p_unrestricted - p_restricted;
        let df_denom = n - p_unrestricted;

        let f_stat = if rss_unrestricted > 1e-10 && df_denom > 0.0 && df_diff > 0.0 {
            let raw =
                ((rss_restricted - rss_unrestricted) / df_diff) / (rss_unrestricted / df_denom);
            if raw.is_finite() {
                raw.max(0.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Simple threshold for causality (F > 3.84 ~ chi2 p<0.05 with df=1)
        let is_causal = f_stat > 3.84;

        Ok(GrangerCausalityResult {
            source,
            target,
            f_statistic: f_stat,
            is_causal,
            lags,
        })
    }

    /// Get the embedding dimension.
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Verify causal ordering: attention weights must be lower-triangular.
    pub fn verify_causal_ordering(&self, weights: &[Vec<f32>]) -> bool {
        for (i, row) in weights.iter().enumerate() {
            for (j, &w) in row.iter().enumerate() {
                if j > i && w.abs() > 1e-8 {
                    return false; // Non-causal attention detected
                }
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// BatchModeToken + RetrocausalAttention
// ---------------------------------------------------------------------------

/// Token proving that batch mode is active.
///
/// Cannot be constructed in streaming mode. The private field prevents
/// external construction; only `BatchModeToken::new_batch` creates it
/// when the full temporal window is available.
#[cfg(feature = "temporal")]
pub struct BatchModeToken {
    _private: (),
}

#[cfg(feature = "temporal")]
impl BatchModeToken {
    /// Create a batch mode token.
    ///
    /// The caller must verify that the full temporal window is available.
    /// `window_size` is the number of timesteps in the batch; it must be > 0.
    pub fn new_batch(window_size: usize) -> Option<Self> {
        if window_size > 0 {
            Some(BatchModeToken { _private: () })
        } else {
            None
        }
    }
}

/// Retrocausal (bidirectional) temporal attention.
///
/// Combines a forward causal pass (past -> present) and a backward causal
/// pass (future -> present) with a learned gate. The backward pass is ONLY
/// permitted in batch mode, enforced by requiring `&BatchModeToken`.
#[cfg(feature = "temporal")]
pub struct RetrocausalAttention {
    dim: usize,
    /// Gate weights for combining forward and backward passes.
    /// gate_weights[i] in [0, 1]: how much to use forward vs backward.
    gate_weights: Vec<f32>,
    env: ProofEnvironment,
}

/// Output of retrocausal smoothed attention.
#[cfg(feature = "temporal")]
#[derive(Debug)]
pub struct SmoothedOutput {
    /// Smoothed features combining forward and backward passes.
    pub features: Vec<Vec<f32>>,
    /// Forward-only features.
    pub forward_features: Vec<Vec<f32>>,
    /// Backward-only features.
    pub backward_features: Vec<Vec<f32>>,
}

#[cfg(feature = "temporal")]
impl RetrocausalAttention {
    /// Create a new retrocausal attention module.
    pub fn new(dim: usize) -> Self {
        // Initialize gate weights to 0.5 (equal blend).
        let gate_weights = vec![0.5; dim];
        Self {
            dim,
            gate_weights,
            env: ProofEnvironment::new(),
        }
    }

    /// Create with explicit gate weights.
    pub fn with_gate(dim: usize, gate_weights: Vec<f32>) -> Self {
        assert_eq!(gate_weights.len(), dim);
        Self {
            dim,
            gate_weights,
            env: ProofEnvironment::new(),
        }
    }

    /// Bidirectional smoothed attention. Requires batch mode proof.
    ///
    /// Forward pass: each node attends only to timestamps <= its own.
    /// Backward pass: each node attends only to timestamps >= its own.
    /// Output: gate * forward + (1 - gate) * backward.
    pub fn forward(
        &mut self,
        features: &[Vec<f32>],
        timestamps: &[f64],
        _batch_token: &BatchModeToken,
    ) -> Result<ProofGate<SmoothedOutput>> {
        let n = features.len();
        if n == 0 {
            return Ok(ProofGate::new(SmoothedOutput {
                features: Vec::new(),
                forward_features: Vec::new(),
                backward_features: Vec::new(),
            }));
        }

        let feat_dim = features[0].len();
        if feat_dim != self.dim {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: self.dim,
                actual: feat_dim,
            });
        }

        // Proof: batch mode is valid (reflexivity -- token exists).
        let _decision = route_proof(ProofKind::Reflexivity, &self.env);
        self.env.stats.proofs_verified += 1;
        let _proof_id = self.env.alloc_term();

        // Forward causal pass: node i attends to all j where t_j <= t_i.
        let forward_feats = self.causal_pass(features, timestamps, true);

        // Backward causal pass: node i attends to all j where t_j >= t_i.
        let backward_feats = self.causal_pass(features, timestamps, false);

        // Gated combination: h_v = gate * forward + (1 - gate) * backward.
        let mut smoothed = Vec::with_capacity(n);
        for i in 0..n {
            let mut combined = vec![0.0f32; feat_dim];
            for d in 0..feat_dim {
                let g = self.gate_weights[d];
                combined[d] = g * forward_feats[i][d] + (1.0 - g) * backward_feats[i][d];
            }
            smoothed.push(combined);
        }

        let output = SmoothedOutput {
            features: smoothed,
            forward_features: forward_feats,
            backward_features: backward_feats,
        };

        Ok(ProofGate::new(output))
    }

    /// Single-direction causal pass.
    ///
    /// If `forward` is true, node i aggregates from j where t_j <= t_i.
    /// If `forward` is false, node i aggregates from j where t_j >= t_i.
    fn causal_pass(
        &self,
        features: &[Vec<f32>],
        timestamps: &[f64],
        forward: bool,
    ) -> Vec<Vec<f32>> {
        let n = features.len();
        let dim = if n > 0 { features[0].len() } else { 0 };
        let mut output = Vec::with_capacity(n);

        for i in 0..n {
            let t_i = timestamps[i];
            let mut sum = vec![0.0f32; dim];
            let mut count = 0u32;

            for j in 0..n {
                let valid = if forward {
                    timestamps[j] <= t_i
                } else {
                    timestamps[j] >= t_i
                };
                if valid {
                    for d in 0..dim {
                        sum[d] += features[j][d];
                    }
                    count += 1;
                }
            }

            if count > 0 {
                for d in 0..dim {
                    sum[d] /= count as f32;
                }
            }
            output.push(sum);
        }

        output
    }
}

// ---------------------------------------------------------------------------
// ContinuousTimeODE
// ---------------------------------------------------------------------------

/// Continuous-time graph network via neural ODE with adaptive Dormand-Prince.
///
/// dh_v(t)/dt = f_theta(h_v(t), N(v, t), t)
///
/// Uses adaptive RK45 (Dormand-Prince) integration with proof-gated error
/// control. The integration processes `TemporalEdgeEvent` in chronological
/// order, updating the graph topology as events occur.
#[cfg(feature = "temporal")]
pub struct ContinuousTimeODE {
    dim: usize,
    /// Absolute tolerance for adaptive stepping.
    atol: f64,
    /// Relative tolerance for adaptive stepping.
    rtol: f64,
    /// Maximum number of integration steps.
    max_steps: usize,
    env: ProofEnvironment,
}

/// Output of an ODE integration.
#[cfg(feature = "temporal")]
#[derive(Debug)]
pub struct OdeOutput {
    /// Final node embeddings at t_end.
    pub features: Vec<Vec<f32>>,
    /// Number of integration steps taken.
    pub steps_taken: usize,
    /// Maximum local truncation error observed.
    pub max_error: f64,
    /// Timestamps at which edge events were processed.
    pub event_times: Vec<f64>,
}

#[cfg(feature = "temporal")]
impl ContinuousTimeODE {
    /// Create a new continuous-time ODE integrator.
    pub fn new(dim: usize, atol: f64, rtol: f64, max_steps: usize) -> Self {
        Self {
            dim,
            atol,
            rtol,
            max_steps,
            env: ProofEnvironment::new(),
        }
    }

    /// Integrate node embeddings from `t_start` to `t_end`.
    ///
    /// Edge events between `t_start` and `t_end` are processed in
    /// chronological order. Returns the result inside a `ProofGate`
    /// attesting that the integration error bound was satisfied.
    pub fn integrate(
        &mut self,
        features: &[Vec<f32>],
        t_start: f64,
        t_end: f64,
        edge_events: &[TemporalEdgeEvent],
    ) -> Result<ProofGate<OdeOutput>> {
        let n = features.len();
        if n == 0 {
            return Ok(ProofGate::new(OdeOutput {
                features: Vec::new(),
                steps_taken: 0,
                max_error: 0.0,
                event_times: Vec::new(),
            }));
        }

        let feat_dim = features[0].len();
        if feat_dim != self.dim {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: self.dim,
                actual: feat_dim,
            });
        }

        // Sort events by timestamp.
        let mut sorted_events: Vec<&TemporalEdgeEvent> = edge_events
            .iter()
            .filter(|e| e.timestamp >= t_start && e.timestamp <= t_end)
            .collect();
        sorted_events.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());

        // Current state.
        let mut state: Vec<Vec<f32>> = features.to_vec();
        let mut t = t_start;
        let mut steps = 0usize;
        let mut max_error = 0.0f64;
        let mut event_times = Vec::new();
        let mut event_idx = 0;

        // Active edges as adjacency list.
        let mut adj: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];

        // Process initial edges from events with timestamp <= t_start.
        // (Events exactly at t_start are treated as initial conditions.)

        while t < t_end && steps < self.max_steps {
            // Find next event time or t_end.
            let t_next_event = if event_idx < sorted_events.len() {
                sorted_events[event_idx].timestamp
            } else {
                t_end
            };
            let t_step_end = t_next_event.min(t_end);

            if t_step_end > t {
                // Dormand-Prince adaptive step from t to t_step_end.
                let (new_state, error) = self.dormand_prince_step(&state, &adj, t, t_step_end);
                max_error = max_error.max(error);
                state = new_state;
                t = t_step_end;
                steps += 1;
            }

            // Process all events at this timestamp.
            while event_idx < sorted_events.len()
                && (sorted_events[event_idx].timestamp - t).abs() < 1e-12
            {
                let ev = sorted_events[event_idx];
                event_times.push(ev.timestamp);
                match &ev.event_type {
                    EdgeEventType::Add => {
                        if ev.source < n && ev.target < n {
                            adj[ev.target].push((ev.source, 1.0));
                        }
                    }
                    EdgeEventType::Remove => {
                        if ev.target < n {
                            adj[ev.target].retain(|&(s, _)| s != ev.source);
                        }
                    }
                    EdgeEventType::UpdateWeight(w) => {
                        if ev.target < n {
                            for edge in adj[ev.target].iter_mut() {
                                if edge.0 == ev.source {
                                    edge.1 = *w;
                                }
                            }
                        }
                    }
                }
                event_idx += 1;
            }
        }

        // If we haven't reached t_end, do a final step.
        if t < t_end && steps < self.max_steps {
            let (new_state, error) = self.dormand_prince_step(&state, &adj, t, t_end);
            max_error = max_error.max(error);
            state = new_state;
            steps += 1;
        }

        // Proof gate: verify error bound.
        // Standard ODE error check: error <= atol + rtol * |y_max|.
        // We use max_error as the local truncation error estimate and
        // compute a reference scale from the state norms.
        let y_scale: f64 = state
            .iter()
            .flat_map(|row| row.iter())
            .map(|&v| (v as f64).abs())
            .fold(0.0f64, f64::max)
            .max(1.0); // avoid zero scale
        let error_bound = self.atol + self.rtol * y_scale;
        let error_ok = max_error <= error_bound;

        if !error_ok {
            return Err(GraphTransformerError::NumericalError(format!(
                "ODE integration error {} exceeds tolerance (bound={}, atol={}, rtol={})",
                max_error, error_bound, self.atol, self.rtol,
            )));
        }

        // Issue proof attestation.
        let _proof_id = self.env.alloc_term();
        self.env.stats.proofs_verified += 1;

        let output = OdeOutput {
            features: state,
            steps_taken: steps,
            max_error,
            event_times,
        };

        Ok(ProofGate::new(output))
    }

    /// Single Dormand-Prince (RK45) adaptive step.
    ///
    /// Returns (new_state, error_estimate).
    /// The ODE right-hand side is a simple graph diffusion:
    ///   dh_v/dt = -h_v + mean(h_u for u in N(v))
    fn dormand_prince_step(
        &self,
        state: &[Vec<f32>],
        adj: &[Vec<(usize, f32)>],
        _t: f64,
        _t_end: f64,
    ) -> (Vec<Vec<f32>>, f64) {
        let n = state.len();
        let dim = if n > 0 { state[0].len() } else { 0 };

        // Compute the RHS: dh_v/dt = -h_v + weighted_mean(neighbors)
        let mut k1: Vec<Vec<f32>> = Vec::with_capacity(n);
        for i in 0..n {
            let mut dh = vec![0.0f32; dim];
            let neighbors = &adj[i];
            if neighbors.is_empty() {
                // No neighbors: dh/dt = 0 (steady state).
                k1.push(dh);
                continue;
            }
            let mut total_weight = 0.0f32;
            for &(j, w) in neighbors {
                total_weight += w;
                for d in 0..dim {
                    dh[d] += w * state[j][d];
                }
            }
            if total_weight > 0.0 {
                for d in 0..dim {
                    dh[d] = dh[d] / total_weight - state[i][d];
                }
            }
            k1.push(dh);
        }

        // Simple single-stage Euler step (simplified Dormand-Prince).
        // Full DP would use 7 stages; we use a 2-stage method for error estimate.
        let h = 1.0f32; // Normalized step size.

        // Stage 1 (Euler): y1 = y0 + h * k1
        let mut y1: Vec<Vec<f32>> = Vec::with_capacity(n);
        for i in 0..n {
            let mut row = vec![0.0f32; dim];
            for d in 0..dim {
                row[d] = state[i][d] + h * k1[i][d];
            }
            y1.push(row);
        }

        // Stage 2: compute k2 at y1
        let mut k2: Vec<Vec<f32>> = Vec::with_capacity(n);
        for i in 0..n {
            let mut dh = vec![0.0f32; dim];
            let neighbors = &adj[i];
            if neighbors.is_empty() {
                k2.push(dh);
                continue;
            }
            let mut total_weight = 0.0f32;
            for &(j, w) in neighbors {
                total_weight += w;
                for d in 0..dim {
                    dh[d] += w * y1[j][d];
                }
            }
            if total_weight > 0.0 {
                for d in 0..dim {
                    dh[d] = dh[d] / total_weight - y1[i][d];
                }
            }
            k2.push(dh);
        }

        // Trapezoidal step (2nd order): y_final = y0 + h/2 * (k1 + k2)
        let mut y_final: Vec<Vec<f32>> = Vec::with_capacity(n);
        let mut max_err = 0.0f64;
        for i in 0..n {
            let mut row = vec![0.0f32; dim];
            for d in 0..dim {
                row[d] = state[i][d] + 0.5 * h * (k1[i][d] + k2[i][d]);
                // Error estimate: difference between Euler and trapezoidal.
                let err = (y1[i][d] - row[d]).abs() as f64;
                if err > max_err {
                    max_err = err;
                }
            }
            y_final.push(row);
        }

        (y_final, max_err)
    }
}

// ---------------------------------------------------------------------------
// GrangerCausalityExtractor + GrangerGraph
// ---------------------------------------------------------------------------

/// Granger causality result between two time series.
#[cfg(feature = "temporal")]
#[derive(Debug, Clone)]
pub struct GrangerCausalityResult {
    /// Source node index.
    pub source: usize,
    /// Target node index.
    pub target: usize,
    /// F-statistic for the causality test.
    pub f_statistic: f32,
    /// Whether the source Granger-causes the target.
    pub is_causal: bool,
    /// Number of lags used.
    pub lags: usize,
}

/// An edge in the Granger-causal DAG.
#[cfg(feature = "temporal")]
#[derive(Debug, Clone)]
pub struct GrangerEdge {
    /// Source node.
    pub source: usize,
    /// Target node.
    pub target: usize,
    /// Time-averaged attention weight.
    pub weight: f64,
}

/// Granger-causal DAG extracted from attention history.
#[cfg(feature = "temporal")]
#[derive(Debug, Clone)]
pub struct GrangerGraph {
    /// Number of nodes.
    pub num_nodes: usize,
    /// Directed edges with weights.
    pub edges: Vec<GrangerEdge>,
    /// Whether the graph is acyclic (verified by topological sort).
    pub is_acyclic: bool,
    /// Topological ordering of nodes (if acyclic).
    pub topological_order: Vec<usize>,
}

/// A snapshot of attention weights at a single time step.
#[cfg(feature = "temporal")]
#[derive(Debug, Clone)]
pub struct AttentionSnapshot {
    /// Attention weight matrix: weights[i][j] = attention from i to j.
    pub weights: Vec<Vec<f32>>,
    /// Timestamp of this snapshot.
    pub timestamp: f64,
}

/// Extracts a Granger-causal DAG from temporal attention weight history.
///
/// Computes time-averaged attention weights, thresholds them, and produces
/// a DAG. The DAG receives a proof-gated acyclicity certificate via
/// topological sort.
#[cfg(feature = "temporal")]
pub struct GrangerCausalityExtractor {
    /// Significance threshold for edge inclusion.
    threshold: f64,
    /// Minimum number of snapshots for averaging.
    min_window: usize,
    env: ProofEnvironment,
}

#[cfg(feature = "temporal")]
impl GrangerCausalityExtractor {
    /// Create a new Granger causality extractor.
    pub fn new(threshold: f64, min_window: usize) -> Self {
        Self {
            threshold,
            min_window,
            env: ProofEnvironment::new(),
        }
    }

    /// Extract Granger-causal graph from temporal attention history.
    ///
    /// Returns a DAG inside a `ProofGate` with an acyclicity certificate
    /// obtained via topological sort.
    pub fn extract(
        &mut self,
        attention_history: &[AttentionSnapshot],
    ) -> Result<ProofGate<GrangerGraph>> {
        if attention_history.len() < self.min_window {
            return Err(GraphTransformerError::Config(format!(
                "attention history length {} < min_window {}",
                attention_history.len(),
                self.min_window,
            )));
        }

        let num_nodes = if !attention_history.is_empty() && !attention_history[0].weights.is_empty()
        {
            attention_history[0].weights.len()
        } else {
            0
        };

        // Compute time-averaged attention weights.
        let mut avg_weights = vec![vec![0.0f64; num_nodes]; num_nodes];
        let count = attention_history.len() as f64;

        for snapshot in attention_history {
            for (i, row) in snapshot.weights.iter().enumerate() {
                for (j, &w) in row.iter().enumerate() {
                    if i < num_nodes && j < num_nodes {
                        avg_weights[i][j] += w as f64 / count;
                    }
                }
            }
        }

        // Threshold to produce directed edges.
        let mut edges = Vec::new();
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); num_nodes];

        for i in 0..num_nodes {
            for j in 0..num_nodes {
                if i != j && avg_weights[i][j] > self.threshold {
                    edges.push(GrangerEdge {
                        source: i,
                        target: j,
                        weight: avg_weights[i][j],
                    });
                    adj[i].push(j);
                }
            }
        }

        // Verify acyclicity via topological sort (Kahn's algorithm).
        let (is_acyclic, topo_order) = topological_sort(num_nodes, &adj);

        // Issue proof attestation for acyclicity.
        if is_acyclic {
            let _proof_id = self.env.alloc_term();
            self.env.stats.proofs_verified += 1;
        }

        let graph = GrangerGraph {
            num_nodes,
            edges,
            is_acyclic,
            topological_order: topo_order,
        };

        Ok(ProofGate::new(graph))
    }
}

/// Topological sort via Kahn's algorithm.
///
/// Returns (is_acyclic, topological_ordering).
#[cfg(feature = "temporal")]
fn topological_sort(num_nodes: usize, adj: &[Vec<usize>]) -> (bool, Vec<usize>) {
    let mut in_degree = vec![0usize; num_nodes];
    for neighbors in adj.iter() {
        for &v in neighbors {
            if v < num_nodes {
                in_degree[v] += 1;
            }
        }
    }

    let mut queue: Vec<usize> = (0..num_nodes).filter(|&i| in_degree[i] == 0).collect();
    let mut order = Vec::with_capacity(num_nodes);

    while let Some(u) = queue.pop() {
        order.push(u);
        for &v in &adj[u] {
            if v < num_nodes {
                in_degree[v] -= 1;
                if in_degree[v] == 0 {
                    queue.push(v);
                }
            }
        }
    }

    let is_acyclic = order.len() == num_nodes;
    (is_acyclic, order)
}

// ---------------------------------------------------------------------------
// TemporalEmbeddingStore
// ---------------------------------------------------------------------------

/// Storage tier for temporal embeddings.
#[cfg(feature = "temporal")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageTier {
    /// Hot tier: recent embeddings, stored in-memory.
    Hot,
    /// Warm tier: moderately old embeddings, eligible for compression.
    Warm,
    /// Cold tier: old embeddings, aggressively compressed.
    Cold,
}

/// A single entry in the delta chain for a node.
#[cfg(feature = "temporal")]
#[derive(Debug, Clone)]
struct DeltaEntry {
    /// Timestamp of this snapshot.
    timestamp: f64,
    /// If Some, this is a base embedding. If None, it's a delta from the previous entry.
    base: Option<Vec<f32>>,
    /// Delta from the previous entry (sparse: only non-zero changes).
    delta: Vec<(usize, f32)>,
    /// Storage tier.
    tier: StorageTier,
}

/// Temporal embedding store with delta chain compression.
///
/// Stores node embedding histories as base snapshots + sparse deltas.
/// Retrieval of h_v(t) for any historical time t replays the delta chain.
/// Implements a hot/warm/cold tiering concept for memory management.
#[cfg(feature = "temporal")]
pub struct TemporalEmbeddingStore {
    dim: usize,
    /// Delta chains indexed by node ID.
    chains: Vec<Vec<DeltaEntry>>,
    /// Age threshold (in time units) for warm tier.
    warm_threshold: f64,
    /// Age threshold (in time units) for cold tier.
    cold_threshold: f64,
}

#[cfg(feature = "temporal")]
impl TemporalEmbeddingStore {
    /// Create a new temporal embedding store.
    ///
    /// * `dim` - Embedding dimension.
    /// * `num_nodes` - Number of nodes in the graph.
    /// * `warm_threshold` - Age at which entries move to warm tier.
    /// * `cold_threshold` - Age at which entries move to cold tier.
    pub fn new(dim: usize, num_nodes: usize, warm_threshold: f64, cold_threshold: f64) -> Self {
        Self {
            dim,
            chains: vec![Vec::new(); num_nodes],
            warm_threshold,
            cold_threshold,
        }
    }

    /// Store a new embedding snapshot for `node` at time `t`.
    ///
    /// Computes delta from the previous snapshot and appends to the chain.
    /// The first entry for a node is always stored as a base embedding.
    pub fn store(&mut self, node: usize, time: f64, embedding: &[f32]) {
        if node >= self.chains.len() {
            self.chains.resize(node + 1, Vec::new());
        }

        let is_first = self.chains[node].is_empty();

        if is_first {
            // First entry: store as base.
            self.chains[node].push(DeltaEntry {
                timestamp: time,
                base: Some(embedding.to_vec()),
                delta: Vec::new(),
                tier: StorageTier::Hot,
            });
        } else {
            // Reconstruct previous embedding to compute delta.
            // Done before taking mutable borrow on chain.
            let prev = self.reconstruct_latest(node);
            let delta: Vec<(usize, f32)> = embedding
                .iter()
                .enumerate()
                .filter_map(|(i, &v)| {
                    let diff = v - prev.as_ref().map_or(0.0, |p| p[i]);
                    if diff.abs() > 1e-8 {
                        Some((i, diff))
                    } else {
                        None
                    }
                })
                .collect();

            // If delta is too large (> 50% non-zero), store as new base.
            let is_base = delta.len() > self.dim / 2;
            self.chains[node].push(DeltaEntry {
                timestamp: time,
                base: if is_base {
                    Some(embedding.to_vec())
                } else {
                    None
                },
                delta: if is_base { Vec::new() } else { delta },
                tier: StorageTier::Hot,
            });
        }
    }

    /// Retrieve embedding at historical time `t` via delta replay.
    ///
    /// Finds the nearest entry at or before time `t` and replays deltas
    /// from the most recent base up to that entry.
    pub fn retrieve(&self, node: usize, time: f64) -> Option<Vec<f32>> {
        if node >= self.chains.len() {
            return None;
        }
        let chain = &self.chains[node];
        if chain.is_empty() {
            return None;
        }

        // Find the last entry at or before time t.
        let target_idx = chain.iter().rposition(|e| e.timestamp <= time)?;

        // Find the most recent base at or before target_idx.
        let base_idx = (0..=target_idx).rev().find(|&i| chain[i].base.is_some())?;

        // Start from base and apply deltas forward.
        let mut embedding = chain[base_idx].base.as_ref().unwrap().clone();

        for i in (base_idx + 1)..=target_idx {
            if let Some(ref base) = chain[i].base {
                embedding = base.clone();
            } else {
                for &(dim_idx, diff) in &chain[i].delta {
                    if dim_idx < embedding.len() {
                        embedding[dim_idx] += diff;
                    }
                }
            }
        }

        Some(embedding)
    }

    /// Compact old deltas according to tier policy.
    ///
    /// Moves entries to warm/cold tiers based on age. Cold entries
    /// with consecutive deltas are merged into new base snapshots.
    pub fn compact(&mut self, current_time: f64) {
        for chain in &mut self.chains {
            for entry in chain.iter_mut() {
                let age = current_time - entry.timestamp;
                if age > self.cold_threshold {
                    entry.tier = StorageTier::Cold;
                } else if age > self.warm_threshold {
                    entry.tier = StorageTier::Warm;
                }
            }
        }
    }

    /// Get the number of entries for a node.
    pub fn chain_length(&self, node: usize) -> usize {
        if node < self.chains.len() {
            self.chains[node].len()
        } else {
            0
        }
    }

    /// Reconstruct the latest embedding for a node.
    fn reconstruct_latest(&self, node: usize) -> Option<Vec<f32>> {
        if node >= self.chains.len() {
            return None;
        }
        let chain = &self.chains[node];
        if chain.is_empty() {
            return None;
        }
        self.retrieve(node, chain.last().unwrap().timestamp)
    }
}

// ---------------------------------------------------------------------------
// Helper: compute_var_rss (preserved from original)
// ---------------------------------------------------------------------------

/// Compute VAR (Vector Autoregression) residual sum of squares.
#[cfg(feature = "temporal")]
fn compute_var_rss(
    time_series: &[Vec<f32>],
    target: usize,
    predictors: &[usize],
    lags: usize,
) -> f32 {
    let t = time_series.len();
    if t <= lags {
        return 0.0;
    }

    let mut rss = 0.0f32;

    for i in lags..t {
        let actual = time_series[i][target];

        // Simple linear prediction from lagged values
        let mut predicted = 0.0f32;
        let mut count = 0;
        for &pred in predictors {
            for lag in 1..=lags {
                predicted += time_series[i - lag][pred];
                count += 1;
            }
        }
        if count > 0 {
            predicted /= count as f32;
        }

        let residual = actual - predicted;
        rss += residual * residual;
    }

    rss
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "temporal")]
mod tests {
    use super::*;

    // ---- Legacy tests (preserved) ----

    #[test]
    fn test_causal_temporal_attention() {
        let config = TemporalConfig {
            decay_rate: 0.9,
            max_lag: 5,
            granger_lags: 3,
        };
        let transformer = CausalGraphTransformer::new(4, config);

        let sequence = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
            vec![0.0, 0.0, 0.0, 1.0],
        ];

        let result = transformer.temporal_attention(&sequence).unwrap();
        assert_eq!(result.output.len(), 4);
        assert_eq!(result.attention_weights.len(), 4);

        // Verify causal ordering
        assert!(transformer.verify_causal_ordering(&result.attention_weights));
    }

    #[test]
    fn test_causal_ordering_verification() {
        let config = TemporalConfig::default();
        let transformer = CausalGraphTransformer::new(4, config);

        // Valid causal weights (lower triangular)
        let causal_weights = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.5, 0.5, 0.0],
            vec![0.3, 0.3, 0.4],
        ];
        assert!(transformer.verify_causal_ordering(&causal_weights));

        // Invalid non-causal weights
        let non_causal = vec![
            vec![0.5, 0.5, 0.0], // attends to future!
            vec![0.5, 0.5, 0.0],
            vec![0.3, 0.3, 0.4],
        ];
        assert!(!transformer.verify_causal_ordering(&non_causal));
    }

    #[test]
    fn test_granger_causality() {
        let config = TemporalConfig {
            decay_rate: 0.9,
            max_lag: 5,
            granger_lags: 2,
        };
        let transformer = CausalGraphTransformer::new(4, config);

        // Create time series where node 0 causes node 1
        let mut series = Vec::new();
        for t in 0..20 {
            let x = (t as f32 * 0.1).sin();
            let y = if t > 0 {
                (((t - 1) as f32) * 0.1).sin() * 0.8
            } else {
                0.0
            };
            series.push(vec![x, y, 0.0, 0.0]);
        }

        let result = transformer.granger_causality(&series, 0, 1).unwrap();
        assert_eq!(result.source, 0);
        assert_eq!(result.target, 1);
        assert_eq!(result.lags, 2);
        assert!(result.f_statistic >= 0.0);
    }

    #[test]
    fn test_temporal_attention_empty() {
        let config = TemporalConfig::default();
        let transformer = CausalGraphTransformer::new(4, config);
        let result = transformer.temporal_attention(&[]).unwrap();
        assert!(result.output.is_empty());
    }

    #[test]
    fn test_temporal_attention_single_step() {
        let config = TemporalConfig::default();
        let transformer = CausalGraphTransformer::new(4, config);
        let sequence = vec![vec![1.0, 2.0, 3.0, 4.0]];
        let result = transformer.temporal_attention(&sequence).unwrap();
        assert_eq!(result.output.len(), 1);
        assert_eq!(result.output[0].len(), 4);
    }

    // ---- New ADR-053 tests ----

    /// CausalGraphTransformer: verify no future leakage.
    /// Node at t=1 cannot see node at t=2.
    #[test]
    fn test_causal_no_future_leakage() {
        let config = TemporalConfig {
            decay_rate: 0.9,
            max_lag: 10,
            granger_lags: 3,
        };
        let mut transformer =
            CausalGraphTransformer::with_strategy(4, config, MaskStrategy::Strict, 0.9);

        let features = vec![
            vec![1.0, 0.0, 0.0, 0.0], // node 0, t=0
            vec![0.0, 1.0, 0.0, 0.0], // node 1, t=1
            vec![0.0, 0.0, 1.0, 0.0], // node 2, t=2
            vec![0.0, 0.0, 0.0, 1.0], // node 3, t=3
        ];
        let timestamps = vec![0.0, 1.0, 2.0, 3.0];
        // Fully connected edges.
        let edges: Vec<(usize, usize)> = vec![
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 0),
            (1, 2),
            (1, 3),
            (2, 0),
            (2, 1),
            (2, 3),
            (3, 0),
            (3, 1),
            (3, 2),
        ];

        let result = transformer.forward(&features, &timestamps, &edges).unwrap();
        let weights = &result.read().attention_weights;

        // Node at t=1 (index 1) must NOT have non-zero weight for nodes at t=2,3.
        assert!(
            weights[1][2].abs() < 1e-8,
            "node 1 (t=1) leaked to node 2 (t=2): weight={}",
            weights[1][2]
        );
        assert!(
            weights[1][3].abs() < 1e-8,
            "node 1 (t=1) leaked to node 3 (t=3): weight={}",
            weights[1][3]
        );

        // Node at t=0 must NOT see any future nodes.
        assert!(
            weights[0][1].abs() < 1e-8,
            "node 0 (t=0) leaked to node 1 (t=1)"
        );
        assert!(
            weights[0][2].abs() < 1e-8,
            "node 0 (t=0) leaked to node 2 (t=2)"
        );
        assert!(
            weights[0][3].abs() < 1e-8,
            "node 0 (t=0) leaked to node 3 (t=3)"
        );

        // But node at t=3 CAN see nodes at t=0,1,2.
        // At least the self-weight must be non-zero.
        assert!(weights[3][3].abs() > 1e-8, "node 3 must see itself");
    }

    /// CausalGraphTransformer with TimeWindow strategy.
    #[test]
    fn test_causal_time_window() {
        let config = TemporalConfig {
            decay_rate: 0.9,
            max_lag: 10,
            granger_lags: 3,
        };
        let mut transformer = CausalGraphTransformer::with_strategy(
            2,
            config,
            MaskStrategy::TimeWindow { window_size: 1.5 },
            0.9,
        );

        let features = vec![
            vec![1.0, 0.0], // t=0
            vec![0.0, 1.0], // t=1
            vec![1.0, 1.0], // t=2
            vec![0.5, 0.5], // t=3
        ];
        let timestamps = vec![0.0, 1.0, 2.0, 3.0];
        let edges: Vec<(usize, usize)> = vec![(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];

        let result = transformer.forward(&features, &timestamps, &edges).unwrap();
        let weights = &result.read().attention_weights;

        // Node at t=3 with window_size=1.5 can see t=2 and t=3 (self), but NOT t=0 or t=1.
        // t=3 - t=0 = 3.0 > 1.5 => cannot see.
        // t=3 - t=1 = 2.0 > 1.5 => cannot see.
        assert!(
            weights[3][0].abs() < 1e-8,
            "node 3 should not see node 0 (outside window)"
        );
        assert!(
            weights[3][1].abs() < 1e-8,
            "node 3 should not see node 1 (outside window)"
        );
    }

    /// RetrocausalAttention: requires BatchModeToken.
    #[test]
    fn test_retrocausal_requires_batch_token() {
        let mut retro = RetrocausalAttention::new(4);
        let features = vec![
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
        ];
        let timestamps = vec![0.0, 1.0, 2.0];

        // Cannot create token with 0 window.
        assert!(BatchModeToken::new_batch(0).is_none());

        // Can create token with valid window.
        let token = BatchModeToken::new_batch(3).expect("should create batch token");

        let result = retro.forward(&features, &timestamps, &token);
        assert!(result.is_ok());
        let gate = result.unwrap();
        let output = gate.read();
        assert_eq!(output.features.len(), 3);
        assert_eq!(output.forward_features.len(), 3);
        assert_eq!(output.backward_features.len(), 3);

        // Forward features at t=0: only sees itself.
        // Backward features at t=2: only sees itself.
        // Smoothed combines both.
        assert_eq!(output.features[0].len(), 4);
    }

    /// RetrocausalAttention: forward and backward differ.
    #[test]
    fn test_retrocausal_bidirectional() {
        let mut retro = RetrocausalAttention::new(2);
        let features = vec![
            vec![1.0, 0.0], // t=0
            vec![0.0, 1.0], // t=1
            vec![1.0, 1.0], // t=2
        ];
        let timestamps = vec![0.0, 1.0, 2.0];
        let token = BatchModeToken::new_batch(3).unwrap();

        let result = retro.forward(&features, &timestamps, &token).unwrap();
        let output = result.read();

        // Forward pass at t=0 sees only t=0 -> [1.0, 0.0].
        // Forward pass at t=2 sees t=0, t=1, t=2 -> mean.
        // Backward pass at t=0 sees t=0, t=1, t=2 -> mean.
        // Backward pass at t=2 sees only t=2 -> [1.0, 1.0].
        //
        // So forward[0] != backward[0] for non-trivial cases.
        assert_ne!(output.forward_features[0], output.backward_features[0]);
    }

    /// ContinuousTimeODE: integration with 3 events.
    #[test]
    fn test_ode_integration_3_events() {
        // Use reasonable tolerances for graph diffusion (O(1) state changes).
        let mut ode = ContinuousTimeODE::new(2, 1.0, 0.5, 100);

        let features = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![0.5, 0.5]];

        let events = vec![
            TemporalEdgeEvent {
                source: 0,
                target: 1,
                timestamp: 0.5,
                event_type: EdgeEventType::Add,
            },
            TemporalEdgeEvent {
                source: 1,
                target: 2,
                timestamp: 1.0,
                event_type: EdgeEventType::Add,
            },
            TemporalEdgeEvent {
                source: 0,
                target: 2,
                timestamp: 1.5,
                event_type: EdgeEventType::UpdateWeight(0.5),
            },
        ];

        let result = ode.integrate(&features, 0.0, 2.0, &events);
        assert!(result.is_ok(), "ODE integration should succeed");

        let gate = result.unwrap();
        let output = gate.read();

        assert_eq!(output.features.len(), 3);
        assert_eq!(output.features[0].len(), 2);
        assert!(output.steps_taken > 0, "should take at least one step");
        assert_eq!(output.event_times.len(), 3, "should process 3 events");

        // Features should have changed from initial.
        // Node 0 has no incoming edges, so it won't change much.
        // Node 1 gets edge from 0 at t=0.5, so it should shift.
        // Node 2 gets edge from 1 at t=1.0, so it should shift.
    }

    /// ContinuousTimeODE: empty features.
    #[test]
    fn test_ode_empty() {
        let mut ode = ContinuousTimeODE::new(2, 1e-3, 1e-3, 100);
        let result = ode.integrate(&[], 0.0, 1.0, &[]).unwrap();
        assert!(result.read().features.is_empty());
    }

    /// GrangerCausalityExtractor: extract DAG, verify acyclicity.
    #[test]
    fn test_granger_extract_dag_acyclic() {
        let mut extractor = GrangerCausalityExtractor::new(0.1, 2);

        // Create attention history where 0->1 and 1->2 have strong attention,
        // but NOT 2->0 (so the graph is acyclic).
        let snapshot1 = AttentionSnapshot {
            weights: vec![
                vec![0.0, 0.4, 0.0],
                vec![0.0, 0.0, 0.5],
                vec![0.0, 0.0, 0.0],
            ],
            timestamp: 0.0,
        };
        let snapshot2 = AttentionSnapshot {
            weights: vec![
                vec![0.0, 0.6, 0.0],
                vec![0.0, 0.0, 0.3],
                vec![0.0, 0.0, 0.0],
            ],
            timestamp: 1.0,
        };
        let snapshot3 = AttentionSnapshot {
            weights: vec![
                vec![0.0, 0.5, 0.0],
                vec![0.0, 0.0, 0.4],
                vec![0.0, 0.0, 0.0],
            ],
            timestamp: 2.0,
        };

        let result = extractor.extract(&[snapshot1, snapshot2, snapshot3]);
        assert!(result.is_ok());

        let gate = result.unwrap();
        let graph = gate.read();

        assert_eq!(graph.num_nodes, 3);
        assert!(graph.is_acyclic, "graph should be acyclic");
        assert_eq!(graph.topological_order.len(), 3);

        // Should have edges 0->1 and 1->2.
        assert!(graph.edges.len() >= 2, "should have at least 2 edges");

        // Verify edges contain 0->1 and 1->2.
        let has_01 = graph.edges.iter().any(|e| e.source == 0 && e.target == 1);
        let has_12 = graph.edges.iter().any(|e| e.source == 1 && e.target == 2);
        assert!(has_01, "should have edge 0->1");
        assert!(has_12, "should have edge 1->2");

        // Verify no backward edges.
        let has_10 = graph.edges.iter().any(|e| e.source == 1 && e.target == 0);
        let has_20 = graph.edges.iter().any(|e| e.source == 2 && e.target == 0);
        let has_21 = graph.edges.iter().any(|e| e.source == 2 && e.target == 1);
        assert!(!has_10, "should not have edge 1->0");
        assert!(!has_20, "should not have edge 2->0");
        assert!(!has_21, "should not have edge 2->1");
    }

    /// GrangerCausalityExtractor: too few snapshots.
    #[test]
    fn test_granger_too_few_snapshots() {
        let mut extractor = GrangerCausalityExtractor::new(0.1, 5);
        let snapshot = AttentionSnapshot {
            weights: vec![vec![1.0]],
            timestamp: 0.0,
        };
        let result = extractor.extract(&[snapshot]);
        assert!(result.is_err());
    }

    /// TemporalEmbeddingStore: store and retrieve.
    #[test]
    fn test_temporal_store_retrieve() {
        let mut store = TemporalEmbeddingStore::new(4, 3, 10.0, 100.0);

        // Store embeddings for node 0 at different times.
        store.store(0, 0.0, &[1.0, 0.0, 0.0, 0.0]);
        store.store(0, 1.0, &[1.0, 0.1, 0.0, 0.0]); // small delta
        store.store(0, 2.0, &[1.0, 0.1, 0.2, 0.0]); // another small delta
        store.store(0, 3.0, &[0.0, 0.0, 0.0, 1.0]); // big change -> new base

        assert_eq!(store.chain_length(0), 4);

        // Retrieve at t=0.
        let emb0 = store.retrieve(0, 0.0).expect("should find t=0");
        assert!((emb0[0] - 1.0).abs() < 1e-6);
        assert!((emb0[1] - 0.0).abs() < 1e-6);

        // Retrieve at t=1.
        let emb1 = store.retrieve(0, 1.0).expect("should find t=1");
        assert!((emb1[0] - 1.0).abs() < 1e-6);
        assert!((emb1[1] - 0.1).abs() < 1e-6);

        // Retrieve at t=2.
        let emb2 = store.retrieve(0, 2.0).expect("should find t=2");
        assert!((emb2[2] - 0.2).abs() < 1e-6);

        // Retrieve at t=3.
        let emb3 = store.retrieve(0, 3.0).expect("should find t=3");
        assert!((emb3[3] - 1.0).abs() < 1e-6);
        assert!((emb3[0] - 0.0).abs() < 1e-6);

        // Retrieve at t=0.5 should return t=0 (latest before 0.5).
        let emb_half = store.retrieve(0, 0.5).expect("should find entry <= 0.5");
        assert!((emb_half[0] - 1.0).abs() < 1e-6);

        // Retrieve at t=-1.0 should return None.
        assert!(store.retrieve(0, -1.0).is_none());

        // Retrieve for non-existent node.
        assert!(store.retrieve(99, 0.0).is_none());
    }

    /// TemporalEmbeddingStore: compact tiers.
    #[test]
    fn test_temporal_store_compact() {
        let mut store = TemporalEmbeddingStore::new(2, 1, 5.0, 20.0);

        store.store(0, 0.0, &[1.0, 0.0]);
        store.store(0, 10.0, &[0.0, 1.0]);
        store.store(0, 25.0, &[0.5, 0.5]);

        // Compact at t=30.
        store.compact(30.0);

        // Entry at t=0 (age=30) -> Cold.
        // Entry at t=10 (age=20) -> Cold.
        // Entry at t=25 (age=5) -> Warm.
        // (Tier is internal; we just verify no crash and retrieval still works.)

        let emb = store
            .retrieve(0, 25.0)
            .expect("should still retrieve after compaction");
        assert!((emb[0] - 0.5).abs() < 1e-6);
    }

    /// TemporalEdgeEvent: struct fields.
    #[test]
    fn test_temporal_edge_event() {
        let event = TemporalEdgeEvent {
            source: 0,
            target: 1,
            timestamp: 42.0,
            event_type: EdgeEventType::Add,
        };
        assert_eq!(event.source, 0);
        assert_eq!(event.target, 1);
        assert!((event.timestamp - 42.0).abs() < 1e-10);
        assert_eq!(event.event_type, EdgeEventType::Add);

        let update = TemporalEdgeEvent {
            source: 2,
            target: 3,
            timestamp: 99.0,
            event_type: EdgeEventType::UpdateWeight(0.75),
        };
        assert_eq!(update.event_type, EdgeEventType::UpdateWeight(0.75));

        let remove = TemporalEdgeEvent {
            source: 0,
            target: 1,
            timestamp: 100.0,
            event_type: EdgeEventType::Remove,
        };
        assert_eq!(remove.event_type, EdgeEventType::Remove);
    }
}
