//! # wifi-densepose-rufield
//!
//! ADR-262 **anti-corruption bridge**: converts RuView's live WiFi-CSI sensing
//! output into signed RuField [`FieldEvent`](rufield_core::FieldEvent)s.
//!
//! This crate is the **single coupling point** (ADR-262 §5.4) between RuView and
//! the standalone RuField MFS spec (`vendor/rufield`, ADR-260). It depends on
//! the four pure-Rust rufield crates **via path** — `rufield-core`,
//! `-provenance`, `-privacy`, `-fusion` — and on **no** RuView internal crate.
//! Inputs are owned primitives ([`SensingSnapshot`]) that mirror what RuView's
//! sensing cycle produces, so the bridge never imports `SensingUpdate` /
//! `TrustedOutput` directly.
//!
//! ## What P1 ships (honesty — ADR-262 §0 / §6)
//!
//! This is **P1 plumbing**: a tested `SensingSnapshot → FieldEvent` conversion
//! plus the **fail-closed privacy mapping** that is the §3.3 correctness item.
//! It is **not** wired into the live server (that is P3) and makes **no accuracy
//! claim** — RuField v0.1 is synthetic end-to-end and RuView's single-link CSI
//! carries its own caveats. The gates here are round-trip / fusability /
//! privacy-safety / determinism, not validated F1.
//!
//! ## The critical correctness item: the privacy mapping (§3.3)
//!
//! RuView's `Derived` class has byte value `1` (below `Anonymous = 2`) yet
//! carries an identity embedding. The bridge maps it to **P4/P5 by information
//! content, never P1** — see [`map_privacy`]. Mapping off the byte would leak
//! identity as low-privacy; [`map_privacy`] (and its dedicated test
//! `derived_identity_never_maps_to_low_privacy`) exist specifically to prevent
//! that.
//!
//! ## Example
//!
//! ```
//! use wifi_densepose_rufield::{
//!     snapshot_to_field_event, SensingSnapshot, SensingFeatures, SensingClass,
//!     RuViewPrivacyClass,
//! };
//! use rufield_provenance::{Signer, is_fusable};
//!
//! let snap = SensingSnapshot {
//!     timestamp_ns: 1_791_986_400_000_000_000,
//!     features: SensingFeatures {
//!         mean_rssi: -55.0,
//!         variance: 0.4,
//!         motion_band_power: 2.0,
//!         breathing_band_power: 0.3,
//!         dominant_freq_hz: 0.25,
//!         change_points: 1,
//!         spectral_power: 3.0,
//!     },
//!     classification: SensingClass {
//!         motion_level: "low".into(),
//!         presence: true,
//!         confidence: 0.82,
//!     },
//!     signal_field: None,
//!     trust_class: RuViewPrivacyClass::Anonymous,
//!     demoted: false,
//!     identity_bound: false,
//!     node_id: "esp32_room_01".into(),
//!     synthetic: false,
//! };
//!
//! let signer = Signer::from_seed(b"adr-262-bridge-seed-32-bytes-ok!");
//! let event = snapshot_to_field_event(&snap, &signer);
//! assert!(is_fusable(&event)); // ed25519-signed, non-synthetic ⇒ fusable
//! ```

#![forbid(unsafe_code)]

pub mod bridge;
pub mod privacy;
pub mod snapshot;
pub mod ultrasonic;

pub use bridge::{snapshot_egress_class, snapshot_to_field_event};
pub use privacy::{apply_demotion_floor, egress_class, map_privacy};
pub use snapshot::{
    RuViewPrivacyClass, SensingClass, SensingFeatures, SensingSnapshot, SignalField,
};
pub use ultrasonic::{ScanError, ScanSource, UltrasonicScan, ULTRASONIC_EGRESS_CLASS};

// Re-export the rufield surface a bridge consumer needs, so callers depend on
// one crate.
pub use rufield_core::{Destination, FieldEvent, Modality, PrivacyClass, PrivacyDecision};
pub use rufield_fusion::RuFieldFusion;
pub use rufield_privacy::{DefaultPrivacyGuard, PrivacyPolicy};
pub use rufield_provenance::{is_fusable, verify_event, Signer};

/// Whether a mapped [`PrivacyClass`] may be surfaced on a **network** egress
/// (ADR-262 §4 P3 — the live `/api/field` / `/ws/field` surface must respect
/// the same default §10 network policy `/ws/sensing` honours, never emitting
/// above-policy data).
///
/// **Fail-closed for a live, unattended surface.** The live RuView surface has
/// **no per-event consent or identity-binding ceremony** — so this is *stricter*
/// than [`DefaultPrivacyGuard::authorize`]: it requires BOTH that the default
/// guard would `Allow` the class onto [`Destination::Network`] with **no consent
/// granted**, AND that the class is at or below the default network ceiling
/// ([`PrivacyClass::P2`]). The second clause deliberately drops P4/P5 even
/// though the guard's consent/identity *exceptions* would let an explicitly
/// consented/identity-bound P4/P5 through — because the live surface cannot
/// honestly assert that consent. Net effect: only **P1/P2** leave the box; P0
/// (raw) and P3/P4/P5 are held edge-local.
///
/// This is the privacy-safety pin for the live surface: a `Derived` cycle maps
/// to P4 (or P5 when identity-bound) via [`map_privacy`] and is therefore
/// **never** surfaced as a network event — neither as a low-privacy P1 (the
/// §3.3 mapping trap) nor at all.
#[must_use]
pub fn network_egress_allowed(class: PrivacyClass, identity_bound: bool) -> bool {
    use rufield_core::PrivacyGuard;
    let guard_allows = matches!(
        DefaultPrivacyGuard::default().authorize(
            class,
            Destination::Network,
            false, // no per-event consent on the live network surface (fail-closed)
            identity_bound,
        ),
        PrivacyDecision::Allow
    );
    // Additionally cap at the default network ceiling: an unattended live
    // surface never asserts the P4-consent / P5-identity exception.
    guard_allows && class <= PrivacyClass::P2
}
