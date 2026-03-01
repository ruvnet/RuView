//! Leader election implementation
//!
//! Implements the Raft leader election algorithm including:
//! - Randomized election timeouts
//! - Vote request handling
//! - Term management
//! - Split vote prevention

use crate::{NodeId, Term};
use rand::Rng;
use std::time::Duration;
use tokio::time::Instant;

/// Election timer with randomized timeout
#[derive(Debug)]
pub struct ElectionTimer {
    /// Last time the timer was reset
    last_reset: Instant,

    /// Current timeout duration
    timeout: Duration,

    /// Minimum election timeout (milliseconds)
    min_timeout_ms: u64,

    /// Maximum election timeout (milliseconds)
    max_timeout_ms: u64,
}

impl ElectionTimer {
    /// Create a new election timer
    pub fn new(min_timeout_ms: u64, max_timeout_ms: u64) -> Self {
        let timeout = Self::random_timeout(min_timeout_ms, max_timeout_ms);
        Self {
            last_reset: Instant::now(),
            timeout,
            min_timeout_ms,
            max_timeout_ms,
        }
    }

    /// Create with default timeouts (150-300ms as per Raft paper)
    pub fn with_defaults() -> Self {
        Self::new(150, 300)
    }

    /// Reset the election timer with a new random timeout
    pub fn reset(&mut self) {
        self.last_reset = Instant::now();
        self.timeout = Self::random_timeout(self.min_timeout_ms, self.max_timeout_ms);
    }

    /// Check if the election timeout has elapsed
    pub fn is_elapsed(&self) -> bool {
        self.last_reset.elapsed() >= self.timeout
    }

    /// Get time remaining until timeout
    pub fn time_remaining(&self) -> Duration {
        self.timeout.saturating_sub(self.last_reset.elapsed())
    }

    /// Generate a random timeout duration
    fn random_timeout(min_ms: u64, max_ms: u64) -> Duration {
        let mut rng = rand::thread_rng();
        let timeout_ms = rng.gen_range(min_ms..=max_ms);
        Duration::from_millis(timeout_ms)
    }

    /// Get the current timeout duration
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

/// Vote tracker for an election
#[derive(Debug)]
pub struct VoteTracker {
    /// Votes received in favor
    votes_received: Vec<NodeId>,

    /// Total number of nodes in the cluster
    cluster_size: usize,

    /// Required number of votes for quorum
    quorum_size: usize,
}

impl VoteTracker {
    /// Create a new vote tracker
    pub fn new(cluster_size: usize) -> Self {
        let quorum_size = (cluster_size / 2) + 1;
        Self {
            votes_received: Vec::new(),
            cluster_size,
            quorum_size,
        }
    }

    /// Record a vote from a node
    pub fn record_vote(&mut self, node_id: NodeId) {
        if !self.votes_received.contains(&node_id) {
            self.votes_received.push(node_id);
        }
    }

    /// Check if quorum has been reached
    pub fn has_quorum(&self) -> bool {
        self.votes_received.len() >= self.quorum_size
    }

    /// Get the number of votes received
    pub fn vote_count(&self) -> usize {
        self.votes_received.len()
    }

    /// Get the required quorum size
    pub fn quorum_size(&self) -> usize {
        self.quorum_size
    }

    /// Reset the vote tracker
    pub fn reset(&mut self) {
        self.votes_received.clear();
    }
}

/// Election state machine
#[derive(Debug)]
pub struct ElectionState {
    /// Current election timer
    pub timer: ElectionTimer,

    /// Vote tracker for current election
    pub votes: VoteTracker,

    /// Current term being contested
    pub current_term: Term,
}

impl ElectionState {
    /// Create a new election state
    pub fn new(cluster_size: usize, min_timeout_ms: u64, max_timeout_ms: u64) -> Self {
        Self {
            timer: ElectionTimer::new(min_timeout_ms, max_timeout_ms),
            votes: VoteTracker::new(cluster_size),
            current_term: 0,
        }
    }

    /// Start a new election for the given term
    pub fn start_election(&mut self, term: Term, self_id: &NodeId) {
        self.current_term = term;
        self.votes.reset();
        self.votes.record_vote(self_id.clone());
        self.timer.reset();
    }

    /// Reset the election timer (when receiving valid heartbeat)
    pub fn reset_timer(&mut self) {
        self.timer.reset();
    }

    /// Check if election timeout has occurred
    pub fn should_start_election(&self) -> bool {
        self.timer.is_elapsed()
    }

    /// Record a vote and check if we won
    pub fn record_vote(&mut self, node_id: NodeId) -> bool {
        self.votes.record_vote(node_id);
        self.votes.has_quorum()
    }

