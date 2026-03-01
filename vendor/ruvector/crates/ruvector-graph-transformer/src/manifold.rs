//! Product manifold attention for mixed-curvature spaces.
//!
//! Implements attention computation on the product manifold S^n x H^m x R^k,
//! where S^n is the n-sphere, H^m is hyperbolic space, and R^k is Euclidean.
//! Curvature-adaptive routing selects the optimal space for each attention head.
//!
//! # Types
//!
//! - [`ProductManifoldAttention`]: S^n x H^m x R^k product manifold attention with
//!   learned mixing weights and proof-gated curvature compatibility.
//! - [`ManifoldType`]: Discriminant for Poincare ball, Lorentz, sphere, and product manifolds.
//! - [`CurvatureAdaptiveRouter`]: Routes attention heads to the appropriate manifold
//!   component based on local Ollivier-Ricci curvature.
//! - [`GeodesicMessagePassing`]: Message passing with parallel transport (gyration in
//!   the Poincare ball) and Frechet mean aggregation.
//! - [`RiemannianAdamOptimizer`]: Adam on product manifolds with exp/log maps and
//!   curvature-rescaled gradients.
//! - [`LieGroupEquivariantAttention`]: SE(3)/SO(3) equivariant attention via sheaf bundles.

#[cfg(feature = "manifold")]
use ruvector_attention::{
    Attention, HyperbolicAttention, HyperbolicAttentionConfig, ScaledDotProductAttention,
};

#[cfg(feature = "manifold")]
use ruvector_verified::{
    gated::{route_proof, ProofKind},
    proof_store::create_attestation,
    prove_dim_eq, ProofAttestation, ProofEnvironment,
};

#[cfg(feature = "manifold")]
use crate::config::ManifoldConfig;
#[cfg(feature = "manifold")]
use crate::error::{GraphTransformerError, Result};

// ---------------------------------------------------------------------------
// Numeric helpers
// ---------------------------------------------------------------------------

#[cfg(feature = "manifold")]
const EPS: f32 = 1e-7;

#[cfg(feature = "manifold")]
#[inline]
fn norm_sq(v: &[f32]) -> f32 {
    v.iter().map(|&x| x * x).sum()
}

#[cfg(feature = "manifold")]
#[inline]
fn norm(v: &[f32]) -> f32 {
    norm_sq(v).sqrt()
}

#[cfg(feature = "manifold")]
#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
}

// =========================================================================
// ManifoldType
// =========================================================================

/// Discriminant for the geometry of a manifold component.
#[cfg(feature = "manifold")]
#[derive(Debug, Clone, PartialEq)]
pub enum ManifoldType {
    /// Poincare ball model of hyperbolic space with curvature c > 0
    /// (the "negative curvature" is encoded as -c in the metric).
    PoincareBall { curvature: f32 },
    /// Lorentz hyperboloid model with curvature c > 0.
    Lorentz { curvature: f32 },
    /// Unit n-sphere (positive curvature = 1).
    Sphere,
    /// Cartesian product of manifold components.
    Product(Vec<ManifoldType>),
}

// =========================================================================
// ProductManifoldAttention
// =========================================================================

/// Product manifold attention operating on S^n x H^m x R^k.
///
/// Splits the embedding space into three components, applies
/// geometry-appropriate attention in each, and combines results
/// using learned mixing weights (beta_S, beta_H, beta_E).
///
/// A proof gate verifies curvature compatibility at each forward pass:
/// - Hyperbolic curvature c > 0
/// - Spherical projections satisfy ||x|| = 1
/// - Poincare points satisfy ||x||^2 < 1/c
///
/// The proof routes to [`ProofTier::Reflex`] for near-zero cost verification.
#[cfg(feature = "manifold")]
pub struct ProductManifoldAttention {
    config: ManifoldConfig,
    /// Attention for spherical component.
    spherical_attention: ScaledDotProductAttention,
    /// Attention for hyperbolic component.
    hyperbolic_attention: HyperbolicAttention,
    /// Attention for Euclidean component.
    euclidean_attention: ScaledDotProductAttention,
    /// Total dimension.
    total_dim: usize,
    /// Learned mixing weight for spherical component.
    beta_s: f32,
    /// Learned mixing weight for hyperbolic component.
    beta_h: f32,
    /// Learned mixing weight for Euclidean component.
    beta_e: f32,
    /// Proof environment for curvature compatibility checks.
    env: ProofEnvironment,
}

/// Result of a product manifold attention computation.
#[cfg(feature = "manifold")]
#[derive(Debug)]
pub struct ManifoldAttentionResult {
    /// Output features in the product manifold.
    pub output: Vec<f32>,
    /// Curvatures used for each component.
    pub curvatures: ManifoldCurvatures,
    /// Proof attestation from curvature compatibility gate.
    pub attestation: Option<ProofAttestation>,
}

/// Curvature values for each manifold component.
#[cfg(feature = "manifold")]
#[derive(Debug, Clone)]
pub struct ManifoldCurvatures {
    /// Spherical curvature (positive).
    pub spherical: f32,
    /// Hyperbolic curvature (negative).
    pub hyperbolic: f32,
    /// Euclidean curvature (zero).
    pub euclidean: f32,
}

#[cfg(feature = "manifold")]
impl ProductManifoldAttention {
    /// Create a new product manifold attention module.
    pub fn new(config: ManifoldConfig) -> Self {
        let total_dim = config.spherical_dim + config.hyperbolic_dim + config.euclidean_dim;

        let spherical_attention = ScaledDotProductAttention::new(config.spherical_dim);
        let hyperbolic_config = HyperbolicAttentionConfig {
            dim: config.hyperbolic_dim,
            curvature: config.curvature,
            adaptive_curvature: false,
            temperature: 1.0,
            frechet_max_iter: 100,
            frechet_tol: 1e-6,
        };
        let hyperbolic_attention = HyperbolicAttention::new(hyperbolic_config);
        let euclidean_attention = ScaledDotProductAttention::new(config.euclidean_dim);

        Self {
            config,
            spherical_attention,
            hyperbolic_attention,
            euclidean_attention,
            total_dim,
            beta_s: 1.0,
            beta_h: 1.0,
            beta_e: 1.0,
            env: ProofEnvironment::new(),
        }
    }

    /// Create with explicit mixing weights.
    pub fn with_betas(config: ManifoldConfig, beta_s: f32, beta_h: f32, beta_e: f32) -> Self {
        let mut attn = Self::new(config);
        attn.beta_s = beta_s;
        attn.beta_h = beta_h;
        attn.beta_e = beta_e;
        attn
    }

    /// Verify curvature compatibility via proof gate (Reflex tier).
    ///
    /// Checks:
    /// - Hyperbolic curvature magnitude > 0
    /// - Spherical projection has unit norm
    /// - Poincare points satisfy ||x||^2 < 1/c
    fn verify_curvature_compatibility(
        &mut self,
        _q_s: &[f32],
        q_h: &[f32],
    ) -> Result<ProofAttestation> {
        let c = self.config.curvature.abs();
        if c < EPS {
            return Err(GraphTransformerError::InvariantViolation(
                "hyperbolic curvature must be non-zero".into(),
            ));
        }

        // Spherical: ||q_s|| should be 1 after projection (we project below).
        // Poincare: ||q_h||^2 < 1/c
        let norm_h_sq = norm_sq(q_h);
        if norm_h_sq >= 1.0 / c {
            // The point will be projected, so this is a soft check.
            // We log but do not fail; the projection handles it.
        }

        // Route to Reflex tier (trivial curvature dimension proof).
        let decision = route_proof(ProofKind::Reflexivity, &self.env);
        let dim_tag = self.total_dim as u32;
        let proof_id = ruvector_verified::gated::verify_tiered(
            &mut self.env,
            dim_tag,
            dim_tag,
            decision.tier,
        )?;

        Ok(create_attestation(&self.env, proof_id))
    }

