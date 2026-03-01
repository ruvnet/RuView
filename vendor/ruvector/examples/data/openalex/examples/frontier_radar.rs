//! OpenAlex Research Frontier Discovery
//!
//! This example detects emerging research frontiers using citation graph analysis
//! and RuVector's dynamic coherence detection.

use chrono::{Duration, Utc};
use ruvector_data_openalex::{
    OpenAlexClient, OpenAlexConfig, EntityType,
    TopicGraph, TopicNode, TopicEdge,
    frontier::{FrontierRadar, FrontierConfig},
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          OpenAlex Research Frontier Discovery                 ║");
    println!("║    Detecting Emerging Research via Citation Dynamics          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize OpenAlex client
    let config = OpenAlexConfig {
        email: Some("ruvector-discovery@example.com".to_string()),
        per_page: 200,
        ..Default::default()
    };
    let client = OpenAlexClient::new(config);

    // Research areas to scan for emerging frontiers
    let research_domains = [
        ("Quantum Machine Learning", "quantum computing AND machine learning"),
        ("Foundation Models", "large language model OR foundation model"),
        ("Embodied AI", "embodied AI OR robotics learning"),
        ("Mechanistic Interpretability", "interpretability AND neural network"),
        ("AI Safety", "AI safety OR alignment"),
        ("Synthetic Biology AI", "synthetic biology AND AI"),
        ("Climate AI", "climate AND machine learning"),
        ("Materials Discovery", "materials discovery AND AI"),
    ];

    println!("🔍 Scanning {} research domains for emerging frontiers...\n", research_domains.len());

    // Configure frontier detection
    let frontier_config = FrontierConfig {
        min_growth_rate: 0.15,           // 15% citation growth threshold
        coherence_sensitivity: 0.7,       // High sensitivity to structure changes
        time_window_months: 6,            // Look at last 6 months
        min_boundary_topics: 3,           // Minimum topics at frontier
        min_papers: 10,                   // Minimum papers to consider
    };

    let mut radar = FrontierRadar::new(frontier_config);

    let mut all_discoveries = Vec::new();
    let cutoff_date = Utc::now() - Duration::days(180);

    for (domain_name, query) in &research_domains {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📚 Domain: {}", domain_name);
        println!();

        // Fetch recent works in this domain
        match client.search_works(query, Some(cutoff_date)).await {
            Ok(works) => {
                println!("   Found {} recent papers", works.len());

                if works.is_empty() {
                    println!("   ⚠️ No papers found, skipping domain\n");
                    continue;
                }

                // Build topic citation graph
                let mut topic_graph = TopicGraph::new();
                let mut topic_papers: HashMap<String, Vec<String>> = HashMap::new();
                let mut topic_citations: HashMap<String, usize> = HashMap::new();

                for work in &works {
                    if let Some(concepts) = &work.concepts {
                        // Add topics and track papers per topic
                        for concept in concepts.iter().filter(|c| c.score > 0.3) {
                            let topic_id = concept.id.clone();

                            // Add topic node if not exists
                            if !topic_graph.nodes.iter().any(|n| n.id == topic_id) {
                                topic_graph.nodes.push(TopicNode {
                                    id: topic_id.clone(),
                                    name: concept.display_name.clone(),
                                    level: concept.level as usize,
                                    paper_count: 1,
                                    citation_count: work.cited_by_count.unwrap_or(0) as usize,
                                    score: concept.score,
                                });
                            } else {
                                // Update counts
                                if let Some(node) = topic_graph.nodes.iter_mut().find(|n| n.id == topic_id) {
                                    node.paper_count += 1;
                                    node.citation_count += work.cited_by_count.unwrap_or(0) as usize;
                                }
                            }

                            topic_papers.entry(topic_id.clone()).or_default().push(work.id.clone());
                            *topic_citations.entry(topic_id.clone()).or_insert(0) += work.cited_by_count.unwrap_or(0) as usize;
                        }

                        // Build edges between co-occurring topics
                        let topic_ids: Vec<String> = concepts.iter()
                            .filter(|c| c.score > 0.3)
                            .map(|c| c.id.clone())
                            .collect();

                        for i in 0..topic_ids.len() {
                            for j in (i + 1)..topic_ids.len() {
                                let source = &topic_ids[i];
                                let target = &topic_ids[j];

                                // Check if edge exists
                                if let Some(edge) = topic_graph.edges.iter_mut()
                                    .find(|e| (e.source == *source && e.target == *target) ||
                                              (e.source == *target && e.target == *source)) {
                                    edge.weight += 1.0;
                                } else {
                                    topic_graph.edges.push(TopicEdge {
                                        source: source.clone(),
                                        target: target.clone(),
                                        weight: 1.0,
                                        citation_flow: 0,
                                    });
                                }
                            }
                        }
                    }
                }

                println!("   Built topic graph: {} nodes, {} edges",
                    topic_graph.nodes.len(), topic_graph.edges.len());

                // Add snapshot to radar
                radar.add_snapshot(Utc::now(), topic_graph.clone());

                // Analyze for emerging frontiers (need at least 2 snapshots for delta)
                // For demo, we analyze the single snapshot structure
                let discoveries = analyze_frontier_structure(&topic_graph, domain_name);

                if !discoveries.is_empty() {
                    println!("\n   🌟 Potential Frontiers Detected:\n");
                    for discovery in &discoveries {
                        all_discoveries.push(discovery.clone());
                        println!("   {}", discovery);
                    }
                } else {
                    println!("   📊 No clear frontier signals in current snapshot");
                }
            }
            Err(e) => {
                println!("   ❌ Error fetching papers: {}", e);
            }
        }
        println!();
    }

    // Cross-domain bridge analysis
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🌉 Cross-Domain Bridge Analysis");
    println!();

    // Look for papers bridging multiple domains
    let bridge_query = "(quantum AND machine learning) OR (biology AND AI AND materials)";
    match client.search_works(bridge_query, Some(cutoff_date)).await {
        Ok(bridge_works) => {
            println!("   Found {} potential bridge papers\n", bridge_works.len());

            // Analyze bridge patterns
            let bridges = analyze_bridge_papers(&bridge_works);
            for bridge in &bridges {
                println!("   {}", bridge);
                all_discoveries.push(bridge.clone());
            }
        }
        Err(e) => {
            println!("   ❌ Error: {}", e);
        }
    }

    // Summary
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                    Discovery Summary                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Total potential frontiers identified: {}", all_discoveries.len());
    println!();

    // Rank discoveries by potential
    println!("📈 Top Emerging Areas (by structural signals):\n");
    for (i, discovery) in all_discoveries.iter().take(5).enumerate() {
        println!("   {}. {}", i + 1, discovery);
    }

    Ok(())
}

/// Analyze topic graph structure for frontier signals
fn analyze_frontier_structure(graph: &TopicGraph, domain: &str) -> Vec<String> {
    let mut discoveries = Vec::new();

    // 1. High-degree but low-level topics (emerging connectors)
    let mut topic_degrees: HashMap<&str, usize> = HashMap::new();
    for edge in &graph.edges {
        *topic_degrees.entry(&edge.source).or_insert(0) += 1;
        *topic_degrees.entry(&edge.target).or_insert(0) += 1;
    }

    for node in &graph.nodes {
        let degree = topic_degrees.get(node.id.as_str()).copied().unwrap_or(0);

        // Look for topics that connect many others but have relatively few papers
        // This indicates an emerging organizing concept
        if degree > 3 && node.paper_count < 20 && node.level >= 2 {
            discoveries.push(format!(
                "🔺 [{}] '{}' - High connectivity ({} connections), only {} papers - potential emerging organizer",
                domain, node.name, degree, node.paper_count
            ));
        }

        // Look for high citation velocity (citations per paper)
        if node.paper_count > 5 {
            let citation_velocity = node.citation_count as f64 / node.paper_count as f64;
            if citation_velocity > 20.0 {
                discoveries.push(format!(
                    "🔥 [{}] '{}' - High citation velocity ({:.1} citations/paper) - gaining attention",
                    domain, node.name, citation_velocity
                ));
            }
        }
    }

    // 2. Detect weakly connected clusters (potential specialization frontiers)
    let components = find_weak_bridges(graph);
    for (topic1, topic2, bridge_strength) in components {
        if bridge_strength < 3.0 && bridge_strength > 0.0 {
            discoveries.push(format!(
                "🌉 [{}] Weak bridge between '{}' and '{}' (strength {:.1}) - potential specialization point",
                domain, topic1, topic2, bridge_strength
            ));
        }
    }

    discoveries
}

/// Find weak bridges between topic clusters
fn find_weak_bridges(graph: &TopicGraph) -> Vec<(String, String, f64)> {
    let mut bridges = Vec::new();

    // Simple heuristic: edges with low weight connecting high-degree nodes
    let mut topic_degrees: HashMap<&str, usize> = HashMap::new();
    for edge in &graph.edges {
        *topic_degrees.entry(&edge.source).or_insert(0) += 1;
        *topic_degrees.entry(&edge.target).or_insert(0) += 1;
    }

    for edge in &graph.edges {
        let source_degree = topic_degrees.get(edge.source.as_str()).copied().unwrap_or(0);
        let target_degree = topic_degrees.get(edge.target.as_str()).copied().unwrap_or(0);

        // High-degree nodes connected by weak edge = potential bridge
        if source_degree > 3 && target_degree > 3 && edge.weight < 3.0 {
            let source_name = graph.nodes.iter()
                .find(|n| n.id == edge.source)
                .map(|n| n.name.clone())
                .unwrap_or_else(|| edge.source.clone());
            let target_name = graph.nodes.iter()
                .find(|n| n.id == edge.target)
                .map(|n| n.name.clone())
                .unwrap_or_else(|| edge.target.clone());

            bridges.push((source_name, target_name, edge.weight));
        }
    }

    bridges
}

/// Analyze papers that bridge multiple research domains
fn analyze_bridge_papers(works: &[ruvector_data_openalex::Work]) -> Vec<String> {
    let mut discoveries = Vec::new();

    // Group papers by their concept combinations
    let mut concept_combos: HashMap<String, Vec<&ruvector_data_openalex::Work>> = HashMap::new();

    for work in works {
        if let Some(concepts) = &work.concepts {
            // Get high-level concepts (level 0-1)
            let high_level: Vec<String> = concepts.iter()
                .filter(|c| c.level <= 1 && c.score > 0.4)
                .map(|c| c.display_name.clone())
                .collect();

            if high_level.len() >= 2 {
                let key = format!("{} ↔ {}", high_level[0], high_level[1]);
                concept_combos.entry(key).or_default().push(work);
            }
        }
    }

    // Report unusual combinations with high citations
    for (combo, papers) in &concept_combos {
        let total_citations: i32 = papers.iter()
            .filter_map(|w| w.cited_by_count)
            .sum();
        let avg_citations = total_citations as f64 / papers.len() as f64;

        if papers.len() >= 3 && avg_citations > 10.0 {
            discoveries.push(format!(
                "🔗 Bridge area: {} ({} papers, {:.0} avg citations) - cross-domain synthesis",
                combo, papers.len(), avg_citations
            ));
        }
    }

    discoveries
}
