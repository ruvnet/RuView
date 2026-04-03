use serde::Serialize;
use std::process::Command;

/// macOS WiFi network info from CoreWLAN via system_profiler.
#[derive(Debug, Clone, Serialize)]
pub struct MacWifiInfo {
    pub ssid: Option<String>,
    pub bssid: Option<String>,
    pub channel: Option<u32>,
    pub rssi: Option<i32>,
    pub noise: Option<i32>,
    pub tx_rate: Option<f64>,
    pub security: Option<String>,
    pub phy_mode: Option<String>,
}

/// macOS system info relevant for RuView diagnostics.
#[derive(Debug, Clone, Serialize)]
pub struct MacSystemInfo {
    pub os_version: String,
    pub arch: String,
    pub model: Option<String>,
    pub wifi_interface: Option<String>,
    pub wifi_power: bool,
    pub serial_drivers: Vec<String>,
}

/// Permission check result for macOS-specific capabilities.
#[derive(Debug, Clone, Serialize)]
pub struct MacPermissions {
    pub network_access: bool,
    pub usb_access: bool,
    pub wifi_scan: bool,
    pub location_services: bool,
}

/// Get current WiFi connection info using CoreWLAN via airport CLI.
/// macOS-only: uses `/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport -I`
#[tauri::command]
pub async fn macos_wifi_info() -> Result<MacWifiInfo, String> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err("macOS-only command".into());
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport")
            .arg("-I")
            .output()
            .map_err(|e| format!("Failed to query WiFi: {}", e))?;

        if !output.status.success() {
            return Err("airport command failed".into());
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut info = MacWifiInfo {
            ssid: None,
            bssid: None,
            channel: None,
            rssi: None,
            noise: None,
            tx_rate: None,
            security: None,
            phy_mode: None,
        };

        for line in text.lines() {
            let parts: Vec<&str> = line.splitn(2, ':').collect();
            if parts.len() != 2 {
                continue;
            }
            let key = parts[0].trim();
            let val = parts[1].trim();

            match key {
                "SSID" => info.ssid = Some(val.to_string()),
                "BSSID" => info.bssid = Some(val.to_string()),
                "channel" => info.channel = val.split(',').next().and_then(|v| v.trim().parse().ok()),
                "agrCtlRSSI" => info.rssi = val.parse().ok(),
                "agrCtlNoise" => info.noise = val.parse().ok(),
                "lastTxRate" => info.tx_rate = val.parse().ok(),
                "link auth" => info.security = Some(val.to_string()),
                "PHY Mode" | "phyMode" => info.phy_mode = Some(val.to_string()),
                _ => {}
            }
        }

        Ok(info)
    }
}

/// Scan nearby WiFi networks (macOS only).
/// Returns list of visible networks with RSSI for site survey.
#[tauri::command]
pub async fn macos_wifi_scan() -> Result<Vec<MacWifiScanResult>, String> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err("macOS-only command".into());
    }

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport")
            .arg("-s")
            .output()
            .map_err(|e| format!("WiFi scan failed: {}", e))?;

        if !output.status.success() {
            return Err("WiFi scan command failed. Location Services may need to be enabled.".into());
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut results = Vec::new();

        for line in text.lines().skip(1) {
            // airport -s format: SSID BSSID RSSI CHANNEL HT CC SECURITY
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Parse fixed-width columns (BSSID is at a fixed position)
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 7 {
                continue;
            }

            // Find BSSID (xx:xx:xx:xx:xx:xx pattern) to anchor parsing
            let bssid_idx = parts.iter().position(|p| p.matches(':').count() == 5);
            if let Some(idx) = bssid_idx {
                let ssid = if idx > 0 {
                    parts[..idx].join(" ")
                } else {
                    String::new()
                };

                results.push(MacWifiScanResult {
                    ssid,
                    bssid: parts[idx].to_string(),
                    rssi: parts.get(idx + 1).and_then(|v| v.parse().ok()).unwrap_or(0),
                    channel: parts.get(idx + 2).and_then(|v| v.split(',').next().and_then(|c| c.parse().ok())).unwrap_or(0),
                    security: parts.get(idx + 5..).map(|s| s.join(" ")).unwrap_or_default(),
                });
            }
        }

        results.sort_by(|a, b| b.rssi.cmp(&a.rssi));
        Ok(results)
    }
}

/// WiFi scan result entry.
#[derive(Debug, Clone, Serialize)]
pub struct MacWifiScanResult {
    pub ssid: String,
    pub bssid: String,
    pub rssi: i32,
    pub channel: u32,
    pub security: String,
}

