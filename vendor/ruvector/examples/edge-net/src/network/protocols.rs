//! Custom libp2p protocols for EdgeNet task negotiation
//!
//! Implements the request-response protocol for direct peer-to-peer
//! task negotiation, including:
//! - Task details request
//! - Work claims with stake
//! - Result submission with proofs
//! - Payment verification and release
//!
//! ## Protocol Flow
//!
//! ```text
//! Requester                           Worker
//!     |                                  |
//!     |--- TaskRequest::GetDetails ---->|
//!     |<-- TaskResponse::Accepted ------|
//!     |                                  |
//!     |--- TaskRequest::SubmitClaim --->|
//!     |<-- TaskResponse::Accepted ------|
//!     |                                  |
//!     |    [Worker executes task]        |
//!     |                                  |
//!     |<-- TaskRequest::SubmitResult ---|
//!     |--- TaskResponse::Verified ----->|
//!     |                                  |
//!     |<-- TaskRequest::ReleasePayment -|
//!     |--- PaymentReleased ------------>|
//! ```

#[cfg(feature = "p2p")]
use libp2p::request_response::{self, Codec};

use async_trait::async_trait;
use futures::prelude::*;
use serde::{Serialize, Deserialize};
use std::io;

use super::p2p::{TaskRequest, TaskResponse};

// ============================================================================
// Protocol Definition
// ============================================================================

/// The task negotiation protocol identifier
#[derive(Debug, Clone)]
pub struct TaskProtocol;

#[cfg(feature = "p2p")]
impl AsRef<str> for TaskProtocol {
    fn as_ref(&self) -> &str {
        "/edge-net/task-negotiate/1.0.0"
    }
}

// ============================================================================
// Codec Implementation
// ============================================================================

/// Codec for serializing/deserializing task requests and responses
///
/// Uses bincode for efficient binary serialization with the following format:
/// - 4 bytes: message length (big-endian u32)
/// - N bytes: bincode-serialized message
#[derive(Debug, Clone, Default)]
pub struct TaskCodec {
    /// Maximum message size in bytes (default: 16MB)
    max_message_size: usize,
}

impl TaskCodec {
    /// Create a new codec with default settings
    pub fn new() -> Self {
        Self {
            max_message_size: 16 * 1024 * 1024, // 16MB
        }
    }

    /// Create a new codec with custom max message size
    pub fn with_max_size(max_message_size: usize) -> Self {
        Self { max_message_size }
    }
}

#[cfg(feature = "p2p")]
#[async_trait]
impl Codec for TaskCodec {
    type Protocol = TaskProtocol;
    type Request = TaskRequest;
    type Response = TaskResponse;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_length_prefixed(io, self.max_message_size).await
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_length_prefixed(io, self.max_message_size).await
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, &req).await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, &res).await
    }
}

// ============================================================================
// Length-Prefixed I/O Helpers
// ============================================================================

/// Read a length-prefixed message from the stream
async fn read_length_prefixed<T, M>(io: &mut T, max_size: usize) -> io::Result<M>
where
    T: AsyncRead + Unpin + Send,
    M: for<'de> Deserialize<'de>,
{
    // Read the 4-byte length prefix
    let mut len_bytes = [0u8; 4];
    io.read_exact(&mut len_bytes).await?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    // Validate length
    if len > max_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Message too large: {} bytes (max: {})", len, max_size),
        ));
    }

    if len == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Empty message",
        ));
    }

    // Read the message body
    let mut buffer = vec![0u8; len];
    io.read_exact(&mut buffer).await?;

    // Deserialize
    bincode::deserialize(&buffer).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Deserialization error: {}", e))
    })
}

/// Write a length-prefixed message to the stream
async fn write_length_prefixed<T, M>(io: &mut T, msg: &M) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
    M: Serialize,
{
    // Serialize the message
    let data = bincode::serialize(msg).map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Serialization error: {}", e))
    })?;

    // Write length prefix
    let len = data.len() as u32;
    io.write_all(&len.to_be_bytes()).await?;

    // Write message body
    io.write_all(&data).await?;
    io.flush().await?;

    Ok(())
}

// ============================================================================
// Additional Protocol Messages
// ============================================================================

