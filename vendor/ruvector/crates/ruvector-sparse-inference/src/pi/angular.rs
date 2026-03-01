//! Angular and hyperspherical embeddings with π phase encoding
//!
//! Many embedding tricks quietly reduce to angles. Cosine similarity is
//! literally angle-based.
//!
//! Using π explicitly:
//! - Map vectors to phase space
//! - Encode direction as multiples of π
//! - Track angular velocity instead of Euclidean distance
//!
//! This is extremely friendly to 5-bit and 7-bit systems because:
//! - Angles saturate naturally
//! - Wraparound is meaningful
//! - Overflow becomes topology, not error
//!
//! That is exactly how biological systems avoid numeric explosion.

use crate::precision::PrecisionLane;
use std::f32::consts::PI;

/// Angular embedding projector
#[derive(Debug, Clone)]
pub struct AngularEmbedding {
    /// Precision lane
    lane: PrecisionLane,
    /// Dimension of embeddings
    dimension: usize,
    /// Phase scale (π / max_value for lane)
    phase_scale: f32,
    /// Angular velocity accumulator
    velocity: Vec<f32>,
}

impl AngularEmbedding {
    /// Create a new angular embedding projector
    pub fn new(lane: PrecisionLane) -> Self {
        let phase_scale = match lane {
            PrecisionLane::Bit3 => PI / 4.0,
            PrecisionLane::Bit5 => PI / 16.0,
            PrecisionLane::Bit7 => PI / 64.0,
            PrecisionLane::Float32 => 1.0,
        };

        Self {
            lane,
            dimension: 0,
            phase_scale,
            velocity: Vec::new(),
        }
    }

    /// Project Euclidean vector to angular space
    pub fn project(&self, values: &[f32]) -> Vec<f32> {
        // Compute magnitude for normalization
        let magnitude = values.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-10);

        // Project to unit hypersphere, then to angles
        values
            .iter()
            .map(|&x| {
                let normalized = x / magnitude;
                // Map [-1, 1] to [-π, π] with phase scale
                normalized * PI * self.phase_scale
            })
            .collect()
    }

    /// Unproject from angular space to Euclidean
    pub fn unproject(&self, angles: &[f32], target_magnitude: f32) -> Vec<f32> {
        angles
            .iter()
            .map(|&angle| {
                let normalized = angle / (PI * self.phase_scale);
                normalized * target_magnitude
            })
            .collect()
    }

    /// Compute angular distance between two vectors
    pub fn angular_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return f32::MAX;
        }

        let angles_a = self.project(a);
        let angles_b = self.project(b);

        // Sum of angular differences (with wraparound handling)
        let mut total_distance = 0.0f32;
        for (&a, &b) in angles_a.iter().zip(angles_b.iter()) {
            let diff = (a - b).abs();
            // Handle wraparound: use shorter arc
            let wrapped_diff = if diff > PI { 2.0 * PI - diff } else { diff };
            total_distance += wrapped_diff * wrapped_diff;
        }

        total_distance.sqrt()
    }

    /// Update angular velocity (for streaming embeddings)
    pub fn update_velocity(&mut self, previous: &[f32], current: &[f32]) {
        if previous.len() != current.len() {
            return;
        }

        let prev_angles = self.project(previous);
        let curr_angles = self.project(current);

        if self.velocity.is_empty() {
            self.velocity = vec![0.0; current.len()];
            self.dimension = current.len();
        }

        // Compute angular velocity (with momentum)
        let momentum = 0.9f32;
        for i in 0..self.dimension.min(self.velocity.len()) {
            let delta = curr_angles[i] - prev_angles[i];
            // Handle wraparound
            let wrapped_delta = if delta > PI {
                delta - 2.0 * PI
            } else if delta < -PI {
                delta + 2.0 * PI
            } else {
                delta
            };
            self.velocity[i] = momentum * self.velocity[i] + (1.0 - momentum) * wrapped_delta;
        }
    }

    /// Get current angular velocity
    pub fn get_velocity(&self) -> &[f32] {
        &self.velocity
    }

    /// Predict next position based on angular velocity
    pub fn predict_next(&self, current: &[f32]) -> Vec<f32> {
        let angles = self.project(current);
        if self.velocity.is_empty() {
            return current.to_vec();
        }

        let predicted_angles: Vec<f32> = angles
            .iter()
            .zip(self.velocity.iter())
            .map(|(&a, &v)| {
                let mut next = a + v;
                // Wrap to [-π, π]
                while next > PI {
                    next -= 2.0 * PI;
                }
                while next < -PI {
                    next += 2.0 * PI;
                }
                next
            })
            .collect();

        // Unproject with original magnitude
        let magnitude = current.iter().map(|x| x * x).sum::<f32>().sqrt();
        self.unproject(&predicted_angles, magnitude)
    }
}

