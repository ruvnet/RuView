# ruvector-mincut-gated-transformer-wasm

WebAssembly bindings for the mincut-gated transformer - ultra-low-latency inference with coherence control.

## Overview

This crate provides JavaScript-friendly WASM bindings for the `ruvector-mincut-gated-transformer` crate, enabling browser-based transformer inference with deterministic latency bounds and explainable decision making.

## Features

- **Zero-copy inference**: Direct memory access from JavaScript
- **Deterministic bounds**: Predictable p99 latency guarantees
- **Explainable decisions**: Every inference produces a witness
- **Coherence control**: Integration with dynamic minimum cut signals
- **Event-driven scheduling**: Optional spike-based compute tier selection

## Installation

### NPM

```bash
npm install ruvector-mincut-gated-transformer-wasm
```

### Build from source

```bash
wasm-pack build --target web
```

## Quick Start

```javascript
import init, { WasmTransformer, WasmGatePacket } from './pkg';

async function run() {
  await init();

  // Create transformer with micro config (optimized for WASM)
  const transformer = new WasmTransformer();

  // Create gate packet from coherence signals
  const gate = new WasmGatePacket();
  gate.lambda = 100;
  gate.lambda_prev = 95;
  gate.boundary_edges = 5;
  gate.boundary_concentration_q15 = 8192;
  gate.partition_count = 3;

  // Run inference
  const tokens = new Uint32Array([1, 2, 3, 4]);
  const result = transformer.infer(tokens, gate);

  console.log('Decision:', result.decision);
  console.log('Reason:', result.reason);
  console.log('Tier:', result.tier);
  console.log('KV writes enabled:', result.kv_writes_enabled);
  console.log('External writes enabled:', result.external_writes_enabled);
  console.log('Logits:', result.logits);
}

run();
```

## API Reference

### WasmTransformer

Main transformer class for inference.

#### Constructor

```javascript
const transformer = new WasmTransformer();
```

Creates a transformer with micro config (sequence length: 32, hidden: 128, heads: 4, layers: 2).

#### Methods

- `new_baseline()`: Create with baseline config (larger model)
- `with_config(config)`: Create with custom configuration
- `infer(tokens, gate)`: Run inference with gate packet
- `infer_with_spikes(tokens, gate, spikes)`: Run inference with gate and spike packets
- `reset()`: Reset all state (KV cache, cached logits)
- `buffer_size()`: Get logits buffer size
- `set_policy(policy)`: Update gate policy

### WasmGatePacket

Gate packet carrying coherence control signals.

#### Constructor

```javascript
const gate = new WasmGatePacket();
```

#### Properties

- `lambda`: Current coherence metric (minimum cut value)
- `lambda_prev`: Previous lambda for trend detection
- `boundary_edges`: Number of edges crossing partition boundaries
- `boundary_concentration_q15`: Boundary concentration (Q15: 0-32767)
- `partition_count`: Number of partitions in graph
- `flags`: Policy flags (force safe mode, etc.)

### WasmSpikePacket

Spike packet for event-driven scheduling.

#### Constructor

```javascript
const spike = new WasmSpikePacket();
```

#### Properties

- `fired`: Spike fired indicator (0 = skip, 1 = active)
- `rate_q15`: Spike rate (Q15: 0-32767)
- `novelty_q15`: Novelty metric (Q15: 0-32767)
- `flags`: Spike flags

### WasmInferResult

Inference result with logits and witness information.

#### Properties

- `logits`: Output logits (Int32Array)
- `decision`: Gate decision ("Allow", "ReduceScope", "FlushKv", "FreezeWrites", "QuarantineUpdates")
- `reason`: Decision reason ("None", "LambdaBelowMin", "LambdaDroppedFast", etc.)
- `tier`: Compute tier used (0-3)
- `kv_writes_enabled`: Whether KV writes were enabled
- `external_writes_enabled`: Whether external writes are enabled
- `effective_seq_len`: Effective sequence length used
- `effective_window`: Effective window size used
- `lambda`: Current lambda value
- `lambda_prev`: Previous lambda value
- `boundary_edges`: Boundary edges count
- `partition_count`: Partition count

## Configuration

### Micro Config (Default)

Optimized for WASM and edge gateways:

```javascript
{
  seq_len_max: 32,
  hidden: 128,
  heads: 4,
  layers: 2,
  window_normal: 8,
  window_degraded: 4,
  ffn_mult: 4,
  logits: 256
}
```

