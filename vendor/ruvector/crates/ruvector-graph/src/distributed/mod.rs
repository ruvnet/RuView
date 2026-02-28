//! Distributed graph query capabilities
//!
//! This module provides comprehensive distributed and federated graph operations:
//! - Graph sharding with multiple partitioning strategies
//! - Distributed query coordination and execution
//! - Cross-cluster federation for multi-cluster queries
//! - Graph-aware replication extending ruvector-replication
//! - Gossip-based cluster membership and health monitoring
//! - High-performance gRPC communication layer

pub mod coordinator;
pub mod federation;
pub mod gossip;
pub mod replication;
pub mod rpc;
pub mod shard;

pub use coordinator::{Coordinator, QueryPlan, ShardCoordinator};
pub use federation::{ClusterRegistry, FederatedQuery, Federation, RemoteCluster};
pub use gossip::{GossipConfig, GossipMembership, MembershipEvent, NodeHealth};
pub use replication::{GraphReplication, GraphReplicationConfig, ReplicationStrategy};
pub use rpc::{GraphRpcService, RpcClient, RpcServer};
pub use shard::{
    EdgeCutMinimizer, GraphShard, HashPartitioner, RangePartitioner, ShardMetadata, ShardStrategy,
};
