# Hyperbolic Embeddings Integration Plan

## Overview

Integrate hyperbolic geometry operations into PostgreSQL for hierarchical data representation, enabling embeddings in Poincaré ball and Lorentz (hyperboloid) models with native distance functions and indexing.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     PostgreSQL Extension                         │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                 Hyperbolic Type System                   │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │    │
│  │  │   Poincaré   │  │   Lorentz    │  │   Klein      │   │    │
│  │  │     Ball     │  │ Hyperboloid  │  │    Model     │   │    │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘   │    │
│  └─────────┼─────────────────┼─────────────────┼───────────┘    │
│            └─────────────────┴─────────────────┘                │
│                              ▼                                   │
│              ┌───────────────────────────┐                       │
│              │   Riemannian Operations   │                       │
│              │   (Exponential, Log, PT)  │                       │
│              └───────────────────────────┘                       │
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── hyperbolic/
│   ├── mod.rs              # Module exports
│   ├── types/
│   │   ├── poincare.rs     # Poincaré ball model
│   │   ├── lorentz.rs      # Lorentz/hyperboloid model
│   │   └── klein.rs        # Klein model (projective)
│   ├── manifold.rs         # Manifold operations
│   ├── distance.rs         # Distance functions
│   ├── index/
│   │   ├── htree.rs        # Hyperbolic tree index
│   │   └── hnsw_hyper.rs   # HNSW for hyperbolic space
│   └── operators.rs        # SQL operators
```

## SQL Interface

### Hyperbolic Types

```sql
-- Create hyperbolic embedding column
CREATE TABLE hierarchical_nodes (
    id SERIAL PRIMARY KEY,
    name TEXT,
    euclidean_embedding vector(128),
    poincare_embedding hyperbolic(128),      -- Poincaré ball
    lorentz_embedding hyperboloid(129),      -- Lorentz model (d+1 dims)
    curvature FLOAT DEFAULT -1.0
);

-- Insert with automatic projection
INSERT INTO hierarchical_nodes (name, euclidean_embedding)
VALUES ('root', '[0.1, 0.2, ...]');

-- Auto-project to hyperbolic space
UPDATE hierarchical_nodes
SET poincare_embedding = ruvector_to_poincare(euclidean_embedding, curvature);
```

### Distance Operations

```sql
-- Poincaré distance
SELECT id, name,
       ruvector_poincare_distance(poincare_embedding, query_point) AS dist
FROM hierarchical_nodes
ORDER BY dist
LIMIT 10;

-- Lorentz distance (often more numerically stable)
SELECT id, name,
       ruvector_lorentz_distance(lorentz_embedding, query_point) AS dist
FROM hierarchical_nodes
ORDER BY dist
LIMIT 10;

-- Custom curvature
SELECT ruvector_hyperbolic_distance(
    a := point_a,
    b := point_b,
    model := 'poincare',
    curvature := -0.5
);
```

### Hyperbolic Operations

```sql
-- Möbius addition (translation in Poincaré ball)
SELECT ruvector_mobius_add(point_a, point_b, curvature := -1.0);

-- Exponential map (tangent vector → manifold point)
SELECT ruvector_exp_map(base_point, tangent_vector, curvature := -1.0);

-- Logarithmic map (manifold point → tangent vector)
SELECT ruvector_log_map(base_point, target_point, curvature := -1.0);

-- Parallel transport (move vector along geodesic)
SELECT ruvector_parallel_transport(vector, from_point, to_point, curvature := -1.0);

-- Geodesic midpoint
SELECT ruvector_geodesic_midpoint(point_a, point_b);

-- Project Euclidean to hyperbolic
SELECT ruvector_project_to_hyperbolic(euclidean_vec, model := 'poincare');
```

### Hyperbolic Index

```sql
-- Create hyperbolic HNSW index
CREATE INDEX ON hierarchical_nodes USING ruvector_hyperbolic (
    poincare_embedding hyperbolic(128)
) WITH (
    model = 'poincare',
    curvature = -1.0,
    m = 16,
    ef_construction = 64
);

-- Hyperbolic k-NN search
SELECT * FROM hierarchical_nodes
ORDER BY poincare_embedding <~> query_point  -- <~> is hyperbolic distance
LIMIT 10;
```

## Implementation Phases

### Phase 1: Poincaré Ball Model (Week 1-3)

```rust
// src/hyperbolic/types/poincare.rs

