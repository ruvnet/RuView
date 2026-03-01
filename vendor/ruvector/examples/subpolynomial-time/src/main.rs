//! Subpolynomial-Time Dynamic Minimum Cut Demo
//!
//! This example demonstrates the key features of the ruvector-mincut crate:
//! 1. Basic minimum cut computation
//! 2. Dynamic updates (insert/delete edges)
//! 3. Exact vs approximate modes
//! 4. Real-time monitoring
//! 5. Network resilience analysis
//! 6. Performance scaling
//! 7. Vector-Graph Fusion with brittleness detection

use rand::prelude::*;
use ruvector_mincut::prelude::*;
use ruvector_mincut::{EventType, MonitorBuilder};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

mod fusion;
use fusion::{
    BrittlenessSignal, FusionConfig, FusionGraph, Optimizer, OptimizerAction, RelationType,
    StructuralMonitor, StructuralMonitorConfig,
};

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  Subpolynomial-Time Dynamic Minimum Cut Algorithm Demo      ║");
    println!("║  ruvector-mincut v0.1.0 + Vector-Graph Fusion               ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // Demo 1: Basic usage
    demo_basic_usage();
    println!("\n{}\n", "─".repeat(64));

    // Demo 2: Dynamic updates
    demo_dynamic_updates();
    println!("\n{}\n", "─".repeat(64));

    // Demo 3: Exact vs approximate
    demo_exact_vs_approximate();
    println!("\n{}\n", "─".repeat(64));

    // Demo 4: Real-time monitoring
    demo_monitoring();
    println!("\n{}\n", "─".repeat(64));

    // Demo 5: Network resilience
    demo_network_resilience();
    println!("\n{}\n", "─".repeat(64));

    // Demo 6: Performance scaling
    demo_performance_scaling();
    println!("\n{}\n", "─".repeat(64));

    // Demo 7: Vector-Graph Fusion
    demo_vector_graph_fusion();
    println!("\n{}\n", "─".repeat(64));

    // Demo 8: Brittleness Detection
    demo_brittleness_detection();
    println!("\n{}\n", "─".repeat(64));

    // Demo 9: Self-Learning Optimization
    demo_self_learning_optimization();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  Demo Complete!                                              ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}

/// Demo 1: Basic minimum cut computation
fn demo_basic_usage() {
    println!("📊 DEMO 1: Basic Minimum Cut Computation");
    println!("Creating a triangle graph with vertices 1, 2, 3...\n");

    // Create a triangle graph: 1-2, 2-3, 3-1
    let mincut = MinCutBuilder::new()
        .exact()
        .with_edges(vec![(1, 2, 1.0), (2, 3, 1.0), (3, 1, 1.0)])
        .build()
        .expect("Failed to build mincut");

    println!("Graph created:");
    println!("  • Vertices: {}", mincut.num_vertices());
    println!("  • Edges: {}", mincut.num_edges());
    println!("  • Connected: {}", mincut.is_connected());

    // Query the minimum cut
    let result = mincut.min_cut();
    println!("\nMinimum cut result:");
    println!("  • Value: {}", result.value);
    println!("  • Is exact: {}", result.is_exact);
    println!("  • Approximation ratio: {}", result.approximation_ratio);

    if let Some((s, t)) = result.partition {
        println!("  • Partition S: {:?}", s);
        println!("  • Partition T: {:?}", t);
    }

    if let Some(cut_edges) = result.cut_edges {
        println!("  • Number of cut edges: {}", cut_edges.len());
        for edge in &cut_edges {
            println!(
                "    - Edge ({}, {}) with weight {}",
                edge.source, edge.target, edge.weight
            );
        }
    }

    // Get graph statistics
    let graph = mincut.graph();
    let stats = graph.read().stats();
    println!("\nGraph statistics:");
    println!("  • Total weight: {}", stats.total_weight);
    println!("  • Min degree: {}", stats.min_degree);
    println!("  • Max degree: {}", stats.max_degree);
    println!("  • Avg degree: {:.2}", stats.avg_degree);
}

