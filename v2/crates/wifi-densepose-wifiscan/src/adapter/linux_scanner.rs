//! Adapter that scans WiFi BSSIDs on Linux by invoking `iw dev <iface> scan`.
//!
//! This is the Linux counterpart to [`NetshBssidScanner`](super::NetshBssidScanner)
//! on Windows and [`MacosCoreWlanScanner`](super::MacosCoreWlanScanner) on macOS.
//!
//! # Design
//!
//! The adapter shells out to `iw dev <interface> scan` (or `iw dev <interface> scan dump`
//! to read cached results without triggering a new scan, which requires root).
//! The output is parsed into [`BssidObservation`] values using the same domain
//! types shared by all platform adapters.
//!
//! # Permissions
//!
//! - `iw dev <iface> scan` requires `CAP_NET_ADMIN` (typically root).
//! - `iw dev <iface> scan dump` reads cached results and may work without root
//!   on some distributions.
//!
//! # Platform
//!
//! Linux only. Gated behind `#[cfg(target_os = "linux")]` at the module level.

use std::process::Command;
use std::time::Instant;

use crate::domain::bssid::{BandType, BssidId, BssidObservation, RadioType};
use crate::error::WifiScanError;

// ---------------------------------------------------------------------------
// LinuxIwScanner
// ---------------------------------------------------------------------------

/// Synchronous WiFi scanner that shells out to `iw dev <interface> scan`.
///
/// Each call to [`scan_sync`](Self::scan_sync) spawns a subprocess, captures
/// stdout, and parses the BSS stanzas into [`BssidObservation`] values.
pub struct LinuxIwScanner {
    /// Wireless interface name (e.g. `"wlan0"`, `"wlp2s0"`).
    interface: String,
    /// If true, use `scan dump` (cached results) instead of triggering a new
    /// scan. This avoids the root requirement but may return stale data.
    use_dump: bool,
}

impl LinuxIwScanner {
    /// Create a scanner for the default interface `wlan0`.
    pub fn new() -> Self {
        Self {
            interface: "wlan0".to_owned(),
            use_dump: false,
        }
    }

    /// Create a scanner for a specific wireless interface.
    pub fn with_interface(iface: impl Into<String>) -> Self {
        Self {
            interface: iface.into(),
            use_dump: false,
        }
    }

    /// Use `scan dump` instead of `scan` to read cached results without root.
    pub fn use_cached(mut self) -> Self {
        self.use_dump = true;
        self
    }

