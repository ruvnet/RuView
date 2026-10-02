//! WiFi-DensePose Sensing Server library.
//!
//! This crate provides:
//! - Vital sign detection from WiFi CSI amplitude data
//! - RVF (RuVector Format) binary container for model weights
//! - Opt-in bearer-token auth for the `/api/v1/*` HTTP surface (`bearer_auth`)
//! - Host-header allowlist / DNS-rebinding defense (`host_validation`)
//! - Generic, leak-free internal-error responses (`error_response`, ADR-080 #2)
//! - Real-time CSI introspection / low-latency tap (`introspection`, ADR-099)

pub mod bearer_auth;
pub mod bootstrap_baseline;
pub mod browser_session;
pub mod ws_ticket;
pub mod cli;
pub mod dataset;
pub mod discovery;
pub mod edge_registry;
pub mod error_response;
pub mod host_validation;
/// ADR-297: per-node vs. fused room inference, with deterministic fusion.
pub mod inference;
pub mod introspection;
pub mod matter;
pub mod model_format;
pub mod mqtt;
pub mod path_safety;
/// ADR-323: fail-closed pose physics integration and raw/refined view selection.
pub mod pose_physics;
/// ADR-295: canonical source-provenance state machine (synthetic can never
/// present as live).
pub mod provenance;
pub mod semantic;
/// ADR-262 P3: the live RuField surface — turns the governed sensing cycle into
/// signed RuField `FieldEvent`s on the additive `/api/field` + `/ws/field`
/// endpoints, via the `wifi-densepose-rufield` anti-corruption bridge.
pub mod rufield_surface;
pub mod rvf_container;
pub mod rvf_pipeline;
pub mod semconv;
pub mod telemetry;
#[allow(dead_code)]
pub mod trainer;
/// ADR-296: UDP data-plane bind scope decision + source IP/CIDR allowlist.
pub mod udp_bind;
/// ADR-380: loopback raw-datagram tee for second consumers of the CSI stream.
pub mod udp_tee;
pub mod vital_signs;
/// ADR-270 Mist and NETGEAR telemetry providers.
pub mod vendor_mist_netgear;
/// ADR-270 Origin AI and Plume/OpenSync providers.
pub mod vendor_origin_plume;
/// ADR-270 scalar, network-only, and fail-closed vendor providers.
pub mod vendor_remaining;
/// ADR-270 provider registry and canonical event helpers.
pub mod vendor_rf;

// ADR-185 §3.2/§13: the AETHER pure-compute stack (contrastive embedding,
// CSI-to-pose transformer, SONA, quantization) was hoisted into the std-only
// `wifi-densepose-aether` leaf crate so the Python `[aether]` wheel can bind it
// without this crate's Axum/tokio/worldgraph/ruvector tree. Re-exported here so
// this crate's own code (`crate::embedding`, `crate::graph_transformer`,
// `crate::sona`) and public API (`wifi_densepose_sensing_server::embedding`, …)
// are unchanged.
pub use wifi_densepose_aether::{embedding, graph_transformer, sona, sparse_inference};
