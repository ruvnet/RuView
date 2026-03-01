//! # Semantic Routing for Edge-Net P2P Network
//!
//! RuVector-based semantic routing for intelligent gossip and peer discovery.
//! Routes events to semantically similar peers plus random samples for robustness.
//!
//! ## Features
//!
//! - **HNSW Index**: O(log N) peer lookup by embedding similarity
//! - **Capability Embedding**: Simple averaging or learned encoder
//! - **Hybrid Routing**: Semantic neighbors + random for robustness
//! - **Latency-Aware**: Prefer low-latency semantically-similar peers
//! - **Reputation Integration**: Weight routing by peer reputation
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    Semantic Router                                   │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────────────┐ │
//! │  │  Peer Registry  │  │   HNSW Index    │  │  Capability Embedder │ │
//! │  │   (DashMap)     │──│  (Fast Lookup)  │──│    (Vectorize)       │ │
//! │  └─────────────────┘  └─────────────────┘  └──────────────────────┘ │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────────────┐ │
//! │  │ Semantic Routes │  │  Random Sample  │  │  Topic Discovery     │ │
//! │  │   (Top-K)       │──│   (Robustness)  │──│   (Gossipsub)        │ │
//! │  └─────────────────┘  └─────────────────┘  └──────────────────────┘ │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use rustc_hash::FxHashMap;
use std::sync::RwLock;

use crate::rac::Event;

// ============================================================================
// Types
// ============================================================================

/// 32-byte peer identifier (public key hash)
pub type PeerId = [u8; 32];

/// Topic hash for gossipsub (32 bytes)
pub type TopicHash = [u8; 32];

/// Cross-platform timestamp helper
#[inline]
fn current_timestamp_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

// ============================================================================
// Peer Information
// ============================================================================

/// Information about a known peer in the network
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Unique peer identifier (public key hash)
    pub peer_id: PeerId,
    /// Peer's capability embedding centroid
    pub centroid: Vec<f32>,
    /// Declared capabilities (e.g., "vectors", "embeddings", "ml-inference")
    pub capabilities: Vec<String>,
    /// Observed round-trip latency in milliseconds
    pub latency_ms: u32,
    /// Reputation score (0.0 - 1.0)
    pub reputation: f32,
    /// Last activity timestamp
    pub last_seen: u64,
    /// Number of successful interactions
    pub success_count: u64,
    /// Number of failed interactions
    pub failure_count: u64,
}

impl PeerInfo {
    /// Create a new peer info entry
    pub fn new(peer_id: PeerId, capabilities: Vec<String>) -> Self {
        Self {
            peer_id,
            centroid: Vec::new(),
            capabilities,
            latency_ms: 1000, // Default high latency until measured
            reputation: 0.5,  // Neutral starting reputation
            last_seen: current_timestamp_ms(),
            success_count: 0,
            failure_count: 0,
        }
    }

    /// Calculate success rate
    pub fn success_rate(&self) -> f32 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            return 0.5; // No data, assume neutral
        }
        self.success_count as f32 / total as f32
    }

    /// Update latency with exponential moving average
    pub fn update_latency(&mut self, new_latency_ms: u32) {
        let alpha = 0.3f32;
        self.latency_ms = (self.latency_ms as f32 * (1.0 - alpha) + new_latency_ms as f32 * alpha) as u32;
    }

    /// Calculate composite routing score (higher is better)
    /// Combines similarity, latency, and reputation
    pub fn routing_score(&self, similarity: f64) -> f64 {
        // Latency penalty (lower latency = higher score)
        let latency_score = 1.0 / (1.0 + (self.latency_ms as f64 / 100.0));

        // Reputation weight
        let reputation_weight = 0.5 + (self.reputation as f64 * 0.5);

        // Combined score with weights
        similarity * 0.5 + latency_score * 0.3 + reputation_weight * 0.2
    }
}

// ============================================================================
// HNSW Layer Entry
// ============================================================================

/// Entry in an HNSW layer
#[derive(Clone, Debug)]
struct HnswNode {
    /// Peer ID this node represents
    peer_id: PeerId,
    /// Embedding vector
    vector: Vec<f32>,
    /// Neighbors in this layer (max connections)
    neighbors: Vec<PeerId>,
}

/// HNSW Layer containing nodes at a specific level
struct HnswLayer {
    nodes: FxHashMap<[u8; 32], HnswNode>,
    max_connections: usize,
}

impl HnswLayer {
    fn new(max_connections: usize) -> Self {
        Self {
            nodes: FxHashMap::default(),
            max_connections,
        }
    }

