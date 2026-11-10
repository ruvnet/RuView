use candle_core::{test_device, test_utils, DType, Device, IndexOp, Result, Tensor, D};
use float8::F8E4M3;

fn zeros(device: &Device) -> Result<()> {
    let tensor = Tensor::zeros((5, 2), DType::F32, device)?;
    let (dim1, dim2) = tensor.dims2()?;
    assert_eq!(dim1, 5);
    assert_eq!(dim2, 2);
    Ok(())
}

fn ones(device: &Device) -> Result<()> {
    assert_eq!(
        Tensor::ones((2, 3), DType::U8, device)?.to_vec2::<u8>()?,
        [[1, 1, 1], [1, 1, 1]],
    );
    assert_eq!(
        Tensor::ones((2, 3), DType::U32, device)?.to_vec2::<u32>()?,
        [[1, 1, 1], [1, 1, 1]],
    );
    assert_eq!(
        Tensor::ones((2, 3), DType::I64, device)?.to_vec2::<i64>()?,
        [[1, 1, 1], [1, 1, 1]],
    );
    assert_eq!(
        Tensor::ones((2, 3), DType::F32, device)?.to_vec2::<f32>()?,
        [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0]],
    );
    if !device.is_metal() {
        assert_eq!(
            Tensor::ones((2, 3), DType::F64, device)?.to_vec2::<f64>()?,
            [[1.0, 1.0, 1.0], [1.0, 1.0, 1.0]],
        );
    }
    assert_eq!(
        Tensor::ones((2, 3), DType::F16, device)?.to_vec2::<half::f16>()?,
        [
            [
                half::f16::from_f32(1.0),
                half::f16::from_f32(1.0),
                half::f16::from_f32(1.0)
            ],
            [
                half::f16::from_f32(1.0),
                half::f16::from_f32(1.0),
                half::f16::from_f32(1.0)
            ]
        ],
    );
    assert_eq!(
        Tensor::ones((2, 3), DType::BF16, device)?.to_vec2::<half::bf16>()?,
        [
            [
                half::bf16::from_f32(1.0),
                half::bf16::from_f32(1.0),
                half::bf16::from_f32(1.0)
            ],
            [
                half::bf16::from_f32(1.0),
                half::bf16::from_f32(1.0),
                half::bf16::from_f32(1.0)
            ]
        ],
    );

    if !device.is_metal() {
        assert_eq!(
            Tensor::ones((2, 3), DType::F8E4M3, device)?.to_vec2::<F8E4M3>()?,
            [
                [
                    F8E4M3::from_f32(1.),
                    F8E4M3::from_f32(1.),
                    F8E4M3::from_f32(1.)
                ],
                [
                    F8E4M3::from_f32(1.),
                    F8E4M3::from_f32(1.),
                    F8E4M3::from_f32(1.)
                ]
            ],
        );
    }
    Ok(())
}

fn full(device: &Device) -> Result<()> {
    let tensor = Tensor::zeros((3, 4), DType::U32, device)?;
    tensor.const_set(42u32.into())?;
    assert_eq!(
        tensor.to_vec2::<u32>()?,
        [[42, 42, 42, 42], [42, 42, 42, 42], [42, 42, 42, 42]]
    );
    tensor.i((.., 2))?.const_set(1337u32.into())?;
    assert_eq!(
        tensor.to_vec2::<u32>()?,
        [[42, 42, 1337, 42], [42, 42, 1337, 42], [42, 42, 1337, 42]]
    );
    tensor.i((2, ..))?.const_set(1u32.into())?;
    assert_eq!(
        tensor.to_vec2::<u32>()?,
        [[42, 42, 1337, 42], [42, 42, 1337, 42], [1, 1, 1, 1]]
    );
    Ok(())
}

fn const_set(device: &Device) -> Result<()> {
    assert_eq!(
        Tensor::full(42u32, (2, 3), device)?.to_vec2::<u32>()?,
        [[42, 42, 42], [42, 42, 42]],
    );
    Ok(())
}

fn arange(device: &Device) -> Result<()> {
    assert_eq!(
        Tensor::arange(0u8, 5u8, device)?.to_vec1::<u8>()?,
        [0, 1, 2, 3, 4],
    );
    assert_eq!(
        Tensor::arange_step(0u8, 5u8, 2, device)?.to_vec1::<u8>()?,
        [0, 2, 4],
    );
    assert_eq!(
        Tensor::arange_step(0u8, 5u8, 3, device)?.to_vec1::<u8>()?,
        [0, 3],
    );
    assert_eq!(
        Tensor::arange_step(5i64, 0i64, -1, device)?.to_vec1::<i64>()?,
        [5, 4, 3, 2, 1],
    );

    if !device.is_metal() {
        assert_eq!(
            Tensor::arange_step(
                F8E4M3::from_f32(0.),
                F8E4M3::from_f32(5.),
                F8E4M3::from_f32(2.),
                device
            )?
            .to_vec1::<F8E4M3>()?,
            [
                F8E4M3::from_f32(0.),
                F8E4M3::from_f32(2.),
                F8E4M3::from_f32(4.),
            ],
        );
    }

    Ok(())
}

fn add_mul(device: &Device) -> Result<()> {
    let tensor = Tensor::new(&[3f32, 1., 4.], device)?;
    let dim1 = tensor.dims1()?;
    assert_eq!(dim1, 3);
    let content: Vec<f32> = tensor.to_vec1()?;
    assert_eq!(content, [3., 1., 4.]);
    let tensor = Tensor::add(&tensor, &tensor)?;
    let content: Vec<f32> = tensor.to_vec1()?;
    assert_eq!(content, [6., 2., 8.]);
    let tensor = Tensor::mul(&tensor, &tensor)?;
    let content: Vec<f32> = tensor.to_vec1()?;
    assert_eq!(content, [36., 4., 64.]);
    Ok(())
}

fn tensor_2d(device: &Device) -> Result<()> {
    let data = &[[3f32, 1., 4., 1., 5.], [2., 1., 7., 8., 2.]];
    let tensor = Tensor::new(data, device)?;
    let dims = tensor.dims2()?;
    assert_eq!(dims, (2, 5));
    let content: Vec<Vec<f32>> = tensor.to_vec2()?;
    assert_eq!(content, data);
    Ok(())
}

fn clamp(device: &Device) -> Result<()> {
    let data = &[[3f32, 1., 4., 1., 5.], [2., 1., 7., 8., 2.]];
    let tensor = Tensor::new(data, device)?;
    let tensor = tensor.clamp(1.5, 6.2)?;
    assert_eq!(
        tensor.to_vec2::<f32>()?,
        [[3.0, 1.5, 4.0, 1.5, 5.0], [2.0, 1.5, 6.2, 6.2, 2.0]],
    );
    Ok(())
}

fn asort(device: &Device) -> Result<()> {
    let data = &[[3f32, 1., 4., 1.1, 5.], [2.1, 1., 7., 8., 2.]];
    let tensor = Tensor::new(data, device)?;
    let indexes = tensor.arg_sort_last_dim(true)?;
    assert_eq!(
        indexes.to_vec2::<u32>()?,
        [[1, 3, 0, 2, 4], [1, 4, 0, 2, 3]],
    );
    let indexes = tensor.arg_sort_last_dim(false)?;
    assert_eq!(
        indexes.to_vec2::<u32>()?,
        [[4, 2, 0, 3, 1], [3, 2, 0, 4, 1]],
    );
    let (sorted, indexes) = tensor.sort_last_dim(true)?;
    assert_eq!(
        indexes.to_vec2::<u32>()?,
        [[1, 3, 0, 2, 4], [1, 4, 0, 2, 3]],
    );
    assert_eq!(
        sorted.to_vec2::<f32>()?,
        [[1.0, 1.1, 3.0, 4.0, 5.0], [1.0, 2.0, 2.1, 7.0, 8.0]]
    );
    let (sorted, indexes) = tensor.sort_last_dim(false)?;
    assert_eq!(
        indexes.to_vec2::<u32>()?,
        [[4, 2, 0, 3, 1], [3, 2, 0, 4, 1]],
    );
    assert_eq!(
        sorted.to_vec2::<f32>()?,
        [[5.0, 4.0, 3.0, 1.1, 1.0], [8.0, 7.0, 2.1, 2.0, 1.0]]
    );
    Ok(())
}

/// Test sorting a large tensor that exceeds 1024 elements.
fn asort_big(device: &Device) -> Result<()> {
    // Skip on metal for now
    if device.is_metal() {
        return Ok(());
    }
    const SIZE: usize = 2000;
    let data: Vec<f32> = (0..SIZE).map(|x| (SIZE - x) as f32).collect();
    let tensor = Tensor::new(data.as_slice(), device)?;

    let indexes = tensor.arg_sort_last_dim(true)?;
    let expected_indexes: Vec<u32> = (0..SIZE).rev().map(|x| x as u32).collect();
    assert_eq!(indexes.to_vec1::<u32>()?, expected_indexes);

    let indexes = tensor.arg_sort_last_dim(false)?;
    let expected_indexes: Vec<u32> = (0..SIZE).map(|x| x as u32).collect();
    assert_eq!(indexes.to_vec1::<u32>()?, expected_indexes);
    Ok(())
}

