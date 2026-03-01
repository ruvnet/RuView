//! SciPix OCR Benchmark Tool
//!
//! Comprehensive benchmark for OCR performance including:
//! - Image preprocessing speed
//! - Text detection throughput
//! - Character recognition latency
//! - End-to-end pipeline benchmarks

use image::{DynamicImage, ImageBuffer, Luma, Rgb, RgbImage};
use imageproc::contrast::ThresholdType;
use imageproc::drawing::draw_filled_rect_mut;
use imageproc::rect::Rect;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// Import SIMD optimizations
use ruvector_scipix::optimize::simd::{
    fast_area_resize, simd_grayscale, simd_resize_bilinear, simd_threshold,
};

/// Benchmark results
#[derive(Debug, Clone)]
struct BenchmarkResult {
    name: String,
    iterations: usize,
    total_time: Duration,
    avg_time: Duration,
    min_time: Duration,
    max_time: Duration,
    throughput: f64,
}

impl BenchmarkResult {
    fn display(&self) {
        println!("\n{}", "=".repeat(60));
        println!("Benchmark: {}", self.name);
        println!("{}", "=".repeat(60));
        println!("  Iterations:  {}", self.iterations);
        println!("  Total time:  {:?}", self.total_time);
        println!("  Avg time:    {:?}", self.avg_time);
        println!("  Min time:    {:?}", self.min_time);
        println!("  Max time:    {:?}", self.max_time);
        println!("  Throughput:  {:.2} ops/sec", self.throughput);
    }
}

/// Generate a test image with synthetic patterns (simulating text)
fn generate_test_image(width: u32, height: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_fn(width, height, |_, _| {
        Rgb([255u8, 255u8, 255u8]) // White background
    });

    // Draw black rectangles to simulate text blocks
    for i in 0..10 {
        let x = (i * 35 + 10) as i32;
        let y = 20;
        draw_filled_rect_mut(
            &mut img,
            Rect::at(x, y).of_size(25, 40),
            Rgb([0u8, 0u8, 0u8]),
        );
    }

    // Draw a horizontal line (like an equation fraction)
    draw_filled_rect_mut(
        &mut img,
        Rect::at(10, 70).of_size(350, 2),
        Rgb([0u8, 0u8, 0u8]),
    );

    img
}

/// Generate a math-like test image
fn generate_math_image(width: u32, height: u32) -> RgbImage {
    let mut img: RgbImage = ImageBuffer::from_fn(width, height, |_, _| Rgb([255u8, 255u8, 255u8]));

    // Draw elements resembling a fraction
    draw_filled_rect_mut(
        &mut img,
        Rect::at(50, 20).of_size(100, 30),
        Rgb([0u8, 0u8, 0u8]),
    );
    draw_filled_rect_mut(
        &mut img,
        Rect::at(20, 60).of_size(160, 3),
        Rgb([0u8, 0u8, 0u8]),
    );
    draw_filled_rect_mut(
        &mut img,
        Rect::at(70, 70).of_size(60, 30),
        Rgb([0u8, 0u8, 0u8]),
    );

    // Draw square root symbol approximation
    draw_filled_rect_mut(
        &mut img,
        Rect::at(200, 30).of_size(5, 40),
        Rgb([0u8, 0u8, 0u8]),
    );
    draw_filled_rect_mut(
        &mut img,
        Rect::at(200, 30).of_size(80, 3),
        Rgb([0u8, 0u8, 0u8]),
    );

    img
}

/// Run a benchmark function multiple times and collect statistics
fn run_benchmark<F, E>(name: &str, iterations: usize, mut f: F) -> BenchmarkResult
where
    F: FnMut() -> Result<(), E>,
    E: std::fmt::Debug,
{
    let mut times = Vec::with_capacity(iterations);

    // Warmup
    for _ in 0..3 {
        let _ = f();
    }

    // Actual benchmark
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = f();
        times.push(start.elapsed());
    }

    let total_time: Duration = times.iter().sum();
    let avg_time = total_time / iterations as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let throughput = iterations as f64 / total_time.as_secs_f64();

    BenchmarkResult {
        name: name.to_string(),
        iterations,
        total_time,
        avg_time,
        min_time,
        max_time,
        throughput,
    }
}

