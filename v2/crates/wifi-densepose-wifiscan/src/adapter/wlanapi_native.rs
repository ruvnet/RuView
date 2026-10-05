//! Native `wlanapi.dll` BSS-list FFI — the real Tier 2 scan path.
//!
//! This module replaces the `netsh.exe` subprocess (one `CreateProcess`
//! per scan, ~2 Hz) with direct calls into the Windows WLAN service:
//!
//! - [`WlanOpenHandle`] — open a client session to the WLAN service.
//! - [`WlanEnumInterfaces`] — enumerate the WLAN adapters.
//! - [`WlanGetNetworkBssList`] — pull the cached BSS entries (per-BSSID
//!   `lRssi`, `ulChCenterFrequency`, `dot11BssPhyType`, SSID) for one
//!   interface, with **no** fresh-scan round-trip on the read path.
//! - [`WlanFreeMemory`] / [`WlanCloseHandle`] — release the returned
//!   list and the session handle.
//!
//! `WlanGetNetworkBssList` reads the driver's *already-maintained* BSS
//! cache, so back-to-back reads are bounded by the WLAN service IPC, not
//! by an active-scan dwell. Calling [`scan_native`] in a loop polls that
//! cache; the driver refreshes it in the background (rarely while
//! associated — [`request_scan`] forces a refresh). That is what makes
//! a >2 Hz observation rate possible — see `WlanApiScanner::benchmark`.
//!
//! # Platform gating (honest, not faked)
//!
//! The real FFI is only compiled and linked on `#[cfg(windows)]`. On
//! every other platform [`scan_native`] returns
//! [`WifiScanError::Unsupported`] — it never fabricates observations.
//!
//! # Safety
//!
//! All `unsafe` is confined to this module (the crate is otherwise
//! `unsafe_code = "deny"`). Each raw pointer returned by the WLAN API is
//! null-checked before deref, every list is iterated within its
//! driver-reported `dwNumberOfItems`, and every allocation the API hands
//! back is released with `WlanFreeMemory` before return (including on the
//! error paths).

use std::time::Instant;

use crate::domain::bssid::{BandType, BssidId, BssidObservation, RadioType};
use crate::error::WifiScanError;

/// Map a center frequency in kHz to an 802.11 channel number.
///
/// Covers 2.4 GHz (ch 1-14), 5 GHz (ch 36-177) and 6 GHz (Wi-Fi 6E).
/// Shared by the native path and unit tests; returns 0 for unknown
/// frequencies so the caller can fall back to band-only classification.
#[allow(clippy::cast_possible_truncation)] // channel numbers always fit u8
pub(crate) fn freq_khz_to_channel(frequency_khz: u32) -> u8 {
    let mhz = frequency_khz / 1000;
    match mhz {
        2412..=2472 => ((mhz - 2407) / 5) as u8,
        2484 => 14,
        5170..=5825 => ((mhz - 5000) / 5) as u8,
        5955..=7115 => ((mhz - 5950) / 5) as u8,
        _ => 0,
    }
}

/// Map a center frequency in kHz to a [`BandType`].
pub(crate) fn freq_khz_to_band(frequency_khz: u32) -> BandType {
    let mhz = frequency_khz / 1000;
    match mhz {
        5000..=5900 => BandType::Band5GHz,
        5925..=7200 => BandType::Band6GHz,
        _ => BandType::Band2_4GHz,
    }
}

/// Map a `DOT11_PHY_TYPE` discriminant to our [`RadioType`].
///
/// Values per `windows_sys` (`dot11_phy_type_*`): ht=7 → n, vht=8 → ac,
/// he=10 → ax, eht=11 → be. Anything older (erp/ofdm/dsss) is treated as
/// 802.11n for downstream purposes since this crate targets HT-or-newer
/// CSI-capable APs; `None` is never returned because callers need a
/// concrete radio type for the observation.
pub(crate) fn phy_type_to_radio(phy: i32) -> RadioType {
    match phy {
        11 => RadioType::Be, // dot11_phy_type_eht
        10 => RadioType::Ax, // dot11_phy_type_he
        8 => RadioType::Ac,  // dot11_phy_type_vht
        _ => RadioType::N,   // dot11_phy_type_ht and legacy/erp/ofdm
    }
}

