//! Spectral Coherence Score for graph index health monitoring.
//!
//! Provides a composite metric measuring structural health of graph indices
//! using spectral graph theory properties. Self-contained, no external solver deps.

use serde::{Deserialize, Serialize};

/// Compressed Sparse Row matrix for Laplacian representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsrMatrixView {
    pub row_ptr: Vec<usize>,
    pub col_indices: Vec<usize>,
    pub values: Vec<f64>,
    pub rows: usize,
    pub cols: usize,
}

impl CsrMatrixView {
    pub fn new(
        row_ptr: Vec<usize>,
        col_indices: Vec<usize>,
        values: Vec<f64>,
        rows: usize,
        cols: usize,
    ) -> Self {
        Self {
            row_ptr,
            col_indices,
            values,
            rows,
            cols,
        }
    }

    /// Build a symmetric adjacency CSR matrix from edges `(u, v, weight)`.
    pub fn from_edges(n: usize, edges: &[(usize, usize, f64)]) -> Self {
        let mut entries: Vec<(usize, usize, f64)> = Vec::with_capacity(edges.len() * 2);
        for &(u, v, w) in edges {
            entries.push((u, v, w));
            if u != v {
                entries.push((v, u, w));
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        Self::from_sorted_entries(n, &entries)
    }

    /// Sparse matrix-vector product: y = A * x.
    pub fn spmv(&self, x: &[f64]) -> Vec<f64> {
        let mut y = vec![0.0; self.rows];
        for i in 0..self.rows {
            let (start, end) = (self.row_ptr[i], self.row_ptr[i + 1]);
            y[i] = (start..end)
                .map(|j| self.values[j] * x[self.col_indices[j]])
                .sum();
        }
        y
    }

    /// Build the graph Laplacian L = D - A from edges.
    pub fn build_laplacian(n: usize, edges: &[(usize, usize, f64)]) -> Self {
        let mut degree = vec![0.0_f64; n];
        let mut entries: Vec<(usize, usize, f64)> = Vec::with_capacity(edges.len() * 2 + n);
        for &(u, v, w) in edges {
            degree[u] += w;
            if u != v {
                degree[v] += w;
                entries.push((u, v, -w));
                entries.push((v, u, -w));
            }
        }
        for i in 0..n {
            entries.push((i, i, degree[i]));
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        Self::from_sorted_entries(n, &entries)
    }

    fn from_sorted_entries(n: usize, entries: &[(usize, usize, f64)]) -> Self {
        let mut row_ptr = vec![0usize; n + 1];
        let mut col_indices = Vec::with_capacity(entries.len());
        let mut values = Vec::with_capacity(entries.len());
        for &(r, c, v) in entries {
            row_ptr[r + 1] += 1;
            col_indices.push(c);
            values.push(v);
        }
        for i in 0..n {
            row_ptr[i + 1] += row_ptr[i];
        }
        Self {
            row_ptr,
            col_indices,
            values,
            rows: n,
            cols: n,
        }
    }
}

/// Configuration for spectral coherence computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralConfig {
    pub alpha: f64,               // Fiedler weight (default 0.3)
    pub beta: f64,                // Spectral gap weight (default 0.3)
    pub gamma: f64,               // Effective resistance weight (default 0.2)
    pub delta: f64,               // Degree regularity weight (default 0.2)
    pub max_iterations: usize,    // Power iteration max (default 50)
    pub tolerance: f64,           // Convergence tolerance (default 1e-6)
    pub refresh_threshold: usize, // Updates before full recompute (default 100)
}

impl Default for SpectralConfig {
    fn default() -> Self {
        Self {
            alpha: 0.3,
            beta: 0.3,
            gamma: 0.2,
            delta: 0.2,
            max_iterations: 50,
            tolerance: 1e-6,
            refresh_threshold: 100,
        }
    }
}

/// Composite spectral coherence score with individual components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectralCoherenceScore {
    pub fiedler: f64,              // Normalized Fiedler value [0,1]
    pub spectral_gap: f64,         // Spectral gap ratio [0,1]
    pub effective_resistance: f64, // Effective resistance score [0,1]
    pub degree_regularity: f64,    // Degree regularity score [0,1]
    pub composite: f64,            // Weighted composite SCS [0,1]
}

// --- Internal helpers ---

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm(v: &[f64]) -> f64 {
    dot(v, v).sqrt()
}

/// CG solve for L*x = b with null-space deflation (L is graph Laplacian).
fn cg_solve(lap: &CsrMatrixView, b: &[f64], max_iter: usize, tol: f64) -> Vec<f64> {
    let n = lap.rows;
    let inv_n = 1.0 / n as f64;
    let b_mean: f64 = b.iter().sum::<f64>() * inv_n;
    let b_def: Vec<f64> = b.iter().map(|v| v - b_mean).collect();
    let mut x = vec![0.0; n];
    let mut r = b_def.clone();
    let mut p = r.clone();
    let mut rs_old = dot(&r, &r);
    if rs_old < tol * tol {
        return x;
    }
    for _ in 0..max_iter {
        let mut ap = lap.spmv(&p);
        let ap_mean: f64 = ap.iter().sum::<f64>() * inv_n;
        ap.iter_mut().for_each(|v| *v -= ap_mean);
        let pap = dot(&p, &ap);
        if pap.abs() < 1e-30 {
            break;
        }
        let alpha = rs_old / pap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rs_new = dot(&r, &r);
        if rs_new.sqrt() < tol {
            break;
        }
        let beta = rs_new / rs_old;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }
        rs_old = rs_new;
    }
    x
}

