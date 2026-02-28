//! RoPE (Rotary Position Embedding) Kernel
//!
//! This kernel implements rotary position embeddings as described in the
//! RoFormer paper (https://arxiv.org/abs/2104.09864).
//!
//! RoPE applies rotation to the query and key vectors in attention,
//! encoding relative positional information.
//!
//! # Compilation
//!
//! To compile this kernel to WASM:
//! ```bash
//! rustc --target wasm32-unknown-unknown \
//!       --crate-type cdylib \
//!       -C opt-level=3 \
//!       -C lto=fat \
//!       kernels/rope.rs \
//!       -o kernels/rope_f32.wasm
//! ```
//!
//! Or use the provided build script in the kernels directory.

#![no_std]
#![no_main]

// Panic handler for no_std
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// Kernel descriptor structure (must match host definition)
#[repr(C)]
pub struct KernelDescriptor {
    pub input_a_offset: u32,    // x tensor
    pub input_a_size: u32,
    pub input_b_offset: u32,    // freqs tensor
    pub input_b_size: u32,
    pub output_offset: u32,
    pub output_size: u32,
    pub scratch_offset: u32,
    pub scratch_size: u32,
    pub params_offset: u32,
    pub params_size: u32,
}

/// RoPE parameters
#[repr(C)]
pub struct RopeParams {
    /// Base frequency (typically 10000.0)
    pub theta: f32,
    /// Sequence length
    pub seq_len: u32,
    /// Head dimension (must be even)
    pub head_dim: u32,
    /// Number of heads
    pub num_heads: u32,
    /// Batch size
    pub batch_size: u32,
}

/// Error codes
const OK: i32 = 0;
const INVALID_INPUT: i32 = 1;
const INVALID_OUTPUT: i32 = 2;
const INVALID_PARAMS: i32 = 3;

/// Initialize kernel (optional, for stateful kernels)
#[no_mangle]
pub extern "C" fn kernel_init(_params_ptr: *const u8, _params_len: u32) -> i32 {
    OK
}

/// Execute RoPE forward pass
///
/// # Memory Layout
///
/// Input A (x): [batch, seq, heads, dim] as f32
/// Input B (freqs): [seq, dim/2] as f32 (precomputed frequencies)
/// Output (y): [batch, seq, heads, dim] as f32
///
/// The kernel applies rotation to pairs of elements:
/// y[..., 2i] = x[..., 2i] * cos(freq) - x[..., 2i+1] * sin(freq)
/// y[..., 2i+1] = x[..., 2i] * sin(freq) + x[..., 2i+1] * cos(freq)
#[no_mangle]
pub extern "C" fn kernel_forward(desc_ptr: *const KernelDescriptor) -> i32 {
    // Safety: We trust the host to provide valid pointers
    let desc = unsafe { &*desc_ptr };

    // Validate inputs
    if desc.input_a_size == 0 {
        return INVALID_INPUT;
    }
    if desc.output_size == 0 || desc.output_size != desc.input_a_size {
        return INVALID_OUTPUT;
    }
    if desc.params_size < core::mem::size_of::<RopeParams>() as u32 {
        return INVALID_PARAMS;
    }

    // Get memory base pointer (WASM linear memory starts at 0)
    let memory_base = 0usize as *mut u8;

    // Get params
    let params = unsafe {
        &*(memory_base.add(desc.params_offset as usize) as *const RopeParams)
    };

    // Validate head_dim is even
    if params.head_dim % 2 != 0 {
        return INVALID_PARAMS;
    }

    let half_dim = params.head_dim / 2;

    // Get tensor pointers
    let x_ptr = unsafe { memory_base.add(desc.input_a_offset as usize) as *const f32 };
    let freqs_ptr = unsafe { memory_base.add(desc.input_b_offset as usize) as *const f32 };
    let y_ptr = unsafe { memory_base.add(desc.output_offset as usize) as *mut f32 };

    // Apply RoPE
    // Loop order: batch -> seq -> head -> dim_pair
    for b in 0..params.batch_size {
        for s in 0..params.seq_len {
            for h in 0..params.num_heads {
                for d in 0..half_dim {
                    // Calculate indices
                    let idx = ((b * params.seq_len + s) * params.num_heads + h) * params.head_dim + d * 2;
                    let freq_idx = s * half_dim + d;

                    unsafe {
                        // Get input values
                        let x0 = *x_ptr.add(idx as usize);
                        let x1 = *x_ptr.add(idx as usize + 1);

                        // Get frequency (precomputed cos and sin are interleaved)
                        let freq = *freqs_ptr.add(freq_idx as usize);
                        let cos_f = libm::cosf(freq);
                        let sin_f = libm::sinf(freq);

                        // Apply rotation
                        let y0 = x0 * cos_f - x1 * sin_f;
                        let y1 = x0 * sin_f + x1 * cos_f;

                        // Write output
                        *y_ptr.add(idx as usize) = y0;
                        *y_ptr.add(idx as usize + 1) = y1;
                    }
                }
            }
        }
    }

    OK
}

