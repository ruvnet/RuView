//! Core types for the RuVector Format (RVF).
//!
//! This crate provides the foundational types shared across all RVF crates:
//! segment headers, type enums, flags, error codes, and format constants.
//!
//! All types are `no_std` compatible by default.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

// Tests always need alloc (for Vec, format!, etc.) even without the feature.
#[cfg(all(test, not(feature = "alloc")))]
extern crate alloc;

pub mod agi_container;
pub mod attestation;
pub mod checksum;
pub mod compression;
pub mod constants;
pub mod cow_map;
pub mod dashboard;
pub mod data_type;
pub mod delta;
pub mod ebpf;
#[cfg(feature = "ed25519")]
pub mod ed25519;
pub mod error;
pub mod filter;
pub mod flags;
pub mod kernel;
pub mod kernel_binding;
pub mod lineage;
pub mod manifest;
pub mod membership;
pub mod profile;
pub mod qr_seed;
pub mod quality;
pub mod quant_type;
pub mod refcount;
pub mod security;
pub mod segment;
pub mod segment_type;
pub mod sha256;
pub mod signature;
pub mod wasm_bootstrap;
pub mod witness;

pub use agi_container::{
    AgiContainerHeader, AuthorityLevel, CoherenceThresholds, ContainerError, ContainerSegments,
    ExecutionMode, ResourceBudget, AGI_HAS_COHERENCE_GATES, AGI_HAS_DOMAIN_EXPANSION, AGI_HAS_EVAL,
    AGI_HAS_KERNEL, AGI_HAS_ORCHESTRATOR, AGI_HAS_SKILLS, AGI_HAS_TOOLS, AGI_HAS_WASM,
    AGI_HAS_WITNESS, AGI_HAS_WORLD_MODEL, AGI_HEADER_SIZE, AGI_MAGIC, AGI_MAX_CONTAINER_SIZE,
    AGI_OFFLINE_CAPABLE, AGI_REPLAY_CAPABLE, AGI_SIGNED, AGI_TAG_AUTHORITY_CONFIG,
    AGI_TAG_COST_CURVE, AGI_TAG_COUNTEREXAMPLES, AGI_TAG_DOMAIN_PROFILE, AGI_TAG_POLICY_KERNEL,
    AGI_TAG_TRANSFER_PRIOR,
};
pub use attestation::{AttestationHeader, AttestationWitnessType, TeePlatform, KEY_TYPE_TEE_BOUND};
pub use checksum::ChecksumAlgo;
pub use compression::CompressionAlgo;
pub use constants::*;
pub use cow_map::{CowMapEntry, CowMapHeader, MapFormat, COWMAP_MAGIC};
pub use dashboard::{DashboardHeader, DASHBOARD_MAGIC, DASHBOARD_MAX_SIZE};
pub use data_type::DataType;
pub use delta::{DeltaEncoding, DeltaHeader, DELTA_MAGIC};
pub use ebpf::{EbpfAttachType, EbpfHeader, EbpfProgramType, EBPF_MAGIC};
#[cfg(feature = "ed25519")]
pub use ed25519::{
    ct_eq_sig, ed25519_sign, ed25519_verify, Ed25519Keypair,
    PUBLIC_KEY_SIZE as ED25519_PUBLIC_KEY_SIZE, SECRET_KEY_SIZE as ED25519_SECRET_KEY_SIZE,
    SIGNATURE_SIZE as ED25519_SIGNATURE_SIZE,
};
pub use error::{ErrorCode, RvfError};
pub use filter::FilterOp;
pub use flags::SegmentFlags;
pub use kernel::{
    ApiTransport, KernelArch, KernelHeader, KernelType, KERNEL_FLAG_ATTESTATION_READY,
    KERNEL_FLAG_COMPRESSED, KERNEL_FLAG_HAS_ADMIN_API, KERNEL_FLAG_HAS_INGEST_API,
    KERNEL_FLAG_HAS_NETWORKING, KERNEL_FLAG_HAS_QUERY_API, KERNEL_FLAG_HAS_VIRTIO_BLK,
    KERNEL_FLAG_HAS_VIRTIO_NET, KERNEL_FLAG_HAS_VSOCK, KERNEL_FLAG_MEASURED,
    KERNEL_FLAG_RELOCATABLE, KERNEL_FLAG_REQUIRES_KVM, KERNEL_FLAG_REQUIRES_TEE,
    KERNEL_FLAG_REQUIRES_UEFI, KERNEL_FLAG_SIGNED, KERNEL_MAGIC,
};
pub use kernel_binding::KernelBinding;
pub use lineage::{
    DerivationType, FileIdentity, LineageRecord, LINEAGE_RECORD_SIZE, WITNESS_DERIVATION,
    WITNESS_LINEAGE_MERGE, WITNESS_LINEAGE_SNAPSHOT, WITNESS_LINEAGE_TRANSFORM,
    WITNESS_LINEAGE_VERIFY,
};
pub use manifest::{
    CentroidPtr, EntrypointPtr, HotCachePtr, Level0Root, PrefetchMapPtr, QuantDictPtr, TopLayerPtr,
};
pub use membership::{FilterMode, FilterType, MembershipHeader, MEMBERSHIP_MAGIC};
pub use profile::{DomainProfile, ProfileId};
pub use qr_seed::{
    HostEntry, LayerEntry, SeedHeader, QR_MAX_BYTES, SEED_COMPRESSED, SEED_ENCRYPTED,
    SEED_HAS_DOWNLOAD, SEED_HAS_MICROKERNEL, SEED_HAS_VECTORS, SEED_HEADER_SIZE, SEED_MAGIC,
    SEED_OFFLINE_CAPABLE, SEED_SIGNED, SEED_STREAM_UPGRADE,
};
pub use quality::{
    derive_response_quality, BudgetReport, BudgetType, DegradationReason, DegradationReport,
    FallbackPath, IndexLayersUsed, QualityPreference, ResponseQuality, RetrievalQuality,
    SafetyNetBudget, SearchEvidenceSummary,
};
pub use quant_type::QuantType;
pub use refcount::{RefcountHeader, REFCOUNT_MAGIC};
pub use security::{HardeningFields, SecurityError, SecurityPolicy};
pub use segment::SegmentHeader;
pub use segment_type::SegmentType;
pub use sha256::{hmac_sha256, sha256, Sha256};
pub use signature::{SignatureAlgo, SignatureFooter};
pub use wasm_bootstrap::{
    WasmHeader, WasmRole, WasmTarget, WASM_FEAT_BULK_MEMORY, WASM_FEAT_EXCEPTION_HANDLING,
    WASM_FEAT_GC, WASM_FEAT_MULTI_VALUE, WASM_FEAT_REFERENCE_TYPES, WASM_FEAT_SIMD,
    WASM_FEAT_TAIL_CALL, WASM_FEAT_THREADS, WASM_MAGIC,
};
pub use witness::{
    GovernanceMode, PolicyCheck, Scorecard, TaskOutcome, WitnessHeader, WITNESS_HEADER_SIZE,
    WITNESS_MAGIC, WIT_HAS_DIFF, WIT_HAS_PLAN, WIT_HAS_POSTMORTEM, WIT_HAS_SPEC, WIT_HAS_TEST_LOG,
    WIT_HAS_TRACE, WIT_SIGNED, WIT_TAG_DIFF, WIT_TAG_PLAN, WIT_TAG_POSTMORTEM, WIT_TAG_SPEC,
    WIT_TAG_TEST_LOG, WIT_TAG_TRACE,
};
#[cfg(feature = "alloc")]
pub use witness::{ToolCallEntry, TOOL_CALL_FIXED_SIZE};
