//! MemoryCoherenceLayer: Track memory entries as sheaf nodes
//!
//! This module implements ADR-CE-019 (Memory as Nodes), providing coherence
//! tracking across RuvLLM's three memory types:
//!
//! - `AgenticMemory`: Long-term patterns and learned behaviors
//! - `WorkingMemory`: Current context and active state
//! - `EpisodicMemory`: Conversation history and past interactions
//!
//! # Architecture
//!
//! ```text
//! +-------------------+     +-------------------+     +-------------------+
//! |   AgenticMemory   |     |   WorkingMemory   |     |  EpisodicMemory   |
//! |   (Long-term)     |     |   (Current)       |     |   (History)       |
//! +--------+----------+     +--------+----------+     +--------+----------+
//!          |                         |                         |
//!          v                         v                         v
//! +------------------------------------------------------------------------+
//! |                        MemoryCoherenceLayer                             |
//! |                                                                         |
//! |  +-------------+    +-------------+    +-------------+                  |
//! |  | Sheaf Node  |----| Sheaf Node  |----| Sheaf Node  |  ...            |
//! |  | (memory_1)  |    | (memory_2)  |    | (memory_3)  |                  |
//! |  +-------------+    +-------------+    +-------------+                  |
//! |                                                                         |
//! |  Edge Types:                                                            |
//! |  - Temporal: Episode N consistent with N-1                              |
//! |  - Semantic: Related facts should agree                                 |
//! |  - Hierarchical: Specific facts consistent with patterns                |
//! +------------------------------------------------------------------------+
//!                                |
//!                                v
//!                    +----------------------+
//!                    |   CoherenceEnergy    |
//!                    | (Contradiction Check)|
//!                    +----------------------+
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::ruvllm_integration::{
//!     MemoryCoherenceLayer, MemoryType, MemoryEntry,
//!     AgenticMemory, WorkingMemory, EpisodicMemory,
//! };
//!
//! let mut layer = MemoryCoherenceLayer::new();
//!
//! // Add a memory entry
//! let entry = MemoryEntry::new(
//!     "user_prefers_dark_mode",
//!     vec![0.8, 0.1, 0.0, 0.5],  // embedding
//!     MemoryType::Agentic,
//! );
//!
//! let result = layer.add_with_coherence(entry)?;
//! if !result.is_coherent {
//!     println!("Warning: Memory contradicts existing knowledge!");
//!     println!("Conflicting memories: {:?}", result.conflicting_memories);
//! }
//! ```
//!
//! # References
//!
//! - ADR-CE-019: Memory as Nodes

use crate::substrate::edge::{EdgeId, SheafEdge, SheafEdgeBuilder};
use crate::substrate::graph::SheafGraph;
use crate::substrate::node::{NodeId, NodeMetadata, SheafNode, SheafNodeBuilder, StateVector};
use crate::substrate::restriction::RestrictionMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Errors that can occur in the memory coherence layer
#[derive(Debug, Error)]
pub enum MemoryCoherenceError {
    /// Memory entry not found
    #[error("Memory entry not found: {0}")]
    MemoryNotFound(MemoryId),

    /// Invalid memory embedding dimension
    #[error("Invalid embedding dimension: expected {expected}, got {actual}")]
    InvalidDimension {
        /// Expected dimension
        expected: usize,
        /// Actual dimension
        actual: usize,
    },

    /// Failed to add edge to graph
    #[error("Failed to add edge: {0}")]
    EdgeCreationFailed(String),

    /// Memory graph is in an inconsistent state
    #[error("Memory graph inconsistent: {0}")]
    GraphInconsistent(String),

    /// Coherence computation failed
    #[error("Coherence computation failed: {0}")]
    CoherenceFailed(String),
}

/// Result type for memory coherence operations
pub type Result<T> = std::result::Result<T, MemoryCoherenceError>;

// ============================================================================
// MEMORY TYPES
// ============================================================================

/// Unique identifier for a memory entry
pub type MemoryId = Uuid;

/// Types of memory in the RuvLLM system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryType {
    /// Long-term patterns and learned behaviors
    ///
    /// These memories persist across sessions and represent stable knowledge.
    /// Example: "User prefers concise responses"
    Agentic,

    /// Current context and active state
    ///
    /// These memories are transient and represent the current working set.
    /// Example: "Currently discussing Rust programming"
    Working,

    /// Conversation history and past interactions
    ///
    /// These memories capture the temporal sequence of interactions.
    /// Example: "User asked about error handling 3 turns ago"
    Episodic,
}

