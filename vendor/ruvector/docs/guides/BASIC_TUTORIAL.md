# Basic Tutorial

This tutorial walks through the core features of Ruvector with practical examples.

## Prerequisites

- Completed [Installation](INSTALLATION.md)
- Basic understanding of vectors/embeddings
- Familiarity with Rust or Node.js

## Tutorial Overview

1. [Create a Vector Database](#1-create-a-vector-database)
2. [Insert Vectors](#2-insert-vectors)
3. [Search for Similar Vectors](#3-search-for-similar-vectors)
4. [Add Metadata](#4-add-metadata)
5. [Batch Operations](#5-batch-operations)
6. [Configure HNSW](#6-configure-hnsw)
7. [Enable Quantization](#7-enable-quantization)
8. [Persistence](#8-persistence)

## 1. Create a Vector Database

### Rust

```rust
use ruvector_core::{VectorDB, DbOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = DbOptions::default();
    options.dimensions = 128;  // Vector dimensionality
    options.storage_path = "./my_vectors.db".to_string();

    let db = VectorDB::new(options)?;
    println!("Created database with 128 dimensions");

    Ok(())
}
```

### Node.js

```javascript
const { VectorDB } = require('ruvector');

const db = new VectorDB({
    dimensions: 128,
    storagePath: './my_vectors.db'
});

console.log('Created database with 128 dimensions');
```

## 2. Insert Vectors

### Rust

```rust
use ruvector_core::{VectorDB, VectorEntry};

fn insert_examples(db: &VectorDB) -> Result<(), Box<dyn std::error::Error>> {
    // Insert a single vector
    let entry = VectorEntry {
        id: None,  // Auto-generate ID
        vector: vec![0.1; 128],
        metadata: None,
    };

    let id = db.insert(entry)?;
    println!("Inserted vector with ID: {}", id);

    // Insert with custom ID
    let entry = VectorEntry {
        id: Some("doc_001".to_string()),
        vector: vec![0.2; 128],
        metadata: None,
    };

    db.insert(entry)?;
    println!("Inserted vector with custom ID: doc_001");

    Ok(())
}
```

### Node.js

```javascript
// Insert a single vector
const id = await db.insert({
    vector: new Float32Array(128).fill(0.1)
});
console.log('Inserted vector with ID:', id);

// Insert with custom ID
await db.insert({
    id: 'doc_001',
    vector: new Float32Array(128).fill(0.2)
});
console.log('Inserted vector with custom ID: doc_001');
```

## 3. Search for Similar Vectors

### Rust

```rust
use ruvector_core::SearchQuery;

fn search_examples(db: &VectorDB) -> Result<(), Box<dyn std::error::Error>> {
    let query = SearchQuery {
        vector: vec![0.15; 128],
        k: 10,  // Return top 10 results
        filter: None,
        ef_search: None,
    };

    let results = db.search(&query)?;

    for (i, result) in results.iter().enumerate() {
        println!(
            "{}. ID: {}, Distance: {:.4}",
            i + 1,
            result.id,
            result.score
        );
    }

    Ok(())
}
```

### Node.js

```javascript
const results = await db.search({
    vector: new Float32Array(128).fill(0.15),
    k: 10
});

results.forEach((result, i) => {
    console.log(`${i + 1}. ID: ${result.id}, Distance: ${result.score.toFixed(4)}`);
});
```

## 4. Add Metadata

Metadata allows you to store additional information with each vector.

### Rust

```rust
use serde_json::json;
use std::collections::HashMap;

fn insert_with_metadata(db: &VectorDB) -> Result<(), Box<dyn std::error::Error>> {
    let mut metadata = HashMap::new();
    metadata.insert("title".to_string(), json!("Example Document"));
    metadata.insert("author".to_string(), json!("Alice"));
    metadata.insert("tags".to_string(), json!(["ml", "ai", "embeddings"]));
    metadata.insert("timestamp".to_string(), json!(1234567890));

    let entry = VectorEntry {
        id: Some("doc_002".to_string()),
        vector: vec![0.3; 128],
        metadata: Some(metadata),
    };

    db.insert(entry)?;
    println!("Inserted vector with metadata");

    Ok(())
}
```

### Node.js

```javascript
await db.insert({
    id: 'doc_002',
    vector: new Float32Array(128).fill(0.3),
    metadata: {
        title: 'Example Document',
        author: 'Alice',
        tags: ['ml', 'ai', 'embeddings'],
        timestamp: 1234567890
    }
});

console.log('Inserted vector with metadata');
```

### Retrieve metadata in search

```javascript
const results = await db.search({
    vector: new Float32Array(128).fill(0.3),
    k: 5,
    includeMetadata: true
});

results.forEach(result => {
    console.log(`ID: ${result.id}`);
    console.log(`Title: ${result.metadata.title}`);
    console.log(`Tags: ${result.metadata.tags.join(', ')}`);
    console.log('---');
});
```

## 5. Batch Operations

Batch operations are significantly faster than individual operations.

### Rust

```rust
fn batch_insert(db: &VectorDB) -> Result<(), Box<dyn std::error::Error>> {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Create 1000 random vectors
    let entries: Vec<VectorEntry> = (0..1000)
        .map(|i| {
            let vector: Vec<f32> = (0..128)
                .map(|_| rng.gen::<f32>())
                .collect();

            VectorEntry {
                id: Some(format!("vec_{:04}", i)),
                vector,
                metadata: None,
            }
        })
        .collect();

    // Batch insert
    let start = std::time::Instant::now();
    let ids = db.insert_batch(entries)?;
    let duration = start.elapsed();

    println!("Inserted {} vectors in {:?}", ids.len(), duration);
    println!("Throughput: {:.0} vectors/sec", ids.len() as f64 / duration.as_secs_f64());

    Ok(())
}
```

### Node.js

```javascript
// Create 1000 random vectors
const entries = Array.from({ length: 1000 }, (_, i) => ({
    id: `vec_${i.toString().padStart(4, '0')}`,
    vector: new Float32Array(128).map(() => Math.random())
}));

// Batch insert
const start = Date.now();
const ids = await db.insertBatch(entries);
const duration = Date.now() - start;

console.log(`Inserted ${ids.length} vectors in ${duration}ms`);
console.log(`Throughput: ${Math.floor(ids.length / (duration / 1000))} vectors/sec`);
```

## 6. Configure HNSW

Tune HNSW parameters for your use case.

### Rust

```rust
use ruvector_core::types::{HnswConfig, DbOptions};
use ruvector_core::DistanceMetric;

fn create_tuned_db() -> Result<VectorDB, Box<dyn std::error::Error>> {
    let mut options = DbOptions::default();
    options.dimensions = 128;
    options.storage_path = "./tuned_db.db".to_string();

    // HNSW configuration
    options.hnsw_config = Some(HnswConfig {
        m: 32,                    // Connections per node (16-64)
        ef_construction: 200,     // Build quality (100-400)
        ef_search: 100,           // Search quality (50-500)
        max_elements: 10_000_000, // Maximum vectors
    });

    // Distance metric
    options.distance_metric = DistanceMetric::Cosine;

    let db = VectorDB::new(options)?;
    println!("Created database with tuned HNSW parameters");

    Ok(db)
}
```

### Node.js

```javascript
const db = new VectorDB({
    dimensions: 128,
    storagePath: './tuned_db.db',
    hnsw: {
        m: 32,              // Connections per node
        efConstruction: 200, // Build quality
        efSearch: 100,      // Search quality
        maxElements: 10_000_000
    },
    distanceMetric: 'cosine'
});

console.log('Created database with tuned HNSW parameters');
```

### Parameter trade-offs

| Parameter | Low | Medium | High |
|-----------|-----|--------|------|
| `m` | 16 (low memory) | 32 (balanced) | 64 (high recall) |
| `ef_construction` | 100 (fast build) | 200 (balanced) | 400 (high quality) |
| `ef_search` | 50 (fast search) | 100 (balanced) | 500 (high recall) |

## 7. Enable Quantization

Reduce memory usage with quantization.

### Rust

```rust
use ruvector_core::types::{QuantizationConfig, DbOptions};

fn create_quantized_db() -> Result<VectorDB, Box<dyn std::error::Error>> {
    let mut options = DbOptions::default();
    options.dimensions = 128;
    options.storage_path = "./quantized_db.db".to_string();

    // Scalar quantization (4x compression)
    options.quantization = Some(QuantizationConfig::Scalar);

    // Product quantization (8-16x compression)
    // options.quantization = Some(QuantizationConfig::Product {
    //     subspaces: 16,
    //     k: 256,
    // });

    let db = VectorDB::new(options)?;
    println!("Created database with scalar quantization");

    Ok(db)
}
```

### Node.js

```javascript
const db = new VectorDB({
    dimensions: 128,
    storagePath: './quantized_db.db',
    quantization: {
        type: 'scalar'  // or 'product', 'binary'
    }
});

console.log('Created database with scalar quantization');
```

### Quantization comparison

| Type | Compression | Recall | Use Case |
|------|-------------|--------|----------|
| None | 1x | 100% | Small datasets, high accuracy |
| Scalar | 4x | 97-99% | General purpose |
| Product | 8-16x | 90-95% | Large datasets |
| Binary | 32x | 80-90% | Filtering stage |

## 8. Persistence

Ruvector automatically persists data to disk.

### Load existing database

```rust
// Rust
let db = VectorDB::open("./my_vectors.db")?;

// Node.js
const db = new VectorDB({ storagePath: './my_vectors.db' });
```

### Export/Import

```rust
// Export to JSON
db.export_json("./export.json")?;

// Import from JSON
db.import_json("./export.json")?;
```

### Backup

```bash
# Simple file copy (database is in a consistent state)
cp -r ./my_vectors.db ./backup/

# Or use ruvector CLI
ruvector export --db ./my_vectors.db --output ./backup.json
ruvector import --db ./new_db.db --input ./backup.json
```

## Complete Example

Here's a complete program combining everything:

```rust
use ruvector_core::{VectorDB, VectorEntry, SearchQuery, DistanceMetric};
use ruvector_core::types::{DbOptions, HnswConfig, QuantizationConfig};
use rand::Rng;
use serde_json::json;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create database with tuned settings
    let mut options = DbOptions::default();
    options.dimensions = 128;
    options.storage_path = "./tutorial_db.db".to_string();
    options.hnsw_config = Some(HnswConfig {
        m: 32,
        ef_construction: 200,
        ef_search: 100,
        max_elements: 1_000_000,
    });
    options.distance_metric = DistanceMetric::Cosine;
    options.quantization = Some(QuantizationConfig::Scalar);

    let db = VectorDB::new(options)?;
    println!("✓ Created database");

    // 2. Insert vectors with metadata
    let mut rng = rand::thread_rng();
    let entries: Vec<VectorEntry> = (0..10000)
        .map(|i| {
            let vector: Vec<f32> = (0..128)
                .map(|_| rng.gen::<f32>())
                .collect();

            let mut metadata = HashMap::new();
            metadata.insert("id".to_string(), json!(i));
            metadata.insert("category".to_string(), json!(i % 10));

            VectorEntry {
                id: Some(format!("doc_{:05}", i)),
                vector,
                metadata: Some(metadata),
            }
        })
        .collect();

    let start = std::time::Instant::now();
    db.insert_batch(entries)?;
    println!("✓ Inserted 10,000 vectors in {:?}", start.elapsed());

    // 3. Search
    let query_vector: Vec<f32> = (0..128).map(|_| rng.gen::<f32>()).collect();
    let query = SearchQuery {
        vector: query_vector,
        k: 10,
        filter: None,
        ef_search: None,
    };

    let start = std::time::Instant::now();
    let results = db.search(&query)?;
    let search_time = start.elapsed();

    println!("✓ Search completed in {:?}", search_time);
    println!("\nTop 10 Results:");
    for (i, result) in results.iter().enumerate() {
        println!("  {}. ID: {}, Distance: {:.4}", i + 1, result.id, result.score);
    }

    Ok(())
}
```

## Next Steps

- [Advanced Features Guide](ADVANCED_FEATURES.md) - Hybrid search, filtering, MMR
- [AgenticDB Tutorial](AGENTICDB_TUTORIAL.md) - Reflexion memory, skills, causal memory
- [Performance Tuning](PERFORMANCE_TUNING.md) - Optimization guide
- [API Reference](../api/RUST_API.md) - Complete API documentation

## Common Patterns

### Pattern 1: Document Embedding Storage

```rust
// Store document embeddings with full metadata
let doc = VectorEntry {
    id: Some(format!("doc_{}", uuid::Uuid::new_v4())),
    vector: embedding,  // From your embedding model
    metadata: Some(HashMap::from([
        ("title".into(), json!(title)),
        ("content".into(), json!(content_preview)),
        ("url".into(), json!(url)),
        ("timestamp".into(), json!(chrono::Utc::now().timestamp())),
    ])),
};
db.insert(doc)?;
```

### Pattern 2: Semantic Search

```rust
// Embed user query
let query_embedding = embed_text(&user_query);

// Search with filters
let results = db.search(&SearchQuery {
    vector: query_embedding,
    k: 20,
    filter: Some(json!({
        "timestamp": { "$gte": one_week_ago }
    })),
    ef_search: None,
})?;

// Return relevant documents
for result in results {
    println!("{}: {}", result.id, result.metadata["title"]);
}
```

### Pattern 3: Recommendation System

```rust
// Get user's liked items
let user_vectors = get_user_liked_vectors(&db, user_id)?;

// Average embeddings
let avg_vector = average_vectors(&user_vectors);

// Find similar items
let recommendations = db.search(&SearchQuery {
    vector: avg_vector,
    k: 10,
    filter: Some(json!({
        "id": { "$nin": user_already_seen }
    })),
    ef_search: None,
})?;
```

## Troubleshooting

### Low Performance
- Enable SIMD: `RUSTFLAGS="-C target-cpu=native" cargo build --release`
- Use batch operations instead of individual inserts
- Tune HNSW parameters (lower `ef_search` for speed)

### High Memory Usage
- Enable quantization
- Use memory-mapped vectors for large datasets
- Reduce `max_elements` or HNSW `m` parameter

### Low Recall
- Increase `ef_construction` and `ef_search`
- Disable or reduce quantization
- Use Cosine distance for normalized vectors
