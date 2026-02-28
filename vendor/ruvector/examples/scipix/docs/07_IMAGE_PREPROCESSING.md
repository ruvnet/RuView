# Image Preprocessing Pipeline for Optimal OCR Performance

## Overview

This document details the image preprocessing pipeline optimized for OCR performance in the ruvector-scipix project. The pipeline transforms raw images into clean, normalized inputs suitable for mathematical OCR models.

## Architecture

```
Raw Image → Load/Decode → Enhancement → Geometric Correction →
Resolution Normalization → Text Region Detection → OCR-Ready Output
```

## 1. Image Loading and Decoding

### Format Support

Support all common image formats with efficient decoding strategies:

```rust
use image::{DynamicImage, ImageFormat, ImageError, GenericImageView};
use std::path::Path;
use std::io::BufReader;
use std::fs::File;

pub struct ImageLoader {
    max_dimension: u32,
    supported_formats: Vec<ImageFormat>,
}

impl ImageLoader {
    pub fn new() -> Self {
        Self {
            max_dimension: 8192, // Prevent memory exhaustion
            supported_formats: vec![
                ImageFormat::Jpeg,
                ImageFormat::Png,
                ImageFormat::Tiff,
                ImageFormat::WebP,
                ImageFormat::Bmp,
                ImageFormat::Gif,
            ],
        }
    }

    /// Load image with memory-efficient strategy
    pub fn load<P: AsRef<Path>>(&self, path: P) -> Result<DynamicImage, ImageError> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        // Load image with format detection
        let img = image::load(reader, ImageFormat::from_path(path.as_ref())?)?;

        // Validate dimensions to prevent memory issues
        let (width, height) = img.dimensions();
        if width > self.max_dimension || height > self.max_dimension {
            return Err(ImageError::Limits(image::error::LimitError::from_kind(
                image::error::LimitErrorKind::DimensionError
            )));
        }

        Ok(img)
    }

    /// Load image from bytes with format hint
    pub fn load_from_memory(&self, buffer: &[u8], format: ImageFormat) -> Result<DynamicImage, ImageError> {
        image::load_from_memory_with_format(buffer, format)
    }

    /// Load with progressive decoding for large images
    pub fn load_progressive<P: AsRef<Path>>(&self, path: P) -> Result<DynamicImage, ImageError> {
        let file = File::open(path.as_ref())?;
        let reader = BufReader::with_capacity(8192, file);

        // For JPEG, enable progressive decoding
        if let Some(ext) = path.as_ref().extension() {
            if ext.to_str().unwrap_or("").to_lowercase() == "jpg" ||
               ext.to_str().unwrap_or("").to_lowercase() == "jpeg" {
                return image::load(reader, ImageFormat::Jpeg);
            }
        }

        image::load(reader, ImageFormat::from_path(path)?)
    }
}

/// Zero-copy image loading for supported formats
pub fn load_zero_copy<P: AsRef<Path>>(path: P) -> Result<DynamicImage, ImageError> {
    // For formats that support memory mapping
    image::open(path)
}
```

### Memory-Efficient Loading Strategies

```rust
use image::{GenericImageView, ImageBuffer, Rgba};

pub struct StreamingLoader {
    chunk_size: usize,
}

impl StreamingLoader {
    /// Load large images in chunks to minimize memory footprint
    pub fn load_chunked<P: AsRef<Path>>(
        &self,
        path: P,
        callback: impl Fn(&ImageBuffer<Rgba<u8>, Vec<u8>>) -> Result<(), ImageError>
    ) -> Result<(), ImageError> {
        let img = image::open(path)?;
        let (width, height) = img.dimensions();

        let chunk_height = (self.chunk_size as u32).min(height);

        for y in (0..height).step_by(chunk_height as usize) {
            let h = chunk_height.min(height - y);
            let chunk = img.crop_imm(0, y, width, h).to_rgba8();
            callback(&chunk)?;
        }

        Ok(())
    }
}
```

## 2. Image Enhancement

### Contrast Adjustment (CLAHE)

