//! StreamTensror useful for streaming ops.
//!
use crate::{Result, Shape, Tensor};

pub trait Dim: crate::shape::Dim + Copy {}
impl<T: crate::shape::Dim + Copy> Dim for T {}

/// A stream tensor is used in streaming module. It can either contain an actual tensor or be
/// empty.
#[derive(Clone)]
pub struct StreamTensor(Option<Tensor>);

impl std::fmt::Debug for StreamTensor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(t) => write!(f, "{:?}", t.shape()),
            None => write!(f, "Empty"),
        }
    }
}

impl std::convert::From<Option<Tensor>> for StreamTensor {
    fn from(value: Option<Tensor>) -> Self {
        Self(value)
    }
}

impl std::convert::From<Tensor> for StreamTensor {
    fn from(value: Tensor) -> Self {
        Self(Some(value))
    }
}

impl std::convert::From<()> for StreamTensor {
    fn from(_value: ()) -> Self {
        Self(None)
    }
}

impl StreamTensor {
    pub fn empty() -> Self {
        Self(None)
    }

    pub fn from_tensor(tensor: Tensor) -> Self {
        Self(Some(tensor))
    }

    pub fn shape(&self) -> Option<&Shape> {
        self.0.as_ref().map(|t| t.shape())
    }

    pub fn cat2<D: Dim>(&self, rhs: &Self, dim: D) -> Result<Self> {
        let xs = match (&self.0, &rhs.0) {
            (Some(lhs), Some(rhs)) => {
                let xs = Tensor::cat(&[lhs, rhs], dim)?;
                Some(xs)
            }
            (Some(xs), None) | (None, Some(xs)) => Some(xs.clone()),
            (None, None) => None,
        };
        Ok(Self(xs))
    }

    pub fn seq_len<D: Dim>(&self, dim: D) -> Result<usize> {
        match &self.0 {
            None => Ok(0),
            Some(v) => v.dim(dim),
        }
    }

    pub fn reset(&mut self) {
        self.0 = None
    }

    pub fn narrow<D: Dim>(&self, dim: D, offset: usize, len: usize) -> Result<StreamTensor> {
        let t = match &self.0 {
            None => None,
            Some(t) => {
                let seq_len = t.dim(dim)?;
                if seq_len <= offset {
                    None
                } else {
                    let t = t.narrow(dim, offset, usize::min(len, seq_len - offset))?;
                    Some(t)
                }
            }
        };
        Ok(Self(t))
    }

    /// Splits the Streaming Tensor on the time axis `dim` with the first `lhs_len` elements
    /// returned in the first output and the remaining in the second output.
    pub fn split<D: Dim>(&self, dim: D, lhs_len: usize) -> Result<(Self, Self)> {
        match &self.0 {
            None => Ok((Self::empty(), Self::empty())),
            Some(t) => {
                let seq_len = t.dim(dim)?;
                let lhs_len = usize::min(seq_len, lhs_len);
                if lhs_len == 0 {
                    Ok((Self::empty(), t.clone().into()))
                } else {
                    let lhs = Self::from_tensor(t.narrow(dim, 0, lhs_len)?);
                    let rhs_len = seq_len - lhs_len;
                    let rhs = if rhs_len == 0 {
                        Self::empty()
                    } else {
                        Self::from_tensor(t.narrow(dim, lhs_len, rhs_len)?)
                    };
                    Ok((lhs, rhs))
                }
            }
        }
    }

    pub fn as_option(&self) -> Option<&Tensor> {
        self.0.as_ref()
    }

    pub fn apply<M: crate::Module>(&self, m: &M) -> Result<Self> {
        match &self.0 {
            None => Ok(Self::empty()),
            Some(t) => Ok(Self::from_tensor(t.apply(m)?)),
        }
    }
}

/// Streaming modules take as input a stream tensor and return a stream tensor. They may perform
/// some internal buffering so that enough data has been received for the module to be able to
/// perform some operations.
pub trait StreamingModule {
    // TODO: Should we also have a flush method?
    fn step(&mut self, xs: &StreamTensor) -> Result<StreamTensor>;
    fn reset_state(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Mul,
    Sub,
    Div,
}

#[derive(Debug, Clone)]
pub struct StreamingBinOp {
    prev_lhs: StreamTensor,
    prev_rhs: StreamTensor,
    pub op: BinOp,
    pub dim: crate::D,
}

impl StreamingBinOp {
    pub fn new(op: BinOp, dim: crate::D) -> Self {
        Self {
            prev_lhs: StreamTensor::empty(),
            prev_rhs: StreamTensor::empty(),
            op,
            dim,
        }
    }

    pub fn reset_state(&mut self) {
        self.prev_lhs.reset();
        self.prev_rhs.reset();
    }

    pub fn forward(&self, lhs: &Tensor, rhs: &Tensor) -> Result<Tensor> {
        match self.op {
            BinOp::Add => Tensor::add(lhs, rhs),
            BinOp::Mul => Tensor::mul(lhs, rhs),
            BinOp::Sub => Tensor::sub(lhs, rhs),
            BinOp::Div => Tensor::div(lhs, rhs),
        }
    }

    pub fn step(&mut self, lhs: &StreamTensor, rhs: &StreamTensor) -> Result<StreamTensor> {
        let lhs = StreamTensor::cat2(&self.prev_lhs, lhs, self.dim)?;
        let rhs = StreamTensor::cat2(&self.prev_rhs, rhs, self.dim)?;
        let lhs_len = lhs.seq_len(self.dim)?;
        let rhs_len = rhs.seq_len(self.dim)?;
        let common_len = usize::min(lhs_len, rhs_len);
        let (lhs, prev_lhs) = lhs.split(self.dim, common_len)?;
        let (rhs, prev_rhs) = rhs.split(self.dim, common_len)?;
        let ys = match (lhs.0, rhs.0) {
            (Some(lhs), Some(rhs)) => {
                let ys = self.forward(&lhs, &rhs)?;
                StreamTensor::from_tensor(ys)
            }
            (None, None) => StreamTensor::empty(),
            (lhs, rhs) => crate::bail!("INTERNAL ERROR inconsistent lhs and rhs {lhs:?} {rhs:?}"),
        };
        self.prev_lhs = prev_lhs;
        self.prev_rhs = prev_rhs;
        Ok(ys)
    }
}

/// Simple wrapper that doesn't do any buffering.
pub struct Map<T: crate::Module>(T);

impl<T: crate::Module> StreamingModule for Map<T> {
    fn reset_state(&mut self) {}

    fn step(&mut self, xs: &StreamTensor) -> Result<StreamTensor> {
        xs.apply(&self.0)
    }
}
