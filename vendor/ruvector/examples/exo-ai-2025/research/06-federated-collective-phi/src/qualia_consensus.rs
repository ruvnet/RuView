// qualia_consensus.rs
// Byzantine Fault Tolerant Consensus Protocol for Qualia
// Based on PBFT (Practical Byzantine Fault Tolerance)

use super::consciousness_crdt::Quale;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Agent identifier
pub type AgentId = u64;

/// View number for PBFT protocol
pub type ViewNumber = u64;

/// Sequence number for ordering qualia proposals
pub type SequenceNumber = u64;

/// Message types in PBFT-Qualia protocol
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum QualiaMessage {
    /// Phase 1: Leader proposes qualia
    QualiaProposal {
        qualia: Quale,
        view: ViewNumber,
        sequence: SequenceNumber,
        leader_id: AgentId,
    },

    /// Phase 2: Agents prepare (validate and vote)
    QualiaPrepare {
        qualia: Quale,
        view: ViewNumber,
        sequence: SequenceNumber,
        agent_id: AgentId,
    },

    /// Phase 3: Agents commit
    QualiaCommit {
        qualia: Quale,
        view: ViewNumber,
        sequence: SequenceNumber,
        agent_id: AgentId,
    },

    /// View change request (if leader is faulty)
    ViewChange {
        new_view: ViewNumber,
        agent_id: AgentId,
    },
}

/// Vote for a quale
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualiaVote {
    pub agent_id: AgentId,
    pub qualia: Quale,
    pub agrees: bool,
}

/// Result of consensus protocol
#[derive(Clone, Debug, PartialEq)]
pub enum ConsensusResult {
    /// Consensus reached on this quale
    Agreed(Quale),
    /// No consensus yet
    Pending,
    /// Consensus failed (too many Byzantine agents)
    Failed,
}

/// PBFT-Qualia consensus node
pub struct QualiaConsensusNode {
    /// This node's agent ID
    agent_id: AgentId,

    /// Total number of agents in the system
    n_agents: usize,

    /// Maximum number of Byzantine agents (f < n/3)
    f_byzantine: usize,

    /// Current view number
    current_view: ViewNumber,

    /// Next sequence number
    next_sequence: SequenceNumber,

    /// Received prepare messages
    prepare_messages: HashMap<SequenceNumber, HashMap<AgentId, QualiaMessage>>,

    /// Received commit messages
    commit_messages: HashMap<SequenceNumber, HashMap<AgentId, QualiaMessage>>,

    /// Agreed qualia (finalized)
    agreed_qualia: HashMap<SequenceNumber, Quale>,

    /// Pending proposals
    pending_proposals: HashMap<SequenceNumber, Quale>,
}

impl QualiaConsensusNode {
    pub fn new(agent_id: AgentId, n_agents: usize) -> Self {
        // Byzantine tolerance: f < n/3
        let f_byzantine = (n_agents - 1) / 3;

        Self {
            agent_id,
            n_agents,
            f_byzantine,
            current_view: 0,
            next_sequence: 0,
            prepare_messages: HashMap::new(),
            commit_messages: HashMap::new(),
            agreed_qualia: HashMap::new(),
            pending_proposals: HashMap::new(),
        }
    }

    /// Propose qualia (as leader)
    pub fn propose_qualia(&mut self, qualia: Quale) -> QualiaMessage {
        let sequence = self.next_sequence;
        self.next_sequence += 1;

        self.pending_proposals.insert(sequence, qualia.clone());

        QualiaMessage::QualiaProposal {
            qualia,
            view: self.current_view,
            sequence,
            leader_id: self.agent_id,
        }
    }

    /// Process received message
    pub fn process_message(&mut self, msg: QualiaMessage) -> Option<QualiaMessage> {
        match msg {
            QualiaMessage::QualiaProposal {
                qualia,
                view,
                sequence,
                leader_id: _,
            } => self.handle_proposal(qualia, view, sequence),

            QualiaMessage::QualiaPrepare {
                qualia,
                view,
                sequence,
                agent_id,
            } => {
                self.handle_prepare(qualia, view, sequence, agent_id);
                None
            }

            QualiaMessage::QualiaCommit {
                qualia,
                view,
                sequence,
                agent_id,
            } => {
                self.handle_commit(qualia, view, sequence, agent_id);
                None
            }

            QualiaMessage::ViewChange {
                new_view,
                agent_id: _,
            } => {
                self.handle_view_change(new_view);
                None
            }
        }
    }

