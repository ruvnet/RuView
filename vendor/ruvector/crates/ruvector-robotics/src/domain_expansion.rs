//! Robotics domain for cross-domain transfer learning.
//!
//! Implements [`ruvector_domain_expansion::Domain`] so that robotics tasks
//! (perception, planning, skill learning) participate in the domain-expansion
//! engine's transfer-learning pipeline alongside Rust synthesis, structured
//! planning, and tool orchestration.
//!
//! ## Task categories
//!
//! | Category | Description |
//! |---|---|
//! | `PointCloudClustering` | Cluster a synthetic point cloud into objects |
//! | `ObstacleAvoidance` | Plan a collision-free path through obstacles |
//! | `SceneGraphConstruction` | Build a scene graph from a set of objects |
//! | `SkillSequencing` | Select and sequence learned motor skills |
//! | `SwarmFormation` | Assign robots to formation positions |
//!
//! ## Transfer synergies
//!
//! - **Planning ↔ Robotics**: Both decompose goals into ordered steps with
//!   resource constraints. Robotics adds spatial reasoning.
//! - **Tool Orchestration ↔ Robotics**: Swarm coordination is structurally
//!   similar to multi-tool pipeline coordination.
//! - **Rust Synthesis ↔ Robotics**: Algorithmic solutions (search, sort,
//!   graph traversal) directly appear in perception and planning kernels.

use rand::Rng;
use ruvector_domain_expansion::domain::{Domain, DomainEmbedding, DomainId, Evaluation, Solution, Task};
use serde::{Deserialize, Serialize};

const EMBEDDING_DIM: usize = 64;

// ---------------------------------------------------------------------------
// Task specification types
// ---------------------------------------------------------------------------

/// Categories of robotics tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoboticsCategory {
    /// Cluster a point cloud into distinct objects.
    PointCloudClustering,
    /// Plan a path that avoids all obstacles.
    ObstacleAvoidance,
    /// Build a scene graph from detected objects.
    SceneGraphConstruction,
    /// Select and order skills to achieve a goal.
    SkillSequencing,
    /// Assign N robots to formation positions.
    SwarmFormation,
}

/// A synthetic obstacle for avoidance tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskObstacle {
    pub center: [f64; 3],
    pub radius: f64,
}

/// A skill reference for sequencing tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSkill {
    pub name: String,
    pub preconditions: Vec<String>,
    pub effects: Vec<String>,
    pub cost: f32,
}

/// Specification for a robotics task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoboticsTaskSpec {
    pub category: RoboticsCategory,
    pub description: String,
    /// Number of points (clustering), obstacles (avoidance), objects (scene
    /// graph), skills (sequencing), or robots (formation).
    pub size: usize,
    /// Spatial extent of the task environment.
    pub world_bounds: [f64; 3],
    /// Obstacles (used by avoidance and scene graph tasks).
    pub obstacles: Vec<TaskObstacle>,
    /// Skills (used by sequencing tasks).
    pub skills: Vec<TaskSkill>,
    /// Start position (avoidance).
    pub start: Option<[f64; 3]>,
    /// Goal position (avoidance).
    pub goal: Option<[f64; 3]>,
    /// Desired formation type name (swarm).
    pub formation: Option<String>,
}

/// A parsed robotics solution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoboticsSolution {
    /// Waypoints for path or formation positions.
    pub waypoints: Vec<[f64; 3]>,
    /// Cluster assignments (point index → cluster id).
    pub cluster_ids: Vec<usize>,
    /// Ordered skill names.
    pub skill_sequence: Vec<String>,
    /// Scene graph edges as (from, to) pairs.
    pub edges: Vec<(usize, usize)>,
}

// ---------------------------------------------------------------------------
// Domain implementation
// ---------------------------------------------------------------------------

/// Robotics domain for the domain-expansion engine.
pub struct RoboticsDomain {
    id: DomainId,
}

impl RoboticsDomain {
    /// Create a new robotics domain.
    pub fn new() -> Self {
        Self {
            id: DomainId("robotics".to_string()),
        }
    }

    // -- task generators ---------------------------------------------------

