//! GPU device initialization and management.
//!
//! This module provides the core GPU device abstraction using wgpu,
//! handling adapter selection, device creation, and queue management.

use std::sync::Arc;
use tracing::{debug, info, warn};
use wgpu::{Adapter, Device, Instance, Queue};

use super::error::{GpuError, GpuResult};

/// Information about the GPU device
#[derive(Debug, Clone)]
pub struct GpuDeviceInfo {
    /// Device name
    pub name: String,
    /// Vendor ID
    pub vendor: u32,
    /// Device ID
    pub device_id: u32,
    /// Device type (discrete, integrated, etc.)
    pub device_type: String,
    /// Backend API (Vulkan, Metal, DX12, etc.)
    pub backend: String,
    /// Maximum buffer size
    pub max_buffer_size: u64,
    /// Maximum compute workgroup size per dimension
    pub max_workgroup_size: [u32; 3],
    /// Maximum compute workgroups per dimension
    pub max_workgroups: [u32; 3],
    /// Maximum storage buffers per shader stage
    pub max_storage_buffers: u32,
}

/// GPU device wrapper providing access to wgpu resources
pub struct GpuDevice {
    instance: Instance,
    adapter: Adapter,
    device: Arc<Device>,
    queue: Arc<Queue>,
    info: GpuDeviceInfo,
}

impl GpuDevice {
    /// Create a new GPU device with default configuration.
    ///
    /// This will:
    /// 1. Create a wgpu instance with all available backends
    /// 2. Request a high-performance adapter
    /// 3. Create the device and queue
    ///
    /// # Errors
    ///
    /// Returns `GpuError::NoAdapter` if no suitable GPU is found.
    /// Returns `GpuError::DeviceRequestFailed` if device creation fails.
    pub async fn new() -> GpuResult<Self> {
        Self::with_options(GpuDeviceOptions::default()).await
    }

    /// Create a new GPU device with custom options.
    pub async fn with_options(options: GpuDeviceOptions) -> GpuResult<Self> {
        let instance = Instance::new(wgpu::InstanceDescriptor {
            backends: options.backends,
            flags: wgpu::InstanceFlags::default(),
            dx12_shader_compiler: wgpu::Dx12Compiler::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::default(),
        });

        debug!(
            "Created wgpu instance with backends: {:?}",
            options.backends
        );

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: options.power_preference,
                compatible_surface: None,
                force_fallback_adapter: options.force_fallback,
            })
            .await
            .ok_or(GpuError::NoAdapter)?;

        let adapter_info = adapter.get_info();
        info!(
            "Selected GPU adapter: {} ({:?})",
            adapter_info.name, adapter_info.backend
        );

        let limits = if options.use_downlevel_limits {
            wgpu::Limits::downlevel_defaults()
        } else {
            wgpu::Limits::default()
        };

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("prime-radiant-gpu"),
                    required_features: options.required_features,
                    required_limits: limits.clone(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await?;

        // Set up error handling
        device.on_uncaptured_error(Box::new(|error| {
            warn!("Uncaptured GPU error: {:?}", error);
        }));

        let info = GpuDeviceInfo {
            name: adapter_info.name.clone(),
            vendor: adapter_info.vendor,
            device_id: adapter_info.device,
            device_type: format!("{:?}", adapter_info.device_type),
            backend: format!("{:?}", adapter_info.backend),
            max_buffer_size: limits.max_buffer_size as u64,
            max_workgroup_size: [
                limits.max_compute_workgroup_size_x,
                limits.max_compute_workgroup_size_y,
                limits.max_compute_workgroup_size_z,
            ],
            max_workgroups: [
                limits.max_compute_workgroups_per_dimension,
                limits.max_compute_workgroups_per_dimension,
                limits.max_compute_workgroups_per_dimension,
            ],
            max_storage_buffers: limits.max_storage_buffers_per_shader_stage,
        };

        debug!("GPU device info: {:?}", info);

        Ok(Self {
            instance,
            adapter,
            device: Arc::new(device),
            queue: Arc::new(queue),
            info,
        })
    }

    /// Get a reference to the wgpu device
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Get a shared reference to the wgpu device
    pub fn device_arc(&self) -> Arc<Device> {
        Arc::clone(&self.device)
    }

    /// Get a reference to the command queue
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Get a shared reference to the command queue
    pub fn queue_arc(&self) -> Arc<Queue> {
        Arc::clone(&self.queue)
    }

    /// Get device information
    pub fn info(&self) -> &GpuDeviceInfo {
        &self.info
    }

    /// Get the wgpu instance
    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    /// Get the wgpu adapter
    pub fn adapter(&self) -> &Adapter {
        &self.adapter
    }

    /// Check if a feature is supported
    pub fn supports_feature(&self, feature: wgpu::Features) -> bool {
        self.adapter.features().contains(feature)
    }

    /// Poll the device for completed work.
    ///
    /// This is useful when you need to ensure GPU work has completed
    /// before continuing on the CPU.
    pub fn poll(&self, wait: bool) -> bool {
        self.device
            .poll(if wait {
                wgpu::Maintain::Wait
            } else {
                wgpu::Maintain::Poll
            })
            .is_queue_empty()
    }

    /// Submit a command buffer to the queue
    pub fn submit(&self, command_buffer: wgpu::CommandBuffer) -> wgpu::SubmissionIndex {
        self.queue.submit(std::iter::once(command_buffer))
    }

    /// Submit multiple command buffers to the queue
    pub fn submit_multiple(
        &self,
        command_buffers: impl IntoIterator<Item = wgpu::CommandBuffer>,
    ) -> wgpu::SubmissionIndex {
        self.queue.submit(command_buffers)
    }
}

/// Options for GPU device creation
#[derive(Debug, Clone)]
pub struct GpuDeviceOptions {
    /// Backends to use (default: all)
    pub backends: wgpu::Backends,
    /// Power preference (default: high performance)
    pub power_preference: wgpu::PowerPreference,
    /// Required GPU features
    pub required_features: wgpu::Features,
    /// Use downlevel limits for broader compatibility
    pub use_downlevel_limits: bool,
    /// Force fallback adapter (software rendering)
    pub force_fallback: bool,
}

impl Default for GpuDeviceOptions {
    fn default() -> Self {
        Self {
            backends: wgpu::Backends::all(),
            power_preference: wgpu::PowerPreference::HighPerformance,
            required_features: wgpu::Features::empty(),
            use_downlevel_limits: false,
            force_fallback: false,
        }
    }
}

impl GpuDeviceOptions {
    /// Create options for low-power mode (integrated GPU preferred)
    pub fn low_power() -> Self {
        Self {
            power_preference: wgpu::PowerPreference::LowPower,
            ..Default::default()
        }
    }

    /// Create options for maximum compatibility
    pub fn compatible() -> Self {
        Self {
            use_downlevel_limits: true,
            ..Default::default()
        }
    }

    /// Create options for software fallback
    pub fn software() -> Self {
        Self {
            force_fallback: true,
            use_downlevel_limits: true,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_options_default() {
        let options = GpuDeviceOptions::default();
        assert_eq!(
            options.power_preference,
            wgpu::PowerPreference::HighPerformance
        );
        assert!(!options.force_fallback);
    }

    #[test]
    fn test_device_options_low_power() {
        let options = GpuDeviceOptions::low_power();
        assert_eq!(options.power_preference, wgpu::PowerPreference::LowPower);
    }

    #[test]
    fn test_device_options_compatible() {
        let options = GpuDeviceOptions::compatible();
        assert!(options.use_downlevel_limits);
    }
}
