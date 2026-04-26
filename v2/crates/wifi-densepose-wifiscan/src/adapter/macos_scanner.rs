//! Adapter that scans WiFi BSSIDs on macOS by invoking a compiled Swift
//! helper binary that uses Apple's CoreWLAN framework.
//!
//! This is the macOS counterpart to [`NetshBssidScanner`](super::NetshBssidScanner)
//! on Windows. It follows ADR-025 (ORCA — macOS CoreWLAN WiFi Sensing).
//!
//! # Design
//!
//! Apple removed the `airport` CLI in macOS Sonoma 14.4+ and CoreWLAN is a
//! Swift/Objective-C framework with no stable C ABI for Rust FFI. We therefore
//! shell out to a small Swift helper (`mac_wifi`) that outputs JSON lines:
//!
//! ```json
//! {"ssid":"MyNetwork","bssid":"aa:bb:cc:dd:ee:ff","rssi":-52,"noise":-90,"channel":36,"band":"5GHz"}
//! ```
//!
//! macOS Sonoma+ redacts real BSSID MACs to `00:00:00:00:00:00` unless the app
//! holds the `com.apple.wifi.scan` entitlement. When we detect a zeroed BSSID
//! we generate a deterministic synthetic MAC via `SHA-256(ssid:channel)[:6]`,
//! setting the locally-administered bit so it never collides with real OUI
//! allocations.
//!
//! # Platform
//!
//! macOS only. Gated behind `#[cfg(target_os = "macos")]` at the module level.

use std::process::Command;
use std::time::Instant;

use crate::domain::bssid::{BandType, BssidId, BssidObservation, RadioType};
use crate::error::WifiScanError;

// ---------------------------------------------------------------------------
// MacosCoreWlanScanner
// ---------------------------------------------------------------------------

/// Synchronous WiFi scanner that shells out to the `mac_wifi` Swift helper.
///
/// The helper binary must be compiled from `v1/src/sensing/mac_wifi.swift` and
/// placed on `$PATH` or at a known location. The scanner invokes it with a
/// `--scan-once` flag (single-shot mode) and parses the JSON output.
///
/// If the helper is not found, [`scan_sync`](Self::scan_sync) returns a
/// [`WifiScanError::ProcessError`].
pub struct MacosCoreWlanScanner {
    /// Path to the `mac_wifi` helper binary. Defaults to `"mac_wifi"` (on PATH).
    helper_path: String,
}

impl MacosCoreWlanScanner {
    /// Create a scanner that looks for `mac_wifi` on `$PATH`.
    pub fn new() -> Self {
        Self {
            helper_path: "mac_wifi".to_owned(),
        }
    }

    /// Create a scanner with an explicit path to the Swift helper binary.
    pub fn with_path(path: impl Into<String>) -> Self {
        Self {
            helper_path: path.into(),
        }
    }

