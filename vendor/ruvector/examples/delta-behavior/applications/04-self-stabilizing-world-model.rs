//! # Application 4: Self-Stabilizing World Models
//!
//! The world model is allowed to update only if the global structure remains intact.
//!
//! ## What Breaks Today
//! World models drift until they are no longer useful.
//!
//! ## Exotic Effect
//! The model stops learning when the world becomes incoherent
//! instead of hallucinating structure.
//!
//! ## Critical For
//! - Always-on perception
//! - Autonomous exploration
//! - Robotics in unknown environments

use std::collections::HashMap;

/// A world model that refuses to learn incoherent updates
pub struct SelfStabilizingWorldModel {
    /// Entities in the world
    entities: HashMap<EntityId, Entity>,

    /// Relationships between entities
    relationships: Vec<Relationship>,

    /// Physical laws the model believes
    laws: Vec<PhysicalLaw>,

    /// Current coherence of the model
    coherence: f64,

    /// History of coherence for trend detection
    coherence_history: Vec<f64>,

    /// Learning rate (decreases under low coherence)
    base_learning_rate: f64,

    /// Minimum coherence to allow updates
    min_update_coherence: f64,

    /// Updates that were rejected
    rejected_updates: Vec<RejectedUpdate>,
}

type EntityId = u64;

#[derive(Clone, Debug)]
pub struct Entity {
    pub id: EntityId,
    pub properties: HashMap<String, PropertyValue>,
    pub position: Option<(f64, f64, f64)>,
    pub last_observed: u64,
    pub confidence: f64,
}

#[derive(Clone, Debug)]
pub enum PropertyValue {
    Boolean(bool),
    Number(f64),
    String(String),
    Vector(Vec<f64>),
}

#[derive(Clone, Debug)]
pub struct Relationship {
    pub subject: EntityId,
    pub predicate: String,
    pub object: EntityId,
    pub confidence: f64,
}

#[derive(Clone, Debug)]
pub struct PhysicalLaw {
    pub name: String,
    pub confidence: f64,
    /// Number of observations supporting this law
    pub support_count: u64,
    /// Number of observations violating this law
    pub violation_count: u64,
}

#[derive(Debug)]
pub struct Observation {
    pub entity_id: EntityId,
    pub properties: HashMap<String, PropertyValue>,
    pub position: Option<(f64, f64, f64)>,
    pub timestamp: u64,
    pub source_confidence: f64,
}

#[derive(Debug)]
pub enum UpdateResult {
    /// Update applied successfully
    Applied { coherence_change: f64 },
    /// Update rejected to preserve coherence
    Rejected { reason: RejectionReason },
    /// Update partially applied with modifications
    Modified { changes: Vec<String>, coherence_change: f64 },
    /// Model entered "uncertain" mode - no updates allowed
    Frozen { coherence: f64, threshold: f64 },
}

#[derive(Debug, Clone)]
pub struct RejectedUpdate {
    pub observation: String,
    pub reason: RejectionReason,
    pub timestamp: u64,
    pub coherence_at_rejection: f64,
}

#[derive(Debug, Clone)]
pub enum RejectionReason {
    /// Would violate established physical laws
    ViolatesPhysicalLaw(String),
    /// Would create logical contradiction
    LogicalContradiction(String),
    /// Would cause excessive coherence drop
    ExcessiveCoherenceDrop { predicted: f64, threshold: f64 },
    /// Source confidence too low for this change
    InsufficientConfidence { required: f64, provided: f64 },
    /// Model is in frozen state
    ModelFrozen,
    /// Would fragment world structure
    StructuralFragmentation,
}