    fn gen_clustering(&self, difficulty: f32, rng: &mut impl Rng) -> RoboticsTaskSpec {
        let num_clusters = if difficulty < 0.3 { 2 } else if difficulty < 0.7 { 5 } else { 10 };
        let pts_per_cluster = if difficulty < 0.3 { 10 } else { 20 };
        let spread = if difficulty < 0.5 { 0.5 } else { 2.0 };

        let mut obstacles = Vec::new();
        for _ in 0..num_clusters {
            obstacles.push(TaskObstacle {
                center: [
                    rng.gen_range(-10.0..10.0),
                    rng.gen_range(-10.0..10.0),
                    rng.gen_range(0.0..5.0),
                ],
                radius: spread,
            });
        }

        RoboticsTaskSpec {
            category: RoboticsCategory::PointCloudClustering,
            description: format!(
                "Cluster {} points into {} groups (spread={:.1}).",
                num_clusters * pts_per_cluster,
                num_clusters,
                spread,
            ),
            size: num_clusters * pts_per_cluster,
            world_bounds: [20.0, 20.0, 5.0],
            obstacles,
            skills: Vec::new(),
            start: None,
            goal: None,
            formation: None,
        }
    }

    fn gen_avoidance(&self, difficulty: f32, rng: &mut impl Rng) -> RoboticsTaskSpec {
        let num_obstacles = if difficulty < 0.3 { 3 } else if difficulty < 0.7 { 8 } else { 15 };
        let mut obstacles = Vec::new();
        for _ in 0..num_obstacles {
            obstacles.push(TaskObstacle {
                center: [
                    rng.gen_range(1.0..9.0),
                    rng.gen_range(1.0..9.0),
                    0.0,
                ],
                radius: rng.gen_range(0.3..1.5),
            });
        }

        RoboticsTaskSpec {
            category: RoboticsCategory::ObstacleAvoidance,
            description: format!(
                "Plan a collision-free path through {} obstacles.",
                num_obstacles,
            ),
            size: num_obstacles,
            world_bounds: [10.0, 10.0, 1.0],
            obstacles,
            skills: Vec::new(),
            start: Some([0.0, 0.0, 0.0]),
            goal: Some([10.0, 10.0, 0.0]),
            formation: None,
        }
    }

    fn gen_scene_graph(&self, difficulty: f32, rng: &mut impl Rng) -> RoboticsTaskSpec {
        let num_objects = if difficulty < 0.3 { 3 } else if difficulty < 0.7 { 8 } else { 15 };
        let mut obstacles = Vec::new();
        for _ in 0..num_objects {
            obstacles.push(TaskObstacle {
                center: [
                    rng.gen_range(0.0..20.0),
                    rng.gen_range(0.0..20.0),
                    rng.gen_range(0.0..5.0),
                ],
                radius: rng.gen_range(0.5..2.0),
            });
        }

        RoboticsTaskSpec {
            category: RoboticsCategory::SceneGraphConstruction,
            description: format!(
                "Build a scene graph with spatial relations for {} objects.",
                num_objects,
            ),
            size: num_objects,
            world_bounds: [20.0, 20.0, 5.0],
            obstacles,
            skills: Vec::new(),
            start: None,
            goal: None,
            formation: None,
        }
    }

    fn gen_skill_sequencing(&self, difficulty: f32, _rng: &mut impl Rng) -> RoboticsTaskSpec {
        let skill_names = if difficulty < 0.3 {
            vec!["approach", "grasp", "lift"]
        } else if difficulty < 0.7 {
            vec!["scan", "approach", "align", "grasp", "lift", "place"]
        } else {
            vec![
                "scan", "classify", "approach", "align", "grasp",
                "lift", "navigate", "place", "verify", "retreat",
            ]
        };

        let skills: Vec<TaskSkill> = skill_names
            .iter()
            .enumerate()
            .map(|(i, &name)| TaskSkill {
                name: name.to_string(),
                preconditions: if i > 0 {
                    vec![format!("{}_done", skill_names[i - 1])]
                } else {
                    Vec::new()
                },
                effects: vec![format!("{}_done", name)],
                cost: (i as f32 + 1.0) * 0.5,
            })
            .collect();

        RoboticsTaskSpec {
            category: RoboticsCategory::SkillSequencing,
            description: format!(
                "Sequence {} skills to achieve a pick-and-place goal.",
                skills.len(),
            ),
            size: skills.len(),
            world_bounds: [10.0, 10.0, 3.0],
            obstacles: Vec::new(),
            skills,
            start: None,
            goal: None,
            formation: None,
        }
    }

