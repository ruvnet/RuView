//! Complete preprocessing pipeline with builder pattern and parallel processing

use super::Result;
use crate::preprocess::{deskew, enhancement, rotation, transforms};
use image::{DynamicImage, GrayImage};
use rayon::prelude::*;
use std::sync::Arc;

/// Progress callback type
pub type ProgressCallback = Arc<dyn Fn(&str, f32) + Send + Sync>;

/// Complete preprocessing pipeline with configurable steps
pub struct PreprocessPipeline {
    auto_rotate: bool,
    auto_deskew: bool,
    enhance_contrast: bool,
    denoise: bool,
    blur_sigma: f32,
    clahe_clip_limit: f32,
    clahe_tile_size: u32,
    threshold: Option<u8>,
    adaptive_threshold: bool,
    adaptive_window_size: u32,
    target_width: Option<u32>,
    target_height: Option<u32>,
    progress_callback: Option<ProgressCallback>,
}

/// Builder for preprocessing pipeline
pub struct PreprocessPipelineBuilder {
    auto_rotate: bool,
    auto_deskew: bool,
    enhance_contrast: bool,
    denoise: bool,
    blur_sigma: f32,
    clahe_clip_limit: f32,
    clahe_tile_size: u32,
    threshold: Option<u8>,
    adaptive_threshold: bool,
    adaptive_window_size: u32,
    target_width: Option<u32>,
    target_height: Option<u32>,
    progress_callback: Option<ProgressCallback>,
}

impl Default for PreprocessPipelineBuilder {
    fn default() -> Self {
        Self {
            auto_rotate: true,
            auto_deskew: true,
            enhance_contrast: true,
            denoise: true,
            blur_sigma: 1.0,
            clahe_clip_limit: 2.0,
            clahe_tile_size: 8,
            threshold: None,
            adaptive_threshold: true,
            adaptive_window_size: 15,
            target_width: None,
            target_height: None,
            progress_callback: None,
        }
    }
}

impl PreprocessPipelineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn auto_rotate(mut self, enable: bool) -> Self {
        self.auto_rotate = enable;
        self
    }

    pub fn auto_deskew(mut self, enable: bool) -> Self {
        self.auto_deskew = enable;
        self
    }

    pub fn enhance_contrast(mut self, enable: bool) -> Self {
        self.enhance_contrast = enable;
        self
    }

    pub fn denoise(mut self, enable: bool) -> Self {
        self.denoise = enable;
        self
    }

    pub fn blur_sigma(mut self, sigma: f32) -> Self {
        self.blur_sigma = sigma;
        self
    }

    pub fn clahe_clip_limit(mut self, limit: f32) -> Self {
        self.clahe_clip_limit = limit;
        self
    }

    pub fn clahe_tile_size(mut self, size: u32) -> Self {
        self.clahe_tile_size = size;
        self
    }

    pub fn threshold(mut self, threshold: Option<u8>) -> Self {
        self.threshold = threshold;
        self
    }

    pub fn adaptive_threshold(mut self, enable: bool) -> Self {
        self.adaptive_threshold = enable;
        self
    }

    pub fn adaptive_window_size(mut self, size: u32) -> Self {
        self.adaptive_window_size = size;
        self
    }

    pub fn target_size(mut self, width: Option<u32>, height: Option<u32>) -> Self {
        self.target_width = width;
        self.target_height = height;
        self
    }

    pub fn progress_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, f32) + Send + Sync + 'static,
    {
        self.progress_callback = Some(Arc::new(callback));
        self
    }

    pub fn build(self) -> PreprocessPipeline {
        PreprocessPipeline {
            auto_rotate: self.auto_rotate,
            auto_deskew: self.auto_deskew,
            enhance_contrast: self.enhance_contrast,
            denoise: self.denoise,
            blur_sigma: self.blur_sigma,
            clahe_clip_limit: self.clahe_clip_limit,
            clahe_tile_size: self.clahe_tile_size,
            threshold: self.threshold,
            adaptive_threshold: self.adaptive_threshold,
            adaptive_window_size: self.adaptive_window_size,
            target_width: self.target_width,
            target_height: self.target_height,
            progress_callback: self.progress_callback,
        }
    }
}

impl PreprocessPipeline {
    /// Create a new pipeline builder
    pub fn builder() -> PreprocessPipelineBuilder {
        PreprocessPipelineBuilder::new()
    }

