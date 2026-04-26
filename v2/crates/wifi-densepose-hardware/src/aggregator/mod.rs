//! UDP aggregator for ESP32 CSI nodes (ADR-018 Layer 2).
//!
//! Receives ADR-018 binary frames over UDP from multiple ESP32 nodes,
//! parses them, tracks per-node state (sequence gaps, drop counting),
//! and forwards parsed `CsiFrame`s to the processing pipeline via an
//! `mpsc` channel.

use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::{self, SyncSender, Receiver};

use crate::csi_frame::CsiFrame;
use crate::esp32_parser::Esp32CsiParser;

/// Configuration for the UDP aggregator.
#[derive(Debug, Clone)]
pub struct AggregatorConfig {
    /// Address to bind the UDP socket to.
    pub bind_addr: String,
    /// Port to listen on.
    pub port: u16,
    /// Channel capacity for the frame sender (0 = unbounded-like behavior via sync).
    pub channel_capacity: usize,
}

impl Default for AggregatorConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0".to_string(),
            port: 5005,
            channel_capacity: 1024,
        }
    }
}

/// Per-node tracking state.
#[derive(Debug)]
struct NodeState {
    /// Last seen sequence number.
    last_sequence: u32,
    /// Total frames received from this node.
    frames_received: u64,
    /// Total dropped frames detected (sequence gaps).
    frames_dropped: u64,
}

impl NodeState {
    fn new(initial_sequence: u32) -> Self {
        Self {
            last_sequence: initial_sequence,
            frames_received: 1,
            frames_dropped: 0,
        }
    }

    /// Update state with a new sequence number. Returns the gap size (0 if contiguous).
    fn update(&mut self, sequence: u32) -> u32 {
        self.frames_received += 1;
        let expected = self.last_sequence.wrapping_add(1);
        let gap = if sequence > expected {
            sequence - expected
        } else {
            0
        };
        self.frames_dropped += gap as u64;
        self.last_sequence = sequence;
        gap
    }
}

/// UDP aggregator that receives CSI frames from ESP32 nodes.
pub struct Esp32Aggregator {
    socket: UdpSocket,
    nodes: HashMap<u8, NodeState>,
    tx: SyncSender<CsiFrame>,
}

impl Esp32Aggregator {
    /// Create a new aggregator bound to the configured address.
    pub fn new(config: &AggregatorConfig) -> io::Result<(Self, Receiver<CsiFrame>)> {
        let addr: SocketAddr = format!("{}:{}", config.bind_addr, config.port)
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let socket = UdpSocket::bind(addr)?;
        let (tx, rx) = mpsc::sync_channel(config.channel_capacity);

        Ok((
            Self {
                socket,
                nodes: HashMap::new(),
                tx,
            },
            rx,
        ))
    }

    /// Create an aggregator from an existing socket (for testing).
    pub fn from_socket(socket: UdpSocket, tx: SyncSender<CsiFrame>) -> Self {
        Self {
            socket,
            nodes: HashMap::new(),
            tx,
        }
    }

    /// Run the blocking receive loop. Call from a dedicated thread.
    pub fn run(&mut self) -> io::Result<()> {
        let mut buf = [0u8; 2048];
        loop {
            let (n, _src) = self.socket.recv_from(&mut buf)?;
            self.handle_packet(&buf[..n]);
        }
    }

    /// Handle a single UDP packet. Public for unit testing.
    pub fn handle_packet(&mut self, data: &[u8]) {
        match Esp32CsiParser::parse_frame(data) {
            Ok((frame, _consumed)) => {
                let node_id = frame.metadata.node_id;
                let seq = frame.metadata.sequence;

                // Track node state
                match self.nodes.get_mut(&node_id) {
                    Some(state) => {
                        state.update(seq);
                    }
                    None => {
                        self.nodes.insert(node_id, NodeState::new(seq));
                    }
                }

                // Send to channel (ignore send errors — receiver may have dropped)
                let _ = self.tx.try_send(frame);
            }
            Err(_) => {
                // Bad packet — silently drop (per ADR-018: aggregator is tolerant)
            }
        }
    }

    /// Get the number of dropped frames for a specific node.
    pub fn drops_for_node(&self, node_id: u8) -> u64 {
        self.nodes.get(&node_id).map_or(0, |s| s.frames_dropped)
    }

