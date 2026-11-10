//! MediaTek CSI REST endpoints.

use axum::{routing::get, Router};
use crate::mediatek_activity;
use crate::mediatek_csi_ring;
use crate::mediatek_devices;
use crate::SharedState;

/// Create sub-router for MediaTek CSI endpoints.
pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route(
            "/api/v1/csi/mediatek/devices",
            get(mediatek_devices::mediatek_csi_devices),
        )
        .route(
            "/api/v1/csi/mediatek/devices/:device_id/frames",
            get(mediatek_csi_ring::mediatek_csi_frames),
        )
        .route(
            "/api/v1/csi/mediatek/devices/:device_id/summary",
            get(mediatek_csi_ring::mediatek_csi_summary),
        )
        .route(
            "/api/v1/csi/mediatek/activity",
            get(mediatek_activity::mediatek_activity_history),
        )
}
