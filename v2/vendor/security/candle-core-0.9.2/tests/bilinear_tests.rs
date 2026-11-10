use candle_core::{test_device, Device, IndexOp, Result, Tensor};

// ============================================================================
// PyTorch Exact Comparison Tests
// ============================================================================
// These tests compare against exact PyTorch outputs to ensure correctness

/* Test corresponds to PyTorch:
import torch
import torch.nn.functional as F
input = torch.arange(16, dtype=torch.float32).reshape(1, 1, 4, 4)
output = F.interpolate(input, size=(8, 8), mode='bilinear', align_corners=False)
*/
fn bilinear_pytorch_2x_upscale(dev: &Device) -> Result<()> {
    let input = Tensor::arange(0f32, 16f32, dev)?.reshape((1, 1, 4, 4))?;
    let output = input.upsample_bilinear2d(8, 8, false)?;

    // PyTorch expected output (verified from PyTorch 2.10.0)
    let expected = Tensor::new(
        &[
            0.0000f32, 0.2500, 0.7500, 1.2500, 1.7500, 2.2500, 2.7500, 3.0000, 1.0000, 1.2500,
            1.7500, 2.2500, 2.7500, 3.2500, 3.7500, 4.0000, 3.0000, 3.2500, 3.7500, 4.2500, 4.7500,
            5.2500, 5.7500, 6.0000, 5.0000, 5.2500, 5.7500, 6.2500, 6.7500, 7.2500, 7.7500, 8.0000,
            7.0000, 7.2500, 7.7500, 8.2500, 8.7500, 9.2500, 9.7500, 10.0000, 9.0000, 9.2500,
            9.7500, 10.2500, 10.7500, 11.2500, 11.7500, 12.0000, 11.0000, 11.2500, 11.7500,
            12.2500, 12.7500, 13.2500, 13.7500, 14.0000, 12.0000, 12.2500, 12.7500, 13.2500,
            13.7500, 14.2500, 14.7500, 15.0000,
        ],
        dev,
    )?
    .reshape((1, 1, 8, 8))?;

    let diff = (&output - &expected)?.abs()?.flatten_all()?.max(0)?;
    let max_diff = diff.to_vec0::<f32>()?;

    assert!(
        max_diff < 1e-4,
        "Max difference {} exceeds threshold 1e-4",
        max_diff
    );
    Ok(())
}

/* Test corresponds to PyTorch:
import torch
import torch.nn.functional as F
input = torch.arange(64, dtype=torch.float32).reshape(1, 1, 8, 8)
output = F.interpolate(input, size=(4, 4), mode='bilinear', align_corners=False)
*/
fn bilinear_pytorch_downscale(dev: &Device) -> Result<()> {
    let input = Tensor::arange(0f32, 64f32, dev)?.reshape((1, 1, 8, 8))?;
    let output = input.upsample_bilinear2d(4, 4, false)?;

    // PyTorch expected output
    let expected = Tensor::new(
        &[
            4.5f32, 6.5, 8.5, 10.5, 20.5, 22.5, 24.5, 26.5, 36.5, 38.5, 40.5, 42.5, 52.5, 54.5,
            56.5, 58.5,
        ],
        dev,
    )?
    .reshape((1, 1, 4, 4))?;

    let diff = (&output - &expected)?.abs()?.flatten_all()?.max(0)?;
    let max_diff = diff.to_vec0::<f32>()?;

    assert!(
        max_diff < 1e-4,
        "Max difference {} exceeds threshold 1e-4",
        max_diff
    );
    Ok(())
}

