//! Vendor RF, Qualcomm, MediaTek, and Realtek datagram dispatch.
//!
//! Complies with ELMAS.md: <= 350 lines per file, early returns, clean abstraction.

use std::net::SocketAddr;
use tracing::{debug, warn};

use crate::api::sensing::primary_source_with_realtek;
use crate::mediatek_activity;
use crate::mediatek_csi;
use crate::mediatek_csi_ring;
use crate::qualcomm_csi;
use crate::realtek_csi;
use crate::realtek_radar;
use crate::runtime::ingress::mediatek_room::{evict_mediatek_device_if_full, mediatek_room_update};
use crate::state::types::ESP32_OFFLINE_TIMEOUT;
use crate::SharedState;

/// Attempt to dispatch an incoming UDP datagram as a vendor-specific format.
/// Returns `true` if handled, or `false` to continue to ESP32 parsers.
pub async fn dispatch_vendor_datagram(
    buf: &[u8],
    len: usize,
    src: SocketAddr,
    state: &SharedState,
) -> bool {
    if len > 0 && buf[0] == b'{' {
        handle_vendor_rf_json(buf, len, src, state).await;
        return true;
    }

    if len < 4 {
        return false;
    }

    let magic = u32::from_le_bytes(buf[..4].try_into().expect("four-byte slice"));
    match magic {
        wifi_densepose_hardware::qualcomm_csi::QUALCOMM_CSI_MAGIC => {
            handle_qualcomm_csi(buf, len, src, state).await;
            true
        }
        wifi_densepose_hardware::mediatek_csi::MEDIATEK_CSI_MAGIC => {
            handle_mediatek_csi(buf, len, src, state).await;
            true
        }
        wifi_densepose_hardware::rtl8720f::RTL8720F_RADAR_MAGIC => {
            handle_rtl8720f_radar(buf, len, src, state).await;
            true
        }
        wifi_densepose_hardware::realtek_csi::RAC1_MAGIC => {
            handle_realtek_csi(buf, len, src, state).await;
            true
        }
        _ => false,
    }
}

async fn handle_vendor_rf_json(buf: &[u8], len: usize, src: SocketAddr, state: &SharedState) {
    match serde_json::from_slice::<wifi_densepose_hardware::vendor_rf::VendorRfEvent>(&buf[..len])
        .map_err(|error| error.to_string())
        .and_then(|event| {
            wifi_densepose_sensing_server::vendor_rf::VendorEventSnapshot::from_event(event)
                .map_err(|error| error.to_string())
        }) {
        Ok(snapshot) if snapshot.event.synthetic => {
            debug!(
                "Vendor RF event from {src}: vendor={} capability={:?} seq={}",
                snapshot.event.vendor.as_str(),
                snapshot.event.capability,
                snapshot.event.sequence
            );
            let json = serde_json::to_string(&snapshot).ok();
            let mut s = state.write().await;
            s.source = snapshot.source.clone();
            s.last_vendor_rf_frame = Some(std::time::Instant::now());
            s.latest_vendor_rf
                .insert(snapshot.event.vendor.as_str().to_string(), snapshot);
            if let Some(json) = json {
                let _ = s.tx.send(json);
            }
        }
        Ok(_) => warn!(
            "Rejected non-synthetic canonical vendor event from {src}; live payloads must use the provider decoder HTTP route"
        ),
        Err(error) => warn!("Rejected ADR-270 vendor event from {src}: {error}"),
    }
}

async fn handle_qualcomm_csi(buf: &[u8], len: usize, src: SocketAddr, state: &SharedState) {
    match wifi_densepose_hardware::qualcomm_csi::CsiFrame::from_bytes(&buf[..len]) {
        Ok((frame, consumed)) if consumed == len => {
            let snapshot = qualcomm_csi::QualcommCsiSnapshot::from_frame(&frame);
            debug!(
                "Qualcomm CSI from {src}: profile={} seq={} dimensions={}x{}x{}",
                snapshot.chipset,
                snapshot.sequence,
                snapshot.tx_count,
                snapshot.rx_count,
                snapshot.subcarrier_count
            );
            let json = serde_json::to_string(&snapshot).ok();
            let mut s = state.write().await;
            s.source = snapshot.source.to_string();
            s.last_qualcomm_frame = Some(std::time::Instant::now());
            s.latest_qualcomm_csi = Some(snapshot);
            if let Some(json) = json {
                let _ = s.tx.send(json);
            }
        }
        Ok((_, consumed)) => warn!(
            "Qualcomm CSI datagram from {src} has trailing bytes: consumed={consumed} received={len}"
        ),
        Err(error) => warn!("Rejected Qualcomm CSI datagram from {src}: {error}"),
    }
}

