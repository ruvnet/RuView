//! Windows WLAN `netsh` command output parsing.
//!
//! Complies with ELMAS.md: pure deterministic parser, <= 350 lines.

/// Parse `netsh wlan show interfaces` output for RSSI (approx dBm), signal quality (pct), and SSID.
pub fn parse_netsh_interfaces_output(output: &str) -> Option<(f64, f64, String)> {
    let mut rssi = None;
    let mut signal = None;
    let mut ssid = None;

    for line in output.lines() {
        let line = line.trim();
        if line.starts_with("Signal") {
            if let Some(pct) = line.split(':').nth(1) {
                let pct = pct.trim().trim_end_matches('%');
                if let Ok(v) = pct.parse::<f64>() {
                    signal = Some(v);
                    rssi = Some(-100.0 + v * 0.6);
                }
            }
        }
        if line.starts_with("SSID") && !line.starts_with("BSSID") {
            if let Some(s) = line.split(':').nth(1) {
                ssid = Some(s.trim().to_string());
            }
        }
    }

    match (rssi, signal, ssid) {
        (Some(r), Some(s), Some(name)) => Some((r, s, name)),
        (Some(r), Some(s), None) => Some((r, s, "Unknown".into())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typical_windows_netsh_output() {
        let raw = r#"
There is 1 interface on the system:

    Name                   : Wi-Fi
    Description            : Intel(R) Wi-Fi 6 AX201 160MHz
    State                  : connected
    SSID                   : Office_5G
    BSSID                  : aa:bb:cc:dd:ee:ff
    Network type           : Infrastructure
    Radio type             : 802.11ax
    Authentication         : WPA2-Personal
    Cipher                 : CCMP
    Connection mode        : Auto Connect
    Channel                : 36
    Receive rate (Mbps)    : 1201
    Transmit rate (Mbps)   : 1201
    Signal                 : 85%
"#;
        let (rssi, signal, ssid) = parse_netsh_interfaces_output(raw).expect("should parse");
        assert_eq!(ssid, "Office_5G");
        assert_eq!(signal, 85.0);
        assert!((rssi - (-49.0)).abs() < 1e-3);
    }
}

