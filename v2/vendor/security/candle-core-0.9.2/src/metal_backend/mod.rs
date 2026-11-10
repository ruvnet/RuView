//! Implementation of Backend traits for Metal
//!
use crate::backend::{BackendDevice, BackendStorage};
use crate::conv::{ParamsConv1D, ParamsConv2D, ParamsConvTranspose1D, ParamsConvTranspose2D};
use crate::op::{BinaryOpT, CmpOp, ReduceOp, UnaryOpT};
use crate::{CpuStorage, CpuStorageRef, DType, Error, Layout, Result, Shape};
use candle_metal_kernels::{
    metal::{Buffer, Commands, Device},
    BufferOffset, CallConvTranspose2dCfg, Kernels, RESOURCE_OPTIONS,
};
use objc2_foundation::NSRange;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, PoisonError, RwLock, TryLockError};

mod device;
pub use device::{DeviceId, MetalDevice};

pub fn buffer_o<'a>(buffer: &'a Buffer, l: &Layout, dtype: DType) -> BufferOffset<'a> {
    BufferOffset {
        buffer,
        offset_in_bytes: l.start_offset() * dtype.size_in_bytes(),
    }
}
/// Simple way to catch lock error without
/// depending on T
#[derive(thiserror::Error, Debug)]
pub enum LockError {
    #[error("{0}")]
    Poisoned(String),
    #[error("Would block")]
    WouldBlock,
}

impl<T> From<TryLockError<T>> for MetalError {
    fn from(value: TryLockError<T>) -> Self {
        match value {
            TryLockError::Poisoned(p) => MetalError::LockError(LockError::Poisoned(p.to_string())),
            TryLockError::WouldBlock => MetalError::LockError(LockError::WouldBlock),
        }
    }
}

impl<T> From<PoisonError<T>> for MetalError {
    fn from(p: PoisonError<T>) -> Self {
        MetalError::LockError(LockError::Poisoned(p.to_string()))
    }
}

/// Metal related errors
#[derive(thiserror::Error, Debug)]
pub enum MetalError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    KernelError(#[from] candle_metal_kernels::MetalKernelError),
    #[error("{0:?}")]
    LockError(LockError),
    #[error("{msg}, expected: {expected:?}, got: {got:?}")]
    UnexpectedDType {
        msg: &'static str,
        expected: DType,
        got: DType,
    },
}

impl From<String> for MetalError {
    fn from(e: String) -> Self {
        MetalError::Message(e)
    }
}

#[derive(Debug, Clone)]
pub struct MetalStorage {
    /// The actual buffer containing the data.
    buffer: Arc<Buffer>,
    /// a reference to the device owning this buffer
    device: MetalDevice,
    /// The count of allocated elements in the buffer
    count: usize,
    /// The dtype is kept since buffers are untyped.
    dtype: DType,
}

impl BackendStorage for MetalStorage {
    type Device = MetalDevice;

    fn try_clone(&self, _: &Layout) -> Result<Self> {
        Ok(self.clone())
    }

    fn dtype(&self) -> DType {
        self.dtype
    }

    fn device(&self) -> &Self::Device {
        &self.device
    }

    fn to_cpu_storage(&self) -> Result<CpuStorage> {
        match self.dtype {
            DType::U8 => Ok(CpuStorage::U8(self.to_cpu()?)),
            DType::U32 => Ok(CpuStorage::U32(self.to_cpu()?)),
            DType::I16 => Ok(CpuStorage::I16(self.to_cpu()?)),
            DType::I32 => Ok(CpuStorage::I32(self.to_cpu()?)),
            DType::I64 => Ok(CpuStorage::I64(self.to_cpu()?)),
            DType::F16 => Ok(CpuStorage::F16(self.to_cpu()?)),
            DType::BF16 => Ok(CpuStorage::BF16(self.to_cpu()?)),
            DType::F32 => Ok(CpuStorage::F32(self.to_cpu()?)),
            DType::F64 => Ok(CpuStorage::F64(self.to_cpu()?)),
            DType::F8E4M3 => Ok(CpuStorage::F8E4M3(self.to_cpu()?)),
            DType::F6E2M3 | DType::F6E3M2 | DType::F4 | DType::F8E8M0 => {
                Err(crate::Error::UnsupportedDTypeForOp(self.dtype, "to_cpu_storage").bt())
            }
        }
    }

    fn affine(&self, layout: &Layout, mul: f64, add: f64) -> Result<Self> {
        let device = self.device().clone();

        let shape = layout.shape();
        let el = shape.elem_count();
        let dtype = self.dtype;

        let buffer = device.new_buffer(el, self.dtype, "affine")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("affine");
        let src = buffer_o(&self.buffer, layout, dtype);
        if layout.is_contiguous() {
            let name = match self.dtype {
                DType::F32 => "affine_f32",
                DType::F16 => "affine_f16",
                DType::BF16 => "affine_bf16",
                DType::U8 => "affine_u8",
                DType::U32 => "affine_u32",
                DType::I64 => "affine_i64",
                dtype => crate::bail!("Metal contiguous affine {dtype:?} not implemented"),
            };
            candle_metal_kernels::call_affine(
                &device.device,
                &encoder,
                &device.kernels,
                name,
                self.dtype.size_in_bytes(),
                el,
                src,
                &buffer,
                mul as f32,
                add as f32,
            )
            .map_err(MetalError::from)?;
        } else {
            let name = match self.dtype {
                DType::F32 => "affine_f32_strided",
                DType::F16 => "affine_f16_strided",
                DType::BF16 => "affine_bf16_strided",
                DType::U8 => "affine_u8_strided",
                DType::U32 => "affine_u32_strided",
                DType::I64 => "affine_i64_strided",
                dtype => crate::bail!("Metal strided affine {dtype:?} not implemented"),
            };
            candle_metal_kernels::call_affine_strided(
                &device.device,
                &encoder,
                &device.kernels,
                name,
                layout.dims(),
                src,
                layout.stride(),
                &buffer,
                mul as f32,
                add as f32,
            )
            .map_err(MetalError::from)?;
        }
        Ok(Self::new(buffer, device.clone(), el, dtype))
    }

    fn powf(&self, layout: &Layout, pow: f64) -> Result<Self> {
        let device = self.device().clone();

        let shape = layout.shape();
        let el = shape.elem_count();
        let dtype = self.dtype;

        let buffer = device.new_buffer(el, self.dtype, "powf")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("powf");
        let src = buffer_o(&self.buffer, layout, dtype);
        if layout.is_contiguous() {
            let name = match self.dtype {
                DType::F32 => "powf_f32",
                DType::F16 => "powf_f16",
                DType::BF16 => "powf_bf16",
                dtype => crate::bail!("Metal contiguous powf {dtype:?} not implemented"),
            };
            candle_metal_kernels::call_powf(
                &device.device,
                &encoder,
                &device.kernels,
                name,
                self.dtype.size_in_bytes(),
                el,
                src,
                &buffer,
                pow as f32,
            )
            .map_err(MetalError::from)?;
        } else {
            let name = match self.dtype {
                DType::F32 => "powf_f32_strided",
                DType::F16 => "powf_f16_strided",
                DType::BF16 => "powf_bf16_strided",
                dtype => crate::bail!("Metal strided powf {dtype:?} not implemented"),
            };
            candle_metal_kernels::call_powf_strided(
                &device.device,
                &encoder,
                &device.kernels,
                name,
                layout.dims(),
                src,
                layout.stride(),
                &buffer,
                pow as f32,
            )
            .map_err(MetalError::from)?;
        }
        Ok(Self::new(buffer, device.clone(), el, dtype))
    }

    fn elu(&self, layout: &Layout, alpha: f64) -> Result<Self> {
        let device = self.device().clone();

        let shape = layout.shape();
        let el = shape.elem_count();
        let dtype = self.dtype;

        let buffer = device.new_buffer(el, self.dtype, "elu")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("elu");
        let src = buffer_o(&self.buffer, layout, self.dtype);
        if layout.is_contiguous() {
            let name = match self.dtype {
                DType::F32 => "elu_f32",
                DType::F16 => "elu_f16",
                DType::BF16 => "elu_bf16",
                dtype => crate::bail!("Metal contiguous elu {dtype:?} not implemented"),
            };
            candle_metal_kernels::call_elu(
                &device.device,
                &encoder,
                &device.kernels,
                name,
                self.dtype.size_in_bytes(),
                el,
                src,
                &buffer,
                alpha as f32,
            )
            .map_err(MetalError::from)?;
        } else {
            let name = match self.dtype {
                DType::F32 => "elu_f32_strided",
                DType::F16 => "elu_f16_strided",
                DType::BF16 => "elu_bf16_strided",
                dtype => crate::bail!("Metal strided elu {dtype:?} not implemented"),
            };
            candle_metal_kernels::call_elu_strided(
                &device.device,
                &encoder,
                &device.kernels,
                name,
                layout.dims(),
                src,
                layout.stride(),
                &buffer,
                alpha as f32,
            )
            .map_err(MetalError::from)?;
        }
        Ok(Self::new(buffer, device.clone(), el, dtype))
    }

