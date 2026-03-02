//! QUIC transport layer for multistatic mesh communication (ADR-032a).
//!
//! Wraps `midstreamer-quic` to provide authenticated, encrypted, and
//! congestion-controlled transport for TDM beacons, CSI frames, and
//! control plane messages between aggregator-class nodes.
//!
//! # Stream Mapping
//!
//! | Stream ID | Purpose | Direction | Priority |
//! |---|---|---|---|
//! | 0 | Sync beacons | Coordinator -> Nodes | Highest |
//! | 1 | CSI frames | Nodes -> Aggregator | High |
//! | 2 | Control plane | Bidirectional | Normal |
//!
//! # Fallback
//!
//! Constrained devices (ESP32-S3) use the manual crypto path from
//! ADR-032 sections 2.1-2.2. The `SecurityMode` enum selects transport.

use std::fmt;

// ---------------------------------------------------------------------------
// Stream identifiers
// ---------------------------------------------------------------------------

/// QUIC stream ID for sync beacon traffic (highest priority).
pub const STREAM_BEACON: u64 = 0;

/// QUIC stream ID for CSI frame traffic (high priority).
pub const STREAM_CSI: u64 = 1;

/// QUIC stream ID for control plane traffic (normal priority).
pub const STREAM_CONTROL: u64 = 2;

// ---------------------------------------------------------------------------
// Security mode
// ---------------------------------------------------------------------------

/// Transport security mode selection (ADR-032a).
///
/// Determines whether communication uses manual HMAC/SipHash over
/// plain UDP (for constrained ESP32-S3 devices) or QUIC with TLS 1.3
/// (for aggregator-class nodes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityMode {
    /// Manual HMAC-SHA256 beacon auth + SipHash-2-4 frame integrity
    /// over plain UDP. Suitable for ESP32-S3 with limited memory.
    ManualCrypto,
    /// QUIC transport with TLS 1.3 AEAD encryption, built-in replay
    /// protection, congestion control, and connection migration.
    QuicTransport,
}

impl Default for SecurityMode {
    fn default() -> Self {
        SecurityMode::QuicTransport
    }
}

impl fmt::Display for SecurityMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecurityMode::ManualCrypto => write!(f, "ManualCrypto (UDP + HMAC/SipHash)"),
            SecurityMode::QuicTransport => write!(f, "QuicTransport (QUIC + TLS 1.3)"),
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors from the QUIC transport layer.
#[derive(Debug, Clone, PartialEq)]
pub enum QuicTransportError {
    /// Connection to the remote endpoint failed.
    ConnectionFailed { reason: String },
    /// The QUIC handshake did not complete within the timeout.
    HandshakeTimeout { timeout_ms: u64 },
    /// A stream could not be opened (e.g., stream limit reached).
    StreamOpenFailed { stream_id: u64 },
    /// Sending data on a stream failed.
    SendFailed { stream_id: u64, reason: String },
    /// Receiving data from a stream failed.
    ReceiveFailed { stream_id: u64, reason: String },
    /// The connection was closed by the remote peer.
    ConnectionClosed { error_code: u64 },
    /// Invalid configuration parameter.
    InvalidConfig { param: String, reason: String },
    /// Fallback to manual crypto was triggered.
    FallbackTriggered { reason: String },
}

impl fmt::Display for QuicTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuicTransportError::ConnectionFailed { reason } => {
                write!(f, "QUIC connection failed: {}", reason)
            }
            QuicTransportError::HandshakeTimeout { timeout_ms } => {
                write!(f, "QUIC handshake timed out after {} ms", timeout_ms)
            }
            QuicTransportError::StreamOpenFailed { stream_id } => {
                write!(f, "Failed to open QUIC stream {}", stream_id)
            }
            QuicTransportError::SendFailed { stream_id, reason } => {
                write!(f, "Send failed on stream {}: {}", stream_id, reason)
            }
            QuicTransportError::ReceiveFailed { stream_id, reason } => {
                write!(f, "Receive failed on stream {}: {}", stream_id, reason)
            }
            QuicTransportError::ConnectionClosed { error_code } => {
                write!(f, "Connection closed with error code {}", error_code)
            }
            QuicTransportError::InvalidConfig { param, reason } => {
                write!(f, "Invalid config '{}': {}", param, reason)
            }
            QuicTransportError::FallbackTriggered { reason } => {
                write!(f, "Fallback to manual crypto: {}", reason)
            }
        }
    }
}

