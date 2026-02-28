//! WebGL2 compute simulation for GPU-accelerated operations
//!
//! Uses ping-pong texture rendering for matrix operations on devices without WebGPU.
//! This approach treats textures as 2D arrays and uses fragment shaders for computation.
//!
//! ## Architecture
//!
//! ```text
//! +-------------+     +----------------+     +-------------+
//! |  Input A    | --> |  Fragment      | --> |  Output     |
//! |  (Texture)  |     |  Shader        |     |  (Texture)  |
//! +-------------+     +----------------+     +-------------+
//!        ^                   |                      |
//!        |                   v                      v
//! +-------------+     +----------------+     +-------------+
//! |  Input B    | --> |  Transform     | --> |  CPU Read   |
//! |  (Texture)  |     |  Feedback      |     |  (Float32)  |
//! +-------------+     +----------------+     +-------------+
//! ```
//!
//! ## Limitations vs WebGPU
//!
//! - No true compute shaders (uses fragment shaders)
//! - Limited to 2D texture operations
//! - Readback through transform feedback or readPixels
//! - Lower performance than WebGPU compute

use wasm_bindgen::prelude::*;
use web_sys::{
    WebGl2RenderingContext, WebGlProgram, WebGlShader, WebGlTexture,
    WebGlFramebuffer, WebGlBuffer, WebGlVertexArrayObject,
};
use crate::compute::tensor::{Tensor, TensorShape};

/// Shader programs for different operations
struct ShaderPrograms {
    matmul: WebGlProgram,
    vector_add: WebGlProgram,
    vector_mul: WebGlProgram,
    softmax: WebGlProgram,
    relu: WebGlProgram,
}

/// WebGL2 compute backend
#[wasm_bindgen]
pub struct WebGl2Compute {
    /// WebGL2 rendering context
    gl: WebGl2RenderingContext,
    /// Shader programs
    programs: ShaderPrograms,
    /// Texture pool for reuse
    texture_pool: Vec<TextureHandle>,
    /// Framebuffer for render-to-texture
    framebuffer: WebGlFramebuffer,
    /// Full-screen quad VAO
    quad_vao: WebGlVertexArrayObject,
    /// Quad vertex buffer
    quad_vbo: WebGlBuffer,
    /// Maximum texture size
    max_texture_size: u32,
    /// Transform feedback buffer for readback
    tf_buffer: WebGlBuffer,
}

/// Handle to a pooled texture
struct TextureHandle {
    texture: WebGlTexture,
    width: u32,
    height: u32,
    in_use: bool,
}

#[wasm_bindgen]
impl WebGl2Compute {
    /// Create a new WebGL2 compute backend
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WebGl2Compute, JsValue> {
        let window = web_sys::window()
            .ok_or_else(|| JsValue::from_str("No window"))?;
        let document = window.document()
            .ok_or_else(|| JsValue::from_str("No document"))?;

        // Create offscreen canvas
        let canvas = document.create_element("canvas")?;
        let canvas: web_sys::HtmlCanvasElement = canvas.dyn_into()?;
        canvas.set_width(1);
        canvas.set_height(1);

        // Get WebGL2 context
        let context_options = js_sys::Object::new();
        js_sys::Reflect::set(&context_options, &"antialias".into(), &false.into())?;
        js_sys::Reflect::set(&context_options, &"depth".into(), &false.into())?;
        js_sys::Reflect::set(&context_options, &"stencil".into(), &false.into())?;
        js_sys::Reflect::set(&context_options, &"preserveDrawingBuffer".into(), &true.into())?;

        let gl: WebGl2RenderingContext = canvas
            .get_context_with_context_options("webgl2", &context_options)?
            .ok_or_else(|| JsValue::from_str("WebGL2 not available"))?
            .dyn_into()?;

        // Enable required extensions
        gl.get_extension("EXT_color_buffer_float")?
            .ok_or_else(|| JsValue::from_str("EXT_color_buffer_float not available"))?;
        gl.get_extension("OES_texture_float_linear")?;

        // Get max texture size
        let max_texture_size = gl.get_parameter(WebGl2RenderingContext::MAX_TEXTURE_SIZE)?
            .as_f64()
            .unwrap_or(4096.0) as u32;

        // Create shader programs
        let programs = ShaderPrograms {
            matmul: create_matmul_program(&gl)?,
            vector_add: create_vector_add_program(&gl)?,
            vector_mul: create_vector_mul_program(&gl)?,
            softmax: create_softmax_program(&gl)?,
            relu: create_relu_program(&gl)?,
        };

        // Create framebuffer
        let framebuffer = gl.create_framebuffer()
            .ok_or_else(|| JsValue::from_str("Failed to create framebuffer"))?;

        // Create full-screen quad
        let (quad_vao, quad_vbo) = create_fullscreen_quad(&gl)?;

        // Create transform feedback buffer
        let tf_buffer = gl.create_buffer()
            .ok_or_else(|| JsValue::from_str("Failed to create TF buffer"))?;

        Ok(WebGl2Compute {
            gl,
            programs,
            texture_pool: Vec::new(),
            framebuffer,
            quad_vao,
            quad_vbo,
            max_texture_size,
            tf_buffer,
        })
    }

