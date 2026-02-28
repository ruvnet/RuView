# ADR-004: Performance Optimization Strategy

## Status

Proposed

## Date

2026-01-15

## Context

7sense is a bioacoustics platform that processes bird call audio using Perch 2.0 embeddings (1536-D vectors from 5-second audio segments at 32kHz) stored in a RuVector-based system with HNSW indexing and GNN learning capabilities. The system must handle:

- **Scale**: 1M+ bird call embeddings with sub-100ms query latency
- **Continuous Learning**: GNN refinement without blocking query operations
- **Hierarchical Data**: Poincare ball hyperbolic embeddings for species/call-type taxonomies
- **Real-time Ingestion**: Streaming audio from field sensors

This ADR defines the performance optimization strategy to meet these requirements while maintaining system reliability and cost efficiency.

## Decision

We adopt a multi-layered performance optimization approach covering HNSW tuning, embedding quantization, batch processing, memory management, caching, GNN scheduling, and horizontal scalability.

---

## 1. HNSW Parameter Tuning

### 1.1 Core Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| **M** (max connections per node) | 32 | Optimal for 1536-D vectors; balances recall vs memory. Higher than default (16) due to high dimensionality. |
| **efConstruction** | 200 | Build-time search depth. Higher ensures quality graph structure for dense embedding spaces. |
| **efSearch** | 128 (default) / 256 (high-recall) | Query-time search depth. Tunable per query based on precision requirements. |
| **maxLevel** | auto (log2(N)/log2(M)) | Automatically determined; ~6-7 levels for 1M vectors with M=32. |

### 1.2 Dimensionality-Specific Adjustments

```
For 1536-D Perch embeddings:
- Use L2 distance (Euclidean) for normalized vectors
- Consider Product Quantization (PQ) for memory reduction (see Section 2)
- Enable SIMD acceleration (AVX-512 where available)
```

### 1.3 Benchmark Targets

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Recall@10 | >= 0.95 | Compare against brute-force ground truth |
| Recall@100 | >= 0.98 | Same |
| Query Latency (p50) | < 10ms | Single-threaded, 1M vectors |
| Query Latency (p99) | < 50ms | Under concurrent load |
| Build Time | < 30 min | For 1M vectors cold start |

### 1.4 Tuning Protocol

```typescript
interface HNSWTuningConfig {
  // Phase 1: Initial calibration (10K sample)
  calibration: {
    sampleSize: 10000,
    mRange: [16, 24, 32, 48],
    efConstructionRange: [100, 200, 400],
    targetRecall: 0.95
  },

  // Phase 2: Full index build with optimal params
  production: {
    m: 32,  // Determined from calibration
    efConstruction: 200,
    efSearchDefault: 128,
    efSearchHighRecall: 256
  },

  // Phase 3: Runtime adaptation
  adaptive: {
    enableDynamicEf: true,
    efFloor: 64,
    efCeiling: 512,
    latencyTarget: 50  // ms
  }
}
```

---

## 2. Embedding Quantization Strategy

### 2.1 Tiered Storage Architecture

```
HOT TIER (Active Queries)
-------------------------
- Format: float32 (full precision)
- Size: 1536 * 4 = 6,144 bytes/vector
- Capacity: ~100K vectors (600MB RAM)
- Use: Real-time queries, recent recordings

WARM TIER (Frequent Access)
---------------------------
- Format: float16 (half precision)
- Size: 1536 * 2 = 3,072 bytes/vector
- Capacity: ~500K vectors (1.5GB RAM)
- Use: Weekly active data, popular species

COLD TIER (Archive)
-------------------
- Format: int8 (scalar quantization)
- Size: 1536 * 1 = 1,536 bytes/vector
- Capacity: ~2M+ vectors (3GB disk)
- Use: Historical data, rare species
```

### 2.2 Quantization Methods

| Method | Compression | Recall Impact | Use Case |
|--------|-------------|---------------|----------|
| **Scalar (int8)** | 4x | -2-3% recall | Cold storage, bulk search |
| **Product Quantization (PQ)** | 8-16x | -3-5% recall | Very large archives |
| **Binary** | 32x | -10-15% recall | First-pass filtering only |

### 2.3 Scalar Quantization Implementation