/* Test corresponds to PyTorch:
import torch
import torch.nn.functional as F
torch.manual_seed(42)
input = torch.randn(1, 2, 4, 4, dtype=torch.float32)
output = F.interpolate(input, size=(8, 8), mode='bilinear', align_corners=False)
*/
fn bilinear_pytorch_multi_channel(dev: &Device) -> Result<()> {
    // Using fixed seed data from PyTorch (seed=42)
    let input = Tensor::new(
        &[
            // Channel 0
            1.9269f32, 1.4873, 0.9007, -2.1055, 0.6784, -1.2345, -0.0431, -1.6047, -0.7521, 1.6487,
            -0.3925, -1.4036, -0.7279, -0.5594, -0.7688, 0.7624, // Channel 1
            1.6423f32, -0.1596, -0.4974, 0.4396, -0.7581, 1.0783, 0.8008, 1.6806, 1.2791, 1.2964,
            0.6105, 1.3347, -0.2316, 0.0418, -0.2516, 0.8599,
        ],
        dev,
    )?
    .reshape((1, 2, 4, 4))?;

    let output = input.upsample_bilinear2d(8, 8, false)?;

    assert_eq!(output.dims(), &[1, 2, 8, 8]);

    // Verify output is finite and in reasonable range
    let output_vec = output.flatten_all()?.to_vec1::<f32>()?;
    for &val in &output_vec {
        assert!(val.is_finite(), "Output contains non-finite value");
    }

    // Check first row of channel 0 from PyTorch output
    let output_ch0_row0 = output.i((0, 0, 0, ..))?.to_vec1::<f32>()?;
    let expected_ch0_row0 = [
        1.9269f32, 1.8170, 1.5972, 1.3406, 1.0474, 0.1492, -1.3540, -2.1055,
    ];

    for (i, (&out, &exp)) in output_ch0_row0
        .iter()
        .zip(expected_ch0_row0.iter())
        .enumerate()
    {
        let diff = (out - exp).abs();
        assert!(
            diff < 1e-3,
            "Channel 0, row 0, index {} differs: got {}, expected {}, diff {}",
            i,
            out,
            exp,
            diff
        );
    }

    // Check first row of channel 1 from PyTorch output
    let output_ch1_row0 = output.i((0, 1, 0, ..))?.to_vec1::<f32>()?;
    let expected_ch1_row0 = [
        1.6423f32, 1.1918, 0.2909, -0.2440, -0.4129, -0.2632, 0.2053, 0.4396,
    ];

    for (i, (&out, &exp)) in output_ch1_row0
        .iter()
        .zip(expected_ch1_row0.iter())
        .enumerate()
    {
        let diff = (out - exp).abs();
        assert!(
            diff < 1e-3,
            "Channel 1, row 0, index {} differs: got {}, expected {}, diff {}",
            i,
            out,
            exp,
            diff
        );
    }

    Ok(())
}

/* Test corresponds to PyTorch:
import torch
import torch.nn.functional as F
input = torch.tensor([[[[1.0, 2.0], [3.0, 4.0]]]], dtype=torch.float32)
output = F.interpolate(input, size=(4, 4), mode='bilinear', align_corners=True)
*/
fn bilinear_pytorch_align_corners_true(dev: &Device) -> Result<()> {
    let input = Tensor::from_vec(vec![1.0f32, 2.0, 3.0, 4.0], (1, 1, 2, 2), dev)?;
    let output = input.upsample_bilinear2d(4, 4, true)?;

    // PyTorch expected output with align_corners=True
    let expected = Tensor::new(
        &[
            1.0f32, 1.3333, 1.6667, 2.0, 1.6667, 2.0, 2.3333, 2.6667, 2.3333, 2.6667, 3.0, 3.3333,
            3.0, 3.3333, 3.6667, 4.0,
        ],
        dev,
    )?
    .reshape((1, 1, 4, 4))?;

    let diff = (&output - &expected)?.abs()?.flatten_all()?.max(0)?;
    let max_diff = diff.to_vec0::<f32>()?;

    assert!(
        max_diff < 1e-3,
        "Max difference {} exceeds threshold 1e-3",
        max_diff
    );

    // Verify corners are exactly preserved with align_corners=True
    let output_vec = output.flatten_all()?.to_vec1::<f32>()?;
    assert!(
        (output_vec[0] - 1.0).abs() < 1e-5,
        "Top-left corner not preserved"
    );
    assert!(
        (output_vec[3] - 2.0).abs() < 1e-5,
        "Top-right corner not preserved"
    );
    assert!(
        (output_vec[12] - 3.0).abs() < 1e-5,
        "Bottom-left corner not preserved"
    );
    assert!(
        (output_vec[15] - 4.0).abs() < 1e-5,
        "Bottom-right corner not preserved"
    );

    Ok(())
}

