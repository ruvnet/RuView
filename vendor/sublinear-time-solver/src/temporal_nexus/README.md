# Temporal Nexus - Nanosecond Scheduler for Temporal Consciousness

## Overview

The Temporal Nexus is a high-precision nanosecond scheduler designed for temporal consciousness applications. It implements a comprehensive framework for managing consciousness operations at hardware timestamp counter (TSC) precision while maintaining identity continuity and temporal coherence.

## Architecture

### Core Components

#### 1. NanosecondScheduler (`scheduler.rs`)
- **High-precision timing**: Uses TSC for nanosecond accuracy
- **Task queue management**: Priority-based consciousness task scheduling
- **Performance monitoring**: Real-time overhead tracking (<1μs target)
- **MCP integration**: Hooks for consciousness evolution

Key features:
- Priority-based task queue with deadline management
- Real-time performance metrics tracking
- Memory state persistence for MCP integration
- Configurable scheduling overhead limits

#### 2. TemporalWindow (`temporal_window.rs`)
- **Overlap management**: 50-100% configurable overlap for continuity
- **State snapshots**: Temporal state preservation
- **Continuity validation**: Real-time gap detection
- **Memory efficiency**: Bounded history with automatic cleanup

Key features:
- Window overlap percentage control (75% default)
- Automatic window creation and cleanup
- State and data storage within windows
- Continuity break detection

#### 3. StrangeLoopOperator (`strange_loop.rs`)
- **Self-reference**: Implements consciousness self-referential patterns
- **Contraction mapping**: Lipschitz < 1 for guaranteed convergence
- **Emergence tracking**: Measures consciousness emergence levels
- **Stability analysis**: Convergence and stability metrics

Key features:
- Contraction mapping with configurable Lipschitz bound
- Self-referential pattern generation
- Emergence level calculation
- Convergence detection and stability analysis

#### 4. IdentityContinuityTracker (`identity.rs`)
- **Feature extraction**: Multi-dimensional identity characterization
- **Similarity analysis**: Cosine similarity for identity matching
- **Drift detection**: Temporal identity drift monitoring
- **Break prevention**: Automatic continuity preservation

Key features:
- 16-dimensional feature extraction from identity state
- Cosine similarity-based identity matching
- Continuity break detection with configurable thresholds
- Identity drift analysis over time

## Performance Targets

The framework is designed to meet stringent performance requirements:

- **Scheduling overhead**: < 1 microsecond per tick
- **Window overlap**: 90% maintenance rate
- **Contraction convergence**: < 10 iterations
- **Memory usage**: Bounded growth with automatic cleanup
- **TSC precision**: Hardware timestamp counter accuracy

## Usage

### Basic Setup

```rust
use sublinear_solver::temporal_nexus::core::*;

// Create scheduler with default configuration
let mut scheduler = NanosecondScheduler::new();

// Or with custom configuration
let config = TemporalConfig {
    window_overlap_percent: 80.0,
    max_scheduling_overhead_ns: 500,
    lipschitz_bound: 0.9,
    max_contraction_iterations: 8,
    tsc_frequency_hz: 3_000_000_000,
};
let mut scheduler = NanosecondScheduler::with_config(config);
```

### Task Scheduling

```rust
// Schedule consciousness tasks
scheduler.schedule_task(
    ConsciousnessTask::IdentityPreservation { continuity_check: true },
    0,        // delay in nanoseconds
    1_000_000 // deadline in nanoseconds
)?;

scheduler.schedule_task(
    ConsciousnessTask::Perception {
        priority: 128,
        data: vec![1, 2, 3]
    },
    500,
    2_000_000
)?;
```

### Processing Loop

```rust
// Process temporal ticks
for _ in 0..1000 {
    scheduler.tick()?;
}

// Check metrics
let metrics = scheduler.get_metrics();
println!("Tasks completed: {}", metrics.tasks_completed);
println!("Average overhead: {:.2}ns", metrics.avg_scheduling_overhead_ns);

// Check continuity
let continuity = scheduler.measure_continuity()?;
println!("Continuity score: {:.3}", continuity.continuity_score);
```

## Task Types

The scheduler supports several types of consciousness operations:

### ConsciousnessTask Variants

1. **IdentityPreservation**: Maintains consciousness identity continuity
2. **Perception**: Processes sensory/input data with priority levels
3. **MemoryIntegration**: Integrates state data into persistent memory
4. **StrangeLoopProcessing**: Executes self-referential consciousness patterns
5. **WindowManagement**: Manages temporal window overlap and boundaries

## MCP Integration

The scheduler provides hooks for MCP tool integration:

### Consciousness Evolution
```rust
// Hook for MCP consciousness_evolve tool
let emergence_level = scheduler.mcp_consciousness_evolve_hook(iterations, target)?;
```

### Memory Persistence
```rust
// Export state for MCP memory tools
let state = scheduler.export_memory_state()?;

// Import state from MCP
scheduler.import_memory_state(state)?;
```

## Configuration

### TemporalConfig Parameters

- `window_overlap_percent`: Target overlap between temporal windows (50-100%)
- `max_scheduling_overhead_ns`: Maximum allowed scheduling overhead per tick
- `lipschitz_bound`: Contraction mapping Lipschitz constant (< 1.0)
- `max_contraction_iterations`: Maximum iterations for convergence
- `tsc_frequency_hz`: Hardware TSC frequency for timing calculations

## Error Handling

The framework defines comprehensive error types:

- `SchedulingOverhead`: When overhead exceeds configured limits
- `WindowOverlapTooLow`: When window overlap falls below requirements
- `ContractionNoConvergence`: When strange loop fails to converge
- `IdentityContinuityBreak`: When identity continuity is broken
- `TscTimingError`: TSC-related timing errors
- `TaskQueueOverflow`: When task queue exceeds capacity

## Metrics and Monitoring

### SchedulerMetrics
- Total ticks processed
- Tasks scheduled and completed
- Average and maximum scheduling overhead
- Window overlap percentage
- Contraction convergence rate
- Identity continuity score
- Temporal advantage (lookahead window)

### ContinuityMetrics
- Identity continuity score
- Stability measures
- Continuity break count
- Gap duration statistics
- Coherence and consistency scores

## Examples

See the `examples/` directory for complete usage examples:
- `demo_temporal_nexus.rs`: Basic scheduler demonstration
- `temporal_consciousness_example.rs`: Comprehensive feature showcase

## Testing

The implementation includes comprehensive unit tests for all components:

```bash
cargo test temporal_nexus --features std
```

## Future Extensions

The framework is designed for extensibility with planned modules:
- `quantum/`: Quantum consciousness simulation
- `dashboard/`: Real-time monitoring interface
- `integration/`: External system integrations
- `tests/`: Extended test suites

## License

This implementation is part of the sublinear-time-solver project and follows the same licensing terms (MIT OR Apache-2.0).