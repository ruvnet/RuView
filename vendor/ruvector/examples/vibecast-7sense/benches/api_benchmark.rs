//! API Benchmark Suite for 7sense
//!
//! Performance targets from ADR-004:
//! - Query latency: <100ms total (end-to-end)
//! - Neighbor search: <50ms p99
//! - Evidence pack generation: <200ms

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::HashMap;
use std::time::Duration;

mod utils;
use utils::*;

// ============================================================================
// Simulated API Types
// ============================================================================

/// Neighbor search request
#[derive(Clone, Debug)]
struct NeighborSearchRequest {
    embedding: Vec<f32>,
    k: usize,
    filter: Option<SearchFilter>,
    include_metadata: bool,
}

/// Search filter for neighbor queries
#[derive(Clone, Debug)]
struct SearchFilter {
    species: Option<Vec<String>>,
    location: Option<BoundingBox>,
    time_range: Option<TimeRange>,
    min_confidence: Option<f32>,
}

#[derive(Clone, Debug)]
struct BoundingBox {
    min_lat: f32,
    max_lat: f32,
    min_lon: f32,
    max_lon: f32,
}

#[derive(Clone, Debug)]
struct TimeRange {
    start: i64,
    end: i64,
}

/// Neighbor search response
#[derive(Clone, Debug)]
struct NeighborSearchResponse {
    results: Vec<SearchResult>,
    total_time_ms: u64,
    cache_hit: bool,
}

#[derive(Clone, Debug)]
struct SearchResult {
    id: String,
    distance: f32,
    metadata: Option<EmbeddingMetadata>,
}

/// Embedding metadata
#[derive(Clone, Debug)]
struct EmbeddingMetadata {
    recording_id: String,
    species: Option<String>,
    call_type: Option<String>,
    location: Option<Location>,
    timestamp: i64,
    confidence: f32,
    audio_url: Option<String>,
}

#[derive(Clone, Debug)]
struct Location {
    lat: f32,
    lon: f32,
    site_name: Option<String>,
}

/// Evidence pack for interpretability
#[derive(Clone, Debug)]
struct EvidencePack {
    query_embedding: Vec<f32>,
    neighbors: Vec<NeighborEvidence>,
    cluster_info: ClusterInfo,
    spectrogram_url: Option<String>,
    attention_map: Option<Vec<Vec<f32>>>,
    confidence_breakdown: ConfidenceBreakdown,
}

#[derive(Clone, Debug)]
struct NeighborEvidence {
    result: SearchResult,
    similarity_score: f32,
    contributing_features: Vec<FeatureContribution>,
}

#[derive(Clone, Debug)]
struct FeatureContribution {
    feature_name: String,
    contribution: f32,
}

#[derive(Clone, Debug)]
struct ClusterInfo {
    cluster_id: i32,
    cluster_size: usize,
    centroid_distance: f32,
    typical_species: Vec<String>,
}

#[derive(Clone, Debug)]
struct ConfidenceBreakdown {
    neighbor_agreement: f32,
    cluster_membership: f32,
    embedding_quality: f32,
    overall: f32,
}

// ============================================================================
// Simulated API Service
// ============================================================================

/// Simulated API service for benchmarking
struct ApiService {
    index: SimpleHnswIndex,
    metadata_store: HashMap<usize, EmbeddingMetadata>,
    cluster_centroids: Vec<Vec<f32>>,
    cluster_assignments: Vec<i32>,
}

