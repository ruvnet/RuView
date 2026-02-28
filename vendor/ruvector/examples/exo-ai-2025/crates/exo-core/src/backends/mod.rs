//! Substrate backends — ADR-029 pluggable compute substrates for EXO-AI.
//! Each backend implements SubstrateBackend, providing different computational modalities.

pub mod neuromorphic;
pub mod quantum_stub;

pub use neuromorphic::NeuromorphicBackend;
pub use quantum_stub::QuantumStubBackend;

/// Unified substrate backend trait — all compute modalities implement this.
pub trait SubstrateBackend: Send + Sync {
    /// Backend identifier
    fn name(&self) -> &'static str;

    /// Similarity search in the backend's representational space.
    fn similarity_search(&self, query: &[f32], k: usize) -> Vec<SearchResult>;

    /// One-shot pattern adaptation (analogous to manifold deformation).
    fn adapt(&mut self, pattern: &[f32], reward: f32) -> AdaptResult;

    /// Check backend health / coherence level (0.0–1.0).
    fn coherence(&self) -> f32;

    /// Reset/clear backend state.
    fn reset(&mut self);
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: u64,
    pub score: f32,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct AdaptResult {
    pub delta_norm: f32,
    pub mode: &'static str,
    pub latency_us: u64,
}