    /// Handle qualia proposal
    fn handle_proposal(
        &mut self,
        qualia: Quale,
        view: ViewNumber,
        sequence: SequenceNumber,
    ) -> Option<QualiaMessage> {
        // Validate proposal
        if view != self.current_view {
            return None; // Wrong view
        }

        // Store pending
        self.pending_proposals.insert(sequence, qualia.clone());

        // Send prepare message
        Some(QualiaMessage::QualiaPrepare {
            qualia,
            view,
            sequence,
            agent_id: self.agent_id,
        })
    }

    /// Handle prepare message
    fn handle_prepare(
        &mut self,
        qualia: Quale,
        view: ViewNumber,
        sequence: SequenceNumber,
        agent_id: AgentId,
    ) {
        if view != self.current_view {
            return;
        }

        let msg = QualiaMessage::QualiaPrepare {
            qualia,
            view,
            sequence,
            agent_id,
        };

        self.prepare_messages
            .entry(sequence)
            .or_insert_with(HashMap::new)
            .insert(agent_id, msg);
    }

    /// Handle commit message
    fn handle_commit(
        &mut self,
        qualia: Quale,
        view: ViewNumber,
        sequence: SequenceNumber,
        agent_id: AgentId,
    ) {
        if view != self.current_view {
            return;
        }

        let msg = QualiaMessage::QualiaCommit {
            qualia,
            view,
            sequence,
            agent_id,
        };

        self.commit_messages
            .entry(sequence)
            .or_insert_with(HashMap::new)
            .insert(agent_id, msg);
    }

    /// Handle view change
    fn handle_view_change(&mut self, new_view: ViewNumber) {
        if new_view > self.current_view {
            self.current_view = new_view;
            // Clear pending state
            self.prepare_messages.clear();
            self.commit_messages.clear();
        }
    }

    /// Check if ready to commit
    pub fn check_ready_to_commit(&mut self, sequence: SequenceNumber) -> Option<QualiaMessage> {
        let prepares = self.prepare_messages.get(&sequence)?;

        // Need at least 2f + 1 prepare messages (including self)
        // For n=4, f=1, we need 2*1 + 1 = 3 prepares
        let required = 2 * self.f_byzantine + 1;
        if prepares.len() >= required {
            // Extract the qualia from prepares
            let qualia = self.pending_proposals.get(&sequence)?.clone();

            // Send commit message
            return Some(QualiaMessage::QualiaCommit {
                qualia,
                view: self.current_view,
                sequence,
                agent_id: self.agent_id,
            });
        }

        None
    }

    /// Check consensus result
    pub fn check_consensus(&mut self, sequence: SequenceNumber) -> ConsensusResult {
        // Check if already agreed
        if let Some(qualia) = self.agreed_qualia.get(&sequence) {
            return ConsensusResult::Agreed(qualia.clone());
        }

        // Check commit messages
        if let Some(commits) = self.commit_messages.get(&sequence) {
            if commits.len() >= 2 * self.f_byzantine + 1 {
                // Consensus reached!
                if let Some(qualia) = self.pending_proposals.get(&sequence) {
                    self.agreed_qualia.insert(sequence, qualia.clone());
                    return ConsensusResult::Agreed(qualia.clone());
                }
            }
        }

        ConsensusResult::Pending
    }

    /// Get current consensus status
    pub fn get_agreed_qualia(&self, sequence: SequenceNumber) -> Option<&Quale> {
        self.agreed_qualia.get(&sequence)
    }

    /// Detect hallucinating agents
    pub fn detect_hallucinations(&self, sequence: SequenceNumber) -> Vec<AgentId> {
        let mut hallucinating = Vec::new();

        if let Some(agreed) = self.agreed_qualia.get(&sequence) {
            // Check prepare messages
            if let Some(prepares) = self.prepare_messages.get(&sequence) {
                for (&agent_id, msg) in prepares {
                    if let QualiaMessage::QualiaPrepare { qualia, .. } = msg {
                        if qualia != agreed {
                            hallucinating.push(agent_id);
                        }
                    }
                }
            }
        }

        hallucinating
    }
}

/// Simplified voting-based consensus (for comparison)
pub struct QualiaVotingConsensus {
    votes: HashMap<Quale, HashSet<AgentId>>,
    n_agents: usize,
    f_byzantine: usize,
}