    fn gen_swarm_formation(&self, difficulty: f32, _rng: &mut impl Rng) -> RoboticsTaskSpec {
        let num_robots = if difficulty < 0.3 { 4 } else if difficulty < 0.7 { 8 } else { 16 };
        let formation = if difficulty < 0.5 { "circle" } else { "grid" };

        RoboticsTaskSpec {
            category: RoboticsCategory::SwarmFormation,
            description: format!(
                "Assign {} robots to a {} formation.",
                num_robots, formation,
            ),
            size: num_robots,
            world_bounds: [20.0, 20.0, 1.0],
            obstacles: Vec::new(),
            skills: Vec::new(),
            start: None,
            goal: None,
            formation: Some(formation.to_string()),
        }
    }

    // -- evaluation helpers ------------------------------------------------

    fn score_clustering(&self, spec: &RoboticsTaskSpec, sol: &RoboticsSolution) -> Evaluation {
        let expected_clusters = spec.obstacles.len();
        let mut notes = Vec::new();

        if sol.cluster_ids.is_empty() {
            return Evaluation::zero(vec!["No cluster assignments".into()]);
        }

        let actual_clusters = *sol.cluster_ids.iter().max().unwrap_or(&0) + 1;
        let cluster_accuracy = if expected_clusters > 0 {
            1.0 - ((actual_clusters as f32 - expected_clusters as f32).abs()
                / expected_clusters as f32)
                .min(1.0)
        } else {
            0.0
        };

        let correctness = cluster_accuracy;
        let efficiency = if sol.cluster_ids.len() == spec.size { 1.0 } else { 0.5 };
        let elegance = if actual_clusters <= expected_clusters * 2 { 0.8 } else { 0.3 };

        if (actual_clusters as i32 - expected_clusters as i32).unsigned_abs() > 2 {
            notes.push(format!(
                "Expected ~{} clusters, got {}",
                expected_clusters, actual_clusters,
            ));
        }

        Evaluation {
            score: (0.6 * correctness + 0.25 * efficiency + 0.15 * elegance).clamp(0.0, 1.0),
            correctness,
            efficiency,
            elegance,
            constraint_results: Vec::new(),
            notes,
        }
    }

    fn score_avoidance(&self, spec: &RoboticsTaskSpec, sol: &RoboticsSolution) -> Evaluation {
        let mut notes = Vec::new();

        if sol.waypoints.is_empty() {
            return Evaluation::zero(vec!["Empty path".into()]);
        }

        // Check start/goal proximity.
        let start = spec.start.unwrap_or([0.0; 3]);
        let goal = spec.goal.unwrap_or([10.0, 10.0, 0.0]);
        let start_dist = dist3(&sol.waypoints[0], &start);
        let goal_dist = sol.waypoints.last().map_or(f64::MAX, |wp| dist3(wp, &goal));
        let reaches_goal = start_dist < 1.0 && goal_dist < 1.0;

        // Check collisions.
        let mut collisions = 0;
        for wp in &sol.waypoints {
            for obs in &spec.obstacles {
                if dist3(wp, &obs.center) < obs.radius {
                    collisions += 1;
                }
            }
        }

        let correctness = if reaches_goal { 0.6 } else { 0.2 }
            + (1.0 - (collisions as f32 / (sol.waypoints.len() * spec.obstacles.len()).max(1) as f32).min(1.0)) * 0.4;
        let efficiency = 1.0 - (sol.waypoints.len() as f32 / 100.0).min(1.0);
        let elegance = if collisions == 0 { 0.9 } else { 0.3 };

        if collisions > 0 {
            notes.push(format!("{} collision(s) detected", collisions));
        }
        if !reaches_goal {
            notes.push("Path does not reach goal".into());
        }

        Evaluation {
            score: (0.6 * correctness + 0.25 * efficiency + 0.15 * elegance).clamp(0.0, 1.0),
            correctness,
            efficiency,
            elegance,
            constraint_results: Vec::new(),
            notes,
        }
    }

