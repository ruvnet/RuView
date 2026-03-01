# Ruvector Filter

[![Crates.io](https://img.shields.io/crates/v/ruvector-filter.svg)](https://crates.io/crates/ruvector-filter)
[![Documentation](https://docs.rs/ruvector-filter/badge.svg)](https://docs.rs/ruvector-filter)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)

**Advanced metadata filtering for Ruvector vector search.**

`ruvector-filter` provides a powerful filter expression language for combining vector similarity search with metadata constraints. Supports complex boolean expressions, range queries, and efficient filter evaluation. Part of the [Ruvector](https://github.com/ruvnet/ruvector) ecosystem.

## Why Ruvector Filter?

- **Rich Expressions**: Complex boolean filter expressions
- **Type-Safe**: Strongly typed filter operations
- **Optimized**: Filter pushdown for efficient evaluation
- **Extensible**: Custom filter operators
- **JSON Compatible**: Easy integration with JSON metadata

## Features

### Core Capabilities

- **Comparison Operators**: `=`, `!=`, `<`, `>`, `<=`, `>=`
- **Boolean Logic**: `AND`, `OR`, `NOT`
- **Range Queries**: `BETWEEN`, `IN`
- **String Matching**: `CONTAINS`, `STARTS_WITH`, `ENDS_WITH`
- **Null Handling**: `IS NULL`, `IS NOT NULL`

### Advanced Features

- **Nested Fields**: Filter on nested JSON properties
- **Array Operations**: `ANY`, `ALL`, `NONE` on arrays
- **Regex Matching**: Pattern-based string filtering
- **Geo Filters**: Distance and bounding box (planned)
- **Custom Functions**: Extensible filter functions

## Installation

Add `ruvector-filter` to your `Cargo.toml`:

```toml
[dependencies]
ruvector-filter = "0.1.1"
```

## Quick Start

### Basic Filtering

```rust
use ruvector_filter::{Filter, FilterBuilder};

// Build filter expression
let filter = FilterBuilder::new()
    .field("category").eq("electronics")
    .and()
    .field("price").lt(1000.0)
    .build()?;

// Apply to search
let results = db.search(SearchQuery {
    vector: query_vec,
    k: 10,
    filter: Some(filter),
    ..Default::default()
})?;
```

### Complex Expressions

```rust
use ruvector_filter::{Filter, FilterExpr, op};

// Complex boolean expression
let filter = op::and(vec![
    op::eq("status", "active"),
    op::or(vec![
        op::gt("priority", 5),
        op::in_("tags", vec!["urgent", "important"]),
    ]),
    op::not(op::eq("archived", true)),
]);

// Range query
let filter = op::and(vec![
    op::between("price", 100.0, 500.0),
    op::between("created_at", "2024-01-01", "2024-12-31"),
]);
```

### String Matching

```rust
use ruvector_filter::op;

// String operations
let filter = op::and(vec![
    op::contains("description", "machine learning"),
    op::starts_with("name", "Project"),
    op::regex("email", r".*@company\.com"),
]);
```

### Nested Field Access

```rust
use ruvector_filter::op;

// Access nested JSON fields
let filter = op::and(vec![
    op::eq("user.role", "admin"),
    op::gt("metadata.views", 1000),
    op::in_("settings.theme", vec!["dark", "light"]),
]);
```

## API Overview

### Core Types

```rust
// Filter expression
pub enum FilterExpr {
    // Comparison
    Eq(String, Value),
    Ne(String, Value),
    Lt(String, Value),
    Gt(String, Value),
    Le(String, Value),
    Ge(String, Value),

    // Boolean
    And(Vec<FilterExpr>),
    Or(Vec<FilterExpr>),
    Not(Box<FilterExpr>),

    // Range
    Between(String, Value, Value),
    In(String, Vec<Value>),

    // String
    Contains(String, String),
    StartsWith(String, String),
    EndsWith(String, String),
    Regex(String, String),

    // Null
    IsNull(String),
    IsNotNull(String),
}

// Filter builder
pub struct FilterBuilder { /* ... */ }
```

### Filter Operations

```rust
// Convenience functions in `op` module
pub mod op {
    pub fn eq(field: &str, value: impl Into<Value>) -> FilterExpr;
    pub fn ne(field: &str, value: impl Into<Value>) -> FilterExpr;
    pub fn lt(field: &str, value: impl Into<Value>) -> FilterExpr;
    pub fn gt(field: &str, value: impl Into<Value>) -> FilterExpr;
    pub fn le(field: &str, value: impl Into<Value>) -> FilterExpr;
    pub fn ge(field: &str, value: impl Into<Value>) -> FilterExpr;

    pub fn and(exprs: Vec<FilterExpr>) -> FilterExpr;
    pub fn or(exprs: Vec<FilterExpr>) -> FilterExpr;
    pub fn not(expr: FilterExpr) -> FilterExpr;

    pub fn between(field: &str, min: impl Into<Value>, max: impl Into<Value>) -> FilterExpr;
    pub fn in_(field: &str, values: Vec<impl Into<Value>>) -> FilterExpr;

    pub fn contains(field: &str, substring: &str) -> FilterExpr;
    pub fn starts_with(field: &str, prefix: &str) -> FilterExpr;
    pub fn ends_with(field: &str, suffix: &str) -> FilterExpr;
    pub fn regex(field: &str, pattern: &str) -> FilterExpr;
}
```

### Filter Evaluation

```rust
impl FilterExpr {
    pub fn evaluate(&self, metadata: &serde_json::Value) -> bool;
    pub fn optimize(&self) -> FilterExpr;
    pub fn to_json(&self) -> serde_json::Value;
    pub fn from_json(json: &serde_json::Value) -> Result<Self>;
}
```

## Performance Tips

1. **Put most selective filters first** in AND expressions
2. **Use IN instead of multiple OR** for equality checks
3. **Avoid regex when possible** - use prefix/suffix matching
4. **Index frequently filtered fields** in your metadata

## Related Crates

- **[ruvector-core](../ruvector-core/)** - Core vector database engine
- **[ruvector-collections](../ruvector-collections/)** - Collection management

## Documentation

- **[Main README](../../README.md)** - Complete project overview
- **[API Documentation](https://docs.rs/ruvector-filter)** - Full API reference
- **[GitHub Repository](https://github.com/ruvnet/ruvector)** - Source code

## License

**MIT License** - see [LICENSE](../../LICENSE) for details.

---

<div align="center">

**Part of [Ruvector](https://github.com/ruvnet/ruvector) - Built by [rUv](https://ruv.io)**

[![Star on GitHub](https://img.shields.io/github/stars/ruvnet/ruvector?style=social)](https://github.com/ruvnet/ruvector)

[Documentation](https://docs.rs/ruvector-filter) | [Crates.io](https://crates.io/crates/ruvector-filter) | [GitHub](https://github.com/ruvnet/ruvector)

</div>
