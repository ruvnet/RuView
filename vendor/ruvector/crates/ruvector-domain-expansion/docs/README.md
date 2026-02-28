# ruvector-domain-expansion

Cross-domain transfer learning engine for general problem-solving capability.

## Core Insight

> True IQ growth appears when a kernel trained on Domain 1 improves Domain 2 faster than Domain 2 alone. That is generalization.

If cost curves compress faster in each new domain, you are increasing general problem-solving capability.

## Architecture

### Two-Layer Learning

```
Policy Learning Layer (Meta Thompson Sampling)
  |
  |  TransferPrior: compact Beta posteriors per bucket/arm
  |  NOT raw trajectories. Ship priors, not memories.
  |
  v
Operator Layer (Domain Kernels)
  |
  |  Rust Synthesis | Planning | Tool Orchestration
  |  Generate tasks, evaluate solutions, produce embeddings
  |
  v
Shared Embedding Space (64-dim)
  Cross-domain similarity via cosine distance
```

### Domains

| Domain | Description | Task Types |
|--------|-------------|------------|
| **Rust Program Synthesis** | Synthesize Rust functions from specs | Transform, DataStructure, Algorithm, TypeLevel, Concurrency |
| **Structured Planning** | Multi-step plans with constraints | ResourceAllocation, DependencyScheduling, StateSpaceSearch, ConstraintSatisfaction |
| **Tool Orchestration** | Coordinate multiple tools/agents | PipelineConstruction, ErrorRecovery, ParallelCoordination, ResourceNegotiation |

### Transfer Protocol

1. Train on Domain 1, extract `TransferPrior` (posterior summaries)
2. Initialize Domain 2 with dampened priors from Domain 1
3. Measure acceleration: cycles to convergence with vs without transfer
4. **Generalization rule**: A delta is promotable only if it improves Domain 2 without regressing Domain 1

### Population-Based Policy Search

Run a population of `PolicyKernel` variants in parallel. Each variant tunes knobs:
- Skip mode policy
- Prepass mode
- Speculation trigger thresholds
- Budget allocation

Selection: keep top performers on holdouts, mutate knobs, repeat. Only merge deltas that pass replay-verify.

### Speculative Dual-Path

When posterior variance is high (top two arms within delta), run both strategies with bounded budgets. Pick the first correct, log the loser as a counterexample.

## Usage

### Rust

```rust
use ruvector_domain_expansion::{
    DomainExpansionEngine, DomainId, ArmId, ContextBucket,
};

// Create engine with 3 core domains
let mut engine = DomainExpansionEngine::new();

// Generate tasks
let tasks = engine.generate_tasks(
    &DomainId("rust_synthesis".into()),
    10,   // count
    0.5,  // difficulty
);

// Select arm via Thompson Sampling
let bucket = ContextBucket {
    difficulty_tier: "medium".into(),
    category: "algorithm".into(),
};
let arm = engine.select_arm(
    &DomainId("rust_synthesis".into()),
    &bucket,
).unwrap();

// Evaluate and record
let eval = engine.evaluate_and_record(
    &DomainId("rust_synthesis".into()),
    &tasks[0],
    &solution,
    bucket,
    arm,
);

// Transfer learning
engine.initiate_transfer(
    &DomainId("rust_synthesis".into()),
    &DomainId("structured_planning".into()),
);

// Verify generalization
let v = engine.verify_transfer(
    &DomainId("rust_synthesis".into()),
    &DomainId("structured_planning".into()),
    0.85, 0.84,  // source before/after
    0.3, 0.7,    // target before/after
    100, 40,     // baseline/transfer cycles
);
assert!(v.promotable);        // improved target without regressing source
assert!(v.acceleration_factor > 1.0);  // 2.5x faster convergence
```

### WASM (JavaScript)

