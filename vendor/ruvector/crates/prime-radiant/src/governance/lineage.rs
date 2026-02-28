//! Lineage Record Entity
//!
//! Implements provenance tracking for all authoritative writes.
//!
//! # Core Invariant
//!
//! **No write without lineage**: Every authoritative write MUST have a lineage record
//! that tracks:
//!
//! - What entity was modified
//! - What operation was performed
//! - What witness authorized the write
//! - Who performed the write
//! - What prior lineage records this depends on
//!
//! # Causal Dependencies
//!
//! Lineage records form a directed acyclic graph (DAG) of dependencies:
//!
//! ```text
//!     L1 ─────┐
//!             ├──► L4 ──► L5
//!     L2 ─────┤
//!             └──► L6
//!     L3 ──────────────► L7
//! ```
//!
//! This enables:
//! - Understanding the causal history of any entity
//! - Detecting concurrent writes
//! - Supporting deterministic replay

use super::{Hash, Timestamp, WitnessId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

/// Unique identifier for a lineage record
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LineageId(pub Uuid);

impl LineageId {
    /// Generate a new random ID
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from a UUID
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get as bytes
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Create a nil/sentinel ID
    #[must_use]
    pub const fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Check if this is the nil ID
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for LineageId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LineageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Reference to an entity in the system
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityRef {
    /// Entity type (e.g., "node", "edge", "policy")
    pub entity_type: String,
    /// Entity identifier
    pub entity_id: String,
    /// Optional namespace/scope
    pub namespace: Option<String>,
    /// Version of the entity (if applicable)
    pub version: Option<u64>,
}

impl EntityRef {
    /// Create a new entity reference
    #[must_use]
    pub fn new(entity_type: impl Into<String>, entity_id: impl Into<String>) -> Self {
        Self {
            entity_type: entity_type.into(),
            entity_id: entity_id.into(),
            namespace: None,
            version: None,
        }
    }

    /// Set the namespace
    #[must_use]
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    /// Set the version
    #[must_use]
    pub const fn with_version(mut self, version: u64) -> Self {
        self.version = Some(version);
        self
    }

    /// Create a node reference
    #[must_use]
    pub fn node(id: impl Into<String>) -> Self {
        Self::new("node", id)
    }

    /// Create an edge reference
    #[must_use]
    pub fn edge(id: impl Into<String>) -> Self {
        Self::new("edge", id)
    }

    /// Create a policy reference
    #[must_use]
    pub fn policy(id: impl Into<String>) -> Self {
        Self::new("policy", id)
    }

    /// Get a canonical string representation
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut s = format!("{}:{}", self.entity_type, self.entity_id);
        if let Some(ref ns) = self.namespace {
            s = format!("{ns}/{s}");
        }
        if let Some(v) = self.version {
            s = format!("{s}@{v}");
        }
        s
    }

    /// Compute content hash
    #[must_use]
    pub fn content_hash(&self) -> Hash {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.entity_type.as_bytes());
        hasher.update(self.entity_id.as_bytes());
        if let Some(ref ns) = self.namespace {
            hasher.update(ns.as_bytes());
        }
        if let Some(v) = self.version {
            hasher.update(&v.to_le_bytes());
        }
        Hash::from_blake3(hasher.finalize())
    }
}

impl std::fmt::Display for EntityRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.canonical())
    }
}

/// Type of operation performed on an entity
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Operation {
    /// Create a new entity
    Create,
    /// Update an existing entity
    Update,
    /// Delete an entity
    Delete,
    /// Archive an entity (soft delete)
    Archive,
    /// Restore an archived entity
    Restore,
    /// Merge entities
    Merge,
    /// Split an entity
    Split,
    /// Transfer ownership
    Transfer,
}

impl Operation {
    /// Check if this operation creates a new entity
    #[must_use]
    pub const fn is_create(&self) -> bool {
        matches!(self, Self::Create | Self::Split)
    }

    /// Check if this operation removes an entity
    #[must_use]
    pub const fn is_destructive(&self) -> bool {
        matches!(self, Self::Delete | Self::Archive | Self::Merge)
    }

