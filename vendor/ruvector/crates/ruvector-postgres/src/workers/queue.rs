//! Task Queue with Priority-Based Scheduling
//!
//! Implements a priority-based task queue with work stealing for load balancing.
//! Supports multiple task types: Maintenance, Training, Integrity, Query.
//!
//! # Queue Architecture
//!
//! ```text
//! +------------------------------------------------------------------+
//! |                     PRIORITY TASK QUEUE                          |
//! +------------------------------------------------------------------+
//! |                                                                  |
//! | Priority 0 (Critical): [Task] [Task] [Task]  <- Processed first  |
//! | Priority 1 (High):     [Task] [Task]                             |
//! | Priority 2 (Medium):   [Task] [Task] [Task] [Task]               |
//! | Priority 3 (Low):      [Task] [Task]         <- Processed last   |
//! |                                                                  |
//! +------------------------------------------------------------------+
//! |                        WORK STEALING                             |
//! |  +--------+    +--------+    +--------+    +--------+            |
//! |  |Worker 1|<-->|Worker 2|<-->|Worker 3|<-->|Worker 4|            |
//! |  | Queue  |    | Queue  |    | Queue  |    | Queue  |            |
//! |  +--------+    +--------+    +--------+    +--------+            |
//! +------------------------------------------------------------------+
//! ```

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering as CmpOrdering;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Task Types and Priority
// ============================================================================

/// Task types supported by the queue
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskType {
    /// Query processing task
    Query,
    /// Insert operation
    Insert,
    /// Delete operation
    Delete,
    /// Index maintenance task
    Maintenance,
    /// GNN training task
    Training,
    /// Integrity monitoring task
    Integrity,
    /// Index build task
    IndexBuild,
    /// Statistics collection
    StatsCollection,
}

impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskType::Query => write!(f, "query"),
            TaskType::Insert => write!(f, "insert"),
            TaskType::Delete => write!(f, "delete"),
            TaskType::Maintenance => write!(f, "maintenance"),
            TaskType::Training => write!(f, "training"),
            TaskType::Integrity => write!(f, "integrity"),
            TaskType::IndexBuild => write!(f, "index_build"),
            TaskType::StatsCollection => write!(f, "stats_collection"),
        }
    }
}

/// Task priority levels
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum TaskPriority {
    /// Critical priority - processed immediately
    Critical = 0,
    /// High priority
    High = 1,
    /// Medium priority (default)
    #[default]
    Medium = 2,
    /// Low priority - background tasks
    Low = 3,
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskPriority::Critical => write!(f, "critical"),
            TaskPriority::High => write!(f, "high"),
            TaskPriority::Medium => write!(f, "medium"),
            TaskPriority::Low => write!(f, "low"),
        }
    }
}

// ============================================================================
// Task Definition
// ============================================================================

/// A task in the queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task ID
    pub id: u64,
    /// Task type
    pub task_type: TaskType,
    /// Priority level
    pub priority: TaskPriority,
    /// Serialized task data
    pub data: Vec<u8>,
    /// Creation timestamp (epoch ms)
    pub created_at: u64,
    /// Deadline (epoch ms, 0 = no deadline)
    pub deadline_ms: u64,
    /// Maximum retries
    pub max_retries: u32,
    /// Current retry count
    pub retry_count: u32,
    /// Collection ID (if applicable)
    pub collection_id: Option<i32>,
    /// Dependencies (task IDs that must complete first)
    pub dependencies: Vec<u64>,
}

impl Task {
    /// Create a new task
    pub fn new(task_type: TaskType, data: Vec<u8>) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);

        Self {
            id: NEXT_ID.fetch_add(1, Ordering::SeqCst),
            task_type,
            priority: TaskPriority::default(),
            data,
            created_at: current_epoch_ms(),
            deadline_ms: 0,
            max_retries: 3,
            retry_count: 0,
            collection_id: None,
            dependencies: Vec::new(),
        }
    }

    /// Set task priority
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set task deadline
    pub fn with_deadline(mut self, deadline_ms: u64) -> Self {
        self.deadline_ms = deadline_ms;
        self
    }

    /// Set max retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set collection ID
    pub fn with_collection(mut self, collection_id: i32) -> Self {
        self.collection_id = Some(collection_id);
        self
    }

    /// Add dependencies
    pub fn with_dependencies(mut self, deps: Vec<u64>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Check if task is expired
    pub fn is_expired(&self) -> bool {
        if self.deadline_ms == 0 {
            return false;
        }
        current_epoch_ms() > self.deadline_ms
    }

    /// Check if task can be retried
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
}

