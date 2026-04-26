//! Adapter that scans WiFi BSSIDs by invoking `netsh wlan show networks mode=bssid`
//! and parsing the textual output.
//!
//! This is the Tier 1 scanner from ADR-022. It works on any Windows machine
//! with a WLAN adapter but is limited to whatever the driver chooses to cache
//! (typically one scan result per ~10 s).
//!
//! # Design notes
//!
//! This adapter is intentionally synchronous. It does **not** implement the
//! async [`WlanScanPort`](crate::port::WlanScanPort) trait so that callers
//! who only need blocking scans can avoid pulling in an async runtime.
//! Wrapping [`scan_sync`](NetshBssidScanner::scan_sync) in a
//! `tokio::task::spawn_blocking` call is trivial if an async interface is
//! desired.

use std::process::Command;
use std::time::Instant;

use crate::domain::bssid::{BandType, BssidId, BssidObservation, RadioType};
use crate::error::WifiScanError;

// ---------------------------------------------------------------------------
// NetshBssidScanner
// ---------------------------------------------------------------------------

/// Synchronous WiFi scanner that shells out to `netsh wlan show networks mode=bssid`.
///
/// Each call to [`scan_sync`](Self::scan_sync) spawns a new subprocess,
/// captures its stdout, and parses the result into a vector of
/// [`BssidObservation`] values.
///
/// # Platform
///
/// Windows only. On other platforms the subprocess will fail with a
/// [`WifiScanError::ProcessError`].
pub struct NetshBssidScanner;

impl NetshBssidScanner {
    /// Create a new scanner instance.
    pub fn new() -> Self {
        Self
    }

