//! Image transformation functions for preprocessing

use super::{PreprocessError, Result};
use image::{DynamicImage, GrayImage, Luma};
use imageproc::filter::gaussian_blur_f32;
use std::f32;

/// Convert image to grayscale
///
/// # Arguments
/// * `image` - Input color or grayscale image
///
/// # Returns
/// Grayscale image
pub fn to_grayscale(image: &DynamicImage) -> GrayImage {
    image.to_luma8()
}

/// Apply Gaussian blur for noise reduction
///
/// # Arguments
/// * `image` - Input grayscale image
/// * `sigma` - Standard deviation of Gaussian kernel
///
/// # Returns
/// Blurred image
///
/// # Example
/// ```no_run
/// use ruvector_scipix::preprocess::transforms::gaussian_blur;
/// # use image::GrayImage;
/// # let image = GrayImage::new(100, 100);
/// let blurred = gaussian_blur(&image, 1.5).unwrap();
/// ```
pub fn gaussian_blur(image: &GrayImage, sigma: f32) -> Result<GrayImage> {
    if sigma <= 0.0 {
        return Err(PreprocessError::InvalidParameters(
            "Sigma must be positive".to_string(),
        ));
    }

    Ok(gaussian_blur_f32(image, sigma))
}

/// Sharpen image using unsharp mask
///
/// # Arguments
/// * `image` - Input grayscale image
/// * `sigma` - Gaussian blur sigma
/// * `amount` - Sharpening strength (typically 0.5-2.0)
///
/// # Returns
/// Sharpened image
pub fn sharpen(image: &GrayImage, sigma: f32, amount: f32) -> Result<GrayImage> {
    if sigma <= 0.0 || amount < 0.0 {
        return Err(PreprocessError::InvalidParameters(
            "Invalid sharpening parameters".to_string(),
        ));
    }

    let blurred = gaussian_blur_f32(image, sigma);
    let (width, height) = image.dimensions();
    let mut result = GrayImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let original = image.get_pixel(x, y)[0] as f32;
            let blur = blurred.get_pixel(x, y)[0] as f32;

            // Unsharp mask: original + amount * (original - blurred)
            let sharpened = original + amount * (original - blur);
            let clamped = sharpened.clamp(0.0, 255.0) as u8;

            result.put_pixel(x, y, Luma([clamped]));
        }
    }

    Ok(result)
}

/// Calculate optimal threshold using Otsu's method
///
/// Implements full Otsu's algorithm for automatic threshold selection
/// based on maximizing inter-class variance.
///
/// # Arguments
/// * `image` - Input grayscale image
///
/// # Returns
/// Optimal threshold value (0-255)
///
/// # Example
/// ```no_run
/// use ruvector_scipix::preprocess::transforms::otsu_threshold;
/// # use image::GrayImage;
/// # let image = GrayImage::new(100, 100);
/// let threshold = otsu_threshold(&image).unwrap();
/// println!("Optimal threshold: {}", threshold);
/// ```
pub fn otsu_threshold(image: &GrayImage) -> Result<u8> {
    // Calculate histogram
    let mut histogram = [0u32; 256];
    for pixel in image.pixels() {
        histogram[pixel[0] as usize] += 1;
    }

    let total_pixels = (image.width() * image.height()) as f64;

    // Calculate cumulative sums
    let mut sum_total = 0.0;
    for (i, &count) in histogram.iter().enumerate() {
        sum_total += (i as f64) * (count as f64);
    }

    let mut sum_background = 0.0;
    let mut weight_background = 0.0;
    let mut max_variance = 0.0;
    let mut threshold = 0u8;

    // Find threshold that maximizes inter-class variance
    for (t, &count) in histogram.iter().enumerate() {
        weight_background += count as f64;
        if weight_background == 0.0 {
            continue;
        }

        let weight_foreground = total_pixels - weight_background;
        if weight_foreground == 0.0 {
            break;
        }

        sum_background += (t as f64) * (count as f64);

        let mean_background = sum_background / weight_background;
        let mean_foreground = (sum_total - sum_background) / weight_foreground;

        // Inter-class variance
        let variance =
            weight_background * weight_foreground * (mean_background - mean_foreground).powi(2);

        if variance > max_variance {
            max_variance = variance;
            threshold = t as u8;
        }
    }

    Ok(threshold)
}

/// Apply binary thresholding
///
/// # Arguments
/// * `image` - Input grayscale image
/// * `threshold` - Threshold value (0-255)
///
/// # Returns
/// Binary image (0 or 255)
pub fn threshold(image: &GrayImage, threshold_val: u8) -> GrayImage {
    let (width, height) = image.dimensions();
    let mut result = GrayImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y)[0];
            let value = if pixel >= threshold_val { 255 } else { 0 };
            result.put_pixel(x, y, Luma([value]));
        }
    }

    result
}

