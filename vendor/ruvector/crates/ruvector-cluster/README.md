# Ruvector Cluster

[![Crates.io](https://img.shields.io/crates/v/ruvector-cluster.svg)](https://crates.io/crates/ruvector-cluster)
[![Documentation](https://docs.rs/ruvector-cluster/badge.svg)](https://docs.rs/ruvector-cluster)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**Distributed clustering and sharding for Ruvector vector databases.**

`ruvector-cluster` provides horizontal scaling capabilities with consistent hashing, shard management, and cluster coordination. Enables Ruvector to scale to billions of vectors across multiple nodes. Part of the [Ruvector](https://github.com/ruvnet/ruvector) ecosystem.

## Why Ruvector Cluster?

- **Horizontal Scaling**: Distribute data across multiple nodes
- **Consistent Hashing**: Minimal rebalancing on cluster changes
- **Auto-Sharding**: Automatic shard distribution and balancing
- **Fault Tolerant**: Handle node failures gracefully
- **Async-First**: Built on Tokio for high-performance networking

## Features

### Core Capabilities

- **Cluster Membership**: Node discovery and health monitoring
- **Consistent Hashing**: Ketama/Jump hash for shard placement
- **Shard Management**: Create, migrate, and balance shards
- **Node Coordination**: Leader election and consensus
- **Failure Detection**: Heartbeat-based failure detection

### Advanced Features

- **Dynamic Rebalancing**: Auto-balance on node join/leave
- **Rack Awareness**: Place replicas across failure domains
- **Hot Spot Detection**: Identify and redistribute hot shards
- **Gradual Migration**: Zero-downtime shard migration
- **Cluster Metrics**: Prometheus-compatible metrics

## Installation

Add `ruvector-cluster` to your `Cargo.toml`:

```toml
[dependencies]
ruvector-cluster = "0.1.1"
```

## Quick Start

### Initialize Cluster

```rust
use ruvector_cluster::{Cluster, ClusterConfig, Node};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure cluster
    let config = ClusterConfig {
        node_id: "node-1".to_string(),
        listen_addr: "0.0.0.0:7000".parse()?,
        seeds: vec!["10.0.0.1:7000".parse()?, "10.0.0.2:7000".parse()?],
        replication_factor: 3,
        num_shards: 64,
        ..Default::default()
    };

    // Create and start cluster
    let cluster = Cluster::new(config).await?;
    cluster.start().await?;

    // Wait for cluster to stabilize
    cluster.wait_for_stable().await?;

    println!("Cluster ready with {} nodes", cluster.node_count().await);

    Ok(())
}
```

### Shard Operations

```rust
use ruvector_cluster::{Cluster, ShardId};

// Get shard for a vector ID
let shard_id = cluster.get_shard_for_key("vector-123")?;

// Get nodes hosting a shard
let nodes = cluster.get_shard_nodes(shard_id).await?;
println!("Shard {} hosted on: {:?}", shard_id, nodes);

// Manual shard migration
cluster.migrate_shard(shard_id, target_node).await?;

// Trigger rebalance
cluster.rebalance().await?;
```

### Cluster Health

```rust
// Check cluster health
let health = cluster.health().await?;
println!("Status: {:?}", health.status);
println!("Healthy nodes: {}/{}", health.healthy_nodes, health.total_nodes);

// Get node status
for node in cluster.nodes().await? {
    println!("{}: {:?} (last seen: {})",
        node.id,
        node.status,
        node.last_heartbeat
    );
}
```

## API Overview

### Core Types

```rust
// Cluster configuration
pub struct ClusterConfig {
    pub node_id: String,
    pub listen_addr: SocketAddr,
    pub seeds: Vec<SocketAddr>,
    pub replication_factor: usize,
    pub num_shards: usize,
    pub heartbeat_interval: Duration,
    pub failure_timeout: Duration,
}

// Node information
pub struct Node {
    pub id: String,
    pub addr: SocketAddr,
    pub status: NodeStatus,
    pub shards: Vec<ShardId>,
    pub last_heartbeat: DateTime<Utc>,
}

// Shard information
pub struct Shard {
    pub id: ShardId,
    pub primary: NodeId,
    pub replicas: Vec<NodeId>,
    pub status: ShardStatus,
    pub size_bytes: u64,
}
```

### Cluster Operations

```rust
impl Cluster {
    pub async fn new(config: ClusterConfig) -> Result<Self>;
    pub async fn start(&self) -> Result<()>;
    pub async fn stop(&self) -> Result<()>;

    // Membership
    pub async fn nodes(&self) -> Result<Vec<Node>>;
    pub async fn node_count(&self) -> usize;
    pub async fn is_leader(&self) -> bool;

    // Sharding
    pub fn get_shard_for_key(&self, key: &str) -> Result<ShardId>;
    pub async fn get_shard_nodes(&self, shard: ShardId) -> Result<Vec<Node>>;
    pub async fn migrate_shard(&self, shard: ShardId, target: &NodeId) -> Result<()>;

    // Health
    pub async fn health(&self) -> Result<ClusterHealth>;
    pub async fn rebalance(&self) -> Result<()>;
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Cluster                               │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐        │
│  │ Node 1  │  │ Node 2  │  │ Node 3  │  │ Node 4  │        │
│  │ Shards: │  │ Shards: │  │ Shards: │  │ Shards: │        │
│  │ 0,4,8   │  │ 1,5,9   │  │ 2,6,10  │  │ 3,7,11  │        │
│  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘        │
│       │            │            │            │              │
│       └────────────┴────────────┴────────────┘              │
│                    Gossip Protocol                          │
└─────────────────────────────────────────────────────────────┘
```

## Related Crates

- **[ruvector-core](../ruvector-core/)** - Core vector database engine
- **[ruvector-raft](../ruvector-raft/)** - RAFT consensus
- **[ruvector-replication](../ruvector-replication/)** - Data replication

## Documentation

- **[Main README](../../README.md)** - Complete project overview
- **[API Documentation](https://docs.rs/ruvector-cluster)** - Full API reference
- **[GitHub Repository](https://github.com/ruvnet/ruvector)** - Source code

## License

**MIT License** - see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [Ruvector](https://github.com/ruvnet/ruvector) - Built by [rUv](https://ruv.io)**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)

[Documentation](https://docs.rs/ruvector-cluster) | [Crates.io](https://crates.io/crates/ruvector-cluster) | [GitHub](https://github.com/ruvnet/ruvector)

</div>
