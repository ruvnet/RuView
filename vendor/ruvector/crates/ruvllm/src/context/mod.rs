//! Context Management System for RuvLLM
//!
//! This module provides intelligent context management with semantic memory,
//! pruning, and integration with Claude Flow's memory system.
//!
//! ## Architecture
//!
//! ```text
//! +---------------------+
//! | IntelligentContext  |
//! |     Manager         |
//! +----------+----------+
//!            |
//!     +------+------+
//!     |             |
//! +---v---+   +-----v-----+
//! |Agentic|   | Semantic  |
//! |Memory |   |   Cache   |
//! +---+---+   +-----------+
//!     |
//! +---+---+---+---+---+
//! |   |   |   |   |   |
//! v   v   v   v   v   v
//! Working  Episodic  Semantic  Procedural
//! Memory   Memory    Memory    Memory
//! ```
//!
//! ## Components
//!
//! - **AgenticMemory**: Unified memory combining working, episodic, semantic, and procedural
//! - **WorkingMemory**: Short-term task context with attention weights
//! - **EpisodicMemory**: Long-term trajectory storage with HNSW indexing
//! - **IntelligentContextManager**: Context preparation with pruning and summarization
//! - **SemanticToolCache**: Tool result caching with similarity matching
//! - **ClaudeFlowMemoryBridge**: Integration with Claude Flow memory system
//!
//! ## Usage
//!
//! ```rust,ignore
//! use ruvllm::context::{
//!     IntelligentContextManager, AgenticMemory, ContextManagerConfig,
//! };
//!
//! // Create context manager
//! let config = ContextManagerConfig::default();
//! let manager = IntelligentContextManager::new(config)?;
//!
//! // Prepare context for a request
//! let prepared = manager.prepare_context(
//!     &messages,
//!     &embedding,
//!     max_tokens,
//! )?;
//!
//! // Store in agentic memory
//! manager.memory().store("key", content, embedding)?;
//! ```

pub mod agentic_memory;
pub mod claude_flow_bridge;
pub mod context_manager;
pub mod episodic_memory;
pub mod semantic_cache;
pub mod working_memory;

// Re-exports
pub use agentic_memory::{AgenticMemory, AgenticMemoryConfig, MemoryType};
pub use claude_flow_bridge::{ClaudeFlowBridgeConfig, ClaudeFlowMemoryBridge, SyncResult};
pub use context_manager::{
    ContextElement, ContextManagerConfig, ElementPriority, IntelligentContextManager,
    PreparedContext, PriorityScorer,
};
pub use episodic_memory::{
    CompressedEpisode, Episode, EpisodeMetadata, EpisodicMemory, EpisodicMemoryConfig,
    Trajectory as EpisodeTrajectory,
};
pub use semantic_cache::{CacheStats, CachedToolResult, SemanticCacheConfig, SemanticToolCache};
pub use working_memory::{
    AttentionWeights, ScratchpadEntry, TaskContext, WorkingMemory, WorkingMemoryConfig,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all exports are accessible
        let _config = ContextManagerConfig::default();
        let _mem_config = AgenticMemoryConfig::default();
        let _cache_config = SemanticCacheConfig::default();
    }
}
