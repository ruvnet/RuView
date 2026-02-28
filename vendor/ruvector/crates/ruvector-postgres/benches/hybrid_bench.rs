//! Hybrid search benchmarks
//!
//! Benchmarks for combining vector search with keyword/BM25 scoring:
//! - Vector-only vs hybrid latency
//! - BM25 scoring overhead
//! - Fusion algorithm comparison (RRF, weighted sum)
//! - Parallel branch execution gain

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

// ============================================================================
// BM25 Implementation
// ============================================================================

mod bm25 {
    use std::cmp::Ordering;
    use std::collections::HashMap;

    /// Simple tokenizer
    pub fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 2)
            .map(|s| s.to_string())
            .collect()
    }

    /// BM25 scoring index
    pub struct BM25Index {
        /// Document frequency for each term
        pub doc_freq: HashMap<String, usize>,
        /// Term frequency per document
        pub term_freq: Vec<HashMap<String, usize>>,
        /// Document lengths
        pub doc_lengths: Vec<usize>,
        /// Average document length
        pub avg_doc_len: f64,
        /// Number of documents
        pub num_docs: usize,
        /// BM25 parameters
        pub k1: f64,
        pub b: f64,
    }

    impl BM25Index {
        pub fn new(k1: f64, b: f64) -> Self {
            Self {
                doc_freq: HashMap::new(),
                term_freq: Vec::new(),
                doc_lengths: Vec::new(),
                avg_doc_len: 0.0,
                num_docs: 0,
                k1,
                b,
            }
        }

        pub fn build(&mut self, documents: &[String]) {
            self.num_docs = documents.len();
            self.term_freq = Vec::with_capacity(documents.len());
            self.doc_lengths = Vec::with_capacity(documents.len());

            let mut total_len = 0usize;

            for doc in documents {
                let tokens = tokenize(doc);
                self.doc_lengths.push(tokens.len());
                total_len += tokens.len();

                let mut tf: HashMap<String, usize> = HashMap::new();
                let mut seen_terms: std::collections::HashSet<String> =
                    std::collections::HashSet::new();

                for token in tokens {
                    *tf.entry(token.clone()).or_insert(0) += 1;

                    if !seen_terms.contains(&token) {
                        *self.doc_freq.entry(token.clone()).or_insert(0) += 1;
                        seen_terms.insert(token);
                    }
                }

                self.term_freq.push(tf);
            }

            self.avg_doc_len = total_len as f64 / documents.len() as f64;
        }

        /// Calculate IDF for a term
        fn idf(&self, term: &str) -> f64 {
            let df = self.doc_freq.get(term).copied().unwrap_or(0) as f64;
            if df == 0.0 {
                return 0.0;
            }
            ((self.num_docs as f64 - df + 0.5) / (df + 0.5) + 1.0).ln()
        }

        /// Score a document against a query
        pub fn score(&self, doc_id: usize, query_tokens: &[String]) -> f64 {
            if doc_id >= self.term_freq.len() {
                return 0.0;
            }

            let doc_tf = &self.term_freq[doc_id];
            let doc_len = self.doc_lengths[doc_id] as f64;

            let mut score = 0.0;

            for term in query_tokens {
                let tf = doc_tf.get(term).copied().unwrap_or(0) as f64;
                if tf == 0.0 {
                    continue;
                }

                let idf = self.idf(term);
                let numerator = tf * (self.k1 + 1.0);
                let denominator =
                    tf + self.k1 * (1.0 - self.b + self.b * (doc_len / self.avg_doc_len));

                score += idf * (numerator / denominator);
            }

            score
        }

        /// Search and return top-k documents
        pub fn search(&self, query: &str, k: usize) -> Vec<(usize, f64)> {
            let query_tokens = tokenize(query);

            let mut scores: Vec<(usize, f64)> = (0..self.num_docs)
                .map(|doc_id| (doc_id, self.score(doc_id, &query_tokens)))
                .filter(|(_, score)| *score > 0.0)
                .collect();

            scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
            scores.truncate(k);
            scores
        }
    }
}