```typescript
class ScalarQuantizer {
  // Per-dimension min/max for calibration
  private mins: Float32Array;
  private maxs: Float32Array;
  private scales: Float32Array;

  calibrate(embeddings: Float32Array[], sampleSize: number = 10000): void {
    // Sample random embeddings for range estimation
    const sample = this.randomSample(embeddings, sampleSize);

    for (let d = 0; d < 1536; d++) {
      const values = sample.map(e => e[d]);
      this.mins[d] = Math.min(...values);
      this.maxs[d] = Math.max(...values);
      this.scales[d] = 255 / (this.maxs[d] - this.mins[d]);
    }
  }

  quantize(embedding: Float32Array): Uint8Array {
    const quantized = new Uint8Array(1536);
    for (let d = 0; d < 1536; d++) {
      const normalized = (embedding[d] - this.mins[d]) * this.scales[d];
      quantized[d] = Math.round(Math.max(0, Math.min(255, normalized)));
    }
    return quantized;
  }

  dequantize(quantized: Uint8Array): Float32Array {
    const embedding = new Float32Array(1536);
    for (let d = 0; d < 1536; d++) {
      embedding[d] = (quantized[d] / this.scales[d]) + this.mins[d];
    }
    return embedding;
  }
}
```

### 2.4 Promotion/Demotion Policy

```
PROMOTION (Cold -> Warm -> Hot)
-------------------------------
Trigger: Query frequency > threshold OR explicit prefetch
- Cold -> Warm: 5+ queries in 24h
- Warm -> Hot: 20+ queries in 1h

DEMOTION (Hot -> Warm -> Cold)
------------------------------
Trigger: Time-based decay OR memory pressure
- Hot -> Warm: No queries in 1h
- Warm -> Cold: No queries in 7d
- LRU eviction when tier exceeds capacity
```

---

## 3. Batch Processing Pipeline

### 3.1 Audio Ingestion Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     AUDIO INGESTION PIPELINE                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  [Sensors]  ──>  [Buffer Queue]  ──>  [Segment Detector]           │
│      │               │                      │                       │
│      │          (5min chunks)         (5s windows)                  │
│      v               v                      v                       │
│  [Raw Storage]  [Batch Accumulator]  [Perch Embedder]              │
│                      │                      │                       │
│                 (1000 segments)        (GPU batch)                  │
│                      v                      v                       │
│              [Embedding Queue]  <──  [1536-D vectors]              │
│                      │                                              │
│                      v                                              │
│              [HNSW Batch Insert]                                   │
│                      │                                              │
│              (async, non-blocking)                                  │
│                      v                                              │
│              [Index + Metadata Store]                              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Batch Sizing Parameters

| Stage | Batch Size | Latency Target | Throughput |
|-------|------------|----------------|------------|
| Audio buffer | 5 min chunks | < 1s queue delay | 100+ streams |
| Segment detection | 100 segments | < 500ms | 1000 segments/s |
| Perch embedding | 64 segments | < 2s GPU | 32 segments/s/GPU |
| HNSW insertion | 1000 vectors | < 100ms | 10K vectors/s |
| Metadata write | 1000 records | < 50ms | 20K records/s |

### 3.3 Backpressure Handling

```typescript
interface BackpressureConfig {
  // Queue depth thresholds
  warningThreshold: 10000,    // Start logging warnings
  throttleThreshold: 50000,   // Reduce intake rate
  dropThreshold: 100000,      // Drop lowest-priority data

  // Priority levels for graceful degradation
  priorities: {
    critical: 'endangered_species',      // Never drop
    high: 'known_species_new_recording', // Drop last
    normal: 'routine_monitoring',        // Standard handling
    low: 'background_noise_samples'      // Drop first
  },

  // Rate limiting
  maxIngestionRate: 10000,  // segments/minute
  burstAllowance: 5000,     // temporary overflow
}
```

### 3.4 Batch Insert Optimization

```typescript
async function batchInsertEmbeddings(
  embeddings: Float32Array[],
  metadata: EmbeddingMetadata[],
  config: BatchConfig
): Promise<BatchResult> {
  const batchSize = config.batchSize || 1000;
  const results: BatchResult = { inserted: 0, failed: 0, latencyMs: [] };

  // Sort by expected cluster for better cache locality
  const sorted = sortByClusterHint(embeddings, metadata);

  for (let i = 0; i < sorted.length; i += batchSize) {
    const batch = sorted.slice(i, i + batchSize);
    const start = performance.now();

    // Parallel insert with connection pooling
    await Promise.all([
      hnswIndex.batchAdd(batch.embeddings),
      metadataStore.batchInsert(batch.metadata)
    ]);

    results.latencyMs.push(performance.now() - start);
    results.inserted += batch.length;
  }

  return results;
}
```

