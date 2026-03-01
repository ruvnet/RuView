//! bioRxiv and medRxiv Preprint Discovery Example
//!
//! This example demonstrates how to use the bioRxiv and medRxiv API clients
//! to fetch preprints and convert them to SemanticVectors for discovery.
//!
//! Run with:
//! ```bash
//! cargo run --example biorxiv_discovery
//! ```

use chrono::NaiveDate;
use ruvector_data_framework::{BiorxivClient, MedrxivClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== bioRxiv Preprint Discovery ===\n");

    // 1. Create bioRxiv client for life sciences preprints
    let biorxiv = BiorxivClient::new();

    // Get recent neuroscience preprints
    println!("Fetching recent neuroscience preprints from bioRxiv...");
    match biorxiv.search_by_category("neuroscience", 5).await {
        Ok(papers) => {
            println!("Found {} neuroscience papers:\n", papers.len());
            for (i, paper) in papers.iter().enumerate() {
                let title = paper.metadata.get("title").map(|s| s.as_str()).unwrap_or("Untitled");
                let doi = paper.metadata.get("doi").map(|s| s.as_str()).unwrap_or("No DOI");
                let category = paper.metadata.get("category").map(|s| s.as_str()).unwrap_or("Unknown");

                println!("{}. {}", i + 1, title);
                println!("   DOI: {}", doi);
                println!("   Category: {}", category);
                println!("   Published: {}", paper.timestamp.format("%Y-%m-%d"));
                println!("   Vector ID: {}", paper.id);
                println!("   Embedding dim: {}", paper.embedding.len());
                println!();
            }
        }
        Err(e) => println!("Error fetching papers: {}", e),
    }

    // 2. Search by date range
    println!("Fetching bioRxiv papers from January 2024...");
    let start = NaiveDate::from_ymd_opt(2024, 1, 1).expect("Valid date");
    let end = NaiveDate::from_ymd_opt(2024, 1, 31).expect("Valid date");

    match biorxiv.search_by_date_range(start, end, Some(3)).await {
        Ok(papers) => {
            println!("Found {} papers from January 2024:\n", papers.len());
            for (i, paper) in papers.iter().enumerate() {
                let title = paper.metadata.get("title").map(|s| s.as_str()).unwrap_or("Untitled");
                let authors = paper.metadata.get("authors").map(|s| s.as_str()).unwrap_or("Unknown");

                println!("{}. {}", i + 1, title);
                println!("   Authors: {}", &authors[..authors.len().min(100)]);
                println!();
            }
        }
        Err(e) => println!("Error: {}", e),
    }

    println!("\n=== medRxiv Medical Preprint Discovery ===\n");

    // 3. Create medRxiv client for medical preprints
    let medrxiv = MedrxivClient::new();

    // Search COVID-19 related papers
    println!("Fetching COVID-19 related preprints from medRxiv...");
    match medrxiv.search_covid(5).await {
        Ok(papers) => {
            println!("Found {} COVID-19 papers:\n", papers.len());
            for (i, paper) in papers.iter().enumerate() {
                let title = paper.metadata.get("title").map(|s| s.as_str()).unwrap_or("Untitled");
                let doi = paper.metadata.get("doi").map(|s| s.as_str()).unwrap_or("No DOI");
                let published = paper.metadata.get("published_status").map(|s| s.as_str()).unwrap_or("preprint");

                println!("{}. {}", i + 1, title);
                println!("   DOI: {}", doi);
                println!("   Status: {}", published);
                println!("   Date: {}", paper.timestamp.format("%Y-%m-%d"));
                println!();
            }
        }
        Err(e) => println!("Error: {}", e),
    }

    // 4. Search clinical research papers
    println!("Fetching clinical research preprints from medRxiv...");
    match medrxiv.search_clinical(3).await {
        Ok(papers) => {
            println!("Found {} clinical research papers:\n", papers.len());
            for (i, paper) in papers.iter().enumerate() {
                let title = paper.metadata.get("title").map(|s| s.as_str()).unwrap_or("Untitled");
                let category = paper.metadata.get("category").map(|s| s.as_str()).unwrap_or("Unknown");

                println!("{}. {}", i + 1, title);
                println!("   Category: {}", category);
                println!();
            }
        }
        Err(e) => println!("Error: {}", e),
    }

    // 5. Get recent papers from both sources
    println!("Fetching recent papers from both bioRxiv and medRxiv...");

    let biorxiv_recent = biorxiv.search_recent(7, 2).await?;
    let medrxiv_recent = medrxiv.search_recent(7, 2).await?;

    println!("\nRecent from bioRxiv (last 7 days): {} papers", biorxiv_recent.len());
    println!("Recent from medRxiv (last 7 days): {} papers", medrxiv_recent.len());

    // Combine both for cross-domain analysis
    let mut all_papers = biorxiv_recent;
    all_papers.extend(medrxiv_recent);

    println!("\nTotal papers for discovery: {}", all_papers.len());
    println!("\nDomain distribution:");

    use ruvector_data_framework::Domain;
    let research_count = all_papers.iter().filter(|p| p.domain == Domain::Research).count();
    let medical_count = all_papers.iter().filter(|p| p.domain == Domain::Medical).count();

    println!("  Research domain: {}", research_count);
    println!("  Medical domain: {}", medical_count);

    println!("\n=== Discovery Complete ===");
    println!("\nThese SemanticVectors can now be used with:");
    println!("  - RuVector's vector database for similarity search");
    println!("  - Graph coherence analysis for pattern detection");
    println!("  - Cross-domain discovery for finding connections");
    println!("  - Time-series analysis for trend detection");

    Ok(())
}
