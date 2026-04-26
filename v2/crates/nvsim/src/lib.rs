//! NV-diamond magnetometer pipeline simulator — deterministic, no hidden mocks.
//!
//! `nvsim` is a standalone leaf crate. It models a forward-only magnetic
//! sensing path — scene → source synthesis → material attenuation → NV
//! ensemble → digitiser → binary frames + SHA-256 witness — using explicit
//! physics approximations validated against published primary sources.
//!
//! It is **not** a hardware-control stack, microscope simulator, full
//! Hamiltonian solver, or claim of fT-level sensitivity. This crate does
//! not control lasers, microwave sources, ADC hardware, or real NV sensors.
//!
//! # Implementation plan
//!
//! See `docs/research/quantum-sensing/15-nvsim-implementation-plan.md` for
//! the six-pass build spec. This release ships **Pass 1 only**: crate
//! scaffold, [`scene`] types, and the [`frame::MagFrame`] binary record.
//!
//! # Pass 1 surface
//!
//! - [`scene::Scene`], [`scene::DipoleSource`], [`scene::CurrentLoop`],
//!   [`scene::FerrousObject`], [`scene::EddyCurrent`]
//! - [`frame::MagFrame`] + [`frame::MAG_FRAME_MAGIC`] (`0xC51A_6E70`)
//! - [`NvsimError`] — top-level error type for parse / serialisation failures
//!
//! Subsequent passes add `source`, `propagation`, `sensor`, `digitiser`,
//! `pipeline`, and `proof` modules.

#![warn(missing_docs)]

pub mod frame;
pub mod propagation;
pub mod scene;
pub mod source;

pub use frame::{MagFrame, MAG_FRAME_MAGIC, MAG_FRAME_VERSION};
pub use propagation::{
    attenuate, material_is_heavy, material_loss_db_per_m, LosSegment, Material, Propagator,
};
pub use scene::{CurrentLoop, DipoleSource, EddyCurrent, FerrousObject, Scene};
pub use source::{
    current_loop_field, dipole_field, ferrous_field, scene_field_at, scene_field_at_sensors,
    R_MIN_M,
};

/// Top-level simulator error type.
#[derive(Debug, thiserror::Error)]
pub enum NvsimError {
    /// JSON serialisation / parsing failed for a scene or frame.
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),

    /// Magic-number mismatch on frame parse.
    #[error("magic mismatch: got 0x{got:08X}, expected 0x{expected:08X}")]
    MagicMismatch {
        /// Magic value received.
        got: u32,
        /// Magic value expected.
        expected: u32,
    },

    /// Frame buffer length disagrees with the fixed v1 layout.
    #[error("frame length mismatch: got {got} bytes, expected {expected}")]
    FrameLengthMismatch {
        /// Bytes received.
        got: usize,
        /// Bytes expected for this version.
        expected: usize,
    },

    /// Frame version is not supported by this build.
    #[error("unsupported frame version: got {got}, this build supports {supported}")]
    UnsupportedVersion {
        /// Version received.
        got: u16,
        /// Highest version this build understands.
        supported: u16,
    },

    /// A configuration value is out of the supported range.
    #[error("invalid config: {0}")]
    InvalidConfig(String),
}

/// Permeability of free space (T·m/A). Jackson 3e §5.6.
pub const MU_0: f64 = 4.0 * std::f64::consts::PI * 1.0e-7;

/// NV electronic gyromagnetic ratio (Hz/T). Doherty 2013 §3.
pub const GAMMA_E: f64 = 28.0e9;

/// NV zero-field-splitting transition (Hz). Doherty 2013 §3.
pub const D_GS: f64 = 2.87e9;