/// Demo 2: Dynamic edge insertions and deletions
fn demo_dynamic_updates() {
    println!("🔄 DEMO 2: Dynamic Updates");
    println!("Starting with an empty graph and adding edges dynamically...\n");

    let mut mincut = MinCutBuilder::new()
        .exact()
        .build()
        .expect("Failed to build mincut");

    println!("Initial state:");
    println!("  • Min cut: {}", mincut.min_cut_value());

    // Insert edges one by one
    println!("\nInserting edge (1, 2)...");
    let cut = mincut.insert_edge(1, 2, 1.0).expect("Insert failed");
    println!("  • New min cut: {}", cut);

    println!("Inserting edge (2, 3)...");
    let cut = mincut.insert_edge(2, 3, 1.0).expect("Insert failed");
    println!("  • New min cut: {}", cut);

    println!("Inserting edge (3, 1)...");
    let cut = mincut.insert_edge(3, 1, 1.0).expect("Insert failed");
    println!("  • New min cut: {} (triangle formed)", cut);

    // Add a fourth vertex
    println!("\nAdding vertex 4 with edge to vertex 3...");
    println!("Inserting edge (3, 4)...");
    let cut = mincut.insert_edge(3, 4, 2.0).expect("Insert failed");
    println!("  • New min cut: {}", cut);

    // Now delete an edge from the triangle
    println!("\nDeleting edge (3, 1)...");
    let cut = mincut.delete_edge(3, 1).expect("Delete failed");
    println!("  • New min cut: {} (triangle broken)", cut);

    // Add it back
    println!("\nRe-inserting edge (1, 3)...");
    let cut = mincut.insert_edge(1, 3, 1.5).expect("Insert failed");
    println!("  • New min cut: {} (different weight this time)", cut);

    // Check algorithm statistics
    let stats = mincut.stats();
    println!("\nAlgorithm statistics:");
    println!(
        "  • Total insertions: {} (including re-insertion)",
        stats.insertions
    );
    println!("  • Total deletions: {}", stats.deletions);
    println!("  • Total queries: {}", stats.queries);
    println!("  • Avg update time: {:.2} μs", stats.avg_update_time_us);
    println!("  • Avg query time: {:.2} μs", stats.avg_query_time_us);
}

/// Demo 3: Exact vs approximate algorithms
fn demo_exact_vs_approximate() {
    println!("⚖️  DEMO 3: Exact vs Approximate Algorithms");
    println!("Comparing exact and approximate modes on the same graph...\n");

    // Create test graph: a bridge graph (two triangles connected by an edge)
    let edges = vec![
        // Triangle 1
        (1, 2, 2.0),
        (2, 3, 2.0),
        (3, 1, 2.0),
        // Bridge
        (3, 4, 1.0),
        // Triangle 2
        (4, 5, 2.0),
        (5, 6, 2.0),
        (6, 4, 2.0),
    ];

    // Exact mode
    println!("Building with exact algorithm...");
    let start = Instant::now();
    let exact_mincut = MinCutBuilder::new()
        .exact()
        .with_edges(edges.clone())
        .build()
        .expect("Failed to build exact");
    let exact_time = start.elapsed();

    let exact_result = exact_mincut.min_cut();
    println!("Exact algorithm:");
    println!("  • Build time: {:?}", exact_time);
    println!("  • Min cut value: {}", exact_result.value);
    println!("  • Is exact: {}", exact_result.is_exact);
    println!(
        "  • Approximation ratio: {}",
        exact_result.approximation_ratio
    );

    // Approximate mode with ε = 0.1 (10% approximation)
    println!("\nBuilding with approximate algorithm (ε = 0.1)...");
    let start = Instant::now();
    let approx_mincut = MinCutBuilder::new()
        .approximate(0.1)
        .with_edges(edges.clone())
        .build()
        .expect("Failed to build approximate");
    let approx_time = start.elapsed();

    let approx_result = approx_mincut.min_cut();
    println!("Approximate algorithm:");
    println!("  • Build time: {:?}", approx_time);
    println!("  • Min cut value: {}", approx_result.value);
    println!("  • Is exact: {}", approx_result.is_exact);
    println!(
        "  • Approximation ratio: {}",
        approx_result.approximation_ratio
    );

    // Compare results
    println!("\nComparison:");
    println!("  • Exact value: {}", exact_result.value);
    println!("  • Approximate value: {}", approx_result.value);
    let error = ((approx_result.value - exact_result.value) / exact_result.value * 100.0).abs();
    println!("  • Error: {:.2}%", error);
    println!(
        "  • Speedup: {:.2}x",
        exact_time.as_secs_f64() / approx_time.as_secs_f64()
    );
}

