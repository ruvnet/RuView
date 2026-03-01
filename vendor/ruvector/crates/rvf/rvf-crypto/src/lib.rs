//! Cryptographic primitives for the RuVector Format (RVF).
//!
//! Provides SHAKE-256 hashing, Ed25519 segment signing/verification,
//! signature footer codec, and WITNESS_SEG audit-trail support.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod attestation;
pub mod footer;
pub mod hash;
pub mod lineage;
#[cfg(feature = "ed25519")]
pub mod sign;
pub mod witness;

pub use attestation::{
    attestation_witness_entry, build_attestation_witness_payload, decode_attestation_header,
    decode_attestation_record, decode_tee_bound_key, encode_attestation_header,
    encode_attestation_record, encode_tee_bound_key, verify_attestation_witness_payload,
    verify_key_binding, QuoteVerifier, TeeBoundKeyRecord, VerifiedAttestationEntry,
};
pub use footer::{decode_signature_footer, encode_signature_footer};
pub use hash::{shake256_128, shake256_256, shake256_hash};
pub use lineage::{
    compute_manifest_hash, lineage_record_from_bytes, lineage_record_to_bytes,
    lineage_witness_entry, verify_lineage_chain,
};
#[cfg(feature = "ed25519")]
pub use sign::{sign_segment, verify_segment};
pub use witness::{create_witness_chain, verify_witness_chain, WitnessEntry};
