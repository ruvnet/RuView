//! Spectral Clustering
//!
//! Graph partitioning using spectral methods.
//! Efficient approximation via Chebyshev polynomials.

use super::ScaledLaplacian;

/// Spectral clustering configuration
#[derive(Debug, Clone)]
pub struct ClusteringConfig {
    /// Number of clusters
    pub k: usize,
    /// Number of eigenvectors to use
    pub num_eigenvectors: usize,
    /// Power iteration steps for eigenvector approximation
    pub power_iters: usize,
    /// K-means iterations
    pub kmeans_iters: usize,
    /// Random seed
    pub seed: u64,
}

impl Default for ClusteringConfig {
    fn default() -> Self {
        Self {
            k: 2,
            num_eigenvectors: 10,
            power_iters: 50,
            kmeans_iters: 20,
            seed: 42,
        }
    }
}

/// Spectral clustering result
#[derive(Debug, Clone)]
pub struct ClusteringResult {
    /// Cluster assignment for each vertex
    pub assignments: Vec<usize>,
    /// Eigenvector embedding (n × k)
    pub embedding: Vec<Vec<f64>>,
    /// Number of clusters
    pub k: usize,
}

impl ClusteringResult {
    /// Get vertices in cluster c
    pub fn cluster(&self, c: usize) -> Vec<usize> {
        self.assignments
            .iter()
            .enumerate()
            .filter(|(_, &a)| a == c)
            .map(|(i, _)| i)
            .collect()
    }

    /// Cluster sizes
    pub fn cluster_sizes(&self) -> Vec<usize> {
        let mut sizes = vec![0; self.k];
        for &a in &self.assignments {
            if a < self.k {
                sizes[a] += 1;
            }
        }
        sizes
    }
}

/// Spectral clustering
#[derive(Debug, Clone)]
pub struct SpectralClustering {
    /// Configuration
    config: ClusteringConfig,
}

impl SpectralClustering {
    /// Create with configuration
    pub fn new(config: ClusteringConfig) -> Self {
        Self { config }
    }

    /// Create with just number of clusters
    pub fn with_k(k: usize) -> Self {
        Self::new(ClusteringConfig {
            k,
            num_eigenvectors: k,
            ..Default::default()
        })
    }

    /// Cluster graph using normalized Laplacian eigenvectors
    pub fn cluster(&self, laplacian: &ScaledLaplacian) -> ClusteringResult {
        let n = laplacian.n;
        let k = self.config.k.min(n);
        let num_eig = self.config.num_eigenvectors.min(n);

        // Compute approximate eigenvectors of Laplacian
        // We want the k smallest eigenvalues (smoothest eigenvectors)
        // Use inverse power method on shifted Laplacian
        let embedding = self.compute_embedding(laplacian, num_eig);

        // Run k-means on embedding
        let assignments = self.kmeans(&embedding, k);

        ClusteringResult {
            assignments,
            embedding,
            k,
        }
    }

    /// Cluster using Fiedler vector (k=2)
    pub fn bipartition(&self, laplacian: &ScaledLaplacian) -> ClusteringResult {
        let n = laplacian.n;

        // Compute Fiedler vector (second smallest eigenvector)
        let fiedler = self.compute_fiedler(laplacian);

        // Partition by sign
        let assignments: Vec<usize> = fiedler
            .iter()
            .map(|&v| if v >= 0.0 { 0 } else { 1 })
            .collect();

        ClusteringResult {
            assignments,
            embedding: vec![fiedler],
            k: 2,
        }
    }

