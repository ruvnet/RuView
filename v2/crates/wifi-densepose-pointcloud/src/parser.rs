//! ADR-018 binary CSI frame parser.
//!
//! Decoding is delegated to `wifi_densepose_hardware::Esp32CsiParser`, the
//! workspace's reference ADR-018 parser, so the byte layout lives in one
//! place. This module only reshapes its output into the per-node history
//! format the pipeline uses (raw I/Q plus first-antenna amplitude/phase).
//!
//! ADR-018 header (firmware `csi_serialize_frame`, little-endian):
//!
//! ```text
//! 0..3   magic 0xC5110001      12..15 sequence (u32)
//! 4      node_id               16     RSSI (i8)
//! 5      n_antennas            17     noise floor (i8)
//! 6..7   n_subcarriers (u16)   18     PPDU type (ADR-110)
//! 8..11  freq_mhz (u32)        19     flags (ADR-110)
//! 20..   I/Q pairs, antenna-major: n_antennas * n_subcarriers * 2 bytes
//! ```
//!
//! Sibling packets multiplexed on the same UDP port (ADR-039 vitals, ADR-081
//! feature state `0xC5110006`, ...) are not CSI frames and are rejected.
//!
//! Returns `None` when the buffer is truncated, the magic is wrong, or the
//! header fails the reference parser's bounds checks — this is a hot path
//! (one call per UDP packet), so callers get an Option, not an error.

use wifi_densepose_hardware::{Esp32CsiParser, ESP32_CSI_MAGIC};

pub(crate) const CSI_HEADER_SIZE: usize = 20;

/// ADR-018 raw CSI magic (`0xC5110001`). Exposed for tests and the
/// `csi-test` synthetic sender.
pub(crate) const MAGIC_V1: u32 = ESP32_CSI_MAGIC;

#[derive(Clone, Debug)]
pub struct CsiFrame {
    pub node_id: u8,
    pub n_antennas: u8,
    pub n_subcarriers: u16,
    /// Channel centre frequency in MHz (header bytes 8..11).
    pub freq_mhz: u32,
    pub rssi: i8,
    pub noise_floor: i8,
    /// Per-node frame sequence counter (header bytes 12..15). ADR-018 carries
    /// no node timestamp.
    pub sequence: u32,
    /// Raw I/Q data, antenna-major: [ant0 I0, Q0, I1, Q1, ..., ant1 I0, ...]
    pub iq_data: Vec<i8>,
    /// Computed amplitude per subcarrier (first antenna): sqrt(I^2 + Q^2)
    pub amplitudes: Vec<f32>,
    /// Computed phase per subcarrier (first antenna): atan2(Q, I)
    pub phases: Vec<f32>,
}

/// Parse an ADR-018 binary CSI frame from a UDP packet.
///
/// Returns `None` if the reference parser rejects the buffer: shorter than
/// the 20-byte header, wrong or sibling-packet magic, antenna/subcarrier
/// count out of range, or a truncated I/Q payload.
pub fn parse_adr018(data: &[u8]) -> Option<CsiFrame> {
    let (frame, _consumed) = Esp32CsiParser::parse_frame(data).ok()?;
    let meta = &frame.metadata;
    let n_sub = meta.n_subcarriers as usize;

    // The reference parser widens each I/Q byte from i8 to i16, so narrowing
    // back is lossless.
    let iq_data: Vec<i8> = frame
        .subcarriers
        .iter()
        .flat_map(|sc| [sc.i as i8, sc.q as i8])
        .collect();

    // Subcarriers are antenna-major, so the first `n_sub` are antenna 0.
    let (amplitudes, phases) = frame
        .subcarriers
        .iter()
        .take(n_sub)
        .map(|sc| {
            let (ii, qq) = (sc.i as f32, sc.q as f32);
            ((ii * ii + qq * qq).sqrt(), qq.atan2(ii))
        })
        .unzip();

    Some(CsiFrame {
        node_id: meta.node_id,
        n_antennas: meta.n_antennas,
        n_subcarriers: meta.n_subcarriers,
        freq_mhz: meta.channel_freq_mhz,
        rssi: meta.rssi_dbm,
        noise_floor: meta.noise_floor_dbm,
        sequence: meta.sequence,
        iq_data,
        amplitudes,
        phases,
    })
}