/* Test corresponds to PyTorch:
import torch
import torch.nn.functional as F
input = torch.arange(16, dtype=torch.float32).reshape(1, 1, 4, 4)
output = F.interpolate(input, scale_factor=2.0, mode='bilinear', align_corners=False)
*/
fn bilinear_pytorch_scale_factor(dev: &Device) -> Result<()> {
    let input = Tensor::arange(0f32, 16f32, dev)?.reshape((1, 1, 4, 4))?;
    let output_scale = input.upsample_bilinear2d_with_scale(2.0, 2.0, false)?;
    let output_size = input.upsample_bilinear2d(8, 8, false)?;

    // scale_factor=2.0 should produce identical results to size=(8, 8)
    let diff = (&output_scale - &output_size)?
        .abs()?
        .flatten_all()?
        .max(0)?;
    let max_diff = diff.to_vec0::<f32>()?;

    assert!(
        max_diff < 1e-6,
        "scale_factor and size methods differ by {}",
        max_diff
    );

    Ok(())
}

/* Test corresponds to PyTorch:
import torch
import torch.nn.functional as F
input = torch.arange(24, dtype=torch.float32).reshape(1, 1, 4, 6)
output = F.interpolate(input, size=(8, 12), mode='bilinear', align_corners=False)
*/
fn bilinear_pytorch_non_square_exact(dev: &Device) -> Result<()> {
    let input = Tensor::arange(0f32, 24f32, dev)?.reshape((1, 1, 4, 6))?;
    let output = input.upsample_bilinear2d(8, 12, false)?;

    // PyTorch expected output (verified from PyTorch 2.10.0)
    #[rustfmt::skip]
    let expected = Tensor::new(
        &[
            0.0f32, 0.25, 0.75, 1.25, 1.75, 2.25, 2.75, 3.25, 3.75, 4.25, 4.75, 5.0,
            1.5, 1.75, 2.25, 2.75, 3.25, 3.75, 4.25, 4.75, 5.25, 5.75, 6.25, 6.5,
            4.5, 4.75, 5.25, 5.75, 6.25, 6.75, 7.25, 7.75, 8.25, 8.75, 9.25, 9.5,
            7.5, 7.75, 8.25, 8.75, 9.25, 9.75, 10.25, 10.75, 11.25, 11.75, 12.25, 12.5,
            10.5, 10.75, 11.25, 11.75, 12.25, 12.75, 13.25, 13.75, 14.25, 14.75, 15.25, 15.5,
            13.5, 13.75, 14.25, 14.75, 15.25, 15.75, 16.25, 16.75, 17.25, 17.75, 18.25, 18.5,
            16.5, 16.75, 17.25, 17.75, 18.25, 18.75, 19.25, 19.75, 20.25, 20.75, 21.25, 21.5,
            18.0, 18.25, 18.75, 19.25, 19.75, 20.25, 20.75, 21.25, 21.75, 22.25, 22.75, 23.0,
        ],
        dev,
    )?
    .reshape((1, 1, 8, 12))?;

    let diff = (&output - &expected)?.abs()?.flatten_all()?.max(0)?;
    let max_diff = diff.to_vec0::<f32>()?;

    assert!(
        max_diff < 1e-4,
        "Max difference {} exceeds threshold 1e-4",
        max_diff
    );
    Ok(())
}

/* Test corresponds to PyTorch:
import torch
import torch.nn.functional as F
input = torch.tensor([[[[5.0]]]], dtype=torch.float32)
output = F.interpolate(input, size=(3, 3), mode='bilinear', align_corners=False)
*/
fn bilinear_pytorch_tiny_1x1_to_3x3(dev: &Device) -> Result<()> {
    let input = Tensor::new(&[5.0f32], dev)?.reshape((1, 1, 1, 1))?;
    let output = input.upsample_bilinear2d(3, 3, false)?;

    // PyTorch expected output: all values should be 5.0
    let expected = Tensor::new(&[5.0f32, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0], dev)?
        .reshape((1, 1, 3, 3))?;

    let diff = (&output - &expected)?.abs()?.flatten_all()?.max(0)?;
    let max_diff = diff.to_vec0::<f32>()?;

    assert!(
        max_diff < 1e-6,
        "Max difference {} exceeds threshold 1e-6",
        max_diff
    );
    Ok(())
}

