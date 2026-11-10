#[cfg(feature = "accelerate")]
extern crate accelerate_src;

#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

use anyhow::Result;
use candle_core::{Device, Tensor};
// xs: [1024, 64, 1924], c Tensor[dims 128, 64, 8; f32, cuda:0] Conv1dConfig { padding: 0, stride: 4, dilation: 1, groups: 1 }
fn main() -> Result<()> {
    let device = Device::new_cuda(0)?;
    let x = Tensor::randn(0f32, 1.0, (1024, 64, 1924), &device)?;
    let c = Tensor::randn(0f32, 1.0, (128, 64, 8), &device)?;
    let _x1 = x.conv1d(&c, 0, 4, 1, 1)?;
    drop(_x1);
    for _ in 0..20 {
        let start_time = std::time::Instant::now();
        let _x1 = x.conv1d(&c, 0, 4, 1, 1)?;
        device.synchronize()?;
        println!("conv1d: {:?}", start_time.elapsed());
    }
    Ok(())
}
