//! Image preprocessing module for OCR pipeline
//!
//! This module provides comprehensive image preprocessing capabilities including:
//! - Image transformations (grayscale, blur, sharpen, threshold)
//! - Rotation detection and correction
//! - Skew correction (deskewing)
//! - Image enhancement (CLAHE, normalization)
//! - Text region segmentation
//! - Complete preprocessing pipeline with parallel processing

pub mod deskew;
pub mod enhancement;
pub mod pipeline;
pub mod rotation;
pub mod segmentation;
pub mod transforms;

use image::{DynamicImage, GrayImage};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Preprocessing error types
#[derive(Error, Debug)]
pub enum PreprocessError {
    #[error("Image loading error: {0}")]
    ImageLoad(String),

    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    #[error("Processing error: {0}")]
    Processing(String),

    #[error("Segmentation error: {0}")]
    Segmentation(String),
}

/// Result type for preprocessing operations
pub type Result<T> = std::result::Result<T, PreprocessError>;

/// Preprocessing options for configuring the pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessOptions {
    /// Enable rotation detection and correction
    pub auto_rotate: bool,

    /// Enable skew detection and correction
    pub auto_deskew: bool,

    /// Enable contrast enhancement
    pub enhance_contrast: bool,

    /// Enable denoising
    pub denoise: bool,

    /// Binarization threshold (None for auto Otsu)
    pub threshold: Option<u8>,

    /// Enable adaptive thresholding
    pub adaptive_threshold: bool,

    /// Adaptive threshold window size
    pub adaptive_window_size: u32,

    /// Target image width (None to keep original)
    pub target_width: Option<u32>,

    /// Target image height (None to keep original)
    pub target_height: Option<u32>,

    /// Enable text region detection
    pub detect_regions: bool,

    /// Gaussian blur sigma for denoising
    pub blur_sigma: f32,

    /// CLAHE clip limit for contrast enhancement
    pub clahe_clip_limit: f32,

    /// CLAHE tile size
    pub clahe_tile_size: u32,
}

impl Default for PreprocessOptions {
    fn default() -> Self {
        Self {
            auto_rotate: true,
            auto_deskew: true,
            enhance_contrast: true,
            denoise: true,
            threshold: None,
            adaptive_threshold: true,
            adaptive_window_size: 15,
            target_width: None,
            target_height: None,
            detect_regions: true,
            blur_sigma: 1.0,
            clahe_clip_limit: 2.0,
            clahe_tile_size: 8,
        }
    }
}

/// Type of text region
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionType {
    /// Regular text
    Text,
    /// Mathematical equation
    Math,
    /// Table
    Table,
    /// Figure/Image
    Figure,
    /// Unknown/Other
    Unknown,
}

/// Detected text region with bounding box
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegion {
    /// Region type
    pub region_type: RegionType,

    /// Bounding box (x, y, width, height)
    pub bbox: (u32, u32, u32, u32),

    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,

    /// Average text height in pixels
    pub text_height: f32,

    /// Detected baseline angle in degrees
    pub baseline_angle: f32,
}

/// Main preprocessing function with configurable options
///
/// # Arguments
/// * `image` - Input image to preprocess
/// * `options` - Preprocessing configuration options
///
/// # Returns
/// Preprocessed grayscale image ready for OCR
///
/// # Example
/// ```no_run
/// use image::open;
/// use ruvector_scipix::preprocess::{preprocess, PreprocessOptions};
///
/// let img = open("document.jpg").unwrap();
/// let options = PreprocessOptions::default();
/// let processed = preprocess(&img, &options).unwrap();
/// ```
pub fn preprocess(image: &DynamicImage, options: &PreprocessOptions) -> Result<GrayImage> {
    pipeline::PreprocessPipeline::builder()
        .auto_rotate(options.auto_rotate)
        .auto_deskew(options.auto_deskew)
        .enhance_contrast(options.enhance_contrast)
        .denoise(options.denoise)
        .blur_sigma(options.blur_sigma)
        .clahe_clip_limit(options.clahe_clip_limit)
        .clahe_tile_size(options.clahe_tile_size)
        .threshold(options.threshold)
        .adaptive_threshold(options.adaptive_threshold)
        .adaptive_window_size(options.adaptive_window_size)
        .target_size(options.target_width, options.target_height)
        .build()
        .process(image)
}

/// Detect text regions in an image
///
/// # Arguments
/// * `image` - Input grayscale image
/// * `min_region_size` - Minimum region size in pixels
///
/// # Returns
/// Vector of detected text regions with metadata
///
/// # Example
/// ```no_run
/// use image::open;
/// use ruvector_scipix::preprocess::detect_text_regions;
///
/// let img = open("document.jpg").unwrap().to_luma8();
/// let regions = detect_text_regions(&img, 100).unwrap();
/// println!("Found {} text regions", regions.len());
/// ```
pub fn detect_text_regions(image: &GrayImage, min_region_size: u32) -> Result<Vec<TextRegion>> {
    segmentation::find_text_regions(image, min_region_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        let mut img = RgbImage::new(width, height);

        // Create a simple test pattern
        for y in 0..height {
            for x in 0..width {
                let val = ((x + y) % 256) as u8;
                img.put_pixel(x, y, Rgb([val, val, val]));
            }
        }

        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn test_preprocess_default_options() {
        let img = create_test_image(100, 100);
        let options = PreprocessOptions::default();

        let result = preprocess(&img, &options);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(processed.width(), 100);
        assert_eq!(processed.height(), 100);
    }

    #[test]
    fn test_preprocess_with_resize() {
        let img = create_test_image(200, 200);
        let mut options = PreprocessOptions::default();
        options.target_width = Some(100);
        options.target_height = Some(100);

        let result = preprocess(&img, &options);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(processed.width(), 100);
        assert_eq!(processed.height(), 100);
    }

    #[test]
    fn test_preprocess_options_builder() {
        let options = PreprocessOptions {
            auto_rotate: false,
            auto_deskew: false,
            enhance_contrast: true,
            denoise: true,
            threshold: Some(128),
            adaptive_threshold: false,
            ..Default::default()
        };

        assert!(!options.auto_rotate);
        assert!(!options.auto_deskew);
        assert!(options.enhance_contrast);
        assert_eq!(options.threshold, Some(128));
    }

    #[test]
    fn test_region_type_serialization() {
        let region = TextRegion {
            region_type: RegionType::Math,
            bbox: (10, 20, 100, 50),
            confidence: 0.95,
            text_height: 12.0,
            baseline_angle: 0.5,
        };

        let json = serde_json::to_string(&region).unwrap();
        let deserialized: TextRegion = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.region_type, RegionType::Math);
        assert_eq!(deserialized.bbox, (10, 20, 100, 50));
        assert!((deserialized.confidence - 0.95).abs() < 0.001);
    }
}
