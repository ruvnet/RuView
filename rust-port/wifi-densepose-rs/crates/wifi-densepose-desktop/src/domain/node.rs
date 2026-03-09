use serde::{Deserialize, Serialize};

/// MAC address value object (e.g., "AA:BB:CC:DD:EE:FF").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MacAddress(pub String);

impl MacAddress {
    pub fn new(addr: impl Into<String>) -> Self {
        Self(addr.into())
    }
}

impl std::fmt::Display for MacAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Node health status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Online,
    Offline,
    Degraded,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Offline
    }
}

/// A discovered ESP32 CSI node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredNode {
    pub ip: String,
    pub mac: Option<String>,
    pub hostname: Option<String>,
    pub node_id: u8,
    pub firmware_version: Option<String>,
    pub health: HealthStatus,
    pub last_seen: String,
}

/// Aggregate root: maintains the set of all known nodes, keyed by MAC.
#[derive(Debug, Default)]
pub struct NodeRegistry {
    nodes: std::collections::HashMap<MacAddress, DiscoveredNode>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or update a node. Deduplicates by MAC address.
    pub fn upsert(&mut self, mac: MacAddress, node: DiscoveredNode) {
        self.nodes.insert(mac, node);
    }

    /// Get a node by MAC address.
    pub fn get(&self, mac: &MacAddress) -> Option<&DiscoveredNode> {
        self.nodes.get(mac)
    }

    /// List all known nodes.
    pub fn all(&self) -> Vec<&DiscoveredNode> {
        self.nodes.values().collect()
    }

    /// Number of registered nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}
