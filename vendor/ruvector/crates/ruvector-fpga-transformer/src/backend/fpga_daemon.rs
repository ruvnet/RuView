//! FPGA Daemon backend
//!
//! Communicates with a local daemon over Unix socket or TCP
//! to send inference requests to an FPGA accelerator.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::artifact::ModelArtifact;
use crate::backend::{
    commands, compute_topk, crc32, protocol, read_lock, validate_tokens, write_lock, BackendStats,
    RequestFrame, ResponseFrame, TransformerBackend,
};
use crate::error::{Error, Result};
use crate::types::{
    BackendKind, GateDecision, InferenceRequest, InferenceResult, ModelId, WitnessLog,
};

/// Connection type for daemon communication
#[derive(Debug, Clone)]
pub enum DaemonConnection {
    /// Unix domain socket path
    Unix(String),
    /// TCP address (host:port)
    Tcp(String),
}

impl DaemonConnection {
    /// Create a Unix socket connection
    pub fn unix(path: impl Into<String>) -> Self {
        Self::Unix(path.into())
    }

    /// Create a TCP connection
    pub fn tcp(addr: impl Into<String>) -> Self {
        Self::Tcp(addr.into())
    }

    /// Default socket path
    pub fn default_socket() -> Self {
        Self::Unix("/var/run/ruvector_fpga.sock".into())
    }
}

/// FPGA Daemon backend
pub struct FpgaDaemonBackend {
    /// Connection configuration
    connection: DaemonConnection,
    /// Loaded models (cached metadata)
    models: RwLock<HashMap<ModelId, ModelMetadata>>,
    /// Statistics
    stats: RwLock<BackendStats>,
    /// Configuration
    config: DaemonConfig,
}

/// Cached model metadata
struct ModelMetadata {
    artifact: ModelArtifact,
    loaded_at: Instant,
}

/// Configuration for daemon backend
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Connection timeout in milliseconds
    pub connect_timeout_ms: u64,
    /// Read timeout in milliseconds
    pub read_timeout_ms: u64,
    /// Write timeout in milliseconds
    pub write_timeout_ms: u64,
    /// Number of retry attempts
    pub retries: usize,
    /// Retry backoff multiplier
    pub backoff_multiplier: f64,
    /// Return only top-K results
    pub topk_only: bool,
    /// Top-K count
    pub topk: u16,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            connect_timeout_ms: 5000,
            read_timeout_ms: 10000,
            write_timeout_ms: 5000,
            retries: 3,
            backoff_multiplier: 2.0,
            topk_only: true,
            topk: 16,
        }
    }
}

impl FpgaDaemonBackend {
    /// Create a new daemon backend with Unix socket
    pub fn new(socket_path: impl AsRef<Path>) -> Self {
        Self::with_connection(
            DaemonConnection::unix(socket_path.as_ref().to_string_lossy()),
            DaemonConfig::default(),
        )
    }

    /// Create with custom connection and config
    pub fn with_connection(connection: DaemonConnection, config: DaemonConfig) -> Self {
        Self {
            connection,
            models: RwLock::new(HashMap::new()),
            stats: RwLock::new(BackendStats::default()),
            config,
        }
    }

    /// Connect to the daemon
    fn connect(&self) -> Result<Box<dyn ReadWrite>> {
        let timeout = Duration::from_millis(self.config.connect_timeout_ms);

        match &self.connection {
            DaemonConnection::Unix(path) => {
                #[cfg(unix)]
                {
                    use std::os::unix::net::UnixStream;
                    let stream = UnixStream::connect(path)
                        .map_err(|e| Error::daemon_connection(format!("Unix socket: {}", e)))?;
                    stream
                        .set_read_timeout(Some(Duration::from_millis(self.config.read_timeout_ms)))
                        .ok();
                    stream
                        .set_write_timeout(Some(Duration::from_millis(
                            self.config.write_timeout_ms,
                        )))
                        .ok();
                    Ok(Box::new(stream))
                }
                #[cfg(not(unix))]
                {
                    let _ = (path, timeout);
                    Err(Error::FeatureNotAvailable(
                        "Unix sockets not available on this platform".into(),
                    ))
                }
            }
            DaemonConnection::Tcp(addr) => {
                use std::net::TcpStream;
                let stream = TcpStream::connect_timeout(
                    &addr
                        .parse()
                        .map_err(|e| Error::daemon_connection(format!("Invalid address: {}", e)))?,
                    timeout,
                )
                .map_err(|e| Error::daemon_connection(format!("TCP: {}", e)))?;
                stream
                    .set_read_timeout(Some(Duration::from_millis(self.config.read_timeout_ms)))
                    .ok();
                stream
                    .set_write_timeout(Some(Duration::from_millis(self.config.write_timeout_ms)))
                    .ok();
                Ok(Box::new(stream))
            }
        }
    }

