//! Stage 3: BSSID spatial correlation via GNN message passing.
//!
//! Builds a cross-correlation graph where nodes are BSSIDs and edges
//! represent temporal cross-correlation between their RSSI histories.
//! A single message-passing step identifies co-varying BSSID clusters
//! that are likely affected by the same person.

/// BSSID correlator that computes pairwise Pearson correlation
/// and identifies co-varying clusters.
///
/// Note: The full `RuvectorLayer` GNN requires matching dimension
/// weights trained on CSI data. For Phase 2 we use a lightweight
/// correlation-based approach that can be upgraded to GNN later.
pub struct BssidCorrelator {
    /// Per-BSSID history buffers for correlation computation.
    histories: Vec<Vec<f32>>,
    /// Maximum history length.
    window: usize,
    /// Number of tracked BSSIDs.
    n_bssids: usize,
    /// Correlation threshold for "co-varying" classification.
    correlation_threshold: f32,
}

impl BssidCorrelator {
    /// Create a new correlator.
    ///
    /// - `n_bssids`: number of BSSID slots.
    /// - `window`: correlation window size (number of frames).
    /// - `correlation_threshold`: minimum |r| to consider BSSIDs co-varying.
    #[must_use]
    pub fn new(n_bssids: usize, window: usize, correlation_threshold: f32) -> Self {
        Self {
            histories: vec![Vec::with_capacity(window); n_bssids],
            window,
            n_bssids,
            correlation_threshold,
        }
    }

    /// Push a new frame of amplitudes and compute correlation features.
    ///
    /// Returns a `CorrelationResult` with the correlation matrix and
    /// cluster assignments.
    pub fn update(&mut self, amplitudes: &[f32]) -> CorrelationResult {
        let n = amplitudes.len().min(self.n_bssids);

        // Update histories
        for (i, &amp) in amplitudes.iter().enumerate().take(n) {
            let hist = &mut self.histories[i];
            hist.push(amp);
            if hist.len() > self.window {
                hist.remove(0);
            }
        }

        // Compute pairwise Pearson correlation
        let mut corr_matrix = vec![vec![0.0f32; n]; n];
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            corr_matrix[i][i] = 1.0;
            for j in (i + 1)..n {
                let r = pearson_r(&self.histories[i], &self.histories[j]);
                corr_matrix[i][j] = r;
                corr_matrix[j][i] = r;
            }
        }

        // Find strongly correlated clusters (simple union-find)
        let clusters = self.find_clusters(&corr_matrix, n);

        // Compute per-BSSID "spatial diversity" score:
        // how many other BSSIDs is each one correlated with
        #[allow(clippy::cast_precision_loss)]
        let diversity: Vec<f32> = (0..n)
            .map(|i| {
                let count = (0..n)
                    .filter(|&j| j != i && corr_matrix[i][j].abs() > self.correlation_threshold)
                    .count();
                count as f32 / (n.max(1) - 1) as f32
            })
            .collect();

        CorrelationResult {
            matrix: corr_matrix,
            clusters,
            diversity,
            n_active: n,
        }
    }

    /// Simple cluster assignment via thresholded correlation.
    fn find_clusters(&self, corr: &[Vec<f32>], n: usize) -> Vec<usize> {
        let mut cluster_id = vec![0usize; n];
        let mut next_cluster = 0usize;
        let mut assigned = vec![false; n];

        for i in 0..n {
            if assigned[i] {
                continue;
            }
            cluster_id[i] = next_cluster;
            assigned[i] = true;

            // BFS: assign same cluster to correlated BSSIDs
            let mut queue = vec![i];
            while let Some(current) = queue.pop() {
                for j in 0..n {
                    if !assigned[j] && corr[current][j].abs() > self.correlation_threshold {
                        cluster_id[j] = next_cluster;
                        assigned[j] = true;
                        queue.push(j);
                    }
                }
            }
            next_cluster += 1;
        }
        cluster_id
    }

    /// Reset all correlation histories.
    pub fn reset(&mut self) {
        for h in &mut self.histories {
            h.clear();
        }
    }
}

