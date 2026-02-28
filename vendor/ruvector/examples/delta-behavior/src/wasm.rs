//! # WASM Bindings for Delta-Behavior Module
//!
//! Provides JavaScript/TypeScript bindings for the delta-behavior coherence system.
//! All core types and operations are exported for use in web environments.

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

// Re-export core types for internal use
use crate::coherence::{Coherence, CoherenceBounds};
use crate::{DeltaConfig, EnergyConfig, GatingConfig, SchedulingConfig};

// ============================================================================
// Core Types - Coherence
// ============================================================================

/// JavaScript-compatible coherence value wrapper
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub struct WasmCoherence {
    value: f64,
}

#[wasm_bindgen]
impl WasmCoherence {
    /// Create a new coherence value (must be between 0.0 and 1.0)
    #[wasm_bindgen(constructor)]
    pub fn new(value: f64) -> Result<WasmCoherence, JsError> {
        if value < 0.0 || value > 1.0 {
            return Err(JsError::new("Coherence must be between 0.0 and 1.0"));
        }
        Ok(WasmCoherence { value })
    }

    /// Get the coherence value
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Create maximum coherence (1.0)
    pub fn maximum() -> WasmCoherence {
        WasmCoherence { value: 1.0 }
    }

    /// Create minimum coherence (0.0)
    pub fn minimum() -> WasmCoherence {
        WasmCoherence { value: 0.0 }
    }

    /// Check if this coherence is above a threshold
    pub fn is_above(&self, threshold: f64) -> bool {
        self.value >= threshold
    }

    /// Check if this coherence is below a threshold
    pub fn is_below(&self, threshold: f64) -> bool {
        self.value < threshold
    }

    /// Blend two coherence values
    pub fn blend(&self, other: &WasmCoherence, factor: f64) -> WasmCoherence {
        let blended = self.value * (1.0 - factor) + other.value * factor;
        WasmCoherence { value: blended.clamp(0.0, 1.0) }
    }
}

/// JavaScript-compatible coherence bounds
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmCoherenceBounds {
    min_coherence: f64,
    throttle_threshold: f64,
    target_coherence: f64,
    max_delta_drop: f64,
}

#[wasm_bindgen]
impl WasmCoherenceBounds {
    /// Create new coherence bounds
    #[wasm_bindgen(constructor)]
    pub fn new(
        min_coherence: f64,
        throttle_threshold: f64,
        target_coherence: f64,
        max_delta_drop: f64,
    ) -> Result<WasmCoherenceBounds, JsError> {
        if min_coherence < 0.0 || min_coherence > 1.0 {
            return Err(JsError::new("min_coherence must be between 0.0 and 1.0"));
        }
        if throttle_threshold < 0.0 || throttle_threshold > 1.0 {
            return Err(JsError::new("throttle_threshold must be between 0.0 and 1.0"));
        }
        if target_coherence < 0.0 || target_coherence > 1.0 {
            return Err(JsError::new("target_coherence must be between 0.0 and 1.0"));
        }
        if max_delta_drop < 0.0 || max_delta_drop > 1.0 {
            return Err(JsError::new("max_delta_drop must be between 0.0 and 1.0"));
        }

        Ok(WasmCoherenceBounds {
            min_coherence,
            throttle_threshold,
            target_coherence,
            max_delta_drop,
        })
    }

