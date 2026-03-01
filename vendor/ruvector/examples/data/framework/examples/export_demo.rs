//! Export Demo - GraphML, DOT, and CSV Export
//!
//! This example demonstrates how to export discovery results in various formats:
//! - GraphML (for Gephi, Cytoscape)
//! - DOT (for Graphviz)
//! - CSV (for patterns and coherence history)
//!
//! Run with:
//! ```bash
//! cargo run --example export_demo --features parallel
//! ```

use chrono::Utc;
use ruvector_data_framework::export::{
    export_all, export_coherence_csv, export_dot, export_graphml, export_patterns_csv,
    export_patterns_with_evidence_csv, ExportFilter,
};
use ruvector_data_framework::optimized::{OptimizedConfig, OptimizedDiscoveryEngine};
use ruvector_data_framework::ruvector_native::{Domain, SemanticVector};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 RuVector Discovery Framework - Export Demo\n");

    // Create an optimized discovery engine
    let config = OptimizedConfig {
        similarity_threshold: 0.65,
        cross_domain: true,
        use_simd: true,
        ..Default::default()
    };

    let mut engine = OptimizedDiscoveryEngine::new(config);

    // Add sample vectors from different domains
    println!("📊 Adding sample vectors...");

    // Climate data vectors
    let climate_vectors = generate_sample_vectors(Domain::Climate, 20, "climate_");
    for vector in climate_vectors {
        engine.add_vector(vector);
    }

    // Finance data vectors
    let finance_vectors = generate_sample_vectors(Domain::Finance, 15, "finance_");
    for vector in finance_vectors {
        engine.add_vector(vector);
    }

    // Research data vectors
    let research_vectors = generate_sample_vectors(Domain::Research, 25, "research_");
    for vector in research_vectors {
        engine.add_vector(vector);
    }

    // Compute coherence and detect patterns
    println!("🔍 Computing coherence and detecting patterns...");
    let patterns = engine.detect_patterns_with_significance();

    // Get coherence history
    // Note: In a real application, you would have accumulated history over time
    let coherence_history = vec![];

    let stats = engine.stats();
    println!("\n📈 Discovery Statistics:");
    println!("   Nodes: {}", stats.total_nodes);
    println!("   Edges: {}", stats.total_edges);
    println!("   Cross-domain edges: {}", stats.cross_domain_edges);
    println!("   Patterns detected: {}", patterns.len());

    // Create output directory
    let output_dir = "discovery_exports";
    std::fs::create_dir_all(output_dir)?;

    println!("\n📁 Exporting to {}/ directory...\n", output_dir);

    // 1. Export full graph to GraphML (for Gephi)
    println!("   ✓ Exporting graph.graphml (for Gephi)");
    export_graphml(&engine, format!("{}/graph.graphml", output_dir), None)?;

    // 2. Export full graph to DOT (for Graphviz)
    println!("   ✓ Exporting graph.dot (for Graphviz)");
    export_dot(&engine, format!("{}/graph.dot", output_dir), None)?;

    // 3. Export climate domain only
    println!("   ✓ Exporting climate_only.graphml");
    let climate_filter = ExportFilter::domain(Domain::Climate);
    export_graphml(
        &engine,
        format!("{}/climate_only.graphml", output_dir),
        Some(climate_filter),
    )?;

    // 4. Export patterns to CSV
    if !patterns.is_empty() {
        println!("   ✓ Exporting patterns.csv");
        export_patterns_csv(&patterns, format!("{}/patterns.csv", output_dir))?;

        println!("   ✓ Exporting patterns_evidence.csv");
        export_patterns_with_evidence_csv(
            &patterns,
            format!("{}/patterns_evidence.csv", output_dir),
        )?;
    }

    // 5. Export coherence history
    if !coherence_history.is_empty() {
        println!("   ✓ Exporting coherence.csv");
        export_coherence_csv(
            &coherence_history,
            format!("{}/coherence.csv", output_dir),
        )?;
    }

    // 6. Export everything at once
    println!("\n   ✓ Exporting all data to {}/full_export/", output_dir);
    export_all(
        &engine,
        &patterns,
        &coherence_history,
        format!("{}/full_export", output_dir),
    )?;

    println!("\n✅ Export complete!\n");
    println!("📊 Visualization options:");
    println!("   1. Open graph.graphml in Gephi:");
    println!("      - File → Open → graph.graphml");
    println!("      - Layout → Force Atlas 2");
    println!("      - Color nodes by 'domain' attribute\n");
    println!("   2. Render graph.dot with Graphviz:");
    println!("      dot -Tpng {}/graph.dot -o graph.png", output_dir);
    println!("      neato -Tsvg {}/graph.dot -o graph.svg\n", output_dir);
    println!("   3. Analyze patterns.csv in Excel/R/Python\n");

    println!("📁 All files exported to: {}/", output_dir);

    Ok(())
}

/// Generate sample vectors for demonstration
fn generate_sample_vectors(domain: Domain, count: usize, prefix: &str) -> Vec<SemanticVector> {
    let mut vectors = Vec::new();
    let dimension = 384;

    for i in 0..count {
        let mut embedding = vec![0.0; dimension];

        // Generate pseudo-random but reproducible embeddings
        let seed = (domain as usize) * 1000 + i;
        for j in 0..dimension {
            let val = ((seed + j) as f32 * 0.1).sin();
            embedding[j] = val;
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        vectors.push(SemanticVector {
            id: format!("{}{}", prefix, i),
            embedding,
            domain,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        });
    }

    vectors
}
