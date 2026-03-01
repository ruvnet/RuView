//! Semantic Memory - Context-Aware AI Memory for ESP32

use heapless::Vec as HVec;
use heapless::String as HString;
use super::{MicroHNSW, HNSWConfig, MicroVector, DistanceMetric};

pub const MAX_MEMORIES: usize = 128;
pub const MAX_TEXT_LEN: usize = 64;
pub const MEMORY_DIM: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryType {
    Preference,
    Fact,
    Event,
    Procedure,
    Entity,
    Emotion,
    Context,
    State,
}

impl MemoryType {
    pub fn priority(&self) -> i32 {
        match self {
            Self::State => 100, Self::Context => 90, Self::Preference => 80, Self::Emotion => 70,
            Self::Procedure => 60, Self::Fact => 50, Self::Event => 40, Self::Entity => 30,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Memory {
    pub id: u32,
    pub memory_type: MemoryType,
    pub timestamp: u32,
    pub text: HString<MAX_TEXT_LEN>,
    pub importance: u8,
    pub access_count: u16,
    pub embedding: HVec<i8, MEMORY_DIM>,
}

impl Memory {
    pub fn new(id: u32, memory_type: MemoryType, text: &str, embedding: &[i8], timestamp: u32) -> Option<Self> {
        let mut text_str = HString::new();
        for c in text.chars().take(MAX_TEXT_LEN) { text_str.push(c).ok()?; }
        let mut embed_vec = HVec::new();
        for &v in embedding.iter().take(MEMORY_DIM) { embed_vec.push(v).ok()?; }
        Some(Self { id, memory_type, timestamp, text: text_str, importance: 50, access_count: 0, embedding: embed_vec })
    }

    pub fn relevance_score(&self, distance: i32, current_time: u32) -> i32 {
        let type_weight = self.memory_type.priority();
        let importance_weight = self.importance as i32;
        let age = current_time.saturating_sub(self.timestamp);
        let recency = 100 - (age / 3600).min(100) as i32;
        let frequency = (self.access_count as i32).min(50);
        let distance_score = 1000 - distance.min(1000);
        (distance_score * 3 + type_weight * 2 + importance_weight + recency + frequency) / 7
    }
}

pub struct SemanticMemory {
    index: MicroHNSW<MEMORY_DIM, MAX_MEMORIES>,
    memories: HVec<Memory, MAX_MEMORIES>,
    next_id: u32,
    current_time: u32,
}

impl SemanticMemory {
    pub fn new() -> Self {
        let config = HNSWConfig { m: 4, m_max0: 8, ef_construction: 16, ef_search: 8, metric: DistanceMetric::Euclidean, binary_mode: false };
        Self { index: MicroHNSW::new(config), memories: HVec::new(), next_id: 0, current_time: 0 }
    }

    pub fn set_time(&mut self, time: u32) { self.current_time = time; }
    pub fn len(&self) -> usize { self.memories.len() }
    pub fn is_empty(&self) -> bool { self.memories.is_empty() }
    pub fn memory_bytes(&self) -> usize { self.index.memory_bytes() + self.memories.len() * core::mem::size_of::<Memory>() }

    pub fn remember(&mut self, memory_type: MemoryType, text: &str, embedding: &[i8]) -> Result<u32, &'static str> {
        if self.memories.len() >= MAX_MEMORIES { self.evict_least_important()?; }

        let id = self.next_id;
        self.next_id += 1;

        let memory = Memory::new(id, memory_type, text, embedding, self.current_time).ok_or("Failed to create memory")?;
        let vec = MicroVector { data: memory.embedding.clone(), id };
        self.index.insert(&vec)?;
        self.memories.push(memory).map_err(|_| "Memory full")?;
        Ok(id)
    }

    pub fn recall(&mut self, query: &[i8], k: usize) -> HVec<(Memory, i32), 16> {
        let mut results = HVec::new();
        let search_results = self.index.search(query, k * 2);

        for result in search_results.iter() {
            if let Some(memory) = self.find_by_id(result.id) {
                let score = memory.relevance_score(result.distance, self.current_time);
                let _ = results.push((memory.clone(), score));
            }
        }

        results.sort_by(|a, b| b.1.cmp(&a.1));
        for (mem, _) in results.iter() { self.increment_access(mem.id); }
        while results.len() > k { results.pop(); }
        results
    }

    pub fn recall_by_type(&mut self, query: &[i8], memory_type: MemoryType, k: usize) -> HVec<Memory, 16> {
        let all = self.recall(query, k * 3);
        let mut filtered = HVec::new();
        for (mem, _) in all {
            if mem.memory_type == memory_type && filtered.len() < k { let _ = filtered.push(mem); }
        }
        filtered
    }

    pub fn recent(&self, k: usize) -> HVec<&Memory, 16> {
        let mut sorted: HVec<&Memory, MAX_MEMORIES> = self.memories.iter().collect();
        sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        let mut result = HVec::new();
        for mem in sorted.iter().take(k) { let _ = result.push(*mem); }
        result
    }

    pub fn forget(&mut self, id: u32) -> bool {
        if let Some(pos) = self.memories.iter().position(|m| m.id == id) {
            self.memories.swap_remove(pos);
            true
        } else { false }
    }

    fn find_by_id(&self, id: u32) -> Option<&Memory> { self.memories.iter().find(|m| m.id == id) }

    fn increment_access(&mut self, id: u32) {
        if let Some(m) = self.memories.iter_mut().find(|m| m.id == id) {
            m.access_count = m.access_count.saturating_add(1);
        }
    }

    fn evict_least_important(&mut self) -> Result<(), &'static str> {
        if self.memories.is_empty() { return Ok(()); }
        let mut min_score = i32::MAX;
        let mut min_idx = 0;
        for (i, mem) in self.memories.iter().enumerate() {
            let score = mem.relevance_score(0, self.current_time);
            if score < min_score { min_score = score; min_idx = i; }
        }
        self.memories.swap_remove(min_idx);
        Ok(())
    }
}

impl Default for SemanticMemory { fn default() -> Self { Self::new() } }
