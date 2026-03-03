/*!
# Hyprstream: Real-time Aggregation Windows and High-Performance Cache for Apache Arrow Flight SQL

Hyprstream is a next-generation application for real-time data ingestion, windowed aggregation, caching, and serving.
Built on Apache Arrow Flight and DuckDB, and developed in Rust, Hyprstream dynamically calculates metrics like running
sums, counts, and averages, enabling blazing-fast data workflows, intelligent caching, and seamless integration with
ADBC-compliant datastores.

## Key Features

### Data Ingestion via Apache Arrow Flight
- Streamlined ingestion using Arrow Flight for efficient columnar data transport
- Real-time streaming support for metrics, datasets, and vectorized data
- Seamless integration with data producers for high-throughput ingestion
- Write-through to ADBC datastores for eventual data consistency

### Intelligent Read Caching with DuckDB
- In-memory performance using DuckDB for lightning-fast caching
- Optimized querying for analytics workloads
- Automatic cache management with configurable expiry policies
- Time-based expiry with future support for LRU/LFU policies

### Data Serving with Arrow Flight SQL
- High-performance queries via Arrow Flight SQL
- Support for vectorized data and analytical queries
- Seamless integration with analytics and visualization tools

### Real-Time Aggregation
- Dynamic metrics with running sums, counts, and averages
- Lightweight state management for aggregate calculations
- Dynamic weight computation for AI/ML pipelines
- Time window partitioning for granular analysis

## Usage

Basic usage example with programmatic configuration:

```rust,no_run
use hyprstream::config::{Settings, EngineConfig, CacheConfig};
use hyprstream::service::FlightServiceImpl;
use std::sync::Arc;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration programmatically
    let mut settings = Settings::default();

    // Configure primary storage engine
    settings.engine.engine = "duckdb".to_string();
    settings.engine.connection = ":memory:".to_string();
    settings.engine.options.insert("threads".to_string(), "4".to_string());

    // Configure caching (optional)
    settings.cache.enabled = true;
    settings.cache.engine = "duckdb".to_string();
    settings.cache.connection = ":memory:".to_string();
    settings.cache.max_duration_secs = 3600;

    // Create and initialize the service
    let service = FlightServiceImpl::from_settings(&settings).await?;

    // Use the service in your application...
    Ok(())
}
```

For detailed configuration options and examples, see:
- [`config`](crate::config) module for configuration options
- [`storage`](crate::storage) module for storage backend details
- [`examples`](examples/) directory for more usage examples
*/

pub mod metrics;
pub mod storage;
pub mod service;
pub mod config;
pub mod aggregation;

pub use service::FlightSqlService;
pub use storage::StorageBackend;
pub use metrics::MetricRecord;
pub use aggregation::{TimeWindow, AggregateFunction, GroupBy, AggregateResult};
