//! Phase 4 – Transfer CRDT
//!
//! Distributed transfer-prior propagation using LWW-Map and G-Set CRDTs.
//!
//! * `publish_prior` – writes a local prior (cycle = LWW timestamp).
//! * `merge_peer`    – merges a peer node's state (last-writer-wins).
//! * `promote_via_consensus` – runs Byzantine commit before accepting a prior.

use ruvector_domain_expansion::DomainId;
use serde::{Deserialize, Serialize};

use crate::consensus::{byzantine_commit, CommitProof};
use crate::crdt::{GSet, LWWMap};
use crate::{FederationError, Result, StateUpdate};

// ─── types ────────────────────────────────────────────────────────────────────

/// Compact summary of a transfer prior for LWW replication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferPriorSummary {
    pub src_domain: String,
    pub dst_domain: String,
    /// Mean reward improvement from the transfer (positive = helpful).
    pub improvement: f32,
    /// Confidence in the estimate (higher = more observations).
    pub confidence: f32,
    /// Training cycle at which this summary was captured.
    pub cycle: u64,
}

// ─── TransferCrdt ─────────────────────────────────────────────────────────────

/// Distributed transfer-prior store using LWW-Map + G-Set CRDTs.
///
/// Multiple federation nodes each maintain their own `TransferCrdt`; calling
/// `merge_peer` synchronises state using last-writer-wins semantics keyed by
/// cycle count, guaranteeing eventual consistency without coordination.
pub struct TransferCrdt {
    /// LWW-Map: key = `"src:dst"`, value = best known prior summary.
    priors: LWWMap<String, TransferPriorSummary>,
    /// G-Set: all domain IDs ever observed by this node.
    domains: GSet<String>,
}

impl TransferCrdt {
    pub fn new() -> Self {
        Self {
            priors: LWWMap::new(),
            domains: GSet::new(),
        }
    }

    /// Publish a local transfer prior.
    ///
    /// `cycle` acts as the LWW timestamp so newer cycles always win
    /// without requiring wall-clock synchronisation.
    pub fn publish_prior(
        &mut self,
        src: &DomainId,
        dst: &DomainId,
        improvement: f32,
        confidence: f32,
        cycle: u64,
    ) {
        let key = format!("{}:{}", src.0, dst.0);
        let summary = TransferPriorSummary {
            src_domain: src.0.clone(),
            dst_domain: dst.0.clone(),
            improvement,
            confidence,
            cycle,
        };
        self.priors.set(key, summary, cycle);
        self.domains.add(src.0.clone());
        self.domains.add(dst.0.clone());
    }

    /// Merge a peer's CRDT state into this node (idempotent, commutative).
    pub fn merge_peer(&mut self, other: &TransferCrdt) {
        self.priors.merge(&other.priors);
        self.domains.merge(&other.domains);
    }

    /// Retrieve the best known prior for a domain pair (if any).
    pub fn best_prior_for(&self, src: &DomainId, dst: &DomainId) -> Option<&TransferPriorSummary> {
        let key = format!("{}:{}", src.0, dst.0);
        self.priors.get(&key)
    }

    /// All domain IDs known to this node.
    pub fn known_domains(&self) -> Vec<String> {
        self.domains.elements().cloned().collect()
    }

    /// Run Byzantine consensus before promoting a prior across the federation.
    ///
    /// Serialises the prior summary as the `StateUpdate` payload and calls the
    /// PBFT-style commit protocol. Requires `peer_count + 1 >= 4` total nodes.
    pub async fn promote_via_consensus(
        &self,
        src: &DomainId,
        dst: &DomainId,
        peer_count: usize,
    ) -> Result<CommitProof> {
        let key = format!("{}:{}", src.0, dst.0);
        let summary = self
            .priors
            .get(&key)
            .ok_or_else(|| FederationError::PeerNotFound(format!("no prior for {key}")))?;

        let data = serde_json::to_vec(summary)
            .map_err(|e| FederationError::ReconciliationError(e.to_string()))?;

        let update = StateUpdate {
            update_id: key,
            data,
            timestamp: current_millis(),
        };

        byzantine_commit(update, peer_count + 1).await
    }
}

impl Default for TransferCrdt {
    fn default() -> Self {
        Self::new()
    }
}

fn current_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_and_retrieve() {
        let mut crdt = TransferCrdt::new();
        let src = DomainId("retrieval".to_string());
        let dst = DomainId("graph".to_string());

        crdt.publish_prior(&src, &dst, 0.15, 0.8, 10);
        let p = crdt.best_prior_for(&src, &dst).unwrap();
        assert_eq!(p.cycle, 10);
        assert!((p.improvement - 0.15).abs() < 1e-5);
    }

    #[test]
    fn test_lww_newer_wins() {
        let mut node_a = TransferCrdt::new();
        let mut node_b = TransferCrdt::new();
        let src = DomainId("x".to_string());
        let dst = DomainId("y".to_string());

        node_a.publish_prior(&src, &dst, 0.1, 0.5, 5); // older cycle
        node_b.publish_prior(&src, &dst, 0.2, 0.9, 10); // newer wins

        node_a.merge_peer(&node_b);
        let p = node_a.best_prior_for(&src, &dst).unwrap();
        assert_eq!(p.cycle, 10);
        assert!((p.improvement - 0.2).abs() < 1e-5);
    }

    #[test]
    fn test_merge_idempotent() {
        let mut crdt = TransferCrdt::new();
        let src = DomainId("a".to_string());
        let dst = DomainId("b".to_string());
        crdt.publish_prior(&src, &dst, 0.3, 0.7, 5);

        let snapshot = TransferCrdt::new(); // empty peer
        crdt.merge_peer(&snapshot);

        // Still has original data
        assert!(crdt.best_prior_for(&src, &dst).is_some());
    }

    #[test]
    fn test_gset_domain_discovery() {
        let mut crdt = TransferCrdt::new();
        crdt.publish_prior(
            &DomainId("a".to_string()),
            &DomainId("b".to_string()),
            0.1,
            0.5,
            1,
        );
        crdt.publish_prior(
            &DomainId("b".to_string()),
            &DomainId("c".to_string()),
            0.2,
            0.6,
            2,
        );
        let domains = crdt.known_domains();
        assert!(domains.contains(&"a".to_string()));
        assert!(domains.contains(&"b".to_string()));
        assert!(domains.contains(&"c".to_string()));
    }

    #[tokio::test]
    async fn test_promote_via_consensus() {
        let mut crdt = TransferCrdt::new();
        let src = DomainId("retrieval".to_string());
        let dst = DomainId("graph".to_string());
        crdt.publish_prior(&src, &dst, 0.3, 0.9, 20);

        // 6 peers + 1 local = 7 total nodes; for n=7: f=2, threshold=5, verify=(16/3)=5 ✓
        let proof = crdt.promote_via_consensus(&src, &dst, 6).await.unwrap();
        assert!(proof.verify(7));
    }
}
