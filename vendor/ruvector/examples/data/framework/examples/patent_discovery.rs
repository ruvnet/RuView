//! Patent Discovery Example
//!
//! Demonstrates using the USPTO PatentsView API client to discover patent data
//! and analyze innovation trends across different technology domains.
//!
//! # Usage
//! ```bash
//! cargo run --example patent_discovery
//! ```

use ruvector_data_framework::{Result, UsptoPatentClient};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔬 Patent Discovery Demo\n");

    // Create USPTO client (no authentication required)
    let client = UsptoPatentClient::new()?;

    // Example 1: Search for quantum computing patents
    println!("📊 Searching for quantum computing patents...");
    match client.search_patents("quantum computing", 5).await {
        Ok(patents) => {
            println!("Found {} patents:", patents.len());
            for (i, patent) in patents.iter().enumerate() {
                println!("\n{}. Patent: {}", i + 1, patent.id);
                if let Some(title) = patent.metadata.get("title") {
                    println!("   Title: {}", title);
                }
                if let Some(assignee) = patent.metadata.get("assignee") {
                    println!("   Assignee: {}", assignee);
                }
                if let Some(cpc) = patent.metadata.get("cpc_codes") {
                    println!("   CPC Codes: {}", cpc);
                }
                println!("   Timestamp: {}", patent.timestamp);
            }
        }
        Err(e) => {
            println!("Error: {}. Skipping this example.", e);
        }
    }

    // Example 2: Search patents by company
    println!("\n\n📊 Searching for Tesla patents...");
    match client.search_by_assignee("Tesla", 3).await {
        Ok(patents) => {
            println!("Found {} Tesla patents:", patents.len());
            for patent in &patents {
                if let Some(title) = patent.metadata.get("title") {
                    println!("  - {} ({})", title, patent.id);
                }
            }
        }
        Err(e) => {
            println!("Error: {}. Skipping this example.", e);
        }
    }

    // Example 3: Search climate change mitigation technologies (CPC Y02)
    println!("\n\n🌍 Searching for climate tech patents (CPC Y02)...");
    match client.search_by_cpc("Y02E", 5).await {
        Ok(patents) => {
            println!("Found {} climate tech patents:", patents.len());
            for patent in &patents {
                if let Some(title) = patent.metadata.get("title") {
                    let cpc = patent.metadata.get("cpc_codes").map(|s| s.as_str()).unwrap_or("N/A");
                    println!("  - {} (CPC: {})", title, cpc);
                }
            }
        }
        Err(e) => {
            println!("Error: {}. Skipping this example.", e);
        }
    }

    // Example 4: Get specific patent details
    println!("\n\n🔍 Getting details for a specific patent...");
    match client.get_patent("10000000").await {
        Ok(Some(patent)) => {
            println!("Patent Details:");
            println!("  ID: {}", patent.id);
            println!("  Title: {}", patent.metadata.get("title").map(|s| s.as_str()).unwrap_or("N/A"));
            println!("  Abstract: {}",
                patent.metadata.get("abstract")
                    .map(|s| if s.len() > 200 { format!("{}...", &s[..200]) } else { s.clone() })
                    .unwrap_or_else(|| "N/A".to_string())
            );
            println!("  Domain: {:?}", patent.domain);
            println!("  Embedding dimension: {}", patent.embedding.len());
        }
        Ok(None) => {
            println!("Patent not found");
        }
        Err(e) => {
            println!("Error: {}. Skipping this example.", e);
        }
    }

    // Example 5: AI/ML patents (CPC G06N)
    println!("\n\n🤖 Searching for AI/ML patents (CPC G06N)...");
    match client.search_by_cpc("G06N", 5).await {
        Ok(patents) => {
            println!("Found {} AI/ML patents:", patents.len());
            for patent in &patents {
                if let Some(title) = patent.metadata.get("title") {
                    let citations = patent.metadata.get("citations_count").map(|s| s.as_str()).unwrap_or("0");
                    println!("  - {} (Citations: {})", title, citations);
                }
            }
        }
        Err(e) => {
            println!("Error: {}. Skipping this example.", e);
        }
    }

    println!("\n✅ Patent discovery complete!");

    Ok(())
}
