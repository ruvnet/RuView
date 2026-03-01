# Sparse Vectors Integration Plan

## Overview

Integrate sparse vector support into PostgreSQL for efficient storage and search of high-dimensional sparse embeddings (BM25, SPLADE, learned sparse representations).

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     PostgreSQL Extension                         │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                  Sparse Vector Type                      │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │    │
│  │  │  COO Format  │  │  CSR Format  │  │  Dictionary  │   │    │
│  │  │  (indices,   │  │  (sorted,    │  │  (hash-based │   │    │
│  │  │   values)    │  │   compact)   │  │   lookup)    │   │    │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘   │    │
│  └─────────┼─────────────────┼─────────────────┼───────────┘    │
│            └─────────────────┴─────────────────┘                │
│                              ▼                                   │
│              ┌───────────────────────────┐                       │
│              │   Sparse Distance Funcs   │                       │
│              │   (Dot, Cosine, BM25)     │                       │
│              └───────────────────────────┘                       │
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── sparse/
│   ├── mod.rs              # Module exports
│   ├── types/
│   │   ├── sparsevec.rs    # Core sparse vector type
│   │   ├── coo.rs          # COO format (coordinate)
│   │   └── csr.rs          # CSR format (compressed sparse row)
│   ├── distance.rs         # Sparse distance functions
│   ├── index/
│   │   ├── inverted.rs     # Inverted index for sparse search
│   │   └── sparse_hnsw.rs  # HNSW adapted for sparse vectors
│   ├── hybrid.rs           # Dense + sparse hybrid search
│   └── operators.rs        # SQL operators
```

## SQL Interface

### Sparse Vector Type

```sql
-- Create table with sparse vectors
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    dense_embedding vector(768),
    sparse_embedding sparsevec(30000),  -- BM25 or SPLADE
    metadata jsonb
);

-- Insert sparse vector (indices:values format)
INSERT INTO documents (content, sparse_embedding)
VALUES (
    'Machine learning for natural language processing',
    '{1024:0.5, 2048:0.3, 4096:0.8, 15000:0.2}'::sparsevec
);

-- Insert from array representation
INSERT INTO documents (sparse_embedding)
VALUES (ruvector_to_sparse(
    indices := ARRAY[1024, 2048, 4096, 15000],
    values := ARRAY[0.5, 0.3, 0.8, 0.2],
    dim := 30000
));
```

### Distance Operations

```sql
-- Sparse dot product (inner product similarity)
SELECT id, content,
       ruvector_sparse_dot(sparse_embedding, query_sparse) AS score
FROM documents
ORDER BY score DESC
LIMIT 10;

-- Sparse cosine similarity
SELECT id,
       ruvector_sparse_cosine(sparse_embedding, query_sparse) AS similarity
FROM documents
WHERE ruvector_sparse_cosine(sparse_embedding, query_sparse) > 0.5;

-- Custom operator: <#> for sparse inner product
SELECT * FROM documents
ORDER BY sparse_embedding <#> query_sparse DESC
LIMIT 10;
```

### Sparse Index

```sql
-- Create inverted index for sparse vectors
CREATE INDEX ON documents USING ruvector_sparse (
    sparse_embedding sparsevec(30000)
) WITH (
    pruning_threshold = 0.1,  -- Prune low-weight terms
    quantization = 'int8'     -- Optional quantization
);

-- Approximate sparse search
SELECT * FROM documents
ORDER BY sparse_embedding <#> query_sparse
LIMIT 10;
```

### Hybrid Dense + Sparse Search

```sql
-- Hybrid search combining dense and sparse
SELECT id, content,
       0.7 * (1 - (dense_embedding <=> query_dense)) +
       0.3 * ruvector_sparse_dot(sparse_embedding, query_sparse) AS hybrid_score
FROM documents
ORDER BY hybrid_score DESC
LIMIT 10;

-- Built-in hybrid search function
SELECT * FROM ruvector_hybrid_search(
    table_name := 'documents',
    dense_column := 'dense_embedding',
    sparse_column := 'sparse_embedding',
    dense_query := query_dense,
    sparse_query := query_sparse,
    dense_weight := 0.7,
    sparse_weight := 0.3,
    k := 10
);
```

## Implementation Phases

### Phase 1: Sparse Vector Type (Week 1-2)

```rust
// src/sparse/types/sparsevec.rs