fn unary_op(device: &Device) -> Result<()> {
    let data = &[[-3f32, 1., 4., -0.1, 0.5], [2.7, -1.8, -0.28, 1.8, 2.8]];
    let tensor = Tensor::new(data, device)?;
    assert_eq!(
        test_utils::to_vec2_round(&tensor.gelu()?, 4)?,
        [
            [-0.0036, 0.8412, 3.9999, -0.046, 0.3457],
            [2.6911, -0.0647, -0.1091, 1.7353, 2.7933]
        ]
    );
    let t_f16 = tensor.to_dtype(DType::F16)?.gelu()?.to_dtype(DType::F32)?;
    let max_diff = (tensor.gelu()? - t_f16)?.flatten_all()?.max(0)?;
    assert!(max_diff.to_vec0::<f32>()? < 5e-3);
    assert_eq!(
        test_utils::to_vec2_round(&tensor.gelu_erf()?, 4)?,
        [
            [-0.004, 0.8413, 3.9999, -0.046, 0.3457],
            [2.6906, -0.0647, -0.1091, 1.7353, 2.7928]
        ]
    );
    assert_eq!(
        test_utils::to_vec2_round(&tensor.erf()?, 4)?,
        [
            [-1.0, 0.8427, 1.0, -0.1125, 0.5205],
            [0.9999, -0.9891, -0.3079, 0.9891, 0.9999]
        ]
    );
    assert_eq!(
        test_utils::to_vec2_round(&tensor.silu()?, 4)?,
        [
            [-0.1423, 0.7311, 3.9281, -0.0475, 0.3112],
            [2.53, -0.2553, -0.1205, 1.5447, 2.6395]
        ]
    );
    assert_eq!(
        test_utils::to_vec2_round(&tensor.ceil()?, 4)?,
        [[-3.0, 1.0, 4.0, -0.0, 1.0], [3.0, -1.0, -0.0, 2.0, 3.0]]
    );
    assert_eq!(
        test_utils::to_vec2_round(&tensor.floor()?, 4)?,
        [[-3.0, 1.0, 4.0, -1.0, 0.0], [2.0, -2.0, -1.0, 1.0, 2.0]]
    );
    assert_eq!(
        test_utils::to_vec2_round(&tensor.round()?, 4)?,
        [[-3.0, 1.0, 4.0, -0.0, 1.0], [3.0, -2.0, -0.0, 2.0, 3.0]]
    );
    let tensor = Tensor::new(&[2997.9246, 314.15926f32], device)?;
    assert_eq!(
        test_utils::to_vec1_round(&tensor.round_to(2)?, 4)?,
        [2997.92, 314.16]
    );
    assert_eq!(
        test_utils::to_vec1_round(&tensor.round_to(-2)?, 4)?,
        [3000.0, 300.]
    );
    let tensor = Tensor::new(
        &[-1.01f32, -0.9, -0.1, 0.0, -0.0, 0.1, 0.9, 1.0, 1.1],
        device,
    )?;
    assert_eq!(
        tensor.sign()?.to_vec1::<f32>()?,
        [-1., -1., -1., 0., 0., 1., 1., 1., 1.]
    );
    let tensor = Tensor::new(&[-1.0f32, 0., -2., 3.], device)?;
    let y = tensor.elu(2.)?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [-1.2642, 0.0000, -1.7293, 3.0000]
    );
    // This test failed on metal prior to the following PR:
    // https://github.com/huggingface/candle/pull/2490
    let y = tensor.reshape((2, 2))?.t()?.elu(2.)?.flatten_all()?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [-1.2642, -1.7293, 0.0000, 3.0000]
    );
    Ok(())
}

fn binary_op(device: &Device) -> Result<()> {
    let data = &[[3f32, 1., 4., 1., 5.], [2., 1., 7., 8., 2.]];
    let tensor1 = Tensor::new(data, device)?;
    let data2 = &[[5f32, 5., 5., 5., 5.], [2., 1., 7., 8., 2.]];
    let tensor2 = Tensor::new(data2, device)?;
    let tensor = (&tensor1 + (&tensor1 * &tensor1)? / (&tensor1 + &tensor2))?;
    let dims = tensor.dims2()?;
    assert_eq!(dims, (2, 5));
    let content: Vec<Vec<f32>> = tensor.to_vec2()?;
    assert_eq!(content[0], [4.125, 1.1666666, 5.7777777, 1.1666666, 7.5]);
    assert_eq!(content[1], [3.0, 1.5, 10.5, 12.0, 3.0]);
    #[allow(clippy::eq_op)]
    let tensor = (&tensor - &tensor)?;
    let content: Vec<Vec<f32>> = tensor.to_vec2()?;
    assert_eq!(content[0], [0., 0., 0., 0., 0.]);

    let min = tensor1.minimum(&(&tensor2 * 0.5)?)?;
    let max = tensor1.maximum(&(&tensor2 * 0.5)?)?;
    assert_eq!(
        min.to_vec2::<f32>()?,
        [[2.5, 1.0, 2.5, 1.0, 2.5], [1.0, 0.5, 3.5, 4.0, 1.0]],
    );
    assert_eq!(
        max.to_vec2::<f32>()?,
        [[3.0, 2.5, 4.0, 2.5, 5.0], [2.0, 1.0, 7.0, 8.0, 2.0]]
    );
    Ok(())
}

fn ternary_op(device: &Device) -> Result<()> {
    let data = &[[0u8, 1, 0, 1, 0], [1, 1, 1, 0, 0]];
    let ids = Tensor::new(data, device)?;
    let data = &[[0f32, 1., 2., 3., 4.], [5., 6., 7., 8., 9.]];
    let a = Tensor::new(data, device)?;
    let data = &[[10f32, 11., 12., 13., 14.], [15., 16., 17., 18., 19.]];
    let b = Tensor::new(data, device)?;
    let tensor = ids.where_cond(&a, &b)?;
    let dims = tensor.dims();
    assert_eq!(dims, [2, 5]);
    let result: Vec<f32> = tensor.flatten_all()?.to_vec1()?;
    assert_eq!(result, [10., 1., 12., 3., 14., 5., 6., 7., 18., 19.]);
    Ok(())
}

fn transpose(device: &Device) -> Result<()> {
    let data = &[[3f32, 1., 4., 1., 5.], [2., 1., 7., 8., 2.]];
    let tensor = Tensor::new(data, device)?.t()?;
    let dims = tensor.dims2()?;
    assert_eq!(dims, (5, 2));
    assert_eq!(
        tensor.to_vec2::<f32>()?,
        &[[3f32, 2.], [1., 1.], [4., 7.], [1., 8.], [5., 2.]]
    );
    assert_eq!(tensor.t()?.to_vec2::<f32>()?, data);
    assert_eq!(tensor.contiguous()?.t()?.to_vec2::<f32>()?, data);
    assert_eq!(((tensor + 1.)?.t()? - 1.)?.to_vec2::<f32>()?, data);
    Ok(())
}

fn var(device: &Device) -> Result<()> {
    // Values taken from https://pytorch.org/docs/stable/generated/torch.var.html
    let data = &[
        [0.2035f32, 1.2959, 1.8101, -0.4644],
        [1.5027, -0.3270, 0.5905, 0.6538],
        [-1.5745, 1.3330, -0.5596, -0.6548],
        [0.1264, -0.5080, 1.6420, 0.1992],
    ];
    let tensor = Tensor::new(data, device)?;
    assert_eq!(
        test_utils::to_vec2_round(&tensor.var_keepdim(1)?, 4)?,
        &[[1.0631], [0.559], [1.4893], [0.8258]]
    );
    Ok(())
}

fn sum(device: &Device) -> Result<()> {
    let data = &[[[3u32, 1, 4], [1, 5, 9]], [[2, 1, 7], [8, 2, 8]]];
    let tensor = Tensor::new(data, device)?;
    assert_eq!(
        tensor.sum_keepdim(2)?.to_vec3::<u32>()?,
        &[[[8], [15]], [[10], [18]]]
    );
    assert_eq!(
        tensor.sum_keepdim(0)?.to_vec3::<u32>()?,
        &[[[5, 2, 11], [9, 7, 17]]],
    );
    assert_eq!(tensor.sum_keepdim((0, 2, 1))?.to_vec3::<u32>()?, &[[[51]]],);
    assert_eq!(
        tensor.t()?.sum_keepdim(1)?.t()?.to_vec3::<u32>()?,
        &[[[8], [15]], [[10], [18]]]
    );
    assert_eq!(
        tensor.sum_keepdim((2, 1))?.to_vec3::<u32>()?,
        &[[[8 + 15]], [[10 + 18]]]
    );
    let data: Vec<u32> = (0..4000u32).collect();
    let tensor = Tensor::new(data.as_slice(), device)?;
    assert_eq!(tensor.sum_keepdim(0)?.to_vec1::<u32>()?, &[7998000]);
    let tensor = tensor.reshape((2000, 2))?;
    assert_eq!(tensor.sum_keepdim((0, 1))?.to_vec2::<u32>()?, &[[7998000]]);
    assert_eq!(
        tensor.sum_keepdim(0)?.sum_keepdim(1)?.to_vec2::<u32>()?,
        &[[7998000]]
    );
    assert_eq!(
        tensor.sum_keepdim(1)?.sum_keepdim(0)?.to_vec2::<u32>()?,
        &[[7998000]]
    );
    assert_eq!(
        tensor.sum_keepdim(0)?.to_vec2::<u32>()?,
        &[[3998000, 4000000]]
    );

    // Make the tensor non contiguous.
    let tensor = tensor.t()?.contiguous()?.t()?;
    assert_eq!(tensor.sum_keepdim((0, 1))?.to_vec2::<u32>()?, &[[7998000]]);
    assert_eq!(
        tensor.sum_keepdim(0)?.sum_keepdim(1)?.to_vec2::<u32>()?,
        &[[7998000]]
    );
    assert_eq!(
        tensor.sum_keepdim(1)?.sum_keepdim(0)?.to_vec2::<u32>()?,
        &[[7998000]]
    );
    assert_eq!(
        tensor.sum_keepdim(0)?.to_vec2::<u32>()?,
        &[[3998000, 4000000]]
    );

    let t1 = tensor.reshape((200, 5, 4))?;
    let t2 = t1.transpose(0, 2)?.contiguous()?.transpose(0, 2)?;
    for tensor in [t1, t2] {
        assert_eq!(
            tensor.sum_keepdim((0, 1, 2))?.to_vec3::<u32>()?,
            &[[[7998000]]]
        );
        assert_eq!(
            tensor
                .sum_keepdim(0)?
                .sum_keepdim(2)?
                .sum_keepdim(1)?
                .to_vec3::<u32>()?,
            &[[[7998000]]]
        );
        assert_eq!(
            tensor
                .sum_keepdim(0)?
                .sum_keepdim((1, 2))?
                .to_vec3::<u32>()?,
            &[[[7998000]]]
        );
        assert_eq!(
            tensor
                .sum_keepdim(1)?
                .sum_keepdim((0, 2))?
                .to_vec3::<u32>()?,
            &[[[7998000]]]
        );
        assert_eq!(
            tensor.sum_keepdim(0)?.to_vec3::<u32>()?,
            &[[
                [398000, 398200, 398400, 398600],
                [398800, 399000, 399200, 399400],
                [399600, 399800, 400000, 400200],
                [400400, 400600, 400800, 401000],
                [401200, 401400, 401600, 401800]
            ]]
        );
    }
    Ok(())
}