    /// Check if WebGL2 compute is available
    #[wasm_bindgen(js_name = isAvailable)]
    pub fn is_available() -> bool {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Ok(canvas) = document.create_element("canvas") {
                    if let Ok(canvas) = canvas.dyn_into::<web_sys::HtmlCanvasElement>() {
                        if let Ok(Some(ctx)) = canvas.get_context("webgl2") {
                            if let Ok(gl) = ctx.dyn_into::<WebGl2RenderingContext>() {
                                return gl.get_extension("EXT_color_buffer_float")
                                    .map(|e| e.is_some())
                                    .unwrap_or(false);
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Get maximum supported texture size
    #[wasm_bindgen(js_name = maxTextureSize)]
    pub fn max_texture_size(&self) -> u32 {
        self.max_texture_size
    }
}

// Non-WASM implementation
impl WebGl2Compute {
    /// Perform matrix multiplication: C = A * B
    pub fn matmul(&self, a: &Tensor, b: &Tensor) -> Result<Tensor, JsValue> {
        if !a.shape().is_matrix() || !b.shape().is_matrix() {
            return Err(JsValue::from_str("Inputs must be matrices"));
        }

        let m = a.shape().rows();
        let k = a.shape().cols();
        let n = b.shape().cols();

        if k != b.shape().rows() {
            return Err(JsValue::from_str("Matrix dimension mismatch"));
        }

        // For small matrices, use CPU
        if m * k * n < 4096 {
            return Ok(self.cpu_matmul(a, b));
        }

        // Upload matrices to textures
        let tex_a = self.upload_matrix(a)?;
        let tex_b = self.upload_matrix(b)?;
        let tex_c = self.create_texture(m as u32, n as u32)?;

        // Bind output texture to framebuffer
        self.gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(&self.framebuffer));
        self.gl.framebuffer_texture_2d(
            WebGl2RenderingContext::FRAMEBUFFER,
            WebGl2RenderingContext::COLOR_ATTACHMENT0,
            WebGl2RenderingContext::TEXTURE_2D,
            Some(&tex_c),
            0,
        );

        // Set viewport
        self.gl.viewport(0, 0, n as i32, m as i32);

        // Use matmul program
        self.gl.use_program(Some(&self.programs.matmul));

        // Bind input textures
        self.gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        self.gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&tex_a));
        let loc_a = self.gl.get_uniform_location(&self.programs.matmul, "u_A");
        self.gl.uniform1i(loc_a.as_ref(), 0);

        self.gl.active_texture(WebGl2RenderingContext::TEXTURE1);
        self.gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&tex_b));
        let loc_b = self.gl.get_uniform_location(&self.programs.matmul, "u_B");
        self.gl.uniform1i(loc_b.as_ref(), 1);

        // Set dimensions
        let loc_dims = self.gl.get_uniform_location(&self.programs.matmul, "u_dims");
        self.gl.uniform3f(loc_dims.as_ref(), m as f32, k as f32, n as f32);

        // Draw full-screen quad
        self.gl.bind_vertex_array(Some(&self.quad_vao));
        self.gl.draw_arrays(WebGl2RenderingContext::TRIANGLE_STRIP, 0, 4);

        // Read back result
        let result = self.read_texture(&tex_c, m as u32, n as u32)?;

        // Cleanup
        self.gl.delete_texture(Some(&tex_a));
        self.gl.delete_texture(Some(&tex_b));
        self.gl.delete_texture(Some(&tex_c));
        self.gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, None);

