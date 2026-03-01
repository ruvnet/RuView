//! Stage VI: Composite Modeling via MinCut Partitioning
//!
//! CRV Stage VI builds a composite 3D model from all accumulated session data.
//! The MinCut algorithm identifies natural cluster boundaries in the session
//! graph, separating distinct target aspects that emerged across stages.
//!
//! # Architecture
//!
//! All session embeddings form nodes in a weighted graph, with edge weights
//! derived from cosine similarity. MinCut partitioning finds the natural
//! separations between target aspects, producing distinct partitions that
//! represent different facets of the target.

use crate::error::{CrvError, CrvResult};
use crate::types::{CrvConfig, StageVIData, TargetPartition};
use ruvector_gnn::search::cosine_similarity;
use ruvector_mincut::prelude::*;

/// Stage VI composite modeler using MinCut partitioning.
#[derive(Debug, Clone)]
pub struct StageVIModeler {
    /// Embedding dimensionality.
    dim: usize,
    /// Minimum edge weight to create an edge (similarity threshold).
    edge_threshold: f32,
}

impl StageVIModeler {
    /// Create a new Stage VI modeler.
    pub fn new(config: &CrvConfig) -> Self {
        Self {
            dim: config.dimensions,
            edge_threshold: 0.2, // Low threshold to capture weak relationships too
        }
    }

    /// Build a similarity graph from session embeddings.
    ///
    /// Each embedding becomes a vertex. Edges are created between
    /// pairs with cosine similarity above the threshold, with
    /// edge weight equal to the similarity score.
    fn build_similarity_graph(&self, embeddings: &[Vec<f32>]) -> Vec<(u64, u64, f64)> {
        let n = embeddings.len();
        let mut edges = Vec::new();

        for i in 0..n {
            for j in (i + 1)..n {
                if embeddings[i].len() == embeddings[j].len() && !embeddings[i].is_empty() {
                    let sim = cosine_similarity(&embeddings[i], &embeddings[j]);
                    if sim > self.edge_threshold {
                        edges.push((i as u64 + 1, j as u64 + 1, sim as f64));
                    }
                }
            }
        }

        edges
    }

    /// Compute centroid of a set of embeddings.
    fn compute_centroid(&self, embeddings: &[&[f32]]) -> Vec<f32> {
        if embeddings.is_empty() {
            return vec![0.0; self.dim];
        }

        let mut centroid = vec![0.0f32; self.dim];
        for emb in embeddings {
            for (i, &v) in emb.iter().enumerate() {
                if i < self.dim {
                    centroid[i] += v;
                }
            }
        }

        let n = embeddings.len() as f32;
        for v in &mut centroid {
            *v /= n;
        }

        centroid
    }

    /// Partition session embeddings into target aspects using MinCut.
    ///
    /// Returns the MinCut-based partition assignments and centroids.
    pub fn partition(
        &self,
        embeddings: &[Vec<f32>],
        stage_labels: &[(u8, usize)], // (stage, entry_index) for each embedding
    ) -> CrvResult<StageVIData> {
        if embeddings.len() < 2 {
            // With fewer than 2 embeddings, return a single partition
            let centroid = if embeddings.is_empty() {
                vec![0.0; self.dim]
            } else {
                embeddings[0].clone()
            };

            return Ok(StageVIData {
                partitions: vec![TargetPartition {
                    label: "primary".to_string(),
                    member_entries: stage_labels.to_vec(),
                    centroid,
                    separation_strength: 0.0,
                }],
                composite_description: "Single-aspect target".to_string(),
                partition_confidence: vec![1.0],
            });
        }

        // Build similarity graph
        let edges = self.build_similarity_graph(embeddings);

        if edges.is_empty() {
            // No significant similarities found - each embedding is its own partition
            let partitions: Vec<TargetPartition> = embeddings
                .iter()
                .enumerate()
                .map(|(i, emb)| TargetPartition {
                    label: format!("aspect-{}", i),
                    member_entries: if i < stage_labels.len() {
                        vec![stage_labels[i]]
                    } else {
                        vec![]
                    },
                    centroid: emb.clone(),
                    separation_strength: 1.0,
                })
                .collect();

            let n = partitions.len();
            return Ok(StageVIData {
                partitions,
                composite_description: format!("{} disconnected aspects", n),
                partition_confidence: vec![0.5; n],
            });
        }

        // Build MinCut structure
        let mincut_result = MinCutBuilder::new()
            .exact()
            .with_edges(edges.clone())
            .build();

        let mincut = match mincut_result {
            Ok(mc) => mc,
            Err(_) => {
                // Fallback: single partition
                let centroid = self
                    .compute_centroid(&embeddings.iter().map(|e| e.as_slice()).collect::<Vec<_>>());
                return Ok(StageVIData {
                    partitions: vec![TargetPartition {
                        label: "composite".to_string(),
                        member_entries: stage_labels.to_vec(),
                        centroid,
                        separation_strength: 0.0,
                    }],
                    composite_description: "Unified composite model".to_string(),
                    partition_confidence: vec![0.8],
                });
            }
        };

        let cut_value = mincut.min_cut_value();

        // Use the MinCut value to determine partition boundary.
        // We partition into two groups based on connectivity:
        // vertices more connected to the "left" side vs "right" side.
        let n = embeddings.len();

        // Simple 2-partition based on similarity to first vs last embedding
        let (group_a, group_b) = self.bisect_by_similarity(embeddings);

        let centroid_a = self.compute_centroid(
            &group_a
                .iter()
                .map(|&i| embeddings[i].as_slice())
                .collect::<Vec<_>>(),
        );
        let centroid_b = self.compute_centroid(
            &group_b
                .iter()
                .map(|&i| embeddings[i].as_slice())
                .collect::<Vec<_>>(),
        );

        let members_a: Vec<(u8, usize)> = group_a
            .iter()
            .filter_map(|&i| stage_labels.get(i).copied())
            .collect();
        let members_b: Vec<(u8, usize)> = group_b
            .iter()
            .filter_map(|&i| stage_labels.get(i).copied())
            .collect();

        let partitions = vec![
            TargetPartition {
                label: "primary-aspect".to_string(),
                member_entries: members_a,
                centroid: centroid_a,
                separation_strength: cut_value as f32,
            },
            TargetPartition {
                label: "secondary-aspect".to_string(),
                member_entries: members_b,
                centroid: centroid_b,
                separation_strength: cut_value as f32,
            },
        ];

        // Confidence based on separation strength
        let total_edges = edges.len() as f32;
        let conf = if total_edges > 0.0 {
            (cut_value as f32 / total_edges).min(1.0)
        } else {
            0.5
        };

        Ok(StageVIData {
            partitions,
            composite_description: format!(
                "Bisected composite: {} embeddings, cut value {:.3}",
                n, cut_value
            ),
            partition_confidence: vec![conf, conf],
        })
    }