impl ApiService {
    fn new(index: SimpleHnswIndex, num_clusters: usize) -> Self {
        let size = index.len();

        // Generate fake metadata
        let mut metadata_store = HashMap::new();
        let species = ["Robin", "Sparrow", "Blackbird", "Thrush", "Finch"];
        let call_types = ["song", "call", "alarm", "contact"];

        for i in 0..size {
            metadata_store.insert(
                i,
                EmbeddingMetadata {
                    recording_id: format!("rec_{}", i),
                    species: Some(species[i % species.len()].to_string()),
                    call_type: Some(call_types[i % call_types.len()].to_string()),
                    location: Some(Location {
                        lat: 51.5 + (i as f32 * 0.001),
                        lon: -0.1 + (i as f32 * 0.001),
                        site_name: Some(format!("Site {}", i % 10)),
                    }),
                    timestamp: 1700000000 + (i as i64 * 300),
                    confidence: 0.7 + (i as f32 % 30) / 100.0,
                    audio_url: Some(format!("https://audio.example.com/{}.wav", i)),
                },
            );
        }

        // Generate cluster centroids and assignments
        let cluster_centroids = generate_random_vectors(num_clusters, PERCH_EMBEDDING_DIM);
        let cluster_assignments: Vec<i32> = (0..size).map(|i| (i % num_clusters) as i32).collect();

        Self {
            index,
            metadata_store,
            cluster_centroids,
            cluster_assignments,
        }
    }

    /// Execute neighbor search
    fn neighbor_search(&self, request: &NeighborSearchRequest) -> NeighborSearchResponse {
        let start = std::time::Instant::now();

        // Perform HNSW search
        let raw_results = self.index.search(&request.embedding, request.k * 2);

        // Apply filters
        let filtered_results: Vec<_> = raw_results
            .into_iter()
            .filter(|(idx, _)| self.apply_filter(*idx, &request.filter))
            .take(request.k)
            .collect();

        // Build response with optional metadata
        let results: Vec<SearchResult> = filtered_results
            .into_iter()
            .map(|(idx, distance)| SearchResult {
                id: format!("emb_{}", idx),
                distance,
                metadata: if request.include_metadata {
                    self.metadata_store.get(&idx).cloned()
                } else {
                    None
                },
            })
            .collect();

        NeighborSearchResponse {
            results,
            total_time_ms: start.elapsed().as_millis() as u64,
            cache_hit: false,
        }
    }

