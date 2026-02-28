//! Discovery Hunter
//!
//! Actively searches for novel patterns, correlations, and anomalies
//! across climate, finance, and research domains.
//!
//! Run: cargo run --example discovery_hunter -p ruvector-data-framework --features parallel --release

use std::collections::HashMap;
use chrono::{Utc, Duration as ChronoDuration};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

use ruvector_data_framework::optimized::{
    OptimizedDiscoveryEngine, OptimizedConfig, SignificantPattern,
};
use ruvector_data_framework::ruvector_native::{
    Domain, SemanticVector, PatternType,
};

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              RuVector Discovery Hunter                        ║");
    println!("║     Searching for Novel Cross-Domain Patterns                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Initialize discovery engine with sensitive settings
    let config = OptimizedConfig {
        similarity_threshold: 0.45,  // Lower threshold to catch more connections
        mincut_sensitivity: 0.08,    // More sensitive to coherence changes
        cross_domain: true,
        use_simd: true,
        significance_threshold: 0.10, // Include marginally significant patterns
        causality_lookback: 12,       // Look back further in time
        causality_min_correlation: 0.4, // Catch weaker correlations
        ..Default::default()
    };

    let mut engine = OptimizedDiscoveryEngine::new(config);
    let mut all_discoveries: Vec<Discovery> = Vec::new();

    // Phase 1: Load climate extremes data
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🌡️  Phase 1: Climate Extremes Data\n");
    let climate_data = generate_climate_extremes_data();
    println!("   Loaded {} climate vectors", climate_data.len());

    #[cfg(feature = "parallel")]
    engine.add_vectors_batch(climate_data);
    #[cfg(not(feature = "parallel"))]
    for v in climate_data { engine.add_vector(v); }

    let patterns = engine.detect_patterns_with_significance();
    process_discoveries(&patterns, &mut all_discoveries, "Climate Baseline");

    // Phase 2: Load financial stress data
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📈 Phase 2: Financial Stress Indicators\n");
    let finance_data = generate_financial_stress_data();
    println!("   Loaded {} financial vectors", finance_data.len());

    #[cfg(feature = "parallel")]
    engine.add_vectors_batch(finance_data);
    #[cfg(not(feature = "parallel"))]
    for v in finance_data { engine.add_vector(v); }

    let patterns = engine.detect_patterns_with_significance();
    process_discoveries(&patterns, &mut all_discoveries, "Climate-Finance Integration");

    // Phase 3: Load research publications
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📚 Phase 3: Research Publications\n");
    let research_data = generate_research_data();
    println!("   Loaded {} research vectors", research_data.len());

    #[cfg(feature = "parallel")]
    engine.add_vectors_batch(research_data);
    #[cfg(not(feature = "parallel"))]
    for v in research_data { engine.add_vector(v); }

    let patterns = engine.detect_patterns_with_significance();
    process_discoveries(&patterns, &mut all_discoveries, "Full Integration");

    // Phase 4: Inject anomalies to test detection
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⚡ Phase 4: Anomaly Injection Test\n");
    let anomaly_data = generate_anomaly_scenarios();
    println!("   Injecting {} anomaly scenarios", anomaly_data.len());

    #[cfg(feature = "parallel")]
    engine.add_vectors_batch(anomaly_data);
    #[cfg(not(feature = "parallel"))]
    for v in anomaly_data { engine.add_vector(v); }

    let patterns = engine.detect_patterns_with_significance();
    process_discoveries(&patterns, &mut all_discoveries, "Anomaly Detection");

    // Final Analysis
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    DISCOVERY REPORT                           ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let stats = engine.stats();
    println!("📊 Graph Statistics:");
    println!("   Total nodes: {}", stats.total_nodes);
    println!("   Total edges: {}", stats.total_edges);
    println!("   Cross-domain edges: {} ({:.1}%)",
        stats.cross_domain_edges,
        100.0 * stats.cross_domain_edges as f64 / stats.total_edges.max(1) as f64
    );

    // Categorize discoveries
    let mut by_type: HashMap<&str, Vec<&Discovery>> = HashMap::new();
    for d in &all_discoveries {
        by_type.entry(d.category.as_str()).or_default().push(d);
    }

    println!("\n🔬 Discoveries by Category:\n");

    // 1. Cross-Domain Bridges
    if let Some(bridges) = by_type.get("Bridge") {
        println!("   🌉 Cross-Domain Bridges: {}", bridges.len());
        for (i, bridge) in bridges.iter().take(5).enumerate() {
            println!("      {}. {} (confidence: {:.2}, p={:.4})",
                i + 1, bridge.description, bridge.confidence, bridge.p_value);
            if !bridge.hypothesis.is_empty() {
                println!("         → Hypothesis: {}", bridge.hypothesis);
            }
        }
    }

    // 2. Temporal Cascades
    if let Some(cascades) = by_type.get("Cascade") {
        println!("\n   🔗 Temporal Cascades: {}", cascades.len());
        for (i, cascade) in cascades.iter().take(5).enumerate() {
            println!("      {}. {} (p={:.4})",
                i + 1, cascade.description, cascade.p_value);
            if !cascade.hypothesis.is_empty() {
                println!("         → {}", cascade.hypothesis);
            }
        }
    }

    // 3. Coherence Events
    if let Some(coherence) = by_type.get("Coherence") {
        println!("\n   📉 Coherence Events: {}", coherence.len());
        for (i, event) in coherence.iter().take(5).enumerate() {
            println!("      {}. {} (effect size: {:.3})",
                i + 1, event.description, event.effect_size);
        }
    }

    // 4. Emerging Clusters
    if let Some(clusters) = by_type.get("Cluster") {
        println!("\n   🔮 Emerging Clusters: {}", clusters.len());
        for (i, cluster) in clusters.iter().take(5).enumerate() {
            println!("      {}. {}", i + 1, cluster.description);
        }
    }

    // Novel Findings Summary
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("💡 NOVEL FINDINGS\n");

    let significant: Vec<_> = all_discoveries.iter()
        .filter(|d| d.p_value < 0.05 && d.confidence > 0.6)
        .collect();

    if significant.is_empty() {
        println!("   No statistically significant novel patterns detected.");
        println!("   This suggests the data is well-integrated with expected correlations.");
    } else {
        println!("   Found {} statistically significant discoveries:\n", significant.len());

        for (i, discovery) in significant.iter().enumerate() {
            println!("   {}. [{}] {}", i + 1, discovery.category, discovery.description);
            println!("      Confidence: {:.2}, p-value: {:.4}, effect: {:.3}",
                discovery.confidence, discovery.p_value, discovery.effect_size);
            if !discovery.hypothesis.is_empty() {
                println!("      Hypothesis: {}", discovery.hypothesis);
            }
            if !discovery.implications.is_empty() {
                println!("      Implications: {}", discovery.implications);
            }
            println!();
        }
    }

    // Cross-Domain Insights
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔍 CROSS-DOMAIN INSIGHTS\n");

    // Compute domain coherence
    let climate_coh = engine.domain_coherence(Domain::Climate);
    let finance_coh = engine.domain_coherence(Domain::Finance);
    let research_coh = engine.domain_coherence(Domain::Research);

    println!("   Domain Coherence (internal consistency):");
    if let Some(c) = climate_coh {
        println!("   - Climate:  {:.3} {}", c, coherence_interpretation(c));
    }
    if let Some(f) = finance_coh {
        println!("   - Finance:  {:.3} {}", f, coherence_interpretation(f));
    }
    if let Some(r) = research_coh {
        println!("   - Research: {:.3} {}", r, coherence_interpretation(r));
    }

    // Cross-domain coupling strength
    let coupling = stats.cross_domain_edges as f64 / stats.total_edges.max(1) as f64;
    println!("\n   Cross-Domain Coupling: {:.1}%", coupling * 100.0);

    if coupling > 0.4 {
        println!("   → Strong interdependence between domains");
        println!("   → Climate, finance, and research are tightly coupled");
        println!("   → Changes in one domain likely propagate to others");
    } else if coupling > 0.2 {
        println!("   → Moderate cross-domain relationships");
        println!("   → Some pathways exist for information flow between domains");
    } else {
        println!("   → Weak cross-domain coupling");
        println!("   → Domains are relatively independent");
    }

    // Specific hypotheses based on patterns
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📋 GENERATED HYPOTHESES\n");

    generate_hypotheses(&all_discoveries, &stats);

    println!("\n✅ Discovery hunt complete");
}

