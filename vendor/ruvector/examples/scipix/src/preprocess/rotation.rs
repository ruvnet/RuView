//! Rotation detection and correction using projection profiles

use super::{PreprocessError, Result};
use image::{GrayImage, Luma};
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use std::f32;

/// Detect rotation angle using projection profile analysis
///
/// Uses horizontal and vertical projection profiles to detect document rotation.
/// Returns angle in degrees (typically in range -45 to +45).
///
/// # Arguments
/// * `image` - Input grayscale image
///
/// # Returns
/// Rotation angle in degrees (positive = clockwise)
///
/// # Example
/// ```no_run
/// use ruvector_scipix::preprocess::rotation::detect_rotation;
/// # use image::GrayImage;
/// # let image = GrayImage::new(100, 100);
/// let angle = detect_rotation(&image).unwrap();
/// println!("Detected rotation: {:.2}°", angle);
/// ```
pub fn detect_rotation(image: &GrayImage) -> Result<f32> {
    let (width, height) = image.dimensions();

    if width < 10 || height < 10 {
        return Err(PreprocessError::InvalidParameters(
            "Image too small for rotation detection".to_string(),
        ));
    }

    // Calculate projection profiles for different angles
    let angles = [-45.0, -30.0, -15.0, 0.0, 15.0, 30.0, 45.0];
    let mut max_score = 0.0;
    let mut best_angle = 0.0;

    for &angle in &angles {
        let score = calculate_projection_score(image, angle);
        if score > max_score {
            max_score = score;
            best_angle = angle;
        }
    }

    // Refine angle with finer search around best candidate
    let fine_angles: Vec<f32> = (-5..=5).map(|i| best_angle + (i as f32) * 2.0).collect();

    max_score = 0.0;
    for angle in fine_angles {
        let score = calculate_projection_score(image, angle);
        if score > max_score {
            max_score = score;
            best_angle = angle;
        }
    }

    Ok(best_angle)
}

/// Calculate projection profile score for a given rotation angle
///
/// Higher scores indicate better alignment with text baselines
fn calculate_projection_score(image: &GrayImage, angle: f32) -> f32 {
    let (width, height) = image.dimensions();

    // For 0 degrees, use direct projection
    if angle.abs() < 0.1 {
        return calculate_horizontal_projection_variance(image);
    }

    // For non-zero angles, calculate projection along rotated axis
    let rad = angle.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let mut projection = vec![0u32; height as usize];

    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y)[0];
            if pixel < 128 {
                // Project pixel onto rotated horizontal axis
                let proj_y = ((y as f32) * cos_a - (x as f32) * sin_a) as i32;
                if proj_y >= 0 && proj_y < height as i32 {
                    projection[proj_y as usize] += 1;
                }
            }
        }
    }

    // Calculate variance of projection (higher = better alignment)
    calculate_variance(&projection)
}

/// Calculate horizontal projection variance
fn calculate_horizontal_projection_variance(image: &GrayImage) -> f32 {
    let (width, height) = image.dimensions();
    let mut projection = vec![0u32; height as usize];

    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y)[0];
            if pixel < 128 {
                projection[y as usize] += 1;
            }
        }
    }

    calculate_variance(&projection)
}

/// Calculate variance of projection profile
fn calculate_variance(projection: &[u32]) -> f32 {
    if projection.is_empty() {
        return 0.0;
    }

    let mean = projection.iter().sum::<u32>() as f32 / projection.len() as f32;
    let variance = projection
        .iter()
        .map(|&x| {
            let diff = x as f32 - mean;
            diff * diff
        })
        .sum::<f32>()
        / projection.len() as f32;

    variance
}

/// Rotate image by specified angle
///
/// # Arguments
/// * `image` - Input grayscale image
/// * `angle` - Rotation angle in degrees (positive = clockwise)
///
/// # Returns
/// Rotated image with bilinear interpolation
///
/// # Example
/// ```no_run
/// use ruvector_scipix::preprocess::rotation::rotate_image;
/// # use image::GrayImage;
/// # let image = GrayImage::new(100, 100);
/// let rotated = rotate_image(&image, 15.0).unwrap();
/// ```
pub fn rotate_image(image: &GrayImage, angle: f32) -> Result<GrayImage> {
    if angle.abs() < 0.01 {
        // No rotation needed
        return Ok(image.clone());
    }

    let radians = -angle.to_radians(); // Negate for correct direction
    let rotated = rotate_about_center(
        image,
        radians,
        Interpolation::Bilinear,
        Luma([255]), // White background
    );

    Ok(rotated)
}

