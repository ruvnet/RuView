//! Governance Layer
//!
//! First-class, immutable, addressable governance objects for the Coherence Engine.
//!
//! This module implements ADR-CE-005: "Governance objects are first-class, immutable, addressable"
//!
//! # Core Invariants
//!
//! 1. **No action without witness**: Every gate decision must produce a `WitnessRecord`
//! 2. **No write without lineage**: Every authoritative write must have a `LineageRecord`
//! 3. **Policy immutability**: Once activated, a `PolicyBundle` cannot be modified
//! 4. **Multi-party approval**: Critical policies require multiple `ApprovalSignature`s
//! 5. **Witness chain integrity**: Each witness references its predecessor via Blake3 hash

mod lineage;
mod policy;
mod repository;
mod witness;

pub use policy::{
    ApprovalSignature, ApproverId, EscalationCondition, EscalationRule, PolicyBundle,
    PolicyBundleBuilder, PolicyBundleId, PolicyBundleRef, PolicyBundleStatus, PolicyError,
    ThresholdConfig,
};

pub use witness::{
    ComputeLane as WitnessComputeLane, EnergySnapshot, GateDecision, WitnessChainError,
    WitnessError, WitnessId, WitnessRecord,
};

pub use lineage::{EntityRef, LineageError, LineageId, LineageRecord, Operation};