    /// Run `netsh wlan show networks mode=bssid` and parse the output
    /// synchronously.
    ///
    /// Returns one [`BssidObservation`] per BSSID seen in the output.
    pub fn scan_sync(&self) -> Result<Vec<BssidObservation>, WifiScanError> {
        let output = Command::new("netsh")
            .args(["wlan", "show", "networks", "mode=bssid"])
            .output()
            .map_err(|e| WifiScanError::ProcessError(format!("failed to run netsh: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WifiScanError::ScanFailed {
                reason: format!("netsh exited with {}: {}", output.status, stderr.trim()),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_netsh_output(&stdout)
    }
}

impl Default for NetshBssidScanner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Intermediate accumulator for fields within a single BSSID sub-block.
///
/// All fields are optional because individual lines may be missing or
/// malformed. When the block is flushed, missing fields fall back to
/// sensible defaults.
#[derive(Default)]
struct BssidBlock {
    mac: Option<BssidId>,
    signal_pct: Option<f64>,
    radio_type: Option<RadioType>,
    band: Option<BandType>,
    channel: Option<u8>,
}

impl BssidBlock {
    /// Convert the accumulated block into a [`BssidObservation`].
    ///
    /// Returns `None` when the mandatory MAC address is missing (e.g.
    /// because the BSSID line contained an unparseable MAC).
    fn into_observation(self, ssid: &str, timestamp: Instant) -> Option<BssidObservation> {
        let bssid = self.mac?;
        let signal_pct = self.signal_pct.unwrap_or(0.0);
        let rssi_dbm = BssidObservation::pct_to_dbm(signal_pct);
        let channel = self.channel.unwrap_or(0);
        let band = self
            .band
            .unwrap_or_else(|| BandType::from_channel(channel));
        let radio_type = self.radio_type.unwrap_or(RadioType::N);

        Some(BssidObservation {
            bssid,
            rssi_dbm,
            signal_pct,
            channel,
            band,
            radio_type,
            ssid: ssid.to_owned(),
            timestamp,
        })
    }
}

/// Parse the text output of `netsh wlan show networks mode=bssid` into a
/// vector of [`BssidObservation`] values.
///
/// The parser walks line-by-line, tracking the current SSID context and
/// accumulating fields for each BSSID sub-block. When a new SSID header,
/// a new BSSID header, or the end of input is reached the accumulated
/// block is flushed as a complete observation.
///
/// Lines that do not match any expected pattern are silently skipped so
/// that headers such as `"Interface name : Wi-Fi"` or localised messages
/// never cause an error.
///
/// # Example
///
/// ```text
/// SSID 1 : MyNetwork
///     Network type            : Infrastructure
///     Authentication          : WPA2-Personal
///     Encryption              : CCMP
///     BSSID 1                 : aa:bb:cc:dd:ee:ff
///          Signal             : 84%
///          Radio type         : 802.11ax
///          Band               : 5 GHz
///          Channel            : 36
/// ```
pub fn parse_netsh_output(output: &str) -> Result<Vec<BssidObservation>, WifiScanError> {
    let timestamp = Instant::now();
    let mut results: Vec<BssidObservation> = Vec::new();

    let mut current_ssid = String::new();
    let mut current_block: Option<BssidBlock> = None;

    for line in output.lines() {
        let trimmed = line.trim();

        // -- SSID header: "SSID 1 : MyNetwork" --------------------------------
        if let Some(ssid_value) = try_parse_ssid_line(trimmed) {
            // Flush the previous BSSID block before switching SSIDs.
            if let Some(block) = current_block.take() {
                if let Some(obs) = block.into_observation(&current_ssid, timestamp) {
                    results.push(obs);
                }
            }
            current_ssid = ssid_value;
            continue;
        }

        // -- BSSID header: "BSSID 1 : d8:32:14:b0:a0:3e" ---------------------
        if let Some(mac) = try_parse_bssid_line(trimmed) {
            // Flush the previous BSSID block before starting a new one.
            if let Some(block) = current_block.take() {
                if let Some(obs) = block.into_observation(&current_ssid, timestamp) {
                    results.push(obs);
                }
            }
            current_block = Some(BssidBlock {
                mac: Some(mac),
                ..Default::default()
            });
            continue;
        }

        // If we see a "BSSID" prefix but the MAC was unparseable, we still
        // want to start a new block (with mac = None) so subsequent field
        // lines are consumed rather than attributed to the previous block.
        if trimmed.to_ascii_uppercase().starts_with("BSSID") && split_kv(trimmed).is_some() {
            if let Some(block) = current_block.take() {
                if let Some(obs) = block.into_observation(&current_ssid, timestamp) {
                    results.push(obs);
                }
            }
            current_block = Some(BssidBlock::default());
            continue;
        }

        // The remaining fields are only meaningful inside a BSSID block.
        let Some(block) = current_block.as_mut() else {
            continue;
        };

        // -- Signal: "Signal             : 84%" --------------------------------
        if let Some(pct) = try_parse_signal_line(trimmed) {
            block.signal_pct = Some(pct);
            continue;
        }

        // -- Radio type: "Radio type         : 802.11ax" -----------------------
        if let Some(radio) = try_parse_radio_type_line(trimmed) {
            block.radio_type = Some(radio);
            continue;
        }

        // -- Band: "Band               : 5 GHz" --------------------------------
        if let Some(band) = try_parse_band_line(trimmed) {
            block.band = Some(band);
            continue;
        }

        // -- Channel: "Channel            : 48" --------------------------------
        if let Some(ch) = try_parse_channel_line(trimmed) {
            block.channel = Some(ch);
        }

        // Unknown lines are silently ignored (graceful handling of
        // malformed or localised output).
    }

    // Flush the final BSSID block.
    if let Some(block) = current_block.take() {
        if let Some(obs) = block.into_observation(&current_ssid, timestamp) {
            results.push(obs);
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Individual line parsers
// ---------------------------------------------------------------------------

/// Parse an SSID header line (`"SSID <N> : <name>"`).
///
/// The SSID name may be empty for hidden networks. Returns `None` when
/// the line does not match.
fn try_parse_ssid_line(line: &str) -> Option<String> {
    let upper = line.to_ascii_uppercase();
    // Must start with "SSID" but must NOT start with "BSSID".
    if !upper.starts_with("SSID") || upper.starts_with("BSSID") {
        return None;
    }
    let (_key, value) = split_kv(line)?;
    Some(value.to_owned())
}

/// Parse a BSSID header line and extract the MAC address.
///
/// Accepts `"BSSID <N>                 : aa:bb:cc:dd:ee:ff"`.
/// Returns `None` if the line is not a BSSID header or the MAC is
/// malformed.
fn try_parse_bssid_line(line: &str) -> Option<BssidId> {
    let upper = line.to_ascii_uppercase();
    if !upper.starts_with("BSSID") {
        return None;
    }
    let (_key, mac_str) = split_kv(line)?;
    BssidId::parse(mac_str.trim()).ok()
}

/// Parse a Signal line and return the percentage value.
///
/// Accepts `"Signal             : 84%"` and returns `84.0`.
/// Also handles values without the trailing `%` sign.
fn try_parse_signal_line(line: &str) -> Option<f64> {
    let upper = line.to_ascii_uppercase();
    if !upper.starts_with("SIGNAL") {
        return None;
    }
    let (_key, value) = split_kv(line)?;
    let digits = value.trim_end_matches('%').trim();
    digits.parse::<f64>().ok()
}

/// Parse a Radio type line.
///
/// Accepts `"Radio type         : 802.11ax"`.
fn try_parse_radio_type_line(line: &str) -> Option<RadioType> {
    let upper = line.to_ascii_uppercase();
    if !upper.starts_with("RADIO TYPE") {
        return None;
    }
    let (_key, value) = split_kv(line)?;
    RadioType::from_netsh_str(value)
}

/// Parse a Band line.
///
/// Accepts `"Band               : 5 GHz"` and variations such as
/// `"2.4 GHz"` and `"6 GHz"`.
fn try_parse_band_line(line: &str) -> Option<BandType> {
    let upper = line.to_ascii_uppercase();
    if !upper.starts_with("BAND") {
        return None;
    }
    let (_key, value) = split_kv(line)?;
    let v = value.to_ascii_lowercase();
    if v.contains("2.4") {
        Some(BandType::Band2_4GHz)
    } else if v.contains('5') && !v.contains('6') {
        Some(BandType::Band5GHz)
    } else if v.contains('6') {
        Some(BandType::Band6GHz)
    } else {
        None
    }
}

/// Parse a Channel line.
///
/// Accepts `"Channel            : 48"`.
fn try_parse_channel_line(line: &str) -> Option<u8> {
    let upper = line.to_ascii_uppercase();
    if !upper.starts_with("CHANNEL") {
        return None;
    }
    let (_key, value) = split_kv(line)?;
    value.trim().parse::<u8>().ok()
}

/// Split a netsh key-value line on the first `" : "` separator.
///
/// The `" : "` (space-colon-space) convention avoids mis-splitting on
/// the colons inside MAC addresses or SSID names that happen to contain
/// colons.
///
/// Also handles the case where the value is empty and the line ends with
/// `" :"` (e.g. `"SSID 1 :"` for hidden networks).
///
/// Returns `(key, value)` with whitespace trimmed from both parts, or
/// `None` when no separator is found.
fn split_kv(line: &str) -> Option<(&str, &str)> {
    // Try " : " first (most common case).
    if let Some(idx) = line.find(" : ") {
        let key = line[..idx].trim();
        let value = line[idx + 3..].trim();
        return Some((key, value));
    }
    // Fall back to " :" at the end of the line (empty value).
    if let Some(stripped) = line.strip_suffix(" :") {
        let key = stripped.trim();
        return Some((key, ""));
    }
    None
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- sample output from the task specification ----------------------------

    const SAMPLE_OUTPUT: &str = "\
SSID 1 : NETGEAR85-5G
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
    BSSID 1                 : d8:32:14:b0:a0:3e
         Signal             : 84%
         Radio type         : 802.11ax
         Band               : 5 GHz
         Channel            : 48

    BSSID 2                 : d8:32:14:b0:a0:3d
         Signal             : 86%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 5

SSID 2 : NeighborNet
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
    BSSID 1                 : aa:bb:cc:dd:ee:ff
         Signal             : 45%
         Radio type         : 802.11ac
         Band               : 5 GHz
         Channel            : 36
";

    // -- full parse tests -----------------------------------------------------

    #[test]
    fn parse_sample_output_yields_three_observations() {
        let results = parse_netsh_output(SAMPLE_OUTPUT).unwrap();
        assert_eq!(results.len(), 3, "expected 3 BSSID observations");
    }

    #[test]
    fn first_bssid_fields() {
        let results = parse_netsh_output(SAMPLE_OUTPUT).unwrap();
        let obs = &results[0];

        assert_eq!(obs.bssid.to_string(), "d8:32:14:b0:a0:3e");
        assert_eq!(obs.ssid, "NETGEAR85-5G");
        assert!(
            (obs.signal_pct - 84.0).abs() < f64::EPSILON,
            "signal_pct should be 84.0, got {}",
            obs.signal_pct
        );
        // pct_to_dbm(84) = 84/2 - 100 = -58
        assert!(
            (obs.rssi_dbm - (-58.0)).abs() < f64::EPSILON,
            "rssi_dbm should be -58.0, got {}",
            obs.rssi_dbm
        );
        assert_eq!(obs.channel, 48);
        assert_eq!(obs.band, BandType::Band5GHz);
        assert_eq!(obs.radio_type, RadioType::Ax);
    }

    #[test]
    fn second_bssid_inherits_same_ssid() {
        let results = parse_netsh_output(SAMPLE_OUTPUT).unwrap();
        let obs = &results[1];

        assert_eq!(obs.bssid.to_string(), "d8:32:14:b0:a0:3d");
        assert_eq!(obs.ssid, "NETGEAR85-5G");
        assert!((obs.signal_pct - 86.0).abs() < f64::EPSILON);
        // pct_to_dbm(86) = 86/2 - 100 = -57
        assert!((obs.rssi_dbm - (-57.0)).abs() < f64::EPSILON);
        assert_eq!(obs.channel, 5);
        assert_eq!(obs.band, BandType::Band2_4GHz);
        assert_eq!(obs.radio_type, RadioType::N);
    }

    #[test]
    fn third_bssid_different_ssid() {
        let results = parse_netsh_output(SAMPLE_OUTPUT).unwrap();
        let obs = &results[2];

        assert_eq!(obs.bssid.to_string(), "aa:bb:cc:dd:ee:ff");
        assert_eq!(obs.ssid, "NeighborNet");
        assert!((obs.signal_pct - 45.0).abs() < f64::EPSILON);
        // pct_to_dbm(45) = 45/2 - 100 = -77.5
        assert!((obs.rssi_dbm - (-77.5)).abs() < f64::EPSILON);
        assert_eq!(obs.channel, 36);
        assert_eq!(obs.band, BandType::Band5GHz);
        assert_eq!(obs.radio_type, RadioType::Ac);
    }

    // -- empty / minimal inputs -----------------------------------------------

    #[test]
    fn empty_output_returns_empty_vec() {
        let results = parse_netsh_output("").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn whitespace_only_output() {
        let results = parse_netsh_output("   \n\n   \n").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn no_networks_message() {
        let output = "There are no wireless networks in range.\n";
        let results = parse_netsh_output(output).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn adapter_disconnected_message() {
        let output = "\
Interface name : Wi-Fi
There is 0 network currently visible.
";
        let results = parse_netsh_output(output).unwrap();
        assert!(results.is_empty());
    }

    // -- signal edge cases ----------------------------------------------------

    #[test]
    fn signal_zero_percent() {
        let input = "\
SSID 1 : WeakNet
    Network type            : Infrastructure
    Authentication          : Open
    Encryption              : None
    BSSID 1                 : 00:11:22:33:44:55
         Signal             : 0%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 1
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].signal_pct - 0.0).abs() < f64::EPSILON);
        // pct_to_dbm(0) = 0/2 - 100 = -100
        assert!((results[0].rssi_dbm - (-100.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn signal_one_hundred_percent() {
        let input = "\
SSID 1 : StrongNet
    Network type            : Infrastructure
    Authentication          : WPA3-Personal
    Encryption              : CCMP
    BSSID 1                 : ff:ee:dd:cc:bb:aa
         Signal             : 100%
         Radio type         : 802.11ax
         Band               : 5 GHz
         Channel            : 149
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].signal_pct - 100.0).abs() < f64::EPSILON);
        // pct_to_dbm(100) = 100/2 - 100 = -50
        assert!((results[0].rssi_dbm - (-50.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn signal_one_percent() {
        let input = "\
SSID 1 : Barely
    Network type            : Infrastructure
    Authentication          : Open
    Encryption              : None
    BSSID 1                 : ab:cd:ef:01:23:45
         Signal             : 1%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 11
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].signal_pct - 1.0).abs() < f64::EPSILON);
        // pct_to_dbm(1) = 0.5 - 100 = -99.5
        assert!((results[0].rssi_dbm - (-99.5)).abs() < f64::EPSILON);
    }

    #[test]
    fn signal_without_percent_sign() {
        // Some locales or future netsh versions might omit the % sign.
        let input = "\
SSID 1 : NoPct
    Network type            : Infrastructure
    BSSID 1                 : 11:22:33:44:55:66
         Signal             : 72
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 6
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].signal_pct - 72.0).abs() < f64::EPSILON);
    }

    // -- SSID edge cases ------------------------------------------------------

    #[test]
    fn hidden_ssid_empty_name() {
        let input = "\
SSID 1 :
    Network type            : Infrastructure
    Authentication          : Open
    Encryption              : None
    BSSID 1                 : ab:cd:ef:01:23:45
         Signal             : 30%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 6
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ssid, "");
    }

    #[test]
    fn unicode_ssid() {
        let input = "\
SSID 1 : \u{2615}CafeWiFi\u{1F4F6}
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
    BSSID 1                 : 12:34:56:78:9a:bc
         Signal             : 60%
         Radio type         : 802.11ac
         Band               : 5 GHz
         Channel            : 44
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ssid, "\u{2615}CafeWiFi\u{1F4F6}");
    }

    #[test]
    fn ssid_with_colons() {
        // An SSID that contains colons should not confuse the parser
        // because we split on " : " (space-colon-space), not bare ":".
        let input = "\
SSID 1 : My:Weird:SSID
    Network type            : Infrastructure
    BSSID 1                 : 11:22:33:44:55:66
         Signal             : 50%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 6
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ssid, "My:Weird:SSID");
    }

    #[test]
    fn bssid_before_any_ssid_uses_empty_ssid() {
        let input = "\
    BSSID 1                 : aa:bb:cc:dd:ee:ff
         Signal             : 50%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 6
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ssid, "");
    }

    // -- missing fields / defaults --------------------------------------------

    #[test]
    fn missing_signal_defaults_to_zero() {
        let input = "\
SSID 1 : Partial
    Network type            : Infrastructure
    BSSID 1                 : 11:22:33:44:55:66
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 11
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].signal_pct - 0.0).abs() < f64::EPSILON);
        assert!((results[0].rssi_dbm - (-100.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn missing_channel_defaults_to_zero() {
        let input = "\
SSID 1 : NoChannel
    Network type            : Infrastructure
    BSSID 1                 : 11:22:33:44:55:66
         Signal             : 50%
         Radio type         : 802.11n
         Band               : 2.4 GHz
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].channel, 0);
    }

    #[test]
    fn missing_radio_type_defaults_to_n() {
        let input = "\
SSID 1 : NoRadio
    Network type            : Infrastructure
    BSSID 1                 : 11:22:33:44:55:66
         Signal             : 50%
         Band               : 5 GHz
         Channel            : 36
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].radio_type, RadioType::N);
    }

    #[test]
    fn missing_band_inferred_from_channel_5ghz() {
        let input = "\
SSID 1 : NoBand5
    Network type            : Infrastructure
    BSSID 1                 : 11:22:33:44:55:66
         Signal             : 50%
         Radio type         : 802.11ac
         Channel            : 149
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].band, BandType::Band5GHz);
    }

    #[test]
    fn missing_band_inferred_from_channel_2_4ghz() {
        let input = "\
SSID 1 : NoBand24
    Network type            : Infrastructure
    BSSID 1                 : 11:22:33:44:55:66
         Signal             : 50%
         Radio type         : 802.11n
         Channel            : 11
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].band, BandType::Band2_4GHz);
    }

    // -- malformed input handling ---------------------------------------------

    #[test]
    fn malformed_lines_are_skipped() {
        let input = "\
SSID 1 : TestNet
    Network type            : Infrastructure
    This line is garbage
    BSSID 1                 : aa:bb:cc:dd:ee:ff
         Signal             : 70%
         Some random text without colon
         Radio type         : 802.11ac
         Band               : 5 GHz
         Channel            : 44
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert!((results[0].signal_pct - 70.0).abs() < f64::EPSILON);
        assert_eq!(results[0].radio_type, RadioType::Ac);
    }

    #[test]
    fn malformed_bssid_mac_is_skipped() {
        let input = "\
SSID 1 : TestNet
    Network type            : Infrastructure
    BSSID 1                 : not-a-mac
         Signal             : 70%
         Radio type         : 802.11ac
         Band               : 5 GHz
         Channel            : 44

    BSSID 2                 : aa:bb:cc:dd:ee:ff
         Signal             : 50%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 6
";
        let results = parse_netsh_output(input).unwrap();
        // The first BSSID has an unparseable MAC so it is dropped.
        // The second BSSID should still parse correctly.
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].bssid.to_string(), "aa:bb:cc:dd:ee:ff");
    }

    // -- multi-SSID / multi-BSSID scenarios -----------------------------------

    #[test]
    fn multiple_ssids_single_bssid_each() {
        let input = "\
SSID 1 : Alpha
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
    BSSID 1                 : 01:02:03:04:05:06
         Signal             : 90%
         Radio type         : 802.11ax
         Band               : 5 GHz
         Channel            : 36

SSID 2 : Bravo
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
    BSSID 1                 : 0a:0b:0c:0d:0e:0f
         Signal             : 40%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 1

SSID 3 : Charlie
    Network type            : Infrastructure
    Authentication          : Open
    Encryption              : None
    BSSID 1                 : a0:b0:c0:d0:e0:f0
         Signal             : 15%
         Radio type         : 802.11ac
         Band               : 5 GHz
         Channel            : 100
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].ssid, "Alpha");
        assert_eq!(results[1].ssid, "Bravo");
        assert_eq!(results[2].ssid, "Charlie");
    }

    #[test]
    fn multiple_ssids_multiple_bssids() {
        let input = "\
SSID 1 : HomeNet
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
    BSSID 1                 : 11:11:11:11:11:11
         Signal             : 95%
         Radio type         : 802.11ax
         Band               : 2.4 GHz
         Channel            : 1
    BSSID 2                 : 22:22:22:22:22:22
         Signal             : 65%
         Radio type         : 802.11ax
         Band               : 5 GHz
         Channel            : 44

SSID 2 : Neighbor
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
    BSSID 1                 : 33:33:33:33:33:33
         Signal             : 30%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 11
    BSSID 2                 : 44:44:44:44:44:44
         Signal             : 18%
         Radio type         : 802.11ac
         Band               : 5 GHz
         Channel            : 149

SSID 3 : Office
    Network type            : Infrastructure
    Authentication          : WPA3-Personal
    Encryption              : GCMP
    BSSID 1                 : 55:55:55:55:55:55
         Signal             : 40%
         Radio type         : 802.11be
         Band               : 6 GHz
         Channel            : 5
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 5, "expected 5 total BSSIDs across 3 SSIDs");

        assert_eq!(results[0].ssid, "HomeNet");
        assert_eq!(results[0].bssid, BssidId::parse("11:11:11:11:11:11").unwrap());
        assert_eq!(results[1].ssid, "HomeNet");
        assert_eq!(results[1].bssid, BssidId::parse("22:22:22:22:22:22").unwrap());

        assert_eq!(results[2].ssid, "Neighbor");
        assert_eq!(results[3].ssid, "Neighbor");

        assert_eq!(results[4].ssid, "Office");
        assert_eq!(results[4].radio_type, RadioType::Be);
        assert_eq!(results[4].band, BandType::Band6GHz);
    }

    // -- band parsing ---------------------------------------------------------

    #[test]
    fn six_ghz_band_parsed() {
        let input = "\
SSID 1 : WiFi6E
    Network type            : Infrastructure
    Authentication          : WPA3-Personal
    Encryption              : GCMP-256
    BSSID 1                 : 01:02:03:04:05:06
         Signal             : 55%
         Radio type         : 802.11ax
         Band               : 6 GHz
         Channel            : 37
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].band, BandType::Band6GHz);
    }

    #[test]
    fn tri_band_output() {
        let input = "\
SSID 1 : TriBand
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
    BSSID 1                 : aa:bb:cc:dd:ee:01
         Signal             : 80%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 6
    BSSID 2                 : aa:bb:cc:dd:ee:02
         Signal             : 70%
         Radio type         : 802.11ac
         Band               : 5 GHz
         Channel            : 36
    BSSID 3                 : aa:bb:cc:dd:ee:03
         Signal             : 55%
         Radio type         : 802.11ax
         Band               : 6 GHz
         Channel            : 1
";
        let results = parse_netsh_output(input).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].band, BandType::Band2_4GHz);
        assert_eq!(results[1].band, BandType::Band5GHz);
        assert_eq!(results[2].band, BandType::Band6GHz);
    }

    // -- dBm conversion -------------------------------------------------------

    #[test]
    fn rssi_dbm_uses_pct_to_dbm() {
        // Verify the parser is consistent with BssidObservation::pct_to_dbm.
        let input = "\
SSID 1 : ConvCheck
    Network type            : Infrastructure
    BSSID 1                 : 01:02:03:04:05:06
         Signal             : 72%
         Radio type         : 802.11n
         Band               : 2.4 GHz
         Channel            : 11
";
        let results = parse_netsh_output(input).unwrap();
        let obs = &results[0];
        let expected = BssidObservation::pct_to_dbm(72.0);
        assert!(
            (obs.rssi_dbm - expected).abs() < f64::EPSILON,
            "rssi_dbm {} should equal pct_to_dbm(72.0) = {}",
            obs.rssi_dbm,
            expected,
        );
    }

    // -- Windows CRLF handling ------------------------------------------------

    #[test]
    fn handles_windows_crlf_line_endings() {
        let output = "SSID 1 : Test\r\n    Network type            : Infrastructure\r\n    Authentication          : Open\r\n    Encryption              : None\r\n    BSSID 1                 : 01:02:03:04:05:06\r\n         Signal             : 50%\r\n         Radio type         : 802.11n\r\n         Band               : 2.4 GHz\r\n         Channel            : 6\r\n";
        let results = parse_netsh_output(output).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].bssid,
            BssidId::parse("01:02:03:04:05:06").unwrap()
        );
        assert!((results[0].signal_pct - 50.0).abs() < f64::EPSILON);
    }

    // -- interface header prefix ----------------------------------------------

    #[test]
    fn output_with_interface_header_prefix() {
        let output = "\
Interface name : Wi-Fi

SSID 1 : TestNet
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
    BSSID 1                 : a1:b2:c3:d4:e5:f6
         Signal             : 88%
         Radio type         : 802.11ax
         Band               : 5 GHz
         Channel            : 36
";
        let results = parse_netsh_output(output).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ssid, "TestNet");
    }

    // -- timestamp consistency ------------------------------------------------

    #[test]
    fn all_observations_share_same_timestamp() {
        let results = parse_netsh_output(SAMPLE_OUTPUT).unwrap();
        assert!(results.len() >= 2);
        let ts = results[0].timestamp;
        for obs in &results[1..] {
            assert_eq!(obs.timestamp, ts);
        }
    }

    // -- extra whitespace / padding -------------------------------------------

    #[test]
    fn bssid_with_extra_trailing_whitespace() {
        let output = "\
SSID 1 : Padded
    Network type            : Infrastructure
    Authentication          : WPA2-Personal
    Encryption              : CCMP
    BSSID 1                 : de:ad:be:ef:ca:fe
         Signal             : 72%
         Radio type         : 802.11ac
         Band               : 5 GHz
         Channel            : 100
";
        let results = parse_netsh_output(output).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ssid, "Padded");
        assert_eq!(results[0].channel, 100);
    }

    // -- line parser unit tests -----------------------------------------------

    #[test]
    fn split_kv_basic() {
        let (k, v) = split_kv("Signal             : 84%").unwrap();
        assert_eq!(k, "Signal");
        assert_eq!(v, "84%");
    }

    #[test]
    fn split_kv_mac_address_value() {
        // The value contains colons but the separator is " : ".
        let (k, v) = split_kv("BSSID 1                 : d8:32:14:b0:a0:3e").unwrap();
        assert_eq!(k, "BSSID 1");
        assert_eq!(v, "d8:32:14:b0:a0:3e");
    }

    #[test]
    fn split_kv_no_separator_returns_none() {
        assert!(split_kv("no separator here").is_none());
    }

    #[test]
    fn split_kv_colon_without_spaces_returns_none() {
        // "aa:bb:cc" has colons but not " : " so it should not match.
        assert!(split_kv("aa:bb:cc").is_none());
    }

    #[test]
    fn try_parse_ssid_line_valid() {
        assert_eq!(
            try_parse_ssid_line("SSID 1 : MyNetwork"),
            Some("MyNetwork".to_owned()),
        );
    }

    #[test]
    fn try_parse_ssid_line_hidden() {
        assert_eq!(try_parse_ssid_line("SSID 1 :"), Some(String::new()));
    }

    #[test]
    fn try_parse_ssid_line_does_not_match_bssid() {
        assert!(try_parse_ssid_line("BSSID 1 : aa:bb:cc:dd:ee:ff").is_none());
    }

    #[test]
    fn try_parse_ssid_line_does_not_match_random() {
        assert!(try_parse_ssid_line("Network type : Infrastructure").is_none());
    }

    #[test]
    fn try_parse_bssid_line_valid() {
        let mac =
            try_parse_bssid_line("BSSID 1                 : d8:32:14:b0:a0:3e").unwrap();
        assert_eq!(mac.to_string(), "d8:32:14:b0:a0:3e");
    }

    #[test]
    fn try_parse_bssid_line_invalid_mac() {
        assert!(
            try_parse_bssid_line("BSSID 1                 : not-a-mac").is_none()
        );
    }

    #[test]
    fn try_parse_signal_line_with_percent() {
        assert_eq!(
            try_parse_signal_line("Signal             : 84%"),
            Some(84.0)
        );
    }

    #[test]
    fn try_parse_signal_line_without_percent() {
        assert_eq!(
            try_parse_signal_line("Signal             : 84"),
            Some(84.0)
        );
    }

    #[test]
    fn try_parse_signal_line_zero() {
        assert_eq!(
            try_parse_signal_line("Signal             : 0%"),
            Some(0.0)
        );
    }

    #[test]
    fn try_parse_channel_line_valid() {
        assert_eq!(try_parse_channel_line("Channel            : 48"), Some(48));
    }

    #[test]
    fn try_parse_channel_line_invalid_returns_none() {
        assert!(try_parse_channel_line("Channel            : abc").is_none());
    }

    #[test]
    fn try_parse_band_line_2_4ghz() {
        assert_eq!(
            try_parse_band_line("Band               : 2.4 GHz"),
            Some(BandType::Band2_4GHz),
        );
    }

    #[test]
    fn try_parse_band_line_5ghz() {
        assert_eq!(
            try_parse_band_line("Band               : 5 GHz"),
            Some(BandType::Band5GHz),
        );
    }

    #[test]
    fn try_parse_band_line_6ghz() {
        assert_eq!(
            try_parse_band_line("Band               : 6 GHz"),
            Some(BandType::Band6GHz),
        );
    }

    #[test]
    fn try_parse_radio_type_line_ax() {
        assert_eq!(
            try_parse_radio_type_line("Radio type         : 802.11ax"),
            Some(RadioType::Ax),
        );
    }

    #[test]
    fn try_parse_radio_type_line_be() {
        assert_eq!(
            try_parse_radio_type_line("Radio type         : 802.11be"),
            Some(RadioType::Be),
        );
    }

    #[test]
    fn try_parse_radio_type_line_ac() {
        assert_eq!(
            try_parse_radio_type_line("Radio type         : 802.11ac"),
            Some(RadioType::Ac),
        );
    }

    #[test]
    fn try_parse_radio_type_line_n() {
        assert_eq!(
            try_parse_radio_type_line("Radio type         : 802.11n"),
            Some(RadioType::N),
        );
    }

    // -- Default / new --------------------------------------------------------

    #[test]
    fn default_creates_scanner() {
        let _scanner = NetshBssidScanner::default();
    }

    #[test]
    fn new_creates_scanner() {
        let _scanner = NetshBssidScanner::new();
    }
}