```rust
use imageproc::contrast::adaptive_threshold;
use imageproc::filter::{gaussian_blur_f32, bilateral_filter};
use image::{GrayImage, Luma};

pub struct ImageEnhancer {
    clahe_clip_limit: f32,
    clahe_tile_size: (u32, u32),
}

impl ImageEnhancer {
    pub fn new() -> Self {
        Self {
            clahe_clip_limit: 2.0,
            clahe_tile_size: (8, 8),
        }
    }

    /// Apply Contrast Limited Adaptive Histogram Equalization
    pub fn apply_clahe(&self, img: &GrayImage) -> GrayImage {
        // Simplified CLAHE implementation
        // For production, use a dedicated CLAHE library
        let (width, height) = img.dimensions();
        let (tile_w, tile_h) = self.clahe_tile_size;

        let mut output = GrayImage::new(width, height);

        // Process each tile
        for tile_y in (0..height).step_by(tile_h as usize) {
            for tile_x in (0..width).step_by(tile_w as usize) {
                let w = tile_w.min(width - tile_x);
                let h = tile_h.min(height - tile_y);

                // Extract tile
                let tile = imageproc::rect::Rect::at(tile_x as i32, tile_y as i32)
                    .of_size(w, h);

                // Compute histogram and equalize with clipping
                let equalized = self.equalize_tile_with_clip(img, tile);

                // Copy back to output
                for y in 0..h {
                    for x in 0..w {
                        output.put_pixel(
                            tile_x + x,
                            tile_y + y,
                            equalized[(y * w + x) as usize]
                        );
                    }
                }
            }
        }

        output
    }

    fn equalize_tile_with_clip(&self, img: &GrayImage, rect: imageproc::rect::Rect) -> Vec<Luma<u8>> {
        let mut histogram = [0u32; 256];
        let mut pixels = Vec::new();

        // Build histogram
        for y in rect.top()..(rect.top() + rect.height() as i32) {
            for x in rect.left()..(rect.left() + rect.width() as i32) {
                if x >= 0 && y >= 0 {
                    let pixel = img.get_pixel(x as u32, y as u32)[0];
                    histogram[pixel as usize] += 1;
                    pixels.push(Luma([pixel]));
                }
            }
        }

        // Apply clip limit
        let total_pixels = pixels.len() as u32;
        let clip_limit = ((self.clahe_clip_limit * total_pixels as f32) / 256.0) as u32;

        let mut clipped_total = 0u32;
        for count in &mut histogram {
            if *count > clip_limit {
                clipped_total += *count - clip_limit;
                *count = clip_limit;
            }
        }

        // Redistribute clipped pixels
        let redistribute = clipped_total / 256;
        for count in &mut histogram {
            *count += redistribute;
        }

        // Build CDF
        let mut cdf = [0u32; 256];
        cdf[0] = histogram[0];
        for i in 1..256 {
            cdf[i] = cdf[i - 1] + histogram[i];
        }

        // Normalize and map pixels
        let cdf_min = cdf.iter().find(|&&x| x > 0).copied().unwrap_or(0);
        let cdf_range = total_pixels - cdf_min;

        pixels.into_iter().map(|pixel| {
            let old_val = pixel[0] as usize;
            let new_val = if cdf_range > 0 {
                (((cdf[old_val] - cdf_min) as f32 / cdf_range as f32) * 255.0) as u8
            } else {
                pixel[0]
            };
            Luma([new_val])
        }).collect()
    }
}
```

### Noise Reduction

```rust
use imageproc::filter::{gaussian_blur_f32, median_filter};

pub struct NoiseReducer;

impl NoiseReducer {
    /// Apply Gaussian blur for noise reduction
    pub fn gaussian_denoise(img: &GrayImage, sigma: f32) -> GrayImage {
        gaussian_blur_f32(img, sigma)
    }

    /// Apply bilateral filter (edge-preserving)
    pub fn bilateral_denoise(
        img: &GrayImage,
        spatial_sigma: f32,
        range_sigma: f32
    ) -> GrayImage {
        let (width, height) = img.dimensions();
        let mut output = GrayImage::new(width, height);

        let kernel_radius = (3.0 * spatial_sigma).ceil() as i32;

        for y in 0..height {
            for x in 0..width {
                let center_val = img.get_pixel(x, y)[0] as f32;
                let mut sum_weights = 0.0f32;
                let mut sum_values = 0.0f32;

                for dy in -kernel_radius..=kernel_radius {
                    for dx in -kernel_radius..=kernel_radius {
                        let nx = (x as i32 + dx).clamp(0, width as i32 - 1) as u32;
                        let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as u32;

                        let neighbor_val = img.get_pixel(nx, ny)[0] as f32;

                        // Spatial weight
                        let spatial_dist = ((dx * dx + dy * dy) as f32).sqrt();
                        let spatial_weight = (-spatial_dist * spatial_dist /
                            (2.0 * spatial_sigma * spatial_sigma)).exp();

                        // Range weight
                        let range_dist = (center_val - neighbor_val).abs();
                        let range_weight = (-range_dist * range_dist /
                            (2.0 * range_sigma * range_sigma)).exp();

                        let weight = spatial_weight * range_weight;
                        sum_weights += weight;
                        sum_values += weight * neighbor_val;
                    }
                }

                let filtered_val = (sum_values / sum_weights).round() as u8;
                output.put_pixel(x, y, Luma([filtered_val]));
            }
        }

        output
    }

    /// Apply median filter for impulse noise
    pub fn median_denoise(img: &GrayImage, kernel_size: u32) -> GrayImage {
        median_filter(img, kernel_size, kernel_size)
    }
}
```

### Sharpening Filters

