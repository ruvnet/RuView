//! Node.js bindings for RuVector Graph Transformer via NAPI-RS
//!
//! Exposes proof-gated operations, sublinear attention, physics-informed
//! layers, biological-inspired learning, verified training, manifold
//! distance, temporal causal attention, and economic game-theoretic
//! attention to Node.js applications.
//!
//! This crate embeds a self-contained graph transformer implementation
//! to avoid coupling with the evolving `ruvector-graph-transformer` crate.

#![deny(clippy::all)]

mod transformer;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use transformer::{CoreGraphTransformer, Edge as CoreEdge, PipelineStage as CorePipelineStage};

/// Graph Transformer with proof-gated operations for Node.js.
///
/// Provides sublinear attention over graph structures, physics-informed
/// layers (Hamiltonian dynamics), biologically-inspired learning (spiking
/// networks, Hebbian plasticity), and verified training with proof receipts.
///
/// # Example
/// ```javascript
/// const { GraphTransformer } = require('ruvector-graph-transformer-node');
/// const gt = new GraphTransformer();
/// console.log(gt.version());
/// ```
#[napi]
pub struct GraphTransformer {
    inner: CoreGraphTransformer,
}

#[napi]
impl GraphTransformer {
    /// Create a new Graph Transformer instance.
    ///
    /// # Arguments
    /// * `config` - Optional JSON configuration (reserved for future use)
    ///
    /// # Example
    /// ```javascript
    /// const gt = new GraphTransformer();
    /// const gt2 = new GraphTransformer({ maxFuel: 10000 });
    /// ```
    #[napi(constructor)]
    pub fn new(_config: Option<serde_json::Value>) -> Self {
        Self {
            inner: CoreGraphTransformer::new(),
        }
    }

    /// Get the library version string.
    ///
    /// # Example
    /// ```javascript
    /// console.log(gt.version()); // "2.0.4"
    /// ```
    #[napi]
    pub fn version(&self) -> String {
        self.inner.version()
    }

    // ===================================================================
    // Proof-Gated Operations
    // ===================================================================

