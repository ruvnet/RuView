//! Multi-node data aggregation.
//!
//! Collects [`NeuralDataPacket`]s from multiple ESP32 nodes and assembles them
//! into a unified [`MultiChannelTimeSeries`] once all nodes have reported for
//! a given time window.

use ruv_neural_core::signal::MultiChannelTimeSeries;
use ruv_neural_core::{Result, RuvNeuralError};

use crate::protocol::NeuralDataPacket;

/// Aggregates data packets from multiple ESP32 sensor nodes.
///
/// Packets are buffered per-node. When every node has contributed at least one
/// packet, [`try_assemble`](NodeAggregator::try_assemble) combines them into a
/// single time series — matching packets by timestamp within the configured
/// sync tolerance.
pub struct NodeAggregator {
    node_count: usize,
    buffers: Vec<Vec<NeuralDataPacket>>,
    sync_tolerance_us: u64,
}

impl NodeAggregator {
    /// Create a new aggregator expecting `node_count` distinct nodes.
    pub fn new(node_count: usize) -> Self {
        Self {
            node_count,
            buffers: vec![Vec::new(); node_count],
            sync_tolerance_us: 1_000, // 1 ms default
        }
    }

    /// Buffer a packet from a specific node.
    pub fn receive_packet(
        &mut self,
        node_id: usize,
        packet: NeuralDataPacket,
    ) -> Result<()> {
        if node_id >= self.node_count {
            return Err(RuvNeuralError::Sensor(format!(
                "Node ID {node_id} out of range (max {})",
                self.node_count - 1
            )));
        }
        self.buffers[node_id].push(packet);
        Ok(())
    }

    /// Try to assemble a [`MultiChannelTimeSeries`] from the buffered packets.
    ///
    /// Returns `Some` when every node has at least one packet whose timestamps
    /// are within `sync_tolerance_us` of each other. The matching packets are
    /// consumed from the buffers.
    pub fn try_assemble(&mut self) -> Option<MultiChannelTimeSeries> {
        // Check that every node has at least one packet
        if self.buffers.iter().any(|b| b.is_empty()) {
            return None;
        }

        // Use the first node's earliest packet as the reference timestamp
        let ref_ts = self.buffers[0][0].header.timestamp_us;

        // Find a matching packet in each buffer
        let mut indices: Vec<usize> = Vec::with_capacity(self.node_count);
        for buf in &self.buffers {
            let found = buf.iter().position(|p| {
                let diff = if p.header.timestamp_us >= ref_ts {
                    p.header.timestamp_us - ref_ts
                } else {
                    ref_ts - p.header.timestamp_us
                };
                diff <= self.sync_tolerance_us
            });
            match found {
                Some(idx) => indices.push(idx),
                None => return None,
            }
        }

        // Remove matched packets and merge channel data
        let mut all_data: Vec<Vec<f64>> = Vec::new();
        let mut sample_rate = 1000.0_f64;

        for (buf_idx, &pkt_idx) in indices.iter().enumerate() {
            let pkt = self.buffers[buf_idx].remove(pkt_idx);
            sample_rate = pkt.header.sample_rate_hz as f64;
            for ch in &pkt.channels {
                let channel_data: Vec<f64> = ch
                    .samples
                    .iter()
                    .map(|&s| s as f64 * ch.scale_factor as f64)
                    .collect();
                all_data.push(channel_data);
            }
        }

        if all_data.is_empty() {
            return None;
        }

        let timestamp = ref_ts as f64 / 1_000_000.0;
        MultiChannelTimeSeries::new(all_data, sample_rate, timestamp).ok()
    }

    /// Set the timestamp tolerance in microseconds for matching packets
    /// across nodes.
    pub fn set_sync_tolerance(&mut self, tolerance_us: u64) {
        self.sync_tolerance_us = tolerance_us;
    }

    /// Returns the number of buffered packets for a given node.
    pub fn buffered_count(&self, node_id: usize) -> usize {
        self.buffers.get(node_id).map_or(0, |b| b.len())
    }

    /// Returns the total number of expected nodes.
    pub fn node_count(&self) -> usize {
        self.node_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ChannelData, NeuralDataPacket, PacketHeader, PACKET_MAGIC, PROTOCOL_VERSION};

    fn make_packet(num_channels: u8, timestamp_us: u64, samples: Vec<i16>) -> NeuralDataPacket {
        let channels = (0..num_channels)
            .map(|id| ChannelData {
                channel_id: id,
                samples: samples.clone(),
                scale_factor: 1.0,
            })
            .collect();

        NeuralDataPacket {
            header: PacketHeader {
                magic: PACKET_MAGIC,
                version: PROTOCOL_VERSION,
                packet_id: 0,
                timestamp_us,
                num_channels,
                samples_per_channel: samples.len() as u16,
                sample_rate_hz: 1000,
            },
            channels,
            quality: vec![255; num_channels as usize],
            checksum: 0,
        }
    }

    #[test]
    fn test_assemble_two_nodes() {
        let mut agg = NodeAggregator::new(2);

        let p0 = make_packet(1, 1000, vec![10, 20, 30]);
        let p1 = make_packet(1, 1000, vec![40, 50, 60]);

        agg.receive_packet(0, p0).unwrap();
        // Only one node has reported — assembly requires all nodes
        assert!(agg.try_assemble().is_none());

        agg.receive_packet(1, p1).unwrap();
        let ts = agg.try_assemble().unwrap();
        assert_eq!(ts.num_channels, 2);
        assert_eq!(ts.num_samples, 3);
        assert!((ts.data[0][0] - 10.0).abs() < 1e-6);
        assert!((ts.data[1][2] - 60.0).abs() < 1e-6);
    }

    #[test]
    fn test_assemble_with_tolerance() {
        let mut agg = NodeAggregator::new(2);
        agg.set_sync_tolerance(500);

        let p0 = make_packet(1, 1000, vec![1, 2]);
        let p1 = make_packet(1, 1400, vec![3, 4]); // Within 500 us tolerance

        agg.receive_packet(0, p0).unwrap();
        agg.receive_packet(1, p1).unwrap();
        assert!(agg.try_assemble().is_some());
    }

    #[test]
    fn test_assemble_exceeds_tolerance() {
        let mut agg = NodeAggregator::new(2);
        agg.set_sync_tolerance(100);

        let p0 = make_packet(1, 1000, vec![1, 2]);
        let p1 = make_packet(1, 2000, vec![3, 4]); // 1000 us apart > 100 us tolerance

        agg.receive_packet(0, p0).unwrap();
        agg.receive_packet(1, p1).unwrap();
        assert!(agg.try_assemble().is_none());
    }

    #[test]
    fn test_receive_invalid_node() {
        let mut agg = NodeAggregator::new(2);
        let p = make_packet(1, 0, vec![1]);
        assert!(agg.receive_packet(5, p).is_err());
    }

    #[test]
    fn test_buffers_consumed_after_assembly() {
        let mut agg = NodeAggregator::new(1);
        let p = make_packet(1, 0, vec![1, 2, 3]);
        agg.receive_packet(0, p).unwrap();
        assert_eq!(agg.buffered_count(0), 1);
        agg.try_assemble().unwrap();
        assert_eq!(agg.buffered_count(0), 0);
    }
}