```rust
use imageproc::filter::sharpen3x3;

pub struct Sharpener;

impl Sharpener {
    /// Apply unsharp mask sharpening
    pub fn unsharp_mask(img: &GrayImage, sigma: f32, amount: f32) -> GrayImage {
        let blurred = gaussian_blur_f32(img, sigma);
        let (width, height) = img.dimensions();
        let mut output = GrayImage::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let original = img.get_pixel(x, y)[0] as f32;
                let blur = blurred.get_pixel(x, y)[0] as f32;
                let sharpened = original + amount * (original - blur);
                output.put_pixel(x, y, Luma([sharpened.clamp(0.0, 255.0) as u8]));
            }
        }

        output
    }

    /// Apply 3x3 sharpening kernel
    pub fn sharpen_3x3(img: &GrayImage) -> GrayImage {
        sharpen3x3(img)
    }

    /// Apply Laplacian sharpening
    pub fn laplacian_sharpen(img: &GrayImage, strength: f32) -> GrayImage {
        let (width, height) = img.dimensions();
        let mut output = GrayImage::new(width, height);

        // Laplacian kernel
        let kernel = [
            [0, -1, 0],
            [-1, 4, -1],
            [0, -1, 0],
        ];

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let mut laplacian = 0.0f32;

                for ky in 0..3 {
                    for kx in 0..3 {
                        let px = img.get_pixel(x + kx - 1, y + ky - 1)[0] as f32;
                        laplacian += px * kernel[ky][kx] as f32;
                    }
                }

                let original = img.get_pixel(x, y)[0] as f32;
                let sharpened = original + strength * laplacian;
                output.put_pixel(x, y, Luma([sharpened.clamp(0.0, 255.0) as u8]));
            }
        }

        output
    }
}
```

### Binarization

```rust
use imageproc::contrast::{otsu_level, threshold, ThresholdType};

pub struct Binarizer;

impl Binarizer {
    /// Apply Otsu's automatic thresholding
    pub fn otsu_binarize(img: &GrayImage) -> GrayImage {
        let threshold_value = otsu_level(img);
        threshold(img, threshold_value, ThresholdType::Binary)
    }

    /// Apply adaptive thresholding
    pub fn adaptive_binarize(img: &GrayImage, block_size: u32, c: i32) -> GrayImage {
        adaptive_threshold(img, block_size)
    }

    /// Apply Sauvola's local thresholding (good for varied illumination)
    pub fn sauvola_binarize(img: &GrayImage, window_size: u32, k: f32) -> GrayImage {
        let (width, height) = img.dimensions();
        let mut output = GrayImage::new(width, height);
        let half_window = (window_size / 2) as i32;

        for y in 0..height {
            for x in 0..width {
                // Compute local mean and standard deviation
                let mut sum = 0.0f32;
                let mut sum_sq = 0.0f32;
                let mut count = 0u32;

                for dy in -half_window..=half_window {
                    for dx in -half_window..=half_window {
                        let nx = (x as i32 + dx).clamp(0, width as i32 - 1) as u32;
                        let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as u32;
                        let val = img.get_pixel(nx, ny)[0] as f32;
                        sum += val;
                        sum_sq += val * val;
                        count += 1;
                    }
                }

                let mean = sum / count as f32;
                let variance = (sum_sq / count as f32) - (mean * mean);
                let std_dev = variance.sqrt();

                // Sauvola threshold
                let threshold = mean * (1.0 + k * ((std_dev / 128.0) - 1.0));
                let pixel_val = img.get_pixel(x, y)[0] as f32;

                let binary = if pixel_val > threshold { 255 } else { 0 };
                output.put_pixel(x, y, Luma([binary]));
            }
        }

        output
    }
}
```

## 3. Geometric Corrections

### Auto-Rotation Detection

```rust
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};
use std::f32::consts::PI;

pub struct RotationDetector;

impl RotationDetector {
    /// Detect rotation angle using projection profiles
    pub fn detect_rotation_angle(img: &GrayImage) -> f32 {
        let angles = [-90.0, -45.0, -30.0, -15.0, 0.0, 15.0, 30.0, 45.0, 90.0, 180.0];
        let mut max_variance = 0.0f32;
        let mut best_angle = 0.0f32;

        for &angle in &angles {
            let rotated = Self::rotate_image(img, angle);
            let variance = Self::compute_horizontal_projection_variance(&rotated);

            if variance > max_variance {
                max_variance = variance;
                best_angle = angle;
            }
        }

        best_angle
    }

    /// Compute variance of horizontal projection profile
    fn compute_horizontal_projection_variance(img: &GrayImage) -> f32 {
        let (width, height) = img.dimensions();
        let mut projections = vec![0u32; height as usize];

        for y in 0..height {
            let mut sum = 0u32;
            for x in 0..width {
                sum += img.get_pixel(x, y)[0] as u32;
            }
            projections[y as usize] = sum;
        }

        // Compute variance
        let mean = projections.iter().sum::<u32>() as f32 / height as f32;
        let variance = projections.iter()
            .map(|&x| (x as f32 - mean).powi(2))
            .sum::<f32>() / height as f32;

        variance
    }

    /// Rotate image by specified angle
    pub fn rotate_image(img: &GrayImage, angle_degrees: f32) -> GrayImage {
        let (width, height) = img.dimensions();
        let center = ((width / 2) as f32, (height / 2) as f32);
        let angle_rad = angle_degrees * PI / 180.0;

        rotate_about_center(img, center, angle_rad, Interpolation::Bilinear, Luma([255]))
    }

    /// Detect and correct common rotation angles (0, 90, -90, 180)
    pub fn auto_correct_rotation(img: &GrayImage) -> (GrayImage, f32) {
        let angle = Self::detect_rotation_angle(img);
        let corrected = Self::rotate_image(img, -angle);
        (corrected, angle)
    }
}
```

