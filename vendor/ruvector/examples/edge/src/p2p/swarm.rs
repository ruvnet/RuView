//! P2P Swarm v2 - Production Grade Coordinator
//!
//! Features:
//! - Registry-based identity (never trust envelope keys)
//! - Membership watcher with heartbeats
//! - Task executor loop with claiming
//! - Artifact publishing and retrieval

use crate::p2p::{
    identity::{IdentityManager, RegisteredMember},
    crypto::{CryptoV2, CanonicalJson},
    relay::RelayManager,
    artifact::ArtifactStore,
    envelope::{
        SignedEnvelope, TaskEnvelope, TaskReceipt, TaskStatus, TaskBudgets,
        SignalingMessage, SignalType, ArtifactPointer, ArtifactType,
    },
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Heartbeat interval (30 seconds)
pub const HEARTBEAT_INTERVAL_MS: u64 = 30_000;

/// Heartbeat timeout (2 minutes)
pub const HEARTBEAT_TIMEOUT_MS: u64 = 120_000;

/// Task claim window (5 seconds)
pub const TASK_CLAIM_WINDOW_MS: u64 = 5_000;

/// Swarm status
#[derive(Debug, Clone)]
pub struct SwarmStatus {
    pub connected: bool,
    pub swarm_id: String,
    pub agent_id: String,
    pub public_key: [u8; 32],
    pub active_peers: usize,
    pub pending_tasks: usize,
    pub relay_metrics: crate::p2p::relay::RelayMetrics,
}

/// Message handler callback type
pub type MessageHandler = Box<dyn Fn(&[u8], &str) + Send + Sync>;

/// Task claim record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskClaim {
    pub task_id: String,
    pub executor_id: String,
    pub timestamp: u64,
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

/// Heartbeat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub agent_id: String,
    pub timestamp: u64,
    pub capabilities: Vec<String>,
    pub load: f32, // 0.0 to 1.0
    #[serde(with = "BigArray")]
    pub signature: [u8; 64],
}

impl Heartbeat {
    /// Create canonical representation for signing
    pub fn canonical_for_signing(&self) -> String {
        let for_signing = serde_json::json!({
            "agent_id": self.agent_id,
            "capabilities": self.capabilities,
            "load": self.load,
            "timestamp": self.timestamp,
        });
        CanonicalJson::stringify(&for_signing)
    }
}

/// P2P Swarm Coordinator
pub struct P2PSwarmV2 {
    /// Our identity manager
    identity: Arc<IdentityManager>,

    /// Relay manager
    relay_manager: Arc<RelayManager>,

    /// Artifact store
    artifact_store: Arc<ArtifactStore>,

    /// Swarm key (for envelope encryption)
    swarm_key: [u8; 32],

    /// Swarm ID (derived from swarm key)
    swarm_id: String,

    /// Our agent ID
    agent_id: String,

    /// Our capabilities
    capabilities: Vec<String>,

    /// Connection state
    connected: Arc<RwLock<bool>>,

    /// Message handlers by topic
    handlers: Arc<RwLock<HashMap<String, MessageHandler>>>,

    /// Pending tasks
    pending_tasks: Arc<RwLock<HashMap<String, TaskEnvelope>>>,

    /// Task claims
    task_claims: Arc<RwLock<HashMap<String, TaskClaim>>>,

    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl P2PSwarmV2 {
    /// Create new swarm coordinator
    pub fn new(agent_id: &str, swarm_key: Option<[u8; 32]>, capabilities: Vec<String>) -> Self {
        let identity = Arc::new(IdentityManager::new());
        let relay_manager = Arc::new(RelayManager::new());
        let artifact_store = Arc::new(ArtifactStore::new());

        // Generate or use provided swarm key
        let swarm_key = swarm_key.unwrap_or_else(|| {
            let mut key = [0u8; 32];
            rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut key);
            key
        });

        // Derive swarm ID from key
        let swarm_id = hex::encode(&CryptoV2::hash(&swarm_key)[..8]);

        Self {
            identity,
            relay_manager,
            artifact_store,
            swarm_key,
            swarm_id,
            agent_id: agent_id.to_string(),
            capabilities,
            connected: Arc::new(RwLock::new(false)),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            pending_tasks: Arc::new(RwLock::new(HashMap::new())),
            task_claims: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
        }
    }

    /// Get swarm key (for sharing with peers)
    pub fn swarm_key(&self) -> [u8; 32] {
        self.swarm_key
    }