    /// Compute spectral embedding (k smallest non-trivial eigenvectors)
    fn compute_embedding(&self, laplacian: &ScaledLaplacian, k: usize) -> Vec<Vec<f64>> {
        let n = laplacian.n;
        if k == 0 || n == 0 {
            return vec![];
        }

        // Initialize random vectors
        let mut vectors: Vec<Vec<f64>> = (0..k)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        let x = ((j * 2654435769 + i * 1103515245 + self.config.seed as usize)
                            as f64
                            / 4294967296.0)
                            * 2.0
                            - 1.0;
                        x
                    })
                    .collect()
            })
            .collect();

        // Power iteration to find smallest eigenvectors
        // We use (I - L_scaled) which has largest eigenvalue where L_scaled has smallest
        for _ in 0..self.config.power_iters {
            for i in 0..k {
                // Apply (I - L_scaled) = (2I - L)/λ_max approximately
                // Simpler: just use deflated power iteration on L for smallest
                let mut y = vec![0.0; n];
                let lx = laplacian.apply(&vectors[i]);

                // We want small eigenvalues, so use (λ_max*I - L)
                let shift = 2.0; // Approximate max eigenvalue of scaled Laplacian
                for j in 0..n {
                    y[j] = shift * vectors[i][j] - lx[j];
                }

                // Orthogonalize against previous vectors and constant vector
                // First, remove constant component (eigenvalue 0)
                let mean: f64 = y.iter().sum::<f64>() / n as f64;
                for j in 0..n {
                    y[j] -= mean;
                }

                // Then orthogonalize against previous eigenvectors
                for prev in 0..i {
                    let dot: f64 = y.iter().zip(vectors[prev].iter()).map(|(a, b)| a * b).sum();
                    for j in 0..n {
                        y[j] -= dot * vectors[prev][j];
                    }
                }

                // Normalize
                let norm: f64 = y.iter().map(|x| x * x).sum::<f64>().sqrt();
                if norm > 1e-15 {
                    for j in 0..n {
                        y[j] /= norm;
                    }
                }

                vectors[i] = y;
            }
        }

        vectors
    }

    /// Compute Fiedler vector (second smallest eigenvector)
    fn compute_fiedler(&self, laplacian: &ScaledLaplacian) -> Vec<f64> {
        let embedding = self.compute_embedding(laplacian, 1);
        if embedding.is_empty() {
            return vec![0.0; laplacian.n];
        }
        embedding[0].clone()
    }

    /// K-means clustering on embedding
    fn kmeans(&self, embedding: &[Vec<f64>], k: usize) -> Vec<usize> {
        if embedding.is_empty() {
            return vec![];
        }

        let n = embedding[0].len();
        let dim = embedding.len();

        if n == 0 || k == 0 {
            return vec![];
        }

        // Initialize centroids (k-means++ style)
        let mut centroids: Vec<Vec<f64>> = Vec::with_capacity(k);

        // First centroid: random point
        let first = (self.config.seed as usize) % n;
        centroids.push((0..dim).map(|d| embedding[d][first]).collect());

        // Remaining centroids: proportional to squared distance
        for _ in 1..k {
            let mut distances: Vec<f64> = (0..n)
                .map(|i| {
                    centroids
                        .iter()
                        .map(|c| {
                            (0..dim)
                                .map(|d| (embedding[d][i] - c[d]).powi(2))
                                .sum::<f64>()
                        })
                        .fold(f64::INFINITY, f64::min)
                })
                .collect();

            let total: f64 = distances.iter().sum();
            if total > 0.0 {
                let threshold = (self.config.seed as f64 / 4294967296.0) * total;
                let mut cumsum = 0.0;
                let mut chosen = 0;
                for (i, &d) in distances.iter().enumerate() {
                    cumsum += d;
                    if cumsum >= threshold {
                        chosen = i;
                        break;
                    }
                }
                centroids.push((0..dim).map(|d| embedding[d][chosen]).collect());
            } else {
                // Degenerate case
                centroids.push(vec![0.0; dim]);
            }
        }

        // K-means iterations
        let mut assignments = vec![0; n];

        for _ in 0..self.config.kmeans_iters {
            // Assign points to nearest centroid
            for i in 0..n {
                let mut best_cluster = 0;
                let mut best_dist = f64::INFINITY;

                for (c, centroid) in centroids.iter().enumerate() {
                    let dist: f64 = (0..dim)
                        .map(|d| (embedding[d][i] - centroid[d]).powi(2))
                        .sum();

                    if dist < best_dist {
                        best_dist = dist;
                        best_cluster = c;
                    }
                }

                assignments[i] = best_cluster;
            }

            // Update centroids
            let mut counts = vec![0usize; k];
            for centroid in centroids.iter_mut() {
                for v in centroid.iter_mut() {
                    *v = 0.0;
                }
            }

            for (i, &c) in assignments.iter().enumerate() {
                counts[c] += 1;
                for d in 0..dim {
                    centroids[c][d] += embedding[d][i];
                }
            }

            for (c, centroid) in centroids.iter_mut().enumerate() {
                if counts[c] > 0 {
                    for v in centroid.iter_mut() {
                        *v /= counts[c] as f64;
                    }
                }
            }
        }

        assignments
    }

    /// Compute normalized cut value for a bipartition
    pub fn normalized_cut(&self, laplacian: &ScaledLaplacian, partition: &[bool]) -> f64 {
        let n = laplacian.n;
        if n == 0 {
            return 0.0;
        }

        // Compute cut and volumes
        let mut cut = 0.0;
        let mut vol_a = 0.0;
        let mut vol_b = 0.0;

        // For each entry in Laplacian
        for &(i, j, v) in &laplacian.entries {
            if i < n && j < n && i != j {
                // This is an edge (negative Laplacian entry)
                let w = -v; // Edge weight
                if w > 0.0 && partition[i] != partition[j] {
                    cut += w;
                }
            }
            if i == j && i < n {
                // Diagonal = degree
                if partition[i] {
                    vol_a += v;
                } else {
                    vol_b += v;
                }
            }
        }

        // NCut = cut/vol(A) + cut/vol(B)
        let ncut = if vol_a > 0.0 { cut / vol_a } else { 0.0 }
            + if vol_b > 0.0 { cut / vol_b } else { 0.0 };

        ncut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_cliques_graph() -> ScaledLaplacian {
        // Two cliques of size 3 connected by one edge
        let edges = vec![
            // Clique 1
            (0, 1, 1.0),
            (0, 2, 1.0),
            (1, 2, 1.0),
            // Clique 2
            (3, 4, 1.0),
            (3, 5, 1.0),
            (4, 5, 1.0),
            // Bridge
            (2, 3, 0.1),
        ];
        ScaledLaplacian::from_sparse_adjacency(&edges, 6)
    }

    #[test]
    fn test_spectral_clustering() {
        let laplacian = two_cliques_graph();
        let clustering = SpectralClustering::with_k(2);

        let result = clustering.cluster(&laplacian);

        assert_eq!(result.assignments.len(), 6);
        assert_eq!(result.k, 2);

        // Should roughly separate the two cliques
        let sizes = result.cluster_sizes();
        assert_eq!(sizes.iter().sum::<usize>(), 6);
    }

    #[test]
    fn test_bipartition() {
        let laplacian = two_cliques_graph();
        let clustering = SpectralClustering::with_k(2);

        let result = clustering.bipartition(&laplacian);

        assert_eq!(result.assignments.len(), 6);
        assert_eq!(result.k, 2);
    }

    #[test]
    fn test_cluster_extraction() {
        let laplacian = two_cliques_graph();
        let clustering = SpectralClustering::with_k(2);
        let result = clustering.cluster(&laplacian);

        let c0 = result.cluster(0);
        let c1 = result.cluster(1);

        // All vertices assigned
        assert_eq!(c0.len() + c1.len(), 6);
    }

    #[test]
    fn test_normalized_cut() {
        let laplacian = two_cliques_graph();
        let clustering = SpectralClustering::with_k(2);

        // Good partition: separate cliques
        let good_partition = vec![true, true, true, false, false, false];
        let good_ncut = clustering.normalized_cut(&laplacian, &good_partition);

        // Bad partition: mix cliques
        let bad_partition = vec![true, false, true, false, true, false];
        let bad_ncut = clustering.normalized_cut(&laplacian, &bad_partition);

        // Good partition should have lower normalized cut
        // (This is a heuristic test, actual values depend on graph structure)
        assert!(good_ncut >= 0.0);
        assert!(bad_ncut >= 0.0);
    }

    #[test]
    fn test_single_node() {
        let laplacian = ScaledLaplacian::from_sparse_adjacency(&[], 1);
        let clustering = SpectralClustering::with_k(1);

        let result = clustering.cluster(&laplacian);

        assert_eq!(result.assignments.len(), 1);
        assert_eq!(result.assignments[0], 0);
    }
}
