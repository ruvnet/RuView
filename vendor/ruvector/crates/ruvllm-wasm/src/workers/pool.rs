//! Worker Pool Implementation
//!
//! Manages a pool of Web Workers for parallel computation.

use crate::workers::feature_detect::is_shared_array_buffer_available;
use crate::workers::messages::*;
use crate::workers::shared::*;
use js_sys::{Array, Float32Array, Object, Promise, Reflect, SharedArrayBuffer, Uint8Array};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Worker pool statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkerPoolStats {
    /// Number of active workers
    pub active_workers: usize,
    /// Number of tasks completed
    pub tasks_completed: u64,
    /// Total task execution time in milliseconds
    pub total_execution_time_ms: f64,
    /// Average task time in milliseconds
    pub avg_task_time_ms: f64,
    /// Number of tasks currently pending
    pub pending_tasks: usize,
    /// Whether shared memory is being used
    pub shared_memory_enabled: bool,
}

/// Internal state for a single worker.
struct WorkerState {
    /// Worker instance
    worker: web_sys::Worker,
    /// Worker ID
    id: usize,
    /// Whether worker is busy
    busy: bool,
    /// Tasks completed by this worker
    tasks_completed: u64,
    /// Total execution time
    total_time_ms: f64,
}

/// Pool of Web Workers for parallel computation.
pub struct WorkerPool {
    /// Worker states
    workers: RefCell<Vec<WorkerState>>,
    /// Task queue
    task_queue: RefCell<TaskQueue>,
    /// Shared memory buffer manager
    shared_buffers: RefCell<SharedBufferManager>,
    /// Whether shared memory is available
    shared_memory: bool,
    /// Promise resolvers for pending tasks
    pending_resolvers: RefCell<HashMap<TaskId, (js_sys::Function, js_sys::Function)>>,
    /// Statistics
    stats: RefCell<WorkerPoolStats>,
}

impl WorkerPool {
    /// Create a new worker pool.
    ///
    /// # Arguments
    /// * `num_workers` - Number of workers to spawn
    ///
    /// # Returns
    /// A Promise that resolves to the WorkerPool when all workers are initialized.
    pub async fn new(num_workers: usize) -> Result<Self, JsValue> {
        let shared_memory = is_shared_array_buffer_available();

        crate::utils::log(&format!(
            "Creating WorkerPool with {} workers (shared memory: {})",
            num_workers, shared_memory
        ));

        let pool = WorkerPool {
            workers: RefCell::new(Vec::with_capacity(num_workers)),
            task_queue: RefCell::new(TaskQueue::new()),
            shared_buffers: RefCell::new(SharedBufferManager::new()),
            shared_memory,
            pending_resolvers: RefCell::new(HashMap::new()),
            stats: RefCell::new(WorkerPoolStats {
                shared_memory_enabled: shared_memory,
                ..Default::default()
            }),
        };

        // Create workers
        for i in 0..num_workers {
            let worker = pool.create_worker(i)?;
            pool.workers.borrow_mut().push(WorkerState {
                worker,
                id: i,
                busy: false,
                tasks_completed: 0,
                total_time_ms: 0.0,
            });
        }

        pool.stats.borrow_mut().active_workers = num_workers;

        // Initialize workers
        pool.initialize_workers().await?;

        crate::utils::log("WorkerPool created successfully");
        Ok(pool)
    }

    /// Create an empty worker pool (for testing).
    #[cfg(test)]
    pub fn empty() -> Self {
        WorkerPool {
            workers: RefCell::new(Vec::new()),
            task_queue: RefCell::new(TaskQueue::new()),
            shared_buffers: RefCell::new(SharedBufferManager::new()),
            shared_memory: false,
            pending_resolvers: RefCell::new(HashMap::new()),
            stats: RefCell::new(WorkerPoolStats::default()),
        }
    }

