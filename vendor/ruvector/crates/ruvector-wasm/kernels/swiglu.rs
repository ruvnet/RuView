//! SwiGLU (Swish-Gated Linear Unit) Activation Kernel
//!
//! This kernel implements the SwiGLU activation function used in models
//! like LLaMA and PaLM. It combines the Swish activation with a gating
//! mechanism.
//!
//! Formula: SwiGLU(x, gate) = swish(gate) * x
//!          where swish(x) = x * sigmoid(x)
//!
//! In practice, this is often used in the FFN:
//!   FFN(x) = (swish(x * W_gate) * (x * W_up)) * W_down
//!
//! This kernel computes: swish(gate) * x
//!
//! # Compilation
//!
//! To compile this kernel to WASM:
//! ```bash
//! rustc --target wasm32-unknown-unknown \
//!       --crate-type cdylib \
//!       -C opt-level=3 \
//!       -C lto=fat \
//!       kernels/swiglu.rs \
//!       -o kernels/swiglu_f32.wasm
//! ```

#![no_std]
#![no_main]

// Panic handler for no_std
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// Kernel descriptor structure
#[repr(C)]
pub struct KernelDescriptor {
    pub input_a_offset: u32,    // x tensor (to be gated)
    pub input_a_size: u32,
    pub input_b_offset: u32,    // gate tensor
    pub input_b_size: u32,
    pub output_offset: u32,
    pub output_size: u32,
    pub scratch_offset: u32,
    pub scratch_size: u32,
    pub params_offset: u32,
    pub params_size: u32,
}

/// SwiGLU parameters
#[repr(C)]
pub struct SwiGluParams {
    /// Number of elements (total size = num_elements * hidden_dim)
    pub num_elements: u32,
    /// Hidden dimension
    pub hidden_dim: u32,
    /// Beta parameter for SiLU/Swish (typically 1.0)
    pub beta: f32,
}

/// Error codes
const OK: i32 = 0;
const INVALID_INPUT: i32 = 1;
const INVALID_OUTPUT: i32 = 2;
const INVALID_PARAMS: i32 = 3;

/// Initialize kernel
#[no_mangle]
pub extern "C" fn kernel_init(_params_ptr: *const u8, _params_len: u32) -> i32 {
    OK
}

/// Compute swish activation: x * sigmoid(beta * x)
#[inline]
fn swish(x: f32, beta: f32) -> f32 {
    x * sigmoid(beta * x)
}

/// Sigmoid function: 1 / (1 + exp(-x))
#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + expf(-x))
}

/// Execute SwiGLU forward pass
///
/// # Memory Layout
///
/// Input A (x): [num_elements, hidden_dim] as f32 (value to gate)
/// Input B (gate): [num_elements, hidden_dim] as f32 (gate values)
/// Output (y): [num_elements, hidden_dim] as f32
///
/// y = swish(gate) * x
#[no_mangle]
pub extern "C" fn kernel_forward(desc_ptr: *const KernelDescriptor) -> i32 {
    let desc = unsafe { &*desc_ptr };

    // Validate inputs
    if desc.input_a_size == 0 || desc.input_b_size == 0 {
        return INVALID_INPUT;
    }
    if desc.input_a_size != desc.input_b_size {
        return INVALID_INPUT; // x and gate must have same size
    }
    if desc.output_size == 0 || desc.output_size != desc.input_a_size {
        return INVALID_OUTPUT;
    }
    if desc.params_size < core::mem::size_of::<SwiGluParams>() as u32 {
        return INVALID_PARAMS;
    }

    let memory_base = 0usize as *mut u8;

    let params = unsafe {
        &*(memory_base.add(desc.params_offset as usize) as *const SwiGluParams)
    };

    let total_elements = (params.num_elements * params.hidden_dim) as usize;
    let beta = params.beta;

    // Get tensor pointers
    let x_ptr = unsafe { memory_base.add(desc.input_a_offset as usize) as *const f32 };
    let gate_ptr = unsafe { memory_base.add(desc.input_b_offset as usize) as *const f32 };
    let y_ptr = unsafe { memory_base.add(desc.output_offset as usize) as *mut f32 };

    // Apply SwiGLU: y = swish(gate) * x
    for i in 0..total_elements {
        unsafe {
            let x_val = *x_ptr.add(i);
            let gate_val = *gate_ptr.add(i);
            let swish_gate = swish(gate_val, beta);
            *y_ptr.add(i) = swish_gate * x_val;
        }
    }

    OK
}

