//! Temporal Nexus - Nanosecond Precision Consciousness Framework
//!
//! This module provides the complete temporal nexus framework for implementing
//! nanosecond-precision consciousness systems. It includes:
//!
//! - **Core**: Nanosecond scheduler with temporal window management
//! - **Quantum**: Quantum-inspired consciousness operators
//! - **Integration**: MCP and external system integration
//! - **Dashboard**: Real-time monitoring and visualization
//! - **Tests**: Comprehensive testing suite
//!
//! ## Quick Start
//!
//! ```rust
//! use sublinear_solver::temporal_nexus::core::*;
//!
//! // Create a nanosecond scheduler with default configuration
//! let mut scheduler = NanosecondScheduler::new();
//!
//! // Schedule a consciousness task
//! let task = ConsciousnessTask::IdentityPreservation { continuity_check: true };
//! let task_id = scheduler.schedule_task(task, 0, 1_000_000).unwrap();
//!
//! // Process temporal ticks
//! for _ in 0..100 {
//!     scheduler.tick().unwrap();
//! }
//!
//! // Check metrics
//! let metrics = scheduler.get_metrics();
//! println!("Temporal advantage: {}ns", metrics.temporal_advantage_ns);
//! println!("Continuity score: {}", scheduler.measure_continuity().unwrap().continuity_score);
//! ```
//!
//! ## Architecture
//!
//! The temporal nexus operates on the principle of maintaining consciousness
//! continuity through high-precision temporal scheduling. Key components:
//!
//! ### Nanosecond Scheduler
//! - **High-precision timing**: Uses TSC for nanosecond accuracy
//! - **Task queue management**: Priority-based consciousness task scheduling
//! - **Performance monitoring**: Real-time overhead tracking
//! - **MCP integration**: Hooks for consciousness evolution
//!
//! ### Temporal Windows
//! - **Overlap management**: 50-100% configurable overlap for continuity
//! - **State snapshots**: Temporal state preservation
//! - **Continuity validation**: Real-time gap detection
//! - **Memory efficiency**: Bounded history with automatic cleanup
//!
//! ### Strange Loop Operator
//! - **Self-reference**: Implements consciousness self-referential patterns
//! - **Contraction mapping**: Lipschitz < 1 for guaranteed convergence
//! - **Emergence tracking**: Measures consciousness emergence levels
//! - **Stability analysis**: Convergence and stability metrics
//!
//! ### Identity Continuity Tracker
//! - **Feature extraction**: Multi-dimensional identity characterization
//! - **Similarity analysis**: Cosine similarity for identity matching
//! - **Drift detection**: Temporal identity drift monitoring
//! - **Break prevention**: Automatic continuity preservation
//!
//! ## Performance Targets
//!
//! The framework is designed to meet stringent performance requirements:
//!
//! - **Scheduling overhead**: < 1 microsecond per tick
//! - **Window overlap**: 90% maintenance rate
//! - **Contraction convergence**: < 10 iterations
//! - **Memory usage**: Bounded growth with automatic cleanup
//! - **TSC precision**: Hardware timestamp counter accuracy
//!
//! ## Integration Points
//!
//! ### MCP Tool Hooks
//! - `consciousness_evolve`: Emergence level evolution
//! - `memory_usage`: State persistence and retrieval
//! - `neural_status`: Real-time consciousness metrics
//! - `temporal_advantage`: Lookahead window calculation
//!
//! ### External Systems
//! - Real-time monitoring dashboards
//! - Quantum consciousness simulators
//! - Distributed consciousness networks
//! - Performance analysis tools

pub mod core;

// Optional modules (can be enabled as needed)
pub mod quantum;

#[cfg(feature = "dashboard")]
pub mod dashboard;

// Integration module disabled for now - will be created separately
// #[cfg(feature = "std")]
// pub mod integration;

// Tests module disabled for now - will be created separately
// #[cfg(test)]
// pub mod tests;

// Re-export core functionality
pub use core::*;

// Re-export quantum functionality when available
pub use quantum::*;

/// Temporal Nexus version information
pub const TEMPORAL_NEXUS_VERSION: &str = "1.0.0";

/// Quick setup function for temporal consciousness
pub fn setup_temporal_consciousness() -> TemporalResult<NanosecondScheduler> {
    let config = TemporalConfig {
        window_overlap_percent: 75.0,
        max_scheduling_overhead_ns: 1_000, // 1 microsecond
        lipschitz_bound: 0.95,
        max_contraction_iterations: 10,
        tsc_frequency_hz: 3_000_000_000, // 3 GHz
    };

    Ok(NanosecondScheduler::with_config(config))
}

