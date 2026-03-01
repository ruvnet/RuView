//! Demonstration of real API client integrations
//!
//! This example shows how to use the OpenAlex, NOAA, and SEC EDGAR clients
//! to fetch real data and convert it to RuVector's DataRecord format.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example api_client_demo
//! ```

use ruvector_data_framework::api_clients::{EdgarClient, NoaaClient, OpenAlexClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== RuVector API Client Demo ===\n");

    // 1. OpenAlex - Academic works
    println!("1. Fetching academic works from OpenAlex...");
    let openalex = OpenAlexClient::new(Some("demo@ruvector.io".to_string()))?;

    match openalex.fetch_works("quantum computing", 5).await {
        Ok(works) => {
            println!("   Found {} academic works", works.len());
            for work in works.iter().take(3) {
                if let Some(title) = work.data.get("title") {
                    println!("   - {} (ID: {})", title.as_str().unwrap_or("N/A"), work.id);
                    if let Some(embedding) = &work.embedding {
                        println!(
                            "     Embedding: [{:.3}, {:.3}, ..., {:.3}] (dim={})",
                            embedding[0],
                            embedding[1],
                            embedding[embedding.len() - 1],
                            embedding.len()
                        );
                    }
                }
            }
        }
        Err(e) => println!("   Error: {}", e),
    }
    println!();

    // 2. OpenAlex - Topics
    println!("2. Fetching research topics from OpenAlex...");
    match openalex.fetch_topics("artificial intelligence").await {
        Ok(topics) => {
            println!("   Found {} topics", topics.len());
            for topic in topics.iter().take(3) {
                if let Some(name) = topic.data.get("display_name") {
                    println!("   - {}", name.as_str().unwrap_or("N/A"));
                }
            }
        }
        Err(e) => println!("   Error: {}", e),
    }
    println!();

    // 3. NOAA - Climate data (using synthetic data since no API token)
    println!("3. Fetching climate data from NOAA...");
    let noaa = NoaaClient::new(None)?;

    match noaa
        .fetch_climate_data("GHCND:USW00094728", "2024-01-01", "2024-01-31")
        .await
    {
        Ok(observations) => {
            println!(
                "   Found {} climate observations (synthetic data)",
                observations.len()
            );
            for obs in observations.iter().take(3) {
                if let (Some(datatype), Some(value)) =
                    (obs.data.get("datatype"), obs.data.get("value"))
                {
                    println!(
                        "   - {}: {} (type: {})",
                        datatype.as_str().unwrap_or("N/A"),
                        value.as_f64().unwrap_or(0.0),
                        obs.record_type
                    );
                }
            }
        }
        Err(e) => println!("   Error: {}", e),
    }
    println!();

    // 4. SEC EDGAR - Company filings
    println!("4. Fetching SEC filings from EDGAR...");
    let edgar = EdgarClient::new("RuVector-Demo demo@ruvector.io".to_string())?;

    // Apple Inc. CIK: 0000320193
    match edgar.fetch_filings("320193", Some("10-K")).await {
        Ok(filings) => {
            println!("   Found {} 10-K filings for Apple Inc.", filings.len());
            for filing in filings.iter().take(3) {
                if let (Some(form), Some(date)) =
                    (filing.data.get("form"), filing.data.get("filing_date"))
                {
                    println!(
                        "   - Form {}: filed on {}",
                        form.as_str().unwrap_or("N/A"),
                        date.as_str().unwrap_or("N/A")
                    );
                }
            }
        }
        Err(e) => println!("   Error: {}", e),
    }
    println!();

    // 5. Demonstrate DataSource trait
    println!("5. Using DataSource trait...");
    use ruvector_data_framework::DataSource;

    let source = openalex;
    println!("   Source ID: {}", source.source_id());

    match source.health_check().await {
        Ok(healthy) => println!("   Health check: {}", if healthy { "OK" } else { "FAILED" }),
        Err(e) => println!("   Health check error: {}", e),
    }

    match source.fetch_batch(None, 3).await {
        Ok((records, cursor)) => {
            println!("   Fetched {} records", records.len());
            println!("   Next cursor: {:?}", cursor);
        }
        Err(e) => println!("   Batch fetch error: {}", e),
    }

    println!("\n=== Demo Complete ===");
    println!("\nKey Features Demonstrated:");
    println!("  - OpenAlex: Academic works and topics with embeddings");
    println!("  - NOAA: Climate observations (synthetic without API token)");
    println!("  - SEC EDGAR: Company filings with metadata");
    println!("  - DataSource trait: Health checks and batch fetching");
    println!("  - Simple embeddings: Bag-of-words text vectors");

    Ok(())
}
