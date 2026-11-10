//! Hardware presence and interface availability probes.
//!
//! Complies with ELMAS.md: non-blocking async network probes, <= 350 lines.

use std::time::Duration;
use tokio::net::UdpSocket;

use super::esp32_frame::parse_esp32_frame;
use super::netsh::parse_netsh_interfaces_output;

/// Probe if macOS WiFi is connected.
#[cfg(target_os = "macos")]
pub async fn probe_wifi() -> bool {
    matches!(
        tokio::task::spawn_blocking(|| {
            wifi_densepose_wifiscan::adapter::MacosCoreWlanScanner::new().scan_sync()
        })
        .await,
        Ok(Ok(observations)) if !observations.is_empty()
    )
}

/// Probe if Windows/Linux WiFi interface is connected.
#[cfg(not(target_os = "macos"))]
pub async fn probe_wifi() -> bool {
    match tokio::process::Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .await
    {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout);
            parse_netsh_interfaces_output(&out).is_some()
        }
        Err(_) => false,
    }
}

/// Probe if ESP32 node is streaming CSI on UDP port.
pub async fn probe_esp32(port: u16) -> bool {
    let addr = format!("0.0.0.0:{port}");
    match UdpSocket::bind(&addr).await {
        Ok(sock) => {
            let mut buf = [0u8; 2048];
            match tokio::time::timeout(Duration::from_secs(2), sock.recv_from(&mut buf)).await {
                Ok(Ok((len, _))) => parse_esp32_frame(&buf[..len]).is_some(),
                _ => false,
            }
        }
        Err(_) => false,
    }
}