impl MemoryType {
    /// Get a human-readable name for the memory type
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryType::Agentic => "agentic",
            MemoryType::Working => "working",
            MemoryType::Episodic => "episodic",
        }
    }

    /// Get the namespace for this memory type in the sheaf graph
    pub fn namespace(&self) -> &'static str {
        match self {
            MemoryType::Agentic => "memory:agentic",
            MemoryType::Working => "memory:working",
            MemoryType::Episodic => "memory:episodic",
        }
    }
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Edge types connecting memory nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryEdgeType {
    /// Temporal edge: Episode N should be consistent with N-1
    ///
    /// Used for episodic memory to ensure temporal coherence.
    Temporal,

    /// Semantic edge: Related facts should agree
    ///
    /// Used when two memories discuss the same topic.
    Semantic,

    /// Hierarchical edge: Specific facts consistent with general patterns
    ///
    /// Used to connect working/episodic memories to agentic patterns.
    Hierarchical,
}

impl MemoryEdgeType {
    /// Get a human-readable name for the edge type
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryEdgeType::Temporal => "temporal",
            MemoryEdgeType::Semantic => "semantic",
            MemoryEdgeType::Hierarchical => "hierarchical",
        }
    }

    /// Get the default weight for this edge type
    ///
    /// Higher weights make contradictions more costly.
    pub fn default_weight(&self) -> f32 {
        match self {
            // Temporal consistency is critical
            MemoryEdgeType::Temporal => 1.5,
            // Semantic consistency is important
            MemoryEdgeType::Semantic => 1.0,
            // Hierarchical allows some variation
            MemoryEdgeType::Hierarchical => 0.8,
        }
    }
}

impl std::fmt::Display for MemoryEdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// MEMORY ENTRY
// ============================================================================

