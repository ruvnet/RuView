//! Link measurements to `rf_link_observation` records.

use ruview_unified::gaussian::{channel_gain, GaussianMap};

use crate::wire::{EvidenceError, EvidenceRecord, RecordBody, RecordMeta, RfLinkObservation};

/// Friis free-space amplitude `λ / (4πd)` between two antennas, taken from
/// `ruview_unified`'s channel model with an empty map so the reference is
/// exactly the one RuView's inverse update uses. `None` when the antennas
/// coincide or the frequency is not a positive finite number.
pub fn free_space_amplitude(tx: [f64; 3], rx: [f64; 3], freq_hz: f64) -> Option<f64> {
    let d2: f64 = (0..3).map(|i| (rx[i] - tx[i]).powi(2)).sum();
    if !(d2.is_finite() && d2 > 1e-18 && freq_hz.is_finite() && freq_hz > 0.0) {
        return None;
    }
    Some(channel_gain(&GaussianMap::new(1.0), tx, rx, freq_hz).norm())
}

/// One link: geometry plus measured and free-space amplitudes (linear,
/// same units).
#[derive(Debug, Clone, PartialEq)]
pub struct LinkMeasurement {
    /// Transmitter antenna position, metres.
    pub tx: [f64; 3],
    /// Receiver antenna position, metres.
    pub rx: [f64; 3],
    /// Carrier frequency, Hz.
    pub freq_hz: f64,
    /// Measured amplitude.
    pub measured_amplitude: f64,
    /// Free-space (reference) amplitude.
    pub free_space_amplitude: f64,
}

impl LinkMeasurement {
    /// A link whose reference is the Friis amplitude for its geometry.
    pub fn against_friis(
        tx: [f64; 3],
        rx: [f64; 3],
        freq_hz: f64,
        measured_amplitude: f64,
    ) -> Result<Self, EvidenceError> {
        let free = free_space_amplitude(tx, rx, freq_hz)
            .ok_or(EvidenceError::OutOfRange("link geometry"))?;
        Ok(Self {
            tx,
            rx,
            freq_hz,
            measured_amplitude,
            free_space_amplitude: free,
        })
    }

    /// `20·log10(free / measured)`: positive when the link is weaker than
    /// free space. Both amplitudes must be positive and finite.
    pub fn excess_loss_db(&self) -> Result<f64, EvidenceError> {
        for (v, field) in [
            (self.measured_amplitude, "measured_amplitude"),
            (self.free_space_amplitude, "free_space_amplitude"),
        ] {
            if !v.is_finite() {
                return Err(EvidenceError::NonFinite(field));
            }
            if v <= 0.0 {
                return Err(EvidenceError::OutOfRange(field));
            }
        }
        Ok(20.0 * (self.free_space_amplitude / self.measured_amplitude).log10())
    }

    /// Build a validated `rf_link_observation` record.
    pub fn to_record(&self, meta: RecordMeta) -> Result<EvidenceRecord, EvidenceError> {
        let body = RfLinkObservation {
            tx: self.tx,
            rx: self.rx,
            freq_hz: self.freq_hz,
            excess_loss_db: self.excess_loss_db()?,
        };
        EvidenceRecord::new(meta, RecordBody::RfLinkObservation(body))
    }
}