/* Test corresponds to PyTorch:
import torch
import torch.nn.functional as F
input = torch.tensor([[[[2.0, 8.0]]]], dtype=torch.float32)
output = F.interpolate(input, size=(3, 6), mode='bilinear', align_corners=False)
*/
fn bilinear_pytorch_tiny_1x2_to_3x6(dev: &Device) -> Result<()> {
    let input = Tensor::new(&[2.0f32, 8.0], dev)?.reshape((1, 1, 1, 2))?;
    let output = input.upsample_bilinear2d(3, 6, false)?;

    // PyTorch expected output
    #[rustfmt::skip]
    let expected = Tensor::new(
        &[
            2.0f32, 2.0, 4.0, 6.0, 8.0, 8.0,
            2.0, 2.0, 4.0, 6.0, 8.0, 8.0,
            2.0, 2.0, 4.0, 6.0, 8.0, 8.0,
        ],
        dev,
    )?
    .reshape((1, 1, 3, 6))?;

    let diff = (&output - &expected)?.abs()?.flatten_all()?.max(0)?;
    let max_diff = diff.to_vec0::<f32>()?;

    assert!(
        max_diff < 1e-6,
        "Max difference {} exceeds threshold 1e-6",
        max_diff
    );
    Ok(())
}

/* Test corresponds to PyTorch:
import torch
import torch.nn.functional as F
torch.manual_seed(123)
input = torch.randn(1, 1, 64, 64, dtype=torch.float32)
output = F.interpolate(input, size=(128, 128), mode='bilinear', align_corners=False)
*/
fn bilinear_pytorch_large_64x64_to_128x128(dev: &Device) -> Result<()> {
    // Test large tensor for numerical stability
    // We'll just verify dimensions and that output is finite
    use candle_core::DType;

    let input = Tensor::randn(0f32, 1f32, (1, 1, 64, 64), dev)?;
    let output = input.upsample_bilinear2d(128, 128, false)?;

    assert_eq!(output.dims(), &[1, 1, 128, 128]);
    assert_eq!(output.dtype(), DType::F32);

    // Verify all values are finite
    let output_vec = output.flatten_all()?.to_vec1::<f32>()?;
    for &val in &output_vec {
        assert!(
            val.is_finite(),
            "Large tensor output contains non-finite value"
        );
    }

    // Verify output is in reasonable range (should be similar to input range)
    let min_val = output_vec.iter().copied().fold(f32::INFINITY, f32::min);
    let max_val = output_vec.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    assert!(
        min_val > -10.0 && max_val < 10.0,
        "Large tensor output values out of expected range: min={}, max={}",
        min_val,
        max_val
    );

    Ok(())
}

// ============================================================================
// Dimension and Shape Tests (Consolidated)
// ============================================================================
// These tests verify correct output dimensions for various input configurations

fn bilinear_output_dimensions(dev: &Device) -> Result<()> {
    // Test 1: Non-square dimensions
    let t1 = Tensor::arange(0f32, 32f32, dev)?.reshape((1, 1, 4, 8))?;
    let out1 = t1.upsample_bilinear2d(6, 12, false)?;
    assert_eq!(out1.dims(), &[1, 1, 6, 12], "Non-square upscale failed");

    // Test 2: Batch processing
    let t2 = Tensor::arange(0f32, 192f32, dev)?.reshape((4, 3, 4, 4))?;
    let out2 = t2.upsample_bilinear2d(8, 8, false)?;
    assert_eq!(out2.dims(), &[4, 3, 8, 8], "Batch processing failed");

    // Test 3: Asymmetric scale factors
    let t3 = Tensor::arange(0f32, 16f32, dev)?.reshape((1, 1, 4, 4))?;
    let out3 = t3.upsample_bilinear2d_with_scale(2.0, 3.0, false)?;
    assert_eq!(out3.dims(), &[1, 1, 8, 12], "Asymmetric scale failed");

    // Test 4: Fractional scale factors
    let t4 = Tensor::arange(0f32, 16f32, dev)?.reshape((1, 1, 4, 4))?;
    let out4 = t4.upsample_bilinear2d_with_scale(1.5, 1.5, false)?;
    assert_eq!(out4.dims(), &[1, 1, 6, 6], "Fractional scale failed");

    // Test 5: Single pixel output
    let t5 = Tensor::arange(0f32, 16f32, dev)?.reshape((1, 1, 4, 4))?;
    let out5 = t5.upsample_bilinear2d(1, 1, false)?;
    assert_eq!(out5.dims(), &[1, 1, 1, 1], "Single pixel output failed");
    let val = out5.flatten_all()?.to_vec1::<f32>()?[0];
    assert!(val.is_finite(), "Single pixel value is not finite");

    // Test 6: Large scale factor
    let t6 = Tensor::arange(0f32, 4f32, dev)?.reshape((1, 1, 2, 2))?;
    let out6 = t6.upsample_bilinear2d_with_scale(5.0, 5.0, false)?;
    assert_eq!(out6.dims(), &[1, 1, 10, 10], "Large scale factor failed");

    Ok(())
}

