//! Semantic Memory - Context-Aware AI Memory for ESP32
//!
//! Enables AI to remember and recall information based on meaning,
//! not just keywords. Perfect for:
//! - Personal assistants that remember preferences
//! - Robots that learn from experience
//! - Smart home devices that understand context
//!
//! # How It Works
//!
//! ```text
//! User: "I like my coffee at 7am"
//!         │
//!         ▼
//! ┌─────────────────┐
//! │ Embed to Vector │ ──▶ [0.2, 0.8, -0.1, ...]
//! └─────────────────┘
//!         │
//!         ▼
//! ┌─────────────────┐
//! │ Store in Memory │ ──▶ ID: 42, Type: Preference
//! └─────────────────┘
//!
//! Later: "What time do I like coffee?"
//!         │
//!         ▼
//! ┌─────────────────┐
//! │ Search Similar  │ ──▶ Found: "I like my coffee at 7am"
//! └─────────────────┘
//! ```

use heapless::Vec as HVec;
use heapless::String as HString;
use super::{MicroHNSW, HNSWConfig, SearchResult, MicroVector, DistanceMetric};

/// Maximum memories
pub const MAX_MEMORIES: usize = 128;
/// Maximum text length per memory
pub const MAX_TEXT_LEN: usize = 64;
/// Embedding dimension
pub const MEMORY_DIM: usize = 32;

/// Memory type classification
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryType {
    /// User preference ("I like X")
    Preference,
    /// Factual knowledge ("X is Y")
    Fact,
    /// Event/experience ("Yesterday I did X")
    Event,
    /// Skill/procedure ("To do X, first Y")
    Procedure,
    /// Entity/person ("John is my friend")
    Entity,
    /// Emotional context ("I feel X about Y")
    Emotion,
    /// Conversation context
    Context,
    /// System/device state
    State,
}

impl MemoryType {
    /// Priority weight for retrieval
    pub fn priority(&self) -> i32 {
        match self {
            Self::State => 100,      // Most recent state is critical
            Self::Context => 90,     // Current conversation context
            Self::Preference => 80,  // User preferences matter
            Self::Emotion => 70,     // Emotional context
            Self::Procedure => 60,   // How-to knowledge
            Self::Fact => 50,        // General facts
            Self::Event => 40,       // Past events
            Self::Entity => 30,      // People/things
        }
    }
}

/// A single memory entry
#[derive(Debug, Clone)]
pub struct Memory {
    /// Unique ID
    pub id: u32,
    /// Memory type
    pub memory_type: MemoryType,
    /// Timestamp (seconds since boot or epoch)
    pub timestamp: u32,
    /// Text content (truncated)
    pub text: HString<MAX_TEXT_LEN>,
    /// Importance score (0-100)
    pub importance: u8,
    /// Access count (for recency weighting)
    pub access_count: u16,
    /// Embedding vector
    pub embedding: HVec<i8, MEMORY_DIM>,
}

impl Memory {
    /// Create new memory
    pub fn new(
        id: u32,
        memory_type: MemoryType,
        text: &str,
        embedding: &[i8],
        timestamp: u32,
    ) -> Option<Self> {
        let mut text_str = HString::new();
        for c in text.chars().take(MAX_TEXT_LEN) {
            text_str.push(c).ok()?;
        }

        let mut embed_vec = HVec::new();
        for &v in embedding.iter().take(MEMORY_DIM) {
            embed_vec.push(v).ok()?;
        }

        Some(Self {
            id,
            memory_type,
            timestamp,
            text: text_str,
            importance: 50,
            access_count: 0,
            embedding: embed_vec,
        })
    }

    /// Calculate relevance score
    pub fn relevance_score(&self, distance: i32, current_time: u32) -> i32 {
        let type_weight = self.memory_type.priority();
        let importance_weight = self.importance as i32;

        // Recency decay (newer = higher score)
        let age_seconds = current_time.saturating_sub(self.timestamp);
        let recency = 100 - (age_seconds / 3600).min(100) as i32; // Decay over hours

        // Access frequency boost
        let frequency = (self.access_count as i32).min(50);

        // Combined score (higher is better, distance is inverted)
        let distance_score = 1000 - distance.min(1000);

        (distance_score * 3 + type_weight * 2 + importance_weight + recency + frequency) / 7
    }
}

/// Semantic Memory System
pub struct SemanticMemory {
    /// HNSW index for fast similarity search
    index: MicroHNSW<MEMORY_DIM, MAX_MEMORIES>,
    /// Memory entries
    memories: HVec<Memory, MAX_MEMORIES>,
    /// Next memory ID
    next_id: u32,
    /// Current time (updated externally)
    current_time: u32,
}

impl SemanticMemory {
    /// Create new semantic memory
    pub fn new() -> Self {
        let config = HNSWConfig {
            m: 4,
            m_max0: 8,
            ef_construction: 16,
            ef_search: 8,
            metric: DistanceMetric::Euclidean,
            binary_mode: false,
        };

        Self {
            index: MicroHNSW::new(config),
            memories: HVec::new(),
            next_id: 0,
            current_time: 0,
        }
    }

