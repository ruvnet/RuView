//! Demo of AI/ML API clients for RuVector data discovery
//!
//! This example demonstrates how to use the various ML clients to fetch
//! models, datasets, and research papers, converting them to SemanticVectors
//! for discovery analysis.
//!
//! # Usage
//! ```bash
//! # Basic demo (uses mock data)
//! cargo run --example ml_clients_demo
//!
//! # With API keys (optional)
//! export HUGGINGFACE_API_KEY="your_key_here"
//! export REPLICATE_API_TOKEN="your_token_here"
//! export TOGETHER_API_KEY="your_key_here"
//! cargo run --example ml_clients_demo
//! ```

use ruvector_data_framework::{
    HuggingFaceClient, OllamaClient, PapersWithCodeClient, ReplicateClient, TogetherAiClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== RuVector ML API Clients Demo ===\n");

    // 1. HuggingFace Demo
    println!("1. HuggingFace Model Hub");
    println!("{}", "-".repeat(50));
    let hf_client = HuggingFaceClient::new();

    match hf_client.search_models("bert", Some("fill-mask")).await {
        Ok(models) => {
            println!("Found {} models matching 'bert'", models.len());
            for model in models.iter().take(3) {
                let vector = hf_client.model_to_vector(model);
                println!("  - {} (downloads: {})", model.model_id, model.downloads.unwrap_or(0));
                println!("    Vector ID: {}", vector.id);
                println!("    Embedding dim: {}", vector.embedding.len());
            }
        }
        Err(e) => println!("Error: {} (using mock data)", e),
    }
    println!();

    // 2. Ollama Demo
    println!("2. Ollama Local LLM");
    println!("{}", "-".repeat(50));
    let mut ollama_client = OllamaClient::new();

    match ollama_client.list_models().await {
        Ok(models) => {
            println!("Available Ollama models: {}", models.len());
            for model in models.iter().take(3) {
                let vector = ollama_client.model_to_vector(model);
                println!("  - {} (size: {} GB)",
                    model.name,
                    model.size.unwrap_or(0) / 1_000_000_000
                );
                println!("    Vector ID: {}", vector.id);
            }
        }
        Err(e) => println!("Error: {} (Ollama may not be running)", e),
    }
    println!();

    // 3. Papers With Code Demo
    println!("3. Papers With Code Research Database");
    println!("{}", "-".repeat(50));
    let pwc_client = PapersWithCodeClient::new();

    match pwc_client.search_papers("transformer").await {
        Ok(papers) => {
            println!("Found {} papers about transformers", papers.len());
            for paper in papers.iter().take(3) {
                let vector = pwc_client.paper_to_vector(paper);
                println!("  - {}", paper.title);
                println!("    Vector ID: {}", vector.id);
                if let Some(url) = &paper.url_abs {
                    println!("    URL: {}", url);
                }
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    println!();

    // 4. Replicate Demo
    println!("4. Replicate Cloud ML Models");
    println!("{}", "-".repeat(50));
    let replicate_client = ReplicateClient::new();

    match replicate_client.get_model("stability-ai", "stable-diffusion").await {
        Ok(Some(model)) => {
            let vector = replicate_client.model_to_vector(&model);
            println!("Model: {}/{}", model.owner, model.name);
            println!("  Description: {}", model.description.as_deref().unwrap_or("N/A"));
            println!("  Vector ID: {}", vector.id);
        }
        Ok(None) => println!("Model not found"),
        Err(e) => println!("Error: {} (using mock data)", e),
    }
    println!();

    // 5. Together AI Demo
    println!("5. Together AI Open Source Models");
    println!("{}", "-".repeat(50));
    let together_client = TogetherAiClient::new();

    match together_client.list_models().await {
        Ok(models) => {
            println!("Available Together AI models: {}", models.len());
            for model in models.iter().take(3) {
                let vector = together_client.model_to_vector(model);
                println!("  - {}", model.display_name.as_deref().unwrap_or(&model.id));
                println!("    Context length: {}", model.context_length.unwrap_or(0));
                println!("    Vector ID: {}", vector.id);
            }
        }
        Err(e) => println!("Error: {} (using mock data)", e),
    }
    println!();

    // 6. Embeddings Demo
    println!("6. Text Embeddings");
    println!("{}", "-".repeat(50));
    let test_text = "Large language models are transforming AI research";

    match ollama_client.embeddings("llama2", test_text).await {
        Ok(embedding) => {
            println!("Generated embedding for: '{}'", test_text);
            println!("  Embedding dimension: {}", embedding.len());
            println!("  First 5 values: {:?}", &embedding[..5.min(embedding.len())]);
        }
        Err(e) => println!("Error: {} (using fallback embedder)", e),
    }
    println!();

    // Summary
    println!("=== Summary ===");
    println!("All ML clients initialized successfully!");
    println!("Set environment variables for API keys to access real data:");
    println!("  - HUGGINGFACE_API_KEY (optional for public models)");
    println!("  - REPLICATE_API_TOKEN (required for inference)");
    println!("  - TOGETHER_API_KEY (required for chat/embeddings)");
    println!("  - Ollama: start service with 'ollama serve'");

    Ok(())
}