    fn score_scene_graph(&self, spec: &RoboticsTaskSpec, sol: &RoboticsSolution) -> Evaluation {
        let expected_objects = spec.obstacles.len();
        let mut notes = Vec::new();

        if sol.edges.is_empty() && expected_objects > 1 {
            notes.push("No edges in scene graph".into());
        }

        // Check node coverage.
        let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for &(a, b) in &sol.edges {
            seen.insert(a);
            seen.insert(b);
        }
        let coverage = if expected_objects > 0 {
            seen.len() as f32 / expected_objects as f32
        } else {
            1.0
        };

        let correctness = coverage.min(1.0);
        let max_edges = if expected_objects >= 2 {
            expected_objects * (expected_objects - 1) / 2
        } else {
            0
        };
        let efficiency = if sol.edges.len() <= max_edges {
            0.9
        } else {
            0.5
        };
        let elegance = if coverage >= 0.8 { 0.8 } else { 0.4 };

        Evaluation {
            score: (0.6 * correctness + 0.25 * efficiency + 0.15 * elegance).clamp(0.0, 1.0),
            correctness,
            efficiency,
            elegance,
            constraint_results: Vec::new(),
            notes,
        }
    }

    fn score_skill_sequence(&self, spec: &RoboticsTaskSpec, sol: &RoboticsSolution) -> Evaluation {
        let mut notes = Vec::new();

        if sol.skill_sequence.is_empty() {
            return Evaluation::zero(vec!["Empty skill sequence".into()]);
        }

        // Check dependency ordering.
        let mut violations = 0;
        for (i, name) in sol.skill_sequence.iter().enumerate() {
            if let Some(skill) = spec.skills.iter().find(|s| &s.name == name) {
                for pre in &skill.preconditions {
                    // Precondition must appear earlier.
                    let pre_skill = pre.trim_end_matches("_done");
                    let pre_pos = sol.skill_sequence.iter().position(|s| s == pre_skill);
                    if let Some(pp) = pre_pos {
                        if pp >= i {
                            violations += 1;
                            notes.push(format!("{} before its precondition {}", name, pre_skill));
                        }
                    } else {
                        violations += 1;
                        notes.push(format!("Missing precondition {} for {}", pre_skill, name));
                    }
                }
            }
        }

        let expected_skills = spec.skills.len();
        let coverage = sol.skill_sequence.len() as f32 / expected_skills.max(1) as f32;
        let dep_penalty = violations as f32 / expected_skills.max(1) as f32;

        let correctness = (coverage.min(1.0) * (1.0 - dep_penalty.min(1.0))).max(0.0);
        let efficiency = if sol.skill_sequence.len() <= expected_skills + 2 { 0.9 } else { 0.5 };
        let elegance = if violations == 0 { 0.9 } else { 0.3 };

        Evaluation {
            score: (0.6 * correctness + 0.25 * efficiency + 0.15 * elegance).clamp(0.0, 1.0),
            correctness,
            efficiency,
            elegance,
            constraint_results: Vec::new(),
            notes,
        }
    }

    fn score_formation(&self, spec: &RoboticsTaskSpec, sol: &RoboticsSolution) -> Evaluation {
        let expected_robots = spec.size;
        let mut notes = Vec::new();

        if sol.waypoints.is_empty() {
            return Evaluation::zero(vec!["No formation positions".into()]);
        }

        let correctness = (sol.waypoints.len() as f32 / expected_robots.max(1) as f32).min(1.0);

        // Check positions are within bounds.
        let bounds = &spec.world_bounds;
        let in_bounds = sol
            .waypoints
            .iter()
            .filter(|w| {
                w[0].abs() <= bounds[0] && w[1].abs() <= bounds[1] && w[2].abs() <= bounds[2]
            })
            .count() as f32
            / sol.waypoints.len().max(1) as f32;
        let efficiency = in_bounds;

        // Check for collisions between robots (min spacing).
        let mut too_close = 0;
        for i in 0..sol.waypoints.len() {
            for j in (i + 1)..sol.waypoints.len() {
                if dist3(&sol.waypoints[i], &sol.waypoints[j]) < 0.5 {
                    too_close += 1;
                }
            }
        }
        let elegance = if too_close == 0 { 0.9 } else { 0.4 };
        if too_close > 0 {
            notes.push(format!("{} robot pair(s) too close (<0.5m)", too_close));
        }

        Evaluation {
            score: (0.6 * correctness + 0.25 * efficiency + 0.15 * elegance).clamp(0.0, 1.0),
            correctness,
            efficiency,
            elegance,
            constraint_results: Vec::new(),
            notes,
        }
    }

