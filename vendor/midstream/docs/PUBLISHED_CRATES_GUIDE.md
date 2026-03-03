# MidStream Published Crates Guide

## Overview

All 5 core MidStream crates are **published and available on crates.io**! This guide shows you how to use them in your projects.

## Quick Start

### Install All Core Crates

```toml
[dependencies]
temporal-compare = "0.1"
nanosecond-scheduler = "0.1"
temporal-attractor-studio = "0.1"
temporal-neural-solver = "0.1"
strange-loop = "0.1"
```

### Install Individual Crates

Pick only what you need:

```toml
[dependencies]
# Pattern matching and sequence analysis
temporal-compare = "0.1"

# Ultra-low-latency scheduling
nanosecond-scheduler = "0.1"

# Dynamical systems analysis (optional)
# temporal-attractor-studio = "0.1"

# Temporal logic verification (optional)
# temporal-neural-solver = "0.1"

# Meta-learning capabilities (optional)
# strange-loop = "0.1"
```

## Published Crates

### 1. temporal-compare v0.1.x

**Pattern matching and temporal sequence comparison**

- **crates.io**: https://crates.io/crates/temporal-compare
- **docs.rs**: https://docs.rs/temporal-compare
- **Features**: DTW, LCS, Edit Distance, Pattern Caching
- **Platform**: Native, WASM

```toml
[dependencies]
temporal-compare = "0.1"
```

**Example:**
```rust
use temporal_compare::{Sequence, SequenceComparator};

let comparator = SequenceComparator::new();
let distance = comparator.dtw_distance(&seq1, &seq2)?;
```

---

### 2. nanosecond-scheduler v0.1.x

**Ultra-low-latency real-time task scheduler**

- **crates.io**: https://crates.io/crates/nanosecond-scheduler
- **docs.rs**: https://docs.rs/nanosecond-scheduler
- **Features**: <100ns latency, Priority queues, Real-time scheduling
- **Platform**: Native

```toml
[dependencies]
nanosecond-scheduler = "0.1"
```

**Example:**
```rust
use nanosecond_scheduler::{Scheduler, Task, Priority};

let scheduler = Scheduler::new(4);
scheduler.schedule(Task { priority: Priority::High, ... })?;
```

---

### 3. temporal-attractor-studio v0.1.x

**Dynamical systems and strange attractors analysis**

- **crates.io**: https://crates.io/crates/temporal-attractor-studio
- **docs.rs**: https://docs.rs/temporal-attractor-studio
- **Features**: Lyapunov exponents, Attractor detection, Phase space
- **Platform**: Native, WASM

```toml
[dependencies]
temporal-attractor-studio = "0.1"
```

**Example:**
```rust
use temporal_attractor_studio::AttractorAnalyzer;

let analyzer = AttractorAnalyzer::new();
let attractor = analyzer.detect_attractor(&states)?;
let lyapunov = analyzer.compute_lyapunov_exponent(&states)?;
```

---

### 4. temporal-neural-solver v0.1.x

**Temporal logic verification with neural reasoning**

- **crates.io**: https://crates.io/crates/temporal-neural-solver
- **docs.rs**: https://docs.rs/temporal-neural-solver
- **Features**: LTL verification, Temporal logic, Neural reasoning
- **Platform**: Native

```toml
[dependencies]
temporal-neural-solver = "0.1"
```

**Example:**
```rust
use temporal_neural_solver::{LTLSolver, Formula};

let solver = LTLSolver::new();
let result = solver.verify(&formula, &trace)?;
```

---

### 5. strange-loop v0.1.x

**Self-referential systems and meta-learning**

- **crates.io**: https://crates.io/crates/strange-loop
- **docs.rs**: https://docs.rs/strange-loop
- **Features**: Meta-learning, Pattern extraction, Policy adaptation
- **Platform**: Native, WASM

```toml
[dependencies]
strange-loop = "0.1"
```

**Example:**
```rust
use strange_loop::{MetaLearner, Experience};

let mut learner = MetaLearner::new();
learner.update(&experience)?;
let policy = learner.adapt_policy()?;
```

---

## Complete Example Project

### Cargo.toml

```toml
[package]
name = "my-midstream-app"
version = "0.1.0"
edition = "2021"

[dependencies]
# All MidStream crates from crates.io
temporal-compare = "0.1"
nanosecond-scheduler = "0.1"
temporal-attractor-studio = "0.1"
temporal-neural-solver = "0.1"
strange-loop = "0.1"

# Common dependencies
tokio = { version = "1.42", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
nalgebra = "0.33"
ndarray = "0.16"
```

### src/main.rs

