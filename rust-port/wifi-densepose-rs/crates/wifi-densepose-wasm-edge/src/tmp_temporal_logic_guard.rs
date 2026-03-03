//! LTL (Linear Temporal Logic) safety invariant checker -- ADR-041 WASM edge module.
//!
//! Encodes 8 safety rules as state machines monitoring CSI-derived events.
//! G-rules (globally) are violated on any single frame; F-rules (eventually)
//! have deadlines. Emits violations with counterexample frame indices.
//!
//! Event IDs: 795-797 (Temporal Logic category).

const NUM_RULES: usize = 8;
const FAST_BREATH_DEADLINE: u32 = 100;   // 5s at 20 Hz
const SEIZURE_EXCLUSION: u32 = 1200;     // 60s at 20 Hz
const MOTION_STOP_DEADLINE: u32 = 6000;  // 300s at 20 Hz

pub const EVENT_LTL_VIOLATION: i32 = 795;
pub const EVENT_LTL_SATISFACTION: i32 = 796;
pub const EVENT_COUNTEREXAMPLE: i32 = 797;

/// Per-frame sensor snapshot for rule evaluation.
#[derive(Clone, Copy)]
pub struct FrameInput {
    pub presence: i32, pub n_persons: i32, pub motion_energy: f32,
    pub coherence: f32, pub breathing_bpm: f32, pub heartrate_bpm: f32,
    pub fall_alert: bool, pub intrusion_alert: bool, pub person_id_active: bool,
    pub vital_signs_active: bool, pub seizure_detected: bool, pub normal_gait: bool,
}
impl FrameInput {
    pub const fn default() -> Self {
        Self { presence:0, n_persons:0, motion_energy:0.0, coherence:1.0,
               breathing_bpm:0.0, heartrate_bpm:0.0, fall_alert:false,
               intrusion_alert:false, person_id_active:false, vital_signs_active:false,
               seizure_detected:false, normal_gait:false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)] #[repr(u8)]
pub enum RuleState { Satisfied=0, Violated=1, Pending=2 }

#[derive(Clone, Copy)]
struct Rule { state: RuleState, deadline: u32, vio_frame: u32 }
impl Rule { const fn new() -> Self { Self { state: RuleState::Satisfied, deadline: 0, vio_frame: 0 } } }

/// LTL safety invariant guard.
pub struct TemporalLogicGuard {
    rules: [Rule; NUM_RULES],
    vio_counts: [u32; NUM_RULES],
    frame_idx: u32,
    report_interval: u32,
}

impl TemporalLogicGuard {
    pub const fn new() -> Self {
        Self { rules: [Rule::new(); NUM_RULES], vio_counts: [0; NUM_RULES],
               frame_idx: 0, report_interval: 200 }
    }