    fn reduce_op(&self, op: ReduceOp, layout: &Layout, sum_dims: &[usize]) -> Result<Self> {
        let device = self.device.clone();

        let src_stride = layout.stride();
        let src_dims = layout.shape().dims();
        // Source dims and strides with the sum dims at the end.
        let mut dims = vec![];
        let mut stride = vec![];
        let mut dst_el: usize = 1;
        for (dim_idx, &d) in src_dims.iter().enumerate() {
            if !sum_dims.contains(&dim_idx) {
                dst_el *= d;
                dims.push(d);
                stride.push(src_stride[dim_idx]);
            }
        }

        for &dim_idx in sum_dims.iter() {
            dims.push(src_dims[dim_idx]);
            stride.push(src_stride[dim_idx]);
        }

        let reduction_shape = Shape::from(dims.clone());

        if layout.is_contiguous() && reduction_shape.is_contiguous(&stride) {
            let (name, check_empty, return_index) = match (op, self.dtype) {
                (ReduceOp::Sum, DType::F32) => ("fast_sum_f32", false, false),
                (ReduceOp::Min, DType::F32) => ("fast_min_f32", true, false),
                (ReduceOp::Max, DType::F32) => ("fast_max_f32", true, false),
                (ReduceOp::ArgMin, DType::F32) => ("fast_argmin_f32", true, true),
                (ReduceOp::ArgMax, DType::F32) => ("fast_argmax_f32", true, true),
                (ReduceOp::Sum, DType::U32) => ("fast_sum_u32", false, false),
                (ReduceOp::Min, DType::U32) => ("fast_min_u32", true, false),
                (ReduceOp::Max, DType::U32) => ("fast_max_u32", true, false),
                (ReduceOp::ArgMin, DType::U32) => ("fast_argmin_u32", true, true),
                (ReduceOp::ArgMax, DType::U32) => ("fast_argmax_u32", true, true),
                (ReduceOp::Sum, DType::F16) => ("fast_sum_f16", false, false),
                (ReduceOp::Min, DType::F16) => ("fast_min_f16", true, false),
                (ReduceOp::Max, DType::F16) => ("fast_max_f16", true, false),
                (ReduceOp::ArgMin, DType::F16) => ("fast_argmin_f16", true, true),
                (ReduceOp::ArgMax, DType::F16) => ("fast_argmax_f16", true, true),
                (ReduceOp::Sum, DType::BF16) => ("fast_sum_bf16", false, false),
                (ReduceOp::Min, DType::BF16) => ("fast_min_bf16", true, false),
                (ReduceOp::Max, DType::BF16) => ("fast_max_bf16", true, false),
                (ReduceOp::ArgMin, DType::BF16) => ("fast_argmin_bf16", true, true),
                (ReduceOp::ArgMax, DType::BF16) => ("fast_argmax_bf16", true, true),
                (ReduceOp::Sum, DType::I64) => ("fast_sum_i64", false, false),
                (ReduceOp::Min, DType::I64) => ("fast_min_i64", true, false),
                (ReduceOp::Max, DType::I64) => ("fast_max_i64", true, false),
                (ReduceOp::ArgMin, DType::I64) => ("fast_argmin_i64", true, true),
                (ReduceOp::ArgMax, DType::I64) => ("fast_argmax_i64", true, true),
                (ReduceOp::Sum, DType::U8) => ("fast_sum_u8", false, false),
                (ReduceOp::Min, DType::U8) => ("fast_min_u8", true, false),
                (ReduceOp::Max, DType::U8) => ("fast_max_u8", true, false),
                (ReduceOp::ArgMin, DType::U8) => ("fast_argmin_u8", true, true),
                (ReduceOp::ArgMax, DType::U8) => ("fast_argmax_u8", true, true),
                (k, dtype) => {
                    crate::bail!("Metal contiguous reduce op {k:?} {dtype:?} not implemented")
                }
            };
            if check_empty && layout.shape().elem_count() == 0 {
                Err(crate::Error::EmptyTensor { op: "reduce" }.bt())?
            }
            let dtype = if return_index { DType::U32 } else { self.dtype };
            let buffer = device.new_buffer(dst_el, dtype, "reduce")?;
            let encoder = self.device.command_encoder()?;
            encoder.set_label("reduce");
            let src = buffer_o(&self.buffer, layout, self.dtype);
            candle_metal_kernels::call_reduce_contiguous(
                &device.device,
                &encoder,
                &device.kernels,
                name,
                src_dims,
                dst_el,
                src,
                &buffer,
            )
            .map_err(MetalError::from)?;

            return Ok(Self::new(buffer, device, dst_el, dtype));
        }

        let (name, check_empty, return_index) = match (op, self.dtype) {
            (ReduceOp::Sum, DType::F32) => ("fast_sum_f32_strided", false, false),
            (ReduceOp::Min, DType::F32) => ("fast_min_f32_strided", true, false),
            (ReduceOp::Max, DType::F32) => ("fast_max_f32_strided", true, false),
            (ReduceOp::ArgMin, DType::F32) => ("fast_argmin_f32_strided", true, true),
            (ReduceOp::ArgMax, DType::F32) => ("fast_argmax_f32_strided", true, true),
            (ReduceOp::Sum, DType::U32) => ("fast_sum_u32_strided", false, false),
            (ReduceOp::Min, DType::U32) => ("fast_min_u32_strided", true, false),
            (ReduceOp::Max, DType::U32) => ("fast_max_u32_strided", true, false),
            (ReduceOp::ArgMin, DType::U32) => ("fast_argmin_u32_strided", true, true),
            (ReduceOp::ArgMax, DType::U32) => ("fast_argmax_u32_strided", true, true),
            (ReduceOp::Sum, DType::F16) => ("fast_sum_f16_strided", false, false),
            (ReduceOp::Min, DType::F16) => ("fast_min_f16_strided", true, false),
            (ReduceOp::Max, DType::F16) => ("fast_max_f16_strided", true, false),
            (ReduceOp::ArgMin, DType::F16) => ("fast_argmin_f16_strided", true, true),
            (ReduceOp::ArgMax, DType::F16) => ("fast_argmax_f16_strided", true, true),
            (ReduceOp::Sum, DType::BF16) => ("fast_sum_bf16_strided", false, false),
            (ReduceOp::Min, DType::BF16) => ("fast_min_bf16_strided", true, false),
            (ReduceOp::Max, DType::BF16) => ("fast_max_bf16_strided", true, false),
            (ReduceOp::ArgMin, DType::BF16) => ("fast_argmin_bf16_strided", true, true),
            (ReduceOp::ArgMax, DType::BF16) => ("fast_argmax_bf16_strided", true, true),
            (ReduceOp::Sum, DType::I64) => ("fast_sum_i64_strided", false, false),
            (ReduceOp::Min, DType::I64) => ("fast_min_i64_strided", true, false),
            (ReduceOp::Max, DType::I64) => ("fast_max_i64_strided", true, false),
            (ReduceOp::ArgMin, DType::I64) => ("fast_argmin_i64_strided", true, true),
            (ReduceOp::ArgMax, DType::I64) => ("fast_argmax_i64_strided", true, true),
            (ReduceOp::Sum, DType::U8) => ("fast_sum_u8_strided", false, false),
            (ReduceOp::Min, DType::U8) => ("fast_min_u8_strided", true, false),
            (ReduceOp::Max, DType::U8) => ("fast_max_u8_strided", true, false),
            (ReduceOp::ArgMin, DType::U8) => ("fast_argmin_u8_strided", true, true),
            (ReduceOp::ArgMax, DType::U8) => ("fast_argmax_u8_strided", true, true),
            (k, dtype) => crate::bail!("Metal strided reduce op {k:?} {dtype:?} not implemented"),
        };
        if check_empty && layout.shape().elem_count() == 0 {
            Err(crate::Error::EmptyTensor { op: "reduce" }.bt())?
        }
        let dtype = if return_index { DType::U32 } else { self.dtype };
        let buffer = device.new_buffer(dst_el, dtype, "reduce")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("reduce");
        let src = buffer_o(&self.buffer, layout, self.dtype);
        candle_metal_kernels::call_reduce_strided(
            &device.device,
            &encoder,
            &device.kernels,
            name,
            &dims,
            &stride,
            dst_el,
            src,
            &buffer,
        )
        .map_err(MetalError::from)?;

        Ok(Self::new(buffer, device, dst_el, dtype))
    }

    fn cmp(&self, op: CmpOp, rhs: &Self, lhs_l: &Layout, rhs_l: &Layout) -> Result<Self> {
        let name = match op {
            CmpOp::Eq => "eq",
            CmpOp::Ne => "ne",
            CmpOp::Le => "le",
            CmpOp::Ge => "ge",
            CmpOp::Lt => "lt",
            CmpOp::Gt => "gt",
        };
        self.binary(name, rhs, lhs_l, rhs_l)
    }

    fn const_set(&mut self, s: crate::scalar::Scalar, l: &Layout) -> Result<()> {
        use crate::scalar::Scalar;
        fn set<S: crate::WithDType + candle_metal_kernels::utils::EncoderParam>(
            self_: &mut MetalStorage,
            s: S,
            l: &Layout,
        ) -> Result<()> {
            let device = self_.device();
            let dtype = self_.dtype;
            let shape = l.shape();
            let el_count = shape.elem_count();
            let encoder = device.command_encoder()?;
            encoder.set_label("const-set");
            let dst = buffer_o(&self_.buffer, l, self_.dtype);

            if l.is_contiguous() {
                use candle_metal_kernels::unary::contiguous;
                let kernel_name = match dtype {
                    DType::F16 => contiguous::const_set::HALF,
                    DType::BF16 => contiguous::const_set::BFLOAT,
                    DType::F32 => contiguous::const_set::FLOAT,
                    DType::I64 => contiguous::const_set::I64,
                    DType::U32 => contiguous::const_set::U32,
                    DType::U8 => contiguous::const_set::U8,
                    DType::F8E4M3 => crate::bail!("unsupported const-set f8e4m3"),
                    DType::F64 => crate::bail!("unsupported const-set f64"),
                    DType::F4
                    | DType::F6E2M3
                    | DType::F6E3M2
                    | DType::F8E8M0
                    | DType::I16
                    | DType::I32 => {
                        return Err(Error::UnsupportedDTypeForOp(dtype, "const-set").bt())
                    }
                };
                candle_metal_kernels::call_const_set_contiguous(
                    &device.device,
                    &encoder,
                    &device.kernels,
                    kernel_name,
                    dtype.size_in_bytes(),
                    el_count,
                    s,
                    dst,
                )
                .map_err(MetalError::from)?;
            } else {
                use candle_metal_kernels::unary::strided;
                let kernel_name = match dtype {
                    DType::F16 => strided::const_set::HALF,
                    DType::BF16 => strided::const_set::BFLOAT,
                    DType::F32 => strided::const_set::FLOAT,
                    DType::I64 => strided::const_set::I64,
                    DType::U32 => strided::const_set::U32,
                    DType::U8 => strided::const_set::U8,
                    DType::F8E4M3 => crate::bail!("unsupported const-set f8e4m3"),
                    DType::F64 => crate::bail!("unsupported const-set f64"),
                    DType::F4
                    | DType::F6E2M3
                    | DType::F6E3M2
                    | DType::F8E8M0
                    | DType::I16
                    | DType::I32 => {
                        return Err(Error::UnsupportedDTypeForOp(dtype, "const-set").bt())
                    }
                };
                candle_metal_kernels::call_const_set_strided(
                    &device.device,
                    &encoder,
                    &device.kernels,
                    kernel_name,
                    l.dims(),
                    s,
                    l.stride(),
                    dst,
                )
                .map_err(MetalError::from)?;
            }
            Ok(())
        }
        match (self.dtype, s) {
            (DType::U8, Scalar::U8(s)) => set(self, s, l),
            (DType::U32, Scalar::U32(s)) => set(self, s, l),
            (DType::I64, Scalar::I64(s)) => set(self, s, l),
            (DType::F16, Scalar::F16(s)) => set(self, s, l),
            (DType::BF16, Scalar::BF16(s)) => set(self, s, l),
            (DType::F32, Scalar::F32(s)) => set(self, s, l),
            (DType::F64, Scalar::F64(s)) => set(self, s, l),
            _ => crate::bail!("dtype mismatch, expected {:?}, got {:?}", self.dtype, s),
        }
    }

