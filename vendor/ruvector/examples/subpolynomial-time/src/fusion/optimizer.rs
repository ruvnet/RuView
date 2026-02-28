//! Optimizer: Maintenance Planning and Actions
//!
//! Provides optimization actions and maintenance planning based on
//! structural monitor signals.

use super::structural_monitor::{BrittlenessSignal, StructuralMonitor, TriggerType};
use std::collections::HashMap;

/// Optimization action types
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizerAction {
    /// Reindex: rebuild vector similarity edges
    Reindex {
        /// Affected nodes
        nodes: Vec<u64>,
        /// New similarity threshold
        new_threshold: Option<f64>,
    },
    /// Rewire: adjust edge capacities
    Rewire {
        /// Edges to strengthen
        strengthen: Vec<(u64, u64, f64)>,
        /// Edges to weaken
        weaken: Vec<(u64, u64, f64)>,
    },
    /// Split shard: divide a partition
    SplitShard {
        /// Shard ID to split
        shard_id: u64,
        /// Split point (if applicable)
        split_at: Option<u64>,
    },
    /// Merge shards: combine partitions
    MergeShards {
        /// Shard IDs to merge
        shard_ids: Vec<u64>,
    },
    /// Learning gate: enable/disable self-learning
    LearningGate {
        /// Whether to enable learning
        enable: bool,
        /// Learning rate adjustment
        rate_multiplier: f64,
    },
    /// No operation needed
    NoOp,
}

/// Learning gate controller
#[derive(Debug, Clone)]
pub struct LearningGate {
    /// Whether learning is enabled
    pub enabled: bool,
    /// Current learning rate
    pub learning_rate: f64,
    /// Base learning rate
    pub base_rate: f64,
    /// Minimum rate before disabling
    pub min_rate: f64,
    /// Maximum rate
    pub max_rate: f64,
}

impl Default for LearningGate {
    fn default() -> Self {
        Self {
            enabled: true,
            learning_rate: 0.01,
            base_rate: 0.01,
            min_rate: 0.001,
            max_rate: 0.1,
        }
    }
}

impl LearningGate {
    /// Create new learning gate
    pub fn new(base_rate: f64) -> Self {
        Self {
            learning_rate: base_rate,
            base_rate,
            ..Default::default()
        }
    }

    /// Adjust learning rate based on signal
    pub fn adjust(&mut self, signal: BrittlenessSignal) {
        match signal {
            BrittlenessSignal::Healthy => {
                // Increase learning rate when stable
                self.learning_rate = (self.learning_rate * 1.1).min(self.max_rate);
            }
            BrittlenessSignal::Warning => {
                // Keep current rate
            }
            BrittlenessSignal::Critical | BrittlenessSignal::Disconnected => {
                // Reduce learning to avoid further instability
                self.learning_rate = (self.learning_rate * 0.5).max(self.min_rate);
                if self.learning_rate <= self.min_rate {
                    self.enabled = false;
                }
            }
        }
    }

    /// Reset to defaults
    pub fn reset(&mut self) {
        self.enabled = true;
        self.learning_rate = self.base_rate;
    }
}

/// A maintenance task
#[derive(Debug, Clone)]
pub struct MaintenanceTask {
    /// Task ID
    pub id: u64,
    /// Action to perform
    pub action: OptimizerAction,
    /// Priority (higher = more urgent)
    pub priority: u8,
    /// Estimated cost (1-10)
    pub cost: u8,
    /// Expected benefit description
    pub benefit: String,
    /// Whether the task is critical
    pub critical: bool,
}

impl MaintenanceTask {
    /// Create new maintenance task
    pub fn new(id: u64, action: OptimizerAction, priority: u8) -> Self {
        let (cost, critical) = match &action {
            OptimizerAction::Reindex { nodes, .. } => {
                (if nodes.len() > 100 { 8 } else { 4 }, false)
            }
            OptimizerAction::Rewire {
                strengthen, weaken, ..
            } => ((strengthen.len() + weaken.len()).min(10) as u8, false),
            OptimizerAction::SplitShard { .. } => (6, false),
            OptimizerAction::MergeShards { shard_ids } => (shard_ids.len().min(10) as u8, false),
            OptimizerAction::LearningGate { enable, .. } => {
                if *enable {
                    (1, false)
                } else {
                    (2, true)
                }
            }
            OptimizerAction::NoOp => (0, false),
        };

        let benefit = match &action {
            OptimizerAction::Reindex { .. } => "Refresh vector similarity edges".to_string(),
            OptimizerAction::Rewire { .. } => "Adjust edge weights for better balance".to_string(),
            OptimizerAction::SplitShard { .. } => {
                "Reduce partition size for better locality".to_string()
            }
            OptimizerAction::MergeShards { .. } => {
                "Combine sparse partitions for density".to_string()
            }
            OptimizerAction::LearningGate { enable, .. } => {
                if *enable {
                    "Re-enable learning for adaptation".to_string()
                } else {
                    "Pause learning to stabilize".to_string()
                }
            }
            OptimizerAction::NoOp => "No action needed".to_string(),
        };

        Self {
            id,
            action,
            priority,
            cost,
            benefit,
            critical,
        }
    }
}

