//! Optimized Discovery Benchmark
//!
//! Compares baseline vs optimized engine performance using realistic
//! data from climate, finance, and research domains.
//!
//! Run: cargo run --example optimized_benchmark -p ruvector-data-framework --features parallel

use std::collections::HashMap;
use std::time::{Duration, Instant};
use chrono::{Utc, Duration as ChronoDuration};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use ruvector_data_framework::ruvector_native::{
    NativeDiscoveryEngine, NativeEngineConfig, Domain, SemanticVector,
};
use ruvector_data_framework::optimized::{
    OptimizedDiscoveryEngine, OptimizedConfig, simd_cosine_similarity,
};

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         RuVector Discovery Engine Benchmark                   ║");
    println!("║    Baseline vs Optimized (SIMD + Parallel + Statistical)     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Generate realistic test data
    let data = generate_multi_domain_data();
    println!("📊 Generated {} vectors across 3 domains\n", data.len());

    // Run benchmarks
    let baseline_results = benchmark_baseline(&data);
    let optimized_results = benchmark_optimized(&data);

    // Print comparison
    print_comparison(&baseline_results, &optimized_results);

    // Run SIMD microbenchmark
    simd_microbenchmark();

    // Run discovery quality benchmark
    discovery_quality_benchmark(&data);

    println!("\n✅ Benchmark complete");
}

/// Generate realistic multi-domain data
fn generate_multi_domain_data() -> Vec<SemanticVector> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut vectors = Vec::with_capacity(500);

    // Climate data - temperature, precipitation, pressure patterns
    let climate_topics = [
        "temperature_anomaly", "precipitation_index", "drought_severity",
        "ocean_heat_content", "arctic_sea_ice", "atmospheric_co2",
        "el_nino_index", "atlantic_oscillation", "monsoon_intensity",
        "wildfire_risk", "flood_probability", "hurricane_potential",
    ];

    for (i, topic) in climate_topics.iter().enumerate() {
        for month in 0..12 {
            let embedding = generate_climate_embedding(&mut rng, i, month);
            vectors.push(SemanticVector {
                id: format!("climate_{}_{}", topic, month),
                embedding,
                domain: Domain::Climate,
                timestamp: Utc::now() - ChronoDuration::days((11 - month as i64) * 30),
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("topic".to_string(), topic.to_string());
                    m.insert("month".to_string(), month.to_string());
                    m
                },
            });
        }
    }

    // Financial data - sector performance, market indicators
    let finance_sectors = [
        "energy_sector", "utilities_sector", "agriculture_commodities",
        "insurance_sector", "real_estate", "transportation",
        "consumer_staples", "materials_sector",
    ];

    for (i, sector) in finance_sectors.iter().enumerate() {
        for quarter in 0..8 {
            let embedding = generate_finance_embedding(&mut rng, i, quarter);
            vectors.push(SemanticVector {
                id: format!("finance_{}_{}", sector, quarter),
                embedding,
                domain: Domain::Finance,
                timestamp: Utc::now() - ChronoDuration::days((7 - quarter as i64) * 90),
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("sector".to_string(), sector.to_string());
                    m.insert("quarter".to_string(), quarter.to_string());
                    m
                },
            });
        }
    }

    // Research data - papers on climate-finance connections
    let research_topics = [
        "climate_risk_pricing", "stranded_assets", "carbon_markets",
        "physical_risk_modeling", "transition_risk", "climate_disclosure",
        "green_bonds", "sustainable_finance",
    ];

    for (i, topic) in research_topics.iter().enumerate() {
        for year in 0..5 {
            let embedding = generate_research_embedding(&mut rng, i, year);
            vectors.push(SemanticVector {
                id: format!("research_{}_{}", topic, 2020 + year),
                embedding,
                domain: Domain::Research,
                timestamp: Utc::now() - ChronoDuration::days((4 - year as i64) * 365),
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("topic".to_string(), topic.to_string());
                    m.insert("year".to_string(), (2020 + year).to_string());
                    m
                },
            });
        }
    }

    vectors
}

