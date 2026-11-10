//! Hardware and network frame ingestion domain.
//!
//! Handles raw datagram reception and binary parsing for:
//! - ESP32 CSI raw IQ frames (ADR-018, ADR-110)
//! - ESP32 edge vitals (ADR-039)
//! - Tier 3 WASM edge output packets (ADR-040)
//! - Multi-sensor fused vitals (ADR-063, mmWave + WiFi)

pub mod esp32_frame;
pub mod esp32_vitals;
pub mod netsh;
pub mod probes;
pub mod simulation;
pub mod source_plan;

pub use esp32_frame::{parse_esp32_frame, Esp32Frame};
pub use esp32_vitals::{
    parse_edge_fused_vitals, parse_esp32_vitals, parse_wasm_output, sanitize_edge_person_count,
    EdgeFusedVitalsPacket, Esp32VitalsPacket, WasmEvent, WasmOutputPacket, EDGE_MAX_PERSONS,
};
pub use netsh::parse_netsh_interfaces_output;
pub use probes::{probe_esp32, probe_wifi};
pub use simulation::generate_simulated_frame;
pub use source_plan::{plan_source, SourcePlan, SOURCE_VALUES};