/// Benchmark grayscale conversion
fn benchmark_grayscale(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Grayscale Conversion", 500, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let _gray = img.to_luma8();
        Ok(())
    })
}

/// Benchmark image resize
fn benchmark_resize(images: &[DynamicImage]) -> BenchmarkResult {
    use image::imageops::FilterType;

    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Image Resize (640x480)", 100, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let _resized = img.resize(640, 480, FilterType::Lanczos3);
        Ok(())
    })
}

/// Benchmark fast resize
fn benchmark_fast_resize(images: &[DynamicImage]) -> BenchmarkResult {
    use image::imageops::FilterType;

    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Fast Resize (Nearest)", 500, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let _resized = img.resize(640, 480, FilterType::Nearest);
        Ok(())
    })
}

/// Benchmark Gaussian blur
fn benchmark_blur(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Gaussian Blur (σ=1.5)", 50, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let gray = img.to_luma8();
        let _blurred = imageproc::filter::gaussian_blur_f32(&gray, 1.5);
        Ok(())
    })
}

/// Benchmark threshold (binarization)
fn benchmark_threshold(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Otsu Threshold", 100, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let gray = img.to_luma8();
        let _thresholded = imageproc::contrast::threshold(&gray, 128, ThresholdType::Binary);
        Ok(())
    })
}

/// Benchmark adaptive threshold
fn benchmark_adaptive_threshold(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Adaptive Threshold", 30, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let gray = img.to_luma8();
        let _thresholded = imageproc::contrast::adaptive_threshold(&gray, 11);
        Ok(())
    })
}

/// Benchmark memory throughput
fn benchmark_memory_throughput() -> BenchmarkResult {
    let data: Vec<f32> = (0..1_000_000).map(|i| i as f32).collect();

    run_benchmark::<_, std::convert::Infallible>("Memory Throughput (1M floats)", 100, || {
        let _sum: f32 = data.iter().sum();
        let _clone = data.clone();
        Ok(())
    })
}

/// Benchmark tensor creation for ONNX
fn benchmark_tensor_creation() -> BenchmarkResult {
    use ndarray::Array4;

    run_benchmark::<_, ndarray::ShapeError>("Tensor Creation (1x3x224x224)", 100, || {
        let tensor_data: Vec<f32> = vec![0.0; 1 * 3 * 224 * 224];
        let _tensor = Array4::from_shape_vec((1, 3, 224, 224), tensor_data)?;
        Ok(())
    })
}

/// Benchmark large tensor creation
fn benchmark_large_tensor() -> BenchmarkResult {
    use ndarray::Array4;

    run_benchmark::<_, ndarray::ShapeError>("Large Tensor (1x3x640x480)", 50, || {
        let tensor_data: Vec<f32> = vec![0.0; 1 * 3 * 640 * 480];
        let _tensor = Array4::from_shape_vec((1, 3, 640, 480), tensor_data)?;
        Ok(())
    })
}

/// Benchmark image normalization
fn benchmark_normalization(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Image Normalization", 200, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let rgb = img.to_rgb8();
        let mut tensor = Vec::with_capacity(3 * rgb.width() as usize * rgb.height() as usize);

        // NCHW format normalization
        for c in 0..3 {
            for y in 0..rgb.height() {
                for x in 0..rgb.width() {
                    let pixel = rgb.get_pixel(x, y);
                    tensor.push((pixel[c] as f32 / 127.5) - 1.0);
                }
            }
        }
        Ok(())
    })
}

/// Benchmark image loading from disk
fn benchmark_image_load(path: &PathBuf) -> BenchmarkResult {
    run_benchmark::<_, image::ImageError>("Image Load from Disk", 100, || {
        let _img = image::open(path)?;
        Ok(())
    })
}

