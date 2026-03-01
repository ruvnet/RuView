//! Meta-Learning Improvements for AGI Learning Architecture
//!
//! Five composable enhancements that layer on top of the existing
//! Thompson Sampling + Population Search + Cost Curve pipeline:
//!
//! 1. **RegretTracker**: Measures optimality gap — cumulative difference
//!    between chosen arms and the best-known arm. You can't improve
//!    what you don't measure.
//!
//! 2. **DecayingBeta**: Beta distribution with exponential forgetting.
//!    Old evidence decays so the system adapts to non-stationary
//!    environments instead of calcifying on stale data.
//!
//! 3. **PlateauDetector**: Detects when learning has stalled by comparing
//!    recent accuracy windows. Triggers strategy changes: more exploration,
//!    cross-domain transfer, or population diversity injection.
//!
//! 4. **ParetoFront**: Multi-objective optimization tracking. Instead of
//!    collapsing accuracy/cost/robustness into one scalar, tracks the full
//!    Pareto front of non-dominated solutions.
//!
//! 5. **CuriosityBonus**: UCB-style exploration bonus for under-visited
//!    context buckets. Directs exploration toward novel contexts rather
//!    than relying solely on Thompson Sampling's implicit exploration.

use crate::cost_curve::CostCurvePoint;
use crate::transfer::{ArmId, BetaParams, ContextBucket};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════
// 1. Regret Tracker
// ═══════════════════════════════════════════════════════════════════

/// Per-bucket regret state: tracks best arm and cumulative regret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketRegret {
    /// Best known arm mean reward.
    pub best_mean: f32,
    /// Which arm is currently best.
    pub best_arm: ArmId,
    /// Cumulative regret: Σ(best_reward - chosen_reward).
    pub cumulative_regret: f64,
    /// Total observations in this bucket.
    pub observations: u64,
    /// Per-cycle regret snapshots for trend analysis.
    pub regret_history: Vec<f64>,
    /// Per-arm running mean for best-arm tracking.
    arm_means: HashMap<ArmId, (f64, u64)>,
}

impl BucketRegret {
    fn new() -> Self {
        Self {
            best_mean: 0.0,
            best_arm: ArmId("unknown".into()),
            cumulative_regret: 0.0,
            observations: 0,
            regret_history: Vec::new(),
            arm_means: HashMap::new(),
        }
    }
}

/// Tracks cumulative regret across all context buckets.
///
/// Regret = Σ(best_arm_mean - chosen_arm_reward) over time.
/// Sublinear regret growth (O(√T)) indicates the system is learning.
/// Linear regret means it's not adapting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegretTracker {
    buckets: HashMap<ContextBucket, BucketRegret>,
    /// Global cumulative regret across all buckets.
    pub total_regret: f64,
    /// Total observations across all buckets.
    pub total_observations: u64,
    /// Snapshot interval: take regret snapshot every N observations.
    snapshot_interval: u64,
}

impl RegretTracker {
    /// Create a new regret tracker.
    pub fn new(snapshot_interval: u64) -> Self {
        Self {
            buckets: HashMap::new(),
            total_regret: 0.0,
            total_observations: 0,
            snapshot_interval: snapshot_interval.max(1),
        }
    }

    /// Record a choice and its reward, updating regret.
    pub fn record(&mut self, bucket: &ContextBucket, arm: &ArmId, reward: f32) {
        // Avoid cloning when entry already exists (hot path optimization).
        if !self.buckets.contains_key(bucket) {
            self.buckets.insert(bucket.clone(), BucketRegret::new());
        }
        let entry = self.buckets.get_mut(bucket).unwrap();

        // Update arm running mean (avoid clone when arm exists).
        if !entry.arm_means.contains_key(arm) {
            entry.arm_means.insert(arm.clone(), (0.0, 0));
        }
        let (sum, count) = entry.arm_means.get_mut(arm).unwrap();
        *sum += reward as f64;
        *count += 1;
        let arm_mean = *sum / *count as f64;

        // Update best arm if this arm's mean exceeds current best
        if arm_mean > entry.best_mean as f64 {
            entry.best_mean = arm_mean as f32;
            entry.best_arm = arm.clone();
        }

        // Instantaneous regret: best_mean - observed_reward
        let instant_regret = (entry.best_mean as f64 - reward as f64).max(0.0);
        entry.cumulative_regret += instant_regret;
        entry.observations += 1;
        self.total_regret += instant_regret;
        self.total_observations += 1;

        // Snapshot on interval
        if entry.observations % self.snapshot_interval == 0 {
            entry.regret_history.push(entry.cumulative_regret);
        }
    }

    /// Regret growth rate for a bucket. Sublinear (< 1.0) means learning.
    ///
    /// Computed as: log(regret) / log(observations).
    /// Perfect learning → 0.5 (O(√T)). No learning → 1.0 (O(T)).
    pub fn regret_growth_rate(&self, bucket: &ContextBucket) -> Option<f32> {
        let entry = self.buckets.get(bucket)?;
        if entry.observations < 10 || entry.cumulative_regret < 1e-10 {
            return None;
        }
        let log_regret = (entry.cumulative_regret).ln();
        let log_t = (entry.observations as f64).ln();
        Some((log_regret / log_t) as f32)
    }

    /// Average regret per observation (lower = better).
    pub fn average_regret(&self) -> f64 {
        if self.total_observations == 0 {
            return 0.0;
        }
        self.total_regret / self.total_observations as f64
    }

