//! Federated Vector Search - Distributed Similarity Search Across ESP32 Clusters
//!
//! Enables vector search across multiple ESP32 chips for:
//! - Larger knowledge bases (1M+ vectors across cluster)
//! - Faster search (parallel query execution)
//! - Resilient systems (no single point of failure)
//! - Distributed embeddings (each chip stores subset)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                     FEDERATED VECTOR SEARCH                                 │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │                                                                             │
//! │   Query: "What is machine learning?"                                        │
//! │              │                                                              │
//! │              ▼                                                              │
//! │   ┌─────────────────┐                                                       │
//! │   │  Coordinator    │ ──▶ Broadcast query to all shards                     │
//! │   │  (Chip 0)       │                                                       │
//! │   └─────────────────┘                                                       │
//! │          │ │ │ │                                                            │
//! │          ▼ ▼ ▼ ▼                                                            │
//! │   ┌────┐ ┌────┐ ┌────┐ ┌────┐                                               │
//! │   │ S1 │ │ S2 │ │ S3 │ │ S4 │  ◀── Each shard searches locally             │
//! │   └────┘ └────┘ └────┘ └────┘                                               │
//! │     │      │      │      │                                                  │
//! │     └──────┴──────┴──────┘                                                  │
//! │              │                                                              │
//! │              ▼                                                              │
//! │   ┌─────────────────┐                                                       │
//! │   │  Merge Results  │ ──▶ Return top-k globally                             │
//! │   └─────────────────┘                                                       │
//! │                                                                             │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```

use heapless::Vec as HVec;
use super::{MicroHNSW, HNSWConfig, SearchResult, MicroVector, DistanceMetric, MAX_VECTORS};

/// Maximum shards in federation
pub const MAX_SHARDS: usize = 16;
/// Local shard capacity
pub const SHARD_CAPACITY: usize = 256;
/// Shard embedding dimension
pub const SHARD_DIM: usize = 32;

/// Shard configuration
#[derive(Debug, Clone)]
pub struct ShardConfig {
    /// Shard ID (0-indexed)
    pub shard_id: u8,
    /// Total shards in federation
    pub total_shards: u8,
    /// This chip's role
    pub role: ShardRole,
    /// Replication factor (1 = no replication)
    pub replication: u8,
}

/// Role of this chip in the federation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShardRole {
    /// Coordinator: receives queries, distributes, merges
    Coordinator,
    /// Worker: stores vectors, processes local queries
    Worker,
    /// Hybrid: both coordinator and worker
    Hybrid,
}

/// Query message between chips
#[derive(Debug, Clone)]
pub struct ShardQuery {
    /// Query ID for tracking
    pub query_id: u32,
    /// Query embedding
    pub embedding: HVec<i8, SHARD_DIM>,
    /// Number of results requested per shard
    pub k: u8,
    /// Source chip ID
    pub source: u8,
}

/// Response from a shard
#[derive(Debug, Clone)]
pub struct ShardResponse {
    /// Query ID this responds to
    pub query_id: u32,
    /// Shard that processed the query
    pub shard_id: u8,
    /// Results from this shard
    pub results: HVec<ShardResult, 16>,
    /// Processing time in microseconds
    pub latency_us: u32,
}

/// Single result from a shard
#[derive(Debug, Clone, Copy)]
pub struct ShardResult {
    /// Vector ID
    pub id: u32,
    /// Distance
    pub distance: i32,
    /// Shard ID where vector lives
    pub shard_id: u8,
}

/// Federated Index (local view)
pub struct FederatedIndex {
    /// Configuration
    config: ShardConfig,
    /// Local HNSW index
    local_index: MicroHNSW<SHARD_DIM, SHARD_CAPACITY>,
    /// Pending queries (for coordinator)
    pending_queries: HVec<(u32, u8), 16>,  // (query_id, responses_received)
    /// Collected results (for merging)
    collected_results: HVec<ShardResult, 64>,
    /// Next query ID
    next_query_id: u32,
    /// Statistics
    local_query_count: u32,
    federated_query_count: u32,
}

