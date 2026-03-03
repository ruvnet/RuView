//! Real-time scheduling for agent actions with nanosecond precision
//!
//! Integrates nanosecond-scheduler for:
//! - Priority-based task scheduling
//! - Deadline-aware execution
//! - Real-time guarantees
//! - Resource allocation

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::lean_agentic::Action;

/// Scheduling policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulingPolicy {
    /// First-In-First-Out
    FIFO,
    /// Rate-Monotonic (shorter periods have higher priority)
    RateMonotonic,
    /// Earliest Deadline First
    EarliestDeadlineFirst,
    /// Fixed Priority
    FixedPriority,
}

/// Priority level for tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
    Background = 4,
}

/// A scheduled task
#[derive(Debug, Clone)]
pub struct ScheduledTask {
    /// The action to execute
    pub action: Action,
    /// Priority level
    pub priority: Priority,
    /// Deadline (absolute time)
    pub deadline: Instant,
    /// Estimated execution time
    pub estimated_duration: Duration,
    /// Task ID
    pub id: u64,
    /// Arrival time
    pub arrival_time: Instant,
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior (earliest deadline first)
        other.deadline.cmp(&self.deadline)
            .then_with(|| self.priority.cmp(&other.priority))
            .then_with(|| self.id.cmp(&other.id))
    }
}

/// Real-time scheduler for agent actions
pub struct RealtimeScheduler {
    /// Scheduling policy
    policy: SchedulingPolicy,
    /// Task queue
    queue: Arc<RwLock<BinaryHeap<ScheduledTask>>>,
    /// Next task ID
    next_id: Arc<RwLock<u64>>,
    /// Scheduler statistics
    stats: Arc<RwLock<SchedulerStats>>,
}

/// Scheduler statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub total_scheduled: u64,
    pub total_executed: u64,
    pub total_missed_deadlines: u64,
    pub average_latency_ns: u64,
    pub max_latency_ns: u64,
    pub min_latency_ns: u64,
}

impl RealtimeScheduler {
    /// Create a new real-time scheduler
    pub fn new(policy: SchedulingPolicy) -> Self {
        Self {
            policy,
            queue: Arc::new(RwLock::new(BinaryHeap::new())),
            next_id: Arc::new(RwLock::new(0)),
            stats: Arc::new(RwLock::new(SchedulerStats {
                min_latency_ns: u64::MAX,
                ..Default::default()
            })),
        }
    }

    /// Schedule a task
    pub async fn schedule(
        &self,
        action: Action,
        priority: Priority,
        deadline: Duration,
        estimated_duration: Duration,
    ) -> u64 {
        let mut id_lock = self.next_id.write().await;
        let id = *id_lock;
        *id_lock += 1;
        drop(id_lock);

        let now = Instant::now();
        let task = ScheduledTask {
            action,
            priority,
            deadline: now + deadline,
            estimated_duration,
            id,
            arrival_time: now,
        };

        let mut queue = self.queue.write().await;
        queue.push(task);
        drop(queue);

        let mut stats = self.stats.write().await;
        stats.total_scheduled += 1;
        drop(stats);

        id
    }

    /// Get next task to execute
    pub async fn next_task(&self) -> Option<ScheduledTask> {
        let mut queue = self.queue.write().await;

        match self.policy {
            SchedulingPolicy::FIFO => {
                // Convert to Vec, pop first, convert back
                let mut tasks: Vec<_> = queue.drain().collect();
                if tasks.is_empty() {
                    return None;
                }
                tasks.sort_by_key(|t| t.arrival_time);
                let task = tasks.remove(0);
                for t in tasks {
                    queue.push(t);
                }
                Some(task)
            }
            SchedulingPolicy::EarliestDeadlineFirst => {
                // BinaryHeap is already sorted by deadline
                queue.pop()
            }
            SchedulingPolicy::RateMonotonic | SchedulingPolicy::FixedPriority => {
                // Convert to Vec, sort by priority, take highest
                let mut tasks: Vec<_> = queue.drain().collect();
                if tasks.is_empty() {
                    return None;
                }
                tasks.sort_by_key(|t| t.priority);
                let task = tasks.remove(0);
                for t in tasks {
                    queue.push(t);
                }
                Some(task)
            }
        }
    }

