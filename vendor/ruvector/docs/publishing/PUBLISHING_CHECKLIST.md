# RuVector Publishing Checklist

**Generated**: 2026-01-18
**Version**: 0.1.32
**Status**: Pre-publication Review

This document tracks the readiness of all ruvector crates for publication to crates.io.

---

## Summary

| Category | Status | Notes |
|----------|--------|-------|
| Cargo.toml Metadata | NEEDS WORK | Missing keywords/categories on core crates |
| Documentation | GOOD | All core crates have READMEs |
| License | PASS | MIT license present and verified |
| CI/CD | PASS | 19 GitHub Actions workflows |
| Tests | PASS | Tests compile successfully |
| Pre-publish Dry Run | NEEDS WORK | Compilation error in SIMD code |

---

## 1. Cargo.toml Metadata Updates

### Workspace Configuration (/Cargo.toml)
- [x] Version: `0.1.32`
- [x] Edition: `2021`
- [x] Rust-version: `1.77`
- [x] License: `MIT`
- [x] Authors: `["Ruvector Team"]`
- [x] Repository: `https://github.com/ruvnet/ruvector`

### Core Crates - Metadata Status

#### ruvector-core
| Field | Status | Current Value |
|-------|--------|---------------|
| name | PASS | `ruvector-core` |
| version | PASS | workspace |
| description | PASS | "High-performance Rust vector database core with HNSW indexing" |
| readme | PASS | `README.md` |
| license | PASS | workspace (MIT) |
| repository | PASS | workspace |
| keywords | MISSING | Need to add |
| categories | MISSING | Need to add |
| documentation | MISSING | Need to add |
| homepage | MISSING | Need to add |

**Recommended additions**:
```toml
keywords = ["vector-database", "hnsw", "similarity-search", "embeddings", "simd"]
categories = ["database", "algorithms", "science"]
documentation = "https://docs.rs/ruvector-core"
homepage = "https://github.com/ruvnet/ruvector"
```

#### ruvector-graph
| Field | Status | Current Value |
|-------|--------|---------------|
| name | PASS | `ruvector-graph` |
| version | PASS | workspace |
| description | PASS | "Distributed Neo4j-compatible hypergraph database with SIMD optimization" |
| readme | PASS | `README.md` |
| keywords | MISSING | Need to add |
| categories | MISSING | Need to add |
| documentation | MISSING | Need to add |

**Recommended additions**:
```toml
keywords = ["graph-database", "cypher", "hypergraph", "neo4j", "distributed"]
categories = ["database", "data-structures", "algorithms"]
documentation = "https://docs.rs/ruvector-graph"
homepage = "https://github.com/ruvnet/ruvector"
```

#### ruvector-gnn
| Field | Status | Current Value |
|-------|--------|---------------|
| name | PASS | `ruvector-gnn` |
| version | PASS | workspace |
| description | PASS | "Graph Neural Network layer for Ruvector on HNSW topology" |
| readme | PASS | `README.md` |
| keywords | MISSING | Need to add |
| categories | MISSING | Need to add |

**Recommended additions**:
```toml
keywords = ["gnn", "graph-neural-network", "machine-learning", "hnsw", "embeddings"]
categories = ["science", "algorithms", "machine-learning"]
documentation = "https://docs.rs/ruvector-gnn"
homepage = "https://github.com/ruvnet/ruvector"
```

#### ruvector-mincut (GOOD)
| Field | Status | Current Value |
|-------|--------|---------------|
| name | PASS | `ruvector-mincut` |
| description | PASS | "World's first subpolynomial dynamic min-cut..." |
| keywords | PASS | `["graph", "minimum-cut", "network-analysis", "self-healing", "dynamic-graph"]` |
| categories | PASS | `["algorithms", "data-structures", "science", "mathematics", "simulation"]` |
| documentation | PASS | `https://docs.rs/ruvector-mincut` |
| homepage | PASS | `https://ruv.io` |

#### ruvector-attention (GOOD)
| Field | Status | Current Value |
|-------|--------|---------------|
| name | PASS | `ruvector-attention` |
| version | NOTE | `0.1.31` (not using workspace) |
| description | PASS | "Attention mechanisms for ruvector..." |
| keywords | PASS | `["attention", "machine-learning", "vector-search", "graph-attention"]` |
| categories | PASS | `["algorithms", "science"]` |

#### ruvector-sona (GOOD)
| Field | Status | Current Value |
|-------|--------|---------------|
| name | PASS | `ruvector-sona` |
| version | NOTE | `0.1.4` (not using workspace) |
| description | PASS | "Self-Optimizing Neural Architecture..." |
| keywords | PASS | `["neural", "learning", "lora", "llm", "adaptive"]` |
| categories | PASS | `["science", "algorithms", "wasm"]` |
| documentation | PASS | `https://docs.rs/sona` |
| homepage | PASS | `https://github.com/ruvnet/ruvector/tree/main/crates/sona` |
| license | PASS | `MIT OR Apache-2.0` |

