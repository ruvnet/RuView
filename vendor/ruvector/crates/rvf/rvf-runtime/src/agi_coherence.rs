//! Coherence monitoring and container validation pipeline (ADR-036).
//!
//! This module provides two main components:
//!
//! - [`CoherenceMonitor`]: tracks real-time coherence state across events,
//!   contradictions, and task rollbacks, transitioning through [`CoherenceState`]
//!   to gate commits, skill promotion, and autonomous execution.
//!
//! - [`ContainerValidator`]: runs the full validation pipeline over an AGI
//!   container's header, segments, and coherence thresholds, collecting all
//!   errors rather than short-circuiting.

use rvf_types::agi_container::*;

// ---------------------------------------------------------------------------
// CoherenceState
// ---------------------------------------------------------------------------

/// Runtime coherence state derived from threshold checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoherenceState {
    /// All signals within bounds.
    Healthy,
    /// Contradiction rate exceeded -- skill promotion frozen.
    SkillFreeze,
    /// Coherence score below minimum -- commits blocked, repair mode.
    RepairMode,
    /// Rollback ratio exceeded -- execution halted, human review required.
    Halted,
}

// ---------------------------------------------------------------------------
// CoherenceReport
// ---------------------------------------------------------------------------

/// Point-in-time snapshot of all coherence metrics.
#[derive(Clone, Debug)]
pub struct CoherenceReport {
    pub state: CoherenceState,
    pub coherence_score: f32,
    pub contradiction_rate: f32,
    pub rollback_ratio: f32,
    pub total_events: u64,
    pub total_contradictions: u64,
    pub total_tasks: u64,
    pub total_rollbacks: u64,
}

// ---------------------------------------------------------------------------
// CoherenceMonitor
// ---------------------------------------------------------------------------

/// Tracks real-time coherence state and gates system actions.
pub struct CoherenceMonitor {
    thresholds: CoherenceThresholds,
    current_coherence: f32,
    total_events: u64,
    total_contradictions: u64,
    total_tasks: u64,
    total_rollbacks: u64,
    state: CoherenceState,
}

impl CoherenceMonitor {
    /// Create a new monitor with the given thresholds. Returns an error if the
    /// thresholds are out of valid ranges.
    pub fn new(thresholds: CoherenceThresholds) -> Result<Self, ContainerError> {
        thresholds.validate()?;
        Ok(Self {
            thresholds,
            current_coherence: 1.0,
            total_events: 0,
            total_contradictions: 0,
            total_tasks: 0,
            total_rollbacks: 0,
            state: CoherenceState::Healthy,
        })
    }

    /// Create a monitor with [`CoherenceThresholds::DEFAULT`].
    pub fn with_defaults() -> Self {
        Self {
            thresholds: CoherenceThresholds::DEFAULT,
            current_coherence: 1.0,
            total_events: 0,
            total_contradictions: 0,
            total_tasks: 0,
            total_rollbacks: 0,
            state: CoherenceState::Healthy,
        }
    }

    /// Update the current coherence score and re-evaluate state.
    pub fn update_coherence(&mut self, score: f32) {
        self.current_coherence = score;
        self.recompute_state();
    }

    /// Record a generic event (increments event counter).
    pub fn record_event(&mut self) {
        self.total_events = self.total_events.saturating_add(1);
    }

    /// Record a contradiction event and re-evaluate state.
    pub fn record_contradiction(&mut self) {
        self.total_contradictions = self.total_contradictions.saturating_add(1);
        self.recompute_state();
    }

    /// Record a task completion. If `rolled_back` is true the rollback counter
    /// is also incremented. State is re-evaluated afterward.
    pub fn record_task_completion(&mut self, rolled_back: bool) {
        self.total_tasks = self.total_tasks.saturating_add(1);
        if rolled_back {
            self.total_rollbacks = self.total_rollbacks.saturating_add(1);
        }
        self.recompute_state();
    }

    /// Current coherence state.
    pub fn state(&self) -> CoherenceState {
        self.state
    }

    /// Whether the system may commit world-model deltas. True when
    /// [`CoherenceState::Healthy`] or [`CoherenceState::SkillFreeze`].
    pub fn can_commit(&self) -> bool {
        matches!(
            self.state,
            CoherenceState::Healthy | CoherenceState::SkillFreeze
        )
    }