/// Extended task information for detailed negotiation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskDetails {
    /// Task identifier
    pub task_id: String,
    /// Task type (e.g., "vectors", "embeddings", "inference")
    pub task_type: String,
    /// Human-readable description
    pub description: String,
    /// Input data hash (for verification)
    pub input_hash: [u8; 32],
    /// Expected output size in bytes
    pub expected_output_size: usize,
    /// Base reward in credits
    pub base_reward: u64,
    /// Bonus multiplier for early completion
    pub early_bonus: f32,
    /// Deadline timestamp (ms since epoch)
    pub deadline_ms: u64,
    /// Number of required confirmations
    pub required_confirmations: u32,
    /// Submitter's stake (for dispute resolution)
    pub submitter_stake: u64,
}

/// Work claim with proof of stake
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkClaim {
    /// Task being claimed
    pub task_id: String,
    /// Worker's node ID
    pub worker_id: String,
    /// Staked amount
    pub stake: u64,
    /// Estimated completion time in ms
    pub estimated_time_ms: u64,
    /// Worker's capability proof
    pub capability_proof: Vec<u8>,
    /// Signature over claim data
    pub signature: Vec<u8>,
}

/// Task result with cryptographic proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task identifier
    pub task_id: String,
    /// Worker's node ID
    pub worker_id: String,
    /// Result data (encrypted with submitter's key)
    pub encrypted_result: Vec<u8>,
    /// Hash of unencrypted result (for verification)
    pub result_hash: [u8; 32],
    /// Proof of work/computation
    pub proof: ComputationProof,
    /// Execution statistics
    pub stats: ExecutionStats,
    /// Signature over result
    pub signature: Vec<u8>,
}

/// Proof of computation for verification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ComputationProof {
    /// Simple hash chain proof
    HashChain {
        /// Intermediate hashes from computation
        intermediate_hashes: Vec<[u8; 32]>,
        /// Final hash
        final_hash: [u8; 32],
    },
    /// Merkle proof of computation steps
    MerkleProof {
        /// Merkle root of computation trace
        root: [u8; 32],
        /// Proof path for sampled steps
        proof_path: Vec<([u8; 32], bool)>,
    },
    /// Zero-knowledge proof (future)
    ZkProof {
        /// Proof bytes (implementation-specific)
        proof_bytes: Vec<u8>,
        /// Verification key
        verification_key: Vec<u8>,
    },
    /// Attestation from trusted execution environment
    TeeAttestation {
        /// Quote from TEE
        quote: Vec<u8>,
        /// Enclave measurement
        measurement: [u8; 32],
    },
}

/// Execution statistics for task completion
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// CPU time in milliseconds
    pub cpu_time_ms: u64,
    /// Wall clock time in milliseconds
    pub wall_time_ms: u64,
    /// Peak memory usage in bytes
    pub peak_memory_bytes: usize,
    /// Number of operations performed
    pub operations: u64,
    /// Input size processed
    pub input_bytes: usize,
    /// Output size generated
    pub output_bytes: usize,
}

/// Payment release request with verification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentRelease {
    /// Task identifier
    pub task_id: String,
    /// Worker to be paid
    pub worker_id: String,
    /// Amount to release
    pub amount: u64,
    /// Verification signatures from validators
    pub validator_signatures: Vec<(String, Vec<u8>)>,
    /// Timestamp of release request
    pub timestamp_ms: u64,
}

/// Dispute filing for contested results
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskDispute {
    /// Task being disputed
    pub task_id: String,
    /// Disputer's node ID
    pub disputer_id: String,
    /// Type of dispute
    pub dispute_type: DisputeType,
    /// Evidence supporting dispute
    pub evidence: Vec<DisputeEvidence>,
    /// Stake for dispute
    pub dispute_stake: u64,
    /// Signature
    pub signature: Vec<u8>,
}

/// Types of task disputes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DisputeType {
    /// Result is incorrect
    IncorrectResult,
    /// Worker didn't complete in time
    Timeout,
    /// Worker submitted invalid proof
    InvalidProof,
    /// Task was never assigned
    Unauthorized,
    /// Payment was not released
    PaymentWithheld,
}

/// Evidence for dispute resolution
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisputeEvidence {
    /// Type of evidence
    pub evidence_type: String,
    /// Evidence data
    pub data: Vec<u8>,
    /// Reference to on-chain/log proof
    pub reference: Option<String>,
}

// ============================================================================
// Protocol Versioning
// ============================================================================

/// Protocol version information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProtocolVersion {
    /// Major version (breaking changes)
    pub major: u32,
    /// Minor version (backward-compatible features)
    pub minor: u32,
    /// Patch version (bug fixes)
    pub patch: u32,
    /// Supported features
    pub features: Vec<String>,
}

