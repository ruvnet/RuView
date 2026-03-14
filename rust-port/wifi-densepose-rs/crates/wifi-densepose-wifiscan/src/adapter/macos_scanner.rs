//! Adapter that scans WiFi BSSIDs on macOS by invoking the canonical Swift
//! CoreWLAN helper.
//!
//! The helper lives at `tools/macos-wifi-scan/main.swift` and is built by
//! `scripts/build-mac-wifi.sh` into the Rust workspace target tree. This
//! adapter resolves the helper path in the following order:
//!
//! 1. `RUVIEW_MAC_WIFI_HELPER`
//! 2. `target/tools/macos-wifi-scan/macos-wifi-scan`
//! 3. `macos-wifi-scan` on `PATH`

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::Deserialize;

use crate::domain::bssid::{BandType, BssidId, BssidObservation, RadioType};
use crate::error::WifiScanError;
use crate::port::WlanScanPort;

const HELPER_ENV_VAR: &str = "RUVIEW_MAC_WIFI_HELPER";
const HELPER_BINARY_NAME: &str = "macos-wifi-scan";
const REPO_LOCAL_HELPER_REL: &str = "target/tools/macos-wifi-scan/macos-wifi-scan";

#[derive(Debug, Deserialize)]
struct ProbeStatus {
    ok: bool,
    interface: String,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HelperObservation {
    timestamp: f64,
    interface: String,
    ssid: String,
    bssid: String,
    bssid_synthetic: bool,
    rssi: f64,
    noise: f64,
    channel: u8,
    band: String,
    tx_rate_mbps: f64,
    is_connected: bool,
}

/// Synchronous WiFi scanner that shells out to the Swift helper.
#[derive(Debug, Clone)]
pub struct MacosCoreWlanScanner {
    helper_path: PathBuf,
}

impl MacosCoreWlanScanner {
    /// Create a scanner using the standard helper resolution order.
    pub fn new() -> Self {
        Self {
            helper_path: resolve_helper_path_for(
                workspace_root().as_path(),
                std::env::var_os(HELPER_ENV_VAR),
            ),
        }
    }

    /// Create a scanner with an explicit helper path.
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            helper_path: path.into(),
        }
    }

    /// Return the resolved helper path.
    pub fn helper_path(&self) -> &Path {
        &self.helper_path
    }

    /// Verify that the helper can reach CoreWLAN and report interface readiness.
    pub fn probe_sync(&self) -> Result<(), WifiScanError> {
        let output = self.run_helper(["--probe"])?;
        let line = output
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .ok_or_else(|| {
                WifiScanError::ParseError("macOS helper probe returned no JSON status".to_string())
            })?;
        let status: ProbeStatus = serde_json::from_str(line).map_err(|err| {
            WifiScanError::ParseError(format!("probe output is not valid JSON: {err}"))
        })?;

        if status.ok {
            Ok(())
        } else {
            Err(WifiScanError::ScanFailed {
                reason: format!(
                    "probe failed on interface {}: {}",
                    status.interface,
                    status
                        .message
                        .unwrap_or_else(|| "helper reported Wi-Fi unavailable".to_string())
                ),
            })
        }
    }

    /// Run one visible-network scan.
    pub fn scan_sync(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        let output = self.run_helper(["--scan-once"])?;
        parse_macos_scan_output(&output)
    }

    /// Return the currently connected AP, if any.
    pub fn connected_sync(&self) -> Result<Option<BssidObservation>, WifiScanError> {
        match self.run_helper(["--connected"]) {
            Ok(output) => Ok(parse_macos_scan_output(&output)?.into_iter().next()),
            Err(WifiScanError::ScanFailed { reason }) if is_not_connected_reason(&reason) => {
                Ok(None)
            }
            Err(err) => Err(err),
        }
    }

    fn run_helper<const N: usize>(&self, args: [&str; N]) -> Result<String, WifiScanError> {
        let output = Command::new(&self.helper_path)
            .args(args)
            .output()
            .map_err(|err| {
                WifiScanError::ProcessError(format!(
                    "failed to run macOS Wi-Fi helper '{}': {err}. Build it with scripts/build-mac-wifi.sh or set {HELPER_ENV_VAR}.",
                    self.helper_path.display()
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WifiScanError::ScanFailed {
                reason: format!(
                    "macOS Wi-Fi helper '{}' exited {} while running {}: {}",
                    self.helper_path.display(),
                    output.status,
                    args.join(" "),
                    stderr.trim()
                ),
            });
        }

        String::from_utf8(output.stdout).map_err(|err| {
            WifiScanError::ParseError(format!("macOS Wi-Fi helper emitted invalid UTF-8: {err}"))
        })
    }
}

impl Default for MacosCoreWlanScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl WlanScanPort for MacosCoreWlanScanner {
    fn scan(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        self.scan_sync()
    }

    fn connected(&self) -> Result<Option<BssidObservation>, WifiScanError> {
        self.connected_sync()
    }
}

/// Parse the NDJSON output from the canonical macOS helper.
pub fn parse_macos_scan_output(output: &str) -> Result<Vec<BssidObservation>, WifiScanError> {
    let timestamp = Instant::now();
    let mut observations = Vec::new();

    for (line_index, line) in output.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let record: HelperObservation = serde_json::from_str(line).map_err(|err| {
            WifiScanError::ParseError(format!(
                "line {} is not valid helper JSON: {err}",
                line_index + 1
            ))
        })?;
        observations.push(helper_observation_to_domain(record, timestamp)?);
    }

    Ok(observations)
}

