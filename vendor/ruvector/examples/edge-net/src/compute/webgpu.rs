//! WebGPU Compute Backend Implementation
//!
//! This module provides GPU-accelerated compute operations using wgpu.
//! It includes optimized pipelines for matrix multiplication, attention,
//! and LoRA adapter inference.

use std::sync::Arc;
use std::collections::HashMap;

use super::{
    ComputeConfig, ComputeError, ComputeMetrics,
    TensorDescriptor, DataType, LoraConfig, AttentionConfig,
    BufferUsage, MATMUL_SHADER, ATTENTION_SHADER, LORA_SHADER,
};

/// Buffer handle for GPU memory
#[derive(Clone)]
pub struct GpuBuffer {
    /// Underlying wgpu buffer
    buffer: Arc<wgpu::Buffer>,
    /// Size in bytes
    size: usize,
    /// Tensor descriptor
    desc: TensorDescriptor,
}

impl GpuBuffer {
    /// Get buffer size in bytes
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get tensor descriptor
    pub fn descriptor(&self) -> &TensorDescriptor {
        &self.desc
    }

    /// Get underlying wgpu buffer
    pub fn raw(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

/// Compute pipeline for a specific operation
struct ComputePipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

/// WebGPU compute backend for GPU-accelerated inference
pub struct WebGpuCompute {
    /// GPU device handle
    device: Arc<wgpu::Device>,
    /// Command queue
    queue: Arc<wgpu::Queue>,
    /// Backend configuration
    config: ComputeConfig,
    /// Matrix multiplication pipeline
    matmul_pipeline: ComputePipeline,
    /// Attention pipeline
    attention_pipeline: ComputePipeline,
    /// LoRA forward pipeline
    lora_pipeline: ComputePipeline,
    /// Staging buffer pool for CPU<->GPU transfers
    staging_pool: StagingBufferPool,
    /// Performance metrics from last operation
    last_metrics: ComputeMetrics,
    /// Device limits
    limits: wgpu::Limits,
}

impl WebGpuCompute {
    /// Create a new WebGPU compute backend
    pub async fn new() -> Result<Self, ComputeError> {
        Self::with_config(ComputeConfig::default()).await
    }

    /// Create with custom configuration
    pub async fn with_config(config: ComputeConfig) -> Result<Self, ComputeError> {
        // Request adapter
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
            flags: wgpu::InstanceFlags::empty(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| ComputeError::DeviceNotAvailable(
                "No suitable GPU adapter found".to_string()
            ))?;

        let limits = adapter.limits();

        // Request device with compute capabilities
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("edge-net-compute"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| ComputeError::DeviceNotAvailable(e.to_string()))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Create compute pipelines
        let matmul_pipeline = Self::create_matmul_pipeline(&device, &config)?;
        let attention_pipeline = Self::create_attention_pipeline(&device, &config)?;
        let lora_pipeline = Self::create_lora_pipeline(&device, &config)?;

        // Create staging buffer pool
        let staging_pool = StagingBufferPool::new(device.clone(), 16 * 1024 * 1024); // 16MB pool

        Ok(Self {
            device,
            queue,
            config,
            matmul_pipeline,
            attention_pipeline,
            lora_pipeline,
            staging_pool,
            last_metrics: ComputeMetrics::default(),
            limits,
        })
    }

    /// Create matrix multiplication pipeline
    fn create_matmul_pipeline(
        device: &wgpu::Device,
        config: &ComputeConfig,
    ) -> Result<ComputePipeline, ComputeError> {
        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("matmul_shader"),
            source: wgpu::ShaderSource::Wgsl(MATMUL_SHADER.into()),
        });

        // Create bind group layout
        // Bindings: 0=A matrix, 1=B matrix, 2=C matrix (output), 3=uniforms
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("matmul_bind_group_layout"),
            entries: &[
                // Matrix A (read-only storage)
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
                // Matrix B (read-only storage)
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
                // Matrix C (read-write storage)
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
                // Uniforms (dimensions)
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

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("matmul_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("matmul_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(ComputePipeline {
            pipeline,
            bind_group_layout,
        })
    }

    /// Create attention pipeline
    fn create_attention_pipeline(
        device: &wgpu::Device,
        config: &ComputeConfig,
    ) -> Result<ComputePipeline, ComputeError> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("attention_shader"),
            source: wgpu::ShaderSource::Wgsl(ATTENTION_SHADER.into()),
        });

