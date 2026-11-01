# ADR-368: RuView compute kernel as a WASM + napi-rs npm package

- **Status**: proposed
- **Date**: 2026-09-30
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: npm, wasm, napi-rs, metaharness, vitals, supply-chain
- **Related**: ADR-021 (vitals), ADR-263/265 (npm harness review and
  distribution), ADR-283 (community metaharness), ADR-285 (WASM-first Homecore
  metaharness)

## Context

`@ruvnet/ruview` (ADR-283) is a dependency-free operator/contributor harness:
it guides, verifies, and delegates, but it cannot run RuView signal processing.
JavaScript consumers (MCP hosts, dashboards, browser demos, agents) that want
real RuView computation must today start the Rust sensing server or re-implement
DSP in JS, which drifts from the Rust source of truth.

We want one Rust implementation reachable from npm with:

1. a portable default that works on every Node platform and in browsers;
2. an optional native path for hosts that want it;
3. the metaharness trust posture: least authority by default, the backend that
   actually ran is always reported, fail-closed on missing or tampered
   artifacts, and no accuracy claim without an evidence tag.

## Decision

### One core, one ABI, two transports

- **`v2/crates/ruview-kernel`** (workspace member) wraps the ADR-021 pipeline
  from `wifi-densepose-vitals` (EMA preprocessing, breathing 0.1–0.5 Hz, heart
  0.8–2.0 Hz, anomaly z-score) in a streaming `Session`, a deterministic
  SplitMix64 synthetic-CSI generator, and a bounded session registry.
- The entire ABI is one function: `call(op, request_json) -> envelope_json`,
  returning `{"ok":true,"result":…}` or
  `{"ok":false,"error":{"code","message"}}`. Operations: `info`,
  `validate_config`, `analyze`, `synthesize`, `session_open|push|summary|close`.
  Keeping the ABI to a single string function makes backend parity a property
  of the transport rather than of duplicated bindings.
- **WASM transport** (`src/wasm_abi.rs`, `wasm32-unknown-unknown` only):
  `rvk_alloc`/`rvk_free`/`rvk_call`/`rvk_abi_version`. The module has **zero
  imports** — no filesystem, network, clock, or randomness. The JS loader
  refuses any module that imports host functions.
- **napi-rs transport** (`v2/crates/ruview-kernel-napi`): exports only
  `abiVersion()` and `call(op, json)`. It is a standalone Cargo workspace
  (listed in the v2 `exclude`) so napi never enters
  `cargo test --workspace`, and it builds with `panic = "unwind"` plus
  `catch_unwind` so a defect becomes an error envelope, not a Node crash.

### Input is untrusted at the ABI

Requests are bounded before processing: 16 MiB per request, 512 subcarriers,
100 000 frames per call or synthesis, 64 concurrent sessions. Unknown JSON
fields are rejected (`deny_unknown_fields`); frame widths must match the
configured subcarrier count; values must be finite with magnitude ≤ 1e9; all
frames of a batch are validated before any is processed. The core returns an
error envelope instead of panicking on caller data (release builds of the WASM
module use `panic = "abort"`, which the loader surfaces as a `trap` and
recovers from with a fresh instance).

### npm package `@ruvnet/ruview-kernel` (`harness/ruview-kernel`)

- Zero runtime dependencies. `loadKernel({backend})` with
  `backend ∈ {wasm (default), napi, auto}` or `RUVIEW_KERNEL_BACKEND`.
  WASM is the default because it carries no host authority. `napi` never
  silently falls back; `auto` prefers napi and records `fallbackReason`.
- Artifacts are built from source by `npm run build` (`scripts/build.mjs`,
  no shell) into `wasm/` and `native/ruview_kernel.<triple>.node`, with
  `SHA256SUMS`. The loader verifies checksums; a mismatch is always fatal and
  never an `auto` fallback trigger; `requireIntegrity` also refuses
  unchecksummed artifacts. Built artifacts are git-ignored and never committed.
- `src/wasm.js` is filesystem-free, so browsers can use
  `loadWasmKernelFromBytes(bytes)`.
- CLI `ruview-kernel`: `doctor`, `info`, `selftest`, `parity`, `synth`,
  `analyze --file`, `bench`. `synth --out` refuses to overwrite files;
  `analyze` reads only a bounded regular file.
- `selfTest()` runs synthesize → analyze → chunked streaming session → hostile
  input, and asserts known-tone recovery. `parity()` diffs every output of
  both backends with a 1e-9 relative tolerance, because `sin`/`exp` come from
  different libm implementations on wasm32 and native targets; exact JSON
  equality is reported separately and is not required.

### Metaharness integration

`@ruvnet/ruview` gains the read-only tool `ruview_kernel_selftest`
(CLI: `ruview kernel [--backend …]`). It dynamically imports the fixed
specifier `@ruvnet/ruview-kernel`, so `@ruvnet/ruview` keeps zero runtime
dependencies, and it returns `kernel_not_installed` when the package is absent.
MCP arguments are limited to `backend` and `seconds`; they cannot choose a
module or artifact path. The capability appears in `ruview_guidance` as
`portable-compute-kernel`.