/// Build a synthetic ADR-018 binary frame (one antenna, 2437 MHz). Used by
/// the `csi-test` CLI subcommand and by the unit tests in this crate.
pub fn build_test_frame(magic: u32, node_id: u8, n_subcarriers: u16, i: usize) -> Vec<u8> {
    let mut buf = Vec::with_capacity(CSI_HEADER_SIZE + (n_subcarriers as usize) * 2);
    buf.extend_from_slice(&magic.to_le_bytes()); // magic (0..4)
    buf.push(node_id); // node_id (4)
    buf.push(1u8); // n_antennas (5)
    buf.extend_from_slice(&n_subcarriers.to_le_bytes()); // n_subcarriers (6..8)
    buf.extend_from_slice(&2437u32.to_le_bytes()); // freq_mhz, channel 6 (8..12)
    buf.extend_from_slice(&(i as u32).to_le_bytes()); // sequence (12..16)
    buf.push((-40i8 - (i % 30) as i8) as u8); // rssi (16)
    buf.push((-90i8) as u8); // noise_floor (17)
    buf.push(0); // PPDU type: HT/legacy (18)
    buf.push(0); // flags (19)
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
    fn parse_test_frame_roundtrips() {
        let frame_bytes = build_test_frame(MAGIC_V1, 0x42, 56, 7);
        let frame = parse_adr018(&frame_bytes).expect("v1 frame should parse");
        assert_eq!(frame.node_id, 0x42);
        assert_eq!(frame.n_antennas, 1);
        assert_eq!(frame.n_subcarriers, 56);
        assert_eq!(frame.freq_mhz, 2437);
        assert_eq!(frame.sequence, 7);
        assert_eq!(frame.rssi, -47);
        assert_eq!(frame.noise_floor, -90);
        assert_eq!(frame.iq_data.len(), 56 * 2);
        assert_eq!(frame.amplitudes.len(), 56);
        assert_eq!(frame.phases.len(), 56);
    }

    #[test]
    fn test_frame_matches_reference_parser() {
        // Cross-check: the synthetic sender emits the layout the workspace's
        // reference ADR-018 parser expects.
        let bytes = build_test_frame(MAGIC_V1, 5, 64, 1234);
        assert_eq!(bytes.len(), CSI_HEADER_SIZE + 64 * 2);
        let (reference, consumed) =
            wifi_densepose_hardware::Esp32CsiParser::parse_frame(&bytes).expect("reference parse");
        assert_eq!(consumed, bytes.len());
        let ours = parse_adr018(&bytes).expect("parse");
        let m = &reference.metadata;
        assert_eq!(ours.node_id, m.node_id);
        assert_eq!(ours.freq_mhz, m.channel_freq_mhz);
        assert_eq!(ours.sequence, m.sequence);
        assert_eq!(ours.rssi, m.rssi_dbm);
        assert_eq!(ours.noise_floor, m.noise_floor_dbm);
        let (amp, _) = reference.to_amplitude_phase();
        let amp32: Vec<f32> = amp.iter().map(|&a| a as f32).collect();
        assert_eq!(ours.amplitudes, amp32);
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
        assert!(
            parse_adr018(&short).is_none(),
            "truncated header must not parse"
        );
    }

    #[test]
    fn parse_rejects_truncated_payload() {
        let mut frame = build_test_frame(MAGIC_V1, 0, 32, 0);
        // Drop half the declared payload.
        frame.truncate(CSI_HEADER_SIZE + 20);
        assert!(
            parse_adr018(&frame).is_none(),
            "truncated payload must not parse"
        );
    }

    // ─── Golden ADR-018 frames, built byte by byte ──────────────────────────
    //
    // Layout per firmware/esp32-csi-node/main/csi_collector.c
    // (csi_serialize_frame) and wifi-densepose-hardware's esp32_parser.rs.

    /// 20-byte ADR-018 header: node 7, 1 antenna, 4 subcarriers, 2437 MHz
    /// (channel 6), sequence 0x01020304, RSSI -52, noise floor -95,
    /// PPDU type 1 (HE-SU), flags 0x01 (bw40).
    const GOLDEN_HEADER: [u8; 20] = [
        0x01, 0x00, 0x11, 0xC5, // 0..3   magic 0xC5110001 (LE)
        0x07, //                   4      node_id
        0x01, //                   5      n_antennas
        0x04, 0x00, //             6..7   n_subcarriers (LE u16) = 4
        0x85, 0x09, 0x00, 0x00, // 8..11  freq_mhz (LE u32) = 2437
        0x04, 0x03, 0x02, 0x01, // 12..15 sequence (LE u32) = 0x01020304
        0xCC, //                   16     rssi (i8) = -52
        0xA1, //                   17     noise_floor (i8) = -95
        0x01, //                   18     PPDU type (ADR-110)
        0x01, //                   19     flags (ADR-110)
    ];

    /// I/Q for one antenna, 4 subcarriers: (3,4) (0,5) (-6,8) (1,0).
    const GOLDEN_IQ_ANT0: [u8; 8] = [0x03, 0x04, 0x00, 0x05, 0xFA, 0x08, 0x01, 0x00];

    fn golden_frame() -> Vec<u8> {
        let mut f = GOLDEN_HEADER.to_vec();
        f.extend_from_slice(&GOLDEN_IQ_ANT0);
        f
    }

    #[test]
    fn golden_frame_reads_rssi_and_noise_floor_at_16_17() {
        let frame = parse_adr018(&golden_frame()).expect("golden frame should parse");
        assert_eq!(frame.node_id, 7);
        assert_eq!(frame.n_antennas, 1);
        assert_eq!(frame.n_subcarriers, 4);
        assert_eq!(frame.rssi, -52, "RSSI is byte 16");
        assert_eq!(frame.noise_floor, -95, "noise floor is byte 17");
        assert_eq!(frame.freq_mhz, 2437, "frequency is u32 at 8..11");
        assert_eq!(frame.sequence, 0x0102_0304, "sequence is u32 at 12..15");
        assert_eq!(frame.iq_data, vec![3, 4, 0, 5, -6, 8, 1, 0]);
        assert_eq!(frame.amplitudes, vec![5.0, 5.0, 10.0, 1.0]);
    }

    #[test]
    fn golden_multi_antenna_frame_is_antenna_major() {
        let mut f = GOLDEN_HEADER.to_vec();
        f[5] = 2; // n_antennas
        f.extend_from_slice(&GOLDEN_IQ_ANT0); // antenna 0, sc 0..3
        f.extend_from_slice(&[0x10, 0x00, 0x10, 0x00, 0x10, 0x00, 0x10, 0x00]); // antenna 1
        let frame = parse_adr018(&f).expect("2-antenna frame should parse");
        assert_eq!(frame.n_antennas, 2);
        assert_eq!(frame.rssi, -52);
        assert_eq!(frame.iq_data.len(), 2 * 4 * 2);
        assert_eq!(&frame.iq_data[8..], &[16, 0, 16, 0, 16, 0, 16, 0]);
        // Amplitude/phase come from antenna 0 only.
        assert_eq!(frame.amplitudes, vec![5.0, 5.0, 10.0, 1.0]);
    }

    #[test]
    fn golden_truncated_frames_do_not_parse() {
        let full = golden_frame();
        // One I/Q byte short.
        assert!(parse_adr018(&full[..full.len() - 1]).is_none());
        // Header cut inside the RSSI/noise-floor bytes.
        assert!(parse_adr018(&full[..17]).is_none());
        // 2 antennas declared, only one antenna of I/Q present.
        let mut two_ant = full.clone();
        two_ant[5] = 2;
        assert!(parse_adr018(&two_ant).is_none());
    }

    #[test]
    fn feature_state_packet_is_not_a_csi_frame() {
        // ADR-081 rv_feature_state_t (60 bytes, magic 0xC5110006) has a
        // different layout: byte 5 is the capture mode and bytes 6..7 are a
        // u16 sequence. Reading it as ADR-018 yields fake subcarriers.
        let mut pkt = vec![0u8; 60];
        pkt[..4].copy_from_slice(&0xC511_0006u32.to_le_bytes());
        pkt[4] = 3; // node_id
        pkt[5] = 1; // mode
        pkt[6..8].copy_from_slice(&10u16.to_le_bytes()); // seq
        assert!(
            parse_adr018(&pkt).is_none(),
            "feature-state packet must not be decoded as CSI"
        );
    }

    #[test]
    fn golden_s3_306_subcarrier_frame_parses() {
        // Real ESP32-S3 frames at 2432 MHz carry 306 subcarriers, 1 antenna.
        let mut f = GOLDEN_HEADER.to_vec();
        f[6..8].copy_from_slice(&306u16.to_le_bytes());
        for k in 0..306u16 {
            f.push(3);
            f.push((k % 4) as u8);
        }
        let frame = parse_adr018(&f).expect("306-subcarrier frame should parse");
        assert_eq!(frame.n_subcarriers, 306);
        assert_eq!(frame.rssi, -52);
        assert_eq!(frame.iq_data.len(), 612);
        assert_eq!(frame.amplitudes.len(), 306);
    }
}
