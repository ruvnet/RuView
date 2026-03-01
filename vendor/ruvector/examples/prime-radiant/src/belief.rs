//! # Topos-Theoretic Belief Model
//!
//! This module implements a belief system using topos theory, where:
//! - Contexts form the objects of the base category
//! - Beliefs are modeled as sheaves over contexts
//! - The internal logic of the topos provides reasoning capabilities
//!
//! ## Key Features
//!
//! - **Contextual beliefs**: Beliefs depend on context
//! - **Belief revision**: Update beliefs while maintaining coherence
//! - **Sheaf-theoretic consistency**: Local beliefs must agree on overlaps

use crate::topos::{Topos, SubobjectClassifier, InternalLogic};
use crate::category::{Category, SetCategory, Object, ObjectData};
use crate::{CategoryError, MorphismId, ObjectId, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// A context for beliefs
///
/// Contexts represent different "worlds" or "situations" where beliefs
/// may have different truth values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Unique identifier
    pub id: ObjectId,
    /// Name of the context
    pub name: String,
    /// Properties of this context
    pub properties: HashMap<String, serde_json::Value>,
    /// Parent context (for context hierarchy)
    pub parent: Option<ObjectId>,
    /// Time of context creation
    pub created_at: u64,
}

impl Context {
    /// Creates a new context
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: ObjectId::new(),
            name: name.into(),
            properties: HashMap::new(),
            parent: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Sets a property
    pub fn with_property(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.properties.insert(key.into(), value);
        self
    }

    /// Sets the parent context
    pub fn with_parent(mut self, parent: ObjectId) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Checks if this context is a subcontext of another
    pub fn is_subcontext_of(&self, other: &ObjectId) -> bool {
        self.parent.as_ref() == Some(other)
    }
}

impl PartialEq for Context {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// A belief state in the topos
///
/// Represents a proposition that may have different truth values
/// in different contexts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeliefState {
    /// Unique identifier
    pub id: ObjectId,
    /// The proposition content
    pub proposition: String,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,
    /// Contexts where this belief holds
    pub holding_contexts: HashSet<ObjectId>,
    /// Contexts where this belief is false
    pub refuting_contexts: HashSet<ObjectId>,
    /// Evidence supporting the belief
    pub evidence: Vec<Evidence>,
    /// Whether this is a derived belief
    pub is_derived: bool,
    /// Timestamp of last update
    pub updated_at: u64,
}

impl BeliefState {
    /// Creates a new belief state
    pub fn new(proposition: impl Into<String>) -> Self {
        Self {
            id: ObjectId::new(),
            proposition: proposition.into(),
            confidence: 0.5,
            holding_contexts: HashSet::new(),
            refuting_contexts: HashSet::new(),
            evidence: Vec::new(),
            is_derived: false,
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Sets the confidence level
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Adds a holding context
    pub fn holds_in(mut self, context: ObjectId) -> Self {
        self.holding_contexts.insert(context);
        self.refuting_contexts.remove(&context);
        self
    }

    /// Adds a refuting context
    pub fn refuted_in(mut self, context: ObjectId) -> Self {
        self.refuting_contexts.insert(context);
        self.holding_contexts.remove(&context);
        self
    }

    /// Adds evidence
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// Gets the truth value in a context
    pub fn truth_in(&self, context: &ObjectId) -> TruthValue {
        if self.holding_contexts.contains(context) {
            TruthValue::True
        } else if self.refuting_contexts.contains(context) {
            TruthValue::False
        } else {
            TruthValue::Unknown
        }
    }

    /// Updates the timestamp
    pub fn touch(&mut self) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

/// Evidence for a belief
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Evidence identifier
    pub id: ObjectId,
    /// Description of the evidence
    pub description: String,
    /// Strength of the evidence (0.0 to 1.0)
    pub strength: f64,
    /// Source of the evidence
    pub source: Option<String>,
    /// Context where this evidence applies
    pub context: Option<ObjectId>,
}

impl Evidence {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            id: ObjectId::new(),
            description: description.into(),
            strength: 0.5,
            source: None,
            context: None,
        }
    }

    pub fn with_strength(mut self, strength: f64) -> Self {
        self.strength = strength.clamp(0.0, 1.0);
        self
    }

