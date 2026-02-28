# sevensense-analysis

[![Crate](https://img.shields.io/badge/crates.io-sevensense--analysis-orange.svg)](https://crates.io/crates/sevensense-analysis)
[![Docs](https://img.shields.io/badge/docs-sevensense--analysis-blue.svg)](https://docs.rs/sevensense-analysis)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

> Advanced acoustic analysis algorithms for bioacoustic pattern discovery.

**sevensense-analysis** provides sophisticated analysis tools for understanding bird vocalizations at scale. From clustering calls into groups, detecting recurring motifs, to modeling temporal patterns with Markov chains, it transforms raw embeddings into actionable ecological insights.

## Features

- **HDBSCAN Clustering**: Density-based clustering for call-type discovery
- **Markov Models**: Temporal sequence analysis and prediction
- **Motif Detection**: Find recurring vocal patterns
- **Statistical Analysis**: Entropy, diversity indices, anomaly scores
- **Temporal Patterns**: Diel rhythms, seasonal trends
- **Multi-scale Analysis**: From milliseconds to months

## Use Cases

| Use Case | Description | Key Functions |
|----------|-------------|---------------|
| Call-Type Clustering | Group similar vocalizations | `hdbscan_cluster()` |
| Sequence Analysis | Model call sequences | `MarkovChain::analyze()` |
| Motif Discovery | Find repeated patterns | `detect_motifs()` |
| Diversity Metrics | Shannon/Simpson indices | `diversity_index()` |
| Periodicity | Detect rhythmic patterns | `detect_periodicity()` |
| Anomaly Detection | Find unusual calls | `anomaly_score()` |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sevensense-analysis = "0.1"
```

## Quick Start

```rust
use sevensense_analysis::{HdbscanClusterer, HdbscanConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Cluster embeddings by call type
    let config = HdbscanConfig {
        min_cluster_size: 5,
        min_samples: 3,
        ..Default::default()
    };

    let clusterer = HdbscanClusterer::new(config);
    let labels = clusterer.fit(&embeddings)?;

    // Count clusters (excluding noise = -1)
    let n_clusters = labels.iter().filter(|&&l| l >= 0).max().unwrap_or(&-1) + 1;
    println!("Found {} call types", n_clusters);

    Ok(())
}
```

---

<details>
<summary><b>Tutorial: HDBSCAN Clustering</b></summary>

### Basic Clustering

```rust
use sevensense_analysis::{HdbscanClusterer, HdbscanConfig};

let config = HdbscanConfig {
    min_cluster_size: 5,      // Minimum points per cluster
    min_samples: 3,           // Core point threshold
    epsilon: 0.0,             // 0 = automatic selection
    metric: DistanceMetric::Euclidean,
};

let clusterer = HdbscanClusterer::new(config);
let result = clusterer.fit(&embeddings)?;

println!("Labels: {:?}", result.labels);
println!("Probabilities: {:?}", result.probabilities);
println!("Outlier scores: {:?}", result.outlier_scores);
```

### Cluster Analysis

```rust
use sevensense_analysis::{cluster_statistics, ClusterStats};

let stats = cluster_statistics(&embeddings, &labels)?;

for (cluster_id, stat) in stats.iter() {
    println!("Cluster {}:", cluster_id);
    println!("  Size: {}", stat.size);
    println!("  Centroid: {:?}", &stat.centroid[..5]);  // First 5 dims
    println!("  Intra-cluster distance: {:.3}", stat.intra_distance);
    println!("  Silhouette score: {:.3}", stat.silhouette);
}
```

### Cluster Assignment for New Data

```rust
// Assign new embeddings to existing clusters
let new_embeddings = load_new_data()?;
let assignments = clusterer.predict(&new_embeddings)?;

for (embedding, cluster) in new_embeddings.iter().zip(assignments.iter()) {
    if *cluster >= 0 {
        println!("Assigned to cluster {}", cluster);
    } else {
        println!("Classified as noise/outlier");
    }
}
```

</details>

<details>
<summary><b>Tutorial: Markov Chain Analysis</b></summary>

### Building a Markov Model

```rust
use sevensense_analysis::{MarkovChain, MarkovConfig};

// Sequences of cluster labels (call types)
let sequences: Vec<Vec<i32>> = vec![
    vec![0, 1, 2, 0, 1],  // Sequence 1
    vec![0, 1, 0, 2, 1],  // Sequence 2
    vec![1, 2, 0, 1, 2],  // Sequence 3
];

let config = MarkovConfig {
    order: 1,              // First-order Markov chain
    smoothing: 0.01,       // Laplace smoothing
};

let chain = MarkovChain::fit(&sequences, config)?;

// Get transition probabilities
let probs = chain.transition_matrix();
println!("P(1|0) = {:.3}", probs[(0, 1)]);  // Probability of 1 given 0
```

### Sequence Prediction

```rust
// Predict next state
let current_state = 0;
let next_probs = chain.predict_next(current_state)?;

println!("Next state probabilities from state {}:", current_state);
for (state, prob) in next_probs.iter().enumerate() {
    println!("  State {}: {:.3}", state, prob);
}

// Generate synthetic sequence
let generated = chain.generate(10, Some(0))?;  // 10 states, starting from 0
println!("Generated sequence: {:?}", generated);
```

### Sequence Analysis

```rust
use sevensense_analysis::MarkovAnalysis;

let analysis = MarkovAnalysis::new(&chain);

// Stationary distribution
let stationary = analysis.stationary_distribution()?;
println!("Stationary distribution: {:?}", stationary);

// Entropy rate
let entropy = analysis.entropy_rate()?;
println!("Entropy rate: {:.3} bits", entropy);

// Expected hitting times
let hitting_times = analysis.mean_hitting_times()?;
println!("Mean hitting time 0→2: {:.2} steps", hitting_times[(0, 2)]);
```

</details>

<details>
<summary><b>Tutorial: Motif Detection</b></summary>

### Finding Repeated Patterns

```rust
use sevensense_analysis::{MotifDetector, MotifConfig};

let config = MotifConfig {
    min_length: 3,           // Minimum motif length
    max_length: 10,          // Maximum motif length
    similarity_threshold: 0.85,
    min_occurrences: 2,
};

let detector = MotifDetector::new(config);
let motifs = detector.detect(&embeddings)?;

for motif in &motifs {
    println!("Motif found:");
    println!("  Length: {} segments", motif.length);
    println!("  Occurrences: {}", motif.occurrences.len());
    println!("  Positions: {:?}", motif.positions());
    println!("  Average similarity: {:.3}", motif.avg_similarity);
}
```

### Motif Visualization

```rust
use sevensense_analysis::motif_to_sequence;

for motif in motifs.iter().take(5) {
    // Get the representative sequence
    let sequence = motif_to_sequence(&embeddings, motif)?;

    println!("Motif #{} (len={})", motif.id, motif.length);
    println!("  Representative: {:?}", sequence);

    // Show all occurrences
    for (i, occ) in motif.occurrences.iter().enumerate() {
        println!("  Occurrence {}: positions {}-{}",
            i, occ.start, occ.end);
    }
}
```

### Cross-Recording Motifs

```rust
// Find motifs that appear across multiple recordings
let recordings: Vec<(RecordingId, Vec<Embedding>)> = load_recordings()?;

let cross_motifs = detector.detect_cross_recording(&recordings)?;

for motif in cross_motifs {
    println!("Cross-recording motif:");
    println!("  Appears in {} recordings", motif.recording_ids.len());
    println!("  Total occurrences: {}", motif.total_occurrences);
}
```

</details>

<details>
<summary><b>Tutorial: Statistical Analysis</b></summary>

### Diversity Indices

```rust
use sevensense_analysis::{shannon_index, simpson_index, species_richness};

// Count species occurrences
let species_counts = count_species(&labels)?;

let shannon = shannon_index(&species_counts);
let simpson = simpson_index(&species_counts);
let richness = species_richness(&species_counts);

println!("Shannon Index (H'): {:.3}", shannon);
println!("Simpson Index (D): {:.3}", simpson);
println!("Species Richness: {}", richness);
```

### Entropy Analysis

```rust
use sevensense_analysis::{sequence_entropy, normalized_entropy};

// Entropy of call sequences
let sequence: Vec<i32> = vec![0, 1, 2, 0, 1, 0, 2, 1, 0];

let entropy = sequence_entropy(&sequence);
let norm_entropy = normalized_entropy(&sequence);

println!("Sequence entropy: {:.3} bits", entropy);
println!("Normalized entropy: {:.3}", norm_entropy);  // 0-1 scale
```

### Periodicity Detection

```rust
use sevensense_analysis::{detect_periodicity, PeriodicityConfig};

let config = PeriodicityConfig {
    min_period: 2,
    max_period: 100,
    confidence_threshold: 0.7,
};

let timestamps: Vec<f64> = get_call_timestamps()?;
let periods = detect_periodicity(&timestamps, config)?;

for (period, confidence) in periods {
    println!("Period: {:.1}s (confidence: {:.2})", period, confidence);
}
```

</details>

<details>
<summary><b>Tutorial: Temporal Analysis</b></summary>

### Diel Activity Patterns

```rust
use sevensense_analysis::{DielAnalyzer, TimeOfDay};

let analyzer = DielAnalyzer::new();

// Analyze activity by time of day
let pattern = analyzer.analyze(&timestamps)?;

println!("Dawn chorus: {} calls", pattern.count(TimeOfDay::Dawn));
println!("Morning: {} calls", pattern.count(TimeOfDay::Morning));
println!("Midday: {} calls", pattern.count(TimeOfDay::Midday));
println!("Evening: {} calls", pattern.count(TimeOfDay::Evening));
println!("Night: {} calls", pattern.count(TimeOfDay::Night));

// Peak activity time
let peak = pattern.peak_hour();
println!("Peak activity: {:02}:00", peak);
```

### Seasonal Trends

```rust
use sevensense_analysis::{SeasonalAnalyzer, Season};

let analyzer = SeasonalAnalyzer::new();
let trend = analyzer.analyze(&dated_records)?;

println!("Spring activity: {:.1}%", trend.percentage(Season::Spring));
println!("Breeding season peak: {:?}", trend.breeding_peak());
println!("Migration periods: {:?}", trend.migration_windows());
```

### Time Series Analysis

```rust
use sevensense_analysis::{TimeSeries, Aggregation};

let series = TimeSeries::from_events(&events)?;

// Aggregate by hour
let hourly = series.aggregate(Aggregation::Hourly)?;

// Detect anomalies
let anomalies = series.detect_anomalies(3.0)?;  // 3-sigma threshold

for anomaly in anomalies {
    println!("Anomaly at {}: {} calls (expected: {})",
        anomaly.timestamp, anomaly.actual, anomaly.expected);
}
```

</details>

---

## Configuration

### HdbscanConfig Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `min_cluster_size` | 5 | Minimum cluster size |
| `min_samples` | 3 | Core point threshold |
| `epsilon` | 0.0 | Distance threshold (0=auto) |
| `metric` | Euclidean | Distance metric |

### MarkovConfig Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `order` | 1 | Markov chain order |
| `smoothing` | 0.01 | Laplace smoothing factor |

## Algorithms

| Algorithm | Complexity | Use Case |
|-----------|------------|----------|
| HDBSCAN | O(n log n) | Clustering with noise |
| Markov Chain | O(n × s²) | Sequence modeling |
| Motif Discovery | O(n² × m) | Pattern finding |
| FFT Periodicity | O(n log n) | Rhythm detection |

## Links

- **Homepage**: [ruv.io](https://ruv.io)
- **Repository**: [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **Crates.io**: [crates.io/crates/sevensense-analysis](https://crates.io/crates/sevensense-analysis)
- **Documentation**: [docs.rs/sevensense-analysis](https://docs.rs/sevensense-analysis)

## License

MIT License - see [LICENSE](../../LICENSE) for details.

---

*Part of the [7sense Bioacoustic Intelligence Platform](https://ruv.io) by rUv*
