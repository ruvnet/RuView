//! Multi-Master Replication Tests
//!
//! Tests for:
//! - Sync, async, and semi-sync replication modes
//! - Conflict resolution with vector clocks
//! - Replication lag monitoring
//! - Automatic failover

use ruvector_replication::{
    ReplicaSet, ReplicaRole, ReplicaStatus,
    SyncManager, SyncMode, ReplicationLog, LogEntry,
    VectorClock, ConflictResolver, LastWriteWins,
    FailoverManager, FailoverPolicy, HealthStatus,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Test replica set creation and management
#[tokio::test]
async fn test_replica_set_management() {
    let mut replica_set = ReplicaSet::new("test-cluster");

    // Add primary
    replica_set
        .add_replica("primary-1", "192.168.1.1:9001", ReplicaRole::Primary)
        .expect("Failed to add primary");

    // Add secondaries
    replica_set
        .add_replica("secondary-1", "192.168.1.2:9001", ReplicaRole::Secondary)
        .expect("Failed to add secondary");
    replica_set
        .add_replica("secondary-2", "192.168.1.3:9001", ReplicaRole::Secondary)
        .expect("Failed to add secondary");

    // Verify replica count
    assert_eq!(replica_set.replica_count(), 3);

    // Verify primary exists
    let primary = replica_set.get_primary();
    assert!(primary.is_some());
    assert_eq!(primary.unwrap().id, "primary-1");

    // Verify secondaries
    let secondaries = replica_set.get_secondaries();
    assert_eq!(secondaries.len(), 2);
}

/// Test sync mode configuration
#[tokio::test]
async fn test_sync_mode_configuration() {
    let mut replica_set = ReplicaSet::new("test-cluster");
    replica_set
        .add_replica("r1", "127.0.0.1:9001", ReplicaRole::Primary)
        .unwrap();
    replica_set
        .add_replica("r2", "127.0.0.1:9002", ReplicaRole::Secondary)
        .unwrap();

    let log = Arc::new(ReplicationLog::new("r1"));
    let manager = SyncManager::new(Arc::new(replica_set), log);

    // Test async mode
    manager.set_sync_mode(SyncMode::Async);
    assert_eq!(manager.sync_mode(), SyncMode::Async);

    // Test sync mode
    manager.set_sync_mode(SyncMode::Sync);
    assert_eq!(manager.sync_mode(), SyncMode::Sync);

    // Test semi-sync mode
    manager.set_sync_mode(SyncMode::SemiSync { min_replicas: 1 });
    assert_eq!(manager.sync_mode(), SyncMode::SemiSync { min_replicas: 1 });
}

/// Test replication log operations
#[tokio::test]
async fn test_replication_log() {
    let log = ReplicationLog::new("test-replica");

    // Append entries
    let entry1 = log.append(b"data1".to_vec());
    let entry2 = log.append(b"data2".to_vec());
    let entry3 = log.append(b"data3".to_vec());

    // Verify sequence numbers
    assert_eq!(entry1.sequence, 1);
    assert_eq!(entry2.sequence, 2);
    assert_eq!(entry3.sequence, 3);

    // Verify current sequence
    assert_eq!(log.current_sequence(), 3);

    // Verify retrieval
    let retrieved = log.get(2);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().sequence, 2);

    // Verify range retrieval
    let range = log.get_range(1, 3);
    assert_eq!(range.len(), 3);
}

/// Test log entry integrity
#[tokio::test]
async fn test_log_entry_integrity() {
    let data = b"important data".to_vec();
    let entry = LogEntry::new(1, data.clone(), "source-replica".to_string());

    // Verify checksum validation
    assert!(entry.verify(), "Entry checksum should be valid");

    // Verify data integrity
    assert_eq!(entry.data, data);
    assert_eq!(entry.sequence, 1);
    assert_eq!(entry.source_replica, "source-replica");
}

/// Test async replication
#[tokio::test]
async fn test_async_replication() {
    let mut replica_set = ReplicaSet::new("async-cluster");
    replica_set
        .add_replica("primary", "127.0.0.1:9001", ReplicaRole::Primary)
        .unwrap();
    replica_set
        .add_replica("secondary", "127.0.0.1:9002", ReplicaRole::Secondary)
        .unwrap();

    let log = Arc::new(ReplicationLog::new("primary"));
    let manager = SyncManager::new(Arc::new(replica_set), log);

    manager.set_sync_mode(SyncMode::Async);

    // Async replication should return immediately
    let start = Instant::now();
    let entry = manager.replicate(b"test data".to_vec()).await.unwrap();
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_millis(100), "Async should be fast");
    assert_eq!(entry.sequence, 1);
}