    // -- embedding ---------------------------------------------------------

    fn extract_features(&self, solution: &Solution) -> Vec<f32> {
        let content = &solution.content;
        let mut features = vec![0.0f32; EMBEDDING_DIM];

        // Parse the structured solution if present.
        let sol: RoboticsSolution = serde_json::from_value(solution.data.clone())
            .or_else(|_| serde_json::from_str(content))
            .unwrap_or(RoboticsSolution {
                waypoints: Vec::new(),
                cluster_ids: Vec::new(),
                skill_sequence: Vec::new(),
                edges: Vec::new(),
            });

        // Feature 0-7: Solution structure.
        features[0] = sol.waypoints.len() as f32 / 50.0;
        features[1] = sol.cluster_ids.len() as f32 / 100.0;
        features[2] = sol.skill_sequence.len() as f32 / 20.0;
        features[3] = sol.edges.len() as f32 / 50.0;
        // Unique clusters.
        let unique_clusters: std::collections::HashSet<&usize> = sol.cluster_ids.iter().collect();
        features[4] = unique_clusters.len() as f32 / 20.0;
        // Unique skills.
        let unique_skills: std::collections::HashSet<&String> = sol.skill_sequence.iter().collect();
        features[5] = unique_skills.len() as f32 / 20.0;
        // Spatial extent of waypoints.
        if !sol.waypoints.is_empty() {
            let max_dist = sol
                .waypoints
                .iter()
                .map(|w| (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt() as f32)
                .fold(0.0f32, f32::max);
            features[6] = max_dist / 50.0;
        }

        // Feature 8-15: Text-based features (cross-domain compatible).
        features[8] = content.matches("cluster").count() as f32 / 5.0;
        features[9] = content.matches("obstacle").count() as f32 / 5.0;
        features[10] = content.matches("path").count() as f32 / 5.0;
        features[11] = content.matches("scene").count() as f32 / 3.0;
        features[12] = content.matches("formation").count() as f32 / 3.0;
        features[13] = content.matches("skill").count() as f32 / 5.0;
        features[14] = content.matches("robot").count() as f32 / 5.0;
        features[15] = content.matches("point").count() as f32 / 10.0;

        // Feature 16-23: Spatial reasoning indicators.
        features[16] = content.matches("distance").count() as f32 / 5.0;
        features[17] = content.matches("position").count() as f32 / 5.0;
        features[18] = content.matches("radius").count() as f32 / 3.0;
        features[19] = content.matches("collision").count() as f32 / 3.0;
        features[20] = content.matches("adjacent").count() as f32 / 3.0;
        features[21] = content.matches("near").count() as f32 / 3.0;
        features[22] = content.matches("velocity").count() as f32 / 3.0;
        features[23] = content.matches("trajectory").count() as f32 / 3.0;

        // Feature 32-39: Planning overlap (cross-domain with PlanningDomain).
        features[32] = content.matches("allocate").count() as f32 / 3.0;
        features[33] = content.matches("schedule").count() as f32 / 3.0;
        features[34] = content.matches("constraint").count() as f32 / 3.0;
        features[35] = content.matches("goal").count() as f32 / 3.0;
        features[36] = content.matches("precondition").count() as f32 / 3.0;
        features[37] = content.matches("parallel").count() as f32 / 3.0;
        features[38] = content.matches("sequence").count() as f32 / 3.0;
        features[39] = content.matches("assign").count() as f32 / 3.0;

        // Feature 48-55: Orchestration overlap (cross-domain with ToolOrchestration).
        features[48] = content.matches("pipeline").count() as f32 / 3.0;
        features[49] = content.matches("sensor").count() as f32 / 3.0;
        features[50] = content.matches("fuse").count() as f32 / 2.0;
        features[51] = content.matches("detect").count() as f32 / 3.0;
        features[52] = content.matches("track").count() as f32 / 3.0;
        features[53] = content.matches("coordinate").count() as f32 / 3.0;
        features[54] = content.matches("merge").count() as f32 / 3.0;
        features[55] = content.matches("update").count() as f32 / 3.0;

        // Normalize to unit length.
        let norm: f32 = features.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-10 {
            for f in &mut features {
                *f /= norm;
            }
        }

        features
    }
}

impl Default for RoboticsDomain {
    fn default() -> Self {
        Self::new()
    }
}

impl Domain for RoboticsDomain {
    fn id(&self) -> &DomainId {
        &self.id
    }

