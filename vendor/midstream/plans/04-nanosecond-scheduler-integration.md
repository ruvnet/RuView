# Nanosecond-Scheduler Integration Strategy

## Executive Summary

This document details the integration of the `nanosecond-scheduler` crate into the Lean Agentic Learning System. The nanosecond-scheduler provides ultra-low-latency, high-precision task scheduling capabilities essential for real-time AI systems, high-frequency decision-making, and time-critical agent operations.

## Research Background

### Real-Time Scheduling Theory

**Definition**: Real-time scheduling involves allocating processor time to tasks with strict timing constraints, ensuring deadlines are met [1].

**Key Concepts**:

1. **Hard Real-Time** [1]: Missing a deadline is catastrophic
   - Medical devices
   - Industrial control systems
   - High-frequency trading

2. **Soft Real-Time** [2]: Missing deadlines degrades performance but isn't catastrophic
   - Video streaming
   - Interactive applications
   - AI inference

3. **Scheduling Algorithms** [3]:
   - **Rate-Monotonic (RM)**: Priority based on period
   - **Earliest Deadline First (EDF)**: Priority based on deadline
   - **Least Laxity First (LLF)**: Priority based on slack time

4. **Jitter and Latency** [4]:
   - **Jitter**: Variation in execution time
   - **Latency**: Time from trigger to execution
   - **Worst-Case Execution Time (WCET)**

### High-Precision Timing

**Modern Hardware Capabilities**:
- CPU TSC (Time Stamp Counter): Nanosecond precision
- HPET (High Precision Event Timer): ~10ns resolution
- RDTSC instruction: Direct cycle counting

**Operating System Support**:
- Linux: `CLOCK_MONOTONIC_RAW`, `SCHED_FIFO`
- RT-Linux patches for deterministic scheduling
- CPU isolation and affinity

### References

[1] Liu, C. L., & Layland, J. W. (1973). "Scheduling algorithms for multiprogramming in a hard-real-time environment." Journal of the ACM, 20(1), 46-61.

[2] Buttazzo, G. C. (2011). "Hard Real-Time Computing Systems." Springer.

[3] Sha, L., et al. (2004). "Real time scheduling theory: A historical perspective." Real-Time Systems, 28(2-3), 101-155.

[4] Kopetz, H. (2011). "Real-Time Systems: Design Principles for Distributed Embedded Applications." Springer.

[5] Brandenburg, B. B., & Anderson, J. H. (2007). "Feather-trace: A light-weight event tracing toolkit." OSPERT 2007.

## Integration Architecture

```
┌─────────────────────────────────────────────────────────────┐
│         Nanosecond-Scheduler Integration                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌────────────────┐        ┌─────────────────┐            │
│  │  High Priority │        │  Deadline       │            │
│  │  Task Queue    │◄──────►│  Manager        │            │
│  │  (ns precision)│        │                 │            │
│  └────────┬───────┘        └─────────┬───────┘            │
│           │                          │                     │
│           │                          ▼                     │
│  ┌────────▼───────┐        ┌─────────────────┐            │
│  │  CPU-Pinned    │        │  Latency        │            │
│  │  Workers       │◄──────►│  Monitor        │            │
│  └────────┬───────┘        └─────────────────┘            │
│           │                                                 │
│  ┌────────▼───────┐        ┌─────────────────┐            │
│  │  Agent         │        │  Real-Time      │            │
│  │  Execution     │◄──────►│  Constraints    │            │
│  └────────────────┘        └─────────────────┘            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Use Cases

### 1. High-Frequency Trading Bot

**Problem**: Execute trades within microsecond time windows.

**Solution**: Schedule trade decisions with nanosecond precision.

**Implementation**:
```rust
let mut scheduler = NanosecondScheduler::new();

// Schedule high-priority trade execution
scheduler.schedule_with_deadline(
    Task::ExecuteTrade(trade),
    Deadline::from_nanos(5_000), // 5 microseconds
    Priority::Critical,
);

// Ensure execution
scheduler.run_until_idle_with_guarantee();
```

### 2. Real-Time Sensor Fusion

**Problem**: Fuse data from multiple sensors with strict timing requirements.

**Solution**: Schedule sensor reads and fusion with precise timing.

**Implementation**:
```rust
// Schedule periodic sensor reads
scheduler.schedule_periodic(
    Task::ReadSensor(sensor_id),
    Period::from_micros(100), // 100μs period
    Priority::High,
);

