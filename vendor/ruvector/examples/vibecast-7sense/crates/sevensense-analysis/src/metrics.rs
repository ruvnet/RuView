//! Clustering quality metrics and evaluation.
//!
//! Provides V-measure, silhouette score, entropy calculations, and other
//! metrics for evaluating clustering quality and sequence analysis.

use ndarray::{Array2, ArrayView1, Axis};
use std::collections::HashMap;

/// Clustering quality metrics collection.
#[derive(Debug, Clone, Default)]
pub struct ClusteringMetrics {
    /// Silhouette score (-1 to 1, higher is better).
    pub silhouette: f32,
    /// V-measure (0 to 1, higher is better).
    pub v_measure: f32,
    /// Homogeneity score (0 to 1).
    pub homogeneity: f32,
    /// Completeness score (0 to 1).
    pub completeness: f32,
    /// Number of clusters found.
    pub n_clusters: usize,
    /// Number of noise points (for density-based clustering).
    pub n_noise: usize,
    /// Inertia (within-cluster sum of squares).
    pub inertia: f32,
}

impl ClusteringMetrics {
    /// Create new clustering metrics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder method to set silhouette score.
    #[must_use]
    pub fn with_silhouette(mut self, silhouette: f32) -> Self {
        self.silhouette = silhouette;
        self
    }

    /// Builder method to set V-measure.
    #[must_use]
    pub fn with_v_measure(mut self, v_measure: f32) -> Self {
        self.v_measure = v_measure;
        self
    }

    /// Builder method to set cluster count.
    #[must_use]
    pub fn with_n_clusters(mut self, n_clusters: usize) -> Self {
        self.n_clusters = n_clusters;
        self
    }

    /// Check if clustering quality is acceptable.
    #[must_use]
    pub fn is_acceptable(&self, min_silhouette: f32, min_v_measure: f32) -> bool {
        self.silhouette >= min_silhouette && self.v_measure >= min_v_measure
    }
}

/// Silhouette score calculator.
///
/// The silhouette score measures how similar an object is to its own cluster
/// compared to other clusters. Values range from -1 to 1, where:
/// - 1 means the sample is far from neighboring clusters
/// - 0 means the sample is on or very close to the decision boundary
/// - -1 means the sample might have been assigned to the wrong cluster
#[derive(Debug, Clone)]
pub struct SilhouetteScore {
    /// Precomputed distance matrix (optional).
    distance_matrix: Option<Array2<f32>>,
}