/// Wrapper for priority queue ordering
#[derive(Debug)]
struct PrioritizedTask {
    task: Task,
    /// Lower value = higher priority (processed first)
    effective_priority: i64,
}

impl PrioritizedTask {
    fn new(task: Task) -> Self {
        // Calculate effective priority:
        // - Base priority (0-3)
        // - Subtract time bonus (older tasks get higher priority)
        // - Add deadline urgency (tasks closer to deadline get higher priority)
        let age_bonus = (current_epoch_ms() - task.created_at) / 1000; // seconds

        let deadline_urgency = if task.deadline_ms > 0 {
            let remaining = task.deadline_ms.saturating_sub(current_epoch_ms());
            if remaining < 1000 {
                -100 // Very urgent
            } else if remaining < 5000 {
                -50
            } else if remaining < 30000 {
                -10
            } else {
                0
            }
        } else {
            0
        };

        let effective_priority =
            (task.priority as i64 * 100) - (age_bonus as i64) + deadline_urgency;

        Self {
            task,
            effective_priority,
        }
    }
}

impl PartialEq for PrioritizedTask {
    fn eq(&self, other: &Self) -> bool {
        self.effective_priority == other.effective_priority
    }
}

impl Eq for PrioritizedTask {}

impl PartialOrd for PrioritizedTask {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedTask {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        // Reverse ordering: lower effective_priority = higher in heap
        other.effective_priority.cmp(&self.effective_priority)
    }
}

// ============================================================================
// Task Queue Implementation
// ============================================================================

/// Priority-based task queue with work stealing
pub struct TaskQueue {
    /// Main priority queue
    queue: Mutex<BinaryHeap<PrioritizedTask>>,
    /// Per-worker local queues for work stealing
    worker_queues: RwLock<Vec<Mutex<Vec<Task>>>>,
    /// Completed task IDs (for dependency tracking)
    completed: RwLock<std::collections::HashSet<u64>>,
    /// Failed task IDs
    failed: RwLock<std::collections::HashSet<u64>>,
    /// Statistics
    stats: QueueStats,
    /// Maximum queue size
    max_size: usize,
}

