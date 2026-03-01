// Multi-Scale Coarse-Graining for Hierarchical Causal Analysis
// Implements k-way aggregation with O(log n) depth

/// Represents a partition of states into groups
#[derive(Debug, Clone)]
pub struct Partition {
    /// groups[i] contains indices of micro-states in macro-state i
    pub groups: Vec<Vec<usize>>,
}

impl Partition {
    pub fn num_macro_states(&self) -> usize {
        self.groups.len()
    }

    pub fn num_micro_states(&self) -> usize {
        self.groups.iter().map(|g| g.len()).sum()
    }

    /// Creates sequential k-way partition
    /// Groups: [0..k), [k..2k), [2k..3k), ...
    pub fn sequential(n: usize, k: usize) -> Self {
        let num_groups = (n + k - 1) / k; // Ceiling division
        let mut groups = Vec::with_capacity(num_groups);

        for i in 0..num_groups {
            let start = i * k;
            let end = (start + k).min(n);
            groups.push((start..end).collect());
        }

        Self { groups }
    }

    /// Creates partition by clustering based on transition similarity
    pub fn from_clustering(labels: Vec<usize>) -> Self {
        let num_clusters = labels.iter().max().map(|&x| x + 1).unwrap_or(0);
        let mut groups = vec![Vec::new(); num_clusters];

        for (state, &label) in labels.iter().enumerate() {
            groups[label].push(state);
        }

        Self { groups }
    }
}

/// Coarse-grains a transition matrix according to a partition
///
/// # Arguments
/// * `micro_matrix` - n×n transition matrix
/// * `partition` - How to group micro-states into macro-states
///
/// # Returns
/// m×m coarse-grained transition matrix where m = number of groups
///
/// # Algorithm
/// T'[I,J] = (1/|group_I|) Σᵢ∈group_I Σⱼ∈group_J T[i,j]
pub fn coarse_grain_transition_matrix(micro_matrix: &[f32], partition: &Partition) -> Vec<f32> {
    let n = (micro_matrix.len() as f32).sqrt() as usize;
    let m = partition.num_macro_states();

    let mut macro_matrix = vec![0.0; m * m];

    for (i_macro, group_i) in partition.groups.iter().enumerate() {
        for (j_macro, group_j) in partition.groups.iter().enumerate() {
            let mut sum = 0.0;

            // Sum transitions from group I to group J
            for &i_micro in group_i {
                for &j_micro in group_j {
                    sum += micro_matrix[i_micro * n + j_micro];
                }
            }

            // Average over source group size
            macro_matrix[i_macro * m + j_macro] = sum / (group_i.len() as f32);
        }
    }

    macro_matrix
}

/// Represents a hierarchical scale structure
#[derive(Debug, Clone)]
pub struct ScaleLevel {
    pub num_states: usize,
    pub transition_matrix: Vec<f32>,
    /// Partition mapping to original micro-states (level 0)
    pub partition: Partition,
}

/// Complete hierarchical decomposition
#[derive(Debug, Clone)]
pub struct ScaleHierarchy {
    pub levels: Vec<ScaleLevel>,
}

impl ScaleHierarchy {
    /// Builds hierarchy using sequential k-way coarse-graining
    ///
    /// # Arguments
    /// * `micro_matrix` - Base-level n×n transition matrix
    /// * `branching_factor` - k (typically 2-8)
    ///
    /// # Returns
    /// Hierarchy with O(log_k n) levels
    pub fn build_sequential(micro_matrix: Vec<f32>, branching_factor: usize) -> Self {
        let n = (micro_matrix.len() as f32).sqrt() as usize;
        let mut levels = Vec::new();

        // Level 0: micro-level
        levels.push(ScaleLevel {
            num_states: n,
            transition_matrix: micro_matrix.clone(),
            partition: Partition {
                groups: (0..n).map(|i| vec![i]).collect(),
            },
        });

        let mut current_matrix = micro_matrix;
        let mut current_partition = Partition {
            groups: (0..n).map(|i| vec![i]).collect(),
        };

        // Build hierarchy bottom-up
        while levels.last().unwrap().num_states > branching_factor {
            let current_n = levels.last().unwrap().num_states;

            // Create k-way partition
            let new_partition = Partition::sequential(current_n, branching_factor);

            // Coarse-grain matrix
            current_matrix = coarse_grain_transition_matrix(&current_matrix, &new_partition);

            // Update partition relative to original micro-states
            current_partition = merge_partitions(&current_partition, &new_partition);

            levels.push(ScaleLevel {
                num_states: new_partition.num_macro_states(),
                transition_matrix: current_matrix.clone(),
                partition: current_partition.clone(),
            });
        }

        Self { levels }
    }

