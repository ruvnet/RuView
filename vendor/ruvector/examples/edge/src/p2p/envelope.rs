//! Message Envelopes - Signed and Encrypted Messages
//!
//! Security principles:
//! - All messages are signed with Ed25519
//! - Signatures cover canonical representation of all fields
//! - TaskReceipt includes full execution binding

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use crate::p2p::crypto::{EncryptedPayload, CanonicalJson, CryptoV2};

/// Signed message envelope for P2P communication
///
/// Design:
/// - Header fields are signed but not encrypted
/// - Payload is encrypted with swarm key (for broadcast) or session key (for direct)
/// - Sender identity verified via registry, NOT from this envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedEnvelope {
    // Header (signed but not encrypted)
    pub message_id: String,
    pub topic: String,
    pub timestamp: u64,
    pub sender_id: String,
    #[serde(with = "hex::serde")]
    pub payload_hash: [u8; 32],
    pub nonce: String,
    pub counter: u64,

    // Signature covers canonical representation of all header fields
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],

    // Encrypted payload (swarm key or session key)
    pub encrypted: EncryptedPayload,
}

impl SignedEnvelope {
    /// Create canonical representation of header for signing
    /// Keys are sorted alphabetically for deterministic output
    pub fn canonical_header(&self) -> String {
        // Create a struct with sorted fields for canonical serialization
        let header = serde_json::json!({
            "counter": self.counter,
            "message_id": self.message_id,
            "nonce": self.nonce,
            "payload_hash": hex::encode(self.payload_hash),
            "sender_id": self.sender_id,
            "timestamp": self.timestamp,
            "topic": self.topic,
        });
        CanonicalJson::stringify(&header)
    }

    /// Create unsigned envelope (for signing externally)
    pub fn new_unsigned(
        message_id: String,
        topic: String,
        sender_id: String,
        payload: &[u8],
        nonce: String,
        counter: u64,
        encrypted: EncryptedPayload,
    ) -> Self {
        Self {
            message_id,
            topic,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            sender_id,
            payload_hash: CryptoV2::hash(payload),
            nonce,
            counter,
            signature: [0u8; 64],
            encrypted,
        }
    }
}

/// Task execution envelope with resource budgets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEnvelope {
    pub task_id: String,
    pub module_cid: String,     // WASM module location
    pub entrypoint: String,      // Function to call
    pub input_cid: String,       // Input data location
    #[serde(with = "hex::serde")]
    pub output_schema_hash: [u8; 32],

    /// Resource budgets for sandbox
    pub budgets: TaskBudgets,

    /// Requester info
    pub requester: String,
    pub deadline: u64,
    pub priority: u8,

    /// Canonical hash of this envelope (for receipts)
    #[serde(with = "hex::serde")]
    pub envelope_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBudgets {
    pub fuel_limit: u64,      // Wasmtime fuel
    pub memory_mb: u32,       // Max memory in MB
    pub timeout_ms: u64,      // Max execution time
}

impl TaskEnvelope {
    /// Create task envelope with computed hash
    pub fn new(
        task_id: String,
        module_cid: String,
        entrypoint: String,
        input_cid: String,
        output_schema_hash: [u8; 32],
        budgets: TaskBudgets,
        requester: String,
        deadline: u64,
        priority: u8,
    ) -> Self {
        let mut envelope = Self {
            task_id,
            module_cid,
            entrypoint,
            input_cid,
            output_schema_hash,
            budgets,
            requester,
            deadline,
            priority,
            envelope_hash: [0u8; 32],
        };

        // Compute envelope hash (excluding envelope_hash field)
        envelope.envelope_hash = envelope.compute_hash();
        envelope
    }

    /// Compute canonical hash of envelope (excluding envelope_hash itself)
    fn compute_hash(&self) -> [u8; 32] {
        let for_hash = serde_json::json!({
            "budgets": {
                "fuel_limit": self.budgets.fuel_limit,
                "memory_mb": self.budgets.memory_mb,
                "timeout_ms": self.budgets.timeout_ms,
            },
            "deadline": self.deadline,
            "entrypoint": self.entrypoint,
            "input_cid": self.input_cid,
            "module_cid": self.module_cid,
            "output_schema_hash": hex::encode(self.output_schema_hash),
            "priority": self.priority,
            "requester": self.requester,
            "task_id": self.task_id,
        });
        let canonical = CanonicalJson::stringify(&for_hash);
        CryptoV2::hash(canonical.as_bytes())
    }
}

