# Installation Guide

This guide covers installation of Ruvector for all supported platforms: Rust, Node.js, WASM/Browser, and CLI.

## Prerequisites

### Rust
- **Rust 1.80+** (latest stable recommended)
- **Cargo** (included with Rust)

Install Rust from [rustup.rs](https://rustup.rs/):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Node.js
- **Node.js 16+** (v18 or v20 recommended)
- **npm** or **yarn**

Download from [nodejs.org](https://nodejs.org/)

### Browser (WASM)
- Modern browser with WebAssembly support
- Chrome 91+, Firefox 89+, Safari 15+, Edge 91+

## Installation

### 1. Rust Library

#### Add to Cargo.toml
```toml
[dependencies]
ruvector-core = "2.0"
```

For the RVF binary format (separate workspace in `crates/rvf`):
```toml
[dependencies]
rvf-runtime = "0.2"
rvf-crypto = "0.2"
rvf-types = "0.2"
```

#### Build with optimizations
```bash
# Standard build
cargo build --release

# With SIMD optimizations (recommended)
RUSTFLAGS="-C target-cpu=native" cargo build --release

# For specific CPU features
RUSTFLAGS="-C target-feature=+avx2,+fma" cargo build --release
```

#### Optional features (ruvector-core)
```toml
[dependencies]
ruvector-core = { version = "2.0", features = ["hnsw", "storage"] }
```

Available features:
- `hnsw`: HNSW indexing (enabled by default)
- `storage`: Persistent storage backend
- `simd`: SIMD intrinsics (enabled by default on x86_64)

### 2. Node.js Package

#### NPM
```bash
npm install ruvector
```

#### Yarn
```bash
yarn add ruvector
```

#### pnpm
```bash
pnpm add ruvector
```

#### Verify installation
```javascript
const { VectorDB } = require('ruvector');
console.log('Ruvector loaded successfully!');
```

#### Platform-specific binaries

RuVector uses NAPI-RS for native bindings. Pre-built binaries are available for:
- **Linux**: x64 (glibc), x64 (musl), arm64 (glibc), arm64 (musl)
- **macOS**: x64, arm64 (Apple Silicon)
- **Windows**: x64

If no pre-built binary is available, it will compile from source (requires Rust).

### 3. Browser (WASM)

#### NPM package
```bash
npm install @ruvector/wasm
```

There are also specialized WASM packages:
```bash
npm install @ruvector/rvf-wasm     # RVF format in browser
npm install @ruvector/gnn-wasm     # Graph neural networks
```

#### Basic usage
```html
<!DOCTYPE html>
<html>
<head>
    <title>RuVector WASM Demo</title>
</head>
<body>
    <script type="module">
        import init, { VectorDB } from '@ruvector/wasm';

        async function main() {
            await init();

            const db = new VectorDB(128); // 128 dimensions
            const id = db.insert(new Float32Array(128).fill(0.1), null);
            console.log('Inserted:', id);

            const results = db.search(new Float32Array(128).fill(0.1), 10);
            console.log('Results:', results);
        }

        main();
    </script>
</body>
</html>
```

### 4. CLI Tool

#### Build from source (not yet on crates.io)
```bash
git clone https://github.com/ruvnet/ruvector.git
cd ruvector
cargo install --path crates/ruvector-cli
```

#### Verify installation
```bash
ruvector --version
```

#### Available subcommands
```bash
ruvector create    # Create a new database
ruvector insert    # Insert vectors
ruvector search    # Search for similar vectors
ruvector info      # Show database info
ruvector export    # Export database
ruvector import    # Import data
ruvector benchmark # Run benchmarks
ruvector graph     # Graph database operations (create, query, shell, serve)
ruvector hooks     # Hooks management
```

## Platform-Specific Notes

### Linux

#### Dependencies
```bash
# Debian/Ubuntu
sudo apt-get install build-essential

# RHEL/CentOS/Fedora
sudo yum groupinstall "Development Tools"

# Arch
sudo pacman -S base-devel
```

#### Permissions
Ensure write access to database directory:
```bash
chmod 755 ./data
```

### macOS

#### Xcode Command Line Tools
```bash
xcode-select --install
```

#### Apple Silicon (M1/M2/M3)
NAPI-RS provides native arm64 binaries. For Rust, ensure you're using the correct toolchain:
```bash
rustup target add aarch64-apple-darwin
```

### Windows

#### Visual Studio Build Tools
Download from [visualstudio.microsoft.com](https://visualstudio.microsoft.com/downloads/)

Install "Desktop development with C++"

#### Windows Subsystem for Linux (WSL)
Alternatively, use WSL2:
```bash
wsl --install
```

Then follow Linux instructions.

## Docker

### Build from source
```dockerfile
FROM rust:1.80 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p ruvector-cli

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/ruvector /usr/local/bin/
CMD ["ruvector", "--help"]
```

```bash
docker build -t ruvector .
docker run -v $(pwd)/data:/data ruvector
```

## Verification

### Rust
```rust
use ruvector_core::VectorDB;
use ruvector_core::types::DbOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = VectorDB::new(DbOptions::default())?;
    println!("VectorDB created successfully");
    Ok(())
}
```

### Node.js
```javascript
const { VectorDB } = require('ruvector');
const db = new VectorDB({ dimensions: 128 });
console.log('VectorDB created successfully!');
```

### CLI
```bash
ruvector --version
ruvector --help
```

## Troubleshooting

### Compilation Errors

**Error**: `error: linking with cc failed`
```bash
# Install build tools (see Platform-Specific Notes above)
```

**Error**: `error: failed to run custom build command for napi`
```bash
# Install Node.js and ensure it's in PATH
which node
npm --version
```

### Runtime Errors

**Error**: `cannot load native addon`
```bash
# Rebuild from source
npm rebuild ruvector
```

**Error**: `SIGSEGV` or segmentation fault
```bash
# Disable SIMD optimizations
export RUVECTOR_DISABLE_SIMD=1
```

### Performance Issues

**Slow queries**
```bash
# Enable SIMD optimizations
export RUSTFLAGS="-C target-cpu=native"
cargo build --release
```

**High memory usage**
```bash
# Enable quantization (see Advanced Features guide)
```

## Next Steps

- [Getting Started Guide](GETTING_STARTED.md) - Quick start tutorial
- [Basic Tutorial](BASIC_TUTORIAL.md) - Step-by-step examples
- [Performance Tuning](PERFORMANCE_TUNING.md) - Optimization guide
- [API Reference](../api/) - Complete API documentation

## Support

For installation issues:
1. Check [GitHub Issues](https://github.com/ruvnet/ruvector/issues)
2. Search [Stack Overflow](https://stackoverflow.com/questions/tagged/ruvector)
3. Open a new issue with:
   - OS and version
   - Rust/Node.js version
   - Error messages and logs
   - Steps to reproduce
