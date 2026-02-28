//! ESP32 CSI frame parser.
//!
//! Parses binary CSI data as produced by ESP-IDF's `wifi_csi_info_t` structure,
//! typically streamed over serial (UART at 921600 baud) or UDP.
//!
//! # ESP32 CSI Binary Format
//!
//! The ESP32 CSI callback produces a buffer with the following layout:
//!
//! ```text
//! Offset  Size  Field
//! ------  ----  -----
//! 0       4     Magic (0xCSI10001 or as configured in firmware)
//! 4       4     Sequence number
//! 8       1     Channel
//! 9       1     Secondary channel
//! 10      1     RSSI (signed)
//! 11      1     Noise floor (signed)
//! 12      2     CSI data length (number of I/Q bytes)
//! 14      6     Source MAC address
//! 20      N     I/Q data (pairs of i8 values, 2 bytes per subcarrier)
//! ```
//!
//! Each subcarrier contributes 2 bytes: one signed byte for I, one for Q.
//! For 20 MHz bandwidth with 56 subcarriers: N = 112 bytes.
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

/// ESP32 CSI binary frame magic number.
///
/// This is a convention for the firmware framing protocol.
/// The actual ESP-IDF callback doesn't include a magic number;
/// our recommended firmware adds this for reliable frame sync.
const ESP32_CSI_MAGIC: u32 = 0xC5110001;

/// Maximum valid subcarrier count for ESP32 (80MHz bandwidth).
const MAX_SUBCARRIERS: usize = 256;

/// Parser for ESP32 CSI binary frames.
pub struct Esp32CsiParser;

