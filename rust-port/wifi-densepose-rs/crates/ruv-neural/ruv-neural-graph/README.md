# ruv-neural-graph

Brain connectivity graph construction from neural signals with graph-theoretic
analysis and spectral properties.

## Overview

`ruv-neural-graph` builds brain connectivity graphs from multi-channel neural
time series data and connectivity matrices. It provides graph-theoretic metrics
(efficiency, clustering, centrality), spectral graph properties (Laplacian,
Fiedler value), brain atlas definitions, petgraph interoperability, and temporal
dynamics tracking for brain topology research.

## Features

- **Graph construction** (`constructor`): Build `BrainGraph` instances from
  connectivity matrices and multi-channel time series data via `BrainGraphConstructor`
- **Brain atlases** (`atlas`): Built-in Desikan-Killiany 68-region atlas with
  support for loading custom atlas definitions
- **Graph metrics** (`metrics`): Global efficiency, local efficiency, clustering
  coefficient, betweenness centrality, degree distribution, modularity,
  graph density, small-world index
- **Spectral analysis** (`spectral`): Graph Laplacian, normalized Laplacian,
  Fiedler value (algebraic connectivity), spectral gap
- **Petgraph bridge** (`petgraph_bridge`): Bidirectional conversion between
  `BrainGraph` and petgraph `Graph` types
- **Temporal dynamics** (`dynamics`): `TopologyTracker` for monitoring graph
  property evolution over time

## Usage

```rust
use ruv_neural_graph::{
    BrainGraphConstructor, load_atlas, AtlasType,
    global_efficiency, clustering_coefficient, modularity,
    fiedler_value, graph_laplacian,
    to_petgraph, from_petgraph,
    TopologyTracker,
};

// Construct a brain graph from a connectivity matrix
let constructor = BrainGraphConstructor::new();
let graph = constructor.from_matrix(&connectivity_matrix, 0.3, atlas)?;

// Compute graph-theoretic metrics
let efficiency = global_efficiency(&graph);
let clustering = clustering_coefficient(&graph);
let mod_score = modularity(&graph);

// Spectral properties
let laplacian = graph_laplacian(&graph);
let fiedler = fiedler_value(&graph);

// Convert to petgraph for additional algorithms
let pg = to_petgraph(&graph);
let brain_graph = from_petgraph(&pg);

// Track topology over time
let mut tracker = TopologyTracker::new();
tracker.update(&graph);
```

## API Reference

| Module            | Key Types / Functions                                             |
|-------------------|-------------------------------------------------------------------|
| `constructor`     | `BrainGraphConstructor`                                           |
| `atlas`           | `load_atlas`, `AtlasType`                                         |
| `metrics`         | `global_efficiency`, `local_efficiency`, `clustering_coefficient`, `betweenness_centrality`, `modularity`, `small_world_index` |
| `spectral`        | `graph_laplacian`, `normalized_laplacian`, `fiedler_value`, `spectral_gap` |
| `petgraph_bridge` | `to_petgraph`, `from_petgraph`                                    |
| `dynamics`        | `TopologyTracker`                                                 |

## Integration

Depends on `ruv-neural-core` for `BrainGraph` and atlas types, and on
`ruv-neural-signal` for connectivity computation. Feeds graphs into
`ruv-neural-mincut` for topology partitioning and into `ruv-neural-viz`
for visualization. Uses `petgraph` for underlying graph data structures.

## License

MIT OR Apache-2.0