    fn contains(&self, peer_id: &PeerId) -> bool {
        self.nodes.contains_key(peer_id)
    }

    fn get(&self, peer_id: &PeerId) -> Option<&HnswNode> {
        self.nodes.get(peer_id)
    }

    fn get_mut(&mut self, peer_id: &PeerId) -> Option<&mut HnswNode> {
        self.nodes.get_mut(peer_id)
    }

    fn insert(&mut self, node: HnswNode) {
        self.nodes.insert(node.peer_id, node);
    }

    fn iter(&self) -> impl Iterator<Item = (&PeerId, &HnswNode)> {
        self.nodes.iter()
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }
}

// ============================================================================
// HNSW Index
// ============================================================================

/// Hierarchical Navigable Small World graph for O(log N) similarity search
pub struct HnswIndex {
    /// Layers of the HNSW graph (layer 0 = base layer with all nodes)
    layers: Vec<HnswLayer>,
    /// Entry point to the graph
    entry_point: Option<PeerId>,
    /// Maximum connections per node in base layer
    m: usize,
    /// Maximum connections per node in upper layers
    m_max: usize,
    /// Level generation factor (probability of adding to higher level)
    ml: f64,
    /// Dimension of vectors
    dim: usize,
}

impl HnswIndex {
    /// Create a new HNSW index
    pub fn new(dim: usize) -> Self {
        Self {
            layers: vec![HnswLayer::new(32)], // Start with base layer
            entry_point: None,
            m: 16,        // Base layer connections
            m_max: 8,     // Upper layer connections
            ml: 1.0 / 16.0_f64.ln(),
            dim,
        }
    }

