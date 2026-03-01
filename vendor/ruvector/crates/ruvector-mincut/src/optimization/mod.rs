//! Performance Optimizations for j-Tree + BMSSP Implementation
//!
//! This module implements the SOTA optimizations from ADR-002-addendum-sota-optimizations.md:
//!
//! 1. **Degree-based presparse (DSpar)**: 5.9x speedup via effective resistance approximation
//! 2. **LRU Cache**: Path distance caching with prefetch optimization
//! 3. **SIMD Operations**: Vectorized distance array computations
//! 4. **Pool Allocators**: Memory-efficient allocations with lazy deallocation
//! 5. **Parallel Updates**: Rayon-based parallel level updates with work-stealing
//! 6. **WASM Optimization**: Batch operations and TypedArray transfers
//!
//! Target: Combined 10x speedup over naive implementation.

pub mod benchmark;
pub mod cache;
pub mod dspar;
pub mod parallel;
pub mod pool;
pub mod simd_distance;
pub mod wasm_batch;

// Re-exports
pub use benchmark::{BenchmarkResult, BenchmarkSuite, OptimizationBenchmark};
pub use cache::{CacheConfig, CacheStats, PathDistanceCache, PrefetchHint};
pub use dspar::{DegreePresparse, PresparseConfig, PresparseResult, PresparseStats};
pub use parallel::{ParallelConfig, ParallelLevelUpdater, WorkStealingScheduler};
pub use pool::{LazyLevel, LevelPool, PoolConfig, PoolStats};
pub use simd_distance::{DistanceArray, SimdDistanceOps};
pub use wasm_batch::{BatchConfig, TypedArrayTransfer, WasmBatchOps};
