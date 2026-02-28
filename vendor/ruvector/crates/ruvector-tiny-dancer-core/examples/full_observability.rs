//! Comprehensive observability example demonstrating routing performance
//!
//! This example demonstrates:
//! - Circuit breaker monitoring
//! - Performance tracking
//! - Response statistics
//! - Different load scenarios
//!
//! Run with: cargo run --example full_observability

use ruvector_tiny_dancer_core::{Candidate, Router, RouterConfig, RoutingRequest, RoutingResponse};
use std::collections::HashMap;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tiny Dancer Full Observability Example ===\n");

    // Create router with full configuration
    let config = RouterConfig {
        model_path: "./models/fastgrnn.safetensors".to_string(),
        confidence_threshold: 0.85,
        max_uncertainty: 0.15,
        enable_circuit_breaker: true,
        circuit_breaker_threshold: 3,
        enable_quantization: true,
        database_path: None,
    };

    let router = Router::new(config)?;

    // Track metrics manually
    let mut total_requests = 0u64;
    let mut successful_requests = 0u64;
    let mut total_latency_us = 0u64;
    let mut lightweight_routes = 0usize;
    let mut powerful_routes = 0usize;

    println!("\n=== Scenario 1: Normal Operations ===\n");

    // Process normal requests
    for i in 0..5 {
        let candidates = create_candidates(i, 3);
        let request = RoutingRequest {
            query_embedding: vec![0.5 + (i as f32 * 0.05); 384],
            candidates,
            metadata: Some(HashMap::from([(
                "scenario".to_string(),
                serde_json::json!("normal_operations"),
            )])),
        };

        total_requests += 1;
        match router.route(request) {
            Ok(response) => {
                successful_requests += 1;
                total_latency_us += response.inference_time_us;
                let (lw, pw) = count_routes(&response);
                lightweight_routes += lw;
                powerful_routes += pw;
                print_response_summary(i + 1, &response);
            }
            Err(e) => {
                eprintln!("Request {} failed: {}", i + 1, e);
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    println!("\n=== Scenario 2: High Load ===\n");

    // Simulate high load with many candidates
    for i in 0..3 {
        let candidates = create_candidates(i, 20); // More candidates
        let request = RoutingRequest {
            query_embedding: vec![0.6; 384],
            candidates,
            metadata: Some(HashMap::from([(
                "scenario".to_string(),
                serde_json::json!("high_load"),
            )])),
        };

        total_requests += 1;
        match router.route(request) {
            Ok(response) => {
                successful_requests += 1;
                total_latency_us += response.inference_time_us;
                let (lw, pw) = count_routes(&response);
                lightweight_routes += lw;
                powerful_routes += pw;
                print_response_summary(i + 1, &response);
            }
            Err(e) => {
                eprintln!("Request {} failed: {}", i + 1, e);
            }
        }
    }

    // Display statistics
    println!("\n=== Performance Statistics ===\n");
    display_statistics(
        total_requests,
        successful_requests,
        total_latency_us,
        lightweight_routes,
        powerful_routes,
        &router,
    );

    println!("\n=== Full Observability Example Complete ===");
    println!("\nMetrics Summary:");
    println!("- Total requests processed");
    println!("- Success/failure rates tracked");
    println!("- Latency statistics computed");
    println!("- Routing decisions categorized");
    println!("- Circuit breaker state monitored");

    Ok(())
}

fn create_candidates(offset: i32, count: usize) -> Vec<Candidate> {
    (0..count)
        .map(|i| {
            let base_score = 0.7 + ((i + offset as usize) as f32 * 0.02) % 0.3;
            Candidate {
                id: format!("candidate-{}-{}", offset, i),
                embedding: vec![base_score; 384],
                metadata: HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                access_count: 10 + i as u64,
                success_rate: 0.85 + (base_score * 0.15),
            }
        })
        .collect()
}

fn count_routes(response: &RoutingResponse) -> (usize, usize) {
    let lightweight = response
        .decisions
        .iter()
        .filter(|d| d.use_lightweight)
        .count();
    let powerful = response.decisions.len() - lightweight;
    (lightweight, powerful)
}

fn print_response_summary(request_num: i32, response: &RoutingResponse) {
    let (lightweight_count, powerful_count) = count_routes(response);

    println!(
        "Request {}: {}μs total, {}μs features, {} candidates",
        request_num,
        response.inference_time_us,
        response.feature_time_us,
        response.candidates_processed
    );
    println!(
        "  Routing: {} lightweight, {} powerful",
        lightweight_count, powerful_count
    );

    if let Some(top_decision) = response.decisions.first() {
        println!(
            "  Top: {} (confidence: {:.3}, uncertainty: {:.3})",
            top_decision.candidate_id, top_decision.confidence, top_decision.uncertainty
        );
    }
}

fn display_statistics(
    total_requests: u64,
    successful_requests: u64,
    total_latency_us: u64,
    lightweight_routes: usize,
    powerful_routes: usize,
    router: &Router,
) {
    let cb_state = match router.circuit_breaker_status() {
        Some(true) => "Closed",
        Some(false) => "Open",
        None => "Disabled",
    };

    let success_rate = if total_requests > 0 {
        (successful_requests as f64 / total_requests as f64) * 100.0
    } else {
        0.0
    };

    let avg_latency = if successful_requests > 0 {
        total_latency_us / successful_requests
    } else {
        0
    };

    println!("Circuit Breaker: {}", cb_state);
    println!("Total Requests: {}", total_requests);
    println!("Successful Requests: {}", successful_requests);
    println!("Success Rate: {:.1}%", success_rate);
    println!("Avg Latency: {}μs", avg_latency);
    println!("Lightweight Routes: {}", lightweight_routes);
    println!("Powerful Routes: {}", powerful_routes);
}
