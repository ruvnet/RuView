//! Compute pipeline management for GPU operations.
//!
//! This module handles shader compilation, pipeline creation, and bind group
//! management for GPU compute operations.

use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info};
use wgpu::{Device, ShaderModule};

use super::buffer::GpuBuffer;
use super::error::{GpuError, GpuResult};
use super::DEFAULT_WORKGROUP_SIZE;

/// Type of binding in a compute shader
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingType {
    /// Storage buffer (read-only)
    StorageReadonly,
    /// Storage buffer (read-write)
    StorageReadWrite,
    /// Uniform buffer
    Uniform,
}

impl BindingType {
    /// Convert to wgpu binding type
    fn to_wgpu(&self) -> wgpu::BindingType {
        match self {
            Self::StorageReadonly => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            Self::StorageReadWrite => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            Self::Uniform => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        }
    }
}

/// Description of a binding in a compute shader
#[derive(Debug, Clone)]
pub struct BindingDesc {
    /// Binding type
    pub binding_type: BindingType,
    /// Optional label for debugging
    pub label: Option<String>,
}

impl BindingDesc {
    /// Create a storage read-only binding
    pub fn storage_readonly() -> Self {
        Self {
            binding_type: BindingType::StorageReadonly,
            label: None,
        }
    }

    /// Create a storage read-write binding
    pub fn storage_readwrite() -> Self {
        Self {
            binding_type: BindingType::StorageReadWrite,
            label: None,
        }
    }

    /// Create a uniform binding
    pub fn uniform() -> Self {
        Self {
            binding_type: BindingType::Uniform,
            label: None,
        }
    }

    /// Add a label to the binding
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Compute pipeline wrapper
pub struct ComputePipeline {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    workgroup_size: [u32; 3],
    entry_point: String,
    binding_count: usize,
}

impl ComputePipeline {
    /// Create a new compute pipeline from shader source.
    ///
    /// # Arguments
    ///
    /// * `device` - The wgpu device
    /// * `shader_source` - WGSL shader source code
    /// * `entry_point` - Entry point function name
    /// * `bindings` - Binding descriptions
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let pipeline = ComputePipeline::from_shader(
    ///     &device,
    ///     r#"
    ///         @group(0) @binding(0) var<storage, read> input: array<f32>;
    ///         @group(0) @binding(1) var<storage, read_write> output: array<f32>;
    ///
    ///         @compute @workgroup_size(256)
    ///         fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    ///             output[id.x] = input[id.x] * 2.0;
    ///         }
    ///     "#,
    ///     "main",
    ///     &[BindingDesc::storage_readonly(), BindingDesc::storage_readwrite()],
    /// );
    /// ```
    pub fn from_shader(
        device: &Device,
        shader_source: &str,
        entry_point: &str,
        bindings: &[BindingDesc],
    ) -> GpuResult<Self> {
        Self::from_shader_with_workgroup_size(
            device,
            shader_source,
            entry_point,
            bindings,
            [DEFAULT_WORKGROUP_SIZE, 1, 1],
        )
    }

    /// Create a pipeline with custom workgroup size.
    pub fn from_shader_with_workgroup_size(
        device: &Device,
        shader_source: &str,
        entry_point: &str,
        bindings: &[BindingDesc],
        workgroup_size: [u32; 3],
    ) -> GpuResult<Self> {
        debug!(
            "Creating compute pipeline with entry point '{}' and {} bindings",
            entry_point,
            bindings.len()
        );

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compute_shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        Self::from_module(device, &shader, entry_point, bindings, workgroup_size)
    }

    /// Create a pipeline from a pre-compiled shader module.
    pub fn from_module(
        device: &Device,
        shader: &ShaderModule,
        entry_point: &str,
        bindings: &[BindingDesc],
        workgroup_size: [u32; 3],
    ) -> GpuResult<Self> {
        // Create bind group layout entries
        let layout_entries: Vec<wgpu::BindGroupLayoutEntry> = bindings
            .iter()
            .enumerate()
            .map(|(i, desc)| wgpu::BindGroupLayoutEntry {
                binding: i as u32,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: desc.binding_type.to_wgpu(),
                count: None,
            })
            .collect();

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compute_bind_group_layout"),
            entries: &layout_entries,
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compute_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compute_pipeline"),
            layout: Some(&pipeline_layout),
            module: shader,
            entry_point: Some(entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(Self {
            pipeline,
            bind_group_layout,
            workgroup_size,
            entry_point: entry_point.to_string(),
            binding_count: bindings.len(),
        })
    }

    /// Create a bind group for this pipeline.
    ///
    /// # Arguments
    ///
    /// * `device` - The wgpu device
    /// * `buffers` - Buffers to bind, in order
    ///
    /// # Panics
    ///
    /// Panics if the number of buffers doesn't match the pipeline's binding count.
    pub fn create_bind_group(
        &self,
        device: &Device,
        buffers: &[&GpuBuffer],
    ) -> GpuResult<wgpu::BindGroup> {
        if buffers.len() != self.binding_count {
            return Err(GpuError::InvalidBindingCount {
                expected: self.binding_count,
                actual: buffers.len(),
            });
        }

        let entries: Vec<wgpu::BindGroupEntry> = buffers
            .iter()
            .enumerate()
            .map(|(i, buffer)| buffer.binding(i as u32))
            .collect();

        Ok(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compute_bind_group"),
            layout: &self.bind_group_layout,
            entries: &entries,
        }))
    }

