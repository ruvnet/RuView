use crate::state::{StateValue, WorldState, StateCondition, ComparisonOperator};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub name: String,
    pub description: String,
    pub conditions: Vec<GoalCondition>,
    pub priority: GoalPriority,
    pub deadline: Option<u64>,
    pub reward: f64,
    pub state: GoalState,
    pub parent_goal_id: Option<String>,
    pub sub_goals: Vec<String>,
    pub metadata: HashMap<String, StateValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalCondition {
    pub state_key: String,
    pub operator: ComparisonOperator,
    pub target_value: StateValue,
    pub weight: f64,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoalPriority {
    Low,
    Medium,
    High,
    Critical,
    Custom(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoalState {
    Pending,
    Active,
    InProgress,
    Completed,
    Failed,
    Cancelled,
    OnHold,
}

impl Goal {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: description.to_string(),
            conditions: Vec::new(),
            priority: GoalPriority::Medium,
            deadline: None,
            reward: 1.0,
            state: GoalState::Pending,
            parent_goal_id: None,
            sub_goals: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn with_priority(mut self, priority: GoalPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_deadline(mut self, deadline: u64) -> Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn with_reward(mut self, reward: f64) -> Self {
        self.reward = reward;
        self
    }

    pub fn with_state(mut self, state: GoalState) -> Self {
        self.state = state;
        self
    }

    pub fn with_parent(mut self, parent_id: &str) -> Self {
        self.parent_goal_id = Some(parent_id.to_string());
        self
    }

    pub fn add_condition(mut self, state_key: &str, operator: ComparisonOperator, target_value: StateValue) -> Self {
        self.conditions.push(GoalCondition {
            state_key: state_key.to_string(),
            operator,
            target_value,
            weight: 1.0,
            required: true,
        });
        self
    }

    pub fn add_weighted_condition(mut self, state_key: &str, operator: ComparisonOperator, target_value: StateValue, weight: f64) -> Self {
        self.conditions.push(GoalCondition {
            state_key: state_key.to_string(),
            operator,
            target_value,
            weight,
            required: true,
        });
        self
    }

    pub fn add_optional_condition(mut self, state_key: &str, operator: ComparisonOperator, target_value: StateValue) -> Self {
        self.conditions.push(GoalCondition {
            state_key: state_key.to_string(),
            operator,
            target_value,
            weight: 1.0,
            required: false,
        });
        self
    }

    pub fn add_sub_goal(mut self, sub_goal_id: &str) -> Self {
        self.sub_goals.push(sub_goal_id.to_string());
        self
    }

    pub fn add_metadata(mut self, key: &str, value: StateValue) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }

    pub fn is_satisfied(&self, world_state: &WorldState) -> bool {
        // All required conditions must be satisfied
        let required_satisfied = self.conditions
            .iter()
            .filter(|c| c.required)
            .all(|condition| self.check_condition(condition, world_state));

        required_satisfied
    }

    pub fn get_satisfaction_score(&self, world_state: &WorldState) -> f64 {
        if self.conditions.is_empty() {
            return 1.0;
        }

        let mut total_score = 0.0;
        let mut total_weight = 0.0;

        for condition in &self.conditions {
            let satisfied = self.check_condition(condition, world_state);
            let score = if satisfied { 1.0 } else { 0.0 };

            total_score += score * condition.weight;
            total_weight += condition.weight;
        }

        if total_weight > 0.0 {
            total_score / total_weight
        } else {
            0.0
        }
    }

    pub fn get_completion_percentage(&self, world_state: &WorldState) -> f64 {
        self.get_satisfaction_score(world_state) * 100.0
    }

    fn check_condition(&self, condition: &GoalCondition, world_state: &WorldState) -> bool {
        let state_condition = StateCondition {
            key: condition.state_key.clone(),
            operator: condition.operator.clone(),
            value: condition.target_value.clone(),
        };
        state_condition.evaluate(world_state)
    }

    pub fn get_unsatisfied_conditions(&self, world_state: &WorldState) -> Vec<&GoalCondition> {
        self.conditions
            .iter()
            .filter(|condition| !self.check_condition(condition, world_state))
            .collect()
    }

    pub fn get_distance_to_goal(&self, world_state: &WorldState) -> f64 {
        let mut total_distance = 0.0;
        let mut condition_count = 0;

        for condition in &self.conditions {
            if let Some(current_value) = world_state.get_state(&condition.state_key) {
                let distance = self.calculate_value_distance(current_value, &condition.target_value);
                total_distance += distance * condition.weight;
                condition_count += 1;
            } else {
                // State doesn't exist - maximum distance
                total_distance += condition.weight;
                condition_count += 1;
            }
        }

        if condition_count > 0 {
            total_distance / condition_count as f64
        } else {
            0.0
        }
    }

    fn calculate_value_distance(&self, current: &StateValue, target: &StateValue) -> f64 {
        match (current, target) {
            (StateValue::Boolean(a), StateValue::Boolean(b)) => {
                if a == b { 0.0 } else { 1.0 }
            }
            (StateValue::Integer(a), StateValue::Integer(b)) => {
                ((*a - *b).abs() as f64).min(100.0) / 100.0
            }
            (StateValue::Float(a), StateValue::Float(b)) => {
                ((a - b).abs()).min(100.0) / 100.0
            }
            (StateValue::String(a), StateValue::String(b)) => {
                if a == b { 0.0 } else { 1.0 }
            }
            _ => 1.0, // Different types or complex types
        }
    }

    pub fn get_priority_value(&self) -> f64 {
        match &self.priority {
            GoalPriority::Low => 1.0,
            GoalPriority::Medium => 2.0,
            GoalPriority::High => 3.0,
            GoalPriority::Critical => 5.0,
            GoalPriority::Custom(value) => *value,
        }
    }

    pub fn is_expired(&self, current_time: u64) -> bool {
        match self.deadline {
            Some(deadline) => current_time > deadline,
            None => false,
        }
    }

    pub fn get_urgency_factor(&self, current_time: u64) -> f64 {
        match self.deadline {
            Some(deadline) => {
                if current_time >= deadline {
                    return 0.0; // Expired
                }

                let time_remaining = deadline - current_time;
                let base_urgency = 1.0 / (1.0 + time_remaining as f64 / 3600.0); // Urgency increases as deadline approaches
                base_urgency * self.get_priority_value()
            }
            None => self.get_priority_value(),
        }
    }

    pub fn can_start(&self, world_state: &WorldState) -> bool {
        // Goal can start if it's pending and any prerequisites are met
        matches!(self.state, GoalState::Pending) && self.check_prerequisites(world_state)
    }

    fn check_prerequisites(&self, world_state: &WorldState) -> bool {
        // Check if there are any blocking conditions
        if let Some(prerequisites) = self.metadata.get("prerequisites") {
            if let Some(prereq_list) = prerequisites.as_list() {
                for prereq in prereq_list {
                    if let Some(prereq_str) = prereq.as_string() {
                        if !world_state.has_state(prereq_str) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    pub fn update_state(&mut self, new_state: GoalState) {
        self.state = new_state;
    }

    pub fn add_progress_metadata(&mut self, key: &str, value: StateValue) {
        self.metadata.insert(format!("progress_{}", key), value);
    }

    pub fn get_estimated_cost(&self, world_state: &WorldState) -> f64 {
        // Simple cost estimation based on distance to goal
        let distance = self.get_distance_to_goal(world_state);
        let base_cost = distance * 10.0; // Base cost factor

        // Adjust for priority
        let priority_factor = match self.priority {
            GoalPriority::Low => 0.5,
            GoalPriority::Medium => 1.0,
            GoalPriority::High => 1.5,
            GoalPriority::Critical => 2.0,
            GoalPriority::Custom(value) => value / 2.0,
        };

        base_cost * priority_factor
    }
}

impl Default for Goal {
    fn default() -> Self {
        Self::new("Default Goal", "A default goal")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalManager {
    goals: HashMap<String, Goal>,
    active_goals: Vec<String>,
    completed_goals: Vec<String>,
    failed_goals: Vec<String>,
}

impl GoalManager {
    pub fn new() -> Self {
        Self {
            goals: HashMap::new(),
            active_goals: Vec::new(),
            completed_goals: Vec::new(),
            failed_goals: Vec::new(),
        }
    }

    pub fn add_goal(&mut self, goal: Goal) {
        let goal_id = goal.id.clone();
        self.goals.insert(goal_id.clone(), goal);

        if matches!(self.goals[&goal_id].state, GoalState::Active | GoalState::InProgress) {
            self.active_goals.push(goal_id);
        }
    }

    pub fn get_goal(&self, goal_id: &str) -> Option<&Goal> {
        self.goals.get(goal_id)
    }

    pub fn get_goal_mut(&mut self, goal_id: &str) -> Option<&mut Goal> {
        self.goals.get_mut(goal_id)
    }

    pub fn update_goal_state(&mut self, goal_id: &str, new_state: GoalState) -> bool {
        if let Some(goal) = self.goals.get_mut(goal_id) {
            let _old_state = std::mem::replace(&mut goal.state, new_state.clone());

            // Update tracking lists
            self.active_goals.retain(|id| id != goal_id);
            self.completed_goals.retain(|id| id != goal_id);
            self.failed_goals.retain(|id| id != goal_id);

            match new_state {
                GoalState::Active | GoalState::InProgress => {
                    self.active_goals.push(goal_id.to_string());
                }
                GoalState::Completed => {
                    self.completed_goals.push(goal_id.to_string());
                }
                GoalState::Failed => {
                    self.failed_goals.push(goal_id.to_string());
                }
                _ => {}
            }

            true
        } else {
            false
        }
    }

    pub fn get_active_goals(&self) -> Vec<&Goal> {
        self.active_goals
            .iter()
            .filter_map(|id| self.goals.get(id))
            .collect()
    }

    pub fn get_highest_priority_goal(&self, _world_state: &WorldState, current_time: u64) -> Option<&Goal> {
        self.get_active_goals()
            .into_iter()
            .filter(|goal| !goal.is_expired(current_time))
            .max_by(|a, b| {
                let a_urgency = a.get_urgency_factor(current_time);
                let b_urgency = b.get_urgency_factor(current_time);
                a_urgency.partial_cmp(&b_urgency).unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    pub fn update_goal_progress(&mut self, world_state: &WorldState) {
        let goal_ids: Vec<String> = self.goals.keys().cloned().collect();

        for goal_id in goal_ids {
            if let Some(goal) = self.goals.get_mut(&goal_id) {
                match goal.state {
                    GoalState::Active | GoalState::InProgress => {
                        if goal.is_satisfied(world_state) {
                            goal.state = GoalState::Completed;
                            self.update_goal_state(&goal_id, GoalState::Completed);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn get_goal_statistics(&self) -> GoalStatistics {
        let total_goals = self.goals.len();
        let active_count = self.active_goals.len();
        let completed_count = self.completed_goals.len();
        let failed_count = self.failed_goals.len();

        let completion_rate = if total_goals > 0 {
            completed_count as f64 / total_goals as f64
        } else {
            0.0
        };

        GoalStatistics {
            total_goals,
            active_goals: active_count,
            completed_goals: completed_count,
            failed_goals: failed_count,
            completion_rate,
        }
    }

    pub fn cleanup_expired_goals(&mut self, current_time: u64) {
        let expired_goal_ids: Vec<String> = self.goals
            .iter()
            .filter(|(_, goal)| goal.is_expired(current_time))
            .map(|(id, _)| id.clone())
            .collect();

        for goal_id in expired_goal_ids {
            self.update_goal_state(&goal_id, GoalState::Failed);
        }
    }
}

impl Default for GoalManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalStatistics {
    pub total_goals: usize,
    pub active_goals: usize,
    pub completed_goals: usize,
    pub failed_goals: usize,
    pub completion_rate: f64,
}

// Common goal builders
pub struct CommonGoals;

impl CommonGoals {
    pub fn reach_location(location: &str) -> Goal {
        Goal::new("reach_location", &format!("Reach location: {}", location))
            .add_condition("location", ComparisonOperator::Equal, StateValue::String(location.to_string()))
            .with_priority(GoalPriority::Medium)
            .with_reward(5.0)
    }

    pub fn collect_item(item: &str) -> Goal {
        Goal::new("collect_item", &format!("Collect item: {}", item))
            .add_condition("holding", ComparisonOperator::Equal, StateValue::String(item.to_string()))
            .with_priority(GoalPriority::Medium)
            .with_reward(3.0)
    }

    pub fn achieve_state(state_key: &str, target_value: StateValue) -> Goal {
        Goal::new("achieve_state", &format!("Achieve state: {} = {:?}", state_key, target_value))
            .add_condition(state_key, ComparisonOperator::Equal, target_value)
            .with_priority(GoalPriority::Medium)
            .with_reward(2.0)
    }

    pub fn maintain_resource(resource: &str, min_amount: i64) -> Goal {
        Goal::new("maintain_resource", &format!("Maintain {} >= {}", resource, min_amount))
            .add_condition(resource, ComparisonOperator::GreaterThanOrEqual, StateValue::Integer(min_amount))
            .with_priority(GoalPriority::High)
            .with_reward(1.0)
    }

    pub fn complete_sequence(sequence_name: &str, steps: Vec<String>) -> Goal {
        let mut goal = Goal::new("complete_sequence", &format!("Complete sequence: {}", sequence_name))
            .with_priority(GoalPriority::High)
            .with_reward(10.0);

        for (i, step) in steps.iter().enumerate() {
            goal = goal.add_condition(&format!("step_{}", i), ComparisonOperator::Equal, StateValue::String(step.clone()));
        }

        goal
    }
}