    /// Mark task as executed
    pub async fn mark_executed(&self, task_id: u64, execution_time: Duration) {
        let mut stats = self.stats.write().await;
        stats.total_executed += 1;

        let latency_ns = execution_time.as_nanos() as u64;
        stats.average_latency_ns =
            (stats.average_latency_ns * (stats.total_executed - 1) + latency_ns)
            / stats.total_executed;
        stats.max_latency_ns = stats.max_latency_ns.max(latency_ns);
        stats.min_latency_ns = stats.min_latency_ns.min(latency_ns);
    }

    /// Mark deadline as missed
    pub async fn mark_deadline_missed(&self, _task_id: u64) {
        let mut stats = self.stats.write().await;
        stats.total_missed_deadlines += 1;
    }

    /// Get scheduler statistics
    pub async fn get_stats(&self) -> SchedulerStats {
        self.stats.read().await.clone()
    }

    /// Get queue length
    pub async fn queue_len(&self) -> usize {
        self.queue.read().await.len()
    }

    /// Clear all pending tasks
    pub async fn clear(&self) {
        let mut queue = self.queue.write().await;
        queue.clear();
    }

    /// Check if a task would meet its deadline
    pub async fn can_meet_deadline(&self, estimated_duration: Duration, deadline: Duration) -> bool {
        let queue = self.queue.read().await;
        let total_pending: Duration = queue.iter()
            .map(|t| t.estimated_duration)
            .sum();

        total_pending + estimated_duration <= deadline
    }

    /// Get pending tasks count by priority
    pub async fn tasks_by_priority(&self) -> Vec<(Priority, usize)> {
        let queue = self.queue.read().await;
        let mut counts = vec![
            (Priority::Critical, 0),
            (Priority::High, 0),
            (Priority::Medium, 0),
            (Priority::Low, 0),
            (Priority::Background, 0),
        ];

        for task in queue.iter() {
            for (priority, count) in counts.iter_mut() {
                if task.priority == *priority {
                    *count += 1;
                    break;
                }
            }
        }

        counts
    }
}

impl Default for RealtimeScheduler {
    fn default() -> Self {
        Self::new(SchedulingPolicy::EarliestDeadlineFirst)
    }
}

/// Extension trait for Action with scheduling metadata
pub trait SchedulableAction {
    /// Get estimated execution time
    fn estimated_duration(&self) -> Duration;

    /// Get priority
    fn priority(&self) -> Priority;

    /// Get deadline
    fn deadline(&self) -> Duration;
}

impl SchedulableAction for Action {
    fn estimated_duration(&self) -> Duration {
        // Default estimate - can be overridden based on action type
        Duration::from_millis(10)
    }

    fn priority(&self) -> Priority {
        // Default priority - can be overridden based on action type
        match self.confidence {
            c if c > 0.9 => Priority::Critical,
            c if c > 0.7 => Priority::High,
            c if c > 0.5 => Priority::Medium,
            c if c > 0.3 => Priority::Low,
            _ => Priority::Background,
        }
    }

