# Real Data Discovery Example

This example demonstrates RuVector's discovery engine on **real academic research papers** fetched from the OpenAlex API.

## What It Does

Fetches actual climate-finance research papers across multiple topics:
- **Climate risk finance** (20 papers)
- **Stranded assets** (15 papers)
- **Carbon pricing markets** (15 papers)
- **Physical climate risk** (15 papers)
- **Transition risk disclosure** (15 papers)

Then runs RuVector's discovery engine to detect:
- Cross-topic bridges (papers connecting different research areas)
- Emerging research clusters
- Consolidation/fragmentation trends
- Anomalous coherence patterns

## Running the Example

```bash
cd examples/data/framework
cargo run --example real_data_discovery
```

## Example Output

```
╔══════════════════════════════════════════════════════════════╗
║     Real Climate-Finance Research Discovery with OpenAlex    ║
║              Powered by RuVector Discovery Engine            ║
╚══════════════════════════════════════════════════════════════╝

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📡 Phase 1: Fetching Research Papers from OpenAlex API

   Querying topics:
   • climate risk finance: fetching 20 papers... ✓ 20 papers
   • stranded assets energy: fetching 15 papers... ✓ 15 papers
   • carbon pricing markets: fetching 15 papers... ✓ 15 papers
   ...

   Total papers fetched: 80
```

## Features

### Real API Integration
- Uses OpenAlex's public API (no authentication required)
- Polite API usage with rate limiting
- Graceful fallback to synthetic data if API fails

### Semantic Analysis
- Simple bag-of-words embeddings (128-dim)
- Converts paper titles + abstracts to vectors
- Preserves citation and concept relationships

### Discovery Engine
- **Graph construction**: Builds semantic graph from paper embeddings
- **Coherence computation**: Dynamic minimum cut algorithm
- **Pattern detection**: Multi-signal trend analysis
  - Cross-topic bridges
  - Emerging clusters
  - Research consolidation/fragmentation
  - Anomaly detection

### Performance
- Processes ~8 papers/second
- Handles 50-100 papers comfortably
- Scalable to larger datasets with optimized backend

## API Rate Limits

OpenAlex allows polite API usage without authentication:
- ~10 requests/second with polite headers
- Built-in retry logic for rate limit errors
- Automatic fallback if API unavailable

To be extra polite, the client includes an email in requests (configurable in the code).

## Customization

### Fetch Different Topics

Edit the `queries` vector in `main()`:

```rust
let queries = vec![
    ("topic_id", "your search query", 20),  // 20 papers
    ("another_topic", "another query", 15), // 15 papers
];
```

### Adjust Discovery Thresholds

Modify the `DiscoveryConfig`:

```rust
let discovery_config = DiscoveryConfig {
    min_signal_strength: 0.01,    // Lower = more patterns
    emergence_threshold: 0.15,     // Cluster growth threshold
    bridge_threshold: 0.25,        // Cross-topic connection threshold
    anomaly_sigma: 2.0,            // Anomaly sensitivity
    ..Default::default()
};
```

### Change Coherence Settings

Adjust the `CoherenceConfig`:

```rust
let coherence_config = CoherenceConfig {
    min_edge_weight: 0.3,          // Similarity threshold
    window_size_secs: 86400 * 365, // Time window (1 year)
    approximate: true,             // Use fast approximate min-cut
    ..Default::default()
};
```

## Understanding Results

### Cross-Topic Bridges
Papers that connect different research areas. High bridge frequency indicates interdisciplinary research.

```
🌉 Cross-Topic Bridges: 3
   1. Climate risk papers bridging to finance literature
      Confidence: 0.85
      Entities: 12 papers
```

### Emerging Clusters
New research areas forming over time. Indicates novel directions.

```
🌱 Emerging Research Clusters: 2
   1. Emerging structure detected: 5 new nodes over 3 windows
      Strength: Moderate
```

### Consolidation/Fragmentation
Shows whether topics are converging or diverging.

```
📈 Consolidating Topics: 1
   • Strengthening trend detected: 3.2% per window
```

## Extending the Example

### Use Advanced Embeddings

Replace `SimpleEmbedder` with a real embedding model:

```rust
// Instead of SimpleEmbedder
use sentence_transformers::SentenceTransformer;

let model = SentenceTransformer::load("all-MiniLM-L6-v2")?;
let embedding = model.encode(&text)?;
```

### Integrate with RuVector Core

Use `ruvector-core` for production vector search:

```rust
use ruvector_core::HnswIndex;

let mut index = HnswIndex::new(128)?;
for record in &records {
    if let Some(embedding) = &record.embedding {
        index.insert(&record.id, embedding)?;
    }
}
```

### Export Results

Save discoveries to JSON:

```rust
use std::fs::File;
use std::io::Write;

let json = serde_json::to_string_pretty(&patterns)?;
let mut file = File::create("discoveries.json")?;
file.write_all(json.as_bytes())?;
```

## Troubleshooting

### API Errors
If you see frequent API errors:
1. Check your internet connection
2. The example will automatically fall back to synthetic data
3. For large queries, add delays between requests

### No Patterns Detected
This is normal with small datasets! Try:
1. Fetching more papers (increase limits)
2. Lowering thresholds in `DiscoveryConfig`
3. Fetching more diverse topics to find bridges

### Out of Memory
For large datasets:
1. Reduce the number of papers fetched
2. Use the `approximate` coherence engine
3. Process in batches

## Learn More

- OpenAlex API: https://docs.openalex.org
- RuVector Discovery: `/examples/data/framework/`
- Min-cut algorithms: `/crates/ruvector-cluster/`
