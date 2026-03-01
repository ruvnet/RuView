//! LLM Response Validation Example
//!
//! This example demonstrates how to use Prime-Radiant's sheaf coherence
//! to validate LLM responses against their context.
//!
//! The validator:
//! 1. Converts context and response embeddings into sheaf graph nodes
//! 2. Adds edges with semantic consistency constraints
//! 3. Computes coherence energy
//! 4. Produces a validation result with witness record for audit
//!
//! Run with: `cargo run --example llm_validation --features ruvllm`

#[cfg(feature = "ruvllm")]
use prime_radiant::ruvllm_integration::{
    EdgeWeights, SheafCoherenceValidator, ValidationContext, ValidatorConfig,
};

#[cfg(feature = "ruvllm")]
fn main() {
    println!("=== Prime-Radiant: LLM Validation Example ===\n");

    // Example 1: Coherent response (passes validation)
    println!("--- Example 1: Coherent LLM Response ---");
    run_coherent_validation();

    println!();

    // Example 2: Incoherent response (fails validation)
    println!("--- Example 2: Incoherent LLM Response ---");
    run_incoherent_validation();

    println!();

    // Example 3: Validation with supporting evidence
    println!("--- Example 3: Validation with Supporting Evidence ---");
    run_validation_with_support();

    println!();

    // Example 4: Demonstrate witness generation
    println!("--- Example 4: Witness Generation for Audit Trail ---");
    run_witness_example();
}

#[cfg(not(feature = "ruvllm"))]
fn main() {
    println!("This example requires the 'ruvllm' feature.");
    println!("Run with: cargo run --example llm_validation --features ruvllm");
}

#[cfg(feature = "ruvllm")]
fn run_coherent_validation() {
    // Create a validator with default configuration
    let mut validator = SheafCoherenceValidator::with_defaults();

    // Create context and response embeddings
    // In practice, these would come from an embedding model
    // Here we simulate a coherent scenario: response is very similar to context

    let context_embedding = create_embedding(64, 1.0, 0.5);
    let response_embedding = create_embedding(64, 1.0, 0.5); // Same as context

    let ctx = ValidationContext::new()
        .with_context_embedding(context_embedding)
        .with_response_embedding(response_embedding)
        .with_scope("general")
        .with_metadata("model", "example-llm")
        .with_metadata("prompt_type", "factual_qa");

    // Validate the response
    match validator.validate(&ctx) {
        Ok(result) => {
            println!("Validation Context:");
            println!("  Embedding dimension: {}", ctx.embedding_dim());
            println!("  Scope: {}", ctx.scope);
            println!();
            println!("Validation Result:");
            println!("  Allowed: {}", result.allowed);
            println!("  Energy: {:.6}", result.energy);
            println!("  Reason: {}", result.reason.as_deref().unwrap_or("N/A"));
            println!();
            println!("Witness:");
            println!("  ID: {}", result.witness.id);
            println!("  Energy at validation: {:.6}", result.witness.energy);
            println!("  Decision allowed: {}", result.witness.decision.allowed);
            println!(
                "  Integrity verified: {}",
                result.witness.verify_integrity()
            );

            if result.allowed {
                println!();
                println!("  -> Response passed coherence validation!");
            }
        }
        Err(e) => {
            println!("Validation failed: {}", e);
        }
    }
}

#[cfg(feature = "ruvllm")]
fn run_incoherent_validation() {
    // Configure a strict validator
    let config = ValidatorConfig {
        default_dim: 64,
        reflex_threshold: 0.01, // Very strict - low energy required
        retrieval_threshold: 0.05,
        heavy_threshold: 0.1,
        include_supporting: false,
        create_cross_support_edges: false,
    };

    let mut validator = SheafCoherenceValidator::with_defaults().with_config(config);

    // Create DIFFERENT embeddings to simulate incoherent response
    // This could represent:
    // - A hallucinated response not supported by context
    // - An off-topic response
    // - Factually inconsistent information

    let context_embedding = create_embedding(64, 1.0, 0.0); // Context about topic A
    let response_embedding = create_embedding(64, -1.0, 0.5); // Response about opposite topic

    let ctx = ValidationContext::new()
        .with_context_embedding(context_embedding)
        .with_response_embedding(response_embedding)
        .with_scope("strict")
        .with_edge_weights(EdgeWeights::strict()) // Use strict weights
        .with_metadata("model", "example-llm")
        .with_metadata("risk_level", "high");

    match validator.validate(&ctx) {
        Ok(result) => {
            println!("Validation Context:");
            println!("  Embedding dimension: {}", ctx.embedding_dim());
            println!("  Edge weights: Strict mode");
            println!();
            println!("Validation Result:");
            println!("  Allowed: {}", result.allowed);
            println!("  Energy: {:.6}", result.energy);
            println!("  Reason: {}", result.reason.as_deref().unwrap_or("N/A"));
            println!();

            if !result.allowed {
                println!("  -> Response REJECTED due to high incoherence!");
                println!("     The response embedding differs significantly from context.");
                println!("     In a real system, this might indicate:");
                println!("     - Hallucination (making up facts)");
                println!("     - Off-topic response");
                println!("     - Contradiction with given context");
            }
        }
        Err(e) => {
            println!("Validation failed: {}", e);
        }
    }
}