    /// Run the Swift helper and parse the output synchronously.
    ///
    /// Returns one [`BssidObservation`] per BSSID seen in the scan.
    pub fn scan_sync(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        let output = Command::new(&self.helper_path)
            .arg("--scan-once")
            .output()
            .map_err(|e| {
                WifiScanError::ProcessError(format!(
                    "failed to run mac_wifi helper ({}): {e}",
                    self.helper_path
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WifiScanError::ScanFailed {
                reason: format!(
                    "mac_wifi exited with {}: {}",
                    output.status,
                    stderr.trim()
                ),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_macos_scan_output(&stdout)
    }
}

impl Default for MacosCoreWlanScanner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Parse the JSON-lines output from the `mac_wifi` Swift helper.
///
/// Each line is expected to be a JSON object with the fields:
/// `ssid`, `bssid`, `rssi`, `noise`, `channel`, `band`.
///
/// Lines that fail to parse are silently skipped (the helper may emit
/// status messages on stdout).
pub fn parse_macos_scan_output(output: &str) -> Result<Vec<BssidObservation>, WifiScanError> {
    let now = Instant::now();
    let mut results = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        if let Some(obs) = parse_json_line(line, now) {
            results.push(obs);
        }
    }

    Ok(results)
}

/// Parse a single JSON line into a [`BssidObservation`].
///
/// Uses a lightweight manual parser to avoid pulling in `serde_json` as a
/// hard dependency. The JSON structure is simple and well-known.
fn parse_json_line(line: &str, timestamp: Instant) -> Option<BssidObservation> {
    let ssid = extract_string_field(line, "ssid")?;
    let bssid_str = extract_string_field(line, "bssid")?;
    let rssi = extract_number_field(line, "rssi")?;
    let channel_f = extract_number_field(line, "channel")?;
    let channel = channel_f as u8;

    // Resolve BSSID: use real MAC if available, otherwise generate synthetic.
    let bssid = resolve_bssid(&bssid_str, &ssid, channel)?;

    let band = BandType::from_channel(channel);

    // macOS CoreWLAN doesn't report radio type directly; infer from band/channel.
    let radio_type = infer_radio_type(channel);

    // Convert RSSI to signal percentage using the standard mapping.
    let signal_pct = ((rssi + 100.0) * 2.0).clamp(0.0, 100.0);

    Some(BssidObservation {
        bssid,
        rssi_dbm: rssi,
        signal_pct,
        channel,
        band,
        radio_type,
        ssid,
        timestamp,
    })
}

/// Resolve a BSSID string to a [`BssidId`].
///
/// If the MAC is all-zeros (macOS redaction), generate a synthetic
/// locally-administered MAC from `SHA-256(ssid:channel)`.
fn resolve_bssid(bssid_str: &str, ssid: &str, channel: u8) -> Option<BssidId> {
    // Try parsing the real BSSID first.
    if let Ok(id) = BssidId::parse(bssid_str) {
        // Check for the all-zeros redacted BSSID.
        if id.0 != [0, 0, 0, 0, 0, 0] {
            return Some(id);
        }
    }

    // Generate synthetic BSSID: SHA-256(ssid:channel), take first 6 bytes,
    // set locally-administered + unicast bits (byte 0: bit 1 set, bit 0 clear).
    Some(synthetic_bssid(ssid, channel))
}

/// Generate a deterministic synthetic BSSID from SSID and channel.
///
/// Uses a simple hash (FNV-1a-inspired) to avoid pulling in `sha2` crate.
/// The locally-administered bit is set so these never collide with real OUI MACs.
fn synthetic_bssid(ssid: &str, channel: u8) -> BssidId {
    // Simple but deterministic hash — FNV-1a 64-bit.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in ssid.as_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash ^= u64::from(channel);
    hash = hash.wrapping_mul(0x0100_0000_01b3);

    let bytes = hash.to_le_bytes();
    let mut mac = [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]];

    // Set locally-administered bit (bit 1 of byte 0) and clear multicast (bit 0).
    mac[0] = (mac[0] | 0x02) & 0xFE;

    BssidId(mac)
}

/// Infer radio type from channel number (best effort on macOS).
fn infer_radio_type(channel: u8) -> RadioType {
    match channel {
        // 5 GHz channels → likely 802.11ac or newer
        36..=177 => RadioType::Ac,
        // 2.4 GHz → at least 802.11n
        _ => RadioType::N,
    }
}

// ---------------------------------------------------------------------------
// Lightweight JSON field extractors
// ---------------------------------------------------------------------------

/// Extract a string field value from a JSON object string.
///
/// Looks for `"key":"value"` or `"key": "value"` patterns.
fn extract_string_field(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let key_pos = json.find(&pattern)?;
    let after_key = &json[key_pos + pattern.len()..];

    // Skip optional whitespace and the colon.
    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_colon = after_colon.trim_start();

    // Expect opening quote.
    let after_quote = after_colon.strip_prefix('"')?;

    // Find closing quote (handle escaped quotes).
    let mut end = 0;
    let bytes = after_quote.as_bytes();
    while end < bytes.len() {
        if bytes[end] == b'"' && (end == 0 || bytes[end - 1] != b'\\') {
            break;
        }
        end += 1;
    }

    Some(after_quote[..end].to_owned())
}

/// Extract a numeric field value from a JSON object string.
///
/// Looks for `"key": <number>` patterns.
fn extract_number_field(json: &str, key: &str) -> Option<f64> {
    let pattern = format!("\"{}\"", key);
    let key_pos = json.find(&pattern)?;
    let after_key = &json[key_pos + pattern.len()..];

    let after_colon = after_key.trim_start().strip_prefix(':')?;
    let after_colon = after_colon.trim_start();

    // Collect digits, sign, and decimal point.
    let num_str: String = after_colon
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '.' || *c == '+' || *c == 'e' || *c == 'E')
        .collect();

    num_str.parse().ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OUTPUT: &str = r#"
{"ssid":"HomeNetwork","bssid":"aa:bb:cc:dd:ee:ff","rssi":-52,"noise":-90,"channel":36,"band":"5GHz"}
{"ssid":"GuestWifi","bssid":"11:22:33:44:55:66","rssi":-71,"noise":-92,"channel":6,"band":"2.4GHz"}
{"ssid":"Redacted","bssid":"00:00:00:00:00:00","rssi":-65,"noise":-88,"channel":149,"band":"5GHz"}
"#;

    #[test]
    fn parse_valid_output() {
        let obs = parse_macos_scan_output(SAMPLE_OUTPUT).unwrap();
        assert_eq!(obs.len(), 3);

        // First entry: real BSSID.
        assert_eq!(obs[0].ssid, "HomeNetwork");
        assert_eq!(obs[0].bssid.to_string(), "aa:bb:cc:dd:ee:ff");
        assert!((obs[0].rssi_dbm - (-52.0)).abs() < f64::EPSILON);
        assert_eq!(obs[0].channel, 36);
        assert_eq!(obs[0].band, BandType::Band5GHz);

        // Second entry: 2.4 GHz.
        assert_eq!(obs[1].ssid, "GuestWifi");
        assert_eq!(obs[1].channel, 6);
        assert_eq!(obs[1].band, BandType::Band2_4GHz);
        assert_eq!(obs[1].radio_type, RadioType::N);

        // Third entry: redacted BSSID → synthetic MAC.
        assert_eq!(obs[2].ssid, "Redacted");
        // Should NOT be all-zeros.
        assert_ne!(obs[2].bssid.0, [0, 0, 0, 0, 0, 0]);
        // Should have locally-administered bit set.
        assert_eq!(obs[2].bssid.0[0] & 0x02, 0x02);
        // Should have unicast bit (multicast cleared).
        assert_eq!(obs[2].bssid.0[0] & 0x01, 0x00);
    }

    #[test]
    fn synthetic_bssid_is_deterministic() {
        let a = synthetic_bssid("TestNet", 36);
        let b = synthetic_bssid("TestNet", 36);
        assert_eq!(a, b);

        // Different SSID or channel → different MAC.
        let c = synthetic_bssid("OtherNet", 36);
        assert_ne!(a, c);

        let d = synthetic_bssid("TestNet", 6);
        assert_ne!(a, d);
    }

    #[test]
    fn parse_empty_and_junk_lines() {
        let output = "\n  \nnot json\n{broken json\n";
        let obs = parse_macos_scan_output(output).unwrap();
        assert!(obs.is_empty());
    }

    #[test]
    fn extract_string_field_basic() {
        let json = r#"{"ssid":"MyNet","bssid":"aa:bb:cc:dd:ee:ff"}"#;
        assert_eq!(extract_string_field(json, "ssid").unwrap(), "MyNet");
        assert_eq!(
            extract_string_field(json, "bssid").unwrap(),
            "aa:bb:cc:dd:ee:ff"
        );
        assert!(extract_string_field(json, "missing").is_none());
    }

    #[test]
    fn extract_number_field_basic() {
        let json = r#"{"rssi":-52,"channel":36}"#;
        assert!((extract_number_field(json, "rssi").unwrap() - (-52.0)).abs() < f64::EPSILON);
        assert!((extract_number_field(json, "channel").unwrap() - 36.0).abs() < f64::EPSILON);
    }

    #[test]
    fn signal_pct_clamping() {
        // RSSI -50 → pct = (-50+100)*2 = 100
        let json = r#"{"ssid":"Test","bssid":"aa:bb:cc:dd:ee:ff","rssi":-50,"channel":1}"#;
        let obs = parse_json_line(json, Instant::now()).unwrap();
        assert!((obs.signal_pct - 100.0).abs() < f64::EPSILON);

        // RSSI -100 → pct = 0
        let json = r#"{"ssid":"Test","bssid":"aa:bb:cc:dd:ee:ff","rssi":-100,"channel":1}"#;
        let obs = parse_json_line(json, Instant::now()).unwrap();
        assert!((obs.signal_pct - 0.0).abs() < f64::EPSILON);
    }
}
