use crate::state::{StateValue, WorldState, StateCondition, LogicalOperator, ComparisonOperator};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub conditions: Vec<RuleCondition>,
    pub actions: Vec<RuleAction>,
    pub priority: u32,
    pub enabled: bool,
    pub execution_count: u32,
    pub last_executed: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleCondition {
    pub condition: StateCondition,
    pub weight: f64,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleAction {
    pub action_type: RuleActionType,
    pub parameters: HashMap<String, StateValue>,
    pub probability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleActionType {
    SetState { key: String, value: StateValue },
    ModifyState { key: String, operation: StateOperation },
    TriggerGoal { goal_id: String },
    ExecuteAction { action_id: String },
    SendMessage { recipient: String, message: String },
    LogEvent { level: LogLevel, message: String },
    Custom { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateOperation {
    Add(StateValue),
    Subtract(StateValue),
    Multiply(StateValue),
    Divide(StateValue),
    Append(StateValue),
    Remove(StateValue),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl DecisionRule {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: description.to_string(),
            conditions: Vec::new(),
            actions: Vec::new(),
            priority: 0,
            enabled: true,
            execution_count: 0,
            last_executed: None,
        }
    }

    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn add_condition(mut self, condition: StateCondition, weight: f64, required: bool) -> Self {
        self.conditions.push(RuleCondition {
            condition,
            weight,
            required,
        });
        self
    }

    pub fn add_simple_condition(mut self, key: &str, operator: ComparisonOperator, value: StateValue) -> Self {
        let condition = StateCondition::new(key, operator, value);
        self.conditions.push(RuleCondition {
            condition,
            weight: 1.0,
            required: true,
        });
        self
    }

    pub fn add_action(mut self, action: RuleAction) -> Self {
        self.actions.push(action);
        self
    }

    pub fn add_set_state_action(mut self, key: &str, value: StateValue) -> Self {
        self.actions.push(RuleAction {
            action_type: RuleActionType::SetState {
                key: key.to_string(),
                value,
            },
            parameters: HashMap::new(),
            probability: 1.0,
        });
        self
    }

    pub fn add_trigger_goal_action(mut self, goal_id: &str) -> Self {
        self.actions.push(RuleAction {
            action_type: RuleActionType::TriggerGoal {
                goal_id: goal_id.to_string(),
            },
            parameters: HashMap::new(),
            probability: 1.0,
        });
        self
    }

    pub fn add_log_action(mut self, level: LogLevel, message: &str) -> Self {
        self.actions.push(RuleAction {
            action_type: RuleActionType::LogEvent {
                level,
                message: message.to_string(),
            },
            parameters: HashMap::new(),
            probability: 1.0,
        });
        self
    }

    pub fn can_execute(&self, world_state: &WorldState) -> bool {
        if !self.enabled {
            return false;
        }

        // Check all required conditions
        for rule_condition in &self.conditions {
            if rule_condition.required && !rule_condition.condition.evaluate(world_state) {
                return false;
            }
        }

        true
    }

    pub fn get_execution_score(&self, world_state: &WorldState) -> f64 {
        if !self.can_execute(world_state) {
            return 0.0;
        }

        let mut total_score = 0.0;
        let mut total_weight = 0.0;

        for rule_condition in &self.conditions {
            let satisfied = rule_condition.condition.evaluate(world_state);
            let score = if satisfied { 1.0 } else { 0.0 };

            total_score += score * rule_condition.weight;
            total_weight += rule_condition.weight;
        }

        let base_score = if total_weight > 0.0 {
            total_score / total_weight
        } else {
            1.0
        };

        // Factor in priority
        let priority_factor = 1.0 + (self.priority as f64 / 100.0);

        base_score * priority_factor
    }

    pub fn execute(&mut self, world_state: &mut WorldState) -> RuleExecutionResult {
        let mut executed_actions = Vec::new();
        let mut successful_actions = 0;

        for action in &self.actions {
            // Check probability
            if action.probability < 1.0 {
                let random_value: f64 = rand::random();
                if random_value > action.probability {
                    continue;
                }
            }

            let result = self.execute_action(action, world_state);
            executed_actions.push(result.clone());

            if result.success {
                successful_actions += 1;
            }
        }

        // Update execution statistics
        self.execution_count += 1;
        self.last_executed = Some(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs());

        RuleExecutionResult {
            rule_id: self.id.clone(),
            success: successful_actions > 0,
            executed_actions,
            total_actions: self.actions.len(),
            successful_actions,
        }
    }

    fn execute_action(&self, action: &RuleAction, world_state: &mut WorldState) -> ActionExecutionResult {
        match &action.action_type {
            RuleActionType::SetState { key, value } => {
                world_state.set_state(key, value.clone());
                ActionExecutionResult {
                    action_type: "set_state".to_string(),
                    success: true,
                    message: Some(format!("Set {} to {:?}", key, value)),
                    error: None,
                }
            }
            RuleActionType::ModifyState { key, operation } => {
                self.execute_modify_state(key, operation, world_state)
            }
            RuleActionType::TriggerGoal { goal_id } => {
                // In a real implementation, this would trigger goal planning
                ActionExecutionResult {
                    action_type: "trigger_goal".to_string(),
                    success: true,
                    message: Some(format!("Triggered goal: {}", goal_id)),
                    error: None,
                }
            }
            RuleActionType::ExecuteAction { action_id } => {
                // In a real implementation, this would execute the specified action
                ActionExecutionResult {
                    action_type: "execute_action".to_string(),
                    success: true,
                    message: Some(format!("Executed action: {}", action_id)),
                    error: None,
                }
            }
            RuleActionType::SendMessage { recipient, message } => {
                // In a real implementation, this would send a message
                ActionExecutionResult {
                    action_type: "send_message".to_string(),
                    success: true,
                    message: Some(format!("Sent message to {}: {}", recipient, message)),
                    error: None,
                }
            }
            RuleActionType::LogEvent { level, message } => {
                // In a real implementation, this would log to a proper logging system
                println!("[{:?}] {}", level, message);
                ActionExecutionResult {
                    action_type: "log_event".to_string(),
                    success: true,
                    message: Some(format!("Logged: {}", message)),
                    error: None,
                }
            }
            RuleActionType::Custom { name } => {
                ActionExecutionResult {
                    action_type: "custom".to_string(),
                    success: false,
                    message: None,
                    error: Some(format!("Custom action '{}' not implemented", name)),
                }
            }
        }
    }

    fn execute_modify_state(&self, key: &str, operation: &StateOperation, world_state: &mut WorldState) -> ActionExecutionResult {
        let current_value = world_state.get_state(key);

        match (current_value, operation) {
            (Some(StateValue::Integer(current)), StateOperation::Add(StateValue::Integer(add_val))) => {
                world_state.set_state(key, StateValue::Integer(current + add_val));
                ActionExecutionResult {
                    action_type: "modify_state".to_string(),
                    success: true,
                    message: Some(format!("Added {} to {}", add_val, key)),
                    error: None,
                }
            }
            (Some(StateValue::Float(current)), StateOperation::Add(StateValue::Float(add_val))) => {
                world_state.set_state(key, StateValue::Float(current + add_val));
                ActionExecutionResult {
                    action_type: "modify_state".to_string(),
                    success: true,
                    message: Some(format!("Added {} to {}", add_val, key)),
                    error: None,
                }
            }
            (Some(StateValue::Integer(current)), StateOperation::Subtract(StateValue::Integer(sub_val))) => {
                world_state.set_state(key, StateValue::Integer(current - sub_val));
                ActionExecutionResult {
                    action_type: "modify_state".to_string(),
                    success: true,
                    message: Some(format!("Subtracted {} from {}", sub_val, key)),
                    error: None,
                }
            }
            (Some(StateValue::List(current)), StateOperation::Append(value)) => {
                let mut new_list = current.clone();
                new_list.push(value.clone());
                world_state.set_state(key, StateValue::List(new_list));
                ActionExecutionResult {
                    action_type: "modify_state".to_string(),
                    success: true,
                    message: Some(format!("Appended {:?} to {}", value, key)),
                    error: None,
                }
            }
            _ => {
                ActionExecutionResult {
                    action_type: "modify_state".to_string(),
                    success: false,
                    message: None,
                    error: Some(format!("Cannot apply operation {:?} to state {}", operation, key)),
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct RuleEngine {
    rules: Vec<DecisionRule>,
    execution_history: Vec<RuleExecutionResult>,
    max_history_size: usize,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            execution_history: Vec::new(),
            max_history_size: 1000,
        }
    }

    pub fn with_max_history(mut self, max_size: usize) -> Self {
        self.max_history_size = max_size;
        self
    }

    pub fn add_rule(&mut self, rule: DecisionRule) {
        self.rules.push(rule);
        self.sort_rules_by_priority();
    }

    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        if let Some(pos) = self.rules.iter().position(|r| r.id == rule_id) {
            self.rules.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn get_rule(&self, rule_id: &str) -> Option<&DecisionRule> {
        self.rules.iter().find(|r| r.id == rule_id)
    }

    pub fn get_rule_mut(&mut self, rule_id: &str) -> Option<&mut DecisionRule> {
        self.rules.iter_mut().find(|r| r.id == rule_id)
    }

    pub fn enable_rule(&mut self, rule_id: &str) -> bool {
        if let Some(rule) = self.get_rule_mut(rule_id) {
            rule.enabled = true;
            true
        } else {
            false
        }
    }

    pub fn disable_rule(&mut self, rule_id: &str) -> bool {
        if let Some(rule) = self.get_rule_mut(rule_id) {
            rule.enabled = false;
            true
        } else {
            false
        }
    }

    pub fn evaluate(&self, world_state: &WorldState) -> Vec<DecisionRecommendation> {
        let mut recommendations = Vec::new();

        for rule in &self.rules {
            if rule.can_execute(world_state) {
                let score = rule.get_execution_score(world_state);
                recommendations.push(DecisionRecommendation {
                    rule_id: rule.id.clone(),
                    rule_name: rule.name.clone(),
                    score,
                    confidence: self.calculate_rule_confidence(rule, world_state),
                    reason: self.generate_rule_reason(rule, world_state),
                });
            }
        }

        // Sort by score (highest first)
        recommendations.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        recommendations
    }

    pub fn execute_all_applicable(&mut self, world_state: &mut WorldState) -> Vec<RuleExecutionResult> {
        let mut results = Vec::new();

        // Clone rules to avoid borrowing issues
        let applicable_rules: Vec<(usize, f64)> = self.rules
            .iter()
            .enumerate()
            .filter_map(|(i, rule)| {
                if rule.can_execute(world_state) {
                    Some((i, rule.get_execution_score(world_state)))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (highest first)
        let mut sorted_applicable: Vec<(usize, f64)> = applicable_rules;
        sorted_applicable.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Execute rules in priority order
        for (rule_index, _score) in sorted_applicable {
            if let Some(rule) = self.rules.get_mut(rule_index) {
                let result = rule.execute(world_state);
                results.push(result);
            }
        }

        // Update execution history
        for result in &results {
            self.execution_history.push(result.clone());
        }

        // Trim history if needed
        if self.execution_history.len() > self.max_history_size {
            let excess = self.execution_history.len() - self.max_history_size;
            self.execution_history.drain(0..excess);
        }

        results
    }

    pub fn execute_rule(&mut self, rule_id: &str, world_state: &mut WorldState) -> Option<RuleExecutionResult> {
        if let Some(rule_index) = self.rules.iter().position(|r| r.id == rule_id) {
            if let Some(rule) = self.rules.get_mut(rule_index) {
                if rule.can_execute(world_state) {
                    let result = rule.execute(world_state);
                    self.execution_history.push(result.clone());
                    return Some(result);
                }
            }
        }
        None
    }

    fn sort_rules_by_priority(&mut self) {
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    fn calculate_rule_confidence(&self, rule: &DecisionRule, world_state: &WorldState) -> f64 {
        let satisfied_conditions = rule.conditions
            .iter()
            .filter(|c| c.condition.evaluate(world_state))
            .count();

        let total_conditions = rule.conditions.len().max(1);
        let base_confidence = satisfied_conditions as f64 / total_conditions as f64;

        // Factor in execution history
        let recent_executions = self.execution_history
            .iter()
            .rev()
            .take(10)
            .filter(|r| r.rule_id == rule.id)
            .count();

        let success_rate = if recent_executions > 0 {
            let successful = self.execution_history
                .iter()
                .rev()
                .take(10)
                .filter(|r| r.rule_id == rule.id && r.success)
                .count();
            successful as f64 / recent_executions as f64
        } else {
            0.8 // Default confidence for new rules
        };

        (base_confidence + success_rate) / 2.0
    }

    fn generate_rule_reason(&self, rule: &DecisionRule, world_state: &WorldState) -> String {
        let satisfied_conditions: Vec<String> = rule.conditions
            .iter()
            .filter(|c| c.condition.evaluate(world_state))
            .map(|c| format!("{} {} {:?}", c.condition.key,
                             self.operator_to_string(&c.condition.operator), c.condition.value))
            .collect();

        if satisfied_conditions.is_empty() {
            "No specific conditions met".to_string()
        } else {
            format!("Conditions met: {}", satisfied_conditions.join(", "))
        }
    }

    fn operator_to_string(&self, op: &ComparisonOperator) -> &'static str {
        match op {
            ComparisonOperator::Equal => "==",
            ComparisonOperator::NotEqual => "!=",
            ComparisonOperator::GreaterThan => ">",
            ComparisonOperator::LessThan => "<",
            ComparisonOperator::GreaterThanOrEqual => ">=",
            ComparisonOperator::LessThanOrEqual => "<=",
            ComparisonOperator::Contains => "contains",
            ComparisonOperator::StartsWith => "starts with",
            ComparisonOperator::EndsWith => "ends with",
        }
    }

    pub fn get_execution_statistics(&self) -> RuleEngineStatistics {
        let total_executions = self.execution_history.len();
        let successful_executions = self.execution_history.iter().filter(|r| r.success).count();

        let success_rate = if total_executions > 0 {
            successful_executions as f64 / total_executions as f64
        } else {
            0.0
        };

        let mut rule_execution_counts = HashMap::new();
        for result in &self.execution_history {
            *rule_execution_counts.entry(result.rule_id.clone()).or_insert(0) += 1;
        }

        RuleEngineStatistics {
            total_rules: self.rules.len(),
            enabled_rules: self.rules.iter().filter(|r| r.enabled).count(),
            total_executions,
            successful_executions,
            success_rate,
            rule_execution_counts,
        }
    }

    pub fn clear_history(&mut self) {
        self.execution_history.clear();
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecommendation {
    pub rule_id: String,
    pub rule_name: String,
    pub score: f64,
    pub confidence: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleExecutionResult {
    pub rule_id: String,
    pub success: bool,
    pub executed_actions: Vec<ActionExecutionResult>,
    pub total_actions: usize,
    pub successful_actions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionExecutionResult {
    pub action_type: String,
    pub success: bool,
    pub message: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleEngineStatistics {
    pub total_rules: usize,
    pub enabled_rules: usize,
    pub total_executions: usize,
    pub successful_executions: usize,
    pub success_rate: f64,
    pub rule_execution_counts: HashMap<String, usize>,
}

// Common rule builders
pub struct CommonRules;

impl CommonRules {
    pub fn threshold_rule(state_key: &str, threshold: f64, action_key: &str, action_value: StateValue) -> DecisionRule {
        DecisionRule::new(
            &format!("{}_threshold", state_key),
            &format!("Trigger when {} exceeds {}", state_key, threshold)
        )
        .add_simple_condition(state_key, ComparisonOperator::GreaterThan, StateValue::Float(threshold))
        .add_set_state_action(action_key, action_value)
        .with_priority(10)
    }

    pub fn resource_management_rule(resource: &str, min_amount: i64, replenish_amount: i64) -> DecisionRule {
        DecisionRule::new(
            &format!("{}_management", resource),
            &format!("Manage {} resource levels", resource)
        )
        .add_simple_condition(resource, ComparisonOperator::LessThan, StateValue::Integer(min_amount))
        .add_set_state_action(resource, StateValue::Integer(replenish_amount))
        .with_priority(20)
    }

    pub fn state_change_logger(state_key: &str) -> DecisionRule {
        DecisionRule::new(
            &format!("{}_logger", state_key),
            &format!("Log changes to {}", state_key)
        )
        .add_simple_condition(state_key, ComparisonOperator::NotEqual, StateValue::String("__previous__".to_string()))
        .add_log_action(LogLevel::Info, &format!("{} state changed", state_key))
        .with_priority(1)
    }

    pub fn conditional_goal_trigger(condition_key: &str, condition_value: StateValue, goal_id: &str) -> DecisionRule {
        DecisionRule::new(
            &format!("trigger_{}", goal_id),
            &format!("Trigger goal {} when condition is met", goal_id)
        )
        .add_simple_condition(condition_key, ComparisonOperator::Equal, condition_value)
        .add_trigger_goal_action(goal_id)
        .with_priority(15)
    }
}