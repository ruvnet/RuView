# Preprocessing Module API Reference

## Quick Start

```rust
use ruvector_scipix::preprocess::{preprocess, PreprocessOptions};
use image::open;

// Basic preprocessing with defaults
let img = open("document.jpg")?;
let options = PreprocessOptions::default();
let processed = preprocess(&img, &options)?;
```

## Core Types

### PreprocessOptions

Complete configuration struct:

```rust
pub struct PreprocessOptions {
    pub auto_rotate: bool,              // Enable rotation detection
    pub auto_deskew: bool,              // Enable skew correction
    pub enhance_contrast: bool,         // Enable CLAHE
    pub denoise: bool,                  // Enable Gaussian blur
    pub threshold: Option<u8>,          // Manual threshold (None = auto Otsu)
    pub adaptive_threshold: bool,       // Use adaptive thresholding
    pub adaptive_window_size: u32,      // Window size for adaptive (odd number)
    pub target_width: Option<u32>,      // Resize width
    pub target_height: Option<u32>,     // Resize height
    pub detect_regions: bool,           // Enable text region detection
    pub blur_sigma: f32,                // Gaussian blur sigma
    pub clahe_clip_limit: f32,          // CLAHE clip limit
    pub clahe_tile_size: u32,           // CLAHE tile size
}
```

### TextRegion

Detected text region with metadata:

```rust
pub struct TextRegion {
    pub region_type: RegionType,        // Text, Math, Table, Figure, Unknown
    pub bbox: (u32, u32, u32, u32),     // (x, y, width, height)
    pub confidence: f32,                // 0.0 to 1.0
    pub text_height: f32,               // Average text height in pixels
    pub baseline_angle: f32,            // Baseline angle in degrees
}
```

## PreprocessPipeline Builder

### Creating a Pipeline

```rust
use ruvector_scipix::preprocess::pipeline::PreprocessPipeline;

let pipeline = PreprocessPipeline::builder()
    // Rotation & Skew
    .auto_rotate(true)
    .auto_deskew(true)
    
    // Enhancement
    .enhance_contrast(true)
    .clahe_clip_limit(2.0)    // 2.0-4.0 recommended
    .clahe_tile_size(8)        // 8 or 16
    
    // Denoising
    .denoise(true)
    .blur_sigma(1.0)           // 0.5-2.0 typical
    
    // Thresholding
    .adaptive_threshold(true)
    .adaptive_window_size(15)  // Must be odd
    .threshold(None)           // None = auto Otsu
    
    // Resizing
    .target_size(Some(800), Some(600))
    
    // Progress tracking
    .progress_callback(|step, progress| {
        println!("{}... {:.0}%", step, progress * 100.0);
    })
    
    .build();
```

### Processing

```rust
// Single image
let result = pipeline.process(&image)?;

// Batch processing (parallel)
let images = vec![img1, img2, img3];
let results = pipeline.process_batch(images)?;

// With intermediates for debugging
let intermediates = pipeline.process_with_intermediates(&image)?;
for (name, img) in intermediates {
    img.save(format!("debug_{}.png", name))?;
}
```

## Module Functions

### transforms.rs

```rust
// Basic operations
pub fn to_grayscale(image: &DynamicImage) -> GrayImage;
pub fn gaussian_blur(image: &GrayImage, sigma: f32) -> Result<GrayImage>;
pub fn sharpen(image: &GrayImage, sigma: f32, amount: f32) -> Result<GrayImage>;

// Thresholding
pub fn otsu_threshold(image: &GrayImage) -> Result<u8>;
pub fn threshold(image: &GrayImage, threshold: u8) -> GrayImage;
pub fn adaptive_threshold(image: &GrayImage, window_size: u32) -> Result<GrayImage>;
```

### rotation.rs

```rust
pub fn detect_rotation(image: &GrayImage) -> Result<f32>;
pub fn rotate_image(image: &GrayImage, angle: f32) -> Result<GrayImage>;
pub fn detect_rotation_with_confidence(image: &GrayImage) -> Result<(f32, f32)>;
pub fn auto_rotate(image: &GrayImage, confidence_threshold: f32) -> Result<(GrayImage, f32, f32)>;
```

### deskew.rs