impl SilhouetteScore {
    /// Create a new silhouette score calculator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            distance_matrix: None,
        }
    }

    /// Create with precomputed distance matrix.
    #[must_use]
    pub fn with_distance_matrix(distance_matrix: Array2<f32>) -> Self {
        Self {
            distance_matrix: Some(distance_matrix),
        }
    }

    /// Compute silhouette score for clustering.
    ///
    /// # Arguments
    ///
    /// * `data` - 2D array where rows are samples and columns are features
    /// * `labels` - Cluster label for each sample
    ///
    /// # Returns
    ///
    /// Mean silhouette score across all samples.
    #[must_use]
    pub fn compute(&self, data: &Array2<f32>, labels: &[i32]) -> f32 {
        let n = data.nrows();
        if n < 2 {
            return 0.0;
        }

        // Get unique cluster labels (excluding noise label -1)
        let unique_labels: Vec<i32> = labels
            .iter()
            .copied()
            .filter(|&l| l >= 0)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if unique_labels.len() < 2 {
            return 0.0;
        }

        let mut silhouette_values = Vec::with_capacity(n);

        for i in 0..n {
            if labels[i] < 0 {
                // Skip noise points
                continue;
            }

            let (a_i, b_i) = self.compute_ab(data, labels, i, &unique_labels);

            let s_i = if a_i < b_i {
                1.0 - (a_i / b_i)
            } else if a_i > b_i {
                (b_i / a_i) - 1.0
            } else {
                0.0
            };

            silhouette_values.push(s_i);
        }

        if silhouette_values.is_empty() {
            return 0.0;
        }

        silhouette_values.iter().sum::<f32>() / silhouette_values.len() as f32
    }

    /// Compute silhouette samples (individual scores).
    #[must_use]
    pub fn compute_samples(&self, data: &Array2<f32>, labels: &[i32]) -> Vec<f32> {
        let n = data.nrows();
        let mut silhouette_values = Vec::with_capacity(n);

        let unique_labels: Vec<i32> = labels
            .iter()
            .copied()
            .filter(|&l| l >= 0)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if unique_labels.len() < 2 {
            return vec![0.0; n];
        }

        for i in 0..n {
            if labels[i] < 0 {
                silhouette_values.push(0.0);
                continue;
            }

            let (a_i, b_i) = self.compute_ab(data, labels, i, &unique_labels);

            let s_i = if a_i < b_i {
                1.0 - (a_i / b_i)
            } else if a_i > b_i {
                (b_i / a_i) - 1.0
            } else {
                0.0
            };

            silhouette_values.push(s_i);
        }

        silhouette_values
    }

    /// Compute a(i) and b(i) for sample i.
    fn compute_ab(
        &self,
        data: &Array2<f32>,
        labels: &[i32],
        i: usize,
        unique_labels: &[i32],
    ) -> (f32, f32) {
        let label_i = labels[i];
        let n = data.nrows();

        // Compute a(i) - mean distance to same cluster
        let mut same_cluster_dist = 0.0f32;
        let mut same_cluster_count = 0;

        for j in 0..n {
            if j != i && labels[j] == label_i {
                same_cluster_dist += self.distance(data, i, j);
                same_cluster_count += 1;
            }
        }

        let a_i = if same_cluster_count > 0 {
            same_cluster_dist / same_cluster_count as f32
        } else {
            0.0
        };

        // Compute b(i) - minimum mean distance to other clusters
        let mut b_i = f32::MAX;

        for &other_label in unique_labels {
            if other_label == label_i {
                continue;
            }

            let mut other_cluster_dist = 0.0f32;
            let mut other_cluster_count = 0;

            for j in 0..n {
                if labels[j] == other_label {
                    other_cluster_dist += self.distance(data, i, j);
                    other_cluster_count += 1;
                }
            }

            if other_cluster_count > 0 {
                let mean_dist = other_cluster_dist / other_cluster_count as f32;
                b_i = b_i.min(mean_dist);
            }
        }

        if b_i == f32::MAX {
            b_i = 0.0;
        }

        (a_i, b_i)
    }

    /// Compute distance between samples i and j.
    fn distance(&self, data: &Array2<f32>, i: usize, j: usize) -> f32 {
        if let Some(ref dm) = self.distance_matrix {
            dm[[i, j]]
        } else {
            self.euclidean_distance(data.row(i), data.row(j))
        }
    }

    /// Compute Euclidean distance between two vectors.
    fn euclidean_distance(&self, a: ArrayView1<f32>, b: ArrayView1<f32>) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

impl Default for SilhouetteScore {
    fn default() -> Self {
        Self::new()
    }
}

/// V-Measure score calculator.
///
/// V-measure is a metric that combines homogeneity and completeness
/// using the harmonic mean (similar to F1 score).
///
/// - Homogeneity: each cluster contains only members of a single class
/// - Completeness: all members of a class are assigned to the same cluster
#[derive(Debug, Clone)]
pub struct VMeasure {
    /// Beta parameter for weighted harmonic mean (default: 1.0).
    beta: f32,
}

impl VMeasure {
    /// Create a new V-measure calculator.
    #[must_use]
    pub fn new() -> Self {
        Self { beta: 1.0 }
    }

    /// Create with custom beta parameter.
    ///
    /// Beta > 1 weights completeness more than homogeneity.
    /// Beta < 1 weights homogeneity more than completeness.
    #[must_use]
    pub fn with_beta(beta: f32) -> Self {
        Self { beta }
    }

