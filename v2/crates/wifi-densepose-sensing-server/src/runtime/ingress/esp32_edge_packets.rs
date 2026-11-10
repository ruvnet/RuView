//! ESP32 sync, fused vitals, and WASM datagram dispatch.
//!
//! Complies with ELMAS.md: <= 350 lines per file, early returns, clean abstraction.

use std::net::SocketAddr;
use tracing::debug;

use wifi_densepose_sensing_server::domain::node::NodeState;
use wifi_densepose_sensing_server::ingest::{parse_edge_fused_vitals, parse_wasm_output};

use crate::SharedState;

/// Attempt to handle an incoming datagram as a sync, fused vitals, or WASM packet.
/// Returns `true` if handled, `false` otherwise.
pub async fn handle_edge_packets(
    buf: &[u8],
    len: usize,
    src: SocketAddr,
    state: &SharedState,
) -> bool {
    // ADR-110 §A0.12: Try sync packet (magic 0xC511_A110).
    if len >= wifi_densepose_hardware::SYNC_PACKET_SIZE {
        let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if magic == wifi_densepose_hardware::SYNC_PACKET_MAGIC {
            if handle_sync_packet(buf, len, src, state).await {
                return true;
            }
        }
    }

    // ADR-063: Try edge fused vitals packet (magic 0xC511_0004).
    if let Some(fused) = parse_edge_fused_vitals(&buf[..len]) {
        debug!(
            "Edge fused vitals from {src}: node={} br={:.1} hr={:.1} mmwave_targets={} fusion_conf={}",
            fused.node_id,
            fused.breathing_rate_bpm,
            fused.heartrate_bpm,
            fused.mmwave_targets,
            fused.fusion_confidence,
        );
        let s = state.write().await;
        if let Ok(json) = serde_json::to_string(&serde_json::json!({
            "type": "edge_fused_vitals",
            "node_id": fused.node_id,
            "breathing_rate_bpm": fused.breathing_rate_bpm,
            "heartrate_bpm": fused.heartrate_bpm,
            "n_persons": fused.n_persons,
            "person_count_valid": fused.person_count_valid,
            "fusion_confidence": fused.fusion_confidence,
            "mmwave": {
                "hr_bpm": fused.mmwave_hr_bpm,
                "br_bpm": fused.mmwave_br_bpm,
                "distance_cm": fused.mmwave_distance_cm,
                "targets": fused.mmwave_targets,
                "confidence": fused.mmwave_confidence,
                "type": fused.mmwave_type,
            },
            "motion_energy": fused.motion_energy,
            "presence_score": fused.presence_score,
            "timestamp_ms": fused.timestamp_ms,
        })) {
            let _ = s.tx.send(json);
        }
        return true;
    }

    // ADR-040: Try WASM output packet (magic 0xC511_0007).
    if let Some(wasm_output) = parse_wasm_output(&buf[..len]) {
        debug!(
            "WASM output from {src}: node={} module={} events={}",
            wasm_output.node_id,
            wasm_output.module_id,
            wasm_output.events.len()
        );
        let mut s = state.write().await;
        if let Ok(json) = serde_json::to_string(&serde_json::json!({
            "type": "wasm_event",
            "node_id": wasm_output.node_id,
            "module_id": wasm_output.module_id,
            "events": wasm_output.events,
        })) {
            let _ = s.tx.send(json);
        }
        s.latest_wasm_events = Some(wasm_output);
        return true;
    }

    false
}

async fn handle_sync_packet(
    buf: &[u8],
    len: usize,
    src: SocketAddr,
    state: &SharedState,
) -> bool {
    match wifi_densepose_hardware::SyncPacket::from_bytes(&buf[..len]) {
        Ok(sync) => {
            debug!(
                "ESP32 sync from {src}: node={} leader={} valid={} smoothed={} seq={} offset_us={}",
                sync.node_id,
                sync.flags.is_leader,
                sync.flags.is_valid,
                sync.flags.smoothed_used,
                sync.sequence,
                sync.local_minus_epoch_us()
            );
            let mut s = state.write().await;
            let node_id = sync.node_id;
            let ns = s.node_states.entry(node_id).or_insert_with(NodeState::new);
            ns.apply_sync_packet(sync, std::time::Instant::now());
            true
        }
        Err(e) => {
            debug!("Sync packet decode error from {src}: {e}");
            false
        }
    }
}

