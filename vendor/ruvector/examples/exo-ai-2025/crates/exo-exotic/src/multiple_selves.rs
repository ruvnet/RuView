//! # Multiple Selves / Dissociation
//!
//! Partitioned consciousness within a single cognitive substrate, modeling
//! competing sub-personalities and the dynamics of self-coherence.
//!
//! ## Key Concepts
//!
//! - **Sub-Personalities**: Distinct processing modes with different goals
//! - **Attention as Arbiter**: Competition for conscious access
//! - **Integration vs Fragmentation**: Coherence of the self
//! - **Executive Function**: Unified decision-making across selves
//!
//! ## Theoretical Basis
//!
//! Inspired by:
//! - Internal Family Systems (IFS) therapy
//! - Dissociative identity research
//! - Marvin Minsky's "Society of Mind"
//! - Global Workspace Theory

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// System managing multiple sub-personalities
#[derive(Debug)]
pub struct MultipleSelvesSystem {
    /// Collection of sub-personalities
    selves: Vec<SubPersonality>,
    /// Currently dominant self
    dominant: Option<Uuid>,
    /// Executive function (arbiter)
    executive: ExecutiveFunction,
    /// Overall coherence measure
    coherence: SelfCoherence,
    /// Integration history
    integration_history: Vec<IntegrationEvent>,
}

/// A sub-personality with its own goals and style
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubPersonality {
    pub id: Uuid,
    /// Name/label for this self
    pub name: String,
    /// Core beliefs/values
    pub beliefs: Vec<Belief>,
    /// Goals this self pursues
    pub goals: Vec<Goal>,
    /// Emotional baseline
    pub emotional_tone: EmotionalTone,
    /// Activation level (0-1)
    pub activation: f64,
    /// Age/experience of this self
    pub age: u64,
    /// Relationships with other selves
    pub relationships: HashMap<Uuid, Relationship>,
}

/// A belief held by a sub-personality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Belief {
    pub content: String,
    pub strength: f64,
    pub valence: f64, // positive/negative
}

/// A goal pursued by a sub-personality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub description: String,
    pub priority: f64,
    pub progress: f64,
}

/// Emotional baseline of a sub-personality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalTone {
    pub valence: f64,   // -1 (negative) to 1 (positive)
    pub arousal: f64,   // 0 (calm) to 1 (excited)
    pub dominance: f64, // 0 (submissive) to 1 (dominant)
}

/// Relationship between sub-personalities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub other_id: Uuid,
    pub relationship_type: RelationshipType,
    pub strength: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RelationshipType {
    Protector,
    Exile,
    Manager,
    Firefighter,
    Ally,
    Rival,
    Neutral,
}

/// Executive function that arbitrates between selves
#[derive(Debug)]
pub struct ExecutiveFunction {
    /// Strength of executive control
    strength: f64,
    /// Decision threshold
    threshold: f64,
    /// Recent decisions
    decisions: Vec<Decision>,
    /// Conflict resolution style
    style: ResolutionStyle,
}

#[derive(Debug, Clone)]
pub enum ResolutionStyle {
    /// Dominant self wins
    Dominance,
    /// Average all inputs
    Averaging,
    /// Negotiate between selves
    Negotiation,
    /// Let them take turns
    TurnTaking,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub id: Uuid,
    pub participants: Vec<Uuid>,
    pub outcome: DecisionOutcome,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub enum DecisionOutcome {
    Unanimous(Uuid),     // All agreed, winner's id
    Majority(Uuid, f64), // Majority, winner and margin
    Executive(Uuid),     // Executive decided
    Conflict,            // Unresolved conflict
}

/// Measure of self-coherence
#[derive(Debug)]
pub struct SelfCoherence {
    /// Overall coherence score (0-1)
    score: f64,
    /// Conflict level
    conflict: f64,
    /// Integration level
    integration: f64,
    /// Stability over time
    #[allow(dead_code)]
    stability: f64,
}

/// Event in integration history
#[derive(Debug, Clone)]
pub struct IntegrationEvent {
    pub event_type: IntegrationType,
    pub selves_involved: Vec<Uuid>,
    pub timestamp: u64,
    pub outcome: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IntegrationType {
    Merge,
    Split,
    Activation,
    Deactivation,
    Conflict,
    Resolution,
}

impl MultipleSelvesSystem {
    /// Create a new multiple selves system
    pub fn new() -> Self {
        Self {
            selves: Vec::new(),
            dominant: None,
            executive: ExecutiveFunction::new(0.7),
            coherence: SelfCoherence::new(),
            integration_history: Vec::new(),
        }
    }

