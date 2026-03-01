# Benchmarking Plan

## Overview

Comprehensive benchmarking strategy for ruvector-postgres covering micro-benchmarks, integration tests, comparison with competitors, and production workload simulation.

## Benchmark Categories

### 1. Micro-Benchmarks

Test individual operations in isolation.

```rust
// benches/distance_bench.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_euclidean_distance(c: &mut Criterion) {
    let dims = [128, 256, 512, 768, 1024, 1536];

    let mut group = c.benchmark_group("euclidean_distance");

    for dim in dims {
        let a: Vec<f32> = (0..dim).map(|_| rand::random()).collect();
        let b: Vec<f32> = (0..dim).map(|_| rand::random()).collect();

        group.bench_with_input(
            BenchmarkId::new("scalar", dim),
            &dim,
            |bench, _| bench.iter(|| euclidean_scalar(&a, &b))
        );

        group.bench_with_input(
            BenchmarkId::new("simd_auto", dim),
            &dim,
            |bench, _| bench.iter(|| euclidean_simd(&a, &b))
        );

        #[cfg(target_arch = "x86_64")]
        {
            group.bench_with_input(
                BenchmarkId::new("avx2", dim),
                &dim,
                |bench, _| bench.iter(|| unsafe { euclidean_avx2(&a, &b) })
            );

            if is_x86_feature_detected!("avx512f") {
                group.bench_with_input(
                    BenchmarkId::new("avx512", dim),
                    &dim,
                    |bench, _| bench.iter(|| unsafe { euclidean_avx512(&a, &b) })
                );
            }
        }
    }

    group.finish();
}

fn bench_cosine_distance(c: &mut Criterion) {
    // Similar structure for cosine
}

fn bench_dot_product(c: &mut Criterion) {
    // Similar structure for dot product
}

criterion_group!(
    distance_benches,
    bench_euclidean_distance,
    bench_cosine_distance,
    bench_dot_product
);
criterion_main!(distance_benches);
```

### Expected Results: Distance Functions

| Operation | Dimension | Scalar (ns) | AVX2 (ns) | AVX-512 (ns) | Speedup |
|-----------|-----------|-------------|-----------|--------------|---------|
| Euclidean | 128 | 180 | 45 | 28 | 6.4x |
| Euclidean | 768 | 980 | 210 | 125 | 7.8x |
| Euclidean | 1536 | 1950 | 420 | 245 | 8.0x |
| Cosine | 128 | 240 | 62 | 38 | 6.3x |
| Cosine | 768 | 1280 | 285 | 168 | 7.6x |
| Dot Product | 768 | 450 | 95 | 58 | 7.8x |

### 2. Index Benchmarks

```rust
// benches/index_bench.rs

fn bench_hnsw_build(c: &mut Criterion) {
    let sizes = [10_000, 100_000, 1_000_000];
    let dims = [128, 768];

    let mut group = c.benchmark_group("hnsw_build");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    for size in sizes {
        for dim in dims {
            let vectors = generate_random_vectors(size, dim);

            group.bench_with_input(
                BenchmarkId::new(format!("{}d", dim), size),
                &(&vectors, dim),
                |bench, (vecs, _)| {
                    bench.iter(|| {
                        let mut index = HnswIndex::new(HnswConfig {
                            m: 16,
                            ef_construction: 200,
                            ..Default::default()
                        });
                        for (i, v) in vecs.iter().enumerate() {
                            index.insert(i as u64, v);
                        }
                    })
                }
            );
        }
    }

    group.finish();
}

fn bench_hnsw_search(c: &mut Criterion) {
    // Pre-build index
    let index = build_hnsw_index(1_000_000, 768);
    let queries = generate_random_vectors(1000, 768);

    let ef_values = [10, 50, 100, 200, 500];
    let k_values = [1, 10, 100];

    let mut group = c.benchmark_group("hnsw_search");

    for ef in ef_values {
        for k in k_values {
            group.bench_with_input(
                BenchmarkId::new(format!("ef{}_k{}", ef, k), "1M"),
                &(&index, &queries, ef, k),
                |bench, (idx, qs, ef, k)| {
                    bench.iter(|| {
                        for q in qs.iter() {
                            idx.search(q, *k, *ef);
                        }
                    })
                }
            );
        }
    }

    group.finish();
}

fn bench_ivfflat_search(c: &mut Criterion) {
    let index = build_ivfflat_index(1_000_000, 768, 1000); // 1000 lists
    let queries = generate_random_vectors(1000, 768);

    let probe_values = [1, 5, 10, 20, 50];

    let mut group = c.benchmark_group("ivfflat_search");

    for probes in probe_values {
        group.bench_with_input(
            BenchmarkId::new(format!("probes{}", probes), "1M"),
            &probes,
            |bench, probes| {
                bench.iter(|| {
                    for q in queries.iter() {
                        index.search(q, 10, *probes);
                    }
                })
            }
        );
    }

    group.finish();
}
```