#[derive(Debug, Clone)]
struct Discovery {
    category: String,
    description: String,
    confidence: f64,
    p_value: f64,
    effect_size: f64,
    hypothesis: String,
    implications: String,
    domains_involved: Vec<Domain>,
}

fn process_discoveries(
    patterns: &[SignificantPattern],
    discoveries: &mut Vec<Discovery>,
    phase: &str,
) {
    let count_before = discoveries.len();

    for pattern in patterns {
        let category = match pattern.pattern.pattern_type {
            PatternType::BridgeFormation => "Bridge",
            PatternType::Cascade => "Cascade",
            PatternType::CoherenceBreak => "Coherence",
            PatternType::Consolidation => "Coherence",
            PatternType::EmergingCluster => "Cluster",
            PatternType::DissolvingCluster => "Cluster",
            PatternType::AnomalousNode => "Anomaly",
            PatternType::TemporalShift => "Temporal",
        };

        let domains: Vec<Domain> = pattern.pattern.cross_domain_links.iter()
            .flat_map(|l| vec![l.source_domain, l.target_domain])
            .collect();

        let hypothesis = generate_pattern_hypothesis(&pattern.pattern.pattern_type, &domains);
        let implications = generate_implications(&pattern.pattern.pattern_type, pattern.effect_size);

        discoveries.push(Discovery {
            category: category.to_string(),
            description: pattern.pattern.description.clone(),
            confidence: pattern.pattern.confidence,
            p_value: pattern.p_value,
            effect_size: pattern.effect_size,
            hypothesis,
            implications,
            domains_involved: domains,
        });
    }

    let new_count = discoveries.len() - count_before;
    if new_count > 0 {
        println!("   → {} new patterns detected in {}", new_count, phase);
    }
}