    /// Update current time
    pub fn set_time(&mut self, time: u32) {
        self.current_time = time;
    }

    /// Number of memories stored
    pub fn len(&self) -> usize {
        self.memories.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }

    /// Memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        self.index.memory_bytes() + self.memories.len() * core::mem::size_of::<Memory>()
    }

    /// Store a new memory
    pub fn remember(
        &mut self,
        memory_type: MemoryType,
        text: &str,
        embedding: &[i8],
    ) -> Result<u32, &'static str> {
        if self.memories.len() >= MAX_MEMORIES {
            // Evict least important memory
            self.evict_least_important()?;
        }

        let id = self.next_id;
        self.next_id += 1;

        let memory = Memory::new(id, memory_type, text, embedding, self.current_time)
            .ok_or("Failed to create memory")?;

        // Add to HNSW index
        let vec = MicroVector {
            data: memory.embedding.clone(),
            id,
        };
        self.index.insert(&vec)?;

        // Store memory
        self.memories.push(memory).map_err(|_| "Memory full")?;

        Ok(id)
    }

    /// Recall memories similar to query
    pub fn recall(&mut self, query_embedding: &[i8], k: usize) -> HVec<(Memory, i32), 16> {
        let mut results = HVec::new();

        let search_results = self.index.search(query_embedding, k * 2);

        for result in search_results.iter() {
            if let Some(memory) = self.find_memory_by_id(result.id) {
                let score = memory.relevance_score(result.distance, self.current_time);
                let _ = results.push((memory.clone(), score));
            }
        }

        // Sort by relevance score
        results.sort_by(|a, b| b.1.cmp(&a.1));

        // Update access counts
        for (mem, _) in results.iter() {
            self.increment_access(mem.id);
        }

        // Truncate to k
        while results.len() > k {
            results.pop();
        }

        results
    }

    /// Recall memories of specific type
    pub fn recall_by_type(
        &mut self,
        query_embedding: &[i8],
        memory_type: MemoryType,
        k: usize,
    ) -> HVec<Memory, 16> {
        let all_results = self.recall(query_embedding, k * 3);

        let mut filtered = HVec::new();
        for (memory, _) in all_results {
            if memory.memory_type == memory_type && filtered.len() < k {
                let _ = filtered.push(memory);
            }
        }

        filtered
    }

    /// Get recent memories
    pub fn recent(&self, k: usize) -> HVec<&Memory, 16> {
        let mut sorted: HVec<&Memory, MAX_MEMORIES> = self.memories.iter().collect();
        sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        let mut result = HVec::new();
        for mem in sorted.iter().take(k) {
            let _ = result.push(*mem);
        }
        result
    }

    /// Forget (remove) a memory
    pub fn forget(&mut self, id: u32) -> bool {
        if let Some(pos) = self.memories.iter().position(|m| m.id == id) {
            self.memories.swap_remove(pos);
            true
        } else {
            false
        }
    }

    /// Find memory by ID
    fn find_memory_by_id(&self, id: u32) -> Option<&Memory> {
        self.memories.iter().find(|m| m.id == id)
    }

    /// Increment access count
    fn increment_access(&mut self, id: u32) {
        if let Some(memory) = self.memories.iter_mut().find(|m| m.id == id) {
            memory.access_count = memory.access_count.saturating_add(1);
        }
    }

    /// Evict least important memory
    fn evict_least_important(&mut self) -> Result<(), &'static str> {
        if self.memories.is_empty() {
            return Ok(());
        }

        // Find memory with lowest score
        let mut min_score = i32::MAX;
        let mut min_idx = 0;

        for (i, memory) in self.memories.iter().enumerate() {
            let score = memory.relevance_score(0, self.current_time);
            if score < min_score {
                min_score = score;
                min_idx = i;
            }
        }

        self.memories.swap_remove(min_idx);
        Ok(())
    }
}

impl Default for SemanticMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_creation() {
        let embedding = [10i8; MEMORY_DIM];
        let memory = Memory::new(1, MemoryType::Preference, "I like coffee", &embedding, 1000);
        assert!(memory.is_some());
        let m = memory.unwrap();
        assert_eq!(m.id, 1);
        assert_eq!(m.memory_type, MemoryType::Preference);
    }

    #[test]
    fn test_semantic_memory() {
        let mut sm = SemanticMemory::new();
        sm.set_time(1000);

        let embed1 = [10i8; MEMORY_DIM];
        let embed2 = [20i8; MEMORY_DIM];

        sm.remember(MemoryType::Preference, "I like tea", &embed1).unwrap();
        sm.remember(MemoryType::Fact, "Water is wet", &embed2).unwrap();

        assert_eq!(sm.len(), 2);

        // Recall similar to embed1
        let query = [11i8; MEMORY_DIM];
        let results = sm.recall(&query, 1);
        assert!(!results.is_empty());
    }
}