    /// Process one frame. Returns events to emit.
    pub fn on_frame(&mut self, input: &FrameInput) -> &[(i32, f32)] {
        self.frame_idx += 1;
        static mut EV: [(i32, f32); 12] = [(0, 0.0); 12];
        let mut n = 0usize;

        // G-rules (0-3, 6): violated when condition holds on any frame.
        let checks: [(usize, bool); 5] = [
            (0, input.presence == 0 && input.fall_alert),
            (1, input.intrusion_alert && input.presence == 0),
            (2, input.n_persons == 0 && input.person_id_active),
            (3, input.coherence < 0.3 && input.vital_signs_active),
            (6, input.heartrate_bpm > 150.0),
        ];
        let mut g = 0usize;
        while g < 5 {
            let (rid, viol) = checks[g];
            if viol {
                if self.rules[rid].state != RuleState::Violated {
                    self.rules[rid].state = RuleState::Violated;
                    self.rules[rid].vio_frame = self.frame_idx;
                    self.vio_counts[rid] += 1;
                    if n + 1 < 12 { unsafe {
                        EV[n] = (EVENT_LTL_VIOLATION, rid as f32);
                        EV[n+1] = (EVENT_COUNTEREXAMPLE, self.frame_idx as f32);
                    } n += 2; }
                }
            } else { self.rules[rid].state = RuleState::Satisfied; }
            g += 1;
        }

        // Rule 4: F(motion_start -> motion_end within 300s).
        if self.check_deadline_rule(4, input.motion_energy > 0.1, MOTION_STOP_DEADLINE) {
            if n + 1 < 12 { unsafe {
                EV[n] = (EVENT_LTL_VIOLATION, 4.0);
                EV[n+1] = (EVENT_COUNTEREXAMPLE, self.frame_idx as f32);
            } n += 2; }
        }

        // Rule 5: G(breathing>40 -> alert within 5s).
        if self.check_deadline_rule(5, input.breathing_bpm > 40.0, FAST_BREATH_DEADLINE) {
            if n + 1 < 12 { unsafe {
                EV[n] = (EVENT_LTL_VIOLATION, 5.0);
                EV[n+1] = (EVENT_COUNTEREXAMPLE, self.frame_idx as f32);
            } n += 2; }
        }

        // Rule 7: G(seizure -> !normal_gait within 60s).
        match self.rules[7].state {
            RuleState::Satisfied => {
                if input.seizure_detected {
                    self.rules[7].state = RuleState::Pending;
                    self.rules[7].deadline = self.frame_idx + SEIZURE_EXCLUSION;
                }
            }
            RuleState::Pending => {
                if input.normal_gait {
                    self.rules[7].state = RuleState::Violated;
                    self.rules[7].vio_frame = self.frame_idx;
                    self.vio_counts[7] += 1;
                    if n + 1 < 12 { unsafe {
                        EV[n] = (EVENT_LTL_VIOLATION, 7.0);
                        EV[n+1] = (EVENT_COUNTEREXAMPLE, self.frame_idx as f32);
                    } n += 2; }
                } else if self.frame_idx >= self.rules[7].deadline {
                    self.rules[7].state = RuleState::Satisfied;
                }
            }
            RuleState::Violated => {
                if self.frame_idx >= self.rules[7].deadline {
                    self.rules[7].state = RuleState::Satisfied;
                }
            }
        }

        if self.frame_idx % self.report_interval == 0 && n < 12 {
            unsafe { EV[n] = (EVENT_LTL_SATISFACTION, self.satisfied_count() as f32); }
            n += 1;
        }
        unsafe { &EV[..n] }
    }

    /// Generic deadline rule: condition triggers pending, expiry = violation,
    /// condition clearing = satisfied. Returns true if a new violation just occurred.
    fn check_deadline_rule(&mut self, rid: usize, cond: bool, deadline: u32) -> bool {
        match self.rules[rid].state {
            RuleState::Satisfied => {
                if cond {
                    self.rules[rid].state = RuleState::Pending;
                    self.rules[rid].deadline = self.frame_idx + deadline;
                }
                false
            }
            RuleState::Pending => {
                if !cond {
                    self.rules[rid].state = RuleState::Satisfied;
                    false
                } else if self.frame_idx >= self.rules[rid].deadline {
                    self.rules[rid].state = RuleState::Violated;
                    self.rules[rid].vio_frame = self.frame_idx;
                    self.vio_counts[rid] += 1;
                    true
                } else {
                    false
                }
            }
            RuleState::Violated => { if !cond { self.rules[rid].state = RuleState::Satisfied; } false }
        }
    }

    pub fn satisfied_count(&self) -> u8 {
        let mut c = 0u8; let mut i = 0;
        while i < NUM_RULES { if self.rules[i].state == RuleState::Satisfied { c += 1; } i += 1; }
        c
    }
    pub fn violation_count(&self, r: usize) -> u32 { if r < NUM_RULES { self.vio_counts[r] } else { 0 } }
    pub fn rule_state(&self, r: usize) -> RuleState {
        if r < NUM_RULES { self.rules[r].state } else { RuleState::Satisfied }
    }
    pub fn last_violation_frame(&self, r: usize) -> u32 {
        if r < NUM_RULES { self.rules[r].vio_frame } else { 0 }
    }
    pub fn frame_index(&self) -> u32 { self.frame_idx }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal() -> FrameInput {
        FrameInput { presence:1, n_persons:1, motion_energy:0.05, coherence:0.8,
                     breathing_bpm:16.0, heartrate_bpm:72.0, fall_alert:false,
                     intrusion_alert:false, person_id_active:true, vital_signs_active:true,
                     seizure_detected:false, normal_gait:true }
    }

