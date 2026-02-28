//! GPU Backend Abstraction Layer
//!
//! Provides a unified interface for different GPU backends:
//! - WebGPU (via wgpu)
//! - CUDA-WASM (optional, via cuda-rust-wasm)
//! - CPU fallback

use crate::{EmbeddingError, Result};
use super::config::{GpuConfig, GpuMemoryStats, GpuMode, PowerPreference};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};

/// Global buffer ID counter
static BUFFER_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
static PIPELINE_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// GPU device information
#[derive(Debug, Clone)]
pub struct GpuInfo {
    /// Device name
    pub name: String,
    /// Vendor name
    pub vendor: String,
    /// Backend type (WebGPU, CUDA-WASM, CPU)
    pub backend: String,
    /// API version
    pub api_version: String,
    /// Driver version
    pub driver_version: String,
    /// Total memory (bytes)
    pub total_memory: u64,
    /// Maximum workgroup size
    pub max_workgroup_size: u32,
    /// Maximum buffer size
    pub max_buffer_size: u64,
    /// Supports compute shaders
    pub supports_compute: bool,
    /// Supports float16
    pub supports_f16: bool,
}

impl Default for GpuInfo {
    fn default() -> Self {
        Self {
            name: "Unknown".to_string(),
            vendor: "Unknown".to_string(),
            backend: "CPU".to_string(),
            api_version: "N/A".to_string(),
            driver_version: "N/A".to_string(),
            total_memory: 0,
            max_workgroup_size: 256,
            max_buffer_size: 128 * 1024 * 1024, // 128MB default
            supports_compute: false,
            supports_f16: false,
        }
    }
}

/// GPU buffer handle
#[derive(Debug, Clone)]
pub struct GpuBuffer {
    /// Buffer ID
    pub id: u64,
    /// Size in bytes
    pub size: u64,
    /// Usage flags
    pub usage: BufferUsage,
}

impl GpuBuffer {
    /// Create a new buffer handle
    pub fn new(size: u64, usage: BufferUsage) -> Self {
        Self {
            id: BUFFER_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
            size,
            usage,
        }
    }
}

/// Buffer usage flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferUsage {
    /// Storage buffer (read-write)
    Storage,
    /// Uniform buffer (read-only)
    Uniform,
    /// Staging buffer (for transfers)
    Staging,
    /// Vertex buffer
    Vertex,
    /// Index buffer
    Index,
}

/// GPU compute pipeline
pub struct ComputePipeline {
    /// Pipeline ID
    pub id: u64,
    /// Shader name
    pub shader_name: String,
    /// Workgroup size
    pub workgroup_size: [u32; 3],
}

impl ComputePipeline {
    /// Create a new pipeline handle
    pub fn new(shader_name: String, workgroup_size: [u32; 3]) -> Self {
        Self {
            id: PIPELINE_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
            shader_name,
            workgroup_size,
        }
    }
}

/// GPU Backend trait - unified interface for all GPU operations
pub trait GpuBackend: Send + Sync {
    /// Check if GPU is available
    fn is_available(&self) -> bool;

    /// Get device information
    fn device_info(&self) -> GpuInfo;

    /// Get memory statistics
    fn memory_stats(&self) -> GpuMemoryStats;

    /// Create a buffer
    fn create_buffer(&self, size: u64, usage: BufferUsage) -> Result<GpuBuffer>;

    /// Write data to buffer
    fn write_buffer(&self, buffer: &GpuBuffer, data: &[u8]) -> Result<()>;

    /// Read data from buffer
    fn read_buffer(&self, buffer: &GpuBuffer, size: u64) -> Result<Vec<u8>>;

    /// Create compute pipeline from shader
    fn create_pipeline(
        &self,
        shader_source: &str,
        entry_point: &str,
        workgroup_size: [u32; 3],
    ) -> Result<ComputePipeline>;

    /// Execute compute pipeline
    fn dispatch(
        &self,
        pipeline: &ComputePipeline,
        bindings: &[&GpuBuffer],
        workgroups: [u32; 3],
    ) -> Result<()>;

    /// Synchronize GPU operations
    fn sync(&self) -> Result<()>;

    /// Release buffer
    fn release_buffer(&self, buffer: GpuBuffer) -> Result<()>;

    /// Release pipeline
    fn release_pipeline(&self, pipeline: ComputePipeline) -> Result<()>;
}

/// GPU Device wrapper with lifetime management
pub struct GpuDevice {
    backend: Arc<dyn GpuBackend>,
    config: GpuConfig,
}

impl GpuDevice {
    /// Create new GPU device
    pub fn new(backend: Arc<dyn GpuBackend>, config: GpuConfig) -> Self {
        Self { backend, config }
    }

    /// Get backend reference
    pub fn backend(&self) -> &dyn GpuBackend {
        self.backend.as_ref()
    }

    /// Get config
    pub fn config(&self) -> &GpuConfig {
        &self.config
    }
}

