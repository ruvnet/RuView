//! Swarm coordination state management.
//!
//! Tracks agent state changes and consensus votes in-memory, with the
//! coordination state serialized alongside the RVF store. State entries
//! and votes are appended chronologically for audit and replay.

/// A recorded agent state change.
#[derive(Clone, Debug, PartialEq)]
pub struct StateEntry {
    /// The agent that produced this state change.
    pub agent_id: String,
    /// State key (e.g., "status", "role", "topology").
    pub key: String,
    /// State value (e.g., "active", "coordinator", "mesh").
    pub value: String,
    /// Timestamp in nanoseconds since the Unix epoch.
    pub timestamp: u64,
}

/// A consensus vote cast by an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct ConsensusVote {
    /// The topic being voted on (e.g., "leader-election-42").
    pub topic: String,
    /// The agent casting the vote.
    pub agent_id: String,
    /// The vote (true = approve, false = reject).
    pub vote: bool,
    /// Timestamp in nanoseconds since the Unix epoch.
    pub timestamp: u64,
}

/// Swarm coordination state tracker.
///
/// Maintains an in-memory log of agent state changes and consensus votes.
/// This state lives alongside the RVF store and is used for coordination
/// protocol decisions (leader election, topology changes, etc.).
pub struct SwarmCoordination {
    states: Vec<StateEntry>,
    votes: Vec<ConsensusVote>,
}

impl SwarmCoordination {
    /// Create a new, empty coordination tracker.
    pub fn new() -> Self {
        Self {
            states: Vec::new(),
            votes: Vec::new(),
        }
    }

    /// Record an agent state change.
    pub fn record_state(
        &mut self,
        agent_id: &str,
        state_key: &str,
        state_value: &str,
    ) -> Result<(), CoordinationError> {
        if agent_id.is_empty() {
            return Err(CoordinationError::EmptyAgentId);
        }
        if state_key.is_empty() {
            return Err(CoordinationError::EmptyKey);
        }
        self.states.push(StateEntry {
            agent_id: agent_id.to_string(),
            key: state_key.to_string(),
            value: state_value.to_string(),
            timestamp: now_ns(),
        });
        Ok(())
    }

    /// Get the state history for a specific agent.
    pub fn get_agent_states(&self, agent_id: &str) -> Vec<StateEntry> {
        self.states
            .iter()
            .filter(|s| s.agent_id == agent_id)
            .cloned()
            .collect()
    }

    /// Get all coordination state entries.
    pub fn get_all_states(&self) -> Vec<StateEntry> {
        self.states.clone()
    }

    /// Record a consensus vote for a topic.
    pub fn record_consensus_vote(
        &mut self,
        topic: &str,
        agent_id: &str,
        vote: bool,
    ) -> Result<(), CoordinationError> {
        if topic.is_empty() {
            return Err(CoordinationError::EmptyTopic);
        }
        if agent_id.is_empty() {
            return Err(CoordinationError::EmptyAgentId);
        }
        self.votes.push(ConsensusVote {
            topic: topic.to_string(),
            agent_id: agent_id.to_string(),
            vote,
            timestamp: now_ns(),
        });
        Ok(())
    }

    /// Get all votes for a specific topic.
    pub fn get_votes(&self, topic: &str) -> Vec<ConsensusVote> {
        self.votes
            .iter()
            .filter(|v| v.topic == topic)
            .cloned()
            .collect()
    }

    /// Get the total number of state entries.
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    /// Get the total number of votes.
    pub fn vote_count(&self) -> usize {
        self.votes.len()
    }
}

impl Default for SwarmCoordination {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors from coordination operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoordinationError {
    /// Agent ID must not be empty.
    EmptyAgentId,
    /// State key must not be empty.
    EmptyKey,
    /// Topic must not be empty.
    EmptyTopic,
}

impl std::fmt::Display for CoordinationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyAgentId => write!(f, "agent_id must not be empty"),
            Self::EmptyKey => write!(f, "state key must not be empty"),
            Self::EmptyTopic => write!(f, "topic must not be empty"),
        }
    }
}

impl std::error::Error for CoordinationError {}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_get_states() {
        let mut coord = SwarmCoordination::new();
        coord.record_state("a1", "status", "active").unwrap();
        coord.record_state("a2", "status", "idle").unwrap();
        coord.record_state("a1", "role", "coordinator").unwrap();

        let a1_states = coord.get_agent_states("a1");
        assert_eq!(a1_states.len(), 2);
        assert_eq!(a1_states[0].key, "status");
        assert_eq!(a1_states[1].key, "role");

        let a2_states = coord.get_agent_states("a2");
        assert_eq!(a2_states.len(), 1);
    }

    #[test]
    fn get_all_states() {
        let mut coord = SwarmCoordination::new();
        coord.record_state("a1", "k1", "v1").unwrap();
        coord.record_state("a2", "k2", "v2").unwrap();

        let all = coord.get_all_states();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn record_and_get_votes() {
        let mut coord = SwarmCoordination::new();
        coord
            .record_consensus_vote("leader-election", "a1", true)
            .unwrap();
        coord
            .record_consensus_vote("leader-election", "a2", false)
            .unwrap();
        coord
            .record_consensus_vote("other-topic", "a1", true)
            .unwrap();

        let votes = coord.get_votes("leader-election");
        assert_eq!(votes.len(), 2);
        assert!(votes[0].vote);
        assert!(!votes[1].vote);

        let other = coord.get_votes("other-topic");
        assert_eq!(other.len(), 1);
    }

    #[test]
    fn empty_agent_id_rejected() {
        let mut coord = SwarmCoordination::new();
        assert_eq!(
            coord.record_state("", "k", "v"),
            Err(CoordinationError::EmptyAgentId)
        );
        assert_eq!(
            coord.record_consensus_vote("topic", "", true),
            Err(CoordinationError::EmptyAgentId)
        );
    }

    #[test]
    fn empty_key_rejected() {
        let mut coord = SwarmCoordination::new();
        assert_eq!(
            coord.record_state("a1", "", "v"),
            Err(CoordinationError::EmptyKey)
        );
    }

    #[test]
    fn empty_topic_rejected() {
        let mut coord = SwarmCoordination::new();
        assert_eq!(
            coord.record_consensus_vote("", "a1", true),
            Err(CoordinationError::EmptyTopic)
        );
    }

    #[test]
    fn counts() {
        let mut coord = SwarmCoordination::new();
        assert_eq!(coord.state_count(), 0);
        assert_eq!(coord.vote_count(), 0);

        coord.record_state("a1", "k", "v").unwrap();
        coord.record_consensus_vote("t", "a1", true).unwrap();

        assert_eq!(coord.state_count(), 1);
        assert_eq!(coord.vote_count(), 1);
    }

    #[test]
    fn no_states_for_unknown_agent() {
        let coord = SwarmCoordination::new();
        assert!(coord.get_agent_states("ghost").is_empty());
    }

    #[test]
    fn no_votes_for_unknown_topic() {
        let coord = SwarmCoordination::new();
        assert!(coord.get_votes("nonexistent").is_empty());
    }

    #[test]
    fn timestamps_are_monotonic() {
        let mut coord = SwarmCoordination::new();
        coord.record_state("a1", "k1", "v1").unwrap();
        coord.record_state("a1", "k2", "v2").unwrap();

        let states = coord.get_agent_states("a1");
        assert!(states[0].timestamp <= states[1].timestamp);
    }
}