    #[test] fn test_init() {
        let g = TemporalLogicGuard::new();
        assert_eq!(g.satisfied_count(), NUM_RULES as u8);
    }

    #[test] fn test_normal_all_satisfied() {
        let mut g = TemporalLogicGuard::new();
        for _ in 0..100 { g.on_frame(&normal()); }
        assert_eq!(g.satisfied_count(), NUM_RULES as u8);
    }

    #[test] fn test_motion_causes_pending() {
        let mut g = TemporalLogicGuard::new();
        let mut inp = normal(); inp.motion_energy = 0.3;
        g.on_frame(&inp);
        assert_eq!(g.rule_state(4), RuleState::Pending);
        assert_eq!(g.satisfied_count(), (NUM_RULES - 1) as u8);
    }

    #[test] fn test_rule0_fall_empty() {
        let mut g = TemporalLogicGuard::new();
        let mut inp = FrameInput::default(); inp.fall_alert = true;
        g.on_frame(&inp);
        assert_eq!(g.rule_state(0), RuleState::Violated);
        assert_eq!(g.violation_count(0), 1);
    }

    #[test] fn test_rule1_intrusion() {
        let mut g = TemporalLogicGuard::new();
        let mut inp = FrameInput::default(); inp.intrusion_alert = true;
        g.on_frame(&inp);
        assert_eq!(g.rule_state(1), RuleState::Violated);
    }

    #[test] fn test_rule2_person_id() {
        let mut g = TemporalLogicGuard::new();
        let mut inp = FrameInput::default(); inp.person_id_active = true;
        g.on_frame(&inp);
        assert_eq!(g.rule_state(2), RuleState::Violated);
    }

    #[test] fn test_rule3_low_coherence() {
        let mut g = TemporalLogicGuard::new();
        let mut inp = normal(); inp.coherence = 0.1;
        g.on_frame(&inp);
        assert_eq!(g.rule_state(3), RuleState::Violated);
    }

    #[test] fn test_rule4_motion_stops() {
        let mut g = TemporalLogicGuard::new();
        let mut inp = normal(); inp.motion_energy = 0.5;
        g.on_frame(&inp);
        assert_eq!(g.rule_state(4), RuleState::Pending);
        inp.motion_energy = 0.0; g.on_frame(&inp);
        assert_eq!(g.rule_state(4), RuleState::Satisfied);
    }

    #[test] fn test_rule6_high_hr() {
        let mut g = TemporalLogicGuard::new();
        let mut inp = normal(); inp.heartrate_bpm = 160.0;
        g.on_frame(&inp);
        assert_eq!(g.rule_state(6), RuleState::Violated);
    }

    #[test] fn test_rule7_seizure() {
        let mut g = TemporalLogicGuard::new();
        let mut inp = normal(); inp.seizure_detected = true; inp.normal_gait = false;
        g.on_frame(&inp);
        assert_eq!(g.rule_state(7), RuleState::Pending);
        inp.seizure_detected = false; inp.normal_gait = true;
        g.on_frame(&inp);
        assert_eq!(g.rule_state(7), RuleState::Violated);
        assert_eq!(g.violation_count(7), 1);
    }

    #[test] fn test_recovery() {
        let mut g = TemporalLogicGuard::new();
        let mut inp = FrameInput::default(); inp.fall_alert = true;
        g.on_frame(&inp);
        assert_eq!(g.rule_state(0), RuleState::Violated);
        inp.fall_alert = false; g.on_frame(&inp);
        assert_eq!(g.rule_state(0), RuleState::Satisfied);
    }

    #[test] fn test_periodic_report() {
        let mut g = TemporalLogicGuard::new();
        let mut got = false;
        for _ in 0..g.report_interval + 1 {
            let ev = g.on_frame(&normal());
            for &(et, _) in ev { if et == EVENT_LTL_SATISFACTION { got = true; } }
        }
        assert!(got);
    }
}
