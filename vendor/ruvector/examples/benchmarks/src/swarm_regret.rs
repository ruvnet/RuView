//! Swarm Controller Regret Tracking
//!
//! Implements sublinear regret metrics for multi-agent control:
//! - Episode-based regret computation
//! - Oracle baseline comparison
//! - Regret curve tracking (R_k/k should decrease)
//!
//! Based on research on sublinear regret in multi-agent and LLM-agent settings

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Episode result from agent execution
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpisodeResult {
    /// Episode number
    pub episode: usize,
    /// Number of puzzles/tasks in episode
    pub num_tasks: usize,
    /// Tasks solved
    pub solved: usize,
    /// Correct solutions
    pub correct: usize,
    /// Total steps taken
    pub total_steps: usize,
    /// Total tool calls
    pub tool_calls: usize,
    /// Total latency in ms
    pub latency_ms: u64,
    /// Agent reward (e.g., accuracy * 100 - steps / 10)
    pub reward: f64,
    /// Oracle reward (best possible performance)
    pub oracle_reward: f64,
}

impl EpisodeResult {
    /// Compute instantaneous regret for this episode
    pub fn regret(&self) -> f64 {
        (self.oracle_reward - self.reward).max(0.0)
    }

    /// Compute accuracy
    pub fn accuracy(&self) -> f64 {
        if self.num_tasks == 0 {
            return 0.0;
        }
        self.correct as f64 / self.num_tasks as f64
    }
}

/// Regret tracker for swarm controller
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegretTracker {
    /// Episode results
    pub episodes: Vec<EpisodeResult>,
    /// Cumulative regret history
    pub cumulative_regret: Vec<f64>,
    /// Average regret history (R_k/k)
    pub average_regret: Vec<f64>,
    /// Window size for moving average
    pub window_size: usize,
    /// Recent rewards for moving average
    recent_rewards: VecDeque<f64>,
}

impl Default for RegretTracker {
    fn default() -> Self {
        Self::new(20)
    }
}

impl RegretTracker {
    /// Create a new regret tracker
    pub fn new(window_size: usize) -> Self {
        Self {
            episodes: Vec::new(),
            cumulative_regret: Vec::new(),
            average_regret: Vec::new(),
            window_size,
            recent_rewards: VecDeque::with_capacity(window_size),
        }
    }

    /// Record an episode result
    pub fn record_episode(&mut self, result: EpisodeResult) {
        let regret = result.regret();
        let k = self.episodes.len() + 1;

        // Update cumulative regret
        let prev_cumulative = self.cumulative_regret.last().copied().unwrap_or(0.0);
        let new_cumulative = prev_cumulative + regret;
        self.cumulative_regret.push(new_cumulative);

        // Update average regret (R_k/k)
        let avg_regret = new_cumulative / k as f64;
        self.average_regret.push(avg_regret);

        // Update moving average window
        self.recent_rewards.push_back(result.reward);
        if self.recent_rewards.len() > self.window_size {
            self.recent_rewards.pop_front();
        }

        self.episodes.push(result);
    }

    /// Get current cumulative regret
    pub fn current_cumulative_regret(&self) -> f64 {
        self.cumulative_regret.last().copied().unwrap_or(0.0)
    }

    /// Get current average regret (R_k/k)
    pub fn current_average_regret(&self) -> f64 {
        self.average_regret.last().copied().unwrap_or(0.0)
    }

    /// Check if regret is sublinear (average regret decreasing)
    pub fn is_sublinear(&self) -> bool {
        if self.average_regret.len() < 5 {
            return true; // Not enough data
        }

        // Check if trend is decreasing
        let n = self.average_regret.len();
        let recent = &self.average_regret[n.saturating_sub(5)..];
        let first = recent[0];
        let last = recent[recent.len() - 1];
        last < first
    }

    /// Get regret trend (slope of average regret)
    pub fn regret_trend(&self) -> f64 {
        if self.average_regret.len() < 2 {
            return 0.0;
        }

        let n = self.average_regret.len();
        let window = n.min(10);
        let recent = &self.average_regret[n - window..];

        // Simple linear regression slope
        let x_mean = (window - 1) as f64 / 2.0;
        let y_mean: f64 = recent.iter().sum::<f64>() / window as f64;

        let mut num = 0.0;
        let mut den = 0.0;
        for (i, y) in recent.iter().enumerate() {
            let x = i as f64;
            num += (x - x_mean) * (y - y_mean);
            den += (x - x_mean) * (x - x_mean);
        }

        if den.abs() < 1e-10 {
            0.0
        } else {
            num / den
        }
    }

    /// Get moving average reward
    pub fn moving_average_reward(&self) -> f64 {
        if self.recent_rewards.is_empty() {
            return 0.0;
        }
        self.recent_rewards.iter().sum::<f64>() / self.recent_rewards.len() as f64
    }

