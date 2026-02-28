//! Compute backend detection and abstraction
//!
//! Detects available compute capabilities (WebGPU, WebGL2, WebWorkers)
//! and provides a unified interface for selecting the best backend.

use wasm_bindgen::prelude::*;

/// Compute capabilities detected on the current device
#[derive(Clone, Debug)]
pub struct ComputeCapability {
    /// WebGPU is available (best performance)
    pub has_webgpu: bool,
    /// WebGL2 is available (fallback for GPU compute)
    pub has_webgl2: bool,
    /// WebGL2 supports floating point textures
    pub has_float_textures: bool,
    /// Transform feedback is available (for GPU readback)
    pub has_transform_feedback: bool,
    /// WebWorkers are available
    pub has_workers: bool,
    /// SharedArrayBuffer is available (for shared memory)
    pub has_shared_memory: bool,
    /// Number of logical CPU cores
    pub worker_count: usize,
    /// Maximum texture size (for WebGL2)
    pub max_texture_size: u32,
    /// Estimated GPU memory (MB)
    pub gpu_memory_mb: u32,
    /// Device description
    pub device_info: String,
}

impl ComputeCapability {
    /// Convert to JavaScript object
    pub fn to_js(&self) -> JsValue {
        let obj = js_sys::Object::new();

        js_sys::Reflect::set(&obj, &"hasWebGPU".into(), &self.has_webgpu.into()).ok();
        js_sys::Reflect::set(&obj, &"hasWebGL2".into(), &self.has_webgl2.into()).ok();
        js_sys::Reflect::set(&obj, &"hasFloatTextures".into(), &self.has_float_textures.into()).ok();
        js_sys::Reflect::set(&obj, &"hasTransformFeedback".into(), &self.has_transform_feedback.into()).ok();
        js_sys::Reflect::set(&obj, &"hasWorkers".into(), &self.has_workers.into()).ok();
        js_sys::Reflect::set(&obj, &"hasSharedMemory".into(), &self.has_shared_memory.into()).ok();
        js_sys::Reflect::set(&obj, &"workerCount".into(), &(self.worker_count as u32).into()).ok();
        js_sys::Reflect::set(&obj, &"maxTextureSize".into(), &self.max_texture_size.into()).ok();
        js_sys::Reflect::set(&obj, &"gpuMemoryMB".into(), &self.gpu_memory_mb.into()).ok();
        js_sys::Reflect::set(&obj, &"deviceInfo".into(), &self.device_info.clone().into()).ok();

        obj.into()
    }

    /// Get recommended backend for a given operation size
    pub fn recommend_backend(&self, operation_size: usize) -> ComputeBackend {
        // WebGPU is always preferred if available
        if self.has_webgpu {
            return ComputeBackend::WebGPU;
        }

        // For large operations, prefer GPU
        if operation_size > 4096 && self.has_webgl2 && self.has_float_textures {
            return ComputeBackend::WebGL2;
        }

        // For medium operations with multiple cores, use workers
        if operation_size > 1024 && self.has_workers && self.worker_count > 1 {
            return ComputeBackend::WebWorkers;
        }

        // Fall back to single-threaded CPU
        ComputeBackend::CPU
    }
}

/// Available compute backends
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComputeBackend {
    /// WebGPU compute shaders (best performance)
    WebGPU,
    /// WebGL2 texture-based compute (fallback GPU)
    WebGL2,
    /// WebWorker pool (CPU parallelism)
    WebWorkers,
    /// Single-threaded CPU (last resort)
    CPU,
}

impl ComputeBackend {
    /// Get backend name
    pub fn name(&self) -> &'static str {
        match self {
            ComputeBackend::WebGPU => "WebGPU",
            ComputeBackend::WebGL2 => "WebGL2",
            ComputeBackend::WebWorkers => "WebWorkers",
            ComputeBackend::CPU => "CPU",
        }
    }

    /// Get relative performance (higher is better)
    pub fn relative_performance(&self) -> f32 {
        match self {
            ComputeBackend::WebGPU => 10.0,
            ComputeBackend::WebGL2 => 5.0,
            ComputeBackend::WebWorkers => 2.0,
            ComputeBackend::CPU => 1.0,
        }
    }
}

/// Detect compute capabilities on the current device
pub fn detect_capabilities() -> Result<ComputeCapability, JsValue> {
    let window = web_sys::window()
        .ok_or_else(|| JsValue::from_str("No window object"))?;

    let navigator = window.navigator();

    // Detect WebGPU
    let has_webgpu = js_sys::Reflect::has(&navigator, &"gpu".into())
        .unwrap_or(false);

    // Detect WebWorkers
    let has_workers = js_sys::Reflect::has(&window, &"Worker".into())
        .unwrap_or(false);

    // Detect SharedArrayBuffer
    let has_shared_memory = js_sys::Reflect::has(&window, &"SharedArrayBuffer".into())
        .unwrap_or(false);

    // Get hardware concurrency (CPU cores)
    let worker_count = navigator.hardware_concurrency() as usize;

    // Detect WebGL2 capabilities
    let document = window.document()
        .ok_or_else(|| JsValue::from_str("No document"))?;

    let (has_webgl2, has_float_textures, has_transform_feedback, max_texture_size, gpu_memory_mb, device_info) =
        detect_webgl2_capabilities(&document)?;

    Ok(ComputeCapability {
        has_webgpu,
        has_webgl2,
        has_float_textures,
        has_transform_feedback,
        has_workers,
        has_shared_memory,
        worker_count: worker_count.max(1),
        max_texture_size,
        gpu_memory_mb,
        device_info,
    })
}

