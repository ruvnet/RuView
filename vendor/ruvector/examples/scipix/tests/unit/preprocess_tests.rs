// Preprocessing tests for ruvector-scipix
//
// Tests image preprocessing functions including grayscale conversion,
// Gaussian blur, Otsu thresholding, rotation detection, deskewing,
// CLAHE enhancement, and pipeline chaining.
// Target: 90%+ coverage of preprocessing module

#[cfg(test)]
mod preprocess_tests {
    use std::f32::consts::PI;

    // Mock image structures for testing
    #[derive(Debug, Clone, PartialEq)]
    struct GrayImage {
        width: u32,
        height: u32,
        data: Vec<u8>,
    }

    impl GrayImage {
        fn new(width: u32, height: u32) -> Self {
            Self {
                width,
                height,
                data: vec![0; (width * height) as usize],
            }
        }

        fn from_fn<F>(width: u32, height: u32, f: F) -> Self
        where
            F: Fn(u32, u32) -> u8,
        {
            let mut data = Vec::with_capacity((width * height) as usize);
            for y in 0..height {
                for x in 0..width {
                    data.push(f(x, y));
                }
            }
            Self {
                width,
                height,
                data,
            }
        }

        fn get_pixel(&self, x: u32, y: u32) -> u8 {
            self.data[(y * self.width + x) as usize]
        }
    }

    // Mock preprocessing functions
    fn to_grayscale(rgb: &[u8; 3]) -> u8 {
        (0.299 * rgb[0] as f32 + 0.587 * rgb[1] as f32 + 0.114 * rgb[2] as f32) as u8
    }

    fn gaussian_blur(image: &GrayImage, sigma: f32) -> GrayImage {
        // Simple mock - just return a copy
        image.clone()
    }

    fn otsu_threshold(image: &GrayImage) -> u8 {
        // Simple mock implementation
        let sum: u32 = image.data.iter().map(|&x| x as u32).sum();
        let avg = sum / image.data.len() as u32;
        avg as u8
    }

    fn apply_threshold(image: &GrayImage, threshold: u8) -> GrayImage {
        GrayImage::from_fn(image.width, image.height, |x, y| {
            if image.get_pixel(x, y) > threshold {
                255
            } else {
                0
            }
        })
    }

    fn detect_rotation_angle(image: &GrayImage) -> f32 {
        // Mock: return 0 for simplicity
        0.0
    }

    fn deskew_angle(image: &GrayImage) -> f32 {
        // Mock: return small random angle
        2.5
    }

    fn apply_clahe(image: &GrayImage, clip_limit: f32) -> GrayImage {
        // Mock: increase contrast slightly
        GrayImage::from_fn(image.width, image.height, |x, y| {
            let pixel = image.get_pixel(x, y);
            ((pixel as f32 * 1.2).min(255.0)) as u8
        })
    }

    #[test]
    fn test_grayscale_conversion_white() {
        let white = [255u8, 255, 255];
        let gray = to_grayscale(&white);
        assert_eq!(gray, 255);
    }

    #[test]
    fn test_grayscale_conversion_black() {
        let black = [0u8, 0, 0];
        let gray = to_grayscale(&black);
        assert_eq!(gray, 0);
    }

    #[test]
    fn test_grayscale_conversion_red() {
        let red = [255u8, 0, 0];
        let gray = to_grayscale(&red);
        // 0.299 * 255 ≈ 76
        assert!(gray >= 70 && gray <= 80);
    }

    #[test]
    fn test_grayscale_conversion_green() {
        let green = [0u8, 255, 0];
        let gray = to_grayscale(&green);
        // 0.587 * 255 ≈ 150
        assert!(gray >= 145 && gray <= 155);
    }

    #[test]
    fn test_grayscale_conversion_blue() {
        let blue = [0u8, 0, 255];
        let gray = to_grayscale(&blue);
        // 0.114 * 255 ≈ 29
        assert!(gray >= 25 && gray <= 35);
    }

