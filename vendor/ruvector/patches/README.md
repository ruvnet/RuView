# Patches Directory

**CRITICAL: Do not delete this directory or its contents!**

This directory contains patched versions of external crates that are necessary for building RuVector.

## hnsw_rs

The `hnsw_rs` directory contains a patched version of the [hnsw_rs](https://crates.io/crates/hnsw_rs) crate.

### Why this patch exists

The official hnsw_rs crate uses `rand 0.9` which is **incompatible with WebAssembly (WASM)** builds. This patched version:

1. Uses `rand 0.8` instead of `rand 0.9` for WASM compatibility
2. Uses Rust edition 2021 (not 2024) for stable Rust toolchain compatibility

### How it's used

The patch is applied via `Cargo.toml` at the workspace root:

```toml
[patch.crates-io]
hnsw_rs = { path = "./patches/hnsw_rs" }
```

This ensures all workspace crates that depend on `hnsw_rs` use this patched version.

### What depends on it

- `ruvector-core` (with `hnsw` feature enabled by default)
- `ruvector-graph` (with `hnsw_rs` feature)
- All native builds (Node.js bindings, CLI tools)

### Consequences of deletion

If this directory is deleted:
- **All CI builds will fail** (Build Native Modules, PostgreSQL Extension CI, etc.)
- `cargo build` will fail with "failed to load source for dependency `hnsw_rs`"
- The project cannot be compiled

### Updating the patch

If you need to update hnsw_rs:
1. Download the new version from crates.io
2. Apply the rand 0.8 compatibility changes from the current patch
3. Test WASM and native builds before committing
