use crate::action::Action;
use crate::astar::{AStarSearch, SearchConstraints, SearchResult};
use crate::goal::{Goal, GoalManager, GoalState};
use crate::state::WorldState;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub goal_id: String,
    pub steps: Vec<PlanStep>,
    pub total_cost: f64,
    pub estimated_duration: f64,
    pub confidence: f64,
    pub created_at: u64,
    pub status: PlanStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub action_id: String,
    pub parameters: HashMap<String, String>,
    pub expected_cost: f64,
    pub expected_duration: f64,
    pub state_before: Option<WorldState>,
    pub state_after: Option<WorldState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanStatus {
    Created,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningResult {
    pub success: bool,
    pub plan: Option<Plan>,
    pub error: Option<String>,
    pub planning_time: f64,
    pub nodes_explored: usize,
    pub alternative_plans: Vec<Plan>,
}

#[derive(Debug)]
pub struct GOAPPlanner {
    actions: IndexMap<String, Action>,
    goals: IndexMap<String, Goal>,
    goal_manager: GoalManager,
    search_algorithm: AStarSearch,
    planning_constraints: SearchConstraints,
    max_alternative_plans: usize,
}

impl GOAPPlanner {
    pub fn new() -> Self {
        Self {
            actions: IndexMap::new(),
            goals: IndexMap::new(),
            goal_manager: GoalManager::new(),
            search_algorithm: AStarSearch::new(),
            planning_constraints: SearchConstraints::new(),
            max_alternative_plans: 3,
        }
    }

    pub fn with_search_constraints(mut self, constraints: SearchConstraints) -> Self {
        self.planning_constraints = constraints;
        self
    }

    pub fn with_max_alternatives(mut self, max_alternatives: usize) -> Self {
        self.max_alternative_plans = max_alternatives;
        self
    }

    pub fn add_action(&mut self, action: Action) {
        self.actions.insert(action.id.clone(), action);
    }

    pub fn remove_action(&mut self, action_id: &str) -> bool {
        self.actions.remove(action_id).is_some()
    }

    pub fn get_action(&self, action_id: &str) -> Option<&Action> {
        self.actions.get(action_id)
    }

    pub fn add_goal(&mut self, goal: Goal) {
        self.goal_manager.add_goal(goal.clone());
        self.goals.insert(goal.id.clone(), goal);
    }

    pub fn remove_goal(&mut self, goal_id: &str) -> bool {
        self.goals.remove(goal_id).is_some()
    }

    pub fn get_goal(&self, goal_id: &str) -> Option<&Goal> {
        self.goals.get(goal_id)
    }

    pub fn plan(&self, current_state: &WorldState, goal_id: &str) -> PlanningResult {
        let planning_start = std::time::Instant::now();

        let goal = match self.goals.get(goal_id) {
            Some(g) => g,
            None => {
                return PlanningResult {
                    success: false,
                    plan: None,
                    error: Some(format!("Goal {} not found", goal_id)),
                    planning_time: 0.0,
                    nodes_explored: 0,
                    alternative_plans: Vec::new(),
                };
            }
        };

        // Check if goal is already satisfied
        if goal.is_satisfied(current_state) {
            return PlanningResult {
                success: true,
                plan: Some(Plan {
                    id: Uuid::new_v4().to_string(),
                    goal_id: goal_id.to_string(),
                    steps: Vec::new(),
                    total_cost: 0.0,
                    estimated_duration: 0.0,
                    confidence: 1.0,
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    status: PlanStatus::Created,
                }),
                error: None,
                planning_time: planning_start.elapsed().as_secs_f64(),
                nodes_explored: 0,
                alternative_plans: Vec::new(),
            };
        }

        // Create target state from goal conditions
        let target_state = self.create_target_state_from_goal(goal, current_state);

        // Get available actions
        let available_actions: Vec<Action> = self.actions.values().cloned().collect();

        // Perform A* search
        let search_result = self.search_algorithm.search_with_constraints(
            current_state,
            &target_state,
            &available_actions,
            &self.planning_constraints,
        );

        let planning_time = planning_start.elapsed().as_secs_f64();

        if search_result.success {
            let plan = self.create_plan_from_search_result(goal_id, &search_result);
            let alternative_plans = self.generate_alternative_plans(current_state, &target_state, &available_actions);

            PlanningResult {
                success: true,
                plan: Some(plan),
                error: None,
                planning_time,
                nodes_explored: search_result.nodes_explored,
                alternative_plans,
            }
        } else {
            PlanningResult {
                success: false,
                plan: None,
                error: search_result.error,
                planning_time,
                nodes_explored: search_result.nodes_explored,
                alternative_plans: Vec::new(),
            }
        }
    }

    pub fn plan_to_state(&self, current_state: &WorldState, target_state: &WorldState) -> PlanningResult {
        let planning_start = std::time::Instant::now();

        let available_actions: Vec<Action> = self.actions.values().cloned().collect();

        let search_result = self.search_algorithm.search_with_constraints(
            current_state,
            target_state,
            &available_actions,
            &self.planning_constraints,
        );

        let planning_time = planning_start.elapsed().as_secs_f64();

        if search_result.success {
            let plan = self.create_plan_from_search_result("direct_state_goal", &search_result);
            let alternative_plans = self.generate_alternative_plans(current_state, target_state, &available_actions);

            PlanningResult {
                success: true,
                plan: Some(plan),
                error: None,
                planning_time,
                nodes_explored: search_result.nodes_explored,
                alternative_plans,
            }
        } else {
            PlanningResult {
                success: false,
                plan: None,
                error: search_result.error,
                planning_time,
                nodes_explored: search_result.nodes_explored,
                alternative_plans: Vec::new(),
            }
        }
    }

    pub fn replan(&self, current_state: &WorldState, existing_plan: &Plan, failed_step_index: usize) -> PlanningResult {
        // Get remaining goal from the original plan
        if let Some(_goal) = self.goals.get(&existing_plan.goal_id) {
            // Try to plan from current state to goal, potentially avoiding the failed action
            let mut modified_constraints = self.planning_constraints.clone();

            // Optionally avoid the failed action
            if failed_step_index < existing_plan.steps.len() {
                let failed_action_id = &existing_plan.steps[failed_step_index].action_id;
                modified_constraints = modified_constraints.forbid_action(failed_action_id);
            }

            let planner_with_constraints = self.clone().with_search_constraints(modified_constraints);
            planner_with_constraints.plan(current_state, &existing_plan.goal_id)
        } else {
            PlanningResult {
                success: false,
                plan: None,
                error: Some("Original goal not found for replanning".to_string()),
                planning_time: 0.0,
                nodes_explored: 0,
                alternative_plans: Vec::new(),
            }
        }
    }

    pub fn get_available_actions(&self, current_state: &WorldState) -> Vec<Action> {
        self.actions
            .values()
            .filter(|action| action.can_execute(current_state))
            .cloned()
            .collect()
    }

    pub fn get_best_next_action(&self, current_state: &WorldState, goal_id: &str) -> Option<&Action> {
        let goal = self.goals.get(goal_id)?;
        let target_state = self.create_target_state_from_goal(goal, current_state);

        let mut best_action = None;
        let mut best_score = f64::NEG_INFINITY;

        for action in self.actions.values() {
            if action.can_execute(current_state) {
                let score = self.evaluate_action_for_goal(action, current_state, &target_state);
                if score > best_score {
                    best_score = score;
                    best_action = Some(action);
                }
            }
        }

        best_action
    }

    fn create_target_state_from_goal(&self, goal: &Goal, current_state: &WorldState) -> WorldState {
        let mut target_state = current_state.clone();

        for condition in &goal.conditions {
            target_state.set_state(&condition.state_key, condition.target_value.clone());
        }

        target_state
    }

    fn create_plan_from_search_result(&self, goal_id: &str, search_result: &SearchResult) -> Plan {
        let mut steps = Vec::new();
        let mut total_duration = 0.0;

        for search_step in &search_result.path {
            if let Some(action) = self.actions.get(&search_step.action_id) {
                total_duration += action.duration;
            }

            steps.push(PlanStep {
                action_id: search_step.action_id.clone(),
                parameters: HashMap::new(),
                expected_cost: search_step.cost,
                expected_duration: self.actions
                    .get(&search_step.action_id)
                    .map(|a| a.duration)
                    .unwrap_or(1.0),
                state_before: Some(search_step.state_before.clone()),
                state_after: Some(search_step.state_after.clone()),
            });
        }

        let confidence = self.calculate_plan_confidence(&steps);

        Plan {
            id: Uuid::new_v4().to_string(),
            goal_id: goal_id.to_string(),
            steps,
            total_cost: search_result.total_cost,
            estimated_duration: total_duration,
            confidence,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            status: PlanStatus::Created,
        }
    }

    fn generate_alternative_plans(&self, current_state: &WorldState, target_state: &WorldState, available_actions: &[Action]) -> Vec<Plan> {
        let mut alternatives = Vec::new();

        // Generate plans with different constraints
        for i in 0..self.max_alternative_plans {
            let mut modified_search = self.search_algorithm.clone();
            modified_search = modified_search.with_max_iterations(5000 + i * 2000);

            let result = modified_search.search(current_state, target_state, available_actions);
            if result.success {
                let plan = self.create_plan_from_search_result("alternative", &result);
                alternatives.push(plan);
            }
        }

        alternatives
    }

    fn evaluate_action_for_goal(&self, action: &Action, current_state: &WorldState, target_state: &WorldState) -> f64 {
        let predicted_state = action.predict_state_after_execution(current_state);
        let distance_before = current_state.distance_to(target_state);
        let distance_after = predicted_state.distance_to(target_state);

        let progress = distance_before - distance_after;
        let cost_penalty = action.get_total_cost(current_state) / 10.0;

        progress - cost_penalty
    }

    fn calculate_plan_confidence(&self, steps: &[PlanStep]) -> f64 {
        if steps.is_empty() {
            return 1.0;
        }

        let mut total_confidence = 0.0;

        for step in steps {
            if let Some(action) = self.actions.get(&step.action_id) {
                // Base confidence from action properties
                let mut step_confidence = 0.8;

                // Adjust based on cost (lower cost = higher confidence)
                step_confidence *= 1.0 / (1.0 + action.cost.base_cost / 10.0);

                // Adjust based on number of preconditions (fewer = higher confidence)
                let precondition_factor = 1.0 / (1.0 + action.preconditions.len() as f64 / 5.0);
                step_confidence *= precondition_factor;

                total_confidence += step_confidence;
            } else {
                total_confidence += 0.1; // Very low confidence for unknown actions
            }
        }

        total_confidence / steps.len() as f64
    }

    pub fn optimize_plan(&self, plan: &Plan, current_state: &WorldState) -> Plan {
        let mut optimized_steps = Vec::new();
        let mut current_opt_state = current_state.clone();

        // Try to merge or optimize steps
        for step in &plan.steps {
            if let Some(action) = self.actions.get(&step.action_id) {
                if action.can_execute(&current_opt_state) {
                    optimized_steps.push(step.clone());
                    current_opt_state = action.predict_state_after_execution(&current_opt_state);
                } else {
                    // Try to find an alternative action that achieves similar results
                    if let Some(alternative) = self.find_alternative_action(action, &current_opt_state) {
                        let mut alt_step = step.clone();
                        alt_step.action_id = alternative.id.clone();
                        alt_step.expected_cost = alternative.get_total_cost(&current_opt_state);
                        alt_step.expected_duration = alternative.duration;
                        optimized_steps.push(alt_step);
                        current_opt_state = alternative.predict_state_after_execution(&current_opt_state);
                    }
                }
            }
        }

        let mut optimized_plan = plan.clone();
        optimized_plan.id = Uuid::new_v4().to_string();
        optimized_plan.steps = optimized_steps;
        optimized_plan.total_cost = optimized_plan.steps.iter().map(|s| s.expected_cost).sum();
        optimized_plan.estimated_duration = optimized_plan.steps.iter().map(|s| s.expected_duration).sum();
        optimized_plan.confidence = self.calculate_plan_confidence(&optimized_plan.steps);

        optimized_plan
    }

    fn find_alternative_action(&self, original_action: &Action, current_state: &WorldState) -> Option<&Action> {
        // Find actions that have similar effects
        for action in self.actions.values() {
            if action.id != original_action.id
                && action.can_execute(current_state)
                && self.actions_have_similar_effects(original_action, action) {
                return Some(action);
            }
        }
        None
    }

    fn actions_have_similar_effects(&self, action1: &Action, action2: &Action) -> bool {
        // Simple comparison - check if they modify some of the same state keys
        let keys1: std::collections::HashSet<&String> = action1.effects.iter().map(|e| &e.state_key).collect();
        let keys2: std::collections::HashSet<&String> = action2.effects.iter().map(|e| &e.state_key).collect();

        !keys1.is_disjoint(&keys2)
    }

    pub fn validate_plan(&self, plan: &Plan, current_state: &WorldState) -> PlanValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut current_validation_state = current_state.clone();

        for (i, step) in plan.steps.iter().enumerate() {
            if let Some(action) = self.actions.get(&step.action_id) {
                // Check if action can be executed in current state
                if !action.can_execute(&current_validation_state) {
                    errors.push(format!("Step {}: Action '{}' cannot be executed", i, action.name));
                }

                // Check for high-cost actions
                if action.cost.base_cost > 10.0 {
                    warnings.push(format!("Step {}: Action '{}' has high cost ({})", i, action.name, action.cost.base_cost));
                }

                // Apply action effects
                current_validation_state = action.predict_state_after_execution(&current_validation_state);
            } else {
                errors.push(format!("Step {}: Unknown action '{}'", i, step.action_id));
            }
        }

        let is_valid = errors.is_empty();
        let success_prob = if is_valid { plan.confidence } else { 0.0 };

        PlanValidationResult {
            is_valid,
            errors,
            warnings,
            estimated_success_probability: success_prob,
        }
    }
}