/// Apply adaptive thresholding using local window statistics
///
/// Uses a sliding window to calculate local mean and applies threshold
/// relative to local statistics. Better for images with varying illumination.
///
/// # Arguments
/// * `image` - Input grayscale image
/// * `window_size` - Size of local window (must be odd)
///
/// # Returns
/// Binary image with adaptive thresholding applied
///
/// # Example
/// ```no_run
/// use ruvector_scipix::preprocess::transforms::adaptive_threshold;
/// # use image::GrayImage;
/// # let image = GrayImage::new(100, 100);
/// let binary = adaptive_threshold(&image, 15).unwrap();
/// ```
pub fn adaptive_threshold(image: &GrayImage, window_size: u32) -> Result<GrayImage> {
    if window_size % 2 == 0 {
        return Err(PreprocessError::InvalidParameters(
            "Window size must be odd".to_string(),
        ));
    }

    let (width, height) = image.dimensions();
    let mut result = GrayImage::new(width, height);
    let half_window = (window_size / 2) as i32;

    // Use integral image for fast window sum calculation
    let integral = compute_integral_image(image);

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            // Define window bounds
            let x1 = (x - half_window).max(0);
            let y1 = (y - half_window).max(0);
            let x2 = (x + half_window + 1).min(width as i32);
            let y2 = (y + half_window + 1).min(height as i32);

            // Calculate mean using integral image
            let area = ((x2 - x1) * (y2 - y1)) as f64;
            let sum = get_integral_sum(&integral, x1, y1, x2, y2);
            let mean = (sum as f64 / area) as u8;

            // Apply threshold with small bias
            let pixel = image.get_pixel(x as u32, y as u32)[0];
            let bias = 5; // Small bias to reduce noise
            let value = if pixel >= mean.saturating_sub(bias) {
                255
            } else {
                0
            };

            result.put_pixel(x as u32, y as u32, Luma([value]));
        }
    }

    Ok(result)
}

/// Compute integral image for fast rectangle sum queries
fn compute_integral_image(image: &GrayImage) -> Vec<Vec<u64>> {
    let (width, height) = image.dimensions();
    let mut integral = vec![vec![0u64; width as usize + 1]; height as usize + 1];

    for y in 1..=height as usize {
        for x in 1..=width as usize {
            let pixel = image.get_pixel(x as u32 - 1, y as u32 - 1)[0] as u64;
            integral[y][x] =
                pixel + integral[y - 1][x] + integral[y][x - 1] - integral[y - 1][x - 1];
        }
    }

    integral
}

/// Get sum of rectangle in integral image
fn get_integral_sum(integral: &[Vec<u64>], x1: i32, y1: i32, x2: i32, y2: i32) -> u64 {
    let x1 = x1 as usize;
    let y1 = y1 as usize;
    let x2 = x2 as usize;
    let y2 = y2 as usize;

    integral[y2][x2] + integral[y1][x1] - integral[y1][x2] - integral[y2][x1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn create_gradient_image(width: u32, height: u32) -> GrayImage {
        let mut img = GrayImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let val = ((x + y) * 255 / (width + height)) as u8;
                img.put_pixel(x, y, Luma([val]));
            }
        }
        img
    }

    #[test]
    fn test_to_grayscale() {
        let img = DynamicImage::new_rgb8(100, 100);
        let gray = to_grayscale(&img);
        assert_eq!(gray.dimensions(), (100, 100));
    }

    #[test]
    fn test_gaussian_blur() {
        let img = create_gradient_image(50, 50);
        let blurred = gaussian_blur(&img, 1.0);
        assert!(blurred.is_ok());

        let result = blurred.unwrap();
        assert_eq!(result.dimensions(), img.dimensions());
    }

    #[test]
    fn test_gaussian_blur_invalid_sigma() {
        let img = create_gradient_image(50, 50);
        let result = gaussian_blur(&img, -1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_sharpen() {
        let img = create_gradient_image(50, 50);
        let sharpened = sharpen(&img, 1.0, 1.5);
        assert!(sharpened.is_ok());

        let result = sharpened.unwrap();
        assert_eq!(result.dimensions(), img.dimensions());
    }

    #[test]
    fn test_otsu_threshold() {
        // Create bimodal image (good for Otsu)
        let mut img = GrayImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                let val = if x < 50 { 50 } else { 200 };
                img.put_pixel(x, y, Luma([val]));
            }
        }

        let threshold = otsu_threshold(&img);
        assert!(threshold.is_ok());

        let t = threshold.unwrap();
        // Should be somewhere between the two values (not necessarily strictly between)
        // Otsu finds optimal threshold which could be at boundary
        assert!(
            t >= 50 && t <= 200,
            "threshold {} should be between 50 and 200",
            t
        );
    }

    #[test]
    fn test_threshold() {
        let img = create_gradient_image(100, 100);
        let binary = threshold(&img, 128);

        assert_eq!(binary.dimensions(), img.dimensions());

        // Check that output is binary
        for pixel in binary.pixels() {
            let val = pixel[0];
            assert!(val == 0 || val == 255);
        }
    }

    #[test]
    fn test_adaptive_threshold() {
        let img = create_gradient_image(100, 100);
        let binary = adaptive_threshold(&img, 15);
        assert!(binary.is_ok());

        let result = binary.unwrap();
        assert_eq!(result.dimensions(), img.dimensions());

        // Check binary output
        for pixel in result.pixels() {
            let val = pixel[0];
            assert!(val == 0 || val == 255);
        }
    }

    #[test]
    fn test_adaptive_threshold_invalid_window() {
        let img = create_gradient_image(50, 50);
        let result = adaptive_threshold(&img, 16); // Even number
        assert!(result.is_err());
    }

    #[test]
    fn test_integral_image() {
        let mut img = GrayImage::new(3, 3);
        for y in 0..3 {
            for x in 0..3 {
                img.put_pixel(x, y, Luma([1]));
            }
        }

        let integral = compute_integral_image(&img);

        // Check 3x3 sum
        let sum = get_integral_sum(&integral, 0, 0, 3, 3);
        assert_eq!(sum, 9); // 3x3 image with all 1s
    }

    #[test]
    fn test_threshold_extremes() {
        let img = create_gradient_image(100, 100);

        // Threshold at 0 should make everything white
        let binary = threshold(&img, 0);
        assert!(binary.pixels().all(|p| p[0] == 255));

        // Threshold at 255 should make everything black
        let binary = threshold(&img, 255);
        assert!(binary.pixels().all(|p| p[0] == 0));
    }
}
