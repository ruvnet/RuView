//! ArXiv Preprint Discovery Example
//!
//! Demonstrates how to use the ArxivClient to fetch and analyze academic papers
//! from ArXiv.org across multiple research domains.
//!
//! Run with:
//! ```bash
//! cargo run --example arxiv_discovery
//! ```

use ruvector_data_framework::{ArxivClient, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== ArXiv Discovery Example ===\n");

    let client = ArxivClient::new();

    // Example 1: Search by keywords
    println!("1. Searching for 'quantum computing' papers...");
    match client.search("quantum computing", 5).await {
        Ok(papers) => {
            println!("   Found {} papers", papers.len());
            for paper in papers.iter().take(3) {
                println!("   - {}", paper.metadata.get("title").map_or("N/A", |s| s.as_str()));
                println!("     ArXiv ID: {}", paper.id);
                println!("     Authors: {}", paper.metadata.get("authors").map_or("N/A", |s| s.as_str()));
                println!();
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Example 2: Search by category (AI papers)
    println!("\n2. Fetching latest AI papers (cs.AI)...");
    match client.search_category("cs.AI", 5).await {
        Ok(papers) => {
            println!("   Found {} AI papers", papers.len());
            let default_text = "N/A".to_string();
            for paper in papers.iter().take(2) {
                println!("   - {}", paper.metadata.get("title").unwrap_or(&default_text));
                let abstract_text = paper.metadata.get("abstract").unwrap_or(&default_text);
                let preview = if abstract_text.len() > 150 {
                    format!("{}...", &abstract_text[..150])
                } else {
                    abstract_text.clone()
                };
                println!("     Abstract: {}", preview);
                println!();
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Example 3: Get recent papers in Machine Learning
    println!("\n3. Getting recent Machine Learning papers (last 7 days)...");
    match client.search_recent("cs.LG", 7).await {
        Ok(papers) => {
            println!("   Found {} recent ML papers", papers.len());
            for paper in papers.iter().take(3) {
                println!("   - {}", paper.metadata.get("title").map_or("N/A", |s| s.as_str()));
                println!("     Published: {}", paper.timestamp.format("%Y-%m-%d"));
                println!("     PDF: {}", paper.metadata.get("pdf_url").map_or("N/A", |s| s.as_str()));
                println!();
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Example 4: Get a specific paper by ID
    println!("\n4. Fetching a specific paper by ArXiv ID...");
    // Note: This is a real ArXiv ID - the famous "Attention is All You Need" paper
    match client.get_paper("1706.03762").await {
        Ok(Some(paper)) => {
            println!("   ✓ Found paper:");
            println!("     Title: {}", paper.metadata.get("title").map_or("N/A", |s| s.as_str()));
            println!("     Authors: {}", paper.metadata.get("authors").map_or("N/A", |s| s.as_str()));
            println!("     Categories: {}", paper.metadata.get("categories").map_or("N/A", |s| s.as_str()));
            println!("     Published: {}", paper.timestamp.format("%Y-%m-%d"));
        }
        Ok(None) => println!("   Paper not found"),
        Err(e) => println!("   Error: {}", e),
    }

    // Example 5: Multi-category search
    println!("\n5. Searching across multiple AI/ML categories...");
    let categories = vec!["cs.AI", "cs.LG", "stat.ML"];
    match client.search_multiple_categories(&categories, 3).await {
        Ok(papers) => {
            println!("   Found {} papers across {} categories", papers.len(), categories.len());

            // Group by category
            let mut by_category: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
            for paper in papers {
                let cats = paper.metadata.get("categories")
                    .map(|s| s.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                by_category.entry(cats).or_insert_with(Vec::new).push(paper);
            }

            for (category, cat_papers) in by_category.iter() {
                println!("   {} papers with categories: {}", cat_papers.len(), category);
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Example 6: Climate science papers
    println!("\n6. Fetching climate science papers (physics.ao-ph)...");
    match client.search_category("physics.ao-ph", 5).await {
        Ok(papers) => {
            println!("   Found {} climate papers", papers.len());
            for paper in papers.iter().take(2) {
                println!("   - {}", paper.metadata.get("title").map_or("N/A", |s| s.as_str()));
                println!("     Domain: {:?}", paper.domain);
                println!();
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    // Example 7: Quantitative Finance papers
    println!("\n7. Fetching quantitative finance papers (q-fin.ST)...");
    match client.search_category("q-fin.ST", 3).await {
        Ok(papers) => {
            println!("   Found {} finance papers", papers.len());
            for paper in papers {
                println!("   - {}", paper.metadata.get("title").map_or("N/A", |s| s.as_str()));
                println!("     Embedding dim: {}", paper.embedding.len());
            }
        }
        Err(e) => println!("   Error: {}", e),
    }

    println!("\n=== Discovery Complete ===");
    println!("\nNote: All papers are converted to SemanticVector format with:");
    println!("  - ID: ArXiv paper ID");
    println!("  - Embedding: Generated from title + abstract (384 dimensions)");
    println!("  - Domain: Research");
    println!("  - Metadata: title, abstract, authors, categories, pdf_url");
    println!("\nThese vectors can be ingested into RuVector for:");
    println!("  - Semantic similarity search");
    println!("  - Cross-domain pattern discovery");
    println!("  - Citation network analysis");
    println!("  - Temporal trend detection");

    Ok(())
}
