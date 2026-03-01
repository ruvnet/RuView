//! GPU Kernel Wrappers
//!
//! Provides Rust wrappers around WGSL compute shaders for coherence computation.
//! Each kernel handles pipeline creation, bind group setup, and dispatch.

use super::buffer::{
    BufferUsage, GpuBuffer, GpuBufferManager, GpuEdge, GpuParams, GpuRestrictionMap,
};
use super::error::{GpuError, GpuResult};
use super::shaders;
use super::workgroup;
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BufferBindingType, ComputePipeline,
    ComputePipelineDescriptor, Device, PipelineLayoutDescriptor, Queue, ShaderModule,
    ShaderModuleDescriptor, ShaderSource, ShaderStages,
};

/// Compute residuals kernel
/// Computes r_e = rho_source(x_source) - rho_target(x_target) for all edges
pub struct ComputeResidualsKernel {
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

impl ComputeResidualsKernel {
    /// Create a new compute residuals kernel
    pub fn new(device: &Device) -> GpuResult<Self> {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("compute_residuals"),
            source: ShaderSource::Wgsl(shaders::COMPUTE_RESIDUALS.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("compute_residuals_bind_group_layout"),
            entries: &[
                // Params uniform
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Node states
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Edges
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Restriction maps
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Restriction data
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Residuals output
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Residual norms output
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("compute_residuals_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("compute_residuals_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self {
            pipeline,
            bind_group_layout,
        })
    }

    /// Create a bind group for execution
    pub fn create_bind_group(
        &self,
        device: &Device,
        params_buffer: &GpuBuffer,
        node_states_buffer: &GpuBuffer,
        edges_buffer: &GpuBuffer,
        restriction_maps_buffer: &GpuBuffer,
        restriction_data_buffer: &GpuBuffer,
        residuals_buffer: &GpuBuffer,
        residual_norms_buffer: &GpuBuffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("compute_residuals_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: node_states_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: edges_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: restriction_maps_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: restriction_data_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: residuals_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: residual_norms_buffer.buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create a bind group using raw wgpu buffers (for pre-allocated buffer optimization)
    pub fn create_bind_group_raw(
        &self,
        device: &Device,
        params_buffer: &wgpu::Buffer,
        node_states_buffer: &wgpu::Buffer,
        edges_buffer: &wgpu::Buffer,
        restriction_maps_buffer: &wgpu::Buffer,
        restriction_data_buffer: &wgpu::Buffer,
        residuals_buffer: &wgpu::Buffer,
        residual_norms_buffer: &wgpu::Buffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("compute_residuals_bind_group_raw"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: node_states_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: edges_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: restriction_maps_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: restriction_data_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: residuals_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: residual_norms_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Get the pipeline for use in command encoder
    pub fn pipeline(&self) -> &ComputePipeline {
        &self.pipeline
    }

    /// Calculate number of workgroups needed
    pub fn workgroup_count(num_edges: u32) -> u32 {
        // One thread per edge, 256 threads per workgroup
        (num_edges + workgroup::SIZE_1D - 1) / workgroup::SIZE_1D
    }
}

/// Compute energy kernel with parallel reduction
pub struct ComputeEnergyKernel {
    main_pipeline: ComputePipeline,
    final_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

/// Parameters for energy reduction
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct EnergyParams {
    /// Number of elements to reduce
    pub num_elements: u32,
    /// Padding
    pub _padding: [u32; 7],
}

impl ComputeEnergyKernel {
    /// Create a new compute energy kernel
    pub fn new(device: &Device) -> GpuResult<Self> {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("compute_energy"),
            source: ShaderSource::Wgsl(shaders::COMPUTE_ENERGY.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("compute_energy_bind_group_layout"),
            entries: &[
                // Params uniform
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Input energies
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Output partial sums
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("compute_energy_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let main_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("compute_energy_main_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let final_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("compute_energy_final_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("final_reduce"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self {
            main_pipeline,
            final_pipeline,
            bind_group_layout,
        })
    }

    /// Create a bind group for execution
    pub fn create_bind_group(
        &self,
        device: &Device,
        params_buffer: &GpuBuffer,
        input_buffer: &GpuBuffer,
        output_buffer: &GpuBuffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("compute_energy_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: input_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create a bind group using raw wgpu buffers (for pre-allocated buffer optimization)
    pub fn create_bind_group_raw(
        &self,
        device: &Device,
        params_buffer: &wgpu::Buffer,
        input_buffer: &wgpu::Buffer,
        output_buffer: &wgpu::Buffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("compute_energy_bind_group_raw"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: input_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Get the main reduction pipeline
    pub fn main_pipeline(&self) -> &ComputePipeline {
        &self.main_pipeline
    }

    /// Get the final reduction pipeline
    pub fn final_pipeline(&self) -> &ComputePipeline {
        &self.final_pipeline
    }

    /// Calculate number of workgroups for first pass
    pub fn workgroup_count(num_elements: u32) -> u32 {
        // One element per thread, 256 threads per workgroup
        (num_elements + workgroup::SIZE_1D - 1) / workgroup::SIZE_1D
    }
}

/// Sheaf attention kernel
pub struct SheafAttentionKernel {
    single_pass_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

/// Attention weight output
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct AttentionWeight {
    pub edge_idx: u32,
    pub source_idx: u32,
    pub target_idx: u32,
    pub raw_score: f32,
    pub attention: f32,
    pub _padding: [u32; 3],
}

impl SheafAttentionKernel {
    /// Create a new sheaf attention kernel
    pub fn new(device: &Device) -> GpuResult<Self> {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("sheaf_attention"),
            source: ShaderSource::Wgsl(shaders::SHEAF_ATTENTION.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("sheaf_attention_bind_group_layout"),
            entries: &[
                // Params
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Edges
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Edge energies
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Attention weights output
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Node exp sums (for normalization)
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("sheaf_attention_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let single_pass_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("sheaf_attention_single_pass_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("compute_attention_single_pass"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self {
            single_pass_pipeline,
            bind_group_layout,
        })
    }

    /// Create a bind group
    pub fn create_bind_group(
        &self,
        device: &Device,
        params_buffer: &GpuBuffer,
        edges_buffer: &GpuBuffer,
        edge_energies_buffer: &GpuBuffer,
        attention_weights_buffer: &GpuBuffer,
        node_exp_sums_buffer: &GpuBuffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("sheaf_attention_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: edges_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: edge_energies_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: attention_weights_buffer.buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: node_exp_sums_buffer.buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Get the single-pass pipeline
    pub fn pipeline(&self) -> &ComputePipeline {
        &self.single_pass_pipeline
    }

    /// Calculate workgroup count
    pub fn workgroup_count(num_edges: u32) -> u32 {
        (num_edges + workgroup::SIZE_1D - 1) / workgroup::SIZE_1D
    }
}

/// Token routing kernel
pub struct TokenRoutingKernel {
    route_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

/// Token input
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Token {
    pub token_id: u32,
    pub node_idx: u32,
    pub action_type: u32,
    pub priority: f32,
}

/// Routing decision output
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct RoutingDecision {
    pub token_id: u32,
    pub assigned_lane: u32,
    pub local_energy: f32,
    pub confidence: f32,
    pub escalation_reason: u32,
    pub num_high_energy_edges: u32,
    pub max_edge_energy: f32,
    pub _padding: u32,
}

/// Lane statistics
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LaneStats {
    pub lane_counts: [u32; 4],
    pub total_energy_per_lane: [f32; 4],
    pub _padding: [u32; 8],
}

impl TokenRoutingKernel {
    /// Create a new token routing kernel
    pub fn new(device: &Device) -> GpuResult<Self> {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("token_routing"),
            source: ShaderSource::Wgsl(shaders::TOKEN_ROUTING.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("token_routing_bind_group_layout"),
            entries: &[
                // Params
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Tokens
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Local energies
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Edge energies
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Node edge counts
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Node edge offsets
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Node edges
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Routing decisions output
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Lane stats output
                BindGroupLayoutEntry {
                    binding: 8,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("token_routing_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let route_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("token_routing_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("route_tokens"),
            compilation_options: Default::default(),
            cache: None,
        });

        Ok(Self {
            route_pipeline,
            bind_group_layout,
        })
    }

    /// Get the routing pipeline
    pub fn pipeline(&self) -> &ComputePipeline {
        &self.route_pipeline
    }

    /// Get bind group layout
    pub fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.bind_group_layout
    }

    /// Calculate workgroup count
    pub fn workgroup_count(num_tokens: u32) -> u32 {
        (num_tokens + workgroup::SIZE_1D - 1) / workgroup::SIZE_1D
    }
}
