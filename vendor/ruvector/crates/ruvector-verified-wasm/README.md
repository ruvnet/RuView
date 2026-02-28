# ruvector-verified-wasm

[![Crates.io](https://img.shields.io/crates/v/ruvector-verified-wasm.svg)](https://crates.io/crates/ruvector-verified-wasm)
[![npm](https://img.shields.io/npm/v/ruvector-verified-wasm.svg)](https://www.npmjs.com/package/ruvector-verified-wasm)
[![License](https://img.shields.io/crates/l/ruvector-verified-wasm.svg)](https://github.com/ruvnet/ruvector)

WebAssembly bindings for [ruvector-verified](https://crates.io/crates/ruvector-verified) — proof-carrying vector operations in the browser. Verify vector dimensions, build typed HNSW indices, and create 82-byte proof attestations entirely client-side with sub-microsecond overhead.

## Quick Start

```js
import init, { JsProofEnv } from "ruvector-verified-wasm";

await init();
const env = new JsProofEnv();

// Prove dimension equality (~500ns)
const proofId = env.prove_dim_eq(384, 384);

// Verify a batch of vectors (flat f32 array)
const flat = new Float32Array(384 * 100); // 100 vectors
const count = env.verify_batch_flat(384, flat);
console.log(`Verified ${count} vectors`);

// Create 82-byte proof attestation
const att = env.create_attestation(proofId);
console.log(att.bytes.length); // 82

// Route proof to cheapest tier
const routing = env.route_proof("dimension");
console.log(routing); // { tier: "reflex", reason: "...", estimated_steps: 1 }

// Get statistics
console.log(env.stats());
```

## API

| Method | Returns | Description |
|--------|---------|-------------|
| `new JsProofEnv()` | `JsProofEnv` | Create environment with all ultra optimizations |
| `.prove_dim_eq(a, b)` | `number` | Prove dimensions equal, returns proof ID |
| `.mk_vector_type(dim)` | `number` | Build `RuVec n` type term |
| `.mk_distance_metric(m)` | `number` | Build metric type: `"L2"`, `"Cosine"`, `"Dot"` |
| `.verify_dim_check(dim, vec)` | `number` | Verify single vector dimension |
| `.verify_batch_flat(dim, flat)` | `number` | Verify N vectors (flat f32 array) |
| `.arena_intern(hi, lo)` | `[id, cached]` | Intern into FastTermArena |
| `.route_proof(kind)` | `object` | Route to Reflex/Standard/Deep tier |
| `.create_attestation(id)` | `object` | Create 82-byte proof witness |
| `.stats()` | `object` | Get verification statistics |
| `.reset()` | `void` | Reset environment |
| `.terms_allocated()` | `number` | Count of allocated proof terms |

## Building

```bash
# With wasm-pack
wasm-pack build crates/ruvector-verified-wasm --target web

# With cargo (for crates.io)
cargo build -p ruvector-verified-wasm --target wasm32-unknown-unknown
```

## License

MIT OR Apache-2.0
