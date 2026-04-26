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
//! 18      2     Reserved
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

use crate::csi_frame::{AntennaConfig, Bandwidth, CsiFrame, CsiMetadata, SubcarrierData};
use crate::error::ParseError;

/// ESP32 CSI binary frame magic number (ADR-018).
const ESP32_CSI_MAGIC: u32 = 0xC5110001;

/// ADR-018 header size in bytes (before I/Q data).
const HEADER_SIZE: usize = 20;

/// Maximum valid subcarrier count for ESP32 (80 MHz bandwidth).
const MAX_SUBCARRIERS: usize = 256;

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
        if data.len() < HEADER_SIZE {
            return Err(ParseError::InsufficientData {
                needed: HEADER_SIZE,
                got: data.len(),
            });
        }

        let mut cursor = Cursor::new(data);

        // Magic (offset 0, 4 bytes)
        let magic = cursor.read_u32::<LittleEndian>().map_err(|_| ParseError::InsufficientData {
            needed: 4,
            got: 0,
        })?;

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
        let n_subcarriers = cursor.read_u16::<LittleEndian>().map_err(|_| ParseError::ByteError {
            offset: 6,
            message: "Failed to read subcarrier count".into(),
        })? as usize;

        if n_subcarriers > MAX_SUBCARRIERS {
            return Err(ParseError::InvalidSubcarrierCount {
                count: n_subcarriers,
                max: MAX_SUBCARRIERS,
            });
        }

        // Frequency MHz (offset 8, 4 bytes LE)
        let channel_freq_mhz = cursor.read_u32::<LittleEndian>().map_err(|_| ParseError::ByteError {
            offset: 8,
            message: "Failed to read frequency".into(),
        })?;

        // Sequence number (offset 12, 4 bytes LE)
        let sequence = cursor.read_u32::<LittleEndian>().map_err(|_| ParseError::ByteError {
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

        // Reserved (offset 18, 2 bytes) — skip
        let _reserved = cursor.read_u16::<LittleEndian>().map_err(|_| ParseError::ByteError {
            offset: 18,
            message: "Failed to read reserved bytes".into(),
        })?;

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

        // Determine bandwidth from subcarrier count
        let bandwidth = match n_subcarriers {
            0..=56 => Bandwidth::Bw20,
            57..=114 => Bandwidth::Bw40,
            115..=242 => Bandwidth::Bw80,
            _ => Bandwidth::Bw160,
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
    fn build_test_frame(node_id: u8, n_antennas: u8, subcarrier_pairs: &[(i8, i8)]) -> Vec<u8> {
        let n_subcarriers = if n_antennas == 0 {
            subcarrier_pairs.len()
        } else {
            subcarrier_pairs.len() / n_antennas as usize
        };

        let mut buf = Vec::new();

        // Magic (offset 0)
        buf.extend_from_slice(&ESP32_CSI_MAGIC.to_le_bytes());
        // Node ID (offset 4)
        buf.push(node_id);
        // Number of antennas (offset 5)
        buf.push(n_antennas);
        // Number of subcarriers (offset 6, LE u16)
        buf.extend_from_slice(&(n_subcarriers as u16).to_le_bytes());
        // Frequency MHz (offset 8, LE u32)
        buf.extend_from_slice(&2437u32.to_le_bytes());
        // Sequence number (offset 12, LE u32)
        buf.extend_from_slice(&1u32.to_le_bytes());
        // RSSI (offset 16, i8)
        buf.push((-50i8) as u8);
        // Noise floor (offset 17, i8)
        buf.push((-95i8) as u8);
        // Reserved (offset 18, 2 bytes)
        buf.extend_from_slice(&[0u8; 2]);
        // I/Q data (offset 20)
        for (i, q) in subcarrier_pairs {
            buf.push(*i as u8);
            buf.push(*q as u8);
        }

        buf
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
        // Corrupt magic
        data[0] = 0xFF;
        let result = Esp32CsiParser::parse_frame(&data);
        assert!(matches!(result, Err(ParseError::InvalidMagic { .. })));
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
}
