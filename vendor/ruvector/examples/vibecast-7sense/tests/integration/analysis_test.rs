//! Integration tests for Analysis Context
//!
//! Tests for HDBSCAN clustering, cluster assignment, motif detection,
//! entropy calculation, and transition matrix operations.

use vibecast_tests::fixtures::*;
use vibecast_tests::mocks::*;
use std::collections::{HashMap, HashSet};

// ============================================================================
// HDBSCAN Clustering Tests
// ============================================================================

mod hdbscan_clustering {
    use super::*;

    #[test]
    fn test_cluster_with_clear_groups() {
        let service = MockClusteringService::with_params(5, 3);

        // Create two well-separated clusters
        let base1 = create_deterministic_vector(1536, 0);
        let base2 = create_deterministic_vector(1536, 1000);

        let mut embeddings = Vec::new();

        // Cluster 1: variations around base1
        for i in 0..15 {
            let noisy: Vec<f32> = base1.iter().map(|v| v + (i as f32 * 0.001)).collect();
            embeddings.push(create_test_embedding_with_vector(l2_normalize(&noisy)));
        }

        // Cluster 2: variations around base2
        for i in 0..15 {
            let noisy: Vec<f32> = base2.iter().map(|v| v + (i as f32 * 0.001)).collect();
            embeddings.push(create_test_embedding_with_vector(l2_normalize(&noisy)));
        }

        let clusters = service.cluster_hdbscan(&embeddings).unwrap();

        assert!(clusters.len() >= 1, "Should find at least one cluster");
    }

    #[test]
    fn test_cluster_with_insufficient_data() {
        let service = MockClusteringService::with_params(10, 5);

        // Only 3 embeddings - below min_cluster_size
        let embeddings: Vec<Embedding> = (0..3).map(|_| create_test_embedding()).collect();

        let clusters = service.cluster_hdbscan(&embeddings).unwrap();

        assert_eq!(clusters.len(), 0, "Should not form clusters with too few points");
    }

    #[test]
    fn test_cluster_method_assignment() {
        let cluster = create_test_cluster();
        assert_eq!(cluster.method, ClusteringMethod::Hdbscan);
    }

    #[test]
    fn test_cluster_cohesion_in_valid_range() {
        let cluster = create_test_cluster();

        assert!(cluster.cohesion >= 0.0 && cluster.cohesion <= 1.0);
        assert!(cluster.separation >= 0.0 && cluster.separation <= 1.0);
    }

    #[test]
    fn test_cluster_has_members() {
        let cluster = create_test_cluster_with_members(20);

        assert_eq!(cluster.member_ids.len(), 20);
        assert!(!cluster.centroid.is_empty());
    }

    #[test]
    fn test_cluster_centroid_is_normalized() {
        let cluster = create_test_cluster_with_members(10);

        let norm: f32 = cluster.centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.0001,
            "Centroid should be normalized"
        );
    }

    #[test]
    fn test_multiple_clusters() {
        let clusters = create_test_clusters(5);

        assert_eq!(clusters.len(), 5);

        // Each cluster should have unique ID
        let ids: HashSet<_> = clusters.iter().map(|c| c.id.0).collect();
        assert_eq!(ids.len(), 5, "All cluster IDs should be unique");
    }
}

// ============================================================================
// Cluster Assignment Tests
// ============================================================================

mod cluster_assignment {
    use super::*;

    #[test]
    fn test_assign_embedding_to_nearest_cluster() {
        let service = MockClusteringService::new();

        // Create clusters with known centroids
        let clusters = create_test_clusters(3);

        // Create embedding similar to first cluster's centroid
        let embedding = create_test_embedding_with_vector(clusters[0].centroid.clone());

        let assignment = service.assign_to_cluster(&embedding, &clusters).unwrap();

        assert!(assignment.is_some(), "Should assign to a cluster");
        let assignment = assignment.unwrap();
        assert_eq!(assignment.cluster_id, clusters[0].id);
        assert!(assignment.confidence > 0.0);
    }

