# ADR-353: fal.ai synthetic visual teacher LoRA

- **Status**: Accepted
- **Date**: 2026-09-02
- **Deciders**: ruv
- **Owners**: RuView visualization / neural-styling maintainers
- **Tags**: fal, lora, synthetic-visualization, display-only, cost-control
- **Parent**: None
- **Extends**: None
- **Supersedes**: None

## Decision

RuView will use one real, fal.ai-trained LoRA fine-tune (a "synthetic visual
teacher") to steer fal.ai image/video generation toward a consistent
"RuView-style" room visualization — glowing pose overlays, WiFi signal arcs,
vitals readout panels, dark tech aesthetic — instead of relying on ad hoc
prompt engineering against a stock model.

**The architecture boundary is non-negotiable:**

> The implementation boundary is now clear: fal.ai will train an optional
> synthetic visual teacher, while RuView's measured RF, pose, vitals,
> identity, and confidence remain local and authoritative.

Concretely:

- The trained model produced here is ONLY ever allowed to generate
  supplementary synthetic visual/pixel output for display or broadcast.
- It must NEVER be a source of, or influence, any measured/derived value: RF/
  CSI measurements, pose estimates, vitals (heart rate/breathing rate),
  identity, or confidence/quality scores. Those stay local, deterministic,
  and authoritative — the existing `wifi-densepose-engine` /
  `ruview-twin` / `ruview-witness` stack — completely untouched by this work.
- Any output this model produces must be clearly framed as SYNTHETIC per this
  repo's rule: "Never present WiFi sensing as camera-grade."
- This model has no read or write path into the RF/pose/vitals/identity
  pipeline. It consumes only a text prompt (and optionally a reference
  image) and returns pixels for display.

**Evidence status:** the training data extraction, training submission
script, and validation script are real and committed. The training run
completed and validation succeeded — trained model id, actual spend, and the
validation sample are recorded in "Result" below. Actual-vs-billed cost
reconciliation against the fal.ai account dashboard remains UNMEASURED (the
figure below is a documented-pricing estimate).

## Context

A concurrent workstream (`ruview-fal-room-viz` / `ruview-room-viz-h3-upgrade`)
generated real fal.ai room-visualization videos across several style presets
(architectural, abstract, branded, and an "h3" refresh: architectural,
cinematic, minimal, abstract) using stock fal.ai video models driven by
prompt engineering alone. A separate concurrently-running agent
("realtime-dual-renderer") is building a neural-styling layer that needs a
*specific* trained model id to call, rather than continuing to rely on prompt
engineering against a generic model.

This ADR's job is narrow: turn the real captured style presets into a real
trained fal.ai LoRA, verify it actually works, and hand off the model id. It
does not implement the renderer integration — that is
`realtime-dual-renderer`'s scope — and it does not touch the measured
RF/pose/vitals/identity/confidence pipeline in any way.

## Training data

Source: the real fal.ai-generated room-visualization MP4s already captured
this session (read from `/tmp/room-viz-*/room-viz.mp4` on `ruvultra`, one per
style preset — not committed to the repo; the repo has no camera or person
data in these clips, they are synthetic renders). `scripts/fal-visual-teacher/extract_frames.py`
extracts one JPEG frame every 1.5 seconds from each of the 7 real style-preset
videos (architectural, abstract, branded, h3-architectural, h3-cinematic,
h3-minimal, h3-abstract), producing 27 frames total, each paired with a
caption `.txt` describing the shared RuView visual motifs. Frames and the
training zip are gitignored (`scripts/fal-visual-teacher/training-data/`,
`training-data.zip`) — they are regenerable from the source MP4s and are not
committed, consistent with the repo's no-generated-artifacts rule.

## Training endpoint and pricing (verified 2026-09-02)

- **Endpoint**: `fal-ai/flux-lora-fast-training` — a real, currently-listed
  fal.ai LoRA training endpoint for FLUX.1, verified against
  `https://fal.ai/models/fal-ai/flux-lora-fast-training/api`.
- **Pricing**: $2 per training run at the default 1000 steps, scaling
  linearly with steps (verified against fal.ai's own pricing text on the
  model page).
- **Mode**: submitted with `is_style: true` (style LoRA, not
  subject/character LoRA) and explicit per-frame caption files, so fal.ai's
  auto-captioning/segmentation is skipped in favor of our own captions.
- **Inference endpoint** (validation only): `fal-ai/flux-lora`, $0.035 per
  megapixel, accepting `loras: [{ path, scale }]` pointing at the trained
  `diffusers_lora_file` URL.

## Budget

- **Original cap**: $15 total (this task's cap only; not shared with other
  concurrent tasks' budgets this session).
- **Raised cap**: the user explicitly raised this task's cap to **$100** on
  2026-09-02, mid-run, as real headroom for a larger/better run or retries —
  not an instruction to spend all of it.
- **Planned spend**: ~$2 for the training run (1000 steps, default) + ~$0.04
  per validation inference image.
- **Actual spend**: ~$2.04 (one 1000-step training run at $2/run + one
  landscape_16_9 validation image at $0.035/megapixel, rounded up to 1
  megapixel). This is a documented-pricing estimate (`CLAIMED`), not yet
  reconciled against the fal.ai account billing dashboard (`MEASURED`).
  Comfortably under both the original $15 cap and the raised $100 cap.
- **Why no larger follow-up run**: the first validation image already showed
  a clean, on-style result (glowing pose overlay, WiFi arcs, vitals panels,
  dark aesthetic, all present and legible) on the first attempt. Spending
  further into the raised budget for marginal improvement was judged not
  worth it against a result that already meets the bar; the fallback path in
  "Consequences" was not needed.

## Result

- **Trained model id / LoRA weights URL**:
  `https://v3b.fal.media/files/b/0aa8c60d/OjaBs6Gi8pMhZqMo79iym_pytorch_lora_weights.safetensors`
  (131 MB `diffusers_lora_file`, paired `config.json` at
  `https://v3b.fal.media/files/b/0aa8c60d/-GU6dd_2ZFqq18I0CuPOB_config.json`).
  Use with `fal-ai/flux-lora` (or any fal.ai FLUX.1 endpoint accepting a
  `loras: [{ path, scale }]` array) — trigger word `ruviewstyle`. Full
  training record: `scripts/fal-visual-teacher/output/training-result.json`
  (gitignored — regenerate by re-running `train_lora.py`, or copy the URL
  above).
- **Training request id**: `01a060b2-c940-7972-abf9-ea48847eb2ae` (fal.ai
  `fal-ai/flux-lora-fast-training`, 1000 steps, `is_style: true`, 27 training
  images, completed in 376s on an H100/H200 worker).
- **Actual spend**: ~$2.04 (see Budget above).
- **Validation sample**: `assets/fal-visual-teacher-sample.png` (+ sibling
  `assets/fal-visual-teacher-sample.json` with the exact prompt, LoRA scale
  1.0, and source fal.ai URL) — a real `fal-ai/flux-lora` inference call
  using the trained LoRA, generated 2026-09-02. The image shows a glowing
  cyan pose skeleton, WiFi signal arcs, vitals-style readout panels, and a
  HUD-style TV overlay in a dark living room — confirming the LoRA learned
  and reproduces the intended RuView visual motifs.
- **Consumer**: `realtime-dual-renderer` (separate worktree
  `~/projects/ruview-worktrees/realtime-render`) — notified directly with the
  model id above.

## Consequences

A real trained LoRA gives the display layer a specific, repeatable style
target instead of prompt engineering against a stock model, at a bounded and
small real cost ($2 training + cents per generated image). Because the model
only ever produces pixels for a prompt, and has no schema path into any
measured/derived RuView value, it cannot silently become a second source of
truth for pose, vitals, identity, or confidence — that boundary is structural
(no shared types, no shared data path), not just documented convention.

If the trained LoRA does not meaningfully improve style consistency over the
stock model (a genuine risk with 27 training images and default steps), the
fallback is to keep using prompt engineering against the stock model and note
that outcome honestly here rather than claim a training win that isn't real.

## References

- `scripts/fal-visual-teacher/extract_frames.py` — real frame extraction
- `scripts/fal-visual-teacher/train_lora.py` — real training submission
- `scripts/fal-visual-teacher/validate.py` — real validation inference
- [fal.ai Flux LoRA Fast Training](https://fal.ai/models/fal-ai/flux-lora-fast-training)
- [fal.ai FLUX.1 with LoRAs (inference)](https://fal.ai/models/fal-ai/flux-lora)
