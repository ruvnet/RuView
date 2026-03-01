//! ReasoningBank - Adaptive Learning for Temporal Reasoning
//!
//! Implements trajectory tracking, verdict judgment, and strategy optimization
//! based on the lean-agentic design pattern.
//!
//! Key components:
//! - Trajectory tracking for solution attempts
//! - Verdict judgment (success/failure/suboptimal)
//! - Pattern learning from successful solutions
//! - Strategy optimization based on historical performance
//! - Confidence calibration from feedback

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Verdict for a solution trajectory
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Verdict {
    /// Solution was correct
    Success,
    /// Solution was acceptable but not optimal
    Acceptable,
    /// Solution was wrong but close
    Suboptimal { reason: String, delta: f64 },
    /// Low confidence in solution
    LowConfidence,
    /// Complete failure
    Failed,
}

impl Verdict {
    pub fn is_success(&self) -> bool {
        matches!(self, Verdict::Success | Verdict::Acceptable)
    }

    pub fn score(&self) -> f64 {
        match self {
            Verdict::Success => 1.0,
            Verdict::Acceptable => 0.8,
            Verdict::Suboptimal { delta, .. } => 0.5 - delta.min(0.3),
            Verdict::LowConfidence => 0.3,
            Verdict::Failed => 0.0,
        }
    }
}

/// A single solution attempt
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolutionAttempt {
    /// The proposed solution
    pub solution: String,
    /// Confidence in this solution
    pub confidence: f64,
    /// Steps taken to reach this solution
    pub steps: usize,
    /// Tool calls made
    pub tool_calls: usize,
    /// Strategy used
    pub strategy: String,
    /// Skip mode used (witness for policy audit: "none", "weekday", "hybrid")
    pub skip_mode: String,
    /// Context bucket key (witness for policy audit: "range:distractor")
    pub context_bucket: String,
}

/// Trajectory tracking for a single puzzle
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trajectory {
    /// Puzzle identifier
    pub puzzle_id: String,
    /// Puzzle difficulty
    pub difficulty: u8,
    /// Constraint types encountered
    pub constraint_types: Vec<String>,
    /// Solution attempts made
    pub attempts: Vec<SolutionAttempt>,
    /// Final verdict
    pub verdict: Option<Verdict>,
    /// Correct solution (if known)
    pub correct_solution: Option<String>,
    /// Time taken in ms
    pub latency_ms: u64,
}

impl Trajectory {
    pub fn new(puzzle_id: &str, difficulty: u8) -> Self {
        Self {
            puzzle_id: puzzle_id.to_string(),
            difficulty,
            constraint_types: Vec::new(),
            attempts: Vec::new(),
            verdict: None,
            correct_solution: None,
            latency_ms: 0,
        }
    }

    pub fn record_attempt(
        &mut self,
        solution: String,
        confidence: f64,
        steps: usize,
        tool_calls: usize,
        strategy: &str,
    ) {
        self.attempts.push(SolutionAttempt {
            solution,
            confidence,
            steps,
            tool_calls,
            strategy: strategy.to_string(),
            skip_mode: String::new(),
            context_bucket: String::new(),
        });
    }

    /// Record attempt with full policy witness (skip_mode + context_bucket).
    pub fn record_attempt_witnessed(
        &mut self,
        solution: String,
        confidence: f64,
        steps: usize,
        tool_calls: usize,
        strategy: &str,
        skip_mode: &str,
        context_bucket: &str,
    ) {
        self.attempts.push(SolutionAttempt {
            solution,
            confidence,
            steps,
            tool_calls,
            strategy: strategy.to_string(),
            skip_mode: skip_mode.to_string(),
            context_bucket: context_bucket.to_string(),
        });
    }

    pub fn set_verdict(&mut self, verdict: Verdict, correct: Option<String>) {
        self.verdict = Some(verdict);
        self.correct_solution = correct;
    }
}

/// Memory class for a learned pattern.
///
/// Patterns flow: Volatile → Trusted (requires counterexample) → Quarantined (on regression).
/// This three-class system prevents memory poisoning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MemoryClass {
    /// New pattern, low confidence, no external authority.
    /// Cannot influence strategy selection unless no Trusted alternative exists.
    Volatile,
    /// Survived promotion: has enough observations AND at least one counterexample.
    /// Used as primary strategy source.
    Trusted,
    /// Implicated in regressions or contradictions.
    /// Cannot be selected by default; only used in explicit recovery mode.
    Quarantined,
}

/// Learned pattern from successful solutions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearnedPattern {
    /// Constraint type this pattern applies to
    pub constraint_type: String,
    /// Difficulty range
    pub difficulty_range: (u8, u8),
    /// Best strategy for this pattern
    pub best_strategy: String,
    /// Success rate with this pattern
    pub success_rate: f64,
    /// Average steps needed
    pub avg_steps: f64,
    /// Number of observations
    pub observations: usize,
    /// Memory classification (Volatile → Trusted → Quarantined)
    pub memory_class: MemoryClass,
    /// Linked counterexample IDs (required for promotion to Trusted)
    pub counterexample_ids: Vec<String>,
    /// Number of holdout failures since promotion
    pub holdout_failures: usize,
}

