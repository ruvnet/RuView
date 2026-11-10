// Variables are wrappers around tensors that can be modified, they are typically used for holding
// weights and being modified by gradient descent.
// We do not expose a public way to create variables as this would break the invariant that the
// tensor within a variable is actually with `is_variable` set to `true`.
use crate::{DType, Device, Error, Result, Shape, Tensor};

/// A variable is a wrapper around a tensor, however variables can have their content modified
/// whereas tensors are immutable.
#[derive(Clone, Debug)]
pub struct Var(Tensor);

impl std::fmt::Display for Var {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::ops::Deref for Var {
    type Target = Tensor;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl Var {
    pub fn zeros<S: Into<Shape>>(shape: S, dtype: DType, device: &Device) -> Result<Self> {
        let inner = Tensor::zeros_impl(shape, dtype, device, true)?;
        Ok(Self(inner))
    }

    pub fn ones<S: Into<Shape>>(shape: S, dtype: DType, device: &Device) -> Result<Self> {
        let inner = Tensor::ones_impl(shape, dtype, device, true)?;
        Ok(Self(inner))
    }

    // Convert a tensor to a variable, if the tensor is already a variable then it is returned as is.
    pub fn from_tensor(t: &Tensor) -> Result<Self> {
        if t.is_variable() {
            Ok(Self(t.clone()))
        } else {
            let inner = t.make_var()?;
            Ok(Self(inner))
        }
    }

    pub fn rand_f64<S: Into<Shape>>(
        lo: f64,
        up: f64,
        s: S,
        dtype: DType,
        device: &Device,
    ) -> Result<Self> {
        let inner = Tensor::rand_f64_impl(lo, up, s, dtype, device, true)?;
        Ok(Self(inner))
    }

    pub fn randn_f64<S: Into<Shape>>(
        mean: f64,
        std: f64,
        s: S,
        dtype: DType,
        device: &Device,
    ) -> Result<Self> {
        let inner = Tensor::randn_f64_impl(mean, std, s, dtype, device, true)?;
        Ok(Self(inner))
    }

    pub fn rand<S: Into<Shape>, T: crate::FloatDType>(
        lo: T,
        up: T,
        s: S,
        device: &Device,
    ) -> Result<Self> {
        let inner = Tensor::rand_impl(lo, up, s, device, true)?;
        Ok(Self(inner))
    }

    pub fn randn<S: Into<Shape>, T: crate::FloatDType>(
        mean: T,
        std: T,
        s: S,
        device: &Device,
    ) -> Result<Self> {
        let inner = Tensor::randn_impl(mean, std, s, device, true)?;
        Ok(Self(inner))
    }

    /// Creates a new tensor on the specified device using the content and shape of the input.
    /// This is similar to `new` but the resulting tensor is a variable.
    pub fn new<A: crate::device::NdArray>(array: A, device: &Device) -> Result<Self> {
        let shape = array.shape()?;
        let inner = Tensor::new_impl(array, shape, device, true)?;
        Ok(Self(inner))
    }

    pub fn from_vec<S: Into<Shape>, D: crate::WithDType>(
        data: Vec<D>,
        shape: S,
        device: &Device,
    ) -> Result<Self> {
        let inner = Tensor::from_vec_impl(data, shape, device, true)?;
        Ok(Self(inner))
    }

    pub fn from_slice<S: Into<Shape>, D: crate::WithDType>(
        array: &[D],
        shape: S,
        device: &Device,
    ) -> Result<Self> {
        let inner = Tensor::new_impl(array, shape.into(), device, true)?;
        Ok(Self(inner))
    }

    pub fn as_detached_tensor(&self) -> Tensor {
        self.0.detach()
    }

    pub fn as_tensor(&self) -> &Tensor {
        &self.0
    }

    /// Consumes this `Var` and return the underlying tensor.
    pub fn into_inner(self) -> Tensor {
        self.0
    }

    /// Sets the content of the inner tensor, this does not require a mutable reference as inner
    /// mutability is used.
    pub fn set(&self, src: &Tensor) -> Result<()> {
        if self.same_storage(src) {
            let msg = "cannot set a variable to a tensor that is derived from its value";
            Err(Error::CannotSetVar { msg }.bt())?
        }
        let (mut dst, layout) = self.storage_mut_and_layout();
        if !layout.is_contiguous() {
            let msg = "cannot set a non-contiguous variable";
            Err(Error::CannotSetVar { msg }.bt())?
        }
        let (src, src_l) = src.storage_and_layout();
        if layout.shape() != src_l.shape() {
            Err(Error::ShapeMismatchBinaryOp {
                lhs: layout.shape().clone(),
                rhs: src_l.shape().clone(),
                op: "set",
            }
            .bt())?
        }
        src.copy_strided_src(&mut dst, layout.start_offset(), src_l)?;
        Ok(())
    }
}
