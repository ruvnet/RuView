//! Transport layer abstraction over ruv-swarm-transport
//!
//! Provides unified interface for WebSocket, SharedMemory, and WASM transports.

use crate::{Result, SwarmError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Transport types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transport {
    /// WebSocket for remote communication
    WebSocket,
    /// SharedMemory for local high-performance IPC
    SharedMemory,
    /// WASM-compatible transport for browser
    #[cfg(feature = "wasm")]
    Wasm,
}

impl Default for Transport {
    fn default() -> Self {
        Transport::WebSocket
    }
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    pub transport_type: Transport,
    pub buffer_size: usize,
    pub reconnect_interval_ms: u64,
    pub max_message_size: usize,
    pub enable_compression: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            transport_type: Transport::WebSocket,
            buffer_size: 1024,
            reconnect_interval_ms: 5000,
            max_message_size: 16 * 1024 * 1024, // 16MB
            enable_compression: true,
        }
    }
}

/// Unified transport handle
pub struct TransportHandle {
    pub(crate) transport_type: Transport,
    pub(crate) sender: mpsc::Sender<Vec<u8>>,
    pub(crate) receiver: Arc<RwLock<mpsc::Receiver<Vec<u8>>>>,
    pub(crate) connected: Arc<RwLock<bool>>,
}

impl TransportHandle {
    /// Create new transport handle
    pub fn new(transport_type: Transport) -> Self {
        let (tx, rx) = mpsc::channel(1024);
        Self {
            transport_type,
            sender: tx,
            receiver: Arc::new(RwLock::new(rx)),
            connected: Arc::new(RwLock::new(false)),
        }
    }

    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }

    /// Send raw bytes
    pub async fn send(&self, data: Vec<u8>) -> Result<()> {
        self.sender
            .send(data)
            .await
            .map_err(|e| SwarmError::Transport(e.to_string()))
    }

    /// Receive raw bytes
    pub async fn recv(&self) -> Result<Vec<u8>> {
        let mut rx = self.receiver.write().await;
        rx.recv()
            .await
            .ok_or_else(|| SwarmError::Transport("Channel closed".into()))
    }
}

/// WebSocket transport implementation
pub mod websocket {
    use super::*;

    /// WebSocket connection state
    pub struct WebSocketTransport {
        pub url: String,
        pub handle: TransportHandle,
    }

    impl WebSocketTransport {
        /// Connect to WebSocket server
        pub async fn connect(url: &str) -> Result<Self> {
            let handle = TransportHandle::new(Transport::WebSocket);

            // In real implementation, use ruv-swarm-transport's WebSocket
            // For now, create a mock connection
            tracing::info!("Connecting to WebSocket: {}", url);

            *handle.connected.write().await = true;

            Ok(Self {
                url: url.to_string(),
                handle,
            })
        }

        /// Send message
        pub async fn send(&self, data: Vec<u8>) -> Result<()> {
            self.handle.send(data).await
        }

        /// Receive message
        pub async fn recv(&self) -> Result<Vec<u8>> {
            self.handle.recv().await
        }
    }
}

/// SharedMemory transport for local IPC
pub mod shared_memory {
    use super::*;

    /// Shared memory segment
    pub struct SharedMemoryTransport {
        pub name: String,
        pub size: usize,
        pub handle: TransportHandle,
    }

    impl SharedMemoryTransport {
        /// Create or attach to shared memory
        pub fn new(name: &str, size: usize) -> Result<Self> {
            let handle = TransportHandle::new(Transport::SharedMemory);

            tracing::info!("Creating shared memory: {} ({}KB)", name, size / 1024);

            Ok(Self {
                name: name.to_string(),
                size,
                handle,
            })
        }

        /// Write to shared memory
        pub async fn write(&self, offset: usize, data: &[u8]) -> Result<()> {
            if offset + data.len() > self.size {
                return Err(SwarmError::Transport("Buffer overflow".into()));
            }
            self.handle.send(data.to_vec()).await
        }

        /// Read from shared memory
        pub async fn read(&self, _offset: usize, _len: usize) -> Result<Vec<u8>> {
            self.handle.recv().await
        }
    }
}

/// WASM-compatible transport
#[cfg(feature = "wasm")]
pub mod wasm_transport {
    use super::*;
    use wasm_bindgen::prelude::*;

    /// WASM transport using BroadcastChannel or postMessage
    #[wasm_bindgen]
    pub struct WasmTransport {
        channel_name: String,
        handle: TransportHandle,
    }

    impl WasmTransport {
        pub fn new(channel_name: &str) -> Result<Self> {
            let handle = TransportHandle::new(Transport::Wasm);

            Ok(Self {
                channel_name: channel_name.to_string(),
                handle,
            })
        }

        pub async fn broadcast(&self, data: Vec<u8>) -> Result<()> {
            self.handle.send(data).await
        }

        pub async fn receive(&self) -> Result<Vec<u8>> {
            self.handle.recv().await
        }
    }
}

/// Transport factory
pub struct TransportFactory;

impl TransportFactory {
    /// Create transport based on type
    pub async fn create(config: &TransportConfig, url: Option<&str>) -> Result<TransportHandle> {
        match config.transport_type {
            Transport::WebSocket => {
                let url = url.ok_or_else(|| SwarmError::Config("URL required for WebSocket".into()))?;
                let ws = websocket::WebSocketTransport::connect(url).await?;
                Ok(ws.handle)
            }
            Transport::SharedMemory => {
                let shm = shared_memory::SharedMemoryTransport::new(
                    "ruvector-swarm",
                    config.buffer_size * 1024,
                )?;
                Ok(shm.handle)
            }
            #[cfg(feature = "wasm")]
            Transport::Wasm => {
                let wasm = wasm_transport::WasmTransport::new("ruvector-channel")?;
                Ok(wasm.handle)
            }
        }
    }
}
