//! Cross-Domain Discovery Example
//!
//! Demonstrates RuVector's unique capability to find connections
//! between climate patterns and financial market behavior.
//!
//! This example explores the hypothesis that climate regime shifts
//! correlate with specific sector performance patterns.

use chrono::{Duration, Utc};
use rand::Rng;
use std::collections::HashMap;

use ruvector_data_framework::ruvector_native::{
    NativeDiscoveryEngine, NativeEngineConfig,
    SemanticVector, Domain, PatternType,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║        Cross-Domain Discovery with RuVector                   ║");
    println!("║     Finding Climate-Finance Correlations via Min-Cut          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Configure the native discovery engine
    let config = NativeEngineConfig {
        min_edge_weight: 0.4,
        similarity_threshold: 0.65,  // Lower threshold to find more connections
        mincut_sensitivity: 0.12,
        cross_domain: true,
        window_seconds: 86400 * 7,   // Weekly windows
        hnsw_m: 16,
        hnsw_ef_construction: 200,
        ..Default::default()
    };

    let mut engine = NativeDiscoveryEngine::new(config);

    println!("🔧 Engine configured for cross-domain discovery");
    println!("   Similarity threshold: 0.65");
    println!("   Min-cut sensitivity: 0.12");
    println!();

    // === Phase 1: Load Climate Data ===
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 Phase 1: Loading Climate Vectors");
    println!();

    let climate_vectors = generate_climate_vectors();
    for vector in &climate_vectors {
        engine.add_vector(vector.clone());
    }
    println!("   Added {} climate vectors", climate_vectors.len());

    // === Phase 2: Load Financial Data ===
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📈 Phase 2: Loading Financial Vectors");
    println!();

    let finance_vectors = generate_finance_vectors();
    for vector in &finance_vectors {
        engine.add_vector(vector.clone());
    }
    println!("   Added {} financial vectors", finance_vectors.len());

    // === Phase 3: Compute Initial Coherence ===
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔗 Phase 3: Computing Cross-Domain Coherence");
    println!();

    let stats = engine.stats();
    println!("   Graph Statistics:");
    println!("      Total nodes: {}", stats.total_nodes);
    println!("      Total edges: {}", stats.total_edges);
    println!("      Cross-domain edges: {}", stats.cross_domain_edges);

    for (domain, count) in &stats.domain_counts {
        println!("      {:?} nodes: {}", domain, count);
    }

    // Compute domain-specific coherence
    println!();
    println!("   Domain Coherence:");
    if let Some(climate_coh) = engine.domain_coherence(Domain::Climate) {
        println!("      Climate: {:.3}", climate_coh);
    }
    if let Some(finance_coh) = engine.domain_coherence(Domain::Finance) {
        println!("      Finance: {:.3}", finance_coh);
    }

    // === Phase 4: Detect Patterns ===
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔍 Phase 4: Pattern Detection");
    println!();

    // First detection (establishes baseline)
    let patterns_baseline = engine.detect_patterns();
    println!("   Baseline established: {} patterns detected", patterns_baseline.len());

    // Simulate time passing with new data
    println!();
    println!("   Simulating market event...");

    // Add vectors representing a market disruption correlated with climate
    let disruption_vectors = generate_disruption_vectors();
    for vector in &disruption_vectors {
        engine.add_vector(vector.clone());
    }

    // Detect patterns after disruption
    let patterns_after = engine.detect_patterns();
    println!("   After event: {} new patterns detected", patterns_after.len());

    // === Phase 5: Analyze Results ===
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 Phase 5: Discovery Results");
    println!();

    let all_patterns: Vec<_> = patterns_baseline.iter()
        .chain(patterns_after.iter())
        .collect();

    // Categorize patterns
    let mut by_type: HashMap<PatternType, Vec<_>> = HashMap::new();
    for pattern in &all_patterns {
        by_type.entry(pattern.pattern_type).or_default().push(pattern);
    }

    for (pattern_type, patterns) in &by_type {
        println!("   {:?}: {} instances", pattern_type, patterns.len());
        for pattern in patterns.iter().take(2) {
            println!("      • {} (confidence: {:.2})", pattern.description, pattern.confidence);

            // Show cross-domain links
            for link in &pattern.cross_domain_links {
                println!("        → {:?} ↔ {:?} (strength: {:.3})",
                    link.source_domain, link.target_domain, link.link_strength);
            }
        }
    }

    // === Phase 6: Novel Discoveries ===
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                   Novel Discoveries                           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Analyze cross-domain bridges
    let bridge_patterns: Vec<_> = all_patterns.iter()
        .filter(|p| p.pattern_type == PatternType::BridgeFormation)
        .collect();

    if !bridge_patterns.is_empty() {
        println!("🌉 Cross-Domain Bridges Discovered:");
        println!();
        for bridge in &bridge_patterns {
            println!("   {}", bridge.description);
            for link in &bridge.cross_domain_links {
                println!("      Hypothesis: {:?} signals may predict {:?} movements",
                    link.source_domain, link.target_domain);
                println!("      Connection strength: {:.3}", link.link_strength);
                println!("      Nodes involved: {} ↔ {}",
                    link.source_nodes.len(), link.target_nodes.len());
            }
            println!();
        }
    }

    // Analyze coherence breaks
    let breaks: Vec<_> = all_patterns.iter()
        .filter(|p| p.pattern_type == PatternType::CoherenceBreak)
        .collect();

    if !breaks.is_empty() {
        println!("⚡ Coherence Breaks (potential regime shifts):");
        println!();
        for (i, brk) in breaks.iter().enumerate() {
            println!("   {}. {}", i + 1, brk.description);
            println!("      Affected nodes: {}", brk.affected_nodes.len());
            println!("      Confidence: {:.2}", brk.confidence);

            if !brk.cross_domain_links.is_empty() {
                println!("      ⚠️ Break involves cross-domain connections!");
                println!("         This may indicate cascading effects between domains.");
            }
            println!();
        }
    }

    // Summary insights
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("💡 Key Insights");
    println!();

    let final_stats = engine.stats();
    let cross_domain_ratio = final_stats.cross_domain_edges as f64 /
        final_stats.total_edges.max(1) as f64;

    println!("   1. Cross-domain connectivity: {:.1}% of edges span domains",
        cross_domain_ratio * 100.0);

    if cross_domain_ratio > 0.1 {
        println!("      → Strong cross-domain coupling detected");
        println!("      → Climate and finance may share common drivers");
    }

    println!();
    println!("   2. Pattern propagation analysis:");
    println!("      → Regime shifts in one domain often coincide with");
    println!("        structural changes in the other");

    println!();
    println!("   3. Predictive potential:");
    println!("      → Cross-domain bridges with strength > 0.7 may offer");
    println!("        early warning signals across domains");

    println!();
    println!("✅ Cross-domain discovery complete");
    println!();

    Ok(())
}

/// Generate climate vectors representing different weather patterns
fn generate_climate_vectors() -> Vec<SemanticVector> {
    let mut rng = rand::thread_rng();
    let mut vectors = Vec::new();

    // Climate pattern archetypes (simplified 32-dim embeddings)
    let patterns = [
        ("arctic_warming", vec![0.9, 0.8, 0.7, 0.1, 0.2]),
        ("tropical_storm", vec![0.3, 0.9, 0.8, 0.6, 0.7]),
        ("drought_pattern", vec![0.1, 0.2, 0.3, 0.9, 0.8]),
        ("el_nino", vec![0.5, 0.6, 0.8, 0.4, 0.5]),
        ("la_nina", vec![0.5, 0.4, 0.2, 0.6, 0.5]),
        ("polar_vortex", vec![0.8, 0.3, 0.2, 0.1, 0.9]),
    ];

    for (i, (name, base_pattern)) in patterns.iter().enumerate() {
        // Generate variations of each pattern
        for j in 0..5 {
            let mut embedding: Vec<f32> = base_pattern.iter()
                .map(|&v| v + rng.gen_range(-0.1..0.1))
                .collect();

            // Pad to 32 dimensions
            while embedding.len() < 32 {
                embedding.push(rng.gen_range(-0.2..0.2));
            }

            vectors.push(SemanticVector {
                id: format!("climate_{}_{}", name, j),
                embedding,
                domain: Domain::Climate,
                timestamp: Utc::now() - Duration::days((i * 5 + j) as i64),
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("pattern".to_string(), name.to_string());
                    m.insert("region".to_string(), ["arctic", "pacific", "atlantic"][j % 3].to_string());
                    m
                },
            });
        }
    }

    vectors
}

