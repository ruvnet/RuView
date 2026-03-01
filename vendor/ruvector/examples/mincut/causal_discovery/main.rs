//! # Temporal Causal Discovery in Networks
//!
//! This example demonstrates how to discover cause-and-effect relationships
//! in dynamic graph networks using temporal event analysis and Granger-like
//! causality detection.
//!
//! ## Key Concepts:
//! - Event tracking with precise timestamps
//! - Granger causality: X causes Y if past X helps predict Y
//! - Temporal correlation vs causation
//! - Predictive modeling based on learned patterns

use ruvector_mincut::{DynamicMinCut, MinCutBuilder};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Types of events that can occur in the network
#[derive(Debug, Clone)]
enum NetworkEvent {
    /// An edge was cut/removed (from, to, timestamp)
    EdgeCut(usize, usize, Instant),
    /// The minimum cut value changed (new_value, timestamp)
    MinCutChange(f64, Instant),
    /// Network partition changed (partition_a, partition_b, timestamp)
    PartitionChange(Vec<usize>, Vec<usize>, Instant),
    /// A critical node was isolated (node_id, timestamp)
    NodeIsolation(usize, Instant),
}

impl NetworkEvent {
    fn timestamp(&self) -> Instant {
        match self {
            NetworkEvent::EdgeCut(_, _, t) => *t,
            NetworkEvent::MinCutChange(_, t) => *t,
            NetworkEvent::PartitionChange(_, _, t) => *t,
            NetworkEvent::NodeIsolation(_, t) => *t,
        }
    }

    fn event_type(&self) -> &str {
        match self {
            NetworkEvent::EdgeCut(_, _, _) => "EdgeCut",
            NetworkEvent::MinCutChange(_, _) => "MinCutChange",
            NetworkEvent::PartitionChange(_, _, _) => "PartitionChange",
            NetworkEvent::NodeIsolation(_, _) => "NodeIsolation",
        }
    }

    fn description(&self) -> String {
        match self {
            NetworkEvent::EdgeCut(from, to, _) => format!("Edge({}, {})", from, to),
            NetworkEvent::MinCutChange(val, _) => format!("MinCut={:.2}", val),
            NetworkEvent::PartitionChange(a, b, _) => {
                format!("Partition[{}|{}]", a.len(), b.len())
            }
            NetworkEvent::NodeIsolation(node, _) => format!("Node {} isolated", node),
        }
    }
}

/// Represents a discovered causal relationship
#[derive(Debug, Clone)]
struct CausalRelation {
    /// Type of the causing event
    cause_type: String,
    /// Type of the effect event
    effect_type: String,
    /// Confidence score (0.0 to 1.0)
    confidence: f64,
    /// Average time delay between cause and effect
    average_delay: Duration,
    /// Number of times this pattern was observed
    occurrences: usize,
    /// Minimum delay observed
    min_delay: Duration,
    /// Maximum delay observed
    max_delay: Duration,
}

impl CausalRelation {
    fn new(cause: String, effect: String) -> Self {
        Self {
            cause_type: cause,
            effect_type: effect,
            confidence: 0.0,
            average_delay: Duration::from_millis(0),
            occurrences: 0,
            min_delay: Duration::from_secs(999),
            max_delay: Duration::from_millis(0),
        }
    }

    fn add_observation(&mut self, delay: Duration) {
        self.occurrences += 1;

        // Update delay statistics
        let total_ms = self.average_delay.as_millis() as u64 * (self.occurrences - 1) as u64;
        let new_avg_ms = (total_ms + delay.as_millis() as u64) / self.occurrences as u64;
        self.average_delay = Duration::from_millis(new_avg_ms);

        if delay < self.min_delay {
            self.min_delay = delay;
        }
        if delay > self.max_delay {
            self.max_delay = delay;
        }
    }