    /// Update cluster size
    pub fn update_cluster_size(&mut self, cluster_size: usize) {
        self.votes = VoteTracker::new(cluster_size);
    }
}

/// Vote request validation
pub struct VoteValidator;

impl VoteValidator {
    /// Validate if a vote request should be granted
    ///
    /// A vote should be granted if:
    /// 1. The candidate's term is at least as current as receiver's term
    /// 2. The receiver hasn't voted in this term, or has voted for this candidate
    /// 3. The candidate's log is at least as up-to-date as receiver's log
    pub fn should_grant_vote(
        receiver_term: Term,
        receiver_voted_for: &Option<NodeId>,
        receiver_last_log_index: u64,
        receiver_last_log_term: Term,
        candidate_id: &NodeId,
        candidate_term: Term,
        candidate_last_log_index: u64,
        candidate_last_log_term: Term,
    ) -> bool {
        // Reject if candidate's term is older
        if candidate_term < receiver_term {
            return false;
        }

        // Check if we can vote for this candidate
        let can_vote = match receiver_voted_for {
            None => true,
            Some(voted_for) => voted_for == candidate_id,
        };

        if !can_vote {
            return false;
        }

        // Check if candidate's log is at least as up-to-date
        Self::is_log_up_to_date(
            candidate_last_log_term,
            candidate_last_log_index,
            receiver_last_log_term,
            receiver_last_log_index,
        )
    }

    /// Check if candidate's log is at least as up-to-date as receiver's
    ///
    /// Raft determines which of two logs is more up-to-date by comparing
    /// the index and term of the last entries in the logs. If the logs have
    /// last entries with different terms, then the log with the later term
    /// is more up-to-date. If the logs end with the same term, then whichever
    /// log is longer is more up-to-date.
    fn is_log_up_to_date(
        candidate_last_term: Term,
        candidate_last_index: u64,
        receiver_last_term: Term,
        receiver_last_index: u64,
    ) -> bool {
        if candidate_last_term != receiver_last_term {
            candidate_last_term >= receiver_last_term
        } else {
            candidate_last_index >= receiver_last_index
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_election_timer() {
        let mut timer = ElectionTimer::new(50, 100);
        assert!(!timer.is_elapsed());

        sleep(Duration::from_millis(150));
        assert!(timer.is_elapsed());

        timer.reset();
        assert!(!timer.is_elapsed());
    }

    #[test]
    fn test_vote_tracker() {
        let mut tracker = VoteTracker::new(5);
        assert_eq!(tracker.quorum_size(), 3);
        assert!(!tracker.has_quorum());

        tracker.record_vote("node1".to_string());
        assert!(!tracker.has_quorum());

        tracker.record_vote("node2".to_string());
        assert!(!tracker.has_quorum());

        tracker.record_vote("node3".to_string());
        assert!(tracker.has_quorum());
    }

    #[test]
    fn test_election_state() {
        let mut state = ElectionState::new(5, 50, 100);
        let self_id = "node1".to_string();

        state.start_election(1, &self_id);
        assert_eq!(state.current_term, 1);
        assert_eq!(state.votes.vote_count(), 1);

        let won = state.record_vote("node2".to_string());
        assert!(!won);

        let won = state.record_vote("node3".to_string());
        assert!(won);
    }

    #[test]
    fn test_vote_validation() {
        // Should grant vote when candidate is up-to-date
        assert!(VoteValidator::should_grant_vote(
            1,
            &None,
            10,
            1,
            &"candidate".to_string(),
            2,
            10,
            1
        ));

        // Should reject when candidate term is older
        assert!(!VoteValidator::should_grant_vote(
            2,
            &None,
            10,
            1,
            &"candidate".to_string(),
            1,
            10,
            1
        ));

        // Should reject when already voted for someone else
        assert!(!VoteValidator::should_grant_vote(
            1,
            &Some("other".to_string()),
            10,
            1,
            &"candidate".to_string(),
            1,
            10,
            1
        ));

        // Should grant when voted for same candidate
        assert!(VoteValidator::should_grant_vote(
            1,
            &Some("candidate".to_string()),
            10,
            1,
            &"candidate".to_string(),
            1,
            10,
            1
        ));
    }

    #[test]
    fn test_log_up_to_date() {
        // Higher term is more up-to-date
        assert!(VoteValidator::is_log_up_to_date(2, 5, 1, 10));
        assert!(!VoteValidator::is_log_up_to_date(1, 10, 2, 5));

        // Same term, longer log is more up-to-date
        assert!(VoteValidator::is_log_up_to_date(1, 10, 1, 5));
        assert!(!VoteValidator::is_log_up_to_date(1, 5, 1, 10));

        // Same term and length is up-to-date
        assert!(VoteValidator::is_log_up_to_date(1, 10, 1, 10));
    }
}
