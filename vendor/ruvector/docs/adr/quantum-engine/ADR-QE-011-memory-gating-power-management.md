# ADR-QE-011: Memory Gating & Power Management

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

---

## Context

ruVector is designed to operate within the Cognitum computing paradigm: a tile-based
architecture with 256 low-power processor cores, event-driven activation, and
aggressive power gating. Agents (software components) remain fully dormant until an
event triggers their activation. Once their work completes, they release all
resources and return to dormancy.

The quantum simulation engine must adhere to this model:

1. **Zero idle footprint**: When no simulation is running, the engine consumes zero
   CPU cycles and zero heap memory beyond its compiled code and static data.
2. **Rapid activation**: The engine must be ready to execute a simulation within
   microseconds of receiving a request.
3. **Prompt resource release**: Upon simulation completion (or failure), all
   allocated memory is immediately freed.
4. **Predictable memory**: Callers must be able to determine exact memory
   requirements before committing to a simulation.

### Memory Scale

The state vector for n qubits requires 2^n complex amplitudes, each consuming 16
bytes (two f64 values):

| Qubits | Amplitudes | Memory | Notes |
|--------|-----------|--------|-------|
| 10 | 1,024 | 16 KiB | Trivial |
| 15 | 32,768 | 512 KiB | Small |
| 20 | 1,048,576 | 16 MiB | Moderate |
| 25 | 33,554,432 | 512 MiB | Large |
| 28 | 268,435,456 | 4 GiB | Needs dedicated memory |
| 30 | 1,073,741,824 | 16 GiB | Workstation-class |
| 32 | 4,294,967,296 | 64 GiB | Server-class |
| 35 | 34,359,738,368 | 512 GiB | HPC |
| 40 | 1,099,511,627,776 | 16 TiB | Infeasible (state vector) |

Each additional qubit doubles memory. This exponential scaling makes memory the
primary resource constraint and the most important resource to manage.

### Edge and Embedded Constraints

On edge devices (embedded ruVector nodes, IoT gateways, mobile processors), memory
is severely limited:

| Platform | Typical RAM | Max qubits (state vector) |
|----------|------------|--------------------------|
| Cognitum tile (single) | 256 MiB | 23 |
| Cognitum tile cluster (4) | 1 GiB | 25 |
| Raspberry Pi 4 | 8 GiB | 28 |
| Mobile device | 4-6 GiB | 27-28 (with other apps) |
| Laptop | 16-64 GiB | 29-31 |
| Server | 256-512 GiB | 33-34 |

### WASM Memory Model

WebAssembly uses a linear memory that can grow but cannot shrink. Once a large
simulation allocates pages, those pages remain mapped until the WASM instance is
destroyed. This is a fundamental platform limitation that must be documented and
accounted for.

## Decision

### 1. Zero-Idle Footprint Architecture

The quantum engine is implemented as a pure library with no runtime overhead:

```rust
// The engine is a collection of functions and types.
// No background threads, no event loops, no persistent state.
// When not called, it consumes exactly zero CPU and zero heap.

pub struct QuantumEngine;  // Zero-sized type; purely a namespace

impl QuantumEngine {
    /// Execute a simulation. All resources are allocated on entry
    /// and freed on exit (or on error).
    pub fn execute(
        circuit: &QuantumCircuit,
        shots: usize,
        config: &SimulationConfig,
    ) -> Result<SimulationResult, SimulationError> {
        // 1. Estimate and validate memory
        let required = Self::estimate_memory(circuit.num_qubits());
        Self::validate_memory_available(required)?;

        // 2. Allocate state vector (the big allocation)
        let mut state = Self::allocate_state(circuit.num_qubits())?;

        // 3. Execute gates (all computation happens here)
        Self::apply_gates(circuit, &mut state, config)?;

        // 4. Measure (if requested)
        let measurements = Self::measure(&state, shots)?;

        // 5. Build result (copies out what we need)
        let result = SimulationResult::from_state_and_measurements(
            &state, measurements, circuit,
        );

        // 6. state is dropped here -- Vec<Complex<f64>> deallocated
        //    No cleanup needed. No finalizers. Just drop.

        Ok(result)
    }
    // state goes out of scope and is deallocated by Rust's ownership system
}
```