pub use repository::{LineageRepository, PolicyRepository, WitnessRepository};

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Blake3 content hash (32 bytes)
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// Create a new hash from bytes
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Create a hash from a Blake3 hasher output
    #[must_use]
    pub fn from_blake3(hash: blake3::Hash) -> Self {
        Self(*hash.as_bytes())
    }

    /// Get the hash as bytes
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Create a zero hash (used as sentinel)
    #[must_use]
    pub const fn zero() -> Self {
        Self([0u8; 32])
    }

    /// Check if this is the zero hash
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }

    /// Convert to hex string
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    ///
    /// # Errors
    ///
    /// Returns an error if the hex string is invalid or wrong length
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", &self.to_hex()[..16])
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Default for Hash {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<blake3::Hash> for Hash {
    fn from(hash: blake3::Hash) -> Self {
        Self::from_blake3(hash)
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Timestamp with nanosecond precision
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp {
    /// Seconds since Unix epoch
    pub secs: i64,
    /// Nanoseconds within the second
    pub nanos: u32,
}

impl Timestamp {
    /// Create a new timestamp
    #[must_use]
    pub const fn new(secs: i64, nanos: u32) -> Self {
        Self { secs, nanos }
    }

    /// Get the current timestamp
    #[must_use]
    pub fn now() -> Self {
        let dt = chrono::Utc::now();
        Self {
            secs: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos(),
        }
    }

    /// Create a timestamp from Unix epoch seconds
    #[must_use]
    pub const fn from_secs(secs: i64) -> Self {
        Self { secs, nanos: 0 }
    }

    /// Convert to Unix epoch milliseconds
    #[must_use]
    pub const fn as_millis(&self) -> i64 {
        self.secs * 1000 + (self.nanos / 1_000_000) as i64
    }

    /// Create from Unix epoch milliseconds
    #[must_use]
    pub const fn from_millis(millis: i64) -> Self {
        Self {
            secs: millis / 1000,
            nanos: ((millis % 1000) * 1_000_000) as u32,
        }
    }

    /// Convert to chrono DateTime
    #[must_use]
    pub fn to_datetime(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(self.secs, self.nanos).unwrap_or_else(chrono::Utc::now)
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.to_datetime().format("%Y-%m-%d %H:%M:%S%.3f UTC")
        )
    }
}

impl From<chrono::DateTime<chrono::Utc>> for Timestamp {
    fn from(dt: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            secs: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos(),
        }
    }
}

/// Semantic version for policy bundles
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Version {
    /// Major version (breaking changes)
    pub major: u32,
    /// Minor version (new features, backward compatible)
    pub minor: u32,
    /// Patch version (bug fixes)
    pub patch: u32,
}

impl Version {
    /// Create a new version
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Initial version (1.0.0)
    #[must_use]
    pub const fn initial() -> Self {
        Self::new(1, 0, 0)
    }

    /// Increment patch version
    #[must_use]
    pub const fn bump_patch(self) -> Self {
        Self {
            major: self.major,
            minor: self.minor,
            patch: self.patch + 1,
        }
    }

    /// Increment minor version (resets patch)
    #[must_use]
    pub const fn bump_minor(self) -> Self {
        Self {
            major: self.major,
            minor: self.minor + 1,
            patch: 0,
        }
    }

    /// Increment major version (resets minor and patch)
    #[must_use]
    pub const fn bump_major(self) -> Self {
        Self {
            major: self.major + 1,
            minor: 0,
            patch: 0,
        }
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
    type Err = GovernanceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(GovernanceError::InvalidVersion(s.to_string()));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| GovernanceError::InvalidVersion(s.to_string()))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| GovernanceError::InvalidVersion(s.to_string()))?;
        let patch = parts[2]
            .parse()
            .map_err(|_| GovernanceError::InvalidVersion(s.to_string()))?;

        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

/// Top-level governance error
#[derive(Debug, Error)]
pub enum GovernanceError {
    /// Policy-related error
    #[error("Policy error: {0}")]
    Policy(#[from] PolicyError),

    /// Witness-related error
    #[error("Witness error: {0}")]
    Witness(#[from] WitnessError),

    /// Lineage-related error
    #[error("Lineage error: {0}")]
    Lineage(#[from] LineageError),

    /// Invalid version format
    #[error("Invalid version format: {0}")]
    InvalidVersion(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Repository error
    #[error("Repository error: {0}")]
    Repository(String),

    /// Invariant violation
    #[error("Invariant violation: {0}")]
    InvariantViolation(String),
}

// Hex encoding utilities (inline to avoid external dependency)
mod hex {
    pub use std::fmt::Write;

    #[derive(Debug)]
    pub enum FromHexError {
        InvalidStringLength,
        InvalidHexCharacter(char),
    }

    impl std::fmt::Display for FromHexError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::InvalidStringLength => write!(f, "invalid hex string length"),
                Self::InvalidHexCharacter(c) => write!(f, "invalid hex character: {c}"),
            }
        }
    }

    impl std::error::Error for FromHexError {}

    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        let bytes = bytes.as_ref();
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            write!(s, "{b:02x}").unwrap();
        }
        s
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, FromHexError> {
        if s.len() % 2 != 0 {
            return Err(FromHexError::InvalidStringLength);
        }

        let mut bytes = Vec::with_capacity(s.len() / 2);
        let mut chars = s.chars();

        while let (Some(h), Some(l)) = (chars.next(), chars.next()) {
            let high = h.to_digit(16).ok_or(FromHexError::InvalidHexCharacter(h))? as u8;
            let low = l.to_digit(16).ok_or(FromHexError::InvalidHexCharacter(l))? as u8;
            bytes.push((high << 4) | low);
        }

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_creation_and_display() {
        let bytes = [1u8; 32];
        let hash = Hash::from_bytes(bytes);

        assert_eq!(hash.as_bytes(), &bytes);
        assert!(!hash.is_zero());

        let hex = hash.to_hex();
        let parsed = Hash::from_hex(&hex).unwrap();
        assert_eq!(hash, parsed);
    }

    #[test]
    fn test_hash_zero() {
        let zero = Hash::zero();
        assert!(zero.is_zero());
        assert_eq!(zero.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn test_timestamp() {
        let ts = Timestamp::now();
        assert!(ts.secs > 0);

        let from_secs = Timestamp::from_secs(1700000000);
        assert_eq!(from_secs.secs, 1700000000);
        assert_eq!(from_secs.nanos, 0);

        let from_millis = Timestamp::from_millis(1700000000123);
        assert_eq!(from_millis.secs, 1700000000);
        assert_eq!(from_millis.nanos, 123_000_000);
    }

    #[test]
    fn test_version() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");

        let parsed: Version = "2.3.4".parse().unwrap();
        assert_eq!(parsed, Version::new(2, 3, 4));

        let bumped = Version::new(1, 2, 3).bump_patch();
        assert_eq!(bumped, Version::new(1, 2, 4));

        let minor_bump = Version::new(1, 2, 3).bump_minor();
        assert_eq!(minor_bump, Version::new(1, 3, 0));

        let major_bump = Version::new(1, 2, 3).bump_major();
        assert_eq!(major_bump, Version::new(2, 0, 0));
    }
}