    /// Compute V-measure score.
    ///
    /// # Arguments
    ///
    /// * `labels_true` - Ground truth class labels
    /// * `labels_pred` - Predicted cluster labels
    ///
    /// # Returns
    ///
    /// Tuple of (v_measure, homogeneity, completeness).
    #[must_use]
    pub fn compute(&self, labels_true: &[i32], labels_pred: &[i32]) -> (f32, f32, f32) {
        if labels_true.len() != labels_pred.len() {
            return (0.0, 0.0, 0.0);
        }

        if labels_true.is_empty() {
            return (1.0, 1.0, 1.0);
        }

        let homogeneity = self.compute_homogeneity(labels_true, labels_pred);
        let completeness = self.compute_completeness(labels_true, labels_pred);

        let v_measure = if homogeneity + completeness == 0.0 {
            0.0
        } else {
            let beta_sq = self.beta * self.beta;
            (1.0 + beta_sq) * homogeneity * completeness
                / (beta_sq * homogeneity + completeness)
        };

        (v_measure, homogeneity, completeness)
    }

    /// Compute homogeneity score.
    ///
    /// Homogeneity measures whether each cluster contains only members
    /// of a single class.
    #[must_use]
    pub fn compute_homogeneity(&self, labels_true: &[i32], labels_pred: &[i32]) -> f32 {
        let h_ck = self.conditional_entropy(labels_true, labels_pred);
        let h_c = self.entropy(labels_true);

        if h_c == 0.0 {
            1.0
        } else {
            1.0 - (h_ck / h_c)
        }
    }

    /// Compute completeness score.
    ///
    /// Completeness measures whether all members of a given class
    /// are assigned to the same cluster.
    #[must_use]
    pub fn compute_completeness(&self, labels_true: &[i32], labels_pred: &[i32]) -> f32 {
        let h_kc = self.conditional_entropy(labels_pred, labels_true);
        let h_k = self.entropy(labels_pred);

        if h_k == 0.0 {
            1.0
        } else {
            1.0 - (h_kc / h_k)
        }
    }

    /// Compute entropy H(labels).
    fn entropy(&self, labels: &[i32]) -> f32 {
        let n = labels.len() as f32;
        if n == 0.0 {
            return 0.0;
        }

        let mut counts: HashMap<i32, usize> = HashMap::new();
        for &label in labels {
            *counts.entry(label).or_insert(0) += 1;
        }

        let mut entropy = 0.0f32;
        for &count in counts.values() {
            let p = count as f32 / n;
            if p > 0.0 {
                entropy -= p * p.ln();
            }
        }

        entropy
    }

    /// Compute conditional entropy H(labels_a | labels_b).
    fn conditional_entropy(&self, labels_a: &[i32], labels_b: &[i32]) -> f32 {
        let n = labels_a.len() as f32;
        if n == 0.0 {
            return 0.0;
        }

        // Build contingency table
        let mut contingency: HashMap<(i32, i32), usize> = HashMap::new();
        let mut counts_b: HashMap<i32, usize> = HashMap::new();

        for (&a, &b) in labels_a.iter().zip(labels_b.iter()) {
            *contingency.entry((a, b)).or_insert(0) += 1;
            *counts_b.entry(b).or_insert(0) += 1;
        }

        let mut cond_entropy = 0.0f32;

        for (&(_a, b), &count_ab) in &contingency {
            let count_b = counts_b[&b];
            let p_ab = count_ab as f32 / n;
            let p_a_given_b = count_ab as f32 / count_b as f32;

            if p_a_given_b > 0.0 {
                cond_entropy -= p_ab * p_a_given_b.ln();
            }
        }

        cond_entropy
    }
}

impl Default for VMeasure {
    fn default() -> Self {
        Self::new()
    }
}