### Expected Results: Index Operations

| Index | Size | Build Time | Memory | Search (p50) | Search (p99) | Recall@10 |
|-------|------|------------|--------|--------------|--------------|-----------|
| HNSW | 100K | 45s | 450MB | 0.8ms | 2.1ms | 0.98 |
| HNSW | 1M | 8min | 4.5GB | 1.2ms | 4.5ms | 0.97 |
| HNSW | 10M | 95min | 45GB | 2.1ms | 8.2ms | 0.96 |
| IVFFlat | 100K | 12s | 320MB | 1.5ms | 4.2ms | 0.92 |
| IVFFlat | 1M | 2min | 3.2GB | 3.2ms | 9.5ms | 0.91 |
| IVFFlat | 10M | 25min | 32GB | 8.5ms | 25ms | 0.89 |

### 3. Quantization Benchmarks

```rust
// benches/quantization_bench.rs

fn bench_quantization_build(c: &mut Criterion) {
    let vectors = generate_random_vectors(100_000, 768);

    let mut group = c.benchmark_group("quantization_build");

    group.bench_function("scalar_q8", |bench| {
        bench.iter(|| ScalarQuantized::from_f32(&vectors))
    });

    group.bench_function("binary", |bench| {
        bench.iter(|| BinaryQuantized::from_f32(&vectors))
    });

    group.bench_function("product_q", |bench| {
        bench.iter(|| ProductQuantized::from_f32(&vectors, 96, 256))
    });

    group.finish();
}

fn bench_quantized_search(c: &mut Criterion) {
    let vectors = generate_random_vectors(1_000_000, 768);
    let query = generate_random_vectors(1, 768).pop().unwrap();

    let sq8 = ScalarQuantized::from_f32(&vectors);
    let binary = BinaryQuantized::from_f32(&vectors);
    let pq = ProductQuantized::from_f32(&vectors, 96, 256);

    let mut group = c.benchmark_group("quantized_search_1M");

    group.bench_function("full_precision", |bench| {
        bench.iter(|| {
            vectors.iter()
                .enumerate()
                .map(|(i, v)| (i, euclidean_distance(&query, v)))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        })
    });

    group.bench_function("scalar_q8", |bench| {
        bench.iter(|| {
            (0..vectors.len())
                .map(|i| (i, sq8.distance(&query, i)))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        })
    });

    group.bench_function("binary_hamming", |bench| {
        let query_bits = binary.quantize_query(&query);
        bench.iter(|| {
            (0..vectors.len())
                .map(|i| (i, binary.hamming_distance(&query_bits, i)))
                .min_by(|a, b| a.1.cmp(&b.1))
        })
    });

    group.finish();
}
```

### Expected Results: Quantization

| Method | Memory (1M 768d) | Search Time | Recall Loss |
|--------|------------------|-------------|-------------|
| Full Precision | 3GB | 850ms | 0% |
| Scalar Q8 | 750MB | 420ms | 1-2% |
| Binary | 94MB | 95ms | 5-10% |
| Product Q | 200MB | 180ms | 2-4% |

### 4. PostgreSQL Integration Benchmarks

```sql
-- Test setup script
CREATE EXTENSION ruvector;

-- Create test table
CREATE TABLE bench_vectors (
    id SERIAL PRIMARY KEY,
    embedding vector(768),
    category TEXT,
    created_at TIMESTAMP DEFAULT NOW()
);

-- Insert test data
INSERT INTO bench_vectors (embedding, category)
SELECT
    array_agg(random())::vector(768),
    'category_' || (i % 100)::text
FROM generate_series(1, 1000000) i
GROUP BY i;

-- Create indexes
CREATE INDEX ON bench_vectors USING hnsw (embedding vector_cosine_ops)
WITH (m = 16, ef_construction = 200);

CREATE INDEX ON bench_vectors USING ivfflat (embedding vector_cosine_ops)
WITH (lists = 1000);

-- Benchmark queries
\timing on

-- Simple k-NN
EXPLAIN ANALYZE
SELECT id, embedding <=> '[...]'::vector AS distance
FROM bench_vectors
ORDER BY distance
LIMIT 10;

-- k-NN with filter
EXPLAIN ANALYZE
SELECT id, embedding <=> '[...]'::vector AS distance
FROM bench_vectors
WHERE category = 'category_42'
ORDER BY distance
LIMIT 10;

-- Batch search
EXPLAIN ANALYZE
SELECT b.id, q.query_id,
       b.embedding <=> q.embedding AS distance
FROM bench_vectors b
CROSS JOIN (
    SELECT 1 AS query_id, '[...]'::vector AS embedding
    UNION ALL
    SELECT 2, '[...]'::vector
    -- ... more queries
) q
ORDER BY q.query_id, distance
LIMIT 100;
```