fn helper_observation_to_domain(
    record: HelperObservation,
    timestamp: Instant,
) -> Result<BssidObservation, WifiScanError> {
    if record.channel == 0 {
        return Err(WifiScanError::ParseError(
            "field `channel` must be greater than 0".to_string(),
        ));
    }

    let _ = (
        record.timestamp,
        record.interface.as_str(),
        record.bssid_synthetic,
        record.noise,
        record.tx_rate_mbps,
        record.is_connected,
    );

    let bssid = BssidId::parse(&record.bssid).map_err(|_| {
        WifiScanError::ParseError(format!(
            "field `bssid` is not a valid MAC address: {}",
            record.bssid
        ))
    })?;
    let band = parse_band_label(&record.band, record.channel)?;

    Ok(BssidObservation {
        bssid,
        rssi_dbm: record.rssi,
        signal_pct: ((record.rssi + 100.0) * 2.0).clamp(0.0, 100.0),
        channel: record.channel,
        band,
        radio_type: infer_radio_type(record.channel, band),
        ssid: record.ssid,
        timestamp,
    })
}

fn parse_band_label(label: &str, channel: u8) -> Result<BandType, WifiScanError> {
    let normalized = label.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "2.4ghz" | "2.4 ghz" | "2.4" => Ok(BandType::Band2_4GHz),
        "5ghz" | "5 ghz" | "5" => Ok(BandType::Band5GHz),
        "6ghz" | "6 ghz" | "6" => Ok(BandType::Band6GHz),
        "" => Ok(BandType::from_channel(channel)),
        _ => Err(WifiScanError::ParseError(format!(
            "field `band` must be one of 2.4GHz, 5GHz, or 6GHz; got '{label}'"
        ))),
    }
}

fn infer_radio_type(channel: u8, band: BandType) -> RadioType {
    match band {
        BandType::Band6GHz => RadioType::Ax,
        BandType::Band5GHz if channel >= 149 => RadioType::Ax,
        BandType::Band5GHz => RadioType::Ac,
        BandType::Band2_4GHz => RadioType::N,
    }
}

