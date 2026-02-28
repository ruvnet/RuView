//! GPU Buffer Management
//!
//! Provides efficient GPU buffer allocation, management, and data transfer
//! for the coherence engine. Implements a buffer pool for reuse and
//! minimizes CPU-GPU synchronization overhead.

use super::error::{GpuError, GpuResult};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device, Queue};

/// Buffer usage flags for coherence computation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferUsage {
    /// Storage buffer for node states
    NodeStates,
    /// Storage buffer for edge data
    EdgeData,
    /// Storage buffer for restriction maps
    RestrictionMaps,
    /// Storage buffer for residuals
    Residuals,
    /// Storage buffer for energy values
    Energies,
    /// Storage buffer for attention weights
    AttentionWeights,
    /// Storage buffer for routing decisions
    RoutingDecisions,
    /// Uniform buffer for shader parameters
    Uniforms,
    /// Staging buffer for CPU readback
    Staging,
}

/// GPU-side node state representation
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuNodeState {
    /// Flattened state vector (padded to MAX_STATE_DIM)
    pub state: [f32; 128], // Will be dynamically sized based on actual dim
    /// Actual dimension of the state vector
    pub dim: u32,
    /// Node index
    pub index: u32,
    /// Padding for alignment
    pub _padding: [u32; 2],
}

/// GPU-side edge representation
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuEdge {
    /// Source node index
    pub source_idx: u32,
    /// Target node index
    pub target_idx: u32,
    /// Edge weight
    pub weight: f32,
    /// Restriction map index for source
    pub rho_source_idx: u32,
    /// Restriction map index for target
    pub rho_target_idx: u32,
    /// Output dimension of restriction maps
    pub comparison_dim: u32,
    /// Padding for alignment
    pub _padding: [u32; 2],
}

/// GPU-side restriction map (dense matrix stored row-major)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuRestrictionMap {
    /// Matrix type: 0=identity, 1=diagonal, 2=projection, 3=dense
    pub map_type: u32,
    /// Input dimension
    pub input_dim: u32,
    /// Output dimension
    pub output_dim: u32,
    /// Offset into the shared data buffer
    pub data_offset: u32,
    /// Number of elements in data
    pub data_len: u32,
    /// Padding for alignment
    pub _padding: [u32; 3],
}

/// GPU-side shader parameters
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuParams {
    /// Number of edges
    pub num_edges: u32,
    /// Number of nodes
    pub num_nodes: u32,
    /// State dimension
    pub state_dim: u32,
    /// Beta parameter for attention
    pub beta: f32,
    /// Lane 0 threshold (reflex)
    pub threshold_lane0: f32,
    /// Lane 1 threshold (retrieval)
    pub threshold_lane1: f32,
    /// Lane 2 threshold (heavy)
    pub threshold_lane2: f32,
    /// Flag to control residual storage (0 = skip, 1 = store)
    /// When computing energy only, skip storage for better performance
    pub store_residuals: u32,
}

/// Wrapper around a wgpu Buffer with metadata
pub struct GpuBuffer {
    /// The underlying wgpu buffer
    pub buffer: Buffer,
    /// Size in bytes
    pub size: usize,
    /// Usage flags
    pub usage: BufferUsage,
    /// Label for debugging
    pub label: String,
}

impl GpuBuffer {
    /// Create a new GPU buffer
    pub fn new(
        device: &Device,
        size: usize,
        usage: BufferUsage,
        label: impl Into<String>,
    ) -> GpuResult<Self> {
        let label = label.into();
        let wgpu_usage = Self::to_wgpu_usage(usage);

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(&label),
            size: size as u64,
            usage: wgpu_usage,
            mapped_at_creation: false,
        });

        Ok(Self {
            buffer,
            size,
            usage,
            label,
        })
    }

    /// Create a new GPU buffer with initial data
    pub fn new_with_data<T: Pod>(
        device: &Device,
        queue: &Queue,
        data: &[T],
        usage: BufferUsage,
        label: impl Into<String>,
    ) -> GpuResult<Self> {
        let label = label.into();
        let bytes = bytemuck::cast_slice(data);
        let size = bytes.len();
        let wgpu_usage = Self::to_wgpu_usage(usage);

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(&label),
            size: size as u64,
            usage: wgpu_usage,
            mapped_at_creation: false,
        });

        queue.write_buffer(&buffer, 0, bytes);

        Ok(Self {
            buffer,
            size,
            usage,
            label,
        })
    }

    /// Write data to the buffer
    pub fn write<T: Pod>(&self, queue: &Queue, data: &[T]) -> GpuResult<()> {
        let bytes = bytemuck::cast_slice(data);
        if bytes.len() > self.size {
            return Err(GpuError::BufferSizeMismatch {
                expected: self.size,
                actual: bytes.len(),
            });
        }
        queue.write_buffer(&self.buffer, 0, bytes);
        Ok(())
    }

    /// Convert our usage to wgpu usage flags
    fn to_wgpu_usage(usage: BufferUsage) -> BufferUsages {
        match usage {
            BufferUsage::NodeStates
            | BufferUsage::EdgeData
            | BufferUsage::RestrictionMaps
            | BufferUsage::Residuals
            | BufferUsage::Energies
            | BufferUsage::AttentionWeights
            | BufferUsage::RoutingDecisions => {
                BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST
            }
            BufferUsage::Uniforms => BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            BufferUsage::Staging => BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        }
    }
}

