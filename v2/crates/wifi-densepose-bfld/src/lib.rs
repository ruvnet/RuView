//! # BFLD — Beamforming Feedback Layer for Detection
//!
//! Privacy-gated WiFi sensing primitives derived from 802.11ac/ax Beamforming
//! Feedback Information (BFI). See [`docs/adr/ADR-118-bfld-beamforming-feedback-layer-for-detection.md`](../../../docs/adr/ADR-118-bfld-beamforming-feedback-layer-for-detection.md).
//!
//! ## Three structural invariants
//!
//! - **I1**: Raw BFI never exits the node.
//! - **I2**: Identity embedding is in-RAM-only.
//! - **I3**: Cross-site identity correlation is cryptographically impossible.
//!
//! Status: P1 in progress — frame format + sink marker traits. P2–P6 follow.
//! The §3.6 Soul Signature matching algorithm is now implemented and tested
//! ([`soul_match`] / [`soul_channels`]): a running per-channel weighted-cosine
//! matcher with measured separability and a real [`coherence_gate::SoulMatchOracle`]
//! ([`soul_match::EnrolledMatcher`]). Named-identity locking remains **data-gated** —
//! it requires the decisive high-weight channels (real AETHER enrollment +
//! body-resonance) to be fed real data, which has not been done; on cardiac +
//! respiratory channels alone identity is NOT separable (see
//! `tests/soul_match.rs::cardiac_alone_cannot_separate_identity_matches_audit`).

#![cfg_attr(not(feature = "std"), no_std)]

pub mod coherence_gate;
pub mod embedding;
pub mod embedding_ring;
#[cfg(feature = "std")]
pub mod emitter;
#[cfg(feature = "std")]
pub mod availability;
#[cfg(feature = "std")]
pub mod event;
#[cfg(feature = "std")]
pub mod cbr;
#[cfg(feature = "std")]
pub mod capture;
#[cfg(feature = "std")]
pub mod features;
#[cfg(feature = "std")]
pub mod steering;
pub mod frame;
#[cfg(feature = "std")]
pub mod ha_discovery;
#[cfg(feature = "std")]
pub mod mqtt_topics;
#[cfg(feature = "std")]
pub mod identity_features;
pub mod identity_risk;
#[cfg(feature = "std")]
pub mod payload;
#[cfg(feature = "std")]
pub mod pipeline;
#[cfg(feature = "std")]
pub mod pipeline_handle;
#[cfg(feature = "std")]
pub mod privacy_gate;
pub mod privacy_mode;
#[cfg(feature = "mqtt")]
pub mod rumqttc_publisher;
pub mod signature_hasher;
pub mod sink;
pub mod soul_channels;
pub mod soul_match;
/// WiFi Veil advisory integration (ADR-294). Feature-gated: `veil`.
#[cfg(feature = "veil")]
pub mod veil;

pub use coherence_gate::{CoherenceGate, MatchOutcome, NullOracle, SoulMatchOracle};
#[cfg(feature = "std")]
pub use emitter::{BfldEmitter, SensingInputs};
#[cfg(feature = "std")]
pub use event::BfldEvent;
#[cfg(feature = "std")]
pub use availability::{
    availability_topic, offline_message, online_message, publish_availability_offline,
    publish_availability_online, PAYLOAD_AVAILABLE, PAYLOAD_NOT_AVAILABLE,
};
#[cfg(feature = "std")]
pub use ha_discovery::{publish_discovery, render_discovery_payloads};
#[cfg(feature = "std")]
pub use mqtt_topics::{publish_event, render_events, CapturePublisher, Publish, TopicMessage};
#[cfg(feature = "mqtt")]
pub use rumqttc_publisher::{with_lwt, RumqttPublisher};
pub use embedding::{IdentityEmbedding, EMBEDDING_DIM};
pub use embedding_ring::{EmbeddingRing, RING_CAPACITY};
#[cfg(feature = "std")]
pub use identity_features::{IdentityFeatures, RISK_FACTOR_BYTES};
pub use identity_risk::{score as identity_risk_score, GateAction};
pub use frame::{BfldFrameHeader, BFLD_MAGIC, BFLD_VERSION, BFLD_HEADER_SIZE};
#[cfg(feature = "std")]
pub use frame::BfldFrame;
#[cfg(feature = "std")]
pub use payload::BfldPayload;
#[cfg(feature = "std")]
pub use pipeline::{BfldConfig, BfldPipeline};
#[cfg(feature = "std")]
pub use pipeline_handle::{BfldPipelineHandle, PipelineInput};
#[cfg(feature = "std")]
pub use privacy_gate::PrivacyGate;
pub use privacy_mode::{PrivacyAction, PrivacyAttestationProof, PrivacyMode};
#[cfg(feature = "std")]
pub use privacy_mode::PrivacyModeRegistry;
pub use signature_hasher::{SignatureHasher, RF_SIGNATURE_LEN, SITE_SALT_LEN};
pub use sink::{check_class, LocalSink, MatterSink, NetworkSink, Sink};
pub use soul_channels::{
    Channel, FeatureError, FeatureVector, MatchWeights, SoulChannels, WeightError, CHANNEL_COUNT,
    DEFAULT_WEIGHTS, FEATURE_VECTOR_CAP,
};
pub use soul_match::{cosine_sim, match_score, MatchScore};
#[cfg(feature = "std")]
pub use soul_match::EnrolledMatcher;

