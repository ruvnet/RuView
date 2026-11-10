use anyhow::Result;
use candle_core::{Device, IndexOp, Tensor};

#[test]
fn integer_index() -> Result<()> {
    let dev = Device::Cpu;

    let tensor = Tensor::arange(0u32, 2 * 3, &dev)?.reshape((2, 3))?;
    let result = tensor.i(1)?;
    assert_eq!(result.dims(), &[3]);
    assert_eq!(result.to_vec1::<u32>()?, &[3, 4, 5]);

    let result = tensor.i((.., 2))?;
    assert_eq!(result.dims(), &[2]);
    assert_eq!(result.to_vec1::<u32>()?, &[2, 5]);

    Ok(())
}

#[test]
fn range_index() -> Result<()> {
    let dev = Device::Cpu;
    // RangeFull
    let tensor = Tensor::arange(0u32, 2 * 3, &dev)?.reshape((2, 3))?;
    let result = tensor.i(..)?;
    assert_eq!(result.dims(), &[2, 3]);
    assert_eq!(result.to_vec2::<u32>()?, &[[0, 1, 2], [3, 4, 5]]);

    // Range
    let tensor = Tensor::arange(0u32, 4 * 3, &dev)?.reshape((4, 3))?;
    let result = tensor.i(1..3)?;
    assert_eq!(result.dims(), &[2, 3]);
    assert_eq!(result.to_vec2::<u32>()?, &[[3, 4, 5], [6, 7, 8]]);

    // RangeFrom
    let result = tensor.i(2..)?;
    assert_eq!(result.dims(), &[2, 3]);
    assert_eq!(result.to_vec2::<u32>()?, &[[6, 7, 8], [9, 10, 11]]);

    // RangeTo
    let result = tensor.i(..2)?;
    assert_eq!(result.dims(), &[2, 3]);
    assert_eq!(result.to_vec2::<u32>()?, &[[0, 1, 2], [3, 4, 5]]);

    // RangeInclusive
    let result = tensor.i(1..=2)?;
    assert_eq!(result.dims(), &[2, 3]);
    assert_eq!(result.to_vec2::<u32>()?, &[[3, 4, 5], [6, 7, 8]]);

    // RangeTo
    let result = tensor.i(..1)?;
    assert_eq!(result.dims(), &[1, 3]);
    assert_eq!(result.to_vec2::<u32>()?, &[[0, 1, 2]]);

    // RangeToInclusive
    let result = tensor.i(..=1)?;
    assert_eq!(result.dims(), &[2, 3]);
    assert_eq!(result.to_vec2::<u32>()?, &[[0, 1, 2], [3, 4, 5]]);

    // Empty range
    let result = tensor.i(1..1)?;
    assert_eq!(result.dims(), &[0, 3]);
    let empty: [[u32; 3]; 0] = [];
    assert_eq!(result.to_vec2::<u32>()?, &empty);

    // Similar to PyTorch, allow empty ranges when the computed length is negative.
    #[allow(clippy::reversed_empty_ranges)]
    let result = tensor.i(1..0)?;
    assert_eq!(result.dims(), &[0, 3]);
    let empty: [[u32; 3]; 0] = [];
    assert_eq!(result.to_vec2::<u32>()?, &empty);
    Ok(())
}

#[test]
fn index_3d() -> Result<()> {
    let tensor = Tensor::from_iter(0..24u32, &Device::Cpu)?.reshape((2, 3, 4))?;
    assert_eq!(tensor.i((0, 0, 0))?.to_scalar::<u32>()?, 0);
    assert_eq!(tensor.i((1, 0, 0))?.to_scalar::<u32>()?, 12);
    assert_eq!(tensor.i((0, 1, 0))?.to_scalar::<u32>()?, 4);
    assert_eq!(tensor.i((0, 1, 3))?.to_scalar::<u32>()?, 7);
    assert_eq!(tensor.i((0..2, 0, 0))?.to_vec1::<u32>()?, &[0, 12]);
    assert_eq!(
        tensor.i((0..2, .., 0))?.to_vec2::<u32>()?,
        &[[0, 4, 8], [12, 16, 20]]
    );
    assert_eq!(
        tensor.i((..2, .., 3))?.to_vec2::<u32>()?,
        &[[3, 7, 11], [15, 19, 23]]
    );
    assert_eq!(tensor.i((1, .., 3))?.to_vec1::<u32>()?, &[15, 19, 23]);
    Ok(())
}

#[test]
fn slice_assign() -> Result<()> {
    let dev = Device::Cpu;

    let tensor = Tensor::arange(0u32, 4 * 5, &dev)?.reshape((4, 5))?;
    let src = Tensor::arange(0u32, 2 * 3, &dev)?.reshape((3, 2))?;
    let out = tensor.slice_assign(&[1..4, 3..5], &src)?;
    assert_eq!(
        out.to_vec2::<u32>()?,
        &[
            [0, 1, 2, 3, 4],
            [5, 6, 7, 0, 1],
            [10, 11, 12, 2, 3],
            [15, 16, 17, 4, 5]
        ]
    );
    let out = tensor.slice_assign(&[0..3, 0..2], &src)?;
    assert_eq!(
        out.to_vec2::<u32>()?,
        &[
            [0, 1, 2, 3, 4],
            [2, 3, 7, 8, 9],
            [4, 5, 12, 13, 14],
            [15, 16, 17, 18, 19]
        ]
    );
    Ok(())
}