Key properties:
- No `new()` or `init()` methods that create persistent state.
- No `Drop` impl with complex cleanup logic.
- No `Arc`, `Mutex`, or shared state between calls.
- Each call is fully independent and self-contained.

### 2. On-Demand Allocation Strategy

State vectors are allocated at simulation start and freed at simulation end:

```rust
fn allocate_state(n_qubits: u32) -> Result<StateVector, SimulationError> {
    let num_amplitudes = 1_usize.checked_shl(n_qubits)
        .ok_or(SimulationError::QubitLimitExceeded {
            requested: n_qubits,
            maximum: (usize::BITS - 1) as u32,
            estimated_memory_bytes: u64::MAX,
            available_memory_bytes: estimate_available_memory() as u64,
        })?;

    let required_bytes = num_amplitudes
        .checked_mul(std::mem::size_of::<Complex<f64>>())
        .ok_or(SimulationError::MemoryAllocationFailed {
            requested_bytes: u64::MAX,
            qubit_count: n_qubits,
            suggestion: "Qubit count exceeds addressable memory",
        })?;

    // Attempt allocation. Rust's global allocator will return an error
    // (with #[global_allocator] configured) or the OS will OOM-kill us.
    // We use try_reserve to handle this gracefully.
    let mut amplitudes = Vec::new();
    amplitudes.try_reserve_exact(num_amplitudes)
        .map_err(|_| SimulationError::MemoryAllocationFailed {
            requested_bytes: required_bytes as u64,
            qubit_count: n_qubits,
            suggestion: "Reduce qubit count or use tensor-network backend",
        })?;

    // Initialize to |00...0> state
    amplitudes.resize(num_amplitudes, Complex::new(0.0, 0.0));
    amplitudes[0] = Complex::new(1.0, 0.0);

    Ok(StateVector { amplitudes, n_qubits })
}
```

The allocation sequence:

```
  IDLE (zero memory)
    |
    v
  estimate_memory(n) --> returns bytes needed
    |
    v
  validate_memory_available(bytes) --> checks against OS/platform limits
    |                                   returns Err if insufficient
    v
  Vec::try_reserve_exact(2^n) --> attempts allocation
    |                              returns Err on failure (no panic)
    v
  ALLOCATED (2^n * 16 bytes on heap)
    |
    v
  [... simulation runs ...]
    |
    v
  Vec::drop() --> automatic deallocation
    |
    v
  IDLE (zero memory)
```

### 3. Memory Estimation API

Callers can query exact memory requirements before committing:

```rust
/// Returns the number of bytes required to simulate n_qubits.
/// This accounts for the state vector plus working memory for
/// gate application (temporary buffers, measurement arrays, etc.).
///
/// # Returns
/// - `Ok(bytes)` if the qubit count is representable
/// - `Err(...)` if 2^n_qubits overflows usize
pub fn estimate_memory(n_qubits: u32) -> Result<MemoryEstimate, SimulationError> {
    let num_amplitudes = 1_usize.checked_shl(n_qubits)
        .ok_or(SimulationError::QubitLimitExceeded {
            requested: n_qubits,
            maximum: (usize::BITS - 1) as u32,
            estimated_memory_bytes: u64::MAX,
            available_memory_bytes: 0,
        })?;

    let state_vector_bytes = num_amplitudes * std::mem::size_of::<Complex<f64>>();

    // Working memory: temporary buffer for gate application (1 amplitude slice)
    // Plus measurement result storage
    let working_bytes = num_amplitudes * std::mem::size_of::<Complex<f64>>() / 4;

    // Thread-local scratch space (per Rayon thread)
    let thread_count = rayon::current_num_threads();
    let scratch_per_thread = 64 * 1024; // 64 KiB per thread for local buffers
    let thread_scratch = thread_count * scratch_per_thread;

    Ok(MemoryEstimate {
        state_vector_bytes: state_vector_bytes as u64,
        working_bytes: working_bytes as u64,
        thread_scratch_bytes: thread_scratch as u64,
        total_bytes: (state_vector_bytes + working_bytes + thread_scratch) as u64,
        num_amplitudes: num_amplitudes as u64,
    })
}

#[derive(Debug, Clone)]
pub struct MemoryEstimate {
    /// Bytes for the state vector (dominant cost).
    pub state_vector_bytes: u64,
    /// Bytes for gate-application working memory.
    pub working_bytes: u64,
    /// Bytes for thread-local scratch space.
    pub thread_scratch_bytes: u64,
    /// Total estimated bytes.
    pub total_bytes: u64,
    /// Number of complex amplitudes.
    pub num_amplitudes: u64,
}

impl MemoryEstimate {
    /// Returns true if the estimate fits within the given byte budget.
    pub fn fits_in(&self, available_bytes: u64) -> bool {
        self.total_bytes <= available_bytes
    }

    /// Suggest the maximum qubits for a given memory budget.
    pub fn max_qubits_for(available_bytes: u64) -> u32 {
        // Each qubit doubles memory; find largest n where 20 * 2^n <= available
        // Factor of 20 accounts for 16-byte amplitudes + 25% working memory
        let effective = available_bytes / 20;
        if effective == 0 { return 0; }
        (effective.ilog2()) as u32
    }
}
```

### 4. Allocation Failure Handling

The engine never panics on allocation failure. All paths return structured errors:

```rust
// Pattern: every allocation is fallible and returns a descriptive error.

// State vector allocation failure:
SimulationError::MemoryAllocationFailed {
    requested_bytes: 17_179_869_184,  // 16 GiB
    qubit_count: 30,
    suggestion: "Reduce qubit count by 2 (to 28, ~4 GiB) or enable tensor-network backend",
}

// Integer overflow (qubit count too large):
SimulationError::QubitLimitExceeded {
    requested: 64,
    maximum: 33,  // based on available memory
    estimated_memory_bytes: u64::MAX,
    available_memory_bytes: 68_719_476_736,  // 64 GiB
}
```

Decision tree on allocation failure:

```
  Memory allocation failed
    |
    +-- Is tensor-network feature enabled?
    |     |
    |     +-- YES: Suggest tensor-network backend
    |     |         (may work if circuit has low treewidth)
    |     |
    |     +-- NO: Suggest reducing qubit count
    |             Calculate: max_qubits = floor(log2(available / 20))
    |             Suggest: "Reduce to {max_qubits} qubits ({memory} bytes)"
    |
    +-- Is the request wildly over budget (>100x)?
    |     |
    |     +-- YES: "Circuit requires {X} GiB but only {Y} MiB available"
    |     |
    |     +-- NO: "Circuit requires {X} GiB, {Y} GiB available.
    |              Reducing by {delta} qubits would fit."
    |
    +-- Return SimulationError (no panic, no abort)
```

### 5. CPU Yielding for Long Simulations

For simulations estimated to exceed 100ms, the engine can optionally yield between
gate batches to allow the OS scheduler to manage power states:

```rust
pub struct YieldConfig {
    /// Enable cooperative yielding between gate batches.
    /// Default: false (maximum throughput).
    pub enabled: bool,

    /// Number of gates to apply before yielding.
    /// Default: 1000.
    pub gates_per_slice: usize,

    /// Yield mechanism.
    /// Default: ThreadYield (std::thread::yield_now).
    pub yield_strategy: YieldStrategy,
}

pub enum YieldStrategy {
    /// Call std::thread::yield_now() between slices.
    ThreadYield,
    /// Sleep for specified duration between slices.
    Sleep(Duration),
    /// Call a user-provided callback between slices.
    Callback(Box<dyn Fn(SliceProgress) + Send>),
}

pub struct SliceProgress {
    pub gates_completed: u64,
    pub gates_remaining: u64,
    pub elapsed: Duration,
    pub estimated_remaining: Duration,
}

// Usage in gate application loop:
fn apply_gates_with_yield(
    circuit: &QuantumCircuit,
    state: &mut StateVector,
    yield_config: &YieldConfig,
) -> Result<(), SimulationError> {
    let gates = circuit.gates();

    for (i, gate) in gates.iter().enumerate() {
        apply_single_gate(gate, state)?;

        if yield_config.enabled && (i + 1) % yield_config.gates_per_slice == 0 {
            match &yield_config.yield_strategy {
                YieldStrategy::ThreadYield => std::thread::yield_now(),
                YieldStrategy::Sleep(d) => std::thread::sleep(*d),
                YieldStrategy::Callback(cb) => cb(SliceProgress {
                    gates_completed: (i + 1) as u64,
                    gates_remaining: (gates.len() - i - 1) as u64,
                    elapsed: start.elapsed(),
                    estimated_remaining: estimate_remaining(i, gates.len(), start),
                }),
            }
        }
    }

    Ok(())
}
```

Yield is **disabled by default** to maximize throughput. It is primarily intended
for:
- Edge devices where power management is critical.
- Interactive applications where UI responsiveness matters.
- Long-running simulations (>1 second) where progress reporting is needed.

### 6. Thread Management

The quantum engine does not create or manage its own threads:

```
  +-----------------------------------------------+
  |              Global Rayon Thread Pool          |
  |  (shared by all ruVector subsystems)          |
  |                                                |
  |  [Thread 0] [Thread 1] ... [Thread N-1]       |
  |     ^           ^              ^               |
  |     |           |              |               |
  |  +--+---+   +--+---+      +---+--+            |
  |  | ruQu |   | ruQu |      | idle |            |
  |  | gate  |   | gate |      |      |            |
  |  | apply |   | apply|      |      |            |
  |  +-------+   +------+      +------+            |
  |                                                |
  |  During simulation: threads work on gates      |
  |  After simulation: threads return to pool      |
  |  Pool idle: OS can power-gate cores            |
  +-----------------------------------------------+
```

Key properties:
- Rayon's global thread pool is initialized once by `ruvector-core` at startup.
- The quantum engine calls `rayon::par_iter()` and related APIs, borrowing threads
  temporarily.
- When simulation completes, all threads are returned to the global pool.
- If no ruVector work is pending, Rayon threads park (blocking on a condvar),
  consuming zero CPU. The OS can then power-gate the underlying cores.

### 7. WASM Memory Considerations

WebAssembly linear memory has a specific behavior that affects resource management:

```
  WASM Memory Layout
  +------------------+------------------+
  |  Initial pages   |  Grown pages     |
  |  (compiled size) |  (runtime alloc) |
  +------------------+------------------+
  0                  initial_size       current_size

  Growth: memory.grow(delta_pages) -> adds pages to the end
  Shrink: NOT SUPPORTED in WASM spec

  After 25-qubit simulation:
  +------------------+----------------------------------+
  |  Initial (1 MiB) |  Grown for state vec (512 MiB)  |  <- HIGH WATER MARK
  +------------------+----------------------------------+

  After simulation completes:
  +------------------+----------------------------------+
  |  Initial (1 MiB) |  FREED internally but pages      |
  |                   |  still mapped (512 MiB virtual)  |
  +------------------+----------------------------------+
  The Rust allocator returns memory to its free list,
  but WASM pages are not returned to the host.
```

**Implications and mitigations**:

1. **Document the behavior**: Users must understand that WASM memory is a high-water
   mark. A 25-qubit simulation permanently increases the WASM instance's memory
   footprint to ~512 MiB.

2. **Instance recycling**: For applications that run multiple simulations, create a
   new WASM instance periodically to reset the memory high-water mark.

3. **Memory budget enforcement**: The WASM host can set `WebAssembly.Memory` with a
   `maximum` parameter to cap growth:

```javascript
const memory = new WebAssembly.Memory({
    initial: 16,      // 1 MiB
    maximum: 8192,     // 512 MiB cap
});
```

4. **Pre-check in WASM**: The engine's `estimate_memory()` function works in WASM
   and should be called before simulation to verify the allocation will succeed.

### 8. Cognitum Tile Integration

On Cognitum's tile-based architecture, the quantum engine maps to tiles as follows:

```
  Cognitum Processor (256 tiles)
  +--------+--------+--------+--------+
  | Tile 0 | Tile 1 | Tile 2 | Tile 3 |  <- Assigned to quantum sim
  | ACTIVE | ACTIVE | ACTIVE | ACTIVE |
  +--------+--------+--------+--------+
  | Tile 4 | Tile 5 | Tile 6 | Tile 7 |  <- Other ruVector work (or sleeping)
  | sleep  | vecDB  | sleep  | sleep  |
  +--------+--------+--------+--------+
  |  ...   |  ...   |  ...   |  ...   |
  | sleep  | sleep  | sleep  | sleep  |  <- Power gated (zero consumption)
  +--------+--------+--------+--------+
```

**Power state diagram for a quantum simulation lifecycle**:

```
  State: ALL_TILES_IDLE
    |
    | Simulation request arrives
    v
  State: ALLOCATING
    Action: Wake tiles 0-3 (or however many are needed)
    Action: Allocate state vector across tile-local memory
    Power: Tiles 0-3 ACTIVE, rest SLEEP
    |
    v
  State: SIMULATING
    Action: Apply gates in parallel across active tiles
    Power: Tiles 0-3 at full clock rate
    Duration: microseconds to seconds depending on circuit
    |
    v
  State: MEASURING
    Action: Sample measurement outcomes
    Power: Tile 0 only (measurement is sequential)
    |
    v
  State: DEALLOCATING
    Action: Free state vector
    Action: Return tiles to idle pool
    |
    v
  State: ALL_TILES_IDLE
    Power: Tiles 0-3 back to SLEEP
    Memory: Zero heap allocation
```

**Tile assignment policy**:
- Small simulations (n <= 20): 1 tile sufficient.
- Medium simulations (20 < n <= 25): 2-4 tiles for parallel gate application.
- Large simulations (25 < n <= 30): All available tiles.
- The tile scheduler (part of Cognitum runtime) handles assignment. The quantum
  engine simply uses Rayon parallelism; the runtime maps Rayon threads to tiles.

### 9. Memory Budget Table

Quick reference for capacity planning:

| Qubits | State Vector | Working Memory | Total | Platform Fit |
|--------|-------------|---------------|-------|-------------|
| 10 | 16 KiB | 4 KiB | 20 KiB | Any |
| 12 | 64 KiB | 16 KiB | 80 KiB | Any |
| 14 | 256 KiB | 64 KiB | 320 KiB | Any |
| 16 | 1 MiB | 256 KiB | 1.3 MiB | Any |
| 18 | 4 MiB | 1 MiB | 5 MiB | Any |
| 20 | 16 MiB | 4 MiB | 20 MiB | Any |
| 22 | 64 MiB | 16 MiB | 80 MiB | Cognitum single tile |
| 24 | 256 MiB | 64 MiB | 320 MiB | Cognitum 2+ tiles |
| 26 | 1 GiB | 256 MiB | 1.3 GiB | Cognitum cluster |
| 28 | 4 GiB | 1 GiB | 5 GiB | Laptop / RPi 8GB |
| 30 | 16 GiB | 4 GiB | 20 GiB | Workstation |
| 32 | 64 GiB | 16 GiB | 80 GiB | Server |
| 34 | 256 GiB | 64 GiB | 320 GiB | Large server |