    /// Check if this operation modifies an entity
    #[must_use]
    pub const fn is_mutation(&self) -> bool {
        matches!(
            self,
            Self::Update | Self::Transfer | Self::Restore | Self::Merge | Self::Split
        )
    }
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create => write!(f, "CREATE"),
            Self::Update => write!(f, "UPDATE"),
            Self::Delete => write!(f, "DELETE"),
            Self::Archive => write!(f, "ARCHIVE"),
            Self::Restore => write!(f, "RESTORE"),
            Self::Merge => write!(f, "MERGE"),
            Self::Split => write!(f, "SPLIT"),
            Self::Transfer => write!(f, "TRANSFER"),
        }
    }
}

/// Lineage-related errors
#[derive(Debug, Error)]
pub enum LineageError {
    /// Missing authorizing witness
    #[error("Missing authorizing witness for lineage {0}")]
    MissingWitness(LineageId),

    /// Dependency not found
    #[error("Dependency not found: {0}")]
    DependencyNotFound(LineageId),

    /// Circular dependency detected
    #[error("Circular dependency detected involving {0}")]
    CircularDependency(LineageId),

    /// Invalid operation for entity state
    #[error("Invalid operation {0} for entity {1}")]
    InvalidOperation(Operation, EntityRef),

    /// Lineage not found
    #[error("Lineage not found: {0}")]
    NotFound(LineageId),

    /// Lineage already exists
    #[error("Lineage already exists: {0}")]
    AlreadyExists(LineageId),

    /// Content hash mismatch
    #[error("Content hash mismatch for lineage {0}")]
    HashMismatch(LineageId),
}