    /// Report progress if callback is set
    fn report_progress(&self, step: &str, progress: f32) {
        if let Some(callback) = &self.progress_callback {
            callback(step, progress);
        }
    }

    /// Process a single image through the complete pipeline
    ///
    /// # Pipeline steps:
    /// 1. Convert to grayscale
    /// 2. Detect and correct rotation (if enabled)
    /// 3. Detect and correct skew (if enabled)
    /// 4. Enhance contrast with CLAHE (if enabled)
    /// 5. Denoise with Gaussian blur (if enabled)
    /// 6. Apply thresholding (binary or adaptive)
    /// 7. Resize to target dimensions (if specified)
    pub fn process(&self, image: &DynamicImage) -> Result<GrayImage> {
        self.report_progress("Starting preprocessing", 0.0);

        // Step 1: Convert to grayscale
        self.report_progress("Converting to grayscale", 0.1);
        let mut gray = transforms::to_grayscale(image);

        // Step 2: Auto-rotate
        if self.auto_rotate {
            self.report_progress("Detecting rotation", 0.2);
            let angle = rotation::detect_rotation(&gray)?;

            if angle.abs() > 0.5 {
                self.report_progress("Correcting rotation", 0.25);
                gray = rotation::rotate_image(&gray, -angle)?;
            }
        }

        // Step 3: Auto-deskew
        if self.auto_deskew {
            self.report_progress("Detecting skew", 0.3);
            let angle = deskew::detect_skew_angle(&gray)?;

            if angle.abs() > 0.5 {
                self.report_progress("Correcting skew", 0.35);
                gray = deskew::deskew_image(&gray, angle)?;
            }
        }

        // Step 4: Enhance contrast
        if self.enhance_contrast {
            self.report_progress("Enhancing contrast", 0.5);
            gray = enhancement::clahe(&gray, self.clahe_clip_limit, self.clahe_tile_size)?;
        }

        // Step 5: Denoise
        if self.denoise {
            self.report_progress("Denoising", 0.6);
            gray = transforms::gaussian_blur(&gray, self.blur_sigma)?;
        }

        // Step 6: Thresholding
        self.report_progress("Applying threshold", 0.7);
        gray = if self.adaptive_threshold {
            transforms::adaptive_threshold(&gray, self.adaptive_window_size)?
        } else if let Some(threshold_val) = self.threshold {
            transforms::threshold(&gray, threshold_val)
        } else {
            // Auto Otsu threshold
            let threshold_val = transforms::otsu_threshold(&gray)?;
            transforms::threshold(&gray, threshold_val)
        };

        // Step 7: Resize
        if let (Some(width), Some(height)) = (self.target_width, self.target_height) {
            self.report_progress("Resizing", 0.9);
            gray = image::imageops::resize(
                &gray,
                width,
                height,
                image::imageops::FilterType::Lanczos3,
            );
        }

        self.report_progress("Preprocessing complete", 1.0);
        Ok(gray)
    }

    /// Process multiple images in parallel
    ///
    /// # Arguments
    /// * `images` - Vector of images to process
    ///
    /// # Returns
    /// Vector of preprocessed images in the same order
    pub fn process_batch(&self, images: Vec<DynamicImage>) -> Result<Vec<GrayImage>> {
        images
            .into_par_iter()
            .map(|img| self.process(&img))
            .collect()
    }

