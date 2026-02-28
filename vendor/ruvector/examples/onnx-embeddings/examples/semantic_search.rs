//! Semantic search example using RuVector integration

use anyhow::Result;
use ruvector_onnx_embeddings::{
    Embedder, RuVectorEmbeddings, IndexConfig, Distance,
};

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Semantic Search with RuVector ONNX Embeddings ===\n");

    // Initialize embedder
    let embedder = Embedder::default_model().await?;
    println!("Loaded model with dimension: {}", embedder.dimension());

    // Create index with custom configuration
    let config = IndexConfig {
        distance: Distance::Cosine,
        max_elements: 100_000,
        ef_search: 100,
    };

    let index = RuVectorEmbeddings::new("semantic_docs", embedder, config)?;

    // Sample document corpus
    let documents = vec![
        ("doc1", "Rust provides memory safety without garbage collection through its ownership system."),
        ("doc2", "Python's simplicity makes it ideal for beginners learning programming."),
        ("doc3", "JavaScript dominates web development with frameworks like React and Vue."),
        ("doc4", "Machine learning models can be trained using TensorFlow or PyTorch."),
        ("doc5", "Docker containers provide consistent deployment environments."),
        ("doc6", "Kubernetes orchestrates containerized applications at scale."),
        ("doc7", "GraphQL offers a more efficient alternative to REST APIs."),
        ("doc8", "PostgreSQL is a powerful open-source relational database."),
        ("doc9", "Redis provides in-memory data storage for caching."),
        ("doc10", "Elasticsearch enables full-text search across large datasets."),
    ];

    // Index documents with metadata
    println!("Indexing {} documents...", documents.len());
    for (id, content) in &documents {
        let metadata = serde_json::json!({ "doc_id": id });
        index.insert(content, Some(metadata))?;
    }

    println!("Index contains {} vectors\n", index.len());

    // Perform semantic searches
    let queries = vec![
        "How can I ensure memory safety in my code?",
        "What's the best language for web applications?",
        "How do I deploy applications in containers?",
        "I need a fast database for caching",
    ];

    for query in queries {
        println!("🔍 Query: \"{}\"\n", query);

        let results = index.search(query, 3)?;

        for (rank, result) in results.iter().enumerate() {
            println!("  {}. [Score: {:.4}]", rank + 1, result.score);
            println!("     {}", result.text);
            if let Some(meta) = &result.metadata {
                if let Some(doc_id) = meta.get("doc_id") {
                    println!("     ({})", doc_id);
                }
            }
            println!();
        }

        println!("{}\n", "-".repeat(70));
    }

    // Find similar documents
    println!("=== Finding Similar Documents ===\n");
    let query_doc = documents[0].1; // Rust document
    println!("Finding documents similar to:\n\"{}\"\n", query_doc);

    let similar = index.search(query_doc, 4)?;
    for (i, result) in similar.iter().skip(1).enumerate() {
        // Skip first (self)
        println!("  {}. [Score: {:.4}] {}", i + 1, result.score, result.text);
    }

    Ok(())
}