/// Whether a radio type advertises a sounding-capable PHY (HT/VHT/HE/EHT)
/// and is therefore a candidate CSI source. All four 802.11 generations
/// we model expose channel-sounding, so this is `true` for every
/// [`RadioType`] — it exists so callers can filter once legacy
/// (non-HT) APs start appearing in the list with a future `RadioType`.
pub(crate) fn is_csi_capable(_radio: RadioType) -> bool {
    true
}

/// Perform one native BSS-list read across all WLAN interfaces.
///
/// Returns every cached BSS entry as a [`BssidObservation`] with real
/// RSSI (dBm), channel/band derived from `ulChCenterFrequency`, and radio
/// type from `dot11BssPhyType`. `timestamp` is stamped at read time.
///
/// # Errors
///
/// - [`WifiScanError::Unsupported`] on non-Windows targets.
/// - [`WifiScanError::ScanFailed`] if a WLAN API call returns a non-zero
///   Win32 error code or yields no usable interface.
#[cfg(windows)]
#[allow(unsafe_code)]
pub(crate) fn scan_native() -> Result<Vec<BssidObservation>, WifiScanError> {
    use std::ptr;
    use windows_sys::Win32::NetworkManagement::WiFi::{
        dot11_BSS_type_any, WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory,
        WlanGetNetworkBssList, WlanOpenHandle, WLAN_BSS_LIST, WLAN_INTERFACE_INFO_LIST,
    };

    const WLAN_CLIENT_VERSION_2: u32 = 2;

    // 1) Open a session handle to the WLAN service.
    let mut negotiated: u32 = 0;
    let mut handle: windows_sys::Win32::Foundation::HANDLE = ptr::null_mut();
    // SAFETY: out-params are valid local addresses; `preserved` must be null.
    let rc = unsafe {
        WlanOpenHandle(
            WLAN_CLIENT_VERSION_2,
            ptr::null(),
            &mut negotiated,
            &mut handle,
        )
    };
    if rc != 0 {
        return Err(WifiScanError::ScanFailed {
            reason: format!("WlanOpenHandle failed (Win32 error {rc})"),
        });
    }

    // Guard so the handle is always closed, even on early return.
    let result = (|| -> Result<Vec<BssidObservation>, WifiScanError> {
        // 2) Enumerate WLAN interfaces.
        let mut iface_list: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();
        // SAFETY: `handle` is a live WLAN session; out-ptr is a local address.
        let rc = unsafe { WlanEnumInterfaces(handle, ptr::null(), &mut iface_list) };
        if rc != 0 || iface_list.is_null() {
            return Err(WifiScanError::ScanFailed {
                reason: format!("WlanEnumInterfaces failed (Win32 error {rc})"),
            });
        }

        let now = Instant::now();
        let mut observations = Vec::new();

        // SAFETY: `iface_list` is non-null and points at a driver-allocated
        // WLAN_INTERFACE_INFO_LIST; `dwNumberOfItems` bounds the trailing
        // flexible array `InterfaceInfo`.
        let n_ifaces = unsafe { (*iface_list).dwNumberOfItems } as usize;
        let iface_base = unsafe { ptr::addr_of!((*iface_list).InterfaceInfo).cast::<
            windows_sys::Win32::NetworkManagement::WiFi::WLAN_INTERFACE_INFO,
        >() };

        for i in 0..n_ifaces {
            // SAFETY: `i < dwNumberOfItems`, so this element is in-bounds.
            let iface = unsafe { &*iface_base.add(i) };
            let guid = iface.InterfaceGuid;

            // 3) Read the cached BSS list for this interface (no SSID
            //    filter, any BSS type, security flag ignored).
            let mut bss_list: *mut WLAN_BSS_LIST = ptr::null_mut();
            // SAFETY: `handle` is live; `&guid` is a valid GUID; null SSID
            // means "all networks"; out-ptr is a local address.
            let rc = unsafe {
                WlanGetNetworkBssList(
                    handle,
                    &guid,
                    ptr::null(),
                    dot11_BSS_type_any,
                    0, // bSecurityEnabled = FALSE → include open + secured
                    ptr::null(),
                    &mut bss_list,
                )
            };
            if rc != 0 || bss_list.is_null() {
                // Interface may be down / mid-reset; skip it rather than
                // failing the whole scan.
                continue;
            }

            // SAFETY: non-null driver-allocated list; `dwNumberOfItems`
            // bounds the trailing `wlanBssEntries` flexible array.
            let n_bss = unsafe { (*bss_list).dwNumberOfItems } as usize;
            let bss_base = unsafe {
                ptr::addr_of!((*bss_list).wlanBssEntries).cast::<
                    windows_sys::Win32::NetworkManagement::WiFi::WLAN_BSS_ENTRY,
                >()
            };

            for b in 0..n_bss {
                // SAFETY: `b < dwNumberOfItems`, element is in-bounds.
                let entry = unsafe { &*bss_base.add(b) };

                let bssid = BssidId(entry.dot11Bssid);
                let rssi_dbm = f64::from(entry.lRssi);
                let signal_pct = ((rssi_dbm + 100.0) * 2.0).clamp(0.0, 100.0);
                let channel = freq_khz_to_channel(entry.ulChCenterFrequency);
                let band = freq_khz_to_band(entry.ulChCenterFrequency);
                let radio_type = phy_type_to_radio(entry.dot11BssPhyType);

                // SSID: `ucSSID[..uSSIDLength]`, may be non-UTF8 → lossy.
                let ssid_len = (entry.dot11Ssid.uSSIDLength as usize).min(32);
                let ssid = String::from_utf8_lossy(&entry.dot11Ssid.ucSSID[..ssid_len])
                    .trim_end_matches('\0')
                    .to_string();

                observations.push(BssidObservation {
                    bssid,
                    rssi_dbm,
                    signal_pct,
                    channel,
                    band,
                    radio_type,
                    ssid,
                    timestamp: now,
                });
            }

            // 5a) Release the per-interface BSS list.
            // SAFETY: `bss_list` was allocated by the WLAN API and is not
            // used after this call.
            unsafe { WlanFreeMemory(bss_list.cast()) };
        }

        // 5b) Release the interface list.
        // SAFETY: `iface_list` was allocated by the WLAN API; not used after.
        unsafe { WlanFreeMemory(iface_list.cast()) };

        Ok(observations)
    })();

    // 6) Always close the session handle.
    // SAFETY: `handle` is a live WLAN session handle obtained above and not
    // used after this call.
    unsafe { WlanCloseHandle(handle, ptr::null()) };

    result
}

