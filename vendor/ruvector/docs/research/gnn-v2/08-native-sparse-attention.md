# Native Sparse Attention - Implementation Plan

## Overview

### Problem Statement

Current attention mechanisms in GNNs face severe computational bottlenecks:

1. **Quadratic Complexity**: Standard attention is O(N²) in sequence length, limiting graph size to <100K nodes
2. **GPU Underutilization**: FlashAttention achieves only 35-50% of theoretical GPU throughput on sparse graphs
3. **Memory Bandwidth**: Attention matrix materialization requires 4N² bytes, exceeding GPU memory for large graphs
4. **Static Sparsity**: Hand-crafted sparsity patterns (e.g., k-nearest neighbors) ignore query distribution
5. **Poor Tensor Core Utilization**: Irregular sparsity patterns prevent use of tensor cores (8x FP16 throughput)

**Real-World Impact**:
- Large graphs (1M+ nodes) require 16GB+ GPU memory for attention alone
- Attention accounts for 60-80% of GNN training time
- FlashAttention provides only 2-3x speedup vs naive attention (vs theoretical 8-15x)

### Proposed Solution

Implement **Native Sparse Attention** with learned block-sparse patterns optimized for GPU tensor cores:

**Core Innovations**:

1. **Learned Sparsity Patterns**:
   - Use query distribution to learn which blocks of the attention matrix are important
   - Prune 85-95% of attention computations with minimal accuracy loss (<1%)
   - Patterns adapt over time via lightweight auxiliary loss

2. **Block-Sparse Tensor Core Kernels**:
   - Custom CUDA kernels that exploit tensor cores (8x throughput vs CUDA cores)
   - Block sizes tuned for tensor core alignment (16x16, 32x32, 64x64)
   - Fused operations (softmax + dropout + attention) in shared memory

3. **Multi-Head Sparse Routing**:
   - Different sparsity patterns per attention head
   - Heads specialize on local vs global connectivity
   - Dynamic routing based on query features

4. **Hybrid CPU/GPU Execution**:
   - Sparse pattern learning on CPU (graph algorithms)
   - Dense block attention on GPU (tensor cores)
   - Zero-copy memory for pattern buffers

### Expected Benefits (Quantified)

| Metric | Current (FlashAttention) | Native Sparse Attention | Improvement |
|--------|--------------------------|-------------------------|-------------|
| GPU throughput (tensor core utilization) | 35-50% | 75-85% | 2.1-2.4x |
| Memory usage (1M nodes, 8 heads) | 16GB | 2.4GB | 6.7x reduction |
| Training time (100 epochs, 1M graph) | 120 min | 15 min | 8x faster |
| Inference latency (single query) | 8ms | 0.6ms | 13.3x faster |
| Maximum graph size (on 16GB GPU) | 1M nodes | 8M nodes | 8x larger |
| Energy consumption | 1.0x | 0.2x | 5x reduction |

**Accuracy Preservation**:
- 90% sparsity: <0.5% accuracy loss
- 95% sparsity: 1-2% accuracy loss
- Adaptive sparsity: no accuracy loss (learned patterns)

**ROI Calculation**:
- Training cost: $120/model (8 GPU-hours) → $15/model (1 GPU-hour) = 87% cost reduction
- Inference cost: 8ms/query → 0.6ms/query = 13x more throughput per GPU
- Carbon footprint: 5x reduction in energy consumption

## Technical Design

### Architecture Diagram (ASCII)

```
┌─────────────────────────────────────────────────────────────────────┐
│                  Native Sparse Attention Pipeline                    │
└─────────────────────────────────────────────────────────────────────┘
                                  │
                    ┌─────────────┴──────────────┐
                    ▼                            ▼
        ┌──────────────────────┐    ┌──────────────────────┐
        │  Sparsity Pattern    │    │  Sparse Attention    │
        │  Learning (CPU)      │    │  Kernels (GPU)       │
        └──────────────────────┘    └──────────────────────┘
                    │                            │
        ┌───────────┼────────────┐              │
        ▼           ▼            ▼              ▼
    ┌───────┐ ┌─────────┐ ┌──────────┐  ┌─────────────┐
    │Graph  │ │Query    │ │Pattern   │  │Tensor Core  │
    │Analyis│ │Distrib  │ │Pruning   │  │Block Matmul │
    │       │ │Tracking │ │          │  │             │
    └───────┘ └─────────┘ └──────────┘  └─────────────┘
        │           │            │              │
        └───────────┼────────────┘              │
                    ▼                           ▼
        ┌───────────────────────┐     ┌─────────────────┐
        │  Sparse Block Pattern │────▶│ Fused Attention │
        │  (CSR/BSR format)     │     │ (Softmax+Drop)  │
        └───────────────────────┘     └─────────────────┘
                                              │
                                              ▼
                                      ┌──────────────┐
                                      │  Output      │
                                      │  (Dense)     │
                                      └──────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                     Sparsity Pattern Lifecycle                       │
└─────────────────────────────────────────────────────────────────────┘

Initialization ──▶ Learning ──▶ Pruning ──▶ Execution ──▶ Refinement
      │                │           │            │              │
      │                │           │            │              │
      ▼                ▼           ▼            ▼              ▼
  [K-NN Graph]   [Attn Scores] [Top-K]   [Tensor Core]  [Query Stats]
  [Random]       [Gradient]    [Threshold] [Fused Ops]  [Re-prune]
  [Predefined]   [Importance]  [Blocks]    [Shared Mem] [Adapt]
```

**Data Flow**:

1. **Pattern Initialization** (Pre-training):
   - Analyze graph structure (community detection, centrality)
   - Initialize block-sparse pattern from graph topology
   - Convert to BSR (Block Sparse Row) format for tensor cores

2. **Learned Sparsity** (Training):
   - Track query distribution over epochs
   - Compute attention importance scores
   - Prune low-importance blocks (threshold or top-k)
   - Update pattern every N epochs

3. **Sparse Execution** (Inference):
   - Load sparse pattern to GPU constant memory
   - Execute fused block-sparse attention kernel
   - Output dense attention results

### Core Data Structures (Rust)