/// A maintenance plan
#[derive(Debug, Clone, Default)]
pub struct MaintenancePlan {
    /// Ordered list of tasks
    pub tasks: Vec<MaintenanceTask>,
    /// Total estimated cost
    pub total_cost: u32,
    /// Plan generation timestamp
    pub created_at: u64,
    /// Human-readable summary
    pub summary: String,
}

impl MaintenancePlan {
    /// Create a new plan
    pub fn new() -> Self {
        Self {
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ..Default::default()
        }
    }

    /// Add a task to the plan
    pub fn add_task(&mut self, task: MaintenanceTask) {
        self.total_cost += u32::from(task.cost);
        self.tasks.push(task);
        self.update_summary();
    }

    /// Sort tasks by priority (highest first)
    pub fn prioritize(&mut self) {
        self.tasks.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Get critical tasks only
    pub fn critical_tasks(&self) -> Vec<&MaintenanceTask> {
        self.tasks.iter().filter(|t| t.critical).collect()
    }

    /// Check if plan is empty
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    fn update_summary(&mut self) {
        let critical_count = self.tasks.iter().filter(|t| t.critical).count();
        self.summary = format!(
            "{} tasks ({} critical), total cost: {}",
            self.tasks.len(),
            critical_count,
            self.total_cost
        );
    }
}

/// Result of optimization analysis
#[derive(Debug, Clone)]
pub struct OptimizationResult {
    /// Current graph health signal
    pub signal: BrittlenessSignal,
    /// Recommended immediate action
    pub immediate_action: OptimizerAction,
    /// Full maintenance plan
    pub plan: MaintenancePlan,
    /// Metrics snapshot
    pub metrics: HashMap<String, f64>,
}

/// The optimizer that plans maintenance actions
#[derive(Debug)]
pub struct Optimizer {
    /// Learning gate controller
    learning_gate: LearningGate,
    /// Task ID counter
    next_task_id: u64,
    /// Last optimization result
    last_result: Option<OptimizationResult>,
}

impl Optimizer {
    /// Create new optimizer
    pub fn new() -> Self {
        Self {
            learning_gate: LearningGate::default(),
            next_task_id: 1,
            last_result: None,
        }
    }

    /// Get the learning gate
    pub fn learning_gate(&self) -> &LearningGate {
        &self.learning_gate
    }

    /// Get mutable learning gate
    pub fn learning_gate_mut(&mut self) -> &mut LearningGate {
        &mut self.learning_gate
    }

    /// Analyze monitor state and generate optimization plan
    pub fn analyze(&mut self, monitor: &StructuralMonitor) -> OptimizationResult {
        let signal = monitor.signal();
        let state = monitor.state();

        // Adjust learning gate based on signal
        self.learning_gate.adjust(signal);

        // Build maintenance plan
        let mut plan = MaintenancePlan::new();
        let mut immediate_action = OptimizerAction::NoOp;

        // Check triggers and add tasks
        for trigger in monitor.triggers() {
            let (action, priority) = self.action_for_trigger(trigger.trigger_type, state);

            if priority >= 8 && matches!(immediate_action, OptimizerAction::NoOp) {
                immediate_action = action.clone();
            }

            let task = MaintenanceTask::new(self.next_task_id, action, priority);
            self.next_task_id += 1;
            plan.add_task(task);
        }

        // Add proactive maintenance based on signal
        if matches!(signal, BrittlenessSignal::Warning) && plan.is_empty() {
            let task = MaintenanceTask::new(
                self.next_task_id,
                OptimizerAction::Rewire {
                    strengthen: state
                        .boundary_edges
                        .iter()
                        .map(|&(u, v)| (u, v, 1.2))
                        .collect(),
                    weaken: Vec::new(),
                },
                5,
            );
            self.next_task_id += 1;
            plan.add_task(task);
        }

        // Sort by priority
        plan.prioritize();

        // Collect metrics
        let mut metrics = HashMap::new();
        metrics.insert("lambda_est".to_string(), state.lambda_est);
        metrics.insert("lambda_trend".to_string(), state.lambda_trend);
        metrics.insert("cut_volatility".to_string(), state.cut_volatility);
        metrics.insert(
            "boundary_edges".to_string(),
            state.boundary_edges.len() as f64,
        );
        metrics.insert(
            "learning_rate".to_string(),
            self.learning_gate.learning_rate,
        );

        let result = OptimizationResult {
            signal,
            immediate_action,
            plan,
            metrics,
        };

        self.last_result = Some(result.clone());
        result
    }

