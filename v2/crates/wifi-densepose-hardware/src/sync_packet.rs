//! ADR-110 §A0.12 sync packet decoder (firmware v0.6.9+).
//!
//! Emitted by the firmware on the same UDP socket as ADR-018 CSI frames,
//! distinguished by leading magic `0xC511A110`. Pairs `(node_id, sequence)`
//! across the two UDP streams so a host aggregator can recover mesh-aligned
//! timestamps for every CSI frame — see `WITNESS-LOG-110 §A0.12` for live
//! verification, `archive/v1/src/hardware/csi_extractor.py:SyncPacketParser`
//! for the matching Python decoder.
//!
//! Wire format (32 bytes, little-endian):
//! ```text
//! [0..3]   magic 0xC511A110 (LE u32)
//! [4]      node_id
//! [5]      proto_ver (currently 0x01)
//! [6]      flags: bit 0 = is_leader
//!                 bit 1 = is_valid (fresh sync within VALID_WINDOW_MS)
//!                 bit 2 = smoothed_used (EMA filter active)
//! [7]      minimum free heap / 2048 (0 = unreported)
//! [8..15]  local esp_timer_get_time() (u64)
//! [16..23] mesh-aligned epoch = local + smoothed offset (u64)
//! [24..27] high-water CSI sequence (u32) — pairing key against ADR-018 frames
//! [28]     reset reason (esp_reset_reason_t)
//! [29]     thermal state: 0 ok, 1 warn, 2 throttled, 3 critical
//! [30]     die temperature, signed whole degrees C (0 = unreported)
//! [31]     WiFi transmit ceiling, whole dBm (0 = unreported)
//! ```
//!
//! Recover the per-board offset for a given sync packet as
//! `local_us - epoch_us` (signed). Follower nodes report the EMA-smoothed
//! offset measured in §A0.10; leader nodes report `~0` modulo call-stack
//! elapsed time (`leader_epoch_us = now_us` by definition).

use serde::{Deserialize, Serialize};

use crate::error::ParseError;

/// Magic constant in the first 4 little-endian bytes of every sync packet.
pub const SYNC_PACKET_MAGIC: u32 = 0xC511_A110;
/// Total wire size of a v0.6.9+ sync packet.
pub const SYNC_PACKET_SIZE: usize = 32;
/// Wire size once the node's own MAC is appended (proto v2).
pub const SYNC_PACKET_SIZE_V2: usize = 38;
/// Wire protocol version currently emitted by firmware.
pub const SYNC_PACKET_PROTO_VER: u8 = 0x02;
/// Protocol version that first carried `node_mac`.
pub const SYNC_PACKET_PROTO_VER_MAC: u8 = 0x02;

/// Decoded ADR-110 §A0.12 sync packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncPacket {
    pub node_id: u8,
    pub proto_ver: u8,
    pub flags: SyncPacketFlags,
    /// Node-local `esp_timer_get_time()` snapshot at emission time.
    pub local_us: u64,
    /// Mesh-aligned epoch — `local_us + smoothed_offset`.
    pub epoch_us: u64,
    /// High-water ADR-018 CSI sequence number at emission time. Host
    /// aggregator pairs (`node_id`, `sequence`) across the two UDP streams
    /// to apply the recovered offset back to in-flight CSI frames.
    pub sequence: u32,
    /// The node's own WiFi STA MAC — proto v2 and later, `None` before.
    ///
    /// Without this the server had to GUESS which node transmitted a frame,
    /// using the one signature available: a MAC heard by every receiver
    /// except one is probably that one's. That holds only when every node
    /// hears every other, which is true of boards piled on a desk and false
    /// of boards spread through a house -- exactly the deployment it exists
    /// for. MEASURED 2026-08-31: with nine nodes placed in real positions the
    /// heuristic identified 0 of 32 links, and every peer link was
    /// misreported as infrastructure.
    ///
    /// A node knows its own MAC for free. Telling the server directly turns
    /// an inference that degrades with distance into a fact that does not.
    pub node_mac: Option<[u8; 6]>,
    /// Node health, carried in bytes that were reserved-zero.
    ///
    /// These nodes live on walls with no console, so a serial log reaches
    /// nobody. The sync packet already leaves every node at ~2 Hz on a
    /// priority path and is already stored per node, so health rides along
    /// for free. Zeros from an older node are indistinguishable from "not
    /// reported", which is why every field is an `Option`.
    pub health: NodeHealth,
}