    /// Check if learning has converged: regret growth rate < threshold.
    pub fn has_converged(&self, bucket: &ContextBucket, threshold: f32) -> bool {
        self.regret_growth_rate(bucket)
            .map_or(false, |rate| rate < threshold)
    }

    /// Get regret summary for all buckets.
    pub fn summary(&self) -> RegretSummary {
        let bucket_rates: Vec<(ContextBucket, f32)> = self
            .buckets
            .keys()
            .filter_map(|b| self.regret_growth_rate(b).map(|r| (b.clone(), r)))
            .collect();

        let mean_rate = if bucket_rates.is_empty() {
            1.0
        } else {
            bucket_rates.iter().map(|(_, r)| r).sum::<f32>() / bucket_rates.len() as f32
        };

        RegretSummary {
            total_regret: self.total_regret,
            total_observations: self.total_observations,
            average_regret: self.average_regret(),
            mean_growth_rate: mean_rate,
            bucket_count: self.buckets.len(),
            converged_buckets: bucket_rates.iter().filter(|(_, r)| *r < 0.7).count(),
        }
    }
}

/// Summary of regret across all buckets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegretSummary {
    pub total_regret: f64,
    pub total_observations: u64,
    pub average_regret: f64,
    /// Mean regret growth rate across buckets. < 0.7 = sublinear = learning.
    pub mean_growth_rate: f32,
    pub bucket_count: usize,
    /// Buckets where regret growth is sublinear (learning converged).
    pub converged_buckets: usize,
}

// ═══════════════════════════════════════════════════════════════════
// 2. Decaying Beta Distribution
// ═══════════════════════════════════════════════════════════════════

/// Beta distribution with exponential forgetting for non-stationary environments.
///
/// On each update, old evidence decays by `decay_factor` before the new
/// observation is added. This gives recent evidence more weight while
/// gradually forgetting stale data.
///
/// Effective window size ≈ 1 / (1 - decay_factor).
/// decay_factor = 0.995 → window ≈ 200 observations.
/// decay_factor = 0.99  → window ≈ 100 observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayingBeta {
    pub alpha: f32,
    pub beta: f32,
    /// Decay factor per observation. 1.0 = no decay (standard Beta).
    pub decay_factor: f32,
    /// Effective sample size (decayed observation count).
    pub effective_n: f32,
}

impl DecayingBeta {
    /// Create with uniform prior and specified decay.
    pub fn new(decay_factor: f32) -> Self {
        Self {
            alpha: 1.0,
            beta: 1.0,
            decay_factor: decay_factor.clamp(0.9, 1.0),
            effective_n: 0.0,
        }
    }

    /// Create from an existing BetaParams with decay.
    pub fn from_beta(params: &BetaParams, decay_factor: f32) -> Self {
        Self {
            alpha: params.alpha,
            beta: params.beta,
            decay_factor: decay_factor.clamp(0.9, 1.0),
            effective_n: params.alpha + params.beta - 2.0,
        }
    }

    /// Update with exponential decay: old evidence shrinks, new evidence adds.
    pub fn update(&mut self, reward: f32) {
        // Decay existing evidence toward the prior
        self.alpha = 1.0 + (self.alpha - 1.0) * self.decay_factor;
        self.beta = 1.0 + (self.beta - 1.0) * self.decay_factor;

        // Add new observation
        self.alpha += reward;
        self.beta += 1.0 - reward;

        // Track effective sample size
        self.effective_n = self.effective_n * self.decay_factor + 1.0;
    }

    /// Mean of the distribution.
    pub fn mean(&self) -> f32 {
        self.alpha / (self.alpha + self.beta)
    }

    /// Variance of the distribution.
    pub fn variance(&self) -> f32 {
        let total = self.alpha + self.beta;
        (self.alpha * self.beta) / (total * total * (total + 1.0))
    }

    /// Convert back to standard BetaParams (snapshot).
    pub fn to_beta_params(&self) -> BetaParams {
        BetaParams {
            alpha: self.alpha,
            beta: self.beta,
        }
    }

    /// Effective window size: how many recent observations dominate.
    pub fn effective_window(&self) -> f32 {
        if self.decay_factor >= 1.0 {
            self.effective_n
        } else {
            1.0 / (1.0 - self.decay_factor)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// 3. Plateau Detector
// ═══════════════════════════════════════════════════════════════════

/// Detects when learning has stalled by comparing accuracy windows.
///
/// Compares the mean accuracy of the most recent `window_size` points
/// against the prior window. If improvement is below threshold,
/// learning has plateaued.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateauDetector {
    /// Number of points per comparison window.
    pub window_size: usize,
    /// Minimum improvement to not be considered a plateau.
    pub improvement_threshold: f32,
    /// Number of consecutive plateau detections.
    pub consecutive_plateaus: u32,
    /// Total plateaus detected.
    pub total_plateaus: u32,
}

/// What to do when a plateau is detected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlateauAction {
    /// Continue learning, no plateau detected.
    Continue,
    /// Mild plateau: increase exploration budget.
    IncreaseExploration,
    /// Moderate plateau: trigger cross-domain transfer.
    TriggerTransfer,
    /// Severe plateau: inject diversity into population.
    InjectDiversity,
    /// Extreme plateau: reset and restart with new strategy.
    Reset,
}

impl PlateauDetector {
    /// Create a new plateau detector.
    pub fn new(window_size: usize, improvement_threshold: f32) -> Self {
        Self {
            window_size: window_size.max(3),
            improvement_threshold: improvement_threshold.max(0.001),
            consecutive_plateaus: 0,
            total_plateaus: 0,
        }
    }

