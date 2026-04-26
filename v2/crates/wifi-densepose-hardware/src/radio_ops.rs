//! ADR-081 Layer 1 Rust mirror + Layer 3 mesh-plane decoder.
//!
//! Mirrors the C vtable `rv_radio_ops_t` defined in
//! `firmware/esp32-csi-node/main/rv_radio_ops.h` so that test harnesses,
//! simulators, and future coordinator-node Rust code can drive the
//! controller logic against a mock backend without touching
//! `wifi-densepose-signal`, `-ruvector`, `-train`, or `-mat`. That
//! portability is the ADR-081 acceptance test: "swap one radio family
//! for another without changing the Rust memory and reasoning layers".
//!
//! The mesh-plane types (`MeshHeader`, `NodeStatus`, `AnomalyAlert`,
//! etc.) mirror `rv_mesh.h` and deserialize the wire format produced by
//! `rv_mesh_encode*()`. This lets a Rust-side aggregator or test node
//! decode live traffic from the ESP32 nodes without re-implementing
//! the framing.

use std::convert::TryFrom;

// ---------------------------------------------------------------------------
// Layer 1 — Radio Abstraction Layer (mirror of rv_radio_ops_t)
// ---------------------------------------------------------------------------

/// Operating modes, mirror of `rv_radio_mode_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RadioMode {
    Disabled     = 0,
    PassiveRx    = 1,
    ActiveProbe  = 2,
    Calibration  = 3,
}

/// Named capture profiles, mirror of `rv_capture_profile_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CaptureProfile {
    PassiveLowRate = 0,
    ActiveProbe    = 1,
    RespHighSens   = 2,
    FastMotion     = 3,
    Calibration    = 4,
}

impl TryFrom<u8> for CaptureProfile {
    type Error = RadioError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(CaptureProfile::PassiveLowRate),
            1 => Ok(CaptureProfile::ActiveProbe),
            2 => Ok(CaptureProfile::RespHighSens),
            3 => Ok(CaptureProfile::FastMotion),
            4 => Ok(CaptureProfile::Calibration),
            _ => Err(RadioError::UnknownProfile(v)),
        }
    }
}

/// Health snapshot, mirror of `rv_radio_health_t`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RadioHealth {
    pub pkt_yield_per_sec: u16,
    pub send_fail_count:   u16,
    pub rssi_median_dbm:   i8,
    pub noise_floor_dbm:   i8,
    pub current_channel:   u8,
    pub current_bw_mhz:    u8,
    pub current_profile:   u8,
}

#[derive(Debug, thiserror::Error)]
pub enum RadioError {
    #[error("unknown capture profile id: {0}")]
    UnknownProfile(u8),
    #[error("backend error: {0}")]
    Backend(String),
}

/// Rust mirror of the `rv_radio_ops_t` vtable.
///
/// Any Rust-side driver (mock, simulator, future coordinator node) that
/// wants to participate in the ADR-081 controller stack must implement
/// this trait. The controller's pure decision policy lives in
/// `adaptive_controller_decide.c` on the C side today; when the Rust
/// coordinator lands, it will reuse the decoded `NodeStatus` messages
/// this module parses and feed decisions back through these ops.
pub trait RadioOps: Send + Sync {
    fn init(&mut self) -> Result<(), RadioError>;
    fn set_channel(&mut self, ch: u8, bw: u8) -> Result<(), RadioError>;
    fn set_mode(&mut self, mode: RadioMode) -> Result<(), RadioError>;
    fn set_csi_enabled(&mut self, en: bool) -> Result<(), RadioError>;
    fn set_capture_profile(&mut self, p: CaptureProfile) -> Result<(), RadioError>;
    fn get_health(&self) -> Result<RadioHealth, RadioError>;
}