/// Phase encoder for quantized values
#[derive(Debug, Clone)]
pub struct PhaseEncoder {
    /// Base frequency (multiples of π)
    base_frequency: f32,
    /// Number of harmonics
    harmonics: usize,
    /// Lookup table for fast encoding
    lut: Option<Vec<f32>>,
}

impl PhaseEncoder {
    /// Create a new phase encoder
    pub fn new(base_frequency: f32, harmonics: usize) -> Self {
        Self {
            base_frequency,
            harmonics,
            lut: None,
        }
    }

    /// Initialize lookup table for given quantization levels
    pub fn with_lut(mut self, levels: usize) -> Self {
        let mut lut = Vec::with_capacity(levels);
        for i in 0..levels {
            let normalized = (i as f32) / (levels - 1) as f32;
            let phase = normalized * 2.0 * PI * self.base_frequency;
            lut.push(phase.sin());
        }
        self.lut = Some(lut);
        self
    }

    /// Encode value to phase
    pub fn encode(&self, value: f32) -> f32 {
        let mut encoded = 0.0f32;
        for h in 0..self.harmonics {
            let freq = self.base_frequency * (h + 1) as f32;
            let weight = 1.0 / (h + 1) as f32; // Harmonic weights
            encoded += weight * (value * freq * PI).sin();
        }
        encoded
    }

    /// Encode quantized value using LUT
    pub fn encode_quantized(&self, level: usize) -> f32 {
        if let Some(ref lut) = self.lut {
            lut.get(level).copied().unwrap_or(0.0)
        } else {
            let normalized = level as f32 / 255.0; // Assume 8-bit max
            self.encode(normalized)
        }
    }

    /// Decode phase to approximate value
    pub fn decode(&self, phase: f32) -> f32 {
        // Inverse is approximate (lossy)
        phase.asin() / (self.base_frequency * PI)
    }
}

/// Hyperspherical projection for high-dimensional embeddings
#[derive(Debug, Clone)]
pub struct HypersphericalProjection {
    /// Input dimension
    input_dim: usize,
    /// Output spherical coordinates (n-1 angles for n dimensions)
    output_dim: usize,
    /// Precision lane
    lane: PrecisionLane,
}

impl HypersphericalProjection {
    /// Create a new hyperspherical projection
    pub fn new(dimension: usize, lane: PrecisionLane) -> Self {
        Self {
            input_dim: dimension,
            output_dim: dimension.saturating_sub(1),
            lane,
        }
    }

    /// Project Cartesian coordinates to hyperspherical (angles)
    pub fn to_spherical(&self, cartesian: &[f32]) -> Vec<f32> {
        if cartesian.len() < 2 {
            return vec![];
        }

        let n = cartesian.len();
        let mut angles = Vec::with_capacity(n - 1);

        // Radius (for reference, not returned)
        let r = cartesian.iter().map(|x| x * x).sum::<f32>().sqrt();
        if r < 1e-10 {
            return vec![0.0; n - 1];
        }

        // Compute angles from the last coordinate backward
        // φ₁ = arctan2(x₂, x₁)
        // φₖ = arccos(xₖ₊₁ / √(xₖ₊₁² + ... + xₙ²)) for k > 1

        // First angle (azimuthal)
        let phi_1 = cartesian[1].atan2(cartesian[0]);
        angles.push(phi_1);

        // Remaining angles (polar)
        for k in 1..(n - 1) {
            let tail_sum: f32 = cartesian[k..].iter().map(|x| x * x).sum();
            let tail_r = tail_sum.sqrt();
            if tail_r < 1e-10 {
                angles.push(0.0);
            } else {
                let phi_k = (cartesian[k] / tail_r).clamp(-1.0, 1.0).acos();
                angles.push(phi_k);
            }
        }

        angles
    }

