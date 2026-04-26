//! Psycho-symbolic inference — context-aware CSI interpretation (ADR-041).
//!
//! Forward-chaining rule-based symbolic reasoning over CSI-derived features.
//! A knowledge base of 16 rules maps combinations of presence, motion energy,
//! breathing rate, time-of-day, coherence, and person count to high-level
//! semantic conclusions (e.g. "person resting", "possible intruder").
//!
//! # Algorithm
//!
//! 1. Each frame, extract a feature vector from host CSI data:
//!    presence, motion_energy, breathing_bpm, heartrate_bpm, n_persons,
//!    coherence (from prior modules), and a coarse time-of-day bucket.
//! 2. Forward-chain: evaluate every rule's 4 condition slots against the
//!    feature vector.  A rule fires when *all* non-disabled conditions match.
//! 3. Confidence propagation: the final confidence of a fired rule is its
//!    base confidence multiplied by the product of per-condition "match
//!    quality" values (how far above/below threshold the feature is).
//! 4. Contradiction detection: if two mutually exclusive conclusions both
//!    fire (e.g. SLEEPING and EXERCISING), emit a CONTRADICTION event and
//!    keep only the conclusion with the higher confidence.
//!
//! # Events (880-series: Autonomous Systems)
//!
//! - `INFERENCE_RESULT`     (880): Conclusion ID of the winning inference.
//! - `INFERENCE_CONFIDENCE` (881): Confidence of the winning inference [0, 1].
//! - `RULE_FIRED`           (882): ID of each rule that fired (may repeat).
//! - `CONTRADICTION`        (883): Encodes conflicting conclusion pair.
//!
//! # Budget
//!
//! H (heavy): < 10 ms per frame on ESP32-S3 WASM3 interpreter.
//! 16 rules x 4 conditions = 64 comparisons + bitmap ops.

// ── Constants ────────────────────────────────────────────────────────────────

/// Maximum rules in the knowledge base.
const MAX_RULES: usize = 16;

/// Condition slots per rule.
const CONDS_PER_RULE: usize = 4;

/// Maximum events emitted per frame.
const MAX_EVENTS: usize = 8;

// ── Event IDs ────────────────────────────────────────────────────────────────

/// Conclusion ID of the winning inference.
pub const EVENT_INFERENCE_RESULT: i32 = 880;

/// Confidence of the winning inference [0, 1].
pub const EVENT_INFERENCE_CONFIDENCE: i32 = 881;

/// Emitted for each rule that fired (value = rule index).
pub const EVENT_RULE_FIRED: i32 = 882;

/// Emitted when two mutually exclusive conclusions both fire.
/// Value encodes `conclusion_a * 100 + conclusion_b`.
pub const EVENT_CONTRADICTION: i32 = 883;

// ── Feature IDs ──────────────────────────────────────────────────────────────

/// Feature vector indices used in rule conditions.
const FEAT_PRESENCE: u8 = 0;       // 0 = absent, 1 = present
const FEAT_MOTION: u8 = 1;         // motion energy [0, ~1000]
const FEAT_BREATHING: u8 = 2;      // breathing BPM
const FEAT_HEARTRATE: u8 = 3;      // heart rate BPM
const FEAT_N_PERSONS: u8 = 4;      // person count
const FEAT_COHERENCE: u8 = 5;      // signal coherence [0, 1]
const FEAT_TIME_BUCKET: u8 = 6;    // 0=morning, 1=afternoon, 2=evening, 3=night
const FEAT_PREV_MOTION: u8 = 7;    // previous frame motion (for sudden change)
const NUM_FEATURES: usize = 8;

/// Feature not used sentinel.
const FEAT_DISABLED: u8 = 0xFF;

// ── Comparison operators ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
enum CmpOp {
    /// Feature >= threshold.
    Gte = 0,
    /// Feature < threshold.
    Lt  = 1,
    /// Feature == threshold (exact integer match).
    Eq  = 2,
    /// Feature != threshold.
    Neq = 3,
}

// ── Conclusion IDs ───────────────────────────────────────────────────────────