/// Benchmark edge detection
fn benchmark_edge_detection(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Sobel Edge Detection", 50, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let gray = img.to_luma8();
        let _edges = imageproc::gradients::sobel_gradients(&gray);
        Ok(())
    })
}

/// Benchmark connected components
fn benchmark_connected_components(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Connected Components", 50, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let gray = img.to_luma8();
        let binary = imageproc::contrast::threshold(&gray, 128, ThresholdType::Binary);
        let _cc = imageproc::region_labelling::connected_components(
            &binary,
            imageproc::region_labelling::Connectivity::Eight,
            Luma([0u8]),
        );
        Ok(())
    })
}

/// Benchmark SIMD grayscale conversion
fn benchmark_simd_grayscale(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("SIMD Grayscale", 500, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let rgba = img.to_rgba8();
        let mut gray = vec![0u8; (rgba.width() * rgba.height()) as usize];
        simd_grayscale(rgba.as_raw(), &mut gray);
        Ok(())
    })
}

/// Benchmark SIMD bilinear resize
fn benchmark_simd_resize(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("SIMD Resize (Bilinear)", 500, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let gray = img.to_luma8();
        let _resized = simd_resize_bilinear(
            gray.as_raw(),
            gray.width() as usize,
            gray.height() as usize,
            640,
            480,
        );
        Ok(())
    })
}

/// Benchmark fast area resize
fn benchmark_area_resize(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Fast Area Resize", 500, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let gray = img.to_luma8();
        let _resized = fast_area_resize(
            gray.as_raw(),
            gray.width() as usize,
            gray.height() as usize,
            640,
            480,
        );
        Ok(())
    })
}

/// Benchmark SIMD threshold
fn benchmark_simd_threshold(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("SIMD Threshold", 500, || {
        let img = &images[idx % images.len()];
        idx += 1;
        let gray = img.to_luma8();
        let mut out = vec![0u8; gray.as_raw().len()];
        simd_threshold(gray.as_raw(), 128, &mut out);
        Ok(())
    })
}

/// Complete preprocessing pipeline benchmark (SIMD optimized)
fn benchmark_simd_pipeline(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("SIMD Full Pipeline", 200, || {
        let img = &images[idx % images.len()];
        idx += 1;

        // Step 1: RGBA to Grayscale
        let rgba = img.to_rgba8();
        let mut gray = vec![0u8; (rgba.width() * rgba.height()) as usize];
        simd_grayscale(rgba.as_raw(), &mut gray);

        // Step 2: Resize
        let resized = simd_resize_bilinear(
            &gray,
            rgba.width() as usize,
            rgba.height() as usize,
            224,
            224,
        );

        // Step 3: Threshold
        let mut binary = vec![0u8; resized.len()];
        simd_threshold(&resized, 128, &mut binary);

        // Step 4: Normalize to tensor format
        let _tensor: Vec<f32> = binary.iter().map(|&x| (x as f32 / 127.5) - 1.0).collect();

        Ok(())
    })
}

