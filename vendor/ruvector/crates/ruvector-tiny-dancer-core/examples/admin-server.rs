//! Admin and health check example for Tiny Dancer
//!
//! This example demonstrates how to implement health checks and
//! administrative functionality for the Tiny Dancer routing system.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example admin-server
//! ```
//!
//! This example shows:
//! - Health check implementations
//! - Configuration inspection
//! - Circuit breaker status monitoring
//! - Hot model reloading
//!
//! For a full HTTP admin server implementation, see the `api` module
//! documentation which requires additional dependencies (axum, tokio).

use ruvector_tiny_dancer_core::{Candidate, Router, RouterConfig, RoutingRequest};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tiny Dancer Admin Example ===\n");

    // Create router with default configuration
    let router_config = RouterConfig {
        model_path: "./models/fastgrnn.safetensors".to_string(),
        confidence_threshold: 0.85,
        max_uncertainty: 0.15,
        enable_circuit_breaker: true,
        circuit_breaker_threshold: 5,
        enable_quantization: true,
        database_path: None,
    };

    println!("Creating router with config:");
    println!("  Model path: {}", router_config.model_path);
    println!(
        "  Confidence threshold: {}",
        router_config.confidence_threshold
    );
    println!("  Max uncertainty: {}", router_config.max_uncertainty);
    println!(
        "  Circuit breaker: {}",
        router_config.enable_circuit_breaker
    );

    let router = Router::new(router_config.clone())?;

    // Health check implementation
    println!("\n--- Health Check ---");
    let health = check_health(&router);
    println!("Status: {}", if health { "healthy" } else { "unhealthy" });

    // Readiness check
    println!("\n--- Readiness Check ---");
    let ready = check_readiness(&router);
    println!("Ready: {}", ready);

    // Configuration info
    println!("\n--- Configuration ---");
    let config = router.config();
    println!("Current configuration: {:?}", config);

    // Circuit breaker status
    println!("\n--- Circuit Breaker Status ---");
    match router.circuit_breaker_status() {
        Some(true) => println!("State: Closed (accepting requests)"),
        Some(false) => println!("State: Open (rejecting requests)"),
        None => println!("State: Disabled"),
    }

    // Test routing to verify system works
    println!("\n--- Test Routing ---");
    let candidates = vec![Candidate {
        id: "test-1".to_string(),
        embedding: vec![0.5; 384],
        metadata: HashMap::new(),
        created_at: chrono::Utc::now().timestamp(),
        access_count: 10,
        success_rate: 0.95,
    }];

    let request = RoutingRequest {
        query_embedding: vec![0.5; 384],
        candidates,
        metadata: None,
    };

    match router.route(request) {
        Ok(response) => {
            println!(
                "Test routing successful: {} candidates in {}μs",
                response.candidates_processed, response.inference_time_us
            );
        }
        Err(e) => {
            println!("Test routing failed: {}", e);
        }
    }

    // Model reload demonstration
    println!("\n--- Model Reload ---");
    println!("Attempting model reload...");
    match router.reload_model() {
        Ok(_) => println!("Model reload: Success"),
        Err(e) => println!("Model reload: {} (expected if model file doesn't exist)", e),
    }

    println!("\n=== Admin Example Complete ===");
    println!("\nFor a full HTTP admin server, you would need:");
    println!("1. Add axum and tokio dependencies");
    println!("2. Enable the admin-api feature");
    println!("3. Use the AdminServer from the api module");

    Ok(())
}

/// Basic health check - returns true if the router is operational
fn check_health(router: &Router) -> bool {
    // A simple health check just verifies the router exists
    // In production, you might also check model availability
    router.config().model_path.len() > 0
}

/// Readiness check - returns true if ready to accept traffic
fn check_readiness(router: &Router) -> bool {
    // Check circuit breaker status
    match router.circuit_breaker_status() {
        Some(is_closed) => is_closed, // Ready only if circuit breaker is closed
        None => true,                 // Ready if circuit breaker is disabled
    }
}
