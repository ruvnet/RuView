//! Image enhancement functions for improving OCR accuracy

use super::{PreprocessError, Result};
use image::{GrayImage, Luma};
use std::cmp;

/// Contrast Limited Adaptive Histogram Equalization (CLAHE)
///
/// Improves local contrast while avoiding over-amplification of noise.
/// Divides image into tiles and applies histogram equalization with clipping.
///
/// # Arguments
/// * `image` - Input grayscale image
/// * `clip_limit` - Contrast clipping limit (typically 2.0-4.0)
/// * `tile_size` - Size of contextual regions (typically 8x8 or 16x16)
///
/// # Returns
/// Enhanced image with improved local contrast
///
/// # Example
/// ```no_run
/// use ruvector_scipix::preprocess::enhancement::clahe;
/// # use image::GrayImage;
/// # let image = GrayImage::new(100, 100);
/// let enhanced = clahe(&image, 2.0, 8).unwrap();
/// ```
pub fn clahe(image: &GrayImage, clip_limit: f32, tile_size: u32) -> Result<GrayImage> {
    if tile_size == 0 || clip_limit <= 0.0 {
        return Err(PreprocessError::InvalidParameters(
            "Invalid CLAHE parameters".to_string(),
        ));
    }

    let (width, height) = image.dimensions();
    let mut result = GrayImage::new(width, height);

    let tiles_x = (width + tile_size - 1) / tile_size;
    let tiles_y = (height + tile_size - 1) / tile_size;

    // Compute histograms and CDFs for each tile
    let mut tile_cdfs = vec![vec![Vec::new(); tiles_x as usize]; tiles_y as usize];

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            let x_start = tx * tile_size;
            let y_start = ty * tile_size;
            let x_end = cmp::min(x_start + tile_size, width);
            let y_end = cmp::min(y_start + tile_size, height);

            let cdf = compute_tile_cdf(image, x_start, y_start, x_end, y_end, clip_limit);
            tile_cdfs[ty as usize][tx as usize] = cdf;
        }
    }

    // Interpolate and apply transformation
    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y)[0];

            // Find tile coordinates
            let tx = (x as f32 / tile_size as f32).floor();
            let ty = (y as f32 / tile_size as f32).floor();

            // Calculate interpolation weights
            let x_ratio = (x as f32 / tile_size as f32) - tx;
            let y_ratio = (y as f32 / tile_size as f32) - ty;

            let tx = tx as usize;
            let ty = ty as usize;

            // Bilinear interpolation between neighboring tiles
            let value = if tx < tiles_x as usize - 1 && ty < tiles_y as usize - 1 {
                let v00 = tile_cdfs[ty][tx][pixel as usize];
                let v10 = tile_cdfs[ty][tx + 1][pixel as usize];
                let v01 = tile_cdfs[ty + 1][tx][pixel as usize];
                let v11 = tile_cdfs[ty + 1][tx + 1][pixel as usize];

                let v0 = v00 * (1.0 - x_ratio) + v10 * x_ratio;
                let v1 = v01 * (1.0 - x_ratio) + v11 * x_ratio;

                v0 * (1.0 - y_ratio) + v1 * y_ratio
            } else if tx < tiles_x as usize - 1 {
                let v0 = tile_cdfs[ty][tx][pixel as usize];
                let v1 = tile_cdfs[ty][tx + 1][pixel as usize];
                v0 * (1.0 - x_ratio) + v1 * x_ratio
            } else if ty < tiles_y as usize - 1 {
                let v0 = tile_cdfs[ty][tx][pixel as usize];
                let v1 = tile_cdfs[ty + 1][tx][pixel as usize];
                v0 * (1.0 - y_ratio) + v1 * y_ratio
            } else {
                tile_cdfs[ty][tx][pixel as usize]
            };

            result.put_pixel(x, y, Luma([(value * 255.0) as u8]));
        }
    }

    Ok(result)
}

