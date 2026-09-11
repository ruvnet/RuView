//! Owned, primitive input types for the ADR-262 bridge.
//!
//! These deliberately **mirror** the shapes RuView's sensing cycle produces
//! (the `/ws/sensing` `SensingUpdate` build site at
//! `wifi-densepose-sensing-server/src/main.rs:~5938` and the `TrustedOutput`
//! trust state surfaced via `EngineBridge` at `main.rs:~5886`) **without
//! importing** RuView's internal crates. Keeping the bridge an anti-corruption
//! layer (ADR-262 §5.4) means it takes owned primitives, not `SensingUpdate`
//! or `TrustedOutput` directly — so this crate never depends on
//! `wifi-densepose-sensing-server`.

use serde::{Deserialize, Serialize};

/// The CSI feature scalars RuView publishes on every `/ws/sensing` cycle.
///
/// Mirrors `FeatureInfo` (`main.rs:368-377`). All values are in RuView's own
/// units; the bridge normalizes them into `Observation.features` for fusion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensingFeatures {
    /// Mean RSSI across the CSI window (dBm).
    pub mean_rssi: f64,
    /// CSI amplitude variance.
    pub variance: f64,
    /// Motion-band spectral power (drives `motion_energy`).
    pub motion_band_power: f64,
    /// Breathing-band spectral power (drives `breathing_band`).
    pub breathing_band_power: f64,
    /// Dominant frequency of the CSI window (Hz).
    pub dominant_freq_hz: f64,
    /// Number of change points detected in the window (drives `transient`).
    pub change_points: usize,
    /// Total spectral power of the window.
    pub spectral_power: f64,
}

/// The RuView classification block. Mirrors `ClassificationInfo`
/// (`main.rs:379-384`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensingClass {
    /// Coarse motion level label (e.g. `"none"`, `"low"`, `"high"`).
    pub motion_level: String,
    /// Whether a person is present.
    pub presence: bool,
    /// Classification confidence `0.0..=1.0`.
    pub confidence: f64,
}

/// A RuView signal field — a floor-plane grid of field values. Mirrors
/// `SignalField` (`main.rs:386-390`). The bridge derives a real position from
/// the strongest field peak (like `field_localize`) and **never fabricates**
/// coordinates when this is absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalField {
    /// Grid dimensions `[x, y, z]`.
    pub grid_size: [usize; 3],
    /// Row-major flattened field values; `len() == grid_size.product()`.
    pub values: Vec<f64>,
}

impl SignalField {
    /// Index `[x, y, z]` of the strongest field cell, or `None` if the grid is
    /// empty / all-NaN. This is the honest "strongest field peak" readout that
    /// `field_localize` (`field_localize.rs:16-27`) exposes — **not** calibrated
    /// triangulation.
    #[must_use]
    pub fn peak_cell(&self) -> Option<[i32; 3]> {
        let [nx, ny, nz] = self.grid_size;
        if nx == 0 || ny == 0 || nz == 0 || self.values.is_empty() {
            return None;
        }
        let mut best_idx: Option<usize> = None;
        let mut best_val = f64::NEG_INFINITY;
        for (i, &v) in self.values.iter().enumerate() {
            if v.is_finite() && v > best_val {
                best_val = v;
                best_idx = Some(i);
            }
        }
        let idx = best_idx?;
        // Row-major: idx = ((x * ny) + y) * nz + z.
        let z = idx % nz;
        let y = (idx / nz) % ny;
        let x = idx / (nz * ny);
        Some([x as i32, y as i32, z as i32])
    }
}

/// RuView's effective privacy class (the `effective_class` / privacy byte on
/// `TrustedOutput`).
///
/// This **mirrors** `wifi_densepose_bfld::PrivacyClass` (`bfld/lib.rs:103-116`,
/// `#[repr(u8)]`) — the four byte-level classes. The byte values are
/// **deliberately non-monotonic in information content**: `Derived = 1` carries
/// an identity embedding yet sorts *below* `Anonymous = 2`. The bridge's
/// `map_privacy` must therefore map by information content, NEVER by byte value
/// (ADR-262 §3.3 — the central correctness item).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuViewPrivacyClass {
    /// Byte `0` — raw CSI amplitude, local-only.
    Raw,
    /// Byte `1` — derived **identity** features (identity_embedding +
    /// identity_risk_score), LAN-only. The dangerous one (§3.3).
    Derived,
    /// Byte `2` — aggregate occupancy / motion, no identity.
    Anonymous,
    /// Byte `3` — care/regulated: occupancy minus risk score and hash;
    /// raw suppressed.
    Restricted,
}

impl RuViewPrivacyClass {
    /// The raw byte value used by RuView's `#[repr(u8)]` enum
    /// (`bfld/lib.rs:103`). Exposed only so callers can demonstrate the
    /// non-monotonicity trap in tests; the bridge never maps off this byte.
    #[must_use]
    pub fn raw_byte(self) -> u8 {
        match self {
            RuViewPrivacyClass::Raw => 0,
            RuViewPrivacyClass::Derived => 1,
            RuViewPrivacyClass::Anonymous => 2,
            RuViewPrivacyClass::Restricted => 3,
        }
    }
}

/// One sensing cycle, as a bridge input. Mirrors the join of `SensingUpdate`
/// (features + classification + signal_field) and the `TrustedOutput` trust
/// state (`trust_class`) that ADR-262 §1.2 / P1 say must be done at the bridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SensingSnapshot {
    /// Capture time, nanoseconds since Unix epoch (the real `SensingUpdate`
    /// timestamp, ns).
    pub timestamp_ns: u64,
    /// CSI feature scalars (`/ws/sensing` feature set).
    pub features: SensingFeatures,
    /// Classification (motion level / presence / confidence).
    pub classification: SensingClass,
    /// Optional signal field for a real position readout.
    pub signal_field: Option<SignalField>,
    /// RuView's effective privacy class (the source-of-truth, §3.3).
    pub trust_class: RuViewPrivacyClass,
    /// Whether the governed engine demoted this cycle (`TrustedOutput.demoted`).
    /// When `true` the emitted event must be `>= P2` and raw suppressed
    /// (§3.3 / §4 P2 gate (b)).
    pub demoted: bool,
    /// Whether this cycle's identity surface is bound to an enrolled identity
    /// (RuView's `identity_bound`). Promotes `Derived` to P5 when set.
    pub identity_bound: bool,
    /// Stable node id (e.g. `"esp32_room_01"`).
    pub node_id: String,
    /// Whether this cycle came from real hardware/replay or a synthetic demo
    /// source (RuView's `--source simulated` / Docker demo mode). Stamped
    /// verbatim onto the emitted event's `ProvenanceRef.synthetic` — this
    /// bridge does not get to assume "real" by default; the caller who knows
    /// where the cycle actually came from must say so.
    #[serde(default)]
    pub synthetic: bool,
}
