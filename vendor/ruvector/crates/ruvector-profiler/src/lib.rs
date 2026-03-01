//! Memory, power, and latency profiling for attention-mechanism benchmarks.

pub mod config_hash;
pub mod csv_emitter;
pub mod latency;
pub mod memory;
pub mod power;

pub use config_hash::{config_hash, BenchConfig};
pub use csv_emitter::{write_latency_csv, write_memory_csv, write_results_csv, ResultRow};
pub use latency::{compute_latency_stats, LatencyRecord, LatencyStats};
pub use memory::{capture_memory, MemoryReport, MemorySnapshot, MemoryTracker};
pub use power::{EnergyResult, MockPowerSource, PowerSample, PowerSource, PowerTracker};