// ============================================================================
// Vector Search (Simplified)
// ============================================================================

mod vector_search {
    use std::cmp::Ordering;

    pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    pub fn search(vectors: &[Vec<f32>], query: &[f32], k: usize) -> Vec<(usize, f32)> {
        let mut results: Vec<(usize, f32)> = vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (i, euclidean_distance(query, v)))
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        results.truncate(k);
        results
    }

    pub fn search_parallel(vectors: &[Vec<f32>], query: &[f32], k: usize) -> Vec<(usize, f32)> {
        use rayon::prelude::*;

        let mut results: Vec<(usize, f32)> = vectors
            .par_iter()
            .enumerate()
            .map(|(i, v)| (i, euclidean_distance(query, v)))
            .collect();

        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        results.truncate(k);
        results
    }
}

// ============================================================================
// Fusion Algorithms
// ============================================================================

mod fusion {
    use std::collections::HashMap;

    /// Reciprocal Rank Fusion
    pub fn rrf(
        vector_results: &[(usize, f32)],
        text_results: &[(usize, f64)],
        k: usize,
        rrf_k: f64,
    ) -> Vec<(usize, f64)> {
        let mut scores: HashMap<usize, f64> = HashMap::new();

        // Vector results
        for (rank, (doc_id, _)) in vector_results.iter().enumerate() {
            let rrf_score = 1.0 / (rrf_k + rank as f64 + 1.0);
            *scores.entry(*doc_id).or_insert(0.0) += rrf_score;
        }

        // Text results
        for (rank, (doc_id, _)) in text_results.iter().enumerate() {
            let rrf_score = 1.0 / (rrf_k + rank as f64 + 1.0);
            *scores.entry(*doc_id).or_insert(0.0) += rrf_score;
        }

        let mut results: Vec<(usize, f64)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(k);
        results
    }

    /// Weighted score fusion (requires normalized scores)
    pub fn weighted_sum(
        vector_results: &[(usize, f32)],
        text_results: &[(usize, f64)],
        k: usize,
        vector_weight: f64,
        text_weight: f64,
    ) -> Vec<(usize, f64)> {
        // Normalize vector scores (lower distance = higher score)
        let max_dist = vector_results
            .iter()
            .map(|(_, d)| *d)
            .fold(0.0f32, f32::max);
        let vector_scores: HashMap<usize, f64> = vector_results
            .iter()
            .map(|(id, dist)| (*id, (1.0 - dist / max_dist.max(1e-6)) as f64))
            .collect();

        // Normalize text scores
        let max_text = text_results.iter().map(|(_, s)| *s).fold(0.0f64, f64::max);
        let text_scores: HashMap<usize, f64> = text_results
            .iter()
            .map(|(id, score)| (*id, score / max_text.max(1e-6)))
            .collect();

        // Combine
        let mut all_ids: std::collections::HashSet<usize> = std::collections::HashSet::new();
        all_ids.extend(vector_scores.keys());
        all_ids.extend(text_scores.keys());

        let mut results: Vec<(usize, f64)> = all_ids
            .iter()
            .map(|&id| {
                let v_score = vector_scores.get(&id).copied().unwrap_or(0.0);
                let t_score = text_scores.get(&id).copied().unwrap_or(0.0);
                (id, vector_weight * v_score + text_weight * t_score)
            })
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(k);
        results
    }

    /// Disjunctive Normalization
    pub fn disjunctive_normalization(
        vector_results: &[(usize, f32)],
        text_results: &[(usize, f64)],
        k: usize,
    ) -> Vec<(usize, f64)> {
        let mut scores: HashMap<usize, f64> = HashMap::new();

        // Vector results (convert distance to similarity)
        let max_dist = vector_results
            .iter()
            .map(|(_, d)| *d)
            .fold(0.0f32, f32::max);
        for (doc_id, dist) in vector_results {
            let sim = 1.0 - (*dist / max_dist.max(1e-6)) as f64;
            scores.insert(*doc_id, sim);
        }

        // Text results (add if not present, max if present)
        let max_text = text_results.iter().map(|(_, s)| *s).fold(0.0f64, f64::max);
        for (doc_id, score) in text_results {
            let norm_score = score / max_text.max(1e-6);
            let current = scores.entry(*doc_id).or_insert(0.0);
            *current = current.max(norm_score);
        }

        let mut results: Vec<(usize, f64)> = scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(k);
        results
    }
}