/// Compute clipped histogram and CDF for a tile
fn compute_tile_cdf(
    image: &GrayImage,
    x_start: u32,
    y_start: u32,
    x_end: u32,
    y_end: u32,
    clip_limit: f32,
) -> Vec<f32> {
    // Calculate histogram
    let mut histogram = [0u32; 256];
    let mut pixel_count = 0;

    for y in y_start..y_end {
        for x in x_start..x_end {
            let pixel = image.get_pixel(x, y)[0];
            histogram[pixel as usize] += 1;
            pixel_count += 1;
        }
    }

    if pixel_count == 0 {
        return vec![0.0; 256];
    }

    // Apply contrast limiting
    let clip_limit_actual = (clip_limit * pixel_count as f32 / 256.0) as u32;
    let mut clipped_total = 0u32;

    for h in histogram.iter_mut() {
        if *h > clip_limit_actual {
            clipped_total += *h - clip_limit_actual;
            *h = clip_limit_actual;
        }
    }

    // Redistribute clipped pixels
    let redistribute = clipped_total / 256;
    let remainder = clipped_total % 256;

    for (i, h) in histogram.iter_mut().enumerate() {
        *h += redistribute;
        if i < remainder as usize {
            *h += 1;
        }
    }

    // Compute cumulative distribution function (CDF)
    let mut cdf = vec![0.0; 256];
    let mut cumsum = 0u32;

    for (i, &h) in histogram.iter().enumerate() {
        cumsum += h;
        cdf[i] = cumsum as f32 / pixel_count as f32;
    }

    cdf
}

/// Normalize brightness across the image
///
/// Adjusts image to have mean brightness of 128
///
/// # Arguments
/// * `image` - Input grayscale image
///
/// # Returns
/// Brightness-normalized image
pub fn normalize_brightness(image: &GrayImage) -> GrayImage {
    let (width, height) = image.dimensions();
    let pixel_count = (width * height) as f32;

    // Calculate mean brightness
    let sum: u32 = image.pixels().map(|p| p[0] as u32).sum();
    let mean = sum as f32 / pixel_count;

    let target_mean = 128.0;
    let adjustment = target_mean - mean;

    // Apply adjustment
    let mut result = GrayImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels() {
        let adjusted = (pixel[0] as f32 + adjustment).clamp(0.0, 255.0) as u8;
        result.put_pixel(x, y, Luma([adjusted]));
    }

    result
}

/// Remove shadows from document image
///
/// Uses morphological operations to estimate and subtract background
///
/// # Arguments
/// * `image` - Input grayscale image
///
/// # Returns
/// Image with reduced shadows
pub fn remove_shadows(image: &GrayImage) -> Result<GrayImage> {
    let (width, height) = image.dimensions();

    // Estimate background using dilation (morphological closing)
    let kernel_size = (width.min(height) / 20).max(15) as usize;
    let background = estimate_background(image, kernel_size);

    // Subtract background
    let mut result = GrayImage::new(width, height);
    for (x, y, pixel) in image.enumerate_pixels() {
        let bg = background.get_pixel(x, y)[0] as i32;
        let fg = pixel[0] as i32;

        // Normalize: (foreground / background) * 255
        let normalized = if bg > 0 {
            ((fg as f32 / bg as f32) * 255.0).min(255.0) as u8
        } else {
            fg as u8
        };

        result.put_pixel(x, y, Luma([normalized]));
    }

    Ok(result)
}

/// Estimate background using max filter (dilation)
fn estimate_background(image: &GrayImage, kernel_size: usize) -> GrayImage {
    let (width, height) = image.dimensions();
    let mut background = GrayImage::new(width, height);
    let half_kernel = (kernel_size / 2) as i32;

    for y in 0..height {
        for x in 0..width {
            let mut max_val = 0u8;

            // Find maximum in kernel window
            for ky in -(half_kernel)..=half_kernel {
                for kx in -(half_kernel)..=half_kernel {
                    let px = (x as i32 + kx).clamp(0, width as i32 - 1) as u32;
                    let py = (y as i32 + ky).clamp(0, height as i32 - 1) as u32;

                    let val = image.get_pixel(px, py)[0];
                    if val > max_val {
                        max_val = val;
                    }
                }
            }

            background.put_pixel(x, y, Luma([max_val]));
        }
    }

    background
}