impl QualiaVotingConsensus {
    pub fn new(n_agents: usize) -> Self {
        let f_byzantine = (n_agents - 1) / 3;

        Self {
            votes: HashMap::new(),
            n_agents,
            f_byzantine,
        }
    }

    /// Add a vote
    pub fn vote(&mut self, agent_id: AgentId, qualia: Quale) {
        self.votes
            .entry(qualia)
            .or_insert_with(HashSet::new)
            .insert(agent_id);
    }

    /// Get consensus result
    pub fn get_consensus(&self) -> ConsensusResult {
        // Find quale with most votes
        let mut max_votes = 0;
        let mut consensus_quale: Option<Quale> = None;

        for (qualia, voters) in &self.votes {
            if voters.len() > max_votes {
                max_votes = voters.len();
                consensus_quale = Some(qualia.clone());
            }
        }

        // Need 2f + 1 votes for Byzantine tolerance
        if max_votes >= 2 * self.f_byzantine + 1 {
            ConsensusResult::Agreed(consensus_quale.unwrap())
        } else if self.votes.values().map(|v| v.len()).sum::<usize>() >= self.n_agents {
            // All agents voted but no consensus
            ConsensusResult::Failed
        } else {
            ConsensusResult::Pending
        }
    }

    /// Detect which agents are hallucinating
    pub fn detect_hallucinations(&self) -> Vec<AgentId> {
        if let ConsensusResult::Agreed(consensus_quale) = self.get_consensus() {
            let mut hallucinating = Vec::new();

            for (quale, voters) in &self.votes {
                if quale != &consensus_quale {
                    hallucinating.extend(voters.iter());
                }
            }

            hallucinating
        } else {
            Vec::new()
        }
    }

    /// Get vote counts
    pub fn vote_counts(&self) -> Vec<(Quale, usize)> {
        self.votes
            .iter()
            .map(|(q, voters)| (q.clone(), voters.len()))
            .collect()
    }
}

/// Distance metric between qualia
pub fn qualia_distance(q1: &Quale, q2: &Quale) -> f64 {
    // Different modality = maximum distance
    if q1.modality != q2.modality {
        return 1.0;
    }

    // Same modality, different content
    if q1.content != q2.content {
        return 0.5;
    }

    // Same content, intensity difference
    (q1.intensity_f64() - q2.intensity_f64()).abs()
}

/// Consensus coordinator managing multiple nodes
pub struct ConsensusCoordinator {
    nodes: HashMap<AgentId, QualiaConsensusNode>,
}

impl ConsensusCoordinator {
    pub fn new(agent_ids: Vec<AgentId>) -> Self {
        let n_agents = agent_ids.len();
        let mut nodes = HashMap::new();

        for &agent_id in &agent_ids {
            nodes.insert(agent_id, QualiaConsensusNode::new(agent_id, n_agents));
        }

        Self { nodes }
    }

    /// Broadcast message to all nodes
    pub fn broadcast(&mut self, msg: QualiaMessage) -> Vec<QualiaMessage> {
        let mut responses = Vec::new();

        for node in self.nodes.values_mut() {
            if let Some(response) = node.process_message(msg.clone()) {
                responses.push(response);
            }
        }

        responses
    }

