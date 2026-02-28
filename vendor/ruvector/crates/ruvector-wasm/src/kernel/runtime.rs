//! Wasmtime Runtime Integration
//!
//! Provides the runtime traits and implementations for executing
//! WASM kernels with Wasmtime.

use crate::kernel::epoch::{EpochConfig, EpochController, EpochDeadline};
use crate::kernel::error::{KernelError, KernelErrorCode, KernelResult};
use crate::kernel::manifest::{KernelDescriptor, KernelInfo, KernelManifest, ResourceLimits};
use crate::kernel::memory::{MemoryLayoutValidator, SharedMemoryProtocol, PAGE_SIZE};
use std::collections::HashMap;
use std::sync::Arc;

/// Runtime configuration for WASM kernel execution
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Epoch configuration
    pub epoch: EpochConfig,

    /// Enable SIMD support
    pub enable_simd: bool,

    /// Enable bulk memory operations
    pub enable_bulk_memory: bool,

    /// Enable multi-value returns
    pub enable_multi_value: bool,

    /// Maximum memory pages per instance
    pub max_memory_pages: u32,

    /// Enable parallel compilation
    pub parallel_compilation: bool,

    /// Optimization level (0-3, where 0=none, 3=maximum)
    pub optimization_level: u8,

    /// Enable instance pooling for reuse
    pub enable_instance_pooling: bool,

    /// Pool size for instance reuse
    pub instance_pool_size: usize,
}

impl RuntimeConfig {
    /// Create configuration for server workloads
    pub fn server() -> Self {
        RuntimeConfig {
            epoch: EpochConfig::server(),
            enable_simd: true,
            enable_bulk_memory: true,
            enable_multi_value: true,
            max_memory_pages: 1024, // 64MB max
            parallel_compilation: true,
            optimization_level: 3,
            enable_instance_pooling: true,
            instance_pool_size: 16,
        }
    }

    /// Create configuration for embedded/constrained workloads
    pub fn embedded() -> Self {
        RuntimeConfig {
            epoch: EpochConfig::embedded(),
            enable_simd: false, // Often unavailable
            enable_bulk_memory: true,
            enable_multi_value: true,
            max_memory_pages: 64, // 4MB max
            parallel_compilation: false,
            optimization_level: 2,
            enable_instance_pooling: false,
            instance_pool_size: 0,
        }
    }

    /// Create configuration for development/debugging
    pub fn development() -> Self {
        RuntimeConfig {
            epoch: EpochConfig::disabled(),
            enable_simd: true,
            enable_bulk_memory: true,
            enable_multi_value: true,
            max_memory_pages: 1024,
            parallel_compilation: true,
            optimization_level: 0, // Fast compilation
            enable_instance_pooling: false,
            instance_pool_size: 0,
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::server()
    }
}

/// Compiled WASM kernel module
#[derive(Debug)]
pub struct CompiledKernel {
    /// Kernel ID
    pub id: String,
    /// Kernel info from manifest
    pub info: KernelInfo,
    /// Compiled module bytes (for caching)
    pub compiled_bytes: Vec<u8>,
    /// Whether module uses SIMD
    pub uses_simd: bool,
    /// Required memory pages
    pub required_pages: u32,
}

/// WASM kernel instance ready for execution
pub struct WasmKernelInstance {
    /// Kernel ID
    kernel_id: String,
    /// Memory allocated for this instance
    memory_pages: u32,
    /// Epoch deadline for this invocation
    deadline: Option<EpochDeadline>,
    /// Memory validator
    validator: MemoryLayoutValidator,
}

impl WasmKernelInstance {
    /// Create a new kernel instance
    pub fn new(kernel_id: String, memory_pages: u32) -> Self {
        WasmKernelInstance {
            kernel_id,
            memory_pages,
            deadline: None,
            validator: MemoryLayoutValidator::new(),
        }
    }

    /// Set execution deadline
    pub fn set_deadline(&mut self, deadline: EpochDeadline) {
        self.deadline = Some(deadline);
    }

    /// Get kernel ID
    pub fn kernel_id(&self) -> &str {
        &self.kernel_id
    }

    /// Get allocated memory pages
    pub fn memory_pages(&self) -> u32 {
        self.memory_pages
    }

    /// Get memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.memory_pages as usize * PAGE_SIZE
    }