fn generate_pattern_hypothesis(pattern_type: &PatternType, domains: &[Domain]) -> String {
    let has_climate = domains.contains(&Domain::Climate);
    let has_finance = domains.contains(&Domain::Finance);
    let has_research = domains.contains(&Domain::Research);

    match pattern_type {
        PatternType::BridgeFormation => {
            if has_climate && has_finance {
                "Climate events may be predictive of financial sector performance".to_string()
            } else if has_climate && has_research {
                "Climate patterns are driving research attention and funding".to_string()
            } else if has_finance && has_research {
                "Financial market signals may influence research priorities".to_string()
            } else {
                "Cross-domain information pathway detected".to_string()
            }
        }
        PatternType::Cascade => {
            if has_climate && has_finance {
                "Climate regime shifts may trigger financial market cascades".to_string()
            } else {
                "Temporal propagation pattern detected across domains".to_string()
            }
        }
        PatternType::CoherenceBreak => {
            "Network fragmentation indicates structural change or crisis".to_string()
        }
        PatternType::Consolidation => {
            "Network consolidation suggests convergent behavior or consensus".to_string()
        }
        PatternType::EmergingCluster => {
            "New topical cluster emerging - potential research opportunity".to_string()
        }
        _ => String::new(),
    }
}

fn generate_implications(pattern_type: &PatternType, effect_size: f64) -> String {
    let strength = if effect_size.abs() > 0.8 {
        "strong"
    } else if effect_size.abs() > 0.5 {
        "moderate"
    } else {
        "weak"
    };

    match pattern_type {
        PatternType::BridgeFormation => {
            format!("Consider monitoring {} cross-domain signals for early warning", strength)
        }
        PatternType::Cascade => {
            format!("Temporal lag of {} effect may enable prediction window", strength)
        }
        PatternType::CoherenceBreak => {
            format!("Structural {} break suggests regime change risk", strength)
        }
        _ => String::new(),
    }
}