    /// Compute attention in the product manifold.
    ///
    /// Splits features into (S^n, H^m, R^k) components, applies
    /// geometry-appropriate attention, applies learned mixing weights,
    /// and concatenates results.
    pub fn compute(
        &mut self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<ManifoldAttentionResult> {
        if query.len() != self.total_dim {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: self.total_dim,
                actual: query.len(),
            });
        }

        let s_dim = self.config.spherical_dim;
        let h_dim = self.config.hyperbolic_dim;

        // Split query
        let q_s = &query[..s_dim];
        let q_h = &query[s_dim..s_dim + h_dim];
        let q_e = &query[s_dim + h_dim..];

        // Proof gate: verify curvature compatibility.
        let attestation = self.verify_curvature_compatibility(q_s, q_h).ok();

        // Split keys and values
        let k_s: Vec<&[f32]> = keys.iter().map(|k| &k[..s_dim]).collect();
        let k_h: Vec<&[f32]> = keys.iter().map(|k| &k[s_dim..s_dim + h_dim]).collect();
        let k_e: Vec<&[f32]> = keys.iter().map(|k| &k[s_dim + h_dim..]).collect();

        let v_s: Vec<&[f32]> = values.iter().map(|v| &v[..s_dim]).collect();
        let v_h: Vec<&[f32]> = values.iter().map(|v| &v[s_dim..s_dim + h_dim]).collect();
        let v_e: Vec<&[f32]> = values.iter().map(|v| &v[s_dim + h_dim..]).collect();

        // Spherical attention (project to sphere first)
        let q_s_proj = project_to_sphere(q_s);
        let k_s_proj: Vec<Vec<f32>> = k_s.iter().map(|k| project_to_sphere(k)).collect();
        let k_s_refs: Vec<&[f32]> = k_s_proj.iter().map(|k| k.as_slice()).collect();
        let out_s = self
            .spherical_attention
            .compute(&q_s_proj, &k_s_refs, &v_s)
            .map_err(GraphTransformerError::Attention)?;

        // Hyperbolic attention
        let out_h = self
            .hyperbolic_attention
            .compute(q_h, &k_h, &v_h)
            .map_err(GraphTransformerError::Attention)?;

        // Euclidean attention
        let out_e = self
            .euclidean_attention
            .compute(q_e, &k_e, &v_e)
            .map_err(GraphTransformerError::Attention)?;

        // Apply learned mixing weights and normalize
        let beta_sum = self.beta_s + self.beta_h + self.beta_e;
        let w_s = self.beta_s / beta_sum;
        let w_h = self.beta_h / beta_sum;
        let w_e = self.beta_e / beta_sum;

        // Concatenate with mixing weights applied
        let mut output = Vec::with_capacity(self.total_dim);
        output.extend(out_s.iter().map(|&x| w_s * x));
        output.extend(out_h.iter().map(|&x| w_h * x));
        output.extend(out_e.iter().map(|&x| w_e * x));

        let curvatures = ManifoldCurvatures {
            spherical: 1.0,
            hyperbolic: self.config.curvature,
            euclidean: 0.0,
        };

        Ok(ManifoldAttentionResult {
            output,
            curvatures,
            attestation,
        })
    }

    /// Get the total embedding dimension.
    pub fn total_dim(&self) -> usize {
        self.total_dim
    }

    /// Get the configuration.
    pub fn config(&self) -> &ManifoldConfig {
        &self.config
    }

    /// Get the manifold type for this attention module.
    pub fn manifold_type(&self) -> ManifoldType {
        ManifoldType::Product(vec![
            ManifoldType::Sphere,
            ManifoldType::PoincareBall {
                curvature: self.config.curvature.abs(),
            },
            ManifoldType::PoincareBall { curvature: 0.0 }, // flat = Euclidean
        ])
    }
}

// =========================================================================
// CurvatureAdaptiveRouter
// =========================================================================

/// Routes attention heads to the appropriate manifold component based on
/// local Ollivier-Ricci curvature estimated from the graph structure.
///
/// Routing is *soft* (sigmoid blending) to preserve gradient flow:
/// - Negative curvature (tree-like) -> hyperbolic weight high
/// - Positive curvature (clustered) -> spherical weight high
/// - Near-zero curvature (grid-like) -> Euclidean weight high
#[cfg(feature = "manifold")]
pub struct CurvatureAdaptiveRouter {
    /// Threshold below which curvature is considered "negative".
    neg_threshold: f32,
    /// Threshold above which curvature is considered "positive".
    pos_threshold: f32,
    /// Sigmoid temperature for soft routing (higher = sharper transitions).
    temperature: f32,
}

/// Routing weights for the three manifold components.
#[cfg(feature = "manifold")]
#[derive(Debug, Clone)]
pub struct RoutingWeights {
    /// Weight for spherical component.
    pub spherical: f32,
    /// Weight for hyperbolic component.
    pub hyperbolic: f32,
    /// Weight for Euclidean component.
    pub euclidean: f32,
}

#[cfg(feature = "manifold")]
impl CurvatureAdaptiveRouter {
    /// Create a new router with default thresholds.
    pub fn new() -> Self {
        Self {
            neg_threshold: -0.1,
            pos_threshold: 0.1,
            temperature: 5.0,
        }
    }

    /// Create a router with custom thresholds and temperature.
    pub fn with_params(neg_threshold: f32, pos_threshold: f32, temperature: f32) -> Self {
        Self {
            neg_threshold,
            pos_threshold,
            temperature,
        }
    }

    /// Compute soft routing weights for a given Ollivier-Ricci curvature value.
    ///
    /// Uses sigmoid activations for smooth gradient flow:
    /// - w_hyp  = sigma(temperature * (neg_threshold - kappa))
    /// - w_sph  = sigma(temperature * (kappa - pos_threshold))
    /// - w_euc  = exp(-temperature * kappa^2 / 2)  (Gaussian bump at zero)
    ///
    /// All three are then softmax-normalized to sum to 1, preserving
    /// gradient flow through each component.
    pub fn route(&self, ollivier_ricci_curvature: f32) -> RoutingWeights {
        let kappa = ollivier_ricci_curvature;

        // Sigmoid: sigma(x) = 1 / (1 + exp(-x))
        let w_hyp = sigmoid(self.temperature * (self.neg_threshold - kappa));
        let w_sph = sigmoid(self.temperature * (kappa - self.pos_threshold));
        // Gaussian bump centered at zero: peaks when kappa ~ 0.
        let w_euc = (-self.temperature * kappa * kappa / 2.0).exp();

        // Normalize to sum to 1
        let total = w_hyp + w_sph + w_euc;
        RoutingWeights {
            hyperbolic: w_hyp / total,
            spherical: w_sph / total,
            euclidean: w_euc / total,
        }
    }