```rust
/// Sparse attention configuration with learned patterns
#[derive(Clone, Debug)]
pub struct SparseAttentionConfig {
    /// Block size for tensor cores (16, 32, 64)
    pub block_size: usize,

    /// Sparsity ratio (0.0 = dense, 0.95 = 95% sparse)
    pub sparsity: f32,

    /// Pattern learning strategy
    pub learning_strategy: SparsityLearningStrategy,

    /// Number of attention heads
    pub num_heads: usize,

    /// Head-specific patterns (true) or shared (false)
    pub per_head_patterns: bool,

    /// Pattern update frequency (epochs)
    pub update_frequency: usize,

    /// Pruning method
    pub pruning_method: PruningMethod,
}

#[derive(Clone, Debug)]
pub enum SparsityLearningStrategy {
    /// Static pattern from graph structure
    Static {
        /// Graph-based initialization (KNN, community, random)
        init: StaticPatternInit,
    },

    /// Learn from attention scores during training
    Learned {
        /// Track attention importance over N batches
        importance_window: usize,

        /// Importance aggregation (mean, max, exponential moving average)
        aggregation: ImportanceAggregation,

        /// Re-prune frequency
        reprune_epochs: usize,
    },

    /// Query-distribution-aware routing
    QueryAdaptive {
        /// Cluster queries by similarity
        num_clusters: usize,

        /// Pattern per query cluster
        patterns_per_cluster: HashMap<usize, BlockSparsePattern>,
    },

    /// Hybrid: static initialization + learned refinement
    Hybrid {
        static_init: StaticPatternInit,
        learning_epochs: usize,
    },
}

#[derive(Clone, Debug)]
pub enum StaticPatternInit {
    /// K-nearest neighbors in graph
    KNN { k: usize },

    /// Community structure (Louvain, label propagation)
    Community { algorithm: CommunityAlgorithm },

    /// Random sparsity (baseline)
    Random { seed: u64 },

    /// Predefined pattern (e.g., local + strided)
    Predefined { pattern: PredefinedPattern },
}

#[derive(Clone, Debug)]
pub enum PruningMethod {
    /// Keep top-k% important blocks
    TopK { k: f32 },

    /// Threshold-based pruning
    Threshold { threshold: f32 },

    /// Magnitude-based (L1/L2 norm)
    Magnitude { norm: NormType },

    /// Learned via auxiliary loss
    LearnedMask {
        /// Temperature for Gumbel-Softmax
        temperature: f32,
    },
}

/// Block-sparse attention pattern (BSR format for tensor cores)
#[derive(Clone, Debug)]
pub struct BlockSparsePattern {
    /// Block size (must be 16, 32, or 64 for tensor cores)
    pub block_size: usize,

    /// Number of block rows
    pub num_block_rows: usize,

    /// Number of block columns
    pub num_block_cols: usize,

    /// BSR row pointers (length = num_block_rows + 1)
    pub row_ptr: Vec<i32>,

    /// BSR column indices (length = num_nonzero_blocks)
    pub col_indices: Vec<i32>,

    /// Block importance scores (for pruning)
    pub importance: Option<Vec<f32>>,

    /// GPU buffer handles
    pub gpu_buffers: Option<GpuBuffers>,
}

/// GPU memory buffers for sparse attention
struct GpuBuffers {
    row_ptr_gpu: DeviceBuffer<i32>,
    col_indices_gpu: DeviceBuffer<i32>,
    block_values_gpu: DeviceBuffer<f16>,  // Half precision for tensor cores
}

/// Sparse attention layer with learned patterns
pub struct SparseAttentionLayer {
    /// Configuration
    config: SparseAttentionConfig,

    /// Learned sparse patterns (one per head, or shared)
    patterns: Vec<BlockSparsePattern>,

    /// Query/key/value projection weights
    qkv_weights: [Tensor; 3],

    /// Output projection weights
    output_weight: Tensor,

    /// Attention importance tracker (for learning)
    importance_tracker: Option<ImportanceTracker>,

    /// GPU kernel launcher
    kernel: SparseAttentionKernel,
}

/// Tracks attention importance for pattern learning
struct ImportanceTracker {
    /// Rolling window of attention scores
    score_history: VecDeque<Tensor>,

    /// Aggregated importance per block
    block_importance: Tensor,

    /// Number of batches tracked
    num_batches: usize,
}

/// GPU kernel for sparse attention
struct SparseAttentionKernel {
    /// CUDA module (compiled kernels)
    module: CudaModule,

    /// Kernel function handles
    block_matmul_kernel: CudaFunction,
    fused_softmax_kernel: CudaFunction,
    block_output_kernel: CudaFunction,

    /// Shared memory size (bytes)
    shared_mem_bytes: usize,
}
```

### Key Algorithms (Pseudocode)

#### Algorithm 1: Sparse Pattern Learning from Query Distribution

