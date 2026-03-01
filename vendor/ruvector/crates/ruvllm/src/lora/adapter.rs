//! Adapter Management: Hot-swapping, Composition, and Memory-efficient Storage
//!
//! This module provides infrastructure for managing multiple LoRA adapters:
//! - Hot-swapping adapters without model reload
//! - Composing multiple adapters (merge, stack, switch)
//! - Memory-efficient storage and caching

use crate::error::{Result, RuvLLMError};
use crate::lora::micro_lora::{LoraAdapter, MicroLoRA, MicroLoraConfig, TargetModule};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

/// Strategy for composing multiple adapters
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompositionStrategy {
    /// Merge adapters by averaging weights
    Average,
    /// Merge adapters by weighted sum
    WeightedSum,
    /// Stack adapters sequentially (apply A then B)
    Sequential,
    /// Use only the most recently activated adapter
    MostRecent,
    /// Select adapter based on task routing
    TaskRouted,
    /// Interpolate between adapters (for smooth transitions)
    Interpolate,
}

impl Default for CompositionStrategy {
    fn default() -> Self {
        Self::MostRecent
    }
}

/// Handle to a registered adapter
#[derive(Debug, Clone)]
pub struct AdapterHandle {
    /// Unique identifier
    pub id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Version number
    pub version: u64,
    /// Reference count
    ref_count: Arc<AtomicUsize>,
    /// Last access timestamp (Unix seconds)
    last_accessed: Arc<AtomicU64>,
}

impl AdapterHandle {
    /// Create a new adapter handle
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            version: 1,
            ref_count: Arc::new(AtomicUsize::new(1)),
            last_accessed: Arc::new(AtomicU64::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            )),
        }
    }

    /// Increment reference count
    pub fn acquire(&self) {
        self.ref_count.fetch_add(1, Ordering::SeqCst);
        self.touch();
    }

    /// Decrement reference count, returns true if count reached zero
    pub fn release(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) == 1
    }

    /// Get current reference count
    pub fn ref_count(&self) -> usize {
        self.ref_count.load(Ordering::SeqCst)
    }

    /// Update last accessed timestamp
    pub fn touch(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_accessed.store(now, Ordering::SeqCst);
    }

    /// Get last accessed timestamp
    pub fn last_accessed(&self) -> u64 {
        self.last_accessed.load(Ordering::SeqCst)
    }
}

/// Entry in the adapter registry
struct RegistryEntry {
    handle: AdapterHandle,
    adapter: Arc<MicroLoRA>,
    metadata: AdapterMetadata,
}

/// Metadata for a registered adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterMetadata {
    /// Task domain this adapter was trained for
    pub domain: Option<String>,
    /// Training data description
    pub training_data: Option<String>,
    /// Quality score from validation
    pub quality_score: f32,
    /// Creation timestamp
    pub created_at: u64,
    /// Tags for categorization
    pub tags: Vec<String>,
}

impl Default for AdapterMetadata {
    fn default() -> Self {
        Self {
            domain: None,
            training_data: None,
            quality_score: 0.0,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            tags: Vec::new(),
        }
    }
}

/// Registry for managing adapter lifecycle
pub struct AdapterRegistry {
    /// Adapters by ID
    adapters: DashMap<Uuid, RegistryEntry>,
    /// Name to ID mapping for lookup
    name_index: DashMap<String, Uuid>,
    /// Current active adapter ID
    active_id: RwLock<Option<Uuid>>,
    /// Maximum adapters to keep in memory
    max_adapters: usize,
    /// Maximum total memory budget (bytes)
    max_memory: usize,
    /// Current memory usage
    current_memory: AtomicUsize,
}

impl AdapterRegistry {
    /// Create a new adapter registry
    pub fn new() -> Self {
        Self {
            adapters: DashMap::new(),
            name_index: DashMap::new(),
            active_id: RwLock::new(None),
            max_adapters: 32,
            max_memory: 64 * 1024 * 1024, // 64MB default
            current_memory: AtomicUsize::new(0),
        }
    }

    /// Create with custom limits
    pub fn with_limits(max_adapters: usize, max_memory: usize) -> Self {
        Self {
            adapters: DashMap::new(),
            name_index: DashMap::new(),
            active_id: RwLock::new(None),
            max_adapters,
            max_memory,
            current_memory: AtomicUsize::new(0),
        }
    }

