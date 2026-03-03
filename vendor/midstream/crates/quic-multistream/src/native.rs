//! Native QUIC implementation using quinn

use crate::{ConnectionStats, QuicError, StreamPriority};
use quinn::{ClientConfig, Endpoint, RecvStream, SendStream, VarInt};
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// QUIC connection wrapper for native targets
pub struct QuicConnection {
    connection: quinn::Connection,
    bytes_sent: Arc<AtomicU64>,
    bytes_received: Arc<AtomicU64>,
}

impl QuicConnection {
    /// Connect to a QUIC server
    ///
    /// # Arguments
    /// * `addr` - Server address (e.g., "localhost:4433")
    ///
    /// # Examples
    /// ```no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use midstreamer_quic::QuicConnection;
    ///
    /// let connection = QuicConnection::connect("localhost:4433").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(addr: &str) -> Result<Self, QuicError> {
        // Parse address
        let socket_addr = addr
            .to_socket_addrs()
            .map_err(|e| QuicError::InvalidConfig(e.to_string()))?
            .next()
            .ok_or_else(|| QuicError::InvalidConfig("Invalid address".to_string()))?;

        // Create client config with TLS (skip verification for demo purposes)
        let mut crypto = quinn::rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(SkipServerVerification::new())
            .with_no_client_auth();

        // Enable ALPN for QUIC
        crypto.alpn_protocols = vec![b"h3".to_vec()];

        let client_config = ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
                .map_err(|e| QuicError::TlsError(format!("{:?}", e)))?,
        ));

        // Create endpoint
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| QuicError::ConnectionFailed(e.to_string()))?;
        endpoint.set_default_client_config(client_config);

        // Connect to server
        let connection = endpoint
            .connect(socket_addr, "localhost")
            .map_err(|e| QuicError::ConnectionFailed(e.to_string()))?
            .await?;

        Ok(Self {
            connection,
            bytes_sent: Arc::new(AtomicU64::new(0)),
            bytes_received: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Open a bidirectional stream
    pub async fn open_bi_stream(&self) -> Result<QuicStream, QuicError> {
        let (send, recv) = self.connection.open_bi().await?;
        Ok(QuicStream::new(
            send,
            recv,
            self.bytes_sent.clone(),
            self.bytes_received.clone(),
        ))
    }

    /// Open a bidirectional stream with priority
    pub async fn open_bi_stream_with_priority(
        &self,
        priority: StreamPriority,
    ) -> Result<QuicStream, QuicError> {
        let (send, recv) = self.connection.open_bi().await?;
        let mut stream = QuicStream::new(
            send,
            recv,
            self.bytes_sent.clone(),
            self.bytes_received.clone(),
        );
        stream.set_priority(priority);
        Ok(stream)
    }

    /// Open a unidirectional stream (send-only)
    pub async fn open_uni_stream(&self) -> Result<QuicSendStream, QuicError> {
        let send = self.connection.open_uni().await?;
        Ok(QuicSendStream::new(send, self.bytes_sent.clone()))
    }

    /// Accept an incoming bidirectional stream
    pub async fn accept_bi_stream(&self) -> Result<QuicStream, QuicError> {
        let (send, recv) = self
            .connection
            .accept_bi()
            .await
            .map_err(|e| QuicError::ConnectionClosed(e.to_string()))?;
        Ok(QuicStream::new(
            send,
            recv,
            self.bytes_sent.clone(),
            self.bytes_received.clone(),
        ))
    }

    /// Get connection statistics
    pub fn stats(&self) -> ConnectionStats {
        let stats = self.connection.stats();
        ConnectionStats {
            active_bi_streams: 0, // Not available in quinn stats
            active_uni_streams: 0,
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            rtt_ms: stats.path.rtt.as_millis() as f64,
        }
    }

    /// Close the connection
    pub fn close(&self, error_code: u64, reason: &[u8]) {
        self.connection.close(VarInt::from_u64(error_code).unwrap(), reason);
    }

    /// Get the remote address
    pub fn remote_address(&self) -> SocketAddr {
        self.connection.remote_address()
    }
}

/// Bidirectional QUIC stream
pub struct QuicStream {
    send: SendStream,
    recv: RecvStream,
    priority: StreamPriority,
    bytes_sent: Arc<AtomicU64>,
    bytes_received: Arc<AtomicU64>,
}

impl QuicStream {
    fn new(
        send: SendStream,
        recv: RecvStream,
        bytes_sent: Arc<AtomicU64>,
        bytes_received: Arc<AtomicU64>,
    ) -> Self {
        Self {
            send,
            recv,
            priority: StreamPriority::default(),
            bytes_sent,
            bytes_received,
        }
    }

    /// Send data on the stream
    pub async fn send(&mut self, data: &[u8]) -> Result<usize, QuicError> {
        self.send.write_all(data).await?;
        self.bytes_sent.fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(data.len())
    }

    /// Receive data from the stream
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, QuicError> {
        let n = self.recv.read(buf).await?.unwrap_or(0);
        self.bytes_received.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }

    /// Finish sending on this stream
    pub async fn finish(&mut self) -> Result<(), QuicError> {
        self.send.finish()
            .map_err(|e| QuicError::StreamError(format!("Failed to finish stream: {:?}", e)))?;
        Ok(())
    }

    /// Set stream priority
    pub fn set_priority(&mut self, priority: StreamPriority) {
        self.priority = priority;
        // Note: quinn doesn't directly expose priority setting
        // This would typically be handled at the application level
    }

    /// Get current priority
    pub fn priority(&self) -> StreamPriority {
        self.priority
    }
}

/// Unidirectional send-only stream
pub struct QuicSendStream {
    send: SendStream,
    bytes_sent: Arc<AtomicU64>,
}

impl QuicSendStream {
    fn new(send: SendStream, bytes_sent: Arc<AtomicU64>) -> Self {
        Self { send, bytes_sent }
    }

    /// Send data on the stream
    pub async fn send(&mut self, data: &[u8]) -> Result<usize, QuicError> {
        self.send.write_all(data).await?;
        self.bytes_sent.fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(data.len())
    }

    /// Finish sending on this stream
    pub async fn finish(&mut self) -> Result<(), QuicError> {
        self.send.finish()
            .map_err(|e| QuicError::StreamError(format!("Failed to finish stream: {:?}", e)))?;
        Ok(())
    }
}

/// Skip server certificate verification (for testing only!)
#[derive(Debug)]
struct SkipServerVerification(Arc<quinn::rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(quinn::rustls::crypto::ring::default_provider())))
    }
}