#### ruvector-postgres (GOOD)
| Field | Status | Current Value |
|-------|--------|---------------|
| name | PASS | `ruvector-postgres` |
| version | NOTE | `2.0.0` (not using workspace) |
| description | PASS | "High-performance PostgreSQL vector database extension v2..." |
| keywords | PASS | `["postgresql", "vector-database", "embeddings", "pgvector", "hnsw"]` |
| categories | PASS | `["database", "science", "algorithms"]` |
| documentation | PASS | `https://docs.rs/ruvector-postgres` |
| homepage | PASS | `https://github.com/ruvnet/ruvector` |

#### ruvector-cli
| Field | Status | Current Value |
|-------|--------|---------------|
| name | PASS | `ruvector-cli` |
| description | PASS | "CLI and MCP server for Ruvector" |
| keywords | MISSING | Need to add |
| categories | MISSING | Need to add |

**Recommended additions**:
```toml
keywords = ["cli", "vector-database", "mcp", "ruvector", "command-line"]
categories = ["command-line-utilities", "database"]
documentation = "https://docs.rs/ruvector-cli"
homepage = "https://github.com/ruvnet/ruvector"
```

#### ruvector-filter
| Field | Status | Current Value |
|-------|--------|---------------|
| name | PASS | `ruvector-filter` |
| description | PASS | "Advanced metadata filtering for Ruvector vector search" |
| rust-version | MISSING | Need to add workspace |
| keywords | MISSING | Need to add |
| categories | MISSING | Need to add |

#### ruvector-collections
| Field | Status | Current Value |
|-------|--------|---------------|
| name | PASS | `ruvector-collections` |
| description | PASS | "High-performance collection management for Ruvector vector databases" |
| rust-version | MISSING | Need to add workspace |
| keywords | MISSING | Need to add |
| categories | MISSING | Need to add |

---

## 2. Documentation Status

### Crate READMEs
| Crate | README | Lines | Status |
|-------|--------|-------|--------|
| ruvector-core | Yes | 511 | GOOD |
| ruvector-graph | Yes | - | GOOD |
| ruvector-gnn | Yes | - | GOOD |
| ruvector-mincut | Yes | - | GOOD |
| ruvector-attention | Yes | - | GOOD |
| sona | Yes | - | GOOD |
| ruvector-postgres | Yes | - | GOOD |
| ruvector-cli | Yes | - | GOOD |

### Doc Comments
| Status | Notes |
|--------|-------|
| NEEDS WORK | 112 missing documentation warnings in ruvector-core |
| PRIORITY | Focus on public API documentation |

**Key areas needing docs**:
- `arena.rs` - Thread-local arena documentation
- `advanced/neural_hash.rs` - Struct field documentation
- Various public structs and functions

### ADR Documentation
| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Ruvector Core Architecture | Proposed |
| ADR-002 | RuvLLM Integration | Proposed |
| ADR-003 | SIMD Optimization Strategy | Proposed |
| ADR-004 | KV Cache Management | Proposed |
| ADR-005 | WASM Runtime Integration | Proposed |
| ADR-006 | Memory Management | Proposed |

---

## 3. Pre-publish Checks

### Cargo Publish Dry Run Results

#### ruvector-core
```
Status: FAILED
Error: cannot find function `euclidean_distance_neon_unrolled_impl`
Location: src/simd_intrinsics.rs:40
```

**Analysis**: The error occurs during verification of the packaged tarball on non-ARM64 systems. The code compiles correctly on ARM64 (Apple Silicon). This is a cross-compilation issue.

**Action Required**:
1. Ensure the simd_intrinsics.rs file has proper `#[cfg(...)]` guards for all platform-specific functions
2. The uncommitted changes in simd_intrinsics.rs need to be reviewed and committed
3. Test on multiple architectures before publish

### Compilation Status
| Crate | Status | Warnings |
|-------|--------|----------|
| ruvector-core | COMPILES | 112 warnings |
| Test compilation | PASS | Tests compile |

---

## 4. License Verification

### LICENSE File
| Field | Value | Status |
|-------|-------|--------|
| Location | `/LICENSE` | PASS |
| Type | MIT | PASS |
| Copyright | 2025 rUv | PASS |
| Format | Standard MIT | PASS |

### Dependency License Compatibility
| License | Compatible with MIT | Status |
|---------|---------------------|--------|
| MIT | Yes | PASS |
| Apache-2.0 | Yes | PASS |
| BSD-* | Yes | PASS |
| ISC | Yes | PASS |

**Note**: All workspace dependencies are compatible with MIT license.

---

## 5. CI/CD Workflows