/// Demo 4: Real-time monitoring with thresholds
fn demo_monitoring() {
    println!("📡 DEMO 4: Real-time Monitoring");
    println!("Setting up event monitoring with thresholds...\n");

    // Create counters for different event types
    let cut_increased_count = Arc::new(AtomicU64::new(0));
    let cut_decreased_count = Arc::new(AtomicU64::new(0));
    let threshold_count = Arc::new(AtomicU64::new(0));
    let disconnected_count = Arc::new(AtomicU64::new(0));

    // Build monitor with thresholds
    let inc_clone = cut_increased_count.clone();
    let dec_clone = cut_decreased_count.clone();
    let thr_clone = threshold_count.clone();
    let dis_clone = disconnected_count.clone();

    let monitor = MonitorBuilder::new()
        .threshold_below(1.5, "critical")
        .threshold_above(5.0, "warning")
        .on_event_type(EventType::CutIncreased, "inc_cb", move |event| {
            inc_clone.fetch_add(1, Ordering::SeqCst);
            println!(
                "  [EVENT] Cut increased: {} → {}",
                event.old_value, event.new_value
            );
        })
        .on_event_type(EventType::CutDecreased, "dec_cb", move |event| {
            dec_clone.fetch_add(1, Ordering::SeqCst);
            println!(
                "  [EVENT] Cut decreased: {} → {}",
                event.old_value, event.new_value
            );
        })
        .on_event_type(EventType::ThresholdCrossedBelow, "thr_cb", move |event| {
            thr_clone.fetch_add(1, Ordering::SeqCst);
            println!(
                "  [ALERT] Threshold crossed below: {} (threshold: {:?})",
                event.new_value, event.threshold
            );
        })
        .on_event_type(EventType::Disconnected, "dis_cb", move |_event| {
            dis_clone.fetch_add(1, Ordering::SeqCst);
            println!("  [CRITICAL] Graph became disconnected!");
        })
        .build();

    println!("Monitor configured with:");
    println!("  • Critical threshold: < 1.5");
    println!("  • Warning threshold: > 5.0");
    println!("  • 4 event callbacks registered\n");

    // Simulate a series of graph changes
    println!("Simulating graph updates...\n");

    monitor.notify(0.0, 2.0, Some((1, 2)));
    std::thread::sleep(std::time::Duration::from_millis(10));

    monitor.notify(2.0, 3.0, Some((2, 3)));
    std::thread::sleep(std::time::Duration::from_millis(10));

    monitor.notify(3.0, 1.0, None);
    std::thread::sleep(std::time::Duration::from_millis(10));

    monitor.notify(1.0, 6.0, Some((3, 4)));
    std::thread::sleep(std::time::Duration::from_millis(10));

    monitor.notify(6.0, 0.0, None);
    std::thread::sleep(std::time::Duration::from_millis(10));

    // Get metrics
    let metrics = monitor.metrics();
    println!("\nMonitoring metrics:");
    println!("  • Total events: {}", metrics.total_events);
    println!(
        "  • Cut increased events: {}",
        cut_increased_count.load(Ordering::SeqCst)
    );
    println!(
        "  • Cut decreased events: {}",
        cut_decreased_count.load(Ordering::SeqCst)
    );
    println!(
        "  • Threshold violations: {}",
        threshold_count.load(Ordering::SeqCst)
    );
    println!(
        "  • Disconnection events: {}",
        disconnected_count.load(Ordering::SeqCst)
    );
    println!("  • Min observed cut: {}", metrics.min_observed);
    println!("  • Max observed cut: {}", metrics.max_observed);
    println!("  • Average cut: {:.2}", metrics.avg_cut);

    // Print event breakdown
    println!("\nEvents by type:");
    for (event_type, count) in &metrics.events_by_type {
        println!("  • {}: {}", event_type, count);
    }
}