/// Non-Windows fallback: the native `wlanapi.dll` path does not exist, so
/// this returns a typed [`WifiScanError::Unsupported`] rather than
/// fabricating data.
#[cfg(not(windows))]
pub(crate) fn scan_native() -> Result<Vec<BssidObservation>, WifiScanError> {
    Err(WifiScanError::Unsupported(
        "native wlanapi.dll scan is only available on Windows; \
         use the netsh fallback or a platform adapter"
            .to_string(),
    ))
}

/// Ask the WLAN service to start a fresh scan on every WLAN interface.
///
/// `WlanGetNetworkBssList` and `netsh wlan show networks mode=bssid` both
/// read the driver's BSS cache, and while the adapter is associated
/// Windows refreshes that cache only rarely — in practice the cache can
/// hold just the connected AP for minutes. Calling this periodically keeps
/// the multi-BSSID view populated. `WlanScan` is asynchronous: it returns
/// immediately and the cache fills a few seconds later.
///
/// Returns the number of interfaces that accepted the request.
///
/// # Errors
///
/// - [`WifiScanError::Unsupported`] on non-Windows targets.
/// - [`WifiScanError::ScanFailed`] if the WLAN service cannot be opened,
///   the interfaces cannot be enumerated, or no interface accepted the
///   request.
#[cfg(windows)]
#[allow(unsafe_code)]
pub fn request_scan() -> Result<usize, WifiScanError> {
    use std::ptr;
    use windows_sys::Win32::NetworkManagement::WiFi::{
        WlanCloseHandle, WlanEnumInterfaces, WlanFreeMemory, WlanOpenHandle, WlanScan,
        WLAN_INTERFACE_INFO, WLAN_INTERFACE_INFO_LIST,
    };

    const WLAN_CLIENT_VERSION_2: u32 = 2;

    let mut negotiated: u32 = 0;
    let mut handle: windows_sys::Win32::Foundation::HANDLE = ptr::null_mut();
    // SAFETY: out-params are valid local addresses; `preserved` must be null.
    let rc = unsafe {
        WlanOpenHandle(
            WLAN_CLIENT_VERSION_2,
            ptr::null(),
            &mut negotiated,
            &mut handle,
        )
    };
    if rc != 0 {
        return Err(WifiScanError::ScanFailed {
            reason: format!("WlanOpenHandle failed (Win32 error {rc})"),
        });
    }

    let result = (|| -> Result<usize, WifiScanError> {
        let mut iface_list: *mut WLAN_INTERFACE_INFO_LIST = ptr::null_mut();
        // SAFETY: `handle` is a live WLAN session; out-ptr is a local address.
        let rc = unsafe { WlanEnumInterfaces(handle, ptr::null(), &mut iface_list) };
        if rc != 0 || iface_list.is_null() {
            return Err(WifiScanError::ScanFailed {
                reason: format!("WlanEnumInterfaces failed (Win32 error {rc})"),
            });
        }

        // SAFETY: `iface_list` is non-null and driver-allocated;
        // `dwNumberOfItems` bounds the trailing `InterfaceInfo` array.
        let n_ifaces = unsafe { (*iface_list).dwNumberOfItems } as usize;
        let iface_base =
            unsafe { ptr::addr_of!((*iface_list).InterfaceInfo).cast::<WLAN_INTERFACE_INFO>() };

        let mut accepted = 0;
        let mut last_rc = 0;
        for i in 0..n_ifaces {
            // SAFETY: `i < dwNumberOfItems`, so this element is in-bounds.
            let guid = unsafe { (*iface_base.add(i)).InterfaceGuid };
            // SAFETY: `handle` is live; `&guid` is a valid GUID; null SSID and
            // IE data request a scan for all networks; `preserved` is null.
            let rc = unsafe { WlanScan(handle, &guid, ptr::null(), ptr::null(), ptr::null()) };
            if rc == 0 {
                accepted += 1;
            } else {
                last_rc = rc;
            }
        }

        // SAFETY: `iface_list` was allocated by the WLAN API; not used after.
        unsafe { WlanFreeMemory(iface_list.cast()) };

        if accepted == 0 {
            return Err(WifiScanError::ScanFailed {
                reason: format!(
                    "WlanScan accepted on 0 of {n_ifaces} interfaces (last Win32 error {last_rc})"
                ),
            });
        }
        Ok(accepted)
    })();

    // SAFETY: `handle` is a live WLAN session handle and not used after this.
    unsafe { WlanCloseHandle(handle, ptr::null()) };

    result
}