    /// Get summary statistics
    pub fn summary(&self) -> RegretSummary {
        let total_episodes = self.episodes.len();
        let total_regret = self.current_cumulative_regret();
        let avg_regret = self.current_average_regret();
        let trend = self.regret_trend();
        let is_sublinear = self.is_sublinear();

        let avg_accuracy = if total_episodes > 0 {
            self.episodes.iter().map(|e| e.accuracy()).sum::<f64>() / total_episodes as f64
        } else {
            0.0
        };

        let avg_reward = if total_episodes > 0 {
            self.episodes.iter().map(|e| e.reward).sum::<f64>() / total_episodes as f64
        } else {
            0.0
        };

        RegretSummary {
            total_episodes,
            total_regret,
            average_regret: avg_regret,
            regret_trend: trend,
            is_sublinear,
            average_accuracy: avg_accuracy,
            average_reward: avg_reward,
            moving_average_reward: self.moving_average_reward(),
        }
    }
}

/// Regret summary statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegretSummary {
    pub total_episodes: usize,
    pub total_regret: f64,
    pub average_regret: f64,
    pub regret_trend: f64,
    pub is_sublinear: bool,
    pub average_accuracy: f64,
    pub average_reward: f64,
    pub moving_average_reward: f64,
}

/// Oracle baseline for computing optimal rewards
#[derive(Clone, Debug)]
pub struct OracleBaseline {
    /// Perfect accuracy reward
    pub perfect_accuracy_reward: f64,
    /// Step penalty factor
    pub step_penalty: f64,
    /// Minimum steps for optimal solution
    pub min_steps: usize,
}

impl Default for OracleBaseline {
    fn default() -> Self {
        Self {
            perfect_accuracy_reward: 100.0,
            step_penalty: 0.1,
            min_steps: 5,
        }
    }
}

impl OracleBaseline {
    /// Compute oracle reward for a task set
    pub fn compute_reward(&self, num_tasks: usize) -> f64 {
        // Oracle solves all tasks with minimum steps
        let accuracy_reward = self.perfect_accuracy_reward;
        let step_cost = (self.min_steps * num_tasks) as f64 * self.step_penalty;
        accuracy_reward - step_cost
    }
}

/// Swarm controller with regret tracking
pub struct SwarmController {
    /// Regret tracker
    pub regret: RegretTracker,
    /// Oracle baseline
    pub oracle: OracleBaseline,
    /// Current episode number
    pub current_episode: usize,
    /// Tasks per episode
    pub tasks_per_episode: usize,
}

impl Default for SwarmController {
    fn default() -> Self {
        Self::new(20)
    }
}

impl SwarmController {
    /// Create a new swarm controller
    pub fn new(tasks_per_episode: usize) -> Self {
        Self {
            regret: RegretTracker::new(20),
            oracle: OracleBaseline::default(),
            current_episode: 0,
            tasks_per_episode,
        }
    }

    /// Start a new episode
    pub fn start_episode(&mut self) {
        self.current_episode += 1;
    }

    /// Record episode completion
    pub fn complete_episode(
        &mut self,
        solved: usize,
        correct: usize,
        total_steps: usize,
        tool_calls: usize,
        latency_ms: u64,
    ) {
        let num_tasks = self.tasks_per_episode;

        // Compute agent reward
        let accuracy = if num_tasks > 0 {
            correct as f64 / num_tasks as f64
        } else {
            0.0
        };
        let agent_reward = accuracy * self.oracle.perfect_accuracy_reward
            - total_steps as f64 * self.oracle.step_penalty;

        // Compute oracle reward
        let oracle_reward = self.oracle.compute_reward(num_tasks);

        let result = EpisodeResult {
            episode: self.current_episode,
            num_tasks,
            solved,
            correct,
            total_steps,
            tool_calls,
            latency_ms,
            reward: agent_reward,
            oracle_reward,
        };

        self.regret.record_episode(result);
    }

    /// Get current regret status
    pub fn status(&self) -> SwarmStatus {
        let summary = self.regret.summary();
        SwarmStatus {
            episode: self.current_episode,
            cumulative_regret: summary.total_regret,
            average_regret: summary.average_regret,
            is_improving: summary.is_sublinear,
            accuracy: summary.average_accuracy,
        }
    }
}

/// Swarm controller status
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwarmStatus {
    pub episode: usize,
    pub cumulative_regret: f64,
    pub average_regret: f64,
    pub is_improving: bool,
    pub accuracy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regret_tracking() {
        let mut tracker = RegretTracker::new(10);

        // Simulate improving performance
        for i in 0..10 {
            let accuracy = 0.5 + 0.05 * i as f64;
            let result = EpisodeResult {
                episode: i + 1,
                num_tasks: 20,
                solved: (20.0 * accuracy) as usize,
                correct: (20.0 * accuracy) as usize,
                total_steps: 100 - i * 5,
                tool_calls: 20,
                latency_ms: 1000,
                reward: accuracy * 100.0 - (100 - i * 5) as f64 * 0.1,
                oracle_reward: 99.0,
            };
            tracker.record_episode(result);
        }

        assert!(tracker.is_sublinear());
        assert!(tracker.regret_trend() < 0.0);
    }

    #[test]
    fn test_swarm_controller() {
        let mut controller = SwarmController::new(20);

        for _ in 0..5 {
            controller.start_episode();
            controller.complete_episode(18, 17, 80, 20, 500);
        }

        let status = controller.status();
        assert_eq!(status.episode, 5);
        assert!(status.accuracy > 0.8);
    }
}
