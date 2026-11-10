#![allow(clippy::approx_constant)]
use anyhow::{Context, Result};
use candle_core::{test_device, test_utils, DType, Device, Shape, Tensor, Var};

fn simple_grad(device: &Device) -> Result<()> {
    let x = Var::new(&[3f32, 1., 4.], device)?;
    let x = x.as_tensor();
    let y = (((x * x)? + x * 5f64)? + 4f64)?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(x.to_vec1::<f32>()?, [3., 1., 4.]);
    // y = x^2 + 5.x + 4
    assert_eq!(y.to_vec1::<f32>()?, [28., 10., 40.]);
    // dy/dx = 2.x + 5
    assert_eq!(grad_x.to_vec1::<f32>()?, [11., 7., 13.]);
    Ok(())
}

fn sum_grad(device: &Device) -> Result<()> {
    let x = Var::new(&[3f32, 1., 4.], device)?;
    let x = x.as_tensor();
    let y = (x.sqr()?.sum_keepdim(0)? * 2.)?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(y.to_vec1::<f32>()?, [52.]);
    // y = 2.x^2 so dy/dx = 4.x
    assert_eq!(grad_x.to_vec1::<f32>()?, &[12., 4., 16.]);

    // Same test as before but squeezing on the last dimension.
    let y = (x.sqr()?.sum_keepdim(0)? * 2.)?.squeeze(0)?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(y.to_scalar::<f32>()?, 52.);
    // y = 2.x^2 so dy/dx = 4.x
    assert_eq!(grad_x.to_vec1::<f32>()?, &[12., 4., 16.]);
    Ok(())
}

fn matmul_grad(device: &Device) -> Result<()> {
    let data: Vec<_> = (0..12).map(|i| i as f32).collect();
    let x = Var::from_slice(&data, (2, 2, 3), device)?;
    let data: Vec<_> = (0..12).map(|i| i as f32).collect();
    let y = Var::from_slice(&data, (2, 3, 2), device)?;
    let c = x.matmul(&y)?;
    let grads = c.backward()?;
    let grad_x = grads.get(&x).context("no grad for x")?;
    let grad_y = grads.get(&y).context("no grad for y")?;
    assert_eq!(grad_x.shape(), &Shape::from((2, 2, 3)));
    assert_eq!(grad_y.shape(), &Shape::from((2, 3, 2)));
    assert_eq!(
        &*grad_x.to_vec3::<f32>()?,
        &[
            [[1., 5., 9.], [1., 5., 9.]],
            [[13., 17., 21.], [13., 17., 21.]]
        ]
    );
    assert_eq!(
        &*grad_y.to_vec3::<f32>()?,
        &[
            [[3., 3.], [5., 5.], [7., 7.]],
            [[15., 15.], [17., 17.], [19., 19.]]
        ]
    );
    Ok(())
}

// The simplest gradient descent, using scalar variable.
fn grad_descent(device: &Device) -> Result<()> {
    let x = Var::new(0f32, device)?;
    let learning_rate = 0.1;
    for _step in 0..100 {
        let xt = x.as_tensor();
        let c = ((xt - 4.2)? * (xt - 4.2)?)?;
        let grads = c.backward()?;
        let x_grad = grads.get(&x).context("no grad for x")?;
        x.set(&(xt - x_grad * learning_rate)?)?
    }
    assert_eq!(x.to_scalar::<f32>()?, 4.199999);
    Ok(())
}