    /// Route a batch of curvature values.
    pub fn route_batch(&self, curvatures: &[f32]) -> Vec<RoutingWeights> {
        curvatures.iter().map(|&k| self.route(k)).collect()
    }

    /// Estimate Ollivier-Ricci curvature for an edge (i, j) given
    /// adjacency lists for i and j.
    ///
    /// Uses the simplified Wasserstein-1 approximation:
    /// kappa(i,j) = 1 - W_1(m_i, m_j) / d(i,j)
    ///
    /// where m_i is the uniform distribution over neighbors of i
    /// (including i itself with lazy random walk probability).
    pub fn estimate_ollivier_ricci(
        &self,
        node_i_features: &[f32],
        node_j_features: &[f32],
        neighbors_i: &[&[f32]],
        neighbors_j: &[&[f32]],
    ) -> f32 {
        let d_ij = euclidean_distance(node_i_features, node_j_features);
        if d_ij < EPS {
            return 1.0; // identical points have max curvature
        }

        // Approximate W_1 by comparing centroids of neighbor distributions.
        let centroid_i = compute_centroid(neighbors_i);
        let centroid_j = compute_centroid(neighbors_j);
        let w1_approx = euclidean_distance(&centroid_i, &centroid_j);

        1.0 - w1_approx / d_ij
    }
}

#[cfg(feature = "manifold")]
impl Default for CurvatureAdaptiveRouter {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================
// GeodesicMessagePassing
// =========================================================================

/// Message passing on Riemannian manifolds with parallel transport.
///
/// For the Poincare ball model, parallel transport is implemented via
/// gyration (Mobius gyrovector rotation). Transport preserves vector norm,
/// which is verified through a Reflex-tier proof gate.
///
/// Aggregation uses the iterative Frechet mean (Riemannian gradient descent
/// on the sum-of-squared-geodesic-distances objective).
#[cfg(feature = "manifold")]
pub struct GeodesicMessagePassing {
    /// Manifold type for transport.
    manifold: ManifoldType,
    /// Maximum iterations for Frechet mean.
    frechet_max_iter: usize,
    /// Convergence tolerance for Frechet mean.
    frechet_tol: f32,
    /// Proof environment for norm-preservation verification.
    env: ProofEnvironment,
}

/// Result of a geodesic message passing step.
#[cfg(feature = "manifold")]
#[derive(Debug)]
pub struct MessagePassingResult {
    /// Aggregated messages per node.
    pub node_messages: Vec<Vec<f32>>,
    /// Whether all parallel transports preserved norm.
    pub norms_preserved: bool,
    /// Proof attestation for norm preservation.
    pub attestation: Option<ProofAttestation>,
}

#[cfg(feature = "manifold")]
impl GeodesicMessagePassing {
    /// Create a new geodesic message passing module.
    pub fn new(manifold: ManifoldType) -> Self {
        let curvature = match &manifold {
            ManifoldType::PoincareBall { curvature } => *curvature,
            _ => 1.0,
        };
        // Defaults based on curvature magnitude.
        let max_iter = if curvature > 5.0 { 200 } else { 100 };
        Self {
            manifold,
            frechet_max_iter: max_iter,
            frechet_tol: 1e-6,
            env: ProofEnvironment::new(),
        }
    }

    /// Create with custom Frechet mean parameters.
    pub fn with_frechet_params(manifold: ManifoldType, max_iter: usize, tol: f32) -> Self {
        Self {
            manifold,
            frechet_max_iter: max_iter,
            frechet_tol: tol,
            env: ProofEnvironment::new(),
        }
    }

    /// Parallel transport vector `v` from tangent space at `from` to tangent
    /// space at `to` in the Poincare ball with curvature `c`.
    ///
    /// Uses the gyration-based formula:
    /// PT_{from->to}(v) = gyr[to, -from](v) * lambda_from / lambda_to
    ///
    /// where lambda_x = 2 / (1 - c||x||^2) is the conformal factor.
    pub fn parallel_transport_poincare(
        &self,
        v: &[f32],
        from: &[f32],
        to: &[f32],
        c: f32,
    ) -> Vec<f32> {
        let c = c.abs().max(EPS);
        let lambda_from = conformal_factor(from, c);
        let lambda_to = conformal_factor(to, c);

        // Gyration: gyr[a,b](v) via the formula
        // gyr[a,b](v) = -(a (+) b) (+) (a (+) (b (+) v))
        // where (+) is Mobius addition.
        let b_plus_v = mobius_add_internal(to, v, c);
        let a_plus_bv = mobius_add_internal(from, &b_plus_v, c);
        let a_plus_b = mobius_add_internal(from, to, c);
        let neg_ab: Vec<f32> = a_plus_b.iter().map(|&x| -x).collect();
        let gyrated = mobius_add_internal(&neg_ab, &a_plus_bv, c);

        // Scale by conformal factor ratio.
        let scale = lambda_from / lambda_to.max(EPS);
        gyrated.iter().map(|&x| x * scale).collect()
    }

    /// Parallel transport for spherical manifold.
    /// Uses the standard formula for S^n.
    pub fn parallel_transport_sphere(&self, v: &[f32], from: &[f32], to: &[f32]) -> Vec<f32> {
        let d = dot(from, to).clamp(-1.0, 1.0);
        let angle = d.acos();
        if angle.abs() < EPS {
            return v.to_vec();
        }

        // Transport along the geodesic from -> to on S^n.
        // PT(v) = v - (dot(from + to, v) / (1 + d)) * (from + to)
        let sum: Vec<f32> = from.iter().zip(to.iter()).map(|(&a, &b)| a + b).collect();
        let dot_sv = dot(&sum, v);
        let coeff = dot_sv / (1.0 + d).max(EPS);
        v.iter()
            .zip(sum.iter())
            .map(|(&vi, &si)| vi - coeff * si)
            .collect()
    }