    /// Get the underlying wgpu pipeline
    pub fn pipeline(&self) -> &wgpu::ComputePipeline {
        &self.pipeline
    }

    /// Get the bind group layout
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Get the workgroup size
    pub fn workgroup_size(&self) -> [u32; 3] {
        self.workgroup_size
    }

    /// Get the entry point name
    pub fn entry_point(&self) -> &str {
        &self.entry_point
    }

    /// Get the number of bindings
    pub fn binding_count(&self) -> usize {
        self.binding_count
    }

    /// Calculate workgroup count for a given data size.
    pub fn calculate_workgroups(&self, data_size: u32) -> [u32; 3] {
        let x = (data_size + self.workgroup_size[0] - 1) / self.workgroup_size[0];
        [x, 1, 1]
    }

    /// Calculate workgroup count for 2D data.
    pub fn calculate_workgroups_2d(&self, width: u32, height: u32) -> [u32; 3] {
        let x = (width + self.workgroup_size[0] - 1) / self.workgroup_size[0];
        let y = (height + self.workgroup_size[1] - 1) / self.workgroup_size[1];
        [x, y, 1]
    }

    /// Calculate workgroup count for 3D data.
    pub fn calculate_workgroups_3d(&self, width: u32, height: u32, depth: u32) -> [u32; 3] {
        let x = (width + self.workgroup_size[0] - 1) / self.workgroup_size[0];
        let y = (height + self.workgroup_size[1] - 1) / self.workgroup_size[1];
        let z = (depth + self.workgroup_size[2] - 1) / self.workgroup_size[2];
        [x, y, z]
    }
}

/// Cache for compute pipelines
pub struct PipelineCache {
    device: Arc<Device>,
    pipelines: DashMap<String, Arc<ComputePipeline>>,
}

impl PipelineCache {
    /// Create a new pipeline cache
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            device,
            pipelines: DashMap::new(),
        }
    }

    /// Get or create a pipeline.
    ///
    /// # Arguments
    ///
    /// * `name` - Unique name for the pipeline
    /// * `shader_source` - WGSL shader source
    /// * `entry_point` - Entry point function name
    /// * `bindings` - Binding descriptions
    pub fn get_or_create(
        &self,
        name: &str,
        shader_source: &str,
        entry_point: &str,
        bindings: &[BindingDesc],
    ) -> GpuResult<Arc<ComputePipeline>> {
        if let Some(pipeline) = self.pipelines.get(name) {
            return Ok(Arc::clone(&pipeline));
        }

        info!("Creating and caching pipeline: {}", name);

        let pipeline =
            ComputePipeline::from_shader(&self.device, shader_source, entry_point, bindings)?;
        let pipeline = Arc::new(pipeline);

        self.pipelines
            .insert(name.to_string(), Arc::clone(&pipeline));

        Ok(pipeline)
    }

    /// Get a cached pipeline by name.
    pub fn get(&self, name: &str) -> Option<Arc<ComputePipeline>> {
        self.pipelines.get(name).map(|p| Arc::clone(&p))
    }

    /// Check if a pipeline exists in cache.
    pub fn contains(&self, name: &str) -> bool {
        self.pipelines.contains_key(name)
    }

    /// Remove a pipeline from cache.
    pub fn remove(&self, name: &str) -> Option<Arc<ComputePipeline>> {
        self.pipelines.remove(name).map(|(_, p)| p)
    }

    /// Clear all cached pipelines.
    pub fn clear(&self) {
        self.pipelines.clear();
    }

    /// Get the number of cached pipelines.
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    /// List all cached pipeline names.
    pub fn names(&self) -> Vec<String> {
        self.pipelines.iter().map(|e| e.key().clone()).collect()
    }
}