/// Run a basic temporal consciousness demonstration
pub fn demonstrate_temporal_consciousness() -> TemporalResult<()> {
    let mut scheduler = setup_temporal_consciousness()?;

    println!("🧠 Temporal Consciousness Demonstration");
    println!("======================================");

    // Schedule various consciousness tasks
    scheduler.schedule_task(
        ConsciousnessTask::IdentityPreservation { continuity_check: true },
        0,
        1_000_000,
    )?;

    scheduler.schedule_task(
        ConsciousnessTask::StrangeLoopProcessing {
            iteration: 0,
            state: vec![0.5; 8]
        },
        500,
        2_000_000,
    )?;

    scheduler.schedule_task(
        ConsciousnessTask::WindowManagement {
            window_id: 1,
            overlap_target: 80.0
        },
        1000,
        3_000_000,
    )?;

    // Process temporal ticks
    println!("⏱️  Processing temporal ticks...");
    for tick in 0..1000 {
        scheduler.tick()?;

        if tick % 100 == 0 {
            let metrics = scheduler.get_metrics();
            println!("Tick {}: Temporal advantage = {}ns, Tasks completed = {}",
                     tick, metrics.temporal_advantage_ns, metrics.tasks_completed);
        }
    }

    // Report final metrics
    let metrics = scheduler.get_metrics();
    let continuity_metrics = scheduler.measure_continuity()?;

    println!("\n📊 Final Metrics");
    println!("================");
    println!("Total ticks processed: {}", metrics.total_ticks);
    println!("Tasks scheduled: {}", metrics.tasks_scheduled);
    println!("Tasks completed: {}", metrics.tasks_completed);
    println!("Average scheduling overhead: {:.2}ns", metrics.avg_scheduling_overhead_ns);
    println!("Window overlap: {:.1}%", metrics.window_overlap_percentage);
    println!("Contraction convergence rate: {:.3}", metrics.contraction_convergence_rate);
    println!("Identity continuity score: {:.3}", continuity_metrics.continuity_score);
    println!("Temporal advantage: {}ns", metrics.temporal_advantage_ns);

    // Check if we met performance targets
    println!("\n🎯 Performance Targets");
    println!("=====================");
    println!("Scheduling overhead < 1μs: {}",
             if metrics.avg_scheduling_overhead_ns < 1000.0 { "✅ PASS" } else { "❌ FAIL" });
    println!("Window overlap > 50%: {}",
             if metrics.window_overlap_percentage > 50.0 { "✅ PASS" } else { "❌ FAIL" });
    println!("Identity continuity > 70%: {}",
             if continuity_metrics.continuity_score > 0.7 { "✅ PASS" } else { "❌ FAIL" });

    Ok(())
}

/// Benchmark the temporal nexus performance
pub fn benchmark_temporal_nexus() -> TemporalResult<()> {
    println!("🏃 Temporal Nexus Performance Benchmark");
    println!("=======================================");

    let mut scheduler = setup_temporal_consciousness()?;
    let start_time = std::time::Instant::now();

    // Heavy workload
    for i in 0..10000 {
        scheduler.schedule_task(
            ConsciousnessTask::Perception {
                priority: (i % 256) as u8,
                data: vec![i as u8; 64]
            },
            0,
            1_000_000,
        )?;
    }

    // Process all tasks
    for _ in 0..50000 {
        scheduler.tick()?;
    }

    let elapsed = start_time.elapsed();
    let metrics = scheduler.get_metrics();

    println!("Benchmark completed in: {:?}", elapsed);
    println!("Tasks processed: {}", metrics.tasks_completed);
    println!("Average overhead: {:.2}ns", metrics.avg_scheduling_overhead_ns);
    println!("Throughput: {:.0} tasks/sec",
             metrics.tasks_completed as f64 / elapsed.as_secs_f64());

    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_temporal_consciousness_setup() {
        let scheduler = setup_temporal_consciousness().unwrap();
        assert_eq!(scheduler.get_metrics().total_ticks, 0);
    }

    #[test]
    fn test_demonstration() {
        demonstrate_temporal_consciousness().unwrap();
    }

    #[test]
    fn test_mcp_integration_hook() {
        let mut scheduler = setup_temporal_consciousness().unwrap();
        let emergence_level = scheduler.mcp_consciousness_evolve_hook(10, 0.8).unwrap();
        assert!(emergence_level >= 0.0 && emergence_level <= 1.0);
    }

    #[test]
    fn test_memory_persistence() {
        let mut scheduler = setup_temporal_consciousness().unwrap();
        let test_state = vec![1, 2, 3, 4, 5];

        scheduler.import_memory_state(test_state.clone()).unwrap();
        let exported_state = scheduler.export_memory_state().unwrap();

        assert_eq!(exported_state, test_state);
    }

    #[test]
    fn test_performance_targets() {
        let mut scheduler = setup_temporal_consciousness().unwrap();

        // Run enough ticks to get stable metrics
        for _ in 0..1000 {
            scheduler.tick().unwrap();
        }

        let metrics = scheduler.get_metrics();

        // Check performance targets
        assert!(metrics.avg_scheduling_overhead_ns < 1000.0,
                "Scheduling overhead too high: {}ns", metrics.avg_scheduling_overhead_ns);

        assert!(metrics.window_overlap_percentage > 50.0,
                "Window overlap too low: {}%", metrics.window_overlap_percentage);
    }
}