/// Deflate vector: remove component along all-ones, then normalize.
fn deflate_and_normalize(v: &mut Vec<f64>) {
    let n = v.len();
    let inv_sqrt_n = 1.0 / (n as f64).sqrt();
    let proj: f64 = v.iter().sum::<f64>() * inv_sqrt_n;
    v.iter_mut().for_each(|x| *x -= proj * inv_sqrt_n);
    let n2 = norm(v);
    if n2 > 1e-30 {
        v.iter_mut().for_each(|x| *x /= n2);
    }
}

/// Estimate the Fiedler value (second smallest eigenvalue) and eigenvector
/// using inverse iteration with null-space deflation.
pub fn estimate_fiedler(lap: &CsrMatrixView, max_iter: usize, tol: f64) -> (f64, Vec<f64>) {
    let n = lap.rows;
    if n <= 1 {
        return (0.0, vec![0.0; n]);
    }
    // Initial vector orthogonal to all-ones.
    let mut v: Vec<f64> = (0..n).map(|i| i as f64 - (n as f64 - 1.0) / 2.0).collect();
    deflate_and_normalize(&mut v);
    let mut eigenvalue = 0.0;
    // Use fewer outer iterations (convergence is typically fast for inverse iteration)
    let outer = max_iter.min(8);
    // Inner CG iterations: enough for approximate solve
    let inner = max_iter.min(15);
    for _ in 0..outer {
        let mut w = cg_solve(lap, &v, inner, tol * 0.1);
        deflate_and_normalize(&mut w);
        if norm(&w) < 1e-30 {
            break;
        }
        let lv = lap.spmv(&w);
        eigenvalue = dot(&w, &lv);
        let residual: f64 = lv
            .iter()
            .zip(w.iter())
            .map(|(li, wi)| (li - eigenvalue * wi).powi(2))
            .sum::<f64>()
            .sqrt();
        v = w;
        if residual < tol {
            break;
        }
    }
    (eigenvalue.max(0.0), v)
}

/// Estimate the largest eigenvalue of the Laplacian via power iteration.
pub fn estimate_largest_eigenvalue(lap: &CsrMatrixView, max_iter: usize) -> f64 {
    let n = lap.rows;
    if n == 0 {
        return 0.0;
    }
    let mut v = vec![1.0 / (n as f64).sqrt(); n];
    let mut ev = 0.0;
    // Power iteration converges fast for the largest eigenvalue
    let iters = max_iter.min(10);
    for _ in 0..iters {
        let w = lap.spmv(&v);
        let wn = norm(&w);
        if wn < 1e-30 {
            return 0.0;
        }
        ev = dot(&v, &w);
        v.iter_mut()
            .zip(w.iter())
            .for_each(|(vi, wi)| *vi = wi / wn);
    }
    ev.max(0.0)
}

/// Spectral gap ratio: fiedler / largest eigenvalue.
pub fn estimate_spectral_gap(fiedler: f64, largest: f64) -> f64 {
    if largest < 1e-30 {
        0.0
    } else {
        (fiedler / largest).clamp(0.0, 1.0)
    }
}

