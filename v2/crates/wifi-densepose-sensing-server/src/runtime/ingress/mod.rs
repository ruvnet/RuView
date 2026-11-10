//! UDP Ingress, packet dispatch, telemetry and room fusion.
//!
//! Complies with ELMAS.md: <= 350 lines, modular architecture.

pub mod esp32_csi;
pub mod esp32_edge_packets;
pub mod esp32_edge_vitals;
pub mod mediatek_room;
pub mod observe;
pub mod task;
pub mod telemetry;
pub mod vendor_dispatch;

pub use task::udp_receiver_task;

pub use mediatek_room::{
    clamp_csi_ring, evict_mediatek_device_if_full, mediatek_room_update,
    MAX_CSI_RING_CAPACITY, MAX_MEDIATEK_DEVICES,
};
pub use observe::*;
pub use telemetry::{
    processing_health, processing_state, ProcessingState, UdpIngress, UDP_INGRESS,
};

#[cfg(test)]
mod tests_capacity;
#[cfg(test)]
mod tests_multi_device;

