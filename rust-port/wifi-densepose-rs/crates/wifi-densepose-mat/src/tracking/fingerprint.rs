//! CSI-based survivor fingerprint for re-identification across signal gaps.
//!
//! Features are extracted from VitalSignsReading and the last-known location.
//! Re-identification matches Lost tracks to new observations by weighted
//! Euclidean distance on normalized biometric features.

use crate::domain::{
    vital_signs::VitalSignsReading,
    coordinates::Coordinates3D,
};

// ---------------------------------------------------------------------------
// Weight constants for the distance metric
// ---------------------------------------------------------------------------

const W_BREATHING_RATE: f32 = 0.40;
const W_BREATHING_AMP: f32 = 0.25;
const W_HEARTBEAT: f32 = 0.20;
const W_LOCATION: f32 = 0.15;

/// Normalisation ranges for features.
///
/// Each range converts raw feature units into a [0, 1]-scale delta so that
/// different physical quantities can be combined with consistent weighting.
const BREATHING_RATE_RANGE: f32 = 30.0; // bpm: typical 0–30 bpm range
const BREATHING_AMP_RANGE: f32 = 1.0;   // amplitude is already [0, 1]
const HEARTBEAT_RANGE: f32 = 80.0;      // bpm: 40–120 → span 80
const LOCATION_RANGE: f32 = 20.0;       // metres, typical room scale

// ---------------------------------------------------------------------------
// CsiFingerprint
// ---------------------------------------------------------------------------

/// Biometric + spatial fingerprint for re-identifying a survivor after signal loss.
///
/// The fingerprint is built from vital-signs measurements and the last known
/// position.  Two survivors are considered the same individual if their
/// fingerprint `distance` falls below a chosen threshold.
#[derive(Debug, Clone)]
pub struct CsiFingerprint {
    /// Breathing rate in breaths-per-minute (primary re-ID feature)
    pub breathing_rate_bpm: f32,
    /// Breathing amplitude (relative, 0..1 scale)
    pub breathing_amplitude: f32,
    /// Heartbeat rate bpm if available
    pub heartbeat_rate_bpm: Option<f32>,
    /// Last known position hint [x, y, z] in metres
    pub location_hint: [f32; 3],
    /// Number of readings averaged into this fingerprint
    pub sample_count: u32,
}

impl CsiFingerprint {
    /// Extract a fingerprint from a vital-signs reading and an optional location.
    ///
    /// When `location` is `None` the location hint defaults to the origin
    /// `[0, 0, 0]`; callers should treat the location component of the
    /// distance as less reliable in that case.
    pub fn from_vitals(vitals: &VitalSignsReading, location: Option<&Coordinates3D>) -> Self {
        let (breathing_rate_bpm, breathing_amplitude) = match &vitals.breathing {
            Some(b) => (b.rate_bpm, b.amplitude.clamp(0.0, 1.0)),
            None => (0.0, 0.0),
        };

        let heartbeat_rate_bpm = vitals.heartbeat.as_ref().map(|h| h.rate_bpm);

        let location_hint = match location {
            Some(loc) => [loc.x as f32, loc.y as f32, loc.z as f32],
            None => [0.0, 0.0, 0.0],
        };

        Self {
            breathing_rate_bpm,
            breathing_amplitude,
            heartbeat_rate_bpm,
            location_hint,
            sample_count: 1,
        }
    }

