//! Comprehensive benchmark suite for the RVF crate family.
//!
//! Measures throughput and latency for wire format, indexing, distance
//! computation, quantization, manifest, runtime, and crypto operations
//! against the acceptance targets in docs/research/rvf/benchmarks/.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// ---------------------------------------------------------------------------
// Deterministic pseudo-random number generator (LCG)
// ---------------------------------------------------------------------------

struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 33) as f32 / (1u64 << 31) as f32
    }

    fn next_f64(&mut self) -> f64 {
        let v = (self.next_u64() >> 33) as f64 / (1u64 << 31) as f64;
        v.clamp(0.001, 0.999)
    }
}

fn make_random_vectors(n: usize, dim: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut rng = Lcg::new(seed);
    (0..n)
        .map(|_| (0..dim).map(|_| rng.next_f32() - 0.5).collect())
        .collect()
}

fn make_random_bytes(size: usize, seed: u64) -> Vec<u8> {
    let mut rng = Lcg::new(seed);
    let mut buf = vec![0u8; size];
    for chunk in buf.chunks_mut(8) {
        let val = rng.next_u64();
        let bytes = val.to_le_bytes();
        let len = chunk.len().min(8);
        chunk[..len].copy_from_slice(&bytes[..len]);
    }
    buf
}

// =========================================================================
// 1. Wire Format Benchmarks
// =========================================================================

fn wire_benchmarks(c: &mut Criterion) {
    use rvf_types::{SegmentFlags, SegmentType};
    use rvf_wire::hash::{compute_crc32c, compute_xxh3_128};
    use rvf_wire::varint::{decode_varint, encode_varint, MAX_VARINT_LEN};
    use rvf_wire::vec_seg_codec::{write_vec_block, VecBlock};
    use rvf_wire::{find_latest_manifest, read_segment, write_segment};

    let mut group = c.benchmark_group("wire");

    // -- segment_write: 1000 vectors, 384-dim, fp16 (2 bytes each) --
    let dim = 384usize;
    let vec_count = 1000usize;
    let bytes_per_vec = dim * 2; // fp16
    let total_payload_size = vec_count * bytes_per_vec;
    let payload = make_random_bytes(total_payload_size, 42);

    group.throughput(Throughput::Bytes(total_payload_size as u64));
    group.bench_function("segment_write_1k_384d_fp16", |b| {
        b.iter(|| {
            black_box(write_segment(
                SegmentType::Vec as u8,
                black_box(&payload),
                SegmentFlags::empty(),
                1,
            ));
        })
    });

    // -- segment_read: parse a VEC_SEG --
    let segment_bytes = write_segment(SegmentType::Vec as u8, &payload, SegmentFlags::empty(), 1);
    group.throughput(Throughput::Bytes(segment_bytes.len() as u64));
    group.bench_function("segment_read_1k_384d_fp16", |b| {
        b.iter(|| {
            black_box(read_segment(black_box(&segment_bytes)).unwrap());
        })
    });

    // -- segment_hash: XXH3-128 of 1MB payload --
    let one_mb = make_random_bytes(1_048_576, 100);
    group.throughput(Throughput::Bytes(1_048_576));
    group.bench_function("xxh3_128_1mb", |b| {
        b.iter(|| {
            black_box(compute_xxh3_128(black_box(&one_mb)));
        })
    });

    // -- crc32c_compute: CRC32C of 1MB payload --
    group.bench_function("crc32c_1mb", |b| {
        b.iter(|| {
            black_box(compute_crc32c(black_box(&one_mb)));
        })
    });

    // -- varint_encode_decode: round-trip 10000 varints --
    let mut rng = Lcg::new(77);
    let varint_values: Vec<u64> = (0..10_000).map(|_| rng.next_u64()).collect();
    group.throughput(Throughput::Elements(10_000));
    group.bench_function("varint_round_trip_10k", |b| {
        b.iter(|| {
            let mut buf = [0u8; MAX_VARINT_LEN];
            for &val in &varint_values {
                let written = encode_varint(val, &mut buf);
                let (decoded, _) = decode_varint(&buf[..written]).unwrap();
                black_box(decoded);
            }
        })
    });

    // -- tail_scan: find manifest in a ~10MB file --
    // Build a synthetic file with a manifest segment at the end.
    let mut file_data = make_random_bytes(10 * 1024 * 1024 - 256, 200);
    let manifest_payload = vec![0u8; 64];
    let manifest_seg = write_segment(
        SegmentType::Manifest as u8,
        &manifest_payload,
        SegmentFlags::empty(),
        99,
    );
    file_data.extend_from_slice(&manifest_seg);
    // Pad to 10MB
    file_data.resize(10 * 1024 * 1024, 0);

    group.throughput(Throughput::Bytes(file_data.len() as u64));
    group.bench_function("tail_scan_10mb", |b| {
        b.iter(|| {
            let _ = black_box(find_latest_manifest(black_box(&file_data)));
        })
    });

    // -- VEC_SEG block write: 1000 vectors, 384-dim f32 --
    let dim_u16 = 384u16;
    let count = 1000u32;
    let mut vec_data = Vec::with_capacity(count as usize * dim_u16 as usize * 4);
    let mut brng = Lcg::new(500);
    for _ in 0..(count as usize * dim_u16 as usize) {
        vec_data.extend_from_slice(&brng.next_f32().to_le_bytes());
    }
    let ids: Vec<u64> = (0..count as u64).collect();
    let block = VecBlock {
        vector_data: vec_data,
        ids,
        dim: dim_u16,
        dtype: 0, // f32
        tier: 0,
    };

    group.bench_function("vec_block_write_1k_384d", |b| {
        b.iter(|| {
            black_box(write_vec_block(black_box(&block)));
        })
    });

    group.finish();
}

