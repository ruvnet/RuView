//! Per-device MediaTek CSI listing.
//!
//! Multiple MediaTek bridges (e.g. two WN586X3 routers) each carry a distinct
//! `device_id` in their `MTC1` frames (ADR-267), but the single
//! `AppStateInner::latest_mediatek_csi` slot only ever remembers the most
//! recently arrived one — a second unit silently overwrites the first. This
//! module reads `AppStateInner::mediatek_csi_by_device`, a per-device map the
//! UDP ingest loop in `main.rs` populates alongside (not instead of) that
//! single slot, so every device can be fused as its own sensor.

use crate::mediatek_csi::MediatekCsiSnapshot;
use crate::SharedState;
use axum::extract::State;
use axum::response::Json;
use std::time::{Duration, Instant};

/// Entries older than this are dropped from the `/devices` response. The map
/// itself is not pruned here — a device that starts sending again simply
/// reappears once its snapshot is fresh. `pub(crate)` so `mediatek_activity`
/// can apply the SAME cutoff (a downstream consumer observed `/devices`
/// and `activity.devices` naming different receiver sets after one unit
/// restarted, because they used different staleness windows) rather than a
/// second constant that could drift out of sync with this one.
pub(crate) const MEDIATEK_DEVICE_STALE: Duration = Duration::from_secs(30);

/// `GET /api/v1/csi/mediatek/devices` — every MediaTek device seen within the
/// staleness window, each snapshot annotated with `age_ms`, sorted by
/// `device_id`. Shape: `{"devices": [...], "total": N}`.
pub(crate) async fn mediatek_csi_devices(
    State(state): State<SharedState>,
) -> Json<serde_json::Value> {
    let s = state.read().await;
    let now = Instant::now();

    let mut entries: Vec<(&String, &MediatekCsiSnapshot, Duration)> = s
        .mediatek_csi_by_device
        .iter()
        .filter_map(|(device_id, (snapshot, seen))| {
            let age = now.saturating_duration_since(*seen);
            (age <= MEDIATEK_DEVICE_STALE).then_some((device_id, snapshot, age))
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    let devices: Vec<serde_json::Value> = entries
        .into_iter()
        .filter_map(|(_, snapshot, age)| {
            let mut value = serde_json::to_value(snapshot).ok()?;
            if let Some(obj) = value.as_object_mut() {
                obj.insert(
                    "age_ms".to_string(),
                    serde_json::json!(age.as_millis() as u64),
                );
            }
            Some(value)
        })
        .collect();

    Json(serde_json::json!({ "total": devices.len(), "devices": devices }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppStateInner;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use wifi_densepose_hardware::mediatek_csi::simulator::{MediatekCsiSimulator, SimulatorConfig};

    fn simulator(device_id: u64) -> MediatekCsiSimulator {
        MediatekCsiSimulator::new(SimulatorConfig {
            device_id,
            ..Default::default()
        })
        .expect("valid simulator config")
    }

    /// Two device_ids arriving interleaved (unit1, unit2, unit1-again) must
    /// both be present in `/devices`, and the shared single-slot
    /// `latest_mediatek_csi` must equal whichever arrived most recently.
    #[tokio::test]
    async fn two_devices_interleaved_both_present_latest_is_most_recent() {
        let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));

        let mut unit1 = simulator(0xa9be_7e5b_b164_4d3a);
        let mut unit2 = simulator(0x1122_3344_5566_7788);

        let unit1_frame1 = MediatekCsiSnapshot::from_frame(&unit1.next_frame());
        let unit2_frame1 = MediatekCsiSnapshot::from_frame(&unit2.next_frame());
        let unit1_frame2 = MediatekCsiSnapshot::from_frame(&unit1.next_frame());
        assert_ne!(unit1_frame1.device_id, unit2_frame1.device_id);
        assert_eq!(unit1_frame1.device_id, unit1_frame2.device_id);
        assert_eq!(unit1_frame2.sequence, unit1_frame1.sequence + 1);

        // Ingest in interleaved order, mirroring the UDP loop in main.rs:
        // insert into the per-device map AND overwrite the single slot.
        for snapshot in [
            unit1_frame1.clone(),
            unit2_frame1.clone(),
            unit1_frame2.clone(),
        ] {
            let mut s = state.write().await;
            let now = Instant::now();
            s.mediatek_csi_by_device
                .insert(snapshot.device_id.clone(), (snapshot.clone(), now));
            s.latest_mediatek_csi = Some(snapshot);
        }

        let Json(value) = mediatek_csi_devices(State(state.clone())).await;
        let devices = value["devices"].as_array().expect("devices array");
        assert_eq!(
            value["total"], 2,
            "both devices must be present, not overwritten"
        );
        assert_eq!(devices.len(), 2);

        let ids: Vec<&str> = devices
            .iter()
            .map(|d| d["device_id"].as_str().unwrap())
            .collect();
        let mut expected_ids = vec![
            unit1_frame1.device_id.as_str(),
            unit2_frame1.device_id.as_str(),
        ];
        expected_ids.sort();
        assert_eq!(ids, expected_ids, "sorted by device_id");

        let unit1_entry = devices
            .iter()
            .find(|d| d["device_id"] == unit1_frame1.device_id.as_str())
            .expect("unit1 present");
        assert_eq!(
            unit1_entry["sequence"], unit1_frame2.sequence,
            "keeps unit1's most recent frame"
        );
        assert!(unit1_entry["age_ms"].as_u64().is_some());

        let s = state.read().await;
        assert_eq!(
            s.latest_mediatek_csi
                .as_ref()
                .map(|snap| snap.device_id.clone()),
            Some(unit1_frame1.device_id.clone()),
            "single latest slot still reflects whichever device arrived last, unchanged behavior"
        );
        assert_eq!(
            s.latest_mediatek_csi.as_ref().map(|snap| snap.sequence),
            Some(unit1_frame2.sequence)
        );
    }

    /// A device older than the staleness window is dropped from `/devices`
    /// even though its entry is still in the map.
    #[tokio::test]
    async fn stale_device_is_excluded_from_response() {
        let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));
        let mut unit = simulator(0xdead_beef_0000_0001);
        let frame = MediatekCsiSnapshot::from_frame(&unit.next_frame());

        {
            let mut s = state.write().await;
            let stale_seen = Instant::now() - Duration::from_secs(31);
            s.mediatek_csi_by_device
                .insert(frame.device_id.clone(), (frame, stale_seen));
        }

        let Json(value) = mediatek_csi_devices(State(state)).await;
        assert_eq!(value["total"], 0);
        assert!(value["devices"].as_array().unwrap().is_empty());
    }
}
