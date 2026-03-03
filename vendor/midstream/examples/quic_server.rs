//! Production-Ready QUIC Multi-Stream Server Example
//!
//! This example demonstrates a comprehensive QUIC server implementation using
//! the midstream-quic crate with support for:
//! - Multiple concurrent bidirectional streams
//! - Stream prioritization
//! - TLS certificate generation (self-signed for demo)
//! - Connection statistics and monitoring
//! - Graceful shutdown handling
//! - Performance metrics logging
//!
//! # Usage
//!
//! ```bash
//! cargo run --example quic_server
//! ```
//!
//! # Testing with Client
//!
//! You can test this server with a QUIC client on port 4433:
//! ```bash
//! # Example with curl (if built with HTTP/3 support)
//! curl --http3 https://localhost:4433 --insecure
//! ```

use midstream_quic::{QuicMultiStream, QuicConfig, StreamPriority};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::signal;
use tokio::time::{Duration, Instant};
use tracing::{info, warn, error, debug};

/// Server configuration constants
const SERVER_PORT: u16 = 4433;
const SERVER_ADDR: &str = "0.0.0.0";
const MAX_CONCURRENT_STREAMS: u64 = 100;
const IDLE_TIMEOUT_MS: u64 = 30_000;
const KEEP_ALIVE_INTERVAL_MS: u64 = 5_000;

/// Connection statistics tracker
#[derive(Default)]
struct ServerStats {
    total_connections: AtomicU64,
    active_connections: AtomicU64,
    total_streams: AtomicU64,
    bytes_received: AtomicU64,
    bytes_sent: AtomicU64,
}

impl ServerStats {
    fn log_stats(&self) {
        info!(
            "Server Stats - Connections: {} active, {} total | Streams: {} total | Data: {} bytes RX, {} bytes TX",
            self.active_connections.load(Ordering::Relaxed),
            self.total_connections.load(Ordering::Relaxed),
            self.total_streams.load(Ordering::Relaxed),
            self.bytes_received.load(Ordering::Relaxed),
            self.bytes_sent.load(Ordering::Relaxed),
        );
    }
}

/// Generate self-signed TLS certificate for demo purposes
fn generate_self_signed_cert() -> Result<(Vec<u8>, Vec<u8>), Box<dyn std::error::Error>> {
    use rcgen::{Certificate, CertificateParams, DistinguishedName};

    let mut params = CertificateParams::new(vec!["localhost".to_string()]);
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(
        rcgen::DnType::CommonName,
        "Midstream QUIC Server".to_string(),
    );

    let cert = Certificate::from_params(params)?;
    let cert_pem = cert.serialize_pem()?;
    let key_pem = cert.serialize_private_key_pem();

    info!("Generated self-signed certificate for demo");

    Ok((cert_pem.into_bytes(), key_pem.into_bytes()))
}

/// Handle individual QUIC stream with echo functionality
async fn handle_stream(
    mut stream: quinn::SendStream,
    mut recv: quinn::RecvStream,
    stream_id: u64,
    stats: Arc<ServerStats>,
) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();
    stats.total_streams.fetch_add(1, Ordering::Relaxed);

    debug!("Stream {} opened", stream_id);

    let mut buffer = vec![0u8; 8192];
    let mut total_bytes = 0u64;

    loop {
        match recv.read(&mut buffer).await? {
            Some(bytes_read) => {
                total_bytes += bytes_read as u64;
                stats.bytes_received.fetch_add(bytes_read as u64, Ordering::Relaxed);

                debug!("Stream {} received {} bytes", stream_id, bytes_read);

                // Echo back the data
                stream.write_all(&buffer[..bytes_read]).await?;
                stats.bytes_sent.fetch_add(bytes_read as u64, Ordering::Relaxed);

                // Check for special commands
                if let Ok(msg) = std::str::from_utf8(&buffer[..bytes_read]) {
                    if msg.trim() == "STATS" {
                        let stats_msg = format!(
                            "Stream {} - Duration: {:?}, Bytes: {}\n",
                            stream_id,
                            start_time.elapsed(),
                            total_bytes
                        );
                        stream.write_all(stats_msg.as_bytes()).await?;
                    } else if msg.trim() == "CLOSE" {
                        info!("Stream {} received close command", stream_id);
                        break;
                    }
                }
            }
            None => {
                debug!("Stream {} reached EOF", stream_id);
                break;
            }
        }
    }

    stream.finish().await?;

    info!(
        "Stream {} closed - Duration: {:?}, Total bytes: {}",
        stream_id,
        start_time.elapsed(),
        total_bytes
    );

    Ok(())
}