// ============================================================================
// Special Behavior Tests
// ============================================================================

fn bilinear_identity(dev: &Device) -> Result<()> {
    // Test that upsampling to the same size returns an identical tensor
    let t = Tensor::arange(0f32, 16f32, dev)?.reshape((1, 1, 4, 4))?;
    let output = t.upsample_bilinear2d(4, 4, false)?;

    let diff = (&t - &output)?.abs()?.flatten_all()?.max(0)?;
    assert!(diff.to_vec0::<f32>()? < 1e-6);
    Ok(())
}

fn bilinear_align_corners_difference(dev: &Device) -> Result<()> {
    // Test that align_corners parameter produces different results
    let t = Tensor::arange(0f32, 16f32, dev)?.reshape((1, 1, 4, 4))?;

    let output_false = t.upsample_bilinear2d(8, 8, false)?;
    let output_true = t.upsample_bilinear2d(8, 8, true)?;

    // Results should be different between align_corners modes
    let diff = (&output_false - &output_true)?.abs()?.sum_all()?;
    assert!(diff.to_vec0::<f32>()? > 0.1);
    Ok(())
}

// ============================================================================
// Test Device Macros
// ============================================================================

// PyTorch exact comparison tests
test_device!(
    bilinear_pytorch_2x_upscale,
    bilinear_pytorch_2x_upscale_cpu,
    bilinear_pytorch_2x_upscale_gpu,
    bilinear_pytorch_2x_upscale_metal
);

test_device!(
    bilinear_pytorch_downscale,
    bilinear_pytorch_downscale_cpu,
    bilinear_pytorch_downscale_gpu,
    bilinear_pytorch_downscale_metal
);

test_device!(
    bilinear_pytorch_multi_channel,
    bilinear_pytorch_multi_channel_cpu,
    bilinear_pytorch_multi_channel_gpu,
    bilinear_pytorch_multi_channel_metal
);

test_device!(
    bilinear_pytorch_align_corners_true,
    bilinear_pytorch_align_corners_true_cpu,
    bilinear_pytorch_align_corners_true_gpu,
    bilinear_pytorch_align_corners_true_metal
);

test_device!(
    bilinear_pytorch_scale_factor,
    bilinear_pytorch_scale_factor_cpu,
    bilinear_pytorch_scale_factor_gpu,
    bilinear_pytorch_scale_factor_metal
);

test_device!(
    bilinear_pytorch_non_square_exact,
    bilinear_pytorch_non_square_exact_cpu,
    bilinear_pytorch_non_square_exact_gpu,
    bilinear_pytorch_non_square_exact_metal
);

test_device!(
    bilinear_pytorch_tiny_1x1_to_3x3,
    bilinear_pytorch_tiny_1x1_to_3x3_cpu,
    bilinear_pytorch_tiny_1x1_to_3x3_gpu,
    bilinear_pytorch_tiny_1x1_to_3x3_metal
);

test_device!(
    bilinear_pytorch_tiny_1x2_to_3x6,
    bilinear_pytorch_tiny_1x2_to_3x6_cpu,
    bilinear_pytorch_tiny_1x2_to_3x6_gpu,
    bilinear_pytorch_tiny_1x2_to_3x6_metal
);

test_device!(
    bilinear_pytorch_large_64x64_to_128x128,
    bilinear_pytorch_large_64x64_to_128x128_cpu,
    bilinear_pytorch_large_64x64_to_128x128_gpu,
    bilinear_pytorch_large_64x64_to_128x128_metal
);

// Dimension tests (consolidated)
test_device!(
    bilinear_output_dimensions,
    bilinear_output_dimensions_cpu,
    bilinear_output_dimensions_gpu,
    bilinear_output_dimensions_metal
);

// Special behavior tests
test_device!(
    bilinear_identity,
    bilinear_identity_cpu,
    bilinear_identity_gpu,
    bilinear_identity_metal
);

test_device!(
    bilinear_align_corners_difference,
    bilinear_align_corners_difference_cpu,
    bilinear_align_corners_difference_gpu,
    bilinear_align_corners_difference_metal
);