use bm25::{tokenize, BM25Index};
use fusion::{disjunctive_normalization, rrf, weighted_sum};
use vector_search::{search as vector_search_fn, search_parallel as vector_search_parallel};

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_random_vectors(n: usize, dims: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..n)
        .map(|_| (0..dims).map(|_| rng.gen_range(-1.0..1.0)).collect())
        .collect()
}

fn generate_random_documents(n: usize, seed: u64) -> Vec<String> {
    let words = [
        "machine",
        "learning",
        "artificial",
        "intelligence",
        "neural",
        "network",
        "deep",
        "training",
        "model",
        "data",
        "algorithm",
        "optimization",
        "gradient",
        "descent",
        "backpropagation",
        "convolution",
        "recurrent",
        "transformer",
        "attention",
        "embedding",
        "vector",
        "search",
        "similarity",
        "distance",
        "nearest",
        "neighbor",
        "index",
        "query",
        "retrieval",
        "ranking",
        "database",
        "storage",
        "distributed",
        "parallel",
        "processing",
    ];

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    (0..n)
        .map(|_| {
            let len = rng.gen_range(20..100);
            (0..len)
                .map(|_| words[rng.gen_range(0..words.len())])
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

// ============================================================================
// Vector-Only vs Hybrid Benchmarks
// ============================================================================

fn bench_vector_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("Vector Only Search");

    for &n in [10_000, 100_000].iter() {
        let dims = 768;
        let vectors = generate_random_vectors(n, dims, 42);
        let query = vectors[0].clone();

        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("sequential", n), &n, |bench, _| {
            bench.iter(|| black_box(vector_search_fn(&vectors, &query, 10)))
        });

        group.bench_with_input(BenchmarkId::new("parallel", n), &n, |bench, _| {
            bench.iter(|| black_box(vector_search_parallel(&vectors, &query, 10)))
        });
    }

    group.finish();
}

fn bench_text_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("Text Only (BM25) Search");

    for &n in [10_000, 100_000].iter() {
        let documents = generate_random_documents(n, 42);

        let mut bm25 = BM25Index::new(1.2, 0.75);
        bm25.build(&documents);

        let query = "machine learning neural network";

        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |bench, _| {
            bench.iter(|| black_box(bm25.search(query, 10)))
        });
    }

    group.finish();
}

fn bench_hybrid_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("Hybrid Search");

    for &n in [10_000, 100_000].iter() {
        let dims = 768;
        let vectors = generate_random_vectors(n, dims, 42);
        let documents = generate_random_documents(n, 42);
        let vector_query = vectors[0].clone();
        let text_query = "machine learning neural network";

        let mut bm25 = BM25Index::new(1.2, 0.75);
        bm25.build(&documents);

        group.throughput(Throughput::Elements(n as u64));

        // Sequential hybrid
        group.bench_with_input(BenchmarkId::new("sequential", n), &n, |bench, _| {
            bench.iter(|| {
                let vector_results = vector_search_fn(&vectors, &vector_query, 100);
                let text_results = bm25.search(text_query, 100);
                black_box(rrf(&vector_results, &text_results, 10, 60.0))
            })
        });

        // Parallel hybrid (branches)
        group.bench_with_input(BenchmarkId::new("parallel_branches", n), &n, |bench, _| {
            bench.iter(|| {
                let (vector_results, text_results) = rayon::join(
                    || vector_search_parallel(&vectors, &vector_query, 100),
                    || bm25.search(text_query, 100),
                );
                black_box(rrf(&vector_results, &text_results, 10, 60.0))
            })
        });
    }

    group.finish();
}

// ============================================================================
// BM25 Overhead Benchmarks
// ============================================================================

