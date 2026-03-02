//! Coarse RF Tomography from link attenuations.
//!
//! Produces a low-resolution 3D occupancy volume by inverting per-link
//! attenuation measurements. Each voxel receives an occupancy probability
//! based on how many links traverse it and how much attenuation those links
//! observed.
//!
//! # Algorithm
//! 1. Define a voxel grid covering the monitored volume
//! 2. For each link, determine which voxels lie along the propagation path
//! 3. Solve the sparse tomographic inverse: attenuation = sum(voxel_density * path_weight)
//! 4. Apply L1 regularization for sparsity (most voxels are unoccupied)
//!
//! # References
//! - ADR-030 Tier 2: Coarse RF Tomography
//! - Wilson & Patwari (2010), "Radio Tomographic Imaging"

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors from tomography operations.
#[derive(Debug, thiserror::Error)]
pub enum TomographyError {
    /// Not enough links for tomographic inversion.
    #[error("Insufficient links: need >= {needed}, got {got}")]
    InsufficientLinks { needed: usize, got: usize },

    /// Grid dimensions are invalid.
    #[error("Invalid grid dimensions: {0}")]
    InvalidGrid(String),

    /// No voxels intersected by any link.
    #[error("No voxels intersected by links — check geometry")]
    NoIntersections,