### Deskewing Algorithms

```rust
use imageproc::hough::detect_lines;

pub struct Deskewer;

impl Deskewer {
    /// Detect skew angle using Hough transform
    pub fn detect_skew_hough(img: &GrayImage) -> f32 {
        // Apply edge detection first
        let edges = imageproc::edges::canny(img, 50.0, 100.0);

        // Use Hough transform to detect dominant lines
        let lines = detect_lines(&edges, 1, PI / 180.0, 50);

        if lines.is_empty() {
            return 0.0;
        }

        // Extract angles from detected lines
        let mut angles: Vec<f32> = lines.iter()
            .map(|line| {
                let theta = line.angle_in_degrees();
                // Normalize to [-45, 45] range
                if theta > 45.0 {
                    theta - 90.0
                } else if theta < -45.0 {
                    theta + 90.0
                } else {
                    theta
                }
            })
            .collect();

        // Find median angle
        angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_angle = if angles.len() % 2 == 0 {
            (angles[angles.len() / 2 - 1] + angles[angles.len() / 2]) / 2.0
        } else {
            angles[angles.len() / 2]
        };

        median_angle
    }

    /// Detect skew using projection profiles
    pub fn detect_skew_projection(img: &GrayImage) -> f32 {
        let test_angles: Vec<f32> = (-45..=45).step_by(1).map(|x| x as f32).collect();
        let mut max_variance = 0.0f32;
        let mut best_angle = 0.0f32;

        for angle in test_angles {
            let rotated = RotationDetector::rotate_image(img, angle);
            let variance = Self::compute_projection_variance(&rotated);

            if variance > max_variance {
                max_variance = variance;
                best_angle = angle;
            }
        }

        -best_angle // Negate to get correction angle
    }

    fn compute_projection_variance(img: &GrayImage) -> f32 {
        let (width, height) = img.dimensions();
        let mut row_sums = vec![0u32; height as usize];

        for y in 0..height {
            for x in 0..width {
                row_sums[y as usize] += img.get_pixel(x, y)[0] as u32;
            }
        }

        let mean = row_sums.iter().sum::<u32>() as f32 / height as f32;
        row_sums.iter()
            .map(|&x| (x as f32 - mean).powi(2))
            .sum::<f32>() / height as f32
    }

    /// Apply deskewing correction
    pub fn deskew(img: &GrayImage) -> (GrayImage, f32) {
        let angle = Self::detect_skew_hough(img);
        let corrected = RotationDetector::rotate_image(img, -angle);
        (corrected, angle)
    }
}
```

### Perspective Correction

```rust
use imageproc::geometric_transformations::warp;
use imageproc::geometric_transformations::Projection;

pub struct PerspectiveCorrector;

impl PerspectiveCorrector {
    /// Detect document corners
    pub fn detect_corners(img: &GrayImage) -> Option<[(f32, f32); 4]> {
        // Apply edge detection
        let edges = imageproc::edges::canny(img, 50.0, 150.0);

        // Find contours (simplified - use a contour detection library in production)
        // For now, assume corners are provided or detected via another method

        // Return corners in order: top-left, top-right, bottom-right, bottom-left
        None // Placeholder
    }

    /// Apply perspective transformation
    pub fn correct_perspective(
        img: &GrayImage,
        src_corners: [(f32, f32); 4],
        output_width: u32,
        output_height: u32
    ) -> GrayImage {
        // Define destination corners (rectangle)
        let dst_corners = [
            (0.0, 0.0),
            (output_width as f32, 0.0),
            (output_width as f32, output_height as f32),
            (0.0, output_height as f32),
        ];

        // Compute perspective transformation matrix
        let projection = Self::compute_perspective_transform(&src_corners, &dst_corners);

        // Apply transformation
        warp(img, &projection, Interpolation::Bilinear, Luma([255]))
    }

    /// Compute perspective transformation matrix
    fn compute_perspective_transform(
        src: &[(f32, f32); 4],
        dst: &[(f32, f32); 4]
    ) -> Projection {
        // Implement perspective matrix computation
        // Using homography estimation (Direct Linear Transform)

        // Placeholder - use a linear algebra library like nalgebra
        Projection::translate(0.0, 0.0)
    }
}
```

## 4. Resolution Handling

### Optimal Resolution for OCR

