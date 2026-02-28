# RVF Acceptance Tests and Performance Targets

## 1. Primary Acceptance Test

> **Cold start on a 10 million vector file: load and answer the first query with a
> useful result (recall@10 >= 0.70) without reading more than the last 4 MB, then
> converge to full quality (recall@10 >= 0.95) as it progressively maps more segments.**

### Test Parameters

```
Dataset:         10 million vectors
Dimensions:      384 (sentence embedding size)
Base dtype:      fp16 (768 bytes per vector)
Raw file size:   ~7.2 GB (vectors only)
With index:      ~10-12 GB total
Query set:       1000 queries from held-out test set
Ground truth:    Brute-force exact k-NN (k=10)
Metric:          L2 distance
```

### Success Criteria

| Phase | Time Budget | Data Read | Min Recall@10 | Description |
|-------|------------|-----------|---------------|-------------|
| Boot | < 5 ms | 4 KB (Level 0) | N/A | Parse root manifest |
| First query | < 50 ms | <= 4 MB | >= 0.70 | Layer A + hot cache |
| Working quality | < 500 ms | <= 200 MB | >= 0.85 | Layer A + B |
| Full quality | < 5 s | <= 4 GB | >= 0.95 | Layers A + B + C |
| Optimized | < 30 s | Full file | >= 0.98 | All layers + hot tier |

### Measurement Methodology

```
1. Create RVF file from 10M vector dataset
   - Build full HNSW index (M=16, ef_construction=200)
   - Compute temperature tiers (default: all warm initially)
   - Write with all segment types

2. Cold start measurement
   - Drop filesystem cache: echo 3 > /proc/sys/vm/drop_caches
   - Open file, start timer
   - Read Level 0 (4 KB), record time T_boot
   - Read hotset data, record time T_hotset
   - Execute first query, record time T_first_query and recall@10
   - Continue progressive loading
   - At each milestone: record time, data read, recall@10

3. Throughput measurement (warm)
   - After full load, execute 1000 queries
   - Measure queries per second (QPS)
   - Measure p50, p95, p99 latency
   - Measure recall@10 average

4. Streaming ingest measurement
   - Start with empty file
   - Ingest 10M vectors in streaming mode
   - Measure ingest rate (vectors/second)
   - Measure file size over time
   - Verify crash safety (kill -9 at random points, verify recovery)
```

## 2. Performance Targets

### Query Latency (10M vectors, 384 dim, fp16)

| Hardware | QPS (single thread) | p50 Latency | p95 Latency | p99 Latency |
|----------|-------------------|-------------|-------------|-------------|
| Desktop (AVX-512) | 5,000-15,000 | 0.1 ms | 0.3 ms | 1.0 ms |
| Desktop (AVX2) | 3,000-8,000 | 0.2 ms | 0.5 ms | 2.0 ms |
| Laptop (NEON) | 2,000-5,000 | 0.3 ms | 1.0 ms | 3.0 ms |
| WASM (browser) | 500-2,000 | 1.0 ms | 3.0 ms | 10.0 ms |
| Cognitum tile | 100-500 | 2.0 ms | 5.0 ms | 15.0 ms |

### Streaming Ingest Rate

| Hardware | Vectors/Second | Bytes/Second | Notes |
|----------|---------------|-------------|-------|
| NVMe SSD | 200K-500K | 150-380 MB/s | fsync every 1000 vectors |
| SATA SSD | 50K-100K | 38-76 MB/s | fsync every 1000 vectors |
| HDD | 10K-30K | 7-23 MB/s | Sequential append |
| Network (1 Gbps) | 50K-100K | 38-76 MB/s | Streaming over network |

### Progressive Load Times

| Phase | NVMe SSD | SATA SSD | HDD | Network |
|-------|----------|----------|-----|---------|
| Boot (4 KB) | < 0.1 ms | < 0.5 ms | < 10 ms | < 50 ms |
| First query (4 MB) | < 2 ms | < 10 ms | < 100 ms | < 500 ms |
| Working quality (200 MB) | < 100 ms | < 500 ms | < 5 s | < 20 s |
| Full quality (4 GB) | < 2 s | < 10 s | < 120 s | < 400 s |

### Space Efficiency

| Configuration | Bytes/Vector | File Size (10M) | Ratio vs Raw |
|--------------|-------------|-----------------|-------------|
| Raw fp32 | 1,536 | 14.3 GB | 1.0x |
| RVF uniform fp16 | 768 + overhead | 8.0 GB | 0.56x |
| RVF adaptive (equilibrium) | ~300 avg | 3.2 GB | 0.22x |
| RVF aggressive (binary cold) | ~100 avg | 1.1 GB | 0.08x |

## 3. Crash Safety Tests

### Test 1: Kill During Vector Ingest

```
1. Start ingesting 1M vectors
2. After 500K vectors: kill -9 the writer
3. Verify: file is readable
4. Verify: latest valid manifest is found
5. Verify: all vectors referenced by latest manifest are intact
6. Verify: no data corruption (all segment hashes valid)
```

**Pass criteria**: Zero data loss for committed segments. At most the
last incomplete segment is lost (bounded by fsync interval).

### Test 2: Kill During Manifest Write

```
1. Create file with 1M vectors
2. Trigger manifest rewrite (add metadata, trigger compaction)
3. Kill -9 during manifest write
4. Verify: file falls back to previous valid manifest
5. Verify: all queries work correctly with previous manifest
```

**Pass criteria**: Automatic fallback to previous manifest. No manual
recovery needed.

### Test 3: Kill During Compaction