/// Task result receipt with full execution binding
///
/// Security: Signature covers ALL fields to prevent tampering
/// Including binding to original TaskEnvelope for traceability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskReceipt {
    // Core result
    pub task_id: String,
    pub executor: String,
    pub result_cid: String,
    pub status: TaskStatus,

    // Resource usage
    pub fuel_used: u64,
    pub memory_peak_mb: u32,
    pub execution_ms: u64,

    // Execution binding (proves this receipt is for this specific execution)
    #[serde(with = "hex::serde")]
    pub input_hash: [u8; 32],
    #[serde(with = "hex::serde")]
    pub output_hash: [u8; 32],
    #[serde(with = "hex::serde")]
    pub module_hash: [u8; 32],
    pub start_timestamp: u64,
    pub end_timestamp: u64,

    // TaskEnvelope binding (proves this receipt matches original task)
    pub module_cid: String,
    pub input_cid: String,
    pub entrypoint: String,
    #[serde(with = "hex::serde")]
    pub output_schema_hash: [u8; 32],
    #[serde(with = "hex::serde")]
    pub task_envelope_hash: [u8; 32],

    // Signature covers ALL fields above
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Success,
    Error,
    Timeout,
    OutOfMemory,
}

impl TaskReceipt {
    /// Create canonical representation for signing
    /// Includes ALL fields for full execution binding
    pub fn canonical_for_signing(&self) -> String {
        let for_signing = serde_json::json!({
            "end_timestamp": self.end_timestamp,
            "entrypoint": self.entrypoint,
            "execution_ms": self.execution_ms,
            "executor": self.executor,
            "fuel_used": self.fuel_used,
            "input_cid": self.input_cid,
            "input_hash": hex::encode(self.input_hash),
            "memory_peak_mb": self.memory_peak_mb,
            "module_cid": self.module_cid,
            "module_hash": hex::encode(self.module_hash),
            "output_hash": hex::encode(self.output_hash),
            "output_schema_hash": hex::encode(self.output_schema_hash),
            "result_cid": self.result_cid,
            "start_timestamp": self.start_timestamp,
            "status": format!("{:?}", self.status),
            "task_envelope_hash": hex::encode(self.task_envelope_hash),
            "task_id": self.task_id,
        });
        CanonicalJson::stringify(&for_signing)
    }

    /// Create unsigned receipt (for signing externally)
    pub fn new_unsigned(
        task: &TaskEnvelope,
        executor: String,
        result_cid: String,
        status: TaskStatus,
        fuel_used: u64,
        memory_peak_mb: u32,
        execution_ms: u64,
        input_hash: [u8; 32],
        output_hash: [u8; 32],
        module_hash: [u8; 32],
        start_timestamp: u64,
        end_timestamp: u64,
    ) -> Self {
        Self {
            task_id: task.task_id.clone(),
            executor,
            result_cid,
            status,
            fuel_used,
            memory_peak_mb,
            execution_ms,
            input_hash,
            output_hash,
            module_hash,
            start_timestamp,
            end_timestamp,
            module_cid: task.module_cid.clone(),
            input_cid: task.input_cid.clone(),
            entrypoint: task.entrypoint.clone(),
            output_schema_hash: task.output_schema_hash,
            task_envelope_hash: task.envelope_hash,
            signature: [0u8; 64],
        }
    }
}

/// Signaling message for WebRTC (via GUN)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalingMessage {
    pub signal_type: SignalType,
    pub from: String,
    pub to: String,
    pub payload: String,
    #[serde(with = "hex::serde")]
    pub payload_hash: [u8; 32],
    pub timestamp: u64,
    pub expires_at: u64,
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SignalType {
    Offer,
    Answer,
    Ice,
}

impl SignalingMessage {
    /// Create canonical representation for signing
    /// Does NOT include sender_pubkey - verified via registry
    pub fn canonical_for_signing(&self) -> String {
        let for_signing = serde_json::json!({
            "expires_at": self.expires_at,
            "from": self.from,
            "payload_hash": hex::encode(self.payload_hash),
            "signal_type": format!("{:?}", self.signal_type),
            "timestamp": self.timestamp,
            "to": self.to,
        });
        CanonicalJson::stringify(&for_signing)
    }