/// Sequence entropy calculator.
///
/// Computes various entropy measures for sequential data,
/// useful for analyzing vocalization patterns.
#[derive(Debug, Clone)]
pub struct SequenceEntropy {
    /// Base of logarithm (default: e for natural log).
    log_base: f32,
}

impl SequenceEntropy {
    /// Create a new sequence entropy calculator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            log_base: std::f32::consts::E,
        }
    }

    /// Create with log base 2 (bits).
    #[must_use]
    pub fn bits() -> Self {
        Self { log_base: 2.0 }
    }

    /// Create with custom log base.
    #[must_use]
    pub fn with_log_base(log_base: f32) -> Self {
        Self { log_base }
    }

    /// Compute Shannon entropy of a distribution.
    ///
    /// # Arguments
    ///
    /// * `probabilities` - Probability distribution (must sum to 1)
    ///
    /// # Returns
    ///
    /// Shannon entropy value.
    #[must_use]
    pub fn shannon_entropy(&self, probabilities: &[f32]) -> f32 {
        let mut entropy = 0.0f32;

        for &p in probabilities {
            if p > 0.0 {
                entropy -= p * self.log(p);
            }
        }

        entropy
    }

    /// Compute entropy from counts.
    ///
    /// # Arguments
    ///
    /// * `counts` - Count of each symbol
    ///
    /// # Returns
    ///
    /// Shannon entropy value.
    #[must_use]
    pub fn entropy_from_counts(&self, counts: &[usize]) -> f32 {
        let total: usize = counts.iter().sum();
        if total == 0 {
            return 0.0;
        }

        let probabilities: Vec<f32> = counts
            .iter()
            .map(|&c| c as f32 / total as f32)
            .collect();

        self.shannon_entropy(&probabilities)
    }

    /// Compute entropy directly from a sequence.
    ///
    /// # Arguments
    ///
    /// * `sequence` - Sequence of symbols
    ///
    /// # Returns
    ///
    /// Shannon entropy value.
    #[must_use]
    pub fn sequence_entropy<T: std::hash::Hash + Eq>(&self, sequence: &[T]) -> f32 {
        if sequence.is_empty() {
            return 0.0;
        }

        let mut counts: HashMap<&T, usize> = HashMap::new();
        for item in sequence {
            *counts.entry(item).or_insert(0) += 1;
        }

        let count_vec: Vec<usize> = counts.values().copied().collect();
        self.entropy_from_counts(&count_vec)
    }

    /// Compute normalized entropy (0 to 1).
    ///
    /// Normalized by maximum possible entropy for the alphabet size.
    #[must_use]
    pub fn normalized_entropy<T: std::hash::Hash + Eq>(&self, sequence: &[T]) -> f32 {
        if sequence.is_empty() {
            return 0.0;
        }

        let mut counts: HashMap<&T, usize> = HashMap::new();
        for item in sequence {
            *counts.entry(item).or_insert(0) += 1;
        }

        let alphabet_size = counts.len();
        if alphabet_size <= 1 {
            return 0.0;
        }

        let entropy = self.entropy_from_counts(&counts.values().copied().collect::<Vec<_>>());
        let max_entropy = self.log(alphabet_size as f32);

        if max_entropy > 0.0 {
            entropy / max_entropy
        } else {
            0.0
        }
    }

    /// Compute joint entropy H(X, Y).
    #[must_use]
    pub fn joint_entropy<T: std::hash::Hash + Eq, U: std::hash::Hash + Eq>(
        &self,
        sequence_x: &[T],
        sequence_y: &[U],
    ) -> f32 {
        if sequence_x.len() != sequence_y.len() || sequence_x.is_empty() {
            return 0.0;
        }

        let n = sequence_x.len() as f32;
        let mut joint_counts: HashMap<(&T, &U), usize> = HashMap::new();

        for (x, y) in sequence_x.iter().zip(sequence_y.iter()) {
            *joint_counts.entry((x, y)).or_insert(0) += 1;
        }

        let mut entropy = 0.0f32;
        for &count in joint_counts.values() {
            let p = count as f32 / n;
            if p > 0.0 {
                entropy -= p * self.log(p);
            }
        }

        entropy
    }

    /// Compute mutual information I(X; Y).
    #[must_use]
    pub fn mutual_information<T: std::hash::Hash + Eq + Clone, U: std::hash::Hash + Eq + Clone>(
        &self,
        sequence_x: &[T],
        sequence_y: &[U],
    ) -> f32 {
        let h_x = self.sequence_entropy(sequence_x);
        let h_y = self.sequence_entropy(sequence_y);
        let h_xy = self.joint_entropy(sequence_x, sequence_y);

        h_x + h_y - h_xy
    }

    /// Compute conditional entropy H(X | Y).
    #[must_use]
    pub fn conditional_entropy<T: std::hash::Hash + Eq, U: std::hash::Hash + Eq>(
        &self,
        sequence_x: &[T],
        sequence_y: &[U],
    ) -> f32 {
        let h_xy = self.joint_entropy(sequence_x, sequence_y);
        let h_y = self.sequence_entropy(sequence_y);

        h_xy - h_y
    }

    /// Compute n-gram entropy (entropy rate estimate).
    ///
    /// # Arguments
    ///
    /// * `sequence` - Input sequence
    /// * `n` - N-gram size
    ///
    /// # Returns
    ///
    /// N-gram entropy value.
    #[must_use]
    pub fn ngram_entropy<T: std::hash::Hash + Eq + Clone>(&self, sequence: &[T], n: usize) -> f32 {
        if sequence.len() < n || n == 0 {
            return 0.0;
        }

        let ngrams: Vec<Vec<&T>> = sequence
            .windows(n)
            .map(|w| w.iter().collect())
            .collect();

        let mut counts: HashMap<Vec<&T>, usize> = HashMap::new();
        for ngram in &ngrams {
            *counts.entry(ngram.clone()).or_insert(0) += 1;
        }

        let count_vec: Vec<usize> = counts.values().copied().collect();
        self.entropy_from_counts(&count_vec) / n as f32
    }

    /// Log with configured base.
    fn log(&self, x: f32) -> f32 {
        if self.log_base == std::f32::consts::E {
            x.ln()
        } else {
            x.ln() / self.log_base.ln()
        }
    }
}