fn min(device: &Device) -> Result<()> {
    let data = &[[[3u32, 1, 4], [1, 5, 9]], [[2, 1, 7], [8, 2, 8]]];
    let tensor = Tensor::new(data, device)?;
    assert_eq!(
        tensor.min_keepdim(2)?.to_vec3::<u32>()?,
        &[[[1], [1]], [[1], [2]]]
    );
    assert_eq!(
        tensor.min_keepdim(0)?.to_vec3::<u32>()?,
        &[[[2, 1, 4], [1, 2, 8]]],
    );
    let data: Vec<u32> = (200..4000u32).collect();
    let tensor = Tensor::new(data.as_slice(), device)?;
    assert_eq!(tensor.min_keepdim(0)?.to_vec1::<u32>()?, &[200]);
    let tensor = tensor.reshape((1900, 2))?;
    assert_eq!(
        tensor.min_keepdim(0)?.min_keepdim(1)?.to_vec2::<u32>()?,
        &[[200]]
    );
    assert_eq!(
        tensor.min_keepdim(1)?.min_keepdim(0)?.to_vec2::<u32>()?,
        &[[200]]
    );
    assert_eq!(tensor.min_keepdim(0)?.to_vec2::<u32>()?, &[[200, 201]]);

    // Make the tensor non contiguous.
    let tensor = tensor.t()?.contiguous()?.t()?;
    assert_eq!(
        tensor.min_keepdim(0)?.min_keepdim(1)?.to_vec2::<u32>()?,
        &[[200]]
    );
    assert_eq!(
        tensor.min_keepdim(1)?.min_keepdim(0)?.to_vec2::<u32>()?,
        &[[200]]
    );
    assert_eq!(tensor.min_keepdim(0)?.to_vec2::<u32>()?, &[[200, 201]]);

    let t1 = tensor.reshape((190, 5, 4))?;
    let t2 = t1.transpose(0, 2)?.contiguous()?.transpose(0, 2)?;
    for tensor in [t1, t2] {
        assert_eq!(
            tensor
                .min_keepdim(0)?
                .min_keepdim(2)?
                .min_keepdim(1)?
                .to_vec3::<u32>()?,
            &[[[200]]]
        );
        assert_eq!(
            tensor.min_keepdim(0)?.to_vec3::<u32>()?,
            &[[
                [200, 201, 202, 203],
                [204, 205, 206, 207],
                [208, 209, 210, 211],
                [212, 213, 214, 215],
                [216, 217, 218, 219]
            ]]
        );
    }
    Ok(())
}

fn max(device: &Device) -> Result<()> {
    let data = &[[[3u32, 1, 4], [1, 5, 9]], [[2, 1, 7], [8, 2, 8]]];
    let tensor = Tensor::new(data, device)?;
    assert_eq!(
        tensor.max_keepdim(2)?.to_vec3::<u32>()?,
        &[[[4], [9]], [[7], [8]]]
    );
    assert_eq!(
        tensor.max_keepdim(0)?.to_vec3::<u32>()?,
        &[[[3, 1, 7], [8, 5, 9]]],
    );
    let data: Vec<u32> = (200..4000u32).collect();
    let tensor = Tensor::new(data.as_slice(), device)?;
    assert_eq!(tensor.max_keepdim(0)?.to_vec1::<u32>()?, &[3999]);
    let tensor = tensor.reshape((1900, 2))?;
    assert_eq!(
        tensor.max_keepdim(0)?.max_keepdim(1)?.to_vec2::<u32>()?,
        &[[3999]]
    );
    assert_eq!(
        tensor.max_keepdim(1)?.max_keepdim(0)?.to_vec2::<u32>()?,
        &[[3999]]
    );
    assert_eq!(tensor.max_keepdim(0)?.to_vec2::<u32>()?, &[[3998, 3999]]);

    // Make the tensor non contiguous.
    let tensor = tensor.t()?.contiguous()?.t()?;
    assert_eq!(
        tensor.max_keepdim(0)?.max_keepdim(1)?.to_vec2::<u32>()?,
        &[[3999]]
    );
    assert_eq!(
        tensor.max_keepdim(1)?.max_keepdim(0)?.to_vec2::<u32>()?,
        &[[3999]]
    );
    assert_eq!(tensor.max_keepdim(0)?.to_vec2::<u32>()?, &[[3998, 3999]]);

    let t1 = tensor.reshape((190, 5, 4))?;
    let t2 = t1.transpose(0, 2)?.contiguous()?.transpose(0, 2)?;
    for tensor in [t1, t2] {
        assert_eq!(
            tensor
                .max_keepdim(0)?
                .max_keepdim(2)?
                .max_keepdim(1)?
                .to_vec3::<u32>()?,
            &[[[3999]]]
        );
        assert_eq!(
            tensor.max_keepdim(0)?.to_vec3::<u32>()?,
            &[[
                [3980, 3981, 3982, 3983],
                [3984, 3985, 3986, 3987],
                [3988, 3989, 3990, 3991],
                [3992, 3993, 3994, 3995],
                [3996, 3997, 3998, 3999]
            ]]
        );
    }
    Ok(())
}

fn argmin(device: &Device) -> Result<()> {
    let data = &[[[3u32, 1, 4], [1, 5, 9]], [[2, 1, 7], [8, 2, 8]]];
    let tensor = Tensor::new(data, device)?;
    assert_eq!(
        tensor.argmin_keepdim(2)?.to_vec3::<u32>()?,
        &[[[1], [0]], [[1], [1]]]
    );
    assert_eq!(
        tensor.argmin_keepdim(0)?.to_vec3::<u32>()?,
        &[[[1, 0, 0], [0, 1, 1]]],
    );
    let data: Vec<u32> = (200..4000u32).collect();
    let tensor = Tensor::new(data.as_slice(), device)?;
    assert_eq!(tensor.argmin_keepdim(0)?.to_vec1::<u32>()?, &[0]);
    let tensor = tensor.reshape((1900, 2))?;
    assert_eq!(
        tensor
            .argmin_keepdim(0)?
            .argmin_keepdim(1)?
            .to_vec2::<u32>()?,
        &[[0]]
    );
    assert_eq!(
        tensor
            .argmin_keepdim(1)?
            .argmin_keepdim(0)?
            .to_vec2::<u32>()?,
        &[[0]]
    );
    assert_eq!(tensor.argmin_keepdim(0)?.to_vec2::<u32>()?, &[[0, 0]]);

    // Make the tensor non contiguous.
    let tensor = tensor.t()?.contiguous()?.t()?;
    assert_eq!(
        tensor
            .argmin_keepdim(0)?
            .argmin_keepdim(1)?
            .to_vec2::<u32>()?,
        &[[0]]
    );
    assert_eq!(
        tensor
            .argmin_keepdim(1)?
            .argmin_keepdim(0)?
            .to_vec2::<u32>()?,
        &[[0]]
    );
    assert_eq!(tensor.argmin_keepdim(0)?.to_vec2::<u32>()?, &[[0, 0]]);

    let t1 = tensor.reshape((190, 5, 4))?;
    let t2 = t1.transpose(0, 2)?.contiguous()?.transpose(0, 2)?;
    for tensor in [t1, t2] {
        assert_eq!(
            tensor
                .argmin_keepdim(0)?
                .argmin_keepdim(2)?
                .argmin_keepdim(1)?
                .to_vec3::<u32>()?,
            &[[[0]]]
        );
        assert_eq!(
            tensor.argmin_keepdim(0)?.to_vec3::<u32>()?,
            &[[
                [0, 0, 0, 0],
                [0, 0, 0, 0],
                [0, 0, 0, 0],
                [0, 0, 0, 0],
                [0, 0, 0, 0],
            ]]
        );
    }
    Ok(())
}

fn argmax(device: &Device) -> Result<()> {
    let data = &[[[3u32, 1, 4], [1, 5, 9]], [[2, 1, 7], [8, 2, 8]]];
    let tensor = Tensor::new(data, device)?;
    assert_eq!(
        tensor.argmax_keepdim(2)?.to_vec3::<u32>()?,
        &[[[2], [2]], [[2], [0]]]
    );
    assert_eq!(
        tensor.argmax_keepdim(0)?.to_vec3::<u32>()?,
        &[[[0, 0, 1], [1, 0, 0]]],
    );
    let data: Vec<u32> = (200..4000u32).collect();
    let tensor = Tensor::new(data.as_slice(), device)?;
    assert_eq!(tensor.argmax_keepdim(0)?.to_vec1::<u32>()?, &[3799]);
    let tensor = tensor.reshape((1900, 2))?;
    assert_eq!(
        tensor
            .argmax_keepdim(0)?
            .argmax_keepdim(1)?
            .to_vec2::<u32>()?,
        &[[0]]
    );
    assert_eq!(
        tensor
            .argmax_keepdim(1)?
            .argmax_keepdim(0)?
            .to_vec2::<u32>()?,
        &[[0]]
    );
    assert_eq!(tensor.argmax_keepdim(0)?.to_vec2::<u32>()?, &[[1899, 1899]]);

    // Make the tensor non contiguous.
    let tensor = tensor.t()?.contiguous()?.t()?;
    assert_eq!(
        tensor
            .argmax_keepdim(0)?
            .argmax_keepdim(1)?
            .to_vec2::<u32>()?,
        &[[0]]
    );
    assert_eq!(
        tensor
            .argmax_keepdim(1)?
            .argmax_keepdim(0)?
            .to_vec2::<u32>()?,
        &[[0]]
    );
    assert_eq!(tensor.argmax_keepdim(0)?.to_vec2::<u32>()?, &[[1899, 1899]]);

    let t1 = tensor.reshape((190, 5, 4))?;
    let t2 = t1.transpose(0, 2)?.contiguous()?.transpose(0, 2)?;
    for tensor in [t1, t2] {
        assert_eq!(
            tensor
                .argmax_keepdim(0)?
                .argmax_keepdim(2)?
                .argmax_keepdim(1)?
                .to_vec3::<u32>()?,
            &[[[0]]]
        );
        assert_eq!(
            tensor.argmax_keepdim(0)?.to_vec3::<u32>()?,
            &[[
                [189, 189, 189, 189],
                [189, 189, 189, 189],
                [189, 189, 189, 189],
                [189, 189, 189, 189],
                [189, 189, 189, 189],
            ]]
        );
    }
    Ok(())
}

fn narrow(device: &Device) -> Result<()> {
    let data = &[[[3f32, 1., 4.], [1., 5., 9.]], [[2., 1., 7.], [8., 2., 8.]]];
    let tensor = Tensor::new(data, device)?;
    assert_eq!(
        tensor.narrow(2, 1, 2)?.to_vec3::<f32>()?,
        &[[[1.0, 4.0], [5.0, 9.0]], [[1.0, 7.0], [2.0, 8.0]]],
    );
    assert_eq!(
        tensor.narrow(1, 1, 1)?.to_vec3::<f32>()?,
        &[[[1.0, 5.0, 9.0]], [[8.0, 2.0, 8.0]]],
    );
    assert_eq!(
        tensor.narrow(0, 0, 1)?.to_vec3::<f32>()?,
        &[[[3.0, 1.0, 4.0], [1.0, 5.0, 9.0]]],
    );
    assert_eq!(
        tensor.narrow(0, 1, 1)?.to_vec3::<f32>()?,
        &[[[2.0, 1.0, 7.0], [8.0, 2.0, 8.0]]],
    );
    // The following has been checked against PyTorch via:
    //   import torch
    //   t = torch.tensor([[[3., 1., 4.], [1., 5., 9.]], [[2., 1., 7.], [8., 2., 8.]]])
    //   t.transpose(-1, -2).narrow(1, 1, 2)
    assert_eq!(
        tensor.t()?.narrow(1, 1, 2)?.to_vec3::<f32>()?,
        &[[[1.0, 5.0], [4.0, 9.0]], [[1.0, 2.0], [7.0, 8.0]]],
    );
    Ok(())
}

