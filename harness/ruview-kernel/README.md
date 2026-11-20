# @ruvnet/ruview-kernel

RuView's Rust signal pipeline for JavaScript. The ADR-021 vitals pipeline
(`wifi-densepose-vitals`) is compiled to **zero-import WebAssembly** (default)
and, optionally, a **napi-rs** native addon. Both expose the same one-function
JSON ABI, so results match across backends (ADR-368).

> Outputs are signal-processing estimates, not clinical or camera-grade
> measurements. `synthesize()` data and every self-test number are
> **SYNTHETIC**.

## Build from a RuView checkout

```bash
rustup target add wasm32-unknown-unknown
cd harness/ruview-kernel
npm run build            # wasm + napi for this platform, with SHA256SUMS
npm test                 # unit, security, and end-to-end tests
node bin/cli.js parity   # wasm vs napi on identical SYNTHETIC input
```

`npm run build:wasm` alone is enough for the portable backend.

## CLI

```bash
ruview-kernel doctor                 # both backends, ABI, integrity
ruview-kernel selftest [--backend wasm|napi|auto]
ruview-kernel synth --seconds 60 --out frames.json
ruview-kernel analyze --file frames.json --readings
ruview-kernel bench --backend napi   # MEASURED on this host only
```

`analyze` accepts a JSON array of frames, `{ "config": {...}, "frames": [...] }`,
or JSONL (one `{"amplitudes":[...],"phases":[...]}` per line).

## API

```js
import { loadKernel, selfTest } from '@ruvnet/ruview-kernel';

const kernel = loadKernel();                 // wasm; or { backend: 'napi' | 'auto' }
console.log(kernel.backend, kernel.fallbackReason, kernel.integrity);

const { frames } = kernel.synthesize({ seconds: 60, breathing_bpm: 15 });
const report = kernel.analyze(frames, { n_subcarriers: 56, sample_rate_hz: 20 });
console.log(report.summary.last.respiratory);   // { bpm, confidence, status }

const session = kernel.openSession({ n_subcarriers: 56 });
const readings = session.push(frames.slice(0, 200));
session.close();
```

### Binary fast path

`analyze()` and `session.push()` automatically send uniform frames as one
`Float64Array` (all amplitudes, then all phases) instead of JSON numbers.
Irregular input still goes through JSON, so errors name the bad frame. To
skip packing entirely:

```js
const data = new Float64Array(frames * 56 * 2);   // amplitudes…, then phases…
kernel.analyzeFlat(data, { n_subcarriers: 56 }, { phases: true });
```

MEASURED on one Linux x64 host (`ruview-kernel bench --seconds 120`): 2.8×
(wasm) and 2.5× (napi) faster than the JSON transport for 2 400 frames. The
binary path is also bit-exact, while JSON number parsing can move inputs by
ULPs.

In a browser, fetch the module yourself and use the filesystem-free entry:

```js
import { loadWasmKernelFromBytes } from '@ruvnet/ruview-kernel/wasm';
const kernel = loadWasmKernelFromBytes(new Uint8Array(await (await fetch(url)).arrayBuffer()));
```

## Backends and trust

| Backend | Authority | Selection |
|---|---|---|
| `wasm` | none: module imports nothing, loader rejects any import | default |
| `napi` | full process (native code) | `backend: 'napi'`; never falls back silently |
| `auto` | napi if present, otherwise wasm with `fallbackReason` | opt-in |

Artifacts are checked against `SHA256SUMS`; a mismatch is always fatal.
Requests are bounded (16 MiB, 512 subcarriers, 100 000 frames per call, 64
sessions) and validated before processing.

## Metaharness

`npx @ruvnet/ruview kernel` (MCP tool `ruview_kernel_selftest`) runs this
package's self-test from the RuView harness when it is installed, and reports
`kernel_not_installed` otherwise.