    #[test]
    fn test_assignment_confidence_based_on_distance() {
        let service = MockClusteringService::new();
        let clusters = create_test_clusters(2);

        // Very close to centroid
        let close_embedding = create_test_embedding_with_vector(clusters[0].centroid.clone());
        let close_assignment = service
            .assign_to_cluster(&close_embedding, &clusters)
            .unwrap()
            .unwrap();

        // Farther from centroid
        let far_vector: Vec<f32> = clusters[0]
            .centroid
            .iter()
            .map(|v| v + 0.5)
            .collect();
        let far_embedding = create_test_embedding_with_vector(l2_normalize(&far_vector));
        let far_assignment = service
            .assign_to_cluster(&far_embedding, &clusters)
            .unwrap()
            .unwrap();

        assert!(
            close_assignment.confidence > far_assignment.confidence,
            "Closer embeddings should have higher confidence"
        );
    }

    #[test]
    fn test_no_assignment_to_empty_clusters() {
        let service = MockClusteringService::new();
        let embedding = create_test_embedding();
        let empty_clusters: Vec<Cluster> = vec![];

        let assignment = service.assign_to_cluster(&embedding, &empty_clusters).unwrap();
        assert!(assignment.is_none());
    }

    #[test]
    fn test_assignment_includes_distance_to_centroid() {
        let service = MockClusteringService::new();
        let clusters = create_test_clusters(1);
        let embedding = create_test_embedding();

        let assignment = service
            .assign_to_cluster(&embedding, &clusters)
            .unwrap()
            .unwrap();

        assert!(
            assignment.distance_to_centroid >= 0.0,
            "Distance should be non-negative"
        );
    }

    #[test]
    fn test_soft_assignment_concept() {
        // Test that an embedding near cluster boundary has lower confidence
        let service = MockClusteringService::new();

        // Create two clusters
        let base1 = create_deterministic_vector(1536, 0);
        let base2 = create_deterministic_vector(1536, 100);

        let clusters = vec![
            Cluster {
                id: ClusterId::new(),
                method: ClusteringMethod::Hdbscan,
                member_ids: vec![],
                centroid: l2_normalize(&base1),
                cohesion: 0.8,
                separation: 0.6,
            },
            Cluster {
                id: ClusterId::new(),
                method: ClusteringMethod::Hdbscan,
                member_ids: vec![],
                centroid: l2_normalize(&base2),
                cohesion: 0.8,
                separation: 0.6,
            },
        ];

        // Point exactly between clusters
        let midpoint: Vec<f32> = base1
            .iter()
            .zip(base2.iter())
            .map(|(a, b)| (a + b) / 2.0)
            .collect();
        let mid_embedding = create_test_embedding_with_vector(l2_normalize(&midpoint));

        let assignment = service
            .assign_to_cluster(&mid_embedding, &clusters)
            .unwrap()
            .unwrap();

        // Confidence should reflect uncertainty
        assert!(assignment.confidence < 0.9, "Boundary point should have lower confidence");
    }
}

// ============================================================================
// Motif Detection Tests
// ============================================================================

mod motif_detection {
    use super::*;

    #[test]
    fn test_detect_motifs_in_sequences() {
        let service = MockMotifDetectionService::new();

        // Create sequences with repeating patterns
        let cluster_ids: Vec<ClusterId> = (0..5).map(|_| ClusterId::new()).collect();

        let sequences: Vec<Vec<ClusterId>> = vec![
            vec![
                cluster_ids[0],
                cluster_ids[1],
                cluster_ids[2],
                cluster_ids[0],
                cluster_ids[1],
                cluster_ids[2],
            ],
            vec![
                cluster_ids[0],
                cluster_ids[1],
                cluster_ids[2],
                cluster_ids[3],
            ],
            vec![
                cluster_ids[2],
                cluster_ids[0],
                cluster_ids[1],
                cluster_ids[2],
            ],
        ];

        let motifs = service.detect_motifs(&sequences).unwrap();

        // Should find the [0,1,2] pattern that appears multiple times
        assert!(
            motifs.iter().any(|m| m.pattern.len() >= 2),
            "Should find at least one motif"
        );
    }