impl quinn::rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &quinn::rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[quinn::rustls::pki_types::CertificateDer<'_>],
        _server_name: &quinn::rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: quinn::rustls::pki_types::UnixTime,
    ) -> Result<quinn::rustls::client::danger::ServerCertVerified, quinn::rustls::Error> {
        Ok(quinn::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &quinn::rustls::pki_types::CertificateDer<'_>,
        dss: &quinn::rustls::DigitallySignedStruct,
    ) -> Result<quinn::rustls::client::danger::HandshakeSignatureValid, quinn::rustls::Error> {
        quinn::rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &quinn::rustls::pki_types::CertificateDer<'_>,
        dss: &quinn::rustls::DigitallySignedStruct,
    ) -> Result<quinn::rustls::client::danger::HandshakeSignatureValid, quinn::rustls::Error> {
        quinn::rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<quinn::rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_stats_tracking() {
        let bytes_sent = Arc::new(AtomicU64::new(100));
        let bytes_received = Arc::new(AtomicU64::new(200));

        assert_eq!(bytes_sent.load(Ordering::Relaxed), 100);
        assert_eq!(bytes_received.load(Ordering::Relaxed), 200);

        bytes_sent.fetch_add(50, Ordering::Relaxed);
        assert_eq!(bytes_sent.load(Ordering::Relaxed), 150);
    }

    #[test]
    fn test_priority_values() {
        assert_eq!(StreamPriority::default(), StreamPriority::Normal);
        assert!(StreamPriority::Critical < StreamPriority::High);
    }
}