    fn to_dtype(&self, layout: &Layout, dtype: DType) -> Result<Self> {
        let device = self.device();
        let shape = layout.shape();
        let el_count = shape.elem_count();
        let buffer = device.new_buffer(el_count, dtype, "to_dtype")?;
        let encoder = device.command_encoder()?;
        encoder.set_label("to_dtype");
        let src = buffer_o(&self.buffer, layout, self.dtype);
        if layout.is_contiguous() {
            let kernel_name = match (self.dtype, dtype) {
                (DType::U32, DType::BF16) => "cast_u32_bf16",
                (DType::U32, DType::F16) => "cast_u32_f16",
                (DType::U32, DType::F32) => "cast_u32_f32",
                (DType::U32, DType::I64) => "cast_u32_i64",
                (DType::U32, DType::U8) => "cast_u32_u8",

                (DType::U8, DType::BF16) => "cast_u8_bf16",
                (DType::U8, DType::F16) => "cast_u8_f16",
                (DType::U8, DType::F32) => "cast_u8_f32",
                (DType::U8, DType::I64) => "cast_u8_i64",
                (DType::U8, DType::U32) => "cast_u8_u32",

                (DType::F32, DType::BF16) => "cast_f32_bf16",
                (DType::F32, DType::F16) => "cast_f32_f16",
                (DType::F32, DType::I64) => "cast_f32_i64",
                (DType::F32, DType::U32) => "cast_f32_u32",
                (DType::F32, DType::U8) => "cast_f32_u8",

                (DType::I64, DType::BF16) => "cast_i64_bf16",
                (DType::I64, DType::F16) => "cast_i64_f16",
                (DType::I64, DType::F32) => "cast_i64_f32",
                (DType::I64, DType::U32) => "cast_i64_u32",
                (DType::I64, DType::U8) => "cast_i64_u8",

                (DType::F16, DType::BF16) => "cast_f16_bf16",
                (DType::F16, DType::F32) => "cast_f16_f32",
                (DType::F16, DType::I64) => "cast_f16_i64",
                (DType::F16, DType::U32) => "cast_f16_u32",
                (DType::F16, DType::U8) => "cast_f16_u8",

                (DType::BF16, DType::F16) => "cast_bf16_f16",
                (DType::BF16, DType::F32) => "cast_bf16_f32",
                (DType::BF16, DType::I64) => "cast_bf16_i64",
                (DType::BF16, DType::U32) => "cast_bf16_u32",
                (DType::BF16, DType::U8) => "cast_bf16_u8",

                (left, right) => {
                    crate::bail!("Metal contiguous to_dtype {left:?} {right:?} not implemented")
                }
            };
            candle_metal_kernels::call_cast_contiguous(
                &device.device,
                &encoder,
                &device.kernels,
                kernel_name,
                self.dtype.size_in_bytes(),
                el_count,
                src,
                &buffer,
            )
            .map_err(MetalError::from)?;
        } else {
            let kernel_name = match (self.dtype, dtype) {
                (DType::BF16, DType::F16) => "cast_bf16_f16_strided",
                (DType::BF16, DType::F32) => "cast_bf16_f32_strided",
                (DType::BF16, DType::I64) => "cast_bf16_i64_strided",
                (DType::BF16, DType::U32) => "cast_bf16_u32_strided",
                (DType::BF16, DType::U8) => "cast_bf16_u8_strided",

                (DType::F16, DType::BF16) => "cast_f16_bf16_strided",
                (DType::F16, DType::F32) => "cast_f16_f32_strided",
                (DType::F16, DType::I64) => "cast_f16_i64_strided",
                (DType::F16, DType::U32) => "cast_f16_u32_strided",
                (DType::F16, DType::U8) => "cast_f16_u8_strided",

                (DType::F32, DType::BF16) => "cast_f32_bf16_strided",
                (DType::F32, DType::F16) => "cast_f32_f16_strided",
                (DType::F32, DType::I64) => "cast_f32_i64_strided",
                (DType::F32, DType::U32) => "cast_f32_u32_strided",
                (DType::F32, DType::U8) => "cast_f32_u8_strided",

                (DType::I64, DType::F32) => "cast_i64_f32_strided",
                (DType::I64, DType::BF16) => "cast_i64_bf16_strided",
                (DType::I64, DType::F16) => "cast_i64_f16_strided",
                (DType::I64, DType::U32) => "cast_i64_u32_strided",
                (DType::I64, DType::U8) => "cast_i64_u8_strided",

                (DType::U32, DType::BF16) => "cast_u32_bf16_strided",
                (DType::U32, DType::F16) => "cast_u32_f16_strided",
                (DType::U32, DType::F32) => "cast_u32_f32_strided",
                (DType::U32, DType::I64) => "cast_u32_i64_strided",
                (DType::U32, DType::U8) => "cast_u32_u8_strided",

                (DType::U8, DType::BF16) => "cast_u8_bf16_strided",
                (DType::U8, DType::F16) => "cast_u8_f16_strided",
                (DType::U8, DType::F32) => "cast_u8_f32_strided",
                (DType::U8, DType::I64) => "cast_u8_i64_strided",
                (DType::U8, DType::U32) => "cast_u8_u32_strided",

                (left, right) => {
                    crate::bail!("Metal strided to_dtype {left:?} {right:?} not implemented")
                }
            };
            candle_metal_kernels::call_cast_strided(
                &device.device,
                &encoder,
                &device.kernels,
                kernel_name,
                layout.dims(),
                src,
                layout.stride(),
                &buffer,
            )
            .map_err(MetalError::from)?;
        }
        Ok(Self::new(buffer, device.clone(), el_count, dtype))
    }

