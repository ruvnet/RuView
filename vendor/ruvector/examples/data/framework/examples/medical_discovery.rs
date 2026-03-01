//! Medical data discovery example
//!
//! Demonstrates integration with PubMed, ClinicalTrials.gov, and FDA APIs
//! for discovering patterns in medical literature and clinical data.

use ruvector_data_framework::{
    ClinicalTrialsClient, FdaClient, PubMedClient,
    ruvector_native::{Domain, NativeDiscoveryEngine, NativeEngineConfig},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🏥 RuVector Medical Data Discovery Example\n");

    // Initialize discovery engine with Medical domain support
    let config = NativeEngineConfig {
        similarity_threshold: 0.7,
        mincut_sensitivity: 0.15,
        cross_domain: true,
        ..Default::default()
    };
    let mut engine = NativeDiscoveryEngine::new(config);

    println!("📚 Step 1: Searching PubMed for COVID-19 research...");
    let pubmed_client = PubMedClient::new(None)?;
    let pubmed_vectors = pubmed_client
        .search_articles("COVID-19 treatment", 10)
        .await?;

    println!("   Found {} articles", pubmed_vectors.len());
    for vector in &pubmed_vectors {
        let title = vector.metadata.get("title").map(String::as_str).unwrap_or("Untitled");
        println!("   - {}", title);

        // Add to discovery engine
        engine.add_vector(vector.clone());
    }

    println!("\n🧪 Step 2: Searching ClinicalTrials.gov for diabetes trials...");
    let trials_client = ClinicalTrialsClient::new()?;
    let trial_vectors = trials_client
        .search_trials("diabetes", Some("RECRUITING"))
        .await?;

    println!("   Found {} trials", trial_vectors.len());
    for vector in &trial_vectors {
        let title = vector.metadata.get("title").map(String::as_str).unwrap_or("Untitled");
        let status = vector.metadata.get("status").map(String::as_str).unwrap_or("UNKNOWN");
        println!("   - {} [{}]", title, status);

        // Add to discovery engine
        engine.add_vector(vector.clone());
    }

    println!("\n💊 Step 3: Searching FDA adverse events for aspirin...");
    let fda_client = FdaClient::new()?;
    let event_vectors = fda_client
        .search_drug_events("aspirin")
        .await?;

    println!("   Found {} adverse event reports", event_vectors.len());
    if !event_vectors.is_empty() {
        for vector in event_vectors.iter().take(5) {
            let drugs = vector.metadata.get("drugs").map(String::as_str).unwrap_or("Unknown");
            let reactions = vector.metadata.get("reactions").map(String::as_str).unwrap_or("Unknown");
            println!("   - Drugs: {} | Reactions: {}", drugs, reactions);

            // Add to discovery engine
            engine.add_vector(vector.clone());
        }
    }

    println!("\n📊 Discovery Engine Statistics:");
    let stats = engine.stats();
    println!("   Total nodes: {}", stats.total_nodes);
    println!("   Total edges: {}", stats.total_edges);
    println!("   Total vectors: {}", stats.total_vectors);
    println!("   Cross-domain edges: {}", stats.cross_domain_edges);

    if let Some(count) = stats.domain_counts.get(&Domain::Medical) {
        println!("   Medical domain nodes: {}", count);
    }

    println!("\n🔍 Step 4: Computing coherence and detecting patterns...");
    let coherence = engine.compute_coherence();
    println!("   Min-cut value: {:.3}", coherence.mincut_value);
    println!("   Partition sizes: {:?}", coherence.partition_sizes);
    println!("   Boundary nodes: {}", coherence.boundary_nodes.len());

    // Detect patterns
    let patterns = engine.detect_patterns();
    println!("\n✨ Detected {} patterns:", patterns.len());
    for pattern in &patterns {
        println!("\n   Pattern: {:?}", pattern.pattern_type);
        println!("   Confidence: {:.2}", pattern.confidence);
        println!("   Description: {}", pattern.description);
        println!("   Affected nodes: {}", pattern.affected_nodes.len());

        if !pattern.cross_domain_links.is_empty() {
            println!("   Cross-domain connections: {}", pattern.cross_domain_links.len());
        }
    }

    println!("\n✅ Medical discovery complete!");

    Ok(())
}