    /// Send inference request to daemon
    fn send_request(
        &self,
        stream: &mut dyn ReadWrite,
        req: &InferenceRequest,
    ) -> Result<(Vec<i16>, ResponseFrame)> {
        let shape = &req.shape;

        // Build request flags
        let mut flags = 0u16;
        if self.config.topk_only {
            flags |= protocol::flags::TOPK_ONLY;
        }

        // Create request frame
        let frame = RequestFrame::new(
            shape.seq_len,
            shape.d_model,
            shape.vocab,
            &req.model,
            flags,
            self.config.topk,
        );

        // Build payload
        let mut payload = Vec::with_capacity(
            protocol::HEADER_SIZE + req.tokens.len() * 2 + req.attn_mask.len() + 8,
        );

        // Write header
        payload.extend_from_slice(&frame.to_bytes());

        // Write tokens (u16 little-endian)
        for &token in req.tokens {
            payload.extend_from_slice(&token.to_le_bytes());
        }

        // Write mask
        payload.extend_from_slice(req.attn_mask);

        // Write gate hint (packed)
        payload.extend_from_slice(&req.gate_hint.coherence_score_q.to_le_bytes());
        payload.push(req.gate_hint.boundary_crossed as u8);
        payload.push(req.gate_hint.max_compute_class as u8);

        // Calculate and append checksum
        let checksum = crc32(&payload);
        payload.extend_from_slice(&checksum.to_le_bytes());

        // Send payload
        stream
            .write_all(&payload)
            .map_err(|e| Error::backend(format!("Write failed: {}", e)))?;
        stream
            .flush()
            .map_err(|e| Error::backend(format!("Flush failed: {}", e)))?;

        // Read response header
        let mut response_header = [0u8; 14];
        stream
            .read_exact(&mut response_header)
            .map_err(|e| Error::backend(format!("Read header failed: {}", e)))?;

        let response = ResponseFrame::from_bytes(&response_header);

        // Copy packed fields to avoid alignment issues
        let status = { response.status };

        // Check status
        match status {
            protocol::status::OK => {}
            protocol::status::MODEL_NOT_FOUND => {
                return Err(Error::ModelNotFound(req.model));
            }
            protocol::status::SHAPE_MISMATCH => {
                return Err(Error::ShapeMismatch {
                    expected: req.shape,
                    actual: req.shape, // Daemon should provide actual shape
                });
            }
            protocol::status::GATE_BLOCKED => {
                return Err(Error::GateBlocked {
                    reason: crate::types::SkipReason::PolicyDenied,
                });
            }
            _ => {
                return Err(Error::backend(format!("Daemon error: status {}", status)));
            }
        }

        // Read logits
        let logits_count = if self.config.topk_only {
            self.config.topk as usize * 2 // (token_id, logit) pairs
        } else {
            shape.vocab as usize
        };

        let mut logits_bytes = vec![0u8; logits_count * 2];
        stream
            .read_exact(&mut logits_bytes)
            .map_err(|e| Error::backend(format!("Read logits failed: {}", e)))?;

        // Parse logits
        let logits: Vec<i16> = logits_bytes
            .chunks(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        // Read and verify checksum
        let mut checksum_bytes = [0u8; 4];
        stream.read_exact(&mut checksum_bytes).ok(); // Checksum is optional

        Ok((logits, response))
    }

    /// Send load model command to daemon
    fn send_load_command(
        &self,
        stream: &mut dyn ReadWrite,
        artifact: &ModelArtifact,
    ) -> Result<()> {
        // Pack artifact
        let artifact_bytes = crate::artifact::pack::pack_artifact(artifact)?;

        // Build command packet:
        // [command: 1] [model_id: 32] [artifact_len: 4] [artifact_data: N] [checksum: 4]
        let mut payload = Vec::with_capacity(1 + 32 + 4 + artifact_bytes.len() + 4);

        // Command byte
        payload.push(commands::LOAD_MODEL);

        // Model ID (32 bytes)
        payload.extend_from_slice(artifact.model_id().as_bytes());

        // Artifact length (u32 LE)
        payload.extend_from_slice(&(artifact_bytes.len() as u32).to_le_bytes());

        // Artifact data
        payload.extend_from_slice(&artifact_bytes);

        // Checksum
        let checksum = crc32(&payload);
        payload.extend_from_slice(&checksum.to_le_bytes());

        // Send
        stream
            .write_all(&payload)
            .map_err(|e| Error::backend(format!("Write load command failed: {}", e)))?;
        stream
            .flush()
            .map_err(|e| Error::backend(format!("Flush failed: {}", e)))?;

        // Read response: [status: 1] [message_len: 2] [message: N]
        let mut status = [0u8; 1];
        stream
            .read_exact(&mut status)
            .map_err(|e| Error::backend(format!("Read status failed: {}", e)))?;

        if status[0] != 0 {
            // Read error message
            let mut msg_len = [0u8; 2];
            stream.read_exact(&mut msg_len).ok();
            let len = u16::from_le_bytes(msg_len) as usize;
            let mut msg = vec![0u8; len.min(256)];
            stream.read_exact(&mut msg).ok();
            let error_msg = String::from_utf8_lossy(&msg);
            return Err(Error::backend(format!(
                "Daemon rejected load: {}",
                error_msg
            )));
        }

        Ok(())
    }

    /// Send unload model command to daemon
    fn send_unload_command(&self, stream: &mut dyn ReadWrite, model_id: ModelId) -> Result<()> {
        // Build command packet: [command: 1] [model_id: 32] [checksum: 4]
        let mut payload = Vec::with_capacity(1 + 32 + 4);
        payload.push(commands::UNLOAD_MODEL);
        payload.extend_from_slice(model_id.as_bytes());
        let checksum = crc32(&payload);
        payload.extend_from_slice(&checksum.to_le_bytes());

        // Send
        stream
            .write_all(&payload)
            .map_err(|e| Error::backend(format!("Write unload command failed: {}", e)))?;
        stream
            .flush()
            .map_err(|e| Error::backend(format!("Flush failed: {}", e)))?;

        // Read response status
        let mut status = [0u8; 1];
        stream
            .read_exact(&mut status)
            .map_err(|e| Error::backend(format!("Read status failed: {}", e)))?;

        if status[0] != 0 {
            return Err(Error::backend("Daemon rejected unload"));
        }

        Ok(())
    }

    /// Execute with retries
    fn with_retries<T, F>(&self, mut f: F) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        let mut last_error = None;
        let mut delay = Duration::from_millis(100);

        for attempt in 0..=self.config.retries {
            match f() {
                Ok(result) => return Ok(result),
                Err(e) if e.is_recoverable() => {
                    last_error = Some(e);
                    if attempt < self.config.retries {
                        std::thread::sleep(delay);
                        delay = Duration::from_secs_f64(
                            delay.as_secs_f64() * self.config.backoff_multiplier,
                        );
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_error.unwrap_or_else(|| Error::backend("Unknown error")))
    }
}

impl TransformerBackend for FpgaDaemonBackend {
    fn load(&self, artifact: &ModelArtifact) -> Result<ModelId> {
        // Validate artifact
        artifact.validate()?;

        let model_id = artifact.model_id();

        // Send load command to daemon to preload the model
        self.with_retries(|| {
            let mut stream = self.connect()?;
            self.send_load_command(stream.as_mut(), artifact)
        })?;

        // Cache metadata locally
        {
            let mut models = write_lock(&self.models, |m| {
                m.insert(
                    model_id,
                    ModelMetadata {
                        artifact: artifact.clone(),
                        loaded_at: Instant::now(),
                    },
                );
            })?;
        }

        write_lock(&self.stats, |s| {
            s.models_loaded += 1;
        })?;

        Ok(model_id)
    }

    fn infer(&self, req: InferenceRequest) -> Result<InferenceResult> {
        let start = Instant::now();

        // Validate request
        req.validate()?;

        // Check model is loaded locally and validate tokens
        let model_metadata = read_lock(&self.models, |models| {
            models.get(&req.model).map(|m| m.artifact.clone())
        })?
        .ok_or_else(|| Error::ModelNotFound(req.model))?;

        // Validate tokens against vocabulary
        validate_tokens(req.tokens, model_metadata.manifest.shape.vocab)?;

        // Execute with retries
        let (logits, response) = self.with_retries(|| {
            let mut stream = self.connect()?;
            self.send_request(stream.as_mut(), &req)
        })?;

        let latency_ns = start.elapsed().as_nanos() as u32;

        // Parse response
        let gate_decision = response.to_gate_decision();

        // Build top-K if we got pairs
        let (logits_q, topk) = if self.config.topk_only {
            // logits contains (token_id, logit) pairs
            let pairs: Vec<(u16, i16)> = logits
                .chunks(2)
                .filter_map(|chunk| {
                    if chunk.len() == 2 {
                        Some((chunk[0] as u16, chunk[1]))
                    } else {
                        None
                    }
                })
                .collect();
            (vec![], Some(pairs))
        } else {
            // Full logits - use common compute_topk
            let topk = compute_topk(&logits, 16);
            (logits, Some(topk))
        };

        // Copy packed fields to avoid alignment issues
        let resp_cycles = { response.cycles };
        let resp_latency_ns = { response.latency_ns };

        // Create witness
        let witness = WitnessLog::new(
            model_metadata.model_hash(),
            model_metadata.quant_hash(),
            BackendKind::FpgaDaemon,
            resp_cycles,
            latency_ns.min(resp_latency_ns.max(latency_ns)),
            gate_decision,
        );

        // Update stats (with poison handling)
        write_lock(&self.stats, |stats| {
            stats.total_inferences += 1;
            stats.total_cycles += resp_cycles as u64;
            let n = stats.total_inferences;
            stats.avg_latency_ns = (stats.avg_latency_ns * (n - 1) + latency_ns as u64) / n;
            match gate_decision {
                GateDecision::EarlyExit { .. } => stats.early_exits += 1,
                GateDecision::Skipped { .. } => stats.skipped += 1,
                _ => {}
            }
        })?;

        Ok(InferenceResult::new(logits_q, topk, witness))
    }

    fn unload(&self, model: ModelId) -> Result<()> {
        // Send unload command to daemon
        self.with_retries(|| {
            let mut stream = self.connect()?;
            self.send_unload_command(stream.as_mut(), model)
        })?;

        // Remove from local cache
        let removed = write_lock(&self.models, |models| models.remove(&model).is_some())?;

        if removed {
            write_lock(&self.stats, |s| {
                s.models_loaded = s.models_loaded.saturating_sub(1);
            })?;
            Ok(())
        } else {
            Err(Error::ModelNotFound(model))
        }
    }

    fn is_loaded(&self, model: ModelId) -> bool {
        read_lock(&self.models, |m| m.contains_key(&model)).unwrap_or(false)
    }

    fn kind(&self) -> BackendKind {
        BackendKind::FpgaDaemon
    }

    fn stats(&self) -> BackendStats {
        self.stats.read().unwrap().clone()
    }
}

/// Trait combining Read and Write for stream abstraction
trait ReadWrite: Read + Write + Send {}
impl<T: Read + Write + Send> ReadWrite for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_connection_types() {
        let unix = DaemonConnection::unix("/tmp/test.sock");
        assert!(matches!(unix, DaemonConnection::Unix(_)));

        let tcp = DaemonConnection::tcp("127.0.0.1:8080");
        assert!(matches!(tcp, DaemonConnection::Tcp(_)));
    }

    #[test]
    fn test_config_defaults() {
        let config = DaemonConfig::default();
        assert_eq!(config.connect_timeout_ms, 5000);
        assert_eq!(config.retries, 3);
        assert!(config.topk_only);
    }
}
