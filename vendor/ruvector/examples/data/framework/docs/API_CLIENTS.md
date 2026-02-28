# RuVector API Client Integration Guide

This document describes the real API client integrations for OpenAlex, NOAA, and SEC EDGAR datasets in the RuVector discovery framework.

## Overview

The `api_clients` module provides three production-ready API clients that fetch data from public APIs and convert it to RuVector's `DataRecord` format with embeddings:

1. **OpenAlexClient** - Academic works, authors, and research topics
2. **NoaaClient** - Climate observations and weather data
3. **EdgarClient** - SEC company filings and financial disclosures

All clients implement the `DataSource` trait for seamless integration with RuVector's discovery pipeline.

## Features

- **Async/Await**: Built on `tokio` and `reqwest` for efficient concurrent requests
- **Rate Limiting**: Automatic rate limiting with configurable delays
- **Retry Logic**: Built-in retry mechanism with exponential backoff
- **Error Handling**: Comprehensive error handling with custom error types
- **Embeddings**: Simple bag-of-words text embeddings (128-dimensional)
- **Relationships**: Automatic extraction of relationships between records
- **DataSource Trait**: Standard interface for data ingestion pipelines

## OpenAlex Client

Academic database with 250M+ works, 60M+ authors, and research topics.

### Quick Start

```rust
use ruvector_data_framework::OpenAlexClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAlexClient::new(Some("your-email@example.com".to_string()))?;

    // Fetch academic works
    let works = client.fetch_works("quantum computing", 10).await?;
    println!("Found {} works", works.len());

    // Fetch research topics
    let topics = client.fetch_topics("artificial intelligence").await?;
    println!("Found {} topics", topics.len());

    Ok(())
}
```

### API Methods

#### `fetch_works(query: &str, limit: usize) -> Result<Vec<DataRecord>>`

Fetch academic works by search query.

**Parameters:**
- `query`: Search string (searches title, abstract, etc.)
- `limit`: Maximum number of results (max 200 per request)

**Returns:**
- `DataRecord` with:
  - `source`: "openalex"
  - `record_type`: "work"
  - `data`: Title, abstract, citations
  - `embedding`: 128-dimensional text vector
  - `relationships`: Authors (`authored_by`) and concepts (`has_concept`)

**Example:**
```rust
let works = client.fetch_works("machine learning", 20).await?;
for work in works {
    println!("Title: {}", work.data["title"]);
    println!("Citations: {}", work.data.get("citations").unwrap_or(&0));
    println!("Authors: {}", work.relationships.len());
}
```

#### `fetch_topics(domain: &str) -> Result<Vec<DataRecord>>`

Fetch research topics by domain.

**Parameters:**
- `domain`: Research domain or keyword

**Returns:**
- `DataRecord` with topic metadata and embeddings

### Data Structure

```rust
DataRecord {
    id: "https://openalex.org/W2964141474",
    source: "openalex",
    record_type: "work",
    timestamp: "2021-05-15T00:00:00Z",
    data: {
        "title": "Attention Is All You Need",
        "abstract": "...",
        "citations": 15234
    },
    embedding: Some(vec![0.12, -0.34, ...]), // 128 dims
    relationships: [
        Relationship {
            target_id: "https://openalex.org/A123456",
            rel_type: "authored_by",
            weight: 1.0,
            properties: { "author_name": "John Doe" }
        }
    ]
}
```

### Rate Limiting

- Default: 100ms between requests
- Polite API usage: Include email in constructor
- Automatic retry on 429 (Too Many Requests)

## NOAA Client

Climate and weather observations from NOAA's NCDC database.

### Quick Start

```rust
use ruvector_data_framework::NoaaClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // API token from https://www.ncdc.noaa.gov/cdo-web/token
    let client = NoaaClient::new(Some("your-noaa-token".to_string()))?;

    // NYC Central Park station
    let observations = client.fetch_climate_data(
        "GHCND:USW00094728",
        "2024-01-01",
        "2024-01-31"
    ).await?;

    for obs in observations {
        println!("{}: {}", obs.data["datatype"], obs.data["value"]);
    }

    Ok(())
}
```

### API Methods

#### `fetch_climate_data(station_id: &str, start_date: &str, end_date: &str) -> Result<Vec<DataRecord>>`

Fetch climate observations for a weather station.