fn unary_grad(device: &Device) -> Result<()> {
    let x = Var::new(&[3f32, 1., 4., 0.15], device)?;
    let x = x.as_tensor();
    let y = (x.log()? + 1.)?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [2.0986, 1.0, 2.3863, -0.8971]
    );
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 4)?,
        [0.3333, 1.0, 0.25, 6.6667]
    );
    let y = x.exp()?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [20.0855, 2.7183, 54.5982, 1.1618]
    );
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 4)?,
        [20.0855, 2.7183, 54.5982, 1.1618]
    );
    let y = x.exp()?.sqr()?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 3)?,
        [403.429, 7.389, 2980.958, 1.35]
    );
    // exp(x)^2 = exp(2*x)
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 2)?,
        [806.86, 14.78, 5961.92, 2.7]
    );
    let y = x.sin()?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [0.1411, 0.8415, -0.7568, 0.1494],
    );
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 4)?,
        [-0.99, 0.5403, -0.6536, 0.9888],
    );
    let y = x.cos()?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [-0.99, 0.5403, -0.6536, 0.9888],
    );
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 4)?,
        [-0.1411, -0.8415, 0.7568, -0.1494],
    );
    let y = x.sqr()?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(y.to_vec1::<f32>()?, [9.0, 1.0, 16.0, 0.0225]);
    assert_eq!(grad_x.to_vec1::<f32>()?, [6.0, 2.0, 8.0, 0.3]);
    let y = x.sqr()?.sqrt()?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(y.to_vec1::<f32>()?, [3.0, 1.0, 4.0, 0.15]);
    assert_eq!(test_utils::to_vec1_round(grad_x, 4)?, [1.0, 1.0, 1.0, 1.0]);
    let y = x.neg()?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(y.to_vec1::<f32>()?, [-3.0, -1.0, -4.0, -0.15]);
    assert_eq!(grad_x.to_vec1::<f32>()?, [-1.0, -1.0, -1.0, -1.0]);
    let y = x.affine(0.2, 1.)?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(y.to_vec1::<f32>()?, [1.6, 1.2, 1.8, 1.03]);
    assert_eq!(grad_x.to_vec1::<f32>()?, [0.2, 0.2, 0.2, 0.2]);
    let y = Tensor::new(1f32, device)?.broadcast_div(x)?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [0.3333, 1.0, 0.25, 6.6667]
    );
    assert_eq!(
        grad_x.to_vec1::<f32>()?,
        [-0.11111111, -1.0, -0.0625, -44.444443],
    );
    let y = x.broadcast_div(&Tensor::new(0.5f32, device)?)?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(y.to_vec1::<f32>()?, [6., 2., 8., 0.3]);
    assert_eq!(grad_x.to_vec1::<f32>()?, [2., 2., 2., 2.]);

    let x = Var::new(&[3f32, 1., 4., 0.15], device)?;
    let y = x.powf(2.5)?;
    let grads = y.backward()?;
    let grad_x = grads.get(&x).context("no grad for x")?;
    assert_eq!(test_utils::to_vec1_round(&y, 2)?, [15.59, 1.0, 32.0, 0.01]);
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 2)?,
        [12.99, 2.5, 20.0, 0.15]
    );

    let y = x.tanh()?;
    let grads = y.backward()?;
    let grad_x = grads.get(&x).context("no grad for x")?;
    assert_eq!(test_utils::to_vec1_round(&y, 2)?, [1.0, 0.76, 1.0, 0.15]);
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 2)?,
        [0.01, 0.42, 0.0, 0.98],
    );

    // testing compared to pytorch nn.GELU(approximate = 'tanh')
    let y = x.gelu()?;
    let grads = y.backward()?;
    let grad_x = grads.get(&x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [2.9964, 0.8412, 3.9999, 0.0839]
    );
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 4)?,
        [1.0116, 1.0830, 1.0003, 0.6188],
    );

    // Testing compared to pytorch torch.erf
    //
    // import torch
    // x = torch.tensor([3.0, 1.0, 4.0, 0.15], requires_grad=True)
    // y = x.erf()
    // print(y)
    // loss = y.sum()
    // loss.backward()
    // print(x.grad)
    let y = x.erf()?;
    let grads = y.backward()?;
    let grad_x = grads.get(&x).context("no grad for x")?;
    assert_eq!(test_utils::to_vec1_round(&y, 4)?, [1.0, 0.8427, 1.0, 0.168]);
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 4)?,
        [0.0001, 0.4151, 0.0, 1.1033],
    );

    // Testing compared to pytorch nn.GELU(approximate = 'none')
    //
    // import torch
    // import torch.nn.functional as F
    // x = torch.tensor([3.0, 1.0, 4.0, 0.15], requires_grad=True)
    // y = F.gelu(x, approximate='none')
    // print(y)
    // loss = y.sum()
    // loss.backward()
    // print(x.grad)
    let y = x.gelu_erf()?;
    let grads = y.backward()?;
    let grad_x = grads.get(&x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [2.9960, 0.8413, 3.9999, 0.0839]
    );
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 4)?,
        [1.0119, 1.0833, 1.0005, 0.6188],
    );

    // Testing compared to pytorch elu
    //
    // import torch
    // import torch.nn.functional as F
    // x = torch.tensor([-1.0, 0.0, -2.0, 3.0], requires_grad=True)
    // y = F.elu(x, alpha=2.0)
    // print(y)
    // loss = y.min
    // loss = y.sum()
    // loss.backward()
    // print(x.grad)
    let elu_x = Var::new(&[-1.0f32, 0., -2., 3.], device)?;
    let y = elu_x.elu(2.)?;
    let grads = y.backward()?;
    let grad_x = grads.get(&elu_x).context("no grad for x")?;

    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [-1.2642, 0.0000, -1.7293, 3.0000]
    );
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 4)?,
        [0.7358, 2.0000, 0.2707, 1.0000]
    );

    // testing compared to pytorch nn.Silu()
    let y = x.silu()?;
    let grads = y.backward()?;
    let grad_x = grads.get(&x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec1_round(&y, 4)?,
        [2.8577, 0.7311, 3.9281, 0.0806]
    );
    assert_eq!(
        test_utils::to_vec1_round(grad_x, 4)?,
        [1.0881, 0.9277, 1.0527, 0.5747],
    );

    if device.is_cpu() {
        let x = Var::new(&[[[1f32, 2., 3.], [4., 5., 6.], [7., 8., 9.]]], device)?;
        let y = x.interpolate1d(12)?.reshape(36)?;

        let z = Tensor::new(
            &[
                1_f32, 02., 03., 04., 05., 06., 07., 08., 09., 10., 11., 12., 13., 14., 15., 16.,
                17., 18., 19., 20., 21., 22., 23., 24., 25., 26., 27., 28., 29., 30., 31., 32.,
                33., 34., 35., 36.,
            ],
            device,
        )?;

        let loss = y.unsqueeze(1)?.transpose(0, 1)?.matmul(&z.unsqueeze(1)?)?;
        let grads = loss.backward()?;
        let grad_x = grads.get(&x).context("no grad for x")?;

        assert_eq!(
            test_utils::to_vec3_round(grad_x, 4)?,
            [[[10_f32, 26., 42.], [58., 74., 90.], [106., 122., 138.]]]
        );
    }

    // manually checked: see comments
    let x = Var::new(&[[[[1f32, 2., 3.], [4., 5., 6.], [7., 8., 9.]]]], device)?;
    let y = x.interpolate2d(6, 6)?.reshape(36)?;

    let z = Tensor::new(
        &[
            1_f32, 02., 03., 04., 05., 06., 07., 08., 09., 10., 11., 12., 13., 14., 15., 16., 17.,
            18., 19., 20., 21., 22., 23., 24., 25., 26., 27., 28., 29., 30., 31., 32., 33., 34.,
            35., 36.,
        ],
        device,
    )?;
    // gradient should be
    // row 1
    // 1+2+7+8 = 18
    // 3+4+9+10 = 26
    // 5+6+11+12 = 34
    // row 2
    // 13+14+19+20 = 66
    // 15+16+21+22 = 74
    // 17+18+23+24 = 82
    // row 3
    // 25+26+31+32 = 114
    // 27+28+33+34 = 122
    // 29+30+35+36 = 130
    let loss = y.unsqueeze(1)?.transpose(0, 1)?.matmul(&z.unsqueeze(1)?)?;

    let grads = loss.backward()?;

    let grad_x = grads.get(&x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec2_round(&grad_x.flatten(0, 2)?, 4)?,
        [[18_f32, 26., 34.], [66., 74., 82.], [114., 122., 130.]]
    );

    // manually checked: see comments
    let x = Var::new(&[[[[1f32, 2.], [4., 5.]]]], device)?;
    let y = x.interpolate2d(6, 6)?.reshape(36)?;

    let z = Tensor::new(
        &[
            1_f32, 02., 03., 04., 05., 06., 07., 08., 09., 10., 11., 12., 13., 14., 15., 16., 17.,
            18., 19., 20., 21., 22., 23., 24., 25., 26., 27., 28., 29., 30., 31., 32., 33., 34.,
            35., 36.,
        ],
        device,
    )?;
    // gradient should be
    // row 1
    // 1+2+3+7+8+9+13+14+15 = 72
    // 4+5+6+10+11+12+16+17+18 = 99
    // row 2
    // 19+20+21+25+26+27+31+32+33 = 234
    // 22+23+24+28+29+30+34+35+36 = 243
    let loss = y.unsqueeze(1)?.transpose(0, 1)?.matmul(&z.unsqueeze(1)?)?;

    let grads = loss.backward()?;

    let grad_x = grads.get(&x).context("no grad for x")?;
    assert_eq!(
        test_utils::to_vec2_round(&grad_x.flatten(0, 2)?, 4)?,
        [[72_f32, 99.], [234., 261.]]
    );

    // manually checked: see comments
    let x = Var::new(&[[[[1f32, 2.], [4., 5.]], [[6f32, 7.], [8., 9.]]]], device)?;

    let y = x.interpolate2d(4, 4)?.reshape(32)?;

    #[rustfmt::skip]
    let z = Tensor::new(
        &[
            1_f32, 02., 03., 04.,
            05.,   06., 07., 08.,
            09.,   10., 11., 12.,
            13.,   14., 15., 16.,
            17.,   18., 19., 20.,
            21.,   22., 23., 24.,
            25.,   26., 27., 28.,
            29.,   30., 31., 32.
        ],
        device,
    )?;
    // gradient should be
    // m1r1
    // 1+2+5+6=14
    // 3+4+7+8=22
    // m1r2
    // 9+10+13+14=46
    // 11+12+15+16=54
    // m2r1
    // 17+18+21+22=78
    // 19+20+23+24=86
    // m2r2
    // 25+26+29+30=110
    // 27+28+31+32=118
    let loss = y.unsqueeze(1)?.transpose(0, 1)?.matmul(&z.unsqueeze(1)?)?;

    let grads = loss.backward()?;

    let grad_x = grads.get(&x).context("no grad for x")?;

    assert_eq!(
        test_utils::to_vec3_round(&grad_x.flatten(0, 1)?, 4)?,
        [[[14_f32, 22.], [46., 54.]], [[78., 86.], [110., 118.]]]
    );

    // manually checked: see comments
    let x = Var::new(
        &[[[[1f32, 2.], [4., 5.]]], [[[6f32, 7.], [8., 9.]]]],
        device,
    )?;

    let y = x.interpolate2d(4, 4)?.reshape(32)?;

    #[rustfmt::skip]
       let z = Tensor::new(
           &[
               1_f32, 02., 03., 04.,
               05.,   06., 07., 08.,
               09.,   10., 11., 12.,
               13.,   14., 15., 16.,
               17.,   18., 19., 20.,
               21.,   22., 23., 24.,
               25.,   26., 27., 28.,
               29.,   30., 31., 32.
           ],
           device,
       )?;
    // gradient should be
    // m1r1
    // 1+2+5+6=14
    // 3+4+7+8=22
    // m1r2
    // 9+10+13+14=46
    // 11+12+15+16=54
    // m2r1
    // 17+18+21+22=78
    // 19+20+23+24=86
    // m2r2
    // 25+26+29+30=110
    // 27+28+31+32=118
    let loss = y.unsqueeze(1)?.transpose(0, 1)?.matmul(&z.unsqueeze(1)?)?;

    let grads = loss.backward()?;

    let grad_x = grads.get(&x).context("no grad for x")?;

    assert_eq!(
        test_utils::to_vec3_round(&grad_x.flatten(0, 1)?, 4)?,
        [[[14_f32, 22.], [46., 54.]], [[78., 86.], [110., 118.]]]
    );
    Ok(())
}