fn broadcast(device: &Device) -> Result<()> {
    let data = &[3f32, 1., 4.];
    let tensor = Tensor::new(data, device)?;
    assert_eq!(
        tensor.broadcast_left((3, 1))?.to_vec3::<f32>()?,
        &[[[3.0, 1.0, 4.0]], [[3.0, 1.0, 4.0]], [[3.0, 1.0, 4.0]]]
    );
    Ok(())
}

fn slice_set(device: &Device) -> Result<()> {
    let (b, h, max_t, d) = (2, 4, 7, 3);
    let cache = Tensor::zeros((b, h, max_t, d), DType::F32, device)?;
    let tensor = Tensor::randn(0f32, 1f32, (b, h, 4, d), device)?;
    cache.slice_set(&tensor, 2, 0)?;
    let cache_t = cache.narrow(2, 0, 4)?;
    let diff = (cache_t - &tensor)?.abs()?.sum_all()?.to_vec0::<f32>()?;
    assert_eq!(diff, 0.);
    cache.slice_set(&tensor, 2, 1)?;
    let cache_t = cache.narrow(2, 1, 4)?;
    let diff = (cache_t - &tensor)?.abs()?.sum_all()?.to_vec0::<f32>()?;
    assert_eq!(diff, 0.);
    let ones = Tensor::ones((b, h, 1, d), DType::F32, device)?;
    cache.slice_set(&ones, 2, 6)?;
    let diff = cache.narrow(2, 5, 1)?.abs()?.sum_all()?.to_vec0::<f32>()?;
    assert_eq!(diff, 0.);
    let diff = (cache.narrow(2, 6, 1)? - 1.)?
        .abs()?
        .sum_all()?
        .to_vec0::<f32>()?;
    assert_eq!(diff, 0.);
    // This used to create a deadlock rather than returning an actual error.
    assert!(cache.slice_set(&cache, 0, 0).is_err());
    Ok(())
}

fn cat(device: &Device) -> Result<()> {
    // 1D
    let t1 = Tensor::new(&[3f32, 1., 4.], device)?;
    let t2 = Tensor::new(&[1f32, 5., 9., 2.], device)?;
    let t3 = Tensor::new(&[6f32, 5., 3., 5., 8., 9.], device)?;
    assert_eq!(Tensor::cat(&[&t1], 0)?.to_vec1::<f32>()?, [3f32, 1., 4.],);
    assert_eq!(
        Tensor::cat(&[&t1, &t2], 0)?.to_vec1::<f32>()?,
        [3f32, 1., 4., 1., 5., 9., 2.],
    );
    assert_eq!(
        Tensor::cat(&[&t1, &t2, &t3], 0)?.to_vec1::<f32>()?,
        [3f32, 1., 4., 1., 5., 9., 2., 6., 5., 3., 5., 8., 9.],
    );

    // 2D
    let data = &[[3f32, 1., 4., 1., 5.], [2., 7., 1., 8., 2.]];
    let t1 = Tensor::new(data, device)?;
    let data2 = &[[5f32, 5., 5., 5., 5.], [2., 7., 1., 8., 2.]];
    let t2 = Tensor::new(data2, device)?;
    assert_eq!(
        Tensor::cat(&[&t1, &t2], 0)?.to_vec2::<f32>()?,
        [
            [3.0, 1.0, 4.0, 1.0, 5.0],
            [2.0, 7.0, 1.0, 8.0, 2.0],
            [5.0, 5.0, 5.0, 5.0, 5.0],
            [2.0, 7.0, 1.0, 8.0, 2.0]
        ]
    );
    // PyTorch equivalent:
    //     import torch
    //     t1 = torch.tensor([[3, 1, 4, 1, 5], [2, 7, 1, 8, 2]])
    //     t2 = torch.tensor([[5]*5, [2, 7, 1, 8, 2]])
    //     torch.cat([t1.t(), t2.t()], dim=1).t()
    assert_eq!(
        Tensor::cat(&[&t1.t()?, &t2.t()?], 1)?
            .t()?
            .to_vec2::<f32>()?,
        [
            [3.0, 1.0, 4.0, 1.0, 5.0],
            [2.0, 7.0, 1.0, 8.0, 2.0],
            [5.0, 5.0, 5.0, 5.0, 5.0],
            [2.0, 7.0, 1.0, 8.0, 2.0]
        ]
    );
    assert_eq!(
        Tensor::cat(&[&t1, &t2], 1)?.to_vec2::<f32>()?,
        [
            [3.0, 1.0, 4.0, 1.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0],
            [2.0, 7.0, 1.0, 8.0, 2.0, 2.0, 7.0, 1.0, 8.0, 2.0]
        ]
    );

    // 3D
    let t1 = Tensor::arange(0, 48i64, device)?.reshape((2, 6, 4))?;
    let t2 = Tensor::arange(100, 124i64, device)?.reshape((2, 3, 4))?;
    let t3 = Tensor::arange(10000, 10032i64, device)?.reshape((2, 4, 4))?;

    let t_cat = Tensor::cat(&[&t1, &t2, &t3], 1)?;

    let t1 = t1.t()?.contiguous()?.t()?;
    let t2 = t2.t()?.contiguous()?.t()?;
    let t3 = t3.t()?.contiguous()?.t()?;
    let t_cat2 = Tensor::cat(&[&t1, &t2, &t3], 1)?;

    let diff = t_cat.eq(&t_cat2)?.to_dtype(DType::F32)?.sum_all()?;
    assert_eq!(diff.to_vec0::<f32>()?, 104.0);
    assert_eq!(t_cat.i((0, 0, 0))?.to_vec0::<i64>()?, 0);
    assert_eq!(t_cat.i((0, 4, 0))?.to_vec0::<i64>()?, 16);
    assert_eq!(t_cat.i((0, 5, 0))?.to_vec0::<i64>()?, 20);
    assert_eq!(t_cat.i((1, 5, 0))?.to_vec0::<i64>()?, 44);
    assert_eq!(t_cat.i((0, 6, 0))?.to_vec0::<i64>()?, 100);
    assert_eq!(t_cat.i((1, 6, 0))?.to_vec0::<i64>()?, 112);
    assert_eq!(t_cat.i((0, 6, 1))?.to_vec0::<i64>()?, 101);
    assert_eq!(t_cat.i((0, 7, 1))?.to_vec0::<i64>()?, 105);
    assert_eq!(t_cat.i((0, 12, 1))?.to_vec0::<i64>()?, 10013);
    assert_eq!(t_cat.i((1, 12, 3))?.to_vec0::<i64>()?, 10031);
    Ok(())
}

fn embeddings(device: &Device) -> Result<()> {
    let ids = Tensor::new(&[0u32, 2u32, 1u32], device)?;
    let t = Tensor::new(&[[0f32, 1f32], [2f32, 3f32], [4f32, 5f32]], device)?;
    let hs = t.embedding(&ids)?;
    assert_eq!(hs.to_vec2::<f32>()?, &[[0.0, 1.0], [4.0, 5.0], [2.0, 3.0]]);
    let hs = t.index_select(&ids, 0)?;
    assert_eq!(hs.to_vec2::<f32>()?, &[[0.0, 1.0], [4.0, 5.0], [2.0, 3.0]]);
    let hs = t.index_select(&ids.to_dtype(DType::I64)?, 0)?;
    assert_eq!(hs.to_vec2::<f32>()?, &[[0.0, 1.0], [4.0, 5.0], [2.0, 3.0]]);
    let ids = Tensor::new(&[u32::MAX, 2u32, u32::MAX], device)?;
    let hs = t.index_select(&ids, 0)?;
    assert_eq!(hs.to_vec2::<f32>()?, &[[0.0, 0.0], [4.0, 5.0], [0.0, 0.0]]);
    Ok(())
}

#[test]
fn index_select_fail() -> Result<()> {
    // Check that an error is properly reported on out of bounds.
    let ids = Tensor::new(&[4u32, 2u32, 1u32], &Device::Cpu)?;
    let t = Tensor::new(&[[0f32, 1f32], [2f32, 3f32], [4f32, 5f32]], &Device::Cpu)?;
    let hs = t.index_select(&ids, 0);
    assert!(hs.is_err());
    Ok(())
}

// The test below triggers an unwinding panic as there is a panic within the
// #[cfg(feature = "cuda")]
// #[test]
// #[should_panic]
// fn index_select_fail_gpu() {
//     // Check that a panic happens for out of bounds in cuda
//     if let Ok(device) = Device::new_cuda(0) {
//         if let Ok(ids) = Tensor::new(&[4u32, 2u32, 1u32], &device) {
//             if let Ok(t) = Tensor::new(&[[0f32, 1f32], [2f32, 3f32], [4f32, 5f32]], &device) {
//                 let _ = t.index_select(&ids, 0);
//             }
//         }
//     }
// }

fn cmp(device: &Device) -> Result<()> {
    let t1 = Tensor::new(&[[0f32, 1f32], [2f32, 3f32], [4f32, 5f32]], device)?;
    let t2 = Tensor::new(&[[1f32, 0f32], [3f32, 3f32], [4f32, 7f32]], device)?;
    assert_eq!(t1.eq(&t2)?.to_vec2::<u8>()?, &[[0, 0], [0, 1], [1, 0]]);
    assert_eq!(t1.ne(&t2)?.to_vec2::<u8>()?, &[[1, 1], [1, 0], [0, 1]]);
    assert_eq!(t1.le(&t2)?.to_vec2::<u8>()?, &[[1, 0], [1, 1], [1, 1]]);
    assert_eq!(t1.lt(&t2)?.to_vec2::<u8>()?, &[[1, 0], [1, 0], [0, 1]]);
    assert_eq!(t1.gt(&t2)?.to_vec2::<u8>()?, &[[0, 1], [0, 0], [0, 0]]);
    assert_eq!(t1.ge(&t2)?.to_vec2::<u8>()?, &[[0, 1], [0, 1], [1, 0]]);
    Ok(())
}