// =========================================================================
// 2. Index Benchmarks
// =========================================================================

fn index_benchmarks(c: &mut Criterion) {
    use rvf_index::{
        build_full_index, build_layer_a, build_layer_c, l2_distance, HnswConfig,
        InMemoryVectorStore, ProgressiveIndex,
    };

    let mut group = c.benchmark_group("index");
    group.sample_size(10); // HNSW builds are expensive

    let dim = 384;
    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 200,
    };

    // -- hnsw_build_1k --
    let vecs_1k = make_random_vectors(1000, dim, 42);
    let store_1k = InMemoryVectorStore::new(vecs_1k.clone());
    let mut rng_1k = Lcg::new(123);
    let rng_vals_1k: Vec<f64> = (0..1000).map(|_| rng_1k.next_f64()).collect();

    group.bench_function("hnsw_build_1k_384d", |b| {
        b.iter(|| {
            black_box(build_full_index(
                &store_1k,
                1000,
                &config,
                &rng_vals_1k,
                &l2_distance,
            ));
        })
    });

    // -- hnsw_build_10k --
    let vecs_10k = make_random_vectors(10_000, dim, 99);
    let store_10k = InMemoryVectorStore::new(vecs_10k.clone());
    let mut rng_10k = Lcg::new(456);
    let rng_vals_10k: Vec<f64> = (0..10_000).map(|_| rng_10k.next_f64()).collect();

    group.bench_function("hnsw_build_10k_384d", |b| {
        b.iter(|| {
            black_box(build_full_index(
                &store_10k,
                10_000,
                &config,
                &rng_vals_10k,
                &l2_distance,
            ));
        })
    });

    // Pre-build graphs for search benchmarks
    let graph_1k = build_full_index(&store_1k, 1000, &config, &rng_vals_1k, &l2_distance);
    let graph_10k = build_full_index(&store_10k, 10_000, &config, &rng_vals_10k, &l2_distance);

    // Generate query vectors
    let queries = make_random_vectors(100, dim, 777);

    // -- hnsw_search_1k --
    group.bench_function("hnsw_search_1k_k10", |b| {
        let mut qi = 0usize;
        b.iter(|| {
            let q = &queries[qi % queries.len()];
            qi += 1;
            black_box(graph_1k.search(q, 10, 100, &store_1k, &l2_distance));
        })
    });

    // -- hnsw_search_10k --
    group.bench_function("hnsw_search_10k_k10", |b| {
        let mut qi = 0usize;
        b.iter(|| {
            let q = &queries[qi % queries.len()];
            qi += 1;
            black_box(graph_10k.search(q, 10, 100, &store_10k, &l2_distance));
        })
    });

    // -- progressive_search_layer_a: search with only Layer A --
    let centroids_count = 32usize;
    let centroids: Vec<Vec<f32>> = make_random_vectors(centroids_count, dim, 333);
    let assignments: Vec<u32> = (0..1000).map(|i| (i % centroids_count) as u32).collect();
    let layer_a = build_layer_a(&graph_1k, &centroids, &assignments, 1000);

    let prog_a = ProgressiveIndex {
        layer_a: Some(layer_a),
        layer_b: None,
        layer_c: None,
    };

    group.bench_function("progressive_search_layer_a", |b| {
        let mut qi = 0usize;
        b.iter(|| {
            let q = &queries[qi % queries.len()];
            qi += 1;
            black_box(prog_a.search(q, 10, 100, &store_1k));
        })
    });

    // -- progressive_search_full: search with all layers (Layer C) --
    let layer_c = build_layer_c(&graph_1k);
    let centroids_full: Vec<Vec<f32>> = make_random_vectors(centroids_count, dim, 444);
    let assignments_full: Vec<u32> = (0..1000).map(|i| (i % centroids_count) as u32).collect();
    let layer_a_full = build_layer_a(&graph_1k, &centroids_full, &assignments_full, 1000);

    let prog_full = ProgressiveIndex {
        layer_a: Some(layer_a_full),
        layer_b: None,
        layer_c: Some(layer_c),
    };

    group.bench_function("progressive_search_full", |b| {
        let mut qi = 0usize;
        b.iter(|| {
            let q = &queries[qi % queries.len()];
            qi += 1;
            black_box(prog_full.search(q, 10, 100, &store_1k));
        })
    });

    group.finish();
}

