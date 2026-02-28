//! Zero-copy serialization strategies
//!
//! Supports both CDR (DDS-compatible) and rkyv (zero-copy) serialization

use crate::error::{Error, Result};
use crate::message::Message;
use serde::{Deserialize, Serialize};

/// Serialization format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// CDR (Common Data Representation) - DDS compatible
    Cdr,
    /// rkyv zero-copy archives
    Rkyv,
    /// JSON (for debugging)
    Json,
}

/// Serialize a message using CDR format
pub fn serialize_cdr<T: Serialize>(msg: &T) -> Result<Vec<u8>> {
    cdr::serialize::<_, _, cdr::CdrBe>(msg, cdr::Infinite)
        .map_err(|e| Error::Serialization(e.to_string()))
}

/// Deserialize a message using CDR format
pub fn deserialize_cdr<T: for<'de> Deserialize<'de>>(data: &[u8]) -> Result<T> {
    cdr::deserialize::<T>(data)
        .map_err(|e| Error::Serialization(e.to_string()))
}

/// Serialize a message using rkyv (zero-copy)
pub fn serialize_rkyv<T>(_msg: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    // Simplified implementation for compatibility
    // In production, use proper rkyv serialization
    Err(Error::Serialization("rkyv serialization not fully implemented".to_string()))
}

/// Serialize a message to JSON
pub fn serialize_json<T: Serialize>(msg: &T) -> Result<String> {
    serde_json::to_string(msg)
        .map_err(|e| Error::Serialization(e.to_string()))
}

/// Deserialize a message from JSON
pub fn deserialize_json<T: for<'de> Deserialize<'de>>(data: &str) -> Result<T> {
    serde_json::from_str(data)
        .map_err(|e| Error::Serialization(e.to_string()))
}

/// Serializer wrapper
pub struct Serializer {
    format: Format,
}

impl Serializer {
    pub fn new(format: Format) -> Self {
        Self { format }
    }

    pub fn serialize<T: Message>(&self, msg: &T) -> Result<Vec<u8>> {
        match self.format {
            Format::Cdr => serialize_cdr(msg),
            Format::Rkyv => serialize_rkyv(msg),
            Format::Json => serialize_json(msg).map(|s| s.into_bytes()),
        }
    }
}

impl Default for Serializer {
    fn default() -> Self {
        Self::new(Format::Cdr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::RobotState;

    #[test]
    fn test_cdr_serialization() {
        let state = RobotState::default();
        let bytes = serialize_cdr(&state).unwrap();
        let decoded: RobotState = deserialize_cdr(&bytes).unwrap();
        assert_eq!(decoded.position, state.position);
    }

    #[test]
    fn test_json_serialization() {
        let state = RobotState::default();
        let json = serialize_json(&state).unwrap();
        let decoded: RobotState = deserialize_json(&json).unwrap();
        assert_eq!(decoded.position, state.position);
    }

    #[test]
    fn test_serializer() {
        let serializer = Serializer::new(Format::Cdr);
        let state = RobotState::default();
        let bytes = serializer.serialize(&state).unwrap();
        assert!(!bytes.is_empty());
    }
}