/// Detect WebGL2-specific capabilities
fn detect_webgl2_capabilities(document: &web_sys::Document) -> Result<(bool, bool, bool, u32, u32, String), JsValue> {
    // Create a temporary canvas to probe WebGL2
    let canvas = document.create_element("canvas")?;
    let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into()?;

    // Try to get WebGL2 context
    let context = match canvas.get_context("webgl2")? {
        Some(ctx) => ctx,
        None => return Ok((false, false, false, 0, 0, "No WebGL2".to_string())),
    };

    let gl: web_sys::WebGl2RenderingContext = context.dyn_into()?;

    // Check for float texture support (required for compute)
    let ext_color_buffer_float = gl.get_extension("EXT_color_buffer_float")?;
    let has_float_textures = ext_color_buffer_float.is_some();

    // Transform feedback is built into WebGL2
    let has_transform_feedback = true;

    // Get max texture size
    let max_texture_size = gl.get_parameter(web_sys::WebGl2RenderingContext::MAX_TEXTURE_SIZE)?
        .as_f64()
        .unwrap_or(4096.0) as u32;

    // Try to get GPU memory info (vendor-specific)
    let gpu_memory_mb = get_gpu_memory_mb(&gl);

    // Get renderer info
    let renderer_info = gl.get_extension("WEBGL_debug_renderer_info")?;
    let device_info = if renderer_info.is_some() {
        // UNMASKED_RENDERER_WEBGL = 0x9246
        let renderer = gl.get_parameter(0x9246)?;
        renderer.as_string().unwrap_or_else(|| "Unknown GPU".to_string())
    } else {
        "Unknown GPU".to_string()
    };

    Ok((true, has_float_textures, has_transform_feedback, max_texture_size, gpu_memory_mb, device_info))
}

/// Try to get GPU memory size (vendor-specific extension)
fn get_gpu_memory_mb(gl: &web_sys::WebGl2RenderingContext) -> u32 {
    // Try WEBGL_memory_info extension (available on some browsers)
    if let Ok(Some(_ext)) = gl.get_extension("WEBGL_memory_info") {
        // GPU_MEMORY_INFO_TOTAL_AVAILABLE_MEMORY_NVX = 0x9048
        if let Ok(mem) = gl.get_parameter(0x9048) {
            if let Some(kb) = mem.as_f64() {
                return (kb / 1024.0) as u32;
            }
        }
    }

    // Default estimate based on typical mobile/desktop GPUs
    // Most modern GPUs have at least 2GB
    2048
}

/// Configuration for compute operations
#[derive(Clone, Debug)]
pub struct ComputeConfig {
    /// Preferred backend (None = auto-select)
    pub preferred_backend: Option<ComputeBackend>,
    /// Maximum memory to use (bytes)
    pub max_memory: usize,
    /// Timeout for operations (ms)
    pub timeout_ms: u32,
    /// Enable profiling
    pub profiling: bool,
}

impl Default for ComputeConfig {
    fn default() -> Self {
        ComputeConfig {
            preferred_backend: None,
            max_memory: 256 * 1024 * 1024, // 256MB
            timeout_ms: 30_000, // 30 seconds
            profiling: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_recommendation() {
        let caps = ComputeCapability {
            has_webgpu: false,
            has_webgl2: true,
            has_float_textures: true,
            has_transform_feedback: true,
            has_workers: true,
            has_shared_memory: true,
            worker_count: 4,
            max_texture_size: 4096,
            gpu_memory_mb: 2048,
            device_info: "Test GPU".to_string(),
        };

        // Large operations should use WebGL2
        assert_eq!(caps.recommend_backend(10000), ComputeBackend::WebGL2);

        // Medium operations with workers should use workers
        assert_eq!(caps.recommend_backend(2000), ComputeBackend::WebWorkers);

        // Small operations should use CPU
        assert_eq!(caps.recommend_backend(100), ComputeBackend::CPU);
    }

    #[test]
    fn test_backend_with_webgpu() {
        let caps = ComputeCapability {
            has_webgpu: true,
            has_webgl2: true,
            has_float_textures: true,
            has_transform_feedback: true,
            has_workers: true,
            has_shared_memory: true,
            worker_count: 4,
            max_texture_size: 4096,
            gpu_memory_mb: 2048,
            device_info: "Test GPU".to_string(),
        };

        // WebGPU should always be preferred
        assert_eq!(caps.recommend_backend(100), ComputeBackend::WebGPU);
        assert_eq!(caps.recommend_backend(10000), ComputeBackend::WebGPU);
    }
}
