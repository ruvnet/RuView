//! GPU Coherence Engine
//!
//! Main entry point for GPU-accelerated coherence computation.
//! Provides automatic CPU fallback when GPU is unavailable.

use super::buffer::{BufferUsage, GpuBufferManager, GpuEdge, GpuParams, GpuRestrictionMap};
use super::error::{GpuError, GpuResult};
use super::kernels::{
    ComputeEnergyKernel, ComputeResidualsKernel, EnergyParams, SheafAttentionKernel,
    TokenRoutingKernel,
};
use crate::coherence::{CoherenceEnergy as CpuCoherenceEnergy, EdgeEnergy};
use crate::substrate::restriction::MatrixStorage;
use crate::substrate::{EdgeId, NodeId, SheafGraph};

use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};
use wgpu::{
    Adapter, Device, DeviceDescriptor, Features, Instance, InstanceDescriptor, Limits,
    PowerPreference, Queue, RequestAdapterOptions,
};

/// GPU configuration
#[derive(Debug, Clone)]
pub struct GpuConfig {
    /// Preferred power preference (high performance vs low power)
    pub power_preference: PowerPreference,
    /// Enable CPU fallback when GPU is unavailable
    pub enable_fallback: bool,
    /// Maximum buffer size in bytes (0 = no limit)
    pub max_buffer_size: usize,
    /// Beta parameter for attention computation
    pub beta: f32,
    /// Lane 0 (reflex) threshold
    pub threshold_lane0: f32,
    /// Lane 1 (retrieval) threshold
    pub threshold_lane1: f32,
    /// Lane 2 (heavy) threshold
    pub threshold_lane2: f32,
    /// Timeout for GPU operations in milliseconds
    pub timeout_ms: u64,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            power_preference: PowerPreference::HighPerformance,
            enable_fallback: true,
            max_buffer_size: 0, // No limit
            beta: 1.0,
            threshold_lane0: 0.01,
            threshold_lane1: 0.1,
            threshold_lane2: 1.0,
            timeout_ms: 5000,
        }
    }
}

/// GPU capabilities and limits
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
    /// Device name
    pub device_name: String,
    /// Vendor
    pub vendor: String,
    /// Backend (Vulkan, Metal, DX12, etc.)
    pub backend: String,
    /// Maximum buffer size
    pub max_buffer_size: u64,
    /// Maximum compute workgroup size
    pub max_workgroup_size: u32,
    /// Maximum compute workgroups per dimension
    pub max_workgroups: [u32; 3],
    /// Whether the GPU supports required features
    pub supported: bool,
}

/// GPU energy result
#[derive(Debug, Clone)]
pub struct GpuCoherenceEnergy {
    /// Total system energy
    pub total_energy: f32,
    /// Per-edge energies
    pub edge_energies: Vec<f32>,
    /// Edge indices (matches edge_energies)
    pub edge_indices: Vec<EdgeId>,
    /// Computation time in microseconds
    pub compute_time_us: u64,
    /// Whether GPU was used (false = CPU fallback)
    pub used_gpu: bool,
}

impl GpuCoherenceEnergy {
    /// Convert to CPU CoherenceEnergy format
    pub fn to_cpu_format(&self, graph: &SheafGraph) -> CpuCoherenceEnergy {
        let mut edge_energy_map = HashMap::new();

        for (i, &edge_id) in self.edge_indices.iter().enumerate() {
            let energy = self.edge_energies[i];
            if let Some(edge) = graph.get_edge(edge_id) {
                let edge_energy = EdgeEnergy::new_lightweight(
                    edge_id.to_string(),
                    edge.source.to_string(),
                    edge.target.to_string(),
                    energy / edge.weight.max(0.001), // Remove weight to get raw norm_sq
                    edge.weight,
                );
                edge_energy_map.insert(edge_id.to_string(), edge_energy);
            }
        }

        CpuCoherenceEnergy::new(
            edge_energy_map,
            &HashMap::new(),
            graph.node_count(),
            format!("gpu-{}", Utc::now().timestamp()),
        )
    }
}