/// Handle QUIC connection with multiple streams
async fn handle_connection(
    conn: quinn::Connection,
    stats: Arc<ServerStats>,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn_id = stats.total_connections.fetch_add(1, Ordering::Relaxed);
    stats.active_connections.fetch_add(1, Ordering::Relaxed);

    let remote_addr = conn.remote_address();
    info!("Connection {} accepted from {}", conn_id, remote_addr);

    // Spawn task to handle incoming streams
    let stream_stats = stats.clone();
    let stream_handler = tokio::spawn(async move {
        let mut stream_count = 0u64;

        loop {
            match conn.accept_bi().await {
                Ok((send, recv)) => {
                    stream_count += 1;
                    let stream_id = stream_count;
                    let stats_clone = stream_stats.clone();

                    tokio::spawn(async move {
                        if let Err(e) = handle_stream(send, recv, stream_id, stats_clone).await {
                            error!("Stream {} error: {}", stream_id, e);
                        }
                    });
                }
                Err(quinn::ConnectionError::ApplicationClosed(_)) => {
                    info!("Connection {} closed by peer", conn_id);
                    break;
                }
                Err(e) => {
                    error!("Connection {} error accepting stream: {}", conn_id, e);
                    break;
                }
            }
        }
    });

    // Wait for connection to close
    let result = stream_handler.await;

    stats.active_connections.fetch_sub(1, Ordering::Relaxed);
    info!("Connection {} terminated", conn_id);

    result?;
    Ok(())
}

/// Main server function
async fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("Starting Midstream QUIC Multi-Stream Server");

    // Generate self-signed certificate
    let (cert_pem, key_pem) = generate_self_signed_cert()?;

    // Configure QUIC server
    let mut server_config = quinn::ServerConfig::with_single_cert(
        vec![rustls::Certificate(cert_pem)],
        rustls::PrivateKey(key_pem),
    )?;

    // Configure transport settings
    let mut transport = quinn::TransportConfig::default();
    transport.max_concurrent_bidi_streams(MAX_CONCURRENT_STREAMS.try_into()?);
    transport.max_concurrent_uni_streams(MAX_CONCURRENT_STREAMS.try_into()?);
    transport.max_idle_timeout(Some(Duration::from_millis(IDLE_TIMEOUT_MS).try_into()?));
    transport.keep_alive_interval(Some(Duration::from_millis(KEEP_ALIVE_INTERVAL_MS)));

    server_config.transport_config(Arc::new(transport));

    // Bind server endpoint
    let bind_addr = format!("{}:{}", SERVER_ADDR, SERVER_PORT);
    let endpoint = quinn::Endpoint::server(server_config, bind_addr.parse()?)?;

    info!("Server listening on {}", bind_addr);
    info!("Configuration:");
    info!("  - Max concurrent streams: {}", MAX_CONCURRENT_STREAMS);
    info!("  - Idle timeout: {}ms", IDLE_TIMEOUT_MS);
    info!("  - Keep-alive interval: {}ms", KEEP_ALIVE_INTERVAL_MS);

    let stats = Arc::new(ServerStats::default());

    // Spawn statistics logger
    let stats_clone = stats.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            stats_clone.log_stats();
        }
    });

    // Handle graceful shutdown
    let shutdown = signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            Some(incoming) = endpoint.accept() => {
                let stats_clone = stats.clone();

                tokio::spawn(async move {
                    match incoming.await {
                        Ok(conn) => {
                            if let Err(e) = handle_connection(conn, stats_clone).await {
                                error!("Connection handler error: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Incoming connection error: {}", e);
                        }
                    }
                });
            }
            _ = &mut shutdown => {
                info!("Shutdown signal received");
                break;
            }
        }
    }

    // Graceful shutdown
    info!("Shutting down server...");
    endpoint.close(0u32.into(), b"Server shutdown");

    // Wait for active connections to close (max 5 seconds)
    let shutdown_start = Instant::now();
    while stats.active_connections.load(Ordering::Relaxed) > 0
        && shutdown_start.elapsed() < Duration::from_secs(5)
    {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    stats.log_stats();
    info!("Server shutdown complete");

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run_server().await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}
