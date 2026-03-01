//! PostgreSQL operator functions for math distances and spectral methods.

use pgrx::prelude::*;
use pgrx::JsonB;

use ruvector_math::optimal_transport::GromovWasserstein;
use ruvector_math::optimal_transport::{OptimalTransport, SinkhornSolver, SlicedWasserstein};
use ruvector_math::product_manifold::ProductManifold;
use ruvector_math::spectral::{GraphFilter, ScaledLaplacian, SpectralClustering, SpectralFilter};
use ruvector_math::spherical::SphericalSpace;

/// Helper: parse a JsonB 2D array into Vec<Vec<f64>>.
fn parse_points(json: &JsonB) -> Vec<Vec<f64>> {
    json.0
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    v.as_array()
                        .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Helper: parse a JsonB 2D array into Vec<Vec<f64>> representing an adjacency/cost matrix.
fn parse_matrix(json: &JsonB) -> Vec<Vec<f64>> {
    parse_points(json)
}

/// Helper: flatten a Vec<Vec<f64>> adjacency matrix into (flat Vec<f64>, n).
fn flatten_adjacency(adj: &[Vec<f64>]) -> (Vec<f64>, usize) {
    let n = adj.len();
    let mut flat = vec![0.0; n * n];
    for (i, row) in adj.iter().enumerate() {
        for (j, &val) in row.iter().enumerate() {
            if j < n {
                flat[i * n + j] = val;
            }
        }
    }
    (flat, n)
}

/// Compute Wasserstein (Earth Mover's) distance between two distributions.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_wasserstein_distance(a: Vec<f32>, b: Vec<f32>, p: default!(i32, 1)) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        pgrx::error!("Distributions must have same non-zero length");
    }

    // 1D Wasserstein: sort and compute L_p distance of CDFs
    let mut a_sorted: Vec<f64> = a.iter().map(|&x| x as f64).collect();
    let mut b_sorted: Vec<f64> = b.iter().map(|&x| x as f64).collect();
    a_sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());
    b_sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());

    let p_f64 = p.max(1) as f64;
    let sum: f64 = a_sorted
        .iter()
        .zip(b_sorted.iter())
        .map(|(x, y)| (x - y).abs().powf(p_f64))
        .sum();

    (sum / a.len() as f64).powf(1.0 / p_f64) as f32
}

/// Compute Sinkhorn optimal transport distance with transport plan.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_sinkhorn_distance(
    cost_json: JsonB,
    w_a: Vec<f32>,
    w_b: Vec<f32>,
    reg: default!(f32, 0.1),
) -> JsonB {
    let cost = parse_matrix(&cost_json);
    if cost.is_empty() {
        pgrx::error!("Cost matrix is empty");
    }

    let wa: Vec<f64> = w_a.iter().map(|&x| x as f64).collect();
    let wb: Vec<f64> = w_b.iter().map(|&x| x as f64).collect();

    let solver = SinkhornSolver::new(reg as f64, 100);
    match solver.solve(&cost, &wa, &wb) {
        Ok(result) => JsonB(serde_json::json!({
            "distance": result.cost,
            "converged": result.converged,
            "iterations": result.iterations,
            "transport_plan": result.plan,
        })),
        Err(e) => pgrx::error!("Sinkhorn failed: {}", e),
    }
}

/// Compute Sliced Wasserstein distance between two point clouds.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_sliced_wasserstein(
    pts_a_json: JsonB,
    pts_b_json: JsonB,
    n_proj: default!(i32, 100),
) -> f32 {
    let pts_a = parse_points(&pts_a_json);
    let pts_b = parse_points(&pts_b_json);

    if pts_a.is_empty() || pts_b.is_empty() {
        pgrx::error!("Point clouds must be non-empty");
    }

    let sw = SlicedWasserstein::new(n_proj as usize).with_seed(42);
    sw.distance(&pts_a, &pts_b) as f32
}

/// Compute KL divergence between two distributions.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_kl_divergence(p: Vec<f32>, q: Vec<f32>) -> f32 {
    if p.len() != q.len() || p.is_empty() {
        pgrx::error!("Distributions must have same non-zero length");
    }

    let kl: f64 = p
        .iter()
        .zip(q.iter())
        .map(|(&pi, &qi)| {
            let pi = (pi as f64).max(1e-12);
            let qi = (qi as f64).max(1e-12);
            pi * (pi / qi).ln()
        })
        .sum();

    kl as f32
}

/// Compute Jensen-Shannon divergence between two distributions.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_jensen_shannon(p: Vec<f32>, q: Vec<f32>) -> f32 {
    if p.len() != q.len() || p.is_empty() {
        pgrx::error!("Distributions must have same non-zero length");
    }

    let n = p.len();
    let m: Vec<f64> = (0..n)
        .map(|i| ((p[i] as f64) + (q[i] as f64)) / 2.0)
        .collect();

    let kl_pm: f64 = (0..n)
        .map(|i| {
            let pi = (p[i] as f64).max(1e-12);
            let mi = m[i].max(1e-12);
            pi * (pi / mi).ln()
        })
        .sum();

    let kl_qm: f64 = (0..n)
        .map(|i| {
            let qi = (q[i] as f64).max(1e-12);
            let mi = m[i].max(1e-12);
            qi * (qi / mi).ln()
        })
        .sum();

    ((kl_pm + kl_qm) / 2.0) as f32
}