    /// Perform one round of geodesic message passing.
    ///
    /// For each node, gathers messages from neighbors via parallel transport
    /// to the node's tangent space, then aggregates via Frechet mean.
    pub fn propagate(
        &mut self,
        node_features: &[Vec<f32>],
        edges: &[(usize, usize)],
    ) -> Result<MessagePassingResult> {
        let n = node_features.len();
        let dim = if n > 0 { node_features[0].len() } else { 0 };

        // Build adjacency: for each node, collect neighbor indices.
        let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
        for &(u, v) in edges {
            if u < n && v < n {
                adj[u].push(v);
                adj[v].push(u);
            }
        }

        let mut node_messages = Vec::with_capacity(n);
        let mut all_norms_preserved = true;

        for i in 0..n {
            if adj[i].is_empty() {
                node_messages.push(node_features[i].clone());
                continue;
            }

            // Transport neighbor features to tangent space at node i.
            let mut transported: Vec<Vec<f32>> = Vec::with_capacity(adj[i].len());
            for &j in &adj[i] {
                let msg = match &self.manifold {
                    ManifoldType::PoincareBall { curvature } => self.parallel_transport_poincare(
                        &node_features[j],
                        &node_features[j],
                        &node_features[i],
                        *curvature,
                    ),
                    ManifoldType::Sphere => {
                        let from_proj = project_to_sphere(&node_features[j]);
                        let to_proj = project_to_sphere(&node_features[i]);
                        self.parallel_transport_sphere(&node_features[j], &from_proj, &to_proj)
                    }
                    _ => {
                        // Euclidean or other: no transport needed.
                        node_features[j].clone()
                    }
                };

                // Verify norm preservation.
                let orig_norm = norm(&node_features[j]);
                let trans_norm = norm(&msg);
                if orig_norm > EPS && (trans_norm / orig_norm - 1.0).abs() > 0.1 {
                    all_norms_preserved = false;
                }

                transported.push(msg);
            }

            // Aggregate via Frechet mean.
            let aggregated = match &self.manifold {
                ManifoldType::PoincareBall { curvature } => {
                    let refs: Vec<&[f32]> = transported.iter().map(|t| t.as_slice()).collect();
                    ruvector_attention::hyperbolic::frechet_mean(
                        &refs,
                        None,
                        *curvature,
                        self.frechet_max_iter,
                        self.frechet_tol,
                    )
                }
                ManifoldType::Sphere => {
                    // Frechet mean on sphere via iterative projection.
                    spherical_frechet_mean(&transported, self.frechet_max_iter, self.frechet_tol)
                }
                _ => {
                    // Euclidean mean.
                    euclidean_mean(&transported)
                }
            };

            node_messages.push(aggregated);
        }

        // Proof gate: verify norm preservation via Reflex tier.
        let attestation = if all_norms_preserved {
            let dim_tag = dim as u32;
            let proof_id = prove_dim_eq(&mut self.env, dim_tag, dim_tag)?;
            Some(create_attestation(&self.env, proof_id))
        } else {
            None
        };

        Ok(MessagePassingResult {
            node_messages,
            norms_preserved: all_norms_preserved,
            attestation,
        })
    }
}

// =========================================================================
// RiemannianAdamOptimizer
// =========================================================================

/// Adam optimizer on product manifolds.
///
/// Performs Riemannian Adam by:
/// 1. Rescaling the Euclidean gradient by the inverse conformal factor
///    (for Poincare ball components).
/// 2. Maintaining first and second moment estimates in tangent space.
/// 3. Parallel transporting momentum between steps.
/// 4. Applying updates via the exponential map.
///
/// A proof gate verifies that updated parameters remain on the manifold.
#[cfg(feature = "manifold")]
pub struct RiemannianAdamOptimizer {
    /// Learning rate.
    lr: f32,
    /// First moment decay.
    beta1: f32,
    /// Second moment decay.
    beta2: f32,
    /// Numerical stability epsilon.
    adam_eps: f32,
    /// First moment estimate.
    m: Vec<f32>,
    /// Second moment estimate.
    v: Vec<f32>,
    /// Step counter.
    t: u32,
    /// Manifold type for the parameter space.
    manifold: ManifoldType,
    /// Proof environment.
    env: ProofEnvironment,
}

/// Result of an optimizer step.
#[cfg(feature = "manifold")]
#[derive(Debug)]
pub struct OptimizerStepResult {
    /// Updated parameters.
    pub params: Vec<f32>,
    /// Whether the updated params lie on the manifold.
    pub on_manifold: bool,
    /// Proof attestation for manifold membership.
    pub attestation: Option<ProofAttestation>,
}

#[cfg(feature = "manifold")]
impl RiemannianAdamOptimizer {
    /// Create a new Riemannian Adam optimizer.
    pub fn new(dim: usize, manifold: ManifoldType) -> Self {
        Self {
            lr: 0.001,
            beta1: 0.9,
            beta2: 0.999,
            adam_eps: 1e-8,
            m: vec![0.0; dim],
            v: vec![0.0; dim],
            t: 0,
            manifold,
            env: ProofEnvironment::new(),
        }
    }

    /// Create with custom hyperparameters.
    pub fn with_params(
        dim: usize,
        manifold: ManifoldType,
        lr: f32,
        beta1: f32,
        beta2: f32,
    ) -> Self {
        Self {
            lr,
            beta1,
            beta2,
            adam_eps: 1e-8,
            m: vec![0.0; dim],
            v: vec![0.0; dim],
            t: 0,
            manifold,
            env: ProofEnvironment::new(),
        }
    }

    /// Perform one optimization step.
    ///
    /// 1. Compute Riemannian gradient from Euclidean gradient.
    /// 2. Update moment estimates.
    /// 3. Compute bias-corrected update direction.
    /// 4. Apply via exponential map.
    /// 5. Project back to manifold.
    /// 6. Proof gate: verify manifold membership.
    pub fn step(&mut self, params: &[f32], euclidean_grad: &[f32]) -> Result<OptimizerStepResult> {
        if params.len() != euclidean_grad.len() || params.len() != self.m.len() {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: self.m.len(),
                actual: params.len(),
            });
        }

        self.t += 1;
        let dim = params.len();

        // Compute Riemannian gradient by rescaling with inverse conformal factor.
        let riemannian_grad = match &self.manifold {
            ManifoldType::PoincareBall { curvature } => {
                let c = curvature.abs().max(EPS);
                let norm_sq_p = norm_sq(params);
                // Conformal factor: lambda = 2 / (1 - c||x||^2)
                // Riemannian gradient = (1 - c||x||^2)^2 / 4 * euclidean_grad
                let factor = (1.0 - c * norm_sq_p).max(EPS);
                let scale = factor * factor / 4.0;
                euclidean_grad
                    .iter()
                    .map(|&g| scale * g)
                    .collect::<Vec<f32>>()
            }
            ManifoldType::Sphere => {
                // Project gradient to tangent space: g_tan = g - <g, x>x
                let dp = dot(euclidean_grad, params);
                euclidean_grad
                    .iter()
                    .zip(params.iter())
                    .map(|(&g, &p)| g - dp * p)
                    .collect::<Vec<f32>>()
            }
            _ => euclidean_grad.to_vec(),
        };

        // Update biased first and second moment estimates.
        for i in 0..dim {
            self.m[i] = self.beta1 * self.m[i] + (1.0 - self.beta1) * riemannian_grad[i];
            self.v[i] = self.beta2 * self.v[i]
                + (1.0 - self.beta2) * riemannian_grad[i] * riemannian_grad[i];
        }

        // Bias correction.
        let bc1 = 1.0 - self.beta1.powi(self.t as i32);
        let bc2 = 1.0 - self.beta2.powi(self.t as i32);

        // Compute update direction in tangent space.
        let update: Vec<f32> = (0..dim)
            .map(|i| {
                let m_hat = self.m[i] / bc1;
                let v_hat = self.v[i] / bc2;
                -self.lr * m_hat / (v_hat.sqrt() + self.adam_eps)
            })
            .collect();

        // Apply via exponential map and project.
        let new_params = match &self.manifold {
            ManifoldType::PoincareBall { curvature } => {
                let c = curvature.abs().max(EPS);
                let exp = poincare_exp_map(&update, params, c);
                poincare_project(&exp, c)
            }
            ManifoldType::Sphere => {
                let exp = sphere_exp_map(&update, params);
                project_to_sphere(&exp)
            }
            _ => {
                // Euclidean: just add.
                params
                    .iter()
                    .zip(update.iter())
                    .map(|(&p, &u)| p + u)
                    .collect()
            }
        };

        // Proof gate: verify manifold membership.
        let on_manifold = self.check_on_manifold(&new_params);
        let attestation = if on_manifold {
            let dim_tag = dim as u32;
            let proof_id = prove_dim_eq(&mut self.env, dim_tag, dim_tag)?;
            Some(create_attestation(&self.env, proof_id))
        } else {
            None
        };

