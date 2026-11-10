//! ESP32 edge vitals, fused mmWave, and WASM packet definitions and parsers.
//!
//! Complies with ADR-039 (edge vitals magic `0xC511_0002`), ADR-040 (WASM output
//! magic `0xC511_0007`), and ADR-063 (edge fused vitals magic `0xC511_0004`).

use serde::Serialize;

/// Maximum number of persons supported by edge heuristics.
pub const EDGE_MAX_PERSONS: u8 = 4;

/// Sanitize raw person count reported by edge node.
/// Returns `(sanitized_count, is_valid)`.
#[inline]
pub fn sanitize_edge_person_count(presence: bool, raw: u8) -> (u8, bool) {
    if raw > EDGE_MAX_PERSONS || (!presence && raw != 0) {
        return (0, false);
    }
    (raw, true)
}

// ── ESP32 Edge Vitals Packet (ADR-039, magic 0xC511_0002) ────────────────────

/// Decoded vitals packet from ESP32 edge processing pipeline.
#[derive(Debug, Clone, Serialize)]
pub struct Esp32VitalsPacket {
    pub node_id: u8,
    pub presence: bool,
    pub fall_detected: bool,
    pub motion: bool,
    pub breathing_rate_bpm: f64,
    pub heartrate_bpm: f64,
    pub rssi: i8,
    pub n_persons: u8,
    pub person_count_valid: bool,
    pub motion_energy: f32,
    pub presence_score: f32,
    pub timestamp_ms: u32,
}

/// Parse a 32-byte edge vitals packet (magic 0xC511_0002).
pub fn parse_esp32_vitals(buf: &[u8]) -> Option<Esp32VitalsPacket> {
    if buf.len() < 32 {
        return None;
    }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0002 {
        return None;
    }

    let node_id = buf[4];
    let flags = buf[5];
    let breathing_raw = u16::from_le_bytes([buf[6], buf[7]]);
    let heartrate_raw = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let rssi = buf[12] as i8;
    let presence = (flags & 0x01) != 0;
    let (n_persons, person_count_valid) = sanitize_edge_person_count(presence, buf[13]);
    let motion_energy = f32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let presence_score = f32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
    let timestamp_ms = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);

    Some(Esp32VitalsPacket {
        node_id,
        presence,
        fall_detected: (flags & 0x02) != 0,
        motion: (flags & 0x04) != 0,
        breathing_rate_bpm: breathing_raw as f64 / 100.0,
        heartrate_bpm: heartrate_raw as f64 / 10000.0,
        rssi,
        n_persons,
        person_count_valid,
        motion_energy,
        presence_score,
        timestamp_ms,
    })
}

// ── ADR-040: WASM Output Packet (magic 0xC511_0007) ──────────────────────────

/// Single WASM event (type + value).
#[derive(Debug, Clone, Serialize)]
pub struct WasmEvent {
    pub event_type: u8,
    pub value: f32,
}

/// Decoded WASM output packet from ESP32 Tier 3 runtime.
#[derive(Debug, Clone, Serialize)]
pub struct WasmOutputPacket {
    pub node_id: u8,
    pub module_id: u8,
    pub events: Vec<WasmEvent>,
}

/// Parse a WASM output packet (magic 0xC511_0007).
pub fn parse_wasm_output(buf: &[u8]) -> Option<WasmOutputPacket> {
    if buf.len() < 8 {
        return None;
    }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0007 {
        return None;
    }

    let node_id = buf[4];
    let module_id = buf[5];
    let event_count = u16::from_le_bytes([buf[6], buf[7]]) as usize;

    let mut events = Vec::with_capacity(event_count);
    let mut offset = 8;
    for _ in 0..event_count {
        if offset + 5 > buf.len() {
            break;
        }
        let event_type = buf[offset];
        let value = f32::from_le_bytes([
            buf[offset + 1],
            buf[offset + 2],
            buf[offset + 3],
            buf[offset + 4],
        ]);
        events.push(WasmEvent { event_type, value });
        offset += 5;
    }

    Some(WasmOutputPacket {
        node_id,
        module_id,
        events,
    })
}

// ── ADR-063: Edge Fused Vitals Packet (magic 0xC511_0004) ─────────────────────

/// 48-byte packed struct emitted by ESP32-C6 + MR60BHA2 mmWave config.
#[derive(Debug, Clone, Serialize)]
pub struct EdgeFusedVitalsPacket {
    pub node_id: u8,
    pub flags: u8,
    pub breathing_rate_bpm: f32,
    pub heartrate_bpm: f32,
    pub rssi: i8,
    pub n_persons: u8,
    pub person_count_valid: bool,
    pub mmwave_type: u8,
    pub fusion_confidence: u8,
    pub motion_energy: f32,
    pub presence_score: f32,
    pub timestamp_ms: u32,
    pub mmwave_hr_bpm: f32,
    pub mmwave_br_bpm: f32,
    pub mmwave_distance_cm: f32,
    pub mmwave_targets: u8,
    pub mmwave_confidence: u8,
}