```javascript
import { WasmDomainExpansionEngine } from 'ruvector-domain-expansion-wasm';

const engine = new WasmDomainExpansionEngine();

// List domains
console.log(engine.domainIds());
// ["rust_synthesis", "structured_planning", "tool_orchestration"]

// Generate tasks
const tasks = engine.generateTasks("rust_synthesis", 10, 0.5);

// Select strategy via Thompson Sampling
const arm = engine.selectArm("rust_synthesis", "medium", "algorithm");

// Check if dual-path speculation needed
if (engine.shouldSpeculate("rust_synthesis", "medium", "algorithm")) {
    // Run both strategies, pick winner
}

// Transfer priors between domains
engine.initiateTransfer("rust_synthesis", "structured_planning");

// Evolve policy kernels
engine.generateHoldouts(10, 0.5);
engine.evaluatePopulation();
engine.evolvePopulation();
console.log(engine.populationStats());

// Acceleration scoreboard
console.log(engine.scoreboardSummary());
```

## Acceptance Test

Domain 2 must converge faster than Domain 1. Measure cycles to reach:
- 95% accuracy
- Target cost per solve
- Target robustness
- Zero policy violations

```rust
use ruvector_domain_expansion::{AccelerationScoreboard, CostCurve, DomainId};

let mut board = AccelerationScoreboard::new();

// Add baseline and transfer curves
board.add_curve(baseline_curve);
board.add_curve(transfer_curve);

// Compute acceleration
let entry = board.compute_acceleration(
    &DomainId("baseline".into()),
    &DomainId("transfer".into()),
).unwrap();

assert!(entry.acceleration > 1.0);  // transfer helped
assert!(entry.generalization_passed);

// Check progressive improvement across multiple domains
assert!(board.progressive_acceleration());
```

## RVF Packaging

Transfer artifacts are designed for RVF segment packaging:

| Segment | Content | Purpose |
|---------|---------|---------|
| `TransferPrior` | Beta posteriors per bucket/arm | Seeds new domain initialization |
| `PolicyKernel` | Knob configuration + fitness history | Best policy for a domain |
| `CostCurve` | Convergence data points | Acceleration measurement |
| `WitnessChain` | Hash of derivation + holdout results | Audit trail |
| `Counterexamples` | Failed solutions per context | Negative signal for future decisions |

## Benchmarks

```bash
cargo bench -p ruvector-domain-expansion
```

Benchmarks cover:
- Task generation (per domain)
- Solution evaluation
- Embedding extraction
- Thompson Sampling arm selection
- Population evolution
- PolicyKnobs mutation
- Cost curve AUC computation
- TransferPrior extraction

## Module Structure

```
src/
  lib.rs                 -- Orchestrator: DomainExpansionEngine
  domain.rs              -- Core Domain trait, Task, Solution, Evaluation, Embedding
  rust_synthesis.rs      -- Rust program synthesis domain
  planning.rs            -- Structured planning tasks domain
  tool_orchestration.rs  -- Tool orchestration problems domain
  transfer.rs            -- Meta Thompson Sampling, TransferPrior, verification
  policy_kernel.rs       -- PolicyKernel, PopulationSearch, PolicyKnobs
  cost_curve.rs          -- CostCurve, AccelerationScoreboard
```

## Tests

49 unit tests covering all modules:

```bash
cargo test -p ruvector-domain-expansion
```

| Module | Tests |
|--------|-------|
| `domain` | 5 tests: types, embedding cosine similarity, evaluation |
| `rust_synthesis` | 5 tests: generation, evaluation, embedding, difficulty |
| `planning` | 5 tests: generation, reference, evaluation, embedding, scaling |
| `tool_orchestration` | 5 tests: generation, reference, evaluation, embedding, errors |
| `transfer` | 6 tests: Beta params, Thompson engine, prior extraction, verification |
| `policy_kernel` | 5 tests: knobs, fitness, evolution, stats, crossover |
| `cost_curve` | 5 tests: convergence, compression, AUC, acceleration, scoreboard |
| `lib` (integration) | 8 tests: engine, tasks, arms, evaluation, embedding, transfer, population |
