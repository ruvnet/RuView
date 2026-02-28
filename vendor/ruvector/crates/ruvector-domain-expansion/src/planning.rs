//! Structured Planning Tasks Domain
//!
//! Generates tasks that require multi-step reasoning and plan construction.
//! Task types include:
//!
//! - **ResourceAllocation**: Assign limited resources to maximize objective
//! - **DependencyScheduling**: Order tasks respecting dependencies and deadlines
//! - **StateSpaceSearch**: Navigate from initial to goal state
//! - **ConstraintSatisfaction**: Find assignments satisfying all constraints
//! - **HierarchicalDecomposition**: Break complex goals into sub-goals
//!
//! Solutions are plans: ordered sequences of actions with preconditions and effects.
//! Cross-domain transfer from Rust synthesis helps because both require:
//! structured decomposition, constraint satisfaction, and efficient search.

use crate::domain::{Domain, DomainEmbedding, DomainId, Evaluation, Solution, Task};
use rand::Rng;
use serde::{Deserialize, Serialize};

const EMBEDDING_DIM: usize = 64;

/// Categories of planning tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanningCategory {
    /// Assign limited resources to competing demands.
    ResourceAllocation,
    /// Schedule tasks with precedence constraints and deadlines.
    DependencyScheduling,
    /// Find a path from initial state to goal state.
    StateSpaceSearch,
    /// Assign values to variables satisfying all constraints.
    ConstraintSatisfaction,
    /// Decompose a high-level goal into achievable sub-tasks.
    HierarchicalDecomposition,
}

/// A resource in the planning world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub name: String,
    pub capacity: u32,
}

/// An action in a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanAction {
    pub name: String,
    pub preconditions: Vec<String>,
    pub effects: Vec<String>,
    pub cost: f32,
    pub duration: u32,
}

/// A dependency edge: task A must complete before task B.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub from: String,
    pub to: String,
}

/// Specification for a planning task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningTaskSpec {
    pub category: PlanningCategory,
    pub description: String,
    /// Available actions in the planning domain.
    pub available_actions: Vec<PlanAction>,
    /// Resources with capacity limits.
    pub resources: Vec<Resource>,
    /// Dependency constraints.
    pub dependencies: Vec<Dependency>,
    /// Initial state predicates.
    pub initial_state: Vec<String>,
    /// Goal state predicates.
    pub goal_state: Vec<String>,
    /// Maximum allowed plan cost.
    pub max_cost: Option<f32>,
    /// Maximum allowed plan steps.
    pub max_steps: Option<usize>,
}

/// A parsed plan from a solution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub steps: Vec<PlanStep>,
}

/// A single step in a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub action: String,
    pub args: Vec<String>,
    pub start_time: Option<u32>,
}

/// Structured planning domain.
pub struct PlanningDomain {
    id: DomainId,
}

impl PlanningDomain {
    pub fn new() -> Self {
        Self {
            id: DomainId("structured_planning".to_string()),
        }
    }

    fn gen_resource_allocation(&self, difficulty: f32) -> PlanningTaskSpec {
        let num_tasks = if difficulty < 0.3 {
            3
        } else if difficulty < 0.7 {
            6
        } else {
            10
        };

        let actions: Vec<PlanAction> = (0..num_tasks)
            .map(|i| PlanAction {
                name: format!("task_{}", i),
                preconditions: vec![format!("resource_available_{}", i % 3)],
                effects: vec![format!("task_{}_complete", i)],
                cost: (i as f32 + 1.0) * 10.0,
                duration: (i as u32 % 5) + 1,
            })
            .collect();

        let resources = vec![
            Resource {
                name: "cpu".into(),
                capacity: if difficulty < 0.5 { 10 } else { 5 },
            },
            Resource {
                name: "memory".into(),
                capacity: if difficulty < 0.5 { 8 } else { 3 },
            },
            Resource {
                name: "io".into(),
                capacity: if difficulty < 0.5 { 6 } else { 2 },
            },
        ];

        let goal_state: Vec<String> = (0..num_tasks)
            .map(|i| format!("task_{}_complete", i))
            .collect();

        PlanningTaskSpec {
            category: PlanningCategory::ResourceAllocation,
            description: format!(
                "Allocate {} resources to complete {} tasks within capacity.",
                resources.len(),
                num_tasks
            ),
            available_actions: actions,
            resources,
            dependencies: Vec::new(),
            initial_state: vec![
                "resource_available_0".into(),
                "resource_available_1".into(),
                "resource_available_2".into(),
            ],
            goal_state,
            max_cost: Some(num_tasks as f32 * 50.0),
            max_steps: Some(num_tasks * 2),
        }
    }

