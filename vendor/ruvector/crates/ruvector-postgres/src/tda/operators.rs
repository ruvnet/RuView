//! PostgreSQL operator functions for TDA.

use pgrx::prelude::*;
use pgrx::JsonB;

use ruvector_math::homology::{
    BirthDeathPair, BottleneckDistance, PersistenceDiagram, PersistentHomology, Point, PointCloud,
    VietorisRips, WassersteinDistance,
};

/// Helper: parse a JsonB array of points into a PointCloud.
fn parse_point_cloud(json: &JsonB) -> PointCloud {
    let points: Vec<Point> = json
        .0
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    if let Some(coords) = v.as_array() {
                        let c: Vec<f64> = coords.iter().filter_map(|x| x.as_f64()).collect();
                        if !c.is_empty() {
                            Some(Point::new(c))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    PointCloud::new(points)
}

/// Helper: parse a persistence diagram from JsonB pairs [[birth, death], ...].
fn parse_diagram(json: &JsonB) -> PersistenceDiagram {
    let mut diagram = PersistenceDiagram::new();

    if let Some(arr) = json.0.as_array() {
        for v in arr {
            if let Some(pair) = v.as_array() {
                if pair.len() >= 2 {
                    if let (Some(birth), Some(death)) = (pair[0].as_f64(), pair[1].as_f64()) {
                        diagram.add(BirthDeathPair::finite(0, birth, death));
                    }
                }
            }
        }
    }

    diagram
}

/// Compute persistent homology (Vietoris-Rips filtration).
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_persistent_homology(
    points_json: JsonB,
    max_dim: default!(i32, 1),
    max_radius: default!(f32, 3.0),
) -> JsonB {
    let cloud = parse_point_cloud(&points_json);
    if cloud.is_empty() {
        return JsonB(serde_json::json!([]));
    }

    let vr = VietorisRips::new(max_dim as usize, max_radius as f64);
    let filtration = vr.build(&cloud);
    let diagram = PersistentHomology::compute(&filtration);

    let result: Vec<serde_json::Value> = diagram
        .pairs
        .iter()
        .map(|pair| {
            serde_json::json!({
                "dimension": pair.dimension,
                "birth": pair.birth,
                "death": pair.death,
                "persistence": pair.persistence(),
            })
        })
        .collect();

    JsonB(serde_json::json!(result))
}

/// Compute Betti numbers at a given radius.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_betti_numbers(
    points_json: JsonB,
    radius: f32,
    max_dim: default!(i32, 2),
) -> Vec<i32> {
    let cloud = parse_point_cloud(&points_json);
    if cloud.is_empty() {
        return Vec::new();
    }

    let vr = VietorisRips::new(max_dim as usize, radius as f64 * 2.0);
    let filtration = vr.build(&cloud);
    let diagram = PersistentHomology::compute(&filtration);

    // Count intervals alive at the given radius
    let mut betti = vec![0i32; (max_dim + 1) as usize];
    for pair in &diagram.pairs {
        let death = pair.death.unwrap_or(f64::INFINITY);
        if pair.dimension <= max_dim as usize
            && pair.birth <= radius as f64
            && death > radius as f64
        {
            betti[pair.dimension] += 1;
        }
    }

    betti
}

/// Compute bottleneck distance between two persistence diagrams.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_bottleneck_distance(diag_a_json: JsonB, diag_b_json: JsonB) -> f32 {
    let diag_a = parse_diagram(&diag_a_json);
    let diag_b = parse_diagram(&diag_b_json);

    BottleneckDistance::compute(&diag_a, &diag_b, 0) as f32
}

/// Compute Wasserstein distance between two persistence diagrams.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_persistence_wasserstein(
    diag_a_json: JsonB,
    diag_b_json: JsonB,
    p: default!(i32, 2),
) -> f32 {
    let diag_a = parse_diagram(&diag_a_json);
    let diag_b = parse_diagram(&diag_b_json);

    let wd = WassersteinDistance::new(p as f64);
    wd.compute(&diag_a, &diag_b, 0) as f32
}