/// Demo 5: Network resilience analysis
fn demo_network_resilience() {
    println!("🛡️  DEMO 5: Network Resilience Analysis");
    println!("Analyzing a network's resistance to failures...\n");

    // Create a network topology: a mesh with redundant paths
    println!("Building a mesh network (6 nodes, 9 edges)...");
    let mincut = MinCutBuilder::new()
        .exact()
        .with_edges(vec![
            // Core ring
            (1, 2, 1.0),
            (2, 3, 1.0),
            (3, 4, 1.0),
            (4, 5, 1.0),
            (5, 6, 1.0),
            (6, 1, 1.0),
            // Cross connections for redundancy
            (1, 3, 1.0),
            (2, 4, 1.0),
            (3, 5, 1.0),
        ])
        .build()
        .expect("Failed to build network");

    let graph = mincut.graph();
    let stats = graph.read().stats();

    println!("\nNetwork topology:");
    println!("  • Nodes: {}", stats.num_vertices);
    println!("  • Links: {}", stats.num_edges);
    println!("  • Avg degree: {:.2}", stats.avg_degree);
    println!("  • Min cut: {}", mincut.min_cut_value());

    println!("\nResilience interpretation:");
    let min_cut = mincut.min_cut_value();
    if min_cut == 0.0 {
        println!("  ❌ Network is disconnected - no resilience");
    } else if min_cut == 1.0 {
        println!("  ⚠️  Single point of failure - low resilience");
    } else if min_cut == 2.0 {
        println!("  ⚡ Moderate resilience - can survive 1 failure");
    } else {
        println!(
            "  ✅ High resilience - can survive {} failures",
            min_cut as u32 - 1
        );
    }

    // Simulate edge failures
    println!("\nSimulating link failures...");
    let result = mincut.min_cut();
    if let Some(cut_edges) = result.cut_edges {
        println!("\nCritical edges (minimum cut set):");
        for (i, edge) in cut_edges.iter().enumerate() {
            println!(
                "  {}. ({}, {}) - weight {}",
                i + 1,
                edge.source,
                edge.target,
                edge.weight
            );
        }
        println!(
            "\nRemoving these {} edge(s) would disconnect the network!",
            cut_edges.len()
        );
    }

    // Identify the partition
    if let Some((s, t)) = result.partition {
        println!("\nNetwork would split into:");
        println!("  • Component A: {} nodes {:?}", s.len(), s);
        println!("  • Component B: {} nodes {:?}", t.len(), t);
    }
}

/// Demo 6: Performance scaling analysis
fn demo_performance_scaling() {
    println!("📈 DEMO 6: Performance Scaling");
    println!("Measuring performance at different graph sizes...\n");

    let sizes = vec![10, 50, 100, 200];
    println!(
        "{:<10} {:<15} {:<15} {:<15}",
        "Vertices", "Edges", "Build Time", "Query Time"
    );
    println!("{}", "─".repeat(60));

    for n in sizes {
        // Create a random graph
        let mut rng = rand::thread_rng();
        let mut edges = Vec::new();

        // Create a path to ensure connectivity
        for i in 0..n - 1 {
            edges.push((i, i + 1, rng.gen_range(1.0..10.0)));
        }

        // Add random edges for density
        let num_random_edges = n / 2;
        for _ in 0..num_random_edges {
            let u = rng.gen_range(0..n);
            let v = rng.gen_range(0..n);
            if u != v {
                edges.push((u, v, rng.gen_range(1.0..10.0)));
            }
        }

        // Build and measure
        let start = Instant::now();
        let mincut = MinCutBuilder::new().exact().with_edges(edges).build();
        let build_time = start.elapsed();

        if let Ok(mincut) = mincut {
            let start = Instant::now();
            let _cut = mincut.min_cut_value();
            let query_time = start.elapsed();

            println!(
                "{:<10} {:<15} {:<15?} {:<15?}",
                n,
                mincut.num_edges(),
                build_time,
                query_time
            );
        }
    }

    println!("\n💡 Key observations:");
    println!("  • Query time is O(1) - constant regardless of size");
    println!("  • Build time grows subpolynomially: O(n^{{o(1)}})");
    println!("  • Update time (insert/delete) is also subpolynomial");

    // Demonstrate update performance
    println!("\nMeasuring update performance on n=100 graph...");
    let mut edges = Vec::new();
    for i in 0..99 {
        edges.push((i, i + 1, 1.0));
    }

    let mut mincut = MinCutBuilder::new()
        .exact()
        .with_edges(edges)
        .build()
        .expect("Build failed");

    // Measure insertions
    let start = Instant::now();
    for i in 0..10 {
        let _ = mincut.insert_edge(i, i + 50, 1.0);
    }
    let insert_time = start.elapsed();

    // Measure deletions
    let start = Instant::now();
    for i in 0..10 {
        let _ = mincut.delete_edge(i, i + 1);
    }
    let delete_time = start.elapsed();

    println!("\nUpdate performance (10 operations):");
    println!("  • Total insertion time: {:?}", insert_time);
    println!("  • Avg per insertion: {:?}", insert_time / 10);
    println!("  • Total deletion time: {:?}", delete_time);
    println!("  • Avg per deletion: {:?}", delete_time / 10);

    let stats = mincut.stats();
    println!("\nAggregate statistics:");
    println!("  • Total updates: {}", stats.insertions + stats.deletions);
    println!("  • Avg update time: {:.2} μs", stats.avg_update_time_us);
}