impl ProtocolVersion {
    /// Current protocol version
    pub fn current() -> Self {
        Self {
            major: 1,
            minor: 0,
            patch: 0,
            features: vec![
                "gossipsub".to_string(),
                "kademlia".to_string(),
                "task-negotiate".to_string(),
                "noise-encryption".to_string(),
            ],
        }
    }

    /// Check if this version is compatible with another
    pub fn is_compatible(&self, other: &ProtocolVersion) -> bool {
        // Same major version = compatible
        self.major == other.major
    }
}

// ============================================================================
// Message Validation
// ============================================================================

/// Validator for protocol messages
pub struct MessageValidator {
    /// Maximum allowed message age in ms
    max_message_age_ms: u64,
    /// Minimum required stake for claims
    min_claim_stake: u64,
    /// Required proof types
    required_proofs: Vec<String>,
}

impl Default for MessageValidator {
    fn default() -> Self {
        Self {
            max_message_age_ms: 300_000, // 5 minutes
            min_claim_stake: 100,
            required_proofs: vec!["hash_chain".to_string()],
        }
    }
}

impl MessageValidator {
    /// Validate a task request
    pub fn validate_request(&self, request: &TaskRequest) -> Result<(), ValidationError> {
        // Basic validation
        if request.task_id.is_empty() {
            return Err(ValidationError::EmptyTaskId);
        }

        if request.encrypted_payload.len() > 16 * 1024 * 1024 {
            return Err(ValidationError::PayloadTooLarge);
        }

        Ok(())
    }

    /// Validate a work claim
    pub fn validate_claim(&self, claim: &WorkClaim) -> Result<(), ValidationError> {
        if claim.stake < self.min_claim_stake {
            return Err(ValidationError::InsufficientStake {
                required: self.min_claim_stake,
                provided: claim.stake,
            });
        }

        if claim.signature.len() != 64 {
            return Err(ValidationError::InvalidSignature);
        }

        Ok(())
    }

    /// Validate a task result
    pub fn validate_result(&self, result: &TaskResult) -> Result<(), ValidationError> {
        if result.encrypted_result.is_empty() {
            return Err(ValidationError::EmptyResult);
        }

        if result.signature.len() != 64 {
            return Err(ValidationError::InvalidSignature);
        }

        // Validate proof type
        match &result.proof {
            ComputationProof::HashChain { intermediate_hashes, .. } => {
                if intermediate_hashes.is_empty() {
                    return Err(ValidationError::InvalidProof("Empty hash chain".to_string()));
                }
            }
            ComputationProof::MerkleProof { proof_path, .. } => {
                if proof_path.is_empty() {
                    return Err(ValidationError::InvalidProof("Empty merkle proof".to_string()));
                }
            }
            _ => {}
        }

        Ok(())
    }
}

/// Validation errors
#[derive(Debug, Clone)]
pub enum ValidationError {
    EmptyTaskId,
    PayloadTooLarge,
    InsufficientStake { required: u64, provided: u64 },
    InvalidSignature,
    EmptyResult,
    InvalidProof(String),
    MessageTooOld,
    UnknownProofType,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::EmptyTaskId => write!(f, "Empty task ID"),
            ValidationError::PayloadTooLarge => write!(f, "Payload too large"),
            ValidationError::InsufficientStake { required, provided } => {
                write!(f, "Insufficient stake: {} required, {} provided", required, provided)
            }
            ValidationError::InvalidSignature => write!(f, "Invalid signature"),
            ValidationError::EmptyResult => write!(f, "Empty result"),
            ValidationError::InvalidProof(msg) => write!(f, "Invalid proof: {}", msg),
            ValidationError::MessageTooOld => write!(f, "Message too old"),
            ValidationError::UnknownProofType => write!(f, "Unknown proof type"),
        }
    }
}