/// Compute topological summary (Betti numbers + persistence statistics + entropy).
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_topological_summary(points_json: JsonB, max_dim: default!(i32, 1)) -> JsonB {
    let cloud = parse_point_cloud(&points_json);
    if cloud.is_empty() {
        return JsonB(serde_json::json!({}));
    }

    // Use a large max_scale to capture all features
    let vr = VietorisRips::new(max_dim as usize, 1000.0);
    let filtration = vr.build(&cloud);
    let diagram = PersistentHomology::compute(&filtration);

    // Compute persistence statistics
    let persistences: Vec<f64> = diagram
        .pairs
        .iter()
        .filter(|p| !p.is_essential())
        .map(|p| p.persistence())
        .filter(|&p| p.is_finite())
        .collect();

    let total_persistence: f64 = persistences.iter().sum();
    let max_persistence = persistences.iter().cloned().fold(0.0f64, f64::max);
    let avg_persistence = if !persistences.is_empty() {
        total_persistence / persistences.len() as f64
    } else {
        0.0
    };

    // Persistence entropy
    let entropy = if total_persistence > 0.0 {
        persistences
            .iter()
            .map(|&p| {
                let prob = p / total_persistence;
                if prob > 0.0 {
                    -prob * prob.ln()
                } else {
                    0.0
                }
            })
            .sum::<f64>()
    } else {
        0.0
    };

    // Betti counts by dimension
    let mut betti_by_dim: std::collections::HashMap<usize, i32> = std::collections::HashMap::new();
    for pair in &diagram.pairs {
        *betti_by_dim.entry(pair.dimension).or_insert(0) += 1;
    }

    JsonB(serde_json::json!({
        "num_features": diagram.pairs.len(),
        "total_persistence": total_persistence,
        "max_persistence": max_persistence,
        "avg_persistence": avg_persistence,
        "persistence_entropy": entropy,
        "betti_counts": betti_by_dim,
    }))
}

/// Detect topological drift between old and new embeddings.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_embedding_drift(old_json: JsonB, new_json: JsonB) -> JsonB {
    let old_cloud = parse_point_cloud(&old_json);
    let new_cloud = parse_point_cloud(&new_json);

    if old_cloud.is_empty() || new_cloud.is_empty() {
        return JsonB(serde_json::json!({"drift_score": 0.0, "status": "insufficient_data"}));
    }

    let vr = VietorisRips::new(1, 1000.0);

    let old_filtration = vr.build(&old_cloud);
    let new_filtration = vr.build(&new_cloud);

    let old_diagram = PersistentHomology::compute(&old_filtration);
    let new_diagram = PersistentHomology::compute(&new_filtration);

    let bottleneck = BottleneckDistance::compute(&old_diagram, &new_diagram, 0);

    let wd = WassersteinDistance::new(2.0);
    let wasserstein = wd.compute(&old_diagram, &new_diagram, 0);

    let drift_score = (bottleneck + wasserstein) / 2.0;

    let status = if drift_score < 0.1 {
        "stable"
    } else if drift_score < 0.5 {
        "moderate_drift"
    } else {
        "significant_drift"
    };

    JsonB(serde_json::json!({
        "drift_score": drift_score,
        "bottleneck_distance": bottleneck,
        "wasserstein_distance": wasserstein,
        "old_features": old_diagram.pairs.len(),
        "new_features": new_diagram.pairs.len(),
        "status": status,
    }))
}

/// Build Vietoris-Rips simplicial complex.
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_vietoris_rips(
    points_json: JsonB,
    max_radius: default!(f32, 2.0),
    max_dim: default!(i32, 2),
) -> JsonB {
    let cloud = parse_point_cloud(&points_json);
    if cloud.is_empty() {
        return JsonB(serde_json::json!({"simplices": [], "num_simplices": 0}));
    }

    let vr = VietorisRips::new(max_dim as usize, max_radius as f64);
    let filtration = vr.build(&cloud);

    let simplices: Vec<serde_json::Value> = filtration
        .simplices
        .iter()
        .map(|fs| {
            serde_json::json!({
                "vertices": &fs.simplex.vertices,
                "dimension": fs.simplex.dim(),
                "filtration_value": fs.birth,
            })
        })
        .collect();

    let mut simplex_counts: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    for fs in &filtration.simplices {
        *simplex_counts.entry(fs.simplex.dim()).or_insert(0) += 1;
    }

    JsonB(serde_json::json!({
        "num_simplices": filtration.simplices.len(),
        "simplex_counts_by_dim": simplex_counts,
        "simplices": simplices,
    }))
}
