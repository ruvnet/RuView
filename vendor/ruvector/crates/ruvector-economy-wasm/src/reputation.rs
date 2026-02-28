//! Reputation Scoring System
//!
//! Multi-factor reputation based on:
//! - Accuracy: Success rate of completed tasks
//! - Uptime: Availability and reliability
//! - Stake: Skin in the game (economic commitment)
//!
//! The composite score determines task priority and trust level.

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Reputation score for a network participant
///
/// Combines multiple factors into a single trust score:
/// - accuracy: 0.0 to 1.0 (success rate of verified tasks)
/// - uptime: 0.0 to 1.0 (availability ratio)
/// - stake: absolute stake amount (economic commitment)
///
/// The composite score is weighted:
/// ```text
/// composite = accuracy^2 * uptime * stake_weight
///
/// where stake_weight = min(1.0, log10(stake + 1) / 6)
/// ```
///
/// This ensures:
/// - Accuracy is most important (squared)
/// - Uptime provides linear scaling
/// - Stake has diminishing returns (log scale)
#[wasm_bindgen]
#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct ReputationScore {
    /// Task success rate (0.0 - 1.0)
    accuracy: f32,
    /// Network availability (0.0 - 1.0)
    uptime: f32,
    /// Staked credits
    stake: u64,
    /// Number of completed tasks
    tasks_completed: u64,
    /// Number of failed/disputed tasks
    tasks_failed: u64,
    /// Total uptime in seconds
    uptime_seconds: u64,
    /// Total possible uptime in seconds (since registration)
    total_seconds: u64,
}

#[wasm_bindgen]
impl ReputationScore {
    /// Create a new reputation score
    #[wasm_bindgen(constructor)]
    pub fn new(accuracy: f32, uptime: f32, stake: u64) -> ReputationScore {
        ReputationScore {
            accuracy: accuracy.clamp(0.0, 1.0),
            uptime: uptime.clamp(0.0, 1.0),
            stake,
            tasks_completed: 0,
            tasks_failed: 0,
            uptime_seconds: 0,
            total_seconds: 0,
        }
    }

    /// Create with detailed tracking
    #[wasm_bindgen(js_name = newWithTracking)]
    pub fn new_with_tracking(
        tasks_completed: u64,
        tasks_failed: u64,
        uptime_seconds: u64,
        total_seconds: u64,
        stake: u64,
    ) -> ReputationScore {
        let accuracy = if tasks_completed + tasks_failed > 0 {
            tasks_completed as f32 / (tasks_completed + tasks_failed) as f32
        } else {
            0.0
        };

        let uptime = if total_seconds > 0 {
            (uptime_seconds as f32 / total_seconds as f32).min(1.0)
        } else {
            0.0
        };

        ReputationScore {
            accuracy,
            uptime,
            stake,
            tasks_completed,
            tasks_failed,
            uptime_seconds,
            total_seconds,
        }
    }

    /// Get accuracy score (0.0 - 1.0)
    #[wasm_bindgen(getter)]
    pub fn accuracy(&self) -> f32 {
        self.accuracy
    }

    /// Get uptime score (0.0 - 1.0)
    #[wasm_bindgen(getter)]
    pub fn uptime(&self) -> f32 {
        self.uptime
    }

    /// Get stake amount
    #[wasm_bindgen(getter)]
    pub fn stake(&self) -> u64 {
        self.stake
    }

    /// Calculate stake weight using logarithmic scaling
    ///
    /// Uses log10(stake + 1) / 6 capped at 1.0
    /// This means:
    /// - 0 stake = 0.0 weight
    /// - 100 stake = ~0.33 weight
    /// - 10,000 stake = ~0.67 weight
    /// - 1,000,000 stake = 1.0 weight (capped)
    #[wasm_bindgen(js_name = stakeWeight)]
    pub fn stake_weight(&self) -> f32 {
        if self.stake == 0 {
            return 0.0;
        }

        let log_stake = (self.stake as f64 + 1.0).log10();
        (log_stake / 6.0).min(1.0) as f32
    }