    pub fn from_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn in_context(mut self, context: ObjectId) -> Self {
        self.context = Some(context);
        self
    }
}

/// Truth values in the internal logic
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TruthValue {
    /// Definitely true
    True,
    /// Definitely false
    False,
    /// Unknown/uncertain
    Unknown,
    /// Both true and false (contradiction)
    Contradiction,
}

impl TruthValue {
    /// Logical conjunction
    pub fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, Self::True) => Self::True,
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::Contradiction, _) | (_, Self::Contradiction) => Self::Contradiction,
            _ => Self::Unknown,
        }
    }

    /// Logical disjunction
    pub fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::False, Self::False) => Self::False,
            (Self::Contradiction, _) | (_, Self::Contradiction) => Self::Contradiction,
            _ => Self::Unknown,
        }
    }

    /// Logical negation
    pub fn not(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
            Self::Contradiction => Self::Contradiction,
        }
    }

    /// Logical implication
    pub fn implies(self, other: Self) -> Self {
        self.not().or(other)
    }

    /// Checks if this is a definite value
    pub fn is_definite(&self) -> bool {
        matches!(self, Self::True | Self::False)
    }
}

/// A sheaf of beliefs over contexts
///
/// Assigns belief states to contexts in a coherent way,
/// satisfying the sheaf axioms.
pub struct Sheaf<T: Clone> {
    /// Sections: assignments of data to contexts
    sections: Arc<DashMap<ObjectId, T>>,
    /// Restriction maps between contexts
    restrictions: Arc<DashMap<(ObjectId, ObjectId), Box<dyn Fn(&T) -> T + Send + Sync>>>,
}

impl<T: Clone> std::fmt::Debug for Sheaf<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sheaf")
            .field("sections_count", &self.sections.len())
            .field("restrictions_count", &self.restrictions.len())
            .finish()
    }
}

impl<T: Clone> Sheaf<T> {
    /// Creates a new sheaf
    pub fn new() -> Self {
        Self {
            sections: Arc::new(DashMap::new()),
            restrictions: Arc::new(DashMap::new()),
        }
    }

    /// Sets a section over a context
    pub fn set_section(&self, context: ObjectId, data: T) {
        self.sections.insert(context, data);
    }

    /// Gets a section over a context
    pub fn get_section(&self, context: &ObjectId) -> Option<T> {
        self.sections.get(context).map(|entry| entry.clone())
    }

    /// Restricts a section to a subcontext
    pub fn restrict(&self, from: &ObjectId, to: &ObjectId) -> Option<T> {
        let section = self.get_section(from)?;
        if let Some(restrict_fn) = self.restrictions.get(&(*from, *to)) {
            Some(restrict_fn(&section))
        } else {
            // Default: return the same section
            Some(section)
        }
    }

    /// Registers a restriction map
    pub fn register_restriction(
        &self,
        from: ObjectId,
        to: ObjectId,
        restrict_fn: impl Fn(&T) -> T + Send + Sync + 'static,
    ) {
        self.restrictions.insert((from, to), Box::new(restrict_fn));
    }
}

impl<T: Clone> Default for Sheaf<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// The belief topos
///
/// A topos structure for reasoning about beliefs across contexts.
#[derive(Debug)]
pub struct BeliefTopos {
    /// All contexts
    contexts: Arc<DashMap<ObjectId, Context>>,
    /// The belief sheaf
    belief_sheaf: Sheaf<BeliefState>,
    /// Internal logic operations
    internal_logic: InternalLogic,
    /// Context refinement morphisms
    refinements: Arc<DashMap<(ObjectId, ObjectId), MorphismId>>,
    /// Belief revision history
    revision_history: Arc<DashMap<ObjectId, Vec<RevisionEvent>>>,
}

impl BeliefTopos {
    /// Creates a new belief topos
    pub fn new() -> Self {
        Self {
            contexts: Arc::new(DashMap::new()),
            belief_sheaf: Sheaf::new(),
            internal_logic: InternalLogic::new(),
            refinements: Arc::new(DashMap::new()),
            revision_history: Arc::new(DashMap::new()),
        }
    }