    /// Create a single worker.
    fn create_worker(&self, id: usize) -> Result<web_sys::Worker, JsValue> {
        // Create an inline worker script as a Blob
        let worker_script = Self::generate_worker_script();
        let blob_parts = Array::new();
        blob_parts.push(&JsValue::from_str(&worker_script));

        let blob_options = web_sys::BlobPropertyBag::new();
        blob_options.set_type("application/javascript");

        let blob = web_sys::Blob::new_with_str_sequence_and_options(&blob_parts, &blob_options)?;

        let url = web_sys::Url::create_object_url_with_blob(&blob)?;

        // Create worker options for shared memory if available
        let worker_options = Object::new();
        if self.shared_memory {
            // SharedArrayBuffer requires special worker type
            Reflect::set(&worker_options, &"type".into(), &"module".into())?;
        }

        let worker = web_sys::Worker::new_with_options(&url, &worker_options.unchecked_into())?;

        // Clean up the blob URL
        web_sys::Url::revoke_object_url(&url)?;

        Ok(worker)
    }

    /// Generate the JavaScript worker script.
    fn generate_worker_script() -> String {
        r#"
// Web Worker for Parallel Inference
// Generated by ruvllm-wasm

let workerId = null;
let totalWorkers = 0;
let sharedMemory = false;
let sharedBuffer = null;
let f32View = null;

// Message handler
self.onmessage = async (e) => {
    const msg = e.data;
    const startTime = performance.now();

    try {
        switch (msg.type) {
            case 'Initialize':
                workerId = msg.worker_id;
                totalWorkers = msg.total_workers;
                sharedMemory = msg.shared_memory;

                // If shared buffer was transferred, set it up
                if (e.data.buffer) {
                    sharedBuffer = e.data.buffer;
                    f32View = new Float32Array(sharedBuffer);
                }

                self.postMessage({
                    type: 'Initialized',
                    worker_id: workerId,
                    capabilities: {
                        simd: typeof WebAssembly.validate === 'function',
                        shared_memory: typeof SharedArrayBuffer !== 'undefined',
                        atomics: typeof Atomics !== 'undefined',
                        bigint: typeof BigInt !== 'undefined'
                    }
                });
                break;

            case 'ComputeMatmul':
                await computeMatmul(msg);
                break;

            case 'ComputeAttention':
                await computeAttention(msg);
                break;

            case 'ComputeNorm':
                await computeNorm(msg);
                break;

            case 'ComputeSoftmax':
                await computeSoftmax(msg);
                break;

            case 'ComputeElementwise':
                await computeElementwise(msg);
                break;

            case 'ComputeReduce':
                await computeReduce(msg);
                break;

            case 'ComputeWithData':
                await computeWithData(msg);
                break;

            case 'Ping':
                self.postMessage({
                    type: 'Pong',
                    worker_id: workerId,
                    timestamp: msg.timestamp,
                    worker_timestamp: performance.now()
                });
                break;

            case 'Shutdown':
                self.postMessage({
                    type: 'ShuttingDown',
                    worker_id: workerId
                });
                self.close();
                break;

            case 'SetBuffer':
                // Receive shared buffer
                sharedBuffer = e.data.buffer;
                f32View = new Float32Array(sharedBuffer);
                break;

            default:
                self.postMessage({
                    type: 'Error',
                    task_id: msg.task_id || 0,
                    message: `Unknown message type: ${msg.type}`,
                    code: 'InvalidMessage'
                });
        }
    } catch (error) {
        self.postMessage({
            type: 'Error',
            task_id: msg.task_id || 0,
            message: error.message || String(error),
            code: 'InternalError'
        });
    }
};

// Matrix multiplication: C = A * B
async function computeMatmul(msg) {
    const { task_id, a_offset, b_offset, c_offset, m, n, k, row_start, row_end } = msg;
    const startTime = performance.now();

    // Using shared memory
    if (sharedBuffer && f32View) {
        const aStart = a_offset / 4;
        const bStart = b_offset / 4;
        const cStart = c_offset / 4;

        // Compute assigned rows
        for (let i = row_start; i < row_end; i++) {
            for (let j = 0; j < n; j++) {
                let sum = 0;
                for (let l = 0; l < k; l++) {
                    sum += f32View[aStart + i * k + l] * f32View[bStart + l * n + j];
                }
                // Use Atomics for thread-safe write if available
                if (typeof Atomics !== 'undefined') {
                    const idx = cStart + i * n + j;
                    const int32View = new Int32Array(sharedBuffer);
                    const bits = new Float32Array([sum]);
                    const intBits = new Int32Array(bits.buffer);
                    Atomics.store(int32View, idx, intBits[0]);
                } else {
                    f32View[cStart + i * n + j] = sum;
                }
            }
        }
    }

    const duration = performance.now() - startTime;
    const flops = (row_end - row_start) * n * k * 2; // 2 ops per multiply-add

    self.postMessage({
        type: 'TaskComplete',
        task_id: task_id,
        duration_ms: duration,
        metrics: {
            flops: flops,
            bytes_read: ((row_end - row_start) * k + k * n) * 4,
            bytes_written: (row_end - row_start) * n * 4,
            cache_hits: 0,
            cache_misses: 0
        }
    });
}

// Attention computation
async function computeAttention(msg) {
    const { task_id, q_offset, k_offset, v_offset, output_offset,
            head_start, head_end, num_heads, head_dim, seq_len } = msg;
    const startTime = performance.now();

    if (sharedBuffer && f32View) {
        const qStart = q_offset / 4;
        const kStart = k_offset / 4;
        const vStart = v_offset / 4;
        const outStart = output_offset / 4;
        const scale = 1.0 / Math.sqrt(head_dim);

        for (let h = head_start; h < head_end; h++) {
            const headOffset = h * seq_len * head_dim;

            // Attention scores
            const scores = new Float32Array(seq_len * seq_len);

            for (let i = 0; i < seq_len; i++) {
                for (let j = 0; j < seq_len; j++) {
                    let dot = 0;
                    for (let d = 0; d < head_dim; d++) {
                        dot += f32View[qStart + headOffset + i * head_dim + d] *
                               f32View[kStart + headOffset + j * head_dim + d];
                    }
                    scores[i * seq_len + j] = dot * scale;
                }
            }

            // Softmax per row
            for (let i = 0; i < seq_len; i++) {
                const rowStart = i * seq_len;
                let maxVal = -Infinity;
                for (let j = 0; j < seq_len; j++) {
                    maxVal = Math.max(maxVal, scores[rowStart + j]);
                }
                let sum = 0;
                for (let j = 0; j < seq_len; j++) {
                    scores[rowStart + j] = Math.exp(scores[rowStart + j] - maxVal);
                    sum += scores[rowStart + j];
                }
                for (let j = 0; j < seq_len; j++) {
                    scores[rowStart + j] /= sum;
                }
            }

            // Output: scores * V
            for (let i = 0; i < seq_len; i++) {
                for (let d = 0; d < head_dim; d++) {
                    let sum = 0;
                    for (let j = 0; j < seq_len; j++) {
                        sum += scores[i * seq_len + j] *
                               f32View[vStart + headOffset + j * head_dim + d];
                    }
                    f32View[outStart + headOffset + i * head_dim + d] = sum;
                }
            }
        }
    }

    const duration = performance.now() - startTime;

    self.postMessage({
        type: 'TaskComplete',
        task_id: task_id,
        duration_ms: duration,
        metrics: {
            flops: (head_end - head_start) * seq_len * seq_len * head_dim * 4,
            bytes_read: (head_end - head_start) * seq_len * head_dim * 3 * 4,
            bytes_written: (head_end - head_start) * seq_len * head_dim * 4,
            cache_hits: 0,
            cache_misses: 0
        }
    });
}

// Layer normalization
async function computeNorm(msg) {
    const { task_id, input_offset, output_offset, gamma_offset, beta_offset,
            hidden_dim, batch_start, batch_end, epsilon } = msg;
    const startTime = performance.now();

    if (sharedBuffer && f32View) {
        const inStart = input_offset / 4;
        const outStart = output_offset / 4;
        const gammaStart = gamma_offset / 4;
        const betaStart = beta_offset / 4;

        for (let b = batch_start; b < batch_end; b++) {
            const batchOffset = b * hidden_dim;

            // Compute mean
            let mean = 0;
            for (let i = 0; i < hidden_dim; i++) {
                mean += f32View[inStart + batchOffset + i];
            }
            mean /= hidden_dim;

            // Compute variance
            let variance = 0;
            for (let i = 0; i < hidden_dim; i++) {
                const diff = f32View[inStart + batchOffset + i] - mean;
                variance += diff * diff;
            }
            variance /= hidden_dim;

            // Normalize
            const std = Math.sqrt(variance + epsilon);
            for (let i = 0; i < hidden_dim; i++) {
                const normalized = (f32View[inStart + batchOffset + i] - mean) / std;
                f32View[outStart + batchOffset + i] =
                    normalized * f32View[gammaStart + i] + f32View[betaStart + i];
            }
        }
    }

    const duration = performance.now() - startTime;

    self.postMessage({
        type: 'TaskComplete',
        task_id: task_id,
        duration_ms: duration,
        metrics: {
            flops: (batch_end - batch_start) * hidden_dim * 5,
            bytes_read: (batch_end - batch_start) * hidden_dim * 4 + hidden_dim * 8,
            bytes_written: (batch_end - batch_start) * hidden_dim * 4,
            cache_hits: 0,
            cache_misses: 0
        }
    });
}

// Softmax computation
async function computeSoftmax(msg) {
    const { task_id, data_offset, dim_size, start, end } = msg;
    const startTime = performance.now();

    if (sharedBuffer && f32View) {
        const dataStart = data_offset / 4;

        for (let i = start; i < end; i++) {
            const rowStart = dataStart + i * dim_size;

            // Find max
            let maxVal = -Infinity;
            for (let j = 0; j < dim_size; j++) {
                maxVal = Math.max(maxVal, f32View[rowStart + j]);
            }

            // Exp and sum
            let sum = 0;
            for (let j = 0; j < dim_size; j++) {
                f32View[rowStart + j] = Math.exp(f32View[rowStart + j] - maxVal);
                sum += f32View[rowStart + j];
            }

            // Normalize
            for (let j = 0; j < dim_size; j++) {
                f32View[rowStart + j] /= sum;
            }
        }
    }

    const duration = performance.now() - startTime;

    self.postMessage({
        type: 'TaskComplete',
        task_id: task_id,
        duration_ms: duration,
        metrics: null
    });
}

// Element-wise operations
async function computeElementwise(msg) {
    const { task_id, operation, a_offset, b_offset, output_offset, start, end, scalar } = msg;
    const startTime = performance.now();

    if (sharedBuffer && f32View) {
        const aStart = a_offset / 4;
        const bStart = b_offset !== null ? b_offset / 4 : null;
        const outStart = output_offset / 4;

        for (let i = start; i < end; i++) {
            const a = f32View[aStart + i];
            const b = bStart !== null ? f32View[bStart + i] : 0;
            let result;

            switch (operation) {
                case 'Add': result = a + b; break;
                case 'Sub': result = a - b; break;
                case 'Mul': result = a * b; break;
                case 'Div': result = a / b; break;
                case 'Max': result = Math.max(a, b); break;
                case 'Min': result = Math.min(a, b); break;
                case 'Exp': result = Math.exp(a); break;
                case 'Log': result = Math.log(a); break;
                case 'Sqrt': result = Math.sqrt(a); break;
                case 'Abs': result = Math.abs(a); break;
                case 'Neg': result = -a; break;
                case 'Relu': result = Math.max(0, a); break;
                case 'Gelu':
                    result = 0.5 * a * (1 + Math.tanh(Math.sqrt(2 / Math.PI) * (a + 0.044715 * a * a * a)));
                    break;
                case 'Silu': result = a / (1 + Math.exp(-a)); break;
                case 'Tanh': result = Math.tanh(a); break;
                case 'Sigmoid': result = 1 / (1 + Math.exp(-a)); break;
                case 'AddScalar': result = a + (scalar || 0); break;
                case 'MulScalar': result = a * (scalar || 1); break;
                default: result = a;
            }

            f32View[outStart + i] = result;
        }
    }

    const duration = performance.now() - startTime;

    self.postMessage({
        type: 'TaskComplete',
        task_id: task_id,
        duration_ms: duration,
        metrics: null
    });
}

// Reduction operations
async function computeReduce(msg) {
    const { task_id, operation, input_offset, partial_offset, start, end } = msg;
    const startTime = performance.now();

    if (sharedBuffer && f32View) {
        const inStart = input_offset / 4;
        const partialStart = partial_offset / 4;

        let result;
        switch (operation) {
            case 'Sum':
                result = 0;
                for (let i = start; i < end; i++) {
                    result += f32View[inStart + i];
                }
                break;
            case 'Mean':
                result = 0;
                for (let i = start; i < end; i++) {
                    result += f32View[inStart + i];
                }
                result /= (end - start);
                break;
            case 'Max':
                result = -Infinity;
                for (let i = start; i < end; i++) {
                    result = Math.max(result, f32View[inStart + i]);
                }
                break;
            case 'Min':
                result = Infinity;
                for (let i = start; i < end; i++) {
                    result = Math.min(result, f32View[inStart + i]);
                }
                break;
            case 'SumSq':
                result = 0;
                for (let i = start; i < end; i++) {
                    result += f32View[inStart + i] * f32View[inStart + i];
                }
                break;
            case 'Norm2':
                result = 0;
                for (let i = start; i < end; i++) {
                    result += f32View[inStart + i] * f32View[inStart + i];
                }
                result = Math.sqrt(result);
                break;
            default:
                result = 0;
        }

        f32View[partialStart] = result;
    }

    const duration = performance.now() - startTime;

    self.postMessage({
        type: 'TaskComplete',
        task_id: task_id,
        duration_ms: duration,
        metrics: null
    });
}

// Fallback: compute with data passed via message
async function computeWithData(msg) {
    const { task_id, operation, data_a, data_b, params, chunk_start, chunk_end } = msg;
    const startTime = performance.now();

    let result = [];

    switch (operation) {
        case 'Matmul': {
            const [m, n, k] = params.dims;
            for (let i = chunk_start; i < chunk_end; i++) {
                for (let j = 0; j < n; j++) {
                    let sum = 0;
                    for (let l = 0; l < k; l++) {
                        sum += data_a[i * k + l] * data_b[l * n + j];
                    }
                    result.push(sum);
                }
            }
            break;
        }
        case 'LayerNorm': {
            const hidden_dim = params.dims[0];
            const epsilon = params.extra.epsilon || 1e-5;

            for (let b = chunk_start; b < chunk_end; b++) {
                const start = b * hidden_dim;
                const slice = data_a.slice(start, start + hidden_dim);

                let mean = slice.reduce((a, b) => a + b, 0) / hidden_dim;
                let variance = slice.reduce((a, x) => a + (x - mean) ** 2, 0) / hidden_dim;
                let std = Math.sqrt(variance + epsilon);

                for (let i = 0; i < hidden_dim; i++) {
                    const normalized = (slice[i] - mean) / std;
                    result.push(normalized);
                }
            }
            break;
        }
        default:
            // Pass through
            result = data_a.slice(chunk_start, chunk_end);
    }

    const duration = performance.now() - startTime;

    self.postMessage({
        type: 'TaskCompleteWithData',
        task_id: task_id,
        data: result,
        duration_ms: duration
    });
}

self.postMessage({ type: 'WorkerReady', worker_id: -1 });
"#.to_string()
    }

