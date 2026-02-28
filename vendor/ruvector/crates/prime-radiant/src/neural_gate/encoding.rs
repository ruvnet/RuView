//! Hyperdimensional computing (HDC) encoding for witnesses.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Default hypervector dimension.
pub const DEFAULT_HDC_DIM: usize = 10000;

/// Operations on hypervectors.
pub trait HypervectorOps {
    /// Bind two hypervectors (element-wise XOR for binary, multiplication for real).
    fn bind(&self, other: &Self) -> Self;

    /// Bundle multiple hypervectors (element-wise majority vote).
    fn bundle(vectors: &[&Self]) -> Self
    where
        Self: Sized;

    /// Permute the hypervector (cyclic shift).
    fn permute(&self, shift: usize) -> Self;

    /// Compute cosine similarity with another hypervector.
    fn similarity(&self, other: &Self) -> f32;

    /// Normalize to unit length.
    fn normalize(&mut self);
}

/// Real-valued hypervector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypervector {
    /// Components of the hypervector.
    pub components: Vec<f32>,
}

impl Hypervector {
    /// Create a new zero hypervector.
    pub fn zeros(dim: usize) -> Self {
        Self {
            components: vec![0.0; dim],
        }
    }

    /// Create a new random hypervector (uniformly distributed).
    pub fn random(dim: usize) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut components = Vec::with_capacity(dim);
        let mut hasher = DefaultHasher::new();

        for i in 0..dim {
            i.hash(&mut hasher);
            let h = hasher.finish();
            // Map to [-1, 1]
            let value = (h as f32 / u64::MAX as f32) * 2.0 - 1.0;
            components.push(value);
            hasher = DefaultHasher::new();
            h.hash(&mut hasher);
        }

        Self { components }
    }

    /// Create from a scalar value.
    pub fn from_scalar(value: f32, dim: usize) -> Self {
        // Use the scalar to seed a deterministic random generator
        let seed = (value * 1000000.0) as u64;
        let mut components = Vec::with_capacity(dim);

        for i in 0..dim {
            // Simple LCG for deterministic generation
            let mixed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(i as u64);
            let normalized = (mixed as f32 / u64::MAX as f32) * 2.0 - 1.0;
            components.push(normalized);
        }

        Self { components }
    }

    /// Create from bytes (e.g., hash).
    pub fn from_bytes(bytes: &[u8], dim: usize) -> Self {
        let mut components = vec![0.0; dim];

        for (i, &b) in bytes.iter().enumerate() {
            let idx = i % dim;
            // Accumulate byte values into components
            components[idx] += (b as f32 / 255.0) * 2.0 - 1.0;
        }

        let mut hv = Self { components };
        hv.normalize();
        hv
    }

    /// Get the dimension.
    pub fn dim(&self) -> usize {
        self.components.len()
    }

    /// Compute the L2 norm.
    pub fn norm(&self) -> f32 {
        self.components.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Scale by a scalar.
    pub fn scale(&mut self, factor: f32) {
        for c in &mut self.components {
            *c *= factor;
        }
    }

    /// Add another hypervector.
    pub fn add(&mut self, other: &Self) {
        for (a, b) in self.components.iter_mut().zip(other.components.iter()) {
            *a += b;
        }
    }
}

impl HypervectorOps for Hypervector {
    fn bind(&self, other: &Self) -> Self {
        let components: Vec<f32> = self
            .components
            .iter()
            .zip(other.components.iter())
            .map(|(a, b)| a * b)
            .collect();
        Self { components }
    }

    fn bundle(vectors: &[&Self]) -> Self {
        if vectors.is_empty() {
            return Self::zeros(DEFAULT_HDC_DIM);
        }

        let dim = vectors[0].dim();
        let mut result = Self::zeros(dim);

        for v in vectors {
            result.add(v);
        }

        result.normalize();
        result
    }

    fn permute(&self, shift: usize) -> Self {
        let n = self.components.len();
        let shift = shift % n;
        let mut components = vec![0.0; n];

        for i in 0..n {
            components[(i + shift) % n] = self.components[i];
        }

        Self { components }
    }