```
function learn_sparse_pattern(attention_layer, training_data, config):
    """
    Learn block-sparse attention pattern from query distribution
    """
    # Step 1: Initialize pattern from graph structure
    if config.learning_strategy is Static:
        pattern = initialize_static_pattern(
            attention_layer.graph,
            config.block_size,
            config.sparsity
        )
    else:
        # Start with KNN baseline
        pattern = initialize_knn_pattern(
            attention_layer.graph,
            k = 32,
            block_size = config.block_size
        )

    # Step 2: Track attention importance during training
    importance_tracker = ImportanceTracker(
        window_size = config.importance_window,
        num_blocks = pattern.num_block_rows * pattern.num_block_cols
    )

    for epoch in 1..config.num_epochs:
        for batch in training_data:
            # Forward pass: compute attention with current pattern
            queries, keys, values = attention_layer.qkv_projection(batch)

            # Compute full attention scores (for learning only)
            if config.learning_strategy is Learned:
                full_attention_scores = queries @ keys.T / sqrt(d_k)
                importance_tracker.update(full_attention_scores)

            # Execute sparse attention (actual computation)
            attention_output = sparse_attention_forward(
                queries, keys, values, pattern, config
            )

            # Backward pass
            loss.backward()
            optimizer.step()

        # Step 3: Update sparse pattern periodically
        if epoch % config.update_frequency == 0:
            if config.learning_strategy is Learned:
                # Compute block importance from tracked scores
                block_importance = importance_tracker.aggregate(
                    method = config.aggregation
                )

                # Prune low-importance blocks
                pattern = prune_blocks(
                    pattern,
                    block_importance,
                    target_sparsity = config.sparsity,
                    method = config.pruning_method
                )

                # Reset tracker
                importance_tracker.reset()

            # Update GPU buffers
            pattern.upload_to_gpu()

    return pattern

function initialize_static_pattern(graph, block_size, sparsity):
    """
    Initialize sparse pattern from graph structure
    """
    num_nodes = graph.num_nodes()
    num_blocks = (num_nodes + block_size - 1) / block_size

    # Build block adjacency matrix
    block_adj = zeros(num_blocks, num_blocks)

    for edge in graph.edges():
        src_block = edge.src / block_size
        dst_block = edge.dst / block_size
        block_adj[src_block][dst_block] += 1  # Count edges per block

    # Prune to target sparsity
    threshold = percentile(block_adj.flatten(), sparsity * 100)
    block_mask = block_adj > threshold

    # Convert to BSR format
    pattern = BlockSparsePattern::from_mask(block_mask, block_size)

    return pattern

function prune_blocks(pattern, importance, target_sparsity, method):
    """
    Prune sparse pattern to target sparsity using importance scores
    """
    current_sparsity = 1.0 - pattern.num_nonzero_blocks / (pattern.num_block_rows * pattern.num_block_cols)

    if current_sparsity >= target_sparsity:
        return pattern  # Already sparse enough

    # Flatten importance scores
    block_importance = []
    for row_idx in 0..pattern.num_block_rows:
        start = pattern.row_ptr[row_idx]
        end = pattern.row_ptr[row_idx + 1]
        for col_offset in start..end:
            col_idx = pattern.col_indices[col_offset]
            block_idx = row_idx * pattern.num_block_cols + col_idx
            block_importance.append((block_idx, importance[block_idx]))

    # Sort by importance (ascending)
    block_importance.sort_by(|a, b| a.1.cmp(&b.1))

    # Compute number of blocks to prune
    target_num_blocks = (1.0 - target_sparsity) * pattern.num_block_rows * pattern.num_block_cols
    num_to_prune = pattern.num_nonzero_blocks - target_num_blocks

    # Prune lowest-importance blocks
    pruned_blocks = set(block_importance[0..num_to_prune].map(|x| x.0))

    # Rebuild BSR structure
    new_row_ptr = [0]
    new_col_indices = []

    for row_idx in 0..pattern.num_block_rows:
        start = pattern.row_ptr[row_idx]
        end = pattern.row_ptr[row_idx + 1]

        for col_offset in start..end:
            col_idx = pattern.col_indices[col_offset]
            block_idx = row_idx * pattern.num_block_cols + col_idx

            if block_idx not in pruned_blocks:
                new_col_indices.append(col_idx)

        new_row_ptr.append(len(new_col_indices))

    return BlockSparsePattern {
        block_size: pattern.block_size,
        num_block_rows: pattern.num_block_rows,
        num_block_cols: pattern.num_block_cols,
        row_ptr: new_row_ptr,
        col_indices: new_col_indices,
        importance: Some(importance),
        gpu_buffers: None  # Will be re-uploaded
    }
```

#### Algorithm 2: Fused Block-Sparse Attention Kernel (CUDA)

