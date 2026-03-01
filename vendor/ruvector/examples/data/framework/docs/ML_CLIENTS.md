# AI/ML API Clients for RuVector Data Discovery Framework

This module provides comprehensive integration with AI/ML platforms for discovering models, datasets, and research papers.

## Available Clients

### 1. HuggingFaceClient

**Purpose**: Access HuggingFace model hub and inference API

**Features**:
- Search models by query and task type
- Get model details and metadata
- List and search datasets
- Run model inference
- Convert models/datasets to SemanticVectors

**API Details**:
- Base URL: `https://huggingface.co/api`
- Rate limit: 30 requests/minute (free tier)
- API key: Optional via `HUGGINGFACE_API_KEY` environment variable
- Mock fallback: Yes (when no API key provided)

**Example**:
```rust
use ruvector_data_framework::HuggingFaceClient;

let client = HuggingFaceClient::new();

// Search for BERT models
let models = client.search_models("bert", Some("fill-mask")).await?;

// Get specific model
let model = client.get_model("bert-base-uncased").await?;

// Convert to vector for discovery
if let Some(m) = model {
    let vector = client.model_to_vector(&m);
    println!("Model: {}, Embedding dim: {}", vector.id, vector.embedding.len());
}

// List datasets
let datasets = client.list_datasets(Some("nlp")).await?;

// Run inference (requires API key)
let result = client.inference(
    "bert-base-uncased",
    serde_json::json!({"inputs": "Hello [MASK]!"})
).await?;
```

### 2. OllamaClient

**Purpose**: Local LLM inference with Ollama

**Features**:
- List locally available models
- Generate text completions
- Chat with message history
- Generate embeddings
- Pull models from Ollama library
- Automatic mock fallback when Ollama not running

**API Details**:
- Base URL: `http://localhost:11434/api` (default)
- Rate limit: None (local service)
- API key: Not required
- Mock fallback: Yes (when Ollama service unavailable)

**Example**:
```rust
use ruvector_data_framework::{OllamaClient, OllamaChatMessage};

let mut client = OllamaClient::new();

// Check if Ollama is running
if client.is_available().await {
    // List available models
    let models = client.list_models().await?;

    // Generate completion
    let response = client.generate(
        "llama2",
        "Explain quantum computing in simple terms"
    ).await?;

    // Chat with message history
    let messages = vec![
        OllamaChatMessage {
            role: "user".to_string(),
            content: "What is machine learning?".to_string(),
        }
    ];
    let chat_response = client.chat("llama2", messages).await?;

    // Generate embeddings
    let embedding = client.embeddings("llama2", "sample text").await?;
    println!("Embedding dimension: {}", embedding.len());
}
```

**Setup**:
```bash
# Install Ollama
curl https://ollama.ai/install.sh | sh

# Start Ollama service
ollama serve

# Pull a model
ollama pull llama2
```

### 3. ReplicateClient

**Purpose**: Access Replicate's cloud ML model platform

**Features**:
- Get model information
- Create predictions (run models)
- Check prediction status
- List model collections
- Convert models to SemanticVectors

**API Details**:
- Base URL: `https://api.replicate.com/v1`
- Rate limit: Varies by plan
- API key: Required via `REPLICATE_API_TOKEN` environment variable
- Mock fallback: Yes (when no API token provided)

**Example**:
```rust
use ruvector_data_framework::ReplicateClient;

let client = ReplicateClient::new();

// Get model info
let model = client.get_model("stability-ai", "stable-diffusion").await?;

if let Some(m) = model {
    println!("Model: {}/{}", m.owner, m.name);

    // Convert to vector
    let vector = client.model_to_vector(&m);

    // Create a prediction
    let prediction = client.create_prediction(
        "stability-ai/stable-diffusion",
        serde_json::json!({
            "prompt": "a beautiful sunset over mountains"
        })
    ).await?;

    // Check prediction status
    let status = client.get_prediction(&prediction.id).await?;
    println!("Status: {}", status.status);
}

// List collections
let collections = client.list_collections().await?;
```

