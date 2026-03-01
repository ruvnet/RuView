# Ruvector Rust API Reference

Complete API reference for `ruvector-core` crate.

## Table of Contents

1. [VectorDB](#vectordb)
2. [AgenticDB](#agenticdb)
3. [Types](#types)
4. [Configuration](#configuration)
5. [Advanced Features](#advanced-features)
6. [Error Handling](#error-handling)

## VectorDB

Core vector database with HNSW indexing.

### Creation

```rust
use ruvector_core::{VectorDB, DbOptions};

pub fn new(options: DbOptions) -> Result<Self>
```

Create a new vector database.

**Parameters**:
- `options`: Database configuration

**Returns**: `Result<VectorDB, RuvectorError>`

**Example**:
```rust
let mut options = DbOptions::default();
options.dimensions = 128;
options.storage_path = "./vectors.db".to_string();

let db = VectorDB::new(options)?;
```

### open

```rust
pub fn open<P: AsRef<Path>>(path: P) -> Result<Self>
```

Open an existing database.

**Parameters**:
- `path`: Path to database directory

**Returns**: `Result<VectorDB, RuvectorError>`

**Example**:
```rust
let db = VectorDB::open("./vectors.db")?;
```

### insert

```rust
pub fn insert(&self, entry: VectorEntry) -> Result<VectorId>
```

Insert a single vector.

**Parameters**:
- `entry`: Vector entry with optional ID and metadata

**Returns**: `Result<VectorId, RuvectorError>` - ID of inserted vector

**Example**:
```rust
let entry = VectorEntry {
    id: None,  // Auto-generate
    vector: vec![0.1; 128],
    metadata: None,
};

let id = db.insert(entry)?;
```

### insert_batch

```rust
pub fn insert_batch(&self, entries: Vec<VectorEntry>) -> Result<Vec<VectorId>>
```

Insert multiple vectors efficiently.

**Parameters**:
- `entries`: Vector of entries to insert

**Returns**: `Result<Vec<VectorId>, RuvectorError>` - IDs of inserted vectors

**Time Complexity**: O(n log m) where n is batch size, m is existing vectors

**Example**:
```rust
let entries: Vec<VectorEntry> = (0..1000)
    .map(|i| VectorEntry {
        id: Some(format!("vec_{}", i)),
        vector: vec![0.1; 128],
        metadata: None,
    })
    .collect();

let ids = db.insert_batch(entries)?;
assert_eq!(ids.len(), 1000);
```

### search

```rust
pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>>
```

Search for similar vectors.

**Parameters**:
- `query`: Search query with vector, k, filters

**Returns**: `Result<Vec<SearchResult>, RuvectorError>` - Top-k results

**Time Complexity**: O(log n) with HNSW

**Example**:
```rust
let query = SearchQuery {
    vector: vec![0.1; 128],
    k: 10,
    filter: None,
    include_vectors: false,
};

let results = db.search(&query)?;

for result in results {
    println!("ID: {}, Distance: {}", result.id, result.distance);
}
```

### delete

```rust
pub fn delete(&self, id: &VectorId) -> Result<()>
```

Delete a vector by ID.

**Parameters**:
- `id`: Vector ID to delete

**Returns**: `Result<(), RuvectorError>`

**Example**:
```rust
db.delete(&"vec_001".to_string())?;
```

### update

```rust
pub fn update(&self, id: &VectorId, entry: VectorEntry) -> Result<()>
```

Update an existing vector.

**Parameters**:
- `id`: Vector ID to update
- `entry`: New vector data

**Returns**: `Result<(), RuvectorError>`

**Example**:
```rust
let new_entry = VectorEntry {
    id: Some("vec_001".to_string()),
    vector: vec![0.2; 128],
    metadata: Some(HashMap::from([
        ("updated".into(), json!(true))
    ])),
};

db.update(&"vec_001".to_string(), new_entry)?;
```

### count

```rust
pub fn count(&self) -> usize
```

Get total number of vectors.

**Returns**: Number of vectors in database

**Example**:
```rust
let total = db.count();
println!("Total vectors: {}", total);
```

## AgenticDB

Extended API with specialized agent memory tables.

### Creation

```rust
use ruvector_core::{AgenticDB, DbOptions};

pub fn new(options: DbOptions) -> Result<Self>
```

Create AgenticDB instance.

**Example**:
```rust
let db = AgenticDB::new(DbOptions::default())?;
```

### Reflexion Memory

#### store_episode

```rust
pub fn store_episode(
    &self,
    task: String,
    actions: Vec<String>,
    observations: Vec<String>,
    critique: String,
) -> Result<String>
```

Store a self-critique episode.

**Parameters**:
- `task`: Task description
- `actions`: Actions taken
- `observations`: Observations made
- `critique`: Self-generated critique

**Returns**: Episode ID

**Example**:
```rust
let id = db.store_episode(
    "Solve coding problem".into(),
    vec!["Read problem".into(), "Write solution".into()],
    vec!["Tests failed".into()],
    "Should test edge cases first".into(),
)?;
```

#### retrieve_episodes

```rust
pub fn retrieve_episodes(
    &self,
    query_embedding: Vec<f32>,
    k: usize,
) -> Result<Vec<ReflexionEpisode>>
```

Retrieve similar past episodes.

**Parameters**:
- `query_embedding`: Embedded critique or task
- `k`: Number of episodes to retrieve

**Returns**: Similar episodes

**Example**:
```rust
let episodes = db.retrieve_episodes(critique_embedding, 5)?;

for ep in episodes {
    println!("Task: {}", ep.task);
    println!("Critique: {}", ep.critique);
}
```

### Skill Library

#### create_skill

```rust
pub fn create_skill(
    &self,
    name: String,
    description: String,
    parameters: HashMap<String, String>,
    examples: Vec<String>,
) -> Result<String>
```

Create a reusable skill.

**Parameters**:
- `name`: Skill name
- `description`: What the skill does
- `parameters`: Required parameters
- `examples`: Usage examples

**Returns**: Skill ID

**Example**:
```rust
let id = db.create_skill(
    "authenticate_user".into(),
    "Authenticate user with JWT token".into(),
    HashMap::from([
        ("token".into(), "string".into()),
        ("user_id".into(), "string".into()),
    ]),
    vec!["authenticate_user(token, user_id)".into()],
)?;
```

#### search_skills

```rust
pub fn search_skills(
    &self,
    query_embedding: Vec<f32>,
    k: usize,
) -> Result<Vec<Skill>>
```

Search for relevant skills.

**Parameters**:
- `query_embedding`: Embedded task description
- `k`: Number of skills to retrieve

**Returns**: Relevant skills

**Example**:
```rust
let skills = db.search_skills(task_embedding, 3)?;

for skill in skills {
    println!("Skill: {} - {}", skill.name, skill.description);
    println!("Success rate: {:.1}%", skill.success_rate * 100.0);
}
```

### Causal Memory

#### add_causal_edge

```rust
pub fn add_causal_edge(
    &self,
    causes: Vec<String>,
    effects: Vec<String>,
    confidence: f64,
    context: String,
) -> Result<String>
```

Add cause-effect relationship.

**Parameters**:
- `causes`: Cause actions/states (hypergraph: multiple causes)
- `effects`: Effect actions/states (hypergraph: multiple effects)
- `confidence`: Confidence score (0-1)
- `context`: Context description

**Returns**: Edge ID

**Example**:
```rust
let id = db.add_causal_edge(
    vec!["authenticate".into(), "validate_token".into()],
    vec!["access_granted".into()],
    0.95,
    "User authentication flow".into(),
)?;
```

#### query_causal

```rust
pub fn query_causal(
    &self,
    query_embedding: Vec<f32>,
    k: usize,
) -> Result<Vec<CausalQueryResult>>
```

Query causal relationships.

**Parameters**:
- `query_embedding`: Embedded context
- `k`: Number of results

**Returns**: Causal edges with utility scores

**Example**:
```rust
let results = db.query_causal(context_embedding, 10)?;

for result in results {
    println!("Causes: {:?} → Effects: {:?}", result.edge.causes, result.edge.effects);
    println!("Utility: {:.4}", result.utility_score);
}
```

### Learning Sessions

#### create_learning_session

```rust
pub fn create_learning_session(
    &self,
    algorithm: String,
    state_dim: usize,
    action_dim: usize,
) -> Result<String>
```

Create RL training session.

**Parameters**:
- `algorithm`: RL algorithm (Q-Learning, DQN, PPO, etc.)
- `state_dim`: State dimensionality
- `action_dim`: Action dimensionality

**Returns**: Session ID

**Example**:
```rust
let session_id = db.create_learning_session(
    "PPO".into(),
    64,  // state_dim
    4,   // action_dim
)?;
```

#### add_experience

```rust
pub fn add_experience(
    &self,
    session_id: &str,
    state: Vec<f32>,
    action: Vec<f32>,
    reward: f64,
    next_state: Vec<f32>,
    done: bool,
) -> Result<()>
```

Add experience to session.

**Parameters**:
- `session_id`: Session ID
- `state`: Current state
- `action`: Action taken
- `reward`: Reward received
- `next_state`: Next state
- `done`: Episode finished?

**Returns**: `Result<(), RuvectorError>`

**Example**:
```rust
db.add_experience(
    &session_id,
    state,
    action,
    1.0,  // reward
    next_state,
    false,  // not done
)?;
```

#### predict_with_confidence

```rust
pub fn predict_with_confidence(
    &self,
    session_id: &str,
    state: Vec<f32>,
) -> Result<Prediction>
```

Predict action with confidence intervals.

**Parameters**:
- `session_id`: Session ID
- `state`: Current state

**Returns**: Prediction with confidence bounds

**Example**:
```rust
let prediction = db.predict_with_confidence(&session_id, state)?;

println!("Action: {:?}", prediction.action);
println!("Confidence: [{:.2}, {:.2}]",
    prediction.confidence_lower,
    prediction.confidence_upper
);
```

## Types

### VectorEntry

```rust
pub struct VectorEntry {
    pub id: Option<String>,
    pub vector: Vec<f32>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}
```

Entry for inserting vectors.

### SearchQuery

```rust
pub struct SearchQuery {
    pub vector: Vec<f32>,
    pub k: usize,
    pub filter: Option<serde_json::Value>,
    pub include_vectors: bool,
}
```

Query for searching vectors.

### SearchResult

```rust
pub struct SearchResult {
    pub id: String,
    pub distance: f32,
    pub vector: Option<Vec<f32>>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}
```

Search result with ID, distance, and optional data.

### DistanceMetric

```rust
pub enum DistanceMetric {
    Euclidean,   // L2 distance
    Cosine,      // 1 - cosine_similarity
    DotProduct,  // -dot_product (for maximization)
    Manhattan,   // L1 distance
}
```

Distance metrics for similarity calculation.

## Configuration

### DbOptions

```rust
pub struct DbOptions {
    pub dimensions: usize,
    pub storage_path: String,
    pub distance_metric: DistanceMetric,
    pub hnsw: HnswConfig,
    pub quantization: QuantizationConfig,
    pub mmap_vectors: bool,
}
```

Database configuration options.

### HnswConfig

```rust
pub struct HnswConfig {
    pub m: usize,               // Connections per node (16-64)
    pub ef_construction: usize, // Build quality (100-400)
    pub ef_search: usize,       // Search quality (50-500)
    pub max_elements: usize,    // Maximum vectors
}
```

HNSW index configuration.

### QuantizationConfig

```rust
pub enum QuantizationConfig {
    None,
    Scalar,                            // 4x compression
    Product { subspaces: usize, k: usize }, // 8-16x compression
    Binary,                            // 32x compression
}
```

Quantization configuration.

## Advanced Features

### HybridSearch

Combine vector similarity with keyword search.

```rust
use ruvector_core::{HybridSearch, HybridConfig};

let config = HybridConfig {
    vector_weight: 0.7,
    bm25_weight: 0.3,
    k1: 1.5,
    b: 0.75,
};

let hybrid = HybridSearch::new(&db, config)?;
let results = hybrid.search(&query_vector, &["keywords"], 10)?;
```

### FilteredSearch

Apply metadata filters.

```rust
use ruvector_core::{FilteredSearch, FilterExpression, FilterStrategy};

let filtered = FilteredSearch::new(&db, FilterStrategy::PreFilter);

let filter = FilterExpression::And(vec![
    FilterExpression::Eq("category".into(), json!("tech")),
    FilterExpression::Gte("score".into(), json!(0.8)),
]);

let results = filtered.search(&query, 10, Some(filter))?;
```

### MMRSearch

Maximal Marginal Relevance for diversity.

```rust
use ruvector_core::{MMRSearch, MMRConfig};

let config = MMRConfig {
    lambda: 0.5,  // Balance relevance vs diversity
    diversity_weight: 0.3,
};

let mmr = MMRSearch::new(&db, config)?;
let results = mmr.search(&query, 20)?;
```

### ConformalPredictor

Uncertainty quantification.

```rust
use ruvector_core::{ConformalPredictor, ConformalConfig};

let mut predictor = ConformalPredictor::new(ConformalConfig {
    alpha: 0.1,  // 90% confidence
    calibration_size: 1000,
});

predictor.calibrate(&calibration_data)?;
let prediction = predictor.predict(&query, &db)?;
```

## Error Handling

### RuvectorError

```rust
pub enum RuvectorError {
    DimensionMismatch { expected: usize, got: usize },
    StorageError(String),
    IndexError(String),
    SerializationError(String),
    IoError(std::io::Error),
    // ... more variants
}
```

All operations return `Result<T, RuvectorError>`.

**Example**:
```rust
match db.insert(entry) {
    Ok(id) => println!("Inserted: {}", id),
    Err(RuvectorError::DimensionMismatch { expected, got }) => {
        eprintln!("Wrong dimensions: expected {}, got {}", expected, got);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Complete API Documentation

For complete auto-generated API docs:

```bash
cargo doc --no-deps --open
```

Or visit: [https://docs.rs/ruvector-core](https://docs.rs/ruvector-core)