impl Default for SequenceEntropy {
    fn default() -> Self {
        Self::new()
    }
}

/// Adjusted Rand Index calculator.
///
/// Measures the similarity between two clusterings, adjusted for chance.
/// Values range from -1 to 1, where 1 is perfect agreement.
#[derive(Debug, Clone, Default)]
pub struct AdjustedRandIndex;

impl AdjustedRandIndex {
    /// Create a new ARI calculator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Compute Adjusted Rand Index.
    ///
    /// # Arguments
    ///
    /// * `labels_true` - Ground truth labels
    /// * `labels_pred` - Predicted cluster labels
    ///
    /// # Returns
    ///
    /// ARI score (-1 to 1).
    #[must_use]
    pub fn compute(&self, labels_true: &[i32], labels_pred: &[i32]) -> f32 {
        if labels_true.len() != labels_pred.len() || labels_true.is_empty() {
            return 0.0;
        }

        let n = labels_true.len();

        // Build contingency table
        let mut contingency: HashMap<(i32, i32), usize> = HashMap::new();
        for (&t, &p) in labels_true.iter().zip(labels_pred.iter()) {
            *contingency.entry((t, p)).or_insert(0) += 1;
        }

        // Row and column sums
        let mut row_sums: HashMap<i32, usize> = HashMap::new();
        let mut col_sums: HashMap<i32, usize> = HashMap::new();

        for (&(t, p), &count) in &contingency {
            *row_sums.entry(t).or_insert(0) += count;
            *col_sums.entry(p).or_insert(0) += count;
        }

        // Compute index
        let mut sum_comb_n = 0i64;
        for &count in contingency.values() {
            sum_comb_n += self.comb2(count);
        }

        let mut sum_comb_a = 0i64;
        for &count in row_sums.values() {
            sum_comb_a += self.comb2(count);
        }

        let mut sum_comb_b = 0i64;
        for &count in col_sums.values() {
            sum_comb_b += self.comb2(count);
        }

        let comb_n_total = self.comb2(n);

        if comb_n_total == 0 {
            return 0.0;
        }

        let expected_index = (sum_comb_a * sum_comb_b) as f64 / comb_n_total as f64;
        let max_index = (sum_comb_a + sum_comb_b) as f64 / 2.0;

        if max_index == expected_index {
            return 0.0;
        }

        let ari = (sum_comb_n as f64 - expected_index) / (max_index - expected_index);
        ari as f32
    }

