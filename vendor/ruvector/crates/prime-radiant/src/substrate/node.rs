//! SheafNode: Entity with fixed-dimensional state vector
//!
//! A node in the sheaf graph represents an entity carrying a state vector (the "stalk"
//! of the sheaf). Nodes are domain-agnostic and can represent:
//!
//! - Facts, hypotheses, beliefs (AI agents)
//! - Trades, positions, signals (finance)
//! - Vitals, diagnoses, treatments (medical)
//! - Sensor readings, goals, plans (robotics)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for a node
pub type NodeId = Uuid;

/// State vector type - fixed-dimensional f32 vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateVector {
    /// The raw vector data
    data: Vec<f32>,
    /// Dimensionality (cached for fast access)
    dim: usize,
}

impl StateVector {
    /// Create a new state vector from a slice
    #[inline]
    pub fn new(data: impl Into<Vec<f32>>) -> Self {
        let data = data.into();
        let dim = data.len();
        Self { data, dim }
    }

    /// Create a zero vector of given dimension
    #[inline]
    pub fn zeros(dim: usize) -> Self {
        Self {
            data: vec![0.0; dim],
            dim,
        }
    }

    /// Create a random unit vector (useful for initialization)
    pub fn random_unit(dim: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut data: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() - 0.5).collect();

        // Normalize to unit length
        let norm: f32 = data.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-10 {
            for x in &mut data {
                *x /= norm;
            }
        }

        Self { data, dim }
    }

    /// Get the dimension of the vector
    #[inline]
    pub fn dim(&self) -> usize {
        self.dim
    }

    /// Get the raw data as a slice
    #[inline]
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    /// Get the raw data as a mutable slice
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        &mut self.data
    }

    /// Compute L2 norm squared (for energy calculations)
    ///
    /// SIMD-optimized: Uses chunks_exact for proper auto-vectorization.
    #[inline]
    pub fn norm_squared(&self) -> f32 {
        // Process 4 elements at a time for auto-vectorization
        let chunks = self.data.chunks_exact(4);
        let remainder = chunks.remainder();

        let mut acc = [0.0f32; 4];
        for chunk in chunks {
            acc[0] += chunk[0] * chunk[0];
            acc[1] += chunk[1] * chunk[1];
            acc[2] += chunk[2] * chunk[2];
            acc[3] += chunk[3] * chunk[3];
        }

        let mut sum = acc[0] + acc[1] + acc[2] + acc[3];
        for &x in remainder {
            sum += x * x;
        }
        sum
    }

    /// Compute L2 norm
    #[inline]
    pub fn norm(&self) -> f32 {
        self.norm_squared().sqrt()
    }

    /// Compute dot product with another vector
    ///
    /// SIMD-optimized: Uses chunks_exact for proper auto-vectorization.
    #[inline]
    pub fn dot(&self, other: &Self) -> f32 {
        debug_assert_eq!(self.dim, other.dim, "Vector dimensions must match");

        // Process 4 elements at a time for auto-vectorization
        let chunks_a = self.data.chunks_exact(4);
        let chunks_b = other.data.chunks_exact(4);
        let remainder_a = chunks_a.remainder();
        let remainder_b = chunks_b.remainder();

        let mut acc = [0.0f32; 4];
        for (ca, cb) in chunks_a.zip(chunks_b) {
            acc[0] += ca[0] * cb[0];
            acc[1] += ca[1] * cb[1];
            acc[2] += ca[2] * cb[2];
            acc[3] += ca[3] * cb[3];
        }

        let mut sum = acc[0] + acc[1] + acc[2] + acc[3];
        for (&a, &b) in remainder_a.iter().zip(remainder_b.iter()) {
            sum += a * b;
        }
        sum
    }

    /// Subtract another vector (for residual calculation)
    ///
    /// SIMD-optimized: Processes elements in order for vectorization.
    #[inline]
    pub fn subtract(&self, other: &Self) -> Self {
        debug_assert_eq!(self.dim, other.dim, "Vector dimensions must match");

        let data: Vec<f32> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(&a, &b)| a - b)
            .collect();

        Self {
            data,
            dim: self.dim,
        }
    }

    /// Add another vector
    #[inline]
    pub fn add(&self, other: &Self) -> Self {
        debug_assert_eq!(self.dim, other.dim, "Vector dimensions must match");

        let data: Vec<f32> = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(&a, &b)| a + b)
            .collect();

        Self {
            data,
            dim: self.dim,
        }
    }

    /// Scale the vector
    #[inline]
    pub fn scale(&self, factor: f32) -> Self {
        let data: Vec<f32> = self.data.iter().map(|&x| x * factor).collect();
        Self {
            data,
            dim: self.dim,
        }
    }

    /// Update the vector in place (for incremental updates)
    #[inline]
    pub fn update(&mut self, new_data: &[f32]) {
        debug_assert_eq!(new_data.len(), self.dim, "Update must match dimension");
        self.data.copy_from_slice(new_data);
    }

    /// Compute hash for fingerprinting (using Blake3 would be better but keep it simple)
    pub fn content_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for &x in &self.data {
            x.to_bits().hash(&mut hasher);
        }
        hasher.finish()
    }
}

