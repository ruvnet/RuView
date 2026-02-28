use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::collections::HashMap;
use std::time::Duration;

/// Benchmark embedding generation
fn bench_embedding_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding_generation");
    group.measurement_time(Duration::from_secs(8));

    let image_sizes = [(224, 224), (384, 384), (512, 512)];

    for (w, h) in image_sizes {
        let image_data = generate_test_image(w, h);

        group.bench_with_input(
            BenchmarkId::new("generate", format!("{}x{}", w, h)),
            &image_data,
            |b, img| {
                b.iter(|| black_box(generate_embedding(black_box(img))));
            },
        );
    }

    group.finish();
}

/// Benchmark similarity search (vector search)
fn bench_similarity_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("similarity_search");
    group.measurement_time(Duration::from_secs(10));

    // Create cache with varying sizes
    let cache_sizes = [100, 1000, 10000];

    for cache_size in cache_sizes {
        let cache = create_embedding_cache(cache_size);
        let query_embedding = generate_random_embedding(512);

        group.bench_with_input(
            BenchmarkId::new("linear_search", cache_size),
            &(&cache, &query_embedding),
            |b, (cache, query)| {
                b.iter(|| {
                    black_box(linear_similarity_search(
                        black_box(cache),
                        black_box(query),
                        10,
                    ))
                });
            },
        );

        // Approximate nearest neighbor search
        group.bench_with_input(
            BenchmarkId::new("ann_search", cache_size),
            &(&cache, &query_embedding),
            |b, (cache, query)| {
                b.iter(|| {
                    black_box(ann_similarity_search(
                        black_box(cache),
                        black_box(query),
                        10,
                    ))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark cache hit latency
fn bench_cache_hit_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_hit_latency");
    group.measurement_time(Duration::from_secs(5));

    let cache = create_embedding_cache(1000);
    let query = generate_random_embedding(512);

    group.bench_function("exact_match", |b| {
        let cached_embedding = cache.values().next().unwrap();
        b.iter(|| {
            black_box(find_exact_match(
                black_box(&cache),
                black_box(cached_embedding),
            ))
        });
    });

    group.bench_function("similarity_threshold", |b| {
        b.iter(|| {
            black_box(find_by_similarity_threshold(
                black_box(&cache),
                black_box(&query),
                0.95,
            ))
        });
    });

    group.finish();
}

/// Benchmark cache miss latency
fn bench_cache_miss_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_miss_latency");
    group.measurement_time(Duration::from_secs(8));

    let cache = create_embedding_cache(1000);
    let new_image = generate_test_image(384, 384);

    group.bench_function("miss_with_generation", |b| {
        b.iter(|| {
            let query_embedding = generate_embedding(black_box(&new_image));
            let result = linear_similarity_search(black_box(&cache), &query_embedding, 1);
            if result.is_empty() || result[0].1 < 0.95 {
                // Cache miss - would need to process
                black_box(process_new_image(black_box(&new_image)))
            } else {
                black_box(result[0].2.clone())
            }
        });
    });

    group.finish();
}

/// Benchmark cache insertion
fn bench_cache_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_insertion");
    group.measurement_time(Duration::from_secs(8));

    group.bench_function("insert_new_entry", |b| {
        let mut cache = create_embedding_cache(1000);
        let mut counter = 0;

        b.iter(|| {
            let embedding = generate_random_embedding(512);
            let key = format!("key_{}", counter);
            cache.insert(key.clone(), embedding);
            counter += 1;
            black_box(&cache)
        });
    });

    group.bench_function("insert_with_eviction", |b| {
        let mut cache = LRUCache::new(1000);
        let mut counter = 0;

        b.iter(|| {
            let embedding = generate_random_embedding(512);
            let key = format!("key_{}", counter);
            cache.insert(key, embedding);
            counter += 1;
            black_box(&cache)
        });
    });

    group.finish();
}

/// Benchmark cache update operations
fn bench_cache_updates(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_updates");
    group.measurement_time(Duration::from_secs(5));

    let mut cache = create_embedding_cache(1000);
    let keys: Vec<_> = cache.keys().cloned().collect();

    group.bench_function("update_existing", |b| {
        let mut idx = 0;
        b.iter(|| {
            let key = &keys[idx % keys.len()];
            let new_embedding = generate_random_embedding(512);
            cache.insert(key.clone(), new_embedding);
            idx += 1;
            black_box(&cache)
        });
    });

    group.finish();
}

/// Benchmark batch cache operations
fn bench_batch_cache_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_cache_operations");
    group.measurement_time(Duration::from_secs(10));

    let batch_sizes = [10, 50, 100];

    for batch_size in batch_sizes {
        let cache = create_embedding_cache(1000);
        let queries: Vec<_> = (0..batch_size)
            .map(|_| generate_random_embedding(512))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("batch_search", batch_size),
            &(&cache, &queries),
            |b, (cache, queries)| {
                b.iter(|| {
                    let results: Vec<_> = queries
                        .iter()
                        .map(|q| linear_similarity_search(black_box(cache), q, 10))
                        .collect();
                    black_box(results)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("batch_insert", batch_size),
            &queries,
            |b, queries| {
                b.iter_with_setup(
                    || create_embedding_cache(1000),
                    |mut cache| {
                        for (i, embedding) in queries.iter().enumerate() {
                            cache.insert(format!("batch_{}", i), embedding.clone());
                        }
                        black_box(cache)
                    },
                );
            },
        );
    }

    group.finish();
}

/// Benchmark cache statistics and monitoring
fn bench_cache_statistics(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_statistics");
    group.measurement_time(Duration::from_secs(5));

    let cache = create_embedding_cache(10000);

    group.bench_function("compute_stats", |b| {
        b.iter(|| black_box(compute_cache_statistics(black_box(&cache))));
    });

    group.bench_function("memory_usage", |b| {
        b.iter(|| black_box(estimate_cache_memory(black_box(&cache))));
    });

    group.finish();
}

// Mock implementations

type Embedding = Vec<f32>;

struct LRUCache {
    capacity: usize,
    cache: HashMap<String, Embedding>,
    access_order: Vec<String>,
}

impl LRUCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cache: HashMap::new(),
            access_order: Vec::new(),
        }
    }

    fn insert(&mut self, key: String, value: Embedding) {
        if self.cache.len() >= self.capacity && !self.cache.contains_key(&key) {
            if let Some(lru_key) = self.access_order.first().cloned() {
                self.cache.remove(&lru_key);
                self.access_order.remove(0);
            }
        }

        self.cache.insert(key.clone(), value);
        self.access_order.retain(|k| k != &key);
        self.access_order.push(key);
    }
}