    fn unary_impl<B: UnaryOpT>(&self, layout: &Layout) -> Result<Self> {
        let device = self.device();
        let dtype = self.dtype;
        let shape = layout.shape();
        let el_count = shape.elem_count();
        let buffer = device.new_buffer(el_count, dtype, B::KERNEL)?;
        let encoder = device.command_encoder()?;
        encoder.set_label(B::KERNEL);
        let src = buffer_o(&self.buffer, layout, self.dtype);

        if layout.is_contiguous() {
            use candle_metal_kernels::unary::contiguous;
            let kernel_name = match (B::KERNEL, dtype) {
                ("uabs", DType::F16) => contiguous::abs::HALF,
                ("uabs", DType::F32) => contiguous::abs::FLOAT,
                ("uabs", DType::BF16) => contiguous::abs::BFLOAT,
                ("uceil", DType::F16) => contiguous::ceil::HALF,
                ("uceil", DType::F32) => contiguous::ceil::FLOAT,
                ("uceil", DType::BF16) => contiguous::ceil::BFLOAT,
                ("ucos", DType::F16) => contiguous::cos::HALF,
                ("ucos", DType::F32) => contiguous::cos::FLOAT,
                ("ucos", DType::BF16) => contiguous::cos::BFLOAT,
                ("uerf", DType::F16) => contiguous::erf::HALF,
                ("uerf", DType::F32) => contiguous::erf::FLOAT,
                ("uerf", DType::BF16) => contiguous::erf::BFLOAT,
                ("uexp", DType::F16) => contiguous::exp::HALF,
                ("uexp", DType::F32) => contiguous::exp::FLOAT,
                ("uexp", DType::BF16) => contiguous::exp::BFLOAT,
                ("ufloor", DType::F16) => contiguous::floor::HALF,
                ("ufloor", DType::F32) => contiguous::floor::FLOAT,
                ("ufloor", DType::BF16) => contiguous::floor::BFLOAT,
                ("ugelu_erf", DType::F16) => contiguous::gelu_erf::HALF,
                ("ugelu_erf", DType::F32) => contiguous::gelu_erf::FLOAT,
                ("ugelu_erf", DType::BF16) => contiguous::gelu_erf::BFLOAT,
                ("ugelu", DType::F16) => contiguous::gelu::HALF,
                ("ugelu", DType::F32) => contiguous::gelu::FLOAT,
                ("ugelu", DType::BF16) => contiguous::gelu::BFLOAT,
                ("ulog", DType::F16) => contiguous::log::HALF,
                ("ulog", DType::F32) => contiguous::log::FLOAT,
                ("ulog", DType::BF16) => contiguous::log::BFLOAT,
                ("uneg", DType::F16) => contiguous::neg::HALF,
                ("uneg", DType::F32) => contiguous::neg::FLOAT,
                ("uneg", DType::BF16) => contiguous::neg::BFLOAT,
                ("urecip", DType::F16) => contiguous::recip::HALF,
                ("urecip", DType::F32) => contiguous::recip::FLOAT,
                ("urecip", DType::BF16) => contiguous::recip::BFLOAT,
                ("urelu", DType::F16) => contiguous::relu::HALF,
                ("urelu", DType::F32) => contiguous::relu::FLOAT,
                ("urelu", DType::BF16) => contiguous::relu::BFLOAT,
                ("uround", DType::F16) => contiguous::round::HALF,
                ("uround", DType::F32) => contiguous::round::FLOAT,
                ("uround", DType::BF16) => contiguous::round::BFLOAT,
                ("usilu", DType::F16) => contiguous::silu::HALF,
                ("usilu", DType::F32) => contiguous::silu::FLOAT,
                ("usilu", DType::BF16) => contiguous::silu::BFLOAT,
                ("usin", DType::F16) => contiguous::sin::HALF,
                ("usin", DType::F32) => contiguous::sin::FLOAT,
                ("usin", DType::BF16) => contiguous::sin::BFLOAT,
                ("usqr", DType::F16) => contiguous::sqr::HALF,
                ("usqr", DType::F32) => contiguous::sqr::FLOAT,
                ("usqr", DType::BF16) => contiguous::sqr::BFLOAT,
                ("usqrt", DType::F16) => contiguous::sqrt::HALF,
                ("usqrt", DType::F32) => contiguous::sqrt::FLOAT,
                ("usqrt", DType::BF16) => contiguous::sqrt::BFLOAT,
                ("utanh", DType::F16) => contiguous::tanh::HALF,
                ("utanh", DType::F32) => contiguous::tanh::FLOAT,
                ("utanh", DType::BF16) => contiguous::tanh::BFLOAT,
                ("usign", DType::F16) => contiguous::sign::HALF,
                ("usign", DType::F32) => contiguous::sign::FLOAT,
                ("usign", DType::BF16) => contiguous::sign::BFLOAT,
                ("usign", DType::I64) => contiguous::sign::I64,
                (name, dtype) => {
                    crate::bail!("Metal contiguous unary {name} {dtype:?} not implemented")
                }
            };

            candle_metal_kernels::call_unary_contiguous(
                &device.device,
                &encoder,
                &device.kernels,
                kernel_name,
                dtype.size_in_bytes(),
                el_count,
                src,
                &buffer,
            )
            .map_err(MetalError::from)?;
        } else {
            use candle_metal_kernels::unary::strided;
            let kernel_name = match (B::KERNEL, dtype) {
                ("ucos", DType::F32) => strided::cos::FLOAT,
                ("usin", DType::F32) => strided::sin::FLOAT,
                ("usqr", DType::F32) => strided::sqr::FLOAT,
                ("usqrt", DType::F32) => strided::sqrt::FLOAT,
                ("uneg", DType::F32) => strided::neg::FLOAT,
                ("uexp", DType::F32) => strided::exp::FLOAT,
                ("ulog", DType::F32) => strided::log::FLOAT,
                ("ugelu", DType::F32) => strided::gelu::FLOAT,
                ("ugelu_erf", DType::F32) => strided::gelu_erf::FLOAT,
                ("uerf", DType::F32) => strided::erf::FLOAT,
                ("usilu", DType::F32) => strided::silu::FLOAT,
                ("uabs", DType::F32) => strided::abs::FLOAT,
                ("uceil", DType::F32) => strided::ceil::FLOAT,
                ("ufloor", DType::F32) => strided::floor::FLOAT,
                ("urelu", DType::F32) => strided::relu::FLOAT,
                ("uround", DType::F32) => strided::round::FLOAT,
                ("utanh", DType::F32) => strided::tanh::FLOAT,

                ("ucos", DType::F16) => strided::cos::HALF,
                ("usin", DType::F16) => strided::sin::HALF,
                ("usqr", DType::F16) => strided::sqr::HALF,
                ("usqrt", DType::F16) => strided::sqrt::HALF,
                ("uneg", DType::F16) => strided::neg::HALF,
                ("uexp", DType::F16) => strided::exp::HALF,
                ("ulog", DType::F16) => strided::log::HALF,
                ("ugelu", DType::F16) => strided::gelu::HALF,
                ("ugelu_erf", DType::F16) => strided::gelu_erf::HALF,
                ("uerf", DType::F16) => strided::erf::HALF,
                ("usilu", DType::F16) => strided::silu::HALF,
                ("uabs", DType::F16) => strided::abs::HALF,
                ("uceil", DType::F16) => strided::ceil::HALF,
                ("ufloor", DType::F16) => strided::floor::HALF,
                ("urelu", DType::F16) => strided::relu::HALF,
                ("uround", DType::F16) => strided::round::HALF,
                ("utanh", DType::F16) => strided::tanh::HALF,

                ("ucos", DType::BF16) => strided::cos::BFLOAT,
                ("usin", DType::BF16) => strided::sin::BFLOAT,
                ("usqr", DType::BF16) => strided::sqr::BFLOAT,
                ("usqrt", DType::BF16) => strided::sqrt::BFLOAT,
                ("uneg", DType::BF16) => strided::neg::BFLOAT,
                ("uexp", DType::BF16) => strided::exp::BFLOAT,
                ("ulog", DType::BF16) => strided::log::BFLOAT,
                ("ugelu", DType::BF16) => strided::gelu::BFLOAT,
                ("ugelu_erf", DType::BF16) => strided::gelu_erf::BFLOAT,
                ("uerf", DType::BF16) => strided::erf::BFLOAT,
                ("usilu", DType::BF16) => strided::silu::BFLOAT,
                ("uabs", DType::BF16) => strided::abs::BFLOAT,
                ("uceil", DType::BF16) => strided::ceil::BFLOAT,
                ("ufloor", DType::BF16) => strided::floor::BFLOAT,
                ("urelu", DType::BF16) => strided::relu::BFLOAT,
                ("uround", DType::BF16) => strided::round::BFLOAT,
                ("utanh", DType::BF16) => strided::tanh::BFLOAT,

                (name, dtype) => {
                    crate::bail!("Metal strided unary {name} {dtype:?} not implemented")
                }
            };
            let dst = BufferOffset::zero_offset(&buffer);
            candle_metal_kernels::call_unary_strided(
                &device.device,
                &encoder,
                &device.kernels,
                kernel_name,
                layout.dims(),
                src,
                layout.stride(),
                dst,
            )
            .map_err(MetalError::from)?;
        }

        Ok(Self::new(buffer, device.clone(), el_count, dtype))
    }

    fn binary_impl<B: BinaryOpT>(
        &self,
        rhs: &Self,
        lhs_l: &Layout,
        rhs_l: &Layout,
    ) -> Result<Self> {
        self.binary(B::KERNEL, rhs, lhs_l, rhs_l)
    }

    fn where_cond(
        &self,
        layout: &Layout,
        t: &Self,
        t_l: &Layout,
        f: &Self,
        f_l: &Layout,
    ) -> Result<Self> {
        let device = self.device.clone();
        let shape = t_l.shape();
        let dims = shape.dims();
        let el = shape.elem_count();
        let dtype = t.dtype;
        let buffer = self.device.new_buffer(el, dtype, "where")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("where");
        if t.dtype() != f.dtype() {
            crate::bail!(
                "Invalid where: different dtypes for values {:?} != {:?}",
                t.dtype(),
                f.dtype()
            );
        }
        let name = match (self.dtype, t.dtype()) {
            (DType::U8, DType::F32) => "where_u8_f32",
            (DType::U32, DType::F32) => "where_u32_f32",
            (DType::U8, DType::BF16) => "where_u8_bf16",
            (DType::U8, DType::F16) => "where_u8_f16",
            (DType::U8, DType::I64) => "where_u8_i64",
            (DType::U8, DType::U32) => "where_u8_u32",
            (DType::U8, DType::U8) => "where_u8_u8",
            (left, right) => crate::bail!("Metal where_cond {left:?} {right:?} not implemented"),
        };
        let src = buffer_o(&self.buffer, layout, self.dtype);
        let t = buffer_o(&t.buffer, t_l, t.dtype);
        let f = buffer_o(&f.buffer, f_l, f.dtype);
        candle_metal_kernels::call_where_cond(
            &device.device,
            &encoder,
            &device.kernels,
            name,
            dtype.size_in_bytes(),
            dims,
            src,
            layout.stride(),
            layout.is_contiguous(),
            t,
            t_l.stride(),
            t_l.is_contiguous(),
            f,
            f_l.stride(),
            f_l.is_contiguous(),
            &buffer,
        )
        .map_err(MetalError::from)?;
        Ok(Self::new(buffer, device, el, dtype))
    }

    fn conv1d(
        &self,
        layout: &Layout,
        kernel: &Self,
        kernel_l: &Layout,
        params: &ParamsConv1D,
    ) -> Result<Self> {
        let device = self.device().clone();
        let shape = layout.shape();
        let dims = shape.dims();
        let strides = layout.stride();

        let stride = params.stride;
        let dilation = params.dilation;
        let padding = params.padding;
        let k_size = params.k_size;
        let l_out = (dims[2] + 2 * padding - dilation * (k_size - 1) - 1) / stride + 1;
        let dst_el = dims[0] * l_out * dims[1] * k_size;
        let dst = self
            .device
            .new_buffer(dst_el, self.dtype, "conv1d_im2col")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("conv1d_im2col");
        let name = match self.dtype {
            DType::F32 => "im2col1d_f32",
            DType::F16 => "im2col1d_f16",
            DType::BF16 => "im2col1d_bf16",
            DType::U8 => "im2col1d_u8",
            DType::U32 => "im2col1d_u32",
            dtype => crate::bail!("Metal conv1d {dtype:?} not implemented"),
        };
        let src = buffer_o(&self.buffer, layout, self.dtype);
        candle_metal_kernels::call_im2col1d_strided(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            name,
            layout.shape().dims(),
            strides,
            (k_size, stride, padding, dilation),
            src,
            &dst,
        )
        .map_err(MetalError::from)?;
        drop(encoder);
        let col = Self {
            buffer: dst,
            device,
            count: dst_el,
            dtype: self.dtype,
        };
        let l_out = params.l_out();
        let b = params.b_size;
        let n = params.c_out;
        let k = params.k_size * params.c_in;
        let m = l_out;
        let col_l = Layout::contiguous((b, m, k));
        let res = if kernel_l.is_contiguous() {
            let kernel_l = Layout::contiguous_with_offset((1, n, k), kernel_l.start_offset())
                .transpose(1, 2)?
                .broadcast_as((b, k, n))?;
            col.matmul(kernel, (b, m, n, k), &col_l, &kernel_l)?
        } else {
            // Make the kernel contiguous if not already the case.
            let mut kernel_c = self.device().zeros_impl(kernel_l.shape(), kernel.dtype())?;
            kernel.copy_strided_src(&mut kernel_c, 0, kernel_l)?;
            let kernel_l = Layout::contiguous_with_offset((1, n, k), kernel_l.start_offset())
                .transpose(1, 2)?
                .broadcast_as((b, k, n))?;
            col.matmul(kernel, (b, m, n, k), &col_l, &kernel_l)?
        };
        let res_l = Layout::contiguous((b, l_out, n)).transpose(1, 2)?;
        let mut res_t = self.device().zeros_impl(res_l.shape(), res.dtype())?;
        res.copy_strided_src(&mut res_t, 0, &res_l)?;
        Ok(res_t)
    }

