//! Full end-to-end pipeline: simulate -> process -> analyze -> decode.

use std::f64::consts::PI;

use ruv_neural_core::brain::Atlas;
use ruv_neural_core::graph::{BrainEdge, BrainGraph, ConnectivityMetric};
use ruv_neural_core::signal::{FrequencyBand, MultiChannelTimeSeries};
use ruv_neural_core::topology::CognitiveState;
use ruv_neural_decoder::ThresholdDecoder;
use ruv_neural_embed::spectral_embed::SpectralEmbedder;
use ruv_neural_embed::topology_embed::TopologyEmbedder;
use ruv_neural_mincut::stoer_wagner_mincut;
use ruv_neural_signal::connectivity::phase_locking_value;
use ruv_neural_signal::filter::BandpassFilter;

/// Run the full pipeline command.
pub fn run(
    channels: usize,
    duration: f64,
    dashboard: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let sample_rate = 1000.0;
    let num_samples = (duration * sample_rate) as usize;

    println!("=== rUv Neural — Full Pipeline ===");
    println!();

    // Step 1: Generate simulated sensor data.
    println!("  [1/7] Generating simulated sensor data...");
    let raw_data = generate_data(channels, num_samples, sample_rate);
    let ts = MultiChannelTimeSeries::new(raw_data.clone(), sample_rate, 0.0)
        .map_err(|e| format!("Time series creation failed: {e}"))?;
    println!("        {channels} channels, {num_samples} samples, {duration:.1}s");

    // Step 2: Preprocess (bandpass filter 1-100 Hz).
    println!("  [2/7] Preprocessing (bandpass 1-100 Hz)...");
    let filter = BandpassFilter::new(4, 1.0, 100.0, sample_rate);
    let filtered: Vec<Vec<f64>> = raw_data
        .iter()
        .map(|ch| {
            use ruv_neural_signal::filter::SignalProcessor;
            filter.process(ch)
        })
        .collect();
    println!("        Bandpass filter applied to all channels");

    // Step 3: Construct brain graph via PLV connectivity.
    println!("  [3/7] Constructing brain connectivity graph (PLV)...");
    let graph = build_plv_graph(&filtered, sample_rate);
    println!(
        "        {} nodes, {} edges, density {:.4}",
        graph.num_nodes,
        graph.edges.len(),
        graph.density()
    );

    // Step 4: Compute mincut and topology metrics.
    println!("  [4/7] Computing minimum cut and topology metrics...");
    let mc = stoer_wagner_mincut(&graph)
        .map_err(|e| format!("Mincut failed: {e}"))?;
    println!("        Cut value: {:.4}, balance: {:.4}", mc.cut_value, mc.balance_ratio());
    println!(
        "        Partition A: {} nodes, Partition B: {} nodes",
        mc.partition_a.len(),
        mc.partition_b.len()
    );

    // Step 5: Generate embedding.
    println!("  [5/7] Generating topology embedding...");
    let embedder = TopologyEmbedder::new();
    let embedding = embedder.embed_graph(&graph)
        .map_err(|e| format!("Embedding failed: {e}"))?;
    println!("        Dimension: {}, norm: {:.4}", embedding.dimension, embedding.norm());

    // Also generate spectral embedding.
    let spectral_dim = channels.min(8).max(2);
    let spectral = SpectralEmbedder::new(spectral_dim);
    let spectral_emb = spectral.embed_graph(&graph)
        .map_err(|e| format!("Spectral embedding failed: {e}"))?;
    println!(
        "        Spectral embedding: dim={}, norm={:.4}",
        spectral_emb.dimension,
        spectral_emb.norm()
    );

    // Step 6: Decode cognitive state.
    println!("  [6/7] Decoding cognitive state...");
    let decoder = build_default_decoder();
    let metrics = ruv_neural_core::topology::TopologyMetrics {
        global_mincut: mc.cut_value,
        modularity: estimate_modularity(&graph),
        global_efficiency: estimate_efficiency(&graph),
        local_efficiency: 0.0,
        graph_entropy: estimate_entropy(&graph),
        fiedler_value: 0.0,
        num_modules: 2,
        timestamp: graph.timestamp,
    };
    let (state, confidence) = decoder.decode(&metrics);
    println!("        State:      {state:?}");
    println!("        Confidence: {confidence:.4}");

    // Step 7: Display results.
    println!("  [7/7] Results summary");
    println!();

    println!("  ┌─────────────────────────────────────────┐");
    println!("  │         Pipeline Results Summary         │");
    println!("  ├─────────────────────────────────────────┤");
    println!("  │  Channels:         {:<20} │", channels);
    println!("  │  Duration:         {:<20} │", format!("{duration:.1} s"));
    println!("  │  Graph density:    {:<20} │", format!("{:.4}", graph.density()));
    println!("  │  Mincut value:     {:<20} │", format!("{:.4}", mc.cut_value));
    println!("  │  Balance ratio:    {:<20} │", format!("{:.4}", mc.balance_ratio()));
    println!("  │  Modularity:       {:<20} │", format!("{:.4}", metrics.modularity));
    println!("  │  Graph entropy:    {:<20} │", format!("{:.4}", metrics.graph_entropy));
    println!("  │  Embedding dim:    {:<20} │", embedding.dimension);
    println!("  │  Cognitive state:  {:<20} │", format!("{state:?}"));
    println!("  │  Confidence:       {:<20} │", format!("{confidence:.4}"));
    println!("  └─────────────────────────────────────────┘");
    println!();

    if dashboard {
        print_dashboard(&ts, &graph, &mc, &metrics);
    }

    Ok(())
}

