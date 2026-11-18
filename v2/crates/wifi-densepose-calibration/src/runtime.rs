//! Mixture-of-specialists runtime (ADR-151 §2.5).
//!
//! Every specialist consumes the same live feature window and emits a
//! `{value, confidence}`. Fusion rules keep the output honest:
//! - the **anomaly** specialist holds a veto — a physically-implausible window
//!   suppresses positive vitals/posture rather than propagating a hallucination;
//! - **presence = absent** short-circuits breathing/heartbeat/posture to `None`
//!   (you cannot have a respiration rate in an empty room);
//! - a **STALE** bank (baseline drift) abstains instead of emitting readings.

use serde::{Deserialize, Serialize};

use crate::bank::SpecialistBank;
use crate::extract::Features;
use crate::specialist::{Specialist, SpecialistReading};

/// Fused room state for one feature window.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoomState {
    /// Presence reading.
    pub presence: Option<SpecialistReading>,
    /// Posture reading.
    pub posture: Option<SpecialistReading>,
    /// Breathing reading (BPM).
    pub breathing: Option<SpecialistReading>,
    /// Heartbeat reading (BPM).
    pub heartbeat: Option<SpecialistReading>,
    /// Restlessness reading [0, 1].
    pub restlessness: Option<SpecialistReading>,
    /// Anomaly reading [0, 1].
    pub anomaly: Option<SpecialistReading>,
    /// Anomaly veto fired — vitals/posture suppressed.
    pub vetoed: bool,
    /// Bank is stale (baseline drift) — readings are not trustworthy.
    pub stale: bool,
}

/// Confidence-gated mixture over a [`SpecialistBank`].
pub struct MixtureOfSpecialists {
    bank: SpecialistBank,
    /// Anomaly score above which vitals/posture are vetoed.
    pub veto_threshold: f32,
}

impl MixtureOfSpecialists {
    /// Wrap a bank with the default veto threshold (0.5).
    pub fn new(bank: SpecialistBank) -> Self {
        Self {
            bank,
            veto_threshold: 0.5,
        }
    }

    /// The underlying bank.
    pub fn bank(&self) -> &SpecialistBank {
        &self.bank
    }