/// Generate climate-like embedding with topic/temporal structure
fn generate_climate_embedding(rng: &mut StdRng, topic_id: usize, time_id: usize) -> Vec<f32> {
    let dim = 128;
    let mut embedding = vec![0.0_f32; dim];

    // Base topic signature
    for i in 0..dim {
        embedding[i] = rng.gen::<f32>() * 0.1;
    }

    // Topic-specific cluster
    let topic_start = (topic_id * 10) % dim;
    for i in 0..10 {
        embedding[(topic_start + i) % dim] += 0.5 + rng.gen::<f32>() * 0.3;
    }

    // Seasonal pattern (affects climate similarity)
    let season = time_id % 4;
    let season_start = 80 + season * 10;
    for i in 0..10 {
        embedding[(season_start + i) % dim] += 0.3 + rng.gen::<f32>() * 0.2;
    }

    // Cross-domain bridge: climate topics 0-2 correlate with finance
    if topic_id < 3 {
        // Add finance-like signature
        for i in 40..50 {
            embedding[i] += 0.3;
        }
    }

    normalize_embedding(&mut embedding);
    embedding
}

/// Generate finance-like embedding
fn generate_finance_embedding(rng: &mut StdRng, sector_id: usize, time_id: usize) -> Vec<f32> {
    let dim = 128;
    let mut embedding = vec![0.0_f32; dim];

    for i in 0..dim {
        embedding[i] = rng.gen::<f32>() * 0.1;
    }

    // Sector cluster
    let sector_start = 40 + (sector_id * 8) % 40;
    for i in 0..8 {
        embedding[(sector_start + i) % dim] += 0.5 + rng.gen::<f32>() * 0.3;
    }

    // Temporal trend
    let trend_strength = time_id as f32 / 8.0;
    for i in 100..110 {
        embedding[i] += trend_strength * 0.2;
    }

    // Cross-domain: energy/utilities correlate with climate
    if sector_id < 2 {
        // Climate-like signature
        for i in 0..10 {
            embedding[i] += 0.35;
        }
    }

    normalize_embedding(&mut embedding);
    embedding
}

/// Generate research-like embedding
fn generate_research_embedding(rng: &mut StdRng, topic_id: usize, year_id: usize) -> Vec<f32> {
    let dim = 128;
    let mut embedding = vec![0.0_f32; dim];

    for i in 0..dim {
        embedding[i] = rng.gen::<f32>() * 0.1;
    }

    // Research topic cluster
    let topic_start = 10 + (topic_id * 12) % 60;
    for i in 0..12 {
        embedding[(topic_start + i) % dim] += 0.5 + rng.gen::<f32>() * 0.2;
    }

    // Bridge to both climate and finance
    // Climate connection
    for i in 0..8 {
        embedding[i] += 0.25;
    }
    // Finance connection
    for i in 45..53 {
        embedding[i] += 0.25;
    }

    // Recent papers have evolved vocabulary
    let recency = year_id as f32 / 5.0;
    for i in 115..125 {
        embedding[i] += recency * 0.3;
    }

    normalize_embedding(&mut embedding);
    embedding
}

fn normalize_embedding(embedding: &mut [f32]) {
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in embedding.iter_mut() {
            *x /= norm;
        }
    }
}

/// Benchmark results
#[derive(Debug)]
struct BenchmarkResults {
    name: String,
    vector_add_time: Duration,
    coherence_time: Duration,
    pattern_detection_time: Duration,
    total_time: Duration,
    edges_created: usize,
    patterns_found: usize,
    cross_domain_edges: usize,
}