**Parameters:**
- `station_id`: GHCND station ID (e.g., "GHCND:USW00094728")
- `start_date`: Start date in YYYY-MM-DD format
- `end_date`: End date in YYYY-MM-DD format

**Returns:**
- `DataRecord` with:
  - `source`: "noaa"
  - `record_type`: "observation"
  - `data`: Station, datatype (TMAX/TMIN/PRCP), value
  - `embedding`: 128-dimensional vector

### Data Types

Common observation types:
- **TMAX**: Maximum temperature (tenths of degrees C)
- **TMIN**: Minimum temperature (tenths of degrees C)
- **PRCP**: Precipitation (tenths of mm)
- **SNOW**: Snowfall (mm)
- **SNWD**: Snow depth (mm)

### Synthetic Data Mode

If no API token is provided, the client generates synthetic data for testing:

```rust
let client = NoaaClient::new(None)?;
let synthetic_data = client.fetch_climate_data(
    "TEST_STATION",
    "2024-01-01",
    "2024-01-31"
).await?;
// Returns 3 synthetic observations (TMAX, TMIN, PRCP)
```

### Rate Limiting

- Default: 200ms between requests (stricter than OpenAlex)
- NOAA has rate limits of ~5 requests/second

## SEC EDGAR Client

SEC company filings and financial disclosures.

### Quick Start

```rust
use ruvector_data_framework::EdgarClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // User agent must include your email per SEC requirements
    let client = EdgarClient::new(
        "MyApp/1.0 (your-email@example.com)".to_string()
    )?;

    // Apple Inc. (CIK: 0000320193)
    let filings = client.fetch_filings("320193", Some("10-K")).await?;

    for filing in filings {
        println!("Form: {}", filing.data["form"]);
        println!("Filed: {}", filing.data["filing_date"]);
        println!("URL: {}", filing.data["filing_url"]);
    }

    Ok(())
}
```

### API Methods

#### `fetch_filings(cik: &str, form_type: Option<&str>) -> Result<Vec<DataRecord>>`

Fetch company filings by CIK (Central Index Key).

**Parameters:**
- `cik`: Company CIK (e.g., "320193" for Apple)
- `form_type`: Optional filter for form type ("10-K", "10-Q", "8-K", etc.)

**Returns:**
- `DataRecord` with:
  - `source`: "edgar"
  - `record_type`: Form type ("10-K", "10-Q", etc.)
  - `data`: CIK, accession number, dates, filing URL
  - `embedding`: 128-dimensional vector

### Common Form Types

- **10-K**: Annual report
- **10-Q**: Quarterly report
- **8-K**: Current events
- **DEF 14A**: Proxy statement
- **S-1**: Registration statement

### Finding CIK Numbers

CIK numbers can be found at:
- https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany
- Search by company name or ticker symbol

**Common CIKs:**
- Apple (AAPL): 0000320193
- Microsoft (MSFT): 0000789019
- Tesla (TSLA): 0001318605
- Amazon (AMZN): 0001018724

### Rate Limiting

- Default: 100ms between requests
- SEC requires max 10 requests/second
- **User-Agent required**: Must include email address

### Data Structure

```rust
DataRecord {
    id: "0000320193_0000320193-23-000106",
    source: "edgar",
    record_type: "10-K",
    timestamp: "2023-11-03T00:00:00Z",
    data: {
        "cik": "0000320193",
        "accession_number": "0000320193-23-000106",
        "filing_date": "2023-11-03",
        "report_date": "2023-09-30",
        "form": "10-K",
        "primary_document": "aapl-20230930.htm",
        "filing_url": "https://www.sec.gov/cgi-bin/viewer?..."
    },
    embedding: Some(vec![...]),
    relationships: []
}
```

## Simple Embedder

All clients use the `SimpleEmbedder` for generating text embeddings.

### Features

- **Bag-of-words**: Simple hash-based word counting
- **Normalized**: L2-normalized vectors
- **Configurable dimension**: Default 128
- **Fast**: No external API calls

### Usage

```rust
use ruvector_data_framework::SimpleEmbedder;

let embedder = SimpleEmbedder::new(128);

// From text
let embedding = embedder.embed_text("machine learning artificial intelligence");
assert_eq!(embedding.len(), 128);

// From JSON
let json = serde_json::json!({"title": "Research Paper"});
let embedding = embedder.embed_json(&json);
```