/// GPU-accelerated coherence engine
pub struct GpuCoherenceEngine {
    device: Arc<Device>,
    queue: Arc<Queue>,
    buffer_manager: GpuBufferManager,
    config: GpuConfig,
    capabilities: GpuCapabilities,

    // Kernels
    residuals_kernel: ComputeResidualsKernel,
    energy_kernel: ComputeEnergyKernel,
    attention_kernel: SheafAttentionKernel,
    routing_kernel: TokenRoutingKernel,

    // Cached graph data
    graph_data: Option<GpuGraphData>,
}

/// Cached graph data on GPU
struct GpuGraphData {
    num_nodes: u32,
    num_edges: u32,
    state_dim: u32,
    node_id_map: HashMap<NodeId, u32>,
    edge_id_map: HashMap<EdgeId, u32>,
    edge_id_reverse: Vec<EdgeId>,

    // Pre-allocated computation buffers (eliminates per-frame allocations)
    params_buffer: wgpu::Buffer,
    energy_params_buffer: wgpu::Buffer,
    partial_sums_buffer: wgpu::Buffer,
    final_params_buffer: wgpu::Buffer,
    total_energy_buffer: wgpu::Buffer,
    energies_staging: wgpu::Buffer,
    total_staging: wgpu::Buffer,

    // Pre-computed workgroup count for energy reduction
    num_workgroups: u32,
}

impl GpuCoherenceEngine {
    /// Create a new GPU coherence engine
    pub async fn new(config: GpuConfig) -> GpuResult<Self> {
        // Create wgpu instance
        let instance = Instance::new(InstanceDescriptor::default());

        // Request adapter
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: config.power_preference,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| GpuError::AdapterRequest("No suitable GPU adapter found".into()))?;

        let capabilities = Self::get_capabilities(&adapter);
        if !capabilities.supported {
            return Err(GpuError::UnsupportedFeature(
                "GPU does not support required features".into(),
            ));
        }

        info!(
            "Using GPU: {} ({}) - {}",
            capabilities.device_name, capabilities.vendor, capabilities.backend
        );

