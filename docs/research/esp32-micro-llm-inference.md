# Deep research: micro-LLM inference on ESP32-S3-class microcontrollers

- **Date**: 2026-08-16
- **Status**: Research survey (no RuView hardware evidence yet)
- **Related ADRs**: ADR-322 (micro-LLM research spike), ADR-328 (flash-resident
  weight streaming), ADR-028 (ESP32 capability audit), ADR-039 (edge
  intelligence tiers), ADR-040 (WASM programmable sensing), ADR-045 (8 MB
  partition table), ADR-110 (ESP32-C6 extension), ADR-175 (INT8 quantization,
  measured)
- **Evidence policy**: Every performance or capability number in this document
  is tagged `MEASURED` (reproduced on RuView hardware with a witness log),
  `CLAIMED` (asserted by an external source, not reproduced by us), or
  `SYNTHETIC` (derived arithmetic, no hardware run). As of this writing there
  are **zero** `MEASURED` micro-LLM numbers in this repository.

## 1. Executive summary

In mid-2026 a developer known as slvDev (YouTube channel "The Stack") published
`esp32-ai`, a demonstration that a 28.9 M-parameter transformer trained on
TinyStories runs entirely on an ~$8 ESP32-S3 at roughly 9.5–9.88 tokens/s
(`CLAIMED`). The result was widely covered (Tom's Hardware, The Register,
CNX Software, Hackster, XDA, geeky-gadgets). The enabling ideas are:

1. **Per-Layer Embeddings (PLE)**, borrowed from Google's Gemma 3n: ~25 M of
   the 28.9 M parameters live in a memory-mapped lookup table in flash that is
   *read*, not computed on — only ~450 bytes of table data are touched per
   token (`CLAIMED`).
2. **4-bit quantization**, compressing the model to 14.9 MB so it fits a
   16 MB flash part (`CLAIMED`).
3. **Memory-hierarchy mapping**: activations and norm weights in ~512 KB SRAM,
   the ~3.9 M-parameter dense core and output head in 8 MB PSRAM, the PLE
   table memory-mapped in flash (`CLAIMED`).
4. **A modified `llama2.c` inference loop** with ESP32-S3 hardware-aware
   optimizations, building on earlier work by DaveBben.

This is a genuine engineering milestone — roughly 100× more parameters than
the prior ESP32 record (~260 K) — but the model is a toy: it generates short
children's stories and, per its own author, "will not answer questions, follow
instructions, write code, or know facts" (`CLAIMED`).

**Bottom line for RuView**: the demo does not run on RuView production nodes
as-is (our fleet is 8 MB/4 MB flash; the demo needs 16 MB), it contends
directly with the CSI sensing workload for both cores and PSRAM, and a
TinyStories model has no useful task capability for RF perception. The
*techniques*, however — flash-resident memory-mapped quantized weights, PLE-
style table lookup, ESP-DSP SIMD kernels, dual-core scheduling — are directly
transferable to RuView's own edge models and are the durable value of this
research. ADR-322 and ADR-328 turn those two conclusions into decisions.

## 2. Landscape and timeline

| Year | Project | Model | Claimed throughput | Hardware | Notes |
|---|---|---|---|---|---|
| 2023 | `karpathy/llama2.c` | any Llama-2-architecture checkpoint | n/a (host) | PC | Single-file C inference engine; upstream of everything below |
| 2024 | `DaveBben/esp32-llm` | 260 K params (tinyllamas, TinyStories) | 19.13 tok/s (`CLAIMED`) | ESP32-S3FH4R2, 2 MB PSRAM | First notable ESP32 LLM port; ESP-DSP SIMD dot products, both cores, 240 MHz CPU, 80 MHz PSRAM, enlarged instruction cache |
| 2026 | `slvDev/esp32-ai` ("The Stack") | 28.9 M params, 4-bit, 14.9 MB, TinyStories | 9.5–9.88 tok/s end-to-end, ~94.9 ms/token compute (`CLAIMED`) | ESP32-S3, 512 KB SRAM, 8 MB PSRAM, **16 MB flash** | First known application of Gemma-3n-style PLE at this scale of hardware; MIT license; SHA-256-verified model fetch |
| 2026 | `wladimiravila/esp32s3-distributed-ai` | 56 M params split across 3 boards | not verified by us | 3× ESP32-S3 over ESP-NOW | "Split-PLE + KV cache, fully offline" (`CLAIMED`); shows the scaling direction |

Context for scale: a 28.9 M-parameter model is ~4 orders of magnitude smaller
than frontier LLMs and ~2 orders smaller than the smallest generally useful
instruction-following models (~1–3 B). TinyStories (Eldan & Li, 2023,
arXiv:2305.07759) exists precisely because sub-100 M models can produce
coherent English only when the training distribution is radically constrained
(vocabulary of a young child, short narrative form).

