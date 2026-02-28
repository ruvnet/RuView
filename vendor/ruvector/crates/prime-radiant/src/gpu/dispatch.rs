//! Kernel dispatch and synchronization for GPU compute operations.
//!
//! This module provides the dispatcher for executing compute kernels on the GPU,
//! including support for:
//! - Single kernel dispatch
//! - Indirect dispatch (workgroup count from GPU buffer)
//! - Chained dispatch for fused kernels
//! - Synchronization and timing

use std::sync::Arc;
use tracing::{debug, trace};
use wgpu::{CommandEncoder, Device, Queue};

use super::buffer::{GpuBuffer, GpuBufferPool};
use super::device::GpuDevice;
use super::error::{GpuError, GpuResult};
use super::pipeline::{ComputePipeline, PipelineCache};

/// Configuration for a dispatch operation
#[derive(Debug, Clone)]
pub struct DispatchConfig {
    /// Label for debugging
    pub label: Option<String>,
    /// Whether to wait for completion
    pub wait: bool,
    /// Timeout in milliseconds (0 = no timeout)
    pub timeout_ms: u64,
}

impl Default for DispatchConfig {
    fn default() -> Self {
        Self {
            label: None,
            wait: false,
            timeout_ms: 0,
        }
    }
}

impl DispatchConfig {
    /// Create a config that waits for completion
    pub fn wait() -> Self {
        Self {
            wait: true,
            ..Default::default()
        }
    }

    /// Create a config with a label
    pub fn with_label(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            ..Default::default()
        }
    }

    /// Set the timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set wait flag
    pub fn with_wait(mut self, wait: bool) -> Self {
        self.wait = wait;
        self
    }
}

/// GPU dispatcher for executing compute kernels
pub struct GpuDispatcher {
    device: Arc<GpuDevice>,
    pipeline_cache: PipelineCache,
    buffer_pool: GpuBufferPool,
}

impl GpuDispatcher {
    /// Create a new dispatcher
    pub fn new(device: Arc<GpuDevice>) -> Self {
        let pipeline_cache = PipelineCache::new(device.device_arc());
        let buffer_pool = GpuBufferPool::new(device.device_arc());

        Self {
            device,
            pipeline_cache,
            buffer_pool,
        }
    }

    /// Get the underlying GPU device
    pub fn device(&self) -> &GpuDevice {
        &self.device
    }

    /// Get the pipeline cache
    pub fn pipeline_cache(&self) -> &PipelineCache {
        &self.pipeline_cache
    }

    /// Get the buffer pool
    pub fn buffer_pool(&self) -> &GpuBufferPool {
        &self.buffer_pool
    }

    /// Dispatch a compute kernel.
    ///
    /// # Arguments
    ///
    /// * `pipeline` - The compute pipeline to execute
    /// * `bind_group` - The bind group with buffer bindings
    /// * `workgroups` - Number of workgroups [x, y, z]
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// dispatcher.dispatch(&pipeline, &bind_group, [4, 1, 1]).await?;
    /// ```
    pub async fn dispatch(
        &self,
        pipeline: &ComputePipeline,
        bind_group: &wgpu::BindGroup,
        workgroups: [u32; 3],
    ) -> GpuResult<()> {
        self.dispatch_with_config(pipeline, bind_group, workgroups, DispatchConfig::default())
            .await
    }

