# Ruvector Raft

[![Crates.io](https://img.shields.io/crates/v/ruvector-raft.svg)](https://crates.io/crates/ruvector-raft)
[![Documentation](https://docs.rs/ruvector-raft/badge.svg)](https://docs.rs/ruvector-raft)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**Raft consensus implementation for Ruvector distributed metadata coordination.**

`ruvector-raft` provides a production-ready Raft consensus implementation for coordinating distributed Ruvector deployments. Ensures strong consistency for cluster metadata, configuration, and leader election. Part of the [Ruvector](https://github.com/ruvnet/ruvector) ecosystem.

## Why Ruvector Raft?

- **Strong Consistency**: Linearizable reads and writes
- **Leader Election**: Automatic failover on leader failure
- **Log Replication**: Durable, replicated transaction log
- **Membership Changes**: Dynamic cluster reconfiguration
- **Snapshot Support**: Log compaction via snapshots

## Features

### Core Capabilities

- **Raft Consensus**: Full Raft protocol implementation
- **Leader Election**: Randomized timeouts, pre-vote protocol
- **Log Replication**: Pipelined append entries
- **Commit Management**: Majority-based commit tracking
- **State Machine**: Generic state machine interface

### Advanced Features

- **Pre-Vote Protocol**: Prevents disruption during network partitions
- **Leadership Transfer**: Graceful leader handoff
- **Read Index**: Linearizable reads without log entry
- **Learner Nodes**: Non-voting members for scaling reads
- **Batch Commits**: Coalesce multiple entries per commit

## Installation

Add `ruvector-raft` to your `Cargo.toml`:

```toml
[dependencies]
ruvector-raft = "0.1.1"
```

## Quick Start

### Create Raft Node

```rust
use ruvector_raft::{RaftNode, RaftConfig, StateMachine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure Raft node
    let config = RaftConfig {
        node_id: 1,
        peers: vec![2, 3],  // Other node IDs
        election_timeout_min: Duration::from_millis(150),
        election_timeout_max: Duration::from_millis(300),
        heartbeat_interval: Duration::from_millis(50),
        ..Default::default()
    };

    // Create state machine
    let state_machine = MyStateMachine::new();

    // Create and start Raft node
    let node = RaftNode::new(config, state_machine).await?;
    node.start().await?;

    // Wait for leader election
    node.wait_for_leader().await?;

    Ok(())
}
```

### Implement State Machine

```rust
use ruvector_raft::{StateMachine, Entry, Snapshot};

struct MyStateMachine {
    data: HashMap<String, String>,
}

impl StateMachine for MyStateMachine {
    type Command = MyCommand;
    type Response = MyResponse;

    fn apply(&mut self, entry: &Entry<Self::Command>) -> Self::Response {
        match &entry.command {
            MyCommand::Set { key, value } => {
                self.data.insert(key.clone(), value.clone());
                MyResponse::Ok
            }
            MyCommand::Get { key } => {
                MyResponse::Value(self.data.get(key).cloned())
            }
            MyCommand::Delete { key } => {
                self.data.remove(key);
                MyResponse::Ok
            }
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            data: bincode::serialize(&self.data).unwrap(),
            last_index: self.last_applied,
            last_term: self.last_term,
        }
    }

    fn restore(&mut self, snapshot: &Snapshot) {
        self.data = bincode::deserialize(&snapshot.data).unwrap();
    }
}
```

### Propose Commands

```rust
// Propose a command (only succeeds on leader)
let response = node.propose(MyCommand::Set {
    key: "foo".to_string(),
    value: "bar".to_string(),
}).await?;

// Read with linearizable consistency
let response = node.read_index(MyCommand::Get {
    key: "foo".to_string(),
}).await?;

// Check leadership
if node.is_leader().await {
    println!("This node is the leader");
}
```

## API Overview

### Core Types

```rust
// Raft configuration
pub struct RaftConfig {
    pub node_id: NodeId,
    pub peers: Vec<NodeId>,
    pub election_timeout_min: Duration,
    pub election_timeout_max: Duration,
    pub heartbeat_interval: Duration,
    pub max_entries_per_append: usize,
    pub snapshot_threshold: u64,
}

// Log entry
pub struct Entry<C> {
    pub index: u64,
    pub term: u64,
    pub command: C,
}

// Snapshot
pub struct Snapshot {
    pub data: Vec<u8>,
    pub last_index: u64,
    pub last_term: u64,
}

// Node state
pub enum NodeState {
    Follower,
    Candidate,
    Leader,
    Learner,
}
```

### Node Operations

```rust
impl<S: StateMachine> RaftNode<S> {
    pub async fn new(config: RaftConfig, state_machine: S) -> Result<Self>;
    pub async fn start(&self) -> Result<()>;
    pub async fn stop(&self) -> Result<()>;

    // Leadership
    pub async fn is_leader(&self) -> bool;
    pub async fn leader_id(&self) -> Option<NodeId>;
    pub async fn wait_for_leader(&self) -> Result<NodeId>;

    // Commands
    pub async fn propose(&self, command: S::Command) -> Result<S::Response>;
    pub async fn read_index(&self, command: S::Command) -> Result<S::Response>;

    // Cluster management
    pub async fn add_node(&self, node_id: NodeId) -> Result<()>;
    pub async fn remove_node(&self, node_id: NodeId) -> Result<()>;
    pub async fn transfer_leadership(&self, target: NodeId) -> Result<()>;
}
```

## Architecture

```
┌────────────────────────────────────────────────────────┐
│                     Raft Cluster                        │
│                                                        │
│   ┌──────────┐    ┌──────────┐    ┌──────────┐        │
│   │  Node 1  │    │  Node 2  │    │  Node 3  │        │
│   │ (Leader) │───▶│(Follower)│    │(Follower)│        │
│   │          │    │          │    │          │        │
│   │ Log:     │    │ Log:     │    │ Log:     │        │
│   │ [1,2,3]  │───▶│ [1,2,3]  │    │ [1,2,3]  │        │
│   └──────────┘    └──────────┘    └──────────┘        │
│         │                               ▲              │
│         └───────────────────────────────┘              │
│                  AppendEntries RPC                     │
└────────────────────────────────────────────────────────┘
```

## Related Crates

- **[ruvector-core](../ruvector-core/)** - Core vector database engine
- **[ruvector-cluster](../ruvector-cluster/)** - Clustering and sharding
- **[ruvector-replication](../ruvector-replication/)** - Data replication

## Documentation

- **[Main README](../../README.md)** - Complete project overview
- **[API Documentation](https://docs.rs/ruvector-raft)** - Full API reference
- **[GitHub Repository](https://github.com/ruvnet/ruvector)** - Source code

## License

**MIT License** - see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [Ruvector](https://github.com/ruvnet/ruvector) - Built by [rUv](https://ruv.io)**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)

[Documentation](https://docs.rs/ruvector-raft) | [Crates.io](https://crates.io/crates/ruvector-raft) | [GitHub](https://github.com/ruvnet/ruvector)

</div>
