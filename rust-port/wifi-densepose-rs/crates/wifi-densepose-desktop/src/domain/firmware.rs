use serde::{Deserialize, Serialize};

/// A firmware binary to be flashed or OTA-pushed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareBinary {
    pub path: String,
    pub size_bytes: u64,
    pub version: Option<String>,
    pub chip_type: Option<String>,
}

/// Lifecycle of a serial flash operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlashPhase {
    Connecting,
    Erasing,
    Writing,
    Verifying,
    Completed,
    Failed,
}

/// A serial flash session aggregate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashSession {
    pub id: String,
    pub port: String,
    pub firmware: FirmwareBinary,
    pub phase: FlashPhase,
    pub bytes_written: u64,
    pub bytes_total: u64,
}

/// Lifecycle of an OTA update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OtaPhase {
    Uploading,
    Rebooting,
    Verifying,
    Completed,
    Failed,
}

/// An OTA update session aggregate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtaSession {
    pub id: String,
    pub target_ip: String,
    pub target_mac: Option<String>,
    pub firmware: FirmwareBinary,
    pub phase: OtaPhase,
    pub bytes_uploaded: u64,
    pub bytes_total: u64,
}

/// Strategy for batch OTA updates across a mesh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OtaStrategy {
    Sequential,
    TdmSafe,
    Parallel,
}

/// A batch OTA session coordinating updates across multiple nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchOtaSession {
    pub id: String,
    pub firmware: FirmwareBinary,
    pub strategy: OtaStrategy,
    pub max_concurrent: usize,
    pub node_count: usize,
    pub completed: usize,
    pub failed: usize,
}
