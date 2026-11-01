//! ESP32 CSI frame parser (ADR-018 binary format).
//!
//! Parses binary CSI data as produced by ADR-018 compliant firmware,
//! typically streamed over UDP from ESP32/ESP32-S3 nodes.
//!
//! # ADR-018 Binary Frame Format
//!
//! ```text
//! Offset  Size  Field
//! ------  ----  -----
//! 0       4     Magic: 0xC5110001
//! 4       1     Node ID
//! 5       1     Number of antennas
//! 6       2     Number of subcarriers (LE u16)
//! 8       4     Frequency MHz (LE u32)
//! 12      4     Sequence number (LE u32)
//! 16      1     RSSI (i8)
//! 17      1     Noise floor (i8)
//! 18      1     PPDU type (ADR-110: 0=HT/legacy, 1=HE-SU, 2=HE-MU, 3=HE-TB)
//! 19      1     Flags (ADR-110: bit0 bw40, bit2 STBC, bit3 LDPC, bit4 15.4-sync)
//! 20      N*2   I/Q pairs (n_antennas * n_subcarriers * 2 bytes)
//! ```
//!
//! Each I/Q pair is 2 signed bytes: I then Q.
//!
//! # No-Mock Guarantee
//!
//! This parser either successfully parses real bytes or returns a specific
//! `ParseError`. It never generates synthetic data.

use byteorder::{LittleEndian, ReadBytesExt};
use chrono::Utc;
use std::io::Cursor;

use crate::csi_frame::{
    Adr018Flags, AntennaConfig, Bandwidth, CsiFrame, CsiMetadata, PpduType, SubcarrierData,
};
use crate::error::ParseError;

/// ESP32 CSI binary frame magic number (ADR-018).
pub const ESP32_CSI_MAGIC: u32 = 0xC5110001;

// ── Sibling RuView wire packets ──────────────────────────────────────────────
// The ESP32 firmware multiplexes several packet types onto the same UDP port
// as ADR-018 raw CSI frames. A CSI-only consumer will therefore see these
// interleaved with CSI frames. They are *not* corruption — they just need a
// different decoder (or can be skipped). See firmware `rv_feature_state.h`.

/// ADR-039 edge vitals packet (32 bytes: HR/BR/presence).
pub const RUVIEW_VITALS_MAGIC: u32 = 0xC5110002;
/// ADR-069 feature-vector packet.
pub const RUVIEW_FEATURE_MAGIC: u32 = 0xC5110003;
/// ADR-063 fused-vitals packet (multi-sensor fusion).
pub const RUVIEW_FUSED_VITALS_MAGIC: u32 = 0xC5110004;
/// ADR-039 compressed-CSI packet.
pub const RUVIEW_COMPRESSED_CSI_MAGIC: u32 = 0xC5110005;
/// ADR-081 compact feature-state packet (the default upstream payload).
pub const RUVIEW_FEATURE_STATE_MAGIC: u32 = 0xC5110006;
/// ADR-095 / #513 on-device temporal-classification packet.
pub const RUVIEW_TEMPORAL_MAGIC: u32 = 0xC5110007;

/// If `magic` is a recognized RuView wire packet other than the ADR-018 raw
/// CSI frame, return a human-readable name for it; otherwise `None`.
///
/// Used by CSI consumers to distinguish "a sibling packet I should route or
/// skip" from "genuine garbage on the wire".
pub fn ruview_sibling_packet_name(magic: u32) -> Option<&'static str> {
    match magic {
        RUVIEW_VITALS_MAGIC => Some("ADR-039 edge vitals"),
        RUVIEW_FEATURE_MAGIC => Some("ADR-069 feature vector"),
        RUVIEW_FUSED_VITALS_MAGIC => Some("ADR-063 fused vitals"),
        RUVIEW_COMPRESSED_CSI_MAGIC => Some("ADR-039 compressed CSI"),
        RUVIEW_FEATURE_STATE_MAGIC => Some("ADR-081 feature state"),
        RUVIEW_TEMPORAL_MAGIC => Some("ADR-095 temporal classification"),
        _ => None,
    }
}