impl TaskQueue {
    /// Create a new task queue
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: Mutex::new(BinaryHeap::with_capacity(max_size)),
            worker_queues: RwLock::new(Vec::new()),
            completed: RwLock::new(std::collections::HashSet::new()),
            failed: RwLock::new(std::collections::HashSet::new()),
            stats: QueueStats::new(),
            max_size,
        }
    }

    /// Add a worker queue for work stealing
    pub fn add_worker(&self) -> usize {
        let mut queues = self.worker_queues.write();
        let worker_id = queues.len();
        queues.push(Mutex::new(Vec::new()));
        worker_id
    }

    /// Remove a worker queue
    pub fn remove_worker(&self, worker_id: usize) {
        let queues = self.worker_queues.read();
        if worker_id < queues.len() {
            // Move remaining tasks back to main queue
            let worker_queue = queues[worker_id].lock();
            let mut main_queue = self.queue.lock();
            for task in worker_queue.iter() {
                main_queue.push(PrioritizedTask::new(task.clone()));
            }
        }
    }

    /// Enqueue a task
    pub fn enqueue(&self, task: Task) -> Result<u64, QueueError> {
        // Check queue size
        if self.len() >= self.max_size {
            self.stats.rejected.fetch_add(1, Ordering::Relaxed);
            return Err(QueueError::QueueFull);
        }

        // Check dependencies
        if !self.dependencies_satisfied(&task) {
            // Queue to pending
            self.stats.pending.fetch_add(1, Ordering::Relaxed);
        }

        let task_id = task.id;
        let mut queue = self.queue.lock();
        queue.push(PrioritizedTask::new(task));

        self.stats.enqueued.fetch_add(1, Ordering::Relaxed);
        self.update_queue_depth();

        Ok(task_id)
    }

    /// Dequeue the highest priority task
    pub fn dequeue(&self) -> Option<Task> {
        let mut queue = self.queue.lock();

        // Skip expired tasks
        while let Some(prioritized) = queue.pop() {
            if prioritized.task.is_expired() {
                self.stats.expired.fetch_add(1, Ordering::Relaxed);
                continue;
            }

            // Check dependencies
            if !self.dependencies_satisfied(&prioritized.task) {
                // Re-queue with lower priority
                queue.push(prioritized);
                continue;
            }

            self.stats.dequeued.fetch_add(1, Ordering::Relaxed);
            self.update_queue_depth();
            return Some(prioritized.task);
        }

        None
    }

    /// Dequeue a task for a specific worker (with work stealing)
    pub fn dequeue_for_worker(&self, worker_id: usize) -> Option<Task> {
        // First try worker's local queue
        {
            let queues = self.worker_queues.read();
            if worker_id < queues.len() {
                if let Some(task) = queues[worker_id].lock().pop() {
                    return Some(task);
                }
            }
        }

        // Try main queue
        if let Some(task) = self.dequeue() {
            return Some(task);
        }

        // Try work stealing from other workers
        self.steal_work(worker_id)
    }

    /// Steal work from another worker
    fn steal_work(&self, worker_id: usize) -> Option<Task> {
        let queues = self.worker_queues.read();

        for (i, worker_queue) in queues.iter().enumerate() {
            if i == worker_id {
                continue;
            }

            let mut queue = worker_queue.lock();
            if !queue.is_empty() {
                // Steal half of the tasks
                let steal_count = queue.len() / 2;
                if steal_count > 0 {
                    let stolen: Vec<_> = queue.drain(..steal_count).collect();
                    if !stolen.is_empty() {
                        self.stats
                            .stolen
                            .fetch_add(stolen.len() as u64, Ordering::Relaxed);
                        return Some(stolen.into_iter().next().unwrap());
                    }
                }
            }
        }

        None
    }

    /// Check if task dependencies are satisfied
    fn dependencies_satisfied(&self, task: &Task) -> bool {
        if task.dependencies.is_empty() {
            return true;
        }

        let completed = self.completed.read();
        task.dependencies
            .iter()
            .all(|dep_id| completed.contains(dep_id))
    }

    /// Mark a task as completed
    pub fn mark_completed(&self, task_id: u64) {
        self.completed.write().insert(task_id);
        self.stats.completed.fetch_add(1, Ordering::Relaxed);
    }

    /// Mark a task as failed
    pub fn mark_failed(&self, task_id: u64) {
        self.failed.write().insert(task_id);
        self.stats.failed.fetch_add(1, Ordering::Relaxed);
    }

    /// Re-queue a failed task for retry
    pub fn requeue_for_retry(&self, mut task: Task) -> Result<(), QueueError> {
        if !task.can_retry() {
            return Err(QueueError::MaxRetriesExceeded);
        }

        task.increment_retry();
        self.stats.retried.fetch_add(1, Ordering::Relaxed);

        // Add to main queue with same priority
        let mut queue = self.queue.lock();
        queue.push(PrioritizedTask::new(task));

        Ok(())
    }

    /// Get queue length
    pub fn len(&self) -> usize {
        self.queue.lock().len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.lock().is_empty()
    }

    /// Update queue depth statistics
    fn update_queue_depth(&self) {
        let depth = self.len() as u64;
        self.stats.current_depth.store(depth, Ordering::Relaxed);
        self.stats.max_depth.fetch_max(depth, Ordering::Relaxed);
    }

    /// Get queue statistics
    pub fn stats(&self) -> QueueStatsSnapshot {
        self.stats.snapshot()
    }

    /// Clear all completed/failed tracking (for memory management)
    pub fn clear_tracking(&self) {
        self.completed.write().clear();
        self.failed.write().clear();
    }

    /// Get tasks by type
    pub fn tasks_by_type(&self, task_type: TaskType) -> Vec<Task> {
        let queue = self.queue.lock();
        queue
            .iter()
            .filter(|pt| pt.task.task_type == task_type)
            .map(|pt| pt.task.clone())
            .collect()
    }

    /// Cancel a task by ID
    pub fn cancel(&self, task_id: u64) -> bool {
        let mut queue = self.queue.lock();
        let initial_len = queue.len();

        // Rebuild heap without the cancelled task
        let remaining: Vec<_> = queue.drain().filter(|pt| pt.task.id != task_id).collect();

        for pt in remaining {
            queue.push(pt);
        }

        if queue.len() < initial_len {
            self.stats.cancelled.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new(10000)
    }
}

// ============================================================================
// Queue Statistics
// ============================================================================

/// Queue statistics counters
pub struct QueueStats {
    /// Total tasks enqueued
    pub enqueued: AtomicU64,
    /// Total tasks dequeued
    pub dequeued: AtomicU64,
    /// Total tasks completed
    pub completed: AtomicU64,
    /// Total tasks failed
    pub failed: AtomicU64,
    /// Total tasks expired
    pub expired: AtomicU64,
    /// Total tasks rejected (queue full)
    pub rejected: AtomicU64,
    /// Total tasks retried
    pub retried: AtomicU64,
    /// Total tasks stolen (work stealing)
    pub stolen: AtomicU64,
    /// Total tasks cancelled
    pub cancelled: AtomicU64,
    /// Tasks pending dependencies
    pub pending: AtomicU64,
    /// Current queue depth
    pub current_depth: AtomicU64,
    /// Maximum queue depth seen
    pub max_depth: AtomicU64,
}

