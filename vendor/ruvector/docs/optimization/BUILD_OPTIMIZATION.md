# Build Optimization Guide

Comprehensive guide for optimizing Ruvector builds for maximum performance.

## Quick Start

### Maximum Performance Build

```bash
# One-command optimized build
RUSTFLAGS="-C target-cpu=native -C target-feature=+avx2,+fma -C link-arg=-fuse-ld=lld" \
cargo build --release
```

## Compiler Flags

### Target CPU Optimization

```bash
# Native CPU (recommended for production)
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Specific CPUs
RUSTFLAGS="-C target-cpu=skylake" cargo build --release
RUSTFLAGS="-C target-cpu=znver3" cargo build --release
RUSTFLAGS="-C target-cpu=neoverse-v1" cargo build --release
```

### SIMD Features

```bash
# AVX2 + FMA
RUSTFLAGS="-C target-feature=+avx2,+fma" cargo build --release

# AVX-512 (if supported)
RUSTFLAGS="-C target-feature=+avx512f,+avx512dq,+avx512vl" cargo build --release

# List available features
rustc --print target-features
```

### Link-Time Optimization

Already configured in Cargo.toml:

```toml
[profile.release]
lto = "fat"           # Maximum LTO
codegen-units = 1     # Single codegen unit
```

Alternatives:

```toml
lto = "thin"          # Faster builds, slightly less optimization
codegen-units = 4     # Parallel codegen (faster builds)
```

### Linker Selection

Use faster linkers:

```bash
# LLD (LLVM linker) - recommended
RUSTFLAGS="-C link-arg=-fuse-ld=lld" cargo build --release

# Mold (fastest)
RUSTFLAGS="-C link-arg=-fuse-ld=mold" cargo build --release

# Gold
RUSTFLAGS="-C link-arg=-fuse-ld=gold" cargo build --release
```

## Profile-Guided Optimization (PGO)

### Step-by-Step PGO

```bash
#!/bin/bash
# pgo_build.sh

set -e

# 1. Clean previous builds
cargo clean

# 2. Build instrumented binary
echo "Building instrumented binary..."
mkdir -p /tmp/pgo-data
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" \
    cargo build --release --bin ruvector-bench

# 3. Run representative workload
echo "Running profiling workload..."
./target/release/ruvector-bench \
    --workload mixed \
    --vectors 1000000 \
    --queries 10000 \
    --dimensions 384

# You can run multiple workloads to cover different scenarios
./target/release/ruvector-bench \
    --workload search-heavy \
    --vectors 500000 \
    --queries 50000

# 4. Merge profiling data
echo "Merging profile data..."
llvm-profdata merge -o /tmp/pgo-data/merged.profdata /tmp/pgo-data/*.profraw

# 5. Build optimized binary
echo "Building PGO-optimized binary..."
RUSTFLAGS="-Cprofile-use=/tmp/pgo-data/merged.profdata -C target-cpu=native" \
    cargo build --release

echo "PGO build complete!"
echo "Binary: ./target/release/ruvector-bench"
```

### Expected PGO Gains

- **Throughput**: +10-15%
- **Latency**: -10-15%
- **Binary Size**: +5-10% (due to profiling data)

## Optimization Levels

### Cargo Profile Configurations

```toml
# Maximum performance (default)
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true

# Fast compilation, good performance
[profile.release-fast]
inherits = "release"
lto = "thin"
codegen-units = 16

# Debug with optimizations
[profile.dev-optimized]
inherits = "dev"
opt-level = 2
```

Build with custom profile:

```bash
cargo build --profile release-fast
```

## CPU-Specific Builds

### Intel CPUs

```bash
# Haswell (AVX2)
RUSTFLAGS="-C target-cpu=haswell" cargo build --release

# Skylake (AVX2 + better)
RUSTFLAGS="-C target-cpu=skylake" cargo build --release

# Cascade Lake (AVX-512)
RUSTFLAGS="-C target-cpu=cascadelake" cargo build --release

# Ice Lake (AVX-512 + more)
RUSTFLAGS="-C target-cpu=icelake-server" cargo build --release
```

### AMD CPUs

```bash
# Zen 2
RUSTFLAGS="-C target-cpu=znver2" cargo build --release

# Zen 3
RUSTFLAGS="-C target-cpu=znver3" cargo build --release

# Zen 4
RUSTFLAGS="-C target-cpu=znver4" cargo build --release
```

### ARM CPUs

```bash
# Neoverse N1
RUSTFLAGS="-C target-cpu=neoverse-n1" cargo build --release

# Neoverse V1
RUSTFLAGS="-C target-cpu=neoverse-v1" cargo build --release

# Apple Silicon
RUSTFLAGS="-C target-cpu=apple-m1" cargo build --release
```

## Dependency Optimization

### Optimize Dependencies

Add to Cargo.toml:

```toml
[profile.release.package."*"]
opt-level = 3
```

