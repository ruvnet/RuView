//! Message Protocol for Web Worker Communication
//!
//! Defines the message types used for communication between the main thread
//! and Web Workers, including task definitions and responses.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a task.
pub type TaskId = u64;

/// Message sent from main thread to worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkerMessage {
    /// Initialize the worker with configuration.
    Initialize {
        /// Worker ID
        worker_id: usize,
        /// Total number of workers
        total_workers: usize,
        /// Whether shared memory is available
        shared_memory: bool,
    },

    /// Matrix multiplication task.
    ComputeMatmul {
        /// Unique task ID
        task_id: TaskId,
        /// Offset into shared buffer for matrix A
        a_offset: usize,
        /// Offset into shared buffer for matrix B
        b_offset: usize,
        /// Offset into shared buffer for output matrix C
        c_offset: usize,
        /// Number of rows in A (and C)
        m: usize,
        /// Number of columns in B (and C)
        n: usize,
        /// Number of columns in A / rows in B
        k: usize,
        /// Starting row for this worker's chunk
        row_start: usize,
        /// Ending row (exclusive) for this worker's chunk
        row_end: usize,
    },

    /// Attention computation task.
    ComputeAttention {
        /// Unique task ID
        task_id: TaskId,
        /// Offset into shared buffer for Q
        q_offset: usize,
        /// Offset into shared buffer for K
        k_offset: usize,
        /// Offset into shared buffer for V
        v_offset: usize,
        /// Offset into shared buffer for output
        output_offset: usize,
        /// Number of heads to process (head_start to head_end)
        head_start: usize,
        /// Ending head (exclusive)
        head_end: usize,
        /// Total number of heads
        num_heads: usize,
        /// Head dimension
        head_dim: usize,
        /// Sequence length
        seq_len: usize,
    },

    /// Layer normalization task.
    ComputeNorm {
        /// Unique task ID
        task_id: TaskId,
        /// Offset into shared buffer for input
        input_offset: usize,
        /// Offset into shared buffer for output
        output_offset: usize,
        /// Offset for gamma (scale) parameters
        gamma_offset: usize,
        /// Offset for beta (shift) parameters
        beta_offset: usize,
        /// Hidden dimension
        hidden_dim: usize,
        /// Starting batch index
        batch_start: usize,
        /// Ending batch index (exclusive)
        batch_end: usize,
        /// Epsilon for numerical stability
        epsilon: f32,
    },

    /// Softmax computation task.
    ComputeSoftmax {
        /// Unique task ID
        task_id: TaskId,
        /// Offset into shared buffer for input/output
        data_offset: usize,
        /// Dimension along which to compute softmax
        dim_size: usize,
        /// Starting index
        start: usize,
        /// Ending index (exclusive)
        end: usize,
    },

    /// Element-wise operation task.
    ComputeElementwise {
        /// Unique task ID
        task_id: TaskId,
        /// Operation type
        operation: ElementwiseOp,
        /// Offset for first input
        a_offset: usize,
        /// Offset for second input (optional for unary ops)
        b_offset: Option<usize>,
        /// Offset for output
        output_offset: usize,
        /// Starting index
        start: usize,
        /// Ending index (exclusive)
        end: usize,
        /// Scalar value (for scalar ops)
        scalar: Option<f32>,
    },

    /// Reduction operation task.
    ComputeReduce {
        /// Unique task ID
        task_id: TaskId,
        /// Operation type
        operation: ReduceOp,
        /// Offset for input
        input_offset: usize,
        /// Offset for partial result
        partial_offset: usize,
        /// Starting index
        start: usize,
        /// Ending index (exclusive)
        end: usize,
    },

    /// Generic task with data copied via message (fallback mode).
    ComputeWithData {
        /// Unique task ID
        task_id: TaskId,
        /// Operation type
        operation: OperationType,
        /// Input data A
        data_a: Vec<f32>,
        /// Input data B (optional)
        data_b: Option<Vec<f32>>,
        /// Operation parameters
        params: OperationParams,
        /// Chunk range
        chunk_start: usize,
        chunk_end: usize,
    },

    /// Ping message for health check.
    Ping {
        /// Timestamp in milliseconds
        timestamp: f64,
    },

    /// Shutdown the worker.
    Shutdown,
}

/// Message sent from worker to main thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkerResponse {
    /// Worker has been initialized.
    Initialized {
        /// Worker ID
        worker_id: usize,
        /// Capabilities
        capabilities: WorkerCapabilities,
    },

    /// Task completed successfully.
    TaskComplete {
        /// Task ID
        task_id: TaskId,
        /// Duration in milliseconds
        duration_ms: f64,
        /// Optional metrics
        metrics: Option<TaskMetrics>,
    },

    /// Task completed with result data (fallback mode).
    TaskCompleteWithData {
        /// Task ID
        task_id: TaskId,
        /// Result data
        data: Vec<f32>,
        /// Duration in milliseconds
        duration_ms: f64,
    },

    /// Task failed.
    Error {
        /// Task ID
        task_id: TaskId,
        /// Error message
        message: String,
        /// Error code
        code: ErrorCode,
    },

    /// Pong response to ping.
    Pong {
        /// Worker ID
        worker_id: usize,
        /// Original timestamp
        timestamp: f64,
        /// Worker's current timestamp
        worker_timestamp: f64,
    },

    /// Worker is shutting down.
    ShuttingDown {
        /// Worker ID
        worker_id: usize,
    },
}