    fn similarity(&self, other: &Self) -> f32 {
        let dot: f32 = self
            .components
            .iter()
            .zip(other.components.iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_a = self.norm();
        let norm_b = other.norm();

        if norm_a < 1e-10 || norm_b < 1e-10 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    fn normalize(&mut self) {
        let norm = self.norm();
        if norm > 1e-10 {
            for c in &mut self.components {
                *c /= norm;
            }
        }
    }
}

/// Encoded witness record as a hypervector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessEncoding {
    /// The hypervector encoding.
    pub hypervector: Hypervector,
    /// Original witness ID (for reference).
    pub witness_id: String,
    /// Energy at time of encoding.
    pub energy: f32,
    /// Decision (allow/deny).
    pub allow: bool,
    /// Timestamp of encoding.
    pub timestamp_ms: u64,
}

impl WitnessEncoding {
    /// Create a new witness encoding.
    pub fn new(
        witness_id: impl Into<String>,
        energy: f32,
        allow: bool,
        policy_hash: &[u8],
        dim: usize,
    ) -> Self {
        let witness_id = witness_id.into();

        // Create component hypervectors
        let energy_hv = Hypervector::from_scalar(energy, dim);
        let decision_hv = Hypervector::from_scalar(if allow { 1.0 } else { -1.0 }, dim);
        let policy_hv = Hypervector::from_bytes(policy_hash, dim);

        // Bind all components
        let bound = energy_hv.bind(&decision_hv).bind(&policy_hv);

        Self {
            hypervector: bound,
            witness_id,
            energy,
            allow,
            timestamp_ms: current_time_ms(),
        }
    }

    /// Get similarity to another encoding.
    pub fn similarity(&self, other: &Self) -> f32 {
        self.hypervector.similarity(&other.hypervector)
    }
}

/// HDC memory for storing and retrieving witness encodings.
pub struct HdcMemory {
    /// Stored encodings indexed by ID.
    encodings: HashMap<String, WitnessEncoding>,
    /// Hypervector dimension.
    dim: usize,
    /// Maximum capacity.
    capacity: usize,
}

impl HdcMemory {
    /// Create a new HDC memory.
    pub fn new(dim: usize, capacity: usize) -> Self {
        Self {
            encodings: HashMap::with_capacity(capacity),
            dim,
            capacity,
        }
    }

    /// Store an encoding.
    pub fn store(&mut self, encoding: WitnessEncoding) {
        // If at capacity, remove oldest
        if self.encodings.len() >= self.capacity {
            // Find oldest
            if let Some(oldest_id) = self
                .encodings
                .iter()
                .min_by_key(|(_, e)| e.timestamp_ms)
                .map(|(id, _)| id.clone())
            {
                self.encodings.remove(&oldest_id);
            }
        }

        self.encodings.insert(encoding.witness_id.clone(), encoding);
    }

    /// Retrieve encodings similar to a query.
    pub fn retrieve(&self, query: &Hypervector, threshold: f32) -> Vec<(String, f32)> {
        let mut results: Vec<_> = self
            .encodings
            .iter()
            .map(|(id, enc)| {
                let sim = enc.hypervector.similarity(query);
                (id.clone(), sim)
            })
            .filter(|(_, sim)| *sim >= threshold)
            .collect();

        // Sort by similarity descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        results
    }

    /// Get an encoding by ID.
    pub fn get(&self, id: &str) -> Option<&WitnessEncoding> {
        self.encodings.get(id)
    }

    /// Get the number of stored encodings.
    pub fn len(&self) -> usize {
        self.encodings.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.encodings.is_empty()
    }

    /// Clear all encodings.
    pub fn clear(&mut self) {
        self.encodings.clear();
    }
}

impl std::fmt::Debug for HdcMemory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdcMemory")
            .field("dim", &self.dim)
            .field("stored", &self.encodings.len())
            .field("capacity", &self.capacity)
            .finish()
    }
}

/// Get current time in milliseconds.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hypervector_operations() {
        let a = Hypervector::random(1000);
        let b = Hypervector::random(1000);

        // Self-similarity should be ~1
        let self_sim = a.similarity(&a);
        assert!((self_sim - 1.0).abs() < 0.01);

        // Random vectors should be nearly orthogonal
        let cross_sim = a.similarity(&b);
        assert!(cross_sim.abs() < 0.2);
    }

    #[test]
    fn test_hypervector_bind() {
        let a = Hypervector::from_scalar(1.0, 1000);
        let b = Hypervector::from_scalar(2.0, 1000);

        let bound = a.bind(&b);
        assert_eq!(bound.dim(), 1000);
    }

    #[test]
    fn test_witness_encoding() {
        let enc = WitnessEncoding::new("test_witness", 0.5, true, &[1, 2, 3, 4], 1000);

        assert_eq!(enc.witness_id, "test_witness");
        assert!(enc.allow);
    }

    #[test]
    fn test_hdc_memory() {
        let mut memory = HdcMemory::new(1000, 100);

        let enc = WitnessEncoding::new("w1", 0.5, true, &[1, 2, 3], 1000);
        memory.store(enc);

        assert_eq!(memory.len(), 1);
        assert!(memory.get("w1").is_some());
    }
}
