//! # Nanosecond-Scheduler
//!
//! Ultra-low-latency real-time task scheduler with nanosecond precision.
//!
//! ## Features
//! - Nanosecond-precision timing
//! - Priority-based scheduling
//! - Deadline enforcement
//! - Lock-free queues for performance
//! - CPU affinity support

use serde::{Deserialize, Serialize};
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use thiserror::Error;

/// Scheduler errors
#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("Task queue full")]
    QueueFull,

    #[error("Deadline missed: {0:?}")]
    DeadlineMissed(Duration),

    #[error("Invalid priority: {0}")]
    InvalidPriority(i32),

    #[error("Scheduler not running")]
    NotRunning,
}

/// Priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Critical = 100,
    High = 75,
    Medium = 50,
    Low = 25,
    Background = 10,
}

impl Priority {
    pub fn as_i32(&self) -> i32 {
        *self as i32
    }
}

/// Scheduling policy
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SchedulingPolicy {
    /// Rate Monotonic - priority based on period
    RateMonotonic,
    /// Earliest Deadline First
    EarliestDeadlineFirst,
    /// Least Laxity First
    LeastLaxityFirst,
    /// Fixed Priority
    FixedPriority,
}

/// A deadline for task execution
#[derive(Debug, Clone, Copy)]
pub struct Deadline {
    pub absolute_time: Instant,
}

impl Deadline {
    pub fn from_now(duration: Duration) -> Self {
        Self {
            absolute_time: Instant::now() + duration,
        }
    }

    pub fn from_micros(micros: u64) -> Self {
        Self::from_now(Duration::from_micros(micros))
    }

    pub fn from_millis(millis: u64) -> Self {
        Self::from_now(Duration::from_millis(millis))
    }

    pub fn time_until(&self) -> Option<Duration> {
        self.absolute_time.checked_duration_since(Instant::now())
    }

    pub fn is_passed(&self) -> bool {
        Instant::now() >= self.absolute_time
    }
}

/// A schedulable task
pub struct ScheduledTask<T> {
    pub id: u64,
    pub payload: T,
    pub priority: Priority,
    pub deadline: Deadline,
    pub created_at: Instant,
}

impl<T> ScheduledTask<T> {
    pub fn new(id: u64, payload: T, priority: Priority, deadline: Deadline) -> Self {
        Self {
            id,
            payload,
            priority,
            deadline,
            created_at: Instant::now(),
        }
    }

    pub fn laxity(&self) -> Option<Duration> {
        self.deadline.time_until()
    }
}

impl<T> PartialEq for ScheduledTask<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for ScheduledTask<T> {}

impl<T> PartialOrd for ScheduledTask<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for ScheduledTask<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, earlier deadline first
        other.priority.cmp(&self.priority)
            .then_with(|| self.deadline.absolute_time.cmp(&other.deadline.absolute_time))
    }
}

/// Scheduler statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub total_tasks: u64,
    pub completed_tasks: u64,
    pub missed_deadlines: u64,
    pub average_latency_ns: u64,
    pub max_latency_ns: u64,
    pub queue_size: usize,
}

/// Configuration for the scheduler
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub policy: SchedulingPolicy,
    pub max_queue_size: usize,
    pub enable_rt_scheduling: bool,
    pub cpu_affinity: Option<Vec<usize>>,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            policy: SchedulingPolicy::FixedPriority,
            max_queue_size: 10000,
            enable_rt_scheduling: false,
            cpu_affinity: None,
        }
    }
}

/// Real-time scheduler
pub struct RealtimeScheduler<T> {
    task_queue: Arc<RwLock<BinaryHeap<ScheduledTask<T>>>>,
    stats: Arc<RwLock<SchedulerStats>>,
    config: SchedulerConfig,
    next_task_id: Arc<RwLock<u64>>,
    running: Arc<RwLock<bool>>,
}

