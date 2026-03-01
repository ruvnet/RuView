//! Repository Traits for Governance Persistence
//!
//! Defines the interface for persisting governance objects:
//! - Policy bundles
//! - Witness records
//! - Lineage records
//!
//! # Design Principles
//!
//! 1. **Async-First**: All operations are async for I/O-bound persistence
//! 2. **Separation of Concerns**: Each governance object has its own repository
//! 3. **Consistency**: Supports transactional semantics where needed
//! 4. **Flexibility**: Can be backed by different storage systems
//!
//! # Implementations
//!
//! The traits in this module can be implemented for various backends:
//! - In-memory (for testing)
//! - PostgreSQL (for production)
//! - SQLite (for embedded)
//! - Hybrid (PostgreSQL + ruvector)

use super::{
    EntityRef, GovernanceError, Hash, LineageError, LineageId, LineageRecord, Operation,
    PolicyBundle, PolicyBundleId, PolicyBundleStatus, PolicyError, Timestamp, WitnessError,
    WitnessId, WitnessRecord,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Result type for repository operations
pub type RepositoryResult<T> = Result<T, GovernanceError>;

/// Query options for listing/searching
#[derive(Clone, Debug, Default)]
pub struct QueryOptions {
    /// Maximum number of results
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
    /// Sort order (true = ascending)
    pub ascending: bool,
}

impl QueryOptions {
    /// Create with limit
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Create with offset
    #[must_use]
    pub const fn with_offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Set sort order to descending
    #[must_use]
    pub const fn descending(mut self) -> Self {
        self.ascending = false;
        self
    }
}

/// Time range filter
#[derive(Clone, Debug)]
pub struct TimeRange {
    /// Start of range (inclusive)
    pub start: Timestamp,
    /// End of range (exclusive)
    pub end: Timestamp,
}

impl TimeRange {
    /// Create a new time range
    #[must_use]
    pub const fn new(start: Timestamp, end: Timestamp) -> Self {
        Self { start, end }
    }

    /// Check if a timestamp is within this range
    #[must_use]
    pub fn contains(&self, ts: Timestamp) -> bool {
        ts >= self.start && ts < self.end
    }
}

// ============================================================================
// Policy Repository
// ============================================================================

/// Repository trait for policy bundles
pub trait PolicyRepository: Send + Sync {
    /// Save a policy bundle
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - A bundle with this ID already exists
    /// - Storage operation fails
    fn save(&self, bundle: &PolicyBundle) -> RepositoryResult<()>;

    /// Get a policy bundle by ID
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get(&self, id: PolicyBundleId) -> RepositoryResult<Option<PolicyBundle>>;

    /// Update an existing policy bundle
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Bundle doesn't exist
    /// - Bundle is in immutable state (Active)
    /// - Storage operation fails
    fn update(&self, bundle: &PolicyBundle) -> RepositoryResult<()>;

    /// Delete a policy bundle (only if in Draft status)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Bundle is not in Draft status
    /// - Storage operation fails
    fn delete(&self, id: PolicyBundleId) -> RepositoryResult<()>;

    /// List all policy bundles with optional filtering
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn list(
        &self,
        status: Option<PolicyBundleStatus>,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<PolicyBundle>>;

    /// Get the currently active policy bundle (there should be at most one)
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_active(&self) -> RepositoryResult<Option<PolicyBundle>>;

    /// Find policy bundles by name pattern
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn find_by_name(
        &self,
        pattern: &str,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<PolicyBundle>>;

    /// Get policy bundle history (all versions)
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_history(&self, name: &str) -> RepositoryResult<Vec<PolicyBundle>>;

    /// Check if a policy bundle exists
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn exists(&self, id: PolicyBundleId) -> RepositoryResult<bool>;
}

// ============================================================================
// Witness Repository
// ============================================================================

/// Repository trait for witness records
pub trait WitnessRepository: Send + Sync {
    /// Save a witness record
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - A witness with this ID already exists
    /// - Chain integrity violation (previous witness doesn't exist)
    /// - Storage operation fails
    fn save(&self, witness: &WitnessRecord) -> RepositoryResult<()>;

    /// Get a witness record by ID
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get(&self, id: WitnessId) -> RepositoryResult<Option<WitnessRecord>>;

    /// Get the most recent witness (head of chain)
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_head(&self) -> RepositoryResult<Option<WitnessRecord>>;

    /// Get witness by sequence number
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_by_sequence(&self, sequence: u64) -> RepositoryResult<Option<WitnessRecord>>;

    /// Get witnesses in a sequence range
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_range(&self, start_seq: u64, end_seq: u64) -> RepositoryResult<Vec<WitnessRecord>>;

    /// Get witnesses in a time range
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_by_time_range(
        &self,
        range: TimeRange,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<WitnessRecord>>;

    /// Get witnesses for a specific action hash
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_by_action(&self, action_hash: Hash) -> RepositoryResult<Vec<WitnessRecord>>;

    /// Get witnesses by policy bundle
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_by_policy(
        &self,
        policy_id: PolicyBundleId,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<WitnessRecord>>;

    /// Get witnesses that resulted in denial
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_denials(&self, options: QueryOptions) -> RepositoryResult<Vec<WitnessRecord>>;

    /// Get witnesses for a correlation ID
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_by_correlation(&self, correlation_id: &str) -> RepositoryResult<Vec<WitnessRecord>>;

    /// Count total witnesses
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn count(&self) -> RepositoryResult<u64>;

    /// Verify chain integrity from a starting point
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Chain has integrity violations
    /// - Storage operation fails
    fn verify_chain(&self, from_sequence: u64) -> RepositoryResult<bool>;
}

// ============================================================================
// Lineage Repository
// ============================================================================

/// Repository trait for lineage records
pub trait LineageRepository: Send + Sync {
    /// Save a lineage record
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - A lineage with this ID already exists
    /// - Dependency validation fails
    /// - Storage operation fails
    fn save(&self, lineage: &LineageRecord) -> RepositoryResult<()>;

    /// Get a lineage record by ID
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get(&self, id: LineageId) -> RepositoryResult<Option<LineageRecord>>;

    /// Get all lineage records for an entity
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_for_entity(
        &self,
        entity_ref: &EntityRef,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<LineageRecord>>;

    /// Get the most recent lineage for an entity
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_latest_for_entity(
        &self,
        entity_ref: &EntityRef,
    ) -> RepositoryResult<Option<LineageRecord>>;

    /// Get lineage records by actor
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_by_actor(
        &self,
        actor: &str,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<LineageRecord>>;

    /// Get lineage records by operation type
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_by_operation(
        &self,
        operation: Operation,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<LineageRecord>>;

    /// Get lineage records by authorizing witness
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_by_witness(&self, witness_id: WitnessId) -> RepositoryResult<Vec<LineageRecord>>;

    /// Get lineage records in a time range
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_by_time_range(
        &self,
        range: TimeRange,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<LineageRecord>>;

    /// Get all dependencies of a lineage record (recursive)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Circular dependency detected
    /// - Storage operation fails
    fn get_all_dependencies(&self, id: LineageId) -> RepositoryResult<Vec<LineageRecord>>;

    /// Get all lineage records that depend on a specific record
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn get_dependents(&self, id: LineageId) -> RepositoryResult<Vec<LineageRecord>>;

    /// Count total lineage records
    ///
    /// # Errors
    ///
    /// Returns error if storage operation fails
    fn count(&self) -> RepositoryResult<u64>;

    /// Verify no circular dependencies exist
    ///
    /// # Errors
    ///
    /// Returns error if circular dependency detected
    fn verify_no_cycles(&self) -> RepositoryResult<bool>;
}

// ============================================================================
// In-Memory Implementation (for testing)
// ============================================================================

/// In-memory policy repository for testing
#[derive(Default)]
pub struct InMemoryPolicyRepository {
    bundles: parking_lot::RwLock<HashMap<PolicyBundleId, PolicyBundle>>,
}

impl InMemoryPolicyRepository {
    /// Create a new in-memory repository
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl PolicyRepository for InMemoryPolicyRepository {
    fn save(&self, bundle: &PolicyBundle) -> RepositoryResult<()> {
        let mut bundles = self.bundles.write();
        if bundles.contains_key(&bundle.id) {
            return Err(GovernanceError::Policy(PolicyError::AlreadyExists(
                bundle.id,
            )));
        }
        bundles.insert(bundle.id, bundle.clone());
        Ok(())
    }

    fn get(&self, id: PolicyBundleId) -> RepositoryResult<Option<PolicyBundle>> {
        Ok(self.bundles.read().get(&id).cloned())
    }

    fn update(&self, bundle: &PolicyBundle) -> RepositoryResult<()> {
        let mut bundles = self.bundles.write();
        if !bundles.contains_key(&bundle.id) {
            return Err(GovernanceError::Policy(PolicyError::ScopeNotFound(
                bundle.id.to_string(),
            )));
        }
        bundles.insert(bundle.id, bundle.clone());
        Ok(())
    }

    fn delete(&self, id: PolicyBundleId) -> RepositoryResult<()> {
        let mut bundles = self.bundles.write();
        if let Some(bundle) = bundles.get(&id) {
            if bundle.status != PolicyBundleStatus::Draft {
                return Err(GovernanceError::Policy(PolicyError::NotEditable(
                    bundle.status,
                )));
            }
        }
        bundles.remove(&id);
        Ok(())
    }

    fn list(
        &self,
        status: Option<PolicyBundleStatus>,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<PolicyBundle>> {
        let bundles = self.bundles.read();
        let mut result: Vec<_> = bundles
            .values()
            .filter(|b| status.map_or(true, |s| b.status == s))
            .cloned()
            .collect();

        result.sort_by(|a, b| {
            if options.ascending {
                a.created_at.cmp(&b.created_at)
            } else {
                b.created_at.cmp(&a.created_at)
            }
        });

        if let Some(offset) = options.offset {
            result = result.into_iter().skip(offset).collect();
        }
        if let Some(limit) = options.limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    fn get_active(&self) -> RepositoryResult<Option<PolicyBundle>> {
        Ok(self
            .bundles
            .read()
            .values()
            .find(|b| b.status == PolicyBundleStatus::Active)
            .cloned())
    }

    fn find_by_name(
        &self,
        pattern: &str,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<PolicyBundle>> {
        let bundles = self.bundles.read();
        let mut result: Vec<_> = bundles
            .values()
            .filter(|b| b.name.contains(pattern))
            .cloned()
            .collect();

        if let Some(limit) = options.limit {
            result.truncate(limit);
        }

        Ok(result)
    }

    fn get_history(&self, name: &str) -> RepositoryResult<Vec<PolicyBundle>> {
        let bundles = self.bundles.read();
        let mut result: Vec<_> = bundles
            .values()
            .filter(|b| b.name == name)
            .cloned()
            .collect();
        result.sort_by(|a, b| a.version.cmp(&b.version));
        Ok(result)
    }

    fn exists(&self, id: PolicyBundleId) -> RepositoryResult<bool> {
        Ok(self.bundles.read().contains_key(&id))
    }
}

/// In-memory witness repository for testing
#[derive(Default)]
pub struct InMemoryWitnessRepository {
    witnesses: parking_lot::RwLock<HashMap<WitnessId, WitnessRecord>>,
    by_sequence: parking_lot::RwLock<HashMap<u64, WitnessId>>,
}

impl InMemoryWitnessRepository {
    /// Create a new in-memory repository
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl WitnessRepository for InMemoryWitnessRepository {
    fn save(&self, witness: &WitnessRecord) -> RepositoryResult<()> {
        let mut witnesses = self.witnesses.write();
        let mut by_sequence = self.by_sequence.write();

        if witnesses.contains_key(&witness.id) {
            return Err(GovernanceError::Witness(WitnessError::AlreadyExists(
                witness.id,
            )));
        }

        // Verify chain integrity
        if let Some(prev_id) = witness.previous_witness {
            if !witnesses.contains_key(&prev_id) {
                return Err(GovernanceError::Witness(WitnessError::ChainError(
                    super::WitnessChainError::PreviousNotFound(prev_id),
                )));
            }
        }

        witnesses.insert(witness.id, witness.clone());
        by_sequence.insert(witness.sequence, witness.id);
        Ok(())
    }

    fn get(&self, id: WitnessId) -> RepositoryResult<Option<WitnessRecord>> {
        Ok(self.witnesses.read().get(&id).cloned())
    }

    fn get_head(&self) -> RepositoryResult<Option<WitnessRecord>> {
        let by_sequence = self.by_sequence.read();
        let witnesses = self.witnesses.read();

        if let Some(max_seq) = by_sequence.keys().max() {
            if let Some(id) = by_sequence.get(max_seq) {
                return Ok(witnesses.get(id).cloned());
            }
        }
        Ok(None)
    }

    fn get_by_sequence(&self, sequence: u64) -> RepositoryResult<Option<WitnessRecord>> {
        let by_sequence = self.by_sequence.read();
        let witnesses = self.witnesses.read();

        if let Some(id) = by_sequence.get(&sequence) {
            return Ok(witnesses.get(id).cloned());
        }
        Ok(None)
    }

    fn get_range(&self, start_seq: u64, end_seq: u64) -> RepositoryResult<Vec<WitnessRecord>> {
        let by_sequence = self.by_sequence.read();
        let witnesses = self.witnesses.read();

        let mut result = Vec::new();
        for seq in start_seq..=end_seq {
            if let Some(id) = by_sequence.get(&seq) {
                if let Some(w) = witnesses.get(id) {
                    result.push(w.clone());
                }
            }
        }
        Ok(result)
    }

    fn get_by_time_range(
        &self,
        range: TimeRange,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<WitnessRecord>> {
        let witnesses = self.witnesses.read();
        let mut result: Vec<_> = witnesses
            .values()
            .filter(|w| range.contains(w.timestamp))
            .cloned()
            .collect();

        result.sort_by(|a, b| a.sequence.cmp(&b.sequence));
        if let Some(limit) = options.limit {
            result.truncate(limit);
        }
        Ok(result)
    }

    fn get_by_action(&self, action_hash: Hash) -> RepositoryResult<Vec<WitnessRecord>> {
        let witnesses = self.witnesses.read();
        Ok(witnesses
            .values()
            .filter(|w| w.action_hash == action_hash)
            .cloned()
            .collect())
    }

    fn get_by_policy(
        &self,
        policy_id: PolicyBundleId,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<WitnessRecord>> {
        let witnesses = self.witnesses.read();
        let mut result: Vec<_> = witnesses
            .values()
            .filter(|w| w.policy_bundle_ref.id == policy_id)
            .cloned()
            .collect();

        if let Some(limit) = options.limit {
            result.truncate(limit);
        }
        Ok(result)
    }

    fn get_denials(&self, options: QueryOptions) -> RepositoryResult<Vec<WitnessRecord>> {
        let witnesses = self.witnesses.read();
        let mut result: Vec<_> = witnesses
            .values()
            .filter(|w| !w.decision.allow)
            .cloned()
            .collect();

        if let Some(limit) = options.limit {
            result.truncate(limit);
        }
        Ok(result)
    }

    fn get_by_correlation(&self, correlation_id: &str) -> RepositoryResult<Vec<WitnessRecord>> {
        let witnesses = self.witnesses.read();
        Ok(witnesses
            .values()
            .filter(|w| w.correlation_id.as_deref() == Some(correlation_id))
            .cloned()
            .collect())
    }

    fn count(&self) -> RepositoryResult<u64> {
        Ok(self.witnesses.read().len() as u64)
    }

    fn verify_chain(&self, from_sequence: u64) -> RepositoryResult<bool> {
        let witnesses = self.witnesses.read();
        let by_sequence = self.by_sequence.read();

        let max_seq = by_sequence.keys().max().copied().unwrap_or(0);

        for seq in from_sequence..=max_seq {
            let Some(id) = by_sequence.get(&seq) else {
                return Ok(false); // Gap in sequence
            };
            let Some(witness) = witnesses.get(id) else {
                return Ok(false);
            };

            if !witness.verify_content_hash() {
                return Ok(false);
            }

            if seq > from_sequence {
                if let Some(prev_id) = witness.previous_witness {
                    if let Some(prev) = witnesses.get(&prev_id) {
                        if witness.verify_chain_link(prev).is_err() {
                            return Ok(false);
                        }
                    } else {
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }
}

/// In-memory lineage repository for testing
#[derive(Default)]
pub struct InMemoryLineageRepository {
    lineages: parking_lot::RwLock<HashMap<LineageId, LineageRecord>>,
    by_entity: parking_lot::RwLock<HashMap<String, Vec<LineageId>>>,
}

impl InMemoryLineageRepository {
    /// Create a new in-memory repository
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl LineageRepository for InMemoryLineageRepository {
    fn save(&self, lineage: &LineageRecord) -> RepositoryResult<()> {
        let mut lineages = self.lineages.write();
        let mut by_entity = self.by_entity.write();

        if lineages.contains_key(&lineage.id) {
            return Err(GovernanceError::Lineage(LineageError::AlreadyExists(
                lineage.id,
            )));
        }

        // Verify dependencies exist
        for dep_id in &lineage.dependencies {
            if !lineages.contains_key(dep_id) {
                return Err(GovernanceError::Lineage(LineageError::DependencyNotFound(
                    *dep_id,
                )));
            }
        }

        lineages.insert(lineage.id, lineage.clone());

        let entity_key = lineage.entity_ref.canonical();
        by_entity.entry(entity_key).or_default().push(lineage.id);

        Ok(())
    }

    fn get(&self, id: LineageId) -> RepositoryResult<Option<LineageRecord>> {
        Ok(self.lineages.read().get(&id).cloned())
    }

    fn get_for_entity(
        &self,
        entity_ref: &EntityRef,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<LineageRecord>> {
        let lineages = self.lineages.read();
        let by_entity = self.by_entity.read();

        let entity_key = entity_ref.canonical();
        let mut result: Vec<_> = by_entity
            .get(&entity_key)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| lineages.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default();

        result.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        if let Some(limit) = options.limit {
            result.truncate(limit);
        }
        Ok(result)
    }

    fn get_latest_for_entity(
        &self,
        entity_ref: &EntityRef,
    ) -> RepositoryResult<Option<LineageRecord>> {
        let lineages = self.lineages.read();
        let by_entity = self.by_entity.read();

        let entity_key = entity_ref.canonical();
        Ok(by_entity.get(&entity_key).and_then(|ids| {
            ids.iter()
                .filter_map(|id| lineages.get(id))
                .max_by_key(|l| l.timestamp)
                .cloned()
        }))
    }

    fn get_by_actor(
        &self,
        actor: &str,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<LineageRecord>> {
        let lineages = self.lineages.read();
        let mut result: Vec<_> = lineages
            .values()
            .filter(|l| l.actor == actor)
            .cloned()
            .collect();

        if let Some(limit) = options.limit {
            result.truncate(limit);
        }
        Ok(result)
    }

    fn get_by_operation(
        &self,
        operation: Operation,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<LineageRecord>> {
        let lineages = self.lineages.read();
        let mut result: Vec<_> = lineages
            .values()
            .filter(|l| l.operation == operation)
            .cloned()
            .collect();

        if let Some(limit) = options.limit {
            result.truncate(limit);
        }
        Ok(result)
    }

    fn get_by_witness(&self, witness_id: WitnessId) -> RepositoryResult<Vec<LineageRecord>> {
        let lineages = self.lineages.read();
        Ok(lineages
            .values()
            .filter(|l| l.authorizing_witness == witness_id)
            .cloned()
            .collect())
    }

    fn get_by_time_range(
        &self,
        range: TimeRange,
        options: QueryOptions,
    ) -> RepositoryResult<Vec<LineageRecord>> {
        let lineages = self.lineages.read();
        let mut result: Vec<_> = lineages
            .values()
            .filter(|l| range.contains(l.timestamp))
            .cloned()
            .collect();

        result.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        if let Some(limit) = options.limit {
            result.truncate(limit);
        }
        Ok(result)
    }

    fn get_all_dependencies(&self, id: LineageId) -> RepositoryResult<Vec<LineageRecord>> {
        let lineages = self.lineages.read();
        let mut visited = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut stack = vec![id];

        while let Some(current_id) = stack.pop() {
            if !visited.insert(current_id) {
                continue;
            }

            if let Some(lineage) = lineages.get(&current_id) {
                if current_id != id {
                    result.push(lineage.clone());
                }
                for dep_id in &lineage.dependencies {
                    if !visited.contains(dep_id) {
                        stack.push(*dep_id);
                    }
                }
            }
        }

        Ok(result)
    }

    fn get_dependents(&self, id: LineageId) -> RepositoryResult<Vec<LineageRecord>> {
        let lineages = self.lineages.read();
        Ok(lineages
            .values()
            .filter(|l| l.dependencies.contains(&id))
            .cloned()
            .collect())
    }

    fn count(&self) -> RepositoryResult<u64> {
        Ok(self.lineages.read().len() as u64)
    }

    fn verify_no_cycles(&self) -> RepositoryResult<bool> {
        let lineages = self.lineages.read();

        // Kahn's algorithm for cycle detection
        let mut in_degree: HashMap<LineageId, usize> = HashMap::new();
        let mut graph: HashMap<LineageId, Vec<LineageId>> = HashMap::new();

        for (id, lineage) in lineages.iter() {
            in_degree.entry(*id).or_insert(0);
            for dep_id in &lineage.dependencies {
                graph.entry(*dep_id).or_default().push(*id);
                *in_degree.entry(*id).or_insert(0) += 1;
            }
        }

        let mut queue: Vec<_> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| *id)
            .collect();

        let mut visited = 0;

        while let Some(id) = queue.pop() {
            visited += 1;
            if let Some(dependents) = graph.get(&id) {
                for dep_id in dependents {
                    if let Some(deg) = in_degree.get_mut(dep_id) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(*dep_id);
                        }
                    }
                }
            }
        }

        Ok(visited == lineages.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::{
        EnergySnapshot, GateDecision, PolicyBundleRef, ThresholdConfig,
        WitnessComputeLane as ComputeLane,
    };

    fn test_policy() -> PolicyBundle {
        let mut policy = PolicyBundle::new("test-policy");
        let _ = policy.add_threshold("default", ThresholdConfig::default());
        policy
    }

    fn test_witness(policy_ref: PolicyBundleRef, prev: Option<&WitnessRecord>) -> WitnessRecord {
        WitnessRecord::new(
            Hash::from_bytes([1u8; 32]),
            EnergySnapshot::new(0.5, 0.3, "test"),
            GateDecision::allow(ComputeLane::Reflex),
            policy_ref,
            prev,
        )
    }

    fn test_lineage(witness_id: WitnessId, deps: Vec<LineageId>) -> LineageRecord {
        LineageRecord::new(
            EntityRef::node("test-node"),
            Operation::Create,
            deps,
            witness_id,
            "test-actor",
        )
    }

    #[test]
    fn test_policy_repository() -> RepositoryResult<()> {
        let repo = InMemoryPolicyRepository::new();

        let policy = test_policy();
        repo.save(&policy)?;

        let retrieved = repo.get(policy.id)?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-policy");

        assert!(repo.exists(policy.id)?);

        Ok(())
    }

    #[test]
    fn test_witness_repository_chain() -> RepositoryResult<()> {
        let repo = InMemoryWitnessRepository::new();
        let policy_ref = test_policy().reference();

        // Genesis witness
        let genesis = test_witness(policy_ref.clone(), None);
        repo.save(&genesis)?;

        // Chain another witness
        let second = test_witness(policy_ref, Some(&genesis));
        repo.save(&second)?;

        assert_eq!(repo.count()?, 2);

        let head = repo.get_head()?;
        assert!(head.is_some());
        assert_eq!(head.unwrap().sequence, 1);

        assert!(repo.verify_chain(0)?);

        Ok(())
    }

    #[test]
    fn test_lineage_repository_dependencies() -> RepositoryResult<()> {
        let repo = InMemoryLineageRepository::new();
        let witness_id = super::super::WitnessId::new();

        // Create root lineage
        let root = test_lineage(witness_id, vec![]);
        repo.save(&root)?;

        // Create dependent lineage
        let dependent = test_lineage(witness_id, vec![root.id]);
        repo.save(&dependent)?;

        // Get dependencies
        let deps = repo.get_all_dependencies(dependent.id)?;
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].id, root.id);

        // Get dependents
        let dependents = repo.get_dependents(root.id)?;
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0].id, dependent.id);

        assert!(repo.verify_no_cycles()?);

        Ok(())
    }

    #[test]
    fn test_query_options() {
        let options = QueryOptions::default()
            .with_limit(10)
            .with_offset(5)
            .descending();

        assert_eq!(options.limit, Some(10));
        assert_eq!(options.offset, Some(5));
        assert!(!options.ascending);
    }
}
