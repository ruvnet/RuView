//! Stage I Encoder: Ideogram Gestalts via Poincaré Ball Embeddings
//!
//! CRV Stage I captures gestalt primitives (manmade, natural, movement, energy,
//! water, land) through ideogram traces. The hierarchical taxonomy of gestalts
//! maps naturally to hyperbolic space, where the Poincaré ball model encodes
//! tree-like structures with exponentially less distortion than Euclidean space.
//!
//! # Architecture
//!
//! Ideogram stroke traces are converted to fixed-dimension feature vectors,
//! then projected into the Poincaré ball. Gestalt classification uses hyperbolic
//! distance to prototype embeddings for each gestalt type.

use crate::error::{CrvError, CrvResult};
use crate::types::{CrvConfig, GestaltType, StageIData};
use ruvector_attention::hyperbolic::{
    exp_map, frechet_mean, log_map, mobius_add, poincare_distance, project_to_ball,
};

/// Stage I encoder using Poincaré ball hyperbolic embeddings.
#[derive(Debug, Clone)]
pub struct StageIEncoder {
    /// Embedding dimensionality.
    dim: usize,
    /// Poincaré ball curvature (positive).
    curvature: f32,
    /// Prototype embeddings for each gestalt type in the Poincaré ball.
    /// Indexed by `GestaltType::index()`.
    prototypes: Vec<Vec<f32>>,
}

impl StageIEncoder {
    /// Create a new Stage I encoder with default gestalt prototypes.
    pub fn new(config: &CrvConfig) -> Self {
        let dim = config.dimensions;
        let curvature = config.curvature;

        // Initialize gestalt prototypes as points in the Poincaré ball.
        // Each prototype is placed at a distinct region of the ball,
        // with hierarchical relationships preserved by hyperbolic distance.
        let prototypes = Self::init_prototypes(dim, curvature);

        Self {
            dim,
            curvature,
            prototypes,
        }
    }

    /// Initialize gestalt prototype embeddings in the Poincaré ball.
    ///
    /// Places each gestalt type at a distinct angular position with
    /// controlled radial distance from the origin. The hierarchical
    /// structure (root → gestalt types → sub-types) is preserved
    /// by the exponential volume growth of hyperbolic space.
    fn init_prototypes(dim: usize, curvature: f32) -> Vec<Vec<f32>> {
        let num_types = GestaltType::all().len();
        let mut prototypes = Vec::with_capacity(num_types);

        for gestalt in GestaltType::all() {
            let idx = gestalt.index();
            // Place each prototype along a different axis direction
            // with a moderate radial distance (0.3-0.5 of ball radius).
            let mut proto = vec![0.0f32; dim];

            // Use multiple dimensions to spread prototypes apart
            let base_dim = idx * (dim / num_types);
            let spread = dim / num_types;

            for d in 0..spread.min(dim - base_dim) {
                let angle = std::f32::consts::PI * 2.0 * (d as f32) / (spread as f32);
                proto[base_dim + d] = 0.3 * angle.cos() / (spread as f32).sqrt();
            }

            // Project to ball to ensure it's inside
            proto = project_to_ball(&proto, curvature, 1e-7);
            prototypes.push(proto);
        }

        prototypes
    }

