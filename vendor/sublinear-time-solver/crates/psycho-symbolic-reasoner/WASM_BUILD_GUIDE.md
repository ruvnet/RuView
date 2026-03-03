# Psycho-Symbolic Reasoner WASM Build Guide

This guide covers the complete WASM compilation pipeline for the psycho-symbolic-reasoner project, which compiles three Rust crates into WebAssembly modules for use in JavaScript/TypeScript applications.

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Project Structure](#project-structure)
4. [Build Process](#build-process)
5. [Testing](#testing)
6. [Usage](#usage)
7. [Troubleshooting](#troubleshooting)
8. [Performance](#performance)

## Overview

The psycho-symbolic-reasoner project consists of three main Rust crates that are compiled to WASM:

- **graph_reasoner**: Knowledge graph construction and inference engine
- **extractors**: Text analysis for sentiment, emotions, and preferences
- **planner**: Goal-oriented action planning (GOAP) system

Each crate is compiled separately and then bundled together into a unified JavaScript package.

## Prerequisites

### Required Tools

1. **Rust** (1.70+)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup target add wasm32-unknown-unknown
   ```

2. **wasm-pack** (0.12+)
   ```bash
   curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
   ```

3. **Node.js** (18+) and npm

### System Requirements

- Operating System: Linux, macOS, or Windows
- Memory: 4GB+ RAM recommended for compilation
- Disk Space: 2GB+ free space

## Project Structure

```
psycho-symbolic-reasoner/
├── graph_reasoner/              # Knowledge graph reasoning
│   ├── src/
│   ├── Cargo.toml
│   ├── wasm-pack.toml          # WASM build configuration
│   └── pkg/                    # Generated WASM output
├── extractors/                 # Text analysis and extraction
│   ├── src/
│   ├── Cargo.toml
│   ├── wasm-pack.toml
│   └── pkg/
├── planner/                    # Goal-oriented planning
│   ├── src/
│   ├── Cargo.toml
│   ├── wasm-pack.toml
│   └── pkg/
├── wasm-dist/                  # Unified bundle output
├── build-wasm.cjs             # Build script
├── test-wasm.cjs              # Test script
├── bundle-wasm.cjs            # Bundling script
└── package.json               # Build configuration
```

## Build Process

### Quick Start

```bash
# Install dependencies
npm install

# Build all WASM modules (development)
node build-wasm.cjs --dev

# Build all WASM modules (production)
node build-wasm.cjs

# Test the modules
node test-wasm.cjs

# Create unified bundle
node bundle-wasm.cjs
```

### Detailed Build Steps

#### 1. Individual Crate Compilation

Each crate is compiled using wasm-pack with specific configurations:

```bash
# Development build (with debug symbols)
wasm-pack build --target web --dev graph_reasoner
wasm-pack build --target web --dev extractors
wasm-pack build --target web --dev planner

# Production build (optimized)
wasm-pack build --target web --release graph_reasoner
wasm-pack build --target web --release extractors
wasm-pack build --target web --release planner
```

#### 2. WASM Configuration

Each crate has a `wasm-pack.toml` file:

```toml
[build]
target = "web"
out-dir = "pkg"
out-name = "crate_name"

[build.profile.release]
wee-alloc = true

[build.profile.dev]
debug = true

[pack]
name = "@psycho-symbolic/crate-name"
```

#### 3. Dependency Configuration

The workspace `Cargo.toml` includes WASM-compatible dependencies:

```toml
# WASM support
wasm-bindgen = "0.2"
js-sys = "0.3"
wasm-bindgen-futures = "0.4"
console_error_panic_hook = "0.1"

# Random number generation (WASM-compatible)
rand = { version = "0.8", features = ["small_rng"] }
getrandom = { version = "0.2", features = ["js"] }
uuid = { version = "1.0", features = ["v4", "wasm-bindgen"] }
```

### Build Scripts

#### build-wasm.cjs

The main build script handles:
- Prerequisites checking
- Parallel compilation of all crates
- Package.json generation with proper exports
- Error handling and reporting

Key features:
- Development/production modes
- Automatic dependency installation
- Progress reporting
- Build verification

#### Features

```javascript
// Check if wasm-pack is installed
function checkWasmPack() { ... }

// Build individual crate
function buildCrate(crateName) { ... }

// Update package.json with exports
function updatePackageJson(crateName, pkgPath) { ... }
```

## Testing

### Test Suite (test-wasm.cjs)

The test script validates:
- Module loading and initialization
- Basic functionality of each crate
- Performance benchmarks
- TypeScript compatibility

#### Running Tests

```bash
# Run all tests
node test-wasm.cjs

# Individual tests are automatically run for:
# - Graph reasoning (fact insertion, queries, inference)
# - Text extraction (sentiment, emotions, preferences)
# - Planning (state management, action planning)
```

#### Test Coverage

1. **Graph Reasoner Tests**:
   - Fact insertion and retrieval
   - Query execution
   - Inference engine
   - Graph statistics

2. **Extractors Tests**:
   - Sentiment analysis
   - Preference extraction
   - Emotion detection
   - Comprehensive text analysis

3. **Planner Tests**:
   - World state management
   - Action and goal definition
   - Plan generation
   - Rule evaluation

#### Performance Benchmarks

The test suite includes performance benchmarks:
- Fact insertion rate: ~22,000 facts/second
- Memory usage tracking
- Execution time measurements

## Usage

### JavaScript/ES6 Modules

```javascript
import PsychoSymbolicReasoner from '@psycho-symbolic/reasoner';

const reasoner = new PsychoSymbolicReasoner();

// Graph reasoning
reasoner.addFact("Alice", "knows", "Bob");
const query = reasoner.query('{"type": "simple", "subject": "Alice"}');

// Text analysis
const sentiment = reasoner.analyzeSentiment("I love this product!");
const emotions = reasoner.detectEmotions("I'm so excited!");

// Planning
reasoner.setState("has_key", '{"type": "boolean", "value": true}');
const plan = reasoner.plan("unlock_door");
```

### TypeScript

```typescript
import PsychoSymbolicReasoner from '@psycho-symbolic/reasoner';

const reasoner = new PsychoSymbolicReasoner();

// Full type support
const capabilities = reasoner.capabilities();
console.log(capabilities.graphReasoning); // boolean
```

### Individual Modules

```javascript
// Import specific modules
import { GraphReasoner } from '@psycho-symbolic/reasoner/graph';
import { TextExtractor } from '@psycho-symbolic/reasoner/extractors';
import { PlannerSystem } from '@psycho-symbolic/reasoner/planner';

const graph = new GraphReasoner();
const extractor = new TextExtractor();
const planner = new PlannerSystem();
```

### Bundle Formats

The build process generates multiple bundle formats:

1. **ES Modules** (`.mjs`): For modern JavaScript environments
2. **CommonJS** (`.cjs`): For Node.js environments
3. **IIFE** (`.bundle.js`): For direct browser inclusion

## Troubleshooting

### Common Issues

#### 1. SystemTime Panics

**Problem**: `time not implemented on this platform` errors

**Solution**: The codebase includes WASM-compatible time functions:

```rust
#[cfg(target_arch = "wasm32")]
fn wasm_compatible_timestamp() -> u64 {
    use js_sys::Date;
    Date::now() as u64 / 1000
}

#[cfg(not(target_arch = "wasm32"))]
fn wasm_compatible_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
```

#### 2. Memory Issues

**Problem**: Out of memory during compilation

**Solutions**:
- Use development builds (`--dev`) for faster compilation
- Increase system memory or swap space
- Build crates individually instead of in parallel

#### 3. Module Loading Errors

**Problem**: Cannot load WASM modules in Node.js

**Solution**: Use `initSync` with buffer for Node.js:

```javascript
const wasmBuffer = fs.readFileSync('module_bg.wasm');
wasmModule.initSync(wasmBuffer);
```

#### 4. TypeScript Errors

**Problem**: Missing type definitions

**Solution**: Ensure TypeScript definitions are generated:
```bash
wasm-pack build --target web --typescript
```

### Debug Mode

For debugging, use development builds:

```bash
node build-wasm.cjs --dev
```

Development builds include:
- Debug symbols
- Unoptimized code
- Faster compilation
- Better error messages

### Logging

Enable detailed logging:

```bash
RUST_LOG=info node build-wasm.cjs
```

## Performance

### Optimization Strategies

1. **Release Builds**: Use `--release` for production
2. **wee_alloc**: Reduces WASM binary size
3. **Feature Flags**: Disable unused features
4. **Bundle Splitting**: Import only needed modules

### Benchmarks

Based on test results:

- **Graph Operations**: 22,000+ facts/second insertion
- **Text Analysis**: Real-time processing for typical inputs
- **Planning**: Sub-millisecond for simple scenarios

### Memory Usage

- **graph_reasoner**: ~200KB base WASM size
- **extractors**: ~150KB base WASM size
- **planner**: ~250KB base WASM size
- **Total Bundle**: ~600KB compressed

### Bundle Size Optimization

Production builds use:
- Dead code elimination
- wasm-opt optimization
- Gzip compression
- Tree shaking for unused exports

## Integration Examples

### Web Application

```html
<!DOCTYPE html>
<html>
<head>
    <script type="module">
        import PsychoSymbolicReasoner from './wasm-dist/index.js';

        async function init() {
            const reasoner = new PsychoSymbolicReasoner();

            // Example usage
            const sentiment = reasoner.analyzeSentiment("Hello world!");
            console.log('Sentiment:', sentiment);
        }

        init();
    </script>
</head>
<body>
    <h1>Psycho-Symbolic Reasoner Demo</h1>
</body>
</html>
```

### Node.js Application

```javascript
const PsychoSymbolicReasoner = require('./wasm-dist/index.cjs');

async function main() {
    const reasoner = new PsychoSymbolicReasoner();

    // Graph reasoning
    reasoner.addFact("user", "prefers", "coffee");

    // Text analysis
    const analysis = reasoner.analyzeText("I really enjoy good coffee!");

    console.log('Analysis:', JSON.parse(analysis));
}

main().catch(console.error);
```

### React Integration

```jsx
import React, { useEffect, useState } from 'react';
import PsychoSymbolicReasoner from '@psycho-symbolic/reasoner';

function App() {
    const [reasoner, setReasoner] = useState(null);
    const [result, setResult] = useState('');

    useEffect(() => {
        const initReasoner = async () => {
            const r = new PsychoSymbolicReasoner();
            setReasoner(r);
        };
        initReasoner();
    }, []);

    const analyzeSentiment = (text) => {
        if (reasoner) {
            const sentiment = reasoner.analyzeSentiment(text);
            setResult(sentiment);
        }
    };

    return (
        <div>
            <h1>Sentiment Analysis</h1>
            <button onClick={() => analyzeSentiment("I love this!")}>
                Analyze
            </button>
            <pre>{result}</pre>
        </div>
    );
}
```

## Contributing

### Adding New Features

1. **Rust Side**: Add functionality to appropriate crate
2. **WASM Bindings**: Expose via `#[wasm_bindgen]`
3. **Tests**: Add tests in `test-wasm.cjs`
4. **Documentation**: Update this guide

### Build System Changes

1. **Build Script**: Modify `build-wasm.cjs`
2. **Dependencies**: Update workspace `Cargo.toml`
3. **Configuration**: Adjust `wasm-pack.toml` files
4. **Testing**: Verify with `test-wasm.cjs`

---

## Support

For issues and questions:
- Check the troubleshooting section above
- Review build logs for specific errors
- Open an issue in the project repository
- Consult the Rust WASM book: https://rustwasm.github.io/book/

---

*This guide covers the complete WASM build pipeline. The system is designed to be robust, performant, and easy to use across different JavaScript environments.*