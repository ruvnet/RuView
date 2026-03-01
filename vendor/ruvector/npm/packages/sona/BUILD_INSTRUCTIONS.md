# SONA NAPI-RS Build Instructions

## Overview

This document describes how to build the SONA Node.js native module from the Rust crate using NAPI-RS.

## Prerequisites

- Rust toolchain (1.70+)
- Node.js (16+)
- npm or yarn
- @napi-rs/cli

## Directory Structure

```
/workspaces/ruvector/
├── crates/sona/                  # Rust crate
│   ├── src/
│   │   ├── napi_simple.rs        # NAPI bindings
│   │   ├── engine.rs             # Core engine
│   │   ├── lora.rs               # LoRA implementations
│   │   ├── types.rs              # Type definitions
│   │   └── ...
│   ├── Cargo.toml                # Rust dependencies
│   └── build.rs                  # Build script
└── npm/packages/sona/            # NPM package
    ├── package.json              # NPM configuration
    ├── index.js                  # JavaScript entry point
    ├── index.d.ts                # TypeScript definitions
    ├── examples/                 # Example scripts
    └── test/                     # Test files
```

## Build Steps

### 1. Build the Rust crate with NAPI feature

```bash
cd /workspaces/ruvector/crates/sona
cargo build --release --features napi
```

### 2. Build the Node.js module

```bash
cd /workspaces/ruvector/npm/packages/sona
npm install
npm run build
```

This will:
- Install dependencies including `@napi-rs/cli`
- Build the native module for your platform
- Generate platform-specific `.node` files

### 3. Run tests

```bash
npm test
```

### 4. Run examples

```bash
node examples/basic-usage.js
node examples/custom-config.js
node examples/llm-integration.js
```

## NAPI-RS Configuration

The build is configured via `package.json`:

```json
{
  "napi": {
    "name": "sona",
    "triples": {
      "defaults": true,
      "additional": [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-gnu",
        "armv7-unknown-linux-gnueabihf",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc"
      ]
    }
  }
}
```

## Cross-Compilation

To build for multiple platforms:

```bash
npm run build -- --target x86_64-unknown-linux-musl
npm run build -- --target aarch64-apple-darwin
npm run build -- --target x86_64-pc-windows-msvc
```

## Publishing

### Prepare for publishing

```bash
napi prepublish -t npm
```

### Create universal binary (macOS)

```bash
napi universal
```

### Publish to npm

```bash
npm publish
```

## API Differences from Rust

The NAPI bindings use a simplified API compared to the Rust API:

### Rust API (via `begin_trajectory`)
```rust
let builder = engine.begin_trajectory(embedding);
builder.add_step(activations, attention, reward);
engine.end_trajectory(builder, quality);
```

### Node.js API (via trajectory ID)
```javascript
const trajId = engine.beginTrajectory(embedding);
engine.addTrajectoryStep(trajId, activations, attention, reward);
engine.setTrajectoryRoute(trajId, "route");
engine.endTrajectory(trajId, quality);
```

This design avoids exposing the `TrajectoryBuilder` struct to JavaScript, which simplifies NAPI bindings.

## Troubleshooting

### Build fails with "could not find \`napi\`"

Ensure you're building with the `napi` feature:
```bash
cargo build --features napi
```

### Module not found at runtime

The native module must be built before running Node.js code:
```bash
npm run build
```

### Platform-specific issues

Check that your Rust toolchain supports the target platform:
```bash
rustup target list
rustup target add <target-triple>
```

## Performance Notes

- The native module uses zero-copy for Float64Arrays where possible
- Global trajectory storage uses `OnceLock` for thread-safe initialization
- Mutex-protected HashMap for trajectory builders (minimal contention)

## Memory Management

- Trajectory builders are stored globally until `endTrajectory` is called
- Finished trajectories are automatically cleaned up
- No manual memory management required in JavaScript

## Feature Flags

The NAPI bindings respect these Cargo features:

- `napi` - Enable NAPI bindings (required)
- `serde-support` - Required by napi feature
- `simd` - Enable SIMD optimizations (optional, recommended)

Build with all features:
```bash
cargo build --release --features napi,simd
```

## License

MIT OR Apache-2.0
