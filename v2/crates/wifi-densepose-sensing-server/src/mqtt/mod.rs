//! ADR-115 §2 — MQTT auto-discovery publisher (HA-DISCO).
//!
//! This module implements the dual-protocol Home Assistant integration's
//! primary path: MQTT + HA auto-discovery. It owns the full lifecycle:
//!
//! 1. Connect to a user-supplied broker with optional TLS / mTLS.
//! 2. Publish HA discovery `config` topics (retained) on connect and at
//!    a refresh interval, so HA auto-creates one device + N entities per
//!    RuView node.
//! 3. Translate `sensing-server` broadcast messages (`edge_vitals`,
//!    `pose_data`, `sensing_update`) into per-entity state messages with
//!    rate limits.
//! 4. Maintain a `availability` topic per entity with LWT for offline
//!    detection.
//!
//! The module is gated behind the `mqtt` Cargo feature so the default
//! `sensing-server` binary stays small for users who don't need HA
//! integration. CLI flags parse unconditionally; the publisher is a
//! no-op without the feature.
//!
//! ## Layout
//!
//! - [`discovery`] — HA discovery payload generators per entity type
//! - [`state`]     — per-entity state-message encoders + rate limiter
//! - [`bridge`]    — sensing-broadcast JSON → [`state::VitalsSnapshot`]
//! - [`planner`]   — entity → state/availability plan + semantic primitives
//! - [`publisher`] — connection lifecycle + topic publication
//! - [`privacy`]   — biometric stripping per `--privacy-mode`
//! - [`config`]    — `MqttConfig` struct fed by [`crate::cli::Args`]
//!
//! ## Cross-protocol coupling
//!
//! The semantic inference layer ([`crate::semantic`], ADR-115 §3.12) runs
//! inside [`planner`], one bus per node, fed by the same snapshots as the
//! raw entities (ADR-115 P4.5). The Matter bridge (ADR-115 §3.11) does not
//! consume semantic states yet.

pub mod bridge;
pub mod config;
pub mod discovery;
pub mod planner;
pub mod privacy;
pub mod security;
// State encoders, the bridge and the planner compile without rumqttc, so
// they're available for testing under `--no-default-features`. Only the
// publisher itself (which holds the `rumqttc::AsyncClient`) needs the
// `mqtt` feature.
pub mod state;

#[cfg(feature = "mqtt")]
pub mod publisher;

pub use config::MqttConfig;
pub use discovery::{
    AvailabilityPayload, DeviceMeta, DiscoveryComponent, DiscoveryConfig, OriginMeta,
};

/// Stable origin string written into every HA discovery payload's `origin`
/// block so HA users can see which RuView version emitted the entities.
pub const ORIGIN_NAME: &str = "wifi-densepose-sensing-server";

/// Stable manufacturer string written into every HA discovery payload's
/// `device` block.
pub const MANUFACTURER: &str = "ruvnet";

/// Stable `support_url` written into every HA discovery payload's `origin`
/// block. Resolves to the HACS Python integration's follow-on repository
/// per ADR-115 §9.3.
pub const SUPPORT_URL: &str = "https://github.com/ruvnet/hass-wifi-densepose";

/// Stable HA discovery topic prefix default. Maintainer-accepted in
/// ADR-115 §9.2 — ship Home Assistant's own default rather than a
/// RuView-namespaced one, so the integration is plug-and-play with a
/// stock Mosquitto add-on. Operators with custom HA setups can override
/// via `--mqtt-prefix`.
pub const DEFAULT_DISCOVERY_PREFIX: &str = "homeassistant";