**Environment Setup**:
```bash
export REPLICATE_API_TOKEN="your_token_here"
```

### 4. TogetherAiClient

**Purpose**: Access Together AI's open source model hosting

**Features**:
- List available models
- Chat completions
- Generate embeddings
- Support for various open source LLMs
- Convert models to SemanticVectors

**API Details**:
- Base URL: `https://api.together.xyz/v1`
- Rate limit: Varies by plan
- API key: Required via `TOGETHER_API_KEY` environment variable
- Mock fallback: Yes (when no API key provided)

**Example**:
```rust
use ruvector_data_framework::{TogetherAiClient, TogetherMessage};

let client = TogetherAiClient::new();

// List models
let models = client.list_models().await?;

for model in models.iter().take(5) {
    println!("Model: {}", model.display_name.as_deref().unwrap_or(&model.id));
    println!("Context: {} tokens", model.context_length.unwrap_or(0));
}

// Chat completion
let messages = vec![
    TogetherMessage {
        role: "user".to_string(),
        content: "Explain neural networks".to_string(),
    }
];

let response = client.chat_completion(
    "togethercomputer/llama-2-7b",
    messages
).await?;

println!("Response: {}", response);

// Generate embeddings
let embedding = client.embeddings(
    "togethercomputer/m2-bert-80M-8k-retrieval",
    "sample text for embedding"
).await?;
```

**Environment Setup**:
```bash
export TOGETHER_API_KEY="your_key_here"
```

### 5. PapersWithCodeClient

**Purpose**: Access Papers With Code research database

**Features**:
- Search ML research papers
- Get paper details
- List datasets
- Get state-of-the-art (SOTA) benchmarks
- Search methods/techniques
- Convert papers/datasets to SemanticVectors

**API Details**:
- Base URL: `https://paperswithcode.com/api/v1`
- Rate limit: 60 requests/minute
- API key: Not required
- Mock fallback: Partial (for some endpoints)

**Example**:
```rust
use ruvector_data_framework::PapersWithCodeClient;

let client = PapersWithCodeClient::new();

// Search papers
let papers = client.search_papers("transformer").await?;

for paper in papers.iter().take(5) {
    println!("Title: {}", paper.title);
    if let Some(url) = &paper.url_abs {
        println!("URL: {}", url);
    }

    // Convert to vector
    let vector = client.paper_to_vector(paper);
    println!("Vector ID: {}", vector.id);
}

// Get specific paper
let paper = client.get_paper("attention-is-all-you-need").await?;

// List datasets
let datasets = client.list_datasets().await?;

for dataset in datasets.iter().take(5) {
    println!("Dataset: {}", dataset.name);

    // Convert to vector
    let vector = client.dataset_to_vector(dataset);
}

// Get SOTA results for a task
let sota_results = client.get_sota("image-classification").await?;

for result in sota_results {
    println!("Task: {}, Dataset: {}, Metric: {}, Value: {}",
        result.task, result.dataset, result.metric, result.value);
}
```

## Integration with RuVector Discovery

All clients provide conversion methods to transform their data into `SemanticVector` format for use with RuVector's discovery engine:

```rust
use ruvector_data_framework::{
    HuggingFaceClient, PapersWithCodeClient, Domain,
    NativeDiscoveryEngine, NativeEngineConfig
};

// Create clients
let hf_client = HuggingFaceClient::new();
let pwc_client = PapersWithCodeClient::new();

// Collect vectors from different sources
let mut vectors = Vec::new();

// Add HuggingFace models
let models = hf_client.search_models("transformer", None).await?;
for model in models {
    vectors.push(hf_client.model_to_vector(&model));
}

// Add research papers
let papers = pwc_client.search_papers("attention mechanism").await?;
for paper in papers {
    vectors.push(pwc_client.paper_to_vector(&paper));
}

// Run discovery analysis
let config = NativeEngineConfig::default();
let mut engine = NativeDiscoveryEngine::new(config);

for vector in vectors {
    engine.ingest_vector(vector)?;
}

// Detect patterns
let patterns = engine.detect_patterns()?;
println!("Found {} discovery patterns", patterns.len());
```