/// Buffer manager for efficient allocation and reuse
pub struct GpuBufferManager {
    device: Arc<Device>,
    queue: Arc<Queue>,
    /// Buffer pool keyed by (usage, size_bucket)
    pool: HashMap<(BufferUsage, usize), Vec<GpuBuffer>>,
    /// Active buffers currently in use
    active: HashMap<String, GpuBuffer>,
}

impl GpuBufferManager {
    /// Create a new buffer manager
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            device,
            queue,
            pool: HashMap::new(),
            active: HashMap::new(),
        }
    }

    /// Allocate or reuse a buffer
    pub fn allocate(
        &mut self,
        size: usize,
        usage: BufferUsage,
        label: impl Into<String>,
    ) -> GpuResult<&GpuBuffer> {
        let label = label.into();
        let bucket = Self::size_bucket(size);

        // Try to reuse from pool
        if let Some(buffers) = self.pool.get_mut(&(usage, bucket)) {
            if let Some(buffer) = buffers.pop() {
                self.active.insert(label.clone(), buffer);
                return Ok(self.active.get(&label).unwrap());
            }
        }

        // Allocate new buffer
        let buffer = GpuBuffer::new(&self.device, bucket, usage, &label)?;
        self.active.insert(label.clone(), buffer);
        Ok(self.active.get(&label).unwrap())
    }

    /// Allocate or reuse a buffer with initial data
    pub fn allocate_with_data<T: Pod>(
        &mut self,
        data: &[T],
        usage: BufferUsage,
        label: impl Into<String>,
    ) -> GpuResult<&GpuBuffer> {
        let label = label.into();
        let size = std::mem::size_of_val(data);
        let bucket = Self::size_bucket(size);

        // Try to reuse from pool
        if let Some(buffers) = self.pool.get_mut(&(usage, bucket)) {
            if let Some(buffer) = buffers.pop() {
                buffer.write(&self.queue, data)?;
                self.active.insert(label.clone(), buffer);
                return Ok(self.active.get(&label).unwrap());
            }
        }

        // Allocate new buffer with data
        let buffer = GpuBuffer::new_with_data(&self.device, &self.queue, data, usage, &label)?;
        self.active.insert(label.clone(), buffer);
        Ok(self.active.get(&label).unwrap())
    }

    /// Get an active buffer by label
    pub fn get(&self, label: &str) -> Option<&GpuBuffer> {
        self.active.get(label)
    }

    /// Release a buffer back to the pool for reuse
    pub fn release(&mut self, label: &str) {
        if let Some(buffer) = self.active.remove(label) {
            let bucket = Self::size_bucket(buffer.size);
            self.pool
                .entry((buffer.usage, bucket))
                .or_default()
                .push(buffer);
        }
    }

    /// Release all active buffers back to the pool
    pub fn release_all(&mut self) {
        let labels: Vec<_> = self.active.keys().cloned().collect();
        for label in labels {
            self.release(&label);
        }
    }

    /// Clear all buffers (both pool and active)
    pub fn clear(&mut self) {
        self.active.clear();
        self.pool.clear();
    }

    /// Round size up to nearest power of 2 for efficient reuse
    fn size_bucket(size: usize) -> usize {
        const MIN_BUCKET: usize = 256;
        if size <= MIN_BUCKET {
            MIN_BUCKET
        } else {
            size.next_power_of_two()
        }
    }

    /// Get the underlying device
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Get the underlying queue
    pub fn queue(&self) -> &Queue {
        &self.queue
    }
}

// ============================================================================
// BUFFER USAGE FLAGS (for pipeline.rs compatibility)
// ============================================================================

/// Buffer usage flags for flexible configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferUsageFlags {
    /// Can be read from GPU (STORAGE)
    pub storage_read: bool,
    /// Can be written to by GPU (STORAGE)
    pub storage_write: bool,
    /// Can be used as uniform buffer
    pub uniform: bool,
    /// Can be mapped for CPU read
    pub map_read: bool,
    /// Can be mapped for CPU write
    pub map_write: bool,
    /// Can be used as copy source
    pub copy_src: bool,
    /// Can be used as copy destination
    pub copy_dst: bool,
    /// Can be used for indirect dispatch
    pub indirect: bool,
}