    /// Whether new skills may be promoted. True only when
    /// [`CoherenceState::Healthy`].
    pub fn can_promote_skill(&self) -> bool {
        self.state == CoherenceState::Healthy
    }

    /// Whether the system requires human review. True when
    /// [`CoherenceState::Halted`].
    pub fn requires_human_review(&self) -> bool {
        self.state == CoherenceState::Halted
    }

    /// Contradiction rate: contradictions per 100 events.
    /// Returns `0.0` when there are no events.
    pub fn contradiction_rate(&self) -> f32 {
        if self.total_events == 0 {
            return 0.0;
        }
        (self.total_contradictions as f32 / self.total_events as f32) * 100.0
    }

    /// Rollback ratio: rollbacks / total tasks.
    /// Returns `0.0` when there are no tasks.
    pub fn rollback_ratio(&self) -> f32 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        self.total_rollbacks as f32 / self.total_tasks as f32
    }

    /// Produce a point-in-time snapshot of all metrics.
    pub fn report(&self) -> CoherenceReport {
        CoherenceReport {
            state: self.state,
            coherence_score: self.current_coherence,
            contradiction_rate: self.contradiction_rate(),
            rollback_ratio: self.rollback_ratio(),
            total_events: self.total_events,
            total_contradictions: self.total_contradictions,
            total_tasks: self.total_tasks,
            total_rollbacks: self.total_rollbacks,
        }
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    /// Determine the worst applicable state from the current metrics.
    /// Priority (most severe first): Halted > RepairMode > SkillFreeze > Healthy.
    fn recompute_state(&mut self) {
        if self.rollback_ratio() > self.thresholds.max_rollback_ratio {
            self.state = CoherenceState::Halted;
        } else if self.current_coherence < self.thresholds.min_coherence_score {
            self.state = CoherenceState::RepairMode;
        } else if self.contradiction_rate() > self.thresholds.max_contradiction_rate {
            self.state = CoherenceState::SkillFreeze;
        } else {
            self.state = CoherenceState::Healthy;
        }
    }
}

// ---------------------------------------------------------------------------
// ContainerValidator
// ---------------------------------------------------------------------------

/// Full validation pipeline for an AGI container.
pub struct ContainerValidator {
    mode: ExecutionMode,
}

impl ContainerValidator {
    /// Create a validator for the given execution mode.
    pub fn new(mode: ExecutionMode) -> Self {
        Self { mode }
    }

    /// Validate container segments against mode requirements.
    pub fn validate_segments(&self, segments: &ContainerSegments) -> Result<(), ContainerError> {
        segments.validate(self.mode)
    }

    /// Validate the AGI container header.
    ///
    /// Checks: magic bytes, version (must be 1), and flag consistency --
    /// replay-capable containers must not claim Live-only features without
    /// the kernel flag.
    pub fn validate_header(&self, header: &AgiContainerHeader) -> Result<(), ContainerError> {
        if !header.is_valid_magic() {
            return Err(ContainerError::InvalidConfig("bad magic bytes"));
        }
        if header.version == 0 || header.version > 1 {
            return Err(ContainerError::InvalidConfig("unsupported header version"));
        }
        // Flag consistency: if REPLAY_CAPABLE is set the container should
        // also have the witness flag, since replays depend on witness chains.
        if header.is_replay_capable() && (header.flags & AGI_HAS_WITNESS == 0) {
            return Err(ContainerError::InvalidConfig(
                "replay-capable flag requires witness flag",
            ));
        }
        Ok(())
    }

    /// Validate coherence threshold ranges.
    pub fn validate_coherence(
        &self,
        thresholds: &CoherenceThresholds,
    ) -> Result<(), ContainerError> {
        thresholds.validate()
    }