    /// Calculate composite reputation score
    ///
    /// Formula: accuracy^2 * uptime * stake_weight
    ///
    /// Returns a value between 0.0 and 1.0
    #[wasm_bindgen(js_name = compositeScore)]
    pub fn composite_score(&self) -> f32 {
        self.accuracy.powi(2) * self.uptime * self.stake_weight()
    }

    /// Get reputation tier based on composite score
    #[wasm_bindgen(js_name = tierName)]
    pub fn tier_name(&self) -> String {
        let score = self.composite_score();

        if score >= 0.9 {
            "Diamond".to_string()
        } else if score >= 0.75 {
            "Platinum".to_string()
        } else if score >= 0.5 {
            "Gold".to_string()
        } else if score >= 0.25 {
            "Silver".to_string()
        } else if score >= 0.1 {
            "Bronze".to_string()
        } else {
            "Newcomer".to_string()
        }
    }

    /// Check if node meets minimum reputation for participation
    #[wasm_bindgen(js_name = meetsMinimum)]
    pub fn meets_minimum(&self, min_accuracy: f32, min_uptime: f32, min_stake: u64) -> bool {
        self.accuracy >= min_accuracy && self.uptime >= min_uptime && self.stake >= min_stake
    }

    /// Record a successful task completion
    #[wasm_bindgen(js_name = recordSuccess)]
    pub fn record_success(&mut self) {
        self.tasks_completed += 1;
        self.update_accuracy();
    }

    /// Record a failed/disputed task
    #[wasm_bindgen(js_name = recordFailure)]
    pub fn record_failure(&mut self) {
        self.tasks_failed += 1;
        self.update_accuracy();
    }

    /// Update uptime tracking
    #[wasm_bindgen(js_name = updateUptime)]
    pub fn update_uptime(&mut self, online_seconds: u64, total_seconds: u64) {
        self.uptime_seconds = online_seconds;
        self.total_seconds = total_seconds;
        if total_seconds > 0 {
            self.uptime = (online_seconds as f32 / total_seconds as f32).min(1.0);
        }
    }

    /// Update stake amount
    #[wasm_bindgen(js_name = updateStake)]
    pub fn update_stake(&mut self, new_stake: u64) {
        self.stake = new_stake;
    }

    /// Get tasks completed
    #[wasm_bindgen(js_name = tasksCompleted)]
    pub fn tasks_completed(&self) -> u64 {
        self.tasks_completed
    }

    /// Get tasks failed
    #[wasm_bindgen(js_name = tasksFailed)]
    pub fn tasks_failed(&self) -> u64 {
        self.tasks_failed
    }

    /// Get total tasks
    #[wasm_bindgen(js_name = totalTasks)]
    pub fn total_tasks(&self) -> u64 {
        self.tasks_completed + self.tasks_failed
    }

    /// Check if this reputation is better than another
    #[wasm_bindgen(js_name = isBetterThan)]
    pub fn is_better_than(&self, other: &ReputationScore) -> bool {
        self.composite_score() > other.composite_score()
    }

    /// Serialize to JSON
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Deserialize from JSON
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<ReputationScore, JsValue> {
        serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse JSON: {}", e)))
    }

    /// Update accuracy from tracked counts
    fn update_accuracy(&mut self) {
        let total = self.tasks_completed + self.tasks_failed;
        if total > 0 {
            self.accuracy = self.tasks_completed as f32 / total as f32;
        }
    }
}

/// Calculate stake weight (WASM export)
#[wasm_bindgen]
pub fn stake_weight(stake: u64) -> f32 {
    if stake == 0 {
        return 0.0;
    }
    let log_stake = (stake as f64 + 1.0).log10();
    (log_stake / 6.0).min(1.0) as f32
}