    /// Encode an ideogram stroke trace into a fixed-dimension feature vector.
    ///
    /// Extracts geometric features from the stroke: curvature statistics,
    /// velocity profile, angular distribution, and bounding box ratios.
    pub fn encode_stroke(&self, stroke: &[(f32, f32)]) -> CrvResult<Vec<f32>> {
        if stroke.is_empty() {
            return Err(CrvError::EmptyInput("Stroke trace is empty".to_string()));
        }

        let mut features = vec![0.0f32; self.dim];

        // Feature 1: Stroke statistics (first few dimensions)
        let n = stroke.len() as f32;
        let (cx, cy) = stroke
            .iter()
            .fold((0.0, 0.0), |(sx, sy), &(x, y)| (sx + x, sy + y));
        features[0] = cx / n; // centroid x
        features[1] = cy / n; // centroid y

        // Feature 2: Bounding box aspect ratio
        let (min_x, max_x) = stroke
            .iter()
            .map(|p| p.0)
            .fold((f32::MAX, f32::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));
        let (min_y, max_y) = stroke
            .iter()
            .map(|p| p.1)
            .fold((f32::MAX, f32::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));
        let width = (max_x - min_x).max(1e-6);
        let height = (max_y - min_y).max(1e-6);
        features[2] = width / height; // aspect ratio

        // Feature 3: Total path length (normalized)
        let mut path_length = 0.0f32;
        for i in 1..stroke.len() {
            let dx = stroke[i].0 - stroke[i - 1].0;
            let dy = stroke[i].1 - stroke[i - 1].1;
            path_length += (dx * dx + dy * dy).sqrt();
        }
        features[3] = path_length / (width + height).max(1e-6);

        // Feature 4: Angular distribution (segment angles)
        if stroke.len() >= 3 {
            let num_angle_bins = 8.min(self.dim.saturating_sub(4));
            for i in 1..stroke.len().saturating_sub(1) {
                let dx1 = stroke[i].0 - stroke[i - 1].0;
                let dy1 = stroke[i].1 - stroke[i - 1].1;
                let dx2 = stroke[i + 1].0 - stroke[i].0;
                let dy2 = stroke[i + 1].1 - stroke[i].1;
                let angle = dy1.atan2(dx1) - dy2.atan2(dx2);
                let bin = ((angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI)
                    * num_angle_bins as f32) as usize;
                let bin = bin.min(num_angle_bins - 1);
                if 4 + bin < self.dim {
                    features[4 + bin] += 1.0 / (stroke.len() as f32 - 2.0).max(1.0);
                }
            }
        }

        // Feature 5: Curvature variance (spread across remaining dimensions)
        if stroke.len() >= 3 {
            let mut curvatures = Vec::new();
            for i in 1..stroke.len() - 1 {
                let dx1 = stroke[i].0 - stroke[i - 1].0;
                let dy1 = stroke[i].1 - stroke[i - 1].1;
                let dx2 = stroke[i + 1].0 - stroke[i].0;
                let dy2 = stroke[i + 1].1 - stroke[i].1;
                let cross = dx1 * dy2 - dy1 * dx2;
                let ds1 = (dx1 * dx1 + dy1 * dy1).sqrt().max(1e-6);
                let ds2 = (dx2 * dx2 + dy2 * dy2).sqrt().max(1e-6);
                curvatures.push(cross / (ds1 * ds2));
            }
            if !curvatures.is_empty() {
                let mean_k: f32 = curvatures.iter().sum::<f32>() / curvatures.len() as f32;
                let var_k: f32 = curvatures.iter().map(|k| (k - mean_k).powi(2)).sum::<f32>()
                    / curvatures.len() as f32;
                if 12 < self.dim {
                    features[12] = mean_k;
                }
                if 13 < self.dim {
                    features[13] = var_k;
                }
            }
        }

        // Normalize the feature vector
        let norm: f32 = features.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            let scale = 0.4 / norm; // keep within ball
            for f in &mut features {
                *f *= scale;
            }
        }