fn index_select(device: &Device) -> Result<()> {
    let ids = Tensor::new(&[0u32, 2u32, 1u32], device)?;
    let t = Tensor::arange(0f32, 12f32, device)?.reshape((4, 3))?;
    assert_eq!(
        t.to_vec2::<f32>()?,
        &[
            [0.0, 1.0, 2.0],
            [3.0, 4.0, 5.0],
            [6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0]
        ]
    );
    for dtype in [DType::U8, DType::U32, DType::I64] {
        let ids = ids.to_dtype(dtype)?;
        let hs = t.index_select(&ids, 1)?;
        assert_eq!(
            hs.to_vec2::<f32>()?,
            &[
                [0.0, 2.0, 1.0],
                [3.0, 5.0, 4.0],
                [6.0, 8.0, 7.0],
                [9.0, 11.0, 10.0]
            ]
        );
        let hs = t.index_select(&ids, 0)?;
        assert_eq!(
            hs.to_vec2::<f32>()?,
            &[[0.0, 1.0, 2.0], [6.0, 7.0, 8.0], [3.0, 4.0, 5.0]]
        );
        // Prior to https://github.com/huggingface/candle/pull/1022
        // There would be a bug where the last values in the result tensor would be set to 0.
        let ids = Tensor::new(&[0u32, 2u32, 1u32, 0u32, 2u32, 1u32], device)?;
        let hs = t.index_select(&ids, 0)?;
        assert_eq!(
            hs.to_vec2::<f32>()?,
            &[
                [0.0, 1.0, 2.0],
                [6.0, 7.0, 8.0],
                [3.0, 4.0, 5.0],
                [0.0, 1.0, 2.0],
                [6.0, 7.0, 8.0],
                [3.0, 4.0, 5.0],
            ]
        );

        // Test when selecting dim > 0 with ids size different from elem count of
        // target dim in source/input.
        let ids = Tensor::new(&[1u32, 0u32, 1u32], device)?;
        let t = Tensor::arange(1f32, 5f32, device)?.reshape((2, 2))?;
        assert_eq!(t.to_vec2::<f32>()?, &[[1.0, 2.0], [3.0, 4.0]]);
        let hs = t.index_select(&ids, 1)?;
        assert_eq!(hs.to_vec2::<f32>()?, &[[2.0, 1.0, 2.0], [4.0, 3.0, 4.0]]);
    }

    Ok(())
}

fn index_add(device: &Device) -> Result<()> {
    let ids = Tensor::new(&[0u32, 1u32, 1u32], device)?;
    let t = Tensor::arange(0f32, 12f32, device)?.reshape((4, 3))?;
    assert_eq!(
        t.to_vec2::<f32>()?,
        &[
            [0.0, 1.0, 2.0],
            [3.0, 4.0, 5.0],
            [6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0]
        ]
    );
    let init = Tensor::ones((4, 2), DType::F32, device)?;
    let hs = init.index_add(&ids, &t, 1)?;
    assert_eq!(
        hs.to_vec2::<f32>()?,
        &[[1.0, 4.0], [4.0, 10.0], [7.0, 16.0], [10.0, 22.0]],
    );
    let init = Tensor::zeros((4, 2), DType::F32, device)?;
    let ids = Tensor::new(&[1u32, 0u32, 0u32], device)?;
    let hs = init.index_add(&ids, &t, 1)?;
    assert_eq!(
        hs.to_vec2::<f32>()?,
        &[[3.0, 0.0], [9.0, 3.0], [15.0, 6.0], [21.0, 9.0]],
    );

    let init = Tensor::zeros((6, 3), DType::F32, device)?;
    let ids = Tensor::new(&[5u32, 0u32, 1u32, 0u32], device)?;
    let hs = init.index_add(&ids, &t, 0)?;
    assert_eq!(
        hs.to_vec2::<f32>()?,
        &[
            [12.0, 14.0, 16.0],
            [6.0, 7.0, 8.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 2.0]
        ]
    );
    Ok(())
}

fn slice_scatter(device: &Device) -> Result<()> {
    let t = Tensor::arange(0f32, 12f32, device)?.reshape((4, 3))?;
    assert_eq!(
        t.to_vec2::<f32>()?,
        &[
            [0.0, 1.0, 2.0],
            [3.0, 4.0, 5.0],
            [6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0]
        ]
    );
    let src = Tensor::arange(100f32, 106f32, device)?.reshape((2, 3))?;
    assert_eq!(
        t.slice_scatter0(&src, 0)?.to_vec2::<f32>()?,
        &[
            [100.0, 101.0, 102.0],
            [103.0, 104.0, 105.0],
            [6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0]
        ]
    );
    assert_eq!(
        t.slice_scatter0(&src, 1)?.to_vec2::<f32>()?,
        &[
            [0.0, 1.0, 2.0],
            [100.0, 101.0, 102.0],
            [103.0, 104.0, 105.0],
            [9.0, 10.0, 11.0]
        ]
    );
    assert_eq!(
        t.slice_scatter0(&src, 2)?.to_vec2::<f32>()?,
        &[
            [0.0, 1.0, 2.0],
            [3.0, 4.0, 5.0],
            [100.0, 101.0, 102.0],
            [103.0, 104.0, 105.0],
        ]
    );
    Ok(())
}

fn scatter(device: &Device) -> Result<()> {
    let t = Tensor::arange(0f32, 12f32, device)?.reshape((4, 3))?;
    assert_eq!(
        t.to_vec2::<f32>()?,
        &[
            [0.0, 1.0, 2.0],
            [3.0, 4.0, 5.0],
            [6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0]
        ]
    );
    let ids = Tensor::new(&[[0u32, 1, 2], [3, 4, 0], [3, 3, 1], [2, 0, 4]], device)?;
    let init = Tensor::ones((4, 5), DType::F32, device)?;
    let hs = init.scatter_add(&ids, &t, 1)?;
    assert_eq!(
        hs.to_vec2::<f32>()?,
        &[
            [1.0, 2.0, 3.0, 1.0, 1.0],
            [6.0, 1.0, 1.0, 4.0, 5.0],
            [1.0, 9.0, 1.0, 14.0, 1.0],
            [11.0, 1.0, 10.0, 1.0, 12.0]
        ]
    );

    let hs = init.scatter(&ids, &t, 1)?;
    assert_eq!(
        hs.to_vec2::<f32>()?,
        &[
            [0.0, 1.0, 2.0, 1.0, 1.0],
            [5.0, 1.0, 1.0, 3.0, 4.0],
            [1.0, 8.0, 1.0, 7.0, 1.0],
            [10.0, 1.0, 9.0, 1.0, 11.0]
        ]
    );

    let init = Tensor::ones((6, 3), DType::F32, device)?;
    let hs = init.scatter_add(&ids, &t, 0)?;
    assert_eq!(
        hs.to_vec2::<f32>()?,
        &[
            [1.0, 11.0, 6.0],
            [1.0, 2.0, 9.0],
            [10.0, 1.0, 3.0],
            [10.0, 8.0, 1.0],
            [1.0, 5.0, 12.0],
            [1.0, 1.0, 1.0]
        ]
    );
    let hs = init.scatter(&ids, &t, 0)?;
    assert_eq!(
        hs.to_vec2::<f32>()?,
        &[
            [0.0, 10.0, 5.0],
            [1.0, 1.0, 8.0],
            [9.0, 1.0, 2.0],
            [6.0, 7.0, 1.0],
            [1.0, 4.0, 11.0],
            [1.0, 1.0, 1.0]
        ]
    );

    let hs = {
        let ids = Tensor::new(
            &[
                [0u32, u32::MAX, 2],
                [3, 4, u32::MAX],
                [3, 3, 1],
                [u32::MAX, u32::MAX, 4],
            ],
            device,
        )?;
        init.scatter(&ids, &t, 0)?
    };
    assert_eq!(
        hs.to_vec2::<f32>()?,
        &[
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 8.0],
            [1.0, 1.0, 2.0],
            [6.0, 7.0, 1.0],
            [1.0, 4.0, 11.0],
            [1.0, 1.0, 1.0]
        ]
    );

    init.scatter_set(&ids, &t, 0)?;
    assert_eq!(
        init.to_vec2::<f32>()?,
        &[
            [0.0, 10.0, 5.0],
            [1.0, 1.0, 8.0],
            [9.0, 1.0, 2.0],
            [6.0, 7.0, 1.0],
            [1.0, 4.0, 11.0],
            [1.0, 1.0, 1.0]
        ]
    );

    Ok(())
}