    fn gen_dependency_scheduling(&self, difficulty: f32) -> PlanningTaskSpec {
        let num_tasks = if difficulty < 0.3 {
            4
        } else if difficulty < 0.7 {
            7
        } else {
            12
        };

        let actions: Vec<PlanAction> = (0..num_tasks)
            .map(|i| PlanAction {
                name: format!("job_{}", i),
                preconditions: if i > 0 {
                    vec![format!("job_{}_done", i - 1)]
                } else {
                    Vec::new()
                },
                effects: vec![format!("job_{}_done", i)],
                cost: 1.0,
                duration: (i as u32 % 3) + 1,
            })
            .collect();

        // Create dependency chain with some parallelism
        let mut dependencies = Vec::new();
        for i in 1..num_tasks {
            // Linear chain
            dependencies.push(Dependency {
                from: format!("job_{}", i - 1),
                to: format!("job_{}", i),
            });
            // Add cross-dependencies at higher difficulty
            if difficulty > 0.5 && i >= 3 && i % 2 == 0 {
                dependencies.push(Dependency {
                    from: format!("job_{}", i - 3),
                    to: format!("job_{}", i),
                });
            }
        }

        PlanningTaskSpec {
            category: PlanningCategory::DependencyScheduling,
            description: format!(
                "Schedule {} jobs respecting {} dependencies, minimizing makespan.",
                num_tasks,
                dependencies.len()
            ),
            available_actions: actions,
            resources: vec![Resource {
                name: "worker".into(),
                capacity: if difficulty < 0.5 { 3 } else { 2 },
            }],
            dependencies,
            initial_state: Vec::new(),
            goal_state: (0..num_tasks).map(|i| format!("job_{}_done", i)).collect(),
            max_cost: None,
            max_steps: Some(num_tasks + 5),
        }
    }

    fn gen_state_space_search(&self, difficulty: f32) -> PlanningTaskSpec {
        let grid_size = if difficulty < 0.3 {
            3
        } else if difficulty < 0.7 {
            5
        } else {
            8
        };

        let actions = vec![
            PlanAction {
                name: "move_up".into(),
                preconditions: vec!["not_top_edge".into()],
                effects: vec!["moved_up".into()],
                cost: 1.0,
                duration: 1,
            },
            PlanAction {
                name: "move_down".into(),
                preconditions: vec!["not_bottom_edge".into()],
                effects: vec!["moved_down".into()],
                cost: 1.0,
                duration: 1,
            },
            PlanAction {
                name: "move_left".into(),
                preconditions: vec!["not_left_edge".into()],
                effects: vec!["moved_left".into()],
                cost: 1.0,
                duration: 1,
            },
            PlanAction {
                name: "move_right".into(),
                preconditions: vec!["not_right_edge".into()],
                effects: vec!["moved_right".into()],
                cost: 1.0,
                duration: 1,
            },
        ];

        PlanningTaskSpec {
            category: PlanningCategory::StateSpaceSearch,
            description: format!(
                "Navigate a {}x{} grid from (0,0) to ({},{}) avoiding obstacles.",
                grid_size,
                grid_size,
                grid_size - 1,
                grid_size - 1
            ),
            available_actions: actions,
            resources: Vec::new(),
            dependencies: Vec::new(),
            initial_state: vec!["at(0,0)".into()],
            goal_state: vec![format!("at({},{})", grid_size - 1, grid_size - 1)],
            max_cost: Some((grid_size as f32) * 4.0),
            max_steps: Some(grid_size * grid_size),
        }
    }