        Ok(features)
    }

    /// Encode complete Stage I data into a Poincaré ball embedding.
    ///
    /// Combines stroke features with the gestalt prototype via Möbius addition,
    /// producing a vector that encodes both the raw ideogram trace and its
    /// gestalt classification in hyperbolic space.
    pub fn encode(&self, data: &StageIData) -> CrvResult<Vec<f32>> {
        let stroke_features = self.encode_stroke(&data.stroke)?;

        // Get the prototype for the classified gestalt type
        let prototype = &self.prototypes[data.classification.index()];

        // Combine stroke features with gestalt prototype via Möbius addition.
        // This places the encoded vector near the gestalt prototype in
        // hyperbolic space, with the stroke features providing the offset.
        let combined = mobius_add(&stroke_features, prototype, self.curvature);

        // Weight by confidence
        let weighted: Vec<f32> = combined
            .iter()
            .map(|&v| v * data.confidence + stroke_features[0] * (1.0 - data.confidence))
            .collect();

        Ok(project_to_ball(&weighted, self.curvature, 1e-7))
    }

    /// Classify a stroke embedding into a gestalt type by finding the
    /// nearest prototype in hyperbolic space.
    pub fn classify(&self, embedding: &[f32]) -> CrvResult<(GestaltType, f32)> {
        if embedding.len() != self.dim {
            return Err(CrvError::DimensionMismatch {
                expected: self.dim,
                actual: embedding.len(),
            });
        }

        let mut best_type = GestaltType::Manmade;
        let mut best_distance = f32::MAX;

        for gestalt in GestaltType::all() {
            let proto = &self.prototypes[gestalt.index()];
            let dist = poincare_distance(embedding, proto, self.curvature);
            if dist < best_distance {
                best_distance = dist;
                best_type = *gestalt;
            }
        }

        // Convert distance to confidence (closer = higher confidence)
        let confidence = (-best_distance).exp();

        Ok((best_type, confidence))
    }

    /// Compute the Fréchet mean of multiple Stage I embeddings.
    ///
    /// Useful for finding the consensus gestalt across multiple sessions
    /// targeting the same coordinate.
    pub fn consensus(&self, embeddings: &[&[f32]]) -> CrvResult<Vec<f32>> {
        if embeddings.is_empty() {
            return Err(CrvError::EmptyInput(
                "No embeddings for consensus".to_string(),
            ));
        }

        Ok(frechet_mean(embeddings, None, self.curvature, 50, 1e-5))
    }

    /// Compute pairwise hyperbolic distance between two Stage I embeddings.
    pub fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        poincare_distance(a, b, self.curvature)
    }

    /// Get the prototype embedding for a gestalt type.
    pub fn prototype(&self, gestalt: GestaltType) -> &[f32] {
        &self.prototypes[gestalt.index()]
    }

    /// Map an embedding to tangent space at the origin for Euclidean operations.
    pub fn to_tangent(&self, embedding: &[f32]) -> Vec<f32> {
        let origin = vec![0.0f32; self.dim];
        log_map(embedding, &origin, self.curvature)
    }

    /// Map a tangent vector back to the Poincaré ball.
    pub fn from_tangent(&self, tangent: &[f32]) -> Vec<f32> {
        let origin = vec![0.0f32; self.dim];
        exp_map(tangent, &origin, self.curvature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CrvConfig {
        CrvConfig {
            dimensions: 32,
            curvature: 1.0,
            ..CrvConfig::default()
        }
    }

    #[test]
    fn test_encoder_creation() {
        let config = test_config();
        let encoder = StageIEncoder::new(&config);
        assert_eq!(encoder.dim, 32);
        assert_eq!(encoder.prototypes.len(), 6);
    }

    #[test]
    fn test_stroke_encoding() {
        let config = test_config();
        let encoder = StageIEncoder::new(&config);

        let stroke = vec![(0.0, 0.0), (1.0, 0.5), (2.0, 1.0), (3.0, 0.5), (4.0, 0.0)];
        let embedding = encoder.encode_stroke(&stroke).unwrap();
        assert_eq!(embedding.len(), 32);

        // Should be inside the Poincaré ball
        let norm_sq: f32 = embedding.iter().map(|x| x * x).sum();
        assert!(norm_sq < 1.0 / config.curvature);
    }

    #[test]
    fn test_full_encode() {
        let config = test_config();
        let encoder = StageIEncoder::new(&config);

        let data = StageIData {
            stroke: vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)],
            spontaneous_descriptor: "angular".to_string(),
            classification: GestaltType::Manmade,
            confidence: 0.9,
        };

        let embedding = encoder.encode(&data).unwrap();
        assert_eq!(embedding.len(), 32);
    }

    #[test]
    fn test_classification() {
        let config = test_config();
        let encoder = StageIEncoder::new(&config);

        // Encode and classify should round-trip for strong prototypes
        let proto = encoder.prototype(GestaltType::Energy).to_vec();
        let (classified, confidence) = encoder.classify(&proto).unwrap();
        assert_eq!(classified, GestaltType::Energy);
        assert!(confidence > 0.5);
    }

    #[test]
    fn test_distance_symmetry() {
        let config = test_config();
        let encoder = StageIEncoder::new(&config);

        let a = encoder.prototype(GestaltType::Manmade);
        let b = encoder.prototype(GestaltType::Natural);

        let d_ab = encoder.distance(a, b);
        let d_ba = encoder.distance(b, a);

        assert!((d_ab - d_ba).abs() < 1e-5);
    }

    #[test]
    fn test_tangent_roundtrip() {
        let config = test_config();
        let encoder = StageIEncoder::new(&config);

        let proto = encoder.prototype(GestaltType::Water).to_vec();
        let tangent = encoder.to_tangent(&proto);
        let recovered = encoder.from_tangent(&tangent);

        // Should approximately round-trip
        let error: f32 = proto
            .iter()
            .zip(&recovered)
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / proto.len() as f32;
        assert!(error < 0.1);
    }
}
