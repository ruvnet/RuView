//! Distributed Systems Integration Tests
//!
//! Comprehensive test suite for horizontal scaling components:
//! - Raft consensus protocol
//! - Multi-master replication
//! - Auto-sharding with consistent hashing
//!
//! These tests simulate a distributed environment similar to E2B sandboxes

pub mod raft_consensus_tests;
pub mod replication_tests;
pub mod sharding_tests;
pub mod cluster_integration_tests;
pub mod performance_benchmarks;