/// Strategy configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Strategy {
    /// Strategy name
    pub name: String,
    /// Whether to use calendar rewriting
    pub use_rewriting: bool,
    /// Maximum search steps
    pub max_steps: usize,
    /// Beam width for search
    pub beam_width: usize,
    /// Confidence threshold
    pub confidence_threshold: f64,
}

impl Default for Strategy {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            use_rewriting: true,
            max_steps: 50,
            beam_width: 3,
            confidence_threshold: 0.7,
        }
    }
}

/// Confidence calibration data
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CalibrationData {
    /// Reported confidence -> actual accuracy mapping
    pub calibration_points: Vec<(f64, bool)>,
    /// Calibrated thresholds by difficulty
    pub thresholds: HashMap<u8, f64>,
}

impl CalibrationData {
    /// Record a calibration point
    pub fn record(&mut self, confidence: f64, correct: bool) {
        self.calibration_points.push((confidence, correct));
    }

    /// Get calibrated confidence threshold for a difficulty
    pub fn get_threshold(&self, difficulty: u8) -> f64 {
        self.thresholds.get(&difficulty).copied().unwrap_or(0.7)
    }

    /// Recalibrate thresholds based on observed data
    pub fn recalibrate(&mut self) {
        if self.calibration_points.len() < 20 {
            return;
        }

        // Group by confidence buckets
        let mut buckets: HashMap<u8, (usize, usize)> = HashMap::new();
        for &(conf, correct) in &self.calibration_points {
            let bucket = (conf * 10.0) as u8;
            let entry = buckets.entry(bucket).or_insert((0, 0));
            entry.0 += 1;
            if correct {
                entry.1 += 1;
            }
        }

        // Find threshold where precision >= 0.9
        for bucket in (5..=10).rev() {
            if let Some(&(total, correct)) = buckets.get(&bucket) {
                if total >= 5 {
                    let precision = correct as f64 / total as f64;
                    if precision >= 0.9 {
                        // Use this bucket's lower bound as default threshold
                        let threshold = bucket as f64 / 10.0;
                        for diff in 1..=10 {
                            self.thresholds.insert(diff, threshold);
                        }
                        break;
                    }
                }
            }
        }
    }
}

/// Serializable memory checkpoint for rollback.
/// Captures the bank state at a point in time so bad learning can be undone.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryCheckpoint {
    /// Unique checkpoint ID
    pub id: usize,
    /// Number of trajectories when checkpoint was taken
    pub trajectories_len: usize,
    /// Snapshot of learned patterns
    pub patterns: HashMap<String, Vec<LearnedPattern>>,
    /// Snapshot of strategy stats
    pub strategy_stats: HashMap<String, StrategyStats>,
    /// Snapshot of calibration data
    pub calibration: CalibrationData,
    /// Snapshot of best strategies
    pub best_strategies: HashMap<u8, String>,
}

/// A trajectory held in quarantine pending review.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuarantinedEntry {
    /// The suspicious trajectory
    pub trajectory: Trajectory,
    /// Why it was quarantined
    pub reason: String,
    /// Counterexample IDs that contradict it
    pub counterexample_ids: Vec<String>,
}

/// Counterexample: evidence of a pattern's failure mode.
/// Promotion to Trusted requires at least one of these linked to the pattern.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Counterexample {
    /// Unique ID
    pub id: String,
    /// Constraint category signature (e.g. "d5_c3")
    pub input_signature: String,
    /// Type of failure
    pub failure_type: String,
    /// Seed that reproduces this failure
    pub reproduction_seed: Option<u64>,
    /// Expected outcome
    pub expected: String,
    /// Actual (wrong) outcome
    pub observed: String,
    /// Linked pattern ID
    pub pattern_id: Option<String>,
    /// Grader verdict
    pub verdict: Option<Verdict>,
}

/// Witness of a rollback event — proves rollback happened and what was discarded.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RollbackWitness {
    /// Checkpoint restored to
    pub checkpoint_id: usize,
    /// Number of trajectories discarded
    pub trajectories_discarded: usize,
    /// Patterns that changed
    pub patterns_restored: usize,
    /// Reason for rollback
    pub reason: String,
    /// Accuracy before rollback
    pub pre_accuracy: f64,
    /// Accuracy after rollback
    pub post_accuracy: f64,
}

/// ReasoningBank - Central learning and adaptation system
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReasoningBank {
    /// All recorded trajectories
    pub trajectories: Vec<Trajectory>,
    /// Learned patterns by constraint type
    pub patterns: HashMap<String, Vec<LearnedPattern>>,
    /// Strategy performance by name
    pub strategy_stats: HashMap<String, StrategyStats>,
    /// Confidence calibration data
    pub calibration: CalibrationData,
    /// Current best strategy by difficulty
    pub best_strategies: HashMap<u8, String>,
    /// Checkpoint stack for rollback (newest last)
    pub checkpoints: Vec<MemoryCheckpoint>,
    /// Quarantine pool: trajectories under review
    pub quarantine: Vec<QuarantinedEntry>,
    /// Counterexamples per constraint type (legacy: raw trajectories)
    pub counterexamples: HashMap<String, Vec<Trajectory>>,
    /// Structured counterexamples with full evidence chain
    pub structured_counterexamples: Vec<Counterexample>,
    /// Rollback witness chain — proof that rollbacks occurred
    pub rollback_witnesses: Vec<RollbackWitness>,
    /// Minimum observations to promote a pattern (evidence threshold)
    pub evidence_threshold: usize,
    /// Next checkpoint ID
    checkpoint_counter: usize,
    /// Next counterexample ID
    counterexample_counter: usize,
    /// Pattern index for O(1) lookups: (constraint_type, difficulty) -> pattern_idx
    #[serde(skip)]
    pattern_index: HashMap<(String, u8), usize>,
    /// Constraint type frequency for prioritization
    #[serde(skip)]
    constraint_frequency: HashMap<String, usize>,
}

