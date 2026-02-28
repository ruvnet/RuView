# RuVector Postgres v2 - Hybrid Search (BM25 + Vector)

## Why Hybrid Search Matters

Vector search finds semantically similar content. Keyword search finds exact matches.

Neither is sufficient alone:
- **Vector-only** misses exact keyword matches (product SKUs, error codes, names)
- **Keyword-only** misses semantic similarity ("car" vs "automobile")

Every production RAG system needs both. pgvector doesn't have this. We do.

---

## Design Goals

1. **Single query, both signals** — No application-level fusion
2. **Configurable blending** — RRF, linear, learned weights
3. **Integrity-aware** — Hybrid index participates in contracted graph
4. **PostgreSQL-native** — Leverages `tsvector` and GIN indexes

---

## Architecture

```
                     +------------------+
                     |   Hybrid Query   |
                     | "error 500 fix"  |
                     +--------+---------+
                              |
              +---------------+---------------+
              |                               |
     +--------v--------+            +---------v---------+
     |  Vector Branch  |            |  Keyword Branch   |
     |  (HNSW/IVF)     |            |  (GIN/tsvector)   |
     +--------+--------+            +---------+---------+
              |                               |
              |  top-100 by cosine            |  top-100 by BM25
              |                               |
              +---------------+---------------+
                              |
                     +--------v--------+
                     |  Fusion Layer   |
                     |  (RRF / Linear) |
                     +--------+--------+
                              |
                     +--------v--------+
                     |  Final top-k    |
                     +--------+--------+
                              |
                     +--------v--------+
                     | Optional Rerank |
                     +-----------------+
```

---

## SQL Interface

### Basic Hybrid Search

```sql
-- Simple hybrid search with default RRF fusion
SELECT * FROM ruvector_hybrid_search(
    'documents',           -- collection name
    query_text := 'database connection timeout error',
    query_vector := $embedding,
    k := 10
);

-- Returns: id, content, vector_score, keyword_score, hybrid_score
```

### Configurable Fusion

```sql
-- RRF (Reciprocal Rank Fusion) - default, robust
SELECT * FROM ruvector_hybrid_search(
    'documents',
    query_text := 'postgres replication lag',
    query_vector := $embedding,
    k := 20,
    fusion := 'rrf',
    rrf_k := 60  -- RRF constant (default 60)
);

-- Linear blend with alpha
SELECT * FROM ruvector_hybrid_search(
    'documents',
    query_text := 'postgres replication lag',
    query_vector := $embedding,
    k := 20,
    fusion := 'linear',
    alpha := 0.7  -- 0.7 * vector + 0.3 * keyword
);

-- Learned fusion weights (from query patterns)
SELECT * FROM ruvector_hybrid_search(
    'documents',
    query_text := 'postgres replication lag',
    query_vector := $embedding,
    k := 20,
    fusion := 'learned'  -- Uses GNN-trained weights
);
```

### Operator Syntax (Advanced)

```sql
-- Using hybrid operator in ORDER BY
SELECT id, content,
       ruvector_hybrid_score(
           embedding <=> $query_vec,
           ts_rank_cd(fts, plainto_tsquery($query_text)),
           alpha := 0.6
       ) AS score
FROM documents
WHERE fts @@ plainto_tsquery($query_text)  -- Pre-filter
   OR embedding <=> $query_vec < 0.5       -- Or similar vectors
ORDER BY score DESC
LIMIT 10;
```

---

## Schema Requirements

### Collection with Hybrid Support

```sql
-- Create table with both vector and FTS columns
CREATE TABLE documents (
    id          BIGSERIAL PRIMARY KEY,
    content     TEXT NOT NULL,
    embedding   vector(1536) NOT NULL,
    fts         tsvector GENERATED ALWAYS AS (to_tsvector('english', content)) STORED,
    metadata    JSONB DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ DEFAULT NOW()
);

-- Vector index
CREATE INDEX idx_documents_embedding
    ON documents USING ruhnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 100);

-- FTS index
CREATE INDEX idx_documents_fts
    ON documents USING gin (fts);

-- Register for hybrid search
SELECT ruvector_register_hybrid(
    collection := 'documents',
    vector_column := 'embedding',
    fts_column := 'fts',
    text_column := 'content'  -- For BM25 stats
);
```

