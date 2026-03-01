//! Wikipedia and Wikidata Knowledge Graph Discovery
//!
//! This example demonstrates using Wikipedia and Wikidata APIs to build
//! knowledge graphs with semantic search and relationship extraction.
//!
//! Usage:
//! ```bash
//! cargo run --example wiki_discovery
//! ```

use ruvector_data_framework::{
    WikipediaClient, WikidataClient,
    DiscoveryPipeline, PipelineConfig,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🌍 Wikipedia and Wikidata Knowledge Graph Discovery\n");

    // ========================================================================
    // Example 1: Search Wikipedia for Climate Change
    // ========================================================================
    println!("📚 Example 1: Wikipedia Climate Change Articles");
    println!("{}", "=".repeat(60));

    let wiki_client = WikipediaClient::new("en".to_string())?;
    let climate_articles = wiki_client.search("climate change", 5).await?;

    println!("Found {} articles:", climate_articles.len());
    for article in &climate_articles {
        let title = article.data.get("title").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let url = article.data.get("url").and_then(|v| v.as_str()).unwrap_or("");
        println!("  📄 {} - {}", title, url);
        println!("     Relationships: {}", article.relationships.len());
    }
    println!();

    // ========================================================================
    // Example 2: Get Specific Wikipedia Article with Links
    // ========================================================================
    println!("📖 Example 2: Detailed Article with Links");
    println!("{}", "=".repeat(60));

    let article = wiki_client.get_article("Artificial intelligence").await?;
    println!("Title: {}", article.data.get("title").and_then(|v| v.as_str()).unwrap_or(""));
    println!("Extract length: {} chars",
        article.data.get("extract").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0));
    println!("Categories: {}",
        article.relationships.iter().filter(|r| r.rel_type == "in_category").count());
    println!("Links: {}",
        article.relationships.iter().filter(|r| r.rel_type == "links_to").count());
    println!();

    // ========================================================================
    // Example 3: Wikidata Entity Search
    // ========================================================================
    println!("🔍 Example 3: Wikidata Entity Search");
    println!("{}", "=".repeat(60));

    let wikidata_client = WikidataClient::new()?;
    let entities = wikidata_client.search_entities("machine learning").await?;

    println!("Found {} entities:", entities.len().min(5));
    for entity in entities.iter().take(5) {
        println!("  🏷️  {} ({})", entity.label, entity.qid);
        println!("     {}", entity.description);
    }
    println!();

    // ========================================================================
    // Example 4: Wikidata SPARQL - Climate Change Entities
    // ========================================================================
    println!("🌡️  Example 4: Climate Change Entities via SPARQL");
    println!("{}", "=".repeat(60));

    let climate_entities = wikidata_client.query_climate_entities().await?;
    println!("Found {} climate-related entities", climate_entities.len());

    for entity in climate_entities.iter().take(10) {
        let label = entity.data.get("label").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let description = entity.data.get("description").and_then(|v| v.as_str()).unwrap_or("");
        println!("  🌍 {} - {}", label, description);
    }
    println!();

    // ========================================================================
    // Example 5: Wikidata SPARQL - Pharmaceutical Companies
    // ========================================================================
    println!("💊 Example 5: Pharmaceutical Companies via SPARQL");
    println!("{}", "=".repeat(60));

    let pharma_companies = wikidata_client.query_pharmaceutical_companies().await?;
    println!("Found {} pharmaceutical companies", pharma_companies.len());

    for company in pharma_companies.iter().take(10) {
        let label = company.data.get("label").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let founded = company.data.get("founded").and_then(|v| v.as_str()).unwrap_or("N/A");
        println!("  🏢 {} (founded: {})", label, founded);
    }
    println!();

    // ========================================================================
    // Example 6: Wikidata SPARQL - Disease Outbreaks
    // ========================================================================
    println!("🦠 Example 6: Disease Outbreaks via SPARQL");
    println!("{}", "=".repeat(60));

    let outbreaks = wikidata_client.query_disease_outbreaks().await?;
    println!("Found {} disease outbreak records", outbreaks.len());

    for outbreak in outbreaks.iter().take(10) {
        let label = outbreak.data.get("label").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let disease = outbreak.data.get("diseaseLabel").and_then(|v| v.as_str()).unwrap_or("Unknown disease");
        let location = outbreak.data.get("locationLabel").and_then(|v| v.as_str()).unwrap_or("Unknown location");
        println!("  🦠 {} - {} in {}", label, disease, location);
    }
    println!();

    // ========================================================================
    // Example 7: Full Discovery Pipeline with Wikipedia
    // ========================================================================
    println!("🔬 Example 7: Full Discovery Pipeline");
    println!("{}", "=".repeat(60));

    let config = PipelineConfig::default();
    let mut pipeline = DiscoveryPipeline::new(config);

    println!("Running discovery on Wikipedia climate data...");
    let patterns = pipeline.run(wiki_client).await?;

    let stats = pipeline.stats();
    println!("\n📊 Discovery Statistics:");
    println!("  Records processed: {}", stats.records_processed);
    println!("  Nodes created: {}", stats.nodes_created);
    println!("  Edges created: {}", stats.edges_created);
    println!("  Patterns discovered: {}", stats.patterns_discovered);
    println!("  Duration: {}ms", stats.duration_ms);

    // Export results
    let output_dir = "./wiki_discovery_output";
    std::fs::create_dir_all(output_dir)?;

    println!("\n💾 Exporting results to {}/", output_dir);

    // Export patterns to CSV
    use std::io::Write;
    let patterns_file = format!("{}/patterns.csv", output_dir);
    let mut file = std::fs::File::create(&patterns_file)?;
    writeln!(file, "category,strength,description,node_count")?;
    for pattern in &patterns {
        writeln!(file, "{:?},{:?},{},{}", pattern.category, pattern.strength, pattern.description.replace(",", ";"), pattern.entities.len())?;
    }

    println!("  ✓ patterns.csv - Pattern metadata ({} patterns)", patterns.len());
    println!();

    // ========================================================================
    // Example 8: Custom SPARQL Query
    // ========================================================================
    println!("⚡ Example 8: Custom SPARQL Query - Nobel Laureates");
    println!("{}", "=".repeat(60));

    let custom_query = r#"
SELECT ?item ?itemLabel ?awardLabel ?year WHERE {
  ?item wdt:P166 ?award.
  ?award wdt:P279* wd:Q7191.  # Nobel Prize
  OPTIONAL { ?award wdt:P585 ?year. }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
ORDER BY DESC(?year)
LIMIT 20
"#;

    let results = wikidata_client.sparql_query(custom_query).await?;
    println!("Found {} Nobel laureates (recent 20):", results.len());

    for result in results.iter().take(10) {
        let name = result.get("itemLabel").map(|s| s.as_str()).unwrap_or("Unknown");
        let award = result.get("awardLabel").map(|s| s.as_str()).unwrap_or("Nobel Prize");
        let year = result.get("year").map(|s| &s[..4]).unwrap_or("N/A");
        println!("  🏆 {} - {} ({})", name, award, year);
    }
    println!();

    println!("✨ Knowledge graph discovery complete!");

    Ok(())
}