/// Get macOS system info relevant to RuView operation.
#[tauri::command]
pub async fn macos_system_info() -> Result<MacSystemInfo, String> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err("macOS-only command".into());
    }

    #[cfg(target_os = "macos")]
    {
        // OS version
        let os_version = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".into());

        // Architecture
        let arch = Command::new("uname")
            .arg("-m")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".into());

        // Model identifier
        let model = Command::new("sysctl")
            .args(["-n", "hw.model"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

        // WiFi interface
        let wifi_interface = Command::new("networksetup")
            .args(["-listallhardwareports"])
            .output()
            .ok()
            .and_then(|o| {
                let text = String::from_utf8_lossy(&o.stdout).to_string();
                let mut found_wifi = false;
                for line in text.lines() {
                    if line.contains("Wi-Fi") {
                        found_wifi = true;
                        continue;
                    }
                    if found_wifi && line.starts_with("Device:") {
                        return Some(line.replace("Device:", "").trim().to_string());
                    }
                }
                None
            });

        // WiFi power status
        let wifi_power = wifi_interface.as_ref()
            .and_then(|iface| {
                Command::new("networksetup")
                    .args(["-getairportpower", iface])
                    .output()
                    .ok()
            })
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("On"))
            .unwrap_or(false);

        // Check for USB serial drivers (kext)
        let serial_drivers = detect_serial_drivers();

        Ok(MacSystemInfo {
            os_version,
            arch,
            model,
            wifi_interface,
            wifi_power,
            serial_drivers,
        })
    }
}

/// Detect installed USB serial drivers on macOS.
#[cfg(target_os = "macos")]
fn detect_serial_drivers() -> Vec<String> {
    let drivers_to_check = [
        ("CH34x", "com.wch.usbserial.CH34x"),
        ("CP210x", "com.silabs.driver.CP210xVCPDriver"),
        ("FTDI", "com.FTDI.driver.FTDIUSBSerialDriver"),
        ("CH9102", "com.wch.usbserial.CH9102"),
    ];

    let mut found = Vec::new();

    for (name, bundle_id) in &drivers_to_check {
        if let Ok(output) = Command::new("kextstat").output() {
            let text = String::from_utf8_lossy(&output.stdout);
            if text.contains(bundle_id) {
                found.push(name.to_string());
            }
        }
    }

    // Also check /dev for any connected USB serial devices
    if let Ok(entries) = std::fs::read_dir("/dev") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("cu.usb") || name.starts_with("cu.wch") {
                found.push(format!("/dev/{}", name));
            }
        }
    }

    found
}

/// Check macOS permissions relevant to RuView.
#[tauri::command]
pub async fn macos_check_permissions() -> Result<MacPermissions, String> {
    #[cfg(not(target_os = "macos"))]
    {
        return Err("macOS-only command".into());
    }

    #[cfg(target_os = "macos")]
    {
        // Network access: try binding a socket
        let network_access = std::net::UdpSocket::bind("0.0.0.0:0").is_ok();

        // USB access: check /dev/cu.usb* exists
        let usb_access = std::fs::read_dir("/dev")
            .map(|entries| {
                entries.flatten().any(|e| {
                    e.file_name().to_string_lossy().starts_with("cu.usb")
                })
            })
            .unwrap_or(false);

        // WiFi scan: try running airport
        let wifi_scan = Command::new("/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport")
            .arg("-I")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        // Location services (needed for WiFi scanning)
        let location_services = Command::new("defaults")
            .args(["read", "/var/db/locationd/clients.plist"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(true); // Assume enabled if can't check

        Ok(MacPermissions {
            network_access,
            usb_access,
            wifi_scan,
            location_services,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wifi_info_struct() {
        let info = MacWifiInfo {
            ssid: Some("TestNet".into()),
            bssid: Some("AA:BB:CC:DD:EE:FF".into()),
            channel: Some(6),
            rssi: Some(-45),
            noise: Some(-90),
            tx_rate: Some(866.0),
            security: Some("wpa2-psk".into()),
            phy_mode: Some("802.11ac".into()),
        };
        assert_eq!(info.ssid, Some("TestNet".into()));
        assert_eq!(info.channel, Some(6));
    }

    #[test]
    fn test_system_info_struct() {
        let info = MacSystemInfo {
            os_version: "14.0".into(),
            arch: "arm64".into(),
            model: Some("Mac14,2".into()),
            wifi_interface: Some("en0".into()),
            wifi_power: true,
            serial_drivers: vec!["CH34x".into()],
        };
        assert_eq!(info.arch, "arm64");
        assert!(info.wifi_power);
    }

    #[test]
    fn test_scan_result_struct() {
        let result = MacWifiScanResult {
            ssid: "MyNetwork".into(),
            bssid: "00:11:22:33:44:55".into(),
            rssi: -52,
            channel: 36,
            security: "WPA2".into(),
        };
        assert_eq!(result.rssi, -52);
    }
}
