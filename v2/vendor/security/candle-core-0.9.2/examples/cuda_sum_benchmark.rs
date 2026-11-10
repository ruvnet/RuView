#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;

use std::str::FromStr;

use anyhow::Result;
use candle_core::{Device, Tensor};

fn cos_sin(n: usize, device: &Device) -> Result<Tensor> {
    let thetas: Vec<_> = (0..n).map(|i| i as f32 / n as f32).collect();
    let xs: Vec<_> = thetas.iter().map(|t| t.cos().abs()).collect();
    let ys: Vec<_> = thetas.iter().map(|t| t.sin().abs()).collect();
    let xs = Tensor::from_vec(xs, (n, 1), device)?;
    let ys = Tensor::from_vec(ys, (1, n), device)?;
    let ys = Tensor::cat(&[&ys, &ys, &ys, &ys, &ys, &ys], 1)?;
    Ok(xs.matmul(&ys)?)
}

fn main() -> Result<()> {
    let device = Device::new_cuda(0)?;
    let args = std::env::args().collect::<Vec<String>>();
    let n = if args.len() < 2 {
        2000usize
    } else {
        usize::from_str(&args[1])?
    };
    let xys_cpu = cos_sin(n, &Device::Cpu)?;
    let xys = cos_sin(n, &device)?;
    println!("{xys_cpu:?} {xys:?}");
    let sum_keepdim_cpu = xys_cpu.sum_keepdim(1)?;
    println!("{sum_keepdim_cpu}");
    let sum_keepdim = xys.sum_keepdim(1)?;
    println!("{sum_keepdim}");
    let start = std::time::Instant::now();
    let n_iters = 100;
    let mut v = 0f32;
    for _i in 0..n_iters {
        let sum_keepdim = xys.sum_keepdim(1)?;
        let sum_keepdim = sum_keepdim.sum_keepdim(0)?;
        let sum_keepdim: f32 = sum_keepdim.reshape(&[])?.to_scalar()?;
        v += sum_keepdim;
    }
    let elapsed = start.elapsed();
    if v > 0. {
        println!(
            "ran {n_iters} iterations, time per iter: {:?} ({v})",
            elapsed.div_f64(n_iters as f64)
        );
    }
    Ok(())
}