---

## 4. Memory Management

### 4.1 Streaming vs Batch Trade-offs

| Mode | Memory Footprint | Latency | Use Case |
|------|------------------|---------|----------|
| **Streaming** | O(window_size) ~50MB | Real-time (<1s) | Live monitoring |
| **Micro-batch** | O(batch_size) ~200MB | Near-real-time (<5s) | Standard ingestion |
| **Batch** | O(full_batch) ~2GB | Minutes | Bulk historical import |

### 4.2 Memory Budget Allocation

```
TOTAL MEMORY BUDGET: 16GB (single node)
=======================================

HNSW Index (Hot):     4GB  (25%)
  - ~650K float32 vectors
  - Navigation structure overhead

Embedding Cache:      3GB  (19%)
  - LRU cache for frequent queries
  - Warm tier spillover

GNN Model:            2GB  (12%)
  - Model parameters
  - Gradient buffers
  - Activation cache

Query Buffers:        2GB  (12%)
  - Concurrent query working memory
  - Result aggregation

Ingestion Pipeline:   2GB  (12%)
  - Audio processing buffers
  - Batch accumulation

Metadata/Index:       2GB  (12%)
  - SQLite/RocksDB buffers
  - B-tree indices

OS/Overhead:          1GB  (6%)
  - System requirements
  - Safety margin
```

### 4.3 Memory Pressure Response

```typescript
interface MemoryManager {
  thresholds: {
    warning: 0.75,    // 75% utilization
    critical: 0.90,   // 90% utilization
    emergency: 0.95   // 95% utilization
  },

  responses: {
    warning: [
      'reduce_batch_sizes',
      'increase_demotion_rate',
      'log_memory_profile'
    ],
    critical: [
      'pause_gnn_training',
      'aggressive_cache_eviction',
      'reject_low_priority_queries'
    ],
    emergency: [
      'stop_ingestion',
      'force_checkpoint',
      'alert_operations'
    ]
  }
}
```

### 4.4 Zero-Copy Optimizations

```typescript
// Use memory-mapped files for large read-only data
const coldTierIndex = mmap('/data/cold_embeddings.bin', {
  mode: 'readonly',
  advice: MADV_RANDOM  // Optimize for random access
});

// Share embedding buffers between query threads
const sharedQueryBuffer = new SharedArrayBuffer(
  QUERY_BATCH_SIZE * EMBEDDING_DIM * 4
);

// Avoid copies in pipeline stages
function processSegment(audio: AudioBuffer): EmbeddingResult {
  // Pass views, not copies
  const spectrogram = computeMelSpectrogram(audio.subarray(0, WINDOW_SIZE));
  const embedding = perchModel.embed(spectrogram);  // Returns view
  return { embedding, metadata: extractMetadata(audio) };
}
```

---

## 5. Caching Strategy

### 5.1 Multi-Level Cache Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                     CACHE HIERARCHY                            │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  L1: Query Result Cache (100MB)                               │
│  ├── Key: hash(query_embedding + search_params)               │
│  ├── TTL: 5 minutes                                           │
│  ├── Hit Rate Target: 40%+ for repeated queries               │
│  └── Eviction: LRU with frequency boost                       │
│                                                                │
│  L2: Nearest Neighbor Cache (500MB)                           │
│  ├── Key: embedding_id                                        │
│  ├── Value: precomputed k-NN list                             │
│  ├── TTL: 1 hour (invalidate on index update)                 │
│  └── Hit Rate Target: 60%+ for hot embeddings                 │
│                                                                │
│  L3: Cluster Centroid Cache (200MB)                           │
│  ├── Key: cluster_id                                          │
│  ├── Value: centroid + exemplar embeddings                    │
│  ├── TTL: 24 hours                                            │
│  └── Use: Fast cluster assignment for new embeddings          │
│                                                                │
│  L4: Metadata Cache (300MB)                                   │
│  ├── Key: embedding_id                                        │
│  ├── Value: species, location, timestamp, etc.                │
│  ├── TTL: None (invalidate on update)                         │
│  └── Hit Rate Target: 90%+ (frequently accessed)              │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### 5.2 Cache Warming Strategy