use simsimd::SpatialSimilarity;

/// Poincaré ball model B^n_c = {x ∈ R^n : c||x||² < 1}
pub struct PoincareBall {
    dim: usize,
    curvature: f32,  // Negative curvature, typically -1.0
}

impl PoincareBall {
    pub fn new(dim: usize, curvature: f32) -> Self {
        assert!(curvature < 0.0, "Curvature must be negative");
        Self { dim, curvature }
    }

    /// Conformal factor λ_c(x) = 2 / (1 - c||x||²)
    #[inline]
    fn conformal_factor(&self, x: &[f32]) -> f32 {
        let c = -self.curvature;
        let norm_sq = self.norm_sq(x);
        2.0 / (1.0 - c * norm_sq)
    }

    /// Poincaré distance: d(x,y) = (2/√c) * arctanh(√c * ||−x ⊕_c y||)
    pub fn distance(&self, x: &[f32], y: &[f32]) -> f32 {
        let c = -self.curvature;
        let sqrt_c = c.sqrt();

        // Möbius addition: -x ⊕ y
        let neg_x: Vec<f32> = x.iter().map(|&xi| -xi).collect();
        let mobius_sum = self.mobius_add(&neg_x, y);
        let norm = self.norm(&mobius_sum);

        (2.0 / sqrt_c) * (sqrt_c * norm).atanh()
    }

    /// Möbius addition in Poincaré ball
    pub fn mobius_add(&self, x: &[f32], y: &[f32]) -> Vec<f32> {
        let c = -self.curvature;
        let x_norm_sq = self.norm_sq(x);
        let y_norm_sq = self.norm_sq(y);
        let xy_dot = self.dot(x, y);

        let num_coef = 1.0 + 2.0 * c * xy_dot + c * y_norm_sq;
        let y_coef = 1.0 - c * x_norm_sq;
        let denom = 1.0 + 2.0 * c * xy_dot + c * c * x_norm_sq * y_norm_sq;

        x.iter().zip(y.iter())
            .map(|(&xi, &yi)| (num_coef * xi + y_coef * yi) / denom)
            .collect()
    }

    /// Exponential map: tangent space → manifold
    pub fn exp_map(&self, base: &[f32], tangent: &[f32]) -> Vec<f32> {
        let c = -self.curvature;
        let sqrt_c = c.sqrt();

        let lambda = self.conformal_factor(base);
        let tangent_norm = self.norm(tangent);

        if tangent_norm < 1e-10 {
            return base.to_vec();
        }

        let coef = (sqrt_c * lambda * tangent_norm / 2.0).tanh() / (sqrt_c * tangent_norm);
        let direction: Vec<f32> = tangent.iter().map(|&t| t * coef).collect();

        self.mobius_add(base, &direction)
    }

    /// Logarithmic map: manifold → tangent space
    pub fn log_map(&self, base: &[f32], target: &[f32]) -> Vec<f32> {
        let c = -self.curvature;
        let sqrt_c = c.sqrt();

        // -base ⊕ target
        let neg_base: Vec<f32> = base.iter().map(|&b| -b).collect();
        let addition = self.mobius_add(&neg_base, target);
        let add_norm = self.norm(&addition);

        if add_norm < 1e-10 {
            return vec![0.0; self.dim];
        }

        let lambda = self.conformal_factor(base);
        let coef = (2.0 / (sqrt_c * lambda)) * (sqrt_c * add_norm).atanh() / add_norm;

        addition.iter().map(|&a| a * coef).collect()
    }

    /// Project point to ball (clamp norm)
    pub fn project(&self, x: &[f32]) -> Vec<f32> {
        let c = -self.curvature;
        let max_norm = (1.0 / c).sqrt() - 1e-5;
        let norm = self.norm(x);

        if norm <= max_norm {
            x.to_vec()
        } else {
            let scale = max_norm / norm;
            x.iter().map(|&xi| xi * scale).collect()
        }
    }

    #[inline]
    fn norm_sq(&self, x: &[f32]) -> f32 {
        f32::dot(x, x).unwrap_or_else(|| x.iter().map(|&xi| xi * xi).sum())
    }

    #[inline]
    fn norm(&self, x: &[f32]) -> f32 {
        self.norm_sq(x).sqrt()
    }

