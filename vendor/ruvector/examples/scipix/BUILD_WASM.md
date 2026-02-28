# Building WebAssembly Module

## Prerequisites

```bash
# Install wasm-pack
cargo install wasm-pack

# Or use npm
npm install -g wasm-pack
```

## Build Commands

### Production Build (Optimized)
```bash
cd /home/user/ruvector/examples/scipix

wasm-pack build \
  --target web \
  --out-dir web/pkg \
  --release \
  -- --features wasm

# Or use the provided script
./web/build.sh
```

### Development Build (Faster, with debug info)
```bash
wasm-pack build \
  --target web \
  --out-dir web/pkg \
  --dev \
  -- --features wasm

# Or
npm run build:dev
```

## Build Output

The build creates:
```
web/pkg/
├── ruvector_scipix.js          # JavaScript bindings
├── ruvector_scipix_bg.wasm     # WASM binary (~800KB gzipped)
├── ruvector_scipix.d.ts        # TypeScript definitions
└── package.json                 # Package metadata
```

## Run Demo

### Simple HTTP Server
```bash
cd web
python3 -m http.server 8080
```

### Using the Build Script
```bash
./web/build.sh --serve
```

### Open in Browser
Navigate to: http://localhost:8080/example.html

## Integration

### In Your Project

#### Install (if published to npm)
```bash
npm install ruvector-scipix-wasm
```

#### Or Copy Files
```bash
cp -r web/pkg your-project/src/wasm/
```

#### Import in JavaScript
```javascript
import { createScipix } from './pkg/ruvector_scipix.js';

const scipix = await createScipix();
const result = await scipix.recognize(imageData);
```

## Troubleshooting

### Build Fails: "wasm32-unknown-unknown not installed"
```bash
rustup target add wasm32-unknown-unknown
```

### Build Fails: Missing dependencies
```bash
# Update Cargo.toml with WASM dependencies
cargo update
```

### CORS Errors in Browser
Ensure you're serving files with proper CORS headers:
```bash
# Use a CORS-enabled server
npm install -g http-server
http-server web -p 8080 --cors
```

### Large Bundle Size
The release build should be optimized. Check:
```bash
# Verify optimization settings in Cargo.toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
```

## Size Optimization

Current optimizations applied:
- ✅ Size optimization (`opt-level = "z"`)
- ✅ LTO enabled
- ✅ Single codegen unit
- ✅ Debug symbols stripped
- ✅ wee_alloc (custom allocator)
- ✅ Panic = abort

Expected sizes:
- Raw WASM: ~1.5MB
- Gzipped: ~800KB
- With Brotli: ~600KB

## Advanced Options

### Custom Features
```bash
# Build with specific features
wasm-pack build --features "wasm,preprocess"

# No default features
wasm-pack build --no-default-features --features wasm
```

### Target Specific Browsers
```bash
# Modern browsers only
wasm-pack build --target web

# For bundlers (Webpack, Rollup)
wasm-pack build --target bundler

# For Node.js
wasm-pack build --target nodejs
```

### Profile Build Time
```bash
cargo build --timings --release --target wasm32-unknown-unknown --features wasm
```

## CI/CD Integration

### GitHub Actions
```yaml
- name: Install wasm-pack
  run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

- name: Build WASM
  run: |
    cd examples/scipix
    wasm-pack build --target web --out-dir web/pkg --release -- --features wasm

- name: Upload artifact
  uses: actions/upload-artifact@v3
  with:
    name: wasm-build
    path: examples/scipix/web/pkg/
```

## Next Steps

1. Build the WASM module
2. Test with the demo HTML
3. Integrate into your application
4. Deploy to production

## References

- [wasm-pack Documentation](https://rustwasm.github.io/wasm-pack/)
- [wasm-bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)
- [Rust WASM Book](https://rustwasm.github.io/docs/book/)