        // Bindings: 0=Q, 1=K, 2=V, 3=Output, 4=Uniforms
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("attention_bind_group_layout"),
            entries: &[
                // Q (query)
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
                // K (key)
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
                // V (value)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Output
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("attention_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("attention_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(ComputePipeline {
            pipeline,
            bind_group_layout,
        })
    }

    /// Create LoRA forward pipeline
    fn create_lora_pipeline(
        device: &wgpu::Device,
        config: &ComputeConfig,
    ) -> Result<ComputePipeline, ComputeError> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lora_shader"),
            source: wgpu::ShaderSource::Wgsl(LORA_SHADER.into()),
        });

        // Bindings: 0=Input, 1=LoRA_A, 2=LoRA_B, 3=Output, 4=Uniforms
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lora_bind_group_layout"),
            entries: &[
                // Input
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
                // LoRA A matrix
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
                // LoRA B matrix
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Output
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lora_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("lora_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(ComputePipeline {
            pipeline,
            bind_group_layout,
        })
    }

    // ========================================================================
    // Buffer Management
    // ========================================================================

    /// Allocate a GPU buffer
    pub fn allocate_buffer(&self, desc: TensorDescriptor, usage: BufferUsage) -> Result<GpuBuffer, ComputeError> {
        let size = desc.size_bytes();

        // Check against device limits
        if size > self.limits.max_buffer_size as usize {
            return Err(ComputeError::BufferAllocationFailed {
                requested: size,
                available: self.limits.max_buffer_size as usize,
            });
        }

        let mut wgpu_usage = wgpu::BufferUsages::empty();
        if usage.map_read { wgpu_usage |= wgpu::BufferUsages::MAP_READ; }
        if usage.map_write { wgpu_usage |= wgpu::BufferUsages::MAP_WRITE; }
        if usage.copy_src { wgpu_usage |= wgpu::BufferUsages::COPY_SRC; }
        if usage.copy_dst { wgpu_usage |= wgpu::BufferUsages::COPY_DST; }
        if usage.storage { wgpu_usage |= wgpu::BufferUsages::STORAGE; }
        if usage.uniform { wgpu_usage |= wgpu::BufferUsages::UNIFORM; }

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("compute_buffer"),
            size: size as u64,
            usage: wgpu_usage,
            mapped_at_creation: false,
        });

        Ok(GpuBuffer {
            buffer: Arc::new(buffer),
            size,
            desc,
        })
    }

    /// Upload data to GPU buffer
    pub async fn upload_buffer(&self, buffer: &GpuBuffer, data: &[u8]) -> Result<(), ComputeError> {
        if data.len() != buffer.size {
            return Err(ComputeError::DimensionMismatch {
                expected: format!("{} bytes", buffer.size),
                actual: format!("{} bytes", data.len()),
            });
        }

        // Use staging buffer for upload
        let staging = self.staging_pool.get_upload_buffer(data.len())?;

        // Write to staging buffer
        self.queue.write_buffer(&staging, 0, data);

        // Copy from staging to destination
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("upload_encoder"),
        });
        encoder.copy_buffer_to_buffer(&staging, 0, buffer.raw(), 0, data.len() as u64);
        self.queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Download data from GPU buffer
    pub async fn download_buffer(&self, buffer: &GpuBuffer) -> Result<Vec<u8>, ComputeError> {
        let staging = self.staging_pool.get_download_buffer(buffer.size)?;

        // Copy from source to staging
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("download_encoder"),
        });
        encoder.copy_buffer_to_buffer(buffer.raw(), 0, &staging, 0, buffer.size as u64);
        self.queue.submit(std::iter::once(encoder.finish()));

        // Map staging buffer and read
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().map_err(|e| ComputeError::DeviceNotAvailable(e.to_string()))?;

        let data = slice.get_mapped_range().to_vec();
        staging.unmap();

        Ok(data)
    }

    // ========================================================================
    // Matrix Multiplication
    // ========================================================================

    /// Perform matrix multiplication: C = A * B
    ///
    /// Dimensions: A (M x K), B (K x N), C (M x N)
    ///
    /// Performance target: 10+ TFLOPS on discrete GPU
    pub async fn matmul(
        &mut self,
        a: &GpuBuffer,
        b: &GpuBuffer,
        c: &GpuBuffer,
        m: u32,
        n: u32,
        k: u32,
    ) -> Result<ComputeMetrics, ComputeError> {
        let start = std::time::Instant::now();

        // Validate dimensions
        let expected_a = (m as usize) * (k as usize) * 4; // f32
        let expected_b = (k as usize) * (n as usize) * 4;
        let expected_c = (m as usize) * (n as usize) * 4;

        if a.size != expected_a || b.size != expected_b || c.size != expected_c {
            return Err(ComputeError::DimensionMismatch {
                expected: format!("A:{}x{}, B:{}x{}, C:{}x{}", m, k, k, n, m, n),
                actual: format!("A:{}, B:{}, C:{} bytes", a.size, b.size, c.size),
            });
        }

        // Create uniforms buffer
        let uniforms = [m, n, k, self.config.tile_size];
        let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matmul_uniforms"),
            contents: bytemuck::cast_slice(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("matmul_bind_group"),
            layout: &self.matmul_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: a.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: b.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: c.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: uniform_buffer.as_entire_binding() },
            ],
        });

        // Dispatch compute
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("matmul_encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("matmul_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.matmul_pipeline.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch workgroups (tile-based)
            let tile_size = self.config.tile_size;
            let workgroups_x = (m + tile_size - 1) / tile_size;
            let workgroups_y = (n + tile_size - 1) / tile_size;
            pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        let kernel_start = std::time::Instant::now();
        self.queue.submit(std::iter::once(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);
        let kernel_time = kernel_start.elapsed();

        let total_time = start.elapsed();

        // Calculate metrics
        let flops = 2.0 * (m as f64) * (n as f64) * (k as f64); // 2*M*N*K for matmul
        let metrics = ComputeMetrics {
            flops,
            bandwidth_gbps: ((a.size + b.size + c.size) as f64) / kernel_time.as_secs_f64() / 1e9,
            kernel_time_ms: kernel_time.as_secs_f64() * 1000.0,
            transfer_time_ms: 0.0, // Data already on GPU
            total_time_ms: total_time.as_secs_f64() * 1000.0,
        };

        self.last_metrics = metrics.clone();
        Ok(metrics)
    }

    // ========================================================================
    // Attention
    // ========================================================================

    /// Compute attention: Output = softmax(Q * K^T / sqrt(d_k)) * V
    ///
    /// Uses flash attention algorithm for memory efficiency.
    ///
    /// Performance target: 2ms for 4K context
    pub async fn attention(
        &mut self,
        q: &GpuBuffer,
        k: &GpuBuffer,
        v: &GpuBuffer,
        output: &GpuBuffer,
        config: &AttentionConfig,
        seq_len: u32,
    ) -> Result<ComputeMetrics, ComputeError> {
        let start = std::time::Instant::now();

        // Validate dimensions
        let hidden_dim = config.hidden_dim();
        let expected_size = (seq_len as usize) * hidden_dim * 4; // f32

        if q.size != expected_size || k.size != expected_size || v.size != expected_size {
            return Err(ComputeError::DimensionMismatch {
                expected: format!("{}x{} = {} bytes", seq_len, hidden_dim, expected_size),
                actual: format!("Q:{}, K:{}, V:{} bytes", q.size, k.size, v.size),
            });
        }

        // Create uniforms buffer
        let scale = config.get_scale();
        let causal_mask = if config.causal { 1u32 } else { 0u32 };
        let uniforms: [f32; 8] = [
            seq_len as f32,
            config.head_dim as f32,
            config.num_heads as f32,
            scale,
            causal_mask as f32,
            0.0, 0.0, 0.0, // padding
        ];
        let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("attention_uniforms"),
            contents: bytemuck::cast_slice(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("attention_bind_group"),
            layout: &self.attention_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: q.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: k.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: v.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: output.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: uniform_buffer.as_entire_binding() },
            ],
        });

        // Dispatch compute
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("attention_encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("attention_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.attention_pipeline.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch: one workgroup per head per batch of sequence positions
            let block_size = 64u32; // Flash attention block size
            let num_blocks = (seq_len + block_size - 1) / block_size;
            pass.dispatch_workgroups(num_blocks, config.num_heads as u32, 1);
        }

        let kernel_start = std::time::Instant::now();
        self.queue.submit(std::iter::once(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);
        let kernel_time = kernel_start.elapsed();

        let total_time = start.elapsed();

        // Calculate metrics (attention has O(n^2*d) complexity)
        let flops = 4.0 * (seq_len as f64).powi(2) * (hidden_dim as f64);
        let metrics = ComputeMetrics {
            flops,
            bandwidth_gbps: ((q.size + k.size + v.size + output.size) as f64) / kernel_time.as_secs_f64() / 1e9,
            kernel_time_ms: kernel_time.as_secs_f64() * 1000.0,
            transfer_time_ms: 0.0,
            total_time_ms: total_time.as_secs_f64() * 1000.0,
        };

        self.last_metrics = metrics.clone();
        Ok(metrics)
    }

    // ========================================================================
    // LoRA Forward
    // ========================================================================

    /// Apply LoRA adapter: output = input + scaling * (input @ A @ B)
    ///
    /// Where A is (in_dim x rank) and B is (rank x out_dim).
    ///
    /// Performance target: <1ms
    pub async fn lora_forward(
        &mut self,
        input: &GpuBuffer,
        lora_a: &GpuBuffer,
        lora_b: &GpuBuffer,
        output: &GpuBuffer,
        config: &LoraConfig,
        batch_size: u32,
    ) -> Result<ComputeMetrics, ComputeError> {
        let start = std::time::Instant::now();

        // Validate dimensions
        let expected_input = (batch_size as usize) * config.in_dim * 4;
        let expected_a = config.a_size() * 4;
        let expected_b = config.b_size() * 4;
        let expected_output = (batch_size as usize) * config.out_dim * 4;

        if input.size != expected_input || lora_a.size != expected_a ||
           lora_b.size != expected_b || output.size != expected_output {
            return Err(ComputeError::DimensionMismatch {
                expected: format!("input:{}x{}, A:{}x{}, B:{}x{}, output:{}x{}",
                    batch_size, config.in_dim, config.in_dim, config.rank,
                    config.rank, config.out_dim, batch_size, config.out_dim),
                actual: format!("input:{}, A:{}, B:{}, output:{} bytes",
                    input.size, lora_a.size, lora_b.size, output.size),
            });
        }

        // Create uniforms buffer
        let scaling = config.scaling();
        let uniforms: [f32; 8] = [
            batch_size as f32,
            config.in_dim as f32,
            config.rank as f32,
            config.out_dim as f32,
            scaling,
            0.0, 0.0, 0.0, // padding
        ];
        let uniform_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("lora_uniforms"),
            contents: bytemuck::cast_slice(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lora_bind_group"),
            layout: &self.lora_pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: input.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: lora_a.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: lora_b.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: output.raw().as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: uniform_buffer.as_entire_binding() },
            ],
        });

        // Dispatch compute
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("lora_encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("lora_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.lora_pipeline.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);

            // Dispatch: one workgroup per batch element
            let workgroup_size = 256u32;
            let workgroups = (batch_size * config.out_dim as u32 + workgroup_size - 1) / workgroup_size;
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        let kernel_start = std::time::Instant::now();
        self.queue.submit(std::iter::once(encoder.finish()));
        self.device.poll(wgpu::Maintain::Wait);
        let kernel_time = kernel_start.elapsed();

        let total_time = start.elapsed();

        // Calculate metrics
        // LoRA: input @ A @ B = 2 matmuls
        let flops = 2.0 * (batch_size as f64) * (config.in_dim as f64) * (config.rank as f64)
                  + 2.0 * (batch_size as f64) * (config.rank as f64) * (config.out_dim as f64);
        let metrics = ComputeMetrics {
            flops,
            bandwidth_gbps: ((input.size + lora_a.size + lora_b.size + output.size) as f64)
                          / kernel_time.as_secs_f64() / 1e9,
            kernel_time_ms: kernel_time.as_secs_f64() * 1000.0,
            transfer_time_ms: 0.0,
            total_time_ms: total_time.as_secs_f64() * 1000.0,
        };

        self.last_metrics = metrics.clone();
        Ok(metrics)
    }

    // ========================================================================
    // Utilities
    // ========================================================================

    /// Get last operation metrics
    pub fn last_metrics(&self) -> &ComputeMetrics {
        &self.last_metrics
    }

    /// Get device limits
    pub fn limits(&self) -> &wgpu::Limits {
        &self.limits
    }

    /// Get configuration
    pub fn config(&self) -> &ComputeConfig {
        &self.config
    }

    /// Synchronize all pending GPU operations
    pub fn sync(&self) {
        self.device.poll(wgpu::Maintain::Wait);
    }
}

