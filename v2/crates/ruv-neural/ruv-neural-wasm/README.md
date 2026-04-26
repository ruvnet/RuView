# ruv-neural-wasm

WebAssembly bindings for browser-based brain topology visualization.

## Overview

`ruv-neural-wasm` provides JavaScript-callable functions for creating, analyzing,
and visualizing brain connectivity graphs directly in the browser. It wraps
`ruv-neural-core` types with `wasm-bindgen` and implements lightweight
WASM-compatible versions of graph algorithms (Stoer-Wagner mincut, spectral
embedding via power iteration, topology metrics, and cognitive state decoding)
that run without heavy native dependencies.

**Note:** This crate is excluded from the default workspace build. Build it
separately targeting `wasm32-unknown-unknown`.

## Features

- **Graph parsing**: `create_brain_graph` -- parse `BrainGraph` from JSON
- **Minimum cut**: `compute_mincut` -- Stoer-Wagner on graphs up to 500 nodes
- **Topology metrics**: `compute_topology_metrics` -- density, efficiency,
  modularity, Fiedler value, entropy, module count
- **Spectral embedding**: `embed_graph` -- power iteration on normalized Laplacian
  (no LAPACK dependency)
- **State decoding**: `decode_state` -- threshold-based cognitive state classification
  from topology metrics
- **RVF I/O**: `load_rvf` / `export_rvf` -- read and write RuVector binary files
- **Streaming** (`streaming`): WebSocket-compatible streaming data processor
- **Visualization data** (`viz_data`): Data structures for D3.js and Three.js rendering

## Build

```bash
# Requires wasm-pack or cargo with wasm32 target
cargo build -p ruv-neural-wasm --target wasm32-unknown-unknown --release

# Or with wasm-pack for npm-ready output
wasm-pack build ruv-neural-wasm --target web
```

## Usage (JavaScript)

```javascript
import init, {
  create_brain_graph,
  compute_mincut,
  compute_topology_metrics,
  embed_graph,
  decode_state,
  export_rvf,
  version,
} from './ruv_neural_wasm.js';

await init();

const graphJson = JSON.stringify({
  num_nodes: 3,
  edges: [
    { source: 0, target: 1, weight: 0.8, metric: "Coherence", frequency_band: "Alpha" },
    { source: 1, target: 2, weight: 0.5, metric: "Coherence", frequency_band: "Beta" },
  ],
  timestamp: 0.0,
  window_duration_s: 1.0,
  atlas: { Custom: 3 },
});

const graph = create_brain_graph(graphJson);
const mincut = compute_mincut(graphJson);
const metrics = compute_topology_metrics(graphJson);
const embedding = embed_graph(graphJson, 2);
const rvfBytes = export_rvf(graphJson);
console.log('Version:', version());
```

## API Reference

| Function                   | Description                                       |
|----------------------------|---------------------------------------------------|
| `create_brain_graph(json)` | Parse JSON into a BrainGraph JS object             |
| `compute_mincut(json)`     | Stoer-Wagner minimum cut, returns MincutResult     |
| `compute_topology_metrics(json)` | Compute TopologyMetrics for a graph          |
| `embed_graph(json, dim)`   | Spectral embedding via power iteration             |
| `decode_state(json)`       | Classify CognitiveState from TopologyMetrics       |
| `load_rvf(bytes)`          | Parse RVF binary data into JS object               |
| `export_rvf(json)`         | Serialize BrainGraph to RVF bytes                  |
| `version()`                | Return crate version string                        |

| Module      | Key Types                                                 |
|-------------|-----------------------------------------------------------|
| `graph_wasm`| `wasm_mincut`, `wasm_embed`, `wasm_topology_metrics`, `wasm_decode` |
| `streaming` | WebSocket streaming data processor                        |
| `viz_data`  | D3.js / Three.js visualization structures                 |

## Integration

Depends on `ruv-neural-core` for `BrainGraph`, `TopologyMetrics`, `RvfFile`,
and `CognitiveState` types. Uses `wasm-bindgen` and `serde-wasm-bindgen` for
JS interop. Designed for browser-based dashboards and real-time visualization
applications.

## License

MIT OR Apache-2.0
