//! Core value objects for BSSID identification and observation.
//!
//! These types form the shared kernel of the BSSID Acquisition bounded context
//! as defined in ADR-022 section 3.1.

use std::fmt;
use std::time::Instant;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::error::WifiScanError;

// ---------------------------------------------------------------------------
// BssidId -- Value Object
// ---------------------------------------------------------------------------

/// A unique BSSID identifier wrapping a 6-byte IEEE 802.11 MAC address.
///
/// This is the primary identity for access points in the multi-BSSID scanning
/// pipeline. Two `BssidId` values are equal when their MAC bytes match.
#[derive(Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct BssidId(pub [u8; 6]);

impl BssidId {
    /// Create a `BssidId` from a byte slice.
    ///
    /// Returns an error if the slice is not exactly 6 bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WifiScanError> {
        let arr: [u8; 6] = bytes
            .try_into()
            .map_err(|_| WifiScanError::InvalidMac { len: bytes.len() })?;
        Ok(Self(arr))
    }

    /// Parse a `BssidId` from a colon-separated hex string such as
    /// `"aa:bb:cc:dd:ee:ff"`.
    pub fn parse(s: &str) -> Result<Self, WifiScanError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 6 {
            return Err(WifiScanError::MacParseFailed {
                input: s.to_owned(),
            });
        }

        let mut bytes = [0u8; 6];
        for (i, part) in parts.iter().enumerate() {
            bytes[i] = u8::from_str_radix(part, 16).map_err(|_| WifiScanError::MacParseFailed {
                input: s.to_owned(),
            })?;
        }
        Ok(Self(bytes))
    }

    /// Return the raw 6-byte MAC address.
    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }
}

impl fmt::Debug for BssidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BssidId({self})")
    }
}

impl fmt::Display for BssidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d, e, g] = self.0;
        write!(f, "{a:02x}:{b:02x}:{c:02x}:{d:02x}:{e:02x}:{g:02x}")
    }
}

// ---------------------------------------------------------------------------
// BandType -- Value Object
// ---------------------------------------------------------------------------

/// The WiFi frequency band on which a BSSID operates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BandType {
    /// 2.4 GHz (channels 1-14)
    Band2_4GHz,
    /// 5 GHz (channels 36-177)
    Band5GHz,
    /// 6 GHz (Wi-Fi 6E / 7)
    Band6GHz,
}

impl BandType {
    /// Infer the band from an 802.11 channel number.
    pub fn from_channel(channel: u8) -> Self {
        match channel {
            1..=14 => Self::Band2_4GHz,
            32..=177 => Self::Band5GHz,
            _ => Self::Band6GHz,
        }
    }
}

impl fmt::Display for BandType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Band2_4GHz => write!(f, "2.4 GHz"),
            Self::Band5GHz => write!(f, "5 GHz"),
            Self::Band6GHz => write!(f, "6 GHz"),
        }
    }
}

// ---------------------------------------------------------------------------
// RadioType -- Value Object
// ---------------------------------------------------------------------------

/// The 802.11 radio standard reported by the access point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RadioType {
    /// 802.11n (Wi-Fi 4)
    N,
    /// 802.11ac (Wi-Fi 5)
    Ac,
    /// 802.11ax (Wi-Fi 6 / 6E)
    Ax,
    /// 802.11be (Wi-Fi 7)
    Be,
}

impl RadioType {
    /// Parse a radio type from a `netsh` output string such as `"802.11ax"`.
    ///
    /// Returns `None` for unrecognised strings.
    pub fn from_netsh_str(s: &str) -> Option<Self> {
        let lower = s.trim().to_ascii_lowercase();
        if lower.contains("802.11be") || lower.contains("be") {
            Some(Self::Be)
        } else if lower.contains("802.11ax") || lower.contains("ax") || lower.contains("wi-fi 6")
        {
            Some(Self::Ax)
        } else if lower.contains("802.11ac") || lower.contains("ac") || lower.contains("wi-fi 5")
        {
            Some(Self::Ac)
        } else if lower.contains("802.11n") || lower.contains("wi-fi 4") {
            Some(Self::N)
        } else {
            None
        }
    }
}

impl fmt::Display for RadioType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::N => write!(f, "802.11n"),
            Self::Ac => write!(f, "802.11ac"),
            Self::Ax => write!(f, "802.11ax"),
            Self::Be => write!(f, "802.11be"),
        }
    }
}

// ---------------------------------------------------------------------------
// BssidObservation -- Value Object
// ---------------------------------------------------------------------------

