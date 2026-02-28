//! Example demonstrating metrics collection with Tiny Dancer
//!
//! This example shows how to:
//! - Collect routing metrics manually
//! - Monitor circuit breaker state
//! - Track routing decisions and latencies
//!
//! Run with: cargo run --example metrics_example

use ruvector_tiny_dancer_core::{Candidate, Router, RouterConfig, RoutingRequest};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tiny Dancer Metrics Example ===\n");

    // Create router with metrics enabled
    let config = RouterConfig {
        model_path: "./models/fastgrnn.safetensors".to_string(),
        confidence_threshold: 0.85,
        max_uncertainty: 0.15,
        enable_circuit_breaker: true,
        circuit_breaker_threshold: 5,
        ..Default::default()
    };

    let router = Router::new(config)?;

    // Track metrics manually
    let mut total_requests = 0u64;
    let mut total_candidates = 0u64;
    let mut total_latency_us = 0u64;
    let mut lightweight_count = 0u64;
    let mut powerful_count = 0u64;

    // Process multiple routing requests
    println!("Processing routing requests...\n");

    for i in 0..10 {
        let candidates = vec![
            Candidate {
                id: format!("candidate-{}-1", i),
                embedding: vec![0.5 + (i as f32 * 0.01); 384],
                metadata: HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                access_count: 10 + i as u64,
                success_rate: 0.95 - (i as f32 * 0.01),
            },
            Candidate {
                id: format!("candidate-{}-2", i),
                embedding: vec![0.3 + (i as f32 * 0.01); 384],
                metadata: HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                access_count: 5 + i as u64,
                success_rate: 0.85 - (i as f32 * 0.01),
            },
            Candidate {
                id: format!("candidate-{}-3", i),
                embedding: vec![0.7 + (i as f32 * 0.01); 384],
                metadata: HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                access_count: 15 + i as u64,
                success_rate: 0.98 - (i as f32 * 0.01),
            },
        ];

        let request = RoutingRequest {
            query_embedding: vec![0.5; 384],
            candidates,
            metadata: None,
        };

        match router.route(request) {
            Ok(response) => {
                total_requests += 1;
                total_candidates += response.candidates_processed as u64;
                total_latency_us += response.inference_time_us;

                // Count routing decisions
                for decision in &response.decisions {
                    if decision.use_lightweight {
                        lightweight_count += 1;
                    } else {
                        powerful_count += 1;
                    }
                }

                println!(
                    "Request {}: Processed {} candidates in {}μs",
                    i + 1,
                    response.candidates_processed,
                    response.inference_time_us
                );
                if let Some(top) = response.decisions.first() {
                    println!(
                        "  Top decision: {} (confidence: {:.3}, lightweight: {})",
                        top.candidate_id, top.confidence, top.use_lightweight
                    );
                }
            }
            Err(e) => {
                eprintln!("Error processing request {}: {}", i + 1, e);
            }
        }
    }

    // Display collected metrics
    println!("\n=== Collected Metrics ===\n");

    let cb_state = match router.circuit_breaker_status() {
        Some(true) => "closed",
        Some(false) => "open",
        None => "disabled",
    };

    let avg_latency = if total_requests > 0 {
        total_latency_us / total_requests
    } else {
        0
    };

    println!("tiny_dancer_routing_requests_total {}", total_requests);
    println!(
        "tiny_dancer_candidates_processed_total {}",
        total_candidates
    );
    println!(
        "tiny_dancer_routing_decisions_total{{model_type=\"lightweight\"}} {}",
        lightweight_count
    );
    println!(
        "tiny_dancer_routing_decisions_total{{model_type=\"powerful\"}} {}",
        powerful_count
    );
    println!("tiny_dancer_avg_latency_us {}", avg_latency);
    println!("tiny_dancer_circuit_breaker_state {}", cb_state);

    println!("\n=== Metrics Collection Complete ===");
    println!("\nThese metrics can be exported to monitoring systems:");
    println!("- Prometheus for time-series collection");
    println!("- Grafana for visualization");
    println!("- Custom dashboards for real-time monitoring");

    Ok(())
}
