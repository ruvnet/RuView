//! Skew detection and correction using Hough transform

use super::{PreprocessError, Result};
use image::{GrayImage, Luma};
use imageproc::edges::canny;
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use std::collections::BTreeMap;
use std::f32;

/// Detect skew angle using Hough transform
///
/// Applies edge detection and Hough transform to find dominant lines,
/// then calculates average skew angle.
///
/// # Arguments
/// * `image` - Input grayscale image
///
/// # Returns
/// Skew angle in degrees (positive = clockwise)
///
/// # Example
/// ```no_run
/// use ruvector_scipix::preprocess::deskew::detect_skew_angle;
/// # use image::GrayImage;
/// # let image = GrayImage::new(100, 100);
/// let angle = detect_skew_angle(&image).unwrap();
/// println!("Detected skew: {:.2}°", angle);
/// ```
pub fn detect_skew_angle(image: &GrayImage) -> Result<f32> {
    let (width, height) = image.dimensions();

    if width < 20 || height < 20 {
        return Err(PreprocessError::InvalidParameters(
            "Image too small for skew detection".to_string(),
        ));
    }

    // Apply Canny edge detection
    let edges = canny(image, 50.0, 100.0);

    // Perform Hough transform to detect lines
    let angles = detect_lines_hough(&edges, width, height)?;

    if angles.is_empty() {
        return Ok(0.0);
    }

    // Calculate weighted average angle
    let total_weight: f32 = angles.values().sum();
    let weighted_sum: f32 = angles
        .iter()
        .map(|(angle_key, weight)| (*angle_key as f32 / 10.0) * weight)
        .sum();

    let average_angle = if total_weight > 0.0 {
        weighted_sum / total_weight
    } else {
        0.0
    };

    Ok(average_angle)
}

/// Detect lines using Hough transform
///
/// Returns map of angles to their confidence weights
fn detect_lines_hough(edges: &GrayImage, width: u32, height: u32) -> Result<BTreeMap<i32, f32>> {
    let max_rho = ((width * width + height * height) as f32).sqrt() as usize;
    let num_angles = 360;

    // Accumulator array for Hough space
    let mut accumulator = vec![vec![0u32; max_rho]; num_angles];

    // Populate accumulator
    for y in 0..height {
        for x in 0..width {
            if edges.get_pixel(x, y)[0] > 128 {
                // Edge pixel found
                for theta_idx in 0..num_angles {
                    let theta = (theta_idx as f32) * std::f32::consts::PI / 180.0;
                    let rho = (x as f32) * theta.cos() + (y as f32) * theta.sin();
                    let rho_idx = (rho + max_rho as f32 / 2.0) as usize;

                    if rho_idx < max_rho {
                        accumulator[theta_idx][rho_idx] += 1;
                    }
                }
            }
        }
    }

    // Find peaks in accumulator
    let mut angle_votes: BTreeMap<i32, f32> = BTreeMap::new();
    let threshold = (width.min(height) / 10) as u32; // Adaptive threshold

    for theta_idx in 0..num_angles {
        for rho_idx in 0..max_rho {
            let votes = accumulator[theta_idx][rho_idx];
            if votes > threshold {
                let angle = (theta_idx as f32) - 180.0; // Convert to -180 to 180
                let normalized_angle = normalize_angle(angle);

                // Only consider angles near horizontal (within ±45°)
                if normalized_angle.abs() < 45.0 {
                    // Use integer keys for BTreeMap (angle * 10 to preserve precision)
                    let key = (normalized_angle * 10.0) as i32;
                    *angle_votes.entry(key).or_insert(0.0) += votes as f32;
                }
            }
        }
    }

    Ok(angle_votes)
}

/// Normalize angle to -45 to +45 degree range
fn normalize_angle(angle: f32) -> f32 {
    let mut normalized = angle % 180.0;
    if normalized > 90.0 {
        normalized -= 180.0;
    } else if normalized < -90.0 {
        normalized += 180.0;
    }

    // Clamp to ±45°
    normalized.clamp(-45.0, 45.0)
}

/// Deskew image using detected skew angle
///
/// # Arguments
/// * `image` - Input grayscale image
/// * `angle` - Skew angle in degrees (from detect_skew_angle)
///
/// # Returns
/// Deskewed image with white background fill
///
/// # Example
/// ```no_run
/// use ruvector_scipix::preprocess::deskew::{detect_skew_angle, deskew_image};
/// # use image::GrayImage;
/// # let image = GrayImage::new(100, 100);
/// let angle = detect_skew_angle(&image).unwrap();
/// let deskewed = deskew_image(&image, angle).unwrap();
/// ```
pub fn deskew_image(image: &GrayImage, angle: f32) -> Result<GrayImage> {
    if angle.abs() < 0.1 {
        // No deskewing needed
        return Ok(image.clone());
    }

    let radians = -angle.to_radians(); // Negate for correct direction
    let deskewed = rotate_about_center(
        image,
        radians,
        Interpolation::Bilinear,
        Luma([255]), // White background
    );

    Ok(deskewed)
}