/// Degree regularity: 1 - (std_dev / mean) of vertex degrees. 1.0 = perfectly regular.
pub fn compute_degree_regularity(lap: &CsrMatrixView) -> f64 {
    let n = lap.rows;
    if n == 0 {
        return 1.0;
    }
    let degrees: Vec<f64> = (0..n)
        .map(|i| {
            let (s, e) = (lap.row_ptr[i], lap.row_ptr[i + 1]);
            (s..e)
                .find(|&j| lap.col_indices[j] == i)
                .map_or(0.0, |j| lap.values[j])
        })
        .collect();
    let mean = degrees.iter().sum::<f64>() / n as f64;
    if mean < 1e-30 {
        return 1.0;
    }
    let std = (degrees.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / n as f64).sqrt();
    (1.0 - std / mean).clamp(0.0, 1.0)
}

/// Estimate average effective resistance by deterministic sampling of vertex pairs.
pub fn estimate_effective_resistance_sampled(lap: &CsrMatrixView, n_samples: usize) -> f64 {
    let n = lap.rows;
    if n < 2 {
        return 0.0;
    }
    let total_pairs = n * (n - 1) / 2;
    let step = if total_pairs <= n_samples {
        1
    } else {
        total_pairs / n_samples
    };
    let max_s = n_samples.min(total_pairs);
    // Fewer CG iterations for resistance estimation (approximate is fine)
    let cg_iters = 10;
    let (mut total, mut sampled, mut idx) = (0.0, 0usize, 0usize);
    'outer: for u in 0..n {
        for v in (u + 1)..n {
            if idx % step == 0 {
                let mut rhs = vec![0.0; n];
                rhs[u] = 1.0;
                rhs[v] = -1.0;
                let x = cg_solve(lap, &rhs, cg_iters, 1e-6);
                total += (x[u] - x[v]).abs();
                sampled += 1;
                if sampled >= max_s {
                    break 'outer;
                }
            }
            idx += 1;
        }
    }
    if sampled == 0 {
        0.0
    } else {
        total / sampled as f64
    }
}

/// Tracks spectral coherence incrementally, recomputing fully when needed.
pub struct SpectralTracker {
    config: SpectralConfig,
    fiedler_estimate: f64,
    gap_estimate: f64,
    resistance_estimate: f64,
    regularity: f64,
    updates_since_refresh: usize,
    fiedler_vector: Option<Vec<f64>>,
}

impl SpectralTracker {
    pub fn new(config: SpectralConfig) -> Self {
        Self {
            config,
            fiedler_estimate: 0.0,
            gap_estimate: 0.0,
            resistance_estimate: 0.0,
            regularity: 1.0,
            updates_since_refresh: 0,
            fiedler_vector: None,
        }
    }

    /// Full spectral computation from a Laplacian.
    pub fn compute(&mut self, lap: &CsrMatrixView) -> SpectralCoherenceScore {
        self.full_recompute(lap);
        self.build_score()
    }

    /// Incremental update using first-order perturbation: delta_lambda ~= v^T(delta_L)v.
    pub fn update_edge(&mut self, lap: &CsrMatrixView, u: usize, v: usize, weight_delta: f64) {
        self.updates_since_refresh += 1;
        if self.needs_refresh() || self.fiedler_vector.is_none() {
            self.full_recompute(lap);
            return;
        }
        if let Some(ref fv) = self.fiedler_vector {
            if u < fv.len() && v < fv.len() {
                let diff = fv[u] - fv[v];
                self.fiedler_estimate =
                    (self.fiedler_estimate + weight_delta * diff * diff).max(0.0);
                let largest = estimate_largest_eigenvalue(lap, self.config.max_iterations);
                self.gap_estimate = estimate_spectral_gap(self.fiedler_estimate, largest);
            }
        }
        self.regularity = compute_degree_regularity(lap);
    }

    pub fn score(&self) -> f64 {
        self.build_score().composite
    }