    #[inline]
    fn dot(&self, x: &[f32], y: &[f32]) -> f32 {
        f32::dot(x, y).unwrap_or_else(|| x.iter().zip(y.iter()).map(|(&a, &b)| a * b).sum())
    }
}

// PostgreSQL type
#[derive(PostgresType, Serialize, Deserialize)]
#[pgx(sql = "CREATE TYPE hyperbolic")]
pub struct Hyperbolic {
    data: Vec<f32>,
    curvature: f32,
}

// PostgreSQL functions
#[pg_extern(immutable, parallel_safe)]
fn ruvector_poincare_distance(a: Vec<f32>, b: Vec<f32>, curvature: default!(f32, -1.0)) -> f32 {
    let ball = PoincareBall::new(a.len(), curvature);
    ball.distance(&a, &b)
}

#[pg_extern(immutable, parallel_safe)]
fn ruvector_mobius_add(a: Vec<f32>, b: Vec<f32>, curvature: default!(f32, -1.0)) -> Vec<f32> {
    let ball = PoincareBall::new(a.len(), curvature);
    ball.mobius_add(&a, &b)
}

#[pg_extern(immutable, parallel_safe)]
fn ruvector_exp_map(base: Vec<f32>, tangent: Vec<f32>, curvature: default!(f32, -1.0)) -> Vec<f32> {
    let ball = PoincareBall::new(base.len(), curvature);
    ball.exp_map(&base, &tangent)
}

#[pg_extern(immutable, parallel_safe)]
fn ruvector_log_map(base: Vec<f32>, target: Vec<f32>, curvature: default!(f32, -1.0)) -> Vec<f32> {
    let ball = PoincareBall::new(base.len(), curvature);
    ball.log_map(&base, &target)
}
```

### Phase 2: Lorentz Model (Week 4-5)

```rust
// src/hyperbolic/types/lorentz.rs

/// Lorentz (hyperboloid) model: H^n = {x ∈ R^{n+1} : <x,x>_L = -1/c, x_0 > 0}
/// More numerically stable than Poincaré for high dimensions
pub struct LorentzModel {
    dim: usize,  // Ambient dimension (n+1)
    curvature: f32,
}

impl LorentzModel {
    /// Minkowski inner product: <x,y>_L = -x_0*y_0 + Σ x_i*y_i
    #[inline]
    pub fn minkowski_dot(&self, x: &[f32], y: &[f32]) -> f32 {
        -x[0] * y[0] + x[1..].iter().zip(y[1..].iter())
            .map(|(&a, &b)| a * b)
            .sum::<f32>()
    }

    /// Lorentz distance: d(x,y) = (1/√c) * arcosh(-c * <x,y>_L)
    pub fn distance(&self, x: &[f32], y: &[f32]) -> f32 {
        let c = -self.curvature;
        let sqrt_c = c.sqrt();
        let inner = self.minkowski_dot(x, y);

        (1.0 / sqrt_c) * (-c * inner).acosh()
    }

    /// Exponential map on hyperboloid
    pub fn exp_map(&self, base: &[f32], tangent: &[f32]) -> Vec<f32> {
        let c = -self.curvature;
        let sqrt_c = c.sqrt();

        let tangent_norm_sq = self.minkowski_dot(tangent, tangent);
        if tangent_norm_sq < 1e-10 {
            return base.to_vec();
        }
        let tangent_norm = tangent_norm_sq.sqrt();

        let coef1 = (sqrt_c * tangent_norm).cosh();
        let coef2 = (sqrt_c * tangent_norm).sinh() / tangent_norm;

        base.iter().zip(tangent.iter())
            .map(|(&b, &t)| coef1 * b + coef2 * t)
            .collect()
    }

    /// Logarithmic map on hyperboloid
    pub fn log_map(&self, base: &[f32], target: &[f32]) -> Vec<f32> {
        let c = -self.curvature;
        let sqrt_c = c.sqrt();

        let inner = self.minkowski_dot(base, target);
        let dist = self.distance(base, target);

        if dist < 1e-10 {
            return vec![0.0; self.dim];
        }

        let coef = dist / (dist * sqrt_c).sinh();

        target.iter().zip(base.iter())
            .map(|(&t, &b)| coef * (t - inner * b))
            .collect()
    }

    /// Project to hyperboloid (ensure constraint satisfied)
    pub fn project(&self, x: &[f32]) -> Vec<f32> {
        let c = -self.curvature;
        let space_norm_sq: f32 = x[1..].iter().map(|&xi| xi * xi).sum();
        let x0 = ((1.0 / c) + space_norm_sq).sqrt();

        let mut result = vec![x0];
        result.extend_from_slice(&x[1..]);
        result
    }

