//! RuVector Format runtime — the main user-facing API.
//!
//! This crate provides [`RvfStore`], the primary interface for creating,
//! opening, querying, and managing RVF vector stores. It ties together
//! the segment model, manifest system, HNSW indexing, quantization, and
//! compaction into a single cohesive runtime.
//!
//! # Architecture
//!
//! - **Append-only writes**: All mutations append new segments; no in-place edits.
//! - **Progressive boot**: Readers see results before the full file is loaded.
//! - **Single-writer / multi-reader**: Advisory lock file enforces exclusivity.
//! - **Background compaction**: Dead space is reclaimed without blocking queries.

pub mod adversarial;
pub mod agi_authority;
pub mod agi_coherence;
pub mod agi_container;
pub mod compaction;
pub mod compress;
pub mod cow;
pub mod cow_compact;
pub mod cow_map;
pub mod deletion;
pub mod dos;
pub mod ffi;
pub mod filter;
pub mod locking;
pub mod membership;
pub mod options;
#[cfg(feature = "qr")]
pub mod qr_encode;
pub mod qr_seed;
pub mod read_path;
pub mod safety_net;
pub mod seed_crypto;
pub mod status;
pub mod store;
pub mod witness;
pub mod write_path;

pub use adversarial::{
    adaptive_n_probe, centroid_distance_cv, combined_effective_n_probe,
    effective_n_probe_with_drift, is_degenerate_distribution, DEGENERATE_CV_THRESHOLD,
};
pub use agi_container::{AgiContainerBuilder, ParsedAgiManifest};
pub use compress::{compress, decompress, CompressError};
pub use cow::{CowEngine, CowStats, WitnessEvent};
pub use cow_compact::CowCompactor;
pub use cow_map::CowMap;
pub use dos::{BudgetTokenBucket, NegativeCache, ProofOfWork, QuerySignature};
pub use filter::FilterExpr;
pub use membership::MembershipFilter;
pub use options::{
    CompactionResult, DeleteResult, IngestResult, MetadataEntry, MetadataValue, QualityEnvelope,
    QueryOptions, RvfOptions, SearchResult, WitnessConfig,
};
#[cfg(feature = "qr")]
pub use qr_encode::{EcLevel, QrCode, QrEncoder, QrError};
pub use qr_seed::{
    make_host_entry, BootstrapProgress, DownloadManifest, ParsedSeed, SeedBuilder, SeedError,
};
pub use safety_net::{
    selective_safety_net_scan, should_activate_safety_net, Candidate, SafetyNetResult,
};
pub use seed_crypto::{
    full_content_hash, layer_content_hash, seed_content_hash, sign_seed, verify_layer, verify_seed,
    SIG_ALGO_HMAC_SHA256,
};
#[cfg(feature = "ed25519")]
pub use seed_crypto::{sign_seed_ed25519, verify_seed_ed25519, SIG_ALGO_ED25519};
pub use status::StoreStatus;
pub use store::RvfStore;
pub use witness::{
    GovernancePolicy, ParsedWitness, ScorecardBuilder, WitnessBuilder, WitnessError,
};