## Environment Variables

| Variable | Client | Required | Description |
|----------|--------|----------|-------------|
| `HUGGINGFACE_API_KEY` | HuggingFaceClient | No | Optional for public models, required for private/inference |
| `REPLICATE_API_TOKEN` | ReplicateClient | Yes* | Required for API access (*falls back to mock) |
| `TOGETHER_API_KEY` | TogetherAiClient | Yes* | Required for API access (*falls back to mock) |
| - | OllamaClient | No | Uses local Ollama service |
| - | PapersWithCodeClient | No | Public API, no key needed |

## Mock Data Fallback

All clients (except PapersWithCodeClient) provide automatic mock data when:
- API keys are not provided
- Services are unavailable
- Rate limits are exceeded (after retries)

This allows for:
- Development without API keys
- Testing without external dependencies
- Graceful degradation in production

## Rate Limiting

All clients implement automatic rate limiting:
- Configurable delays between requests
- Exponential backoff on failures
- Automatic retry logic (up to 3 retries)
- Respects API rate limits

## Error Handling

All clients use the framework's `Result<T>` type with `FrameworkError`:

```rust
use ruvector_data_framework::{HuggingFaceClient, FrameworkError};

match hf_client.search_models("bert", None).await {
    Ok(models) => {
        println!("Found {} models", models.len());
    }
    Err(FrameworkError::Network(e)) => {
        eprintln!("Network error: {}", e);
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

## Testing

The module includes comprehensive unit tests:

```bash
# Run all ML client tests
cargo test ml_clients

# Run specific client tests
cargo test ml_clients::tests::test_huggingface
cargo test ml_clients::tests::test_ollama
cargo test ml_clients::tests::test_replicate
cargo test ml_clients::tests::test_together
cargo test ml_clients::tests::test_paperswithcode

# Run integration tests (requires API keys)
cargo test ml_clients::tests --ignored
```

## Example Application

See `examples/ml_clients_demo.rs` for a complete demonstration:

```bash
# Run demo (uses mock data)
cargo run --example ml_clients_demo

# Run with API keys
export HUGGINGFACE_API_KEY="your_key"
export REPLICATE_API_TOKEN="your_token"
export TOGETHER_API_KEY="your_key"
cargo run --example ml_clients_demo
```

## Performance Considerations

- **HuggingFace**: 30 req/min free tier → 2 second delays
- **Ollama**: Local, minimal delays (100ms)
- **Replicate**: Pay-per-use, 1 second delays
- **Together AI**: Pay-per-use, 1 second delays
- **Papers With Code**: 60 req/min → 1 second delays

For bulk operations, use batch processing with appropriate delays.

## Architecture

All clients follow a consistent pattern:

1. **Client struct**: Holds HTTP client, embedder, base URL, credentials
2. **API response structs**: Deserialize API responses
3. **Public methods**: High-level API operations
4. **Conversion methods**: Transform to `SemanticVector`
5. **Mock methods**: Provide fallback data
6. **Retry logic**: Handle transient failures
7. **Tests**: Comprehensive unit testing

## Dependencies

- `reqwest`: HTTP client
- `tokio`: Async runtime
- `serde`: Serialization/deserialization
- `chrono`: Timestamp handling
- `urlencoding`: URL parameter encoding

## Contributing

When adding new ML API clients:

1. Follow the established pattern (see existing clients)
2. Implement rate limiting
3. Provide mock fallback data
4. Add comprehensive tests (at least 15 tests)
5. Update this documentation
6. Add example usage

## License

Same as RuVector framework license.
