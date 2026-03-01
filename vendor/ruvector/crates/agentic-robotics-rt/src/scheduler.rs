//! Priority-based task scheduler

use crate::RTPriority;
use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::time::{Duration, Instant};

/// Scheduled task
#[derive(Debug)]
pub struct ScheduledTask {
    pub priority: RTPriority,
    pub deadline: Instant,
    pub task_id: u64,
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.deadline == other.deadline
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
        // Higher priority first, then earlier deadline
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => other.deadline.cmp(&self.deadline),
            ordering => ordering,
        }
    }
}

/// Priority scheduler
pub struct PriorityScheduler {
    queue: BinaryHeap<ScheduledTask>,
    next_task_id: u64,
}

impl PriorityScheduler {
    /// Create a new scheduler
    pub fn new() -> Self {
        Self {
            queue: BinaryHeap::new(),
            next_task_id: 0,
        }
    }

    /// Schedule a task
    pub fn schedule(&mut self, priority: RTPriority, deadline: Duration) -> u64 {
        let task_id = self.next_task_id;
        self.next_task_id += 1;

        let task = ScheduledTask {
            priority,
            deadline: Instant::now() + deadline,
            task_id,
        };

        self.queue.push(task);
        task_id
    }

    /// Get the next task to execute
    pub fn next_task(&mut self) -> Option<ScheduledTask> {
        self.queue.pop()
    }

    /// Get the number of pending tasks
    pub fn pending_tasks(&self) -> usize {
        self.queue.len()
    }

    /// Clear all tasks
    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

impl Default for PriorityScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler() {
        let mut scheduler = PriorityScheduler::new();

        // Schedule tasks with different priorities
        scheduler.schedule(RTPriority::Low, Duration::from_millis(100));
        scheduler.schedule(RTPriority::High, Duration::from_millis(100));
        scheduler.schedule(RTPriority::Critical, Duration::from_millis(100));

        assert_eq!(scheduler.pending_tasks(), 3);

        // Should get critical first
        let task1 = scheduler.next_task().unwrap();
        assert_eq!(task1.priority, RTPriority::Critical);

        // Then high
        let task2 = scheduler.next_task().unwrap();
        assert_eq!(task2.priority, RTPriority::High);

        // Then low
        let task3 = scheduler.next_task().unwrap();
        assert_eq!(task3.priority, RTPriority::Low);

        assert_eq!(scheduler.pending_tasks(), 0);
    }
}