impl QueueStats {
    /// Create new statistics
    pub fn new() -> Self {
        Self {
            enqueued: AtomicU64::new(0),
            dequeued: AtomicU64::new(0),
            completed: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            expired: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            retried: AtomicU64::new(0),
            stolen: AtomicU64::new(0),
            cancelled: AtomicU64::new(0),
            pending: AtomicU64::new(0),
            current_depth: AtomicU64::new(0),
            max_depth: AtomicU64::new(0),
        }
    }

    /// Get a snapshot of current statistics
    pub fn snapshot(&self) -> QueueStatsSnapshot {
        QueueStatsSnapshot {
            enqueued: self.enqueued.load(Ordering::Relaxed),
            dequeued: self.dequeued.load(Ordering::Relaxed),
            completed: self.completed.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            expired: self.expired.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
            retried: self.retried.load(Ordering::Relaxed),
            stolen: self.stolen.load(Ordering::Relaxed),
            cancelled: self.cancelled.load(Ordering::Relaxed),
            pending: self.pending.load(Ordering::Relaxed),
            current_depth: self.current_depth.load(Ordering::Relaxed),
            max_depth: self.max_depth.load(Ordering::Relaxed),
        }
    }
}

impl Default for QueueStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of queue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatsSnapshot {
    pub enqueued: u64,
    pub dequeued: u64,
    pub completed: u64,
    pub failed: u64,
    pub expired: u64,
    pub rejected: u64,
    pub retried: u64,
    pub stolen: u64,
    pub cancelled: u64,
    pub pending: u64,
    pub current_depth: u64,
    pub max_depth: u64,
}

impl QueueStatsSnapshot {
    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "enqueued": self.enqueued,
            "dequeued": self.dequeued,
            "completed": self.completed,
            "failed": self.failed,
            "expired": self.expired,
            "rejected": self.rejected,
            "retried": self.retried,
            "stolen": self.stolen,
            "cancelled": self.cancelled,
            "pending": self.pending,
            "current_depth": self.current_depth,
            "max_depth": self.max_depth,
            "success_rate": if self.completed + self.failed > 0 {
                self.completed as f64 / (self.completed + self.failed) as f64
            } else {
                1.0
            },
        })
    }
}

// ============================================================================
// Queue Errors
// ============================================================================

/// Queue errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    /// Queue is full
    QueueFull,
    /// Maximum retries exceeded
    MaxRetriesExceeded,
    /// Task not found
    TaskNotFound,
    /// Dependencies not met
    DependenciesNotMet,
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueError::QueueFull => write!(f, "Task queue is full"),
            QueueError::MaxRetriesExceeded => write!(f, "Maximum retries exceeded"),
            QueueError::TaskNotFound => write!(f, "Task not found"),
            QueueError::DependenciesNotMet => write!(f, "Task dependencies not met"),
        }
    }
}

impl std::error::Error for QueueError {}

// ============================================================================
// Global Task Queues
// ============================================================================

/// Global task queue registry
static TASK_QUEUES: OnceLock<TaskQueueRegistry> = OnceLock::new();

/// Registry of task queues by type
pub struct TaskQueueRegistry {
    /// Query task queue (high throughput)
    pub queries: TaskQueue,
    /// Maintenance task queue
    pub maintenance: TaskQueue,
    /// Training task queue
    pub training: TaskQueue,
    /// Integrity task queue
    pub integrity: TaskQueue,
}

impl TaskQueueRegistry {
    /// Create new registry
    pub fn new() -> Self {
        Self {
            queries: TaskQueue::new(10000),
            maintenance: TaskQueue::new(1000),
            training: TaskQueue::new(100),
            integrity: TaskQueue::new(500),
        }
    }

    /// Get queue for task type
    pub fn get_queue(&self, task_type: TaskType) -> &TaskQueue {
        match task_type {
            TaskType::Query | TaskType::Insert | TaskType::Delete => &self.queries,
            TaskType::Maintenance | TaskType::IndexBuild | TaskType::StatsCollection => {
                &self.maintenance
            }
            TaskType::Training => &self.training,
            TaskType::Integrity => &self.integrity,
        }
    }

