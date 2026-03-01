//! Shared vector memory for distributed RAG
//!
//! Enables agents to share vector embeddings and semantic memories across the swarm.

use crate::{Result, SwarmError, compression::TensorCodec};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

/// Vector memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    pub id: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub metadata: HashMap<String, String>,
    pub timestamp: u64,
    pub owner_agent: String,
    pub access_count: u64,
}

impl VectorEntry {
    pub fn new(id: &str, content: &str, embedding: Vec<f32>, owner: &str) -> Self {
        Self {
            id: id.to_string(),
            content: content.to_string(),
            embedding,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            owner_agent: owner.to_string(),
            access_count: 0,
        }
    }

    /// Compute cosine similarity with query vector
    pub fn similarity(&self, query: &[f32]) -> f32 {
        if self.embedding.len() != query.len() {
            return 0.0;
        }

        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for (a, b) in self.embedding.iter().zip(query.iter()) {
            dot += a * b;
            norm_a += a * a;
            norm_b += b * b;
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

/// Shared vector memory across swarm
pub struct VectorMemory {
    entries: Arc<RwLock<HashMap<String, VectorEntry>>>,
    agent_id: String,
    max_entries: usize,
    codec: TensorCodec,
}

impl VectorMemory {
    /// Create new vector memory
    pub fn new(agent_id: &str, max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            agent_id: agent_id.to_string(),
            max_entries,
            codec: TensorCodec::new(),
        }
    }

    /// Store a vector entry
    pub fn store(&self, content: &str, embedding: Vec<f32>) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let entry = VectorEntry::new(&id, content, embedding, &self.agent_id);

        let mut entries = self.entries.write();

        // Evict oldest if at capacity
        if entries.len() >= self.max_entries {
            if let Some(oldest_id) = entries
                .iter()
                .min_by_key(|(_, e)| e.timestamp)
                .map(|(id, _)| id.clone())
            {
                entries.remove(&oldest_id);
            }
        }

        entries.insert(id.clone(), entry);
        Ok(id)
    }

    /// Search for similar vectors
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(VectorEntry, f32)> {
        let mut entries = self.entries.write();

        let mut results: Vec<_> = entries
            .values_mut()
            .map(|entry| {
                entry.access_count += 1;
                let score = entry.similarity(query);
                (entry.clone(), score)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    /// Get entry by ID
    pub fn get(&self, id: &str) -> Option<VectorEntry> {
        let mut entries = self.entries.write();
        if let Some(entry) = entries.get_mut(id) {
            entry.access_count += 1;
            Some(entry.clone())
        } else {
            None
        }
    }

    /// Delete entry
    pub fn delete(&self, id: &str) -> bool {
        let mut entries = self.entries.write();
        entries.remove(id).is_some()
    }

    /// Serialize all entries for sync
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let entries = self.entries.read();
        let data: Vec<_> = entries.values().cloned().collect();
        let json = serde_json::to_vec(&data)
            .map_err(|e| SwarmError::Serialization(e.to_string()))?;
        self.codec.compress(&json)
    }

    /// Merge entries from peer
    pub fn merge(&self, data: &[u8]) -> Result<usize> {
        let json = self.codec.decompress(data)?;
        let peer_entries: Vec<VectorEntry> = serde_json::from_slice(&json)
            .map_err(|e| SwarmError::Serialization(e.to_string()))?;

        let mut entries = self.entries.write();
        let mut merged = 0;

        for entry in peer_entries {
            if !entries.contains_key(&entry.id) {
                if entries.len() < self.max_entries {
                    entries.insert(entry.id.clone(), entry);
                    merged += 1;
                }
            }
        }

        Ok(merged)
    }

    /// Get memory stats
    pub fn stats(&self) -> MemoryStats {
        let entries = self.entries.read();

        let total_vectors = entries.len();
        let total_dims: usize = entries.values().map(|e| e.embedding.len()).sum();
        let avg_dims = if total_vectors > 0 {
            total_dims / total_vectors
        } else {
            0
        };

        let total_accesses: u64 = entries.values().map(|e| e.access_count).sum();

        MemoryStats {
            total_entries: total_vectors,
            avg_dimensions: avg_dims,
            total_accesses,
            memory_bytes: total_dims * 4, // f32 = 4 bytes
        }
    }
}

/// Memory statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_entries: usize,
    pub avg_dimensions: usize,
    pub total_accesses: u64,
    pub memory_bytes: usize,
}

/// Shared memory segment for high-performance local IPC
pub struct SharedMemory {
    name: String,
    size: usize,
    // In real implementation, this would use mmap or shared memory
    buffer: Arc<RwLock<Vec<u8>>>,
}

impl SharedMemory {
    /// Create or attach to shared memory segment
    pub fn new(name: &str, size: usize) -> Result<Self> {
        Ok(Self {
            name: name.to_string(),
            size,
            buffer: Arc::new(RwLock::new(vec![0u8; size])),
        })
    }

    /// Write data at offset
    pub fn write(&self, offset: usize, data: &[u8]) -> Result<()> {
        let mut buffer = self.buffer.write();

        if offset + data.len() > self.size {
            return Err(SwarmError::Transport("Buffer overflow".into()));
        }

        buffer[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Read data at offset
    pub fn read(&self, offset: usize, len: usize) -> Result<Vec<u8>> {
        let buffer = self.buffer.read();

        if offset + len > self.size {
            return Err(SwarmError::Transport("Buffer underflow".into()));
        }

        Ok(buffer[offset..offset + len].to_vec())
    }

    /// Get segment info
    pub fn info(&self) -> SharedMemoryInfo {
        SharedMemoryInfo {
            name: self.name.clone(),
            size: self.size,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedMemoryInfo {
    pub name: String,
    pub size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_memory() {
        let memory = VectorMemory::new("test-agent", 100);

        let embedding = vec![0.1, 0.2, 0.3, 0.4];
        let id = memory.store("test content", embedding.clone()).unwrap();

        let results = memory.search(&embedding, 5);
        assert!(!results.is_empty());
        assert!(results[0].1 > 0.99); // Should be almost identical

        let entry = memory.get(&id);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().content, "test content");
    }

    #[test]
    fn test_shared_memory() {
        let shm = SharedMemory::new("test-segment", 1024).unwrap();

        let data = b"Hello, Swarm!";
        shm.write(0, data).unwrap();

        let read = shm.read(0, data.len()).unwrap();
        assert_eq!(read, data);
    }
}
