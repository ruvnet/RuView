//! Sensing update observation and telemetry event dispatching.
//!
//! Complies with ELMAS.md: <= 350 lines per file, early returns.

use tracing::info;
use wifi_densepose_sensing_server::domain::SensingUpdate;
use wifi_densepose_sensing_server::{semconv, telemetry};

/// Cadence, in sensing ticks, of periodic telemetry snapshots.
pub const TELEMETRY_SNAPSHOT_TICKS: u64 = 100;

/// Emit curated telemetry events for a finished sensing cycle.
pub fn observe_sensing_update(prev: Option<&SensingUpdate>, update: &SensingUpdate) {
    if !telemetry::curated_events_enabled() {
        return;
    }

    let presence = update.classification.presence;
    if prev.map(|u| u.classification.presence) != Some(presence) {
        let state = if presence { "present" } else { "absent" };
        info!(
            name: semconv::EVENT_RUVIEW_PRESENCE_CHANGED,
            {
                "ruview.presence.state" = state,
                "ruview.motion.level" = %update.classification.motion_level,
                "ruview.inference.confidence" = update.classification.confidence,
                "ruview.persons.count" = update.estimated_persons.unwrap_or(0) as u64,
                "ruview.csi.source" = %update.source,
            },
            "presence changed: {state}"
        );
    }

    if update.tick % TELEMETRY_SNAPSHOT_TICKS == 0 {
        info!(
            name: semconv::EVENT_RUVIEW_CSI_STATS,
            {
                "ruview.csi.frames_total" = update.tick,
                "ruview.csi.nodes_active" = update.nodes.len() as u64,
                "ruview.csi.source" = %update.source,
            },
            "csi stats: {} frames processed, {} active node(s)",
            update.tick,
            update.nodes.len()
        );

        if let Some(v) = &update.vital_signs {
            if v.breathing_rate_bpm.is_some() || v.heart_rate_bpm.is_some() {
                info!(
                    name: semconv::EVENT_RUVIEW_VITALS_ESTIMATE,
                    {
                        "ruview.vitals.breathing_rate_bpm" = v.breathing_rate_bpm.unwrap_or(0.0),
                        "ruview.vitals.heart_rate_bpm" = v.heart_rate_bpm.unwrap_or(0.0),
                        "ruview.vitals.breathing_confidence" = v.breathing_confidence,
                        "ruview.vitals.heartbeat_confidence" = v.heartbeat_confidence,
                        "ruview.csi.source" = %update.source,
                    },
                    "vitals estimate"
                );
            }
        }
    }
}

pub fn update_introspection_tap(s: &mut crate::state::types::AppStateInner, amplitudes: &[f64]) {
    let intro_feature = if amplitudes.is_empty() {
        0.0
    } else {
        amplitudes.iter().copied().sum::<f64>() / amplitudes.len() as f64
    };
    let intro_ts_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let _ = s.intro.update(intro_ts_ns, intro_feature);
    if let Ok(intro_json) = serde_json::to_string(s.intro.snapshot()) {
        let _ = s.intro_tx.send(intro_json);
    }
}

pub fn build_active_node_info(
    s: &mut crate::state::types::AppStateInner,
    now: std::time::Instant,
) -> Vec<wifi_densepose_sensing_server::domain::NodeInfo> {
    use wifi_densepose_sensing_server::domain::{
        node_inference_for, node_is_fresh, node_positions_by_active_id,
        note_default_position_nodes, NodeInfo, DEFAULT_NODE_POSITION,
    };

    let suppress_raw = s.engine_bridge.suppress_raw_outputs();
    let resolved_positions =
        node_positions_by_active_id(&s.node_positions_config, &s.node_states, now);
    note_default_position_nodes(
        &s.node_positions_config,
        &s.node_states,
        &mut s.default_position_noted,
        now,
    );

    s.node_states
        .iter()
        .filter(|(_, n)| node_is_fresh(n, now))
        .map(|(&id, n)| NodeInfo {
            node_id: id,
            device_id: None,
            rssi_dbm: n.rssi_history.back().copied().unwrap_or(0.0),
            position: resolved_positions
                .get(&id)
                .copied()
                .unwrap_or(DEFAULT_NODE_POSITION),
            amplitude: if suppress_raw {
                vec![]
            } else {
                n.frame_history
                    .back()
                    .map(|a| {
                        a.iter()
                            .take(crate::adaptive_classifier::RECORDED_AMPLITUDE_LEN)
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default()
            },
            subcarrier_count: if suppress_raw {
                0
            } else {
                n.frame_history.back().map_or(0, |a| a.len())
            },
            sync: n.sync_snapshot(),
            node_inference: Some(node_inference_for(n, now)),
            mediatek_diagnostics: None,
        })
        .collect()
}