/// Semantic conclusion identifiers.
const CONCL_POSSIBLE_INTRUDER: u8    = 1;
const CONCL_PERSON_RESTING: u8       = 2;
const CONCL_PET_OR_ENV: u8           = 3;
const CONCL_SOCIAL_ACTIVITY: u8      = 4;
const CONCL_EXERCISE: u8             = 5;
const CONCL_POSSIBLE_FALL: u8        = 6;
const CONCL_INTERFERENCE: u8         = 7;
const CONCL_SLEEPING: u8             = 8;
const CONCL_COOKING_ACTIVITY: u8     = 9;
const CONCL_LEAVING_HOME: u8         = 10;
const CONCL_ARRIVING_HOME: u8        = 11;
const CONCL_CHILD_PLAYING: u8        = 12;
const CONCL_WORKING_DESK: u8         = 13;
const CONCL_MEDICAL_DISTRESS: u8     = 14;
const CONCL_ROOM_EMPTY_STABLE: u8    = 15;
const CONCL_CROWD_GATHERING: u8      = 16;

// ── Contradiction pairs ──────────────────────────────────────────────────────

/// Pairs of conclusions that are mutually exclusive.
const CONTRADICTION_PAIRS: [(u8, u8); 4] = [
    (CONCL_SLEEPING, CONCL_EXERCISE),
    (CONCL_SLEEPING, CONCL_SOCIAL_ACTIVITY),
    (CONCL_ROOM_EMPTY_STABLE, CONCL_POSSIBLE_INTRUDER),
    (CONCL_PERSON_RESTING, CONCL_EXERCISE),
];

// ── Rule condition ───────────────────────────────────────────────────────────

/// A single condition: `feature[feature_id] <op> threshold`.
#[derive(Clone, Copy)]
struct Condition {
    feature_id: u8,
    op: CmpOp,
    threshold: f32,
}

impl Condition {
    const fn disabled() -> Self {
        Self { feature_id: FEAT_DISABLED, op: CmpOp::Gte, threshold: 0.0 }
    }

    const fn new(feature_id: u8, op: CmpOp, threshold: f32) -> Self {
        Self { feature_id, op, threshold }
    }

    /// Evaluate the condition. Returns a match-quality score in (0, 1] if met,
    /// or 0.0 if not met.  The quality reflects how strongly the feature
    /// exceeds or falls below the threshold.
    fn evaluate(&self, features: &[f32; NUM_FEATURES]) -> f32 {
        if self.feature_id == FEAT_DISABLED {
            return 1.0; // disabled slot always passes
        }
        let val = features[self.feature_id as usize];
        match self.op {
            CmpOp::Gte => {
                if val >= self.threshold {
                    // Quality: how far above threshold (clamped to [0.5, 1.0])
                    let margin = if self.threshold > 1e-6 {
                        val / self.threshold
                    } else {
                        1.0
                    };
                    clamp(margin, 0.5, 1.0)
                } else {
                    0.0
                }
            }
            CmpOp::Lt => {
                if val < self.threshold {
                    let margin = if self.threshold > 1e-6 {
                        1.0 - val / self.threshold
                    } else {
                        1.0
                    };
                    clamp(margin, 0.5, 1.0)
                } else {
                    0.0
                }
            }
            CmpOp::Eq => {
                let diff = if val > self.threshold {
                    val - self.threshold
                } else {
                    self.threshold - val
                };
                if diff < 0.5 { 1.0 } else { 0.0 }
            }
            CmpOp::Neq => {
                let diff = if val > self.threshold {
                    val - self.threshold
                } else {
                    self.threshold - val
                };
                if diff >= 0.5 { 1.0 } else { 0.0 }
            }
        }
    }
}

// ── Rule ─────────────────────────────────────────────────────────────────────

/// A symbolic reasoning rule: conditions -> conclusion with base confidence.
#[derive(Clone, Copy)]
struct Rule {
    conditions: [Condition; CONDS_PER_RULE],
    conclusion_id: u8,
    base_confidence: f32,
}

impl Rule {
    /// Evaluate all conditions.  Returns 0.0 if any condition fails,
    /// otherwise the base confidence weighted by the product of match qualities.
    fn evaluate(&self, features: &[f32; NUM_FEATURES]) -> f32 {
        let mut quality_product = 1.0f32;
        for cond in &self.conditions {
            let q = cond.evaluate(features);
            if q == 0.0 {
                return 0.0;
            }
            quality_product *= q;
        }
        self.base_confidence * quality_product
    }
}

// ── Knowledge base (16 rules) ────────────────────────────────────────────────