    /// Add a new sub-personality
    pub fn add_self(&mut self, name: &str, emotional_tone: EmotionalTone) -> Uuid {
        let id = Uuid::new_v4();
        self.selves.push(SubPersonality {
            id,
            name: name.to_string(),
            beliefs: Vec::new(),
            goals: Vec::new(),
            emotional_tone,
            activation: 0.5,
            age: 0,
            relationships: HashMap::new(),
        });

        if self.dominant.is_none() {
            self.dominant = Some(id);
        }

        id
    }

    /// Measure overall coherence
    pub fn measure_coherence(&mut self) -> f64 {
        if self.selves.is_empty() {
            return 1.0; // Single self = perfectly coherent
        }

        // Calculate belief consistency
        let belief_coherence = self.calculate_belief_coherence();

        // Calculate goal alignment
        let goal_alignment = self.calculate_goal_alignment();

        // Calculate relationship harmony
        let harmony = self.calculate_harmony();

        // Overall coherence
        self.coherence.score = (belief_coherence + goal_alignment + harmony) / 3.0;
        self.coherence.integration = (belief_coherence + goal_alignment) / 2.0;
        self.coherence.conflict = 1.0 - harmony;

        self.coherence.score
    }

    fn calculate_belief_coherence(&self) -> f64 {
        if self.selves.len() < 2 {
            return 1.0;
        }

        let mut total_similarity = 0.0;
        let mut count = 0;

        for i in 0..self.selves.len() {
            for j in i + 1..self.selves.len() {
                let sim = self.belief_similarity(&self.selves[i], &self.selves[j]);
                total_similarity += sim;
                count += 1;
            }
        }

        if count > 0 {
            total_similarity / count as f64
        } else {
            1.0
        }
    }

    fn belief_similarity(&self, a: &SubPersonality, b: &SubPersonality) -> f64 {
        if a.beliefs.is_empty() || b.beliefs.is_empty() {
            return 0.5; // Neutral if no beliefs
        }

        // Compare emotional tones as proxy for beliefs
        let valence_diff = (a.emotional_tone.valence - b.emotional_tone.valence).abs();
        let arousal_diff = (a.emotional_tone.arousal - b.emotional_tone.arousal).abs();

        1.0 - (valence_diff + arousal_diff) / 2.0
    }

    fn calculate_goal_alignment(&self) -> f64 {
        if self.selves.len() < 2 {
            return 1.0;
        }

        // Check if goals point in same direction
        let mut total_alignment = 0.0;
        let mut count = 0;

        for self_entity in &self.selves {
            for goal in &self_entity.goals {
                total_alignment += goal.priority * goal.progress;
                count += 1;
            }
        }

        if count > 0 {
            (total_alignment / count as f64).min(1.0)
        } else {
            0.5
        }
    }

    fn calculate_harmony(&self) -> f64 {
        let mut positive_relationships = 0;
        let mut total_relationships = 0;

        for self_entity in &self.selves {
            for (_, rel) in &self_entity.relationships {
                total_relationships += 1;
                if matches!(
                    rel.relationship_type,
                    RelationshipType::Ally
                        | RelationshipType::Protector
                        | RelationshipType::Neutral
                ) {
                    positive_relationships += 1;
                }
            }
        }

        if total_relationships > 0 {
            positive_relationships as f64 / total_relationships as f64
        } else {
            0.5 // Neutral if no relationships
        }
    }

