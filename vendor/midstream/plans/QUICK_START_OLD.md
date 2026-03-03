# MidStream Quick Start Guide

Get up and running with MidStream in 5 minutes!

## Prerequisites

- Rust 1.70+ (`rustup update`)
- Node.js 16+ (for WASM package)
- Git

## Installation

### Option 1: Use the WASM Package (Fastest)

```bash
# Install from npm
cd npm-wasm
npm install

# Run the demo
npm run dev
# Open http://localhost:8080
```

### Option 2: Build from Source

```bash
# Clone the repository
git clone <your-repo-url>
cd midstream

# Build all crates
cargo build --workspace --release

# Run tests
cargo test --workspace

# Run benchmarks
./scripts/run_benchmarks.sh
```

## Quick Examples

### 1. Temporal Pattern Matching

```rust
use temporal_compare::{TemporalComparator, Sequence, ComparisonAlgorithm};

let comparator = TemporalComparator::new(1000, 10000);
let seq1 = Sequence::from_values(vec![1, 2, 3, 4, 5]);
let seq2 = Sequence::from_values(vec![1, 2, 4, 5]);

let result = comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW)?;
println!("DTW distance: {}", result.distance);
```

### 2. Real-Time Scheduling

```rust
use nanosecond_scheduler::{RealtimeScheduler, Priority, Deadline};

let scheduler = RealtimeScheduler::new(config);

let task_id = scheduler.schedule(
    my_task,
    Deadline::from_millis(100),
    Priority::High,
)?;

let task = scheduler.next_task().unwrap();
scheduler.execute_task(task, |payload| {
    // Execute your task
});
```

### 3. QUIC Multi-Stream

```rust
use quic_multistream::{QuicConnection, StreamPriority};

// Connect
let connection = QuicConnection::connect("https://server.example.com:4433").await?;

// Open stream
let mut stream = connection.open_bi_stream_with_priority(StreamPriority::High).await?;

// Send data
stream.send(b"Hello QUIC!").await?;

// Receive response
let mut buffer = vec![0u8; 1024];
let n = stream.recv(&mut buffer).await?;
```

### 4. Browser/WASM Usage

```html
<!DOCTYPE html>
<html>
<head>
    <script type="module">
        import init, { TemporalCompare } from './pkg/midstream_wasm.js';
        
        async function run() {
            await init();
            
            const compare = new TemporalCompare(1000);
            const distance = compare.dtw([1, 2, 3], [1, 2, 4]);
            
            console.log('DTW distance:', distance);
        }
        
        run();
    </script>
</head>
<body>
    <h1>MidStream WASM Demo</h1>
</body>
</html>
```

## Performance Expectations

| Operation | Native | WASM | Target |
|-----------|--------|------|--------|
| DTW (n=100) | ~8ms | ~16ms | <10ms (native) |
| Scheduling | ~85ns | N/A | <100ns |
| QUIC stream | ~0.8ms | ~1.5ms | <1ms |
| Pattern match | ~4ms | ~12ms | <5ms (native) |

## Running Examples

```bash
# QUIC server
cargo run --example quic_server

# Browser demo
cd npm-wasm
npm run dev
```

## Running Benchmarks

```bash
# All benchmarks
./scripts/run_benchmarks.sh

# Specific crate
cargo bench --bench temporal_bench

# Compare branches
./scripts/benchmark_comparison.sh main feature-branch
```

## Troubleshooting

### Build Issues

**Problem**: `cargo: command not found`
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

**Problem**: WASM build fails
```bash
# Install wasm-pack
cargo install wasm-pack

# Build WASM
cd npm-wasm
npm run build:wasm
```

### Runtime Issues

**Problem**: Tests fail
```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean
cargo build --workspace
```

## Next Steps

1. Read the [complete README](README.md)
2. Explore [API documentation](docs/api-reference.md)
3. Try the [interactive demo](npm-wasm/examples/demo.html)
4. Check [benchmark results](benches/README.md)
5. Review [architecture docs](docs/quic-architecture.md)

## Getting Help

- 📖 Documentation: `docs/`
- 💬 Examples: `examples/`
- 🐛 Issues: GitHub Issues
- 📧 Contact: See README.md

---

**Happy streaming with MidStream!** 🚀