    /// Check if learning has plateaued and recommend an action.
    pub fn check(&mut self, points: &[CostCurvePoint]) -> PlateauAction {
        if points.len() < self.window_size * 2 {
            self.consecutive_plateaus = 0;
            return PlateauAction::Continue;
        }

        let n = points.len();
        let recent = &points[n - self.window_size..];
        let prior = &points[n - 2 * self.window_size..n - self.window_size];

        let recent_mean = recent.iter().map(|p| p.accuracy).sum::<f32>() / recent.len() as f32;
        let prior_mean = prior.iter().map(|p| p.accuracy).sum::<f32>() / prior.len() as f32;

        let improvement = recent_mean - prior_mean;

        if improvement.abs() < self.improvement_threshold {
            self.consecutive_plateaus += 1;
            self.total_plateaus += 1;

            match self.consecutive_plateaus {
                1 => PlateauAction::IncreaseExploration,
                2..=3 => PlateauAction::TriggerTransfer,
                4..=6 => PlateauAction::InjectDiversity,
                _ => PlateauAction::Reset,
            }
        } else {
            self.consecutive_plateaus = 0;
            PlateauAction::Continue
        }
    }

    /// Check if cost has plateaued (not just accuracy).
    pub fn check_cost(&self, points: &[CostCurvePoint]) -> bool {
        if points.len() < self.window_size * 2 {
            return false;
        }

        let n = points.len();
        let recent = &points[n - self.window_size..];
        let prior = &points[n - 2 * self.window_size..n - self.window_size];

        let recent_cost =
            recent.iter().map(|p| p.cost_per_solve).sum::<f32>() / recent.len() as f32;
        let prior_cost = prior.iter().map(|p| p.cost_per_solve).sum::<f32>() / prior.len() as f32;

        // Cost should be decreasing; if it's not, that's a plateau
        (prior_cost - recent_cost).abs() < self.improvement_threshold
    }

    /// Compute learning velocity: rate of accuracy change per cycle.
    pub fn learning_velocity(&self, points: &[CostCurvePoint]) -> f32 {
        if points.len() < 2 {
            return 0.0;
        }
        let n = points.len();
        let window = self.window_size.min(n);
        let recent = &points[n - window..];

        if recent.len() < 2 {
            return 0.0;
        }

        let first = recent.first().unwrap();
        let last = recent.last().unwrap();
        let dt = (last.cycle - first.cycle).max(1) as f32;

        (last.accuracy - first.accuracy) / dt
    }
}

// ═══════════════════════════════════════════════════════════════════
// 4. Pareto Front (Multi-Objective Optimization)
// ═══════════════════════════════════════════════════════════════════

/// A point in objective space with its kernel identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParetoPoint {
    /// Kernel identifier.
    pub kernel_id: String,
    /// Objective values (higher is better for all).
    /// Convention: [accuracy, -cost, robustness].
    pub objectives: Vec<f32>,
    /// Generation when this point was added.
    pub generation: u32,
}

/// Multi-objective Pareto front tracker.
///
/// Instead of collapsing multiple objectives into one weighted scalar,
/// tracks the full set of non-dominated solutions. A solution is
/// non-dominated if no other solution is better on ALL objectives.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParetoFront {
    /// Current non-dominated solutions.
    pub front: Vec<ParetoPoint>,
    /// Total points evaluated.
    pub evaluated: u64,
    /// Number of front updates (when a new point enters the front).
    pub front_updates: u64,
}

impl ParetoFront {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if point `a` dominates point `b`.
    ///
    /// Dominance: a is at least as good as b on all objectives,
    /// and strictly better on at least one.
    pub fn dominates(a: &[f32], b: &[f32]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut at_least_equal = true;
        let mut strictly_better = false;

        for (ai, bi) in a.iter().zip(b.iter()) {
            if ai < bi {
                at_least_equal = false;
                break;
            }
            if ai > bi {
                strictly_better = true;
            }
        }

        at_least_equal && strictly_better
    }

    /// Insert a point into the front. Returns true if the point is non-dominated.
    ///
    /// Removes any existing points that the new point dominates.
    pub fn insert(&mut self, point: ParetoPoint) -> bool {
        self.evaluated += 1;

        // Check if any existing point dominates the new one
        for existing in &self.front {
            if Self::dominates(&existing.objectives, &point.objectives) {
                return false; // Dominated, don't add
            }
        }

        // Remove points dominated by the new one
        self.front
            .retain(|existing| !Self::dominates(&point.objectives, &existing.objectives));

        self.front.push(point);
        self.front_updates += 1;
        true
    }

    /// Hypervolume indicator: volume of objective space dominated by the front.
    ///
    /// Uses a reference point (all zeros) as the origin.
    /// Higher hypervolume = better front coverage.
    /// Only exact for 2D; uses approximation for higher dimensions.
    pub fn hypervolume(&self, reference: &[f32]) -> f32 {
        if self.front.is_empty() || reference.is_empty() {
            return 0.0;
        }

        let dim = reference.len();
        if dim == 2 {
            self.hypervolume_2d(reference)
        } else {
            // Approximate: sum of per-point dominated rectangles (overcounts overlaps)
            self.front
                .iter()
                .map(|p| {
                    p.objectives
                        .iter()
                        .zip(reference.iter())
                        .map(|(oi, ri)| (oi - ri).max(0.0))
                        .product::<f32>()
                })
                .sum()
        }
    }

