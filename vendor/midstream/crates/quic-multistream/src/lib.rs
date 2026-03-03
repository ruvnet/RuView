//! # QUIC Multi-Stream
//!
//! Cross-platform QUIC multi-stream support for native and WASM targets.
//!
//! ## Features
//! - Unified API for native (quinn) and WASM (WebTransport)
//! - Multiplexed bidirectional and unidirectional streams
//! - Stream prioritization for QoS
//! - 0-RTT connection establishment (native)
//! - Built-in encryption and security
//!
//! ## Examples
//!
//! ### Native Example
//! ```no_run
//! # #[cfg(not(target_arch = "wasm32"))]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use midstreamer_quic::{QuicConnection, StreamPriority};
//!
//! // Connect to server
//! let connection = QuicConnection::connect("localhost:4433").await?;
//!
//! // Open bidirectional stream
//! let mut stream = connection.open_bi_stream().await?;
//!
//! // Send data
//! stream.send(b"Hello QUIC!").await?;
//!
//! // Receive response
//! let mut buffer = vec![0u8; 1024];
//! let n = stream.recv(&mut buffer).await?;
//!
//! println!("Received: {:?}", &buffer[..n]);
//! # Ok(())
//! # }
//! ```
//!
//! ### WASM Example
//! ```no_run
//! # #[cfg(target_arch = "wasm32")]
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use midstreamer_quic::QuicConnection;
//!
//! // Connect via WebTransport
//! let connection = QuicConnection::connect("https://server.example.com").await?;
//!
//! // Open stream and communicate
//! let mut stream = connection.open_bi_stream().await?;
//! stream.send(b"Hello from WASM!").await?;
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;


#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;

/// Errors that can occur during QUIC operations
#[derive(Debug, Error)]
pub enum QuicError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Send error: {0}")]
    SendError(String),

    #[error("Receive error: {0}")]
    RecvError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("TLS error: {0}")]
    TlsError(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Connection closed: {0}")]
    ConnectionClosed(String),

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Quinn error: {0}")]
    QuinnError(#[from] quinn::ConnectionError),

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Quinn connect error: {0}")]
    QuinnConnectError(#[from] quinn::ConnectError),

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Quinn read error: {0}")]
    QuinnReadError(#[from] quinn::ReadError),

    #[cfg(not(target_arch = "wasm32"))]
    #[error("Quinn write error: {0}")]
    QuinnWriteError(#[from] quinn::WriteError),

    #[cfg(target_arch = "wasm32")]
    #[error("WASM error: {0}")]
    WasmError(String),

    #[error("IO error: {0}")]
    IoError(String),
}

#[cfg(target_arch = "wasm32")]
impl From<wasm_bindgen::JsValue> for QuicError {
    fn from(err: wasm_bindgen::JsValue) -> Self {
        QuicError::WasmError(format!("{:?}", err))
    }
}

/// Stream priority for quality-of-service control
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum StreamPriority {
    /// Critical priority (highest)
    Critical = 0,
    /// High priority
    High = 1,
    /// Normal priority (default)
    Normal = 2,
    /// Low priority
    Low = 3,
}

impl Default for StreamPriority {
    fn default() -> Self {
        StreamPriority::Normal
    }
}

impl fmt::Display for StreamPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamPriority::Critical => write!(f, "Critical"),
            StreamPriority::High => write!(f, "High"),
            StreamPriority::Normal => write!(f, "Normal"),
            StreamPriority::Low => write!(f, "Low"),
        }
    }
}

/// Connection statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStats {
    /// Number of active bidirectional streams
    pub active_bi_streams: usize,
    /// Number of active unidirectional streams
    pub active_uni_streams: usize,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Round-trip time in milliseconds
    pub rtt_ms: f64,
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self {
            active_bi_streams: 0,
            active_uni_streams: 0,
            bytes_sent: 0,
            bytes_received: 0,
            rtt_ms: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(StreamPriority::Critical < StreamPriority::High);
        assert!(StreamPriority::High < StreamPriority::Normal);
        assert!(StreamPriority::Normal < StreamPriority::Low);
    }

    #[test]
    fn test_priority_default() {
        assert_eq!(StreamPriority::default(), StreamPriority::Normal);
    }

    #[test]
    fn test_priority_display() {
        assert_eq!(StreamPriority::Critical.to_string(), "Critical");
        assert_eq!(StreamPriority::High.to_string(), "High");
        assert_eq!(StreamPriority::Normal.to_string(), "Normal");
        assert_eq!(StreamPriority::Low.to_string(), "Low");
    }

    #[test]
    fn test_connection_stats_default() {
        let stats = ConnectionStats::default();
        assert_eq!(stats.active_bi_streams, 0);
        assert_eq!(stats.active_uni_streams, 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.bytes_received, 0);
        assert_eq!(stats.rtt_ms, 0.0);
    }

    #[test]
    fn test_error_display() {
        let err = QuicError::ConnectionFailed("test".to_string());
        assert!(err.to_string().contains("Connection failed"));

        let err = QuicError::StreamError("stream".to_string());
        assert!(err.to_string().contains("Stream error"));
    }

    #[test]
    fn test_priority_serialization() {
        let priority = StreamPriority::High;
        let json = serde_json::to_string(&priority).unwrap();
        let deserialized: StreamPriority = serde_json::from_str(&json).unwrap();
        assert_eq!(priority, deserialized);
    }

    #[test]
    fn test_stats_serialization() {
        let stats = ConnectionStats {
            active_bi_streams: 5,
            active_uni_streams: 3,
            bytes_sent: 1024,
            bytes_received: 2048,
            rtt_ms: 15.5,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: ConnectionStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.active_bi_streams, deserialized.active_bi_streams);
        assert_eq!(stats.rtt_ms, deserialized.rtt_ms);
    }

    #[test]
    fn test_error_conversion() {
        let err = QuicError::IoError("io test".to_string());
        assert!(matches!(err, QuicError::IoError(_)));
    }
}
