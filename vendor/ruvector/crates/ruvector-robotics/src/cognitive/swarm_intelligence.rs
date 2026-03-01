//! Multi-robot swarm coordination.
//!
//! Provides formation computation, capability-based task assignment, and
//! simple majority consensus for distributed decision making.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Formation
// ---------------------------------------------------------------------------

/// Types of spatial formations a swarm can adopt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FormationType {
    Line,
    Circle,
    Grid,
    Custom(Vec<[f64; 3]>),
}

/// A formation specification: type, spacing, and center point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Formation {
    pub formation_type: FormationType,
    pub spacing: f64,
    pub center: [f64; 3],
}

// ---------------------------------------------------------------------------
// Robots & tasks
// ---------------------------------------------------------------------------

/// Capabilities advertised by a single robot in the swarm.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RobotCapabilities {
    pub id: u64,
    pub max_speed: f64,
    pub payload: f64,
    pub sensors: Vec<String>,
}

/// A task to be assigned to one or more robots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwarmTask {
    pub id: u64,
    pub description: String,
    pub location: [f64; 3],
    pub required_capabilities: Vec<String>,
    pub priority: u8,
}

/// The result of assigning a task to a robot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub robot_id: u64,
    pub task_id: u64,
    pub estimated_completion: f64,
}

// ---------------------------------------------------------------------------
// Consensus
// ---------------------------------------------------------------------------

