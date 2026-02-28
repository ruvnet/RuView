//! Real Data Discovery Example
//!
//! Fetches actual climate-finance research papers from OpenAlex API
//! and runs RuVector's discovery engine to find:
//! - Cross-topic bridges
//! - Emerging research clusters
//! - Pattern trends and anomalies
//!
//! This demonstrates real-world discovery on live academic data.
//!
//! ## Embedder Options
//! - Default: SimpleEmbedder (bag-of-words, fast but low quality)
//! - With `onnx-embeddings` feature: OnnxEmbedder (neural, high quality)
//!
//! Run with ONNX:
//! ```bash
//! cargo run --example real_data_discovery --features onnx-embeddings --release
//! ```

use std::collections::HashMap;
use std::time::Instant;

use ruvector_data_framework::{
    CoherenceConfig, CoherenceEngine, DiscoveryConfig, DiscoveryEngine, OpenAlexClient,
    PatternCategory, SimpleEmbedder, Embedder,
};

#[cfg(feature = "onnx-embeddings")]
use ruvector_data_framework::OnnxEmbedder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║     Real Climate-Finance Research Discovery with OpenAlex    ║");
    println!("║              Powered by RuVector Discovery Engine            ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let start = Instant::now();

    // ============================================================================
    // Phase 1: Fetch Real Data from OpenAlex
    // ============================================================================
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📡 Phase 1: Fetching Research Papers from OpenAlex API");
    println!();

    // Create OpenAlex client (polite API usage)
    let client = OpenAlexClient::new(Some("ruvector-demo@example.com".to_string()))?;

    // Define research queries covering climate-finance intersection
    let queries = vec![
        ("climate_risk_finance", "climate risk finance", 20),
        ("stranded_assets", "stranded assets energy", 15),
        ("carbon_pricing", "carbon pricing markets", 15),
        ("physical_climate_risk", "physical climate risk", 15),
        ("transition_risk", "transition risk disclosure", 15),
    ];

    let mut all_records = Vec::new();
    let mut papers_by_topic: HashMap<String, usize> = HashMap::new();

    println!("   Querying topics:");
    for (topic_id, query, limit) in &queries {
        print!("   • {}: fetching {} papers... ", query, limit);
        std::io::Write::flush(&mut std::io::stdout())?;

        match client.fetch_works(query, *limit).await {
            Ok(records) => {
                println!("✓ {} papers", records.len());
                papers_by_topic.insert(topic_id.to_string(), records.len());
                all_records.extend(records);
            }
            Err(e) => {
                println!("⚠️  API error: {}", e);
                println!("      Falling back to synthetic data for this topic");

                // Generate synthetic data as fallback
                let synthetic = generate_synthetic_papers(topic_id, *limit);
                papers_by_topic.insert(topic_id.to_string(), synthetic.len());
                all_records.extend(synthetic);
            }
        }
    }

    println!();
    println!("   Total papers fetched: {}", all_records.len());
    println!("   Data sources breakdown:");
    for (topic, count) in &papers_by_topic {
        println!("      {} → {} papers", topic, count);
    }

    if all_records.is_empty() {
        println!();
        println!("❌ No data available. Please check your internet connection.");
        return Ok(());
    }

    // ============================================================================
    // Phase 1.5: Re-embed with ONNX (if feature enabled)
    // ============================================================================
    #[cfg(feature = "onnx-embeddings")]
    {
        println!();
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🧠 Phase 1.5: Generating Neural Embeddings (ONNX)");
        println!();
        println!("   Loading MiniLM-L6-v2 model (384-dim semantic embeddings)...");

        let onnx_start = Instant::now();
        match OnnxEmbedder::new().await {
            Ok(embedder) => {
                println!("   ✓ Model loaded in {:?}", onnx_start.elapsed());
                println!("   Embedding {} papers...", all_records.len());

                let embed_start = Instant::now();
                for record in &mut all_records {
                    // Extract text from JSON data for embedding
                    let title = record.data.get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let abstract_text = record.data.get("abstract")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let concepts = record.data.get("concepts")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter()
                            .filter_map(|c| c.get("display_name").and_then(|n| n.as_str()))
                            .collect::<Vec<_>>()
                            .join(" "))
                        .unwrap_or_default();

                    let text = format!("{} {} {}", title, abstract_text, concepts);
                    let embedding = embedder.embed_text(&text);
                    record.embedding = Some(embedding);
                }

                println!("   ✓ Embedded {} papers in {:?}", all_records.len(), embed_start.elapsed());
                println!("   Embedding dimension: 384 (semantic)");
            }
            Err(e) => {
                println!("   ⚠️  ONNX model failed to load: {}", e);
                println!("   Falling back to bag-of-words embeddings");
            }
        }
    }

    #[cfg(not(feature = "onnx-embeddings"))]
    {
        println!();
        println!("   💡 Tip: Enable ONNX embeddings for better discovery quality:");
        println!("      cargo run --example real_data_discovery --features onnx-embeddings --release");
    }

    // ============================================================================
    // Phase 2: Build Coherence Graph
    // ============================================================================
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔗 Phase 2: Building Semantic Coherence Graph");
    println!();

    let coherence_config = CoherenceConfig {
        min_edge_weight: 0.3,      // Moderate similarity threshold
        window_size_secs: 86400 * 365 * 3, // 3 year window (catch all papers)
        window_step_secs: 86400 * 30,      // Monthly steps
        approximate: true,
        epsilon: 0.1,
        parallel: true,
        track_boundaries: true,
        similarity_threshold: 0.5,  // Connect papers with >= 50% similarity
        use_embeddings: true,       // Use ONNX embeddings for edge creation
        hnsw_k_neighbors: 30,       // Search 30 nearest neighbors per paper
        hnsw_min_records: 50,       // Use HNSW for datasets >= 50 records
    };

    let mut coherence = CoherenceEngine::new(coherence_config);

    println!("   Computing coherence signals from {} papers...", all_records.len());
    let signals = match coherence.compute_from_records(&all_records) {
        Ok(sigs) => {
            println!("   ✓ Generated {} coherence signals", sigs.len());
            sigs
        }
        Err(e) => {
            println!("   ⚠️  Coherence computation failed: {}", e);
            println!("      Using simplified analysis");
            vec![] // Continue with empty signals
        }
    };

    // Graph statistics
    println!();
    println!("   Graph Statistics:");
    println!("      Nodes: {}", coherence.node_count());
    println!("      Edges: {}", coherence.edge_count());

    if !signals.is_empty() {
        let avg_min_cut = signals.iter()
            .map(|s| s.min_cut_value)
            .sum::<f64>() / signals.len() as f64;
        let avg_nodes = signals.iter()
            .map(|s| s.node_count)
            .sum::<usize>() / signals.len();

        println!("      Avg min-cut value: {:.3}", avg_min_cut);
        println!("      Avg nodes per window: {}", avg_nodes);
    }

    // ============================================================================
    // Phase 3: Pattern Discovery
    // ============================================================================
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔍 Phase 3: Running Discovery Engine");
    println!();

    let discovery_config = DiscoveryConfig {
        min_signal_strength: 0.01,
        lookback_windows: 5,
        emergence_threshold: 0.15,
        split_threshold: 0.4,
        bridge_threshold: 0.25,
        detect_anomalies: true,
        anomaly_sigma: 2.0,
    };

    let mut discovery = DiscoveryEngine::new(discovery_config);

    println!("   Detecting patterns...");
    let patterns = discovery.detect(&signals)?;

    println!("   ✓ Discovered {} patterns", patterns.len());

    // ============================================================================
    // Phase 4: Analysis & Results
    // ============================================================================
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 Phase 4: Discovery Results");
    println!();

    if patterns.is_empty() {
        println!("   No significant patterns detected in this dataset.");
        println!("   Try adjusting thresholds or fetching more papers.");
    } else {
        // Categorize patterns
        let mut by_category: HashMap<PatternCategory, Vec<_>> = HashMap::new();
        for pattern in &patterns {
            by_category
                .entry(pattern.category)
                .or_default()
                .push(pattern);
        }

        println!("   Pattern Categories:");
        println!();

        // Bridges (most interesting for cross-domain)
        if let Some(bridges) = by_category.get(&PatternCategory::Bridge) {
            println!("   🌉 Cross-Topic Bridges: {}", bridges.len());
            for (i, bridge) in bridges.iter().enumerate().take(3) {
                println!("      {}. {}", i + 1, bridge.description);
                println!("         Confidence: {:.2}", bridge.confidence);
                println!("         Entities: {} papers", bridge.entities.len());
                if !bridge.evidence.is_empty() {
                    println!(
                        "         Evidence: {}",
                        bridge.evidence[0].explanation
                    );
                }
                println!();
            }
        }

        // Emergence
        if let Some(emergence) = by_category.get(&PatternCategory::Emergence) {
            println!("   🌱 Emerging Research Clusters: {}", emergence.len());
            for (i, pattern) in emergence.iter().enumerate().take(2) {
                println!("      {}. {}", i + 1, pattern.description);
                println!("         Strength: {:?}", pattern.strength);
                println!();
            }
        }

        // Consolidation trends
        if let Some(consol) = by_category.get(&PatternCategory::Consolidation) {
            println!("   📈 Consolidating Topics: {}", consol.len());
            for pattern in consol.iter().take(2) {
                println!("      • {}", pattern.description);
            }
            println!();
        }

        // Dissolution trends
        if let Some(dissol) = by_category.get(&PatternCategory::Dissolution) {
            println!("   📉 Fragmenting Topics: {}", dissol.len());
            for pattern in dissol.iter().take(2) {
                println!("      • {}", pattern.description);
            }
            println!();
        }

        // Anomalies
        if let Some(anomalies) = by_category.get(&PatternCategory::Anomaly) {
            println!("   ⚡ Anomalous Coherence Patterns: {}", anomalies.len());
            for (i, anomaly) in anomalies.iter().enumerate().take(2) {
                println!("      {}. {}", i + 1, anomaly.description);
                if !anomaly.evidence.is_empty() {
                    println!(
                        "         {}",
                        anomaly.evidence[0].explanation
                    );
                }
            }
            println!();
        }

        // Splits
        if let Some(splits) = by_category.get(&PatternCategory::Split) {
            println!("   🔀 Research Splits: {}", splits.len());
            for pattern in splits.iter().take(2) {
                println!("      • {}", pattern.description);
            }
            println!();
        }
    }

    // ============================================================================
    // Phase 5: Key Insights
    // ============================================================================
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                      Key Insights                             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    println!("   📚 Dataset Summary:");
    println!("      Total papers analyzed: {}", all_records.len());
    println!("      Research topics covered: {}", papers_by_topic.len());
    println!("      Patterns discovered: {}", patterns.len());
    println!();

    println!("   🔬 Methodology:");
    #[cfg(feature = "onnx-embeddings")]
    println!("      • Semantic embeddings: ONNX MiniLM-L6-v2 (384-dim neural)");
    #[cfg(not(feature = "onnx-embeddings"))]
    println!("      • Semantic embeddings: Simple bag-of-words (128-dim)");
    println!("      • Graph construction: Citation + concept relationships");
    println!("      • Coherence metric: Dynamic minimum cut");
    println!("      • Pattern detection: Multi-signal trend analysis");
    println!();

    println!("   💡 Research Directions:");
    if patterns.iter().any(|p| p.category == PatternCategory::Bridge) {
        println!("      ✓ Strong cross-topic connections detected");
        println!("        → Climate and finance research are converging");
    }
    if patterns.iter().any(|p| p.category == PatternCategory::Emergence) {
        println!("      ✓ New research clusters emerging");
        println!("        → Novel areas of investigation forming");
    }
    if patterns.iter().any(|p| p.category == PatternCategory::Consolidation) {
        println!("      ✓ Topics consolidating");
        println!("        → Research maturing around key themes");
    }

    println!();
    println!("   ⚡ Performance:");
    println!("      Total runtime: {:.2}s", start.elapsed().as_secs_f64());
    println!("      Papers/second: {:.0}", all_records.len() as f64 / start.elapsed().as_secs_f64());
    println!();

    println!("✅ Discovery complete!");
    println!();

    Ok(())
}

