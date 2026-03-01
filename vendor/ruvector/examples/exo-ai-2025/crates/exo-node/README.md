# exo-node

Node.js bindings for EXO-AI cognitive substrate via NAPI-RS.

[![Crates.io](https://img.shields.io/crates/v/exo-node.svg)](https://crates.io/crates/exo-node)
[![Documentation](https://docs.rs/exo-node/badge.svg)](https://docs.rs/exo-node)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

## Overview

`exo-node` provides native Node.js bindings:

- **NAPI-RS Bindings**: High-performance native module
- **Async Support**: Full async/await support via Tokio
- **TypeScript Types**: Complete TypeScript definitions
- **Native Performance**: Direct Rust execution

## Installation

```bash
npm install exo-node
```

## Usage

```javascript
const exo = require('exo-node');

// Create consciousness substrate
const substrate = new exo.ConsciousnessSubstrate();
substrate.addPattern(pattern);
const phi = substrate.computePhi();
```

## Links

- [GitHub](https://github.com/ruvnet/ruvector)
- [Website](https://ruv.io)
- [EXO-AI Documentation](https://github.com/ruvnet/ruvector/tree/main/examples/exo-ai-2025)

## License

MIT OR Apache-2.0