impl std::error::Error for QuicTransportError {}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the QUIC transport layer.
#[derive(Debug, Clone)]
pub struct QuicTransportConfig {
    /// Bind address for the QUIC endpoint (e.g., "0.0.0.0:4433").
    pub bind_addr: String,
    /// Handshake timeout in milliseconds.
    pub handshake_timeout_ms: u64,
    /// Keep-alive interval in milliseconds (0 = disabled).
    pub keepalive_ms: u64,
    /// Maximum idle timeout in milliseconds.
    pub idle_timeout_ms: u64,
    /// Maximum number of concurrent bidirectional streams.
    pub max_streams: u64,
    /// Whether to enable connection migration.
    pub enable_migration: bool,
    /// Security mode (QUIC or manual crypto fallback).
    pub security_mode: SecurityMode,
    /// Maximum datagram size (QUIC transport parameter).
    pub max_datagram_size: usize,
}

impl Default for QuicTransportConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:4433".to_string(),
            handshake_timeout_ms: 100,
            keepalive_ms: 5_000,
            idle_timeout_ms: 30_000,
            max_streams: 8,
            enable_migration: true,
            security_mode: SecurityMode::QuicTransport,
            max_datagram_size: 1350,
        }
    }
}

impl QuicTransportConfig {
    /// Validate the configuration, returning an error if invalid.
    pub fn validate(&self) -> Result<(), QuicTransportError> {
        if self.bind_addr.is_empty() {
            return Err(QuicTransportError::InvalidConfig {
                param: "bind_addr".into(),
                reason: "must not be empty".into(),
            });
        }
        if self.handshake_timeout_ms == 0 {
            return Err(QuicTransportError::InvalidConfig {
                param: "handshake_timeout_ms".into(),
                reason: "must be > 0".into(),
            });
        }
        if self.max_streams == 0 {
            return Err(QuicTransportError::InvalidConfig {
                param: "max_streams".into(),
                reason: "must be > 0".into(),
            });
        }
        if self.max_datagram_size < 100 {
            return Err(QuicTransportError::InvalidConfig {
                param: "max_datagram_size".into(),
                reason: "must be >= 100 bytes".into(),
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Transport statistics
// ---------------------------------------------------------------------------

/// Runtime statistics for the QUIC transport.
#[derive(Debug, Clone, Default)]
pub struct TransportStats {
    /// Total bytes sent across all streams.
    pub bytes_sent: u64,
    /// Total bytes received across all streams.
    pub bytes_received: u64,
    /// Number of beacons sent on stream 0.
    pub beacons_sent: u64,
    /// Number of beacons received on stream 0.
    pub beacons_received: u64,
    /// Number of CSI frames sent on stream 1.
    pub csi_frames_sent: u64,
    /// Number of CSI frames received on stream 1.
    pub csi_frames_received: u64,
    /// Number of control messages exchanged on stream 2.
    pub control_messages: u64,
    /// Number of connection migrations completed.
    pub migrations_completed: u64,
    /// Number of times fallback to manual crypto was used.
    pub fallback_count: u64,
    /// Current round-trip time estimate in microseconds.
    pub rtt_us: u64,
}

impl TransportStats {
    /// Total packets processed (sent + received across all types).
    pub fn total_packets(&self) -> u64 {
        self.beacons_sent
            + self.beacons_received
            + self.csi_frames_sent
            + self.csi_frames_received
            + self.control_messages
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

/// Message type tag for QUIC stream multiplexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    /// Sync beacon (stream 0).
    Beacon = 0x01,
    /// CSI frame data (stream 1).
    CsiFrame = 0x02,
    /// Control plane command (stream 2).
    Control = 0x03,
    /// Heartbeat / keepalive.
    Heartbeat = 0x04,
    /// Key rotation notification.
    KeyRotation = 0x05,
}

impl MessageType {
    /// Parse a message type from a byte tag.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(MessageType::Beacon),
            0x02 => Some(MessageType::CsiFrame),
            0x03 => Some(MessageType::Control),
            0x04 => Some(MessageType::Heartbeat),
            0x05 => Some(MessageType::KeyRotation),
            _ => None,
        }
    }

    /// Convert to the stream ID this message type should use.
    pub fn stream_id(&self) -> u64 {
        match self {
            MessageType::Beacon => STREAM_BEACON,
            MessageType::CsiFrame => STREAM_CSI,
            MessageType::Control | MessageType::Heartbeat | MessageType::KeyRotation => {
                STREAM_CONTROL
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Framed message
// ---------------------------------------------------------------------------

/// A framed message for QUIC stream transport.
///
/// Wire format:
/// ```text
/// [0]      message_type (u8)
/// [1..5]   payload_len  (LE u32)
/// [5..5+N] payload      (N bytes)
/// ```
#[derive(Debug, Clone)]
pub struct FramedMessage {
    /// Type of this message.
    pub message_type: MessageType,
    /// Raw payload bytes.
    pub payload: Vec<u8>,
}

/// Header size for a framed message (1 byte type + 4 bytes length).
pub const FRAMED_HEADER_SIZE: usize = 5;

impl FramedMessage {
    /// Create a new framed message.
    pub fn new(message_type: MessageType, payload: Vec<u8>) -> Self {
        Self {
            message_type,
            payload,
        }
    }

    /// Serialize the message to bytes (header + payload).
    pub fn to_bytes(&self) -> Vec<u8> {
        let len = self.payload.len() as u32;
        let mut buf = Vec::with_capacity(FRAMED_HEADER_SIZE + self.payload.len());
        buf.push(self.message_type as u8);
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Deserialize a framed message from bytes.
    ///
    /// Returns the message and the number of bytes consumed, or `None`
    /// if the buffer is too short or the message type is invalid.
    pub fn from_bytes(buf: &[u8]) -> Option<(Self, usize)> {
        if buf.len() < FRAMED_HEADER_SIZE {
            return None;
        }
        let msg_type = MessageType::from_byte(buf[0])?;
        let payload_len =
            u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
        let total = FRAMED_HEADER_SIZE + payload_len;
        if buf.len() < total {
            return None;
        }
        let payload = buf[FRAMED_HEADER_SIZE..total].to_vec();
        Some((
            Self {
                message_type: msg_type,
                payload,
            },
            total,
        ))
    }

    /// Total wire size of this message.
    pub fn wire_size(&self) -> usize {
        FRAMED_HEADER_SIZE + self.payload.len()
    }
}

// ---------------------------------------------------------------------------
// QUIC transport handle
// ---------------------------------------------------------------------------

/// Connection state for the QUIC transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected.
    Disconnected,
    /// TLS handshake in progress.
    Connecting,
    /// Connection established, streams available.
    Connected,
    /// Connection is draining (graceful close in progress).
    Draining,
    /// Connection closed (terminal state).
    Closed,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Disconnected => write!(f, "Disconnected"),
            ConnectionState::Connecting => write!(f, "Connecting"),
            ConnectionState::Connected => write!(f, "Connected"),
            ConnectionState::Draining => write!(f, "Draining"),
            ConnectionState::Closed => write!(f, "Closed"),
        }
    }
}

/// QUIC transport handle for a single connection.
///
/// Manages the lifecycle of a QUIC connection, including handshake,
/// stream management, and graceful shutdown. In production, this wraps
/// the `midstreamer-quic` connection object.
#[derive(Debug)]
pub struct QuicTransportHandle {
    /// Configuration used to create this handle.
    config: QuicTransportConfig,
    /// Current connection state.
    state: ConnectionState,
    /// Transport statistics.
    stats: TransportStats,
    /// Remote peer address (populated after connect).
    remote_addr: Option<String>,
    /// Active security mode (may differ from config if fallback occurred).
    active_mode: SecurityMode,
}

impl QuicTransportHandle {
    /// Create a new transport handle with the given configuration.
    pub fn new(config: QuicTransportConfig) -> Result<Self, QuicTransportError> {
        config.validate()?;
        let mode = config.security_mode;
        Ok(Self {
            config,
            state: ConnectionState::Disconnected,
            stats: TransportStats::default(),
            remote_addr: None,
            active_mode: mode,
        })
    }

    /// Current connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Active security mode.
    pub fn active_mode(&self) -> SecurityMode {
        self.active_mode
    }

    /// Reference to transport statistics.
    pub fn stats(&self) -> &TransportStats {
        &self.stats
    }

    /// Mutable reference to transport statistics.
    pub fn stats_mut(&mut self) -> &mut TransportStats {
        &mut self.stats
    }

    /// Reference to the configuration.
    pub fn config(&self) -> &QuicTransportConfig {
        &self.config
    }

    /// Remote peer address (if connected).
    pub fn remote_addr(&self) -> Option<&str> {
        self.remote_addr.as_deref()
    }

    /// Simulate initiating a connection to a remote peer.
    ///
    /// In production, this would perform the QUIC handshake via
    /// `midstreamer-quic`. Here we model the state transitions.
    pub fn connect(&mut self, remote_addr: &str) -> Result<(), QuicTransportError> {
        if remote_addr.is_empty() {
            return Err(QuicTransportError::ConnectionFailed {
                reason: "empty remote address".into(),
            });
        }
        self.state = ConnectionState::Connecting;
        // In production: midstreamer_quic::connect(remote_addr, &self.config)
        self.remote_addr = Some(remote_addr.to_string());
        self.state = ConnectionState::Connected;
        Ok(())
    }

    /// Record a beacon sent on stream 0.
    pub fn record_beacon_sent(&mut self, size: usize) {
        self.stats.beacons_sent += 1;
        self.stats.bytes_sent += size as u64;
    }

    /// Record a beacon received on stream 0.
    pub fn record_beacon_received(&mut self, size: usize) {
        self.stats.beacons_received += 1;
        self.stats.bytes_received += size as u64;
    }

    /// Record a CSI frame sent on stream 1.
    pub fn record_csi_sent(&mut self, size: usize) {
        self.stats.csi_frames_sent += 1;
        self.stats.bytes_sent += size as u64;
    }

    /// Record a CSI frame received on stream 1.
    pub fn record_csi_received(&mut self, size: usize) {
        self.stats.csi_frames_received += 1;
        self.stats.bytes_received += size as u64;
    }

    /// Record a control message on stream 2.
    pub fn record_control_message(&mut self, size: usize) {
        self.stats.control_messages += 1;
        self.stats.bytes_sent += size as u64;
    }

    /// Trigger fallback to manual crypto mode.
    pub fn trigger_fallback(&mut self, reason: &str) -> Result<(), QuicTransportError> {
        self.active_mode = SecurityMode::ManualCrypto;
        self.stats.fallback_count += 1;
        self.state = ConnectionState::Disconnected;
        Err(QuicTransportError::FallbackTriggered {
            reason: reason.to_string(),
        })
    }

    /// Gracefully close the connection.
    pub fn close(&mut self) {
        if self.state == ConnectionState::Connected {
            self.state = ConnectionState::Draining;
        }
        self.state = ConnectionState::Closed;
    }

    /// Whether the connection is in a usable state.
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- SecurityMode tests ----

    #[test]
    fn test_security_mode_default() {
        assert_eq!(SecurityMode::default(), SecurityMode::QuicTransport);
    }

    #[test]
    fn test_security_mode_display() {
        let quic = format!("{}", SecurityMode::QuicTransport);
        assert!(quic.contains("QUIC"));
        assert!(quic.contains("TLS 1.3"));

        let manual = format!("{}", SecurityMode::ManualCrypto);
        assert!(manual.contains("ManualCrypto"));
        assert!(manual.contains("HMAC"));
    }

    #[test]
    fn test_security_mode_equality() {
        assert_eq!(SecurityMode::QuicTransport, SecurityMode::QuicTransport);
        assert_ne!(SecurityMode::QuicTransport, SecurityMode::ManualCrypto);
    }

    // ---- QuicTransportConfig tests ----

    #[test]
    fn test_config_default() {
        let cfg = QuicTransportConfig::default();
        assert_eq!(cfg.bind_addr, "0.0.0.0:4433");
        assert_eq!(cfg.handshake_timeout_ms, 100);
        assert_eq!(cfg.max_streams, 8);
        assert!(cfg.enable_migration);
        assert_eq!(cfg.security_mode, SecurityMode::QuicTransport);
        assert_eq!(cfg.max_datagram_size, 1350);
    }

    #[test]
    fn test_config_validate_ok() {
        let cfg = QuicTransportConfig::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_config_validate_empty_bind_addr() {
        let cfg = QuicTransportConfig {
            bind_addr: String::new(),
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, QuicTransportError::InvalidConfig { .. }));
    }

    #[test]
    fn test_config_validate_zero_handshake_timeout() {
        let cfg = QuicTransportConfig {
            handshake_timeout_ms: 0,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, QuicTransportError::InvalidConfig { .. }));
    }

    #[test]
    fn test_config_validate_zero_max_streams() {
        let cfg = QuicTransportConfig {
            max_streams: 0,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, QuicTransportError::InvalidConfig { .. }));
    }

    #[test]
    fn test_config_validate_small_datagram() {
        let cfg = QuicTransportConfig {
            max_datagram_size: 50,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, QuicTransportError::InvalidConfig { .. }));
    }

    // ---- MessageType tests ----

    #[test]
    fn test_message_type_from_byte() {
        assert_eq!(MessageType::from_byte(0x01), Some(MessageType::Beacon));
        assert_eq!(MessageType::from_byte(0x02), Some(MessageType::CsiFrame));
        assert_eq!(MessageType::from_byte(0x03), Some(MessageType::Control));
        assert_eq!(MessageType::from_byte(0x04), Some(MessageType::Heartbeat));
        assert_eq!(MessageType::from_byte(0x05), Some(MessageType::KeyRotation));
        assert_eq!(MessageType::from_byte(0x00), None);
        assert_eq!(MessageType::from_byte(0xFF), None);
    }

    #[test]
    fn test_message_type_stream_id() {
        assert_eq!(MessageType::Beacon.stream_id(), STREAM_BEACON);
        assert_eq!(MessageType::CsiFrame.stream_id(), STREAM_CSI);
        assert_eq!(MessageType::Control.stream_id(), STREAM_CONTROL);
        assert_eq!(MessageType::Heartbeat.stream_id(), STREAM_CONTROL);
        assert_eq!(MessageType::KeyRotation.stream_id(), STREAM_CONTROL);
    }

    // ---- FramedMessage tests ----

    #[test]
    fn test_framed_message_roundtrip() {
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let msg = FramedMessage::new(MessageType::Beacon, payload.clone());

        let bytes = msg.to_bytes();
        assert_eq!(bytes.len(), FRAMED_HEADER_SIZE + 4);

        let (decoded, consumed) = FramedMessage::from_bytes(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.message_type, MessageType::Beacon);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn test_framed_message_empty_payload() {
        let msg = FramedMessage::new(MessageType::Heartbeat, vec![]);
        let bytes = msg.to_bytes();
        assert_eq!(bytes.len(), FRAMED_HEADER_SIZE);

        let (decoded, consumed) = FramedMessage::from_bytes(&bytes).unwrap();
        assert_eq!(consumed, FRAMED_HEADER_SIZE);
        assert!(decoded.payload.is_empty());
    }

    #[test]
    fn test_framed_message_too_short() {
        assert!(FramedMessage::from_bytes(&[0x01, 0x00]).is_none());
    }

    #[test]
    fn test_framed_message_invalid_type() {
        let bytes = [0xFF, 0x00, 0x00, 0x00, 0x00];
        assert!(FramedMessage::from_bytes(&bytes).is_none());
    }

    #[test]
    fn test_framed_message_truncated_payload() {
        // Header says 10 bytes payload but only 5 available
        let mut bytes = vec![0x01];
        bytes.extend_from_slice(&10u32.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 5]);
        assert!(FramedMessage::from_bytes(&bytes).is_none());
    }

    #[test]
    fn test_framed_message_wire_size() {
        let msg = FramedMessage::new(MessageType::CsiFrame, vec![0; 100]);
        assert_eq!(msg.wire_size(), FRAMED_HEADER_SIZE + 100);
    }

    #[test]
    fn test_framed_message_large_payload() {
        let payload = vec![0xAB; 4096];
        let msg = FramedMessage::new(MessageType::CsiFrame, payload.clone());
        let bytes = msg.to_bytes();
        let (decoded, _) = FramedMessage::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.payload.len(), 4096);
        assert_eq!(decoded.payload, payload);
    }

    // ---- ConnectionState tests ----

    #[test]
    fn test_connection_state_display() {
        assert_eq!(format!("{}", ConnectionState::Disconnected), "Disconnected");
        assert_eq!(format!("{}", ConnectionState::Connected), "Connected");
        assert_eq!(format!("{}", ConnectionState::Draining), "Draining");
    }

    // ---- TransportStats tests ----

    #[test]
    fn test_transport_stats_default() {
        let stats = TransportStats::default();
        assert_eq!(stats.total_packets(), 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.bytes_received, 0);
    }

    #[test]
    fn test_transport_stats_total_packets() {
        let stats = TransportStats {
            beacons_sent: 10,
            beacons_received: 8,
            csi_frames_sent: 100,
            csi_frames_received: 95,
            control_messages: 5,
            ..Default::default()
        };
        assert_eq!(stats.total_packets(), 218);
    }

    #[test]
    fn test_transport_stats_reset() {
        let mut stats = TransportStats {
            beacons_sent: 10,
            bytes_sent: 1000,
            ..Default::default()
        };
        stats.reset();
        assert_eq!(stats.beacons_sent, 0);
        assert_eq!(stats.bytes_sent, 0);
    }

    // ---- QuicTransportHandle tests ----

    #[test]
    fn test_handle_creation() {
        let handle = QuicTransportHandle::new(QuicTransportConfig::default()).unwrap();
        assert_eq!(handle.state(), ConnectionState::Disconnected);
        assert_eq!(handle.active_mode(), SecurityMode::QuicTransport);
        assert!(!handle.is_connected());
        assert!(handle.remote_addr().is_none());
    }

    #[test]
    fn test_handle_creation_invalid_config() {
        let cfg = QuicTransportConfig {
            bind_addr: String::new(),
            ..Default::default()
        };
        assert!(QuicTransportHandle::new(cfg).is_err());
    }

    #[test]
    fn test_handle_connect() {
        let mut handle = QuicTransportHandle::new(QuicTransportConfig::default()).unwrap();
        handle.connect("192.168.1.100:4433").unwrap();
        assert!(handle.is_connected());
        assert_eq!(handle.remote_addr(), Some("192.168.1.100:4433"));
    }

    #[test]
    fn test_handle_connect_empty_addr() {
        let mut handle = QuicTransportHandle::new(QuicTransportConfig::default()).unwrap();
        let err = handle.connect("").unwrap_err();
        assert!(matches!(err, QuicTransportError::ConnectionFailed { .. }));
    }

    #[test]
    fn test_handle_record_beacon() {
        let mut handle = QuicTransportHandle::new(QuicTransportConfig::default()).unwrap();
        handle.record_beacon_sent(28);
        handle.record_beacon_sent(28);
        handle.record_beacon_received(28);
        assert_eq!(handle.stats().beacons_sent, 2);
        assert_eq!(handle.stats().beacons_received, 1);
        assert_eq!(handle.stats().bytes_sent, 56);
        assert_eq!(handle.stats().bytes_received, 28);
    }

    #[test]
    fn test_handle_record_csi() {
        let mut handle = QuicTransportHandle::new(QuicTransportConfig::default()).unwrap();
        handle.record_csi_sent(512);
        handle.record_csi_received(512);
        assert_eq!(handle.stats().csi_frames_sent, 1);
        assert_eq!(handle.stats().csi_frames_received, 1);
    }

    #[test]
    fn test_handle_record_control() {
        let mut handle = QuicTransportHandle::new(QuicTransportConfig::default()).unwrap();
        handle.record_control_message(64);
        assert_eq!(handle.stats().control_messages, 1);
    }

    #[test]
    fn test_handle_fallback() {
        let mut handle = QuicTransportHandle::new(QuicTransportConfig::default()).unwrap();
        handle.connect("192.168.1.1:4433").unwrap();
        let err = handle.trigger_fallback("handshake timeout").unwrap_err();
        assert!(matches!(err, QuicTransportError::FallbackTriggered { .. }));
        assert_eq!(handle.active_mode(), SecurityMode::ManualCrypto);
        assert_eq!(handle.state(), ConnectionState::Disconnected);
        assert_eq!(handle.stats().fallback_count, 1);
    }

    #[test]
    fn test_handle_close() {
        let mut handle = QuicTransportHandle::new(QuicTransportConfig::default()).unwrap();
        handle.connect("192.168.1.1:4433").unwrap();
        assert!(handle.is_connected());
        handle.close();
        assert_eq!(handle.state(), ConnectionState::Closed);
        assert!(!handle.is_connected());
    }

    #[test]
    fn test_handle_close_when_disconnected() {
        let mut handle = QuicTransportHandle::new(QuicTransportConfig::default()).unwrap();
        handle.close();
        assert_eq!(handle.state(), ConnectionState::Closed);
    }

    // ---- Error display tests ----

    #[test]
    fn test_error_display() {
        let err = QuicTransportError::HandshakeTimeout { timeout_ms: 100 };
        assert!(format!("{}", err).contains("100 ms"));

        let err = QuicTransportError::StreamOpenFailed { stream_id: 1 };
        assert!(format!("{}", err).contains("stream 1"));
    }

    // ---- Stream constants ----

    #[test]
    fn test_stream_constants() {
        assert_eq!(STREAM_BEACON, 0);
        assert_eq!(STREAM_CSI, 1);
        assert_eq!(STREAM_CONTROL, 2);
    }
}