// ============================================================================
// Staging Buffer Pool
// ============================================================================

/// Pool of reusable staging buffers for CPU<->GPU transfers
struct StagingBufferPool {
    device: Arc<wgpu::Device>,
    upload_buffers: Vec<wgpu::Buffer>,
    download_buffers: Vec<wgpu::Buffer>,
    max_pool_size: usize,
}

impl StagingBufferPool {
    fn new(device: Arc<wgpu::Device>, max_pool_size: usize) -> Self {
        Self {
            device,
            upload_buffers: Vec::new(),
            download_buffers: Vec::new(),
            max_pool_size,
        }
    }

    fn get_upload_buffer(&self, size: usize) -> Result<wgpu::Buffer, ComputeError> {
        // For simplicity, always create new buffer (production would pool)
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_upload"),
            size: size as u64,
            usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        Ok(buffer)
    }

    fn get_download_buffer(&self, size: usize) -> Result<wgpu::Buffer, ComputeError> {
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_download"),
            size: size as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(buffer)
    }
}

// ============================================================================
// wgpu::util helpers
// ============================================================================

mod wgpu_util {
    use super::*;

    impl wgpu::Device {
        pub fn create_buffer_init(&self, desc: &wgpu::util::BufferInitDescriptor) -> wgpu::Buffer {
            wgpu::util::DeviceExt::create_buffer_init(self, desc)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a GPU and are marked as ignored by default
    // Run with: cargo test --features webgpu -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_webgpu_init() {
        let compute = WebGpuCompute::new().await;
        assert!(compute.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_buffer_allocation() {
        let compute = WebGpuCompute::new().await.unwrap();
        let desc = TensorDescriptor::matrix(1024, 1024, DataType::F32);
        let buffer = compute.allocate_buffer(desc, BufferUsage::storage());
        assert!(buffer.is_ok());
        assert_eq!(buffer.unwrap().size(), 1024 * 1024 * 4);
    }
}