// ==================== CPU Backend ====================

/// CPU fallback backend
pub struct CpuBackend;

impl GpuBackend for CpuBackend {
    fn is_available(&self) -> bool {
        true // CPU always available
    }

    fn device_info(&self) -> GpuInfo {
        GpuInfo {
            name: "CPU Fallback".to_string(),
            vendor: "N/A".to_string(),
            backend: "CPU".to_string(),
            supports_compute: false,
            ..Default::default()
        }
    }

    fn memory_stats(&self) -> GpuMemoryStats {
        GpuMemoryStats::default()
    }

    fn create_buffer(&self, size: u64, usage: BufferUsage) -> Result<GpuBuffer> {
        Ok(GpuBuffer::new(size, usage))
    }

    fn write_buffer(&self, _buffer: &GpuBuffer, _data: &[u8]) -> Result<()> {
        Ok(()) // No-op for CPU
    }

    fn read_buffer(&self, _buffer: &GpuBuffer, size: u64) -> Result<Vec<u8>> {
        Ok(vec![0u8; size as usize])
    }

    fn create_pipeline(
        &self,
        _shader_source: &str,
        entry_point: &str,
        workgroup_size: [u32; 3],
    ) -> Result<ComputePipeline> {
        Ok(ComputePipeline::new(entry_point.to_string(), workgroup_size))
    }

    fn dispatch(
        &self,
        _pipeline: &ComputePipeline,
        _bindings: &[&GpuBuffer],
        _workgroups: [u32; 3],
    ) -> Result<()> {
        Ok(()) // No-op for CPU
    }

    fn sync(&self) -> Result<()> {
        Ok(())
    }

    fn release_buffer(&self, _buffer: GpuBuffer) -> Result<()> {
        Ok(())
    }

    fn release_pipeline(&self, _pipeline: ComputePipeline) -> Result<()> {
        Ok(())
    }
}

// ==================== WebGPU Backend ====================

#[cfg(feature = "gpu")]
use wgpu;

#[cfg(feature = "cuda-wasm")]
use bytemuck;

/// WebGPU backend (via wgpu) with proper buffer management
#[cfg(feature = "gpu")]
pub struct WebGpuBackend {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_info: wgpu::AdapterInfo,
    /// Active buffers indexed by buffer ID
    buffers: Mutex<HashMap<u64, wgpu::Buffer>>,
    /// Active pipelines indexed by pipeline ID
    pipelines: Mutex<HashMap<u64, wgpu::ComputePipeline>>,
    /// Bind group layouts for compute pipelines
    bind_group_layouts: Mutex<HashMap<u64, wgpu::BindGroupLayout>>,
}

#[cfg(feature = "gpu")]
impl WebGpuBackend {
    /// Create new WebGPU backend
    pub async fn new(config: &GpuConfig) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let power_pref = match config.power_preference {
            PowerPreference::LowPower => wgpu::PowerPreference::LowPower,
            PowerPreference::HighPerformance => wgpu::PowerPreference::HighPerformance,
            PowerPreference::None => wgpu::PowerPreference::None,
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: power_pref,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| EmbeddingError::GpuNotAvailable {
                reason: "No GPU adapter found".to_string(),
            })?;

        let adapter_info = adapter.get_info();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("RuVector GPU"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| EmbeddingError::GpuInitFailed {
                reason: format!("Failed to create device: {}", e),
            })?;

        Ok(Self {
            device,
            queue,
            adapter_info,
            buffers: Mutex::new(HashMap::new()),
            pipelines: Mutex::new(HashMap::new()),
            bind_group_layouts: Mutex::new(HashMap::new()),
        })
    }

    /// Convert BufferUsage to wgpu::BufferUsages
    fn to_wgpu_usage(usage: BufferUsage) -> wgpu::BufferUsages {
        match usage {
            BufferUsage::Storage => {
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC
            }
            BufferUsage::Uniform => {
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
            }
            BufferUsage::Staging => {
                wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST
            }
            BufferUsage::Vertex => {
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST
            }
            BufferUsage::Index => {
                wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST
            }
        }
    }
}

#[cfg(feature = "gpu")]
impl GpuBackend for WebGpuBackend {
    fn is_available(&self) -> bool {
        true
    }

    fn device_info(&self) -> GpuInfo {
        GpuInfo {
            name: self.adapter_info.name.clone(),
            vendor: format!("{:?}", self.adapter_info.vendor),
            backend: format!("{:?}", self.adapter_info.backend),
            api_version: "WebGPU".to_string(),
            driver_version: self.adapter_info.driver.clone(),
            total_memory: 0, // WebGPU doesn't expose this directly
            max_workgroup_size: self.device.limits().max_compute_workgroup_size_x,
            max_buffer_size: self.device.limits().max_storage_buffer_binding_size as u64,
            supports_compute: true,
            supports_f16: self.device.features().contains(wgpu::Features::SHADER_F16),
        }
    }