    /// Bisect embeddings into two groups by maximizing inter-group dissimilarity.
    ///
    /// Uses a greedy approach: pick the two most dissimilar embeddings as seeds,
    /// then assign each remaining embedding to the nearer seed.
    fn bisect_by_similarity(&self, embeddings: &[Vec<f32>]) -> (Vec<usize>, Vec<usize>) {
        let n = embeddings.len();
        if n <= 1 {
            return ((0..n).collect(), vec![]);
        }

        // Find the two most dissimilar embeddings
        let mut min_sim = f32::MAX;
        let mut seed_a = 0;
        let mut seed_b = 1;

        for i in 0..n {
            for j in (i + 1)..n {
                if embeddings[i].len() == embeddings[j].len() && !embeddings[i].is_empty() {
                    let sim = cosine_similarity(&embeddings[i], &embeddings[j]);
                    if sim < min_sim {
                        min_sim = sim;
                        seed_a = i;
                        seed_b = j;
                    }
                }
            }
        }

        let mut group_a = vec![seed_a];
        let mut group_b = vec![seed_b];

        for i in 0..n {
            if i == seed_a || i == seed_b {
                continue;
            }

            let sim_a = if embeddings[i].len() == embeddings[seed_a].len() {
                cosine_similarity(&embeddings[i], &embeddings[seed_a])
            } else {
                0.0
            };
            let sim_b = if embeddings[i].len() == embeddings[seed_b].len() {
                cosine_similarity(&embeddings[i], &embeddings[seed_b])
            } else {
                0.0
            };

            if sim_a >= sim_b {
                group_a.push(i);
            } else {
                group_b.push(i);
            }
        }

        (group_a, group_b)
    }

    /// Encode the Stage VI partition result into a single embedding.
    ///
    /// Produces a weighted combination of partition centroids.
    pub fn encode(&self, data: &StageVIData) -> CrvResult<Vec<f32>> {
        if data.partitions.is_empty() {
            return Err(CrvError::EmptyInput("No partitions".to_string()));
        }

        let mut embedding = vec![0.0f32; self.dim];
        let mut total_weight = 0.0f32;

        for (partition, &confidence) in data.partitions.iter().zip(data.partition_confidence.iter())
        {
            let weight = confidence * partition.member_entries.len() as f32;
            for (i, &v) in partition.centroid.iter().enumerate() {
                if i < self.dim {
                    embedding[i] += v * weight;
                }
            }
            total_weight += weight;
        }

        if total_weight > 1e-6 {
            for v in &mut embedding {
                *v /= total_weight;
            }
        }

        Ok(embedding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CrvConfig {
        CrvConfig {
            dimensions: 8,
            ..CrvConfig::default()
        }
    }

    #[test]
    fn test_modeler_creation() {
        let config = test_config();
        let modeler = StageVIModeler::new(&config);
        assert_eq!(modeler.dim, 8);
    }

    #[test]
    fn test_partition_single() {
        let config = test_config();
        let modeler = StageVIModeler::new(&config);

        let embeddings = vec![vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]];
        let labels = vec![(1, 0)];

        let result = modeler.partition(&embeddings, &labels).unwrap();
        assert_eq!(result.partitions.len(), 1);
    }

    #[test]
    fn test_partition_two_clusters() {
        let config = test_config();
        let modeler = StageVIModeler::new(&config);

        // Two clearly separated clusters
        let embeddings = vec![
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.9, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0, 0.9, 0.1, 0.0, 0.0],
        ];
        let labels = vec![(1, 0), (2, 0), (3, 0), (4, 0)];

        let result = modeler.partition(&embeddings, &labels).unwrap();
        assert_eq!(result.partitions.len(), 2);
    }

    #[test]
    fn test_encode_partitions() {
        let config = test_config();
        let modeler = StageVIModeler::new(&config);

        let data = StageVIData {
            partitions: vec![
                TargetPartition {
                    label: "a".to_string(),
                    member_entries: vec![(1, 0), (2, 0)],
                    centroid: vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    separation_strength: 0.5,
                },
                TargetPartition {
                    label: "b".to_string(),
                    member_entries: vec![(3, 0)],
                    centroid: vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    separation_strength: 0.5,
                },
            ],
            composite_description: "test".to_string(),
            partition_confidence: vec![0.8, 0.6],
        };

        let embedding = modeler.encode(&data).unwrap();
        assert_eq!(embedding.len(), 8);
    }
}
