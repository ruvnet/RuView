# ruvector-crv

CRV (Coordinate Remote Viewing) protocol integration for ruvector.

Maps the 6-stage CRV signal line methodology to ruvector's subsystems:

| CRV Stage | Data Type | ruvector Component |
|-----------|-----------|-------------------|
| Stage I (Ideograms) | Gestalt primitives | Poincaré ball hyperbolic embeddings |
| Stage II (Sensory) | Textures, colors, temps | Multi-head attention vectors |
| Stage III (Dimensional) | Spatial sketches | GNN graph topology |
| Stage IV (Emotional) | AOL, intangibles | SNN temporal encoding |
| Stage V (Interrogation) | Signal line probing | Differentiable search |
| Stage VI (3D Model) | Composite model | MinCut partitioning |

## Quick Start

```rust
use ruvector_crv::{CrvConfig, CrvSessionManager, GestaltType, StageIData};

// Create session manager with default config (384 dimensions)
let config = CrvConfig::default();
let mut manager = CrvSessionManager::new(config);

// Create a session for a target coordinate
manager.create_session("session-001".to_string(), "1234-5678".to_string()).unwrap();

// Add Stage I ideogram data
let stage_i = StageIData {
    stroke: vec![(0.0, 0.0), (1.0, 0.5), (2.0, 1.0), (3.0, 0.5)],
    spontaneous_descriptor: "angular rising".to_string(),
    classification: GestaltType::Manmade,
    confidence: 0.85,
};

let embedding = manager.add_stage_i("session-001", &stage_i).unwrap();
assert_eq!(embedding.len(), 384);
```

## Architecture

The Poincaré ball embedding for Stage I gestalts encodes the hierarchical
gestalt taxonomy (root → manmade/natural/movement/energy/water/land) with
exponentially less distortion than Euclidean space.

For AOL (Analytical Overlay) separation, the spiking neural network temporal
encoding models signal-vs-noise discrimination: high-frequency spike bursts
correlate with AOL contamination, while sustained low-frequency patterns
indicate clean signal line data.

MinCut partitioning in Stage VI identifies natural cluster boundaries in the
accumulated session graph, separating distinct target aspects.

## Cross-Session Convergence

Multiple sessions targeting the same coordinate can be analyzed for
convergence — agreement between independent viewers strengthens the
signal validity:

```rust
// After adding data to multiple sessions for "1234-5678"...
let convergence = manager.find_convergence("1234-5678", 0.75).unwrap();
// convergence.scores contains similarity values for converging entries
```

## License

MIT OR Apache-2.0
