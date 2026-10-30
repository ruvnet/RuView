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

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::domain::bssid::{BandType, BssidId, BssidObservation, RadioType};
use crate::error::WifiScanError;

// ---------------------------------------------------------------------------
// MacosCoreWlanScanner
// ---------------------------------------------------------------------------

/// How the Swift helper is launched.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Launch {
    /// Plain CLI helper (on `$PATH` or an explicit path). macOS treats the
    /// terminal as the responsible app, so SSID/BSSID come back redacted.
    Helper(String),
    /// `MacWifi.app` launched through LaunchServices (`open`), so the bundle is
    /// its own responsible process and can hold a Location Services grant
    /// (ADR-025 Amendment 2). Gives the real SSID/BSSID once authorized.
    AppBundle(PathBuf),
}

/// Synchronous WiFi scanner that shells out to the `mac_wifi` Swift helper.
///
/// Preferred: the `MacWifi.app` bundle built by `tools/mac-wifi-helper/build.sh`,
/// found via `RUVIEW_MAC_WIFI_APP`, `~/Applications/MacWifi.app` or
/// `/Applications/MacWifi.app`. Fallback: the CLI helper compiled from
/// `archive/v1/src/sensing/mac_wifi.swift` on `$PATH` (redacted connected link
/// only). The helper is invoked with `--scan-once` and its JSON is parsed.
///
/// If the helper is not found, [`scan_sync`](Self::scan_sync) returns a
/// [`WifiScanError::ProcessError`].
pub struct MacosCoreWlanScanner {
    launch: Launch,
}

/// Environment variable naming an explicit `MacWifi.app` bundle path.
/// Set it to an empty string to disable bundle discovery.
pub const MAC_WIFI_APP_ENV: &str = "RUVIEW_MAC_WIFI_APP";

/// Locate an installed `MacWifi.app` bundle, if any.
fn find_app_bundle() -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = match std::env::var(MAC_WIFI_APP_ENV) {
        Ok(explicit) if explicit.trim().is_empty() => return None,
        Ok(explicit) => vec![PathBuf::from(explicit)],
        Err(_) => {
            let mut v = Vec::new();
            if let Some(home) = std::env::var_os("HOME") {
                v.push(PathBuf::from(home).join("Applications/MacWifi.app"));
            }
            v.push(PathBuf::from("/Applications/MacWifi.app"));
            v
        }
    };
    candidates
        .into_iter()
        .find(|app| app.join("Contents/MacOS/mac_wifi").is_file())
}

impl MacosCoreWlanScanner {
    /// Use an installed `MacWifi.app` if present, else `mac_wifi` on `$PATH`.
    pub fn new() -> Self {
        let launch = match find_app_bundle() {
            Some(app) => Launch::AppBundle(app),
            None => Launch::Helper("mac_wifi".to_owned()),
        };
        Self { launch }
    }

    /// Create a scanner with an explicit path to the CLI helper binary.
    pub fn with_path(path: impl Into<String>) -> Self {
        Self {
            launch: Launch::Helper(path.into()),
        }
    }

    /// Create a scanner that launches an explicit `MacWifi.app` bundle.
    pub fn with_app_bundle(path: impl Into<PathBuf>) -> Self {
        Self {
            launch: Launch::AppBundle(path.into()),
        }
    }

    /// True when scans go through the `MacWifi.app` bundle.
    pub fn uses_app_bundle(&self) -> bool {
        matches!(self.launch, Launch::AppBundle(_))
    }

    /// Run the Swift helper and parse the output synchronously.
    ///
    /// Returns one [`BssidObservation`] for the connected link.
    /// Helpers that fail to exit within five seconds are killed and reaped.
    pub fn scan_sync(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        match &self.launch {
            Launch::Helper(path) => scan_with_helper(path),
            Launch::AppBundle(app) => scan_with_app_bundle(app),
        }
    }
}

/// Wait for `child` with the five-second bound shared by both launch modes.
/// Older helpers ignore --scan-once and stream forever; bound the wait so an
/// outdated installation cannot hang capture or auto-detect.
fn wait_bounded(child: &mut Child) -> Result<(), WifiScanError> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            status => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(WifiScanError::ProcessError(match status {
                    Err(e) => format!("failed to wait for mac_wifi: {e}"),
                    _ => "mac_wifi --scan-once timed out; rebuild the Swift helper".into(),
                }));
            }
        }
    }
}

