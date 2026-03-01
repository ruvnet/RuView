use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

/// Benchmark text detection model inference
fn bench_text_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_detection_model");
    group.measurement_time(Duration::from_secs(10));

    let sizes = [(224, 224), (384, 384), (512, 512)];

    for (w, h) in sizes {
        let input_tensor = create_input_tensor(w, h, 3);

        group.bench_with_input(
            BenchmarkId::new("inference", format!("{}x{}", w, h)),
            &input_tensor,
            |b, tensor| {
                b.iter(|| black_box(run_detection_model(black_box(tensor))));
            },
        );
    }

    group.finish();
}

/// Benchmark text recognition model inference
fn bench_text_recognition(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_recognition_model");
    group.measurement_time(Duration::from_secs(10));

    // Recognition typically works on smaller cropped regions
    let sizes = [(32, 128), (48, 192), (64, 256)];

    for (h, w) in sizes {
        let input_tensor = create_input_tensor(w, h, 1);

        group.bench_with_input(
            BenchmarkId::new("inference", format!("{}x{}", w, h)),
            &input_tensor,
            |b, tensor| {
                b.iter(|| black_box(run_recognition_model(black_box(tensor))));
            },
        );
    }

    group.finish();
}

/// Benchmark math equation model inference
fn bench_math_model(c: &mut Criterion) {
    let mut group = c.benchmark_group("math_model");
    group.measurement_time(Duration::from_secs(10));

    let sizes = [(224, 224), (320, 320), (384, 384)];

    for (w, h) in sizes {
        let input_tensor = create_input_tensor(w, h, 3);

        group.bench_with_input(
            BenchmarkId::new("inference", format!("{}x{}", w, h)),
            &input_tensor,
            |b, tensor| {
                b.iter(|| black_box(run_math_model(black_box(tensor))));
            },
        );
    }

    group.finish();
}

/// Benchmark tensor preprocessing operations
fn bench_tensor_preprocessing(c: &mut Criterion) {
    let mut group = c.benchmark_group("tensor_preprocessing");
    group.measurement_time(Duration::from_secs(8));

    let image_data = vec![128u8; 384 * 384 * 3];

    group.bench_function("normalization", |b| {
        b.iter(|| black_box(normalize_tensor(black_box(&image_data))));
    });

    group.bench_function("standardization", |b| {
        b.iter(|| black_box(standardize_tensor(black_box(&image_data))));
    });

    group.bench_function("to_chw_layout", |b| {
        b.iter(|| black_box(convert_to_chw(black_box(&image_data), 384, 384)));
    });

    group.bench_function("add_batch_dimension", |b| {
        let tensor = normalize_tensor(&image_data);
        b.iter(|| black_box(add_batch_dim(black_box(&tensor))));
    });

    group.finish();
}

/// Benchmark output postprocessing
fn bench_output_postprocessing(c: &mut Criterion) {
    let mut group = c.benchmark_group("output_postprocessing");
    group.measurement_time(Duration::from_secs(8));

    let detection_output = create_detection_output(1000);
    let recognition_output = create_recognition_output(100);

    group.bench_function("nms_filtering", |b| {
        b.iter(|| black_box(apply_nms(black_box(&detection_output), 0.5)));
    });

    group.bench_function("confidence_filtering", |b| {
        b.iter(|| black_box(filter_by_confidence(black_box(&detection_output), 0.7)));
    });

    group.bench_function("decode_sequence", |b| {
        b.iter(|| black_box(decode_ctc_output(black_box(&recognition_output))));
    });

    group.bench_function("beam_search", |b| {
        b.iter(|| black_box(beam_search_decode(black_box(&recognition_output), 5)));
    });

    group.finish();
}

/// Benchmark batch inference
fn bench_batch_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_inference");
    group.measurement_time(Duration::from_secs(15));

    let batch_sizes = [1, 4, 8, 16];
    let size = (384, 384);

    for batch_size in batch_sizes {
        let batch_tensor = create_batch_tensor(batch_size, size.0, size.1, 3);

        group.bench_with_input(
            BenchmarkId::new("detection_batch", batch_size),
            &batch_tensor,
            |b, tensor| {
                b.iter(|| black_box(run_detection_model(black_box(tensor))));
            },
        );
    }

    group.finish();
}

