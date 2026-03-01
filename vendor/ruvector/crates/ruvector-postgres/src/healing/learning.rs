//! Learning System for Self-Healing Engine
//!
//! Tracks remediation outcomes and adjusts strategy selection:
//! - Outcome recording with full context
//! - Strategy weight updates based on success/failure
//! - Confidence scoring for strategies
//! - Effectiveness reporting

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::detector::{Problem, ProblemType, Severity};
use super::strategies::RemediationResult;

// ============================================================================
// Outcome Record
// ============================================================================

/// A recorded remediation outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeRecord {
    /// Unique ID
    pub id: u64,
    /// Problem type
    pub problem_type: ProblemType,
    /// Problem severity
    pub severity: Severity,
    /// Strategy used
    pub strategy_name: String,
    /// Whether remediation succeeded
    pub success: bool,
    /// Whether improvement was verified
    pub verified: bool,
    /// Actions taken
    pub actions_taken: usize,
    /// Improvement percentage
    pub improvement_pct: f32,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Timestamp
    pub timestamp: u64,
    /// Human feedback score (if provided, 0-1)
    pub feedback_score: Option<f32>,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl OutcomeRecord {
    /// Create from a problem and result
    pub fn from_result(
        id: u64,
        problem: &Problem,
        strategy_name: &str,
        result: &RemediationResult,
        verified: bool,
    ) -> Self {
        Self {
            id,
            problem_type: problem.problem_type,
            severity: problem.severity,
            strategy_name: strategy_name.to_string(),
            success: result.is_success(),
            verified,
            actions_taken: result.actions_taken,
            improvement_pct: result.improvement_pct,
            duration_ms: result.duration_ms,
            error_message: result.error_message.clone(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            feedback_score: None,
            metadata: result.metadata.clone(),
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "problem_type": self.problem_type.to_string(),
            "severity": format!("{:?}", self.severity).to_lowercase(),
            "strategy_name": self.strategy_name,
            "success": self.success,
            "verified": self.verified,
            "actions_taken": self.actions_taken,
            "improvement_pct": self.improvement_pct,
            "duration_ms": self.duration_ms,
            "error_message": self.error_message,
            "timestamp": self.timestamp,
            "feedback_score": self.feedback_score,
        })
    }
}

// ============================================================================
// Strategy Weight
// ============================================================================

/// Strategy weight with confidence metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyWeight {
    /// Strategy name
    pub strategy_name: String,
    /// Current weight (1.0 = baseline)
    pub weight: f32,
    /// Confidence in weight (0-1)
    pub confidence: f32,
    /// Number of observations
    pub observations: usize,
    /// Success count
    pub successes: usize,
    /// Average improvement when successful
    pub avg_improvement: f32,
    /// Average duration in milliseconds
    pub avg_duration_ms: u64,
    /// Last update timestamp
    pub last_updated: u64,
}