/// Provenance tracking for an authoritative write
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LineageRecord {
    /// Unique lineage identifier
    pub id: LineageId,
    /// Entity that was modified
    pub entity_ref: EntityRef,
    /// Operation performed
    pub operation: Operation,
    /// Causal dependencies (prior lineage records this depends on)
    pub dependencies: Vec<LineageId>,
    /// Witness that authorized this write
    pub authorizing_witness: WitnessId,
    /// Actor who performed the write
    pub actor: String,
    /// Creation timestamp
    pub timestamp: Timestamp,
    /// Content hash for integrity
    pub content_hash: Hash,
    /// Optional description of the change
    pub description: Option<String>,
    /// Optional previous state hash (for updates)
    pub previous_state_hash: Option<Hash>,
    /// Optional new state hash
    pub new_state_hash: Option<Hash>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl LineageRecord {
    /// Create a new lineage record
    #[must_use]
    pub fn new(
        entity_ref: EntityRef,
        operation: Operation,
        dependencies: Vec<LineageId>,
        authorizing_witness: WitnessId,
        actor: impl Into<String>,
    ) -> Self {
        let id = LineageId::new();
        let timestamp = Timestamp::now();

        let mut record = Self {
            id,
            entity_ref,
            operation,
            dependencies,
            authorizing_witness,
            actor: actor.into(),
            timestamp,
            content_hash: Hash::zero(), // Placeholder
            description: None,
            previous_state_hash: None,
            new_state_hash: None,
            metadata: HashMap::new(),
        };

        record.content_hash = record.compute_content_hash();
        record
    }

    /// Create a lineage record for entity creation
    #[must_use]
    pub fn create(
        entity_ref: EntityRef,
        authorizing_witness: WitnessId,
        actor: impl Into<String>,
    ) -> Self {
        Self::new(
            entity_ref,
            Operation::Create,
            Vec::new(),
            authorizing_witness,
            actor,
        )
    }

    /// Create a lineage record for entity update
    #[must_use]
    pub fn update(
        entity_ref: EntityRef,
        dependencies: Vec<LineageId>,
        authorizing_witness: WitnessId,
        actor: impl Into<String>,
    ) -> Self {
        Self::new(
            entity_ref,
            Operation::Update,
            dependencies,
            authorizing_witness,
            actor,
        )
    }

    /// Create a lineage record for entity deletion
    #[must_use]
    pub fn delete(
        entity_ref: EntityRef,
        dependencies: Vec<LineageId>,
        authorizing_witness: WitnessId,
        actor: impl Into<String>,
    ) -> Self {
        Self::new(
            entity_ref,
            Operation::Delete,
            dependencies,
            authorizing_witness,
            actor,
        )
    }

    /// Set description
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self.content_hash = self.compute_content_hash();
        self
    }

    /// Set previous state hash
    #[must_use]
    pub fn with_previous_state(mut self, hash: Hash) -> Self {
        self.previous_state_hash = Some(hash);
        self.content_hash = self.compute_content_hash();
        self
    }

    /// Set new state hash
    #[must_use]
    pub fn with_new_state(mut self, hash: Hash) -> Self {
        self.new_state_hash = Some(hash);
        self.content_hash = self.compute_content_hash();
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self.content_hash = self.compute_content_hash();
        self
    }

    /// Compute the content hash using Blake3
    #[must_use]
    pub fn compute_content_hash(&self) -> Hash {
        let mut hasher = blake3::Hasher::new();

        // Core identifying fields
        hasher.update(self.id.as_bytes());
        hasher.update(self.entity_ref.content_hash().as_bytes());
        hasher.update(&[self.operation as u8]);

        // Dependencies (sorted for determinism)
        let mut deps: Vec<_> = self.dependencies.iter().collect();
        deps.sort_by_key(|d| d.0);
        for dep in deps {
            hasher.update(dep.as_bytes());
        }

        // Authorization
        hasher.update(self.authorizing_witness.as_bytes());
        hasher.update(self.actor.as_bytes());

        // Timestamp
        hasher.update(&self.timestamp.secs.to_le_bytes());
        hasher.update(&self.timestamp.nanos.to_le_bytes());

        // Optional fields
        if let Some(ref desc) = self.description {
            hasher.update(desc.as_bytes());
        }
        if let Some(ref prev) = self.previous_state_hash {
            hasher.update(prev.as_bytes());
        }
        if let Some(ref new) = self.new_state_hash {
            hasher.update(new.as_bytes());
        }

        // Metadata (sorted for determinism)
        let mut meta_keys: Vec<_> = self.metadata.keys().collect();
        meta_keys.sort();
        for key in meta_keys {
            hasher.update(key.as_bytes());
            if let Some(value) = self.metadata.get(key) {
                hasher.update(value.as_bytes());
            }
        }

        Hash::from_blake3(hasher.finalize())
    }

    /// Verify the content hash is correct
    #[must_use]
    pub fn verify_content_hash(&self) -> bool {
        self.content_hash == self.compute_content_hash()
    }

    /// Check if this lineage has no dependencies (root lineage)
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.dependencies.is_empty()
    }

    /// Check if this lineage depends on a specific lineage
    #[must_use]
    pub fn depends_on(&self, other: LineageId) -> bool {
        self.dependencies.contains(&other)
    }
}

impl PartialEq for LineageRecord {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for LineageRecord {}

impl std::hash::Hash for LineageRecord {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

/// Builder for lineage records with validation
pub struct LineageBuilder {
    entity_ref: Option<EntityRef>,
    operation: Option<Operation>,
    dependencies: Vec<LineageId>,
    authorizing_witness: Option<WitnessId>,
    actor: Option<String>,
    description: Option<String>,
    previous_state_hash: Option<Hash>,
    new_state_hash: Option<Hash>,
    metadata: HashMap<String, String>,
}

impl LineageBuilder {
    /// Create a new builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            entity_ref: None,
            operation: None,
            dependencies: Vec::new(),
            authorizing_witness: None,
            actor: None,
            description: None,
            previous_state_hash: None,
            new_state_hash: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the entity reference
    #[must_use]
    pub fn entity(mut self, entity_ref: EntityRef) -> Self {
        self.entity_ref = Some(entity_ref);
        self
    }

    /// Set the operation
    #[must_use]
    pub fn operation(mut self, op: Operation) -> Self {
        self.operation = Some(op);
        self
    }

    /// Add a dependency
    #[must_use]
    pub fn depends_on(mut self, dep: LineageId) -> Self {
        self.dependencies.push(dep);
        self
    }