    fn update_confidence(&mut self, total_cause_events: usize, total_effect_events: usize) {
        // Confidence based on:
        // 1. How often effect follows cause vs total effects
        // 2. Consistency of timing (lower variance = higher confidence)
        let occurrence_ratio = self.occurrences as f64 / total_effect_events.max(1) as f64;

        // Timing consistency (inverse of variance)
        let delay_range = self
            .max_delay
            .as_millis()
            .saturating_sub(self.min_delay.as_millis()) as f64;
        let avg_delay = self.average_delay.as_millis().max(1) as f64;
        let timing_consistency = 1.0 / (1.0 + delay_range / avg_delay);

        // Combined confidence
        self.confidence = (occurrence_ratio * 0.7 + timing_consistency * 0.3).min(1.0);
    }
}

/// Main analyzer for discovering causal relationships in networks
struct CausalNetworkAnalyzer {
    /// All recorded events in chronological order
    events: Vec<NetworkEvent>,
    /// Discovered causal relationships
    causal_relations: HashMap<(String, String), CausalRelation>,
    /// Maximum time window for causality detection (ms)
    causality_window: Duration,
    /// Minimum confidence threshold for reporting
    confidence_threshold: f64,
}

impl CausalNetworkAnalyzer {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            causal_relations: HashMap::new(),
            causality_window: Duration::from_millis(200),
            confidence_threshold: 0.3,
        }
    }

    /// Record a new event
    fn record_event(&mut self, event: NetworkEvent) {
        self.events.push(event);
    }

    /// Analyze all events to discover causal relationships
    fn discover_causality(&mut self) {
        println!(
            "\n🔍 Analyzing {} events for causal patterns...",
            self.events.len()
        );

        // For each event, look for preceding events that might be causes
        for i in 0..self.events.len() {
            let effect = &self.events[i];
            let effect_time = effect.timestamp();
            let effect_type = effect.event_type().to_string();

            // Look backwards in time for potential causes
            for j in (0..i).rev() {
                let cause = &self.events[j];
                let cause_time = cause.timestamp();

                // Calculate time difference
                let delay = effect_time.duration_since(cause_time);

                // Check if within causality window
                if delay > self.causality_window {
                    break; // Too far back in time
                }

                let cause_type = cause.event_type().to_string();
                let key = (cause_type.clone(), effect_type.clone());

                // Record this potential causal relationship
                self.causal_relations
                    .entry(key.clone())
                    .or_insert_with(|| CausalRelation::new(cause_type.clone(), effect_type.clone()))
                    .add_observation(delay);
            }
        }

        // Update confidence scores
        let event_counts = self.count_events_by_type();

        // Collect counts first to avoid borrow issues
        let counts_vec: Vec<_> = self
            .causal_relations
            .keys()
            .map(|(cause_type, effect_type)| {
                let cause_count = *event_counts.get(cause_type.as_str()).unwrap_or(&0);
                let effect_count = *event_counts.get(effect_type.as_str()).unwrap_or(&0);
                (
                    (cause_type.clone(), effect_type.clone()),
                    cause_count,
                    effect_count,
                )
            })
            .collect();

        for ((cause_type, effect_type), cause_count, effect_count) in counts_vec {
            if let Some(relation) = self.causal_relations.get_mut(&(cause_type, effect_type)) {
                relation.update_confidence(cause_count, effect_count);
            }
        }
    }

    /// Count events by type
    fn count_events_by_type(&self) -> HashMap<&str, usize> {
        let mut counts = HashMap::new();
        for event in &self.events {
            *counts.entry(event.event_type()).or_insert(0) += 1;
        }
        counts
    }

    /// Get significant causal relationships
    fn get_significant_relations(&self) -> Vec<&CausalRelation> {
        let mut relations: Vec<_> = self
            .causal_relations
            .values()
            .filter(|r| r.confidence >= self.confidence_threshold && r.occurrences >= 2)
            .collect();

        relations.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        relations
    }

    /// Predict what might happen next based on recent events
    fn predict_next_events(&self, lookback_ms: u64) -> Vec<(String, f64, Duration)> {
        if self.events.is_empty() {
            return Vec::new();
        }

        let last_event_time = self.events.last().unwrap().timestamp();
        let lookback_window = Duration::from_millis(lookback_ms);

        // Find recent events
        let recent_events: Vec<_> = self
            .events
            .iter()
            .rev()
            .take_while(|e| last_event_time.duration_since(e.timestamp()) <= lookback_window)
            .collect();

        if recent_events.is_empty() {
            return Vec::new();
        }

        println!(
            "\n🔮 Analyzing {} recent events for predictions...",
            recent_events.len()
        );

        // For each recent event, find what it typically causes
        let mut predictions: HashMap<String, (f64, Duration, usize)> = HashMap::new();

        for recent_event in recent_events {
            let cause_type = recent_event.event_type();

            // Find all effects this cause type produces
            for ((cause, effect), relation) in &self.causal_relations {
                if cause == cause_type && relation.confidence >= self.confidence_threshold {
                    let entry = predictions.entry(effect.clone()).or_insert((
                        0.0,
                        Duration::from_millis(0),
                        0,
                    ));
                    entry.0 += relation.confidence;
                    entry.1 += relation.average_delay;
                    entry.2 += 1;
                }
            }
        }

        // Calculate average confidence and delay for each prediction
        let mut result: Vec<_> = predictions
            .into_iter()
            .map(|(effect, (total_conf, total_delay, count))| {
                let avg_conf = total_conf / count as f64;
                let avg_delay = total_delay / count as u32;
                (effect, avg_conf, avg_delay)
            })
            .collect();

        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        result
    }

    /// Print causal graph visualization
    fn print_causal_graph(&self) {
        println!("\n📊 CAUSAL GRAPH");
        println!("═══════════════════════════════════════════════════════════");

        let relations = self.get_significant_relations();

        if relations.is_empty() {
            println!("No significant causal relationships found.");
            return;
        }

        for relation in relations {
            println!(
                "{} ──[{:.0}ms]──> {} (confidence: {:.1}%, n={})",
                relation.cause_type,
                relation.average_delay.as_millis(),
                relation.effect_type,
                relation.confidence * 100.0,
                relation.occurrences
            );
            println!(
                "  └─ Delay range: {:.0}ms - {:.0}ms",
                relation.min_delay.as_millis(),
                relation.max_delay.as_millis()
            );
        }
    }

    /// Print event timeline
    fn print_timeline(&self, max_events: usize) {
        println!("\n📅 EVENT TIMELINE (last {} events)", max_events);
        println!("═══════════════════════════════════════════════════════════");

        let start_time = self
            .events
            .first()
            .map(|e| e.timestamp())
            .unwrap_or_else(Instant::now);

        for event in self.events.iter().rev().take(max_events).rev() {
            let elapsed = event.timestamp().duration_since(start_time);
            println!(
                "T+{:6.0}ms: {} - {}",
                elapsed.as_millis(),
                event.event_type(),
                event.description()
            );
        }
    }
}