// Schedule fusion with deadline
scheduler.schedule_with_deadline(
    Task::FuseSensorData,
    Deadline::from_micros(150),
    Priority::High,
);
```

### 3. Low-Latency Inference

**Problem**: ML inference must complete within strict latency budget.

**Solution**: Schedule inference with guaranteed execution time.

**Implementation**:
```rust
// Schedule inference with WCET guarantee
let wcet = estimate_worst_case_execution_time(&model);

scheduler.schedule_with_wcet(
    Task::RunInference(model, input),
    wcet,
    Deadline::from_micros(1000), // 1ms deadline
    Priority::High,
);
```

## Technical Specifications

### API Design

```rust
pub struct NanosecondScheduler {
    task_queue: PriorityQueue<ScheduledTask>,
    workers: Vec<CpuPinnedWorker>,
    latency_monitor: LatencyMonitor,
    config: SchedulerConfig,
}

pub struct ScheduledTask {
    pub id: TaskId,
    pub task: Task,
    pub priority: Priority,
    pub deadline: Option<Instant>,
    pub period: Option<Duration>,
    pub wcet: Option<Duration>,
}

pub enum Priority {
    Critical,  // RT priority 99
    High,      // RT priority 90
    Normal,    // RT priority 50
    Low,       // SCHED_OTHER
}

pub struct SchedulerConfig {
    pub enable_cpu_pinning: bool,
    pub enable_rt_scheduling: bool,
    pub num_workers: usize,
    pub latency_budget_ns: u64,
}

impl NanosecondScheduler {
    pub fn new(config: SchedulerConfig) -> Result<Self, Error>;

    pub fn schedule(
        &mut self,
        task: Task,
        priority: Priority,
    ) -> TaskHandle;

    pub fn schedule_with_deadline(
        &mut self,
        task: Task,
        deadline: Deadline,
        priority: Priority,
    ) -> TaskHandle;

    pub fn schedule_periodic(
        &mut self,
        task: Task,
        period: Period,
        priority: Priority,
    ) -> TaskHandle;

    pub fn schedule_with_wcet(
        &mut self,
        task: Task,
        wcet: Duration,
        deadline: Deadline,
        priority: Priority,
    ) -> TaskHandle;

    pub fn cancel(&mut self, handle: TaskHandle) -> Result<(), Error>;

    pub fn get_latency_stats(&self) -> LatencyStats;

    pub fn wait_for_completion(&self, handle: TaskHandle) -> Result<TaskResult, Error>;
}
```

### Performance Requirements

| Metric | Target | Rationale |
|--------|--------|-----------|
| Scheduling overhead | <100ns | Minimal impact |
| Jitter | <1μs | Predictable execution |
| Deadline miss rate | <0.001% | High reliability |
| Context switch latency | <2μs | Fast transitions |
| Wakeup latency | <10μs | Responsive |

## Integration Points

### 1. Agent Decision Scheduling

**Location**: `src/lean_agentic/agent.rs`

**Enhancement**:
```rust
pub struct RealTimeAgent {
    agent: AgenticLoop,
    scheduler: NanosecondScheduler,
    latency_budget: Duration,
}

impl RealTimeAgent {
    pub async fn make_decision_with_deadline(
        &mut self,
        context: &Context,
        deadline: Deadline,
    ) -> Result<Action, Error> {
        let task = Task::PlanAndAct {
            context: context.clone(),
        };

        let handle = self.scheduler.schedule_with_deadline(
            task,
            deadline,
            Priority::High,
        );

        // Wait for completion
        match self.scheduler.wait_for_completion(handle) {
            Ok(TaskResult::Action(action)) => Ok(action),
            Err(e) => Err(Error::DeadlineMissed(e)),
        }
    }
}
```

### 2. Stream Processing with Latency Guarantees

**Location**: `src/lean_agentic/learning.rs`

**Enhancement**:
```rust
impl StreamLearner {
    pub fn process_stream_with_latency_guarantee(
        &mut self,
        stream: impl Stream<Item = Message>,
        max_latency: Duration,
    ) -> impl Stream<Item = ProcessingResult> {
        let scheduler = NanosecondScheduler::new(config);

        stream.map(move |message| {
            let deadline = Instant::now() + max_latency;

            let handle = scheduler.schedule_with_deadline(
                Task::ProcessMessage(message),
                deadline,
                Priority::High,
            );

            scheduler.wait_for_completion(handle)
        })
    }
}
```

### 3. Knowledge Graph Updates with Priority

**Location**: `src/lean_agentic/knowledge.rs`

**Enhancement**:
```rust
impl KnowledgeGraph {
    pub fn update_with_priority(
        &mut self,
        entities: Vec<Entity>,
        priority: Priority,
    ) -> TaskHandle {
        self.scheduler.schedule(
            Task::UpdateKnowledgeGraph { entities },
            priority,
        )
    }