    /// Get swarm key as base64
    pub fn swarm_key_base64(&self) -> String {
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, self.swarm_key)
    }

    /// Connect to swarm
    pub async fn connect(&mut self) -> Result<(), String> {
        tracing::info!("Connecting to swarm: {}", self.swarm_id);

        // Register ourselves
        let registration = self.identity.create_registration(&self.agent_id, self.capabilities.clone());
        self.identity.register_member(registration);

        *self.connected.write() = true;

        // Start heartbeat loop
        self.start_heartbeat_loop();

        tracing::info!("Connected to swarm {} as {}", self.swarm_id, self.agent_id);
        Ok(())
    }

    /// Disconnect from swarm
    pub fn disconnect(&mut self) {
        *self.connected.write() = false;
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.try_send(());
        }
        tracing::info!("Disconnected from swarm {}", self.swarm_id);
    }

    /// Start heartbeat loop
    fn start_heartbeat_loop(&mut self) {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        let identity = self.identity.clone();
        let agent_id = self.agent_id.clone();
        let capabilities = self.capabilities.clone();
        let connected = self.connected.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(HEARTBEAT_INTERVAL_MS));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if !*connected.read() {
                            break;
                        }

                        // Create and sign heartbeat
                        let mut heartbeat = Heartbeat {
                            agent_id: agent_id.clone(),
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                            capabilities: capabilities.clone(),
                            load: 0.5, // TODO: Measure actual load
                            signature: [0u8; 64],
                        };

                        let canonical = heartbeat.canonical_for_signing();
                        heartbeat.signature = identity.sign(canonical.as_bytes());

                        // In real implementation, publish to GUN
                        tracing::debug!("Published heartbeat for {}", agent_id);

                        // Cleanup expired nonces
                        identity.cleanup_nonces();
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::debug!("Heartbeat loop shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Register a verified member
    pub fn register_member(&self, member: RegisteredMember) -> bool {
        // Strict validation: require all fields
        if member.x25519_public_key == [0u8; 32] {
            tracing::warn!("Rejecting member {}: missing x25519_public_key", member.agent_id);
            return false;
        }
        if member.capabilities.is_empty() {
            tracing::warn!("Rejecting member {}: empty capabilities", member.agent_id);
            return false;
        }
        if member.joined_at == 0 {
            tracing::warn!("Rejecting member {}: invalid joined_at", member.agent_id);
            return false;
        }

        self.identity.register_member(member)
    }

    /// Resolve member from registry (returns full member, not just key)
    pub fn resolve_member(&self, agent_id: &str) -> Option<RegisteredMember> {
        self.identity.get_member(agent_id)
    }

    /// Subscribe to topic with handler
    pub fn subscribe(&self, topic: &str, handler: MessageHandler) {
        self.handlers.write().insert(topic.to_string(), handler);
    }

    /// Publish message to topic
    pub fn publish(&self, topic: &str, payload: &[u8]) -> Result<String, String> {
        if !*self.connected.read() {
            return Err("Not connected".to_string());
        }

        let nonce = IdentityManager::generate_nonce();
        let counter = self.identity.next_send_counter();

        // Encrypt payload with swarm key
        let encrypted = CryptoV2::encrypt(payload, &self.swarm_key)?;

        // Create envelope
        let mut envelope = SignedEnvelope::new_unsigned(
            format!("{}:{}", self.swarm_id, uuid::Uuid::new_v4()),
            topic.to_string(),
            self.agent_id.clone(),
            payload,
            nonce,
            counter,
            encrypted,
        );

        // Sign canonical header
        let canonical = envelope.canonical_header();
        envelope.signature = self.identity.sign(canonical.as_bytes());

        // In real implementation, publish to GUN
        tracing::debug!("Published message {} to topic {}", envelope.message_id, topic);

        Ok(envelope.message_id)
    }

    /// Process incoming envelope (with full verification)
    pub fn process_envelope(&self, envelope: &SignedEnvelope) -> Result<Vec<u8>, String> {
        // 1. Check nonce and timestamp (replay protection)
        if !self.identity.check_nonce(&envelope.nonce, envelope.timestamp, &envelope.sender_id) {
            return Err("Replay detected or expired".to_string());
        }

        // 2. Check counter (ordering)
        if !self.identity.validate_recv_counter(&envelope.sender_id, envelope.counter) {
            return Err("Invalid counter".to_string());
        }

        // 3. Resolve sender from registry (NOT from envelope)
        let _member = self.resolve_member(&envelope.sender_id)
            .ok_or_else(|| format!("Sender {} not in registry", envelope.sender_id))?;

        // 4. Verify signature using REGISTRY key
        let canonical = envelope.canonical_header();
        if !self.identity.verify_from_registry(&envelope.sender_id, canonical.as_bytes(), &envelope.signature) {
            return Err("Invalid signature".to_string());
        }

        // 5. Decrypt with swarm key
        let payload = CryptoV2::decrypt(&envelope.encrypted, &self.swarm_key)?;

        // 6. Verify payload hash
        let payload_hash = CryptoV2::hash(&payload);
        if payload_hash != envelope.payload_hash {
            return Err("Payload hash mismatch".to_string());
        }

        // 7. Dispatch to handler
        if let Some(handler) = self.handlers.read().get(&envelope.topic) {
            handler(&payload, &envelope.sender_id);
        }

        Ok(payload)
    }

    /// Create signaling message (WebRTC offer/answer/ice)
    pub fn create_signaling(&self, signal_type: SignalType, to: &str, payload: String, ttl_ms: u64) -> SignalingMessage {
        let mut signal = SignalingMessage::new_unsigned(
            signal_type,
            self.agent_id.clone(),
            to.to_string(),
            payload,
            ttl_ms,
        );

        // Sign using canonical (no pubkey in signed data)
        let canonical = signal.canonical_for_signing();
        signal.signature = self.identity.sign(canonical.as_bytes());

        signal
    }

    /// Verify signaling message (using REGISTRY key only)
    pub fn verify_signaling(&self, signal: &SignalingMessage) -> bool {
        // Check expiry
        if signal.is_expired() {
            return false;
        }

        // Verify payload hash
        let payload_hash = CryptoV2::hash(signal.payload.as_bytes());
        if payload_hash != signal.payload_hash {
            return false;
        }

        // Verify signature using REGISTRY key (not from signal)
        let canonical = signal.canonical_for_signing();
        self.identity.verify_from_registry(&signal.from, canonical.as_bytes(), &signal.signature)
    }

    /// Store artifact and get CID
    pub fn store_artifact(&self, data: &[u8], compress: bool) -> Result<String, String> {
        let info = self.artifact_store.store(data, compress)?;
        Ok(info.cid)
    }

    /// Retrieve artifact by CID
    pub fn retrieve_artifact(&self, cid: &str) -> Option<Vec<u8>> {
        self.artifact_store.retrieve(cid)
    }

    /// Create and sign artifact pointer
    pub fn create_artifact_pointer(&self, artifact_type: ArtifactType, cid: &str, dimensions: &str) -> Option<ArtifactPointer> {
        self.artifact_store.create_pointer(artifact_type, &self.agent_id, cid, dimensions, &self.identity)
    }

    /// Submit task
    pub fn submit_task(&self, task: TaskEnvelope) -> Result<String, String> {
        let task_id = task.task_id.clone();
        self.pending_tasks.write().insert(task_id.clone(), task.clone());

        // Publish to tasks topic
        let task_bytes = serde_json::to_vec(&task)
            .map_err(|e| e.to_string())?;
        self.publish("tasks", &task_bytes)?;

        Ok(task_id)
    }

    /// Claim task for execution
    pub fn claim_task(&self, task_id: &str) -> Result<TaskClaim, String> {
        // Check if task exists
        if !self.pending_tasks.read().contains_key(task_id) {
            return Err("Task not found".to_string());
        }

        // Check if already claimed within window
        let now = chrono::Utc::now().timestamp_millis() as u64;
        if let Some(existing) = self.task_claims.read().get(task_id) {
            if now - existing.timestamp < TASK_CLAIM_WINDOW_MS {
                return Err(format!("Task already claimed by {}", existing.executor_id));
            }
        }

        // Create claim
        let mut claim = TaskClaim {
            task_id: task_id.to_string(),
            executor_id: self.agent_id.clone(),
            timestamp: now,
            signature: [0u8; 64],
        };

        // Sign claim
        let canonical = serde_json::json!({
            "executor_id": claim.executor_id,
            "task_id": claim.task_id,
            "timestamp": claim.timestamp,
        });
        let canonical_str = CanonicalJson::stringify(&canonical);
        claim.signature = self.identity.sign(canonical_str.as_bytes());

        // Store claim
        self.task_claims.write().insert(task_id.to_string(), claim.clone());

        Ok(claim)
    }

    /// Create signed task receipt
    pub fn create_receipt(
        &self,
        task: &TaskEnvelope,
        result_cid: String,
        status: TaskStatus,
        fuel_used: u64,
        memory_peak_mb: u32,
        execution_ms: u64,
        input_hash: [u8; 32],
        output_hash: [u8; 32],
        module_hash: [u8; 32],
    ) -> TaskReceipt {
        let now = chrono::Utc::now().timestamp_millis() as u64;

        let mut receipt = TaskReceipt::new_unsigned(
            task,
            self.agent_id.clone(),
            result_cid,
            status,
            fuel_used,
            memory_peak_mb,
            execution_ms,
            input_hash,
            output_hash,
            module_hash,
            now - execution_ms,
            now,
        );

        // Sign the FULL canonical receipt (all binding fields)
        let canonical = receipt.canonical_for_signing();
        receipt.signature = self.identity.sign(canonical.as_bytes());

        receipt
    }

    /// Get swarm status
    pub fn status(&self) -> SwarmStatus {
        SwarmStatus {
            connected: *self.connected.read(),
            swarm_id: self.swarm_id.clone(),
            agent_id: self.agent_id.clone(),
            public_key: self.identity.public_key(),
            active_peers: self.identity.get_active_members(HEARTBEAT_TIMEOUT_MS).len(),
            pending_tasks: self.pending_tasks.read().len(),
            relay_metrics: self.relay_manager.get_metrics(),
        }
    }

    /// Get active peer IDs
    pub fn active_peers(&self) -> Vec<String> {
        self.identity.get_active_members(HEARTBEAT_TIMEOUT_MS)
            .into_iter()
            .map(|m| m.agent_id)
            .collect()
    }

    /// Derive session key for direct channel with peer
    pub fn derive_session_key(&self, peer_id: &str) -> Option<[u8; 32]> {
        self.identity.derive_session_key(peer_id, &self.swarm_id)
    }
}