    /// Compute binomial coefficient C(n, 2).
    fn comb2(&self, n: usize) -> i64 {
        if n < 2 {
            0
        } else {
            (n * (n - 1) / 2) as i64
        }
    }
}

/// Davies-Bouldin Index calculator.
///
/// Measures the average similarity between each cluster and its most
/// similar cluster. Lower values indicate better clustering.
#[derive(Debug, Clone, Default)]
pub struct DaviesBouldinIndex;

impl DaviesBouldinIndex {
    /// Create a new DBI calculator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Compute Davies-Bouldin Index.
    ///
    /// # Arguments
    ///
    /// * `data` - 2D array where rows are samples
    /// * `labels` - Cluster labels for each sample
    /// * `centroids` - Cluster centroids
    ///
    /// # Returns
    ///
    /// DBI score (lower is better).
    #[must_use]
    pub fn compute(
        &self,
        data: &Array2<f32>,
        labels: &[i32],
        centroids: &Array2<f32>,
    ) -> f32 {
        let k = centroids.nrows();
        if k < 2 {
            return 0.0;
        }

        // Compute within-cluster scatter for each cluster
        let mut scatter = vec![0.0f32; k];
        let mut counts = vec![0usize; k];

        for (i, &label) in labels.iter().enumerate() {
            if label < 0 {
                continue;
            }
            let cluster_idx = label as usize;
            if cluster_idx >= k {
                continue;
            }

            let dist = self.euclidean_distance(data.row(i), centroids.row(cluster_idx));
            scatter[cluster_idx] += dist;
            counts[cluster_idx] += 1;
        }

        // Average scatter per cluster
        for i in 0..k {
            if counts[i] > 0 {
                scatter[i] /= counts[i] as f32;
            }
        }

        // Compute DBI
        let mut dbi_sum = 0.0f32;

        for i in 0..k {
            let mut max_ratio = 0.0f32;

            for j in 0..k {
                if i == j {
                    continue;
                }

                let centroid_dist =
                    self.euclidean_distance(centroids.row(i), centroids.row(j));

                if centroid_dist > 0.0 {
                    let ratio = (scatter[i] + scatter[j]) / centroid_dist;
                    max_ratio = max_ratio.max(ratio);
                }
            }

            dbi_sum += max_ratio;
        }

        dbi_sum / k as f32
    }

    /// Compute Euclidean distance between two vectors.
    fn euclidean_distance(&self, a: ArrayView1<f32>, b: ArrayView1<f32>) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }
}

/// Calinski-Harabasz Index calculator.
///
/// Also known as the Variance Ratio Criterion. Higher values
/// indicate better-defined clusters.
#[derive(Debug, Clone, Default)]
pub struct CalinskiHarabaszIndex;

