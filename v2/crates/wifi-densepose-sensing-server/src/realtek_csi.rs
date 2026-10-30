//! Bounded summaries for ADR-323 Realtek RTL8721Dx (AmebaDplus) 1x1 CSI frames.
//!
//! The UDP data plane that carries these frames is unauthenticated, and
//! ADR-323 records that "RAC1 CRC checks corruption, not origin". A payload
//! flag therefore cannot establish where a frame came from, and the peer
//! addresses inside the payload are not evidence the server can attribute.
//! What is published here consequently:
//!
//! - states the origin as `unverified` until a frame is bound to an
//!   authenticated source;
//! - pseudonymises the peer and trigger addresses instead of republishing
//!   them;
//! - carries the arrival time and a freshness state, so a consumer can age
//!   the snapshot and see that it expired;
//! - exposes the sender's synthetic flag as a claim, never as an origin.
//!
//! (issue #1950)

use serde::Serialize;
use std::sync::OnceLock;
use wifi_densepose_hardware::realtek_csi::{CsiFlags, CsiFrame};

/// A snapshot younger than this is `fresh`.
pub(crate) const REALTEK_CSI_FRESH_MS: u64 = 5_000;

/// A snapshot older than this is `offline`: it is no longer published as a
/// measurement.
pub(crate) const REALTEK_CSI_EXPIRY_MS: u64 = 30_000;

/// The origin of a Realtek CSI frame while no authenticated source binding
/// exists. The UDP receiver is unauthenticated (ADR-296), so `unverified` is
/// the only honest value (issue #1950).
pub(crate) const REALTEK_CSI_ORIGIN_UNVERIFIED: &str = "unverified";

/// The freshness of a published snapshot, as a consumer sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Freshness {
    /// Newer than [`REALTEK_CSI_FRESH_MS`].
    Fresh,
    /// Older than [`REALTEK_CSI_FRESH_MS`] and younger than
    /// [`REALTEK_CSI_EXPIRY_MS`].
    Stale,
    /// Older than [`REALTEK_CSI_EXPIRY_MS`]: no longer a measurement.
    Offline,
}

impl Freshness {
    /// The freshness of a snapshot of the given age.
    pub(crate) fn of_age(age_ms: u64) -> Self {
        if age_ms > REALTEK_CSI_EXPIRY_MS {
            Self::Offline
        } else if age_ms > REALTEK_CSI_FRESH_MS {
            Self::Stale
        } else {
            Self::Fresh
        }
    }
}

/// Why an inbound Realtek CSI frame may not update the published snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RejectReason {
    /// The frame carries `csi_valid = false`.
    InvalidCsi,
    /// The frame's sequence is not newer than the last accepted one.
    ReplayOrOutOfOrder,
}

impl RejectReason {
    /// The stable wire name of the reason.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidCsi => "csi_invalid",
            Self::ReplayOrOutOfOrder => "replay_or_out_of_order",
        }
    }
}

/// Whether `sequence` is newer than `last` under the documented wrap policy.
///
/// The RAC1 sequence counter is a `u32` that wraps. A frame counts as newer
/// when it differs from the last accepted value and the distance is below
/// half the counter space, so a wrapped counter still reads as newer while a
/// replayed or reordered frame does not.
pub(crate) fn is_newer_sequence(sequence: u32, last: u32) -> bool {
    sequence != last && sequence.wrapping_sub(last) < 0x8000_0000
}

/// Decide whether an inbound frame may update the published snapshot.
///
/// A frame must carry a valid CSI payload and must be newer than the last
/// accepted frame for its node, so a replayed or reordered datagram never
/// reaches the state or the broadcast (issue #1950).
pub(crate) fn accept_frame(
    frame: &CsiFrame,
    last_sequence: Option<u32>,
) -> Result<(), RejectReason> {
    if !frame.csi_valid {
        return Err(RejectReason::InvalidCsi);
    }
    if let Some(last) = last_sequence {
        if !is_newer_sequence(frame.sequence, last) {
            return Err(RejectReason::ReplayOrOutOfOrder);
        }
    }
    Ok(())
}