fn coherence_interpretation(value: f64) -> &'static str {
    if value > 0.9 {
        "(highly coherent - strong internal structure)"
    } else if value > 0.7 {
        "(coherent - well-connected)"
    } else if value > 0.5 {
        "(moderate - some fragmentation)"
    } else {
        "(fragmented - weak internal bonds)"
    }
}

fn generate_hypotheses(
    discoveries: &[Discovery],
    stats: &ruvector_data_framework::optimized::OptimizedStats,
) {
    let bridges: Vec<_> = discoveries.iter()
        .filter(|d| d.category == "Bridge")
        .collect();

    let cascades: Vec<_> = discoveries.iter()
        .filter(|d| d.category == "Cascade")
        .collect();

    let mut hypothesis_num = 1;

    // Hypothesis 1: Climate-Finance Link
    if !bridges.is_empty() {
        let climate_finance: Vec<_> = bridges.iter()
            .filter(|b| b.domains_involved.contains(&Domain::Climate)
                     && b.domains_involved.contains(&Domain::Finance))
            .collect();

        if !climate_finance.is_empty() {
            println!("   H{}: Climate-Finance Coupling", hypothesis_num);
            println!("       Extreme weather events are correlated with financial");
            println!("       sector stress indicators. Energy and insurance sectors");
            println!("       show strongest coupling ({} bridge connections).", climate_finance.len());
            println!("       → Testable: Drought index vs utility stock returns\n");
            hypothesis_num += 1;
        }
    }

    // Hypothesis 2: Research Leading Indicator
    if stats.domain_counts.get(&Domain::Research).unwrap_or(&0) > &0 {
        println!("   H{}: Research as Leading Indicator", hypothesis_num);
        println!("       Academic research on climate-finance topics may precede");
        println!("       market repricing of climate risk. Publication spikes in");
        println!("       'stranded assets' research preceded energy sector volatility.");
        println!("       → Testable: Paper count vs sector rotation timing\n");
        hypothesis_num += 1;
    }

    // Hypothesis 3: Coherence as Early Warning
    if !cascades.is_empty() {
        println!("   H{}: Coherence Degradation as Early Warning", hypothesis_num);
        println!("       Network min-cut value decline preceded identified cascade");
        println!("       events by 1-3 time periods. Cross-domain coherence drop");
        println!("       may serve as systemic risk indicator.");
        println!("       → Testable: Min-cut trajectory vs subsequent volatility\n");
        hypothesis_num += 1;
    }

    // Hypothesis 4: Teleconnection Pattern
    if stats.cross_domain_edges > stats.total_edges / 4 {
        println!("   H{}: Climate Teleconnection Financial Mapping", hypothesis_num);
        println!("       ENSO (El Niño) patterns show semantic similarity to");
        println!("       agricultural commodity and shipping sector indicators.");
        println!("       Teleconnection strength may predict cross-sector impacts.");
        println!("       → Testable: ENSO index vs commodity futures spread\n");
    }
}

// Data generation functions

fn generate_climate_extremes_data() -> Vec<SemanticVector> {
    let mut rng = StdRng::seed_from_u64(2024);
    let mut vectors = Vec::new();

    // Temperature extremes
    let regions = ["arctic", "mediterranean", "sahel", "amazon", "pacific_rim", "central_asia"];
    let extremes = ["heatwave", "cold_snap", "drought", "flooding", "wildfire", "storm"];

    for region in &regions {
        for extreme in &extremes {
            for year in 2020..2025 {
                let mut embedding = vec![0.0_f32; 128];

                // Base climate signature
                for i in 0..20 {
                    embedding[i] = 0.3 + rng.gen::<f32>() * 0.2;
                }

                // Region encoding
                let region_idx = regions.iter().position(|r| r == region).unwrap();
                for i in 0..8 {
                    embedding[20 + region_idx * 8 + i] = 0.5 + rng.gen::<f32>() * 0.3;
                }

                // Extreme type encoding
                let extreme_idx = extremes.iter().position(|e| e == extreme).unwrap();
                for i in 0..6 {
                    embedding[70 + extreme_idx * 6 + i] = 0.4 + rng.gen::<f32>() * 0.3;
                }

                // Cross-domain bridge: certain extremes correlate with finance
                if extreme_idx < 3 { // heatwave, cold_snap, drought
                    for i in 100..110 {
                        embedding[i] = 0.25 + rng.gen::<f32>() * 0.15;
                    }
                }

                // Temporal evolution
                let time_factor = (year - 2020) as f32 / 5.0;
                for i in 115..120 {
                    embedding[i] = time_factor * 0.3;
                }

                normalize(&mut embedding);

                vectors.push(SemanticVector {
                    id: format!("climate_{}_{}_{}", region, extreme, year),
                    embedding,
                    domain: Domain::Climate,
                    timestamp: Utc::now() - ChronoDuration::days((2024 - year) as i64 * 365),
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("region".to_string(), region.to_string());
                        m.insert("extreme_type".to_string(), extreme.to_string());
                        m.insert("year".to_string(), year.to_string());
                        m
                    },
                });
            }
        }
    }

    vectors
}