    fn memory_stats(&self) -> GpuMemoryStats {
        let buffers = self.buffers.lock().unwrap();
        let total_allocated: u64 = buffers.values().map(|b| b.size()).sum();
        GpuMemoryStats {
            total: total_allocated,
            used: total_allocated,
            free: 0, // WebGPU doesn't expose this
            peak: total_allocated,
        }
    }

    fn create_buffer(&self, size: u64, usage: BufferUsage) -> Result<GpuBuffer> {
        let handle = GpuBuffer::new(size, usage);

        let wgpu_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("RuVector Buffer {}", handle.id)),
            size,
            usage: Self::to_wgpu_usage(usage),
            mapped_at_creation: false,
        });

        self.buffers.lock().unwrap().insert(handle.id, wgpu_buffer);

        Ok(handle)
    }

    fn write_buffer(&self, buffer: &GpuBuffer, data: &[u8]) -> Result<()> {
        let buffers = self.buffers.lock().unwrap();
        let wgpu_buffer = buffers.get(&buffer.id).ok_or_else(|| {
            EmbeddingError::GpuBufferError {
                reason: format!("Buffer {} not found", buffer.id),
            }
        })?;

        self.queue.write_buffer(wgpu_buffer, 0, data);
        Ok(())
    }

    fn read_buffer(&self, buffer: &GpuBuffer, size: u64) -> Result<Vec<u8>> {
        let buffers = self.buffers.lock().unwrap();
        let wgpu_buffer = buffers.get(&buffer.id).ok_or_else(|| {
            EmbeddingError::GpuBufferError {
                reason: format!("Buffer {} not found", buffer.id),
            }
        })?;

        // Create staging buffer for reading
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Read Buffer"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Copy from GPU buffer to staging buffer
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Read Buffer Encoder"),
        });
        encoder.copy_buffer_to_buffer(wgpu_buffer, 0, &staging_buffer, 0, size);
        self.queue.submit(std::iter::once(encoder.finish()));

        // Map and read the staging buffer
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        self.device.poll(wgpu::Maintain::Wait);

        rx.recv()
            .map_err(|e| EmbeddingError::GpuOperationFailed {
                operation: "read_buffer".to_string(),
                reason: format!("Channel error: {}", e),
            })?
            .map_err(|e| EmbeddingError::GpuOperationFailed {
                operation: "read_buffer".to_string(),
                reason: format!("Buffer map failed: {:?}", e),
            })?;

        let data = buffer_slice.get_mapped_range();
        let result = data.to_vec();
        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }

    fn create_pipeline(
        &self,
        shader_source: &str,
        entry_point: &str,
        workgroup_size: [u32; 3],
    ) -> Result<ComputePipeline> {
        let handle = ComputePipeline::new(entry_point.to_string(), workgroup_size);

        // Create shader module
        let shader_module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("Shader: {}", entry_point)),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout for storage buffers + uniform params
        // Layout: binding 0-2 are storage, binding 3 is uniform params
        let bind_group_layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(&format!("BindGroupLayout: {}", entry_point)),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("PipelineLayout: {}", entry_point)),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(&format!("Pipeline: {}", entry_point)),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some(entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        self.pipelines.lock().unwrap().insert(handle.id, compute_pipeline);
        self.bind_group_layouts.lock().unwrap().insert(handle.id, bind_group_layout);

        Ok(handle)
    }

    fn dispatch(
        &self,
        pipeline: &ComputePipeline,
        bindings: &[&GpuBuffer],
        workgroups: [u32; 3],
    ) -> Result<()> {
        let pipelines = self.pipelines.lock().unwrap();
        let layouts = self.bind_group_layouts.lock().unwrap();
        let buffers = self.buffers.lock().unwrap();

        let compute_pipeline = pipelines.get(&pipeline.id).ok_or_else(|| {
            EmbeddingError::GpuOperationFailed {
                operation: "dispatch".to_string(),
                reason: format!("Pipeline {} not found", pipeline.id),
            }
        })?;

        let bind_group_layout = layouts.get(&pipeline.id).ok_or_else(|| {
            EmbeddingError::GpuOperationFailed {
                operation: "dispatch".to_string(),
                reason: format!("BindGroupLayout for pipeline {} not found", pipeline.id),
            }
        })?;

        // Build bind group entries
        let mut bind_group_entries = Vec::new();
        for (i, buf_handle) in bindings.iter().enumerate() {
            let wgpu_buffer = buffers.get(&buf_handle.id).ok_or_else(|| {
                EmbeddingError::GpuBufferError {
                    reason: format!("Buffer {} not found", buf_handle.id),
                }
            })?;
            bind_group_entries.push(wgpu::BindGroupEntry {
                binding: i as u32,
                resource: wgpu_buffer.as_entire_binding(),
            });
        }

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute BindGroup"),
            layout: bind_group_layout,
            entries: &bind_group_entries,
        });

        // Create command encoder and dispatch
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Compute Encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(compute_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(workgroups[0], workgroups[1], workgroups[2]);
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    fn sync(&self) -> Result<()> {
        self.device.poll(wgpu::Maintain::Wait);
        Ok(())
    }

    fn release_buffer(&self, buffer: GpuBuffer) -> Result<()> {
        self.buffers.lock().unwrap().remove(&buffer.id);
        Ok(())
    }

    fn release_pipeline(&self, pipeline: ComputePipeline) -> Result<()> {
        self.pipelines.lock().unwrap().remove(&pipeline.id);
        self.bind_group_layouts.lock().unwrap().remove(&pipeline.id);
        Ok(())
    }
}