impl BufferUsageFlags {
    /// Storage buffer (read-only)
    pub const fn storage_readonly() -> Self {
        Self {
            storage_read: true,
            storage_write: false,
            uniform: false,
            map_read: false,
            map_write: false,
            copy_src: true,
            copy_dst: true,
            indirect: false,
        }
    }

    /// Storage buffer (read-write)
    pub const fn storage_readwrite() -> Self {
        Self {
            storage_read: true,
            storage_write: true,
            uniform: false,
            map_read: false,
            map_write: false,
            copy_src: true,
            copy_dst: true,
            indirect: false,
        }
    }

    /// Uniform buffer
    pub const fn uniform() -> Self {
        Self {
            storage_read: false,
            storage_write: false,
            uniform: true,
            map_read: false,
            map_write: false,
            copy_src: false,
            copy_dst: true,
            indirect: false,
        }
    }

    /// Staging buffer for read-back
    pub const fn staging_read() -> Self {
        Self {
            storage_read: false,
            storage_write: false,
            uniform: false,
            map_read: true,
            map_write: false,
            copy_src: false,
            copy_dst: true,
            indirect: false,
        }
    }

    /// Staging buffer for upload
    pub const fn staging_write() -> Self {
        Self {
            storage_read: false,
            storage_write: false,
            uniform: false,
            map_read: false,
            map_write: true,
            copy_src: true,
            copy_dst: false,
            indirect: false,
        }
    }

    /// Indirect dispatch buffer
    pub const fn indirect() -> Self {
        Self {
            storage_read: true,
            storage_write: true,
            uniform: false,
            map_read: false,
            map_write: false,
            copy_src: true,
            copy_dst: true,
            indirect: true,
        }
    }

    /// Convert to wgpu buffer usages
    pub fn to_wgpu(&self) -> BufferUsages {
        let mut usages = BufferUsages::empty();

        if self.storage_read || self.storage_write {
            usages |= BufferUsages::STORAGE;
        }
        if self.uniform {
            usages |= BufferUsages::UNIFORM;
        }
        if self.map_read {
            usages |= BufferUsages::MAP_READ;
        }
        if self.map_write {
            usages |= BufferUsages::MAP_WRITE;
        }
        if self.copy_src {
            usages |= BufferUsages::COPY_SRC;
        }
        if self.copy_dst {
            usages |= BufferUsages::COPY_DST;
        }
        if self.indirect {
            usages |= BufferUsages::INDIRECT;
        }

        usages
    }
}

// ============================================================================
// BUFFER KEY AND POOL (for dispatch.rs compatibility)
// ============================================================================

/// Key for buffer pool lookups
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BufferKey {
    /// Buffer size in bytes
    pub size: u64,
    /// Buffer usage flags
    pub usage: BufferUsageFlags,
}

impl BufferKey {
    /// Create a new buffer key
    pub fn new(size: u64, usage: BufferUsageFlags) -> Self {
        Self { size, usage }
    }
}

/// Buffer pool for reusing GPU allocations with DashMap for concurrent access
pub struct GpuBufferPool {
    device: Arc<Device>,
    buffers: dashmap::DashMap<BufferKey, Vec<GpuBuffer>>,
    max_pool_size: usize,
}

impl GpuBufferPool {
    /// Create a new buffer pool
    pub fn new(device: Arc<Device>) -> Self {
        Self::with_capacity(device, super::DEFAULT_POOL_CAPACITY)
    }

    /// Create a new buffer pool with custom capacity
    pub fn with_capacity(device: Arc<Device>, max_pool_size: usize) -> Self {
        Self {
            device,
            buffers: dashmap::DashMap::new(),
            max_pool_size,
        }
    }

