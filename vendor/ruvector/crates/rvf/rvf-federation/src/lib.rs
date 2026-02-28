//! Federated RVF transfer learning.
//!
//! This crate implements the federation protocol described in ADR-057:
//! - **PII stripping**: Three-stage pipeline (detect, redact, attest)
//! - **Differential privacy**: Gaussian/Laplace noise, RDP accountant, gradient clipping
//! - **Federation protocol**: Export builder, import merger, version-aware conflict resolution
//! - **Federated aggregation**: FedAvg, FedProx, Byzantine-tolerant weighted averaging
//! - **Segment types**: FederatedManifest, DiffPrivacyProof, RedactionLog, AggregateWeights

pub mod types;
pub mod error;
pub mod pii_strip;
pub mod diff_privacy;
pub mod federation;
pub mod aggregate;
pub mod policy;

pub use types::*;
pub use error::FederationError;
pub use pii_strip::PiiStripper;
pub use diff_privacy::{DiffPrivacyEngine, PrivacyAccountant};
pub use federation::{ExportBuilder, ImportMerger};
pub use aggregate::{FederatedAggregator, AggregationStrategy};
pub use policy::FederationPolicy;