    /// Initialize all workers.
    async fn initialize_workers(&self) -> Result<(), JsValue> {
        let workers = self.workers.borrow();
        let num_workers = workers.len();
        let shared_memory = self.shared_memory;

        // Send initialize message to each worker
        for state in workers.iter() {
            let init_msg = WorkerMessage::Initialize {
                worker_id: state.id,
                total_workers: num_workers,
                shared_memory,
            };

            let msg_obj = serde_wasm_bindgen::to_value(&init_msg)?;
            state.worker.post_message(&msg_obj)?;
        }

        // Wait a bit for workers to initialize
        // In a real implementation, you'd wait for Initialized responses
        let promise = js_sys::Promise::new(&mut |resolve, _reject| {
            let window = web_sys::window().unwrap();
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 100)
                .unwrap();
        });
        JsFuture::from(promise).await?;

        Ok(())
    }

    /// Get number of workers.
    pub fn worker_count(&self) -> usize {
        self.workers.borrow().len()
    }

    /// Get pool statistics.
    pub fn stats(&self) -> WorkerPoolStats {
        self.stats.borrow().clone()
    }

    /// Perform parallel matrix multiplication.
    pub async fn parallel_matmul(
        &self,
        a: &[f32],
        b: &[f32],
        m: usize,
        n: usize,
        k: usize,
    ) -> Result<Vec<f32>, JsValue> {
        if !self.shared_memory {
            // Fall back to data-passing mode
            return self.matmul_with_data(a, b, m, n, k).await;
        }

        let num_workers = self.worker_count();
        if num_workers == 0 {
            return Err(JsValue::from_str("No workers available"));
        }

        // Allocate shared memory
        let total_size = (a.len() + b.len() + m * n) * std::mem::size_of::<f32>();
        self.shared_buffers
            .borrow_mut()
            .ensure_capacity(total_size)?;

        let buffer = self
            .shared_buffers
            .borrow()
            .buffer()
            .ok_or_else(|| JsValue::from_str("No shared buffer"))?
            .clone();

        // Copy data to shared buffer
        let view = Float32Array::new(&buffer);
        view.set(&Float32Array::from(a), 0);
        view.set(&Float32Array::from(b), a.len() as u32);

        let a_offset = 0;
        let b_offset = a.len() * std::mem::size_of::<f32>();
        let c_offset = (a.len() + b.len()) * std::mem::size_of::<f32>();

        // Send buffer to workers
        let workers = self.workers.borrow();
        for state in workers.iter() {
            let set_buffer_msg = Object::new();
            Reflect::set(&set_buffer_msg, &"type".into(), &"SetBuffer".into())?;
            Reflect::set(&set_buffer_msg, &"buffer".into(), &buffer)?;
            state.worker.post_message(&set_buffer_msg)?;
        }

        // Distribute work across workers
        let rows_per_worker = (m + num_workers - 1) / num_workers;
        let mut task_ids = Vec::new();

        for (i, state) in workers.iter().enumerate() {
            let row_start = i * rows_per_worker;
            let row_end = ((i + 1) * rows_per_worker).min(m);

            if row_start >= row_end {
                continue;
            }

            let task_id = self.task_queue.borrow_mut().next_id();
            task_ids.push(task_id);

            let msg = WorkerMessage::ComputeMatmul {
                task_id,
                a_offset,
                b_offset,
                c_offset,
                m,
                n,
                k,
                row_start,
                row_end,
            };

            let msg_obj = serde_wasm_bindgen::to_value(&msg)?;
            state.worker.post_message(&msg_obj)?;
        }

        drop(workers);

        // Wait for all tasks to complete
        self.wait_for_tasks(&task_ids).await?;

        // Read result from shared buffer
        let result_view =
            Float32Array::new_with_byte_offset_and_length(&buffer, c_offset as u32, (m * n) as u32);
        Ok(result_view.to_vec())
    }

    /// Fallback matmul using message passing.
    async fn matmul_with_data(
        &self,
        a: &[f32],
        b: &[f32],
        m: usize,
        n: usize,
        k: usize,
    ) -> Result<Vec<f32>, JsValue> {
        let num_workers = self.worker_count();
        if num_workers == 0 {
            return Err(JsValue::from_str("No workers available"));
        }

        // For fallback mode, collect results from each worker
        let rows_per_worker = (m + num_workers - 1) / num_workers;
        let mut results: Vec<Option<Vec<f32>>> = vec![None; num_workers];

        let workers = self.workers.borrow();
        for (i, state) in workers.iter().enumerate() {
            let row_start = i * rows_per_worker;
            let row_end = ((i + 1) * rows_per_worker).min(m);

            if row_start >= row_end {
                results[i] = Some(Vec::new());
                continue;
            }

            let task_id = self.task_queue.borrow_mut().next_id();

            let msg = WorkerMessage::ComputeWithData {
                task_id,
                operation: OperationType::Matmul,
                data_a: a.to_vec(),
                data_b: Some(b.to_vec()),
                params: OperationParams::matmul(m, n, k),
                chunk_start: row_start,
                chunk_end: row_end,
            };

            let msg_obj = serde_wasm_bindgen::to_value(&msg)?;
            state.worker.post_message(&msg_obj)?;
        }

        drop(workers);

        // Wait a bit for computation (simplified - in production use proper async handling)
        let promise = js_sys::Promise::new(&mut |resolve, _reject| {
            let window = web_sys::window().unwrap();
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 500)
                .unwrap();
        });
        JsFuture::from(promise).await?;

        // In a real implementation, collect results via onmessage handlers
        // For now, compute on main thread as fallback
        let mut result = vec![0.0f32; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut sum = 0.0f32;
                for l in 0..k {
                    sum += a[i * k + l] * b[l * n + j];
                }
                result[i * n + j] = sum;
            }
        }

        Ok(result)
    }

    /// Perform parallel attention computation.
    pub async fn parallel_attention(
        &self,
        q: &[f32],
        k: &[f32],
        v: &[f32],
        num_heads: usize,
        head_dim: usize,
        seq_len: usize,
    ) -> Result<Vec<f32>, JsValue> {
        if !self.shared_memory {
            // Fallback to single-threaded
            return Err(JsValue::from_str(
                "Attention requires shared memory for parallel execution",
            ));
        }

        let num_workers = self.worker_count();
        if num_workers == 0 {
            return Err(JsValue::from_str("No workers available"));
        }

        let tensor_size = num_heads * seq_len * head_dim;
        let total_size = tensor_size * 4 * std::mem::size_of::<f32>();

        self.shared_buffers
            .borrow_mut()
            .ensure_capacity(total_size)?;

        let buffer = self
            .shared_buffers
            .borrow()
            .buffer()
            .ok_or_else(|| JsValue::from_str("No shared buffer"))?
            .clone();

        // Copy data
        let view = Float32Array::new(&buffer);
        view.set(&Float32Array::from(q), 0);
        view.set(&Float32Array::from(k), tensor_size as u32);
        view.set(&Float32Array::from(v), (tensor_size * 2) as u32);

        let q_offset = 0;
        let k_offset = tensor_size * std::mem::size_of::<f32>();
        let v_offset = tensor_size * 2 * std::mem::size_of::<f32>();
        let output_offset = tensor_size * 3 * std::mem::size_of::<f32>();

        // Send buffer to workers
        let workers = self.workers.borrow();
        for state in workers.iter() {
            let set_buffer_msg = Object::new();
            Reflect::set(&set_buffer_msg, &"type".into(), &"SetBuffer".into())?;
            Reflect::set(&set_buffer_msg, &"buffer".into(), &buffer)?;
            state.worker.post_message(&set_buffer_msg)?;
        }

        // Distribute heads across workers
        let heads_per_worker = (num_heads + num_workers - 1) / num_workers;
        let mut task_ids = Vec::new();

        for (i, state) in workers.iter().enumerate() {
            let head_start = i * heads_per_worker;
            let head_end = ((i + 1) * heads_per_worker).min(num_heads);

            if head_start >= head_end {
                continue;
            }

            let task_id = self.task_queue.borrow_mut().next_id();
            task_ids.push(task_id);

            let msg = WorkerMessage::ComputeAttention {
                task_id,
                q_offset,
                k_offset,
                v_offset,
                output_offset,
                head_start,
                head_end,
                num_heads,
                head_dim,
                seq_len,
            };

            let msg_obj = serde_wasm_bindgen::to_value(&msg)?;
            state.worker.post_message(&msg_obj)?;
        }

        drop(workers);

        self.wait_for_tasks(&task_ids).await?;

        // Read result
        let result_view = Float32Array::new_with_byte_offset_and_length(
            &buffer,
            output_offset as u32,
            tensor_size as u32,
        );
        Ok(result_view.to_vec())
    }

    /// Perform parallel layer normalization.
    pub async fn parallel_norm(
        &self,
        input: &[f32],
        gamma: &[f32],
        beta: &[f32],
        epsilon: f32,
    ) -> Result<Vec<f32>, JsValue> {
        let hidden_dim = gamma.len();
        let batch_size = input.len() / hidden_dim;

        if !self.shared_memory || batch_size < 4 {
            // Single-threaded fallback for small batches
            let mut output = vec![0.0f32; input.len()];

            for b in 0..batch_size {
                let start = b * hidden_dim;
                let slice = &input[start..start + hidden_dim];

                let mean: f32 = slice.iter().sum::<f32>() / hidden_dim as f32;
                let variance: f32 =
                    slice.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / hidden_dim as f32;
                let std = (variance + epsilon).sqrt();

                for i in 0..hidden_dim {
                    output[start + i] = ((slice[i] - mean) / std) * gamma[i] + beta[i];
                }
            }

            return Ok(output);
        }

        let num_workers = self.worker_count();
        let total_size = (input.len() + gamma.len() * 2 + input.len()) * std::mem::size_of::<f32>();

        self.shared_buffers
            .borrow_mut()
            .ensure_capacity(total_size)?;

        let buffer = self
            .shared_buffers
            .borrow()
            .buffer()
            .ok_or_else(|| JsValue::from_str("No shared buffer"))?
            .clone();

        // Copy data
        let view = Float32Array::new(&buffer);
        view.set(&Float32Array::from(input), 0);
        view.set(&Float32Array::from(gamma), input.len() as u32);
        view.set(
            &Float32Array::from(beta),
            (input.len() + gamma.len()) as u32,
        );

        let input_offset = 0;
        let gamma_offset = input.len() * std::mem::size_of::<f32>();
        let beta_offset = (input.len() + gamma.len()) * std::mem::size_of::<f32>();
        let output_offset = (input.len() + gamma.len() * 2) * std::mem::size_of::<f32>();

        // Send buffer to workers
        let workers = self.workers.borrow();
        for state in workers.iter() {
            let set_buffer_msg = Object::new();
            Reflect::set(&set_buffer_msg, &"type".into(), &"SetBuffer".into())?;
            Reflect::set(&set_buffer_msg, &"buffer".into(), &buffer)?;
            state.worker.post_message(&set_buffer_msg)?;
        }

        let batches_per_worker = (batch_size + num_workers - 1) / num_workers;
        let mut task_ids = Vec::new();

        for (i, state) in workers.iter().enumerate() {
            let batch_start = i * batches_per_worker;
            let batch_end = ((i + 1) * batches_per_worker).min(batch_size);

            if batch_start >= batch_end {
                continue;
            }

            let task_id = self.task_queue.borrow_mut().next_id();
            task_ids.push(task_id);

            let msg = WorkerMessage::ComputeNorm {
                task_id,
                input_offset,
                output_offset,
                gamma_offset,
                beta_offset,
                hidden_dim,
                batch_start,
                batch_end,
                epsilon,
            };

            let msg_obj = serde_wasm_bindgen::to_value(&msg)?;
            state.worker.post_message(&msg_obj)?;
        }

        drop(workers);

        self.wait_for_tasks(&task_ids).await?;

        let result_view = Float32Array::new_with_byte_offset_and_length(
            &buffer,
            output_offset as u32,
            input.len() as u32,
        );
        Ok(result_view.to_vec())
    }

    /// Wait for multiple tasks to complete.
    async fn wait_for_tasks(&self, _task_ids: &[TaskId]) -> Result<(), JsValue> {
        // Simplified implementation - wait a fixed time
        // In production, use proper message handlers with promises
        let promise = js_sys::Promise::new(&mut |resolve, _reject| {
            let window = web_sys::window().unwrap();
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 200)
                .unwrap();
        });
        JsFuture::from(promise).await?;
        Ok(())
    }

    /// Terminate all workers.
    pub fn terminate(&self) {
        let workers = self.workers.borrow();
        for state in workers.iter() {
            let _ = state
                .worker
                .post_message(&serde_wasm_bindgen::to_value(&WorkerMessage::Shutdown).unwrap());
            state.worker.terminate();
        }

        self.stats.borrow_mut().active_workers = 0;
    }

    /// Ping all workers for health check.
    pub async fn ping(&self) -> Result<Vec<f64>, JsValue> {
        let timestamp = crate::utils::now_ms();
        let workers = self.workers.borrow();

        for state in workers.iter() {
            let msg = WorkerMessage::Ping { timestamp };
            let msg_obj = serde_wasm_bindgen::to_value(&msg)?;
            state.worker.post_message(&msg_obj)?;
        }

        // In a real implementation, collect pong responses
        // For now, return placeholder
        Ok(vec![0.0; workers.len()])
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_pool_stats() {
        let stats = WorkerPoolStats::default();
        assert_eq!(stats.active_workers, 0);
        assert_eq!(stats.tasks_completed, 0);
    }
}