/// Detect rotation with confidence score
///
/// Returns tuple of (angle, confidence) where confidence is 0.0-1.0
pub fn detect_rotation_with_confidence(image: &GrayImage) -> Result<(f32, f32)> {
    let angle = detect_rotation(image)?;

    // Calculate confidence based on projection profile variance difference
    let current_score = calculate_projection_score(image, angle);
    let baseline_score = calculate_projection_score(image, 0.0);

    // Confidence is relative improvement over baseline
    let confidence = if baseline_score > 0.0 {
        (current_score / baseline_score).min(1.0)
    } else {
        0.5 // Default moderate confidence
    };

    Ok((angle, confidence))
}

/// Auto-rotate image only if confidence is above threshold
///
/// # Arguments
/// * `image` - Input grayscale image
/// * `confidence_threshold` - Minimum confidence (0.0-1.0) to apply rotation
///
/// # Returns
/// Tuple of (rotated_image, angle_applied, confidence)
pub fn auto_rotate(image: &GrayImage, confidence_threshold: f32) -> Result<(GrayImage, f32, f32)> {
    let (angle, confidence) = detect_rotation_with_confidence(image)?;

    if confidence >= confidence_threshold && angle.abs() > 0.5 {
        let rotated = rotate_image(image, -angle)?;
        Ok((rotated, angle, confidence))
    } else {
        Ok((image.clone(), 0.0, confidence))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_text_image() -> GrayImage {
        let mut img = GrayImage::new(200, 100);

        // Fill with white
        for pixel in img.pixels_mut() {
            *pixel = Luma([255]);
        }

        // Draw some horizontal lines (simulating text)
        for y in [20, 25, 50, 55] {
            for x in 10..190 {
                img.put_pixel(x, y, Luma([0]));
            }
        }

        img
    }

    #[test]
    fn test_detect_rotation_straight() {
        let img = create_text_image();
        let angle = detect_rotation(&img);

        assert!(angle.is_ok());
        let a = angle.unwrap();
        // Should detect near-zero rotation
        assert!(a.abs() < 10.0);
    }

    #[test]
    fn test_rotate_image() {
        let img = create_text_image();

        // Rotate by 15 degrees
        let rotated = rotate_image(&img, 15.0);
        assert!(rotated.is_ok());

        let result = rotated.unwrap();
        assert_eq!(result.dimensions(), img.dimensions());
    }

    #[test]
    fn test_rotate_no_change() {
        let img = create_text_image();

        // Rotate by ~0 degrees
        let rotated = rotate_image(&img, 0.001);
        assert!(rotated.is_ok());

        let result = rotated.unwrap();
        assert_eq!(result.dimensions(), img.dimensions());
    }

    #[test]
    fn test_rotation_confidence() {
        let img = create_text_image();
        let result = detect_rotation_with_confidence(&img);

        assert!(result.is_ok());
        let (angle, confidence) = result.unwrap();

        assert!(confidence >= 0.0 && confidence <= 1.0);
        println!(
            "Detected angle: {:.2}°, confidence: {:.2}",
            angle, confidence
        );
    }

    #[test]
    fn test_auto_rotate_with_threshold() {
        let img = create_text_image();

        // High threshold - should not rotate if confidence is low
        let result = auto_rotate(&img, 0.95);
        assert!(result.is_ok());

        let (rotated, angle, confidence) = result.unwrap();
        assert_eq!(rotated.dimensions(), img.dimensions());
        println!(
            "Auto-rotate: angle={:.2}°, confidence={:.2}",
            angle, confidence
        );
    }

    #[test]
    fn test_projection_variance() {
        let projection = vec![10, 50, 100, 50, 10];
        let variance = calculate_variance(&projection);
        assert!(variance > 0.0);
    }

    #[test]
    fn test_rotation_small_image_error() {
        let small_img = GrayImage::new(5, 5);
        let result = detect_rotation(&small_img);
        assert!(result.is_err());
    }

    #[test]
    fn test_rotation_roundtrip() {
        let img = create_text_image();

        // Rotate and unrotate
        let rotated = rotate_image(&img, 30.0).unwrap();
        let unrotated = rotate_image(&rotated, -30.0).unwrap();

        assert_eq!(unrotated.dimensions(), img.dimensions());
    }
}