    /// Get the number of tracked nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// Helper: build an ADR-018 frame packet for testing.
    fn build_test_packet(node_id: u8, sequence: u32, n_subcarriers: usize) -> Vec<u8> {
        let mut buf = Vec::new();

        // Magic
        buf.extend_from_slice(&0xC5110001u32.to_le_bytes());
        // Node ID
        buf.push(node_id);
        // Antennas
        buf.push(1);
        // Subcarriers (LE u16)
        buf.extend_from_slice(&(n_subcarriers as u16).to_le_bytes());
        // Frequency MHz (LE u32)
        buf.extend_from_slice(&2437u32.to_le_bytes());
        // Sequence (LE u32)
        buf.extend_from_slice(&sequence.to_le_bytes());
        // RSSI (i8)
        buf.push((-50i8) as u8);
        // Noise floor (i8)
        buf.push((-90i8) as u8);
        // Reserved
        buf.extend_from_slice(&[0u8; 2]);
        // I/Q data
        for i in 0..n_subcarriers {
            buf.push((i % 127) as u8); // I
            buf.push(((i * 2) % 127) as u8); // Q
        }

        buf
    }

    #[test]
    fn test_aggregator_receives_valid_frame() {
        let (tx, rx) = mpsc::sync_channel(16);
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut agg = Esp32Aggregator::from_socket(socket, tx);

        let pkt = build_test_packet(1, 0, 4);
        agg.handle_packet(&pkt);

        let frame = rx.try_recv().unwrap();
        assert_eq!(frame.metadata.node_id, 1);
        assert_eq!(frame.metadata.sequence, 0);
        assert_eq!(frame.subcarrier_count(), 4);
    }

    #[test]
    fn test_aggregator_tracks_sequence_gaps() {
        let (tx, _rx) = mpsc::sync_channel(16);
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut agg = Esp32Aggregator::from_socket(socket, tx);

        // Send seq 0
        agg.handle_packet(&build_test_packet(1, 0, 4));
        // Send seq 5 (gap of 4)
        agg.handle_packet(&build_test_packet(1, 5, 4));

        assert_eq!(agg.drops_for_node(1), 4);
    }

    #[test]
    fn test_aggregator_handles_bad_packet() {
        let (tx, rx) = mpsc::sync_channel(16);
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut agg = Esp32Aggregator::from_socket(socket, tx);

        // Garbage bytes — should not panic or produce a frame
        agg.handle_packet(&[0xFF, 0xFE, 0xFD, 0xFC, 0x00]);

        assert!(rx.try_recv().is_err());
        assert_eq!(agg.node_count(), 0);
    }

    #[test]
    fn test_aggregator_multi_node() {
        let (tx, rx) = mpsc::sync_channel(16);
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut agg = Esp32Aggregator::from_socket(socket, tx);

        agg.handle_packet(&build_test_packet(1, 0, 4));
        agg.handle_packet(&build_test_packet(2, 0, 4));

        assert_eq!(agg.node_count(), 2);

        let f1 = rx.try_recv().unwrap();
        let f2 = rx.try_recv().unwrap();
        assert_eq!(f1.metadata.node_id, 1);
        assert_eq!(f2.metadata.node_id, 2);
    }

    #[test]
    fn test_aggregator_loopback_udp() {
        // Full UDP roundtrip via loopback
        let recv_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let recv_addr = recv_socket.local_addr().unwrap();
        recv_socket.set_nonblocking(true).unwrap();

        let send_socket = UdpSocket::bind("127.0.0.1:0").unwrap();

        let (tx, rx) = mpsc::sync_channel(16);
        let mut agg = Esp32Aggregator::from_socket(recv_socket, tx);

        // Send a packet via UDP
        let pkt = build_test_packet(3, 42, 4);
        send_socket.send_to(&pkt, recv_addr).unwrap();

        // Read from the socket and handle
        let mut buf = [0u8; 2048];
        // Small delay to let the packet arrive
        std::thread::sleep(std::time::Duration::from_millis(50));
        if let Ok((n, _)) = agg.socket.recv_from(&mut buf) {
            agg.handle_packet(&buf[..n]);
        }

        let frame = rx.try_recv().unwrap();
        assert_eq!(frame.metadata.node_id, 3);
        assert_eq!(frame.metadata.sequence, 42);
    }
}
