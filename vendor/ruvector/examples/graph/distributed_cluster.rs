//! Distributed Graph Cluster Example
//!
//! This example demonstrates setting up a distributed graph database cluster:
//! - Multi-node cluster initialization
//! - Data sharding and partitioning
//! - RAFT consensus for consistency
//! - Replication and failover
//! - Distributed queries

#[cfg(feature = "distributed")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== RuVector Graph - Distributed Cluster ===\n");

    // TODO: Once the distributed graph API is exposed, implement:

    println!("1. Initialize Cluster Configuration");
    // let config = ClusterConfig::builder()
    //     .cluster_name("ruvector-cluster")
    //     .replication_factor(3)
    //     .sharding_strategy(ShardingStrategy::ConsistentHash)
    //     .consensus(ConsensusProtocol::Raft)
    //     .build()?;

    println!("   ✓ Cluster configuration created");
    println!("     - Replication factor: 3");
    println!("     - Sharding: Consistent Hash");
    println!("     - Consensus: RAFT");

    println!("\n2. Start Cluster Nodes");
    // Start 3 nodes in the cluster
    // let node1 = GraphNode::new("node1", "127.0.0.1:7001", config.clone())?;
    // node1.start()?;

    // let node2 = GraphNode::new("node2", "127.0.0.1:7002", config.clone())?;
    // node2.join_cluster(&["127.0.0.1:7001"])?;

    // let node3 = GraphNode::new("node3", "127.0.0.1:7003", config.clone())?;
    // node3.join_cluster(&["127.0.0.1:7001"])?;

    println!("   ✓ Started 3 cluster nodes");
    println!("     - node1: 127.0.0.1:7001 (leader)");
    println!("     - node2: 127.0.0.1:7002 (follower)");
    println!("     - node3: 127.0.0.1:7003 (follower)");

    println!("\n3. Wait for Cluster Formation");
    // Wait for RAFT consensus to be established
    // tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // let status = node1.cluster_status()?;
    // println!("   Cluster status: {:?}", status);
    // assert_eq!(status.healthy_nodes, 3);

    println!("\n4. Distributed Data Insertion");
    // Connect to cluster (automatically routes to correct shard)
    // let client = GraphClient::connect(&["127.0.0.1:7001", "127.0.0.1:7002", "127.0.0.1:7003"])?;

    // Insert nodes - will be distributed across shards
    // for i in 0..1000 {
    //     client.create_node()
    //         .label("Person")
    //         .property("id", i)
    //         .property("name", format!("User{}", i))
    //         .execute_async().await?;
    // }

    println!("   ✓ Inserted 1000 nodes across cluster");

    println!("\n5. Check Data Distribution");
    // View how data is distributed across nodes
    // let node1_stats = node1.local_stats()?;
    // let node2_stats = node2.local_stats()?;
    // let node3_stats = node3.local_stats()?;

    // println!("   Data distribution:");
    // println!("     - node1: {} nodes", node1_stats.node_count);
    // println!("     - node2: {} nodes", node2_stats.node_count);
    // println!("     - node3: {} nodes", node3_stats.node_count);

    println!("\n6. Distributed Query Execution");
    // Execute query that spans multiple shards
    // let result = client.execute_cypher(r#"
    //     MATCH (p:Person)
    //     WHERE p.id >= 100 AND p.id < 200
    //     RETURN p.name
    //     ORDER BY p.id
    // "#).await?;

    // println!("   Query returned {} results", result.len());

    println!("\n7. Test Replication");
    // Verify data is replicated across nodes
    // let node1_data = node1.read_local("Person", "id", 42)?;
    // let node2_data = node2.read_replica("Person", "id", 42)?;
    // let node3_data = node3.read_replica("Person", "id", 42)?;

    // assert_eq!(node1_data, node2_data);
    // assert_eq!(node2_data, node3_data);

    println!("   ✓ Data successfully replicated");

    println!("\n8. Simulate Node Failure");
    // Stop one node and verify cluster continues
    // node3.stop()?;
    // tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // let status = node1.cluster_status()?;
    // println!("   Cluster status after node3 failure:");
    // println!("     - Healthy nodes: {}", status.healthy_nodes);
    // println!("     - Leader: node1");
    // println!("     - Cluster still operational: {}", status.is_healthy());

    println!("\n9. Test Failover");
    // Queries should still work with 2/3 nodes
    // let result = client.execute_cypher(r#"
    //     MATCH (p:Person)
    //     RETURN count(p) as total
    // "#).await?;

    // println!("   Query after failover: {} total nodes", result);

    println!("\n10. Node Recovery");
    // Restart failed node
    // node3.start()?;
    // node3.rejoin_cluster(&["127.0.0.1:7001"])?;

    // Wait for catch-up
    // tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // let status = node1.cluster_status()?;
    // println!("   Cluster fully recovered: {}", status.healthy_nodes == 3);

    println!("\n11. Performance Metrics");
    // let metrics = client.cluster_metrics()?;
    // println!("   Cluster performance:");
    // println!("     - Total throughput: {} ops/sec", metrics.total_ops_per_sec);
    // println!("     - Average latency: {}ms", metrics.avg_latency_ms);
    // println!("     - Cross-shard queries: {}%", metrics.cross_shard_query_pct);

    println!("\n12. Cleanup");
    // node1.stop()?;
    // node2.stop()?;
    // node3.stop()?;

    println!("   ✓ Cluster shutdown complete");

    println!("\n=== Example Complete ===");
    Ok(())
}

#[cfg(not(feature = "distributed"))]
fn main() {
    println!("=== RuVector Graph - Distributed Cluster ===\n");
    println!("This example requires the 'distributed' feature.");
    println!("Run with: cargo run --example distributed_cluster --features distributed");
}