// ==================== CUDA-WASM Backend ====================

/// CUDA-WASM backend using WebAssembly for compute
///
/// This backend transpiles CUDA-like kernels to WebAssembly for portable
/// GPU-like compute across platforms. It provides:
/// - SIMD-accelerated operations via WASM SIMD128
/// - Parallel execution via rayon
/// - Memory-mapped buffers for efficient data transfer
///
/// Architecture:
/// - Kernels are defined as Rust functions compiled to WASM
/// - Buffer management tracks allocations in a HashMap
/// - Dispatch executes kernels with workgroup-like parallelism
#[cfg(feature = "cuda-wasm")]
pub struct CudaWasmBackend {
    /// Buffer storage (simulates device memory)
    buffers: Mutex<HashMap<u64, Vec<u8>>>,
    /// Compiled kernel cache
    kernels: Mutex<HashMap<String, CudaWasmKernel>>,
    /// Device info
    device_info: GpuInfo,
    /// Memory statistics
    memory_stats: Mutex<CudaWasmMemoryStats>,
}

#[cfg(feature = "cuda-wasm")]
struct CudaWasmKernel {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    workgroup_size: [u32; 3],
    // Entry point function pointer
    entry_point: fn(&[&[u8]], &mut [u8], &CudaWasmParams),
}

#[cfg(feature = "cuda-wasm")]
#[derive(Default)]
struct CudaWasmMemoryStats {
    allocated: u64,
    peak: u64,
}

#[cfg(feature = "cuda-wasm")]
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CudaWasmParams {
    pub workgroups: [u32; 3],
    pub workgroup_size: [u32; 3],
}

#[cfg(feature = "cuda-wasm")]
impl CudaWasmBackend {
    /// Create new CUDA-WASM backend
    pub async fn new(config: &GpuConfig) -> Result<Self> {
        // Check if WASM SIMD is available (always true for now - fallback is scalar)
        let supports_simd = cfg!(target_feature = "simd128");

        let device_info = GpuInfo {
            name: "CUDA-WASM Compute".to_string(),
            vendor: "RuVector".to_string(),
            backend: "CUDA-WASM".to_string(),
            api_version: "1.0".to_string(),
            driver_version: env!("CARGO_PKG_VERSION").to_string(),
            total_memory: config.max_memory * 1024 * 1024,
            max_workgroup_size: 256,
            max_buffer_size: config.max_memory * 1024 * 1024,
            supports_compute: true,
            supports_f16: false,
        };

        // Log SIMD availability (we still work without it via scalar fallback)
        if !supports_simd {
            tracing::debug!("WASM SIMD not available, using scalar fallback");
        }

        Ok(Self {
            buffers: Mutex::new(HashMap::new()),
            kernels: Mutex::new(HashMap::new()),
            device_info,
            memory_stats: Mutex::new(CudaWasmMemoryStats::default()),
        })
    }

    /// Register built-in CUDA-WASM kernels
    fn register_builtin_kernels(&self) {
        let mut kernels = self.kernels.lock().unwrap();

        // Batch cosine similarity kernel
        kernels.insert("batch_cosine_similarity".to_string(), CudaWasmKernel {
            name: "batch_cosine_similarity".to_string(),
            workgroup_size: [256, 1, 1],
            entry_point: Self::kernel_batch_cosine_similarity,
        });

        // Dot product kernel
        kernels.insert("dot_product".to_string(), CudaWasmKernel {
            name: "dot_product".to_string(),
            workgroup_size: [256, 1, 1],
            entry_point: Self::kernel_dot_product,
        });

        // Mean pooling kernel
        kernels.insert("mean_pool".to_string(), CudaWasmKernel {
            name: "mean_pool".to_string(),
            workgroup_size: [64, 1, 1],
            entry_point: Self::kernel_mean_pool,
        });

        // Euclidean distance kernel
        kernels.insert("euclidean_distance".to_string(), CudaWasmKernel {
            name: "euclidean_distance".to_string(),
            workgroup_size: [256, 1, 1],
            entry_point: Self::kernel_euclidean_distance,
        });

        // L2 normalize kernel
        kernels.insert("l2_normalize".to_string(), CudaWasmKernel {
            name: "l2_normalize".to_string(),
            workgroup_size: [256, 1, 1],
            entry_point: Self::kernel_l2_normalize,
        });

        // Max pooling kernel
        kernels.insert("max_pool".to_string(), CudaWasmKernel {
            name: "max_pool".to_string(),
            workgroup_size: [64, 1, 1],
            entry_point: Self::kernel_max_pool,
        });

        // Matrix-vector multiplication kernel
        kernels.insert("matmul".to_string(), CudaWasmKernel {
            name: "matmul".to_string(),
            workgroup_size: [16, 16, 1],
            entry_point: Self::kernel_matmul,
        });

        // Vector addition kernel
        kernels.insert("vector_add".to_string(), CudaWasmKernel {
            name: "vector_add".to_string(),
            workgroup_size: [256, 1, 1],
            entry_point: Self::kernel_vector_add,
        });
    }