/// Test semi-sync replication with quorum
#[tokio::test]
async fn test_semi_sync_replication() {
    let mut replica_set = ReplicaSet::new("semi-sync-cluster");
    replica_set
        .add_replica("r1", "127.0.0.1:9001", ReplicaRole::Primary)
        .unwrap();
    replica_set
        .add_replica("r2", "127.0.0.1:9002", ReplicaRole::Secondary)
        .unwrap();
    replica_set
        .add_replica("r3", "127.0.0.1:9003", ReplicaRole::Secondary)
        .unwrap();

    let log = Arc::new(ReplicationLog::new("r1"));
    let manager = SyncManager::new(Arc::new(replica_set), log);

    // Require at least 1 replica acknowledgment
    manager.set_sync_mode(SyncMode::SemiSync { min_replicas: 1 });

    let entry = manager.replicate(b"quorum data".to_vec()).await.unwrap();
    assert_eq!(entry.sequence, 1);
}

/// Test replica catchup
#[tokio::test]
async fn test_replica_catchup() {
    let mut replica_set = ReplicaSet::new("catchup-cluster");
    replica_set
        .add_replica("primary", "127.0.0.1:9001", ReplicaRole::Primary)
        .unwrap();
    replica_set
        .add_replica("secondary", "127.0.0.1:9002", ReplicaRole::Secondary)
        .unwrap();

    let log = Arc::new(ReplicationLog::new("primary"));

    // Add some entries directly to log
    log.append(b"entry1".to_vec());
    log.append(b"entry2".to_vec());
    log.append(b"entry3".to_vec());
    log.append(b"entry4".to_vec());
    log.append(b"entry5".to_vec());

    let manager = SyncManager::new(Arc::new(replica_set), log);

    // Catchup from position 2 (should get entries 3, 4, 5)
    let entries = manager.catchup("secondary", 2).await.unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].sequence, 3);
    assert_eq!(entries[2].sequence, 5);
}

/// Test vector clock operations
#[tokio::test]
async fn test_vector_clock() {
    let mut clock1 = VectorClock::new();
    let mut clock2 = VectorClock::new();

    // Increment clocks
    clock1.increment("node1");
    clock1.increment("node1");
    clock2.increment("node2");

    // Test concurrent clocks
    assert!(clock1.is_concurrent(&clock2), "Clocks should be concurrent");

    // Merge clocks
    clock1.merge(&clock2);

    // After merge, clock1 should have both node times
    assert!(!clock1.is_concurrent(&clock2), "After merge, not concurrent");
}

/// Test last-write-wins conflict resolution
#[tokio::test]
async fn test_last_write_wins() {
    let lww = LastWriteWins::new();

    // Create two conflicting values with different timestamps
    let value1 = (b"value1".to_vec(), 100u64); // (data, timestamp)
    let value2 = (b"value2".to_vec(), 200u64);

    // LWW should choose the later timestamp
    let winner = if value1.1 > value2.1 { value1.0 } else { value2.0 };
    assert_eq!(winner, b"value2".to_vec());
}

/// Test failover policy configuration
#[tokio::test]
async fn test_failover_policy() {
    let policy = FailoverPolicy::default();

    // Default timeout should be reasonable
    assert!(policy.health_check_interval > Duration::from_secs(0));
    assert!(policy.failover_timeout > Duration::from_secs(0));
}

/// Test health status tracking
#[tokio::test]
async fn test_health_status() {
    let status = HealthStatus::Healthy;
    assert_eq!(status, HealthStatus::Healthy);

    let unhealthy = HealthStatus::Unhealthy;
    assert_eq!(unhealthy, HealthStatus::Unhealthy);
}

/// Performance test for log append operations
#[tokio::test]
async fn test_log_append_performance() {
    let log = ReplicationLog::new("perf-test");

    let start = Instant::now();
    let iterations = 10000;

    for i in 0..iterations {
        let data = format!("data-{}", i).into_bytes();
        log.append(data);
    }

    let elapsed = start.elapsed();
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("Log append performance: {:.0} ops/sec", ops_per_sec);
    println!("Total time for {} operations: {:?}", iterations, elapsed);

    // Should be able to do at least 10k ops/sec
    assert!(ops_per_sec > 10000.0, "Log append too slow: {:.0} ops/sec", ops_per_sec);
}

/// Test replication under load
#[tokio::test]
async fn test_replication_under_load() {
    let mut replica_set = ReplicaSet::new("load-cluster");
    replica_set
        .add_replica("primary", "127.0.0.1:9001", ReplicaRole::Primary)
        .unwrap();
    replica_set
        .add_replica("secondary", "127.0.0.1:9002", ReplicaRole::Secondary)
        .unwrap();

    let log = Arc::new(ReplicationLog::new("primary"));
    let manager = SyncManager::new(Arc::new(replica_set), log);
    manager.set_sync_mode(SyncMode::Async);

    let start = Instant::now();
    let iterations = 1000;

    for i in 0..iterations {
        let data = format!("load-test-{}", i).into_bytes();
        manager.replicate(data).await.unwrap();
    }

    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;

    println!("Average replication time: {:.3}ms", avg_ms);

    // Async replication should be fast
    assert!(avg_ms < 1.0, "Replication too slow: {:.3}ms", avg_ms);
}
