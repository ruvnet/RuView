//! Example demonstrating basic tracing with the Tiny Dancer routing system
//!
//! This example shows how to:
//! - Create and configure a router
//! - Process routing requests
//! - Monitor timing and performance
//!
//! Run with: cargo run --example tracing_example

use ruvector_tiny_dancer_core::{Candidate, Router, RouterConfig, RoutingRequest};
use std::collections::HashMap;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tiny Dancer Routing Example with Timing ===\n");

    // Create router with configuration
    let config = RouterConfig {
        model_path: "./models/fastgrnn.safetensors".to_string(),
        confidence_threshold: 0.85,
        max_uncertainty: 0.15,
        enable_circuit_breaker: true,
        circuit_breaker_threshold: 5,
        ..Default::default()
    };

    let router = Router::new(config)?;

    // Process requests with timing
    println!("Processing requests with timing information...\n");

    for i in 0..3 {
        let request_start = Instant::now();
        println!("Request {} - Processing", i + 1);

        // Create candidates
        let candidates = vec![
            Candidate {
                id: format!("candidate-{}-1", i),
                embedding: vec![0.5; 384],
                metadata: HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                access_count: 10,
                success_rate: 0.95,
            },
            Candidate {
                id: format!("candidate-{}-2", i),
                embedding: vec![0.3; 384],
                metadata: HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                access_count: 5,
                success_rate: 0.85,
            },
        ];

        let request = RoutingRequest {
            query_embedding: vec![0.5; 384],
            candidates: candidates.clone(),
            metadata: None,
        };

        // Route request
        match router.route(request) {
            Ok(response) => {
                let total_time = request_start.elapsed();
                println!(
                    "\nRequest {}: Processed {} candidates in {}μs (total: {:?})",
                    i + 1,
                    response.candidates_processed,
                    response.inference_time_us,
                    total_time
                );

                for decision in response.decisions.iter().take(2) {
                    println!(
                        "  - {} (confidence: {:.2}, lightweight: {})",
                        decision.candidate_id, decision.confidence, decision.use_lightweight
                    );
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }

        println!();
    }

    println!("\n=== Routing Example Complete ===");
    println!("\nTiming breakdown available in each response:");
    println!("- inference_time_us: Total inference time");
    println!("- feature_time_us: Feature engineering time");
    println!("- candidates_processed: Number of candidates evaluated");

    Ok(())
}