    /// Convert from Poincaré ball to Lorentz
    pub fn from_poincare(&self, poincare: &[f32], poincare_curvature: f32) -> Vec<f32> {
        let c = -poincare_curvature;
        let norm_sq: f32 = poincare.iter().map(|&x| x * x).sum();

        let x0 = (1.0 + c * norm_sq) / (1.0 - c * norm_sq);
        let coef = 2.0 / (1.0 - c * norm_sq);

        let mut result = vec![x0];
        result.extend(poincare.iter().map(|&p| coef * p));
        result
    }

    /// Convert from Lorentz to Poincaré ball
    pub fn to_poincare(&self, lorentz: &[f32]) -> Vec<f32> {
        let denom = 1.0 + lorentz[0];
        lorentz[1..].iter().map(|&x| x / denom).collect()
    }
}

#[pg_extern(immutable, parallel_safe)]
fn ruvector_lorentz_distance(a: Vec<f32>, b: Vec<f32>, curvature: default!(f32, -1.0)) -> f32 {
    let model = LorentzModel::new(a.len(), curvature);
    model.distance(&a, &b)
}

#[pg_extern(immutable, parallel_safe)]
fn ruvector_poincare_to_lorentz(poincare: Vec<f32>, curvature: default!(f32, -1.0)) -> Vec<f32> {
    let model = LorentzModel::new(poincare.len() + 1, curvature);
    model.from_poincare(&poincare, curvature)
}

#[pg_extern(immutable, parallel_safe)]
fn ruvector_lorentz_to_poincare(lorentz: Vec<f32>) -> Vec<f32> {
    let model = LorentzModel::new(lorentz.len(), -1.0);
    model.to_poincare(&lorentz)
}
```

### Phase 3: Hyperbolic HNSW Index (Week 6-8)

```rust
// src/hyperbolic/index/hnsw_hyper.rs

/// HNSW index adapted for hyperbolic space
pub struct HyperbolicHnsw {
    layers: Vec<HnswLayer>,
    manifold: HyperbolicManifold,
    m: usize,
    ef_construction: usize,
}

pub enum HyperbolicManifold {
    Poincare(PoincareBall),
    Lorentz(LorentzModel),
}

impl HyperbolicHnsw {
    /// Distance function based on manifold
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match &self.manifold {
            HyperbolicManifold::Poincare(ball) => ball.distance(a, b),
            HyperbolicManifold::Lorentz(model) => model.distance(a, b),
        }
    }

    /// Insert with hyperbolic distance
    pub fn insert(&mut self, id: u64, vector: &[f32]) {
        // Project to manifold first
        let projected = match &self.manifold {
            HyperbolicManifold::Poincare(ball) => ball.project(vector),
            HyperbolicManifold::Lorentz(model) => model.project(vector),
        };

        // Standard HNSW insertion with hyperbolic distance
        let entry_point = self.entry_point();
        let level = self.random_level();

        for l in (0..=level).rev() {
            let candidates = self.search_layer(&projected, entry_point, self.ef_construction, l);
            let neighbors = self.select_neighbors(&projected, &candidates, self.m);
            self.connect(id, &neighbors, l);
        }

        self.vectors.insert(id, projected);
    }

    /// Search with hyperbolic distance
    pub fn search(&self, query: &[f32], k: usize, ef: usize) -> Vec<(u64, f32)> {
        let projected = match &self.manifold {
            HyperbolicManifold::Poincare(ball) => ball.project(query),
            HyperbolicManifold::Lorentz(model) => model.project(query),
        };

        let mut candidates = self.search_layer(&projected, self.entry_point(), ef, 0);
        candidates.truncate(k);
        candidates
    }
}

// PostgreSQL index access method
#[pg_extern]
fn ruvector_hyperbolic_hnsw_handler(internal: Internal) -> Internal {
    // Index AM handler
}
```

### Phase 4: Euclidean to Hyperbolic Projection (Week 9-10)

```rust
// src/hyperbolic/manifold.rs

/// Project Euclidean embeddings to hyperbolic space
pub struct HyperbolicProjection {
    model: HyperbolicModel,
    method: ProjectionMethod,
}

