//! Specialist models (ADR-151 Stage 4).
//!
//! One small, room-calibrated model per biological signal — *specialisation over
//! scale*. Each is fit from the labelled enrollment anchors and is tiny: a
//! threshold, a handful of nearest-prototype vectors, or a band-limited
//! periodicity read. Faster, cheaper, more private, and — because it is tuned to
//! this room's fingerprint — often better than one oversized general model.
//!
//! (ADR-151's frozen Hugging-Face RF Foundation Encoder backbone is the planned
//! upgrade path: these heads would then sit over a shared embedding. The
//! statistical heads here make the pipeline runnable and validatable today.)

use serde::{Deserialize, Serialize};

use crate::anchor::{AnchorLabel, Posture};
use crate::extract::{AnchorFeature, Features};

/// Default minimum breathing-band periodicity score to report a rate, used when
/// a [`BreathingSpecialist`] carries no explicit `min_score` (the serde / pre-
/// trained-default case). Respiration is a strong, narrowband modulation, so a
/// moderate floor rejects noise windows without dropping real breaths.
pub const DEFAULT_BREATHING_MIN_SCORE: f32 = 0.25;

/// Default minimum HR-band periodicity score, used when a [`HeartbeatSpecialist`]
/// carries no explicit `min_score`. Higher than breathing's: sub-mm chest
/// displacement at HR frequencies sits near the CSI noise floor (ADR-151 §3.2),
/// so the heartbeat head demands a cleaner peak before reporting.
pub const DEFAULT_HEARTBEAT_MIN_SCORE: f32 = 0.3;

/// Multiple of the typical inter-anchor spread ([`AnomalySpecialist::scale`])
/// beyond which a live window is fully out-of-distribution (anomaly score 1.0):
/// a window more than this many spreads from every enrolled prototype is novel.
pub const ANOMALY_OUTLIER_SPREADS: f32 = 2.0;

/// Anomaly score above which the window is *labelled* "anomalous" (vs "normal").
/// Distinct from the runtime veto threshold ([`crate::runtime`]); this only
/// drives the human-readable label.
pub const ANOMALY_LABEL_CUTOFF: f32 = 0.5;

/// Fraction by which occupied-anchor variance must exceed the empty-room
/// baseline for [`PresenceSpecialist`]'s variance channel to be trusted
/// (issue #1440). Below this margin the channel is disabled rather than
/// producing a midpoint threshold that can sit below the empty-room
/// baseline itself.
pub const VARIANCE_SEPARATION_MARGIN: f32 = 0.05;

/// Which biological signal a specialist estimates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialistKind {
    /// Respiration rate.
    Breathing,
    /// Heart rate (experimental on commodity CSI).
    Heartbeat,
    /// Sleep restlessness / movement intensity.
    Restlessness,
    /// Body posture (standing / sitting / lying).
    Posture,
    /// Presence (room occupied or not).
    Presence,
    /// Physically-implausible / out-of-distribution signal.
    Anomaly,
}

/// A single specialist's output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecialistReading {
    /// Which specialist.
    pub kind: SpecialistKind,
    /// Numeric value (BPM, score, or class index — see [`SpecialistReading::label`]).
    pub value: f32,
    /// Confidence in `[0, 1]`.
    pub confidence: f32,
    /// Optional human-readable label (e.g. posture class).
    pub label: Option<String>,
}

/// Common specialist behaviour.
pub trait Specialist {
    /// Which signal this estimates.
    fn kind(&self) -> SpecialistKind;
    /// Infer from a live feature window; `None` when not applicable / no confidence.
    fn infer(&self, f: &Features) -> Option<SpecialistReading>;
}

// ---------------------------------------------------------------------------
// Presence
// ---------------------------------------------------------------------------

