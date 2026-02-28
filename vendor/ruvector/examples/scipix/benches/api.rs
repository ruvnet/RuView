use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

/// Benchmark API request parsing
fn bench_request_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_parsing");
    group.measurement_time(Duration::from_secs(5));

    let json_payloads = vec![
        ("small", r#"{"image_url": "http://example.com/img.jpg"}"#),
        (
            "medium",
            r#"{
            "image_url": "http://example.com/img.jpg",
            "options": {
                "languages": ["en", "es"],
                "format": "latex",
                "inline_mode": true
            }
        }"#,
        ),
        (
            "large",
            r#"{
            "image_url": "http://example.com/img.jpg",
            "options": {
                "languages": ["en", "es", "fr", "de"],
                "format": "latex",
                "inline_mode": true,
                "detect_orientation": true,
                "skip_preprocessing": false,
                "models": ["text", "math", "table"],
                "confidence_threshold": 0.8
            },
            "metadata": {
                "user_id": "12345",
                "session_id": "abcde",
                "timestamp": 1234567890
            }
        }"#,
        ),
    ];

    for (name, payload) in json_payloads {
        group.bench_with_input(BenchmarkId::new("parse_json", name), &payload, |b, json| {
            b.iter(|| black_box(parse_ocr_request(black_box(json))));
        });
    }

    group.finish();
}