    /// Process image and return intermediate results from each step
    ///
    /// Useful for debugging and visualization
    pub fn process_with_intermediates(
        &self,
        image: &DynamicImage,
    ) -> Result<Vec<(String, GrayImage)>> {
        let mut results = Vec::new();

        // Step 1: Grayscale
        let mut gray = transforms::to_grayscale(image);
        results.push(("01_grayscale".to_string(), gray.clone()));

        // Step 2: Rotation
        if self.auto_rotate {
            let angle = rotation::detect_rotation(&gray)?;
            if angle.abs() > 0.5 {
                gray = rotation::rotate_image(&gray, -angle)?;
                results.push(("02_rotated".to_string(), gray.clone()));
            }
        }

        // Step 3: Deskew
        if self.auto_deskew {
            let angle = deskew::detect_skew_angle(&gray)?;
            if angle.abs() > 0.5 {
                gray = deskew::deskew_image(&gray, angle)?;
                results.push(("03_deskewed".to_string(), gray.clone()));
            }
        }

        // Step 4: Enhancement
        if self.enhance_contrast {
            gray = enhancement::clahe(&gray, self.clahe_clip_limit, self.clahe_tile_size)?;
            results.push(("04_enhanced".to_string(), gray.clone()));
        }

        // Step 5: Denoise
        if self.denoise {
            gray = transforms::gaussian_blur(&gray, self.blur_sigma)?;
            results.push(("05_denoised".to_string(), gray.clone()));
        }

        // Step 6: Threshold
        gray = if self.adaptive_threshold {
            transforms::adaptive_threshold(&gray, self.adaptive_window_size)?
        } else if let Some(threshold_val) = self.threshold {
            transforms::threshold(&gray, threshold_val)
        } else {
            let threshold_val = transforms::otsu_threshold(&gray)?;
            transforms::threshold(&gray, threshold_val)
        };
        results.push(("06_thresholded".to_string(), gray.clone()));

        // Step 7: Resize
        if let (Some(width), Some(height)) = (self.target_width, self.target_height) {
            gray = image::imageops::resize(
                &gray,
                width,
                height,
                image::imageops::FilterType::Lanczos3,
            );
            results.push(("07_resized".to_string(), gray.clone()));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn create_test_image() -> DynamicImage {
        let mut img = RgbImage::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                let val = ((x + y) / 2) as u8;
                img.put_pixel(x, y, Rgb([val, val, val]));
            }
        }
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn test_pipeline_builder() {
        let pipeline = PreprocessPipeline::builder()
            .auto_rotate(false)
            .denoise(true)
            .blur_sigma(1.5)
            .build();

        assert!(!pipeline.auto_rotate);
        assert!(pipeline.denoise);
        assert!((pipeline.blur_sigma - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_pipeline_process() {
        let img = create_test_image();
        let pipeline = PreprocessPipeline::builder()
            .auto_rotate(false)
            .auto_deskew(false)
            .build();

        let result = pipeline.process(&img);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(processed.width(), 100);
        assert_eq!(processed.height(), 100);
    }

    #[test]
    fn test_pipeline_with_resize() {
        let img = create_test_image();
        let pipeline = PreprocessPipeline::builder()
            .target_size(Some(50), Some(50))
            .auto_rotate(false)
            .auto_deskew(false)
            .build();

        let result = pipeline.process(&img);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(processed.width(), 50);
        assert_eq!(processed.height(), 50);
    }

    #[test]
    fn test_pipeline_batch_processing() {
        let images = vec![
            create_test_image(),
            create_test_image(),
            create_test_image(),
        ];

        let pipeline = PreprocessPipeline::builder()
            .auto_rotate(false)
            .auto_deskew(false)
            .build();

        let results = pipeline.process_batch(images);
        assert!(results.is_ok());

        let processed = results.unwrap();
        assert_eq!(processed.len(), 3);
    }

    #[test]
    fn test_pipeline_intermediates() {
        let img = create_test_image();
        let pipeline = PreprocessPipeline::builder()
            .auto_rotate(false)
            .auto_deskew(false)
            .enhance_contrast(true)
            .denoise(true)
            .build();

        let result = pipeline.process_with_intermediates(&img);
        assert!(result.is_ok());

        let intermediates = result.unwrap();
        assert!(!intermediates.is_empty());
        assert!(intermediates
            .iter()
            .any(|(name, _)| name.contains("grayscale")));
        assert!(intermediates
            .iter()
            .any(|(name, _)| name.contains("thresholded")));
    }

    #[test]
    fn test_progress_callback() {
        use std::sync::{Arc, Mutex};

        let progress_steps = Arc::new(Mutex::new(Vec::new()));
        let progress_clone = Arc::clone(&progress_steps);

        let pipeline = PreprocessPipeline::builder()
            .auto_rotate(false)
            .auto_deskew(false)
            .progress_callback(move |step, _progress| {
                progress_clone.lock().unwrap().push(step.to_string());
            })
            .build();

        let img = create_test_image();
        let _ = pipeline.process(&img);

        let steps = progress_steps.lock().unwrap();
        assert!(!steps.is_empty());
        assert!(steps.iter().any(|s| s.contains("Starting")));
        assert!(steps.iter().any(|s| s.contains("complete")));
    }
}