### Feature Selection

Disable unused features:

```toml
[dependencies]
tokio = { version = "1", default-features = false, features = ["rt-multi-thread"] }
```

## Cross-Compilation

### Building for Different Targets

```bash
# Add target
rustup target add x86_64-unknown-linux-musl

# Build for target
cargo build --release --target x86_64-unknown-linux-musl

# With optimizations
RUSTFLAGS="-C target-cpu=generic" \
    cargo build --release --target x86_64-unknown-linux-musl
```

## Build Scripts

### Automated Optimized Build

```bash
#!/bin/bash
# build_optimized.sh

set -euo pipefail

# Detect CPU
CPU_ARCH=$(lscpu | grep "Model name" | sed 's/Model name: *//')
echo "Detected CPU: $CPU_ARCH"

# Set optimal flags
if [[ $CPU_ARCH == *"Intel"* ]]; then
    if [[ $CPU_ARCH == *"Ice Lake"* ]] || [[ $CPU_ARCH == *"Cascade Lake"* ]]; then
        TARGET_CPU="icelake-server"
        TARGET_FEATURES="+avx512f,+avx512dq"
    else
        TARGET_CPU="skylake"
        TARGET_FEATURES="+avx2,+fma"
    fi
elif [[ $CPU_ARCH == *"AMD"* ]]; then
    if [[ $CPU_ARCH == *"Zen 3"* ]]; then
        TARGET_CPU="znver3"
    elif [[ $CPU_ARCH == *"Zen 4"* ]]; then
        TARGET_CPU="znver4"
    else
        TARGET_CPU="znver2"
    fi
    TARGET_FEATURES="+avx2,+fma"
else
    TARGET_CPU="native"
    TARGET_FEATURES="+avx2,+fma"
fi

echo "Using target-cpu: $TARGET_CPU"
echo "Using target-features: $TARGET_FEATURES"

# Build
RUSTFLAGS="-C target-cpu=$TARGET_CPU -C target-feature=$TARGET_FEATURES -C link-arg=-fuse-ld=lld" \
    cargo build --release

echo "Build complete!"
ls -lh target/release/
```

## Benchmarking Builds

### Compare Optimization Levels

```bash
#!/bin/bash
# benchmark_builds.sh

echo "Building and benchmarking different optimization levels..."

# Baseline
cargo clean
cargo build --release
hyperfine --warmup 3 './target/release/ruvector-bench' --export-json baseline.json

# With target-cpu=native
cargo clean
RUSTFLAGS="-C target-cpu=native" cargo build --release
hyperfine --warmup 3 './target/release/ruvector-bench' --export-json native.json

# With AVX2
cargo clean
RUSTFLAGS="-C target-feature=+avx2,+fma" cargo build --release
hyperfine --warmup 3 './target/release/ruvector-bench' --export-json avx2.json

# Compare
echo "Comparing results..."
hyperfine --warmup 3 \
    -n "baseline" './target/release-baseline/ruvector-bench' \
    -n "native" './target/release-native/ruvector-bench' \
    -n "avx2" './target/release-avx2/ruvector-bench'
```

## Production Build Checklist

- [ ] Use `target-cpu=native` or specific CPU
- [ ] Enable LTO (`lto = "fat"`)
- [ ] Set `codegen-units = 1`
- [ ] Enable `panic = "abort"`
- [ ] Strip symbols (`strip = true`)
- [ ] Use fast linker (lld or mold)
- [ ] Run PGO if possible
- [ ] Test on production-like workload
- [ ] Verify SIMD instructions with `objdump`
- [ ] Benchmark before deployment

## Verification

### Check SIMD Instructions

```bash
# Check for AVX2
objdump -d target/release/ruvector-bench | grep vfmadd

# Check for AVX-512
objdump -d target/release/ruvector-bench | grep vfmadd512

# Check all SIMD instructions
objdump -d target/release/ruvector-bench | grep -E "vmovups|vfmadd|vaddps"
```

### Verify Optimizations

```bash
# Check optimization level
readelf -p .comment target/release/ruvector-bench

# Check binary size
ls -lh target/release/ruvector-bench

# Check linked libraries
ldd target/release/ruvector-bench
```

## Troubleshooting

### Build Errors

**Problem**: AVX-512 not supported

```bash
# Fall back to AVX2
RUSTFLAGS="-C target-feature=+avx2,+fma" cargo build --release
```

**Problem**: Linker errors

```bash
# Use system linker
cargo build --release
# No RUSTFLAGS needed
```

**Problem**: Slow builds

```bash
# Use thin LTO and parallel codegen
[profile.release]
lto = "thin"
codegen-units = 16
```

## References

- [rustc Codegen Options](https://doc.rust-lang.org/rustc/codegen-options/)
- [Cargo Profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [PGO Guide](https://doc.rust-lang.org/rustc/profile-guided-optimization.html)