    fn name(&self) -> &str {
        "Cognitive Robotics"
    }

    fn generate_tasks(&self, count: usize, difficulty: f32) -> Vec<Task> {
        let mut rng = rand::thread_rng();
        let difficulty = difficulty.clamp(0.0, 1.0);

        (0..count)
            .map(|i| {
                let roll: f32 = rng.gen();
                let spec = if roll < 0.2 {
                    self.gen_clustering(difficulty, &mut rng)
                } else if roll < 0.4 {
                    self.gen_avoidance(difficulty, &mut rng)
                } else if roll < 0.6 {
                    self.gen_scene_graph(difficulty, &mut rng)
                } else if roll < 0.8 {
                    self.gen_skill_sequencing(difficulty, &mut rng)
                } else {
                    self.gen_swarm_formation(difficulty, &mut rng)
                };

                Task {
                    id: format!("robotics_{}_d{:.0}", i, difficulty * 100.0),
                    domain_id: self.id.clone(),
                    difficulty,
                    spec: serde_json::to_value(&spec).unwrap_or_default(),
                    constraints: Vec::new(),
                }
            })
            .collect()
    }

    fn evaluate(&self, task: &Task, solution: &Solution) -> Evaluation {
        let spec: RoboticsTaskSpec = match serde_json::from_value(task.spec.clone()) {
            Ok(s) => s,
            Err(e) => return Evaluation::zero(vec![format!("Invalid task spec: {}", e)]),
        };

        let sol: RoboticsSolution = serde_json::from_value(solution.data.clone())
            .or_else(|_| serde_json::from_str(&solution.content))
            .unwrap_or(RoboticsSolution {
                waypoints: Vec::new(),
                cluster_ids: Vec::new(),
                skill_sequence: Vec::new(),
                edges: Vec::new(),
            });

        match spec.category {
            RoboticsCategory::PointCloudClustering => self.score_clustering(&spec, &sol),
            RoboticsCategory::ObstacleAvoidance => self.score_avoidance(&spec, &sol),
            RoboticsCategory::SceneGraphConstruction => self.score_scene_graph(&spec, &sol),
            RoboticsCategory::SkillSequencing => self.score_skill_sequence(&spec, &sol),
            RoboticsCategory::SwarmFormation => self.score_formation(&spec, &sol),
        }
    }

    fn embed(&self, solution: &Solution) -> DomainEmbedding {
        let features = self.extract_features(solution);
        DomainEmbedding::new(features, self.id.clone())
    }

    fn embedding_dim(&self) -> usize {
        EMBEDDING_DIM
    }