    /// Run consensus round
    pub fn run_consensus_round(&mut self, leader_id: AgentId, qualia: Quale) -> ConsensusResult {
        // Leader proposes
        let proposal = self
            .nodes
            .get_mut(&leader_id)
            .unwrap()
            .propose_qualia(qualia);

        // Broadcast proposal
        let prepares = self.broadcast(proposal);

        // Broadcast prepares
        for prepare in prepares {
            let commits = self.broadcast(prepare);

            // Broadcast commits
            for commit in commits {
                self.broadcast(commit);
            }
        }

        // Check consensus in any node (should be same across all honest nodes)
        if let Some(node) = self.nodes.values_mut().next() {
            node.check_consensus(0)
        } else {
            ConsensusResult::Failed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voting_consensus_success() {
        let mut consensus = QualiaVotingConsensus::new(10);

        let red_apple = Quale::new("vision".to_string(), "red apple".to_string(), 0.9);

        // 7 agents vote for red apple
        for i in 0..7 {
            consensus.vote(i, red_apple.clone());
        }

        // 3 Byzantine agents vote for green apple
        let green_apple = Quale::new("vision".to_string(), "green apple".to_string(), 0.9);
        for i in 7..10 {
            consensus.vote(i, green_apple.clone());
        }

        let result = consensus.get_consensus();
        assert_eq!(result, ConsensusResult::Agreed(red_apple));

        let hallucinating = consensus.detect_hallucinations();
        assert_eq!(hallucinating.len(), 3); // 3 Byzantine agents detected
    }

    #[test]
    fn test_voting_consensus_failure() {
        let mut consensus = QualiaVotingConsensus::new(10);

        let red = Quale::new("vision".to_string(), "red".to_string(), 0.9);
        let blue = Quale::new("vision".to_string(), "blue".to_string(), 0.9);

        // Equal split (5-5)
        for i in 0..5 {
            consensus.vote(i, red.clone());
        }
        for i in 5..10 {
            consensus.vote(i, blue.clone());
        }

        let result = consensus.get_consensus();
        assert_eq!(result, ConsensusResult::Failed); // No 2f+1 majority
    }

    #[test]
    fn test_pbft_node_basic() {
        let mut node = QualiaConsensusNode::new(1, 4); // 4 nodes, f=1

        let qualia = Quale::new("vision".to_string(), "red".to_string(), 0.8);

        // Node 1 proposes
        let proposal = node.propose_qualia(qualia.clone());

        // Simulate receiving own proposal
        let prepare = node.process_message(proposal);
        assert!(prepare.is_some());

        // Also need to record the prepare from self
        if let Some(QualiaMessage::QualiaPrepare {
            qualia: q,
            view,
            sequence,
            agent_id,
        }) = prepare
        {
            node.handle_prepare(q, view, sequence, agent_id);
        }

        // Simulate receiving prepare from 2 other nodes (total 3, >= 2f+1)
        node.handle_prepare(qualia.clone(), 0, 0, 2);
        node.handle_prepare(qualia.clone(), 0, 0, 3);

        // Should be ready to commit
        let commit_msg = node.check_ready_to_commit(0);
        assert!(commit_msg.is_some());

        // Simulate receiving commit messages
        node.handle_commit(qualia.clone(), 0, 0, 1);
        node.handle_commit(qualia.clone(), 0, 0, 2);
        node.handle_commit(qualia.clone(), 0, 0, 3);

        // Check consensus
        let result = node.check_consensus(0);
        assert_eq!(result, ConsensusResult::Agreed(qualia));
    }

    #[test]
    fn test_qualia_distance() {
        let q1 = Quale::new("vision".to_string(), "red".to_string(), 0.8);
        let q2 = Quale::new("vision".to_string(), "red".to_string(), 0.6);
        let q3 = Quale::new("vision".to_string(), "blue".to_string(), 0.8);
        let q4 = Quale::new("audio".to_string(), "beep".to_string(), 0.8);

        assert!(qualia_distance(&q1, &q2) < 0.3); // Same content, different intensity
        assert_eq!(qualia_distance(&q1, &q3), 0.5); // Different content
        assert_eq!(qualia_distance(&q1, &q4), 1.0); // Different modality
    }

    #[test]
    fn test_hallucination_detection() {
        let mut node = QualiaConsensusNode::new(1, 4);

        let correct_qualia = Quale::new("vision".to_string(), "red".to_string(), 0.8);
        let hallucination = Quale::new("vision".to_string(), "unicorn".to_string(), 1.0);

        // Set pending proposal to correct qualia
        node.pending_proposals.insert(0, correct_qualia.clone());

        // Agents 1,2,3 see red (correct)
        node.handle_prepare(correct_qualia.clone(), 0, 0, 1);
        node.handle_prepare(correct_qualia.clone(), 0, 0, 2);
        node.handle_prepare(correct_qualia.clone(), 0, 0, 3);

        // Agent 4 hallucinates unicorn
        node.handle_prepare(hallucination.clone(), 0, 0, 4);

        // Commits
        node.handle_commit(correct_qualia.clone(), 0, 0, 1);
        node.handle_commit(correct_qualia.clone(), 0, 0, 2);
        node.handle_commit(correct_qualia.clone(), 0, 0, 3);

        let result = node.check_consensus(0);
        assert_eq!(result, ConsensusResult::Agreed(correct_qualia));

        let hallucinating = node.detect_hallucinations(0);
        assert!(
            hallucinating.contains(&4),
            "Agent 4 should be detected as hallucinating"
        );
    }
}