### 5. Competitor Comparison

```python
# benchmark_comparison.py

import time
import numpy as np
from typing import List, Tuple

# Test data
SIZES = [10_000, 100_000, 1_000_000]
DIMS = [128, 768, 1536]
K = 10
QUERIES = 1000

def run_pgvector_benchmark(conn, size, dim):
    """Benchmark pgvector"""
    # Setup
    conn.execute(f"""
        CREATE TABLE pgvector_test (
            id SERIAL PRIMARY KEY,
            embedding vector({dim})
        );
        CREATE INDEX ON pgvector_test USING hnsw (embedding vector_cosine_ops);
    """)

    # Insert
    start = time.time()
    # ... bulk insert
    build_time = time.time() - start

    # Search
    query = np.random.randn(dim).astype(np.float32)
    start = time.time()
    for _ in range(QUERIES):
        conn.execute(f"""
            SELECT id FROM pgvector_test
            ORDER BY embedding <=> %s
            LIMIT {K}
        """, (query.tolist(),))
    search_time = (time.time() - start) / QUERIES * 1000

    return {
        'build_time': build_time,
        'search_time_ms': search_time,
    }

def run_ruvector_benchmark(conn, size, dim):
    """Benchmark ruvector-postgres"""
    # Similar setup with ruvector
    pass

def run_pinecone_benchmark(index, size, dim):
    """Benchmark Pinecone (cloud)"""
    pass

def run_qdrant_benchmark(client, size, dim):
    """Benchmark Qdrant"""
    pass

def run_milvus_benchmark(collection, size, dim):
    """Benchmark Milvus"""
    pass

# Run all benchmarks
results = {}
for size in SIZES:
    for dim in DIMS:
        results[(size, dim)] = {
            'pgvector': run_pgvector_benchmark(...),
            'ruvector': run_ruvector_benchmark(...),
            'qdrant': run_qdrant_benchmark(...),
            'milvus': run_milvus_benchmark(...),
        }

# Generate comparison report
```

### Expected Comparison Results

| System | 1M Build | 1M Search (p50) | 1M Search (p99) | Memory | Recall@10 |
|--------|----------|-----------------|-----------------|--------|-----------|
| **ruvector-postgres** | **5min** | **0.9ms** | **3.2ms** | **4.2GB** | **0.97** |
| pgvector | 12min | 2.1ms | 8.5ms | 4.8GB | 0.95 |
| Qdrant | 7min | 1.2ms | 4.1ms | 4.5GB | 0.96 |
| Milvus | 8min | 1.5ms | 5.2ms | 5.1GB | 0.96 |
| Pinecone (P1) | 3min* | 5ms* | 15ms* | N/A | 0.98 |

*Cloud latency includes network overhead

### 6. Stress Testing

```bash
#!/bin/bash
# stress_test.sh

# Configuration
DURATION=3600  # 1 hour
CONCURRENCY=100
QPS_TARGET=10000

# Start PostgreSQL with ruvector
pg_ctl start -D $PGDATA

# Run pgbench-style workload
pgbench -c $CONCURRENCY -j 10 -T $DURATION \
    -f stress_queries.sql \
    -P 10 \
    --rate=$QPS_TARGET \
    testdb

# Monitor during test
while true; do
    psql -c "SELECT * FROM ruvector_stats();" >> stats.log
    psql -c "SELECT * FROM pg_stat_activity WHERE state = 'active';" >> activity.log
    sleep 10
done
```

### stress_queries.sql

```sql
-- Mixed workload
\set query_type random(1, 100)

\if :query_type <= 60
    -- 60% simple k-NN
    SELECT id FROM vectors
    ORDER BY embedding <=> :'random_vector'::vector
    LIMIT 10;
\elif :query_type <= 80
    -- 20% filtered k-NN
    SELECT id FROM vectors
    WHERE category = :'random_category'
    ORDER BY embedding <=> :'random_vector'::vector
    LIMIT 10;
\elif :query_type <= 90
    -- 10% batch search
    SELECT v.id, q.id as query_id
    FROM vectors v, query_batch q
    ORDER BY v.embedding <=> q.embedding
    LIMIT 100;
\else
    -- 10% insert
    INSERT INTO vectors (embedding, category)
    VALUES (:'random_vector'::vector, :'random_category');
\endif
```

### 7. Memory Benchmarks

