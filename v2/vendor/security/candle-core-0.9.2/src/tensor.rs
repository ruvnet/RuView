//! Tensors are N-dimensional matrixes of elements using a single data type.
#![allow(clippy::redundant_closure_call)]
use crate::backend::{BackendDevice, BackendStorage};
use crate::op::{BackpropOp, BinaryOp, CmpOp, Op, ReduceOp, UnaryOp};
use crate::scalar::TensorOrScalar;
use crate::shape::{Dim, Dims, ShapeWithOneHole};
use crate::{bail, storage::Storage, DType, Device, Error, Layout, Result, Shape};
use std::sync::{Arc, RwLock};

/// Unique identifier for tensors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TensorId(usize);

impl TensorId {
    fn new() -> Self {
        // https://users.rust-lang.org/t/idiomatic-rust-way-to-generate-unique-id/33805
        use std::sync::atomic;
        static COUNTER: atomic::AtomicUsize = atomic::AtomicUsize::new(1);
        Self(COUNTER.fetch_add(1, atomic::Ordering::Relaxed))
    }
}

pub struct Tensor_ {
    id: TensorId,
    // As we provide inner mutability on the tensor content, the alternatives are:
    // - Using a mutex, this would have the highest cost when retrieving the storage but would
    //   prevent errors when concurrent access takes place. Mutex would also be subject to
    //   deadlocks for example using the current code if the same tensor is used twice by a single
    //   binary op.
    // - Using a refcell unsafe cell would have some intermediary cost, borrow checking would be
    //   verified dynamically, but the resulting tensors would not be send or sync.
    // - Using an unsafe cell would have the lowest cost but undefined behavior on concurrent
    //   accesses.
    // Ideally, we would use Arc<Storage> for tensors on which we don't plan on modifying the data
    // and Arc<Mutex<Storage>> for tensors where the data could be modified, e.g. variables but
    // that's tricky to encode in the current setup.
    storage: Arc<RwLock<Storage>>,
    layout: Layout,
    op: BackpropOp,
    is_variable: bool,
    dtype: DType,
    device: Device,
}

impl AsRef<Tensor> for Tensor {
    fn as_ref(&self) -> &Tensor {
        self
    }
}

// Tensors are refcounted so that cloning is cheap when building the op graph.
// Storages are also refcounted independently so that its possible to avoid
// copying the storage for operations that only modify the shape or stride.
#[derive(Clone)]
/// The core struct for manipulating tensors.
///
/// ```rust
/// use candle_core::{Tensor, DType, Device};
///
/// let a = Tensor::arange(0f32, 6f32, &Device::Cpu)?.reshape((2, 3))?;
/// let b = Tensor::arange(0f32, 12f32, &Device::Cpu)?.reshape((3, 4))?;
///
/// let c = a.matmul(&b)?;
/// # Ok::<(), candle_core::Error>(())
/// ```
///
/// Tensors are reference counted with [`Arc`] so cloning them is cheap.
pub struct Tensor(Arc<Tensor_>);

impl std::ops::Deref for Tensor {
    type Target = Tensor_;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

macro_rules! unary_op {
    ($fn_name:ident, $op_name:ident) => {
        pub fn $fn_name(&self) -> Result<Self> {
            let shape = self.shape();
            if shape.elem_count() == 0 {
                return Ok(self.clone());
            }
            let storage = self
                .storage()
                .unary_impl::<crate::op::$op_name>(self.layout())?;
            let op = BackpropOp::new1(self, |s| Op::Unary(s, UnaryOp::$op_name));
            Ok(from_storage(storage, shape.clone(), op, false))
        }
    };
}

macro_rules! binary_op {
    ($fn_name:ident, $op_name:ident) => {
        pub fn $fn_name(&self, rhs: &Self) -> Result<Self> {
            let shape = self.same_shape_binary_op(rhs, stringify!($fn_name))?;
            if shape.elem_count() == 0 {
                return Ok(self.clone());
            }
            let storage = self.storage().binary_impl::<crate::op::$op_name>(
                &*rhs.storage(),
                self.layout(),
                rhs.layout(),
            )?;
            let op = BackpropOp::new2(self, rhs, |t1, t2| Op::Binary(t1, t2, BinaryOp::$op_name));
            Ok(from_storage(storage, shape.clone(), op, false))
        }
    };
}

macro_rules! binary_op_scalar {
    ($fn_name:ident, $op_name:ident) => {
        pub fn $fn_name<T: TensorOrScalar>(&self, rhs: T) -> Result<Self> {
            let rhs = match rhs.to_tensor_scalar()? {
                crate::scalar::TensorScalar::Tensor(rhs) => rhs,
                crate::scalar::TensorScalar::Scalar(rhs) => rhs
                    .to_dtype(self.dtype())?
                    .to_device(self.device())?
                    .broadcast_as(self.shape())?,
            };
            let shape = self.same_shape_binary_op(&rhs, stringify!($fn_name))?;
            if self.elem_count() == 0 {
                return Ok(self.clone());
            }
            let storage = self.storage().binary_impl::<crate::op::$op_name>(
                &*rhs.storage(),
                self.layout(),
                rhs.layout(),
            )?;
            let op = BackpropOp::new2(self, &rhs, |t1, t2| Op::Binary(t1, t2, BinaryOp::$op_name));
            Ok(from_storage(storage, shape.clone(), op, false))
        }
    };
}

macro_rules! broadcast_binary_op {
    ($fn_name:ident, $inner_fn_name:ident) => {
        pub fn $fn_name(&self, rhs: &Self) -> Result<Self> {
            let lhs = self;
            let shape = lhs
                .shape()
                .broadcast_shape_binary_op(rhs.shape(), stringify!($fn_name))?;
            let l_broadcast = shape != *lhs.shape();
            let r_broadcast = shape != *rhs.shape();
            match (l_broadcast, r_broadcast) {
                (true, true) => lhs
                    .broadcast_as(&shape)?
                    .$inner_fn_name(&rhs.broadcast_as(&shape)?),
                (false, true) => lhs.$inner_fn_name(&rhs.broadcast_as(&shape)?),
                (true, false) => lhs.broadcast_as(&shape)?.$inner_fn_name(rhs),
                (false, false) => lhs.$inner_fn_name(rhs),
            }
        }
    };
}

/// Creates a fresh tensor structure based on a storage and a shape, this uses contiguous strides.
pub(crate) fn from_storage<S: Into<Shape>>(
    storage: Storage,
    shape: S,
    op: BackpropOp,
    is_variable: bool,
) -> Tensor {
    let dtype = storage.dtype();
    let device = storage.device();
    let tensor_ = Tensor_ {
        id: TensorId::new(),
        storage: Arc::new(RwLock::new(storage)),
        layout: Layout::contiguous(shape),
        op,
        is_variable,
        dtype,
        device,
    };
    Tensor(Arc::new(tensor_))
}

impl Tensor {
    pub(crate) fn ones_impl<S: Into<Shape>>(
        shape: S,
        dtype: DType,
        device: &Device,
        is_variable: bool,
    ) -> Result<Self> {
        let none = BackpropOp::none();
        let shape = shape.into();
        let mut storage = unsafe { device.alloc_uninit(&shape, dtype)? };
        let layout = Layout::contiguous(shape.clone());
        storage.const_set(crate::scalar::Scalar::one(dtype), &layout)?;
        Ok(from_storage(storage, shape, none, is_variable))
    }

    /// Creates a new tensor filled with ones.
    ///
    /// ```rust
    /// use candle_core::{Tensor, DType, Device};
    /// let a = Tensor::ones((2, 3), DType::F32, &Device::Cpu)?;
    /// let b = Tensor::from_slice(&[1.0f32, 1.0, 1.0, 1.0, 1.0, 1.0], (2, 3), &Device::Cpu)?;
    /// // a == b
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn ones<S: Into<Shape>>(shape: S, dtype: DType, device: &Device) -> Result<Self> {
        Self::ones_impl(shape, dtype, device, false)
    }

    pub fn const_set(&self, value: crate::scalar::Scalar) -> Result<()> {
        self.storage_mut().const_set(value, self.layout())
    }

    pub fn zero_set(&self) -> Result<()> {
        self.const_set(crate::scalar::Scalar::zero(self.dtype()))
    }

    pub fn one_set(&self) -> Result<()> {
        self.const_set(crate::scalar::Scalar::one(self.dtype()))
    }

    /// Creates a new tensor filled with ones with same shape, dtype, and device as the other tensor.
    ///
    /// ```rust
    /// use candle_core::{Tensor, DType, Device};
    /// let a = Tensor::zeros((2, 3), DType::F32, &Device::Cpu)?;
    /// let b = a.ones_like()?;
    /// // b == a + 1
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn ones_like(&self) -> Result<Self> {
        Tensor::ones(self.shape(), self.dtype(), self.device())
    }

    // Do not expose outside of the crate, the `is_variable=true` case should only be accessed from
    // the variable module.
    pub(crate) fn zeros_impl<S: Into<Shape>>(
        shape: S,
        dtype: DType,
        device: &Device,
        is_variable: bool,
    ) -> Result<Self> {
        let none = BackpropOp::none();
        let shape = shape.into();
        let storage = device.zeros(&shape, dtype)?;
        Ok(from_storage(storage, shape, none, is_variable))
    }

    /// Creates a new tensor filled with zeros.
    ///
    /// ```rust
    /// use candle_core::{Tensor, DType, Device};
    /// let a = Tensor::zeros((2, 3), DType::F32, &Device::Cpu)?;
    /// let b = Tensor::from_slice(&[0.0f32, 0.0, 0.0, 0.0, 0.0, 0.0], (2, 3), &Device::Cpu)?;
    /// // a == b
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn zeros<S: Into<Shape>>(shape: S, dtype: DType, device: &Device) -> Result<Self> {
        Self::zeros_impl(shape, dtype, device, false)
    }

    /// Creates a new tensor filled with zeros with same shape, dtype, and device as the other
    /// tensor.
    ///
    /// ```rust
    /// use candle_core::{Tensor, DType, Device};
    /// let a = Tensor::zeros((2, 3), DType::F32, &Device::Cpu)?;
    /// let b = a.zeros_like()?;
    /// // b is on CPU f32.
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn zeros_like(&self) -> Result<Self> {
        Tensor::zeros(self.shape(), self.dtype(), self.device())
    }

    // Do not expose outside of the crate, the `is_variable=true` case should only be accessed from
    // the variable module.
    pub(crate) unsafe fn empty_impl<S: Into<Shape>>(
        shape: S,
        dtype: DType,
        device: &Device,
        is_variable: bool,
    ) -> Result<Self> {
        let none = BackpropOp::none();
        let shape = shape.into();
        let storage = device.alloc_uninit(&shape, dtype)?;
        Ok(from_storage(storage, shape, none, is_variable))
    }

    /// Creates a new tensor filled with uninitialized memory.
    ///
    /// # Safety
    /// This returns uninitialized memory.
    ///
    /// ```rust
    /// use candle_core::{Tensor, DType, Device};
    /// let a = unsafe { Tensor::empty((2, 3), DType::F32, &Device::Cpu)? };
    /// // a == b
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub unsafe fn empty<S: Into<Shape>>(shape: S, dtype: DType, device: &Device) -> Result<Self> {
        Self::empty_impl(shape, dtype, device, false)
    }

    /// Creates a new tensor filled with uninitialized memory of the same shape, dtype, and device as the other
    /// tensor.
    ///
    /// # Safety
    /// This returns uninitialized memory.
    ///
    /// ```rust
    /// use candle_core::{Tensor, DType, Device};
    /// let a = Tensor::zeros((2, 3), DType::F32, &Device::Cpu)?;
    /// let b = unsafe { a.empty_like()? };
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub unsafe fn empty_like(&self) -> Result<Self> {
        Tensor::empty(self.shape(), self.dtype(), self.device())
    }

    pub(crate) fn rand_impl<S: Into<Shape>, T: crate::FloatDType>(
        lo: T,
        up: T,
        s: S,
        device: &Device,
        is_variable: bool,
    ) -> Result<Self> {
        let s = s.into();
        let storage = device.rand_uniform(lo, up, &s)?;
        let none = BackpropOp::none();
        Ok(from_storage(storage, s, none, is_variable))
    }

    pub(crate) fn rand_f64_impl<S: Into<Shape>>(
        lo: f64,
        up: f64,
        s: S,
        dtype: DType,
        device: &Device,
        is_variable: bool,
    ) -> Result<Self> {
        let s = s.into();
        let storage = device.rand_uniform_f64(lo, up, &s, dtype)?;
        let none = BackpropOp::none();
        Ok(from_storage(storage, s, none, is_variable))
    }

    /// Creates a new tensor initialized with values sampled uniformly between `lo` and `up`.
    pub fn rand<S: Into<Shape>, T: crate::FloatDType>(
        lo: T,
        up: T,
        s: S,
        device: &Device,
    ) -> Result<Self> {
        Self::rand_impl(lo, up, s, device, false)
    }

    pub fn rand_like(&self, lo: f64, up: f64) -> Result<Self> {
        Tensor::rand_f64_impl(lo, up, self.shape(), self.dtype(), self.device(), false)
    }

    pub(crate) fn randn_impl<S: Into<Shape>, T: crate::FloatDType>(
        mean: T,
        std: T,
        s: S,
        device: &Device,
        is_variable: bool,
    ) -> Result<Self> {
        let s = s.into();
        let storage = device.rand_normal(mean, std, &s)?;
        let none = BackpropOp::none();
        Ok(from_storage(storage, s, none, is_variable))
    }

    pub(crate) fn randn_f64_impl<S: Into<Shape>>(
        mean: f64,
        std: f64,
        s: S,
        dtype: DType,
        device: &Device,
        is_variable: bool,
    ) -> Result<Self> {
        let s = s.into();
        let storage = device.rand_normal_f64(mean, std, &s, dtype)?;
        let none = BackpropOp::none();
        Ok(from_storage(storage, s, none, is_variable))
    }

    pub fn randn_like(&self, mean: f64, stdev: f64) -> Result<Self> {
        Tensor::randn_f64_impl(
            mean,
            stdev,
            self.shape(),
            self.dtype(),
            self.device(),
            false,
        )
    }

    /// Creates a new tensor initialized with values sampled from a normal distribution with the
    /// specified `mean` and standard deviation `std`.
    pub fn randn<S: Into<Shape>, T: crate::FloatDType>(
        mean: T,
        std: T,
        s: S,
        device: &Device,
    ) -> Result<Self> {
        Self::randn_impl(mean, std, s, device, false)
    }