### 10. Allocation and Deallocation Sequence Diagram

```
  Caller                Engine                  OS/Allocator
    |                     |                         |
    |  execute(circuit)   |                         |
    |-------------------->|                         |
    |                     |                         |
    |                     |  estimate_memory(n)     |
    |                     |  validate_available()   |
    |                     |                         |
    |                     |  try_reserve_exact(2^n) |
    |                     |------------------------>|
    |                     |                         |
    |                     |     Ok(ptr) or Err      |
    |                     |<------------------------|
    |                     |                         |
    |                     |  [if Err: return        |
    |                     |   SimulationError]      |
    |                     |                         |
    |                     |  initialize |00...0>    |
    |                     |  apply gates            |
    |                     |  measure                |
    |                     |                         |
    |                     |  build result           |
    |                     |  (copies measurements,  |
    |                     |   expectation values)   |
    |                     |                         |
    |                     |  drop(state_vector)     |
    |                     |------------------------>|
    |                     |                         |  free(ptr, 2^n * 16)
    |                     |                         |
    |  Ok(result)         |                         |
    |<--------------------|                         |
    |                     |                         |
    |  [Engine holds ZERO |                         |
    |   heap memory now]  |                         |
```

## Consequences

### Positive

1. **True zero-idle cost**: No background resource consumption. Perfectly aligned
   with Cognitum's event-driven architecture and power gating.
2. **Predictable memory**: `estimate_memory()` gives exact requirements before
   committing, preventing OOM surprises.
3. **Graceful degradation**: Allocation failures return structured errors with
   actionable suggestions, never panics.
4. **Platform portable**: The same allocation strategy works on native (Linux, macOS,
   Windows), WASM, and embedded (Cognitum tiles).
5. **No resource leaks**: Rust's ownership system guarantees deallocation on all
   exit paths (success, error, panic).

### Negative

1. **No state caching**: Each simulation allocates and deallocates independently.
   Repeated simulations on the same qubit count pay allocation cost each time.
   Mitigation: allocation is O(2^n) but fast compared to O(G * 2^n) simulation.
2. **WASM memory high-water mark**: Cannot reclaim WASM linear memory pages.
   Documented as a platform limitation with instance-recycling workaround.
3. **No memory pooling**: Could theoretically amortize allocation across simulations,
   but this conflicts with the zero-idle-footprint requirement.
4. **Yield overhead**: When enabled, cooperative yielding adds per-slice overhead.
   Mitigated by making it opt-in and configurable.

### Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| OOM despite estimate_memory check | Low | Crash | Check returns conservative estimate including working memory |
| WASM instance runs out of address space | Medium | Failure | Set `WebAssembly.Memory` maximum; document limitation |
| Allocation latency spike (OS page faults) | Medium | Slow start | Consider `madvise` / `mlock` hints for large allocations |
| Rayon thread pool contention | Medium | Degraded perf | Quantum engine yields between slices; Rayon work-stealing handles contention |

## References

- Cognitum Architecture Specification: event-driven tile-based computing
- Rust `Vec::try_reserve_exact`: https://doc.rust-lang.org/std/vec/struct.Vec.html#method.try_reserve_exact
- WebAssembly Memory: https://webassembly.github.io/spec/core/syntax/modules.html#memories
- Rayon thread pool: https://docs.rs/rayon
- ADR-QE-001: Core Engine Architecture (zero-overhead design principle)
- ADR-QE-005: WASM Compilation Target (WASM constraints)
- ADR-QE-009: Tensor Network Evaluation Mode (alternative for large circuits)
- ADR-QE-010: Observability & Monitoring (memory metrics reporting)