    /// Activate a sub-personality
    pub fn activate(&mut self, self_id: Uuid, level: f64) {
        if let Some(self_entity) = self.selves.iter_mut().find(|s| s.id == self_id) {
            self_entity.activation = level.clamp(0.0, 1.0);

            self.integration_history.push(IntegrationEvent {
                event_type: IntegrationType::Activation,
                selves_involved: vec![self_id],
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                outcome: level,
            });
        }

        // Update dominant if necessary
        self.update_dominant();
    }

    fn update_dominant(&mut self) {
        self.dominant = self
            .selves
            .iter()
            .max_by(|a, b| a.activation.partial_cmp(&b.activation).unwrap())
            .map(|s| s.id);
    }

    /// Create conflict between selves
    pub fn create_conflict(&mut self, self1: Uuid, self2: Uuid) {
        if let Some(s1) = self.selves.iter_mut().find(|s| s.id == self1) {
            s1.relationships.insert(
                self2,
                Relationship {
                    other_id: self2,
                    relationship_type: RelationshipType::Rival,
                    strength: 0.7,
                },
            );
        }

        if let Some(s2) = self.selves.iter_mut().find(|s| s.id == self2) {
            s2.relationships.insert(
                self1,
                Relationship {
                    other_id: self1,
                    relationship_type: RelationshipType::Rival,
                    strength: 0.7,
                },
            );
        }

        self.integration_history.push(IntegrationEvent {
            event_type: IntegrationType::Conflict,
            selves_involved: vec![self1, self2],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            outcome: -0.5,
        });
    }

    /// Resolve conflict through executive function
    pub fn resolve_conflict(&mut self, self1: Uuid, self2: Uuid) -> Option<Uuid> {
        let winner = self.executive.arbitrate(&self.selves, self1, self2);

        if winner.is_some() {
            // Update relationship to neutral
            if let Some(s1) = self.selves.iter_mut().find(|s| s.id == self1) {
                if let Some(rel) = s1.relationships.get_mut(&self2) {
                    rel.relationship_type = RelationshipType::Neutral;
                }
            }

            if let Some(s2) = self.selves.iter_mut().find(|s| s.id == self2) {
                if let Some(rel) = s2.relationships.get_mut(&self1) {
                    rel.relationship_type = RelationshipType::Neutral;
                }
            }

            self.integration_history.push(IntegrationEvent {
                event_type: IntegrationType::Resolution,
                selves_involved: vec![self1, self2],
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                outcome: 0.8,
            });
        }

        winner
    }

    /// Merge two sub-personalities
    pub fn merge(&mut self, self1: Uuid, self2: Uuid) -> Option<Uuid> {
        let s1_idx = self.selves.iter().position(|s| s.id == self1)?;
        let s2_idx = self.selves.iter().position(|s| s.id == self2)?;

        // Create merged self
        let merged_id = Uuid::new_v4();
        let s1 = &self.selves[s1_idx];
        let s2 = &self.selves[s2_idx];

        let merged = SubPersonality {
            id: merged_id,
            name: format!("{}-{}", s1.name, s2.name),
            beliefs: [s1.beliefs.clone(), s2.beliefs.clone()].concat(),
            goals: [s1.goals.clone(), s2.goals.clone()].concat(),
            emotional_tone: EmotionalTone {
                valence: (s1.emotional_tone.valence + s2.emotional_tone.valence) / 2.0,
                arousal: (s1.emotional_tone.arousal + s2.emotional_tone.arousal) / 2.0,
                dominance: (s1.emotional_tone.dominance + s2.emotional_tone.dominance) / 2.0,
            },
            activation: (s1.activation + s2.activation) / 2.0,
            age: s1.age.max(s2.age),
            relationships: HashMap::new(),
        };

        // Remove old selves (handle indices carefully)
        let (first, second) = if s1_idx > s2_idx {
            (s1_idx, s2_idx)
        } else {
            (s2_idx, s1_idx)
        };
        self.selves.remove(first);
        self.selves.remove(second);

        self.selves.push(merged);

        self.integration_history.push(IntegrationEvent {
            event_type: IntegrationType::Merge,
            selves_involved: vec![self1, self2, merged_id],
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            outcome: 1.0,
        });

        Some(merged_id)
    }

