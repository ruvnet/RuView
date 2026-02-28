# RuVector Streaming Data Ingestion

Real-time streaming data ingestion with windowed analysis, pattern detection, and backpressure handling.

## Features

- **Async Stream Processing**: Non-blocking ingestion of continuous data streams
- **Windowed Analysis**: Support for tumbling and sliding time windows
- **Real-time Pattern Detection**: Automatic pattern detection with customizable callbacks
- **Backpressure Handling**: Automatic flow control to prevent memory overflow
- **Comprehensive Metrics**: Throughput, latency, and pattern detection statistics
- **SIMD Acceleration**: Leverages optimized vector operations for high performance
- **Parallel Processing**: Configurable concurrency for batch operations

## Quick Start

```rust
use ruvector_data_framework::{
    StreamingEngine, StreamingEngineBuilder,
    ruvector_native::{Domain, SemanticVector},
};
use futures::stream;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create streaming engine with builder pattern
    let mut engine = StreamingEngineBuilder::new()
        .window_size(Duration::from_secs(60))
        .slide_interval(Duration::from_secs(30))
        .batch_size(100)
        .max_buffer_size(10000)
        .build();

    // Set pattern detection callback
    engine.set_pattern_callback(|pattern| {
        println!("Pattern detected: {:?}", pattern.pattern.pattern_type);
        println!("Confidence: {:.2}", pattern.pattern.confidence);
    }).await;

    // Create a stream of vectors
    let vectors = vec![/* your SemanticVector instances */];
    let vector_stream = stream::iter(vectors);

    // Ingest the stream
    engine.ingest_stream(vector_stream).await?;

    // Get metrics
    let metrics = engine.metrics().await;
    println!("Processed: {} vectors", metrics.vectors_processed);
    println!("Patterns detected: {}", metrics.patterns_detected);
    println!("Throughput: {:.1} vectors/sec", metrics.throughput_per_sec);

    Ok(())
}
```

## Window Types

### Sliding Windows

Overlapping time windows that provide continuous analysis:

```rust
let engine = StreamingEngineBuilder::new()
    .window_size(Duration::from_secs(60))      // 60-second windows
    .slide_interval(Duration::from_secs(30))   // Slide every 30 seconds
    .build();
```

**Use case**: Continuous monitoring with overlapping context

### Tumbling Windows

Non-overlapping time windows for discrete analysis:

```rust
let engine = StreamingEngineBuilder::new()
    .window_size(Duration::from_secs(60))
    .tumbling_windows()                        // No overlap
    .build();
```

**Use case**: Batch processing with clear boundaries

## Configuration

### StreamingConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `window_size` | `Duration` | 60s | Time window size |
| `slide_interval` | `Option<Duration>` | Some(30s) | Sliding window interval (None = tumbling) |
| `max_buffer_size` | `usize` | 10,000 | Max vectors before backpressure |
| `batch_size` | `usize` | 100 | Vectors per batch |
| `max_concurrency` | `usize` | 4 | Max parallel processing tasks |
| `auto_detect_patterns` | `bool` | true | Enable automatic pattern detection |
| `detection_interval` | `usize` | 100 | Detect patterns every N vectors |

### OptimizedConfig (Discovery)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `similarity_threshold` | `f64` | 0.65 | Min cosine similarity for edges |
| `mincut_sensitivity` | `f64` | 0.12 | Min-cut change threshold |
| `cross_domain` | `bool` | true | Enable cross-domain pattern detection |
| `use_simd` | `bool` | true | Use SIMD acceleration |
| `significance_threshold` | `f64` | 0.05 | P-value threshold for significance |

## Pattern Detection

The streaming engine automatically detects patterns using statistical significance testing:

```rust
engine.set_pattern_callback(|pattern| {
    match pattern.pattern.pattern_type {
        PatternType::CoherenceBreak => {
            println!("Network fragmentation detected!");
        },
        PatternType::Consolidation => {
            println!("Network strengthening detected!");
        },
        PatternType::BridgeFormation => {
            println!("Cross-domain connection detected!");
        },
        PatternType::Cascade => {
            println!("Temporal causality detected!");
        },
        _ => {}
    }

    // Check statistical significance
    if pattern.is_significant {
        println!("P-value: {:.4}", pattern.p_value);
        println!("Effect size: {:.2}", pattern.effect_size);
    }
}).await;
```

### Pattern Types

- **CoherenceBreak**: Network is fragmenting (min-cut decreased)
- **Consolidation**: Network is strengthening (min-cut increased)
- **EmergingCluster**: New dense subgraph forming
- **DissolvingCluster**: Existing cluster dissolving
- **BridgeFormation**: Cross-domain connections forming
- **Cascade**: Changes propagating through network
- **TemporalShift**: Temporal pattern change detected
- **AnomalousNode**: Outlier vector detected

## Metrics

### StreamingMetrics

```rust
pub struct StreamingMetrics {
    pub vectors_processed: u64,        // Total vectors ingested
    pub patterns_detected: u64,        // Total patterns found
    pub avg_latency_ms: f64,          // Average processing latency
    pub throughput_per_sec: f64,      // Vectors per second
    pub windows_processed: u64,        // Time windows analyzed
    pub backpressure_events: u64,     // Times buffer was full
    pub errors: u64,                  // Processing errors
    pub peak_buffer_size: usize,      // Max buffer usage
}
```