fn bench_bm25_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("BM25 Index Build");

    for &n in [1_000, 10_000, 100_000].iter() {
        let documents = generate_random_documents(n, 42);

        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::from_parameter(n), &documents, |bench, docs| {
            bench.iter(|| {
                let mut bm25 = BM25Index::new(1.2, 0.75);
                bm25.build(docs);
                black_box(bm25)
            })
        });
    }

    group.finish();
}

fn bench_bm25_query_lengths(c: &mut Criterion) {
    let mut group = c.benchmark_group("BM25 Query Length");

    let n = 100_000;
    let documents = generate_random_documents(n, 42);

    let mut bm25 = BM25Index::new(1.2, 0.75);
    bm25.build(&documents);

    let queries = [
        "machine",
        "machine learning",
        "machine learning neural network",
        "machine learning neural network deep training model",
        "machine learning neural network deep training model algorithm optimization gradient descent",
    ];

    for query in queries.iter() {
        let token_count = tokenize(query).len();

        group.bench_with_input(
            BenchmarkId::new("tokens", token_count),
            query,
            |bench, q| bench.iter(|| black_box(bm25.search(q, 10))),
        );
    }

    group.finish();
}

// ============================================================================
// Fusion Algorithm Comparison
// ============================================================================

fn bench_fusion_algorithms(c: &mut Criterion) {
    let mut group = c.benchmark_group("Fusion Algorithms");

    let n = 100_000;
    let dims = 768;
    let vectors = generate_random_vectors(n, dims, 42);
    let documents = generate_random_documents(n, 42);
    let vector_query = vectors[0].clone();
    let text_query = "machine learning neural network";

    let mut bm25 = BM25Index::new(1.2, 0.75);
    bm25.build(&documents);

    // Pre-compute search results
    let vector_results = vector_search_fn(&vectors, &vector_query, 1000);
    let text_results = bm25.search(text_query, 1000);

    for &k in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::new("rrf", k), &k, |bench, &k_val| {
            bench.iter(|| black_box(rrf(&vector_results, &text_results, k_val, 60.0)))
        });

        group.bench_with_input(BenchmarkId::new("weighted_sum", k), &k, |bench, &k_val| {
            bench.iter(|| {
                black_box(weighted_sum(
                    &vector_results,
                    &text_results,
                    k_val,
                    0.6,
                    0.4,
                ))
            })
        });

        group.bench_with_input(
            BenchmarkId::new("disjunctive_norm", k),
            &k,
            |bench, &k_val| {
                bench.iter(|| {
                    black_box(disjunctive_normalization(
                        &vector_results,
                        &text_results,
                        k_val,
                    ))
                })
            },
        );
    }

    group.finish();
}

fn bench_rrf_k_parameter(c: &mut Criterion) {
    let mut group = c.benchmark_group("RRF K Parameter");

    let n = 100_000;
    let dims = 768;
    let vectors = generate_random_vectors(n, dims, 42);
    let documents = generate_random_documents(n, 42);
    let vector_query = vectors[0].clone();
    let text_query = "machine learning neural network";

    let mut bm25 = BM25Index::new(1.2, 0.75);
    bm25.build(&documents);

    let vector_results = vector_search_fn(&vectors, &vector_query, 1000);
    let text_results = bm25.search(text_query, 1000);

    for &rrf_k in [1.0, 20.0, 60.0, 100.0, 200.0].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(rrf_k as i32),
            &rrf_k,
            |bench, &k| bench.iter(|| black_box(rrf(&vector_results, &text_results, 10, k))),
        );
    }

    group.finish();
}