fn gather(device: &Device) -> Result<()> {
    let ids = Tensor::new(&[[0u32], [2u32], [1u32], [0u32]], device)?;
    let t = Tensor::arange(0f32, 12f32, device)?.reshape((4, 3))?;
    assert_eq!(
        t.to_vec2::<f32>()?,
        &[
            [0.0, 1.0, 2.0],
            [3.0, 4.0, 5.0],
            [6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0]
        ]
    );
    let hs = t.gather(&ids, 1)?;
    assert_eq!(hs.to_vec2::<f32>()?, &[[0.0], [5.0], [7.0], [9.0]]);
    let ids = Tensor::new(
        &[[0u32, 0u32], [2u32, 0u32], [1u32, 1u32], [0u32, 2u32]],
        device,
    )?;
    let hs = t.gather(&ids, 1)?;
    assert_eq!(
        hs.to_vec2::<f32>()?,
        &[[0.0, 0.0], [5.0, 3.0], [7.0, 7.0], [9.0, 11.0]]
    );
    let ids = Tensor::new(&[[0u32, 2u32, 0u32]], device)?;
    let hs = t.gather(&ids, 0)?;
    assert_eq!(hs.to_vec2::<f32>()?, &[[0.0, 7.0, 2.0]]);
    let ids = Tensor::new(&[[0u32, 2u32, 0u32], [0u32, 1u32, 1u32]], device)?;
    let hs = t.gather(&ids, 0)?;
    assert_eq!(hs.to_vec2::<f32>()?, &[[0.0, 7.0, 2.0], [0.0, 4.0, 5.0]]);

    let hs = {
        let ids = Tensor::new(
            &[
                [0u32, 0u32],
                [2u32, u32::MAX],
                [u32::MAX, 1u32],
                [0u32, 2u32],
            ],
            device,
        )?;
        t.gather(&ids, 1)?
    };
    assert_eq!(
        hs.to_vec2::<f32>()?,
        &[[0.0, 0.0], [5.0, 0.0], [0.0, 7.0], [9.0, 11.0]]
    );

    // Random data

    // Dim: 0
    let t = Tensor::new(
        &[
            [
                [108_f32, -47., 16., -56., -83., -130., 210.],
                [253., 95., 151., 228., -210., -123., -127.],
                [-9., -217., 2., -78., 163., 245., -204.],
                [-246., 79., -238., 88., -226., -184., 171.],
                [8., -48., -153., 234., -34., 166., -153.],
                [124., 0., -10., -61., -242., -15., -238.],
            ],
            [
                [12., -64., -199., 244., -240., 156., -128.],
                [173., -57., 4., -198., 233., -110., 238.],
                [95., 82., 0., 240., 53., -211., 209.],
                [-122., 167., -212., 227., -144., 61., 118.],
                [-63., -146., 200., 244., 168., -167., 116.],
                [-125., -147., 110., -253., -178., -250., -18.],
            ],
            [
                [57., 86., -50., 56., 92., 205., -78.],
                [-137., -156., -18., 248., -61., -239., 14.],
                [-248., -30., -50., -70., -251., 250., -83.],
                [-221., 67., 72., 59., -24., -154., 232.],
                [-144., -23., -74., 5., 93., 171., 205.],
                [46., -77., -38., -226., 246., 161., -17.],
            ],
            [
                [-153., -231., -236., 161., 126., 2., -22.],
                [-229., -41., 209., 164., 234., 160., 57.],
                [223., 254., -186., -162., -46., -160., -102.],
                [65., 30., 213., -253., 59., 224., -154.],
                [-82., -203., -177., 17., 31., -256., -246.],
                [176., -135., -65., 54., -56., 210., 76.],
            ],
            [
                [-10., -245., 168., 124., -14., -33., -178.],
                [25., -43., -39., 132., -89., 169., 179.],
                [187., -215., 32., -133., 87., -7., -168.],
                [-224., -215., -5., -230., -58., -162., 128.],
                [158., -137., -122., -100., -202., -83., 136.],
                [30., -185., -144., 250., 209., -40., 127.],
            ],
            [
                [-196., 108., -245., 122., 146., -228., 62.],
                [-1., -66., 160., 137., 13., -172., -21.],
                [244., 199., -164., 28., 119., -175., 198.],
                [-62., 253., -162., 195., -95., -230., -211.],
                [123., -72., -26., -107., -139., 64., 245.],
                [11., -126., -182., 108., -12., 184., -127.],
            ],
            [
                [-159., 126., 176., 161., 73., -111., -138.],
                [-187., 214., -217., -33., -223., -201., -212.],
                [-61., -120., -166., -172., -95., 53., 196.],
                [-33., 86., 134., -152., 154., -53., 74.],
                [186., -28., -154., -174., 141., -109., 217.],
                [82., 35., 252., 145., 181., 74., -87.],
            ],
        ],
        device,
    )?;

    let ids = Tensor::new(
        &[
            [
                [6_u32, 6, 4, 3, 4, 4, 6],
                [3, 3, 2, 4, 4, 4, 6],
                [3, 3, 0, 2, 4, 6, 4],
                [2, 5, 1, 2, 6, 6, 1],
                [2, 1, 6, 5, 3, 2, 3],
                [6, 1, 0, 1, 0, 2, 6],
            ],
            [
                [4, 6, 4, 3, 3, 3, 2],
                [4, 3, 2, 4, 4, 4, 6],
                [2, 3, 0, 2, 4, 6, 4],
                [6, 5, 1, 2, 6, 6, 1],
                [4, 1, 6, 5, 3, 2, 3],
                [1, 1, 0, 1, 0, 2, 6],
            ],
            [
                [3, 6, 4, 3, 3, 3, 2],
                [2, 3, 2, 4, 4, 4, 6],
                [4, 3, 0, 2, 4, 6, 4],
                [0, 5, 1, 2, 6, 6, 1],
                [6, 1, 6, 5, 3, 2, 3],
                [4, 1, 0, 1, 0, 2, 6],
            ],
            [
                [0, 6, 4, 3, 3, 3, 2],
                [5, 3, 2, 4, 4, 4, 6],
                [0, 3, 0, 2, 4, 6, 4],
                [3, 5, 1, 2, 6, 6, 1],
                [0, 1, 6, 5, 3, 2, 3],
                [3, 1, 0, 1, 0, 2, 6],
            ],
        ],
        device,
    )?;

    let hs = t.gather(&ids, 0)?;
    assert_eq!(
        hs.to_vec3::<f32>()?,
        &[
            [
                [-159_f32, 126., 168., 161., -14., -33., -138.],
                [-229., -41., -18., 132., -89., 169., -212.],
                [223., 254., 2., -70., 87., 53., -168.],
                [-221., 253., -212., 59., 154., -53., 118.],
                [-144., -146., -154., -107., 31., 171., -246.],
                [82., -147., -10., -253., -242., 161., -87.]
            ],
            [
                [-10., 126., 168., 161., 126., 2., -78.],
                [25., -41., -18., 132., -89., 169., -212.],
                [-248., 254., 2., -70., 87., 53., -168.],
                [-33., 253., -212., 59., 154., -53., 118.],
                [158., -146., -154., -107., 31., 171., -246.],
                [-125., -147., -10., -253., -242., 161., -87.]
            ],
            [
                [-153., 126., 168., 161., 126., 2., -78.],
                [-137., -41., -18., 132., -89., 169., -212.],
                [187., 254., 2., -70., 87., 53., -168.],
                [-246., 253., -212., 59., 154., -53., 118.],
                [186., -146., -154., -107., 31., 171., -246.],
                [30., -147., -10., -253., -242., 161., -87.]
            ],
            [
                [108., 126., 168., 161., 126., 2., -78.],
                [-1., -41., -18., 132., -89., 169., -212.],
                [-9., 254., 2., -70., 87., 53., -168.],
                [65., 253., -212., 59., 154., -53., 118.],
                [8., -146., -154., -107., 31., 171., -246.],
                [176., -147., -10., -253., -242., 161., -87.]
            ]
        ]
    );

    // Dim: 1
    let t = Tensor::new(
        &[
            [
                [-117_f32, -175., 69., -163.],
                [200., 242., -21., -67.],
                [179., 150., -126., -75.],
                [-118., 38., -138., -13.],
                [-221., 136., -185., 180.],
                [58., 182., -204., -149.],
            ],
            [
                [3., -148., -58., -154.],
                [-43., 45., -108., 4.],
                [-69., -249., -71., -21.],
                [80., 110., -152., -235.],
                [-88., 7., 92., -250.],
                [-186., 207., -242., 98.],
            ],
            [
                [238., 19., 64., -242.],
                [-150., -97., 218., 58.],
                [111., -233., 204., -212.],
                [-242., -232., 83., 42.],
                [153., 62., -251., 219.],
                [-117., 36., -119., 10.],
            ],
            [
                [215., 159., -169., -27.],
                [-83., 101., -88., 169.],
                [-205., 93., 225., -64.],
                [-162., 240., 214., 23.],
                [-112., 6., 21., 245.],
                [-38., 113., 93., 215.],
            ],
            [
                [91., -188., -148., 101.],
                [74., 203., -35., 55.],
                [-116., -130., -153., -96.],
                [58., 22., -45., -194.],
                [-221., -134., 73., 159.],
                [-203., -254., 31., 235.],
            ],
            [
                [105., -53., 61., 186.],
                [-195., 234., 75., -1.],
                [51., 139., 160., -108.],
                [-173., -167., 161., 19.],
                [83., -246., 156., -222.],
                [109., 39., -149., 137.],
            ],
        ],
        device,
    )?;

    let ids = Tensor::new(
        &[
            [[4_u32, 4, 4, 2]],
            [[0, 4, 4, 3]],
            [[1, 5, 3, 4]],
            [[0, 3, 3, 2]],
            [[1, 1, 5, 2]],
            [[1, 4, 5, 4]],
        ],
        device,
    )?;

    let hs = t.gather(&ids, 1)?;
    assert_eq!(
        hs.to_vec3::<f32>()?,
        &[
            [[-221., 136., -185., -75.]],
            [[3., 7., 92., -235.]],
            [[-150., 36., 83., 219.]],
            [[215., 240., 214., -64.]],
            [[74., 203., 31., -96.]],
            [[-195., -246., -149., -222.]]
        ]
    );

    // Dim: 2
    let t = Tensor::new(
        &[
            [[-162_f32, 202.], [-126., -39.], [35., -65.], [1., 80.]],
            [[37., 248.], [-191., 89.], [117., -40.], [-217., 220.]],
        ],
        device,
    )?;

    let ids = Tensor::new(&[[[1_u32], [0], [1], [1]], [[0], [1], [0], [1]]], device)?;

    let hs = t.gather(&ids, 2)?;
    assert_eq!(
        hs.to_vec3::<f32>()?,
        &[
            [[202.], [-126.], [-65.], [80.]],
            [[37.], [89.], [117.], [220.]]
        ]
    );

    let t = Tensor::new(
        &[
            [[-21_f32, -197.], [194., 122.]],
            [[255., -106.], [-191., 250.]],
            [[33., -117.], [43., 10.]],
            [[-130., 238.], [-217., -92.]],
        ],
        device,
    )?;

    let ids = Tensor::new(
        &[
            [[0_u32, 1], [1, 0]],
            [[1, 0], [0, 1]],
            [[0, 1], [0, 1]],
            [[1, 0], [1, 0]],
        ],
        device,
    )?;

    let hs = t.gather(&ids, 2)?;
    assert_eq!(
        hs.to_vec3::<f32>()?,
        &[
            [[-21., -197.], [122., 194.]],
            [[-106., 255.], [-191., 250.]],
            [[33., -117.], [43., 10.]],
            [[238., -130.], [-92., -217.]]
        ]
    );

    Ok(())
}

