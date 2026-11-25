//! Linux WiFi probe path (Task 2): a cached `iw scan dump` must yield
//! observations, and interface detection must never panic.
//!
//! Run: `cargo test -p wifi-densepose-wifiscan --test linux_scan_test`

use wifi_densepose_wifiscan::{
    detect_interface, parse_iw_dev_interfaces, parse_iw_scan_output, LinuxIwScanner,
};

/// A realistic cached `iw dev <iface> scan dump` with two BSS stanzas.
const CACHED_SCAN_DUMP: &str = "\
BSS aa:bb:cc:dd:ee:ff(on wlp2s0)\n\
\tfreq: 5180\n\
\tsignal: -52.00 dBm\n\
\tSSID: HomeNetwork\n\
\tDS Parameter set: channel 36\n\
BSS 11:22:33:44:55:66(on wlp2s0)\n\
\tfreq: 2437\n\
\tsignal: -71.00 dBm\n\
\tSSID: GuestWifi\n\
\tDS Parameter set: channel 6\n";

#[test]
fn linux_probe_finds_cached_scan() {
    // The probe rule: non-empty cached dump ⇒ WiFi present.
    let obs = parse_iw_scan_output(CACHED_SCAN_DUMP)
        .expect("cached dump must parse");
    assert!(
        !obs.is_empty(),
        "probe_wifi (Linux) treats a non-empty scan dump as WiFi-present"
    );
    assert_eq!(obs.len(), 2);
    assert_eq!(obs[0].ssid, "HomeNetwork");
}

#[test]
fn linux_probe_empty_dump_means_absent() {
    let obs = parse_iw_scan_output("").expect("empty dump must parse");
    assert!(obs.is_empty());
}

#[test]
fn linux_detect_interface_parses_iw_dev() {
    let out = "phy#0\n\tInterface wlp2s0\n\t\tifindex 3\n";
    assert_eq!(
        parse_iw_dev_interfaces(out).as_deref(),
        Some("wlp2s0")
    );
}

#[test]
fn linux_scanner_defaults_to_detected_or_wlan0() {
    // Must construct without panicking, even in containers without WiFi.
    let scanner = LinuxIwScanner::new().use_cached();
    let _ = detect_interface();
    // Scan itself may fail here (no `iw` / no hardware); the probe treats
    // that as absent. Only assert the constructor path is infallible.
    let _ = scanner;
}
