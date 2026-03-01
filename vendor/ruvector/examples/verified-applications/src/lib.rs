//! 10 exotic applications of ruvector-verified beyond dimension checks.
//!
//! Each module demonstrates a real-world domain where proof-carrying vector
//! operations provide structural safety that runtime assertions cannot.

pub mod agent_contracts;
pub mod financial_routing;
pub mod legal_forensics;
pub mod medical_diagnostics;
pub mod quantization_proof;
pub mod sensor_swarm;
pub mod simulation_integrity;
pub mod vector_signatures;
pub mod verified_memory;
pub mod weapons_filter;

/// Shared proof receipt that all domains produce.
#[derive(Debug, Clone)]
pub struct ProofReceipt {
    /// Domain identifier (e.g. "weapons", "medical", "trade").
    pub domain: String,
    /// Human-readable description of what was proved.
    pub claim: String,
    /// Proof term ID in the environment.
    pub proof_id: u32,
    /// 82-byte attestation bytes.
    pub attestation_bytes: Vec<u8>,
    /// Proof tier used (reflex/standard/deep).
    pub tier: String,
    /// Whether the gate passed.
    pub gate_passed: bool,
}