impl CalinskiHarabaszIndex {
    /// Create a new CHI calculator.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Compute Calinski-Harabasz Index.
    ///
    /// # Arguments
    ///
    /// * `data` - 2D array where rows are samples
    /// * `labels` - Cluster labels for each sample
    /// * `centroids` - Cluster centroids
    ///
    /// # Returns
    ///
    /// CHI score (higher is better).
    #[must_use]
    pub fn compute(
        &self,
        data: &Array2<f32>,
        labels: &[i32],
        centroids: &Array2<f32>,
    ) -> f32 {
        let n = data.nrows();
        let k = centroids.nrows();

        if k < 2 || n <= k {
            return 0.0;
        }

        // Compute global centroid
        let global_centroid = data.mean_axis(Axis(0)).unwrap();

        // Compute between-cluster dispersion (BGSS)
        let mut bgss = 0.0f32;
        let mut counts = vec![0usize; k];

        for &label in labels {
            if label >= 0 && (label as usize) < k {
                counts[label as usize] += 1;
            }
        }

        for (i, centroid) in centroids.outer_iter().enumerate() {
            let dist_sq: f32 = centroid
                .iter()
                .zip(global_centroid.iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum();
            bgss += counts[i] as f32 * dist_sq;
        }

        // Compute within-cluster dispersion (WGSS)
        let mut wgss = 0.0f32;
        for (i, &label) in labels.iter().enumerate() {
            if label < 0 {
                continue;
            }
            let cluster_idx = label as usize;
            if cluster_idx >= k {
                continue;
            }

            let dist_sq: f32 = data
                .row(i)
                .iter()
                .zip(centroids.row(cluster_idx).iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum();
            wgss += dist_sq;
        }

        if wgss == 0.0 {
            return 0.0;
        }

        // CH Index = (BGSS / (k-1)) / (WGSS / (n-k))
        let chi = (bgss / (k - 1) as f32) / (wgss / (n - k) as f32);
        chi
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silhouette_score() {
        let scorer = SilhouetteScore::new();

        // Create simple 2-cluster data
        let data = Array2::from_shape_vec(
            (6, 2),
            vec![
                0.0, 0.0, 0.1, 0.1, 0.0, 0.1, // Cluster 0
                5.0, 5.0, 5.1, 5.1, 5.0, 5.1, // Cluster 1
            ],
        )
        .unwrap();

        let labels = vec![0, 0, 0, 1, 1, 1];
        let score = scorer.compute(&data, &labels);

        // Well-separated clusters should have high silhouette score
        assert!(score > 0.5);
    }

    #[test]
    fn test_v_measure() {
        let vm = VMeasure::new();

        // Perfect clustering
        let labels_true = vec![0, 0, 1, 1, 2, 2];
        let labels_pred = vec![0, 0, 1, 1, 2, 2];

        let (v, h, c) = vm.compute(&labels_true, &labels_pred);
        assert!((v - 1.0).abs() < 0.01);
        assert!((h - 1.0).abs() < 0.01);
        assert!((c - 1.0).abs() < 0.01);

        // Random clustering should have low V-measure
        let labels_random = vec![0, 1, 0, 1, 0, 1];
        let (v_rand, _, _) = vm.compute(&labels_true, &labels_random);
        assert!(v_rand < 0.5);
    }

    #[test]
    fn test_sequence_entropy() {
        let calc = SequenceEntropy::new();

        // Uniform distribution has max entropy
        let uniform = vec![0, 1, 2, 3, 0, 1, 2, 3];
        let e_uniform = calc.sequence_entropy(&uniform);

        // Constant sequence has zero entropy
        let constant = vec![0, 0, 0, 0, 0, 0, 0, 0];
        let e_constant = calc.sequence_entropy(&constant);

        assert!(e_uniform > 0.0);
        assert!(e_constant == 0.0);
    }

    #[test]
    fn test_normalized_entropy() {
        let calc = SequenceEntropy::new();

        // Uniform distribution should have normalized entropy close to 1
        let uniform = vec![0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3];
        let ne = calc.normalized_entropy(&uniform);
        assert!((ne - 1.0).abs() < 0.1);

        // Heavily skewed distribution should have lower normalized entropy than uniform
        // Using very extreme skew: 15 zeros, 1 one for entropy < 0.5
        let skewed = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        let ne_skewed = calc.normalized_entropy(&skewed);
        assert!(ne_skewed < ne, "Skewed distribution should have lower entropy than uniform");
    }

    #[test]
    fn test_adjusted_rand_index() {
        let ari = AdjustedRandIndex::new();

        // Perfect agreement
        let labels_true = vec![0, 0, 1, 1, 2, 2];
        let labels_pred = vec![0, 0, 1, 1, 2, 2];

        let score = ari.compute(&labels_true, &labels_pred);
        assert!((score - 1.0).abs() < 0.01);

        // Complete disagreement should have ARI close to 0 or negative
        let labels_random = vec![2, 1, 0, 2, 1, 0];
        let score_random = ari.compute(&labels_true, &labels_random);
        assert!(score_random < score);
    }

    #[test]
    fn test_davies_bouldin_index() {
        let dbi = DaviesBouldinIndex::new();

        // Well-separated clusters
        let data = Array2::from_shape_vec(
            (6, 2),
            vec![
                0.0, 0.0, 0.1, 0.0, 0.0, 0.1, // Cluster 0
                10.0, 10.0, 10.1, 10.0, 10.0, 10.1, // Cluster 1
            ],
        )
        .unwrap();

        let labels = vec![0, 0, 0, 1, 1, 1];
        let centroids = Array2::from_shape_vec(
            (2, 2),
            vec![0.033, 0.033, 10.033, 10.033],
        )
        .unwrap();

        let score = dbi.compute(&data, &labels, &centroids);
        // Well-separated clusters should have low DBI
        assert!(score < 1.0);
    }

    #[test]
    fn test_calinski_harabasz_index() {
        let chi = CalinskiHarabaszIndex::new();

        // Well-separated clusters
        let data = Array2::from_shape_vec(
            (6, 2),
            vec![
                0.0, 0.0, 0.1, 0.0, 0.0, 0.1, // Cluster 0
                10.0, 10.0, 10.1, 10.0, 10.0, 10.1, // Cluster 1
            ],
        )
        .unwrap();

        let labels = vec![0, 0, 0, 1, 1, 1];
        let centroids = Array2::from_shape_vec(
            (2, 2),
            vec![0.033, 0.033, 10.033, 10.033],
        )
        .unwrap();

        let score = chi.compute(&data, &labels, &centroids);
        // Well-separated clusters should have high CHI
        assert!(score > 10.0);
    }

    #[test]
    fn test_ngram_entropy() {
        let calc = SequenceEntropy::new();

        // Periodic sequence should have lower n-gram entropy
        let periodic = vec![0, 1, 0, 1, 0, 1, 0, 1];
        let random = vec![0, 1, 1, 0, 0, 1, 0, 0];

        let e_periodic = calc.ngram_entropy(&periodic, 2);
        let e_random = calc.ngram_entropy(&random, 2);

        // Both should be positive
        assert!(e_periodic > 0.0);
        assert!(e_random > 0.0);
    }

    #[test]
    fn test_mutual_information() {
        let calc = SequenceEntropy::new();

        // Identical sequences should have high MI
        let seq = vec![0, 1, 2, 0, 1, 2];
        let mi_same = calc.mutual_information(&seq, &seq);

        // Different sequences should have lower MI
        let seq2 = vec![2, 1, 0, 2, 1, 0];
        let mi_diff = calc.mutual_information(&seq, &seq2);

        assert!(mi_same > 0.0);
        // MI with itself equals entropy
        let entropy = calc.sequence_entropy(&seq);
        assert!((mi_same - entropy).abs() < 0.01);
    }

    #[test]
    fn test_clustering_metrics() {
        let metrics = ClusteringMetrics::new()
            .with_silhouette(0.8)
            .with_v_measure(0.9)
            .with_n_clusters(5);

        assert!(metrics.is_acceptable(0.5, 0.5));
        assert!(!metrics.is_acceptable(0.9, 0.9));
    }
}
