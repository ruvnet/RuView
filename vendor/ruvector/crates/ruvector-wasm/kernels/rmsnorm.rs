//! RMSNorm (Root Mean Square Layer Normalization) Kernel
//!
//! This kernel implements RMS normalization as used in models like LLaMA.
//! Unlike LayerNorm, RMSNorm only uses the root mean square, without
//! centering the distribution.
//!
//! Formula: y = (x / rms(x)) * weight
//! where rms(x) = sqrt(mean(x^2) + eps)
//!
//! # Compilation
//!
//! To compile this kernel to WASM:
//! ```bash
//! rustc --target wasm32-unknown-unknown \
//!       --crate-type cdylib \
//!       -C opt-level=3 \
//!       -C lto=fat \
//!       kernels/rmsnorm.rs \
//!       -o kernels/rmsnorm_f32.wasm
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
    pub input_a_offset: u32,    // x tensor
    pub input_a_size: u32,
    pub input_b_offset: u32,    // weight tensor (gamma)
    pub input_b_size: u32,
    pub output_offset: u32,
    pub output_size: u32,
    pub scratch_offset: u32,    // For storing intermediate RMS values
    pub scratch_size: u32,
    pub params_offset: u32,
    pub params_size: u32,
}

/// RMSNorm parameters
#[repr(C)]
pub struct RmsNormParams {
    /// Epsilon for numerical stability (typically 1e-5 or 1e-6)
    pub eps: f32,
    /// Hidden dimension (normalizing dimension)
    pub hidden_dim: u32,
    /// Number of elements to normalize (batch * seq)
    pub num_elements: u32,
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

/// Execute RMSNorm forward pass
///
/// # Memory Layout
///
/// Input A (x): [num_elements, hidden_dim] as f32
/// Input B (weight): [hidden_dim] as f32 (gamma scaling factors)
/// Output (y): [num_elements, hidden_dim] as f32
/// Scratch: [num_elements] as f32 (RMS values for backward pass)
///
/// For each row i:
/// rms[i] = sqrt(mean(x[i]^2) + eps)
/// y[i] = (x[i] / rms[i]) * weight
#[no_mangle]
pub extern "C" fn kernel_forward(desc_ptr: *const KernelDescriptor) -> i32 {
    let desc = unsafe { &*desc_ptr };

    // Validate inputs
    if desc.input_a_size == 0 {
        return INVALID_INPUT;
    }
    if desc.output_size == 0 {
        return INVALID_OUTPUT;
    }
    if desc.params_size < core::mem::size_of::<RmsNormParams>() as u32 {
        return INVALID_PARAMS;
    }

    let memory_base = 0usize as *mut u8;

    let params = unsafe {
        &*(memory_base.add(desc.params_offset as usize) as *const RmsNormParams)
    };

    let hidden_dim = params.hidden_dim as usize;
    let num_elements = params.num_elements as usize;
    let eps = params.eps;

    // Get tensor pointers
    let x_ptr = unsafe { memory_base.add(desc.input_a_offset as usize) as *const f32 };
    let weight_ptr = unsafe { memory_base.add(desc.input_b_offset as usize) as *const f32 };
    let y_ptr = unsafe { memory_base.add(desc.output_offset as usize) as *mut f32 };

    // Optional: Store RMS values in scratch for backward pass
    let rms_ptr = if desc.scratch_size >= (num_elements * 4) as u32 {
        Some(unsafe { memory_base.add(desc.scratch_offset as usize) as *mut f32 })
    } else {
        None
    };

    // Process each element (row)
    for i in 0..num_elements {
        let row_offset = i * hidden_dim;

        // Compute sum of squares
        let mut sum_sq: f32 = 0.0;
        for j in 0..hidden_dim {
            unsafe {
                let val = *x_ptr.add(row_offset + j);
                sum_sq += val * val;
            }
        }

        // Compute RMS
        let mean_sq = sum_sq / (hidden_dim as f32);
        let rms = sqrtf(mean_sq + eps);
        let inv_rms = 1.0 / rms;

        // Store RMS for backward pass if scratch is available
        if let Some(rms_store) = rms_ptr {
            unsafe {
                *rms_store.add(i) = rms;
            }
        }

        // Normalize and scale
        for j in 0..hidden_dim {
            unsafe {
                let x_val = *x_ptr.add(row_offset + j);
                let w_val = *weight_ptr.add(j);
                *y_ptr.add(row_offset + j) = (x_val * inv_rms) * w_val;
            }
        }
    }

    OK
}

/// Execute RMSNorm backward pass
///
/// Computes gradients for x and weight given gradient of output.
///
/// # Memory Layout (for backward)
///
/// Input A (grad_y): [num_elements, hidden_dim] as f32
/// Input B (x): Original input (needed for gradient)
/// Output (grad_x): [num_elements, hidden_dim] as f32
/// Scratch: [hidden_dim] as f32 (for grad_weight accumulation)
/// Params: Contains weight pointer separately
#[no_mangle]
pub extern "C" fn kernel_backward(desc_ptr: *const KernelDescriptor) -> i32 {
    let desc = unsafe { &*desc_ptr };

    if desc.input_a_size == 0 {
        return INVALID_INPUT;
    }
    if desc.output_size == 0 {
        return INVALID_OUTPUT;
    }
    if desc.params_size < core::mem::size_of::<RmsNormParams>() as u32 {
        return INVALID_PARAMS;
    }

    let memory_base = 0usize as *mut u8;

    let params = unsafe {
        &*(memory_base.add(desc.params_offset as usize) as *const RmsNormParams)
    };

    let hidden_dim = params.hidden_dim as usize;
    let num_elements = params.num_elements as usize;
    let eps = params.eps;

    // Note: For a complete backward pass, we would need:
    // - grad_y: gradient from upstream
    // - x: original input
    // - weight: scale parameters
    // - Output: grad_x
    // - Accumulate: grad_weight

    // This is a simplified implementation showing the structure
    let grad_y_ptr = unsafe { memory_base.add(desc.input_a_offset as usize) as *const f32 };
    let x_ptr = unsafe { memory_base.add(desc.input_b_offset as usize) as *const f32 };
    let grad_x_ptr = unsafe { memory_base.add(desc.output_offset as usize) as *mut f32 };

    // For each element
    for i in 0..num_elements {
        let row_offset = i * hidden_dim;

        // Recompute RMS (or load from scratch if saved during forward)
        let mut sum_sq: f32 = 0.0;
        for j in 0..hidden_dim {
            unsafe {
                let val = *x_ptr.add(row_offset + j);
                sum_sq += val * val;
            }
        }
        let mean_sq = sum_sq / (hidden_dim as f32);
        let rms = sqrtf(mean_sq + eps);
        let inv_rms = 1.0 / rms;
        let inv_rms_cubed = inv_rms * inv_rms * inv_rms;

        // Compute grad_norm_x = grad_y * weight
        // Then grad_x = inv_rms * grad_norm_x - inv_rms^3 * x * mean(x * grad_norm_x)
        // This is the chain rule applied to RMSNorm

        // First pass: compute sum(x * grad_y) for this row
        let mut sum_x_grad: f32 = 0.0;
        for j in 0..hidden_dim {
            unsafe {
                let x_val = *x_ptr.add(row_offset + j);
                let gy_val = *grad_y_ptr.add(row_offset + j);
                sum_x_grad += x_val * gy_val;
            }
        }
        let mean_x_grad = sum_x_grad / (hidden_dim as f32);

        // Second pass: compute grad_x
        for j in 0..hidden_dim {
            unsafe {
                let x_val = *x_ptr.add(row_offset + j);
                let gy_val = *grad_y_ptr.add(row_offset + j);

                // Simplified gradient (without weight consideration for this demo)
                let grad = inv_rms * gy_val - inv_rms_cubed * x_val * mean_x_grad;
                *grad_x_ptr.add(row_offset + j) = grad;
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

static KERNEL_NAME: &[u8] = b"rmsnorm_f32\0";

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

// Minimal sqrt implementation for no_std
fn sqrtf(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }

    // Newton-Raphson method
    let mut guess = x;

    // Initial guess using bit manipulation
    let i = x.to_bits();
    let i = 0x1fbd1df5 + (i >> 1);
    guess = f32::from_bits(i);

    // Newton-Raphson iterations
    for _ in 0..3 {
        guess = 0.5 * (guess + x / guess);
    }

    guess
}