## 3. Technique deep dive

### 3.1 Per-Layer Embeddings (PLE) / flash-resident lookup tables

Gemma 3n introduced PLE to cut the *resident* memory footprint of a model:
a large share of parameters is restructured into per-layer embedding tables
that are gathered by token id rather than multiplied against activations.
Gathers are sparse — a handful of rows per token — so the table can live in
slow, cheap storage (here: memory-mapped SPI flash via the ESP32-S3 MMU)
without putting flash bandwidth on the critical path. `esp32-ai` claims only
~450 bytes of table reads per token (`CLAIMED`), which at ~10 tok/s is a
trivial ~4.5 KB/s of flash read traffic.

Why this matters generally: it converts the dominant constraint from
*RAM capacity* to *flash capacity*, and flash is the cheapest memory on these
parts. The compute-active core shrinks to ~3.9 M parameters (~2 MB at 4-bit),
which fits comfortably in PSRAM.

### 3.2 Quantization

- 4-bit weight quantization: 28.9 M params → 14.9 MB total artifact
  (`CLAIMED`). Sub-byte weights on Xtensa cost unpack instructions; the
  scheme's viability at 9.5 tok/s suggests unpacking is amortized inside the
  SIMD kernels.
- RuView precedent: ADR-175 measured INT8 quantization of a RuView pose model
  and is our internal quality bar for how quantization claims must be
  validated (accuracy delta on a held-out split, not vibes).

### 3.3 Memory-hierarchy mapping (claimed layout of `esp32-ai`)

| Tier | Size | Speed class | Contents |
|---|---|---|---|
| Internal SRAM | 512 KB | fastest | activations, normalization weights, hot scratch |
| Octal/Quad PSRAM | 8 MB | ~80 MHz SPI | dense transformer core (~3.9 M params) + output head, KV cache |
| SPI flash (memory-mapped) | 16 MB | slowest | 25 M-param PLE table (14.9 MB artifact total) |

### 3.4 Compute optimizations (from `esp32-llm`, inherited by successors)

