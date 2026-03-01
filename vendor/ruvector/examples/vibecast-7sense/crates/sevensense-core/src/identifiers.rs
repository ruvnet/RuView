//! Strongly-typed entity identifiers.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Macro to generate strongly-typed ID wrappers around UUID.
macro_rules! define_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Creates a new random ID.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Creates an ID from an existing UUID.
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Returns the underlying UUID.
            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            /// Parses an ID from a string.
            pub fn parse_str(s: &str) -> Result<Self, uuid::Error> {
                Ok(Self(Uuid::parse_str(s)?))
            }

            /// Creates a nil (all zeros) ID.
            #[must_use]
            pub const fn nil() -> Self {
                Self(Uuid::nil())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }
    };
}

define_id!(RecordingId, "Unique identifier for an audio recording.");
define_id!(SegmentId, "Unique identifier for a call segment.");
define_id!(EmbeddingId, "Unique identifier for an embedding vector.");
define_id!(AnalysisId, "Unique identifier for an analysis result.");
define_id!(SpeciesId, "Unique identifier for a species.");
define_id!(ModelId, "Unique identifier for a trained model.");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_creation() {
        let id1 = RecordingId::new();
        let id2 = RecordingId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_id_display() {
        let id = RecordingId::nil();
        assert_eq!(id.to_string(), "00000000-0000-0000-0000-000000000000");
    }
}