// =========================================================================
// 3. Distance Benchmarks
// =========================================================================

fn distance_benchmarks(c: &mut Criterion) {
    use rvf_index::{cosine_distance, dot_product, l2_distance};

    let mut group = c.benchmark_group("distance");

    for &dim in &[384usize, 768, 1536] {
        let vecs = make_random_vectors(2, dim, dim as u64);
        let a = &vecs[0];
        let b = &vecs[1];

        group.throughput(Throughput::Elements(dim as u64));

        group.bench_with_input(
            BenchmarkId::new("l2", dim),
            &(a.clone(), b.clone()),
            |bench, (a, b)| bench.iter(|| black_box(l2_distance(black_box(a), black_box(b)))),
        );

        if dim == 384 {
            group.bench_with_input(
                BenchmarkId::new("cosine", dim),
                &(a.clone(), b.clone()),
                |bench, (a, b)| {
                    bench.iter(|| black_box(cosine_distance(black_box(a), black_box(b))))
                },
            );

            group.bench_with_input(
                BenchmarkId::new("dot_product", dim),
                &(a.clone(), b.clone()),
                |bench, (a, b)| bench.iter(|| black_box(dot_product(black_box(a), black_box(b)))),
            );
        }
    }

    group.finish();
}

// =========================================================================
// 4. Quantization Benchmarks
// =========================================================================