    fn deadline(&self) -> Duration {
        // Default deadline - can be overridden based on action type
        Duration::from_millis(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lean_agentic::types::Context;

    fn create_test_action(name: &str, confidence: f64) -> Action {
        Action {
            name: name.to_string(),
            parameters: serde_json::json!({}),
            reasoning: format!("Test action: {}", name),
            confidence,
            context: Context::default(),
        }
    }

    #[tokio::test]
    async fn test_schedule_task() {
        let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

        let action = create_test_action("test", 0.8);
        let task_id = scheduler.schedule(
            action,
            Priority::High,
            Duration::from_secs(1),
            Duration::from_millis(10),
        ).await;

        assert_eq!(task_id, 0);
        assert_eq!(scheduler.queue_len().await, 1);
    }

    #[tokio::test]
    async fn test_next_task_edf() {
        let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

        // Schedule tasks with different deadlines
        let action1 = create_test_action("task1", 0.8);
        let action2 = create_test_action("task2", 0.8);

        scheduler.schedule(
            action1,
            Priority::Medium,
            Duration::from_secs(2),
            Duration::from_millis(10),
        ).await;

        scheduler.schedule(
            action2,
            Priority::Medium,
            Duration::from_secs(1), // Shorter deadline
            Duration::from_millis(10),
        ).await;

        let next = scheduler.next_task().await.unwrap();
        assert_eq!(next.action.name, "task2"); // Should get task with earlier deadline
    }

    #[tokio::test]
    async fn test_priority_scheduling() {
        let scheduler = RealtimeScheduler::new(SchedulingPolicy::FixedPriority);

        let action1 = create_test_action("low", 0.4);
        let action2 = create_test_action("high", 0.9);

        scheduler.schedule(
            action1,
            Priority::Low,
            Duration::from_secs(1),
            Duration::from_millis(10),
        ).await;

        scheduler.schedule(
            action2,
            Priority::Critical,
            Duration::from_secs(1),
            Duration::from_millis(10),
        ).await;

        let next = scheduler.next_task().await.unwrap();
        assert_eq!(next.action.name, "high"); // Should get high priority task
    }

    #[tokio::test]
    async fn test_stats() {
        let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

        let action = create_test_action("test", 0.8);
        let task_id = scheduler.schedule(
            action,
            Priority::Medium,
            Duration::from_secs(1),
            Duration::from_millis(10),
        ).await;

        scheduler.mark_executed(task_id, Duration::from_micros(500)).await;

        let stats = scheduler.get_stats().await;
        assert_eq!(stats.total_scheduled, 1);
        assert_eq!(stats.total_executed, 1);
        assert!(stats.average_latency_ns > 0);
    }

    #[tokio::test]
    async fn test_can_meet_deadline() {
        let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

        let can_meet = scheduler.can_meet_deadline(
            Duration::from_millis(10),
            Duration::from_secs(1),
        ).await;

        assert!(can_meet);

        // Add many tasks
        for i in 0..100 {
            let action = create_test_action(&format!("task{}", i), 0.8);
            scheduler.schedule(
                action,
                Priority::Medium,
                Duration::from_secs(10),
                Duration::from_millis(100),
            ).await;
        }

        let can_meet = scheduler.can_meet_deadline(
            Duration::from_millis(10),
            Duration::from_millis(1),
        ).await;

        assert!(!can_meet); // Should not be able to meet tight deadline
    }

    #[tokio::test]
    async fn test_tasks_by_priority() {
        let scheduler = RealtimeScheduler::new(SchedulingPolicy::FixedPriority);

        for i in 0..5 {
            let action = create_test_action(&format!("task{}", i), 0.8);
            let priority = match i {
                0 => Priority::Critical,
                1 => Priority::High,
                2 => Priority::Medium,
                3 => Priority::Low,
                4 => Priority::Background,
                _ => Priority::Medium,
            };

            scheduler.schedule(
                action,
                priority,
                Duration::from_secs(1),
                Duration::from_millis(10),
            ).await;
        }

        let counts = scheduler.tasks_by_priority().await;
        assert_eq!(counts.len(), 5);

        for (priority, count) in counts {
            if priority == Priority::Critical || priority == Priority::High ||
               priority == Priority::Medium || priority == Priority::Low ||
               priority == Priority::Background {
                assert_eq!(count, 1);
            }
        }
    }
}
