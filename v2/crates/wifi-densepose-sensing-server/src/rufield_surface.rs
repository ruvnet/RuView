//! ADR-262 **P3** — the live RuField surface.
//!
//! This is the data-path wiring that turns RuView's governed sensing cycle into
//! signed RuField [`FieldEvent`]s on two **additive** network endpoints:
//!
//! - `GET /api/field`  — the most recent surfaced `FieldEvent`(s) as JSON;
//! - `GET /ws/field`   — a WebSocket that streams each cycle's `FieldEvent`
//!   (mirrors the `/ws/sensing` broadcast-subscribe pattern).
//!
//! It is purely additive: `/ws/sensing` and every existing endpoint are
//! unchanged. The conversion itself lives entirely in the P1
//! [`wifi_densepose_rufield`] anti-corruption bridge (ADR-262 §5.4 — the single
//! coupling point); this module only (a) holds the dedicated signer + a bounded
//! ring buffer of recent events in server state, (b) builds a
//! [`SensingSnapshot`] from the **same real data** the cycle already produced
//! (`SensingUpdate` features/classification/signal_field joined with the
//! governed-engine [`TrustedOutput`] trust state at `main.rs:~5886`/`:~5938`),
//! and (c) applies the §10 network egress gate so above-policy classes never
//! reach the wire.
//!
//! ## Honesty (ADR-262 §0 / §6)
//!
//! This wires **real** RuView sensing into RuField events on a live endpoint,
//! but: (a) it is the **single-link CSI** sensing with its existing caveats —
//! there is **no validated room-coordinate accuracy** (`field_localize` says so;
//! positions are "strongest field peak", not triangulation); (b) the signing
//! key is a **dedicated dev/sensing key** pending the ADR-262 §8 Q1 ownership
//! decision (reusing the `cog-ha-matter` Ed25519 key is the **deferred P2**
//! call — P3 deliberately uses a standalone key so it does not pre-empt that);
//! (c) **no accuracy is claimed.** The win is narrowly: "RuView's live sensing
//! now speaks RuField on `/ws/field`."

use std::collections::VecDeque;
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Json},
};
use tokio::sync::{broadcast, RwLock};

// Re-export the bridge input types `main.rs` needs to build a snapshot, so the
// server-side call site depends only on `rufield_surface` (the server seam).
pub use wifi_densepose_rufield::{
    network_egress_allowed, snapshot_to_field_event, FieldEvent, RuViewPrivacyClass,
    SensingClass, SensingFeatures, SensingSnapshot, Signer, SignalField,
};

/// How many recent surfaced `FieldEvent`s the ring buffer retains. Small and
/// bounded — this is a live tap, not a store (ADR-262 §4 P3 "small bounded ring
/// buffer of recent events").
pub const FIELD_RING_CAPACITY: usize = 64;

/// Broadcast channel depth for `/ws/field`. Matches the `/ws/sensing` `tx`
/// channel size (256) so a slow field client drops messages rather than
/// stalling the sensing loop.
pub const FIELD_BROADCAST_CAPACITY: usize = 256;

/// Environment variable carrying the 32-byte hex/raw signing seed for the
/// dedicated RuField sensing signer. When unset, a deterministic dev default is
/// used (with a logged warning). See [`FieldSurface::from_env`].
pub const SIGNING_SEED_ENV: &str = "WDP_RUFIELD_SIGNING_SEED";

/// Deterministic dev signing seed used when [`SIGNING_SEED_ENV`] is unset. This
/// is a **dev/sensing key**, intentionally standalone (ADR-262 §8 Q1 — the
/// `cog-ha-matter` key reuse is the deferred P2 decision, not pre-empted here).
const DEV_SIGNING_SEED: &[u8; 32] = b"adr262-ruview-rufield-dev-seed!!";

/// The live RuField surface state held in `AppStateInner` (ADR-262 P3).
///
/// Owns the **dedicated** ed25519 [`Signer`], a bounded ring buffer of the most
/// recent network-surfaced events, and the `/ws/field` broadcast sender.
pub struct FieldSurface {
    signer: Signer,
    /// Bounded ring of recent **network-surfaced** events (most recent last).
    recent: VecDeque<FieldEvent>,
    /// Broadcast topic for `/ws/field` (JSON-serialized `FieldEvent`s).
    tx: broadcast::Sender<String>,
    /// True when the dev default seed is in use (drives a one-time warning and
    /// is surfaced in `/api/field` metadata so operators can see they are on a
    /// dev key).
    using_dev_key: bool,
}