fn broadcasting(device: &Device) -> Result<()> {
    let t1 = Tensor::arange(0f32, 24f32, device)?.reshape((4, 2, 3))?;
    let t2 = Tensor::new(&[100f32, 200f32], device)?;
    let s = t1.broadcast_add(&t2.reshape((2, 1))?)?;
    assert_eq!(
        s.to_vec3::<f32>()?,
        &[
            [[100.0, 101.0, 102.0], [203.0, 204.0, 205.0]],
            [[106.0, 107.0, 108.0], [209.0, 210.0, 211.0]],
            [[112.0, 113.0, 114.0], [215.0, 216.0, 217.0]],
            [[118.0, 119.0, 120.0], [221.0, 222.0, 223.0]]
        ]
    );
    let s = t1.t()?.broadcast_add(&t2)?;
    assert_eq!(
        s.to_vec3::<f32>()?,
        &[
            [[100.0, 203.0], [101.0, 204.0], [102.0, 205.0]],
            [[106.0, 209.0], [107.0, 210.0], [108.0, 211.0]],
            [[112.0, 215.0], [113.0, 216.0], [114.0, 217.0]],
            [[118.0, 221.0], [119.0, 222.0], [120.0, 223.0]]
        ]
    );
    let s = t1.broadcast_sub(&t2.reshape((2, 1))?)?;
    assert_eq!(
        s.to_vec3::<f32>()?,
        &[
            [[-100.0, -99.0, -98.0], [-197.0, -196.0, -195.0]],
            [[-94.0, -93.0, -92.0], [-191.0, -190.0, -189.0]],
            [[-88.0, -87.0, -86.0], [-185.0, -184.0, -183.0]],
            [[-82.0, -81.0, -80.0], [-179.0, -178.0, -177.0]]
        ]
    );
    let s = t1.t()?.broadcast_sub(&t2)?;
    assert_eq!(
        s.to_vec3::<f32>()?,
        &[
            [[-100.0, -197.0], [-99.0, -196.0], [-98.0, -195.0]],
            [[-94.0, -191.0], [-93.0, -190.0], [-92.0, -189.0]],
            [[-88.0, -185.0], [-87.0, -184.0], [-86.0, -183.0]],
            [[-82.0, -179.0], [-81.0, -178.0], [-80.0, -177.0]]
        ]
    );
    // Test a narrowed version as this uses a layout start_offset.
    let t1 = t1.i(2..)?;
    let s = t1.broadcast_add(&t2.reshape((2, 1))?)?;
    assert_eq!(
        s.to_vec3::<f32>()?,
        &[
            [[112.0, 113.0, 114.0], [215.0, 216.0, 217.0]],
            [[118.0, 119.0, 120.0], [221.0, 222.0, 223.0]]
        ]
    );
    let s = t1.t()?.broadcast_add(&t2)?;
    assert_eq!(
        s.to_vec3::<f32>()?,
        &[
            [[112.0, 215.0], [113.0, 216.0], [114.0, 217.0]],
            [[118.0, 221.0], [119.0, 222.0], [120.0, 223.0]]
        ]
    );
    let s = t1.broadcast_sub(&t2.reshape((2, 1))?)?;
    assert_eq!(
        s.to_vec3::<f32>()?,
        &[
            [[-88.0, -87.0, -86.0], [-185.0, -184.0, -183.0]],
            [[-82.0, -81.0, -80.0], [-179.0, -178.0, -177.0]]
        ]
    );
    let s = t1.t()?.broadcast_sub(&t2)?;
    assert_eq!(
        s.to_vec3::<f32>()?,
        &[
            [[-88.0, -185.0], [-87.0, -184.0], [-86.0, -183.0]],
            [[-82.0, -179.0], [-81.0, -178.0], [-80.0, -177.0]]
        ]
    );
    let t3 = Tensor::new(1f32, device)?.broadcast_div(&t2)?;
    let s = t1.broadcast_mul(&t2.reshape((2, 1))?)?;
    let s_div = t1.broadcast_div(&t3.reshape((2, 1))?)?;
    assert_eq!(
        s.to_vec3::<f32>()?,
        &[
            [[1200.0, 1300.0, 1400.0], [3000.0, 3200.0, 3400.0]],
            [[1800.0, 1900.0, 2000.0], [4200.0, 4400.0, 4600.0]]
        ]
    );
    assert_eq!(s.to_vec3::<f32>()?, s_div.to_vec3::<f32>()?,);
    let s = t1.t()?.broadcast_mul(&t2)?;
    let s_div = t1.t()?.broadcast_div(&t3)?;
    assert_eq!(
        s.to_vec3::<f32>()?,
        &[
            [[1200.0, 3000.0], [1300.0, 3200.0], [1400.0, 3400.0]],
            [[1800.0, 4200.0], [1900.0, 4400.0], [2000.0, 4600.0]]
        ]
    );
    assert_eq!(s.to_vec3::<f32>()?, s_div.to_vec3::<f32>()?,);
    Ok(())
}

fn randn(device: &Device) -> Result<()> {
    let tensor = Tensor::randn(0f32, 1f32, (5, 3), device)?;
    assert_eq!(tensor.dims(), [5, 3]);
    // Check that the seed gets updated by checking that
    // a new series of numbers is generated each time
    let tensor2 = Tensor::randn(0f32, 1f32, (5, 3), device)?;
    assert_ne!(tensor.to_vec2::<f32>()?, tensor2.to_vec2::<f32>()?);
    let tensor = Tensor::rand(0f32, 1f32, (5, 3), device)?;
    assert_eq!(tensor.dims(), [5, 3]);
    // Check that the seed gets updated by checking that
    // a new series of numbers is generated each time
    let tensor2 = Tensor::rand(0f32, 1f32, (5, 3), device)?;
    assert_ne!(tensor.to_vec2::<f32>()?, tensor2.to_vec2::<f32>()?);
    // We do not expect deterministic elements at any index.
    // There once was a bug that had a deterministic zero element in evenly sized tensors.
    const N: usize = 2;
    let v = (0..100)
        .map(|_| Tensor::randn(0f32, 1f32, N, device).and_then(|t| t.to_vec1::<f32>()))
        .collect::<Result<Vec<_>>>()?;
    assert!(
        (0..N).all(|i| v.windows(2).any(|pair| pair[0][i] != pair[1][i])),
        "There are deterministic values in the randn tensors"
    );
    let v = (0..100)
        .map(|_| Tensor::rand(0f32, 1f32, N, device).and_then(|t| t.to_vec1::<f32>()))
        .collect::<Result<Vec<_>>>()?;
    assert!(
        (0..N).all(|i| v.windows(2).any(|pair| pair[0][i] != pair[1][i])),
        "There are deterministic values in the rand tensors"
    );
    Ok(())
}

fn zero_dim(device: &Device) -> Result<()> {
    let t = Tensor::zeros((4, 0, 1), DType::F32, device)?;
    assert_eq!(t.dims3()?, (4, 0, 1));
    let t2 = Tensor::zeros((4, 3, 1), DType::F32, device)?;
    let t_cat = Tensor::cat(&[&t, &t2], 1)?;
    assert_eq!(t_cat.dims3()?, (4, 3, 1));
    let t_cat = Tensor::cat(&[&t, &t], 1)?;
    assert_eq!(t_cat.dims3()?, (4, 0, 1));
    let t_unary = t.sqrt()?;
    assert_eq!(t_unary.dims3()?, (4, 0, 1));
    let t_plus = (&t + 1.)?;
    assert_eq!(t_plus.dims3()?, (4, 0, 1));
    let t_mm = t2.matmul(&t.t()?)?;
    assert_eq!(t_mm.dims3()?, (4, 3, 0));
    let t_mm = t.matmul(&t2.t()?)?;
    assert_eq!(t_mm.dims3()?, (4, 0, 3));
    let t_mm = t.t()?.matmul(&t)?;
    assert_eq!(t_mm.dims3()?, (4, 1, 1));
    Ok(())
}

test_device!(zeros, zeros_cpu, zeros_gpu, zeros_metal);
test_device!(ones, ones_cpu, ones_gpu, ones_metal);
test_device!(full, full_cpu, full_gpu, full_metal);
test_device!(const_set, cs_cpu, cs_gpu, cs_metal);
test_device!(arange, arange_cpu, arange_gpu, arange_metal);
test_device!(add_mul, add_mul_cpu, add_mul_gpu, add_mul_metal);
test_device!(tensor_2d, tensor_2d_cpu, tensor_2d_gpu, tensor_2d_metal);
test_device!(narrow, narrow_cpu, narrow_gpu, narrow_metal);
test_device!(broadcast, broadcast_cpu, broadcast_gpu, broadcast_metal);
test_device!(slice_set, ss_cpu, ss_gpu, ss_metal);
test_device!(cat, cat_cpu, cat_gpu, cat_metal);
test_device!(sum, sum_cpu, sum_gpu, sum_metal);
test_device!(min, min_cpu, min_gpu, min_metal);
test_device!(max, max_cpu, max_gpu, max_metal);
test_device!(argmax, argmax_cpu, argmax_gpu, argmax_metal);
test_device!(argmin, argmin_cpu, argmin_gpu, argmin_metal);
test_device!(transpose, transpose_cpu, transpose_gpu, transpose_metal);
test_device!(unary_op, unary_op_cpu, unary_op_gpu, unary_op_metal);
test_device!(binary_op, binary_op_cpu, binary_op_gpu, binary_op_metal);
test_device!(ternary_op, ternary_op_cpu, ternary_op_gpu, ternary_op_metal);
test_device!(embeddings, embeddings_cpu, embeddings_gpu, embeddings_metal);
test_device!(cmp, cmp_cpu, cmp_gpu, cmp_metal);
test_device!(
    broadcasting,
    broadcasting_cpu,
    broadcasting_gpu,
    broadcasting_metal
);
test_device!(
    index_select,
    index_select_cpu,
    index_select_gpu,
    index_select_metal
);
test_device!(index_add, index_add_cpu, index_add_gpu, index_add_metal);
test_device!(gather, gather_cpu, gather_gpu, gather_metal);
test_device!(scatter, scatter_cpu, scatter_gpu, scatter_metal);
test_device!(
    slice_scatter,
    slice_scatter_cpu,
    slice_scatter_gpu,
    slice_scatter_metal
);
test_device!(randn, randn_cpu, randn_gpu, randn_metal);
test_device!(clamp, clamp_cpu, clamp_gpu, clamp_metal);
test_device!(asort, asort_cpu, asort_gpu, asort_metal);
test_device!(asort_big, asort_big_cpu, asort_big_gpu, asort_big_metal);
test_device!(var, var_cpu, var_gpu, var_metal);
test_device!(zero_dim, zero_dim_cpu, zero_dim_gpu, zero_dim_metal);

fn tensor_send_sync(device: &Device) -> Result<()> {
    let tensor = Tensor::new(vec![1.0f32, 2.0, 3.0], device)?;

    for _ in 0..10 {
        let tensor = tensor.clone();
        std::thread::spawn(move || {
            let new = tensor.add(&tensor).unwrap();
            let result: Vec<f32> = new.to_vec1().unwrap();
            assert_eq!(result, vec![2.0f32, 4.0, 6.0]);
        });
    }
    let result: Vec<f32> = tensor.to_vec1().unwrap();
    assert_eq!(result, vec![1.0f32, 2.0, 3.0]);

    let tensor = Tensor::new(vec![1.0f32, 2.0, 3.0], device)?;
    tensor.device().synchronize().unwrap();

    let new = std::thread::spawn(move || {
        let new = tensor.add(&tensor).unwrap();
        new.device().synchronize().unwrap();
        new
    })
    .join()
    .unwrap();
    let result: Vec<f32> = new.to_vec1().unwrap();
    assert_eq!(result, vec![2.0f32, 4.0, 6.0]);

    Ok(())
}
test_device!(
    tensor_send_sync,
    tensor_send_sync_cpu,
    tensor_send_sync_gpu,
    tensor_send_sync_metal
);

