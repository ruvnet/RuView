//! `cog-pose-estimation` library surface.
//!
//! See `ADR-101` for the design and `ADR-100` for the surrounding Cog
//! packaging spec. This crate is intentionally a thin shell around
//! `wifi-densepose-train`'s exported model types — the heavy lifting
//! (encoder, pose head) lives there.

pub mod config;
pub mod inference;
pub mod manifest;
pub mod publisher;
pub mod runtime;

/// Cog identifier — matches the on-disk path
/// `/var/lib/cognitum/apps/pose-estimation/`.
pub const COG_ID: &str = "pose-estimation";

/// Cog version (sourced from Cargo.toml at build time).
pub const COG_VERSION: &str = env!("CARGO_PKG_VERSION");