impl From<Vec<f32>> for StateVector {
    fn from(data: Vec<f32>) -> Self {
        Self::new(data)
    }
}

impl From<&[f32]> for StateVector {
    fn from(data: &[f32]) -> Self {
        Self::new(data.to_vec())
    }
}

impl AsRef<[f32]> for StateVector {
    fn as_ref(&self) -> &[f32] {
        &self.data
    }
}

/// Metadata associated with a node
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeMetadata {
    /// Human-readable label/name
    pub label: Option<String>,
    /// Node type for filtering (e.g., "fact", "hypothesis", "belief")
    pub node_type: Option<String>,
    /// Namespace/scope for multi-tenant isolation
    pub namespace: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Arbitrary key-value properties
    pub properties: HashMap<String, serde_json::Value>,
    /// Source/provenance information
    pub source: Option<String>,
    /// Confidence score (0.0-1.0) if applicable
    pub confidence: Option<f32>,
}

impl NodeMetadata {
    /// Create empty metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Create metadata with a label
    pub fn with_label(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            ..Default::default()
        }
    }

    /// Check if node belongs to a namespace
    pub fn in_namespace(&self, namespace: &str) -> bool {
        self.namespace.as_deref() == Some(namespace)
    }

    /// Check if node has a specific tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

/// A node in the sheaf graph carrying a fixed-dimensional state vector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafNode {
    /// Unique node identifier
    pub id: NodeId,
    /// Fixed-dimensional state vector (stalk of the sheaf)
    pub state: StateVector,
    /// Metadata for filtering and governance
    pub metadata: NodeMetadata,
    /// Timestamp of creation
    pub created_at: DateTime<Utc>,
    /// Timestamp of last state update
    pub updated_at: DateTime<Utc>,
    /// Version counter for optimistic concurrency
    pub version: u64,
}

impl SheafNode {
    /// Create a new sheaf node with the given state vector
    pub fn new(state: StateVector) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            state,
            metadata: NodeMetadata::default(),
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }

    /// Create a new node with a specific ID
    pub fn with_id(id: NodeId, state: StateVector) -> Self {
        let now = Utc::now();
        Self {
            id,
            state,
            metadata: NodeMetadata::default(),
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }

    /// Get the dimension of the node's state vector
    #[inline]
    pub fn dim(&self) -> usize {
        self.state.dim()
    }

    /// Update the state vector
    ///
    /// Increments version and updates timestamp.
    pub fn update_state(&mut self, new_state: StateVector) {
        debug_assert_eq!(
            new_state.dim(),
            self.state.dim(),
            "State dimension must not change"
        );
        self.state = new_state;
        self.updated_at = Utc::now();
        self.version += 1;
    }

    /// Update the state vector in place from a slice
    pub fn update_state_from_slice(&mut self, data: &[f32]) {
        self.state.update(data);
        self.updated_at = Utc::now();
        self.version += 1;
    }

    /// Compute a content hash for fingerprinting
    pub fn content_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        hasher.write_u64(self.state.content_hash());
        hasher.write_u64(self.version);
        hasher.finish()
    }

    /// Check if node is stale (state hasn't been updated since cutoff)
    pub fn is_stale(&self, cutoff: DateTime<Utc>) -> bool {
        self.updated_at < cutoff
    }
}

/// Builder for constructing SheafNode instances
#[derive(Debug, Default)]
pub struct SheafNodeBuilder {
    id: Option<NodeId>,
    state: Option<StateVector>,
    metadata: NodeMetadata,
}