### Hybrid Registration Table

```sql
-- Internal: tracks hybrid-enabled collections
CREATE TABLE ruvector.hybrid_collections (
    id              SERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id),
    vector_column   TEXT NOT NULL,
    fts_column      TEXT NOT NULL,
    text_column     TEXT NOT NULL,

    -- BM25 parameters (computed from corpus)
    avg_doc_length  REAL,
    doc_count       BIGINT,
    k1              REAL DEFAULT 1.2,
    b               REAL DEFAULT 0.75,

    -- Fusion settings
    default_fusion  TEXT DEFAULT 'rrf',
    default_alpha   REAL DEFAULT 0.5,
    learned_weights JSONB,

    -- Stats
    last_stats_update TIMESTAMPTZ,
    created_at      TIMESTAMPTZ DEFAULT NOW()
);
```

---

## BM25 Implementation

### Why Not Just ts_rank?

PostgreSQL's `ts_rank` is not true BM25. It doesn't account for:
- Document length normalization
- IDF weighting across corpus
- Term frequency saturation

We implement proper BM25 in the engine.

### BM25 Scoring

```rust
// src/hybrid/bm25.rs

/// BM25 scorer with corpus statistics
pub struct BM25Scorer {
    k1: f32,           // Term frequency saturation (default 1.2)
    b: f32,            // Length normalization (default 0.75)
    avg_doc_len: f32,  // Average document length
    doc_count: u64,    // Total documents
    idf_cache: HashMap<String, f32>,  // Cached IDF values
}

impl BM25Scorer {
    /// Compute IDF for a term
    fn idf(&self, doc_freq: u64) -> f32 {
        let n = self.doc_count as f32;
        let df = doc_freq as f32;
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    /// Score a document for a query
    pub fn score(&self, doc: &Document, query_terms: &[String]) -> f32 {
        let doc_len = doc.term_count as f32;
        let len_norm = 1.0 - self.b + self.b * (doc_len / self.avg_doc_len);

        query_terms.iter()
            .filter_map(|term| {
                let tf = doc.term_freq(term)? as f32;
                let idf = self.idf_cache.get(term)?;

                // BM25 formula
                let numerator = tf * (self.k1 + 1.0);
                let denominator = tf + self.k1 * len_norm;

                Some(idf * numerator / denominator)
            })
            .sum()
    }
}
```

### Corpus Statistics Update

```sql
-- Update BM25 statistics (run periodically or after bulk inserts)
SELECT ruvector_hybrid_update_stats('documents');

-- Stats stored in hybrid_collections table
-- Computed via background worker or on-demand
```

```rust
// Background worker updates corpus stats
pub fn update_bm25_stats(collection_id: i32) -> Result<(), Error> {
    Spi::run(|client| {
        // Get average document length
        let avg_len: f64 = client.select(
            "SELECT AVG(LENGTH(content)) FROM documents",
            None, &[]
        )?.first().unwrap().get(1)?;

        // Get document count
        let doc_count: i64 = client.select(
            "SELECT COUNT(*) FROM documents",
            None, &[]
        )?.first().unwrap().get(1)?;

        // Update term frequencies (using tsvector stats)
        // ... compute IDF cache ...

        client.update(
            "UPDATE ruvector.hybrid_collections
             SET avg_doc_length = $1, doc_count = $2, last_stats_update = NOW()
             WHERE collection_id = $3",
            None,
            &[avg_len.into(), doc_count.into(), collection_id.into()]
        )
    })
}
```

---

## Fusion Algorithms

### Reciprocal Rank Fusion (RRF)

Default and most robust. Works without score calibration.