### Baseline Config

Larger model for more capacity:

```javascript
const transformer = WasmTransformer.new_baseline();
// seq_len_max: 64, hidden: 256, heads: 4, layers: 4, logits: 1024
```

### Custom Config

```javascript
const config = {
  seq_len_max: 32,
  hidden: 128,
  heads: 4,
  layers: 2,
  window_normal: 8,
  window_degraded: 4,
  ffn_mult: 4,
  logits: 256,
  layers_degraded: 1,
  seq_len_degraded: 16,
  seq_len_safe: 4,
  enable_kv_cache: true,
  enable_external_writes: true
};

const transformer = WasmTransformer.with_config(config);
```

## Gate Policy

Control when the gate intervenes:

```javascript
const policy = {
  lambda_min: 30,
  drop_ratio_q15_max: 12288,  // ~37.5%
  boundary_edges_max: 20,
  boundary_concentration_q15_max: 20480,  // ~62.5%
  partitions_max: 10,
  spike_rate_q15_max: 16384,
  spike_novelty_q15_min: 2048,
  allow_kv_write_when_unstable: true,
  allow_external_write_when_unstable: false
};

transformer.set_policy(policy);
```

## Decision Types

### Gate Decisions

- **Allow**: Proceed normally with full capabilities
- **ReduceScope**: Reduce sequence length and window size
- **FlushKv**: Flush KV cache before proceeding
- **FreezeWrites**: Run in read-only mode (no KV updates)
- **QuarantineUpdates**: Run compute but discard all state changes

### Decision Reasons

- **None**: No intervention needed
- **LambdaBelowMin**: Lambda below minimum threshold
- **LambdaDroppedFast**: Lambda dropped too quickly
- **BoundarySpike**: Boundary edge count exceeded threshold
- **BoundaryConcentrationSpike**: Boundary concentration too high
- **PartitionDrift**: Partition count indicates drift
- **SpikeStorm**: Spike rate indicates overload
- **ForcedByFlag**: Forced by flag in gate packet

## Examples

### Basic Inference

```javascript
const transformer = new WasmTransformer();
const gate = new WasmGatePacket();
const tokens = new Uint32Array([1, 2, 3, 4]);
const result = transformer.infer(tokens, gate);
console.log(result.decision);
```

### With Spike Scheduling

```javascript
const transformer = new WasmTransformer();
const gate = new WasmGatePacket();
const spike = new WasmSpikePacket();
spike.fired = 1;
spike.novelty_q15 = 8192;

const tokens = new Uint32Array([1, 2, 3, 4]);
const result = transformer.infer_with_spikes(tokens, gate, spike);
```

### Handling Interventions

```javascript
const transformer = new WasmTransformer();
const gate = new WasmGatePacket();
gate.lambda = 10;  // Low coherence
gate.lambda_prev = 100;

const tokens = new Uint32Array([1, 2, 3, 4]);
const result = transformer.infer(tokens, gate);

if (result.decision !== 'Allow') {
  console.log('Intervention triggered:', result.reason);
  console.log('Effective seq_len:', result.effective_seq_len);
  console.log('KV writes:', result.kv_writes_enabled);
}
```

## Building

### Development

```bash
wasm-pack build --dev --target web
```

### Release (optimized)

```bash
wasm-pack build --release --target web
```

### For Node.js

```bash
wasm-pack build --target nodejs
```

### For Bundlers

```bash
wasm-pack build --target bundler
```

## Testing

### Browser tests

```bash
wasm-pack test --headless --firefox
wasm-pack test --headless --chrome
```

### Node.js tests

```bash
wasm-pack test --node
```

## Performance

The WASM bindings maintain the core performance characteristics:

- **Allocation-free hot path**: Zero heap allocations during inference
- **Predictable latency**: Bounded p99 latency guarantees
- **Small binary size**: ~50KB compressed (micro config)
- **Low memory footprint**: ~128KB runtime state (micro config)

## Integration with RuVector

This transformer integrates with the RuVector ecosystem:

- **ruvector-mincut**: Provides coherence signals via gate packets
- **ruvector-core**: Vector search and semantic retrieval
- **ruvector-router**: Query routing and orchestration

## License

MIT OR Apache-2.0

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [Core Library](../ruvector-mincut-gated-transformer)
- [RuVector Documentation](../../README.md)