/// ADR-018 header size in bytes (before I/Q data).
const HEADER_SIZE: usize = 20;

/// Maximum I/Q pairs in one frame (all antennas together). The firmware
/// serializes into `CSI_MAX_FRAME_SIZE = CSI_HEADER_SIZE + 4 * 256 * 2`
/// (`firmware/esp32-csi-node/main/csi_collector.h`) and drops anything larger,
/// so a frame never carries more than 2048 I/Q bytes = 1024 pairs. Real
/// ESP32-S3 frames carry up to 306 subcarriers on one antenna (merged
/// LLTF/HT-LTF buffers), which the old 256-subcarrier cap rejected.
const MAX_IQ_PAIRS: usize = 1024;

/// Maximum antenna count for ESP32.
const MAX_ANTENNAS: u8 = 4;

/// Parser for ESP32 CSI binary frames (ADR-018 format).
pub struct Esp32CsiParser;

impl Esp32CsiParser {
    /// Parse a single CSI frame from a byte buffer.
    ///
    /// The buffer must contain at least the header (20 bytes) plus the I/Q data.
    /// Returns the parsed frame and the number of bytes consumed.
    pub fn parse_frame(data: &[u8]) -> Result<(CsiFrame, usize), ParseError> {
        // A recognized sibling packet (ADR-039 vitals, ADR-081 feature state, …)
        // multiplexed onto the CSI UDP port should be reported as such — not as
        // "insufficient data" or "invalid magic" — so callers can route or skip
        // it. These packets are all >= 4 bytes; classify before the CSI-frame
        // length gate. (RuView#517)
        if data.len() >= 4 {
            let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            if let Some(kind) = ruview_sibling_packet_name(magic) {
                return Err(ParseError::NonCsiPacket { magic, kind });
            }
        }

        if data.len() < HEADER_SIZE {
            return Err(ParseError::InsufficientData {
                needed: HEADER_SIZE,
                got: data.len(),
            });
        }

        let mut cursor = Cursor::new(data);

        // Magic (offset 0, 4 bytes)
        let magic = cursor
            .read_u32::<LittleEndian>()
            .map_err(|_| ParseError::InsufficientData { needed: 4, got: 0 })?;

        if magic != ESP32_CSI_MAGIC {
            return Err(ParseError::InvalidMagic {
                expected: ESP32_CSI_MAGIC,
                got: magic,
            });
        }

        // Node ID (offset 4, 1 byte)
        let node_id = cursor.read_u8().map_err(|_| ParseError::ByteError {
            offset: 4,
            message: "Failed to read node ID".into(),
        })?;

        // Number of antennas (offset 5, 1 byte)
        let n_antennas = cursor.read_u8().map_err(|_| ParseError::ByteError {
            offset: 5,
            message: "Failed to read antenna count".into(),
        })?;

        if n_antennas == 0 || n_antennas > MAX_ANTENNAS {
            return Err(ParseError::InvalidAntennaCount { count: n_antennas });
        }

        // Number of subcarriers (offset 6, 2 bytes LE)
        let n_subcarriers =
            cursor
                .read_u16::<LittleEndian>()
                .map_err(|_| ParseError::ByteError {
                    offset: 6,
                    message: "Failed to read subcarrier count".into(),
                })? as usize;

        let max_subcarriers = MAX_IQ_PAIRS / n_antennas as usize;
        if n_subcarriers > max_subcarriers {
            return Err(ParseError::InvalidSubcarrierCount {
                count: n_subcarriers,
                max: max_subcarriers,
            });
        }

        // Frequency MHz (offset 8, 4 bytes LE)
        let channel_freq_mhz =
            cursor
                .read_u32::<LittleEndian>()
                .map_err(|_| ParseError::ByteError {
                    offset: 8,
                    message: "Failed to read frequency".into(),
                })?;

        // Sequence number (offset 12, 4 bytes LE)
        let sequence = cursor
            .read_u32::<LittleEndian>()
            .map_err(|_| ParseError::ByteError {
                offset: 12,
                message: "Failed to read sequence number".into(),
            })?;

        // RSSI (offset 16, 1 byte signed)
        let rssi_dbm = cursor.read_i8().map_err(|_| ParseError::ByteError {
            offset: 16,
            message: "Failed to read RSSI".into(),
        })?;

        // Noise floor (offset 17, 1 byte signed)
        let noise_floor_dbm = cursor.read_i8().map_err(|_| ParseError::ByteError {
            offset: 17,
            message: "Failed to read noise floor".into(),
        })?;

        // ADR-110: bytes 18-19 carry PPDU type + flags (previously reserved-zero,
        // now opt-in via CONFIG_CSI_FRAME_HE_TAGGING in firmware). Pre-ADR-110
        // firmware sends zeros, which round-trip as PpduType::HtLegacy +
        // Adr018Flags::default() — fully backwards compatible.
        let ppdu_byte = cursor.read_u8().map_err(|_| ParseError::ByteError {
            offset: 18,
            message: "Failed to read PPDU type byte".into(),
        })?;
        let flags_byte = cursor.read_u8().map_err(|_| ParseError::ByteError {
            offset: 19,
            message: "Failed to read flags byte".into(),
        })?;
        let ppdu_type = PpduType::from_byte(ppdu_byte);
        let adr018_flags = Adr018Flags::from_byte(flags_byte);

        // I/Q data: n_antennas * n_subcarriers * 2 bytes
        let iq_pair_count = n_antennas as usize * n_subcarriers;
        let iq_byte_count = iq_pair_count * 2;
        let total_frame_size = HEADER_SIZE + iq_byte_count;

        if data.len() < total_frame_size {
            return Err(ParseError::InsufficientData {
                needed: total_frame_size,
                got: data.len(),
            });
        }

        // Parse I/Q pairs — stored as [ant0_sc0_I, ant0_sc0_Q, ant0_sc1_I, ant0_sc1_Q, ..., ant1_sc0_I, ...]
        let iq_start = HEADER_SIZE;
        let mut subcarriers = Vec::with_capacity(iq_pair_count);

        let half = n_subcarriers as i16 / 2;

        for ant in 0..n_antennas as usize {
            for sc_idx in 0..n_subcarriers {
                let byte_offset = iq_start + (ant * n_subcarriers + sc_idx) * 2;
                let i_val = data[byte_offset] as i8 as i16;
                let q_val = data[byte_offset + 1] as i8 as i16;

                let index = if (sc_idx as i16) < half {
                    -(half - sc_idx as i16)
                } else {
                    sc_idx as i16 - half + 1
                };

                subcarriers.push(SubcarrierData {
                    i: i_val,
                    q: q_val,
                    index,
                });
            }
        }

        // Determine bandwidth from PPDU type + subcarrier count (ADR-110).
        //
        // HE-LTF uses a 4x denser tone grid than HT-LTF on the same channel
        // width: HE20 = 256-FFT (242 active tones), HE40 = 512-FFT (484
        // active). So a 256-bin frame on an HE PPDU is *20 MHz*, not 160.
        // For HE frames the firmware also writes the bandwidth into byte 19
        // bit 0 (see Adr018Flags::bw40) — prefer that when set.
        //
        // HT/legacy keeps the count heuristic, with 64 included in the 20 MHz
        // bucket: ESP32 HT20 CSI delivers the full 64-bin FFT grid (live
        // capture evidence: 148-byte frames = 64 subcarriers on a 20 MHz
        // channel, issue #1005). Larger counts come from merged LLTF/HT-LTF
        // buffers (e.g. 192 = 3 x 64, 306) on 20/40 MHz channels; ESP32-family
        // radios do not use 80/160 MHz, so anything above 64 maps to 40 MHz.
        //
        // CLAIMED: the "HT/legacy count above 64 means 40 MHz" rule is inferred
        // from the datasheet, not measured on these frames. ESP32-C5/C6 are not
        // separately covered by it.
        let bandwidth = if ppdu_type.is_he() {
            if adr018_flags.bw40 || n_subcarriers > 256 {
                Bandwidth::Bw40
            } else {
                Bandwidth::Bw20
            }
        } else {
            match n_subcarriers {
                0..=64 => Bandwidth::Bw20,
                _ => Bandwidth::Bw40,
            }
        };

        let frame = CsiFrame {
            metadata: CsiMetadata {
                timestamp: Utc::now(),
                node_id,
                n_antennas,
                n_subcarriers: n_subcarriers as u16,
                channel_freq_mhz,
                rssi_dbm,
                noise_floor_dbm,
                bandwidth,
                antenna_config: AntennaConfig {
                    tx_antennas: 1,
                    rx_antennas: n_antennas,
                },
                sequence,
                ppdu_type,
                adr018_flags,
            },
            subcarriers,
        };

        Ok((frame, total_frame_size))
    }