    /// Get dominant self
    pub fn get_dominant(&self) -> Option<&SubPersonality> {
        self.dominant
            .and_then(|id| self.selves.iter().find(|s| s.id == id))
    }

    /// Get all selves
    pub fn all_selves(&self) -> &[SubPersonality] {
        &self.selves
    }

    /// Get self count
    pub fn self_count(&self) -> usize {
        self.selves.len()
    }

    /// Get coherence
    pub fn coherence(&self) -> &SelfCoherence {
        &self.coherence
    }
}

impl Default for MultipleSelvesSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutiveFunction {
    /// Create new executive function
    pub fn new(strength: f64) -> Self {
        Self {
            strength,
            threshold: 0.6,
            decisions: Vec::new(),
            style: ResolutionStyle::Negotiation,
        }
    }

    /// Arbitrate between two selves
    pub fn arbitrate(&mut self, selves: &[SubPersonality], id1: Uuid, id2: Uuid) -> Option<Uuid> {
        let s1 = selves.iter().find(|s| s.id == id1)?;
        let s2 = selves.iter().find(|s| s.id == id2)?;

        let outcome = match self.style {
            ResolutionStyle::Dominance => {
                // Most activated wins
                if s1.activation > s2.activation {
                    DecisionOutcome::Majority(id1, s1.activation - s2.activation)
                } else {
                    DecisionOutcome::Majority(id2, s2.activation - s1.activation)
                }
            }
            ResolutionStyle::Averaging => {
                // Neither wins clearly
                DecisionOutcome::Conflict
            }
            ResolutionStyle::Negotiation => {
                // Executive decides based on strength
                if self.strength > self.threshold {
                    let winner = if s1.emotional_tone.dominance > s2.emotional_tone.dominance {
                        id1
                    } else {
                        id2
                    };
                    DecisionOutcome::Executive(winner)
                } else {
                    DecisionOutcome::Conflict
                }
            }
            ResolutionStyle::TurnTaking => {
                // Alternate based on history
                let last_winner = self.decisions.last().and_then(|d| match &d.outcome {
                    DecisionOutcome::Unanimous(id)
                    | DecisionOutcome::Majority(id, _)
                    | DecisionOutcome::Executive(id) => Some(*id),
                    _ => None,
                });

                let winner = match last_winner {
                    Some(w) if w == id1 => id2,
                    Some(w) if w == id2 => id1,
                    _ => id1,
                };
                DecisionOutcome::Majority(winner, 0.5)
            }
        };

        let winner = match &outcome {
            DecisionOutcome::Unanimous(id)
            | DecisionOutcome::Majority(id, _)
            | DecisionOutcome::Executive(id) => Some(*id),
            DecisionOutcome::Conflict => None,
        };

        self.decisions.push(Decision {
            id: Uuid::new_v4(),
            participants: vec![id1, id2],
            outcome,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        });

        winner
    }

    /// Set resolution style
    pub fn set_style(&mut self, style: ResolutionStyle) {
        self.style = style;
    }
}

impl SelfCoherence {
    /// Create new coherence tracker
    pub fn new() -> Self {
        Self {
            score: 1.0,
            conflict: 0.0,
            integration: 1.0,
            stability: 1.0,
        }
    }

    /// Get coherence score
    pub fn score(&self) -> f64 {
        self.score
    }

    /// Get conflict level
    pub fn conflict(&self) -> f64 {
        self.conflict
    }