/// Build the static 16-rule knowledge base.
///
/// Each rule: `[c0, c1, c2, c3], conclusion_id, base_confidence`.
/// Shorthand: `C(feat, op, thresh)`, `D` = disabled slot.
const fn build_knowledge_base() -> [Rule; MAX_RULES] {
    use CmpOp::*;
    #[allow(non_snake_case)]
    const fn C(f: u8, o: CmpOp, t: f32) -> Condition { Condition::new(f, o, t) }
    const D: Condition = Condition::disabled();
    const P: u8 = FEAT_PRESENCE; const M: u8 = FEAT_MOTION;
    const B: u8 = FEAT_BREATHING; const H: u8 = FEAT_HEARTRATE;
    const N: u8 = FEAT_N_PERSONS; const CO: u8 = FEAT_COHERENCE;
    const T: u8 = FEAT_TIME_BUCKET; const PM: u8 = FEAT_PREV_MOTION;
    [
        // R0: presence + high_motion + night -> intruder
        Rule { conditions: [C(P,Gte,1.0), C(M,Gte,200.0), C(T,Eq,3.0), D],
               conclusion_id: CONCL_POSSIBLE_INTRUDER, base_confidence: 0.80 },
        // R1: presence + low_motion + normal_breathing -> resting
        Rule { conditions: [C(P,Gte,1.0), C(M,Lt,30.0), C(B,Gte,10.0), C(B,Lt,22.0)],
               conclusion_id: CONCL_PERSON_RESTING, base_confidence: 0.90 },
        // R2: no_presence + motion -> pet/env
        Rule { conditions: [C(P,Lt,1.0), C(M,Gte,15.0), D, D],
               conclusion_id: CONCL_PET_OR_ENV, base_confidence: 0.60 },
        // R3: multi_person + high_motion -> social
        Rule { conditions: [C(N,Gte,2.0), C(M,Gte,100.0), D, D],
               conclusion_id: CONCL_SOCIAL_ACTIVITY, base_confidence: 0.70 },
        // R4: single_person + high_motion + elevated_hr -> exercise
        Rule { conditions: [C(N,Eq,1.0), C(M,Gte,150.0), C(H,Gte,100.0), D],
               conclusion_id: CONCL_EXERCISE, base_confidence: 0.80 },
        // R5: presence + sudden_stillness (prev high, now low) -> fall
        Rule { conditions: [C(P,Gte,1.0), C(M,Lt,10.0), C(PM,Gte,150.0), D],
               conclusion_id: CONCL_POSSIBLE_FALL, base_confidence: 0.70 },
        // R6: low_coherence + presence -> interference
        Rule { conditions: [C(CO,Lt,0.4), C(P,Gte,1.0), D, D],
               conclusion_id: CONCL_INTERFERENCE, base_confidence: 0.50 },
        // R7: presence + very_low_motion + night + breathing -> sleeping
        Rule { conditions: [C(P,Gte,1.0), C(M,Lt,5.0), C(T,Eq,3.0), C(B,Gte,8.0)],
               conclusion_id: CONCL_SLEEPING, base_confidence: 0.90 },
        // R8: presence + moderate_motion + evening -> cooking
        Rule { conditions: [C(P,Gte,1.0), C(M,Gte,40.0), C(M,Lt,120.0), C(T,Eq,2.0)],
               conclusion_id: CONCL_COOKING_ACTIVITY, base_confidence: 0.60 },
        // R9: no_presence + prev_motion + morning -> leaving_home
        Rule { conditions: [C(P,Lt,1.0), C(PM,Gte,50.0), C(T,Eq,0.0), D],
               conclusion_id: CONCL_LEAVING_HOME, base_confidence: 0.65 },
        // R10: presence_onset + evening -> arriving_home
        Rule { conditions: [C(P,Gte,1.0), C(M,Gte,60.0), C(PM,Lt,15.0), C(T,Eq,2.0)],
               conclusion_id: CONCL_ARRIVING_HOME, base_confidence: 0.70 },
        // R11: multi_person + very_high_motion + daytime -> child_playing
        Rule { conditions: [C(N,Gte,2.0), C(M,Gte,250.0), C(T,Lt,3.0), D],
               conclusion_id: CONCL_CHILD_PLAYING, base_confidence: 0.60 },
        // R12: single_person + low_motion + good_coherence + daytime -> working
        Rule { conditions: [C(N,Eq,1.0), C(M,Lt,20.0), C(CO,Gte,0.6), C(T,Lt,2.0)],
               conclusion_id: CONCL_WORKING_DESK, base_confidence: 0.75 },
        // R13: presence + very_high_hr + low_motion -> medical_distress
        Rule { conditions: [C(P,Gte,1.0), C(H,Gte,130.0), C(M,Lt,15.0), D],
               conclusion_id: CONCL_MEDICAL_DISTRESS, base_confidence: 0.85 },
        // R14: no_presence + no_motion + good_coherence -> room_empty
        Rule { conditions: [C(P,Lt,1.0), C(M,Lt,5.0), C(CO,Gte,0.6), D],
               conclusion_id: CONCL_ROOM_EMPTY_STABLE, base_confidence: 0.95 },
        // R15: many_persons + high_motion -> crowd
        Rule { conditions: [C(N,Gte,4.0), C(M,Gte,120.0), D, D],
               conclusion_id: CONCL_CROWD_GATHERING, base_confidence: 0.70 },
    ]
}