/// Parse an ADR-063 edge fused vitals packet (magic 0xC511_0004, 48 bytes).
pub fn parse_edge_fused_vitals(buf: &[u8]) -> Option<EdgeFusedVitalsPacket> {
    if buf.len() < 48 {
        return None;
    }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0004 {
        return None;
    }

    let node_id = buf[4];
    let flags = buf[5];
    let breathing_raw = u16::from_le_bytes([buf[6], buf[7]]);
    let heartrate_raw = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let rssi = buf[12] as i8;
    let any_presence = (flags & 0x09) != 0;
    let (n_persons, person_count_valid) = sanitize_edge_person_count(any_presence, buf[13]);
    let mmwave_type = buf[14];
    let fusion_confidence = buf[15];
    let motion_energy = f32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let presence_score = f32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
    let timestamp_ms = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
    let mmwave_hr_bpm = f32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);
    let mmwave_br_bpm = f32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);
    let mmwave_distance_cm = f32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]);
    let mmwave_targets = buf[40];
    let mmwave_confidence = buf[41];

    Some(EdgeFusedVitalsPacket {
        node_id,
        flags,
        breathing_rate_bpm: breathing_raw as f32 / 100.0,
        heartrate_bpm: heartrate_raw as f32 / 10000.0,
        rssi,
        n_persons,
        person_count_valid,
        mmwave_type,
        fusion_confidence,
        motion_energy,
        presence_score,
        timestamp_ms,
        mmwave_hr_bpm,
        mmwave_br_bpm,
        mmwave_distance_cm,
        mmwave_targets,
        mmwave_confidence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge_packet(presence: bool, n_persons: u8) -> Vec<u8> {
        let mut buf = vec![0u8; 32];
        buf[0..4].copy_from_slice(&0xC511_0002u32.to_le_bytes());
        buf[4] = 4;
        buf[5] = u8::from(presence);
        buf[13] = n_persons;
        buf
    }

    #[test]
    fn contradictory_count_fails_closed() {
        let parsed = parse_esp32_vitals(&edge_packet(false, 4)).expect("valid packet");
        assert_eq!(parsed.n_persons, 0);
        assert!(!parsed.person_count_valid);
    }

    #[test]
    fn present_count_is_preserved() {
        let parsed = parse_esp32_vitals(&edge_packet(true, 3)).expect("valid packet");
        assert_eq!(parsed.n_persons, 3);
        assert!(parsed.person_count_valid);
    }

    fn build_fused_vitals_packet() -> Vec<u8> {
        let mut buf = vec![0u8; 48];
        buf[0..4].copy_from_slice(&0xC511_0004u32.to_le_bytes());
        buf[4] = 9;
        buf[5] = 0b0000_1001;
        buf[6..8].copy_from_slice(&1600u16.to_le_bytes());
        buf[8..12].copy_from_slice(&720_000u32.to_le_bytes());
        buf[12] = (-55i8) as u8;
        buf[13] = 1;
        buf[14] = 2;
        buf[15] = 85;
        buf[16..20].copy_from_slice(&0.42f32.to_le_bytes());
        buf[20..24].copy_from_slice(&0.95f32.to_le_bytes());
        buf[24..28].copy_from_slice(&1_234_567u32.to_le_bytes());
        buf[28..32].copy_from_slice(&71.5f32.to_le_bytes());
        buf[32..36].copy_from_slice(&15.8f32.to_le_bytes());
        buf[36..40].copy_from_slice(&182.0f32.to_le_bytes());
        buf[40] = 1;
        buf[41] = 90;
        buf
    }

    #[test]
    fn parse_edge_fused_vitals_extracts_fields_correctly() {
        let buf = build_fused_vitals_packet();
        let pkt = parse_edge_fused_vitals(&buf).expect("must parse a well-formed packet");
        assert_eq!(pkt.node_id, 9);
        assert_eq!(pkt.flags, 0b0000_1001);
        assert!((pkt.breathing_rate_bpm - 16.0).abs() < 1e-3);
        assert!((pkt.heartrate_bpm - 72.0).abs() < 1e-3);
        assert_eq!(pkt.rssi, -55);
        assert_eq!(pkt.n_persons, 1);
        assert!(pkt.person_count_valid);
        assert_eq!(pkt.mmwave_type, 2);
        assert_eq!(pkt.fusion_confidence, 85);
        assert_eq!(pkt.timestamp_ms, 1_234_567);
    }

    #[test]
    fn parse_edge_fused_vitals_rejects_short_buffer() {
        let buf = build_fused_vitals_packet();
        assert!(parse_edge_fused_vitals(&buf[..47]).is_none());
    }

    #[test]
    fn parse_edge_fused_vitals_rejects_wrong_magic() {
        let mut buf = build_fused_vitals_packet();
        buf[0..4].copy_from_slice(&0xC511_0007u32.to_le_bytes());
        assert!(parse_edge_fused_vitals(&buf).is_none());
    }

    #[test]
    fn parse_wasm_output_rejects_legacy_0004_magic() {
        let buf = build_fused_vitals_packet();
        assert!(parse_wasm_output(&buf).is_none());
    }

    #[test]
    fn parse_wasm_output_accepts_new_0007_magic() {
        let mut buf = vec![0u8; 8];
        buf[0..4].copy_from_slice(&0xC511_0007u32.to_le_bytes());
        buf[4] = 5;
        buf[5] = 1;
        buf[6..8].copy_from_slice(&0u16.to_le_bytes());
        let pkt = parse_wasm_output(&buf).expect("0xC511_0007 must parse");
        assert_eq!(pkt.node_id, 5);
        assert_eq!(pkt.module_id, 1);
        assert!(pkt.events.is_empty());
    }
}

