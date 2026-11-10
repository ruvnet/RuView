#[cfg(feature = "accelerate")]
extern crate accelerate_src;

#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

use anyhow::Result;
use candle_core::{Device, Tensor};

fn main() -> Result<()> {
    // This requires the code to be run with MTL_CAPTURE_ENABLED=1
    let device = Device::new_metal(0)?;
    let metal_device = match &device {
        Device::Metal(m) => m,
        _ => anyhow::bail!("unexpected device"),
    };
    metal_device.capture("/tmp/candle.gputrace")?;
    // This first synchronize ensures that a new command buffer gets created after setting up the
    // capture scope.
    device.synchronize()?;
    let x = Tensor::randn(0f32, 1.0, (128, 128), &device)?;
    let x1 = x.add(&x)?;
    println!("{x1:?}");
    // This second synchronize ensures that the command buffer gets committed before the end of the
    // capture scope.
    device.synchronize()?;
    Ok(())
}
