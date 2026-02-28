//! Central cognitive loop: perceive -> think -> act -> learn.
//!
//! The [`CognitiveCore`] drives the robot's high-level autonomy by filtering
//! percepts, selecting actions through utility-based reasoning, and
//! incorporating feedback to improve future decisions.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Operating mode of the cognitive system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CognitiveMode {
    /// Fast stimulus-response behaviour.
    Reactive,
    /// Goal-directed planning and reasoning.
    Deliberative,
    /// Override mode for safety-critical situations.
    Emergency,
}

/// Current phase of the cognitive loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CognitiveState {
    Idle,
    Perceiving,
    Thinking,
    Acting,
    Learning,
}

/// The kind of action the robot can execute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ActionType {
    Move([f64; 3]),
    Rotate(f64),
    Grasp(String),
    Release,
    Speak(String),
    Wait(u64),
}

// ---------------------------------------------------------------------------
// Data structs
// ---------------------------------------------------------------------------

/// A command to execute an action with priority and confidence metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionCommand {
    pub action: ActionType,
    pub priority: u8,
    pub confidence: f64,
}

/// A single percept received from a sensor or subsystem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Percept {
    pub source: String,
    pub data: Vec<f64>,
    pub confidence: f64,
    pub timestamp: i64,
}

/// A decision produced by the think phase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub action: ActionCommand,
    pub reasoning: String,
    pub utility: f64,
}

/// Feedback from the environment after executing an action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub success: bool,
    pub reward: f64,
    pub description: String,
}

/// Configuration for the cognitive core.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitiveConfig {
    pub mode: CognitiveMode,
    pub attention_threshold: f64,
    pub learning_rate: f64,
    pub max_percepts: usize,
}