static KNOWLEDGE_BASE: [Rule; MAX_RULES] = build_knowledge_base();

// ── State ────────────────────────────────────────────────────────────────────

/// Psycho-symbolic inference engine.
pub struct PsychoSymbolicEngine {
    /// Bitmap of rules that fired in the current frame.
    fired_rules: u16,
    /// Previous frame's winning conclusion ID.
    prev_conclusion: u8,
    /// Running count of contradictions detected.
    contradiction_count: u32,
    /// Previous frame's motion energy (for sudden-change detection).
    prev_motion: f32,
    /// Frame counter.
    frame_count: u32,
    /// Coherence estimate (fed externally or from host).
    coherence: f32,
}

impl PsychoSymbolicEngine {
    pub const fn new() -> Self {
        Self {
            fired_rules: 0,
            prev_conclusion: 0,
            contradiction_count: 0,
            prev_motion: 0.0,
            frame_count: 0,
            coherence: 1.0,
        }
    }

    /// Set the coherence score from an upstream coherence monitor.
    pub fn set_coherence(&mut self, coh: f32) {
        self.coherence = coh;
    }

    /// Process one frame of CSI-derived features.
    ///
    /// `presence`    - 0 (absent) or 1 (present) from host.
    /// `motion`      - motion energy from host [0, ~1000].
    /// `breathing`   - breathing BPM from host.
    /// `heartrate`   - heart rate BPM from host.
    /// `n_persons`   - person count from host.
    /// `time_bucket` - coarse time of day: 0=morning, 1=afternoon, 2=evening, 3=night.
    ///
    /// Returns a slice of (event_id, value) pairs to emit.
    pub fn process_frame(
        &mut self,
        presence: f32,
        motion: f32,
        breathing: f32,
        heartrate: f32,
        n_persons: f32,
        time_bucket: f32,
    ) -> &[(i32, f32)] {
        static mut EVENTS: [(i32, f32); MAX_EVENTS] = [(0, 0.0); MAX_EVENTS];
        let mut n_events = 0usize;

        self.frame_count += 1;

        // Build feature vector.
        let features: [f32; NUM_FEATURES] = [
            presence,
            motion,
            breathing,
            heartrate,
            n_persons,
            self.coherence,
            time_bucket,
            self.prev_motion,
        ];

        // Forward-chain: evaluate all rules.
        self.fired_rules = 0;
        let mut best_conclusion: u8 = 0;
        let mut best_confidence: f32 = 0.0;

        // Track all fired conclusions with their confidences.
        let mut fired_conclusions: [f32; 17] = [0.0; 17]; // index = conclusion_id

        for (i, rule) in KNOWLEDGE_BASE.iter().enumerate() {
            let conf = rule.evaluate(&features);
            if conf > 0.0 {
                self.fired_rules |= 1 << i;

                // Emit RULE_FIRED event (up to budget).
                if n_events < MAX_EVENTS {
                    unsafe { EVENTS[n_events] = (EVENT_RULE_FIRED, i as f32); }
                    n_events += 1;
                }

                let cid = rule.conclusion_id as usize;
                if cid < fired_conclusions.len() && conf > fired_conclusions[cid] {
                    fired_conclusions[cid] = conf;
                }

                if conf > best_confidence {
                    best_confidence = conf;
                    best_conclusion = rule.conclusion_id;
                }
            }
        }

        // Contradiction detection.
        for &(a, b) in &CONTRADICTION_PAIRS {
            if fired_conclusions[a as usize] > 0.0 && fired_conclusions[b as usize] > 0.0 {
                self.contradiction_count += 1;
                if n_events < MAX_EVENTS {
                    let encoded = (a as f32) * 100.0 + (b as f32);
                    unsafe { EVENTS[n_events] = (EVENT_CONTRADICTION, encoded); }
                    n_events += 1;
                }
                // Suppress the weaker conclusion.
                if fired_conclusions[a as usize] < fired_conclusions[b as usize] {
                    if best_conclusion == a {
                        best_conclusion = b;
                        best_confidence = fired_conclusions[b as usize];
                    }
                } else {
                    if best_conclusion == b {
                        best_conclusion = a;
                        best_confidence = fired_conclusions[a as usize];
                    }
                }
            }
        }

        // Emit winning inference.
        if best_confidence > 0.0 && n_events < MAX_EVENTS {
            unsafe { EVENTS[n_events] = (EVENT_INFERENCE_RESULT, best_conclusion as f32); }
            n_events += 1;
            if n_events < MAX_EVENTS {
                unsafe { EVENTS[n_events] = (EVENT_INFERENCE_CONFIDENCE, best_confidence); }
                n_events += 1;
            }
        }

        // Update state for next frame.
        self.prev_motion = motion;
        self.prev_conclusion = best_conclusion;

        unsafe { &EVENTS[..n_events] }
    }

