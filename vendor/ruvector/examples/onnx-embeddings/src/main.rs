//! RuVector ONNX Embeddings - Example Usage
//!
//! This example demonstrates how to use ONNX-based embedding generation
//! with RuVector for semantic search and RAG pipelines.

use anyhow::Result;
use ruvector_onnx_embeddings::{
    prelude::*, EmbedderBuilder, PretrainedModel, PoolingStrategy,
    RuVectorBuilder, RagPipeline, Distance,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║     RuVector ONNX Embeddings - Reimagined for Rust            ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    // Run examples
    basic_embedding_example().await?;
    batch_embedding_example().await?;
    semantic_search_example().await?;
    rag_pipeline_example().await?;
    clustering_example().await?;

    println!("\n✅ All examples completed successfully!");

    Ok(())
}

/// Basic embedding generation
async fn basic_embedding_example() -> Result<()> {
    println!("\n━━━ Example 1: Basic Embedding Generation ━━━");

    // Create embedder with default model (all-MiniLM-L6-v2)
    let mut embedder = Embedder::default_model().await?;

    println!("Model: {}", embedder.model_info().name);
    println!("Dimension: {}", embedder.dimension());

    // Embed a single sentence
    let text = "The quick brown fox jumps over the lazy dog.";
    let embedding = embedder.embed_one(text)?;

    println!("Input: \"{}\"", text);
    println!("Embedding shape: [{}]", embedding.len());
    println!(
        "First 5 values: [{:.4}, {:.4}, {:.4}, {:.4}, {:.4}]",
        embedding[0], embedding[1], embedding[2], embedding[3], embedding[4]
    );

    // Compute similarity between two sentences
    let text1 = "I love programming in Rust.";
    let text2 = "Rust is my favorite programming language.";
    let text3 = "The weather is nice today.";

    let sim_related = embedder.similarity(text1, text2)?;
    let sim_unrelated = embedder.similarity(text1, text3)?;

    println!("\nSimilarity comparisons:");
    println!("  \"{}\"\n  vs\n  \"{}\"", text1, text2);
    println!("  Similarity: {:.4}", sim_related);
    println!();
    println!("  \"{}\"\n  vs\n  \"{}\"", text1, text3);
    println!("  Similarity: {:.4}", sim_unrelated);

    Ok(())
}

/// Batch embedding with parallel processing
async fn batch_embedding_example() -> Result<()> {
    println!("\n━━━ Example 2: Batch Embedding ━━━");

    // Create embedder with custom configuration
    let mut embedder = EmbedderBuilder::new()
        .pretrained(PretrainedModel::AllMiniLmL6V2)
        .pooling(PoolingStrategy::Mean)
        .normalize(true)
        .batch_size(64)
        .build()
        .await?;

    let texts = vec![
        "Artificial intelligence is transforming technology.",
        "Machine learning models learn from data.",
        "Deep learning uses neural networks.",
        "Natural language processing understands text.",
        "Computer vision analyzes images.",
        "Reinforcement learning optimizes decisions.",
        "Vector databases enable semantic search.",
        "Embeddings capture semantic meaning.",
    ];

    println!("Embedding {} texts...", texts.len());

    let start = std::time::Instant::now();
    let output = embedder.embed(&texts)?;
    let elapsed = start.elapsed();

    println!("Completed in {:?}", elapsed);
    println!("Total embeddings: {}", output.len());
    println!("Embedding dimension: {}", output.dimension);

    // Show token counts
    println!("\nToken counts per text:");
    for (i, (text, tokens)) in texts.iter().zip(output.token_counts.iter()).enumerate() {
        println!("  [{}] {} tokens: \"{}...\"", i, tokens, &text[..40.min(text.len())]);
    }

    Ok(())
}