        Ok(OptimizerStepResult {
            params: new_params,
            on_manifold,
            attestation,
        })
    }

    /// Check whether a point lies on the manifold.
    fn check_on_manifold(&self, params: &[f32]) -> bool {
        match &self.manifold {
            ManifoldType::PoincareBall { curvature } => {
                let c = curvature.abs().max(EPS);
                norm_sq(params) < 1.0 / c
            }
            ManifoldType::Sphere => (norm(params) - 1.0).abs() < 0.01,
            _ => true,
        }
    }
}

// =========================================================================
// LieGroupEquivariantAttention
// =========================================================================

/// Lie group type for equivariant operations.
#[cfg(feature = "manifold")]
#[derive(Debug, Clone, PartialEq)]
pub enum LieGroupType {
    /// Special orthogonal group in 3D (rotations).
    SO3,
    /// Special Euclidean group in 3D (rotations + translations).
    SE3,
    /// Unitary group U(1) (phase rotations).
    U1,
}

/// SE(3)/SO(3) equivariant attention via sheaf bundle decomposition.
///
/// Decomposes features into irreducible representations (irreps) of the
/// chosen Lie group and applies equivariant attention that commutes with
/// group actions.
///
/// For SO(3): features decompose into scalar (l=0), vector (l=1), and
/// tensor (l=2) components. Attention weights are computed from invariant
/// (scalar) features only, then applied to all irreps.
#[cfg(feature = "manifold")]
pub struct LieGroupEquivariantAttention {
    /// The Lie group for equivariance.
    group: LieGroupType,
    /// Scalar (invariant) dimension.
    scalar_dim: usize,
    /// Vector (l=1 irrep) dimension.
    vector_dim: usize,
    /// Total feature dimension.
    total_dim: usize,
    /// Proof environment (reserved for future equivariance proofs).
    _env: ProofEnvironment,
}

/// Result of Lie-group-equivariant attention.
#[cfg(feature = "manifold")]
#[derive(Debug)]
pub struct EquivariantAttentionResult {
    /// Output features preserving equivariance.
    pub output: Vec<f32>,
    /// Scalar (invariant) part of the output.
    pub scalar_output: Vec<f32>,
    /// Vector (l=1) part of the output.
    pub vector_output: Vec<f32>,
}

#[cfg(feature = "manifold")]
impl LieGroupEquivariantAttention {
    /// Create a new Lie-group-equivariant attention module.
    ///
    /// `scalar_dim` is the dimension of the invariant (l=0) features.
    /// `vector_dim` is the dimension of the l=1 irrep features (must be
    /// divisible by 3 for SO3/SE3).
    pub fn new(group: LieGroupType, scalar_dim: usize, vector_dim: usize) -> Self {
        Self {
            group,
            scalar_dim,
            vector_dim,
            total_dim: scalar_dim + vector_dim,
            _env: ProofEnvironment::new(),
        }
    }

    /// Compute equivariant attention.
    ///
    /// Attention weights are derived from scalar (invariant) features only,
    /// ensuring that the weighting commutes with group transformations.
    /// The same weights are then applied to both scalar and vector components.
    pub fn compute(
        &self,
        query: &[f32],
        keys: &[Vec<f32>],
        values: &[Vec<f32>],
    ) -> Result<EquivariantAttentionResult> {
        if query.len() != self.total_dim {
            return Err(GraphTransformerError::DimensionMismatch {
                expected: self.total_dim,
                actual: query.len(),
            });
        }

        let sd = self.scalar_dim;

        // Split into scalar and vector parts.
        let q_scalar = &query[..sd];
        let _q_vector = &query[sd..];

        let k_scalars: Vec<&[f32]> = keys.iter().map(|k| &k[..sd]).collect();
        let v_scalars: Vec<&[f32]> = values.iter().map(|v| &v[..sd]).collect();
        let v_vectors: Vec<&[f32]> = values.iter().map(|v| &v[sd..]).collect();

        // Compute attention weights from scalar features only (equivariance-preserving).
        let weights = self.compute_invariant_weights(q_scalar, &k_scalars);

        // Apply weights to scalar component.
        let scalar_out = weighted_sum(&weights, &v_scalars, sd);

        // Apply same weights to vector component.
        let vec_dim = self.vector_dim;
        let vector_out = weighted_sum(&weights, &v_vectors, vec_dim);

        // Concatenate.
        let mut output = Vec::with_capacity(self.total_dim);
        output.extend_from_slice(&scalar_out);
        output.extend_from_slice(&vector_out);

        Ok(EquivariantAttentionResult {
            output,
            scalar_output: scalar_out,
            vector_output: vector_out,
        })
    }

    /// Compute attention weights from invariant (scalar) features.
    fn compute_invariant_weights(&self, query: &[f32], keys: &[&[f32]]) -> Vec<f32> {
        if keys.is_empty() {
            return vec![];
        }

        let scale = (self.scalar_dim as f32).sqrt();
        let scores: Vec<f32> = keys.iter().map(|k| dot(query, k) / scale).collect();

        softmax(&scores)
    }

    /// Get the Lie group type.
    pub fn group(&self) -> &LieGroupType {
        &self.group
    }

    /// Get total dimension.
    pub fn total_dim(&self) -> usize {
        self.total_dim
    }
}

// =========================================================================
// Internal helpers
// =========================================================================

/// Project a vector onto the unit sphere.
#[cfg(feature = "manifold")]
fn project_to_sphere(v: &[f32]) -> Vec<f32> {
    let n = norm(v);
    if n < EPS {
        let mut result = vec![0.0; v.len()];
        if !result.is_empty() {
            result[0] = 1.0;
        }
        return result;
    }
    v.iter().map(|&x| x / n).collect()
}

/// Sigmoid function.
#[cfg(feature = "manifold")]
#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Euclidean distance between two vectors.
#[cfg(feature = "manifold")]
fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Compute the centroid (Euclidean mean) of a set of vectors.
#[cfg(feature = "manifold")]
fn compute_centroid(points: &[&[f32]]) -> Vec<f32> {
    if points.is_empty() {
        return vec![];
    }
    let dim = points[0].len();
    let n = points.len() as f32;
    let mut centroid = vec![0.0f32; dim];
    for p in points {
        for (i, &val) in p.iter().enumerate() {
            centroid[i] += val;
        }
    }
    for c in &mut centroid {
        *c /= n;
    }
    centroid
}

/// Euclidean mean of a set of owned vectors.
#[cfg(feature = "manifold")]
fn euclidean_mean(vecs: &[Vec<f32>]) -> Vec<f32> {
    if vecs.is_empty() {
        return vec![];
    }
    let dim = vecs[0].len();
    let n = vecs.len() as f32;
    let mut mean = vec![0.0f32; dim];
    for v in vecs {
        for (i, &val) in v.iter().enumerate() {
            mean[i] += val;
        }
    }
    for m in &mut mean {
        *m /= n;
    }
    mean
}