    /// Create unsigned signaling message
    pub fn new_unsigned(
        signal_type: SignalType,
        from: String,
        to: String,
        payload: String,
        ttl_ms: u64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        Self {
            signal_type,
            from,
            to,
            payload_hash: CryptoV2::hash(payload.as_bytes()),
            payload,
            timestamp: now,
            expires_at: now + ttl_ms,
            signature: [0u8; 64],
        }
    }

    /// Check if signal is expired
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp_millis() as u64;
        now > self.expires_at
    }
}

/// Artifact pointer (small metadata that goes to GUN)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactPointer {
    pub artifact_type: ArtifactType,
    pub agent_id: String,
    pub cid: String,
    pub version: u32,
    #[serde(with = "hex::serde")]
    pub schema_hash: [u8; 8],
    pub dimensions: String,
    #[serde(with = "hex::serde")]
    pub checksum: [u8; 16],
    pub timestamp: u64,
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArtifactType {
    QTable,
    MemoryVectors,
    ModelWeights,
    Trajectory,
}

impl ArtifactPointer {
    /// Create canonical representation for signing
    pub fn canonical_for_signing(&self) -> String {
        let for_signing = serde_json::json!({
            "agent_id": self.agent_id,
            "artifact_type": format!("{:?}", self.artifact_type),
            "checksum": hex::encode(self.checksum),
            "cid": self.cid,
            "dimensions": self.dimensions,
            "schema_hash": hex::encode(self.schema_hash),
            "timestamp": self.timestamp,
            "version": self.version,
        });
        CanonicalJson::stringify(&for_signing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_canonical_header() {
        let encrypted = EncryptedPayload {
            ciphertext: vec![1, 2, 3],
            iv: [0u8; 12],
            tag: [0u8; 16],
        };

        let envelope = SignedEnvelope::new_unsigned(
            "msg-001".to_string(),
            "test-topic".to_string(),
            "sender-001".to_string(),
            b"test payload",
            "abc123".to_string(),
            1,
            encrypted,
        );

        let canonical1 = envelope.canonical_header();
        let canonical2 = envelope.canonical_header();

        // Must be deterministic
        assert_eq!(canonical1, canonical2);

        // Must contain all fields
        assert!(canonical1.contains("msg-001"));
        assert!(canonical1.contains("test-topic"));
        assert!(canonical1.contains("sender-001"));
    }

    #[test]
    fn test_task_receipt_includes_all_binding_fields() {
        let task = TaskEnvelope::new(
            "task-001".to_string(),
            "local:abc123".to_string(),
            "process".to_string(),
            "local:input123".to_string(),
            [0u8; 32],
            TaskBudgets {
                fuel_limit: 1000000,
                memory_mb: 128,
                timeout_ms: 30000,
            },
            "requester-001".to_string(),
            chrono::Utc::now().timestamp_millis() as u64 + 60000,
            1,
        );

        let receipt = TaskReceipt::new_unsigned(
            &task,
            "executor-001".to_string(),
            "local:result123".to_string(),
            TaskStatus::Success,
            500000,
            64,
            1500,
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            1000,
            2500,
        );

        let canonical = receipt.canonical_for_signing();

        // Must include execution binding fields
        assert!(canonical.contains("input_hash"));
        assert!(canonical.contains("output_hash"));
        assert!(canonical.contains("module_hash"));

        // Must include task envelope binding
        assert!(canonical.contains("module_cid"));
        assert!(canonical.contains("input_cid"));
        assert!(canonical.contains("entrypoint"));
        assert!(canonical.contains("task_envelope_hash"));
    }

    #[test]
    fn test_signaling_message_no_pubkey_in_signature() {
        let signal = SignalingMessage::new_unsigned(
            SignalType::Offer,
            "alice".to_string(),
            "bob".to_string(),
            "sdp data here".to_string(),
            60000,
        );

        let canonical = signal.canonical_for_signing();

        // Should NOT contain any pubkey field
        assert!(!canonical.contains("pubkey"));
        assert!(!canonical.contains("public_key"));
    }
}