- ESP-DSP dot-product kernels using the ESP32-S3's PIE 128-bit SIMD
  extensions (the S3's differentiator vs. plain ESP32/C-series).
- Both Xtensa LX7 cores active during matmuls (second core normally runs the
  WiFi stack — these demos do not run WiFi during inference).
- 240 MHz CPU, 80 MHz PSRAM clock, enlarged instruction cache.
- DaveBben's measured jump to 19.13 tok/s on a 260 K model (`CLAIMED`) came
  primarily from SIMD + dual-core, an indication of how memory- and
  compute-bound the naive loop is.

### 3.5 What the model actually does

TinyStories-class models emit grammatical, mostly-coherent short fiction.
They have no instruction following, no factual recall, no structured-output
reliability. Every secondary source and the author agree on this. Any plan
that assumes "small LLM = small ChatGPT" is wrong at this parameter scale.

## 4. Fit analysis against RuView hardware and workload

### 4.1 Flash: the demo does not fit our fleet

RuView production partition table (ADR-045, `partitions_display.csv`):
8 MB flash = 2× 2 MB OTA app slots + 1.875 MB SPIFFS + NVS/PHY/otadata.
The 4 MB variant (issue #265) is tighter still. A 14.9 MB model artifact
**cannot be stored on either** — not in SPIFFS, not memory-mapped, not at
all (`SYNTHETIC`, from partition arithmetic). Direct replication requires a
16 MB-flash S3 variant (e.g. N16R8 modules), which is a *hardware SKU change*,
not a firmware change.

Fitting inside the existing 1.875 MB SPIFFS would cap a 4-bit model near
~3.5 M total parameters (`SYNTHETIC`) — a regime where even TinyStories
coherence degrades sharply.

### 4.2 RAM: feasible in isolation, contended in practice

Current firmware budget (firmware README): ~35 KB SRAM used with ~280 KB SRAM
free; 640 KB PSRAM for WASM arenas with ~7.36 MB PSRAM free. A ~2 MB dense
core + KV cache would fit free PSRAM (`SYNTHETIC`). But:

### 4.3 Compute and scheduling: direct conflict with the day job

The published demos own **both cores** and disable radio work during
inference. RuView nodes run WiFi + 20 Hz CSI capture on core 0 and the
Tier 1–2 DSP pipeline (plus WASM3 modules) on core 1 (ADR-039). CSI capture
is the product; a micro-LLM competing for core 1 and PSRAM bandwidth would
degrade the primary sensing path. On-node LLM inference is therefore only
plausible as (a) a mutually exclusive duty mode, or (b) a dedicated companion
node that does no CSI capture.

### 4.4 ESP32-C6 is out of scope

The C6 (ADR-110 research target) is a single-core RISC-V without the S3's
PIE SIMD and typically without PSRAM — no published micro-LLM result exists
for it and the arithmetic is unfavorable. S3-only.

### 4.5 Capability fit: what would a node even say?

RuView's edge outputs are compact typed packets (32-byte vitals, ADR-039) and
server-side semantics. Candidate LLM uses on-node and their honest status:

| Candidate use | Verdict | Why |
|---|---|---|
| Natural-language event narration ("someone fell in the kitchen") | Weak | A template engine does this deterministically in <1 KB; a TinyStories-class model adds hallucination risk to a safety-relevant path with zero benefit |
| On-device Q&A / configuration assistant | No | Requires instruction following; not available at this scale |
| Semantic compression of CSI events | Research-only | Would require a custom domain model trained on RuView event→text pairs; no dataset exists today |
| Demonstrating the platform's headroom / marketing | Real but bounded | Legitimate as a clearly labeled demo, never as a shipped claim |

The durable value is the **memory architecture**, not the language model:
flash-resident memory-mapped quantized weights + PLE-style tables would let
RuView's *own* future neural presence/pose edge models grow well beyond
current RAM budgets on 8 MB parts (ADR-328).

## 5. Risks and honest-labeling obligations

- All throughput/size numbers above are `CLAIMED` until reproduced on RuView
  silicon with a captured boot/runtime log (witness-log discipline, cf.
  ADR-028/WITNESS-LOG-028). A successful build or QEMU run is not hardware
  evidence.
- Third-party code and model weights (`esp32-ai` MIT; `llama2.c` MIT;
  `esp32-llm` license to be verified at intake) require license review before
  entering any RuView tree; model weights are unreviewed generated artifacts
  and must not be committed (repository non-negotiables).
- Never present micro-LLM output as perception evidence. It must not enter
  the evidence engine (ADR-304) or any published sensing claim.
- Marketing risk: "LLM on our sensor" invites camera-grade-style
  overclaiming. Any public statement must carry the demo framing and
  evidence tags.

## 6. Recommended validation plan (if the ADR-322 spike is funded)

1. Acquire an ESP32-S3 N16R8 (16 MB flash / 8 MB PSRAM) devkit — companion
   hardware, not a fleet SKU.
2. Reproduce `esp32-ai` unmodified; capture serial witness log (boot, model
   SHA-256, tok/s over ≥500 tokens). This produces the first `MEASURED` row.
3. Measure tok/s with WiFi stack active vs. disabled to quantify the
   coexistence penalty.
4. Attempt a ≤3.5 M-parameter retrain fitting the 8 MB partition map, solely
   to characterize the quality cliff (`SYNTHETIC` sizing above).
5. Separately benchmark the transferable kernels (ESP-DSP SIMD matmul,
   memory-mapped flash weight streaming) against RuView's own edge-model
   workloads — this feeds ADR-328 regardless of the LLM outcome.

## 7. Sources

- Geeky-Gadgets coverage (task prompt): https://www.geeky-gadgets.com/run-llm-esp32-microcontroller/
- Project repo: https://github.com/slvDev/esp32-ai (MIT)
- Prior art: https://github.com/DaveBben/esp32-llm ; https://github.com/karpathy/llama2.c
- Distributed follow-on: https://github.com/wladimiravila/esp32s3-distributed-ai
- Tom's Hardware: https://www.tomshardware.com/tech-industry/artificial-intelligence/ai-developer-runs-28-9-million-parameter-model-on-usd10-esp32-s3-microcontroller-uses-googles-per-layer-embeddings-technique-stores-table-on-16mb-flash-memory
- The Register: https://www.theregister.com/edge-and-iot/2026/08/04/dev-proves-llms-will-run-on-anything-even-a-10-microcontroller/5283088
- CNX Software: https://www.cnx-software.com/2026/08/03/28-9m-parameter-llm-runs-locally-on-esp32-s3-at-9-tokens-s/
- Hackster: https://www.hackster.io/news/running-a-28-9m-parameter-llm-on-an-8-microcontroller-173f1f370708
- XDA: https://www.xda-developers.com/someone-squeezed-a-289m-llm-onto-an-esp32-s3-and-so-can-you/
- TinyStories: Eldan & Li 2023, arXiv:2305.07759
- Gemma 3n / Per-Layer Embeddings: Google AI developer announcements, 2025
