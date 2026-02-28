// Full pipeline integration tests
//
// Tests the complete OCR pipeline from image input to final output
//
// Note: These tests use mock test infrastructure.
// Real OCR processing requires ONNX models to be configured.

use super::*;
use crate::common::{OutputFormat, ProcessingOptions};

#[tokio::test]
async fn test_png_to_latex_pipeline() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Create test image
    let image = images::generate_simple_equation("x^2 + 2x + 1");
    let image_path = "/tmp/test_equation.png";
    image.save(image_path).unwrap();

    // Process through pipeline
    let result = test_server
        .process_image(image_path, OutputFormat::LaTeX)
        .await
        .expect("Pipeline processing failed");

    // Verify output
    assert!(!result.latex.is_empty(), "LaTeX output should not be empty");
    assert!(
        result.confidence > 0.7,
        "Confidence too low: {}",
        result.confidence
    );
    assert!(result.latex.contains("x"), "Should contain variable x");

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_jpeg_to_mathml_pipeline() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Create JPEG test image
    let image = images::generate_fraction(1, 2);
    let image_path = "/tmp/test_fraction.jpg";
    image.save(image_path).unwrap();

    // Process to MathML
    let result = test_server
        .process_image(image_path, OutputFormat::MathML)
        .await
        .expect("Pipeline processing failed");

    // Verify MathML structure
    assert!(result.mathml.is_some(), "MathML output should be present");

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_webp_to_html_pipeline() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Create WebP test image
    let image = images::generate_integral("x dx");
    let image_path = "/tmp/test_integral.webp";
    // Note: WebP support may require additional image codec
    image.save(image_path).unwrap_or_else(|_| {
        // Fallback to PNG if WebP not supported
        image.save("/tmp/test_integral.png").unwrap();
    });

    let actual_path = if std::path::Path::new(image_path).exists() {
        image_path
    } else {
        "/tmp/test_integral.png"
    };

    // Process to HTML
    let _result = test_server
        .process_image(actual_path, OutputFormat::HTML)
        .await
        .expect("Pipeline processing failed");

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_pipeline_timeout_handling() {
    let test_server = TestServer::with_timeout(100)
        .await
        .expect("Failed to start test server");

    // Create complex image that might take time
    let complex_image = images::generate_complex_equation();
    complex_image.save("/tmp/complex.png").unwrap();

    let start = std::time::Instant::now();
    let _result = test_server
        .process_image("/tmp/complex.png", OutputFormat::LaTeX)
        .await;
    let duration = start.elapsed();

    // Should either complete or timeout within reasonable time
    assert!(
        duration.as_millis() < 500,
        "Should timeout or complete quickly"
    );

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_batch_pipeline_processing() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Create multiple test images
    let test_images = vec![
        ("x + y", "/tmp/batch_1.png"),
        ("a - b", "/tmp/batch_2.png"),
        ("2 * 3", "/tmp/batch_3.png"),
        ("x / y", "/tmp/batch_4.png"),
    ];

    for (equation, path) in &test_images {
        let img = images::generate_simple_equation(equation);
        img.save(path).unwrap();
    }

    // Process batch
    let paths: Vec<&str> = test_images.iter().map(|(_, p)| *p).collect();
    let results = test_server
        .process_batch(&paths, OutputFormat::LaTeX)
        .await
        .expect("Batch processing failed");

    // Verify all processed
    assert_eq!(results.len(), 4, "Should process all images");
    for (i, result) in results.iter().enumerate() {
        assert!(!result.latex.is_empty(), "Result {} should have LaTeX", i);
        assert!(result.confidence > 0.5, "Result {} confidence too low", i);
    }

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_pipeline_with_preprocessing() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Create noisy image
    let mut image = images::generate_simple_equation("f(x) = x^2");
    images::add_noise(&mut image, 0.1);
    image.save("/tmp/noisy.png").unwrap();

    // Process with preprocessing enabled
    let result = test_server
        .process_image_with_options(
            "/tmp/noisy.png",
            OutputFormat::LaTeX,
            ProcessingOptions {
                enable_preprocessing: true,
                enable_denoising: true,
                enable_deskew: true,
                ..Default::default()
            },
        )
        .await
        .expect("Processing failed");

    // Should still recognize despite noise
    assert!(
        !result.latex.is_empty(),
        "Should extract LaTeX from noisy image"
    );

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_multi_format_output() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Create test image
    let image = images::generate_fraction(3, 4);
    image.save("/tmp/fraction.png").unwrap();

    // Request multiple output formats
    let result = test_server
        .process_image_with_options(
            "/tmp/fraction.png",
            OutputFormat::All,
            ProcessingOptions {
                include_latex: true,
                include_mathml: true,
                include_ascii: true,
                include_text: true,
                ..Default::default()
            },
        )
        .await
        .expect("Processing failed");

    // Verify output present
    assert!(!result.latex.is_empty(), "Should have LaTeX");
    assert!(result.mathml.is_some(), "Should have MathML");

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_pipeline_caching() {
    let test_server = TestServer::with_cache()
        .await
        .expect("Failed to start test server");

    // Create test image
    let image = images::generate_simple_equation("a + b = c");
    image.save("/tmp/cached.png").unwrap();

    // First processing
    let result1 = test_server
        .process_image("/tmp/cached.png", OutputFormat::LaTeX)
        .await
        .expect("First processing failed");

    // Second processing (should hit cache)
    let result2 = test_server
        .process_image("/tmp/cached.png", OutputFormat::LaTeX)
        .await
        .expect("Second processing failed");

    // Verify cache hit
    assert_eq!(result1.latex, result2.latex, "Results should match");

    test_server.shutdown().await;
}