impl Esp32CsiParser {
    /// Parse a single CSI frame from a byte buffer.
    ///
    /// The buffer must contain at least the header (20 bytes) plus the I/Q data.
    /// Returns the parsed frame and the number of bytes consumed.
    pub fn parse_frame(data: &[u8]) -> Result<(CsiFrame, usize), ParseError> {
        if data.len() < 20 {
            return Err(ParseError::InsufficientData {
                needed: 20,
                got: data.len(),
            });
        }

        let mut cursor = Cursor::new(data);

        // Read magic
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

        // Sequence number
        let sequence = cursor.read_u32::<LittleEndian>().map_err(|_| ParseError::InsufficientData {
            needed: 8,
            got: 4,
        })?;

        // Channel info
        let channel = cursor.read_u8().map_err(|_| ParseError::ByteError {
            offset: 8,
            message: "Failed to read channel".into(),
        })?;

        let secondary_channel = cursor.read_u8().map_err(|_| ParseError::ByteError {
            offset: 9,
            message: "Failed to read secondary channel".into(),
        })?;

        // RSSI (signed)
        let rssi = cursor.read_i8().map_err(|_| ParseError::ByteError {
            offset: 10,
            message: "Failed to read RSSI".into(),
        })? as i32;

        if rssi > 0 || rssi < -100 {
            return Err(ParseError::InvalidRssi { value: rssi });
        }

        // Noise floor (signed)
        let noise_floor = cursor.read_i8().map_err(|_| ParseError::ByteError {
            offset: 11,
            message: "Failed to read noise floor".into(),
        })? as i32;

        // CSI data length
        let iq_length = cursor.read_u16::<LittleEndian>().map_err(|_| ParseError::ByteError {
            offset: 12,
            message: "Failed to read I/Q length".into(),
        })? as usize;

        // Source MAC
        let mut mac = [0u8; 6];
        for (i, byte) in mac.iter_mut().enumerate() {
            *byte = cursor.read_u8().map_err(|_| ParseError::ByteError {
                offset: 14 + i,
                message: "Failed to read MAC address".into(),
            })?;
        }

        // Validate I/Q length
        let subcarrier_count = iq_length / 2;
        if subcarrier_count > MAX_SUBCARRIERS {
            return Err(ParseError::InvalidSubcarrierCount {
                count: subcarrier_count,
                max: MAX_SUBCARRIERS,
            });
        }

        if iq_length % 2 != 0 {
            return Err(ParseError::IqLengthMismatch {
                expected: subcarrier_count * 2,
                got: iq_length,
            });
        }

        // Check we have enough bytes for the I/Q data
        let total_frame_size = 20 + iq_length;
        if data.len() < total_frame_size {
            return Err(ParseError::InsufficientData {
                needed: total_frame_size,
                got: data.len(),
            });
        }

        // Parse I/Q pairs
        let iq_start = 20;
        let mut subcarriers = Vec::with_capacity(subcarrier_count);

        // Subcarrier index mapping for 20 MHz: -28 to +28 (skipping 0)
        let half = subcarrier_count as i16 / 2;

        for sc_idx in 0..subcarrier_count {
            let byte_offset = iq_start + sc_idx * 2;
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

        // Determine bandwidth from subcarrier count
        let bandwidth = match subcarrier_count {
            0..=56 => Bandwidth::Bw20,
            57..=114 => Bandwidth::Bw40,
            115..=242 => Bandwidth::Bw80,
            _ => Bandwidth::Bw160,
        };

        let frame = CsiFrame {
            metadata: CsiMetadata {
                timestamp: Utc::now(),
                rssi,
                noise_floor,
                channel,
                secondary_channel,
                bandwidth,
                antenna_config: AntennaConfig {
                    tx_antennas: 1,
                    rx_antennas: 1,
                },
                source_mac: Some(mac),
                sequence,
            },
            subcarriers,
        };

        Ok((frame, total_frame_size))
    }

    /// Parse multiple frames from a byte buffer (e.g., from a serial read).
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

    /// Build a valid ESP32 CSI frame with known I/Q values.
    fn build_test_frame(subcarrier_pairs: &[(i8, i8)]) -> Vec<u8> {
        let mut buf = Vec::new();

        // Magic
        buf.extend_from_slice(&ESP32_CSI_MAGIC.to_le_bytes());
        // Sequence
        buf.extend_from_slice(&1u32.to_le_bytes());
        // Channel
        buf.push(6);
        // Secondary channel
        buf.push(0);
        // RSSI
        buf.push((-50i8) as u8);
        // Noise floor
        buf.push((-95i8) as u8);
        // I/Q length
        let iq_len = (subcarrier_pairs.len() * 2) as u16;
        buf.extend_from_slice(&iq_len.to_le_bytes());
        // MAC
        buf.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        // I/Q data
        for (i, q) in subcarrier_pairs {
            buf.push(*i as u8);
            buf.push(*q as u8);
        }

        buf
    }

    #[test]
    fn test_parse_valid_frame() {
        let pairs: Vec<(i8, i8)> = (0..56).map(|i| (i as i8, (i * 2 % 127) as i8)).collect();
        let data = build_test_frame(&pairs);

        let (frame, consumed) = Esp32CsiParser::parse_frame(&data).unwrap();

        assert_eq!(consumed, 20 + 112);
        assert_eq!(frame.subcarrier_count(), 56);
        assert_eq!(frame.metadata.rssi, -50);
        assert_eq!(frame.metadata.channel, 6);
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
        let mut data = build_test_frame(&[(10, 20)]);
        // Corrupt magic
        data[0] = 0xFF;
        let result = Esp32CsiParser::parse_frame(&data);
        assert!(matches!(result, Err(ParseError::InvalidMagic { .. })));
    }

    #[test]
    fn test_amplitude_phase_from_known_iq() {
        let pairs = vec![(100i8, 0i8), (0, 50), (30, 40)];
        let data = build_test_frame(&pairs);
        let (frame, _) = Esp32CsiParser::parse_frame(&data).unwrap();

        let (amps, phases) = frame.to_amplitude_phase();
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
        let frame1 = build_test_frame(&pairs);
        let frame2 = build_test_frame(&pairs);

        let mut combined = Vec::new();
        combined.extend_from_slice(&frame1);
        combined.extend_from_slice(&frame2);

        let (frames, _consumed) = Esp32CsiParser::parse_stream(&combined);
        assert_eq!(frames.len(), 2);
    }

    #[test]
    fn test_parse_stream_with_garbage() {
        let pairs: Vec<(i8, i8)> = (0..4).map(|i| (10 + i, 20 + i)).collect();
        let frame = build_test_frame(&pairs);

        let mut data = Vec::new();
        data.extend_from_slice(&[0xFF, 0xFF, 0xFF]); // garbage
        data.extend_from_slice(&frame);

        let (frames, _) = Esp32CsiParser::parse_stream(&data);
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn test_mac_address_parsed() {
        let pairs = vec![(10i8, 20i8)];
        let data = build_test_frame(&pairs);
        let (frame, _) = Esp32CsiParser::parse_frame(&data).unwrap();

        assert_eq!(
            frame.metadata.source_mac,
            Some([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF])
        );
    }
}