/// Benchmark response serialization
fn bench_response_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("response_serialization");
    group.measurement_time(Duration::from_secs(5));

    let responses = vec![
        ("simple", create_simple_response()),
        ("detailed", create_detailed_response()),
        ("batch", create_batch_response(10)),
    ];

    for (name, response) in responses {
        group.bench_with_input(
            BenchmarkId::new("serialize_json", name),
            &response,
            |b, resp| {
                b.iter(|| black_box(serialize_response(black_box(resp))));
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent request handling
fn bench_concurrent_requests(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_requests");
    group.measurement_time(Duration::from_secs(10));

    let concurrent_levels = [1, 5, 10, 20, 50];

    for concurrency in concurrent_levels {
        group.bench_with_input(
            BenchmarkId::new("handle_requests", concurrency),
            &concurrency,
            |b, &level| {
                b.iter(|| {
                    let handles: Vec<_> = (0..level).map(|_| handle_single_request()).collect();
                    black_box(handles)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark middleware overhead
fn bench_middleware_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("middleware_overhead");
    group.measurement_time(Duration::from_secs(5));

    let request = create_mock_request();

    group.bench_function("no_middleware", |b| {
        b.iter(|| black_box(handle_request_direct(black_box(&request))));
    });

    group.bench_function("with_auth", |b| {
        b.iter(|| {
            let authed = auth_middleware(black_box(&request));
            black_box(handle_request_direct(black_box(&authed)))
        });
    });

    group.bench_function("with_logging", |b| {
        b.iter(|| {
            let logged = logging_middleware(black_box(&request));
            black_box(handle_request_direct(black_box(&logged)))
        });
    });

    group.bench_function("full_stack", |b| {
        b.iter(|| {
            let req = black_box(&request);
            let authed = auth_middleware(req);
            let logged = logging_middleware(&authed);
            let validated = validation_middleware(&logged);
            let rate_limited = rate_limit_middleware(&validated);
            black_box(handle_request_direct(black_box(&rate_limited)))
        });
    });

    group.finish();
}

/// Benchmark request validation
fn bench_request_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_validation");
    group.measurement_time(Duration::from_secs(5));

    let valid_request = create_valid_request();
    let invalid_request = create_invalid_request();

    group.bench_function("validate_valid", |b| {
        b.iter(|| black_box(validate_request(black_box(&valid_request))));
    });

    group.bench_function("validate_invalid", |b| {
        b.iter(|| black_box(validate_request(black_box(&invalid_request))));
    });

    group.finish();
}

/// Benchmark rate limiting
fn bench_rate_limiting(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limiting");
    group.measurement_time(Duration::from_secs(5));

    let mut limiter = RateLimiter::new(100, Duration::from_secs(60));

    group.bench_function("check_limit", |b| {
        b.iter(|| black_box(limiter.check_limit("user_123")));
    });

    group.bench_function("update_limit", |b| {
        b.iter(|| {
            limiter.record_request("user_123");
            black_box(&limiter)
        });
    });

    group.finish();
}

/// Benchmark error handling
fn bench_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("create_error_response", |b| {
        b.iter(|| black_box(create_error_response("Invalid request", 400)));
    });

    group.bench_function("log_and_respond", |b| {
        b.iter(|| {
            let error = "Processing failed";
            log_error(error);
            black_box(create_error_response(error, 500))
        });
    });

    group.finish();
}

/// Benchmark end-to-end API request
fn bench_e2e_api_request(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e_api_request");
    group.measurement_time(Duration::from_secs(15));

    let request_json = r#"{
        "image_url": "http://example.com/img.jpg",
        "options": {
            "format": "latex"
        }
    }"#;

    group.bench_function("full_request_cycle", |b| {
        b.iter(|| {
            // Parse
            let request = parse_ocr_request(black_box(request_json));

            // Validate
            let _validated = validate_request(&request);

            // Auth
            let _authed = auth_middleware(&request);

            // Process (simulated)
            let response = process_ocr_request(&request);

            // Serialize
            let json = serialize_response(&response);

            black_box(json)
        });
    });

    group.finish();
}

// Mock types and implementations

#[derive(Clone)]
struct OcrRequest {
    image_url: String,
    options: RequestOptions,
}

#[derive(Clone)]
struct RequestOptions {
    format: String,
    languages: Vec<String>,
    confidence_threshold: f32,
}

#[derive(Clone)]
struct OcrResponse {
    text: String,
    latex: String,
    confidence: f32,
    regions: Vec<Region>,
}

#[derive(Clone)]
struct Region {
    bbox: [f32; 4],
    text: String,
    confidence: f32,
}

struct RateLimiter {
    max_requests: usize,
    window: Duration,
    requests: std::collections::HashMap<String, Vec<std::time::Instant>>,
}

impl RateLimiter {
    fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            max_requests,
            window,
            requests: std::collections::HashMap::new(),
        }
    }

    fn check_limit(&mut self, user_id: &str) -> bool {
        let now = std::time::Instant::now();
        let requests = self
            .requests
            .entry(user_id.to_string())
            .or_insert_with(Vec::new);

        requests.retain(|&req_time| now.duration_since(req_time) < self.window);

        requests.len() < self.max_requests
    }

    fn record_request(&mut self, user_id: &str) {
        let now = std::time::Instant::now();
        self.requests
            .entry(user_id.to_string())
            .or_insert_with(Vec::new)
            .push(now);
    }
}

fn parse_ocr_request(json: &str) -> OcrRequest {
    // Simulate JSON parsing
    OcrRequest {
        image_url: "http://example.com/img.jpg".to_string(),
        options: RequestOptions {
            format: "latex".to_string(),
            languages: vec!["en".to_string()],
            confidence_threshold: 0.8,
        },
    }
}

fn serialize_response(response: &OcrResponse) -> String {
    // Simulate JSON serialization
    format!(
        r#"{{"text":"{}","latex":"{}","confidence":{}}}"#,
        response.text, response.latex, response.confidence
    )
}

fn create_simple_response() -> OcrResponse {
    OcrResponse {
        text: "E = mc^2".to_string(),
        latex: "E = mc^2".to_string(),
        confidence: 0.95,
        regions: vec![],
    }
}

fn create_detailed_response() -> OcrResponse {
    OcrResponse {
        text: "Complex equation with multiple terms".to_string(),
        latex: "\\int_0^1 x^2 dx = \\frac{1}{3}".to_string(),
        confidence: 0.92,
        regions: vec![
            Region {
                bbox: [0.0, 0.0, 100.0, 50.0],
                text: "integral".to_string(),
                confidence: 0.95,
            },
            Region {
                bbox: [100.0, 0.0, 200.0, 50.0],
                text: "equals".to_string(),
                confidence: 0.98,
            },
        ],
    }
}

fn create_batch_response(count: usize) -> OcrResponse {
    let regions: Vec<_> = (0..count)
        .map(|i| Region {
            bbox: [i as f32 * 10.0, 0.0, (i + 1) as f32 * 10.0, 50.0],
            text: format!("region_{}", i),
            confidence: 0.9,
        })
        .collect();

    OcrResponse {
        text: "Batch text".to_string(),
        latex: "batch latex".to_string(),
        confidence: 0.9,
        regions,
    }
}

fn handle_single_request() -> OcrResponse {
    create_simple_response()
}

fn create_mock_request() -> OcrRequest {
    OcrRequest {
        image_url: "http://example.com/img.jpg".to_string(),
        options: RequestOptions {
            format: "latex".to_string(),
            languages: vec!["en".to_string()],
            confidence_threshold: 0.8,
        },
    }
}

fn handle_request_direct(request: &OcrRequest) -> OcrResponse {
    process_ocr_request(request)
}

fn auth_middleware(request: &OcrRequest) -> OcrRequest {
    // Simulate auth check
    request.clone()
}

fn logging_middleware(request: &OcrRequest) -> OcrRequest {
    // Simulate logging
    request.clone()
}

fn validation_middleware(request: &OcrRequest) -> OcrRequest {
    // Simulate validation
    request.clone()
}

fn rate_limit_middleware(request: &OcrRequest) -> OcrRequest {
    // Simulate rate limiting
    request.clone()
}

fn create_valid_request() -> OcrRequest {
    create_mock_request()
}

fn create_invalid_request() -> OcrRequest {
    OcrRequest {
        image_url: "".to_string(),
        options: RequestOptions {
            format: "invalid".to_string(),
            languages: vec![],
            confidence_threshold: -1.0,
        },
    }
}

fn validate_request(request: &OcrRequest) -> Result<(), String> {
    if request.image_url.is_empty() {
        return Err("Image URL is required".to_string());
    }
    if request.options.confidence_threshold < 0.0 || request.options.confidence_threshold > 1.0 {
        return Err("Invalid confidence threshold".to_string());
    }
    Ok(())
}

fn create_error_response(message: &str, _code: u16) -> String {
    format!(r#"{{"error":"{}"}}"#, message)
}

fn log_error(_message: &str) {
    // Simulate logging
}

fn process_ocr_request(_request: &OcrRequest) -> OcrResponse {
    // Simulate OCR processing
    create_simple_response()
}

criterion_group!(
    benches,
    bench_request_parsing,
    bench_response_serialization,
    bench_concurrent_requests,
    bench_middleware_overhead,
    bench_request_validation,
    bench_rate_limiting,
    bench_error_handling,
    bench_e2e_api_request
);
criterion_main!(benches);