/// Compute Fisher information metric.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_fisher_information(dist: Vec<f32>, tangent: Vec<f32>) -> f32 {
    if dist.len() != tangent.len() || dist.is_empty() {
        pgrx::error!("Distribution and tangent must have same non-zero length");
    }

    let fisher: f64 = dist
        .iter()
        .zip(tangent.iter())
        .map(|(&p, &t)| {
            let p = (p as f64).max(1e-12);
            let t = t as f64;
            (t * t) / p
        })
        .sum();

    fisher as f32
}

/// Spectral clustering on an adjacency matrix.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_spectral_cluster(adj_json: JsonB, k: i32) -> Vec<i32> {
    let adj = parse_matrix(&adj_json);
    if adj.is_empty() {
        return Vec::new();
    }

    let (flat, n) = flatten_adjacency(&adj);
    let laplacian = ScaledLaplacian::from_adjacency(&flat, n);
    let clustering = SpectralClustering::with_k(k as usize);
    let result = clustering.cluster(&laplacian);
    result.assignments.iter().map(|&l| l as i32).collect()
}

/// Apply Chebyshev polynomial graph filter.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_chebyshev_filter(
    adj_json: JsonB,
    signal: Vec<f32>,
    filter_type: default!(&str, "'low_pass'"),
    degree: default!(i32, 10),
) -> Vec<f32> {
    let adj = parse_matrix(&adj_json);
    if adj.is_empty() || signal.is_empty() {
        return Vec::new();
    }

    let signal_f64: Vec<f64> = signal.iter().map(|&x| x as f64).collect();
    let (flat, n) = flatten_adjacency(&adj);
    let laplacian = ScaledLaplacian::from_adjacency(&flat, n);
    let deg = degree as usize;

    let spec_filter = match filter_type.to_lowercase().as_str() {
        "high_pass" => SpectralFilter::high_pass(0.5, deg),
        "band_pass" => SpectralFilter::band_pass(0.3, 0.7, deg),
        _ => SpectralFilter::low_pass(0.5, deg),
    };

    let filter = GraphFilter::new(laplacian, spec_filter);
    let result = filter.apply(&signal_f64);
    result.iter().map(|&x| x as f32).collect()
}

/// Compute heat kernel graph diffusion.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_graph_diffusion(
    adj_json: JsonB,
    signal: Vec<f32>,
    diffusion_time: default!(f32, 1.0),
    degree: default!(i32, 10),
) -> Vec<f32> {
    let adj = parse_matrix(&adj_json);
    if adj.is_empty() || signal.is_empty() {
        return Vec::new();
    }

    let signal_f64: Vec<f64> = signal.iter().map(|&x| x as f64).collect();
    let (flat, n) = flatten_adjacency(&adj);
    let laplacian = ScaledLaplacian::from_adjacency(&flat, n);

    let spec_filter = SpectralFilter::heat(diffusion_time as f64, degree as usize);
    let filter = GraphFilter::new(laplacian, spec_filter);
    let result = filter.apply(&signal_f64);
    result.iter().map(|&x| x as f32).collect()
}

/// Compute product manifold distance (Euclidean x Hyperbolic x Spherical).
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_product_manifold_distance(
    a: Vec<f32>,
    b: Vec<f32>,
    e_dim: i32,
    h_dim: i32,
    s_dim: i32,
) -> f32 {
    if a.len() != b.len() {
        pgrx::error!("Vectors must have same dimension");
    }

    let a_f64: Vec<f64> = a.iter().map(|&x| x as f64).collect();
    let b_f64: Vec<f64> = b.iter().map(|&x| x as f64).collect();

    let manifold = ProductManifold::new(e_dim as usize, h_dim as usize, s_dim as usize);
    match manifold.distance(&a_f64, &b_f64) {
        Ok(d) => d as f32,
        Err(e) => pgrx::error!("Product manifold distance failed: {}", e),
    }
}

/// Compute spherical (great-circle) distance.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_spherical_distance(a: Vec<f32>, b: Vec<f32>) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        pgrx::error!("Vectors must have same non-zero dimension");
    }

    let a_f64: Vec<f64> = a.iter().map(|&x| x as f64).collect();
    let b_f64: Vec<f64> = b.iter().map(|&x| x as f64).collect();

    let space = SphericalSpace::new(a.len());
    match space.distance(&a_f64, &b_f64) {
        Ok(d) => d as f32,
        Err(e) => pgrx::error!("Spherical distance failed: {}", e),
    }
}

/// Compute Gromov-Wasserstein distance between two metric spaces.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_gromov_wasserstein(pts_a_json: JsonB, pts_b_json: JsonB) -> JsonB {
    let pts_a = parse_points(&pts_a_json);
    let pts_b = parse_points(&pts_b_json);

    if pts_a.is_empty() || pts_b.is_empty() {
        pgrx::error!("Point clouds must be non-empty");
    }

    let gw = GromovWasserstein::new(0.1);
    match gw.solve(&pts_a, &pts_b) {
        Ok(result) => JsonB(serde_json::json!({
            "distance": result.loss.sqrt(),
            "converged": result.converged,
            "coupling": result.transport_plan,
        })),
        Err(e) => pgrx::error!("Gromov-Wasserstein failed: {}", e),
    }
}