    /// Validate a descriptor before execution
    pub fn validate_descriptor(&self, desc: &KernelDescriptor) -> KernelResult<()> {
        self.validator.validate_descriptor(desc, self.memory_size())
    }

    /// Check if deadline exceeded (if set)
    pub fn check_deadline(&self, controller: &EpochController) -> bool {
        if let Some(deadline) = &self.deadline {
            deadline.is_exceeded(controller.current())
        } else {
            false
        }
    }
}

impl std::fmt::Debug for WasmKernelInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmKernelInstance")
            .field("kernel_id", &self.kernel_id)
            .field("memory_pages", &self.memory_pages)
            .field("deadline", &self.deadline)
            .finish()
    }
}

/// Trait for kernel runtime implementations
pub trait KernelRuntime: Send + Sync {
    /// Load and compile a kernel from WASM bytes
    fn compile_kernel(
        &self,
        id: &str,
        wasm_bytes: &[u8],
        info: &KernelInfo,
    ) -> KernelResult<CompiledKernel>;

    /// Create an instance of a compiled kernel
    fn instantiate(&self, kernel: &CompiledKernel) -> KernelResult<WasmKernelInstance>;

    /// Execute a kernel with the given descriptor
    fn execute(
        &self,
        instance: &mut WasmKernelInstance,
        descriptor: &KernelDescriptor,
        memory: &mut [u8],
    ) -> KernelResult<()>;

    /// Get runtime configuration
    fn config(&self) -> &RuntimeConfig;

    /// Get epoch controller
    fn epoch_controller(&self) -> &EpochController;

    /// Increment epoch (should be called periodically)
    fn tick(&self) {
        self.epoch_controller().increment();
    }
}

/// Mock runtime for testing without Wasmtime dependency
#[derive(Debug)]
pub struct MockKernelRuntime {
    config: RuntimeConfig,
    epoch_controller: EpochController,
    /// Registered kernel behaviors for testing
    kernel_behaviors: HashMap<String, MockKernelBehavior>,
}

/// Mock kernel behavior for testing
#[derive(Debug, Clone)]
pub enum MockKernelBehavior {
    /// Always succeed
    Success,
    /// Always fail with error code
    Fail(KernelErrorCode),
    /// Timeout (exceed epoch)
    Timeout,
    /// Return specific output data
    ReturnData(Vec<u8>),
}

impl MockKernelRuntime {
    /// Create a new mock runtime
    pub fn new(config: RuntimeConfig) -> Self {
        MockKernelRuntime {
            epoch_controller: EpochController::new(config.epoch.tick_interval()),
            config,
            kernel_behaviors: HashMap::new(),
        }
    }

    /// Register a mock behavior for a kernel
    pub fn register_behavior(&mut self, kernel_id: &str, behavior: MockKernelBehavior) {
        self.kernel_behaviors
            .insert(kernel_id.to_string(), behavior);
    }
}

impl KernelRuntime for MockKernelRuntime {
    fn compile_kernel(
        &self,
        id: &str,
        _wasm_bytes: &[u8],
        info: &KernelInfo,
    ) -> KernelResult<CompiledKernel> {
        Ok(CompiledKernel {
            id: id.to_string(),
            info: info.clone(),
            compiled_bytes: vec![], // No actual compilation
            uses_simd: false,
            required_pages: info.resource_limits.max_memory_pages,
        })
    }

    fn instantiate(&self, kernel: &CompiledKernel) -> KernelResult<WasmKernelInstance> {
        Ok(WasmKernelInstance::new(
            kernel.id.clone(),
            kernel.required_pages,
        ))
    }

    fn execute(
        &self,
        instance: &mut WasmKernelInstance,
        descriptor: &KernelDescriptor,
        memory: &mut [u8],
    ) -> KernelResult<()> {
        // Validate descriptor first
        instance.validate_descriptor(descriptor)?;

        // Check deadline
        if instance.check_deadline(&self.epoch_controller) {
            return Err(KernelError::EpochDeadline);
        }

        // Look up mock behavior
        let behavior = self
            .kernel_behaviors
            .get(instance.kernel_id())
            .cloned()
            .unwrap_or(MockKernelBehavior::Success);

        match behavior {
            MockKernelBehavior::Success => Ok(()),
            MockKernelBehavior::Fail(code) => Err(KernelError::KernelTrap {
                code: code as u32,
                message: Some(code.to_string()),
            }),
            MockKernelBehavior::Timeout => Err(KernelError::EpochDeadline),
            MockKernelBehavior::ReturnData(data) => {
                // Copy data to output region
                let out_start = descriptor.output_offset as usize;
                let out_end = out_start + descriptor.output_size.min(data.len() as u32) as usize;
                if out_end <= memory.len() {
                    let copy_len = (out_end - out_start).min(data.len());
                    memory[out_start..out_start + copy_len].copy_from_slice(&data[..copy_len]);
                }
                Ok(())
            }
        }
    }

