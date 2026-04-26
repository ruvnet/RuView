//! Communication protocol between ESP32 sensor nodes and the RuVector backend.
//!
//! Defines binary-serializable data packets with CRC32 checksums for reliable
//! transfer over WiFi or UART.

use ruv_neural_core::signal::MultiChannelTimeSeries;
use ruv_neural_core::{Result, RuvNeuralError};
use serde::{Deserialize, Serialize};

/// Magic bytes identifying a rUv Neural data packet.
pub const PACKET_MAGIC: [u8; 4] = [b'r', b'U', b'v', b'N'];

/// Current protocol version.
pub const PROTOCOL_VERSION: u8 = 1;

/// Header of a neural data packet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketHeader {
    /// Magic bytes — must be `b"rUvN"`.
    pub magic: [u8; 4],
    /// Protocol version.
    pub version: u8,
    /// Monotonically increasing packet identifier.
    pub packet_id: u32,
    /// Timestamp in microseconds since boot (or epoch).
    pub timestamp_us: u64,
    /// Number of channels in this packet.
    pub num_channels: u8,
    /// Number of samples per channel.
    pub samples_per_channel: u16,
    /// Sample rate in Hz.
    pub sample_rate_hz: u16,
}

/// Per-channel sample data within a packet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelData {
    /// Channel identifier.
    pub channel_id: u8,
    /// Fixed-point sample values for bandwidth efficiency.
    pub samples: Vec<i16>,
    /// Multiply each sample by this factor to obtain femtotesla.
    pub scale_factor: f32,
}

/// Data packet sent from an ESP32 node to the RuVector backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralDataPacket {
    /// Packet header with metadata.
    pub header: PacketHeader,
    /// Per-channel sample data.
    pub channels: Vec<ChannelData>,
    /// Per-channel signal quality indicator (0 = worst, 255 = best).
    pub quality: Vec<u8>,
    /// CRC32 checksum of the serialized payload (header + channels + quality).
    pub checksum: u32,
}

impl NeuralDataPacket {
    /// Create a new empty packet for the given number of channels.
    pub fn new(num_channels: u8) -> Self {
        Self {
            header: PacketHeader {
                magic: PACKET_MAGIC,
                version: PROTOCOL_VERSION,
                packet_id: 0,
                timestamp_us: 0,
                num_channels,
                samples_per_channel: 0,
                sample_rate_hz: 1000,
            },
            channels: (0..num_channels)
                .map(|id| ChannelData {
                    channel_id: id,
                    samples: Vec::new(),
                    scale_factor: 1.0,
                })
                .collect(),
            quality: vec![255; num_channels as usize],
            checksum: 0,
        }
    }

    /// Serialize the packet to a byte vector (JSON for portability in std
    /// mode; a production ESP32 build would use a compact binary format).
    pub fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }

    /// Deserialize a packet from bytes.
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let packet: NeuralDataPacket = serde_json::from_slice(data).map_err(|e| {
            RuvNeuralError::Serialization(format!("Failed to deserialize packet: {e}"))
        })?;
        if packet.header.magic != PACKET_MAGIC {
            return Err(RuvNeuralError::Serialization(
                "Invalid magic bytes".into(),
            ));
        }
        Ok(packet)
    }

    /// Compute CRC32 checksum of a byte slice using the IEEE polynomial.
    pub fn compute_checksum(data: &[u8]) -> u32 {
        // CRC32 IEEE polynomial lookup-free implementation
        let mut crc: u32 = 0xFFFF_FFFF;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB8_8320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }

    /// Recompute and store the checksum for this packet.
    pub fn update_checksum(&mut self) {
        let mut pkt = self.clone();
        pkt.checksum = 0;
        let bytes = pkt.serialize();
        self.checksum = Self::compute_checksum(&bytes);
    }

    /// Verify that the stored checksum matches the payload.
    pub fn verify_checksum(&self) -> bool {
        let mut pkt = self.clone();
        let stored = pkt.checksum;
        pkt.checksum = 0;
        let bytes = pkt.serialize();
        let computed = Self::compute_checksum(&bytes);
        stored == computed
    }

    /// Convert this packet into a [`MultiChannelTimeSeries`] by scaling the
    /// fixed-point samples back to floating-point femtotesla values.
    pub fn to_multichannel_timeseries(&self) -> Result<MultiChannelTimeSeries> {
        let data: Vec<Vec<f64>> = self
            .channels
            .iter()
            .map(|ch| {
                ch.samples
                    .iter()
                    .map(|&s| s as f64 * ch.scale_factor as f64)
                    .collect()
            })
            .collect();

        let sample_rate = self.header.sample_rate_hz as f64;
        let timestamp = self.header.timestamp_us as f64 / 1_000_000.0;
        MultiChannelTimeSeries::new(data, sample_rate, timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let mut pkt = NeuralDataPacket::new(2);
        pkt.header.packet_id = 42;
        pkt.header.timestamp_us = 123_456_789;
        pkt.header.samples_per_channel = 3;
        pkt.channels[0].samples = vec![100, 200, 300];
        pkt.channels[0].scale_factor = 0.5;
        pkt.channels[1].samples = vec![400, 500, 600];
        pkt.channels[1].scale_factor = 1.0;

        let bytes = pkt.serialize();
        let decoded = NeuralDataPacket::deserialize(&bytes).unwrap();

        assert_eq!(decoded.header.packet_id, 42);
        assert_eq!(decoded.header.num_channels, 2);
        assert_eq!(decoded.channels[0].samples, vec![100, 200, 300]);
        assert_eq!(decoded.channels[1].samples, vec![400, 500, 600]);
    }

    #[test]
    fn test_checksum_verification() {
        let mut pkt = NeuralDataPacket::new(1);
        pkt.channels[0].samples = vec![10, 20, 30];
        pkt.update_checksum();

        assert!(pkt.verify_checksum());

        // Corrupt a value
        pkt.channels[0].samples[0] = 999;
        assert!(!pkt.verify_checksum());
    }

    #[test]
    fn test_to_multichannel_timeseries() {
        let mut pkt = NeuralDataPacket::new(2);
        pkt.header.sample_rate_hz = 500;
        pkt.header.samples_per_channel = 3;
        pkt.channels[0].samples = vec![100, 200, 300];
        pkt.channels[0].scale_factor = 2.0;
        pkt.channels[1].samples = vec![10, 20, 30];
        pkt.channels[1].scale_factor = 0.5;

        let ts = pkt.to_multichannel_timeseries().unwrap();
        assert_eq!(ts.num_channels, 2);
        assert_eq!(ts.num_samples, 3);
        assert!((ts.data[0][0] - 200.0).abs() < 1e-6);
        assert!((ts.data[1][2] - 15.0).abs() < 1e-6);
    }

    #[test]
    fn test_invalid_magic_rejected() {
        let mut pkt = NeuralDataPacket::new(1);
        pkt.header.magic = [0, 0, 0, 0];
        let bytes = pkt.serialize();
        assert!(NeuralDataPacket::deserialize(&bytes).is_err());
    }

    #[test]
    fn test_compute_checksum_deterministic() {
        let data = b"hello world";
        let c1 = NeuralDataPacket::compute_checksum(data);
        let c2 = NeuralDataPacket::compute_checksum(data);
        assert_eq!(c1, c2);
        assert_ne!(c1, 0);
    }
}