    /// Adds a context
    pub fn add_context(&self, context: Context) -> ObjectId {
        let id = context.id;
        self.contexts.insert(id, context);
        id
    }

    /// Gets a context by ID
    pub fn get_context(&self, id: &ObjectId) -> Option<Context> {
        self.contexts.get(id).map(|entry| entry.clone())
    }

    /// Gets all contexts
    pub fn contexts(&self) -> Vec<Context> {
        self.contexts.iter().map(|e| e.value().clone()).collect()
    }

    /// Adds a belief in a context
    pub fn add_belief(&self, context: ObjectId, belief: BeliefState) {
        self.belief_sheaf.set_section(context, belief);
    }

    /// Gets a belief in a context
    pub fn get_belief(&self, context: &ObjectId) -> Option<BeliefState> {
        self.belief_sheaf.get_section(context)
    }

    /// Queries the truth value of a belief in a context
    pub fn query_truth(&self, belief_id: &ObjectId, context: &ObjectId) -> TruthValue {
        if let Some(belief) = self.get_belief(context) {
            if belief.id == *belief_id {
                return belief.truth_in(context);
            }
        }
        TruthValue::Unknown
    }

    /// Revises a belief based on new evidence
    pub fn revise_belief(
        &self,
        belief_id: ObjectId,
        context: ObjectId,
        evidence: Evidence,
    ) -> Result<()> {
        let mut belief = self
            .get_belief(&context)
            .ok_or_else(|| CategoryError::ObjectNotFound(belief_id))?;

        // Update confidence based on evidence
        let old_confidence = belief.confidence;
        let evidence_impact = evidence.strength * 0.5;
        belief.confidence = (belief.confidence + evidence_impact).clamp(0.0, 1.0);
        belief.evidence.push(evidence.clone());
        belief.touch();

        // Record revision
        let event = RevisionEvent {
            belief_id,
            context,
            old_confidence,
            new_confidence: belief.confidence,
            evidence: evidence.id,
            timestamp: belief.updated_at,
        };

        self.revision_history
            .entry(belief_id)
            .or_insert_with(Vec::new)
            .push(event);

        // Update the belief
        self.belief_sheaf.set_section(context, belief);

        Ok(())
    }

    /// Checks consistency of beliefs across contexts
    pub fn check_consistency(&self) -> ConsistencyResult {
        let mut result = ConsistencyResult::new();

        // Check for contradictions within contexts
        for entry in self.contexts.iter() {
            let context_id = *entry.key();
            if let Some(belief) = self.get_belief(&context_id) {
                if belief.holding_contexts.contains(&context_id)
                    && belief.refuting_contexts.contains(&context_id)
                {
                    result.contradictions.push(Contradiction {
                        belief: belief.id,
                        context: context_id,
                        reason: "Belief both holds and is refuted in same context".to_string(),
                    });
                }
            }
        }

        // Check sheaf consistency (beliefs agree on overlaps)
        // Simplified: check parent-child consistency
        for entry in self.contexts.iter() {
            let context = entry.value();
            if let Some(parent_id) = context.parent {
                if let (Some(child_belief), Some(parent_belief)) = (
                    self.get_belief(&context.id),
                    self.get_belief(&parent_id),
                ) {
                    // Child should not contradict parent
                    if child_belief.truth_in(&context.id) != parent_belief.truth_in(&parent_id) {
                        let child_truth = child_belief.truth_in(&context.id);
                        let parent_truth = parent_belief.truth_in(&parent_id);
                        if child_truth.is_definite() && parent_truth.is_definite() {
                            result.sheaf_violations.push(SheafViolation {
                                child_context: context.id,
                                parent_context: parent_id,
                                reason: "Child context contradicts parent".to_string(),
                            });
                        }
                    }
                }
            }
        }

        result.is_consistent = result.contradictions.is_empty()
            && result.sheaf_violations.is_empty();

        result
    }