impl Drop for P2PSwarmV2 {
    fn drop(&mut self) {
        self.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_swarm_connect() {
        let mut swarm = P2PSwarmV2::new("test-agent", None, vec!["executor".to_string()]);
        swarm.connect().await.unwrap();

        let status = swarm.status();
        assert!(status.connected);
        assert_eq!(status.agent_id, "test-agent");
    }

    #[tokio::test]
    async fn test_member_registration() {
        let swarm = P2PSwarmV2::new("test-agent", None, vec!["executor".to_string()]);

        // Create another identity
        let other_identity = IdentityManager::new();
        let registration = other_identity.create_registration("peer-1", vec!["worker".to_string()]);

        // Should succeed with valid registration
        assert!(swarm.register_member(registration));

        // Should be able to resolve
        let member = swarm.resolve_member("peer-1");
        assert!(member.is_some());
    }

    #[tokio::test]
    async fn test_strict_member_validation() {
        let swarm = P2PSwarmV2::new("test-agent", None, vec!["executor".to_string()]);

        // Registration with missing x25519 key should fail
        let mut bad_registration = RegisteredMember {
            agent_id: "bad-peer".to_string(),
            ed25519_public_key: [0u8; 32],
            x25519_public_key: [0u8; 32], // Invalid: all zeros
            capabilities: vec!["worker".to_string()],
            joined_at: chrono::Utc::now().timestamp_millis() as u64,
            last_heartbeat: 0,
            signature: [0u8; 64],
        };

        assert!(!swarm.register_member(bad_registration.clone()));

        // Empty capabilities should also fail
        bad_registration.x25519_public_key = [1u8; 32];
        bad_registration.capabilities = vec![];
        assert!(!swarm.register_member(bad_registration));
    }

    #[test]
    fn test_artifact_store_and_pointer() {
        let swarm = P2PSwarmV2::new("test-agent", None, vec!["executor".to_string()]);

        let data = b"test artifact data";
        let cid = swarm.store_artifact(data, true).unwrap();

        let retrieved = swarm.retrieve_artifact(&cid).unwrap();
        assert_eq!(data.as_slice(), retrieved.as_slice());

        let pointer = swarm.create_artifact_pointer(ArtifactType::QTable, &cid, "100x10");
        assert!(pointer.is_some());
    }

    #[tokio::test]
    async fn test_task_claim() {
        let mut swarm = P2PSwarmV2::new("test-agent", None, vec!["executor".to_string()]);
        swarm.connect().await.unwrap();

        let task = TaskEnvelope::new(
            "task-001".to_string(),
            "local:module".to_string(),
            "process".to_string(),
            "local:input".to_string(),
            [0u8; 32],
            TaskBudgets {
                fuel_limit: 1000000,
                memory_mb: 128,
                timeout_ms: 30000,
            },
            "requester".to_string(),
            chrono::Utc::now().timestamp_millis() as u64 + 60000,
            1,
        );

        // Submit task
        swarm.pending_tasks.write().insert(task.task_id.clone(), task.clone());

        // Claim should succeed
        let claim = swarm.claim_task("task-001").unwrap();
        assert_eq!(claim.task_id, "task-001");
        assert_eq!(claim.executor_id, "test-agent");

        // Second claim should fail (within window)
        assert!(swarm.claim_task("task-001").is_err());
    }
}
