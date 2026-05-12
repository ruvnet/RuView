use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemporalError {
    #[error("temporal head config invalid: {0}")]
    InvalidConfig(&'static str),

    /// Retained for back-compat with v0.1 callers; superseded by the
    /// per-operation errors below now that Dense is implemented.
    #[error("dense MHA backend not implemented yet (ADR-096 §4.4 follow-up)")]
    DenseBackendNotImplemented,

    /// Dense MHA has no notion of an accumulated KV cache — every
    /// new frame requires recomputing the full N² attention matrix
    /// (the structural gap ADR-096 §3.2 flagged). Callers that want
    /// streaming decode must use the SparseGqa backend.
    #[error("dense backend does not support streaming step(); use SparseGqa for online decode")]
    BackendDoesNotSupportStreaming,

    #[error("sparse attention kernel error: {0}")]
    Kernel(String),
}

impl From<ruvllm_sparse_attention::AttentionError> for TemporalError {
    fn from(e: ruvllm_sparse_attention::AttentionError) -> Self {
        TemporalError::Kernel(format!("{e}"))
    }
}
