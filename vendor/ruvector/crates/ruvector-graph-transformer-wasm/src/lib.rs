//! WASM bindings for `ruvector-graph-transformer`: proof-gated graph attention
//! in the browser.
//!
//! # Quick Start (JavaScript)
//!
//! ```js
//! import init, { JsGraphTransformer } from "ruvector-graph-transformer-wasm";
//!
//! await init();
//! const gt = new JsGraphTransformer();
//!
//! // Create a proof gate and prove dimensions
//! const gate = gt.createProofGate(128);
//! const proof = gt.proveDimension(128, 128);
//!
//! // Sublinear attention
//! const result = gt.sublinearAttention(
//!     new Float64Array([0.1, 0.2]),
//!     [[1, 2], [0, 2], [0, 1]],
//!     2, 2,
//! );
//!
//! // Physics: Hamiltonian step with graph edges
//! const state = gt.hamiltonianStep([1.0, 0.0], [0.0, 1.0], [{ src: 0, tgt: 1 }]);
//!
//! // Biological: spiking step
//! const spikes = gt.spikingStep([[0.8, 0.6], [0.1, 0.2]], [0, 0.5, 0.3, 0]);
//!
//! // Temporal: causal attention
//! const attn = gt.causalAttention([1.0, 0.0], [1.0, 2.0, 3.0], [{ src: 0, tgt: 1 }]);
//!
//! // Manifold: product manifold attention
//! const manifold = gt.productManifoldAttention([1.0, 0.5], [{ src: 0, tgt: 1 }]);
//!
//! // Verified training
//! const training = gt.verifiedTrainingStep([1.0, 2.0], [0.5, 1.0], [0.5, 0.5]);
//!
//! // Economic: game-theoretic attention
//! const eqm = gt.gameTheoreticAttention([1.0, 0.5, 0.8], [{ src: 0, tgt: 1 }]);
//!
//! // Stats
//! console.log(gt.stats());
//! ```

mod transformer;
mod utils;

use transformer::{CoreGraphTransformer, Edge, PipelineStage as CorePipelineStage};
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// Module init
// ---------------------------------------------------------------------------

/// Called automatically when the WASM module is loaded.
#[wasm_bindgen(start)]
pub fn init() {
    utils::set_panic_hook();
}

/// Return the crate version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// ---------------------------------------------------------------------------
// JsGraphTransformer -- main entry point
// ---------------------------------------------------------------------------

/// Graph transformer for the browser.
///
/// Wraps the core `CoreGraphTransformer` and exposes proof-gated, sublinear,
/// physics, biological, verified-training, manifold, temporal, and economic
/// operations via wasm_bindgen.
#[wasm_bindgen]
pub struct JsGraphTransformer {
    inner: CoreGraphTransformer,
}

#[wasm_bindgen]
impl JsGraphTransformer {
    /// Create a new graph transformer.
    ///
    /// `config` is an optional JS object (reserved for future use).
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<JsGraphTransformer, JsError> {
        let _ = config; // reserved for future configuration
        Ok(Self {
            inner: CoreGraphTransformer::new(),
        })
    }

    /// Get the library version string.
    #[wasm_bindgen]
    pub fn version(&self) -> String {
        self.inner.version()
    }

    // ===================================================================
    // Proof-Gated Operations
    // ===================================================================