    /// Exact 2D hypervolume via sweep line.
    fn hypervolume_2d(&self, reference: &[f32]) -> f32 {
        if self.front.is_empty() {
            return 0.0;
        }

        let mut points: Vec<(f32, f32)> = self
            .front
            .iter()
            .map(|p| {
                let x = p.objectives.first().copied().unwrap_or(0.0);
                let y = p.objectives.get(1).copied().unwrap_or(0.0);
                (x, y)
            })
            .collect();

        // Sort by x descending
        points.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let ref_x = reference.first().copied().unwrap_or(0.0);
        let ref_y = reference.get(1).copied().unwrap_or(0.0);

        let mut volume = 0.0f32;
        let mut prev_y = ref_y;

        for &(x, y) in &points {
            if y > prev_y {
                volume += (x - ref_x) * (y - prev_y);
                prev_y = y;
            }
        }

        volume
    }

    /// Size of the Pareto front.
    pub fn len(&self) -> usize {
        self.front.len()
    }

    /// Whether the front is empty.
    pub fn is_empty(&self) -> bool {
        self.front.is_empty()
    }

    /// Get the front point that maximizes a specific objective.
    pub fn best_on(&self, objective_index: usize) -> Option<&ParetoPoint> {
        self.front.iter().max_by(|a, b| {
            let va = a.objectives.get(objective_index).copied().unwrap_or(0.0);
            let vb = b.objectives.get(objective_index).copied().unwrap_or(0.0);
            va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Spread: range on each objective dimension. Higher = more diverse front.
    pub fn spread(&self) -> Vec<f32> {
        if self.front.is_empty() {
            return Vec::new();
        }
        let dim = self.front[0].objectives.len();
        (0..dim)
            .map(|i| {
                let vals: Vec<f32> = self.front.iter().map(|p| p.objectives[i]).collect();
                let min = vals.iter().cloned().fold(f32::INFINITY, f32::min);
                let max = vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                max - min
            })
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════
// 5. Curiosity Bonus (UCB-style exploration)
// ═══════════════════════════════════════════════════════════════════

/// UCB-style exploration bonus for under-visited context buckets.
///
/// Adds sqrt(2 * ln(N) / n_i) bonus to arm selection, where N is total
/// observations and n_i is observations for this bucket/arm.
/// This prioritizes under-explored contexts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuriosityBonus {
    /// Per-bucket, per-arm visit counts.
    visit_counts: HashMap<ContextBucket, HashMap<ArmId, u64>>,
    /// Total visit count across everything.
    pub total_visits: u64,
    /// Exploration coefficient (higher = more curious).
    pub exploration_coeff: f32,
}

impl CuriosityBonus {
    /// Create with a given exploration coefficient.
    /// Standard UCB uses sqrt(2) ≈ 1.41. Higher = more exploration.
    pub fn new(exploration_coeff: f32) -> Self {
        Self {
            visit_counts: HashMap::new(),
            total_visits: 0,
            exploration_coeff: exploration_coeff.max(0.0),
        }
    }

    /// Record a visit to a bucket/arm.
    pub fn record_visit(&mut self, bucket: &ContextBucket, arm: &ArmId) {
        // Hot path: avoid cloning when entries already exist.
        if let Some(arms) = self.visit_counts.get_mut(bucket) {
            if let Some(count) = arms.get_mut(arm) {
                *count += 1;
            } else {
                arms.insert(arm.clone(), 1);
            }
        } else {
            let mut arms = HashMap::new();
            arms.insert(arm.clone(), 1u64);
            self.visit_counts.insert(bucket.clone(), arms);
        }
        self.total_visits += 1;
    }

    /// Compute the exploration bonus for a bucket/arm combination.
    ///
    /// Bonus = c * sqrt(ln(N) / n), where:
    /// - c is the exploration coefficient
    /// - N is total visits
    /// - n is visits to this specific bucket/arm
    pub fn bonus(&self, bucket: &ContextBucket, arm: &ArmId) -> f32 {
        if self.total_visits < 2 {
            return self.exploration_coeff; // Maximum bonus when no data
        }

        let arm_visits = self
            .visit_counts
            .get(bucket)
            .and_then(|arms| arms.get(arm))
            .copied()
            .unwrap_or(0);

        if arm_visits == 0 {
            return self.exploration_coeff * 2.0; // Never-visited bonus
        }

        let log_n = (self.total_visits as f32).ln();
        self.exploration_coeff * (log_n / arm_visits as f32).sqrt()
    }

    /// Find the most under-explored bucket (lowest total visits).
    pub fn most_curious_bucket(&self) -> Option<&ContextBucket> {
        // Find buckets with fewest total visits
        let mut min_visits = u64::MAX;
        let mut most_curious = None;

        for (bucket, arms) in &self.visit_counts {
            let total: u64 = arms.values().sum();
            if total < min_visits {
                min_visits = total;
                most_curious = Some(bucket);
            }
        }

        most_curious
    }

    /// Novelty score for a bucket: inverse of visit density.
    /// Higher = more novel / less explored.
    pub fn novelty_score(&self, bucket: &ContextBucket) -> f32 {
        if self.total_visits == 0 {
            return 1.0;
        }

        let bucket_visits: u64 = self
            .visit_counts
            .get(bucket)
            .map(|arms| arms.values().sum())
            .unwrap_or(0);

        if bucket_visits == 0 {
            return 1.0;
        }

        1.0 - (bucket_visits as f32 / self.total_visits as f32)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Integrated Meta-Learning Engine
// ═══════════════════════════════════════════════════════════════════

/// Unified meta-learning engine that composes all five improvements.
///
/// Drop-in enhancement for the existing DomainExpansionEngine.
/// Call `record_decision` after each arm selection and `check_plateau`
/// periodically to get adaptive strategy recommendations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaLearningEngine {
    pub regret: RegretTracker,
    pub plateau: PlateauDetector,
    pub pareto: ParetoFront,
    pub curiosity: CuriosityBonus,
    /// Per-bucket decaying beta distributions (optional overlay).
    pub decaying_betas: HashMap<(ContextBucket, ArmId), DecayingBeta>,
    /// Decay factor for the decaying beta distributions.
    decay_factor: f32,
}

impl MetaLearningEngine {
    /// Create with standard parameters.
    pub fn new() -> Self {
        Self {
            regret: RegretTracker::new(50),
            plateau: PlateauDetector::new(5, 0.005),
            pareto: ParetoFront::new(),
            curiosity: CuriosityBonus::new(1.41),
            decaying_betas: HashMap::new(),
            decay_factor: 0.995,
        }
    }

    /// Create with custom parameters.
    pub fn with_config(
        regret_snapshot_interval: u64,
        plateau_window: usize,
        plateau_threshold: f32,
        exploration_coeff: f32,
        decay_factor: f32,
    ) -> Self {
        Self {
            regret: RegretTracker::new(regret_snapshot_interval),
            plateau: PlateauDetector::new(plateau_window, plateau_threshold),
            pareto: ParetoFront::new(),
            curiosity: CuriosityBonus::new(exploration_coeff),
            decaying_betas: HashMap::new(),
            decay_factor,
        }
    }

    /// Record a decision outcome. Call after every arm selection.
    pub fn record_decision(&mut self, bucket: &ContextBucket, arm: &ArmId, reward: f32) {
        // 1. Track regret
        self.regret.record(bucket, arm, reward);

        // 2. Update curiosity counts
        self.curiosity.record_visit(bucket, arm);

        // 3. Update decaying beta for this bucket/arm.
        //    Avoid tuple clone on hot path when entry exists.
        let key = (bucket.clone(), arm.clone());
        if let Some(db) = self.decaying_betas.get_mut(&key) {
            db.update(reward);
        } else {
            let mut db = DecayingBeta::new(self.decay_factor);
            db.update(reward);
            self.decaying_betas.insert(key, db);
        }
    }

    /// Record a population kernel's multi-objective performance.
    pub fn record_kernel(
        &mut self,
        kernel_id: &str,
        accuracy: f32,
        cost: f32,
        robustness: f32,
        generation: u32,
    ) {
        let point = ParetoPoint {
            kernel_id: kernel_id.to_string(),
            // Convention: higher is better, so negate cost
            objectives: vec![accuracy, -cost, robustness],
            generation,
        };
        self.pareto.insert(point);
    }

    /// Check the cost curve for plateau and recommend action.
    pub fn check_plateau(&mut self, points: &[CostCurvePoint]) -> PlateauAction {
        self.plateau.check(points)
    }

    /// Get the curiosity-boosted score for an arm.
    ///
    /// Combines the Thompson Sampling estimate with an exploration bonus.
    pub fn boosted_score(&self, bucket: &ContextBucket, arm: &ArmId, thompson_sample: f32) -> f32 {
        let bonus = self.curiosity.bonus(bucket, arm);
        thompson_sample + bonus
    }

    /// Get the decaying beta mean for a bucket/arm (if tracked).
    pub fn decaying_mean(&self, bucket: &ContextBucket, arm: &ArmId) -> Option<f32> {
        let key = (bucket.clone(), arm.clone());
        self.decaying_betas.get(&key).map(|db| db.mean())
    }

    /// Comprehensive health check of the learning system.
    pub fn health_check(&self) -> MetaLearningHealth {
        let regret_summary = self.regret.summary();
        let pareto_size = self.pareto.len();

        let is_learning = regret_summary.mean_growth_rate < 0.8;
        let is_diverse = pareto_size >= 3;
        let is_exploring = self.curiosity.total_visits > 0;

        MetaLearningHealth {
            regret: regret_summary,
            pareto_size,
            pareto_hypervolume: self.pareto.hypervolume(&[0.0, -1.0, 0.0]),
            consecutive_plateaus: self.plateau.consecutive_plateaus,
            total_plateaus: self.plateau.total_plateaus,
            curiosity_total_visits: self.curiosity.total_visits,
            decaying_beta_count: self.decaying_betas.len(),
            is_learning,
            is_diverse,
            is_exploring,
        }
    }
}

impl Default for MetaLearningEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Health summary of the meta-learning system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaLearningHealth {
    pub regret: RegretSummary,
    pub pareto_size: usize,
    pub pareto_hypervolume: f32,
    pub consecutive_plateaus: u32,
    pub total_plateaus: u32,
    pub curiosity_total_visits: u64,
    pub decaying_beta_count: usize,
    /// True if regret growth is sublinear (system is learning).
    pub is_learning: bool,
    /// True if Pareto front has diverse solutions.
    pub is_diverse: bool,
    /// True if curiosity is actively exploring.
    pub is_exploring: bool,
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bucket(tier: &str, cat: &str) -> ContextBucket {
        ContextBucket {
            difficulty_tier: tier.into(),
            category: cat.into(),
        }
    }

    // -- RegretTracker tests --

    #[test]
    fn test_regret_tracker_empty() {
        let tracker = RegretTracker::new(10);
        assert_eq!(tracker.total_regret, 0.0);
        assert_eq!(tracker.average_regret(), 0.0);
    }

    #[test]
    fn test_regret_tracker_optimal_arm() {
        let mut tracker = RegretTracker::new(10);
        let bucket = test_bucket("easy", "test");
        let arm = ArmId("best".into());

        // Always picking the best arm → zero regret
        for _ in 0..100 {
            tracker.record(&bucket, &arm, 0.9);
        }

        assert_eq!(tracker.total_observations, 100);
        // All same arm, so regret is 0
        assert!(tracker.total_regret < 1e-6);
    }

    #[test]
    fn test_regret_tracker_suboptimal_arm() {
        let mut tracker = RegretTracker::new(10);
        let bucket = test_bucket("medium", "test");
        let good = ArmId("good".into());
        let bad = ArmId("bad".into());

        // Establish good arm's mean
        for _ in 0..50 {
            tracker.record(&bucket, &good, 0.9);
        }

        // Now pick the bad arm repeatedly → regret accumulates
        for _ in 0..50 {
            tracker.record(&bucket, &bad, 0.3);
        }

        assert!(tracker.total_regret > 0.0);
        assert!(tracker.average_regret() > 0.0);
    }

    #[test]
    fn test_regret_growth_rate() {
        let mut tracker = RegretTracker::new(5);
        let bucket = test_bucket("hard", "test");
        let arm_a = ArmId("a".into());
        let arm_b = ArmId("b".into());

        for _ in 0..50 {
            tracker.record(&bucket, &arm_a, 0.8);
        }
        for _ in 0..50 {
            tracker.record(&bucket, &arm_b, 0.4);
        }

        let rate = tracker.regret_growth_rate(&bucket);
        assert!(rate.is_some());
        // Rate should be defined (we have enough observations)
    }

    #[test]
    fn test_regret_summary() {
        let mut tracker = RegretTracker::new(10);
        let bucket = test_bucket("easy", "algo");
        let arm = ArmId("test".into());

        for _ in 0..20 {
            tracker.record(&bucket, &arm, 0.7);
        }

        let summary = tracker.summary();
        assert_eq!(summary.total_observations, 20);
        assert_eq!(summary.bucket_count, 1);
    }

    // -- DecayingBeta tests --

    #[test]
    fn test_decaying_beta_initial() {
        let db = DecayingBeta::new(0.995);
        assert!((db.mean() - 0.5).abs() < 1e-6); // Uniform prior
        assert_eq!(db.effective_n, 0.0);
    }

    #[test]
    fn test_decaying_beta_update() {
        let mut db = DecayingBeta::new(0.995);

        for _ in 0..100 {
            db.update(0.9); // Mostly successes
        }

        assert!(db.mean() > 0.7); // Should reflect high success rate
        assert!(db.effective_n > 50.0); // Decayed but substantial
    }

    #[test]
    fn test_decaying_beta_adapts() {
        let mut db = DecayingBeta::new(0.99); // Faster decay

        // First: many successes
        for _ in 0..100 {
            db.update(0.95);
        }
        let mean_after_good = db.mean();
        assert!(mean_after_good > 0.8);

        // Then: many failures (environment changed)
        for _ in 0..100 {
            db.update(0.1);
        }
        let mean_after_bad = db.mean();

        // With decay, it should adapt toward the new distribution
        assert!(mean_after_bad < mean_after_good);
        assert!(mean_after_bad < 0.5); // Should reflect recent failures
    }

    #[test]
    fn test_decaying_beta_window() {
        let db = DecayingBeta::new(0.99);
        let window = db.effective_window();
        assert!((window - 100.0).abs() < 1.0); // 1/(1-0.99) = 100

        let db2 = DecayingBeta::new(0.995);
        let window2 = db2.effective_window();
        assert!((window2 - 200.0).abs() < 1.0); // 1/(1-0.005) = 200
    }

    #[test]
    fn test_decaying_to_standard() {
        let mut db = DecayingBeta::new(0.995);
        for _ in 0..10 {
            db.update(0.8);
        }
        let params = db.to_beta_params();
        assert!(params.alpha > 1.0);
        assert!(params.beta > 1.0);
        assert!((params.mean() - db.mean()).abs() < 1e-6);
    }

    // -- PlateauDetector tests --

    #[test]
    fn test_plateau_no_data() {
        let mut detector = PlateauDetector::new(3, 0.01);
        let action = detector.check(&[]);
        assert_eq!(action, PlateauAction::Continue);
    }

    #[test]
    fn test_plateau_not_enough_data() {
        let mut detector = PlateauDetector::new(3, 0.01);
        let points: Vec<CostCurvePoint> = (0..4)
            .map(|i| CostCurvePoint {
                cycle: i as u64,
                accuracy: 0.5 + i as f32 * 0.1,
                cost_per_solve: 0.1,
                robustness: 0.8,
                policy_violations: 0,
                timestamp: i as f64,
            })
            .collect();

        let action = detector.check(&points);
        assert_eq!(action, PlateauAction::Continue);
    }

    #[test]
    fn test_plateau_detected() {
        let mut detector = PlateauDetector::new(3, 0.01);

        // Flat accuracy → plateau
        let points: Vec<CostCurvePoint> = (0..6)
            .map(|i| CostCurvePoint {
                cycle: i as u64,
                accuracy: 0.80 + (i as f32 * 0.001), // Nearly flat
                cost_per_solve: 0.1,
                robustness: 0.8,
                policy_violations: 0,
                timestamp: i as f64,
            })
            .collect();

        let action = detector.check(&points);
        assert_ne!(action, PlateauAction::Continue);
    }

    #[test]
    fn test_plateau_improving() {
        let mut detector = PlateauDetector::new(3, 0.01);

        // Clear improvement → no plateau
        let points: Vec<CostCurvePoint> = (0..6)
            .map(|i| CostCurvePoint {
                cycle: i as u64,
                accuracy: 0.50 + i as f32 * 0.08, // Strong improvement
                cost_per_solve: 0.1,
                robustness: 0.8,
                policy_violations: 0,
                timestamp: i as f64,
            })
            .collect();

        let action = detector.check(&points);
        assert_eq!(action, PlateauAction::Continue);
    }

    #[test]
    fn test_plateau_escalation() {
        let mut detector = PlateauDetector::new(3, 0.01);

        let flat_points: Vec<CostCurvePoint> = (0..6)
            .map(|i| CostCurvePoint {
                cycle: i as u64,
                accuracy: 0.80,
                cost_per_solve: 0.1,
                robustness: 0.8,
                policy_violations: 0,
                timestamp: i as f64,
            })
            .collect();

        assert_eq!(
            detector.check(&flat_points),
            PlateauAction::IncreaseExploration
        );
        assert_eq!(detector.check(&flat_points), PlateauAction::TriggerTransfer);
        assert_eq!(detector.check(&flat_points), PlateauAction::TriggerTransfer);
        assert_eq!(detector.check(&flat_points), PlateauAction::InjectDiversity);
    }

    #[test]
    fn test_learning_velocity() {
        let detector = PlateauDetector::new(3, 0.01);

        let points: Vec<CostCurvePoint> = (0..6)
            .map(|i| CostCurvePoint {
                cycle: i as u64,
                accuracy: 0.50 + i as f32 * 0.1,
                cost_per_solve: 0.1,
                robustness: 0.8,
                policy_violations: 0,
                timestamp: i as f64,
            })
            .collect();

        let velocity = detector.learning_velocity(&points);
        assert!(velocity > 0.0); // Should be positive (improving)
    }

    // -- ParetoFront tests --

    #[test]
    fn test_pareto_dominates() {
        assert!(ParetoFront::dominates(&[0.9, -0.1, 0.8], &[0.8, -0.2, 0.7]));
        assert!(!ParetoFront::dominates(
            &[0.9, -0.3, 0.8],
            &[0.8, -0.1, 0.7]
        ));
        assert!(!ParetoFront::dominates(
            &[0.9, -0.1, 0.8],
            &[0.9, -0.1, 0.8]
        )); // Equal
    }

    #[test]
    fn test_pareto_insert_non_dominated() {
        let mut front = ParetoFront::new();

        // Two non-dominated points (tradeoff: accuracy vs cost)
        assert!(front.insert(ParetoPoint {
            kernel_id: "a".into(),
            objectives: vec![0.9, -0.3, 0.7],
            generation: 0,
        }));
        assert!(front.insert(ParetoPoint {
            kernel_id: "b".into(),
            objectives: vec![0.7, -0.1, 0.9],
            generation: 0,
        }));

        assert_eq!(front.len(), 2);
    }

    #[test]
    fn test_pareto_insert_dominated() {
        let mut front = ParetoFront::new();

        front.insert(ParetoPoint {
            kernel_id: "good".into(),
            objectives: vec![0.9, -0.1, 0.9],
            generation: 0,
        });

        // This is dominated by "good" on all objectives
        let added = front.insert(ParetoPoint {
            kernel_id: "bad".into(),
            objectives: vec![0.5, -0.5, 0.5],
            generation: 0,
        });

        assert!(!added);
        assert_eq!(front.len(), 1);
    }

    #[test]
    fn test_pareto_removes_dominated() {
        let mut front = ParetoFront::new();

        front.insert(ParetoPoint {
            kernel_id: "old".into(),
            objectives: vec![0.5, -0.3, 0.5],
            generation: 0,
        });

        // New point dominates old
        front.insert(ParetoPoint {
            kernel_id: "new".into(),
            objectives: vec![0.9, -0.1, 0.9],
            generation: 1,
        });

        assert_eq!(front.len(), 1);
        assert_eq!(front.front[0].kernel_id, "new");
    }

    #[test]
    fn test_pareto_best_on_objective() {
        let mut front = ParetoFront::new();

        front.insert(ParetoPoint {
            kernel_id: "accurate".into(),
            objectives: vec![0.95, -0.5, 0.6],
            generation: 0,
        });
        front.insert(ParetoPoint {
            kernel_id: "cheap".into(),
            objectives: vec![0.7, -0.05, 0.7],
            generation: 0,
        });
        front.insert(ParetoPoint {
            kernel_id: "robust".into(),
            objectives: vec![0.8, -0.3, 0.95],
            generation: 0,
        });

        assert_eq!(front.best_on(0).unwrap().kernel_id, "accurate");
        assert_eq!(front.best_on(1).unwrap().kernel_id, "cheap"); // -0.05 > -0.5
        assert_eq!(front.best_on(2).unwrap().kernel_id, "robust");
    }

    #[test]
    fn test_pareto_spread() {
        let mut front = ParetoFront::new();

        // Non-dominated tradeoff: a is better on obj0, b is better on obj1.
        front.insert(ParetoPoint {
            kernel_id: "a".into(),
            objectives: vec![0.9, -0.5],
            generation: 0,
        });
        front.insert(ParetoPoint {
            kernel_id: "b".into(),
            objectives: vec![0.5, -0.1],
            generation: 0,
        });

        assert_eq!(front.len(), 2); // Both should survive (non-dominated)
        let spread = front.spread();
        assert_eq!(spread.len(), 2);
        assert!((spread[0] - 0.4).abs() < 1e-4); // 0.9 - 0.5
        assert!((spread[1] - 0.4).abs() < 1e-4); // -0.1 - (-0.5)
    }

    #[test]
    fn test_pareto_hypervolume_2d() {
        let mut front = ParetoFront::new();

        front.insert(ParetoPoint {
            kernel_id: "a".into(),
            objectives: vec![1.0, 1.0],
            generation: 0,
        });

        let hv = front.hypervolume(&[0.0, 0.0]);
        assert!((hv - 1.0).abs() < 1e-4); // 1x1 rectangle
    }

    // -- CuriosityBonus tests --

    #[test]
    fn test_curiosity_bonus_unvisited() {
        let curiosity = CuriosityBonus::new(1.41);
        let bucket = test_bucket("hard", "novel");
        let arm = ArmId("new".into());

        let bonus = curiosity.bonus(&bucket, &arm);
        assert!(bonus > 0.0); // Should have high bonus for unvisited
    }

    #[test]
    fn test_curiosity_bonus_decays_with_visits() {
        let mut curiosity = CuriosityBonus::new(1.41);
        let bucket = test_bucket("easy", "test");
        let arm = ArmId("a".into());

        let bonus_before = curiosity.bonus(&bucket, &arm);

        for _ in 0..50 {
            curiosity.record_visit(&bucket, &arm);
        }

        let bonus_after = curiosity.bonus(&bucket, &arm);
        assert!(bonus_after < bonus_before); // Bonus should decrease
    }

    #[test]
    fn test_curiosity_novelty_score() {
        let mut curiosity = CuriosityBonus::new(1.41);
        let explored = test_bucket("easy", "common");
        let novel = test_bucket("hard", "rare");
        let arm = ArmId("a".into());

        for _ in 0..100 {
            curiosity.record_visit(&explored, &arm);
        }
        curiosity.record_visit(&novel, &arm);

        let explored_novelty = curiosity.novelty_score(&explored);
        let novel_novelty = curiosity.novelty_score(&novel);

        assert!(novel_novelty > explored_novelty);
    }

    // -- MetaLearningEngine integration tests --

    #[test]
    fn test_meta_engine_creation() {
        let engine = MetaLearningEngine::new();
        assert_eq!(engine.regret.total_observations, 0);
        assert!(engine.pareto.is_empty());
        assert_eq!(engine.curiosity.total_visits, 0);
    }

    #[test]
    fn test_meta_engine_record_decision() {
        let mut engine = MetaLearningEngine::new();
        let bucket = test_bucket("medium", "algo");
        let arm = ArmId("greedy".into());

        for _ in 0..50 {
            engine.record_decision(&bucket, &arm, 0.85);
        }

        assert_eq!(engine.regret.total_observations, 50);
        assert_eq!(engine.curiosity.total_visits, 50);
        assert!(engine.decaying_mean(&bucket, &arm).unwrap() > 0.7);
    }

    #[test]
    fn test_meta_engine_boosted_score() {
        let mut engine = MetaLearningEngine::new();
        let explored = test_bucket("easy", "common");
        let novel = test_bucket("hard", "rare");
        let arm = ArmId("a".into());

        // Explore one bucket heavily
        for _ in 0..100 {
            engine.record_decision(&explored, &arm, 0.8);
        }

        let score_explored = engine.boosted_score(&explored, &arm, 0.5);
        let score_novel = engine.boosted_score(&novel, &arm, 0.5);

        // Novel bucket should get higher boosted score
        assert!(score_novel > score_explored);
    }

    #[test]
    fn test_meta_engine_kernel_recording() {
        let mut engine = MetaLearningEngine::new();

        engine.record_kernel("k1", 0.9, 0.3, 0.7, 0);
        engine.record_kernel("k2", 0.7, 0.1, 0.9, 0);
        engine.record_kernel("k3", 0.5, 0.5, 0.5, 0); // Dominated by k1

        // k1 and k2 are non-dominated; k3 is dominated
        assert!(engine.pareto.len() <= 2);
    }

    #[test]
    fn test_meta_engine_health_check() {
        let mut engine = MetaLearningEngine::new();
        let bucket = test_bucket("medium", "test");
        let arm = ArmId("a".into());

        for _ in 0..100 {
            engine.record_decision(&bucket, &arm, 0.8);
        }

        let health = engine.health_check();
        assert_eq!(health.curiosity_total_visits, 100);
        assert!(health.is_exploring);
    }

    #[test]
    fn test_meta_engine_plateau_check() {
        let mut engine = MetaLearningEngine::new();

        let flat_points: Vec<CostCurvePoint> = (0..10)
            .map(|i| CostCurvePoint {
                cycle: i as u64,
                accuracy: 0.80,
                cost_per_solve: 0.1,
                robustness: 0.8,
                policy_violations: 0,
                timestamp: i as f64,
            })
            .collect();

        let action = engine.check_plateau(&flat_points);
        assert_ne!(action, PlateauAction::Continue);
    }

    #[test]
    fn test_meta_engine_default() {
        let engine = MetaLearningEngine::default();
        assert_eq!(engine.curiosity.exploration_coeff, 1.41);
    }
}