/// Benchmark model warm-up time
fn bench_model_warmup(c: &mut Criterion) {
    let mut group = c.benchmark_group("model_warmup");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("detection_model_init", |b| {
        b.iter_with_large_drop(|| black_box(initialize_detection_model()));
    });

    group.bench_function("recognition_model_init", |b| {
        b.iter_with_large_drop(|| black_box(initialize_recognition_model()));
    });

    group.bench_function("math_model_init", |b| {
        b.iter_with_large_drop(|| black_box(initialize_math_model()));
    });

    group.finish();
}

/// Benchmark end-to-end inference pipeline
fn bench_e2e_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_inference_pipeline");
    group.measurement_time(Duration::from_secs(15));

    let image_data = vec![128u8; 384 * 384 * 3];

    group.bench_function("full_pipeline", |b| {
        b.iter(|| {
            // Preprocessing
            let normalized = normalize_tensor(black_box(&image_data));
            let chw = convert_to_chw(&normalized, 384, 384);
            let batched = add_batch_dim(&chw);

            // Detection
            let detection_output = run_detection_model(&batched);
            let boxes = apply_nms(&detection_output, 0.5);

            // Recognition (simulated for each box)
            let mut results = Vec::new();
            for _box in boxes.iter().take(5) {
                let rec_output = run_recognition_model(&batched);
                let text = decode_ctc_output(&rec_output);
                results.push(text);
            }

            black_box(results)
        });
    });

    group.finish();
}

// Mock implementations

fn create_input_tensor(width: u32, height: u32, channels: u32) -> Vec<f32> {
    vec![0.5f32; (width * height * channels) as usize]
}

fn create_batch_tensor(batch: usize, width: u32, height: u32, channels: u32) -> Vec<f32> {
    vec![0.5f32; batch * (width * height * channels) as usize]
}

fn run_detection_model(input: &[f32]) -> Vec<Detection> {
    // Simulate model inference
    let output_size = input.len() / 100;
    (0..output_size)
        .map(|i| Detection {
            bbox: [i as f32, i as f32, (i + 10) as f32, (i + 10) as f32],
            confidence: 0.8 + (i % 20) as f32 / 100.0,
            class_id: i % 10,
        })
        .collect()
}

fn run_recognition_model(input: &[f32]) -> Vec<f32> {
    // Simulate CTC output: [time_steps, vocab_size]
    let time_steps = 32;
    let vocab_size = 64;
    vec![0.1f32; time_steps * vocab_size]
}

fn run_math_model(input: &[f32]) -> Vec<f32> {
    // Simulate math model output
    vec![0.5f32; input.len() / 10]
}

fn initialize_detection_model() -> Vec<u8> {
    std::thread::sleep(Duration::from_millis(100));
    vec![0u8; 1024 * 1024]
}

fn initialize_recognition_model() -> Vec<u8> {
    std::thread::sleep(Duration::from_millis(80));
    vec![0u8; 512 * 1024]
}

fn initialize_math_model() -> Vec<u8> {
    std::thread::sleep(Duration::from_millis(120));
    vec![0u8; 2048 * 1024]
}

fn normalize_tensor(data: &[u8]) -> Vec<f32> {
    data.iter().map(|&x| x as f32 / 255.0).collect()
}

fn standardize_tensor(data: &[u8]) -> Vec<f32> {
    let mean = 128.0f32;
    let std = 64.0f32;
    data.iter().map(|&x| (x as f32 - mean) / std).collect()
}

fn convert_to_chw(data: &[f32], width: u32, height: u32) -> Vec<f32> {
    // Convert HWC to CHW layout
    let channels = data.len() / (width * height) as usize;
    let mut chw = Vec::with_capacity(data.len());

    for c in 0..channels {
        for h in 0..height {
            for w in 0..width {
                let hwc_idx = ((h * width + w) * channels as u32 + c as u32) as usize;
                chw.push(data[hwc_idx]);
            }
        }
    }

    chw
}