    /// Create a proof gate for the given embedding dimension.
    ///
    /// Returns a serialized `ProofGate` object.
    pub fn create_proof_gate(&mut self, dim: u32) -> Result<JsValue, JsError> {
        let gate = self.inner.create_proof_gate(dim);
        serde_wasm_bindgen::to_value(&gate).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Prove that two dimensions are equal.
    ///
    /// Returns `{ proof_id, expected, actual, verified }`.
    pub fn prove_dimension(&mut self, expected: u32, actual: u32) -> Result<JsValue, JsError> {
        let result = self
            .inner
            .prove_dimension(expected, actual)
            .map_err(|e| JsError::new(&format!("{e}")))?;
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Create a proof attestation for a given proof ID.
    ///
    /// Returns the attestation as a byte buffer (82 bytes).
    pub fn create_attestation(&self, proof_id: u32) -> Result<Vec<u8>, JsError> {
        let att = self.inner.create_attestation(proof_id);
        Ok(att.to_bytes())
    }

    /// Verify an attestation from its byte representation.
    ///
    /// Returns `true` if the attestation is structurally valid.
    pub fn verify_attestation(&self, bytes: &[u8]) -> bool {
        self.inner.verify_attestation(bytes)
    }

    /// Compose a chain of pipeline stages, verifying type compatibility.
    ///
    /// `stages` is a JS array of `{ name, input_type_id, output_type_id }`.
    /// Returns a composed proof with the overall input/output types.
    pub fn compose_proofs(&mut self, stages: JsValue) -> Result<JsValue, JsError> {
        let rust_stages: Vec<CorePipelineStage> = serde_wasm_bindgen::from_value(stages)
            .map_err(|e| JsError::new(&format!("invalid stages: {e}")))?;
        let result = self
            .inner
            .compose_proofs(&rust_stages)
            .map_err(|e| JsError::new(&format!("{e}")))?;
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    // ===================================================================
    // Sublinear Attention
    // ===================================================================

    /// Sublinear graph attention using personalized PageRank sparsification.
    ///
    /// `query` is a Float64Array, `edges` is `[[u32, ...], ...]`.
    /// Returns `{ scores, top_k_indices, sparsity_ratio }`.
    pub fn sublinear_attention(
        &mut self,
        query: JsValue,
        edges: JsValue,
        dim: u32,
        k: u32,
    ) -> Result<JsValue, JsError> {
        let q: Vec<f64> = serde_wasm_bindgen::from_value(query)
            .map_err(|e| JsError::new(&format!("invalid query: {e}")))?;
        let ed: Vec<Vec<u32>> = serde_wasm_bindgen::from_value(edges)
            .map_err(|e| JsError::new(&format!("invalid edges: {e}")))?;
        let result = self
            .inner
            .sublinear_attention(&q, &ed, dim, k)
            .map_err(|e| JsError::new(&format!("{e}")))?;
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Compute personalized PageRank scores from a source node.
    ///
    /// Returns array of PPR scores, one per node.
    pub fn ppr_scores(
        &mut self,
        source: u32,
        adjacency: JsValue,
        alpha: f64,
    ) -> Result<JsValue, JsError> {
        let adj: Vec<Vec<u32>> = serde_wasm_bindgen::from_value(adjacency)
            .map_err(|e| JsError::new(&format!("invalid adjacency: {e}")))?;
        let scores = self.inner.ppr_scores(source, &adj, alpha);
        serde_wasm_bindgen::to_value(&scores).map_err(|e| JsError::new(&e.to_string()))
    }

    // ===================================================================
    // Physics-Informed Layers
    // ===================================================================

    /// Symplectic integrator step (leapfrog / Stormer-Verlet).
    ///
    /// `positions` and `momenta` are Float64Arrays, `edges` is
    /// `[{ src, tgt }, ...]`. Returns `{ positions, momenta, energy,
    /// energy_conserved }`.
    pub fn hamiltonian_step(
        &mut self,
        positions: JsValue,
        momenta: JsValue,
        edges: JsValue,
    ) -> Result<JsValue, JsError> {
        let pos: Vec<f64> = serde_wasm_bindgen::from_value(positions)
            .map_err(|e| JsError::new(&format!("invalid positions: {e}")))?;
        let mom: Vec<f64> = serde_wasm_bindgen::from_value(momenta)
            .map_err(|e| JsError::new(&format!("invalid momenta: {e}")))?;
        let ed: Vec<Edge> = serde_wasm_bindgen::from_value(edges)
            .map_err(|e| JsError::new(&format!("invalid edges: {e}")))?;
        let result = self
            .inner
            .hamiltonian_step_graph(&pos, &mom, &ed, 0.01)
            .map_err(|e| JsError::new(&format!("{e}")))?;
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Verify energy conservation between two states.
    ///
    /// Returns `{ conserved, delta, relative_error }`.
    pub fn verify_energy_conservation(
        &self,
        before: f64,
        after: f64,
        tolerance: f64,
    ) -> Result<JsValue, JsError> {
        let v = self
            .inner
            .verify_energy_conservation(before, after, tolerance);
        serde_wasm_bindgen::to_value(&v).map_err(|e| JsError::new(&e.to_string()))
    }

    // ===================================================================
    // Biological-Inspired
    // ===================================================================

    /// Spiking neural attention step over 2D features with adjacency.
    ///
    /// `features` is `[[f64, ...], ...]`, `adjacency` is a flat row-major
    /// Float64Array (n x n). Returns `{ features, spikes, weights }`.
    pub fn spiking_step(
        &mut self,
        features: JsValue,
        adjacency: JsValue,
    ) -> Result<JsValue, JsError> {
        let feats: Vec<Vec<f64>> = serde_wasm_bindgen::from_value(features)
            .map_err(|e| JsError::new(&format!("invalid features: {e}")))?;
        let adj: Vec<f64> = serde_wasm_bindgen::from_value(adjacency)
            .map_err(|e| JsError::new(&format!("invalid adjacency: {e}")))?;
        let result = self.inner.spiking_step(&feats, &adj, 1.0);
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Hebbian weight update.
    ///
    /// `pre`, `post`, `weights` are Float64Arrays. Returns updated weights.
    pub fn hebbian_update(
        &mut self,
        pre: JsValue,
        post: JsValue,
        weights: JsValue,
    ) -> Result<JsValue, JsError> {
        let pre_v: Vec<f64> = serde_wasm_bindgen::from_value(pre)
            .map_err(|e| JsError::new(&format!("invalid pre: {e}")))?;
        let post_v: Vec<f64> = serde_wasm_bindgen::from_value(post)
            .map_err(|e| JsError::new(&format!("invalid post: {e}")))?;
        let w: Vec<f64> = serde_wasm_bindgen::from_value(weights)
            .map_err(|e| JsError::new(&format!("invalid weights: {e}")))?;
        let result = self.inner.hebbian_update(&pre_v, &post_v, &w, 0.01);
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    // ===================================================================
    // Temporal
    // ===================================================================

    /// Causal attention with temporal ordering over graph edges.
    ///
    /// `features` is a Float64Array, `timestamps` is a Float64Array,
    /// `edges` is `[{ src, tgt }, ...]`.
    /// Returns attention-weighted output features.
    pub fn causal_attention(
        &mut self,
        features: JsValue,
        timestamps: JsValue,
        edges: JsValue,
    ) -> Result<JsValue, JsError> {
        let feats: Vec<f64> = serde_wasm_bindgen::from_value(features)
            .map_err(|e| JsError::new(&format!("invalid features: {e}")))?;
        let ts: Vec<f64> = serde_wasm_bindgen::from_value(timestamps)
            .map_err(|e| JsError::new(&format!("invalid timestamps: {e}")))?;
        let ed: Vec<Edge> = serde_wasm_bindgen::from_value(edges)
            .map_err(|e| JsError::new(&format!("invalid edges: {e}")))?;
        let result = self.inner.causal_attention_graph(&feats, &ts, &ed);
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Extract Granger causality DAG from attention history.
    ///
    /// `attention_history` is a flat Float64Array (T x N row-major).
    /// Returns `{ edges: [{ source, target, f_statistic, is_causal }], num_nodes }`.
    pub fn granger_extract(
        &mut self,
        attention_history: JsValue,
        num_nodes: u32,
        num_steps: u32,
    ) -> Result<JsValue, JsError> {
        let hist: Vec<f64> = serde_wasm_bindgen::from_value(attention_history)
            .map_err(|e| JsError::new(&format!("invalid attention_history: {e}")))?;
        let dag = self.inner.granger_extract(&hist, num_nodes, num_steps);
        serde_wasm_bindgen::to_value(&dag).map_err(|e| JsError::new(&e.to_string()))
    }

    // ===================================================================
    // Manifold
    // ===================================================================

    /// Product manifold attention with mixed curvatures.
    ///
    /// `features` is a Float64Array, `edges` is `[{ src, tgt }, ...]`.
    /// Optional `curvatures` (defaults to `[0.0, -1.0]`).
    /// Returns `{ output, curvatures, distances }`.
    pub fn product_manifold_attention(
        &mut self,
        features: JsValue,
        edges: JsValue,
    ) -> Result<JsValue, JsError> {
        let feats: Vec<f64> = serde_wasm_bindgen::from_value(features)
            .map_err(|e| JsError::new(&format!("invalid features: {e}")))?;
        let ed: Vec<Edge> = serde_wasm_bindgen::from_value(edges)
            .map_err(|e| JsError::new(&format!("invalid edges: {e}")))?;
        let curvatures = vec![0.0, -1.0]; // default mixed curvatures
        let result = self
            .inner
            .product_manifold_attention(&feats, &ed, &curvatures);
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Product manifold distance between two points.
    ///
    /// `a` and `b` are Float64Arrays, `curvatures` is `[number, ...]`.
    pub fn product_manifold_distance(
        &self,
        a: JsValue,
        b: JsValue,
        curvatures: JsValue,
    ) -> Result<f64, JsError> {
        let av: Vec<f64> = serde_wasm_bindgen::from_value(a)
            .map_err(|e| JsError::new(&format!("invalid a: {e}")))?;
        let bv: Vec<f64> = serde_wasm_bindgen::from_value(b)
            .map_err(|e| JsError::new(&format!("invalid b: {e}")))?;
        let cv: Vec<f64> = serde_wasm_bindgen::from_value(curvatures)
            .map_err(|e| JsError::new(&format!("invalid curvatures: {e}")))?;
        Ok(self.inner.product_manifold_distance(&av, &bv, &cv))
    }

    // ===================================================================
    // Verified Training
    // ===================================================================

    /// Verified training step with features, targets, and weights.
    ///
    /// `features`, `targets`, `weights` are Float64Arrays.
    /// Returns `{ weights, certificate_id, loss, loss_monotonic,
    /// lipschitz_satisfied }`.
    pub fn verified_training_step(
        &mut self,
        features: JsValue,
        targets: JsValue,
        weights: JsValue,
    ) -> Result<JsValue, JsError> {
        let f: Vec<f64> = serde_wasm_bindgen::from_value(features)
            .map_err(|e| JsError::new(&format!("invalid features: {e}")))?;
        let t: Vec<f64> = serde_wasm_bindgen::from_value(targets)
            .map_err(|e| JsError::new(&format!("invalid targets: {e}")))?;
        let w: Vec<f64> = serde_wasm_bindgen::from_value(weights)
            .map_err(|e| JsError::new(&format!("invalid weights: {e}")))?;
        let result = self
            .inner
            .verified_training_step(&f, &t, &w, 0.001)
            .map_err(|e| JsError::new(&format!("{e}")))?;
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// A single verified SGD step (raw weights + gradients).
    ///
    /// Returns `{ weights, proof_id, loss_before, loss_after, gradient_norm }`.
    pub fn verified_step(
        &mut self,
        weights: JsValue,
        gradients: JsValue,
        lr: f64,
    ) -> Result<JsValue, JsError> {
        let w: Vec<f64> = serde_wasm_bindgen::from_value(weights)
            .map_err(|e| JsError::new(&format!("invalid weights: {e}")))?;
        let g: Vec<f64> = serde_wasm_bindgen::from_value(gradients)
            .map_err(|e| JsError::new(&format!("invalid gradients: {e}")))?;
        let result = self
            .inner
            .verified_step(&w, &g, lr)
            .map_err(|e| JsError::new(&format!("{e}")))?;
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    // ===================================================================
    // Economic / Game-Theoretic
    // ===================================================================

    /// Game-theoretic attention: computes Nash equilibrium allocations.
    ///
    /// `features` is a Float64Array, `edges` is `[{ src, tgt }, ...]`.
    /// Returns `{ allocations, utilities, nash_gap, converged }`.
    pub fn game_theoretic_attention(
        &mut self,
        features: JsValue,
        edges: JsValue,
    ) -> Result<JsValue, JsError> {
        let feats: Vec<f64> = serde_wasm_bindgen::from_value(features)
            .map_err(|e| JsError::new(&format!("invalid features: {e}")))?;
        let ed: Vec<Edge> = serde_wasm_bindgen::from_value(edges)
            .map_err(|e| JsError::new(&format!("invalid edges: {e}")))?;
        let result = self.inner.game_theoretic_attention(&feats, &ed);
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    // ===================================================================
    // Stats & Reset
    // ===================================================================

    /// Return transformer statistics.
    ///
    /// Returns `{ proofs_constructed, proofs_verified, cache_hits,
    /// cache_misses, attention_ops, physics_ops, bio_ops, training_steps }`.
    pub fn stats(&self) -> Result<JsValue, JsError> {
        let s = self.inner.stats();
        serde_wasm_bindgen::to_value(&s).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Reset all internal state (caches, counters, gates).
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_nonempty() {
        assert!(!version().is_empty());
    }
}