    /// Parse multiple frames from a byte buffer (e.g., from a UDP read).
    ///
    /// Returns all successfully parsed frames and the total bytes consumed.
    pub fn parse_stream(data: &[u8]) -> (Vec<CsiFrame>, usize) {
        let mut frames = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            match Self::parse_frame(&data[offset..]) {
                Ok((frame, consumed)) => {
                    frames.push(frame);
                    offset += consumed;
                }
                Err(_) => {
                    // Try to find next magic number for resync
                    offset += 1;
                    while offset + 4 <= data.len() {
                        let candidate = u32::from_le_bytes([
                            data[offset],
                            data[offset + 1],
                            data[offset + 2],
                            data[offset + 3],
                        ]);
                        if candidate == ESP32_CSI_MAGIC {
                            break;
                        }
                        offset += 1;
                    }
                }
            }
        }

        (frames, offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a valid ADR-018 ESP32 CSI frame with known parameters.
    /// PPDU type + flags bytes (offset 18-19) are zero — pre-ADR-110 default,
    /// which round-trips as PpduType::HtLegacy + Adr018Flags::default().
    fn build_test_frame(node_id: u8, n_antennas: u8, subcarrier_pairs: &[(i8, i8)]) -> Vec<u8> {
        build_test_frame_with_he(node_id, n_antennas, subcarrier_pairs, 0, 0)
    }

    /// ADR-110-aware variant: explicit byte 18 (PPDU type) and byte 19 (flags).
    fn build_test_frame_with_he(
        node_id: u8,
        n_antennas: u8,
        subcarrier_pairs: &[(i8, i8)],
        ppdu_byte: u8,
        flags_byte: u8,
    ) -> Vec<u8> {
        let n_subcarriers = if n_antennas == 0 {
            subcarrier_pairs.len()
        } else {
            subcarrier_pairs.len() / n_antennas as usize
        };

        let mut buf = Vec::new();
        buf.extend_from_slice(&ESP32_CSI_MAGIC.to_le_bytes());
        buf.push(node_id);
        buf.push(n_antennas);
        buf.extend_from_slice(&(n_subcarriers as u16).to_le_bytes());
        buf.extend_from_slice(&2437u32.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.push((-50i8) as u8);
        buf.push((-95i8) as u8);
        buf.push(ppdu_byte);
        buf.push(flags_byte);
        for (i, q) in subcarrier_pairs {
            buf.push(*i as u8);
            buf.push(*q as u8);
        }

        buf
    }

    // ── ADR-110: byte 18-19 round-trip tests ─────────────────────────────────

    #[test]
    fn adr110_pre_adr110_firmware_round_trips_as_ht_legacy_default_flags() {
        // Pre-ADR-110 firmware writes zeros to bytes 18-19. The parser must
        // surface that as HtLegacy + default flags so old aggregators see
        // identical behavior to before the extension.
        let data = build_test_frame(1, 1, &[(0, 0); 56]);
        let (frame, _) = Esp32CsiParser::parse_frame(&data).unwrap();
        assert_eq!(frame.metadata.ppdu_type, PpduType::HtLegacy);
        assert_eq!(frame.metadata.adr018_flags, Adr018Flags::default());
        assert!(!frame.metadata.ppdu_type.is_he());
    }

    #[test]
    fn adr110_he_su_ppdu_decodes() {
        let data = build_test_frame_with_he(2, 1, &[(0, 0); 56], /*PPDU*/ 1, /*flags*/ 0);
        let (frame, _) = Esp32CsiParser::parse_frame(&data).unwrap();
        assert_eq!(frame.metadata.ppdu_type, PpduType::HeSu);
        assert!(frame.metadata.ppdu_type.is_he());
    }

    #[test]
    fn adr110_he_mu_he_tb_decode() {
        let mu = build_test_frame_with_he(3, 1, &[(0, 0); 56], 2, 0);
        let tb = build_test_frame_with_he(4, 1, &[(0, 0); 56], 3, 0);
        let (mu_frame, _) = Esp32CsiParser::parse_frame(&mu).unwrap();
        let (tb_frame, _) = Esp32CsiParser::parse_frame(&tb).unwrap();
        assert_eq!(mu_frame.metadata.ppdu_type, PpduType::HeMu);
        assert_eq!(tb_frame.metadata.ppdu_type, PpduType::HeTb);
    }

    #[test]
    fn adr110_unknown_ppdu_byte_decodes_as_unknown() {
        let data = build_test_frame_with_he(5, 1, &[(0, 0); 56], 0xFF, 0);
        let (frame, _) = Esp32CsiParser::parse_frame(&data).unwrap();
        assert_eq!(frame.metadata.ppdu_type, PpduType::Unknown);
    }

    #[test]
    fn adr110_flags_round_trip_all_bits() {
        // All known flag bits set: bw40 + STBC + LDPC + sync + sanitized prefix = 0x3D.
        let data = build_test_frame_with_he(6, 1, &[(0, 0); 56], 1, 0x3D);
        let (frame, _) = Esp32CsiParser::parse_frame(&data).unwrap();
        assert!(frame.metadata.adr018_flags.bw40);
        assert!(frame.metadata.adr018_flags.stbc);
        assert!(frame.metadata.adr018_flags.ldpc);
        assert!(frame.metadata.adr018_flags.ieee802154_sync_valid);
        assert!(frame.metadata.adr018_flags.first_word_sanitized);
        // Round-trip the encoder
        assert_eq!(frame.metadata.adr018_flags.to_byte(), 0x3D);
    }

    #[test]
    fn adr110_ppdu_byte_round_trips_for_known_variants() {
        for v in [PpduType::HtLegacy, PpduType::HeSu, PpduType::HeMu, PpduType::HeTb, PpduType::Unknown] {
            assert_eq!(PpduType::from_byte(v.to_byte()), v, "round-trip failed for {v:?}");
        }
    }

    #[test]
    fn test_parse_valid_frame() {
        // 1 antenna, 56 subcarriers
        let pairs: Vec<(i8, i8)> = (0..56).map(|i| (i as i8, (i * 2 % 127) as i8)).collect();
        let data = build_test_frame(1, 1, &pairs);

        let (frame, consumed) = Esp32CsiParser::parse_frame(&data).unwrap();

        assert_eq!(consumed, HEADER_SIZE + 56 * 2);
        assert_eq!(frame.subcarrier_count(), 56);
        assert_eq!(frame.metadata.node_id, 1);
        assert_eq!(frame.metadata.n_antennas, 1);
        assert_eq!(frame.metadata.n_subcarriers, 56);
        assert_eq!(frame.metadata.rssi_dbm, -50);
        assert_eq!(frame.metadata.channel_freq_mhz, 2437);
        assert_eq!(frame.metadata.bandwidth, Bandwidth::Bw20);
        assert!(frame.is_valid());
    }

    #[test]
    fn test_parse_insufficient_data() {
        let data = &[0u8; 10];
        let result = Esp32CsiParser::parse_frame(data);
        assert!(matches!(result, Err(ParseError::InsufficientData { .. })));
    }

    #[test]
    fn test_parse_invalid_magic() {
        let mut data = build_test_frame(1, 1, &[(10, 20)]);
        // Corrupt magic to a value that isn't any known RuView packet.
        data[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        let result = Esp32CsiParser::parse_frame(&data);
        assert!(matches!(result, Err(ParseError::InvalidMagic { .. })));
    }

    #[test]
    fn test_sibling_vitals_packet_is_not_invalid_magic() {
        // RuView#517: a 32-byte ADR-039 vitals packet (magic 0xC5110002)
        // arrives on the same UDP port as CSI frames. It must be reported as
        // a recognized sibling packet, not a corrupt CSI frame.
        let mut data = vec![0u8; 32];
        data[0..4].copy_from_slice(&RUVIEW_VITALS_MAGIC.to_le_bytes());
        match Esp32CsiParser::parse_frame(&data) {
            Err(ParseError::NonCsiPacket { magic, kind }) => {
                assert_eq!(magic, RUVIEW_VITALS_MAGIC);
                assert_eq!(kind, "ADR-039 edge vitals");
            }
            other => panic!("expected NonCsiPacket, got {other:?}"),
        }
    }

    #[test]
    fn test_all_sibling_magics_classified() {
        for m in [
            RUVIEW_VITALS_MAGIC,
            RUVIEW_FEATURE_MAGIC,
            RUVIEW_FUSED_VITALS_MAGIC,
            RUVIEW_COMPRESSED_CSI_MAGIC,
            RUVIEW_FEATURE_STATE_MAGIC,
            RUVIEW_TEMPORAL_MAGIC,
        ] {
            assert!(
                ruview_sibling_packet_name(m).is_some(),
                "{m:#010x} unclassified"
            );
            let mut data = vec![0u8; 24];
            data[0..4].copy_from_slice(&m.to_le_bytes());
            assert!(
                matches!(
                    Esp32CsiParser::parse_frame(&data),
                    Err(ParseError::NonCsiPacket { .. })
                ),
                "{m:#010x} should parse as NonCsiPacket"
            );
        }
        // The CSI magic itself is not a "sibling".
        assert!(ruview_sibling_packet_name(ESP32_CSI_MAGIC).is_none());
    }

    #[test]
    fn test_amplitude_phase_from_known_iq() {
        let pairs = vec![(100i8, 0i8), (0, 50), (30, 40)];
        let data = build_test_frame(1, 1, &pairs);
        let (frame, _) = Esp32CsiParser::parse_frame(&data).unwrap();

        let (amps, _phases) = frame.to_amplitude_phase();
        assert_eq!(amps.len(), 3);

        // I=100, Q=0 -> amplitude=100
        assert!((amps[0] - 100.0).abs() < 0.01);
        // I=0, Q=50 -> amplitude=50
        assert!((amps[1] - 50.0).abs() < 0.01);
        // I=30, Q=40 -> amplitude=50
        assert!((amps[2] - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_stream_with_multiple_frames() {
        let pairs: Vec<(i8, i8)> = (0..4).map(|i| (10 + i, 20 + i)).collect();
        let frame1 = build_test_frame(1, 1, &pairs);
        let frame2 = build_test_frame(2, 1, &pairs);

        let mut combined = Vec::new();
        combined.extend_from_slice(&frame1);
        combined.extend_from_slice(&frame2);

        let (frames, _consumed) = Esp32CsiParser::parse_stream(&combined);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].metadata.node_id, 1);
        assert_eq!(frames[1].metadata.node_id, 2);
    }

    #[test]
    fn test_parse_stream_with_garbage() {
        let pairs: Vec<(i8, i8)> = (0..4).map(|i| (10 + i, 20 + i)).collect();
        let frame = build_test_frame(1, 1, &pairs);

        let mut data = Vec::new();
        data.extend_from_slice(&[0xFF, 0xFF, 0xFF]); // garbage
        data.extend_from_slice(&frame);

        let (frames, _) = Esp32CsiParser::parse_stream(&data);
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn test_multi_antenna_frame() {
        // 3 antennas, 4 subcarriers each = 12 I/Q pairs total
        let mut pairs = Vec::new();
        for ant in 0..3u8 {
            for sc in 0..4u8 {
                pairs.push(((ant * 10 + sc) as i8, ((ant * 10 + sc) * 2) as i8));
            }
        }

        let data = build_test_frame(5, 3, &pairs);
        let (frame, consumed) = Esp32CsiParser::parse_frame(&data).unwrap();

        assert_eq!(consumed, HEADER_SIZE + 12 * 2);
        assert_eq!(frame.metadata.node_id, 5);
        assert_eq!(frame.metadata.n_antennas, 3);
        assert_eq!(frame.metadata.n_subcarriers, 4);
        assert_eq!(frame.subcarrier_count(), 12); // 3 antennas * 4 subcarriers
        assert_eq!(frame.metadata.antenna_config.rx_antennas, 3);
    }

    // ── Real-hardware subcarrier counts ──────────────────────────────────────

    #[test]
    fn s3_306_subcarrier_single_antenna_frame_parses() {
        // ESP32-S3 nodes on firmware 0.8.12 at 2432 MHz emit 1-antenna frames
        // with 306 subcarriers (612 I/Q bytes) alongside 192/128/64 counts.
        let pairs: Vec<(i8, i8)> = (0..306)
            .map(|k| ((k % 7) as i8, -((k % 5) as i8)))
            .collect();
        let data = build_test_frame(2, 1, &pairs);
        assert_eq!(data.len(), HEADER_SIZE + 612);
        let (frame, consumed) = Esp32CsiParser::parse_frame(&data).expect("306-sc frame");
        assert_eq!(consumed, data.len());
        assert_eq!(frame.metadata.n_subcarriers, 306);
        assert_eq!(frame.subcarrier_count(), 306);
        assert_eq!(frame.metadata.rssi_dbm, -50);
    }

    #[test]
    fn ht_bandwidth_never_exceeds_40mhz_for_esp32_counts() {
        // ESP32-family radios use 20/40 MHz channels. Merged-LTF buffers
        // (192 = 3 x 64, 306) must not be classified as 80/160 MHz.
        for &n in &[64usize, 128, 188, 192, 306] {
            let pairs = vec![(1i8, 1i8); n];
            let (frame, _) = Esp32CsiParser::parse_frame(&build_test_frame(1, 1, &pairs))
                .unwrap_or_else(|e| panic!("{n}-sc frame: {e}"));
            let bw = frame.metadata.bandwidth;
            assert!(
                matches!(bw, Bandwidth::Bw20 | Bandwidth::Bw40),
                "{n} subcarriers classified as {bw:?}"
            );
        }
    }

    #[test]
    fn iq_pairs_beyond_firmware_buffer_are_rejected() {
        // Firmware CSI_MAX_FRAME_SIZE leaves 2048 I/Q bytes = 1024 pairs.
        let max = vec![(0i8, 0i8); MAX_IQ_PAIRS];
        assert!(Esp32CsiParser::parse_frame(&build_test_frame(1, 1, &max)).is_ok());
        let over = vec![(0i8, 0i8); MAX_IQ_PAIRS + 1];
        assert!(matches!(
            Esp32CsiParser::parse_frame(&build_test_frame(1, 1, &over)),
            Err(ParseError::InvalidSubcarrierCount { .. })
        ));
        // The bound is on total pairs: 2 antennas x 600 subcarriers = 1200.
        let two_ant = vec![(0i8, 0i8); 1200];
        assert!(matches!(
            Esp32CsiParser::parse_frame(&build_test_frame(1, 2, &two_ant)),
            Err(ParseError::InvalidSubcarrierCount {
                count: 600,
                max: 512
            })
        ));
    }
}