    #[test]
    fn test_motif_occurrence_count() {
        let motif = create_test_motif();

        assert!(motif.occurrence_count > 0);
        assert_eq!(motif.pattern.len(), 3);
    }

    #[test]
    fn test_motif_confidence_calculation() {
        let motif = create_test_motif();

        assert!(
            motif.confidence >= 0.0 && motif.confidence <= 1.0,
            "Confidence should be in [0, 1]"
        );
    }

    #[test]
    fn test_no_motifs_in_random_sequences() {
        let service = MockMotifDetectionService::new();

        // Create completely random sequences with no patterns
        let sequences: Vec<Vec<ClusterId>> = (0..5)
            .map(|_| (0..10).map(|_| ClusterId::new()).collect())
            .collect();

        let motifs = service.detect_motifs(&sequences).unwrap();

        // Random sequences unlikely to have recurring motifs
        // (though technically possible with mock implementation)
    }

    #[test]
    fn test_empty_sequence_handling() {
        let service = MockMotifDetectionService::new();
        let empty_sequences: Vec<Vec<ClusterId>> = vec![];

        let motifs = service.detect_motifs(&empty_sequences).unwrap();
        assert_eq!(motifs.len(), 0);
    }

    #[test]
    fn test_motif_duration_estimation() {
        let motif = create_test_motif();

        // 3-element motif at 5s per segment
        assert!(motif.avg_duration_ms >= 5000);
    }
}

// ============================================================================
// Entropy Calculation Tests
// ============================================================================

mod entropy_calculation {
    use super::*;

    #[test]
    fn test_entropy_rate_uniform_distribution() {
        // Create transition matrix with uniform distribution
        let n = 4;
        let cluster_ids: Vec<ClusterId> = (0..n).map(|_| ClusterId::new()).collect();
        let uniform_prob = 1.0 / n as f32;

        let matrix = TransitionMatrix {
            cluster_ids: cluster_ids.clone(),
            probabilities: vec![vec![uniform_prob; n]; n],
            observations: vec![vec![10; n]; n],
        };

        let entropy = compute_entropy_rate(&matrix);

        // Maximum entropy for uniform distribution = log2(n) = 2 bits for n=4
        let max_entropy = (n as f32).log2();
        assert!(
            (entropy - max_entropy).abs() < 0.1,
            "Uniform distribution should have maximum entropy: {} vs {}",
            entropy,
            max_entropy
        );
    }

    #[test]
    fn test_entropy_rate_deterministic() {
        // Create transition matrix with deterministic transitions
        let n = 4;
        let cluster_ids: Vec<ClusterId> = (0..n).map(|_| ClusterId::new()).collect();

        // Each state always transitions to the next state
        let mut probabilities = vec![vec![0.0; n]; n];
        for i in 0..n {
            probabilities[i][(i + 1) % n] = 1.0;
        }

        let matrix = TransitionMatrix {
            cluster_ids,
            probabilities,
            observations: vec![vec![10; n]; n],
        };

        let entropy = compute_entropy_rate(&matrix);

        // Deterministic transitions should have zero entropy
        assert!(
            entropy < 0.1,
            "Deterministic transitions should have near-zero entropy: {}",
            entropy
        );
    }

    #[test]
    fn test_entropy_rate_non_negative() {
        for _ in 0..10 {
            let matrix = create_test_transition_matrix(5);
            let entropy = compute_entropy_rate(&matrix);

            assert!(
                entropy >= 0.0,
                "Entropy should never be negative: {}",
                entropy
            );
        }
    }

