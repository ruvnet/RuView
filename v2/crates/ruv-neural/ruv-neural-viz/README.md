# ruv-neural-viz

Brain topology visualization, ASCII rendering, and export formats.

## Overview

`ruv-neural-viz` provides layout algorithms, color mapping, terminal-friendly
ASCII rendering, animation frame generation, and export to standard graph
visualization formats for brain connectivity graphs. It turns `BrainGraph` and
mincut analysis results into visual output suitable for terminal dashboards,
web applications, and graph analysis tools.

## Features

- **Layout algorithms** (`layout`): `ForceDirectedLayout` for spring-based node
  positioning and `AnatomicalLayout` for MNI-coordinate-based brain region
  placement; circular layout variants
- **Color mapping** (`colormap`): `ColorMap` with cool-warm, viridis, and
  module-color schemes for mapping scalar values (edge weights, node degrees)
  to colors
- **ASCII rendering** (`ascii`): Terminal-friendly renderers for brain graphs,
  mincut partitions, sparkline time series, connectivity matrices, and
  real-time dashboard views
- **Export formats** (`export`): D3.js JSON (force-directed graph format),
  Graphviz DOT, GEXF (Gephi), and CSV timeline export
- **Animation** (`animation`): `AnimationFrames` generator from temporal
  `BrainGraphSequence` data with `AnimatedNode`, `AnimatedEdge`, and
  `AnimationFrame` types; configurable `LayoutType` per frame

## Usage

```rust
use ruv_neural_viz::{
    ForceDirectedLayout, AnatomicalLayout, ColorMap,
    AnimationFrames, LayoutType,
};
use ruv_neural_viz::ascii;
use ruv_neural_viz::export;

// Force-directed layout for a brain graph
let layout = ForceDirectedLayout::new();
let positions = layout.compute(&graph);

// Anatomical layout using MNI coordinates
let anat_layout = AnatomicalLayout::new();
let positions = anat_layout.compute(&graph, &parcellation);

// Color mapping
let cmap = ColorMap::cool_warm();
let color = cmap.map(0.75); // returns (r, g, b)

// ASCII rendering to terminal
ascii::render_graph(&graph);
ascii::render_mincut(&mincut_result);

// Export to D3.js JSON
let d3_json = export::to_d3_json(&graph, &positions);

// Export to Graphviz DOT
let dot = export::to_dot(&graph);

// Generate animation frames from temporal sequence
let frames = AnimationFrames::from_sequence(
    &graph_sequence,
    LayoutType::ForceDirected,
);
```

## API Reference

| Module      | Key Types / Functions                                          |
|-------------|----------------------------------------------------------------|
| `layout`    | `ForceDirectedLayout`, `AnatomicalLayout`                      |
| `colormap`  | `ColorMap`                                                     |
| `ascii`     | Graph, mincut, sparkline, matrix, and dashboard renderers      |
| `export`    | `to_d3_json`, `to_dot`, `to_gexf`, `to_csv_timeline`          |
| `animation` | `AnimationFrames`, `AnimationFrame`, `AnimatedNode`, `AnimatedEdge`, `LayoutType` |

## Feature Flags

| Feature | Default | Description                         |
|---------|---------|-------------------------------------|
| `std`   | Yes     | Standard library support            |
| `ascii` | No      | ASCII art rendering for terminal    |

## Integration

Depends on `ruv-neural-core` for `BrainGraph` types, `ruv-neural-graph` for
graph metrics used in layout computation, and `ruv-neural-mincut` for partition
visualization. Used by `ruv-neural-cli` for terminal dashboard output and
export commands.

## License

MIT OR Apache-2.0