/// Non-Windows fallback: there is no WLAN service to ask for a scan.
#[cfg(not(windows))]
pub fn request_scan() -> Result<usize, WifiScanError> {
    Err(WifiScanError::Unsupported(
        "WlanScan is only available on Windows".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freq_to_channel_2_4ghz() {
        assert_eq!(freq_khz_to_channel(2_412_000), 1);
        assert_eq!(freq_khz_to_channel(2_437_000), 6);
        assert_eq!(freq_khz_to_channel(2_462_000), 11);
        assert_eq!(freq_khz_to_channel(2_484_000), 14);
    }

    #[test]
    fn freq_to_channel_5ghz() {
        assert_eq!(freq_khz_to_channel(5_180_000), 36);
        assert_eq!(freq_khz_to_channel(5_745_000), 149);
    }

    #[test]
    fn freq_to_channel_6ghz() {
        assert_eq!(freq_khz_to_channel(5_955_000), 1);
        assert_eq!(freq_khz_to_channel(5_975_000), 5);
    }

    #[test]
    fn freq_to_channel_unknown_is_zero() {
        assert_eq!(freq_khz_to_channel(900_000), 0);
    }

    #[test]
    fn freq_to_band_classification() {
        assert_eq!(freq_khz_to_band(2_437_000), BandType::Band2_4GHz);
        assert_eq!(freq_khz_to_band(5_180_000), BandType::Band5GHz);
        assert_eq!(freq_khz_to_band(5_975_000), BandType::Band6GHz);
    }

    #[test]
    fn phy_type_maps_to_radio() {
        assert_eq!(phy_type_to_radio(7), RadioType::N); // ht
        assert_eq!(phy_type_to_radio(8), RadioType::Ac); // vht
        assert_eq!(phy_type_to_radio(10), RadioType::Ax); // he
        assert_eq!(phy_type_to_radio(11), RadioType::Be); // eht
        assert_eq!(phy_type_to_radio(4), RadioType::N); // ofdm → n
    }

    #[test]
    fn csi_capable_for_all_modeled_radios() {
        for r in [RadioType::N, RadioType::Ac, RadioType::Ax, RadioType::Be] {
            assert!(is_csi_capable(r));
        }
    }

    /// On non-Windows targets the native path must be an honest typed
    /// `Unsupported`, never a fabricated list.
    #[cfg(not(windows))]
    #[test]
    fn native_scan_unsupported_off_windows() {
        match scan_native() {
            Err(WifiScanError::Unsupported(_)) => {}
            other => panic!("expected Unsupported off-Windows, got {other:?}"),
        }
    }

    /// Off Windows there is no WLAN service to ask for a fresh scan, so the
    /// request must be an honest typed `Unsupported`.
    #[cfg(not(windows))]
    #[test]
    fn request_scan_unsupported_off_windows() {
        match crate::request_scan() {
            Err(WifiScanError::Unsupported(_)) => {}
            other => panic!("expected Unsupported off-Windows, got {other:?}"),
        }
    }

    /// On Windows the scan request must reach the real `WlanScan` FFI.
    /// A CI box with the WLAN service disabled may report `ScanFailed`,
    /// but never `Unsupported`.
    #[cfg(windows)]
    #[test]
    fn request_scan_runs_real_ffi_on_windows() {
        match crate::request_scan() {
            Ok(_) | Err(WifiScanError::ScanFailed { .. }) => {}
            Err(e) => panic!("unexpected scan request error: {e:?}"),
        }
    }

    /// On Windows the native path must execute the real FFI and return a
    /// `Vec` (possibly empty if the BSS cache is cold) — never an error
    /// from the happy path on a machine with a WLAN interface. We accept
    /// either Ok (real adapter present) or a ScanFailed (CI box with the
    /// WLAN service disabled), but it must NOT be Unsupported on Windows.
    #[cfg(windows)]
    #[test]
    fn native_scan_runs_real_ffi_on_windows() {
        match scan_native() {
            Ok(list) => {
                // Real entries (if any) must have plausible RSSI.
                for obs in &list {
                    assert!(
                        obs.rssi_dbm <= 0.0 && obs.rssi_dbm >= -120.0,
                        "implausible RSSI from native FFI: {}",
                        obs.rssi_dbm
                    );
                }
            }
            Err(WifiScanError::ScanFailed { .. }) => { /* WLAN service off — acceptable in CI */ }
            Err(WifiScanError::Unsupported(_)) => {
                panic!("native path must not report Unsupported on Windows")
            }
            Err(e) => panic!("unexpected native scan error: {e:?}"),
        }
    }
}
