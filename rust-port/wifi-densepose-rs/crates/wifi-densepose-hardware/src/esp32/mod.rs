//! ESP32 hardware protocol modules.
//!
//! Implements sensing-first RF protocols for ESP32-S3 mesh nodes,
//! including TDM (Time-Division Multiplexed) sensing schedules
//! per ADR-029 (RuvSense) and ADR-031 (RuView).
//!
//! ## Security (ADR-032 / ADR-032a)
//!
//! - `quic_transport` -- QUIC-based authenticated transport for aggregator nodes
//! - `secure_tdm` -- Secured TDM protocol with dual-mode (QUIC / manual crypto)

pub mod tdm;
pub mod quic_transport;
pub mod secure_tdm;

pub use tdm::{
    TdmSchedule, TdmCoordinator, TdmSlot, TdmSlotCompleted,
    SyncBeacon, TdmError,
};

pub use quic_transport::{
    SecurityMode, QuicTransportConfig, QuicTransportHandle, QuicTransportError,
    TransportStats, ConnectionState, MessageType, FramedMessage,
    STREAM_BEACON, STREAM_CSI, STREAM_CONTROL,
};

pub use secure_tdm::{
    SecureTdmCoordinator, SecureTdmConfig, SecureTdmError,
    SecLevel, AuthenticatedBeacon, SecureCycleOutput,
    ReplayWindow, AUTHENTICATED_BEACON_SIZE,
};
