# ruvector-core-linux-arm64-gnu

[![npm version](https://badge.fury.io/js/ruvector-core-linux-arm64-gnu.svg)](https://www.npmjs.com/package/ruvector-core-linux-arm64-gnu)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Linux ARM64 GNU native binding for ruvector-core**

This package contains the native Node.js binding (`.node` file) for Linux ARM64 systems with GNU libc. It is automatically installed as an optional dependency when you install `ruvector-core` on a compatible system.

🌐 **[Visit ruv.io](https://ruv.io)** for more AI infrastructure tools

## Installation

**You should not install this package directly.** Instead, install the main package:

```bash
npm install ruvector-core
```

The correct platform-specific package will be automatically installed based on your system.

## System Requirements

- **Operating System**: Linux (GNU libc)
- **Architecture**: ARM64 / AArch64
- **Node.js**: 18.0.0 or higher
- **libc**: GNU C Library (glibc)

## Compatibility

This package is compatible with:
- Ubuntu 18.04+ (ARM64)
- Debian 10+ Buster (ARM64)
- CentOS 7+ / RHEL 7+ (ARM64)
- Amazon Linux 2+ (Graviton processors)
- Raspberry Pi OS 64-bit
- Most ARM64 Linux distributions using glibc

## What's Inside

This package contains:
- **ruvector.node** - Native binary module compiled from Rust for ARM64
- **index.js** - Module loader with error handling
- Full HNSW indexing implementation
- SIMD-optimized vector operations for ARM NEON
- Multi-threaded async operations via Tokio

## Performance

When running on Linux ARM64 systems (like AWS Graviton), you can expect:
- **50,000+ vector inserts per second**
- **10,000+ searches per second** (k=10)
- **~50 bytes memory per 128-dim vector**
- **Sub-millisecond latency** for most operations
- Optimized for ARM NEON SIMD instructions

## Popular ARM64 Platforms

- **AWS Graviton** (EC2 instances)
- **Raspberry Pi 4/5** (64-bit OS)
- **NVIDIA Jetson** (edge AI devices)
- **Apple Silicon** (via Docker/Linux)
- **Oracle Cloud** (Ampere processors)

## Building from Source

If you need to rebuild the native module:

```bash
# Clone the repository
git clone https://github.com/ruvnet/ruvector.git
cd ruvector

# Install Rust toolchain with ARM64 target
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-unknown-linux-gnu

# Build for Linux ARM64
cd npm/packages/core
npm run build:napi -- --target aarch64-unknown-linux-gnu
```

## Troubleshooting

### Module Not Found Error

If you see "Cannot find module 'ruvector-core-linux-arm64-gnu'":

1. Verify you're on a Linux ARM64 system: `uname -m` should output `aarch64`
2. Reinstall with optional dependencies: `npm install --include=optional ruvector-core`
3. Check Node.js version: `node --version` should be 18.0.0 or higher

### Binary Compatibility Issues

If the module fails to load:
1. Ensure you have glibc installed: `ldd --version`
2. The binary requires glibc 2.17+ (CentOS 7+) or 2.27+ (Ubuntu 18.04+)
3. For Alpine Linux or musl-based systems, this package will not work (use a glibc-based distro)

### Cross-Compilation

When building on x64 for ARM64:
```bash
# Install cross-compilation tools
sudo apt-get install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu

# Set environment variable
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

# Build
npm run build:napi -- --target aarch64-unknown-linux-gnu
```

## Related Packages

- **[ruvector-core](https://www.npmjs.com/package/ruvector-core)** - Main package (install this)
- **[ruvector-core-linux-x64-gnu](https://www.npmjs.com/package/ruvector-core-linux-x64-gnu)** - Linux x64
- **[ruvector-core-darwin-x64](https://www.npmjs.com/package/ruvector-core-darwin-x64)** - macOS Intel
- **[ruvector-core-darwin-arm64](https://www.npmjs.com/package/ruvector-core-darwin-arm64)** - macOS Apple Silicon
- **[ruvector-core-win32-x64-msvc](https://www.npmjs.com/package/ruvector-core-win32-x64-msvc)** - Windows x64

## Resources

- 🏠 [Homepage](https://ruv.io)
- 📦 [GitHub Repository](https://github.com/ruvnet/ruvector)
- 📚 [Documentation](https://github.com/ruvnet/ruvector/tree/main/docs)
- 🐛 [Issue Tracker](https://github.com/ruvnet/ruvector/issues)

## License

MIT License - see [LICENSE](https://github.com/ruvnet/ruvector/blob/main/LICENSE) for details.

---

Built with ❤️ by the [ruv.io](https://ruv.io) team