impl FieldSurface {
    /// Build a surface with an explicit 32-byte seed (deterministic signer).
    #[must_use]
    pub fn from_seed(seed: &[u8; 32], using_dev_key: bool) -> Self {
        let (tx, _rx) = broadcast::channel(FIELD_BROADCAST_CAPACITY);
        Self {
            signer: Signer::from_seed(seed),
            recent: VecDeque::with_capacity(FIELD_RING_CAPACITY),
            tx,
            using_dev_key,
        }
    }

    /// Build a surface from the environment (ADR-262 §4 P3 / open-question 1).
    ///
    /// Reads [`SIGNING_SEED_ENV`] as either a 64-char hex string or a raw 32+
    /// byte UTF-8 value (first 32 bytes used). When unset/invalid it falls back
    /// to the deterministic [`DEV_SIGNING_SEED`] and logs a `WARN` — the key is
    /// a standalone **dev/sensing** key, NOT the deferred-P2 `cog-ha-matter`
    /// key.
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var(SIGNING_SEED_ENV).ok().and_then(|v| parse_seed(&v)) {
            Some(seed) => {
                tracing::info!(
                    "ADR-262 P3: RuField surface using signing seed from {SIGNING_SEED_ENV} \
                     (dedicated sensing key)"
                );
                Self::from_seed(&seed, false)
            }
            None => {
                tracing::warn!(
                    "ADR-262 P3: {SIGNING_SEED_ENV} unset/invalid — RuField surface using the \
                     DETERMINISTIC DEV signing key. This is a dev/sensing key pending the \
                     ADR-262 §8 Q1 (P2) key-ownership decision; set {SIGNING_SEED_ENV} (64-hex \
                     or 32-byte value) for a real deployment."
                );
                Self::from_seed(DEV_SIGNING_SEED, true)
            }
        }
    }

    /// The public key of the dedicated signer (hex), so consumers can verify
    /// receipts without the private seed.
    #[must_use]
    pub fn signer_pubkey_hex(&self) -> String {
        self.signer.public_hex()
    }

    /// Whether the dev default key is in use.
    #[must_use]
    pub fn using_dev_key(&self) -> bool {
        self.using_dev_key
    }

    /// A `/ws/field` subscription.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// The most recent surfaced events, oldest→newest.
    #[must_use]
    pub fn recent(&self) -> Vec<FieldEvent> {
        self.recent.iter().cloned().collect()
    }

    /// Convert one cycle's [`SensingSnapshot`] into a signed [`FieldEvent`],
    /// apply the §10 network egress gate, and — **iff** the event may leave the
    /// box — push it into the ring + broadcast it on `/ws/field`.
    ///
    /// Returns `Some(event)` when an event was surfaced, `None` when the cycle
    /// was held edge-local (above network policy — e.g. a `Derived → P4/P5`
    /// cycle) or carried no presence. Two structural guarantees live here, so
    /// they hold regardless of caller:
    ///
    /// - **no phantom events** — a no-presence cycle (`presence == false`)
    ///   surfaces nothing (ADR-262 §4 P3 / §6); there is no person to describe.
    /// - **privacy-safety pin** — above-policy classes (P0, P3–P5) are never
    ///   placed on the network surface; only egress-safe P1/P2 events leave.
    pub fn emit(&mut self, snap: &SensingSnapshot) -> Option<FieldEvent> {
        // No-presence ⇒ no phantom event (fabricating one would be dishonest).
        if !snap.classification.presence {
            return None;
        }

        let event = snapshot_to_field_event(snap, &self.signer);

        // §10 network egress gate (ADR-262 §4 P3): only P1/P2 leave the box by
        // default; P0 raw and P3/P4/P5 (above the default P2 ceiling, or
        // identity/biometric) are held edge-local. A `Derived` cycle is P4/P5
        // ⇒ never surfaced as a low-privacy network event.
        if !network_egress_allowed(event.observation.privacy_class, snap.identity_bound) {
            tracing::trace!(
                privacy_class = ?event.observation.privacy_class,
                "ADR-262 P3: cycle held edge-local (above network policy), not surfaced on /api/field"
            );
            return None;
        }

        if self.recent.len() == FIELD_RING_CAPACITY {
            self.recent.pop_front();
        }
        self.recent.push_back(event.clone());

        if let Ok(json) = serde_json::to_string(&event) {
            let _ = self.tx.send(json);
        }
        Some(event)
    }
}

