//! Byzantine fault-tolerant consensus
//!
//! Implements PBFT-style consensus for state updates across federation:
//! - Pre-prepare phase
//! - Prepare phase
//! - Commit phase
//! - Proof generation

use crate::{FederationError, PeerId, Result, StateUpdate};
use serde::{Deserialize, Serialize};

/// Consensus message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsensusMessage {
    PrePrepare { proposal: SignedProposal },
    Prepare { digest: Vec<u8>, sender: PeerId },
    Commit { digest: Vec<u8>, sender: PeerId },
}

/// Signed proposal for a state update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedProposal {
    pub update: StateUpdate,
    pub sequence_number: u64,
    pub signature: Vec<u8>,
}

/// Proof that consensus was reached
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitProof {
    pub update_id: String,
    pub commit_messages: Vec<CommitMessage>,
    pub timestamp: u64,
}

impl CommitProof {
    /// Verify that proof contains sufficient commits
    pub fn verify(&self, total_nodes: usize) -> bool {
        let threshold = byzantine_threshold(total_nodes);
        self.commit_messages.len() >= threshold
    }
}

/// A commit message from a peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitMessage {
    pub peer_id: PeerId,
    pub digest: Vec<u8>,
    pub signature: Vec<u8>,
}

/// Result of a consensus attempt
#[derive(Debug)]
pub enum CommitResult {
    Success(CommitProof),
    InsufficientPrepares,
    InsufficientCommits,
}

/// Calculate Byzantine fault threshold
///
/// For n = 3f + 1 nodes, we can tolerate f Byzantine faults.
/// Consensus requires 2f + 1 = (2n + 2) / 3 agreements.
fn byzantine_threshold(n: usize) -> usize {
    (2 * n + 2) / 3
}

/// Execute Byzantine fault-tolerant consensus on a state update
///
/// # PBFT Protocol
///
/// 1. **Pre-prepare**: Leader proposes update
/// 2. **Prepare**: Nodes acknowledge receipt (2f+1 required)
/// 3. **Commit**: Nodes commit to proposal (2f+1 required)
/// 4. **Execute**: Update is applied with proof
///
/// # Implementation from PSEUDOCODE.md
///
/// ```pseudocode
/// FUNCTION ByzantineCommit(update, federation):
///     n = federation.node_count()
///     f = (n - 1) / 3
///     threshold = 2*f + 1
///
///     // Phase 1: Pre-prepare
///     IF federation.is_leader():
///         proposal = SignedProposal(update, sequence_number=NEXT_SEQ)
///         Broadcast(federation.nodes, PrePrepare(proposal))
///
///     // Phase 2: Prepare
///     pre_prepare = ReceivePrePrepare()
///     IF ValidateProposal(pre_prepare):
///         prepare_msg = Prepare(pre_prepare.digest, local_id)
///         Broadcast(federation.nodes, prepare_msg)
///
///     prepares = CollectMessages(type=Prepare, count=threshold)
///     IF len(prepares) < threshold:
///         RETURN InsufficientPrepares
///
///     // Phase 3: Commit
///     commit_msg = Commit(pre_prepare.digest, local_id)
///     Broadcast(federation.nodes, commit_msg)
///
///     commits = CollectMessages(type=Commit, count=threshold)
///     IF len(commits) >= threshold:
///         federation.apply_update(update)
///         proof = CommitProof(commits)
///         RETURN Success(proof)
///     ELSE:
///         RETURN InsufficientCommits
/// ```
pub async fn byzantine_commit(update: StateUpdate, peer_count: usize) -> Result<CommitProof> {
    let n = peer_count;
    let f = if n > 0 { (n - 1) / 3 } else { 0 };
    let threshold = 2 * f + 1;

    if n < 4 {
        return Err(FederationError::InsufficientPeers {
            needed: 4,
            actual: n,
        });
    }

    // Phase 1: Pre-prepare (leader proposes)
    let sequence_number = get_next_sequence_number();
    let proposal = SignedProposal {
        update: update.clone(),
        sequence_number,
        signature: sign_proposal(&update),
    };

    // Broadcast pre-prepare (simulated)
    let _pre_prepare = ConsensusMessage::PrePrepare {
        proposal: proposal.clone(),
    };

    // Phase 2: Prepare (nodes acknowledge)
    let digest = compute_digest(&update);

    // Simulate collecting prepare messages from peers
    let prepares = simulate_prepare_phase(&digest, threshold)?;

    if prepares.len() < threshold {
        return Err(FederationError::ConsensusError(format!(
            "Insufficient prepares: got {}, needed {}",
            prepares.len(),
            threshold
        )));
    }

    // Phase 3: Commit (nodes commit)
    let commit_messages = simulate_commit_phase(&digest, threshold)?;

    if commit_messages.len() < threshold {
        return Err(FederationError::ConsensusError(format!(
            "Insufficient commits: got {}, needed {}",
            commit_messages.len(),
            threshold
        )));
    }

    // Create proof
    let proof = CommitProof {
        update_id: update.update_id.clone(),
        commit_messages,
        timestamp: current_timestamp(),
    };

    // Verify proof
    if !proof.verify(n) {
        return Err(FederationError::ConsensusError(
            "Proof verification failed".to_string(),
        ));
    }

    Ok(proof)
}