impl FederatedIndex {
    /// Create new federated index
    pub fn new(config: ShardConfig) -> Self {
        let hnsw_config = HNSWConfig {
            m: 6,
            m_max0: 12,
            ef_construction: 24,
            ef_search: 16,
            metric: DistanceMetric::Euclidean,
            binary_mode: false,
        };

        Self {
            config,
            local_index: MicroHNSW::new(hnsw_config),
            pending_queries: HVec::new(),
            collected_results: HVec::new(),
            next_query_id: 0,
            local_query_count: 0,
            federated_query_count: 0,
        }
    }

    /// Insert vector into local shard
    pub fn insert(&mut self, vector: &MicroVector<SHARD_DIM>) -> Result<usize, &'static str> {
        // Check if this vector belongs to this shard (hash-based sharding)
        let shard_for_id = (vector.id as usize) % (self.config.total_shards as usize);

        if shard_for_id != self.config.shard_id as usize {
            return Err("Vector belongs to different shard");
        }

        self.local_index.insert(vector)
    }

    /// Insert vector regardless of sharding (for local-only mode)
    pub fn insert_local(&mut self, vector: &MicroVector<SHARD_DIM>) -> Result<usize, &'static str> {
        self.local_index.insert(vector)
    }

    /// Number of vectors in local shard
    pub fn local_count(&self) -> usize {
        self.local_index.len()
    }

    /// Estimated total vectors across federation
    pub fn estimated_total(&self) -> usize {
        self.local_index.len() * self.config.total_shards as usize
    }

    /// Local search only
    pub fn search_local(&mut self, query: &[i8], k: usize) -> HVec<SearchResult, 32> {
        self.local_query_count += 1;
        self.local_index.search(query, k)
    }

    /// Create a federated query (for coordinator)
    pub fn create_query(&mut self, embedding: &[i8], k: u8) -> ShardQuery {
        let query_id = self.next_query_id;
        self.next_query_id += 1;
        self.federated_query_count += 1;

        // Track pending query
        let _ = self.pending_queries.push((query_id, 0));

        let mut embed = HVec::new();
        for &v in embedding.iter().take(SHARD_DIM) {
            let _ = embed.push(v);
        }

        ShardQuery {
            query_id,
            embedding: embed,
            k,
            source: self.config.shard_id,
        }
    }

    /// Process incoming query (for workers)
    pub fn process_query(&mut self, query: &ShardQuery) -> ShardResponse {
        let start = 0u32; // Would use actual timer on ESP32

        let local_results = self.local_index.search(&query.embedding, query.k as usize);

        let mut results = HVec::new();
        for r in local_results.iter() {
            let _ = results.push(ShardResult {
                id: r.id,
                distance: r.distance,
                shard_id: self.config.shard_id,
            });
        }

        let latency = 100u32; // Simulated

        ShardResponse {
            query_id: query.query_id,
            shard_id: self.config.shard_id,
            results,
            latency_us: latency,
        }
    }

    /// Collect response from shard (for coordinator)
    pub fn collect_response(&mut self, response: ShardResponse) {
        // Add results to collected
        for r in response.results.iter() {
            let _ = self.collected_results.push(*r);
        }

        // Update pending query
        for (qid, count) in self.pending_queries.iter_mut() {
            if *qid == response.query_id {
                *count += 1;
                break;
            }
        }
    }

    /// Check if all responses received
    pub fn is_query_complete(&self, query_id: u32) -> bool {
        for (qid, count) in self.pending_queries.iter() {
            if *qid == query_id {
                return *count >= self.config.total_shards;
            }
        }
        false
    }

    /// Merge and return final results
    pub fn merge_results(&mut self, query_id: u32, k: usize) -> HVec<ShardResult, 32> {
        // Sort by distance
        self.collected_results.sort_by_key(|r| r.distance);

        // Take top k
        let mut final_results = HVec::new();
        for r in self.collected_results.iter().take(k) {
            let _ = final_results.push(*r);
        }

        // Clean up
        self.collected_results.clear();
        self.pending_queries.retain(|(qid, _)| *qid != query_id);

        final_results
    }

    /// Get shard ID for a vector ID
    pub fn shard_for_id(vector_id: u32, total_shards: u8) -> u8 {
        (vector_id % total_shards as u32) as u8
    }

    /// Get configuration
    pub fn config(&self) -> &ShardConfig {
        &self.config
    }

    /// Get statistics
    pub fn stats(&self) -> (u32, u32) {
        (self.local_query_count, self.federated_query_count)
    }
}

