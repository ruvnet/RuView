# exo-hypergraph

Hypergraph substrate for higher-order relational reasoning with persistent
homology and sheaf theory. Enables cognitive representations that go beyond
pairwise edges to capture n-ary relationships natively.

## Features

- **Hyperedge storage** -- first-class support for edges that connect
  arbitrary sets of nodes, stored in a compressed sparse format.
- **Sheaf sections** -- attach typed data (sections) to nodes and edges
  with consistency conditions enforced by sheaf restriction maps.
- **Sparse persistent homology (PPR-based O(n/epsilon))** -- computes
  topological features efficiently using personalised PageRank
  sparsification.
- **Betti number computation** -- extracts Betti-0 (connected components),
  Betti-1 (loops), and higher Betti numbers to summarise structural
  topology.

## Quick Start

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
exo-hypergraph = "0.1"
```

Basic usage:

```rust
use exo_hypergraph::{HypergraphSubstrate, HypergraphConfig};
use exo_core::{EntityId, Relation, RelationType};

let config = HypergraphConfig::default();
let mut hg = HypergraphSubstrate::new(config);

let e1 = EntityId::new();
let e2 = EntityId::new();
let e3 = EntityId::new();

// Create a 3-way hyperedge
let relation = Relation {
    relation_type: RelationType::new("collaboration"),
    properties: serde_json::json!({"project": "EXO-AI"}),
};
hg.create_hyperedge(&[e1, e2, e3], &relation).unwrap();

// Compute topological invariants
let betti = hg.betti_numbers(2);
println!("Betti numbers: {:?}", betti);
```

## Crate Layout

| Module      | Purpose                                    |
|-------------|--------------------------------------------|
| `graph`     | Core hypergraph data structure              |
| `sheaf`     | Sheaf sections and restriction maps         |
| `homology`  | Sparse persistent homology pipeline         |
| `betti`     | Betti number extraction and summarisation   |

## Requirements

- Rust 1.78+
- Depends on `exo-core`

## Links

- [GitHub](https://github.com/ruvnet/ruvector)
- [EXO-AI Documentation](https://github.com/ruvnet/ruvector/tree/main/examples/exo-ai-2025)

## License

MIT OR Apache-2.0
