# RuVector-PGlite Implementation Plan

## 🎯 Executive Summary

Create **@ruvector/pglite** - a lightweight WASM-based vector database extension for [PGlite](https://github.com/electric-sql/pglite) that brings ruvector's vector capabilities to browsers, edge environments, and serverless platforms.

**Target**: ~500KB-1MB WASM bundle (vs full PostgreSQL extension)
**Use Cases**: Browser vector search, edge computing, serverless, local-first apps

## 📊 Current State Analysis

### ruvector-postgres (Existing)
- **Framework**: pgrx 0.12 (Rust → PostgreSQL extension)
- **Build**: `cdylib` → native .so/.dylib/.dll
- **Size**: Full PostgreSQL extension (~10-20MB)
- **Features**: 53+ SQL functions, SIMD, GNN, SPARQL, Hyperbolic embeddings
- **Target**: PostgreSQL 14-17

### PGlite (Target Platform)
- **Size**: 3MB gzipped WASM
- **PostgreSQL**: v16.3 compiled to WASM
- **Extensions**: Supports dynamic loading (pgvector confirmed working)
- **Platforms**: Browser, Node.js, Bun, Deno
- **Limitation**: Single-user/connection

## 🏗️ Architecture Design

### Three-Tier Strategy

```
┌─────────────────────────────────────────────────────────┐
│  ruvector-core (NEW)                                    │
│  - Shared vector types and operations                   │
│  - Platform-agnostic (no_std compatible)                │
│  - Used by both postgres and pglite variants            │
└─────────────────────────────────────────────────────────┘
              ▲                           ▲
              │                           │
┌─────────────┴─────────┐   ┌────────────┴──────────────┐
│ ruvector-postgres     │   │  ruvector-pglite (NEW)    │
│ - Full features       │   │  - Lightweight subset     │
│ - pgrx framework      │   │  - WASM target            │
│ - Native compilation  │   │  - pgrx + wasm32          │
│ - 53+ functions       │   │  - Essential functions    │
└───────────────────────┘   └───────────────────────────┘
```

### Feature Comparison Matrix

| Component | ruvector-postgres | ruvector-pglite |
|-----------|-------------------|-----------------|
| **Vector Types** |
| `vector` (f32) | ✅ | ✅ |
| `halfvec` (f16) | ✅ | ✅ |
| `binaryvec` | ✅ | ✅ |
| `sparsevec` | ✅ | ✅ (simplified) |
| `productvec` | ✅ | ❌ |
| `scalarvec` | ✅ | ❌ |
| **Distance Metrics** |
| L2 (Euclidean) | ✅ | ✅ |
| Cosine | ✅ | ✅ |
| Inner Product | ✅ | ✅ |
| L1 (Manhattan) | ✅ | ✅ |
| Hamming | ✅ | ✅ |
| Jaccard | ✅ | ❌ |
| **Indexing** |
| HNSW | ✅ Full | ✅ Lite (M=8, ef=32) |
| IVFFlat | ✅ | ❌ |
| Flat (brute-force) | ✅ | ✅ |
| **Quantization** |
| Binary | ✅ | ✅ |
| Scalar (SQ8) | ✅ | ✅ |
| Product (PQ) | ✅ | ❌ |
| **SIMD** |
| AVX-512 | ✅ | ❌ (WASM SIMD only) |
| AVX2 | ✅ | ❌ |
| NEON | ✅ | ❌ |
| WASM SIMD | ❌ | ✅ |
| **Advanced Features** |
| GNN (GCN/GraphSage) | ✅ | ❌ |
| SPARQL/Cypher | ✅ | ❌ |
| ReasoningBank | ✅ | ❌ |
| Hyperbolic space | ✅ | ❌ |
| Attention mechanisms | ✅ | ❌ |
| Routing (Tiny Dancer) | ✅ | ❌ |
| **Target Size** | 10-20MB | 500KB-1MB |

## 🛠️ Implementation Phases

### Phase 1: Core Extraction (Week 1)

**Goal**: Create `ruvector-core` with shared types

```rust
// crates/ruvector-core/
├── Cargo.toml          // no_std compatible
├── src/
│   ├── lib.rs
│   ├── types/
│   │   ├── vector.rs   // f32 vector
│   │   ├── halfvec.rs  // f16 vector
│   │   ├── binary.rs   // binary vector
│   │   └── sparse.rs   // sparse vector (COO format)
│   ├── distance/
│   │   ├── euclidean.rs
│   │   ├── cosine.rs
│   │   ├── inner.rs
│   │   ├── hamming.rs
│   │   └── traits.rs
│   ├── quantization/
│   │   ├── binary.rs
│   │   └── scalar.rs
│   └── simd/
│       ├── wasm.rs     // WASM SIMD intrinsics
│       └── dispatch.rs // Runtime dispatch
```

**Key Changes**:
- Extract from `ruvector-postgres/src/types/*`
- Make `no_std` compatible (with `alloc` feature)
- No PostgreSQL dependencies
- WASM SIMD support via `core::arch::wasm32`

### Phase 2: PGlite Extension (Week 2)

**Goal**: Create minimal PostgreSQL extension for WASM

```rust
// crates/ruvector-pglite/
├── Cargo.toml          // target: wasm32-unknown-unknown
├── src/
│   ├── lib.rs          // pgrx initialization
│   ├── types.rs        // PostgreSQL type wrappers
│   ├── distance.rs     // Distance SQL functions
│   ├── operators.rs    // <->, <=>, <#> operators
│   ├── index/
│   │   ├── hnsw_lite.rs  // Simplified HNSW (M=8)
│   │   └── flat.rs       // Brute-force fallback
│   └── quantization.rs   // Binary + Scalar only
```

**Cargo.toml Configuration**:
```toml
[package]
name = "ruvector-pglite"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
ruvector-core = { path = "../ruvector-core", default-features = false }
pgrx = { version = "0.12", default-features = false }
half = { version = "2.4", default-features = false }
serde = { version = "1.0", default-features = false, features = ["alloc"] }

[profile.release]
opt-level = "z"          # Optimize for size
lto = true               # Link-time optimization
codegen-units = 1        # Single codegen unit
panic = "abort"          # No unwinding
strip = true             # Strip symbols

[package.metadata.pgrx]
pg16 = "pg16"  # Match PGlite's PostgreSQL version
```

**Build Configuration** (`.cargo/config.toml`):
```toml
[target.wasm32-unknown-unknown]
rustflags = [
    "-C", "target-feature=+simd128",  # Enable WASM SIMD
    "-C", "opt-level=z",              # Size optimization
]
```

### Phase 3: WASM Build Pipeline (Week 2)

**Goal**: Automated WASM compilation

```bash
# scripts/build-pglite.sh
#!/bin/bash
set -e

echo "Building ruvector-pglite for WASM..."

# Install wasm32 target
rustup target add wasm32-unknown-unknown

# Build with pgrx for wasm32
cd crates/ruvector-pglite
cargo pgrx package --target wasm32-unknown-unknown --pg-version 16

# Output: target/wasm32-unknown-unknown/release/ruvector_pglite.wasm

# Optimize with wasm-opt (from binaryen)
wasm-opt -Oz \
  target/wasm32-unknown-unknown/release/ruvector_pglite.wasm \
  -o ../../npm/packages/pglite/dist/ruvector.wasm

echo "✅ WASM build complete: $(du -h ../../npm/packages/pglite/dist/ruvector.wasm)"
```

**GitHub Actions** (`.github/workflows/build-pglite.yml`):
```yaml
name: Build PGlite WASM Extension

on:
  push:
    tags: ['pglite-v*']

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Install pgrx
        run: cargo install --locked cargo-pgrx@0.12

      - name: Install binaryen (wasm-opt)
        run: sudo apt-get install -y binaryen

      - name: Build WASM
        run: ./scripts/build-pglite.sh

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ruvector-pglite-wasm
          path: npm/packages/pglite/dist/ruvector.wasm
```

### Phase 4: NPM Package (Week 3)

**Goal**: TypeScript wrapper for PGlite

```typescript
// npm/packages/pglite/
├── package.json
├── tsconfig.json
├── src/
│   ├── index.ts        // Main export
│   ├── types.ts        // TypeScript types
│   ├── loader.ts       // WASM loader
│   └── extension.ts    // PGlite extension wrapper
├── dist/
│   ├── ruvector.wasm   // Built artifact
│   ├── index.js        // Compiled JS
│   └── index.d.ts      // Type definitions
└── examples/
    ├── browser.html
    ├── node.js
    └── deno.ts
```

**package.json**:
```json
{
  "name": "@ruvector/pglite",
  "version": "0.1.0",
  "description": "Lightweight vector database for PGlite (WASM)",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "files": ["dist"],
  "keywords": ["pglite", "vector", "wasm", "embeddings", "similarity"],
  "peerDependencies": {
    "@electric-sql/pglite": "^0.2.0"
  },
  "devDependencies": {
    "@electric-sql/pglite": "^0.2.0",
    "typescript": "^5.0.0"
  }
}
```

**Extension Wrapper** (`src/extension.ts`):
```typescript
import type { Extension } from '@electric-sql/pglite';

// WASM binary embedded
import wasmBinary from '../dist/ruvector.wasm';

export const ruvector: Extension = {
  name: 'ruvector',
  setup: async (pg, context) => {
    // Load WASM extension
    const wasmModule = await WebAssembly.instantiate(wasmBinary);

    // Register with PGlite
    await pg.exec(`
      CREATE EXTENSION IF NOT EXISTS ruvector CASCADE;
    `);

    console.log('✅ RuVector extension loaded');
  }
};
```

**Usage Example** (`examples/browser.html`):
```html
<!DOCTYPE html>
<html>
<head>
  <script type="module">
    import { PGlite } from 'https://cdn.jsdelivr.net/npm/@electric-sql/pglite/dist/index.js';
    import { ruvector } from 'https://cdn.jsdelivr.net/npm/@ruvector/pglite/dist/index.js';

    const db = await PGlite.create({
      extensions: { ruvector }
    });

    // Create table with vector column
    await db.exec(`
      CREATE TABLE embeddings (
        id SERIAL PRIMARY KEY,
        content TEXT,
        embedding vector(384)
      );

      CREATE INDEX ON embeddings USING hnsw (embedding vector_cosine_ops);
    `);

    // Insert vectors
    const embedding = Array(384).fill(0).map(() => Math.random());
    await db.query(
      'INSERT INTO embeddings (content, embedding) VALUES ($1, $2)',
      ['Sample text', JSON.stringify(embedding)]
    );

    // Similarity search
    const results = await db.query(`
      SELECT content, embedding <=> $1 AS distance
      FROM embeddings
      ORDER BY distance
      LIMIT 5
    `, [JSON.stringify(embedding)]);

    console.log('Search results:', results.rows);
  </script>
</head>
<body>
  <h1>RuVector PGlite Demo</h1>
  <p>Check browser console for results</p>
</body>
</html>
```

### Phase 5: Testing & Optimization (Week 4)

**Test Suite**:
```rust
// crates/ruvector-pglite/tests/integration.rs
#[cfg(test)]
mod tests {
    use pgrx::pg_test;

    #[pg_test]
    fn test_vector_creation() {
        let vec = Spi::get_one::<Vec<f32>>(
            "SELECT '[1,2,3]'::vector"
        ).unwrap();
        assert_eq!(vec.len(), 3);
    }

    #[pg_test]
    fn test_cosine_distance() {
        let dist = Spi::get_one::<f32>(
            "SELECT '[1,0,0]'::vector <=> '[0,1,0]'::vector"
        ).unwrap();
        assert!((dist - 1.0).abs() < 0.001);
    }

    #[pg_test]
    fn test_hnsw_index() {
        Spi::run("
            CREATE TABLE items (id int, vec vector(3));
            INSERT INTO items VALUES (1, '[1,0,0]'), (2, '[0,1,0]');
            CREATE INDEX ON items USING hnsw (vec vector_cosine_ops);
        ").unwrap();
    }
}
```

**Size Optimization Checklist**:
- [ ] Use `opt-level = "z"` in release profile
- [ ] Enable LTO (Link-Time Optimization)
- [ ] Strip debug symbols
- [ ] Run `wasm-opt -Oz`
- [ ] Minimize dependencies (use `default-features = false`)
- [ ] Avoid large data structures in binary
- [ ] Use lazy initialization for indexes

## 📈 Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| WASM size | < 1MB | `du -h ruvector.wasm` |
| Load time (browser) | < 500ms | Performance API |
| Query latency (1k vectors) | < 10ms | Benchmark suite |
| Memory usage | < 50MB for 100k vectors | Chrome DevTools |
| Compatibility | Chrome 91+, Firefox 89+, Safari 16.4+ | Manual testing |

## 🚀 Deployment Strategy

### Publishing Flow

1. **Build WASM**: `./scripts/build-pglite.sh`
2. **Run tests**: `cargo test -p ruvector-pglite`
3. **Optimize**: `wasm-opt -Oz`
4. **Publish npm**: `npm publish --access public`
5. **Tag release**: `git tag pglite-v0.1.0 && git push --tags`

### CDN Distribution

```javascript
// Via unpkg
import { ruvector } from 'https://unpkg.com/@ruvector/pglite@latest/dist/index.js';

// Via jsDelivr
import { ruvector } from 'https://cdn.jsdelivr.net/npm/@ruvector/pglite@latest/dist/index.js';

// Via esm.sh
import { ruvector } from 'https://esm.sh/@ruvector/pglite@latest';
```

## 🎯 Use Cases Enabled

1. **In-Browser Semantic Search**
   - Chat interfaces with local embedding search
   - Document search without server round-trips
   - Privacy-first search (data never leaves browser)

2. **Edge Computing**
   - Cloudflare Workers with vector search
   - Deno Deploy with similarity matching
   - Vercel Edge Functions with embeddings

3. **Desktop Apps**
   - Electron apps with local vector DB
   - Tauri apps with Rust + WASM synergy
   - VS Code extensions with semantic code search

4. **Mobile Apps**
   - React Native with local vector search
   - Capacitor/Ionic with PGlite
   - Expo apps with offline-first embeddings

5. **Development/Testing**
   - No Docker required for local dev
   - Fast test suites with in-memory DB
   - Prototype vector apps in CodeSandbox/StackBlitz

## 🔄 Maintenance Plan

- **Weekly**: Monitor PGlite releases for compatibility
- **Monthly**: Sync features from ruvector-postgres
- **Quarterly**: Performance audits and size optimization
- **Yearly**: Major version alignment with PostgreSQL

## 📚 Documentation Plan

1. **README.md**: Quick start, installation, examples
2. **API.md**: Full API reference for all functions
3. **PERFORMANCE.md**: Benchmarks and optimization tips
4. **MIGRATION.md**: Guide for pgvector users
5. **CONTRIBUTING.md**: How to contribute to pglite variant

## 🤝 Collaboration Opportunities

- **PGlite Team**: Coordinate on extension API improvements
- **pgvector Team**: Ensure SQL compatibility
- **Transformers.js**: Integration examples for embeddings
- **LangChain**: Add @ruvector/pglite as vector store option

## ⚠️ Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| WASM size bloat | High | Aggressive optimization, feature gating |
| pgrx WASM support gaps | Medium | Fallback to manual FFI if needed |
| PGlite breaking changes | Medium | Pin to stable versions, monitor releases |
| Performance vs native | Low | Clear documentation of tradeoffs |

## 📅 Timeline

- **Week 1**: Core extraction, architecture setup
- **Week 2**: PGlite extension, WASM build pipeline
- **Week 3**: NPM package, TypeScript wrapper, examples
- **Week 4**: Testing, optimization, documentation
- **Week 5**: Beta release, community feedback
- **Week 6**: v1.0 launch

---

**Next Steps**: Ready to proceed? I can start with Phase 1 (Core Extraction) immediately.