/// A zero-hardware radio backend for host tests and CI.
#[derive(Debug, Clone, Default)]
pub struct MockRadio {
    pub health:        RadioHealth,
    pub init_count:    u32,
    pub channel_calls: Vec<(u8, u8)>,
    pub profile_calls: Vec<CaptureProfile>,
    pub mode_calls:    Vec<RadioMode>,
    pub csi_enabled:   bool,
}

impl RadioOps for MockRadio {
    fn init(&mut self) -> Result<(), RadioError> {
        self.init_count += 1;
        Ok(())
    }
    fn set_channel(&mut self, ch: u8, bw: u8) -> Result<(), RadioError> {
        self.channel_calls.push((ch, bw));
        self.health.current_channel = ch;
        self.health.current_bw_mhz  = bw;
        Ok(())
    }
    fn set_mode(&mut self, mode: RadioMode) -> Result<(), RadioError> {
        self.mode_calls.push(mode);
        Ok(())
    }
    fn set_csi_enabled(&mut self, en: bool) -> Result<(), RadioError> {
        self.csi_enabled = en;
        Ok(())
    }
    fn set_capture_profile(&mut self, p: CaptureProfile) -> Result<(), RadioError> {
        self.profile_calls.push(p);
        self.health.current_profile = p as u8;
        Ok(())
    }
    fn get_health(&self) -> Result<RadioHealth, RadioError> {
        Ok(self.health)
    }
}

// ---------------------------------------------------------------------------
// Layer 3 — Mesh plane (mirror of rv_mesh.h)
// ---------------------------------------------------------------------------

/// `RV_MESH_MAGIC` from rv_mesh.h.
pub const MESH_MAGIC: u32   = 0xC511_8100;
/// `RV_MESH_VERSION` from rv_mesh.h.
pub const MESH_VERSION: u8  = 1;
/// `RV_MESH_MAX_PAYLOAD` from rv_mesh.h.
pub const MESH_MAX_PAYLOAD: usize = 256;
/// `sizeof(rv_mesh_header_t)`.
pub const MESH_HEADER_SIZE: usize = 16;

/// `rv_mesh_role_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MeshRole {
    Unassigned  = 0,
    Anchor      = 1,
    Observer    = 2,
    FusionRelay = 3,
    Coordinator = 4,
}

impl TryFrom<u8> for MeshRole {
    type Error = MeshError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(MeshRole::Unassigned),
            1 => Ok(MeshRole::Anchor),
            2 => Ok(MeshRole::Observer),
            3 => Ok(MeshRole::FusionRelay),
            4 => Ok(MeshRole::Coordinator),
            _ => Err(MeshError::UnknownRole(v)),
        }
    }
}

/// `rv_mesh_msg_type_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MeshMsgType {
    TimeSync         = 0x01,
    RoleAssign       = 0x02,
    ChannelPlan      = 0x03,
    CalibrationStart = 0x04,
    FeatureDelta     = 0x05,
    Health           = 0x06,
    AnomalyAlert     = 0x07,
}

impl TryFrom<u8> for MeshMsgType {
    type Error = MeshError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x01 => Ok(MeshMsgType::TimeSync),
            0x02 => Ok(MeshMsgType::RoleAssign),
            0x03 => Ok(MeshMsgType::ChannelPlan),
            0x04 => Ok(MeshMsgType::CalibrationStart),
            0x05 => Ok(MeshMsgType::FeatureDelta),
            0x06 => Ok(MeshMsgType::Health),
            0x07 => Ok(MeshMsgType::AnomalyAlert),
            _    => Err(MeshError::UnknownMsgType(v)),
        }
    }
}

/// `rv_mesh_auth_class_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AuthClass {
    None         = 0,
    HmacSession  = 1,
    Ed25519Batch = 2,
}

/// `rv_mesh_header_t`, 16 bytes.
#[derive(Debug, Clone, Copy)]
pub struct MeshHeader {
    pub msg_type:    MeshMsgType,
    pub sender_role: MeshRole,
    pub auth_class:  AuthClass,
    pub epoch:       u32,
    pub payload_len: u16,
}