impl SheafNodeBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the node ID
    pub fn id(mut self, id: NodeId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the state vector
    pub fn state(mut self, state: impl Into<StateVector>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Set the state from a slice
    pub fn state_from_slice(mut self, data: &[f32]) -> Self {
        self.state = Some(StateVector::new(data.to_vec()));
        self
    }

    /// Set a zero state of given dimension
    pub fn zero_state(mut self, dim: usize) -> Self {
        self.state = Some(StateVector::zeros(dim));
        self
    }

    /// Set a random unit state of given dimension
    pub fn random_state(mut self, dim: usize) -> Self {
        self.state = Some(StateVector::random_unit(dim));
        self
    }

    /// Set the label
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.metadata.label = Some(label.into());
        self
    }

    /// Set the node type
    pub fn node_type(mut self, node_type: impl Into<String>) -> Self {
        self.metadata.node_type = Some(node_type.into());
        self
    }

    /// Set the namespace
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.metadata.namespace = Some(namespace.into());
        self
    }

    /// Add a tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.metadata.tags.push(tag.into());
        self
    }

    /// Add multiple tags
    pub fn tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        for tag in tags {
            self.metadata.tags.push(tag.into());
        }
        self
    }

    /// Set a property
    pub fn property(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.metadata.properties.insert(key.into(), value.into());
        self
    }

    /// Set the source
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.metadata.source = Some(source.into());
        self
    }

    /// Set the confidence
    pub fn confidence(mut self, confidence: f32) -> Self {
        self.metadata.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// Build the node
    ///
    /// # Panics
    ///
    /// Panics if no state vector was provided.
    pub fn build(self) -> SheafNode {
        let state = self.state.expect("State vector is required");
        let now = Utc::now();

        SheafNode {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            state,
            metadata: self.metadata,
            created_at: now,
            updated_at: now,
            version: 1,
        }
    }

    /// Try to build the node, returning an error if state is missing
    pub fn try_build(self) -> Result<SheafNode, &'static str> {
        let state = self.state.ok_or("State vector is required")?;
        let now = Utc::now();

        Ok(SheafNode {
            id: self.id.unwrap_or_else(Uuid::new_v4),
            state,
            metadata: self.metadata,
            created_at: now,
            updated_at: now,
            version: 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_vector_creation() {
        let v = StateVector::new(vec![1.0, 2.0, 3.0]);
        assert_eq!(v.dim(), 3);
        assert_eq!(v.as_slice(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_state_vector_zeros() {
        let v = StateVector::zeros(5);
        assert_eq!(v.dim(), 5);
        assert!(v.as_slice().iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_state_vector_norm() {
        let v = StateVector::new(vec![3.0, 4.0]);
        assert!((v.norm() - 5.0).abs() < 1e-6);
        assert!((v.norm_squared() - 25.0).abs() < 1e-6);
    }

    #[test]
    fn test_state_vector_dot() {
        let a = StateVector::new(vec![1.0, 2.0, 3.0]);
        let b = StateVector::new(vec![4.0, 5.0, 6.0]);
        assert!((a.dot(&b) - 32.0).abs() < 1e-6);
    }

    #[test]
    fn test_state_vector_subtract() {
        let a = StateVector::new(vec![5.0, 10.0]);
        let b = StateVector::new(vec![2.0, 3.0]);
        let c = a.subtract(&b);
        assert_eq!(c.as_slice(), &[3.0, 7.0]);
    }

    #[test]
    fn test_state_vector_scale() {
        let v = StateVector::new(vec![1.0, 2.0, 3.0]);
        let scaled = v.scale(2.0);
        assert_eq!(scaled.as_slice(), &[2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_node_builder() {
        let node = SheafNodeBuilder::new()
            .state_from_slice(&[1.0, 2.0, 3.0])
            .label("test_node")
            .node_type("fact")
            .namespace("test")
            .tag("important")
            .confidence(0.95)
            .build();

        assert_eq!(node.dim(), 3);
        assert_eq!(node.metadata.label, Some("test_node".to_string()));
        assert_eq!(node.metadata.node_type, Some("fact".to_string()));
        assert_eq!(node.metadata.namespace, Some("test".to_string()));
        assert!(node.metadata.has_tag("important"));
        assert_eq!(node.metadata.confidence, Some(0.95));
    }

    #[test]
    fn test_node_update_state() {
        let mut node = SheafNode::new(StateVector::new(vec![1.0, 2.0]));
        let old_version = node.version;
        let old_updated = node.updated_at;

        std::thread::sleep(std::time::Duration::from_millis(1));
        node.update_state(StateVector::new(vec![3.0, 4.0]));

        assert_eq!(node.version, old_version + 1);
        assert!(node.updated_at > old_updated);
        assert_eq!(node.state.as_slice(), &[3.0, 4.0]);
    }

    #[test]
    fn test_node_content_hash() {
        let node1 = SheafNodeBuilder::new()
            .id(Uuid::new_v4())
            .state_from_slice(&[1.0, 2.0])
            .build();

        let node2 = SheafNodeBuilder::new()
            .id(node1.id)
            .state_from_slice(&[1.0, 2.0])
            .build();

        // Same content should produce same hash (version may differ slightly)
        // This is a simple check - in practice we'd use a proper content hash
        assert_eq!(node1.state.content_hash(), node2.state.content_hash());
    }

    #[test]
    fn test_random_unit_vector() {
        let v = StateVector::random_unit(100);
        assert_eq!(v.dim(), 100);
        // Should be approximately unit length
        assert!((v.norm() - 1.0).abs() < 0.01);
    }
}