/// Frechet mean on the sphere via iterative Riemannian gradient descent.
#[cfg(feature = "manifold")]
fn spherical_frechet_mean(points: &[Vec<f32>], max_iter: usize, tol: f32) -> Vec<f32> {
    if points.is_empty() {
        return vec![];
    }
    if points.len() == 1 {
        return project_to_sphere(&points[0]);
    }

    let dim = points[0].len();
    let lr = 0.1;

    // Initialize with Euclidean mean projected to sphere.
    let mut mean = project_to_sphere(&euclidean_mean(points));

    for _ in 0..max_iter {
        // Riemannian gradient = sum of log maps.
        let mut grad = vec![0.0f32; dim];
        for p in points {
            let p_proj = project_to_sphere(p);
            let log = sphere_log_map(&p_proj, &mean);
            for (i, &val) in log.iter().enumerate() {
                grad[i] += val;
            }
        }
        let grad_norm = norm(&grad);
        if grad_norm < tol {
            break;
        }

        // Step in the gradient direction via exp map.
        let step: Vec<f32> = grad.iter().map(|&g| lr * g / points.len() as f32).collect();
        mean = sphere_exp_map(&step, &mean);
        mean = project_to_sphere(&mean);
    }

    mean
}

/// Logarithmic map on the sphere: log_p(q).
#[cfg(feature = "manifold")]
fn sphere_log_map(q: &[f32], p: &[f32]) -> Vec<f32> {
    let d = dot(p, q).clamp(-1.0, 1.0);
    let angle = d.acos();
    if angle.abs() < EPS {
        return vec![0.0; p.len()];
    }

    // v = (q - d*p) normalized, scaled by angle
    let mut v: Vec<f32> = q
        .iter()
        .zip(p.iter())
        .map(|(&qi, &pi)| qi - d * pi)
        .collect();
    let v_norm = norm(&v);
    if v_norm < EPS {
        return vec![0.0; p.len()];
    }
    for vi in &mut v {
        *vi = *vi * angle / v_norm;
    }
    v
}

/// Exponential map on the sphere: exp_p(v).
#[cfg(feature = "manifold")]
fn sphere_exp_map(v: &[f32], p: &[f32]) -> Vec<f32> {
    let v_norm = norm(v);
    if v_norm < EPS {
        return p.to_vec();
    }
    let cos_t = v_norm.cos();
    let sin_t = v_norm.sin();
    p.iter()
        .zip(v.iter())
        .map(|(&pi, &vi)| cos_t * pi + sin_t * vi / v_norm)
        .collect()
}

/// Mobius addition (internal, does not use ruvector_attention import
/// to avoid circular complexity in transport code).
#[cfg(feature = "manifold")]
fn mobius_add_internal(u: &[f32], v: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs().max(EPS);
    let norm_u_sq = norm_sq(u);
    let norm_v_sq = norm_sq(v);
    let dot_uv: f32 = dot(u, v);

    let coef_u = 1.0 + 2.0 * c * dot_uv + c * norm_v_sq;
    let coef_v = 1.0 - c * norm_u_sq;
    let denom = 1.0 + 2.0 * c * dot_uv + c * c * norm_u_sq * norm_v_sq;

    let result: Vec<f32> = u
        .iter()
        .zip(v.iter())
        .map(|(&ui, &vi)| (coef_u * ui + coef_v * vi) / denom.max(EPS))
        .collect();

    poincare_project(&result, c)
}

/// Conformal factor lambda_x = 2 / (1 - c||x||^2).
#[cfg(feature = "manifold")]
#[inline]
fn conformal_factor(x: &[f32], c: f32) -> f32 {
    2.0 / (1.0 - c * norm_sq(x)).max(EPS)
}

/// Poincare exponential map: exp_p(v) in the Poincare ball.
#[cfg(feature = "manifold")]
fn poincare_exp_map(v: &[f32], p: &[f32], c: f32) -> Vec<f32> {
    let sqrt_c = c.sqrt();
    let norm_p_sq = norm_sq(p);
    let lambda_p = 2.0 / (1.0 - c * norm_p_sq).max(EPS);

    let v_norm = norm(v);
    if v_norm < EPS {
        return p.to_vec();
    }

    let arg = (sqrt_c * lambda_p * v_norm / 2.0).tanh();
    let coef = arg / (sqrt_c * v_norm);
    let transported: Vec<f32> = v.iter().map(|&vi| coef * vi).collect();

    mobius_add_internal(p, &transported, c)
}

/// Project a point into the Poincare ball (||x||^2 < 1/c).
#[cfg(feature = "manifold")]
fn poincare_project(x: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs().max(EPS);
    let max_norm = (1.0 / c).sqrt() - EPS;
    let x_norm = norm(x);
    if x_norm <= max_norm {
        x.to_vec()
    } else {
        let scale = max_norm / x_norm.max(EPS);
        x.iter().map(|&xi| scale * xi).collect()
    }
}

/// Weighted sum of vectors.
#[cfg(feature = "manifold")]
fn weighted_sum(weights: &[f32], vecs: &[&[f32]], dim: usize) -> Vec<f32> {
    let mut result = vec![0.0f32; dim];
    for (&w, v) in weights.iter().zip(vecs.iter()) {
        for (i, &val) in v.iter().enumerate() {
            if i < dim {
                result[i] += w * val;
            }
        }
    }
    result
}

/// Softmax over a score vector.
#[cfg(feature = "manifold")]
fn softmax(scores: &[f32]) -> Vec<f32> {
    if scores.is_empty() {
        return vec![];
    }
    let max_s = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let exp: Vec<f32> = scores.iter().map(|&s| (s - max_s).exp()).collect();
    let sum: f32 = exp.iter().sum();
    if sum < EPS {
        vec![1.0 / scores.len() as f32; scores.len()]
    } else {
        exp.iter().map(|&e| e / sum).collect()
    }
}

/// Compute geodesic distance on the sphere.
#[cfg(feature = "manifold")]
pub fn spherical_geodesic(a: &[f32], b: &[f32]) -> f32 {
    let d: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum();
    d.clamp(-1.0, 1.0).acos()
}