fn scan_with_helper(path: &str) -> Result<Vec<BssidObservation>, WifiScanError> {
    let mut child = Command::new(path)
        .arg("--scan-once")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            WifiScanError::ProcessError(format!("failed to run mac_wifi helper ({path}): {e}"))
        })?;
    wait_bounded(&mut child)?;
    let output = child.wait_with_output().map_err(|e| {
        WifiScanError::ProcessError(format!("failed to read mac_wifi output: {e}"))
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WifiScanError::ScanFailed {
            reason: format!("mac_wifi exited with {}: {}", output.status, stderr.trim()),
        });
    }
    parse_macos_scan_output(&String::from_utf8_lossy(&output.stdout))
}

/// Launch `MacWifi.app` through LaunchServices so it is its own responsible
/// process. `open --stdout` appends, so every scan gets fresh temp files.
fn scan_with_app_bundle(app: &Path) -> Result<Vec<BssidObservation>, WifiScanError> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("ruview-macwifi-{}-{n}", std::process::id()));
    let out = base.with_extension("json");
    let err = base.with_extension("err");
    for f in [&out, &err] {
        let _ = std::fs::remove_file(f);
    }

    let result = (|| {
        let mut child = Command::new("/usr/bin/open")
            .args(["-W", "-g", "-n", "--stdout"])
            .arg(&out)
            .arg("--stderr")
            .arg(&err)
            .arg(app)
            .args(["--args", "--scan-once"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                WifiScanError::ProcessError(format!(
                    "failed to run mac_wifi helper ({}): {e}",
                    app.display()
                ))
            })?;
        wait_bounded(&mut child)?;
        let status = child.wait().map_err(|e| {
            WifiScanError::ProcessError(format!("failed to wait for open: {e}"))
        })?;
        let stdout = std::fs::read_to_string(&out).unwrap_or_default();
        if !status.success() || stdout.trim().is_empty() {
            let stderr = std::fs::read_to_string(&err).unwrap_or_default();
            return Err(WifiScanError::ScanFailed {
                reason: format!(
                    "MacWifi.app ({}) exited with {status}: {}",
                    app.display(),
                    stderr.trim()
                ),
            });
        }
        parse_macos_scan_output(&stdout)
    })();

    for f in [&out, &err] {
        let _ = std::fs::remove_file(f);
    }
    result
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

    // `--scan-once` reports only the connected link and marks it so.
    let connected = extract_bool_field(line, "connected").unwrap_or(false);

    // Resolve BSSID: use real MAC if available, otherwise generate synthetic.
    let bssid = resolve_bssid(&bssid_str, &ssid, channel, connected)?;

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
/// locally-administered MAC from the SSID and channel. When both are redacted
/// (no Location Services permission), keep the observation only if the helper
/// marked it as the connected link: there is exactly one, so it cannot collide
/// with another network. Otherwise abstain.
fn resolve_bssid(bssid_str: &str, ssid: &str, channel: u8, connected: bool) -> Option<BssidId> {
    // Try parsing the real BSSID first.
    if let Ok(id) = BssidId::parse(bssid_str) {
        // Check for the all-zeros redacted BSSID.
        if id.0 != [0, 0, 0, 0, 0, 0] {
            return Some(id);
        }
    }

    // Without either identity, unrelated networks on the same channel would
    // collapse to one synthetic BSSID. Do not emit an observation in that case.
    if ssid.trim().is_empty() {
        return connected.then(|| synthetic_bssid(REDACTED_CONNECTED_LINK, channel));
    }

    // Generate synthetic BSSID from SSID and channel, take first 6 bytes,
    // set locally-administered + unicast bits (byte 0: bit 1 set, bit 0 clear).
    Some(synthetic_bssid(ssid, channel))
}

/// Hash key for the connected link when macOS redacts both SSID and BSSID.
/// The NUL prefix keeps it from matching any real SSID.
const REDACTED_CONNECTED_LINK: &str = "\u{0}redacted-connected-link";

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