```rust
pub fn detect_skew_angle(image: &GrayImage) -> Result<f32>;
pub fn deskew_image(image: &GrayImage, angle: f32) -> Result<GrayImage>;
pub fn auto_deskew(image: &GrayImage, max_angle: f32) -> Result<(GrayImage, f32)>;
pub fn detect_skew_projection(image: &GrayImage) -> Result<f32>;
```

### enhancement.rs

```rust
pub fn clahe(image: &GrayImage, clip_limit: f32, tile_size: u32) -> Result<GrayImage>;
pub fn normalize_brightness(image: &GrayImage) -> GrayImage;
pub fn remove_shadows(image: &GrayImage) -> Result<GrayImage>;
pub fn contrast_stretch(image: &GrayImage) -> GrayImage;
```

### segmentation.rs

```rust
pub fn find_text_regions(image: &GrayImage, min_region_size: u32) -> Result<Vec<TextRegion>>;
pub fn merge_overlapping_regions(regions: Vec<(u32, u32, u32, u32)>, merge_distance: u32) -> Vec<(u32, u32, u32, u32)>;
pub fn find_text_lines(image: &GrayImage, regions: &[(u32, u32, u32, u32)]) -> Vec<Vec<(u32, u32, u32, u32)>>;
```

## Common Workflows

### Document Scanning

```rust
let pipeline = PreprocessPipeline::builder()
    .auto_rotate(true)
    .auto_deskew(true)
    .enhance_contrast(true)
    .remove_shadows(true)  // Note: not in builder, manual call
    .adaptive_threshold(true)
    .build();
```

### Low-Quality Images

```rust
let pipeline = PreprocessPipeline::builder()
    .denoise(true)
    .blur_sigma(1.5)        // Higher blur for noise
    .enhance_contrast(true)
    .clahe_clip_limit(3.0)  // Higher clip for more contrast
    .adaptive_threshold(true)
    .adaptive_window_size(21) // Larger window
    .build();
```

### Fast Processing

```rust
let pipeline = PreprocessPipeline::builder()
    .auto_rotate(false)     // Skip if not needed
    .auto_deskew(false)
    .enhance_contrast(false)
    .denoise(false)
    .threshold(Some(128))   // Fixed threshold
    .build();
```

### High Quality

```rust
let pipeline = PreprocessPipeline::builder()
    .auto_rotate(true)
    .auto_deskew(true)
    .enhance_contrast(true)
    .clahe_clip_limit(2.0)
    .clahe_tile_size(16)    // Larger tiles
    .denoise(true)
    .blur_sigma(0.8)        // Gentle blur
    .adaptive_threshold(true)
    .adaptive_window_size(11)
    .build();
```

## Error Handling

```rust
use ruvector_scipix::preprocess::PreprocessError;

match preprocess(&img, &options) {
    Ok(processed) => { /* success */ },
    Err(PreprocessError::ImageLoad(msg)) => { /* handle load error */ },
    Err(PreprocessError::InvalidParameters(msg)) => { /* handle invalid params */ },
    Err(PreprocessError::Processing(msg)) => { /* handle processing error */ },
    Err(PreprocessError::Segmentation(msg)) => { /* handle segmentation error */ },
}
```

## Performance Tips

1. **Batch Processing**: Use `process_batch()` for multiple images
2. **Disable Unused Steps**: Turn off rotation/deskew if not needed
3. **Fixed Threshold**: Use manual threshold instead of Otsu for speed
4. **Smaller Tiles**: Use 8x8 CLAHE tiles for speed, 16x16 for quality
5. **Target Size**: Resize before processing to reduce computation

## Parameter Tuning

### blur_sigma
- **0.5-1.0**: Minimal noise reduction
- **1.0-1.5**: Moderate (recommended)
- **1.5-2.5**: Heavy denoising

### clahe_clip_limit
- **1.5-2.0**: Subtle enhancement
- **2.0-3.0**: Moderate (recommended)
- **3.0-4.0**: Strong enhancement

### clahe_tile_size
- **4**: Very local, may cause artifacts
- **8**: Good balance (recommended)
- **16**: Smoother, less local

### adaptive_window_size
- **7-11**: Small features, faster
- **13-17**: Medium (recommended)
- **19-25**: Large features, slower

## Examples

See `/home/user/ruvector/examples/scipix/examples/` for complete working examples.