    fn conv_transpose1d(
        &self,
        layout: &Layout,
        k: &Self,
        k_layout: &Layout,
        params: &ParamsConvTranspose1D,
    ) -> Result<Self> {
        const USE_COL2IM_CONV1D_TR: bool = true;

        let can_use_col2im = k_layout.is_contiguous()
            && params.dilation == 1
            && params.padding == 0
            && params.output_padding == 0;
        let l_out = params.l_out();
        let dst_el = params.c_out * l_out * params.b_size;

        let buffer = if USE_COL2IM_CONV1D_TR && can_use_col2im {
            let (b_size, c_in, l_in) = layout.shape().dims3()?;
            let (c_in2, c_out, k_size) = k_layout.shape().dims3()?;
            if c_in != c_in2 {
                crate::bail!(
                    "convtr1d: shape mismatch on c_in {:?} {:?}",
                    layout.shape(),
                    k_layout.shape()
                )
            }
            let buffer = self
                .device
                .new_buffer(dst_el, self.dtype, "conv_transpose1d")?;

            let name = match self.dtype {
                DType::F32 => "col2im1d_f32",
                DType::F16 => "col2im1d_f16",
                DType::BF16 => "col2im1d_bf16",
                DType::U32 => "col2im1d_u32",
                DType::U8 => "col2im1d_u8",
                dtype => crate::bail!("metal col2im1d {dtype:?} not implemented"),
            };
            let col = {
                // This merges the last two dimensions of the kernel together.
                let kernel_l_mm = Layout::new(
                    (b_size, c_in, k_size * c_out).into(),
                    vec![0, k_size * c_out, 1],
                    k_layout.start_offset(),
                );
                self.matmul(
                    k,
                    (b_size, l_in, c_out * k_size, c_in),
                    &layout.transpose(1, 2)?,
                    &kernel_l_mm,
                )?
            };
            // It is important for the command encoder to be obtained *after* the matmul
            // kernel has run, otherwise we might use a command-buffer that has been committed
            // already resulting in the following error.
            // _status < MTLCommandBufferStatusCommitted >
            // -[IOGPUMetalCommandBuffer setCurrentCommandEncoder:]
            let encoder = self.device.command_encoder()?;
            encoder.set_label("col2im1d");
            candle_metal_kernels::call_col2im1d(
                &self.device.device,
                &encoder,
                &self.device.kernels,
                name,
                &[b_size, l_in, c_out, k_size],
                params.k_size,
                params.stride,
                BufferOffset::zero_offset(&col.buffer),
                &buffer,
            )
            .map_err(MetalError::from)?;
            buffer
        } else {
            let buffer = self
                .device
                .new_buffer(dst_el, self.dtype, "conv_transpose1d")?;

            let encoder = self.device.command_encoder()?;
            encoder.set_label("conv_transpose1d");
            let name = match self.dtype {
                DType::F32 => "conv_transpose1d_f32",
                DType::F16 => "conv_transpose1d_f16",
                DType::BF16 => "conv_transpose1d_bf16",
                DType::U32 => "conv_transpose1d_u32",
                DType::U8 => "conv_transpose1d_u8",
                dtype => crate::bail!("Metal conv_transpose1d {dtype:?} not implemented"),
            };
            candle_metal_kernels::call_conv_transpose1d(
                &self.device.device,
                &encoder,
                &self.device.kernels,
                name,
                params.dilation,
                params.stride,
                params.padding,
                params.output_padding,
                params.c_out,
                l_out,
                params.b_size,
                layout.dims(),
                layout.stride(),
                k_layout.dims(),
                k_layout.stride(),
                &self.buffer,
                layout.start_offset() * self.dtype.size_in_bytes(),
                &k.buffer,
                k_layout.start_offset() * k.dtype.size_in_bytes(),
                &buffer,
            )
            .map_err(MetalError::from)?;
            buffer
        };
        Ok(Self::new(buffer, self.device.clone(), dst_el, self.dtype))
    }

    fn conv2d(
        &self,
        layout: &Layout,
        kernel: &Self,
        kernel_l: &Layout,
        params: &ParamsConv2D,
    ) -> Result<Self> {
        let device = self.device().clone();
        let shape = layout.shape();
        let dims = shape.dims();

        let stride = params.stride;
        let dilation = params.dilation;
        let padding = params.padding;
        let h_k = params.k_h;
        let w_k = params.k_w;
        let h = dims[2];
        let w = dims[3];
        let h_out = (h + 2 * padding - dilation * (h_k - 1) - 1) / stride + 1;
        let w_out = (w + 2 * padding - dilation * (w_k - 1) - 1) / stride + 1;
        let dst_el = dims[0] * h_out * w_out * dims[1] * h_k * w_k;

        let dst = self
            .device
            .new_buffer(dst_el, self.dtype, "conv2d_im2col")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("conv2d_im2col");
        let name = match self.dtype {
            DType::F32 => "im2col_f32",
            DType::F16 => "im2col_f16",
            DType::BF16 => "im2col_bf16",
            DType::U8 => "im2col_u8",
            DType::U32 => "im2col_u32",
            dtype => crate::bail!("Metal conv2d {dtype:?} not implemented"),
        };
        let src = buffer_o(&self.buffer, layout, self.dtype);
        candle_metal_kernels::call_im2col_strided(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            name,
            layout.shape().dims(),
            layout.stride(),
            (h_k, w_k, stride, padding, dilation),
            src,
            &dst,
        )
        .map_err(MetalError::from)?;
        drop(encoder);
        let col = Self {
            buffer: dst,
            device,
            count: dst_el,
            dtype: self.dtype,
        };
        let h_out = params.out_h();
        let w_out = params.out_w();
        let b = params.b_size;
        let n = params.c_out;
        let k = params.k_h * params.k_w * params.c_in;
        let m = h_out * w_out;
        let col_l = Layout::contiguous((b, m, k));
        let res = if kernel_l.is_contiguous() {
            let kernel_l = Layout::contiguous_with_offset((1, n, k), kernel_l.start_offset())
                .transpose(1, 2)?
                .broadcast_as((b, k, n))?;
            col.matmul(kernel, (b, m, n, k), &col_l, &kernel_l)?
        } else {
            // Make the kernel contiguous if not already the case.
            let mut kernel_c = self.device().zeros_impl(kernel_l.shape(), kernel.dtype())?;
            kernel.copy_strided_src(&mut kernel_c, 0, kernel_l)?;
            let kernel_l = Layout::contiguous_with_offset((1, n, k), kernel_l.start_offset())
                .transpose(1, 2)?
                .broadcast_as((b, k, n))?;
            col.matmul(kernel, (b, m, n, k), &col_l, &kernel_l)?
        };
        let res_l = Layout::contiguous((b, h_out, w_out, n))
            .transpose(1, 2)?
            .transpose(1, 3)?;
        let mut res_t = self.device().zeros_impl(res_l.shape(), res.dtype())?;
        res.copy_strided_src(&mut res_t, 0, &res_l)?;
        Ok(res_t)
    }

    fn conv_transpose2d(
        &self,
        l: &Layout,
        kernel: &Self,
        kernel_l: &Layout,
        params: &ParamsConvTranspose2D,
    ) -> Result<Self> {
        // Kernel shape: (c_in_k, c_out, h_k, w_k)
        // Input shape: (b_size, c_in, h_in, w_in)
        let (out_w, out_h) = (params.out_w(), params.out_h());
        let dst_el = params.c_out * out_w * out_h * params.b_size;

        let dims = l.dims();
        if dims.len() != 4 {
            crate::bail!("unexpected input shape for conv_transpose2d {dims:?}, expected 4")
        }

        let k_dims = kernel_l.dims();
        if k_dims.len() != 4 {
            crate::bail!("unexpected kernel shape for conv_transpose2d {k_dims:?}, expected 4")
        }

        let buffer = self
            .device
            .new_buffer(dst_el, self.dtype, "conv_transpose2d")?;

        let encoder = self.device.command_encoder()?;
        encoder.set_label("conv_transpose2d");

        let name = match self.dtype {
            DType::F32 => "conv_transpose2d_f32",
            DType::F16 => "conv_transpose2d_f16",
            DType::BF16 => "conv_transpose2d_bf16",
            dtype => crate::bail!("Metal conv_transpose2d {dtype:?} not implemented"),
        };

        candle_metal_kernels::call_conv_transpose2d(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            name,
            CallConvTranspose2dCfg {
                dilation: params.dilation,
                stride: params.stride,
                padding: params.padding,
                output_padding: params.output_padding,
                c_out: params.c_out,
                out_h,
                out_w,
                b_size: params.b_size,
                input_dims: l.dims(),
                input_stride: l.stride(),
                kernel_dims: kernel_l.dims(),
                kernel_stride: kernel_l.stride(),
                input_offset: l.start_offset() * self.dtype.size_in_bytes(),
                kernel_offset: kernel_l.start_offset() * kernel.dtype.size_in_bytes(),
            },
            &self.buffer,
            &kernel.buffer,
            &buffer,
        )
        .map_err(MetalError::from)?;
        Ok(Self::new(buffer, self.device.clone(), dst_el, self.dtype))
    }

    fn avg_pool2d(
        &self,
        inp_l: &Layout,
        (w_k, h_k): (usize, usize),
        (w_stride, h_stride): (usize, usize),
    ) -> Result<Self> {
        let shape = inp_l.shape();
        let (b_size, channels, width, height) = shape.dims4()?;
        let strides = inp_l.stride();
        let name = match self.dtype {
            DType::F32 => "avg_pool2d_f32",
            DType::F16 => "avg_pool2d_f16",
            DType::BF16 => "avg_pool2d_bf16",
            DType::U8 => "avg_pool2d_u8",
            DType::U32 => "avg_pool2d_u32",
            dtype => crate::bail!("Metal avg_pool2d {dtype:?} not implemented"),
        };
        let out_w = (width - w_k) / w_stride + 1;
        let out_h = (height - h_k) / h_stride + 1;
        let dst_el = out_w * out_h * b_size * channels;
        let buffer = self.device.new_buffer(dst_el, self.dtype, "avg_pool2d")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("avg_pool2d");
        candle_metal_kernels::call_pool2d(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            name,
            inp_l.dims(),
            strides,
            out_w,
            out_h,
            w_k,
            h_k,
            w_stride,
            h_stride,
            &self.buffer,
            &buffer,
        )
        .map_err(MetalError::from)?;
        Ok(Self::new(buffer, self.device.clone(), dst_el, self.dtype))
    }