impl Clone for GOAPPlanner {
    fn clone(&self) -> Self {
        Self {
            actions: self.actions.clone(),
            goals: self.goals.clone(),
            goal_manager: GoalManager::new(), // Create new goal manager for clone
            search_algorithm: AStarSearch::new(),
            planning_constraints: self.planning_constraints.clone(),
            max_alternative_plans: self.max_alternative_plans,
        }
    }
}

impl Default for GOAPPlanner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub estimated_success_probability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanExecutionContext {
    pub plan_id: String,
    pub current_step_index: usize,
    pub start_time: u64,
    pub elapsed_time: f64,
    pub execution_log: Vec<StepExecutionResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionResult {
    pub step_index: usize,
    pub action_id: String,
    pub success: bool,
    pub actual_cost: f64,
    pub actual_duration: f64,
    pub error: Option<String>,
    pub state_after_execution: WorldState,
}

// Plan monitoring and adaptation
pub struct PlanMonitor {
    execution_contexts: HashMap<String, PlanExecutionContext>,
    success_threshold: f64,
    replan_threshold: f64,
}

impl PlanMonitor {
    pub fn new() -> Self {
        Self {
            execution_contexts: HashMap::new(),
            success_threshold: 0.8,
            replan_threshold: 0.5,
        }
    }