## Evidence

- **SYNTHETIC** (`cd harness/ruview-kernel && node bin/cli.js selftest`):
  60 s at 20 Hz, 56 subcarriers, 15 bpm breathing and 72 bpm heart tones →
  recovered 15.0 bpm respiratory (valid) and 70.6 bpm heart (degraded
  confidence 0.44). Tolerances asserted: ±3 bpm respiratory, ±8 bpm heart.
- **SYNTHETIC** parity (`node bin/cli.js parity`): 900 frames / 45 readings,
  no difference above 1e-9 relative between wasm32 and linux-x64-gnu napi;
  outputs are not byte-identical.
- **MEASURED** on one Linux x64 development container
  (`node bin/cli.js bench --backend wasm|napi --seconds 120`): about 78 ms
  (wasm) and 63 ms (napi) per 2 400-frame `analyze`, including JSON transfer.
  Single-host timing, not a cross-platform claim.
- The WASM release artifact is about 230 KB and imports nothing
  (`node scripts/verify-artifacts.mjs --require-wasm`).

None of these numbers says anything about real-world vital-sign accuracy.
That still requires the ADR-293 ground-truth protocol with a reference device.

## Amendment 1 (2026-10-01): binary transport and optimization evidence

**Profile (MEASURED on one Linux x64 development container, 2 400 frames ×
56 subcarriers).** JSON was the dominant cost:

- JS `JSON.stringify` of the request alone took 52–78 ms against
  107–124 ms for a whole `analyze`.
- Rust then re-parsed about 270 000 numbers.

**Decision.** Add a binary fast path without changing the ABI version.

- New operations `analyze_flat` and `session_push_flat`, carried by
  `call_f64(op, json, &[f64])`:
  - WASM: `rvk_call_f64`, which reads bytes so alignment does not matter;
  - napi: `callF64(op, json, Float64Array)`.
- `analyze()` and `session.push()` pack uniform frames into one
  `Float64Array` (all amplitudes, then all phases) and use the binary path
  automatically. Irregular input still takes the JSON path so its error names
  the bad frame.
- `analyzeFlat(Float64Array, …)` is the zero-copy entry.
- `Session::step` is shared by both paths.

**Evidence.**

- **Byte-identical outputs.** On identical `f64` inputs the two paths
  produce byte-identical JSON (`tests/abi.rs`).
- **A JSON precision finding.** serde_json's default float parser is not
  correctly rounded, so the JSON transport perturbs inputs by ULPs (34 of 160
  amplitude values in a 2 s, 4-subcarrier SYNTHETIC sample). The binary path is bit-exact.
- **Speed.** MEASURED, `ruview-kernel bench --seconds 120`, same host:

  | Backend | JSON transport | Binary transport | Speed-up |
  |---|---|---|---|
  | wasm | 134.8 ms | 47.8 ms (~50 000 frames/s) | 2.82× |
  | napi | 109.7 ms | 44.8 ms (~53 600 frames/s) | 2.45× |

- **Remaining cost is DSP.** Native timing gives heart-rate extraction
  20.8–22.9 ms of 30.6–38.3 ms (about 60–68%, MEASURED; reproducer:
  `cargo run --release -p ruview-kernel --example profile_stages`): the per-frame autocorrelation over the 15 s window
  in `wifi-densepose-vitals`. That crate is shared with the production sensing
  server, and an incremental ACF would change floating-point summation order
  and therefore outputs. It is left as a follow-up that needs its own review
  and regression evidence.

**Rejected after measurement.** MEASURED, three runs on the same host;
reproducer: `node bench/wasm-variants.mjs` in `harness/ruview-kernel`. Each
run is the median of 15 `analyzeFlat` calls on 2 400 frames. All variants
produced byte-identical outputs.

| Variant | Size | Median ms (3 runs) | Decision |
|---|---|---|---|
| baseline (`opt-level = 3`) | 244 KB | 38.3–40.6 | kept |
| `+simd128` | 242 KB | 37.4–54.7 | rejected: no consistent gain, and it needs SIMD-capable engines |
| `opt-level = "s"` | 227 KB | 40.8–70.1 | rejected: consistently slower for a 17 KB size saving |

## Consequences

- JavaScript consumers get real RuView DSP without a server, in Node or the
  browser, with one Rust source of truth.
- A second native build matrix exists. Until per-platform packages
  (`@ruvnet/ruview-kernel-<triple>`) are published by CI, only WASM is
  portable; the loader already resolves those package names.
- JSON transfer dominates the cost of small calls; a typed-array fast path can
  be added later behind a new ABI version.

## Rollout and rollback

1. CI (`ruview-kernel-npm.yml`) builds both transports from source, runs the
   Rust tests, the package tests with `RUVIEW_KERNEL_REQUIRE_NAPI=1`, parity,
   artifact verification, and `npm pack --dry-run`.
2. Publication stays CI-only with provenance (ADR-265) and requires explicit
   maintainer approval; this ADR does not add a publish job.
3. Rollback: remove the `ruview_kernel_selftest` tool (the harness has no
   dependency on the package) and deprecate the npm version; the Rust crates
   are additive.
