use crate::{Error, Tensor};
use std::ops::{
    Bound, Range, RangeBounds, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive,
};

impl Tensor {
    /// Intended to be use by the trait `.i()`
    ///
    /// ```
    /// # use candle_core::{Tensor, DType, Device, IndexOp};
    /// let a = Tensor::zeros((2, 3), DType::F32, &Device::Cpu)?;
    ///
    /// let c = a.i(0..1)?;
    /// assert_eq!(c.shape().dims(), &[1, 3]);
    ///
    /// let c = a.i(0)?;
    /// assert_eq!(c.shape().dims(), &[3]);
    ///
    /// let c = a.i((.., ..2) )?;
    /// assert_eq!(c.shape().dims(), &[2, 2]);
    ///
    /// let c = a.i((.., ..=2))?;
    /// assert_eq!(c.shape().dims(), &[2, 3]);
    ///
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    fn index(&self, indexers: &[TensorIndexer]) -> Result<Self, Error> {
        let mut x = self.clone();
        let dims = self.shape().dims();
        let mut current_dim = 0;
        for (i, indexer) in indexers.iter().enumerate() {
            x = match indexer {
                TensorIndexer::Select(n) => x.narrow(current_dim, *n, 1)?.squeeze(current_dim)?,
                TensorIndexer::Narrow(left_bound, right_bound) => {
                    let start = match left_bound {
                        Bound::Included(n) => *n,
                        Bound::Excluded(n) => *n + 1,
                        Bound::Unbounded => 0,
                    };
                    let stop = match right_bound {
                        Bound::Included(n) => *n + 1,
                        Bound::Excluded(n) => *n,
                        Bound::Unbounded => dims[i],
                    };
                    let out = x.narrow(current_dim, start, stop.saturating_sub(start))?;
                    current_dim += 1;
                    out
                }
                TensorIndexer::IndexSelect(indexes) => {
                    if indexes.rank() != 1 {
                        crate::bail!("multi-dimensional tensor indexing is not supported")
                    }
                    let out = x.index_select(&indexes.to_device(x.device())?, current_dim)?;
                    current_dim += 1;
                    out
                }
                TensorIndexer::Err(e) => crate::bail!("indexing error {e:?}"),
            };
        }
        Ok(x)
    }
}

#[derive(Debug)]
/// Generic structure used to index a slice of the tensor
pub enum TensorIndexer {
    /// This selects the elements for which an index has some specific value.
    Select(usize),
    /// This is a regular slice, purely indexing a chunk of the tensor
    Narrow(Bound<usize>, Bound<usize>),
    /// Indexing via a 1d tensor
    IndexSelect(Tensor),
    Err(Error),
}

impl From<usize> for TensorIndexer {
    fn from(index: usize) -> Self {
        TensorIndexer::Select(index)
    }
}

impl From<&[u32]> for TensorIndexer {
    fn from(index: &[u32]) -> Self {
        match Tensor::new(index, &crate::Device::Cpu) {
            Ok(tensor) => TensorIndexer::IndexSelect(tensor),
            Err(e) => TensorIndexer::Err(e),
        }
    }
}

impl From<Vec<u32>> for TensorIndexer {
    fn from(index: Vec<u32>) -> Self {
        let len = index.len();
        match Tensor::from_vec(index, len, &crate::Device::Cpu) {
            Ok(tensor) => TensorIndexer::IndexSelect(tensor),
            Err(e) => TensorIndexer::Err(e),
        }
    }
}

impl From<&Tensor> for TensorIndexer {
    fn from(tensor: &Tensor) -> Self {
        TensorIndexer::IndexSelect(tensor.clone())
    }
}

trait RB: RangeBounds<usize> {}
impl RB for Range<usize> {}
impl RB for RangeFrom<usize> {}
impl RB for RangeFull {}
impl RB for RangeInclusive<usize> {}
impl RB for RangeTo<usize> {}
impl RB for RangeToInclusive<usize> {}

impl<T: RB> From<T> for TensorIndexer {
    fn from(range: T) -> Self {
        use std::ops::Bound::*;
        let start = match range.start_bound() {
            Included(idx) => Included(*idx),
            Excluded(idx) => Excluded(*idx),
            Unbounded => Unbounded,
        };
        let end = match range.end_bound() {
            Included(idx) => Included(*idx),
            Excluded(idx) => Excluded(*idx),
            Unbounded => Unbounded,
        };
        TensorIndexer::Narrow(start, end)
    }
}

/// Trait used to implement multiple signatures for ease of use of the slicing
/// of a tensor
pub trait IndexOp<T> {
    /// Returns a slicing iterator which are the chunks of data necessary to
    /// reconstruct the desired tensor.
    fn i(&self, index: T) -> Result<Tensor, Error>;
}

