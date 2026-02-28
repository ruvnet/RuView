//! LoRA Adapter Manager
//!
//! Manages loading, caching, and hot-swapping of LoRA adapters for
//! efficient model customization at runtime.
//!
//! ## Features
//!
//! - **Hot-swapping**: Switch adapters without model reload
//! - **Memory pooling**: Shared memory pool with KV cache
//! - **Versioning**: Track adapter versions for updates
//! - **Caching**: LRU cache for frequently used adapters

use crate::error::{Result, RuvLLMError};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// LoRA adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    /// Adapter name/identifier
    pub name: String,
    /// LoRA rank (typically 4, 8, 16, 32)
    pub rank: usize,
    /// Alpha scaling factor
    pub alpha: f32,
    /// Dropout rate (0.0 = no dropout)
    pub dropout: f32,
    /// Target modules (e.g., ["q_proj", "v_proj"])
    pub target_modules: Vec<String>,
    /// Whether to merge adapter into base weights
    pub merge_weights: bool,
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            rank: 8,
            alpha: 16.0,
            dropout: 0.0,
            target_modules: vec!["q_proj".to_string(), "v_proj".to_string()],
            merge_weights: false,
        }
    }
}

/// LoRA adapter weights for a single layer
#[derive(Debug, Clone)]
pub struct LoraLayerWeights {
    /// A matrix (in_features x rank)
    pub lora_a: Vec<f32>,
    /// B matrix (rank x out_features)
    pub lora_b: Vec<f32>,
    /// Input dimension
    pub in_features: usize,
    /// Output dimension
    pub out_features: usize,
    /// LoRA rank
    pub rank: usize,
}

impl LoraLayerWeights {
    /// Create new LoRA layer weights (initialized to zero for A, random for B typically)
    pub fn new(in_features: usize, out_features: usize, rank: usize) -> Self {
        Self {
            lora_a: vec![0.0; in_features * rank],
            lora_b: vec![0.0; rank * out_features],
            in_features,
            out_features,
            rank,
        }
    }

    /// Apply LoRA to input: output = input @ (A @ B * scale)
    pub fn apply(&self, input: &[f32], alpha: f32) -> Vec<f32> {
        let scale = alpha / self.rank as f32;

        // input @ A: (batch, in_features) @ (in_features, rank) -> (batch, rank)
        let batch_size = input.len() / self.in_features;
        let mut intermediate = vec![0.0; batch_size * self.rank];

        for b in 0..batch_size {
            for r in 0..self.rank {
                let mut sum = 0.0;
                for i in 0..self.in_features {
                    sum += input[b * self.in_features + i] * self.lora_a[i * self.rank + r];
                }
                intermediate[b * self.rank + r] = sum;
            }
        }

        // intermediate @ B: (batch, rank) @ (rank, out_features) -> (batch, out_features)
        let mut output = vec![0.0; batch_size * self.out_features];

        for b in 0..batch_size {
            for o in 0..self.out_features {
                let mut sum = 0.0;
                for r in 0..self.rank {
                    sum += intermediate[b * self.rank + r] * self.lora_b[r * self.out_features + o];
                }
                output[b * self.out_features + o] = sum * scale;
            }
        }

        output
    }

    /// Get memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        (self.lora_a.len() + self.lora_b.len()) * std::mem::size_of::<f32>()
    }
}

/// Complete LoRA adapter with all layer weights
#[derive(Debug, Clone)]
pub struct LoraAdapter {
    /// Unique adapter ID
    pub id: Uuid,
    /// Configuration
    pub config: AdapterConfig,
    /// Layer weights by module name
    pub layers: HashMap<String, LoraLayerWeights>,
    /// Version number
    pub version: u64,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Reference count
    ref_count: Arc<std::sync::atomic::AtomicUsize>,
}

impl LoraAdapter {
    /// Create a new LoRA adapter
    pub fn new(config: AdapterConfig) -> Self {
        Self {
            id: Uuid::new_v4(),
            config,
            layers: HashMap::new(),
            version: 1,
            created_at: chrono::Utc::now(),
            ref_count: Arc::new(std::sync::atomic::AtomicUsize::new(1)),
        }
    }