    /// Builds hierarchy using optimal coarse-graining (minimizes redundancy)
    /// More expensive but finds better emergence
    pub fn build_optimal(micro_matrix: Vec<f32>, branching_factor: usize) -> Self {
        let n = (micro_matrix.len() as f32).sqrt() as usize;
        let mut levels = Vec::new();

        // Level 0
        levels.push(ScaleLevel {
            num_states: n,
            transition_matrix: micro_matrix.clone(),
            partition: Partition {
                groups: (0..n).map(|i| vec![i]).collect(),
            },
        });

        let mut current_matrix = micro_matrix;
        let mut current_partition = Partition {
            groups: (0..n).map(|i| vec![i]).collect(),
        };

        while levels.last().unwrap().num_states > branching_factor {
            let current_n = levels.last().unwrap().num_states;

            // Find optimal partition using similarity clustering
            let new_partition =
                find_optimal_partition(&current_matrix, current_n, branching_factor);

            current_matrix = coarse_grain_transition_matrix(&current_matrix, &new_partition);

            current_partition = merge_partitions(&current_partition, &new_partition);

            levels.push(ScaleLevel {
                num_states: new_partition.num_macro_states(),
                transition_matrix: current_matrix.clone(),
                partition: current_partition.clone(),
            });
        }

        Self { levels }
    }

    pub fn num_scales(&self) -> usize {
        self.levels.len()
    }

    pub fn scale(&self, index: usize) -> Option<&ScaleLevel> {
        self.levels.get(index)
    }
}

/// Merges two partitions: applies new_partition to current_partition
///
/// Example:
/// current: [[0,1], [2,3], [4,5]]
/// new: [[0,1], [2]]  (groups 0&1 together, group 2 alone)
/// result: [[0,1,2,3], [4,5]]
fn merge_partitions(current: &Partition, new: &Partition) -> Partition {
    let mut merged_groups = Vec::new();

    for new_group in &new.groups {
        let mut merged_group = Vec::new();

        for &macro_state in new_group {
            // Add all micro-states from this macro-state
            if let Some(micro_states) = current.groups.get(macro_state) {
                merged_group.extend(micro_states);
            }
        }

        merged_groups.push(merged_group);
    }

    Partition {
        groups: merged_groups,
    }
}

/// Finds optimal k-way partition by minimizing within-group variance
/// Uses k-means-like clustering on transition probability vectors
fn find_optimal_partition(matrix: &[f32], n: usize, k: usize) -> Partition {
    if n <= k {
        // Can't cluster into more groups than states
        return Partition::sequential(n, k);
    }

    // Extract row vectors (outgoing transition probabilities)
    let mut rows: Vec<Vec<f32>> = Vec::with_capacity(n);
    for i in 0..n {
        rows.push(matrix[i * n..(i + 1) * n].to_vec());
    }

    // Simple k-means clustering
    let labels = kmeans_cluster(&rows, k);

    Partition::from_clustering(labels)
}