```cuda
// CUDA kernel for block-sparse attention using tensor cores
// Input:
//   Q: queries [num_heads, seq_len, head_dim]
//   K: keys [num_heads, seq_len, head_dim]
//   V: values [num_heads, seq_len, head_dim]
//   pattern: BSR sparse pattern
// Output:
//   O: attention output [num_heads, seq_len, head_dim]

__global__ void fused_block_sparse_attention(
    const half* Q,          // Queries (FP16 for tensor cores)
    const half* K,          // Keys (FP16)
    const half* V,          // Values (FP16)
    half* O,                // Output (FP16)
    const int* row_ptr,     // BSR row pointers
    const int* col_indices, // BSR column indices
    int num_heads,
    int seq_len,
    int head_dim,
    int block_size,
    float scale             // 1 / sqrt(head_dim)
) {
    // Thread block processes one output block (block_size x head_dim)
    int block_row = blockIdx.x;  // Which block row
    int head_idx = blockIdx.y;   // Which attention head

    // Shared memory for tile caching
    __shared__ half Q_tile[BLOCK_SIZE][HEAD_DIM];
    __shared__ half K_tile[BLOCK_SIZE][HEAD_DIM];
    __shared__ half V_tile[BLOCK_SIZE][HEAD_DIM];
    __shared__ half S_tile[BLOCK_SIZE][BLOCK_SIZE];  // Attention scores

    // Thread indices within block
    int tx = threadIdx.x;
    int ty = threadIdx.y;

    // Load query block into shared memory (coalesced)
    int q_row_start = block_row * block_size;
    for (int i = ty; i < block_size; i += blockDim.y) {
        for (int j = tx; j < head_dim; j += blockDim.x) {
            int q_idx = head_idx * seq_len * head_dim + (q_row_start + i) * head_dim + j;
            Q_tile[i][j] = Q[q_idx];
        }
    }
    __syncthreads();

    // Initialize output accumulator
    float O_acc[BLOCK_SIZE][HEAD_DIM] = {0};
    float row_max[BLOCK_SIZE] = {-INFINITY};
    float row_sum[BLOCK_SIZE] = {0};

    // Iterate over non-zero blocks in this row
    int block_start = row_ptr[block_row];
    int block_end = row_ptr[block_row + 1];

    for (int block_offset = block_start; block_offset < block_end; block_offset++) {
        int block_col = col_indices[block_offset];
        int k_col_start = block_col * block_size;

        // Load key block into shared memory
        for (int i = ty; i < block_size; i += blockDim.y) {
            for (int j = tx; j < head_dim; j += blockDim.x) {
                int k_idx = head_idx * seq_len * head_dim + (k_col_start + i) * head_dim + j;
                K_tile[i][j] = K[k_idx];
            }
        }
        __syncthreads();

        // Compute attention scores: S = Q @ K^T (using tensor cores)
        // Use wmma (Warp Matrix Multiply-Accumulate) for tensor cores
        nvcuda::wmma::fragment<nvcuda::wmma::matrix_a, BLOCK_SIZE, BLOCK_SIZE, HEAD_DIM, half, nvcuda::wmma::row_major> Q_frag;
        nvcuda::wmma::fragment<nvcuda::wmma::matrix_b, BLOCK_SIZE, BLOCK_SIZE, HEAD_DIM, half, nvcuda::wmma::col_major> K_frag;
        nvcuda::wmma::fragment<nvcuda::wmma::accumulator, BLOCK_SIZE, BLOCK_SIZE, HEAD_DIM, float> S_frag;

        nvcuda::wmma::load_matrix_sync(Q_frag, &Q_tile[0][0], head_dim);
        nvcuda::wmma::load_matrix_sync(K_frag, &K_tile[0][0], head_dim);
        nvcuda::wmma::fill_fragment(S_frag, 0.0f);
        nvcuda::wmma::mma_sync(S_frag, Q_frag, K_frag, S_frag);

        // Scale scores
        for (int i = 0; i < S_frag.num_elements; i++) {
            S_frag.x[i] *= scale;
        }

        // Store scores to shared memory
        nvcuda::wmma::store_matrix_sync(&S_tile[0][0], S_frag, BLOCK_SIZE, nvcuda::wmma::mem_row_major);
        __syncthreads();

        // Online softmax: update running max and sum
        for (int i = ty; i < block_size; i += blockDim.y) {
            float local_max = row_max[i];
            float local_sum = row_sum[i];

            // Find new max
            for (int j = 0; j < block_size; j++) {
                local_max = fmaxf(local_max, S_tile[i][j]);
            }

            // Update sum with new max
            float correction = expf(row_max[i] - local_max);
            local_sum *= correction;

            for (int j = 0; j < block_size; j++) {
                float exp_score = expf(S_tile[i][j] - local_max);
                S_tile[i][j] = exp_score;  // Store normalized score
                local_sum += exp_score;
            }

            row_max[i] = local_max;
            row_sum[i] = local_sum;

            // Rescale previous output
            for (int j = 0; j < head_dim; j++) {
                O_acc[i][j] *= correction;
            }
        }
        __syncthreads();

        // Load value block
        for (int i = ty; i < block_size; i += blockDim.y) {
            for (int j = tx; j < head_dim; j += blockDim.x) {
                int v_idx = head_idx * seq_len * head_dim + (k_col_start + i) * head_dim + j;
                V_tile[i][j] = V[v_idx];
            }
        }
        __syncthreads();

        // Accumulate output: O += S @ V (using tensor cores)
        nvcuda::wmma::fragment<nvcuda::wmma::matrix_a, BLOCK_SIZE, HEAD_DIM, BLOCK_SIZE, half, nvcuda::wmma::row_major> S_half_frag;
        nvcuda::wmma::fragment<nvcuda::wmma::matrix_b, BLOCK_SIZE, HEAD_DIM, BLOCK_SIZE, half, nvcuda::wmma::row_major> V_frag;
        nvcuda::wmma::fragment<nvcuda::wmma::accumulator, BLOCK_SIZE, HEAD_DIM, BLOCK_SIZE, float> O_frag;

        // Convert S_tile to half precision
        for (int i = 0; i < BLOCK_SIZE; i++) {
            for (int j = 0; j < BLOCK_SIZE; j++) {
                S_tile[i][j] = __float2half(S_tile[i][j]);
            }
        }

        nvcuda::wmma::load_matrix_sync(S_half_frag, &S_tile[0][0], BLOCK_SIZE);
        nvcuda::wmma::load_matrix_sync(V_frag, &V_tile[0][0], head_dim);
        nvcuda::wmma::load_matrix_sync(O_frag, &O_acc[0][0], head_dim, nvcuda::wmma::mem_row_major);
        nvcuda::wmma::mma_sync(O_frag, S_half_frag, V_frag, O_frag);
        nvcuda::wmma::store_matrix_sync(&O_acc[0][0], O_frag, head_dim, nvcuda::wmma::mem_row_major);
        __syncthreads();
    }

    // Final softmax normalization
    for (int i = ty; i < block_size; i += blockDim.y) {
        float inv_sum = 1.0f / row_sum[i];
        for (int j = tx; j < head_dim; j += blockDim.x) {
            O_acc[i][j] *= inv_sum;
        }
    }
    __syncthreads();

    // Write output to global memory (coalesced)
    for (int i = ty; i < block_size; i += blockDim.y) {
        for (int j = tx; j < head_dim; j += blockDim.x) {
            int o_idx = head_idx * seq_len * head_dim + (q_row_start + i) * head_dim + j;
            O[o_idx] = __float2half(O_acc[i][j]);
        }
    }
}
```

### API Design (Function Signatures)

