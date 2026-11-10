//! Top-level UDP receiver task for CSI and radar frames.
//!
//! Complies with ELMAS.md: <= 350 lines per file, early returns, clean abstraction.

use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{debug, error, info, warn};

use wifi_densepose_sensing_server::udp_bind::UdpSourceAllowlist;
use wifi_densepose_sensing_server::udp_tee::UdpTee;

use super::esp32_csi::handle_esp32_csi_frame;
use super::esp32_edge_packets::handle_edge_packets;
use super::esp32_edge_vitals::handle_esp32_vitals;
use super::telemetry::UDP_INGRESS;
use super::vendor_dispatch::dispatch_vendor_datagram;
use crate::SharedState;

/// Runs the continuous UDP receiver loop on the configured socket.
pub async fn udp_receiver_task(
    state: SharedState,
    bind_ip: std::net::IpAddr,
    udp_port: u16,
    allowlist: Arc<UdpSourceAllowlist>,
    tee: Option<Arc<UdpTee>>,
) {
    let addr = format!("{bind_ip}:{udp_port}");
    let socket = match UdpSocket::bind(&addr).await {
        Ok(s) => {
            info!(
                "UDP listening on {addr} for ESP32, MediaTek, Qualcomm, RTL8721Dx CSI, and RTL8720F radar frames"
            );
            s
        }
        Err(e) => {
            error!("Failed to bind UDP {addr}: {e}");
            return;
        }
    };

    UDP_INGRESS
        .running
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let mut buf = vec![0u8; wifi_densepose_hardware::rtl8720f::RTL8720F_RADAR_MAX_FRAME_LEN];

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, src)) => {
                UDP_INGRESS
                    .datagrams
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                UDP_INGRESS.last_datagram_unix_ms.store(
                    chrono::Utc::now().timestamp_millis().max(0) as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );

                if !allowlist.admit(src.ip()) {
                    let dropped = UDP_INGRESS
                        .dropped_by_allowlist
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        + 1;
                    if dropped.is_power_of_two() {
                        warn!(
                            "Dropped UDP frame from disallowed source {src} (allowlist active; total dropped={dropped})"
                        );
                    } else {
                        debug!(
                            "Dropped UDP frame from disallowed source {src} (allowlist active; total dropped={dropped})"
                        );
                    }
                    continue;
                }

                if let Some(tee) = &tee {
                    tee.forward(&buf[..len]);
                }

                if dispatch_vendor_datagram(&buf, len, src, &state).await {
                    continue;
                }

                if handle_esp32_vitals(&buf, len, src, &state).await {
                    continue;
                }

                if handle_edge_packets(&buf, len, src, &state).await {
                    continue;
                }

                let _ = handle_esp32_csi_frame(&buf, len, src, &state).await;
            }
            Err(e) => {
                warn!("UDP recv error: {e}");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