    /// Create a proof gate for a given dimension.
    ///
    /// Returns a JSON object describing the gate (id, dimension, verified).
    ///
    /// # Arguments
    /// * `dim` - The dimension to gate on
    ///
    /// # Example
    /// ```javascript
    /// const gate = gt.createProofGate(128);
    /// console.log(gate.dimension); // 128
    /// ```
    #[napi]
    pub fn create_proof_gate(&mut self, dim: u32) -> Result<serde_json::Value> {
        let gate = self.inner.create_proof_gate(dim);
        serde_json::to_value(&gate).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    /// Prove that two dimensions are equal.
    ///
    /// Returns a proof result with proof_id, expected, actual, and verified fields.
    ///
    /// # Arguments
    /// * `expected` - The expected dimension
    /// * `actual` - The actual dimension
    ///
    /// # Example
    /// ```javascript
    /// const proof = gt.proveDimension(128, 128);
    /// console.log(proof.verified); // true
    /// ```
    #[napi]
    pub fn prove_dimension(&mut self, expected: u32, actual: u32) -> Result<serde_json::Value> {
        let result = self
            .inner
            .prove_dimension(expected, actual)
            .map_err(|e| Error::new(Status::GenericFailure, format!("{}", e)))?;
        serde_json::to_value(&result).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    /// Create a proof attestation (serializable receipt) for a given proof ID.
    ///
    /// Returns the attestation as a byte buffer (82 bytes) that can be
    /// embedded in RVF WITNESS_SEG entries.
    ///
    /// # Arguments
    /// * `proof_id` - The proof term ID to create an attestation for
    ///
    /// # Example
    /// ```javascript
    /// const proof = gt.proveDimension(64, 64);
    /// const attestation = gt.createAttestation(proof.proof_id);
    /// console.log(attestation.length); // 82
    /// ```
    #[napi]
    pub fn create_attestation(&self, proof_id: u32) -> Result<Vec<u8>> {
        let att = self.inner.create_attestation(proof_id);
        Ok(att.to_bytes())
    }

    /// Compose a chain of pipeline stages, verifying type compatibility.
    ///
    /// Each stage must have `name`, `input_type_id`, and `output_type_id`.
    /// Returns a composed proof with the overall input/output types and
    /// the number of stages verified.
    ///
    /// # Arguments
    /// * `stages` - Array of stage descriptors as JSON objects
    ///
    /// # Example
    /// ```javascript
    /// const composed = gt.composeProofs([
    ///   { name: 'embed', input_type_id: 1, output_type_id: 2 },
    ///   { name: 'align', input_type_id: 2, output_type_id: 3 },
    /// ]);
    /// console.log(composed.chain_name); // "embed >> align"
    /// ```
    #[napi]
    pub fn compose_proofs(&mut self, stages: Vec<serde_json::Value>) -> Result<serde_json::Value> {
        let rust_stages: Vec<CorePipelineStage> = stages
            .into_iter()
            .map(|v| {
                serde_json::from_value(v).map_err(|e| {
                    Error::new(
                        Status::InvalidArg,
                        format!("Invalid stage descriptor: {}", e),
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let result = self
            .inner
            .compose_proofs(&rust_stages)
            .map_err(|e| Error::new(Status::GenericFailure, format!("{}", e)))?;

        serde_json::to_value(&result).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    /// Verify an attestation from its byte representation.
    ///
    /// Returns `true` if the attestation is structurally valid.
    ///
    /// # Arguments
    /// * `bytes` - The attestation bytes (82 bytes minimum)
    ///
    /// # Example
    /// ```javascript
    /// const valid = gt.verifyAttestation(attestationBytes);
    /// ```
    #[napi]
    pub fn verify_attestation(&self, bytes: Vec<u8>) -> bool {
        self.inner.verify_attestation(&bytes)
    }

    // ===================================================================
    // Sublinear Attention
    // ===================================================================

    /// Sublinear graph attention using personalized PageRank sparsification.
    ///
    /// Instead of attending to all N nodes (O(N*d)), uses PPR to select
    /// the top-k most relevant nodes, achieving O(k*d) complexity.
    ///
    /// # Arguments
    /// * `query` - Query vector (length must equal `dim`)
    /// * `edges` - Adjacency list: edges[i] is the list of neighbor indices for node i
    /// * `dim` - Dimension of the query vector
    /// * `k` - Number of top nodes to attend to
    ///
    /// # Returns
    /// JSON object with `scores`, `top_k_indices`, and `sparsity_ratio`
    ///
    /// # Example
    /// ```javascript
    /// const result = gt.sublinearAttention([1.0, 0.5], [[1, 2], [0, 2], [0, 1]], 2, 2);
    /// console.log(result.top_k_indices);
    /// ```
    #[napi]
    pub fn sublinear_attention(
        &mut self,
        query: Vec<f64>,
        edges: Vec<Vec<u32>>,
        dim: u32,
        k: u32,
    ) -> Result<serde_json::Value> {
        let result = self
            .inner
            .sublinear_attention(&query, &edges, dim, k)
            .map_err(|e| Error::new(Status::GenericFailure, format!("{}", e)))?;

        serde_json::to_value(&result).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    /// Compute personalized PageRank scores from a source node.
    ///
    /// # Arguments
    /// * `source` - Source node index
    /// * `adjacency` - Adjacency list for the graph
    /// * `alpha` - Teleport probability (typically 0.15)
    ///
    /// # Returns
    /// Array of PPR scores, one per node
    ///
    /// # Example
    /// ```javascript
    /// const scores = gt.pprScores(0, [[1], [0, 2], [1]], 0.15);
    /// ```
    #[napi]
    pub fn ppr_scores(
        &mut self,
        source: u32,
        adjacency: Vec<Vec<u32>>,
        alpha: f64,
    ) -> Result<Vec<f64>> {
        Ok(self.inner.ppr_scores(source, &adjacency, alpha))
    }

    // ===================================================================
    // Physics-Informed Layers
    // ===================================================================

    /// Symplectic integrator step (leapfrog / Stormer-Verlet).
    ///
    /// Integrates Hamiltonian dynamics with a harmonic potential V(q) = 0.5*|q|^2,
    /// preserving the symplectic structure (energy-conserving).
    ///
    /// # Arguments
    /// * `positions` - Position coordinates
    /// * `momenta` - Momentum coordinates (same length as positions)
    /// * `dt` - Time step
    ///
    /// # Returns
    /// JSON object with `positions`, `momenta`, and `energy`
    ///
    /// # Example
    /// ```javascript
    /// const state = gt.hamiltonianStep([1.0, 0.0], [0.0, 1.0], 0.01);
    /// console.log(state.energy);
    /// ```
    #[napi]
    pub fn hamiltonian_step(
        &mut self,
        positions: Vec<f64>,
        momenta: Vec<f64>,
        dt: f64,
    ) -> Result<serde_json::Value> {
        let result = self
            .inner
            .hamiltonian_step(&positions, &momenta, dt)
            .map_err(|e| Error::new(Status::GenericFailure, format!("{}", e)))?;

        serde_json::to_value(&result).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    /// Hamiltonian step with graph edge interactions.
    ///
    /// `positions` and `momenta` are arrays of coordinates. `edges` is an
    /// array of `{ src, tgt }` objects defining graph interactions.
    ///
    /// # Returns
    /// JSON object with `positions`, `momenta`, `energy`, and `energy_conserved`
    ///
    /// # Example
    /// ```javascript
    /// const state = gt.hamiltonianStepGraph(
    ///   [1.0, 0.0], [0.0, 1.0],
    ///   [{ src: 0, tgt: 1 }], 0.01
    /// );
    /// ```
    #[napi]
    pub fn hamiltonian_step_graph(
        &mut self,
        positions: Vec<f64>,
        momenta: Vec<f64>,
        edges: Vec<serde_json::Value>,
        dt: f64,
    ) -> Result<serde_json::Value> {
        let rust_edges: Vec<CoreEdge> = edges
            .into_iter()
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid edge: {}", e)))
            })
            .collect::<Result<Vec<_>>>()?;

        let result = self
            .inner
            .hamiltonian_step_graph(&positions, &momenta, &rust_edges, dt)
            .map_err(|e| Error::new(Status::GenericFailure, format!("{}", e)))?;

        serde_json::to_value(&result).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    // ===================================================================
    // Biological-Inspired
    // ===================================================================

    /// Spiking neural attention: event-driven sparse attention.
    ///
    /// Nodes emit attention only when their membrane potential exceeds
    /// a threshold, producing sparse activation patterns.
    ///
    /// # Arguments
    /// * `spikes` - Membrane potentials for each node
    /// * `edges` - Adjacency list for the graph
    /// * `threshold` - Firing threshold
    ///
    /// # Returns
    /// Output activation vector (one value per node)
    ///
    /// # Example
    /// ```javascript
    /// const output = gt.spikingAttention([0.5, 1.5, 0.3], [[1], [0, 2], [1]], 1.0);
    /// ```
    #[napi]
    pub fn spiking_attention(
        &mut self,
        spikes: Vec<f64>,
        edges: Vec<Vec<u32>>,
        threshold: f64,
    ) -> Result<Vec<f64>> {
        Ok(self.inner.spiking_attention(&spikes, &edges, threshold))
    }

    /// Hebbian learning rule update.
    ///
    /// Applies the outer-product Hebbian rule: w_ij += lr * pre_i * post_j.
    /// The weight vector is a flattened (pre.len * post.len) matrix.
    ///
    /// # Arguments
    /// * `pre` - Pre-synaptic activations
    /// * `post` - Post-synaptic activations
    /// * `weights` - Current weight vector (flattened matrix)
    /// * `lr` - Learning rate
    ///
    /// # Returns
    /// Updated weight vector
    ///
    /// # Example
    /// ```javascript
    /// const updated = gt.hebbianUpdate([1.0, 0.0], [0.0, 1.0], [0, 0, 0, 0], 0.1);
    /// ```
    #[napi]
    pub fn hebbian_update(
        &mut self,
        pre: Vec<f64>,
        post: Vec<f64>,
        weights: Vec<f64>,
        lr: f64,
    ) -> Result<Vec<f64>> {
        Ok(self.inner.hebbian_update(&pre, &post, &weights, lr))
    }

    /// Spiking step over 2D node features with adjacency matrix.
    ///
    /// `features` is an array of arrays (n x dim). `adjacency` is a flat
    /// row-major array (n x n). Returns `{ features, spikes, weights }`.
    ///
    /// # Example
    /// ```javascript
    /// const result = gt.spikingStep(
    ///   [[0.8, 0.6], [0.1, 0.2]],
    ///   [0, 0.5, 0.3, 0]
    /// );
    /// ```
    #[napi]
    pub fn spiking_step(
        &mut self,
        features: Vec<Vec<f64>>,
        adjacency: Vec<f64>,
    ) -> Result<serde_json::Value> {
        let result = self.inner.spiking_step(&features, &adjacency, 1.0);
        serde_json::to_value(&result).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    // ===================================================================
    // Verified Training
    // ===================================================================

    /// A single verified SGD step with proof of gradient application.
    ///
    /// Applies w' = w - lr * grad and returns the new weights along with
    /// a proof receipt, loss before/after, and gradient norm.
    ///
    /// # Arguments
    /// * `weights` - Current weight vector
    /// * `gradients` - Gradient vector (same length as weights)
    /// * `lr` - Learning rate
    ///
    /// # Returns
    /// JSON object with `weights`, `proof_id`, `loss_before`, `loss_after`, `gradient_norm`
    ///
    /// # Example
    /// ```javascript
    /// const result = gt.verifiedStep([1.0, 2.0], [0.1, 0.2], 0.01);
    /// console.log(result.loss_after < result.loss_before); // true
    /// ```
    #[napi]
    pub fn verified_step(
        &mut self,
        weights: Vec<f64>,
        gradients: Vec<f64>,
        lr: f64,
    ) -> Result<serde_json::Value> {
        let result = self
            .inner
            .verified_step(&weights, &gradients, lr)
            .map_err(|e| Error::new(Status::GenericFailure, format!("{}", e)))?;

        serde_json::to_value(&result).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    /// Verified training step with features, targets, and weights.
    ///
    /// Computes MSE loss, applies SGD, and produces a training certificate.
    ///
    /// # Arguments
    /// * `features` - Input feature vector
    /// * `targets` - Target values
    /// * `weights` - Current weight vector
    ///
    /// # Returns
    /// JSON object with `weights`, `certificate_id`, `loss`,
    /// `loss_monotonic`, `lipschitz_satisfied`
    ///
    /// # Example
    /// ```javascript
    /// const result = gt.verifiedTrainingStep([1.0, 2.0], [0.5, 1.0], [0.5, 0.5]);
    /// ```
    #[napi]
    pub fn verified_training_step(
        &mut self,
        features: Vec<f64>,
        targets: Vec<f64>,
        weights: Vec<f64>,
    ) -> Result<serde_json::Value> {
        let result = self
            .inner
            .verified_training_step(&features, &targets, &weights, 0.001)
            .map_err(|e| Error::new(Status::GenericFailure, format!("{}", e)))?;

        serde_json::to_value(&result).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    // ===================================================================
    // Manifold
    // ===================================================================

    /// Product manifold distance (mixed curvature spaces).
    ///
    /// Splits vectors into sub-spaces according to the curvatures array:
    /// - curvature > 0: spherical distance
    /// - curvature < 0: hyperbolic distance
    /// - curvature == 0: Euclidean distance
    ///
    /// # Arguments
    /// * `a` - First point
    /// * `b` - Second point (same length as `a`)
    /// * `curvatures` - Curvature for each sub-space
    ///
    /// # Returns
    /// The product manifold distance as a number
    ///
    /// # Example
    /// ```javascript
    /// const d = gt.productManifoldDistance([1, 0, 0, 1], [0, 1, 1, 0], [0.0, -1.0]);
    /// ```
    #[napi]
    pub fn product_manifold_distance(&self, a: Vec<f64>, b: Vec<f64>, curvatures: Vec<f64>) -> f64 {
        self.inner.product_manifold_distance(&a, &b, &curvatures)
    }

    /// Product manifold attention with mixed curvatures.
    ///
    /// Computes attention in a product of spherical, hyperbolic, and
    /// Euclidean subspaces, combining the results.
    ///
    /// # Arguments
    /// * `features` - Input feature vector
    /// * `edges` - Array of `{ src, tgt }` objects
    ///
    /// # Returns
    /// JSON object with `output`, `curvatures`, `distances`
    ///
    /// # Example
    /// ```javascript
    /// const result = gt.productManifoldAttention(
    ///   [1.0, 0.5, -0.3, 0.8],
    ///   [{ src: 0, tgt: 1 }]
    /// );
    /// ```
    #[napi]
    pub fn product_manifold_attention(
        &mut self,
        features: Vec<f64>,
        edges: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let rust_edges: Vec<CoreEdge> = edges
            .into_iter()
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid edge: {}", e)))
            })
            .collect::<Result<Vec<_>>>()?;

        let curvatures = vec![0.0, -1.0]; // default mixed curvatures
        let result = self
            .inner
            .product_manifold_attention(&features, &rust_edges, &curvatures);

        serde_json::to_value(&result).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    // ===================================================================
    // Temporal
    // ===================================================================

    /// Causal attention with temporal ordering.
    ///
    /// Attention scores are masked so that a key at time t_j can only
    /// attend to queries at time t_i <= t_j (no information leakage
    /// from the future).
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `keys` - Array of key vectors
    /// * `timestamps` - Timestamp for each key (same length as keys)
    ///
    /// # Returns
    /// Softmax attention weights (one per key, sums to 1.0)
    ///
    /// # Example
    /// ```javascript
    /// const scores = gt.causalAttention(
    ///   [1.0, 0.0],
    ///   [[1.0, 0.0], [0.0, 1.0], [0.5, 0.5]],
    ///   [1.0, 2.0, 3.0]
    /// );
    /// ```
    #[napi]
    pub fn causal_attention(
        &mut self,
        query: Vec<f64>,
        keys: Vec<Vec<f64>>,
        timestamps: Vec<f64>,
    ) -> Result<Vec<f64>> {
        Ok(self.inner.causal_attention(&query, &keys, &timestamps))
    }

    /// Causal attention over features, timestamps, and graph edges.
    ///
    /// Returns attention-weighted output features where each node can
    /// only attend to neighbors with earlier or equal timestamps.
    ///
    /// # Arguments
    /// * `features` - Feature value for each node
    /// * `timestamps` - Timestamp for each node
    /// * `edges` - Array of `{ src, tgt }` objects
    ///
    /// # Returns
    /// Array of attention-weighted output values
    ///
    /// # Example
    /// ```javascript
    /// const output = gt.causalAttentionGraph(
    ///   [1.0, 0.5, 0.8],
    ///   [1.0, 2.0, 3.0],
    ///   [{ src: 0, tgt: 1 }, { src: 1, tgt: 2 }]
    /// );
    /// ```
    #[napi]
    pub fn causal_attention_graph(
        &mut self,
        features: Vec<f64>,
        timestamps: Vec<f64>,
        edges: Vec<serde_json::Value>,
    ) -> Result<Vec<f64>> {
        let rust_edges: Vec<CoreEdge> = edges
            .into_iter()
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid edge: {}", e)))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(self
            .inner
            .causal_attention_graph(&features, &timestamps, &rust_edges))
    }

    /// Extract Granger causality DAG from attention history.
    ///
    /// Tests pairwise Granger causality between all nodes and returns
    /// edges where the F-statistic exceeds the significance threshold.
    ///
    /// # Arguments
    /// * `attention_history` - Flat array (T x N, row-major)
    /// * `num_nodes` - Number of nodes N
    /// * `num_steps` - Number of time steps T
    ///
    /// # Returns
    /// JSON object with `edges` and `num_nodes`
    ///
    /// # Example
    /// ```javascript
    /// const dag = gt.grangerExtract(flatHistory, 3, 20);
    /// console.log(dag.edges); // [{ source, target, f_statistic, is_causal }]
    /// ```
    #[napi]
    pub fn granger_extract(
        &mut self,
        attention_history: Vec<f64>,
        num_nodes: u32,
        num_steps: u32,
    ) -> Result<serde_json::Value> {
        let dag = self
            .inner
            .granger_extract(&attention_history, num_nodes, num_steps);

        serde_json::to_value(&dag).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    // ===================================================================
    // Economic / Game-Theoretic
    // ===================================================================

    /// Game-theoretic attention: computes Nash equilibrium allocations.
    ///
    /// Each node is a player with features as utility parameters. Edges
    /// define strategic interactions. Uses best-response iteration to
    /// converge to Nash equilibrium.
    ///
    /// # Arguments
    /// * `features` - Feature/utility value for each node
    /// * `edges` - Array of `{ src, tgt }` objects
    ///
    /// # Returns
    /// JSON object with `allocations`, `utilities`, `nash_gap`, `converged`
    ///
    /// # Example
    /// ```javascript
    /// const result = gt.gameTheoreticAttention(
    ///   [1.0, 0.5, 0.8],
    ///   [{ src: 0, tgt: 1 }, { src: 1, tgt: 2 }]
    /// );
    /// console.log(result.converged); // true
    /// ```
    #[napi]
    pub fn game_theoretic_attention(
        &mut self,
        features: Vec<f64>,
        edges: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let rust_edges: Vec<CoreEdge> = edges
            .into_iter()
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|e| Error::new(Status::InvalidArg, format!("Invalid edge: {}", e)))
            })
            .collect::<Result<Vec<_>>>()?;

        let result = self.inner.game_theoretic_attention(&features, &rust_edges);

        serde_json::to_value(&result).map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Serialization error: {}", e),
            )
        })
    }

    // ===================================================================
    // Stats
    // ===================================================================

    /// Get aggregate statistics as a JSON object.
    ///
    /// # Example
    /// ```javascript
    /// const stats = gt.stats();
    /// console.log(stats.proofs_verified);
    /// ```
    #[napi]
    pub fn stats(&self) -> serde_json::Value {
        serde_json::to_value(self.inner.stats()).unwrap_or(serde_json::Value::Null)
    }

    /// Reset all internal state (caches, counters, gates).
    ///
    /// # Example
    /// ```javascript
    /// gt.reset();
    /// ```
    #[napi]
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

/// Get the library version.
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Module initialization message.
#[napi]
pub fn init() -> String {
    "RuVector Graph Transformer Node.js bindings initialized".to_string()
}
