# Ruvector Server

[![Crates.io](https://img.shields.io/crates/v/ruvector-server.svg)](https://crates.io/crates/ruvector-server)
[![Documentation](https://docs.rs/ruvector-server/badge.svg)](https://docs.rs/ruvector-server)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**High-performance REST API server for Ruvector vector databases.**

`ruvector-server` provides a production-ready HTTP API built on Axum with CORS support, compression, and OpenAPI documentation. Exposes full Ruvector functionality via RESTful endpoints. Part of the [Ruvector](https://github.com/ruvnet/ruvector) ecosystem.

## Why Ruvector Server?

- **Fast**: Built on Axum and Tokio for high throughput
- **Production Ready**: CORS, compression, tracing built-in
- **RESTful API**: Standard HTTP endpoints for all operations
- **OpenAPI**: Auto-generated API documentation
- **Multi-Collection**: Support multiple vector collections

## Features

### Core Capabilities

- **Vector CRUD**: Insert, get, update, delete vectors
- **Search API**: k-NN search with filtering
- **Batch Operations**: Bulk insert and search
- **Collection Management**: Create and manage collections
- **Health Checks**: Liveness and readiness probes

### Advanced Features

- **CORS Support**: Configurable cross-origin requests
- **Compression**: GZIP response compression
- **Tracing**: Request tracing with tower-http
- **Rate Limiting**: Request rate limiting (planned)
- **Authentication**: API key auth (planned)

## Installation

Add `ruvector-server` to your `Cargo.toml`:

```toml
[dependencies]
ruvector-server = "0.1.1"
```

## Quick Start

### Start Server

```rust
use ruvector_server::{Server, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure server
    let config = ServerConfig {
        host: "0.0.0.0".to_string(),
        port: 8080,
        cors_origins: vec!["*".to_string()],
        enable_compression: true,
        ..Default::default()
    };

    // Create and start server
    let server = Server::new(config)?;
    server.run().await?;

    Ok(())
}
```

### API Endpoints

```bash
# Health check
GET /health

# Collections
POST   /collections              # Create collection
GET    /collections              # List collections
GET    /collections/{name}       # Get collection info
DELETE /collections/{name}       # Delete collection

# Vectors
POST   /collections/{name}/vectors       # Insert vector(s)
GET    /collections/{name}/vectors/{id}  # Get vector
DELETE /collections/{name}/vectors/{id}  # Delete vector

# Search
POST   /collections/{name}/search        # k-NN search
POST   /collections/{name}/search/batch  # Batch search
```

### Example Requests

```bash
# Create collection
curl -X POST http://localhost:8080/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "documents",
    "dimensions": 384,
    "distance_metric": "cosine"
  }'

# Insert vector
curl -X POST http://localhost:8080/collections/documents/vectors \
  -H "Content-Type: application/json" \
  -d '{
    "id": "doc-1",
    "vector": [0.1, 0.2, 0.3, ...],
    "metadata": {"title": "Hello World"}
  }'

# Search
curl -X POST http://localhost:8080/collections/documents/search \
  -H "Content-Type: application/json" \
  -d '{
    "vector": [0.1, 0.2, 0.3, ...],
    "k": 10,
    "filter": {"category": "tech"}
  }'
```

## API Overview

### Server Configuration

```rust
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub enable_compression: bool,
    pub max_body_size: usize,
    pub request_timeout: Duration,
}
```

### Response Types

```rust
// Search response
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub took_ms: u64,
}

pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub vector: Option<Vec<f32>>,
    pub metadata: Option<serde_json::Value>,
}

// Collection info
pub struct CollectionInfo {
    pub name: String,
    pub dimensions: usize,
    pub count: usize,
    pub distance_metric: String,
}
```

### Error Handling

```rust
// API errors return standard format
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

// HTTP status codes:
// 200 - Success
// 201 - Created
// 400 - Bad Request
// 404 - Not Found
// 500 - Internal Error
```

## Docker Deployment

```dockerfile
FROM rust:1.77 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p ruvector-server

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/ruvector-server /usr/local/bin/
EXPOSE 8080
CMD ["ruvector-server"]
```

```bash
docker build -t ruvector-server .
docker run -p 8080:8080 ruvector-server
```

## Related Crates

- **[ruvector-core](../ruvector-core/)** - Core vector database engine
- **[ruvector-collections](../ruvector-collections/)** - Collection management
- **[ruvector-cli](../ruvector-cli/)** - Command-line interface

## Documentation

- **[Main README](../../README.md)** - Complete project overview
- **[API Documentation](https://docs.rs/ruvector-server)** - Full API reference
- **[GitHub Repository](https://github.com/ruvnet/ruvector)** - Source code

## License

**MIT License** - see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [Ruvector](https://github.com/ruvnet/ruvector) - Built by [rUv](https://ruv.io)**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)

[Documentation](https://docs.rs/ruvector-server) | [Crates.io](https://crates.io/crates/ruvector-server) | [GitHub](https://github.com/ruvnet/ruvector)

</div>