impl Default for ReasoningBank {
    fn default() -> Self {
        Self {
            trajectories: Vec::new(),
            patterns: HashMap::new(),
            strategy_stats: HashMap::new(),
            calibration: CalibrationData::default(),
            best_strategies: HashMap::new(),
            checkpoints: Vec::new(),
            quarantine: Vec::new(),
            counterexamples: HashMap::new(),
            structured_counterexamples: Vec::new(),
            rollback_witnesses: Vec::new(),
            evidence_threshold: 5,
            checkpoint_counter: 0,
            counterexample_counter: 0,
            pattern_index: HashMap::new(),
            constraint_frequency: HashMap::new(),
        }
    }
}

/// Statistics for a strategy
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StrategyStats {
    pub attempts: usize,
    pub successes: usize,
    pub total_steps: usize,
    pub total_latency_ms: u64,
}

impl StrategyStats {
    pub fn success_rate(&self) -> f64 {
        if self.attempts == 0 {
            return 0.5; // Prior
        }
        self.successes as f64 / self.attempts as f64
    }

    pub fn avg_steps(&self) -> f64 {
        if self.attempts == 0 {
            return 50.0;
        }
        self.total_steps as f64 / self.attempts as f64
    }
}

impl ReasoningBank {
    /// Create a new ReasoningBank
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a completed trajectory
    pub fn record_trajectory(&mut self, trajectory: Trajectory) {
        // Update strategy stats
        if let Some(attempt) = trajectory.attempts.first() {
            let stats = self
                .strategy_stats
                .entry(attempt.strategy.clone())
                .or_default();
            stats.attempts += 1;
            stats.total_steps += attempt.steps;
            stats.total_latency_ms += trajectory.latency_ms;

            if trajectory
                .verdict
                .as_ref()
                .map(|v| v.is_success())
                .unwrap_or(false)
            {
                stats.successes += 1;
            }
        }

        // Update calibration
        if let Some(attempt) = trajectory.attempts.first() {
            let correct = trajectory
                .verdict
                .as_ref()
                .map(|v| v.is_success())
                .unwrap_or(false);
            self.calibration.record(attempt.confidence, correct);
        }

        // Learn patterns from successful trajectories
        if trajectory
            .verdict
            .as_ref()
            .map(|v| v.is_success())
            .unwrap_or(false)
        {
            self.learn_from_success(&trajectory);
        }

        // Update best strategies
        self.update_best_strategies();

        // Store trajectory
        self.trajectories.push(trajectory);

        // Recalibrate periodically
        if self.trajectories.len() % 50 == 0 {
            self.calibration.recalibrate();
        }
    }

    /// Learn patterns from a successful trajectory
    fn learn_from_success(&mut self, trajectory: &Trajectory) {
        let attempt = match trajectory.attempts.first() {
            Some(a) => a,
            None => return,
        };

        for constraint_type in &trajectory.constraint_types {
            // Update constraint frequency
            *self
                .constraint_frequency
                .entry(constraint_type.clone())
                .or_insert(0) += 1;

            let patterns = self.patterns.entry(constraint_type.clone()).or_default();

            // Find or create pattern
            let pattern_idx = patterns.iter().position(|p| {
                p.best_strategy == attempt.strategy
                    && trajectory.difficulty >= p.difficulty_range.0
                    && trajectory.difficulty <= p.difficulty_range.1
            });

            if let Some(idx) = pattern_idx {
                // Update existing pattern
                let p = &mut patterns[idx];
                let n = p.observations as f64;
                p.success_rate = (p.success_rate * n + 1.0) / (n + 1.0);
                p.avg_steps = (p.avg_steps * n + attempt.steps as f64) / (n + 1.0);
                p.observations += 1;

                // Update pattern index for fast lookup
                self.pattern_index
                    .insert((constraint_type.clone(), trajectory.difficulty), idx);
            } else {
                // Create new pattern
                let new_idx = patterns.len();
                patterns.push(LearnedPattern {
                    constraint_type: constraint_type.clone(),
                    difficulty_range: (
                        trajectory.difficulty.saturating_sub(2),
                        trajectory.difficulty.saturating_add(2),
                    ),
                    best_strategy: attempt.strategy.clone(),
                    success_rate: 1.0,
                    avg_steps: attempt.steps as f64,
                    observations: 1,
                    memory_class: MemoryClass::Volatile,
                    counterexample_ids: Vec::new(),
                    holdout_failures: 0,
                });

                // Index the new pattern
                for d in trajectory.difficulty.saturating_sub(2)
                    ..=trajectory.difficulty.saturating_add(2)
                {
                    self.pattern_index
                        .insert((constraint_type.clone(), d), new_idx);
                }
            }
        }
    }