/// Per-node health sampled at sync-emission time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NodeHealth {
    /// `esp_reset_reason()` from the node's last boot. The single most useful
    /// byte here: a node that reboots tells you almost nothing, but one that
    /// reboots with `Brownout` tells you its supply sagged, which on shared
    /// outlets is a wiring fault rather than a firmware one.
    pub reset_reason: u8,
    /// 0 ok, 1 warn, 2 throttled, 3 critical.
    pub thermal_state: u8,
    /// Die temperature, whole degrees C. `None` when unreported.
    pub die_c: Option<i8>,
    /// Current WiFi transmit ceiling in whole dBm.
    ///
    /// Read this before diagnosing a weak link. A thermally throttled node's
    /// RSSI falls at *both* ends, which looks exactly like an obstruction;
    /// only the transmit power distinguishes a hot node from a wall.
    pub tx_dbm: Option<u8>,
    /// Minimum free heap since boot, KiB. A slow leak and an overheat look
    /// identical after ten days unless this has been walking downward.
    pub min_heap_kib: Option<u32>,
}

impl NodeHealth {
    /// Human-readable reset cause, from `esp_reset_reason_t`.
    #[must_use]
    pub fn reset_reason_name(&self) -> &'static str {
        match self.reset_reason {
            0 => "unknown",
            1 => "power-on",
            3 => "software",
            4 => "panic",
            5 => "interrupt watchdog",
            6 => "task watchdog",
            7 => "other watchdog",
            8 => "deep-sleep wake",
            9 => "brownout",
            10 => "sdio",
            _ => "other",
        }
    }

    /// True when the last restart was not a clean one — panic, watchdog or
    /// brownout. These are the ones worth surfacing without being asked.
    #[must_use]
    pub fn rebooted_badly(&self) -> bool {
        matches!(self.reset_reason, 4 | 5 | 6 | 7 | 9)
    }
}

/// Flag bits packed into byte 6 of the sync packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SyncPacketFlags {
    pub is_leader: bool,
    pub is_valid: bool,
    pub smoothed_used: bool,
}

impl SyncPacketFlags {
    pub fn from_byte(b: u8) -> Self {
        Self {
            is_leader: (b & 0x01) != 0,
            is_valid: (b & 0x02) != 0,
            smoothed_used: (b & 0x04) != 0,
        }
    }

    pub fn to_byte(self) -> u8 {
        let mut b = 0u8;
        if self.is_leader { b |= 0x01; }
        if self.is_valid { b |= 0x02; }
        if self.smoothed_used { b |= 0x04; }
        b
    }
}

impl SyncPacket {
    /// Decode a 32-byte sync packet. Returns `ParseError::InvalidMagic` if
    /// the leading u32 doesn't match `SYNC_PACKET_MAGIC` (host should
    /// dispatch on the magic before calling this — see crate-level docs).
    pub fn from_bytes(buf: &[u8]) -> Result<Self, ParseError> {
        if buf.len() < SYNC_PACKET_SIZE {
            return Err(ParseError::InsufficientData {
                needed: SYNC_PACKET_SIZE,
                got: buf.len(),
            });
        }
        let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        if magic != SYNC_PACKET_MAGIC {
            return Err(ParseError::InvalidMagic { expected: SYNC_PACKET_MAGIC, got: magic });
        }
        let node_id = buf[4];
        let proto_ver = buf[5];
        let flags = SyncPacketFlags::from_byte(buf[6]);
        let min_heap = buf[7];
        let local_us = u64::from_le_bytes(buf[8..16].try_into().unwrap());
        let epoch_us = u64::from_le_bytes(buf[16..24].try_into().unwrap());
        let sequence = u32::from_le_bytes(buf[24..28].try_into().unwrap());
        // Length AND version must both agree before trusting the tail: a
        // padded v1 datagram would otherwise yield six bytes of whatever the
        // sender's stack left there, and a plausible-looking wrong MAC is
        // worse than no MAC at all.
        let node_mac = if proto_ver >= SYNC_PACKET_PROTO_VER_MAC
            && buf.len() >= SYNC_PACKET_SIZE_V2
        {
            let mut m = [0u8; 6];
            m.copy_from_slice(&buf[32..38]);
            // All-zero is what an uninitialised buffer looks like, and is not
            // a valid station address.
            if m == [0u8; 6] { None } else { Some(m) }
        } else {
            None
        };
        let health = NodeHealth {
            reset_reason: buf[28],
            thermal_state: buf[29],
            // 0 is "unreported" rather than 0 C: the firmware sends 0 when the
            // sensor has not produced a reading, and a node genuinely at 0 C
            // is not a case this fleet needs to distinguish.
            die_c: if buf[30] == 0 { None } else { Some(buf[30] as i8) },
            tx_dbm: if buf[31] == 0 { None } else { Some(buf[31]) },
            min_heap_kib: if min_heap == 0 { None } else { Some(u32::from(min_heap) * 2) },
        };
        Ok(Self {
            node_id,
            proto_ver,
            flags,
            local_us,
            epoch_us,
            sequence,
            node_mac,
            health,
        })
    }