    pub fn critical_update(
        &mut self,
        entity: Entity,
        deadline: Deadline,
    ) -> Result<(), Error> {
        let handle = self.scheduler.schedule_with_deadline(
            Task::UpdateEntity { entity },
            deadline,
            Priority::Critical,
        );

        self.scheduler.wait_for_completion(handle)?;
        Ok(())
    }
}
```

## Implementation Phases

### Phase 1: Core Scheduler (Week 1)
- [ ] Implement priority queue
- [ ] Add CPU pinning support
- [ ] Create RT scheduling integration
- [ ] Implement basic task execution
- [ ] Write unit tests

### Phase 2: Deadline Management (Week 2)
- [ ] Add deadline tracking
- [ ] Implement EDF scheduling
- [ ] Create WCET estimation
- [ ] Add deadline miss detection
- [ ] Write integration tests

### Phase 3: Latency Monitoring (Week 3)
- [ ] Implement latency tracking
- [ ] Add jitter measurement
- [ ] Create performance metrics
- [ ] Add alerting for violations
- [ ] Benchmark performance

### Phase 4: Advanced Features (Week 4)
- [ ] Add periodic task support
- [ ] Implement admission control
- [ ] Create task dependencies
- [ ] Add load balancing
- [ ] Write documentation

## Benchmarking Strategy

### Benchmark Suite

```rust
#[bench]
fn bench_schedule_overhead(b: &mut Bencher) {
    let mut scheduler = NanosecondScheduler::new(default_config());
    let task = Task::Noop;

    b.iter(|| {
        scheduler.schedule(task.clone(), Priority::Normal)
    });
}

#[bench]
fn bench_deadline_scheduling(b: &mut Bencher) {
    let mut scheduler = NanosecondScheduler::new(default_config());
    let deadline = Deadline::from_micros(100);

    b.iter(|| {
        let handle = scheduler.schedule_with_deadline(
            Task::Compute(|_| 42),
            deadline,
            Priority::High,
        );
        scheduler.wait_for_completion(handle)
    });
}

#[bench]
fn bench_periodic_tasks(b: &mut Bencher) {
    let mut scheduler = NanosecondScheduler::new(default_config());

    b.iter(|| {
        scheduler.schedule_periodic(
            Task::Noop,
            Period::from_micros(100),
            Priority::Normal,
        )
    });
}
```

### Latency Measurement

```rust
#[test]
fn measure_scheduling_latency() {
    let mut scheduler = NanosecondScheduler::new(config);
    let mut latencies = Vec::new();

    for _ in 0..10000 {
        let start = Instant::now();

        let handle = scheduler.schedule(
            Task::Noop,
            Priority::High,
        );

        scheduler.wait_for_completion(handle).unwrap();

        let latency = start.elapsed();
        latencies.push(latency);
    }

    let stats = LatencyStats::from_samples(&latencies);

    assert!(stats.p99() < Duration::from_micros(10));
    assert!(stats.max() < Duration::from_micros(50));

    println!("Scheduling latency:");
    println!("  p50: {:?}", stats.p50());
    println!("  p99: {:?}", stats.p99());
    println!("  max: {:?}", stats.max());
}
```

## Platform-Specific Optimizations

### Linux

```rust
#[cfg(target_os = "linux")]
fn configure_rt_scheduling() -> Result<(), Error> {
    use libc::{sched_setscheduler, sched_param, SCHED_FIFO};

    let param = sched_param {
        sched_priority: 99,
    };

    unsafe {
        if sched_setscheduler(0, SCHED_FIFO, &param) != 0 {
            return Err(Error::RtSchedulingFailed);
        }
    }

    // Pin to isolated CPU
    pin_to_cpu(7)?;

    Ok(())
}