    // ==================== Built-in Kernels ====================

    fn kernel_batch_cosine_similarity(inputs: &[&[u8]], output: &mut [u8], _params: &CudaWasmParams) {
        // Parse params from first input (uniform buffer)
        if inputs.len() < 4 || inputs[3].len() < 8 {
            return;
        }

        let dimension = u32::from_le_bytes(inputs[3][0..4].try_into().unwrap_or([0; 4])) as usize;
        let num_candidates = u32::from_le_bytes(inputs[3][4..8].try_into().unwrap_or([0; 4])) as usize;

        if dimension == 0 || num_candidates == 0 {
            return;
        }

        let query: &[f32] = bytemuck::cast_slice(inputs[0]);
        let candidates: &[f32] = bytemuck::cast_slice(inputs[1]);
        let results: &mut [f32] = bytemuck::cast_slice_mut(output);

        // Process each candidate in parallel
        use rayon::prelude::*;
        results.par_iter_mut().enumerate().take(num_candidates).for_each(|(idx, result)| {
            let base = idx * dimension;
            if base + dimension > candidates.len() {
                *result = 0.0;
                return;
            }

            let mut dot = 0.0f32;
            let mut norm_a = 0.0f32;
            let mut norm_b = 0.0f32;

            for i in 0..dimension.min(query.len()) {
                let a = query[i];
                let b = candidates[base + i];
                dot += a * b;
                norm_a += a * a;
                norm_b += b * b;
            }

            let norm_product = (norm_a * norm_b).sqrt();
            *result = if norm_product > 1e-12 { dot / norm_product } else { 0.0 };
        });
    }

    fn kernel_dot_product(inputs: &[&[u8]], output: &mut [u8], _params: &CudaWasmParams) {
        if inputs.len() < 4 || inputs[3].len() < 8 {
            return;
        }

        let dimension = u32::from_le_bytes(inputs[3][0..4].try_into().unwrap_or([0; 4])) as usize;
        let num_candidates = u32::from_le_bytes(inputs[3][4..8].try_into().unwrap_or([0; 4])) as usize;

        if dimension == 0 || num_candidates == 0 {
            return;
        }

        let query: &[f32] = bytemuck::cast_slice(inputs[0]);
        let candidates: &[f32] = bytemuck::cast_slice(inputs[1]);
        let results: &mut [f32] = bytemuck::cast_slice_mut(output);

        use rayon::prelude::*;
        results.par_iter_mut().enumerate().take(num_candidates).for_each(|(idx, result)| {
            let base = idx * dimension;
            if base + dimension > candidates.len() {
                *result = 0.0;
                return;
            }

            *result = (0..dimension.min(query.len()))
                .map(|i| query[i] * candidates[base + i])
                .sum();
        });
    }

    fn kernel_mean_pool(inputs: &[&[u8]], output: &mut [u8], _params: &CudaWasmParams) {
        if inputs.len() < 4 || inputs[3].len() < 12 {
            return;
        }

        let batch_size = u32::from_le_bytes(inputs[3][0..4].try_into().unwrap_or([0; 4])) as usize;
        let seq_length = u32::from_le_bytes(inputs[3][4..8].try_into().unwrap_or([0; 4])) as usize;
        let hidden_size = u32::from_le_bytes(inputs[3][8..12].try_into().unwrap_or([0; 4])) as usize;

        if batch_size == 0 || seq_length == 0 || hidden_size == 0 {
            return;
        }

        let tokens: &[f32] = bytemuck::cast_slice(inputs[0]);
        let attention_mask: &[i64] = bytemuck::cast_slice(inputs[1]);
        let results: &mut [f32] = bytemuck::cast_slice_mut(output);

        use rayon::prelude::*;
        results.par_chunks_mut(hidden_size).enumerate().take(batch_size).for_each(|(batch_idx, out_chunk)| {
            let tokens_base = batch_idx * seq_length * hidden_size;
            let mask_base = batch_idx * seq_length;

            out_chunk.fill(0.0);
            let mut count = 0.0f32;

            for seq_idx in 0..seq_length {
                if mask_base + seq_idx < attention_mask.len() && attention_mask[mask_base + seq_idx] == 1 {
                    let start = tokens_base + seq_idx * hidden_size;
                    for (j, out_val) in out_chunk.iter_mut().enumerate() {
                        if start + j < tokens.len() {
                            *out_val += tokens[start + j];
                        }
                    }
                    count += 1.0;
                }
            }

            if count > 0.0 {
                for val in out_chunk.iter_mut() {
                    *val /= count;
                }
            }
        });
    }