/// Worker capabilities reported during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkerCapabilities {
    /// SIMD support available
    pub simd: bool,
    /// SharedArrayBuffer support
    pub shared_memory: bool,
    /// Atomics support
    pub atomics: bool,
    /// BigInt support
    pub bigint: bool,
}

/// Metrics from task execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskMetrics {
    /// Number of floating point operations
    pub flops: u64,
    /// Bytes read
    pub bytes_read: u64,
    /// Bytes written
    pub bytes_written: u64,
    /// Cache hits (if applicable)
    pub cache_hits: u64,
    /// Cache misses (if applicable)
    pub cache_misses: u64,
}

/// Element-wise operations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ElementwiseOp {
    /// Addition
    Add,
    /// Subtraction
    Sub,
    /// Multiplication
    Mul,
    /// Division
    Div,
    /// Maximum
    Max,
    /// Minimum
    Min,
    /// Power
    Pow,
    /// Exponential
    Exp,
    /// Natural logarithm
    Log,
    /// Square root
    Sqrt,
    /// Absolute value
    Abs,
    /// Negation
    Neg,
    /// ReLU activation
    Relu,
    /// GeLU activation
    Gelu,
    /// SiLU (Swish) activation
    Silu,
    /// Tanh activation
    Tanh,
    /// Sigmoid activation
    Sigmoid,
    /// Add scalar
    AddScalar,
    /// Multiply by scalar
    MulScalar,
}

/// Reduction operations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReduceOp {
    /// Sum reduction
    Sum,
    /// Mean reduction
    Mean,
    /// Max reduction
    Max,
    /// Min reduction
    Min,
    /// Product reduction
    Prod,
    /// Sum of squares
    SumSq,
    /// L2 norm
    Norm2,
}

/// Operation type for generic tasks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OperationType {
    /// Matrix multiplication
    Matmul,
    /// Attention computation
    Attention,
    /// Layer normalization
    LayerNorm,
    /// Softmax
    Softmax,
    /// Element-wise
    Elementwise,
    /// Reduction
    Reduce,
}

/// Parameters for generic operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationParams {
    /// Matrix dimensions [m, n, k] for matmul
    pub dims: Vec<usize>,
    /// Additional parameters
    pub extra: HashMap<String, f64>,
}

impl Default for OperationParams {
    fn default() -> Self {
        OperationParams {
            dims: Vec::new(),
            extra: HashMap::new(),
        }
    }
}

impl OperationParams {
    /// Create parameters for matrix multiplication.
    pub fn matmul(m: usize, n: usize, k: usize) -> Self {
        OperationParams {
            dims: vec![m, n, k],
            extra: HashMap::new(),
        }
    }

    /// Create parameters for attention.
    pub fn attention(num_heads: usize, head_dim: usize, seq_len: usize) -> Self {
        let mut extra = HashMap::new();
        extra.insert("num_heads".to_string(), num_heads as f64);
        extra.insert("head_dim".to_string(), head_dim as f64);
        extra.insert("seq_len".to_string(), seq_len as f64);

        OperationParams {
            dims: vec![num_heads, head_dim, seq_len],
            extra,
        }
    }

    /// Create parameters for layer norm.
    pub fn layer_norm(hidden_dim: usize, epsilon: f32) -> Self {
        let mut extra = HashMap::new();
        extra.insert("epsilon".to_string(), epsilon as f64);

        OperationParams {
            dims: vec![hidden_dim],
            extra,
        }
    }
}

/// Error codes for worker responses.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    /// Invalid message format
    InvalidMessage,
    /// Memory access violation
    MemoryError,
    /// Invalid dimensions
    DimensionMismatch,
    /// Operation not supported
    UnsupportedOperation,
    /// Worker not initialized
    NotInitialized,
    /// Out of memory
    OutOfMemory,
    /// Internal error
    InternalError,
    /// Timeout
    Timeout,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::InvalidMessage => write!(f, "Invalid message format"),
            ErrorCode::MemoryError => write!(f, "Memory access violation"),
            ErrorCode::DimensionMismatch => write!(f, "Dimension mismatch"),
            ErrorCode::UnsupportedOperation => write!(f, "Unsupported operation"),
            ErrorCode::NotInitialized => write!(f, "Worker not initialized"),
            ErrorCode::OutOfMemory => write!(f, "Out of memory"),
            ErrorCode::InternalError => write!(f, "Internal error"),
            ErrorCode::Timeout => write!(f, "Operation timed out"),
        }
    }
}