```typescript
interface CacheWarmingConfig {
  // Startup warming
  startup: {
    // Load most queried embeddings from past 24h
    recentQueryEmbeddings: 10000,
    // Load all cluster centroids
    clusterCentroids: 'all',
    // Load endangered species data
    prioritySpecies: ['species_list_from_config']
  },

  // Predictive warming
  predictive: {
    // Time-based patterns
    schedules: [
      { time: '05:00', action: 'warm_dawn_chorus_species' },
      { time: '19:00', action: 'warm_dusk_species' }
    ],
    // Geographic patterns
    sensorActivation: {
      triggerRadius: '50km',
      preloadNeighborSites: true
    }
  },

  // Query-driven warming
  queryDriven: {
    // On any query, prefetch neighbors' neighbors
    prefetchDepth: 2,
    prefetchCount: 10
  }
}
```

### 5.3 Cache Invalidation

```typescript
// Event-driven invalidation
const cacheInvalidator = {
  onEmbeddingInsert(id: string, embedding: Float32Array): void {
    // Invalidate affected NN caches
    const affectedNeighbors = hnswIndex.getNeighbors(id, 50);
    affectedNeighbors.forEach(nid => nnCache.invalidate(nid));

    // Invalidate cluster centroid if significantly different
    const cluster = clusterAssignment.get(id);
    if (cluster && distanceFromCentroid(embedding, cluster) > threshold) {
      centroidCache.invalidate(cluster.id);
    }
  },

  onClusterUpdate(clusterId: string): void {
    // Invalidate centroid and all member NN caches
    centroidCache.invalidate(clusterId);
    const members = clusterMembers.get(clusterId);
    members.forEach(mid => nnCache.invalidate(mid));
  },

  onGNNTrainingComplete(): void {
    // Embeddings may have shifted - invalidate distance-based caches
    nnCache.clear();
    queryCache.clear();
    // Centroid cache can remain (recomputed lazily)
  }
}
```

---

## 6. GNN Training Schedule

### 6.1 Training Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    GNN TRAINING PIPELINE                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ONLINE LEARNING (Continuous)                                      │
│  ├── Trigger: Every 1000 new embeddings                            │
│  ├── Scope: Local neighborhood refinement                          │
│  ├── Duration: < 100ms (non-blocking)                              │
│  └── Method: Single GNN message-passing step                       │
│                                                                     │
│  INCREMENTAL TRAINING (Scheduled)                                  │
│  ├── Trigger: Hourly (off-peak) or 10K new embeddings              │
│  ├── Scope: Updated subgraph (new nodes + 2-hop neighbors)         │
│  ├── Duration: 1-5 minutes                                         │
│  └── Method: 3-5 GNN epochs on affected subgraph                   │
│                                                                     │
│  FULL RETRAINING (Periodic)                                        │
│  ├── Trigger: Weekly (Sunday 02:00-06:00) or manual                │
│  ├── Scope: Entire graph                                           │
│  ├── Duration: 1-4 hours                                           │
│  └── Method: Full GNN training with hyperparameter tuning          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 6.2 Non-Blocking Training Protocol

```typescript
class GNNTrainingScheduler {
  private queryPriorityLock = new AsyncLock();

  async onlineUpdate(newEmbeddings: EmbeddingBatch): Promise<void> {
    // Non-blocking: runs in background, doesn't affect queries
    setImmediate(async () => {
      const subgraph = this.extractLocalSubgraph(newEmbeddings);
      await this.gnn.singleStep(subgraph);
    });
  }

  async incrementalTrain(): Promise<TrainingResult> {
    // Acquire read lock (queries continue, writes pause)
    await this.queryPriorityLock.acquireRead();

    try {
      const updatedSubgraph = this.getUpdatedSubgraph();
      const result = await this.gnn.train(updatedSubgraph, {
        epochs: 5,
        earlyStop: { patience: 2, minDelta: 0.001 }
      });

      // Apply updates atomically
      await this.applyEmbeddingUpdates(result.refinedEmbeddings);
      return result;
    } finally {
      this.queryPriorityLock.release();
    }
  }

  async fullRetrain(): Promise<TrainingResult> {
    // Acquire write lock (pause all operations)
    await this.queryPriorityLock.acquireWrite();

    try {
      // Checkpoint current state for rollback
      await this.checkpoint();

      const result = await this.gnn.fullTrain({
        epochs: 50,
        learningRate: 0.001,
        earlyStop: { patience: 10, minDelta: 0.0001 }
      });

      // Validate before applying
      if (result.validationRecall < 0.90) {
        await this.rollback();
        throw new Error('Training degraded recall, rolled back');
      }

      await this.applyFullUpdate(result);
      return result;
    } finally {
      this.queryPriorityLock.release();
    }
  }
}
```