/// A memory entry to be tracked for coherence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique identifier
    pub id: MemoryId,
    /// Human-readable key or description
    pub key: String,
    /// Embedding vector representing the memory content
    pub embedding: Vec<f32>,
    /// Type of memory
    pub memory_type: MemoryType,
    /// Optional sequence number for episodic memories
    pub sequence: Option<u64>,
    /// Timestamp when the memory was created
    pub created_at: DateTime<Utc>,
    /// Optional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl MemoryEntry {
    /// Create a new memory entry
    pub fn new(key: impl Into<String>, embedding: Vec<f32>, memory_type: MemoryType) -> Self {
        Self {
            id: Uuid::new_v4(),
            key: key.into(),
            embedding,
            memory_type,
            sequence: None,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new episodic memory with a sequence number
    pub fn episodic(key: impl Into<String>, embedding: Vec<f32>, sequence: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            key: key.into(),
            embedding,
            memory_type: MemoryType::Episodic,
            sequence: Some(sequence),
            created_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the entry
    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get the dimension of the embedding
    pub fn dim(&self) -> usize {
        self.embedding.len()
    }

    /// Convert to a sheaf node
    pub fn to_sheaf_node(&self) -> SheafNode {
        SheafNodeBuilder::new()
            .id(self.id)
            .state(StateVector::new(self.embedding.clone()))
            .label(&self.key)
            .node_type(self.memory_type.as_str())
            .namespace(self.memory_type.namespace())
            .build()
    }
}

// ============================================================================
// COHERENCE RESULT
// ============================================================================

/// Result of adding a memory with coherence checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceResult {
    /// The memory ID that was added
    pub memory_id: MemoryId,
    /// The node ID in the sheaf graph
    pub node_id: NodeId,
    /// Whether the memory is coherent with existing memories
    pub is_coherent: bool,
    /// Total coherence energy (lower = more coherent)
    pub energy: f32,
    /// Local energy for this memory's neighborhood
    pub local_energy: f32,
    /// IDs of memories that conflict with this one
    pub conflicting_memories: Vec<MemoryId>,
    /// Edges that were created
    pub edges_created: Vec<EdgeId>,
    /// Timestamp of the check
    pub checked_at: DateTime<Utc>,
}

impl CoherenceResult {
    /// Create a coherent result
    pub fn coherent(memory_id: MemoryId, node_id: NodeId, energy: f32, edges: Vec<EdgeId>) -> Self {
        Self {
            memory_id,
            node_id,
            is_coherent: true,
            energy,
            local_energy: 0.0,
            conflicting_memories: Vec::new(),
            edges_created: edges,
            checked_at: Utc::now(),
        }
    }

    /// Create an incoherent result
    pub fn incoherent(
        memory_id: MemoryId,
        node_id: NodeId,
        energy: f32,
        local_energy: f32,
        conflicts: Vec<MemoryId>,
        edges: Vec<EdgeId>,
    ) -> Self {
        Self {
            memory_id,
            node_id,
            is_coherent: false,
            energy,
            local_energy,
            conflicting_memories: conflicts,
            edges_created: edges,
            checked_at: Utc::now(),
        }
    }
}

// ============================================================================
// MEMORY TRAITS
// ============================================================================

/// Trait for accessing agentic (long-term) memory
///
/// Agentic memories represent stable, learned patterns that persist
/// across sessions. They capture user preferences, domain knowledge,
/// and behavioral patterns.
pub trait AgenticMemory {
    /// Store a pattern in agentic memory
    fn store_pattern(&mut self, key: &str, embedding: &[f32]) -> Result<MemoryId>;

    /// Retrieve a pattern by key
    fn get_pattern(&self, key: &str) -> Option<&[f32]>;

    /// List all pattern keys
    fn pattern_keys(&self) -> Vec<String>;

    /// Remove a pattern
    fn remove_pattern(&mut self, key: &str) -> bool;

    /// Check if a pattern exists
    fn has_pattern(&self, key: &str) -> bool {
        self.get_pattern(key).is_some()
    }
}

/// Trait for accessing working (current context) memory
///
/// Working memories represent the active context of the current
/// interaction. They are transient and may be cleared between sessions.
pub trait WorkingMemory {
    /// Set a context value
    fn set_context(&mut self, key: &str, embedding: &[f32]) -> Result<MemoryId>;

    /// Get a context value
    fn get_context(&self, key: &str) -> Option<&[f32]>;

    /// Clear all working memory
    fn clear(&mut self);

    /// List all context keys
    fn context_keys(&self) -> Vec<String>;

    /// Get the current context size (number of entries)
    fn size(&self) -> usize;
}

/// Trait for accessing episodic (conversation history) memory
///
/// Episodic memories capture the temporal sequence of interactions.
/// They are ordered and support retrieval by sequence number or
/// time range.
pub trait EpisodicMemory {
    /// Add an episode (returns the sequence number)
    fn add_episode(&mut self, key: &str, embedding: &[f32]) -> Result<(MemoryId, u64)>;

    /// Get an episode by sequence number
    fn get_episode(&self, sequence: u64) -> Option<&[f32]>;

    /// Get the most recent N episodes
    fn recent_episodes(&self, n: usize) -> Vec<(u64, &[f32])>;

    /// Get episodes in a sequence range
    fn episodes_in_range(&self, start: u64, end: u64) -> Vec<(u64, &[f32])>;

    /// Get the current sequence number (next episode will be this + 1)
    fn current_sequence(&self) -> u64;

    /// Trim episodes older than a certain sequence number
    fn trim_before(&mut self, sequence: u64);
}

// ============================================================================
// MEMORY COHERENCE LAYER
// ============================================================================

/// Configuration for the memory coherence layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCoherenceConfig {
    /// Expected embedding dimension
    pub embedding_dim: usize,
    /// Energy threshold for coherence (below = coherent)
    pub coherence_threshold: f32,
    /// Whether to automatically create semantic edges
    pub auto_semantic_edges: bool,
    /// Semantic similarity threshold for creating edges (cosine similarity)
    pub semantic_similarity_threshold: f32,
    /// Whether to automatically create hierarchical edges
    pub auto_hierarchical_edges: bool,
    /// Maximum number of semantic edges per memory
    pub max_semantic_edges: usize,
}

impl Default for MemoryCoherenceConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 64,
            coherence_threshold: 0.5,
            auto_semantic_edges: true,
            semantic_similarity_threshold: 0.7,
            auto_hierarchical_edges: true,
            max_semantic_edges: 5,
        }
    }
}

/// The main memory coherence layer
///
/// This struct integrates RuvLLM's three memory types with the sheaf graph
/// coherence system to detect contradictions between memories.
pub struct MemoryCoherenceLayer {
    /// The underlying sheaf graph
    graph: SheafGraph,
    /// Configuration
    config: MemoryCoherenceConfig,
    /// Mapping from memory ID to node ID
    memory_to_node: HashMap<MemoryId, NodeId>,
    /// Mapping from node ID to memory ID
    node_to_memory: HashMap<NodeId, MemoryId>,
    /// Agentic memory storage
    agentic_memories: HashMap<String, (MemoryId, Vec<f32>)>,
    /// Working memory storage
    working_memories: HashMap<String, (MemoryId, Vec<f32>)>,
    /// Episodic memory storage (sequence -> (id, key, embedding))
    episodic_memories: Vec<(MemoryId, String, Vec<f32>)>,
    /// Current episodic sequence counter
    episodic_sequence: u64,
}