/// Task status for tracking progress.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is pending
    Pending,
    /// Task is being processed
    Processing,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// Pending task information.
#[derive(Debug, Clone)]
pub struct PendingTask {
    /// Task ID
    pub task_id: TaskId,
    /// Operation type
    pub operation: OperationType,
    /// Status
    pub status: TaskStatus,
    /// Assigned worker ID
    pub worker_id: Option<usize>,
    /// Start time
    pub started_at: Option<f64>,
}

impl PendingTask {
    /// Create a new pending task.
    pub fn new(task_id: TaskId, operation: OperationType) -> Self {
        PendingTask {
            task_id,
            operation,
            status: TaskStatus::Pending,
            worker_id: None,
            started_at: None,
        }
    }
}

/// Task queue for managing pending tasks.
#[derive(Debug, Default)]
pub struct TaskQueue {
    tasks: HashMap<TaskId, PendingTask>,
    next_task_id: TaskId,
}

impl TaskQueue {
    /// Create a new task queue.
    pub fn new() -> Self {
        TaskQueue {
            tasks: HashMap::new(),
            next_task_id: 1,
        }
    }

    /// Generate a new task ID.
    pub fn next_id(&mut self) -> TaskId {
        let id = self.next_task_id;
        self.next_task_id += 1;
        id
    }

    /// Add a task to the queue.
    pub fn add(&mut self, task: PendingTask) {
        self.tasks.insert(task.task_id, task);
    }

    /// Get a task by ID.
    pub fn get(&self, task_id: TaskId) -> Option<&PendingTask> {
        self.tasks.get(&task_id)
    }

    /// Get a mutable reference to a task.
    pub fn get_mut(&mut self, task_id: TaskId) -> Option<&mut PendingTask> {
        self.tasks.get_mut(&task_id)
    }

    /// Remove a task from the queue.
    pub fn remove(&mut self, task_id: TaskId) -> Option<PendingTask> {
        self.tasks.remove(&task_id)
    }

    /// Update task status.
    pub fn update_status(&mut self, task_id: TaskId, status: TaskStatus) {
        if let Some(task) = self.tasks.get_mut(&task_id) {
            task.status = status;
        }
    }

    /// Get all pending tasks.
    pub fn pending_tasks(&self) -> Vec<&PendingTask> {
        self.tasks
            .values()
            .filter(|t| t.status == TaskStatus::Pending)
            .collect()
    }

    /// Get number of pending tasks.
    pub fn pending_count(&self) -> usize {
        self.tasks
            .values()
            .filter(|t| t.status == TaskStatus::Pending)
            .count()
    }

    /// Clear all tasks.
    pub fn clear(&mut self) {
        self.tasks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_queue() {
        let mut queue = TaskQueue::new();

        let id1 = queue.next_id();
        let id2 = queue.next_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        queue.add(PendingTask::new(id1, OperationType::Matmul));
        queue.add(PendingTask::new(id2, OperationType::Attention));

        assert_eq!(queue.pending_count(), 2);

        queue.update_status(id1, TaskStatus::Completed);
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_operation_params() {
        let params = OperationParams::matmul(10, 20, 30);
        assert_eq!(params.dims, vec![10, 20, 30]);

        let params = OperationParams::layer_norm(512, 1e-5);
        assert_eq!(params.dims, vec![512]);
        assert!((params.extra["epsilon"] - 1e-5).abs() < 1e-10);
    }

    #[test]
    fn test_message_serialization() {
        let msg = WorkerMessage::ComputeMatmul {
            task_id: 1,
            a_offset: 0,
            b_offset: 1000,
            c_offset: 2000,
            m: 10,
            n: 20,
            k: 30,
            row_start: 0,
            row_end: 5,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let parsed: WorkerMessage = serde_json::from_str(&json).unwrap();

        match parsed {
            WorkerMessage::ComputeMatmul {
                task_id, m, n, k, ..
            } => {
                assert_eq!(task_id, 1);
                assert_eq!(m, 10);
                assert_eq!(n, 20);
                assert_eq!(k, 30);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_response_serialization() {
        let resp = WorkerResponse::TaskComplete {
            task_id: 42,
            duration_ms: 123.45,
            metrics: Some(TaskMetrics {
                flops: 1000000,
                bytes_read: 4000,
                bytes_written: 2000,
                ..Default::default()
            }),
        };

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: WorkerResponse = serde_json::from_str(&json).unwrap();

        match parsed {
            WorkerResponse::TaskComplete {
                task_id,
                duration_ms,
                metrics,
            } => {
                assert_eq!(task_id, 42);
                assert!((duration_ms - 123.45).abs() < 0.001);
                assert!(metrics.is_some());
                assert_eq!(metrics.unwrap().flops, 1000000);
            }
            _ => panic!("Wrong response type"),
        }
    }
}