/// The result of a consensus vote among swarm members.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsensusResult {
    pub proposal: String,
    pub votes_for: usize,
    pub votes_against: usize,
    pub accepted: bool,
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Configuration for the swarm coordinator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwarmConfig {
    pub max_robots: usize,
    pub communication_range: f64,
    pub consensus_threshold: f64,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            max_robots: 10,
            communication_range: 50.0,
            consensus_threshold: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Coordinator
// ---------------------------------------------------------------------------

/// Coordinates a swarm of robots for formation, task assignment, and consensus.
#[derive(Debug, Clone)]
pub struct SwarmCoordinator {
    config: SwarmConfig,
    robots: HashMap<u64, RobotCapabilities>,
}

impl SwarmCoordinator {
    /// Create a new coordinator with the given configuration.
    pub fn new(config: SwarmConfig) -> Self {
        Self {
            config,
            robots: HashMap::new(),
        }
    }

    /// Register a robot's capabilities. Respects `max_robots`.
    pub fn register_robot(&mut self, capabilities: RobotCapabilities) -> bool {
        if self.robots.len() >= self.config.max_robots {
            return false;
        }
        self.robots.insert(capabilities.id, capabilities);
        true
    }

    /// Number of registered robots.
    pub fn robot_count(&self) -> usize {
        self.robots.len()
    }

    /// Assign tasks to robots using a greedy capability-matching strategy.
    ///
    /// Tasks are processed in priority order (highest first). Each task is
    /// assigned to the first unassigned robot that possesses all required
    /// capabilities.
    pub fn assign_tasks(&self, tasks: &[SwarmTask]) -> Vec<TaskAssignment> {
        let mut sorted_tasks: Vec<&SwarmTask> = tasks.iter().collect();
        sorted_tasks.sort_by(|a, b| b.priority.cmp(&a.priority));

        let mut assigned_robots: HashSet<u64> = HashSet::new();
        let mut assignments = Vec::new();

        for task in &sorted_tasks {
            for (id, caps) in &self.robots {
                if assigned_robots.contains(id) {
                    continue;
                }
                let has_caps = task
                    .required_capabilities
                    .iter()
                    .all(|req| caps.sensors.contains(req));
                if has_caps {
                    // Estimate completion as distance / speed.
                    let dx = task.location[0];
                    let dy = task.location[1];
                    let dz = task.location[2];
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    let est = if caps.max_speed > 0.0 {
                        dist / caps.max_speed
                    } else {
                        f64::INFINITY
                    };

                    assignments.push(TaskAssignment {
                        robot_id: *id,
                        task_id: task.id,
                        estimated_completion: est,
                    });
                    assigned_robots.insert(*id);
                    break;
                }
            }
        }

        assignments
    }

    /// Compute target positions for each robot given a formation spec.
    pub fn compute_formation(&self, formation: &Formation) -> Vec<[f64; 3]> {
        let n = self.robots.len();
        if n == 0 {
            return Vec::new();
        }

        match &formation.formation_type {
            FormationType::Line => (0..n)
                .map(|i| {
                    let offset = (i as f64 - (n as f64 - 1.0) / 2.0) * formation.spacing;
                    [
                        formation.center[0] + offset,
                        formation.center[1],
                        formation.center[2],
                    ]
                })
                .collect(),

            FormationType::Circle => {
                let radius = formation.spacing * n as f64 / (2.0 * std::f64::consts::PI);
                (0..n)
                    .map(|i| {
                        let angle = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
                        [
                            formation.center[0] + radius * angle.cos(),
                            formation.center[1] + radius * angle.sin(),
                            formation.center[2],
                        ]
                    })
                    .collect()
            }

            FormationType::Grid => {
                let cols = (n as f64).sqrt().ceil() as usize;
                (0..n)
                    .map(|i| {
                        let row = i / cols;
                        let col = i % cols;
                        [
                            formation.center[0] + col as f64 * formation.spacing,
                            formation.center[1] + row as f64 * formation.spacing,
                            formation.center[2],
                        ]
                    })
                    .collect()
            }

            FormationType::Custom(positions) => positions.clone(),
        }
    }

    /// Run a simple majority consensus vote among all registered robots.
    ///
    /// Each robot "votes" deterministically based on its ID parity (a
    /// placeholder for real voting logic). The proposal is accepted when
    /// the fraction of `for` votes meets the threshold.
    pub fn propose_consensus(&self, proposal: &str) -> ConsensusResult {
        let total = self.robots.len();
        if total == 0 {
            return ConsensusResult {
                proposal: proposal.to_string(),
                votes_for: 0,
                votes_against: 0,
                accepted: false,
            };
        }

        // Deterministic placeholder vote: even IDs vote for, odd against.
        let votes_for = self.robots.keys().filter(|id| *id % 2 == 0).count();
        let votes_against = total - votes_for;
        let ratio = votes_for as f64 / total as f64;

        ConsensusResult {
            proposal: proposal.to_string(),
            votes_for,
            votes_against,
            accepted: ratio >= self.config.consensus_threshold,
        }
    }

    /// Read-only access to the configuration.
    pub fn config(&self) -> &SwarmConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_robot(id: u64, sensors: Vec<&str>) -> RobotCapabilities {
        RobotCapabilities {
            id,
            max_speed: 1.0,
            payload: 5.0,
            sensors: sensors.into_iter().map(String::from).collect(),
        }
    }

    fn make_task(id: u64, caps: Vec<&str>, priority: u8) -> SwarmTask {
        SwarmTask {
            id,
            description: format!("task_{}", id),
            location: [3.0, 4.0, 0.0],
            required_capabilities: caps.into_iter().map(String::from).collect(),
            priority,
        }
    }

    #[test]
    fn test_register_robot() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        assert!(coord.register_robot(make_robot(1, vec!["lidar"])));
        assert_eq!(coord.robot_count(), 1);
    }

    #[test]
    fn test_register_respects_max() {
        let mut coord = SwarmCoordinator::new(SwarmConfig {
            max_robots: 1,
            ..SwarmConfig::default()
        });
        assert!(coord.register_robot(make_robot(1, vec![])));
        assert!(!coord.register_robot(make_robot(2, vec![])));
    }

    #[test]
    fn test_assign_tasks_capability_match() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        coord.register_robot(make_robot(1, vec!["camera"]));
        coord.register_robot(make_robot(2, vec!["lidar"]));

        let tasks = vec![make_task(10, vec!["lidar"], 5)];
        let assignments = coord.assign_tasks(&tasks);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].robot_id, 2);
        assert_eq!(assignments[0].task_id, 10);
    }

    #[test]
    fn test_assign_no_capable_robot() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        coord.register_robot(make_robot(1, vec!["camera"]));
        let tasks = vec![make_task(10, vec!["sonar"], 5)];
        let assignments = coord.assign_tasks(&tasks);
        assert!(assignments.is_empty());
    }

    #[test]
    fn test_line_formation() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        coord.register_robot(make_robot(1, vec![]));
        coord.register_robot(make_robot(2, vec![]));
        coord.register_robot(make_robot(3, vec![]));
        let formation = Formation {
            formation_type: FormationType::Line,
            spacing: 2.0,
            center: [0.0, 0.0, 0.0],
        };
        let positions = coord.compute_formation(&formation);
        assert_eq!(positions.len(), 3);
    }

    #[test]
    fn test_circle_formation() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        for i in 0..4 {
            coord.register_robot(make_robot(i, vec![]));
        }
        let formation = Formation {
            formation_type: FormationType::Circle,
            spacing: 2.0,
            center: [0.0, 0.0, 0.0],
        };
        let positions = coord.compute_formation(&formation);
        assert_eq!(positions.len(), 4);
    }

    #[test]
    fn test_consensus_accepted() {
        let mut coord = SwarmCoordinator::new(SwarmConfig {
            consensus_threshold: 0.5,
            ..SwarmConfig::default()
        });
        // Even IDs vote for.
        coord.register_robot(make_robot(2, vec![]));
        coord.register_robot(make_robot(4, vec![]));
        coord.register_robot(make_robot(5, vec![]));
        let result = coord.propose_consensus("explore area B");
        assert_eq!(result.votes_for, 2);
        assert_eq!(result.votes_against, 1);
        assert!(result.accepted);
    }

    #[test]
    fn test_consensus_rejected() {
        let mut coord = SwarmCoordinator::new(SwarmConfig {
            consensus_threshold: 0.8,
            ..SwarmConfig::default()
        });
        coord.register_robot(make_robot(1, vec![])); // odd -> against
        coord.register_robot(make_robot(3, vec![])); // odd -> against
        coord.register_robot(make_robot(2, vec![])); // even -> for
        let result = coord.propose_consensus("attack");
        assert!(!result.accepted);
    }

    #[test]
    fn test_consensus_empty_swarm() {
        let coord = SwarmCoordinator::new(SwarmConfig::default());
        let result = coord.propose_consensus("noop");
        assert!(!result.accepted);
        assert_eq!(result.votes_for, 0);
    }

    #[test]
    fn test_grid_formation() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        for i in 0..4 {
            coord.register_robot(make_robot(i, vec![]));
        }
        let formation = Formation {
            formation_type: FormationType::Grid,
            spacing: 1.0,
            center: [0.0, 0.0, 0.0],
        };
        let positions = coord.compute_formation(&formation);
        assert_eq!(positions.len(), 4);
        // 4 robots => 2x2 grid
        assert!((positions[0][0] - 0.0).abs() < 1e-9);
        assert!((positions[1][0] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_custom_formation() {
        let mut coord = SwarmCoordinator::new(SwarmConfig::default());
        coord.register_robot(make_robot(1, vec![]));
        let custom_pos = vec![[10.0, 20.0, 0.0]];
        let formation = Formation {
            formation_type: FormationType::Custom(custom_pos.clone()),
            spacing: 0.0,
            center: [0.0, 0.0, 0.0],
        };
        let positions = coord.compute_formation(&formation);
        assert_eq!(positions, custom_pos);
    }
}