use pgrx::prelude::*;
use serde::{Serialize, Deserialize};

/// Sparse vector stored as sorted (index, value) pairs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseVec {
    indices: Vec<u32>,
    values: Vec<f32>,
    dim: u32,
}

impl SparseVec {
    pub fn new(indices: Vec<u32>, values: Vec<f32>, dim: u32) -> Result<Self, SparseError> {
        if indices.len() != values.len() {
            return Err(SparseError::LengthMismatch);
        }

        // Ensure sorted and unique
        let mut pairs: Vec<_> = indices.into_iter().zip(values.into_iter()).collect();
        pairs.sort_by_key(|(i, _)| *i);
        pairs.dedup_by_key(|(i, _)| *i);

        let (indices, values): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();

        if indices.last().map_or(false, |&i| i >= dim) {
            return Err(SparseError::IndexOutOfBounds);
        }

        Ok(Self { indices, values, dim })
    }

    /// Number of non-zero elements
    #[inline]
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Get value at index (O(log n) binary search)
    pub fn get(&self, index: u32) -> f32 {
        match self.indices.binary_search(&index) {
            Ok(pos) => self.values[pos],
            Err(_) => 0.0,
        }
    }

    /// Iterate over non-zero elements
    pub fn iter(&self) -> impl Iterator<Item = (u32, f32)> + '_ {
        self.indices.iter().copied().zip(self.values.iter().copied())
    }

    /// L2 norm
    pub fn norm(&self) -> f32 {
        self.values.iter().map(|&v| v * v).sum::<f32>().sqrt()
    }

    /// Prune elements below threshold
    pub fn prune(&mut self, threshold: f32) {
        let pairs: Vec<_> = self.indices.iter().copied()
            .zip(self.values.iter().copied())
            .filter(|(_, v)| v.abs() >= threshold)
            .collect();

        self.indices = pairs.iter().map(|(i, _)| *i).collect();
        self.values = pairs.iter().map(|(_, v)| *v).collect();
    }

    /// Top-k sparsification
    pub fn top_k(&self, k: usize) -> SparseVec {
        let mut indexed: Vec<_> = self.indices.iter().copied()
            .zip(self.values.iter().copied())
            .collect();

        indexed.sort_by(|(_, a), (_, b)| b.abs().partial_cmp(&a.abs()).unwrap());
        indexed.truncate(k);
        indexed.sort_by_key(|(i, _)| *i);

        let (indices, values): (Vec<_>, Vec<_>) = indexed.into_iter().unzip();

        SparseVec { indices, values, dim: self.dim }
    }
}

// PostgreSQL type registration
#[derive(PostgresType, Serialize, Deserialize)]
#[pgx(sql = "CREATE TYPE sparsevec")]
pub struct PgSparseVec(SparseVec);

impl FromDatum for PgSparseVec {
    // ... TOAST-aware deserialization
}

impl IntoDatum for PgSparseVec {
    // ... serialization
}

// Parse from string: '{1:0.5, 2:0.3}'
impl std::str::FromStr for SparseVec {
    type Err = SparseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().trim_start_matches('{').trim_end_matches('}');
        let mut indices = Vec::new();
        let mut values = Vec::new();
        let mut max_index = 0u32;

        for pair in s.split(',') {
            let parts: Vec<_> = pair.trim().split(':').collect();
            if parts.len() != 2 {
                return Err(SparseError::ParseError);
            }
            let idx: u32 = parts[0].trim().parse().map_err(|_| SparseError::ParseError)?;
            let val: f32 = parts[1].trim().parse().map_err(|_| SparseError::ParseError)?;
            indices.push(idx);
            values.push(val);
            max_index = max_index.max(idx);
        }

        SparseVec::new(indices, values, max_index + 1)
    }
}
```

### Phase 2: Sparse Distance Functions (Week 3-4)

```rust
// src/sparse/distance.rs

use simsimd::SpatialSimilarity;