/// Binary presence gate learned from empty vs occupied anchors.
///
/// Two complementary signals (ADR-152 finding, "variance-only presence"):
/// - **variance** — motion/occupancy energy; catches a moving person but is
///   blind to a *motionless* one, whose body raises the scalar *mean* (extra
///   multipath energy) while barely raising variance;
/// - **mean shift** — |mean − empty-room mean|; catches the motionless person
///   the variance channel misses. Symmetric (abs) because a body can shadow
///   paths and *lower* the mean too.
///
/// Present when EITHER channel fires.
/// `serde` support for an `f32` field that may legitimately hold a non-finite
/// sentinel.
///
/// [`PresenceSpecialist::train`] stores `f32::INFINITY` in `threshold` when the
/// variance channel is disabled (issue #1440). JSON cannot represent infinity,
/// so `serde_json` writes `null` for it and then refuses to read the value back
/// (`invalid type: null, expected f32`): a bank trained on a room whose
/// variance channel is inert could be written but never loaded again.
/// Round-tripping the sentinel explicitly keeps such banks loadable.
mod nonfinite_f32 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &f32, serializer: S) -> Result<S::Ok, S::Error> {
        if value.is_finite() {
            serializer.serialize_f32(*value)
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
        Ok(Option::<f32>::deserialize(deserializer)?.unwrap_or(f32::INFINITY))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceSpecialist {
    /// Decision threshold on series variance.
    ///
    /// `f32::INFINITY` when the variance channel is disabled, which JSON cannot
    /// represent — see [`nonfinite_f32`].
    #[serde(with = "nonfinite_f32")]
    pub threshold: f32,
    /// Occupied-anchor mean variance (for confidence scaling).
    pub occupied_var: f32,
    /// Empty-room mean of the scalar series (mean-shift reference).
    #[serde(default)]
    pub empty_mean: f32,
    /// |mean − empty_mean| beyond which the mean alone indicates presence.
    /// `None` disables the channel — both for banks persisted before the
    /// channel existed (serde default) and for rooms where the empty/occupied
    /// means don't separate at train time.
    #[serde(default)]
    pub mean_dist_threshold: Option<f32>,
}

impl PresenceSpecialist {
    /// Fit from anchors: variance threshold at the midpoint between the empty
    /// variance and the mean occupied variance; mean-shift threshold at half
    /// the empty→occupied mean distance (inert when the means don't separate).
    ///
    /// The variance channel is itself inert (never fires) when `occ_var`
    /// does not genuinely exceed `empty_var` (issue #1440): a midpoint
    /// threshold assumes occupied windows are noisier than the empty-room
    /// baseline, but a still/quiet occupant can measure *less* variance
    /// than an empty room's ambient/interference noise floor. Left
    /// unguarded, the midpoint then sits *below* the empty-room baseline
    /// itself, so a genuinely empty room reads "present" on every frame —
    /// worse than no signal at all. Matches the mean-shift channel, which
    /// already goes inert (`None`) under the equivalent condition.
    pub fn train(anchors: &[AnchorFeature]) -> Option<Self> {
        let empty = anchors.iter().find(|a| a.label == AnchorLabel::Empty)?;
        let occ: Vec<&Features> = anchors
            .iter()
            .filter(|a| a.label.expects_presence())
            .map(|a| &a.features)
            .collect();
        if occ.is_empty() {
            return None;
        }
        let occ_var = occ.iter().map(|f| f.variance).sum::<f32>() / occ.len() as f32;
        let occ_mean = occ.iter().map(|f| f.mean).sum::<f32>() / occ.len() as f32;
        let empty_var = empty.features.variance;
        let empty_mean = empty.features.mean;

        let mean_dist = (occ_mean - empty_mean).abs();
        let mean_dist_threshold = (mean_dist > 1e-4).then(|| 0.5 * mean_dist);

        let variance_separates = occ_var > empty_var * (1.0 + VARIANCE_SEPARATION_MARGIN);
        let threshold = if variance_separates {
            0.5 * (empty_var + occ_var)
        } else {
            f32::INFINITY
        };

        Some(Self {
            threshold,
            occupied_var: occ_var.max(empty_var + 1e-3),
            empty_mean,
            mean_dist_threshold,
        })
    }
}

impl Specialist for PresenceSpecialist {
    fn kind(&self) -> SpecialistKind {
        SpecialistKind::Presence
    }
    fn infer(&self, f: &Features) -> Option<SpecialistReading> {
        let by_variance = f.variance > self.threshold;
        let mean_dist = (f.mean - self.empty_mean).abs();
        let by_mean = self.mean_dist_threshold.is_some_and(|thr| mean_dist > thr);
        let present = by_variance || by_mean;

        // Confidence: strongest margin among the channels that are enabled.
        // An infinite threshold means the variance channel was disabled at
        // train time (issue #1440) — it must contribute 0, not the spurious
        // 1.0 that `(x - inf).abs() / span` would otherwise clamp to.
        let var_conf = if self.threshold.is_finite() {
            let var_span = (self.occupied_var - self.threshold).max(1e-3);
            ((f.variance - self.threshold).abs() / var_span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mean_conf = self
            .mean_dist_threshold
            .map(|thr| ((mean_dist - thr).abs() / thr.max(1e-3)).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let confidence = var_conf.max(mean_conf);

        Some(SpecialistReading {
            kind: SpecialistKind::Presence,
            value: if present { 1.0 } else { 0.0 },
            confidence,
            label: Some(if present { "present" } else { "absent" }.into()),
        })
    }
}

// ---------------------------------------------------------------------------
// Posture (nearest-prototype)
// ---------------------------------------------------------------------------

/// Posture classifier: nearest prototype over the feature embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostureSpecialist {
    /// `(posture, embedding)` prototypes from the posture anchors.
    pub prototypes: Vec<(Posture, [f32; 5])>,
}

impl PostureSpecialist {
    /// Fit prototypes from any anchor that establishes a posture.
    pub fn train(anchors: &[AnchorFeature]) -> Option<Self> {
        let prototypes: Vec<(Posture, [f32; 5])> = anchors
            .iter()
            .filter_map(|a| a.label.posture().map(|p| (p, a.features.embedding())))
            .collect();
        if prototypes.is_empty() {
            None
        } else {
            Some(Self { prototypes })
        }
    }

    fn posture_str(p: Posture) -> &'static str {
        match p {
            Posture::Standing => "standing",
            Posture::Sitting => "sitting",
            Posture::Lying => "lying",
        }
    }
}

impl Specialist for PostureSpecialist {
    fn kind(&self) -> SpecialistKind {
        SpecialistKind::Posture
    }
    fn infer(&self, f: &Features) -> Option<SpecialistReading> {
        let emb = f.embedding();
        let mut best = (f32::MAX, Posture::Standing);
        let mut second = f32::MAX;
        for (p, proto) in &self.prototypes {
            let d: f32 = emb.iter().zip(proto).map(|(a, b)| (a - b) * (a - b)).sum();
            if d < best.0 {
                second = best.0;
                best = (d, *p);
            } else if d < second {
                second = d;
            }
        }
        // Confidence from the margin between nearest and runner-up.
        let confidence = if second.is_finite() && (best.0 + second) > 1e-6 {
            ((second - best.0) / (second + best.0)).clamp(0.0, 1.0)
        } else {
            0.5
        };
        Some(SpecialistReading {
            kind: SpecialistKind::Posture,
            value: best.1 as u8 as f32,
            confidence,
            label: Some(Self::posture_str(best.1).into()),
        })
    }
}

// ---------------------------------------------------------------------------
// Breathing / Heartbeat (band-limited periodicity)
// ---------------------------------------------------------------------------

/// Respiration-rate read from the breathing-band periodicity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BreathingSpecialist {
    /// Minimum periodicity score to report a rate.
    pub min_score: f32,
}

impl Specialist for BreathingSpecialist {
    fn kind(&self) -> SpecialistKind {
        SpecialistKind::Breathing
    }
    fn infer(&self, f: &Features) -> Option<SpecialistReading> {
        let min = if self.min_score > 0.0 {
            self.min_score
        } else {
            DEFAULT_BREATHING_MIN_SCORE
        };
        if f.breathing_score < min || f.breathing_hz <= 0.0 {
            return None;
        }
        Some(SpecialistReading {
            kind: SpecialistKind::Breathing,
            value: f.breathing_hz * 60.0,
            confidence: f.breathing_score,
            label: None,
        })
    }
}

/// Heart-rate read from the HR-band periodicity (experimental on CSI).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeartbeatSpecialist {
    /// Minimum periodicity score to report a rate.
    pub min_score: f32,
}

impl Specialist for HeartbeatSpecialist {
    fn kind(&self) -> SpecialistKind {
        SpecialistKind::Heartbeat
    }
    fn infer(&self, f: &Features) -> Option<SpecialistReading> {
        let min = if self.min_score > 0.0 {
            self.min_score
        } else {
            DEFAULT_HEARTBEAT_MIN_SCORE
        };
        if f.heart_score < min || f.heart_hz <= 0.0 {
            return None;
        }
        Some(SpecialistReading {
            kind: SpecialistKind::Heartbeat,
            value: f.heart_hz * 60.0,
            confidence: f.heart_score,
            label: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Restlessness
// ---------------------------------------------------------------------------

/// Restlessness: live motion normalized between the calm (sleep) and active
/// (small-move) anchors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestlessnessSpecialist {
    /// Motion at rest (sleep posture).
    pub calm_motion: f32,
    /// Motion when actively moving.
    pub active_motion: f32,
}

impl RestlessnessSpecialist {
    /// Fit from the sleep-posture (calm) and small-move (active) anchors.
    pub fn train(anchors: &[AnchorFeature]) -> Option<Self> {
        let calm = anchors
            .iter()
            .find(|a| a.label == AnchorLabel::SleepPosture)
            .or_else(|| anchors.iter().find(|a| a.label == AnchorLabel::LieDown))?
            .features
            .motion;
        let active = anchors
            .iter()
            .find(|a| a.label == AnchorLabel::SmallMove)?
            .features
            .motion;
        if active <= calm {
            return None;
        }
        Some(Self {
            calm_motion: calm,
            active_motion: active,
        })
    }
}

impl Specialist for RestlessnessSpecialist {
    fn kind(&self) -> SpecialistKind {
        SpecialistKind::Restlessness
    }
    fn infer(&self, f: &Features) -> Option<SpecialistReading> {
        let span = (self.active_motion - self.calm_motion).max(1e-3);
        let r = ((f.motion - self.calm_motion) / span).clamp(0.0, 1.0);
        Some(SpecialistReading {
            kind: SpecialistKind::Restlessness,
            value: r,
            confidence: 0.7,
            label: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Anomaly (novelty vs anchor prototypes)
// ---------------------------------------------------------------------------

/// Anomaly detector: distance from the manifold of enrolled anchors. A live
/// window far from every anchor prototype is out-of-distribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalySpecialist {
    /// Anchor embeddings (the in-distribution manifold).
    pub prototypes: Vec<[f32; 5]>,
    /// Distance scale (typical inter-anchor spread) for normalization.
    pub scale: f32,
}

impl AnomalySpecialist {
    /// Fit from all anchor embeddings.
    pub fn train(anchors: &[AnchorFeature]) -> Option<Self> {
        if anchors.len() < 2 {
            return None;
        }
        let prototypes: Vec<[f32; 5]> = anchors.iter().map(|a| a.features.embedding()).collect();
        // Scale = mean nearest-neighbour distance among prototypes.
        let mut nn_sum = 0.0f32;
        for (i, p) in prototypes.iter().enumerate() {
            let mut best = f32::MAX;
            for (j, q) in prototypes.iter().enumerate() {
                if i == j {
                    continue;
                }
                let d: f32 = p.iter().zip(q).map(|(a, b)| (a - b) * (a - b)).sum();
                best = best.min(d);
            }
            if best.is_finite() {
                nn_sum += best.sqrt();
            }
        }
        let scale = (nn_sum / prototypes.len() as f32).max(1e-3);
        Some(Self { prototypes, scale })
    }
}

impl Specialist for AnomalySpecialist {
    fn kind(&self) -> SpecialistKind {
        SpecialistKind::Anomaly
    }
    fn infer(&self, f: &Features) -> Option<SpecialistReading> {
        let emb = f.embedding();
        let mut best = f32::MAX;
        for proto in &self.prototypes {
            let d: f32 = emb
                .iter()
                .zip(proto)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f32>()
                .sqrt();
            best = best.min(d);
        }
        // Beyond ANOMALY_OUTLIER_SPREADS× the typical spread → fully anomalous.
        let score = (best / (ANOMALY_OUTLIER_SPREADS * self.scale)).clamp(0.0, 1.0);
        Some(SpecialistReading {
            kind: SpecialistKind::Anomaly,
            value: score,
            confidence: 0.6,
            label: Some(if score > ANOMALY_LABEL_CUTOFF { "anomalous" } else { "normal" }.into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feat(variance: f32, motion: f32, br_hz: f32, br_score: f32) -> Features {
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

    fn af(label: AnchorLabel, variance: f32, motion: f32) -> AnchorFeature {
        AnchorFeature {
            room_id: "r".into(),
            label,
            features: feat(variance, motion, 0.0, 0.0),
        }
    }

    /// Like `feat` but with an explicit series mean (the presence mean-gate input).
    fn feat_mean(mean: f32, variance: f32, motion: f32) -> Features {
        Features {
            mean,
            variance,
            motion,
            breathing_score: 0.0,
            breathing_hz: 0.0,
            heart_score: 0.0,
            heart_hz: 0.0,
        }
    }

    fn af_mean(label: AnchorLabel, mean: f32, variance: f32, motion: f32) -> AnchorFeature {
        AnchorFeature {
            room_id: "r".into(),
            label,
            features: feat_mean(mean, variance, motion),
        }
    }

    /// Regression: a bank whose variance channel is disabled stores
    /// `threshold = f32::INFINITY`. `serde_json` encodes that as `null`, so the
    /// value has to round-trip through that encoding instead of failing to
    /// load — otherwise `room-status`/`room-watch` reject a freshly trained
    /// bank (issue #1440).
    #[test]
    fn presence_specialist_roundtrips_an_infinite_threshold() {
        let mut p = PresenceSpecialist {
            threshold: f32::INFINITY,
            occupied_var: 1.0,
            empty_mean: 2.0,
            mean_dist_threshold: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: PresenceSpecialist =
            serde_json::from_str(&json).unwrap_or_else(|e| panic!("cannot load {json}: {e}"));
        assert!(back.threshold.is_infinite(), "sentinel lost in {json}");

        // A finite threshold must still be written as a plain number.
        p.threshold = 5.5;
        let json = serde_json::to_string(&p).unwrap();
        let back: PresenceSpecialist = serde_json::from_str(&json).unwrap();
        assert_eq!(back.threshold, 5.5);
        assert!(json.contains("5.5"), "finite threshold not inline: {json}");
    }

    #[test]
    fn presence_learns_threshold_and_classifies() {
        let anchors = vec![
            af(AnchorLabel::Empty, 1.0, 0.1),
            af(AnchorLabel::StandStill, 10.0, 0.2),
        ];
        let p = PresenceSpecialist::train(&anchors).unwrap();
        assert!(p.infer(&feat(12.0, 0.2, 0.0, 0.0)).unwrap().value == 1.0);
        assert!(p.infer(&feat(1.0, 0.1, 0.0, 0.0)).unwrap().value == 0.0);
    }

    /// Issue #1440: a still/quiet occupant can measure LESS variance than an
    /// empty room's ambient/interference noise floor. Before the
    /// `variance_separates` guard, the midpoint threshold would then sit
    /// below the empty-room baseline itself, so a window matching that
    /// baseline exactly (empty room) read "present" — the reporter's
    /// "known-empty room read present 31/31 frames" symptom. The means are
    /// identical here (no mean-shift channel to fall back on), isolating
    /// the variance-channel bug.
    #[test]
    fn presence_inverted_variance_never_reports_empty_room_as_present() {
        let anchors = vec![
            af(AnchorLabel::Empty, 5.0, 0.1),
            af(AnchorLabel::StandStill, 2.0, 0.15),
        ];
        let p = PresenceSpecialist::train(&anchors).unwrap();
        let r = p.infer(&feat(5.0, 0.1, 0.0, 0.0)).unwrap();
        assert_eq!(r.value, 0.0, "empty-room-baseline variance must not read present when occ_var < empty_var");
        assert_eq!(r.confidence, 0.0, "a disabled channel must not report spurious confidence");
    }

    /// ADR-152 "variance-only presence" regression: a MOTIONLESS person raises
    /// the scalar mean (extra multipath energy) but barely the variance — the
    /// mean channel must still detect them, and a window matching the empty
    /// room on BOTH channels must still read absent.
    #[test]
    fn presence_detects_motionless_person_via_mean_shift() {
        let anchors = vec![
            af_mean(AnchorLabel::Empty, 1.0, 1.0, 0.1),
            af_mean(AnchorLabel::StandStill, 1.6, 10.0, 0.2),
            af_mean(AnchorLabel::LieDown, 1.5, 8.0, 0.15),
        ];
        let p = PresenceSpecialist::train(&anchors).unwrap();
        // Motionless person: variance at the empty level, mean shifted.
        let r = p.infer(&feat_mean(1.55, 1.0, 0.05)).unwrap();
        assert_eq!(r.value, 1.0, "motionless person must read present");
        // Truly empty window: both channels quiet.
        let r = p.infer(&feat_mean(1.0, 1.0, 0.05)).unwrap();
        assert_eq!(r.value, 0.0, "empty room must still read absent");
    }

    /// Banks persisted BEFORE the mean gate existed must deserialize to the
    /// inert (+∞) gate and keep their original variance-only behavior.
    #[test]
    fn presence_old_bank_json_stays_variance_only() {
        let old_json = r#"{"threshold":5.5,"occupied_var":10.0}"#;
        let p: PresenceSpecialist = serde_json::from_str(old_json).unwrap();
        assert!(p.mean_dist_threshold.is_none());
        // Mean wildly shifted but variance below threshold → still absent
        // (old behavior preserved; the mean channel is disabled).
        let r = p.infer(&feat_mean(99.0, 1.0, 0.05)).unwrap();
        assert_eq!(r.value, 0.0);
    }

    #[test]
    fn posture_nearest_prototype() {
        let anchors = vec![
            af(AnchorLabel::StandStill, 10.0, 0.2),
            af(AnchorLabel::Sit, 6.0, 0.2),
            af(AnchorLabel::LieDown, 3.0, 0.2),
        ];
        let post = PostureSpecialist::train(&anchors).unwrap();
        // A window close to the standing prototype.
        let r = post.infer(&feat(10.1, 0.2, 0.0, 0.0)).unwrap();
        assert_eq!(r.label.as_deref(), Some("standing"));
    }

    #[test]
    fn breathing_reports_bpm() {
        let b = BreathingSpecialist::default();
        let r = b.infer(&feat(5.0, 0.2, 0.3, 0.8)).unwrap();
        assert!((r.value - 18.0).abs() < 0.1); // 0.3 Hz = 18 BPM
        assert!(r.confidence > 0.5);
        assert!(b.infer(&feat(5.0, 0.2, 0.3, 0.1)).is_none()); // low score → none
    }

    /// De-magic pin: the named default min-scores must equal the historical
    /// literal values, and the gate boundary must be `score >= min` (a window
    /// exactly at the default floor reports; a hair below does not).
    #[test]
    fn default_min_score_constants_match_prior_literals() {
        assert_eq!(DEFAULT_BREATHING_MIN_SCORE, 0.25);
        assert_eq!(DEFAULT_HEARTBEAT_MIN_SCORE, 0.3);
        let b = BreathingSpecialist::default(); // min_score = 0.0 → uses default
        assert!(
            b.infer(&feat(5.0, 0.2, 0.3, DEFAULT_BREATHING_MIN_SCORE)).is_some(),
            "score exactly at the default floor must report"
        );
        assert!(
            b.infer(&feat(5.0, 0.2, 0.3, DEFAULT_BREATHING_MIN_SCORE - 1e-3)).is_none(),
            "score below the default floor must not report"
        );
    }

    /// De-magic pin for the anomaly score scale + label cutoff (value-identical
    /// to the prior `2.0 * scale` / `> 0.5` literals).
    #[test]
    fn anomaly_constants_match_prior_literals() {
        assert_eq!(ANOMALY_OUTLIER_SPREADS, 2.0);
        assert_eq!(ANOMALY_LABEL_CUTOFF, 0.5);
    }

    #[test]
    fn restlessness_normalizes() {
        let anchors = vec![
            af(AnchorLabel::SleepPosture, 3.0, 0.1),
            af(AnchorLabel::SmallMove, 3.0, 1.1),
        ];
        let rs = RestlessnessSpecialist::train(&anchors).unwrap();
        assert!(rs.infer(&feat(3.0, 0.1, 0.0, 0.0)).unwrap().value < 0.1);
        assert!(rs.infer(&feat(3.0, 1.1, 0.0, 0.0)).unwrap().value > 0.9);
    }

    #[test]
    fn anomaly_flags_outliers() {
        let anchors = vec![
            af(AnchorLabel::Empty, 1.0, 0.1),
            af(AnchorLabel::StandStill, 10.0, 0.2),
            af(AnchorLabel::Sit, 6.0, 0.2),
        ];
        let a = AnomalySpecialist::train(&anchors).unwrap();
        // Far-out window.
        let r = a.infer(&feat(500.0, 50.0, 0.0, 0.0)).unwrap();
        assert!(r.value > 0.5, "score {}", r.value);
    }
}
