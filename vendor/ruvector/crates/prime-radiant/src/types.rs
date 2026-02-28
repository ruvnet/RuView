//! Shared types for the Prime-Radiant coherence engine.
//!
//! This module provides common types used across all bounded contexts:
//! - Identifiers (NodeId, EdgeId, WitnessId, etc.)
//! - Primitives (Timestamp, Hash, Version)
//! - Type aliases for consistency

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ============================================================================
// IDENTIFIER TYPES
// ============================================================================

/// Unique identifier for a node in the sheaf graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(Uuid);

impl PartialOrd for NodeId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NodeId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.as_bytes().cmp(other.0.as_bytes())
    }
}

impl NodeId {
    /// Create a new random node ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Convert to u64 for tile sharding
    pub fn as_u64(&self) -> u64 {
        self.0.as_u64_pair().0
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "node:{}", self.0)
    }
}

/// Unique identifier for an edge in the sheaf graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(Uuid);

impl EdgeId {
    /// Create a new random edge ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Create from source and target node IDs (deterministic)
    pub fn from_endpoints(source: NodeId, target: NodeId) -> Self {
        let mut bytes = Vec::with_capacity(32);
        bytes.extend_from_slice(source.as_uuid().as_bytes());
        bytes.extend_from_slice(target.as_uuid().as_bytes());
        let hash = blake3::hash(&bytes);
        Self(Uuid::from_slice(&hash.as_bytes()[..16]).unwrap())
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for EdgeId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "edge:{}", self.0)
    }
}

/// Unique identifier for a graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphId(Uuid);

impl GraphId {
    /// Create a new random graph ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for GraphId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for GraphId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "graph:{}", self.0)
    }
}

/// Identifier for a scope (namespace for coherence isolation)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScopeId(String);

impl ScopeId {
    /// Create a new scope ID
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the scope name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Global scope (default)
    pub fn global() -> Self {
        Self::new("global")
    }
}

impl Default for ScopeId {
    fn default() -> Self {
        Self::global()
    }
}

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scope:{}", self.0)
    }
}

impl From<&str> for ScopeId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ScopeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Identifier for a namespace (multi-tenant isolation)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NamespaceId(String);

impl NamespaceId {
    /// Create a new namespace ID
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the namespace name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Default namespace
    pub fn default_namespace() -> Self {
        Self::new("default")
    }
}

impl Default for NamespaceId {
    fn default() -> Self {
        Self::default_namespace()
    }
}

impl fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ns:{}", self.0)
    }
}

/// Unique identifier for a policy bundle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyBundleId(Uuid);

impl PolicyBundleId {
    /// Create a new random policy bundle ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Convert to bytes
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl Default for PolicyBundleId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PolicyBundleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "policy:{}", self.0)
    }
}

/// Unique identifier for a witness record
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WitnessId(Uuid);

impl WitnessId {
    /// Create a new random witness ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }

    /// Convert to bytes
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Parse from string
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for WitnessId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WitnessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "witness:{}", self.0)
    }
}

/// Unique identifier for a lineage record
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LineageId(Uuid);

impl LineageId {
    /// Create a new random lineage ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for LineageId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LineageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lineage:{}", self.0)
    }
}

/// Unique identifier for an actor (user or system)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(Uuid);

impl ActorId {
    /// Create a new random actor ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// System actor
    pub fn system() -> Self {
        Self(Uuid::nil())
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for ActorId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "actor:{}", self.0)
    }
}

/// Unique identifier for an approver (policy signer)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApproverId(Uuid);

impl ApproverId {
    /// Create a new random approver ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for ApproverId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ApproverId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "approver:{}", self.0)
    }
}

// ============================================================================
// PRIMITIVE TYPES
// ============================================================================

/// Timestamp with nanosecond precision
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(i64);

impl Timestamp {
    /// Create a timestamp for the current moment
    pub fn now() -> Self {
        Self(chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0))
    }

    /// Create from nanoseconds since Unix epoch
    pub fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    /// Get nanoseconds since Unix epoch
    pub fn as_nanos(&self) -> i64 {
        self.0
    }

    /// Get milliseconds since Unix epoch
    pub fn as_millis(&self) -> i64 {
        self.0 / 1_000_000
    }

    /// Get seconds since Unix epoch
    pub fn as_secs(&self) -> i64 {
        self.0 / 1_000_000_000
    }

    /// Convert to chrono DateTime
    pub fn to_datetime(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp_nanos(self.0)
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_datetime().to_rfc3339())
    }
}

impl From<chrono::DateTime<chrono::Utc>> for Timestamp {
    fn from(dt: chrono::DateTime<chrono::Utc>) -> Self {
        Self(dt.timestamp_nanos_opt().unwrap_or(0))
    }
}

/// Blake3 hash for content integrity
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash([u8; 32]);

impl Hash {
    /// Create a hash from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Hash arbitrary data
    pub fn digest(data: &[u8]) -> Self {
        Self(*blake3::hash(data).as_bytes())
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Zero hash (placeholder)
    pub fn zero() -> Self {
        Self([0u8; 32])
    }

    /// Check if this is the zero hash
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }
}

impl Default for Hash {
    fn default() -> Self {
        Self::zero()
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", hex::encode(&self.0[..8]))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.0))
    }
}

impl From<blake3::Hash> for Hash {
    fn from(h: blake3::Hash) -> Self {
        Self(*h.as_bytes())
    }
}

/// Semantic version
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version {
    /// Major version
    pub major: u32,
    /// Minor version
    pub minor: u32,
    /// Patch version
    pub patch: u32,
}

impl Version {
    /// Create a new version
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Initial version (0.1.0)
    pub const fn initial() -> Self {
        Self::new(0, 1, 0)
    }

    /// Increment major version
    pub fn bump_major(&self) -> Self {
        Self::new(self.major + 1, 0, 0)
    }

    /// Increment minor version
    pub fn bump_minor(&self) -> Self {
        Self::new(self.major, self.minor + 1, 0)
    }

    /// Increment patch version
    pub fn bump_patch(&self) -> Self {
        Self::new(self.major, self.minor, self.patch + 1)
    }
}

impl Default for Version {
    fn default() -> Self {
        Self::initial()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl std::str::FromStr for Version {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid version format: {}", s));
        }

        let major = parts[0]
            .parse()
            .map_err(|e| format!("Invalid major: {}", e))?;
        let minor = parts[1]
            .parse()
            .map_err(|e| format!("Invalid minor: {}", e))?;
        let patch = parts[2]
            .parse()
            .map_err(|e| format!("Invalid patch: {}", e))?;

        Ok(Self::new(major, minor, patch))
    }
}

// ============================================================================
// HELPER MODULE FOR HEX ENCODING
// ============================================================================

mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0xf) as usize] as char);
        }
        s
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id() {
        let id1 = NodeId::new();
        let id2 = NodeId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_edge_id_from_endpoints() {
        let n1 = NodeId::new();
        let n2 = NodeId::new();

        let e1 = EdgeId::from_endpoints(n1, n2);
        let e2 = EdgeId::from_endpoints(n1, n2);
        assert_eq!(e1, e2);

        let e3 = EdgeId::from_endpoints(n2, n1);
        assert_ne!(e1, e3);
    }

    #[test]
    fn test_hash_digest() {
        let h1 = Hash::digest(b"hello");
        let h2 = Hash::digest(b"hello");
        let h3 = Hash::digest(b"world");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_version_parsing() {
        let v: Version = "1.2.3".parse().unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_timestamp() {
        let t1 = Timestamp::now();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let t2 = Timestamp::now();
        assert!(t2 > t1);
    }
}
