# @ruvector/core-darwin-arm64

Native macOS ARM64 bindings for @ruvector/core.

This package contains the native Node.js addon for macOS (Apple Silicon) systems.

## Installation

This package is automatically installed as an optional dependency of `@ruvector/core` when running on macOS ARM64 systems.

```bash
npm install @ruvector/core
```

## Direct Installation

You can also install this package directly:

```bash
npm install @ruvector/core-darwin-arm64
```

## Usage

```javascript
const { VectorDb } = require('@ruvector/core-darwin-arm64');

const db = new VectorDb({
  dimensions: 128,
  storagePath: './vectors.db'
});

// Insert vectors
await db.insert({
  id: 'vec1',
  vector: new Float32Array([...])
});

// Search
const results = await db.search({
  vector: new Float32Array([...]),
  k: 10
});
```

## Requirements

- Node.js >= 18
- macOS (Apple Silicon - M1, M2, M3, etc.)

## License

MIT