    pub(crate) fn new_impl<A: crate::device::NdArray>(
        array: A,
        shape: Shape,
        device: &Device,
        is_variable: bool,
    ) -> Result<Self> {
        let n: usize = shape.elem_count();
        let buffer_size: usize = array.shape()?.elem_count();
        if buffer_size != n {
            return Err(Error::ShapeMismatch { buffer_size, shape }.bt());
        }
        let storage = device.storage(array)?;
        let none = BackpropOp::none();
        Ok(from_storage(storage, shape, none, is_variable))
    }

    /// Creates a new tensor on the specified device using the content and shape of the input.
    pub fn new<A: crate::device::NdArray>(array: A, device: &Device) -> Result<Self> {
        let shape = array.shape()?;
        Self::new_impl(array, shape, device, false)
    }

    /// Returns a new tensor with all the elements having the same specified value.
    ///```rust
    /// use candle_core::{Tensor, Device};
    /// let a = Tensor::full(3.5, (2, 4), &Device::Cpu)?;
    ///
    /// assert_eq!(a.to_vec2::<f64>()?, &[
    ///     [3.5, 3.5, 3.5, 3.5],
    ///     [3.5, 3.5, 3.5, 3.5],
    /// ]);
    /// # Ok::<(), candle_core::Error>(())
    pub fn full<D: crate::WithDType, S: Into<Shape>>(
        value: D,
        shape: S,
        device: &Device,
    ) -> Result<Self> {
        let none = BackpropOp::none();
        let shape = shape.into();
        let mut storage = unsafe { device.alloc_uninit(&shape, D::DTYPE)? };
        let layout = Layout::contiguous(shape.clone());
        storage.const_set(value.to_scalar(), &layout)?;
        Ok(from_storage(storage, shape, none, false))
    }

    /// Creates a new 1D tensor from an iterator.
    ///```rust
    /// use candle_core::{Tensor, Device};
    /// let a = Tensor::from_iter( [1.0, 2.0, 3.0, 4.0].into_iter(), &Device::Cpu)?;
    ///
    /// assert_eq!(a.to_vec1::<f64>()?, &[1.0, 2.0, 3.0, 4.0]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn from_iter<D: crate::WithDType>(
        iter: impl IntoIterator<Item = D>,
        device: &Device,
    ) -> Result<Self> {
        let data = iter.into_iter().collect::<Vec<_>>();
        let len = data.len();
        Self::from_vec_impl(data, len, device, false)
    }