impl MemoryCoherenceLayer {
    /// Create a new memory coherence layer with default configuration
    pub fn new() -> Self {
        Self::with_config(MemoryCoherenceConfig::default())
    }

    /// Create a new memory coherence layer with custom configuration
    pub fn with_config(config: MemoryCoherenceConfig) -> Self {
        Self {
            graph: SheafGraph::with_namespace("memory"),
            config,
            memory_to_node: HashMap::new(),
            node_to_memory: HashMap::new(),
            agentic_memories: HashMap::new(),
            working_memories: HashMap::new(),
            episodic_memories: Vec::new(),
            episodic_sequence: 0,
        }
    }

    /// Get the underlying sheaf graph (for advanced operations)
    pub fn graph(&self) -> &SheafGraph {
        &self.graph
    }

    /// Get the configuration
    pub fn config(&self) -> &MemoryCoherenceConfig {
        &self.config
    }

    /// Get the number of memory entries
    pub fn memory_count(&self) -> usize {
        self.memory_to_node.len()
    }

    /// Check if a memory exists
    pub fn has_memory(&self, id: MemoryId) -> bool {
        self.memory_to_node.contains_key(&id)
    }

    /// Get the node ID for a memory
    pub fn node_for_memory(&self, id: MemoryId) -> Option<NodeId> {
        self.memory_to_node.get(&id).copied()
    }

    /// Get the memory ID for a node
    pub fn memory_for_node(&self, id: NodeId) -> Option<MemoryId> {
        self.node_to_memory.get(&id).copied()
    }

    /// Add a memory entry with coherence checking
    ///
    /// This is the main entry point for adding memories. It:
    /// 1. Creates a sheaf node for the memory entry
    /// 2. Adds edges to related memories based on type and similarity
    /// 3. Computes coherence energy
    /// 4. Returns a result indicating whether the memory is coherent
    pub fn add_with_coherence(&mut self, entry: MemoryEntry) -> Result<CoherenceResult> {
        // Validate embedding dimension
        if entry.dim() != self.config.embedding_dim {
            return Err(MemoryCoherenceError::InvalidDimension {
                expected: self.config.embedding_dim,
                actual: entry.dim(),
            });
        }

        let memory_id = entry.id;
        let memory_type = entry.memory_type;

        // Create sheaf node
        let node = entry.to_sheaf_node();
        let node_id = self.graph.add_node(node);

        // Track the mapping
        self.memory_to_node.insert(memory_id, node_id);
        self.node_to_memory.insert(node_id, memory_id);

        // Store in appropriate memory storage
        match memory_type {
            MemoryType::Agentic => {
                self.agentic_memories
                    .insert(entry.key.clone(), (memory_id, entry.embedding.clone()));
            }
            MemoryType::Working => {
                self.working_memories
                    .insert(entry.key.clone(), (memory_id, entry.embedding.clone()));
            }
            MemoryType::Episodic => {
                self.episodic_memories.push((
                    memory_id,
                    entry.key.clone(),
                    entry.embedding.clone(),
                ));
                self.episodic_sequence += 1;
            }
        }

        // Create edges to related memories
        let edges = self.create_edges_for_memory(&entry)?;

        // Compute coherence energy
        let total_energy = self.graph.compute_energy();
        let local_energy = self.graph.compute_local_energy(node_id);

        // Check if coherent
        let is_coherent = local_energy <= self.config.coherence_threshold;

        if is_coherent {
            Ok(CoherenceResult::coherent(
                memory_id,
                node_id,
                total_energy.total_energy,
                edges,
            ))
        } else {
            // Find conflicting memories
            let conflicts = self.find_conflicting_memories(node_id);
            Ok(CoherenceResult::incoherent(
                memory_id,
                node_id,
                total_energy.total_energy,
                local_energy,
                conflicts,
                edges,
            ))
        }
    }

    /// Remove a memory entry
    pub fn remove_memory(&mut self, id: MemoryId) -> Result<()> {
        let node_id = self
            .memory_to_node
            .remove(&id)
            .ok_or(MemoryCoherenceError::MemoryNotFound(id))?;

        self.node_to_memory.remove(&node_id);
        self.graph.remove_node(node_id);

        // Remove from storage
        self.agentic_memories.retain(|_, (mid, _)| *mid != id);
        self.working_memories.retain(|_, (mid, _)| *mid != id);
        self.episodic_memories.retain(|(mid, _, _)| *mid != id);

        Ok(())
    }

    /// Compute the overall coherence energy
    pub fn compute_energy(&self) -> f32 {
        self.graph.compute_energy().total_energy
    }