/// Swarm Vector Store - Shared vector memory across swarm
pub struct SwarmVectorStore {
    /// Local shard
    shard: FederatedIndex,
    /// Peer chip IDs
    peers: HVec<u8, MAX_SHARDS>,
    /// Shared knowledge count per peer
    peer_counts: HVec<u32, MAX_SHARDS>,
}

impl SwarmVectorStore {
    /// Create swarm vector store
    pub fn new(chip_id: u8, total_chips: u8) -> Self {
        let config = ShardConfig {
            shard_id: chip_id,
            total_shards: total_chips,
            role: if chip_id == 0 { ShardRole::Hybrid } else { ShardRole::Worker },
            replication: 1,
        };

        let mut peers = HVec::new();
        let mut peer_counts = HVec::new();
        for i in 0..total_chips {
            if i != chip_id {
                let _ = peers.push(i);
                let _ = peer_counts.push(0);
            }
        }

        Self {
            shard: FederatedIndex::new(config),
            peers,
            peer_counts,
        }
    }

    /// Store shared knowledge
    pub fn share_knowledge(&mut self, embedding: &[i8], id: u32) -> Result<(), &'static str> {
        let mut vec_data = HVec::new();
        for &v in embedding.iter().take(SHARD_DIM) {
            vec_data.push(v).map_err(|_| "Overflow")?;
        }

        let vec = MicroVector { data: vec_data, id };
        self.shard.insert_local(&vec)?;
        Ok(())
    }

    /// Query swarm knowledge
    pub fn query_swarm(&mut self, embedding: &[i8], k: usize) -> HVec<SearchResult, 32> {
        // For now, just query local shard
        // In real implementation, would broadcast to peers
        self.shard.search_local(embedding, k)
    }

    /// Sync with peer (called when communication received)
    pub fn sync_peer(&mut self, peer_id: u8, vectors: &[(u32, HVec<i8, SHARD_DIM>)]) {
        for (id, embedding) in vectors {
            let vec = MicroVector { data: embedding.clone(), id: *id };
            let _ = self.shard.insert_local(&vec);
        }

        // Update peer count
        if let Some(pos) = self.peers.iter().position(|&p| p == peer_id) {
            if pos < self.peer_counts.len() {
                self.peer_counts[pos] += vectors.len() as u32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_federated_index() {
        let config = ShardConfig {
            shard_id: 0,
            total_shards: 4,
            role: ShardRole::Hybrid,
            replication: 1,
        };

        let mut index = FederatedIndex::new(config);

        // Insert vectors that hash to this shard
        for i in (0..20).step_by(4) {  // IDs 0, 4, 8, 12, 16 belong to shard 0
            let data: HVec<i8, SHARD_DIM> = (0..SHARD_DIM).map(|j| ((i + j) % 100) as i8).collect();
            let vec = MicroVector { data, id: i as u32 };
            index.insert(&vec).unwrap();
        }

        assert!(index.local_count() > 0);
    }

    #[test]
    fn test_swarm_store() {
        let mut store = SwarmVectorStore::new(0, 4);

        for i in 0..10 {
            let embedding = [(i * 10) as i8; SHARD_DIM];
            store.share_knowledge(&embedding, i).unwrap();
        }

        let query = [25i8; SHARD_DIM];
        let results = store.query_swarm(&query, 3);
        assert!(!results.is_empty());
    }
}