/// Simple k-means clustering for transition probability vectors
/// Returns cluster labels for each state
fn kmeans_cluster(data: &[Vec<f32>], k: usize) -> Vec<usize> {
    let n = data.len();
    if n <= k {
        return (0..n).collect();
    }

    let dim = data[0].len();

    // Initialize centroids (first k data points)
    let mut centroids: Vec<Vec<f32>> = data[..k].to_vec();
    let mut labels = vec![0; n];

    // Iterate until convergence (max 20 iterations)
    for _ in 0..20 {
        let old_labels = labels.clone();

        // Assign each point to nearest centroid
        for (i, point) in data.iter().enumerate() {
            let mut min_dist = f32::MAX;
            let mut best_cluster = 0;

            for (c, centroid) in centroids.iter().enumerate() {
                let dist = euclidean_distance(point, centroid);
                if dist < min_dist {
                    min_dist = dist;
                    best_cluster = c;
                }
            }

            labels[i] = best_cluster;
        }

        // Update centroids
        for c in 0..k {
            let cluster_points: Vec<_> = data
                .iter()
                .zip(&labels)
                .filter(|(_, &label)| label == c)
                .map(|(point, _)| point)
                .collect();

            if !cluster_points.is_empty() {
                centroids[c] = compute_centroid(&cluster_points, dim);
            }
        }

        // Check convergence
        if labels == old_labels {
            break;
        }
    }

    labels
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn compute_centroid(points: &[&Vec<f32>], dim: usize) -> Vec<f32> {
    let n = points.len() as f32;
    let mut centroid = vec![0.0; dim];

    for point in points {
        for (i, &val) in point.iter().enumerate() {
            centroid[i] += val / n;
        }
    }

    centroid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequential_partition() {
        let partition = Partition::sequential(10, 3);

        assert_eq!(partition.num_macro_states(), 4); // [0-2], [3-5], [6-8], [9]
        assert_eq!(partition.groups[0], vec![0, 1, 2]);
        assert_eq!(partition.groups[3], vec![9]);
    }

    #[test]
    fn test_coarse_grain_deterministic() {
        // 4-state cycle: 0→1→2→3→0
        let mut micro = vec![0.0; 16];
        micro[0 * 4 + 1] = 1.0;
        micro[1 * 4 + 2] = 1.0;
        micro[2 * 4 + 3] = 1.0;
        micro[3 * 4 + 0] = 1.0;

        // Partition into 2 groups: [0,1] and [2,3]
        let partition = Partition {
            groups: vec![vec![0, 1], vec![2, 3]],
        };

        let macro_matrix = coarse_grain_transition_matrix(&micro, &partition);

        // Should get 2×2 matrix
        assert_eq!(macro_matrix.len(), 4);

        // Group 0 transitions to group 1 with prob 0.5 (state 0→1 or 1→2)
        assert!((macro_matrix[0 * 2 + 1] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_hierarchy_construction() {
        // 16-state random system
        let mut matrix = vec![0.0; 256];
        for i in 0..16 {
            for j in 0..16 {
                matrix[i * 16 + j] = ((i + j) % 10) as f32 / 10.0;
            }
            // Normalize row
            let row_sum: f32 = matrix[i * 16..(i + 1) * 16].iter().sum();
            for j in 0..16 {
                matrix[i * 16 + j] /= row_sum;
            }
        }

        let hierarchy = ScaleHierarchy::build_sequential(matrix, 2);

        // Should have 4 levels (16, 8, 4, 2) - stops at branching_factor
        assert_eq!(hierarchy.num_scales(), 4);
        assert_eq!(hierarchy.levels[0].num_states, 16);
        assert_eq!(hierarchy.levels[1].num_states, 8);
        assert_eq!(hierarchy.levels[2].num_states, 4);
        assert_eq!(hierarchy.levels[3].num_states, 2);
    }

    #[test]
    fn test_partition_merging() {
        let partition1 = Partition {
            groups: vec![vec![0, 1], vec![2, 3], vec![4, 5]],
        };

        let partition2 = Partition {
            groups: vec![vec![0, 1], vec![2]], // Merge groups 0&1, keep group 2
        };

        let merged = merge_partitions(&partition1, &partition2);

        assert_eq!(merged.num_macro_states(), 2);
        assert_eq!(merged.groups[0], vec![0, 1, 2, 3]);
        assert_eq!(merged.groups[1], vec![4, 5]);
    }

    #[test]
    fn test_kmeans_clustering() {
        let data = vec![
            vec![1.0, 0.0],
            vec![0.9, 0.1],
            vec![0.0, 1.0],
            vec![0.1, 0.9],
        ];

        let labels = kmeans_cluster(&data, 2);

        // Points 0&1 should cluster together, 2&3 together
        assert_eq!(labels[0], labels[1]);
        assert_eq!(labels[2], labels[3]);
        assert_ne!(labels[0], labels[2]);
    }
}