### 6.3 Training Resource Allocation

```
OFF-PEAK (02:00-06:00 local time)
---------------------------------
- Full retraining allowed
- 100% GPU utilization
- Query latency SLA relaxed to 200ms

PEAK HOURS (06:00-22:00)
------------------------
- Online updates only
- GPU limited to 20% for training
- Query latency SLA: 50ms p99

TRANSITION PERIODS
------------------
- Incremental training allowed
- GPU limited to 50% for training
- Query latency SLA: 100ms p99
```

---

## 7. Benchmarking Framework

### 7.1 Benchmark Suite

```typescript
interface BenchmarkSuite {
  // Core HNSW benchmarks
  hnsw: {
    insertThroughput: {
      description: 'Vectors inserted per second',
      target: '>= 10,000 vectors/s',
      dataset: '1M random 1536-D vectors'
    },
    queryLatency: {
      description: 'Single query latency distribution',
      targets: {
        p50: '<= 10ms',
        p95: '<= 30ms',
        p99: '<= 50ms'
      },
      dataset: '1M indexed, 10K queries'
    },
    recallAtK: {
      description: 'Recall compared to brute force',
      targets: {
        recall10: '>= 0.95',
        recall100: '>= 0.98'
      }
    },
    concurrentQueries: {
      description: 'Throughput under concurrent load',
      target: '>= 1,000 QPS at p99 < 100ms',
      concurrency: [1, 10, 50, 100, 200]
    }
  },

  // End-to-end pipeline benchmarks
  pipeline: {
    audioToEmbedding: {
      description: 'Full audio processing latency',
      target: '<= 200ms per 5s segment',
      includeIO: true
    },
    ingestionThroughput: {
      description: 'Sustained ingestion rate',
      target: '>= 100 segments/second',
      duration: '1 hour'
    },
    queryWithMetadata: {
      description: 'Query + metadata fetch',
      target: '<= 75ms p99'
    }
  },

  // GNN-specific benchmarks
  gnn: {
    onlineUpdateLatency: {
      description: 'Single-step GNN update',
      target: '<= 100ms for 1K node subgraph'
    },
    incrementalTrainTime: {
      description: 'Hourly incremental training',
      target: '<= 5 minutes for 10K updates'
    },
    recallImprovement: {
      description: 'Recall gain from GNN refinement',
      target: '>= 2% improvement over baseline HNSW'
    }
  },

  // Memory benchmarks
  memory: {
    indexMemoryPerVector: {
      description: 'Memory per indexed vector',
      target: '<= 8KB (including overhead)'
    },
    cacheHitRate: {
      description: 'Cache effectiveness',
      targets: {
        queryCache: '>= 30%',
        nnCache: '>= 50%',
        metadataCache: '>= 80%'
      }
    },
    quantizationRecallLoss: {
      description: 'Recall loss from int8 quantization',
      target: '<= 3%'
    }
  }
}
```

### 7.2 SLA Definitions

| Operation | p50 | p95 | p99 | p99.9 |
|-----------|-----|-----|-----|-------|
| kNN Query (k=10) | 5ms | 20ms | 50ms | 100ms |
| kNN Query (k=100) | 10ms | 40ms | 80ms | 150ms |
| Range Query (r<0.5) | 15ms | 50ms | 100ms | 200ms |
| Insert Single | 1ms | 5ms | 10ms | 20ms |
| Batch Insert (1000) | 50ms | 100ms | 200ms | 500ms |
| Cluster Assignment | 20ms | 50ms | 100ms | 200ms |
| Full Pipeline (audio->result) | 200ms | 500ms | 1000ms | 2000ms |

### 7.3 Continuous Benchmarking

