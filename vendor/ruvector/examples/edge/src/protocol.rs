//! Swarm communication protocol
//!
//! Defines message types and serialization for agent communication.

use crate::intelligence::LearningState;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Message types for swarm communication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Agent joining the swarm
    Join,
    /// Agent leaving the swarm
    Leave,
    /// Heartbeat/ping
    Ping,
    /// Heartbeat response
    Pong,
    /// Sync learning patterns
    SyncPatterns,
    /// Request patterns from peer
    RequestPatterns,
    /// Sync vector memories
    SyncMemories,
    /// Request memories from peer
    RequestMemories,
    /// Broadcast task to swarm
    BroadcastTask,
    /// Task result
    TaskResult,
    /// Coordinator election
    Election,
    /// Coordinator announcement
    Coordinator,
    /// Error message
    Error,
}

/// Swarm message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmMessage {
    pub id: String,
    pub message_type: MessageType,
    pub sender_id: String,
    pub recipient_id: Option<String>, // None = broadcast
    pub payload: MessagePayload,
    pub timestamp: u64,
    pub ttl: u32, // Time-to-live in hops
}

impl SwarmMessage {
    /// Create new message
    pub fn new(message_type: MessageType, sender_id: &str, payload: MessagePayload) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            message_type,
            sender_id: sender_id.to_string(),
            recipient_id: None,
            payload,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            ttl: 10,
        }
    }

    /// Create directed message
    pub fn directed(
        message_type: MessageType,
        sender_id: &str,
        recipient_id: &str,
        payload: MessagePayload,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            message_type,
            sender_id: sender_id.to_string(),
            recipient_id: Some(recipient_id.to_string()),
            payload,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            ttl: 10,
        }
    }

    /// Create join message
    pub fn join(agent_id: &str, role: &str, capabilities: Vec<String>) -> Self {
        Self::new(
            MessageType::Join,
            agent_id,
            MessagePayload::Join(JoinPayload {
                agent_role: role.to_string(),
                capabilities,
                version: env!("CARGO_PKG_VERSION").to_string(),
            }),
        )
    }

    /// Create leave message
    pub fn leave(agent_id: &str) -> Self {
        Self::new(MessageType::Leave, agent_id, MessagePayload::Empty)
    }

    /// Create ping message
    pub fn ping(agent_id: &str) -> Self {
        Self::new(MessageType::Ping, agent_id, MessagePayload::Empty)
    }

    /// Create pong response
    pub fn pong(agent_id: &str) -> Self {
        Self::new(MessageType::Pong, agent_id, MessagePayload::Empty)
    }

    /// Create pattern sync message
    pub fn sync_patterns(agent_id: &str, state: LearningState) -> Self {
        Self::new(
            MessageType::SyncPatterns,
            agent_id,
            MessagePayload::Patterns(PatternsPayload {
                state,
                compressed: false,
            }),
        )
    }

    /// Create pattern request message
    pub fn request_patterns(agent_id: &str, since_version: u64) -> Self {
        Self::new(
            MessageType::RequestPatterns,
            agent_id,
            MessagePayload::Request(RequestPayload {
                since_version,
                max_entries: 1000,
            }),
        )
    }

    /// Create task broadcast message
    pub fn broadcast_task(agent_id: &str, task: TaskPayload) -> Self {
        Self::new(MessageType::BroadcastTask, agent_id, MessagePayload::Task(task))
    }

    /// Create error message
    pub fn error(agent_id: &str, error: &str) -> Self {
        Self::new(
            MessageType::Error,
            agent_id,
            MessagePayload::Error(ErrorPayload {
                code: "ERROR".to_string(),
                message: error.to_string(),
            }),
        )
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }

    /// Check if message is expired (based on timestamp)
    pub fn is_expired(&self, max_age_ms: u64) -> bool {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        now - self.timestamp > max_age_ms
    }

    /// Decrement TTL for forwarding
    pub fn decrement_ttl(&mut self) -> bool {
        if self.ttl > 0 {
            self.ttl -= 1;
            true
        } else {
            false
        }
    }
}

/// Message payload variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessagePayload {
    Empty,
    Join(JoinPayload),
    Patterns(PatternsPayload),
    Memories(MemoriesPayload),
    Request(RequestPayload),
    Task(TaskPayload),
    TaskResult(TaskResultPayload),
    Election(ElectionPayload),
    Error(ErrorPayload),
    Raw(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinPayload {
    pub agent_role: String,
    pub capabilities: Vec<String>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternsPayload {
    pub state: LearningState,
    pub compressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoriesPayload {
    pub entries: Vec<u8>, // Compressed vector entries
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestPayload {
    pub since_version: u64,
    pub max_entries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPayload {
    pub task_id: String,
    pub task_type: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub priority: u8,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResultPayload {
    pub task_id: String,
    pub success: bool,
    pub result: serde_json::Value,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectionPayload {
    pub candidate_id: String,
    pub priority: u64,
    pub term: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = SwarmMessage::join("agent-001", "worker", vec!["compute".to_string()]);

        let bytes = msg.to_bytes().unwrap();
        let decoded = SwarmMessage::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.sender_id, "agent-001");
        assert!(matches!(decoded.message_type, MessageType::Join));
    }

    #[test]
    fn test_ttl_decrement() {
        let mut msg = SwarmMessage::ping("agent-001");
        assert_eq!(msg.ttl, 10);

        assert!(msg.decrement_ttl());
        assert_eq!(msg.ttl, 9);

        msg.ttl = 0;
        assert!(!msg.decrement_ttl());
    }
}