    /// Add a layer to the adapter
    pub fn add_layer(&mut self, module_name: String, weights: LoraLayerWeights) {
        self.layers.insert(module_name, weights);
    }

    /// Get total memory usage
    pub fn memory_bytes(&self) -> usize {
        self.layers.values().map(|l| l.memory_bytes()).sum()
    }

    /// Apply adapter to a specific module's output
    pub fn apply(&self, module_name: &str, input: &[f32], base_output: &mut [f32]) -> Result<()> {
        if let Some(layer) = self.layers.get(module_name) {
            let delta = layer.apply(input, self.config.alpha);
            if delta.len() != base_output.len() {
                return Err(RuvLLMError::Adapter(format!(
                    "Output size mismatch: expected {}, got {}",
                    base_output.len(),
                    delta.len()
                )));
            }
            for (out, d) in base_output.iter_mut().zip(delta.iter()) {
                *out += d;
            }
        }
        Ok(())
    }

    /// Increment reference count
    pub fn inc_ref(&self) {
        self.ref_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// Decrement reference count, returns true if count reached zero
    pub fn dec_ref(&self) -> bool {
        self.ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst)
            == 1
    }

    /// Get current reference count
    pub fn ref_count(&self) -> usize {
        self.ref_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// Adapter cache entry
struct CacheEntry {
    adapter: Arc<LoraAdapter>,
    last_accessed: chrono::DateTime<chrono::Utc>,
}

/// LoRA adapter manager
pub struct AdapterManager {
    /// Loaded adapters by ID
    adapters: DashMap<Uuid, Arc<LoraAdapter>>,
    /// Name to ID mapping
    name_to_id: DashMap<String, Uuid>,
    /// LRU cache for eviction
    cache: RwLock<Vec<CacheEntry>>,
    /// Maximum number of adapters to keep loaded
    max_loaded: usize,
    /// Maximum total memory for adapters
    max_memory_bytes: usize,
    /// Current memory usage
    current_memory: std::sync::atomic::AtomicUsize,
}

impl AdapterManager {
    /// Create a new adapter manager
    pub fn new() -> Self {
        Self {
            adapters: DashMap::new(),
            name_to_id: DashMap::new(),
            cache: RwLock::new(Vec::new()),
            max_loaded: 16,
            max_memory_bytes: 512 * 1024 * 1024, // 512MB
            current_memory: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Create with custom limits
    pub fn with_limits(max_loaded: usize, max_memory_bytes: usize) -> Self {
        Self {
            adapters: DashMap::new(),
            name_to_id: DashMap::new(),
            cache: RwLock::new(Vec::new()),
            max_loaded,
            max_memory_bytes,
            current_memory: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Load an adapter
    pub fn load(&self, adapter: LoraAdapter) -> Result<Uuid> {
        let memory_needed = adapter.memory_bytes();

        // Check memory limits
        self.ensure_memory(memory_needed)?;

        let id = adapter.id;
        let name = adapter.config.name.clone();
        let adapter = Arc::new(adapter);

        self.adapters.insert(id, adapter.clone());
        self.name_to_id.insert(name, id);

        // Add to cache
        let mut cache = self.cache.write();
        cache.push(CacheEntry {
            adapter,
            last_accessed: chrono::Utc::now(),
        });

        self.current_memory
            .fetch_add(memory_needed, std::sync::atomic::Ordering::SeqCst);

        Ok(id)
    }

    /// Ensure there's enough memory for a new adapter
    fn ensure_memory(&self, needed: usize) -> Result<()> {
        let current = self
            .current_memory
            .load(std::sync::atomic::Ordering::SeqCst);

        if current + needed <= self.max_memory_bytes {
            return Ok(());
        }

        // Need to evict some adapters
        let mut cache = self.cache.write();

        // Sort by last accessed (oldest first)
        cache.sort_by(|a, b| a.last_accessed.cmp(&b.last_accessed));

        let mut freed = 0;
        while freed < needed && !cache.is_empty() {
            if let Some(entry) = cache.first() {
                if entry.adapter.ref_count() <= 1 {
                    let id = entry.adapter.id;
                    let size = entry.adapter.memory_bytes();

                    // Remove from maps
                    self.adapters.remove(&id);
                    self.name_to_id.remove(&entry.adapter.config.name);

                    cache.remove(0);
                    freed += size;
                    self.current_memory
                        .fetch_sub(size, std::sync::atomic::Ordering::SeqCst);
                } else {
                    // Adapter is in use, move to end
                    let entry = cache.remove(0);
                    cache.push(entry);
                }
            }
        }

        if freed < needed {
            return Err(RuvLLMError::OutOfMemory(
                "Cannot free enough memory for new adapter".to_string(),
            ));
        }

        Ok(())
    }

    /// Get adapter by ID
    pub fn get(&self, id: &Uuid) -> Option<Arc<LoraAdapter>> {
        if let Some(adapter) = self.adapters.get(id) {
            // Update last accessed
            let mut cache = self.cache.write();
            if let Some(entry) = cache.iter_mut().find(|e| e.adapter.id == *id) {
                entry.last_accessed = chrono::Utc::now();
            }
            Some(adapter.clone())
        } else {
            None
        }
    }

    /// Get adapter by name
    pub fn get_by_name(&self, name: &str) -> Option<Arc<LoraAdapter>> {
        self.name_to_id.get(name).and_then(|id| self.get(&id))
    }

    /// Unload an adapter
    pub fn unload(&self, id: &Uuid) -> Result<()> {
        if let Some((_, adapter)) = self.adapters.remove(id) {
            self.name_to_id.remove(&adapter.config.name);

            let mut cache = self.cache.write();
            cache.retain(|e| e.adapter.id != *id);

            self.current_memory
                .fetch_sub(adapter.memory_bytes(), std::sync::atomic::Ordering::SeqCst);
        }
        Ok(())
    }

    /// List all loaded adapters
    pub fn list(&self) -> Vec<AdapterInfo> {
        self.adapters
            .iter()
            .map(|entry| {
                let adapter = entry.value();
                AdapterInfo {
                    id: adapter.id,
                    name: adapter.config.name.clone(),
                    rank: adapter.config.rank,
                    version: adapter.version,
                    memory_bytes: adapter.memory_bytes(),
                    ref_count: adapter.ref_count(),
                }
            })
            .collect()
    }

    /// Get memory statistics
    pub fn memory_stats(&self) -> AdapterMemoryStats {
        AdapterMemoryStats {
            total_budget: self.max_memory_bytes,
            used_bytes: self
                .current_memory
                .load(std::sync::atomic::Ordering::SeqCst),
            adapter_count: self.adapters.len(),
            max_adapters: self.max_loaded,
        }
    }
}

impl Default for AdapterManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Adapter information summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    /// Adapter ID
    pub id: Uuid,
    /// Adapter name
    pub name: String,
    /// LoRA rank
    pub rank: usize,
    /// Version number
    pub version: u64,
    /// Memory usage
    pub memory_bytes: usize,
    /// Current reference count
    pub ref_count: usize,
}

/// Adapter memory statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdapterMemoryStats {
    /// Total memory budget
    pub total_budget: usize,
    /// Currently used bytes
    pub used_bytes: usize,
    /// Number of loaded adapters
    pub adapter_count: usize,
    /// Maximum number of adapters
    pub max_adapters: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lora_layer_weights() {
        let weights = LoraLayerWeights::new(4, 4, 2);
        assert_eq!(weights.lora_a.len(), 8); // 4 * 2
        assert_eq!(weights.lora_b.len(), 8); // 2 * 4
    }

    #[test]
    fn test_lora_adapter() {
        let config = AdapterConfig {
            name: "test".to_string(),
            rank: 4,
            ..Default::default()
        };

        let mut adapter = LoraAdapter::new(config);
        adapter.add_layer("q_proj".to_string(), LoraLayerWeights::new(64, 64, 4));

        assert_eq!(adapter.layers.len(), 1);
        assert!(adapter.memory_bytes() > 0);
    }

    #[test]
    fn test_adapter_manager() {
        let manager = AdapterManager::new();

        let adapter = LoraAdapter::new(AdapterConfig::default());
        let id = manager.load(adapter).unwrap();

        assert!(manager.get(&id).is_some());
        assert!(manager.get_by_name("default").is_some());

        manager.unload(&id).unwrap();
        assert!(manager.get(&id).is_none());
    }
}
