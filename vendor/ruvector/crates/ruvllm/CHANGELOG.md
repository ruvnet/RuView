# Changelog

All notable changes to the ruvllm crate will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2025-01-19

### Added
- Multi-threaded GEMM/GEMV with Rayon (12.7x speedup on M4 Pro)
- Flash Attention 2 with auto block sizing (+10% throughput)
- INT8/INT4/Q4_K quantized inference kernels (4-8x memory reduction)
- Optimized Metal GPU shaders (simdgroup_matrix)
- Memory pool with arena allocator (zero-alloc inference)
- WASM support via ruvllm-wasm crate
- npm package integration (@ruvector/ruvllm v2)
- Paged attention for non-contiguous KV cache
- Grouped-Query Attention (GQA) and Multi-Query Attention (MQA) support
- Two-tier KV cache with FP16 tail and quantized cold storage
- MicroLoRA for real-time per-request adaptation (<1ms latency)
- EWC++ (Elastic Weight Consolidation) to prevent catastrophic forgetting
- SONA learning integration with three-tier loops (instant/background/deep)
- Native Metal compute shaders for M4 Pro optimization
- Candle backend integration for HuggingFace model loading

### Changed
- GEMV performance: 6 GFLOPS -> 35.9 GFLOPS (6x improvement)
- GEMM performance: 6 GFLOPS -> 19.2 GFLOPS (3.2x improvement)
- Cache blocking tuned for M4 Pro (96x64x256 tiles)
- 12x4 micro-kernel for better register utilization
- RMSNorm optimized with NEON SIMD (620ns for 4096 dim, 16x better than target)
- Flash Attention achieves 840us for 256-token sequences
- MicroLoRA forward pass: 8.56us scalar, 2.61us SIMD (117x/383x better than target)

### Fixed
- Parameter estimation accuracy for 7B models
- Doctest crate name compatibility
- KV cache migration batch sizing for latency spikes
- Memory bandwidth optimization for large matrix operations

### Performance Highlights (M4 Pro, 48GB RAM)

| Operation | Latency | Target | Status |
|-----------|---------|--------|--------|
| Flash Attention (256 seq) | 840us | <2ms | 2.4x better |
| RMSNorm (4096 dim) | 620ns | <10us | 16x better |
| GEMV (4096x4096) | 1.36ms | <5ms | 3.7x better |
| MicroLoRA forward (rank=2, dim=4096) | 8.56us | <1ms | 117x better |
| RoPE with tables (128 dim, 32 tokens) | 1.33us | <50us | 37x better |

## [0.1.32] - 2025-01-18

### Added
- Initial ruvllm-integration crate with basic LLM serving runtime
- Paged attention implementation
- KV cache management
- SONA learning integration scaffolding
- Basic NEON SIMD kernels for ARM64

### Dependencies
- ruvector-core for storage backend
- ruvector-sona for learning integration
- candle-core, candle-nn, candle-transformers for ML backend
- tokenizers for text processing
- hf-hub for model downloads