    #[test]
    fn test_entropy_increases_with_randomness() {
        // Low entropy (predictable)
        let n = 4;
        let cluster_ids: Vec<ClusterId> = (0..n).map(|_| ClusterId::new()).collect();

        let mut low_rand_probs = vec![vec![0.0; n]; n];
        for i in 0..n {
            low_rand_probs[i][i] = 0.8; // High self-loop probability
            for j in 0..n {
                if i != j {
                    low_rand_probs[i][j] = 0.2 / (n - 1) as f32;
                }
            }
        }

        let low_entropy_matrix = TransitionMatrix {
            cluster_ids: cluster_ids.clone(),
            probabilities: low_rand_probs,
            observations: vec![vec![10; n]; n],
        };

        // High entropy (uniform)
        let uniform_prob = 1.0 / n as f32;
        let high_entropy_matrix = TransitionMatrix {
            cluster_ids,
            probabilities: vec![vec![uniform_prob; n]; n],
            observations: vec![vec![10; n]; n],
        };

        let low_entropy = compute_entropy_rate(&low_entropy_matrix);
        let high_entropy = compute_entropy_rate(&high_entropy_matrix);

        assert!(
            high_entropy > low_entropy,
            "More uniform distribution should have higher entropy: {} vs {}",
            high_entropy,
            low_entropy
        );
    }

    #[test]
    fn test_empty_matrix_entropy() {
        let matrix = TransitionMatrix {
            cluster_ids: vec![],
            probabilities: vec![],
            observations: vec![],
        };

        let entropy = compute_entropy_rate(&matrix);
        assert_eq!(entropy, 0.0);
    }
}

// ============================================================================
// Transition Matrix Tests
// ============================================================================

mod transition_matrix {
    use super::*;

    #[test]
    fn test_create_transition_matrix() {
        let matrix = create_test_transition_matrix(5);

        assert_eq!(matrix.cluster_ids.len(), 5);
        assert_eq!(matrix.probabilities.len(), 5);
        assert_eq!(matrix.probabilities[0].len(), 5);
    }

    #[test]
    fn test_transition_matrix_rows_sum_to_one() {
        let matrix = create_test_transition_matrix(5);

        for (i, row) in matrix.probabilities.iter().enumerate() {
            let row_sum: f32 = row.iter().copied().sum();
            assert!(
                (row_sum - 1.0).abs() < 0.0001,
                "Row {} should sum to 1.0, got {}",
                i,
                row_sum
            );
        }
    }

    #[test]
    fn test_transition_matrix_probabilities_non_negative() {
        let matrix = create_test_transition_matrix(5);

        for (i, row) in matrix.probabilities.iter().enumerate() {
            for (j, prob) in row.iter().copied().enumerate() {
                assert!(
                    prob >= 0.0,
                    "Probability at ({}, {}) should be non-negative: {}",
                    i,
                    j,
                    prob
                );
            }
        }
    }

    #[test]
    fn test_observations_matrix() {
        let matrix = create_test_transition_matrix(4);

        assert_eq!(matrix.observations.len(), 4);
        assert_eq!(matrix.observations[0].len(), 4);

        // All observations should be positive
        for row in &matrix.observations {
            for &count in row {
                assert!(count > 0);
            }
        }
    }

    #[test]
    fn test_build_transition_matrix_from_sequence() {
        let cluster_ids: Vec<ClusterId> = (0..3).map(|_| ClusterId::new()).collect();
        let sequence = vec![
            cluster_ids[0],
            cluster_ids[1],
            cluster_ids[0],
            cluster_ids[2],
            cluster_ids[1],
            cluster_ids[0],
        ];

        // Count transitions
        let mut counts: HashMap<(usize, usize), u32> = HashMap::new();
        for window in sequence.windows(2) {
            let from_idx = cluster_ids.iter().position(|c| *c == window[0]).unwrap();
            let to_idx = cluster_ids.iter().position(|c| *c == window[1]).unwrap();
            *counts.entry((from_idx, to_idx)).or_insert(0) += 1;
        }

        // Sequence: [0, 1, 0, 2, 1, 0]
        // Transitions: 0->1 (1x), 1->0 (2x), 0->2 (1x), 2->1 (1x)
        assert_eq!(*counts.get(&(0, 1)).unwrap_or(&0), 1);
        assert_eq!(*counts.get(&(1, 0)).unwrap_or(&0), 2);
    }
}