impl StrategyWeight {
    /// Create new weight for strategy
    pub fn new(strategy_name: &str) -> Self {
        Self {
            strategy_name: strategy_name.to_string(),
            weight: 1.0,
            confidence: 0.0,
            observations: 0,
            successes: 0,
            avg_improvement: 0.0,
            avg_duration_ms: 0,
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Update with new observation
    pub fn update(&mut self, success: bool, improvement_pct: f32, duration_ms: u64) {
        self.observations += 1;
        if success {
            self.successes += 1;
        }

        // Update running averages
        let n = self.observations as f32;
        self.avg_improvement = ((n - 1.0) * self.avg_improvement + improvement_pct) / n;
        self.avg_duration_ms = ((self.observations as u64 - 1) * self.avg_duration_ms
            + duration_ms)
            / self.observations as u64;

        // Calculate success rate
        let success_rate = self.successes as f32 / self.observations as f32;

        // Weight = success_rate * (1 + avg_improvement/100)
        self.weight = success_rate * (1.0 + self.avg_improvement / 100.0);
        self.weight = self.weight.max(0.1).min(2.0);

        // Confidence increases with observations (asymptotic to 1.0)
        self.confidence = 1.0 - 1.0 / (1.0 + (self.observations as f32 / 10.0));

        self.last_updated = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// Get success rate
    pub fn success_rate(&self) -> f32 {
        if self.observations > 0 {
            self.successes as f32 / self.observations as f32
        } else {
            0.0
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "strategy_name": self.strategy_name,
            "weight": self.weight,
            "confidence": self.confidence,
            "observations": self.observations,
            "successes": self.successes,
            "success_rate": self.success_rate(),
            "avg_improvement": self.avg_improvement,
            "avg_duration_ms": self.avg_duration_ms,
            "last_updated": self.last_updated,
        })
    }
}

// ============================================================================
// Outcome Tracker
// ============================================================================

/// Tracks remediation outcomes for learning
#[derive(Clone)]
pub struct OutcomeTracker {
    /// Outcome history
    history: std::sync::Arc<RwLock<VecDeque<OutcomeRecord>>>,
    /// Strategy weights
    weights: std::sync::Arc<RwLock<HashMap<String, StrategyWeight>>>,
    /// Maximum history size
    max_history: usize,
    /// Next record ID
    next_id: std::sync::Arc<AtomicU64>,
}

impl OutcomeTracker {
    /// Create new tracker
    pub fn new() -> Self {
        Self {
            history: std::sync::Arc::new(RwLock::new(VecDeque::new())),
            weights: std::sync::Arc::new(RwLock::new(HashMap::new())),
            max_history: 10000,
            next_id: std::sync::Arc::new(AtomicU64::new(1)),
        }
    }

    /// Create with custom history size
    pub fn with_max_history(max_history: usize) -> Self {
        Self {
            history: std::sync::Arc::new(RwLock::new(VecDeque::new())),
            weights: std::sync::Arc::new(RwLock::new(HashMap::new())),
            max_history,
            next_id: std::sync::Arc::new(AtomicU64::new(1)),
        }
    }

    /// Record a remediation outcome
    pub fn record(
        &self,
        problem: &Problem,
        strategy_name: &str,
        result: &RemediationResult,
        verified: bool,
    ) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let record = OutcomeRecord::from_result(id, problem, strategy_name, result, verified);

        // Add to history
        let mut history = self.history.write();
        history.push_back(record.clone());
        while history.len() > self.max_history {
            history.pop_front();
        }

        // Update strategy weight
        let mut weights = self.weights.write();
        let weight = weights
            .entry(strategy_name.to_string())
            .or_insert_with(|| StrategyWeight::new(strategy_name));
        weight.update(verified, result.improvement_pct, result.duration_ms);
    }

    /// Get recent outcomes
    pub fn get_recent(&self, limit: usize) -> Vec<OutcomeRecord> {
        let history = self.history.read();
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Get outcomes since timestamp
    pub fn get_since(&self, since: u64) -> Vec<OutcomeRecord> {
        let history = self.history.read();
        history
            .iter()
            .filter(|r| r.timestamp >= since)
            .cloned()
            .collect()
    }

    /// Get outcomes for a specific strategy
    pub fn get_for_strategy(&self, strategy_name: &str, limit: usize) -> Vec<OutcomeRecord> {
        let history = self.history.read();
        history
            .iter()
            .rev()
            .filter(|r| r.strategy_name == strategy_name)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get outcomes for a specific problem type
    pub fn get_for_problem_type(
        &self,
        problem_type: ProblemType,
        limit: usize,
    ) -> Vec<OutcomeRecord> {
        let history = self.history.read();
        history
            .iter()
            .rev()
            .filter(|r| r.problem_type == problem_type)
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get strategy weight
    pub fn get_weight(&self, strategy_name: &str) -> Option<StrategyWeight> {
        self.weights.read().get(strategy_name).cloned()
    }

    /// Get all strategy weights
    pub fn get_all_weights(&self) -> Vec<StrategyWeight> {
        self.weights.read().values().cloned().collect()
    }

    /// Add human feedback to an outcome
    pub fn add_feedback(&self, outcome_id: u64, score: f32) -> bool {
        let mut history = self.history.write();
        for record in history.iter_mut() {
            if record.id == outcome_id {
                record.feedback_score = Some(score.max(0.0).min(1.0));
                return true;
            }
        }
        false
    }

    /// Get overall statistics
    pub fn get_stats(&self) -> TrackerStats {
        let history = self.history.read();
        let weights = self.weights.read();

        let total = history.len();
        let successes = history.iter().filter(|r| r.success && r.verified).count();
        let total_improvement: f32 = history.iter().map(|r| r.improvement_pct).sum();
        let total_duration: u64 = history.iter().map(|r| r.duration_ms).sum();

        TrackerStats {
            total_outcomes: total,
            successful_outcomes: successes,
            success_rate: if total > 0 {
                successes as f32 / total as f32
            } else {
                0.0
            },
            avg_improvement: if total > 0 {
                total_improvement / total as f32
            } else {
                0.0
            },
            avg_duration_ms: if total > 0 {
                total_duration / total as u64
            } else {
                0
            },
            tracked_strategies: weights.len(),
        }
    }

    /// Generate effectiveness report
    pub fn effectiveness_report(&self) -> EffectivenessReport {
        let weights = self.get_all_weights();
        let stats = self.get_stats();

        let strategy_reports: Vec<StrategyEffectiveness> = weights
            .iter()
            .map(|w| {
                let recent = self.get_for_strategy(&w.strategy_name, 10);
                StrategyEffectiveness {
                    strategy_name: w.strategy_name.clone(),
                    weight: w.weight,
                    confidence: w.confidence,
                    success_rate: w.success_rate(),
                    avg_improvement: w.avg_improvement,
                    recent_outcomes: recent.len(),
                }
            })
            .collect();

        EffectivenessReport {
            strategies: strategy_reports,
            overall_success_rate: stats.success_rate,
            avg_time_to_recovery_ms: stats.avg_duration_ms,
            total_outcomes: stats.total_outcomes,
        }
    }

    /// Update weights from historical data (for batch learning)
    pub fn recalculate_weights(&self, lookback: Duration) {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - lookback.as_secs();

        let history = self.history.read();
        let mut weights = self.weights.write();

        // Group outcomes by strategy
        let mut strategy_outcomes: HashMap<String, Vec<&OutcomeRecord>> = HashMap::new();
        for record in history.iter().filter(|r| r.timestamp >= cutoff) {
            strategy_outcomes
                .entry(record.strategy_name.clone())
                .or_default()
                .push(record);
        }

        // Recalculate each strategy's weight
        for (strategy_name, outcomes) in strategy_outcomes {
            let weight = weights
                .entry(strategy_name.clone())
                .or_insert_with(|| StrategyWeight::new(&strategy_name));

            // Reset counters
            weight.observations = outcomes.len();
            weight.successes = outcomes.iter().filter(|o| o.success && o.verified).count();
            weight.avg_improvement =
                outcomes.iter().map(|o| o.improvement_pct).sum::<f32>() / outcomes.len() as f32;
            weight.avg_duration_ms =
                outcomes.iter().map(|o| o.duration_ms).sum::<u64>() / outcomes.len() as u64;

            // Recalculate weight
            let success_rate = weight.success_rate();
            weight.weight = success_rate * (1.0 + weight.avg_improvement / 100.0);
            weight.weight = weight.weight.max(0.1).min(2.0);
            weight.confidence = 1.0 - 1.0 / (1.0 + (weight.observations as f32 / 10.0));
            weight.last_updated = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
    }
}

impl Default for OutcomeTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracker statistics
#[derive(Debug, Clone)]
pub struct TrackerStats {
    pub total_outcomes: usize,
    pub successful_outcomes: usize,
    pub success_rate: f32,
    pub avg_improvement: f32,
    pub avg_duration_ms: u64,
    pub tracked_strategies: usize,
}

impl TrackerStats {
    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "total_outcomes": self.total_outcomes,
            "successful_outcomes": self.successful_outcomes,
            "success_rate": self.success_rate,
            "avg_improvement": self.avg_improvement,
            "avg_duration_ms": self.avg_duration_ms,
            "tracked_strategies": self.tracked_strategies,
        })
    }
}

/// Strategy effectiveness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyEffectiveness {
    pub strategy_name: String,
    pub weight: f32,
    pub confidence: f32,
    pub success_rate: f32,
    pub avg_improvement: f32,
    pub recent_outcomes: usize,
}

/// Effectiveness report
#[derive(Debug, Clone)]
pub struct EffectivenessReport {
    pub strategies: Vec<StrategyEffectiveness>,
    pub overall_success_rate: f32,
    pub avg_time_to_recovery_ms: u64,
    pub total_outcomes: usize,
}

impl EffectivenessReport {
    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "strategies": self.strategies,
            "overall_success_rate": self.overall_success_rate,
            "avg_time_to_recovery_ms": self.avg_time_to_recovery_ms,
            "total_outcomes": self.total_outcomes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_problem() -> Problem {
        Problem::new(ProblemType::IndexDegradation, Severity::Medium)
    }