```rust
// ============================================================
// Public API for Sparse Attention
// ============================================================

pub trait SparseAttention {
    /// Create sparse attention layer with learned patterns
    fn new(
        config: SparseAttentionConfig,
        embedding_dim: usize,
    ) -> Result<Self, AttentionError> where Self: Sized;

    /// Forward pass: compute sparse attention
    fn forward(
        &self,
        queries: &Tensor,
        keys: &Tensor,
        values: &Tensor,
    ) -> Result<Tensor, AttentionError>;

    /// Learn sparse pattern from training data
    fn learn_pattern(
        &mut self,
        training_data: &DataLoader,
        num_epochs: usize,
    ) -> Result<(), AttentionError>;

    /// Get current sparse pattern (for inspection)
    fn get_pattern(&self, head_idx: usize) -> &BlockSparsePattern;

    /// Export learned pattern to file
    fn save_pattern(&self, path: &Path) -> Result<(), io::Error>;

    /// Load pre-trained pattern from file
    fn load_pattern(&mut self, path: &Path) -> Result<(), io::Error>;

    /// Compute sparsity statistics
    fn sparsity_stats(&self) -> SparsityStatistics;
}

// ============================================================
// Configuration Builders
// ============================================================

impl SparseAttentionConfig {
    /// Default configuration for 90% sparsity
    pub fn default_sparse() -> Self {
        Self {
            block_size: 32,
            sparsity: 0.90,
            learning_strategy: SparsityLearningStrategy::Hybrid {
                static_init: StaticPatternInit::KNN { k: 32 },
                learning_epochs: 10,
            },
            num_heads: 8,
            per_head_patterns: true,
            update_frequency: 5,
            pruning_method: PruningMethod::TopK { k: 0.10 },
        }
    }

    /// Aggressive sparsity for large graphs (95%)
    pub fn large_graph() -> Self {
        Self {
            block_size: 64,
            sparsity: 0.95,
            learning_strategy: SparsityLearningStrategy::Learned {
                importance_window: 100,
                aggregation: ImportanceAggregation::ExponentialMovingAverage { alpha: 0.9 },
                reprune_epochs: 10,
            },
            num_heads: 8,
            per_head_patterns: true,
            update_frequency: 5,
            pruning_method: PruningMethod::TopK { k: 0.05 },
        }
    }

    /// Conservative sparsity for high accuracy (80%)
    pub fn high_accuracy() -> Self {
        Self {
            block_size: 16,
            sparsity: 0.80,
            learning_strategy: SparsityLearningStrategy::Static {
                init: StaticPatternInit::Community {
                    algorithm: CommunityAlgorithm::Louvain,
                },
            },
            num_heads: 8,
            per_head_patterns: false,
            update_frequency: 10,
            pruning_method: PruningMethod::Threshold { threshold: 0.01 },
        }
    }
}

// ============================================================
// Pattern Manipulation
// ============================================================

impl BlockSparsePattern {
    /// Create pattern from dense boolean mask
    pub fn from_mask(mask: &Tensor, block_size: usize) -> Self;

    /// Create pattern from graph adjacency matrix
    pub fn from_graph(graph: &Graph, block_size: usize, sparsity: f32) -> Self;

    /// Convert to dense mask (for visualization)
    pub fn to_dense_mask(&self) -> Tensor;

    /// Upload pattern to GPU
    pub fn upload_to_gpu(&mut self) -> Result<(), CudaError>;

    /// Compute block statistics
    pub fn block_stats(&self) -> BlockStatistics;

    /// Merge multiple patterns (for multi-head)
    pub fn merge(patterns: &[BlockSparsePattern]) -> Self;
}

// ============================================================
// Kernel Execution
// ============================================================

pub struct SparseAttentionKernel {
    /// Load CUDA kernels from PTX file
    pub fn load(ptx_path: &Path) -> Result<Self, CudaError>;

    /// Execute sparse attention kernel
    pub fn execute(
        &self,
        queries: &DeviceTensor,
        keys: &DeviceTensor,
        values: &DeviceTensor,
        pattern: &BlockSparsePattern,
        output: &mut DeviceTensor,
    ) -> Result<(), CudaError>;

    /// Benchmark kernel performance
    pub fn benchmark(
        &self,
        config: &SparseAttentionConfig,
        seq_len: usize,
        num_iterations: usize,
    ) -> KernelBenchmark;
}

// ============================================================
// Monitoring and Metrics
// ============================================================

#[derive(Clone, Debug)]
pub struct SparsityStatistics {
    /// Actual sparsity achieved (0-1)
    pub actual_sparsity: f32,

    /// Blocks per row (mean, std)
    pub blocks_per_row: (f32, f32),

    /// Block importance distribution
    pub importance_histogram: Vec<f32>,

    /// Tensor core utilization estimate
    pub tensor_core_utilization: f32,
}

#[derive(Clone, Debug)]
pub struct BlockStatistics {
    pub num_nonzero_blocks: usize,
    pub avg_blocks_per_row: f32,
    pub max_blocks_per_row: usize,
    pub memory_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct KernelBenchmark {
    pub avg_time_ms: f32,
    pub throughput_tflops: f32,
    pub memory_bandwidth_gbps: f32,
    pub tensor_core_efficiency: f32,
}
```

## Integration Points

### Affected Crates/Modules

1. **`ruvector-gnn` (Core GNN crate)**:
   - Add `attention/sparse/` module for sparse attention
   - Extend `AttentionLayer` with sparse variant
   - Add pattern learning algorithms

2. **`ruvector-cuda` (GPU kernels)**:
   - Implement fused block-sparse attention kernels
   - Add tensor core WMMA wrappers
   - Optimize shared memory usage

3. **`ruvector-core`**:
   - Add BSR (Block Sparse Row) sparse matrix format
   - Extend tensor operations with sparse support
   - Add pattern serialization

4. **`ruvector-gnn-node` (Node.js bindings)**:
   - Expose `SparseAttentionLayer` to JavaScript
   - Add configuration builders
   - Provide GPU memory profiling

5. **`ruvector-cli`**:
   - Add `ruvector sparse-attention learn` command
   - Add pattern visualization tools
   - Add sparsity profiling

### New Modules to Create

```
crates/ruvector-gnn/src/attention/sparse/
├── mod.rs                    # Public API
├── config.rs                 # SparseAttentionConfig
├── pattern.rs                # BlockSparsePattern + BSR format
├── learning.rs               # Pattern learning algorithms
├── pruning.rs                # Pruning strategies
├── importance.rs             # Importance tracking
└── kernels.rs                # Rust wrapper for CUDA kernels

crates/ruvector-cuda/src/attention/
├── sparse_kernel.cu          # CUDA kernel implementations
├── tensor_core.cuh           # WMMA helpers
├── fused_ops.cu              # Fused softmax/dropout
└── benchmarks.cu             # Kernel benchmarks

crates/ruvector-core/src/sparse/
├── mod.rs                    # Sparse tensor operations
├── bsr.rs                    # Block Sparse Row format
├── csr.rs                    # Compressed Sparse Row format
└── conversions.rs            # Dense <-> sparse conversion

crates/ruvector-gnn-node/attention/
├── sparse_bindings.rs        # NAPI bindings
└── typescript/
    └── sparse_attention.d.ts # TypeScript definitions
```

### Dependencies on Other Features