    /// Creates a new 1D tensor with values from the interval `[start, end)` taken with a common
    /// difference `1` from `start`.
    ///```rust
    /// use candle_core::{Tensor, Device};
    /// let a = Tensor::arange(2., 5., &Device::Cpu)?;
    ///
    /// assert_eq!(a.to_vec1::<f64>()?, &[2., 3., 4.]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn arange<D: crate::WithDType>(start: D, end: D, device: &Device) -> Result<Self> {
        Self::arange_step(start, end, D::one(), device)
    }

    /// Creates a new 1D tensor with values from the interval `[start, end)` taken with a common
    /// difference `step` from `start`.
    ///```rust
    /// use candle_core::{Tensor, Device};
    /// let a = Tensor::arange_step(2.0, 4.0, 0.5, &Device::Cpu)?;
    ///
    /// assert_eq!(a.to_vec1::<f64>()?, &[2.0, 2.5, 3.0, 3.5]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn arange_step<D: crate::WithDType>(
        start: D,
        end: D,
        step: D,
        device: &Device,
    ) -> Result<Self> {
        if D::is_zero(&step) {
            bail!("step cannot be zero")
        }
        let mut data = vec![];
        let mut current = start;
        if step >= D::zero() {
            while current < end {
                data.push(current);
                current += step;
            }
        } else {
            while current > end {
                data.push(current);
                current += step;
            }
        }
        let len = data.len();
        Self::from_vec_impl(data, len, device, false)
    }

    pub(crate) fn from_vec_impl<S: ShapeWithOneHole, D: crate::WithDType>(
        data: Vec<D>,
        shape: S,
        device: &Device,
        is_variable: bool,
    ) -> Result<Self> {
        let shape = shape.into_shape(data.len())?;
        let storage = device.storage_owned(data)?;
        let none = BackpropOp::none();
        Ok(from_storage(storage, shape, none, is_variable))
    }

    /// Creates a new tensor initialized with values from the input vector. The number of elements
    /// in this vector must be the same as the number of elements defined by the shape.
    /// If the device is cpu, no data copy is made.
    ///```rust
    /// use candle_core::{Tensor, Device};
    /// let a = Tensor::from_vec(vec!{1., 2., 3., 4., 5., 6.}, (2, 3), &Device::Cpu)?;
    ///
    /// assert_eq!(a.to_vec2::<f64>()?, &[
    ///     [1., 2., 3.],
    ///     [4., 5., 6.]
    /// ]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn from_vec<S: ShapeWithOneHole, D: crate::WithDType>(
        data: Vec<D>,
        shape: S,
        device: &Device,
    ) -> Result<Self> {
        Self::from_vec_impl(data, shape, device, false)
    }

    /// Creates a new tensor initialized with values from the input slice. The number of elements
    /// in this vector must be the same as the number of elements defined by the shape.
    ///```rust
    /// use candle_core::{Tensor, Device};
    /// let values = vec![1., 2., 3., 4., 5., 6., 7., 8.];
    /// let a = Tensor::from_slice(&values[1..7], (2, 3), &Device::Cpu)?;
    ///
    /// assert_eq!(a.to_vec2::<f64>()?, &[
    ///     [2., 3., 4.],
    ///     [5., 6., 7.]
    /// ]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn from_slice<S: ShapeWithOneHole, D: crate::WithDType>(
        array: &[D],
        shape: S,
        device: &Device,
    ) -> Result<Self> {
        let shape = shape.into_shape(array.len())?;
        let storage = device.storage_from_slice(array)?;
        let none = BackpropOp::none();
        Ok(from_storage(storage, shape, none, false))
    }

    pub(crate) fn same_shape_binary_op(&self, rhs: &Self, op: &'static str) -> Result<&Shape> {
        let lhs = self.shape();
        let rhs = rhs.shape();
        if lhs != rhs {
            Err(Error::ShapeMismatchBinaryOp {
                lhs: lhs.clone(),
                rhs: rhs.clone(),
                op,
            }
            .bt())
        } else {
            Ok(lhs)
        }
    }

    /// Returns true if the computation graph should track this op, that is if it is
    /// a variable or if it has some variable as dependencies.
    pub fn track_op(&self) -> bool {
        self.is_variable || self.op.is_some()
    }

    /// Creates a fresh tensor structure based on a storage and a shape.
    ///
    /// # Note
    /// - This uses contiguous strides
    /// - Ensure the shape is compatible with the shape of the storage.
    pub fn from_storage<S: Into<Shape>>(
        storage: Storage,
        shape: S,
        op: BackpropOp,
        is_variable: bool,
    ) -> Tensor {
        from_storage(storage, shape, op, is_variable)
    }

    // TODO: Also make an inplace version or a pre-allocated? This could be tricky
    // if this can create cycles in the compute graph.
    binary_op!(add, Add);
    binary_op!(mul, Mul);
    binary_op!(sub, Sub);
    binary_op!(div, Div);
    binary_op_scalar!(maximum, Maximum);
    binary_op_scalar!(minimum, Minimum);
    broadcast_binary_op!(broadcast_add, add);
    broadcast_binary_op!(broadcast_mul, mul);
    broadcast_binary_op!(broadcast_sub, sub);
    broadcast_binary_op!(broadcast_div, div);
    broadcast_binary_op!(broadcast_maximum, maximum);
    broadcast_binary_op!(broadcast_minimum, minimum);
    broadcast_binary_op!(broadcast_eq, eq);
    broadcast_binary_op!(broadcast_ne, ne);
    broadcast_binary_op!(broadcast_lt, lt);
    broadcast_binary_op!(broadcast_le, le);
    broadcast_binary_op!(broadcast_gt, gt);
    broadcast_binary_op!(broadcast_ge, ge);

    unary_op!(recip, Recip);
    unary_op!(neg, Neg);
    unary_op!(exp, Exp);
    unary_op!(log, Log);
    unary_op!(sin, Sin);
    unary_op!(cos, Cos);
    unary_op!(tanh, Tanh);
    unary_op!(abs, Abs);
    unary_op!(sqr, Sqr);
    unary_op!(sqrt, Sqrt);
    unary_op!(gelu, Gelu);
    unary_op!(gelu_erf, GeluErf);
    unary_op!(erf, Erf);
    unary_op!(relu, Relu);
    unary_op!(silu, Silu);
    unary_op!(ceil, Ceil);
    unary_op!(floor, Floor);
    unary_op!(round, Round);
    unary_op!(sign, Sign);

    /// Round element of the input tensor to the nearest integer.
    ///
    /// If the number of decimals is negative, it specifies the number of positions to the left of
    /// the decimal point.
    pub fn round_to(&self, decimals: i32) -> Result<Self> {
        let mult = 10f64.powi(decimals);
        (self * mult)?.round()? * (1f64 / mult)
    }

    /// Retrieves the single scalar value hold in the tensor. If the tensor contains multiple
    /// dimensions, an error is returned instead.
    pub fn to_scalar<S: crate::WithDType>(&self) -> Result<S> {
        if self.rank() != 0 {
            Err(Error::UnexpectedNumberOfDims {
                expected: 0,
                got: self.rank(),
                shape: self.shape().clone(),
            }
            .bt())?
        }
        let from_cpu_storage = |cpu_storage: &crate::CpuStorage| {
            let data = S::cpu_storage_as_slice(cpu_storage)?;
            Ok::<_, Error>(data[self.layout().start_offset()])
        };
        match &*self.storage() {
            Storage::Cpu(cpu_storage) => from_cpu_storage(cpu_storage),
            Storage::Cuda(storage) => from_cpu_storage(&storage.to_cpu_storage()?),
            Storage::Metal(storage) => from_cpu_storage(&storage.to_cpu_storage()?),
        }
    }

    /// An alias for `to_scalar`.
    pub fn to_vec0<S: crate::WithDType>(&self) -> Result<S> {
        self.to_scalar::<S>()
    }

    /// Repeat this tensor along the specified dimensions.
    pub fn repeat<S: Into<Shape>>(&self, shape: S) -> Result<Tensor> {
        // Similar to PyTorch, we extend the number of dimensions of self if needed.
        let repeats = shape.into();
        let repeats = repeats.dims();
        let mut inp = if self.rank() < repeats.len() {
            let shape = [vec![1; repeats.len() - self.rank()], self.dims().to_vec()].concat();
            self.reshape(shape)?
        } else {
            self.clone()
        };
        for (idx, &repeat) in repeats.iter().enumerate() {
            if repeat > 1 {
                inp = Tensor::cat(&vec![&inp; repeat], idx)?
            }
        }
        Ok(inp)
    }

    /// Creates grids of coordinates specified by the 1D inputs.
    ///
    /// # Arguments
    ///
    /// * `args` - A slice of 1D tensors.
    /// * `xy_indexing` - Whether to use xy indexing or ij indexing. If xy is selected, the
    ///   first dimension corresponds to the cardinality of the second input and the second
    ///   dimension corresponds to the cardinality of the first input. If ij is selected, the
    ///   dimensions are in the same order as the cardinality of the inputs.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device, Shape};
    /// let x = Tensor::new(&[1f32, 2., 3.], &Device::Cpu)?;
    /// let y = Tensor::new(&[4f32, 5., 6.], &Device::Cpu)?;
    ///
    /// let grids_xy = Tensor::meshgrid(&[&x, &y], true)?;
    ///
    /// assert_eq!(grids_xy.len(), 2);
    /// assert_eq!(grids_xy[0].dims(), &[3, 3]);
    ///
    /// assert_eq!(grids_xy[0].to_vec2::<f32>()?, &[[1., 2., 3.], [1., 2., 3.], [1., 2., 3.]]);
    /// assert_eq!(grids_xy[1].to_vec2::<f32>()?, &[[4., 4., 4.], [5., 5., 5.], [6., 6., 6.]]);
    ///
    /// let grids_ij = Tensor::meshgrid(&[&x, &y], false)?;
    ///
    /// assert_eq!(grids_ij[0].to_vec2::<f32>()?, &[[1., 1., 1.], [2., 2., 2.], [3., 3., 3.]]);
    /// assert_eq!(grids_ij[1].to_vec2::<f32>()?, &[[4., 5., 6.], [4., 5., 6.], [4., 5., 6.]]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// * Will return `Err` if `args` contains less than 2 tensors.
    ///
    pub fn meshgrid<A: AsRef<Tensor>>(args: &[A], xy_indexing: bool) -> Result<Vec<Self>> {
        if args.len() <= 1 {
            Err(Error::OpRequiresAtLeastTwoTensors { op: "meshgrid" }.bt())?
        }
        let args: Vec<_> = if xy_indexing {
            args.iter().rev().collect()
        } else {
            args.iter().collect()
        };

        let mut shape = Vec::with_capacity(args.len());
        for arg in args.iter() {
            shape.push(arg.as_ref().dims1()?)
        }

        let mut grids = Vec::with_capacity(args.len());
        for idx in 0..args.len() {
            let mut ones = vec![1usize; args.len()];
            ones[idx] = shape[idx];
            let arg = args[idx].as_ref().reshape(ones)?;
            let mut repeats = shape.clone();
            repeats[idx] = 1;
            let repeated_tensor = arg.repeat(repeats)?;
            grids.push(repeated_tensor);
        }
        if xy_indexing {
            grids.reverse();
        }
        Ok(grids)
    }

    /// This operation multiplies the input tensor by `mul` then adds `add` and return the result.
    /// The input values `mul` and `add` are casted to the appropriate type so some rounding might
    /// be performed.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let a = Tensor::new(&[[0f32, 1.], [2., 3.]], &Device::Cpu)?;
    /// let a = a.affine(4., -2.)?;
    /// assert_eq!(a.to_vec2::<f32>()?, &[[-2.0, 2.0], [6.0, 10.0]]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn affine(&self, mul: f64, add: f64) -> Result<Self> {
        if self.elem_count() == 0 {
            return Ok(self.clone());
        }
        let storage = self.storage().affine(self.layout(), mul, add)?;
        let op = BackpropOp::new1(self, |arg| Op::Affine { arg, mul, add });
        Ok(from_storage(storage, self.shape(), op, false))
    }

    /// Applies the Exponential Linear Unit (ELU) function on each element of the input tensor.
    pub fn elu(&self, alpha: f64) -> Result<Self> {
        if self.elem_count() == 0 {
            return Ok(self.clone());
        }
        let storage = self.storage().elu(self.layout(), alpha)?;
        let op = BackpropOp::new1(self, |t| Op::Elu(t, alpha));
        Ok(from_storage(storage, self.shape(), op, false))
    }

    /// Raise the tensor to some float exponent `e`.
    pub fn powf(&self, e: f64) -> Result<Self> {
        if self.elem_count() == 0 {
            return Ok(self.clone());
        }
        let storage = self.storage().powf(self.layout(), e)?;
        let op = BackpropOp::new1(self, |t| Op::Powf(t, e));
        Ok(from_storage(storage, self.shape(), op, false))
    }

    pub(crate) fn check_dim(&self, dim: usize, op: &'static str) -> Result<()> {
        if dim >= self.dims().len() {
            Err(Error::DimOutOfRange {
                shape: self.shape().clone(),
                dim: dim as i32,
                op,
            }
            .bt())?
        } else {
            Ok(())
        }
    }

    /// Split a tensor into the specified number of chunks, this may return less chunks than
    /// specified.
    pub fn chunk<D: Dim>(&self, chunks: usize, dim: D) -> Result<Vec<Self>> {
        let dim = dim.to_index(self.shape(), "chunk")?;
        let size = self.dim(dim)?;
        if size < chunks {
            (0..size).map(|i| self.narrow(dim, i, 1)).collect()
        } else {
            let chunk_size = size / chunks;
            let cnt_additional = size % chunks;
            let mut tensors = vec![];
            let mut sum_chunk_size = 0;
            for i in 0..chunks {
                let chunk_size = if i < cnt_additional {
                    chunk_size + 1
                } else {
                    chunk_size
                };
                let tensor = self.narrow(dim, sum_chunk_size, chunk_size)?;
                tensors.push(tensor);
                sum_chunk_size += chunk_size
            }
            Ok(tensors)
        }
    }

    /// Returns a new tensor that is a narrowed version of the input, the dimension `dim`
    /// ranges from `start` to `start + len`.
    /// ```
    /// use candle_core::{Tensor, Device};
    /// let a = Tensor::new(&[
    ///     [0f32, 1., 2.],
    ///     [3.  , 4., 5.],
    ///     [6.  , 7., 8.]
    /// ], &Device::Cpu)?;
    ///
    /// let b = a.narrow(0, 1, 2)?;
    /// assert_eq!(b.shape().dims(), &[2, 3]);
    /// assert_eq!(b.to_vec2::<f32>()?, &[
    ///     [3., 4., 5.],
    ///     [6., 7., 8.]
    /// ]);
    ///
    /// let c = a.narrow(1, 1, 1)?;
    /// assert_eq!(c.shape().dims(), &[3, 1]);
    /// assert_eq!(c.to_vec2::<f32>()?, &[
    ///     [1.],
    ///     [4.],
    ///     [7.]
    /// ]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn narrow<D: Dim>(&self, dim: D, start: usize, len: usize) -> Result<Self> {
        let dims = self.dims();
        let dim = dim.to_index(self.shape(), "narrow")?;
        let err = |msg| {
            Err::<(), _>(
                Error::NarrowInvalidArgs {
                    shape: self.shape().clone(),
                    dim,
                    start,
                    len,
                    msg,
                }
                .bt(),
            )
        };
        if start > dims[dim] {
            err("start > dim_len")?
        }
        if start.saturating_add(len) > dims[dim] {
            err("start + len > dim_len")?
        }
        if start == 0 && dims[dim] == len {
            Ok(self.clone())
        } else {
            let op = BackpropOp::new1(self, |t| Op::Narrow(t, dim, start, len));
            let layout = self.layout().narrow(dim, start, len)?;
            let tensor_ = Tensor_ {
                id: TensorId::new(),
                storage: self.storage.clone(),
                layout,
                op,
                is_variable: false,
                dtype: self.dtype,
                device: self.device.clone(),
            };
            Ok(Tensor(Arc::new(tensor_)))
        }
    }

    fn squeeze_dims(self, dims: &[usize]) -> Result<Self> {
        match dims {
            [] => Ok(self),
            [i] => self.squeeze(*i),
            dims => {
                let dims = self
                    .dims()
                    .iter()
                    .enumerate()
                    .filter_map(|(dim_idx, &v)| {
                        if dims.contains(&dim_idx) {
                            None
                        } else {
                            Some(v)
                        }
                    })
                    .collect::<Vec<_>>();
                self.reshape(dims)
            }
        }
    }

    fn reduce_impl<D: Dim>(&self, dim: D, keepdim: bool, op: ReduceOp) -> Result<Self> {
        let dim = dim.to_index(self.shape(), op.name())?;
        let storage = self.storage().reduce_op(op, self.layout(), &[dim])?;
        let mut dims = self.dims().to_vec();
        dims[dim] = 1;
        let op = match op {
            ReduceOp::Sum | ReduceOp::Min | ReduceOp::Max => {
                BackpropOp::new1(self, |arg| Op::Reduce(arg, op, dims.to_vec()))
            }
            ReduceOp::ArgMin | ReduceOp::ArgMax => BackpropOp::none(),
        };
        let res = from_storage(storage, dims, op, false);
        if keepdim {
            Ok(res)
        } else {
            res.squeeze_dims(&[dim])
        }
    }

    fn sum_impl<D: Dims>(&self, sum_dims: D, keepdim: bool) -> Result<Self> {
        let sum_dims = sum_dims.to_indexes(self.shape(), "sum")?;
        let storage = self
            .storage()
            .reduce_op(ReduceOp::Sum, self.layout(), &sum_dims)?;
        let mut dims = self.dims().to_vec();
        for &sum_dim in sum_dims.iter() {
            dims[sum_dim] = 1
        }
        let op = BackpropOp::new1(self, |a| Op::Reduce(a, ReduceOp::Sum, dims.to_vec()));
        let sum = from_storage(storage, dims, op, false);
        if keepdim {
            Ok(sum)
        } else {
            sum.squeeze_dims(&sum_dims)
        }
    }

    /// Roll the tensor input along the given dimension.
    /// Elements that are shifted beyond the last position are re-introduced at the first position.
    ///
    /// ```rust
    /// # use candle_core::{Tensor, Device};
    /// let tensor = Tensor::new(&[[0f32, 1.], [2., 3.], [4., 5.]], &Device::Cpu)?;
    /// let tensor = tensor.roll(1, 0)?;
    /// assert_eq!(tensor.to_vec2::<f32>()?, &[[4., 5.], [0., 1.], [2., 3.]]);
    /// let tensor = Tensor::new(&[[0f32, 1.], [2., 3.], [4., 5.]], &Device::Cpu)?;
    /// let tensor = tensor.roll(-1, 0)?;
    /// assert_eq!(tensor.to_vec2::<f32>()?, &[[2., 3.], [4., 5.], [0., 1.]]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn roll<D>(&self, shift: i32, dim: D) -> Result<Self>
    where
        D: Dim + Clone,
    {
        let dim = dim.to_index(self.shape(), "roll")?;
        let dim_size = self.dim(dim)?;
        let shift = shift.rem_euclid(dim_size as i32) as usize;
        if shift == 0 {
            Ok(self.clone())
        } else {
            let a = self.narrow(dim, 0, dim_size - shift)?;
            let b = self.narrow(dim, dim_size - shift, shift)?;
            Tensor::cat(&[&b, &a], dim)
        }
    }

    /// Returns the sum of all elements in the input tensor. The sum is performed over all the
    /// input dimensions.
    ///
    /// The resulting tensor has a shape that is similar to the shape of the input tensor, except
    /// that the number of elements for each dimension index in `sum_dims` is 1.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let a = Tensor::new(&[[0f32, 1.], [2., 3.]], &Device::Cpu)?;
    /// let s = a.sum_keepdim(0)?;
    /// assert_eq!(s.to_vec2::<f32>()?, &[[2., 4.]]);
    /// let s = a.sum_keepdim(1)?;
    /// assert_eq!(s.to_vec2::<f32>()?, &[[1.], [5.]]);
    /// let s = a.sum_keepdim((0, 1))?;
    /// assert_eq!(s.to_vec2::<f32>()?, &[[6.]]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn sum_keepdim<D: Dims>(&self, sum_dims: D) -> Result<Self> {
        self.sum_impl(sum_dims, true)
    }

    /// Returns the sum of all elements in the input tensor. The sum is performed over all the
    /// input dimensions and compared to `sum_keepdim` these dimensions are squeezed rather than
    /// kept.
    pub fn sum<D: Dims>(&self, sum_dims: D) -> Result<Self> {
        self.sum_impl(sum_dims, false)
    }

    /// Returns the mean of all elements in the input tensor. The mean is performed over all the
    /// input dimensions.
    ///
    /// The resulting tensor has a shape that is similar to the shape of the input tensor, except
    /// that the number of elements for each dimension index in `mean_dims` is 1.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let a = Tensor::new(&[[0f32, 1.], [2., 3.]], &Device::Cpu)?;
    /// let s = a.mean_keepdim(0)?;
    /// assert_eq!(s.to_vec2::<f32>()?, &[[1., 2.]]);
    /// let s = a.mean_keepdim(1)?;
    /// assert_eq!(s.to_vec2::<f32>()?, &[[0.5], [2.5]]);
    /// let s = a.mean_keepdim((0, 1))?;
    /// assert_eq!(s.to_vec2::<f32>()?, &[[1.5]]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn mean_keepdim<D: Dims>(&self, mean_dims: D) -> Result<Self> {
        let mean_dims = mean_dims.to_indexes(self.shape(), "mean-keepdim")?;
        let reduced_dim: usize = mean_dims.iter().map(|i| self.dims()[*i]).product();
        let scale = 1f64 / (reduced_dim as f64);
        self.sum_impl(mean_dims, true)? * scale
    }

    /// Returns the mean of all elements in the input tensor. The mean is performed over all the
    /// input dimensions and compared to `mean_keepdim` these dimensions are squeezed rather than
    /// kept.
    pub fn mean<D: Dims>(&self, mean_dims: D) -> Result<Self> {
        let mean_dims = mean_dims.to_indexes(self.shape(), "mean")?;
        let reduced_dim: usize = mean_dims.iter().map(|i| self.dims()[*i]).product();
        let scale = 1f64 / (reduced_dim as f64);
        self.sum_impl(mean_dims, false)? * scale
    }

    /// Returns the unbiased variance over the selected dimension.
    pub fn var_keepdim<D: Dim>(&self, dim: D) -> Result<Self> {
        let dim = dim.to_index(self.shape(), "var")?;
        let mean = self.mean_keepdim(dim)?;
        let squares = self.broadcast_sub(&mean)?.sqr()?;
        squares.sum_impl(dim, true)? / (self.dim(dim)? - 1) as f64
    }

    /// Returns the unbiased variance over the selected dimension.
    pub fn var<D: Dim>(&self, dim: D) -> Result<Self> {
        let dim = dim.to_index(self.shape(), "var")?;
        self.var_keepdim(dim)?.squeeze(dim)
    }

    /// Gathers the maximum value across the selected dimension. The resulting shape has the same
    /// number of dimensions as the original tensor and the select dimension has a single element.
    pub fn max_keepdim<D: Dim>(&self, dim: D) -> Result<Self> {
        self.reduce_impl(dim, true, ReduceOp::Max)
    }

    /// Similar to `max_keepdim` but the target dimension is squeezed.
    pub fn max<D: Dim>(&self, dim: D) -> Result<Self> {
        self.reduce_impl(dim, false, ReduceOp::Max)
    }

    /// Gathers the minimum value across the selected dimension. The resulting shape has the same
    /// number of dimensions as the original tensor and the select dimension has a single element.
    pub fn min_keepdim<D: Dim>(&self, dim: D) -> Result<Self> {
        self.reduce_impl(dim, true, ReduceOp::Min)
    }

    /// Similar to `min_keepdim` but the target dimension is squeezed.
    pub fn min<D: Dim>(&self, dim: D) -> Result<Self> {
        self.reduce_impl(dim, false, ReduceOp::Min)
    }

    pub fn argmax_keepdim<D: Dim>(&self, dim: D) -> Result<Self> {
        self.reduce_impl(dim, true, ReduceOp::ArgMax)
    }

    /// Similar to `argmax_keepdim` but the target dimension is squeezed.
    pub fn argmax<D: Dim>(&self, dim: D) -> Result<Self> {
        self.reduce_impl(dim, false, ReduceOp::ArgMax)
    }

    pub fn argmin_keepdim<D: Dim>(&self, dim: D) -> Result<Self> {
        self.reduce_impl(dim, true, ReduceOp::ArgMin)
    }

    /// Similar to `argmin_keepdim` but the target dimension is squeezed.
    pub fn argmin<D: Dim>(&self, dim: D) -> Result<Self> {
        self.reduce_impl(dim, false, ReduceOp::ArgMin)
    }

    /// Element-wise comparison between two tensors, e.g. equality, greater than, ... The actual
    /// comparison operation is specified by the `op` argument.
    ///
    /// The returned tensor has the same shape as the original tensors and uses `u8` elements.
    pub fn cmp<T: TensorOrScalar>(&self, rhs: T, op: CmpOp) -> Result<Self> {
        let rhs = match rhs.to_tensor_scalar()? {
            crate::scalar::TensorScalar::Tensor(rhs) => rhs,
            crate::scalar::TensorScalar::Scalar(rhs) => rhs
                .to_dtype(self.dtype())?
                .to_device(self.device())?
                .broadcast_as(self.shape())?,
        };
        let shape = self.same_shape_binary_op(&rhs, "cmp")?;
        let storage = self
            .storage()
            .cmp(op, &rhs.storage(), self.layout(), rhs.layout())?;
        let op = BackpropOp::new1(self, |a| Op::Cmp(a, op));
        Ok(from_storage(storage, shape.dims(), op, false))
    }

    /// Element-wise equality.
    pub fn eq<T: TensorOrScalar>(&self, rhs: T) -> Result<Self> {
        self.cmp(rhs, CmpOp::Eq)
    }

    /// Element-wise non-equality.
    pub fn ne<T: TensorOrScalar>(&self, rhs: T) -> Result<Self> {
        self.cmp(rhs, CmpOp::Ne)
    }

    /// Element-wise comparison with lower-than, the returned tensor uses value 1 where `self <
    /// rhs` and 0 otherwise.
    pub fn lt<T: TensorOrScalar>(&self, rhs: T) -> Result<Self> {
        self.cmp(rhs, CmpOp::Lt)
    }

    /// Element-wise comparison with greater-than, the returned tensor uses value 1 where `self >
    /// rhs` and 0 otherwise.
    pub fn gt<T: TensorOrScalar>(&self, rhs: T) -> Result<Self> {
        self.cmp(rhs, CmpOp::Gt)
    }

    /// Element-wise comparison with greater-equal, the returned tensor uses value 1 where `self >=
    /// rhs` and 0 otherwise.
    pub fn ge<T: TensorOrScalar>(&self, rhs: T) -> Result<Self> {
        self.cmp(rhs, CmpOp::Ge)
    }

    /// Element-wise comparison with lower-equal, the returned tensor uses value 1 where `self <=
    /// rhs` and 0 otherwise.
    pub fn le<T: TensorOrScalar>(&self, rhs: T) -> Result<Self> {
        self.cmp(rhs, CmpOp::Le)
    }

    /// Clamp the tensor values to be between `min` and `max`.
    pub fn clamp<T1: TensorOrScalar, T2: TensorOrScalar>(&self, min: T1, max: T2) -> Result<Self> {
        self.maximum(min)?.minimum(max)
    }

    /// Interpolate the input tensor to the `target_size` size, taking the value of the nearest element.
    ///
    /// The input tensor should have three dimensions, `(batch, channels, l)`, the returned
    /// tensor also has three dimensions, `(batch, channels, target_size)`.
    pub fn interpolate1d(&self, target_size: usize) -> Result<Self> {
        let (n, c, _l) = self.dims3()?;
        let op = BackpropOp::new1(self, |arg| Op::UpsampleNearest1D { arg, target_size });
        let storage = self
            .storage()
            .upsample_nearest1d(self.layout(), target_size)?;
        Ok(from_storage(storage, (n, c, target_size), op, false))
    }

    /// Alias for `interpolate1d`.
    pub fn upsample_nearest1d(&self, target_size: usize) -> Result<Self> {
        self.interpolate1d(target_size)
    }

    /// Interpolate the input tensor to the `(target_h, target_w)` size, taking the value of the
    /// nearest element.
    ///
    /// The input tensor should have four dimensions, `(batch, channels, h, w)`, the returned
    /// tensor also has four dimensions, `(batch, channels, target_h, target_w)`.
    pub fn interpolate2d(&self, target_h: usize, target_w: usize) -> Result<Self> {
        let (n, c, _h, _w) = self.dims4()?;
        let op = BackpropOp::new1(self, |arg| Op::UpsampleNearest2D {
            arg,
            target_h,
            target_w,
        });
        let storage = self
            .storage()
            .upsample_nearest2d(self.layout(), target_h, target_w)?;
        Ok(from_storage(storage, (n, c, target_h, target_w), op, false))
    }

    /// Alias for `interpolate2d`.
    pub fn upsample_nearest2d(&self, target_h: usize, target_w: usize) -> Result<Self> {
        self.interpolate2d(target_h, target_w)
    }

    /// Bilinear interpolation to resize the input tensor to the specified size.
    ///
    /// The input tensor should have four dimensions: `(batch, channels, h, w)`.
    /// The returned tensor also has four dimensions: `(batch, channels, target_h, target_w)`.
    ///
    /// # Arguments
    ///
    /// * `target_h` - Target height
    /// * `target_w` - Target width  
    /// * `align_corners` - If true, corner pixels are aligned. If false (default),
    ///   pixels are treated as areas (matches PyTorch default behavior).
    ///
    /// # Example
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// # fn main() -> candle_core::Result<()> {
    /// let t = Tensor::arange(0f32, 16f32, &Device::Cpu)?.reshape((1, 1, 4, 4))?;
    /// let upsampled = t.upsample_bilinear2d(8, 8, false)?;
    /// assert_eq!(upsampled.dims(), &[1, 1, 8, 8]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn upsample_bilinear2d(
        &self,
        target_h: usize,
        target_w: usize,
        align_corners: bool,
    ) -> Result<Self> {
        let (n, c, _h, _w) = self.dims4()?;
        let op = BackpropOp::new1(self, |arg| Op::UpsampleBilinear2D {
            arg,
            target_h,
            target_w,
            align_corners,
        });
        // Pass None for scale factors (size mode)
        let storage = self.storage().upsample_bilinear2d(
            self.layout(),
            target_h,
            target_w,
            align_corners,
            None,
            None,
        )?;
        Ok(from_storage(storage, (n, c, target_h, target_w), op, false))
    }

    /// Bilinear interpolation using scale factors.
    ///
    /// Similar to `upsample_bilinear2d` but uses scale factors instead of absolute sizes.
    /// This matches PyTorch's `interpolate(scale_factor=...)` behavior.
    ///
    /// # Arguments
    ///
    /// * `scale_h` - Height scaling factor
    /// * `scale_w` - Width scaling factor
    /// * `align_corners` - If true, corner pixels are aligned
    ///
    /// # Example
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// # fn main() -> candle_core::Result<()> {
    /// let t = Tensor::arange(0f32, 16f32, &Device::Cpu)?.reshape((1, 1, 4, 4))?;
    /// // Scale by 2x in both dimensions
    /// let upsampled = t.upsample_bilinear2d_with_scale(2.0, 2.0, false)?;
    /// assert_eq!(upsampled.dims(), &[1, 1, 8, 8]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn upsample_bilinear2d_with_scale(
        &self,
        scale_h: f64,
        scale_w: f64,
        align_corners: bool,
    ) -> Result<Self> {
        let (n, c, height_in, width_in) = self.dims4()?;

        // Calculate output size (floor, matching PyTorch)
        let height_out = (height_in as f64 * scale_h).floor() as usize;
        let width_out = (width_in as f64 * scale_w).floor() as usize;

        // Early return if size unchanged
        if height_in == height_out && width_in == width_out {
            return Ok(self.clone());
        }

        let op = BackpropOp::new1(self, |arg| Op::UpsampleBilinear2D {
            arg,
            target_h: height_out,
            target_w: width_out,
            align_corners,
        });

        // Pass original scale factors (scale_factor mode)
        // This ensures PyTorch-compatible scale calculation
        let storage = self.storage().upsample_bilinear2d(
            self.layout(),
            height_out,
            width_out,
            align_corners,
            Some(scale_h),
            Some(scale_w),
        )?;
        Ok(from_storage(
            storage,
            (n, c, height_out, width_out),
            op,
            false,
        ))
    }

    /// 2D average pooling over an input tensor with multiple channels.
    ///
    /// The input tensor should have four dimensions, `(batch, channels, h, w)`, the returned
    /// tensor also has four dimensions, `(batch, channels, h', w')`. The pooling is performed on
    /// the two last dimensions using a kernel of size `sz`. The returned element is the average
    /// value over the kernel window.
    pub fn avg_pool2d<T: crate::ToUsize2>(&self, sz: T) -> Result<Self> {
        let sz = sz.to_usize2();
        self.avg_pool2d_with_stride(sz, sz)
    }

    /// Same as `avg_pool2d` but with a `stride` that can be set to a value different from the
    /// kernel size.
    pub fn avg_pool2d_with_stride<T: crate::ToUsize2>(
        &self,
        kernel_size: T,
        stride: T,
    ) -> Result<Self> {
        let kernel_size = kernel_size.to_usize2();
        let stride = stride.to_usize2();
        let (n, c, h, w) = self.dims4()?;
        if h < kernel_size.0 || w < kernel_size.1 {
            bail!("kernel-size {kernel_size:?} is larger than the input size {h},{w}")
        }
        // https://pytorch.org/docs/stable/generated/torch.nn.AvgPool2d.html#torch.nn.AvgPool2d
        let h_out = (h - kernel_size.0) / stride.0 + 1;
        let w_out = (w - kernel_size.1) / stride.1 + 1;
        let op = BackpropOp::new1(self, |arg| Op::AvgPool2D {
            arg,
            kernel_size,
            stride,
        });
        let storage = self
            .storage()
            .avg_pool2d(self.layout(), kernel_size, stride)?;
        Ok(from_storage(storage, (n, c, h_out, w_out), op, false))
    }

    /// 2D max pooling over an input tensor with multiple channels.
    ///
    /// The input tensor should have four dimensions, `(batch, channels, h, w)`, the returned
    /// tensor also has four dimensions, `(batch, channels, h', w')`. The pooling is performed on
    /// the two last dimensions using a kernel of size `sz`, the returned element is the maximum
    /// value over the kernel window.
    pub fn max_pool2d<T: crate::ToUsize2>(&self, sz: T) -> Result<Self> {
        let sz = sz.to_usize2();
        self.max_pool2d_with_stride(sz, sz)
    }

    /// Same as `max_pool2d` but with a `stride` that can be set to a value different from the
    /// kernel size.
    pub fn max_pool2d_with_stride<T: crate::ToUsize2>(
        &self,
        kernel_size: T,
        stride: T,
    ) -> Result<Self> {
        let kernel_size = kernel_size.to_usize2();
        let stride = stride.to_usize2();
        let (n, c, h, w) = self.dims4()?;
        if h < kernel_size.0 || w < kernel_size.1 {
            bail!("kernel-size {kernel_size:?} is larger than the input size {h},{w}")
        }
        // https://pytorch.org/docs/stable/generated/torch.nn.MaxPool2d.html#torch.nn.MaxPool2d
        let h_out = (h - kernel_size.0) / stride.0 + 1;
        let w_out = (w - kernel_size.1) / stride.1 + 1;
        let op = BackpropOp::new1(self, |arg| Op::MaxPool2D {
            arg,
            kernel_size,
            stride,
        });
        let storage = self
            .storage()
            .max_pool2d(self.layout(), kernel_size, stride)?;
        Ok(from_storage(storage, (n, c, h_out, w_out), op, false))
    }

    /// Computes the dot product of two 1D tensors.
    ///
    /// - If inputs are 1D vectors (`[n]`), returns their scalar dot product.
    /// - Panics if shapes are not compatible
    /// - Not supported for integer dtypes
    ///
    /// # Example (vectors)
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let t1 = Tensor::new(&[1.0, 2.0, 3.0], &Device::Cpu)?;
    /// let t2 = Tensor::new(&[4.0, 5.0, 6.0], &Device::Cpu)?;
    /// let res = t1.dot(&t2)?;
    /// assert_eq!(res.to_scalar::<f64>()?, 32.);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn dot(&self, rhs: &Self) -> Result<Self> {
        if self.dims().len() != 1 || rhs.dims().len() != 1 {
            return Err(Error::ShapeMismatchBinaryOp {
                lhs: self.shape().clone(),
                rhs: rhs.shape().clone(),
                op: "dot",
            });
        }

        (self * rhs).and_then(|ret| ret.sum_all())
    }

    /// Computes the **Frobenius norm** (L2 norm of all elements) of the tensor.
    /// - Output is `sqrt(sum(x^2))`.
    /// - Always returns a scalar (`[]` shape).
    ///
    /// # Example
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let t = Tensor::new(&[[3., 4.], [0., 0.]], &Device::Cpu)?;
    /// let norm = t.norm()?;
    /// assert_eq!(norm.to_scalar::<f64>()?, 5.);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn norm(&self) -> Result<Self> {
        if self.dtype().is_int() {
            bail!("norm not supported for integer dtypes");
        }

        self.sqr().and_then(|x| x.sum_all()).and_then(|x| x.sqrt())
    }

    /// Performs strict matrix-vector multiplication (`[m, n] * [n] = [m]`).
    ///
    /// - If `self` is a matrix (`[m, n]`) and `rhs` is a vector (`[n]`), returns a vector (`[m]`).
    /// - **No broadcasting**: Panics if `self` is not 2D or if `rhs` is not 1D with matching size.
    ///
    /// # Example
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let mat = Tensor::new(&[[1., 2., 3.], [4., 5., 6.]], &Device::Cpu)?;
    /// let vec = Tensor::new(&[1., 1., 1.], &Device::Cpu)?;
    /// let res = mat.mv(&vec)?;
    /// assert_eq!(res.to_vec1::<f64>()?, [6., 15.]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn mv(&self, rhs: &Self) -> Result<Self> {
        // Strict shape checks
        let lhs_dims = self.dims();
        let rhs_dims = rhs.dims();
        if lhs_dims.len() != 2 || rhs_dims.len() != 1 || lhs_dims[1] != rhs_dims[0] {
            return Err(Error::ShapeMismatchBinaryOp {
                lhs: self.shape().clone(),
                rhs: rhs.shape().clone(),
                op: "mv",
            });
        }

        // Direct matmul after ensuring rhs is column vector
        self.matmul(&rhs.unsqueeze(1)?)?.squeeze(1)
    }

    /// Returns the matrix-multiplication of the input tensor with the other provided tensor.
    ///
    /// # Arguments
    ///
    /// * `self` - A tensor with dimensions `b1, b2, ..., bi, m, k`.
    /// * `rhs` - A tensor with dimensions `b1, b2, ..., bi, k, n`.
    ///
    /// The resulting tensor has dimensions `b1, b2, ..., bi, m, n`.
    pub fn matmul(&self, rhs: &Self) -> Result<Self> {
        let a_dims = self.shape().dims();
        let b_dims = rhs.shape().dims();

        let dim = a_dims.len();

        if dim < 2 || b_dims.len() != dim {
            Err(Error::ShapeMismatchBinaryOp {
                lhs: self.shape().clone(),
                rhs: rhs.shape().clone(),
                op: "matmul",
            }
            .bt())?
        }

        let m = a_dims[dim - 2];
        let k = a_dims[dim - 1];
        let k2 = b_dims[dim - 2];
        let n = b_dims[dim - 1];

        let c_shape = Shape::from(&a_dims[..dim - 2]).extend(&[m, n]);
        if c_shape.elem_count() == 0 || k == 0 {
            return Tensor::zeros(c_shape, self.dtype(), self.device());
        }
        let batching: usize = a_dims[..dim - 2].iter().product();
        let batching_b: usize = b_dims[..dim - 2].iter().product();
        if k != k2 || batching != batching_b {
            Err(Error::ShapeMismatchBinaryOp {
                lhs: self.shape().clone(),
                rhs: rhs.shape().clone(),
                op: "matmul",
            }
            .bt())?
        }

        let storage = self.storage().matmul(
            &rhs.storage(),
            (batching, m, n, k),
            self.layout(),
            rhs.layout(),
        )?;
        let op = BackpropOp::new2(self, rhs, Op::Matmul);
        Ok(from_storage(storage, c_shape, op, false))
    }

    /// Matrix-multiplication with broadcasting support.
    ///
    /// Compared to `matmul` the two matrixes are allowed to have different dimensions as long as
    /// they are compatible for broadcast. E.g. if `self` has shape `(j, 1, n, k)` and `rhs` has
    /// shape `(l, k, m)`, the output will have shape `(j, l, n, m)`.
    pub fn broadcast_matmul(&self, rhs: &Self) -> Result<Self> {
        let lhs = self;
        let (l_shape, r_shape) = lhs.shape().broadcast_shape_matmul(rhs.shape())?;
        let l_broadcast = l_shape != *lhs.shape();
        let r_broadcast = r_shape != *rhs.shape();
        // TODO: Avoid concretising the broadcasted matrixes via contiguous.
        match (l_broadcast, r_broadcast) {
            (true, true) => lhs
                .broadcast_as(&l_shape)?
                .contiguous()?
                .matmul(&rhs.broadcast_as(&r_shape)?.contiguous()?),
            (false, true) => lhs.matmul(&rhs.broadcast_as(&r_shape)?.contiguous()?),
            (true, false) => lhs.broadcast_as(&l_shape)?.contiguous()?.matmul(rhs),
            (false, false) => lhs.matmul(rhs),
        }
    }

    /// Returns a tensor with the same shape as the input tensor, the values are taken from
    /// `on_true` if the input tensor value is not zero, and `on_false` at the positions where the
    /// input tensor is equal to zero.
    pub fn where_cond(&self, on_true: &Self, on_false: &Self) -> Result<Self> {
        let _shap = self.same_shape_binary_op(on_true, "where_cond")?;
        let shape = self.same_shape_binary_op(on_false, "where_cond")?;
        let storage = self.storage().where_cond(
            self.layout(),
            &on_true.storage(),
            on_true.layout(),
            &on_false.storage(),
            on_false.layout(),
        )?;
        let op = BackpropOp::new3(self, on_true, on_false, Op::WhereCond);
        Ok(from_storage(storage, shape, op, false))
    }

    /// Returns a tensor with the values from the `self` tensor at the index corresponding to the
    /// values hold in the `ids` tensor.
    ///
    /// # Arguments
    ///
    /// * `self` - A tensor with dimensions `v, h`.
    /// * `ids` - A tensor with dimensions `s` and with integer values between 0 and v (exclusive).
    ///
    /// The resulting tensor has dimensions `s, h`. `s` is called the sequence length, `v` the
    /// vocabulary size, and `h` the hidden size.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let values = Tensor::new(&[[0f32, 1.], [2., 3.], [4., 5.]], &Device::Cpu)?;
    /// let ids = Tensor::new(&[2u32, 1u32, 2u32], &Device::Cpu)?;
    /// let emb = values.embedding(&ids)?;
    /// assert_eq!(emb.to_vec2::<f32>()?, &[[4., 5.], [2., 3.], [4., 5.]]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn embedding(&self, ids: &Self) -> Result<Self> {
        if self.rank() != 2 || ids.rank() != 1 {
            Err(Error::ShapeMismatchBinaryOp {
                lhs: self.shape().clone(),
                rhs: ids.shape().clone(),
                op: "embedding",
            }
            .bt())?
        }
        self.index_select(ids, 0)
    }

    fn scatter_checks(&self, indexes: &Self, source: &Self, dim: usize) -> Result<()> {
        let source_dims = source.dims();
        let self_dims = self.dims();
        let mismatch = if source_dims.len() != self_dims.len() {
            true
        } else {
            let mut mismatch = false;
            for (i, (&d1, &d2)) in self_dims.iter().zip(source_dims.iter()).enumerate() {
                if i != dim && d1 != d2 {
                    mismatch = true;
                    break;
                }
            }
            mismatch
        };
        if mismatch {
            Err(Error::ShapeMismatchBinaryOp {
                op: "scatter (self, src)",
                lhs: self.shape().clone(),
                rhs: source.shape().clone(),
            }
            .bt())?
        }
        if indexes.dims() != source.dims() {
            Err(Error::ShapeMismatchBinaryOp {
                op: "scatter (indexes, src)",
                lhs: indexes.shape().clone(),
                rhs: source.shape().clone(),
            }
            .bt())?
        }
        Ok(())
    }

    pub fn scatter<D: Dim>(&self, indexes: &Self, source: &Self, dim: D) -> Result<Self> {
        let dim = dim.to_index(self.shape(), "scatter")?;
        self.scatter_checks(indexes, source, dim)?;
        let shape = self.shape();
        let mut storage = unsafe { self.device().alloc_uninit(shape, self.dtype())? };
        self.storage()
            .copy_strided_src(&mut storage, 0, self.layout())?;
        let layout = Layout::contiguous(shape);
        storage.scatter_set(
            &layout,
            &indexes.storage(),
            indexes.layout(),
            &source.storage(),
            source.layout(),
            dim,
        )?;
        let op = BackpropOp::new3(self, indexes, source, |t1, t2, t3| {
            Op::Scatter(t1, t2, t3, dim)
        });
        Ok(from_storage(storage, self.shape(), op, false))
    }

    pub fn scatter_set<D: Dim>(&self, indexes: &Self, source: &Self, dim: D) -> Result<()> {
        if self.same_storage(source) {
            crate::bail!("cannot use slice_set when self and src share their storage")
        }
        let dim = dim.to_index(self.shape(), "scatter-set")?;
        self.scatter_checks(indexes, source, dim)?;
        self.storage_mut().scatter_set(
            self.layout(),
            &indexes.storage(),
            indexes.layout(),
            &source.storage(),
            source.layout(),
            dim,
        )?;
        Ok(())
    }

    pub fn scatter_add<D: Dim>(&self, indexes: &Self, source: &Self, dim: D) -> Result<Self> {
        let dim = dim.to_index(self.shape(), "scatter-add")?;
        self.scatter_checks(indexes, source, dim)?;
        let shape = self.shape();
        let mut storage = unsafe { self.device().alloc_uninit(shape, self.dtype())? };
        self.storage()
            .copy_strided_src(&mut storage, 0, self.layout())?;
        let layout = Layout::contiguous(shape);
        storage.scatter_add(
            &layout,
            &indexes.storage(),
            indexes.layout(),
            &source.storage(),
            source.layout(),
            dim,
        )?;
        let op = BackpropOp::new3(self, indexes, source, |t1, t2, t3| {
            Op::ScatterAdd(t1, t2, t3, dim)
        });
        Ok(from_storage(storage, self.shape(), op, false))
    }

    pub fn scatter_add_set<D: Dim>(&self, indexes: &Self, source: &Self, dim: D) -> Result<()> {
        if self.same_storage(source) {
            crate::bail!("cannot use slice_set when self and src share their storage")
        }
        let dim = dim.to_index(self.shape(), "scatter-add-set")?;
        self.scatter_checks(indexes, source, dim)?;
        self.storage_mut().scatter_add(
            self.layout(),
            &indexes.storage(),
            indexes.layout(),
            &source.storage(),
            source.layout(),
            dim,
        )?;
        Ok(())
    }

    /// Embeds the values of the `src` tensor into the `self` tensor on the specified dimension.
    pub fn slice_scatter<D: Dim>(&self, src: &Self, dim: D, start: usize) -> Result<Self> {
        let dim = dim.to_index(self.shape(), "slice-scatter")?;
        if dim == 0 {
            self.slice_scatter0(src, start)
        } else {
            // TODO: Maybe we want to add a more efficient implementation at some point.
            self.transpose(0, dim)?
                .slice_scatter0(&src.transpose(0, dim)?, start)?
                .transpose(0, dim)
        }
    }

    /// Embeds the values of the `src` tensor into the `self` tensor on the first dimension.
    pub fn slice_scatter0(&self, src: &Self, start: usize) -> Result<Self> {
        if self.dtype() != src.dtype() {
            Err(Error::DTypeMismatchBinaryOp {
                lhs: self.dtype(),
                rhs: src.dtype(),
                op: "slice-scatter",
            }
            .bt())?
        }
        if self.device().location() != src.device.location() {
            Err(Error::DeviceMismatchBinaryOp {
                lhs: self.device().location(),
                rhs: src.device().location(),
                op: "slice-scatter",
            }
            .bt())?
        }
        if self.rank() != src.rank() {
            Err(Error::UnexpectedNumberOfDims {
                expected: self.rank(),
                got: src.rank(),
                shape: src.shape().clone(),
            }
            .bt())?
        }
        let shape_ok =
            self.dims()
                .iter()
                .zip(src.dims().iter())
                .enumerate()
                .all(|(dim_idx, (&d1, &d2))| {
                    if 0 == dim_idx {
                        d2 + start <= d1
                    } else {
                        d1 == d2
                    }
                });
        if !shape_ok {
            Err(Error::ShapeMismatchBinaryOp {
                op: "slice-scatter (self, src)",
                lhs: self.shape().clone(),
                rhs: src.shape().clone(),
            }
            .bt())?
        }
        let mut storage = unsafe { self.device().alloc_uninit(self.shape(), self.dtype())? };
        self.storage()
            .copy_strided_src(&mut storage, 0, self.layout())?;
        let offset = start * src.dims()[1..].iter().product::<usize>();
        src.storage()
            .copy_strided_src(&mut storage, offset, src.layout())?;
        let op = BackpropOp::new2(self, src, |t1, t2| Op::SliceScatter0(t1, t2, start));
        Ok(from_storage(storage, self.shape(), op, false))
    }

    /// Accumulate element from `source` at indexes `indexes` and add them to `self`.
    pub fn index_add<D: Dim>(&self, indexes: &Self, source: &Self, dim: D) -> Result<Self> {
        let dim = dim.to_index(self.shape(), "index-add")?;
        let source_dims = source.dims();
        let self_dims = self.dims();
        let mismatch = if source_dims.len() != self_dims.len() {
            true
        } else {
            let mut mismatch = false;
            for (i, (&d1, &d2)) in self_dims.iter().zip(source_dims.iter()).enumerate() {
                if i != dim && d1 != d2 {
                    mismatch = true;
                    break;
                }
            }
            mismatch
        };
        if mismatch {
            Err(Error::ShapeMismatchBinaryOp {
                op: "index-add (self, source)",
                lhs: self.shape().clone(),
                rhs: source.shape().clone(),
            }
            .bt())?
        }
        // The number of element in indexes must match the dimension on which the add is
        // performed on the source tensor (and the index values from `indexes` are taken from
        // the target tensor self)
        let indexes_len = indexes.dims1()?;
        if source_dims[dim] != indexes_len {
            Err(Error::ShapeMismatchBinaryOp {
                op: "index-add (ids, source))",
                lhs: indexes.shape().clone(),
                rhs: source.shape().clone(),
            }
            .bt())?
        }
        let storage = self.storage().index_add(
            self.layout(),
            &indexes.storage(),
            indexes.layout(),
            &source.storage(),
            source.layout(),
            dim,
        )?;
        let op = BackpropOp::new3(self, indexes, source, |t1, t2, t3| {
            Op::IndexAdd(t1, t2, t3, dim)
        });
        Ok(from_storage(storage, self.shape(), op, false))
    }

    /// Gather values across the target dimension.
    ///
    /// # Arguments
    ///
    /// * `self` - The input tensor.
    /// * `indexes` - The indices of elements to gather, this should have same number of dimensions as `self`
    ///   and indexes.dims()[d] <= self.dims()[d] for all dimensions d != dim
    /// * `dim` - the target dimension.
    ///
    /// The resulting tensor has the same shape as `indexes` and use values from `self` indexed on
    /// dimension `dim` by the values in `indexes`.
    pub fn gather<D: Dim>(&self, indexes: &Self, dim: D) -> Result<Self> {
        let dim = dim.to_index(self.shape(), "gather")?;

        let self_dims = self.dims();
        let indexes_dims = indexes.dims();
        let mismatch = if indexes_dims.len() != self_dims.len() {
            true
        } else {
            let mut mismatch = false;
            for (i, (&d1, &d2)) in self_dims.iter().zip(indexes_dims.iter()).enumerate() {
                if i != dim && d1 < d2 {
                    mismatch = true;
                    break;
                }
            }
            mismatch
        };
        if mismatch {
            Err(Error::ShapeMismatchBinaryOp {
                op: "gather",
                lhs: self.shape().clone(),
                rhs: indexes.shape().clone(),
            }
            .bt())?
        }
        let storage =
            self.storage()
                .gather(self.layout(), &indexes.storage(), indexes.layout(), dim)?;
        let op = BackpropOp::new2(self, indexes, |t1, t2| Op::Gather(t1, t2, dim));
        Ok(from_storage(storage, indexes.shape(), op, false))
    }

    /// Select values for the input tensor at the target indexes across the specified dimension.
    ///
    /// The `indexes` is argument is an int tensor with a single dimension.
    /// The output has the same number of dimension as the `self` input. The target dimension of
    /// the output has length the length of `indexes` and the values are taken from `self` using
    /// the index from `indexes`. Other dimensions have the same number of elements as the input
    /// tensor.
    pub fn index_select<D: Dim>(&self, indexes: &Self, dim: D) -> Result<Self> {
        let dim = dim.to_index(self.shape(), "index-select")?;
        let indexes_len = match indexes.dims() {
            [l] => *l,
            _ => Err(Error::ShapeMismatchBinaryOp {
                lhs: self.shape().clone(),
                rhs: indexes.shape().clone(),
                op: "index-select",
            }
            .bt())?,
        };
        let storage = self.storage().index_select(
            &indexes.storage(),
            self.layout(),
            indexes.layout(),
            dim,
        )?;
        let mut dims = self.dims().to_vec();
        dims[dim] = indexes_len;
        let op = BackpropOp::new2(self, indexes, |t1, t2| Op::IndexSelect(t1, t2, dim));
        Ok(from_storage(storage, dims, op, false))
    }

    /// Returns an iterator over position of the elements in the storage when ranging over the
    /// index tuples in lexicographic order.
    pub fn strided_index(&self) -> crate::StridedIndex<'_> {
        self.layout.strided_index()
    }

    /// Similar to `strided_index` but returns the position of the start of each contiguous block
    /// as well as the length of the contiguous blocks. For a contiguous tensor, the index iterator
    /// will only return the start offset and the size would be the number of elements in the
    /// tensor.
    pub fn strided_blocks(&self) -> crate::StridedBlocks<'_> {
        self.layout.strided_blocks()
    }

    /// Returns the data contained in a 1D tensor as a vector of scalar values.
    pub fn to_vec1<S: crate::WithDType>(&self) -> Result<Vec<S>> {
        if self.rank() != 1 {
            Err(Error::UnexpectedNumberOfDims {
                expected: 1,
                got: self.rank(),
                shape: self.shape().clone(),
            }
            .bt())?
        }
        let from_cpu_storage = |cpu_storage: &crate::CpuStorage| {
            let data = S::cpu_storage_as_slice(cpu_storage)?;
            let data = match self.layout.contiguous_offsets() {
                Some((o1, o2)) => data[o1..o2].to_vec(),
                None => self.strided_index().map(|i| data[i]).collect(),
            };
            Ok::<Vec<_>, Error>(data)
        };
        match &*self.storage() {
            Storage::Cpu(storage) => from_cpu_storage(storage),
            Storage::Cuda(storage) => from_cpu_storage(&storage.to_cpu_storage()?),
            Storage::Metal(storage) => from_cpu_storage(&storage.to_cpu_storage()?),
        }
    }

    /// Returns the data contained in a 2D tensor as a vector of vector of scalar values.
    pub fn to_vec2<S: crate::WithDType>(&self) -> Result<Vec<Vec<S>>> {
        let (dim1, dim2) = self.dims2()?;
        let from_cpu_storage = |cpu_storage: &crate::CpuStorage| {
            let data = S::cpu_storage_as_slice(cpu_storage)?;
            let mut rows = vec![];
            match self.layout.contiguous_offsets() {
                Some((o1, o2)) => {
                    let data = &data[o1..o2];
                    for idx_row in 0..dim1 {
                        rows.push(data[idx_row * dim2..(idx_row + 1) * dim2].to_vec())
                    }
                }
                None => {
                    let mut src_index = self.strided_index();
                    for _idx_row in 0..dim1 {
                        let row = (0..dim2).map(|_| data[src_index.next().unwrap()]).collect();
                        rows.push(row)
                    }
                    assert!(src_index.next().is_none());
                }
            }
            Ok(rows)
        };
        match &*self.storage() {
            Storage::Cpu(storage) => from_cpu_storage(storage),
            Storage::Cuda(storage) => from_cpu_storage(&storage.to_cpu_storage()?),
            Storage::Metal(storage) => from_cpu_storage(&storage.to_cpu_storage()?),
        }
    }

    /// Returns the data contained in a 3D tensor.
    pub fn to_vec3<S: crate::WithDType>(&self) -> Result<Vec<Vec<Vec<S>>>> {
        let (dim1, dim2, dim3) = self.dims3()?;
        let from_cpu_storage = |cpu_storage: &crate::CpuStorage| {
            let data = S::cpu_storage_as_slice(cpu_storage)?;
            let mut top_rows = vec![];
            match self.layout.contiguous_offsets() {
                Some((o1, o2)) => {
                    let data = &data[o1..o2];
                    let dim23 = dim2 * dim3;
                    for idx1 in 0..dim1 {
                        let data = &data[idx1 * dim23..(idx1 + 1) * dim23];
                        let mut rows = vec![];
                        for idx2 in 0..dim2 {
                            rows.push(data[idx2 * dim3..(idx2 + 1) * dim3].to_vec())
                        }
                        top_rows.push(rows);
                    }
                }
                None => {
                    let mut src_index = self.strided_index();
                    for _idx in 0..dim1 {
                        let mut rows = vec![];
                        for _jdx in 0..dim2 {
                            let row = (0..dim3).map(|_| data[src_index.next().unwrap()]).collect();
                            rows.push(row)
                        }
                        top_rows.push(rows);
                    }
                    assert!(src_index.next().is_none());
                }
            }
            Ok(top_rows)
        };
        match &*self.storage() {
            Storage::Cpu(storage) => from_cpu_storage(storage),
            Storage::Cuda(storage) => from_cpu_storage(&storage.to_cpu_storage()?),
            Storage::Metal(storage) => from_cpu_storage(&storage.to_cpu_storage()?),
        }
    }

    /// The dtype for the elements stored in the input tensor.
    pub fn dtype(&self) -> DType {
        self.dtype
    }

    /// The device on which the input tensor is located.
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// The tensor shape, i.e. dimension sizes on each axis.
    pub fn shape(&self) -> &Shape {
        self.layout().shape()
    }

    /// The dimension size for this tensor on each axis.
    pub fn dims(&self) -> &[usize] {
        self.shape().dims()
    }

    /// The dimension size for a specified dimension index.
    pub fn dim<D: Dim>(&self, dim: D) -> Result<usize> {
        let dim = dim.to_index(self.shape(), "dim")?;
        Ok(self.dims()[dim])
    }

    /// The layout of the input tensor, this stores both the shape of the tensor as well as the
    /// strides and the start offset to apply to the underlying storage.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn stride(&self) -> &[usize] {
        self.layout.stride()
    }

    /// The number of dimensions for this tensor, 0 for a scalar tensor, 1 for a 1D tensor, etc.
    pub fn rank(&self) -> usize {
        self.shape().rank()
    }

    /// The number of elements stored in this tensor.
    pub fn elem_count(&self) -> usize {
        self.shape().elem_count()
    }

    /// The unique identifier for this tensor.
    pub fn id(&self) -> TensorId {
        self.id
    }

    /// Whether this tensor is a variable or not. A variable is a tensor for which gradient is
    /// tracked and on which backpropagation can be performed.
    pub fn is_variable(&self) -> bool {
        self.is_variable
    }

    pub(crate) fn op(&self) -> &Option<Op> {
        &self.op
    }

    /// Computes the max of all the elements in this tensor and returns a tensor holding this
    /// scalar with zero dimensions.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let tensor = Tensor::new(&[[0f32, 1.], [2., 3.], [4., 5.]], &Device::Cpu)?;
    /// let tensor = tensor.max_all()?;
    /// assert_eq!(tensor.to_scalar::<f32>()?, 5.);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn max_all(&self) -> Result<Tensor> {
        if self.rank() == 0 {
            Ok(self.clone())
        } else {
            self.flatten_all()?.max(0)
        }
    }

    /// Computes the min of all the elements in this tensor and returns a tensor holding this
    /// scalar with zero dimensions.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let tensor = Tensor::new(&[[0f32, 1.], [2., 3.], [4., 5.]], &Device::Cpu)?;
    /// let tensor = tensor.min_all()?;
    /// assert_eq!(tensor.to_scalar::<f32>()?, 0.);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn min_all(&self) -> Result<Tensor> {
        if self.rank() == 0 {
            Ok(self.clone())
        } else {
            self.flatten_all()?.min(0)
        }
    }

    /// Computes the sum of all the elements in this tensor and returns a tensor holding this
    /// scalar with zero dimensions.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let tensor = Tensor::new(&[[0f32, 1.], [2., 3.], [4., 5.]], &Device::Cpu)?;
    /// let tensor = tensor.sum_all()?;
    /// assert_eq!(tensor.to_scalar::<f32>()?, 15.);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn sum_all(&self) -> Result<Tensor> {
        let dims: Vec<_> = (0..self.rank()).collect();
        self.sum(dims)
    }

    pub fn mean_all(&self) -> Result<Tensor> {
        self.sum_all()? / self.elem_count() as f64
    }

    fn flatten_<D1: Dim, D2: Dim>(
        &self,
        start_dim: Option<D1>,
        end_dim: Option<D2>,
    ) -> Result<Tensor> {
        if self.rank() == 0 {
            self.reshape(1)
        } else {
            let start_dim = match start_dim {
                None => 0,
                Some(dim) => dim.to_index(self.shape(), "flatten")?,
            };
            let end_dim = match end_dim {
                None => self.rank() - 1,
                Some(dim) => dim.to_index(self.shape(), "flatten")?,
            };
            if start_dim < end_dim {
                let dims = self.dims();
                let mut dst_dims = dims[..start_dim].to_vec();
                dst_dims.push(dims[start_dim..end_dim + 1].iter().product::<usize>());
                if end_dim + 1 < dims.len() {
                    dst_dims.extend(&dims[end_dim + 1..]);
                }
                self.reshape(dst_dims)
            } else {
                Ok(self.clone())
            }
        }
    }

    /// Flattens the input tensor on the dimension indexes from `start_dim` to `end_dim` (both
    /// inclusive).
    pub fn flatten<D1: Dim, D2: Dim>(&self, start_dim: D1, end_dim: D2) -> Result<Tensor> {
        self.flatten_(Some(start_dim), Some(end_dim))
    }

    /// Flattens the input tensor on the dimension indexes from `0` to `end_dim` (inclusive).
    pub fn flatten_to<D: Dim>(&self, end_dim: D) -> Result<Tensor> {
        self.flatten_(None::<usize>, Some(end_dim))
    }

    /// Flattens the input tensor on the dimension indexes from `start_dim` (inclusive) to the last
    /// dimension.
    pub fn flatten_from<D: Dim>(&self, start_dim: D) -> Result<Tensor> {
        self.flatten_(Some(start_dim), None::<usize>)
    }

    /// Flattens the input tensor by reshaping it into a one dimension tensor.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let tensor = Tensor::new(&[[0f32, 1.], [2., 3.], [4., 5.]], &Device::Cpu)?;
    /// let tensor = tensor.flatten_all()?;
    /// assert_eq!(tensor.to_vec1::<f32>()?, &[0., 1., 2., 3., 4., 5.]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn flatten_all(&self) -> Result<Tensor> {
        self.flatten_(None::<usize>, None::<usize>)
    }

    /// Returns the sub-tensor fixing the index at `i` on the first dimension.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let tensor = Tensor::new(&[[0f32, 1.], [2., 3.], [4., 5.]], &Device::Cpu)?;
    /// let t = tensor.get(0)?;
    /// assert_eq!(t.to_vec1::<f32>()?, &[0., 1.]);
    /// let t = tensor.get(1)?;
    /// assert_eq!(t.to_vec1::<f32>()?, &[2., 3.]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn get(&self, i: usize) -> Result<Tensor> {
        let dims = self.dims();
        if dims.is_empty() {
            Ok(self.clone())
        } else {
            self.narrow(0, i, 1)?.reshape(&dims[1..])
        }
    }

    /// Returns the sub-tensor fixing the index at `index` on the dimension `dim`.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let tensor = Tensor::new(&[[0f32, 1.], [2., 3.], [4., 5.]], &Device::Cpu)?;
    /// let t = tensor.get_on_dim(1, 0)?;
    /// assert_eq!(t.to_vec1::<f32>()?, &[0., 2., 4.]);
    /// let t = tensor.get_on_dim(1, 1)?;
    /// assert_eq!(t.to_vec1::<f32>()?, &[1., 3., 5.]);
    /// let t = tensor.get_on_dim(0, 1)?;
    /// assert_eq!(t.to_vec1::<f32>()?, &[2., 3.]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn get_on_dim<D: Dim>(&self, dim: D, index: usize) -> Result<Tensor> {
        let dim = dim.to_index(self.shape(), "get_on_dim")?;
        self.narrow(dim, index, 1)?.squeeze(dim)
    }

    /// Returns a tensor that is a transposed version of the input, the two last dimensions of the
    /// input are swapped.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let tensor = Tensor::new(&[[0f32, 1.], [2., 3.], [4., 5.]], &Device::Cpu)?;
    /// let tensor = tensor.t()?;
    /// assert_eq!(tensor.to_vec2::<f32>()?, &[[0.0, 2.0, 4.0], [1.0, 3.0, 5.0]]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn t(&self) -> Result<Tensor> {
        let rank = self.rank();
        if rank < 2 {
            Err(Error::UnexpectedNumberOfDims {
                expected: 2,
                got: rank,
                shape: self.shape().clone(),
            }
            .bt())?
        }
        self.transpose(rank - 2, rank - 1)
    }

    /// Returns a tensor that is a transposed version of the input, the given dimensions are
    /// swapped.
    pub fn transpose<D1: Dim, D2: Dim>(&self, dim1: D1, dim2: D2) -> Result<Tensor> {
        let dim1 = dim1.to_index(self.shape(), "transpose")?;
        let dim2 = dim2.to_index(self.shape(), "transpose")?;
        if dim1 == dim2 {
            return Ok(self.clone());
        }
        let op = BackpropOp::new1(self, |t| Op::Transpose(t, dim1, dim2));
        let tensor_ = Tensor_ {
            id: TensorId::new(),
            storage: self.storage.clone(),
            layout: self.layout.transpose(dim1, dim2)?,
            op,
            is_variable: false,
            dtype: self.dtype,
            device: self.device.clone(),
        };
        Ok(Tensor(Arc::new(tensor_)))
    }

    /// Returns a tensor with the same data as the input where the dimensions have been permuted.
    /// dims must be a permutation, i.e. include each dimension index exactly once.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let tensor = Tensor::arange(0u32, 120u32, &Device::Cpu)?.reshape((2, 3, 4, 5))?;
    /// assert_eq!(tensor.dims(), &[2, 3, 4, 5]);
    /// let tensor = tensor.permute((2, 3, 1, 0))?;
    /// assert_eq!(tensor.dims(), &[4, 5, 3, 2]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn permute<D: Dims>(&self, dims: D) -> Result<Tensor> {
        let dims = dims.to_indexes(self.shape(), "permute")?;
        // O(n^2) permutation check but these arrays are small.
        let is_permutation =
            dims.len() == self.rank() && (0..dims.len()).all(|i| dims.contains(&i));
        if !is_permutation {
            bail!(
                "dimension mismatch in permute, tensor {:?}, dims: {:?}",
                self.dims(),
                dims
            )
        }
        let op = BackpropOp::new1(self, |t| Op::Permute(t, dims.clone()));
        let tensor_ = Tensor_ {
            id: TensorId::new(),
            storage: self.storage.clone(),
            layout: self.layout.permute(&dims)?,
            op,
            is_variable: false,
            dtype: self.dtype,
            device: self.device.clone(),
        };
        Ok(Tensor(Arc::new(tensor_)))
    }

    /// Returns true if the data is stored in a C contiguous (aka row major) way.
    pub fn is_contiguous(&self) -> bool {
        self.layout.is_contiguous()
    }

    /// Returns true if the data is stored in a Fortran contiguous (aka column major) way.
    pub fn is_fortran_contiguous(&self) -> bool {
        self.layout.is_fortran_contiguous()
    }

    /// Compared to clone, this copies the actual storage but may fail because of running out of
    /// memory.
    pub fn copy(&self) -> Result<Tensor> {
        let op = BackpropOp::new1(self, Op::Copy);
        let tensor_ = Tensor_ {
            id: TensorId::new(),
            storage: Arc::new(RwLock::new(self.storage().try_clone(self.layout())?)),
            layout: self.layout.clone(),
            op,
            is_variable: false,
            dtype: self.dtype,
            device: self.device.clone(),
        };
        Ok(Tensor(Arc::new(tensor_)))
    }

    /// Returns a new tensor detached from the current graph, gradient are not propagated through
    /// this new node. The storage of this tensor is shared with the initial tensor.
    ///
    /// If the tensor is already detached from the computation graph, the same tensor is returned.
    pub fn detach(&self) -> Tensor {
        if self.op.is_none() && !self.is_variable {
            self.clone()
        } else {
            let tensor_ = Tensor_ {
                id: TensorId::new(),
                storage: self.storage.clone(),
                layout: self.layout.clone(),
                op: BackpropOp::none(),
                is_variable: false,
                dtype: self.dtype,
                device: self.device.clone(),
            };
            Tensor(Arc::new(tensor_))
        }
    }

    /// If the target device is the same as the tensor device, only a shallow copy is performed.
    pub fn to_device(&self, device: &Device) -> Result<Tensor> {
        if self.device().same_device(device) {
            Ok(self.clone())
        } else {
            let storage = match (&*self.storage(), device) {
                (Storage::Cpu(storage), Device::Cuda(cuda)) => {
                    Storage::Cuda(cuda.storage_from_cpu_storage(storage)?)
                }
                (Storage::Cpu(storage), Device::Metal(metal)) => {
                    Storage::Metal(metal.storage_from_cpu_storage(storage)?)
                }
                (Storage::Cuda(storage), Device::Cpu) => Storage::Cpu(storage.to_cpu_storage()?),
                (Storage::Metal(storage), Device::Cpu) => Storage::Cpu(storage.to_cpu_storage()?),
                (Storage::Cuda(storage), Device::Cuda(cuda)) => {
                    // can't clone storage if it's the same device because of the underlying device ptr
                    let dst_storage = storage.transfer_to_device(cuda)?;
                    Storage::Cuda(dst_storage)
                }
                (Storage::Cpu(storage), Device::Cpu) => Storage::Cpu(storage.clone()),
                _ => {
                    bail!(
                        "not implemented yet, self.device: {:?}, device: {:?}",
                        self.device(),
                        device
                    )
                }
            };
            let op = BackpropOp::new1(self, Op::ToDevice);
            let tensor_ = Tensor_ {
                id: TensorId::new(),
                storage: Arc::new(RwLock::new(storage)),
                layout: self.layout.clone(),
                op,
                is_variable: false,
                dtype: self.dtype,
                device: device.clone(),
            };
            Ok(Tensor(Arc::new(tensor_)))
        }
    }

    /// Returns a new tensor duplicating data from the original tensor. New dimensions are inserted
    /// on the left.
    pub fn broadcast_left<S: Into<Shape>>(&self, left_shape: S) -> Result<Self> {
        let left_shape = left_shape.into();
        let mut dims = left_shape.into_dims();
        dims.extend(self.dims());
        self.broadcast_as(dims)
    }

    /// Broadcast the input tensor to the target shape. This returns an error if the input shape is
    /// not compatible with the target shape.
    ///
    /// If the input shape is `i_1, i_2, ... i_k`, the target shape has to have `k` dimensions or
    /// more and shape `j_1, ..., j_l, t_1, t_2, ..., t_k`. The dimensions `j_1` to `j_l` can have
    /// any value, the dimension `t_a` must be equal to `i_a` if `i_a` is different from 1. If
    /// `i_a` is equal to 1, any value can be used.
    pub fn broadcast_as<S: Into<Shape>>(&self, shape: S) -> Result<Self> {
        let tensor_ = Tensor_ {
            id: TensorId::new(),
            storage: self.storage.clone(),
            layout: self.layout.broadcast_as(shape)?,
            op: BackpropOp::new1(self, Op::Broadcast),
            is_variable: false,
            dtype: self.dtype,
            device: self.device.clone(),
        };
        Ok(Tensor(Arc::new(tensor_)))
    }

    /// An alias for broadcast_as.
    pub fn expand<S: Into<Shape>>(&self, shape: S) -> Result<Self> {
        self.broadcast_as(shape)
    }

    /// Casts the input tensor to the target `dtype`.
    ///
    /// ```rust
    /// use candle_core::{Tensor, Device};
    /// let tensor = Tensor::new(3.14159265358979f64, &Device::Cpu)?;
    /// assert_eq!(tensor.to_scalar::<f64>()?, 3.14159265358979);
    /// let tensor = tensor.to_dtype(candle_core::DType::F32)?;
    /// assert_eq!(tensor.to_scalar::<f32>()?, 3.1415927);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn to_dtype(&self, dtype: DType) -> Result<Self> {
        if self.dtype() == dtype {
            Ok(self.clone())
        } else {
            let shape = self.shape();
            let storage = self.storage().to_dtype(self.layout(), dtype)?;
            let op = BackpropOp::new1(self, Op::ToDType);
            Ok(from_storage(storage, shape.clone(), op, false))
        }
    }

    /// Returns a tensor that is in row major order. This is the same as the original tensor if it
    /// was already contiguous, otherwise a copy is triggered.
    pub fn contiguous(&self) -> Result<Tensor> {
        if self.is_contiguous() {
            Ok(self.clone())
        } else {
            let shape = self.shape();
            let mut storage = unsafe { self.device().alloc_uninit(shape, self.dtype())? };
            self.storage()
                .copy_strided_src(&mut storage, 0, self.layout())?;
            let op = BackpropOp::new1(self, Op::Copy);
            Ok(from_storage(storage, shape.clone(), op, false))
        }
    }

    /// Returns a tensor that is in row major order. This always makes a copy.
    pub fn force_contiguous(&self) -> Result<Tensor> {
        let shape = self.shape();
        let mut storage = unsafe { self.device().alloc_uninit(shape, self.dtype())? };
        self.storage()
            .copy_strided_src(&mut storage, 0, self.layout())?;
        let op = BackpropOp::new1(self, Op::Copy);
        Ok(from_storage(storage, shape.clone(), op, false))
    }

    /// Create a variable based on the values currently stored in a tensor. The storage is always
    /// copied.
    pub(crate) fn make_var(&self) -> Result<Tensor> {
        let shape = self.shape().clone();
        let mut storage = unsafe { self.device().alloc_uninit(&shape, self.dtype())? };
        self.storage()
            .copy_strided_src(&mut storage, 0, self.layout())?;
        Ok(from_storage(storage, shape, BackpropOp::none(), true))
    }

    /// Reshape returns a tensor with the target shape provided that the number of elements of the
    /// original tensor is the same.
    /// If the input tensor is contiguous, this is a view on the original data. Otherwise this uses
    /// a new storage and copies the data over, the returned tensor is always contiguous.
    ///
    /// The shape can be specified using a tuple of `usize` and at most one `()` in which case
    /// the behavior is the same as when using `-1` in PyTorch: this dimension size is adjusted so
    /// as to match the number of elements in the tensor.
    ///
    /// ```rust
    /// # use candle_core::{Tensor, DType, Device, D};
    /// let a = Tensor::zeros((2, 3), DType::F32, &Device::Cpu)?;
    ///
    /// let c = a.reshape((1, 6))?;
    /// assert_eq!(c.shape().dims(), &[1, 6]);
    ///
    /// let c = a.reshape((3, 2))?;
    /// assert_eq!(c.shape().dims(), &[3, 2]);
    ///
    /// let c = a.reshape((2, (), 1))?;
    /// assert_eq!(c.shape().dims(), &[2, 3, 1]);
    ///
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn reshape<S: ShapeWithOneHole>(&self, s: S) -> Result<Tensor> {
        let shape = s.into_shape(self.elem_count())?;
        if shape.elem_count() != self.elem_count() {
            return Err(Error::ShapeMismatchBinaryOp {
                lhs: self.shape().clone(),
                rhs: shape,
                op: "reshape",
            }
            .bt());
        }
        let op = BackpropOp::new1(self, Op::Reshape);
        if self.is_contiguous() {
            let tensor_ = Tensor_ {
                id: TensorId::new(),
                storage: self.storage.clone(),
                layout: Layout::contiguous_with_offset(shape, self.layout.start_offset()),
                op,
                is_variable: false,
                dtype: self.dtype,
                device: self.device.clone(),
            };
            Ok(Tensor(Arc::new(tensor_)))
        } else {
            let mut storage = unsafe { self.device().alloc_uninit(&shape, self.dtype())? };
            self.storage()
                .copy_strided_src(&mut storage, 0, self.layout())?;
            Ok(from_storage(storage, shape, op, false))
        }
    }

    /// Creates a new tensor with the specified dimension removed if its size was one.
    ///
    /// ```rust
    /// # use candle_core::{Tensor, DType, Device, D};
    /// let a = Tensor::zeros((2, 3, 1), DType::F32, &Device::Cpu)?;
    ///
    /// let c = a.squeeze(2)?;
    /// assert_eq!(c.shape().dims(), &[2, 3]);
    ///
    /// let c = a.squeeze(D::Minus1)?;
    /// assert_eq!(c.shape().dims(), &[2, 3]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn squeeze<D: Dim>(&self, dim: D) -> Result<Self> {
        // The PyTorch semantics are to return the same tensor if the target dimension
        // does not have a size of 1.
        let dims = self.dims();
        let dim = dim.to_index(self.shape(), "squeeze")?;
        if dims[dim] == 1 {
            let mut dims = dims.to_vec();
            let mut strides = self.stride().to_vec();
            dims.remove(dim);
            strides.remove(dim);
            let tensor_ = Tensor_ {
                id: TensorId::new(),
                storage: self.storage.clone(),
                layout: Layout::new(dims.into(), strides, self.layout.start_offset()),
                op: BackpropOp::new1(self, Op::Reshape),
                is_variable: false,
                dtype: self.dtype,
                device: self.device.clone(),
            };
            Ok(Tensor(Arc::new(tensor_)))
        } else {
            Ok(self.clone())
        }
    }

    /// Creates a new tensor with a dimension of size one inserted at the specified position.
    ///
    /// ```rust
    /// # use candle_core::{Tensor, DType, Device, D};
    /// let a = Tensor::zeros((2, 3), DType::F32, &Device::Cpu)?;
    ///
    /// let c = a.unsqueeze(0)?;
    /// assert_eq!(c.shape().dims(), &[1, 2, 3]);
    ///
    /// let c = a.unsqueeze(D::Minus1)?;
    /// assert_eq!(c.shape().dims(), &[2, 3, 1]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn unsqueeze<D: Dim>(&self, dim: D) -> Result<Self> {
        let mut dims = self.dims().to_vec();
        let mut strides = self.stride().to_vec();
        let dim = dim.to_index_plus_one(self.shape(), "unsqueeze")?;
        // Cannot panic because to_index_plus_one already checks dimensions
        dims.insert(dim, 1);
        // Any stride would work here, but we pick one so as to maximize the probability to remain
        // C contiguous.
        let stride = if dim < strides.len() { strides[dim] } else { 1 };
        strides.insert(dim, stride);
        let tensor_ = Tensor_ {
            id: TensorId::new(),
            storage: self.storage.clone(),
            layout: Layout::new(dims.into(), strides, self.layout.start_offset()),
            op: BackpropOp::new1(self, Op::Reshape),
            is_variable: false,
            dtype: self.dtype,
            device: self.device.clone(),
        };
        Ok(Tensor(Arc::new(tensor_)))
    }

    /// Stacks two or more tensors along a particular dimension.
    ///
    /// All tensors must have the same rank, and the output has one additional rank
    ///
    /// ```rust
    /// # use candle_core::{Tensor, DType, Device};
    /// let a = Tensor::zeros((2, 3), DType::F32, &Device::Cpu)?;
    /// let b = Tensor::zeros((2, 3), DType::F32, &Device::Cpu)?;
    ///
    /// let c = Tensor::stack(&[&a, &b], 0)?;
    /// assert_eq!(c.shape().dims(), &[2, 2, 3]);
    ///
    /// let c = Tensor::stack(&[&a, &b], 2)?;
    /// assert_eq!(c.shape().dims(), &[2, 3, 2]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn stack<A: AsRef<Tensor>, D: Dim>(args: &[A], dim: D) -> Result<Self> {
        if args.is_empty() {
            Err(Error::OpRequiresAtLeastOneTensor { op: "stack" }.bt())?
        }
        let dim = dim.to_index_plus_one(args[0].as_ref().shape(), "stack")?;
        let args = args
            .iter()
            .map(|t| t.as_ref().unsqueeze(dim))
            .collect::<Result<Vec<_>>>()?;
        Self::cat(&args, dim)
    }

    /// Pad the input tensor using 0s along dimension `dim`. This adds `left` elements before the
    /// input tensor values and `right` elements after.
    pub fn pad_with_zeros<D: Dim>(&self, dim: D, left: usize, right: usize) -> Result<Self> {
        if left == 0 && right == 0 {
            Ok(self.clone())
        } else if left == 0 {
            let dim = dim.to_index(self.shape(), "pad_with_zeros")?;
            let mut dims = self.dims().to_vec();
            dims[dim] = right;
            let right = Tensor::zeros(dims.as_slice(), self.dtype, self.device())?;
            Tensor::cat(&[self, &right], dim)
        } else if right == 0 {
            let dim = dim.to_index(self.shape(), "pad_with_zeros")?;
            let mut dims = self.dims().to_vec();
            dims[dim] = left;
            let left = Tensor::zeros(dims.as_slice(), self.dtype, self.device())?;
            Tensor::cat(&[&left, self], dim)
        } else {
            let dim = dim.to_index(self.shape(), "pad_with_zeros")?;
            let mut dims = self.dims().to_vec();
            dims[dim] = left;
            let left = Tensor::zeros(dims.as_slice(), self.dtype, self.device())?;
            dims[dim] = right;
            let right = Tensor::zeros(dims.as_slice(), self.dtype, self.device())?;
            Tensor::cat(&[&left, self, &right], dim)
        }
    }

    /// Pad the input tensor using same values along dimension `dim`. This adds `left` elements before the
    /// input tensor values and `right` elements after.
    pub fn pad_with_same<D: Dim>(&self, dim: D, left: usize, right: usize) -> Result<Self> {
        if left == 0 && right == 0 {
            Ok(self.clone())
        } else if self.elem_count() == 0 {
            bail!("cannot use pad_with_same on an empty tensor")
        } else if left == 0 {
            let dim = dim.to_index(self.shape(), "pad_with_same")?;
            let r = self.narrow(dim, self.dim(dim)? - 1, 1)?;
            let mut v = vec![self];
            for _ in 0..right {
                v.push(&r)
            }
            Tensor::cat(&v, dim)
        } else if right == 0 {
            let dim = dim.to_index(self.shape(), "pad_with_same")?;
            let l = self.narrow(dim, 0, 1)?;
            let mut v = vec![];
            for _ in 0..left {
                v.push(&l)
            }
            v.push(self);
            Tensor::cat(&v, dim)
        } else {
            let dim = dim.to_index(self.shape(), "pad_with_same")?;
            let l = self.narrow(dim, 0, 1)?;
            let r = self.narrow(dim, self.dim(dim)? - 1, 1)?;
            let mut v = vec![];
            for _ in 0..left {
                v.push(&l)
            }
            v.push(self);
            for _ in 0..right {
                v.push(&r)
            }
            Tensor::cat(&v, dim)
        }
    }

    /// Run the `forward` method of `m` on `self`.
    pub fn apply<M: crate::Module>(&self, m: &M) -> Result<Self> {
        m.forward(self)
    }

    /// Run the `forward` method of `m` on `self`.
    pub fn apply_t<M: crate::ModuleT>(&self, m: &M, train: bool) -> Result<Self> {
        m.forward_t(self, train)
    }

    pub(crate) fn storage(&self) -> std::sync::RwLockReadGuard<'_, Storage> {
        self.storage.read().unwrap()
    }

    pub(crate) fn storage_mut(&self) -> std::sync::RwLockWriteGuard<'_, Storage> {
        self.storage.write().unwrap()
    }

    // If we extend the visibility of this function to be usable outside of this crate, we should
    // make it unsafe.
    pub(crate) fn storage_mut_and_layout(
        &self,
    ) -> (std::sync::RwLockWriteGuard<'_, Storage>, &Layout) {
        let storage = self.storage.write().unwrap();
        (storage, &self.layout)
    }

    /// The storage used by this tensor, together with the layout to use to access it safely.
    pub fn storage_and_layout(&self) -> (std::sync::RwLockReadGuard<'_, Storage>, &Layout) {
        let storage = self.storage.read().unwrap();
        (storage, &self.layout)
    }

    pub(crate) fn same_storage(&self, rhs: &Self) -> bool {
        let lhs: &RwLock<Storage> = self.storage.as_ref();
        let rhs: &RwLock<Storage> = rhs.storage.as_ref();
        std::ptr::eq(lhs, rhs)
    }

    /// Normalize a 'relative' axis value: positive values are kept, negative
    /// values means counting the dimensions from the back.
    pub fn normalize_axis(&self, axis: i64) -> Result<usize> {
        let rank = self.rank() as i64;
        if rank <= axis {
            bail!("axis {axis} is too large, tensor rank {rank}")
        } else if 0 <= axis {
            Ok(axis as usize)
        } else {
            let naxis = rank + axis;
            if naxis < 0 {
                bail!("axis {axis} is too small, tensor rank {rank}")
            }
            Ok(naxis as usize)
        }
    }

    /// Returns a lower triangular matrix of ones of size n by n.
    pub fn tril2(n: usize, dtype: DType, device: &Device) -> Result<Self> {
        let t = Tensor::arange(0u32, n as u32, device)?;
        let t1 = t.reshape((1, n))?.broadcast_as((n, n))?;
        let t2 = t.reshape((n, 1))?.broadcast_as((n, n))?;
        t1.le(&t2)?.to_dtype(dtype)
    }

    /// Returns an upper triangular matrix of ones of size n by n.
    pub fn triu2(n: usize, dtype: DType, device: &Device) -> Result<Self> {
        let t = Tensor::arange(0u32, n as u32, device)?;
        let t1 = t.reshape((1, n))?.broadcast_as((n, n))?;
        let t2 = t.reshape((n, 1))?.broadcast_as((n, n))?;
        t1.ge(&t2)?.to_dtype(dtype)
    }

    /// Returns a matrix with a diagonal of ones of size n by n.
    pub fn eye(n: usize, dtype: DType, device: &Device) -> Result<Self> {
        let t = Tensor::arange(0u32, n as u32, device)?;
        let t1 = t.reshape((1, n))?.broadcast_as((n, n))?;
        let t2 = t.reshape((n, 1))?.broadcast_as((n, n))?;
        t1.eq(&t2)?.to_dtype(dtype)
    }

    /// Returns the cumulative sum of elements of the input tensor summed over the specified
    /// dimension.
    ///
    /// This operation is most efficient when dim is the last dimension of the tensor.
    pub fn cumsum<D: Dim>(&self, dim: D) -> Result<Self> {
        let dim = dim.to_index(self.shape(), "cumsum")?;
        let rank = self.rank();
        if rank == 0 {
            return Ok(self.clone());
        }
        let n_axis = self.dim(dim)?;
        let triu = Tensor::triu2(n_axis, self.dtype(), self.device())?;
        if rank == 1 {
            self.unsqueeze(0)?.matmul(&triu)?.squeeze(0)
        } else {
            let last = rank - 1;
            let t = self.transpose(dim, last)?;
            let t = t.broadcast_matmul(&triu)?;
            t.transpose(dim, last)
        }
    }

    /// Returns a copy of `self` where the values within `ranges` have been replaced with the
    /// content of `src`.
    pub fn slice_assign<D: std::ops::RangeBounds<usize>>(
        &self,
        ranges: &[D],
        src: &Tensor,
    ) -> Result<Self> {
        let src_dims = src.dims();
        let self_dims = self.dims();
        if self_dims.len() != src_dims.len() {
            bail!(
                "slice-assign requires input with the same rank {} <> {}",
                self_dims.len(),
                src_dims.len()
            )
        }
        if self_dims.len() != ranges.len() {
            bail!(
                "slice-assign requires input with the same rank as there are ranges {} <> {}",
                self_dims.len(),
                ranges.len()
            )
        }
        let mut src = src.clone();
        let mut mask = Self::ones(src.shape(), DType::U8, src.device())?;
        for (i, range) in ranges.iter().enumerate() {
            let start_included = match range.start_bound() {
                std::ops::Bound::Unbounded => 0,
                std::ops::Bound::Included(v) => *v,
                std::ops::Bound::Excluded(v) => *v + 1,
            };
            let end_excluded = match range.end_bound() {
                std::ops::Bound::Unbounded => self_dims[i],
                std::ops::Bound::Included(v) => *v + 1,
                std::ops::Bound::Excluded(v) => *v,
            };
            if end_excluded <= start_included {
                bail!("slice-assign: empty range for dim {i}, {start_included} {end_excluded}")
            }
            if self_dims[i] < end_excluded {
                bail!(
                    "slice-assign: upper bound is out of range for dim {i}, {end_excluded} {}",
                    self_dims[i]
                )
            }
            if end_excluded - start_included != src_dims[i] {
                bail!(
                    "slice-assign: the range for dim {i} ({start_included}..{end_excluded}) does not match the size of src {}", src_dims[i]
                )
            }
            src = src.pad_with_zeros(i, start_included, self_dims[i] - end_excluded)?;
            mask = mask.pad_with_zeros(i, start_included, self_dims[i] - end_excluded)?
        }
        mask.where_cond(/* on_true= */ &src, /* on_false= */ self)
    }

    /// Returns log(sum(exp(tensor), dim)).
    pub fn log_sum_exp<D: Dims>(&self, sum_dims: D) -> Result<Self> {
        let sum_dims = sum_dims.to_indexes(self.shape(), "log-sum-exp")?;
        if sum_dims.is_empty() {
            return Ok(self.clone());
        }
        let max = sum_dims[1..]
            .iter()
            .try_fold(self.max_keepdim(sum_dims[0])?, |max, &dim| {
                max.max_keepdim(dim)
            })?;
        let exp = self.broadcast_sub(&max)?.exp()?;
        let sum = exp.sum(sum_dims.clone())?;

        sum.log()? + max.squeeze_dims(&sum_dims)
    }

    /// Pointwise pow operation.
    pub fn pow(&self, rhs: &Tensor) -> Result<Self> {
        rhs.mul(&self.log()?)?.exp()
    }

    /// Broadcasting version of `pow`.
    pub fn broadcast_pow(&self, rhs: &Tensor) -> Result<Self> {
        rhs.broadcast_mul(&self.log()?)?.exp()
    }

    /// Returns a new tensor with the order of elements reversed along the specified dimensions.
    /// This function makes a copy of the tensor’s data.
    ///
    /// ```rust
    /// # use candle_core::{Tensor, Device};
    /// let t = Tensor::arange(0., 6., &Device::Cpu)?.reshape((2, 3))?;
    /// assert_eq!(t.to_vec2::<f64>()?, &[[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]]);
    /// let t_flipped = t.flip(&[0])?;
    /// assert_eq!(t_flipped.to_vec2::<f64>()?, &[[3.0, 4.0, 5.0], [0.0, 1.0, 2.0]]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    pub fn flip(&self, dims: &[usize]) -> Result<Tensor> {
        let mut result = self.clone();
        for &dim in dims.iter() {
            let size = result.dim(dim)?;
            let indices: Vec<i64> = (0..size).rev().map(|x| x as i64).collect();
            let indices_tensor = Tensor::from_vec(indices, (size,), result.device())?;
            result = result.index_select(&indices_tensor, dim)?;
        }
        Ok(result)
    }

    /// Returns a view of which contains all slices of size `size` from self tensor in the dimension
    /// `dim` and stepped by `step`.
    pub fn unfold<D: Dim>(&self, dim: D, size: usize, step: usize) -> Result<Self> {
        // https://github.com/pytorch/pytorch/blob/75b0720a97ac5d82e8a7a1a6ae7c5f7a87d7183d/aten/src/ATen/native/TensorShape.cpp#L3785-L3804
        let mut sizes = self.dims().to_vec();
        let mut strides = self.stride().to_vec();

        let dim = dim.to_index(self.shape(), "unfold")?;

        let max_len = if self.dims().is_empty() {
            1
        } else {
            sizes[dim]
        };
        if size > max_len {
            bail!(
                "unsqueeze: maximum size for tensor at dimension {dim} is {max_len} but size is {size}"
            )
        }
        sizes.push(size);
        strides.push(if self.dims().is_empty() {
            1
        } else {
            strides[dim]
        });

        if !self.dims().is_empty() {
            sizes[dim] = ((sizes[dim] as f32 - size as f32) / step as f32 + 1.) as usize;
            strides[dim] *= step;
        }

        let tensor_ = Tensor_ {
            id: TensorId::new(),
            storage: self.storage.clone(),
            layout: Layout::new(sizes.into(), strides, self.layout.start_offset()),
            op: BackpropOp::new1(self, Op::Reshape),
            is_variable: false,
            dtype: self.dtype,
            device: self.device.clone(),
        };
        Ok(Tensor(Arc::new(tensor_)))
    }
}

