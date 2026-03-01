//! WebAssembly bindings for ruvector-math
//!
//! This crate provides JavaScript/TypeScript bindings for the advanced
//! mathematics in ruvector-math, enabling browser-based vector search
//! with optimal transport, information geometry, and product manifolds.

use ruvector_math::{
    information_geometry::{FisherInformation, NaturalGradient},
    optimal_transport::{GromovWasserstein, SinkhornSolver, SlicedWasserstein},
    product_manifold::ProductManifold,
    spherical::SphericalSpace,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// ============================================================================
// Optimal Transport
// ============================================================================

/// Sliced Wasserstein distance calculator for WASM
#[wasm_bindgen]
pub struct WasmSlicedWasserstein {
    inner: SlicedWasserstein,
}

#[wasm_bindgen]
impl WasmSlicedWasserstein {
    /// Create a new Sliced Wasserstein calculator
    ///
    /// @param num_projections - Number of random 1D projections (100-1000 typical)
    #[wasm_bindgen(constructor)]
    pub fn new(num_projections: usize) -> Self {
        Self {
            inner: SlicedWasserstein::new(num_projections),
        }
    }

    /// Set Wasserstein power (1 for W1, 2 for W2)
    #[wasm_bindgen(js_name = withPower)]
    pub fn with_power(self, p: f64) -> Self {
        Self {
            inner: self.inner.with_power(p),
        }
    }

    /// Set random seed for reproducibility
    #[wasm_bindgen(js_name = withSeed)]
    pub fn with_seed(self, seed: u64) -> Self {
        Self {
            inner: self.inner.with_seed(seed),
        }
    }

    /// Compute distance between two point clouds
    ///
    /// @param source - Source points as flat array [x1, y1, z1, x2, y2, z2, ...]
    /// @param target - Target points as flat array
    /// @param dim - Dimension of each point
    #[wasm_bindgen]
    pub fn distance(&self, source: &[f64], target: &[f64], dim: usize) -> f64 {
        use ruvector_math::optimal_transport::OptimalTransport;

        let source_points = to_points(source, dim);
        let target_points = to_points(target, dim);

        self.inner.distance(&source_points, &target_points)
    }

    /// Compute weighted distance
    #[wasm_bindgen(js_name = weightedDistance)]
    pub fn weighted_distance(
        &self,
        source: &[f64],
        source_weights: &[f64],
        target: &[f64],
        target_weights: &[f64],
        dim: usize,
    ) -> f64 {
        use ruvector_math::optimal_transport::OptimalTransport;

        let source_points = to_points(source, dim);
        let target_points = to_points(target, dim);

        self.inner.weighted_distance(
            &source_points,
            source_weights,
            &target_points,
            target_weights,
        )
    }
}

/// Sinkhorn optimal transport solver for WASM
#[wasm_bindgen]
pub struct WasmSinkhorn {
    inner: SinkhornSolver,
}

#[wasm_bindgen]
impl WasmSinkhorn {
    /// Create a new Sinkhorn solver
    ///
    /// @param regularization - Entropy regularization (0.01-0.1 typical)
    /// @param max_iterations - Maximum iterations (100-1000 typical)
    #[wasm_bindgen(constructor)]
    pub fn new(regularization: f64, max_iterations: usize) -> Self {
        Self {
            inner: SinkhornSolver::new(regularization, max_iterations),
        }
    }

    /// Compute transport cost between point clouds
    #[wasm_bindgen]
    pub fn distance(&self, source: &[f64], target: &[f64], dim: usize) -> Result<f64, JsError> {
        let source_points = to_points(source, dim);
        let target_points = to_points(target, dim);

        self.inner
            .distance(&source_points, &target_points)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Solve optimal transport and return transport plan
    #[wasm_bindgen(js_name = solveTransport)]
    pub fn solve_transport(
        &self,
        cost_matrix: &[f64],
        source_weights: &[f64],
        target_weights: &[f64],
        n: usize,
        m: usize,
    ) -> Result<TransportResult, JsError> {
        let cost = to_matrix(cost_matrix, n, m);

        let result = self
            .inner
            .solve(&cost, source_weights, target_weights)
            .map_err(|e| JsError::new(&e.to_string()))?;

        Ok(TransportResult {
            plan: result.plan.into_iter().flatten().collect(),
            cost: result.cost,
            iterations: result.iterations,
            converged: result.converged,
        })
    }
}

/// Result of Sinkhorn transport computation
#[wasm_bindgen]
pub struct TransportResult {
    plan: Vec<f64>,
    cost: f64,
    iterations: usize,
    converged: bool,
}

#[wasm_bindgen]
impl TransportResult {
    /// Get transport plan as flat array
    #[wasm_bindgen(getter)]
    pub fn plan(&self) -> Vec<f64> {
        self.plan.clone()
    }

    /// Get total transport cost
    #[wasm_bindgen(getter)]
    pub fn cost(&self) -> f64 {
        self.cost
    }

    /// Get number of iterations
    #[wasm_bindgen(getter)]
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    /// Whether algorithm converged
    #[wasm_bindgen(getter)]
    pub fn converged(&self) -> bool {
        self.converged
    }
}

/// Gromov-Wasserstein distance for WASM
#[wasm_bindgen]
pub struct WasmGromovWasserstein {
    inner: GromovWasserstein,
}

#[wasm_bindgen]
impl WasmGromovWasserstein {
    /// Create a new Gromov-Wasserstein calculator
    #[wasm_bindgen(constructor)]
    pub fn new(regularization: f64) -> Self {
        Self {
            inner: GromovWasserstein::new(regularization),
        }
    }

    /// Compute GW distance between point clouds
    #[wasm_bindgen]
    pub fn distance(&self, source: &[f64], target: &[f64], dim: usize) -> Result<f64, JsError> {
        let source_points = to_points(source, dim);
        let target_points = to_points(target, dim);

        self.inner
            .distance(&source_points, &target_points)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

// ============================================================================
// Information Geometry
// ============================================================================

/// Fisher Information for WASM
#[wasm_bindgen]
pub struct WasmFisherInformation {
    inner: FisherInformation,
}

#[wasm_bindgen]
impl WasmFisherInformation {
    /// Create a new Fisher Information calculator
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: FisherInformation::new(),
        }
    }

    /// Set damping factor
    #[wasm_bindgen(js_name = withDamping)]
    pub fn with_damping(self, damping: f64) -> Self {
        Self {
            inner: self.inner.with_damping(damping),
        }
    }

    /// Compute diagonal FIM from gradient samples
    #[wasm_bindgen(js_name = diagonalFim)]
    pub fn diagonal_fim(
        &self,
        gradients: &[f64],
        _num_samples: usize,
        dim: usize,
    ) -> Result<Vec<f64>, JsError> {
        let grads = to_points(gradients, dim);
        self.inner
            .diagonal_fim(&grads)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Compute natural gradient
    #[wasm_bindgen(js_name = naturalGradient)]
    pub fn natural_gradient(&self, fim_diag: &[f64], gradient: &[f64], damping: f64) -> Vec<f64> {
        gradient
            .iter()
            .zip(fim_diag.iter())
            .map(|(&g, &f)| g / (f + damping))
            .collect()
    }
}

/// Natural Gradient optimizer for WASM
#[wasm_bindgen]
pub struct WasmNaturalGradient {
    inner: NaturalGradient,
}

#[wasm_bindgen]
impl WasmNaturalGradient {
    /// Create a new Natural Gradient optimizer
    #[wasm_bindgen(constructor)]
    pub fn new(learning_rate: f64) -> Self {
        Self {
            inner: NaturalGradient::new(learning_rate),
        }
    }

    /// Set damping factor
    #[wasm_bindgen(js_name = withDamping)]
    pub fn with_damping(self, damping: f64) -> Self {
        Self {
            inner: self.inner.with_damping(damping),
        }
    }

    /// Use diagonal approximation
    #[wasm_bindgen(js_name = withDiagonal)]
    pub fn with_diagonal(self, use_diagonal: bool) -> Self {
        Self {
            inner: self.inner.with_diagonal(use_diagonal),
        }
    }

    /// Compute update step
    #[wasm_bindgen]
    pub fn step(
        &mut self,
        gradient: &[f64],
        gradient_samples: Option<Vec<f64>>,
        dim: usize,
    ) -> Result<Vec<f64>, JsError> {
        let samples = gradient_samples.map(|s| to_points(&s, dim));

        self.inner
            .step(gradient, samples.as_deref())
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Reset optimizer state
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

// ============================================================================
// Spherical Geometry
// ============================================================================

/// Spherical space operations for WASM
#[wasm_bindgen]
pub struct WasmSphericalSpace {
    inner: SphericalSpace,
}

#[wasm_bindgen]
impl WasmSphericalSpace {
    /// Create a new spherical space S^{n-1} embedded in R^n
    #[wasm_bindgen(constructor)]
    pub fn new(ambient_dim: usize) -> Self {
        Self {
            inner: SphericalSpace::new(ambient_dim),
        }
    }

    /// Get ambient dimension
    #[wasm_bindgen(getter, js_name = ambientDim)]
    pub fn ambient_dim(&self) -> usize {
        self.inner.ambient_dim()
    }

    /// Project point onto sphere
    #[wasm_bindgen]
    pub fn project(&self, point: &[f64]) -> Result<Vec<f64>, JsError> {
        self.inner
            .project(point)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Geodesic distance on sphere
    #[wasm_bindgen]
    pub fn distance(&self, x: &[f64], y: &[f64]) -> Result<f64, JsError> {
        self.inner
            .distance(x, y)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Exponential map: move from x in direction v
    #[wasm_bindgen(js_name = expMap)]
    pub fn exp_map(&self, x: &[f64], v: &[f64]) -> Result<Vec<f64>, JsError> {
        self.inner
            .exp_map(x, v)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Logarithmic map: tangent vector at x pointing toward y
    #[wasm_bindgen(js_name = logMap)]
    pub fn log_map(&self, x: &[f64], y: &[f64]) -> Result<Vec<f64>, JsError> {
        self.inner
            .log_map(x, y)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Geodesic interpolation at fraction t
    #[wasm_bindgen]
    pub fn geodesic(&self, x: &[f64], y: &[f64], t: f64) -> Result<Vec<f64>, JsError> {
        self.inner
            .geodesic(x, y, t)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Fréchet mean of points
    #[wasm_bindgen(js_name = frechetMean)]
    pub fn frechet_mean(&self, points: &[f64], dim: usize) -> Result<Vec<f64>, JsError> {
        let pts = to_points(points, dim);
        self.inner
            .frechet_mean(&pts, None)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

// ============================================================================
// Product Manifolds
// ============================================================================

/// Product manifold for WASM: E^e × H^h × S^s
#[wasm_bindgen]
pub struct WasmProductManifold {
    inner: ProductManifold,
}

#[wasm_bindgen]
impl WasmProductManifold {
    /// Create a new product manifold
    ///
    /// @param euclidean_dim - Dimension of Euclidean component
    /// @param hyperbolic_dim - Dimension of hyperbolic component
    /// @param spherical_dim - Dimension of spherical component
    #[wasm_bindgen(constructor)]
    pub fn new(euclidean_dim: usize, hyperbolic_dim: usize, spherical_dim: usize) -> Self {
        Self {
            inner: ProductManifold::new(euclidean_dim, hyperbolic_dim, spherical_dim),
        }
    }

    /// Get total dimension
    #[wasm_bindgen(getter)]
    pub fn dim(&self) -> usize {
        self.inner.dim()
    }

    /// Project point onto manifold
    #[wasm_bindgen]
    pub fn project(&self, point: &[f64]) -> Result<Vec<f64>, JsError> {
        self.inner
            .project(point)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Compute distance in product manifold
    #[wasm_bindgen]
    pub fn distance(&self, x: &[f64], y: &[f64]) -> Result<f64, JsError> {
        self.inner
            .distance(x, y)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Exponential map
    #[wasm_bindgen(js_name = expMap)]
    pub fn exp_map(&self, x: &[f64], v: &[f64]) -> Result<Vec<f64>, JsError> {
        self.inner
            .exp_map(x, v)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Logarithmic map
    #[wasm_bindgen(js_name = logMap)]
    pub fn log_map(&self, x: &[f64], y: &[f64]) -> Result<Vec<f64>, JsError> {
        self.inner
            .log_map(x, y)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Geodesic interpolation
    #[wasm_bindgen]
    pub fn geodesic(&self, x: &[f64], y: &[f64], t: f64) -> Result<Vec<f64>, JsError> {
        self.inner
            .geodesic(x, y, t)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Fréchet mean
    #[wasm_bindgen(js_name = frechetMean)]
    pub fn frechet_mean(&self, points: &[f64], _num_points: usize) -> Result<Vec<f64>, JsError> {
        let dim = self.inner.dim();
        let pts = to_points(points, dim);
        self.inner
            .frechet_mean(&pts, None)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// K-nearest neighbors
    #[wasm_bindgen]
    pub fn knn(&self, query: &[f64], points: &[f64], k: usize) -> Result<Vec<u32>, JsError> {
        let dim = self.inner.dim();
        let pts = to_points(points, dim);
        let neighbors = self
            .inner
            .knn(query, &pts, k)
            .map_err(|e| JsError::new(&e.to_string()))?;

        Ok(neighbors.into_iter().map(|(idx, _)| idx as u32).collect())
    }

    /// Pairwise distances
    #[wasm_bindgen(js_name = pairwiseDistances)]
    pub fn pairwise_distances(&self, points: &[f64]) -> Result<Vec<f64>, JsError> {
        let dim = self.inner.dim();
        let pts = to_points(points, dim);
        let dists = self
            .inner
            .pairwise_distances(&pts)
            .map_err(|e| JsError::new(&e.to_string()))?;

        Ok(dists.into_iter().flatten().collect())
    }
}

// ============================================================================
// Utility functions
// ============================================================================

/// Convert flat array to vector of points
fn to_points(flat: &[f64], dim: usize) -> Vec<Vec<f64>> {
    flat.chunks(dim).map(|c| c.to_vec()).collect()
}

/// Convert flat array to matrix
fn to_matrix(flat: &[f64], rows: usize, cols: usize) -> Vec<Vec<f64>> {
    flat.chunks(cols).take(rows).map(|c| c.to_vec()).collect()
}

// ============================================================================
// TypeScript type definitions
// ============================================================================

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
/** Sliced Wasserstein distance for comparing point cloud distributions */
export interface SlicedWassersteinOptions {
    numProjections?: number;
    power?: number;
    seed?: number;
}

/** Sinkhorn optimal transport options */
export interface SinkhornOptions {
    regularization?: number;
    maxIterations?: number;
    threshold?: number;
}

/** Product manifold configuration */
export interface ProductManifoldConfig {
    euclideanDim: number;
    hyperbolicDim: number;
    sphericalDim: number;
    hyperbolicCurvature?: number;
    sphericalCurvature?: number;
}
"#;