```rust
use image::imageops::{FilterType, resize};

pub struct ResolutionHandler {
    target_size: (u32, u32),
    min_size: (u32, u32),
    max_size: (u32, u32),
}

impl ResolutionHandler {
    pub fn new() -> Self {
        Self {
            target_size: (384, 384), // Optimal for most OCR models
            min_size: (224, 224),
            max_size: (640, 640),
        }
    }

    /// Resize image to optimal resolution
    pub fn normalize_resolution(&self, img: &DynamicImage) -> DynamicImage {
        let (width, height) = img.dimensions();

        // Check if resize is needed
        if width >= self.min_size.0 && width <= self.max_size.0 &&
           height >= self.min_size.1 && height <= self.max_size.1 {
            return img.clone();
        }

        // Compute scaling factor while preserving aspect ratio
        let scale = self.compute_scale_factor(width, height);
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;

        // Use high-quality filter for upscaling, faster filter for downscaling
        let filter = if scale > 1.0 {
            FilterType::Lanczos3
        } else {
            FilterType::Triangle
        };

        DynamicImage::ImageRgba8(resize(img, new_width, new_height, filter))
    }

    /// Compute optimal scale factor
    fn compute_scale_factor(&self, width: u32, height: u32) -> f32 {
        let target_w = self.target_size.0 as f32;
        let target_h = self.target_size.1 as f32;

        let scale_w = target_w / width as f32;
        let scale_h = target_h / height as f32;

        // Use the minimum scale to ensure entire image fits
        scale_w.min(scale_h)
    }

    /// Upscale with super-resolution (placeholder for SR models)
    pub fn upscale_sr(&self, img: &DynamicImage, factor: u32) -> DynamicImage {
        // Placeholder for super-resolution model
        // In production, integrate with ONNX/TensorFlow models
        let (width, height) = img.dimensions();
        DynamicImage::ImageRgba8(resize(
            img,
            width * factor,
            height * factor,
            FilterType::Lanczos3
        ))
    }

    /// Preserve aspect ratio with padding
    pub fn resize_with_padding(&self, img: &DynamicImage) -> DynamicImage {
        let (width, height) = img.dimensions();
        let (target_w, target_h) = self.target_size;

        let scale = (target_w as f32 / width as f32).min(target_h as f32 / height as f32);
        let new_width = (width as f32 * scale) as u32;
        let new_height = (height as f32 * scale) as u32;

        // Resize image
        let resized = resize(img, new_width, new_height, FilterType::Lanczos3);

        // Create padded image
        let mut padded = DynamicImage::new_rgba8(target_w, target_h);
        let x_offset = (target_w - new_width) / 2;
        let y_offset = (target_h - new_height) / 2;

        image::imageops::overlay(&mut padded, &resized, x_offset as i64, y_offset as i64);

        padded
    }
}
```

## 5. Text Region Detection

### Connected Component Analysis

```rust
use imageproc::region_labelling::{connected_components, Connectivity};
use imageproc::rect::Rect;

pub struct TextDetector {
    min_component_size: u32,
    max_component_size: u32,
}

impl TextDetector {
    pub fn new() -> Self {
        Self {
            min_component_size: 10,
            max_component_size: 10000,
        }
    }

    /// Detect text regions using connected components
    pub fn detect_text_regions(&self, binary_img: &GrayImage) -> Vec<Rect> {
        let labeled = connected_components(binary_img, Connectivity::Eight, Luma([0]));
        let mut components = std::collections::HashMap::new();

        // Group pixels by label
        for y in 0..labeled.height() {
            for x in 0..labeled.width() {
                let label = labeled.get_pixel(x, y)[0];
                if label > 0 {
                    components.entry(label)
                        .or_insert_with(Vec::new)
                        .push((x, y));
                }
            }
        }

        // Compute bounding boxes
        let mut bboxes = Vec::new();
        for (_, pixels) in components {
            if pixels.len() < self.min_component_size as usize ||
               pixels.len() > self.max_component_size as usize {
                continue;
            }

            let min_x = pixels.iter().map(|(x, _)| x).min().unwrap();
            let max_x = pixels.iter().map(|(x, _)| x).max().unwrap();
            let min_y = pixels.iter().map(|(_, y)| y).min().unwrap();
            let max_y = pixels.iter().map(|(_, y)| y).max().unwrap();

            bboxes.push(Rect::at(*min_x as i32, *min_y as i32)
                .of_size((max_x - min_x + 1), (max_y - min_y + 1)));
        }

        bboxes
    }

    /// Merge nearby bounding boxes (for text lines)
    pub fn merge_bboxes(&self, bboxes: Vec<Rect>, max_distance: u32) -> Vec<Rect> {
        if bboxes.is_empty() {
            return Vec::new();
        }

        let mut merged = Vec::new();
        let mut used = vec![false; bboxes.len()];

        for i in 0..bboxes.len() {
            if used[i] {
                continue;
            }

            let mut current = bboxes[i];
            used[i] = true;

            // Try to merge with other boxes
            let mut changed = true;
            while changed {
                changed = false;
                for j in 0..bboxes.len() {
                    if used[j] {
                        continue;
                    }

                    if Self::boxes_close(&current, &bboxes[j], max_distance) {
                        current = Self::union_rect(&current, &bboxes[j]);
                        used[j] = true;
                        changed = true;
                    }
                }
            }

            merged.push(current);
        }

        merged
    }

    fn boxes_close(a: &Rect, b: &Rect, max_distance: u32) -> bool {
        let dx = if a.left() > b.right() {
            a.left() - b.right()
        } else if b.left() > a.right() {
            b.left() - a.right()
        } else {
            0
        };

        let dy = if a.top() > b.bottom() {
            a.top() - b.bottom()
        } else if b.top() > a.bottom() {
            b.top() - a.bottom()
        } else {
            0
        };

        (dx * dx + dy * dy) <= (max_distance * max_distance) as i32
    }

    fn union_rect(a: &Rect, b: &Rect) -> Rect {
        let left = a.left().min(b.left());
        let top = a.top().min(b.top());
        let right = a.right().max(b.right());
        let bottom = a.bottom().max(b.bottom());

        Rect::at(left, top).of_size((right - left) as u32, (bottom - top) as u32)
    }
}
```