async fn handle_mediatek_csi(buf: &[u8], len: usize, src: SocketAddr, state: &SharedState) {
    match wifi_densepose_hardware::mediatek_csi::CsiFrame::from_bytes(&buf[..len]) {
        Ok((frame, consumed)) if consumed == len => {
            let snapshot = mediatek_csi::MediatekCsiSnapshot::from_frame(&frame);
            debug!(
                "MediaTek CSI from {src}: profile={} seq={} dimensions={}x{}x{}",
                snapshot.chipset,
                snapshot.sequence,
                snapshot.tx_count,
                snapshot.rx_count,
                snapshot.subcarrier_count
            );
            let json = serde_json::to_string(&snapshot).ok();
            let now = std::time::Instant::now();
            let device_id = snapshot.device_id.clone();
            let source_label = snapshot.source.to_string();
            let mean_amplitude = snapshot.mean_amplitude;
            let peak_amplitude = snapshot.peak_amplitude;
            let mut s = state.write().await;
            s.source = source_label.clone();
            s.last_mediatek_frame = Some(now);
            evict_mediatek_device_if_full(&mut s, &device_id);
            s.mediatek_csi_by_device
                .insert(device_id.clone(), (snapshot.clone(), now));
            s.latest_mediatek_csi = Some(snapshot);
            if let Some(json) = json {
                let _ = s.tx.send(json);
            }

            let ring_capacity = s.csi_ring_capacity;
            s.mediatek_csi_ring_by_device
                .entry(device_id.clone())
                .or_insert_with(|| mediatek_csi_ring::DeviceRing::new(ring_capacity))
                .push(&frame);

            if let Some(mean_amp) = mean_amplitude {
                let peak_amp = peak_amplitude.unwrap_or(mean_amp);
                if let Some(ring) = s.mediatek_csi_ring_by_device.get(&device_id) {
                    let (fast, frames_2s) = ring.fast_activity(mediatek_activity::FAST_WINDOW);
                    let floor_seed = s.activity_floor_seed.get(&device_id).copied();
                    s.mediatek_activity_by_device
                        .entry(device_id.clone())
                        .or_insert_with(|| {
                            let mut tracker = mediatek_activity::ActivityTracker::default();
                            if let Some(seed) = floor_seed {
                                tracker.seed_floor(seed);
                            }
                            tracker
                        })
                        .observe(now, fast, frames_2s);
                }

                s.mediatek_heuristic_by_device
                    .entry(device_id)
                    .or_default()
                    .observe(now, mean_amp as f64, peak_amp as f64);
                s.tick += 1;
                let tick = s.tick;
                let update = mediatek_room_update(
                    &mut s,
                    now,
                    ESP32_OFFLINE_TIMEOUT,
                    tick,
                    source_label,
                );
                if let Ok(json) = serde_json::to_string(&update) {
                    let _ = s.tx.send(json);
                }
                s.latest_update = Some(update);
            }
        }
        Ok((_, consumed)) => warn!(
            "MediaTek CSI datagram from {src} has trailing bytes: consumed={consumed} received={len}"
        ),
        Err(error) => warn!("Rejected MediaTek CSI datagram from {src}: {error}"),
    }
}

async fn handle_rtl8720f_radar(buf: &[u8], len: usize, src: SocketAddr, state: &SharedState) {
    match wifi_densepose_hardware::rtl8720f::RadarFrame::from_bytes(&buf[..len]) {
        Ok((frame, consumed)) if consumed == len => {
            let snapshot = realtek_radar::RealtekRadarSnapshot::from_frame(&frame);
            debug!(
                "RTL8720F radar from {src}: type={} seq={} elements={}",
                snapshot.report_type, snapshot.sequence, snapshot.element_count
            );
            let json = serde_json::to_string(&snapshot).ok();
            let mut s = state.write().await;
            s.source = snapshot.source.to_string();
            s.last_realtek_frame = Some(std::time::Instant::now());
            s.latest_realtek_radar = Some(snapshot);
            if let Some(json) = json {
                let _ = s.tx.send(json);
            }
        }
        Ok((_, consumed)) => warn!(
            "RTL8720F radar datagram from {src} has trailing bytes: consumed={consumed} received={len}"
        ),
        Err(error) => warn!("Rejected RTL8720F radar datagram from {src}: {error}"),
    }
}

async fn handle_realtek_csi(buf: &[u8], len: usize, src: SocketAddr, state: &SharedState) {
    match wifi_densepose_hardware::realtek_csi::CsiFrame::from_bytes(&buf[..len]) {
        Ok((frame, consumed)) if consumed == len => {
            let snapshot = realtek_csi::RealtekCsiSnapshot::from_frame(&frame);
            debug!(
                "RTL8721Dx CSI from {src}: node={} seq={} subcarriers={}",
                snapshot.node_id, snapshot.sequence, snapshot.num_sub_carrier
            );
            let json = serde_json::to_string(&snapshot).ok();
            let mut s = state.write().await;
            s.source = primary_source_with_realtek(s.last_esp32_frame, snapshot.source);
            s.last_realtek_csi_frame = Some(std::time::Instant::now());
            s.latest_realtek_csi = Some(snapshot);
            if let Some(json) = json {
                let _ = s.tx.send(json);
            }
        }
        Ok((_, consumed)) => warn!(
            "RTL8721Dx CSI datagram from {src} has trailing bytes: consumed={consumed} received={len}"
        ),
        Err(error) => warn!("Rejected RTL8721Dx CSI datagram from {src}: {error}"),
    }
}