/// Pre-defined shaders for common coherence operations
pub mod shaders {
    /// WGSL shader for computing residuals
    pub const RESIDUAL_COMPUTE: &str = r#"
        // Node states: [node_count, dim]
        @group(0) @binding(0) var<storage, read> node_states: array<f32>;
        // Edge info: [edge_count, 4] - source_idx, target_idx, weight, padding
        @group(0) @binding(1) var<storage, read> edges: array<vec4<f32>>;
        // Restriction map (identity for simplicity): [dim, dim]
        @group(0) @binding(2) var<storage, read> restriction: array<f32>;
        // Output residuals: [edge_count]
        @group(0) @binding(3) var<storage, read_write> residuals: array<f32>;
        // Params: [dim, node_count, edge_count, 0]
        @group(0) @binding(4) var<uniform> params: vec4<u32>;

        @compute @workgroup_size(256)
        fn main(@builtin(global_invocation_id) id: vec3<u32>) {
            let edge_idx = id.x;
            let edge_count = params.z;
            let dim = params.x;

            if (edge_idx >= edge_count) {
                return;
            }

            let edge = edges[edge_idx];
            let source_idx = u32(edge.x);
            let target_idx = u32(edge.y);
            let weight = edge.z;

            // Compute residual = ||rho_u(x_u) - rho_v(x_v)||^2
            var residual: f32 = 0.0;
            for (var d: u32 = 0u; d < dim; d = d + 1u) {
                let source_val = node_states[source_idx * dim + d];
                let target_val = node_states[target_idx * dim + d];
                let diff = source_val - target_val;
                residual = residual + diff * diff;
            }

            residuals[edge_idx] = weight * residual;
        }
    "#;

    /// WGSL shader for parallel reduction (sum)
    pub const REDUCE_SUM: &str = r#"
        @group(0) @binding(0) var<storage, read> input: array<f32>;
        @group(0) @binding(1) var<storage, read_write> output: array<f32>;
        @group(0) @binding(2) var<uniform> count: u32;

        var<workgroup> shared_data: array<f32, 256>;

        @compute @workgroup_size(256)
        fn main(
            @builtin(global_invocation_id) global_id: vec3<u32>,
            @builtin(local_invocation_id) local_id: vec3<u32>,
            @builtin(workgroup_id) workgroup_id: vec3<u32>
        ) {
            let tid = local_id.x;
            let gid = global_id.x;

            // Load data into shared memory
            if (gid < count) {
                shared_data[tid] = input[gid];
            } else {
                shared_data[tid] = 0.0;
            }
            workgroupBarrier();

            // Parallel reduction
            for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
                if (tid < s) {
                    shared_data[tid] = shared_data[tid] + shared_data[tid + s];
                }
                workgroupBarrier();
            }

            // Write result
            if (tid == 0u) {
                output[workgroup_id.x] = shared_data[0];
            }
        }
    "#;

    /// WGSL shader for matrix-vector multiplication
    pub const MATVEC: &str = r#"
        @group(0) @binding(0) var<storage, read> matrix: array<f32>;
        @group(0) @binding(1) var<storage, read> vector: array<f32>;
        @group(0) @binding(2) var<storage, read_write> result: array<f32>;
        // params: [rows, cols, 0, 0]
        @group(0) @binding(3) var<uniform> params: vec4<u32>;

        @compute @workgroup_size(256)
        fn main(@builtin(global_invocation_id) id: vec3<u32>) {
            let row = id.x;
            let rows = params.x;
            let cols = params.y;

            if (row >= rows) {
                return;
            }

            var sum: f32 = 0.0;
            for (var c: u32 = 0u; c < cols; c = c + 1u) {
                sum = sum + matrix[row * cols + c] * vector[c];
            }

            result[row] = sum;
        }
    "#;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binding_desc() {
        let readonly = BindingDesc::storage_readonly();
        assert_eq!(readonly.binding_type, BindingType::StorageReadonly);

        let readwrite = BindingDesc::storage_readwrite();
        assert_eq!(readwrite.binding_type, BindingType::StorageReadWrite);

        let uniform = BindingDesc::uniform();
        assert_eq!(uniform.binding_type, BindingType::Uniform);
    }

    #[test]
    fn test_binding_with_label() {
        let binding = BindingDesc::storage_readonly().with_label("input_buffer");
        assert_eq!(binding.label.as_deref(), Some("input_buffer"));
    }
}