impl std::error::Error for ValidationError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_codec_new() {
        let codec = TaskCodec::new();
        assert_eq!(codec.max_message_size, 16 * 1024 * 1024);
    }

    #[test]
    fn test_task_codec_with_max_size() {
        let codec = TaskCodec::with_max_size(1024);
        assert_eq!(codec.max_message_size, 1024);
    }

    #[test]
    fn test_task_details_serialization() {
        let details = TaskDetails {
            task_id: "task-123".to_string(),
            task_type: "vectors".to_string(),
            description: "Process vector batch".to_string(),
            input_hash: [0u8; 32],
            expected_output_size: 1024,
            base_reward: 100,
            early_bonus: 1.5,
            deadline_ms: 1000000,
            required_confirmations: 3,
            submitter_stake: 500,
        };

        let serialized = bincode::serialize(&details).unwrap();
        let deserialized: TaskDetails = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.task_id, "task-123");
        assert_eq!(deserialized.base_reward, 100);
    }

    #[test]
    fn test_work_claim_serialization() {
        let claim = WorkClaim {
            task_id: "task-123".to_string(),
            worker_id: "worker-456".to_string(),
            stake: 200,
            estimated_time_ms: 5000,
            capability_proof: vec![1, 2, 3],
            signature: vec![0u8; 64],
        };

        let serialized = bincode::serialize(&claim).unwrap();
        let deserialized: WorkClaim = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.worker_id, "worker-456");
        assert_eq!(deserialized.stake, 200);
    }

    #[test]
    fn test_computation_proof_variants() {
        let hash_proof = ComputationProof::HashChain {
            intermediate_hashes: vec![[1u8; 32], [2u8; 32]],
            final_hash: [3u8; 32],
        };

        let merkle_proof = ComputationProof::MerkleProof {
            root: [4u8; 32],
            proof_path: vec![([5u8; 32], true), ([6u8; 32], false)],
        };

        // Both should serialize/deserialize
        let serialized_hash = bincode::serialize(&hash_proof).unwrap();
        let serialized_merkle = bincode::serialize(&merkle_proof).unwrap();

        let _: ComputationProof = bincode::deserialize(&serialized_hash).unwrap();
        let _: ComputationProof = bincode::deserialize(&serialized_merkle).unwrap();
    }

    #[test]
    fn test_protocol_version() {
        let v = ProtocolVersion::current();
        assert_eq!(v.major, 1);
        assert!(v.features.contains(&"gossipsub".to_string()));
    }

    #[test]
    fn test_protocol_compatibility() {
        let v1 = ProtocolVersion { major: 1, minor: 0, patch: 0, features: vec![] };
        let v2 = ProtocolVersion { major: 1, minor: 1, patch: 0, features: vec![] };
        let v3 = ProtocolVersion { major: 2, minor: 0, patch: 0, features: vec![] };

        assert!(v1.is_compatible(&v2));
        assert!(!v1.is_compatible(&v3));
    }

    #[test]
    fn test_message_validator_default() {
        let validator = MessageValidator::default();
        assert_eq!(validator.max_message_age_ms, 300_000);
        assert_eq!(validator.min_claim_stake, 100);
    }

    #[test]
    fn test_validate_claim_insufficient_stake() {
        let validator = MessageValidator::default();
        let claim = WorkClaim {
            task_id: "task-123".to_string(),
            worker_id: "worker-456".to_string(),
            stake: 50, // Below minimum
            estimated_time_ms: 5000,
            capability_proof: vec![],
            signature: vec![0u8; 64],
        };

        let result = validator.validate_claim(&claim);
        assert!(matches!(result, Err(ValidationError::InsufficientStake { .. })));
    }

    #[test]
    fn test_validate_claim_success() {
        let validator = MessageValidator::default();
        let claim = WorkClaim {
            task_id: "task-123".to_string(),
            worker_id: "worker-456".to_string(),
            stake: 200,
            estimated_time_ms: 5000,
            capability_proof: vec![],
            signature: vec![0u8; 64],
        };

        assert!(validator.validate_claim(&claim).is_ok());
    }

    #[test]
    fn test_execution_stats() {
        let stats = ExecutionStats {
            cpu_time_ms: 1000,
            wall_time_ms: 1200,
            peak_memory_bytes: 64 * 1024 * 1024,
            operations: 1_000_000,
            input_bytes: 4096,
            output_bytes: 1024,
        };

        let serialized = bincode::serialize(&stats).unwrap();
        let deserialized: ExecutionStats = bincode::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.cpu_time_ms, 1000);
        assert_eq!(deserialized.operations, 1_000_000);
    }

    #[test]
    fn test_dispute_types() {
        let dispute = TaskDispute {
            task_id: "task-123".to_string(),
            disputer_id: "disputer-456".to_string(),
            dispute_type: DisputeType::IncorrectResult,
            evidence: vec![],
            dispute_stake: 1000,
            signature: vec![0u8; 64],
        };

        let serialized = bincode::serialize(&dispute).unwrap();
        let deserialized: TaskDispute = bincode::deserialize(&serialized).unwrap();

        assert!(matches!(deserialized.dispute_type, DisputeType::IncorrectResult));
    }

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::InsufficientStake { required: 100, provided: 50 };
        let msg = err.to_string();
        assert!(msg.contains("100"));
        assert!(msg.contains("50"));
    }
}