```yaml
# .github/workflows/performance.yml
benchmark_schedule:
  nightly:
    - hnsw_insert_throughput
    - hnsw_query_latency
    - hnsw_recall

  weekly:
    - full_pipeline_benchmark
    - gnn_training_benchmark
    - memory_pressure_test

  on_release:
    - all_benchmarks
    - scalability_test_10M
    - longevity_test_24h

regression_thresholds:
  latency_increase: 10%   # Alert if p99 increases by 10%
  throughput_decrease: 5% # Alert if QPS drops by 5%
  recall_decrease: 1%     # Alert if recall drops by 1%
```

---

## 8. Horizontal Scalability

### 8.1 Sharding Strategy

```
SHARDING APPROACH: Geographic + Temporal Hybrid
===============================================

Primary Shard Key: Geographic Region (sensor cluster)
- Shard 0: North America West
- Shard 1: North America East
- Shard 2: Europe
- Shard 3: Asia-Pacific
- Shard 4: South America
- Shard 5: Africa

Secondary Partition: Temporal (within shard)
- Hot: Current month
- Warm: Past 12 months
- Cold: Archive (>12 months)

Cross-Shard Queries:
- Use scatter-gather pattern
- Merge results by distance
- Timeout per shard: 100ms
```

### 8.2 Cluster Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    DISTRIBUTED ARCHITECTURE                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐           │
│   │  Query LB   │    │  Query LB   │    │  Query LB   │           │
│   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘           │
│          │                  │                  │                   │
│          v                  v                  v                   │
│   ┌─────────────────────────────────────────────────────┐         │
│   │               Query Router (Consistent Hash)         │         │
│   └─────────────────────────┬───────────────────────────┘         │
│                             │                                      │
│      ┌──────────────────────┼──────────────────────┐              │
│      │                      │                      │              │
│      v                      v                      v              │
│ ┌─────────┐           ┌─────────┐           ┌─────────┐          │
│ │ Shard 0 │           │ Shard 1 │           │ Shard N │          │
│ │ (3 rep) │           │ (3 rep) │           │ (3 rep) │          │
│ └────┬────┘           └────┬────┘           └────┬────┘          │
│      │                     │                     │                │
│      v                     v                     v                │
│ ┌─────────┐           ┌─────────┐           ┌─────────┐          │
│ │  HNSW   │           │  HNSW   │           │  HNSW   │          │
│ │  Index  │           │  Index  │           │  Index  │          │
│ └─────────┘           └─────────┘           └─────────┘          │
│                                                                   │
│   ┌─────────────────────────────────────────────────────┐        │
│   │          Shared Metadata Store (Distributed)         │        │
│   └─────────────────────────────────────────────────────┘        │
│                                                                   │
│   ┌─────────────────────────────────────────────────────┐        │
│   │       GNN Training Coordinator (Async Gossip)        │        │
│   └─────────────────────────────────────────────────────┘        │
│                                                                   │
└─────────────────────────────────────────────────────────────────────┘
```

### 8.3 Scaling Thresholds

| Metric | Scale-Out Trigger | Scale-In Trigger |
|--------|-------------------|------------------|
| Query Latency p99 | > 80ms sustained 5min | < 30ms sustained 1h |
| CPU Utilization | > 70% sustained 5min | < 30% sustained 1h |
| Memory Utilization | > 80% | < 50% sustained 1h |
| Queue Depth | > 10K pending | < 1K sustained 30min |
| Shard Size | > 500K vectors | N/A (don't scale in) |

### 8.4 Cross-Shard Query Protocol

```typescript
async function globalKnnQuery(
  query: Float32Array,
  k: number,
  options: QueryOptions
): Promise<SearchResult[]> {
  const shards = options.shards || getAllShards();
  const perShardK = Math.ceil(k * 1.5);  // Over-fetch for merge

  // Scatter phase
  const shardPromises = shards.map(shard =>
    queryShardWithTimeout(shard, query, perShardK, options.timeout || 100)
  );

  // Gather phase with partial results on timeout
  const results = await Promise.allSettled(shardPromises);

  // Merge and re-rank
  const allResults = results
    .filter(r => r.status === 'fulfilled')
    .flatMap(r => r.value);

  // Sort by distance and take top-k
  allResults.sort((a, b) => a.distance - b.distance);
  return allResults.slice(0, k);
}
```

---

## 9. Latency Budget Breakdown

### 9.1 Query Path Latency Budget

```
TOTAL BUDGET: 50ms (p99 target)
===============================