    /// Get integration level
    pub fn integration(&self) -> f64 {
        self.integration
    }
}

impl Default for SelfCoherence {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiple_selves_creation() {
        let system = MultipleSelvesSystem::new();
        assert_eq!(system.self_count(), 0);
    }

    #[test]
    fn test_add_selves() {
        let mut system = MultipleSelvesSystem::new();

        let id1 = system.add_self(
            "Protector",
            EmotionalTone {
                valence: 0.3,
                arousal: 0.7,
                dominance: 0.8,
            },
        );

        let id2 = system.add_self(
            "Inner Child",
            EmotionalTone {
                valence: 0.8,
                arousal: 0.6,
                dominance: 0.3,
            },
        );

        assert_eq!(system.self_count(), 2);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_coherence_measurement() {
        let mut system = MultipleSelvesSystem::new();

        // Single self = high coherence
        system.add_self(
            "Core",
            EmotionalTone {
                valence: 0.5,
                arousal: 0.5,
                dominance: 0.5,
            },
        );

        let coherence = system.measure_coherence();
        assert!(coherence >= 0.0 && coherence <= 1.0);
    }

    #[test]
    fn test_activation() {
        let mut system = MultipleSelvesSystem::new();

        let id = system.add_self(
            "Test",
            EmotionalTone {
                valence: 0.5,
                arousal: 0.5,
                dominance: 0.5,
            },
        );

        system.activate(id, 0.9);

        let dominant = system.get_dominant();
        assert!(dominant.is_some());
        assert_eq!(dominant.unwrap().id, id);
    }

    #[test]
    fn test_conflict_and_resolution() {
        let mut system = MultipleSelvesSystem::new();

        let id1 = system.add_self(
            "Self1",
            EmotionalTone {
                valence: 0.8,
                arousal: 0.5,
                dominance: 0.7,
            },
        );

        let id2 = system.add_self(
            "Self2",
            EmotionalTone {
                valence: 0.2,
                arousal: 0.5,
                dominance: 0.3,
            },
        );

        system.create_conflict(id1, id2);
        let initial_coherence = system.measure_coherence();

        system.resolve_conflict(id1, id2);
        let final_coherence = system.measure_coherence();

        // Coherence should improve after resolution
        assert!(final_coherence >= initial_coherence);
    }

    #[test]
    fn test_merge() {
        let mut system = MultipleSelvesSystem::new();

        let id1 = system.add_self(
            "Part1",
            EmotionalTone {
                valence: 0.6,
                arousal: 0.4,
                dominance: 0.5,
            },
        );

        let id2 = system.add_self(
            "Part2",
            EmotionalTone {
                valence: 0.4,
                arousal: 0.6,
                dominance: 0.5,
            },
        );

        assert_eq!(system.self_count(), 2);

        let merged_id = system.merge(id1, id2);
        assert!(merged_id.is_some());
        assert_eq!(system.self_count(), 1);
    }

    #[test]
    fn test_executive_function() {
        let mut exec = ExecutiveFunction::new(0.8);

        let selves = vec![
            SubPersonality {
                id: Uuid::new_v4(),
                name: "Strong".to_string(),
                beliefs: Vec::new(),
                goals: Vec::new(),
                emotional_tone: EmotionalTone {
                    valence: 0.5,
                    arousal: 0.5,
                    dominance: 0.9,
                },
                activation: 0.8,
                age: 10,
                relationships: HashMap::new(),
            },
            SubPersonality {
                id: Uuid::new_v4(),
                name: "Weak".to_string(),
                beliefs: Vec::new(),
                goals: Vec::new(),
                emotional_tone: EmotionalTone {
                    valence: 0.5,
                    arousal: 0.5,
                    dominance: 0.1,
                },
                activation: 0.2,
                age: 5,
                relationships: HashMap::new(),
            },
        ];

        let winner = exec.arbitrate(&selves, selves[0].id, selves[1].id);
        assert!(winner.is_some());
    }
}