    /// Extract structural features from a planning solution.
    fn extract_features(&self, solution: &Solution) -> Vec<f32> {
        let content = &solution.content;
        let mut features = vec![0.0f32; EMBEDDING_DIM];

        // Parse the plan
        let plan: Plan = serde_json::from_str(&solution.data.to_string())
            .or_else(|_| serde_json::from_str(content))
            .unwrap_or(Plan { steps: Vec::new() });

        // Feature 0-7: Plan structure
        features[0] = plan.steps.len() as f32 / 20.0;
        features[1] = {
            let unique_actions: std::collections::HashSet<&str> =
                plan.steps.iter().map(|s| s.action.as_str()).collect();
            unique_actions.len() as f32 / plan.steps.len().max(1) as f32
        };
        // Sequential vs parallel indicator
        features[2] = plan
            .steps
            .windows(2)
            .filter(|w| w[0].start_time == w[1].start_time)
            .count() as f32
            / plan.steps.len().max(1) as f32;
        // Average args per step
        features[3] = plan.steps.iter().map(|s| s.args.len() as f32).sum::<f32>()
            / plan.steps.len().max(1) as f32
            / 5.0;

        // Feature 8-15: Action type distribution
        let action_counts: std::collections::HashMap<&str, usize> =
            plan.steps
                .iter()
                .fold(std::collections::HashMap::new(), |mut acc, s| {
                    *acc.entry(s.action.as_str()).or_insert(0) += 1;
                    acc
                });
        let max_count = action_counts.values().max().copied().unwrap_or(0);
        features[8] = action_counts.len() as f32 / 10.0;
        features[9] = max_count as f32 / plan.steps.len().max(1) as f32;

        // Feature 16-23: Text-based features from content
        features[16] = content.matches("allocate").count() as f32 / 5.0;
        features[17] = content.matches("schedule").count() as f32 / 5.0;
        features[18] = content.matches("move").count() as f32 / 10.0;
        features[19] = content.matches("assign").count() as f32 / 5.0;
        features[20] = content.matches("wait").count() as f32 / 5.0;
        features[21] = content.matches("parallel").count() as f32 / 3.0;
        features[22] = content.matches("constraint").count() as f32 / 5.0;
        features[23] = content.matches("deadline").count() as f32 / 3.0;

        // Feature 32-39: Structural complexity indicators
        features[32] = content.matches("->").count() as f32 / 10.0;
        features[33] = content.matches("if ").count() as f32 / 5.0;
        features[34] = content.matches("then ").count() as f32 / 5.0;
        features[35] = content.matches("before").count() as f32 / 5.0;
        features[36] = content.matches("after").count() as f32 / 5.0;
        features[37] = content.matches("while").count() as f32 / 3.0;
        features[38] = content.matches("until").count() as f32 / 3.0;
        features[39] = content.matches("complete").count() as f32 / 5.0;

        // Feature 48-55: Resource usage indicators
        features[48] = content.matches("cpu").count() as f32 / 3.0;
        features[49] = content.matches("memory").count() as f32 / 3.0;
        features[50] = content.matches("worker").count() as f32 / 3.0;
        features[51] = content.matches("capacity").count() as f32 / 3.0;
        features[52] = content.matches("cost").count() as f32 / 5.0;
        features[53] = content.matches("time").count() as f32 / 5.0;
        features[54] = content.matches("resource").count() as f32 / 5.0;
        features[55] = content.matches("limit").count() as f32 / 3.0;

        // Normalize
        let norm: f32 = features.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-10 {
            for f in &mut features {
                *f /= norm;
            }
        }

        features
    }