    pub fn full_recompute(&mut self, lap: &CsrMatrixView) {
        let (fiedler_raw, fv) =
            estimate_fiedler(lap, self.config.max_iterations, self.config.tolerance);
        let largest = estimate_largest_eigenvalue(lap, self.config.max_iterations);
        let n = lap.rows;
        self.fiedler_estimate = if n > 0 {
            (fiedler_raw / n as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.gap_estimate = estimate_spectral_gap(fiedler_raw, largest);
        let r_raw = estimate_effective_resistance_sampled(lap, 3.min(n * (n - 1) / 2));
        self.resistance_estimate = 1.0 / (1.0 + r_raw);
        self.regularity = compute_degree_regularity(lap);
        self.fiedler_vector = Some(fv);
        self.updates_since_refresh = 0;
    }

    pub fn needs_refresh(&self) -> bool {
        self.updates_since_refresh >= self.config.refresh_threshold
    }

    fn build_score(&self) -> SpectralCoherenceScore {
        let c = self.config.alpha * self.fiedler_estimate
            + self.config.beta * self.gap_estimate
            + self.config.gamma * self.resistance_estimate
            + self.config.delta * self.regularity;
        SpectralCoherenceScore {
            fiedler: self.fiedler_estimate,
            spectral_gap: self.gap_estimate,
            effective_resistance: self.resistance_estimate,
            degree_regularity: self.regularity,
            composite: c.clamp(0.0, 1.0),
        }
    }
}

/// Alert types for graph index health degradation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthAlert {
    FragileIndex { fiedler: f64 },
    PoorExpansion { gap: f64 },
    HighResistance { resistance: f64 },
    LowCoherence { scs: f64 },
    RebuildRecommended { reason: String },
}

/// Health monitor for HNSW graph indices using spectral coherence.
pub struct HnswHealthMonitor {
    tracker: SpectralTracker,
    min_fiedler: f64,
    min_spectral_gap: f64,
    max_resistance: f64,
    min_composite_scs: f64,
}

impl HnswHealthMonitor {
    pub fn new(config: SpectralConfig) -> Self {
        Self {
            tracker: SpectralTracker::new(config),
            min_fiedler: 0.05,
            min_spectral_gap: 0.01,
            max_resistance: 0.95,
            min_composite_scs: 0.3,
        }
    }

    pub fn update(&mut self, lap: &CsrMatrixView, edge_change: Option<(usize, usize, f64)>) {
        match edge_change {
            Some((u, v, d)) => self.tracker.update_edge(lap, u, v, d),
            None => self.tracker.full_recompute(lap),
        }
    }

    pub fn check_health(&self) -> Vec<HealthAlert> {
        let s = self.tracker.build_score();
        let mut alerts = Vec::new();
        if s.fiedler < self.min_fiedler {
            alerts.push(HealthAlert::FragileIndex { fiedler: s.fiedler });
        }
        if s.spectral_gap < self.min_spectral_gap {
            alerts.push(HealthAlert::PoorExpansion {
                gap: s.spectral_gap,
            });
        }
        if s.effective_resistance > self.max_resistance {
            alerts.push(HealthAlert::HighResistance {
                resistance: s.effective_resistance,
            });
        }
        if s.composite < self.min_composite_scs {
            alerts.push(HealthAlert::LowCoherence { scs: s.composite });
        }
        if alerts.len() >= 2 {
            alerts.push(HealthAlert::RebuildRecommended {
                reason: format!(
                    "{} health issues detected. Full rebuild recommended.",
                    alerts.len()
                ),
            });
        }
        alerts
    }