Access metrics:

```rust
let metrics = engine.metrics().await;
println!("Throughput: {:.1} vectors/sec", metrics.throughput_per_sec);
println!("Avg latency: {:.2}ms", metrics.avg_latency_ms);
println!("Uptime: {:.1}s", metrics.uptime_secs());
```

## Performance Optimization

### Batch Size

Larger batches improve throughput but increase latency:

```rust
.batch_size(500)  // High throughput, higher latency
.batch_size(50)   // Lower throughput, lower latency
```

### Concurrency

Increase parallel processing for CPU-bound workloads:

```rust
.max_concurrency(8)  // 8 concurrent batch processors
```

### Buffer Size

Control memory usage and backpressure:

```rust
.max_buffer_size(50000)  // Larger buffer, less backpressure
.max_buffer_size(1000)   // Smaller buffer, more backpressure
```

### SIMD Acceleration

Enable SIMD for 4-8x speedup on vector operations:

```rust
use ruvector_data_framework::optimized::OptimizedConfig;

let discovery_config = OptimizedConfig {
    use_simd: true,  // Enable SIMD (default)
    ..Default::default()
};
```

## Examples

### Climate Data Streaming

```rust
use futures::stream;
use std::time::Duration;

// Configure for climate data analysis
let engine = StreamingEngineBuilder::new()
    .window_size(Duration::from_secs(3600))    // 1-hour windows
    .slide_interval(Duration::from_secs(900))  // Slide every 15 minutes
    .batch_size(200)
    .max_concurrency(4)
    .build();

// Stream climate observations
let climate_stream = get_climate_data_stream().await?;
engine.ingest_stream(climate_stream).await?;
```

### Financial Market Data

```rust
// Configure for high-frequency financial data
let engine = StreamingEngineBuilder::new()
    .window_size(Duration::from_secs(60))      // 1-minute windows
    .slide_interval(Duration::from_secs(10))   // Slide every 10 seconds
    .batch_size(1000)                          // Large batches
    .max_concurrency(8)                        // High parallelism
    .detection_interval(500)                   // Check patterns frequently
    .build();

let market_stream = get_market_data_stream().await?;
engine.ingest_stream(market_stream).await?;
```

## Backpressure Handling

The streaming engine automatically applies backpressure when the buffer fills:

```rust
let engine = StreamingEngineBuilder::new()
    .max_buffer_size(5000)  // Limit to 5000 vectors
    .build();

// Engine will slow down ingestion if processing can't keep up
engine.ingest_stream(fast_stream).await?;

let metrics = engine.metrics().await;
println!("Backpressure events: {}", metrics.backpressure_events);
```

## Error Handling

```rust
use ruvector_data_framework::Result;

async fn ingest_with_error_handling() -> Result<()> {
    let mut engine = StreamingEngineBuilder::new().build();

    match engine.ingest_stream(vector_stream).await {
        Ok(_) => println!("Ingestion complete"),
        Err(e) => {
            eprintln!("Ingestion error: {}", e);
            let metrics = engine.metrics().await;
            eprintln!("Processed {} vectors before error", metrics.vectors_processed);
        }
    }

    Ok(())
}
```

## Running the Examples

```bash
# Basic streaming demo
cargo run --example streaming_demo --features parallel

# Specific examples
cargo run --example streaming_demo --features parallel -- sliding
cargo run --example streaming_demo --features parallel -- tumbling
cargo run --example streaming_demo --features parallel -- patterns
cargo run --example streaming_demo --features parallel -- throughput
```

## Best Practices

1. **Choose appropriate window sizes**: Too small = noise, too large = delayed detection
2. **Tune batch size**: Balance throughput vs. latency for your use case
3. **Monitor backpressure**: High backpressure indicates processing bottleneck
4. **Use SIMD**: Enable SIMD for significant performance gains on x86_64
5. **Set significance thresholds**: Adjust p-value threshold to reduce false positives
6. **Profile your workload**: Use metrics to identify optimization opportunities

## Troubleshooting

### High Latency

- Reduce batch size
- Increase concurrency
- Enable SIMD acceleration
- Check for slow pattern callbacks

### High Memory Usage

- Reduce max_buffer_size
- Reduce window size
- Increase processing speed

### Missed Patterns

- Increase detection_interval frequency
- Lower similarity_threshold
- Lower significance_threshold
- Increase window overlap (sliding windows)

## Architecture

```
                    ┌─────────────────────┐
                    │  Input Stream       │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │  Backpressure       │
                    │  Semaphore          │
                    └──────────┬──────────┘
                               │
            ┌──────────────────┼──────────────────┐
            │                  │                  │
    ┌───────▼────────┐ ┌──────▼─────────┐ ┌─────▼──────┐
    │  Window 1      │ │  Window 2      │ │  Window N  │
    │  (Sliding)     │ │  (Sliding)     │ │  (Sliding) │
    └───────┬────────┘ └──────┬─────────┘ └─────┬──────┘
            │                  │                  │
            └──────────────────┼──────────────────┘
                               │
                    ┌──────────▼──────────┐
                    │  Batch Processor    │
                    │  (Parallel)         │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │  Discovery Engine   │
                    │  (SIMD + Min-Cut)   │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │  Pattern Detection  │
                    │  (Statistical)      │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │  Callbacks          │
                    └─────────────────────┘
```

## License

Same as RuVector project.
