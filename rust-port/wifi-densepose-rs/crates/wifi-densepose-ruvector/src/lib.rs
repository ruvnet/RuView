//! RuVector v2.0.4 integration layer for WiFi-DensePose — ADR-017.
//!
//! This crate implements all 7 ADR-017 ruvector integration points for the
//! signal-processing pipeline (`signal`) and the Multi-AP Triage (MAT) module
//! (`mat`). Each integration point wraps a ruvector crate with WiFi-DensePose
//! domain logic so that callers never depend on ruvector directly.
//!
//! # Modules
//!
//! - [`signal`]: CSI signal processing — subcarrier partitioning, spectrogram
//!   gating, BVP aggregation, and Fresnel geometry solving.
//! - [`mat`]: Disaster detection — TDoA triangulation, compressed breathing
//!   buffer, and compressed heartbeat spectrogram.
//!
//! # ADR-017 Integration Map
//!
//! | File | ruvector crate | Purpose |
//! |------|----------------|---------|
//! | `signal/subcarrier` | ruvector-mincut | Graph min-cut subcarrier partitioning |
//! | `signal/spectrogram` | ruvector-attn-mincut | Attention-gated spectrogram denoising |
//! | `signal/bvp` | ruvector-attention | Attention-weighted BVP aggregation |
//! | `signal/fresnel` | ruvector-solver | Fresnel geometry estimation |
//! | `mat/triangulation` | ruvector-solver | TDoA survivor localisation |
//! | `mat/breathing` | ruvector-temporal-tensor | Tiered compressed breathing buffer |
//! | `mat/heartbeat` | ruvector-temporal-tensor | Tiered compressed heartbeat spectrogram |

#![warn(missing_docs)]

pub mod mat;
pub mod signal;
