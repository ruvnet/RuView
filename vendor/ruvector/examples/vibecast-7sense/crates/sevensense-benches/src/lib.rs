//! 7sense Benchmarks - Performance testing for all bounded contexts
//!
//! This crate contains comprehensive benchmarks for:
//! - HNSW vector search (150x speedup target)
//! - Perch 2.0 embedding inference
//! - Clustering algorithms (HDBSCAN, K-Means)
//! - API endpoint throughput

pub mod utils;

pub use utils::*;