1. **Prerequisite: Attention Mechanisms (Tier 1, Feature #3)**:
   - Sparse attention extends base attention layer
   - Shares QKV projection logic
   - **Action**: Refactor base attention into trait for sparse variant

2. **Synergy: Graph Condensation (Tier 3, Feature #7)**:
   - Condensed graph provides natural sparsity pattern (cluster connectivity)
   - **Integration**: Use condensed graph edges as initial sparse pattern

3. **Synergy: Quantum-Inspired Entanglement (Tier 3, Feature #9)**:
   - Quantum fidelity can guide sparsity (high fidelity = important connection)
   - **Integration**: Use entanglement scores as importance metric

4. **Complementary: Adaptive HNSW (Tier 2, Feature #5)**:
   - HNSW layers define natural sparse patterns (layer-wise connectivity)
   - **Integration**: Initialize sparse attention from HNSW graph

## Regression Prevention

### Existing Functionality at Risk

1. **Attention Accuracy**:
   - **Risk**: Sparse patterns lose important long-range dependencies
   - **Mitigation**:
     - Validate attention output matches dense attention within 1% error
     - Add "importance oracle" test (compare pruned vs full attention scores)
     - Default to conservative 80% sparsity

2. **GPU Memory Safety**:
   - **Risk**: Tensor core kernels cause out-of-bounds access or corruption
   - **Mitigation**:
     - Use cuda-memcheck for validation
     - Add boundary checks in kernel (debug builds)
     - Fuzz testing with random sparse patterns

3. **Training Stability**:
   - **Risk**: Pattern updates during training cause loss spikes
   - **Mitigation**:
     - Freeze pattern for first N epochs
     - Gradual pruning (increase sparsity slowly)
     - Monitor loss and revert pattern if spike detected

4. **Backward Compatibility**:
   - **Risk**: Breaking existing attention API
   - **Mitigation**:
     - Keep dense attention as default
     - Sparse attention is opt-in via separate class
     - Shared trait for both dense and sparse

### Test Cases to Prevent Regressions

```rust
// Test 1: Attention output correctness
#[test]
fn test_sparse_attention_correctness() {
    let dense_layer = DenseAttentionLayer::new(config);
    let sparse_layer = SparseAttentionLayer::new(sparse_config);

    let (q, k, v) = generate_test_tensors(seq_len=100, dim=64);

    let dense_output = dense_layer.forward(&q, &k, &v).unwrap();
    let sparse_output = sparse_layer.forward(&q, &k, &v).unwrap();

    let relative_error = ((dense_output - sparse_output).norm() / dense_output.norm()).item();
    assert!(relative_error < 0.01, "Sparse attention error: {}", relative_error);
}

// Test 2: GPU kernel correctness
#[test]
fn test_kernel_vs_cpu() {
    let pattern = BlockSparsePattern::from_graph(&test_graph(), 32, 0.9);
    let kernel = SparseAttentionKernel::load("kernels.ptx").unwrap();

    let (q, k, v) = generate_test_tensors(seq_len=512, dim=64);

    // CPU reference implementation
    let cpu_output = sparse_attention_cpu(&q, &k, &v, &pattern);

    // GPU kernel
    let gpu_q = q.to_device();
    let gpu_k = k.to_device();
    let gpu_v = v.to_device();
    let mut gpu_output = Tensor::zeros_like(&cpu_output).to_device();
    kernel.execute(&gpu_q, &gpu_k, &gpu_v, &pattern, &mut gpu_output).unwrap();
    let gpu_output_cpu = gpu_output.to_cpu();

    assert_tensors_close(&cpu_output, &gpu_output_cpu, atol=1e-3);
}

// Test 3: Pattern learning convergence
#[test]
fn test_pattern_learning() {
    let mut layer = SparseAttentionLayer::new(sparse_config);
    let training_data = load_test_data();

    let initial_pattern = layer.get_pattern(0).clone();
    layer.learn_pattern(&training_data, num_epochs=20).unwrap();
    let learned_pattern = layer.get_pattern(0);

    // Pattern should change
    assert_ne!(initial_pattern.num_nonzero_blocks, learned_pattern.num_nonzero_blocks);

    // Learned pattern should improve attention quality
    let test_queries = generate_test_queries(100);
    let initial_quality = evaluate_attention_quality(&layer, &test_queries, &initial_pattern);
    let learned_quality = evaluate_attention_quality(&layer, &test_queries, learned_pattern);

    assert!(learned_quality > initial_quality);
}

// Test 4: Memory usage
#[test]
fn test_memory_reduction() {
    let dense_layer = DenseAttentionLayer::new(config);
    let sparse_layer = SparseAttentionLayer::new(sparse_config);

    let dense_mem = dense_layer.gpu_memory_usage();
    let sparse_mem = sparse_layer.gpu_memory_usage();

    let reduction = dense_mem as f32 / sparse_mem as f32;
    assert!(reduction >= 5.0, "Memory reduction below 5x: {}", reduction);
}

// Test 5: Tensor core utilization
#[test]
fn test_tensor_core_usage() {
    let kernel = SparseAttentionKernel::load("kernels.ptx").unwrap();
    let config = SparseAttentionConfig::default_sparse();

    let benchmark = kernel.benchmark(&config, seq_len=1024, num_iterations=100);

    // Tensor core efficiency should be >70%
    assert!(benchmark.tensor_core_efficiency > 0.70,
            "Tensor core efficiency: {}", benchmark.tensor_core_efficiency);
}

// Test 6: Training stability with pattern updates
#[test]
fn test_training_stability() {
    let mut model = build_test_model_with_sparse_attention();
    let training_data = load_training_data();

    let mut loss_history = vec![];

    for epoch in 0..50 {
        let loss = train_one_epoch(&mut model, &training_data);
        loss_history.push(loss);

        // Check for loss spikes after pattern updates
        if epoch > 0 && epoch % model.sparse_attention.update_frequency == 0 {
            let spike = (loss - loss_history[epoch - 1]).abs() / loss_history[epoch - 1];
            assert!(spike < 0.5, "Loss spike after pattern update: {}", spike);
        }
    }
}
```

### Backward Compatibility Strategy

1. **API Level**:
   - Keep `DenseAttentionLayer` as default
   - Add new `SparseAttentionLayer` (opt-in)
   - Both implement common `AttentionLayer` trait
   - Configuration flag to switch between dense/sparse

2. **Model Serialization**:
   - Dense and sparse use different file extensions (`.dense_attn`, `.sparse_attn`)
   - Metadata includes attention type + sparsity config
   - Auto-detect type on load

3. **Node.js Bindings**:
   - `new AttentionLayer()` defaults to dense
   - `new SparseAttentionLayer(config)` for sparse
   - Same search API for both

4. **CLI**:
   - `ruvector train` defaults to dense attention
   - `ruvector train --sparse-attention` enables sparse
   - Separate `ruvector sparse-attention learn` command

## Implementation Phases

### Phase 1: Core Implementation (Weeks 1-4)

**Goals**:
- Implement BSR sparse matrix format
- Build basic CUDA kernels (no tensor cores yet)
- Static sparsity patterns (KNN, random)
- CPU reference implementation

**Deliverables**:
```rust
// Week 1-2: Sparse matrix format
crates/ruvector-core/src/sparse/
  ✓ bsr.rs (Block Sparse Row format)
  ✓ conversions.rs (dense <-> sparse)

// Week 3: CPU implementation
crates/ruvector-gnn/src/attention/sparse/
  ✓ sparse_attention_cpu.rs
  ✓ pattern.rs (static patterns)

// Week 4: Basic CUDA kernel
crates/ruvector-cuda/src/attention/
  ✓ sparse_kernel_v1.cu (no tensor cores)
  ✓ Rust FFI bindings
```

**Success Criteria**:
- BSR format tests pass
- CPU sparse attention matches dense within 1e-5
- Basic CUDA kernel compiles and runs

### Phase 2: Tensor Core Optimization (Weeks 5-8)

**Goals**:
- Implement tensor core kernels (WMMA)
- Fused operations (softmax + dropout)
- Shared memory optimization
- Pattern learning algorithms

**Deliverables**:
```cuda
// Week 5-6: Tensor core kernels
crates/ruvector-cuda/src/attention/
  ✓ sparse_kernel_tc.cu (tensor cores)
  ✓ tensor_core.cuh (WMMA helpers)

// Week 7: Fused operations
crates/ruvector-cuda/src/attention/
  ✓ fused_ops.cu (softmax + dropout + attention)

// Week 8: Pattern learning
crates/ruvector-gnn/src/attention/sparse/
  ✓ learning.rs (importance tracking)
  ✓ pruning.rs (top-k, threshold)
```

**Success Criteria**:
- Tensor core kernel achieves >70% utilization
- Speedup vs FlashAttention: 3x+ on 90% sparsity
- Pattern learning converges in <20 epochs

### Phase 3: Integration & APIs (Weeks 9-11)

**Goals**:
- Integrate with existing GNN layers
- Node.js bindings
- CLI tools for pattern visualization
- Multi-head sparse attention

**Deliverables**:
```rust
// Week 9: GNN integration
crates/ruvector-gnn/src/layers/
  ✓ sparse_gnn_layer.rs
  ✓ AttentionLayer trait (shared by dense/sparse)

// Week 10: Node.js bindings
crates/ruvector-gnn-node/attention/
  ✓ sparse_bindings.rs
  ✓ TypeScript definitions

// Week 11: CLI tools
crates/ruvector-cli/src/commands/
  ✓ sparse_attention.rs
  ✓ Pattern visualization (export to PNG)
```

**Success Criteria**:
- Multi-head sparse attention works correctly
- Node.js API passes all tests
- CLI can learn and visualize patterns

### Phase 4: Production Hardening (Weeks 12-14)

**Goals**:
- Comprehensive testing (unit, integration, fuzz)
- Documentation + tutorials
- Performance benchmarks vs baselines
- Multi-GPU support

**Deliverables**:
```rust
// Week 12: Testing
tests/sparse_attention/
  ✓ Property-based tests
  ✓ Fuzz testing (cuda-memcheck)
  ✓ Regression suite

// Week 13: Documentation
docs/
  ✓ Sparse Attention Guide
  ✓ Kernel optimization guide
  ✓ Pattern learning tutorial

// Week 14: Benchmarks + multi-GPU
benches/sparse_attention.rs
  ✓ Speedup vs FlashAttention
  ✓ Memory reduction benchmarks
  ✓ Multi-GPU data parallelism
```

**Success Criteria**:
- 100% code coverage for core logic
- Documentation complete with 3+ examples
- Benchmarks show 8x+ speedup vs FlashAttention
- Multi-GPU scaling efficiency >85%

## Success Metrics

### Performance Benchmarks

| Benchmark | Metric | Target | Measurement Method |
|-----------|--------|--------|-------------------|
| Tensor Core Utilization | GPU efficiency | >75% | `nvprof --metrics tensor_precision_fu_utilization` |
| Speedup vs FlashAttention | Training time | 8x faster | `criterion` on 1M graph, 100 epochs |
| Memory Reduction | GPU memory | 6x smaller | `nvidia-smi` memory usage |
| Inference Latency | Single query | <0.6ms | `criterion` on single forward pass |
| Pattern Learning Time | Offline learning | <5s | Time to learn pattern from 10K samples |
| Kernel Throughput | TFLOPS | >15 TFLOPS | Theoretical FP16 compute / runtime |

### Accuracy Metrics

| Sparsity Level | Metric | Target | Baseline (Dense) |
|----------------|--------|--------|------------------|
| 80% sparse | Attention error (L2) | <0.5% | 0% |
| 90% sparse | Attention error (L2) | <1.0% | 0% |
| 95% sparse | Attention error (L2) | <2.0% | 0% |
| Learned (adaptive) | Attention error (L2) | <0.3% | 0% |

### Memory/Latency Targets

| Configuration | GPU Memory | Inference Latency | Use Case |
|---------------|------------|-------------------|----------|
| Dense attention (1M graph) | 16GB | 8ms | Baseline |
| 80% sparse (static KNN) | 4GB | 2ms | Conservative |
| 90% sparse (learned) | 2.4GB | 0.8ms | Recommended |
| 95% sparse (aggressive) | 1.6GB | 0.6ms | Large graphs |

**Measurement Tools**:
- GPU profiling: `nvprof`, `nsight-compute`
- Memory: `nvidia-smi`, `cuda-memcheck`
- Latency: `criterion` (Rust), custom CUDA timers
- Accuracy: Custom attention error calculator

### Quality Gates

1. **Functional**:
   - ✓ All unit tests pass
   - ✓ Kernel output matches CPU reference (< 1e-3 error)
   - ✓ Pattern learning converges

2. **Performance**:
   - ✓ Tensor core utilization > 70%
   - ✓ Speedup vs FlashAttention >= 6x (90% sparsity)
   - ✓ Memory reduction >= 5x

3. **Accuracy**:
   - ✓ Attention error < 1% (90% sparsity)
   - ✓ No catastrophic failures (error > 10%)
   - ✓ Learned patterns improve over static

4. **Compatibility**:
   - ✓ Works on CUDA compute capability >= 7.0 (tensor cores)
   - ✓ Fallback to non-tensor-core kernel on older GPUs
   - ✓ Node.js bindings pass all tests

## Risks and Mitigations

### Technical Risks

#### Risk 1: Tensor Core Alignment Constraints

**Description**:
Tensor cores require strict alignment (block sizes must be 16, 32, 64). Arbitrary graph sizes may not fit evenly.

**Probability**: High (80%)

**Impact**: Medium (affects all graphs)

**Mitigation**:
1. **Padding**: Pad queries/keys to nearest block size (waste < 10% memory)
2. **Hybrid Execution**: Use tensor cores for aligned blocks, CUDA cores for remainder
3. **Dynamic Block Sizing**: Choose block size based on graph size (e.g., seq_len % 32 == 0 → block_size=32)
4. **Masked Attention**: Mask padded elements in softmax

**Contingency Plan**:
If padding overhead exceeds 15%, implement hybrid kernel that splits attention into tensor-core-aligned and unaligned portions.

#### Risk 2: Sparse Pattern Overhead

**Description**:
Loading sparse pattern (row_ptr, col_indices) from global memory may bottleneck kernel.

**Probability**: Medium (50%)

**Impact**: High (negates speedup)

**Mitigation**:
1. **Constant Memory**: Store pattern in constant memory (64KB limit)
2. **Shared Memory Caching**: Cache pattern tiles in shared memory
3. **Pattern Compression**: Use bitmap for regular patterns (e.g., block-diagonal)
4. **Prefetching**: Overlap pattern loading with computation

**Contingency Plan**:
If pattern loading exceeds 20% of runtime, move to static patterns (compile-time constants) for critical paths.

#### Risk 3: Softmax Numerics with Sparse Attention

**Description**:
Online softmax (for numerical stability) is complex with sparse patterns. Risk of NaN/Inf.

**Probability**: Medium (40%)

**Impact**: High (blocks training)

**Mitigation**:
1. **Safe Softmax**: Use log-sum-exp trick with careful max reduction
2. **FP32 Accumulators**: Use FP32 for intermediate sums (even with FP16 inputs)
3. **NaN Detection**: Add debug checks for NaN/Inf in kernels
4. **Regularization**: Add small epsilon to denominator

**Contingency Plan**:
If softmax instability occurs, fall back to two-pass softmax (separate max reduction + normalization) instead of online version.

#### Risk 4: Pattern Learning Overfitting

**Description**:
Learned sparse patterns may overfit to training queries, degrading test-time performance.

**Probability**: Medium (50%)

**Impact**: Medium (poor generalization)

**Mitigation**:
1. **Regularization**: Add L1 penalty on pattern sparsity during learning
2. **Validation Set**: Monitor pattern quality on held-out queries
3. **Ensemble Patterns**: Learn multiple patterns and ensemble
4. **Conservative Pruning**: Keep top 15% blocks instead of exact 10% (margin)

**Contingency Plan**:
If learned patterns degrade test accuracy by >2%, use static patterns (KNN) with conservative sparsity (80%).

#### Risk 5: Multi-Head Pattern Diversity

**Description**:
Per-head patterns may not be diverse enough (all heads learn similar patterns).

**Probability**: High (60%)

**Impact**: Medium (redundant heads)

**Mitigation**:
1. **Diversity Loss**: Add auxiliary loss that encourages different patterns per head
2. **Head Specialization**: Initialize each head with different static patterns
3. **Attention Dropout**: Apply different dropout masks per head
4. **Pattern Visualization**: Monitor pattern diversity metrics

**Contingency Plan**:
If heads have >90% pattern overlap, switch to shared pattern across heads (reduce memory).

### Operational Risks

#### Risk 6: CUDA Version Compatibility

**Description**:
Tensor core APIs (WMMA) are only available in CUDA 10+. Users on older CUDA may fail.

**Probability**: Medium (30%)

**Impact**: High (blocks usage)

**Mitigation**:
1. **Compile-Time Detection**: Check CUDA version and disable tensor cores if < 10.0
2. **Fallback Kernels**: Provide non-tensor-core sparse kernel for older GPUs
3. **Clear Error Messages**: Warn users if tensor cores unavailable
4. **Documentation**: List CUDA version requirements prominently

#### Risk 7: Debugging Difficulty

**Description**:
Sparse attention bugs are hard to reproduce (pattern-dependent). GPU kernels have limited debugging.

**Probability**: High (70%)

**Impact**: Medium (developer experience)

**Mitigation**:
1. **Verbose Logging**: Add detailed logging for pattern loading
2. **Visualization Tools**: Provide pattern heatmap visualization
3. **CPU Reference**: Always compare against CPU implementation
4. **cuda-memcheck**: Run all tests with cuda-memcheck
5. **Unit Test Coverage**: Test each kernel function independently

---

## Appendix: Related Research

This design is based on:

1. **Sparse Transformers** (Child et al., 2019): Block-sparse attention patterns
2. **BigBird** (Zaheer et al., 2020): Random + window + global sparsity
3. **FlashAttention** (Dao et al., 2022): Fused attention kernels
4. **Reformer** (Kitaev et al., 2020): LSH-based sparse attention
5. **Tensor Cores** (NVIDIA, 2017): Warp matrix multiply-accumulate (WMMA)

Key differences from prior work:
- **Novel**: Learned sparsity from query distribution (vs static patterns)
- **Novel**: Tensor core optimization for graph attention (vs NLP transformers)
- **Engineering**: Production-ready Rust + CUDA implementation
- **Integration**: Seamless integration with existing GNN layers