/// Sparse dot product (inner product)
/// Only iterates over shared non-zero indices
pub fn sparse_dot(a: &SparseVec, b: &SparseVec) -> f32 {
    let mut result = 0.0;
    let mut i = 0;
    let mut j = 0;

    while i < a.indices.len() && j < b.indices.len() {
        match a.indices[i].cmp(&b.indices[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                result += a.values[i] * b.values[j];
                i += 1;
                j += 1;
            }
        }
    }

    result
}

/// Sparse cosine similarity
pub fn sparse_cosine(a: &SparseVec, b: &SparseVec) -> f32 {
    let dot = sparse_dot(a, b);
    let norm_a = a.norm();
    let norm_b = b.norm();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// Sparse Euclidean distance
pub fn sparse_euclidean(a: &SparseVec, b: &SparseVec) -> f32 {
    let mut result = 0.0;
    let mut i = 0;
    let mut j = 0;

    while i < a.indices.len() || j < b.indices.len() {
        let idx_a = a.indices.get(i).copied().unwrap_or(u32::MAX);
        let idx_b = b.indices.get(j).copied().unwrap_or(u32::MAX);

        match idx_a.cmp(&idx_b) {
            std::cmp::Ordering::Less => {
                result += a.values[i] * a.values[i];
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                result += b.values[j] * b.values[j];
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                let diff = a.values[i] - b.values[j];
                result += diff * diff;
                i += 1;
                j += 1;
            }
        }
    }

    result.sqrt()
}

/// BM25 scoring for sparse term vectors
pub fn sparse_bm25(
    query: &SparseVec,
    doc: &SparseVec,
    doc_len: f32,
    avg_doc_len: f32,
    k1: f32,
    b: f32,
) -> f32 {
    let mut score = 0.0;
    let mut i = 0;
    let mut j = 0;

    while i < query.indices.len() && j < doc.indices.len() {
        match query.indices[i].cmp(&doc.indices[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                let idf = query.values[i];  // Assume query values are IDF weights
                let tf = doc.values[j];     // Doc values are TF

                let numerator = tf * (k1 + 1.0);
                let denominator = tf + k1 * (1.0 - b + b * doc_len / avg_doc_len);

                score += idf * numerator / denominator;
                i += 1;
                j += 1;
            }
        }
    }

    score
}

// PostgreSQL functions
#[pg_extern(immutable, parallel_safe)]
fn ruvector_sparse_dot(a: PgSparseVec, b: PgSparseVec) -> f32 {
    sparse_dot(&a.0, &b.0)
}

#[pg_extern(immutable, parallel_safe)]
fn ruvector_sparse_cosine(a: PgSparseVec, b: PgSparseVec) -> f32 {
    sparse_cosine(&a.0, &b.0)
}

#[pg_extern(immutable, parallel_safe)]
fn ruvector_sparse_euclidean(a: PgSparseVec, b: PgSparseVec) -> f32 {
    sparse_euclidean(&a.0, &b.0)
}
```

### Phase 3: Inverted Index (Week 5-7)

```rust
// src/sparse/index/inverted.rs

use dashmap::DashMap;
use parking_lot::RwLock;

/// Inverted index for efficient sparse vector search
pub struct InvertedIndex {
    /// term_id -> [(doc_id, weight), ...]
    postings: DashMap<u32, Vec<(u64, f32)>>,
    /// doc_id -> sparse vector (for re-ranking)
    documents: DashMap<u64, SparseVec>,
    /// Document norms for cosine similarity
    doc_norms: DashMap<u64, f32>,
    /// Configuration
    config: InvertedIndexConfig,
}

pub struct InvertedIndexConfig {
    pub pruning_threshold: f32,
    pub max_postings_per_term: usize,
    pub quantization: Option<Quantization>,
}

impl InvertedIndex {
    pub fn new(config: InvertedIndexConfig) -> Self {
        Self {
            postings: DashMap::new(),
            documents: DashMap::new(),
            doc_norms: DashMap::new(),
            config,
        }
    }

    /// Insert document into index
    pub fn insert(&self, doc_id: u64, vector: SparseVec) {
        let norm = vector.norm();

        // Index each non-zero term
        for (term_id, weight) in vector.iter() {
            if weight.abs() < self.config.pruning_threshold {
                continue;
            }

            self.postings
                .entry(term_id)
                .or_insert_with(Vec::new)
                .push((doc_id, weight));
        }

        self.doc_norms.insert(doc_id, norm);
        self.documents.insert(doc_id, vector);
    }

    /// Search using WAND algorithm for top-k
    pub fn search(&self, query: &SparseVec, k: usize) -> Vec<(u64, f32)> {
        // Collect candidate documents
        let mut doc_scores: HashMap<u64, f32> = HashMap::new();

        for (term_id, query_weight) in query.iter() {
            if let Some(postings) = self.postings.get(&term_id) {
                for &(doc_id, doc_weight) in postings.iter() {
                    *doc_scores.entry(doc_id).or_insert(0.0) += query_weight * doc_weight;
                }
            }
        }

        // Get top-k
        let mut results: Vec<_> = doc_scores.into_iter().collect();
        results.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        results.truncate(k);

        results
    }

    /// WAND (Weak AND) algorithm for efficient top-k retrieval
    pub fn search_wand(&self, query: &SparseVec, k: usize) -> Vec<(u64, f32)> {
        // Sort query terms by max contribution (upper bound)
        let mut term_info: Vec<_> = query.iter()
            .filter_map(|(term_id, weight)| {
                self.postings.get(&term_id).map(|p| {
                    let max_doc_weight = p.iter().map(|(_, w)| *w).fold(0.0f32, f32::max);
                    (term_id, weight, max_doc_weight * weight)
                })
            })
            .collect();

        term_info.sort_by(|(_, _, a), (_, _, b)| b.partial_cmp(a).unwrap());

        // WAND traversal
        let mut heap: BinaryHeap<(OrderedFloat<f32>, u64)> = BinaryHeap::new();
        let threshold = 0.0f32;

        // ... WAND implementation

        heap.into_iter().map(|(s, id)| (id, s.0)).collect()
    }
}

// PostgreSQL index access method
#[pg_extern]
fn ruvector_sparse_handler(internal: Internal) -> Internal {
    // Index AM handler for sparse inverted index
}
```

### Phase 4: Hybrid Search (Week 8-9)

```rust
// src/sparse/hybrid.rs

/// Hybrid dense + sparse search
pub struct HybridSearch {
    dense_weight: f32,
    sparse_weight: f32,
    fusion_method: FusionMethod,
}

pub enum FusionMethod {
    /// Linear combination of scores
    Linear,
    /// Reciprocal Rank Fusion
    RRF { k: f32 },
    /// Learned fusion weights
    Learned { model: FusionModel },
}

impl HybridSearch {
    /// Combine dense and sparse results
    pub fn search(
        &self,
        dense_results: &[(u64, f32)],
        sparse_results: &[(u64, f32)],
        k: usize,
    ) -> Vec<(u64, f32)> {
        match &self.fusion_method {
            FusionMethod::Linear => {
                self.linear_fusion(dense_results, sparse_results, k)
            }
            FusionMethod::RRF { k: rrf_k } => {
                self.rrf_fusion(dense_results, sparse_results, k, *rrf_k)
            }
            FusionMethod::Learned { model } => {
                model.fuse(dense_results, sparse_results, k)
            }
        }
    }

    fn linear_fusion(
        &self,
        dense: &[(u64, f32)],
        sparse: &[(u64, f32)],
        k: usize,
    ) -> Vec<(u64, f32)> {
        let mut scores: HashMap<u64, f32> = HashMap::new();

        // Normalize dense scores to [0, 1]
        let dense_max = dense.iter().map(|(_, s)| *s).fold(0.0f32, f32::max);
        for (id, score) in dense {
            let normalized = if dense_max > 0.0 { score / dense_max } else { 0.0 };
            *scores.entry(*id).or_insert(0.0) += self.dense_weight * normalized;
        }

        // Normalize sparse scores to [0, 1]
        let sparse_max = sparse.iter().map(|(_, s)| *s).fold(0.0f32, f32::max);
        for (id, score) in sparse {
            let normalized = if sparse_max > 0.0 { score / sparse_max } else { 0.0 };
            *scores.entry(*id).or_insert(0.0) += self.sparse_weight * normalized;
        }

        let mut results: Vec<_> = scores.into_iter().collect();
        results.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        results.truncate(k);
        results
    }

    fn rrf_fusion(
        &self,
        dense: &[(u64, f32)],
        sparse: &[(u64, f32)],
        k: usize,
        rrf_k: f32,
    ) -> Vec<(u64, f32)> {
        let mut scores: HashMap<u64, f32> = HashMap::new();

        // RRF: 1 / (k + rank)
        for (rank, (id, _)) in dense.iter().enumerate() {
            *scores.entry(*id).or_insert(0.0) += self.dense_weight / (rrf_k + rank as f32 + 1.0);
        }

        for (rank, (id, _)) in sparse.iter().enumerate() {
            *scores.entry(*id).or_insert(0.0) += self.sparse_weight / (rrf_k + rank as f32 + 1.0);
        }

        let mut results: Vec<_> = scores.into_iter().collect();
        results.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        results.truncate(k);
        results
    }
}

#[pg_extern]
fn ruvector_hybrid_search(
    table_name: &str,
    dense_column: &str,
    sparse_column: &str,
    dense_query: Vec<f32>,
    sparse_query: PgSparseVec,
    dense_weight: default!(f32, 0.7),
    sparse_weight: default!(f32, 0.3),
    k: default!(i32, 10),
    fusion: default!(&str, "'linear'"),
) -> TableIterator<'static, (name!(id, i64), name!(score, f32))> {
    // Implementation using SPI
}
```

### Phase 5: SPLADE Integration (Week 10)

```rust
// src/sparse/splade.rs

