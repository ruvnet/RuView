use wasm_bindgen::prelude::*;

pub mod state;
pub mod action;
pub mod goal;
pub mod planner;
pub mod rules;
pub mod astar;

pub use state::{StateValue, WorldState};
pub use action::{Action, ActionCost, ActionEffect, ActionPrecondition};
pub use goal::{Goal, GoalCondition, GoalPriority, GoalState};
pub use planner::{GOAPPlanner, Plan, PlanningResult, PlanStep};
pub use rules::{DecisionRule, RuleCondition, RuleEngine};
pub use astar::{AStarSearch, SearchNode, SearchResult};

// WASM bindings
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct PlannerSystem {
    planner: GOAPPlanner,
    world_state: WorldState,
    rule_engine: RuleEngine,
}

#[wasm_bindgen]
impl PlannerSystem {
    #[wasm_bindgen(constructor)]
    pub fn new() -> PlannerSystem {
        PlannerSystem {
            planner: GOAPPlanner::new(),
            world_state: WorldState::new(),
            rule_engine: RuleEngine::new(),
        }
    }

    #[wasm_bindgen]
    pub fn set_state(&mut self, key: &str, value: &str) -> bool {
        match serde_json::from_str::<StateValue>(value) {
            Ok(state_value) => {
                self.world_state.set_state(key, state_value);
                true
            }
            Err(_) => false,
        }
    }

    #[wasm_bindgen]
    pub fn get_state(&self, key: &str) -> String {
        match self.world_state.get_state(key) {
            Some(value) => serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
        }
    }

    #[wasm_bindgen]
    pub fn add_action(&mut self, action_json: &str) -> bool {
        match serde_json::from_str::<Action>(action_json) {
            Ok(action) => {
                self.planner.add_action(action);
                true
            }
            Err(_) => false,
        }
    }

    #[wasm_bindgen]
    pub fn add_goal(&mut self, goal_json: &str) -> bool {
        match serde_json::from_str::<Goal>(goal_json) {
            Ok(goal) => {
                self.planner.add_goal(goal);
                true
            }
            Err(_) => false,
        }
    }

    #[wasm_bindgen]
    pub fn plan(&self, goal_id: &str) -> String {
        let result = self.planner.plan(&self.world_state, goal_id);
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
    }

    #[wasm_bindgen]
    pub fn plan_to_state(&self, target_state_json: &str) -> String {
        match serde_json::from_str::<WorldState>(target_state_json) {
            Ok(target_state) => {
                let result = self.planner.plan_to_state(&self.world_state, &target_state);
                serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
            }
            Err(e) => format!("{{\"error\": \"{}\"}}", e),
        }
    }

    #[wasm_bindgen]
    pub fn execute_plan(&mut self, plan_json: &str) -> String {
        match serde_json::from_str::<Plan>(plan_json) {
            Ok(plan) => {
                let result = self.execute_plan_steps(&plan);
                serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
            }
            Err(e) => format!("{{\"error\": \"{}\"}}", e),
        }
    }

    #[wasm_bindgen]
    pub fn add_rule(&mut self, rule_json: &str) -> bool {
        match serde_json::from_str::<DecisionRule>(rule_json) {
            Ok(rule) => {
                self.rule_engine.add_rule(rule);
                true
            }
            Err(_) => false,
        }
    }

    #[wasm_bindgen]
    pub fn evaluate_rules(&self) -> String {
        let decisions = self.rule_engine.evaluate(&self.world_state);
        serde_json::to_string(&decisions).unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen]
    pub fn get_world_state(&self) -> String {
        serde_json::to_string(&self.world_state).unwrap_or_else(|_| "{}".to_string())
    }

    #[wasm_bindgen]
    pub fn get_available_actions(&self) -> String {
        let actions = self.planner.get_available_actions(&self.world_state);
        serde_json::to_string(&actions).unwrap_or_else(|_| "[]".to_string())
    }
}

impl PlannerSystem {
    fn execute_plan_steps(&mut self, plan: &Plan) -> ExecutionResult {
        let mut executed_steps = Vec::new();
        let mut current_state = self.world_state.clone();
        let mut total_cost = 0.0;

        for step in &plan.steps {
            if let Some(action) = self.planner.get_action(&step.action_id) {
                // Check if action can still be executed
                if action.can_execute(&current_state) {
                    // Apply action effects
                    for effect in &action.effects {
                        current_state.set_state(&effect.state_key, effect.value.clone());
                    }

                    executed_steps.push(step.clone());
                    total_cost += action.cost.base_cost;
                } else {
                    return ExecutionResult {
                        success: false,
                        executed_steps,
                        final_state: current_state,
                        total_cost,
                        error: Some(format!("Action '{}' cannot be executed", step.action_id)),
                    };
                }
            } else {
                return ExecutionResult {
                    success: false,
                    executed_steps,
                    final_state: current_state,
                    total_cost,
                    error: Some(format!("Action '{}' not found", step.action_id)),
                };
            }
        }

        // Update world state
        self.world_state = current_state.clone();

        ExecutionResult {
            success: true,
            executed_steps,
            final_state: current_state,
            total_cost,
            error: None,
        }
    }
}

impl Default for PlannerSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub executed_steps: Vec<PlanStep>,
    pub final_state: WorldState,
    pub total_cost: f64,
    pub error: Option<String>,
}