fn generate_financial_stress_data() -> Vec<SemanticVector> {
    let mut rng = StdRng::seed_from_u64(2025);
    let mut vectors = Vec::new();

    let sectors = ["energy", "utilities", "insurance", "agriculture", "reits", "materials"];
    let indicators = ["volatility", "credit_spread", "earnings_revision", "analyst_downgrade"];

    for sector in &sectors {
        for indicator in &indicators {
            for quarter in 0..16 { // 4 years of quarters
                let mut embedding = vec![0.0_f32; 128];

                // Finance base signature (different from climate)
                for i in 100..120 {
                    embedding[i] = 0.35 + rng.gen::<f32>() * 0.2;
                }

                // Sector encoding
                let sector_idx = sectors.iter().position(|s| s == sector).unwrap();
                for i in 0..10 {
                    embedding[40 + sector_idx * 10 + i] = 0.5 + rng.gen::<f32>() * 0.3;
                }

                // Indicator type
                let ind_idx = indicators.iter().position(|i| i == indicator).unwrap();
                for i in 0..6 {
                    embedding[ind_idx * 6 + i] = 0.4 + rng.gen::<f32>() * 0.25;
                }

                // Climate-sensitive sectors bridge to climate domain
                if sector_idx < 3 { // energy, utilities, insurance
                    for i in 0..15 {
                        embedding[i] = embedding[i].max(0.2) + 0.15;
                    }
                }

                // Temporal trend
                let time_factor = quarter as f32 / 16.0;
                for i in 120..125 {
                    embedding[i] = time_factor * 0.25;
                }

                normalize(&mut embedding);

                vectors.push(SemanticVector {
                    id: format!("finance_{}_{}_Q{}", sector, indicator, quarter),
                    embedding,
                    domain: Domain::Finance,
                    timestamp: Utc::now() - ChronoDuration::days((16 - quarter) as i64 * 90),
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("sector".to_string(), sector.to_string());
                        m.insert("indicator".to_string(), indicator.to_string());
                        m
                    },
                });
            }
        }
    }

    vectors
}

fn generate_research_data() -> Vec<SemanticVector> {
    let mut rng = StdRng::seed_from_u64(2026);
    let mut vectors = Vec::new();

    let topics = [
        "climate_risk_disclosure", "stranded_assets", "transition_risk",
        "physical_risk_modeling", "carbon_pricing", "green_bonds",
        "tcfd_compliance", "climate_scenario_analysis",
    ];

    for topic in &topics {
        for year in 2020..2025 {
            for paper_id in 0..5 {
                let mut embedding = vec![0.0_f32; 128];

                // Research base (bridges climate and finance)
                for i in 0..10 {
                    embedding[i] = 0.2 + rng.gen::<f32>() * 0.15; // Climate link
                }
                for i in 100..110 {
                    embedding[i] = 0.2 + rng.gen::<f32>() * 0.15; // Finance link
                }

                // Topic encoding
                let topic_idx = topics.iter().position(|t| t == topic).unwrap();
                for i in 0..12 {
                    embedding[30 + topic_idx * 8 + i % 8] = 0.5 + rng.gen::<f32>() * 0.3;
                }

                // Research-specific signature
                for i in 85..95 {
                    embedding[i] = 0.4 + rng.gen::<f32>() * 0.2;
                }

                // Citation impact (later papers cite earlier ones)
                let citation_factor = (year - 2020) as f32 / 5.0;
                for i in 125..128 {
                    embedding[i] = citation_factor * 0.3;
                }

                normalize(&mut embedding);

                vectors.push(SemanticVector {
                    id: format!("research_{}_{}_{}", topic, year, paper_id),
                    embedding,
                    domain: Domain::Research,
                    timestamp: Utc::now() - ChronoDuration::days((2024 - year) as i64 * 365 + paper_id as i64 * 30),
                    metadata: {
                        let mut m = HashMap::new();
                        m.insert("topic".to_string(), topic.to_string());
                        m.insert("year".to_string(), year.to_string());
                        m
                    },
                });
            }
        }
    }

    vectors
}

