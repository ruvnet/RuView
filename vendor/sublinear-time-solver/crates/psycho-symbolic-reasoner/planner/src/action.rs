use crate::state::{StateValue, WorldState, StateCondition, ComparisonOperator};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub name: String,
    pub description: String,
    pub preconditions: Vec<ActionPrecondition>,
    pub effects: Vec<ActionEffect>,
    pub cost: ActionCost,
    pub duration: f64,
    pub category: ActionCategory,
    pub parameters: HashMap<String, StateValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPrecondition {
    pub state_key: String,
    pub condition: ComparisonOperator,
    pub expected_value: StateValue,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEffect {
    pub state_key: String,
    pub value: StateValue,
    pub probability: f64,
    pub delay: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionCost {
    pub base_cost: f64,
    pub resource_costs: HashMap<String, f64>,
    pub time_cost: f64,
    pub energy_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionCategory {
    Movement,
    Interaction,
    Communication,
    Resource,
    Combat,
    Utility,
    Custom(String),
}

impl Action {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: description.to_string(),
            preconditions: Vec::new(),
            effects: Vec::new(),
            cost: ActionCost::default(),
            duration: 1.0,
            category: ActionCategory::Utility,
            parameters: HashMap::new(),
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn with_category(mut self, category: ActionCategory) -> Self {
        self.category = category;
        self
    }

    pub fn with_duration(mut self, duration: f64) -> Self {
        self.duration = duration;
        self
    }

    pub fn add_precondition(mut self, state_key: &str, condition: ComparisonOperator, expected_value: StateValue) -> Self {
        self.preconditions.push(ActionPrecondition {
            state_key: state_key.to_string(),
            condition,
            expected_value,
            required: true,
        });
        self
    }

    pub fn add_optional_precondition(mut self, state_key: &str, condition: ComparisonOperator, expected_value: StateValue) -> Self {
        self.preconditions.push(ActionPrecondition {
            state_key: state_key.to_string(),
            condition,
            expected_value,
            required: false,
        });
        self
    }

    pub fn add_effect(mut self, state_key: &str, value: StateValue) -> Self {
        self.effects.push(ActionEffect {
            state_key: state_key.to_string(),
            value,
            probability: 1.0,
            delay: None,
        });
        self
    }

    pub fn add_probabilistic_effect(mut self, state_key: &str, value: StateValue, probability: f64) -> Self {
        self.effects.push(ActionEffect {
            state_key: state_key.to_string(),
            value,
            probability,
            delay: None,
        });
        self
    }

    pub fn add_delayed_effect(mut self, state_key: &str, value: StateValue, delay: f64) -> Self {
        self.effects.push(ActionEffect {
            state_key: state_key.to_string(),
            value,
            probability: 1.0,
            delay: Some(delay),
        });
        self
    }

    pub fn with_cost(mut self, base_cost: f64) -> Self {
        self.cost.base_cost = base_cost;
        self
    }

    pub fn add_resource_cost(mut self, resource: &str, cost: f64) -> Self {
        self.cost.resource_costs.insert(resource.to_string(), cost);
        self
    }

    pub fn with_time_cost(mut self, time_cost: f64) -> Self {
        self.cost.time_cost = time_cost;
        self
    }

    pub fn with_energy_cost(mut self, energy_cost: f64) -> Self {
        self.cost.energy_cost = energy_cost;
        self
    }

    pub fn add_parameter(mut self, key: &str, value: StateValue) -> Self {
        self.parameters.insert(key.to_string(), value);
        self
    }

    pub fn can_execute(&self, world_state: &WorldState) -> bool {
        // Check all required preconditions
        for precondition in &self.preconditions {
            if precondition.required && !self.check_precondition(precondition, world_state) {
                return false;
            }
        }
        true
    }

    pub fn get_execution_score(&self, world_state: &WorldState) -> f64 {
        let mut score = 1.0;

        // Score based on satisfied preconditions
        let satisfied_preconditions = self.preconditions
            .iter()
            .filter(|p| self.check_precondition(p, world_state))
            .count();

        let total_preconditions = self.preconditions.len().max(1);
        let precondition_score = satisfied_preconditions as f64 / total_preconditions as f64;
        score *= precondition_score;

        // Factor in cost (lower cost = higher score)
        let cost_factor = 1.0 / (1.0 + self.get_total_cost(world_state));
        score *= cost_factor;

        // Factor in number of effects (more effects = potentially more useful)
        let effect_bonus = 1.0 + (self.effects.len() as f64 * 0.1);
        score *= effect_bonus;

        score
    }

    fn check_precondition(&self, precondition: &ActionPrecondition, world_state: &WorldState) -> bool {
        let condition = StateCondition {
            key: precondition.state_key.clone(),
            operator: precondition.condition.clone(),
            value: precondition.expected_value.clone(),
        };
        condition.evaluate(world_state)
    }

    pub fn apply_effects(&self, world_state: &mut WorldState) -> Vec<EffectApplication> {
        let mut applied_effects = Vec::new();

        for effect in &self.effects {
            // Check probability
            if effect.probability < 1.0 {
                let random_value: f64 = rand::random();
                if random_value > effect.probability {
                    applied_effects.push(EffectApplication {
                        effect: effect.clone(),
                        applied: false,
                        reason: Some("Probability check failed".to_string()),
                    });
                    continue;
                }
            }

            // Apply effect (ignoring delay for immediate application)
            let old_value = world_state.get_state(&effect.state_key).cloned();
            world_state.set_state(&effect.state_key, effect.value.clone());

            applied_effects.push(EffectApplication {
                effect: effect.clone(),
                applied: true,
                reason: old_value.map(|v| format!("Changed from {:?}", v)),
            });
        }

        applied_effects
    }

    pub fn predict_state_after_execution(&self, world_state: &WorldState) -> WorldState {
        let mut predicted_state = world_state.clone();

        for effect in &self.effects {
            if effect.probability >= 0.5 {  // Only apply likely effects in prediction
                predicted_state.set_state(&effect.state_key, effect.value.clone());
            }
        }

        predicted_state
    }

    pub fn get_total_cost(&self, world_state: &WorldState) -> f64 {
        let mut total_cost = self.cost.base_cost + self.cost.time_cost + self.cost.energy_cost;

        // Add resource costs
        for (_resource, cost) in &self.cost.resource_costs {
            total_cost += cost;
        }

        // Factor in current world state for dynamic costs
        if let Some(multiplier) = world_state.get_state("cost_multiplier") {
            if let Some(mult_value) = multiplier.as_float() {
                total_cost *= mult_value;
            }
        }

        total_cost.max(0.0)
    }

    pub fn get_missing_preconditions(&self, world_state: &WorldState) -> Vec<&ActionPrecondition> {
        self.preconditions
            .iter()
            .filter(|p| p.required && !self.check_precondition(p, world_state))
            .collect()
    }

    pub fn estimate_success_probability(&self, world_state: &WorldState) -> f64 {
        if !self.can_execute(world_state) {
            return 0.0;
        }

        // Base probability starts at 1.0
        let mut probability = 1.0;

        // Factor in precondition satisfaction
        let satisfied_optional = self.preconditions
            .iter()
            .filter(|p| !p.required)
            .filter(|p| self.check_precondition(p, world_state))
            .count();

        let total_optional = self.preconditions
            .iter()
            .filter(|p| !p.required)
            .count();

        if total_optional > 0 {
            let optional_bonus = satisfied_optional as f64 / total_optional as f64;
            probability *= 0.8 + (optional_bonus * 0.2);  // Bonus for satisfied optional preconditions
        }

        // Factor in effect probabilities
        if !self.effects.is_empty() {
            let avg_effect_probability = self.effects
                .iter()
                .map(|e| e.probability)
                .sum::<f64>() / self.effects.len() as f64;
            probability *= avg_effect_probability;
        }

        probability.min(1.0).max(0.0)
    }

    pub fn get_category_name(&self) -> String {
        match &self.category {
            ActionCategory::Movement => "Movement".to_string(),
            ActionCategory::Interaction => "Interaction".to_string(),
            ActionCategory::Communication => "Communication".to_string(),
            ActionCategory::Resource => "Resource".to_string(),
            ActionCategory::Combat => "Combat".to_string(),
            ActionCategory::Utility => "Utility".to_string(),
            ActionCategory::Custom(name) => name.clone(),
        }
    }
}

impl ActionCost {
    pub fn new(base_cost: f64) -> Self {
        Self {
            base_cost,
            resource_costs: HashMap::new(),
            time_cost: 0.0,
            energy_cost: 0.0,
        }
    }

    pub fn zero() -> Self {
        Self::new(0.0)
    }
}

impl Default for ActionCost {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectApplication {
    pub effect: ActionEffect,
    pub applied: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionExecution {
    pub action_id: String,
    pub start_time: u64,
    pub duration: f64,
    pub success: bool,
    pub applied_effects: Vec<EffectApplication>,
    pub cost_paid: f64,
    pub world_state_before: WorldState,
    pub world_state_after: WorldState,
}

// Action template system for creating common actions
pub struct ActionTemplate {
    pub name: String,
    pub description: String,
    pub category: ActionCategory,
    pub base_cost: f64,
    pub duration: f64,
}

impl ActionTemplate {
    pub fn movement(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            category: ActionCategory::Movement,
            base_cost: 1.0,
            duration: 1.0,
        }
    }

    pub fn interaction(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            category: ActionCategory::Interaction,
            base_cost: 2.0,
            duration: 2.0,
        }
    }

    pub fn resource(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            category: ActionCategory::Resource,
            base_cost: 1.5,
            duration: 3.0,
        }
    }

    pub fn build(self) -> Action {
        Action::new(&self.name, &self.description)
            .with_category(self.category)
            .with_cost(self.base_cost)
            .with_duration(self.duration)
    }
}

// Common action builders
pub struct CommonActions;

impl CommonActions {
    pub fn move_to(location: &str) -> Action {
        Action::new("move_to", &format!("Move to {}", location))
            .with_category(ActionCategory::Movement)
            .add_precondition("can_move", ComparisonOperator::Equal, StateValue::Boolean(true))
            .add_effect("location", StateValue::String(location.to_string()))
            .add_effect("moved", StateValue::Boolean(true))
            .with_cost(1.0)
            .with_time_cost(1.0)
    }

    pub fn pick_up_item(item: &str) -> Action {
        Action::new("pick_up", &format!("Pick up {}", item))
            .with_category(ActionCategory::Interaction)
            .add_precondition("hands_free", ComparisonOperator::Equal, StateValue::Boolean(true))
            .add_precondition("item_available", ComparisonOperator::Equal, StateValue::Boolean(true))
            .add_effect("holding", StateValue::String(item.to_string()))
            .add_effect("hands_free", StateValue::Boolean(false))
            .with_cost(0.5)
    }

    pub fn use_item(item: &str) -> Action {
        Action::new("use_item", &format!("Use {}", item))
            .with_category(ActionCategory::Utility)
            .add_precondition("holding", ComparisonOperator::Equal, StateValue::String(item.to_string()))
            .add_effect("item_used", StateValue::Boolean(true))
            .with_cost(1.0)
    }

    pub fn talk_to(person: &str) -> Action {
        Action::new("talk_to", &format!("Talk to {}", person))
            .with_category(ActionCategory::Communication)
            .add_precondition("can_speak", ComparisonOperator::Equal, StateValue::Boolean(true))
            .add_effect("talked_to", StateValue::String(person.to_string()))
            .add_effect("conversation_active", StateValue::Boolean(true))
            .with_cost(0.5)
            .with_time_cost(2.0)
    }

    pub fn wait(duration: f64) -> Action {
        Action::new("wait", &format!("Wait for {} seconds", duration))
            .with_category(ActionCategory::Utility)
            .add_effect("time_passed", StateValue::Float(duration))
            .with_cost(0.1)
            .with_time_cost(duration)
            .with_duration(duration)
    }

    pub fn rest() -> Action {
        Action::new("rest", "Rest to recover energy")
            .with_category(ActionCategory::Utility)
            .add_effect("energy", StateValue::Integer(100))
            .add_effect("rested", StateValue::Boolean(true))
            .with_cost(0.0)
            .with_time_cost(5.0)
            .with_duration(5.0)
    }
}