    pub fn start_execution(&mut self, plan: &Plan) -> String {
        let context_id = Uuid::new_v4().to_string();
        let context = PlanExecutionContext {
            plan_id: plan.id.clone(),
            current_step_index: 0,
            start_time: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            elapsed_time: 0.0,
            execution_log: Vec::new(),
        };

        self.execution_contexts.insert(context_id.clone(), context);
        context_id
    }

    pub fn record_step_execution(&mut self, context_id: &str, result: StepExecutionResult) -> bool {
        if let Some(context) = self.execution_contexts.get_mut(context_id) {
            context.execution_log.push(result);
            context.current_step_index += 1;
            true
        } else {
            false
        }
    }

    pub fn should_replan(&self, context_id: &str) -> bool {
        if let Some(context) = self.execution_contexts.get(context_id) {
            let success_rate = self.calculate_success_rate(context);
            success_rate < self.replan_threshold
        } else {
            false
        }
    }

    fn calculate_success_rate(&self, context: &PlanExecutionContext) -> f64 {
        if context.execution_log.is_empty() {
            return 1.0;
        }

        let successful_steps = context.execution_log.iter().filter(|r| r.success).count();
        successful_steps as f64 / context.execution_log.len() as f64
    }
}

impl Default for PlanMonitor {
    fn default() -> Self {
        Self::new()
    }
}