### Line Segmentation

```rust
pub struct LineSegmenter;

impl LineSegmenter {
    /// Segment image into text lines using horizontal projection
    pub fn segment_lines(img: &GrayImage) -> Vec<(u32, u32)> {
        let (_, height) = img.dimensions();
        let projection = Self::horizontal_projection(img);

        // Find valleys in projection (gaps between lines)
        let mut lines = Vec::new();
        let mut in_line = false;
        let mut line_start = 0u32;

        let threshold = Self::compute_threshold(&projection);

        for (y, &val) in projection.iter().enumerate() {
            if val > threshold && !in_line {
                line_start = y as u32;
                in_line = true;
            } else if val <= threshold && in_line {
                lines.push((line_start, y as u32));
                in_line = false;
            }
        }

        // Handle last line
        if in_line {
            lines.push((line_start, height));
        }

        lines
    }

    fn horizontal_projection(img: &GrayImage) -> Vec<u32> {
        let (width, height) = img.dimensions();
        let mut projection = vec![0u32; height as usize];

        for y in 0..height {
            let mut sum = 0u32;
            for x in 0..width {
                // Count black pixels (text)
                if img.get_pixel(x, y)[0] < 128 {
                    sum += 1;
                }
            }
            projection[y as usize] = sum;
        }

        projection
    }

    fn compute_threshold(projection: &[u32]) -> u32 {
        let max_val = projection.iter().max().copied().unwrap_or(0);
        max_val / 10 // 10% of maximum
    }
}
```

## 6. PDF/Document Processing

### PDF Rasterization

```rust
// Note: Requires pdfium-render or pdf crate
// This is a conceptual implementation

use std::path::Path;

pub struct PdfProcessor {
    dpi: u32,
    page_limit: Option<usize>,
}

impl PdfProcessor {
    pub fn new() -> Self {
        Self {
            dpi: 300, // Standard DPI for OCR
            page_limit: Some(100), // Prevent processing huge PDFs
        }
    }

    /// Rasterize PDF to images
    /// Note: Requires pdfium-render crate
    pub fn rasterize_pdf<P: AsRef<Path>>(
        &self,
        pdf_path: P
    ) -> Result<Vec<DynamicImage>, Box<dyn std::error::Error>> {
        // Placeholder - actual implementation requires pdfium-render
        //
        // Example with pdfium-render:
        // let pdfium = Pdfium::new(...)?;
        // let document = pdfium.load_pdf_from_file(pdf_path, None)?;
        //
        // let mut images = Vec::new();
        // for page_index in 0..document.pages().len() {
        //     let page = document.pages().get(page_index)?;
        //     let render_config = PdfRenderConfig::new()
        //         .set_target_width(((page.width() * self.dpi as f32) / 72.0) as u32)
        //         .set_target_height(((page.height() * self.dpi as f32) / 72.0) as u32);
        //
        //     let bitmap = page.render_with_config(&render_config)?;
        //     let image = bitmap.as_image();
        //     images.push(image);
        // }

        Ok(Vec::new())
    }

    /// Extract embedded images from PDF
    pub fn extract_images<P: AsRef<Path>>(
        &self,
        pdf_path: P
    ) -> Result<Vec<DynamicImage>, Box<dyn std::error::Error>> {
        // Placeholder for image extraction
        // Actual implementation requires PDF parsing
        Ok(Vec::new())
    }

    /// Process multi-page PDF
    pub fn process_multipage<P: AsRef<Path>>(
        &self,
        pdf_path: P,
        processor: impl Fn(DynamicImage) -> Result<(), Box<dyn std::error::Error>>
    ) -> Result<(), Box<dyn std::error::Error>> {
        let images = self.rasterize_pdf(pdf_path)?;

        for (i, img) in images.into_iter().enumerate() {
            if let Some(limit) = self.page_limit {
                if i >= limit {
                    break;
                }
            }
            processor(img)?;
        }

        Ok(())
    }
}
```

## 7. Memory and Performance Optimization

### Zero-Copy Operations

```rust
use std::sync::Arc;
use image::ImageBuffer;

pub struct OptimizedProcessor {
    use_zero_copy: bool,
}

impl OptimizedProcessor {
    /// Process image with zero-copy where possible
    pub fn process_zero_copy(img: Arc<DynamicImage>) -> Arc<DynamicImage> {
        // Use Arc to share image data without copying
        // Apply non-mutating operations
        img
    }

    /// Use image views instead of copying
    pub fn process_view<'a>(img: &'a DynamicImage) -> &'a DynamicImage {
        // Return views when possible
        img
    }
}
```