fn quantization_benchmarks(c: &mut Criterion) {
    use rvf_quant::{
        encode_binary, hamming_distance, CountMinSketch, ProductQuantizer, ScalarQuantizer,
    };

    let mut group = c.benchmark_group("quant");

    let dim = 384;
    let vecs_1k = make_random_vectors(1000, dim, 55);
    let vec_refs: Vec<&[f32]> = vecs_1k.iter().map(|v| v.as_slice()).collect();

    // -- scalar_quant_encode: encode 1000 vectors --
    let sq = ScalarQuantizer::train(&vec_refs);
    group.throughput(Throughput::Elements(1000));

    group.bench_function("scalar_quant_encode_1k", |b| {
        b.iter(|| {
            for v in &vecs_1k {
                black_box(sq.encode_vec(black_box(v)));
            }
        })
    });

    // -- scalar_quant_distance: distance in quantized space (1000 pairs) --
    let encoded: Vec<Vec<u8>> = vecs_1k.iter().map(|v| sq.encode_vec(v)).collect();
    group.throughput(Throughput::Elements(1000));

    group.bench_function("scalar_quant_distance_1k", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let j = (i + 1) % 1000;
                black_box(sq.distance_l2_quantized(black_box(&encoded[i]), black_box(&encoded[j])));
            }
        })
    });

    // -- pq_encode: PQ encode 100 vectors --
    // dim=384, m=48 gives sub_dim=8
    let vecs_100 = make_random_vectors(100, dim, 66);
    let pq_train_refs: Vec<&[f32]> = vecs_1k.iter().map(|v| v.as_slice()).collect();
    let pq = ProductQuantizer::train(&pq_train_refs, 48, 256, 10);

    group.throughput(Throughput::Elements(100));
    group.bench_function("pq_encode_100", |b| {
        b.iter(|| {
            for v in &vecs_100 {
                black_box(pq.encode_vec(black_box(v)));
            }
        })
    });

    // -- pq_adc_distance: ADC distance with precomputed tables --
    let query = &vecs_1k[0];
    let tables = pq.compute_distance_tables(query);
    let pq_codes: Vec<Vec<u8>> = vecs_1k.iter().map(|v| pq.encode_vec(v)).collect();
    group.throughput(Throughput::Elements(1000));

    group.bench_function("pq_adc_distance_1k", |b| {
        b.iter(|| {
            for codes in &pq_codes {
                black_box(ProductQuantizer::distance_adc(
                    black_box(&tables),
                    black_box(codes),
                ));
            }
        })
    });

    // -- binary_encode: binary quantize 1000 vectors --
    group.throughput(Throughput::Elements(1000));
    group.bench_function("binary_encode_1k", |b| {
        b.iter(|| {
            for v in &vecs_1k {
                black_box(encode_binary(black_box(v)));
            }
        })
    });

    // -- hamming_distance: 1000 pairs --
    let binary_codes: Vec<Vec<u8>> = vecs_1k.iter().map(|v| encode_binary(v)).collect();
    group.throughput(Throughput::Elements(1000));

    group.bench_function("hamming_distance_1k", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let j = (i + 1) % 1000;
                black_box(hamming_distance(
                    black_box(&binary_codes[i]),
                    black_box(&binary_codes[j]),
                ));
            }
        })
    });

    // -- sketch_increment: Count-Min Sketch 10000 increments --
    group.throughput(Throughput::Elements(10_000));
    group.bench_function("sketch_increment_10k", |b| {
        b.iter(|| {
            let mut sketch = CountMinSketch::default_sketch();
            for i in 0..10_000u64 {
                sketch.increment(black_box(i));
            }
            black_box(&sketch);
        })
    });

    // -- sketch_estimate: 10000 lookups --
    let mut sketch = CountMinSketch::default_sketch();
    for i in 0..10_000u64 {
        sketch.increment(i);
    }
    group.throughput(Throughput::Elements(10_000));
    group.bench_function("sketch_estimate_10k", |b| {
        b.iter(|| {
            for i in 0..10_000u64 {
                black_box(sketch.estimate(black_box(i)));
            }
        })
    });

    group.finish();
}

// =========================================================================
// 5. Manifest Benchmarks
// =========================================================================

