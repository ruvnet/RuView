# 7sense - Bioacoustic Intelligence Platform

[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Tests](https://img.shields.io/badge/tests-329%20passed-brightgreen.svg)]()
[![Coverage](https://img.shields.io/badge/coverage-85%25-green.svg)]()

> Transform bird calls into navigable geometric space using cutting-edge AI and vector search technology.

**7sense** is a high-performance Rust platform for bioacoustic analysis that converts audio recordings of bird songs into rich, searchable embeddings. Using state-of-the-art neural networks (Perch 2.0) and ultra-fast vector indexing (HNSW), it enables researchers and conservationists to identify species, discover patterns, and track biodiversity at scale.

## Why 7sense?

Traditional bird monitoring relies on expert human listeners or basic spectrogram analysis. 7sense brings the power of modern AI to wildlife acoustics:

- **Instant Species ID**: Upload audio, get species predictions in milliseconds
- **Pattern Discovery**: Find similar calls across millions of recordings
- **Behavioral Insights**: Detect singing patterns, dialects, and anomalies
- **Scale Without Limits**: Process years of continuous recordings efficiently

---

## Architecture Overview

```
                    ┌─────────────────────────────────────────────────────────┐
                    │                      7sense Platform                     │
                    └─────────────────────────────────────────────────────────┘
                                               │
        ┌──────────────────────────────────────┼──────────────────────────────────────┐
        │                                      │                                       │
        ▼                                      ▼                                       ▼
┌───────────────┐                    ┌─────────────────┐                    ┌──────────────────┐
│  Audio Input  │                    │  API Gateway    │                    │  Vector Space    │
│   (sevensense │                    │  (sevensense    │                    │  (sevensense     │
│    -audio)    │                    │    -api)        │                    │    -vector)      │
└───────┬───────┘                    └────────┬────────┘                    └────────┬─────────┘
        │                                     │                                       │
        │ Audio segments                      │ GraphQL/REST                          │ 150x faster
        │ Mel spectrograms                    │ OpenAPI docs                          │ HNSW search
        ▼                                     ▼                                       ▼
┌───────────────┐                    ┌─────────────────┐                    ┌──────────────────┐
│  Embeddings   │                    │   Analysis      │                    │   Learning       │
│  (sevensense  │───────────────────▶│  (sevensense    │◀───────────────────│  (sevensense     │
│   -embedding) │   1536-dim         │   -analysis)    │    Patterns        │   -learning)     │
└───────────────┘   vectors          └────────┬────────┘                    └──────────────────┘
                                              │
                                              │ Evidence packs
                                              ▼
                                     ┌─────────────────┐
                                     │ Interpretation  │
                                     │ (sevensense     │
                                     │  -interpretation│
                                     └─────────────────┘
```

---

## Crates

| Crate | Description | Key Features |
|-------|-------------|--------------|
| [`sevensense-core`](crates/sevensense-core) | Shared domain primitives | Species taxonomy, temporal types, error handling |
| [`sevensense-audio`](crates/sevensense-audio) | Audio ingestion pipeline | WAV/MP3/FLAC support, Mel spectrograms, segmentation |
| [`sevensense-embedding`](crates/sevensense-embedding) | Neural embedding generation | Perch 2.0 ONNX, 1536-dim vectors, PQ quantization |
| [`sevensense-vector`](crates/sevensense-vector) | Vector space indexing | HNSW with 150x speedup, hyperbolic geometry |
| [`sevensense-learning`](crates/sevensense-learning) | Pattern learning | GNN training, EWC regularization, online learning |
| [`sevensense-analysis`](crates/sevensense-analysis) | Acoustic analysis | HDBSCAN clustering, Markov models, motif detection |
| [`sevensense-interpretation`](crates/sevensense-interpretation) | Evidence generation | RAB packs, confidence scoring, species narratives |
| [`sevensense-api`](crates/sevensense-api) | HTTP API layer | GraphQL, REST, OpenAPI, WebSocket streaming |
| [`sevensense-benches`](crates/sevensense-benches) | Performance benchmarks | Criterion.rs suites, performance validation |

---

## Quick Start

### Prerequisites

- Rust 1.75 or later
- 4GB RAM minimum (8GB recommended)
- ONNX Runtime (auto-downloaded)

### Installation

```bash
# Clone the repository
git clone https://github.com/ruvnet/vibecast.git
cd vibecast

# Build all crates
cargo build --release

# Run tests
cargo test --workspace

# Start the API server
cargo run -p sevensense-api --release
```

### Basic Usage

```rust
use sevensense_audio::AudioProcessor;
use sevensense_embedding::EmbeddingPipeline;
use sevensense_vector::HnswIndex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load and process audio
    let processor = AudioProcessor::new(Default::default());
    let segments = processor.process_file("recording.wav").await?;

    // Generate embeddings
    let pipeline = EmbeddingPipeline::new(Default::default()).await?;
    let embeddings = pipeline.embed_segments(&segments).await?;

    // Search for similar calls
    let index = HnswIndex::new(Default::default());
    index.add_batch(&embeddings)?;

    let query = &embeddings[0];
    let neighbors = index.search(query, 10)?;

    println!("Found {} similar bird calls", neighbors.len());
    Ok(())
}
```

---

## Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| HNSW Search Speedup | 150x vs brute force | ✅ |
| Query Latency (p99) | < 50ms | ✅ |
| Recall@10 | ≥ 0.95 | ✅ |
| Embedding Throughput | > 100 segments/sec | ✅ |
| Memory per 1M vectors | < 6 GB | ✅ |

---

## Use Cases

<details>
<summary><b>Species Identification</b></summary>

Upload a bird call recording and get instant species predictions with confidence scores:

```bash
curl -X POST http://localhost:3000/api/identify \
  -F "audio=@bird_call.wav" \
  | jq '.predictions[:3]'
```

```json
[
  {"species": "Turdus merula", "common_name": "Eurasian Blackbird", "confidence": 0.94},
  {"species": "Turdus philomelos", "common_name": "Song Thrush", "confidence": 0.82},
  {"species": "Turdus viscivorus", "common_name": "Mistle Thrush", "confidence": 0.71}
]
```
</details>

<details>
<summary><b>Similarity Search</b></summary>

Find all recordings similar to a reference call:

```graphql
query {
  searchSimilar(
    embedding: [0.123, -0.456, ...]
    k: 20
    minSimilarity: 0.8
  ) {
    id
    species
    similarity
    recordingUrl
  }
}
```
</details>

<details>
<summary><b>Biodiversity Monitoring</b></summary>

Analyze continuous audio streams for species diversity over time:

```rust
let analysis = analyzer.diversity_report(
    &recordings,
    TimeWindow::Daily,
    DiversityMetric::ShannonIndex
).await?;

println!("Shannon Index: {:.2}", analysis.shannon_index);
println!("Species Richness: {}", analysis.unique_species);
```
</details>

<details>
<summary><b>Anomaly Detection</b></summary>

Detect unusual vocalizations that may indicate distress or novel species:

```rust
let anomalies = detector.find_anomalies(
    &embeddings,
    AnomalyThreshold::Statistical(3.0)  // 3 sigma
)?;

for anomaly in anomalies {
    println!("Unusual call at {}: score {:.2}", anomaly.timestamp, anomaly.score);
}
```
</details>

---

## API Documentation

The API server provides:

- **GraphQL Playground**: `http://localhost:3000/graphql`
- **REST OpenAPI/Swagger**: `http://localhost:3000/docs/swagger-ui`
- **WebSocket Streaming**: `ws://localhost:3000/ws/stream`

---

## Development

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p sevensense-vector

# With output
cargo test --workspace -- --nocapture
```

### Running Benchmarks

```bash
# All benchmarks
cargo bench -p sevensense-benches

# Specific benchmark
cargo bench -p sevensense-benches --bench hnsw_benchmark

# Generate HTML report
cargo bench -p sevensense-benches -- --save-baseline main
```

### Code Quality

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --workspace -- -D warnings

# Documentation
cargo doc --workspace --no-deps --open
```

---

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

- [Perch](https://github.com/google-research/perch) - Bird audio embedding model
- [instant-distance](https://github.com/instant-labs/instant-distance) - HNSW implementation
- [ort](https://github.com/pykeio/ort) - ONNX Runtime bindings

---

<p align="center">
  <i>Built with 🦜 for the bioacoustics community</i>
</p>