/// Simulate a dynamic network with events
fn simulate_dynamic_network(analyzer: &mut CausalNetworkAnalyzer) {
    println!("🌐 Simulating dynamic network operations...\n");

    let start_time = Instant::now();

    // Build initial network
    let edges = vec![
        (0, 1, 5.0),
        (0, 2, 3.0),
        (1, 2, 2.0),
        (1, 3, 6.0),
        (2, 3, 4.0),
        (2, 4, 3.0),
        (3, 5, 4.0),
        (4, 5, 2.0),
        (4, 6, 5.0),
        (5, 7, 3.0),
        (6, 7, 4.0),
    ];

    let mut mincut = MinCutBuilder::new()
        .exact()
        .with_edges(edges.clone())
        .build()
        .expect("Failed to build mincut");

    // Calculate initial mincut
    let initial_cut = mincut.min_cut_value();
    println!("Initial MinCut: {:.2}", initial_cut);

    analyzer.record_event(NetworkEvent::MinCutChange(initial_cut, Instant::now()));

    std::thread::sleep(Duration::from_millis(20));

    // Simulate sequence of network changes
    println!("\n--- Simulating network dynamics ---\n");

    // Scenario 1: Cut critical edge -> causes mincut change
    println!("📌 Cutting edge (1, 3)...");
    let _ = mincut.delete_edge(1, 3);
    analyzer.record_event(NetworkEvent::EdgeCut(1, 3, Instant::now()));

    std::thread::sleep(Duration::from_millis(30));

    let new_cut = mincut.min_cut_value();
    println!("   MinCut changed: {:.2} → {:.2}", initial_cut, new_cut);
    analyzer.record_event(NetworkEvent::MinCutChange(new_cut, Instant::now()));

    std::thread::sleep(Duration::from_millis(25));

    // Scenario 2: Cut another edge -> causes partition change
    println!("\n📌 Cutting edge (2, 4)...");
    let _ = mincut.delete_edge(2, 4);
    analyzer.record_event(NetworkEvent::EdgeCut(2, 4, Instant::now()));

    std::thread::sleep(Duration::from_millis(40));

    analyzer.record_event(NetworkEvent::PartitionChange(
        vec![0, 1, 2],
        vec![3, 4, 5, 6, 7],
        Instant::now(),
    ));

    std::thread::sleep(Duration::from_millis(15));

    // Scenario 3: Multiple edge cuts leading to node isolation
    println!("\n📌 Cutting edges around node 4...");
    let _ = mincut.delete_edge(3, 5);
    analyzer.record_event(NetworkEvent::EdgeCut(3, 5, Instant::now()));

    std::thread::sleep(Duration::from_millis(35));

    let _ = mincut.delete_edge(4, 6);
    analyzer.record_event(NetworkEvent::EdgeCut(4, 6, Instant::now()));

    std::thread::sleep(Duration::from_millis(45));

    analyzer.record_event(NetworkEvent::NodeIsolation(4, Instant::now()));

    std::thread::sleep(Duration::from_millis(20));

    let final_cut = mincut.min_cut_value();
    analyzer.record_event(NetworkEvent::MinCutChange(final_cut, Instant::now()));

    println!("\n   Final MinCut: {:.2}", final_cut);

    let total_time = Instant::now().duration_since(start_time);
    println!("\nSimulation completed in {:.0}ms", total_time.as_millis());
}

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║  TEMPORAL CAUSAL DISCOVERY IN NETWORKS                    ║");
    println!("║  Discovering Cause-Effect Relationships in Dynamic Graphs ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let mut analyzer = CausalNetworkAnalyzer::new();

    // Run simulation
    simulate_dynamic_network(&mut analyzer);

    // Show event timeline
    analyzer.print_timeline(15);

    // Discover causal relationships
    analyzer.discover_causality();

    // Display causal graph
    analyzer.print_causal_graph();

    // Make predictions
    println!("\n🔮 PREDICTIONS");
    println!("═══════════════════════════════════════════════════════════");
    println!("Based on recent events, likely future events:");

    let predictions = analyzer.predict_next_events(100);

    if predictions.is_empty() {
        println!("No predictions available (insufficient causal data).");
    } else {
        for (i, (event_type, confidence, expected_delay)) in predictions.iter().enumerate() {
            println!(
                "{}. {} in ~{:.0}ms (confidence: {:.1}%)",
                i + 1,
                event_type,
                expected_delay.as_millis(),
                confidence * 100.0
            );
        }
    }

    // Explain concepts
    println!("\n\n💡 KEY CONCEPTS");
    println!("═══════════════════════════════════════════════════════════");
    println!("1. CORRELATION vs CAUSATION:");
    println!("   - Correlation: Events happen together");
    println!("   - Causation: One event CAUSES another");
    println!("   - We use temporal ordering: causes precede effects");

    println!("\n2. GRANGER CAUSALITY:");
    println!("   - Event X 'Granger-causes' Y if:");
    println!("     * X consistently occurs before Y");
    println!("     * Knowing X improves prediction of Y");
    println!("     * Time delay is consistent");

    println!("\n3. PRACTICAL APPLICATIONS:");
    println!("   - Network failure prediction");
    println!("   - Anomaly detection (unexpected causal chains)");
    println!("   - System optimization (remove causal bottlenecks)");
    println!("   - Root cause analysis in distributed systems");

    println!("\n4. TEMPORAL WINDOW:");
    println!(
        "   - {}ms window used for causality",
        analyzer.causality_window.as_millis()
    );
    println!("   - Events within window may be causally related");
    println!("   - Longer window = more potential causes found");

    println!("\n✅ Analysis complete!");
}