    /// Run all validations, collecting every error rather than
    /// short-circuiting on the first failure.
    pub fn validate_full(
        &self,
        header: &AgiContainerHeader,
        segments: &ContainerSegments,
        thresholds: &CoherenceThresholds,
    ) -> Vec<ContainerError> {
        let mut errors = Vec::new();

        if let Err(e) = self.validate_header(header) {
            errors.push(e);
        }
        if let Err(e) = self.validate_segments(segments) {
            errors.push(e);
        }
        if let Err(e) = self.validate_coherence(thresholds) {
            errors.push(e);
        }

        errors
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helpers --

    fn valid_header() -> AgiContainerHeader {
        AgiContainerHeader {
            magic: AGI_MAGIC,
            version: 1,
            flags: AGI_HAS_KERNEL | AGI_HAS_WITNESS | AGI_REPLAY_CAPABLE,
            container_id: [0x01; 16],
            build_id: [0x02; 16],
            created_ns: 1_700_000_000_000_000_000,
            model_id_hash: [0; 8],
            policy_hash: [0; 8],
        }
    }

    fn valid_segments() -> ContainerSegments {
        ContainerSegments {
            manifest_present: true,
            kernel_present: true,
            world_model_present: true,
            ..Default::default()
        }
    }

    // -----------------------------------------------------------------------
    // CoherenceMonitor — state transitions
    // -----------------------------------------------------------------------

    #[test]
    fn monitor_starts_healthy() {
        let m = CoherenceMonitor::with_defaults();
        assert_eq!(m.state(), CoherenceState::Healthy);
    }

    #[test]
    fn monitor_healthy_to_skill_freeze() {
        let mut m = CoherenceMonitor::with_defaults();
        // Default max_contradiction_rate is 5.0 per 100 events.
        // Record 100 events with 6 contradictions -> rate = 6.0 > 5.0.
        for _ in 0..100 {
            m.record_event();
        }
        for _ in 0..6 {
            m.record_contradiction();
        }
        assert_eq!(m.state(), CoherenceState::SkillFreeze);
    }

    #[test]
    fn monitor_healthy_to_repair_mode() {
        let mut m = CoherenceMonitor::with_defaults();
        // Default min_coherence_score is 0.70.
        m.update_coherence(0.50);
        assert_eq!(m.state(), CoherenceState::RepairMode);
    }

    #[test]
    fn monitor_healthy_to_halted() {
        let mut m = CoherenceMonitor::with_defaults();
        // Default max_rollback_ratio is 0.20.
        // 10 tasks, 3 rolled back -> ratio = 0.30 > 0.20.
        for _ in 0..7 {
            m.record_task_completion(false);
        }
        for _ in 0..3 {
            m.record_task_completion(true);
        }
        assert_eq!(m.state(), CoherenceState::Halted);
    }

    #[test]
    fn halted_takes_priority_over_repair_mode() {
        let mut m = CoherenceMonitor::with_defaults();
        // Both rollback ratio and coherence score are bad.
        m.update_coherence(0.50);
        for _ in 0..5 {
            m.record_task_completion(false);
        }
        for _ in 0..5 {
            m.record_task_completion(true);
        }
        // Halted (rollback) is highest severity and wins.
        assert_eq!(m.state(), CoherenceState::Halted);
    }

    #[test]
    fn repair_mode_takes_priority_over_skill_freeze() {
        let mut m = CoherenceMonitor::with_defaults();
        // Both coherence and contradiction rate are bad.
        m.update_coherence(0.50);
        for _ in 0..100 {
            m.record_event();
        }
        for _ in 0..10 {
            m.record_contradiction();
        }
        // RepairMode wins over SkillFreeze.
        assert_eq!(m.state(), CoherenceState::RepairMode);
    }

    #[test]
    fn recovery_back_to_healthy() {
        let mut m = CoherenceMonitor::with_defaults();
        m.update_coherence(0.50);
        assert_eq!(m.state(), CoherenceState::RepairMode);
        m.update_coherence(0.90);
        assert_eq!(m.state(), CoherenceState::Healthy);
    }

    // -----------------------------------------------------------------------
    // CoherenceMonitor — gate queries
    // -----------------------------------------------------------------------

    #[test]
    fn can_commit_when_healthy() {
        let m = CoherenceMonitor::with_defaults();
        assert!(m.can_commit());
    }

    #[test]
    fn can_commit_when_skill_freeze() {
        let mut m = CoherenceMonitor::with_defaults();
        for _ in 0..100 {
            m.record_event();
        }
        for _ in 0..6 {
            m.record_contradiction();
        }
        assert_eq!(m.state(), CoherenceState::SkillFreeze);
        assert!(m.can_commit());
    }

    #[test]
    fn cannot_commit_when_repair_mode() {
        let mut m = CoherenceMonitor::with_defaults();
        m.update_coherence(0.50);
        assert!(!m.can_commit());
    }

    #[test]
    fn cannot_commit_when_halted() {
        let mut m = CoherenceMonitor::with_defaults();
        m.record_task_completion(true); // 1 task, 1 rollback -> ratio = 1.0
        assert!(!m.can_commit());
    }

    #[test]
    fn can_promote_skill_only_when_healthy() {
        let m = CoherenceMonitor::with_defaults();
        assert!(m.can_promote_skill());

        let mut m2 = CoherenceMonitor::with_defaults();
        for _ in 0..100 {
            m2.record_event();
        }
        for _ in 0..6 {
            m2.record_contradiction();
        }
        assert_eq!(m2.state(), CoherenceState::SkillFreeze);
        assert!(!m2.can_promote_skill());
    }

    #[test]
    fn requires_human_review_only_when_halted() {
        let m = CoherenceMonitor::with_defaults();
        assert!(!m.requires_human_review());

        let mut m2 = CoherenceMonitor::with_defaults();
        m2.record_task_completion(true);
        assert_eq!(m2.state(), CoherenceState::Halted);
        assert!(m2.requires_human_review());
    }

    // -----------------------------------------------------------------------
    // CoherenceMonitor — rate calculations
    // -----------------------------------------------------------------------

    #[test]
    fn contradiction_rate_calculation() {
        let mut m = CoherenceMonitor::with_defaults();
        for _ in 0..200 {
            m.record_event();
        }
        for _ in 0..4 {
            m.record_contradiction();
        }
        // 4 contradictions in (200 events + 4 contradiction events via record_contradiction)
        // record_contradiction does not call record_event, so total_events = 200.
        // Rate = (4 / 200) * 100 = 2.0.
        assert!((m.contradiction_rate() - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rollback_ratio_calculation() {
        let mut m = CoherenceMonitor::with_defaults();
        for _ in 0..8 {
            m.record_task_completion(false);
        }
        for _ in 0..2 {
            m.record_task_completion(true);
        }
        // 2 rollbacks / 10 tasks = 0.20.
        assert!((m.rollback_ratio() - 0.20).abs() < f32::EPSILON);
    }

    // -----------------------------------------------------------------------
    // CoherenceMonitor — edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn contradiction_rate_zero_events() {
        let m = CoherenceMonitor::with_defaults();
        assert!((m.contradiction_rate() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rollback_ratio_zero_tasks() {
        let m = CoherenceMonitor::with_defaults();
        assert!((m.rollback_ratio() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn new_with_invalid_thresholds() {
        let bad = CoherenceThresholds {
            min_coherence_score: 2.0,
            ..CoherenceThresholds::DEFAULT
        };
        assert!(CoherenceMonitor::new(bad).is_err());
    }

    #[test]
    fn new_with_valid_thresholds() {
        let m = CoherenceMonitor::new(CoherenceThresholds::STRICT).unwrap();
        assert_eq!(m.state(), CoherenceState::Healthy);
    }

    #[test]
    fn report_snapshot() {
        let mut m = CoherenceMonitor::with_defaults();
        m.update_coherence(0.85);
        for _ in 0..50 {
            m.record_event();
        }
        m.record_contradiction();
        // 9 successful + 1 rollback = ratio 0.10, within the 0.20 threshold.
        for _ in 0..9 {
            m.record_task_completion(false);
        }
        m.record_task_completion(true);

        let r = m.report();
        assert_eq!(r.state, CoherenceState::Healthy);
        assert!((r.coherence_score - 0.85).abs() < f32::EPSILON);
        assert_eq!(r.total_events, 50);
        assert_eq!(r.total_contradictions, 1);
        assert_eq!(r.total_tasks, 10);
        assert_eq!(r.total_rollbacks, 1);
    }

    // -----------------------------------------------------------------------
    // ContainerValidator — validate_segments
    // -----------------------------------------------------------------------

    #[test]
    fn validator_segments_delegates_ok() {
        let v = ContainerValidator::new(ExecutionMode::Live);
        let segs = valid_segments();
        assert!(v.validate_segments(&segs).is_ok());
    }

    #[test]
    fn validator_segments_delegates_error() {
        let v = ContainerValidator::new(ExecutionMode::Replay);
        let segs = ContainerSegments {
            manifest_present: true,
            witness_count: 0,
            ..Default::default()
        };
        assert!(v.validate_segments(&segs).is_err());
    }

    // -----------------------------------------------------------------------
    // ContainerValidator — validate_header
    // -----------------------------------------------------------------------

    #[test]
    fn validator_header_ok() {
        let v = ContainerValidator::new(ExecutionMode::Live);
        assert!(v.validate_header(&valid_header()).is_ok());
    }

    #[test]
    fn validator_header_bad_magic() {
        let v = ContainerValidator::new(ExecutionMode::Live);
        let mut h = valid_header();
        h.magic = 0xDEADBEEF;
        assert_eq!(
            v.validate_header(&h),
            Err(ContainerError::InvalidConfig("bad magic bytes"))
        );
    }

    #[test]
    fn validator_header_bad_version_zero() {
        let v = ContainerValidator::new(ExecutionMode::Live);
        let mut h = valid_header();
        h.version = 0;
        assert_eq!(
            v.validate_header(&h),
            Err(ContainerError::InvalidConfig("unsupported header version"))
        );
    }

    #[test]
    fn validator_header_bad_version_future() {
        let v = ContainerValidator::new(ExecutionMode::Live);
        let mut h = valid_header();
        h.version = 99;
        assert_eq!(
            v.validate_header(&h),
            Err(ContainerError::InvalidConfig("unsupported header version"))
        );
    }

    #[test]
    fn validator_header_replay_without_witness() {
        let v = ContainerValidator::new(ExecutionMode::Live);
        let h = AgiContainerHeader {
            magic: AGI_MAGIC,
            version: 1,
            flags: AGI_REPLAY_CAPABLE, // missing AGI_HAS_WITNESS
            container_id: [0; 16],
            build_id: [0; 16],
            created_ns: 0,
            model_id_hash: [0; 8],
            policy_hash: [0; 8],
        };
        assert_eq!(
            v.validate_header(&h),
            Err(ContainerError::InvalidConfig(
                "replay-capable flag requires witness flag"
            ))
        );
    }

    // -----------------------------------------------------------------------
    // ContainerValidator — validate_full
    // -----------------------------------------------------------------------

    #[test]
    fn validator_full_all_ok() {
        let v = ContainerValidator::new(ExecutionMode::Live);
        let errs = v.validate_full(
            &valid_header(),
            &valid_segments(),
            &CoherenceThresholds::DEFAULT,
        );
        assert!(errs.is_empty());
    }

    #[test]
    fn validator_full_collects_multiple_errors() {
        let v = ContainerValidator::new(ExecutionMode::Live);

        // Bad header (wrong magic).
        let mut h = valid_header();
        h.magic = 0xBAD0CAFE;

        // Bad segments (no kernel, no wasm, no world model for Live).
        let segs = ContainerSegments {
            manifest_present: true,
            ..Default::default()
        };

        // Bad thresholds.
        let bad_thresh = CoherenceThresholds {
            min_coherence_score: -1.0,
            ..CoherenceThresholds::DEFAULT
        };

        let errs = v.validate_full(&h, &segs, &bad_thresh);
        // Expect at least 3 errors: header, segments, and thresholds.
        assert!(errs.len() >= 3, "expected >= 3 errors, got {}", errs.len());
    }

    #[test]
    fn validator_full_partial_errors() {
        let v = ContainerValidator::new(ExecutionMode::Replay);

        // Good header, bad segments (replay needs witness), good thresholds.
        let segs = ContainerSegments {
            manifest_present: true,
            witness_count: 0,
            ..Default::default()
        };

        let errs = v.validate_full(&valid_header(), &segs, &CoherenceThresholds::DEFAULT);
        assert_eq!(errs.len(), 1);
    }
}
