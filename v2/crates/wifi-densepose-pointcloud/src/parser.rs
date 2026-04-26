//! ADR-018 binary CSI frame parser.
//!
//! Two header magics are accepted: `0xC5110001` (raw CSI, v1) and
//! `0xC5110006` (feature state, v6). The header is 20 bytes; everything
//! after is interleaved I/Q bytes per subcarrier per antenna.
//!
//! Returns `None` when the buffer is truncated or the magic is wrong —
//! this is a hot path (one call per UDP packet) so we prefer Option over
//! a full `anyhow::Error` that would allocate.

const CSI_MAGIC_V6: u32 = 0xC511_0006;
const CSI_MAGIC_V1: u32 = 0xC511_0001;
pub(crate) const CSI_HEADER_SIZE: usize = 20;

/// Accept both header magics — `0xC5110001` (raw CSI) and
/// `0xC5110006` (feature state). Exposed for tests.
#[allow(dead_code)]
pub(crate) const MAGIC_V1: u32 = CSI_MAGIC_V1;
#[allow(dead_code)]
pub(crate) const MAGIC_V6: u32 = CSI_MAGIC_V6;

#[derive(Clone, Debug)]
pub struct CsiFrame {
    pub node_id: u8,
    pub n_antennas: u8,
    pub n_subcarriers: u16,
    pub channel: u8,
    pub rssi: i8,
    pub noise_floor: i8,
    pub timestamp_us: u32,
    /// Raw I/Q data: [I0, Q0, I1, Q1, ...] for each subcarrier
    pub iq_data: Vec<i8>,
    /// Computed amplitude per subcarrier: sqrt(I^2 + Q^2)
    pub amplitudes: Vec<f32>,
    /// Computed phase per subcarrier: atan2(Q, I)
    pub phases: Vec<f32>,
}

/// Parse an ADR-018 binary CSI frame from a UDP packet.
///
/// Returns `None` if:
/// - the buffer is shorter than the 20-byte header
/// - the magic does not match either accepted value
/// - the declared I/Q payload is truncated
pub fn parse_adr018(data: &[u8]) -> Option<CsiFrame> {
    if data.len() < CSI_HEADER_SIZE { return None; }

    let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    if magic != CSI_MAGIC_V6 && magic != CSI_MAGIC_V1 { return None; }

    let node_id = data[4];
    let n_antennas = data[5].max(1);
    let n_subcarriers = u16::from_le_bytes([data[6], data[7]]);
    let channel = data[8];
    let rssi = data[9] as i8;
    let noise_floor = data[10] as i8;
    let timestamp_us = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);

    let iq_len = (n_subcarriers as usize) * 2 * (n_antennas as usize);
    if data.len() < CSI_HEADER_SIZE + iq_len { return None; }

    let iq_data: Vec<i8> = data[CSI_HEADER_SIZE..CSI_HEADER_SIZE + iq_len]
        .iter().map(|&b| b as i8).collect();

    // Compute amplitude and phase per subcarrier (first antenna).
    let mut amplitudes = Vec::with_capacity(n_subcarriers as usize);
    let mut phases = Vec::with_capacity(n_subcarriers as usize);
    for i in 0..n_subcarriers as usize {
        let idx = i * 2;
        if idx + 1 < iq_data.len() {
            let ii = iq_data[idx] as f32;
            let qq = iq_data[idx + 1] as f32;
            amplitudes.push((ii * ii + qq * qq).sqrt());
            phases.push(qq.atan2(ii));
        }
    }

    Some(CsiFrame {
        node_id, n_antennas, n_subcarriers, channel, rssi, noise_floor,
        timestamp_us, iq_data, amplitudes, phases,
    })
}

/// Build a synthetic ADR-018 binary frame. Used by the `csi-test` CLI
/// subcommand and by the unit tests in this module.
pub fn build_test_frame(magic: u32, node_id: u8, n_subcarriers: u16, i: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(CSI_HEADER_SIZE + (n_subcarriers as usize) * 2);
    buf.extend_from_slice(&magic.to_le_bytes());                 // magic (0..4)
    buf.push(node_id);                                           // node_id (4)
    buf.push(1u8);                                               // n_antennas (5)
    buf.extend_from_slice(&n_subcarriers.to_le_bytes());         // n_subcarriers (6..8)
    buf.push(6u8);                                               // channel (8)
    buf.push((-40i8 - (i % 30) as i8) as u8);                    // rssi (9)
    buf.push((-90i8) as u8);                                     // noise_floor (10)
    buf.extend_from_slice(&[0u8; 5]);                            // reserved (11..16)
    buf.extend_from_slice(&(i as u32).to_le_bytes());            // timestamp_us (16..20)
    for j in 0..(n_subcarriers as usize) {
        buf.push(((i + j) as i8).wrapping_mul(3) as u8);
        buf.push(((i + j) as i8).wrapping_mul(5) as u8);
    }
    buf
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_magic_v1_roundtrips() {
        let frame_bytes = build_test_frame(MAGIC_V1, 0x42, 56, 7);
        let frame = parse_adr018(&frame_bytes).expect("v1 frame should parse");
        assert_eq!(frame.node_id, 0x42);
        assert_eq!(frame.n_antennas, 1);
        assert_eq!(frame.n_subcarriers, 56);
        assert_eq!(frame.channel, 6);
        assert_eq!(frame.timestamp_us, 7);
        assert_eq!(frame.iq_data.len(), 56 * 2);
        assert_eq!(frame.amplitudes.len(), 56);
        assert_eq!(frame.phases.len(), 56);
    }

    #[test]
    fn parse_magic_v6_roundtrips() {
        let frame_bytes = build_test_frame(MAGIC_V6, 0x09, 114, 0);
        let frame = parse_adr018(&frame_bytes).expect("v6 frame should parse");
        assert_eq!(frame.node_id, 0x09);
        assert_eq!(frame.n_antennas, 1);
        assert_eq!(frame.n_subcarriers, 114);
        assert_eq!(frame.channel, 6);
        // With i=0, noise_floor=-90 per build_test_frame.
        assert_eq!(frame.noise_floor, -90);
        // With i=0, timestamp_us=0.
        assert_eq!(frame.timestamp_us, 0);
        assert_eq!(frame.iq_data.len(), 114 * 2);
    }

    #[test]
    fn parse_rejects_wrong_magic() {
        let mut bad = build_test_frame(MAGIC_V1, 0, 8, 0);
        // Flip magic to something unrelated.
        bad[0] = 0xFF;
        bad[1] = 0xFF;
        bad[2] = 0xFF;
        bad[3] = 0xFF;
        assert!(parse_adr018(&bad).is_none(), "bad magic should not parse");
    }

    #[test]
    fn parse_rejects_truncated_header() {
        let short = vec![0u8; CSI_HEADER_SIZE - 1];
        assert!(parse_adr018(&short).is_none(), "truncated header must not parse");
    }

    #[test]
    fn parse_rejects_truncated_payload() {
        let mut frame = build_test_frame(MAGIC_V1, 0, 32, 0);
        // Drop half the declared payload.
        frame.truncate(CSI_HEADER_SIZE + 20);
        assert!(parse_adr018(&frame).is_none(), "truncated payload must not parse");
    }
}