        // Request device
        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("prime_radiant_gpu"),
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                    memory_hints: Default::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::DeviceCreation(e.to_string()))?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        // Create kernels
        let residuals_kernel = ComputeResidualsKernel::new(&device)?;
        let energy_kernel = ComputeEnergyKernel::new(&device)?;
        let attention_kernel = SheafAttentionKernel::new(&device)?;
        let routing_kernel = TokenRoutingKernel::new(&device)?;

        // Create buffer manager
        let buffer_manager = GpuBufferManager::new(device.clone(), queue.clone());

        Ok(Self {
            device,
            queue,
            buffer_manager,
            config,
            capabilities,
            residuals_kernel,
            energy_kernel,
            attention_kernel,
            routing_kernel,
            graph_data: None,
        })
    }

    /// Try to create a GPU engine, returning None if GPU is unavailable
    pub async fn try_new(config: GpuConfig) -> Option<Self> {
        match Self::new(config).await {
            Ok(engine) => Some(engine),
            Err(e) => {
                warn!("GPU initialization failed: {}. Will use CPU fallback.", e);
                None
            }
        }
    }

    /// Get GPU capabilities
    fn get_capabilities(adapter: &Adapter) -> GpuCapabilities {
        let info = adapter.get_info();
        let limits = adapter.limits();

        GpuCapabilities {
            device_name: info.name,
            vendor: format!("{:?}", info.vendor),
            backend: format!("{:?}", info.backend),
            max_buffer_size: limits.max_buffer_size as u64,
            max_workgroup_size: limits.max_compute_workgroup_size_x,
            max_workgroups: [
                limits.max_compute_workgroups_per_dimension,
                limits.max_compute_workgroups_per_dimension,
                limits.max_compute_workgroups_per_dimension,
            ],
            supported: true,
        }
    }

    /// Upload graph data to GPU
    pub fn upload_graph(&mut self, graph: &SheafGraph) -> GpuResult<()> {
        if graph.edge_count() == 0 {
            return Err(GpuError::EmptyGraph);
        }

        let num_nodes = graph.node_count() as u32;
        let num_edges = graph.edge_count() as u32;

        // Build node ID mapping
        let mut node_id_map = HashMap::new();
        let node_ids = graph.node_ids();
        for (i, node_id) in node_ids.iter().enumerate() {
            node_id_map.insert(*node_id, i as u32);
        }

        // Determine state dimension from first node
        let state_dim = node_ids
            .first()
            .and_then(|id| graph.get_node(*id))
            .map(|n| n.dim())
            .unwrap_or(64) as u32;

        // Flatten node states
        let mut node_states: Vec<f32> = Vec::with_capacity((num_nodes * state_dim) as usize);
        for node_id in &node_ids {
            if let Some(state) = graph.node_state(*node_id) {
                node_states.extend(state.iter().cloned());
                // Pad if needed
                for _ in state.len()..(state_dim as usize) {
                    node_states.push(0.0);
                }
            }
        }

        // Build edge data and restriction maps
        let mut edges: Vec<GpuEdge> = Vec::with_capacity(num_edges as usize);
        let mut restriction_maps: Vec<GpuRestrictionMap> = Vec::new();
        let mut restriction_data: Vec<f32> = Vec::new();
        let mut edge_id_map = HashMap::new();
        let mut edge_id_reverse = Vec::new();

        let edge_ids = graph.edge_ids();
        for (i, edge_id) in edge_ids.iter().enumerate() {
            edge_id_map.insert(*edge_id, i as u32);
            edge_id_reverse.push(*edge_id);

            if let Some(edge) = graph.get_edge(*edge_id) {
                let source_idx = *node_id_map.get(&edge.source).unwrap_or(&0);
                let target_idx = *node_id_map.get(&edge.target).unwrap_or(&0);

                // Convert restriction maps
                let rho_source_idx = restriction_maps.len() as u32;
                let gpu_rho_source =
                    Self::convert_restriction_map(&edge.rho_source, &mut restriction_data);
                restriction_maps.push(gpu_rho_source);

                let rho_target_idx = restriction_maps.len() as u32;
                let gpu_rho_target =
                    Self::convert_restriction_map(&edge.rho_target, &mut restriction_data);
                restriction_maps.push(gpu_rho_target);

                edges.push(GpuEdge {
                    source_idx,
                    target_idx,
                    weight: edge.weight,
                    rho_source_idx,
                    rho_target_idx,
                    comparison_dim: edge.comparison_dim() as u32,
                    _padding: [0; 2],
                });
            }
        }

        // Ensure restriction_data is not empty (GPU buffers can't be zero-sized)
        if restriction_data.is_empty() {
            restriction_data.push(0.0);
        }

        // Upload to GPU
        self.buffer_manager.allocate_with_data(
            &node_states,
            BufferUsage::NodeStates,
            "node_states",
        )?;

        self.buffer_manager
            .allocate_with_data(&edges, BufferUsage::EdgeData, "edges")?;

        self.buffer_manager.allocate_with_data(
            &restriction_maps,
            BufferUsage::RestrictionMaps,
            "restriction_maps",
        )?;

        self.buffer_manager.allocate_with_data(
            &restriction_data,
            BufferUsage::RestrictionMaps,
            "restriction_data",
        )?;

        // Allocate output buffers
        let max_comparison_dim = edges
            .iter()
            .map(|e| e.comparison_dim)
            .max()
            .unwrap_or(state_dim);
        let residuals_size = (num_edges * max_comparison_dim) as usize * std::mem::size_of::<f32>();
        let energies_size = num_edges as usize * std::mem::size_of::<f32>();

        self.buffer_manager
            .allocate(residuals_size, BufferUsage::Residuals, "residuals")?;

        self.buffer_manager
            .allocate(energies_size, BufferUsage::Energies, "edge_energies")?;

        // Pre-allocate computation buffers to eliminate per-frame allocations
        let num_workgroups = ComputeEnergyKernel::workgroup_count(num_edges);

        // Params buffer (GpuParams)
        let params_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("params_preallocated"),
            size: std::mem::size_of::<GpuParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Energy params buffer (EnergyParams)
        let energy_params_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("energy_params_preallocated"),
            size: std::mem::size_of::<EnergyParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Partial sums buffer (one f32 per workgroup)
        let partial_sums_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("partial_sums_preallocated"),
            size: ((num_workgroups as usize).max(1) * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Final params buffer (for second reduction pass)
        let final_params_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("final_params_preallocated"),
            size: std::mem::size_of::<EnergyParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Total energy buffer (single f32)
        let total_energy_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("total_energy_preallocated"),
            size: std::mem::size_of::<f32>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Staging buffer for edge energies readback
        let energies_staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("energies_staging_preallocated"),
            size: (num_edges as usize * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Staging buffer for total energy readback
        let total_staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("total_staging_preallocated"),
            size: std::mem::size_of::<f32>() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Store graph data with pre-allocated buffers
        self.graph_data = Some(GpuGraphData {
            num_nodes,
            num_edges,
            state_dim,
            node_id_map,
            edge_id_map,
            edge_id_reverse,
            params_buffer,
            energy_params_buffer,
            partial_sums_buffer,
            final_params_buffer,
            total_energy_buffer,
            energies_staging,
            total_staging,
            num_workgroups,
        });

        debug!(
            "Uploaded graph to GPU: {} nodes, {} edges, state_dim={}, workgroups={}",
            num_nodes, num_edges, state_dim, num_workgroups
        );

        Ok(())
    }

    /// Convert a RestrictionMap to GPU format
    fn convert_restriction_map(
        map: &crate::substrate::RestrictionMap,
        data: &mut Vec<f32>,
    ) -> GpuRestrictionMap {
        let data_offset = data.len() as u32;

        let (map_type, data_len) = match &map.matrix {
            MatrixStorage::Identity => (0, 0),
            MatrixStorage::Diagonal(scales) => {
                data.extend(scales.iter().cloned());
                (1, scales.len() as u32)
            }
            MatrixStorage::Projection { indices, .. } => {
                data.extend(indices.iter().map(|&i| i as f32));
                (2, indices.len() as u32)
            }
            MatrixStorage::Sparse { values, .. } => {
                // Simplified: just store values (would need row/col in practice)
                data.extend(values.iter().cloned());
                (3, values.len() as u32)
            }
            MatrixStorage::Csr(csr) => {
                // CSR format: store values similar to sparse
                // Note: In practice, GPU would need row_ptr and col_indices too
                data.extend(csr.values.iter().cloned());
                (3, csr.values.len() as u32)
            }
            MatrixStorage::Dense {
                data: matrix_data, ..
            } => {
                data.extend(matrix_data.iter().cloned());
                (3, matrix_data.len() as u32)
            }
        };

        GpuRestrictionMap {
            map_type,
            input_dim: map.input_dim() as u32,
            output_dim: map.output_dim() as u32,
            data_offset,
            data_len,
            _padding: [0; 3],
        }
    }

    /// Compute coherence energy on GPU
    /// Uses pre-allocated buffers to eliminate per-frame allocations
    pub async fn compute_energy(&mut self) -> GpuResult<GpuCoherenceEnergy> {
        let start = std::time::Instant::now();

        let graph_data = self
            .graph_data
            .as_ref()
            .ok_or_else(|| GpuError::Internal("Graph not uploaded".into()))?;

        let num_edges = graph_data.num_edges;
        let num_workgroups = graph_data.num_workgroups;

        // Write params to pre-allocated buffer (no allocation)
        let params = GpuParams {
            num_edges,
            num_nodes: graph_data.num_nodes,
            state_dim: graph_data.state_dim,
            beta: self.config.beta,
            threshold_lane0: self.config.threshold_lane0,
            threshold_lane1: self.config.threshold_lane1,
            threshold_lane2: self.config.threshold_lane2,
            store_residuals: 1, // Store residuals by default for gradient computation
        };
        self.queue
            .write_buffer(&graph_data.params_buffer, 0, bytemuck::bytes_of(&params));

        // Write energy params to pre-allocated buffer (no allocation)
        let energy_params = EnergyParams {
            num_elements: num_edges,
            _padding: [0; 7],
        };
        self.queue.write_buffer(
            &graph_data.energy_params_buffer,
            0,
            bytemuck::bytes_of(&energy_params),
        );

        // Get managed buffers for bind group creation
        let node_states_buf = self
            .buffer_manager
            .get("node_states")
            .ok_or_else(|| GpuError::Internal("Node states buffer not found".into()))?;
        let edges_buf = self
            .buffer_manager
            .get("edges")
            .ok_or_else(|| GpuError::Internal("Edges buffer not found".into()))?;
        let restriction_maps_buf = self
            .buffer_manager
            .get("restriction_maps")
            .ok_or_else(|| GpuError::Internal("Restriction maps buffer not found".into()))?;
        let restriction_data_buf = self
            .buffer_manager
            .get("restriction_data")
            .ok_or_else(|| GpuError::Internal("Restriction data buffer not found".into()))?;
        let residuals_buf = self
            .buffer_manager
            .get("residuals")
            .ok_or_else(|| GpuError::Internal("Residuals buffer not found".into()))?;
        let energies_buf = self
            .buffer_manager
            .get("edge_energies")
            .ok_or_else(|| GpuError::Internal("Edge energies buffer not found".into()))?;

        // Create bind group for residuals kernel using pre-allocated params buffer
        let residuals_bind_group = self.residuals_kernel.create_bind_group_raw(
            &self.device,
            &graph_data.params_buffer,
            &node_states_buf.buffer,
            &edges_buf.buffer,
            &restriction_maps_buf.buffer,
            &restriction_data_buf.buffer,
            &residuals_buf.buffer,
            &energies_buf.buffer,
        );

        // Create bind group for energy reduction using pre-allocated buffers
        let energy_bind_group = self.energy_kernel.create_bind_group_raw(
            &self.device,
            &graph_data.energy_params_buffer,
            &energies_buf.buffer,
            &graph_data.partial_sums_buffer,
        );

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("compute_energy_encoder"),
            });

        // Dispatch residuals computation
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute_residuals_pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(self.residuals_kernel.pipeline());
            compute_pass.set_bind_group(0, &residuals_bind_group, &[]);
            compute_pass.dispatch_workgroups(
                ComputeResidualsKernel::workgroup_count(num_edges),
                1,
                1,
            );
        }

        // Dispatch energy reduction
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute_energy_pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(self.energy_kernel.main_pipeline());
            compute_pass.set_bind_group(0, &energy_bind_group, &[]);
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        // If we have multiple workgroups, do final reduction
        if num_workgroups > 1 {
            // Write final params to pre-allocated buffer (no allocation)
            let final_params = EnergyParams {
                num_elements: num_workgroups,
                _padding: [0; 7],
            };
            self.queue.write_buffer(
                &graph_data.final_params_buffer,
                0,
                bytemuck::bytes_of(&final_params),
            );

            let final_bind_group = self.energy_kernel.create_bind_group_raw(
                &self.device,
                &graph_data.final_params_buffer,
                &graph_data.partial_sums_buffer,
                &graph_data.total_energy_buffer,
            );

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("final_reduce_pass"),
                    timestamp_writes: None,
                });

                compute_pass.set_pipeline(self.energy_kernel.final_pipeline());
                compute_pass.set_bind_group(0, &final_bind_group, &[]);
                compute_pass.dispatch_workgroups(1, 1, 1);
            }
        }

        // Copy results to pre-allocated staging buffers (no allocation)
        encoder.copy_buffer_to_buffer(
            &energies_buf.buffer,
            0,
            &graph_data.energies_staging,
            0,
            (num_edges as usize * std::mem::size_of::<f32>()) as u64,
        );

        if num_workgroups > 1 {
            encoder.copy_buffer_to_buffer(
                &graph_data.total_energy_buffer,
                0,
                &graph_data.total_staging,
                0,
                std::mem::size_of::<f32>() as u64,
            );
        } else {
            encoder.copy_buffer_to_buffer(
                &graph_data.partial_sums_buffer,
                0,
                &graph_data.total_staging,
                0,
                std::mem::size_of::<f32>() as u64,
            );
        }

        // Submit commands
        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back results from pre-allocated staging buffers
        let edge_energies = Self::read_buffer_f32(
            &self.device,
            &graph_data.energies_staging,
            num_edges as usize,
        )
        .await?;
        let total_energy =
            Self::read_buffer_f32(&self.device, &graph_data.total_staging, 1).await?[0];

        let compute_time_us = start.elapsed().as_micros() as u64;

        debug!(
            "GPU energy computation: total={:.6}, {} edges, {}us (pre-allocated buffers)",
            total_energy, num_edges, compute_time_us
        );

        Ok(GpuCoherenceEnergy {
            total_energy,
            edge_energies,
            edge_indices: graph_data.edge_id_reverse.clone(),
            compute_time_us,
            used_gpu: true,
        })
    }

    /// Read f32 buffer back to CPU
    async fn read_buffer_f32(
        device: &Device,
        buffer: &wgpu::Buffer,
        count: usize,
    ) -> GpuResult<Vec<f32>> {
        let buffer_slice = buffer.slice(..);

        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        device.poll(wgpu::Maintain::Wait);

        receiver
            .await
            .map_err(|_| GpuError::BufferRead("Channel closed".into()))?
            .map_err(|e| GpuError::BufferRead(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();
        let result: Vec<f32> =
            bytemuck::cast_slice(&data[..count * std::mem::size_of::<f32>()]).to_vec();

        drop(data);
        buffer.unmap();

        Ok(result)
    }

    /// Get GPU capabilities
    pub fn capabilities(&self) -> &GpuCapabilities {
        &self.capabilities
    }

    /// Get configuration
    pub fn config(&self) -> &GpuConfig {
        &self.config
    }

    /// Check if GPU is available
    pub fn is_available(&self) -> bool {
        self.capabilities.supported
    }

    /// Release all GPU resources
    pub fn release(&mut self) {
        self.buffer_manager.clear();
        self.graph_data = None;
    }
}

/// Synchronous wrapper for GPU coherence engine using pollster
pub mod sync {
    use super::*;

    /// Synchronously create a GPU engine
    pub fn create_engine(config: GpuConfig) -> GpuResult<GpuCoherenceEngine> {
        pollster::block_on(GpuCoherenceEngine::new(config))
    }

    /// Try to create GPU engine synchronously
    pub fn try_create_engine(config: GpuConfig) -> Option<GpuCoherenceEngine> {
        pollster::block_on(GpuCoherenceEngine::try_new(config))
    }

    /// Compute energy synchronously
    pub fn compute_energy(engine: &mut GpuCoherenceEngine) -> GpuResult<GpuCoherenceEnergy> {
        pollster::block_on(engine.compute_energy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_config_default() {
        let config = GpuConfig::default();
        assert!(config.enable_fallback);
        assert_eq!(config.beta, 1.0);
        assert!(config.threshold_lane0 < config.threshold_lane1);
        assert!(config.threshold_lane1 < config.threshold_lane2);
    }

    #[test]
    fn test_gpu_params_size() {
        assert_eq!(std::mem::size_of::<GpuParams>(), 32);
    }

    #[test]
    fn test_energy_params_size() {
        assert_eq!(std::mem::size_of::<EnergyParams>(), 32);
    }
}