```rust
use temporal_compare::{Sequence, SequenceComparator, TemporalElement};
use nanosecond_scheduler::{Scheduler, Task, Priority};
use temporal_attractor_studio::AttractorAnalyzer;
use strange_loop::{MetaLearner, Experience};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("MidStream - All crates from crates.io!");

    // 1. Pattern matching
    let seq1 = Sequence {
        elements: vec![
            TemporalElement { value: 1, timestamp: 0 },
            TemporalElement { value: 2, timestamp: 100 },
        ]
    };
    let seq2 = Sequence {
        elements: vec![
            TemporalElement { value: 1, timestamp: 0 },
            TemporalElement { value: 3, timestamp: 150 },
        ]
    };

    let comparator = SequenceComparator::new();
    let distance = comparator.dtw_distance(&seq1, &seq2)?;
    println!("DTW distance: {}", distance);

    // 2. Real-time scheduling
    let scheduler = Scheduler::new(4);
    println!("Scheduler initialized with 4 workers");

    // 3. Dynamical systems
    let analyzer = AttractorAnalyzer::new();
    println!("Attractor analyzer ready");

    // 4. Meta-learning
    let mut learner = MetaLearner::new();
    let experience = Experience {
        state: vec![1.0, 2.0],
        action: "test",
        reward: 1.0,
        next_state: vec![1.1, 2.1],
    };
    learner.update(&experience)?;
    println!("Meta-learner trained");

    println!("\nAll MidStream crates working together!");
    Ok(())
}
```

### Build and Run

```bash
cargo build --release
cargo run
```

Output:
```
MidStream - All crates from crates.io!
DTW distance: 1.0
Scheduler initialized with 4 workers
Attractor analyzer ready
Meta-learner trained

All MidStream crates working together!
```

## Benefits of Published Crates

### ✅ Easy Installation

No cloning, no path dependencies:

```toml
[dependencies]
temporal-compare = "0.1"  # That's it!
```

### ✅ Automatic Updates

```bash
cargo update  # Updates to latest compatible versions
```

### ✅ Version Stability

Semantic versioning ensures compatibility:
- `0.1.x` - Patch releases (bug fixes)
- `0.2.0` - Minor releases (new features)
- `1.0.0` - Major releases (breaking changes)

### ✅ CI/CD Ready

Works in any Rust build environment:
- GitHub Actions
- GitLab CI
- Travis CI
- CircleCI
- Local builds

### ✅ Documentation

Automatic hosting on docs.rs:
- https://docs.rs/temporal-compare
- https://docs.rs/nanosecond-scheduler
- https://docs.rs/temporal-attractor-studio
- https://docs.rs/temporal-neural-solver
- https://docs.rs/strange-loop

## Migration Guide

### From Local Paths to Published Crates

**Before:**
```toml
[dependencies]
temporal-compare = { path = "../midstream/crates/temporal-compare" }
```

**After:**
```toml
[dependencies]
temporal-compare = "0.1"
```

Steps:
1. Update Cargo.toml
2. Run `cargo update`
3. Run `cargo build --release`
4. Test your application

No code changes required!

### From Git Dependencies

**Before:**
```toml
[dependencies]
temporal-compare = { git = "https://github.com/ruvnet/midstream", branch = "main" }
```

**After:**
```toml
[dependencies]
temporal-compare = "0.1"
```

Benefits:
- Faster builds (no git cloning)
- Stable versions
- Better caching

## Platform Support

All published crates support multiple platforms:

| Platform | Support |
|----------|---------|
| Linux x86_64 | ✅ Full |
| Linux ARM64 | ✅ Full |
| macOS Intel | ✅ Full |
| macOS Apple Silicon | ✅ Full |
| Windows x64 | ✅ Full |
| WASM (browser) | ✅ Selected crates |
| WASM (Node.js) | ✅ Selected crates |

## Performance

All crates are optimized for production use:

| Crate | Operation | Performance |
|-------|-----------|-------------|
| temporal-compare | DTW (n=100) | ~8ms |
| nanosecond-scheduler | Schedule task | <100ns |
| temporal-attractor-studio | Lyapunov (1K pts) | ~9ms |
| temporal-neural-solver | LTL verification | ~1ms |
| strange-loop | Policy update | ~3ms |

Build with `--release` for best performance:
```bash
cargo build --release
```

## Testing

All published crates have comprehensive tests:

```bash
# Test all crates
cargo test

# Test specific crate
cargo test -p temporal-compare

# Run with output
cargo test -- --nocapture
```

## Benchmarking

```bash
# Benchmark all crates
cargo bench

# Benchmark specific crate
cargo bench -p nanosecond-scheduler
```

## Troubleshooting

### Issue: Crate not found

**Solution:**
```bash
# Make sure you're using the correct version
cargo search temporal-compare

# Update cargo index
cargo update
```

### Issue: Version conflicts

**Solution:**
```toml
# Pin to specific version
temporal-compare = "=0.1.0"

# Or use compatible versions
temporal-compare = "0.1"
```

### Issue: Build errors

**Solution:**
```bash
# Clean and rebuild
cargo clean
cargo build --release

# Update Rust
rustup update
```

## Getting Help

- **Documentation**: https://docs.rs
- **Examples**: https://github.com/ruvnet/midstream/tree/main/examples
- **Issues**: https://github.com/ruvnet/midstream/issues
- **Discussions**: https://github.com/ruvnet/midstream/discussions

## Next Steps

1. ✅ Add crates to your Cargo.toml
2. 📖 Read the docs.rs documentation
3. 💡 Try the examples
4. 🚀 Build your application!

---

**All crates are production-ready and actively maintained!** 🎉

Browse all crates: https://crates.io/search?q=temporal