/// Benchmark the baseline engine
fn benchmark_baseline(data: &[SemanticVector]) -> BenchmarkResults {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📈 Running Baseline Engine Benchmark...\n");

    let config = NativeEngineConfig {
        similarity_threshold: 0.55,
        mincut_sensitivity: 0.10,
        cross_domain: true,
        ..Default::default()
    };

    let mut engine = NativeDiscoveryEngine::new(config);
    let total_start = Instant::now();

    // Add vectors
    let add_start = Instant::now();
    for vector in data {
        engine.add_vector(vector.clone());
    }
    let vector_add_time = add_start.elapsed();
    println!("   Vector insertion: {:?}", vector_add_time);

    // Compute coherence
    let coherence_start = Instant::now();
    let snapshot = engine.compute_coherence();
    let coherence_time = coherence_start.elapsed();
    println!("   Coherence computation: {:?}", coherence_time);
    println!("   Min-cut value: {:.4}", snapshot.mincut_value);

    // Pattern detection
    let pattern_start = Instant::now();
    let patterns = engine.detect_patterns();
    let pattern_detection_time = pattern_start.elapsed();
    println!("   Pattern detection: {:?}", pattern_detection_time);

    let total_time = total_start.elapsed();
    let stats = engine.stats();

    println!("\n   Results:");
    println!("   - Edges: {}", stats.total_edges);
    println!("   - Cross-domain edges: {}", stats.cross_domain_edges);
    println!("   - Patterns found: {}", patterns.len());

    BenchmarkResults {
        name: "Baseline".to_string(),
        vector_add_time,
        coherence_time,
        pattern_detection_time,
        total_time,
        edges_created: stats.total_edges,
        patterns_found: patterns.len(),
        cross_domain_edges: stats.cross_domain_edges,
    }
}

/// Benchmark the optimized engine
fn benchmark_optimized(data: &[SemanticVector]) -> BenchmarkResults {
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🚀 Running Optimized Engine Benchmark...\n");

    let config = OptimizedConfig {
        similarity_threshold: 0.55,
        mincut_sensitivity: 0.10,
        cross_domain: true,
        use_simd: true,
        batch_size: 128,
        significance_threshold: 0.05,
        causality_lookback: 8,
        causality_min_correlation: 0.5,
        ..Default::default()
    };

    let mut engine = OptimizedDiscoveryEngine::new(config);
    let total_start = Instant::now();

    // Batch add vectors
    let add_start = Instant::now();
    #[cfg(feature = "parallel")]
    {
        engine.add_vectors_batch(data.to_vec());
    }
    #[cfg(not(feature = "parallel"))]
    {
        for vector in data {
            engine.add_vector(vector.clone());
        }
    }
    let vector_add_time = add_start.elapsed();
    println!("   Vector insertion (batch): {:?}", vector_add_time);

    // Compute coherence with caching
    let coherence_start = Instant::now();
    let snapshot = engine.compute_coherence();
    let coherence_time = coherence_start.elapsed();
    println!("   Coherence computation: {:?}", coherence_time);
    println!("   Min-cut value: {:.4}", snapshot.mincut_value);

    // Pattern detection with significance
    let pattern_start = Instant::now();
    let patterns = engine.detect_patterns_with_significance();
    let pattern_detection_time = pattern_start.elapsed();
    println!("   Pattern detection (w/ stats): {:?}", pattern_detection_time);

    let total_time = total_start.elapsed();
    let stats = engine.stats();
    let metrics = engine.metrics();

    println!("\n   Results:");
    println!("   - Edges: {}", stats.total_edges);
    println!("   - Cross-domain edges: {}", stats.cross_domain_edges);
    println!("   - Patterns found: {}", patterns.len());
    println!("   - Significant patterns: {}", patterns.iter().filter(|p| p.is_significant).count());
    println!("   - Vector comparisons: {}", stats.total_comparisons);

    // Show significant patterns
    let significant: Vec<_> = patterns.iter().filter(|p| p.is_significant).collect();
    if !significant.is_empty() {
        println!("\n   📊 Significant Patterns (p < 0.05):");
        for pattern in significant.iter().take(5) {
            println!("      • {} (p={:.4}, effect={:.3})",
                pattern.pattern.description,
                pattern.p_value,
                pattern.effect_size
            );
        }
    }

    BenchmarkResults {
        name: "Optimized".to_string(),
        vector_add_time,
        coherence_time,
        pattern_detection_time,
        total_time,
        edges_created: stats.total_edges,
        patterns_found: patterns.len(),
        cross_domain_edges: stats.cross_domain_edges,
    }
}