### SIMD Acceleration

```rust
// Enable SIMD for performance-critical operations
#[cfg(target_feature = "avx2")]
use std::arch::x86_64::*;

pub struct SimdProcessor;

impl SimdProcessor {
    /// SIMD-accelerated threshold operation
    #[cfg(target_feature = "avx2")]
    pub unsafe fn threshold_simd(data: &mut [u8], threshold: u8) {
        let threshold_vec = _mm256_set1_epi8(threshold as i8);
        let chunks = data.chunks_exact_mut(32);

        for chunk in chunks {
            let values = _mm256_loadu_si256(chunk.as_ptr() as *const __m256i);
            let mask = _mm256_cmpgt_epi8(values, threshold_vec);
            let result = _mm256_and_si256(mask, _mm256_set1_epi8(-1));
            _mm256_storeu_si256(chunk.as_mut_ptr() as *mut __m256i, result);
        }
    }

    /// Portable version without SIMD
    pub fn threshold_portable(data: &mut [u8], threshold: u8) {
        for pixel in data {
            *pixel = if *pixel > threshold { 255 } else { 0 };
        }
    }
}
```

### GPU Preprocessing (via wgpu)

```rust
// Conceptual GPU preprocessing using wgpu
use wgpu;

pub struct GpuPreprocessor {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GpuPreprocessor {
    /// Initialize GPU context
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .ok_or("Failed to find adapter")?;

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor::default(),
            None
        ).await?;

        Ok(Self { device, queue })
    }

    /// GPU-accelerated gaussian blur
    pub fn gpu_gaussian_blur(&self, img: &GrayImage, sigma: f32) -> GrayImage {
        // Implement GPU-based blur using compute shaders
        // Upload image to GPU, run shader, download result
        img.clone() // Placeholder
    }

    /// Batch processing on GPU
    pub fn batch_process_gpu(&self, images: Vec<GrayImage>) -> Vec<GrayImage> {
        // Process multiple images in parallel on GPU
        images // Placeholder
    }
}
```

### Batch Operations

```rust
use rayon::prelude::*;

pub struct BatchProcessor;

impl BatchProcessor {
    /// Process multiple images in parallel
    pub fn batch_process<F>(images: Vec<DynamicImage>, processor: F) -> Vec<DynamicImage>
    where
        F: Fn(DynamicImage) -> DynamicImage + Sync + Send
    {
        images.into_par_iter()
            .map(processor)
            .collect()
    }

    /// Pipeline processing with parallelism
    pub fn pipeline_process(images: Vec<DynamicImage>) -> Vec<GrayImage> {
        images.into_par_iter()
            .map(|img| img.to_luma8())
            .map(|img| NoiseReducer::gaussian_denoise(&img, 1.0))
            .map(|img| Binarizer::otsu_binarize(&img))
            .collect()
    }
}
```

### Memory Pool

```rust
use std::collections::VecDeque;

pub struct ImagePool {
    pool: VecDeque<ImageBuffer<Rgba<u8>, Vec<u8>>>,
    max_size: usize,
}

impl ImagePool {
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: VecDeque::new(),
            max_size,
        }
    }

    /// Get buffer from pool or allocate new
    pub fn acquire(&mut self, width: u32, height: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        self.pool.pop_front()
            .unwrap_or_else(|| ImageBuffer::new(width, height))
    }

    /// Return buffer to pool
    pub fn release(&mut self, mut buffer: ImageBuffer<Rgba<u8>, Vec<u8>>) {
        if self.pool.len() < self.max_size {
            // Clear buffer
            for pixel in buffer.pixels_mut() {
                *pixel = Rgba([0, 0, 0, 0]);
            }
            self.pool.push_back(buffer);
        }
    }
}
```

## Complete Pipeline Implementation