fn add_batch_dim(tensor: &[f32]) -> Vec<f32> {
    tensor.to_vec()
}

#[derive(Clone)]
struct Detection {
    bbox: [f32; 4],
    confidence: f32,
    class_id: usize,
}

fn create_detection_output(count: usize) -> Vec<Detection> {
    (0..count)
        .map(|i| Detection {
            bbox: [i as f32, i as f32, (i + 10) as f32, (i + 10) as f32],
            confidence: 0.5 + (i % 50) as f32 / 100.0,
            class_id: i % 10,
        })
        .collect()
}

fn create_recognition_output(time_steps: usize) -> Vec<f32> {
    vec![0.1f32; time_steps * 64]
}

fn apply_nms(detections: &[Detection], iou_threshold: f32) -> Vec<Detection> {
    let mut filtered = Vec::new();
    let mut sorted = detections.to_vec();
    sorted.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

    for det in sorted {
        let overlap = filtered
            .iter()
            .any(|kept: &Detection| calculate_iou(&det.bbox, &kept.bbox) > iou_threshold);

        if !overlap {
            filtered.push(det);
        }
    }

    filtered
}

fn calculate_iou(box1: &[f32; 4], box2: &[f32; 4]) -> f32 {
    let x1 = box1[0].max(box2[0]);
    let y1 = box1[1].max(box2[1]);
    let x2 = box1[2].min(box2[2]);
    let y2 = box1[3].min(box2[3]);

    let intersection = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let area1 = (box1[2] - box1[0]) * (box1[3] - box1[1]);
    let area2 = (box2[2] - box2[0]) * (box2[3] - box2[1]);
    let union = area1 + area2 - intersection;

    if union > 0.0 {
        intersection / union
    } else {
        0.0
    }
}

fn filter_by_confidence(detections: &[Detection], threshold: f32) -> Vec<Detection> {
    detections
        .iter()
        .filter(|d| d.confidence >= threshold)
        .cloned()
        .collect()
}

fn decode_ctc_output(logits: &[f32]) -> String {
    // Simple greedy CTC decoding
    let time_steps = logits.len() / 64;
    let mut result = String::new();
    let mut prev_char = None;

    for t in 0..time_steps {
        let start_idx = t * 64;
        let end_idx = start_idx + 64;
        let step_logits = &logits[start_idx..end_idx];

        let (max_idx, _) = step_logits
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        if max_idx > 0 && Some(max_idx) != prev_char {
            result.push((b'a' + max_idx as u8 % 26) as char);
        }

        prev_char = Some(max_idx);
    }

    result
}

fn beam_search_decode(logits: &[f32], beam_width: usize) -> String {
    // Simplified beam search
    let time_steps = logits.len() / 64;
    let mut beams: Vec<(String, f32)> = vec![(String::new(), 0.0)];

    for t in 0..time_steps {
        let start_idx = t * 64;
        let end_idx = start_idx + 64;
        let step_logits = &logits[start_idx..end_idx];

        let mut new_beams = Vec::new();

        for (text, score) in &beams {
            for (char_idx, &logit) in step_logits.iter().enumerate().take(beam_width) {
                let mut new_text = text.clone();
                if char_idx > 0 {
                    new_text.push((b'a' + char_idx as u8 % 26) as char);
                }
                new_beams.push((new_text, score + logit));
            }
        }

        new_beams.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        beams = new_beams.into_iter().take(beam_width).collect();
    }

    beams[0].0.clone()
}

criterion_group!(
    benches,
    bench_text_detection,
    bench_text_recognition,
    bench_math_model,
    bench_tensor_preprocessing,
    bench_output_postprocessing,
    bench_batch_inference,
    bench_model_warmup,
    bench_e2e_pipeline
);
criterion_main!(benches);
