//! Agentic loop for autonomous decision-making (Plan-Act-Observe-Learn)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use async_trait::async_trait;

use super::types::{Context, AgentState, Goal, Policy, Reward};
use super::LeanAgenticConfig;

/// Agentic loop orchestrator
pub struct AgenticLoop {
    /// Current agent state
    state: AgentState,

    /// Configuration
    config: LeanAgenticConfig,

    /// Action history
    action_history: Vec<Action>,

    /// Total reward accumulated
    total_reward: f64,

    /// Action execution count
    action_count: u64,
}

impl AgenticLoop {
    pub fn new(config: LeanAgenticConfig) -> Self {
        Self {
            state: AgentState::default(),
            config,
            action_history: Vec::new(),
            total_reward: 0.0,
            action_count: 0,
        }
    }

    /// Plan phase: Generate a plan based on goals and context
    pub async fn plan(&self, context: &Context, input: &str) -> Result<Plan, String> {
        let mut plan = Plan {
            goal: Goal {
                id: format!("goal_{}", self.action_count),
                description: format!("Process: {}", input),
                priority: 1.0,
                achieved: false,
            },
            steps: Vec::new(),
            estimated_reward: 0.0,
            confidence: 0.0,
        };

        // Analyze input to determine appropriate actions
        let actions = self.generate_action_candidates(input, context).await;

        // Rank actions by expected reward
        let ranked_actions = self.rank_actions(actions).await;

        // Add top actions to plan
        for (i, action) in ranked_actions.iter().take(self.config.max_planning_depth).enumerate() {
            plan.steps.push(PlanStep {
                sequence: i,
                action: action.clone(),
                preconditions: vec![],
                postconditions: vec![],
            });
        }

        plan.estimated_reward = ranked_actions.first()
            .map(|a| a.expected_reward)
            .unwrap_or(0.0);

        plan.confidence = if !plan.steps.is_empty() { 0.8 } else { 0.0 };

        Ok(plan)
    }

    /// Act phase: Select and prepare an action from the plan
    pub async fn select_action(&self, plan: &Plan) -> Result<Action, String> {
        if plan.steps.is_empty() {
            return Err("Empty plan".to_string());
        }

        // Select first step with highest confidence
        let step = &plan.steps[0];
        Ok(step.action.clone())
    }

    /// Execute an action and return observation
    pub async fn execute(&mut self, action: &Action) -> Result<Observation, String> {
        self.action_count += 1;
        self.action_history.push(action.clone());

        // Simulate action execution
        let observation = Observation {
            success: true,
            result: format!("Executed: {}", action.action_type),
            changes: vec![format!("Action {} completed", action.action_type)],
            timestamp: chrono::Utc::now().timestamp(),
        };

        Ok(observation)
    }

    /// Compute reward based on observation
    pub async fn compute_reward(&self, observation: &Observation) -> Result<Reward, String> {
        let base_reward = if observation.success { 1.0 } else { -1.0 };

        // Bonus for meaningful changes
        let change_bonus = observation.changes.len() as f64 * 0.1;

        Ok(base_reward + change_bonus)
    }

    /// Learn phase: Update policies based on experience
    pub async fn learn(&mut self, signal: LearningSignal) -> Result<(), String> {
        self.total_reward += signal.reward;

        // Update policy based on reward
        let policy = Policy {
            condition: format!("When: {}", signal.action.description),
            action: signal.action.action_type.clone(),
            expected_reward: signal.reward,
            usage_count: 1,
        };

        // Check if similar policy exists
        if let Some(existing) = self.state.policies.iter_mut()
            .find(|p| p.action == policy.action) {
            // Update existing policy with exponential moving average
            existing.expected_reward = 0.9 * existing.expected_reward + 0.1 * signal.reward;
            existing.usage_count += 1;
        } else {
            // Add new policy
            self.state.policies.push(policy);
        }

        // Update confidence based on learning
        self.state.confidence = (self.total_reward / self.action_count as f64).clamp(0.0, 1.0);

        Ok(())
    }

    async fn generate_action_candidates(&self, input: &str, context: &Context) -> Vec<Action> {
        let mut candidates = Vec::new();

        // Generate different action types based on input
        let input_lower = input.to_lowercase();

        if input_lower.contains("weather") {
            candidates.push(Action {
                action_type: "get_weather".to_string(),
                description: "Fetch weather information".to_string(),
                parameters: HashMap::from([
                    ("query".to_string(), input.to_string()),
                ]),
                tool_calls: vec!["weather_api".to_string()],
                expected_outcome: Some("Weather data".to_string()),
                expected_reward: 0.8,
            });
        }

        if input_lower.contains("learn") || input_lower.contains("remember") {
            candidates.push(Action {
                action_type: "update_knowledge".to_string(),
                description: "Update knowledge graph".to_string(),
                parameters: HashMap::from([
                    ("content".to_string(), input.to_string()),
                ]),
                tool_calls: vec![],
                expected_outcome: Some("Knowledge updated".to_string()),
                expected_reward: 0.9,
            });
        }

        // Default action: process and respond
        candidates.push(Action {
            action_type: "process_text".to_string(),
            description: format!("Process: {}", input),
            parameters: HashMap::from([
                ("text".to_string(), input.to_string()),
            ]),
            tool_calls: vec![],
            expected_outcome: Some("Processed text".to_string()),
            expected_reward: 0.5,
        });

        candidates
    }

    async fn rank_actions(&self, mut actions: Vec<Action>) -> Vec<Action> {
        // Sort by expected reward and learned policies
        actions.sort_by(|a, b| {
            let a_boost = self.state.policies.iter()
                .find(|p| p.action == a.action_type)
                .map(|p| p.expected_reward)
                .unwrap_or(0.0);

            let b_boost = self.state.policies.iter()
                .find(|p| p.action == b.action_type)
                .map(|p| p.expected_reward)
                .unwrap_or(0.0);

            let a_score = a.expected_reward + a_boost * 0.5;
            let b_score = b.expected_reward + b_boost * 0.5;

            b_score.partial_cmp(&a_score).unwrap()
        });

        actions
    }

    pub fn action_count(&self) -> u64 {
        self.action_count
    }

    pub fn average_reward(&self) -> f64 {
        if self.action_count == 0 {
            0.0
        } else {
            self.total_reward / self.action_count as f64
        }
    }
}

/// An action the agent can take
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub action_type: String,
    pub description: String,
    pub parameters: HashMap<String, String>,
    pub tool_calls: Vec<String>,
    pub expected_outcome: Option<String>,
    pub expected_reward: f64,
}

/// An observation from the environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub success: bool,
    pub result: String,
    pub changes: Vec<String>,
    pub timestamp: i64,
}

/// A plan for achieving a goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub goal: Goal,
    pub steps: Vec<PlanStep>,
    pub estimated_reward: f64,
    pub confidence: f64,
}

/// A step in a plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub sequence: usize,
    pub action: Action,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
}

/// Learning signal for the agent
#[derive(Debug, Clone)]
pub struct LearningSignal {
    pub action: Action,
    pub observation: Observation,
    pub reward: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agentic_loop() {
        let config = LeanAgenticConfig::default();
        let mut agent = AgenticLoop::new(config);

        let context = Context::default();
        let plan = agent.plan(&context, "test input").await.unwrap();

        assert!(!plan.steps.is_empty());
    }
}