fn manifest_benchmarks(c: &mut Criterion) {
    use rvf_manifest::{boot_phase1, read_level0, write_level0};
    use rvf_types::{
        CentroidPtr, EntrypointPtr, HotCachePtr, Level0Root, PrefetchMapPtr, QuantDictPtr,
        TopLayerPtr, ROOT_MANIFEST_SIZE,
    };

    let mut group = c.benchmark_group("manifest");

    // Build a representative Level 0 root
    let mut root = Level0Root::zeroed();
    root.version = 1;
    root.flags = 0x0004;
    root.l1_manifest_offset = 0x1_0000;
    root.l1_manifest_length = 0x2000;
    root.total_vector_count = 10_000_000;
    root.dimension = 384;
    root.base_dtype = 1;
    root.profile_id = 2;
    root.epoch = 42;
    root.created_ns = 1_700_000_000_000_000_000;
    root.modified_ns = 1_700_000_001_000_000_000;
    root.entrypoint = EntrypointPtr {
        seg_offset: 0x1000,
        block_offset: 64,
        count: 3,
    };
    root.toplayer = TopLayerPtr {
        seg_offset: 0x2000,
        block_offset: 128,
        node_count: 500,
    };
    root.centroid = CentroidPtr {
        seg_offset: 0x3000,
        block_offset: 0,
        count: 256,
    };
    root.quantdict = QuantDictPtr {
        seg_offset: 0x4000,
        block_offset: 0,
        size: 8192,
    };
    root.hot_cache = HotCachePtr {
        seg_offset: 0x5000,
        block_offset: 0,
        vector_count: 1000,
    };
    root.prefetch_map = PrefetchMapPtr {
        offset: 0x6000,
        entries: 200,
        _pad: 0,
    };

    // -- level0_write --
    group.throughput(Throughput::Bytes(ROOT_MANIFEST_SIZE as u64));
    group.bench_function("level0_write", |b| {
        b.iter(|| {
            black_box(write_level0(black_box(&root)));
        })
    });

    // -- level0_read --
    let l0_bytes = write_level0(&root);
    group.throughput(Throughput::Bytes(ROOT_MANIFEST_SIZE as u64));
    group.bench_function("level0_read", |b| {
        b.iter(|| {
            black_box(read_level0(black_box(&l0_bytes)).unwrap());
        })
    });

    // -- boot_phase1: progressive boot Phase 1 on a test file --
    // Minimal file: padding + Level 0 at the tail
    let mut file_data = vec![0u8; 16384];
    let l0_written = write_level0(&root);
    file_data.extend_from_slice(&l0_written);

    group.bench_function("boot_phase1", |b| {
        b.iter(|| {
            black_box(boot_phase1(black_box(&file_data)).unwrap());
        })
    });

    group.finish();
}

// =========================================================================
// 6. Runtime Benchmarks
// =========================================================================