impl<T> IndexOp<T> for Tensor
where
    T: Into<TensorIndexer>,
{
    ///```rust
    /// use candle_core::{Tensor, DType, Device, IndexOp};
    /// let a = Tensor::new(&[
    ///     [0., 1.],
    ///     [2., 3.],
    ///     [4., 5.]
    /// ], &Device::Cpu)?;
    ///
    /// let b = a.i(0)?;
    /// assert_eq!(b.shape().dims(), &[2]);
    /// assert_eq!(b.to_vec1::<f64>()?, &[0., 1.]);
    ///
    /// let c = a.i(..2)?;
    /// assert_eq!(c.shape().dims(), &[2, 2]);
    /// assert_eq!(c.to_vec2::<f64>()?, &[
    ///     [0., 1.],
    ///     [2., 3.]
    /// ]);
    ///
    /// let d = a.i(1..)?;
    /// assert_eq!(d.shape().dims(), &[2, 2]);
    /// assert_eq!(d.to_vec2::<f64>()?, &[
    ///     [2., 3.],
    ///     [4., 5.]
    /// ]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    fn i(&self, index: T) -> Result<Tensor, Error> {
        self.index(&[index.into()])
    }
}

impl<A> IndexOp<(A,)> for Tensor
where
    A: Into<TensorIndexer>,
{
    ///```rust
    /// use candle_core::{Tensor, DType, Device, IndexOp};
    /// let a = Tensor::new(&[
    ///     [0f32, 1.],
    ///     [2.  , 3.],
    ///     [4.  , 5.]
    /// ], &Device::Cpu)?;
    ///
    /// let b = a.i((0,))?;
    /// assert_eq!(b.shape().dims(), &[2]);
    /// assert_eq!(b.to_vec1::<f32>()?, &[0., 1.]);
    ///
    /// let c = a.i((..2,))?;
    /// assert_eq!(c.shape().dims(), &[2, 2]);
    /// assert_eq!(c.to_vec2::<f32>()?, &[
    ///     [0., 1.],
    ///     [2., 3.]
    /// ]);
    ///
    /// let d = a.i((1..,))?;
    /// assert_eq!(d.shape().dims(), &[2, 2]);
    /// assert_eq!(d.to_vec2::<f32>()?, &[
    ///     [2., 3.],
    ///     [4., 5.]
    /// ]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    fn i(&self, (a,): (A,)) -> Result<Tensor, Error> {
        self.index(&[a.into()])
    }
}
#[allow(non_snake_case)]
impl<A, B> IndexOp<(A, B)> for Tensor
where
    A: Into<TensorIndexer>,
    B: Into<TensorIndexer>,
{
    ///```rust
    /// use candle_core::{Tensor, DType, Device, IndexOp};
    /// let a = Tensor::new(&[[0f32, 1., 2.], [3., 4., 5.], [6., 7., 8.]], &Device::Cpu)?;
    ///
    /// let b = a.i((1, 0))?;
    /// assert_eq!(b.to_vec0::<f32>()?, 3.);
    ///
    /// let c = a.i((..2, 1))?;
    /// assert_eq!(c.shape().dims(), &[2]);
    /// assert_eq!(c.to_vec1::<f32>()?, &[1., 4.]);
    ///
    /// let d = a.i((2.., ..))?;
    /// assert_eq!(d.shape().dims(), &[1, 3]);
    /// assert_eq!(d.to_vec2::<f32>()?, &[[6., 7., 8.]]);
    /// # Ok::<(), candle_core::Error>(())
    /// ```
    fn i(&self, (a, b): (A, B)) -> Result<Tensor, Error> {
        self.index(&[a.into(), b.into()])
    }
}

macro_rules! index_op_tuple {
    ($doc:tt, $($t:ident),+) => {
        #[allow(non_snake_case)]
        impl<$($t),*> IndexOp<($($t,)*)> for Tensor
        where
            $($t: Into<TensorIndexer>,)*
        {
            #[doc=$doc]
            fn i(&self, ($($t,)*): ($($t,)*)) -> Result<Tensor, Error> {
                self.index(&[$($t.into(),)*])
            }
        }
    };
}

index_op_tuple!("see [TensorIndex#method.i]", A, B, C);
index_op_tuple!("see [TensorIndex#method.i]", A, B, C, D);
index_op_tuple!("see [TensorIndex#method.i]", A, B, C, D, E);
index_op_tuple!("see [TensorIndex#method.i]", A, B, C, D, E, F);
index_op_tuple!("see [TensorIndex#method.i]", A, B, C, D, E, F, G);