        Ok(Tensor::from_vec(result, TensorShape::matrix(m, n)))
    }

    /// Element-wise vector operations
    pub fn vector_op(&self, a: &[f32], b: &[f32], op: &str) -> Result<Vec<f32>, JsValue> {
        if a.len() != b.len() {
            return Err(JsValue::from_str("Vector length mismatch"));
        }

        let len = a.len();

        // For small vectors, use CPU
        if len < 1024 {
            return Ok(match op {
                "add" => a.iter().zip(b.iter()).map(|(x, y)| x + y).collect(),
                "sub" => a.iter().zip(b.iter()).map(|(x, y)| x - y).collect(),
                "mul" => a.iter().zip(b.iter()).map(|(x, y)| x * y).collect(),
                "div" => a.iter().zip(b.iter()).map(|(x, y)| x / y).collect(),
                _ => return Err(JsValue::from_str(&format!("Unknown op: {}", op))),
            });
        }

        // Calculate texture dimensions (square-ish)
        let width = (len as f32).sqrt().ceil() as u32;
        let height = ((len as u32 + width - 1) / width).max(1);

        // Pad data to fill texture
        let padded_len = (width * height) as usize;
        let mut a_padded = a.to_vec();
        let mut b_padded = b.to_vec();
        a_padded.resize(padded_len, 0.0);
        b_padded.resize(padded_len, 0.0);

        // Upload to textures
        let tex_a = self.upload_data(&a_padded, width, height)?;
        let tex_b = self.upload_data(&b_padded, width, height)?;
        let tex_c = self.create_texture(width, height)?;

        // Select program
        let program = match op {
            "add" | "sub" => &self.programs.vector_add,
            "mul" | "div" => &self.programs.vector_mul,
            _ => return Err(JsValue::from_str(&format!("Unknown op: {}", op))),
        };

        // Bind framebuffer
        self.gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(&self.framebuffer));
        self.gl.framebuffer_texture_2d(
            WebGl2RenderingContext::FRAMEBUFFER,
            WebGl2RenderingContext::COLOR_ATTACHMENT0,
            WebGl2RenderingContext::TEXTURE_2D,
            Some(&tex_c),
            0,
        );

        self.gl.viewport(0, 0, width as i32, height as i32);
        self.gl.use_program(Some(program));

        // Bind textures
        self.gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        self.gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&tex_a));
        self.gl.uniform1i(self.gl.get_uniform_location(program, "u_A").as_ref(), 0);

        self.gl.active_texture(WebGl2RenderingContext::TEXTURE1);
        self.gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&tex_b));
        self.gl.uniform1i(self.gl.get_uniform_location(program, "u_B").as_ref(), 1);

        // Set operation mode
        let op_mode = match op {
            "add" => 0.0,
            "sub" => 1.0,
            "mul" => 0.0,
            "div" => 1.0,
            _ => 0.0,
        };
        self.gl.uniform1f(self.gl.get_uniform_location(program, "u_mode").as_ref(), op_mode);

        // Draw
        self.gl.bind_vertex_array(Some(&self.quad_vao));
        self.gl.draw_arrays(WebGl2RenderingContext::TRIANGLE_STRIP, 0, 4);

        // Read back
        let result = self.read_texture(&tex_c, width, height)?;

        // Cleanup
        self.gl.delete_texture(Some(&tex_a));
        self.gl.delete_texture(Some(&tex_b));
        self.gl.delete_texture(Some(&tex_c));
        self.gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, None);

        // Trim to original length
        Ok(result[..len].to_vec())
    }

    /// Upload matrix to texture
    fn upload_matrix(&self, tensor: &Tensor) -> Result<WebGlTexture, JsValue> {
        let rows = tensor.shape().rows() as u32;
        let cols = tensor.shape().cols() as u32;
        self.upload_data(tensor.data(), cols, rows)
    }

    /// Upload data to a float texture
    fn upload_data(&self, data: &[f32], width: u32, height: u32) -> Result<WebGlTexture, JsValue> {
        let texture = self.gl.create_texture()
            .ok_or_else(|| JsValue::from_str("Failed to create texture"))?;

        self.gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));

        // Set texture parameters
        self.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            WebGl2RenderingContext::NEAREST as i32,
        );
        self.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            WebGl2RenderingContext::NEAREST as i32,
        );
        self.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_S,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        self.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_T,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );

        // Create Float32Array view
        let array = js_sys::Float32Array::from(data);

        // Upload as R32F texture
        self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
            WebGl2RenderingContext::TEXTURE_2D,
            0,
            WebGl2RenderingContext::R32F as i32,
            width as i32,
            height as i32,
            0,
            WebGl2RenderingContext::RED,
            WebGl2RenderingContext::FLOAT,
            Some(&array),
        )?;

        Ok(texture)
    }

    /// Create an empty float texture
    fn create_texture(&self, width: u32, height: u32) -> Result<WebGlTexture, JsValue> {
        let texture = self.gl.create_texture()
            .ok_or_else(|| JsValue::from_str("Failed to create texture"))?;

        self.gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));

        self.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            WebGl2RenderingContext::NEAREST as i32,
        );
        self.gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            WebGl2RenderingContext::NEAREST as i32,
        );

        self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_array_buffer_view(
            WebGl2RenderingContext::TEXTURE_2D,
            0,
            WebGl2RenderingContext::R32F as i32,
            width as i32,
            height as i32,
            0,
            WebGl2RenderingContext::RED,
            WebGl2RenderingContext::FLOAT,
            None,
        )?;

        Ok(texture)
    }

    /// Read texture data back to CPU
    fn read_texture(&self, texture: &WebGlTexture, width: u32, height: u32) -> Result<Vec<f32>, JsValue> {
        // Bind texture to framebuffer
        self.gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, Some(&self.framebuffer));
        self.gl.framebuffer_texture_2d(
            WebGl2RenderingContext::FRAMEBUFFER,
            WebGl2RenderingContext::COLOR_ATTACHMENT0,
            WebGl2RenderingContext::TEXTURE_2D,
            Some(texture),
            0,
        );

        // Read pixels as RGBA (WebGL2 limitation for readPixels)
        let pixel_count = (width * height) as usize;
        let mut rgba_data = vec![0u8; pixel_count * 4 * 4]; // RGBA * f32

        // Use readPixels with RGBA format
        let float_array = js_sys::Float32Array::new_with_length(pixel_count as u32 * 4);

        self.gl.read_pixels_with_array_buffer_view(
            0, 0,
            width as i32, height as i32,
            WebGl2RenderingContext::RGBA,
            WebGl2RenderingContext::FLOAT,
            &float_array,
        )?;

        // Extract R channel (our actual data)
        let mut result = Vec::with_capacity(pixel_count);
        for i in 0..pixel_count {
            result.push(float_array.get_index((i * 4) as u32));
        }

        Ok(result)
    }

    /// CPU fallback for small matrices
    fn cpu_matmul(&self, a: &Tensor, b: &Tensor) -> Tensor {
        let m = a.shape().rows();
        let k = a.shape().cols();
        let n = b.shape().cols();

        let a_data = a.data();
        let b_data = b.data();
        let mut result = vec![0.0f32; m * n];

        for i in 0..m {
            for j in 0..n {
                let mut sum = 0.0;
                for kk in 0..k {
                    sum += a_data[i * k + kk] * b_data[kk * n + j];
                }
                result[i * n + j] = sum;
            }
        }

        Tensor::from_vec(result, TensorShape::matrix(m, n))
    }
}