```rust
use image::{DynamicImage, GrayImage};

pub struct PreprocessingPipeline {
    loader: ImageLoader,
    enhancer: ImageEnhancer,
    rotation_detector: RotationDetector,
    deskewer: Deskewer,
    resolution_handler: ResolutionHandler,
    binarizer: Binarizer,
    text_detector: TextDetector,
}

impl PreprocessingPipeline {
    pub fn new() -> Self {
        Self {
            loader: ImageLoader::new(),
            enhancer: ImageEnhancer::new(),
            rotation_detector: RotationDetector,
            deskewer: Deskewer,
            resolution_handler: ResolutionHandler::new(),
            binarizer: Binarizer,
            text_detector: TextDetector::new(),
        }
    }

    /// Run complete preprocessing pipeline
    pub fn process<P: AsRef<std::path::Path>>(
        &self,
        path: P
    ) -> Result<ProcessedImage, ImageError> {
        // 1. Load image
        let img = self.loader.load(path)?;

        // 2. Convert to grayscale
        let gray = img.to_luma8();

        // 3. Enhance contrast
        let enhanced = self.enhancer.apply_clahe(&gray);

        // 4. Denoise
        let denoised = NoiseReducer::bilateral_denoise(&enhanced, 3.0, 50.0);

        // 5. Auto-rotate
        let (rotated, rotation_angle) = self.rotation_detector.auto_correct_rotation(&denoised);

        // 6. Deskew
        let (deskewed, skew_angle) = self.deskewer.deskew(&rotated);

        // 7. Normalize resolution
        let normalized = self.resolution_handler.normalize_resolution(
            &DynamicImage::ImageLuma8(deskewed)
        );

        // 8. Final enhancement and binarization
        let final_gray = normalized.to_luma8();
        let sharpened = Sharpener::unsharp_mask(&final_gray, 1.0, 0.5);
        let binary = self.binarizer.otsu_binarize(&sharpened);

        // 9. Detect text regions
        let text_regions = self.text_detector.detect_text_regions(&binary);
        let merged_regions = self.text_detector.merge_bboxes(text_regions, 10);

        Ok(ProcessedImage {
            processed: binary,
            original_dimensions: img.dimensions(),
            rotation_angle,
            skew_angle,
            text_regions: merged_regions,
        })
    }

    /// Process with custom pipeline steps
    pub fn process_custom<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        config: PipelineConfig
    ) -> Result<ProcessedImage, ImageError> {
        let img = self.loader.load(path)?;
        let mut current = img.to_luma8();

        if config.enhance {
            current = self.enhancer.apply_clahe(&current);
        }

        if config.denoise {
            current = NoiseReducer::gaussian_denoise(&current, config.denoise_sigma);
        }

        if config.auto_rotate {
            let (rotated, _) = self.rotation_detector.auto_correct_rotation(&current);
            current = rotated;
        }

        if config.deskew {
            let (deskewed, _) = self.deskewer.deskew(&current);
            current = deskewed;
        }

        if config.binarize {
            current = self.binarizer.adaptive_binarize(&current, config.binarize_block_size, 0);
        }

        Ok(ProcessedImage {
            processed: current,
            original_dimensions: img.dimensions(),
            rotation_angle: 0.0,
            skew_angle: 0.0,
            text_regions: Vec::new(),
        })
    }
}

pub struct ProcessedImage {
    pub processed: GrayImage,
    pub original_dimensions: (u32, u32),
    pub rotation_angle: f32,
    pub skew_angle: f32,
    pub text_regions: Vec<imageproc::rect::Rect>,
}

pub struct PipelineConfig {
    pub enhance: bool,
    pub denoise: bool,
    pub denoise_sigma: f32,
    pub auto_rotate: bool,
    pub deskew: bool,
    pub binarize: bool,
    pub binarize_block_size: u32,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            enhance: true,
            denoise: true,
            denoise_sigma: 1.0,
            auto_rotate: true,
            deskew: true,
            binarize: true,
            binarize_block_size: 15,
        }
    }
}
```

## Performance Benchmarks

Expected performance characteristics:

- **Image Loading**: 10-50ms (depending on size and format)
- **CLAHE Enhancement**: 20-100ms (depends on tile size)
- **Bilateral Filtering**: 50-200ms (edge-preserving but slower)
- **Gaussian Blur**: 10-30ms (fast)
- **Rotation Detection**: 100-300ms (tests multiple angles)
- **Deskewing**: 50-150ms
- **Binarization**: 10-30ms
- **Text Detection**: 30-100ms
- **Full Pipeline**: 300-800ms per image

SIMD and GPU acceleration can improve these by 2-10x.

## Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
image = "0.24"
imageproc = "0.23"
rayon = "1.7"  # Parallel processing
nalgebra = "0.32"  # Linear algebra for transformations

[target.'cfg(target_feature = "avx2")'.dependencies]
# SIMD-specific dependencies

[features]
gpu = ["wgpu"]  # Optional GPU support
simd = []  # Enable SIMD optimizations
```

## Usage Example

```rust
use preprocessing_pipeline::PreprocessingPipeline;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pipeline = PreprocessingPipeline::new();

    // Process single image
    let result = pipeline.process("document.jpg")?;
    result.processed.save("processed.png")?;

    println!("Rotation: {}°", result.rotation_angle);
    println!("Skew: {}°", result.skew_angle);
    println!("Text regions: {}", result.text_regions.len());

    // Batch process
    let images = vec!["doc1.jpg", "doc2.png", "doc3.tiff"];
    let results: Vec<_> = images.into_par_iter()
        .map(|path| pipeline.process(path))
        .collect::<Result<_, _>>()?;

    Ok(())
}
```

## Future Enhancements

1. **Deep Learning Integration**: Use neural networks for super-resolution and denoising
2. **Document Layout Analysis**: Detect columns, tables, figures
3. **Adaptive Pipeline**: Automatically select preprocessing steps based on image quality
4. **Real-time Processing**: Optimize for video/camera feeds
5. **Cloud GPU Support**: Integrate with cloud GPU services for batch processing

---

This preprocessing pipeline provides a solid foundation for optimal OCR performance in the ruvector-scipix project.