    /// Performs belief propagation from parent to child contexts
    pub fn propagate_beliefs(&self) {
        for entry in self.contexts.iter() {
            let context = entry.value();
            if let Some(parent_id) = context.parent {
                if let Some(parent_belief) = self.get_belief(&parent_id) {
                    // Propagate to child if child has no belief
                    if self.get_belief(&context.id).is_none() {
                        let child_belief = BeliefState {
                            id: ObjectId::new(),
                            proposition: parent_belief.proposition.clone(),
                            confidence: parent_belief.confidence * 0.9, // Slight degradation
                            holding_contexts: parent_belief.holding_contexts.clone(),
                            refuting_contexts: parent_belief.refuting_contexts.clone(),
                            evidence: vec![],
                            is_derived: true,
                            updated_at: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        };
                        self.belief_sheaf.set_section(context.id, child_belief);
                    }
                }
            }
        }
    }

    /// Gets the internal logic
    pub fn logic(&self) -> &InternalLogic {
        &self.internal_logic
    }

    /// Gets revision history for a belief
    pub fn revision_history(&self, belief_id: &ObjectId) -> Vec<RevisionEvent> {
        self.revision_history
            .get(belief_id)
            .map(|e| e.clone())
            .unwrap_or_default()
    }
}

impl Default for BeliefTopos {
    fn default() -> Self {
        Self::new()
    }
}

/// A belief revision event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionEvent {
    pub belief_id: ObjectId,
    pub context: ObjectId,
    pub old_confidence: f64,
    pub new_confidence: f64,
    pub evidence: ObjectId,
    pub timestamp: u64,
}

/// Result of consistency checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyResult {
    pub is_consistent: bool,
    pub contradictions: Vec<Contradiction>,
    pub sheaf_violations: Vec<SheafViolation>,
}

impl ConsistencyResult {
    pub fn new() -> Self {
        Self {
            is_consistent: true,
            contradictions: Vec::new(),
            sheaf_violations: Vec::new(),
        }
    }
}

impl Default for ConsistencyResult {
    fn default() -> Self {
        Self::new()
    }
}

/// A contradiction in beliefs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    pub belief: ObjectId,
    pub context: ObjectId,
    pub reason: String,
}

/// A violation of sheaf axioms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SheafViolation {
    pub child_context: ObjectId,
    pub parent_context: ObjectId,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = Context::new("test")
            .with_property("key", serde_json::json!("value"));

        assert_eq!(ctx.name, "test");
        assert!(ctx.properties.contains_key("key"));
    }

    #[test]
    fn test_belief_state() {
        let ctx = ObjectId::new();
        let belief = BeliefState::new("The sky is blue")
            .with_confidence(0.9)
            .holds_in(ctx);

        assert_eq!(belief.truth_in(&ctx), TruthValue::True);
        assert!(belief.confidence > 0.8);
    }

    #[test]
    fn test_truth_value_logic() {
        assert_eq!(TruthValue::True.and(TruthValue::True), TruthValue::True);
        assert_eq!(TruthValue::True.and(TruthValue::False), TruthValue::False);
        assert_eq!(TruthValue::True.or(TruthValue::False), TruthValue::True);
        assert_eq!(TruthValue::False.not(), TruthValue::True);
    }

    #[test]
    fn test_belief_topos() {
        let topos = BeliefTopos::new();

        let ctx = topos.add_context(Context::new("world1"));
        let belief = BeliefState::new("Water is wet")
            .with_confidence(0.95)
            .holds_in(ctx);

        topos.add_belief(ctx, belief);

        let retrieved = topos.get_belief(&ctx);
        assert!(retrieved.is_some());
        assert!(retrieved.unwrap().confidence > 0.9);
    }

    #[test]
    fn test_consistency_check() {
        let topos = BeliefTopos::new();

        let ctx = topos.add_context(Context::new("test"));
        let belief = BeliefState::new("Test belief").holds_in(ctx);
        topos.add_belief(ctx, belief);

        let result = topos.check_consistency();
        assert!(result.is_consistent);
    }

    #[test]
    fn test_belief_revision() {
        let topos = BeliefTopos::new();

        let ctx = topos.add_context(Context::new("test"));
        let belief = BeliefState::new("Hypothesis").with_confidence(0.5);
        topos.add_belief(ctx, belief.clone());

        let evidence = Evidence::new("Supporting observation").with_strength(0.8);
        topos.revise_belief(belief.id, ctx, evidence).unwrap();

        let revised = topos.get_belief(&ctx).unwrap();
        assert!(revised.confidence > 0.5);
    }
}