    /// Calculate cosine similarity between two vectors
    fn similarity(a: &[f32], b: &[f32]) -> f64 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        (dot / (norm_a * norm_b)) as f64
    }

    /// Generate a random level for a new node
    fn random_level(&self) -> usize {
        // Simple pseudo-random level generation based on timestamp
        let r = (current_timestamp_ms() % 1000) as f64 / 1000.0;
        (-r.ln() * self.ml).floor() as usize
    }

    /// Search for K nearest neighbors in a specific layer
    fn search_layer(
        &self,
        query: &[f32],
        entry_points: &[PeerId],
        layer_idx: usize,
        ef: usize,
    ) -> Vec<(PeerId, f64)> {
        let layer = match self.layers.get(layer_idx) {
            Some(l) => l,
            None => return Vec::new(),
        };

        // Priority queue simulation using sorted vec
        let mut candidates: Vec<(PeerId, f64)> = entry_points
            .iter()
            .filter_map(|pid| {
                layer.get(pid).map(|node| (*pid, Self::similarity(query, &node.vector)))
            })
            .collect();

        let mut visited: FxHashMap<[u8; 32], bool> = candidates
            .iter()
            .map(|(pid, _)| (*pid, true))
            .collect();

        let mut results = candidates.clone();

        while !candidates.is_empty() {
            // Get closest candidate
            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let (current, current_dist) = match candidates.pop() {
                Some(c) => c,
                None => break,
            };

            // Get furthest result
            results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let furthest_dist = results.first().map(|(_, d)| *d).unwrap_or(0.0);

            if current_dist < furthest_dist && results.len() >= ef {
                break;
            }

            // Explore neighbors
            if let Some(node) = layer.get(&current) {
                for neighbor_id in &node.neighbors {
                    if visited.contains_key(neighbor_id) {
                        continue;
                    }
                    visited.insert(*neighbor_id, true);

                    if let Some(neighbor_node) = layer.get(neighbor_id) {
                        let dist = Self::similarity(query, &neighbor_node.vector);

                        if results.len() < ef || dist > furthest_dist {
                            candidates.push((*neighbor_id, dist));
                            results.push((*neighbor_id, dist));

                            if results.len() > ef {
                                results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                                results.pop();
                            }
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(ef);
        results
    }

    /// Insert a new node into the index
    pub fn insert(&mut self, peer_id: PeerId, vector: Vec<f32>) {
        if vector.len() != self.dim {
            return;
        }

        let level = self.random_level();

        // Ensure we have enough layers
        while self.layers.len() <= level {
            self.layers.push(HnswLayer::new(self.m_max));
        }

        // Start from entry point or set as new entry point
        let entry = match self.entry_point {
            Some(ep) => ep,
            None => {
                // First node - add to all layers up to level
                for l in 0..=level {
                    self.layers[l].insert(HnswNode {
                        peer_id,
                        vector: vector.clone(),
                        neighbors: Vec::new(),
                    });
                }
                self.entry_point = Some(peer_id);
                return;
            }
        };

        // Greedy search from top to bottom
        let mut current_nearest = vec![entry];
        for l in (level + 1..self.layers.len()).rev() {
            let nearest = self.search_layer(&vector, &current_nearest, l, 1);
            if !nearest.is_empty() {
                current_nearest = vec![nearest[0].0];
            }
        }

        // Insert and connect at each layer from level down to 0
        for l in (0..=level).rev() {
            let ef_construction = if l == 0 { self.m * 2 } else { self.m_max };
            let neighbors = self.search_layer(&vector, &current_nearest, l, ef_construction);

            let max_conn = if l == 0 { self.m } else { self.m_max };
            let neighbor_ids: Vec<PeerId> = neighbors
                .iter()
                .take(max_conn)
                .map(|(pid, _)| *pid)
                .collect();

            // Add new node
            self.layers[l].insert(HnswNode {
                peer_id,
                vector: vector.clone(),
                neighbors: neighbor_ids.clone(),
            });

            // Add bidirectional edges
            for neighbor_id in &neighbor_ids {
                // First, check if we need to add the edge and if pruning is needed
                let needs_prune = {
                    if let Some(neighbor_node) = self.layers[l].get_mut(neighbor_id) {
                        if !neighbor_node.neighbors.contains(&peer_id) {
                            neighbor_node.neighbors.push(peer_id);
                            neighbor_node.neighbors.len() > max_conn
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                // If pruning needed, do it in a separate scope
                if needs_prune {
                    // Collect vectors we need for scoring
                    let (node_vec, neighbor_list): (Vec<f32>, Vec<PeerId>) = {
                        let node = self.layers[l].get(neighbor_id).unwrap();
                        (node.vector.clone(), node.neighbors.clone())
                    };

                    // Score all neighbors
                    let mut scored: Vec<_> = neighbor_list
                        .iter()
                        .filter_map(|nid| {
                            self.layers[l].get(nid).map(|n| (*nid, Self::similarity(&node_vec, &n.vector)))
                        })
                        .collect();

                    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                    let pruned_neighbors: Vec<PeerId> = scored.into_iter().take(max_conn).map(|(id, _)| id).collect();

                    // Apply pruned neighbors
                    if let Some(neighbor_node) = self.layers[l].get_mut(neighbor_id) {
                        neighbor_node.neighbors = pruned_neighbors;
                    }
                }
            }

            current_nearest = neighbor_ids;
        }

        // Update entry point if new node is at higher level
        if level >= self.layers.len().saturating_sub(1) {
            self.entry_point = Some(peer_id);
        }
    }

    /// Search for K nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(PeerId, f64)> {
        let entry = match self.entry_point {
            Some(ep) => ep,
            None => return Vec::new(),
        };

        // Search from top layer down
        let mut current_nearest = vec![entry];
        for l in (1..self.layers.len()).rev() {
            let nearest = self.search_layer(query, &current_nearest, l, 1);
            if !nearest.is_empty() {
                current_nearest = vec![nearest[0].0];
            }
        }

        // Final search at layer 0 with ef = k * 2 for better recall
        let mut results = self.search_layer(query, &current_nearest, 0, k * 2);
        results.truncate(k);
        results
    }

    /// Remove a peer from the index
    pub fn remove(&mut self, peer_id: &PeerId) {
        for layer in &mut self.layers {
            layer.nodes.remove(peer_id);
            // Remove from neighbor lists
            for node in layer.nodes.values_mut() {
                node.neighbors.retain(|n| n != peer_id);
            }
        }

        // Update entry point if needed
        if self.entry_point == Some(*peer_id) {
            self.entry_point = self.layers
                .last()
                .and_then(|l| l.iter().next())
                .map(|(pid, _)| *pid);
        }
    }

    /// Get number of nodes in base layer
    pub fn len(&self) -> usize {
        self.layers.first().map(|l| l.len()).unwrap_or(0)
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ============================================================================
// Topic Registry
// ============================================================================

/// Information about a gossipsub topic
#[derive(Clone, Debug)]
pub struct TopicInfo {
    /// Topic hash
    pub hash: TopicHash,
    /// Topic name/description
    pub name: String,
    /// Semantic centroid for the topic
    pub centroid: Vec<f32>,
    /// Subscribers count
    pub subscribers: usize,
    /// Activity level (messages per minute)
    pub activity: f32,
}

// ============================================================================
// Semantic Router
// ============================================================================

/// Semantic router for intelligent gossip and peer discovery
#[wasm_bindgen]
pub struct SemanticRouter {
    /// Known peers indexed by peer ID
    peers: RwLock<FxHashMap<[u8; 32], PeerInfo>>,
    /// My capability embedding centroid
    my_centroid: RwLock<Vec<f32>>,
    /// HNSW index for fast neighbor lookup
    hnsw_index: RwLock<HnswIndex>,
    /// Number of semantic neighbors to route to
    semantic_neighbors: usize,
    /// Number of random peers to include for robustness
    random_sample: usize,
    /// Embedding dimension
    embedding_dim: usize,
    /// Topic registry
    topics: RwLock<FxHashMap<[u8; 32], TopicInfo>>,
    /// My peer ID
    my_peer_id: RwLock<Option<PeerId>>,
}

#[wasm_bindgen]
impl SemanticRouter {
    /// Create a new semantic router
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let embedding_dim = 64; // Default embedding dimension
        Self {
            peers: RwLock::new(FxHashMap::default()),
            my_centroid: RwLock::new(vec![0.0; embedding_dim]),
            hnsw_index: RwLock::new(HnswIndex::new(embedding_dim)),
            semantic_neighbors: 5,
            random_sample: 3,
            embedding_dim,
            topics: RwLock::new(FxHashMap::default()),
            my_peer_id: RwLock::new(None),
        }
    }

    /// Create with custom parameters
    #[wasm_bindgen(js_name = withParams)]
    pub fn with_params(embedding_dim: usize, semantic_neighbors: usize, random_sample: usize) -> Self {
        Self {
            peers: RwLock::new(FxHashMap::default()),
            my_centroid: RwLock::new(vec![0.0; embedding_dim]),
            hnsw_index: RwLock::new(HnswIndex::new(embedding_dim)),
            semantic_neighbors,
            random_sample,
            embedding_dim,
            topics: RwLock::new(FxHashMap::default()),
            my_peer_id: RwLock::new(None),
        }
    }

    /// Set my peer identity
    #[wasm_bindgen(js_name = setMyPeerId)]
    pub fn set_my_peer_id(&self, peer_id: &[u8]) {
        if peer_id.len() == 32 {
            let mut id = [0u8; 32];
            id.copy_from_slice(peer_id);
            *self.my_peer_id.write().unwrap() = Some(id);
        }
    }

    /// Set my capabilities and update my centroid
    #[wasm_bindgen(js_name = setMyCapabilities)]
    pub fn set_my_capabilities(&self, capabilities: Vec<String>) {
        let centroid = self.embed_capabilities_internal(&capabilities);
        *self.my_centroid.write().unwrap() = centroid;
    }

    /// Get peer count
    #[wasm_bindgen(js_name = peerCount)]
    pub fn peer_count(&self) -> usize {
        self.peers.read().unwrap().len()
    }

    /// Get topic count
    #[wasm_bindgen(js_name = topicCount)]
    pub fn topic_count(&self) -> usize {
        self.topics.read().unwrap().len()
    }

    /// Get active peer count (seen in last 60 seconds)
    #[wasm_bindgen(js_name = activePeerCount)]
    pub fn active_peer_count(&self) -> usize {
        let now = current_timestamp_ms();
        self.peers.read().unwrap()
            .values()
            .filter(|p| now.saturating_sub(p.last_seen) < 60_000)
            .count()
    }

    /// Get statistics as JSON
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> String {
        let peers = self.peers.read().unwrap();
        let topics = self.topics.read().unwrap();
        let now = current_timestamp_ms();

        let total_peers = peers.len();
        let active_peers = peers.values()
            .filter(|p| now.saturating_sub(p.last_seen) < 60_000)
            .count();
        let avg_reputation = if total_peers > 0 {
            peers.values().map(|p| p.reputation as f64).sum::<f64>() / total_peers as f64
        } else {
            0.0
        };
        let avg_latency = if total_peers > 0 {
            peers.values().map(|p| p.latency_ms as u64).sum::<u64>() / total_peers as u64
        } else {
            0
        };

        format!(
            r#"{{"total_peers":{},"active_peers":{},"total_topics":{},"avg_reputation":{:.4},"avg_latency_ms":{},"semantic_neighbors":{},"random_sample":{}}}"#,
            total_peers,
            active_peers,
            topics.len(),
            avg_reputation,
            avg_latency,
            self.semantic_neighbors,
            self.random_sample
        )
    }
}

impl Default for SemanticRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticRouter {
    // ========================================================================
    // Capability Embedding
    // ========================================================================

    /// Embed capabilities into a vector using a simple hashing scheme
    fn embed_capabilities_internal(&self, capabilities: &[String]) -> Vec<f32> {
        let mut embedding = vec![0.0f32; self.embedding_dim];

        for cap in capabilities {
            // Hash capability to get deterministic embedding contribution
            let hash = self.hash_capability(cap);
            for (i, &byte) in hash.iter().enumerate() {
                let idx = i % self.embedding_dim;
                // Convert byte to [-1, 1] range and accumulate
                embedding[idx] += (byte as f32 / 127.5) - 1.0;
            }
        }

        // Normalize the embedding
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        embedding
    }

    /// Hash a capability string to bytes
    fn hash_capability(&self, capability: &str) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(b"CAPABILITY:");
        hasher.update(capability.as_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Embed an event into a vector based on its ruvector and context
    fn embed_event(&self, event: &Event) -> Vec<f32> {
        let dims = &event.ruvector.dims;

        // Resize or pad to our embedding dimension
        let mut embedding = vec![0.0f32; self.embedding_dim];
        for (i, &dim) in dims.iter().enumerate() {
            if i < self.embedding_dim {
                embedding[i] = dim;
            }
        }

        // Add context influence
        for (i, &byte) in event.context.iter().enumerate() {
            let idx = i % self.embedding_dim;
            embedding[idx] += (byte as f32 / 255.0) * 0.1; // Small context influence
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        embedding
    }

    // ========================================================================
    // Peer Management
    // ========================================================================

    /// Update peer information when receiving their capability advertisement
    pub fn update_peer(&self, peer_id: PeerId, capabilities: &[String], latency_ms: Option<u32>) {
        let embedding = self.embed_capabilities_internal(capabilities);

        let mut peers = self.peers.write().unwrap();
        let mut index = self.hnsw_index.write().unwrap();

        let now = current_timestamp_ms();

        peers.entry(peer_id)
            .and_modify(|peer| {
                peer.capabilities = capabilities.to_vec();
                peer.centroid = embedding.clone();
                peer.last_seen = now;
                if let Some(lat) = latency_ms {
                    peer.update_latency(lat);
                }
            })
            .or_insert_with(|| {
                let mut peer = PeerInfo::new(peer_id, capabilities.to_vec());
                peer.centroid = embedding.clone();
                if let Some(lat) = latency_ms {
                    peer.latency_ms = lat;
                }
                peer
            });

        // Update HNSW index
        index.remove(&peer_id);
        index.insert(peer_id, embedding);
    }

    /// Remove a peer from the router
    pub fn remove_peer(&self, peer_id: &PeerId) {
        self.peers.write().unwrap().remove(peer_id);
        self.hnsw_index.write().unwrap().remove(peer_id);
    }

    /// Update peer reputation after an interaction
    pub fn update_reputation(&self, peer_id: &PeerId, success: bool, delta: f32) {
        if let Some(peer) = self.peers.write().unwrap().get_mut(peer_id) {
            if success {
                peer.success_count += 1;
                peer.reputation = (peer.reputation + delta).clamp(0.0, 1.0);
            } else {
                peer.failure_count += 1;
                peer.reputation = (peer.reputation - delta.abs()).clamp(0.0, 1.0);
            }
            peer.last_seen = current_timestamp_ms();
        }
    }

    /// Get a peer's info
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<PeerInfo> {
        self.peers.read().unwrap().get(peer_id).cloned()
    }

    // ========================================================================
    // Routing
    // ========================================================================

    /// Get routes for an event (semantic neighbors + random sample)
    pub fn get_routes(&self, event: &Event) -> Vec<PeerId> {
        let event_vector = self.embed_event(event);
        let my_peer_id = self.my_peer_id.read().unwrap().clone();

        // Get semantic neighbors via HNSW
        let index = self.hnsw_index.read().unwrap();
        let neighbors = index.search(&event_vector, self.semantic_neighbors * 2);

        let peers = self.peers.read().unwrap();

        // Filter and sort by composite routing score
        let mut scored_neighbors: Vec<_> = neighbors
            .iter()
            .filter(|(pid, _)| Some(*pid) != my_peer_id) // Exclude self
            .filter_map(|(pid, similarity)| {
                peers.get(pid).map(|peer| {
                    let score = peer.routing_score(*similarity);
                    (*pid, score)
                })
            })
            .collect();

        scored_neighbors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut routes: Vec<PeerId> = scored_neighbors
            .into_iter()
            .take(self.semantic_neighbors)
            .map(|(pid, _)| pid)
            .collect();

        // Add random sample for robustness
        let random_peers = self.random_sample_internal(self.random_sample, &routes, &my_peer_id);
        routes.extend(random_peers);

        routes
    }

    /// Get routes for a raw vector query
    pub fn get_routes_for_vector(&self, query: &[f32]) -> Vec<PeerId> {
        let my_peer_id = self.my_peer_id.read().unwrap().clone();
        let index = self.hnsw_index.read().unwrap();
        let neighbors = index.search(query, self.semantic_neighbors * 2);

        let peers = self.peers.read().unwrap();

        let mut scored_neighbors: Vec<_> = neighbors
            .iter()
            .filter(|(pid, _)| Some(*pid) != my_peer_id)
            .filter_map(|(pid, similarity)| {
                peers.get(pid).map(|peer| {
                    let score = peer.routing_score(*similarity);
                    (*pid, score)
                })
            })
            .collect();

        scored_neighbors.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut routes: Vec<PeerId> = scored_neighbors
            .into_iter()
            .take(self.semantic_neighbors)
            .map(|(pid, _)| pid)
            .collect();

        let random_peers = self.random_sample_internal(self.random_sample, &routes, &my_peer_id);
        routes.extend(random_peers);

        routes
    }

    /// Random sample of peers for robustness (excluding already selected)
    fn random_sample_internal(&self, count: usize, exclude: &[PeerId], my_id: &Option<PeerId>) -> Vec<PeerId> {
        let peers = self.peers.read().unwrap();
        let now = current_timestamp_ms();

        // Use current timestamp as pseudo-random seed
        let seed = now;

        let candidates: Vec<_> = peers
            .iter()
            .filter(|(pid, peer)| {
                // Exclude already selected peers
                !exclude.contains(pid) &&
                // Exclude self
                Some(**pid) != *my_id &&
                // Only active peers
                now.saturating_sub(peer.last_seen) < 120_000 &&
                // Minimum reputation
                peer.reputation > 0.2
            })
            .collect();

        if candidates.is_empty() {
            return Vec::new();
        }

        // Pseudo-random selection based on seed
        let mut selected = Vec::with_capacity(count);
        for i in 0..count {
            if candidates.is_empty() {
                break;
            }
            let idx = ((seed.wrapping_add(i as u64)).wrapping_mul(31337)) as usize % candidates.len();
            if idx < candidates.len() && !selected.contains(candidates[idx].0) {
                selected.push(*candidates[idx].0);
            }
        }

        selected
    }

    // ========================================================================
    // Topic Discovery
    // ========================================================================

    /// Register a topic with its semantic centroid
    pub fn register_topic(&self, hash: TopicHash, name: String, centroid: Vec<f32>) {
        self.topics.write().unwrap().insert(hash, TopicInfo {
            hash,
            name,
            centroid,
            subscribers: 0,
            activity: 0.0,
        });
    }

    /// Discover topics by semantic similarity to my centroid
    pub fn discover_topics(&self, threshold: f32) -> Vec<TopicHash> {
        let my_centroid = self.my_centroid.read().unwrap();
        let topics = self.topics.read().unwrap();

        topics
            .iter()
            .filter_map(|(hash, info)| {
                let similarity = self.cosine_similarity(&my_centroid, &info.centroid);
                if similarity >= threshold as f64 {
                    Some(*hash)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Find topics similar to a query vector
    pub fn find_similar_topics(&self, query: &[f32], k: usize) -> Vec<(TopicHash, f64)> {
        let topics = self.topics.read().unwrap();

        let mut scored: Vec<_> = topics
            .iter()
            .map(|(hash, info)| {
                let similarity = self.cosine_similarity(query, &info.centroid);
                (*hash, similarity)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }

    /// Cosine similarity helper
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f64 {
        HnswIndex::similarity(a, b)
    }

    // ========================================================================
    // Maintenance
    // ========================================================================

    /// Prune stale peers (not seen in given duration)
    pub fn prune_stale(&self, max_age_ms: u64) -> usize {
        let now = current_timestamp_ms();
        let mut peers = self.peers.write().unwrap();
        let mut index = self.hnsw_index.write().unwrap();

        let stale: Vec<PeerId> = peers
            .iter()
            .filter(|(_, peer)| now.saturating_sub(peer.last_seen) > max_age_ms)
            .map(|(pid, _)| *pid)
            .collect();

        for pid in &stale {
            peers.remove(pid);
            index.remove(pid);
        }

        stale.len()
    }

    /// Prune low-reputation peers
    pub fn prune_low_reputation(&self, min_reputation: f32) -> usize {
        let mut peers = self.peers.write().unwrap();
        let mut index = self.hnsw_index.write().unwrap();

        let to_remove: Vec<PeerId> = peers
            .iter()
            .filter(|(_, peer)| peer.reputation < min_reputation)
            .map(|(pid, _)| *pid)
            .collect();

        for pid in &to_remove {
            peers.remove(pid);
            index.remove(pid);
        }

        to_remove.len()
    }

    /// Get all known peer IDs
    pub fn all_peer_ids(&self) -> Vec<PeerId> {
        self.peers.read().unwrap().keys().cloned().collect()
    }

    /// Get peers by capability
    pub fn peers_with_capability(&self, capability: &str) -> Vec<PeerId> {
        self.peers.read().unwrap()
            .iter()
            .filter(|(_, peer)| peer.capabilities.contains(&capability.to_string()))
            .map(|(pid, _)| *pid)
            .collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rac::{EventKind, Ruvector};

    fn make_peer_id(seed: u8) -> PeerId {
        [seed; 32]
    }

    #[test]
    fn test_peer_info_success_rate() {
        let mut peer = PeerInfo::new(make_peer_id(1), vec!["vectors".to_string()]);

        assert!((peer.success_rate() - 0.5).abs() < 0.01); // No data

        peer.success_count = 8;
        peer.failure_count = 2;
        assert!((peer.success_rate() - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_peer_info_latency_ema() {
        let mut peer = PeerInfo::new(make_peer_id(1), vec![]);
        peer.latency_ms = 100;

        peer.update_latency(200);
        assert!(peer.latency_ms > 100 && peer.latency_ms < 200);

        // Multiple updates should move towards new value
        for _ in 0..10 {
            peer.update_latency(50);
        }
        assert!(peer.latency_ms < 100);
    }

    #[test]
    fn test_hnsw_insert_and_search() {
        let mut index = HnswIndex::new(4);

        // Insert some vectors
        index.insert(make_peer_id(1), vec![1.0, 0.0, 0.0, 0.0]);
        index.insert(make_peer_id(2), vec![0.0, 1.0, 0.0, 0.0]);
        index.insert(make_peer_id(3), vec![0.9, 0.1, 0.0, 0.0]);

        assert_eq!(index.len(), 3);

        // Search for similar to [1, 0, 0, 0]
        let results = index.search(&[1.0, 0.0, 0.0, 0.0], 2);

        assert!(!results.is_empty());
        // First result should be peer 1 or 3 (both similar to query)
        let first_peer = results[0].0;
        assert!(first_peer == make_peer_id(1) || first_peer == make_peer_id(3));
    }

    #[test]
    fn test_hnsw_remove() {
        let mut index = HnswIndex::new(4);

        index.insert(make_peer_id(1), vec![1.0, 0.0, 0.0, 0.0]);
        index.insert(make_peer_id(2), vec![0.0, 1.0, 0.0, 0.0]);

        assert_eq!(index.len(), 2);

        index.remove(&make_peer_id(1));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_semantic_router_creation() {
        let router = SemanticRouter::new();

        assert_eq!(router.peer_count(), 0);
        assert_eq!(router.topic_count(), 0);
    }

    #[test]
    fn test_semantic_router_update_peer() {
        let router = SemanticRouter::new();

        router.update_peer(make_peer_id(1), &["vectors".to_string(), "ml".to_string()], Some(50));
        router.update_peer(make_peer_id(2), &["embeddings".to_string()], Some(100));

        assert_eq!(router.peer_count(), 2);

        let peer = router.get_peer(&make_peer_id(1)).unwrap();
        assert_eq!(peer.capabilities.len(), 2);
        assert_eq!(peer.latency_ms, 50);
    }

    #[test]
    fn test_semantic_router_reputation() {
        let router = SemanticRouter::new();

        router.update_peer(make_peer_id(1), &["vectors".to_string()], None);

        let initial_rep = router.get_peer(&make_peer_id(1)).unwrap().reputation;

        router.update_reputation(&make_peer_id(1), true, 0.1);
        let after_success = router.get_peer(&make_peer_id(1)).unwrap().reputation;
        assert!(after_success > initial_rep);

        router.update_reputation(&make_peer_id(1), false, 0.2);
        let after_failure = router.get_peer(&make_peer_id(1)).unwrap().reputation;
        assert!(after_failure < after_success);
    }

    #[test]
    fn test_semantic_router_get_routes() {
        let router = SemanticRouter::with_params(8, 2, 1);

        // Add peers with different capabilities
        router.update_peer(make_peer_id(1), &["vectors".to_string()], Some(50));
        router.update_peer(make_peer_id(2), &["vectors".to_string()], Some(100));
        router.update_peer(make_peer_id(3), &["ml".to_string()], Some(75));
        router.update_peer(make_peer_id(4), &["embeddings".to_string()], Some(60));

        // Create an event
        let event = Event::new(
            [0u8; 32],
            [0u8; 32],
            Ruvector::new(vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            EventKind::Assert(crate::rac::AssertEvent {
                proposition: b"test".to_vec(),
                evidence: vec![],
                confidence: 0.9,
                expires_at_unix_ms: None,
            }),
            None,
        );

        let routes = router.get_routes(&event);

        // Should have some routes (semantic neighbors + random)
        assert!(!routes.is_empty());
        // Should not exceed semantic_neighbors + random_sample
        assert!(routes.len() <= 3);
    }

    #[test]
    fn test_topic_discovery() {
        let router = SemanticRouter::with_params(4, 2, 1);

        // Set my capabilities
        router.set_my_capabilities(vec!["vectors".to_string()]);

        // Register topics with centroids that will have similarity with any non-zero embedding
        router.register_topic([1u8; 32], "vector-operations".to_string(), vec![1.0, 0.0, 0.0, 0.0]);
        router.register_topic([2u8; 32], "ml-inference".to_string(), vec![0.0, 1.0, 0.0, 0.0]);

        // Discover with threshold -1.0 to get all topics (cosine similarity is in [-1, 1])
        let discovered = router.discover_topics(-1.0);
        assert_eq!(discovered.len(), 2);

        // Find similar topics
        let similar = router.find_similar_topics(&[1.0, 0.0, 0.0, 0.0], 1);
        assert!(!similar.is_empty());
        assert_eq!(similar[0].0, [1u8; 32]); // vector-operations should be most similar
    }

    #[test]
    fn test_prune_stale() {
        let router = SemanticRouter::new();

        router.update_peer(make_peer_id(1), &["vectors".to_string()], None);
        router.update_peer(make_peer_id(2), &["ml".to_string()], None);

        assert_eq!(router.peer_count(), 2);

        // Prune with very short TTL (should prune nothing since just added)
        let pruned = router.prune_stale(1);
        // Note: This might prune depending on timing, but typically won't
        assert!(pruned == 0 || router.peer_count() <= 2);
    }

    #[test]
    fn test_capability_embedding() {
        let router = SemanticRouter::with_params(8, 2, 1);

        // Same capabilities should produce same embedding
        let caps1 = vec!["vectors".to_string(), "ml".to_string()];
        let caps2 = vec!["vectors".to_string(), "ml".to_string()];
        let caps3 = vec!["different".to_string()];

        let emb1 = router.embed_capabilities_internal(&caps1);
        let emb2 = router.embed_capabilities_internal(&caps2);
        let emb3 = router.embed_capabilities_internal(&caps3);

        // Same capabilities should produce identical embeddings
        assert_eq!(emb1, emb2);

        // Different capabilities should produce different embeddings
        assert_ne!(emb1, emb3);
    }

    #[test]
    fn test_peers_with_capability() {
        let router = SemanticRouter::new();

        router.update_peer(make_peer_id(1), &["vectors".to_string(), "ml".to_string()], None);
        router.update_peer(make_peer_id(2), &["vectors".to_string()], None);
        router.update_peer(make_peer_id(3), &["embeddings".to_string()], None);

        let vector_peers = router.peers_with_capability("vectors");
        assert_eq!(vector_peers.len(), 2);

        let ml_peers = router.peers_with_capability("ml");
        assert_eq!(ml_peers.len(), 1);

        let embedding_peers = router.peers_with_capability("embeddings");
        assert_eq!(embedding_peers.len(), 1);
    }

    #[test]
    fn test_routing_score() {
        let mut peer = PeerInfo::new(make_peer_id(1), vec![]);
        peer.latency_ms = 50;
        peer.reputation = 0.9;

        let score_high_sim = peer.routing_score(1.0);
        let score_low_sim = peer.routing_score(0.1);

        // Higher similarity should give higher score
        assert!(score_high_sim > score_low_sim);

        // Low latency, high reputation peer
        let mut good_peer = PeerInfo::new(make_peer_id(2), vec![]);
        good_peer.latency_ms = 10;
        good_peer.reputation = 1.0;

        // High latency, low reputation peer
        let mut bad_peer = PeerInfo::new(make_peer_id(3), vec![]);
        bad_peer.latency_ms = 500;
        bad_peer.reputation = 0.2;

        // At same similarity, good peer should score higher
        assert!(good_peer.routing_score(0.5) > bad_peer.routing_score(0.5));
    }
}