/// Semantic search with RuVector
async fn semantic_search_example() -> Result<()> {
    println!("\n━━━ Example 3: Semantic Search with RuVector ━━━");

    // Create embedder
    let embedder = Embedder::default_model().await?;

    // Create RuVector index
    let index = RuVectorBuilder::new("semantic_search")
        .embedder(embedder)
        .distance(Distance::Cosine)
        .max_elements(10_000)
        .build()?;

    // Knowledge base about programming languages
    let documents = vec![
        "Rust is a systems programming language focused on safety and performance.",
        "Python is widely used for machine learning and data science applications.",
        "JavaScript is the language of the web, running in browsers everywhere.",
        "Go is designed for building scalable and efficient server applications.",
        "TypeScript adds static typing to JavaScript for better developer experience.",
        "C++ provides low-level control and high performance for system software.",
        "Java is a mature, object-oriented language popular in enterprise software.",
        "Swift is Apple's modern language for iOS and macOS development.",
        "Kotlin is a concise language that runs on the JVM, popular for Android.",
        "Haskell is a purely functional programming language with strong typing.",
    ];

    println!("Indexing {} documents...", documents.len());
    index.insert_batch(&documents)?;

    println!("Index size: {} vectors", index.len());

    // Perform searches
    let queries = vec![
        "What language is best for web development?",
        "I want to build a high-performance system application",
        "Which language should I learn for machine learning?",
        "I need a language for mobile app development",
    ];

    for query in queries {
        println!("\n🔍 Query: \"{}\"", query);
        let results = index.search(query, 3)?;

        for (i, result) in results.iter().enumerate() {
            println!(
                "  {}. (score: {:.4}) {}",
                i + 1,
                result.score,
                result.text
            );
        }
    }

    Ok(())
}

/// RAG (Retrieval-Augmented Generation) pipeline
async fn rag_pipeline_example() -> Result<()> {
    println!("\n━━━ Example 4: RAG Pipeline ━━━");

    let embedder = Embedder::default_model().await?;

    let index = RuVectorEmbeddings::new_default("rag_index", embedder)?;
    let rag = RagPipeline::new(index, 3);

    // Add knowledge base
    let knowledge = vec![
        "RuVector is a distributed vector database that learns and adapts.",
        "RuVector uses HNSW indexing for fast approximate nearest neighbor search.",
        "The embedding dimension in RuVector is configurable based on your model.",
        "RuVector supports multiple distance metrics: Cosine, Euclidean, and Dot Product.",
        "Graph Neural Networks in RuVector improve search quality over time.",
        "RuVector integrates with ONNX models for native embedding generation.",
        "The NAPI-RS bindings allow using RuVector from Node.js applications.",
        "RuVector supports WebAssembly for running in web browsers.",
        "Raft consensus enables distributed deployment of RuVector clusters.",
        "Quantization in RuVector provides 2-32x memory compression.",
    ];

    println!("Loading {} documents into RAG pipeline...", knowledge.len());
    rag.add_documents(&knowledge)?;

    // Generate context for questions
    let questions = vec![
        "How does RuVector achieve fast search?",
        "Can I use RuVector in a web browser?",
        "What compression options does RuVector have?",
    ];

    for question in questions {
        println!("\n❓ Question: {}", question);
        let context = rag.format_context(question)?;
        println!("Generated Context:\n{}", context);
        println!("{}", "─".repeat(60));
    }

    Ok(())
}

/// Text clustering example
async fn clustering_example() -> Result<()> {
    println!("\n━━━ Example 5: Text Clustering ━━━");

    let mut embedder = Embedder::default_model().await?;

    // Texts from different categories
    let texts = vec![
        // Technology
        "Artificial intelligence is revolutionizing industries.",
        "Machine learning algorithms process large datasets.",
        "Neural networks mimic the human brain.",
        // Sports
        "Football is the most popular sport worldwide.",
        "Basketball requires speed and agility.",
        "Tennis is played on different court surfaces.",
        // Food
        "Italian pasta comes in many shapes and sizes.",
        "Sushi is a traditional Japanese dish.",
        "French cuisine is known for its elegance.",
    ];

    println!("Clustering {} texts into 3 categories...", texts.len());

    let clusters = embedder.cluster(&texts, 3)?;

    // Group texts by cluster
    let mut groups: std::collections::HashMap<usize, Vec<&str>> = std::collections::HashMap::new();
    for (i, &cluster) in clusters.iter().enumerate() {
        groups.entry(cluster).or_default().push(texts[i]);
    }

    println!("\nCluster assignments:");
    for (cluster_id, members) in groups.iter() {
        println!("\n📁 Cluster {}:", cluster_id);
        for text in members {
            println!("   • {}", text);
        }
    }

    Ok(())
}