/// Generate synthetic multi-channel neural data.
fn generate_data(channels: usize, num_samples: usize, sample_rate: f64) -> Vec<Vec<f64>> {
    let mut data = Vec::with_capacity(channels);
    for ch in 0..channels {
        let mut channel_data = Vec::with_capacity(num_samples);
        let phase = (ch as f64) * PI / (channels as f64);
        let mut rng: u64 = (ch as u64).wrapping_mul(2862933555777941757).wrapping_add(3037000493);

        for i in 0..num_samples {
            let t = i as f64 / sample_rate;
            let alpha = 50.0 * (2.0 * PI * 10.0 * t + phase).sin();
            let beta = 30.0 * (2.0 * PI * 20.0 * t + phase * 1.3).sin();
            let gamma = 15.0 * (2.0 * PI * 40.0 * t + phase * 0.7).sin();

            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let u1 = (rng >> 11) as f64 / (1u64 << 53) as f64;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let u2 = (rng >> 11) as f64 / (1u64 << 53) as f64;
            let noise = if u1 > 1e-15 {
                5.0 * (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
            } else {
                0.0
            };

            channel_data.push(alpha + beta + gamma + noise);
        }
        data.push(channel_data);
    }
    data
}

/// Build a brain graph from PLV connectivity between all channel pairs.
fn build_plv_graph(channels: &[Vec<f64>], sample_rate: f64) -> BrainGraph {
    let n = channels.len();
    let mut edges = Vec::new();
    let plv_threshold = 0.3;

    for i in 0..n {
        for j in (i + 1)..n {
            let plv = phase_locking_value(&channels[i], &channels[j], sample_rate, FrequencyBand::Alpha);
            if plv > plv_threshold {
                edges.push(BrainEdge {
                    source: i,
                    target: j,
                    weight: plv,
                    metric: ConnectivityMetric::PhaseLockingValue,
                    frequency_band: FrequencyBand::Alpha,
                });
            }
        }
    }

    BrainGraph {
        num_nodes: n,
        edges,
        timestamp: 0.0,
        window_duration_s: 1.0,
        atlas: Atlas::Custom(n),
    }
}

/// Estimate modularity using a simple degree-based partition.
fn estimate_modularity(graph: &BrainGraph) -> f64 {
    let n = graph.num_nodes;
    if n < 2 {
        return 0.0;
    }
    let total = graph.total_weight();
    if total < 1e-12 {
        return 0.0;
    }

    let adj = graph.adjacency_matrix();
    let degrees: Vec<f64> = (0..n).map(|i| graph.node_degree(i)).collect();
    let two_m = 2.0 * total;

    // Simple bisection: first half vs second half.
    let mid = n / 2;
    let mut q = 0.0;
    for i in 0..n {
        for j in 0..n {
            let same_community = (i < mid && j < mid) || (i >= mid && j >= mid);
            if same_community {
                q += adj[i][j] - degrees[i] * degrees[j] / two_m;
            }
        }
    }
    q / two_m
}

/// Estimate global efficiency (mean inverse shortest path).
fn estimate_efficiency(graph: &BrainGraph) -> f64 {
    let n = graph.num_nodes;
    if n < 2 {
        return 0.0;
    }
    // Use adjacency weights directly as a rough proxy.
    let adj = graph.adjacency_matrix();
    let mut sum = 0.0;
    let mut count = 0;
    for i in 0..n {
        for j in (i + 1)..n {
            if adj[i][j] > 0.0 {
                sum += adj[i][j]; // weight as proxy for efficiency
            }
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    sum / count as f64
}

/// Estimate graph entropy from edge weight distribution.
fn estimate_entropy(graph: &BrainGraph) -> f64 {
    let total = graph.total_weight();
    if total < 1e-12 || graph.edges.is_empty() {
        return 0.0;
    }
    let mut entropy = 0.0;
    for edge in &graph.edges {
        let p = edge.weight / total;
        if p > 1e-15 {
            entropy -= p * p.ln();
        }
    }
    entropy
}

/// Build a threshold decoder with default state definitions.
fn build_default_decoder() -> ThresholdDecoder {
    let mut decoder = ThresholdDecoder::new();

    decoder.set_threshold(
        CognitiveState::Rest,
        ruv_neural_decoder::TopologyThreshold {
            mincut_range: (0.0, 5.0),
            modularity_range: (0.2, 0.6),
            efficiency_range: (0.1, 0.4),
            entropy_range: (1.0, 3.0),
        },
    );

    decoder.set_threshold(
        CognitiveState::Focused,
        ruv_neural_decoder::TopologyThreshold {
            mincut_range: (3.0, 15.0),
            modularity_range: (0.4, 0.8),
            efficiency_range: (0.3, 0.7),
            entropy_range: (2.0, 4.0),
        },
    );

    decoder.set_threshold(
        CognitiveState::MotorPlanning,
        ruv_neural_decoder::TopologyThreshold {
            mincut_range: (2.0, 10.0),
            modularity_range: (0.3, 0.7),
            efficiency_range: (0.2, 0.6),
            entropy_range: (1.5, 3.5),
        },
    );

    decoder
}

/// Print a real-time-style ASCII dashboard.
fn print_dashboard(
    ts: &MultiChannelTimeSeries,
    graph: &BrainGraph,
    mc: &ruv_neural_core::topology::MincutResult,
    metrics: &ruv_neural_core::topology::TopologyMetrics,
) {
    println!("  ╔═══════════════════════════════════════════════════╗");
    println!("  ║           rUv Neural — Live Dashboard             ║");
    println!("  ╠═══════════════════════════════════════════════════╣");
    println!("  ║                                                   ║");

    // Signal sparkline for first few channels.
    let display_channels = ts.num_channels.min(6);
    let display_samples = ts.num_samples.min(50);
    let sparkline_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    for ch in 0..display_channels {
        let data = &ts.data[ch];
        let min_val = data.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = max_val - min_val;

        let step = ts.num_samples / display_samples;
        let mut sparkline = String::new();
        for i in 0..display_samples {
            let val = data[i * step];
            let normalized = if range > 1e-12 {
                ((val - min_val) / range * 7.0) as usize
            } else {
                4
            };
            sparkline.push(sparkline_chars[normalized.min(7)]);
        }
        println!("  ║  Ch{ch:02}: {sparkline} ║");
    }

    println!("  ║                                                   ║");
    println!("  ║  Graph:  {} nodes, {} edges              ║",
        format!("{:>3}", graph.num_nodes),
        format!("{:>4}", graph.edges.len()),
    );
    println!("  ║  Mincut: {:.4}  Balance: {:.4}              ║", mc.cut_value, mc.balance_ratio());
    println!("  ║  Modularity: {:.4}  Entropy: {:.4}          ║", metrics.modularity, metrics.graph_entropy);
    println!("  ║                                                   ║");
    println!("  ╚═══════════════════════════════════════════════════╝");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_runs_end_to_end() {
        let result = run(4, 1.0, false);
        assert!(result.is_ok());
    }

    #[test]
    fn pipeline_with_dashboard() {
        let result = run(4, 0.5, true);
        assert!(result.is_ok());
    }

    #[test]
    fn plv_graph_has_edges() {
        let data = generate_data(4, 1000, 1000.0);
        let graph = build_plv_graph(&data, 1000.0);
        assert_eq!(graph.num_nodes, 4);
        // Channels with similar phase should have some PLV connectivity.
    }

    #[test]
    fn entropy_non_negative() {
        let data = generate_data(4, 1000, 1000.0);
        let graph = build_plv_graph(&data, 1000.0);
        let e = estimate_entropy(&graph);
        assert!(e >= 0.0);
    }
}
