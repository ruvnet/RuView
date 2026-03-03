use crate::action::Action;
use crate::state::WorldState;
// Note: pathfinding::astar requires Ord for costs, but we need f64
// We'll implement a simplified A* instead
// use pathfinding::prelude::astar;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct SearchNode {
    pub state: WorldState,
    pub cost: f64,
    pub heuristic: f64,
    pub action_taken: Option<String>,
    pub parent: Option<Box<SearchNode>>,
}

impl SearchNode {
    pub fn new(state: WorldState, cost: f64, heuristic: f64) -> Self {
        Self {
            state,
            cost,
            heuristic,
            action_taken: None,
            parent: None,
        }
    }

    pub fn with_action(mut self, action_id: String) -> Self {
        self.action_taken = Some(action_id);
        self
    }

    pub fn with_parent(mut self, parent: SearchNode) -> Self {
        self.parent = Some(Box::new(parent));
        self
    }

    pub fn total_cost(&self) -> f64 {
        self.cost + self.heuristic
    }
}

impl PartialEq for SearchNode {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
    }
}

impl Eq for SearchNode {}

impl Hash for SearchNode {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Create a simple hash based on state content
        self.state.to_compact_string().hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub success: bool,
    pub path: Vec<SearchStep>,
    pub total_cost: f64,
    pub nodes_explored: usize,
    pub time_taken: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchStep {
    pub action_id: String,
    pub state_before: WorldState,
    pub state_after: WorldState,
    pub cost: f64,
}

#[derive(Debug, Clone)]
pub struct AStarSearch {
    max_iterations: usize,
    time_limit: f64,
    distance_threshold: f64,
}

impl AStarSearch {
    pub fn new() -> Self {
        Self {
            max_iterations: 10000,
            time_limit: 30.0, // 30 seconds
            distance_threshold: 0.01,
        }
    }

    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    pub fn with_time_limit(mut self, time_limit: f64) -> Self {
        self.time_limit = time_limit;
        self
    }

    pub fn with_distance_threshold(mut self, threshold: f64) -> Self {
        self.distance_threshold = threshold;
        self
    }

    pub fn search(
        &self,
        start_state: &WorldState,
        goal_state: &WorldState,
        available_actions: &[Action],
    ) -> SearchResult {
        let search_start = std::time::Instant::now();

        // Convert to simplified state representation for pathfinding
        let start_node = StateNode::from_world_state(start_state);
        let goal_node = StateNode::from_world_state(goal_state);

        let mut nodes_explored = 0;

        // Simplified A* implementation since pathfinding crate requires Ord for costs
        let result = self.simplified_astar(
            start_node,
            goal_node,
            available_actions,
            &mut nodes_explored,
            search_start,
        );

        let time_taken = search_start.elapsed().as_secs_f64();

        match result {
            Some((path, total_cost)) => {
                let search_steps = self.convert_path_to_steps(path, start_state, available_actions);
                SearchResult {
                    success: true,
                    path: search_steps,
                    total_cost,
                    nodes_explored,
                    time_taken,
                    error: None,
                }
            }
            None => SearchResult {
                success: false,
                path: Vec::new(),
                total_cost: 0.0,
                nodes_explored,
                time_taken,
                error: Some("No path found".to_string()),
            },
        }
    }

    fn simplified_astar(
        &self,
        start_node: StateNode,
        goal_node: StateNode,
        available_actions: &[Action],
        nodes_explored: &mut usize,
        search_start: std::time::Instant,
    ) -> Option<(Vec<StateNode>, f64)> {
        // Simplified A* implementation that returns a basic path
        // This is a placeholder that would need a full implementation
        let path = vec![start_node.clone(), goal_node.clone()];
        *nodes_explored += 2;

        if search_start.elapsed().as_secs_f64() > self.time_limit {
            return None;
        }

        Some((path, 2.0)) // Basic cost
    }

    fn get_successors(&self, node: &StateNode, actions: &[Action]) -> Vec<(StateNode, f64)> {
        let mut successors = Vec::new();
        let current_state = node.to_world_state();

        for action in actions {
            if action.can_execute(&current_state) {
                let predicted_state = action.predict_state_after_execution(&current_state);
                let successor_node = StateNode::from_world_state(&predicted_state);
                let cost = action.get_total_cost(&current_state);

                // Store action information in the node
                let mut successor_with_action = successor_node;
                successor_with_action.last_action = Some(action.id.clone());

                successors.push((successor_with_action, cost));
            }
        }

        successors
    }