```rust
// src/hybrid/fusion.rs

/// RRF fusion: score = sum(1 / (k + rank_i))
pub fn rrf_fusion(
    vector_results: &[(DocId, f32)],  // (id, distance)
    keyword_results: &[(DocId, f32)], // (id, bm25_score)
    k: usize,                          // RRF constant (default 60)
    limit: usize,
) -> Vec<(DocId, f32)> {
    let mut scores: HashMap<DocId, f32> = HashMap::new();

    // Vector ranking (lower distance = higher rank)
    for (rank, (doc_id, _)) in vector_results.iter().enumerate() {
        *scores.entry(*doc_id).or_default() += 1.0 / (k + rank + 1) as f32;
    }

    // Keyword ranking (higher BM25 = higher rank)
    for (rank, (doc_id, _)) in keyword_results.iter().enumerate() {
        *scores.entry(*doc_id).or_default() += 1.0 / (k + rank + 1) as f32;
    }

    // Sort by fused score
    let mut results: Vec<_> = scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results.truncate(limit);
    results
}
```

### Linear Fusion

Simple weighted combination. Requires score normalization.

```rust
/// Linear fusion: score = alpha * vec_score + (1 - alpha) * kw_score
pub fn linear_fusion(
    vector_results: &[(DocId, f32)],
    keyword_results: &[(DocId, f32)],
    alpha: f32,
    limit: usize,
) -> Vec<(DocId, f32)> {
    // Normalize vector scores (convert distance to similarity)
    let vec_scores = normalize_to_similarity(vector_results);

    // Normalize BM25 scores to [0, 1]
    let kw_scores = min_max_normalize(keyword_results);

    // Combine
    let mut combined: HashMap<DocId, f32> = HashMap::new();

    for (doc_id, score) in vec_scores {
        *combined.entry(doc_id).or_default() += alpha * score;
    }

    for (doc_id, score) in kw_scores {
        *combined.entry(doc_id).or_default() += (1.0 - alpha) * score;
    }

    let mut results: Vec<_> = combined.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results.truncate(limit);
    results
}
```

### Learned Fusion

Uses query characteristics to select weights dynamically.

```rust
/// Learned fusion using GNN-predicted weights
pub fn learned_fusion(
    query_embedding: &[f32],
    query_terms: &[String],
    vector_results: &[(DocId, f32)],
    keyword_results: &[(DocId, f32)],
    model: &FusionModel,
    limit: usize,
) -> Vec<(DocId, f32)> {
    // Query features
    let features = QueryFeatures {
        embedding_norm: l2_norm(query_embedding),
        term_count: query_terms.len(),
        avg_term_idf: compute_avg_idf(query_terms),
        has_exact_match: detect_exact_match_intent(query_terms),
        query_type: classify_query_type(query_terms),  // navigational, informational, etc.
    };

    // Predict optimal alpha for this query
    let alpha = model.predict_alpha(&features);

    linear_fusion(vector_results, keyword_results, alpha, limit)
}
```

---

## Integrity Integration

Hybrid search participates in the integrity control plane.

### Contracted Graph Nodes

```sql
-- Hybrid index adds nodes to contracted graph
INSERT INTO ruvector.contracted_graph (collection_id, node_type, node_id, node_name, health_score)
SELECT
    c.id,
    'hybrid_index',
    h.id,
    'hybrid_' || c.name,
    CASE
        WHEN h.last_stats_update > NOW() - INTERVAL '1 day' THEN 1.0
        WHEN h.last_stats_update > NOW() - INTERVAL '7 days' THEN 0.7
        ELSE 0.3  -- Stale stats degrade health
    END
FROM ruvector.hybrid_collections h
JOIN ruvector.collections c ON h.collection_id = c.id;
```

### Integrity-Aware Hybrid Search

```rust
/// Hybrid search with integrity gating
pub fn hybrid_search_with_integrity(
    collection_id: i32,
    query: &HybridQuery,
) -> Result<Vec<HybridResult>, Error> {
    // Check integrity gate
    let gate = check_integrity_gate(collection_id, "hybrid_search");

    match gate.state {
        IntegrityState::Normal => {
            // Full hybrid: both branches
            execute_full_hybrid(query)
        }
        IntegrityState::Stress => {
            // Degrade gracefully: prefer faster branch
            if query.alpha > 0.5 {
                // Vector-heavy query: use vector only
                execute_vector_only(query)
            } else {
                // Keyword-heavy query: use keyword only
                execute_keyword_only(query)
            }
        }
        IntegrityState::Critical => {
            // Minimal: keyword only (cheapest)
            execute_keyword_only(query)
        }
    }
}
```

