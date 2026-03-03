//! Core types for the lean agentic system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Context for agent decision-making
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Context {
    /// Current conversation history
    pub history: Vec<String>,

    /// User preferences learned over time
    pub preferences: HashMap<String, f64>,

    /// Session metadata
    pub session_id: String,

    /// Environment state
    pub environment: HashMap<String, serde_json::Value>,

    /// Timestamp
    pub timestamp: i64,
}

impl Context {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            timestamp: chrono::Utc::now().timestamp(),
            ..Default::default()
        }
    }

    pub fn add_message(&mut self, message: String) {
        self.history.push(message);
        self.timestamp = chrono::Utc::now().timestamp();
    }

    pub fn set_preference(&mut self, key: String, value: f64) {
        self.preferences.insert(key, value);
    }
}

/// Agent state representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Current goals
    pub goals: Vec<Goal>,

    /// Beliefs about the world
    pub beliefs: HashMap<String, Belief>,

    /// Current intentions
    pub intentions: Vec<Intention>,

    /// Learned policies
    pub policies: Vec<Policy>,

    /// Confidence scores
    pub confidence: f64,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            goals: Vec::new(),
            beliefs: HashMap::new(),
            intentions: Vec::new(),
            policies: Vec::new(),
            confidence: 1.0,
        }
    }
}

/// A goal the agent is trying to achieve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub description: String,
    pub priority: f64,
    pub achieved: bool,
}

/// A belief about the world state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Belief {
    pub proposition: String,
    pub confidence: f64,
    pub evidence: Vec<String>,
}

/// An intention to perform actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intention {
    pub goal_id: String,
    pub action_sequence: Vec<String>,
    pub committed: bool,
}

/// A learned policy for decision-making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub condition: String,
    pub action: String,
    pub expected_reward: f64,
    pub usage_count: u64,
}

/// Reward signal for learning
pub type Reward = f64;

/// Stream message with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub content: String,
    pub metadata: HashMap<String, serde_json::Value>,
    pub timestamp: i64,
    pub sender: String,
}

impl StreamMessage {
    pub fn new(content: String, sender: String) -> Self {
        Self {
            content,
            sender,
            timestamp: chrono::Utc::now().timestamp(),
            metadata: HashMap::new(),
        }
    }
}