```rust
// benches/memory_bench.rs

fn bench_memory_footprint(c: &mut Criterion) {
    let sizes = [100_000, 1_000_000, 10_000_000];

    println!("\n=== Memory Footprint Analysis ===\n");

    for size in sizes {
        println!("Size: {} vectors", size);

        // Full precision vectors
        let vectors: Vec<Vec<f32>> = generate_random_vectors(size, 768);
        let raw_size = size * 768 * 4;
        println!("  Raw vectors: {} MB", raw_size / 1_000_000);

        // HNSW index
        let hnsw = HnswIndex::new(HnswConfig::default());
        for (i, v) in vectors.iter().enumerate() {
            hnsw.insert(i as u64, v);
        }
        println!("  HNSW overhead: {} MB", hnsw.memory_usage() / 1_000_000);

        // Quantized
        let sq8 = ScalarQuantized::from_f32(&vectors);
        println!("  SQ8 size: {} MB", sq8.memory_usage() / 1_000_000);

        let binary = BinaryQuantized::from_f32(&vectors);
        println!("  Binary size: {} MB", binary.memory_usage() / 1_000_000);

        println!();
    }
}
```

### 8. Recall vs Latency Analysis

```python
# recall_latency_analysis.py

import matplotlib.pyplot as plt
import numpy as np

def measure_recall_latency_tradeoff(index, queries, ground_truth, ef_values):
    """Measure recall vs latency for different ef values"""
    results = []

    for ef in ef_values:
        latencies = []
        recalls = []

        for i, query in enumerate(queries):
            start = time.time()
            results = index.search(query, k=10, ef=ef)
            latency = (time.time() - start) * 1000

            recall = len(set(results) & set(ground_truth[i])) / 10

            latencies.append(latency)
            recalls.append(recall)

        results.append({
            'ef': ef,
            'avg_latency': np.mean(latencies),
            'p99_latency': np.percentile(latencies, 99),
            'avg_recall': np.mean(recalls),
        })

    return results

# Plot results
plt.figure(figsize=(10, 6))
plt.plot([r['avg_latency'] for r in results],
         [r['avg_recall'] for r in results], 'b-o')
plt.xlabel('Latency (ms)')
plt.ylabel('Recall@10')
plt.title('Recall vs Latency Tradeoff')
plt.savefig('recall_latency.png')
```

## Benchmark Automation

### CI/CD Integration

```yaml
# .github/workflows/benchmark.yml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install dependencies
        run: |
          sudo apt-get install postgresql-16
          cargo install cargo-criterion

      - name: Run micro-benchmarks
        run: |
          cargo criterion --output-format json > bench_results.json

      - name: Run PostgreSQL benchmarks
        run: |
          ./scripts/run_pg_benchmarks.sh

      - name: Compare with baseline
        run: |
          python scripts/compare_benchmarks.py \
            --baseline baseline.json \
            --current bench_results.json \
            --threshold 10

      - name: Upload results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: bench_results.json
```

### Benchmark Dashboard

```sql
-- Create benchmark results table
CREATE TABLE benchmark_results (
    id SERIAL PRIMARY KEY,
    run_date TIMESTAMP DEFAULT NOW(),
    git_commit TEXT,
    benchmark_name TEXT,
    metric_name TEXT,
    value FLOAT,
    unit TEXT,
    metadata JSONB
);

-- Query for trend analysis
SELECT
    date_trunc('day', run_date) AS day,
    benchmark_name,
    AVG(value) AS avg_value,
    MIN(value) AS min_value,
    MAX(value) AS max_value
FROM benchmark_results
WHERE metric_name = 'search_latency_p50'
  AND run_date > NOW() - INTERVAL '30 days'
GROUP BY 1, 2
ORDER BY 1, 2;
```

## Reporting Format

### Performance Report Template

```markdown
# RuVector-Postgres Performance Report

**Date:** 2024-XX-XX
**Version:** 0.X.0
**Commit:** abc123

## Summary

- Overall performance: **X% faster** than pgvector
- Memory efficiency: **X% less** than competitors
- Recall@10: **0.97** (target: 0.95)

## Detailed Results

### Index Build Performance
| Size | HNSW Time | IVFFlat Time | Memory |
|------|-----------|--------------|--------|
| 100K | Xs | Xs | XMB |
| 1M | Xm | Xm | XGB |

### Search Latency (1M vectors, 768d)
| Metric | HNSW | IVFFlat | Target |
|--------|------|---------|--------|
| p50 | Xms | Xms | <2ms |
| p99 | Xms | Xms | <10ms |
| QPS | X | X | >5000 |

### Comparison with Competitors
[Charts and tables]

## Recommendations

1. For latency-sensitive workloads: Use HNSW with ef_search=64
2. For memory-constrained: Use IVFFlat with SQ8 quantization
3. For maximum throughput: Enable parallel search with 4 workers
```

## Running Benchmarks

```bash
# Run all micro-benchmarks
cargo bench --features bench

# Run specific benchmark
cargo bench -- distance

# Run PostgreSQL benchmarks
./scripts/run_pg_benchmarks.sh

# Generate comparison report
python scripts/generate_report.py

# Quick smoke test
cargo bench -- --quick
```