### Algorithm

1. Convert text to lowercase
2. Split into words (filter words < 3 chars)
3. Hash each word to embedding dimension index
4. Count occurrences in embedding vector
5. L2-normalize the vector

**Note**: This is a simple demo embedder. For production, consider using transformer-based models.

## DataSource Trait

All clients implement the `DataSource` trait for pipeline integration.

```rust
use ruvector_data_framework::{DataSource, OpenAlexClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAlexClient::new(None)?;

    // Source identifier
    println!("Source: {}", client.source_id()); // "openalex"

    // Health check
    let healthy = client.health_check().await?;
    println!("Healthy: {}", healthy);

    // Batch fetching
    let (records, next_cursor) = client.fetch_batch(None, 10).await?;
    println!("Fetched {} records", records.len());

    Ok(())
}
```

## Integration with Discovery Pipeline

Combine API clients with RuVector's discovery pipeline:

```rust
use ruvector_data_framework::{
    OpenAlexClient, DiscoveryPipeline, PipelineConfig
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create API client
    let client = OpenAlexClient::new(Some("demo@example.com".to_string()))?;

    // Configure discovery pipeline
    let config = PipelineConfig::default();
    let mut pipeline = DiscoveryPipeline::new(config);

    // Run discovery
    let patterns = pipeline.run(client).await?;

    println!("Discovered {} patterns", patterns.len());
    for pattern in patterns {
        println!("- {:?}: {}", pattern.category, pattern.description);
    }

    Ok(())
}
```

## Error Handling

All clients use the framework's `FrameworkError` type:

```rust
use ruvector_data_framework::{Result, FrameworkError};

async fn fetch_data() -> Result<()> {
    match client.fetch_works("query", 10).await {
        Ok(works) => println!("Success: {} works", works.len()),
        Err(FrameworkError::Network(e)) => eprintln!("Network error: {}", e),
        Err(FrameworkError::Config(msg)) => eprintln!("Config error: {}", msg),
        Err(e) => eprintln!("Other error: {}", e),
    }
    Ok(())
}
```

## Testing

Run tests for the API clients:

```bash
# All API client tests
cargo test --lib api_clients

# Specific test
cargo test --lib test_simple_embedder

# Run the demo example
cargo run --example api_client_demo
```

## Examples

See `/home/user/ruvector/examples/data/framework/examples/api_client_demo.rs` for a complete working example.

```bash
cd /home/user/ruvector/examples/data/framework
cargo run --example api_client_demo
```

## Performance Considerations

### Rate Limiting

Each client has default rate limits to comply with API terms of service:
- **OpenAlex**: 100ms (10 req/sec)
- **NOAA**: 200ms (5 req/sec)
- **EDGAR**: 100ms (10 req/sec)

### Retry Strategy

- 3 retries with exponential backoff
- 1 second initial retry delay
- Doubles on each retry

### Memory Usage

- Embeddings are 128-dimensional (512 bytes per vector)
- Records cached during batch operations
- Use streaming for large datasets

## API Keys and Authentication

### OpenAlex
- **No API key required**
- Recommended: Provide email via constructor
- Polite pool: 100k requests/day

### NOAA
- **API token required** for production use
- Get token: https://www.ncdc.noaa.gov/cdo-web/token
- Free tier: 1000 requests/day
- Synthetic data mode available (no token)

### SEC EDGAR
- **No API key required**
- **User-Agent header required** (must include email)
- Rate limit: 10 requests/second
- Full access to public filings

## Future Enhancements

Potential improvements:
- [ ] Transformer-based embeddings (sentence-transformers)
- [ ] Pagination support for large result sets
- [ ] Caching layer for repeated queries
- [ ] Batch embedding generation
- [ ] Additional data sources (arXiv, PubMed, etc.)
- [ ] WebSocket streaming for real-time updates
- [ ] GraphQL support for flexible queries

## Resources

- **OpenAlex**: https://docs.openalex.org/
- **NOAA NCDC**: https://www.ncdc.noaa.gov/cdo-web/webservices/v2
- **SEC EDGAR**: https://www.sec.gov/edgar/sec-api-documentation
- **RuVector Framework**: /home/user/ruvector/examples/data/framework/

## License

Same as parent RuVector project.