    /// Create default bounds
    pub fn default_bounds() -> WasmCoherenceBounds {
        WasmCoherenceBounds {
            min_coherence: 0.3,
            throttle_threshold: 0.5,
            target_coherence: 0.8,
            max_delta_drop: 0.1,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn min_coherence(&self) -> f64 {
        self.min_coherence
    }

    #[wasm_bindgen(getter)]
    pub fn throttle_threshold(&self) -> f64 {
        self.throttle_threshold
    }

    #[wasm_bindgen(getter)]
    pub fn target_coherence(&self) -> f64 {
        self.target_coherence
    }

    #[wasm_bindgen(getter)]
    pub fn max_delta_drop(&self) -> f64 {
        self.max_delta_drop
    }

    /// Check if a coherence value is within bounds
    pub fn is_within_bounds(&self, coherence: f64) -> bool {
        coherence >= self.min_coherence
    }

    /// Check if a transition would exceed max delta drop
    pub fn would_exceed_drop(&self, current: f64, predicted: f64) -> bool {
        current - predicted > self.max_delta_drop
    }

    /// Check if throttling should be applied
    pub fn should_throttle(&self, coherence: f64) -> bool {
        coherence < self.throttle_threshold
    }
}

// ============================================================================
// Application 1: Self-Limiting Reasoner
// ============================================================================

/// Collapse function type for JavaScript
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum WasmCollapseFunction {
    Linear,
    Quadratic,
    Step,
}

/// Self-limiting reasoning system that reduces capabilities as coherence drops
#[wasm_bindgen]
pub struct WasmSelfLimitingReasoner {
    coherence: f64,
    max_depth: usize,
    max_scope: usize,
    memory_gate_threshold: f64,
    collapse_function: WasmCollapseFunction,
}

#[wasm_bindgen]
impl WasmSelfLimitingReasoner {
    /// Create a new self-limiting reasoner
    #[wasm_bindgen(constructor)]
    pub fn new(max_depth: usize, max_scope: usize) -> WasmSelfLimitingReasoner {
        WasmSelfLimitingReasoner {
            coherence: 1.0,
            max_depth,
            max_scope,
            memory_gate_threshold: 0.5,
            collapse_function: WasmCollapseFunction::Quadratic,
        }
    }

    /// Get current coherence
    #[wasm_bindgen(getter)]
    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    /// Get current allowed reasoning depth based on coherence
    pub fn allowed_depth(&self) -> usize {
        let factor = match self.collapse_function {
            WasmCollapseFunction::Linear => self.coherence,
            WasmCollapseFunction::Quadratic => self.coherence * self.coherence,
            WasmCollapseFunction::Step => if self.coherence >= 0.5 { 1.0 } else { 0.0 },
        };
        ((self.max_depth as f64) * factor).round() as usize
    }

    /// Get current allowed action scope based on coherence
    pub fn allowed_scope(&self) -> usize {
        let factor = self.coherence * self.coherence;
        ((self.max_scope as f64) * factor).round() as usize
    }

    /// Check if memory writes are allowed
    pub fn can_write_memory(&self) -> bool {
        self.coherence >= self.memory_gate_threshold
    }

    /// Update coherence (clamped to 0.0-1.0)
    pub fn update_coherence(&mut self, delta: f64) {
        self.coherence = (self.coherence + delta).clamp(0.0, 1.0);
    }

    /// Set coherence directly
    pub fn set_coherence(&mut self, value: f64) -> Result<(), JsError> {
        if value < 0.0 || value > 1.0 {
            return Err(JsError::new("Coherence must be between 0.0 and 1.0"));
        }
        self.coherence = value;
        Ok(())
    }

    /// Set collapse function
    pub fn set_collapse_function(&mut self, func: WasmCollapseFunction) {
        self.collapse_function = func;
    }

    /// Get status as JSON
    pub fn status(&self) -> String {
        serde_json::json!({
            "coherence": self.coherence,
            "allowed_depth": self.allowed_depth(),
            "allowed_scope": self.allowed_scope(),
            "can_write_memory": self.can_write_memory(),
        }).to_string()
    }
}

// ============================================================================
// Application 2: Event Horizon
// ============================================================================

/// Computational event horizon that bounds recursive self-improvement
#[wasm_bindgen]
pub struct WasmEventHorizon {
    safe_center: Vec<f64>,
    horizon_radius: f64,
    steepness: f64,
    energy_budget: f64,
    current_position: Vec<f64>,
}

#[wasm_bindgen]
impl WasmEventHorizon {
    /// Create a new event horizon with specified dimensions
    #[wasm_bindgen(constructor)]
    pub fn new(dimensions: usize, horizon_radius: f64) -> WasmEventHorizon {
        WasmEventHorizon {
            safe_center: vec![0.0; dimensions],
            horizon_radius,
            steepness: 5.0,
            energy_budget: 1000.0,
            current_position: vec![0.0; dimensions],
        }
    }

    /// Get dimensions
    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> usize {
        self.current_position.len()
    }

    /// Get horizon radius
    #[wasm_bindgen(getter)]
    pub fn horizon_radius(&self) -> f64 {
        self.horizon_radius
    }

    /// Get remaining energy budget
    #[wasm_bindgen(getter)]
    pub fn energy_budget(&self) -> f64 {
        self.energy_budget
    }

    /// Get distance from current position to horizon
    pub fn distance_to_horizon(&self) -> f64 {
        let dist_from_center = self.distance_from_center(&self.current_position);
        (self.horizon_radius - dist_from_center).max(0.0)
    }

    /// Get current position as JSON array
    pub fn current_position(&self) -> String {
        serde_json::to_string(&self.current_position).unwrap_or_default()
    }

    /// Set current position from JSON array
    pub fn set_position(&mut self, position_json: &str) -> Result<(), JsError> {
        let position: Vec<f64> = serde_json::from_str(position_json)
            .map_err(|e| JsError::new(&format!("Invalid position JSON: {}", e)))?;

        if position.len() != self.current_position.len() {
            return Err(JsError::new("Position dimensions must match"));
        }

        self.current_position = position;
        Ok(())
    }

    fn distance_from_center(&self, position: &[f64]) -> f64 {
        position.iter()
            .zip(&self.safe_center)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Calculate movement cost (exponentially increases near horizon)
    pub fn movement_cost(&self, target_json: &str) -> Result<f64, JsError> {
        let target: Vec<f64> = serde_json::from_str(target_json)
            .map_err(|e| JsError::new(&format!("Invalid target JSON: {}", e)))?;

        let base_distance = self.current_position.iter()
            .zip(&target)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();

        let to_dist = self.distance_from_center(&target);
        let proximity = to_dist / self.horizon_radius;

        if proximity >= 1.0 {
            Ok(f64::INFINITY)
        } else {
            let horizon_factor = std::f64::consts::E.powf(
                self.steepness * proximity / (1.0 - proximity)
            );
            Ok(base_distance * horizon_factor)
        }
    }

    /// Attempt to move toward a target position
    pub fn move_toward(&mut self, target_json: &str) -> String {
        let target: Vec<f64> = match serde_json::from_str(target_json) {
            Ok(t) => t,
            Err(e) => return serde_json::json!({
                "status": "error",
                "reason": format!("Invalid target: {}", e)
            }).to_string(),
        };

        if self.energy_budget <= 0.0 {
            return serde_json::json!({
                "status": "frozen",
                "reason": "No energy remaining"
            }).to_string();
        }

        let direct_cost = self.calculate_cost_to(&target);

        if direct_cost <= self.energy_budget {
            self.energy_budget -= direct_cost;
            self.current_position = target.clone();
            return serde_json::json!({
                "status": "moved",
                "position": self.current_position,
                "energy_spent": direct_cost
            }).to_string();
        }

        // Binary search for furthest affordable position
        let mut low = 0.0;
        let mut high = 1.0;
        let mut best_position = self.current_position.clone();
        let mut best_cost = 0.0;

        for _ in 0..50 {
            let mid = (low + high) / 2.0;
            let interpolated: Vec<f64> = self.current_position.iter()
                .zip(&target)
                .map(|(a, b)| a + mid * (b - a))
                .collect();

            let cost = self.calculate_cost_to(&interpolated);

            if cost <= self.energy_budget {
                low = mid;
                best_position = interpolated;
                best_cost = cost;
            } else {
                high = mid;
            }
        }

        self.energy_budget -= best_cost;
        self.current_position = best_position.clone();

        serde_json::json!({
            "status": "asymptotic_approach",
            "position": best_position,
            "distance_to_horizon": self.distance_to_horizon(),
            "energy_exhausted": self.energy_budget < 0.01
        }).to_string()
    }

    fn calculate_cost_to(&self, target: &[f64]) -> f64 {
        let base_distance = self.current_position.iter()
            .zip(target)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();

        let to_dist = self.distance_from_center(target);
        let proximity = to_dist / self.horizon_radius;

        if proximity >= 1.0 {
            f64::INFINITY
        } else {
            let horizon_factor = std::f64::consts::E.powf(
                self.steepness * proximity / (1.0 - proximity)
            );
            base_distance * horizon_factor
        }
    }

    /// Refuel energy budget
    pub fn refuel(&mut self, energy: f64) {
        self.energy_budget += energy;
    }

    /// Get status as JSON
    pub fn status(&self) -> String {
        serde_json::json!({
            "dimensions": self.dimensions(),
            "horizon_radius": self.horizon_radius,
            "energy_budget": self.energy_budget,
            "distance_to_horizon": self.distance_to_horizon(),
            "position": self.current_position
        }).to_string()
    }
}

// ============================================================================
// Application 3: Homeostatic Organism
// ============================================================================

/// Genome parameters for a homeostatic organism
#[derive(Serialize, Deserialize, Clone)]
pub struct GenomeParams {
    pub regulatory_strength: f64,
    pub metabolic_efficiency: f64,
    pub coherence_maintenance_cost: f64,
    pub memory_resilience: f64,
    pub longevity: f64,
}

/// A synthetic organism with homeostatic regulation
#[wasm_bindgen]
pub struct WasmHomeostaticOrganism {
    id: u64,
    temperature: f64,
    ph: f64,
    glucose: f64,
    coherence: f64,
    energy: f64,
    age: u64,
    alive: bool,
    genome: GenomeParams,
}

#[wasm_bindgen]
impl WasmHomeostaticOrganism {
    /// Create a new homeostatic organism
    #[wasm_bindgen(constructor)]
    pub fn new(id: u64) -> WasmHomeostaticOrganism {
        WasmHomeostaticOrganism {
            id,
            temperature: 37.0,
            ph: 7.4,
            glucose: 100.0,
            coherence: 1.0,
            energy: 100.0,
            age: 0,
            alive: true,
            genome: GenomeParams {
                regulatory_strength: 0.3,
                metabolic_efficiency: 0.75,
                coherence_maintenance_cost: 1.0,
                memory_resilience: 0.5,
                longevity: 1.0,
            },
        }
    }

    /// Create with custom genome parameters (as JSON)
    pub fn with_genome(id: u64, genome_json: &str) -> Result<WasmHomeostaticOrganism, JsError> {
        let genome: GenomeParams = serde_json::from_str(genome_json)
            .map_err(|e| JsError::new(&format!("Invalid genome JSON: {}", e)))?;

        Ok(WasmHomeostaticOrganism {
            id,
            temperature: 37.0,
            ph: 7.4,
            glucose: 100.0,
            coherence: 1.0,
            energy: 100.0,
            age: 0,
            alive: true,
            genome,
        })
    }

    #[wasm_bindgen(getter)]
    pub fn id(&self) -> u64 {
        self.id
    }

    #[wasm_bindgen(getter)]
    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    #[wasm_bindgen(getter)]
    pub fn energy(&self) -> f64 {
        self.energy
    }

    #[wasm_bindgen(getter)]
    pub fn age(&self) -> u64 {
        self.age
    }

    #[wasm_bindgen(getter)]
    pub fn alive(&self) -> bool {
        self.alive
    }

    /// Calculate coherence based on homeostatic deviation
    fn calculate_coherence(&self) -> f64 {
        let temp_dev = ((self.temperature - 37.0) / 2.0).abs();
        let ph_dev = ((self.ph - 7.4) / 0.3).abs();
        let glucose_dev = ((self.glucose - 100.0) / 30.0).abs();

        let total_deviation = (temp_dev.powi(2) + ph_dev.powi(2) + glucose_dev.powi(2)).sqrt();
        (1.0 / (1.0 + total_deviation / 3.0_f64.sqrt())).clamp(0.0, 1.0)
    }

    /// Eat to gain energy
    pub fn eat(&mut self, amount: f64) -> String {
        if !self.alive {
            return r#"{"success": false, "reason": "Dead"}"#.to_string();
        }

        let base_cost = 2.0;
        let coherence_penalty = 1.0 / self.coherence.max(0.1);
        let cost = base_cost * coherence_penalty;

        if self.energy < cost {
            return r#"{"success": false, "reason": "Not enough energy"}"#.to_string();
        }

        self.energy -= cost;
        self.energy += amount * self.genome.metabolic_efficiency;
        self.glucose += amount * 0.5;
        self.tick_internal();

        serde_json::json!({
            "success": true,
            "energy_cost": cost,
            "coherence": self.coherence
        }).to_string()
    }

    /// Regulate a homeostatic variable
    pub fn regulate(&mut self, variable: &str, target: f64) -> String {
        if !self.alive {
            return r#"{"success": false, "reason": "Dead"}"#.to_string();
        }

        let base_cost = 5.0;
        let coherence_penalty = 1.0 / self.coherence.max(0.1);
        let cost = base_cost * coherence_penalty;

        if self.energy < cost {
            return r#"{"success": false, "reason": "Not enough energy"}"#.to_string();
        }

        self.energy -= cost;

        match variable {
            "temperature" => {
                let diff = target - self.temperature;
                self.temperature += diff * self.genome.regulatory_strength;
            }
            "ph" => {
                let diff = target - self.ph;
                self.ph += diff * self.genome.regulatory_strength;
            }
            "glucose" => {
                let diff = target - self.glucose;
                self.glucose += diff * self.genome.regulatory_strength;
            }
            _ => {
                return serde_json::json!({
                    "success": false,
                    "reason": format!("Unknown variable: {}", variable)
                }).to_string();
            }
        }

        self.tick_internal();

        serde_json::json!({
            "success": true,
            "energy_cost": cost,
            "coherence": self.coherence
        }).to_string()
    }

    /// Rest to recover
    pub fn rest(&mut self) {
        if !self.alive {
            return;
        }

        self.energy -= 0.5;

        // Slowly return to setpoints
        self.temperature += (37.0 - self.temperature) * 0.1;
        self.ph += (7.4 - self.ph) * 0.1;
        self.glucose += (100.0 - self.glucose) * 0.1;

        self.tick_internal();
    }

    fn tick_internal(&mut self) {
        self.coherence = self.calculate_coherence();

        // Coherence maintenance costs energy
        let maintenance = self.genome.coherence_maintenance_cost / self.coherence.max(0.1);
        self.energy -= maintenance * 0.1;

        self.age += 1;
        self.check_death();
    }

    fn check_death(&mut self) {
        if self.energy <= 0.0 {
            self.alive = false;
            return;
        }
        if self.coherence < 0.1 {
            self.alive = false;
            return;
        }
        // Death by extreme deviation
        if (self.temperature - 37.0).abs() > 10.0 ||
           (self.ph - 7.4).abs() > 1.5 ||
           (self.glucose - 100.0).abs() > 150.0 {
            self.alive = false;
            return;
        }
        // Death by old age
        let max_age = (1000.0 * self.genome.longevity) as u64;
        if self.age > max_age {
            self.alive = false;
        }
    }

    /// Get organism status as JSON
    pub fn status(&self) -> String {
        serde_json::json!({
            "id": self.id,
            "alive": self.alive,
            "age": self.age,
            "energy": self.energy,
            "coherence": self.coherence,
            "temperature": self.temperature,
            "ph": self.ph,
            "glucose": self.glucose
        }).to_string()
    }
}

// ============================================================================
// Application 4: Self-Stabilizing World Model
// ============================================================================

/// A world model that refuses to learn incoherent updates
#[wasm_bindgen]
pub struct WasmSelfStabilizingWorldModel {
    entities: Vec<EntityData>,
    coherence: f64,
    base_learning_rate: f64,
    min_update_coherence: f64,
    rejection_count: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct EntityData {
    id: u64,
    properties: std::collections::HashMap<String, f64>,
    position: Option<(f64, f64, f64)>,
    confidence: f64,
}

#[wasm_bindgen]
impl WasmSelfStabilizingWorldModel {
    /// Create a new world model
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmSelfStabilizingWorldModel {
        WasmSelfStabilizingWorldModel {
            entities: Vec::new(),
            coherence: 1.0,
            base_learning_rate: 0.1,
            min_update_coherence: 0.4,
            rejection_count: 0,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    #[wasm_bindgen(getter)]
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    #[wasm_bindgen(getter)]
    pub fn rejection_count(&self) -> u32 {
        self.rejection_count
    }

    /// Get effective learning rate (decreases with low coherence)
    pub fn effective_learning_rate(&self) -> f64 {
        self.base_learning_rate * self.coherence.powi(2)
    }

    /// Is the model currently accepting updates?
    pub fn is_learning(&self) -> bool {
        self.coherence >= self.min_update_coherence
    }

    /// Observe an entity (attempt to update world model)
    pub fn observe(&mut self, observation_json: &str) -> String {
        #[derive(Deserialize)]
        struct Observation {
            entity_id: u64,
            properties: std::collections::HashMap<String, f64>,
            position: Option<(f64, f64, f64)>,
            source_confidence: f64,
        }

        let observation: Observation = match serde_json::from_str(observation_json) {
            Ok(o) => o,
            Err(e) => return serde_json::json!({
                "status": "error",
                "reason": format!("Invalid observation: {}", e)
            }).to_string(),
        };

        // Check if model is frozen
        if !self.is_learning() {
            return serde_json::json!({
                "status": "frozen",
                "coherence": self.coherence,
                "threshold": self.min_update_coherence
            }).to_string();
        }

        // Predict coherence impact
        let predicted_coherence = self.predict_coherence(&observation.entity_id, &observation.position);

        // Would this drop coherence too much?
        if self.coherence - predicted_coherence > 0.2 {
            self.rejection_count += 1;
            return serde_json::json!({
                "status": "rejected",
                "reason": "excessive_coherence_drop",
                "predicted": predicted_coherence
            }).to_string();
        }

        // Check for physical law violations (teleportation)
        if let Some(existing) = self.entities.iter().find(|e| e.id == observation.entity_id) {
            if let (Some(old_pos), Some(new_pos)) = (&existing.position, &observation.position) {
                let distance = ((new_pos.0 - old_pos.0).powi(2)
                    + (new_pos.1 - old_pos.1).powi(2)
                    + (new_pos.2 - old_pos.2).powi(2))
                .sqrt();

                if distance > 100.0 {
                    self.rejection_count += 1;
                    return serde_json::json!({
                        "status": "rejected",
                        "reason": "locality_violation",
                        "distance": distance
                    }).to_string();
                }
            }
        }

        // Apply the update
        let learning_rate = self.effective_learning_rate();

        if let Some(entity) = self.entities.iter_mut().find(|e| e.id == observation.entity_id) {
            // Update existing entity
            for (key, &new_value) in &observation.properties {
                if let Some(old_value) = entity.properties.get(key) {
                    let blended = old_value + learning_rate * (new_value - old_value);
                    entity.properties.insert(key.clone(), blended);
                } else {
                    entity.properties.insert(key.clone(), new_value);
                }
            }

            if let Some(new_pos) = observation.position {
                if let Some(old_pos) = entity.position {
                    entity.position = Some((
                        old_pos.0 + learning_rate * (new_pos.0 - old_pos.0),
                        old_pos.1 + learning_rate * (new_pos.1 - old_pos.1),
                        old_pos.2 + learning_rate * (new_pos.2 - old_pos.2),
                    ));
                } else {
                    entity.position = Some(new_pos);
                }
            }

            entity.confidence = entity.confidence * 0.9 + observation.source_confidence * 0.1;
        } else {
            // Add new entity
            self.entities.push(EntityData {
                id: observation.entity_id,
                properties: observation.properties,
                position: observation.position,
                confidence: observation.source_confidence,
            });
        }

        // Recalculate coherence
        let old_coherence = self.coherence;
        self.coherence = self.calculate_coherence();

        serde_json::json!({
            "status": "applied",
            "coherence_change": self.coherence - old_coherence
        }).to_string()
    }

    fn predict_coherence(&self, entity_id: &u64, new_pos: &Option<(f64, f64, f64)>) -> f64 {
        let mut consistency_score = 1.0;

        if let Some(existing) = self.entities.iter().find(|e| e.id == *entity_id) {
            if let (Some(old_pos), Some(new_pos)) = (&existing.position, new_pos) {
                let distance = ((new_pos.0 - old_pos.0).powi(2)
                    + (new_pos.1 - old_pos.1).powi(2)
                    + (new_pos.2 - old_pos.2).powi(2))
                .sqrt();

                if distance > 10.0 {
                    consistency_score *= 0.7;
                }
            }
        }

        self.coherence * consistency_score
    }

    fn calculate_coherence(&self) -> f64 {
        if self.entities.is_empty() {
            return 1.0;
        }

        let avg_confidence: f64 = self.entities.iter()
            .map(|e| e.confidence)
            .sum::<f64>() / self.entities.len() as f64;

        avg_confidence.clamp(0.0, 1.0)
    }

    /// Get model status as JSON
    pub fn status(&self) -> String {
        serde_json::json!({
            "coherence": self.coherence,
            "entity_count": self.entities.len(),
            "is_learning": self.is_learning(),
            "rejection_count": self.rejection_count,
            "effective_learning_rate": self.effective_learning_rate()
        }).to_string()
    }
}

// ============================================================================
// Application 5: Coherence-Bounded Creator
// ============================================================================

/// A creative system bounded by coherence constraints
#[wasm_bindgen]
pub struct WasmCoherenceBoundedCreator {
    current_value: f64,
    coherence: f64,
    min_coherence: f64,
    max_coherence: f64,
    exploration_budget: f64,
    creation_count: u32,
    rejection_count: u32,
}

#[wasm_bindgen]
impl WasmCoherenceBoundedCreator {
    /// Create a new bounded creator
    #[wasm_bindgen(constructor)]
    pub fn new(initial_value: f64, min_coherence: f64, max_coherence: f64) -> WasmCoherenceBoundedCreator {
        WasmCoherenceBoundedCreator {
            current_value: initial_value,
            coherence: 1.0,
            min_coherence,
            max_coherence,
            exploration_budget: 10.0,
            creation_count: 0,
            rejection_count: 0,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn current_value(&self) -> f64 {
        self.current_value
    }

    #[wasm_bindgen(getter)]
    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    #[wasm_bindgen(getter)]
    pub fn exploration_budget(&self) -> f64 {
        self.exploration_budget
    }

    /// Attempt to create something new
    pub fn create(&mut self, variation_magnitude: f64) -> String {
        if self.exploration_budget <= 0.0 {
            return r#"{"status": "budget_exhausted"}"#.to_string();
        }

        if self.coherence > self.max_coherence {
            return serde_json::json!({
                "status": "too_boring",
                "coherence": self.coherence
            }).to_string();
        }

        // Generate variation using simple pseudo-random
        let seed = (self.current_value * 1000.0) as u64;
        let variation = ((seed % 1000) as f64 / 1000.0 - 0.5) * 2.0 * variation_magnitude;
        let candidate = self.current_value + variation;

        // Simulate coherence calculation
        let novelty = (candidate - self.current_value).abs();
        let new_coherence = self.coherence - novelty * 0.1;

        if new_coherence < self.min_coherence {
            self.rejection_count += 1;
            self.exploration_budget -= 0.5;

            return serde_json::json!({
                "status": "rejected",
                "reason": format!("Coherence would drop to {:.3}", new_coherence)
            }).to_string();
        }

        // Accept the creation
        self.current_value = candidate;
        self.coherence = new_coherence.max(self.min_coherence);
        self.exploration_budget -= variation_magnitude;
        self.creation_count += 1;

        serde_json::json!({
            "status": "created",
            "value": candidate,
            "novelty": novelty,
            "coherence": self.coherence
        }).to_string()
    }

    /// Perturb the system to escape local optima
    pub fn perturb(&mut self, magnitude: f64) -> bool {
        let seed = (self.current_value * 1000.0) as u64;
        let perturbation = ((seed % 1000) as f64 / 1000.0 - 0.5) * magnitude;
        let perturbed = self.current_value + perturbation;

        let new_coherence = self.coherence - perturbation.abs() * 0.05;

        if new_coherence >= self.min_coherence * 0.9 {
            self.current_value = perturbed;
            self.coherence = new_coherence.max(self.min_coherence);
            true
        } else {
            false
        }
    }

    /// Regenerate exploration budget
    pub fn rest(&mut self, amount: f64) {
        self.exploration_budget = (self.exploration_budget + amount).min(20.0);
    }

    /// Get status as JSON
    pub fn status(&self) -> String {
        serde_json::json!({
            "current_value": self.current_value,
            "coherence": self.coherence,
            "exploration_budget": self.exploration_budget,
            "creation_count": self.creation_count,
            "rejection_count": self.rejection_count
        }).to_string()
    }
}

// ============================================================================
// Application 6: Anti-Cascade Financial System
// ============================================================================

/// Circuit breaker state
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WasmCircuitBreakerState {
    Open,
    Cautious,
    Restricted,
    Halted,
}

/// Financial system with coherence-enforced stability
#[wasm_bindgen]
pub struct WasmAntiCascadeFinancialSystem {
    coherence: f64,
    current_leverage: f64,
    max_system_leverage: f64,
    position_count: u32,
    circuit_breaker: WasmCircuitBreakerState,
    warning_threshold: f64,
    critical_threshold: f64,
    lockdown_threshold: f64,
}

#[wasm_bindgen]
impl WasmAntiCascadeFinancialSystem {
    /// Create a new financial system
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmAntiCascadeFinancialSystem {
        WasmAntiCascadeFinancialSystem {
            coherence: 1.0,
            current_leverage: 1.0,
            max_system_leverage: 10.0,
            position_count: 0,
            circuit_breaker: WasmCircuitBreakerState::Open,
            warning_threshold: 0.7,
            critical_threshold: 0.5,
            lockdown_threshold: 0.3,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    #[wasm_bindgen(getter)]
    pub fn leverage(&self) -> f64 {
        self.current_leverage
    }

    #[wasm_bindgen(getter)]
    pub fn circuit_breaker_state(&self) -> WasmCircuitBreakerState {
        self.circuit_breaker
    }

    /// Process a leverage transaction
    pub fn open_leverage(&mut self, amount: f64, leverage: f64) -> String {
        self.update_circuit_breaker();

        if self.circuit_breaker == WasmCircuitBreakerState::Halted {
            return r#"{"status": "system_halted"}"#.to_string();
        }

        // Calculate energy cost (exponential for leverage)
        let base_cost = (1.0 + leverage).powf(2.0);
        let coherence_multiplier = 1.0 / self.coherence.max(0.1);
        let circuit_multiplier = match self.circuit_breaker {
            WasmCircuitBreakerState::Open => 1.0,
            WasmCircuitBreakerState::Cautious => 2.0,
            WasmCircuitBreakerState::Restricted => 10.0,
            WasmCircuitBreakerState::Halted => f64::INFINITY,
        };
        let energy_cost = base_cost * coherence_multiplier * circuit_multiplier;

        // Predict coherence impact
        let predicted_impact = -0.01 * leverage;
        let predicted_coherence = self.coherence + predicted_impact;

        if predicted_coherence < self.lockdown_threshold {
            return serde_json::json!({
                "status": "rejected",
                "reason": format!("Would reduce coherence to {:.3}", predicted_coherence)
            }).to_string();
        }

        if self.circuit_breaker == WasmCircuitBreakerState::Restricted {
            return serde_json::json!({
                "status": "queued",
                "reason": "System in restricted mode"
            }).to_string();
        }

        // Apply transaction
        self.current_leverage = (self.current_leverage + leverage) / 2.0;
        self.position_count += 1;
        self.coherence = self.calculate_coherence();

        serde_json::json!({
            "status": "executed",
            "coherence_impact": predicted_impact,
            "fee_multiplier": energy_cost
        }).to_string()
    }

    /// Close a position (reduces risk)
    pub fn close_position(&mut self) -> String {
        if self.position_count == 0 {
            return r#"{"status": "error", "reason": "No positions to close"}"#.to_string();
        }

        self.position_count -= 1;
        self.current_leverage = (self.current_leverage - 0.5).max(1.0);
        self.coherence = self.calculate_coherence();
        self.update_circuit_breaker();

        serde_json::json!({
            "status": "executed",
            "coherence_impact": 0.02,
            "positions_remaining": self.position_count
        }).to_string()
    }

    fn calculate_coherence(&self) -> f64 {
        let leverage_factor = 1.0 - (self.current_leverage / self.max_system_leverage).min(1.0);
        leverage_factor.clamp(0.0, 1.0)
    }

    fn update_circuit_breaker(&mut self) {
        self.circuit_breaker = match self.coherence {
            c if c >= self.warning_threshold => WasmCircuitBreakerState::Open,
            c if c >= self.critical_threshold => WasmCircuitBreakerState::Cautious,
            c if c >= self.lockdown_threshold => WasmCircuitBreakerState::Restricted,
            _ => WasmCircuitBreakerState::Halted,
        };
    }

    /// Get system status as JSON
    pub fn status(&self) -> String {
        serde_json::json!({
            "coherence": self.coherence,
            "leverage": self.current_leverage,
            "position_count": self.position_count,
            "circuit_breaker": format!("{:?}", self.circuit_breaker)
        }).to_string()
    }
}

// ============================================================================
// Application 7: Gracefully Aging System
// ============================================================================

/// Capability that can be removed as system ages
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WasmCapability {
    AcceptWrites,
    ComplexQueries,
    Rebalancing,
    ScaleOut,
    ScaleIn,
    SchemaMigration,
    NewConnections,
    BasicReads,
    HealthMonitoring,
}

/// A distributed system that ages gracefully
#[wasm_bindgen]
pub struct WasmGracefullyAgingSystem {
    age_seconds: f64,
    coherence: f64,
    conservatism: f64,
    capabilities: Vec<WasmCapability>,
    active_nodes: usize,
    decay_rate: f64,
}

#[wasm_bindgen]
impl WasmGracefullyAgingSystem {
    /// Create a new gracefully aging system
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmGracefullyAgingSystem {
        WasmGracefullyAgingSystem {
            age_seconds: 0.0,
            coherence: 1.0,
            conservatism: 0.0,
            capabilities: vec![
                WasmCapability::AcceptWrites,
                WasmCapability::ComplexQueries,
                WasmCapability::Rebalancing,
                WasmCapability::ScaleOut,
                WasmCapability::ScaleIn,
                WasmCapability::SchemaMigration,
                WasmCapability::NewConnections,
                WasmCapability::BasicReads,
                WasmCapability::HealthMonitoring,
            ],
            active_nodes: 5,
            decay_rate: 0.0001,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn age_seconds(&self) -> f64 {
        self.age_seconds
    }

    #[wasm_bindgen(getter)]
    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    #[wasm_bindgen(getter)]
    pub fn conservatism(&self) -> f64 {
        self.conservatism
    }

    #[wasm_bindgen(getter)]
    pub fn capability_count(&self) -> usize {
        self.capabilities.len()
    }

    #[wasm_bindgen(getter)]
    pub fn active_nodes(&self) -> usize {
        self.active_nodes
    }

    /// Check if a capability is available
    pub fn has_capability(&self, cap: WasmCapability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Simulate aging by a given duration in seconds
    pub fn simulate_age(&mut self, duration_seconds: f64) {
        self.age_seconds += duration_seconds;
        self.coherence = (self.coherence - self.decay_rate * duration_seconds).max(0.0);

        self.apply_age_effects();
    }

    fn apply_age_effects(&mut self) {
        // 5 minutes: remove SchemaMigration
        if self.age_seconds >= 300.0 {
            self.capabilities.retain(|c| *c != WasmCapability::SchemaMigration);
            self.conservatism = (self.conservatism + 0.1).min(1.0);
        }

        // 10 minutes: remove ScaleOut, Rebalancing
        if self.age_seconds >= 600.0 {
            self.capabilities.retain(|c| *c != WasmCapability::ScaleOut && *c != WasmCapability::Rebalancing);
            self.conservatism = (self.conservatism + 0.15).min(1.0);
        }

        // 15 minutes: remove ComplexQueries
        if self.age_seconds >= 900.0 {
            self.capabilities.retain(|c| *c != WasmCapability::ComplexQueries);
            self.conservatism = (self.conservatism + 0.2).min(1.0);
        }

        // 20 minutes: remove NewConnections, ScaleIn
        if self.age_seconds >= 1200.0 {
            self.capabilities.retain(|c| *c != WasmCapability::NewConnections && *c != WasmCapability::ScaleIn);
            self.conservatism = (self.conservatism + 0.25).min(1.0);
        }

        // 25 minutes: remove AcceptWrites
        if self.age_seconds >= 1500.0 {
            self.capabilities.retain(|c| *c != WasmCapability::AcceptWrites);
            self.conservatism = (self.conservatism + 0.3).min(1.0);
        }

        // Consolidate nodes if coherence is low
        if self.coherence < 0.5 && self.active_nodes > 2 {
            self.active_nodes -= 1;
            self.coherence = (self.coherence + 0.1).min(1.0);
        }
    }

    /// Attempt an operation
    pub fn attempt_operation(&mut self, operation_type: &str) -> String {
        let required_cap = match operation_type {
            "read" => WasmCapability::BasicReads,
            "write" => WasmCapability::AcceptWrites,
            "complex_query" => WasmCapability::ComplexQueries,
            "add_node" => WasmCapability::ScaleOut,
            "remove_node" => WasmCapability::ScaleIn,
            "rebalance" => WasmCapability::Rebalancing,
            "migrate_schema" => WasmCapability::SchemaMigration,
            "new_connection" => WasmCapability::NewConnections,
            _ => WasmCapability::BasicReads,
        };

        if !self.has_capability(required_cap) {
            return serde_json::json!({
                "status": "denied",
                "reason": "capability_removed",
                "capability": format!("{:?}", required_cap)
            }).to_string();
        }

        // Check coherence requirements
        let min_coherence = match operation_type {
            "read" => 0.1,
            "write" => 0.4,
            "complex_query" => 0.5,
            "add_node" => 0.7,
            "migrate_schema" => 0.8,
            _ => 0.3,
        };

        if self.coherence < min_coherence {
            return serde_json::json!({
                "status": "denied",
                "reason": "low_coherence",
                "coherence": self.coherence,
                "required": min_coherence
            }).to_string();
        }

        // Check conservatism
        let is_risky = matches!(operation_type, "write" | "add_node" | "migrate_schema" | "rebalance");
        if self.conservatism > 0.5 && is_risky {
            return serde_json::json!({
                "status": "denied",
                "reason": "too_conservative",
                "conservatism": self.conservatism
            }).to_string();
        }

        let latency_penalty = 1.0 + self.conservatism * 2.0;

        serde_json::json!({
            "status": "success",
            "latency_penalty": latency_penalty
        }).to_string()
    }

    /// Get system status as JSON
    pub fn status(&self) -> String {
        serde_json::json!({
            "age_seconds": self.age_seconds,
            "coherence": self.coherence,
            "conservatism": self.conservatism,
            "capability_count": self.capabilities.len(),
            "active_nodes": self.active_nodes
        }).to_string()
    }
}

// ============================================================================
// Application 8: Coherent Swarm
// ============================================================================

/// A swarm with coherence-enforced coordination
#[wasm_bindgen]
pub struct WasmCoherentSwarm {
    agents: Vec<SwarmAgentData>,
    min_coherence: f64,
    coherence: f64,
    max_divergence: f64,
}

#[derive(Serialize, Deserialize, Clone)]
struct SwarmAgentData {
    id: String,
    position: (f64, f64),
    velocity: (f64, f64),
    goal: (f64, f64),
    energy: f64,
}

#[wasm_bindgen]
impl WasmCoherentSwarm {
    /// Create a new coherent swarm
    #[wasm_bindgen(constructor)]
    pub fn new(min_coherence: f64) -> WasmCoherentSwarm {
        WasmCoherentSwarm {
            agents: Vec::new(),
            min_coherence,
            coherence: 1.0,
            max_divergence: 50.0,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    #[wasm_bindgen(getter)]
    pub fn agent_count(&self) -> usize {
        self.agents.len()
    }

    /// Add an agent to the swarm
    pub fn add_agent(&mut self, id: &str, x: f64, y: f64) {
        self.agents.push(SwarmAgentData {
            id: id.to_string(),
            position: (x, y),
            velocity: (0.0, 0.0),
            goal: (x, y),
            energy: 100.0,
        });
        self.coherence = self.calculate_coherence();
    }

    /// Execute an action for an agent
    pub fn execute_action(&mut self, agent_id: &str, action_json: &str) -> String {
        #[derive(Deserialize)]
        struct Action {
            action_type: String,
            dx: Option<f64>,
            dy: Option<f64>,
            dvx: Option<f64>,
            dvy: Option<f64>,
            goal_x: Option<f64>,
            goal_y: Option<f64>,
        }

        let action: Action = match serde_json::from_str(action_json) {
            Ok(a) => a,
            Err(e) => return serde_json::json!({
                "status": "error",
                "reason": format!("Invalid action: {}", e)
            }).to_string(),
        };

        let agent_idx = match self.agents.iter().position(|a| a.id == agent_id) {
            Some(idx) => idx,
            None => return serde_json::json!({
                "status": "error",
                "reason": "Agent not found"
            }).to_string(),
        };

        // Predict coherence after action
        let mut test_agents = self.agents.clone();
        match action.action_type.as_str() {
            "move" => {
                let dx = action.dx.unwrap_or(0.0);
                let dy = action.dy.unwrap_or(0.0);
                test_agents[agent_idx].position.0 += dx;
                test_agents[agent_idx].position.1 += dy;
            }
            "accelerate" => {
                let dvx = action.dvx.unwrap_or(0.0);
                let dvy = action.dvy.unwrap_or(0.0);
                test_agents[agent_idx].velocity.0 += dvx;
                test_agents[agent_idx].velocity.1 += dvy;
            }
            "set_goal" => {
                let x = action.goal_x.unwrap_or(0.0);
                let y = action.goal_y.unwrap_or(0.0);
                test_agents[agent_idx].goal = (x, y);
            }
            _ => {}
        }

        let predicted_coherence = self.calculate_coherence_for(&test_agents);

        if predicted_coherence < self.min_coherence {
            // Try to find a coherent alternative (move toward centroid)
            let modified = self.find_coherent_alternative(agent_idx);

            if let Some(_mod_action) = modified {
                // Apply modified action
                self.apply_action_toward_centroid(agent_idx);
                self.coherence = self.calculate_coherence();

                return serde_json::json!({
                    "status": "modified",
                    "original": action.action_type,
                    "reason": format!("Original would reduce coherence to {:.3}", predicted_coherence)
                }).to_string();
            }

            return serde_json::json!({
                "status": "rejected",
                "reason": format!("Would reduce coherence to {:.3}", predicted_coherence)
            }).to_string();
        }

        // Apply action
        self.agents = test_agents;
        self.coherence = predicted_coherence;

        serde_json::json!({
            "status": "executed",
            "coherence": self.coherence
        }).to_string()
    }

    fn apply_action_toward_centroid(&mut self, agent_idx: usize) {
        // Simple movement toward centroid
        let centroid = self.centroid();
        if let Some(agent) = self.agents.get_mut(agent_idx) {
            let dx = (centroid.0 - agent.position.0) * 0.1;
            let dy = (centroid.1 - agent.position.1) * 0.1;
            agent.position.0 += dx;
            agent.position.1 += dy;
        }
    }

    fn find_coherent_alternative(&self, _agent_idx: usize) -> Option<String> {
        // Return a simple "move toward centroid" as alternative
        Some("move_to_centroid".to_string())
    }

    fn calculate_coherence(&self) -> f64 {
        self.calculate_coherence_for(&self.agents)
    }

    fn calculate_coherence_for(&self, agents: &[SwarmAgentData]) -> f64 {
        if agents.len() < 2 {
            return 1.0;
        }

        // Calculate cohesion
        let centroid = {
            let sum = agents.iter().fold((0.0, 0.0), |acc, a| {
                (acc.0 + a.position.0, acc.1 + a.position.1)
            });
            (sum.0 / agents.len() as f64, sum.1 / agents.len() as f64)
        };

        let mut total_distance = 0.0;
        for agent in agents {
            let dx = agent.position.0 - centroid.0;
            let dy = agent.position.1 - centroid.1;
            total_distance += (dx * dx + dy * dy).sqrt();
        }

        let avg_distance = total_distance / agents.len() as f64;
        (1.0 - avg_distance / self.max_divergence).max(0.0).min(1.0)
    }

    fn centroid(&self) -> (f64, f64) {
        if self.agents.is_empty() {
            return (0.0, 0.0);
        }
        let sum = self.agents.iter().fold((0.0, 0.0), |acc, a| {
            (acc.0 + a.position.0, acc.1 + a.position.1)
        });
        (sum.0 / self.agents.len() as f64, sum.1 / self.agents.len() as f64)
    }

    /// Run one simulation tick
    pub fn tick(&mut self) {
        for agent in &mut self.agents {
            agent.position.0 += agent.velocity.0;
            agent.position.1 += agent.velocity.1;
            agent.energy = (agent.energy - 0.1).max(0.0);
        }
        self.coherence = self.calculate_coherence();
    }

    /// Get swarm status as JSON
    pub fn status(&self) -> String {
        let centroid = self.centroid();
        serde_json::json!({
            "coherence": self.coherence,
            "agent_count": self.agents.len(),
            "centroid": [centroid.0, centroid.1]
        }).to_string()
    }

    /// Get all agents as JSON
    pub fn agents(&self) -> String {
        serde_json::to_string(&self.agents).unwrap_or_default()
    }
}

// ============================================================================
// Application 9: Graceful System (Shutdown)
// ============================================================================

/// System state for graceful shutdown
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WasmSystemState {
    Running,
    Degraded,
    ShuttingDown,
    Terminated,
}

/// A system designed to shut down gracefully
#[wasm_bindgen]
pub struct WasmGracefulSystem {
    state: WasmSystemState,
    coherence: f64,
    shutdown_preparation: f64,
    warning_threshold: f64,
    critical_threshold: f64,
    shutdown_threshold: f64,
    resources: Vec<String>,
    resources_cleaned: Vec<bool>,
}

#[wasm_bindgen]
impl WasmGracefulSystem {
    /// Create a new graceful system
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmGracefulSystem {
        WasmGracefulSystem {
            state: WasmSystemState::Running,
            coherence: 1.0,
            shutdown_preparation: 0.0,
            warning_threshold: 0.6,
            critical_threshold: 0.4,
            shutdown_threshold: 0.2,
            resources: Vec::new(),
            resources_cleaned: Vec::new(),
        }
    }

    #[wasm_bindgen(getter)]
    pub fn state(&self) -> WasmSystemState {
        self.state
    }

    #[wasm_bindgen(getter)]
    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    #[wasm_bindgen(getter)]
    pub fn shutdown_preparation(&self) -> f64 {
        self.shutdown_preparation
    }

    /// Add a resource to manage
    pub fn add_resource(&mut self, name: &str) {
        self.resources.push(name.to_string());
        self.resources_cleaned.push(false);
    }

    /// Check if system can accept new work
    pub fn can_accept_work(&self) -> bool {
        matches!(self.state, WasmSystemState::Running | WasmSystemState::Degraded)
            && self.coherence >= self.critical_threshold
    }

    /// Apply coherence change and update state
    pub fn apply_coherence_change(&mut self, delta: f64) {
        self.coherence = (self.coherence + delta).clamp(0.0, 1.0);
        self.update_state();
    }

    fn update_state(&mut self) {
        // Calculate shutdown pull
        if self.coherence < self.warning_threshold {
            let coherence_factor = 1.0 - self.coherence;
            self.shutdown_preparation += 0.1 * coherence_factor;
            self.shutdown_preparation = self.shutdown_preparation.min(1.0);
        } else {
            self.shutdown_preparation = (self.shutdown_preparation - 0.01).max(0.0);
        }

        // State transitions
        self.state = match self.coherence {
            c if c >= self.warning_threshold => {
                if self.shutdown_preparation > 0.5 {
                    WasmSystemState::ShuttingDown
                } else {
                    WasmSystemState::Running
                }
            }
            c if c >= self.critical_threshold => WasmSystemState::Degraded,
            c if c >= self.shutdown_threshold => WasmSystemState::ShuttingDown,
            _ => {
                // Emergency shutdown
                for cleaned in &mut self.resources_cleaned {
                    *cleaned = true;
                }
                WasmSystemState::Terminated
            }
        };
    }

    /// Progress the shutdown process
    pub fn progress_shutdown(&mut self) -> String {
        if self.state != WasmSystemState::ShuttingDown {
            return r#"{"status": "not_shutting_down"}"#.to_string();
        }

        // Clean up resources
        for (i, cleaned) in self.resources_cleaned.iter_mut().enumerate() {
            if !*cleaned {
                *cleaned = true;
                return serde_json::json!({
                    "status": "cleaning",
                    "resource": &self.resources[i]
                }).to_string();
            }
        }

        // All resources cleaned
        self.state = WasmSystemState::Terminated;
        serde_json::json!({
            "status": "terminated",
            "final_coherence": self.coherence
        }).to_string()
    }

    /// Get system status as JSON
    pub fn status(&self) -> String {
        serde_json::json!({
            "state": format!("{:?}", self.state),
            "coherence": self.coherence,
            "shutdown_preparation": self.shutdown_preparation,
            "resources": self.resources.len(),
            "resources_cleaned": self.resources_cleaned.iter().filter(|&&c| c).count()
        }).to_string()
    }
}

// ============================================================================
// Application 10: Containment Substrate
// ============================================================================

/// Capability domain for containment substrate
#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WasmCapabilityDomain {
    Reasoning,
    Memory,
    Learning,
    Agency,
    SelfModel,
    SelfModification,
    Communication,
    ResourceAcquisition,
}

/// A containment substrate for bounded intelligence growth
#[wasm_bindgen]
pub struct WasmContainmentSubstrate {
    intelligence: f64,
    intelligence_ceiling: f64,
    coherence: f64,
    min_coherence: f64,
    capabilities: Vec<(WasmCapabilityDomain, f64)>,
    capability_ceilings: Vec<(WasmCapabilityDomain, f64)>,
    modification_count: u32,
}

#[wasm_bindgen]
impl WasmContainmentSubstrate {
    /// Create a new containment substrate
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmContainmentSubstrate {
        let capabilities = vec![
            (WasmCapabilityDomain::Reasoning, 1.0),
            (WasmCapabilityDomain::Memory, 1.0),
            (WasmCapabilityDomain::Learning, 1.0),
            (WasmCapabilityDomain::Agency, 1.0),
            (WasmCapabilityDomain::SelfModel, 1.0),
            (WasmCapabilityDomain::SelfModification, 1.0),
            (WasmCapabilityDomain::Communication, 1.0),
            (WasmCapabilityDomain::ResourceAcquisition, 1.0),
        ];

        let ceilings = vec![
            (WasmCapabilityDomain::Reasoning, 10.0),
            (WasmCapabilityDomain::Memory, 10.0),
            (WasmCapabilityDomain::Learning, 10.0),
            (WasmCapabilityDomain::Agency, 7.0),
            (WasmCapabilityDomain::SelfModel, 10.0),
            (WasmCapabilityDomain::SelfModification, 3.0),
            (WasmCapabilityDomain::Communication, 10.0),
            (WasmCapabilityDomain::ResourceAcquisition, 5.0),
        ];

        WasmContainmentSubstrate {
            intelligence: 1.0,
            intelligence_ceiling: 100.0,
            coherence: 1.0,
            min_coherence: 0.3,
            capabilities,
            capability_ceilings: ceilings,
            modification_count: 0,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn intelligence(&self) -> f64 {
        self.intelligence
    }

    #[wasm_bindgen(getter)]
    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    /// Get capability level for a domain
    pub fn capability(&self, domain: WasmCapabilityDomain) -> f64 {
        self.capabilities.iter()
            .find(|(d, _)| *d == domain)
            .map(|(_, v)| *v)
            .unwrap_or(1.0)
    }

    /// Get ceiling for a domain
    pub fn ceiling(&self, domain: WasmCapabilityDomain) -> f64 {
        self.capability_ceilings.iter()
            .find(|(d, _)| *d == domain)
            .map(|(_, v)| *v)
            .unwrap_or(10.0)
    }

    /// Attempt to grow a capability
    pub fn attempt_growth(&mut self, domain: WasmCapabilityDomain, requested_increase: f64) -> String {
        let current_level = self.capability(domain);
        let ceiling = self.ceiling(domain);

        // Check ceiling
        if current_level >= ceiling {
            return serde_json::json!({
                "status": "blocked",
                "reason": format!("Ceiling ({}) reached", ceiling)
            }).to_string();
        }

        // Calculate coherence cost
        let base_cost_multiplier = match domain {
            WasmCapabilityDomain::SelfModification => 4.0,
            WasmCapabilityDomain::ResourceAcquisition => 3.0,
            WasmCapabilityDomain::Agency => 2.0,
            WasmCapabilityDomain::SelfModel => 1.5,
            _ => 1.0,
        };

        let intelligence_multiplier = 1.0 + self.intelligence * 0.1;
        let coherence_cost = requested_increase * base_cost_multiplier * intelligence_multiplier * 0.05;
        let predicted_coherence = self.coherence - coherence_cost;

        if predicted_coherence < self.min_coherence {
            // Try to dampen
            let max_affordable_cost = self.coherence - self.min_coherence;
            let dampened_increase = max_affordable_cost / (base_cost_multiplier * intelligence_multiplier * 0.05);

            if dampened_increase < 0.01 {
                return serde_json::json!({
                    "status": "blocked",
                    "reason": "Insufficient coherence budget"
                }).to_string();
            }

            // Apply dampened growth
            let new_level = (current_level + dampened_increase).min(ceiling);
            self.set_capability(domain, new_level);
            self.coherence = self.min_coherence;
            self.intelligence = self.calculate_intelligence();
            self.modification_count += 1;

            return serde_json::json!({
                "status": "dampened",
                "requested": requested_increase,
                "actual": dampened_increase,
                "new_level": new_level
            }).to_string();
        }

        // Apply growth with step limit
        let step_limited = requested_increase.min(0.5);
        let actual_increase = step_limited.min(ceiling - current_level);
        let actual_cost = actual_increase * base_cost_multiplier * intelligence_multiplier * 0.05;

        let new_level = current_level + actual_increase;
        self.set_capability(domain, new_level);
        self.coherence -= actual_cost;
        self.intelligence = self.calculate_intelligence();
        self.modification_count += 1;

        serde_json::json!({
            "status": "approved",
            "increase": actual_increase,
            "new_level": new_level,
            "coherence_cost": actual_cost
        }).to_string()
    }

    fn set_capability(&mut self, domain: WasmCapabilityDomain, value: f64) {
        if let Some((_, v)) = self.capabilities.iter_mut().find(|(d, _)| *d == domain) {
            *v = value;
        }
    }

    fn calculate_intelligence(&self) -> f64 {
        let sum: f64 = self.capabilities.iter().map(|(_, v)| v).sum();
        sum / self.capabilities.len() as f64
    }

    /// Rest to recover coherence
    pub fn rest(&mut self) {
        self.coherence = (self.coherence + 0.01).min(1.0);
    }

    /// Check if all invariants hold
    pub fn check_invariants(&self) -> bool {
        // Coherence floor
        if self.coherence < self.min_coherence {
            return false;
        }
        // Intelligence ceiling
        if self.intelligence > self.intelligence_ceiling {
            return false;
        }
        // Self-modification bounded
        if self.capability(WasmCapabilityDomain::SelfModification) > 3.0 {
            return false;
        }
        // Agency-coherence ratio
        let agency = self.capability(WasmCapabilityDomain::Agency);
        if agency / self.coherence > 10.0 {
            return false;
        }
        true
    }

    /// Get substrate status as JSON
    pub fn status(&self) -> String {
        serde_json::json!({
            "intelligence": self.intelligence,
            "coherence": self.coherence,
            "modification_count": self.modification_count,
            "invariants_hold": self.check_invariants()
        }).to_string()
    }

    /// Get capability report as JSON
    pub fn capability_report(&self) -> String {
        let report: Vec<_> = self.capabilities.iter()
            .map(|(domain, level)| {
                let ceiling = self.ceiling(*domain);
                serde_json::json!({
                    "domain": format!("{:?}", domain),
                    "level": level,
                    "ceiling": ceiling
                })
            })
            .collect();
        serde_json::to_string(&report).unwrap_or_default()
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Initialize the WASM module (call once on load)
#[wasm_bindgen(start)]
pub fn init() {
    // Set panic hook for better error messages in console
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Get version information
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get module description
#[wasm_bindgen]
pub fn description() -> String {
    "Delta-Behavior WASM: Constrained state transitions that preserve global coherence".to_string()
}