    fn create_result(success: bool, improvement: f32) -> RemediationResult {
        if success {
            RemediationResult::success(1, improvement).with_duration(1000)
        } else {
            RemediationResult::failure("test error").with_duration(500)
        }
    }

    #[test]
    fn test_record_outcome() {
        let tracker = OutcomeTracker::new();
        let problem = create_problem();
        let result = create_result(true, 15.0);

        tracker.record(&problem, "test_strategy", &result, true);

        let recent = tracker.get_recent(10);
        assert_eq!(recent.len(), 1);
        assert!(recent[0].success);
        assert!(recent[0].verified);
    }

    #[test]
    fn test_weight_updates() {
        let tracker = OutcomeTracker::new();
        let problem = create_problem();

        // Record successes
        for _ in 0..5 {
            let result = create_result(true, 20.0);
            tracker.record(&problem, "test_strategy", &result, true);
        }

        let weight = tracker.get_weight("test_strategy").unwrap();
        assert_eq!(weight.observations, 5);
        assert_eq!(weight.successes, 5);
        assert!(weight.weight > 1.0); // Should be elevated
        assert!(weight.confidence > 0.3); // Should have some confidence
    }

    #[test]
    fn test_mixed_outcomes() {
        let tracker = OutcomeTracker::new();
        let problem = create_problem();

        // 3 successes
        for _ in 0..3 {
            let result = create_result(true, 10.0);
            tracker.record(&problem, "test_strategy", &result, true);
        }

        // 2 failures
        for _ in 0..2 {
            let result = create_result(false, 0.0);
            tracker.record(&problem, "test_strategy", &result, false);
        }

        let weight = tracker.get_weight("test_strategy").unwrap();
        assert_eq!(weight.observations, 5);
        assert_eq!(weight.successes, 3);
        assert!((weight.success_rate() - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_get_for_strategy() {
        let tracker = OutcomeTracker::new();
        let problem = create_problem();

        // Record for different strategies
        tracker.record(&problem, "strategy_a", &create_result(true, 10.0), true);
        tracker.record(&problem, "strategy_b", &create_result(true, 15.0), true);
        tracker.record(&problem, "strategy_a", &create_result(true, 20.0), true);

        let a_outcomes = tracker.get_for_strategy("strategy_a", 10);
        assert_eq!(a_outcomes.len(), 2);

        let b_outcomes = tracker.get_for_strategy("strategy_b", 10);
        assert_eq!(b_outcomes.len(), 1);
    }

    #[test]
    fn test_feedback() {
        let tracker = OutcomeTracker::new();
        let problem = create_problem();

        tracker.record(&problem, "test_strategy", &create_result(true, 10.0), true);

        let recent = tracker.get_recent(1);
        let id = recent[0].id;

        assert!(tracker.add_feedback(id, 0.9));

        let updated = tracker.get_recent(1);
        assert_eq!(updated[0].feedback_score, Some(0.9));
    }

    #[test]
    fn test_max_history() {
        let tracker = OutcomeTracker::with_max_history(5);
        let problem = create_problem();

        // Record 10 outcomes
        for i in 0..10 {
            tracker.record(
                &problem,
                "test_strategy",
                &create_result(true, i as f32),
                true,
            );
        }

        let history = tracker.get_recent(100);
        assert_eq!(history.len(), 5); // Should be capped at 5
    }

    #[test]
    fn test_effectiveness_report() {
        let tracker = OutcomeTracker::new();
        let problem = create_problem();

        for _ in 0..5 {
            tracker.record(&problem, "strategy_a", &create_result(true, 15.0), true);
        }
        for _ in 0..5 {
            tracker.record(&problem, "strategy_b", &create_result(true, 25.0), true);
        }

        let report = tracker.effectiveness_report();
        assert_eq!(report.strategies.len(), 2);
        assert_eq!(report.total_outcomes, 10);
        assert_eq!(report.overall_success_rate, 1.0);
    }

    #[test]
    fn test_strategy_weight_confidence() {
        let mut weight = StrategyWeight::new("test");

        // Few observations = low confidence
        weight.update(true, 10.0, 1000);
        assert!(weight.confidence < 0.5);

        // More observations = higher confidence
        for _ in 0..20 {
            weight.update(true, 10.0, 1000);
        }
        assert!(weight.confidence > 0.5);
    }

    #[test]
    fn test_tracker_stats() {
        let tracker = OutcomeTracker::new();
        let problem = create_problem();

        tracker.record(&problem, "strategy_a", &create_result(true, 10.0), true);
        tracker.record(&problem, "strategy_b", &create_result(false, 0.0), false);

        let stats = tracker.get_stats();
        assert_eq!(stats.total_outcomes, 2);
        assert_eq!(stats.successful_outcomes, 1);
        assert_eq!(stats.success_rate, 0.5);
    }
}