    /// Exponential moving-average update: blend a new observation into the
    /// fingerprint.
    ///
    /// `alpha = 0.3` is the weight given to the incoming observation; the
    /// existing fingerprint retains weight `1 − alpha = 0.7`.
    ///
    /// The `sample_count` is incremented by one after each call.
    pub fn update_from_vitals(
        &mut self,
        vitals: &VitalSignsReading,
        location: Option<&Coordinates3D>,
    ) {
        const ALPHA: f32 = 0.3;
        const ONE_MINUS_ALPHA: f32 = 1.0 - ALPHA;

        // Breathing rate and amplitude
        if let Some(b) = &vitals.breathing {
            self.breathing_rate_bpm =
                ONE_MINUS_ALPHA * self.breathing_rate_bpm + ALPHA * b.rate_bpm;
            self.breathing_amplitude =
                ONE_MINUS_ALPHA * self.breathing_amplitude
                    + ALPHA * b.amplitude.clamp(0.0, 1.0);
        }

        // Heartbeat: blend if both present, replace if only new is present,
        // leave unchanged if only old is present, clear if new reading has none.
        match (&self.heartbeat_rate_bpm, vitals.heartbeat.as_ref()) {
            (Some(old), Some(new)) => {
                self.heartbeat_rate_bpm =
                    Some(ONE_MINUS_ALPHA * old + ALPHA * new.rate_bpm);
            }
            (None, Some(new)) => {
                self.heartbeat_rate_bpm = Some(new.rate_bpm);
            }
            (Some(_), None) | (None, None) => {
                // Retain existing value; no new heartbeat information.
            }
        }

        // Location
        if let Some(loc) = location {
            let new_loc = [loc.x as f32, loc.y as f32, loc.z as f32];
            for i in 0..3 {
                self.location_hint[i] =
                    ONE_MINUS_ALPHA * self.location_hint[i] + ALPHA * new_loc[i];
            }
        }

        self.sample_count += 1;
    }