```
1. Create file with 1M vectors across 100 small VEC_SEGs
2. Trigger compaction
3. Kill -9 during compaction
4. Verify: file is readable (old segments still valid)
5. Verify: partial compaction output is safely ignored
```

**Pass criteria**: Old segments remain valid. Incomplete compaction
output has no manifest reference and is safely orphaned.

### Test 4: Bit Flip Detection

```
1. Create valid RVF file
2. Flip random bits in various locations
3. Verify: corruption detected by hash/CRC checks
4. Verify: specific corrupted segment identified
5. Verify: other segments still readable
```

**Pass criteria**: 100% detection of single-bit flips. Corruption
isolated to affected segment.

## 4. Scalability Tests

### Test: 1 Billion Vectors

```
Dataset:     1B vectors, 384 dimensions, fp16
File size:   ~700 GB (raw) -> ~200 GB (adaptive RVF)
Hardware:    Server with 256 GB RAM, NVMe array

Verify:
  - Boot time < 10 ms
  - First query < 100 ms
  - Full quality convergence < 60 s
  - Recall@10 >= 0.95 at full quality
  - Streaming ingest sustained at 100K+ vectors/second
```

### Test: High Dimensionality

```
Dataset:     1M vectors, 4096 dimensions (LLM embeddings)
File size:   ~8 GB (fp16)

Verify:
  - PQ compression to 5-bit achieves >= 10x compression
  - Recall@10 >= 0.90 with PQ
  - Query latency < 5 ms (p95) with PQ + HNSW
```

### Test: Multi-File Sharding

```
Dataset:     100M vectors across 10 shard files
Verify:
  - Transparent query across all shards
  - Shard addition without full rebuild
  - Individual shard compaction
  - Shard removal with manifest update only
```

## 5. WASM Performance Tests

### Browser Environment

```
Runtime:     Chrome V8 / Firefox SpiderMonkey
SIMD:        WASM v128
Memory:      Limited to 4 GB WASM heap

Test: Load 1M vector RVF file via fetch()
  - Boot time < 50 ms
  - First query < 200 ms (after boot)
  - QPS >= 500 (single thread)
  - Memory usage < 500 MB
```

### Cognitum Tile Simulation

```
Runtime:     wasmtime with memory limits
Code limit:  8 KB
Data limit:  8 KB
Scratch:     64 KB

Test: Process 1000 blocks via hub protocol
  - Distance computation matches reference implementation
  - Top-K results match brute-force within quantization tolerance
  - No memory access out of bounds
  - Tile recovers from simulated faults
```

## 6. Interoperability Tests

### Round-Trip Test

```
1. Create RVF file from numpy arrays
2. Read back with independent implementation
3. Verify: all vectors bit-identical
4. Verify: all metadata preserved
5. Verify: index produces same results
```

### Profile Compatibility Test

```
1. Create RVDNA file with genomic data
2. Create RVText file with text embeddings
3. Read both with generic RVF reader
4. Verify: generic reader can access vectors and metadata
5. Verify: profile-specific features degrade gracefully
```

### Version Forward Compatibility Test

```
1. Create RVF file with version 1
2. Add segments with hypothetical version 2 features (unknown tags)
3. Read with version 1 reader
4. Verify: version 1 reader skips unknown segments/tags
5. Verify: version 1 data is fully accessible
```

## 7. Security Tests

### Signature Verification

```
1. Create signed RVF file (ML-DSA-65)
2. Verify all segment signatures
3. Modify one byte in a signed segment
4. Verify: modification detected
5. Verify: other segments still valid
```

### Encryption Round-Trip

```
1. Create encrypted RVF file (ML-KEM-768 + AES-256-GCM)
2. Decrypt with correct key
3. Verify: plaintext matches original
4. Attempt decrypt with wrong key
5. Verify: decryption fails (GCM auth tag mismatch)
```

### Key Rotation

```
1. Create file signed with key A
2. Rotate to key B (write CRYPTO_SEG rotation record)
3. Write new segments signed with key B
4. Verify: old segments valid with key A
5. Verify: new segments valid with key B
6. Verify: cross-signature in rotation record is valid
```

## 8. Benchmark Harness

### Recommended Tools

| Purpose | Tool | Notes |
|---------|------|-------|
| Latency measurement | criterion (Rust) / benchmark.js | Statistical rigor |
| Recall measurement | Custom recall@K computation | Against brute-force ground truth |
| Memory profiling | valgrind massif / Chrome DevTools | Peak and sustained |
| I/O profiling | blktrace / iostat | Verify read patterns |
| SIMD verification | Intel SDE / ARM emulator | Correct SIMD codegen |
| Crash testing | Custom harness with kill -9 | Random timing |

### Report Format

Each benchmark run produces a report:

```json
{
  "test_name": "cold_start_10m",
  "dataset": {
    "vector_count": 10000000,
    "dimensions": 384,
    "dtype": "fp16",
    "file_size_bytes": 10737418240
  },
  "hardware": {
    "cpu": "Intel Xeon w5-3435X",
    "simd": "AVX-512",
    "ram_gb": 256,
    "storage": "NVMe Samsung 990 Pro"
  },
  "results": {
    "boot_ms": 0.08,
    "first_query_ms": 12.3,
    "first_query_recall_at_10": 0.73,
    "working_quality_ms": 340,
    "working_quality_recall_at_10": 0.87,
    "full_quality_ms": 3200,
    "full_quality_recall_at_10": 0.96,
    "steady_state_qps": 8500,
    "steady_state_p50_ms": 0.12,
    "steady_state_p95_ms": 0.28,
    "steady_state_p99_ms": 0.85,
    "data_read_first_query_mb": 3.2,
    "data_read_working_quality_mb": 180
  }
}
```