    /// Run `iw dev <iface> scan` and parse the output synchronously.
    ///
    /// Returns one [`BssidObservation`] per BSS stanza in the output.
    pub fn scan_sync(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        let scan_cmd = if self.use_dump { "dump" } else { "scan" };

        let mut args = vec!["dev", &self.interface, "scan"];
        if self.use_dump {
            args.push(scan_cmd);
        }

        // iw uses "scan dump" not "scan scan dump"
        let args = if self.use_dump {
            vec!["dev", &self.interface, "scan", "dump"]
        } else {
            vec!["dev", &self.interface, "scan"]
        };

        let output = Command::new("iw")
            .args(&args)
            .output()
            .map_err(|e| {
                WifiScanError::ProcessError(format!(
                    "failed to run `iw {}`: {e}",
                    args.join(" ")
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WifiScanError::ScanFailed {
                reason: format!(
                    "iw exited with {}: {}",
                    output.status,
                    stderr.trim()
                ),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_iw_scan_output(&stdout)
    }
}

impl Default for LinuxIwScanner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Intermediate accumulator for fields within a single BSS stanza.
#[derive(Default)]
struct BssStanza {
    bssid: Option<String>,
    ssid: Option<String>,
    signal_dbm: Option<f64>,
    freq_mhz: Option<u32>,
    channel: Option<u8>,
}

impl BssStanza {
    /// Flush this stanza into a [`BssidObservation`], if we have enough data.
    fn flush(self, timestamp: Instant) -> Option<BssidObservation> {
        let bssid_str = self.bssid?;
        let bssid = BssidId::parse(&bssid_str).ok()?;
        let rssi_dbm = self.signal_dbm.unwrap_or(-90.0);

        // Determine channel from explicit field or frequency.
        let channel = self.channel.or_else(|| {
            self.freq_mhz.map(freq_to_channel)
        }).unwrap_or(0);

        let band = BandType::from_channel(channel);
        let radio_type = infer_radio_type_from_freq(self.freq_mhz.unwrap_or(0));
        let signal_pct = ((rssi_dbm + 100.0) * 2.0).clamp(0.0, 100.0);

        Some(BssidObservation {
            bssid,
            rssi_dbm,
            signal_pct,
            channel,
            band,
            radio_type,
            ssid: self.ssid.unwrap_or_default(),
            timestamp,
        })
    }
}

/// Parse the text output of `iw dev <iface> scan [dump]`.
///
/// The output consists of BSS stanzas, each starting with:
/// ```text
/// BSS aa:bb:cc:dd:ee:ff(on wlan0)
/// ```
/// followed by indented key-value lines.
pub fn parse_iw_scan_output(output: &str) -> Result<Vec<BssidObservation>, WifiScanError> {
    let now = Instant::now();
    let mut results = Vec::new();
    let mut current: Option<BssStanza> = None;

    for line in output.lines() {
        // New BSS stanza starts with "BSS " at column 0.
        if line.starts_with("BSS ") {
            // Flush previous stanza.
            if let Some(stanza) = current.take() {
                if let Some(obs) = stanza.flush(now) {
                    results.push(obs);
                }
            }

            // Parse BSSID from "BSS aa:bb:cc:dd:ee:ff(on wlan0)" or
            // "BSS aa:bb:cc:dd:ee:ff -- associated".
            let rest = &line[4..];
            let mac_end = rest.find(|c: char| !c.is_ascii_hexdigit() && c != ':')
                .unwrap_or(rest.len());
            let mac = &rest[..mac_end];

            if mac.len() == 17 {
                let mut stanza = BssStanza::default();
                stanza.bssid = Some(mac.to_lowercase());
                current = Some(stanza);
            }
            continue;
        }

        // Indented lines belong to the current stanza.
        let trimmed = line.trim();
        if let Some(ref mut stanza) = current {
            if let Some(rest) = trimmed.strip_prefix("SSID:") {
                stanza.ssid = Some(rest.trim().to_owned());
            } else if let Some(rest) = trimmed.strip_prefix("signal:") {
                // "signal: -52.00 dBm"
                stanza.signal_dbm = parse_signal_dbm(rest);
            } else if let Some(rest) = trimmed.strip_prefix("freq:") {
                // "freq: 5180"
                stanza.freq_mhz = rest.trim().parse().ok();
            } else if let Some(rest) = trimmed.strip_prefix("DS Parameter set: channel") {
                // "DS Parameter set: channel 6"
                stanza.channel = rest.trim().parse().ok();
            }
        }
    }

    // Flush the last stanza.
    if let Some(stanza) = current.take() {
        if let Some(obs) = stanza.flush(now) {
            results.push(obs);
        }
    }

    Ok(results)
}

/// Convert a frequency in MHz to an 802.11 channel number.
fn freq_to_channel(freq_mhz: u32) -> u8 {
    match freq_mhz {
        // 2.4 GHz: channels 1-14.
        2412..=2472 => ((freq_mhz - 2407) / 5) as u8,
        2484 => 14,
        // 5 GHz: channels 36-177.
        5170..=5885 => ((freq_mhz - 5000) / 5) as u8,
        // 6 GHz (Wi-Fi 6E).
        5955..=7115 => ((freq_mhz - 5950) / 5) as u8,
        _ => 0,
    }
}

/// Parse a signal strength string like "-52.00 dBm" into dBm.
fn parse_signal_dbm(s: &str) -> Option<f64> {
    let s = s.trim();
    // Take everything up to " dBm" or just parse the number.
    let num_part = s.split_whitespace().next()?;
    num_part.parse().ok()
}

/// Infer radio type from frequency (best effort).
fn infer_radio_type_from_freq(freq_mhz: u32) -> RadioType {
    match freq_mhz {
        5955..=7115 => RadioType::Ax, // 6 GHz → Wi-Fi 6E
        5170..=5885 => RadioType::Ac, // 5 GHz → likely 802.11ac
        _ => RadioType::N,            // 2.4 GHz → at least 802.11n
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Real-world `iw dev wlan0 scan` output (truncated to 3 BSSes).
    const SAMPLE_IW_OUTPUT: &str = "\
BSS aa:bb:cc:dd:ee:ff(on wlan0)
\tTSF: 123456789 usec
\tfreq: 5180
\tbeacon interval: 100 TUs
\tcapability: ESS Privacy (0x0011)
\tsignal: -52.00 dBm
\tSSID: HomeNetwork
\tDS Parameter set: channel 36
BSS 11:22:33:44:55:66(on wlan0)
\tfreq: 2437
\tsignal: -71.00 dBm
\tSSID: GuestWifi
\tDS Parameter set: channel 6
BSS de:ad:be:ef:ca:fe(on wlan0) -- associated
\tfreq: 5745
\tsignal: -45.00 dBm
\tSSID: OfficeNet
";

    #[test]
    fn parse_three_bss_stanzas() {
        let obs = parse_iw_scan_output(SAMPLE_IW_OUTPUT).unwrap();
        assert_eq!(obs.len(), 3);

        // First BSS.
        assert_eq!(obs[0].ssid, "HomeNetwork");
        assert_eq!(obs[0].bssid.to_string(), "aa:bb:cc:dd:ee:ff");
        assert!((obs[0].rssi_dbm - (-52.0)).abs() < f64::EPSILON);
        assert_eq!(obs[0].channel, 36);
        assert_eq!(obs[0].band, BandType::Band5GHz);

        // Second BSS: 2.4 GHz.
        assert_eq!(obs[1].ssid, "GuestWifi");
        assert_eq!(obs[1].channel, 6);
        assert_eq!(obs[1].band, BandType::Band2_4GHz);
        assert_eq!(obs[1].radio_type, RadioType::N);

        // Third BSS: "-- associated" suffix.
        assert_eq!(obs[2].ssid, "OfficeNet");
        assert_eq!(obs[2].bssid.to_string(), "de:ad:be:ef:ca:fe");
        assert!((obs[2].rssi_dbm - (-45.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn freq_to_channel_conversion() {
        assert_eq!(freq_to_channel(2412), 1);
        assert_eq!(freq_to_channel(2437), 6);
        assert_eq!(freq_to_channel(2462), 11);
        assert_eq!(freq_to_channel(2484), 14);
        assert_eq!(freq_to_channel(5180), 36);
        assert_eq!(freq_to_channel(5745), 149);
        assert_eq!(freq_to_channel(5955), 1); // 6 GHz channel 1
        assert_eq!(freq_to_channel(9999), 0); // Unknown
    }

    #[test]
    fn parse_signal_dbm_values() {
        assert!((parse_signal_dbm(" -52.00 dBm").unwrap() - (-52.0)).abs() < f64::EPSILON);
        assert!((parse_signal_dbm("-71.00 dBm").unwrap() - (-71.0)).abs() < f64::EPSILON);
        assert!((parse_signal_dbm("-45.00").unwrap() - (-45.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn empty_output() {
        let obs = parse_iw_scan_output("").unwrap();
        assert!(obs.is_empty());
    }

    #[test]
    fn missing_ssid_defaults_to_empty() {
        let output = "\
BSS 11:22:33:44:55:66(on wlan0)
\tfreq: 2437
\tsignal: -60.00 dBm
";
        let obs = parse_iw_scan_output(output).unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].ssid, "");
    }

    #[test]
    fn channel_from_freq_when_ds_param_missing() {
        let output = "\
BSS aa:bb:cc:dd:ee:ff(on wlan0)
\tfreq: 5180
\tsignal: -50.00 dBm
\tSSID: NoDS
";
        let obs = parse_iw_scan_output(output).unwrap();
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].channel, 36); // Derived from 5180 MHz.
    }
}