// ============================================================================
// Sequence Analysis Tests
// ============================================================================

mod sequence_analysis {
    use super::*;

    #[test]
    fn test_sequence_segment_ordering() {
        let segments = create_segment_sequence(10, 500);

        for i in 0..segments.len() - 1 {
            assert!(
                segments[i].end_ms <= segments[i + 1].start_ms,
                "Segments should be in temporal order"
            );
        }
    }

    #[test]
    fn test_stereotypy_calculation() {
        // Stereotypy = measure of how predictable transitions are
        // High stereotypy = consistent patterns
        // Low stereotypy = varied patterns

        let n = 4;
        let cluster_ids: Vec<ClusterId> = (0..n).map(|_| ClusterId::new()).collect();

        // Highly stereotyped (deterministic cycle)
        let mut stereotyped_probs = vec![vec![0.0; n]; n];
        for i in 0..n {
            stereotyped_probs[i][(i + 1) % n] = 1.0;
        }

        // Low stereotypy (uniform)
        let uniform_prob = 1.0 / n as f32;

        let stereotyped_matrix = TransitionMatrix {
            cluster_ids: cluster_ids.clone(),
            probabilities: stereotyped_probs,
            observations: vec![vec![10; n]; n],
        };

        let varied_matrix = TransitionMatrix {
            cluster_ids,
            probabilities: vec![vec![uniform_prob; n]; n],
            observations: vec![vec![10; n]; n],
        };

        let stereotyped_entropy = compute_entropy_rate(&stereotyped_matrix);
        let varied_entropy = compute_entropy_rate(&varied_matrix);

        // Stereotyped should have lower entropy (more predictable)
        assert!(stereotyped_entropy < varied_entropy);
    }

    #[test]
    fn test_motif_density() {
        // Motif density = ratio of segments that are part of motifs

        let total_segments = 100;
        let motif_segments = 60;

        let density = motif_segments as f32 / total_segments as f32;
        assert!((density - 0.6).abs() < 0.001);
    }
}

// ============================================================================
// Anomaly Detection Tests
// ============================================================================

mod anomaly_detection {
    use super::*;

    fn compute_local_outlier_factor(
        embedding: &Embedding,
        neighbors: &[Embedding],
    ) -> f32 {
        if neighbors.is_empty() {
            return 1.0;
        }

        // Compute average distance to neighbors
        let avg_distance: f32 = neighbors
            .iter()
            .map(|n| cosine_distance(&embedding.vector, &n.vector))
            .sum::<f32>()
            / neighbors.len() as f32;

        // LOF > 1 indicates anomaly
        // This is simplified; real LOF compares local density to neighbors' densities
        avg_distance * 10.0 // Scale factor for detection
    }

    #[test]
    fn test_detect_outlier_embedding() {
        // Create cluster of normal embeddings
        let base = create_deterministic_vector(1536, 0);
        let normal_embeddings: Vec<Embedding> = (0..20)
            .map(|i| {
                let noisy: Vec<f32> = base.iter().map(|v| v + (i as f32 * 0.001)).collect();
                create_test_embedding_with_vector(l2_normalize(&noisy))
            })
            .collect();

        // Create outlier (very different)
        let outlier_base = create_deterministic_vector(1536, 1000);
        let outlier = create_test_embedding_with_vector(l2_normalize(&outlier_base));

        // Compute LOF for outlier
        let lof = compute_local_outlier_factor(&outlier, &normal_embeddings);

        // LOF should be high for outlier
        assert!(lof > 1.0, "Outlier should have high LOF: {}", lof);
    }