/// Extract a boolean field value (`"key": true|false`) from a JSON object string.
fn extract_bool_field(json: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{key}\"");
    let key_pos = json.find(&pattern)?;
    let after = json[key_pos + pattern.len()..].trim_start().strip_prefix(':')?.trim_start();
    if after.starts_with("true") {
        Some(true)
    } else if after.starts_with("false") {
        Some(false)
    } else {
        None
    }
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
        .take_while(|c| {
            c.is_ascii_digit() || *c == '-' || *c == '.' || *c == '+' || *c == 'e' || *c == 'E'
        })
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
    fn helper_process_errors_are_reported() {
        assert!(matches!(
            MacosCoreWlanScanner::with_path("/usr/bin/false").scan_sync(),
            Err(WifiScanError::ScanFailed { .. })
        ));
        assert!(matches!(
            MacosCoreWlanScanner::with_path("/dev/null/mac_wifi").scan_sync(),
            Err(WifiScanError::ProcessError(_))
        ));
    }

    #[test]
    fn app_bundle_discovery_honours_env() {
        // Empty value disables discovery; a path without the executable is skipped.
        let dir = std::env::temp_dir().join(format!("macwifi-app-{}", std::process::id()));
        let exe = dir.join("MacWifi.app/Contents/MacOS");
        std::fs::create_dir_all(&exe).unwrap();
        let app = dir.join("MacWifi.app");

        std::env::set_var(MAC_WIFI_APP_ENV, "");
        assert_eq!(find_app_bundle(), None);
        assert!(!MacosCoreWlanScanner::new().uses_app_bundle());

        std::env::set_var(MAC_WIFI_APP_ENV, &app);
        assert_eq!(find_app_bundle(), None, "bundle without executable is ignored");

        std::fs::write(exe.join("mac_wifi"), b"").unwrap();
        assert_eq!(find_app_bundle(), Some(app.clone()));
        assert!(MacosCoreWlanScanner::new().uses_app_bundle());

        std::env::remove_var(MAC_WIFI_APP_ENV);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_app_bundle_is_a_scan_error() {
        let r = MacosCoreWlanScanner::with_app_bundle("/nonexistent/MacWifi.app").scan_sync();
        assert!(r.is_err());
    }

    #[test]
    fn legacy_helper_is_timed_out() {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("mac-wifi-timeout-{}", std::process::id()));
        // exec preserves the helper PID, so killing it cannot orphan sleep.
        std::fs::write(&path, "#!/bin/sh\nexec /bin/sleep 30\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let start = Instant::now();
        let result = MacosCoreWlanScanner::with_path(path.to_string_lossy()).scan_sync();
        std::fs::remove_file(path).unwrap();
        assert!(matches!(result, Err(WifiScanError::ProcessError(ref e)) if e.contains("timed out")));
        assert!(start.elapsed() < Duration::from_secs(15));
    }

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
    fn unavailable_bssid_without_ssid_abstains() {
        for bssid in ["00:00:00:00:00:00", "", "invalid"] {
            for ssid in ["", "   "] {
                let output = format!(
                    r#"{{"ssid":"{ssid}","bssid":"{bssid}","rssi":-65,"noise":-88,"channel":36}}"#
                );
                assert!(parse_macos_scan_output(&output).unwrap().is_empty());
            }
        }
    }

    // macOS without Location Services: the helper's connected-link sample has
    // a blank SSID and zero BSSID but real rssi/channel. Keep it (#H15).
    #[test]
    fn redacted_connected_link_is_kept() {
        let output = r#"{"bssid":"00:00:00:00:00:00","channel":40,"connected":true,"noise":-94,"rssi":-57,"ssid":"","timestamp":1790888839.88,"tx_rate":576}"#;
        let obs = parse_macos_scan_output(output).unwrap();
        assert_eq!(obs.len(), 1);
        assert!((obs[0].rssi_dbm - (-57.0)).abs() < f64::EPSILON);
        assert_eq!(obs[0].channel, 40);
        assert!(obs[0].ssid.is_empty(), "never invent an SSID");
        assert_ne!(obs[0].bssid.0, [0; 6]);
        assert_eq!(obs[0].bssid.0[0] & 0x03, 0x02, "locally administered unicast");
        // Stable across samples, distinct from any real SSID's synthetic id.
        let again = parse_macos_scan_output(output).unwrap();
        assert_eq!(obs[0].bssid, again[0].bssid);
        assert_ne!(obs[0].bssid, synthetic_bssid("", 40));
    }

    #[test]
    fn redacted_line_not_marked_connected_still_abstains() {
        for connected in ["", r#","connected":false"#] {
            let output = format!(
                r#"{{"ssid":"","bssid":"00:00:00:00:00:00","rssi":-65,"channel":36{connected}}}"#
            );
            assert!(parse_macos_scan_output(&output).unwrap().is_empty());
        }
    }

    #[test]
    fn extract_bool_field_basic() {
        assert_eq!(extract_bool_field(r#"{"connected":true}"#, "connected"), Some(true));
        assert_eq!(extract_bool_field(r#"{"connected" : false}"#, "connected"), Some(false));
        assert_eq!(extract_bool_field(r#"{"connected":1}"#, "connected"), None);
        assert_eq!(extract_bool_field(r#"{"x":true}"#, "connected"), None);
    }

    #[test]
    fn real_bssid_without_ssid_is_preserved() {
        let output = r#"{"ssid":"","bssid":"aa:bb:cc:dd:ee:ff","rssi":-65,"noise":-88,"channel":36}"#;
        let obs = parse_macos_scan_output(output).unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].bssid.to_string(), "aa:bb:cc:dd:ee:ff");
        assert!(obs[0].ssid.is_empty());
        assert_eq!(obs[0].rssi_dbm, -65.0);
        assert_eq!(obs[0].channel, 36);
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