    /// Get all queue statistics
    pub fn all_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "queries": self.queries.stats().to_json(),
            "maintenance": self.maintenance.stats().to_json(),
            "training": self.training.stats().to_json(),
            "integrity": self.integrity.stats().to_json(),
        })
    }
}

impl Default for TaskQueueRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the global task queue registry
pub fn get_task_queues() -> &'static TaskQueueRegistry {
    TASK_QUEUES.get_or_init(TaskQueueRegistry::new)
}

/// Initialize task queues
pub fn init_task_queues() {
    let _ = get_task_queues();
    pgrx::log!("Task queues initialized");
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get current epoch time in milliseconds
fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new(TaskType::Query, vec![1, 2, 3])
            .with_priority(TaskPriority::High)
            .with_collection(1);

        assert_eq!(task.task_type, TaskType::Query);
        assert_eq!(task.priority, TaskPriority::High);
        assert_eq!(task.collection_id, Some(1));
        assert!(!task.is_expired());
    }

    #[test]
    fn test_task_expiry() {
        let mut task = Task::new(TaskType::Query, vec![]);
        task.deadline_ms = current_epoch_ms() - 1000; // 1 second ago
        assert!(task.is_expired());

        task.deadline_ms = current_epoch_ms() + 1000; // 1 second from now
        assert!(!task.is_expired());
    }

    #[test]
    fn test_task_retry() {
        let mut task = Task::new(TaskType::Query, vec![]).with_max_retries(2);

        assert!(task.can_retry());
        task.increment_retry();
        assert!(task.can_retry());
        task.increment_retry();
        assert!(!task.can_retry());
    }

    #[test]
    fn test_queue_basic_operations() {
        let queue = TaskQueue::new(100);

        let task1 = Task::new(TaskType::Query, vec![1]).with_priority(TaskPriority::Low);
        let task2 = Task::new(TaskType::Query, vec![2]).with_priority(TaskPriority::High);

        queue.enqueue(task1).unwrap();
        queue.enqueue(task2).unwrap();

        assert_eq!(queue.len(), 2);

        // Higher priority should come first
        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.priority, TaskPriority::High);
    }

    #[test]
    fn test_queue_full() {
        let queue = TaskQueue::new(2);

        let task = Task::new(TaskType::Query, vec![]);

        assert!(queue.enqueue(task.clone()).is_ok());
        assert!(queue.enqueue(task.clone()).is_ok());
        assert_eq!(queue.enqueue(task.clone()), Err(QueueError::QueueFull));
    }

    #[test]
    fn test_queue_dependencies() {
        let queue = TaskQueue::new(100);

        let task1_id = 1000;
        let task2 = Task {
            id: 2000,
            dependencies: vec![task1_id],
            ..Task::new(TaskType::Query, vec![])
        };

        queue.enqueue(task2.clone()).unwrap();

        // Should not be able to dequeue because dependency not met
        // (Note: in this implementation it will re-queue, so let's test completion)
        queue.mark_completed(task1_id);

        // Now dependencies are satisfied
        let dequeued = queue.dequeue().unwrap();
        assert_eq!(dequeued.id, 2000);
    }

    #[test]
    fn test_queue_cancel() {
        let queue = TaskQueue::new(100);

        let task = Task::new(TaskType::Query, vec![]);
        let task_id = task.id;

        queue.enqueue(task).unwrap();
        assert_eq!(queue.len(), 1);

        assert!(queue.cancel(task_id));
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_work_stealing() {
        let queue = TaskQueue::new(100);

        let worker0 = queue.add_worker();
        let worker1 = queue.add_worker();

        // Add tasks to worker0's local queue
        {
            let queues = queue.worker_queues.read();
            let mut w0_queue = queues[worker0].lock();
            for i in 0..4 {
                w0_queue.push(Task::new(TaskType::Query, vec![i as u8]));
            }
        }

        // Worker1 should be able to steal
        let stolen = queue.dequeue_for_worker(worker1);
        assert!(stolen.is_some());

        let stats = queue.stats();
        assert!(stats.stolen > 0);
    }

    #[test]
    fn test_queue_stats() {
        let queue = TaskQueue::new(100);

        let task = Task::new(TaskType::Query, vec![]);
        let task_id = task.id;

        queue.enqueue(task).unwrap();
        let _ = queue.dequeue();
        queue.mark_completed(task_id);

        let stats = queue.stats();
        assert_eq!(stats.enqueued, 1);
        assert_eq!(stats.dequeued, 1);
        assert_eq!(stats.completed, 1);
    }
}
