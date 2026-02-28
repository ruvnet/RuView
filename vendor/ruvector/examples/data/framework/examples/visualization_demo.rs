//! Visualization Demo
//!
//! Demonstrates ASCII graph visualization, domain matrices, coherence timelines,
//! and pattern summaries for the RuVector discovery framework.

use chrono::{Duration, Utc};
use ruvector_data_framework::optimized::{OptimizedConfig, OptimizedDiscoveryEngine, SignificantPattern};
use ruvector_data_framework::ruvector_native::{Domain, SemanticVector};
use ruvector_data_framework::visualization::{
    render_dashboard, render_coherence_timeline, render_domain_matrix,
    render_graph_ascii, render_pattern_summary,
};
use std::collections::HashMap;

fn main() {
    println!("\n🎨 RuVector Discovery Framework - Visualization Demo\n");

    // Create an optimized discovery engine
    let config = OptimizedConfig {
        similarity_threshold: 0.65,
        mincut_sensitivity: 0.12,
        cross_domain: true,
        batch_size: 256,
        use_simd: true,
        ..Default::default()
    };

    let mut engine = OptimizedDiscoveryEngine::new(config);

    // Add sample vectors across domains
    println!("📊 Adding sample data...\n");

    let now = Utc::now();

    // Climate domain vectors
    for i in 0..8 {
        let vector = SemanticVector {
            id: format!("climate_{}", i),
            embedding: vec![0.5 + i as f32 * 0.05; 128],
            domain: Domain::Climate,
            timestamp: now,
            metadata: HashMap::new(),
        };
        engine.add_vector(vector);
    }

    // Finance domain vectors
    for i in 0..6 {
        let vector = SemanticVector {
            id: format!("finance_{}", i),
            embedding: vec![0.3 + i as f32 * 0.05; 128],
            domain: Domain::Finance,
            timestamp: now,
            metadata: HashMap::new(),
        };
        engine.add_vector(vector);
    }

    // Research domain vectors
    for i in 0..5 {
        let vector = SemanticVector {
            id: format!("research_{}", i),
            embedding: vec![0.7 + i as f32 * 0.05; 128],
            domain: Domain::Research,
            timestamp: now,
            metadata: HashMap::new(),
        };
        engine.add_vector(vector);
    }

    // Compute coherence and detect patterns
    println!("🔍 Computing coherence and detecting patterns...\n");

    let mut coherence_history = Vec::new();
    let mut all_patterns = Vec::new();

    // Simulate multiple timesteps
    for step in 0..5 {
        let timestamp = now + Duration::hours(step);
        let coherence = engine.compute_coherence();
        coherence_history.push((timestamp, coherence.mincut_value));

        let patterns = engine.detect_patterns_with_significance();
        all_patterns.extend(patterns);
    }

    // Display individual visualizations
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("1️⃣  GRAPH VISUALIZATION");
    println!("═══════════════════════════════════════════════════════════════════════════════\n");
    println!("{}", render_graph_ascii(&engine, 80, 20));

    println!("\n═══════════════════════════════════════════════════════════════════════════════");
    println!("2️⃣  DOMAIN CONNECTIVITY MATRIX");
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("{}", render_domain_matrix(&engine));

    println!("\n═══════════════════════════════════════════════════════════════════════════════");
    println!("3️⃣  COHERENCE TIMELINE");
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("{}", render_coherence_timeline(&coherence_history));

    println!("\n═══════════════════════════════════════════════════════════════════════════════");
    println!("4️⃣  PATTERN SUMMARY");
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("{}", render_pattern_summary(&all_patterns));

    println!("\n═══════════════════════════════════════════════════════════════════════════════");
    println!("5️⃣  COMPLETE DASHBOARD");
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("{}", render_dashboard(&engine, &all_patterns, &coherence_history));

    println!("\n✅ Visualization demo complete!\n");

    // Display stats
    let stats = engine.stats();
    println!("📈 Final Statistics:");
    println!("   • Total nodes: {}", stats.total_nodes);
    println!("   • Total edges: {}", stats.total_edges);
    println!("   • Cross-domain edges: {}", stats.cross_domain_edges);
    println!("   • Patterns discovered: {}", all_patterns.len());
    println!("   • Coherence samples: {}", coherence_history.len());
    println!("   • Cache hit rate: {:.1}%", stats.cache_hit_rate * 100.0);
    println!("   • Total comparisons: {}", stats.total_comparisons);
    println!();
}