/// Generate synthetic papers as fallback when API fails
fn generate_synthetic_papers(
    topic_id: &str,
    count: usize,
) -> Vec<ruvector_data_framework::DataRecord> {
    use chrono::Utc;

    let embedder = SimpleEmbedder::new(128);
    let mut records = Vec::new();

    // Topic-specific keywords
    let keywords = match topic_id {
        "climate_risk_finance" => vec!["climate", "risk", "finance", "investment", "portfolio"],
        "stranded_assets" => vec!["stranded", "assets", "fossil", "fuel", "transition"],
        "carbon_pricing" => vec!["carbon", "pricing", "emissions", "trading", "markets"],
        "physical_climate_risk" => vec!["physical", "climate", "risk", "adaptation", "resilience"],
        "transition_risk" => vec!["transition", "risk", "disclosure", "reporting", "climate"],
        _ => vec!["climate", "finance", "research"],
    };

    for i in 0..count {
        // Generate synthetic title and abstract
        let title = format!(
            "{} in {}: A Study of {} Systems",
            keywords[i % keywords.len()].to_uppercase(),
            keywords[(i + 1) % keywords.len()],
            keywords[(i + 2) % keywords.len()]
        );

        let abstract_text = format!(
            "This paper examines {} and {} in the context of {}. \
             We analyze {} patterns and their implications for {}. \
             Our findings suggest important relationships between these factors.",
            keywords[0],
            keywords[1],
            keywords[2],
            keywords[3 % keywords.len()],
            keywords[4 % keywords.len()]
        );

        let text = format!("{} {}", title, abstract_text);
        let embedding = embedder.embed_text(&text);

        let mut data_map = serde_json::Map::new();
        data_map.insert("title".to_string(), serde_json::json!(title));
        data_map.insert("abstract".to_string(), serde_json::json!(abstract_text));
        data_map.insert("citations".to_string(), serde_json::json!(i * 5));
        data_map.insert("synthetic".to_string(), serde_json::json!(true));

        records.push(ruvector_data_framework::DataRecord {
            id: format!("synthetic_{}_{}", topic_id, i),
            source: "openalex_synthetic".to_string(),
            record_type: "work".to_string(),
            timestamp: Utc::now() - chrono::Duration::days((i * 30) as i64),
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships: Vec::new(),
        });
    }

    records
}