/// Calculate composite reputation score (WASM export)
#[wasm_bindgen]
pub fn composite_reputation(accuracy: f32, uptime: f32, stake: u64) -> f32 {
    let rep = ReputationScore::new(accuracy, uptime, stake);
    rep.composite_score()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_reputation() {
        let rep = ReputationScore::new(0.95, 0.98, 1000);
        assert!((rep.accuracy() - 0.95).abs() < 0.001);
        assert!((rep.uptime() - 0.98).abs() < 0.001);
        assert_eq!(rep.stake(), 1000);
    }

    #[test]
    fn test_clamp_values() {
        let rep = ReputationScore::new(1.5, -0.5, 100);
        assert!((rep.accuracy() - 1.0).abs() < 0.001);
        assert!((rep.uptime() - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_stake_weight() {
        // 0 stake = 0 weight
        assert_eq!(stake_weight(0), 0.0);

        // 1M stake = 1.0 weight (log10(1M) = 6)
        let weight = stake_weight(1_000_000);
        assert!((weight - 1.0).abs() < 0.01);

        // 10K stake = ~0.67 weight (log10(10K) = 4)
        let weight = stake_weight(10_000);
        assert!(weight > 0.6 && weight < 0.75);
    }

    #[test]
    fn test_composite_score() {
        // Perfect accuracy (1.0), perfect uptime (1.0), max stake weight
        let rep = ReputationScore::new(1.0, 1.0, 1_000_000);
        let score = rep.composite_score();
        assert!((score - 1.0).abs() < 0.01);

        // Zero accuracy = zero score
        let rep_zero = ReputationScore::new(0.0, 1.0, 1_000_000);
        assert!(rep_zero.composite_score() < 0.01);
    }

    #[test]
    fn test_tier_names() {
        let diamond = ReputationScore::new(1.0, 1.0, 1_000_000);
        assert_eq!(diamond.tier_name(), "Diamond");

        let newcomer = ReputationScore::new(0.1, 0.1, 10);
        assert_eq!(newcomer.tier_name(), "Newcomer");
    }

    #[test]
    fn test_record_success_failure() {
        let mut rep = ReputationScore::new(0.5, 1.0, 1000);
        rep.tasks_completed = 5;
        rep.tasks_failed = 5;

        rep.record_success();
        assert_eq!(rep.tasks_completed(), 6);
        assert!((rep.accuracy() - 6.0 / 11.0).abs() < 0.001);

        rep.record_failure();
        assert_eq!(rep.tasks_failed(), 6);
        assert!((rep.accuracy() - 6.0 / 12.0).abs() < 0.001);
    }

    #[test]
    fn test_meets_minimum() {
        let rep = ReputationScore::new(0.95, 0.98, 1000);

        assert!(rep.meets_minimum(0.9, 0.95, 500));
        assert!(!rep.meets_minimum(0.99, 0.95, 500)); // Accuracy too low
        assert!(!rep.meets_minimum(0.9, 0.99, 500)); // Uptime too low
        assert!(!rep.meets_minimum(0.9, 0.95, 2000)); // Stake too low
    }

    #[test]
    fn test_is_better_than() {
        let better = ReputationScore::new(0.95, 0.98, 10000);
        let worse = ReputationScore::new(0.8, 0.9, 1000);

        assert!(better.is_better_than(&worse));
        assert!(!worse.is_better_than(&better));
    }

    #[test]
    fn test_with_tracking() {
        let rep = ReputationScore::new_with_tracking(
            90,   // completed
            10,   // failed
            3600, // uptime
            4000, // total
            5000, // stake
        );

        assert!((rep.accuracy() - 0.9).abs() < 0.001);
        assert!((rep.uptime() - 0.9).abs() < 0.001);
        assert_eq!(rep.stake(), 5000);
    }

    #[test]
    fn test_json_serialization() {
        let rep = ReputationScore::new(0.95, 0.98, 1000);
        let json = rep.to_json();
        assert!(json.contains("accuracy"));

        let parsed = ReputationScore::from_json(&json).unwrap();
        assert!((parsed.accuracy() - rep.accuracy()).abs() < 0.001);
    }
}