### GitHub Actions (19 workflows)
| Workflow | Purpose | Status |
|----------|---------|--------|
| agentic-synth-ci.yml | Agentic synthesis CI | ACTIVE |
| benchmarks.yml | Performance benchmarks | ACTIVE |
| build-attention.yml | Attention crate builds | ACTIVE |
| build-gnn.yml | GNN crate builds | ACTIVE |
| build-graph-node.yml | Graph node builds | ACTIVE |
| build-native.yml | Native builds (all platforms) | ACTIVE |
| build-router.yml | Router builds | ACTIVE |
| build-tiny-dancer.yml | Tiny Dancer builds | ACTIVE |
| docker-publish.yml | Docker image publishing | ACTIVE |
| edge-net-models.yml | Edge network models | ACTIVE |
| hooks-ci.yml | Hooks CI testing | ACTIVE |
| postgres-extension-ci.yml | PostgreSQL extension CI | ACTIVE |
| publish-all.yml | Multi-crate publishing | ACTIVE |
| release.yml | Release automation | ACTIVE |
| ruvector-postgres-ci.yml | PostgreSQL crate CI | ACTIVE |
| ruvllm-build.yml | RuvLLM builds | ACTIVE |
| ruvllm-native.yml | RuvLLM native builds | ACTIVE |
| sona-napi.yml | SONA NAPI builds | ACTIVE |
| validate-lockfile.yml | Lockfile validation | ACTIVE |

---

## 6. CHANGELOG Status

### Current CHANGELOG.md
- Format: Keep a Changelog compliant
- Last documented version: `0.1.0` (2025-11-19)
- Unreleased section: Contains documentation updates

### Required Updates
- [ ] Add v0.1.32 release notes
- [ ] Document ADR-based architecture decisions
- [ ] Add AVX-512 SIMD optimization features (ADR-003)
- [ ] Document WASM runtime integration (ADR-005)
- [ ] Document memory management improvements (ADR-006)
- [ ] Add KV cache management features (ADR-004)

---

## 7. Action Items

### High Priority (Before Publish)

1. **Fix SIMD Compilation Issue**
   - Review uncommitted changes in `crates/ruvector-core/src/simd_intrinsics.rs`
   - Ensure proper `#[cfg(...)]` guards for cross-platform compilation
   - Commit or revert changes

2. **Add Missing Metadata**
   ```bash
   # Add to these crates:
   # - ruvector-core: keywords, categories, documentation, homepage
   # - ruvector-graph: keywords, categories, documentation
   # - ruvector-gnn: keywords, categories, documentation
   # - ruvector-cli: keywords, categories, documentation
   # - ruvector-filter: rust-version.workspace, keywords, categories
   # - ruvector-collections: rust-version.workspace, keywords, categories
   ```

3. **Version Alignment**
   - `ruvector-attention` uses `0.1.31` instead of workspace
   - `ruvector-sona` uses `0.1.4` instead of workspace
   - `ruvector-postgres` uses `2.0.0` instead of workspace
   - Decide: Keep independent versions or align to workspace?

### Medium Priority

4. **Documentation Improvements**
   - Address 112 missing documentation warnings
   - Add doc examples to public APIs
   - Run `cargo doc --no-deps` and fix any errors

5. **CHANGELOG Updates**
   - Add v0.1.32 section
   - Document ADR-based features

### Low Priority

6. **Test Coverage**
   - Run full test suite: `cargo test --workspace`
   - Ensure all tests pass before publish

7. **Clean Up Warnings**
   - Fix 18 unused import/variable warnings
   - Run `cargo fix` for auto-fixable issues

---

## 8. Publishing Order

When ready to publish, use this order (respecting dependencies):

```
1. ruvector-core (no internal deps)
2. ruvector-filter (depends on ruvector-core)
3. ruvector-collections (depends on ruvector-core)
4. ruvector-metrics (depends on ruvector-core)
5. ruvector-snapshot (depends on ruvector-core)
6. ruvector-graph (depends on ruvector-core)
7. ruvector-gnn (depends on ruvector-core)
8. ruvector-cluster (depends on ruvector-core)
9. ruvector-raft (depends on ruvector-core)
10. ruvector-replication (depends on ruvector-core, ruvector-raft)
11. ruvector-router-core (depends on ruvector-core)
12. ruvector-mincut (depends on ruvector-core, optional ruvector-graph)
13. ruvector-attention (depends on optional ruvector-math)
14. ruvector-sona (no ruvector deps)
15. ruvector-tiny-dancer-core (depends on ruvector-core, ruvector-router-core)
16. ruvector-dag (depends on ruvector-core, ruvector-attention, ruvector-mincut)
17. ruvector-server (depends on multiple crates)
18. ruvector-cli (depends on ruvector-core, ruvector-graph, ruvector-gnn)
19. Platform bindings (-node, -wasm variants) last
```

---

## 9. Commands Reference

```bash
# Verify a single crate
cargo publish --dry-run -p ruvector-core --allow-dirty

# Build documentation
cargo doc --no-deps -p ruvector-core

# Run tests
cargo test -p ruvector-core

# Check all crates compile
cargo check --workspace

# Fix auto-fixable warnings
cargo fix --workspace --allow-dirty

# Publish (when ready)
cargo publish -p ruvector-core
```

---

## Approval Checklist

Before publishing, confirm:

- [ ] All metadata fields added to crates
- [ ] SIMD compilation issue resolved
- [ ] Tests pass on all platforms
- [ ] Documentation builds without errors
- [ ] CHANGELOG updated
- [ ] Version numbers consistent
- [ ] Git working directory clean
- [ ] GitHub Actions CI passing

---

**Last Updated**: 2026-01-18
**Next Review**: Before v0.1.32 release