    pub fn score(&self) -> SpectralCoherenceScore {
        self.tracker.build_score()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> Vec<(usize, usize, f64)> {
        vec![(0, 1, 1.0), (1, 2, 1.0), (0, 2, 1.0)]
    }
    fn path4() -> Vec<(usize, usize, f64)> {
        vec![(0, 1, 1.0), (1, 2, 1.0), (2, 3, 1.0)]
    }
    fn cycle4() -> Vec<(usize, usize, f64)> {
        vec![(0, 1, 1.0), (1, 2, 1.0), (2, 3, 1.0), (3, 0, 1.0)]
    }

    #[test]
    fn test_laplacian_construction() {
        let lap = CsrMatrixView::build_laplacian(3, &triangle());
        assert_eq!(lap.rows, 3);
        for i in 0..3 {
            let (s, e) = (lap.row_ptr[i], lap.row_ptr[i + 1]);
            let row_sum: f64 = lap.values[s..e].iter().sum();
            assert!(row_sum.abs() < 1e-10, "Row {} sum = {}", i, row_sum);
            let diag = (s..e)
                .find(|&j| lap.col_indices[j] == i)
                .map(|j| lap.values[j])
                .unwrap();
            assert!((diag - 2.0).abs() < 1e-10, "Diag[{}] = {}", i, diag);
        }
    }

    #[test]
    fn test_fiedler_value_triangle() {
        // K3 eigenvalues: 0, 3, 3. Fiedler = 3.0.
        let lap = CsrMatrixView::build_laplacian(3, &triangle());
        let (f, _) = estimate_fiedler(&lap, 200, 1e-8);
        assert!(
            (f - 3.0).abs() < 0.15,
            "Triangle Fiedler = {} (expected ~3.0)",
            f
        );
    }

    #[test]
    fn test_fiedler_value_path() {
        // P4 eigenvalues: 0, 2-sqrt(2), 2, 2+sqrt(2). Fiedler ~= 0.5858.
        let lap = CsrMatrixView::build_laplacian(4, &path4());
        let (f, _) = estimate_fiedler(&lap, 200, 1e-8);
        let expected = 2.0 - std::f64::consts::SQRT_2;
        assert!(
            (f - expected).abs() < 0.15,
            "Path Fiedler = {} (expected ~{})",
            f,
            expected
        );
    }

    #[test]
    fn test_degree_regularity_regular_graph() {
        let lap = CsrMatrixView::build_laplacian(4, &cycle4());
        assert!((compute_degree_regularity(&lap) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_scs_bounds() {
        let mut t = SpectralTracker::new(SpectralConfig::default());
        let s = t.compute(&CsrMatrixView::build_laplacian(4, &cycle4()));
        assert!(s.composite >= 0.0 && s.composite <= 1.0);
        assert!(s.fiedler >= 0.0 && s.fiedler <= 1.0);
        assert!(s.spectral_gap >= 0.0 && s.spectral_gap <= 1.0);
        assert!(s.effective_resistance >= 0.0 && s.effective_resistance <= 1.0);
        assert!(s.degree_regularity >= 0.0 && s.degree_regularity <= 1.0);
    }

    #[test]
    fn test_scs_monotonicity() {
        let full = vec![
            (0, 1, 1.0),
            (0, 2, 1.0),
            (0, 3, 1.0),
            (1, 2, 1.0),
            (1, 3, 1.0),
            (2, 3, 1.0),
        ];
        let sparse = vec![(0, 1, 1.0), (1, 2, 1.0), (2, 3, 1.0)];
        let mut tf = SpectralTracker::new(SpectralConfig::default());
        let mut ts = SpectralTracker::new(SpectralConfig::default());
        let sf = tf.compute(&CsrMatrixView::build_laplacian(4, &full));
        let ss = ts.compute(&CsrMatrixView::build_laplacian(4, &sparse));
        assert!(
            sf.composite >= ss.composite,
            "Full {} < sparse {}",
            sf.composite,
            ss.composite
        );
    }

    #[test]
    fn test_tracker_incremental() {
        let edges = vec![
            (0, 1, 1.0),
            (1, 2, 1.0),
            (2, 3, 1.0),
            (3, 0, 1.0),
            (0, 2, 1.0),
            (1, 3, 1.0),
        ];
        let mut tracker = SpectralTracker::new(SpectralConfig::default());
        let lap = CsrMatrixView::build_laplacian(4, &edges);
        tracker.compute(&lap);

        // Small perturbation for accurate first-order approximation.
        let delta = 0.05;
        let updated: Vec<_> = edges
            .iter()
            .map(|&(u, v, w)| {
                if u == 1 && v == 3 {
                    (u, v, w + delta)
                } else {
                    (u, v, w)
                }
            })
            .collect();
        let lap_u = CsrMatrixView::build_laplacian(4, &updated);
        tracker.update_edge(&lap_u, 1, 3, delta);
        let si = tracker.score();

        let mut tf = SpectralTracker::new(SpectralConfig::default());
        let sf = tf.compute(&lap_u).composite;
        let diff = (si - sf).abs();
        assert!(
            diff < 0.5 * sf.max(0.01),
            "Incremental {} vs full {} (diff {})",
            si,
            sf,
            diff
        );

        // Verify forced refresh matches full recompute closely.
        let mut tr = SpectralTracker::new(SpectralConfig {
            refresh_threshold: 1,
            ..Default::default()
        });
        tr.compute(&lap);
        tr.updates_since_refresh = 1;
        tr.update_edge(&lap_u, 1, 3, delta);
        assert!(
            (tr.score() - sf).abs() < 0.05,
            "Refreshed {} vs full {}",
            tr.score(),
            sf
        );
    }

    #[test]
    fn test_health_alerts() {
        let weak = vec![(0, 1, 0.01), (1, 2, 0.01)];
        let mut m = HnswHealthMonitor::new(SpectralConfig::default());
        m.update(&CsrMatrixView::build_laplacian(3, &weak), None);
        let alerts = m.check_health();
        assert!(
            alerts.iter().any(|a| matches!(
                a,
                HealthAlert::FragileIndex { .. } | HealthAlert::LowCoherence { .. }
            )),
            "Weak graph should trigger alerts. Got: {:?}",
            alerts
        );
        let mut ms = HnswHealthMonitor::new(SpectralConfig::default());
        ms.update(&CsrMatrixView::build_laplacian(3, &triangle()), None);
        assert!(ms.check_health().len() <= alerts.len());
    }
}