/// Demo 7: Vector-Graph Fusion
fn demo_vector_graph_fusion() {
    println!("🔗 DEMO 7: Vector-Graph Fusion");
    println!("Combining vector similarity with graph relations...\n");

    // Create fusion graph with custom config
    let config = FusionConfig {
        vector_weight: 0.6,
        graph_weight: 0.4,
        similarity_threshold: 0.5,
        top_k: 5,
        ..Default::default()
    };

    let mut fusion = FusionGraph::with_config(config);

    // Ingest document vectors (simulating embeddings)
    println!("Ingesting document vectors...");
    let docs = vec![
        (1, vec![1.0, 0.0, 0.0, 0.0]), // Topic A
        (2, vec![0.9, 0.1, 0.0, 0.0]), // Similar to Topic A
        (3, vec![0.8, 0.2, 0.0, 0.0]), // Similar to Topic A
        (4, vec![0.0, 1.0, 0.0, 0.0]), // Topic B
        (5, vec![0.0, 0.9, 0.1, 0.0]), // Similar to Topic B
        (6, vec![0.0, 0.0, 1.0, 0.0]), // Topic C (isolated)
    ];

    for (id, vec) in &docs {
        fusion.ingest_node_with_id(*id, vec.clone());
    }

    println!("  • Nodes: {}", fusion.num_nodes());
    println!("  • Edges from similarity: {}", fusion.num_edges());

    // Add explicit relations
    println!("\nAdding explicit graph relations...");
    fusion.add_relation(1, 4, RelationType::References, 0.8);
    fusion.add_relation(2, 5, RelationType::CoOccurs, 0.6);

    println!("  • Total edges: {}", fusion.num_edges());
    println!("  • Min cut estimate: {:.2}", fusion.min_cut());

    // Show fusion edge capacities
    println!("\nEdge capacity computation:");
    println!("  c(u,v) = w_v * f_v(similarity) + w_g * f_g(strength, type)");
    println!("  where w_v=0.6, w_g=0.4");

    for edge in fusion.get_edges().iter().take(5) {
        let origin = match edge.origin {
            fusion::EdgeOrigin::Vector => "Vector",
            fusion::EdgeOrigin::Graph => "Graph",
            fusion::EdgeOrigin::SelfLearn => "Learned",
        };
        println!(
            "  • ({}, {}) [{:>7}]: raw={:.2}, capacity={:.4}",
            edge.src, edge.dst, origin, edge.raw_strength, edge.capacity
        );
    }

    // Query with brittleness awareness
    println!("\nQuerying for Topic A documents...");
    let result = fusion.query(&[1.0, 0.0, 0.0, 0.0], 4);
    println!("  • Retrieved: {:?}", result.nodes);
    println!("  • Subgraph min-cut: {:.2}", result.min_cut);

    if let Some(warning) = result.brittleness_warning {
        println!("  • ⚠️  {}", warning);
    } else {
        println!("  • ✓ Good connectivity");
    }
}

