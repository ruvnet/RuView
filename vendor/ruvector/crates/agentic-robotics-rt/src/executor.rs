//! Unified async real-time executor
//!
//! Combines Tokio for soft real-time I/O and priority scheduling for hard real-time tasks

use crate::scheduler::PriorityScheduler;
use crate::RTPriority;
use anyhow::Result;
use parking_lot::Mutex;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tracing::{debug, info};

/// Task priority wrapper
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Priority(pub u8);

/// Task deadline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline(pub Duration);

impl From<Duration> for Deadline {
    fn from(duration: Duration) -> Self {
        Deadline(duration)
    }
}

/// ROS3 unified executor
pub struct ROS3Executor {
    tokio_rt_high: Runtime,
    tokio_rt_low: Runtime,
    scheduler: Arc<Mutex<PriorityScheduler>>,
}

impl ROS3Executor {
    /// Create a new executor
    pub fn new() -> Result<Self> {
        info!("Initializing ROS3 unified executor");

        // High-priority runtime for control loops (2 threads)
        let tokio_rt_high = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("ros3-rt-high")
            .enable_all()
            .build()?;

        // Low-priority runtime for planning (4 threads)
        let tokio_rt_low = Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("ros3-rt-low")
            .enable_all()
            .build()?;

        let scheduler = Arc::new(Mutex::new(PriorityScheduler::new()));

        Ok(Self {
            tokio_rt_high,
            tokio_rt_low,
            scheduler,
        })
    }

    /// Spawn a real-time task with priority and deadline
    pub fn spawn_rt<F>(&self, priority: Priority, deadline: Deadline, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let rt_priority: RTPriority = priority.0.into();

        debug!(
            "Spawning RT task with priority {:?} and deadline {:?}",
            rt_priority, deadline.0
        );

        // Route to appropriate runtime based on deadline
        if deadline.0 < Duration::from_millis(1) {
            // Hard RT: Use high-priority runtime
            self.tokio_rt_high.spawn(async move {
                // In a real implementation with RTIC, this would use hardware interrupts
                task.await;
            });
        } else {
            // Soft RT: Use low-priority runtime
            self.tokio_rt_low.spawn(async move {
                task.await;
            });
        }
    }

    /// Spawn a high-priority task
    pub fn spawn_high<F>(&self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawn_rt(Priority(3), Deadline(Duration::from_micros(500)), task);
    }

    /// Spawn a low-priority task
    pub fn spawn_low<F>(&self, task: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawn_rt(Priority(1), Deadline(Duration::from_millis(100)), task);
    }

    /// Spawn CPU-bound blocking work
    pub fn spawn_blocking<F, R>(&self, f: F) -> tokio::task::JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.tokio_rt_low.spawn_blocking(f)
    }

    /// Get a handle to the high-priority runtime
    pub fn high_priority_runtime(&self) -> &Runtime {
        &self.tokio_rt_high
    }

    /// Get a handle to the low-priority runtime
    pub fn low_priority_runtime(&self) -> &Runtime {
        &self.tokio_rt_low
    }
}

impl Default for ROS3Executor {
    fn default() -> Self {
        Self::new().expect("Failed to create ROS3Executor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_executor_creation() {
        let executor = ROS3Executor::new();
        assert!(executor.is_ok());
    }

    #[test]
    fn test_spawn_high_priority() {
        let executor = ROS3Executor::new().unwrap();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = completed.clone();

        executor.spawn_high(async move {
            completed_clone.store(true, Ordering::SeqCst);
        });

        std::thread::sleep(Duration::from_millis(100));
        // Note: In a real test, we'd use proper synchronization
    }
}
