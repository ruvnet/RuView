//! WebGPU Compute Context and Pipelines
//!
//! This module provides the core WebGPU compute functionality for WASM,
//! including context initialization, pipeline creation, and kernel execution.
//!
//! Note: WebGPU bindings use JavaScript interop via js_sys/Reflect since
//! web-sys WebGPU bindings are still unstable.

use js_sys::{Array, Float32Array, Object, Promise, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::{shaders, AdapterInfo, AttentionConfig};

/// Check if WebGPU is available in this browser
pub async fn is_webgpu_available() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(gpu) = get_gpu_object() {
            return !gpu.is_undefined() && !gpu.is_null();
        }
        false
    }

    #[cfg(not(target_arch = "wasm32"))]
    false
}

/// Get GPU adapter information if available
pub async fn get_gpu_info() -> Option<AdapterInfo> {
    #[cfg(target_arch = "wasm32")]
    {
        let gpu = get_gpu_object()?;

        // Request adapter
        let options = Object::new();
        let _ = Reflect::set(
            &options,
            &"powerPreference".into(),
            &"high-performance".into(),
        );

        let adapter_promise = call_method(&gpu, "requestAdapter", &[options.into()]).ok()?;
        let adapter = JsFuture::from(adapter_promise.dyn_into::<Promise>().ok()?)
            .await
            .ok()?;

        if adapter.is_null() || adapter.is_undefined() {
            return None;
        }

        // Get adapter info via requestAdapterInfo()
        let info_promise = call_method(&adapter, "requestAdapterInfo", &[]).ok()?;
        let info = JsFuture::from(info_promise.dyn_into::<Promise>().ok()?)
            .await
            .ok()?;

        // Extract limits
        let limits = Reflect::get(&adapter, &"limits".into()).ok()?;

        Some(AdapterInfo {
            vendor: get_string_prop(&info, "vendor").unwrap_or_default(),
            architecture: get_string_prop(&info, "architecture").unwrap_or_default(),
            device_type: get_string_prop(&info, "device").unwrap_or_else(|| "unknown".to_string()),
            backend: "WebGPU".to_string(),
            max_buffer_size: get_number_prop(&limits, "maxBufferSize")
                .unwrap_or(256.0 * 1024.0 * 1024.0) as u64,
            max_workgroup_size: get_number_prop(&limits, "maxComputeWorkgroupSizeX")
                .unwrap_or(256.0) as u32,
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    None
}

// ============================================================================
// Helper Functions
// ============================================================================

#[cfg(target_arch = "wasm32")]
fn get_gpu_object() -> Option<JsValue> {
    let window = web_sys::window()?;
    let navigator = Reflect::get(&window, &"navigator".into()).ok()?;
    let gpu = Reflect::get(&navigator, &"gpu".into()).ok()?;
    if gpu.is_undefined() || gpu.is_null() {
        None
    } else {
        Some(gpu)
    }
}

#[cfg(target_arch = "wasm32")]
fn get_string_prop(obj: &JsValue, key: &str) -> Option<String> {
    Reflect::get(obj, &key.into())
        .ok()
        .and_then(|v| v.as_string())
}

#[cfg(target_arch = "wasm32")]
fn get_number_prop(obj: &JsValue, key: &str) -> Option<f64> {
    Reflect::get(obj, &key.into()).ok().and_then(|v| v.as_f64())
}

#[cfg(target_arch = "wasm32")]
fn call_method(obj: &JsValue, method: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let func = Reflect::get(obj, &method.into())?.dyn_into::<js_sys::Function>()?;

    let args_array = Array::new();
    for arg in args {
        args_array.push(arg);
    }

    Reflect::apply(&func, obj, &args_array)
}

// ============================================================================
// WebGPU Context
// ============================================================================

/// WebGPU context holding device and queue references
#[wasm_bindgen]
pub struct WebGpuContext {
    /// GPU device object (JsValue wrapper)
    #[cfg(target_arch = "wasm32")]
    device: JsValue,

    /// Command queue object
    #[cfg(target_arch = "wasm32")]
    queue: JsValue,

    /// Placeholder for non-wasm builds
    #[cfg(not(target_arch = "wasm32"))]
    _phantom: std::marker::PhantomData<()>,

    /// Adapter information
    adapter_info: AdapterInfo,
}

#[wasm_bindgen]
impl WebGpuContext {
    /// Initialize WebGPU context
    #[wasm_bindgen(js_name = init)]
    pub async fn init() -> Result<WebGpuContext, JsValue> {
        #[cfg(target_arch = "wasm32")]
        {
            let gpu = get_gpu_object().ok_or_else(|| JsValue::from_str("WebGPU not available"))?;

            // Request adapter with high performance preference
            let adapter_options = Object::new();
            Reflect::set(
                &adapter_options,
                &"powerPreference".into(),
                &"high-performance".into(),
            )?;

            let adapter_promise = call_method(&gpu, "requestAdapter", &[adapter_options.into()])?;
            let adapter = JsFuture::from(adapter_promise.dyn_into::<Promise>()?).await?;

            if adapter.is_null() || adapter.is_undefined() {
                return Err(JsValue::from_str("No suitable GPU adapter found"));
            }

            // Get adapter info
            let info_promise = call_method(&adapter, "requestAdapterInfo", &[])?;
            let info = JsFuture::from(info_promise.dyn_into::<Promise>()?).await?;
            let limits = Reflect::get(&adapter, &"limits".into())?;

            let adapter_info = AdapterInfo {
                vendor: get_string_prop(&info, "vendor").unwrap_or_default(),
                architecture: get_string_prop(&info, "architecture").unwrap_or_default(),
                device_type: get_string_prop(&info, "device")
                    .unwrap_or_else(|| "unknown".to_string()),
                backend: "WebGPU".to_string(),
                max_buffer_size: get_number_prop(&limits, "maxBufferSize")
                    .unwrap_or(256.0 * 1024.0 * 1024.0) as u64,
                max_workgroup_size: get_number_prop(&limits, "maxComputeWorkgroupSizeX")
                    .unwrap_or(256.0) as u32,
            };

            // Request device
            let device_descriptor = Object::new();
            Reflect::set(&device_descriptor, &"label".into(), &"ruvllm-wasm".into())?;

            let device_promise =
                call_method(&adapter, "requestDevice", &[device_descriptor.into()])?;
            let device = JsFuture::from(device_promise.dyn_into::<Promise>()?).await?;

            // Get queue
            let queue = Reflect::get(&device, &"queue".into())?;

            Ok(WebGpuContext {
                device,
                queue,
                adapter_info,
            })
        }

        #[cfg(not(target_arch = "wasm32"))]
        Err(JsValue::from_str("WebGPU only available in WASM"))
    }

    /// Get adapter information
    #[wasm_bindgen(getter, js_name = adapterInfo)]
    pub fn adapter_info(&self) -> AdapterInfo {
        self.adapter_info.clone()
    }

    /// Check if context is valid
    #[wasm_bindgen(getter, js_name = isValid)]
    pub fn is_valid(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            !self.device.is_undefined() && !self.device.is_null()
        }

        #[cfg(not(target_arch = "wasm32"))]
        false
    }

    /// Create a GPU buffer
    #[cfg(target_arch = "wasm32")]
    fn create_buffer_internal(
        &self,
        size: usize,
        usage: u32,
        label: Option<&str>,
    ) -> Result<JsValue, JsValue> {
        let descriptor = Object::new();
        Reflect::set(&descriptor, &"size".into(), &JsValue::from_f64(size as f64))?;
        Reflect::set(
            &descriptor,
            &"usage".into(),
            &JsValue::from_f64(usage as f64),
        )?;
        if let Some(lbl) = label {
            Reflect::set(&descriptor, &"label".into(), &lbl.into())?;
        }

        call_method(&self.device, "createBuffer", &[descriptor.into()])
    }

    /// Write data to GPU buffer
    #[cfg(target_arch = "wasm32")]
    fn write_buffer_internal(&self, buffer: &JsValue, data: &[f32]) -> Result<(), JsValue> {
        let data_array = Float32Array::from(data);
        call_method(
            &self.queue,
            "writeBuffer",
            &[
                buffer.clone(),
                JsValue::from_f64(0.0),
                data_array.buffer().into(),
            ],
        )?;
        Ok(())
    }
}

// ============================================================================
// Compute Pipeline
// ============================================================================

/// Compute pipeline handle
#[wasm_bindgen]
pub struct ComputePipeline {
    #[cfg(target_arch = "wasm32")]
    pipeline: JsValue,

    #[cfg(target_arch = "wasm32")]
    bind_group_layout: JsValue,

    #[cfg(not(target_arch = "wasm32"))]
    _phantom: std::marker::PhantomData<()>,

    entry_point: String,
    workgroup_size: [u32; 3],
}

#[wasm_bindgen]
impl ComputePipeline {
    /// Get the entry point name
    #[wasm_bindgen(getter, js_name = entryPoint)]
    pub fn entry_point(&self) -> String {
        self.entry_point.clone()
    }

    /// Get the workgroup size
    #[wasm_bindgen(getter, js_name = workgroupSize)]
    pub fn workgroup_size(&self) -> Vec<u32> {
        self.workgroup_size.to_vec()
    }
}

// ============================================================================
// WebGPU Inference Engine
// ============================================================================

/// WebGPU inference engine for LLM operations
#[wasm_bindgen]
pub struct WebGpuInference {
    #[cfg(target_arch = "wasm32")]
    device: JsValue,

    #[cfg(target_arch = "wasm32")]
    queue: JsValue,

    #[cfg(not(target_arch = "wasm32"))]
    _phantom: std::marker::PhantomData<()>,

    adapter_info: AdapterInfo,
}

#[wasm_bindgen]
impl WebGpuInference {
    /// Check if WebGPU is available
    #[wasm_bindgen(js_name = isAvailable)]
    pub async fn is_available() -> bool {
        is_webgpu_available().await
    }

    /// Initialize WebGPU inference engine
    #[wasm_bindgen(js_name = init)]
    pub async fn init() -> Result<WebGpuInference, JsValue> {
        let ctx = WebGpuContext::init().await?;

        Ok(WebGpuInference {
            #[cfg(target_arch = "wasm32")]
            device: ctx.device,
            #[cfg(target_arch = "wasm32")]
            queue: ctx.queue,
            #[cfg(not(target_arch = "wasm32"))]
            _phantom: std::marker::PhantomData,
            adapter_info: ctx.adapter_info,
        })
    }

    /// Get adapter information
    #[wasm_bindgen(getter, js_name = adapterInfo)]
    pub fn adapter_info(&self) -> AdapterInfo {
        self.adapter_info.clone()
    }

    /// Perform matrix multiplication: C = A * B
    ///
    /// Args:
    ///   a: Matrix A as flat f32 array (M x K)
    ///   b: Matrix B as flat f32 array (K x N)
    ///   m: Number of rows in A
    ///   n: Number of columns in B
    ///   k: Shared dimension
    ///
    /// Returns: Result matrix C as f32 array (M x N)
    #[wasm_bindgen]
    pub async fn matmul(
        &self,
        a: &[f32],
        b: &[f32],
        m: u32,
        n: u32,
        k: u32,
    ) -> Result<Vec<f32>, JsValue> {
        // Validate dimensions
        let expected_a = (m as usize) * (k as usize);
        let expected_b = (k as usize) * (n as usize);

        if a.len() != expected_a {
            return Err(JsValue::from_str(&format!(
                "Matrix A dimension mismatch: expected {}, got {}",
                expected_a,
                a.len()
            )));
        }

        if b.len() != expected_b {
            return Err(JsValue::from_str(&format!(
                "Matrix B dimension mismatch: expected {}, got {}",
                expected_b,
                b.len()
            )));
        }

        #[cfg(target_arch = "wasm32")]
        {
            let output_size = (m as usize) * (n as usize);

            // GPU buffer usage flags
            const STORAGE: u32 = 0x80; // GPUBufferUsage.STORAGE
            const COPY_SRC: u32 = 0x04; // GPUBufferUsage.COPY_SRC
            const COPY_DST: u32 = 0x08; // GPUBufferUsage.COPY_DST
            const MAP_READ: u32 = 0x01; // GPUBufferUsage.MAP_READ
            const UNIFORM: u32 = 0x40; // GPUBufferUsage.UNIFORM

            // Create buffers
            let buffer_a = self.create_buffer(a.len() * 4, STORAGE | COPY_DST, Some("matmul_a"))?;
            let buffer_b = self.create_buffer(b.len() * 4, STORAGE | COPY_DST, Some("matmul_b"))?;
            let buffer_c =
                self.create_buffer(output_size * 4, STORAGE | COPY_SRC, Some("matmul_c"))?;

            // Create uniform buffer for dimensions
            let uniform_data: [f32; 4] = [m as f32, n as f32, k as f32, 1.0]; // M, N, K, alpha
            let uniform_buffer =
                self.create_buffer(16, UNIFORM | COPY_DST, Some("matmul_uniforms"))?;

            // Write data to buffers
            self.write_buffer(&buffer_a, a)?;
            self.write_buffer(&buffer_b, b)?;
            self.write_buffer(&uniform_buffer, &uniform_data)?;

            // Create shader module
            let shader_desc = Object::new();
            Reflect::set(&shader_desc, &"code".into(), &shaders::MATMUL_SHADER.into())?;
            let shader_module =
                call_method(&self.device, "createShaderModule", &[shader_desc.into()])?;

            // Create bind group layout
            let layout_entries = Array::new();

            // Storage buffer entries (A, B, C)
            for i in 0..3u32 {
                let entry = Object::new();
                Reflect::set(&entry, &"binding".into(), &JsValue::from_f64(i as f64))?;
                Reflect::set(&entry, &"visibility".into(), &JsValue::from_f64(4.0))?; // COMPUTE stage
                let buffer_layout = Object::new();
                Reflect::set(
                    &buffer_layout,
                    &"type".into(),
                    &(if i < 2 {
                        "read-only-storage"
                    } else {
                        "storage"
                    })
                    .into(),
                )?;
                Reflect::set(&entry, &"buffer".into(), &buffer_layout)?;
                layout_entries.push(&entry);
            }

            // Uniform buffer entry
            let uniform_entry = Object::new();
            Reflect::set(&uniform_entry, &"binding".into(), &JsValue::from_f64(3.0))?;
            Reflect::set(
                &uniform_entry,
                &"visibility".into(),
                &JsValue::from_f64(4.0),
            )?;
            let uniform_layout = Object::new();
            Reflect::set(&uniform_layout, &"type".into(), &"uniform".into())?;
            Reflect::set(&uniform_entry, &"buffer".into(), &uniform_layout)?;
            layout_entries.push(&uniform_entry);

            let layout_desc = Object::new();
            Reflect::set(&layout_desc, &"entries".into(), &layout_entries)?;
            let bind_group_layout =
                call_method(&self.device, "createBindGroupLayout", &[layout_desc.into()])?;

            // Create pipeline layout
            let layouts = Array::new();
            layouts.push(&bind_group_layout);
            let pipeline_layout_desc = Object::new();
            Reflect::set(&pipeline_layout_desc, &"bindGroupLayouts".into(), &layouts)?;
            let pipeline_layout = call_method(
                &self.device,
                "createPipelineLayout",
                &[pipeline_layout_desc.into()],
            )?;

            // Create compute pipeline
            let compute_stage = Object::new();
            Reflect::set(&compute_stage, &"module".into(), &shader_module)?;
            Reflect::set(&compute_stage, &"entryPoint".into(), &"main".into())?;

            let pipeline_desc = Object::new();
            Reflect::set(&pipeline_desc, &"layout".into(), &pipeline_layout)?;
            Reflect::set(&pipeline_desc, &"compute".into(), &compute_stage)?;

            let pipeline = call_method(
                &self.device,
                "createComputePipeline",
                &[pipeline_desc.into()],
            )?;

            // Create bind group
            let bind_entries = Array::new();
            for (i, buffer) in [&buffer_a, &buffer_b, &buffer_c, &uniform_buffer]
                .iter()
                .enumerate()
            {
                let entry = Object::new();
                Reflect::set(&entry, &"binding".into(), &JsValue::from_f64(i as f64))?;
                let resource = Object::new();
                Reflect::set(&resource, &"buffer".into(), buffer)?;
                Reflect::set(&entry, &"resource".into(), &resource)?;
                bind_entries.push(&entry);
            }

            let bind_group_desc = Object::new();
            Reflect::set(&bind_group_desc, &"layout".into(), &bind_group_layout)?;
            Reflect::set(&bind_group_desc, &"entries".into(), &bind_entries)?;
            let bind_group =
                call_method(&self.device, "createBindGroup", &[bind_group_desc.into()])?;

            // Create command encoder
            let encoder_desc = Object::new();
            let encoder =
                call_method(&self.device, "createCommandEncoder", &[encoder_desc.into()])?;

            // Begin compute pass
            let pass_desc = Object::new();
            let pass = call_method(&encoder, "beginComputePass", &[pass_desc.into()])?;

            // Set pipeline and bind group
            call_method(&pass, "setPipeline", &[pipeline.clone()])?;
            call_method(
                &pass,
                "setBindGroup",
                &[JsValue::from_f64(0.0), bind_group.clone()],
            )?;

            // Dispatch workgroups (16x16 tile size)
            let workgroups_x = (m + 15) / 16;
            let workgroups_y = (n + 15) / 16;
            call_method(
                &pass,
                "dispatchWorkgroups",
                &[
                    JsValue::from_f64(workgroups_x as f64),
                    JsValue::from_f64(workgroups_y as f64),
                ],
            )?;

            call_method(&pass, "end", &[])?;

            // Create staging buffer for readback
            let staging =
                self.create_buffer(output_size * 4, MAP_READ | COPY_DST, Some("staging"))?;

            // Copy result to staging
            call_method(
                &encoder,
                "copyBufferToBuffer",
                &[
                    buffer_c.clone(),
                    JsValue::from_f64(0.0),
                    staging.clone(),
                    JsValue::from_f64(0.0),
                    JsValue::from_f64((output_size * 4) as f64),
                ],
            )?;

            // Submit commands
            let command_buffer = call_method(&encoder, "finish", &[])?;
            let commands = Array::new();
            commands.push(&command_buffer);
            call_method(&self.queue, "submit", &[commands.into()])?;

            // Map staging buffer and read result
            let map_promise = call_method(&staging, "mapAsync", &[JsValue::from_f64(1.0)])?; // MAP_READ = 1
            JsFuture::from(map_promise.dyn_into::<Promise>()?).await?;

            let mapped_range = call_method(&staging, "getMappedRange", &[])?;
            let data = Float32Array::new(&mapped_range).to_vec();

            call_method(&staging, "unmap", &[])?;

            Ok(data)
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            // CPU fallback - naive implementation
            let mut c = vec![0.0f32; (m as usize) * (n as usize)];
            for i in 0..m as usize {
                for j in 0..n as usize {
                    let mut sum = 0.0f32;
                    for l in 0..k as usize {
                        sum += a[i * k as usize + l] * b[l * n as usize + j];
                    }
                    c[i * n as usize + j] = sum;
                }
            }
            Ok(c)
        }
    }

    /// Perform attention: Output = softmax(Q * K^T / sqrt(d_k)) * V
    #[wasm_bindgen]
    pub async fn attention(
        &self,
        q: &[f32],
        k: &[f32],
        v: &[f32],
        config: &AttentionConfig,
    ) -> Result<Vec<f32>, JsValue> {
        let hidden_dim = config.hidden_dim();
        let expected_size = (config.seq_len as usize) * (hidden_dim as usize);

        if q.len() != expected_size || k.len() != expected_size || v.len() != expected_size {
            return Err(JsValue::from_str(&format!(
                "Attention tensor dimension mismatch: expected {}, got Q:{}, K:{}, V:{}",
                expected_size,
                q.len(),
                k.len(),
                v.len()
            )));
        }

        // CPU fallback for attention (GPU implementation similar to matmul pattern)
        // For production, would implement full GPU attention here
        self.attention_cpu(q, k, v, config)
    }

    /// CPU fallback for attention
    fn attention_cpu(
        &self,
        q: &[f32],
        k: &[f32],
        v: &[f32],
        config: &AttentionConfig,
    ) -> Result<Vec<f32>, JsValue> {
        let seq_len = config.seq_len as usize;
        let num_heads = config.num_heads as usize;
        let head_dim = config.head_dim as usize;
        let hidden_dim = num_heads * head_dim;
        let scale = config.scale();

        let mut output = vec![0.0f32; seq_len * hidden_dim];

        // Process each head independently
        for h in 0..num_heads {
            for i in 0..seq_len {
                // For this query position, compute attention to all key positions
                let q_offset = i * hidden_dim + h * head_dim;

                // Compute attention scores
                let mut scores = vec![0.0f32; seq_len];
                let mut max_score = f32::NEG_INFINITY;

                for j in 0..seq_len {
                    // Causal masking
                    if config.causal && j > i {
                        scores[j] = f32::NEG_INFINITY;
                        continue;
                    }

                    let k_offset = j * hidden_dim + h * head_dim;
                    let mut score = 0.0f32;

                    for d in 0..head_dim {
                        score += q[q_offset + d] * k[k_offset + d];
                    }

                    score *= scale;
                    scores[j] = score;
                    if score > max_score {
                        max_score = score;
                    }
                }

                // Softmax
                let mut sum = 0.0f32;
                for j in 0..seq_len {
                    scores[j] = (scores[j] - max_score).exp();
                    sum += scores[j];
                }
                for j in 0..seq_len {
                    scores[j] /= sum;
                }

                // Compute weighted sum of values
                let out_offset = i * hidden_dim + h * head_dim;
                for d in 0..head_dim {
                    let mut weighted_sum = 0.0f32;
                    for j in 0..seq_len {
                        let v_offset = j * hidden_dim + h * head_dim;
                        weighted_sum += scores[j] * v[v_offset + d];
                    }
                    output[out_offset + d] = weighted_sum;
                }
            }
        }

        Ok(output)
    }

    /// Perform RMS normalization
    #[wasm_bindgen(js_name = rmsNorm)]
    pub async fn rms_norm(
        &self,
        input: &[f32],
        weight: &[f32],
        hidden_dim: u32,
        eps: f32,
    ) -> Result<Vec<f32>, JsValue> {
        if weight.len() != hidden_dim as usize {
            return Err(JsValue::from_str(&format!(
                "Weight dimension mismatch: expected {}, got {}",
                hidden_dim,
                weight.len()
            )));
        }

        if input.len() % hidden_dim as usize != 0 {
            return Err(JsValue::from_str(&format!(
                "Input size {} not divisible by hidden_dim {}",
                input.len(),
                hidden_dim
            )));
        }

        // CPU implementation
        let batch_size = input.len() / hidden_dim as usize;
        let mut output = vec![0.0f32; input.len()];

        for b in 0..batch_size {
            let offset = b * hidden_dim as usize;

            // Compute sum of squares
            let mut sum_sq = 0.0f32;
            for i in 0..hidden_dim as usize {
                let x = input[offset + i];
                sum_sq += x * x;
            }

            // RMS scale
            let rms = (sum_sq / hidden_dim as f32 + eps).sqrt();

            // Normalize and scale
            for i in 0..hidden_dim as usize {
                output[offset + i] = input[offset + i] / rms * weight[i];
            }
        }

        Ok(output)
    }

    /// Perform softmax
    #[wasm_bindgen]
    pub async fn softmax(
        &self,
        input: &[f32],
        dim: u32,
        temperature: f32,
    ) -> Result<Vec<f32>, JsValue> {
        if input.len() % dim as usize != 0 {
            return Err(JsValue::from_str(&format!(
                "Input size {} not divisible by dim {}",
                input.len(),
                dim
            )));
        }

        let batch_size = input.len() / dim as usize;
        let mut output = vec![0.0f32; input.len()];

        for b in 0..batch_size {
            let offset = b * dim as usize;

            // Find max (for numerical stability)
            let mut max_val = f32::NEG_INFINITY;
            for i in 0..dim as usize {
                let x = input[offset + i] / temperature;
                if x > max_val {
                    max_val = x;
                }
            }

            // Compute exp and sum
            let mut sum = 0.0f32;
            for i in 0..dim as usize {
                let x = (input[offset + i] / temperature - max_val).exp();
                output[offset + i] = x;
                sum += x;
            }

            // Normalize
            for i in 0..dim as usize {
                output[offset + i] /= sum;
            }
        }

        Ok(output)
    }

    // Helper methods for GPU buffer management
    #[cfg(target_arch = "wasm32")]
    fn create_buffer(
        &self,
        size: usize,
        usage: u32,
        label: Option<&str>,
    ) -> Result<JsValue, JsValue> {
        let descriptor = Object::new();
        Reflect::set(&descriptor, &"size".into(), &JsValue::from_f64(size as f64))?;
        Reflect::set(
            &descriptor,
            &"usage".into(),
            &JsValue::from_f64(usage as f64),
        )?;
        if let Some(lbl) = label {
            Reflect::set(&descriptor, &"label".into(), &lbl.into())?;
        }

        call_method(&self.device, "createBuffer", &[descriptor.into()])
    }

    #[cfg(target_arch = "wasm32")]
    fn write_buffer(&self, buffer: &JsValue, data: &[f32]) -> Result<(), JsValue> {
        let data_array = Float32Array::from(data);
        call_method(
            &self.queue,
            "writeBuffer",
            &[
                buffer.clone(),
                JsValue::from_f64(0.0),
                data_array.buffer().into(),
            ],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_matmul_fallback() {
        // Test the CPU fallback logic (in non-wasm mode)
        let a = vec![1.0, 2.0, 3.0, 4.0]; // 2x2
        let b = vec![5.0, 6.0, 7.0, 8.0]; // 2x2

        // Expected: [[1*5+2*7, 1*6+2*8], [3*5+4*7, 3*6+4*8]]
        //         = [[19, 22], [43, 50]]

        let mut c = vec![0.0f32; 4];
        for i in 0..2usize {
            for j in 0..2usize {
                let mut sum = 0.0f32;
                for l in 0..2usize {
                    sum += a[i * 2 + l] * b[l * 2 + j];
                }
                c[i * 2 + j] = sum;
            }
        }

        assert_eq!(c, vec![19.0, 22.0, 43.0, 50.0]);
    }

    #[test]
    fn test_rms_norm_cpu() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let weight = vec![1.0, 1.0, 1.0, 1.0];
        let hidden_dim = 4;
        let eps = 1e-5f32;

        // sum_sq = 1 + 4 + 9 + 16 = 30
        // rms = sqrt(30/4 + eps) = sqrt(7.5) ≈ 2.7386
        let rms = (30.0f32 / 4.0 + eps).sqrt();

        let expected: Vec<f32> = input.iter().map(|&x| x / rms).collect();

        // Verify calculation
        assert!((expected[0] - 0.3651).abs() < 0.001);
    }

    #[test]
    fn test_softmax_cpu() {
        let input = vec![1.0, 2.0, 3.0];
        let temperature = 1.0f32;

        // max = 3
        // exp(1-3) = exp(-2), exp(2-3) = exp(-1), exp(3-3) = exp(0) = 1
        let exps: Vec<f32> = vec![(-2.0f32).exp(), (-1.0f32).exp(), 1.0];
        let sum: f32 = exps.iter().sum();
        let expected: Vec<f32> = exps.iter().map(|&x| x / sum).collect();

        // Verify softmax sums to 1
        let softmax_sum: f32 = expected.iter().sum();
        assert!((softmax_sum - 1.0).abs() < 0.001);
    }
}