fn pin_to_cpu(cpu: usize) -> Result<(), Error> {
    use libc::{cpu_set_t, sched_setaffinity, CPU_SET, CPU_ZERO};

    unsafe {
        let mut cpu_set: cpu_set_t = std::mem::zeroed();
        CPU_ZERO(&mut cpu_set);
        CPU_SET(cpu, &mut cpu_set);

        if sched_setaffinity(0, std::mem::size_of::<cpu_set_t>(), &cpu_set) != 0 {
            return Err(Error::CpuPinningFailed);
        }
    }

    Ok(())
}
```

### Windows

```rust
#[cfg(target_os = "windows")]
fn configure_high_priority() -> Result<(), Error> {
    use winapi::um::processthreadsapi::{
        GetCurrentThread, SetThreadPriority
    };
    use winapi::um::winbase::THREAD_PRIORITY_TIME_CRITICAL;

    unsafe {
        let thread = GetCurrentThread();
        if SetThreadPriority(thread, THREAD_PRIORITY_TIME_CRITICAL) == 0 {
            return Err(Error::PrioritySettingFailed);
        }
    }

    Ok(())
}
```

## Success Criteria

- [ ] Scheduling overhead < 100ns (p99)
- [ ] Jitter < 1μs (p99)
- [ ] Deadline miss rate < 0.001%
- [ ] Context switch latency < 2μs
- [ ] Support for 10,000+ tasks/second
- [ ] Zero priority inversions in tests
- [ ] Full platform support (Linux, macOS, Windows)

## Safety and Error Handling

### Deadline Misses

```rust
pub enum DeadlineViolation {
    SoftMiss { actual: Duration, expected: Duration },
    HardMiss { actual: Duration, expected: Duration },
}

impl NanosecondScheduler {
    fn handle_deadline_miss(&mut self, task: &ScheduledTask, violation: DeadlineViolation) {
        match violation {
            DeadlineViolation::SoftMiss { actual, expected } => {
                tracing::warn!(
                    task_id = ?task.id,
                    actual_ns = actual.as_nanos(),
                    expected_ns = expected.as_nanos(),
                    "Soft deadline missed"
                );
            }
            DeadlineViolation::HardMiss { actual, expected } => {
                tracing::error!(
                    task_id = ?task.id,
                    actual_ns = actual.as_nanos(),
                    expected_ns = expected.as_nanos(),
                    "Hard deadline missed - critical violation"
                );
                self.trigger_emergency_protocol(task);
            }
        }
    }
}
```

## Monitoring Dashboard

```rust
pub struct LatencyMonitor {
    samples: RingBuffer<Duration>,
    violations: Vec<DeadlineViolation>,
    stats: LatencyStats,
}

impl LatencyMonitor {
    pub fn report(&self) -> MonitoringReport {
        MonitoringReport {
            p50_latency: self.stats.p50(),
            p99_latency: self.stats.p99(),
            max_latency: self.stats.max(),
            deadline_miss_rate: self.calculate_miss_rate(),
            jitter: self.calculate_jitter(),
            utilization: self.calculate_utilization(),
        }
    }
}
```

## Future Enhancements

1. **GPU Scheduling**: Extend to CUDA/OpenCL tasks
2. **Distributed Scheduling**: Coordinate across machines
3. **Energy-Aware**: Optimize for power consumption
4. **Predictive Scheduling**: ML-based WCET prediction
5. **Formal Verification**: Prove schedulability

## References

[1] Liu & Layland (1973). Scheduling algorithms for hard-real-time.
[2] Buttazzo (2011). Hard Real-Time Computing Systems.
[3] Sha et al. (2004). Real time scheduling theory.
[4] Kopetz (2011). Real-Time Systems.
[5] Brandenburg & Anderson (2007). Feather-trace.

## Appendix A: Example Usage

```rust
use midstream::nanosecond_scheduler::*;

// Create scheduler with RT configuration
let config = SchedulerConfig {
    enable_cpu_pinning: true,
    enable_rt_scheduling: true,
    num_workers: 4,
    latency_budget_ns: 1_000, // 1μs
};

let mut scheduler = NanosecondScheduler::new(config)?;

// Schedule high-priority task with deadline
let handle = scheduler.schedule_with_deadline(
    Task::ProcessCriticalEvent(event),
    Deadline::from_micros(100),
    Priority::Critical,
);

// Wait for completion
match scheduler.wait_for_completion(handle) {
    Ok(result) => println!("Completed: {:?}", result),
    Err(Error::DeadlineMissed(..)) => eprintln!("Deadline violated!"),
}

// Get performance statistics
let stats = scheduler.get_latency_stats();
println!("Latency p99: {:?}", stats.p99());
```