    fn max_pool2d(
        &self,
        inp_l: &Layout,
        (w_k, h_k): (usize, usize),
        (w_stride, h_stride): (usize, usize),
    ) -> Result<Self> {
        let shape = inp_l.shape();
        let (b_size, channels, width, height) = shape.dims4()?;
        let strides = inp_l.stride();
        let name = match self.dtype {
            DType::F32 => "max_pool2d_f32",
            DType::F16 => "max_pool2d_f16",
            DType::BF16 => "max_pool2d_bf16",
            DType::U8 => "max_pool2d_u8",
            DType::U32 => "max_pool2d_u32",
            dtype => crate::bail!("Metal max_pool2d {dtype:?} not implemented"),
        };
        let out_w = (width - w_k) / w_stride + 1;
        let out_h = (height - h_k) / h_stride + 1;
        let dst_el = out_w * out_h * b_size * channels;
        let buffer = self.device.new_buffer(dst_el, self.dtype, "max_pool2d")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("max_pool2d");
        candle_metal_kernels::call_pool2d(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            name,
            inp_l.dims(),
            strides,
            out_w,
            out_h,
            w_k,
            h_k,
            w_stride,
            h_stride,
            &self.buffer,
            &buffer,
        )
        .map_err(MetalError::from)?;
        Ok(Self::new(buffer, self.device.clone(), dst_el, self.dtype))
    }

    fn upsample_nearest1d(&self, _: &Layout, _: usize) -> Result<Self> {
        crate::bail!("Metal upsample_nearest1d not implemented")
    }

    fn upsample_nearest2d(&self, inp_l: &Layout, out_w: usize, out_h: usize) -> Result<Self> {
        // let inp = &inp.slice(inp_l.start_offset()..);
        let shape = inp_l.shape();
        let dims = shape.dims();
        let strides = inp_l.stride();
        if dims.len() != 4 {
            crate::bail!("unexpected input shape for upsample {dims:?}")
        }
        let name = match self.dtype {
            DType::F32 => "upsample_nearest2d_f32",
            DType::F16 => "upsample_nearest2d_f16",
            DType::BF16 => "upsample_nearest2d_bf16",
            DType::U8 => "upsample_nearest2d_u8",
            DType::U32 => "upsample_nearest2d_u32",
            dtype => crate::bail!("Metal upsample_nearest2d {dtype:?} not implemented"),
        };

        let dst_el = out_w * out_h * dims[0] * dims[1];
        let buffer = self
            .device
            .new_buffer(dst_el, self.dtype, "upsample_nearest2d")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("upsample_nearest2d");
        let src = buffer_o(&self.buffer, inp_l, self.dtype);
        candle_metal_kernels::call_upsample_nearest_2d(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            name,
            dims,
            strides,
            out_w,
            out_h,
            src,
            &buffer,
        )
        .map_err(MetalError::from)?;
        Ok(Self::new(buffer, self.device.clone(), dst_el, self.dtype))
    }

    fn upsample_bilinear2d(
        &self,
        inp_l: &Layout,
        out_h: usize,
        out_w: usize,
        align_corners: bool,
        scale_h: Option<f64>,
        scale_w: Option<f64>,
    ) -> Result<Self> {
        let shape = inp_l.shape();
        let dims = shape.dims();
        let strides = inp_l.stride();

        if dims.len() != 4 {
            crate::bail!("unexpected input shape for upsample_bilinear2d {dims:?}")
        }

        let name = match self.dtype {
            DType::F32 => "upsample_bilinear2d_f32",
            DType::F16 => "upsample_bilinear2d_f16",
            DType::BF16 => "upsample_bilinear2d_bf16",
            DType::U8 => "upsample_bilinear2d_u8",
            DType::U32 => "upsample_bilinear2d_u32",
            dtype => crate::bail!("Metal upsample_bilinear2d {dtype:?} not implemented"),
        };

        let dst_el = out_w * out_h * dims[0] * dims[1];
        let buffer = self
            .device
            .new_buffer(dst_el, self.dtype, "upsample_bilinear2d")?;

        let encoder = self.device.command_encoder()?;
        encoder.set_label("upsample_bilinear2d");

        let src = buffer_o(&self.buffer, inp_l, self.dtype);
        candle_metal_kernels::call_upsample_bilinear_2d(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            name,
            dims,
            strides,
            out_w,
            out_h,
            align_corners,
            scale_h,
            scale_w,
            src,
            &buffer,
        )
        .map_err(MetalError::from)?;

        Ok(Self::new(buffer, self.device.clone(), dst_el, self.dtype))
    }

    fn gather(&self, src_l: &Layout, ids: &Self, ids_l: &Layout, dim: usize) -> Result<Self> {
        if !ids_l.is_contiguous() {
            return Err(crate::Error::RequiresContiguous { op: "gather" }.bt());
        };
        let ids_el = ids_l.dims()[dim];
        let dst_el = ids_l.shape().elem_count();
        let dtype = self.dtype;
        let device = self.device();
        let buffer = device.new_buffer(dst_el, dtype, "gather")?;
        let name = match (ids.dtype, self.dtype) {
            (DType::U8, DType::U8) => "gather_u8_u8",
            (DType::U8, DType::F32) => "gather_u8_f32",
            (DType::U8, DType::F16) => "gather_u8_f16",
            (DType::U8, DType::BF16) => "gather_u8_bf16",
            (DType::U8, DType::U32) => "gather_u8_u32",
            (DType::U8, DType::I64) => "gather_u8_i64",
            (DType::U32, DType::F32) => "gather_u32_f32",
            (DType::U32, DType::F16) => "gather_u32_f16",
            (DType::U32, DType::BF16) => "gather_u32_bf16",
            (DType::U32, DType::U32) => "gather_u32_u32",
            (DType::U32, DType::I64) => "gather_u32_i64",
            (DType::I64, DType::F32) => "gather_i64_f32",
            (DType::I64, DType::F16) => "gather_i64_f16",
            (DType::I64, DType::BF16) => "gather_i64_bf16",
            (DType::I64, DType::U32) => "gather_i64_u32",
            (DType::I64, DType::I64) => "gather_i64_i64",
            (left, right) => crate::bail!("Metal gather {left:?} {right:?} not implemented"),
        };
        let encoder = self.device.command_encoder()?;
        encoder.set_label("gather");
        let src = buffer_o(&self.buffer, src_l, dtype);
        let ids = buffer_o(&ids.buffer, ids_l, ids.dtype);
        candle_metal_kernels::call_gather(
            &device.device,
            &encoder,
            &self.device.kernels,
            name,
            src_l.dims(),
            ids_el,
            dim,
            src,
            ids,
            &buffer,
        )
        .map_err(MetalError::from)?;
        Ok(Self::new(buffer, device.clone(), dst_el, dtype))
    }

    fn scatter_set(
        &mut self,
        l: &Layout,
        ids: &Self,
        ids_l: &Layout,
        src: &Self,
        src_l: &Layout,
        dim: usize,
    ) -> Result<()> {
        if !l.is_contiguous() || !ids_l.is_contiguous() || !src_l.is_contiguous() {
            return Err(crate::Error::RequiresContiguous { op: "scatter" }.bt());
        };
        let name = match (ids.dtype, self.dtype) {
            (DType::U8, DType::F32) => "s_u8_f32",
            (DType::U8, DType::F16) => "s_u8_f16",
            (DType::U8, DType::BF16) => "s_u8_bf16",
            (DType::U32, DType::U32) => "s_u32_u32",
            (DType::U32, DType::F32) => "s_u32_f32",
            (DType::U32, DType::F16) => "s_u32_f16",
            (DType::U32, DType::BF16) => "s_u32_bf16",
            (DType::I64, DType::F32) => "s_i64_f32",
            (DType::I64, DType::F16) => "s_i64_f16",
            (DType::I64, DType::BF16) => "s_i64_bf16",
            _ => Err(MetalError::UnexpectedDType {
                msg: "scatter ids should be u8/u32/i64",
                expected: DType::U32,
                got: ids.dtype(),
            })?,
        };
        let encoder = self.device.command_encoder()?;
        encoder.set_label("scatter");
        let dst = buffer_o(&self.buffer, l, self.dtype);
        let src = buffer_o(&src.buffer, src_l, src.dtype);
        let ids = buffer_o(&ids.buffer, ids_l, ids.dtype);
        candle_metal_kernels::call_scatter(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            name,
            src_l.dims(),
            l.dims(),
            dim,
            src,
            ids,
            dst,
        )
        .map_err(MetalError::from)?;
        Ok(())
    }

    fn scatter_add_set(
        &mut self,
        l: &Layout,
        ids: &Self,
        ids_l: &Layout,
        src: &Self,
        src_l: &Layout,
        dim: usize,
    ) -> Result<()> {
        if !l.is_contiguous() || !ids_l.is_contiguous() || !src_l.is_contiguous() {
            return Err(crate::Error::RequiresContiguous { op: "scatter-add" }.bt());
        };
        let name = match (ids.dtype, self.dtype) {
            (DType::U8, DType::F32) => "sa_u8_f32",
            (DType::U8, DType::F16) => "sa_u8_f16",
            (DType::U8, DType::BF16) => "sa_u8_bf16",
            (DType::U32, DType::U32) => "sa_u32_u32",
            (DType::U32, DType::F32) => "sa_u32_f32",
            (DType::U32, DType::F16) => "sa_u32_f16",
            (DType::U32, DType::BF16) => "sa_u32_bf16",
            (DType::I64, DType::F32) => "sa_i64_f32",
            (DType::I64, DType::F16) => "sa_i64_f16",
            (DType::I64, DType::BF16) => "sa_i64_bf16",
            _ => Err(MetalError::UnexpectedDType {
                msg: "scatter-add ids should be u8/u32/i64",
                expected: DType::U32,
                got: ids.dtype(),
            })?,
        };
        let encoder = self.device.command_encoder()?;
        encoder.set_label("scatter_add");
        let dst = buffer_o(&self.buffer, l, self.dtype);
        let src = buffer_o(&src.buffer, src_l, src.dtype);
        let ids = buffer_o(&ids.buffer, ids_l, ids.dtype);
        candle_metal_kernels::call_scatter(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            name,
            src_l.dims(),
            l.dims(),
            dim,
            src,
            ids,
            dst,
        )
        .map_err(MetalError::from)?;
        Ok(())
    }