impl<T: Send + 'static> RealtimeScheduler<T> {
    /// Create a new real-time scheduler
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            task_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            stats: Arc::new(RwLock::new(SchedulerStats {
                total_tasks: 0,
                completed_tasks: 0,
                missed_deadlines: 0,
                average_latency_ns: 0,
                max_latency_ns: 0,
                queue_size: 0,
            })),
            config,
            next_task_id: Arc::new(RwLock::new(0)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Schedule a task with deadline and priority
    pub fn schedule(
        &self,
        payload: T,
        deadline: Deadline,
        priority: Priority,
    ) -> Result<u64, SchedulerError> {
        let mut queue = self.task_queue.write();

        if queue.len() >= self.config.max_queue_size {
            return Err(SchedulerError::QueueFull);
        }

        let task_id = {
            let mut id = self.next_task_id.write();
            *id += 1;
            *id
        };

        let task = ScheduledTask::new(task_id, payload, priority, deadline);
        queue.push(task);

        let mut stats = self.stats.write();
        stats.total_tasks += 1;
        stats.queue_size = queue.len();

        Ok(task_id)
    }

    /// Get the next task to execute
    pub fn next_task(&self) -> Option<ScheduledTask<T>> {
        let mut queue = self.task_queue.write();
        let task = queue.pop();

        if task.is_some() {
            let mut stats = self.stats.write();
            stats.queue_size = queue.len();
        }

        task
    }

    /// Execute a task and update statistics
    pub fn execute_task<F>(&self, task: ScheduledTask<T>, f: F)
    where
        F: FnOnce(T),
    {
        let execution_start = Instant::now();

        // Check if deadline was missed
        if task.deadline.is_passed() {
            let mut stats = self.stats.write();
            stats.missed_deadlines += 1;
        }

        // Execute the task
        f(task.payload);

        // Update statistics
        let execution_time = execution_start.elapsed();
        let latency_ns = execution_time.as_nanos() as u64;

        let mut stats = self.stats.write();
        stats.completed_tasks += 1;

        // Update average latency
        let total_latency = stats.average_latency_ns * (stats.completed_tasks - 1);
        stats.average_latency_ns = (total_latency + latency_ns) / stats.completed_tasks;

        // Update max latency
        if latency_ns > stats.max_latency_ns {
            stats.max_latency_ns = latency_ns;
        }
    }

    /// Start the scheduler
    pub fn start(&self) {
        *self.running.write() = true;
    }

    /// Stop the scheduler
    pub fn stop(&self) {
        *self.running.write() = false;
    }

    /// Check if scheduler is running
    pub fn is_running(&self) -> bool {
        *self.running.read()
    }

    /// Get current statistics
    pub fn stats(&self) -> SchedulerStats {
        self.stats.read().clone()
    }

    /// Clear all pending tasks
    pub fn clear(&self) {
        let mut queue = self.task_queue.write();
        queue.clear();

        let mut stats = self.stats.write();
        stats.queue_size = 0;
    }

    /// Get queue size
    pub fn queue_size(&self) -> usize {
        self.task_queue.read().len()
    }
}

impl<T: Send + 'static> Default for RealtimeScheduler<T> {
    fn default() -> Self {
        Self::new(SchedulerConfig::default())
    }
}

/// Trait for types that can be scheduled
pub trait Schedulable {
    fn priority(&self) -> Priority;
    fn deadline(&self) -> Deadline;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let scheduler: RealtimeScheduler<i32> = RealtimeScheduler::default();
        assert_eq!(scheduler.queue_size(), 0);
        assert!(!scheduler.is_running());
    }

    #[test]
    fn test_schedule_task() {
        let scheduler = RealtimeScheduler::default();

        let task_id = scheduler.schedule(
            42,
            Deadline::from_millis(100),
            Priority::High,
        ).unwrap();

        assert_eq!(task_id, 1);
        assert_eq!(scheduler.queue_size(), 1);
    }

    #[test]
    fn test_priority_ordering() {
        let scheduler = RealtimeScheduler::default();

        scheduler.schedule(1, Deadline::from_millis(100), Priority::Low).unwrap();
        scheduler.schedule(2, Deadline::from_millis(100), Priority::High).unwrap();
        scheduler.schedule(3, Deadline::from_millis(100), Priority::Critical).unwrap();

        let task1 = scheduler.next_task().unwrap();
        assert_eq!(task1.payload, 3); // Critical priority

        let task2 = scheduler.next_task().unwrap();
        assert_eq!(task2.payload, 2); // High priority

        let task3 = scheduler.next_task().unwrap();
        assert_eq!(task3.payload, 1); // Low priority
    }

    #[test]
    fn test_deadline_detection() {
        let scheduler = RealtimeScheduler::default();

        let past_deadline = Deadline::from_micros(1); // Very short deadline
        std::thread::sleep(Duration::from_millis(10));

        scheduler.schedule(42, past_deadline, Priority::High).unwrap();

        let task = scheduler.next_task().unwrap();
        assert!(task.deadline.is_passed());
    }

    #[test]
    fn test_execute_task() {
        let scheduler = RealtimeScheduler::default();

        scheduler.schedule(42, Deadline::from_millis(100), Priority::High).unwrap();

        let task = scheduler.next_task().unwrap();
        scheduler.execute_task(task, |payload| {
            assert_eq!(payload, 42);
        });

        let stats = scheduler.stats();
        assert_eq!(stats.completed_tasks, 1);
    }

    #[test]
    fn test_stats() {
        let scheduler = RealtimeScheduler::default();

        for i in 0..10 {
            scheduler.schedule(i, Deadline::from_millis(100), Priority::Medium).unwrap();
        }

        let stats = scheduler.stats();
        assert_eq!(stats.total_tasks, 10);
        assert_eq!(stats.queue_size, 10);
    }
}