    /// Check if the memory system is coherent
    pub fn is_coherent(&self) -> bool {
        self.compute_energy() <= self.config.coherence_threshold * self.memory_count() as f32
    }

    /// Find memories that conflict with the overall system
    pub fn find_incoherent_memories(&self) -> Vec<(MemoryId, f32)> {
        let mut results = Vec::new();
        let threshold = self.config.coherence_threshold;

        for (&memory_id, &node_id) in &self.memory_to_node {
            let local_energy = self.graph.compute_local_energy(node_id);
            if local_energy > threshold {
                results.push((memory_id, local_energy));
            }
        }

        // Sort by energy (highest first)
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }

    // ========================================================================
    // Private Helper Methods
    // ========================================================================

    /// Create edges for a newly added memory
    fn create_edges_for_memory(&mut self, entry: &MemoryEntry) -> Result<Vec<EdgeId>> {
        let mut edges = Vec::new();
        let node_id = self.memory_to_node[&entry.id];
        let dim = self.config.embedding_dim;

        match entry.memory_type {
            MemoryType::Episodic => {
                // Create temporal edge to previous episode
                if let Some(prev_seq) = entry.sequence.map(|s| s.saturating_sub(1)) {
                    if prev_seq < self.episodic_memories.len() as u64 && prev_seq > 0 {
                        let prev_idx = prev_seq as usize - 1;
                        if prev_idx < self.episodic_memories.len() {
                            let prev_id = self.episodic_memories[prev_idx].0;
                            if let Some(&prev_node) = self.memory_to_node.get(&prev_id) {
                                if let Some(edge_id) = self.create_edge(
                                    prev_node,
                                    node_id,
                                    MemoryEdgeType::Temporal,
                                    dim,
                                )? {
                                    edges.push(edge_id);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        // Create semantic edges if enabled
        if self.config.auto_semantic_edges {
            let semantic_edges = self.create_semantic_edges(entry, node_id)?;
            edges.extend(semantic_edges);
        }

        // Create hierarchical edges if enabled
        if self.config.auto_hierarchical_edges && entry.memory_type != MemoryType::Agentic {
            let hierarchical_edges = self.create_hierarchical_edges(entry, node_id)?;
            edges.extend(hierarchical_edges);
        }

        Ok(edges)
    }

    /// Create semantic edges based on embedding similarity
    fn create_semantic_edges(
        &mut self,
        entry: &MemoryEntry,
        node_id: NodeId,
    ) -> Result<Vec<EdgeId>> {
        let mut edges = Vec::new();
        let dim = self.config.embedding_dim;
        let threshold = self.config.semantic_similarity_threshold;
        let max_edges = self.config.max_semantic_edges;

        // Find similar memories (by cosine similarity)
        let mut candidates: Vec<(MemoryId, f32)> = Vec::new();

        // Check agentic memories
        for (_, (mid, emb)) in &self.agentic_memories {
            if *mid != entry.id {
                let sim = cosine_similarity(&entry.embedding, emb);
                if sim >= threshold {
                    candidates.push((*mid, sim));
                }
            }
        }

        // Check working memories
        for (_, (mid, emb)) in &self.working_memories {
            if *mid != entry.id {
                let sim = cosine_similarity(&entry.embedding, emb);
                if sim >= threshold {
                    candidates.push((*mid, sim));
                }
            }
        }

        // Sort by similarity and take top N
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(max_edges);

        // Create edges
        for (other_id, _) in candidates {
            if let Some(&other_node) = self.memory_to_node.get(&other_id) {
                if let Some(edge_id) =
                    self.create_edge(other_node, node_id, MemoryEdgeType::Semantic, dim)?
                {
                    edges.push(edge_id);
                }
            }
        }

        Ok(edges)
    }

    /// Create hierarchical edges from specific memories to agentic patterns
    fn create_hierarchical_edges(
        &mut self,
        entry: &MemoryEntry,
        node_id: NodeId,
    ) -> Result<Vec<EdgeId>> {
        let dim = self.config.embedding_dim;
        let threshold = self.config.semantic_similarity_threshold * 0.8; // Slightly lower threshold

        // Collect pattern nodes first to avoid borrow conflict
        let pattern_nodes: Vec<NodeId> = self
            .agentic_memories
            .iter()
            .filter_map(|(_, (pattern_id, pattern_emb))| {
                let sim = cosine_similarity(&entry.embedding, pattern_emb);
                if sim >= threshold {
                    self.memory_to_node.get(pattern_id).copied()
                } else {
                    None
                }
            })
            .collect();

        // Now create edges with mutable access
        let mut edges = Vec::new();
        for pattern_node in pattern_nodes {
            if let Some(edge_id) = self.create_edge(
                pattern_node, // Pattern is source (general)
                node_id,      // Memory is target (specific)
                MemoryEdgeType::Hierarchical,
                dim,
            )? {
                edges.push(edge_id);
            }
        }

        Ok(edges)
    }

    /// Create a single edge between two nodes
    fn create_edge(
        &mut self,
        source: NodeId,
        target: NodeId,
        edge_type: MemoryEdgeType,
        dim: usize,
    ) -> Result<Option<EdgeId>> {
        // Skip if source and target are the same
        if source == target {
            return Ok(None);
        }

        // Skip if edge already exists
        let existing_edges = self.graph.edges_incident_to(source);
        for edge_id in existing_edges {
            if let Some(edge) = self.graph.get_edge(edge_id) {
                if (edge.source == source && edge.target == target)
                    || (edge.source == target && edge.target == source)
                {
                    return Ok(None);
                }
            }
        }

        let edge = SheafEdgeBuilder::new(source, target)
            .identity_restrictions(dim)
            .weight(edge_type.default_weight())
            .edge_type(edge_type.as_str())
            .namespace("memory")
            .build();

        match self.graph.add_edge(edge) {
            Ok(id) => Ok(Some(id)),
            Err(e) => Err(MemoryCoherenceError::EdgeCreationFailed(e.to_string())),
        }
    }

    /// Find memories that conflict with a given node
    fn find_conflicting_memories(&self, node_id: NodeId) -> Vec<MemoryId> {
        let mut conflicts = Vec::new();
        let threshold = self.config.coherence_threshold;

        // Get all edges incident to this node
        let edges = self.graph.edges_incident_to(node_id);

        for edge_id in edges {
            if let Some(edge) = self.graph.get_edge(edge_id) {
                // Get the state vectors
                let source_state = self.graph.node_state(edge.source);
                let target_state = self.graph.node_state(edge.target);

                if let (Some(src), Some(tgt)) = (source_state, target_state) {
                    let energy = edge.weighted_residual_energy(&src, &tgt);
                    if energy > threshold {
                        // Find the other node in the edge
                        let other_node = if edge.source == node_id {
                            edge.target
                        } else {
                            edge.source
                        };

                        if let Some(&memory_id) = self.node_to_memory.get(&other_node) {
                            conflicts.push(memory_id);
                        }
                    }
                }
            }
        }

        conflicts
    }
}

impl Default for MemoryCoherenceLayer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TRAIT IMPLEMENTATIONS
// ============================================================================

impl AgenticMemory for MemoryCoherenceLayer {
    fn store_pattern(&mut self, key: &str, embedding: &[f32]) -> Result<MemoryId> {
        let entry = MemoryEntry::new(key, embedding.to_vec(), MemoryType::Agentic);
        let result = self.add_with_coherence(entry)?;
        Ok(result.memory_id)
    }

    fn get_pattern(&self, key: &str) -> Option<&[f32]> {
        self.agentic_memories
            .get(key)
            .map(|(_, emb)| emb.as_slice())
    }

    fn pattern_keys(&self) -> Vec<String> {
        self.agentic_memories.keys().cloned().collect()
    }

    fn remove_pattern(&mut self, key: &str) -> bool {
        if let Some((id, _)) = self.agentic_memories.get(key).cloned() {
            self.remove_memory(id).is_ok()
        } else {
            false
        }
    }
}

impl WorkingMemory for MemoryCoherenceLayer {
    fn set_context(&mut self, key: &str, embedding: &[f32]) -> Result<MemoryId> {
        // Remove existing context with this key if it exists
        if let Some((id, _)) = self.working_memories.get(key).cloned() {
            let _ = self.remove_memory(id);
        }

        let entry = MemoryEntry::new(key, embedding.to_vec(), MemoryType::Working);
        let result = self.add_with_coherence(entry)?;
        Ok(result.memory_id)
    }

    fn get_context(&self, key: &str) -> Option<&[f32]> {
        self.working_memories
            .get(key)
            .map(|(_, emb)| emb.as_slice())
    }

    fn clear(&mut self) {
        let ids: Vec<_> = self.working_memories.values().map(|(id, _)| *id).collect();
        for id in ids {
            let _ = self.remove_memory(id);
        }
    }

    fn context_keys(&self) -> Vec<String> {
        self.working_memories.keys().cloned().collect()
    }

    fn size(&self) -> usize {
        self.working_memories.len()
    }
}

impl EpisodicMemory for MemoryCoherenceLayer {
    fn add_episode(&mut self, key: &str, embedding: &[f32]) -> Result<(MemoryId, u64)> {
        let sequence = self.episodic_sequence + 1;
        let entry = MemoryEntry::episodic(key, embedding.to_vec(), sequence);
        let result = self.add_with_coherence(entry)?;
        Ok((result.memory_id, sequence))
    }

    fn get_episode(&self, sequence: u64) -> Option<&[f32]> {
        if sequence == 0 || sequence > self.episodic_memories.len() as u64 {
            return None;
        }
        let idx = (sequence - 1) as usize;
        self.episodic_memories
            .get(idx)
            .map(|(_, _, emb)| emb.as_slice())
    }

    fn recent_episodes(&self, n: usize) -> Vec<(u64, &[f32])> {
        let start = self.episodic_memories.len().saturating_sub(n);
        self.episodic_memories[start..]
            .iter()
            .enumerate()
            .map(|(i, (_, _, emb))| ((start + i + 1) as u64, emb.as_slice()))
            .collect()
    }

    fn episodes_in_range(&self, start: u64, end: u64) -> Vec<(u64, &[f32])> {
        let start_idx = start.saturating_sub(1) as usize;
        let end_idx = (end as usize).min(self.episodic_memories.len());

        if start_idx >= end_idx {
            return Vec::new();
        }

        self.episodic_memories[start_idx..end_idx]
            .iter()
            .enumerate()
            .map(|(i, (_, _, emb))| ((start_idx + i + 1) as u64, emb.as_slice()))
            .collect()
    }

    fn current_sequence(&self) -> u64 {
        self.episodic_sequence
    }

    fn trim_before(&mut self, sequence: u64) {
        if sequence == 0 {
            return;
        }

        let trim_idx = (sequence.saturating_sub(1) as usize).min(self.episodic_memories.len());

        // Collect IDs to remove first
        let ids_to_remove: Vec<MemoryId> = self.episodic_memories[..trim_idx]
            .iter()
            .map(|(id, _, _)| *id)
            .collect();

        // Remove from the vec
        self.episodic_memories.drain(..trim_idx);

        // Then remove from graph
        for id in ids_to_remove {
            let _ = self.remove_memory(id);
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vectors must have same length");

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denom = (norm_a.sqrt() * norm_b.sqrt()).max(1e-10);
    dot / denom
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_random_embedding(dim: usize) -> Vec<f32> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..dim).map(|_| rng.gen::<f32>()).collect()
    }

    fn make_similar_embedding(base: &[f32], noise: f32) -> Vec<f32> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        base.iter()
            .map(|&x| x + rng.gen::<f32>() * noise - noise / 2.0)
            .collect()
    }

    #[test]
    fn test_memory_entry_creation() {
        let embedding = vec![1.0, 0.5, 0.0];
        let entry = MemoryEntry::new("test_key", embedding.clone(), MemoryType::Agentic);

        assert_eq!(entry.key, "test_key");
        assert_eq!(entry.embedding, embedding);
        assert_eq!(entry.memory_type, MemoryType::Agentic);
        assert!(entry.sequence.is_none());
    }

    #[test]
    fn test_episodic_entry_creation() {
        let embedding = vec![1.0, 0.5, 0.0];
        let entry = MemoryEntry::episodic("episode_1", embedding.clone(), 5);

        assert_eq!(entry.memory_type, MemoryType::Episodic);
        assert_eq!(entry.sequence, Some(5));
    }

    #[test]
    fn test_memory_coherence_layer_creation() {
        let layer = MemoryCoherenceLayer::new();
        assert_eq!(layer.memory_count(), 0);
        assert!(layer.is_coherent());
    }

    #[test]
    fn test_add_agentic_memory() {
        let mut layer = MemoryCoherenceLayer::with_config(MemoryCoherenceConfig {
            embedding_dim: 4,
            ..Default::default()
        });

        let embedding = vec![1.0, 0.5, 0.0, 0.2];
        let entry = MemoryEntry::new("pattern_1", embedding, MemoryType::Agentic);
        let result = layer.add_with_coherence(entry).unwrap();

        assert!(result.is_coherent);
        assert_eq!(layer.memory_count(), 1);
        assert!(layer.has_memory(result.memory_id));
    }

    #[test]
    fn test_add_conflicting_memories() {
        let mut layer = MemoryCoherenceLayer::with_config(MemoryCoherenceConfig {
            embedding_dim: 4,
            coherence_threshold: 0.1,
            auto_semantic_edges: true,
            semantic_similarity_threshold: 0.5,
            ..Default::default()
        });

        // Add first memory
        let emb1 = vec![1.0, 0.0, 0.0, 0.0];
        let entry1 = MemoryEntry::new("fact_1", emb1, MemoryType::Agentic);
        layer.add_with_coherence(entry1).unwrap();

        // Add contradicting memory (opposite direction)
        let emb2 = vec![-1.0, 0.0, 0.0, 0.0];
        let entry2 = MemoryEntry::new("fact_2", emb2, MemoryType::Working);
        let result2 = layer.add_with_coherence(entry2).unwrap();

        // The second memory might be flagged as incoherent if edges were created
        // depending on similarity threshold
        assert_eq!(layer.memory_count(), 2);
    }

    #[test]
    fn test_agentic_memory_trait() {
        let mut layer = MemoryCoherenceLayer::with_config(MemoryCoherenceConfig {
            embedding_dim: 4,
            ..Default::default()
        });

        let embedding = vec![1.0, 0.5, 0.0, 0.2];
        let id = layer.store_pattern("user_preference", &embedding).unwrap();

        assert!(layer.has_pattern("user_preference"));
        assert_eq!(
            layer.get_pattern("user_preference"),
            Some(embedding.as_slice())
        );

        let keys = layer.pattern_keys();
        assert_eq!(keys.len(), 1);
        assert!(keys.contains(&"user_preference".to_string()));

        assert!(layer.remove_pattern("user_preference"));
        assert!(!layer.has_pattern("user_preference"));
    }

    #[test]
    fn test_working_memory_trait() {
        let mut layer = MemoryCoherenceLayer::with_config(MemoryCoherenceConfig {
            embedding_dim: 4,
            ..Default::default()
        });

        let emb1 = vec![1.0, 0.5, 0.0, 0.2];
        layer.set_context("current_topic", &emb1).unwrap();

        assert_eq!(layer.size(), 1);
        assert_eq!(layer.get_context("current_topic"), Some(emb1.as_slice()));

        // Update context
        let emb2 = vec![0.0, 1.0, 0.5, 0.3];
        layer.set_context("current_topic", &emb2).unwrap();

        assert_eq!(layer.size(), 1); // Should replace, not add
        assert_eq!(layer.get_context("current_topic"), Some(emb2.as_slice()));

        layer.clear();
        assert_eq!(layer.size(), 0);
    }

    #[test]
    fn test_episodic_memory_trait() {
        let mut layer = MemoryCoherenceLayer::with_config(MemoryCoherenceConfig {
            embedding_dim: 4,
            ..Default::default()
        });

        // Add episodes
        let emb1 = vec![1.0, 0.0, 0.0, 0.0];
        let emb2 = vec![0.0, 1.0, 0.0, 0.0];
        let emb3 = vec![0.0, 0.0, 1.0, 0.0];

        let (_, seq1) = layer.add_episode("turn_1", &emb1).unwrap();
        let (_, seq2) = layer.add_episode("turn_2", &emb2).unwrap();
        let (_, seq3) = layer.add_episode("turn_3", &emb3).unwrap();

        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(seq3, 3);
        assert_eq!(layer.current_sequence(), 3);

        // Get specific episode
        assert_eq!(layer.get_episode(2), Some(emb2.as_slice()));

        // Get recent episodes
        let recent = layer.recent_episodes(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].0, 2);
        assert_eq!(recent[1].0, 3);

        // Get range
        let range = layer.episodes_in_range(1, 3);
        assert_eq!(range.len(), 2);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 1e-6);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_memory_type_display() {
        assert_eq!(MemoryType::Agentic.to_string(), "agentic");
        assert_eq!(MemoryType::Working.to_string(), "working");
        assert_eq!(MemoryType::Episodic.to_string(), "episodic");
    }

    #[test]
    fn test_edge_type_weights() {
        assert!(
            MemoryEdgeType::Temporal.default_weight() > MemoryEdgeType::Semantic.default_weight()
        );
        assert!(
            MemoryEdgeType::Semantic.default_weight()
                > MemoryEdgeType::Hierarchical.default_weight()
        );
    }

    #[test]
    fn test_dimension_validation() {
        let mut layer = MemoryCoherenceLayer::with_config(MemoryCoherenceConfig {
            embedding_dim: 4,
            ..Default::default()
        });

        // Wrong dimension should fail
        let wrong_dim = vec![1.0, 0.5, 0.0]; // 3 instead of 4
        let entry = MemoryEntry::new("test", wrong_dim, MemoryType::Agentic);
        let result = layer.add_with_coherence(entry);

        assert!(matches!(
            result,
            Err(MemoryCoherenceError::InvalidDimension { .. })
        ));
    }
}