    fn index_select(&self, ids: &Self, src_l: &Layout, ids_l: &Layout, dim: usize) -> Result<Self> {
        if !ids_l.is_contiguous() {
            crate::bail!("Metal index_select requires contiguous ids")
        }
        let left_size: usize = src_l.dims()[..dim].iter().product();
        let right_size: usize = src_l.dims()[dim + 1..].iter().product();
        let ids_el = ids_l.shape().elem_count();
        let dst_el = ids_el * left_size * right_size;
        let dtype = self.dtype;
        let device = self.device();
        let buffer = device.new_buffer(dst_el, dtype, "index_select")?;
        let name = match (ids.dtype, self.dtype) {
            (DType::U8, DType::U8) => "is_u8_u8",
            (DType::U8, DType::U32) => "is_u8_u32",
            (DType::U8, DType::I64) => "is_u8_i64",
            (DType::U8, DType::BF16) => "is_u8_bf16",
            (DType::U8, DType::F32) => "is_u8_f32",
            (DType::U8, DType::F16) => "is_u8_f16",

            (DType::U32, DType::U8) => "is_u32_u8",
            (DType::U32, DType::U32) => "is_u32_u32",
            (DType::U32, DType::I64) => "is_u32_i64",
            (DType::U32, DType::F32) => "is_u32_f32",
            (DType::U32, DType::F16) => "is_u32_f16",
            (DType::U32, DType::BF16) => "is_u32_bf16",

            (DType::I64, DType::U8) => "is_i64_u8",
            (DType::I64, DType::U32) => "is_i64_u32",
            (DType::I64, DType::I64) => "is_i64_i64",
            (DType::I64, DType::F32) => "is_i64_f32",
            (DType::I64, DType::F16) => "is_i64_f16",
            (DType::I64, DType::BF16) => "is_i64_bf16",

            (left, right) => {
                crate::bail!("Metal contiguous index_select {left:?} {right:?} not implemented")
            }
        };
        let encoder = self.device.command_encoder()?;
        let src = buffer_o(&self.buffer, src_l, dtype);
        let ids = buffer_o(&ids.buffer, ids_l, ids.dtype);
        candle_metal_kernels::call_index_select(
            &device.device,
            &encoder,
            &self.device.kernels,
            name,
            src_l.dims(),
            ids_el,
            dim,
            src_l.is_contiguous(),
            src_l.dims(),
            src_l.stride(),
            src,
            ids,
            &buffer,
        )
        .map_err(MetalError::from)?;
        Ok(Self::new(buffer, device.clone(), dst_el, dtype))
    }

    fn index_add(
        &self,
        l: &Layout,
        ids: &Self,
        ids_l: &Layout,
        src: &Self,
        src_l: &Layout,
        dim: usize,
    ) -> Result<Self> {
        let mut acc = self.device.zeros_impl(l.shape(), self.dtype())?;
        self.copy_strided_src(&mut acc, 0, l)?;
        if !ids_l.is_contiguous() || !src_l.is_contiguous() {
            return Err(crate::Error::RequiresContiguous { op: "index-add" }.bt());
        };
        let name = match (ids.dtype, self.dtype) {
            (DType::I64, DType::BF16) => "ia_i64_bf16",
            (DType::I64, DType::F16) => "ia_i64_f16",
            (DType::I64, DType::F32) => "ia_i64_f32",
            (DType::I64, DType::I64) => "ia_i64_i64",
            (DType::I64, DType::U32) => "ia_i64_u32",
            (DType::I64, DType::U8) => "ia_i64_u8",

            (DType::U32, DType::BF16) => "ia_u32_bf16",
            (DType::U32, DType::F16) => "ia_u32_f16",
            (DType::U32, DType::F32) => "ia_u32_f32",
            (DType::U32, DType::I64) => "ia_u32_i64",
            (DType::U32, DType::U32) => "ia_u32_u32",
            (DType::U32, DType::U8) => "ia_u32_u8",

            (DType::U8, DType::BF16) => "ia_u8_bf16",
            (DType::U8, DType::F16) => "ia_u8_f16",
            (DType::U8, DType::F32) => "ia_u8_f32",
            (DType::U8, DType::I64) => "ia_u8_i64",
            (DType::U8, DType::U32) => "ia_u8_u32",
            (DType::U8, DType::U8) => "ia_u8_u8",

            _ => Err(MetalError::UnexpectedDType {
                msg: "index-add ids should be u8/u32/i64",
                expected: DType::U32,
                got: ids.dtype(),
            })?,
        };
        let encoder = self.device.command_encoder()?;
        encoder.set_label("index_add");
        let src = buffer_o(&src.buffer, src_l, src.dtype);
        let ids = buffer_o(&ids.buffer, ids_l, ids.dtype);
        candle_metal_kernels::call_index_add(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            name,
            src_l.dims(),
            l.dims(),
            ids_l.dims(),
            dim,
            src,
            ids,
            &acc.buffer,
        )
        .map_err(MetalError::from)?;
        Ok(acc)
    }

    fn matmul(
        &self,
        rhs: &Self,
        (b, m, n, k): (usize, usize, usize, usize),
        lhs_l: &Layout,
        rhs_l: &Layout,
    ) -> Result<Self> {
        let buffer = self.device.new_buffer(b * m * n, self.dtype, "matmul")?;
        let encoder = self.device.command_encoder()?;
        encoder.set_label("matmul");
        let dtype = match self.dtype {
            DType::F32 => candle_metal_kernels::GemmDType::F32,
            DType::F16 => candle_metal_kernels::GemmDType::F16,
            DType::BF16 => candle_metal_kernels::GemmDType::BF16,
            dtype => {
                return Err(
                    MetalError::Message(format!("mlx matmul doesn't support {dtype:?}")).into(),
                )
            }
        };
        candle_metal_kernels::call_mlx_gemm(
            &self.device.device,
            &encoder,
            &self.device.kernels,
            dtype,
            (b, m, n, k),
            lhs_l.stride(),
            lhs_l.start_offset() * self.dtype.size_in_bytes(),
            &self.buffer,
            rhs_l.stride(),
            rhs_l.start_offset() * rhs.dtype.size_in_bytes(),
            &rhs.buffer,
            &buffer,
        )
        .map_err(MetalError::from)?;

        Ok(Self::new(
            buffer,
            self.device.clone(),
            b * m * n,
            self.dtype(),
        ))
    }

    fn copy2d(
        &self,
        dst: &mut Self,
        d1: usize,
        d2: usize,
        src_s: usize,
        dst_s: usize,
        src_o: usize,
        dst_o: usize,
    ) -> Result<()> {
        if self.dtype() != dst.dtype() {
            crate::bail!(
                "copy2d with inconsistent dtypes {:?} {:?}",
                self.dtype(),
                dst.dtype()
            )
        }
        if src_s == d2 && dst_s == d2 {
            let blit = self.device.blit_command_encoder()?;
            blit.set_label("copy2d_contiguous");
            let src_offset = src_o * self.dtype.size_in_bytes();
            let length = d1 * d2 * self.dtype.size_in_bytes();
            let dst_offset = dst_o * dst.dtype().size_in_bytes();
            blit.copy_from_buffer(&self.buffer, src_offset, dst.buffer(), dst_offset, length);
            blit.end_encoding();
        } else {
            let el_count = d1 * d2;
            if el_count == 0 {
                return Ok(());
            }
            let kernel_name = match self.dtype {
                DType::F32 => candle_metal_kernels::copy2d::FLOAT,
                DType::F16 => candle_metal_kernels::copy2d::HALF,
                DType::BF16 => candle_metal_kernels::copy2d::BFLOAT,
                DType::I64 => candle_metal_kernels::copy2d::I64,
                DType::U32 => candle_metal_kernels::copy2d::U32,
                DType::U8 => candle_metal_kernels::copy2d::U8,
                dtype => crate::bail!("Metal copy2d {dtype:?} not implemented"),
            };
            let encoder = self.device.command_encoder()?;
            encoder.set_label("copy2d");
            candle_metal_kernels::call_copy2d(
                &self.device.device,
                &encoder,
                &self.device.kernels,
                kernel_name,
                &self.buffer,
                &dst.buffer,
                d1,
                d2,
                src_s,
                dst_s,
                src_o * self.dtype.size_in_bytes(),
                dst_o * self.dtype.size_in_bytes(),
            )
            .map_err(MetalError::from)?;
        }
        Ok(())
    }

    fn copy_strided_src(&self, dst: &mut Self, dst_offset: usize, src_l: &Layout) -> Result<()> {
        if src_l.is_contiguous() && self.dtype == dst.dtype() {
            let blit = self.device.blit_command_encoder()?;
            blit.set_label("copy_contiguous");
            let src_offset = src_l.start_offset() * self.dtype.size_in_bytes();
            let length = src_l.shape().elem_count() * self.dtype.size_in_bytes();
            let dst_offset = dst_offset * dst.dtype().size_in_bytes();
            blit.copy_from_buffer(&self.buffer, src_offset, dst.buffer(), dst_offset, length);
            blit.end_encoding();
        } else {
            let src_shape = src_l.shape();
            let el_count = src_shape.elem_count();
            if el_count == 0 {
                return Ok(());
            }
            let kernel_name = match self.dtype {
                DType::F32 => candle_metal_kernels::unary::strided::copy::FLOAT,
                DType::F16 => candle_metal_kernels::unary::strided::copy::HALF,
                DType::BF16 => candle_metal_kernels::unary::strided::copy::BFLOAT,
                DType::I64 => candle_metal_kernels::unary::strided::copy::I64,
                DType::U32 => candle_metal_kernels::unary::strided::copy::U32,
                DType::U8 => candle_metal_kernels::unary::strided::copy::U8,
                dtype => crate::bail!("Metal copy_strided {dtype:?} not implemented"),
            };
            let src = buffer_o(&self.buffer, src_l, self.dtype);
            let dst = BufferOffset {
                buffer: &dst.buffer,
                offset_in_bytes: dst_offset * dst.dtype.size_in_bytes(),
            };
            let encoder = self.device.command_encoder()?;
            encoder.set_label("copy_strided");
            candle_metal_kernels::call_unary_strided(
                &self.device.device,
                &encoder,
                &self.device.kernels,
                kernel_name,
                src_l.dims(),
                src,
                src_l.stride(),
                dst,
            )
            .map_err(MetalError::from)?;
        }
        Ok(())
    }
}