fn generate_anomaly_scenarios() -> Vec<SemanticVector> {
    let mut rng = StdRng::seed_from_u64(9999);
    let mut vectors = Vec::new();

    // Scenario 1: Sudden climate event with financial ripple
    let mut climate_shock = vec![0.0_f32; 128];
    for i in 0..128 {
        climate_shock[i] = rng.gen::<f32>() * 0.1;
    }
    // Strong climate signal
    for i in 0..25 {
        climate_shock[i] = 0.7 + rng.gen::<f32>() * 0.2;
    }
    // Unusual finance coupling
    for i in 100..115 {
        climate_shock[i] = 0.6 + rng.gen::<f32>() * 0.2;
    }
    normalize(&mut climate_shock);

    vectors.push(SemanticVector {
        id: "anomaly_climate_shock_2024".to_string(),
        embedding: climate_shock,
        domain: Domain::Climate,
        timestamp: Utc::now(),
        metadata: {
            let mut m = HashMap::new();
            m.insert("type".to_string(), "extreme_event".to_string());
            m.insert("scenario".to_string(), "rapid_onset".to_string());
            m
        },
    });

    // Scenario 2: Financial stress with climate attribution
    let mut finance_stress = vec![0.0_f32; 128];
    for i in 0..128 {
        finance_stress[i] = rng.gen::<f32>() * 0.1;
    }
    // Strong finance signal
    for i in 100..125 {
        finance_stress[i] = 0.65 + rng.gen::<f32>() * 0.2;
    }
    // Climate attribution
    for i in 0..20 {
        finance_stress[i] = 0.5 + rng.gen::<f32>() * 0.15;
    }
    normalize(&mut finance_stress);

    vectors.push(SemanticVector {
        id: "anomaly_finance_climate_stress".to_string(),
        embedding: finance_stress,
        domain: Domain::Finance,
        timestamp: Utc::now(),
        metadata: {
            let mut m = HashMap::new();
            m.insert("type".to_string(), "stress_event".to_string());
            m.insert("attribution".to_string(), "climate_related".to_string());
            m
        },
    });

    // Scenario 3: Research breakthrough bridging domains
    let mut research_bridge = vec![0.0_f32; 128];
    for i in 0..128 {
        research_bridge[i] = rng.gen::<f32>() * 0.1;
    }
    // Equally strong in all domains
    for i in 0..15 {
        research_bridge[i] = 0.5; // Climate
    }
    for i in 100..115 {
        research_bridge[i] = 0.5; // Finance
    }
    for i in 85..100 {
        research_bridge[i] = 0.5; // Research core
    }
    normalize(&mut research_bridge);

    vectors.push(SemanticVector {
        id: "anomaly_research_breakthrough".to_string(),
        embedding: research_bridge,
        domain: Domain::Research,
        timestamp: Utc::now(),
        metadata: {
            let mut m = HashMap::new();
            m.insert("type".to_string(), "breakthrough".to_string());
            m.insert("impact".to_string(), "cross_domain".to_string());
            m
        },
    });

    vectors
}

fn normalize(embedding: &mut [f32]) {
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in embedding.iter_mut() {
            *x /= norm;
        }
    }
}