    fn kernel_euclidean_distance(inputs: &[&[u8]], output: &mut [u8], _params: &CudaWasmParams) {
        if inputs.len() < 4 || inputs[3].len() < 8 {
            return;
        }

        let dimension = u32::from_le_bytes(inputs[3][0..4].try_into().unwrap_or([0; 4])) as usize;
        let num_candidates = u32::from_le_bytes(inputs[3][4..8].try_into().unwrap_or([0; 4])) as usize;

        if dimension == 0 || num_candidates == 0 {
            return;
        }

        let query: &[f32] = bytemuck::cast_slice(inputs[0]);
        let candidates: &[f32] = bytemuck::cast_slice(inputs[1]);
        let results: &mut [f32] = bytemuck::cast_slice_mut(output);

        use rayon::prelude::*;
        results.par_iter_mut().enumerate().take(num_candidates).for_each(|(idx, result)| {
            let base = idx * dimension;
            if base + dimension > candidates.len() {
                *result = 0.0;
                return;
            }

            let sum_sq: f32 = (0..dimension.min(query.len()))
                .map(|i| {
                    let diff = query[i] - candidates[base + i];
                    diff * diff
                })
                .sum();

            *result = sum_sq.sqrt();
        });
    }

    fn kernel_l2_normalize(inputs: &[&[u8]], output: &mut [u8], _params: &CudaWasmParams) {
        if inputs.len() < 4 || inputs[3].len() < 8 {
            return;
        }

        let dimension = u32::from_le_bytes(inputs[3][0..4].try_into().unwrap_or([0; 4])) as usize;
        let num_vectors = u32::from_le_bytes(inputs[3][4..8].try_into().unwrap_or([0; 4])) as usize;

        if dimension == 0 || num_vectors == 0 {
            return;
        }

        let input_vectors: &[f32] = bytemuck::cast_slice(inputs[0]);
        let output_vectors: &mut [f32] = bytemuck::cast_slice_mut(output);

        use rayon::prelude::*;
        output_vectors.par_chunks_mut(dimension).enumerate().take(num_vectors).for_each(|(vec_idx, out_chunk)| {
            let base = vec_idx * dimension;
            if base + dimension > input_vectors.len() {
                return;
            }

            // Compute norm
            let norm_sq: f32 = (0..dimension)
                .map(|i| {
                    let val = input_vectors[base + i];
                    val * val
                })
                .sum();

            let norm = norm_sq.sqrt();

            // Normalize
            if norm > 1e-12 {
                for (i, out_val) in out_chunk.iter_mut().enumerate() {
                    *out_val = input_vectors[base + i] / norm;
                }
            } else {
                for (i, out_val) in out_chunk.iter_mut().enumerate() {
                    *out_val = input_vectors[base + i];
                }
            }
        });
    }

    fn kernel_max_pool(inputs: &[&[u8]], output: &mut [u8], _params: &CudaWasmParams) {
        if inputs.len() < 4 || inputs[3].len() < 12 {
            return;
        }

        let batch_size = u32::from_le_bytes(inputs[3][0..4].try_into().unwrap_or([0; 4])) as usize;
        let seq_length = u32::from_le_bytes(inputs[3][4..8].try_into().unwrap_or([0; 4])) as usize;
        let hidden_size = u32::from_le_bytes(inputs[3][8..12].try_into().unwrap_or([0; 4])) as usize;

        if batch_size == 0 || seq_length == 0 || hidden_size == 0 {
            return;
        }

        let tokens: &[f32] = bytemuck::cast_slice(inputs[0]);
        let attention_mask: &[i64] = bytemuck::cast_slice(inputs[1]);
        let results: &mut [f32] = bytemuck::cast_slice_mut(output);

        use rayon::prelude::*;
        results.par_chunks_mut(hidden_size).enumerate().take(batch_size).for_each(|(batch_idx, out_chunk)| {
            let tokens_base = batch_idx * seq_length * hidden_size;
            let mask_base = batch_idx * seq_length;

            out_chunk.fill(f32::NEG_INFINITY);
            let mut found = false;

            for seq_idx in 0..seq_length {
                if mask_base + seq_idx < attention_mask.len() && attention_mask[mask_base + seq_idx] == 1 {
                    let start = tokens_base + seq_idx * hidden_size;
                    for (j, out_val) in out_chunk.iter_mut().enumerate() {
                        if start + j < tokens.len() {
                            let val = tokens[start + j];
                            if !found || val > *out_val {
                                *out_val = val;
                            }
                        }
                    }
                    found = true;
                }
            }

            // Replace -inf with 0 if no tokens found
            if !found {
                out_chunk.fill(0.0);
            }
        });
    }