    /// Project hyperspherical coordinates back to Cartesian
    pub fn to_cartesian(&self, angles: &[f32], radius: f32) -> Vec<f32> {
        if angles.is_empty() {
            return vec![];
        }

        let n = angles.len() + 1;
        let mut cartesian = Vec::with_capacity(n);

        // x₁ = r * sin(φₙ₋₁) * ... * sin(φ₂) * cos(φ₁)
        // x₂ = r * sin(φₙ₋₁) * ... * sin(φ₂) * sin(φ₁)
        // xₖ = r * sin(φₙ₋₁) * ... * sin(φₖ) * cos(φₖ₋₁) for k > 2
        // xₙ = r * cos(φₙ₋₁)

        let mut sin_product = radius;
        for &angle in angles.iter().rev().skip(1) {
            sin_product *= angle.sin();
        }

        // First two coordinates
        cartesian.push(sin_product * angles[0].cos());
        cartesian.push(sin_product * angles[0].sin());

        // Remaining coordinates
        sin_product = radius;
        for i in (1..angles.len()).rev() {
            sin_product *= angles[i].sin();
            cartesian.push(sin_product * angles[i - 1].cos());
        }

        // Last coordinate
        cartesian.push(radius * angles.last().unwrap_or(&0.0).cos());

        // Note: reconstruction may not be perfect for all inputs
        cartesian.truncate(n);
        cartesian
    }

    /// Compute geodesic distance on hypersphere
    pub fn geodesic_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return f32::MAX;
        }

        // Normalize to unit sphere
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-10);
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-10);

        // Compute dot product of normalized vectors
        let dot: f32 = a
            .iter()
            .zip(b.iter())
            .map(|(&x, &y)| (x / norm_a) * (y / norm_b))
            .sum();

        // Geodesic distance = arccos(dot product)
        dot.clamp(-1.0, 1.0).acos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_angular_embedding_project() {
        let embedding = AngularEmbedding::new(PrecisionLane::Bit5);
        let values = vec![1.0, 2.0, 3.0, 4.0];
        let angles = embedding.project(&values);

        assert_eq!(angles.len(), values.len());
        // All angles should be within bounds
        for &angle in &angles {
            assert!(angle.abs() <= PI);
        }
    }

    #[test]
    fn test_angular_embedding_roundtrip() {
        let embedding = AngularEmbedding::new(PrecisionLane::Bit7);
        let values = vec![1.0, 2.0, 3.0, 4.0];
        let magnitude = values.iter().map(|x| x * x).sum::<f32>().sqrt();

        let angles = embedding.project(&values);
        let recovered = embedding.unproject(&angles, magnitude);

        // Should approximately recover original
        for (&orig, &rec) in values.iter().zip(recovered.iter()) {
            assert!((orig - rec).abs() < 0.1, "orig={}, rec={}", orig, rec);
        }
    }

    #[test]
    fn test_angular_distance() {
        let embedding = AngularEmbedding::new(PrecisionLane::Bit5);

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let c = vec![1.0, 0.0, 0.0];

        let dist_ab = embedding.angular_distance(&a, &b);
        let dist_ac = embedding.angular_distance(&a, &c);

        assert!(dist_ac < 0.001); // Same vectors
        assert!(dist_ab > 0.0); // Different vectors
    }

    #[test]
    fn test_phase_encoder() {
        let encoder = PhaseEncoder::new(1.0, 3);

        let e1 = encoder.encode(0.0);
        let e2 = encoder.encode(0.5);
        let e3 = encoder.encode(1.0);

        // Different inputs should produce different outputs
        assert!(e1 != e2);
        assert!(e2 != e3);
    }

    #[test]
    fn test_phase_encoder_lut() {
        let encoder = PhaseEncoder::new(1.0, 1).with_lut(16);

        let e1 = encoder.encode_quantized(0);
        let e2 = encoder.encode_quantized(8);
        let e3 = encoder.encode_quantized(15);

        assert!(e1 != e2);
        assert!(e2 != e3);
    }

    #[test]
    fn test_hyperspherical_projection() {
        let proj = HypersphericalProjection::new(3, PrecisionLane::Bit5);

        let cartesian = vec![1.0, 0.0, 0.0];
        let spherical = proj.to_spherical(&cartesian);

        assert_eq!(spherical.len(), 2);
    }

    #[test]
    fn test_geodesic_distance() {
        let proj = HypersphericalProjection::new(3, PrecisionLane::Bit5);

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let c = vec![1.0, 0.0, 0.0];

        let dist_ab = proj.geodesic_distance(&a, &b);
        let dist_ac = proj.geodesic_distance(&a, &c);

        assert!(dist_ac < 0.001); // Same direction
        assert!((dist_ab - PI / 2.0).abs() < 0.001); // Orthogonal = π/2
    }
}