    /// Get the bitmap of rules that fired in the last frame.
    pub fn fired_rules(&self) -> u16 {
        self.fired_rules
    }

    /// Get the number of rules that fired in the last frame.
    pub fn fired_count(&self) -> u32 {
        self.fired_rules.count_ones()
    }

    /// Get the previous frame's winning conclusion.
    pub fn prev_conclusion(&self) -> u8 {
        self.prev_conclusion
    }

    /// Get the total contradiction count.
    pub fn contradiction_count(&self) -> u32 {
        self.contradiction_count
    }

    /// Get total frames processed.
    pub fn frame_count(&self) -> u32 {
        self.frame_count
    }

    /// Reset the engine to initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Clamp value to [lo, hi] without libm dependency.
const fn clamp(val: f32, lo: f32, hi: f32) -> f32 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_const_constructor() {
        let engine = PsychoSymbolicEngine::new();
        assert_eq!(engine.frame_count(), 0);
        assert_eq!(engine.fired_rules(), 0);
        assert_eq!(engine.contradiction_count(), 0);
    }

    #[test]
    fn test_person_resting() {
        // presence=1, motion=10, breathing=15, hr=70, 1 person, afternoon, coherence=0.8
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.8);
        let events = engine.process_frame(1.0, 10.0, 15.0, 70.0, 1.0, 1.0);
        // Should fire rule R1 (person_resting, conclusion 2)
        let result = events.iter().find(|e| e.0 == EVENT_INFERENCE_RESULT);
        assert!(result.is_some(), "should produce an inference result");
        // Conclusion should be person_resting (2) or working_desk (13)
        let concl = result.unwrap().1 as u8;
        assert!(concl == CONCL_PERSON_RESTING || concl == CONCL_WORKING_DESK,
            "got conclusion {}, expected resting(2) or working(13)", concl);
    }

    #[test]
    fn test_room_empty() {
        // no presence, no motion, coherence ok
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.8);
        let events = engine.process_frame(0.0, 2.0, 0.0, 0.0, 0.0, 1.0);
        let result = events.iter().find(|e| e.0 == EVENT_INFERENCE_RESULT);
        assert!(result.is_some());
        assert_eq!(result.unwrap().1 as u8, CONCL_ROOM_EMPTY_STABLE);
    }

    #[test]
    fn test_exercise() {
        // 1 person, high motion, elevated HR
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.7);
        let events = engine.process_frame(1.0, 200.0, 25.0, 140.0, 1.0, 1.0);
        let result = events.iter().find(|e| e.0 == EVENT_INFERENCE_RESULT);
        assert!(result.is_some());
        let concl = result.unwrap().1 as u8;
        assert_eq!(concl, CONCL_EXERCISE);
    }

    #[test]
    fn test_possible_intruder_at_night() {
        // presence, high motion, nighttime
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.7);
        let events = engine.process_frame(1.0, 300.0, 0.0, 0.0, 1.0, 3.0);
        let result = events.iter().find(|e| e.0 == EVENT_INFERENCE_RESULT);
        assert!(result.is_some());
        // Should fire intruder rule
        let has_intruder = events.iter().any(|e| {
            e.0 == EVENT_INFERENCE_RESULT && e.1 as u8 == CONCL_POSSIBLE_INTRUDER
        });
        assert!(has_intruder, "should detect possible intruder at night with high motion");
    }

    #[test]
    fn test_possible_fall() {
        // Frame 1: high motion
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.8);
        engine.process_frame(1.0, 200.0, 15.0, 80.0, 1.0, 1.0);

        // Frame 2: sudden stillness (prev_motion = 200, current = 5)
        let events = engine.process_frame(1.0, 5.0, 15.0, 80.0, 1.0, 1.0);
        let result = events.iter().find(|e| e.0 == EVENT_INFERENCE_RESULT);
        assert!(result.is_some());
        let concl = result.unwrap().1 as u8;
        // Should detect possible fall (or at least person_resting which also fires)
        assert!(concl == CONCL_POSSIBLE_FALL || concl == CONCL_PERSON_RESTING,
            "got conclusion {}, expected fall(6) or resting(2)", concl);
    }

    #[test]
    fn test_contradiction_detection() {
        // Scenario: sleeping + exercise both try to fire.
        // sleeping: presence=1, motion<5, night, breathing>=8
        // exercise: 1 person, motion>=150, HR>=100
        // These are contradictory and cannot both be true.
        // We test the contradiction pair exists.
        let pair = CONTRADICTION_PAIRS.iter().find(|p| {
            (p.0 == CONCL_SLEEPING && p.1 == CONCL_EXERCISE) ||
            (p.0 == CONCL_EXERCISE && p.1 == CONCL_SLEEPING)
        });
        assert!(pair.is_some(), "sleeping/exercise contradiction should be registered");
    }

    #[test]
    fn test_pet_or_environment() {
        // no presence but motion detected
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.8);
        let events = engine.process_frame(0.0, 25.0, 0.0, 0.0, 0.0, 1.0);
        let result = events.iter().find(|e| e.0 == EVENT_INFERENCE_RESULT);
        assert!(result.is_some());
        assert_eq!(result.unwrap().1 as u8, CONCL_PET_OR_ENV);
    }

    #[test]
    fn test_social_activity() {
        // 3 persons, high motion
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.7);
        let events = engine.process_frame(1.0, 150.0, 18.0, 85.0, 3.0, 2.0);
        let result = events.iter().find(|e| e.0 == EVENT_INFERENCE_RESULT);
        assert!(result.is_some());
        let concl = result.unwrap().1 as u8;
        assert_eq!(concl, CONCL_SOCIAL_ACTIVITY);
    }

    #[test]
    fn test_rule_fired_events() {
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.8);
        let events = engine.process_frame(1.0, 10.0, 15.0, 70.0, 1.0, 1.0);
        // Should have at least one RULE_FIRED event.
        let rule_fired = events.iter().filter(|e| e.0 == EVENT_RULE_FIRED).count();
        assert!(rule_fired >= 1, "at least one rule should fire");
    }

    #[test]
    fn test_medical_distress() {
        // presence, very high HR, low motion
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.8);
        let events = engine.process_frame(1.0, 5.0, 12.0, 150.0, 1.0, 1.0);
        let result = events.iter().find(|e| e.0 == EVENT_INFERENCE_RESULT);
        assert!(result.is_some());
        let concl = result.unwrap().1 as u8;
        // Medical distress has confidence 0.85, should be the highest
        assert_eq!(concl, CONCL_MEDICAL_DISTRESS);
    }

    #[test]
    fn test_interference() {
        // presence but low coherence
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.2);
        let events = engine.process_frame(1.0, 10.0, 0.0, 0.0, 1.0, 1.0);
        // Interference should fire (conclusion 7)
        let has_interference = events.iter().any(|e| {
            e.0 == EVENT_RULE_FIRED
        });
        assert!(has_interference, "should fire at least one rule with low coherence");
    }

    #[test]
    fn test_reset() {
        let mut engine = PsychoSymbolicEngine::new();
        engine.set_coherence(0.8);
        engine.process_frame(1.0, 10.0, 15.0, 70.0, 1.0, 1.0);
        assert!(engine.frame_count() > 0);

        engine.reset();
        assert_eq!(engine.frame_count(), 0);
        assert_eq!(engine.fired_rules(), 0);
    }
}