/// Execute RoPE backward pass (gradient computation)
///
/// The backward pass is the same rotation with negated sin,
/// since the Jacobian of rotation is another rotation.
#[no_mangle]
pub extern "C" fn kernel_backward(desc_ptr: *const KernelDescriptor) -> i32 {
    // For RoPE, backward is essentially the same operation with transposed rotation
    // (negated sin terms), but the structure is identical
    let desc = unsafe { &*desc_ptr };

    if desc.input_a_size == 0 {
        return INVALID_INPUT;
    }
    if desc.output_size == 0 || desc.output_size != desc.input_a_size {
        return INVALID_OUTPUT;
    }
    if desc.params_size < core::mem::size_of::<RopeParams>() as u32 {
        return INVALID_PARAMS;
    }

    let memory_base = 0usize as *mut u8;

    let params = unsafe {
        &*(memory_base.add(desc.params_offset as usize) as *const RopeParams)
    };

    if params.head_dim % 2 != 0 {
        return INVALID_PARAMS;
    }

    let half_dim = params.head_dim / 2;

    let grad_y_ptr = unsafe { memory_base.add(desc.input_a_offset as usize) as *const f32 };
    let freqs_ptr = unsafe { memory_base.add(desc.input_b_offset as usize) as *const f32 };
    let grad_x_ptr = unsafe { memory_base.add(desc.output_offset as usize) as *mut f32 };

    // Backward RoPE: apply inverse rotation (transpose = negate sin)
    for b in 0..params.batch_size {
        for s in 0..params.seq_len {
            for h in 0..params.num_heads {
                for d in 0..half_dim {
                    let idx = ((b * params.seq_len + s) * params.num_heads + h) * params.head_dim + d * 2;
                    let freq_idx = s * half_dim + d;

                    unsafe {
                        let gy0 = *grad_y_ptr.add(idx as usize);
                        let gy1 = *grad_y_ptr.add(idx as usize + 1);

                        let freq = *freqs_ptr.add(freq_idx as usize);
                        let cos_f = libm::cosf(freq);
                        let sin_f = libm::sinf(freq);

                        // Inverse rotation (transpose)
                        let gx0 = gy0 * cos_f + gy1 * sin_f;
                        let gx1 = -gy0 * sin_f + gy1 * cos_f;

                        *grad_x_ptr.add(idx as usize) = gx0;
                        *grad_x_ptr.add(idx as usize + 1) = gx1;
                    }
                }
            }
        }
    }

    OK
}

/// Kernel info structure
#[repr(C)]
pub struct KernelInfo {
    pub name_ptr: *const u8,
    pub name_len: u32,
    pub version_major: u16,
    pub version_minor: u16,
    pub version_patch: u16,
    pub supports_backward: bool,
}

static KERNEL_NAME: &[u8] = b"rope_f32\0";

/// Get kernel metadata
#[no_mangle]
pub extern "C" fn kernel_info(info_ptr: *mut KernelInfo) -> i32 {
    if info_ptr.is_null() {
        return INVALID_PARAMS;
    }

    unsafe {
        (*info_ptr).name_ptr = KERNEL_NAME.as_ptr();
        (*info_ptr).name_len = KERNEL_NAME.len() as u32 - 1; // Exclude null terminator
        (*info_ptr).version_major = 1;
        (*info_ptr).version_minor = 0;
        (*info_ptr).version_patch = 0;
        (*info_ptr).supports_backward = true;
    }

    OK
}

/// Cleanup kernel resources
#[no_mangle]
pub extern "C" fn kernel_cleanup() -> i32 {
    // No resources to cleanup for this stateless kernel
    OK
}

// Minimal libm implementations for no_std
mod libm {
    // Simple Taylor series approximations for sin and cos
    // In production, use more accurate implementations or link to libm

    const PI: f32 = 3.14159265358979323846;
    const TWO_PI: f32 = 2.0 * PI;

    fn normalize_angle(mut x: f32) -> f32 {
        // Reduce to [-PI, PI]
        while x > PI {
            x -= TWO_PI;
        }
        while x < -PI {
            x += TWO_PI;
        }
        x
    }

    pub fn sinf(x: f32) -> f32 {
        let x = normalize_angle(x);
        // Taylor series: sin(x) = x - x^3/3! + x^5/5! - x^7/7! + ...
        let x2 = x * x;
        let x3 = x2 * x;
        let x5 = x3 * x2;
        let x7 = x5 * x2;
        let x9 = x7 * x2;

        x - x3 / 6.0 + x5 / 120.0 - x7 / 5040.0 + x9 / 362880.0
    }

    pub fn cosf(x: f32) -> f32 {
        let x = normalize_angle(x);
        // Taylor series: cos(x) = 1 - x^2/2! + x^4/4! - x^6/6! + ...
        let x2 = x * x;
        let x4 = x2 * x2;
        let x6 = x4 * x2;
        let x8 = x6 * x2;

        1.0 - x2 / 2.0 + x4 / 24.0 - x6 / 720.0 + x8 / 40320.0
    }
}