    fn apply_filter(&self, idx: usize, filter: &Option<SearchFilter>) -> bool {
        match filter {
            None => true,
            Some(f) => {
                if let Some(metadata) = self.metadata_store.get(&idx) {
                    // Species filter
                    if let Some(species_list) = &f.species {
                        if let Some(species) = &metadata.species {
                            if !species_list.contains(species) {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }

                    // Confidence filter
                    if let Some(min_conf) = f.min_confidence {
                        if metadata.confidence < min_conf {
                            return false;
                        }
                    }

                    // Time range filter
                    if let Some(time_range) = &f.time_range {
                        if metadata.timestamp < time_range.start
                            || metadata.timestamp > time_range.end
                        {
                            return false;
                        }
                    }

                    // Location filter
                    if let Some(bbox) = &f.location {
                        if let Some(loc) = &metadata.location {
                            if loc.lat < bbox.min_lat
                                || loc.lat > bbox.max_lat
                                || loc.lon < bbox.min_lon
                                || loc.lon > bbox.max_lon
                            {
                                return false;
                            }
                        } else {
                            return false;
                        }
                    }

                    true
                } else {
                    false
                }
            }
        }
    }

    /// Generate evidence pack for interpretability
    fn generate_evidence_pack(&self, embedding: &[f32], k: usize) -> EvidencePack {
        // Get neighbors
        let raw_results = self.index.search(embedding, k);

        let neighbors: Vec<NeighborEvidence> = raw_results
            .iter()
            .map(|(idx, distance)| {
                let metadata = self.metadata_store.get(idx).cloned();
                let similarity = 1.0 / (1.0 + distance);

                // Generate feature contributions (mock)
                let contributions: Vec<FeatureContribution> = (0..5)
                    .map(|i| FeatureContribution {
                        feature_name: format!("feature_{}", i),
                        contribution: similarity * (1.0 - i as f32 * 0.1),
                    })
                    .collect();

                NeighborEvidence {
                    result: SearchResult {
                        id: format!("emb_{}", idx),
                        distance: *distance,
                        metadata,
                    },
                    similarity_score: similarity,
                    contributing_features: contributions,
                }
            })
            .collect();

        // Compute cluster info
        let cluster_info = self.compute_cluster_info(embedding);

        // Compute confidence breakdown
        let confidence_breakdown = self.compute_confidence(embedding, &neighbors);

        EvidencePack {
            query_embedding: embedding.to_vec(),
            neighbors,
            cluster_info,
            spectrogram_url: Some("https://spectrograms.example.com/query.png".to_string()),
            attention_map: Some(self.generate_attention_map()),
            confidence_breakdown,
        }
    }

    fn compute_cluster_info(&self, embedding: &[f32]) -> ClusterInfo {
        // Find nearest cluster
        let mut best_cluster = 0;
        let mut best_distance = f32::MAX;

        for (i, centroid) in self.cluster_centroids.iter().enumerate() {
            let dist = l2_distance(embedding, centroid);
            if dist < best_distance {
                best_distance = dist;
                best_cluster = i;
            }
        }

        // Count cluster members
        let cluster_size = self
            .cluster_assignments
            .iter()
            .filter(|&&c| c == best_cluster as i32)
            .count();

        ClusterInfo {
            cluster_id: best_cluster as i32,
            cluster_size,
            centroid_distance: best_distance,
            typical_species: vec!["Robin".to_string(), "Sparrow".to_string()],
        }
    }

    fn compute_confidence(&self, _embedding: &[f32], neighbors: &[NeighborEvidence]) -> ConfidenceBreakdown {
        // Compute neighbor agreement
        let neighbor_agreement = if !neighbors.is_empty() {
            let avg_sim: f32 = neighbors.iter().map(|n| n.similarity_score).sum::<f32>()
                / neighbors.len() as f32;
            avg_sim
        } else {
            0.0
        };

        ConfidenceBreakdown {
            neighbor_agreement,
            cluster_membership: 0.85,
            embedding_quality: 0.92,
            overall: (neighbor_agreement + 0.85 + 0.92) / 3.0,
        }
    }

    fn generate_attention_map(&self) -> Vec<Vec<f32>> {
        // Generate a small mock attention map
        (0..32)
            .map(|i| (0..128).map(|j| ((i * j) % 100) as f32 / 100.0).collect())
            .collect()
    }
}

// ============================================================================
// Neighbor Search Benchmarks
// ============================================================================

/// Benchmark neighbor search endpoint
fn benchmark_neighbor_search_endpoint(c: &mut Criterion) {
    let mut group = c.benchmark_group("neighbor_search_endpoint");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(15));

    // Build test service
    let index = setup_test_index(50_000);
    let service = ApiService::new(index, 50);

    let query = generate_random_vectors(1, PERCH_EMBEDDING_DIM).remove(0);

    // Basic search without metadata
    group.bench_function("basic_k10", |b| {
        let request = NeighborSearchRequest {
            embedding: query.clone(),
            k: 10,
            filter: None,
            include_metadata: false,
        };
        b.iter(|| black_box(service.neighbor_search(&request)));
    });

    // Search with metadata
    group.bench_function("with_metadata_k10", |b| {
        let request = NeighborSearchRequest {
            embedding: query.clone(),
            k: 10,
            filter: None,
            include_metadata: true,
        };
        b.iter(|| black_box(service.neighbor_search(&request)));
    });

    // Search with filters
    group.bench_function("filtered_k10", |b| {
        let request = NeighborSearchRequest {
            embedding: query.clone(),
            k: 10,
            filter: Some(SearchFilter {
                species: Some(vec!["Robin".to_string(), "Sparrow".to_string()]),
                location: None,
                time_range: None,
                min_confidence: Some(0.8),
            }),
            include_metadata: true,
        };
        b.iter(|| black_box(service.neighbor_search(&request)));
    });

    // Different k values
    for &k in &[10, 50, 100] {
        group.bench_with_input(BenchmarkId::new("k", k), &k, |b, &k| {
            let request = NeighborSearchRequest {
                embedding: query.clone(),
                k,
                filter: None,
                include_metadata: true,
            };
            b.iter(|| black_box(service.neighbor_search(&request)));
        });
    }

    group.finish();
}

/// Benchmark search throughput under concurrent load
fn benchmark_search_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_throughput");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(20));