/// Print comparison of results
fn print_comparison(baseline: &BenchmarkResults, optimized: &BenchmarkResults) {
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    Performance Comparison                     ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let speedup = |base: Duration, opt: Duration| -> f64 {
        base.as_secs_f64() / opt.as_secs_f64().max(0.0001)
    };

    println!("   ┌─────────────────────┬─────────────┬─────────────┬──────────┐");
    println!("   │ Operation           │ Baseline    │ Optimized   │ Speedup  │");
    println!("   ├─────────────────────┼─────────────┼─────────────┼──────────┤");

    println!("   │ Vector Insertion    │ {:>9.2}ms │ {:>9.2}ms │ {:>6.2}x  │",
        baseline.vector_add_time.as_secs_f64() * 1000.0,
        optimized.vector_add_time.as_secs_f64() * 1000.0,
        speedup(baseline.vector_add_time, optimized.vector_add_time)
    );

    println!("   │ Coherence Compute   │ {:>9.2}ms │ {:>9.2}ms │ {:>6.2}x  │",
        baseline.coherence_time.as_secs_f64() * 1000.0,
        optimized.coherence_time.as_secs_f64() * 1000.0,
        speedup(baseline.coherence_time, optimized.coherence_time)
    );

    println!("   │ Pattern Detection   │ {:>9.2}ms │ {:>9.2}ms │ {:>6.2}x  │",
        baseline.pattern_detection_time.as_secs_f64() * 1000.0,
        optimized.pattern_detection_time.as_secs_f64() * 1000.0,
        speedup(baseline.pattern_detection_time, optimized.pattern_detection_time)
    );

    println!("   ├─────────────────────┼─────────────┼─────────────┼──────────┤");
    println!("   │ TOTAL               │ {:>9.2}ms │ {:>9.2}ms │ {:>6.2}x  │",
        baseline.total_time.as_secs_f64() * 1000.0,
        optimized.total_time.as_secs_f64() * 1000.0,
        speedup(baseline.total_time, optimized.total_time)
    );
    println!("   └─────────────────────┴─────────────┴─────────────┴──────────┘");

    println!("\n   Quality Metrics:");
    println!("   - Edges created: {} → {} (same algorithm)",
        baseline.edges_created, optimized.edges_created);
    println!("   - Cross-domain: {} → {}",
        baseline.cross_domain_edges, optimized.cross_domain_edges);
    println!("   - Patterns: {} → {} (+ statistical filtering)",
        baseline.patterns_found, optimized.patterns_found);
}

/// SIMD microbenchmark
fn simd_microbenchmark() {
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⚡ SIMD Vector Operations Microbenchmark\n");

    let mut rng = StdRng::seed_from_u64(123);
    let dim = 128;
    let iterations = 100_000;

    // Generate test vectors
    let vectors: Vec<Vec<f32>> = (0..100)
        .map(|_| {
            let mut v: Vec<f32> = (0..dim).map(|_| rng.gen()).collect();
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            for x in &mut v {
                *x /= norm;
            }
            v
        })
        .collect();

    // Benchmark SIMD cosine
    let start = Instant::now();
    let mut sum = 0.0_f32;
    for i in 0..iterations {
        let a = &vectors[i % 100];
        let b = &vectors[(i + 1) % 100];
        sum += simd_cosine_similarity(a, b);
    }
    let simd_time = start.elapsed();

    // Benchmark standard cosine
    let start = Instant::now();
    let mut sum2 = 0.0_f32;
    for i in 0..iterations {
        let a = &vectors[i % 100];
        let b = &vectors[(i + 1) % 100];
        sum2 += standard_cosine(a, b);
    }
    let std_time = start.elapsed();

    println!("   {} cosine similarity operations on {}-dim vectors:\n", iterations, dim);
    println!("   SIMD version:     {:>8.2}ms ({:.2} M ops/sec)",
        simd_time.as_secs_f64() * 1000.0,
        iterations as f64 / simd_time.as_secs_f64() / 1_000_000.0
    );
    println!("   Standard version: {:>8.2}ms ({:.2} M ops/sec)",
        std_time.as_secs_f64() * 1000.0,
        iterations as f64 / std_time.as_secs_f64() / 1_000_000.0
    );
    println!("   Speedup: {:.2}x", std_time.as_secs_f64() / simd_time.as_secs_f64());
    println!("   (checksum: {:.4}, {:.4})", sum, sum2);
}