    fn config(&self) -> &RuntimeConfig {
        &self.config
    }

    fn epoch_controller(&self) -> &EpochController {
        &self.epoch_controller
    }
}

/// Kernel manager for loading and executing kernel packs
pub struct KernelManager<R: KernelRuntime> {
    /// Runtime implementation
    runtime: Arc<R>,
    /// Loaded manifests
    manifests: HashMap<String, KernelManifest>,
    /// Compiled kernels
    compiled_kernels: HashMap<String, CompiledKernel>,
    /// Active kernel pack
    active_pack: Option<String>,
}

impl<R: KernelRuntime> KernelManager<R> {
    /// Create a new kernel manager
    pub fn new(runtime: Arc<R>) -> Self {
        KernelManager {
            runtime,
            manifests: HashMap::new(),
            compiled_kernels: HashMap::new(),
            active_pack: None,
        }
    }

    /// Load a kernel pack manifest
    pub fn load_manifest(&mut self, pack_name: &str, manifest: KernelManifest) {
        self.manifests.insert(pack_name.to_string(), manifest);
    }

    /// Compile a kernel from a loaded pack
    pub fn compile_kernel(
        &mut self,
        pack_name: &str,
        kernel_id: &str,
        wasm_bytes: &[u8],
    ) -> KernelResult<()> {
        let manifest =
            self.manifests
                .get(pack_name)
                .ok_or_else(|| KernelError::KernelNotFound {
                    kernel_id: format!("pack:{}", pack_name),
                })?;

        let info = manifest
            .get_kernel(kernel_id)
            .ok_or_else(|| KernelError::KernelNotFound {
                kernel_id: kernel_id.to_string(),
            })?;

        let compiled = self.runtime.compile_kernel(kernel_id, wasm_bytes, info)?;
        self.compiled_kernels
            .insert(kernel_id.to_string(), compiled);

        Ok(())
    }

    /// Set the active kernel pack
    pub fn set_active_pack(&mut self, pack_name: &str) -> KernelResult<()> {
        if self.manifests.contains_key(pack_name) {
            self.active_pack = Some(pack_name.to_string());
            Ok(())
        } else {
            Err(KernelError::KernelNotFound {
                kernel_id: format!("pack:{}", pack_name),
            })
        }
    }

    /// Execute a kernel
    pub fn execute(
        &self,
        kernel_id: &str,
        descriptor: &KernelDescriptor,
        memory: &mut [u8],
    ) -> KernelResult<()> {
        let compiled =
            self.compiled_kernels
                .get(kernel_id)
                .ok_or_else(|| KernelError::KernelNotFound {
                    kernel_id: kernel_id.to_string(),
                })?;

        let mut instance = self.runtime.instantiate(compiled)?;

        // Set deadline if epoch is enabled
        if self.runtime.config().epoch.enabled {
            let budget = compiled.info.resource_limits.max_epoch_ticks;
            let deadline = EpochDeadline::new(self.runtime.epoch_controller().current(), budget);
            instance.set_deadline(deadline);
        }

        self.runtime.execute(&mut instance, descriptor, memory)
    }

    /// Get kernel info
    pub fn get_kernel_info(&self, kernel_id: &str) -> Option<&KernelInfo> {
        self.compiled_kernels.get(kernel_id).map(|k| &k.info)
    }

    /// List compiled kernel IDs
    pub fn list_kernels(&self) -> Vec<&str> {
        self.compiled_kernels.keys().map(|s| s.as_str()).collect()
    }