/// Create fullscreen quad for render-to-texture
fn create_fullscreen_quad(gl: &WebGl2RenderingContext) -> Result<(WebGlVertexArrayObject, WebGlBuffer), JsValue> {
    let vao = gl.create_vertex_array()
        .ok_or_else(|| JsValue::from_str("Failed to create VAO"))?;
    let vbo = gl.create_buffer()
        .ok_or_else(|| JsValue::from_str("Failed to create VBO"))?;

    gl.bind_vertex_array(Some(&vao));
    gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));

    // Fullscreen quad vertices (position + texcoord)
    let vertices: [f32; 16] = [
        -1.0, -1.0, 0.0, 0.0,
         1.0, -1.0, 1.0, 0.0,
        -1.0,  1.0, 0.0, 1.0,
         1.0,  1.0, 1.0, 1.0,
    ];

    let array = js_sys::Float32Array::from(vertices.as_slice());
    gl.buffer_data_with_array_buffer_view(
        WebGl2RenderingContext::ARRAY_BUFFER,
        &array,
        WebGl2RenderingContext::STATIC_DRAW,
    );

    // Position attribute
    gl.enable_vertex_attrib_array(0);
    gl.vertex_attrib_pointer_with_i32(0, 2, WebGl2RenderingContext::FLOAT, false, 16, 0);

    // Texcoord attribute
    gl.enable_vertex_attrib_array(1);
    gl.vertex_attrib_pointer_with_i32(1, 2, WebGl2RenderingContext::FLOAT, false, 16, 8);

    Ok((vao, vbo))
}