    /// Dispatch with custom configuration.
    pub async fn dispatch_with_config(
        &self,
        pipeline: &ComputePipeline,
        bind_group: &wgpu::BindGroup,
        workgroups: [u32; 3],
        config: DispatchConfig,
    ) -> GpuResult<()> {
        // Validate workgroup count
        let limits = &self.device.info().max_workgroups;
        if workgroups[0] > limits[0] || workgroups[1] > limits[1] || workgroups[2] > limits[2] {
            return Err(GpuError::InvalidWorkgroupSize {
                x: workgroups[0],
                y: workgroups[1],
                z: workgroups[2],
            });
        }

        let label = config.label.as_deref().unwrap_or("dispatch");
        debug!(
            "Dispatching '{}' with workgroups [{}, {}, {}]",
            label, workgroups[0], workgroups[1], workgroups[2]
        );

        let mut encoder = self
            .device
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(label),
                timestamp_writes: None,
            });

            pass.set_pipeline(pipeline.pipeline());
            pass.set_bind_group(0, Some(bind_group), &[]);
            pass.dispatch_workgroups(workgroups[0], workgroups[1], workgroups[2]);
        }

        self.device.submit(encoder.finish());

        if config.wait {
            self.device.poll(true);
        }

        Ok(())
    }

    /// Dispatch using indirect workgroup count from a buffer.
    ///
    /// The indirect buffer must contain [x, y, z] workgroup counts as u32.
    pub async fn dispatch_indirect(
        &self,
        pipeline: &ComputePipeline,
        bind_group: &wgpu::BindGroup,
        indirect_buffer: &GpuBuffer,
    ) -> GpuResult<()> {
        self.dispatch_indirect_with_config(
            pipeline,
            bind_group,
            indirect_buffer,
            0,
            DispatchConfig::default(),
        )
        .await
    }

    /// Dispatch indirect with offset and configuration.
    pub async fn dispatch_indirect_with_config(
        &self,
        pipeline: &ComputePipeline,
        bind_group: &wgpu::BindGroup,
        indirect_buffer: &GpuBuffer,
        indirect_offset: u64,
        config: DispatchConfig,
    ) -> GpuResult<()> {
        let label = config.label.as_deref().unwrap_or("dispatch_indirect");
        debug!("Dispatching indirect '{}'", label);

        let mut encoder = self
            .device
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(label),
                timestamp_writes: None,
            });

            pass.set_pipeline(pipeline.pipeline());
            pass.set_bind_group(0, Some(bind_group), &[]);
            pass.dispatch_workgroups_indirect(indirect_buffer.buffer(), indirect_offset);
        }

        self.device.submit(encoder.finish());

        if config.wait {
            self.device.poll(true);
        }

        Ok(())
    }

    /// Dispatch multiple kernels in a chain (fused execution).
    ///
    /// All dispatches are recorded into a single command buffer for
    /// optimal GPU utilization.
    ///
    /// # Arguments
    ///
    /// * `dispatches` - List of (pipeline, bind_group, workgroups) tuples
    pub async fn dispatch_chain(
        &self,
        dispatches: &[(&ComputePipeline, &wgpu::BindGroup, [u32; 3])],
    ) -> GpuResult<()> {
        self.dispatch_chain_with_config(dispatches, DispatchConfig::default())
            .await
    }

    /// Dispatch chain with custom configuration.
    pub async fn dispatch_chain_with_config(
        &self,
        dispatches: &[(&ComputePipeline, &wgpu::BindGroup, [u32; 3])],
        config: DispatchConfig,
    ) -> GpuResult<()> {
        if dispatches.is_empty() {
            return Ok(());
        }

        let label = config.label.as_deref().unwrap_or("dispatch_chain");
        debug!(
            "Dispatching chain '{}' with {} kernels",
            label,
            dispatches.len()
        );

        let mut encoder = self
            .device
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });

        for (i, (pipeline, bind_group, workgroups)) in dispatches.iter().enumerate() {
            trace!(
                "Chain dispatch {}: workgroups [{}, {}, {}]",
                i,
                workgroups[0],
                workgroups[1],
                workgroups[2]
            );

            let pass_label = format!("{}_pass_{}", label, i);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(&pass_label),
                timestamp_writes: None,
            });

            pass.set_pipeline(pipeline.pipeline());
            pass.set_bind_group(0, Some(*bind_group), &[]);
            pass.dispatch_workgroups(workgroups[0], workgroups[1], workgroups[2]);
        }

        self.device.submit(encoder.finish());

        if config.wait {
            self.device.poll(true);
        }

        Ok(())
    }

    /// Record dispatches to a command encoder without submitting.
    ///
    /// This is useful when you want to combine compute with other operations.
    pub fn record_dispatch(
        &self,
        encoder: &mut CommandEncoder,
        pipeline: &ComputePipeline,
        bind_group: &wgpu::BindGroup,
        workgroups: [u32; 3],
        label: Option<&str>,
    ) {
        let pass_label = label.unwrap_or("recorded_dispatch");

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(pass_label),
            timestamp_writes: None,
        });

        pass.set_pipeline(pipeline.pipeline());
        pass.set_bind_group(0, Some(bind_group), &[]);
        pass.dispatch_workgroups(workgroups[0], workgroups[1], workgroups[2]);
    }

    /// Wait for all pending GPU work to complete.
    pub fn synchronize(&self) {
        self.device.poll(true);
    }

    /// Poll for completed work without blocking.
    pub fn poll(&self) -> bool {
        self.device.poll(false)
    }
}

/// Builder for constructing complex dispatch operations
pub struct DispatchBuilder<'a> {
    dispatcher: &'a GpuDispatcher,
    dispatches: Vec<(Arc<ComputePipeline>, wgpu::BindGroup, [u32; 3])>,
    config: DispatchConfig,
}

impl<'a> DispatchBuilder<'a> {
    /// Create a new dispatch builder
    pub fn new(dispatcher: &'a GpuDispatcher) -> Self {
        Self {
            dispatcher,
            dispatches: Vec::new(),
            config: DispatchConfig::default(),
        }
    }

    /// Add a dispatch to the chain
    pub fn add(
        mut self,
        pipeline: Arc<ComputePipeline>,
        bind_group: wgpu::BindGroup,
        workgroups: [u32; 3],
    ) -> Self {
        self.dispatches.push((pipeline, bind_group, workgroups));
        self
    }

    /// Set the configuration
    pub fn config(mut self, config: DispatchConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the label
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.config.label = Some(label.into());
        self
    }

    /// Set wait flag
    pub fn wait(mut self) -> Self {
        self.config.wait = true;
        self
    }

    /// Execute all dispatches
    pub async fn execute(self) -> GpuResult<()> {
        if self.dispatches.is_empty() {
            return Ok(());
        }

        let refs: Vec<(&ComputePipeline, &wgpu::BindGroup, [u32; 3])> = self
            .dispatches
            .iter()
            .map(|(p, b, w)| (p.as_ref(), b, *w))
            .collect();

        self.dispatcher
            .dispatch_chain_with_config(&refs, self.config)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_config_default() {
        let config = DispatchConfig::default();
        assert!(!config.wait);
        assert!(config.label.is_none());
        assert_eq!(config.timeout_ms, 0);
    }

    #[test]
    fn test_dispatch_config_wait() {
        let config = DispatchConfig::wait();
        assert!(config.wait);
    }

    #[test]
    fn test_dispatch_config_builder() {
        let config = DispatchConfig::with_label("test")
            .with_timeout(1000)
            .with_wait(true);

        assert_eq!(config.label.as_deref(), Some("test"));
        assert_eq!(config.timeout_ms, 1000);
        assert!(config.wait);
    }
}