/// Enhance contrast using simple linear stretch
///
/// Maps min-max range to 0-255
pub fn contrast_stretch(image: &GrayImage) -> GrayImage {
    // Find min and max values
    let mut min_val = 255u8;
    let mut max_val = 0u8;

    for pixel in image.pixels() {
        let val = pixel[0];
        if val < min_val {
            min_val = val;
        }
        if val > max_val {
            max_val = val;
        }
    }

    if min_val == max_val {
        return image.clone();
    }

    // Stretch contrast
    let (width, height) = image.dimensions();
    let mut result = GrayImage::new(width, height);
    let range = (max_val - min_val) as f32;

    for (x, y, pixel) in image.enumerate_pixels() {
        let val = pixel[0];
        let stretched = ((val - min_val) as f32 / range * 255.0) as u8;
        result.put_pixel(x, y, Luma([stretched]));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_image() -> GrayImage {
        let mut img = GrayImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                let val = ((x + y) / 2) as u8;
                img.put_pixel(x, y, Luma([val]));
            }
        }
        img
    }

    #[test]
    fn test_clahe() {
        let img = create_test_image();
        let enhanced = clahe(&img, 2.0, 8);

        assert!(enhanced.is_ok());
        let result = enhanced.unwrap();
        assert_eq!(result.dimensions(), img.dimensions());
    }

    #[test]
    fn test_clahe_invalid_params() {
        let img = create_test_image();

        // Invalid tile size
        let result = clahe(&img, 2.0, 0);
        assert!(result.is_err());

        // Invalid clip limit
        let result = clahe(&img, -1.0, 8);
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_brightness() {
        let img = create_test_image();
        let normalized = normalize_brightness(&img);

        assert_eq!(normalized.dimensions(), img.dimensions());

        // Check that mean is closer to 128
        let sum: u32 = normalized.pixels().map(|p| p[0] as u32).sum();
        let mean = sum as f32 / (100.0 * 100.0);

        assert!((mean - 128.0).abs() < 5.0);
    }

    #[test]
    fn test_remove_shadows() {
        let img = create_test_image();
        let result = remove_shadows(&img);

        assert!(result.is_ok());
        let shadow_removed = result.unwrap();
        assert_eq!(shadow_removed.dimensions(), img.dimensions());
    }

    #[test]
    fn test_contrast_stretch() {
        // Create low contrast image
        let mut img = GrayImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                let val = 100 + ((x + y) / 10) as u8; // Range: 100-119
                img.put_pixel(x, y, Luma([val]));
            }
        }

        let stretched = contrast_stretch(&img);

        // Check that range is now 0-255
        let mut min_val = 255u8;
        let mut max_val = 0u8;
        for pixel in stretched.pixels() {
            let val = pixel[0];
            if val < min_val {
                min_val = val;
            }
            if val > max_val {
                max_val = val;
            }
        }

        assert_eq!(min_val, 0);
        assert_eq!(max_val, 255);
    }

    #[test]
    fn test_contrast_stretch_uniform() {
        // Uniform image should remain unchanged
        let mut img = GrayImage::new(50, 50);
        for pixel in img.pixels_mut() {
            *pixel = Luma([128]);
        }

        let stretched = contrast_stretch(&img);

        for pixel in stretched.pixels() {
            assert_eq!(pixel[0], 128);
        }
    }

    #[test]
    fn test_estimate_background() {
        let img = create_test_image();
        let background = estimate_background(&img, 5);

        assert_eq!(background.dimensions(), img.dimensions());

        // Background should have higher values (max filter)
        for (orig, bg) in img.pixels().zip(background.pixels()) {
            assert!(bg[0] >= orig[0]);
        }
    }

    #[test]
    fn test_clahe_various_tile_sizes() {
        let img = create_test_image();

        for tile_size in [4, 8, 16, 32] {
            let result = clahe(&img, 2.0, tile_size);
            assert!(result.is_ok());
        }
    }
}