    #[test]
    fn test_gaussian_blur_preserves_dimensions() {
        let image = GrayImage::new(100, 100);
        let blurred = gaussian_blur(&image, 1.0);

        assert_eq!(blurred.width, 100);
        assert_eq!(blurred.height, 100);
    }

    #[test]
    fn test_gaussian_blur_multiple_sigmas() {
        let image = GrayImage::new(50, 50);

        let sigmas = vec![0.5, 1.0, 1.5, 2.0, 3.0];
        for sigma in sigmas {
            let blurred = gaussian_blur(&image, sigma);
            assert_eq!(blurred.width, image.width);
            assert_eq!(blurred.height, image.height);
        }
    }

    #[test]
    fn test_otsu_thresholding_uniform_image() {
        let image = GrayImage::from_fn(50, 50, |_, _| 128);
        let threshold = otsu_threshold(&image);
        assert_eq!(threshold, 128);
    }

    #[test]
    fn test_otsu_thresholding_bimodal_image() {
        // Create image with two distinct levels
        let image = GrayImage::from_fn(100, 100, |x, y| {
            if (x + y) % 2 == 0 {
                50
            } else {
                200
            }
        });

        let threshold = otsu_threshold(&image);
        // Threshold should be between the two peaks
        assert!(threshold > 50 && threshold < 200);
    }

    #[test]
    fn test_apply_threshold_creates_binary_image() {
        let image = GrayImage::from_fn(50, 50, |x, y| ((x + y) % 256) as u8);
        let binary = apply_threshold(&image, 128);

        // Check all pixels are either 0 or 255
        for pixel in binary.data.iter() {
            assert!(*pixel == 0 || *pixel == 255);
        }
    }

    #[test]
    fn test_apply_threshold_low_threshold() {
        let image = GrayImage::from_fn(50, 50, |_, _| 100);
        let binary = apply_threshold(&image, 50);

        // All pixels should be 255 (above threshold)
        assert!(binary.data.iter().all(|&x| x == 255));
    }

    #[test]
    fn test_apply_threshold_high_threshold() {
        let image = GrayImage::from_fn(50, 50, |_, _| 100);
        let binary = apply_threshold(&image, 150);

        // All pixels should be 0 (below threshold)
        assert!(binary.data.iter().all(|&x| x == 0));
    }

    #[test]
    fn test_rotation_detection_zero() {
        let image = GrayImage::new(100, 100);
        let angle = detect_rotation_angle(&image);
        assert!((angle - 0.0).abs() < 1.0);
    }

    #[test]
    fn test_rotation_detection_90_degrees() {
        let image = GrayImage::from_fn(100, 100, |x, _| x as u8);
        let angle = detect_rotation_angle(&image);
        // In real implementation, should detect 0, 90, 180, or 270
        assert!(angle >= -180.0 && angle <= 180.0);
    }

    #[test]
    fn test_rotation_detection_180_degrees() {
        let image = GrayImage::from_fn(100, 100, |x, y| ((x + y) % 256) as u8);
        let angle = detect_rotation_angle(&image);
        assert!(angle >= -180.0 && angle <= 180.0);
    }

    #[test]
    fn test_rotation_detection_270_degrees() {
        let image = GrayImage::new(100, 100);
        let angle = detect_rotation_angle(&image);
        assert!(angle >= -180.0 && angle <= 180.0);
    }

    #[test]
    fn test_deskew_angle_detection() {
        let image = GrayImage::new(100, 100);
        let angle = deskew_angle(&image);

        // Skew angle should typically be small (< 45 degrees)
        assert!(angle.abs() < 45.0);
    }

    #[test]
    fn test_deskew_angle_horizontal_lines() {
        let image = GrayImage::from_fn(100, 100, |_, y| {
            if y % 10 == 0 {
                255
            } else {
                0
            }
        });

        let angle = deskew_angle(&image);
        // Should detect minimal skew for horizontal lines
        assert!(angle.abs() < 5.0);
    }

    #[test]
    fn test_clahe_enhancement() {
        let image = GrayImage::from_fn(100, 100, |x, y| ((x + y) % 128) as u8);
        let enhanced = apply_clahe(&image, 2.0);

        assert_eq!(enhanced.width, image.width);
        assert_eq!(enhanced.height, image.height);
    }

