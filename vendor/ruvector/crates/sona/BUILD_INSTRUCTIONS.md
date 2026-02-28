# SONA WASM Build Instructions

## Prerequisites

1. Install Rust and wasm32 target:
```bash
rustup target add wasm32-unknown-unknown
```

2. Install wasm-pack (recommended):
```bash
cargo install wasm-pack
```

## Building for WASM

### Option 1: Using wasm-pack (Recommended)

```bash
cd crates/sona

# For web (browser)
wasm-pack build --target web --features wasm --out-dir wasm-example/pkg

# For Node.js
wasm-pack build --target nodejs --features wasm

# For bundlers (webpack, rollup, etc.)
wasm-pack build --target bundler --features wasm

# Release build (optimized)
wasm-pack build --target web --features wasm --release --out-dir wasm-example/pkg
```

### Option 2: Using cargo directly

```bash
cd crates/sona
cargo build --target wasm32-unknown-unknown --features wasm --release
```

The WASM file will be at: `../../target/wasm32-unknown-unknown/release/sona.wasm`

## Running the Example

1. Build the WASM module:
```bash
cd crates/sona
wasm-pack build --target web --features wasm --out-dir wasm-example/pkg
```

2. Serve the example:
```bash
cd wasm-example
python3 -m http.server 8080
# Or use any static server
```

3. Open browser:
```
http://localhost:8080
```

## File Structure

After building, you'll have:

```
crates/sona/
├── src/
│   ├── lib.rs           # Main library
│   ├── wasm.rs          # WASM bindings
│   ├── engine.rs        # SONA engine
│   ├── lora.rs          # LoRA implementations
│   ├── trajectory.rs    # Trajectory tracking
│   ├── ewc.rs           # EWC++ implementation
│   ├── reasoning_bank.rs # Pattern storage
│   ├── types.rs         # Core types
│   └── loops/           # Learning loops
├── wasm-example/
│   ├── index.html       # Demo page
│   ├── index.js         # Demo logic
│   ├── package.json     # NPM config
│   └── pkg/             # Generated WASM package
│       ├── sona.js      # JS bindings
│       ├── sona_bg.wasm # WASM binary
│       ├── sona.d.ts    # TypeScript definitions
│       └── package.json # NPM package info
└── Cargo.toml           # Rust config
```

## Optimizing Build Size

### 1. Use release profile
```bash
wasm-pack build --target web --features wasm --release
```

### 2. Enable wasm-opt (automatically done by wasm-pack)
The `wasm-release` profile in Cargo.toml is optimized for size:
```toml
[profile.wasm-release]
inherits = "release"
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization
panic = "abort"      # Smaller panic handler
```

### 3. Use wasm-snip to remove panicking infrastructure
```bash
cargo install wasm-snip
wasm-snip target/wasm32-unknown-unknown/release/sona.wasm \
  -o sona_snipped.wasm
```

## Troubleshooting

### Build Errors

**Error: `getrandom` not found**
- Solution: Make sure the `wasm` feature is enabled, which includes `getrandom` with `js` feature.

**Error: Missing `wasm-bindgen`**
- Solution: Add `wasm-bindgen` to dependencies with the `wasm` feature.

### Runtime Errors

**Error: Memory allocation failed**
- Solution: Increase WASM memory limit in your environment.

**Error: Module not found**
- Solution: Make sure paths in `index.html` correctly point to `pkg/sona.js`.

## Performance Tips

1. **Use release builds** in production for better performance
2. **Enable SIMD** if targeting modern browsers (requires additional features)
3. **Lazy load** the WASM module to improve initial page load
4. **Use Web Workers** for heavy computations to avoid blocking UI

## NPM Publishing

To publish the WASM package to NPM:

```bash
cd crates/sona
wasm-pack build --target bundler --features wasm --release
wasm-pack publish
```

## Size Comparison

- **Debug build**: ~9MB
- **Release build**: ~2-3MB
- **Release + wasm-opt**: ~1-2MB
- **With all optimizations**: < 1MB

## Browser Compatibility

- **Chrome/Edge**: 91+ (full support)
- **Firefox**: 89+ (full support)
- **Safari**: 14.1+ (full support)
- **Node.js**: 16+ (with `--experimental-wasm-modules`)

## Next Steps

- See [README.md](./README.md) for API documentation
- Check [wasm-example/](./wasm-example/) for usage examples
- Read [API Reference](./docs/API.md) for detailed API docs