impl MetalStorage {
    pub fn new(buffer: Arc<Buffer>, device: MetalDevice, count: usize, dtype: DType) -> Self {
        Self {
            buffer,
            device,
            count,
            dtype,
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn binary(
        &self,
        op: &'static str,
        rhs: &Self,
        lhs_l: &Layout,
        rhs_l: &Layout,
    ) -> Result<Self> {
        fn kernel_name(op: &'static str, dtype: &DType, suffix: &str) -> String {
            format!("{op}_{}{}", dtype.as_str(), suffix)
        }
        let device = self.device();
        let shape = lhs_l.shape();
        let el_count = shape.elem_count();
        let encoder = device.command_encoder()?;
        let lhs = buffer_o(&self.buffer, lhs_l, self.dtype);
        let rhs = buffer_o(&rhs.buffer, rhs_l, rhs.dtype);

        let dtype = match op {
            "eq" | "ne" | "le" | "lt" | "ge" | "gt" => DType::U8,
            _ => self.dtype,
        };
        let lhs_contiguous = lhs_l.is_contiguous();
        let rhs_contiguous = rhs_l.is_contiguous();

        let buffer = if lhs_contiguous && rhs_contiguous {
            let kernel = kernel_name(op, &self.dtype, "");
            let buffer = device.new_buffer(el_count, dtype, op)?;
            candle_metal_kernels::call_binary_contiguous(
                &device.device,
                &encoder,
                &device.kernels,
                kernel,
                self.dtype.size_in_bytes(),
                el_count,
                lhs,
                rhs,
                &buffer,
            )
            .map_err(MetalError::from)?;
            buffer
        } else {
            let strided_suffix = if lhs_contiguous {
                "_rstrided"
            } else if rhs_contiguous {
                "_lstrided"
            } else {
                "_strided"
            };
            let kernel = kernel_name(op, &self.dtype, strided_suffix);
            let buffer = device.new_buffer(el_count, dtype, op)?;
            candle_metal_kernels::call_binary_strided(
                &device.device,
                &encoder,
                &device.kernels,
                kernel,
                self.dtype.size_in_bytes(),
                lhs_l.dims(),
                lhs,
                lhs_l.stride(),
                rhs,
                rhs_l.stride(),
                &buffer,
            )
            .map_err(MetalError::from)?;
            buffer
        };
        encoder.set_label("binary");
        Ok(Self::new(buffer, device.clone(), el_count, dtype))
    }

    pub(crate) fn to_cpu<T: Clone>(&self) -> Result<Vec<T>> {
        let size = self.count * self.dtype.size_in_bytes();
        let buffer = self.device.allocate_buffer(size)?;
        {
            let blit = self.device.blit_command_encoder()?;
            blit.set_label("blit_to_cpu");
            blit.copy_from_buffer(&self.buffer, 0, &buffer, 0, size);
            blit.end_encoding();
        }
        self.device.wait_until_completed()?;
        Ok(read_to_vec(&buffer, self.count))
    }
}

impl BackendDevice for MetalDevice {
    type Storage = MetalStorage;

    fn new(ordinal: usize) -> Result<Self> {
        let device = Device::all().swap_remove(ordinal);
        let command_queue = device.new_command_queue().map_err(MetalError::from)?;
        let kernels = Arc::new(Kernels::new());
        let seed = Arc::new(Mutex::new(
            device
                .new_buffer_with_data(
                    [299792458u64].as_ptr() as *const c_void,
                    4,
                    RESOURCE_OPTIONS,
                )
                .map_err(MetalError::from)?,
        ));
        let commands = Commands::new(command_queue).map_err(MetalError::from)?;
        Ok(Self {
            id: DeviceId::new(),
            device,
            commands: Arc::new(RwLock::new(commands)),
            buffers: Arc::new(RwLock::new(HashMap::new())),
            kernels,
            seed,
            seed_value: Arc::new(RwLock::new(299792458)),
        })
    }

    fn location(&self) -> crate::DeviceLocation {
        crate::DeviceLocation::Metal {
            gpu_id: self.registry_id() as usize,
        }
    }

    fn same_device(&self, rhs: &Self) -> bool {
        self.id == rhs.id
    }

    unsafe fn alloc_uninit(&self, shape: &Shape, dtype: DType) -> Result<MetalStorage> {
        let buffer = self.new_buffer(shape.elem_count(), dtype, "alloc-uninit")?;
        Ok(MetalStorage::new(
            buffer,
            self.clone(),
            shape.elem_count(),
            dtype,
        ))
    }

    fn zeros_impl(&self, shape: &Shape, dtype: DType) -> Result<MetalStorage> {
        let size = shape.elem_count() * dtype.size_in_bytes();
        let buffer = self.allocate_zeros(size)?;
        Ok(MetalStorage::new(
            buffer,
            self.clone(),
            shape.elem_count(),
            dtype,
        ))
    }

    fn storage_from_slice<T: crate::WithDType>(&self, s: &[T]) -> Result<Self::Storage> {
        let (count, buffer) = match T::cpu_storage_ref(s) {
            CpuStorageRef::U8(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorageRef::U32(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorageRef::I16(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorageRef::I32(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorageRef::I64(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorageRef::BF16(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorageRef::F16(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorageRef::F32(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorageRef::F64(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorageRef::F8E4M3(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorageRef::F6E2M3(_)
            | CpuStorageRef::F6E3M2(_)
            | CpuStorageRef::F4(_)
            | CpuStorageRef::F8E8M0(_) => {
                return Err(Error::UnsupportedDTypeForOp(T::DTYPE, "to_dtype").bt())
            }
        };
        Ok(Self::Storage::new(buffer?, self.clone(), count, T::DTYPE))
    }

    fn storage_from_cpu_storage(&self, storage: &CpuStorage) -> Result<Self::Storage> {
        let (count, buffer) = match storage {
            CpuStorage::U8(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorage::U32(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorage::I16(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorage::I32(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorage::I64(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorage::BF16(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorage::F16(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorage::F32(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorage::F64(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorage::F8E4M3(storage) => (storage.len(), self.new_buffer_with_data(storage)),
            CpuStorage::F6E2M3(_)
            | CpuStorage::F6E3M2(_)
            | CpuStorage::F4(_)
            | CpuStorage::F8E8M0(_) => {
                return Err(Error::UnsupportedDTypeForOp(storage.dtype(), "to_dtype").bt())
            }
        };
        Ok(Self::Storage::new(
            buffer?,
            self.clone(),
            count,
            storage.dtype(),
        ))
    }

    fn storage_from_cpu_storage_owned(&self, storage: CpuStorage) -> Result<Self::Storage> {
        self.storage_from_cpu_storage(&storage)
    }

    fn rand_uniform(
        &self,
        shape: &Shape,
        dtype: DType,
        min: f64,
        max: f64,
    ) -> Result<Self::Storage> {
        let name = match dtype {
            DType::F32 => "rand_uniform_f32",
            DType::F16 => "rand_uniform_f16",
            DType::BF16 => "rand_uniform_bf16",
            dtype => crate::bail!("rand_uniform not implemented for {dtype:?}"),
        };
        let buffer = self.new_buffer(shape.elem_count(), dtype, "rand_uniform")?;
        let encoder = self.command_encoder()?;
        encoder.set_label("rand_uniform");
        candle_metal_kernels::call_random_uniform(
            &self.device,
            &encoder,
            &self.kernels,
            name,
            min as f32,
            max as f32,
            shape.elem_count(),
            &self.seed.lock().unwrap(),
            &buffer,
        )
        .map_err(MetalError::from)?;

        Ok(Self::Storage::new(
            buffer,
            self.clone(),
            shape.elem_count(),
            dtype,
        ))
    }

    fn rand_normal(
        &self,
        shape: &Shape,
        dtype: DType,
        mean: f64,
        stddev: f64,
    ) -> Result<Self::Storage> {
        let name = match dtype {
            DType::F32 => "rand_normal_f32",
            DType::F16 => "rand_normal_f16",
            DType::BF16 => "rand_normal_bf16",
            dtype => crate::bail!("rand_uniform not implemented for {dtype:?}"),
        };
        let buffer = self.new_buffer(shape.elem_count(), dtype, "rand_normal")?;
        let encoder = self.command_encoder()?;
        encoder.set_label("rand_normal");
        candle_metal_kernels::call_random_normal(
            &self.device,
            &encoder,
            &self.kernels,
            name,
            mean as f32,
            stddev as f32,
            shape.elem_count(),
            &self.seed.lock().unwrap(),
            &buffer,
        )
        .map_err(MetalError::from)?;

        Ok(Self::Storage::new(
            buffer,
            self.clone(),
            shape.elem_count(),
            dtype,
        ))
    }

    fn set_seed(&self, seed: u64) -> Result<()> {
        *self.seed_value.write().unwrap() = seed;

        let seed_buffer = self.seed.try_lock().map_err(MetalError::from)?;
        let contents = seed_buffer.data();
        unsafe {
            std::ptr::copy_nonoverlapping([seed].as_ptr(), contents as *mut u64, 1);
        }
        seed_buffer.did_modify_range(NSRange::new(0, 8));

        Ok(())
    }

    fn get_current_seed(&self) -> Result<u64> {
        Ok(*self.seed_value.read().unwrap())
    }

    fn synchronize(&self) -> Result<()> {
        self.wait_until_completed()
    }
}

fn read_to_vec<T: Clone>(buffer: &Buffer, n: usize) -> Vec<T> {
    let ptr = buffer.contents() as *const T;
    assert!(!ptr.is_null());
    let slice = unsafe { std::slice::from_raw_parts(ptr, n) };
    slice.to_vec()
}