    /// Evaluate a planning solution.
    fn score_plan(&self, spec: &PlanningTaskSpec, solution: &Solution) -> Evaluation {
        let content = &solution.content;
        let mut correctness = 0.0f32;
        let mut efficiency = 0.5f32;
        let mut elegance = 0.5f32;
        let mut notes = Vec::new();

        // Parse plan from solution
        let plan: Option<Plan> = serde_json::from_str(&solution.data.to_string())
            .ok()
            .or_else(|| serde_json::from_str(content).ok());

        let plan = match plan {
            Some(p) => p,
            None => {
                // Fall back to text analysis
                let has_steps = content.contains("step") || content.contains("action");
                if has_steps {
                    correctness = 0.2;
                }
                return Evaluation {
                    score: correctness * 0.6,
                    correctness,
                    efficiency: 0.0,
                    elegance: 0.0,
                    constraint_results: Vec::new(),
                    notes: vec!["Could not parse structured plan".into()],
                };
            }
        };

        // Check plan is non-empty
        if plan.steps.is_empty() {
            return Evaluation::zero(vec!["Empty plan".into()]);
        }

        // Check goal coverage: how many goal predicates are addressed
        let goal_coverage = spec
            .goal_state
            .iter()
            .filter(|goal| {
                plan.steps.iter().any(|step| {
                    let action_name = &step.action;
                    // Check if any action's effects mention this goal
                    spec.available_actions
                        .iter()
                        .any(|a| a.name == *action_name && a.effects.iter().any(|e| e == *goal))
                })
            })
            .count() as f32
            / spec.goal_state.len().max(1) as f32;

        correctness = goal_coverage;

        // Check dependency ordering
        let mut dep_violations = 0;
        for dep in &spec.dependencies {
            let from_pos = plan.steps.iter().position(|s| s.action == dep.from);
            let to_pos = plan.steps.iter().position(|s| s.action == dep.to);
            if let (Some(f), Some(t)) = (from_pos, to_pos) {
                if f >= t {
                    dep_violations += 1;
                    notes.push(format!(
                        "Dependency violation: {} must come before {}",
                        dep.from, dep.to
                    ));
                }
            }
        }
        if !spec.dependencies.is_empty() {
            let dep_score = 1.0 - (dep_violations as f32 / spec.dependencies.len() as f32);
            correctness = correctness * 0.5 + dep_score * 0.5;
        }

        // Efficiency: compare to max allowed steps/cost
        if let Some(max_steps) = spec.max_steps {
            let step_ratio = plan.steps.len() as f32 / max_steps as f32;
            efficiency = if step_ratio <= 1.0 {
                1.0 - (step_ratio * 0.5) // Fewer steps = better
            } else {
                0.5 / step_ratio // Penalty for exceeding max
            };
        }

        if let Some(max_cost) = spec.max_cost {
            let total_cost: f32 = plan
                .steps
                .iter()
                .filter_map(|step| {
                    spec.available_actions
                        .iter()
                        .find(|a| a.name == step.action)
                        .map(|a| a.cost)
                })
                .sum();
            if total_cost > max_cost {
                efficiency *= 0.5;
                notes.push(format!(
                    "Plan cost {:.1} exceeds budget {:.1}",
                    total_cost, max_cost
                ));
            }
        }

        // Elegance: minimal redundancy, good parallelism
        let unique_actions: std::collections::HashSet<&str> =
            plan.steps.iter().map(|s| s.action.as_str()).collect();
        let redundancy = 1.0 - (unique_actions.len() as f32 / plan.steps.len().max(1) as f32);
        elegance = 1.0 - redundancy * 0.5;

        // Bonus for parallel scheduling
        if plan
            .steps
            .windows(2)
            .any(|w| w[0].start_time == w[1].start_time)
        {
            elegance += 0.1;
        }
        elegance = elegance.clamp(0.0, 1.0);

        let score = 0.6 * correctness + 0.25 * efficiency + 0.15 * elegance;
        Evaluation {
            score: score.clamp(0.0, 1.0),
            correctness,
            efficiency,
            elegance,
            constraint_results: Vec::new(),
            notes,
        }
    }
}

impl Default for PlanningDomain {
    fn default() -> Self {
        Self::new()
    }
}

impl Domain for PlanningDomain {
    fn id(&self) -> &DomainId {
        &self.id
    }

    fn name(&self) -> &str {
        "Structured Planning"
    }