    fn reference_solution(&self, task: &Task) -> Option<Solution> {
        let spec: RoboticsTaskSpec = serde_json::from_value(task.spec.clone()).ok()?;

        let sol = match spec.category {
            RoboticsCategory::PointCloudClustering => {
                // Assign each point-group to its own cluster.
                let num_clusters = spec.obstacles.len().max(1);
                let cluster_ids: Vec<usize> = (0..spec.size)
                    .map(|i| i * num_clusters / spec.size.max(1))
                    .collect();
                RoboticsSolution {
                    waypoints: Vec::new(),
                    cluster_ids,
                    skill_sequence: Vec::new(),
                    edges: Vec::new(),
                }
            }
            RoboticsCategory::ObstacleAvoidance => {
                // Straight-line path (naive reference).
                let start = spec.start.unwrap_or([0.0; 3]);
                let goal = spec.goal.unwrap_or([10.0, 10.0, 0.0]);
                let steps = 10;
                let waypoints: Vec<[f64; 3]> = (0..=steps)
                    .map(|s| {
                        let t = s as f64 / steps as f64;
                        [
                            start[0] + (goal[0] - start[0]) * t,
                            start[1] + (goal[1] - start[1]) * t,
                            start[2] + (goal[2] - start[2]) * t,
                        ]
                    })
                    .collect();
                RoboticsSolution {
                    waypoints,
                    cluster_ids: Vec::new(),
                    skill_sequence: Vec::new(),
                    edges: Vec::new(),
                }
            }
            RoboticsCategory::SceneGraphConstruction => {
                // Connect all pairs within distance 10.
                let mut edges = Vec::new();
                for i in 0..spec.obstacles.len() {
                    for j in (i + 1)..spec.obstacles.len() {
                        let d = dist3(&spec.obstacles[i].center, &spec.obstacles[j].center);
                        if d < 10.0 {
                            edges.push((i, j));
                        }
                    }
                }
                RoboticsSolution {
                    waypoints: Vec::new(),
                    cluster_ids: Vec::new(),
                    skill_sequence: Vec::new(),
                    edges,
                }
            }
            RoboticsCategory::SkillSequencing => {
                let skill_sequence: Vec<String> =
                    spec.skills.iter().map(|s| s.name.clone()).collect();
                RoboticsSolution {
                    waypoints: Vec::new(),
                    cluster_ids: Vec::new(),
                    skill_sequence,
                    edges: Vec::new(),
                }
            }
            RoboticsCategory::SwarmFormation => {
                // Circle formation.
                let n = spec.size;
                let waypoints: Vec<[f64; 3]> = (0..n)
                    .map(|i| {
                        let angle = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                        [5.0 * angle.cos(), 5.0 * angle.sin(), 0.0]
                    })
                    .collect();
                RoboticsSolution {
                    waypoints,
                    cluster_ids: Vec::new(),
                    skill_sequence: Vec::new(),
                    edges: Vec::new(),
                }
            }
        };

        let content = serde_json::to_string_pretty(&sol).ok()?;
        Some(Solution {
            task_id: task.id.clone(),
            content,
            data: serde_json::to_value(&sol).ok()?,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn dist3(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_robotics_domain_id() {
        let domain = RoboticsDomain::new();
        assert_eq!(domain.id().0, "robotics");
        assert_eq!(domain.name(), "Cognitive Robotics");
    }

    #[test]
    fn test_generate_tasks_all_difficulties() {
        let domain = RoboticsDomain::new();
        for &d in &[0.1, 0.5, 0.9] {
            let tasks = domain.generate_tasks(10, d);
            assert_eq!(tasks.len(), 10);
            for task in &tasks {
                assert_eq!(task.domain_id, *domain.id());
            }
        }
    }

    #[test]
    fn test_reference_solution_exists() {
        let domain = RoboticsDomain::new();
        let tasks = domain.generate_tasks(20, 0.5);
        for task in &tasks {
            let ref_sol = domain.reference_solution(task);
            assert!(ref_sol.is_some(), "Reference solution missing for {}", task.id);
        }
    }

    #[test]
    fn test_evaluate_reference_solutions() {
        let domain = RoboticsDomain::new();
        let tasks = domain.generate_tasks(20, 0.3);
        for task in &tasks {
            if let Some(solution) = domain.reference_solution(task) {
                let eval = domain.evaluate(task, &solution);
                assert!(
                    eval.score >= 0.0 && eval.score <= 1.0,
                    "Score out of range for {}: {}",
                    task.id,
                    eval.score,
                );
            }
        }
    }

    #[test]
    fn test_embedding_dimension() {
        let domain = RoboticsDomain::new();
        assert_eq!(domain.embedding_dim(), EMBEDDING_DIM);

        let sol = Solution {
            task_id: "test".into(),
            content: "cluster points into groups near obstacles using distance threshold".into(),
            data: serde_json::Value::Null,
        };
        let embedding = domain.embed(&sol);
        assert_eq!(embedding.dim, EMBEDDING_DIM);
        assert_eq!(embedding.vector.len(), EMBEDDING_DIM);
    }

    #[test]
    fn test_cross_domain_embedding_compatibility() {
        let domain = RoboticsDomain::new();

        let robotics_sol = Solution {
            task_id: "r1".into(),
            content: "plan path through obstacles avoiding collision with distance checks".into(),
            data: serde_json::Value::Null,
        };
        let robotics_emb = domain.embed(&robotics_sol);

        // Embedding should be same dimension as other domains (64).
        assert_eq!(robotics_emb.dim, 64);

        // Cosine similarity with itself should be 1.0.
        let self_sim = robotics_emb.cosine_similarity(&robotics_emb);
        assert!((self_sim - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_score_skill_ordering_violation() {
        let domain = RoboticsDomain::new();

        let spec = RoboticsTaskSpec {
            category: RoboticsCategory::SkillSequencing,
            description: "test".into(),
            size: 3,
            world_bounds: [10.0, 10.0, 3.0],
            obstacles: Vec::new(),
            skills: vec![
                TaskSkill {
                    name: "approach".into(),
                    preconditions: Vec::new(),
                    effects: vec!["approach_done".into()],
                    cost: 1.0,
                },
                TaskSkill {
                    name: "grasp".into(),
                    preconditions: vec!["approach_done".into()],
                    effects: vec!["grasp_done".into()],
                    cost: 1.0,
                },
                TaskSkill {
                    name: "lift".into(),
                    preconditions: vec!["grasp_done".into()],
                    effects: vec!["lift_done".into()],
                    cost: 1.0,
                },
            ],
            start: None,
            goal: None,
            formation: None,
        };

        // Correct ordering.
        let good = RoboticsSolution {
            waypoints: Vec::new(),
            cluster_ids: Vec::new(),
            skill_sequence: vec!["approach".into(), "grasp".into(), "lift".into()],
            edges: Vec::new(),
        };
        let good_eval = domain.score_skill_sequence(&spec, &good);
        assert!(good_eval.correctness > 0.5);

        // Bad ordering (reversed).
        let bad = RoboticsSolution {
            waypoints: Vec::new(),
            cluster_ids: Vec::new(),
            skill_sequence: vec!["lift".into(), "grasp".into(), "approach".into()],
            edges: Vec::new(),
        };
        let bad_eval = domain.score_skill_sequence(&spec, &bad);
        assert!(bad_eval.score < good_eval.score);
    }

    #[test]
    fn test_engine_with_robotics_domain() {
        use ruvector_domain_expansion::DomainExpansionEngine;

        let mut engine = DomainExpansionEngine::new();
        engine.register_domain(Box::new(RoboticsDomain::new()));

        let ids = engine.domain_ids();
        // 3 built-in + robotics = 4.
        assert_eq!(ids.len(), 4);
        assert!(ids.iter().any(|id| id.0 == "robotics"));

        // Generate tasks from robotics domain.
        let domain_id = DomainId("robotics".into());
        let tasks = engine.generate_tasks(&domain_id, 5, 0.5);
        assert_eq!(tasks.len(), 5);

        // Embed a robotics solution.
        let sol = Solution {
            task_id: "r".into(),
            content: "navigate robot through obstacle field using sensor fusion pipeline".into(),
            data: serde_json::Value::Null,
        };
        let emb = engine.embed(&domain_id, &sol);
        assert!(emb.is_some());
        assert_eq!(emb.unwrap().dim, 64);
    }

    #[test]
    fn test_transfer_from_planning_to_robotics() {
        use ruvector_domain_expansion::transfer::{ArmId, ContextBucket};
        use ruvector_domain_expansion::DomainExpansionEngine;

        let mut engine = DomainExpansionEngine::new();
        engine.register_domain(Box::new(RoboticsDomain::new()));

        let planning_id = DomainId("structured_planning".into());
        let robotics_id = DomainId("robotics".into());

        let bucket = ContextBucket {
            difficulty_tier: "medium".into(),
            category: "spatial".into(),
        };

        // Train on planning domain.
        for _ in 0..30 {
            engine.thompson.record_outcome(
                &planning_id,
                bucket.clone(),
                ArmId("greedy".into()),
                0.85,
                1.0,
            );
        }

        // Transfer to robotics.
        engine.initiate_transfer(&planning_id, &robotics_id);

        // Verify transfer priors are seeded.
        let arm = engine.select_arm(&robotics_id, &bucket);
        assert!(arm.is_some());
    }
}