    /// Record multiple trajectories in batch (for parallel processing)
    pub fn record_trajectories_batch(&mut self, trajectories: Vec<Trajectory>) {
        for trajectory in trajectories {
            self.record_trajectory(trajectory);
        }
    }

    /// Update best strategies by difficulty.
    /// Excludes strategies whose primary patterns are quarantined.
    fn update_best_strategies(&mut self) {
        // Collect quarantined strategy names
        let quarantined_strategies: std::collections::HashSet<String> = self
            .patterns
            .values()
            .flat_map(|ps| ps.iter())
            .filter(|p| p.memory_class == MemoryClass::Quarantined)
            .map(|p| p.best_strategy.clone())
            .collect();

        for difficulty in 1..=10 {
            let mut best_strategy = "default".to_string();
            let mut best_score = 0.0;

            for (strategy, stats) in &self.strategy_stats {
                // Skip quarantined strategies
                if quarantined_strategies.contains(strategy) {
                    continue;
                }
                // Score = success_rate - penalty for steps
                let score = stats.success_rate() - (stats.avg_steps() / 100.0);
                if score > best_score {
                    best_score = score;
                    best_strategy = strategy.clone();
                }
            }

            self.best_strategies.insert(difficulty, best_strategy);
        }
    }

    /// Get recommended strategy for a puzzle (optimized with index).
    /// Prefers Trusted patterns. Falls back to Volatile. Never uses Quarantined.
    pub fn get_strategy(&self, difficulty: u8, constraint_types: &[String]) -> Strategy {
        // Phase 1: Look for Trusted patterns (fast path via index)
        for ct in constraint_types {
            if let Some(&idx) = self.pattern_index.get(&(ct.clone(), difficulty)) {
                if let Some(patterns) = self.patterns.get(ct) {
                    if let Some(pattern) = patterns.get(idx) {
                        if pattern.memory_class == MemoryClass::Trusted
                            && pattern.success_rate > 0.7
                            && pattern.observations >= 3
                        {
                            return self.strategy_from_name(&pattern.best_strategy, difficulty);
                        }
                    }
                }
            }
        }

        // Phase 2: Linear search for Trusted patterns
        for ct in constraint_types {
            if let Some(patterns) = self.patterns.get(ct) {
                let best = patterns
                    .iter()
                    .filter(|p| {
                        p.memory_class == MemoryClass::Trusted
                            && difficulty >= p.difficulty_range.0
                            && difficulty <= p.difficulty_range.1
                    })
                    .max_by(|a, b| a.success_rate.partial_cmp(&b.success_rate).unwrap());

                if let Some(pattern) = best {
                    if pattern.success_rate > 0.7 && pattern.observations >= 3 {
                        return self.strategy_from_name(&pattern.best_strategy, difficulty);
                    }
                }
            }
        }

        // Phase 3: Fall back to Volatile patterns (not yet promoted)
        for ct in constraint_types {
            if let Some(patterns) = self.patterns.get(ct) {
                let best = patterns
                    .iter()
                    .filter(|p| {
                        p.memory_class != MemoryClass::Quarantined
                            && difficulty >= p.difficulty_range.0
                            && difficulty <= p.difficulty_range.1
                            && p.success_rate > 0.7
                            && p.observations >= 3
                    })
                    .max_by(|a, b| a.success_rate.partial_cmp(&b.success_rate).unwrap());

                if let Some(pattern) = best {
                    return self.strategy_from_name(&pattern.best_strategy, difficulty);
                }
            }
        }

        // Phase 4: Fall back to global best strategy for difficulty
        let strategy_name = self
            .best_strategies
            .get(&difficulty)
            .cloned()
            .unwrap_or_else(|| "default".to_string());

        self.strategy_from_name(&strategy_name, difficulty)
    }

    pub fn strategy_from_name(&self, name: &str, difficulty: u8) -> Strategy {
        match name {
            "aggressive" => Strategy {
                name: "aggressive".to_string(),
                use_rewriting: true,
                max_steps: 30,
                beam_width: 5,
                confidence_threshold: 0.6,
            },
            "conservative" => Strategy {
                name: "conservative".to_string(),
                use_rewriting: true,
                max_steps: 100,
                beam_width: 2,
                confidence_threshold: 0.85,
            },
            "adaptive" => Strategy {
                name: "adaptive".to_string(),
                use_rewriting: true,
                max_steps: 50 + (difficulty as usize * 5),
                beam_width: 3,
                confidence_threshold: self.calibration.get_threshold(difficulty),
            },
            _ => Strategy::default(),
        }
    }

    /// Get hints for a puzzle based on learned patterns
    pub fn get_hints(&self, constraint_types: &[String]) -> Vec<String> {
        let mut hints = Vec::new();

        for ct in constraint_types {
            if let Some(patterns) = self.patterns.get(ct) {
                for pattern in patterns.iter().filter(|p| p.observations >= 5) {
                    hints.push(format!(
                        "For {} constraints, {} strategy has {:.0}% success",
                        ct,
                        pattern.best_strategy,
                        pattern.success_rate * 100.0
                    ));
                }
            }
        }

        hints
    }