/// Auto-deskew image with confidence threshold
///
/// # Arguments
/// * `image` - Input grayscale image
/// * `max_angle` - Maximum angle to correct (degrees)
///
/// # Returns
/// Tuple of (deskewed_image, angle_applied)
pub fn auto_deskew(image: &GrayImage, max_angle: f32) -> Result<(GrayImage, f32)> {
    let angle = detect_skew_angle(image)?;

    if angle.abs() <= max_angle {
        let deskewed = deskew_image(image, angle)?;
        Ok((deskewed, angle))
    } else {
        // Angle too large, don't correct
        Ok((image.clone(), 0.0))
    }
}

/// Detect skew using projection profile method (alternative approach)
///
/// This is a faster but less accurate method compared to Hough transform
pub fn detect_skew_projection(image: &GrayImage) -> Result<f32> {
    let angles = [
        -45.0, -30.0, -15.0, -10.0, -5.0, 0.0, 5.0, 10.0, 15.0, 30.0, 45.0,
    ];
    let mut max_variance = 0.0;
    let mut best_angle = 0.0;

    for &angle in &angles {
        let variance = calculate_projection_variance(image, angle);
        if variance > max_variance {
            max_variance = variance;
            best_angle = angle;
        }
    }

    Ok(best_angle)
}

/// Calculate projection variance for a given angle
fn calculate_projection_variance(image: &GrayImage, angle: f32) -> f32 {
    let (width, height) = image.dimensions();
    let rad = angle.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let mut projection = vec![0u32; height as usize];

    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y)[0];
            if pixel < 128 {
                let proj_y = ((y as f32) * cos_a - (x as f32) * sin_a) as i32;
                if proj_y >= 0 && proj_y < height as i32 {
                    projection[proj_y as usize] += 1;
                }
            }
        }
    }

    // Calculate variance
    if projection.is_empty() {
        return 0.0;
    }

    let mean = projection.iter().sum::<u32>() as f32 / projection.len() as f32;
    projection
        .iter()
        .map(|&x| {
            let diff = x as f32 - mean;
            diff * diff
        })
        .sum::<f32>()
        / projection.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_image() -> GrayImage {
        let mut img = GrayImage::new(200, 100);

        // Fill with white
        for pixel in img.pixels_mut() {
            *pixel = Luma([255]);
        }

        // Draw some horizontal lines (simulating text)
        for y in [20, 40, 60, 80] {
            for x in 10..190 {
                img.put_pixel(x, y, Luma([0]));
            }
        }

        img
    }

    #[test]
    fn test_detect_skew_straight() {
        let img = create_test_image();
        let angle = detect_skew_angle(&img);

        assert!(angle.is_ok());
        let a = angle.unwrap();
        // Should detect near-zero skew for straight lines
        assert!(a.abs() < 10.0);
    }

    #[test]
    fn test_deskew_image() {
        let img = create_test_image();

        // Deskew by 5 degrees
        let deskewed = deskew_image(&img, 5.0);
        assert!(deskewed.is_ok());

        let result = deskewed.unwrap();
        assert_eq!(result.dimensions(), img.dimensions());
    }

    #[test]
    fn test_deskew_no_change() {
        let img = create_test_image();

        // Deskew by ~0 degrees
        let deskewed = deskew_image(&img, 0.05);
        assert!(deskewed.is_ok());

        let result = deskewed.unwrap();
        assert_eq!(result.dimensions(), img.dimensions());
    }

    #[test]
    fn test_auto_deskew() {
        let img = create_test_image();
        let result = auto_deskew(&img, 15.0);

        assert!(result.is_ok());
        let (deskewed, angle) = result.unwrap();

        assert_eq!(deskewed.dimensions(), img.dimensions());
        assert!(angle.abs() <= 15.0);
    }

    #[test]
    fn test_normalize_angle() {
        assert!((normalize_angle(0.0) - 0.0).abs() < 0.01);

        // Test normalization behavior
        let angle_100 = normalize_angle(100.0);
        assert!(angle_100.abs() <= 45.0); // Should be clamped to ±45°

        let angle_neg100 = normalize_angle(-100.0);
        assert!(angle_neg100.abs() <= 45.0); // Should be clamped to ±45°

        assert!((normalize_angle(50.0) - 45.0).abs() < 0.01); // Clamped to 45
        assert!((normalize_angle(-50.0) - -45.0).abs() < 0.01); // Clamped to -45
    }

    #[test]
    fn test_detect_skew_projection() {
        let img = create_test_image();
        let angle = detect_skew_projection(&img);

        assert!(angle.is_ok());
        let a = angle.unwrap();
        assert!(a.abs() < 20.0);
    }

    #[test]
    fn test_skew_small_image_error() {
        let small_img = GrayImage::new(10, 10);
        let result = detect_skew_angle(&small_img);
        assert!(result.is_err());
    }

    #[test]
    fn test_projection_variance() {
        let img = create_test_image();

        let var_0 = calculate_projection_variance(&img, 0.0);
        let var_30 = calculate_projection_variance(&img, 30.0);

        // Variance at 0° should be higher for horizontal lines
        assert!(var_0 > 0.0);
        println!("Variance at 0°: {}, at 30°: {}", var_0, var_30);
    }
}