fn binary_grad(device: &Device) -> Result<()> {
    let x = Var::new(&[3f32, 1., -4., -1.], device)?;
    let x = x.as_tensor();
    // leaky relu
    let y = x.maximum(&(x * 0.1)?)?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(x.to_vec1::<f32>()?, [3., 1., -4., -1.]);
    assert_eq!(y.to_vec1::<f32>()?, [3., 1., -0.4, -0.1]);
    assert_eq!(grad_x.to_vec1::<f32>()?, [1., 1., 0.1, 0.1]);

    let y = x.minimum(&(x * 0.1)?)?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(y.to_vec1::<f32>()?, [0.3, 0.1, -4., -1.]);
    assert_eq!(grad_x.to_vec1::<f32>()?, [0.1, 0.1, 1., 1.]);

    // This one is easy to mess up, we want the gradient to be one as it is the identity function.
    let y = x.minimum(x)?;
    let grads = y.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    assert_eq!(y.to_vec1::<f32>()?, [3., 1., -4., -1.]);
    assert_eq!(grad_x.to_vec1::<f32>()?, [1., 1., 1., 1.]);

    let x_var = Var::new(&[3f32, 1., -4., -1., 5., 9.], device)?;
    let x = x_var.as_tensor();
    let y_var = Var::new(&[2f32, 7., 1.], device)?;
    let y = y_var.as_tensor();

    let ss = x
        .reshape((2, 3))?
        .slice_scatter0(&y.reshape((1, 3))?, 1)?
        .sqr()?;
    let grads = ss.backward()?;
    let grad_x = grads.get(x).context("no grad for x")?;
    let grad_y = grads.get(y).context("no grad for y")?;
    assert_eq!(ss.to_vec2::<f32>()?, [[9., 1., 16.], [4., 49., 1.]]);
    assert_eq!(grad_x.to_vec1::<f32>()?, [6.0, 2.0, -8.0, 0.0, 0.0, 0.0]);
    assert_eq!(grad_y.to_vec1::<f32>()?, [4.0, 14.0, 2.0]);
    Ok(())
}

