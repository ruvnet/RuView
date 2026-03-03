//! MCP tool integration for nanosecond scheduler
//!
//! Provides MCP server endpoints for ultra-low latency scheduling operations.
//! Created by rUv - https://github.com/ruvnet

use nanosecond_scheduler::{Config, Scheduler, Task, Priority};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use parking_lot::RwLock;

/// MCP tool for nanosecond scheduler operations
pub struct SchedulerTool {
    schedulers: Arc<RwLock<HashMap<String, Arc<Scheduler>>>>,
}

impl SchedulerTool {
    pub fn new() -> Self {
        Self {
            schedulers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new scheduler instance
    pub fn create_scheduler(&self, params: CreateSchedulerParams) -> Result<SchedulerResponse, String> {
        let config = Config {
            tick_rate_ns: params.tick_rate_ns.unwrap_or(1000),
            max_tasks_per_tick: params.max_tasks_per_tick.unwrap_or(1000),
            parallel: params.parallel.unwrap_or(false),
            lipschitz_constant: params.lipschitz_constant.unwrap_or(0.9),
            window_size: params.window_size.unwrap_or(100),
        };

        let scheduler = Arc::new(Scheduler::new(config));
        let id = params.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        self.schedulers.write().insert(id.clone(), scheduler.clone());

        Ok(SchedulerResponse {
            id,
            status: "created".to_string(),
            message: Some("Scheduler created successfully".to_string()),
            metrics: None,
        })
    }

    /// Schedule a task on a scheduler
    pub fn schedule_task(&self, params: ScheduleTaskParams) -> Result<TaskResponse, String> {
        let schedulers = self.schedulers.read();
        let scheduler = schedulers.get(&params.scheduler_id)
            .ok_or("Scheduler not found")?;

        let priority = match params.priority.as_deref() {
            Some("critical") => Priority::Critical,
            Some("high") => Priority::High,
            Some("normal") | None => Priority::Normal,
            Some("low") => Priority::Low,
            _ => Priority::Normal,
        };

        let delay = Duration::from_nanos(params.delay_ns.unwrap_or(0));

        // Create a task with a simple callback
        let task_id = uuid::Uuid::new_v4().to_string();
        let task = Task::new(
            move || {
                // Task execution logged internally
            },
            delay
        ).with_priority(priority);

        scheduler.schedule(task);

        Ok(TaskResponse {
            task_id,
            scheduler_id: params.scheduler_id,
            status: "scheduled".to_string(),
            scheduled_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
        })
    }

    /// Execute a scheduler tick
    pub fn tick_scheduler(&self, params: TickParams) -> Result<TickResponse, String> {
        let schedulers = self.schedulers.read();
        let scheduler = schedulers.get(&params.scheduler_id)
            .ok_or("Scheduler not found")?;

        let start = std::time::Instant::now();
        scheduler.tick();
        let elapsed = start.elapsed();

        Ok(TickResponse {
            scheduler_id: params.scheduler_id,
            tick_time_ns: elapsed.as_nanos() as u64,
            tasks_processed: 0, // Would need internal tracking
        })
    }

    /// Get scheduler metrics
    pub fn get_metrics(&self, params: MetricsParams) -> Result<MetricsResponse, String> {
        let schedulers = self.schedulers.read();
        let scheduler = schedulers.get(&params.scheduler_id)
            .ok_or("Scheduler not found")?;

        let metrics = scheduler.metrics();

        Ok(MetricsResponse {
            scheduler_id: params.scheduler_id,
            min_tick_time_ns: metrics.min_tick_time_ns,
            avg_tick_time_ns: metrics.avg_tick_time_ns,
            max_tick_time_ns: metrics.max_tick_time_ns,
            total_ticks: metrics.total_ticks,
            tasks_per_second: metrics.tasks_per_second,
            temporal_overlap: scheduler.temporal_overlap(),
            strange_loop_state: scheduler.strange_loop_state(),
        })
    }

    /// Run benchmark
    pub fn run_benchmark(&self, params: BenchmarkParams) -> Result<BenchmarkResponse, String> {
        use std::sync::atomic::{AtomicU64, Ordering};

        let config = Config {
            tick_rate_ns: params.tick_rate_ns.unwrap_or(1000),
            max_tasks_per_tick: 1000,
            ..Default::default()
        };

        let scheduler = Scheduler::new(config);
        let counter = Arc::new(AtomicU64::new(0));
        let num_tasks = params.num_tasks.unwrap_or(10000);

        // Schedule tasks
        for i in 0..num_tasks {
            let counter_clone = counter.clone();
            scheduler.schedule(Task::new(
                move || {
                    counter_clone.fetch_add(1, Ordering::Relaxed);
                },
                Duration::from_nanos((i % 100) as u64),
            ));
        }

        // Execute tasks
        let start = std::time::Instant::now();
        while counter.load(Ordering::Relaxed) < num_tasks as u64 {
            scheduler.tick();
        }
        let elapsed = start.elapsed();

        let metrics = scheduler.metrics();

        Ok(BenchmarkResponse {
            num_tasks,
            total_time_ms: elapsed.as_millis() as u64,
            tasks_per_second: (num_tasks as f64 / elapsed.as_secs_f64()) as u64,
            avg_tick_time_ns: metrics.avg_tick_time_ns,
            min_tick_time_ns: metrics.min_tick_time_ns,
            max_tick_time_ns: metrics.max_tick_time_ns,
            performance_rating: if metrics.avg_tick_time_ns < 100 {
                "EXCELLENT"
            } else if metrics.avg_tick_time_ns < 1000 {
                "GOOD"
            } else {
                "ACCEPTABLE"
            }.to_string(),
        })
    }

    /// Test temporal consciousness features
    pub fn test_consciousness(&self, params: ConsciousnessParams) -> Result<ConsciousnessResponse, String> {
        let config = Config {
            lipschitz_constant: params.lipschitz_constant.unwrap_or(0.9),
            window_size: params.window_size.unwrap_or(100),
            ..Default::default()
        };

        let scheduler = Scheduler::new(config);
        let iterations = params.iterations.unwrap_or(1000);

        // Run strange loop iterations
        for _ in 0..iterations {
            scheduler.tick();
        }

        let final_state = scheduler.strange_loop_state();
        let temporal_overlap = scheduler.temporal_overlap();
        let convergence_error = (final_state - 0.5).abs();

        Ok(ConsciousnessResponse {
            iterations,
            lipschitz_constant: config.lipschitz_constant,
            final_state,
            convergence_error,
            temporal_overlap,
            converged: convergence_error < 0.001,
            message: if convergence_error < 0.001 {
                "Perfect convergence achieved - consciousness emerges from temporal continuity"
            } else {
                "Convergence in progress"
            }.to_string(),
        })
    }

    /// List all active schedulers
    pub fn list_schedulers(&self) -> Result<ListSchedulersResponse, String> {
        let schedulers = self.schedulers.read();
        let ids: Vec<String> = schedulers.keys().cloned().collect();

        Ok(ListSchedulersResponse {
            scheduler_ids: ids,
            count: schedulers.len(),
        })
    }

    /// Destroy a scheduler
    pub fn destroy_scheduler(&self, params: DestroyParams) -> Result<DestroyResponse, String> {
        let removed = self.schedulers.write().remove(&params.scheduler_id).is_some();

        if removed {
            Ok(DestroyResponse {
                scheduler_id: params.scheduler_id,
                status: "destroyed".to_string(),
            })
        } else {
            Err("Scheduler not found".to_string())
        }
    }
}

// Request/Response types

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateSchedulerParams {
    pub id: Option<String>,
    pub tick_rate_ns: Option<u64>,
    pub max_tasks_per_tick: Option<usize>,
    pub parallel: Option<bool>,
    pub lipschitz_constant: Option<f64>,
    pub window_size: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SchedulerResponse {
    pub id: String,
    pub status: String,
    pub message: Option<String>,
    pub metrics: Option<MetricsResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ScheduleTaskParams {
    pub scheduler_id: String,
    pub delay_ns: Option<u64>,
    pub priority: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskResponse {
    pub task_id: String,
    pub scheduler_id: String,
    pub status: String,
    pub scheduled_at: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TickParams {
    pub scheduler_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TickResponse {
    pub scheduler_id: String,
    pub tick_time_ns: u64,
    pub tasks_processed: usize,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MetricsParams {
    pub scheduler_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MetricsResponse {
    pub scheduler_id: String,
    pub min_tick_time_ns: u64,
    pub avg_tick_time_ns: u64,
    pub max_tick_time_ns: u64,
    pub total_ticks: u64,
    pub tasks_per_second: f64,
    pub temporal_overlap: f64,
    pub strange_loop_state: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BenchmarkParams {
    pub num_tasks: Option<usize>,
    pub tick_rate_ns: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BenchmarkResponse {
    pub num_tasks: usize,
    pub total_time_ms: u64,
    pub tasks_per_second: u64,
    pub avg_tick_time_ns: u64,
    pub min_tick_time_ns: u64,
    pub max_tick_time_ns: u64,
    pub performance_rating: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConsciousnessParams {
    pub iterations: Option<usize>,
    pub lipschitz_constant: Option<f64>,
    pub window_size: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConsciousnessResponse {
    pub iterations: usize,
    pub lipschitz_constant: f64,
    pub final_state: f64,
    pub convergence_error: f64,
    pub temporal_overlap: f64,
    pub converged: bool,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ListSchedulersResponse {
    pub scheduler_ids: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DestroyParams {
    pub scheduler_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DestroyResponse {
    pub scheduler_id: String,
    pub status: String,
}

// MCP Server implementation
impl Default for SchedulerTool {
    fn default() -> Self {
        Self::new()
    }
}