//! Error types for the wifi-densepose-wifiscan crate.

use std::fmt;

/// Errors that can occur during WiFi scanning and BSSID processing.
#[derive(Debug, Clone)]
pub enum WifiScanError {
    /// The BSSID MAC address bytes are invalid (must be exactly 6 bytes).
    InvalidMac {
        /// The number of bytes that were provided.
        len: usize,
    },

    /// Failed to parse a MAC address string (expected `aa:bb:cc:dd:ee:ff`).
    MacParseFailed {
        /// The input string that could not be parsed.
        input: String,
    },

    /// The scan backend returned an error.
    ScanFailed {
        /// Human-readable description of what went wrong.
        reason: String,
    },

    /// Too few BSSIDs are visible for multi-AP mode.
    InsufficientBssids {
        /// Number of BSSIDs observed.
        observed: usize,
        /// Minimum required for multi-AP mode.
        required: usize,
    },

    /// A BSSID was not found in the registry.
    BssidNotFound {
        /// The MAC address that was not found.
        bssid: [u8; 6],
    },

    /// The subcarrier map is full and cannot accept more BSSIDs.
    SubcarrierMapFull {
        /// Maximum capacity of the subcarrier map.
        max: usize,
    },

    /// An RSSI value is out of the expected range.
    RssiOutOfRange {
        /// The invalid RSSI value in dBm.
        value: f64,
    },

    /// The requested operation is not supported by this adapter.
    Unsupported(String),

    /// Failed to execute the scan subprocess.
    ProcessError(String),

    /// Failed to parse scan output.
    ParseError(String),
}

impl fmt::Display for WifiScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMac { len } => {
                write!(f, "invalid MAC address: expected 6 bytes, got {len}")
            }
            Self::MacParseFailed { input } => {
                write!(
                    f,
                    "failed to parse MAC address from '{input}': expected aa:bb:cc:dd:ee:ff"
                )
            }
            Self::ScanFailed { reason } => {
                write!(f, "WiFi scan failed: {reason}")
            }
            Self::InsufficientBssids { observed, required } => {
                write!(
                    f,
                    "insufficient BSSIDs for multi-AP mode: {observed} observed, {required} required"
                )
            }
            Self::BssidNotFound { bssid } => {
                write!(
                    f,
                    "BSSID not found in registry: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                    bssid[0], bssid[1], bssid[2], bssid[3], bssid[4], bssid[5]
                )
            }
            Self::SubcarrierMapFull { max } => {
                write!(
                    f,
                    "subcarrier map is full at {max} entries; cannot add more BSSIDs"
                )
            }
            Self::RssiOutOfRange { value } => {
                write!(f, "RSSI value {value} dBm is out of expected range [-120, 0]")
            }
            Self::Unsupported(msg) => {
                write!(f, "unsupported operation: {msg}")
            }
            Self::ProcessError(msg) => {
                write!(f, "scan process error: {msg}")
            }
            Self::ParseError(msg) => {
                write!(f, "scan output parse error: {msg}")
            }
        }
    }
}

impl std::error::Error for WifiScanError {}