    /// Recover the signed offset between this node's local monotonic clock
    /// and the mesh epoch (`local_us - epoch_us`). For followers this is
    /// the EMA-smoothed offset; for leaders this is approximately 0 (a few
    /// µs of call-stack elapsed only).
    pub fn local_minus_epoch_us(&self) -> i64 {
        (self.local_us as i64) - (self.epoch_us as i64)
    }

    /// Given a CSI frame's node-local `esp_timer_get_time()` snapshot,
    /// recover the mesh-aligned timestamp using this sync packet as the
    /// reference point.
    ///
    /// Math (all in node-local µs, see ADR-110 §A0.12):
    ///
    /// ```text
    ///   offset           = epoch_us - local_us               (signed; this packet)
    ///   mesh_epoch(frame) = local_at_frame_us + offset
    ///                    = local_at_frame_us + (epoch_us - local_us)
    /// ```
    ///
    /// On the leader this gives `≈ local_at_frame_us`. On a follower this
    /// gives the mesh-aligned time aligned to the leader's clock within
    /// the §A0.10 measured 104 µs stdev (the same EMA-smoothed offset
    /// the firmware applied when it built this sync packet's `epoch_us`).
    ///
    /// Use this on the host side whenever a CSI frame arrives with
    /// ADR-018 byte 19 bit 4 set: look up the matching node's most-recent
    /// `SyncPacket`, call `apply_to_local(frame.local_us)`, stamp the
    /// result on the frame for downstream multistatic fusion.
    pub fn apply_to_local(&self, local_at_frame_us: u64) -> u64 {
        // Compute the offset as a signed delta in the µs domain. Adding it
        // back to the frame's local snapshot recovers the mesh epoch.
        let offset = (self.epoch_us as i64).wrapping_sub(self.local_us as i64);
        (local_at_frame_us as i64).wrapping_add(offset) as u64
    }

    /// Recover the mesh-aligned timestamp for an in-flight CSI frame
    /// **using its ADR-018 sequence number** as the timeline anchor.
    ///
    /// CSI frames carry no per-frame `local_us` field (ADR-018 v1 wire
    /// format reserves no slot for it — see WITNESS-LOG-110 §A0.11),
    /// but they do carry a 32-bit sequence number. The firmware emits
    /// a sync packet alongside CSI frames, stamping the sequence
    /// high-water observed at emit time into [`SyncPacket::sequence`].
    ///
    /// Given a frame's sequence and the node's observed CSI frame rate,
    /// estimate the node-local time at the frame and apply the mesh
    /// offset:
    ///
    /// ```text
    ///   Δframes  = frame_seq - sync.sequence       (wrapping)
    ///   Δus      = Δframes × 1_000_000 / fps_hz    (node-local)
    ///   local_at = sync.local_us + Δus
    ///   mesh     = local_at + (sync.epoch_us - sync.local_us)
    /// ```
    ///
    /// `fps_hz` must be > 0; pass the firmware's `CSI_MIN_SEND_INTERVAL_US`
    /// inverse (≈ 20 fps) or a measured rate from the broadcast-tick task.
    /// The estimate is exact when the frame rate is stable (a node holding
    /// 20 fps within ±1 frame for the sync→frame interval gives
    /// |error| < 1/fps_hz ≈ 50 ms × the per-frame jitter ratio).
    pub fn mesh_aligned_us_for_sequence(&self, frame_seq: u32, fps_hz: f64) -> u64 {
        debug_assert!(fps_hz > 0.0, "fps_hz must be positive");
        let dframes = (frame_seq.wrapping_sub(self.sequence)) as i64;
        let dus = (dframes as f64 * 1_000_000.0 / fps_hz) as i64;
        let local_at = (self.local_us as i64).wrapping_add(dus) as u64;
        self.apply_to_local(local_at)
    }