/// The current wall-clock time in unix milliseconds.
pub(crate) fn now_unix_ms() -> u64 {
    chrono::Utc::now().timestamp_millis().max(0) as u64
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct RealtekCsiSnapshot {
    pub event_type: &'static str,
    /// The stream identity. The `:simulated` suffix reflects the payload's own
    /// synthetic flag: it can only make the data look more synthetic, never
    /// less, so it fails safe. It is a claim, not an origin — see `origin`.
    pub source: &'static str,
    /// The authoritative origin of the frame. `unverified` while no
    /// authenticated source binding exists.
    pub origin: &'static str,
    pub csi_mode: &'static str,
    pub node_id: u8,
    pub sequence: u32,
    pub timestamp_us: u32,
    /// When the server received the frame, in unix milliseconds.
    pub received_at_unix_ms: u64,
    /// Pseudonym of the peer address. The raw address is never published.
    pub peer_mac_pseudonym: String,
    /// Pseudonym of the trigger address. The raw address is never published.
    pub trig_mac_pseudonym: String,
    pub channel: u8,
    pub bandwidth_mhz: u16,
    pub rx_rate: u8,
    pub protocol_mode: String,
    pub num_sub_carrier: u16,
    pub num_bit_per_tone: u8,
    pub decimation: u8,
    pub rssi_dbm: i8,
    pub rxsc: u8,
    pub csi_valid: bool,
    /// The sender's own synthetic flag. It is a claim, not an origin:
    /// nothing in the unauthenticated payload authenticates it, so the
    /// authoritative origin is `origin`.
    pub payload_claims_synthetic: bool,
    pub mean_amplitude: Option<f32>,
    pub peak_amplitude: Option<f32>,
}

impl RealtekCsiSnapshot {
    pub(crate) fn from_frame(frame: &CsiFrame) -> Self {
        Self::from_frame_at(frame, now_unix_ms())
    }

    pub(crate) fn from_frame_at(frame: &CsiFrame, received_at_unix_ms: u64) -> Self {
        let (mean_amplitude, peak_amplitude) = amplitude_summary(frame);
        let payload_claims_synthetic = frame.flags.contains(CsiFlags::SYNTHETIC);
        Self {
            event_type: "realtek_csi",
            source: if payload_claims_synthetic {
                "realtek_csi:simulated"
            } else {
                "realtek_csi"
            },
            origin: REALTEK_CSI_ORIGIN_UNVERIFIED,
            csi_mode: match frame.csi_mode {
                wifi_densepose_hardware::realtek_csi::CsiMode::Passive => "passive",
                wifi_densepose_hardware::realtek_csi::CsiMode::Active => "active",
                wifi_densepose_hardware::realtek_csi::CsiMode::BoardToBoard => "board_to_board",
            },
            node_id: frame.node_id,
            sequence: frame.sequence,
            timestamp_us: frame.timestamp_us,
            received_at_unix_ms,
            peer_mac_pseudonym: mac_pseudonym(&frame.peer_mac),
            trig_mac_pseudonym: mac_pseudonym(&frame.trig_mac),
            channel: frame.channel,
            bandwidth_mhz: if frame.bandwidth == 1 { 40 } else { 20 },
            rx_rate: frame.rx_rate,
            protocol_mode: format!("{:?}", frame.protocol_mode).to_ascii_lowercase(),
            num_sub_carrier: frame.num_sub_carrier,
            num_bit_per_tone: frame.num_bit_per_tone,
            decimation: frame.decimation,
            rssi_dbm: frame.rssi_dbm,
            rxsc: frame.rxsc,
            csi_valid: frame.csi_valid,
            payload_claims_synthetic,
            mean_amplitude,
            peak_amplitude,
        }
    }
}

/// Per-process salt: a pseudonym is stable inside one run and cannot be
/// correlated across restarts.
fn mac_salt() -> u64 {
    static SALT: OnceLock<u64> = OnceLock::new();
    *SALT.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        nanos ^ (u64::from(std::process::id()) << 32)
    })
}