/// Compile a shader
fn compile_shader(gl: &WebGl2RenderingContext, shader_type: u32, source: &str) -> Result<WebGlShader, JsValue> {
    let shader = gl.create_shader(shader_type)
        .ok_or_else(|| JsValue::from_str("Failed to create shader"))?;

    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if !gl.get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        let log = gl.get_shader_info_log(&shader)
            .unwrap_or_else(|| "Unknown error".to_string());
        gl.delete_shader(Some(&shader));
        return Err(JsValue::from_str(&format!("Shader compile error: {}", log)));
    }

    Ok(shader)
}

/// Link a shader program
fn link_program(gl: &WebGl2RenderingContext, vertex: &WebGlShader, fragment: &WebGlShader) -> Result<WebGlProgram, JsValue> {
    let program = gl.create_program()
        .ok_or_else(|| JsValue::from_str("Failed to create program"))?;

    gl.attach_shader(&program, vertex);
    gl.attach_shader(&program, fragment);
    gl.link_program(&program);

    if !gl.get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        let log = gl.get_program_info_log(&program)
            .unwrap_or_else(|| "Unknown error".to_string());
        gl.delete_program(Some(&program));
        return Err(JsValue::from_str(&format!("Program link error: {}", log)));
    }

    Ok(program)
}

/// Vertex shader for all compute operations
const VERTEX_SHADER: &str = r#"#version 300 es
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_texcoord;
out vec2 v_texcoord;
void main() {
    gl_Position = vec4(a_position, 0.0, 1.0);
    v_texcoord = a_texcoord;
}
"#;