    /// Infer fused room state, marking `stale` if the bank was trained against a
    /// different baseline than `current_baseline_id`.
    pub fn infer(&self, f: &Features, current_baseline_id: &str) -> RoomState {
        let mut state = RoomState {
            stale: self.bank.is_stale(current_baseline_id),
            ..Default::default()
        };
        if state.stale {
            return state;
        }

        // Anomaly first — it can veto everything else.
        state.anomaly = self.bank.anomaly.as_ref().and_then(|a| a.infer(f));
        let vetoed = state
            .anomaly
            .as_ref()
            .map(|r| r.value >= self.veto_threshold)
            .unwrap_or(false);
        state.vetoed = vetoed;

        // Presence gate.
        state.presence = self.bank.presence.as_ref().and_then(|p| p.infer(f));
        let present = state
            .presence
            .as_ref()
            .map(|r| r.value > 0.5)
            // An incomplete bank has no authority to unlock dependent output.
            .unwrap_or(false);

        // Restlessness is reported regardless of presence (movement implies presence).
        state.restlessness = self.bank.restlessness.as_ref().and_then(|r| r.infer(f));

        // Vitals + posture only when present and not vetoed.
        if present && !vetoed {
            state.posture = self.bank.posture.as_ref().and_then(|p| p.infer(f));
            state.breathing = self.bank.breathing.infer(f);
            state.heartbeat = self.bank.heartbeat.infer(f);
        }

        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::AnchorLabel;
    use crate::extract::{AnchorFeature, Features};

    fn af(label: AnchorLabel, variance: f32, motion: f32) -> AnchorFeature {
        AnchorFeature {
            room_id: "r".into(),
            label,
            features: Features {
                mean: 1.0,
                variance,
                motion,
                breathing_score: 0.0,
                breathing_hz: 0.0,
                heart_score: 0.0,
                heart_hz: 0.0,
            },
        }
    }

    fn bank() -> SpecialistBank {
        let anchors = vec![
            af(AnchorLabel::Empty, 1.0, 0.1),
            af(AnchorLabel::StandStill, 10.0, 0.2),
            af(AnchorLabel::Sit, 6.0, 0.2),
            af(AnchorLabel::LieDown, 3.0, 0.2),
            af(AnchorLabel::SmallMove, 4.0, 1.2),
            af(AnchorLabel::SleepPosture, 3.0, 0.1),
        ];
        SpecialistBank::train("r", "base-1", &anchors, 1000).unwrap()
    }

    fn live(variance: f32, motion: f32, br_hz: f32, br_score: f32) -> Features {
        Features {
            mean: 1.0,
            variance,
            motion,
            breathing_score: br_score,
            breathing_hz: br_hz,
            heart_score: 0.0,
            heart_hz: 0.0,
        }
    }

    #[test]
    fn empty_room_suppresses_vitals() {
        let mix = MixtureOfSpecialists::new(bank());
        let s = mix.infer(&live(1.0, 0.1, 0.3, 0.9), "base-1");
        assert_eq!(s.presence.unwrap().value, 0.0);
        assert!(s.breathing.is_none(), "no breathing in an empty room");
        assert!(s.posture.is_none());
    }

    #[test]
    fn present_room_reports_breathing() {
        let mix = MixtureOfSpecialists::new(bank());
        let s = mix.infer(&live(10.0, 0.2, 0.3, 0.9), "base-1");
        assert_eq!(s.presence.unwrap().value, 1.0);
        let br = s.breathing.unwrap();
        assert!((br.value - 18.0).abs() < 0.2);
    }

    #[test]
    fn anomaly_vetoes_vitals() {
        let mix = MixtureOfSpecialists::new(bank());
        // Wildly out-of-distribution window → anomaly veto.
        let s = mix.infer(&live(5000.0, 200.0, 0.3, 0.9), "base-1");
        assert!(s.vetoed);
        assert!(s.breathing.is_none());
    }

    #[test]
    fn stale_bank_flagged() {
        let mix = MixtureOfSpecialists::new(bank());
        let s = mix.infer(&live(10.0, 0.2, 0.3, 0.9), "base-2");
        assert!(s.stale);
        assert!(s.presence.is_none(), "stale presence must abstain");
        assert!(s.breathing.is_none(), "stale vitals must abstain");
        assert!(s.posture.is_none(), "stale posture must abstain");
        assert!(s.heartbeat.is_none());
        assert!(s.restlessness.is_none());
        assert!(s.anomaly.is_none());
        assert!(!s.vetoed, "abstention is not an observed anomaly veto");
        let json = serde_json::to_value(&s).unwrap();
        assert!(json["presence"].is_null(), "unknown must not serialize as absent");
        assert_eq!(json["stale"], true);
    }

    #[test]
    fn missing_presence_specialist_abstains_and_suppresses_vitals() {
        let mut incomplete = bank();
        incomplete.presence = None;
        let mix = MixtureOfSpecialists::new(incomplete);

        let mut features = live(10.0, 0.2, 0.3, 0.9);
        features.heart_score = 0.95;
        features.heart_hz = 1.2;
        let s = mix.infer(&features, "base-1");

        assert!(!s.stale);
        assert!(s.anomaly.is_some(), "unaffected specialist still executes");
        assert!(s.restlessness.is_some(), "independent movement evidence is retained");
        assert!(serde_json::to_value(&s).unwrap()["presence"].is_null());
        assert!(s.presence.is_none());
        assert!(s.breathing.is_none());
        assert!(s.heartbeat.is_none());
        assert!(s.posture.is_none());
    }
}