    fn heuristic(&self, current: &StateNode, goal: &StateNode) -> f64 {
        let current_state = current.to_world_state();
        let goal_state = goal.to_world_state();
        current_state.distance_to(&goal_state)
    }

    fn is_goal(&self, current: &StateNode, goal: &StateNode) -> bool {
        let current_state = current.to_world_state();
        let goal_state = goal.to_world_state();
        current_state.distance_to(&goal_state) < self.distance_threshold
    }

    fn convert_path_to_steps(
        &self,
        path: Vec<StateNode>,
        start_state: &WorldState,
        available_actions: &[Action],
    ) -> Vec<SearchStep> {
        let mut steps = Vec::new();
        let mut current_state = start_state.clone();

        for (i, node) in path.iter().enumerate() {
            if i == 0 {
                continue; // Skip the start node
            }

            if let Some(action_id) = &node.last_action {
                if let Some(action) = available_actions.iter().find(|a| a.id == *action_id) {
                    let next_state = node.to_world_state();
                    let cost = action.get_total_cost(&current_state);

                    steps.push(SearchStep {
                        action_id: action_id.clone(),
                        state_before: current_state.clone(),
                        state_after: next_state.clone(),
                        cost,
                    });

                    current_state = next_state;
                }
            }
        }

        steps
    }

    pub fn search_with_multiple_goals(
        &self,
        start_state: &WorldState,
        goal_states: &[WorldState],
        available_actions: &[Action],
    ) -> SearchResult {
        let mut best_result = SearchResult {
            success: false,
            path: Vec::new(),
            total_cost: f64::INFINITY,
            nodes_explored: 0,
            time_taken: 0.0,
            error: Some("No valid goals".to_string()),
        };

        for goal_state in goal_states {
            let result = self.search(start_state, goal_state, available_actions);
            if result.success && result.total_cost < best_result.total_cost {
                best_result = result;
            }
        }

        best_result
    }

    pub fn search_with_constraints(
        &self,
        start_state: &WorldState,
        goal_state: &WorldState,
        available_actions: &[Action],
        constraints: &SearchConstraints,
    ) -> SearchResult {
        // Filter actions based on constraints
        let filtered_actions: Vec<&Action> = available_actions
            .iter()
            .filter(|action| self.satisfies_constraints(action, constraints))
            .collect();

        if filtered_actions.is_empty() {
            return SearchResult {
                success: false,
                path: Vec::new(),
                total_cost: 0.0,
                nodes_explored: 0,
                time_taken: 0.0,
                error: Some("No actions satisfy constraints".to_string()),
            };
        }

        let actions_refs: Vec<Action> = filtered_actions.into_iter().cloned().collect();
        self.search(start_state, goal_state, &actions_refs)
    }

    fn satisfies_constraints(&self, action: &Action, constraints: &SearchConstraints) -> bool {
        // Check maximum cost constraint
        if let Some(max_cost) = constraints.max_action_cost {
            if action.cost.base_cost > max_cost {
                return false;
            }
        }

        // Check maximum duration constraint
        if let Some(max_duration) = constraints.max_action_duration {
            if action.duration > max_duration {
                return false;
            }
        }

        // Check forbidden actions
        if constraints.forbidden_actions.contains(&action.id) {
            return false;
        }

        // Check required action categories
        if !constraints.allowed_categories.is_empty() {
            let action_category = action.get_category_name();
            if !constraints.allowed_categories.contains(&action_category) {
                return false;
            }
        }

        true
    }
}

impl Default for AStarSearch {
    fn default() -> Self {
        Self::new()
    }
}

// Simplified state representation for pathfinding
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StateNode {
    state_hash: String,
    last_action: Option<String>,
}

impl StateNode {
    fn from_world_state(state: &WorldState) -> Self {
        Self {
            state_hash: state.to_compact_string(),
            last_action: None,
        }
    }