/// Create matrix multiplication program
fn create_matmul_program(gl: &WebGl2RenderingContext) -> Result<WebGlProgram, JsValue> {
    const MATMUL_FRAG: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_A;
uniform sampler2D u_B;
uniform vec3 u_dims; // M, K, N
in vec2 v_texcoord;
out float fragColor;

void main() {
    float M = u_dims.x;
    float K = u_dims.y;
    float N = u_dims.z;

    // Output position
    float i = floor(v_texcoord.y * M);
    float j = floor(v_texcoord.x * N);

    float sum = 0.0;
    for (float k = 0.0; k < K; k += 1.0) {
        float a = texture(u_A, vec2((k + 0.5) / K, (i + 0.5) / M)).r;
        float b = texture(u_B, vec2((j + 0.5) / N, (k + 0.5) / K)).r;
        sum += a * b;
    }

    fragColor = sum;
}
"#;

    let vs = compile_shader(gl, WebGl2RenderingContext::VERTEX_SHADER, VERTEX_SHADER)?;
    let fs = compile_shader(gl, WebGl2RenderingContext::FRAGMENT_SHADER, MATMUL_FRAG)?;
    link_program(gl, &vs, &fs)
}

/// Create vector addition program
fn create_vector_add_program(gl: &WebGl2RenderingContext) -> Result<WebGlProgram, JsValue> {
    const VECTOR_ADD_FRAG: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_A;
uniform sampler2D u_B;
uniform float u_mode; // 0 = add, 1 = sub
in vec2 v_texcoord;
out float fragColor;

void main() {
    float a = texture(u_A, v_texcoord).r;
    float b = texture(u_B, v_texcoord).r;
    fragColor = u_mode < 0.5 ? a + b : a - b;
}
"#;

    let vs = compile_shader(gl, WebGl2RenderingContext::VERTEX_SHADER, VERTEX_SHADER)?;
    let fs = compile_shader(gl, WebGl2RenderingContext::FRAGMENT_SHADER, VECTOR_ADD_FRAG)?;
    link_program(gl, &vs, &fs)
}

/// Create vector multiplication program
fn create_vector_mul_program(gl: &WebGl2RenderingContext) -> Result<WebGlProgram, JsValue> {
    const VECTOR_MUL_FRAG: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_A;
uniform sampler2D u_B;
uniform float u_mode; // 0 = mul, 1 = div
in vec2 v_texcoord;
out float fragColor;

void main() {
    float a = texture(u_A, v_texcoord).r;
    float b = texture(u_B, v_texcoord).r;
    fragColor = u_mode < 0.5 ? a * b : a / max(b, 1e-7);
}
"#;

    let vs = compile_shader(gl, WebGl2RenderingContext::VERTEX_SHADER, VERTEX_SHADER)?;
    let fs = compile_shader(gl, WebGl2RenderingContext::FRAGMENT_SHADER, VECTOR_MUL_FRAG)?;
    link_program(gl, &vs, &fs)
}

/// Create softmax program
fn create_softmax_program(gl: &WebGl2RenderingContext) -> Result<WebGlProgram, JsValue> {
    const SOFTMAX_FRAG: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_A;
uniform vec2 u_size;
in vec2 v_texcoord;
out float fragColor;

void main() {
    // First pass would compute max, second pass computes exp/sum
    // This is a simplified single-pass version for small vectors
    float x = texture(u_A, v_texcoord).r;
    fragColor = exp(x);
}
"#;

    let vs = compile_shader(gl, WebGl2RenderingContext::VERTEX_SHADER, VERTEX_SHADER)?;
    let fs = compile_shader(gl, WebGl2RenderingContext::FRAGMENT_SHADER, SOFTMAX_FRAG)?;
    link_program(gl, &vs, &fs)
}

/// Create ReLU program
fn create_relu_program(gl: &WebGl2RenderingContext) -> Result<WebGlProgram, JsValue> {
    const RELU_FRAG: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_A;
in vec2 v_texcoord;
out float fragColor;

void main() {
    float x = texture(u_A, v_texcoord).r;
    fragColor = max(x, 0.0);
}
"#;

    let vs = compile_shader(gl, WebGl2RenderingContext::VERTEX_SHADER, VERTEX_SHADER)?;
    let fs = compile_shader(gl, WebGl2RenderingContext::FRAGMENT_SHADER, RELU_FRAG)?;
    link_program(gl, &vs, &fs)
}

#[cfg(test)]
mod tests {
    // WebGL tests require browser environment
}