/// Generate finance vectors representing market conditions
fn generate_finance_vectors() -> Vec<SemanticVector> {
    let mut rng = rand::thread_rng();
    let mut vectors = Vec::new();

    // Market condition archetypes
    let conditions = [
        ("bull_market", vec![0.9, 0.7, 0.3, 0.2, 0.1]),
        ("bear_market", vec![0.1, 0.3, 0.7, 0.8, 0.9]),
        ("volatility_spike", vec![0.5, 0.9, 0.8, 0.5, 0.6]),
        ("sector_rotation", vec![0.4, 0.5, 0.6, 0.5, 0.4]),
        ("commodity_surge", vec![0.7, 0.6, 0.5, 0.8, 0.7]),  // Correlates with climate!
        ("energy_crisis", vec![0.3, 0.8, 0.9, 0.7, 0.8]),    // Correlates with climate!
    ];

    for (i, (name, base_pattern)) in conditions.iter().enumerate() {
        for j in 0..4 {
            let mut embedding: Vec<f32> = base_pattern.iter()
                .map(|&v| v + rng.gen_range(-0.1..0.1))
                .collect();

            // Pad to 32 dimensions - add some dimensions that correlate with climate
            // This simulates real-world climate-finance correlations
            while embedding.len() < 32 {
                let climate_correlated = if name.contains("commodity") || name.contains("energy") {
                    // These patterns should correlate with climate patterns
                    0.5 + rng.gen_range(-0.1..0.3)
                } else {
                    rng.gen_range(-0.3..0.3)
                };
                embedding.push(climate_correlated);
            }

            vectors.push(SemanticVector {
                id: format!("finance_{}_{}", name, j),
                embedding,
                domain: Domain::Finance,
                timestamp: Utc::now() - Duration::days((i * 4 + j) as i64),
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("condition".to_string(), name.to_string());
                    m.insert("sector".to_string(), ["energy", "tech", "materials", "utilities"][j % 4].to_string());
                    m
                },
            });
        }
    }

    vectors
}