    /// Set all dependencies
    #[must_use]
    pub fn dependencies(mut self, deps: Vec<LineageId>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Set the authorizing witness
    #[must_use]
    pub fn authorized_by(mut self, witness: WitnessId) -> Self {
        self.authorizing_witness = Some(witness);
        self
    }

    /// Set the actor
    #[must_use]
    pub fn actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Set description
    #[must_use]
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set previous state hash
    #[must_use]
    pub fn previous_state(mut self, hash: Hash) -> Self {
        self.previous_state_hash = Some(hash);
        self
    }

    /// Set new state hash
    #[must_use]
    pub fn new_state(mut self, hash: Hash) -> Self {
        self.new_state_hash = Some(hash);
        self
    }

    /// Add metadata
    #[must_use]
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Build the lineage record
    ///
    /// # Errors
    ///
    /// Returns error if required fields are missing
    pub fn build(self) -> Result<LineageRecord, LineageError> {
        let entity_ref = self.entity_ref.ok_or_else(|| {
            LineageError::InvalidOperation(
                self.operation.unwrap_or(Operation::Create),
                EntityRef::new("unknown", "unknown"),
            )
        })?;

        let operation = self.operation.unwrap_or(Operation::Create);

        let authorizing_witness = self
            .authorizing_witness
            .ok_or_else(|| LineageError::MissingWitness(LineageId::nil()))?;

        let actor = self.actor.unwrap_or_else(|| "unknown".to_string());

        let mut record = LineageRecord::new(
            entity_ref,
            operation,
            self.dependencies,
            authorizing_witness,
            actor,
        );

        if let Some(desc) = self.description {
            record = record.with_description(desc);
        }
        if let Some(prev) = self.previous_state_hash {
            record = record.with_previous_state(prev);
        }
        if let Some(new) = self.new_state_hash {
            record = record.with_new_state(new);
        }
        for (key, value) in self.metadata {
            record = record.with_metadata(key, value);
        }

        Ok(record)
    }
}

impl Default for LineageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks lineage for an entity across multiple operations
pub struct EntityLineageTracker {
    /// Entity being tracked
    pub entity_ref: EntityRef,
    /// All lineage records for this entity (ordered by timestamp)
    pub lineage: Vec<LineageRecord>,
    /// Current state hash
    pub current_state_hash: Option<Hash>,
}

impl EntityLineageTracker {
    /// Create a new tracker
    #[must_use]
    pub fn new(entity_ref: EntityRef) -> Self {
        Self {
            entity_ref,
            lineage: Vec::new(),
            current_state_hash: None,
        }
    }

    /// Add a lineage record
    ///
    /// # Errors
    ///
    /// Returns error if the record is for a different entity
    pub fn add(&mut self, record: LineageRecord) -> Result<(), LineageError> {
        if record.entity_ref != self.entity_ref {
            return Err(LineageError::InvalidOperation(
                record.operation,
                self.entity_ref.clone(),
            ));
        }

        // Update current state hash
        if let Some(ref new_hash) = record.new_state_hash {
            self.current_state_hash = Some(*new_hash);
        }

        // Insert in timestamp order
        let pos = self
            .lineage
            .iter()
            .position(|r| r.timestamp > record.timestamp)
            .unwrap_or(self.lineage.len());
        self.lineage.insert(pos, record);

        Ok(())
    }

    /// Get the most recent lineage record
    #[must_use]
    pub fn latest(&self) -> Option<&LineageRecord> {
        self.lineage.last()
    }

    /// Get all dependencies for this entity
    #[must_use]
    pub fn all_dependencies(&self) -> Vec<LineageId> {
        self.lineage
            .iter()
            .flat_map(|r| r.dependencies.iter().copied())
            .collect()
    }

    /// Check if the entity has been deleted
    #[must_use]
    pub fn is_deleted(&self) -> bool {
        self.lineage
            .last()
            .map_or(false, |r| r.operation == Operation::Delete)
    }