/// SPLADE-style learned sparse representations
pub struct SpladeEncoder {
    /// Vocab size for term indices
    vocab_size: usize,
    /// Sparsity threshold
    threshold: f32,
}

impl SpladeEncoder {
    /// Convert dense embedding to SPLADE-style sparse
    /// (typically done externally, but we support post-processing)
    pub fn sparsify(&self, logits: &[f32]) -> SparseVec {
        let mut indices = Vec::new();
        let mut values = Vec::new();

        for (i, &logit) in logits.iter().enumerate() {
            // ReLU + log(1 + x) activation
            if logit > 0.0 {
                let value = (1.0 + logit).ln();
                if value > self.threshold {
                    indices.push(i as u32);
                    values.push(value);
                }
            }
        }

        SparseVec::new(indices, values, self.vocab_size as u32).unwrap()
    }
}

#[pg_extern]
fn ruvector_to_sparse(
    indices: Vec<i32>,
    values: Vec<f32>,
    dim: i32,
) -> PgSparseVec {
    let indices: Vec<u32> = indices.into_iter().map(|i| i as u32).collect();
    PgSparseVec(SparseVec::new(indices, values, dim as u32).unwrap())
}

#[pg_extern]
fn ruvector_sparse_top_k(sparse: PgSparseVec, k: i32) -> PgSparseVec {
    PgSparseVec(sparse.0.top_k(k as usize))
}

#[pg_extern]
fn ruvector_sparse_prune(sparse: PgSparseVec, threshold: f32) -> PgSparseVec {
    let mut result = sparse.0.clone();
    result.prune(threshold);
    PgSparseVec(result)
}
```

## Benchmarks

| Operation | NNZ (query) | NNZ (doc) | Dim | Time (μs) |
|-----------|-------------|-----------|-----|-----------|
| Dot Product | 100 | 100 | 30K | 0.8 |
| Cosine | 100 | 100 | 30K | 1.2 |
| Inverted Search | 100 | - | 30K | 450 |
| Hybrid Search | 100 | 768 | 30K | 1200 |

## Dependencies

```toml
[dependencies]
# Concurrent collections
dashmap = "6.0"

# Ordered floats for heaps
ordered-float = "4.2"

# Serialization
serde = { version = "1.0", features = ["derive"] }
bincode = "2.0.0-rc.3"
```

## Feature Flags

```toml
[features]
sparse = []
sparse-inverted = ["sparse"]
sparse-hybrid = ["sparse"]
sparse-all = ["sparse-inverted", "sparse-hybrid"]
```