    /// Serialize back to wire bytes (32 bytes, little-endian).
    pub fn to_bytes(&self) -> [u8; SYNC_PACKET_SIZE] {
        let mut out = [0u8; SYNC_PACKET_SIZE];
        out[0..4].copy_from_slice(&SYNC_PACKET_MAGIC.to_le_bytes());
        out[4] = self.node_id;
        out[5] = self.proto_ver;
        out[6] = self.flags.to_byte();
        out[7] = self.health.min_heap_kib.map_or(0, |k| (k / 2).min(255) as u8);
        out[8..16].copy_from_slice(&self.local_us.to_le_bytes());
        out[16..24].copy_from_slice(&self.epoch_us.to_le_bytes());
        out[24..28].copy_from_slice(&self.sequence.to_le_bytes());
        out[28] = self.health.reset_reason;
        out[29] = self.health.thermal_state;
        out[30] = self.health.die_c.unwrap_or(0) as u8;
        out[31] = self.health.tx_dbm.unwrap_or(0);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduces the COM9 follower sync-pkt #1 captured in WITNESS-LOG-110 §A0.12.
    #[test]
    fn follower_typical_packet_roundtrips() {
        let pkt = SyncPacket {
            node_id: 9,
            proto_ver: 1,
            flags: SyncPacketFlags { is_leader: false, is_valid: true, smoothed_used: true },
            local_us: 28_798_450,
            epoch_us: 27_634_885,
            sequence: 20,
            node_mac: None,
            health: NodeHealth::default(),
        };
        let wire = pkt.to_bytes();
        let decoded = SyncPacket::from_bytes(&wire).unwrap();
        assert_eq!(decoded, pkt);
        // The 1.16-second boot delta §A0.10 measured between COM9 and COM12.
        assert_eq!(decoded.local_minus_epoch_us(), 1_163_565);
        assert_eq!(decoded.flags.to_byte(), 0x06);
    }

    /// COM12 leader case from WITNESS-LOG-110 §A0.12: flags=0x03, epoch ≈ local.
    #[test]
    fn leader_packet_has_local_close_to_epoch() {
        let pkt = SyncPacket {
            node_id: 12,
            proto_ver: 1,
            flags: SyncPacketFlags { is_leader: true, is_valid: true, smoothed_used: false },
            local_us: 28_864_932,
            epoch_us: 28_864_939,
            sequence: 20,
            node_mac: None,
            health: NodeHealth::default(),
        };
        let wire = pkt.to_bytes();
        let decoded = SyncPacket::from_bytes(&wire).unwrap();
        assert_eq!(decoded.flags.to_byte(), 0x03);
        assert_eq!(decoded.local_minus_epoch_us(), -7);  // leader has zero offset modulo call-stack
        assert!(decoded.flags.is_leader);
        assert!(decoded.flags.is_valid);
        assert!(!decoded.flags.smoothed_used);
    }

    /// A v2 packet carries the node's own MAC, which is what lets the server
    /// attribute a peer link instead of guessing the transmitter.
    #[test]
    fn v2_packet_carries_the_node_mac() {
        let mac = [0x9c, 0xcc, 0x01, 0x40, 0x18, 0xb8];
        let mut wire = SyncPacket {
            node_id: 0, proto_ver: SYNC_PACKET_PROTO_VER_MAC,
            flags: SyncPacketFlags::default(),
            local_us: 1, epoch_us: 1, sequence: 7, node_mac: None,
            health: NodeHealth::default(),
        }.to_bytes().to_vec();
        wire.extend_from_slice(&mac);
        let got = SyncPacket::from_bytes(&wire).expect("v2 parses");
        assert_eq!(got.node_mac, Some(mac));
    }

    /// A v1 node emits 32 bytes. If a datagram is padded — by a stack, a
    /// buffer reused between sends, anything — the tail is not a MAC, and a
    /// plausible-looking wrong MAC would mis-attribute every link that node
    /// transmits. Version must agree as well as length.
    #[test]
    fn padded_v1_packet_yields_no_mac() {
        let mut wire = SyncPacket {
            node_id: 3, proto_ver: 0x01, flags: SyncPacketFlags::default(),
            local_us: 1, epoch_us: 1, sequence: 7, node_mac: None,
            health: NodeHealth::default(),
        }.to_bytes().to_vec();
        wire.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef, 0x01, 0x02]);
        let got = SyncPacket::from_bytes(&wire).expect("v1 still parses");
        assert_eq!(got.node_mac, None, "v1 must not report a MAC from padding");
    }

    /// An all-zero tail is what an uninitialised buffer looks like, and is not
    /// a valid station address.
    #[test]
    fn all_zero_mac_is_treated_as_absent() {
        let mut wire = SyncPacket {
            node_id: 4, proto_ver: SYNC_PACKET_PROTO_VER_MAC,
            flags: SyncPacketFlags::default(),
            local_us: 1, epoch_us: 1, sequence: 7, node_mac: None,
            health: NodeHealth::default(),
        }.to_bytes().to_vec();
        wire.extend_from_slice(&[0u8; 6]);
        assert_eq!(SyncPacket::from_bytes(&wire).unwrap().node_mac, None);
    }

    #[test]
    fn magic_mismatch_is_typed_error() {
        let mut wire = SyncPacket {
            node_id: 1, proto_ver: 1, flags: SyncPacketFlags::default(),
            local_us: 0, epoch_us: 0, sequence: 0, node_mac: None, health: NodeHealth::default(),
        }.to_bytes();
        wire[0] = 0x01;  // corrupt magic low byte
        let err = SyncPacket::from_bytes(&wire).unwrap_err();
        match err {
            ParseError::InvalidMagic { got, .. } => assert_ne!(got, SYNC_PACKET_MAGIC),
            other => panic!("expected InvalidMagic, got {other:?}"),
        }
    }

    #[test]
    fn short_packet_is_typed_error() {
        let wire = [0u8; 16];  // half a packet
        let err = SyncPacket::from_bytes(&wire).unwrap_err();
        match err {
            ParseError::InsufficientData { needed, got } => {
                assert_eq!(needed, SYNC_PACKET_SIZE);
                assert_eq!(got, 16);
            }
            other => panic!("expected InsufficientData, got {other:?}"),
        }
    }

    /// Every (leader, valid, smoothed_used) triple round-trips independently.
    #[test]
    fn all_flag_combinations_roundtrip() {
        for &is_leader in &[false, true] {
            for &is_valid in &[false, true] {
                for &smoothed_used in &[false, true] {
                    let flags = SyncPacketFlags { is_leader, is_valid, smoothed_used };
                    let pkt = SyncPacket {
                        node_id: 1, proto_ver: 1, flags,
                        local_us: 1234, epoch_us: 5678, sequence: 99, node_mac: None, health: NodeHealth::default(),
                    };
                    let wire = pkt.to_bytes();
                    let decoded = SyncPacket::from_bytes(&wire).unwrap();
                    assert_eq!(decoded.flags, flags);
                    assert_eq!(decoded.flags.to_byte(), flags.to_byte());
                }
            }
        }
    }

    /// A host dispatches CSI vs sync purely on the leading u32. The two
    /// magics must therefore never collide.
    #[test]
    fn sync_and_csi_magics_differ() {
        assert_ne!(SYNC_PACKET_MAGIC, crate::esp32_parser::ESP32_CSI_MAGIC);
    }

    /// Applying a sync packet to its own local_us must recover its own
    /// epoch_us. Foundational identity for the math.
    #[test]
    fn apply_to_local_recovers_packet_epoch() {
        let pkt = SyncPacket {
            node_id: 9, proto_ver: 1,
            flags: SyncPacketFlags { is_leader: false, is_valid: true, smoothed_used: true },
            local_us: 28_798_450, epoch_us: 27_634_885, sequence: 20, node_mac: None, health: NodeHealth::default(),
        };
        assert_eq!(pkt.apply_to_local(pkt.local_us), pkt.epoch_us);
    }

    /// A CSI frame's local timestamp arriving after the sync packet
    /// gets the same offset applied — the µs delta between sync and frame
    /// is preserved on both clocks.
    #[test]
    fn apply_to_local_preserves_inter_frame_delta() {
        let pkt = SyncPacket {
            node_id: 9, proto_ver: 1,
            flags: SyncPacketFlags { is_leader: false, is_valid: true, smoothed_used: true },
            local_us: 28_798_450, epoch_us: 27_634_885, sequence: 20, node_mac: None, health: NodeHealth::default(),
        };
        // Frame arrives 100 ms after the sync packet on the follower's local clock.
        let local_at_frame = pkt.local_us + 100_000;
        let mesh_epoch = pkt.apply_to_local(local_at_frame);
        // Mesh epoch should also be 100 ms after the sync packet's epoch.
        assert_eq!(mesh_epoch, pkt.epoch_us + 100_000);
        // Offset must equal local - epoch on both clocks.
        assert_eq!(local_at_frame - mesh_epoch, pkt.local_us - pkt.epoch_us);
    }

    /// Leader sync packet has near-zero offset, so apply_to_local is
    /// approximately identity (modulo the few µs call-stack delta).
    #[test]
    fn apply_to_local_on_leader_is_near_identity() {
        let pkt = SyncPacket {
            node_id: 12, proto_ver: 1,
            flags: SyncPacketFlags { is_leader: true, is_valid: true, smoothed_used: false },
            local_us: 28_864_932, epoch_us: 28_864_939, sequence: 20, node_mac: None, health: NodeHealth::default(),
        };
        let frame_local = 30_000_000u64;
        let mesh = pkt.apply_to_local(frame_local);
        assert!((mesh as i64 - frame_local as i64).abs() <= 100,
                "leader apply should be within 100 µs of identity, got {} delta",
                mesh as i64 - frame_local as i64);
    }

    /// At the sync packet's own sequence number, the interpolated mesh
    /// time must equal `epoch_us` exactly.
    #[test]
    fn mesh_aligned_for_sequence_identity_at_sync_point() {
        let pkt = SyncPacket {
            node_id: 9, proto_ver: 1,
            flags: SyncPacketFlags { is_leader: false, is_valid: true, smoothed_used: true },
            local_us: 28_798_450, epoch_us: 27_634_885, sequence: 20, node_mac: None, health: NodeHealth::default(),
        };
        assert_eq!(pkt.mesh_aligned_us_for_sequence(20, 20.0), pkt.epoch_us);
    }

    /// 20 frames after the sync packet at 20 Hz → mesh time advances by 1 s,
    /// preserving the leader/follower clock offset.
    #[test]
    fn mesh_aligned_for_sequence_extrapolates_forward() {
        let pkt = SyncPacket {
            node_id: 9, proto_ver: 1,
            flags: SyncPacketFlags { is_leader: false, is_valid: true, smoothed_used: true },
            local_us: 28_798_450, epoch_us: 27_634_885, sequence: 20, node_mac: None, health: NodeHealth::default(),
        };
        // 20 frames at 20 fps = 1 000 000 µs
        let mesh = pkt.mesh_aligned_us_for_sequence(40, 20.0);
        assert_eq!(mesh, pkt.epoch_us + 1_000_000);
    }

    /// Sequence wraparound (u32 overflow) must extrapolate forward by one
    /// frame, not jump backward by 2^32. The wrapping_sub semantics in
    /// the implementation guard this.
    #[test]
    fn mesh_aligned_for_sequence_handles_seq_wraparound() {
        let pkt = SyncPacket {
            node_id: 9, proto_ver: 1,
            flags: SyncPacketFlags { is_leader: false, is_valid: true, smoothed_used: true },
            local_us: 10_000, epoch_us: 10_000, sequence: u32::MAX, node_mac: None, health: NodeHealth::default(),
        };
        // Next sequence after u32::MAX is 0 (wrap). Δframes = 1, not -2^32.
        let mesh = pkt.mesh_aligned_us_for_sequence(0, 20.0);
        assert_eq!(mesh, pkt.epoch_us + 50_000);  // 1 frame at 20 fps = 50 ms
    }

    /// End-to-end ADR-110 pipeline sanity:
    ///   (1) firmware emits sync packet (bytes built here as a stand-in)
    ///   (2) host wire-decodes via from_bytes
    ///   (3) a CSI frame arrives 100 sequences later (≈ 5 s @ 20 fps)
    ///   (4) mesh_aligned_us_for_sequence recovers its mesh timestamp
    /// Asserts that the recovered mesh time matches sync.epoch_us + Δus exactly,
    /// and cross-checks against apply_to_local. This is the contract every
    /// downstream multistatic-fusion consumer relies on.
    #[test]
    fn end_to_end_sync_decode_then_frame_mesh_recovery() {
        let pkt = SyncPacket {
            node_id: 9,
            proto_ver: 1,
            flags: SyncPacketFlags { is_leader: false, is_valid: true, smoothed_used: true },
            local_us: 28_798_450,
            epoch_us: 27_634_885,
            sequence: 20,
            node_mac: None,
            health: NodeHealth::default(),
        };
        let wire = pkt.to_bytes();
        assert_eq!(wire.len(), SYNC_PACKET_SIZE);
        let decoded = SyncPacket::from_bytes(&wire).unwrap();
        assert_eq!(decoded, pkt);

        // 5 s after sync at 20 fps = 100 frames later
        let frame_seq = pkt.sequence + 100;
        let mesh_us = decoded.mesh_aligned_us_for_sequence(frame_seq, 20.0);
        assert_eq!(mesh_us, pkt.epoch_us + 5_000_000);

        // Same mesh time via direct apply_to_local — both paths must agree
        let local_at_frame = pkt.local_us + 5_000_000;
        assert_eq!(decoded.apply_to_local(local_at_frame), mesh_us);
    }

    #[test]
    fn wire_size_constant_is_correct() {
        let pkt = SyncPacket {
            node_id: 0, proto_ver: 1, flags: SyncPacketFlags::default(),
            local_us: 0, epoch_us: 0, sequence: 0, node_mac: None, health: NodeHealth::default(),
        };
        assert_eq!(pkt.to_bytes().len(), SYNC_PACKET_SIZE);
        assert_eq!(SYNC_PACKET_SIZE, 32);
    }

    /// ADR-110 iter 21 — cross-language wire-format conformance gate.
    ///
    /// These exact bytes are ALSO pinned in the Python test
    /// `test_canonical_wire_bytes_match_rust_decoder` in
    /// `archive/v1/tests/unit/test_esp32_binary_parser.py`. If this
    /// canonical hex stops matching what Python emits for the same
    /// SyncPacket fields, ONE of the decoders has drifted from the wire.
    ///
    /// Canonical packet: COM9 sync-pkt #1 from §A0.12 live capture.
    #[test]
    fn canonical_wire_bytes_match_python_decoder() {
        // Exact bytes matching the Python pin (hex-decoded by hand to bytes).
        let canonical: [u8; 32] = [
            0x10, 0xa1, 0x11, 0xc5,  // magic 0xC511A110 (LE u32)
            0x09,                     // node_id = 9
            0x01,                     // proto_ver = 1
            0x06,                     // flags: bit1=is_valid, bit2=smoothed_used
            0x00,                     // reserved
            0xf2, 0x6d, 0xb7, 0x01, 0x00, 0x00, 0x00, 0x00,  // local_us = 28_798_450
            0xc5, 0xac, 0xa5, 0x01, 0x00, 0x00, 0x00, 0x00,  // epoch_us = 27_634_885
            0x14, 0x00, 0x00, 0x00,  // sequence = 20
            0x00, 0x00, 0x00, 0x00,  // reserved
        ];
        let decoded = SyncPacket::from_bytes(&canonical).unwrap();
        assert_eq!(decoded.node_id, 9);
        assert_eq!(decoded.proto_ver, 1);
        assert_eq!(decoded.flags.to_byte(), 0x06);
        assert!(!decoded.flags.is_leader);
        assert!(decoded.flags.is_valid);
        assert!(decoded.flags.smoothed_used);
        assert_eq!(decoded.local_us, 28_798_450);
        assert_eq!(decoded.epoch_us, 27_634_885);
        assert_eq!(decoded.sequence, 20);
        // §A0.10's measured 1.16-second boot delta.
        assert_eq!(decoded.local_minus_epoch_us(), 1_163_565);

        // Round-trip: re-encoding the decoded struct must produce the same
        // canonical bytes — this is what catches any drift in to_bytes.
        let re_encoded = decoded.to_bytes();
        assert_eq!(re_encoded, canonical,
                   "Rust to_bytes drifted from the canonical pin — Python decoder will break");
    }
}