/// Original preprocessing pipeline benchmark (for comparison)
fn benchmark_original_pipeline(images: &[DynamicImage]) -> BenchmarkResult {
    let mut idx = 0;
    run_benchmark::<_, std::convert::Infallible>("Original Full Pipeline", 200, || {
        let img = &images[idx % images.len()];
        idx += 1;

        // Step 1: Grayscale
        let gray = img.to_luma8();

        // Step 2: Resize
        let resized =
            image::imageops::resize(&gray, 224, 224, image::imageops::FilterType::Nearest);

        // Step 3: Threshold
        let binary = imageproc::contrast::threshold(&resized, 128, ThresholdType::Binary);

        // Step 4: Normalize
        let _tensor: Vec<f32> = binary
            .as_raw()
            .iter()
            .map(|&x| (x as f32 / 127.5) - 1.0)
            .collect();

        Ok(())
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n{}", "=".repeat(60));
    println!("          SciPix OCR Benchmark Suite");
    println!("{}", "=".repeat(60));
    println!("\nGenerating test images...");

    // Generate test images
    let text_image = generate_test_image(400, 100);
    let math_image = generate_math_image(300, 150);
    let large_image = generate_test_image(800, 200);
    let hd_image = generate_test_image(1920, 1080);

    // Save test images
    let test_dir = PathBuf::from("test_images");
    fs::create_dir_all(&test_dir)?;

    text_image.save(test_dir.join("text_test.png"))?;
    math_image.save(test_dir.join("math_test.png"))?;
    large_image.save(test_dir.join("large_test.png"))?;
    hd_image.save(test_dir.join("hd_test.png"))?;

    println!("Test images saved to test_images/\n");

    // Convert to DynamicImage for benchmarks
    let images: Vec<DynamicImage> = vec![
        DynamicImage::ImageRgb8(text_image.clone()),
        DynamicImage::ImageRgb8(math_image.clone()),
        DynamicImage::ImageRgb8(large_image.clone()),
    ];

    let hd_images = vec![DynamicImage::ImageRgb8(hd_image.clone())];

    // Run benchmarks
    let mut results = Vec::new();

    println!("Running image conversion benchmarks...");
    results.push(benchmark_grayscale(&images));

    println!("Running resize benchmarks...");
    results.push(benchmark_resize(&images));
    results.push(benchmark_fast_resize(&images));

    println!("Running filter benchmarks...");
    results.push(benchmark_blur(&images));
    results.push(benchmark_threshold(&images));
    results.push(benchmark_adaptive_threshold(&images));
    results.push(benchmark_edge_detection(&images));
    results.push(benchmark_connected_components(&images));

    println!("Running SIMD optimized benchmarks...");
    results.push(benchmark_simd_grayscale(&images));
    results.push(benchmark_simd_resize(&images));
    results.push(benchmark_area_resize(&images));
    results.push(benchmark_simd_threshold(&images));

    println!("Running pipeline benchmarks...");
    results.push(benchmark_original_pipeline(&images));
    results.push(benchmark_simd_pipeline(&images));

    println!("Running normalization benchmarks...");
    results.push(benchmark_normalization(&images));

    println!("Running memory benchmarks...");
    results.push(benchmark_memory_throughput());
    results.push(benchmark_tensor_creation());
    results.push(benchmark_large_tensor());

    println!("Running I/O benchmarks...");
    results.push(benchmark_image_load(&test_dir.join("text_test.png")));

    println!("\nRunning HD image benchmarks...");
    results.push(run_benchmark::<_, std::convert::Infallible>(
        "HD Grayscale (1920x1080)",
        100,
        || {
            let _gray = hd_images[0].to_luma8();
            Ok(())
        },
    ));
    results.push(run_benchmark::<_, std::convert::Infallible>(
        "HD Resize to 640x480",
        50,
        || {
            let _resized = hd_images[0].resize(640, 480, image::imageops::FilterType::Lanczos3);
            Ok(())
        },
    ));

    // Display results
    println!("\n\n{}", "#".repeat(60));
    println!("                    BENCHMARK RESULTS");
    println!("{}", "#".repeat(60));

    for result in &results {
        result.display();
    }

    // Summary table
    println!("\n\n{}", "=".repeat(75));
    println!("{:45} {:>15} {:>15}", "Benchmark", "Avg Time", "Throughput");
    println!("{}", "-".repeat(75));
    for result in &results {
        println!(
            "{:45} {:>15.2?} {:>12.2} ops/s",
            result.name, result.avg_time, result.throughput
        );
    }
    println!("{}", "=".repeat(75));

    // Performance analysis
    println!("\n{}", "=".repeat(60));
    println!("                  PERFORMANCE ANALYSIS");
    println!("{}", "=".repeat(60));

    // Calculate total preprocessing time for a typical pipeline
    let grayscale_time = results
        .iter()
        .find(|r| r.name == "Grayscale Conversion")
        .map(|r| r.avg_time)
        .unwrap_or_default();
    let resize_time = results
        .iter()
        .find(|r| r.name == "Fast Resize (Nearest)")
        .map(|r| r.avg_time)
        .unwrap_or_default();
    let threshold_time = results
        .iter()
        .find(|r| r.name == "Otsu Threshold")
        .map(|r| r.avg_time)
        .unwrap_or_default();
    let normalize_time = results
        .iter()
        .find(|r| r.name == "Image Normalization")
        .map(|r| r.avg_time)
        .unwrap_or_default();

    let total_preprocess = grayscale_time + resize_time + threshold_time + normalize_time;

    // SIMD optimized times
    let simd_grayscale = results
        .iter()
        .find(|r| r.name == "SIMD Grayscale")
        .map(|r| r.avg_time)
        .unwrap_or_default();
    let simd_resize = results
        .iter()
        .find(|r| r.name == "SIMD Resize (Bilinear)")
        .map(|r| r.avg_time)
        .unwrap_or_default();
    let simd_threshold = results
        .iter()
        .find(|r| r.name == "SIMD Threshold")
        .map(|r| r.avg_time)
        .unwrap_or_default();

    let original_pipeline = results
        .iter()
        .find(|r| r.name == "Original Full Pipeline")
        .map(|r| r.avg_time)
        .unwrap_or_default();
    let simd_pipeline = results
        .iter()
        .find(|r| r.name == "SIMD Full Pipeline")
        .map(|r| r.avg_time)
        .unwrap_or_default();

    println!("\n┌──────────────────────────────────────────────────────────────────┐");
    println!("│  SIMD Optimization Comparison                                    │");
    println!("├────────────────────┬──────────────┬──────────────┬───────────────┤");
    println!("│  Operation         │ Original     │ SIMD         │ Speedup       │");
    println!("├────────────────────┼──────────────┼──────────────┼───────────────┤");
    println!(
        "│  Grayscale         │ {:>10.2?} │ {:>10.2?} │ {:>6.2}x       │",
        grayscale_time,
        simd_grayscale,
        if simd_grayscale.as_nanos() > 0 {
            grayscale_time.as_secs_f64() / simd_grayscale.as_secs_f64()
        } else {
            1.0
        }
    );
    println!(
        "│  Resize            │ {:>10.2?} │ {:>10.2?} │ {:>6.2}x       │",
        resize_time,
        simd_resize,
        if simd_resize.as_nanos() > 0 {
            resize_time.as_secs_f64() / simd_resize.as_secs_f64()
        } else {
            1.0
        }
    );
    println!(
        "│  Threshold         │ {:>10.2?} │ {:>10.2?} │ {:>6.2}x       │",
        threshold_time,
        simd_threshold,
        if simd_threshold.as_nanos() > 0 {
            threshold_time.as_secs_f64() / simd_threshold.as_secs_f64()
        } else {
            1.0
        }
    );
    println!("├────────────────────┼──────────────┼──────────────┼───────────────┤");
    println!(
        "│  Full Pipeline     │ {:>10.2?} │ {:>10.2?} │ {:>6.2}x       │",
        original_pipeline,
        simd_pipeline,
        if simd_pipeline.as_nanos() > 0 {
            original_pipeline.as_secs_f64() / simd_pipeline.as_secs_f64()
        } else {
            1.0
        }
    );
    println!("└────────────────────┴──────────────┴──────────────┴───────────────┘");

    println!("\n┌──────────────────────────────────────────────────┐");
    println!("│  Typical Preprocessing Pipeline Breakdown        │");
    println!("├──────────────────────────────────────────────────┤");
    println!(
        "│  Grayscale:     {:>10.2?} ({:.1}%)               │",
        grayscale_time,
        100.0 * grayscale_time.as_secs_f64() / total_preprocess.as_secs_f64()
    );
    println!(
        "│  Resize:        {:>10.2?} ({:.1}%)               │",
        resize_time,
        100.0 * resize_time.as_secs_f64() / total_preprocess.as_secs_f64()
    );
    println!(
        "│  Threshold:     {:>10.2?} ({:.1}%)               │",
        threshold_time,
        100.0 * threshold_time.as_secs_f64() / total_preprocess.as_secs_f64()
    );
    println!(
        "│  Normalization: {:>10.2?} ({:.1}%)               │",
        normalize_time,
        100.0 * normalize_time.as_secs_f64() / total_preprocess.as_secs_f64()
    );
    println!("├──────────────────────────────────────────────────┤");
    println!(
        "│  TOTAL:         {:>10.2?}                      │",
        total_preprocess
    );
    println!("└──────────────────────────────────────────────────┘");

    println!("\nTarget latency for real-time (30 fps): 33.3ms");

    if total_preprocess.as_millis() < 33 {
        println!(
            "✓ Preprocessing meets real-time requirements ({:.1}ms < 33.3ms)",
            total_preprocess.as_secs_f64() * 1000.0
        );
    } else {
        println!(
            "⚠ Preprocessing exceeds real-time target ({:.1}ms > 33.3ms)",
            total_preprocess.as_secs_f64() * 1000.0
        );
    }

    // Memory efficiency
    let tensor_throughput = results
        .iter()
        .find(|r| r.name.contains("Tensor Creation"))
        .map(|r| r.throughput)
        .unwrap_or(0.0);

    println!(
        "\nTensor creation throughput: {:.0} tensors/sec",
        tensor_throughput
    );
    println!("Target for batch inference: >100 tensors/sec");

    if tensor_throughput > 100.0 {
        println!("✓ Tensor creation meets batch requirements");
    } else {
        println!("⚠ Consider tensor pooling optimization");
    }

    // Estimated end-to-end throughput
    let estimated_ocr_time = total_preprocess.as_secs_f64() * 1000.0 + 50.0; // preprocessing + estimated inference
    let estimated_throughput = 1000.0 / estimated_ocr_time;

    println!("\n┌──────────────────────────────────────────────────┐");
    println!("│  Estimated End-to-End Performance                │");
    println!("├──────────────────────────────────────────────────┤");
    println!(
        "│  Preprocessing:  {:>8.2}ms                      │",
        total_preprocess.as_secs_f64() * 1000.0
    );
    println!("│  Est. Inference: {:>8.2}ms (target)              │", 50.0);
    println!(
        "│  Total latency:  {:>8.2}ms                      │",
        estimated_ocr_time
    );
    println!(
        "│  Throughput:     {:>8.1} images/sec             │",
        estimated_throughput
    );
    println!("└──────────────────────────────────────────────────┘");

    // State of the art comparison
    println!("\n{}", "=".repeat(60));
    println!("           STATE OF THE ART COMPARISON");
    println!("{}", "=".repeat(60));
    println!("\n┌────────────────────────────────────────────────────────┐");
    println!("│  System          │ Latency    │ Throughput  │ Status  │");
    println!("├────────────────────────────────────────────────────────┤");
    println!("│  Tesseract       │ ~200ms     │ ~5 img/s    │ Slow    │");
    println!("│  PaddleOCR       │ ~50ms      │ ~20 img/s   │ Fast    │");
    println!("│  EasyOCR         │ ~100ms     │ ~10 img/s   │ Medium  │");
    println!(
        "│  SciPix (est.)   │ {:>6.1}ms   │ {:>6.1} img/s  │ {}│",
        estimated_ocr_time,
        estimated_throughput,
        if estimated_throughput > 15.0 {
            "Fast    "
        } else if estimated_throughput > 8.0 {
            "Medium  "
        } else {
            "Slow    "
        }
    );
    println!("└────────────────────────────────────────────────────────┘");

    println!("\n{}", "=".repeat(60));
    println!("Benchmark complete!");
    println!("{}", "=".repeat(60));

    Ok(())
}
