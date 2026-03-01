//! Mock QuDAG Server for testing

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct MockQuDagServer {
    proposals: Arc<Mutex<HashMap<String, MockProposal>>>,
    patterns: Arc<Mutex<Vec<MockPattern>>>,
    balances: Arc<Mutex<HashMap<String, f64>>>,
}

#[derive(Debug, Clone)]
pub struct MockProposal {
    pub id: String,
    pub status: String,
    pub votes_for: u64,
    pub votes_against: u64,
    pub finalized: bool,
}

#[derive(Debug, Clone)]
pub struct MockPattern {
    pub id: String,
    pub vector: Vec<f32>,
    pub round: u64,
}

impl MockQuDagServer {
    pub fn new() -> Self {
        Self {
            proposals: Arc::new(Mutex::new(HashMap::new())),
            patterns: Arc::new(Mutex::new(Vec::new())),
            balances: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn endpoint(&self) -> String {
        "mock://localhost:8443".to_string()
    }

    pub fn submit_proposal(&self, vector: Vec<f32>) -> String {
        let id = format!("prop_{}", rand::random::<u64>());

        let proposal = MockProposal {
            id: id.clone(),
            status: "pending".to_string(),
            votes_for: 0,
            votes_against: 0,
            finalized: false,
        };

        self.proposals.lock().unwrap().insert(id.clone(), proposal);
        id
    }

    pub fn get_proposal(&self, id: &str) -> Option<MockProposal> {
        self.proposals.lock().unwrap().get(id).cloned()
    }

    pub fn finalize_proposal(&self, id: &str, accept: bool) {
        if let Some(proposal) = self.proposals.lock().unwrap().get_mut(id) {
            proposal.status = if accept { "accepted" } else { "rejected" }.to_string();
            proposal.finalized = true;
            proposal.votes_for = if accept { 100 } else { 30 };
            proposal.votes_against = if accept { 20 } else { 70 };
        }
    }

    pub fn add_pattern(&self, vector: Vec<f32>, round: u64) -> String {
        let id = format!("pat_{}", rand::random::<u64>());

        self.patterns.lock().unwrap().push(MockPattern {
            id: id.clone(),
            vector,
            round,
        });

        id
    }

    pub fn get_patterns_since(&self, round: u64) -> Vec<MockPattern> {
        self.patterns.lock().unwrap()
            .iter()
            .filter(|p| p.round >= round)
            .cloned()
            .collect()
    }

    pub fn set_balance(&self, node_id: &str, balance: f64) {
        self.balances.lock().unwrap().insert(node_id.to_string(), balance);
    }

    pub fn get_balance(&self, node_id: &str) -> f64 {
        self.balances.lock().unwrap().get(node_id).copied().unwrap_or(0.0)
    }

    pub fn stake(&self, node_id: &str, amount: f64) -> Result<(), String> {
        let mut balances = self.balances.lock().unwrap();
        let balance = balances.get(node_id).copied().unwrap_or(0.0);

        if balance < amount {
            return Err("Insufficient balance".to_string());
        }

        balances.insert(node_id.to_string(), balance - amount);
        Ok(())
    }
}

impl Default for MockQuDagServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a pre-populated mock server for testing
pub fn create_test_server() -> MockQuDagServer {
    let server = MockQuDagServer::new();

    // Add some patterns
    for round in 0..10 {
        let vector: Vec<f32> = (0..256).map(|i| (i as f32 / 256.0).sin()).collect();
        server.add_pattern(vector, round);
    }

    // Set up balances
    server.set_balance("test_node", 1000.0);

    server
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_submit_proposal() {
        let server = MockQuDagServer::new();
        let vector = vec![0.1; 256];

        let id = server.submit_proposal(vector);
        assert!(id.starts_with("prop_"));

        let proposal = server.get_proposal(&id).unwrap();
        assert_eq!(proposal.status, "pending");
        assert_eq!(proposal.votes_for, 0);
    }

    #[test]
    fn test_finalize_proposal() {
        let server = MockQuDagServer::new();
        let id = server.submit_proposal(vec![0.1; 256]);

        server.finalize_proposal(&id, true);

        let proposal = server.get_proposal(&id).unwrap();
        assert_eq!(proposal.status, "accepted");
        assert!(proposal.finalized);
        assert!(proposal.votes_for > proposal.votes_against);
    }

    #[test]
    fn test_add_pattern() {
        let server = MockQuDagServer::new();
        let vector = vec![0.2; 128];

        let id = server.add_pattern(vector.clone(), 5);
        assert!(id.starts_with("pat_"));

        let patterns = server.get_patterns_since(5);
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].round, 5);
    }

    #[test]
    fn test_stake() {
        let server = MockQuDagServer::new();
        server.set_balance("node1", 1000.0);

        assert!(server.stake("node1", 100.0).is_ok());
        assert_eq!(server.get_balance("node1"), 900.0);

        assert!(server.stake("node1", 2000.0).is_err());
    }

    #[test]
    fn test_create_test_server() {
        let server = create_test_server();

        let patterns = server.get_patterns_since(0);
        assert_eq!(patterns.len(), 10);

        assert_eq!(server.get_balance("test_node"), 1000.0);
    }
}