/// Generate vectors representing a disruption event that affects both domains
fn generate_disruption_vectors() -> Vec<SemanticVector> {
    let mut rng = rand::thread_rng();
    let mut vectors = Vec::new();

    // Climate disruption (e.g., extreme weather)
    let climate_disruption: Vec<f32> = (0..32)
        .map(|i| if i < 10 { 0.85 + rng.gen_range(-0.05..0.05) } else { rng.gen_range(0.3..0.6) })
        .collect();

    vectors.push(SemanticVector {
        id: "disruption_climate_1".to_string(),
        embedding: climate_disruption.clone(),
        domain: Domain::Climate,
        timestamp: Utc::now(),
        metadata: {
            let mut m = HashMap::new();
            m.insert("type".to_string(), "extreme_event".to_string());
            m
        },
    });

    // Correlated finance disruption (e.g., commodity spike)
    // Make embedding similar to climate disruption to trigger cross-domain detection
    let finance_disruption: Vec<f32> = climate_disruption.iter()
        .map(|&v| v + rng.gen_range(-0.15..0.15))  // Similar but not identical
        .collect();

    vectors.push(SemanticVector {
        id: "disruption_finance_1".to_string(),
        embedding: finance_disruption,
        domain: Domain::Finance,
        timestamp: Utc::now(),
        metadata: {
            let mut m = HashMap::new();
            m.insert("type".to_string(), "commodity_shock".to_string());
            m
        },
    });

    // Add more correlated disruption vectors
    for i in 2..5 {
        let similar: Vec<f32> = climate_disruption.iter()
            .map(|&v| v + rng.gen_range(-0.12..0.12))
            .collect();

        vectors.push(SemanticVector {
            id: format!("disruption_{}_{}", if i % 2 == 0 { "climate" } else { "finance" }, i),
            embedding: similar,
            domain: if i % 2 == 0 { Domain::Climate } else { Domain::Finance },
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        });
    }

    vectors
}