/// Compute digest of a state update
fn compute_digest(update: &StateUpdate) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&update.update_id);
    hasher.update(&update.data);
    hasher.update(&update.timestamp.to_le_bytes());
    hasher.finalize().to_vec()
}

/// Sign a proposal (placeholder)
fn sign_proposal(update: &StateUpdate) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"signature:");
    hasher.update(&update.update_id);
    hasher.finalize().to_vec()
}

/// Get next sequence number (placeholder)
fn get_next_sequence_number() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::SeqCst)
}

/// Simulate prepare phase (placeholder for network communication)
fn simulate_prepare_phase(digest: &[u8], threshold: usize) -> Result<Vec<(PeerId, Vec<u8>)>> {
    let mut prepares = Vec::new();

    // Simulate receiving prepare messages from peers
    for i in 0..threshold {
        let peer_id = PeerId::new(format!("peer_{}", i));
        prepares.push((peer_id, digest.to_vec()));
    }

    Ok(prepares)
}

/// Simulate commit phase (placeholder for network communication)
fn simulate_commit_phase(digest: &[u8], threshold: usize) -> Result<Vec<CommitMessage>> {
    let mut commits = Vec::new();

    // Simulate receiving commit messages from peers
    for i in 0..threshold {
        let peer_id = PeerId::new(format!("peer_{}", i));
        let signature = sign_commit(digest, &peer_id);

        commits.push(CommitMessage {
            peer_id,
            digest: digest.to_vec(),
            signature,
        });
    }

    Ok(commits)
}

/// Sign a commit message (placeholder)
fn sign_commit(digest: &[u8], peer_id: &PeerId) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"commit:");
    hasher.update(digest);
    hasher.update(peer_id.0.as_bytes());
    hasher.finalize().to_vec()
}

/// Get current timestamp
fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_byzantine_commit_success() {
        let update = StateUpdate {
            update_id: "test_update_1".to_string(),
            data: vec![1, 2, 3, 4],
            timestamp: current_timestamp(),
        };

        // Need at least 4 nodes for BFT (n = 3f + 1, f = 1)
        let proof = byzantine_commit(update, 4).await.unwrap();

        assert!(proof.verify(4));
        assert_eq!(proof.update_id, "test_update_1");
    }

    #[tokio::test]
    async fn test_byzantine_commit_insufficient_peers() {
        let update = StateUpdate {
            update_id: "test_update_2".to_string(),
            data: vec![1, 2, 3],
            timestamp: current_timestamp(),
        };

        // Only 3 nodes - not enough for BFT
        let result = byzantine_commit(update, 3).await;

        assert!(result.is_err());
        match result {
            Err(FederationError::InsufficientPeers { needed, actual }) => {
                assert_eq!(needed, 4);
                assert_eq!(actual, 3);
            }
            _ => panic!("Expected InsufficientPeers error"),
        }
    }

    #[test]
    fn test_byzantine_threshold() {
        // n = 3f + 1, threshold = 2f + 1
        assert_eq!(byzantine_threshold(4), 3); // f=1, 2f+1=3
        assert_eq!(byzantine_threshold(7), 5); // f=2, 2f+1=5
        assert_eq!(byzantine_threshold(10), 7); // f=3, 2f+1=7
    }

    #[test]
    fn test_commit_proof_verification() {
        let proof = CommitProof {
            update_id: "test".to_string(),
            commit_messages: vec![
                CommitMessage {
                    peer_id: PeerId::new("peer1".to_string()),
                    digest: vec![1, 2, 3],
                    signature: vec![4, 5, 6],
                },
                CommitMessage {
                    peer_id: PeerId::new("peer2".to_string()),
                    digest: vec![1, 2, 3],
                    signature: vec![7, 8, 9],
                },
                CommitMessage {
                    peer_id: PeerId::new("peer3".to_string()),
                    digest: vec![1, 2, 3],
                    signature: vec![10, 11, 12],
                },
            ],
            timestamp: current_timestamp(),
        };

        // For 4 nodes, need 3 commits
        assert!(proof.verify(4));

        // For 7 nodes, would need 5 commits
        assert!(!proof.verify(7));
    }
}