fn bench_weight_ratios(c: &mut Criterion) {
    let mut group = c.benchmark_group("Weight Ratios");

    let n = 100_000;
    let dims = 768;
    let vectors = generate_random_vectors(n, dims, 42);
    let documents = generate_random_documents(n, 42);
    let vector_query = vectors[0].clone();
    let text_query = "machine learning neural network";

    let mut bm25 = BM25Index::new(1.2, 0.75);
    bm25.build(&documents);

    let vector_results = vector_search_fn(&vectors, &vector_query, 1000);
    let text_results = bm25.search(text_query, 1000);

    let ratios = [
        (0.0, 1.0, "text_only"),
        (0.3, 0.7, "text_heavy"),
        (0.5, 0.5, "balanced"),
        (0.7, 0.3, "vector_heavy"),
        (1.0, 0.0, "vector_only"),
    ];

    for (vector_w, text_w, name) in ratios.iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &(*vector_w, *text_w),
            |bench, &(v_w, t_w)| {
                bench.iter(|| black_box(weighted_sum(&vector_results, &text_results, 10, v_w, t_w)))
            },
        );
    }

    group.finish();
}

// ============================================================================
// Parallel Branch Execution
// ============================================================================

fn bench_parallel_execution_gain(c: &mut Criterion) {
    let mut group = c.benchmark_group("Parallel Branch Execution");

    for &n in [10_000, 50_000, 100_000].iter() {
        let dims = 768;
        let vectors = generate_random_vectors(n, dims, 42);
        let documents = generate_random_documents(n, 42);
        let vector_query = vectors[0].clone();
        let text_query = "machine learning neural network";

        let mut bm25 = BM25Index::new(1.2, 0.75);
        bm25.build(&documents);

        // Sequential
        group.bench_with_input(BenchmarkId::new("sequential", n), &n, |bench, _| {
            bench.iter(|| {
                let vector_results = vector_search_fn(&vectors, &vector_query, 100);
                let text_results = bm25.search(text_query, 100);
                black_box((vector_results, text_results))
            })
        });

        // Parallel with rayon::join
        group.bench_with_input(BenchmarkId::new("parallel_join", n), &n, |bench, _| {
            bench.iter(|| {
                let (vector_results, text_results) = rayon::join(
                    || vector_search_fn(&vectors, &vector_query, 100),
                    || bm25.search(text_query, 100),
                );
                black_box((vector_results, text_results))
            })
        });

        // Parallel vector search only
        group.bench_with_input(BenchmarkId::new("parallel_vector", n), &n, |bench, _| {
            bench.iter(|| {
                let vector_results = vector_search_parallel(&vectors, &vector_query, 100);
                let text_results = bm25.search(text_query, 100);
                black_box((vector_results, text_results))
            })
        });

        // Full parallel
        group.bench_with_input(BenchmarkId::new("full_parallel", n), &n, |bench, _| {
            bench.iter(|| {
                let (vector_results, text_results) = rayon::join(
                    || vector_search_parallel(&vectors, &vector_query, 100),
                    || bm25.search(text_query, 100),
                );
                black_box((vector_results, text_results))
            })
        });
    }

    group.finish();
}

// ============================================================================
// Candidate Count Analysis
// ============================================================================

fn bench_candidate_counts(c: &mut Criterion) {
    let mut group = c.benchmark_group("Candidate Count Analysis");

    let n = 100_000;
    let dims = 768;
    let vectors = generate_random_vectors(n, dims, 42);
    let documents = generate_random_documents(n, 42);
    let vector_query = vectors[0].clone();
    let text_query = "machine learning neural network";

    let mut bm25 = BM25Index::new(1.2, 0.75);
    bm25.build(&documents);

    for &candidates in [50, 100, 200, 500, 1000, 2000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(candidates),
            &candidates,
            |bench, &k_candidates| {
                bench.iter(|| {
                    let (vector_results, text_results) = rayon::join(
                        || vector_search_parallel(&vectors, &vector_query, k_candidates),
                        || bm25.search(text_query, k_candidates),
                    );
                    black_box(rrf(&vector_results, &text_results, 10, 60.0))
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    // Vector vs Text
    bench_vector_only,
    bench_text_only,
    bench_hybrid_search,
    // BM25 Overhead
    bench_bm25_build,
    bench_bm25_query_lengths,
    // Fusion Algorithms
    bench_fusion_algorithms,
    bench_rrf_k_parameter,
    bench_weight_ratios,
    // Parallel Execution
    bench_parallel_execution_gain,
    bench_candidate_counts,
);

criterion_main!(benches);