fn standard_cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b)
}

/// Discovery quality benchmark
fn discovery_quality_benchmark(data: &[SemanticVector]) {
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔍 Discovery Quality Analysis\n");

    let config = OptimizedConfig {
        similarity_threshold: 0.55,
        cross_domain: true,
        significance_threshold: 0.05,
        causality_lookback: 8,
        causality_min_correlation: 0.5,
        ..Default::default()
    };

    let mut engine = OptimizedDiscoveryEngine::new(config);

    // Add data in temporal batches to detect patterns
    let batch_size = data.len() / 4;
    let mut all_patterns = Vec::new();

    for (batch_idx, batch) in data.chunks(batch_size).enumerate() {
        #[cfg(feature = "parallel")]
        {
            engine.add_vectors_batch(batch.to_vec());
        }
        #[cfg(not(feature = "parallel"))]
        {
            for v in batch {
                engine.add_vector(v.clone());
            }
        }

        let patterns = engine.detect_patterns_with_significance();
        all_patterns.extend(patterns);

        println!("   Batch {} ({} vectors): {} patterns detected",
            batch_idx + 1, batch.len(), all_patterns.len());
    }

    // Analyze cross-domain discoveries
    let stats = engine.stats();

    println!("\n   Cross-Domain Analysis:");
    println!("   ─────────────────────────");
    println!("   Climate nodes:  {}", stats.domain_counts.get(&Domain::Climate).unwrap_or(&0));
    println!("   Finance nodes:  {}", stats.domain_counts.get(&Domain::Finance).unwrap_or(&0));
    println!("   Research nodes: {}", stats.domain_counts.get(&Domain::Research).unwrap_or(&0));
    println!("   Cross-domain edges: {} ({:.1}% of total)",
        stats.cross_domain_edges,
        100.0 * stats.cross_domain_edges as f64 / stats.total_edges.max(1) as f64
    );

    // Domain coherence
    println!("\n   Domain Coherence Scores:");
    if let Some(coh) = engine.domain_coherence(Domain::Climate) {
        println!("   Climate:  {:.3}", coh);
    }
    if let Some(coh) = engine.domain_coherence(Domain::Finance) {
        println!("   Finance:  {:.3}", coh);
    }
    if let Some(coh) = engine.domain_coherence(Domain::Research) {
        println!("   Research: {:.3}", coh);
    }

    // Show discovered cross-domain bridges
    let bridges: Vec<_> = all_patterns.iter()
        .filter(|p| !p.pattern.cross_domain_links.is_empty())
        .collect();

    if !bridges.is_empty() {
        println!("\n   🌉 Cross-Domain Bridges Found: {}", bridges.len());
        for bridge in bridges.iter().take(3) {
            for link in &bridge.pattern.cross_domain_links {
                println!("      {:?} ↔ {:?} (strength: {:.3}, type: {})",
                    link.source_domain,
                    link.target_domain,
                    link.link_strength,
                    link.link_type
                );
            }
        }
    }

    // Causality patterns
    let causality: Vec<_> = all_patterns.iter()
        .filter(|p| matches!(p.pattern.pattern_type, ruvector_data_framework::ruvector_native::PatternType::Cascade))
        .collect();

    if !causality.is_empty() {
        println!("\n   🔗 Temporal Causality Patterns: {}", causality.len());
        for pattern in causality.iter().take(3) {
            println!("      {} (p={:.4})", pattern.pattern.description, pattern.p_value);
        }
    }
}
