//! Custom pipeline example
//!
//! This example demonstrates how to create a custom OCR pipeline with:
//! - Custom preprocessing steps
//! - Post-processing filters
//! - Integration with external services
//! - Custom output formatting
//!
//! Usage:
//! ```bash
//! cargo run --example custom_pipeline -- image.png
//! ```

use anyhow::{Context, Result};
use image::{DynamicImage, ImageBuffer, Luma};
use ruvector_scipix::{OcrConfig, OcrEngine, OcrResult, OutputFormat};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
struct CustomPipeline {
    engine: OcrEngine,
    preprocessing: Vec<PreprocessStep>,
    postprocessing: Vec<PostprocessStep>,
}

#[derive(Debug, Clone)]
enum PreprocessStep {
    Denoise,
    Sharpen,
    ContrastEnhancement,
    Binarization,
    Deskew,
}

#[derive(Debug, Clone)]
enum PostprocessStep {
    SpellCheck,
    LatexValidation,
    ConfidenceFilter(f32),
    CustomFormatter,
}

#[derive(Debug, Serialize, Deserialize)]
struct PipelineResult {
    original_result: String,
    processed_result: String,
    latex: String,
    confidence: f32,
    preprocessing_steps: Vec<String>,
    postprocessing_steps: Vec<String>,
    validation_results: ValidationResults,
}

#[derive(Debug, Serialize, Deserialize)]
struct ValidationResults {
    latex_valid: bool,
    spell_check_corrections: usize,
    confidence_threshold_passed: bool,
}

impl CustomPipeline {
    async fn new(config: OcrConfig) -> Result<Self> {
        let engine = OcrEngine::new(config).await?;

        Ok(Self {
            engine,
            preprocessing: vec![
                PreprocessStep::Denoise,
                PreprocessStep::ContrastEnhancement,
                PreprocessStep::Sharpen,
                PreprocessStep::Binarization,
            ],
            postprocessing: vec![
                PostprocessStep::ConfidenceFilter(0.7),
                PostprocessStep::LatexValidation,
                PostprocessStep::SpellCheck,
                PostprocessStep::CustomFormatter,
            ],
        })
    }

    async fn process(&self, image: DynamicImage) -> Result<PipelineResult> {
        // Apply preprocessing steps
        let mut processed_image = image;
        let mut preprocessing_log = Vec::new();

        for step in &self.preprocessing {
            processed_image = self.apply_preprocessing(processed_image, step)?;
            preprocessing_log.push(format!("{:?}", step));
        }

        // Run OCR
        let ocr_result = self.engine.recognize(&processed_image).await?;

        // Apply postprocessing steps
        let mut result_text = ocr_result.text.clone();
        let mut postprocessing_log = Vec::new();
        let mut validation = ValidationResults {
            latex_valid: false,
            spell_check_corrections: 0,
            confidence_threshold_passed: false,
        };

        for step in &self.postprocessing {
            let (new_text, step_validation) =
                self.apply_postprocessing(result_text.clone(), &ocr_result, step)?;
            result_text = new_text;
            postprocessing_log.push(format!("{:?}", step));

            // Update validation results
            match step {
                PostprocessStep::LatexValidation => {
                    validation.latex_valid = step_validation.unwrap_or(false);
                }
                PostprocessStep::SpellCheck => {
                    validation.spell_check_corrections = step_validation.unwrap_or(0) as usize;
                }
                PostprocessStep::ConfidenceFilter(threshold) => {
                    validation.confidence_threshold_passed = ocr_result.confidence >= *threshold;
                }
                _ => {}
            }
        }

        Ok(PipelineResult {
            original_result: ocr_result.text.clone(),
            processed_result: result_text,
            latex: ocr_result.to_format(OutputFormat::LaTeX)?,
            confidence: ocr_result.confidence,
            preprocessing_steps: preprocessing_log,
            postprocessing_steps: postprocessing_log,
            validation_results: validation,
        })
    }

    fn apply_preprocessing(
        &self,
        image: DynamicImage,
        step: &PreprocessStep,
    ) -> Result<DynamicImage> {
        match step {
            PreprocessStep::Denoise => Ok(denoise_image(image)),
            PreprocessStep::Sharpen => Ok(sharpen_image(image)),
            PreprocessStep::ContrastEnhancement => Ok(enhance_contrast(image)),
            PreprocessStep::Binarization => Ok(binarize_image(image)),
            PreprocessStep::Deskew => Ok(deskew_image(image)),
        }
    }

    fn apply_postprocessing(
        &self,
        text: String,
        result: &OcrResult,
        step: &PostprocessStep,
    ) -> Result<(String, Option<i32>)> {
        match step {
            PostprocessStep::SpellCheck => {
                let (corrected, corrections) = spell_check(&text);
                Ok((corrected, Some(corrections as i32)))
            }
            PostprocessStep::LatexValidation => {
                let valid = validate_latex(&text);
                Ok((text, Some(if valid { 1 } else { 0 })))
            }
            PostprocessStep::ConfidenceFilter(threshold) => {
                if result.confidence >= *threshold {
                    Ok((text, Some(1)))
                } else {
                    Ok((format!("[Low Confidence] {}", text), Some(0)))
                }
            }
            PostprocessStep::CustomFormatter => {
                let formatted = custom_format(&text);
                Ok((formatted, None))
            }
        }
    }
}