/// Parse [`SIGNING_SEED_ENV`] as 64-char hex or a raw 32+ byte UTF-8 value.
fn parse_seed(v: &str) -> Option<[u8; 32]> {
    let v = v.trim();
    // 64 hex chars → 32 bytes.
    if v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()) {
        let mut out = [0u8; 32];
        for (i, chunk) in v.as_bytes().chunks(2).enumerate() {
            let hi = (chunk[0] as char).to_digit(16)?;
            let lo = (chunk[1] as char).to_digit(16)?;
            out[i] = ((hi << 4) | lo) as u8;
        }
        return Some(out);
    }
    // Otherwise: first 32 bytes of the raw value (must be at least 32 long so a
    // short/typo'd value fails closed to the dev key rather than a weak key).
    let bytes = v.as_bytes();
    if bytes.len() >= 32 {
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes[..32]);
        return Some(out);
    }
    None
}

/// Build a [`SensingSnapshot`] from the real per-cycle values (ADR-262 P3 §4.2).
///
/// This is the join the ADR mandates: `SensingUpdate` features / classification
/// / signal-field **plus** the governed engine's `effective_class` / `demoted`
/// / `identity_bound` trust state. All inputs are the same real data the cycle
/// already computed — nothing is fabricated. `signal_field` is passed through as
/// the honest "strongest field peak" readout (no calibrated coordinates).
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn build_snapshot(
    timestamp_ns: u64,
    node_id: String,
    features: SensingFeatures,
    classification: SensingClass,
    signal_field: Option<SignalField>,
    trust_class: RuViewPrivacyClass,
    demoted: bool,
    identity_bound: bool,
    synthetic: bool,
) -> SensingSnapshot {
    SensingSnapshot {
        timestamp_ns,
        features,
        classification,
        signal_field,
        trust_class,
        demoted,
        identity_bound,
        node_id,
        synthetic,
    }
}

/// Map RuView's live governed-engine `bfld::PrivacyClass` (the `effective_class`
/// on `TrustedOutput`) onto the bridge's [`RuViewPrivacyClass`] input.
///
/// This is a **lossless, same-meaning** re-encoding of the four byte-level
/// classes — both enums are `Raw/Derived/Anonymous/Restricted` in the same
/// order. It exists only so `main.rs` can pass the engine's class into the
/// bridge without the bridge depending on `wifi-densepose-bfld` (keeping it an
/// anti-corruption layer, ADR-262 §5.4). The information-content privacy
/// mapping (the §3.3 correctness item) happens *inside* the bridge.
#[must_use]
pub fn ruview_class_from_bfld(class: wifi_densepose_bfld::PrivacyClass) -> RuViewPrivacyClass {
    use wifi_densepose_bfld::PrivacyClass as B;
    match class {
        B::Raw => RuViewPrivacyClass::Raw,
        B::Derived => RuViewPrivacyClass::Derived,
        B::Anonymous => RuViewPrivacyClass::Anonymous,
        B::Restricted => RuViewPrivacyClass::Restricted,
    }
}

// ── Handlers ────────────────────────────────────────────────────────────────

/// Shared state for the field surface handlers. Generic over the lock guard so
/// the module can be tested in isolation with a tiny state (ADR-262 P3 test
/// gate) and wired into the full `AppStateInner` in `main.rs` via an adapter.
pub type FieldState = Arc<RwLock<FieldSurface>>;

/// `GET /api/field` — the most recent network-surfaced `FieldEvent`s as JSON,
/// plus surface metadata (the signer pubkey + whether a dev key is in use).
///
/// When no event has been surfaced yet (empty room / above-policy cycles only)
/// the `events` array is empty — an **explicit empty payload**, never a
/// fabricated event (ADR-262 §4 P3 / §6 honesty).
pub async fn api_field(State(state): State<FieldState>) -> Json<serde_json::Value> {
    let s = state.read().await;
    Json(serde_json::json!({
        "spec": "rufield",
        "endpoint": "/api/field",
        "signer_pubkey_hex": s.signer_pubkey_hex(),
        "dev_signing_key": s.using_dev_key(),
        "events": s.recent(),
    }))
}