    fn generate_tasks(&self, count: usize, difficulty: f32) -> Vec<Task> {
        let mut rng = rand::thread_rng();
        let difficulty = difficulty.clamp(0.0, 1.0);

        (0..count)
            .map(|i| {
                let category_roll: f32 = rng.gen();
                let spec = if category_roll < 0.35 {
                    self.gen_resource_allocation(difficulty)
                } else if category_roll < 0.7 {
                    self.gen_dependency_scheduling(difficulty)
                } else {
                    self.gen_state_space_search(difficulty)
                };

                Task {
                    id: format!("planning_{}_d{:.0}", i, difficulty * 100.0),
                    domain_id: self.id.clone(),
                    difficulty,
                    spec: serde_json::to_value(&spec).unwrap_or_default(),
                    constraints: Vec::new(),
                }
            })
            .collect()
    }

    fn evaluate(&self, task: &Task, solution: &Solution) -> Evaluation {
        let spec: PlanningTaskSpec = match serde_json::from_value(task.spec.clone()) {
            Ok(s) => s,
            Err(e) => return Evaluation::zero(vec![format!("Invalid task spec: {}", e)]),
        };
        self.score_plan(&spec, solution)
    }

    fn embed(&self, solution: &Solution) -> DomainEmbedding {
        let features = self.extract_features(solution);
        DomainEmbedding::new(features, self.id.clone())
    }

    fn embedding_dim(&self) -> usize {
        EMBEDDING_DIM
    }

    fn reference_solution(&self, task: &Task) -> Option<Solution> {
        let spec: PlanningTaskSpec = serde_json::from_value(task.spec.clone()).ok()?;

        // Generate a naive sequential plan that executes all actions in order
        let steps: Vec<PlanStep> = spec
            .available_actions
            .iter()
            .enumerate()
            .map(|(i, a)| PlanStep {
                action: a.name.clone(),
                args: Vec::new(),
                start_time: Some(i as u32),
            })
            .collect();

        let plan = Plan { steps };
        let content = serde_json::to_string_pretty(&plan).ok()?;

        Some(Solution {
            task_id: task.id.clone(),
            content,
            data: serde_json::to_value(&plan).ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_planning_tasks() {
        let domain = PlanningDomain::new();
        let tasks = domain.generate_tasks(5, 0.5);
        assert_eq!(tasks.len(), 5);
        for task in &tasks {
            assert_eq!(task.domain_id, domain.id);
        }
    }

    #[test]
    fn test_reference_solution_exists() {
        let domain = PlanningDomain::new();
        let tasks = domain.generate_tasks(3, 0.3);
        for task in &tasks {
            let ref_sol = domain.reference_solution(task);
            assert!(ref_sol.is_some(), "Should produce reference solution");
        }
    }

    #[test]
    fn test_evaluate_reference() {
        let domain = PlanningDomain::new();
        let tasks = domain.generate_tasks(3, 0.3);
        for task in &tasks {
            if let Some(solution) = domain.reference_solution(task) {
                let eval = domain.evaluate(task, &solution);
                assert!(eval.score >= 0.0 && eval.score <= 1.0);
            }
        }
    }

    #[test]
    fn test_embed_planning() {
        let domain = PlanningDomain::new();
        let solution = Solution {
            task_id: "test".into(),
            content: "allocate cpu to task_0, schedule job_1 after job_0".into(),
            data: serde_json::json!({ "steps": [] }),
        };
        let embedding = domain.embed(&solution);
        assert_eq!(embedding.dim, EMBEDDING_DIM);
    }

    #[test]
    fn test_difficulty_scaling() {
        let domain = PlanningDomain::new();
        let easy = domain.generate_tasks(1, 0.1);
        let hard = domain.generate_tasks(1, 0.9);

        let easy_spec: PlanningTaskSpec = serde_json::from_value(easy[0].spec.clone()).unwrap();
        let hard_spec: PlanningTaskSpec = serde_json::from_value(hard[0].spec.clone()).unwrap();

        assert!(
            hard_spec.available_actions.len() >= easy_spec.available_actions.len(),
            "Harder tasks should have more actions"
        );
    }
}