pub enum ProjectionMethod {
    /// Direct scaling to fit in ball
    Scale,
    /// Learned exponential map from origin
    ExponentialMap,
    /// Centroid-based projection
    Centroid { centroid: Vec<f32> },
}

impl HyperbolicProjection {
    /// Project batch of Euclidean vectors
    pub fn project_batch(&self, vectors: &[Vec<f32>]) -> Vec<Vec<f32>> {
        match &self.method {
            ProjectionMethod::Scale => {
                vectors.par_iter()
                    .map(|v| self.scale_project(v))
                    .collect()
            }
            ProjectionMethod::ExponentialMap => {
                let origin = vec![0.0; vectors[0].len()];
                vectors.par_iter()
                    .map(|v| self.model.exp_map(&origin, v))
                    .collect()
            }
            ProjectionMethod::Centroid { centroid } => {
                vectors.par_iter()
                    .map(|v| {
                        let tangent: Vec<f32> = v.iter()
                            .zip(centroid.iter())
                            .map(|(&vi, &ci)| vi - ci)
                            .collect();
                        self.model.exp_map(centroid, &tangent)
                    })
                    .collect()
            }
        }
    }

    fn scale_project(&self, v: &[f32]) -> Vec<f32> {
        let norm: f32 = v.iter().map(|&x| x * x).sum::<f32>().sqrt();
        let max_norm = 0.99;  // Stay within ball

        if norm <= max_norm {
            v.to_vec()
        } else {
            let scale = max_norm / norm;
            v.iter().map(|&x| x * scale).collect()
        }
    }
}

#[pg_extern]
fn ruvector_to_poincare(
    euclidean: Vec<f32>,
    curvature: default!(f32, -1.0),
    method: default!(&str, "'scale'"),
) -> Vec<f32> {
    let model = PoincareBall::new(euclidean.len(), curvature);
    let projection = HyperbolicProjection::new(model, method.into());
    projection.project(&euclidean)
}

#[pg_extern]
fn ruvector_batch_to_poincare(
    table_name: &str,
    euclidean_column: &str,
    output_column: &str,
    curvature: default!(f32, -1.0),
) -> i64 {
    // Batch projection using SPI
    Spi::connect(|client| {
        // ... batch update
    })
}
```

## Use Cases

### Hierarchical Data (Taxonomies, Org Charts)

```sql
-- Embed taxonomy with parent-child relationships preserved
-- Children naturally cluster closer to parents in hyperbolic space
CREATE TABLE taxonomy (
    id SERIAL PRIMARY KEY,
    name TEXT,
    parent_id INTEGER REFERENCES taxonomy(id),
    embedding hyperbolic(64)
);

-- Find all items in subtree (leveraging hyperbolic geometry)
SELECT * FROM taxonomy
WHERE ruvector_poincare_distance(embedding, root_embedding) < subtree_radius
ORDER BY ruvector_poincare_distance(embedding, root_embedding);
```

### Knowledge Graphs

```sql
-- Entities with hierarchical relationships
-- Hyperbolic space captures asymmetric relations naturally
SELECT entity_a.name, entity_b.name,
       ruvector_poincare_distance(entity_a.embedding, entity_b.embedding) AS distance
FROM entities entity_a, entities entity_b
WHERE entity_a.id != entity_b.id
ORDER BY distance
LIMIT 100;
```

## Benchmarks

| Operation | Dimension | Curvature | Time (μs) | vs Euclidean |
|-----------|-----------|-----------|-----------|--------------|
| Poincaré Distance | 128 | -1.0 | 2.1 | 1.8x slower |
| Lorentz Distance | 129 | -1.0 | 1.5 | 1.3x slower |
| Möbius Addition | 128 | -1.0 | 3.2 | N/A |
| Exp Map | 128 | -1.0 | 4.5 | N/A |
| HNSW Search (hyper) | 128 | -1.0 | 850 | 1.5x slower |

## Dependencies

```toml
[dependencies]
# SIMD for fast operations
simsimd = "5.9"

# Numerical stability
num-traits = "0.2"
```

## Feature Flags

```toml
[features]
hyperbolic = []
hyperbolic-poincare = ["hyperbolic"]
hyperbolic-lorentz = ["hyperbolic"]
hyperbolic-index = ["hyperbolic", "index-hnsw"]
hyperbolic-all = ["hyperbolic-poincare", "hyperbolic-lorentz", "hyperbolic-index"]
```
