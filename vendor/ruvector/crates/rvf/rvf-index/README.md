# rvf-index

Progressive HNSW indexing with tiered Layer A/B/C search for RuVector Format.

## Overview

`rvf-index` implements a Hierarchical Navigable Small World (HNSW) index optimized for the RVF storage model:

- **Layer A** -- hot vectors, full-precision, in-memory graph
- **Layer B** -- warm vectors, quantized, memory-mapped
- **Layer C** -- cold vectors, compressed, on-disk with lazy loading
- **Progressive build** -- index grows incrementally without full rebuilds

## Usage

```toml
[dependencies]
rvf-index = "0.1"
```

## Features

- `std` (default) -- enable `std` support
- `simd` -- enable SIMD-accelerated distance computations

## License

MIT OR Apache-2.0