    fn to_world_state(&self) -> WorldState {
        // This is a simplified conversion
        // In a real implementation, you'd need to store/reconstruct the full state
        WorldState::new() // Placeholder - would need proper deserialization
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConstraints {
    pub max_action_cost: Option<f64>,
    pub max_action_duration: Option<f64>,
    pub forbidden_actions: HashSet<String>,
    pub allowed_categories: HashSet<String>,
    pub max_total_cost: Option<f64>,
    pub max_total_duration: Option<f64>,
}

impl SearchConstraints {
    pub fn new() -> Self {
        Self {
            max_action_cost: None,
            max_action_duration: None,
            forbidden_actions: HashSet::new(),
            allowed_categories: HashSet::new(),
            max_total_cost: None,
            max_total_duration: None,
        }
    }

    pub fn with_max_action_cost(mut self, cost: f64) -> Self {
        self.max_action_cost = Some(cost);
        self
    }

    pub fn with_max_action_duration(mut self, duration: f64) -> Self {
        self.max_action_duration = Some(duration);
        self
    }

    pub fn forbid_action(mut self, action_id: &str) -> Self {
        self.forbidden_actions.insert(action_id.to_string());
        self
    }

    pub fn allow_category(mut self, category: &str) -> Self {
        self.allowed_categories.insert(category.to_string());
        self
    }

    pub fn with_max_total_cost(mut self, cost: f64) -> Self {
        self.max_total_cost = Some(cost);
        self
    }

    pub fn with_max_total_duration(mut self, duration: f64) -> Self {
        self.max_total_duration = Some(duration);
        self
    }
}

impl Default for SearchConstraints {
    fn default() -> Self {
        Self::new()
    }
}

// Advanced search algorithms
pub struct BidirectionalSearch {
    forward_search: AStarSearch,
    backward_search: AStarSearch,
}

impl BidirectionalSearch {
    pub fn new() -> Self {
        Self {
            forward_search: AStarSearch::new(),
            backward_search: AStarSearch::new(),
        }
    }

    pub fn search(
        &self,
        start_state: &WorldState,
        goal_state: &WorldState,
        available_actions: &[Action],
    ) -> SearchResult {
        // Simplified bidirectional search - would need more sophisticated implementation
        // For now, just use forward search
        self.forward_search.search(start_state, goal_state, available_actions)
    }
}

impl Default for BidirectionalSearch {
    fn default() -> Self {
        Self::new()
    }
}

pub struct IterativeDeepeningSearch {
    max_depth: usize,
    depth_increment: usize,
}

impl IterativeDeepeningSearch {
    pub fn new() -> Self {
        Self {
            max_depth: 20,
            depth_increment: 2,
        }
    }

    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    pub fn search(
        &self,
        start_state: &WorldState,
        goal_state: &WorldState,
        available_actions: &[Action],
    ) -> SearchResult {
        for depth in (1..=self.max_depth).step_by(self.depth_increment) {
            let search = AStarSearch::new().with_max_iterations(depth * 100);
            let result = search.search(start_state, goal_state, available_actions);

            if result.success {
                return result;
            }
        }

        SearchResult {
            success: false,
            path: Vec::new(),
            total_cost: 0.0,
            nodes_explored: 0,
            time_taken: 0.0,
            error: Some("Maximum depth reached".to_string()),
        }
    }
}

impl Default for IterativeDeepeningSearch {
    fn default() -> Self {
        Self::new()
    }
}

// Search metrics and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMetrics {
    pub total_searches: usize,
    pub successful_searches: usize,
    pub average_cost: f64,
    pub average_time: f64,
    pub average_nodes_explored: f64,
    pub success_rate: f64,
}

impl SearchMetrics {
    pub fn new() -> Self {
        Self {
            total_searches: 0,
            successful_searches: 0,
            average_cost: 0.0,
            average_time: 0.0,
            average_nodes_explored: 0.0,
            success_rate: 0.0,
        }
    }

    pub fn add_result(&mut self, result: &SearchResult) {
        self.total_searches += 1;

        if result.success {
            self.successful_searches += 1;
        }

        // Update running averages
        let n = self.total_searches as f64;
        self.average_cost = (self.average_cost * (n - 1.0) + result.total_cost) / n;
        self.average_time = (self.average_time * (n - 1.0) + result.time_taken) / n;
        self.average_nodes_explored = (self.average_nodes_explored * (n - 1.0) + result.nodes_explored as f64) / n;

        self.success_rate = self.successful_searches as f64 / self.total_searches as f64;
    }
}

impl Default for SearchMetrics {
    fn default() -> Self {
        Self::new()
    }
}