/// FNV-1a over the per-process salt and the address.
fn mac_pseudonym(mac: &[u8; 6]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in mac_salt().to_le_bytes().iter().chain(mac.iter()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("pm:{hash:016x}")
}

fn amplitude_summary(frame: &CsiFrame) -> (Option<f32>, Option<f32>) {
    if frame.payload.is_empty() {
        return (None, None);
    }
    let amplitudes: Vec<f32> = frame
        .payload
        .iter()
        .map(|(i, q)| (*i as f32).hypot(*q as f32))
        .collect();
    let mean = amplitudes.iter().sum::<f32>() / amplitudes.len() as f32;
    let peak = amplitudes.into_iter().max_by(f32::total_cmp);
    (Some(mean), peak)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_hardware::realtek_csi::simulator::{RealtekCsiSimulator, SimulatorConfig};

    fn simulated_frame() -> CsiFrame {
        let mut sim = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        sim.next_frame()
    }

    #[test]
    fn simulator_summary_preserves_dimensions_and_provenance() {
        let mut sim = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let snapshot = RealtekCsiSnapshot::from_frame(&sim.next_frame());
        assert_eq!(snapshot.num_sub_carrier, 52);
        assert_eq!(snapshot.csi_mode, "passive");
        assert!(snapshot.mean_amplitude.unwrap() > 0.0);
        assert!(snapshot.peak_amplitude.unwrap() >= snapshot.mean_amplitude.unwrap());
    }

    #[test]
    fn source_string_is_distinct_from_realtek_radar() {
        let mut sim = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let snapshot = RealtekCsiSnapshot::from_frame(&sim.next_frame());
        // Must never collapse to the bare "realtek"/"realtek:simulated" source
        // strings already owned by the RTL8720F radar path (realtek_radar.rs) —
        // effective_source()'s prefix matching in main.rs depends on this.
        assert_ne!(snapshot.source, "realtek");
        assert_ne!(snapshot.source, "realtek:simulated");
        assert!(snapshot.source.starts_with("realtek_csi"));
    }

    #[test]
    fn origin_is_unverified_and_never_taken_from_the_payload_flag() {
        // The simulator sets the SYNTHETIC flag. The published origin must not
        // follow it: the payload is unauthenticated (ADR-323).
        let snapshot = RealtekCsiSnapshot::from_frame(&simulated_frame());
        assert_eq!(snapshot.origin, REALTEK_CSI_ORIGIN_UNVERIFIED);
        assert!(snapshot.payload_claims_synthetic);
        // The claim can only make the data look more synthetic, never less.
        assert_eq!(snapshot.source, "realtek_csi:simulated");
    }

    #[test]
    fn a_frame_that_does_not_claim_synthetic_is_still_unverified() {
        let mut frame = simulated_frame();
        frame.flags = wifi_densepose_hardware::realtek_csi::CsiFlags(0);
        let snapshot = RealtekCsiSnapshot::from_frame(&frame);
        assert!(!snapshot.payload_claims_synthetic);
        assert_eq!(snapshot.source, "realtek_csi");
        // A sender cannot make the server vouch for its frames.
        assert_eq!(snapshot.origin, REALTEK_CSI_ORIGIN_UNVERIFIED);
    }

    #[test]
    fn raw_addresses_are_never_published() {
        let frame = simulated_frame();
        let snapshot = RealtekCsiSnapshot::from_frame(&frame);
        let raw = frame
            .peer_mac
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":");
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(!json.contains(&raw), "the raw address must not be published");
        assert!(!json.contains("peer_mac\""));
        assert!(snapshot.peer_mac_pseudonym.starts_with("pm:"));
        assert_ne!(snapshot.peer_mac_pseudonym, snapshot.trig_mac_pseudonym);
    }

    #[test]
    fn the_pseudonym_is_stable_inside_one_run() {
        let frame = simulated_frame();
        let first = RealtekCsiSnapshot::from_frame(&frame);
        let second = RealtekCsiSnapshot::from_frame(&frame);
        assert_eq!(first.peer_mac_pseudonym, second.peer_mac_pseudonym);
    }

    #[test]
    fn freshness_follows_the_age_of_the_snapshot() {
        assert_eq!(Freshness::of_age(0), Freshness::Fresh);
        assert_eq!(Freshness::of_age(REALTEK_CSI_FRESH_MS), Freshness::Fresh);
        assert_eq!(Freshness::of_age(REALTEK_CSI_FRESH_MS + 1), Freshness::Stale);
        assert_eq!(Freshness::of_age(REALTEK_CSI_EXPIRY_MS), Freshness::Stale);
        assert_eq!(Freshness::of_age(REALTEK_CSI_EXPIRY_MS + 1), Freshness::Offline);
    }

    #[test]
    fn a_frame_with_csi_valid_false_is_rejected() {
        let mut frame = simulated_frame();
        frame.csi_valid = false;
        assert_eq!(accept_frame(&frame, None), Err(RejectReason::InvalidCsi));
        assert_eq!(
            RejectReason::InvalidCsi.as_str(),
            "csi_invalid"
        );
    }

    #[test]
    fn a_replayed_or_reordered_sequence_is_rejected() {
        let mut frame = simulated_frame();
        frame.sequence = 10;
        assert_eq!(accept_frame(&frame, Some(9)), Ok(()));
        assert_eq!(
            accept_frame(&frame, Some(10)),
            Err(RejectReason::ReplayOrOutOfOrder)
        );
        assert_eq!(
            accept_frame(&frame, Some(11)),
            Err(RejectReason::ReplayOrOutOfOrder)
        );
    }

    #[test]
    fn a_wrapped_sequence_still_counts_as_newer() {
        assert!(is_newer_sequence(5, u32::MAX));
        assert!(!is_newer_sequence(u32::MAX, 5));
        assert!(!is_newer_sequence(7, 7));
    }
}