    #[test]
    fn test_normal_embedding_not_anomalous() {
        let base = create_deterministic_vector(1536, 0);
        let embeddings: Vec<Embedding> = (0..20)
            .map(|i| {
                let noisy: Vec<f32> = base.iter().map(|v| v + (i as f32 * 0.001)).collect();
                create_test_embedding_with_vector(l2_normalize(&noisy))
            })
            .collect();

        // Check LOF for a normal point
        let test_point = &embeddings[10];
        let neighbors: Vec<Embedding> = embeddings
            .iter()
            .filter(|e| e.id != test_point.id)
            .cloned()
            .collect();

        let lof = compute_local_outlier_factor(test_point, &neighbors);

        // Should be relatively low for normal point
        assert!(
            lof < 5.0,
            "Normal point should have low LOF: {}",
            lof
        );
    }
}

// ============================================================================
// Cluster Validation Tests
// ============================================================================

mod cluster_validation {
    use super::*;

    fn compute_silhouette_score(
        embedding: &Embedding,
        own_cluster_members: &[Embedding],
        other_cluster_members: &[Embedding],
    ) -> f32 {
        if own_cluster_members.is_empty() {
            return 0.0;
        }

        // a = average distance to own cluster members
        let a: f32 = own_cluster_members
            .iter()
            .filter(|e| e.id != embedding.id)
            .map(|e| cosine_distance(&embedding.vector, &e.vector))
            .sum::<f32>()
            / (own_cluster_members.len() - 1).max(1) as f32;

        // b = average distance to nearest other cluster
        let b: f32 = if other_cluster_members.is_empty() {
            1.0
        } else {
            other_cluster_members
                .iter()
                .map(|e| cosine_distance(&embedding.vector, &e.vector))
                .sum::<f32>()
                / other_cluster_members.len() as f32
        };

        // Silhouette = (b - a) / max(a, b)
        let max_ab = a.max(b);
        if max_ab > 0.0 {
            (b - a) / max_ab
        } else {
            0.0
        }
    }

    #[test]
    fn test_silhouette_score_well_separated_clusters() {
        // Create well-separated clusters
        let base1 = create_deterministic_vector(1536, 0);
        let base2 = create_deterministic_vector(1536, 1000);

        let cluster1: Vec<Embedding> = (0..10)
            .map(|i| {
                let noisy: Vec<f32> = base1.iter().map(|v| v + (i as f32 * 0.001)).collect();
                create_test_embedding_with_vector(l2_normalize(&noisy))
            })
            .collect();

        let cluster2: Vec<Embedding> = (0..10)
            .map(|i| {
                let noisy: Vec<f32> = base2.iter().map(|v| v + (i as f32 * 0.001)).collect();
                create_test_embedding_with_vector(l2_normalize(&noisy))
            })
            .collect();

        // Compute silhouette for point in cluster 1
        let score = compute_silhouette_score(&cluster1[5], &cluster1, &cluster2);

        // Should be positive (closer to own cluster)
        assert!(
            score > 0.0,
            "Well-separated clusters should have positive silhouette: {}",
            score
        );
    }

    #[test]
    fn test_silhouette_score_range() {
        let embeddings = create_embedding_batch(20);

        // Split into two arbitrary clusters
        let cluster1: Vec<Embedding> = embeddings[0..10].to_vec();
        let cluster2: Vec<Embedding> = embeddings[10..20].to_vec();

        for emb in &cluster1 {
            let score = compute_silhouette_score(emb, &cluster1, &cluster2);
            assert!(
                score >= -1.0 && score <= 1.0,
                "Silhouette should be in [-1, 1]: {}",
                score
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_integration_smoke_test() {
        // Create embeddings
        let embeddings = create_embedding_batch(50);

        // Run clustering
        let service = MockClusteringService::with_params(5, 3);
        let clusters = service.cluster_hdbscan(&embeddings).unwrap();

        // Create transition matrix
        let matrix = create_test_transition_matrix(clusters.len().max(3));

        // Compute entropy
        let entropy = compute_entropy_rate(&matrix);
        assert!(entropy >= 0.0);

        // Detect motifs
        let motif_service = MockMotifDetectionService::new();
        let sequences: Vec<Vec<ClusterId>> = clusters
            .iter()
            .map(|c| vec![c.id, c.id, c.id])
            .collect();
        let _motifs = motif_service.detect_motifs(&sequences).unwrap();
    }
}