// There was originally a bug on the CPU implementation for randn
// https://github.com/huggingface/candle/issues/381
#[test]
fn randn_hasneg() -> Result<()> {
    let t = Tensor::randn(0f32, 1f32, 200, &Device::Cpu)?.to_vec1::<f32>()?;
    if t.iter().all(|&v| v >= 0.) {
        candle_core::bail!("all values in tensors are non-negative")
    }
    Ok(())
}

#[test]
fn pad_with_same() -> Result<()> {
    let t = Tensor::arange(1f32, 5f32, &Device::Cpu)?.reshape((2, 2))?;
    let t0 = t.pad_with_same(0, 1, 2)?;
    assert_eq!(
        t0.to_vec2::<f32>()?,
        [[1.0, 2.0], [1.0, 2.0], [3.0, 4.0], [3.0, 4.0], [3.0, 4.0]]
    );
    let t1 = t.pad_with_same(1, 1, 2)?;
    assert_eq!(
        t1.to_vec2::<f32>()?,
        [[1.0, 1.0, 2.0, 2.0, 2.0], [3.0, 3.0, 4.0, 4.0, 4.0]]
    );
    Ok(())
}

#[test]
fn i64_abs() -> Result<()> {
    let t = Tensor::new(&[-42i64, 1337], &Device::Cpu)?;
    let t = t.abs()?;
    assert_eq!(t.to_vec1::<i64>()?, [42, 1337]);
    Ok(())
}

#[test]
fn tril_triu_eye() -> Result<()> {
    let t = Tensor::tril2(4, DType::F32, &Device::Cpu)?;
    assert_eq!(
        t.to_vec2::<f32>()?,
        [
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 1.0, 0.0, 0.0],
            [1.0, 1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0, 1.0]
        ],
    );
    let t = Tensor::triu2(4, DType::F32, &Device::Cpu)?;
    assert_eq!(
        t.to_vec2::<f32>()?,
        [
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0]
        ]
    );
    let t = Tensor::eye(4, DType::F32, &Device::Cpu)?;
    assert_eq!(
        t.to_vec2::<f32>()?,
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0]
        ]
    );
    Ok(())
}

#[test]
fn cumsum() -> Result<()> {
    let t = &[3f32, 1., 4., 1., 5.];
    let t = Tensor::new(t, &Device::Cpu)?;
    assert_eq!(t.cumsum(0)?.to_vec1::<f32>()?, [3., 4., 8., 9., 14.]);
    let t = t.unsqueeze(1)?;
    assert_eq!(
        t.cumsum(0)?.to_vec2::<f32>()?,
        [[3.0], [4.0], [8.0], [9.0], [14.0]]
    );
    assert_eq!(
        t.cumsum(1)?.to_vec2::<f32>()?,
        [[3.0], [1.0], [4.0], [1.0], [5.0]]
    );
    let t = &[[3f32, 1., 4., 1., 5.], [2., 1., 7., 8., 2.]];
    let t = Tensor::new(t, &Device::Cpu)?;
    assert_eq!(
        t.cumsum(1)?.to_vec2::<f32>()?,
        [[3.0, 4.0, 8.0, 9.0, 14.0], [2.0, 3.0, 10.0, 18.0, 20.0]],
    );
    assert_eq!(
        t.cumsum(0)?.to_vec2::<f32>()?,
        [[3.0, 1.0, 4.0, 1.0, 5.0], [5.0, 2.0, 11.0, 9.0, 7.0]]
    );
    Ok(())
}

/// A helper function for floating point comparison. Both a and b must be 1D Tensor and contains the same amount of data.
/// Assertion passes if the difference of all pairs of a and b is smaller than epsilon.
fn assert_close(a: &Tensor, b: &Tensor, epsilon: f64) -> Result<()> {
    let a_vec: Vec<f64> = a.to_vec1()?;
    let b_vec: Vec<f64> = b.to_vec1()?;

    assert_eq!(a_vec.len(), b_vec.len());
    for (a, b) in a_vec.iter().zip(b_vec.iter()) {
        assert!((a - b).abs() < epsilon);
    }
    Ok(())
}

#[test]
fn log_sum_exp() -> Result<()> {
    let input = Tensor::new(
        &[
            [[1f64, 2., 3.], [4., 5., 6.]],
            [[-1000.0, -999.0, -1001.0], [1000.0, 999.0, 1001.0]],
        ],
        &Device::Cpu,
    )?;

    let output = input.log_sum_exp(D::Minus1)?;
    // The expectations obtained from pytorch.
    let expected = Tensor::new(&[[3.4076, 6.4076], [-998.5924, 1001.4076]], &Device::Cpu)?;
    assert_eq!(output.dims(), expected.dims());
    assert_close(&output.flatten_all()?, &expected.flatten_all()?, 0.00001)?;

    assert_eq!(
        input.log_sum_exp((0, 1))?.to_vec1::<f64>()?,
        [1000.0, 999.0, 1001.0]
    );
    assert_eq!(
        input.log_sum_exp(())?.to_vec3::<f64>()?,
        input.to_vec3::<f64>()?
    );

    Ok(())
}

#[test]
fn pow() -> Result<()> {
    let lhs = Tensor::new(&[[1f32, 2., 3.], [4., 5., 6.]], &Device::Cpu)?;
    let rhs = (&lhs - 2.)?;
    let res = lhs.pow(&rhs)?;
    assert_eq!(
        test_utils::to_vec2_round(&res, 3)?,
        [[1.0, 1.0, 3.0], [16.0, 125.0, 1296.0]]
    );
    Ok(())
}

#[test]
fn test_flip_1d() -> Result<()> {
    // 1D: [0, 1, 2, 3, 4]
    let t = Tensor::arange(0.0, 5.0, &Device::Cpu)?.reshape((5,))?;
    let flipped = t.flip(&[0])?;
    // Expected: [4, 3, 2, 1, 0]
    let expected = Tensor::from_vec(vec![4.0, 3.0, 2.0, 1.0, 0.0], (5,), &Device::Cpu)?;
    candle_core::test_utils::assert_tensor_eq(&flipped, &expected)?;
    Ok(())
}

#[test]
fn test_flip_2d() -> Result<()> {
    // 2D:
    // [[0, 1, 2],
    //  [3, 4, 5]]
    let t = Tensor::arange(0.0, 6.0, &Device::Cpu)?.reshape((2, 3))?;
    let flipped = t.flip(&[0, 1])?;
    // Expected:
    // [[5, 4, 3],
    //  [2, 1, 0]]
    let expected = Tensor::from_vec(vec![5.0, 4.0, 3.0, 2.0, 1.0, 0.0], (2, 3), &Device::Cpu)?;
    candle_core::test_utils::assert_tensor_eq(&flipped, &expected)?;
    Ok(())
}

#[test]
fn test_flip_3d_channels() -> Result<()> {
    // 3D:
    // [[[0,1,2],
    //   [3,4,5]],
    //
    //  [[6,7,8],
    //   [9,10,11]]]
    let t = Tensor::arange(0.0, 12.0, &Device::Cpu)?.reshape((2, 2, 3))?;
    let flipped = t.flip(&[2])?;
    // Expected:
    // [[[2,1,0],
    //   [5,4,3]],
    //
    //  [[8,7,6],
    //   [11,10,9]]]
    let expected = Tensor::from_vec(
        vec![2.0, 1.0, 0.0, 5.0, 4.0, 3.0, 8.0, 7.0, 6.0, 11.0, 10.0, 9.0],
        (2, 2, 3),
        &Device::Cpu,
    )?;
    candle_core::test_utils::assert_tensor_eq(&flipped, &expected)?;
    Ok(())
}

#[test]
fn tensor_new() -> Result<()> {
    let t1 = Tensor::new(vec![1f32, 2.0, 3.0], &Device::Cpu)?;
    assert_eq!(t1.to_vec1::<f32>()?, [1.0, 2.0, 3.0]);
    let t2 = Tensor::new(vec![vec![1f32, 2., 3.], vec![4., 5., 6.]], &Device::Cpu)?;
    assert_eq!(t2.to_vec2::<f32>()?, [[1., 2., 3.], [4., 5., 6.]]);
    let t3 = Tensor::new(
        vec![
            vec![vec![1f32, 2., 3.], vec![4., 5., 6.]],
            vec![vec![3f32, 1., 4.], vec![1., 5., 9.]],
        ],
        &Device::Cpu,
    )?;
    assert_eq!(
        t3.to_vec3::<f32>()?,
        [
            [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
            [[3.0, 1.0, 4.0], [1.0, 5.0, 9.0]]
        ]
    );
    Ok(())
}

#[test]
fn tensor_norm() -> Result<()> {
    let t = Tensor::new(&[[3., 4.], [0., 0.]], &Device::Cpu)?;
    let norm = t.norm()?;
    assert_eq!(norm.to_scalar::<f64>()?, 5.);
    Ok(())
}

#[cfg(feature = "cuda")]
#[test]
fn transfers_cuda_to_device() -> Result<()> {
    use rand::seq::SliceRandom;

    let devices = cudarc::driver::safe::CudaContext::device_count()
        .map_err(candle_core::cuda::CudaError::from)?;
    if devices < 2 {
        return Ok(());
    }
    let first = Device::new_cuda(0)?;

    let mut data: Vec<u32> = (0..262144).collect();
    let mut rng = rand::rng();
    data.shuffle(&mut rng);

    let t1 = Tensor::from_vec(data, (512, 512), &first)?;
    let second = Device::new_cuda(1)?;
    let t2 = t1.to_device(&second)?;

    assert_ne!(
        t1.device().as_cuda_device()?.id(),
        t2.device().as_cuda_device()?.id()
    );
    Ok(())
}

#[cfg(feature = "cuda")]
#[test]
fn allocates_twice_when_transferring_to_same_device() -> Result<()> {
    use std::{ops::Deref, sync::RwLockReadGuard};

    use candle_core::Storage;
    use rand::seq::SliceRandom;

    let first = Device::new_cuda(0)?;
    let second = Device::new_cuda(0)?;

    let mut data: Vec<u32> = (0..262144).collect();
    let mut rng = rand::rng();
    data.shuffle(&mut rng);

    let t1 = Tensor::from_vec(data, (512, 512), &first)?;
    let t2 = t1.to_device(&second)?;

    let (storage1, _) = t1.storage_and_layout();
    let (storage2, _) = t2.storage_and_layout();
    let extract = |s: RwLockReadGuard<'_, Storage>| match &s.deref() {
        Storage::Cuda(c) => {
            use cudarc::driver::DevicePtr;
            let slice = c.as_cuda_slice::<u32>().unwrap();
            let ptr = slice.device_ptr(slice.stream()).0;
            ptr
        }
        _ => unimplemented!(),
    };
    let id1 = extract(storage1);
    let id2 = extract(storage2);
    assert_ne!(id1, id2);
    Ok(())
}