/// Privacy classification carried in every `BfldFrame`. See ADR-120 §2.1.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrivacyClass {
    /// Local-only research data including raw BFI matrix. Never networked.
    Raw = 0,
    /// Operator-acknowledged research mode over LAN. Downsampled angles +
    /// identity_embedding + identity_risk_score available. Required for
    /// Soul Signature deployments (ADR-120 §2.7).
    Derived = 1,
    /// Production default: aggregate sensing only, no identity-derived fields.
    Anonymous = 2,
    /// Care-home / regulated deployments: class 2 minus risk score and hash.
    Restricted = 3,
}

impl PrivacyClass {
    /// Returns `true` if frames of this class may cross a `NetworkSink`.
    /// Class 0 (`Raw`) is local-only by structural invariant I1.
    #[must_use]
    pub const fn allows_network(self) -> bool {
        !matches!(self, Self::Raw)
    }

    /// Returns `true` if frames of this class may cross the Matter boundary.
    /// Only classes 2 and 3 are Matter-eligible. See ADR-122 §2.4.
    #[must_use]
    pub const fn allows_matter(self) -> bool {
        matches!(self, Self::Anonymous | Self::Restricted)
    }

    /// Returns the byte value of this class (0..=3) for serialization.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for PrivacyClass {
    type Error = BfldError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Raw),
            1 => Ok(Self::Derived),
            2 => Ok(Self::Anonymous),
            3 => Ok(Self::Restricted),
            other => Err(BfldError::InvalidPrivacyClass(other)),
        }
    }
}

/// Errors produced by BFLD operations.
#[derive(Debug, thiserror::Error)]
pub enum BfldError {
    /// Header magic did not match `BFLD_MAGIC`.
    #[error("invalid BFLD magic: expected 0x{BFLD_MAGIC:08X}, got 0x{0:08X}")]
    InvalidMagic(u32),

    /// Header version unsupported.
    #[error("unsupported BFLD version: {0}")]
    UnsupportedVersion(u16),

    /// Payload CRC32 mismatch — frame corrupted or tampered.
    #[error("payload CRC mismatch: expected 0x{expected:08X}, got 0x{actual:08X}")]
    Crc {
        /// CRC value the header declared.
        expected: u32,
        /// CRC value computed over the received payload.
        actual: u32,
    },

    /// Attempted to publish a class-0 (`Raw`) frame through a network sink.
    /// Enforces structural invariant I1.
    #[error("privacy violation: {reason}")]
    PrivacyViolation {
        /// `Sink::KIND` of the sink that rejected the frame.
        reason: &'static str,
    },

    /// Byte value did not map to any defined `PrivacyClass` (0..=3).
    #[error("invalid PrivacyClass byte: {0}")]
    InvalidPrivacyClass(u8),

    /// Buffer too short for header (86 bytes) or header + declared payload.
    #[error("truncated frame: got {got} bytes, need at least {need}")]
    TruncatedFrame {
        /// Bytes available in the input buffer.
        got: usize,
        /// Bytes the header indicates are required.
        need: usize,
    },

    /// Payload section length-prefix decoding failed or trailing bytes left over.
    #[error("malformed payload section at offset {offset}: {reason}")]
    MalformedSection {
        /// Byte offset within the payload where parsing failed.
        offset: usize,
        /// Human-readable reason for the failure.
        reason: &'static str,
    },

    /// Attempted to demote a frame to a class with MORE information than the
    /// current class (lower numerical value). `demote` is monotonic; the only
    /// way to add information back is to receive a fresh frame.
    #[error("invalid demote: cannot move from class {from} to class {to}")]
    InvalidDemote {
        /// Source class byte value.
        from: u8,
        /// Refused target class byte value.
        to: u8,
    },
}