    /// Register a new adapter
    pub fn register(
        &self,
        name: String,
        adapter: MicroLoRA,
        metadata: AdapterMetadata,
    ) -> Result<AdapterHandle> {
        let memory_needed = adapter.memory_bytes();

        // Ensure we have space
        self.ensure_capacity(memory_needed)?;

        let handle = AdapterHandle::new(name.clone());
        let id = handle.id;

        // Check if name already exists
        if self.name_index.contains_key(&name) {
            return Err(RuvLLMError::Adapter(format!(
                "Adapter with name '{}' already exists",
                name
            )));
        }

        let entry = RegistryEntry {
            handle: handle.clone(),
            adapter: Arc::new(adapter),
            metadata,
        };

        self.adapters.insert(id, entry);
        self.name_index.insert(name, id);
        self.current_memory
            .fetch_add(memory_needed, Ordering::SeqCst);

        Ok(handle)
    }

    /// Get adapter by ID (returns cloned Arc)
    pub fn get(&self, id: &Uuid) -> Option<Arc<MicroLoRA>> {
        self.adapters.get(id).map(|entry| {
            entry.handle.touch();
            entry.adapter.clone()
        })
    }

    /// Get adapter by name
    pub fn get_by_name(&self, name: &str) -> Option<Arc<MicroLoRA>> {
        self.name_index.get(name).and_then(|id| self.get(&id))
    }

    /// Set active adapter by ID
    pub fn set_active(&self, id: Uuid) -> Result<()> {
        if !self.adapters.contains_key(&id) {
            return Err(RuvLLMError::NotFound(format!("Adapter {} not found", id)));
        }
        *self.active_id.write() = Some(id);
        Ok(())
    }

    /// Set active adapter by name
    pub fn set_active_by_name(&self, name: &str) -> Result<()> {
        let id = self
            .name_index
            .get(name)
            .map(|r| *r)
            .ok_or_else(|| RuvLLMError::NotFound(format!("Adapter '{}' not found", name)))?;
        self.set_active(id)
    }

    /// Get the currently active adapter
    pub fn get_active(&self) -> Option<Arc<MicroLoRA>> {
        self.active_id.read().and_then(|id| self.get(&id))
    }

    /// Unregister an adapter
    pub fn unregister(&self, id: &Uuid) -> Result<()> {
        if let Some((_, entry)) = self.adapters.remove(id) {
            self.name_index.remove(&entry.handle.name);
            self.current_memory
                .fetch_sub(entry.adapter.memory_bytes(), Ordering::SeqCst);

            // Clear active if this was the active adapter
            let mut active = self.active_id.write();
            if *active == Some(*id) {
                *active = None;
            }
        }
        Ok(())
    }

    /// List all registered adapters
    pub fn list(&self) -> Vec<AdapterInfo> {
        self.adapters
            .iter()
            .map(|entry| AdapterInfo {
                id: entry.handle.id,
                name: entry.handle.name.clone(),
                version: entry.handle.version,
                ref_count: entry.handle.ref_count(),
                memory_bytes: entry.adapter.memory_bytes(),
                domain: entry.metadata.domain.clone(),
                quality_score: entry.metadata.quality_score,
                last_accessed: entry.handle.last_accessed(),
            })
            .collect()
    }

    /// Get memory statistics
    pub fn memory_stats(&self) -> RegistryStats {
        RegistryStats {
            adapter_count: self.adapters.len(),
            max_adapters: self.max_adapters,
            used_bytes: self.current_memory.load(Ordering::SeqCst),
            max_bytes: self.max_memory,
            active_id: *self.active_id.read(),
        }
    }