    /// Observation vector length mismatch.
    #[error("Observation length mismatch: expected {expected}, got {got}")]
    ObservationMismatch { expected: usize, got: usize },
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the voxel grid and tomographic solver.
#[derive(Debug, Clone)]
pub struct TomographyConfig {
    /// Number of voxels along X axis.
    pub nx: usize,
    /// Number of voxels along Y axis.
    pub ny: usize,
    /// Number of voxels along Z axis.
    pub nz: usize,
    /// Physical extent of the grid: `[x_min, y_min, z_min, x_max, y_max, z_max]`.
    pub bounds: [f64; 6],
    /// L1 regularization weight (higher = sparser solution).
    pub lambda: f64,
    /// Maximum iterations for the solver.
    pub max_iterations: usize,
    /// Convergence tolerance.
    pub tolerance: f64,
    /// Minimum links required for inversion (default 8).
    pub min_links: usize,
}

impl Default for TomographyConfig {
    fn default() -> Self {
        Self {
            nx: 8,
            ny: 8,
            nz: 4,
            bounds: [0.0, 0.0, 0.0, 6.0, 6.0, 3.0],
            lambda: 0.1,
            max_iterations: 100,
            tolerance: 1e-4,
            min_links: 8,
        }
    }
}

// ---------------------------------------------------------------------------
// Geometry types
// ---------------------------------------------------------------------------

/// A 3D position.
#[derive(Debug, Clone, Copy)]
pub struct Position3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// A link between a transmitter and receiver.
#[derive(Debug, Clone)]
pub struct LinkGeometry {
    /// Transmitter position.
    pub tx: Position3D,
    /// Receiver position.
    pub rx: Position3D,
    /// Link identifier.
    pub link_id: usize,
}

impl LinkGeometry {
    /// Euclidean distance between TX and RX.
    pub fn distance(&self) -> f64 {
        let dx = self.rx.x - self.tx.x;
        let dy = self.rx.y - self.tx.y;
        let dz = self.rx.z - self.tx.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

// ---------------------------------------------------------------------------
// Occupancy volume
// ---------------------------------------------------------------------------

/// 3D occupancy grid resulting from tomographic inversion.
#[derive(Debug, Clone)]
pub struct OccupancyVolume {
    /// Voxel densities in row-major order `[nz][ny][nx]`.
    pub densities: Vec<f64>,
    /// Grid dimensions.
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
    /// Physical bounds.
    pub bounds: [f64; 6],
    /// Number of occupied voxels (density > threshold).
    pub occupied_count: usize,
    /// Total voxel count.
    pub total_voxels: usize,
    /// Solver residual at convergence.
    pub residual: f64,
    /// Number of iterations used.
    pub iterations: usize,
}

impl OccupancyVolume {
    /// Get density at voxel (ix, iy, iz). Returns None if out of bounds.
    pub fn get(&self, ix: usize, iy: usize, iz: usize) -> Option<f64> {
        if ix < self.nx && iy < self.ny && iz < self.nz {
            Some(self.densities[iz * self.ny * self.nx + iy * self.nx + ix])
        } else {
            None
        }
    }

    /// Voxel size along each axis.
    pub fn voxel_size(&self) -> [f64; 3] {
        [
            (self.bounds[3] - self.bounds[0]) / self.nx as f64,
            (self.bounds[4] - self.bounds[1]) / self.ny as f64,
            (self.bounds[5] - self.bounds[2]) / self.nz as f64,
        ]
    }

    /// Center position of voxel (ix, iy, iz).
    pub fn voxel_center(&self, ix: usize, iy: usize, iz: usize) -> Position3D {
        let vs = self.voxel_size();
        Position3D {
            x: self.bounds[0] + (ix as f64 + 0.5) * vs[0],
            y: self.bounds[1] + (iy as f64 + 0.5) * vs[1],
            z: self.bounds[2] + (iz as f64 + 0.5) * vs[2],
        }
    }
}

// ---------------------------------------------------------------------------
// Tomographic solver
// ---------------------------------------------------------------------------

/// Coarse RF tomography solver.
///
/// Given a set of TX-RX links and per-link attenuation measurements,
/// reconstructs a 3D occupancy volume using L1-regularized least squares.
pub struct RfTomographer {
    config: TomographyConfig,
    /// Precomputed weight matrix: `weight_matrix[link_idx]` is a list of
    /// (voxel_index, weight) pairs.
    weight_matrix: Vec<Vec<(usize, f64)>>,
    /// Number of voxels.
    n_voxels: usize,
}

impl RfTomographer {
    /// Create a new tomographer with the given configuration and link geometry.
    pub fn new(config: TomographyConfig, links: &[LinkGeometry]) -> Result<Self, TomographyError> {
        if links.len() < config.min_links {
            return Err(TomographyError::InsufficientLinks {
                needed: config.min_links,
                got: links.len(),
            });
        }
        if config.nx == 0 || config.ny == 0 || config.nz == 0 {
            return Err(TomographyError::InvalidGrid(
                "Grid dimensions must be > 0".into(),
            ));
        }

        let n_voxels = config
            .nx
            .checked_mul(config.ny)
            .and_then(|v| v.checked_mul(config.nz))
            .ok_or_else(|| {
                TomographyError::InvalidGrid(format!(
                    "Grid dimensions overflow: {}x{}x{}",
                    config.nx, config.ny, config.nz
                ))
            })?;

        // Precompute weight matrix
        let weight_matrix: Vec<Vec<(usize, f64)>> = links
            .iter()
            .map(|link| compute_link_weights(link, &config))
            .collect();

        // Ensure at least one link intersects some voxels
        let total_weights: usize = weight_matrix.iter().map(|w| w.len()).sum();
        if total_weights == 0 {
            return Err(TomographyError::NoIntersections);
        }

        Ok(Self {
            config,
            weight_matrix,
            n_voxels,
        })
    }

    /// Reconstruct occupancy from per-link attenuation measurements.
    ///
    /// `attenuations` has one entry per link (same order as links passed to `new`).
    /// Higher attenuation indicates more obstruction along the link path.
    pub fn reconstruct(&self, attenuations: &[f64]) -> Result<OccupancyVolume, TomographyError> {
        if attenuations.len() != self.weight_matrix.len() {
            return Err(TomographyError::ObservationMismatch {
                expected: self.weight_matrix.len(),
                got: attenuations.len(),
            });
        }

        // ISTA (Iterative Shrinkage-Thresholding Algorithm) for L1 minimization
        // min ||Wx - y||^2 + lambda * ||x||_1
        let mut x = vec![0.0_f64; self.n_voxels];
        let n_links = attenuations.len();

        // Estimate step size: 1 / L where L is the Lipschitz constant of the
        // gradient of ||Wx - y||^2, i.e. the spectral norm of W^T W.
        // A safe upper bound is the Frobenius norm squared of W (sum of all
        // squared entries), since ||W^T W|| <= ||W||_F^2.
        let frobenius_sq: f64 = self
            .weight_matrix
            .iter()
            .flat_map(|ws| ws.iter().map(|&(_, w)| w * w))
            .sum();
        let lipschitz = frobenius_sq.max(1e-10);
        let step_size = 1.0 / lipschitz;

        let mut residual = 0.0_f64;
        let mut iterations = 0;

        for iter in 0..self.config.max_iterations {
            // Compute gradient: W^T (Wx - y)
            let mut gradient = vec![0.0_f64; self.n_voxels];
            residual = 0.0;

            for (link_idx, weights) in self.weight_matrix.iter().enumerate() {
                // Forward: Wx for this link
                let predicted: f64 = weights.iter().map(|&(idx, w)| w * x[idx]).sum();
                let diff = predicted - attenuations[link_idx];
                residual += diff * diff;

                // Backward: accumulate gradient
                for &(idx, w) in weights {
                    gradient[idx] += w * diff;
                }
            }

            residual = (residual / n_links as f64).sqrt();

            // Gradient step + soft thresholding (proximal L1)
            let mut max_change = 0.0_f64;
            for i in 0..self.n_voxels {
                let new_val = x[i] - step_size * gradient[i];
                // Soft thresholding
                let threshold = self.config.lambda * step_size;
                let shrunk = if new_val > threshold {
                    new_val - threshold
                } else if new_val < -threshold {
                    new_val + threshold
                } else {
                    0.0
                };
                // Non-negativity constraint (density >= 0)
                let clamped = shrunk.max(0.0);
                max_change = max_change.max((clamped - x[i]).abs());
                x[i] = clamped;
            }

            iterations = iter + 1;

            if max_change < self.config.tolerance {
                break;
            }
        }

        // Count occupied voxels (density > 0.01)
        let occupied_count = x.iter().filter(|&&d| d > 0.01).count();

        Ok(OccupancyVolume {
            densities: x,
            nx: self.config.nx,
            ny: self.config.ny,
            nz: self.config.nz,
            bounds: self.config.bounds,
            occupied_count,
            total_voxels: self.n_voxels,
            residual,
            iterations,
        })
    }

    /// Number of links in this tomographer.
    pub fn n_links(&self) -> usize {
        self.weight_matrix.len()
    }

    /// Number of voxels in the grid.
    pub fn n_voxels(&self) -> usize {
        self.n_voxels
    }
}

// ---------------------------------------------------------------------------
// Weight computation (simplified ray-voxel intersection)
// ---------------------------------------------------------------------------

/// Compute the intersection weights of a link with the voxel grid.
///
/// Uses a simplified approach: for each voxel, computes the minimum
/// distance from the voxel center to the link ray. Voxels within
/// one Fresnel zone receive weight proportional to closeness.
fn compute_link_weights(link: &LinkGeometry, config: &TomographyConfig) -> Vec<(usize, f64)> {
    let vx = (config.bounds[3] - config.bounds[0]) / config.nx as f64;
    let vy = (config.bounds[4] - config.bounds[1]) / config.ny as f64;
    let vz = (config.bounds[5] - config.bounds[2]) / config.nz as f64;

    // Fresnel zone half-width (approximate)
    let link_dist = link.distance();
    let wavelength = 0.06; // ~5 GHz
    let fresnel_radius = (wavelength * link_dist / 4.0).sqrt().max(vx.max(vy));

    let dx = link.rx.x - link.tx.x;
    let dy = link.rx.y - link.tx.y;
    let dz = link.rx.z - link.tx.z;

    let mut weights = Vec::new();

    for iz in 0..config.nz {
        for iy in 0..config.ny {
            for ix in 0..config.nx {
                let cx = config.bounds[0] + (ix as f64 + 0.5) * vx;
                let cy = config.bounds[1] + (iy as f64 + 0.5) * vy;
                let cz = config.bounds[2] + (iz as f64 + 0.5) * vz;

                // Point-to-line distance
                let dist = point_to_segment_distance(
                    cx, cy, cz, link.tx.x, link.tx.y, link.tx.z, dx, dy, dz, link_dist,
                );

                if dist < fresnel_radius {
                    // Weight decays with distance from link ray
                    let w = 1.0 - dist / fresnel_radius;
                    let idx = iz * config.ny * config.nx + iy * config.nx + ix;
                    weights.push((idx, w));
                }
            }
        }
    }

    weights
}

/// Distance from point (px,py,pz) to line segment defined by start + t*dir
/// where dir = (dx,dy,dz) and segment length = `seg_len`.
fn point_to_segment_distance(
    px: f64,
    py: f64,
    pz: f64,
    sx: f64,
    sy: f64,
    sz: f64,
    dx: f64,
    dy: f64,
    dz: f64,
    seg_len: f64,
) -> f64 {
    if seg_len < 1e-12 {
        return ((px - sx).powi(2) + (py - sy).powi(2) + (pz - sz).powi(2)).sqrt();
    }

    // Project point onto line: t = dot(P-S, D) / |D|^2
    let t = ((px - sx) * dx + (py - sy) * dy + (pz - sz) * dz) / (seg_len * seg_len);
    let t_clamped = t.clamp(0.0, 1.0);

    let closest_x = sx + t_clamped * dx;
    let closest_y = sy + t_clamped * dy;
    let closest_z = sz + t_clamped * dz;

    ((px - closest_x).powi(2) + (py - closest_y).powi(2) + (pz - closest_z).powi(2)).sqrt()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_square_links() -> Vec<LinkGeometry> {
        // 4 nodes in a square at z=1.5, 12 directed links
        let nodes = [
            Position3D {
                x: 0.5,
                y: 0.5,
                z: 1.5,
            },
            Position3D {
                x: 5.5,
                y: 0.5,
                z: 1.5,
            },
            Position3D {
                x: 5.5,
                y: 5.5,
                z: 1.5,
            },
            Position3D {
                x: 0.5,
                y: 5.5,
                z: 1.5,
            },
        ];
        let mut links = Vec::new();
        let mut id = 0;
        for i in 0..4 {
            for j in 0..4 {
                if i != j {
                    links.push(LinkGeometry {
                        tx: nodes[i],
                        rx: nodes[j],
                        link_id: id,
                    });
                    id += 1;
                }
            }
        }
        links
    }

    #[test]
    fn test_tomographer_creation() {
        let links = make_square_links();
        let config = TomographyConfig {
            min_links: 8,
            ..Default::default()
        };
        let tomo = RfTomographer::new(config, &links).unwrap();
        assert_eq!(tomo.n_links(), 12);
        assert_eq!(tomo.n_voxels(), 8 * 8 * 4);
    }

    #[test]
    fn test_insufficient_links() {
        let links = vec![LinkGeometry {
            tx: Position3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rx: Position3D {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            link_id: 0,
        }];
        let config = TomographyConfig {
            min_links: 8,
            ..Default::default()
        };
        assert!(matches!(
            RfTomographer::new(config, &links),
            Err(TomographyError::InsufficientLinks { .. })
        ));
    }

    #[test]
    fn test_invalid_grid() {
        let links = make_square_links();
        let config = TomographyConfig {
            nx: 0,
            ..Default::default()
        };
        assert!(matches!(
            RfTomographer::new(config, &links),
            Err(TomographyError::InvalidGrid(_))
        ));
    }

    #[test]
    fn test_zero_attenuation_empty_room() {
        let links = make_square_links();
        let config = TomographyConfig {
            min_links: 8,
            ..Default::default()
        };
        let tomo = RfTomographer::new(config, &links).unwrap();

        // Zero attenuation = empty room
        let attenuations = vec![0.0; tomo.n_links()];
        let volume = tomo.reconstruct(&attenuations).unwrap();

        assert_eq!(volume.total_voxels, 8 * 8 * 4);
        // All densities should be zero or near zero
        assert!(
            volume.occupied_count == 0,
            "Empty room should have no occupied voxels, got {}",
            volume.occupied_count
        );
    }

    #[test]
    fn test_nonzero_attenuation_produces_density() {
        let links = make_square_links();
        let config = TomographyConfig {
            min_links: 8,
            lambda: 0.001, // light regularization so solution is not zeroed
            max_iterations: 500,
            tolerance: 1e-8,
            ..Default::default()
        };
        let tomo = RfTomographer::new(config, &links).unwrap();

        // Strong attenuations to represent obstructed links
        let attenuations: Vec<f64> = (0..tomo.n_links()).map(|i| 5.0 + 1.0 * i as f64).collect();
        let volume = tomo.reconstruct(&attenuations).unwrap();

        // Check that at least some voxels have non-negligible density
        let any_nonzero = volume.densities.iter().any(|&d| d > 1e-6);
        assert!(
            any_nonzero,
            "Non-zero attenuation should produce non-zero voxel densities"
        );
    }

    #[test]
    fn test_observation_mismatch() {
        let links = make_square_links();
        let config = TomographyConfig {
            min_links: 8,
            ..Default::default()
        };
        let tomo = RfTomographer::new(config, &links).unwrap();

        let attenuations = vec![0.1; 3]; // wrong count
        assert!(matches!(
            tomo.reconstruct(&attenuations),
            Err(TomographyError::ObservationMismatch { .. })
        ));
    }

    #[test]
    fn test_voxel_access() {
        let links = make_square_links();
        let config = TomographyConfig {
            min_links: 8,
            ..Default::default()
        };
        let tomo = RfTomographer::new(config, &links).unwrap();

        let attenuations = vec![0.0; tomo.n_links()];
        let volume = tomo.reconstruct(&attenuations).unwrap();

        // Valid access
        assert!(volume.get(0, 0, 0).is_some());
        assert!(volume.get(7, 7, 3).is_some());
        // Out of bounds
        assert!(volume.get(8, 0, 0).is_none());
        assert!(volume.get(0, 8, 0).is_none());
        assert!(volume.get(0, 0, 4).is_none());
    }

    #[test]
    fn test_voxel_center() {
        let links = make_square_links();
        let config = TomographyConfig {
            nx: 6,
            ny: 6,
            nz: 3,
            min_links: 8,
            ..Default::default()
        };
        let tomo = RfTomographer::new(config, &links).unwrap();

        let attenuations = vec![0.0; tomo.n_links()];
        let volume = tomo.reconstruct(&attenuations).unwrap();

        let center = volume.voxel_center(0, 0, 0);
        assert!(center.x > 0.0 && center.x < 1.0);
        assert!(center.y > 0.0 && center.y < 1.0);
        assert!(center.z > 0.0 && center.z < 1.0);
    }

    #[test]
    fn test_voxel_size() {
        let links = make_square_links();
        let config = TomographyConfig {
            nx: 6,
            ny: 6,
            nz: 3,
            bounds: [0.0, 0.0, 0.0, 6.0, 6.0, 3.0],
            min_links: 8,
            ..Default::default()
        };
        let tomo = RfTomographer::new(config, &links).unwrap();

        let attenuations = vec![0.0; tomo.n_links()];
        let volume = tomo.reconstruct(&attenuations).unwrap();
        let vs = volume.voxel_size();

        assert!((vs[0] - 1.0).abs() < 1e-10);
        assert!((vs[1] - 1.0).abs() < 1e-10);
        assert!((vs[2] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_point_to_segment_distance() {
        // Point directly on the segment
        let d = point_to_segment_distance(0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0);
        assert!(d < 1e-10);

        // Point 1 unit above the midpoint
        let d = point_to_segment_distance(0.5, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0);
        assert!((d - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_link_distance() {
        let link = LinkGeometry {
            tx: Position3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            rx: Position3D {
                x: 3.0,
                y: 4.0,
                z: 0.0,
            },
            link_id: 0,
        };
        assert!((link.distance() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_solver_convergence() {
        let links = make_square_links();
        let config = TomographyConfig {
            min_links: 8,
            lambda: 0.01,
            max_iterations: 500,
            tolerance: 1e-6,
            ..Default::default()
        };
        let tomo = RfTomographer::new(config, &links).unwrap();

        let attenuations: Vec<f64> = (0..tomo.n_links())
            .map(|i| 0.3 * (i as f64 * 0.7).sin().abs())
            .collect();
        let volume = tomo.reconstruct(&attenuations).unwrap();

        assert!(volume.residual.is_finite());
        assert!(volume.iterations > 0);
    }
}