    fn kernel_matmul(inputs: &[&[u8]], output: &mut [u8], _params: &CudaWasmParams) {
        if inputs.len() < 4 || inputs[3].len() < 8 {
            return;
        }

        let rows = u32::from_le_bytes(inputs[3][0..4].try_into().unwrap_or([0; 4])) as usize;
        let cols = u32::from_le_bytes(inputs[3][4..8].try_into().unwrap_or([0; 4])) as usize;

        if rows == 0 || cols == 0 {
            return;
        }

        let matrix: &[f32] = bytemuck::cast_slice(inputs[0]);
        let vector: &[f32] = bytemuck::cast_slice(inputs[1]);
        let results: &mut [f32] = bytemuck::cast_slice_mut(output);

        use rayon::prelude::*;
        results.par_iter_mut().enumerate().take(rows).for_each(|(row, result)| {
            let row_start = row * cols;
            if row_start + cols > matrix.len() || cols > vector.len() {
                *result = 0.0;
                return;
            }

            *result = (0..cols)
                .map(|col| matrix[row_start + col] * vector[col])
                .sum();
        });
    }

    fn kernel_vector_add(inputs: &[&[u8]], output: &mut [u8], _params: &CudaWasmParams) {
        if inputs.len() < 4 || inputs[3].len() < 4 {
            return;
        }

        let length = u32::from_le_bytes(inputs[3][0..4].try_into().unwrap_or([0; 4])) as usize;

        if length == 0 {
            return;
        }

        let a: &[f32] = bytemuck::cast_slice(inputs[0]);
        let b: &[f32] = bytemuck::cast_slice(inputs[1]);
        let results: &mut [f32] = bytemuck::cast_slice_mut(output);

        use rayon::prelude::*;
        results.par_iter_mut().enumerate().take(length).for_each(|(idx, result)| {
            if idx < a.len() && idx < b.len() {
                *result = a[idx] + b[idx];
            } else {
                *result = 0.0;
            }
        });
    }
}

#[cfg(feature = "cuda-wasm")]
impl GpuBackend for CudaWasmBackend {
    fn is_available(&self) -> bool {
        true // CUDA-WASM always available as software fallback
    }

    fn device_info(&self) -> GpuInfo {
        self.device_info.clone()
    }

    fn memory_stats(&self) -> GpuMemoryStats {
        let stats = self.memory_stats.lock().unwrap();
        GpuMemoryStats {
            total: self.device_info.total_memory,
            used: stats.allocated,
            free: self.device_info.total_memory.saturating_sub(stats.allocated),
            peak: stats.peak,
        }
    }

    fn create_buffer(&self, size: u64, usage: BufferUsage) -> Result<GpuBuffer> {
        let handle = GpuBuffer::new(size, usage);

        // Allocate buffer storage
        let buffer = vec![0u8; size as usize];
        self.buffers.lock().unwrap().insert(handle.id, buffer);

        // Update memory stats
        let mut stats = self.memory_stats.lock().unwrap();
        stats.allocated += size;
        stats.peak = stats.peak.max(stats.allocated);

        Ok(handle)
    }

    fn write_buffer(&self, buffer: &GpuBuffer, data: &[u8]) -> Result<()> {
        let mut buffers = self.buffers.lock().unwrap();
        let buf = buffers.get_mut(&buffer.id).ok_or_else(|| {
            EmbeddingError::GpuBufferError {
                reason: format!("Buffer {} not found", buffer.id),
            }
        })?;

        let len = data.len().min(buf.len());
        buf[..len].copy_from_slice(&data[..len]);
        Ok(())
    }

    fn read_buffer(&self, buffer: &GpuBuffer, size: u64) -> Result<Vec<u8>> {
        let buffers = self.buffers.lock().unwrap();
        let buf = buffers.get(&buffer.id).ok_or_else(|| {
            EmbeddingError::GpuBufferError {
                reason: format!("Buffer {} not found", buffer.id),
            }
        })?;

        let len = (size as usize).min(buf.len());
        Ok(buf[..len].to_vec())
    }

    fn create_pipeline(
        &self,
        _shader_source: &str,
        entry_point: &str,
        workgroup_size: [u32; 3],
    ) -> Result<ComputePipeline> {
        // Register built-in kernels if not already done
        if self.kernels.lock().unwrap().is_empty() {
            self.register_builtin_kernels();
        }

        Ok(ComputePipeline::new(entry_point.to_string(), workgroup_size))
    }

