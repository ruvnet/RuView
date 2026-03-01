# ruqu-wasm

[![Crates.io](https://img.shields.io/crates/v/ruqu-wasm.svg)](https://crates.io/crates/ruqu-wasm)
[![npm](https://img.shields.io/npm/v/@ruvector/ruqu-wasm.svg)](https://www.npmjs.com/package/@ruvector/ruqu-wasm)
[![License](https://img.shields.io/crates/l/ruqu-wasm.svg)](https://github.com/ruvnet/ruvector)

**Run quantum simulations in the browser** — WebAssembly bindings for ruqu-core and ruqu-algorithms with 25-qubit support.

## Features

- **Browser-Native** — Run quantum circuits directly in JavaScript/TypeScript
- **5 Simulation Backends** — StateVector, Stabilizer, Clifford+T, TensorNetwork, Hardware
- **25-Qubit Limit** — Optimized for browser memory constraints (~1GB for 25 qubits)
- **Full Algorithm Suite** — VQE, Grover, QAOA, Surface Code available
- **OpenQASM 3.0** — Export circuits to standard quantum assembly format
- **Zero Dependencies** — Pure WASM, no server required
- **TypeScript Types** — Full type definitions included

## Installation

### npm

```bash
npm install @ruvector/ruqu-wasm
```

### Rust (for building)

```bash
cargo add ruqu-wasm
wasm-pack build --target web
```

## Quick Start (JavaScript)

```javascript
import init, { Circuit, Simulator } from '@ruvector/ruqu-wasm';

await init();

// Create a Bell state
const circuit = new Circuit(2);
circuit.h(0);        // Hadamard on qubit 0
circuit.cnot(0, 1);  // CNOT: entangle qubits

// Run simulation
const sim = new Simulator();
const state = sim.run(circuit);

// Measure
const result = state.measureAll();
console.log(`Measured: ${result.toString(2).padStart(2, '0')}`);
// Output: "00" or "11" with 50% probability each
```

## React Example

```tsx
import { useEffect, useState } from 'react';
import init, { Circuit, Simulator } from '@ruvector/ruqu-wasm';

function QuantumDemo() {
  const [result, setResult] = useState<string | null>(null);

  useEffect(() => {
    async function runQuantum() {
      await init();

      const circuit = new Circuit(3);
      circuit.h(0);
      circuit.cnot(0, 1);
      circuit.cnot(1, 2);  // GHZ state

      const sim = new Simulator();
      const state = sim.run(circuit);
      setResult(state.measureAll().toString(2).padStart(3, '0'));
    }
    runQuantum();
  }, []);

  return <div>Quantum result: {result ?? 'Computing...'}</div>;
}
```

## API Reference

### Circuit

```typescript
class Circuit {
  constructor(nQubits: number);

  // Single-qubit gates
  h(qubit: number): void;      // Hadamard
  x(qubit: number): void;      // Pauli-X (NOT)
  y(qubit: number): void;      // Pauli-Y
  z(qubit: number): void;      // Pauli-Z
  rx(qubit: number, theta: number): void;  // X-rotation
  ry(qubit: number, theta: number): void;  // Y-rotation
  rz(qubit: number, theta: number): void;  // Z-rotation

  // Two-qubit gates
  cnot(control: number, target: number): void;
  cz(control: number, target: number): void;
  swap(q1: number, q2: number): void;

  // Three-qubit gates
  toffoli(c1: number, c2: number, target: number): void;
}
```

### Simulator

```typescript
class Simulator {
  constructor();
  run(circuit: Circuit): QuantumState;
}
```

### QuantumState

```typescript
class QuantumState {
  measureAll(): number;
  measure(qubit: number): number;
  probability(bitstring: number): number;
  amplitudes(): Float64Array;  // Complex interleaved [re, im, re, im, ...]
}
```

## Algorithms

### Grover's Search

```javascript
import { Grover } from '@ruvector/ruqu-wasm';

const grover = new Grover(4);  // 4 qubits = search space of 16
grover.setTarget(0b1010);       // Search for |1010⟩

const result = grover.search();
console.log(`Found: ${result.toString(2).padStart(4, '0')}`);
```

### VQE

```javascript
import { VQE, Hamiltonian } from '@ruvector/ruqu-wasm';

const h = new Hamiltonian();
h.addTerm("ZZ", 0.5);
h.addTerm("XX", 0.3);

const vqe = new VQE(h, nQubits: 4);
const energy = vqe.optimize({ maxIter: 100 });
console.log(`Ground state energy: ${energy}`);
```

## Performance

| Qubits | Memory | Init Time | Gate Time |
|--------|--------|-----------|-----------|
| 10 | 16 KB | 1ms | 0.01ms |
| 15 | 512 KB | 5ms | 0.1ms |
| 20 | 16 MB | 50ms | 5ms |
| 25 | 512 MB | 500ms | 150ms |

**Note**: 25 qubits requires ~1GB browser memory. Use Web Workers for heavy simulations.

## Web Worker Example

```javascript
// worker.js
import init, { Circuit, Simulator } from '@ruvector/ruqu-wasm';

self.onmessage = async (e) => {
  await init();
  const { gates, nQubits } = e.data;

  const circuit = new Circuit(nQubits);
  gates.forEach(g => circuit[g.name](...g.args));

  const sim = new Simulator();
  const state = sim.run(circuit);

  self.postMessage({ result: state.measureAll() });
};
```

## Bundle Size

| Build | Size (gzip) |
|-------|-------------|
| Core only | 45 KB |
| With algorithms | 120 KB |
| Full bundle | 180 KB |

## Browser Support

- Chrome 89+
- Firefox 89+
- Safari 15+
- Edge 89+

Requires WebAssembly SIMD for optimal performance (available in all modern browsers).

## Related Packages

- [`ruqu-core`](https://crates.io/crates/ruqu-core) — Rust quantum simulator
- [`ruqu-algorithms`](https://crates.io/crates/ruqu-algorithms) — Algorithm implementations
- [`ruqu-exotic`](https://crates.io/crates/ruqu-exotic) — Experimental hybrids

## Documentation

- [WASM Strategy (ADR-QE-003)](https://github.com/ruvnet/ruvector/blob/main/docs/adr/quantum-engine/ADR-QE-003-wasm-compilation-strategy.md)
- [Performance (ADR-QE-004)](https://github.com/ruvnet/ruvector/blob/main/docs/adr/quantum-engine/ADR-QE-004-performance-optimization-benchmarks.md)

## License

MIT OR Apache-2.0