    /// Get the last optimization result
    pub fn last_result(&self) -> Option<&OptimizationResult> {
        self.last_result.as_ref()
    }

    /// Generate action for a trigger type
    fn action_for_trigger(
        &self,
        trigger_type: TriggerType,
        state: &super::structural_monitor::MonitorState,
    ) -> (OptimizerAction, u8) {
        match trigger_type {
            TriggerType::IslandingRisk => {
                // Strengthen boundary edges to prevent islanding
                let strengthen: Vec<_> = state
                    .boundary_edges
                    .iter()
                    .map(|&(u, v)| (u, v, 1.5))
                    .collect();
                (
                    OptimizerAction::Rewire {
                        strengthen,
                        weaken: Vec::new(),
                    },
                    9,
                )
            }
            TriggerType::Instability => {
                // Pause learning to stabilize
                (
                    OptimizerAction::LearningGate {
                        enable: false,
                        rate_multiplier: 0.5,
                    },
                    7,
                )
            }
            TriggerType::Degradation => {
                // Reindex to refresh connections
                (
                    OptimizerAction::Reindex {
                        nodes: Vec::new(),        // All nodes
                        new_threshold: Some(0.6), // Lower threshold
                    },
                    6,
                )
            }
            TriggerType::OverClustering => {
                // Merge shards
                (
                    OptimizerAction::MergeShards {
                        shard_ids: vec![0, 1], // Placeholder
                    },
                    4,
                )
            }
            TriggerType::Disconnected => {
                // Critical: attempt to reconnect
                (
                    OptimizerAction::Reindex {
                        nodes: Vec::new(),
                        new_threshold: Some(0.5), // Very low threshold
                    },
                    10,
                )
            }
        }
    }
}

impl Default for Optimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_creation() {
        let optimizer = Optimizer::new();
        assert!(optimizer.learning_gate().enabled);
    }

    #[test]
    fn test_learning_gate_adjustment() {
        let mut gate = LearningGate::default();

        // Healthy should increase rate
        let initial_rate = gate.learning_rate;
        gate.adjust(BrittlenessSignal::Healthy);
        assert!(gate.learning_rate > initial_rate);

        // Critical should decrease rate
        gate.adjust(BrittlenessSignal::Critical);
        assert!(gate.learning_rate < initial_rate);
    }

    #[test]
    fn test_maintenance_plan() {
        let mut plan = MaintenancePlan::new();
        assert!(plan.is_empty());

        let task = MaintenanceTask::new(1, OptimizerAction::NoOp, 5);
        plan.add_task(task);
        assert!(!plan.is_empty());
        assert_eq!(plan.tasks.len(), 1);
    }

    #[test]
    fn test_plan_prioritization() {
        let mut plan = MaintenancePlan::new();

        plan.add_task(MaintenanceTask::new(1, OptimizerAction::NoOp, 3));
        plan.add_task(MaintenanceTask::new(2, OptimizerAction::NoOp, 9));
        plan.add_task(MaintenanceTask::new(3, OptimizerAction::NoOp, 5));

        plan.prioritize();

        assert_eq!(plan.tasks[0].priority, 9);
        assert_eq!(plan.tasks[1].priority, 5);
        assert_eq!(plan.tasks[2].priority, 3);
    }

    #[test]
    fn test_optimizer_analyze() {
        let mut optimizer = Optimizer::new();
        let mut monitor = StructuralMonitor::new();

        // Healthy observation
        monitor.observe(5.0, vec![]);
        let result = optimizer.analyze(&monitor);
        assert_eq!(result.signal, BrittlenessSignal::Healthy);

        // Critical observation
        monitor.observe(0.5, vec![(1, 2)]);
        let result = optimizer.analyze(&monitor);
        assert_eq!(result.signal, BrittlenessSignal::Critical);
        assert!(!result.plan.is_empty());
    }

    #[test]
    fn test_action_generation() {
        let optimizer = Optimizer::new();
        let state = super::super::structural_monitor::MonitorState {
            lambda_est: 0.5,
            boundary_edges: vec![(1, 2)],
            ..Default::default()
        };

        let (action, priority) = optimizer.action_for_trigger(TriggerType::IslandingRisk, &state);
        assert!(priority >= 8);
        assert!(matches!(action, OptimizerAction::Rewire { .. }));
    }
}