#[test]
fn test_flip_backprop() -> Result<()> {
    let device = &Device::Cpu;

    // Create a tensor (leaf node) that requires gradients
    let x = Var::ones((2, 2), DType::F64, device)?;
    let weights = Tensor::arange(1.0, 5.0, device)?.reshape((2, 2))?;

    let y = x.matmul(&weights)?;
    let expected_y = Tensor::from_vec(vec![4.0, 6.0, 4.0, 6.0], (2, 2), device)?;
    candle_core::test_utils::assert_tensor_eq(&y, &expected_y)?;

    let z = y.flip(&[1])?;
    let expected_z = Tensor::from_vec(vec![6.0, 4.0, 6.0, 4.0], (2, 2), device)?;
    candle_core::test_utils::assert_tensor_eq(&z, &expected_z)?;

    let loss = z.sum_all()?;

    let grad_store = loss.backward()?;
    let grad_x = grad_store.get_id(x.id()).unwrap();

    let flipped_weights = weights.flip(&[1])?;
    let dloss_dy = Tensor::ones((2, 2), DType::F64, device)?;
    // dloss/dx = dloss/dy @ dy/dx = ones @ weight.flip.T
    let expected_grad = dloss_dy.matmul(&flipped_weights.t()?)?;
    candle_core::test_utils::assert_tensor_eq(grad_x, &expected_grad)?;

    Ok(())
}

test_device!(
    simple_grad,
    simple_grad_cpu,
    simple_grad_gpu,
    simple_grad_metal
);
test_device!(sum_grad, sum_grad_cpu, sum_grad_gpu, sum_grad_metal);
test_device!(
    matmul_grad,
    matmul_grad_cpu,
    matmul_grad_gpu,
    matmul_grad_metal
);
test_device!(
    grad_descent,
    grad_descent_cpu,
    grad_descent_gpu,
    grad_descent_metal
);
test_device!(unary_grad, unary_grad_cpu, unary_grad_gpu, unary_grad_metal);
test_device!(
    binary_grad,
    binary_grad_cpu,
    binary_grad_gpu,
    binary_grad_metal
);