macro_rules! bin_trait {
    ($trait:ident, $fn1:ident, $mul:expr, $add:expr) => {
        impl<B: std::borrow::Borrow<Tensor>> std::ops::$trait<B> for Tensor {
            type Output = Result<Tensor>;

            fn $fn1(self, rhs: B) -> Self::Output {
                Tensor::$fn1(&self, rhs.borrow())
            }
        }

        impl<B: std::borrow::Borrow<Tensor>> std::ops::$trait<B> for &Tensor {
            type Output = Result<Tensor>;

            fn $fn1(self, rhs: B) -> Self::Output {
                Tensor::$fn1(&self, rhs.borrow())
            }
        }

        impl<B: std::borrow::Borrow<Tensor>> std::ops::$trait<Tensor> for Result<B> {
            type Output = Result<Tensor>;

            fn $fn1(self, rhs: Tensor) -> Self::Output {
                Tensor::$fn1(self?.borrow(), &rhs)
            }
        }

        impl<B: std::borrow::Borrow<Tensor>> std::ops::$trait<&Tensor> for Result<B> {
            type Output = Result<Tensor>;

            fn $fn1(self, rhs: &Tensor) -> Self::Output {
                Tensor::$fn1(self?.borrow(), rhs)
            }
        }

        impl<B: std::borrow::Borrow<Tensor>> std::ops::$trait<Result<B>> for Tensor {
            type Output = Result<Tensor>;

            fn $fn1(self, rhs: Result<B>) -> Self::Output {
                Tensor::$fn1(&self, rhs?.borrow())
            }
        }

        impl<B: std::borrow::Borrow<Tensor>> std::ops::$trait<Result<B>> for &Tensor {
            type Output = Result<Tensor>;

            fn $fn1(self, rhs: Result<B>) -> Self::Output {
                Tensor::$fn1(&self, rhs?.borrow())
            }
        }

        impl std::ops::$trait<f64> for Tensor {
            type Output = Result<Tensor>;

            fn $fn1(self, rhs: f64) -> Self::Output {
                self.affine($mul(rhs), $add(rhs))
            }
        }

        impl std::ops::$trait<f64> for &Tensor {
            type Output = Result<Tensor>;

            fn $fn1(self, rhs: f64) -> Self::Output {
                self.affine($mul(rhs), $add(rhs))
            }
        }
    };
}

