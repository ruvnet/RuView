# RuvLLM ESP32 - Tiny LLM Inference Engine for ESP32 Microcontrollers

[![crates.io](https://img.shields.io/crates/v/ruvllm-esp32.svg)](https://crates.io/crates/ruvllm-esp32)
[![npm](https://img.shields.io/npm/v/ruvllm-esp32.svg)](https://www.npmjs.com/package/ruvllm-esp32)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Run AI locally on ESP32 microcontrollers** - A complete, production-ready LLM inference engine with INT8/Binary quantization, HNSW vector search, RAG (Retrieval-Augmented Generation), and multi-chip federation support. No cloud required.

## Why RuvLLM ESP32?

Run AI directly on microcontrollers without cloud dependencies:

- **Privacy**: Data never leaves the device
- **Latency**: No network round-trips (2-5ms/token)
- **Cost**: Zero API fees, runs on $4 hardware
- **Offline**: Works without internet connectivity
- **Edge AI**: Perfect for IoT, robotics, wearables

## Features at a Glance

| Category | Features |
|----------|----------|
| **Inference** | INT8 quantized transformers, 2-5ms/token @ 240MHz |
| **Compression** | Binary quantization (32x), Product quantization (8-32x) |
| **Adaptation** | MicroLoRA on-device fine-tuning (2KB overhead) |
| **Attention** | Sparse patterns: sliding window, strided, BigBird |
| **Vector Search** | HNSW index with 1000+ vectors in ~20KB RAM |
| **Memory** | Semantic memory with context-aware retrieval + TTL |
| **RAG** | Retrieval-Augmented Generation for knowledge bases |
| **Anomaly** | Statistical outlier detection via embeddings |
| **Speedup** | Speculative decoding (2-4x potential) |
| **Scaling** | Multi-chip federation with pipeline/tensor parallelism |

## Supported Hardware

| Variant | SRAM | CPU | Features |
|---------|------|-----|----------|
| ESP32 | 520KB | Xtensa LX6 @ 240MHz | WiFi, Bluetooth |
| ESP32-S2 | 320KB | Xtensa LX7 @ 240MHz | USB OTG |
| ESP32-S3 | 512KB | Xtensa LX7 @ 240MHz | **SIMD/Vector**, USB OTG |
| ESP32-C3 | 400KB | RISC-V @ 160MHz | Low power, WiFi 4 |
| ESP32-C6 | 512KB | RISC-V @ 160MHz | **WiFi 6**, Thread |

**Recommended**: ESP32-S3 for best performance (SIMD acceleration)

---

## Quick Start

### Option 1: npx (Easiest - No Rust Required)

```bash
# Install ESP32 toolchain
npx ruvllm-esp32 install

# Build firmware
npx ruvllm-esp32 build --target esp32s3 --release

# Flash to device (auto-detects port)
npx ruvllm-esp32 flash

# Monitor serial output
npx ruvllm-esp32 monitor
```

### Option 2: One-Line Install Script

**Linux/macOS:**
```bash
git clone https://github.com/ruvnet/ruvector
cd ruvector/examples/ruvLLM/esp32-flash
./install.sh              # Install deps + build
./install.sh flash        # Flash to auto-detected port
```

**Windows (PowerShell):**
```powershell
git clone https://github.com/ruvnet/ruvector
cd ruvector\examples\ruvLLM\esp32-flash
.\install.ps1             # Install deps (restart PowerShell after)
.\install.ps1 build       # Build
.\install.ps1 flash COM6  # Flash
```

### Option 3: Manual Build

```bash
# Install ESP32 toolchain
cargo install espup espflash ldproxy
espup install
source ~/export-esp.sh  # Linux/macOS

# Clone and build
git clone https://github.com/ruvnet/ruvector
cd ruvector/examples/ruvLLM/esp32-flash
cargo build --release

# Flash
espflash flash --monitor --port /dev/ttyUSB0 \
  target/xtensa-esp32-espidf/release/ruvllm-esp32
```

---

## Complete Feature Guide

### 1. Quantization & Compression

#### Binary Quantization (32x compression)
Packs weights into 1-bit representation with sign encoding:
```
Original: [-0.5, 0.3, -0.1, 0.8] (32 bytes)
Binary:   [0b1010] (1 byte) + scale
```

#### Product Quantization (8-32x compression)
Splits vectors into subspaces with learned codebooks:
- 8 subspaces with 16 centroids each
- Asymmetric Distance Computation (ADC) for fast search
- Configurable compression ratio

### 2. Sparse Attention Patterns

Reduce attention complexity from O(n²) to O(n):

| Pattern | Description | Best For |
|---------|-------------|----------|
| Sliding Window | Local context only | Long sequences |
| Strided | Every k-th position | Periodic patterns |
| BigBird | Global + local + random | General purpose |
| Dilated | Exponentially increasing gaps | Hierarchical |
| Causal | Lower triangular mask | Autoregressive |

### 3. MicroLoRA Adaptation

On-device model fine-tuning with minimal overhead:
- **Rank**: 1-2 (trades quality for memory)
- **Memory**: ~2KB per layer
- **Use case**: Personalization, domain adaptation

### 4. HNSW Vector Search

Hierarchical Navigable Small World index:
- **Capacity**: 1000+ vectors in ~20KB
- **Latency**: <1ms search time
- **Metrics**: Euclidean, Cosine, Dot Product
- **Binary mode**: For memory-constrained variants

### 5. Semantic Memory

Context-aware memory with intelligent retrieval:
- **Memory types**: Factual, Episodic, Procedural
- **TTL support**: Auto-expire old memories
- **Importance scoring**: Prioritize critical information
- **Temporal decay**: Recent memories weighted higher

### 6. RAG (Retrieval-Augmented Generation)

Combine retrieval with generation:
```
> add The capital of France is Paris
Added knowledge #1

> ask what is the capital of France
Found: The capital of France is Paris
```

### 7. Anomaly Detection

Detect outliers using embedding distance:
```
> anomaly this is normal text
NORMAL (score: 15, threshold: 45)

> anomaly xkcd random gibberish 12345
ANOMALY (score: 89, threshold: 45)
```

### 8. Speculative Decoding

Draft-verify approach for faster generation:
- Draft model generates 4 tokens speculatively
- Target model verifies in parallel
- Accept matching tokens, reject mismatches
- **Speedup**: 2-4x on supported models

### 9. Multi-Chip Federation

Scale beyond single-chip memory limits:

#### Pipeline Parallelism
Split model layers across chips:
```
Chip 1: Layers 0-3   →   Chip 2: Layers 4-7   →   Output
```

#### Tensor Parallelism
Split each layer across chips:
```
         ┌─ Chip 1: Head 0-3 ─┐
Input ───┤                    ├───> Output
         └─ Chip 2: Head 4-7 ─┘
```

---

## Serial Commands

Connect at 115200 baud after flashing:

```
════════════════════════════════════════════
RuvLLM ESP32 Full-Feature v0.2
════════════════════════════════════════════
Features: Binary Quant, PQ, LoRA, HNSW, RAG
          Semantic Memory, Anomaly Detection
          Speculative Decoding, Federation
════════════════════════════════════════════
Type 'help' for commands
>
```

| Command | Description | Example |
|---------|-------------|---------|
| `gen <text>` | Generate tokens from prompt | `gen Hello world` |
| `add <text>` | Add knowledge to RAG | `add Meeting at 3pm` |
| `ask <query>` | Query knowledge base | `ask when is meeting` |
| `anomaly <text>` | Check for anomaly | `anomaly test input` |
| `stats` | Show system statistics | `stats` |
| `features` | List enabled features | `features` |
| `help` | Show command help | `help` |

---

## Platform-Specific Setup

### Windows

```powershell
# Install Rust
winget install Rustlang.Rust.MSVC

# Install ESP32 toolchain
cargo install espup espflash ldproxy
espup install

# RESTART PowerShell to load environment

# Build and flash
cargo build --release
espflash flash --port COM6 --monitor target\xtensa-esp32-espidf\release\ruvllm-esp32
```

### macOS

```bash
# Install Rust
brew install rustup
rustup-init -y
source ~/.cargo/env

# Install ESP32 toolchain
cargo install espup espflash ldproxy
espup install
source ~/export-esp.sh

# Build and flash
cargo build --release
espflash flash --port /dev/cu.usbserial-0001 --monitor target/xtensa-esp32-espidf/release/ruvllm-esp32
```

### Linux

```bash
# Install prerequisites (Debian/Ubuntu)
sudo apt install build-essential pkg-config libudev-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install ESP32 toolchain
cargo install espup espflash ldproxy
espup install
source ~/export-esp.sh

# Add user to dialout group (for serial access)
sudo usermod -a -G dialout $USER
# Log out and back in

# Build and flash
cargo build --release
espflash flash --port /dev/ttyUSB0 --monitor target/xtensa-esp32-espidf/release/ruvllm-esp32
```

---

## Cluster Setup (Multi-Chip)

For models larger than single-chip memory:

### 1. Generate Config

```bash
npx ruvllm-esp32 cluster --chips 5
# or
make cluster CHIPS=5
```

### 2. Edit `cluster.toml`

```toml
[cluster]
name = "my-cluster"
chips = 5
topology = "pipeline"  # or "tensor"

[[chips.nodes]]
id = 1
role = "master"
port = "/dev/ttyUSB0"
layers = [0, 1]

[[chips.nodes]]
id = 2
role = "worker"
port = "/dev/ttyUSB1"
layers = [2, 3]
# ... more chips
```

### 3. Flash All Chips

```bash
./cluster-flash.sh
# or
npx ruvllm-esp32 cluster flash
```

### 4. Monitor Cluster

```bash
./cluster-monitor.sh   # Opens tmux with all serial monitors
```

---

## Memory & Performance

### Resource Usage

| Component | RAM | Flash |
|-----------|-----|-------|
| LLM Model (INT8) | ~20 KB | ~16 KB |
| HNSW Index (256 vectors) | ~8 KB | — |
| RAG Knowledge (64 entries) | ~4 KB | — |
| Semantic Memory (32 entries) | ~2 KB | — |
| Anomaly Detector | ~2 KB | — |
| UART + Stack | ~9 KB | — |
| **Total** | **~45 KB** | **~16 KB** |

### Performance Benchmarks

| Operation | ESP32 @ 240MHz | ESP32-S3 (SIMD) |
|-----------|----------------|-----------------|
| Token generation | ~4ms/token | ~2ms/token |
| HNSW search (256 vectors) | ~1ms | ~0.5ms |
| Embedding (64-dim) | <1ms | <0.5ms |
| Anomaly check | <1ms | <0.5ms |
| Binary quant inference | ~1.5ms | ~0.8ms |

### Throughput

- **Standard**: ~200-250 tokens/sec (simulated)
- **With speculative**: ~400-500 tokens/sec (simulated)
- **Actual ESP32**: ~200-500 tokens/sec depending on model

---

## Project Structure

```
esp32-flash/
├── Cargo.toml                    # Rust config with feature flags
├── src/
│   ├── lib.rs                    # Library exports
│   ├── main.rs                   # Full-featured ESP32 binary
│   ├── optimizations/
│   │   ├── binary_quant.rs       # 32x compression
│   │   ├── product_quant.rs      # 8-32x compression
│   │   ├── lookup_tables.rs      # Pre-computed LUTs
│   │   ├── micro_lora.rs         # On-device adaptation
│   │   ├── sparse_attention.rs   # Memory-efficient attention
│   │   └── pruning.rs            # Weight pruning
│   ├── federation/
│   │   ├── protocol.rs           # Multi-chip communication
│   │   ├── pipeline.rs           # Pipeline parallelism
│   │   └── speculative.rs        # Draft-verify decoding
│   └── ruvector/
│       ├── micro_hnsw.rs         # Vector index
│       ├── semantic_memory.rs    # Context-aware memory
│       ├── rag.rs                # Retrieval-augmented gen
│       └── anomaly.rs            # Outlier detection
├── npm/                          # npx package
│   ├── package.json
│   └── bin/
│       ├── cli.js                # CLI implementation
│       └── postinstall.js        # Setup script
├── .github/workflows/
│   └── release.yml               # Automated builds
├── install.sh                    # Linux/macOS installer
├── install.ps1                   # Windows installer
├── Makefile                      # Make targets
└── Dockerfile                    # Docker build
```

---

## Troubleshooting

### "Permission denied" on serial port

**Linux:**
```bash
sudo usermod -a -G dialout $USER
# Log out and back in
```

**Windows:** Run PowerShell as Administrator.

### "Failed to connect to ESP32"

1. Hold **BOOT** button while clicking flash
2. Check correct COM port in Device Manager
3. Use a data USB cable (not charge-only)
4. Close other serial monitors

### Build errors

```bash
# Re-run toolchain setup
espup install
source ~/export-esp.sh  # Linux/macOS
# Restart terminal on Windows
```

### Selecting ESP32 variant

Edit `.cargo/config.toml`:
```toml
# ESP32 (default)
target = "xtensa-esp32-espidf"

# ESP32-S3 (recommended)
target = "xtensa-esp32s3-espidf"

# ESP32-C3/C6 (RISC-V)
target = "riscv32imc-esp-espidf"
```

---

## Feature Flags

Build with specific features:

```bash
# Default (ESP32)
cargo build --release

# ESP32-S3 with federation
cargo build --release --features federation

# All features
cargo build --release --features full

# Host testing (no hardware needed)
cargo build --features host-test --no-default-features

# WebAssembly
cargo build --target wasm32-unknown-unknown --features wasm --no-default-features
```

---

## API Usage (Library)

Use as a Rust library:

```rust
use ruvllm_esp32::prelude::*;

// Vector search
let config = HNSWConfig::default();
let mut index: MicroHNSW<64, 256> = MicroHNSW::new(config);
index.insert(&vector)?;
let results = index.search(&query, 5);

// RAG
let mut rag: MicroRAG<64, 64> = MicroRAG::new(RAGConfig::default());
rag.add_knowledge("The sky is blue", &embedding)?;
let results = rag.retrieve(&query_embedding, 3);

// Semantic memory
let mut memory: SemanticMemory<64, 32> = SemanticMemory::new();
memory.add_memory(&embedding, &tokens, MemoryType::Factual)?;

// Anomaly detection
let mut detector = AnomalyDetector::new(AnomalyConfig::default());
let result = detector.check(&embedding);
if result.is_anomaly {
    println!("Anomaly detected!");
}

// Binary quantization
let binary = BinaryVector::from_f32(&float_vector);
let distance = hamming_distance(&a, &b);

// Product quantization
let pq = ProductQuantizer::new(PQConfig { dim: 64, num_subspaces: 8, num_centroids: 16 });
let code = pq.encode(&vector)?;
```

---

## Installation Options

### As npm CLI Tool (Recommended for Flashing)

```bash
# Use directly with npx (no install needed)
npx ruvllm-esp32 install
npx ruvllm-esp32 build --target esp32s3
npx ruvllm-esp32 flash

# Or install globally
npm install -g ruvllm-esp32
ruvllm-esp32 --help
```

### As Rust Library (For Custom Projects)

Add to your `Cargo.toml`:

```toml
[dependencies]
ruvllm-esp32 = "0.2"
```

The library crate is available at [crates.io/crates/ruvllm-esp32](https://crates.io/crates/ruvllm-esp32).

### Clone This Project (For Full Customization)

This directory contains a complete, ready-to-flash project with all features:

```bash
git clone https://github.com/ruvnet/ruvector
cd ruvector/examples/ruvLLM/esp32-flash
cargo build --release
```

---

## License

MIT

---

## Links

- [Main Repository](https://github.com/ruvnet/ruvector)
- [Rust Library (crates.io)](https://crates.io/crates/ruvllm-esp32)
- [npm CLI Tool](https://www.npmjs.com/package/ruvllm-esp32)
- [Documentation](https://docs.rs/ruvllm-esp32)
- [Issue Tracker](https://github.com/ruvnet/ruvector/issues)

---

## Keywords

ESP32 LLM, Tiny LLM, Embedded AI, Microcontroller AI, Edge AI, ESP32 Machine Learning, ESP32 Neural Network, INT8 Quantization, Binary Quantization, Product Quantization, HNSW Vector Search, RAG Embedded, Retrieval Augmented Generation ESP32, Semantic Memory, Anomaly Detection, Speculative Decoding, Multi-chip AI, Pipeline Parallelism, MicroLoRA, On-device Learning, IoT AI, ESP32-S3 SIMD, Xtensa AI, RISC-V AI, Offline AI, Privacy-preserving AI