/// `GET /ws/field` — upgrade to a WebSocket that streams each surfaced
/// `FieldEvent` (JSON) as the sensing loop emits it. Mirrors `/ws/sensing`:
/// subscribe to the broadcast topic and forward.
pub async fn ws_field(ws: WebSocketUpgrade, State(state): State<FieldState>) -> impl IntoResponse {
    let rx = {
        let s = state.read().await;
        s.subscribe()
    };
    ws.on_upgrade(move |socket| handle_ws_field_client(socket, rx))
}

async fn handle_ws_field_client(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    // Forward broadcast events; exit on client close or fatal lag.
    loop {
        match rx.recv().await {
            Ok(json) => {
                if socket.send(Message::Text(json)).await.is_err() {
                    break; // client gone
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                // Slow client missed events — keep going from the latest.
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

/// Build the additive field-surface router. Mounted into the main HTTP router
/// in `main.rs`; also used standalone by the integration tests (ADR-262 P3
/// gate, `tower::oneshot`).
#[must_use]
pub fn router(state: FieldState) -> axum::Router {
    use axum::routing::get;
    axum::Router::new()
        .route("/api/field", get(api_field))
        .route("/ws/field", get(ws_field))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_rufield::{is_fusable, PrivacyClass};

    fn features() -> SensingFeatures {
        SensingFeatures {
            mean_rssi: -55.0,
            variance: 0.4,
            motion_band_power: 2.0,
            breathing_band_power: 0.3,
            dominant_freq_hz: 0.25,
            change_points: 1,
            spectral_power: 3.0,
        }
    }

    fn present_class() -> SensingClass {
        SensingClass {
            motion_level: "low".into(),
            presence: true,
            confidence: 0.82,
        }
    }

    #[test]
    fn parse_seed_hex_and_raw_and_short() {
        // 64 hex chars → 32 bytes.
        let hex = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        let parsed = parse_seed(hex).expect("valid hex seed");
        assert_eq!(parsed[0], 0x00);
        assert_eq!(parsed[31], 0xff);
        // Raw 32-byte value.
        assert!(parse_seed("0123456789abcdef0123456789abcdef").is_some());
        // Too short → fail closed (None → dev key).
        assert!(parse_seed("short").is_none());
    }

    #[test]
    fn anonymous_cycle_surfaces_fusable_event() {
        let mut surface = FieldSurface::from_seed(DEV_SIGNING_SEED, true);
        let snap = build_snapshot(
            1_791_986_400_000_000_000,
            "esp32_room_01".into(),
            features(),
            present_class(),
            None,
            RuViewPrivacyClass::Anonymous, // → P2, network-allowed
            false,
            false,
            false,
        );
        let ev = surface.emit(&snap).expect("anonymous P2 cycle is surfaced");
        assert_eq!(ev.observation.privacy_class, PrivacyClass::P2);
        assert!(is_fusable(&ev), "live event must be ed25519-signed & fusable");
        assert_eq!(surface.recent().len(), 1);
    }

    #[test]
    fn derived_cycle_never_surfaces_low_privacy() {
        // The privacy-safety pin: a Derived (identity) cycle maps to P4/P5 and
        // is held edge-local — it must NEVER appear on the network surface.
        let mut surface = FieldSurface::from_seed(DEV_SIGNING_SEED, true);
        for identity_bound in [false, true] {
            let snap = build_snapshot(
                1_791_986_400_000_000_000,
                "esp32_room_01".into(),
                features(),
                present_class(),
                None,
                RuViewPrivacyClass::Derived,
                false,
                identity_bound,
                false,
            );
            assert!(
                surface.emit(&snap).is_none(),
                "Derived cycle (identity_bound={identity_bound}) must be held edge-local"
            );
        }
        assert!(surface.recent().is_empty(), "no Derived event may reach the surface");
    }

    #[test]
    fn ring_buffer_is_bounded() {
        let mut surface = FieldSurface::from_seed(DEV_SIGNING_SEED, true);
        for i in 0..(FIELD_RING_CAPACITY + 10) {
            let snap = build_snapshot(
                1_791_986_400_000_000_000 + i as u64,
                "esp32_room_01".into(),
                features(),
                present_class(),
                None,
                RuViewPrivacyClass::Anonymous,
                false,
                false,
                false,
            );
            surface.emit(&snap);
        }
        assert_eq!(surface.recent().len(), FIELD_RING_CAPACITY);
    }
}