/// Demo 8: Brittleness Detection
fn demo_brittleness_detection() {
    println!("🔍 DEMO 8: Brittleness Detection");
    println!("Monitoring graph health with structural analysis...\n");

    let mut monitor = StructuralMonitor::with_config(StructuralMonitorConfig {
        window_size: 10,
        lambda_low: 3.0,
        lambda_critical: 1.5,
        volatility_threshold: 0.5,
        trend_slope_threshold: -0.2,
    });

    // Simulate a series of graph states
    println!("Simulating graph evolution...\n");
    let observations = vec![
        (5.0, "Healthy: strong connectivity"),
        (4.5, "Slight decrease"),
        (4.2, "Continuing decline"),
        (3.8, "Below warning threshold"),
        (2.5, "Warning: low connectivity"),
        (1.8, "Approaching critical"),
        (1.0, "Critical: islanding risk!"),
    ];

    for (lambda, description) in observations {
        let triggers = monitor.observe(lambda, vec![(1, 2), (2, 3)]);
        let signal = monitor.signal();

        let signal_icon = match signal {
            BrittlenessSignal::Healthy => "🟢",
            BrittlenessSignal::Warning => "🟡",
            BrittlenessSignal::Critical => "🔴",
            BrittlenessSignal::Disconnected => "⚫",
        };

        println!(
            "{} λ={:.1}: {} [{}]",
            signal_icon,
            lambda,
            description,
            signal.as_str()
        );

        for trigger in triggers {
            println!(
                "   ⚡ TRIGGER: {:?} (severity: {:.0}%)",
                trigger.trigger_type,
                trigger.severity * 100.0
            );
            println!("      → {}", trigger.recommendation);
        }
    }

    // Show trend analysis
    println!("\nTrend Analysis:");
    let state = monitor.state();
    let trend_dir = if state.lambda_trend > 0.0 {
        "↑"
    } else {
        "↓"
    };
    println!(
        "  • Trend slope: {}{:.3} per observation",
        trend_dir,
        state.lambda_trend.abs()
    );
    println!("  • Volatility: {:.3}", state.cut_volatility);
    println!("  • Boundary edges: {}", state.boundary_edges.len());

    println!("\nMonitor Report:");
    println!("  {}", monitor.report());
}

/// Demo 9: Self-Learning Optimization
fn demo_self_learning_optimization() {
    println!("🧠 DEMO 9: Self-Learning Optimization");
    println!("Adaptive maintenance planning with learning gate...\n");

    let mut monitor = StructuralMonitor::new();
    let mut optimizer = Optimizer::new();

    // Simulate different graph health states
    let scenarios = vec![
        (5.0, "Healthy graph"),
        (2.5, "Degrading connectivity"),
        (0.8, "Critical brittleness"),
        (3.5, "Recovering"),
        (4.0, "Stable again"),
    ];

    println!("Scenario-based optimization:\n");

    for (lambda, description) in scenarios {
        println!("━━━ {} (λ={}) ━━━", description, lambda);

        // Update monitor
        monitor.observe(lambda, vec![(1, 2)]);

        // Get optimization result
        let result = optimizer.analyze(&monitor);

        println!(
            "Signal: {} | Learning rate: {:.4}",
            result.signal.as_str(),
            optimizer.learning_gate().learning_rate
        );

        // Show immediate action if any
        match &result.immediate_action {
            OptimizerAction::NoOp => {
                println!("Immediate action: None needed");
            }
            OptimizerAction::Rewire { strengthen, .. } => {
                println!(
                    "Immediate action: Rewire ({} edges to strengthen)",
                    strengthen.len()
                );
            }
            OptimizerAction::Reindex { new_threshold, .. } => {
                println!("Immediate action: Reindex (threshold: {:?})", new_threshold);
            }
            OptimizerAction::LearningGate {
                enable,
                rate_multiplier,
            } => {
                println!(
                    "Immediate action: {} learning (rate x{})",
                    if *enable { "Enable" } else { "Disable" },
                    rate_multiplier
                );
            }
            _ => {
                println!("Immediate action: {:?}", result.immediate_action);
            }
        }

        // Show maintenance plan
        if !result.plan.is_empty() {
            println!("Maintenance plan: {}", result.plan.summary);
            for task in result.plan.tasks.iter().take(2) {
                println!("  • [P{}] {}", task.priority, task.benefit);
            }
        }

        println!();
    }

    // Show metrics summary
    println!("Final Optimization Metrics:");
    if let Some(result) = optimizer.last_result() {
        for (key, value) in &result.metrics {
            println!("  • {}: {:.4}", key, value);
        }
    }

    println!("\nLearning Gate Status:");
    let gate = optimizer.learning_gate();
    println!("  • Enabled: {}", gate.enabled);
    println!("  • Current rate: {:.4}", gate.learning_rate);
    println!("  • Base rate: {:.4}", gate.base_rate);
}