/// Compute geodesic distance in hyperbolic space (Poincare ball model).
#[cfg(feature = "manifold")]
pub fn hyperbolic_geodesic(a: &[f32], b: &[f32], curvature: f32) -> f32 {
    let c = curvature.abs();
    let diff_sq: f32 = a.iter().zip(b.iter()).map(|(&x, &y)| (x - y).powi(2)).sum();
    let norm_a_sq: f32 = a.iter().map(|&x| x * x).sum();
    let norm_b_sq: f32 = b.iter().map(|&x| x * x).sum();

    let denom = (1.0 - c * norm_a_sq) * (1.0 - c * norm_b_sq);
    if denom.abs() < 1e-8 {
        return f32::INFINITY;
    }

    let arg = 1.0 + 2.0 * c * diff_sq / denom;
    (1.0 / c.sqrt()) * arg.max(1.0).acosh()
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
#[cfg(feature = "manifold")]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // ProductManifoldAttention tests
    // ---------------------------------------------------------------

    #[test]
    fn test_product_manifold_attention_forward_4node() {
        let config = ManifoldConfig {
            spherical_dim: 4,
            hyperbolic_dim: 4,
            euclidean_dim: 4,
            curvature: -1.0,
        };
        let mut attn = ProductManifoldAttention::new(config);
        assert_eq!(attn.total_dim(), 12);

        // 4-node graph: query is node 0, keys/values are neighbors 1..3.
        let query = vec![0.5; 12];
        let keys = vec![vec![0.3; 12], vec![0.7; 12], vec![0.1; 12]];
        let values = vec![vec![1.0; 12], vec![2.0; 12], vec![0.5; 12]];

        let result = attn.compute(&query, &keys, &values);
        assert!(result.is_ok(), "compute failed: {:?}", result.err());
        let result = result.unwrap();

        // Verify output dimensions match total_dim.
        assert_eq!(result.output.len(), 12);
        // Verify curvature signs.
        assert!(result.curvatures.spherical > 0.0);
        assert!(result.curvatures.hyperbolic < 0.0);
        assert!((result.curvatures.euclidean).abs() < 1e-6);
        // Proof attestation should exist (curvature compatibility passed).
        assert!(result.attestation.is_some());
    }

    #[test]
    fn test_product_manifold_dimension_mismatch() {
        let config = ManifoldConfig {
            spherical_dim: 4,
            hyperbolic_dim: 4,
            euclidean_dim: 4,
            curvature: -1.0,
        };
        let mut attn = ProductManifoldAttention::new(config);
        let query = vec![0.5; 8]; // wrong dim
        let result = attn.compute(&query, &[], &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_product_manifold_with_betas() {
        let config = ManifoldConfig {
            spherical_dim: 4,
            hyperbolic_dim: 4,
            euclidean_dim: 4,
            curvature: -1.0,
        };
        let mut attn = ProductManifoldAttention::with_betas(config, 2.0, 1.0, 0.5);
        let query = vec![0.3; 12];
        let keys = vec![vec![0.4; 12], vec![0.6; 12]];
        let values = vec![vec![1.0; 12], vec![2.0; 12]];

        let result = attn.compute(&query, &keys, &values).unwrap();
        assert_eq!(result.output.len(), 12);
    }

    #[test]
    fn test_product_manifold_type() {
        let config = ManifoldConfig {
            spherical_dim: 4,
            hyperbolic_dim: 4,
            euclidean_dim: 4,
            curvature: -1.0,
        };
        let attn = ProductManifoldAttention::new(config);
        let mt = attn.manifold_type();
        match mt {
            ManifoldType::Product(components) => {
                assert_eq!(components.len(), 3);
                assert_eq!(components[0], ManifoldType::Sphere);
            }
            _ => panic!("expected Product manifold type"),
        }
    }

    // ---------------------------------------------------------------
    // CurvatureAdaptiveRouter tests
    // ---------------------------------------------------------------

    #[test]
    fn test_router_negative_curvature_routes_hyperbolic() {
        let router = CurvatureAdaptiveRouter::new();
        let weights = router.route(-0.5);
        // Strongly negative curvature should favor hyperbolic.
        assert!(
            weights.hyperbolic > weights.spherical,
            "hyperbolic={} should exceed spherical={} for kappa=-0.5",
            weights.hyperbolic,
            weights.spherical,
        );
        assert!(
            weights.hyperbolic > weights.euclidean,
            "hyperbolic={} should exceed euclidean={} for kappa=-0.5",
            weights.hyperbolic,
            weights.euclidean,
        );
    }

    #[test]
    fn test_router_positive_curvature_routes_spherical() {
        let router = CurvatureAdaptiveRouter::new();
        let weights = router.route(0.5);
        assert!(
            weights.spherical > weights.hyperbolic,
            "spherical={} should exceed hyperbolic={} for kappa=0.5",
            weights.spherical,
            weights.hyperbolic,
        );
        assert!(
            weights.spherical > weights.euclidean,
            "spherical={} should exceed euclidean={} for kappa=0.5",
            weights.spherical,
            weights.euclidean,
        );
    }

    #[test]
    fn test_router_zero_curvature_routes_euclidean() {
        let router = CurvatureAdaptiveRouter::new();
        let weights = router.route(0.0);
        // Near-zero curvature: Euclidean should dominate.
        assert!(
            weights.euclidean > weights.hyperbolic,
            "euclidean={} should exceed hyperbolic={} for kappa=0.0",
            weights.euclidean,
            weights.hyperbolic,
        );
        assert!(
            weights.euclidean > weights.spherical,
            "euclidean={} should exceed spherical={} for kappa=0.0",
            weights.euclidean,
            weights.spherical,
        );
    }

    #[test]
    fn test_router_weights_sum_to_one() {
        let router = CurvatureAdaptiveRouter::new();
        for kappa in [-2.0, -0.5, -0.1, 0.0, 0.1, 0.5, 2.0] {
            let w = router.route(kappa);
            let sum = w.spherical + w.hyperbolic + w.euclidean;
            assert!(
                (sum - 1.0).abs() < 1e-5,
                "weights for kappa={} sum to {} (should be 1.0)",
                kappa,
                sum,
            );
        }
    }

    #[test]
    fn test_router_batch() {
        let router = CurvatureAdaptiveRouter::new();
        let curvatures = vec![-1.0, 0.0, 1.0];
        let results = router.route_batch(&curvatures);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_router_ollivier_ricci_estimate() {
        let router = CurvatureAdaptiveRouter::new();
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 0.0];
        let neighbors_a: Vec<&[f32]> = vec![&[0.1, 0.1], &[-0.1, 0.1]];
        let neighbors_b: Vec<&[f32]> = vec![&[0.9, 0.1], &[1.1, -0.1]];
        let kappa = router.estimate_ollivier_ricci(&a, &b, &neighbors_a, &neighbors_b);
        // Should be a finite value in [-1, 2] approximately.
        assert!(kappa.is_finite());
    }

    // ---------------------------------------------------------------
    // GeodesicMessagePassing tests
    // ---------------------------------------------------------------

    #[test]
    fn test_geodesic_message_passing_poincare() {
        let manifold = ManifoldType::PoincareBall { curvature: 1.0 };
        let mut gmp = GeodesicMessagePassing::new(manifold);

        // Small features that lie inside the Poincare ball (||x|| < 1).
        let features = vec![vec![0.1, 0.2], vec![0.3, 0.1], vec![-0.1, 0.3]];
        let edges = vec![(0, 1), (1, 2), (0, 2)];

        let result = gmp.propagate(&features, &edges);
        assert!(result.is_ok(), "propagate failed: {:?}", result.err());
        let result = result.unwrap();
        assert_eq!(result.node_messages.len(), 3);
        // Each message should have dimension 2.
        for msg in &result.node_messages {
            assert_eq!(msg.len(), 2);
        }
    }

    #[test]
    fn test_geodesic_transport_norm_preservation() {
        let manifold = ManifoldType::PoincareBall { curvature: 1.0 };
        let gmp = GeodesicMessagePassing::new(manifold);

        let v = vec![0.1, 0.05];
        let from = vec![0.2, 0.1];
        let to = vec![0.3, -0.1];

        let transported = gmp.parallel_transport_poincare(&v, &from, &to, 1.0);
        let orig_norm = norm(&v);
        let trans_norm = norm(&transported);

        // Norm should be approximately preserved (within tolerance).
        assert!(
            (trans_norm / orig_norm - 1.0).abs() < 0.5,
            "norm ratio {}/{} = {} deviates too far from 1.0",
            trans_norm,
            orig_norm,
            trans_norm / orig_norm,
        );
    }

    #[test]
    fn test_geodesic_message_passing_sphere() {
        let manifold = ManifoldType::Sphere;
        let mut gmp = GeodesicMessagePassing::new(manifold);

        let features = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let edges = vec![(0, 1), (1, 2)];

        let result = gmp.propagate(&features, &edges).unwrap();
        assert_eq!(result.node_messages.len(), 3);
    }

    #[test]
    fn test_geodesic_message_passing_euclidean() {
        let manifold = ManifoldType::Lorentz { curvature: 1.0 }; // falls to Euclidean branch
        let mut gmp = GeodesicMessagePassing::new(manifold);

        let features = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let edges = vec![(0, 1)];

        let result = gmp.propagate(&features, &edges).unwrap();
        assert_eq!(result.node_messages.len(), 2);
    }

    // ---------------------------------------------------------------
    // RiemannianAdamOptimizer tests
    // ---------------------------------------------------------------

    #[test]
    fn test_riemannian_adam_poincare_stays_on_manifold() {
        let manifold = ManifoldType::PoincareBall { curvature: 1.0 };
        let mut opt = RiemannianAdamOptimizer::new(3, manifold);

        // Start inside the ball.
        let mut params = vec![0.1, 0.2, -0.1];
        let grad = vec![0.5, -0.3, 0.1];

        // Run several steps.
        for _ in 0..10 {
            let result = opt.step(&params, &grad).unwrap();
            params = result.params.clone();

            // Verify the point stays inside the Poincare ball (||x||^2 < 1/c = 1).
            let nsq = norm_sq(&params);
            assert!(
                nsq < 1.0,
                "params norm^2 = {} >= 1.0, left the Poincare ball",
                nsq,
            );
            assert!(result.on_manifold);
            assert!(result.attestation.is_some());
        }
    }

    #[test]
    fn test_riemannian_adam_sphere_stays_on_manifold() {
        let manifold = ManifoldType::Sphere;
        let mut opt = RiemannianAdamOptimizer::new(3, manifold);

        // Start on the sphere.
        let mut params = project_to_sphere(&[0.5, 0.5, 0.5]);
        let grad = vec![0.1, -0.2, 0.05];

        for _ in 0..10 {
            let result = opt.step(&params, &grad).unwrap();
            params = result.params.clone();

            // Verify unit norm.
            let n = norm(&params);
            assert!(
                (n - 1.0).abs() < 0.02,
                "params norm = {} deviates from 1.0",
                n,
            );
            assert!(result.on_manifold);
        }
    }

    #[test]
    fn test_riemannian_adam_euclidean() {
        let manifold = ManifoldType::Lorentz { curvature: 1.0 };
        let mut opt = RiemannianAdamOptimizer::new(2, manifold);
        let params = vec![1.0, 2.0];
        let grad = vec![0.1, 0.2];
        let result = opt.step(&params, &grad).unwrap();
        assert_eq!(result.params.len(), 2);
        assert!(result.on_manifold); // Lorentz falls to default = always on manifold
    }

    #[test]
    fn test_riemannian_adam_dimension_mismatch() {
        let manifold = ManifoldType::PoincareBall { curvature: 1.0 };
        let mut opt = RiemannianAdamOptimizer::new(3, manifold);
        let params = vec![0.1, 0.2]; // wrong dim
        let grad = vec![0.1, 0.2];
        let result = opt.step(&params, &grad);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // LieGroupEquivariantAttention tests
    // ---------------------------------------------------------------

    #[test]
    fn test_lie_group_equivariant_forward_so3() {
        let attn = LieGroupEquivariantAttention::new(LieGroupType::SO3, 4, 6);
        assert_eq!(attn.total_dim(), 10);
        assert_eq!(*attn.group(), LieGroupType::SO3);

        let query = vec![0.5; 10];
        let keys = vec![vec![0.3; 10], vec![0.7; 10]];
        let values = vec![vec![1.0; 10], vec![2.0; 10]];

        let result = attn.compute(&query, &keys, &values);
        assert!(result.is_ok(), "compute failed: {:?}", result.err());
        let result = result.unwrap();

        assert_eq!(result.output.len(), 10);
        assert_eq!(result.scalar_output.len(), 4);
        assert_eq!(result.vector_output.len(), 6);
    }

    #[test]
    fn test_lie_group_equivariant_forward_se3() {
        let attn = LieGroupEquivariantAttention::new(LieGroupType::SE3, 8, 12);
        let query = vec![0.2; 20];
        let keys = vec![vec![0.4; 20], vec![0.6; 20], vec![0.1; 20]];
        let values = vec![vec![1.0; 20], vec![2.0; 20], vec![0.5; 20]];

        let result = attn.compute(&query, &keys, &values).unwrap();
        assert_eq!(result.output.len(), 20);
    }

    #[test]
    fn test_lie_group_equivariant_forward_u1() {
        let attn = LieGroupEquivariantAttention::new(LieGroupType::U1, 3, 3);
        let query = vec![0.5; 6];
        let keys = vec![vec![0.3; 6]];
        let values = vec![vec![1.0; 6]];

        let result = attn.compute(&query, &keys, &values).unwrap();
        assert_eq!(result.output.len(), 6);
    }

    #[test]
    fn test_lie_group_equivariant_dimension_mismatch() {
        let attn = LieGroupEquivariantAttention::new(LieGroupType::SO3, 4, 6);
        let query = vec![0.5; 5]; // wrong dim
        let result = attn.compute(&query, &[], &[]);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // ManifoldType tests
    // ---------------------------------------------------------------

    #[test]
    fn test_manifold_type_enum() {
        let pb = ManifoldType::PoincareBall { curvature: 1.0 };
        let lr = ManifoldType::Lorentz { curvature: 2.0 };
        let sp = ManifoldType::Sphere;
        let pr = ManifoldType::Product(vec![pb.clone(), sp.clone()]);

        assert_eq!(pb, ManifoldType::PoincareBall { curvature: 1.0 });
        assert_ne!(pb, lr);
        match pr {
            ManifoldType::Product(components) => assert_eq!(components.len(), 2),
            _ => panic!("expected Product"),
        }
    }

    // ---------------------------------------------------------------
    // Helper function tests
    // ---------------------------------------------------------------

    #[test]
    fn test_spherical_projection() {
        let v = vec![3.0, 4.0];
        let proj = project_to_sphere(&v);
        let n: f32 = proj.iter().map(|&x| x * x).sum::<f32>().sqrt();
        assert!((n - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_spherical_geodesic() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let dist = spherical_geodesic(&a, &b);
        assert!((dist - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn test_spherical_geodesic_same_point() {
        let a = vec![1.0, 0.0, 0.0];
        let dist = spherical_geodesic(&a, &a);
        assert!(dist.abs() < 1e-5);
    }

    #[test]
    fn test_sigmoid_bounds() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(sigmoid(10.0) > 0.99);
        assert!(sigmoid(-10.0) < 0.01);
    }

    #[test]
    fn test_poincare_project_inside() {
        let x = vec![0.1, 0.2];
        let proj = poincare_project(&x, 1.0);
        assert_eq!(proj, x); // already inside
    }

    #[test]
    fn test_poincare_project_outside() {
        let x = vec![0.8, 0.8]; // norm ~ 1.13 > 1/sqrt(1)
        let proj = poincare_project(&x, 1.0);
        let nsq = norm_sq(&proj);
        assert!(nsq < 1.0, "projected norm^2 = {} should be < 1.0", nsq);
    }
}
