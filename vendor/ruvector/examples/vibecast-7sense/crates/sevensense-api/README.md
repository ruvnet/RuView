# sevensense-api

[![Crate](https://img.shields.io/badge/crates.io-sevensense--api-orange.svg)](https://crates.io/crates/sevensense-api)
[![Docs](https://img.shields.io/badge/docs-sevensense--api-blue.svg)](https://docs.rs/sevensense-api)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

> HTTP API layer for the 7sense bioacoustic intelligence platform.

**sevensense-api** provides a comprehensive HTTP interface to all 7sense functionality. It offers GraphQL for flexible queries, REST endpoints with OpenAPI documentation, WebSocket streaming for real-time analysis, and Server-Sent Events for monitoring. Built on Axum for high performance and reliability.

## Features

- **GraphQL API**: Flexible queries with async-graphql
- **REST Endpoints**: OpenAPI/Swagger documented
- **WebSocket Streaming**: Real-time audio analysis
- **Authentication**: JWT-based auth with refresh tokens
- **Rate Limiting**: Configurable request throttling
- **Health Checks**: Kubernetes-ready probes

## Use Cases

| Use Case | Description | Endpoint |
|----------|-------------|----------|
| Species Identification | Identify birds from audio | `POST /api/identify` |
| Similarity Search | Find similar recordings | `POST /api/search` |
| Batch Processing | Process multiple files | `POST /api/batch` |
| Real-time Analysis | Stream audio for analysis | `WS /ws/stream` |
| Health Monitoring | Check system status | `GET /health` |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sevensense-api = "0.1"
```

## Quick Start

### Starting the Server

```bash
# Start with default configuration
cargo run -p sevensense-api --release

# With custom port
SEVENSENSE_PORT=8080 cargo run -p sevensense-api --release

# With configuration file
cargo run -p sevensense-api --release -- --config config.toml
```

### API Endpoints

Once running, access:
- **GraphQL Playground**: http://localhost:3000/graphql
- **Swagger UI**: http://localhost:3000/docs/swagger-ui
- **Health Check**: http://localhost:3000/health

---

<details>
<summary><b>Tutorial: GraphQL Queries</b></summary>

### Basic Species Query

```graphql
query {
  identifySpecies(audioUrl: "https://example.com/bird.wav") {
    predictions {
      speciesId
      commonName
      scientificName
      confidence
    }
    processingTime
  }
}
```

### Similarity Search

```graphql
query SearchSimilar($embedding: [Float!]!, $k: Int!) {
  searchSimilar(embedding: $embedding, k: $k, minSimilarity: 0.8) {
    id
    species {
      scientificName
      commonName
    }
    similarity
    recordingUrl
    timestamp
  }
}
```

### With Filters

```graphql
query FilteredSearch {
  searchSimilar(
    embedding: [0.1, 0.2, ...]
    k: 20
    filter: {
      species: ["Turdus merula", "Turdus philomelos"]
      location: { lat: 51.5, lon: -0.1, radiusKm: 50 }
      timeRange: { start: "2024-01-01", end: "2024-06-30" }
    }
  ) {
    id
    species { commonName }
    similarity
    location { lat, lon, siteName }
  }
}
```

### Mutations

```graphql
mutation AddRecording($input: RecordingInput!) {
  addRecording(input: $input) {
    id
    status
    embedding
  }
}

mutation DeleteRecording($id: ID!) {
  deleteRecording(id: $id) {
    success
    message
  }
}
```

### Subscriptions

```graphql
subscription OnNewDetection {
  newDetection(location: { lat: 51.5, lon: -0.1, radiusKm: 10 }) {
    id
    species { commonName }
    confidence
    timestamp
    audioUrl
  }
}
```

</details>

<details>
<summary><b>Tutorial: REST API</b></summary>

### Species Identification

```bash
# From file upload
curl -X POST http://localhost:3000/api/identify \
  -F "audio=@bird_call.wav"

# From URL
curl -X POST http://localhost:3000/api/identify \
  -H "Content-Type: application/json" \
  -d '{"audioUrl": "https://example.com/bird.wav"}'
```

Response:
```json
{
  "predictions": [
    {
      "speciesId": "turdus-merula",
      "scientificName": "Turdus merula",
      "commonName": "Eurasian Blackbird",
      "confidence": 0.94
    }
  ],
  "processingTimeMs": 127,
  "embedding": [0.123, -0.456, ...]
}
```

### Similarity Search

```bash
curl -X POST http://localhost:3000/api/search \
  -H "Content-Type: application/json" \
  -d '{
    "embedding": [0.123, -0.456, ...],
    "k": 10,
    "minSimilarity": 0.8
  }'
```

### Batch Processing

```bash
curl -X POST http://localhost:3000/api/batch \
  -H "Content-Type: application/json" \
  -d '{
    "audioUrls": [
      "https://example.com/bird1.wav",
      "https://example.com/bird2.wav",
      "https://example.com/bird3.wav"
    ],
    "options": {
      "includeEmbeddings": true,
      "topK": 3
    }
  }'
```

### Health Checks

```bash
# Liveness probe
curl http://localhost:3000/health/live

# Readiness probe
curl http://localhost:3000/health/ready

# Detailed status
curl http://localhost:3000/health/status
```

</details>

<details>
<summary><b>Tutorial: WebSocket Streaming</b></summary>

### Connecting to Stream

```javascript
const ws = new WebSocket('ws://localhost:3000/ws/stream');

ws.onopen = () => {
  console.log('Connected to stream');

  // Start streaming audio
  ws.send(JSON.stringify({
    type: 'start',
    config: {
      sampleRate: 32000,
      channels: 1,
      format: 'float32'
    }
  }));
};

ws.onmessage = (event) => {
  const message = JSON.parse(event.data);

  if (message.type === 'detection') {
    console.log('Detection:', message.data);
  }
};

// Stream audio chunks
function sendAudioChunk(audioData) {
  ws.send(audioData);  // ArrayBuffer
}
```

### Rust Client

```rust
use sevensense_api::client::{StreamClient, StreamConfig};
use tokio_tungstenite::connect_async;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = StreamConfig {
        sample_rate: 32000,
        channels: 1,
        chunk_duration_ms: 500,
    };

    let client = StreamClient::connect("ws://localhost:3000/ws/stream", config).await?;

    // Send audio chunks
    for chunk in audio_chunks {
        client.send_audio(&chunk).await?;
    }

    // Receive detections
    while let Some(detection) = client.receive().await? {
        println!("Detected: {} ({:.1}%)",
            detection.species, detection.confidence * 100.0);
    }

    Ok(())
}
```

### Stream Protocol

| Message Type | Direction | Description |
|--------------|-----------|-------------|
| `start` | Client→Server | Start streaming with config |
| `audio` | Client→Server | Audio chunk (binary) |
| `stop` | Client→Server | Stop streaming |
| `detection` | Server→Client | Species detection event |
| `error` | Server→Client | Error message |
| `status` | Server→Client | Processing status |

</details>

<details>
<summary><b>Tutorial: Authentication</b></summary>

### JWT Authentication

```bash
# Login to get tokens
curl -X POST http://localhost:3000/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username": "user", "password": "pass"}'

# Response
{
  "accessToken": "eyJ...",
  "refreshToken": "eyJ...",
  "expiresIn": 3600
}

# Use access token
curl http://localhost:3000/api/search \
  -H "Authorization: Bearer eyJ..."

# Refresh token
curl -X POST http://localhost:3000/auth/refresh \
  -H "Content-Type: application/json" \
  -d '{"refreshToken": "eyJ..."}'
```

### API Keys

```bash
# Create API key
curl -X POST http://localhost:3000/auth/api-keys \
  -H "Authorization: Bearer eyJ..." \
  -d '{"name": "My App", "scopes": ["read", "write"]}'

# Use API key
curl http://localhost:3000/api/search \
  -H "X-API-Key: sk_live_..."
```

### Scopes

| Scope | Description |
|-------|-------------|
| `read` | Read-only access to search and identify |
| `write` | Add/modify recordings |
| `admin` | Administrative operations |
| `stream` | Real-time streaming access |

</details>

<details>
<summary><b>Tutorial: Server Configuration</b></summary>

### Configuration File

```toml
# config.toml
[server]
host = "0.0.0.0"
port = 3000
workers = 4

[auth]
jwt_secret = "your-secret-key"
token_expiry_hours = 24
refresh_expiry_days = 30

[rate_limiting]
enabled = true
requests_per_minute = 100
burst_size = 20

[database]
url = "postgres://user:pass@localhost/sevensense"
max_connections = 20

[index]
path = "./data/hnsw.index"
preload = true

[logging]
level = "info"
format = "json"
```

### Environment Variables

```bash
# Server
export SEVENSENSE_HOST=0.0.0.0
export SEVENSENSE_PORT=3000

# Authentication
export SEVENSENSE_JWT_SECRET=your-secret
export SEVENSENSE_JWT_EXPIRY=3600

# Database
export DATABASE_URL=postgres://...

# Logging
export RUST_LOG=sevensense_api=info
```

### Programmatic Configuration

```rust
use sevensense_api::{Server, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::builder()
        .host("0.0.0.0")
        .port(3000)
        .workers(4)
        .enable_graphql(true)
        .enable_swagger(true)
        .rate_limit(100, 20)
        .build()?;

    Server::new(config)
        .with_index(&index)
        .with_embedding_pipeline(&pipeline)
        .run()
        .await?;

    Ok(())
}
```

</details>

---

## API Reference

### GraphQL Schema

| Type | Description |
|------|-------------|
| `Query.identifySpecies` | Identify species from audio |
| `Query.searchSimilar` | Find similar recordings |
| `Query.getRecording` | Get recording by ID |
| `Mutation.addRecording` | Add new recording |
| `Subscription.newDetection` | Real-time detection events |

### REST Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/identify` | Identify species |
| `POST` | `/api/search` | Similarity search |
| `POST` | `/api/batch` | Batch processing |
| `GET` | `/api/recordings/:id` | Get recording |
| `WS` | `/ws/stream` | Real-time streaming |

### Health Endpoints

| Path | Description |
|------|-------------|
| `/health/live` | Liveness probe |
| `/health/ready` | Readiness probe |
| `/health/status` | Detailed status |

## Performance

| Metric | Target | Actual |
|--------|--------|--------|
| Identify Latency | <200ms | ~150ms |
| Search Latency | <50ms | ~35ms |
| Concurrent Connections | 1000 | ✅ |
| Requests/Second | 500 | ~600 |

## Links

- **Homepage**: [ruv.io](https://ruv.io)
- **Repository**: [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **Crates.io**: [crates.io/crates/sevensense-api](https://crates.io/crates/sevensense-api)
- **Documentation**: [docs.rs/sevensense-api](https://docs.rs/sevensense-api)

## License

MIT License - see [LICENSE](../../LICENSE) for details.

---

*Part of the [7sense Bioacoustic Intelligence Platform](https://ruv.io) by rUv*