bin_trait!(Add, add, |_| 1., |v| v);
bin_trait!(Sub, sub, |_| 1., |v: f64| -v);
bin_trait!(Mul, mul, |v| v, |_| 0.);
bin_trait!(Div, div, |v| 1. / v, |_| 0.);

impl std::ops::Add<Tensor> for f64 {
    type Output = Result<Tensor>;

    fn add(self, rhs: Tensor) -> Self::Output {
        rhs + self
    }
}

impl std::ops::Add<&Tensor> for f64 {
    type Output = Result<Tensor>;

    fn add(self, rhs: &Tensor) -> Self::Output {
        rhs + self
    }
}

impl std::ops::Mul<Tensor> for f64 {
    type Output = Result<Tensor>;

    fn mul(self, rhs: Tensor) -> Self::Output {
        rhs * self
    }
}

impl std::ops::Mul<&Tensor> for f64 {
    type Output = Result<Tensor>;

    fn mul(self, rhs: &Tensor) -> Self::Output {
        rhs * self
    }
}

impl std::ops::Sub<Tensor> for f64 {
    type Output = Result<Tensor>;

    fn sub(self, rhs: Tensor) -> Self::Output {
        rhs.affine(-1., self)
    }
}

impl std::ops::Sub<&Tensor> for f64 {
    type Output = Result<Tensor>;

    fn sub(self, rhs: &Tensor) -> Self::Output {
        rhs.affine(-1., self)
    }
}

impl std::ops::Div<Tensor> for f64 {
    type Output = Result<Tensor>;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Tensor) -> Self::Output {
        rhs.recip()? * self
    }
}

impl std::ops::Div<&Tensor> for f64 {
    type Output = Result<Tensor>;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: &Tensor) -> Self::Output {
        rhs.recip()? * self
    }
}

impl<S: Into<Shape>> From<(Storage, S)> for Tensor {
    fn from((storage, shape): (Storage, S)) -> Self {
        from_storage(storage, shape, BackpropOp::none(), false)
    }
}