/// Result of correlation analysis.
#[derive(Debug, Clone)]
pub struct CorrelationResult {
    /// n x n Pearson correlation matrix.
    pub matrix: Vec<Vec<f32>>,
    /// Cluster assignment per BSSID.
    pub clusters: Vec<usize>,
    /// Per-BSSID spatial diversity score [0, 1].
    pub diversity: Vec<f32>,
    /// Number of active BSSIDs in this frame.
    pub n_active: usize,
}

impl CorrelationResult {
    /// Number of distinct clusters.
    #[must_use]
    pub fn n_clusters(&self) -> usize {
        self.clusters.iter().copied().max().map_or(0, |m| m + 1)
    }

    /// Mean absolute correlation (proxy for signal coherence).
    #[must_use]
    pub fn mean_correlation(&self) -> f32 {
        if self.n_active < 2 {
            return 0.0;
        }
        let mut sum = 0.0f32;
        let mut count = 0;
        for i in 0..self.n_active {
            for j in (i + 1)..self.n_active {
                sum += self.matrix[i][j].abs();
                count += 1;
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let mean = if count == 0 { 0.0 } else { sum / count as f32 };
        mean
    }
}

/// Pearson correlation coefficient between two equal-length slices.
#[allow(clippy::cast_precision_loss)]
fn pearson_r(x: &[f32], y: &[f32]) -> f32 {
    let n = x.len().min(y.len());
    if n < 2 {
        return 0.0;
    }
    let n_f = n as f32;

    let mean_x: f32 = x.iter().take(n).sum::<f32>() / n_f;
    let mean_y: f32 = y.iter().take(n).sum::<f32>() / n_f;

    let mut cov = 0.0f32;
    let mut var_x = 0.0f32;
    let mut var_y = 0.0f32;

    for i in 0..n {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    let denom = (var_x * var_y).sqrt();
    if denom < 1e-12 {
        0.0
    } else {
        cov / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pearson_perfect_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r = pearson_r(&x, &y);
        assert!((r - 1.0).abs() < 1e-5, "perfect positive correlation: {r}");
    }

    #[test]
    fn pearson_negative_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![10.0, 8.0, 6.0, 4.0, 2.0];
        let r = pearson_r(&x, &y);
        assert!((r - (-1.0)).abs() < 1e-5, "perfect negative correlation: {r}");
    }

    #[test]
    fn pearson_no_correlation() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![5.0, 1.0, 4.0, 2.0, 3.0]; // shuffled
        let r = pearson_r(&x, &y);
        assert!(r.abs() < 0.5, "low correlation expected: {r}");
    }

    #[test]
    fn correlator_basic_update() {
        let mut corr = BssidCorrelator::new(3, 10, 0.7);
        // Push several identical frames
        for _ in 0..5 {
            corr.update(&[1.0, 2.0, 3.0]);
        }
        let result = corr.update(&[1.0, 2.0, 3.0]);
        assert_eq!(result.n_active, 3);
    }

    #[test]
    fn correlator_detects_covarying_bssids() {
        let mut corr = BssidCorrelator::new(3, 20, 0.8);
        // BSSID 0 and 1 co-vary, BSSID 2 is independent
        for i in 0..20 {
            let v = i as f32;
            corr.update(&[v, v * 2.0, 5.0]); // 0 and 1 correlate, 2 is constant
        }
        let result = corr.update(&[20.0, 40.0, 5.0]);
        // BSSIDs 0 and 1 should be in the same cluster
        assert_eq!(
            result.clusters[0], result.clusters[1],
            "co-varying BSSIDs should cluster: {:?}",
            result.clusters
        );
    }

    #[test]
    fn mean_correlation_zero_for_one_bssid() {
        let result = CorrelationResult {
            matrix: vec![vec![1.0]],
            clusters: vec![0],
            diversity: vec![0.0],
            n_active: 1,
        };
        assert!((result.mean_correlation() - 0.0).abs() < 1e-5);
    }
}
