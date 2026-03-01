//! Two-level manifest system for the RuVector Format (RVF).
//!
//! The manifest system enables progressive boot:
//! - **Level 0** (fixed 4096 bytes at EOF): hotset pointers for instant query
//! - **Level 1** (variable-size TLV records): full segment directory
//!
//! A reader only needs Level 0 to start answering approximate queries.
//! Level 1 is loaded asynchronously for full-quality results.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod boot;
pub mod chain;
pub mod directory;
pub mod level0;
pub mod level1;
pub mod writer;

pub use boot::{boot_phase1, boot_phase2, extract_hotset_offsets, BootState, HotsetPointers};
pub use chain::OverlayChain;
pub use directory::{SegmentDirEntry, SegmentDirectory};
pub use level0::{read_level0, validate_level0, write_level0};
pub use level1::{read_tlv_records, write_tlv_records, Level1Manifest, ManifestTag, TlvRecord};
pub use writer::{build_manifest, build_manifest_at};

#[cfg(feature = "std")]
pub use writer::commit_manifest;
