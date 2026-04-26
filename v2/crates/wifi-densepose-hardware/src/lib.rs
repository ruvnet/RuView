//! WiFi-DensePose hardware interface abstractions.
//!
//! This crate provides platform-agnostic types and parsers for WiFi CSI data
//! from various hardware sources:
//!
//! - **ESP32/ESP32-S3**: Parses ADR-018 binary CSI frames streamed over UDP
//! - **UDP Aggregator**: Receives frames from multiple ESP32 nodes (ADR-018 Layer 2)
//! - **Bridge**: Converts CsiFrame → CsiData for the detection pipeline (ADR-018 Layer 3)
//!
//! # Design Principles
//!
//! 1. **No mock data**: All parsers either parse real bytes or return explicit errors
//! 2. **No hardware dependency at compile time**: Parsing is done on byte buffers,
//!    not through FFI to ESP-IDF or kernel modules
//! 3. **Deterministic**: Same bytes in → same parsed output, always
//!
//! # Example
//!
//! ```rust
//! use wifi_densepose_hardware::{CsiFrame, Esp32CsiParser, ParseError};
//!
//! // Parse ESP32 CSI data from UDP bytes
//! let raw_bytes: &[u8] = &[/* ADR-018 binary frame */];
//! match Esp32CsiParser::parse_frame(raw_bytes) {
//!     Ok((frame, consumed)) => {
//!         println!("Parsed {} subcarriers ({} bytes)", frame.subcarrier_count(), consumed);
//!         let (amplitudes, phases) = frame.to_amplitude_phase();
//!         // Feed into detection pipeline...
//!     }
//!     Err(ParseError::InsufficientData { needed, got }) => {
//!         eprintln!("Need {} bytes, got {}", needed, got);
//!     }
//!     Err(e) => eprintln!("Parse error: {}", e),
//! }
//! ```

mod csi_frame;
mod error;
mod esp32_parser;
pub mod aggregator;
mod bridge;
pub mod esp32;

// ADR-081: Rust mirror of the firmware radio abstraction layer (L1) and
// mesh sensing plane (L3). Lets host tests, simulators, and future
// coordinator-node Rust code drive the controller stack without
// touching any downstream signal/ruvector/train/mat crate.
pub mod radio_ops;

pub use csi_frame::{CsiFrame, CsiMetadata, SubcarrierData, Bandwidth, AntennaConfig};
pub use error::ParseError;
pub use esp32_parser::Esp32CsiParser;
pub use bridge::CsiData;
pub use radio_ops::{
    RadioOps, RadioMode, CaptureProfile, RadioHealth, RadioError, MockRadio,
    MeshRole, MeshMsgType, AuthClass, MeshHeader, NodeStatus, AnomalyAlert,
    MeshError, MESH_MAGIC, MESH_VERSION, MESH_HEADER_SIZE, MESH_MAX_PAYLOAD,
    crc32_ieee, decode_mesh, decode_node_status, decode_anomaly_alert,
    encode_health,
};