    /// Calculate learning progress metrics
    pub fn learning_progress(&self) -> LearningProgress {
        let total = self.trajectories.len();
        if total == 0 {
            return LearningProgress::default();
        }

        let successes = self
            .trajectories
            .iter()
            .filter(|t| t.verdict.as_ref().map(|v| v.is_success()).unwrap_or(false))
            .count();

        // Calculate improvement over time (compare first half vs second half)
        let half = total / 2;
        let first_half_success = self.trajectories[..half]
            .iter()
            .filter(|t| t.verdict.as_ref().map(|v| v.is_success()).unwrap_or(false))
            .count() as f64
            / half as f64;

        let second_half_success = self.trajectories[half..]
            .iter()
            .filter(|t| t.verdict.as_ref().map(|v| v.is_success()).unwrap_or(false))
            .count() as f64
            / (total - half) as f64;

        let improvement = second_half_success - first_half_success;

        // Calculate pattern coverage
        let unique_patterns: usize = self.patterns.values().map(|ps| ps.len()).sum();

        LearningProgress {
            total_trajectories: total,
            success_rate: successes as f64 / total as f64,
            improvement_rate: improvement,
            patterns_learned: unique_patterns,
            strategies_tried: self.strategy_stats.len(),
            is_improving: improvement > 0.0,
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Memory Checkpoint & Rollback (Caveat 2: memory poisoning defense)
    // ═══════════════════════════════════════════════════════════════════

    /// Create a checkpoint of the current bank state. Returns the checkpoint ID.
    /// Use `rollback_to(id)` to restore this state if subsequent learning is bad.
    pub fn checkpoint(&mut self) -> usize {
        let id = self.checkpoint_counter;
        self.checkpoint_counter += 1;
        self.checkpoints.push(MemoryCheckpoint {
            id,
            trajectories_len: self.trajectories.len(),
            patterns: self.patterns.clone(),
            strategy_stats: self.strategy_stats.clone(),
            calibration: self.calibration.clone(),
            best_strategies: self.best_strategies.clone(),
        });
        id
    }

    /// Roll back to a previous checkpoint. Returns true if the checkpoint was found.
    /// Discards all learning after the checkpoint: trajectories are truncated,
    /// patterns/strategies/calibration are restored from the snapshot.
    pub fn rollback_to(&mut self, checkpoint_id: usize) -> bool {
        let pos = self.checkpoints.iter().position(|c| c.id == checkpoint_id);
        if let Some(idx) = pos {
            let cp = self.checkpoints[idx].clone();
            // Truncate trajectories to checkpoint length
            self.trajectories.truncate(cp.trajectories_len);
            // Restore learned state
            self.patterns = cp.patterns;
            self.strategy_stats = cp.strategy_stats;
            self.calibration = cp.calibration;
            self.best_strategies = cp.best_strategies;
            // Discard checkpoints after this one
            self.checkpoints.truncate(idx + 1);
            // Rebuild index
            self.rebuild_pattern_index();
            true
        } else {
            false
        }
    }

    /// Number of checkpoints currently stored.
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// Rebuild the pattern index from current patterns (called after rollback).
    fn rebuild_pattern_index(&mut self) {
        self.pattern_index.clear();
        for (ct, patterns) in &self.patterns {
            for (idx, pattern) in patterns.iter().enumerate() {
                for d in pattern.difficulty_range.0..=pattern.difficulty_range.1 {
                    self.pattern_index.insert((ct.clone(), d), idx);
                }
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // Quarantine & Counterexamples (evidence binding)
    // ═══════════════════════════════════════════════════════════════════

    /// Send a trajectory to quarantine instead of learning from it.
    /// Quarantined trajectories do NOT update patterns or strategies.
    pub fn quarantine_trajectory(&mut self, trajectory: Trajectory, reason: &str) {
        self.quarantine.push(QuarantinedEntry {
            trajectory,
            reason: reason.to_string(),
            counterexample_ids: Vec::new(),
        });
    }

    /// Record a counterexample: a trajectory that contradicts an existing pattern.
    /// If counterexamples for a constraint type exceed the threshold, the pattern
    /// is demoted (success_rate reduced, observations reset).
    pub fn record_counterexample(&mut self, constraint_type: &str, trajectory: Trajectory) {
        let examples = self
            .counterexamples
            .entry(constraint_type.to_string())
            .or_default();
        examples.push(trajectory);

        // Check if counterexamples exceed threshold → demote pattern
        let threshold = self.evidence_threshold;
        if examples.len() >= threshold {
            if let Some(patterns) = self.patterns.get_mut(constraint_type) {
                for pattern in patterns.iter_mut() {
                    if pattern.observations < threshold {
                        // Weak pattern contradicted by strong evidence → demote
                        pattern.success_rate *= 0.5;
                        pattern.observations = 0;
                    }
                }
            }
        }
    }

    /// Check if a pattern has enough evidence for promotion to Trusted.
    /// Requires: >= evidence_threshold observations, at least 1 counterexample linked,
    /// more observations than counterexamples, success_rate > 0.7.
    pub fn is_pattern_promoted(&self, constraint_type: &str, difficulty: u8) -> bool {
        let counter_count = self
            .counterexamples
            .get(constraint_type)
            .map(|v| v.len())
            .unwrap_or(0);

        if let Some(patterns) = self.patterns.get(constraint_type) {
            for pattern in patterns {
                if difficulty >= pattern.difficulty_range.0
                    && difficulty <= pattern.difficulty_range.1
                    && pattern.observations >= self.evidence_threshold
                    && pattern.observations > counter_count
                    && pattern.success_rate > 0.7
                {
                    return true;
                }
            }
        }
        false
    }

    /// Promote eligible Volatile patterns to Trusted.
    /// A pattern can only be promoted if it has at least one counterexample entry.
    /// This is the "counterexample-first promotion" rule.
    pub fn promote_patterns(&mut self) -> usize {
        let mut promoted = 0;
        let threshold = self.evidence_threshold;

        for (ct, patterns) in self.patterns.iter_mut() {
            let counter_count = self.counterexamples.get(ct).map(|v| v.len()).unwrap_or(0);
            // Counterexample-first: must have at least 1 counterexample
            let has_counterexample = counter_count > 0;

            for pattern in patterns.iter_mut() {
                if pattern.memory_class == MemoryClass::Volatile
                    && pattern.observations >= threshold
                    && pattern.success_rate > 0.7
                    && has_counterexample
                    && pattern.observations > counter_count
                {
                    pattern.memory_class = MemoryClass::Trusted;
                    promoted += 1;
                }
            }
        }
        promoted
    }

    /// Demote a Trusted pattern to Quarantined.
    /// Called when a Trusted pattern causes a holdout failure.
    /// Automatically refreshes best_strategies to exclude the quarantined pattern.
    pub fn demote_to_quarantined(&mut self, constraint_type: &str, difficulty: u8) -> bool {
        let mut found = false;
        if let Some(patterns) = self.patterns.get_mut(constraint_type) {
            for pattern in patterns.iter_mut() {
                if pattern.memory_class == MemoryClass::Trusted
                    && difficulty >= pattern.difficulty_range.0
                    && difficulty <= pattern.difficulty_range.1
                {
                    pattern.memory_class = MemoryClass::Quarantined;
                    pattern.holdout_failures += 1;
                    found = true;
                }
            }
        }
        if found {
            self.update_best_strategies();
        }
        found
    }

    /// Record a structured counterexample with full evidence chain.
    pub fn record_structured_counterexample(&mut self, ce: Counterexample) -> String {
        let id = format!("ce_{}", self.counterexample_counter);
        self.counterexample_counter += 1;
        let mut ce = ce;
        ce.id = id.clone();

        // Link to pattern if pattern_id provided
        if let Some(ref pid) = ce.pattern_id {
            // Find and link
            for patterns in self.patterns.values_mut() {
                for pattern in patterns.iter_mut() {
                    if pattern.constraint_type == *pid
                        || format!("{}_{}", pattern.constraint_type, pattern.best_strategy) == *pid
                    {
                        pattern.counterexample_ids.push(id.clone());
                    }
                }
            }
        }

        self.structured_counterexamples.push(ce);
        id
    }

    /// Record a rollback with a full witness.
    pub fn rollback_with_witness(
        &mut self,
        checkpoint_id: usize,
        reason: &str,
        pre_accuracy: f64,
        post_accuracy: f64,
    ) -> bool {
        let pre_traj_len = self.trajectories.len();
        let pre_pattern_count: usize = self.patterns.values().map(|v| v.len()).sum();
        let ok = self.rollback_to(checkpoint_id);
        if ok {
            let post_traj_len = self.trajectories.len();
            let post_pattern_count: usize = self.patterns.values().map(|v| v.len()).sum();
            self.rollback_witnesses.push(RollbackWitness {
                checkpoint_id,
                trajectories_discarded: pre_traj_len.saturating_sub(post_traj_len),
                patterns_restored: pre_pattern_count.saturating_sub(post_pattern_count),
                reason: reason.to_string(),
                pre_accuracy,
                post_accuracy,
            });
        }
        ok
    }

    /// Count of Volatile patterns.
    pub fn volatile_count(&self) -> usize {
        self.patterns
            .values()
            .flat_map(|ps| ps.iter())
            .filter(|p| p.memory_class == MemoryClass::Volatile)
            .count()
    }

    /// Count of Trusted patterns.
    pub fn trusted_count(&self) -> usize {
        self.patterns
            .values()
            .flat_map(|ps| ps.iter())
            .filter(|p| p.memory_class == MemoryClass::Trusted)
            .count()
    }

    /// Count of Quarantined patterns.
    pub fn quarantined_pattern_count(&self) -> usize {
        self.patterns
            .values()
            .flat_map(|ps| ps.iter())
            .filter(|p| p.memory_class == MemoryClass::Quarantined)
            .count()
    }

    /// Record a trajectory with quarantine gating: if the trajectory is
    /// solved-but-wrong (contradiction), quarantine it instead of learning.
    /// Otherwise, record normally.
    pub fn record_trajectory_gated(&mut self, trajectory: Trajectory) {
        let is_contradiction = trajectory
            .verdict
            .as_ref()
            .map(|v| !v.is_success())
            .unwrap_or(true)
            && trajectory
                .attempts
                .iter()
                .any(|a| !a.solution.is_empty() && a.solution != "none");

        if is_contradiction {
            // Quarantine: record as counterexample but don't learn from it
            for ct in &trajectory.constraint_types {
                self.record_counterexample(ct, trajectory.clone());
            }
            self.quarantine_trajectory(trajectory, "contradiction: solved but incorrect");
        } else {
            self.record_trajectory(trajectory);
        }
    }

    /// Quarantine pool size.
    pub fn quarantine_len(&self) -> usize {
        self.quarantine.len()
    }

    /// Total counterexamples across all constraint types.
    pub fn counterexample_count(&self) -> usize {
        self.counterexamples.values().map(|v| v.len()).sum()
    }
}

/// Learning progress summary
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LearningProgress {
    pub total_trajectories: usize,
    pub success_rate: f64,
    pub improvement_rate: f64,
    pub patterns_learned: usize,
    pub strategies_tried: usize,
    pub is_improving: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reasoning_bank_learning() {
        let mut bank = ReasoningBank::new();

        // Record some successful trajectories
        for i in 0..10 {
            let mut traj = Trajectory::new(&format!("puzzle_{}", i), 5);
            traj.constraint_types.push("RelativeDate".to_string());
            traj.record_attempt("2024-01-15".to_string(), 0.8, 20, 5, "adaptive");
            traj.set_verdict(Verdict::Success, Some("2024-01-15".to_string()));
            traj.latency_ms = 100;
            bank.record_trajectory(traj);
        }

        // Should have learned patterns
        assert!(bank.patterns.contains_key("RelativeDate"));
        assert!(bank.strategy_stats.contains_key("adaptive"));

        let stats = &bank.strategy_stats["adaptive"];
        assert_eq!(stats.successes, 10);
        assert!(stats.success_rate() > 0.9);
    }

    #[test]
    fn test_strategy_selection() {
        let mut bank = ReasoningBank::new();

        // Train on aggressive strategy for easy puzzles
        for i in 0..20 {
            let mut traj = Trajectory::new(&format!("easy_{}", i), 3);
            traj.constraint_types.push("Before".to_string());
            traj.record_attempt("2024-01-10".to_string(), 0.9, 10, 2, "aggressive");
            traj.set_verdict(Verdict::Success, Some("2024-01-10".to_string()));
            bank.record_trajectory(traj);
        }

        // Should recommend aggressive for easy puzzles
        let strategy = bank.get_strategy(3, &["Before".to_string()]);
        assert_eq!(strategy.name, "aggressive");
    }

    #[test]
    fn test_checkpoint_and_rollback() {
        let mut bank = ReasoningBank::new();

        // Learn 5 trajectories
        for i in 0..5 {
            let mut traj = Trajectory::new(&format!("good_{}", i), 5);
            traj.constraint_types.push("Before".to_string());
            traj.record_attempt("2024-01-10".into(), 0.9, 10, 1, "default");
            traj.set_verdict(Verdict::Success, None);
            bank.record_trajectory(traj);
        }
        assert_eq!(bank.trajectories.len(), 5);

        // Checkpoint
        let cp_id = bank.checkpoint();
        assert_eq!(bank.checkpoint_count(), 1);

        // Learn 5 more (potentially bad)
        for i in 0..5 {
            let mut traj = Trajectory::new(&format!("bad_{}", i), 5);
            traj.constraint_types.push("Before".to_string());
            traj.record_attempt("wrong".into(), 0.1, 50, 1, "default");
            traj.set_verdict(Verdict::Failed, None);
            bank.record_trajectory(traj);
        }
        assert_eq!(bank.trajectories.len(), 10);

        // Rollback to checkpoint
        assert!(bank.rollback_to(cp_id));
        assert_eq!(bank.trajectories.len(), 5);
        // Bad learning should be gone
        assert!(bank
            .trajectories
            .iter()
            .all(|t| t.puzzle_id.starts_with("good_")));
    }

    #[test]
    fn test_quarantine_gating() {
        let mut bank = ReasoningBank::new();

        // Record a contradiction: has a solution but verdict is Failed
        let mut traj = Trajectory::new("contra_1", 5);
        traj.constraint_types.push("InMonth".to_string());
        traj.record_attempt("2024-06-15".into(), 0.5, 20, 1, "default");
        traj.set_verdict(Verdict::Failed, None);
        bank.record_trajectory_gated(traj);

        // Should be quarantined, not in main trajectories
        assert_eq!(bank.quarantine_len(), 1);
        assert_eq!(bank.trajectories.len(), 0);
        assert_eq!(bank.counterexample_count(), 1);
    }

    #[test]
    fn test_evidence_binding() {
        let mut bank = ReasoningBank::new();
        bank.evidence_threshold = 5;

        // Record 3 successes (below threshold)
        for i in 0..3 {
            let mut traj = Trajectory::new(&format!("ev_{}", i), 5);
            traj.constraint_types.push("DayOfWeek".to_string());
            traj.record_attempt("2024-01-15".into(), 0.9, 10, 1, "default");
            traj.set_verdict(Verdict::Success, None);
            bank.record_trajectory(traj);
        }

        // Not yet promoted (only 3 observations, need 5)
        assert!(!bank.is_pattern_promoted("DayOfWeek", 5));

        // Record 3 more (now 6 total, above threshold)
        for i in 3..6 {
            let mut traj = Trajectory::new(&format!("ev_{}", i), 5);
            traj.constraint_types.push("DayOfWeek".to_string());
            traj.record_attempt("2024-01-15".into(), 0.9, 10, 1, "default");
            traj.set_verdict(Verdict::Success, None);
            bank.record_trajectory(traj);
        }

        // Now promoted
        assert!(bank.is_pattern_promoted("DayOfWeek", 5));
    }

    #[test]
    fn test_counterexample_demotion() {
        let mut bank = ReasoningBank::new();
        bank.evidence_threshold = 3;

        // Build a weak pattern (2 observations, below threshold)
        for i in 0..2 {
            let mut traj = Trajectory::new(&format!("weak_{}", i), 5);
            traj.constraint_types.push("Between".to_string());
            traj.record_attempt("2024-01-10".into(), 0.8, 15, 1, "default");
            traj.set_verdict(Verdict::Success, None);
            bank.record_trajectory(traj);
        }

        let orig_rate = bank
            .patterns
            .get("Between")
            .and_then(|ps| ps.first())
            .map(|p| p.success_rate)
            .unwrap_or(0.0);
        assert!(orig_rate > 0.9);

        // Record 3 counterexamples (meets threshold)
        for i in 0..3 {
            let mut traj = Trajectory::new(&format!("counter_{}", i), 5);
            traj.constraint_types.push("Between".to_string());
            traj.record_attempt("wrong".into(), 0.2, 30, 1, "default");
            traj.set_verdict(Verdict::Failed, None);
            bank.record_counterexample("Between", traj);
        }

        // Pattern should be demoted
        let new_rate = bank
            .patterns
            .get("Between")
            .and_then(|ps| ps.first())
            .map(|p| p.success_rate)
            .unwrap_or(1.0);
        assert!(new_rate < orig_rate);
    }

    #[test]
    fn test_three_class_promotion() {
        let mut bank = ReasoningBank::new();
        bank.evidence_threshold = 3;

        // Build a pattern with 5 observations
        for i in 0..5 {
            let mut traj = Trajectory::new(&format!("promo_{}", i), 5);
            traj.constraint_types.push("Month".to_string());
            traj.record_attempt("2024-06-15".into(), 0.9, 10, 1, "adaptive");
            traj.set_verdict(Verdict::Success, None);
            bank.record_trajectory(traj);
        }

        // Pattern starts as Volatile
        assert_eq!(bank.volatile_count(), 1);
        assert_eq!(bank.trusted_count(), 0);

        // Cannot promote without counterexample
        let promoted = bank.promote_patterns();
        assert_eq!(promoted, 0);
        assert_eq!(bank.trusted_count(), 0);

        // Record a counterexample
        let ce_traj = Trajectory::new("fail_1", 5);
        bank.record_counterexample("Month", ce_traj);

        // Now promotion succeeds (has counterexample + enough observations)
        let promoted = bank.promote_patterns();
        assert_eq!(promoted, 1);
        assert_eq!(bank.trusted_count(), 1);
        assert_eq!(bank.volatile_count(), 0);

        // Demote to Quarantined (holdout failure)
        let demoted = bank.demote_to_quarantined("Month", 5);
        assert!(demoted);
        assert_eq!(bank.quarantined_pattern_count(), 1);
        assert_eq!(bank.trusted_count(), 0);

        // Strategy selection should skip Quarantined patterns
        let strategy = bank.get_strategy(5, &["Month".to_string()]);
        assert_ne!(strategy.name, "adaptive"); // Falls back to default
    }

    #[test]
    fn test_rollback_witness() {
        let mut bank = ReasoningBank::new();

        // Record trajectories
        for i in 0..3 {
            let mut traj = Trajectory::new(&format!("w_{}", i), 5);
            traj.constraint_types.push("Year".to_string());
            traj.record_attempt("2024-01-01".into(), 0.9, 10, 1, "default");
            traj.set_verdict(Verdict::Success, None);
            bank.record_trajectory(traj);
        }

        let cp = bank.checkpoint();

        // Record bad trajectories
        for i in 3..6 {
            let mut traj = Trajectory::new(&format!("w_{}", i), 5);
            traj.constraint_types.push("Year".to_string());
            traj.record_attempt("wrong".into(), 0.1, 50, 1, "default");
            traj.set_verdict(Verdict::Failed, None);
            bank.record_trajectory(traj);
        }

        assert_eq!(bank.trajectories.len(), 6);

        // Rollback with witness
        let ok = bank.rollback_with_witness(cp, "accuracy regression", 0.90, 0.50);
        assert!(ok);
        assert_eq!(bank.trajectories.len(), 3);
        assert_eq!(bank.rollback_witnesses.len(), 1);

        let witness = &bank.rollback_witnesses[0];
        assert_eq!(witness.trajectories_discarded, 3);
        assert_eq!(witness.reason, "accuracy regression");
    }

    #[test]
    fn test_structured_counterexample() {
        let mut bank = ReasoningBank::new();

        let ce = Counterexample {
            id: String::new(), // Will be assigned
            input_signature: "d5_c3".to_string(),
            failure_type: "contradiction".to_string(),
            reproduction_seed: Some(42),
            expected: "2024-06-15".to_string(),
            observed: "2024-07-15".to_string(),
            pattern_id: None,
            verdict: Some(Verdict::Failed),
        };

        let id = bank.record_structured_counterexample(ce);
        assert!(id.starts_with("ce_"));
        assert_eq!(bank.structured_counterexamples.len(), 1);
        assert_eq!(bank.structured_counterexamples[0].id, id);
    }
}
