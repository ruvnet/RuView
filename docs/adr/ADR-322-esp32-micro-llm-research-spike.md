# ADR-322: ESP32-S3 micro-LLM inference — bounded research spike, production non-goal

- **Status**: Proposed
- **Date**: 2026-08-16
- **Deciders**: ruv
- **Owners**: RuView firmware and edge runtime maintainers
- **Tags**: esp32, llm, edge, research, evidence-labeling, firmware
- **Numbering note**: originally authored as ADR-324/ADR-325; renumbered to
  ADR-322/ADR-328 on 2026-08-22 to resolve a collision after ADR-324 through
  ADR-327 were assigned to unrelated work merged in the interim. ADR-322 fills
  an observed gap in the index; ADR-328 is the next free number after
  ADR-327.
- **Extends**: ADR-028, ADR-039, ADR-040, ADR-045, ADR-110, ADR-175, ADR-304
- **Supersedes**: None
- **Companion research**: `docs/research/esp32-micro-llm-inference.md`
- **Companion ADR**: ADR-328 (flash-resident weight streaming)

## Executive decision

RuView will treat on-device micro-LLM inference on ESP32-S3 as a **bounded,
optional research spike on dedicated companion hardware**, and explicitly a
**non-goal for production CSI node firmware**. No LLM code, model weights, or
partition changes enter `firmware/esp32-csi-node/` under this ADR. Every
externally sourced performance number is `CLAIMED` until reproduced on RuView
silicon with a captured witness log.

## Context

Public demonstrations in 2026 (slvDev's `esp32-ai`, building on
`karpathy/llama2.c` and `DaveBben/esp32-llm`) show a 28.9 M-parameter
TinyStories transformer running at 9.5–9.88 tok/s (`CLAIMED`) on an ESP32-S3
with 512 KB SRAM, 8 MB PSRAM, and 16 MB flash, using 4-bit quantization and
Gemma-3n-style Per-Layer Embeddings memory-mapped from flash. The companion
research document analyzes the techniques and coverage in depth.

The question for RuView is whether any of this belongs on our sensing nodes.
The material facts are:

1. **It does not fit our fleet.** Production nodes use the ADR-045 8 MB
   partition map (2× 2 MB OTA + 1.875 MB SPIFFS) or the 4 MB variant. The
   14.9 MB model artifact cannot be stored on either (`SYNTHETIC`, partition
   arithmetic). Replication requires a 16 MB-flash S3 SKU we do not deploy.
2. **It contends with the product.** The demos own both cores and run no
   radio traffic during inference. RuView nodes dedicate core 0 to
   WiFi + 20 Hz CSI capture and core 1 to the Tier 1–2 DSP pipeline and WASM
   modules (ADR-039). LLM inference would degrade the primary sensing path.
3. **The model class has no task capability.** TinyStories-scale models
   generate short fiction only — no instruction following, no factual recall.
   Every on-node use case we examined is served better by deterministic
   templates or is blocked on datasets that do not exist.
4. **The memory techniques are genuinely valuable** — for RuView's own edge
   models, which is split out as ADR-328 so its fate is independent of the
   LLM demo's.

The rejected premise is: "an LLM now runs on our chip, therefore our product
should run an LLM." Feasibility of a demo is not fitness for a sensing
appliance.

## Decision

1. **Production non-goal.** `firmware/esp32-csi-node/` gains no LLM inference
   path, no model partitions, and no LLM-derived output fields. Any future
   reversal requires a new ADR with `MEASURED` evidence of zero CSI-path
   regression.
2. **Bounded research spike (optional, unscheduled).** If funded, the spike
   runs on a dedicated ESP32-S3 N16R8-class devkit that performs no CSI
   capture, following the validation plan in the research document:
   unmodified `esp32-ai` reproduction with witness log; WiFi-coexistence
   penalty measurement; a ≤3.5 M-parameter fit-our-flash retrain solely to
   characterize the quality cliff.
3. **Evidence labeling.** All numbers from external sources remain `CLAIMED`
   in every RuView document, README, and communication until step 2 produces
   `MEASURED` rows backed by a committed witness log (WITNESS-LOG discipline
   per ADR-028). Builds and simulators are not hardware evidence.
4. **Trust boundaries.** Third-party inference code and weights undergo
   license review at intake; model weights are never committed to this
   repository; fetched artifacts are SHA-256 pinned. Micro-LLM output is
   never admitted into the evidence engine (ADR-304), never labeled as
   perception, and never enters a safety-relevant path (fall/presence
   alerts).
5. **ESP32-C6 exclusion.** The spike targets S3 only; the single-core RISC-V
   C6 without PIE SIMD is out of scope (ADR-110 unaffected).

## Options considered

- **Do nothing.** Cheapest, but discards transferable memory techniques and
  leaves recurring "can we run an LLM on the nodes?" questions undocumented.
  Rejected in favor of a written boundary.
- **Integrate into production firmware as a Tier 3.** Rejected: flash
  impossibility on deployed SKUs, core/PSRAM contention with sensing,
  no task capability at this model scale, safety-path hallucination risk.
- **Companion-hardware research spike (chosen).** Isolates risk, produces
  `MEASURED` data, feeds ADR-328.
- **Server-side LLM only.** Remains the correct place for any real language
  capability; this ADR does not change server architecture.

## Consequences

- Contributors get a citable "no" for production LLM integration and a
  citable "yes, like this" for research.
- The spike, if run, costs one devkit and bounded engineering time, and
  produces the repository's first `MEASURED` micro-LLM data.
- Risk accepted: the field moves fast; this ADR may need revisiting if
  instruction-capable models reach ~10 M parameters. The evidence-labeling
  and non-contention requirements would still hold.

## References

- `docs/research/esp32-micro-llm-inference.md` (sources, techniques, fit
  analysis)
- ADR-028 (capability audit / witness discipline), ADR-039 (edge tiers),
  ADR-045 (partition map), ADR-175 (measured quantization precedent),
  ADR-304 (evidence engine), ADR-328 (companion decision)
