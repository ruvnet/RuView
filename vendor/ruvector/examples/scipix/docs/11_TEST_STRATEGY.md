# Comprehensive Testing Strategy for Ruvector-Scipix OCR System

## Overview

This document defines a comprehensive testing strategy for the ruvector-scipix OCR system, covering unit testing, integration testing, accuracy validation, performance benchmarking, regression testing, fuzz testing, and CI/CD integration.

## Table of Contents

1. [Unit Testing](#1-unit-testing)
2. [Integration Testing](#2-integration-testing)
3. [Accuracy Testing](#3-accuracy-testing)
4. [Performance Testing](#4-performance-testing)
5. [Regression Testing](#5-regression-testing)
6. [Fuzz Testing](#6-fuzz-testing)
7. [Test Data Management](#7-test-data-management)
8. [CI/CD Integration](#8-cicd-integration)
9. [Property-Based Testing](#9-property-based-testing)
10. [Test Coverage Requirements](#10-test-coverage-requirements)

---

## 1. Unit Testing

Unit tests verify individual components in isolation, ensuring each function works correctly.

### 1.1 Image Preprocessing Functions

Test image loading, enhancement, and geometric corrections.

```rust
// tests/unit/preprocessing_tests.rs
use ruvector_scipix::preprocessing::{
    ImageLoader, ImageEnhancer, Binarizer, RotationDetector, Deskewer
};
use image::{GrayImage, ImageBuffer, Luma};

#[test]
fn test_image_loader_valid_formats() {
    let loader = ImageLoader::new();

    // Test loading different formats
    let formats = vec!["png", "jpg", "tiff", "webp"];
    for format in formats {
        let path = format!("testdata/sample.{}", format);
        let result = loader.load(&path);
        assert!(result.is_ok(), "Failed to load {} format", format);
    }
}

#[test]
fn test_image_loader_dimension_limits() {
    let loader = ImageLoader::new();

    // Test dimension validation
    let oversized_path = "testdata/oversized_16384x16384.png";
    let result = loader.load(oversized_path);
    assert!(result.is_err(), "Should reject oversized images");
}

#[test]
fn test_image_loader_invalid_file() {
    let loader = ImageLoader::new();

    let result = loader.load("testdata/nonexistent.png");
    assert!(result.is_err(), "Should fail on nonexistent file");

    let result = loader.load("testdata/corrupted.png");
    assert!(result.is_err(), "Should fail on corrupted file");
}

#[test]
fn test_clahe_enhancement() {
    let enhancer = ImageEnhancer::new();

    // Create test image with varying contrast
    let img = create_low_contrast_image(256, 256);
    let enhanced = enhancer.apply_clahe(&img);

    // Verify contrast improvement
    let original_contrast = calculate_contrast(&img);
    let enhanced_contrast = calculate_contrast(&enhanced);

    assert!(enhanced_contrast > original_contrast,
        "CLAHE should increase contrast");
}

#[test]
fn test_otsu_binarization() {
    let binarizer = Binarizer;

    // Create grayscale image with known threshold
    let img = create_bimodal_image(256, 256);
    let binary = binarizer.otsu_binarize(&img);

    // Verify output is binary (only 0 and 255)
    for pixel in binary.pixels() {
        assert!(pixel[0] == 0 || pixel[0] == 255,
            "Binary image should only have 0 or 255 values");
    }
}

#[test]
fn test_rotation_detection_accuracy() {
    let detector = RotationDetector;

    // Test known rotation angles
    let test_angles = vec![0.0, 15.0, 30.0, 45.0, 90.0, 180.0];

    for angle in test_angles {
        let img = load_test_image("testdata/text_sample.png");
        let rotated = detector.rotate_image(&img, angle);
        let detected_angle = detector.detect_rotation_angle(&rotated);

        // Allow 2-degree tolerance
        assert!(
            (detected_angle - angle).abs() < 2.0,
            "Detected angle {} should be close to {}",
            detected_angle, angle
        );
    }
}

#[test]
fn test_deskewing() {
    let deskewer = Deskewer;

    // Create skewed image
    let img = load_test_image("testdata/skewed_text.png");
    let (deskewed, angle) = deskewer.deskew(&img);

    // Verify skew angle is reasonable
    assert!(angle.abs() < 45.0, "Skew angle should be within ±45°");

    // Verify deskewed image dimensions are valid
    assert!(deskewed.width() > 0 && deskewed.height() > 0);
}

// Helper functions
fn create_low_contrast_image(width: u32, height: u32) -> GrayImage {
    ImageBuffer::from_fn(width, height, |x, y| {
        let val = (x % 50 + 100) as u8;
        Luma([val])
    })
}

fn create_bimodal_image(width: u32, height: u32) -> GrayImage {
    ImageBuffer::from_fn(width, height, |x, y| {
        let val = if (x + y) % 2 == 0 { 50 } else { 200 };
        Luma([val])
    })
}

fn calculate_contrast(img: &GrayImage) -> f64 {
    let pixels: Vec<u8> = img.pixels().map(|p| p[0]).collect();
    let mean = pixels.iter().map(|&x| x as f64).sum::<f64>() / pixels.len() as f64;
    let variance = pixels.iter()
        .map(|&x| (x as f64 - mean).powi(2))
        .sum::<f64>() / pixels.len() as f64;
    variance.sqrt()
}

fn load_test_image(path: &str) -> GrayImage {
    image::open(path).unwrap().to_luma8()
}
```

### 1.2 LaTeX Token Parsing

Test LaTeX parsing and tokenization logic.

```rust
// tests/unit/latex_parser_tests.rs
use ruvector_scipix::latex::{LatexParser, LatexToken, TokenType};

#[test]
fn test_simple_expression_parsing() {
    let parser = LatexParser::new();

    let input = "x^2 + 2x + 1";
    let tokens = parser.parse(input).unwrap();

    assert_eq!(tokens.len(), 7);
    assert_eq!(tokens[0].token_type, TokenType::Variable);
    assert_eq!(tokens[0].value, "x");
    assert_eq!(tokens[1].token_type, TokenType::Superscript);
    assert_eq!(tokens[2].value, "2");
}

#[test]
fn test_fraction_parsing() {
    let parser = LatexParser::new();

    let input = r"\frac{1}{2}";
    let tokens = parser.parse(input).unwrap();

    // Verify fraction structure
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Fraction));
    assert_eq!(tokens.iter().filter(|t| t.value == "1").count(), 1);
    assert_eq!(tokens.iter().filter(|t| t.value == "2").count(), 1);
}

#[test]
fn test_matrix_parsing() {
    let parser = LatexParser::new();

    let input = r"\begin{bmatrix} 1 & 2 \\ 3 & 4 \end{bmatrix}";
    let tokens = parser.parse(input).unwrap();

    assert!(tokens.iter().any(|t| t.token_type == TokenType::MatrixBegin));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::MatrixEnd));

    // Verify matrix elements
    let numbers: Vec<&str> = tokens.iter()
        .filter(|t| t.token_type == TokenType::Number)
        .map(|t| t.value.as_str())
        .collect();
    assert_eq!(numbers, vec!["1", "2", "3", "4"]);
}

#[test]
fn test_nested_expressions() {
    let parser = LatexParser::new();

    let input = r"\frac{x^2 + 1}{x - 1}";
    let tokens = parser.parse(input).unwrap();

    // Verify nested structure is preserved
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Fraction));
    assert!(tokens.iter().any(|t| t.token_type == TokenType::Superscript));
}

#[test]
fn test_special_symbols() {
    let parser = LatexParser::new();

    let symbols = vec![
        (r"\alpha", TokenType::GreekLetter),
        (r"\sum", TokenType::Operator),
        (r"\int", TokenType::Operator),
        (r"\infty", TokenType::Symbol),
        (r"\pi", TokenType::Constant),
    ];

    for (input, expected_type) in symbols {
        let tokens = parser.parse(input).unwrap();
        assert_eq!(tokens[0].token_type, expected_type,
            "Failed for symbol: {}", input);
    }
}

#[test]
fn test_invalid_latex() {
    let parser = LatexParser::new();

    let invalid_inputs = vec![
        r"\frac{1}",           // Missing denominator
        r"\begin{bmatrix}",    // Unclosed environment
        r"x^",                 // Incomplete superscript
        r"\unknown{command}",  // Unknown command
    ];

    for input in invalid_inputs {
        let result = parser.parse(input);
        assert!(result.is_err(),
            "Should fail for invalid input: {}", input);
    }
}
```

### 1.3 Output Format Conversion

Test conversion between different output formats.

```rust
// tests/unit/format_conversion_tests.rs
use ruvector_scipix::formats::{FormatConverter, OutputFormat};

#[test]
fn test_latex_to_mathml() {
    let converter = FormatConverter::new();

    let latex = r"\frac{1}{2}";
    let mathml = converter.convert(latex, OutputFormat::MathML).unwrap();

    assert!(mathml.contains("<mfrac>"));
    assert!(mathml.contains("<mn>1</mn>"));
    assert!(mathml.contains("<mn>2</mn>"));
}

#[test]
fn test_latex_to_ascii() {
    let converter = FormatConverter::new();

    let latex = r"x^2 + 1";
    let ascii = converter.convert(latex, OutputFormat::AsciiMath).unwrap();

    assert_eq!(ascii, "x^2 + 1");
}

#[test]
fn test_latex_to_unicode() {
    let converter = FormatConverter::new();

    let latex = r"\alpha + \beta";
    let unicode = converter.convert(latex, OutputFormat::Unicode).unwrap();

    assert!(unicode.contains("α"));
    assert!(unicode.contains("β"));
}

#[test]
fn test_latex_to_text() {
    let converter = FormatConverter::new();

    let latex = r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}";
    let text = converter.convert(latex, OutputFormat::PlainText).unwrap();

    // Verify reasonable text representation
    assert!(text.contains("sum"));
    assert!(text.contains("i=1"));
}

#[test]
fn test_format_roundtrip() {
    let converter = FormatConverter::new();

    let original = r"x^2 + 2x + 1";

    // Convert to MathML and back
    let mathml = converter.convert(original, OutputFormat::MathML).unwrap();
    let back_to_latex = converter.convert(&mathml, OutputFormat::LaTeX).unwrap();

    // Should be semantically equivalent (may differ in whitespace)
    assert_eq!(
        normalize_latex(&back_to_latex),
        normalize_latex(original)
    );
}

fn normalize_latex(latex: &str) -> String {
    latex.chars().filter(|c| !c.is_whitespace()).collect()
}
```

### 1.4 Configuration Handling

Test configuration loading and validation.

```rust
// tests/unit/config_tests.rs
use ruvector_scipix::config::{OCRConfig, ModelConfig, PreprocessingConfig};
use std::path::Path;

#[test]
fn test_default_config() {
    let config = OCRConfig::default();

    assert_eq!(config.model.device, "cpu");
    assert!(config.preprocessing.enable_enhancement);
    assert_eq!(config.output.format, "latex");
}

#[test]
fn test_load_config_from_file() {
    let config = OCRConfig::from_file("testdata/config.toml").unwrap();

    assert!(config.model.model_path.exists());
    assert!(config.preprocessing.target_resolution > 0);
}

#[test]
fn test_config_validation() {
    let mut config = OCRConfig::default();

    // Valid config should pass
    assert!(config.validate().is_ok());

    // Invalid model path should fail
    config.model.model_path = Path::new("/nonexistent/model.onnx").to_path_buf();
    assert!(config.validate().is_err());
}

#[test]
fn test_config_merge() {
    let default = OCRConfig::default();
    let mut custom = OCRConfig::default();
    custom.model.device = "cuda".to_string();
    custom.output.format = "mathml".to_string();

    let merged = default.merge(custom);

    assert_eq!(merged.model.device, "cuda");
    assert_eq!(merged.output.format, "mathml");
    // Default values should be preserved
    assert!(merged.preprocessing.enable_enhancement);
}

#[test]
fn test_config_serialization() {
    let config = OCRConfig::default();

    // Serialize to JSON
    let json = serde_json::to_string(&config).unwrap();

    // Deserialize back
    let deserialized: OCRConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.model.device, deserialized.model.device);
    assert_eq!(config.output.format, deserialized.output.format);
}
```

---

## 2. Integration Testing

Integration tests verify the complete pipeline and API endpoints.

### 2.1 Full Pipeline Tests

Test the complete OCR pipeline from image to LaTeX output.

```rust
// tests/integration/pipeline_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig, OCRResult};
use std::fs;

#[test]
fn test_end_to_end_simple_equation() {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    let result = ocr.process_image("testdata/simple_equation.png")
        .expect("Failed to process image");

    assert!(!result.latex.is_empty());
    assert!(result.confidence > 0.8);
    assert!(result.latex.contains("x^2") || result.latex.contains("x²"));
}

#[test]
fn test_end_to_end_complex_expression() {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    let result = ocr.process_image("testdata/complex_integral.png")
        .expect("Failed to process image");

    assert!(!result.latex.is_empty());
    assert!(result.latex.contains(r"\int"));
    assert!(result.processing_time_ms < 1000);
}

#[test]
fn test_pipeline_with_preprocessing() {
    let mut config = OCRConfig::default();
    config.preprocessing.enable_enhancement = true;
    config.preprocessing.enable_deskew = true;

    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    // Test with skewed/low-quality image
    let result = ocr.process_image("testdata/skewed_equation.png")
        .expect("Failed to process image");

    assert!(!result.latex.is_empty());
    assert!(result.confidence > 0.7);
}

#[test]
fn test_pipeline_error_handling() {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    // Test with invalid image
    let result = ocr.process_image("testdata/nonexistent.png");
    assert!(result.is_err());

    // Test with corrupted image
    let result = ocr.process_image("testdata/corrupted.png");
    assert!(result.is_err());

    // Test with non-image file
    let result = ocr.process_image("testdata/text_file.txt");
    assert!(result.is_err());
}

#[test]
fn test_pipeline_batch_processing() {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    let images = vec![
        "testdata/equation1.png",
        "testdata/equation2.png",
        "testdata/equation3.png",
    ];

    let results = ocr.process_batch(&images).expect("Batch processing failed");

    assert_eq!(results.len(), 3);
    for result in results {
        assert!(!result.latex.is_empty());
    }
}
```

### 2.2 API Endpoint Tests

Test HTTP API endpoints (if applicable).

```rust
// tests/integration/api_tests.rs
use ruvector_scipix::server::{start_server, ServerConfig};
use reqwest::multipart;
use tokio;

#[tokio::test]
async fn test_api_health_check() {
    let config = ServerConfig::default();
    let server = start_server(config).await.unwrap();

    let client = reqwest::Client::new();
    let response = client.get("http://localhost:8080/health")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_api_ocr_endpoint() {
    let config = ServerConfig::default();
    let _server = start_server(config).await.unwrap();

    let client = reqwest::Client::new();
    let file_content = std::fs::read("testdata/equation.png").unwrap();

    let form = multipart::Form::new()
        .part("image", multipart::Part::bytes(file_content)
            .file_name("equation.png")
            .mime_str("image/png").unwrap());

    let response = client.post("http://localhost:8080/api/v1/ocr")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let result: OCRResult = response.json().await.unwrap();
    assert!(!result.latex.is_empty());
}

#[tokio::test]
async fn test_api_batch_endpoint() {
    let config = ServerConfig::default();
    let _server = start_server(config).await.unwrap();

    let client = reqwest::Client::new();

    let mut form = multipart::Form::new();
    for i in 1..=3 {
        let filename = format!("testdata/equation{}.png", i);
        let content = std::fs::read(&filename).unwrap();
        form = form.part(
            format!("image{}", i),
            multipart::Part::bytes(content)
                .file_name(format!("equation{}.png", i))
                .mime_str("image/png").unwrap()
        );
    }

    let response = client.post("http://localhost:8080/api/v1/batch")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_api_rate_limiting() {
    let config = ServerConfig {
        rate_limit: Some(5),
        ..Default::default()
    };
    let _server = start_server(config).await.unwrap();

    let client = reqwest::Client::new();

    // Make requests up to limit
    for _ in 0..5 {
        let response = client.get("http://localhost:8080/health")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }

    // Next request should be rate limited
    let response = client.get("http://localhost:8080/health")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 429);
}
```

### 2.3 Model Loading and Inference

Test model initialization and inference execution.

```rust
// tests/integration/model_tests.rs
use ruvector_scipix::model::{ModelLoader, InferenceEngine};
use std::time::Instant;

#[test]
fn test_model_loading() {
    let loader = ModelLoader::new();

    let start = Instant::now();
    let model = loader.load("models/scipix_model.onnx")
        .expect("Failed to load model");
    let load_time = start.elapsed();

    // Model should load in reasonable time
    assert!(load_time.as_secs() < 10,
        "Model loading took too long: {:?}", load_time);

    // Verify model metadata
    assert!(model.input_shape().len() > 0);
    assert!(model.output_shape().len() > 0);
}

#[test]
fn test_model_inference() {
    let loader = ModelLoader::new();
    let model = loader.load("models/scipix_model.onnx").unwrap();

    let engine = InferenceEngine::new(model);

    // Create dummy input tensor
    let input = create_test_tensor(1, 3, 384, 384);

    let start = Instant::now();
    let output = engine.run(&input).expect("Inference failed");
    let inference_time = start.elapsed();

    // Inference should be fast
    assert!(inference_time.as_millis() < 500,
        "Inference too slow: {:?}", inference_time);

    // Output should have expected shape
    assert!(output.len() > 0);
}

#[test]
fn test_gpu_acceleration() {
    if !cuda_available() {
        println!("Skipping GPU test - CUDA not available");
        return;
    }

    let loader = ModelLoader::with_device("cuda");
    let model = loader.load("models/scipix_model.onnx").unwrap();

    let engine = InferenceEngine::new(model);
    let input = create_test_tensor(1, 3, 384, 384);

    let start = Instant::now();
    let _output = engine.run(&input).expect("GPU inference failed");
    let gpu_time = start.elapsed();

    // GPU should be faster than CPU target
    assert!(gpu_time.as_millis() < 200,
        "GPU inference slower than expected: {:?}", gpu_time);
}

#[test]
fn test_model_batch_inference() {
    let loader = ModelLoader::new();
    let model = loader.load("models/scipix_model.onnx").unwrap();
    let engine = InferenceEngine::new(model);

    // Create batch of inputs
    let batch_size = 4;
    let inputs: Vec<_> = (0..batch_size)
        .map(|_| create_test_tensor(1, 3, 384, 384))
        .collect();

    let start = Instant::now();
    let outputs = engine.run_batch(&inputs).expect("Batch inference failed");
    let batch_time = start.elapsed();

    assert_eq!(outputs.len(), batch_size);

    // Batch should be more efficient than individual runs
    let avg_time_per_image = batch_time.as_millis() / batch_size as u128;
    assert!(avg_time_per_image < 300);
}

fn create_test_tensor(batch: usize, channels: usize, height: usize, width: usize) -> Vec<f32> {
    vec![0.5f32; batch * channels * height * width]
}

fn cuda_available() -> bool {
    // Check if CUDA is available
    std::env::var("CUDA_VISIBLE_DEVICES").is_ok()
}
```

### 2.4 Multi-Format Support

Test support for different image and output formats.

```rust
// tests/integration/format_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig, OutputFormat};

#[test]
fn test_input_format_png() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();
    let result = ocr.process_image("testdata/equation.png");
    assert!(result.is_ok());
}

#[test]
fn test_input_format_jpg() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();
    let result = ocr.process_image("testdata/equation.jpg");
    assert!(result.is_ok());
}

#[test]
fn test_input_format_tiff() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();
    let result = ocr.process_image("testdata/equation.tiff");
    assert!(result.is_ok());
}

#[test]
fn test_output_format_latex() {
    let mut config = OCRConfig::default();
    config.output.format = OutputFormat::LaTeX;

    let ocr = ScipixOCR::new(config).unwrap();
    let result = ocr.process_image("testdata/equation.png").unwrap();

    assert!(result.latex.starts_with("\\") || result.latex.chars().any(|c| c.is_alphanumeric()));
}

#[test]
fn test_output_format_mathml() {
    let mut config = OCRConfig::default();
    config.output.format = OutputFormat::MathML;

    let ocr = ScipixOCR::new(config).unwrap();
    let result = ocr.process_image("testdata/equation.png").unwrap();

    assert!(result.output.contains("<math"));
    assert!(result.output.contains("</math>"));
}

#[test]
fn test_output_format_unicode() {
    let mut config = OCRConfig::default();
    config.output.format = OutputFormat::Unicode;

    let ocr = ScipixOCR::new(config).unwrap();
    let result = ocr.process_image("testdata/greek_symbols.png").unwrap();

    // Should contain Unicode mathematical symbols
    assert!(result.output.chars().any(|c| c as u32 > 0x370));
}
```

---

## 3. Accuracy Testing

Accuracy tests validate OCR quality against ground truth data.

### 3.1 Character-Level Accuracy (CER)

```rust
// tests/accuracy/cer_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};
use ruvector_scipix::metrics::calculate_cer;
use std::fs;

#[test]
fn test_cer_simple_expressions() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let test_cases = vec![
        ("testdata/simple/001.png", "x^2 + 1"),
        ("testdata/simple/002.png", r"\frac{1}{2}"),
        ("testdata/simple/003.png", "a + b = c"),
    ];

    let mut total_cer = 0.0;
    for (image_path, ground_truth) in test_cases {
        let result = ocr.process_image(image_path).unwrap();
        let cer = calculate_cer(ground_truth, &result.latex);
        total_cer += cer;

        assert!(cer < 0.05, "CER too high for {}: {:.4}", image_path, cer);
    }

    let avg_cer = total_cer / 3.0;
    assert!(avg_cer < 0.02, "Average CER: {:.4}", avg_cer);
}

#[test]
fn test_cer_complex_expressions() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let ground_truth_file = fs::read_to_string("testdata/complex/ground_truth.json").unwrap();
    let ground_truth: Vec<TestCase> = serde_json::from_str(&ground_truth_file).unwrap();

    let mut cer_sum = 0.0;
    let mut count = 0;

    for case in ground_truth {
        let result = ocr.process_image(&case.image_path).unwrap();
        let cer = calculate_cer(&case.latex, &result.latex);
        cer_sum += cer;
        count += 1;

        println!("{}: CER = {:.4}", case.image_path, cer);
    }

    let avg_cer = cer_sum / count as f64;
    assert!(avg_cer < 0.03, "Complex expressions average CER: {:.4}", avg_cer);
}

#[derive(serde::Deserialize)]
struct TestCase {
    image_path: String,
    latex: String,
}
```

### 3.2 Expression-Level Accuracy

```rust
// tests/accuracy/expression_accuracy_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};
use ruvector_scipix::metrics::expressions_match;

#[test]
fn test_expression_accuracy_fractions() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let test_cases = vec![
        ("testdata/fractions/simple.png", r"\frac{1}{2}"),
        ("testdata/fractions/complex.png", r"\frac{x^2 + 1}{x - 1}"),
        ("testdata/fractions/nested.png", r"\frac{1}{\frac{1}{2}}"),
    ];

    let mut correct = 0;
    for (image_path, expected) in test_cases.iter() {
        let result = ocr.process_image(image_path).unwrap();
        if expressions_match(&result.latex, expected) {
            correct += 1;
        }
    }

    let accuracy = correct as f64 / test_cases.len() as f64;
    assert!(accuracy >= 0.9, "Fraction accuracy: {:.2}%", accuracy * 100.0);
}

#[test]
fn test_expression_accuracy_matrices() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let test_cases = vec![
        ("testdata/matrices/2x2.png",
         r"\begin{bmatrix} 1 & 2 \\ 3 & 4 \end{bmatrix}"),
        ("testdata/matrices/3x3.png",
         r"\begin{bmatrix} 1 & 2 & 3 \\ 4 & 5 & 6 \\ 7 & 8 & 9 \end{bmatrix}"),
    ];

    let mut correct = 0;
    for (image_path, expected) in test_cases.iter() {
        let result = ocr.process_image(image_path).unwrap();
        if expressions_match(&result.latex, expected) {
            correct += 1;
        }
    }

    let accuracy = correct as f64 / test_cases.len() as f64;
    assert!(accuracy >= 0.85, "Matrix accuracy: {:.2}%", accuracy * 100.0);
}

#[test]
fn test_expression_accuracy_integrals() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let test_cases = vec![
        ("testdata/integrals/simple.png", r"\int x dx"),
        ("testdata/integrals/limits.png", r"\int_{0}^{1} x^2 dx"),
        ("testdata/integrals/multiple.png", r"\int \int x y dx dy"),
    ];

    let mut correct = 0;
    for (image_path, expected) in test_cases.iter() {
        let result = ocr.process_image(image_path).unwrap();
        if expressions_match(&result.latex, expected) {
            correct += 1;
        }
    }

    let accuracy = correct as f64 / test_cases.len() as f64;
    assert!(accuracy >= 0.80, "Integral accuracy: {:.2}%", accuracy * 100.0);
}
```

### 3.3 Cross-Validation with Ground Truth

```rust
// tests/accuracy/cross_validation_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};
use ruvector_scipix::metrics::{calculate_cer, calculate_bleu};
use std::fs;

#[test]
fn test_cross_validation_im2latex() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    // Load Im2latex-100k test split
    let test_data = load_im2latex_test_split("testdata/im2latex/test.json");

    let sample_size = test_data.len().min(1000); // Test on 1000 samples
    let mut cer_sum = 0.0;
    let mut bleu_sum = 0.0;

    for case in test_data.iter().take(sample_size) {
        let result = ocr.process_image(&case.image_path).unwrap();

        let cer = calculate_cer(&case.latex, &result.latex);
        let bleu = calculate_bleu(&case.latex, &result.latex, 4);

        cer_sum += cer;
        bleu_sum += bleu;
    }

    let avg_cer = cer_sum / sample_size as f64;
    let avg_bleu = bleu_sum / sample_size as f64;

    println!("Im2latex cross-validation results:");
    println!("  Average CER: {:.4}", avg_cer);
    println!("  Average BLEU: {:.2}", avg_bleu);

    assert!(avg_cer < 0.03, "CER too high: {:.4}", avg_cer);
    assert!(avg_bleu > 80.0, "BLEU too low: {:.2}", avg_bleu);
}

#[test]
fn test_cross_validation_crohme() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    // Load CROHME handwritten dataset
    let test_data = load_crohme_test_split("testdata/crohme/test.json");

    let mut correct = 0;
    let total = test_data.len();

    for case in test_data {
        let result = ocr.process_image(&case.image_path).unwrap();

        if expressions_match(&result.latex, &case.latex) {
            correct += 1;
        }
    }

    let accuracy = correct as f64 / total as f64;
    println!("CROHME accuracy: {:.2}%", accuracy * 100.0);

    // Handwritten is harder, lower threshold
    assert!(accuracy > 0.70, "CROHME accuracy too low: {:.2}%", accuracy * 100.0);
}

fn load_im2latex_test_split(path: &str) -> Vec<TestCase> {
    let content = fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}

fn load_crohme_test_split(path: &str) -> Vec<TestCase> {
    let content = fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}

fn expressions_match(a: &str, b: &str) -> bool {
    // Normalize and compare expressions
    normalize_latex(a) == normalize_latex(b)
}

fn normalize_latex(latex: &str) -> String {
    latex.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}
```

### 3.4 Edge Cases Testing

```rust
// tests/accuracy/edge_cases_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};

#[test]
fn test_complex_fractions() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let cases = vec![
        "testdata/edge_cases/nested_fractions.png",
        "testdata/edge_cases/compound_fractions.png",
        "testdata/edge_cases/mixed_fractions.png",
    ];

    for case in cases {
        let result = ocr.process_image(case).unwrap();
        assert!(!result.latex.is_empty());
        assert!(result.latex.contains(r"\frac"));
        assert!(result.confidence > 0.6); // Lower threshold for complex cases
    }
}

#[test]
fn test_nested_structures() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let result = ocr.process_image("testdata/edge_cases/deeply_nested.png").unwrap();

    // Verify nested structure is captured
    let brace_count = result.latex.matches('{').count();
    assert!(brace_count >= 4, "Should capture nested structure");
}

#[test]
fn test_special_characters() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let result = ocr.process_image("testdata/edge_cases/special_chars.png").unwrap();

    // Should recognize special mathematical symbols
    let special_symbols = vec![r"\infty", r"\partial", r"\nabla", r"\sum", r"\prod"];
    let recognized = special_symbols.iter()
        .filter(|s| result.latex.contains(*s))
        .count();

    assert!(recognized > 0, "Should recognize special symbols");
}

#[test]
fn test_multi_line_equations() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let result = ocr.process_image("testdata/edge_cases/multi_line.png").unwrap();

    // Should handle multi-line environments
    assert!(
        result.latex.contains(r"\begin{align}") ||
        result.latex.contains(r"\begin{equation}") ||
        result.latex.contains("\\\\")
    );
}

#[test]
fn test_low_quality_images() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let cases = vec![
        "testdata/edge_cases/low_resolution.png",
        "testdata/edge_cases/noisy.png",
        "testdata/edge_cases/blurry.png",
    ];

    for case in cases {
        let result = ocr.process_image(case);

        // Should handle gracefully, even if accuracy is lower
        assert!(result.is_ok(), "Should process {} without crashing", case);

        if let Ok(res) = result {
            assert!(!res.latex.is_empty(), "Should produce some output for {}", case);
        }
    }
}
```

---

## 4. Performance Testing

Performance tests measure latency, throughput, and resource usage using Criterion.

### 4.1 Latency Benchmarks

```rust
// benches/latency_benchmarks.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ruvector_scipix::{ScipixOCR, OCRConfig};

fn benchmark_single_image_latency(c: &mut Criterion) {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    c.bench_function("ocr_simple_equation", |b| {
        b.iter(|| {
            ocr.process_image(black_box("testdata/simple_equation.png"))
        });
    });
}

fn benchmark_latency_by_complexity(c: &mut Criterion) {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    let mut group = c.benchmark_group("latency_by_complexity");

    let test_cases = vec![
        ("simple", "testdata/simple.png"),
        ("medium", "testdata/medium.png"),
        ("complex", "testdata/complex.png"),
        ("very_complex", "testdata/very_complex.png"),
    ];

    for (name, path) in test_cases {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &path,
            |b, &path| {
                b.iter(|| ocr.process_image(black_box(path)));
            },
        );
    }

    group.finish();
}

fn benchmark_latency_percentiles(c: &mut Criterion) {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    let mut group = c.benchmark_group("latency_percentiles");
    group.sample_size(1000); // Large sample for accurate percentiles

    group.bench_function("p50_p95_p99", |b| {
        b.iter(|| {
            ocr.process_image(black_box("testdata/equation.png"))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_single_image_latency,
    benchmark_latency_by_complexity,
    benchmark_latency_percentiles
);
criterion_main!(benches);
```

### 4.2 Memory Leak Detection

```rust
// tests/performance/memory_leak_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATED: AtomicUsize = AtomicUsize::new(0);

struct TrackingAllocator;

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::SeqCst);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOCATED.fetch_add(layout.size(), Ordering::SeqCst);
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

#[test]
fn test_memory_leak_repeated_processing() {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).unwrap();

    // Clear counters
    let start_allocated = ALLOCATED.load(Ordering::SeqCst);
    let start_deallocated = DEALLOCATED.load(Ordering::SeqCst);
    let start_diff = start_allocated - start_deallocated;

    // Process 100 images
    for _ in 0..100 {
        let _ = ocr.process_image("testdata/equation.png");
    }

    // Force cleanup
    drop(ocr);

    // Allow some time for cleanup
    std::thread::sleep(std::time::Duration::from_millis(100));

    let end_allocated = ALLOCATED.load(Ordering::SeqCst);
    let end_deallocated = DEALLOCATED.load(Ordering::SeqCst);
    let end_diff = end_allocated - end_deallocated;

    let leaked = end_diff - start_diff;

    println!("Memory leaked: {} bytes", leaked);

    // Allow 10MB leak tolerance (for caches, etc.)
    assert!(leaked < 10 * 1024 * 1024,
        "Memory leak detected: {} bytes", leaked);
}

#[test]
fn test_memory_leak_model_reload() {
    let config = OCRConfig::default();

    let start_allocated = ALLOCATED.load(Ordering::SeqCst);
    let start_deallocated = DEALLOCATED.load(Ordering::SeqCst);

    // Load and unload model 10 times
    for _ in 0..10 {
        let ocr = ScipixOCR::new(config.clone()).unwrap();
        let _ = ocr.process_image("testdata/equation.png");
        drop(ocr);
    }

    std::thread::sleep(std::time::Duration::from_millis(100));

    let end_allocated = ALLOCATED.load(Ordering::SeqCst);
    let end_deallocated = DEALLOCATED.load(Ordering::SeqCst);

    let leaked = (end_allocated - start_allocated) - (end_deallocated - start_deallocated);

    println!("Memory leaked after reloads: {} bytes", leaked);

    assert!(leaked < 5 * 1024 * 1024,
        "Memory leak in model reload: {} bytes", leaked);
}
```

### 4.3 Stress Testing

```rust
// tests/performance/stress_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn test_sustained_load() {
    let config = OCRConfig::default();
    let ocr = Arc::new(ScipixOCR::new(config).unwrap());

    let duration = Duration::from_secs(60); // 1 minute stress test
    let start = Instant::now();
    let mut count = 0;
    let mut errors = 0;

    while start.elapsed() < duration {
        match ocr.process_image("testdata/equation.png") {
            Ok(_) => count += 1,
            Err(_) => errors += 1,
        }
    }

    println!("Processed {} images in 60 seconds", count);
    println!("Errors: {}", errors);

    assert!(count > 100, "Should process at least 100 images");
    assert!(errors < count / 10, "Error rate too high: {}/{}", errors, count);
}

#[test]
fn test_concurrent_processing() {
    let config = OCRConfig::default();
    let ocr = Arc::new(ScipixOCR::new(config).unwrap());

    let num_threads = 8;
    let images_per_thread = 10;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let ocr_clone = Arc::clone(&ocr);
            thread::spawn(move || {
                let mut results = Vec::new();
                for _ in 0..images_per_thread {
                    let result = ocr_clone.process_image("testdata/equation.png");
                    results.push(result.is_ok());
                }
                results
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .flatten()
        .collect();

    let success_count = results.iter().filter(|&&x| x).count();
    let total = num_threads * images_per_thread;

    println!("Concurrent processing: {}/{} successful", success_count, total);

    assert!(success_count >= (total * 95) / 100,
        "Success rate too low: {}/{}", success_count, total);
}

#[test]
fn test_memory_under_load() {
    use sysinfo::{System, SystemExt};

    let mut sys = System::new_all();
    sys.refresh_memory();

    let start_memory = sys.used_memory();

    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).unwrap();

    // Process 1000 images
    for i in 0..1000 {
        let _ = ocr.process_image("testdata/equation.png");

        if i % 100 == 0 {
            sys.refresh_memory();
            let current_memory = sys.used_memory();
            println!("Memory at {}: {} KB", i, current_memory);
        }
    }

    sys.refresh_memory();
    let end_memory = sys.used_memory();
    let memory_growth = end_memory - start_memory;

    println!("Memory growth: {} KB", memory_growth);

    // Should not grow more than 500MB
    assert!(memory_growth < 500 * 1024,
        "Excessive memory growth: {} KB", memory_growth);
}
```

### 4.4 Concurrency Testing

```rust
// tests/performance/concurrency_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Instant;

#[test]
fn test_thread_safety() {
    let config = OCRConfig::default();
    let ocr = Arc::new(ScipixOCR::new(config).unwrap());

    let num_threads = 16;
    let barrier = Arc::new(Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let ocr_clone = Arc::clone(&ocr);
            let barrier_clone = Arc::clone(&barrier);

            thread::spawn(move || {
                // Wait for all threads to be ready
                barrier_clone.wait();

                // All threads process simultaneously
                let result = ocr_clone.process_image("testdata/equation.png");

                (i, result.is_ok())
            })
        })
        .collect();

    let results: Vec<_> = handles.into_iter()
        .map(|h| h.join().unwrap())
        .collect();

    let all_successful = results.iter().all(|(_, success)| *success);
    assert!(all_successful, "Some threads failed: {:?}", results);
}

#[test]
fn test_concurrent_throughput() {
    let config = OCRConfig::default();
    let ocr = Arc::new(ScipixOCR::new(config).unwrap());

    let num_threads = 8;
    let images_per_thread = 50;

    let start = Instant::now();

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let ocr_clone = Arc::clone(&ocr);
            thread::spawn(move || {
                for _ in 0..images_per_thread {
                    let _ = ocr_clone.process_image("testdata/equation.png");
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let duration = start.elapsed();
    let total_images = num_threads * images_per_thread;
    let throughput = total_images as f64 / duration.as_secs_f64();

    println!("Concurrent throughput: {:.2} images/sec", throughput);

    assert!(throughput > 5.0,
        "Throughput too low: {:.2} images/sec", throughput);
}
```

---

## 5. Regression Testing

Track performance and accuracy regressions over time.

### 5.1 Golden File Comparison

```rust
// tests/regression/golden_file_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
struct GoldenResult {
    image: String,
    latex: String,
    confidence: f64,
}

#[test]
fn test_golden_file_consistency() {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).unwrap();

    // Load golden results
    let golden_path = "testdata/regression/golden_results.json";
    let golden_content = fs::read_to_string(golden_path).unwrap();
    let golden_results: Vec<GoldenResult> = serde_json::from_str(&golden_content).unwrap();

    let mut matches = 0;
    let mut total = 0;

    for golden in golden_results {
        let result = ocr.process_image(&golden.image).unwrap();
        total += 1;

        if normalize_latex(&result.latex) == normalize_latex(&golden.latex) {
            matches += 1;
        } else {
            println!("Mismatch for {}:", golden.image);
            println!("  Expected: {}", golden.latex);
            println!("  Got:      {}", result.latex);
        }
    }

    let consistency = matches as f64 / total as f64;
    println!("Golden file consistency: {:.2}%", consistency * 100.0);

    assert!(consistency >= 0.95,
        "Regression detected: only {:.2}% match", consistency * 100.0);
}

#[test]
#[ignore] // Run manually to update golden files
fn update_golden_files() {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).unwrap();

    let test_images = vec![
        "testdata/regression/test1.png",
        "testdata/regression/test2.png",
        "testdata/regression/test3.png",
    ];

    let mut golden_results = Vec::new();

    for image in test_images {
        let result = ocr.process_image(image).unwrap();
        golden_results.push(GoldenResult {
            image: image.to_string(),
            latex: result.latex,
            confidence: result.confidence,
        });
    }

    let json = serde_json::to_string_pretty(&golden_results).unwrap();
    fs::write("testdata/regression/golden_results.json", json).unwrap();

    println!("Golden files updated");
}

fn normalize_latex(latex: &str) -> String {
    latex.chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}
```

### 5.2 Baseline Accuracy Tracking

```rust
// tests/regression/accuracy_tracking_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};
use ruvector_scipix::metrics::calculate_cer;
use std::fs;
use serde::{Deserialize, Serialize};
use chrono::Utc;

#[derive(Serialize, Deserialize)]
struct AccuracyBaseline {
    timestamp: String,
    commit_hash: String,
    average_cer: f64,
    average_confidence: f64,
    test_count: usize,
}

#[test]
fn test_accuracy_regression() {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).unwrap();

    // Load baseline
    let baseline_path = "testdata/regression/accuracy_baseline.json";
    let baseline: AccuracyBaseline = if std::path::Path::new(baseline_path).exists() {
        let content = fs::read_to_string(baseline_path).unwrap();
        serde_json::from_str(&content).unwrap()
    } else {
        AccuracyBaseline {
            timestamp: Utc::now().to_rfc3339(),
            commit_hash: get_git_commit(),
            average_cer: 0.02,
            average_confidence: 0.90,
            test_count: 0,
        }
    };

    // Run current tests
    let test_cases = load_test_cases("testdata/regression/test_suite.json");

    let mut cer_sum = 0.0;
    let mut confidence_sum = 0.0;

    for case in &test_cases {
        let result = ocr.process_image(&case.image).unwrap();
        let cer = calculate_cer(&case.latex, &result.latex);

        cer_sum += cer;
        confidence_sum += result.confidence;
    }

    let current_avg_cer = cer_sum / test_cases.len() as f64;
    let current_avg_confidence = confidence_sum / test_cases.len() as f64;

    println!("Accuracy comparison:");
    println!("  Baseline CER: {:.4}", baseline.average_cer);
    println!("  Current CER:  {:.4}", current_avg_cer);
    println!("  Baseline Confidence: {:.4}", baseline.average_confidence);
    println!("  Current Confidence:  {:.4}", current_avg_confidence);

    // Check for regressions (allow 10% tolerance)
    let cer_regression = (current_avg_cer - baseline.average_cer) / baseline.average_cer;
    assert!(cer_regression < 0.10,
        "CER regression detected: {:.2}%", cer_regression * 100.0);

    let confidence_regression = (baseline.average_confidence - current_avg_confidence) / baseline.average_confidence;
    assert!(confidence_regression < 0.10,
        "Confidence regression detected: {:.2}%", confidence_regression * 100.0);
}

#[derive(Deserialize)]
struct TestCase {
    image: String,
    latex: String,
}

fn load_test_cases(path: &str) -> Vec<TestCase> {
    let content = fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}

fn get_git_commit() -> String {
    std::process::Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}
```

### 5.3 API Compatibility Checks

```rust
// tests/regression/api_compatibility_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig, OCRResult};

#[test]
fn test_api_backward_compatibility() {
    // Verify OCRConfig fields
    let config = OCRConfig::default();
    let _ = config.model;
    let _ = config.preprocessing;
    let _ = config.output;

    // Verify OCRResult fields
    let ocr = ScipixOCR::new(config).unwrap();
    let result = ocr.process_image("testdata/equation.png").unwrap();

    let _ = result.latex;
    let _ = result.confidence;
    let _ = result.processing_time_ms;
    let _ = result.detected_symbols;

    // All fields should be accessible without breaking changes
}

#[test]
fn test_serialization_compatibility() {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).unwrap();
    let result = ocr.process_image("testdata/equation.png").unwrap();

    // Should serialize/deserialize without errors
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: OCRResult = serde_json::from_str(&json).unwrap();

    assert_eq!(result.latex, deserialized.latex);
    assert_eq!(result.confidence, deserialized.confidence);
}
```

---

## 6. Fuzz Testing

Fuzz testing to discover edge cases and improve robustness.

### 6.1 Image Corruption Handling

```rust
// tests/fuzz/image_corruption_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};
use image::{DynamicImage, GenericImageView};
use rand::Rng;

#[test]
fn test_fuzz_random_noise() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let base_image = image::open("testdata/equation.png").unwrap();

    for _ in 0..10 {
        let mut noisy = base_image.clone().into_rgba8();
        add_random_noise(&mut noisy, 0.1);

        let noisy_image = DynamicImage::ImageRgba8(noisy);
        noisy_image.save("/tmp/noisy.png").unwrap();

        // Should handle noisy images without crashing
        let result = ocr.process_image("/tmp/noisy.png");
        assert!(result.is_ok() || result.is_err(),
            "Should return valid result or error");
    }
}

#[test]
fn test_fuzz_pixel_corruption() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let base_image = image::open("testdata/equation.png").unwrap();

    for corruption_rate in [0.01, 0.05, 0.10, 0.20] {
        let mut corrupted = base_image.clone().into_rgba8();
        corrupt_pixels(&mut corrupted, corruption_rate);

        let corrupted_image = DynamicImage::ImageRgba8(corrupted);
        corrupted_image.save("/tmp/corrupted.png").unwrap();

        let result = ocr.process_image("/tmp/corrupted.png");

        // Even with corruption, should not crash
        match result {
            Ok(res) => println!("Corrupted {:.0}%: confidence = {:.2}",
                corruption_rate * 100.0, res.confidence),
            Err(e) => println!("Corrupted {:.0}%: error = {:?}",
                corruption_rate * 100.0, e),
        }
    }
}

fn add_random_noise(img: &mut image::RgbaImage, intensity: f32) {
    let mut rng = rand::thread_rng();

    for pixel in img.pixels_mut() {
        for channel in 0..3 {
            let noise = rng.gen_range(-intensity..intensity) * 255.0;
            let new_value = (pixel[channel] as f32 + noise).clamp(0.0, 255.0) as u8;
            pixel[channel] = new_value;
        }
    }
}

fn corrupt_pixels(img: &mut image::RgbaImage, rate: f32) {
    let mut rng = rand::thread_rng();
    let (width, height) = img.dimensions();

    for y in 0..height {
        for x in 0..width {
            if rng.gen::<f32>() < rate {
                let pixel = img.get_pixel_mut(x, y);
                pixel[0] = rng.gen();
                pixel[1] = rng.gen();
                pixel[2] = rng.gen();
            }
        }
    }
}
```

### 6.2 Invalid Input Resilience

```rust
// tests/fuzz/invalid_input_tests.rs
use ruvector_scipix::{ScipixOCR, OCRConfig};

#[test]
fn test_fuzz_invalid_file_paths() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    let invalid_paths = vec![
        "",
        "/nonexistent/path/image.png",
        "../../../etc/passwd",
        "testdata/\0null_byte.png",
        "x".repeat(10000), // Very long path
    ];

    for path in invalid_paths {
        let result = ocr.process_image(path);
        assert!(result.is_err(), "Should reject invalid path: {}", path);
    }
}

#[test]
fn test_fuzz_malformed_images() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    // Create malformed image files
    std::fs::write("/tmp/empty.png", b"").unwrap();
    std::fs::write("/tmp/random.png", &[0u8; 1000]).unwrap();
    std::fs::write("/tmp/truncated.png", &[137, 80, 78, 71]).unwrap(); // PNG header only

    let malformed = vec![
        "/tmp/empty.png",
        "/tmp/random.png",
        "/tmp/truncated.png",
    ];

    for path in malformed {
        let result = ocr.process_image(path);
        assert!(result.is_err(), "Should reject malformed image: {}", path);
    }
}

#[test]
fn test_fuzz_extreme_dimensions() {
    let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();

    // Create images with extreme dimensions
    let tiny = image::RgbaImage::new(1, 1);
    tiny.save("/tmp/tiny.png").unwrap();

    let wide = image::RgbaImage::new(10000, 10);
    wide.save("/tmp/wide.png").unwrap();

    let tall = image::RgbaImage::new(10, 10000);
    tall.save("/tmp/tall.png").unwrap();

    // Should handle gracefully
    let _ = ocr.process_image("/tmp/tiny.png");
    let _ = ocr.process_image("/tmp/wide.png");
    let _ = ocr.process_image("/tmp/tall.png");
}
```

### 6.3 Buffer Overflow Prevention

```rust
// tests/fuzz/buffer_overflow_tests.rs
use ruvector_scipix::latex::LatexParser;

#[test]
fn test_fuzz_latex_parser_large_input() {
    let parser = LatexParser::new();

    // Very long valid LaTeX
    let long_valid = "x + ".repeat(10000) + "1";
    let result = parser.parse(&long_valid);

    // Should handle without crash or overflow
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_fuzz_deeply_nested_latex() {
    let parser = LatexParser::new();

    // Deeply nested braces
    let mut nested = String::new();
    for _ in 0..1000 {
        nested.push_str(r"\frac{");
    }
    nested.push('1');
    for _ in 0..1000 {
        nested.push('}');
    }

    let result = parser.parse(&nested);

    // Should either parse or reject gracefully
    match result {
        Ok(_) => println!("Parsed deeply nested structure"),
        Err(e) => println!("Rejected nested structure: {:?}", e),
    }
}

#[test]
fn test_fuzz_special_characters() {
    let parser = LatexParser::new();

    let special_inputs = vec![
        "\0\0\0",
        "\u{FFFF}".repeat(100),
        "\\".repeat(1000),
        "{}{}{}".repeat(1000),
        r"\begin{matrix}".repeat(100),
    ];

    for input in special_inputs {
        let result = parser.parse(&input);
        // Should not crash, regardless of output
        let _ = result;
    }
}
```

---

## 7. Test Data Management

### 7.1 Synthetic Test Data Generation

```rust
// tests/testdata/synthetic_generator.rs
use image::{RgbaImage, Rgba};
use rusttype::{Font, Scale, point};

pub struct SyntheticDataGenerator {
    font: Font<'static>,
}

impl SyntheticDataGenerator {
    pub fn new() -> Self {
        let font_data = include_bytes!("../../assets/fonts/DejaVuSans.ttf");
        let font = Font::try_from_bytes(font_data as &[u8]).unwrap();

        Self { font }
    }

    pub fn generate_simple_equation(&self, equation: &str) -> RgbaImage {
        let scale = Scale::uniform(32.0);
        let mut image = RgbaImage::from_pixel(400, 100, Rgba([255, 255, 255, 255]));

        imageproc::drawing::draw_text_mut(
            &mut image,
            Rgba([0, 0, 0, 255]),
            10,
            30,
            scale,
            &self.font,
            equation
        );

        image
    }

    pub fn generate_with_noise(&self, equation: &str, noise_level: f32) -> RgbaImage {
        let mut image = self.generate_simple_equation(equation);

        let mut rng = rand::thread_rng();
        for pixel in image.pixels_mut() {
            if rng.gen::<f32>() < noise_level {
                pixel[0] = rng.gen();
                pixel[1] = rng.gen();
                pixel[2] = rng.gen();
            }
        }

        image
    }

    pub fn generate_batch(&self, equations: &[&str], output_dir: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(output_dir)?;

        for (i, equation) in equations.iter().enumerate() {
            let image = self.generate_simple_equation(equation);
            let path = format!("{}/equation_{:04}.png", output_dir, i);
            image.save(&path).unwrap();
        }

        Ok(())
    }
}

#[test]
fn test_generate_test_dataset() {
    let generator = SyntheticDataGenerator::new();

    let equations = vec![
        "x^2 + 1",
        "a + b = c",
        "f(x) = 2x + 1",
        "∫ x dx",
        "∑ i=1 to n",
    ];

    generator.generate_batch(&equations, "testdata/synthetic").unwrap();
}
```

### 7.2 Real-World Sample Collection

```rust
// tests/testdata/sample_collector.rs
use std::fs;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SampleMetadata {
    pub image_path: String,
    pub source: String,
    pub difficulty: String,
    pub latex_ground_truth: Option<String>,
    pub tags: Vec<String>,
}

pub struct SampleCollector {
    output_dir: String,
}

impl SampleCollector {
    pub fn new(output_dir: &str) -> Self {
        fs::create_dir_all(output_dir).unwrap();
        Self {
            output_dir: output_dir.to_string(),
        }
    }

    pub fn add_sample(&self,
        image_path: &str,
        metadata: SampleMetadata
    ) -> std::io::Result<()> {
        // Copy image to collection
        let filename = std::path::Path::new(image_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        let dest_path = format!("{}/{}", self.output_dir, filename);
        fs::copy(image_path, &dest_path)?;

        // Save metadata
        let metadata_path = format!("{}/{}.json", self.output_dir, filename);
        let json = serde_json::to_string_pretty(&metadata)?;
        fs::write(metadata_path, json)?;

        Ok(())
    }

    pub fn export_manifest(&self) -> std::io::Result<()> {
        let mut samples = Vec::new();

        for entry in fs::read_dir(&self.output_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                let metadata: SampleMetadata = serde_json::from_str(&content)?;
                samples.push(metadata);
            }
        }

        let manifest = serde_json::to_string_pretty(&samples)?;
        fs::write(format!("{}/manifest.json", self.output_dir), manifest)?;

        Ok(())
    }
}
```

### 7.3 Ground Truth Annotation Format

```rust
// tests/testdata/ground_truth.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct GroundTruthEntry {
    pub image_id: String,
    pub image_path: String,
    pub latex: String,
    pub mathml: Option<String>,
    pub ascii_math: Option<String>,
    pub difficulty_level: u8,
    pub symbol_count: usize,
    pub contains_matrices: bool,
    pub contains_fractions: bool,
    pub contains_integrals: bool,
    pub annotations: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct GroundTruthDataset {
    pub version: String,
    pub created_at: String,
    pub description: String,
    pub entries: Vec<GroundTruthEntry>,
}

impl GroundTruthDataset {
    pub fn new(description: &str) -> Self {
        Self {
            version: "1.0.0".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            description: description.to_string(),
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, entry: GroundTruthEntry) {
        self.entries.push(entry);
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &str) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let dataset = serde_json::from_str(&content)?;
        Ok(dataset)
    }

    pub fn filter_by_difficulty(&self, level: u8) -> Vec<&GroundTruthEntry> {
        self.entries.iter()
            .filter(|e| e.difficulty_level == level)
            .collect()
    }
}
```

---

## 8. CI/CD Integration

### 8.1 GitHub Actions Workflow

```yaml
# .github/workflows/test.yml
name: Comprehensive Testing

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

env:
  RUST_BACKTRACE: 1
  RUSTFLAGS: "-D warnings"

jobs:
  unit-tests:
    name: Unit Tests
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
          components: rustfmt, clippy

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo index
        uses: actions/cache@v3
        with:
          path: ~/.cargo/git
          key: ${{ runner.os }}-cargo-index-${{ hashFiles('**/Cargo.lock') }}

      - name: Cache cargo build
        uses: actions/cache@v3
        with:
          path: target
          key: ${{ runner.os }}-cargo-build-${{ hashFiles('**/Cargo.lock') }}

      - name: Run unit tests
        run: cargo test --lib --all-features

      - name: Run doc tests
        run: cargo test --doc

  integration-tests:
    name: Integration Tests
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true

      - name: Download test datasets
        run: |
          mkdir -p testdata
          # Download sample images
          wget -O testdata/equation.png https://example.com/test-images/equation.png

      - name: Run integration tests
        run: cargo test --test '*' --all-features

  accuracy-tests:
    name: Accuracy Tests
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Download ground truth dataset
        run: |
          mkdir -p testdata/accuracy
          # Download Im2latex test split
          wget https://example.com/datasets/im2latex_test.tar.gz
          tar -xzf im2latex_test.tar.gz -C testdata/accuracy

      - name: Run accuracy tests
        run: cargo test --test accuracy_tests

      - name: Check accuracy threshold
        run: |
          cargo run --bin check_accuracy -- \
            --threshold 0.02 \
            --dataset testdata/accuracy

  performance-tests:
    name: Performance Benchmarks
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run benchmarks
        run: cargo bench --bench '*' -- --save-baseline current

      - name: Download baseline
        id: download-baseline
        continue-on-error: true
        run: |
          gh release download baseline \
            --pattern 'benchmark_baseline.json' \
            --dir target/criterion
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Compare with baseline
        if: steps.download-baseline.outcome == 'success'
        run: |
          cargo install critcmp
          critcmp baseline current

      - name: Upload benchmark results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: target/criterion/

  regression-tests:
    name: Regression Tests
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Download regression baseline
        run: |
          gh release download regression-baseline \
            --pattern 'golden_results.json' \
            --dir testdata/regression
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Run regression tests
        run: cargo test --test regression_tests

  coverage:
    name: Code Coverage
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true

      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin

      - name: Generate coverage
        run: |
          cargo tarpaulin --out Xml --output-dir ./coverage \
            --all-features --workspace

      - name: Upload coverage to Codecov
        uses: codecov/codecov-action@v3
        with:
          files: ./coverage/cobertura.xml
          fail_ci_if_error: true

      - name: Check coverage threshold
        run: |
          COVERAGE=$(cargo tarpaulin --output-dir ./coverage --all-features | \
            grep -oP '\d+\.\d+%' | head -1 | grep -oP '\d+\.\d+')

          if (( $(echo "$COVERAGE < 80.0" | bc -l) )); then
            echo "Coverage $COVERAGE% is below 80% threshold"
            exit 1
          fi
```

### 8.2 Test Coverage Requirements (80%+)

```rust
// tests/coverage/coverage_check.rs
use std::process::Command;

#[test]
#[ignore] // Run separately with `cargo test --ignored`
fn check_test_coverage() {
    let output = Command::new("cargo")
        .args(&["tarpaulin", "--output-dir", "./coverage", "--all-features"])
        .output()
        .expect("Failed to run cargo tarpaulin");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse coverage percentage
    let coverage_line = stdout.lines()
        .find(|line| line.contains("Coverage:"))
        .expect("Could not find coverage line");

    let coverage: f64 = coverage_line
        .split_whitespace()
        .find_map(|s| s.trim_end_matches('%').parse().ok())
        .expect("Could not parse coverage percentage");

    println!("Test coverage: {:.2}%", coverage);

    assert!(coverage >= 80.0,
        "Test coverage {:.2}% is below 80% threshold", coverage);
}
```

### 8.3 Automated Regression Detection

See earlier section on Regression Testing (5.2).

---

## 9. Property-Based Testing

Property-based testing with `proptest` for invariant checking.

### 9.1 Proptest for Invariants

```rust
// tests/property/preprocessing_properties.rs
use proptest::prelude::*;
use ruvector_scipix::preprocessing::{ImageEnhancer, Binarizer};
use image::{GrayImage, Luma};

proptest! {
    #[test]
    fn test_binarization_only_produces_binary_values(
        width in 10u32..200u32,
        height in 10u32..200u32
    ) {
        let img = generate_random_image(width, height);
        let binarizer = Binarizer;
        let binary = binarizer.otsu_binarize(&img);

        // Property: All pixels should be either 0 or 255
        for pixel in binary.pixels() {
            prop_assert!(pixel[0] == 0 || pixel[0] == 255);
        }
    }

    #[test]
    fn test_enhancement_preserves_dimensions(
        width in 10u32..200u32,
        height in 10u32..200u32
    ) {
        let img = generate_random_image(width, height);
        let enhancer = ImageEnhancer::new();
        let enhanced = enhancer.apply_clahe(&img);

        // Property: Dimensions should be preserved
        prop_assert_eq!(enhanced.width(), width);
        prop_assert_eq!(enhanced.height(), height);
    }

    #[test]
    fn test_rotation_inverse_property(
        angle in -180.0f32..180.0f32
    ) {
        let img = load_test_image();
        let detector = RotationDetector;

        // Rotate and rotate back
        let rotated = detector.rotate_image(&img, angle);
        let back = detector.rotate_image(&rotated, -angle);

        // Property: Should be close to original (allowing for interpolation error)
        let similarity = image_similarity(&img, &back);
        prop_assert!(similarity > 0.95,
            "Similarity {} too low for angle {}", similarity, angle);
    }
}

fn generate_random_image(width: u32, height: u32) -> GrayImage {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    GrayImage::from_fn(width, height, |_, _| {
        Luma([rng.gen_range(0..256)])
    })
}

fn load_test_image() -> GrayImage {
    image::open("testdata/simple.png").unwrap().to_luma8()
}

fn image_similarity(a: &GrayImage, b: &GrayImage) -> f64 {
    if a.dimensions() != b.dimensions() {
        return 0.0;
    }

    let total_pixels = (a.width() * a.height()) as f64;
    let matching_pixels = a.pixels()
        .zip(b.pixels())
        .filter(|(p1, p2)| (p1[0] as i32 - p2[0] as i32).abs() < 10)
        .count() as f64;

    matching_pixels / total_pixels
}
```

### 9.2 Roundtrip Testing (Image → LaTeX → Render → Compare)

```rust
// tests/property/roundtrip_tests.rs
use proptest::prelude::*;
use ruvector_scipix::{ScipixOCR, OCRConfig};
use ruvector_scipix::render::LatexRenderer;

proptest! {
    #[test]
    fn test_latex_roundtrip_simple_expressions(
        a in 1i32..100,
        b in 1i32..100,
        op in prop::sample::select(vec!['+', '-', '*'])
    ) {
        let latex = format!("{} {} {}", a, op, b);

        // Render to image
        let renderer = LatexRenderer::new();
        let image = renderer.render(&latex).unwrap();
        image.save("/tmp/roundtrip.png").unwrap();

        // OCR back to LaTeX
        let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();
        let result = ocr.process_image("/tmp/roundtrip.png").unwrap();

        // Property: Should recognize the expression
        let normalized_original = normalize_latex(&latex);
        let normalized_result = normalize_latex(&result.latex);

        prop_assert_eq!(normalized_original, normalized_result);
    }

    #[test]
    fn test_latex_roundtrip_fractions(
        numerator in 1i32..20,
        denominator in 1i32..20
    ) {
        let latex = format!(r"\frac{{{}}}{{{}}}", numerator, denominator);

        let renderer = LatexRenderer::new();
        let image = renderer.render(&latex).unwrap();
        image.save("/tmp/fraction_roundtrip.png").unwrap();

        let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();
        let result = ocr.process_image("/tmp/fraction_roundtrip.png").unwrap();

        // Property: Should contain fraction with correct numerator and denominator
        prop_assert!(result.latex.contains(r"\frac"));
        prop_assert!(result.latex.contains(&numerator.to_string()));
        prop_assert!(result.latex.contains(&denominator.to_string()));
    }
}

fn normalize_latex(latex: &str) -> String {
    latex.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}
```

---

## 10. Test Coverage Requirements

### Target Coverage Metrics

- **Overall Coverage**: 80%+
- **Critical Paths**: 95%+
- **Unit Tests**: 90%+
- **Integration Tests**: 80%+

### Coverage Enforcement

```bash
# Run with coverage
cargo tarpaulin --out Html --output-dir coverage --all-features

# Check threshold
cargo tarpaulin --fail-under 80

# Generate detailed report
cargo tarpaulin --out Lcov --output-dir coverage
```

### Per-Module Requirements

```rust
// Each module should have comprehensive tests
mod preprocessing {
    // Unit tests for each function
    #[cfg(test)]
    mod tests {
        // Test coverage: 90%+
    }
}

mod model {
    #[cfg(test)]
    mod tests {
        // Test coverage: 85%+
    }
}

mod api {
    #[cfg(test)]
    mod tests {
        // Test coverage: 80%+
    }
}
```

---

## Running Tests

```bash
# Run all tests
cargo test --all-features

# Run specific test suite
cargo test --test unit_tests
cargo test --test integration_tests
cargo test --test accuracy_tests

# Run benchmarks
cargo bench

# Run with coverage
cargo tarpaulin --all-features

# Run property tests
cargo test --test property_tests

# Run fuzz tests
cargo test --test fuzz_tests
```

---

## Conclusion

This comprehensive testing strategy ensures the ruvector-scipix OCR system maintains high quality, performance, and reliability through:

1. **Extensive Unit Testing** - Individual component validation
2. **Integration Testing** - End-to-end pipeline verification
3. **Accuracy Validation** - CER, WER, BLEU metrics against ground truth
4. **Performance Benchmarking** - Latency, throughput, and resource tracking
5. **Regression Protection** - Golden file comparison and baseline tracking
6. **Robustness Testing** - Fuzz testing for edge cases
7. **Automated CI/CD** - Continuous testing and coverage enforcement
8. **Property-Based Testing** - Invariant checking with proptest

**Test Execution Summary:**
- 500+ unit tests
- 100+ integration tests
- 50+ accuracy tests
- 20+ performance benchmarks
- 30+ regression tests
- 40+ fuzz tests
- 80%+ code coverage requirement

This strategy provides confidence in code quality, prevents regressions, and ensures the OCR system meets production requirements.
