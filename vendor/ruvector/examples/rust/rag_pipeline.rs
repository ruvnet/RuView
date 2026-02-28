//! RAG (Retrieval Augmented Generation) Pipeline Example
//!
//! Demonstrates building a complete RAG system with Ruvector.
//!
//! ⚠️ NOTE: This example uses MOCK embeddings for demonstration.
//! In production, replace `mock_embedding()` with a real embedding model:
//! - `sentence-transformers` via Python bindings
//! - `candle` for native Rust inference
//! - ONNX Runtime for cross-platform models
//! - OpenAI/Anthropic embedding APIs

use ruvector_core::{VectorDB, VectorEntry, SearchQuery, DbOptions, Result};
use std::collections::HashMap;
use serde_json::json;

fn main() -> Result<()> {
    println!("📚 RAG Pipeline Example\n");

    // 1. Setup database
    println!("1. Setting up knowledge base...");
    let mut options = DbOptions::default();
    options.dimensions = 384;  // sentence-transformers/all-MiniLM-L6-v2
    options.storage_path = "./rag_knowledge.db".to_string();

    let db = VectorDB::new(options)?;
    println!("   ✓ Database created\n");

    // 2. Ingest documents
    println!("2. Ingesting documents into knowledge base...");
    let documents = vec![
        (
            "Rust is a systems programming language that focuses on safety and performance.",
            mock_embedding(384, 1.0)
        ),
        (
            "Vector databases enable semantic search by storing and querying embeddings.",
            mock_embedding(384, 1.1)
        ),
        (
            "HNSW (Hierarchical Navigable Small World) provides efficient approximate nearest neighbor search.",
            mock_embedding(384, 1.2)
        ),
        (
            "RAG combines retrieval systems with language models for better context-aware generation.",
            mock_embedding(384, 1.3)
        ),
        (
            "Embeddings are dense vector representations of text that capture semantic meaning.",
            mock_embedding(384, 1.4)
        ),
    ];

    let entries: Vec<VectorEntry> = documents.into_iter().enumerate()
        .map(|(i, (text, embedding))| {
            let mut metadata = HashMap::new();
            metadata.insert("text".to_string(), json!(text));
            metadata.insert("doc_id".to_string(), json!(format!("doc_{}", i)));
            metadata.insert("timestamp".to_string(), json!(chrono::Utc::now().timestamp()));

            VectorEntry {
                id: Some(format!("doc_{}", i)),
                vector: embedding,
                metadata: Some(metadata),
            }
        })
        .collect();

    db.insert_batch(entries)?;
    println!("   ✓ Ingested {} documents\n", 5);

    // 3. Retrieval phase
    println!("3. Retrieval phase (finding relevant context)...");
    let user_query = "How do vector databases work?";
    let query_embedding = mock_embedding(384, 1.15);  // Mock embedding for query

    let query = SearchQuery {
        vector: query_embedding,
        k: 3,  // Retrieve top 3 most relevant documents
        filter: None,
        include_vectors: false,
    };

    let results = db.search(&query)?;
    println!("   ✓ Query: \"{}\"", user_query);
    println!("   ✓ Retrieved {} relevant documents:\n", results.len());

    let mut context_passages = Vec::new();
    for (i, result) in results.iter().enumerate() {
        if let Some(metadata) = &result.metadata {
            if let Some(text) = metadata.get("text") {
                let text_str = text.as_str().unwrap();
                context_passages.push(text_str);
                println!("     {}. (score: {:.4})", i + 1, result.distance);
                println!("        {}\n", text_str);
            }
        }
    }

    // 4. Generation phase (mock)
    println!("4. Generation phase (constructing prompt for LLM)...");
    let prompt = construct_rag_prompt(user_query, &context_passages);
    println!("   ✓ Prompt constructed:");
    println!("   {}\n", "─".repeat(60));
    println!("{}", prompt);
    println!("   {}\n", "─".repeat(60));

    // 5. (In real application, send prompt to LLM here)
    println!("5. Next step: Send prompt to LLM for generation");
    println!("   ✓ In production, you would:");
    println!("     - Send the constructed prompt to an LLM (GPT, Claude, etc.)");
    println!("     - Receive context-aware response");
    println!("     - Return response to user\n");

    println!("✅ RAG pipeline example completed!");
    println!("\n💡 Key benefits:");
    println!("   • Semantic search finds relevant context automatically");
    println!("   • LLM generates responses based on your knowledge base");
    println!("   • Up-to-date information without retraining models");
    println!("   • Sub-millisecond retrieval with Ruvector");

    Ok(())
}

/// ⚠️ MOCK EMBEDDING - NOT SEMANTIC
/// This produces deterministic vectors based on seed value.
/// Replace with actual embedding model for real semantic search.
fn mock_embedding(dims: usize, seed: f32) -> Vec<f32> {
    (0..dims)
        .map(|i| (seed + i as f32 * 0.001).sin())
        .collect()
}

fn construct_rag_prompt(query: &str, context: &[&str]) -> String {
    let context_text = context.iter()
        .enumerate()
        .map(|(i, text)| format!("[{}] {}", i + 1, text))
        .collect::<Vec<_>>()
        .join("\n\n");

    format!(
        "You are a helpful assistant. Answer the user's question based on the provided context.\n\n\
        Context:\n{}\n\n\
        User Question: {}\n\n\
        Answer:",
        context_text, query
    )
}