impl Default for CognitiveConfig {
    fn default() -> Self {
        Self {
            mode: CognitiveMode::Reactive,
            attention_threshold: 0.5,
            learning_rate: 0.01,
            max_percepts: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// Core
// ---------------------------------------------------------------------------

/// Central cognitive controller implementing perceive-think-act-learn.
#[derive(Debug, Clone)]
pub struct CognitiveCore {
    state: CognitiveState,
    config: CognitiveConfig,
    percept_buffer: VecDeque<Percept>,
    decision_history: Vec<Decision>,
    cumulative_reward: f64,
}

impl CognitiveCore {
    /// Create a new cognitive core with the given configuration.
    pub fn new(config: CognitiveConfig) -> Self {
        Self {
            state: CognitiveState::Idle,
            config,
            percept_buffer: VecDeque::new(),
            decision_history: Vec::new(),
            cumulative_reward: 0.0,
        }
    }

    /// Ingest a percept and transition to the Perceiving state.
    ///
    /// Percepts below the attention threshold are silently dropped.
    /// When the buffer exceeds `max_percepts`, the oldest entry is removed.
    pub fn perceive(&mut self, percept: Percept) -> CognitiveState {
        self.state = CognitiveState::Perceiving;

        if percept.confidence < self.config.attention_threshold {
            return self.state;
        }

        if self.percept_buffer.len() >= self.config.max_percepts {
            self.percept_buffer.pop_front(); // O(1) with VecDeque
        }
        self.percept_buffer.push_back(percept);
        self.state
    }

    /// Deliberate over buffered percepts and produce a decision.
    ///
    /// Returns `None` when no percepts are available.
    pub fn think(&mut self) -> Option<Decision> {
        self.state = CognitiveState::Thinking;

        if self.percept_buffer.is_empty() {
            return None;
        }

        // Simple heuristic: pick the most confident percept and derive an action.
        let best = self
            .percept_buffer
            .iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))?;

        let action_type = if best.data.len() >= 3 {
            ActionType::Move([best.data[0], best.data[1], best.data[2]])
        } else {
            ActionType::Wait(100)
        };

        let decision = Decision {
            action: ActionCommand {
                action: action_type,
                priority: match self.config.mode {
                    CognitiveMode::Emergency => 255,
                    CognitiveMode::Deliberative => 128,
                    CognitiveMode::Reactive => 64,
                },
                confidence: best.confidence,
            },
            reasoning: format!("Best percept from '{}' (conf={:.2})", best.source, best.confidence),
            utility: best.confidence,
        };

        self.decision_history.push(decision.clone());
        Some(decision)
    }

    /// Convert a decision into an executable action command.
    pub fn act(&mut self, decision: Decision) -> ActionCommand {
        self.state = CognitiveState::Acting;
        decision.action
    }

    /// Incorporate feedback from the environment to improve future behaviour.
    pub fn learn(&mut self, outcome: Outcome) {
        self.state = CognitiveState::Learning;
        self.cumulative_reward += outcome.reward * self.config.learning_rate;

        // Adjust attention threshold based on success/failure.
        if outcome.success {
            self.config.attention_threshold =
                (self.config.attention_threshold - 0.01).max(0.1);
        } else {
            self.config.attention_threshold =
                (self.config.attention_threshold + 0.01).min(0.9);
        }

        // Clear processed percepts so the next cycle starts fresh.
        self.percept_buffer.clear();
        self.state = CognitiveState::Idle;
    }

    /// Current cognitive state.
    pub fn state(&self) -> CognitiveState {
        self.state
    }

    /// Current operating mode.
    pub fn mode(&self) -> CognitiveMode {
        self.config.mode
    }

    /// Number of percepts currently buffered.
    pub fn percept_count(&self) -> usize {
        self.percept_buffer.len()
    }

    /// Number of decisions made so far.
    pub fn decision_count(&self) -> usize {
        self.decision_history.len()
    }

    /// Accumulated reward scaled by learning rate.
    pub fn cumulative_reward(&self) -> f64 {
        self.cumulative_reward
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_core() -> CognitiveCore {
        CognitiveCore::new(CognitiveConfig::default())
    }

    fn make_percept(source: &str, data: Vec<f64>, confidence: f64) -> Percept {
        Percept {
            source: source.into(),
            data,
            confidence,
            timestamp: 1000,
        }
    }

    #[test]
    fn test_initial_state() {
        let core = default_core();
        assert_eq!(core.state(), CognitiveState::Idle);
        assert_eq!(core.mode(), CognitiveMode::Reactive);
        assert_eq!(core.percept_count(), 0);
    }

    #[test]
    fn test_perceive_above_threshold() {
        let mut core = default_core();
        let state = core.perceive(make_percept("lidar", vec![1.0, 2.0, 3.0], 0.8));
        assert_eq!(state, CognitiveState::Perceiving);
        assert_eq!(core.percept_count(), 1);
    }

    #[test]
    fn test_perceive_below_threshold() {
        let mut core = default_core();
        core.perceive(make_percept("lidar", vec![1.0], 0.1));
        assert_eq!(core.percept_count(), 0);
    }

    #[test]
    fn test_think_produces_decision() {
        let mut core = default_core();
        core.perceive(make_percept("cam", vec![1.0, 2.0, 3.0], 0.9));
        let decision = core.think();
        assert!(decision.is_some());
        let d = decision.unwrap();
        assert_eq!(d.action.priority, 64); // Reactive mode
        assert_eq!(core.decision_count(), 1);
    }

    #[test]
    fn test_think_empty_buffer() {
        let mut core = default_core();
        assert!(core.think().is_none());
    }

    #[test]
    fn test_act_returns_command() {
        let mut core = default_core();
        core.perceive(make_percept("cam", vec![1.0, 2.0, 3.0], 0.9));
        let decision = core.think().unwrap();
        let cmd = core.act(decision);
        assert_eq!(cmd.action, ActionType::Move([1.0, 2.0, 3.0]));
        assert_eq!(core.state(), CognitiveState::Acting);
    }

    #[test]
    fn test_learn_adjusts_threshold() {
        let mut core = default_core();
        let initial = core.config.attention_threshold;
        core.learn(Outcome {
            success: true,
            reward: 1.0,
            description: "ok".into(),
        });
        assert!(core.config.attention_threshold < initial);
        assert_eq!(core.state(), CognitiveState::Idle);
    }

    #[test]
    fn test_learn_failure_raises_threshold() {
        let mut core = default_core();
        let initial = core.config.attention_threshold;
        core.learn(Outcome {
            success: false,
            reward: -1.0,
            description: "fail".into(),
        });
        assert!(core.config.attention_threshold > initial);
    }

    #[test]
    fn test_emergency_priority() {
        let mut core = CognitiveCore::new(CognitiveConfig {
            mode: CognitiveMode::Emergency,
            ..CognitiveConfig::default()
        });
        core.perceive(make_percept("ir", vec![5.0, 6.0, 7.0], 0.99));
        let d = core.think().unwrap();
        assert_eq!(d.action.priority, 255);
    }

    #[test]
    fn test_percept_buffer_overflow() {
        let mut core = CognitiveCore::new(CognitiveConfig {
            max_percepts: 2,
            ..CognitiveConfig::default()
        });
        core.perceive(make_percept("a", vec![1.0], 0.8));
        core.perceive(make_percept("b", vec![2.0], 0.8));
        core.perceive(make_percept("c", vec![3.0], 0.8));
        assert_eq!(core.percept_count(), 2);
    }
}