    /// Get runtime reference
    pub fn runtime(&self) -> &R {
        &self.runtime
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::manifest::{DataType, KernelCategory, ResourceLimits, ShapeDim, TensorSpec};

    fn mock_kernel_info(id: &str) -> KernelInfo {
        KernelInfo {
            id: id.to_string(),
            name: format!("Test {}", id),
            category: KernelCategory::Custom,
            path: format!("{}.wasm", id),
            hash: "sha256:test".to_string(),
            entry_point: "kernel_forward".to_string(),
            inputs: vec![TensorSpec {
                name: "x".to_string(),
                dtype: DataType::F32,
                shape: vec![ShapeDim::Symbolic("batch".to_string())],
            }],
            outputs: vec![TensorSpec {
                name: "y".to_string(),
                dtype: DataType::F32,
                shape: vec![ShapeDim::Symbolic("batch".to_string())],
            }],
            params: HashMap::new(),
            resource_limits: ResourceLimits::default(),
            platforms: HashMap::new(),
            benchmarks: HashMap::new(),
        }
    }

    #[test]
    fn test_runtime_config() {
        let server = RuntimeConfig::server();
        assert!(server.enable_simd);
        assert_eq!(server.optimization_level, 3);

        let embedded = RuntimeConfig::embedded();
        assert!(!embedded.enable_simd);
        assert!(!embedded.parallel_compilation);

        let dev = RuntimeConfig::development();
        assert_eq!(dev.optimization_level, 0);
    }

    #[test]
    fn test_mock_runtime() {
        let mut runtime = MockKernelRuntime::new(RuntimeConfig::default());

        // Test success behavior
        runtime.register_behavior("test_kernel", MockKernelBehavior::Success);

        let info = mock_kernel_info("test_kernel");
        let compiled = runtime.compile_kernel("test_kernel", &[], &info).unwrap();
        let mut instance = runtime.instantiate(&compiled).unwrap();

        let mut desc = KernelDescriptor::new();
        desc.input_a_offset = 0;
        desc.input_a_size = 1024;
        desc.output_offset = 1024;
        desc.output_size = 1024;

        let mut memory = vec![0u8; 65536];
        let result = runtime.execute(&mut instance, &desc, &mut memory);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_runtime_failure() {
        let mut runtime = MockKernelRuntime::new(RuntimeConfig::default());
        runtime.register_behavior(
            "failing_kernel",
            MockKernelBehavior::Fail(KernelErrorCode::InvalidInput),
        );

        let info = mock_kernel_info("failing_kernel");
        let compiled = runtime
            .compile_kernel("failing_kernel", &[], &info)
            .unwrap();
        let mut instance = runtime.instantiate(&compiled).unwrap();

        let desc = KernelDescriptor::new();
        let mut memory = vec![0u8; 65536];
        let result = runtime.execute(&mut instance, &desc, &mut memory);
        assert!(matches!(result, Err(KernelError::KernelTrap { .. })));
    }

    #[test]
    fn test_wasm_kernel_instance() {
        let mut instance = WasmKernelInstance::new("test".to_string(), 256);

        assert_eq!(instance.kernel_id(), "test");
        assert_eq!(instance.memory_pages(), 256);
        assert_eq!(instance.memory_size(), 256 * PAGE_SIZE);

        // Test deadline
        let controller = EpochController::default_interval();
        let deadline = EpochDeadline::new(0, 100);
        instance.set_deadline(deadline);

        assert!(!instance.check_deadline(&controller));

        // Exceed deadline
        for _ in 0..100 {
            controller.increment();
        }
        assert!(instance.check_deadline(&controller));
    }

    #[test]
    fn test_kernel_manager() {
        let runtime = Arc::new(MockKernelRuntime::new(RuntimeConfig::default()));
        let mut manager = KernelManager::new(runtime);

        // Create a minimal manifest
        let manifest = KernelManifest {
            schema: String::new(),
            version: "1.0.0".to_string(),
            name: "test-pack".to_string(),
            description: "Test".to_string(),
            min_runtime_version: "0.1.0".to_string(),
            max_runtime_version: "1.0.0".to_string(),
            created_at: "2026-01-18T00:00:00Z".to_string(),
            author: crate::kernel::manifest::AuthorInfo {
                name: "Test".to_string(),
                email: "test@test.com".to_string(),
                signing_key: "test".to_string(),
            },
            kernels: vec![mock_kernel_info("rope_f32")],
            fallbacks: HashMap::new(),
        };

        manager.load_manifest("test-pack", manifest);
        manager.set_active_pack("test-pack").unwrap();

        // Compile kernel
        manager
            .compile_kernel("test-pack", "rope_f32", &[])
            .unwrap();

        assert_eq!(manager.list_kernels(), vec!["rope_f32"]);
    }
}
