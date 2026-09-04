# ADR-328: Flash-resident weight streaming for RuView edge models

- **Status**: Proposed
- **Date**: 2026-08-16
- **Deciders**: ruv
- **Owners**: RuView firmware and edge runtime maintainers
- **Tags**: esp32, memory, quantization, edge, inference, firmware, wasm
- **Numbering note**: originally authored as ADR-325; renumbered to ADR-328 on
  2026-08-22 to resolve a collision with ADR-325 (Cognitum Spaces activation)
  merged in the interim. See ADR-322 for the companion renumbering.
- **Extends**: ADR-028, ADR-039, ADR-040, ADR-045, ADR-102, ADR-163, ADR-175
- **Supersedes**: None
- **Companion research**: `docs/research/esp32-micro-llm-inference.md`
- **Companion ADR**: ADR-322 (micro-LLM research spike)

## Executive decision

RuView adopts a **design investigation** into flash-resident, memory-mapped,
quantized weight storage for its **own** edge inference models on ESP32-S3 —
the durable technique behind the 2026 micro-LLM demos — decoupled from any
language-model ambition. The target outcome is that future neural
presence/pose/vitals edge models are bounded by *flash* capacity rather than
by the ~7.36 MB of free PSRAM or ~280 KB of free SRAM, on the existing 8 MB
fleet SKU. This ADR authorizes design and benchmarking only; any partition
map or wire change requires its own follow-up ADR with `MEASURED` evidence.

## Context

Today RuView's on-node intelligence is DSP (ADR-039 Tiers 1–2) plus WASM3
modules in 640 KB of PSRAM arenas (ADR-040/041), with neural inference done
server-side. If RuView ever ships a neural edge model (e.g. an on-node
presence classifier to cut the Tier 2 heuristic's false-positive rate), the
naive approach loads all weights into PSRAM/SRAM, capping model size and
competing with WASM arenas and CSI buffers.

The `esp32-ai` demonstration (`CLAIMED`, see companion research) validated a
different memory contract on identical silicon: keep only activations and hot
weights in SRAM, the compute-dense core in PSRAM, and the bulk of parameters
4-bit-quantized in flash, accessed via the ESP32-S3 MMU's memory-mapped
read path with sparse, cache-friendly gathers (~450 bytes/token in the demo).
The same contract applies to non-LLM models: embedding/lookup layers,
frozen feature banks, and per-domain calibration tables are all
sparse-access, read-only structures.

RuView precedents: ADR-175 established measured INT8 quantization discipline;
ADR-163 established edge latency measurement; ADR-028 established witness
evidence. This ADR composes them.

## Decision

1. **Adopt the memory contract as a design target** for future RuView edge
   models on ESP32-S3: read-only quantized weight banks live in a dedicated
   flash region and are memory-mapped, not copied; PSRAM holds compute-dense
   weights and scratch; SRAM holds activations. INT8 remains the default
   quantization (ADR-175 precedent); sub-byte formats require their own
   measured accuracy gate.
2. **Fit the fleet, not the demo.** All designs must fit the ADR-045 8 MB
   partition map's current free space (1.875 MB SPIFFS region or a future
   dedicated `model` data partition of comparable size). Designs requiring a
   16 MB SKU are research-only under ADR-322.
3. **Benchmark before build.** The first deliverable is a microbenchmark
   suite on real S3 silicon measuring: memory-mapped flash gather latency and
   bandwidth (sequential vs. strided), ESP-DSP SIMD matmul throughput
   against PSRAM- vs. flash-resident operands, and CSI-pipeline interference
   (CSI packets/s at 20 Hz with the benchmark running on core 1). Results
   are committed as `MEASURED` with witness logs; until then, all sizing in
   this ADR is `SYNTHETIC`/`CLAIMED`.
4. **Integrity and provenance.** Any model bank flashed to a node carries a
   SHA-256 recorded in NVS and reported in the node hello; unsigned or
   mismatched banks are rejected at mount time. Model artifacts are never
   committed to this repository; distribution follows the edge module
   registry path (ADR-102) with the same review gates as WASM modules.
5. **Non-goals.** No production partition change, no wire-format change, no
   model training, and no LLM integration under this ADR. WASM-visible
   weight access (a `model_read` host API) is deferred until the benchmark
   shows the flash path sustains it alongside CSI capture.

## Options considered

- **Status quo (all weights in RAM).** Simple, but caps any future edge model
  at well under the free PSRAM after WASM arenas, and couples model growth to
  RAM contention with the sensing pipeline. Rejected as the default.
- **Flash-resident streaming (chosen for investigation).** Raises the
  capacity ceiling ~an order of magnitude on existing SKUs; costs MMU window
  management and benchmark-verified bandwidth budgeting.
- **16 MB SKU migration.** Hardware cost and fleet churn for an unproven
  need; remains available later and is orthogonal to this technique.
- **Keep all neural inference server-side.** Remains the default for heavy
  models; this ADR only lowers the barrier for small on-node models where
  latency/privacy justify them.

## Consequences

- Positive: future edge models are flash-bounded (~1.8 MB usable today,
  `SYNTHETIC`) instead of RAM-bounded; techniques arrive pre-validated by a
  measured benchmark rather than by external claims; provenance gating
  extends the existing module-registry trust model to weights.
- Negative/risks: flash read contention with OTA and SPIFFS logging must be
  measured, not assumed; MMU mapping bugs are a new failure class at the
  hardware boundary (input validation at the mount path is mandatory);
  benchmark effort is spent even if no neural edge model ever ships.
- The ADR-322 spike and this benchmark share kernels; either can proceed
  without the other.

## References

- `docs/research/esp32-micro-llm-inference.md` (technique analysis, sources)
- ADR-028 (witness evidence), ADR-039 (edge tiers and core budget), ADR-040/
  ADR-041 (WASM sensing), ADR-045 (partition map), ADR-102 (edge module
  registry), ADR-163 (edge latency measurement), ADR-175 (measured INT8
  quantization)