fn runtime_benchmarks(c: &mut Criterion) {
    use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
    use tempfile::TempDir;

    let mut group = c.benchmark_group("runtime");
    group.sample_size(10); // File I/O is expensive

    let dim = 384;

    // -- store_create --
    group.bench_function("store_create", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let path = dir.path().join("bench.rvf");
                (dir, path)
            },
            |(_dir, path)| {
                let options = RvfOptions {
                    dimension: dim as u16,
                    ..Default::default()
                };
                let store = RvfStore::create(&path, options).unwrap();
                store.close().unwrap();
            },
        );
    });

    // -- store_ingest_100 --
    let vecs_100 = make_random_vectors(100, dim, 800);

    group.bench_function("store_ingest_100", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let path = dir.path().join("ingest100.rvf");
                let options = RvfOptions {
                    dimension: dim as u16,
                    ..Default::default()
                };
                let store = RvfStore::create(&path, options).unwrap();
                (dir, store)
            },
            |(_dir, mut store)| {
                let vec_refs: Vec<&[f32]> = vecs_100.iter().map(|v| v.as_slice()).collect();
                let ids: Vec<u64> = (0..100).collect();
                store.ingest_batch(&vec_refs, &ids, None).unwrap();
                store.close().unwrap();
            },
        );
    });

    // -- store_ingest_1000 --
    let vecs_1000 = make_random_vectors(1000, dim, 900);

    group.bench_function("store_ingest_1000", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let path = dir.path().join("ingest1k.rvf");
                let options = RvfOptions {
                    dimension: dim as u16,
                    ..Default::default()
                };
                let store = RvfStore::create(&path, options).unwrap();
                (dir, store)
            },
            |(_dir, mut store)| {
                let vec_refs: Vec<&[f32]> = vecs_1000.iter().map(|v| v.as_slice()).collect();
                let ids: Vec<u64> = (0..1000).collect();
                store.ingest_batch(&vec_refs, &ids, None).unwrap();
                store.close().unwrap();
            },
        );
    });

    // -- store_query_100: query k=10 from 100-vector store --
    let query_vecs = make_random_vectors(20, dim, 1000);

    group.bench_function("store_query_100", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let path = dir.path().join("query100.rvf");
                let options = RvfOptions {
                    dimension: dim as u16,
                    ..Default::default()
                };
                let mut store = RvfStore::create(&path, options).unwrap();
                let vec_refs: Vec<&[f32]> = vecs_100.iter().map(|v| v.as_slice()).collect();
                let ids: Vec<u64> = (0..100).collect();
                store.ingest_batch(&vec_refs, &ids, None).unwrap();
                (dir, store)
            },
            |(_dir, store)| {
                let opts = QueryOptions::default();
                for q in &query_vecs {
                    black_box(store.query(q, 10, &opts).unwrap());
                }
                store.close().unwrap();
            },
        );
    });

    // -- store_query_1000: query k=10 from 1000-vector store --
    group.bench_function("store_query_1000", |b| {
        b.iter_with_setup(
            || {
                let dir = TempDir::new().unwrap();
                let path = dir.path().join("query1k.rvf");
                let options = RvfOptions {
                    dimension: dim as u16,
                    ..Default::default()
                };
                let mut store = RvfStore::create(&path, options).unwrap();
                let vec_refs: Vec<&[f32]> = vecs_1000.iter().map(|v| v.as_slice()).collect();
                let ids: Vec<u64> = (0..1000).collect();
                store.ingest_batch(&vec_refs, &ids, None).unwrap();
                (dir, store)
            },
            |(_dir, store)| {
                let opts = QueryOptions::default();
                for q in &query_vecs {
                    black_box(store.query(q, 10, &opts).unwrap());
                }
                store.close().unwrap();
            },
        );
    });

    group.finish();
}

// =========================================================================
// 7. Crypto Benchmarks
// =========================================================================

fn crypto_benchmarks(c: &mut Criterion) {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    use rvf_crypto::{shake256_256, sign_segment, verify_segment};
    use rvf_types::SegmentHeader;

    let mut group = c.benchmark_group("crypto");

    // -- shake256_1kb --
    let one_kb = make_random_bytes(1024, 300);
    group.throughput(Throughput::Bytes(1024));
    group.bench_function("shake256_1kb", |b| {
        b.iter(|| {
            black_box(shake256_256(black_box(&one_kb)));
        })
    });

    // -- shake256_1mb --
    let one_mb = make_random_bytes(1_048_576, 400);
    group.throughput(Throughput::Bytes(1_048_576));
    group.bench_function("shake256_1mb", |b| {
        b.iter(|| {
            black_box(shake256_256(black_box(&one_mb)));
        })
    });

    // -- ed25519_sign --
    let key = SigningKey::generate(&mut OsRng);
    let header = SegmentHeader::new(0x01, 42);
    let payload = make_random_bytes(4096, 500);

    group.bench_function("ed25519_sign", |b| {
        b.iter(|| {
            black_box(sign_segment(
                black_box(&header),
                black_box(&payload),
                black_box(&key),
            ));
        })
    });

    // -- ed25519_verify --
    let footer = sign_segment(&header, &payload, &key);
    let pubkey = key.verifying_key();

    group.bench_function("ed25519_verify", |b| {
        b.iter(|| {
            black_box(verify_segment(
                black_box(&header),
                black_box(&payload),
                black_box(&footer),
                black_box(&pubkey),
            ));
        })
    });

    group.finish();
}

// =========================================================================
// Criterion Group and Main
// =========================================================================

criterion_group!(
    benches,
    wire_benchmarks,
    index_benchmarks,
    distance_benchmarks,
    quantization_benchmarks,
    manifest_benchmarks,
    runtime_benchmarks,
    crypto_benchmarks,
);

criterion_main!(benches);