/// A single observation of a BSSID from a WiFi scan.
///
/// This is the fundamental measurement unit: one access point observed once
/// at a specific point in time.
#[derive(Clone, Debug)]
pub struct BssidObservation {
    /// The MAC address of the observed access point.
    pub bssid: BssidId,
    /// Received signal strength in dBm (typically -30 to -90).
    pub rssi_dbm: f64,
    /// Signal quality as a percentage (0-100), as reported by the driver.
    pub signal_pct: f64,
    /// The 802.11 channel number.
    pub channel: u8,
    /// The frequency band.
    pub band: BandType,
    /// The 802.11 radio standard.
    pub radio_type: RadioType,
    /// The SSID (network name). May be empty for hidden networks.
    pub ssid: String,
    /// When this observation was captured.
    pub timestamp: Instant,
}

impl BssidObservation {
    /// Convert signal percentage (0-100) to an approximate dBm value.
    ///
    /// Uses the common linear mapping: `dBm = (pct / 2) - 100`.
    /// This matches the conversion used by Windows WLAN API.
    pub fn pct_to_dbm(pct: f64) -> f64 {
        (pct / 2.0) - 100.0
    }

    /// Convert dBm to a linear amplitude suitable for pseudo-CSI frames.
    ///
    /// Formula: `10^((rssi_dbm + 100) / 20)`, mapping -100 dBm to 1.0.
    pub fn rssi_to_amplitude(rssi_dbm: f64) -> f64 {
        10.0_f64.powf((rssi_dbm + 100.0) / 20.0)
    }

    /// Return the amplitude of this observation (linear scale).
    pub fn amplitude(&self) -> f64 {
        Self::rssi_to_amplitude(self.rssi_dbm)
    }

    /// Encode the channel number as a pseudo-phase value in `[0, pi]`.
    ///
    /// This provides downstream pipeline compatibility with code that expects
    /// phase data, even though RSSI-based scanning has no true phase.
    pub fn pseudo_phase(&self) -> f64 {
        (self.channel as f64 / 48.0) * std::f64::consts::PI
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bssid_id_roundtrip() {
        let mac = [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
        let id = BssidId(mac);
        assert_eq!(id.to_string(), "aa:bb:cc:dd:ee:ff");
        assert_eq!(BssidId::parse("aa:bb:cc:dd:ee:ff").unwrap(), id);
    }

    #[test]
    fn bssid_id_parse_errors() {
        assert!(BssidId::parse("aa:bb:cc").is_err());
        assert!(BssidId::parse("zz:bb:cc:dd:ee:ff").is_err());
        assert!(BssidId::parse("").is_err());
    }

    #[test]
    fn bssid_id_from_bytes() {
        let bytes = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let id = BssidId::from_bytes(&bytes).unwrap();
        assert_eq!(id.0, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

        assert!(BssidId::from_bytes(&[0x01, 0x02]).is_err());
    }

    #[test]
    fn band_type_from_channel() {
        assert_eq!(BandType::from_channel(1), BandType::Band2_4GHz);
        assert_eq!(BandType::from_channel(11), BandType::Band2_4GHz);
        assert_eq!(BandType::from_channel(36), BandType::Band5GHz);
        assert_eq!(BandType::from_channel(149), BandType::Band5GHz);
    }

    #[test]
    fn radio_type_from_netsh() {
        assert_eq!(RadioType::from_netsh_str("802.11ax"), Some(RadioType::Ax));
        assert_eq!(RadioType::from_netsh_str("802.11ac"), Some(RadioType::Ac));
        assert_eq!(RadioType::from_netsh_str("802.11n"), Some(RadioType::N));
        assert_eq!(RadioType::from_netsh_str("802.11be"), Some(RadioType::Be));
        assert_eq!(RadioType::from_netsh_str("unknown"), None);
    }

    #[test]
    fn pct_to_dbm_conversion() {
        // 100% -> -50 dBm
        assert!((BssidObservation::pct_to_dbm(100.0) - (-50.0)).abs() < f64::EPSILON);
        // 0% -> -100 dBm
        assert!((BssidObservation::pct_to_dbm(0.0) - (-100.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn rssi_to_amplitude_baseline() {
        // At -100 dBm, amplitude should be 1.0
        let amp = BssidObservation::rssi_to_amplitude(-100.0);
        assert!((amp - 1.0).abs() < 1e-9);
        // At -80 dBm, amplitude should be 10.0
        let amp = BssidObservation::rssi_to_amplitude(-80.0);
        assert!((amp - 10.0).abs() < 1e-9);
    }
}
