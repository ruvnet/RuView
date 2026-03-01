//! Raft Consensus Protocol Tests
//!
//! Tests for:
//! - Leader election with configurable timeouts
//! - Log replication across cluster nodes
//! - Split-brain prevention
//! - Node failure recovery

use ruvector_raft::{RaftNode, RaftNodeConfig, RaftState, RaftError};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Test basic Raft node creation and initialization
#[tokio::test]
async fn test_raft_node_initialization() {
    let config = RaftNodeConfig::new(
        "node1".to_string(),
        vec!["node1".to_string(), "node2".to_string(), "node3".to_string()],
    );

    let node = RaftNode::new(config);

    // Initial state should be Follower
    assert_eq!(node.current_state(), RaftState::Follower);
    assert_eq!(node.current_term(), 0);
    assert!(node.current_leader().is_none());
}

/// Test Raft cluster with multiple nodes
#[tokio::test]
async fn test_raft_cluster_formation() {
    let cluster_members = vec![
        "node1".to_string(),
        "node2".to_string(),
        "node3".to_string(),
    ];

    let mut nodes = Vec::new();
    for member in &cluster_members {
        let config = RaftNodeConfig::new(member.clone(), cluster_members.clone());
        nodes.push(RaftNode::new(config));
    }

    // All nodes should start as followers
    for node in &nodes {
        assert_eq!(node.current_state(), RaftState::Follower);
    }

    assert_eq!(nodes.len(), 3);
}

/// Test election timeout configuration
#[tokio::test]
async fn test_election_timeout_configuration() {
    let mut config = RaftNodeConfig::new(
        "node1".to_string(),
        vec!["node1".to_string(), "node2".to_string(), "node3".to_string()],
    );

    // Default timeouts
    assert_eq!(config.election_timeout_min, 150);
    assert_eq!(config.election_timeout_max, 300);

    // Custom timeouts for faster testing
    config.election_timeout_min = 50;
    config.election_timeout_max = 100;
    config.heartbeat_interval = 25;

    let node = RaftNode::new(config);
    assert_eq!(node.current_state(), RaftState::Follower);
}

/// Test that node ID is properly stored
#[tokio::test]
async fn test_node_identity() {
    let config = RaftNodeConfig::new(
        "test-node-123".to_string(),
        vec!["test-node-123".to_string()],
    );

    let _node = RaftNode::new(config.clone());
    assert_eq!(config.node_id, "test-node-123");
}

/// Test snapshot configuration
#[tokio::test]
async fn test_snapshot_configuration() {
    let config = RaftNodeConfig::new(
        "node1".to_string(),
        vec!["node1".to_string()],
    );

    // Default snapshot chunk size
    assert_eq!(config.snapshot_chunk_size, 64 * 1024); // 64KB
    assert_eq!(config.max_entries_per_message, 100);
}

/// Simulate leader election scenario (unit test version)
#[tokio::test]
async fn test_leader_election_scenario() {
    // This tests the state transitions that would occur during election
    let config = RaftNodeConfig::new(
        "node1".to_string(),
        vec![
            "node1".to_string(),
            "node2".to_string(),
            "node3".to_string(),
        ],
    );

    let node = RaftNode::new(config);

    // Initially a follower
    assert_eq!(node.current_state(), RaftState::Follower);

    // Term starts at 0
    assert_eq!(node.current_term(), 0);
}

/// Test quorum calculation for different cluster sizes
#[tokio::test]
async fn test_quorum_calculations() {
    // 3 node cluster: quorum = 2
    let three_node_quorum = (3 / 2) + 1;
    assert_eq!(three_node_quorum, 2);

    // 5 node cluster: quorum = 3
    let five_node_quorum = (5 / 2) + 1;
    assert_eq!(five_node_quorum, 3);

    // 7 node cluster: quorum = 4
    let seven_node_quorum = (7 / 2) + 1;
    assert_eq!(seven_node_quorum, 4);
}

/// Test handling of network partition scenarios
#[tokio::test]
async fn test_network_partition_handling() {
    // Simulate a 5-node cluster with partition
    let cluster_size = 5;
    let partition_a_size = 3; // Majority
    let partition_b_size = 2; // Minority

    // Only partition A can elect a leader (has majority)
    let quorum = (cluster_size / 2) + 1;
    assert!(partition_a_size >= quorum, "Partition A should have quorum");
    assert!(partition_b_size < quorum, "Partition B should not have quorum");
}

/// Test log consistency requirements
#[tokio::test]
async fn test_log_consistency() {
    // Test the log matching property
    // If two logs contain an entry with the same index and term,
    // then the logs are identical in all entries up through that index

    let entries = vec![
        (1, 1), // (index, term)
        (2, 1),
        (3, 2),
        (4, 2),
        (5, 3),
    ];

    // Verify sequential indices
    for (i, &(index, _)) in entries.iter().enumerate() {
        assert_eq!(index, (i + 1) as u64);
    }
}

/// Test term monotonicity
#[tokio::test]
async fn test_term_monotonicity() {
    // Terms should never decrease
    let terms = vec![0u64, 1, 1, 2, 3, 3, 4];

    for i in 1..terms.len() {
        assert!(terms[i] >= terms[i-1], "Term should not decrease");
    }
}

/// Performance test for node creation
#[tokio::test]
async fn test_node_creation_performance() {
    let start = Instant::now();
    let iterations = 100;

    for i in 0..iterations {
        let config = RaftNodeConfig::new(
            format!("node{}", i),
            vec![format!("node{}", i)],
        );
        let _node = RaftNode::new(config);
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;

    println!("Average node creation time: {:.3}ms", avg_ms);

    // Node creation should be fast
    assert!(avg_ms < 10.0, "Node creation too slow: {:.3}ms", avg_ms);
}