    /// Acquire a buffer from the pool or create a new one.
    pub fn acquire(&self, size: u64, usage: BufferUsageFlags) -> GpuResult<GpuBuffer> {
        if size > super::MAX_BUFFER_SIZE {
            return Err(GpuError::BufferTooLarge {
                size,
                max: super::MAX_BUFFER_SIZE,
            });
        }

        let key = BufferKey::new(size, usage);

        // Try to get from pool
        if let Some(mut buffers) = self.buffers.get_mut(&key) {
            if let Some(buffer) = buffers.pop() {
                return Ok(buffer);
            }
        }

        // Create new buffer
        let wgpu_buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some("pooled_buffer"),
            size,
            usage: usage.to_wgpu(),
            mapped_at_creation: false,
        });

        Ok(GpuBuffer {
            buffer: wgpu_buffer,
            size: size as usize,
            usage: BufferUsage::Staging, // Default usage type
            label: "pooled_buffer".to_string(),
        })
    }

    /// Return a buffer to the pool for reuse.
    pub fn release(&self, buffer: GpuBuffer) {
        let size = buffer.size as u64;
        let usage = BufferUsageFlags::storage_readwrite(); // Default
        let key = BufferKey::new(size, usage);

        let mut buffers = self.buffers.entry(key).or_insert_with(Vec::new);
        if buffers.len() < self.max_pool_size {
            buffers.push(buffer);
        }
    }

    /// Clear all pooled buffers
    pub fn clear(&self) {
        self.buffers.clear();
    }

    /// Get statistics about the pool
    pub fn stats(&self) -> PoolStats {
        let mut total_buffers = 0;
        let mut total_bytes = 0u64;

        for entry in self.buffers.iter() {
            total_buffers += entry.value().len();
            total_bytes += entry.key().size * entry.value().len() as u64;
        }

        PoolStats {
            total_buffers,
            total_bytes,
            bucket_count: self.buffers.len(),
        }
    }
}

/// Statistics about the buffer pool
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total number of pooled buffers
    pub total_buffers: usize,
    /// Total bytes allocated in pool
    pub total_bytes: u64,
    /// Number of unique buffer configurations
    pub bucket_count: usize,
}

// ============================================================================
// EXTENDED GPUBUFFER METHODS (for pipeline.rs compatibility)
// ============================================================================

impl GpuBuffer {
    /// Create a binding entry for this buffer.
    pub fn binding(&self, binding: u32) -> wgpu::BindGroupEntry {
        wgpu::BindGroupEntry {
            binding,
            resource: self.buffer.as_entire_binding(),
        }
    }

    /// Get the underlying wgpu buffer
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Create a new storage buffer with initial data (for dispatch compatibility)
    pub fn new_storage<T: Pod>(
        device: &Device,
        queue: &Queue,
        data: &[T],
        read_write: bool,
    ) -> GpuResult<Self> {
        let usage = if read_write {
            BufferUsage::Residuals
        } else {
            BufferUsage::NodeStates
        };
        Self::new_with_data(device, queue, data, usage, "storage_buffer")
    }

    /// Create a new uninitialized storage buffer
    pub fn new_storage_uninit<T: Pod>(
        device: &Device,
        count: usize,
        read_write: bool,
    ) -> GpuResult<Self> {
        let size = count * std::mem::size_of::<T>();
        let usage = if read_write {
            BufferUsage::Residuals
        } else {
            BufferUsage::NodeStates
        };
        Self::new(device, size, usage, "storage_buffer_uninit")
    }

    /// Create a new uniform buffer with data
    pub fn new_uniform<T: Pod>(device: &Device, queue: &Queue, data: &T) -> GpuResult<Self> {
        Self::new_with_data(
            device,
            queue,
            std::slice::from_ref(data),
            BufferUsage::Uniforms,
            "uniform_buffer",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_bucket() {
        assert_eq!(GpuBufferManager::size_bucket(100), 256);
        assert_eq!(GpuBufferManager::size_bucket(256), 256);
        assert_eq!(GpuBufferManager::size_bucket(257), 512);
        assert_eq!(GpuBufferManager::size_bucket(1000), 1024);
    }

    #[test]
    fn test_gpu_params_alignment() {
        // Ensure our GPU structs are properly aligned for wgpu
        assert_eq!(std::mem::size_of::<GpuParams>(), 32);
        assert_eq!(std::mem::align_of::<GpuParams>(), 4);
    }

    #[test]
    fn test_gpu_edge_alignment() {
        assert_eq!(std::mem::size_of::<GpuEdge>(), 32);
        assert_eq!(std::mem::align_of::<GpuEdge>(), 4);
    }

    #[test]
    fn test_gpu_restriction_map_alignment() {
        assert_eq!(std::mem::size_of::<GpuRestrictionMap>(), 32);
        assert_eq!(std::mem::align_of::<GpuRestrictionMap>(), 4);
    }

    #[test]
    fn test_buffer_usage_flags() {
        let readonly = BufferUsageFlags::storage_readonly();
        assert!(readonly.storage_read);
        assert!(!readonly.storage_write);

        let readwrite = BufferUsageFlags::storage_readwrite();
        assert!(readwrite.storage_read);
        assert!(readwrite.storage_write);
    }

    #[test]
    fn test_buffer_key_equality() {
        let key1 = BufferKey::new(1024, BufferUsageFlags::storage_readonly());
        let key2 = BufferKey::new(1024, BufferUsageFlags::storage_readonly());
        let key3 = BufferKey::new(2048, BufferUsageFlags::storage_readonly());

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }
}
