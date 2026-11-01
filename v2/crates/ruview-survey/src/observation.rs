//! [`RangeObservation`]: one measured distance between two fixed nodes.
//!
//! The record is source-agnostic. FTM, UWB, BLE Channel Sounding and a tape
//! measure all produce the same shape; [`RangeMethod`] says which one did.

use serde::{Deserialize, Serialize};

use crate::SurveyError;

/// Largest range accepted, in metres. Anything longer is a parse or unit
/// error for an indoor deployment.
pub const MAX_RANGE_M: f64 = 100.0;

/// Most observations accepted by one [`crate::check`] call.
pub const MAX_OBSERVATIONS: usize = 10_000;

/// Which ranging source produced an observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeMethod {
    /// Wi-Fi Fine Timing Measurement (802.11mc RTT), ESP32 STA to SoftAP.
    Ftm,
    /// UWB two-way ranging (ADR-144 hardware family).
    Uwb,
    /// Bluetooth Channel Sounding, through the BLE CS adapter boundary.
    BleCs,
    /// Operator-entered distance (tape or laser).
    Manual,
}

/// One measured node-to-node distance.
///
/// For FTM, `initiator` is the station and `responder` is the SoftAP. The
/// pair is unordered for geometry, but the roles matter for calibration
/// because the measured bias moved with each responder start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RangeObservation {
    /// Node id that initiated the exchange (`node_id` in `--node-positions`).
    pub initiator: u8,
    /// Node id that responded.
    pub responder: u8,
    /// Reported distance in metres (session median for FTM).
    pub range_m: f64,
    /// One-sigma spread of the session in metres. Use 0 when unknown.
    #[serde(default)]
    pub sigma_m: f64,
    /// Ranging method.
    pub method: RangeMethod,
    /// Number of valid frames or exchanges behind `range_m`.
    #[serde(default)]
    pub samples: u16,
    /// Median RSSI of the session, if the source reports one.
    #[serde(default)]
    pub rssi_dbm: Option<i16>,
    /// Channel bandwidth in MHz, if relevant (FTM: 20 or 40).
    #[serde(default)]
    pub bandwidth_mhz: Option<u16>,
    /// Host receive time in microseconds since the Unix epoch.
    #[serde(default)]
    pub at_us: u64,
    /// Boot counter or boot nonce of the responder. Calibration is only
    /// valid within one responder boot.
    #[serde(default)]
    pub responder_boot: Option<u32>,
    /// True once a per-boot calibration offset has been applied.
    #[serde(default)]
    pub calibrated: bool,
}

impl RangeObservation {
    /// A minimal FTM observation, mostly for tests and examples.
    pub fn ftm(initiator: u8, responder: u8, range_m: f64, sigma_m: f64) -> Self {
        Self {
            initiator,
            responder,
            range_m,
            sigma_m,
            method: RangeMethod::Ftm,
            samples: 0,
            rssi_dbm: None,
            bandwidth_mhz: None,
            at_us: 0,
            responder_boot: None,
            calibrated: false,
        }
    }

    /// The unordered node pair, smaller id first.
    pub fn pair(&self) -> (u8, u8) {
        if self.initiator <= self.responder {
            (self.initiator, self.responder)
        } else {
            (self.responder, self.initiator)
        }
    }

    /// Reject self links, non-finite values and out-of-range distances.
    pub fn validate(&self) -> Result<(), SurveyError> {
        if self.initiator == self.responder {
            return Err(SurveyError::InvalidObservation(format!(
                "node {} ranged to itself",
                self.initiator
            )));
        }
        if !self.range_m.is_finite() || !(0.0..=MAX_RANGE_M).contains(&self.range_m) {
            return Err(SurveyError::InvalidObservation(format!(
                "range {} m for {}-{} is outside 0..={MAX_RANGE_M}",
                self.range_m, self.initiator, self.responder
            )));
        }
        if !self.sigma_m.is_finite() || !(0.0..=MAX_RANGE_M).contains(&self.sigma_m) {
            return Err(SurveyError::InvalidObservation(format!(
                "sigma {} m for {}-{} is not a finite non-negative value",
                self.sigma_m, self.initiator, self.responder
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_is_unordered() {
        assert_eq!(RangeObservation::ftm(5, 2, 1.0, 0.1).pair(), (2, 5));
        assert_eq!(RangeObservation::ftm(2, 5, 1.0, 0.1).pair(), (2, 5));
    }

    #[test]
    fn validate_rejects_bad_values() {
        assert!(RangeObservation::ftm(1, 1, 1.0, 0.1).validate().is_err());
        assert!(RangeObservation::ftm(1, 2, -0.1, 0.1).validate().is_err());
        assert!(RangeObservation::ftm(1, 2, f64::NAN, 0.1)
            .validate()
            .is_err());
        assert!(RangeObservation::ftm(1, 2, 101.0, 0.1).validate().is_err());
        assert!(RangeObservation::ftm(1, 2, 1.0, f64::INFINITY)
            .validate()
            .is_err());
        assert!(RangeObservation::ftm(1, 2, 0.0, 0.0).validate().is_ok());
    }

    #[test]
    fn deserializes_with_optional_fields_missing() {
        let o: RangeObservation =
            serde_json::from_str(r#"{"initiator":1,"responder":2,"range_m":3.5,"method":"ftm"}"#)
                .unwrap();
        assert_eq!(o.sigma_m, 0.0);
        assert_eq!(o.responder_boot, None);
        assert!(!o.calibrated);
    }
}