/// Execute SwiGLU backward pass
///
/// Given grad_y, compute grad_x and grad_gate.
///
/// grad_x = swish(gate) * grad_y
/// grad_gate = x * grad_y * (sigmoid(gate) + gate * sigmoid(gate) * (1 - sigmoid(gate)))
///           = x * grad_y * sigmoid(gate) * (1 + gate * (1 - sigmoid(gate)))
///
/// For this simplified kernel:
/// Input A (grad_y): gradient from upstream
/// Input B contains both (x, gate) - simplified layout
/// Output (grad_x): gradient w.r.t. x
/// Scratch: gradient w.r.t. gate (if space available)
#[no_mangle]
pub extern "C" fn kernel_backward(desc_ptr: *const KernelDescriptor) -> i32 {
    let desc = unsafe { &*desc_ptr };

    if desc.input_a_size == 0 {
        return INVALID_INPUT;
    }
    if desc.output_size == 0 {
        return INVALID_OUTPUT;
    }
    if desc.params_size < core::mem::size_of::<SwiGluParams>() as u32 {
        return INVALID_PARAMS;
    }

    let memory_base = 0usize as *mut u8;

    let params = unsafe {
        &*(memory_base.add(desc.params_offset as usize) as *const SwiGluParams)
    };

    let total_elements = (params.num_elements * params.hidden_dim) as usize;
    let beta = params.beta;

    // For backward, input_b should contain original gate values
    // This is a simplified layout - real implementation would use separate descriptors
    let grad_y_ptr = unsafe { memory_base.add(desc.input_a_offset as usize) as *const f32 };
    let gate_ptr = unsafe { memory_base.add(desc.input_b_offset as usize) as *const f32 };
    let grad_x_ptr = unsafe { memory_base.add(desc.output_offset as usize) as *mut f32 };

    // Compute grad_x = swish(gate) * grad_y
    // (simplified: we would also need original x to compute grad_gate)
    for i in 0..total_elements {
        unsafe {
            let grad_y_val = *grad_y_ptr.add(i);
            let gate_val = *gate_ptr.add(i);
            let swish_gate = swish(gate_val, beta);
            *grad_x_ptr.add(i) = swish_gate * grad_y_val;
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

static KERNEL_NAME: &[u8] = b"swiglu_f32\0";

/// Get kernel metadata
#[no_mangle]
pub extern "C" fn kernel_info(info_ptr: *mut KernelInfo) -> i32 {
    if info_ptr.is_null() {
        return INVALID_PARAMS;
    }

    unsafe {
        (*info_ptr).name_ptr = KERNEL_NAME.as_ptr();
        (*info_ptr).name_len = KERNEL_NAME.len() as u32 - 1;
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
    OK
}

// Minimal exp implementation for no_std
fn expf(x: f32) -> f32 {
    // Handle edge cases
    if x > 88.0 {
        return f32::INFINITY;
    }
    if x < -88.0 {
        return 0.0;
    }

    // Use range reduction: exp(x) = 2^k * exp(r)
    // where k = round(x / ln(2)) and r = x - k * ln(2)
    const LN2: f32 = 0.693147180559945;
    const LN2_INV: f32 = 1.442695040888963;

    let k = (x * LN2_INV + 0.5).floor();
    let r = x - k * LN2;

    // Taylor series for exp(r) where |r| <= ln(2)/2
    // exp(r) ≈ 1 + r + r^2/2! + r^3/3! + r^4/4! + r^5/5! + r^6/6!
    let r2 = r * r;
    let r3 = r2 * r;
    let r4 = r2 * r2;
    let r5 = r4 * r;
    let r6 = r3 * r3;

    let exp_r = 1.0 + r + r2 * 0.5 + r3 * 0.166666667 + r4 * 0.041666667 + r5 * 0.008333333 + r6 * 0.001388889;

    // Combine: exp(x) = 2^k * exp(r)
    // 2^k can be computed via bit manipulation
    let k_int = k as i32;
    let scale_bits = ((127 + k_int) as u32) << 23;
    let scale = f32::from_bits(scale_bits);

    exp_r * scale
}

/// Compute GeGLU variant (alternative activation)
/// GeGLU(x, gate) = gelu(gate) * x
/// This is provided as an alternative, not used in default forward
#[allow(dead_code)]
fn gelu(x: f32) -> f32 {
    // Approximate GELU: 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
    const SQRT_2_OVER_PI: f32 = 0.7978845608028654;
    const COEFF: f32 = 0.044715;

    let x3 = x * x * x;
    let inner = SQRT_2_OVER_PI * (x + COEFF * x3);
    0.5 * x * (1.0 + tanhf(inner))
}

/// Minimal tanh implementation
#[allow(dead_code)]
fn tanhf(x: f32) -> f32 {
    // tanh(x) = (exp(2x) - 1) / (exp(2x) + 1)
    // For numerical stability with large |x|
    if x > 10.0 {
        return 1.0;
    }
    if x < -10.0 {
        return -1.0;
    }

    let exp_2x = expf(2.0 * x);
    (exp_2x - 1.0) / (exp_2x + 1.0)
}
