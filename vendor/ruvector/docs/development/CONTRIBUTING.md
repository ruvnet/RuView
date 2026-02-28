# Contributing to Ruvector

Thank you for your interest in contributing to Ruvector! This document provides guidelines and instructions for contributing.

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Getting Started](#getting-started)
3. [Development Setup](#development-setup)
4. [Code Style](#code-style)
5. [Testing](#testing)
6. [Pull Request Process](#pull-request-process)
7. [Commit Guidelines](#commit-guidelines)
8. [Documentation](#documentation)
9. [Performance](#performance)
10. [Community](#community)

## Code of Conduct

### Our Pledge

We pledge to make participation in our project a harassment-free experience for everyone, regardless of age, body size, disability, ethnicity, gender identity and expression, level of experience, nationality, personal appearance, race, religion, or sexual identity and orientation.

### Our Standards

**Positive behavior includes**:
- Using welcoming and inclusive language
- Being respectful of differing viewpoints
- Gracefully accepting constructive criticism
- Focusing on what is best for the community
- Showing empathy towards other community members

**Unacceptable behavior includes**:
- Trolling, insulting/derogatory comments, and personal attacks
- Public or private harassment
- Publishing others' private information without permission
- Other conduct which could reasonably be considered inappropriate

## Getting Started

### Prerequisites

- **Rust 1.77+**: Install from [rustup.rs](https://rustup.rs/)
- **Node.js 16+**: For Node.js bindings testing
- **Git**: For version control
- **cargo-nextest** (optional but recommended): `cargo install cargo-nextest`

### Fork and Clone

1. Fork the repository on GitHub
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/ruvector.git
   cd ruvector
   ```
3. Add upstream remote:
   ```bash
   git remote add upstream https://github.com/ruvnet/ruvector.git
   ```

## Development Setup

### Build the Project

```bash
# Build all crates
cargo build

# Build with optimizations
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Build specific crate
cargo build -p ruvector-core
```

### Run Tests

```bash
# Run all tests
cargo test

# Run tests with nextest (parallel, faster)
cargo nextest run

# Run specific test
cargo test test_hnsw_search

# Run with logging
RUST_LOG=debug cargo test

# Run benchmarks
cargo bench
```

### Check Code

```bash
# Format code
cargo fmt

# Check formatting without changes
cargo fmt -- --check

# Run clippy lints
cargo clippy --all-targets --all-features -- -D warnings

# Check all crates
cargo check --all-features
```

## Code Style

### Rust Style Guide

We follow the [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/) with these additions:

#### Naming Conventions

```rust
// Structs: PascalCase
struct VectorDatabase { }

// Functions: snake_case
fn insert_vector() { }

// Constants: SCREAMING_SNAKE_CASE
const MAX_DIMENSIONS: usize = 65536;

// Type parameters: Single uppercase letter or PascalCase
fn generic<T>() { }
fn generic<TMetric: DistanceMetric>() { }
```

#### Documentation

All public items must have doc comments:

```rust
/// A high-performance vector database.
///
/// # Examples
///
/// ```
/// use ruvector_core::VectorDB;
///
/// let db = VectorDB::new(DbOptions::default())?;
/// ```
pub struct VectorDB { }

/// Insert a vector into the database.
///
/// # Arguments
///
/// * `entry` - The vector entry to insert
///
/// # Returns
///
/// The ID of the inserted vector
///
/// # Errors
///
/// Returns `RuvectorError` if insertion fails
pub fn insert(&self, entry: VectorEntry) -> Result<VectorId> {
    // ...
}
```

#### Error Handling

- Use `Result<T, RuvectorError>` for fallible operations
- Use `thiserror` for error types
- Provide context with error messages

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuvectorError {
    #[error("Vector dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

#### Performance

- Use `#[inline]` for hot path functions
- Profile before optimizing
- Document performance characteristics

```rust
/// Distance calculation (hot path, inlined)
#[inline]
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    // SIMD-optimized implementation
}
```

### TypeScript/JavaScript Style

For Node.js bindings:

```typescript
// Use TypeScript for type safety
interface VectorEntry {
    id?: string;
    vector: Float32Array;
    metadata?: Record<string, any>;
}

// Async/await for async operations
async function search(query: Float32Array): Promise<SearchResult[]> {
    return await db.search({ vector: query, k: 10 });
}

// Use const/let, never var
const db = new VectorDB(options);
let results = await db.search(query);
```

## Testing

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_insert() {
        // Arrange
        let db = VectorDB::new(DbOptions::default()).unwrap();
        let entry = VectorEntry {
            id: None,
            vector: vec![0.1; 128],
            metadata: None,
        };

        // Act
        let id = db.insert(entry).unwrap();

        // Assert
        assert!(!id.is_empty());
    }

    #[test]
    fn test_error_handling() {
        let db = VectorDB::new(DbOptions::default()).unwrap();
        let wrong_dims = vec![0.1; 64]; // Wrong dimensions

        let result = db.insert(VectorEntry {
            id: None,
            vector: wrong_dims,
            metadata: None,
        });

        assert!(result.is_err());
    }
}
```

### Property-Based Testing

Use `proptest` for property-based tests:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_distance_symmetry(
        a in prop::collection::vec(any::<f32>(), 128),
        b in prop::collection::vec(any::<f32>(), 128)
    ) {
        let d1 = euclidean_distance(&a, &b);
        let d2 = euclidean_distance(&b, &a);
        assert!((d1 - d2).abs() < 1e-5);
    }
}
```

### Benchmarking

Use `criterion` for benchmarks:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_search(c: &mut Criterion) {
    let db = setup_db();
    let query = vec![0.1; 128];

    c.bench_function("search 1M vectors", |b| {
        b.iter(|| {
            db.search(black_box(&SearchQuery {
                vector: query.clone(),
                k: 10,
                filter: None,
                include_vectors: false,
            }))
        })
    });
}

criterion_group!(benches, benchmark_search);
criterion_main!(benches);
```

### Test Coverage

Aim for:
- **Unit tests**: 80%+ coverage
- **Integration tests**: All major features
- **Property tests**: Core algorithms
- **Benchmarks**: Performance-critical paths

## Pull Request Process

### Before Submitting

1. **Create an issue** first for major changes
2. **Fork and branch**: Create a feature branch
   ```bash
   git checkout -b feature/my-new-feature
   ```
3. **Write tests**: Ensure new code has tests
4. **Run checks**:
   ```bash
   cargo fmt
   cargo clippy --all-targets --all-features -- -D warnings
   cargo test
   cargo bench
   ```
5. **Update documentation**: Update relevant docs
6. **Add changelog entry**: Update CHANGELOG.md

### PR Template

```markdown
## Description

Brief description of changes

## Motivation

Why is this change needed?

## Changes

- Change 1
- Change 2

## Testing

How was this tested?

## Performance Impact

Any performance implications?

## Checklist

- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] Changelog updated
- [ ] Code formatted (`cargo fmt`)
- [ ] Lints passing (`cargo clippy`)
- [ ] All tests passing (`cargo test`)
```

### Review Process

1. **Automated checks**: CI must pass
2. **Code review**: At least one maintainer approval
3. **Discussion**: Address reviewer feedback
4. **Merge**: Squash and merge or rebase

## Commit Guidelines

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types**:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Test additions/changes
- `chore`: Build process or auxiliary tool changes

**Examples**:

```
feat(hnsw): add parallel index construction

Implement parallel HNSW construction using rayon for faster
index building on multi-core systems.

- Split graph construction across threads
- Use atomic operations for thread-safe updates
- Achieve 4x speedup on 8-core system

Closes #123
```

```
fix(quantization): correct product quantization distance calculation

The distance calculation was not using precomputed lookup tables,
causing incorrect results.

Fixes #456
```

### Commit Hygiene

- One logical change per commit
- Write clear, descriptive messages
- Reference issues/PRs when applicable
- Keep commits focused and atomic

## Documentation

### Code Documentation

- **Public APIs**: Comprehensive rustdoc comments
- **Examples**: Include usage examples in doc comments
- **Safety**: Document unsafe code thoroughly
- **Panics**: Document panic conditions

### User Documentation

Update relevant docs:
- **README.md**: Overview and quick start
- **guides/**: User guides and tutorials
- **api/**: API reference documentation
- **CHANGELOG.md**: User-facing changes

### Documentation Style

```rust
/// A vector database with HNSW indexing.
///
/// `VectorDB` provides fast approximate nearest neighbor search using
/// Hierarchical Navigable Small World (HNSW) graphs. It supports:
///
/// - Sub-millisecond query latency
/// - 95%+ recall with proper tuning
/// - Memory-mapped storage for large datasets
/// - Multiple distance metrics (Euclidean, Cosine, etc.)
///
/// # Examples
///
/// ```
/// use ruvector_core::{VectorDB, VectorEntry, DbOptions};
///
/// let mut options = DbOptions::default();
/// options.dimensions = 128;
///
/// let db = VectorDB::new(options)?;
///
/// let entry = VectorEntry {
///     id: None,
///     vector: vec![0.1; 128],
///     metadata: None,
/// };
///
/// let id = db.insert(entry)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Performance
///
/// - Search: O(log n) with HNSW
/// - Insert: O(log n) amortized
/// - Memory: ~640 bytes per vector (M=32)
pub struct VectorDB { }
```

## Performance

### Performance Guidelines

1. **Profile first**: Use `cargo flamegraph` or `perf`
2. **Measure impact**: Benchmark before/after
3. **Document trade-offs**: Explain performance vs. other concerns
4. **Use SIMD**: Leverage SIMD intrinsics for hot paths
5. **Avoid allocations**: Reuse buffers in hot loops

### Benchmarking Changes

```bash
# Benchmark baseline
git checkout main
cargo bench -- --save-baseline main

# Benchmark your changes
git checkout feature-branch
cargo bench -- --baseline main
```

### Performance Checklist

- [ ] Profiled hot paths
- [ ] Benchmarked changes
- [ ] No performance regressions
- [ ] Documented performance characteristics
- [ ] Considered memory usage

## Community

### Getting Help

- **GitHub Issues**: Bug reports and feature requests
- **Discussions**: Questions and general discussion
- **Pull Requests**: Code contributions

### Reporting Bugs

Use the bug report template:

```markdown
**Describe the bug**
Clear description of the bug

**To Reproduce**
1. Step 1
2. Step 2
3. See error

**Expected behavior**
What you expected to happen

**Environment**
- OS: [e.g., Ubuntu 22.04]
- Rust version: [e.g., 1.77.0]
- Ruvector version: [e.g., 0.1.0]

**Additional context**
Any other relevant information
```

### Feature Requests

Use the feature request template:

```markdown
**Is your feature request related to a problem?**
Clear description of the problem

**Describe the solution you'd like**
What you want to happen

**Describe alternatives you've considered**
Other solutions you've thought about

**Additional context**
Any other relevant information
```

## License

By contributing to Ruvector, you agree that your contributions will be licensed under the MIT License.

## Questions?

Feel free to open an issue or discussion if you have questions about contributing!

---

Thank you for contributing to Ruvector! 🚀