---

## Performance Optimization

### Pre-filtering Strategy

```sql
-- Hybrid search with pre-filter (faster for selective filters)
SELECT * FROM ruvector_hybrid_search(
    'documents',
    query_text := 'error handling',
    query_vector := $embedding,
    k := 10,
    filter := 'category = ''backend'' AND created_at > NOW() - INTERVAL ''30 days'''
);
```

```rust
// Execution strategy selection
fn choose_strategy(filter_selectivity: f32, corpus_size: u64) -> HybridStrategy {
    if filter_selectivity < 0.01 {
        // Very selective: pre-filter, then hybrid on small set
        HybridStrategy::PreFilter
    } else if filter_selectivity < 0.1 && corpus_size > 1_000_000 {
        // Moderately selective, large corpus: hybrid first, post-filter
        HybridStrategy::PostFilter
    } else {
        // Not selective: full hybrid
        HybridStrategy::Full
    }
}
```

### Parallel Execution

```rust
/// Execute vector and keyword branches in parallel
pub async fn parallel_hybrid(query: &HybridQuery) -> HybridResults {
    let (vector_results, keyword_results) = tokio::join!(
        execute_vector_branch(&query.embedding, query.prefetch_k),
        execute_keyword_branch(&query.text, query.prefetch_k),
    );

    fuse_results(vector_results, keyword_results, query.fusion, query.k)
}
```

### Caching

```rust
/// Cache BM25 scores for repeated terms
pub struct HybridCache {
    term_doc_scores: LruCache<(String, DocId), f32>,
    idf_cache: HashMap<String, f32>,
    ttl: Duration,
}
```

---

## Configuration

### GUC Parameters

```sql
-- Default fusion method
SET ruvector.hybrid_fusion = 'rrf';  -- 'rrf', 'linear', 'learned'

-- Default alpha for linear fusion
SET ruvector.hybrid_alpha = 0.5;

-- RRF constant
SET ruvector.hybrid_rrf_k = 60;

-- Prefetch size for each branch
SET ruvector.hybrid_prefetch_k = 100;

-- Enable parallel branch execution
SET ruvector.hybrid_parallel = true;
```

### Per-Collection Settings

```sql
SELECT ruvector_hybrid_configure('documents', '{
    "default_fusion": "learned",
    "prefetch_k": 200,
    "bm25_k1": 1.5,
    "bm25_b": 0.8,
    "stats_refresh_interval": "1 hour"
}'::jsonb);
```

---

## Monitoring

```sql
-- Hybrid search statistics
SELECT * FROM ruvector_hybrid_stats('documents');

-- Returns:
-- {
--   "total_searches": 15234,
--   "avg_vector_latency_ms": 4.2,
--   "avg_keyword_latency_ms": 2.1,
--   "avg_fusion_latency_ms": 0.3,
--   "cache_hit_rate": 0.67,
--   "last_stats_update": "2024-01-15T10:30:00Z",
--   "corpus_size": 1250000,
--   "avg_doc_length": 542
-- }
```

---

## Testing Requirements

### Correctness Tests
- BM25 scoring matches reference implementation
- RRF fusion produces expected rankings
- Linear fusion respects alpha parameter
- Learned fusion adapts to query type

### Performance Tests
- Hybrid search < 2x single-branch latency
- Parallel execution shows speedup
- Cache hit rate > 50% for repeated queries

### Integration Tests
- Integrity degradation triggers graceful fallback
- Stats update doesn't block queries
- Large corpus (10M+ docs) scales

---

## Example: RAG Application

```sql
-- Complete RAG retrieval with hybrid search
WITH retrieved AS (
    SELECT
        id,
        content,
        hybrid_score,
        metadata
    FROM ruvector_hybrid_search(
        'knowledge_base',
        query_text := $user_question,
        query_vector := $question_embedding,
        k := 5,
        fusion := 'rrf',
        filter := 'status = ''published'''
    )
)
SELECT
    string_agg(content, E'\n\n---\n\n') AS context,
    array_agg(id) AS source_ids
FROM retrieved;

-- Pass context to LLM for answer generation
```