    /// Weighted normalised Euclidean distance to another fingerprint.
    ///
    /// Returns a value in `[0, ∞)`.  Values below ~0.35 indicate a likely
    /// match for a typical indoor environment; this threshold should be
    /// tuned to operational conditions.
    ///
    /// ### Weight redistribution when heartbeat is absent
    ///
    /// If either fingerprint lacks a heartbeat reading the 0.20 weight
    /// normally assigned to heartbeat is redistributed proportionally
    /// among the remaining three features so that the total weight still
    /// sums to 1.0.
    pub fn distance(&self, other: &CsiFingerprint) -> f32 {
        // --- normalised feature deltas ---

        let d_breathing_rate =
            (self.breathing_rate_bpm - other.breathing_rate_bpm).abs() / BREATHING_RATE_RANGE;

        let d_breathing_amp =
            (self.breathing_amplitude - other.breathing_amplitude).abs() / BREATHING_AMP_RANGE;

        // Location: 3-D Euclidean distance, then normalise.
        let loc_dist = {
            let dx = self.location_hint[0] - other.location_hint[0];
            let dy = self.location_hint[1] - other.location_hint[1];
            let dz = self.location_hint[2] - other.location_hint[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        };
        let d_location = loc_dist / LOCATION_RANGE;

        // --- heartbeat with weight redistribution ---
        let (heartbeat_term, effective_w_heartbeat) =
            match (self.heartbeat_rate_bpm, other.heartbeat_rate_bpm) {
                (Some(a), Some(b)) => {
                    let d = (a - b).abs() / HEARTBEAT_RANGE;
                    (d * W_HEARTBEAT, W_HEARTBEAT)
                }
                // One or both fingerprints lack heartbeat — exclude the feature.
                _ => (0.0_f32, 0.0_f32),
            };

        // Total weight of present features.
        let total_weight =
            W_BREATHING_RATE + W_BREATHING_AMP + effective_w_heartbeat + W_LOCATION;

        // Renormalise weights so they sum to 1.0.
        let scale = if total_weight > 1e-6 {
            1.0 / total_weight
        } else {
            1.0
        };

        let distance = (W_BREATHING_RATE * d_breathing_rate
            + W_BREATHING_AMP * d_breathing_amp
            + heartbeat_term
            + W_LOCATION * d_location)
            * scale;

        distance
    }

    /// Returns `true` if `self.distance(other) < threshold`.
    pub fn matches(&self, other: &CsiFingerprint, threshold: f32) -> bool {
        self.distance(other) < threshold
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::vital_signs::{
        BreathingPattern, BreathingType, HeartbeatSignature, MovementProfile, SignalStrength,
        VitalSignsReading,
    };
    use crate::domain::coordinates::Coordinates3D;

    /// Helper to build a VitalSignsReading with controlled breathing and heartbeat.
    fn make_vitals(
        breathing_rate: f32,
        amplitude: f32,
        heartbeat_rate: Option<f32>,
    ) -> VitalSignsReading {
        let breathing = Some(BreathingPattern {
            rate_bpm: breathing_rate,
            amplitude,
            regularity: 0.9,
            pattern_type: BreathingType::Normal,
        });

        let heartbeat = heartbeat_rate.map(|r| HeartbeatSignature {
            rate_bpm: r,
            variability: 0.05,
            strength: SignalStrength::Strong,
        });

        VitalSignsReading::new(breathing, heartbeat, MovementProfile::default())
    }

    /// Helper to build a Coordinates3D at the given position.
    fn make_location(x: f64, y: f64, z: f64) -> Coordinates3D {
        Coordinates3D::with_default_uncertainty(x, y, z)
    }

    /// A fingerprint's distance to itself must be zero (or numerically negligible).
    #[test]
    fn test_fingerprint_self_distance() {
        let vitals = make_vitals(15.0, 0.7, Some(72.0));
        let loc = make_location(3.0, 4.0, 0.0);
        let fp = CsiFingerprint::from_vitals(&vitals, Some(&loc));

        let d = fp.distance(&fp);
        assert!(
            d.abs() < 1e-5,
            "Self-distance should be ~0.0, got {}",
            d
        );
    }

    /// Two fingerprints with identical breathing rates, amplitudes, heartbeat
    /// rates, and locations should be within the threshold.
    #[test]
    fn test_fingerprint_threshold() {
        let vitals = make_vitals(15.0, 0.6, Some(72.0));
        let loc = make_location(2.0, 3.0, 0.0);

        let fp1 = CsiFingerprint::from_vitals(&vitals, Some(&loc));
        let fp2 = CsiFingerprint::from_vitals(&vitals, Some(&loc));

        assert!(
            fp1.matches(&fp2, 0.35),
            "Identical fingerprints must match at threshold 0.35 (distance = {})",
            fp1.distance(&fp2)
        );
    }

    /// Fingerprints with very different breathing rates and locations should
    /// have a distance well above 0.35.
    #[test]
    fn test_fingerprint_very_different() {
        let vitals_a = make_vitals(8.0, 0.3, None);
        let loc_a = make_location(0.0, 0.0, 0.0);
        let fp_a = CsiFingerprint::from_vitals(&vitals_a, Some(&loc_a));

        let vitals_b = make_vitals(20.0, 0.8, None);
        let loc_b = make_location(15.0, 10.0, 0.0);
        let fp_b = CsiFingerprint::from_vitals(&vitals_b, Some(&loc_b));

        let d = fp_a.distance(&fp_b);
        assert!(
            d > 0.35,
            "Very different fingerprints should have distance > 0.35, got {}",
            d
        );
    }

    /// `update_from_vitals` must shift values toward the new observation
    /// (EMA blend) without overshooting.
    #[test]
    fn test_fingerprint_update() {
        // Start with breathing_rate = 12.0
        let initial_vitals = make_vitals(12.0, 0.5, Some(60.0));
        let loc = make_location(0.0, 0.0, 0.0);
        let mut fp = CsiFingerprint::from_vitals(&initial_vitals, Some(&loc));

        let original_rate = fp.breathing_rate_bpm;

        // Update toward 20.0 bpm
        let new_vitals = make_vitals(20.0, 0.8, Some(80.0));
        let new_loc = make_location(5.0, 0.0, 0.0);
        fp.update_from_vitals(&new_vitals, Some(&new_loc));

        // The blended rate must be strictly between the two values.
        assert!(
            fp.breathing_rate_bpm > original_rate,
            "Rate should increase after update toward 20.0, got {}",
            fp.breathing_rate_bpm
        );
        assert!(
            fp.breathing_rate_bpm < 20.0,
            "Rate must not overshoot 20.0 (EMA), got {}",
            fp.breathing_rate_bpm
        );

        // Location should have moved toward the new observation.
        assert!(
            fp.location_hint[0] > 0.0,
            "x-hint should be positive after update toward x=5, got {}",
            fp.location_hint[0]
        );

        // Sample count must be incremented.
        assert_eq!(fp.sample_count, 2, "sample_count should be 2 after one update");
    }
}