    fn dispatch(
        &self,
        pipeline: &ComputePipeline,
        bindings: &[&GpuBuffer],
        workgroups: [u32; 3],
    ) -> Result<()> {
        // Get kernel entry point
        let entry_point = {
            let kernels = self.kernels.lock().unwrap();
            let kernel = kernels.get(&pipeline.shader_name).ok_or_else(|| {
                EmbeddingError::GpuOperationFailed {
                    operation: "dispatch".to_string(),
                    reason: format!("Kernel '{}' not found", pipeline.shader_name),
                }
            })?;
            kernel.entry_point
        };

        // Get output buffer id (binding 2)
        let output_id = if bindings.len() > 2 { bindings[2].id } else { return Ok(()); };

        // Clone input buffers for kernel execution
        let (input_copies, output_size): (Vec<Vec<u8>>, usize) = {
            let buffers = self.buffers.lock().unwrap();

            // Verify all buffers exist
            for (i, buf_handle) in bindings.iter().enumerate() {
                if !buffers.contains_key(&buf_handle.id) {
                    return Err(EmbeddingError::GpuBufferError {
                        reason: format!("Buffer {} not found at binding {}", buf_handle.id, i),
                    });
                }
            }

            let copies: Vec<Vec<u8>> = bindings.iter()
                .map(|b| buffers.get(&b.id).cloned().unwrap_or_default())
                .collect();

            let out_size = buffers.get(&output_id).map(|v| v.len()).unwrap_or(0);

            (copies, out_size)
        };

        // Execute kernel with copied buffers
        let params = CudaWasmParams {
            workgroups,
            workgroup_size: pipeline.workgroup_size,
        };

        let input_refs: Vec<&[u8]> = input_copies.iter().map(|v| v.as_slice()).collect();
        let mut temp_output = vec![0u8; output_size];

        entry_point(&input_refs, &mut temp_output, &params);

        // Write output back
        {
            let mut buffers = self.buffers.lock().unwrap();
            if let Some(out) = buffers.get_mut(&output_id) {
                out.copy_from_slice(&temp_output);
            }
        }

        Ok(())
    }

    fn sync(&self) -> Result<()> {
        // CUDA-WASM executes synchronously, no-op
        Ok(())
    }

    fn release_buffer(&self, buffer: GpuBuffer) -> Result<()> {
        let mut buffers = self.buffers.lock().unwrap();
        if let Some(buf) = buffers.remove(&buffer.id) {
            let mut stats = self.memory_stats.lock().unwrap();
            stats.allocated = stats.allocated.saturating_sub(buf.len() as u64);
        }
        Ok(())
    }

    fn release_pipeline(&self, _pipeline: ComputePipeline) -> Result<()> {
        // Kernels are cached, no cleanup needed
        Ok(())
    }
}

// ==================== Factory Functions ====================

/// Create appropriate backend based on configuration
pub async fn create_backend(config: &GpuConfig) -> Result<Box<dyn GpuBackend>> {
    match config.mode {
        GpuMode::CpuOnly => {
            Ok(Box::new(CpuBackend))
        }
        #[cfg(feature = "gpu")]
        GpuMode::WebGpu => {
            match WebGpuBackend::new(config).await {
                Ok(backend) => Ok(Box::new(backend)),
                Err(e) if config.fallback_to_cpu => {
                    tracing::warn!("WebGPU not available, falling back to CPU: {}", e);
                    Ok(Box::new(CpuBackend))
                }
                Err(e) => Err(e),
            }
        }
        #[cfg(feature = "cuda-wasm")]
        GpuMode::CudaWasm => {
            match CudaWasmBackend::new(config).await {
                Ok(backend) => Ok(Box::new(backend)),
                Err(e) if config.fallback_to_cpu => {
                    tracing::warn!("CUDA-WASM not available, falling back to CPU: {}", e);
                    Ok(Box::new(CpuBackend))
                }
                Err(e) => Err(e),
            }
        }
        GpuMode::Auto => {
            #[cfg(feature = "gpu")]
            {
                if let Ok(backend) = WebGpuBackend::new(config).await {
                    return Ok(Box::new(backend));
                }
            }
            #[cfg(feature = "cuda-wasm")]
            {
                if let Ok(backend) = CudaWasmBackend::new(config).await {
                    return Ok(Box::new(backend));
                }
            }
            Ok(Box::new(CpuBackend))
        }
        #[allow(unreachable_patterns)]
        _ => Ok(Box::new(CpuBackend)),
    }
}

/// Probe GPU availability without full initialization
pub async fn probe_gpu() -> bool {
    #[cfg(feature = "gpu")]
    {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .is_some()
    }
    #[cfg(not(feature = "gpu"))]
    {
        false
    }
}

/// Get GPU info without full backend creation
pub async fn get_device_info() -> Option<GpuInfo> {
    #[cfg(feature = "gpu")]
    {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await?;

        let info = adapter.get_info();
        Some(GpuInfo {
            name: info.name,
            vendor: format!("{:?}", info.vendor),
            backend: format!("{:?}", info.backend),
            api_version: "WebGPU".to_string(),
            driver_version: info.driver,
            supports_compute: true,
            ..Default::default()
        })
    }
    #[cfg(not(feature = "gpu"))]
    {
        None
    }
}