    /// Get lineage records by operation type
    #[must_use]
    pub fn by_operation(&self, op: Operation) -> Vec<&LineageRecord> {
        self.lineage.iter().filter(|r| r.operation == op).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_witness_id() -> WitnessId {
        WitnessId::new()
    }

    #[test]
    fn test_entity_ref() {
        let entity = EntityRef::node("node-123")
            .with_namespace("test")
            .with_version(1);

        assert_eq!(entity.entity_type, "node");
        assert_eq!(entity.entity_id, "node-123");
        assert_eq!(entity.namespace, Some("test".to_string()));
        assert_eq!(entity.version, Some(1));
        assert_eq!(entity.canonical(), "test/node:node-123@1");
    }

    #[test]
    fn test_lineage_creation() {
        let entity = EntityRef::node("node-1");
        let witness = test_witness_id();

        let lineage = LineageRecord::create(entity.clone(), witness, "alice");

        assert_eq!(lineage.operation, Operation::Create);
        assert!(lineage.is_root());
        assert!(lineage.verify_content_hash());
    }

    #[test]
    fn test_lineage_with_dependencies() {
        let entity = EntityRef::node("node-1");
        let witness = test_witness_id();

        let dep1 = LineageId::new();
        let dep2 = LineageId::new();

        let lineage = LineageRecord::update(entity, vec![dep1, dep2], witness, "bob");

        assert!(!lineage.is_root());
        assert!(lineage.depends_on(dep1));
        assert!(lineage.depends_on(dep2));
    }

    #[test]
    fn test_lineage_builder() -> Result<(), LineageError> {
        let lineage = LineageBuilder::new()
            .entity(EntityRef::edge("edge-1"))
            .operation(Operation::Update)
            .authorized_by(test_witness_id())
            .actor("charlie")
            .description("Updated edge weight")
            .previous_state(Hash::from_bytes([1u8; 32]))
            .new_state(Hash::from_bytes([2u8; 32]))
            .metadata("reason", "optimization")
            .build()?;

        assert_eq!(lineage.operation, Operation::Update);
        assert!(lineage.description.is_some());
        assert!(lineage.previous_state_hash.is_some());
        assert!(lineage.new_state_hash.is_some());
        assert_eq!(
            lineage.metadata.get("reason"),
            Some(&"optimization".to_string())
        );

        Ok(())
    }

    #[test]
    fn test_entity_lineage_tracker() -> Result<(), LineageError> {
        let entity = EntityRef::node("node-1");
        let witness = test_witness_id();

        let mut tracker = EntityLineageTracker::new(entity.clone());

        // Create
        let create = LineageRecord::create(entity.clone(), witness, "alice")
            .with_new_state(Hash::from_bytes([1u8; 32]));
        tracker.add(create)?;

        // Update
        let update = LineageRecord::update(
            entity.clone(),
            vec![tracker.latest().unwrap().id],
            witness,
            "bob",
        )
        .with_previous_state(Hash::from_bytes([1u8; 32]))
        .with_new_state(Hash::from_bytes([2u8; 32]));
        tracker.add(update)?;

        assert_eq!(tracker.lineage.len(), 2);
        assert_eq!(
            tracker.current_state_hash,
            Some(Hash::from_bytes([2u8; 32]))
        );
        assert!(!tracker.is_deleted());

        Ok(())
    }

    #[test]
    fn test_content_hash_determinism() {
        let entity = EntityRef::node("node-1");
        let witness = test_witness_id();

        let lineage = LineageRecord::create(entity, witness, "alice").with_description("test");

        let hash1 = lineage.compute_content_hash();
        let hash2 = lineage.compute_content_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_tamper_detection() {
        let entity = EntityRef::node("node-1");
        let witness = test_witness_id();

        let mut lineage = LineageRecord::create(entity, witness, "alice");

        // Tamper with the record
        lineage.actor = "mallory".to_string();

        // Hash should no longer match
        assert!(!lineage.verify_content_hash());
    }

    #[test]
    fn test_operation_classification() {
        assert!(Operation::Create.is_create());
        assert!(Operation::Split.is_create());
        assert!(!Operation::Update.is_create());

        assert!(Operation::Delete.is_destructive());
        assert!(Operation::Archive.is_destructive());
        assert!(!Operation::Create.is_destructive());

        assert!(Operation::Update.is_mutation());
        assert!(!Operation::Create.is_mutation());
    }
}