// Preprocessing implementations
fn denoise_image(image: DynamicImage) -> DynamicImage {
    // Simplified denoising using median filter
    image.blur(1.0)
}

fn sharpen_image(image: DynamicImage) -> DynamicImage {
    // Simplified sharpening
    image.unsharpen(2.0, 1)
}

fn enhance_contrast(image: DynamicImage) -> DynamicImage {
    // Simplified contrast enhancement
    image.adjust_contrast(20.0)
}

fn binarize_image(image: DynamicImage) -> DynamicImage {
    // Otsu's binarization (simplified)
    let gray = image.to_luma8();
    let threshold = calculate_otsu_threshold(&gray);

    let binary = ImageBuffer::from_fn(gray.width(), gray.height(), |x, y| {
        let pixel = gray.get_pixel(x, y)[0];
        if pixel > threshold {
            Luma([255u8])
        } else {
            Luma([0u8])
        }
    });

    DynamicImage::ImageLuma8(binary)
}

fn deskew_image(image: DynamicImage) -> DynamicImage {
    // Simplified deskew - in production, use Hough transform
    image
}

fn calculate_otsu_threshold(gray: &ImageBuffer<Luma<u8>, Vec<u8>>) -> u8 {
    // Simplified Otsu's method
    let mut histogram = [0u32; 256];

    for pixel in gray.pixels() {
        histogram[pixel[0] as usize] += 1;
    }

    let total = gray.width() * gray.height();
    let mut sum = 0u64;
    for (i, &count) in histogram.iter().enumerate() {
        sum += i as u64 * count as u64;
    }

    let mut sum_background = 0u64;
    let mut weight_background = 0u32;
    let mut max_variance = 0.0f64;
    let mut threshold = 0u8;

    for (t, &count) in histogram.iter().enumerate() {
        weight_background += count;
        if weight_background == 0 {
            continue;
        }

        let weight_foreground = total - weight_background;
        if weight_foreground == 0 {
            break;
        }

        sum_background += t as u64 * count as u64;

        let mean_background = sum_background as f64 / weight_background as f64;
        let mean_foreground = (sum - sum_background) as f64 / weight_foreground as f64;

        let variance = weight_background as f64
            * weight_foreground as f64
            * (mean_background - mean_foreground).powi(2);

        if variance > max_variance {
            max_variance = variance;
            threshold = t as u8;
        }
    }

    threshold
}

// Postprocessing implementations
fn spell_check(text: &str) -> (String, usize) {
    // Simplified spell check - in production, use a proper library
    // For demo, just return the original text
    (text.to_string(), 0)
}

fn validate_latex(text: &str) -> bool {
    // Simplified LaTeX validation
    // Check for balanced braces and common LaTeX patterns
    let open_braces = text.matches('{').count();
    let close_braces = text.matches('}').count();

    open_braces == close_braces
}

fn custom_format(text: &str) -> String {
    // Custom formatting - e.g., add proper spacing, formatting
    text.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <image_path>", args[0]);
        std::process::exit(1);
    }

    let image_path = &args[1];

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("Loading image: {}", image_path);
    let image = image::open(image_path)?;

    // Create custom pipeline
    let config = OcrConfig::default();
    let pipeline = CustomPipeline::new(config).await?;

    println!("Processing with custom pipeline...");
    let result = pipeline.process(image).await?;

    // Display results
    println!("\n{}", "=".repeat(80));
    println!("Pipeline Results");
    println!("{}", "=".repeat(80));

    println!("\n📝 Original OCR Result:");
    println!("{}", result.original_result);

    println!("\n✨ Processed Result:");
    println!("{}", result.processed_result);

    println!("\n🔢 LaTeX:");
    println!("{}", result.latex);

    println!("\n📊 Confidence: {:.2}%", result.confidence * 100.0);

    println!("\n🔧 Preprocessing Steps:");
    for step in &result.preprocessing_steps {
        println!("  - {}", step);
    }

    println!("\n🔄 Postprocessing Steps:");
    for step in &result.postprocessing_steps {
        println!("  - {}", step);
    }

    println!("\n✅ Validation:");
    println!("  LaTeX Valid: {}", result.validation_results.latex_valid);
    println!(
        "  Spell Corrections: {}",
        result.validation_results.spell_check_corrections
    );
    println!(
        "  Confidence Passed: {}",
        result.validation_results.confidence_threshold_passed
    );

    println!("\n{}", "=".repeat(80));

    // Save full results
    let json = serde_json::to_string_pretty(&result)?;
    std::fs::write("pipeline_results.json", json)?;
    println!("\nFull results saved to: pipeline_results.json");

    Ok(())
}