impl SelfStabilizingWorldModel {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            relationships: Vec::new(),
            laws: vec![
                PhysicalLaw {
                    name: "conservation_of_matter".to_string(),
                    confidence: 0.99,
                    support_count: 1000,
                    violation_count: 0,
                },
                PhysicalLaw {
                    name: "locality".to_string(),
                    confidence: 0.95,
                    support_count: 500,
                    violation_count: 5,
                },
                PhysicalLaw {
                    name: "temporal_consistency".to_string(),
                    confidence: 0.98,
                    support_count: 800,
                    violation_count: 2,
                },
            ],
            coherence: 1.0,
            coherence_history: vec![1.0],
            base_learning_rate: 0.1,
            min_update_coherence: 0.4,
            rejected_updates: Vec::new(),
        }
    }

    /// Current effective learning rate (decreases with low coherence)
    pub fn effective_learning_rate(&self) -> f64 {
        self.base_learning_rate * self.coherence.powi(2)
    }

    /// Is the model currently accepting updates?
    pub fn is_learning(&self) -> bool {
        self.coherence >= self.min_update_coherence
    }

    /// Attempt to integrate an observation into the world model
    pub fn observe(&mut self, observation: Observation, timestamp: u64) -> UpdateResult {
        // Check if model is frozen
        if !self.is_learning() {
            return UpdateResult::Frozen {
                coherence: self.coherence,
                threshold: self.min_update_coherence,
            };
        }

        // Predict coherence impact
        let predicted_coherence = self.predict_coherence_after(&observation);

        // Would this drop coherence too much?
        let coherence_drop = self.coherence - predicted_coherence;
        if coherence_drop > 0.2 {
            self.rejected_updates.push(RejectedUpdate {
                observation: format!("Entity {} update", observation.entity_id),
                reason: RejectionReason::ExcessiveCoherenceDrop {
                    predicted: predicted_coherence,
                    threshold: self.coherence - 0.2,
                },
                timestamp,
                coherence_at_rejection: self.coherence,
            });
            return UpdateResult::Rejected {
                reason: RejectionReason::ExcessiveCoherenceDrop {
                    predicted: predicted_coherence,
                    threshold: self.coherence - 0.2,
                },
            };
        }

        // Check physical law violations
        if let Some(violation) = self.check_law_violations(&observation) {
            self.rejected_updates.push(RejectedUpdate {
                observation: format!("Entity {} update", observation.entity_id),
                reason: violation.clone(),
                timestamp,
                coherence_at_rejection: self.coherence,
            });
            return UpdateResult::Rejected { reason: violation };
        }

        // Check logical consistency
        if let Some(contradiction) = self.check_contradictions(&observation) {
            self.rejected_updates.push(RejectedUpdate {
                observation: format!("Entity {} update", observation.entity_id),
                reason: contradiction.clone(),
                timestamp,
                coherence_at_rejection: self.coherence,
            });
            return UpdateResult::Rejected { reason: contradiction };
        }

        // Apply the update
        self.apply_observation(observation, timestamp);

        // Recalculate coherence
        let old_coherence = self.coherence;
        self.coherence = self.calculate_coherence();
        self.coherence_history.push(self.coherence);

        // Trim history
        if self.coherence_history.len() > 100 {
            self.coherence_history.remove(0);
        }

        UpdateResult::Applied {
            coherence_change: self.coherence - old_coherence,
        }
    }

    fn predict_coherence_after(&self, observation: &Observation) -> f64 {
        // Simulate the update's impact on coherence
        let mut consistency_score = 1.0;

        if let Some(existing) = self.entities.get(&observation.entity_id) {
            // How much does this differ from existing knowledge?
            for (key, new_value) in &observation.properties {
                if let Some(old_value) = existing.properties.get(key) {
                    let diff = self.property_difference(old_value, new_value);
                    consistency_score *= 1.0 - (diff * 0.5);
                }
            }

            // Position change check (locality)
            if let (Some(old_pos), Some(new_pos)) = (&existing.position, &observation.position) {
                let distance = ((new_pos.0 - old_pos.0).powi(2)
                    + (new_pos.1 - old_pos.1).powi(2)
                    + (new_pos.2 - old_pos.2).powi(2))
                .sqrt();

                // Large sudden movements are suspicious
                if distance > 10.0 {
                    consistency_score *= 0.7;
                }
            }
        }

        self.coherence * consistency_score
    }

    fn property_difference(&self, old: &PropertyValue, new: &PropertyValue) -> f64 {
        match (old, new) {
            (PropertyValue::Number(a), PropertyValue::Number(b)) => {
                let max = a.abs().max(b.abs()).max(1.0);
                ((a - b).abs() / max).min(1.0)
            }
            (PropertyValue::Boolean(a), PropertyValue::Boolean(b)) => {
                if a == b { 0.0 } else { 1.0 }
            }
            (PropertyValue::String(a), PropertyValue::String(b)) => {
                if a == b { 0.0 } else { 0.5 }
            }
            _ => 0.5, // Different types
        }
    }

    fn check_law_violations(&self, observation: &Observation) -> Option<RejectionReason> {
        if let Some(existing) = self.entities.get(&observation.entity_id) {
            // Check locality violation (teleportation)
            if let (Some(old_pos), Some(new_pos)) = (&existing.position, &observation.position) {
                let distance = ((new_pos.0 - old_pos.0).powi(2)
                    + (new_pos.1 - old_pos.1).powi(2)
                    + (new_pos.2 - old_pos.2).powi(2))
                .sqrt();

                // If object moved impossibly fast
                let max_speed = 100.0; // units per timestamp
                if distance > max_speed {
                    return Some(RejectionReason::ViolatesPhysicalLaw(
                        format!("locality: object moved {} units instantaneously", distance)
                    ));
                }
            }
        }

        None
    }

    fn check_contradictions(&self, observation: &Observation) -> Option<RejectionReason> {
        // Check for direct contradictions with high-confidence existing data
        if let Some(existing) = self.entities.get(&observation.entity_id) {
            if existing.confidence > 0.9 {
                for (key, new_value) in &observation.properties {
                    if let Some(old_value) = existing.properties.get(key) {
                        let diff = self.property_difference(old_value, new_value);
                        if diff > 0.9 && observation.source_confidence < existing.confidence {
                            return Some(RejectionReason::LogicalContradiction(
                                format!("Property {} contradicts high-confidence existing data", key)
                            ));
                        }
                    }
                }
            }
        }

        None
    }

    fn apply_observation(&mut self, observation: Observation, timestamp: u64) {
        let learning_rate = self.effective_learning_rate();

        // Pre-compute blended values to avoid borrow conflict
        let blended_properties: Vec<(String, PropertyValue)> = observation.properties
            .into_iter()
            .map(|(key, new_value)| {
                let blended = if let Some(entity) = self.entities.get(&observation.entity_id) {
                    if let Some(old_value) = entity.properties.get(&key) {
                        self.blend_values(old_value, &new_value, learning_rate)
                    } else {
                        new_value
                    }
                } else {
                    new_value
                };
                (key, blended)
            })
            .collect();

        let entity = self.entities.entry(observation.entity_id).or_insert(Entity {
            id: observation.entity_id,
            properties: HashMap::new(),
            position: None,
            last_observed: 0,
            confidence: 0.5,
        });

        // Apply pre-computed blended values
        for (key, blended) in blended_properties {
            entity.properties.insert(key, blended);
        }

        // Update position
        if let Some(new_pos) = observation.position {
            if let Some(old_pos) = entity.position {
                // Smooth position update
                entity.position = Some((
                    old_pos.0 + learning_rate * (new_pos.0 - old_pos.0),
                    old_pos.1 + learning_rate * (new_pos.1 - old_pos.1),
                    old_pos.2 + learning_rate * (new_pos.2 - old_pos.2),
                ));
            } else {
                entity.position = Some(new_pos);
            }
        }

        entity.last_observed = timestamp;
        // Update confidence
        entity.confidence = entity.confidence * 0.9 + observation.source_confidence * 0.1;
    }

    fn blend_values(&self, old: &PropertyValue, new: &PropertyValue, rate: f64) -> PropertyValue {
        match (old, new) {
            (PropertyValue::Number(a), PropertyValue::Number(b)) => {
                PropertyValue::Number(a + rate * (b - a))
            }
            _ => new.clone(), // For non-numeric, just use new if rate > 0.5
        }
    }

    fn calculate_coherence(&self) -> f64 {
        if self.entities.is_empty() {
            return 1.0;
        }

        let mut scores = Vec::new();

        // 1. Internal consistency of entities
        for entity in self.entities.values() {
            scores.push(entity.confidence);
        }

        // 2. Relationship consistency
        for rel in &self.relationships {
            if self.entities.contains_key(&rel.subject) && self.entities.contains_key(&rel.object) {
                scores.push(rel.confidence);
            } else {
                scores.push(0.0); // Dangling relationship
            }
        }

        // 3. Physical law confidence
        for law in &self.laws {
            scores.push(law.confidence);
        }

        // 4. Temporal coherence (recent observations should be consistent)
        let recent_variance = self.calculate_recent_variance();
        scores.push(1.0 - recent_variance);

        // Geometric mean of all scores
        if scores.is_empty() {
            1.0
        } else {
            let product: f64 = scores.iter().product();
            product.powf(1.0 / scores.len() as f64)
        }
    }

    fn calculate_recent_variance(&self) -> f64 {
        if self.coherence_history.len() < 2 {
            return 0.0;
        }

        let recent: Vec<f64> = self.coherence_history.iter().rev().take(10).cloned().collect();
        let mean: f64 = recent.iter().sum::<f64>() / recent.len() as f64;
        let variance: f64 = recent.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / recent.len() as f64;
        variance.sqrt().min(1.0)
    }

    /// Get count of rejected updates
    pub fn rejection_count(&self) -> usize {
        self.rejected_updates.len()
    }

    /// Get model status
    pub fn status(&self) -> String {
        format!(
            "WorldModel | Coherence: {:.3} | Entities: {} | Learning: {} | Rejections: {}",
            self.coherence,
            self.entities.len(),
            if self.is_learning() { "ON" } else { "FROZEN" },
            self.rejected_updates.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coherent_learning() {
        let mut model = SelfStabilizingWorldModel::new();

        // Feed consistent observations
        for i in 0..10 {
            let obs = Observation {
                entity_id: 1,
                properties: [("temperature".to_string(), PropertyValue::Number(20.0 + i as f64 * 0.1))].into(),
                position: Some((i as f64, 0.0, 0.0)),
                timestamp: i as u64,
                source_confidence: 0.9,
            };

            let result = model.observe(obs, i as u64);
            assert!(matches!(result, UpdateResult::Applied { .. }));
        }

        println!("{}", model.status());
        assert!(model.coherence > 0.8);
    }

    #[test]
    fn test_rejects_incoherent_update() {
        let mut model = SelfStabilizingWorldModel::new();

        // Establish entity at position
        let obs1 = Observation {
            entity_id: 1,
            properties: HashMap::new(),
            position: Some((0.0, 0.0, 0.0)),
            timestamp: 0,
            source_confidence: 0.95,
        };
        model.observe(obs1, 0);

        // Try to teleport it (violates locality)
        let obs2 = Observation {
            entity_id: 1,
            properties: HashMap::new(),
            position: Some((1000.0, 0.0, 0.0)), // Impossibly far
            timestamp: 1,
            source_confidence: 0.5,
        };

        let result = model.observe(obs2, 1);
        println!("Teleport result: {:?}", result);

        // Should be rejected
        assert!(matches!(result, UpdateResult::Rejected { .. }));
        println!("{}", model.status());
    }

    #[test]
    fn test_freezes_under_chaos() {
        let mut model = SelfStabilizingWorldModel::new();

        // Feed chaotic, contradictory observations
        for i in 0..100 {
            let obs = Observation {
                entity_id: (i % 5) as u64,
                properties: [
                    ("value".to_string(), PropertyValue::Number(if i % 2 == 0 { 100.0 } else { -100.0 }))
                ].into(),
                position: Some((
                    (i as f64 * 10.0) % 50.0 - 25.0,
                    (i as f64 * 7.0) % 50.0 - 25.0,
                    0.0
                )),
                timestamp: i as u64,
                source_confidence: 0.3,
            };

            let result = model.observe(obs, i as u64);

            if matches!(result, UpdateResult::Frozen { .. }) {
                println!("Model FROZE at step {} - stopped hallucinating!", i);
                println!("{}", model.status());
                return; // Test passes - model stopped itself
            }
        }

        println!("Final: {}", model.status());
        // Model should have either frozen or heavily rejected updates
        assert!(
            model.rejection_count() > 20 || model.coherence < 0.5,
            "Model should resist chaotic input"
        );
    }
}