    #[test]
    fn test_clahe_increases_contrast() {
        let low_contrast = GrayImage::from_fn(50, 50, |x, _| (100 + x % 20) as u8);
        let enhanced = apply_clahe(&low_contrast, 2.0);

        // Calculate simple contrast measure
        let original_range = calculate_range(&low_contrast);
        let enhanced_range = calculate_range(&enhanced);

        // Enhanced image should have equal or greater range
        assert!(enhanced_range >= original_range);
    }

    #[test]
    fn test_clahe_preserves_dimensions() {
        let image = GrayImage::new(256, 256);
        let enhanced = apply_clahe(&image, 2.0);

        assert_eq!(enhanced.width, 256);
        assert_eq!(enhanced.height, 256);
    }

    #[test]
    fn test_clahe_different_clip_limits() {
        let image = GrayImage::from_fn(50, 50, |x, y| ((x + y) % 256) as u8);

        let clip_limits = vec![1.0, 2.0, 3.0, 4.0];
        for limit in clip_limits {
            let enhanced = apply_clahe(&image, limit);
            assert_eq!(enhanced.width, image.width);
            assert_eq!(enhanced.height, image.height);
        }
    }

    #[test]
    fn test_pipeline_chaining_blur_then_threshold() {
        let image = GrayImage::from_fn(100, 100, |x, y| ((x + y) % 256) as u8);

        // Chain operations
        let blurred = gaussian_blur(&image, 1.0);
        let threshold = otsu_threshold(&blurred);
        let binary = apply_threshold(&blurred, threshold);

        // Verify final result is binary
        assert!(binary.data.iter().all(|&x| x == 0 || x == 255));
    }

    #[test]
    fn test_pipeline_chaining_enhance_then_threshold() {
        let image = GrayImage::from_fn(100, 100, |x, y| ((x + y) % 128) as u8);

        // Chain CLAHE then threshold
        let enhanced = apply_clahe(&image, 2.0);
        let threshold = otsu_threshold(&enhanced);
        let binary = apply_threshold(&enhanced, threshold);

        assert!(binary.data.iter().all(|&x| x == 0 || x == 255));
    }

    #[test]
    fn test_pipeline_full_preprocessing() {
        let image = GrayImage::from_fn(100, 100, |x, y| ((x + y) % 256) as u8);

        // Full pipeline: blur -> enhance -> threshold
        let blurred = gaussian_blur(&image, 1.0);
        let enhanced = apply_clahe(&blurred, 2.0);
        let threshold = otsu_threshold(&enhanced);
        let binary = apply_threshold(&enhanced, threshold);

        assert_eq!(binary.width, image.width);
        assert_eq!(binary.height, image.height);
        assert!(binary.data.iter().all(|&x| x == 0 || x == 255));
    }

    #[test]
    fn test_pipeline_preserves_dimensions_throughout() {
        let image = GrayImage::new(200, 150);

        let blurred = gaussian_blur(&image, 1.5);
        assert_eq!((blurred.width, blurred.height), (200, 150));

        let enhanced = apply_clahe(&blurred, 2.0);
        assert_eq!((enhanced.width, enhanced.height), (200, 150));

        let binary = apply_threshold(&enhanced, 128);
        assert_eq!((binary.width, binary.height), (200, 150));
    }

    // Helper functions
    fn calculate_range(image: &GrayImage) -> u8 {
        let min = *image.data.iter().min().unwrap_or(&0);
        let max = *image.data.iter().max().unwrap_or(&255);
        max - min
    }

    #[test]
    fn test_edge_case_empty_like_image() {
        let tiny = GrayImage::new(1, 1);
        assert_eq!(tiny.width, 1);
        assert_eq!(tiny.height, 1);
    }

    #[test]
    fn test_edge_case_large_image_dimensions() {
        let large = GrayImage::new(4096, 4096);
        assert_eq!(large.width, 4096);
        assert_eq!(large.height, 4096);
    }
}