#[cfg(feature = "ruvllm")]
fn run_validation_with_support() {
    // Configure validator to include supporting embeddings
    let config = ValidatorConfig {
        default_dim: 64,
        reflex_threshold: 0.3,
        retrieval_threshold: 0.6,
        heavy_threshold: 0.9,
        include_supporting: true,         // Enable supporting evidence
        create_cross_support_edges: true, // Create edges between support docs
    };

    let mut validator = SheafCoherenceValidator::with_defaults().with_config(config);

    // Create embeddings: context, response, and retrieved support documents
    let context_embedding = create_embedding(64, 0.8, 0.3);
    let response_embedding = create_embedding(64, 0.75, 0.35); // Similar to context

    // Supporting documents (e.g., from RAG retrieval)
    let support_1 = create_embedding(64, 0.85, 0.28); // Close to context
    let support_2 = create_embedding(64, 0.78, 0.32); // Also close

    let ctx = ValidationContext::new()
        .with_context_embedding(context_embedding)
        .with_response_embedding(response_embedding)
        .with_supporting_embedding(support_1)
        .with_supporting_embedding(support_2)
        .with_scope("rag_qa")
        .with_metadata("retriever", "dense_passage")
        .with_metadata("num_docs", "2");

    match validator.validate(&ctx) {
        Ok(result) => {
            println!("Validation with Supporting Evidence:");
            println!("  Context embedding: 64 dimensions");
            println!("  Response embedding: 64 dimensions");
            println!("  Supporting documents: 2");
            println!();
            println!("Sheaf Graph Structure:");
            println!("  - Context node connected to Response");
            println!("  - Context node connected to each Support doc");
            println!("  - Response node connected to each Support doc");
            println!("  - Support docs connected to each other (cross-edges enabled)");
            println!();
            println!("Validation Result:");
            println!("  Allowed: {}", result.allowed);
            println!("  Energy: {:.6}", result.energy);
            println!();

            // Show edge breakdown
            if !result.edge_breakdown.is_empty() {
                println!("Edge Energy Breakdown:");
                for (edge_type, energy) in &result.edge_breakdown {
                    println!("  {}: {:.6}", edge_type, energy);
                }
            }
        }
        Err(e) => {
            println!("Validation failed: {}", e);
        }
    }
}

#[cfg(feature = "ruvllm")]
fn run_witness_example() {
    let mut validator = SheafCoherenceValidator::with_defaults();

    let context_embedding = create_embedding(64, 1.0, 0.0);
    let response_embedding = create_embedding(64, 0.9, 0.1);

    let ctx = ValidationContext::new()
        .with_context_embedding(context_embedding)
        .with_response_embedding(response_embedding)
        .with_scope("audit_example")
        .with_metadata("user_id", "user_12345")
        .with_metadata("session_id", "sess_abc");

    match validator.validate(&ctx) {
        Ok(result) => {
            println!("Witness Record for Audit Trail:");
            println!("================================");
            println!();
            println!("Witness ID: {}", result.witness.id);
            println!("Timestamp: {:?}", result.witness.timestamp);
            println!();
            println!("Content Hashes (for integrity verification):");
            println!("  Context hash: {}", result.witness.context_hash);
            println!("  Response hash: {}", result.witness.response_hash);
            println!("  Fingerprint: {}", result.witness.fingerprint);
            println!();
            println!("Decision Details:");
            println!("  Scope: {}", result.witness.scope);
            println!("  Allowed: {}", result.witness.decision.allowed);
            println!(
                "  Compute lane: {} (0=Reflex, 1=Retrieval, 2=Heavy, 3=Human)",
                result.witness.decision.lane
            );
            println!("  Confidence: {:.4}", result.witness.decision.confidence);
            println!("  Energy: {:.6}", result.witness.energy);
            println!();
            println!("Integrity Verification:");
            println!("  Hash matches: {}", result.witness.verify_integrity());
            println!();
            println!("Request Correlation:");
            println!("  Request ID: {}", result.request_id);
            println!();
            println!("This witness record provides:");
            println!("  - Cryptographic proof of the validation decision");
            println!("  - Content hashes for tamper detection");
            println!("  - Correlation ID for request tracing");
            println!("  - Energy metrics for monitoring");
        }
        Err(e) => {
            println!("Validation failed: {}", e);
        }
    }
}

/// Helper function to create a test embedding
/// base_value and variation control the embedding pattern
#[cfg(feature = "ruvllm")]
fn create_embedding(dim: usize, base_value: f32, variation: f32) -> Vec<f32> {
    (0..dim)
        .map(|i| {
            let angle = (i as f32) * std::f32::consts::PI / (dim as f32);
            base_value * angle.cos() + variation * angle.sin()
        })
        .collect()
}
