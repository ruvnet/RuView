# MidStream Quick Start Guide

Get up and running with MidStream in 5 minutes using published crates from crates.io!

## Prerequisites

- **Rust 1.71+** - Install via rustup
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source ~/.cargo/env
  ```
- **Node.js 18+** - For WASM/TypeScript features (optional)
  ```bash
  # Using nvm (recommended)
  nvm install 18
  nvm use 18
  ```

## Installation Options

### Option 1: Use Published Crates (Recommended) ⭐

All five core MidStream crates are **published on crates.io** and ready to use!

```bash
# Create a new Rust project
cargo new my-midstream-app
cd my-midstream-app
```

Add crates to your `Cargo.toml`:

```toml
[dependencies]
# All published MidStream crates from crates.io
temporal-compare = "0.1"
nanosecond-scheduler = "0.1"
temporal-attractor-studio = "0.1"
temporal-neural-solver = "0.1"
strange-loop = "0.1"

# Additional common dependencies
tokio = { version = "1.42", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

Build and run:

```bash
cargo build --release
cargo run
```

**That's it!** Cargo will automatically download all dependencies from crates.io.

### Option 2: Use Individual Crates

Install only the crates you need:

```toml
[dependencies]
# Pick and choose from published crates
temporal-compare = "0.1"        # For pattern matching and DTW
nanosecond-scheduler = "0.1"    # For real-time scheduling
# temporal-attractor-studio = "0.1"  # Optional: dynamical systems
# temporal-neural-solver = "0.1"     # Optional: LTL verification
# strange-loop = "0.1"               # Optional: meta-learning
```

### Option 3: Use the WASM Package

```bash
# Install from npm
npm install midstream-wasm

# Or build from source
cd npm-wasm
npm install
npm run dev
# Open http://localhost:8080
```

### Option 4: Build from Source (Development)

For development or latest features:

```bash
# Clone the repository
git clone https://github.com/ruvnet/midstream.git
cd midstream

# Build all crates
cargo build --workspace --release

# Run tests
cargo test --workspace

# Run benchmarks
cargo bench --workspace
```

## Quick Examples

### 1. Temporal Pattern Matching

Create `src/main.rs`:

```rust
use temporal_compare::{Sequence, SequenceComparator, TemporalElement};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create sequences from crates.io published crate
    let seq1 = Sequence {
        elements: vec![
            TemporalElement { value: 1, timestamp: 0 },
            TemporalElement { value: 2, timestamp: 100 },
            TemporalElement { value: 3, timestamp: 200 },
        ]
    };

    let seq2 = Sequence {
        elements: vec![
            TemporalElement { value: 1, timestamp: 0 },
            TemporalElement { value: 2, timestamp: 150 },
            TemporalElement { value: 4, timestamp: 300 },
        ]
    };

    // Compare using published crate
    let comparator = SequenceComparator::new();
    let distance = comparator.dtw_distance(&seq1, &seq2)?;
    let lcs = comparator.lcs(&seq1, &seq2)?;

    println!("DTW distance: {}", distance);
    println!("LCS length: {}", lcs.len());

    Ok(())
}
```

Run it:

```bash
cargo run --release
```

### 2. Real-Time Scheduling

Add to `Cargo.toml`:

```toml
[dependencies]
nanosecond-scheduler = "0.1"  # From crates.io
tokio = { version = "1.42", features = ["full"] }
```

Create `src/main.rs`:

```rust
use nanosecond_scheduler::{Scheduler, Task, Priority};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use published nanosecond-scheduler from crates.io
    let scheduler = Scheduler::new(4); // 4 worker threads

    // Schedule high-priority task
    scheduler.schedule(Task {
        priority: Priority::High,
        deadline: Duration::from_millis(10),
        work: Box::new(|| {
            println!("High-priority task executing!");
        }),
    })?;

    scheduler.run().await?;

    Ok(())
}
```

### 3. Dynamical Systems Analysis

```toml
[dependencies]
temporal-attractor-studio = "0.1"  # From crates.io
nalgebra = "0.33"
```

```rust
use temporal_attractor_studio::{AttractorAnalyzer, SystemState};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let analyzer = AttractorAnalyzer::new();

    // Analyze time series data
    let states: Vec<SystemState> = vec![
        SystemState::new(vec![1.0, 2.0]),
        SystemState::new(vec![1.1, 2.1]),
        SystemState::new(vec![1.0, 2.0]),
    ];

    let attractor = analyzer.detect_attractor(&states)?;
    let lyapunov = analyzer.compute_lyapunov_exponent(&states)?;

    println!("Attractor type: {:?}", attractor);
    println!("Lyapunov exponent: {}", lyapunov);

    Ok(())
}
```

### 4. Meta-Learning with Strange Loop

```toml
[dependencies]
strange-loop = "0.1"  # From crates.io
```

```rust
use strange_loop::{MetaLearner, Experience};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut learner = MetaLearner::new();

    // Learn from experience
    let experience = Experience {
        state: vec![1.0, 2.0, 3.0],
        action: "move_forward",
        reward: 1.5,
        next_state: vec![1.1, 2.1, 3.1],
    };

    learner.update(&experience)?;

    // Adapt policy based on learned patterns
    let new_policy = learner.adapt_policy()?;
    println!("Policy adapted successfully!");

    Ok(())
}
```

### 5. Browser/WASM Usage

All published crates support WASM compilation:

```html
<!DOCTYPE html>
<html>
<head>
    <script type="module">
        import init, { TemporalCompare } from './pkg/midstream_wasm.js';

        async function run() {
            await init();

            // Use published crate in browser
            const compare = new TemporalCompare();
            const distance = compare.dtw([1, 2, 3], [1, 2, 4]);

            console.log('DTW distance:', distance);
        }

        run();
    </script>
</head>
<body>
    <h1>MidStream WASM Demo</h1>
    <p>Using published crates from crates.io in the browser!</p>
</body>
</html>
```

## Performance Expectations

Published crates deliver excellent performance:

| Operation | Native | WASM | Status |
|-----------|--------|------|--------|
| DTW (n=100) | ~8ms | ~16ms | ✅ Production |
| Scheduling | ~85ns | N/A | ✅ Production |
| Pattern match | ~4ms | ~12ms | ✅ Production |
| Lyapunov calc | ~9ms | ~18ms | ✅ Production |

## Crate Links

Browse all published crates on crates.io:

- 📦 **[temporal-compare](https://crates.io/crates/temporal-compare)** - Pattern matching and DTW
- 📦 **[nanosecond-scheduler](https://crates.io/crates/nanosecond-scheduler)** - Real-time scheduling
- 📦 **[temporal-attractor-studio](https://crates.io/crates/temporal-attractor-studio)** - Dynamical systems
- 📦 **[temporal-neural-solver](https://crates.io/crates/temporal-neural-solver)** - LTL verification
- 📦 **[strange-loop](https://crates.io/crates/strange-loop)** - Meta-learning

## Documentation

Each published crate has comprehensive documentation on docs.rs:

- 📚 [temporal-compare docs](https://docs.rs/temporal-compare)
- 📚 [nanosecond-scheduler docs](https://docs.rs/nanosecond-scheduler)
- 📚 [temporal-attractor-studio docs](https://docs.rs/temporal-attractor-studio)
- 📚 [temporal-neural-solver docs](https://docs.rs/temporal-neural-solver)
- 📚 [strange-loop docs](https://docs.rs/strange-loop)

## Running Examples

```bash
# Clone repository for examples
git clone https://github.com/ruvnet/midstream.git
cd midstream

# Run examples using published crates
cargo run --example lean_agentic_streaming
cargo run --example openrouter

# QUIC server (uses workspace crate)
cargo run --example quic_server
```

## Running Tests

Test the published crates:

```bash
# Test all workspace crates
cargo test --workspace

# Test specific published crate
cargo test -p temporal-compare

# With output
cargo test -- --nocapture
```

## Running Benchmarks

```bash
# All benchmarks
cargo bench --workspace

# Specific crate benchmark
cargo bench -p nanosecond-scheduler

# Save baseline for comparison
cargo bench -- --save-baseline main
```

## Troubleshooting

### Build Issues

**Problem**: `cargo: command not found`
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup update
```

**Problem**: Crate not found on crates.io
```bash
# Make sure you're using version 0.1 or compatible
# Check latest versions:
cargo search temporal-compare
cargo search nanosecond-scheduler
```

**Problem**: WASM build fails
```bash
# Install wasm-pack
cargo install wasm-pack

# Build WASM from published crates
wasm-pack build --target web
```

### Runtime Issues

**Problem**: Version conflicts
```bash
# Update all dependencies
cargo update

# Use specific versions
temporal-compare = "=0.1.0"
```

**Problem**: Performance issues
```bash
# Always build with --release for production
cargo build --release

# Run benchmarks to verify
cargo bench
```

## Version Information

All published crates are at version **0.1.x**:

- temporal-compare: 0.1.x
- nanosecond-scheduler: 0.1.x
- temporal-attractor-studio: 0.1.x
- temporal-neural-solver: 0.1.x
- strange-loop: 0.1.x

Check for updates:

```bash
cargo update
cargo outdated  # If you have cargo-outdated installed
```

## Next Steps

1. ✅ Install published crates from crates.io
2. 📖 Read the [complete README](../README.md)
3. 🔍 Explore [API documentation](https://docs.rs)
4. 💡 Try the examples above
5. 🚀 Build your real-time application!

## Getting Help

- 📖 **Documentation**: [docs.rs](https://docs.rs) for each crate
- 💬 **Examples**: `examples/` directory in repository
- 🐛 **Issues**: [GitHub Issues](https://github.com/ruvnet/midstream/issues)
- 📧 **Contact**: See main README.md

---

**Happy streaming with MidStream published crates!** 🚀

**All core crates are production-ready and available on crates.io**