┌────────────────────────────────────────────────────┐
│ Component                  │ Budget  │ % of Total │
├────────────────────────────────────────────────────┤
│ Network (client -> LB)     │  5ms    │   10%      │
│ Load Balancer routing      │  1ms    │    2%      │
│ Query parsing/validation   │  1ms    │    2%      │
│ Cache lookup (L1-L4)       │  3ms    │    6%      │
│ HNSW search (k=10)         │ 25ms    │   50%      │
│ Metadata fetch             │  5ms    │   10%      │
│ Result serialization       │  2ms    │    4%      │
│ Network (LB -> client)     │  5ms    │   10%      │
│ Buffer/headroom            │  3ms    │    6%      │
├────────────────────────────────────────────────────┤
│ TOTAL                      │ 50ms    │  100%      │
└────────────────────────────────────────────────────┘
```

### 9.2 Ingestion Path Latency Budget

```
TOTAL BUDGET: 200ms (p99 target for single segment)
==================================================

┌────────────────────────────────────────────────────┐
│ Component                  │ Budget  │ % of Total │
├────────────────────────────────────────────────────┤
│ Audio receive/decode       │ 10ms    │    5%      │
│ Mel spectrogram compute    │ 20ms    │   10%      │
│ Perch model inference      │ 80ms    │   40%      │
│ Embedding normalization    │  5ms    │    2%      │
│ HNSW insertion             │ 20ms    │   10%      │
│ Metadata write             │ 10ms    │    5%      │
│ Cache invalidation         │ 10ms    │    5%      │
│ Acknowledgment             │  5ms    │    2%      │
│ Buffer/headroom            │ 40ms    │   20%      │
├────────────────────────────────────────────────────┤
│ TOTAL                      │200ms    │  100%      │
└────────────────────────────────────────────────────┘
```

### 9.3 GNN Training Latency Constraints

```
ONLINE UPDATE: 100ms max (non-blocking)
---------------------------------------
- Subgraph extraction: 20ms
- Single GNN forward pass: 50ms
- Embedding update (async): 30ms

INCREMENTAL TRAINING: 5 min max
-------------------------------
- Subgraph construction: 30s
- Training (5 epochs): 4 min
- Embedding sync: 30s

FULL RETRAINING: 4 hour max
---------------------------
- Graph snapshot: 10 min
- Training (50 epochs): 3.5 hours
- Validation: 10 min
- Cutover: 10 min
```

---

## Consequences

### Positive

- **Sub-100ms query latency** achieved through HNSW tuning and multi-level caching
- **4x storage reduction** for cold data via int8 scalar quantization
- **Non-blocking GNN learning** enables continuous improvement without query degradation
- **Linear horizontal scaling** via geographic sharding
- **Clear SLAs** enable capacity planning and alerting

### Negative

- **Increased operational complexity** from multi-tier storage and distributed architecture
- **Memory overhead** from caching layers (~1.1GB dedicated to caches)
- **Quantization recall loss** of 2-3% for cold tier data
- **Cross-shard query overhead** adds latency for global searches

### Neutral

- **Trade-off flexibility** allows tuning precision vs. latency per use case
- **Benchmark-driven development** requires ongoing measurement infrastructure

---

## Implementation Phases

### Phase 1: Foundation (Weeks 1-2)
- Implement HNSW with tuned parameters
- Set up benchmark suite
- Establish baseline metrics

### Phase 2: Optimization (Weeks 3-4)
- Implement scalar quantization
- Add multi-level caching
- Optimize batch ingestion pipeline

### Phase 3: Learning (Weeks 5-6)
- Integrate GNN training scheduler
- Implement non-blocking updates
- Validate recall improvements

### Phase 4: Scale (Weeks 7-8)
- Implement sharding layer
- Deploy distributed architecture
- Load test at 1M+ vectors

---

## References

- Perch 2.0: https://arxiv.org/abs/2508.04665
- RuVector: https://github.com/ruvnet/ruvector
- HNSW Paper: Malkov & Yashunin, 2018
- Product Quantization: Jegou et al., 2011
- Graph Attention Networks: Velickovic et al., 2018