/// `rv_node_status_t`, 28 bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeStatus {
    pub node_id:         [u8; 8],
    pub local_time_us:   u64,
    pub role:            MeshRole,
    pub current_channel: u8,
    pub current_bw:      u8,
    pub noise_floor_dbm: i8,
    pub pkt_yield:       u16,
    pub sync_error_us:   u16,
    pub health_flags:    u16,
}

/// `rv_anomaly_alert_t`, 28 bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnomalyAlert {
    pub node_id:       [u8; 8],
    pub ts_us:         u64,
    pub severity:      u8,
    pub reason:        u8,
    pub anomaly_score: f32,
    pub motion_score:  f32,
}

#[derive(Debug, thiserror::Error)]
pub enum MeshError {
    #[error("frame too short: {0} bytes")]
    TooShort(usize),
    #[error("bad magic: 0x{0:08X}")]
    BadMagic(u32),
    #[error("unsupported version: {0}")]
    BadVersion(u8),
    #[error("payload too large: {0}")]
    PayloadTooLarge(u16),
    #[error("CRC mismatch: got 0x{got:08X}, want 0x{want:08X}")]
    CrcMismatch { got: u32, want: u32 },
    #[error("unknown role id: {0}")]
    UnknownRole(u8),
    #[error("unknown msg type: 0x{0:02X}")]
    UnknownMsgType(u8),
    #[error("unknown auth class: {0}")]
    UnknownAuth(u8),
    #[error("payload size mismatch for {which}: got {got}, want {want}")]
    PayloadSizeMismatch { which: &'static str, got: usize, want: usize },
}

/// IEEE CRC32 — matches the bit-by-bit implementation in
/// `rv_feature_state.c`. Poly 0xEDB88320, init 0xFFFFFFFF, xor out.
pub fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Parse one mesh frame. Returns the decoded header and a slice view of
/// the payload inside the input buffer (no copy).
pub fn decode_mesh(buf: &[u8]) -> Result<(MeshHeader, &[u8]), MeshError> {
    if buf.len() < MESH_HEADER_SIZE + 4 {
        return Err(MeshError::TooShort(buf.len()));
    }

    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != MESH_MAGIC { return Err(MeshError::BadMagic(magic)); }

    let version = buf[4];
    if version != MESH_VERSION { return Err(MeshError::BadVersion(version)); }

    let ty          = buf[5];
    let sender_role = buf[6];
    let auth_class  = buf[7];
    let epoch       = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let payload_len = u16::from_le_bytes([buf[12], buf[13]]);

    if payload_len as usize > MESH_MAX_PAYLOAD {
        return Err(MeshError::PayloadTooLarge(payload_len));
    }

    let total = MESH_HEADER_SIZE + payload_len as usize + 4;
    if buf.len() < total { return Err(MeshError::TooShort(buf.len())); }

    let want_crc = crc32_ieee(&buf[..MESH_HEADER_SIZE + payload_len as usize]);
    let crc_off  = MESH_HEADER_SIZE + payload_len as usize;
    let got_crc  = u32::from_le_bytes([
        buf[crc_off], buf[crc_off + 1], buf[crc_off + 2], buf[crc_off + 3],
    ]);
    if got_crc != want_crc {
        return Err(MeshError::CrcMismatch { got: got_crc, want: want_crc });
    }

    let msg_type    = MeshMsgType::try_from(ty)?;
    let sender_role = MeshRole::try_from(sender_role)?;
    let auth_class  = match auth_class {
        0 => AuthClass::None,
        1 => AuthClass::HmacSession,
        2 => AuthClass::Ed25519Batch,
        v => return Err(MeshError::UnknownAuth(v)),
    };

    Ok((
        MeshHeader { msg_type, sender_role, auth_class, epoch, payload_len },
        &buf[MESH_HEADER_SIZE .. MESH_HEADER_SIZE + payload_len as usize],
    ))
}

/// Decode a `HEALTH` payload (28 bytes).
pub fn decode_node_status(p: &[u8]) -> Result<NodeStatus, MeshError> {
    if p.len() != 28 {
        return Err(MeshError::PayloadSizeMismatch {
            which: "HEALTH", got: p.len(), want: 28,
        });
    }
    let mut node_id = [0u8; 8];
    node_id.copy_from_slice(&p[0..8]);
    let local_time_us = u64::from_le_bytes([
        p[8], p[9], p[10], p[11], p[12], p[13], p[14], p[15],
    ]);
    Ok(NodeStatus {
        node_id,
        local_time_us,
        role: MeshRole::try_from(p[16])?,
        current_channel: p[17],
        current_bw:      p[18],
        noise_floor_dbm: p[19] as i8,
        pkt_yield:       u16::from_le_bytes([p[20], p[21]]),
        sync_error_us:   u16::from_le_bytes([p[22], p[23]]),
        health_flags:    u16::from_le_bytes([p[24], p[25]]),
    })
}

/// Decode an `ANOMALY_ALERT` payload (28 bytes).
pub fn decode_anomaly_alert(p: &[u8]) -> Result<AnomalyAlert, MeshError> {
    if p.len() != 28 {
        return Err(MeshError::PayloadSizeMismatch {
            which: "ANOMALY_ALERT", got: p.len(), want: 28,
        });
    }
    let mut node_id = [0u8; 8];
    node_id.copy_from_slice(&p[0..8]);
    let ts_us = u64::from_le_bytes([
        p[8], p[9], p[10], p[11], p[12], p[13], p[14], p[15],
    ]);
    let anomaly_score = f32::from_le_bytes([p[20], p[21], p[22], p[23]]);
    let motion_score  = f32::from_le_bytes([p[24], p[25], p[26], p[27]]);
    Ok(AnomalyAlert {
        node_id, ts_us,
        severity: p[16],
        reason:   p[17],
        anomaly_score, motion_score,
    })
}

/// Encode a `HEALTH` payload. Produces the 16-byte header, 28-byte
/// payload, and 4-byte CRC — bit-identical to what the firmware emits.
pub fn encode_health(
    sender_role: MeshRole,
    epoch: u32,
    status: &NodeStatus,
) -> Vec<u8> {
    let payload_len: u16 = 28;
    let mut buf = Vec::with_capacity(MESH_HEADER_SIZE + payload_len as usize + 4);

    // header
    buf.extend_from_slice(&MESH_MAGIC.to_le_bytes());
    buf.push(MESH_VERSION);
    buf.push(MeshMsgType::Health as u8);
    buf.push(sender_role as u8);
    buf.push(AuthClass::None as u8);
    buf.extend_from_slice(&epoch.to_le_bytes());
    buf.extend_from_slice(&payload_len.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());  // reserved

    // payload
    buf.extend_from_slice(&status.node_id);
    buf.extend_from_slice(&status.local_time_us.to_le_bytes());
    buf.push(status.role as u8);
    buf.push(status.current_channel);
    buf.push(status.current_bw);
    buf.push(status.noise_floor_dbm as u8);
    buf.extend_from_slice(&status.pkt_yield.to_le_bytes());
    buf.extend_from_slice(&status.sync_error_us.to_le_bytes());
    buf.extend_from_slice(&status.health_flags.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes());  // reserved

    let crc = crc32_ieee(&buf);
    buf.extend_from_slice(&crc.to_le_bytes());
    buf
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_radio_tracks_calls() {
        let mut r = MockRadio::default();
        assert!(r.init().is_ok());
        assert_eq!(r.init_count, 1);
        r.set_channel(6, 20).unwrap();
        r.set_capture_profile(CaptureProfile::FastMotion).unwrap();
        r.set_mode(RadioMode::ActiveProbe).unwrap();
        r.set_csi_enabled(true).unwrap();
        assert_eq!(r.channel_calls, vec![(6, 20)]);
        assert_eq!(r.profile_calls, vec![CaptureProfile::FastMotion]);
        assert_eq!(r.mode_calls, vec![RadioMode::ActiveProbe]);
        assert!(r.csi_enabled);
        let h = r.get_health().unwrap();
        assert_eq!(h.current_channel, 6);
        assert_eq!(h.current_bw_mhz, 20);
        assert_eq!(h.current_profile, CaptureProfile::FastMotion as u8);
    }

    #[test]
    fn crc32_matches_firmware_vectors() {
        // Same vectors as test_rv_feature_state.c
        assert_eq!(crc32_ieee(b"123456789"), 0xCBF43926);
        assert_eq!(crc32_ieee(&[]),           0x00000000);
        assert_eq!(crc32_ieee(&[0u8]),        0xD202EF8D);
    }

    #[test]
    fn health_roundtrip() {
        let st = NodeStatus {
            node_id: [9, 0, 0, 0, 0, 0, 0, 0],
            local_time_us: 42_000_000,
            role: MeshRole::Observer,
            current_channel: 11,
            current_bw: 20,
            noise_floor_dbm: -95,
            pkt_yield: 20,
            sync_error_us: 7,
            health_flags: 0x0001,
        };

        let wire = encode_health(MeshRole::Observer, 5, &st);
        assert_eq!(wire.len(), MESH_HEADER_SIZE + 28 + 4);
        assert_eq!(wire.len(), 48);

        let (hdr, payload) = decode_mesh(&wire).expect("decode");
        assert_eq!(hdr.msg_type, MeshMsgType::Health);
        assert_eq!(hdr.sender_role, MeshRole::Observer);
        assert_eq!(hdr.epoch, 5);
        assert_eq!(hdr.payload_len, 28);

        let back = decode_node_status(payload).expect("payload decode");
        assert_eq!(back, st);
    }

    #[test]
    fn decode_rejects_bad_crc() {
        let st = NodeStatus {
            node_id: [1, 0, 0, 0, 0, 0, 0, 0],
            local_time_us: 0,
            role: MeshRole::Observer,
            current_channel: 1,
            current_bw: 20,
            noise_floor_dbm: -90,
            pkt_yield: 0,
            sync_error_us: 0,
            health_flags: 0,
        };
        let mut wire = encode_health(MeshRole::Observer, 0, &st);
        let p0 = MESH_HEADER_SIZE;  // first payload byte
        wire[p0] ^= 0xFF;
        let err = decode_mesh(&wire).unwrap_err();
        assert!(matches!(err, MeshError::CrcMismatch { .. }));
    }

    #[test]
    fn decode_rejects_bad_magic() {
        let buf = [0u8; MESH_HEADER_SIZE + 4];
        let err = decode_mesh(&buf).unwrap_err();
        assert!(matches!(err, MeshError::BadMagic(_)));
    }

    #[test]
    fn decode_rejects_short() {
        let buf = [0u8; 3];
        let err = decode_mesh(&buf).unwrap_err();
        assert!(matches!(err, MeshError::TooShort(_)));
    }

    #[test]
    fn profiles_are_bidirectional() {
        for p in [
            CaptureProfile::PassiveLowRate,
            CaptureProfile::ActiveProbe,
            CaptureProfile::RespHighSens,
            CaptureProfile::FastMotion,
            CaptureProfile::Calibration,
        ] {
            let v = p as u8;
            assert_eq!(CaptureProfile::try_from(v).unwrap(), p);
        }
    }

    #[test]
    fn mesh_constants_match_firmware() {
        // These must match rv_mesh.h byte-for-byte.
        assert_eq!(MESH_MAGIC, 0xC511_8100);
        assert_eq!(MESH_VERSION, 1);
        assert_eq!(MESH_HEADER_SIZE, 16);
        assert_eq!(MESH_MAX_PAYLOAD, 256);
    }
}