fn is_not_connected_reason(reason: &str) -> bool {
    let lower = reason.to_ascii_lowercase();
    lower.contains("not connected to an access point")
        || lower.contains("waiting for wi-fi association")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

fn resolve_helper_path_for(workspace_root: &Path, env_override: Option<OsString>) -> PathBuf {
    if let Some(env_override) = env_override.filter(|value| !value.is_empty()) {
        return PathBuf::from(env_override);
    }

    let repo_local = workspace_root.join(REPO_LOCAL_HELPER_REL);
    if repo_local.is_file() {
        return repo_local;
    }

    PathBuf::from(HELPER_BINARY_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ruview-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    const SAMPLE_OUTPUT: &str = r#"{"timestamp":1710000000.0,"interface":"en0","ssid":"Home","bssid":"aa:bb:cc:dd:ee:ff","bssid_synthetic":false,"rssi":-52.0,"noise":-90.0,"channel":36,"band":"5GHz","tx_rate_mbps":866.7,"is_connected":true}
{"timestamp":1710000001.0,"interface":"en0","ssid":"Guest","bssid":"11:22:33:44:55:66","bssid_synthetic":false,"rssi":-71.0,"noise":-92.0,"channel":6,"band":"2.4GHz","tx_rate_mbps":144.0,"is_connected":false}"#;

    #[test]
    fn parse_helper_output_uses_contract_fields() {
        let observations = parse_macos_scan_output(SAMPLE_OUTPUT).unwrap();
        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0].ssid, "Home");
        assert_eq!(observations[0].bssid.to_string(), "aa:bb:cc:dd:ee:ff");
        assert_eq!(observations[0].band, BandType::Band5GHz);
        assert_eq!(observations[0].radio_type, RadioType::Ac);
        assert_eq!(observations[1].band, BandType::Band2_4GHz);
        assert_eq!(observations[1].radio_type, RadioType::N);
    }

    #[test]
    fn parse_helper_output_reports_missing_fields() {
        let err = parse_macos_scan_output(
            r#"{"timestamp":1710000000.0,"interface":"en0","ssid":"Home","rssi":-52.0,"noise":-90.0,"channel":36,"band":"5GHz","tx_rate_mbps":866.7,"is_connected":true}"#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("line 1"));
        assert!(err.to_string().contains("bssid"));
    }

    #[test]
    fn probe_status_json_is_supported() {
        let status: ProbeStatus =
            serde_json::from_str(r#"{"ok":true,"interface":"en0","message":"ready"}"#).unwrap();
        assert!(status.ok);
        assert_eq!(status.interface, "en0");
    }

    #[test]
    fn helper_path_prefers_env_override() {
        let workspace = unique_temp_dir("env-path");
        let resolved =
            resolve_helper_path_for(&workspace, Some(OsString::from("/tmp/custom-helper")));
        assert_eq!(resolved, PathBuf::from("/tmp/custom-helper"));
        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn helper_path_uses_repo_local_binary_when_present() {
        let workspace = unique_temp_dir("repo-path");
        let helper = workspace.join(REPO_LOCAL_HELPER_REL);
        std::fs::create_dir_all(helper.parent().unwrap()).unwrap();
        std::fs::write(&helper, b"binary").unwrap();

        let resolved = resolve_helper_path_for(&workspace, None);
        assert_eq!(resolved, helper);

        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn helper_path_falls_back_to_path_binary() {
        let workspace = unique_temp_dir("path-fallback");
        let resolved = resolve_helper_path_for(&workspace, None);
        assert_eq!(resolved, PathBuf::from(HELPER_BINARY_NAME));
        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn compile_time_trait_check() {
        fn assert_port<T: WlanScanPort>() {}
        assert_port::<MacosCoreWlanScanner>();
    }

    #[test]
    fn connected_failure_reason_is_recognized() {
        assert!(is_not_connected_reason(
            "macOS Wi-Fi helper '/tmp/helper' exited 1 while running --connected: Wi-Fi interface en0 is not connected to an access point"
        ));
    }
}