    let index = setup_test_index(50_000);
    let service = ApiService::new(index, 50);

    // Batch of queries
    let queries = generate_random_vectors(100, PERCH_EMBEDDING_DIM);
    let requests: Vec<NeighborSearchRequest> = queries
        .into_iter()
        .map(|embedding| NeighborSearchRequest {
            embedding,
            k: 10,
            filter: None,
            include_metadata: true,
        })
        .collect();

    group.throughput(Throughput::Elements(requests.len() as u64));
    group.bench_function("batch_100_queries", |b| {
        b.iter(|| {
            for request in &requests {
                black_box(service.neighbor_search(request));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Evidence Pack Benchmarks
// ============================================================================

/// Benchmark evidence pack generation
fn benchmark_evidence_pack_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("evidence_pack_generation");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(15));

    let index = setup_test_index(50_000);
    let service = ApiService::new(index, 50);

    let query = generate_random_vectors(1, PERCH_EMBEDDING_DIM).remove(0);

    // Basic evidence pack
    group.bench_function("basic", |b| {
        b.iter(|| black_box(service.generate_evidence_pack(&query, 10)));
    });

    // Different neighbor counts
    for &k in &[5, 10, 20, 50] {
        group.bench_with_input(BenchmarkId::new("neighbors", k), &k, |b, &k| {
            b.iter(|| black_box(service.generate_evidence_pack(&query, k)));
        });
    }

    group.finish();
}

// ============================================================================
// Filter Performance Benchmarks
// ============================================================================

/// Benchmark filter application performance
fn benchmark_filter_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_performance");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    let index = setup_test_index(50_000);
    let service = ApiService::new(index, 50);

    let query = generate_random_vectors(1, PERCH_EMBEDDING_DIM).remove(0);

    // No filter
    group.bench_function("no_filter", |b| {
        let request = NeighborSearchRequest {
            embedding: query.clone(),
            k: 100,
            filter: None,
            include_metadata: false,
        };
        b.iter(|| black_box(service.neighbor_search(&request)));
    });

    // Species filter only
    group.bench_function("species_filter", |b| {
        let request = NeighborSearchRequest {
            embedding: query.clone(),
            k: 100,
            filter: Some(SearchFilter {
                species: Some(vec!["Robin".to_string()]),
                location: None,
                time_range: None,
                min_confidence: None,
            }),
            include_metadata: false,
        };
        b.iter(|| black_box(service.neighbor_search(&request)));
    });

    // Confidence filter only
    group.bench_function("confidence_filter", |b| {
        let request = NeighborSearchRequest {
            embedding: query.clone(),
            k: 100,
            filter: Some(SearchFilter {
                species: None,
                location: None,
                time_range: None,
                min_confidence: Some(0.9),
            }),
            include_metadata: false,
        };
        b.iter(|| black_box(service.neighbor_search(&request)));
    });

    // All filters combined
    group.bench_function("all_filters", |b| {
        let request = NeighborSearchRequest {
            embedding: query.clone(),
            k: 100,
            filter: Some(SearchFilter {
                species: Some(vec!["Robin".to_string(), "Sparrow".to_string()]),
                location: Some(BoundingBox {
                    min_lat: 51.0,
                    max_lat: 52.0,
                    min_lon: -1.0,
                    max_lon: 1.0,
                }),
                time_range: Some(TimeRange {
                    start: 1700000000,
                    end: 1710000000,
                }),
                min_confidence: Some(0.8),
            }),
            include_metadata: false,
        };
        b.iter(|| black_box(service.neighbor_search(&request)));
    });

    group.finish();
}

// ============================================================================
// Latency Analysis
// ============================================================================

/// Analyze end-to-end latency against targets
fn analyze_api_latency() {
    use std::time::Instant;

    println!("\n=== API Latency Analysis ===\n");

    // Build service
    let index = setup_test_index(100_000);
    let service = ApiService::new(index, 50);

    let queries = generate_random_vectors(1000, PERCH_EMBEDDING_DIM);

    // Neighbor search latency
    let mut search_latencies = Vec::new();
    for query in &queries {
        let request = NeighborSearchRequest {
            embedding: query.clone(),
            k: 10,
            filter: None,
            include_metadata: true,
        };

        let start = Instant::now();
        let _ = service.neighbor_search(&request);
        search_latencies.push(start.elapsed());
    }

    let search_stats = PerformanceStats::from_latencies(search_latencies);
    println!("Neighbor Search (k=10, with metadata):");
    println!("{}", search_stats.report());
    println!(
        "  p99 target: {}ms ({})",
        targets::QUERY_LATENCY_P99_MS,
        if search_stats.p99 <= Duration::from_millis(targets::QUERY_LATENCY_P99_MS) {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "  Total target: {}ms ({})",
        targets::TOTAL_QUERY_LATENCY_MS,
        if search_stats.p99 <= Duration::from_millis(targets::TOTAL_QUERY_LATENCY_MS) {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!();

    // Evidence pack latency
    let mut evidence_latencies = Vec::new();
    for query in queries.iter().take(100) {
        let start = Instant::now();
        let _ = service.generate_evidence_pack(query, 10);
        evidence_latencies.push(start.elapsed());
    }

    let evidence_stats = PerformanceStats::from_latencies(evidence_latencies);
    println!("Evidence Pack Generation (10 neighbors):");
    println!("{}", evidence_stats.report());
    println!(
        "  p99 target: 200ms ({})",
        if evidence_stats.p99 <= Duration::from_millis(200) {
            "PASS"
        } else {
            "FAIL"
        }
    );
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    name = search_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_neighbor_search_endpoint, benchmark_search_throughput
);

criterion_group!(
    name = evidence_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_evidence_pack_generation
);

criterion_group!(
    name = filter_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_filter_performance
);

criterion_main!(search_benches, evidence_benches, filter_benches);

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neighbor_search_basic() {
        let index = setup_test_index(1000);
        let service = ApiService::new(index, 10);

        let query = generate_random_vectors(1, PERCH_EMBEDDING_DIM).remove(0);
        let request = NeighborSearchRequest {
            embedding: query,
            k: 10,
            filter: None,
            include_metadata: true,
        };

        let response = service.neighbor_search(&request);
        assert_eq!(response.results.len(), 10);
        assert!(response.results.iter().all(|r| r.metadata.is_some()));
    }

    #[test]
    fn test_neighbor_search_with_filter() {
        let index = setup_test_index(1000);
        let service = ApiService::new(index, 10);

        let query = generate_random_vectors(1, PERCH_EMBEDDING_DIM).remove(0);
        let request = NeighborSearchRequest {
            embedding: query,
            k: 10,
            filter: Some(SearchFilter {
                species: Some(vec!["Robin".to_string()]),
                location: None,
                time_range: None,
                min_confidence: Some(0.7),
            }),
            include_metadata: true,
        };

        let response = service.neighbor_search(&request);
        // All results should match filter
        for result in &response.results {
            if let Some(metadata) = &result.metadata {
                assert_eq!(metadata.species, Some("Robin".to_string()));
                assert!(metadata.confidence >= 0.7);
            }
        }
    }

    #[test]
    fn test_evidence_pack_generation() {
        let index = setup_test_index(1000);
        let service = ApiService::new(index, 10);

        let query = generate_random_vectors(1, PERCH_EMBEDDING_DIM).remove(0);
        let evidence = service.generate_evidence_pack(&query, 10);

        assert_eq!(evidence.neighbors.len(), 10);
        assert!(evidence.confidence_breakdown.overall > 0.0);
        assert!(evidence.cluster_info.cluster_size > 0);
    }

    #[test]
    #[ignore] // Run with: cargo test --release -- --ignored --nocapture
    fn run_api_latency_analysis() {
        analyze_api_latency();
    }
}