fn generate_test_image(width: u32, height: u32) -> Vec<u8> {
    vec![128u8; (width * height * 3) as usize]
}

fn generate_random_embedding(dim: usize) -> Embedding {
    (0..dim).map(|i| (i as f32 * 0.001) % 1.0).collect()
}

fn generate_embedding(image_data: &[u8]) -> Embedding {
    // Simulate embedding generation from image
    let dim = 512;
    let mut embedding = Vec::with_capacity(dim);

    for i in 0..dim {
        let idx = (i * image_data.len() / dim) % image_data.len();
        embedding.push(image_data[idx] as f32 / 255.0);
    }

    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    embedding.iter_mut().for_each(|x| *x /= norm);

    embedding
}

fn create_embedding_cache(size: usize) -> HashMap<String, Embedding> {
    let mut cache = HashMap::new();
    for i in 0..size {
        let embedding = generate_random_embedding(512);
        cache.insert(format!("image_{}", i), embedding);
    }
    cache
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

fn linear_similarity_search(
    cache: &HashMap<String, Embedding>,
    query: &Embedding,
    top_k: usize,
) -> Vec<(String, f32, Embedding)> {
    let mut results: Vec<_> = cache
        .iter()
        .map(|(key, embedding)| {
            let similarity = cosine_similarity(query, embedding);
            (key.clone(), similarity, embedding.clone())
        })
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results.truncate(top_k);
    results
}

fn ann_similarity_search(
    cache: &HashMap<String, Embedding>,
    query: &Embedding,
    top_k: usize,
) -> Vec<(String, f32, Embedding)> {
    // Simplified ANN using random sampling
    let sample_size = (cache.len() / 10).max(100).min(cache.len());
    let mut results: Vec<_> = cache
        .iter()
        .enumerate()
        .filter(|(i, _)| i % (cache.len() / sample_size.max(1)) == 0)
        .map(|(_, (key, embedding))| {
            let similarity = cosine_similarity(query, embedding);
            (key.clone(), similarity, embedding.clone())
        })
        .collect();

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results.truncate(top_k);
    results
}

fn find_exact_match(cache: &HashMap<String, Embedding>, query: &Embedding) -> Option<String> {
    cache.iter().find_map(|(key, embedding)| {
        if embedding.len() == query.len()
            && embedding
                .iter()
                .zip(query.iter())
                .all(|(a, b)| (a - b).abs() < 1e-6)
        {
            Some(key.clone())
        } else {
            None
        }
    })
}

fn find_by_similarity_threshold(
    cache: &HashMap<String, Embedding>,
    query: &Embedding,
    threshold: f32,
) -> Option<(String, f32)> {
    cache
        .iter()
        .filter_map(|(key, embedding)| {
            let similarity = cosine_similarity(query, embedding);
            if similarity >= threshold {
                Some((key.clone(), similarity))
            } else {
                None
            }
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
}

fn process_new_image(_image_data: &[u8]) -> String {
    // Simulate OCR processing
    std::thread::sleep(Duration::from_millis(50));
    "processed_result".to_string()
}

struct CacheStatistics {
    size: usize,
    avg_embedding_norm: f32,
    memory_bytes: usize,
}

fn compute_cache_statistics(cache: &HashMap<String, Embedding>) -> CacheStatistics {
    let size = cache.len();
    let avg_norm = if size > 0 {
        let total_norm: f32 = cache
            .values()
            .map(|emb| emb.iter().map(|x| x * x).sum::<f32>().sqrt())
            .sum();
        total_norm / size as f32
    } else {
        0.0
    };

    let memory_bytes = estimate_cache_memory(cache);

    CacheStatistics {
        size,
        avg_embedding_norm: avg_norm,
        memory_bytes,
    }
}

fn estimate_cache_memory(cache: &HashMap<String, Embedding>) -> usize {
    let key_bytes: usize = cache.keys().map(|k| k.len()).sum();
    let embedding_bytes: usize = cache.values().map(|e| e.len() * 4).sum();
    key_bytes + embedding_bytes + cache.len() * 64 // HashMap overhead
}

criterion_group!(
    benches,
    bench_embedding_generation,
    bench_similarity_search,
    bench_cache_hit_latency,
    bench_cache_miss_latency,
    bench_cache_insertion,
    bench_cache_updates,
    bench_batch_cache_ops,
    bench_cache_statistics
);
criterion_main!(benches);