    /// Ensure capacity for new adapter
    fn ensure_capacity(&self, needed: usize) -> Result<()> {
        let current = self.current_memory.load(Ordering::SeqCst);

        if current + needed <= self.max_memory && self.adapters.len() < self.max_adapters {
            return Ok(());
        }

        // Need to evict some adapters
        let mut entries: Vec<_> = self
            .adapters
            .iter()
            .map(|e| {
                (
                    e.key().clone(),
                    e.handle.last_accessed(),
                    e.handle.ref_count(),
                )
            })
            .collect();

        // Sort by last accessed (oldest first), then by ref count (lowest first)
        entries.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));

        let mut freed = 0;
        for (id, _, ref_count) in entries {
            if freed >= needed && self.adapters.len() < self.max_adapters {
                break;
            }

            // Don't evict if in use
            if ref_count > 1 {
                continue;
            }

            if let Some((_, entry)) = self.adapters.remove(&id) {
                freed += entry.adapter.memory_bytes();
                self.name_index.remove(&entry.handle.name);
                self.current_memory
                    .fetch_sub(entry.adapter.memory_bytes(), Ordering::SeqCst);
            }
        }

        if freed < needed || self.adapters.len() >= self.max_adapters {
            return Err(RuvLLMError::OutOfMemory(
                "Cannot free enough memory for new adapter".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a registered adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterInfo {
    pub id: Uuid,
    pub name: String,
    pub version: u64,
    pub ref_count: usize,
    pub memory_bytes: usize,
    pub domain: Option<String>,
    pub quality_score: f32,
    pub last_accessed: u64,
}

/// Registry statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegistryStats {
    pub adapter_count: usize,
    pub max_adapters: usize,
    pub used_bytes: usize,
    pub max_bytes: usize,
    pub active_id: Option<Uuid>,
}

/// Pool of pre-allocated adapters for fast acquisition
pub struct AdapterPool {
    /// Available adapters
    available: RwLock<Vec<MicroLoRA>>,
    /// Pool configuration
    config: MicroLoraConfig,
    /// Pool size
    size: usize,
}

impl AdapterPool {
    /// Create a new adapter pool
    pub fn new(config: MicroLoraConfig, size: usize) -> Self {
        let available: Vec<_> = (0..size).map(|_| MicroLoRA::new(config.clone())).collect();

        Self {
            available: RwLock::new(available),
            config,
            size,
        }
    }

    /// Acquire an adapter from the pool
    pub fn acquire(&self) -> Option<MicroLoRA> {
        self.available.write().pop()
    }

    /// Return an adapter to the pool
    pub fn release(&self, mut adapter: MicroLoRA) {
        adapter.reset();
        let mut available = self.available.write();
        if available.len() < self.size {
            available.push(adapter);
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            total_size: self.size,
            available: self.available.read().len(),
            config: self.config.clone(),
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total_size: usize,
    pub available: usize,
    pub config: MicroLoraConfig,
}

/// Composer for multiple adapters
pub struct AdapterComposer {
    /// Adapters to compose
    adapters: Vec<(Arc<MicroLoRA>, f32)>, // (adapter, weight)
    /// Composition strategy
    strategy: CompositionStrategy,
    /// Interpolation factor (for Interpolate strategy)
    interpolation: f32,
    /// Task router (for TaskRouted strategy)
    task_router: Option<Box<dyn Fn(&[f32]) -> usize + Send + Sync>>,
}

impl AdapterComposer {
    /// Create a new composer with default strategy
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
            strategy: CompositionStrategy::default(),
            interpolation: 0.5,
            task_router: None,
        }
    }

    /// Create with specific strategy
    pub fn with_strategy(strategy: CompositionStrategy) -> Self {
        Self {
            adapters: Vec::new(),
            strategy,
            interpolation: 0.5,
            task_router: None,
        }
    }

    /// Add an adapter with weight
    pub fn add(&mut self, adapter: Arc<MicroLoRA>, weight: f32) {
        self.adapters.push((adapter, weight));
    }

    /// Set interpolation factor
    pub fn set_interpolation(&mut self, factor: f32) {
        self.interpolation = factor.clamp(0.0, 1.0);
    }

    /// Set task router function
    pub fn set_task_router<F>(&mut self, router: F)
    where
        F: Fn(&[f32]) -> usize + Send + Sync + 'static,
    {
        self.task_router = Some(Box::new(router));
    }

    /// Forward pass through composed adapters
    pub fn forward(&self, x: &[f32], module: &TargetModule) -> Vec<f32> {
        if self.adapters.is_empty() {
            return vec![0.0; x.len()];
        }

        match self.strategy {
            CompositionStrategy::Average => self.forward_average(x, module),
            CompositionStrategy::WeightedSum => self.forward_weighted(x, module),
            CompositionStrategy::Sequential => self.forward_sequential(x, module),
            CompositionStrategy::MostRecent => self.forward_most_recent(x, module),
            CompositionStrategy::TaskRouted => self.forward_task_routed(x, module),
            CompositionStrategy::Interpolate => self.forward_interpolate(x, module),
        }
    }

    fn forward_average(&self, x: &[f32], module: &TargetModule) -> Vec<f32> {
        let n = self.adapters.len() as f32;
        let mut result = vec![0.0; x.len()];

        for (adapter, _) in &self.adapters {
            let output = adapter.forward(x, module);
            for (r, o) in result.iter_mut().zip(output.iter()) {
                *r += o / n;
            }
        }

        result
    }

    fn forward_weighted(&self, x: &[f32], module: &TargetModule) -> Vec<f32> {
        let total_weight: f32 = self.adapters.iter().map(|(_, w)| w).sum();
        let mut result = vec![0.0; x.len()];

        for (adapter, weight) in &self.adapters {
            let output = adapter.forward(x, module);
            let normalized_weight = weight / total_weight;
            for (r, o) in result.iter_mut().zip(output.iter()) {
                *r += o * normalized_weight;
            }
        }

        result
    }

    fn forward_sequential(&self, x: &[f32], module: &TargetModule) -> Vec<f32> {
        let mut current = x.to_vec();

        for (adapter, _) in &self.adapters {
            let delta = adapter.forward(&current, module);
            for (c, d) in current.iter_mut().zip(delta.iter()) {
                *c += d;
            }
        }

        // Return only the delta (subtract original input)
        for (c, &orig) in current.iter_mut().zip(x.iter()) {
            *c -= orig;
        }

        current
    }

    fn forward_most_recent(&self, x: &[f32], module: &TargetModule) -> Vec<f32> {
        if let Some((adapter, _)) = self.adapters.last() {
            adapter.forward(x, module)
        } else {
            vec![0.0; x.len()]
        }
    }

    fn forward_task_routed(&self, x: &[f32], module: &TargetModule) -> Vec<f32> {
        if let Some(ref router) = self.task_router {
            let idx = router(x);
            if idx < self.adapters.len() {
                return self.adapters[idx].0.forward(x, module);
            }
        }
        // Fall back to most recent
        self.forward_most_recent(x, module)
    }

    fn forward_interpolate(&self, x: &[f32], module: &TargetModule) -> Vec<f32> {
        if self.adapters.len() < 2 {
            return self.forward_most_recent(x, module);
        }

        // Interpolate between last two adapters
        let (adapter_a, _) = &self.adapters[self.adapters.len() - 2];
        let (adapter_b, _) = &self.adapters[self.adapters.len() - 1];

        let output_a = adapter_a.forward(x, module);
        let output_b = adapter_b.forward(x, module);

        let t = self.interpolation;
        output_a
            .iter()
            .zip(output_b.iter())
            .map(|(a, b)| a * (1.0 - t) + b * t)
            .collect()
    }

    /// Clear all adapters
    pub fn clear(&mut self) {
        self.adapters.clear();
    }

    /// Get number of adapters
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }
}

impl Default for AdapterComposer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_handle() {
        let handle = AdapterHandle::new("test".to_string());
        assert_eq!(handle.ref_count(), 1);

        handle.acquire();
        assert_eq!(handle.ref_count(), 2);

        handle.release();
        assert_eq!(handle.ref_count(), 1);
    }

    #[test]
    fn test_registry_basic() {
        let registry = AdapterRegistry::new();
        let config = MicroLoraConfig::for_hidden_dim(64);
        let adapter = MicroLoRA::new(config);

        let handle = registry
            .register(
                "test-adapter".to_string(),
                adapter,
                AdapterMetadata::default(),
            )
            .unwrap();

        assert_eq!(registry.list().len(), 1);
        assert!(registry.get(&handle.id).is_some());
        assert!(registry.get_by_name("test-adapter").is_some());
    }

    #[test]
    fn test_registry_active() {
        let registry = AdapterRegistry::new();
        let config = MicroLoraConfig::for_hidden_dim(64);

        let adapter1 = MicroLoRA::new(config.clone());
        let handle1 = registry
            .register(
                "adapter-1".to_string(),
                adapter1,
                AdapterMetadata::default(),
            )
            .unwrap();

        let adapter2 = MicroLoRA::new(config);
        let _handle2 = registry
            .register(
                "adapter-2".to_string(),
                adapter2,
                AdapterMetadata::default(),
            )
            .unwrap();

        registry.set_active(handle1.id).unwrap();
        assert!(registry.get_active().is_some());

        registry.set_active_by_name("adapter-2").unwrap();
    }

    #[test]
    fn test_adapter_pool() {
        let config = MicroLoraConfig::for_hidden_dim(64);
        let pool = AdapterPool::new(config, 3);

        let stats = pool.stats();
        assert_eq!(stats.total_size, 3);
        assert_eq!(stats.available, 3);

        let adapter1 = pool.acquire().unwrap();
        let adapter2 = pool.acquire().unwrap();

        assert_eq!(pool.stats().available, 1);

        pool.release(adapter1);
        assert_eq!(pool.stats().available, 2);

        pool.release(adapter2);
        assert_eq!(pool.stats().available, 3);
    }

    #[test]
    fn test_composer_average() {
        let config = MicroLoraConfig::for_hidden_dim(64);
        let adapter1 = Arc::new(MicroLoRA::new(config.clone()));
        let adapter2 = Arc::new(MicroLoRA::new(config));

        let mut composer = AdapterComposer::with_strategy(CompositionStrategy::Average);
        composer.add(adapter1, 1.0);
        composer.add(adapter2, 1.0);

        let input = vec![0.1; 64];
        let output = composer.forward(&input, &TargetModule::QProj);
        assert_eq!(output.len(), 64);
    }

    #[test]
    fn test_composer_weighted() {
        let config = MicroLoraConfig::for_hidden_dim(64);
        let adapter1 = Arc::new(MicroLoRA::new(config.clone()));
        let adapter2 = Arc::new(MicroLoRA::new(config));

        let mut composer = AdapterComposer::with_strategy(CompositionStrategy::WeightedSum);
        composer.add(adapter1, 0.7);
        composer.add(adapter2, 0.3);

        let input = vec![0.1; 64];
        let output = composer.forward(&input, &TargetModule::QProj);
        assert_eq!(output.len(), 64);
    }
}
