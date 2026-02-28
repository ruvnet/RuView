# API Reference

## Core Types

### MinCutWrapper
Primary interface for dynamic minimum cut.

```rust
use ruvector_mincut::{MinCutWrapper, DynamicGraph};
use std::sync::Arc;

// Create wrapper
let graph = Arc::new(DynamicGraph::new());
let mut wrapper = MinCutWrapper::new(graph);

// Handle updates
wrapper.insert_edge(edge_id, u, v);
wrapper.delete_edge(edge_id, u, v);

// Query minimum cut
match wrapper.query() {
    MinCutResult::Disconnected => println!("Min cut: 0"),
    MinCutResult::Value { cut_value, witness } => {
        println!("Min cut: {}", cut_value);
    }
}
```

### ProperCutInstance Trait
Interface for bounded-range instances.

```rust
pub trait ProperCutInstance: Send + Sync {
    fn init(graph: &DynamicGraph, lambda_min: u64, lambda_max: u64) -> Self;
    fn apply_inserts(&mut self, edges: &[(EdgeId, VertexId, VertexId)]);
    fn apply_deletes(&mut self, edges: &[(EdgeId, VertexId, VertexId)]);
    fn query(&self) -> InstanceResult;
    fn bounds(&self) -> (u64, u64);
}

pub enum InstanceResult {
    ValueInRange { value: u64, witness: WitnessHandle },
    AboveRange,
}
```

### WitnessHandle
Compact representation of a cut.

```rust
pub struct WitnessHandle {
    // Arc-based for cheap cloning
}

impl WitnessHandle {
    pub fn contains(&self, v: VertexId) -> bool;
    pub fn boundary_size(&self) -> u64;
    pub fn seed(&self) -> VertexId;
    pub fn cardinality(&self) -> u64;
    pub fn materialize_partition(&self) -> (HashSet<VertexId>, HashSet<VertexId>);
}
```

### LocalKCutOracle
Deterministic local minimum cut oracle.

```rust
pub trait LocalKCutOracle: Send + Sync {
    fn search(&self, graph: &DynamicGraph, query: LocalKCutQuery) -> LocalKCutResult;
}

pub struct LocalKCutQuery {
    pub seed_vertices: Vec<VertexId>,
    pub budget_k: u64,
    pub radius: usize,
}

pub enum LocalKCutResult {
    Found { witness: WitnessHandle, cut_value: u64 },
    NoneInLocality,
}
```

### CutCertificate
Verifiable certificate for minimum cut.

```rust
pub struct CutCertificate {
    pub witnesses: Vec<WitnessSummary>,
    pub localkcut_responses: Vec<LocalKCutResponse>,
    pub best_witness_idx: Option<usize>,
    pub timestamp: SystemTime,
    pub version: u32,
}

impl CutCertificate {
    pub fn verify(&self) -> Result<(), CertificateError>;
    pub fn to_json(&self) -> String;
}
```

## Examples

See `/examples/` directory for complete examples.
