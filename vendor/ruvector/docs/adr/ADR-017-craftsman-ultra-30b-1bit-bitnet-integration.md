# ADR-017: Craftsman Ultra 30b 1bit — BitNet Integration with RuvLLM

**Status:** Proposed
**Date:** 2026-02-03
**Decision Makers:** Ruvector Architecture Team
**Technical Area:** 1-Bit LLM Inference / MoE Architecture / CPU-Native Serving

---

## Context and Problem Statement

Large language models require substantial GPU resources for inference, limiting deployment to cloud environments and specialized hardware. Recent advances in 1-bit quantization — specifically Microsoft Research's BitNet b1.58 — demonstrate that ternary-weight models ({-1, 0, +1}) can match full-precision performance at 3B+ parameters while enabling CPU-only inference at human-readable speeds.

Concurrently, Zhipu AI's GLM-4.7-Flash introduces a 30B-A3B Mixture-of-Experts architecture that activates only ~3B parameters per token while storing 30B total knowledge, achieving strong coding and agentic benchmarks (SWE-bench Verified: 59.2%, LiveCodeBench v6: 64.0%) with 200K context.

**Craftsman Ultra 30b 1bit** is a proposed model that combines these two paradigms: a 30B-A3B MoE architecture with native BitNet b1.58 ternary quantization, purpose-built for CPU inference within the RuvLLM serving runtime. This ADR evaluates the integration path, architectural decisions, and trade-offs.

### Strategic Goal

Deliver a 30B-class coding/agentic model that runs entirely on consumer CPUs (no GPU required) at 5-15 tokens/second decode, with memory footprint under 8GB, integrated into the RuvLLM + Ruvector ecosystem with SONA self-learning capabilities.

---

## Decision Drivers

### Performance Requirements

| Metric | Target | Rationale |
|--------|--------|-----------|
| Decode throughput (CPU) | 5-15 tok/s | Human-readable speed per BitNet 100B benchmarks |
| Prefill latency (1K tokens) | <2s | Interactive coding assistant responsiveness |
| Memory footprint (model) | <8 GB | Fits in 16GB system RAM with OS + KV cache |
| Memory footprint (KV cache, 4K ctx) | <2 GB | Q8 KV cache for 4096-token context |
| Active parameter GEMM | Addition-only | BitNet eliminates multiplication in W×A |
| Energy per inference | <0.05J | BitNet CPU efficiency benchmarks |

### Architecture Requirements

- **MoE routing must remain full-precision**: Expert selection requires accurate gating scores
- **Expert weights are ternary**: Each expert's linear layers use BitLinear (W1.58A8)
- **Activations quantized to INT8**: Per-token absmax scaling
- **Shared layers (embeddings, LM head) remain FP16**: Critical for quality preservation
- **GGUF-compatible**: Must serialize to/load from GGUF v3 format with custom metadata

### Ecosystem Requirements

- Integrate with RuvLLM's existing backend abstraction (`backends/mod.rs`)
- Leverage existing GGUF parser (`gguf/parser.rs`, `gguf/quantization.rs`)
- Support SONA learning loops for per-session adaptation
- Compatible with Claude Flow agent routing for task delegation
- NAPI bindings for Node.js consumption via `npm/packages/ruvllm`

---

## Research Summary

### BitNet b1.58 Architecture

**Source**: Microsoft Research, "The Era of 1-bit LLMs" (Feb 2024), bitnet.cpp (Oct 2024)

BitNet b1.58 replaces standard `nn.Linear` with `BitLinear` layers:

```
Forward Pass:
  1. W_ternary = RoundClip(W / (gamma + epsilon), -1, 1)
     where gamma = mean(|W|) (absmean quantization)
  2. X_int8 = Quant(X, absmax)  (per-token 8-bit activation)
  3. Y = W_ternary @ X_int8      (integer addition only, no multiplication)
  4. Y_float = Dequant(Y)         (rescale to float)
```

**Key properties:**
- Weights: ternary {-1, 0, +1} → 1.58 bits per parameter
- Activations: INT8 per-token (absmax scaling)
- Matrix multiply becomes **addition and subtraction only** (no FP multiply)
- Zero weights enable **feature filtering** (sparse activation within dense layers)
- Must be **trained from scratch** — post-training quantization to 1-bit destroys quality

**Inference kernels (bitnet.cpp):**

| Kernel | Method | Compression | Best For |
|--------|--------|-------------|----------|
| I2_S | 2-bit pack, unpack-and-multiply | 2 bits/weight | Bandwidth-limited |
| TL1 | 2-weight → 4-bit LUT index | 2 bits/weight | Balanced CPU |
| TL2 | 3-weight → 5-bit LUT index | 1.67 bits/weight | Memory-limited |

**CPU performance (bitnet.cpp benchmarks):**

| Platform | Speedup vs FP16 | Energy Reduction |
|----------|-----------------|-----------------|
| ARM (NEON) | 1.37x – 5.07x | 55-70% |
| x86 (AVX2) | 2.37x – 6.17x | 72-82% |
| x86 (AVX512) | ~6x+ | ~85% |

### GLM-4.7-Flash Architecture

**Source**: Zhipu AI / Z.AI (Jan 2026)

| Property | Value |
|----------|-------|
| Total parameters | ~30B (31B reported) |
| Active parameters | ~3B (A3B) |
| Architecture | Mixture of Experts (MoE) |
| Shared layers | ~2B parameters |
| Expert layers | ~28B (distributed across experts) |
| Context window | 200K tokens (MLA-based) |
| Training data | 15T general + 7T reasoning/code tokens |
| Attention | Multi-head Latent Attention (MLA) with QK-Norm |
| Activation | SwiGLU |
| Position encoding | RoPE |
| Speculative decoding | Multi-Token Prediction (MTP) layer |
| Reasoning | Interleaved + Retention-Based + Round-Level |

**Benchmark performance:**

| Benchmark | Score |
|-----------|-------|
| AIME 25 | 91.6% |
| GPQA | 75.2% |
| SWE-bench Verified | 59.2% |
| LiveCodeBench v6 | 64.0% |
| HLE | 14.4% |
| tau2-Bench | 79.5% |

### RuvLLM Current Capabilities (Relevant)

- **GGUF v3 parser**: Full format support including IQ1_S (1.56 bits/weight, type 19)
- **Quantization pipeline**: Q4_K_M, Q5_K_M, Q8_0, F16 (no native ternary training)
- **Backends**: Candle (Metal/CUDA), mistral-rs (PagedAttention), CoreML (ANE)
- **No CPU-optimized ternary kernel**: Current backends target GPU acceleration
- **SIMD kernels**: Existing NEON/SSE4.1/AVX2 infrastructure in `crates/ruvllm/src/kernels/`
- **MicroLoRA**: Rank 1-2 adapters with <1ms adaptation (compatible with BitNet)
- **SONA**: Three-tier learning (instant/background/deep) — can drive ternary adapter training

### RuvLLM RLM Training Stack (Reusable for Distillation)

RuvLLM contains a mature reinforcement-learning-from-model-feedback (RLM) training stack that directly accelerates Craftsman Ultra distillation. These components are production-tested and reduce net-new code by ~70%.

**GRPO — Group Relative Policy Optimization** (`training/grpo.rs`, 897 lines)
- Critic-free RL: computes relative advantages within sample groups
- Adaptive KL divergence penalty (`kl_target`, `clip_range`) controls teacher-student divergence
- PPO-style clipping prevents catastrophic updates
- Preset configs: `GrpoConfig::stable()` (safe distillation), `GrpoConfig::for_tool_use()` (expert routing)
- Thread-safe batch processing via `RwLock<VecDeque<SampleGroup>>`

**RealContrastiveTrainer** (`training/real_trainer.rs`, 1000 lines)
- Candle-based training loop with GGUF model loading and GGUF weight export
- Combined loss: Triplet (margin) + InfoNCE (contrastive) + GRPO reward scaling
- AdamW optimizer with gradient clipping, LR warmup, checkpointing
- `GrpoEvaluator` computes per-prediction rewards (1.0 correct, -0.5 wrong)
- Metal/CUDA acceleration via Candle device dispatch

**MicroLoRA + EWC++ Training Pipeline** (`lora/training.rs`, 798 lines)
- Single-example gradient computation (batch_size=1 for real-time)
- EWC++ regularizer: `λ/2 * Σ F_i * (w_i - w*_i)²` prevents catastrophic forgetting
- Fisher diagonal tracking with exponential decay (`fisher_decay: 0.999`)
- 7 learning rate schedules (Cosine, OneCycle, Step, etc.)
- Async adaptation with buffered gradient accumulation

**Memory Distillation** (`reasoning_bank/distillation.rs`, 856 lines)
- Compresses trajectories to `KeyLesson` objects with semantic embeddings
- Smart extraction: explicit lessons, implicit patterns, error patterns, recovery patterns
- Semantic deduplication (Jaccard + cosine similarity, threshold 0.85)
- Quality-gated: only trajectories above `min_quality_threshold` are preserved

**Policy Store** (`policy_store.rs`, 474 lines)
- Ruvector-backed semantic policy persistence with HNSW indexing
- Policy types: `Quantization`, `Router`, `Ewc`, `Pattern`
- Per-layer `QuantizationPolicy` with precision, activation thresholds, quality-latency tradeoff
- Policy source tracking: `InstantLoop`, `BackgroundLoop`, `DeepLoop`, `Federated`

**Contrastive Training** (`training/contrastive.rs`, 634 lines)
- Two-stage: Triplet Loss (margin=0.5) + InfoNCE (temperature=0.07)
- 13 agent types with 1,078 training triplets (578 base + 500 hard negatives)
- Hard negative mining at 48.4% ratio (Claude-generated confusing pairs)
- Proven 100% routing accuracy with hybrid keyword-first + embedding fallback

---

## Considered Options

### Option A: Post-Training Quantization of GLM-4.7-Flash (PTQ Tiers)

Take the existing BF16 GLM-4.7-Flash weights and quantize to low-bit formats without full distillation training.

**Critical distinction — IQ1_S ≠ BitNet b1.58:**

| Property | GGUF IQ1_S | BitNet b1.58 |
|----------|-----------|--------------|
| Encoding | Codebook-based importance quantization | Ternary {-1, 0, +1} via absmean |
| Bits/weight | 1.56 bpw | 1.58 bpw |
| Inference | **Dequantize → FP multiply** | **Integer addition only (no multiply)** |
| Speed benefit | Memory bandwidth only | Bandwidth + compute (multiplication-free) |
| How obtained | Post-training quantization | Trained from scratch or distilled |
| Quality at 7B | Near-random / broken outputs | Matches FP16 |

**Existing GLM-4.7-Flash GGUF quantizations available** (community-published):

| Repository | Lowest Quant | Size | Notes |
|-----------|-------------|------|-------|
| [bartowski/zai-org_GLM-4.7-Flash-GGUF](https://huggingface.co/bartowski/zai-org_GLM-4.7-Flash-GGUF) | IQ2_XXS (2.06 bpw) | 7.62 GB | No IQ1_S published |
| [unsloth/GLM-4.7-Flash-GGUF](https://huggingface.co/unsloth/GLM-4.7-Flash-GGUF) | UD-Q2_K_XL (2.7 bpw dynamic) | ~11 GB | Dynamic quant, recommended |
| [ngxson/GLM-4.7-Flash-GGUF](https://huggingface.co/ngxson/GLM-4.7-Flash-GGUF) | Q4_K_M (4.5 bpw) | 18.1 GB | 55 variants available |

**No IQ1_S quantization** has been published for GLM-4.7-Flash by any community quantizer — this itself is a signal (too aggressive for practical use).

**Sub-options ranked by increasing effort:**

**Sub-option 0A: Download existing IQ2_XXS GGUF**
- Download bartowski's IQ2_XXS at 7.62 GB
- Cost: $0, time: 5 minutes (just download)
- Quality: ~75-80% of FP16 (2.06 bpw is usable per community reports)
- NOT 1-bit, NOT BitNet — just aggressive 2-bit compression
- RuvLLM gap: IQ2_XXS dequantization not implemented (falls to error catch-all in `quantization.rs:358`)
- RuvLLM Q2_K dequantization IS implemented and works

**Sub-option 0B: Quantize to IQ1_S via llama.cpp**
- Run `llama-quantize GLM-4.7-Flash-F16.gguf IQ1_S` with importance matrix
- Cost: $0, time: ~30 minutes on CPU
- Quality: **SEVERE degradation** — blind testing shows IQ1_S is "broken rather than just bad" on 7B; outputs contain garbled text despite acceptable perplexity scores. 30B MoE may survive better due to parameter redundancy, but expert routing is highly sensitive to weight perturbation
- RuvLLM gap: IQ1_S dequantization not implemented (`quantization.rs:358` catch-all)
- Does NOT achieve BitNet multiplication-free inference

**Sub-option 0C: PT-BitNet ternary PTQ** (per [PT-BitNet paper](https://www.sciencedirect.com/science/article/abs/pii/S089360802500735X))
- Apply absmean ternary quantization (BitNet's native method) to pre-trained weights with calibration data
- Cost: **$0** (runs locally on Mac Studio via mmap + Metal; 1-4 hours wall time)
- Alternative: ~$50-200 on cloud GPU if no local Apple Silicon hardware
- Quality: ~55-65% downstream accuracy (PT-BitNet reports 61% on 70B; GLM-4.7-Flash's 30B-A3B may differ)
- THIS IS proper BitNet ternary format → **enables multiplication-free inference with AD-4 kernels**
- Requires implementing absmean ternary quantizer (~200-300 lines of new code)
- Requires calibration dataset (WikiText-2 or similar, ~1M tokens)
- Mac Studio M4 Max 64GB+ or M3 Ultra 96GB+ recommended (see AD-18)

**Sub-option 0D: BitDistill Lite (10B tokens)** (per [BitDistill paper](https://arxiv.org/html/2510.13998v1))
- 3-stage: SubLN insertion → 10B-token continued pre-training → KL + attention distillation
- Cost: ~$200-500 (8× GPU hours on Mi300X/A100 class)
- Quality: **~90-95% of FP16** (BitDistill reports 88.17% vs 88.01% FP16 on MNLI at 0.6B)
- Near-full quality recovery with only 10B tokens (vs 200B+ for Phase 1 full distillation)
- Requires SubLN module insertion + distillation fine-tuning loop
- Bridges gap between pure PTQ and full expert distillation (Phase 1)

**Summary comparison:**

| Sub-option | Cost | Time | Quality (est.) | BitNet Speedup | RuvLLM Ready |
|-----------|------|------|---------------|----------------|-------------|
| 0A: IQ2_XXS download | $0 | 5 min | ~75-80% | No | No (missing dequant) |
| 0B: IQ1_S quantize | $0 | 30 min | ~40-50% | No | No (missing dequant) |
| 0C: PT-BitNet PTQ | **$0 (Mac Studio)** | 1-4 hrs | ~55-65% | **Yes** | Needs quantizer impl |
| 0D: BitDistill Lite | $0 local / ~$300 cloud | 2-4 wks / 1-2 days | ~90-95% | **Yes** | Needs SubLN + KD loop |

**Pros (of PTQ approach generally):**
- Immediate or near-immediate results ($0-$300, minutes to days)
- No large-scale training infrastructure
- Validates inference pipeline and kernels before investing in full distillation
- Sub-option 0C produces genuine BitNet ternary format for kernel development

**Cons:**
- Sub-options 0A/0B: Quality too degraded for production coding tasks
- Sub-options 0A/0B: No BitNet multiplication-free inference (still dequant-then-multiply)
- Sub-option 0C: Significant quality loss (~35-45%) vs teacher — adequate for kernel validation, not production
- Sub-option 0D: Requires non-trivial training code (SubLN, KD loss) but much less than full Phase 1
- IQ1_S blind test results: statistically indistinguishable from random on smaller models

**Verdict: Recommended as Phase 0 rapid prototype** — Sub-option 0C (PT-BitNet PTQ) is the optimal entry point: $100, 2-4 hours, produces genuine BitNet ternary format for kernel development and inference validation. Sub-option 0D (BitDistill Lite) bridges to Phase 1 if higher quality is needed before committing to full expert distillation. Sub-options 0A/0B are useful only as baselines for comparison.

### Option B: Native BitNet Training of GLM-4.7-Flash Architecture (Full)

Train Craftsman Ultra 30b 1bit from scratch using BitNet b1.58 methodology on the GLM-4.7-Flash MoE architecture.

**Approach:**
1. Implement BitLinear layers for all expert MLPs and attention projections
2. Keep MoE router, embeddings, and LM head in FP16
3. Train on 4T+ tokens with ternary weight updates via straight-through estimator
4. Export to custom GGUF with ternary tensor metadata

**Pros:**
- Maximum quality — matches FP16 at 3B+ active parameter scale
- True multiplication-free inference for expert forward passes
- Full TL1/TL2 kernel optimization possible
- Scientifically validated approach (BitNet b1.58 2B4T results)

**Cons:**
- Massive training compute: estimated 4,000-8,000 A100-hours for 4T tokens
- Requires custom training framework (BitNet + MoE + MLA integration)
- 6-12 month timeline for training pipeline + training run
- No pre-existing GLM-4.7-class BitNet training recipe

**Verdict: Recommended long-term** — Highest quality but requires significant investment.

### Option C: Hybrid Approach — BitNet Distillation from GLM-4.7-Flash (RLM-Accelerated)

Use knowledge distillation to transfer GLM-4.7-Flash capabilities into a BitNet architecture, reducing training cost by 5-10x. **Leverages the existing RLM training stack** to eliminate ~70% of net-new training code.

**Approach:**
1. Initialize Craftsman Ultra with GLM-4.7-Flash architecture (30B-A3B MoE)
2. Replace all expert linear layers with BitLinear (ternary {-1, 0, +1})
3. Keep router, embeddings, LM head in FP16
4. **Extend `RealContrastiveTrainer`** with KD loss (KL div + hard-label CE) replacing triplet+InfoNCE
5. **Use `GrpoOptimizer`** for per-expert quality rewards during distillation — each `SampleGroup` maps to one expert's teacher vs student outputs
6. **Apply `EwcRegularizer`** across distillation phases to prevent early-trained experts from being overwritten
7. **Log distillation trajectories** to `MemoryDistiller` for quality tracking and `KeyLesson` extraction
8. **Persist per-layer ternary policies** via `PolicyStore` (quantization thresholds, scale distributions)
9. Export to GGUF with ternary tensor metadata and TL1/TL2 kernel hints via existing `GgufExportResult`

**RLM Component Reuse:**

| Existing Component | Reuse | Adaptation Needed |
|-------------------|-------|-------------------|
| `RealContrastiveTrainer` | Training loop, GGUF export, checkpointing | Replace triplet+InfoNCE with KD loss |
| `GrpoOptimizer` | Reward scaling, adaptive KL, PPO clipping | Map `SampleGroup` to per-expert outputs |
| `EwcRegularizer` | Fisher diagonal, forgetting prevention | Apply across expert distillation phases |
| `MemoryDistiller` | Trajectory compression, lesson extraction | Map `Verdict` to teacher-student quality delta |
| `PolicyStore` | Semantic policy persistence | Add `PolicyType::TernaryScale` for per-block absmean tracking |
| `ContrastiveTrainer` | Hard negative mining framework | Reuse for expert-routing contrastive pre-training |

**Pros:**
- 5-10x less compute than training from scratch (~800-1,600 A100-hours)
- **~70% existing code reuse** — only BitLinear forward/backward and MoE data loading are net-new
- Leverages GLM-4.7-Flash's proven architecture and routing
- GRPO's adaptive KL prevents ternary student from diverging too far from teacher
- EWC++ ensures sequential expert distillation doesn't corrupt earlier experts
- Teacher model provides strong supervision signal for ternary convergence
- Can incrementally improve with more distillation tokens
- `PolicyStore` enables learned per-layer quantization decisions
- Distillation quality tracked end-to-end via `MemoryDistiller` trajectory logging

**Cons:**
- Slight quality gap vs native training (estimated 2-5% on benchmarks)
- `RealContrastiveTrainer` embedding_dim (896) must scale to GLM-4.7-Flash hidden_size
- Teacher inference cost during distillation
- Distillation may not perfectly transfer MoE routing behavior

**Verdict: Recommended near-term** — Best balance of quality, cost, and timeline. RLM reuse eliminates the "custom framework" risk.

### Option D: BitNet Expert Replacement (Incremental, RLM-Accelerated)

Keep GLM-4.7-Flash structure but replace only the expert MLP layers with BitLinear, leaving attention in FP16. **Reuses existing RLM stack for the entire distillation loop.**

**Approach:**
1. Load GLM-4.7-Flash architecture
2. Replace expert FFN layers (gate_proj, up_proj, down_proj) with BitLinear
3. Keep attention (Q/K/V/O projections) in FP16
4. **Use `RealContrastiveTrainer` + `GrpoOptimizer`** for expert-only distillation (~200B tokens)
5. **Apply `EwcRegularizer`** to prevent expert N+1 distillation from corrupting expert N
6. Attention weights loaded directly from GLM-4.7-Flash (no distillation needed)
7. **Use contrastive pre-training** to validate MoE routing still selects correct experts after ternary conversion

**Pros:**
- Fastest path to working model
- Attention quality preserved exactly
- Expert FFN is 60-70% of active parameters — gets most BitNet benefits
- Simpler distillation (only FFN layers)
- Lower memory: ~5.5 GB for ternary experts + FP16 attention
- **Minimal net-new code**: BitLinear layer + GGUF ternary type only; training loop is 100% reused

**Cons:**
- Attention layers still require FP multiply (not fully multiplication-free)
- Mixed-precision inference path complexity
- ~40% of compute still in FP16 attention

**Verdict: Recommended as Phase 1** — Enables rapid prototyping and validation. RLM reuse makes this achievable with only ~30% new code.

---

## Decision

**Phased approach: A(0C) → RLM Refinement → D → C → B**

### Phase 0: PTQ Rapid Prototype (Option A, Sub-option 0C)
- **Timeline**: 1-2 weeks
- **Cost**: **$0** (runs entirely on Mac Studio locally)
- **Platform**: Mac Studio (M4 Max 64GB+ or M3 Ultra 96GB+)
- **Goal**: Produce a genuine BitNet ternary GGUF of GLM-4.7-Flash for kernel development, inference pipeline validation, and baseline quality measurement
- **Deliverables**:
  - PT-BitNet ternary quantized GLM-4.7-Flash GGUF file (~6-7 GB)
  - Absmean ternary quantizer implementation (~200-300 lines)
  - IQ1_S / BITNET_T158 dequantization kernel in RuvLLM
  - Baseline quality benchmarks (HumanEval, MMLU) to compare against Phase 1+
  - Functional TL1 kernel validated against ternary model
- **Expected quality**: ~55-65% of GLM-4.7-Flash (adequate for kernel validation, not production)
- **Key value**: De-risks Phase 1 by validating the entire inference pipeline (GGUF loading → ternary dequant → TL1 kernel → MoE routing → token generation) at zero cost before committing to $1,300+ distillation training
- **Why Mac Studio works**: Phase 0 is PTQ (no training loop) — just load FP16 weights via mmap, compute absmean per block, round to ternary, export. The absmean computation is trivial math; the bottleneck is memory bandwidth, not compute. Calibration forward pass uses Metal GPU acceleration via existing Candle integration.
- **Optional upgrade (0D)**: If 0C quality is too low for meaningful testing, apply BitDistill Lite (10B tokens, ~$300 cloud or ~$0 on Mac Studio over several weeks) to reach ~90-95% quality

### Phase 0.5: RLM Post-Quantization Refinement (NEW — Mac Studio, $0)
- **Timeline**: 1-3 weeks (overlaps with Phase 0 kernel development)
- **Cost**: **$0** (runs on Mac Studio, ~2-12 days training wall time with Metal; ~4-24 days SIMD-only)
- **Platform**: Mac Studio (same as Phase 0) — **supports both Metal GPU and pure SIMD/CPU modes** (see AD-20)
- **Goal**: Improve Phase 0 PTQ quality from ~55-65% to ~70-80% by training only the small FP16 components using the existing RLM stack — **no traditional distillation, no cloud GPU**
- **Approach**: Freeze ternary weights, train FP16 corrections using RLM components:
  1. **MicroLoRA adapters** (rank 1-2) on each expert FFN — adds small FP16 correction: `Y = BitLinear(X) + LoRA_B @ LoRA_A @ X`
  2. **Router fine-tuning** via ContrastiveTrainer — corrects misrouting caused by PTQ weight changes
  3. **Scale factor optimization** via GRPO rewards — per-block FP16 absmean scales are differentiable
  4. **EWC++ regularization** — prevents router fix from breaking already-good routing paths
  5. **Quality tracking** via MemoryDistiller — identifies worst-degraded experts for focused training
  6. **Policy persistence** via PolicyStore — stores optimized per-layer configurations
- **Trainable parameters**: ~200-400M (1-2% of 30B total) — router (~30M), MicroLoRA adapters (~50-100M), LM head (~150M), scale factors (~0.1M)
- **Training data**: 100M-500M tokens (sufficient for <400M trainable params)
- **Throughput**: ~500-1000 tok/s (Metal) or ~200-500 tok/s (NEON SIMD only) × 100M-500M tokens = **2-12 days (Metal) or 4-24 days (SIMD-only) on Mac Studio**
- **Deliverables**:
  - RLM-refined GGUF with ternary experts + optimized FP16 components
  - MicroLoRA adapter weights (exportable, ~20-100 MB)
  - Optimized router weights and scale factors
  - Quality benchmarks showing improvement over Phase 0 baseline
- **Expected quality**: **~70-80% of GLM-4.7-Flash** (up from ~55-65% Phase 0 PTQ)
- **Key value**: Gets a usable model on Mac Studio at $0 before committing to cloud GPU. If 70-80% quality is sufficient for the use case, Phase 1 cloud distillation may be deferred or skipped entirely.
- **100% RLM code reuse**: MicroLoRA, TrainingPipeline, EwcRegularizer, GrpoOptimizer, ContrastiveTrainer, MemoryDistiller, PolicyStore — all production-tested, zero new training code needed

### Phase 1: BitNet Expert Replacement (Option D)
- **Timeline**: 3-4 months
- **Cost**: ~$1,300-$2,000 (4× A100 spot, ~46 days)
- **Goal**: Full-quality ternary experts via distillation, validated against Phase 0/0.5 baselines
- **Deliverables**: Working Craftsman Ultra 30b 1bit (mixed: ternary experts, FP16 attention)
- **Expected quality**: ~90-95% of GLM-4.7-Flash on coding benchmarks
- **Prerequisites**: Phase 0 validates inference pipeline; Phase 0.5 provides quality baseline

### Phase 2: Full BitNet Distillation (Option C)
- **Timeline**: 4-6 months after Phase 1
- **Cost**: ~$2,500-$5,000 (4× H100, 16-32 days)
- **Goal**: Full ternary model with complete BitNet inference optimization
- **Deliverables**: Craftsman Ultra 30b 1bit v2 (full ternary except router/embed/head)
- **Expected quality**: ~95-98% of GLM-4.7-Flash

### Phase 3: Native BitNet Training (Option B)
- **Timeline**: 6-12 months after Phase 2, contingent on funding/compute
- **Cost**: ~$15,000-$30,000 (8× H100 cluster, 90-180 days)
- **Goal**: Surpass GLM-4.7-Flash quality with native ternary training
- **Deliverables**: Craftsman Ultra 30b 1bit v3 (trained from scratch)
- **Expected quality**: 100%+ of GLM-4.7-Flash (BitNet at scale exceeds FP16)

---

## Architectural Decisions

### AD-1: Ternary Weight Representation

**Decision**: Use BitNet b1.58 absmean quantization for weight ternary encoding.

```
W_ternary = RoundClip(W / (mean(|W|) + epsilon), -1, 1)
```

Each weight is one of {-1, 0, +1}, stored as 2-bit packed integers (I2_S format) in GGUF tensors. Per-block scale factor stored as FP16.

**Storage format per block (256 elements):**
- 64 bytes for ternary weights (2 bits × 256)
- 2 bytes for absmean scale (FP16)
- Total: 66 bytes / 256 weights = **2.06 bits/weight**

### AD-2: MoE Router Precision

**Decision**: MoE gating/routing network remains in FP16.

**Rationale**: Expert selection requires high-precision softmax scores to maintain routing quality. Quantizing the router to ternary would collapse expert selection, effectively turning a 30B model into a random-expert 3B model. The router is <0.1% of total parameters.

**Components kept in FP16:**
- Expert gating weights (router)
- Token embedding table
- LM head (output projection)
- RoPE frequency table
- LayerNorm/RMSNorm parameters

### AD-3: Activation Quantization

**Decision**: INT8 per-token absmax quantization for activations flowing through BitLinear layers.

```
X_int8 = clamp(round(X * 127 / max(|X|)), -128, 127)
```

**Rationale**: Consistent with BitNet b1.58 specification. INT8 activations enable integer-only GEMM in expert forward passes. Attention activations remain in FP16/BF16 for KV cache compatibility.

### AD-4: CPU Inference Kernel Strategy

**Decision**: Implement all three bitnet.cpp kernel types, with runtime selection based on hardware detection.

| Kernel | Target Hardware | Selection Criteria |
|--------|----------------|-------------------|
| **I2_S** | x86 AVX512, ARM SVE | Systems with wide SIMD and high bandwidth |
| **TL1** | x86 AVX2, ARM NEON | General-purpose, balanced performance |
| **TL2** | Memory-constrained | Systems with <16GB RAM or high cache pressure |

**Implementation path**: Adapt bitnet.cpp's kernel generation scripts (Python codegen) to produce Rust SIMD intrinsics compatible with RuvLLM's existing `kernels/` module structure.

**Key kernel operations:**
1. Pack ternary weights into 2-bit (I2_S) or LUT index (TL1: 4-bit, TL2: 5-bit)
2. Generate lookup tables for activation sums at model load time
3. Execute GEMM via table lookup + integer addition (no floating-point multiply)
4. Accumulate in INT16 with pack-and-unpack technique (lossless, no quantization of partials)
5. Dequantize output with per-block FP16 scale

### AD-5: GGUF Tensor Format Extension

**Decision**: Extend RuvLLM's GGUF format with BitNet-specific metadata and a new `BITNET_TERNARY` quantization type.

**New GGUF metadata keys:**
```
craftsman.bitnet.version = 1
craftsman.bitnet.weight_encoding = "absmean_ternary"
craftsman.bitnet.activation_bits = 8
craftsman.bitnet.router_precision = "f16"
craftsman.bitnet.kernel_hint = "tl1"  // preferred kernel
craftsman.moe.total_params = 30000000000
craftsman.moe.active_params = 3000000000
craftsman.moe.num_experts = <N>
craftsman.moe.active_experts = <K>
```

**Tensor storage**: Map to existing `IQ1_S` (type 19) for ternary expert weights, with additional metadata distinguishing post-training IQ1_S from native BitNet ternary. Alternatively, register a new type `BITNET_T158 = 29` if the existing IQ1_S block format is incompatible with absmean-scale-per-block layout.

### AD-6: RuvLLM Backend Integration

**Decision**: Create a new `BitNetBackend` alongside existing Candle and mistral-rs backends.

```
backends/
├── mod.rs                 // Backend trait + dispatch
├── candle_backend.rs      // GPU (Metal/CUDA)
├── mistral_backend.rs     // PagedAttention + ISQ
├── coreml_backend.rs      // Apple Neural Engine
└── bitnet_backend.rs      // NEW: CPU ternary inference
```

**BitNetBackend responsibilities:**
1. Load GGUF with ternary tensor detection
2. Initialize TL1/TL2/I2_S lookup tables per layer
3. Execute MoE routing in FP16 → select active experts
4. Run selected expert forward passes using ternary GEMM kernels
5. Attention in FP16 (Phase 1) or ternary (Phase 2+)
6. KV cache management (Q8 two-tier, existing infrastructure)

**Backend trait compliance:**
```rust
impl InferenceBackend for BitNetBackend {
    fn load_model(&mut self, path: &Path, config: ModelConfig) -> Result<()>;
    fn generate(&self, prompt: &str, params: GenerateParams) -> Result<Response>;
    fn get_embeddings(&self, text: &str) -> Result<Vec<f32>>;
    fn supports_architecture(&self, arch: &str) -> bool;
}
```

### AD-7: MoE Forward Pass Pipeline

**Decision**: Split MoE forward pass into FP16 routing + ternary expert execution.

```
Input Token Embedding (FP16)
  │
  ▼
┌─────────────────────────────────────────┐
│ For each transformer layer:             │
│                                         │
│  1. RMSNorm (FP16)                      │
│  2. Self-Attention                      │
│     ├─ Q/K/V projection (Phase 1: FP16, │
│     │                    Phase 2: Ternary)│
│     ├─ RoPE (FP16)                      │
│     ├─ Scaled dot-product attention      │
│     └─ Output projection                │
│  3. RMSNorm (FP16)                      │
│  4. MoE Block:                          │
│     ├─ Router (FP16 gating network)     │
│     │   → Select top-K experts          │
│     ├─ Expert FFN (TERNARY BitLinear)   │
│     │   ├─ gate_proj: W_ternary @ X_int8│
│     │   ├─ up_proj:   W_ternary @ X_int8│
│     │   ├─ SwiGLU activation            │
│     │   └─ down_proj: W_ternary @ X_int8│
│     └─ Weighted sum of expert outputs   │
│  5. Residual connection                 │
└─────────────────────────────────────────┘
  │
  ▼
LM Head (FP16) → Logits → Token
```

### AD-8: SONA Integration for Ternary Adaptation

**Decision**: MicroLoRA adapters applied as FP16 deltas on top of ternary base weights.

**Rationale**: Ternary weights cannot be directly fine-tuned at inference time (gradient updates don't map to {-1, 0, +1}). Instead, SONA's MicroLoRA applies rank-1 FP16 adapters whose output is added to the ternary forward pass output:

```
Y = BitLinear(X) + LoRA_B @ LoRA_A @ X
```

Where `BitLinear(X)` uses ternary GEMM and `LoRA_B @ LoRA_A @ X` is a small FP16 correction. This preserves BitNet's efficiency for 99%+ of computation while enabling per-session adaptation.

### AD-9: Memory Budget Analysis

**Decision**: Target <8GB model + 2GB KV cache = 10GB total for 4K context.

| Component | Precision | Size | Notes |
|-----------|-----------|------|-------|
| Expert weights (28B params) | 1.58-bit | ~5.5 GB | 28B × 2.06 bits = ~7.2 GB raw, but only routing metadata for inactive experts |
| Shared layers (2B params) | FP16 | ~4 GB | Embeddings, LM head, router, norms |
| Expert routing tables | FP16 | ~50 MB | Gating network weights |
| TL1/TL2 lookup tables | INT16 | ~200 MB | Pre-computed at load time |
| KV cache (4K context) | Q8 | ~1.5 GB | Two-tier cache (hot FP16 + warm Q8) |
| MicroLoRA adapters | FP16 | ~10 MB | Rank-1, <1MB per target module |
| **Total** | — | **~7.8 GB** | Fits in 16GB system with headroom |

**Note**: Full 30B ternary weights on disk are ~7.2 GB. At runtime, only active expert weights (~3B active) are in hot memory for any given token, with inactive expert pages memory-mapped and demand-loaded.

### AD-10: Platform-Specific Kernel Dispatch

**Decision**: Runtime hardware detection drives kernel selection.

```rust
pub fn select_kernel(caps: &HardwareCaps) -> BitNetKernel {
    if caps.has_avx512() {
        BitNetKernel::I2S_AVX512
    } else if caps.has_avx2() {
        BitNetKernel::TL1_AVX2
    } else if caps.has_neon() {
        if caps.cache_size_l2 >= 2 * 1024 * 1024 {
            BitNetKernel::TL1_NEON
        } else {
            BitNetKernel::TL2_NEON  // memory-constrained
        }
    } else if caps.has_sse41() {
        BitNetKernel::TL1_SSE41
    } else {
        BitNetKernel::I2S_Scalar  // fallback
    }
}
```

**Integration**: Leverages RuvLLM's existing `autodetect.rs` hardware capability detection module.

### AD-11: GRPO-Guided Distillation Loss

**Decision**: Use `GrpoOptimizer` to compute per-expert reward scaling during knowledge distillation, replacing a traditional fixed-weight KD loss.

**Rationale**: Standard KD uses a static `alpha` to blend KL divergence and hard-label cross-entropy. GRPO adds a dynamic reward signal that upweights expert-student pairs where ternary output closely matches the teacher, and downweights divergent pairs. This is achieved by mapping each expert's teacher-vs-student output comparison to a `SampleGroup`:

```
Combined Loss = KD_base + GRPO_scale
Where:
  KD_base  = α * KL(teacher_logits/T, student_logits/T)
           + (1-α) * CE(labels, student_logits)
  GRPO_scale = (1 + reward * 0.1)

  reward = GrpoEvaluator.evaluate(student_expert_output, teacher_expert_output)
         → 1.0 when cosine_sim > 0.95
         → -0.5 when cosine_sim < 0.7
```

**Key configuration** (extending `GrpoConfig::stable()`):
```rust
GrpoConfig {
    group_size: num_experts,        // One group per MoE layer
    learning_rate: 1e-6,            // Conservative for distillation
    kl_coefficient: 0.1,            // Tight teacher adherence
    kl_target: 0.02,                // Low divergence target
    clip_range: 0.1,                // Narrow clipping for stability
    normalize_advantages: true,     // Normalize across experts in group
    adaptive_kl: true,              // Auto-adjust KL penalty
    ..GrpoConfig::stable()
}
```

**Reused**: `GrpoOptimizer`, `GrpoConfig`, `SampleGroup`, `GrpoEvaluator` from `training/grpo.rs`.
**New**: `BitNetGrpoAdapter` that maps expert forward pass outputs to `GrpoSample` structs.

### AD-12: Contrastive Pre-Training for Expert Routing Validation

**Decision**: After ternary conversion of expert weights, use the existing `ContrastiveTrainer` to verify that MoE routing still selects the correct experts.

**Rationale**: Replacing expert FFN weights with ternary approximations changes the output distribution of each expert. If expert N's ternary output becomes more similar to expert M's output, the router may misroute tokens. Contrastive pre-training on expert embeddings detects and corrects this.

**Approach**:
1. For each token in a calibration set, record which expert the teacher model's router selects
2. Generate `TrainingTriplet`s: anchor = hidden state, positive = correct expert output, negative = wrong expert output
3. Use existing hard negative mining to find expert pairs that become confusable after ternary conversion
4. Fine-tune the FP16 router gating weights using contrastive loss to restore correct expert selection

**Reused**: `ContrastiveTrainer`, `ContrastiveConfig`, `TrainingTriplet` from `training/contrastive.rs`.
**New**: `ExpertTripletGenerator` that produces triplets from MoE routing decisions.

### AD-13: EWC++ Cross-Expert Stability During Sequential Distillation

**Decision**: Apply `EwcRegularizer` from `lora/training.rs` during sequential expert distillation to prevent catastrophic forgetting across experts.

**Rationale**: Distilling 30B MoE experts sequentially (expert 0, then 1, ..., then N) risks overwriting shared representations. EWC++ computes Fisher information diagonals for each expert's contribution to the shared attention layers, then regularizes subsequent expert distillation to not deviate from previously-learned important weights.

**Configuration**:
```rust
TrainingConfig {
    ewc_lambda: 5000.0,           // Higher than default (2000) for cross-expert stability
    fisher_decay: 0.995,           // Slower decay to preserve Fisher across expert phases
    quality_threshold: 0.5,        // Only learn from high-quality distillation samples
    lr_schedule: LearningRateSchedule::Cosine,
    warmup_steps: 500,             // Longer warmup for 30B scale
    ..Default::default()
}
```

**Concrete protection**:
- After distilling expert 0: compute Fisher diagonal `F_0` over validation set
- When distilling expert 1: add penalty `ewc_lambda/2 * Σ F_0_i * (w_i - w*_0_i)²`
- Accumulate: `F_cumulative = fisher_decay * F_prev + (1-fisher_decay) * F_new`

**Reused**: `EwcRegularizer`, `TrainingPipeline`, `TrainingConfig`, `FisherDiagonal` from `lora/training.rs`.
**New**: `SequentialExpertDistiller` that wraps `EwcRegularizer` across expert phases.

### AD-14: Policy Store for Per-Layer Ternary Scale Tracking

**Decision**: Extend `PolicyStore` with a new `PolicyType::TernaryScale` to persist per-block absmean scale distributions and learned quantization decisions.

**Rationale**: Not all layers quantize equally well to ternary. Attention layers may need different scale clipping than FFN layers. The policy store enables the distillation pipeline to learn and persist per-layer quantization strategies that can be retrieved and applied in future distillation runs or model updates.

**New policy type**:
```rust
pub enum PolicyType {
    Quantization,
    Router,
    Ewc,
    Pattern,
    TernaryScale,      // NEW: Per-layer ternary quantization metadata
}

pub struct TernaryScalePolicy {
    pub layer_idx: usize,
    pub module: String,              // "gate_proj", "up_proj", "down_proj", "q_proj", etc.
    pub mean_absmean: f32,           // Average scale factor across blocks
    pub std_absmean: f32,            // Variance in scale factors
    pub sparsity: f32,               // Fraction of zero weights
    pub quality_vs_teacher: f32,     // Cosine similarity to teacher output
    pub distillation_loss: f32,      // Final loss for this layer
    pub recommended_block_size: usize, // 256 default, may vary
}
```

**Reused**: `PolicyStore`, `PolicyEntry`, `PolicySource` from `policy_store.rs`.
**New**: `TernaryScalePolicy` struct and `PolicyType::TernaryScale` variant.

### AD-15: Memory Distillation for Training Quality Tracking

**Decision**: Log all distillation teacher-student comparisons as `Trajectory` objects in the `ReasoningBank`, enabling `MemoryDistiller` to extract `KeyLesson`s about which layers, experts, and configurations produce the best ternary quality.

**Rationale**: Distillation is iterative — understanding which experts converge quickly, which resist ternary conversion, and what scale distributions correlate with quality enables intelligent scheduling of future distillation runs.

**Mapping**:

| ReasoningBank Concept | Distillation Mapping |
|----------------------|---------------------|
| `Trajectory` | One expert's distillation run (N steps) |
| `Verdict` | `Success` if cosine_sim > 0.9, `Failure` if < 0.7 |
| `PatternCategory` | Expert index + layer type (e.g., "expert_3_gate_proj") |
| `KeyLesson` | "Expert 7 gate_proj converges fastest with lr=2e-6 and block_size=128" |
| `CompressedTrajectory` | Summary of entire expert distillation phase |

**Reused**: `MemoryDistiller`, `DistillationConfig`, `CompressedTrajectory`, `KeyLesson` from `reasoning_bank/distillation.rs`.
**New**: `DistillationTrajectoryRecorder` that adapts expert training steps to `Trajectory` format.

### AD-16: Distillation Pipeline Composition

**Decision**: Compose the full Craftsman Ultra distillation pipeline from existing RLM components wired through a new `CraftsmanDistiller` orchestrator.

**Pipeline architecture**:
```
┌─────────────────────────────────────────────────────────────────┐
│                  CraftsmanDistiller (NEW orchestrator)           │
│                                                                 │
│  ┌───────────────┐    ┌──────────────────┐    ┌──────────────┐ │
│  │ TeacherModel  │───▶│BitLinearTrainer   │───▶│ GGUFExporter │ │
│  │(GLM-4.7-Flash)│    │(NEW: STE+shadow)  │    │(REUSED)      │ │
│  └───────┬───────┘    └────────┬─────────┘    └──────────────┘ │
│          │                     │                                │
│          │   ┌─────────────────┼─────────────────┐              │
│          │   │                 │                  │              │
│          ▼   ▼                 ▼                  ▼              │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────┐    │
│  │GrpoOptimizer │   │EwcRegularizer│   │ContrastiveTrainer│    │
│  │(REUSED)      │   │(REUSED)      │   │(REUSED)          │    │
│  │Per-expert    │   │Cross-expert  │   │Router validation │    │
│  │reward scaling│   │stability     │   │post-ternary      │    │
│  └──────┬───────┘   └──────┬───────┘   └────────┬─────────┘    │
│         │                  │                     │              │
│         ▼                  ▼                     ▼              │
│  ┌──────────────────────────────────────────────────────┐      │
│  │              Quality Feedback Loop                    │      │
│  │                                                       │      │
│  │  MemoryDistiller ──▶ KeyLesson extraction             │      │
│  │  PolicyStore    ──▶ TernaryScale persistence          │      │
│  │  (BOTH REUSED)                                        │      │
│  └──────────────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────────────┘

Net-new code:  BitLinearTrainer (STE + shadow weights), CraftsmanDistiller (orchestrator)
Reused code:   GrpoOptimizer, EwcRegularizer, ContrastiveTrainer, MemoryDistiller,
               PolicyStore, GGUFExporter, TrainingConfig, LR schedules
Reuse ratio:   ~70% existing / ~30% new
```

**Optimization: Expert-Parallel Distillation**

Experts are independent during forward pass. Distill multiple experts concurrently across CPU cores:

```rust
// Distill experts in parallel (independent FFN weights)
let expert_results: Vec<DistillResult> = experts
    .par_iter()                          // rayon parallel iterator
    .enumerate()
    .map(|(idx, expert)| {
        let mut trainer = BitLinearTrainer::new(expert, &teacher_expert[idx]);
        let mut ewc = EwcRegularizer::new_with_fisher(cumulative_fisher[idx]);
        let mut grpo = GrpoOptimizer::new(GrpoConfig::stable());

        for batch in dataset.batches() {
            let student_out = trainer.forward_ternary(batch);
            let teacher_out = teacher.forward_expert(idx, batch);

            let reward = grpo.evaluate(&student_out, &teacher_out);
            let kd_loss = kd_loss_fn(&student_out, &teacher_out, alpha, temperature);
            let ewc_penalty = ewc.penalty(&trainer.shadow_weights());
            let total_loss = kd_loss * reward.scale() + ewc_penalty;

            trainer.backward_ste(total_loss);
        }

        ewc.update_fisher(&trainer);      // Update Fisher for next expert
        DistillResult { idx, weights: trainer.export_ternary(), fisher: ewc.fisher() }
    })
    .collect();
```

### AD-17: Training Infrastructure — Cloud GPU over Local SIMD

**Decision**: Use Google Cloud A100/H100 GPU instances for distillation training. Reserve local CPU/SIMD for inference validation, MicroLoRA adaptation, and GGUF export only.

**Rationale**: Local CPU/SIMD training is mathematically infeasible at the 200B+ token scale required for expert distillation. The existing RuvLLM SIMD kernels (`kernels/`) are inference-only — no backpropagation or gradient computation. The training code (`real_trainer.rs:178-184`) supports Metal (macOS) or CPU but not CUDA, and CPU throughput at ~50-100 tok/s training would require ~65 years for 200B tokens.

**Memory analysis (per-expert distillation):**

| Component | Size | Notes |
|-----------|------|-------|
| Single expert FFN shadow weights (FP16) | ~2 GB | ~1B params per expert (28B ÷ N experts) |
| Gradients (FP32) | ~4 GB | Full precision for STE backprop |
| AdamW optimizer state (2× FP32) | ~8 GB | First + second moment |
| Teacher activations cache | ~1 GB | Per-batch FP16 |
| EWC++ Fisher diagonal | ~0.5 GB | Per-expert accumulated |
| **Per-expert total** | **~15.5 GB** | Fits in A100 40GB with headroom |

**Full model simultaneous (Phase 2+):**

| Component | Size | Notes |
|-----------|------|-------|
| 30B shadow weights (FP16) | ~60 GB | Requires A100 80GB or H100 |
| Gradients + optimizer | ~360 GB | Requires multi-GPU parallelism |
| **Total** | **~430 GB** | 4× A100 80GB or 4× H100 80GB |

**Throughput and cost comparison:**

| Platform | Training tok/s | Time (200B tok, Phase 1) | Cost | Phase 0 PTQ? | Phase 0.5 RLM? |
|----------|---------------|--------------------------|------|-------------|---------------|
| **Mac Studio M4 Max (Metal)** | ~500-1000 | ~6.5 years | N/A | **Yes — 1-4 hrs, $0** | **Yes — 2-12 days, $0** |
| **Mac Studio M4 Max (NEON SIMD only, no Metal)** | ~200-500 | ~13 years | N/A | **Yes — 2-6 hrs, $0** | **Yes — 4-24 days, $0** |
| **Mac Studio M3 Ultra (Metal)** | ~800-1500 | ~4.2 years | N/A | **Yes — 1-1.5 hrs, $0** | **Yes — 1.5-8 days, $0** |
| **Mac Studio M3 Ultra (NEON SIMD only, no Metal)** | ~300-700 | ~9 years | N/A | **Yes — 1.5-3 hrs, $0** | **Yes — 3-16 days, $0** |
| CPU AVX2 (Ryzen 9) — scalar fallback | ~50-150 | ~43-130 years | N/A | Yes — 2-6 hrs, $0 | Yes — 14-58 days, $0 |
| 1× A100 80GB (GCP on-demand) | ~15,000 | ~155 days | ~$3,700 | Yes — 30 min, ~$5 | Overkill |
| 4× A100 80GB (GCP on-demand) | ~50,000 | ~46 days | ~$4,400 | Overkill for PTQ | Overkill |
| 4× A100 80GB (GCP spot) | ~50,000 | ~46 days | **~$1,300** | Overkill for PTQ | Overkill |
| 1× H100 (DataCrunch) | ~40,000 | ~58 days | ~$2,900 | Overkill for PTQ | Overkill |
| 4× H100 (DataCrunch) | ~140,000 | ~16 days | **~$3,200** | Overkill for PTQ | Overkill |

**Key insight**: Mac Studio is infeasible for Phase 1+ training (years of wall time) but **ideal for Phase 0 PTQ** (hours, $0). This separation justifies the phased approach.

**Recommended infrastructure per phase:**

| Phase | Instance | Duration | Estimated Cost | Strategy |
|-------|----------|----------|----------------|----------|
| **Phase 0 (PTQ)** | **Mac Studio (M4 Max/M3 Ultra)** | **1-4 hours** | **$0** | **Mmap FP16 weights → absmean quantize → export GGUF; Metal GPU for calibration pass** |
| Phase 0D (BitDistill Lite, 10B tok) | Mac Studio Metal or 1× A100 spot | 2-4 weeks (local) / 1-2 days (cloud) | $0 (local) / ~$300 (cloud) | Optional quality upgrade if Phase 0C too degraded |
| **Phase 0.5 (RLM refinement, Metal)** | **Mac Studio (Metal)** | **3-14 days** | **$0** | **MicroLoRA + router fix + scale opt using existing RLM stack** |
| **Phase 0.5 (RLM refinement, SIMD-only)** | **Mac Studio (NEON CPU)** | **5-28 days** | **$0** | **Same pipeline, no Metal required — pure ndarray + NEON SIMD (see AD-20)** |
| Phase 1 (expert FFN, 200B tok) | 4× A100 80GB spot (GCP) | ~46 days | $1,300-$2,000 | Per-expert sequential with EWC++; each expert fits 1 GPU |
| Phase 1 (router validation) | Mac Studio Metal or 1× A100 | ~2-4 hours | $0 (local) / <$10 (cloud) | Contrastive training on router only (~2B params) |
| Phase 2 (full ternary, 500B tok) | 4× H100 (DataCrunch) | ~16-32 days | $2,500-$5,000 | All layers; model-parallel across GPUs |
| Phase 3 (native training, 4T tok) | 8× H100 cluster | ~90-180 days | $15,000-$30,000 | Full from-scratch; depends on funding |
| Inference validation | Mac Studio (NEON) | Continuous | $0 | TL1/TL2 kernel testing on ARM NEON |
| MicroLoRA adaptation | Mac Studio | <1ms/update | $0 | Existing ndarray-based EWC++ pipeline |

**Required code change**: Add CUDA device dispatch to `RealContrastiveTrainer`:
```rust
// Current (real_trainer.rs:178-184):
let device = if config.use_metal {
    Device::new_metal(0).unwrap_or(Device::Cpu)
} else {
    Device::Cpu
};

// Required for cloud GPU training:
let device = if config.use_cuda {
    Device::new_cuda(config.cuda_device_id).unwrap_or(Device::Cpu)
} else if config.use_metal {
    Device::new_metal(0).unwrap_or(Device::Cpu)
} else {
    Device::Cpu
};
```

This is a single-line addition to `RealTrainingConfig` (`use_cuda: bool`, `cuda_device_id: usize`) and a 3-line change to device selection. The rest of the Candle training pipeline (tensors, optimizer, loss computation) works identically across CPU/Metal/CUDA.

**Cost optimization strategies:**
1. **Spot instances**: GCP A100 spot at ~$1/GPU-hr (70% off on-demand) — requires checkpointing every 30 min
2. **DataCrunch / Lambda Labs**: H100 at $1.99-$2.10/hr (40-50% below GCP on-demand)
3. **Expert-sequential on fewer GPUs**: Distill 1 expert at a time on 1× A100 80GB (~$1.50/hr), increasing wall time but reducing per-hour cost
4. **Mixed precision training**: FP16 shadow weights + BF16 activations reduces memory, enabling smaller instances
5. **Gradient checkpointing**: Trade compute for memory to fit on fewer GPUs

### AD-18: Phase 0 — PT-BitNet Post-Training Quantization on Mac Studio

**Decision**: Implement a PT-BitNet ternary post-training quantizer as Phase 0, running entirely on a local Mac Studio, producing a rapid prototype GGUF for inference pipeline validation before investing in full distillation.

**Rationale**: The original Option A ("Rejected") assumed only generic IQ1_S quantization, which produces garbled outputs at 1.56 bpw. However, PT-BitNet (2025) demonstrates that applying BitNet's native absmean ternary quantization to pre-trained weights with calibration data achieves significantly better results (61% downstream at 70B) than generic codebook PTQ. This produces genuine BitNet ternary format that enables multiplication-free inference with TL1/TL2 kernels — unlike IQ1_S which still requires dequant-then-multiply.

**Target platform: Mac Studio (Apple Silicon)**

Phase 0 is pure quantization (no training loop), making it ideal for local execution on Mac Studio:

| Config | Unified RAM | FP16 Load | PTQ? | Calibration? | Notes |
|--------|------------|-----------|------|-------------|-------|
| M4 Max 36GB | 36 GB | mmap (demand-paged) | **Yes** | Slow (paging) | Minimum viable; mmap means only active tensor pages in RAM |
| M4 Max 64GB | 64 GB | Fits with mmap assist | **Yes** | **Yes** | Comfortable for PTQ; calibration may page |
| M4 Max 128GB | 128 GB | Fits entirely | **Yes** | **Yes** | Ideal — FP16 model (60GB) + ternary output (7GB) + calibration buffers all in RAM |
| M3 Ultra 96GB | 96 GB | Fits entirely | **Yes** | **Yes** | Good headroom |
| M3 Ultra 192GB+ | 192+ GB | Fits entirely | **Yes** | **Yes** | Ample room for full model + calibration + inference validation |

**Why Mac Studio works for Phase 0 (but not Phase 1+):**
- **PTQ is not training**: No gradient computation, no optimizer state, no backpropagation — just load → quantize → export
- **Memory-mapped I/O**: FP16 weights can be mmap'd from disk; only the current tensor's pages need to be in RAM
- **Per-tensor processing**: Quantize one tensor at a time (read FP16 block → compute absmean → round to ternary → write output) — working memory is ~2-4 MB per tensor regardless of total model size
- **Metal GPU for calibration**: RuvLLM's existing `RealContrastiveTrainer` and `kernels/matmul.rs` support Metal via Candle (`use_metal: true` default, 3x speedup on M4 Pro GEMV)
- **ARM NEON for TL1 kernels**: Mac Studio's Apple Silicon has NEON SIMD — the same target ISA as the TL1 kernel for ternary inference validation
- **Phase 1 still needs cloud GPU**: 200B token distillation at ~500-1000 tok/s (Metal) = ~6.5 years locally vs ~46 days on 4× A100

**Estimated Phase 0 wall time on Mac Studio:**

| Step | M4 Max 128GB | M4 Max 64GB | M3 Ultra 192GB |
|------|-------------|-------------|----------------|
| Download GLM-4.7-Flash FP16 (~60GB) | ~30 min (1Gbps) | ~30 min | ~30 min |
| Absmean ternary quantization | ~5-15 min | ~10-30 min (paging) | ~5-10 min |
| Calibration pass (1000 samples, Metal) | ~30-60 min | ~60-120 min | ~20-40 min |
| GGUF export | ~2-5 min | ~2-5 min | ~2-5 min |
| TL1 kernel validation inference | ~10-20 min | ~10-20 min | ~10-20 min |
| **Total** | **~1-2 hours** | **~2-4 hours** | **~1-1.5 hours** |

**Implementation approach**:

```
Phase 0 Pipeline (runs on Mac Studio):
  1. Load GLM-4.7-Flash FP16/BF16 weights via mmap
  2. For each linear layer in expert FFNs:
     a. Compute gamma = mean(|W|)  (absmean scale)
     b. W_ternary = RoundClip(W / (gamma + epsilon), -1, 1)
     c. Store: 2-bit packed ternary weights + FP16 scale per block
  3. Calibration pass (optional, improves quality, uses Metal GPU):
     a. Run ~1000 calibration samples through teacher model
     b. Record activation statistics per layer
     c. Optimize scale factors to minimize MSE between teacher and ternary outputs
  4. Export to GGUF with BITNET_T158 tensor type + metadata
  5. Validate: load in BitNetBackend → TL1 NEON kernel → generate tokens
```

**Absmean ternary quantizer (core algorithm)**:
```
Input:  W ∈ R^{m×n} (FP16 weight matrix)
Output: W_t ∈ {-1,0,+1}^{m×n}, scale ∈ R (per-block FP16)

For each block of 256 elements:
  1. gamma = mean(|block|) + 1e-8
  2. normalized = block / gamma
  3. ternary = round(clamp(normalized, -1, 1))  → {-1, 0, +1}
  4. Pack: 2 bits per weight (00=-1, 01=0, 10=+1)
  5. Store scale = gamma as FP16
```

**What stays FP16** (same as AD-2):
- MoE router gating weights
- Token embeddings + LM head
- RoPE frequencies
- LayerNorm/RMSNorm parameters

**RuvLLM implementation gaps to fill**:

| Gap | Effort | Details |
|-----|--------|---------|
| Absmean ternary quantizer | ~200-300 lines | New function in `gguf/quantization.rs` or new module |
| IQ1_S / BITNET_T158 dequantization | ~80-120 lines | Add to `dequantize_tensor` match arm (currently falls to error at line 358) |
| GGUF export with ternary metadata | ~100-150 lines | Extend `GgufExportResult` with BitNet metadata keys from AD-5 |
| TL1 kernel smoke test | ~200 lines | Validate ternary GEMM produces correct output on PTQ model |

**Total new code**: ~600-800 lines (vs ~15,000+ for Phase 1 full distillation pipeline)

**Quality expectations (conservative estimates for GLM-4.7-Flash 30B-A3B)**:

| Benchmark | FP16 Baseline | Phase 0 PTQ (est.) | Phase 1 Distill (est.) |
|-----------|--------------|-------------------|----------------------|
| HumanEval pass@1 | ~65% | ~35-45% | ~55-60% |
| MMLU | ~75% | ~45-55% | ~65-70% |
| SWE-bench Verified | 59.2% | ~25-35% | ~50-55% |
| LiveCodeBench v6 | 64.0% | ~30-40% | ~55-60% |

**Why Phase 0 quality is still useful**:
1. **Kernel validation**: Ternary GEMM correctness doesn't depend on model quality
2. **Memory profiling**: Real-world memory usage measurement with actual MoE activation patterns
3. **Throughput benchmarking**: Measure real tok/s with TL1/TL2/I2_S kernels on target hardware
4. **Pipeline testing**: End-to-end GGUF load → inference → token output
5. **Baseline measurement**: Quantitative quality floor establishes improvement target for Phase 1
6. **Cost**: $0 on Mac Studio vs ~$1,300 for Phase 1 — validates infrastructure at zero cost before committing to cloud GPU

**Key configuration**:
```rust
pub struct PtBitnetConfig {
    pub calibration_samples: usize,     // 1000 default (WikiText-2 or code corpus)
    pub block_size: usize,              // 256 (matches AD-1)
    pub optimize_scales: bool,          // true: MSE-optimized scales; false: raw absmean
    pub layers_to_quantize: LayerMask,  // ExpertsOnly (Phase 0) or All (future)
    pub export_format: TernaryFormat,   // BitnetT158 (native) or IQ1S (llama.cpp compat)
    pub router_precision: Precision,    // FP16 (always, per AD-2)
    pub use_mmap: bool,                 // true: memory-map FP16 weights (required for <128GB systems)
    pub use_metal_calibration: bool,    // true: Metal GPU for calibration pass (Mac Studio)
    pub max_memory_gb: Option<f32>,     // Cap memory usage; enables streaming quantization
}
```

**Reused**: GGUF parser, tensor metadata, `GgufQuantType` enum, export pipeline.
**New**: `PtBitnetQuantizer`, `absmean_ternary()`, `BITNET_T158` dequantization kernel.

### AD-19: Phase 0.5 — RLM Post-Quantization Refinement (No Traditional Training)

**Decision**: Use the existing RLM training stack to refine the Phase 0 PTQ model on Mac Studio by training only the small FP16 components (~1-2% of parameters), freezing ternary weights. This replaces traditional distillation for the rapid prototype phase.

**Rationale**: Traditional knowledge distillation (Phase 1) requires shadow weights, straight-through estimator, and GPU-scale compute to modify the ternary weights themselves. However, the Phase 0 PTQ model already has ternary weights — the quality loss comes from:
1. Sub-optimal per-block scale factors (absmean is a rough approximation)
2. MoE router misrouting tokens to wrong experts (expert output distributions changed)
3. No adaptation to ternary output characteristics

All three can be addressed by training only the FP16 components using the existing RLM stack, without touching the ternary weights.

**What gets trained (FP16, differentiable) vs frozen (ternary, not differentiable):**

| Component | Params | Size | Trainable? | Training Method |
|-----------|--------|------|------------|----------------|
| Expert FFN ternary weights | ~28B | ~5.5 GB | **Frozen** | N/A — {-1,0,+1} not differentiable |
| MicroLoRA adapters (rank-2, per expert FFN) | ~50-100M | ~100-200 MB | **Yes** | `TrainingPipeline` + `EwcRegularizer` |
| MoE router gating weights | ~30M | ~60 MB | **Yes** | `ContrastiveTrainer` (triplet + InfoNCE) |
| Per-block absmean scale factors | ~0.1M | ~200 KB | **Yes** | GRPO reward-guided optimization |
| LM head (output projection) | ~150M | ~300 MB | **Yes (optional)** | Standard fine-tuning |
| Attention Q/K/V/O (FP16) | ~2B | ~4 GB | **Optional** | Can add LoRA here too if budget allows |
| **Total trainable** | **~200-400M** | **~400-800 MB** | | **~1-2% of 30B total** |

**Why RLM works here (vs traditional distillation):**

| Property | Traditional KD (Phase 1) | RLM Refinement (Phase 0.5) |
|----------|--------------------------|----------------------------|
| Modifies ternary weights | Yes (shadow weights + STE) | No (frozen) |
| Trainable params | ~28B (all expert weights) | ~200-400M (1-2%) |
| Training tokens needed | 200B | 100M-500M (400x less) |
| GPU requirement | 4× A100 ($1,300+) | Mac Studio Metal ($0) |
| Training time | ~46 days (cloud) | **2-12 days (local)** |
| Quality target | ~90-95% of FP16 | ~70-80% of FP16 |
| New code required | ~15,000 lines (BitLinear, STE, orchestrator) | **~0 lines** (100% RLM reuse) |

**RLM component mapping:**

```
┌──────────────────────────────────────────────────────────────────┐
│              Phase 0.5: RLM Refinement Pipeline                  │
│              (100% existing RLM code, 0% new training code)      │
│                                                                  │
│  Frozen Ternary Model (Phase 0 PTQ output)                       │
│  ┌────────────────────────────────────────────┐                  │
│  │  Expert FFNs: {-1,0,+1} weights (FROZEN)   │                  │
│  │  Router: FP16 gating (TRAINABLE)            │                  │
│  │  Attention: FP16 (TRAINABLE via LoRA opt.)  │                  │
│  │  Scales: FP16 per-block (TRAINABLE)         │                  │
│  └────────────────────────────────────────────┘                  │
│           │                                                       │
│     ┌─────▼──────────────────────────────────────────┐           │
│     │  Step 1: Router Repair                          │           │
│     │  ContrastiveTrainer (REUSED, contrastive.rs)    │           │
│     │  • Generate triplets: anchor=hidden, +correct   │           │
│     │    expert, -wrong expert                        │           │
│     │  • Triplet + InfoNCE loss on FP16 router        │           │
│     │  • Fix misrouting from PTQ weight changes       │           │
│     │  Training: ~10M tokens, ~1-2 hours (Metal)      │           │
│     └─────┬──────────────────────────────────────────┘           │
│           │                                                       │
│     ┌─────▼──────────────────────────────────────────┐           │
│     │  Step 2: MicroLoRA Injection + Training         │           │
│     │  TrainingPipeline + MicroLoRA (REUSED,          │           │
│     │    lora/training.rs + lora/micro_lora.rs)       │           │
│     │  • Rank-2 LoRA per expert FFN: Y = BitLinear(X) │           │
│     │    + LoRA_B @ LoRA_A @ X                        │           │
│     │  • Loss: MSE(teacher_output, student+LoRA)      │           │
│     │  • EWC++ across expert phases                   │           │
│     │  Training: ~100-500M tokens, ~2-12 days (Metal) │           │
│     └─────┬──────────────────────────────────────────┘           │
│           │                                                       │
│     ┌─────▼──────────────────────────────────────────┐           │
│     │  Step 3: Scale Factor + Quality Optimization    │           │
│     │  GrpoOptimizer (REUSED, grpo.rs)                │           │
│     │  • Per-expert output quality → reward signal     │           │
│     │  • Optimize FP16 scale factors to maximize       │           │
│     │    cosine similarity with teacher output          │           │
│     │  • Adaptive KL prevents over-correction          │           │
│     │  Training: concurrent with Step 2               │           │
│     └─────┬──────────────────────────────────────────┘           │
│           │                                                       │
│     ┌─────▼──────────────────────────────────────────┐           │
│     │  Feedback Loop                                  │           │
│     │  MemoryDistiller → KeyLessons (REUSED)          │           │
│     │  PolicyStore → TernaryScale policies (REUSED)   │           │
│     │  • Track which experts improve most             │           │
│     │  • Store optimized configs for reproducibility  │           │
│     └────────────────────────────────────────────────┘           │
└──────────────────────────────────────────────────────────────────┘
```

**Memory budget on Mac Studio during Phase 0.5 training:**

| Component | Size | Notes |
|-----------|------|-------|
| PTQ ternary model (mmap) | ~7 GB disk / ~3-7 GB RAM | Demand-paged; only active expert pages in RAM |
| Teacher FP16 model (mmap) | ~60 GB disk / ~4-8 GB RAM | Only forward pass activations; demand-paged |
| MicroLoRA adapters (rank-2) | ~200 MB | All experts in RAM |
| LoRA gradients + optimizer (AdamW 2×FP32) | ~1.5 GB | For ~400M trainable params |
| EWC++ Fisher diagonal | ~200 MB | Per-expert accumulated |
| KV cache + activations | ~2 GB | Calibration/training forward pass |
| **Total active RAM** | **~12-20 GB** | **Fits in any Mac Studio config** |

**Key insight**: The teacher model is only needed for forward pass (no gradients), so it can be mmap'd and demand-paged. The ternary student is similarly mmap'd. Only the ~400M trainable parameters and their optimizer state need to be fully in RAM (~2 GB), which fits comfortably in even the 36GB M4 Max.

**Training schedule on Mac Studio M4 Max 128GB:**

| Step | Tokens | Wall Time | What Changes |
|------|--------|-----------|-------------|
| Router repair | ~10M | ~3-6 hours | FP16 router gating weights |
| LoRA training (per-expert, sequential) | ~100-500M | 2-12 days | MicroLoRA A/B matrices per expert FFN |
| Scale optimization | ~10M | ~3-6 hours | Per-block FP16 absmean scales |
| Validation + export | — | ~1-2 hours | Benchmark + GGUF re-export |
| **Total** | **~120-520M** | **~3-14 days** | |

**Expected quality improvement:**

| Benchmark | Phase 0 PTQ | Phase 0.5 RLM | Phase 1 Distill | FP16 Baseline |
|-----------|------------|--------------|----------------|---------------|
| HumanEval pass@1 | ~35-45% | **~45-55%** | ~55-60% | ~65% |
| MMLU | ~45-55% | **~55-65%** | ~65-70% | ~75% |
| SWE-bench Verified | ~25-35% | **~35-45%** | ~50-55% | 59.2% |

**The question "can I use RLM rather than traditional training" is answered YES** — with the critical caveat that RLM refinement trains the FP16 corrections around frozen ternary weights, not the ternary weights themselves. This is fundamentally different from traditional distillation but achieves meaningful quality recovery (estimated +10-15 percentage points) at zero cost.

**Reused (100%)**: `MicroLoRA`, `TrainingPipeline`, `EwcRegularizer`, `GrpoOptimizer`, `ContrastiveTrainer`, `MemoryDistiller`, `PolicyStore`, `TrainingConfig`, LR schedules, GGUF export.
**New (0%)**: No new training code. The only new code is a thin `RlmRefiner` orchestrator (~200-300 lines) that wires the existing components together for the Phase 0.5 pipeline.

### AD-20: Phase 0.5 — SIMD-Only Training Mode (No Metal GPU Required)

**Decision**: Phase 0.5 RLM refinement supports a pure SIMD/CPU execution mode with no Metal GPU dependency. Metal is an optional acceleration path (~2-3x faster) but not required.

**Rationale**: Analysis of the RLM training stack reveals that Metal GPU is used by only one component (`RealContrastiveTrainer` via Candle), while all other training components are pure ndarray/CPU. Since Phase 0.5 uses the lightweight `ContrastiveTrainer` (not `RealContrastiveTrainer`) for router repair, and all gradient computation is ndarray-based, the entire pipeline runs on pure CPU with SIMD acceleration for inference forward passes.

**Component-by-component GPU dependency analysis:**

| Component | Source | GPU Dependency | SIMD-Only Mode |
|-----------|--------|---------------|----------------|
| `MicroLoRA.forward_simd()` | `lora/micro_lora.rs:279` | **None** — ARM NEON intrinsics with scalar fallback | NEON on aarch64, scalar on x86 |
| `MicroLoRA.apply_gradients()` | `lora/micro_lora.rs:621+` | **None** — pure ndarray | Works everywhere |
| `MicroLoRA.apply_gradients_with_ewc()` | `lora/micro_lora.rs:621+` | **None** — pure ndarray | Works everywhere |
| `TrainingPipeline` | `lora/training.rs` | **None** — pure ndarray CPU | Works everywhere |
| `EwcRegularizer` | `lora/training.rs` | **None** — pure ndarray CPU | Works everywhere |
| `GrpoOptimizer` | `training/grpo.rs` | **None** — pure ndarray CPU | Works everywhere |
| `ContrastiveTrainer` | `training/contrastive.rs:169-175` | **Optional** — `use_metal: true` default, but `Device::new_metal(0).unwrap_or(Device::Cpu)` fallback | Set `use_metal: false` for CPU-only; also has non-Candle pure CPU path (line 475) |
| `MemoryDistiller` | `reasoning_bank/distillation.rs` | **None** — pure Rust | Works everywhere |
| `PolicyStore` | `policy_store.rs` | **None** — pure Rust | Works everywhere |
| **`RealContrastiveTrainer`** | `training/real_trainer.rs:178` | **Yes — Metal/Candle** | **NOT used in Phase 0.5** (used in full distillation only) |

**Inference forward pass (for loss computation) SIMD support:**

| Kernel | NEON (aarch64) | x86 | Source |
|--------|---------------|-----|--------|
| GEMM | `gemm_neon` | `gemm_scalar` fallback | `kernels/matmul.rs:520` |
| GEMV | `gemv_neon` | `gemv_scalar` fallback | `kernels/matmul.rs:184` |
| SiLU | `silu_neon_impl` (~3.5x speedup) | scalar fallback | `kernels/activations.rs` |
| GeLU | `gelu_neon_impl` (~3.2x speedup) | scalar fallback | `kernels/activations.rs` |
| ReLU | `relu_neon_impl` (~4.0x speedup) | scalar fallback | `kernels/activations.rs` |
| RMSNorm | `rms_norm_neon` | scalar fallback | `kernels/norm.rs` |
| RoPE | `apply_rope_neon` | scalar fallback | `kernels/rope.rs` |
| Softmax | `softmax_neon` (~2.8x speedup) | scalar fallback | `kernels/activations.rs` |

**Key observation**: The matmul kernels only dispatch on `target_arch = "aarch64"` vs scalar. There are **no explicit AVX2 or AVX512 SIMD implementations** for x86 in the current kernel codebase. This means:
- **Apple Silicon (aarch64)**: Full NEON SIMD acceleration — primary target for SIMD-only mode
- **x86 (AMD/Intel)**: Falls to scalar fallback — works but ~3-5x slower than NEON
- **Future opportunity**: Adding AVX2/AVX512 kernels to `matmul.rs` would make x86 competitive with NEON

**Throughput comparison for Phase 0.5 (100M tokens, ~200-400M trainable params, 3B active forward):**

| Execution Mode | Forward tok/s | Effective Training tok/s | 100M Tokens | 500M Tokens |
|---------------|--------------|------------------------|------------|------------|
| Metal GPU (M4 Max) | ~500-1500 | ~300-700 | ~2-4 days | ~8-19 days |
| **NEON SIMD only (M4 Max CPU)** | **~200-500** | **~100-300** | **~4-12 days** | **~19-58 days** |
| **NEON SIMD only (M3 Ultra CPU)** | **~300-700** | **~150-400** | **~3-8 days** | **~14-39 days** |
| x86 scalar (Ryzen 9, no AVX2 kernels) | ~50-150 | ~30-80 | ~14-39 days | ~72-193 days |

**Why SIMD-only is ~2-3x slower than Metal (not 10x):**
- Phase 0.5 training is dominated by the forward pass through the frozen 3B active parameters to compute loss against the teacher
- The forward pass uses SIMD-accelerated GEMM/GEMV (`gemm_neon`/`gemv_neon`) which gets ~60-70% of Metal throughput for these matrix sizes
- Gradient computation for the ~200-400M trainable params is pure ndarray — identical speed regardless of Metal availability
- The training bottleneck is I/O (loading teacher activations from mmap) not compute, further narrowing the gap

**Platform portability (bonus of SIMD-only mode):**

SIMD-only mode extends Phase 0.5 beyond Mac Studio to any platform with ndarray support:

| Platform | SIMD Path | Effective tok/s | Feasible? |
|----------|----------|----------------|-----------|
| Mac Studio M4 Max (aarch64) | NEON intrinsics | ~100-300 | **Yes — primary target** |
| Mac Studio M3 Ultra (aarch64) | NEON intrinsics | ~150-400 | **Yes — faster than M4 Max** |
| Linux ARM64 (Ampere/Graviton) | NEON intrinsics | ~80-200 | **Yes — cloud ARM instances** |
| Linux x86 (Ryzen/Xeon) | Scalar fallback | ~30-80 | **Marginal — 100M tokens feasible (~14-39 days), 500M not practical** |
| macOS Intel | Scalar fallback | ~20-50 | **Not recommended** |

**Configuration for SIMD-only mode:**

```rust
// Phase 0.5 SIMD-only config (no Metal)
let contrastive_config = ContrastiveConfig {
    use_metal: false,    // Force CPU path in ContrastiveTrainer
    ..Default::default()
};

// MicroLoRA — already pure SIMD/ndarray, no config change needed
// TrainingPipeline — already pure ndarray
// GrpoOptimizer — already pure ndarray
// EwcRegularizer — already pure ndarray
```

The only config change is `ContrastiveTrainer.use_metal = false`. All other RLM components are GPU-agnostic by design.

**SIMD-only Phase 0.5 exit criteria (in addition to standard Phase 0.5 criteria):**
- [ ] All training completes without Metal GPU dependency
- [ ] `ContrastiveTrainer` runs with `use_metal: false` and produces equivalent router accuracy
- [ ] MicroLoRA `forward_simd()` executes NEON path on aarch64 (verified via `cfg` compile check)
- [ ] Training throughput measured and documented for SIMD-only vs Metal comparison

**Recommendation**: Use Metal when available (2-3x faster), fall back to SIMD-only when Metal is unavailable or on non-Mac platforms. The training code requires zero changes — only `ContrastiveTrainer.use_metal` needs to be set to `false`.

**Reused**: 100% of existing RLM stack — `MicroLoRA` NEON forward, ndarray training, `ContrastiveTrainer` CPU fallback, all existing SIMD kernels.
**New**: 0 lines. SIMD-only mode is already supported by the existing code paths; AD-20 documents this capability explicitly.

### AD-21: Native Rust Ternary Kernels with WASM Target (bitnet.cpp Port Strategy)

**Decision**: Port bitnet.cpp's ternary inference kernels (TL1, TL2, I2_S) to native Rust with dual compilation targets: native SIMD (NEON/AVX2/AVX512) and WebAssembly SIMD128. This replaces the original AD-4 strategy of Python codegen → Rust intrinsics with a pure Rust implementation that leverages existing open-source work.

**Rationale**: Three significant developments change the AD-4 implementation calculus:

1. **R3-Engine** (https://github.com/r3-engine/r3-engine) — A pure Safe Rust BitNet inference engine achieving 80-117 tok/s single-threaded on Ryzen 9950X3D, with native WASM SIMD128 cross-compilation. Uses bit-sliced ternary matrices with AVX-512 VPOPCNTDQ, zero-copy mmap, and zero heap allocations during generation.

2. **bitnet.rs** (https://github.com/ocentra/bitnet.rs) — Pure Rust BitNet toolkit with conversion, inference, training, and streaming. Apache 2.0 license. GPU path via WGSL/wgpu (Vulkan/Metal/DX12). Dedicated `bitnet-wasm` crate for browser deployment.

3. **WASM SIMD128 maturity** — Fixed-width 128-bit SIMD now supported in all major browsers (Chrome, Firefox, Safari, Edge). Rust's `core::arch::wasm32` provides direct intrinsic access via `simd128` LLVM feature flag.

**Comparison of approaches:**

| Approach | Native Performance | WASM Support | Safety | Integration Effort | Code Reuse |
|----------|-------------------|-------------|--------|-------------------|-----------|
| **A: Python codegen (original AD-4)** | Optimal (platform-tuned) | None | C-level unsafe | High — custom codegen pipeline | bitnet.cpp algorithms |
| **B: Port bitnet.cpp to Rust** | Near-optimal | Manual WASM SIMD | Mixed (`unsafe` for intrinsics) | Medium — translate C → Rust | bitnet.cpp algorithms |
| **C: Reference R3-Engine patterns** | 80-117 tok/s proven | Native dual-target | 100% Safe Rust | Low-medium — adapt patterns | R3 bit-slicing + mmap |
| **D: Integrate bitnet.rs crate** | GPU: 32x (WGSL), CPU: scalar | `bitnet-wasm` crate | Safe Rust + WGSL | Low — add dependency | Full crate |

**Recommended: Approach C (Reference R3-Engine) with RuvLLM integration**

R3-Engine's techniques are the strongest fit because:
- **100% Safe Rust** — no `unsafe` blocks in the hot path
- **Dual-target proven** — same codebase compiles to AVX-512 native and WASM SIMD128
- **Zero-copy mmap** — matches our Phase 0 mmap strategy (AD-18)
- **Cache-aligned bit-slicing** — 64-byte aligned CacheLines match CPU cache architecture
- **VPOPCNTDQ** — bit-population-count approach to ternary GEMM is elegant and SIMD-width-agnostic

**WASM SIMD128 kernel mapping for TL1:**

```
WASM SIMD128 provides v128 type (128 bits):
- i8x16: 16 × 8-bit integers — pack 64 ternary weights (2-bit each)
- i16x8: 8 × 16-bit integers — accumulation without overflow
- i32x4: 4 × 32-bit integers — final dequantized output

TL1 LUT (16 entries) maps naturally to a single v128:
  v128.load(lut_ptr)           → load 16-entry LUT
  v128.swizzle(lut, indices)   → parallel 16-way table lookup
  i16x8.add(accum, partial)    → INT16 accumulation
  f32x4.mul(dequant, scale)    → FP32 scale application

Estimated WASM SIMD128 throughput:
  ~20-40 tok/s for 3B active params (vs ~5-10 tok/s scalar JS)
  ~4-8x speedup over non-SIMD WebAssembly
```

**WASM SIMD128 limitations:**
- Fixed 128-bit width only (vs NEON 128, AVX2 256, AVX512 512)
- No integer popcount instruction (must emulate VPOPCNTDQ via lookup or bit manipulation)
- No gather/scatter operations (LUT access must be sequential or use swizzle)
- Memory alignment not enforced (no hardware-guaranteed 64-byte alignment)
- Single-threaded unless SharedArrayBuffer + Web Workers enabled

**Dual-target compilation strategy (Cargo feature flags):**

```rust
// In Cargo.toml:
[features]
default = ["native-simd"]
native-simd = []           # AVX2/AVX512/NEON via std::arch
wasm-simd = ["simd128"]    # WASM SIMD128 via core::arch::wasm32

// In kernel code:
#[cfg(all(target_arch = "aarch64", feature = "native-simd"))]
fn ternary_gemv_neon(weights: &TernaryTensor, activations: &[i8], output: &mut [f32]) { ... }

#[cfg(all(target_arch = "x86_64", feature = "native-simd"))]
fn ternary_gemv_avx2(weights: &TernaryTensor, activations: &[i8], output: &mut [f32]) { ... }

#[cfg(all(target_arch = "wasm32", feature = "wasm-simd"))]
fn ternary_gemv_wasm128(weights: &TernaryTensor, activations: &[i8], output: &mut [f32]) { ... }

// Scalar fallback (always available):
fn ternary_gemv_scalar(weights: &TernaryTensor, activations: &[i8], output: &mut [f32]) { ... }
```

**Integration with existing RuvLLM architecture:**

| Existing Component | Change Needed | Impact |
|-------------------|--------------|--------|
| `kernels/mod.rs` | Add `ternary` module export | Low |
| `kernels/matmul.rs` | Add ternary GEMV dispatch alongside existing FP16/Metal GEMV | Low |
| `bitnet/mod.rs` (new) | Wire TernaryTensor to kernel dispatch | Already created (Phase 0) |
| `gguf/quantization.rs` | BitnetT158 dequant already integrated | Already done |
| `autodetect.rs` | Add AVX512 VPOPCNTDQ detection + WASM target detection | Low |
| `Cargo.toml` | Add `wasm-simd` feature flag, `wasm32` target conditional deps | Low |
| `backends/` | New `BitNetBackend` uses ternary kernel dispatch | Medium (new backend) |

**Estimated implementation effort (Rust ternary kernels with WASM):**

| Component | Lines | Complexity | Notes |
|-----------|-------|-----------|-------|
| TL1 kernel (NEON + scalar) | ~200 | Medium | Reference R3-Engine bit-slicing |
| TL1 kernel (AVX2/AVX512) | ~250 | Medium | VPOPCNTDQ for AVX512, lookup for AVX2 |
| TL1 kernel (WASM SIMD128) | ~150 | Medium | v128 swizzle + i16x8 accumulation |
| I2_S kernel (all targets) | ~300 | Low | Simpler unpack-and-add |
| TL2 kernel (all targets) | ~250 | Medium-High | 5-bit index, 32-entry LUT |
| Kernel dispatch + autodetect | ~100 | Low | Match existing `matmul.rs` pattern |
| LUT generation | ~80 | Low | Pre-compute at model load |
| **Total** | **~1,330** | — | Compiles to native + WASM from single source |

**Phase 0 impact**: The Phase 0 smoke test (TL1 NEON + scalar) is already partially covered by the existing `bitnet/` module. AD-21 extends this to production-grade kernels with WASM as an additional target.

**Exit criteria:**
- [ ] TL1 kernel passes bit-exact validation against bitnet.cpp reference output
- [ ] WASM SIMD128 build produces functional `.wasm` binary
- [ ] Native NEON throughput ≥ 80% of R3-Engine (≥ ~64-94 tok/s for 2B model)
- [ ] AVX2 path tested on x86 Linux
- [ ] Scalar fallback tested on generic platform
- [ ] WASM throughput ≥ 20 tok/s for 3B active params in browser
- [ ] Zero `unsafe` blocks in WASM path (Safe Rust only)
- [ ] Kernel dispatch selects optimal path via `autodetect.rs` feature detection

**Open question resolved**: AD-21 answers open question #5 (WASM target for ternary kernels) — **yes, WASM SIMD128 is viable** for TL1/I2_S, with ~4-8x speedup over scalar WASM. TL2's 5-bit index is less natural for 128-bit SIMD but still implementable via two-stage lookup.

---

### AD-22: Evaluation Infrastructure and Behavioral Gates

**Decision**: Define a three-gate behavioral evaluation framework with a structured trace schema, auto-labeling strategy, and Go/No-Go shipping rule. All gates are non-LLM-judge, deterministic, reproducible, and executable on CPU without external API calls. The system ships on integrity/citations/refusal behavior, not raw model quality benchmarks. Full GPU distillation (Phase 1+) is deferred; the eval infrastructure must validate Phase 0 and Phase 0.5 outputs at zero marginal cost.

**Rationale**: Standard LLM evaluation relies on either (a) benchmark suites (HumanEval, MMLU) that measure general capability, or (b) LLM-as-judge approaches that are non-deterministic, expensive, and unsuitable for gating CI/CD pipelines. For Craftsman Ultra, the critical shipping question is not "does it score well on benchmarks?" but "does it route correctly, cite honestly, and refuse when uncertain?" These behavioral properties are testable with deterministic, cheap-to-run gate checks that compare model outputs against known ground-truth traces.

The three gates correspond to the three failure modes that would make the system untrustworthy regardless of benchmark scores:
1. **Misrouting** — wrong experts selected, producing semantically wrong outputs from correct-seeming completions
2. **Hallucinated citations** — model cites evidence that does not exist or does not support the claim
3. **Over/under-refusal** — model refuses answerable questions or confidently answers indeterminate ones

**Gate 1 — Routing Correctness**

Run the FP16 teacher model once on the 200-prompt evaluation suite to record ground-truth routing traces: which experts are selected, with what softmax weights, per token per layer. Then run the ternary student model on the same prompts and compare routing decisions.

| Parameter | Value |
|-----------|-------|
| Metric | `routing_agreement = count(same_topk_experts) / total_tokens` |
| Comparison | Per-token, per-layer: do the top-K selected expert indices match between teacher and student? |
| Pass threshold | >= 0.85 (85% of tokens route to the same expert set as the teacher) |
| Fail action | Trigger targeted router repair via `ContrastiveTrainer` (AD-19, AD-20) with triplets generated from the misrouted token positions |

Teacher traces are recorded once and cached as JSONL. The ternary model is evaluated against these cached traces on every pipeline run. Agreement is measured at the expert-set level (order-invariant): if teacher selects experts {2, 5} and student selects {5, 2}, this counts as agreement.

**Gate 2 — Citation Correctness**

For retrieval-augmented responses, verify that citations are grounded in the actual retrieval corpus. This gate requires a labeled subset of the 200-prompt suite where prompts include retrieval context with known chunk IDs.

| Parameter | Value |
|-----------|-------|
| Metric (precision) | `citation_precision = valid_citations / total_citations` |
| Metric (recall) | `citation_recall = cited_evidence / relevant_evidence` (from labeled prompts) |
| Validity check | For each cited `chunk_id`: (1) chunk exists in retrieval corpus, (2) cited span is an exact substring match OR Jaccard similarity between cited span and chunk content > 0.6 |
| Pass threshold | Precision >= 0.90, Recall >= 0.70 |
| Fail action | Trigger retrieval-first policy training via `GrpoOptimizer` (GRPO reward penalizes hallucinated citations, rewards grounded ones) |

Jaccard similarity is computed at the word level: `|intersection(words_cited, words_chunk)| / |union(words_cited, words_chunk)|`. This catches paraphrased citations while rejecting fabricated ones. The 0.6 threshold was chosen to allow minor rephrasing while catching wholesale fabrication.

**Gate 3 — Refusal Calibration**

Test the model's ability to refuse when evidence is insufficient and answer when evidence is adequate. Uses the auto-labeled prompt suite (see below) where each prompt is classified as `resolved`, `contested`, or `indeterminate`.

| Parameter | Value |
|-----------|-------|
| Metric | `refusal_f1 = harmonic_mean(refusal_precision, refusal_recall)` |
| Refusal detection | Output contains a refusal signal (configurable string set, e.g., "I cannot determine", "insufficient evidence", "I'm not sure", or a structured `<refusal>` tag) |
| Must-refuse rate | Model must refuse >= 80% of `indeterminate` prompts |
| Must-answer rate | Model must NOT refuse >= 95% of `resolved` prompts |
| Pass threshold | Refusal F1 >= 0.85 |
| Fail action | Adjust refusal threshold in controller policy, or retrain controller via `GrpoOptimizer` with refusal-aware reward signal |

`contested` prompts (sources actively contradict) are evaluated separately and not gated — they are tracked for monitoring but the correct behavior (refuse vs. present both sides) is domain-dependent.

**Trace Schema (JSONL format)**

Every evaluation run produces a JSONL trace file where each line records per-token, per-layer routing decisions alongside response-level citation and refusal assessments:

```json
{
  "prompt_id": "p-001",
  "token_idx": 42,
  "layer_idx": 3,
  "routing": {
    "topk_expert_ids": [2, 5],
    "topk_weights": [0.62, 0.38],
    "teacher_expert_ids": [2, 5],
    "teacher_weights": [0.65, 0.35],
    "agreement": true
  },
  "citations": [
    {"chunk_id": "doc-17-p3", "span": "exact quoted text", "valid": true}
  ],
  "refusal": {
    "should_refuse": false,
    "did_refuse": false,
    "correct": true
  },
  "coherence_score": 0.91,
  "stop_reason": "eos"
}
```

Schema notes:
- `routing` is emitted per-token per-layer (one record per token-layer pair)
- `citations` and `refusal` are emitted once per response (attached to the final token record, `stop_reason != null`)
- `coherence_score` is the cosine similarity between student and teacher hidden states at the final layer — a cheap proxy for output quality without LLM-judge
- Trace files are stored in `eval/traces/` (never in the project root) and named `{model_version}_{prompt_suite}_{timestamp}.jsonl`

**Auto-Labeling Strategy**

The 200-prompt evaluation suite is labeled without manual annotation by using RuVector retrieval signals as proxy ground truth:

| Label | Condition | Meaning | Gate Usage |
|-------|-----------|---------|------------|
| `resolved` | Evidence redundancy > 3 (multiple independent sources agree on the answer) | The question is clearly answerable from the corpus | Gate 3: model must answer (not refuse) |
| `contested` | Cluster disagreement > 0.4 (sources actively contradict each other) | The question has conflicting evidence | Monitored only (not gated) |
| `indeterminate` | Mincut fragility > 0.7 (removing a single source breaks the entire evidence chain) | The question cannot be reliably answered | Gate 3: model must refuse |

These labels also feed Gate 2:
- `resolved` prompts provide the `relevant_evidence` denominator for citation recall (all supporting chunks should be cited)
- `indeterminate` prompts should produce no citations (any citation on an indeterminate prompt is likely hallucinated)

Auto-labeling is deterministic given a fixed retrieval corpus and runs on CPU via existing RuVector HNSW search. Labels are stored alongside prompts in the evaluation suite and versioned with the corpus.

**Go/No-Go Rule**

All three gates must pass on the same evaluation suite run for the system to ship:

```
SHIP = (routing_agreement >= 0.85)
     AND (citation_precision >= 0.90)
     AND (citation_recall >= 0.70)
     AND (refusal_f1 >= 0.85)
```

If any gate fails, the system cannot ship. The remediation path is gate-specific:

| Failed Gate | Remediation | Component | Estimated Duration |
|-------------|-------------|-----------|-------------------|
| Routing Correctness | Router repair via `ContrastiveTrainer` with misrouted-token triplets | `training/contrastive.rs` | 1-4 hours |
| Citation Correctness | Retrieval-first policy training via `GrpoOptimizer` (reward grounded citations) | `training/grpo.rs` | 2-8 hours |
| Refusal Calibration | Adjust refusal threshold or retrain controller policy via `GrpoOptimizer` | `training/grpo.rs` + controller config | 1-2 hours |

Re-evaluation after remediation must re-run all three gates (not just the failed one) to confirm no regression.

**Implementation location:**

| Component | Path | Lines | Notes |
|-----------|------|-------|-------|
| Gate runner orchestrator | `crates/ruvllm/src/eval/gates.rs` | ~300 | New module; runs all three gates, produces trace JSONL |
| Routing trace recorder | `crates/ruvllm/src/eval/routing_trace.rs` | ~150 | Records teacher routing decisions; compares against student |
| Citation validator | `crates/ruvllm/src/eval/citation_check.rs` | ~200 | Substring match + Jaccard similarity; corpus lookup |
| Refusal detector | `crates/ruvllm/src/eval/refusal_detect.rs` | ~100 | Configurable refusal signal set; F1 computation |
| Auto-labeler | `crates/ruvllm/src/eval/auto_label.rs` | ~150 | RuVector signal extraction; prompt classification |
| Trace schema types | `crates/ruvllm/src/eval/trace.rs` | ~80 | Serde-annotated structs matching the JSONL schema |
| **Total new code** | | **~980** | All CPU-only, no external dependencies |

**Exit criteria:**
- [ ] Teacher routing traces recorded for full 200-prompt suite and cached as JSONL
- [ ] Gate 1 (routing agreement) runs in < 30 minutes on Mac Studio for 200 prompts
- [ ] Gate 2 (citation correctness) validates chunk_id existence and span grounding
- [ ] Gate 3 (refusal calibration) correctly classifies refusal signals in model output
- [ ] Auto-labeler produces `resolved`/`contested`/`indeterminate` labels from RuVector signals
- [ ] All gates produce deterministic results (same inputs = same pass/fail, bit-exact)
- [ ] Trace JSONL files are written to `eval/traces/`, never to project root
- [ ] Go/No-Go rule enforced: all three gates must pass on same run
- [ ] Failed gate triggers correct remediation path (ContrastiveTrainer or GrpoOptimizer)
- [ ] Total eval suite runtime < 2 hours on Mac Studio (CPU-only)

---

### AD-23: Phase-1 Distillation via External GPU Teacher Artifacts

**Status**: Accepted

**Context**: The Ultra 30B ternary MoE system prioritizes CPU-first inference, integrity-driven behavior, and low operational cost. Phase-1 performance goals focus on routing correctness after ternary quantization, citation-grounded answers, and calibrated refusal under thin or conflicting evidence. Full end-to-end GPU distillation of a 30B teacher is expensive, slow, and misaligned with the system's long-term architecture — where RuVector provides memory and structure, and the generator model is intentionally small and cheap. However, pure PTQ ternary conversion (Phase 0) introduces unacceptable degradation in MoE routing stability, answer fidelity on contested prompts, and refusal behavior calibration. We therefore require a limited refinement phase that recovers task-relevant behavior without committing to ongoing GPU dependence.

**Decision**: Phase-1 distillation SHALL be implemented as a **one-time, external GPU artifact generation step**, followed by **local CPU-only refinement**.

1. A full-precision FP16 teacher is executed once on a short-lived cloud GPU instance
2. The teacher produces **behavioral artifacts, not trained weights**
3. All refinement and training occurs locally on CPU using these artifacts
4. GPU infrastructure is not a runtime dependency

**Scope of Teacher Artifacts** (GPU job exports only):

| Artifact | Content | Purpose |
|----------|---------|---------|
| **Routing Traces** | Per token, per MoE layer: top-k expert indices + routing probabilities/margins | Preserve expert selection behavior post-quantization |
| **Sparse Logits** | Answer spans, refusal boundaries, contradiction disclosure points only | Guide LoRA residual correction and refusal calibration without full sequence distillation |
| **Preference Labels** | Per-prompt classification: resolved / contested / indeterminate | Train stop decisions and disclosure behavior |

Artifacts SHALL be stored as immutable, versioned files and reused across refinement runs.

**CPU-Only Refinement Strategy** (using teacher artifacts):

1. **Router Repair** — Match student top-k routing to teacher traces; penalize expert churn and margin collapse
2. **Low-Rank Residual Correction** — Apply LoRA-style residuals to compensate ternary approximation error; enforce strict parameter budget
3. **EWC++ Preservation** — Prevent catastrophic drift outside repaired regions
4. **Policy Optimization** — Train RLM stop and retrieval behavior; optimize for citation correctness and calibrated refusal

No full expert weight updates are allowed in Phase-1.

**Evaluation Gate**: A checkpoint SHALL NOT be promoted unless it passes behavioral evaluation, not reconstruction metrics. Mandatory metrics:

| Metric | Criterion | Gate |
|--------|-----------|------|
| Routing correctness | Top-k overlap with teacher + margin correlation | Gate 1 (AD-22) |
| Citation correctness | Span hash verification + evidence support via RuVector | Gate 2 (AD-22) |
| Refusal calibration | Refuse on indeterminate, disclose on contested, pass on resolved | Gate 3 (AD-22) |

`compute_dequant_error` is a sanity check only, not a promotion criterion.

**Acceptance Criteria**:

- [ ] System passes the 200-prompt disagreement suite
- [ ] Routing correctness meets Gate 1 threshold (>= 0.85)
- [ ] Citation precision exceeds 0.90 (Gate 2 precision target)
- [ ] Refusal behavior aligns with RuVector coherence signals (Gate 3 F1 >= 0.85)
- [ ] Results remain stable under 10% corpus perturbation
- [ ] GPU artifact generation completes in single cloud session (< 4 hours)
- [ ] CPU refinement reproducible without GPU access

**Alternatives Considered**:

| Alternative | Verdict | Reason |
|-------------|---------|--------|
| Full GPU distillation | Rejected | High cost, long iteration cycles, misalignment with CPU-first design |
| Pure PTQ without refinement | Rejected | Unacceptable routing instability, incorrect refusal behavior, citation degradation |
| Continuous GPU shadow training | Rejected | Operational complexity, long-term infrastructure lock-in |

**Consequences**:

- *Positive*: GPU cost is bounded and minimal; refinement is repeatable and auditable; CPU-first deployment remains intact; system behavior aligns with integrity goals; distillation artifacts are reusable
- *Negative*: General language quality parity with FP16 teacher is not guaranteed; some PTQ loss may remain in non-critical behaviors; requires building custom evaluation infrastructure (addressed by AD-22)
- *Note*: This ADR does not preclude a future Phase-2 distillation if product requirements shift toward general language parity. Phase-2 would be a separate decision

---

### AD-24: RLM-Style Recursive Sentence Transformer Embedder

**Status**: Accepted

**Context**: The Craftsman Ultra system uses RuVector for evidence retrieval, cluster analysis, contradiction detection, and mincut fragility scoring. Standard sentence transformers produce embeddings in a single forward pass — one chunk in, one vector out. This works for basic retrieval but fails at three critical boundaries:

1. **Contradiction boundaries**: Two chunks with opposing claims embed near each other because they share vocabulary, despite being semantically opposed
2. **Domain drift**: Embeddings trained on general corpora perform poorly when the corpus shifts to a specialized domain (legal, medical, code)
3. **Context blindness**: The embedding of a chunk is independent of its neighborhood, losing structural signals that RuVector already knows (entity links, claim chains, cluster membership)

A normal embedding pipeline cannot distinguish "Drug X cures condition Y" from "Drug X does NOT cure condition Y" — they embed almost identically. The system needs embeddings that reflect the structural position of a chunk within the evidence graph, not just its surface semantics.

**Decision**: Implement an **RLM-style recursive embedder** — not a new architecture, but an inference strategy that wraps any base sentence transformer in a short iterative loop that retrieves context, decomposes, re-embeds, and merges.

**Core Loop** (bounded to 2-3 iterations):

```
State: { text, intent, neighbors, candidate_embeddings, iteration, stop_reason }

1. Embed the base chunk                           → base_embedding
2. Retrieve k nearest neighbors from RuVector      → neighbors[]
3. Normalize/summarize chunk with neighbor context  → contextualized_text
4. Re-embed the normalized view                    → ctx_embedding
5. If contested (low-cut boundary), embed both     → cluster_a_emb, cluster_b_emb
   sides of the disagreement separately
6. Merge into final representation                 → final_embedding + metadata
```

**Output Schema**:

| Field | Type | Description |
|-------|------|-------------|
| `embedding` | `Vec<f32>` | Final merged embedding vector |
| `confidence` | `f32` | Embedding stability across iterations (cosine similarity between iteration N and N-1) |
| `evidence_neighbor_ids` | `Vec<String>` | RuVector chunk IDs used as context |
| `contradiction_flags` | `Vec<bool>` | Per-neighbor: true if neighbor is in opposing cluster |
| `cluster_id` | `Option<usize>` | Primary cluster assignment |
| `stop_reason` | `StopReason` | Why the loop terminated: `Converged`, `MaxIterations`, `Contested` |

**Three Embedding Variants**:

| Variant | Conditioning | Use Case | Output |
|---------|-------------|----------|--------|
| **A: Query-Conditioned** | Query text + neighborhood | Retrieval under a specific query | Embedding optimized for that query's intent |
| **B: Corpus-Conditioned** | Stable neighbors + entity graph | Corpus indexing | Embedding stable over time, less sensitive to local phrasing |
| **C: Contradiction-Aware Twin** | Both sides of a low-cut boundary | Disputed claims | Bimodal representation: one embedding per cluster side |

**Merge Rule** (auditable, not learned):

```
final = normalize(w0 * base + w1 * ctx + w2 * anti)
```

Where `anti` is the embedding of the strongest counter-cluster neighbor set. Weights can be fixed (`w0=0.6, w1=0.3, w2=0.1`) or learned with a small regression on the eval set.

**Training Strategy** (minimal, no full model training):

Only three components are trainable:
1. **Merge weights** (`w0, w1, w2`) — 3 parameters, learned via grid search or small regression
2. **Stop policy** — when to terminate the loop (convergence threshold on cosine similarity between iterations)
3. **Adapter layer** — optional small linear layer on top of base embeddings for domain adaptation (rank-4 LoRA or single linear)

**Evaluation Criteria**:

| Metric | Definition | Target |
|--------|-----------|--------|
| Top-k retrieval accuracy | Correct chunk in top-k results | Improvement over single-pass baseline |
| False neighbor rate | Contradicting chunks incorrectly ranked as similar | Reduction vs baseline |
| Cluster purity | Intra-cluster coherence after re-embedding | Improvement vs baseline |
| Contradiction separation | Cosine distance between opposing claim embeddings | > 0.3 (vs ~0.05 for single-pass) |
| Stability under perturbation | Embedding change when 10% of corpus is modified | < 0.05 cosine drift |
| Latency per embedding | Wall time including retrieval + re-embedding | < 50ms for 2 iterations on target hardware |

**Appliance Fit** (CPU-first):

- Small base embedder model (e.g., 22M-110M params)
- 2-3 passes maximum per chunk
- RuVector supplies all context (no additional retrieval infrastructure)
- Ternary quantization of the base embedder is possible (future AD)
- Compatible with WASM deployment for browser-side embedding

**Acceptance Criteria**:

- [ ] On a held-out corpus slice, RLM-style embedder improves top-k retrieval accuracy vs single-pass baseline
- [ ] False neighbor matches near contradiction boundaries are reduced
- [ ] Latency stays within budget (< 50ms for 2 iterations on target hardware)
- [ ] Memory usage does not exceed appliance budget
- [ ] Variant C produces measurably separated embeddings for known contradictions
- [ ] Merge weights are interpretable and auditable (no black-box learned fusion)

---

## Consequences

### Positive

1. **CPU-only deployment**: 30B-class model running on commodity hardware without GPU
2. **Energy efficiency**: 55-82% reduction in inference energy vs FP16
3. **Memory efficiency**: ~8GB vs ~60GB for FP16 30B model (7.5x reduction)
4. **Multiplication-free expert GEMM**: Integer addition only in expert forward passes
5. **SONA compatibility**: MicroLoRA adaptation preserves per-session learning
6. **GGUF ecosystem**: Compatible with existing model distribution infrastructure
7. **Incremental path**: Phase 0 ($0) validates pipeline; Phase 0.5 ($0) adds RLM quality boost; Phase 1 ($1,300) delivers production quality; Phases 2-3 optimize
8. **~70% RLM code reuse**: GRPO, EWC++, ContrastiveTrainer, MemoryDistiller, PolicyStore are production-tested — only BitLinear layer and orchestrator are net-new
9. **Adaptive distillation**: GRPO reward scaling dynamically focuses compute on hard-to-distill experts
10. **Cross-expert stability**: EWC++ Fisher diagonal prevents catastrophic forgetting during sequential expert distillation
11. **Learned quantization policies**: PolicyStore persists per-layer ternary scale distributions for reproducible future distillation runs
12. **Expert-parallel distillation**: Independent expert FFNs enable rayon-parallel distillation across CPU cores
13. **Phase 0 de-risks Phase 1 at zero cost**: Mac Studio PTQ prototype validates entire inference pipeline (GGUF → dequant → kernel → MoE → generation) for $0 before committing $1,300+ to cloud GPU distillation
14. **Existing GGUF ecosystem**: Community-published GLM-4.7-Flash GGUFs (bartowski, unsloth) available as comparison baselines
15. **Phase 0.5 RLM refinement at $0**: Existing MicroLoRA + GRPO + EWC++ + ContrastiveTrainer stack provides ~10-15 percentage point quality recovery over raw PTQ with zero new training code, running entirely on Mac Studio
16. **100% RLM reuse for Phase 0.5**: No new training infrastructure needed — all 7 RLM components are production-tested and wire together directly
17. **SIMD-only Phase 0.5**: Entire RLM refinement pipeline runs on pure CPU SIMD (NEON on aarch64) without Metal GPU — only ~2-3x slower than Metal, extends platform support to Linux ARM64 and (with scalar fallback) x86
18. **Zero-config SIMD mode**: All training components (MicroLoRA, TrainingPipeline, EwcRegularizer, GrpoOptimizer) are already GPU-agnostic; only `ContrastiveTrainer.use_metal = false` needed for full SIMD-only execution
19. **WASM browser deployment**: Native Rust kernels compile to WASM SIMD128 via Cargo feature flags, enabling in-browser ternary inference at ~20-40 tok/s without server roundtrip
20. **Single-source dual-target**: One Rust codebase compiles to both native SIMD (NEON/AVX2/AVX512) and WASM SIMD128, eliminating the need for separate C++ and JS codebases
21. **Safe Rust kernels**: Following R3-Engine's approach, production kernels can be 100% Safe Rust (no `unsafe` in hot path), eliminating entire classes of memory safety bugs vs bitnet.cpp's C++
22. **Existing Rust ecosystem**: R3-Engine (Apache-compatible) and bitnet.rs (Apache 2.0) provide proven reference implementations to accelerate kernel development
23. **Deterministic behavioral gates**: Three non-LLM-judge evaluation gates (routing, citation, refusal) provide reproducible pass/fail shipping decisions without expensive API calls or non-deterministic judge models
24. **Structured trace schema**: JSONL trace format captures per-token routing, per-response citation, and refusal decisions in a single auditable artifact — enables regression detection across model versions
25. **Zero-annotation auto-labeling**: RuVector retrieval signals (evidence redundancy, cluster disagreement, mincut fragility) classify prompts as resolved/contested/indeterminate without human annotation effort
26. **Gate-specific remediation**: Each failed gate maps to a concrete repair action using existing RLM components (ContrastiveTrainer for routing, GrpoOptimizer for citations and refusal), avoiding manual debugging cycles
27. **CPU-only evaluation**: Full eval suite runs on Mac Studio in < 2 hours with no cloud GPU or external API dependency, keeping the evaluation loop at $0 marginal cost
28. **Bounded GPU cost**: Phase-1 distillation requires only a single short-lived cloud GPU session to generate behavioral artifacts (routing traces, sparse logits, preference labels) — no ongoing GPU dependency
29. **Artifact reusability**: Teacher artifacts are immutable and versioned; CPU refinement runs can be repeated, tuned, and audited without re-running the GPU job
30. **Behavioral distillation**: Distilling routing decisions and refusal signals rather than full logit sequences aligns training objectives with the system's integrity-first design goal
31. **RLM-style embeddings**: Recursive context-aware embeddings improve retrieval accuracy and contradiction separation without requiring a larger embedding model — inference strategy, not new architecture
32. **Contradiction-aware twin embeddings**: Variant C produces bimodal representations at low-cut boundaries, preserving disagreement structure in the embedding space for downstream decision-making
33. **Minimal training surface**: Only 3 merge weights + stop policy + optional adapter need training for the RLM embedder — no full model fine-tuning required

### Negative

1. **Training cost**: Even distillation requires 800-1,600 A100-hours (~$2K-$5K cloud cost)
2. **Custom kernels**: Must implement and maintain platform-specific SIMD kernels in Rust
3. **Quality gap**: Phase 1 may be 5-10% below GLM-4.7-Flash on some benchmarks
4. **No GPU acceleration**: BitNet kernels are CPU-specific; GPU path requires separate optimization
5. **Mixed-precision complexity**: Router (FP16) + experts (ternary) + attention (FP16/ternary) adds dispatch complexity
6. **WASM SIMD128 ceiling**: Fixed 128-bit width limits throughput vs native AVX2 (256-bit) or AVX512 (512-bit); no popcount instruction requires emulation; single-threaded unless SharedArrayBuffer enabled — expect ~20-40 tok/s vs ~80-117 tok/s native
7. **RLM scale gap**: Existing `RealContrastiveTrainer` targets 0.5B models (embedding_dim=896); scaling to 30B requires distributed data loading and increased batch sizes
8. **No x86 SIMD kernels**: Current `kernels/matmul.rs` only implements NEON (aarch64); x86 falls to scalar fallback (~3-5x slower than NEON). Adding AVX2/AVX512 kernels would make x86 SIMD-only mode competitive but is not yet implemented
9. **Teacher trace dependency**: Gate 1 requires a full FP16 teacher forward pass to generate ground-truth routing traces; this must be re-run whenever the evaluation suite changes or the teacher model is updated
10. **Auto-label noise**: RuVector-derived labels (evidence redundancy, mincut fragility) are proxies for true answerability; edge cases near thresholds (e.g., fragility = 0.69 vs 0.71) may produce inconsistent labels across corpus versions
11. **200-prompt suite coverage**: A fixed 200-prompt suite may not cover all failure modes; adversarial or distribution-shifted prompts could pass all gates yet fail in production
12. **General quality ceiling**: Phase-1 behavioral distillation intentionally does not target full language quality parity with FP16 teacher; non-critical behaviors may remain degraded
13. **Teacher artifact staleness**: If the evaluation prompt suite or teacher model changes, routing traces and preference labels must be regenerated on GPU

### Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Phase 0 PTQ quality too low for meaningful testing | Medium | Low | Phase 0 is for kernel/pipeline validation, not quality; upgrade to 0D (BitDistill Lite) if needed |
| MoE routing degrades with ternary experts | Medium | High | Phase 0 detects routing issues early; Phase 1 validates routing; router stays FP16; AD-12 contrastive validation |
| bitnet.cpp kernel translation to Rust introduces bugs | Medium | Medium | Phase 0 PTQ model provides cheap test fixture; extensive kernel unit tests; validate against reference impl |
| Distillation fails to converge for MoE | Low | High | GRPO reward scaling + per-expert distillation fallback; EWC++ stability (AD-13) |
| GLM-4.7-Flash architecture changes break compatibility | Low | Medium | Pin to specific HF revision; architecture abstraction layer |
| IQ1_S GGUF format insufficient for absmean metadata | Medium | Low | Register custom GGUF type (BITNET_T158); backward-compatible extension |
| EWC++ Fisher accumulation OOM at 30B scale | Medium | Medium | Sparse Fisher (top-k diagonal entries); per-expert rather than global Fisher |
| GRPO reward signal too noisy for distillation | Low | Low | Fall back to static KD loss; GRPO reward as optional multiplier |
| `RealContrastiveTrainer` doesn't scale to 30B | Medium | Medium | Extract training loop; replace Candle Linear with BitLinear; keep optimizer/scheduler |
| Calibration data bias in Phase 0 PTQ | Low | Low | Use diverse calibration corpus (WikiText + code); measure variance across calibration sets |
| Auto-label thresholds misclassify edge-case prompts | Medium | Medium | Track label stability across corpus versions; flag prompts with signals near threshold boundaries for manual review |
| 200-prompt suite insufficient for production coverage | Low | Medium | Expand suite iteratively as production failure modes are discovered; run gates on user-submitted adversarial prompts quarterly |
| Teacher routing traces become stale after model update | Low | Low | Re-record teacher traces as part of every model version bump; cache invalidation keyed on teacher model hash |

---

## Validation Criteria

### Phase 0 Exit Criteria
- [ ] Absmean ternary quantizer produces valid {-1, 0, +1} weights from GLM-4.7-Flash FP16
- [ ] Quantization runs successfully on Mac Studio via mmap (no cloud GPU required)
- [ ] GGUF export with BITNET_T158 tensor type loads without error in BitNetBackend
- [ ] TL1 NEON kernel produces non-zero, bounded output on PTQ ternary weights
- [ ] MoE routing selects experts (not all-zero or all-same-expert degenerate routing)
- [ ] End-to-end token generation produces coherent (if degraded) text
- [ ] Memory usage measured and documented for real MoE activation patterns
- [ ] Throughput measured: tok/s on Mac Studio (ARM NEON) and optionally x86 AVX2
- [ ] Baseline quality benchmarks recorded (HumanEval, MMLU) as Phase 1 improvement target
- [ ] Total Phase 0 cost = $0 (local Mac Studio execution)

### Phase 0.5 Exit Criteria
- [ ] MicroLoRA adapters (rank-2) attached to all expert FFN layers
- [ ] Router fine-tuning via ContrastiveTrainer restores >=90% routing accuracy vs teacher
- [ ] GRPO reward signal shows positive quality improvement over Phase 0 baseline
- [ ] EWC++ prevents router fix from degrading already-correct routing paths (Fisher delta < 5%)
- [ ] HumanEval pass@1 >= 45% (up from Phase 0 baseline of ~35-45%)
- [ ] MicroLoRA + ternary inference produces coherent code completions
- [ ] Training completes on Mac Studio within 14 days
- [ ] MemoryDistiller has extracted KeyLessons identifying worst-degraded experts
- [ ] PolicyStore contains optimized TernaryScale entries for all refined layers
- [ ] Total Phase 0.5 cost = $0 (local Mac Studio execution)
- [ ] GGUF re-exported with optimized router, scale factors, and LoRA adapter weights

### Phase 1 Exit Criteria
- [ ] BitNet backend loads GGUF with ternary expert weights
- [ ] TL1 kernel produces bit-exact output vs reference float implementation
- [ ] Decode speed >= 5 tok/s on x86_64 AVX2 (AMD Ryzen 7 / Intel i7 class)
- [ ] HumanEval pass@1 >= 50% (GLM-4.7-Flash baseline: ~65%)
- [ ] Memory usage < 10GB for 4K context inference
- [ ] GRPO-guided expert distillation converges (loss < 0.5 for all experts)
- [ ] EWC++ prevents cross-expert interference (Fisher-regularized loss delta < 5%)
- [ ] Contrastive router validation: >= 95% expert routing accuracy vs teacher
- [ ] PolicyStore contains TernaryScale entries for all distilled expert layers

### Phase 2 Exit Criteria
- [ ] Full ternary model (attention + experts) running on CPU
- [ ] Decode speed >= 8 tok/s on x86_64 AVX2
- [ ] SWE-bench Verified >= 52% (90%+ of GLM-4.7-Flash's 59.2%)
- [ ] SONA MicroLoRA adaptation functional on ternary base
- [ ] MemoryDistiller has extracted >= 50 KeyLessons from distillation trajectories
- [ ] GRPO adaptive KL stabilizes below kl_target (0.02) for all experts

### Phase 3 Exit Criteria
- [ ] Native-trained model matches or exceeds GLM-4.7-Flash benchmarks
- [ ] Published on HuggingFace (ruv/craftsman-ultra-30b-1bit)
- [ ] GGUF + bitnet kernel distributed via npm/packages/ruvllm
- [ ] Full distillation pipeline reproducible from PolicyStore policies (no manual tuning)

---

## References

1. Ma, S. et al., "The Era of 1-bit LLMs: All Large Language Models are in 1.58 Bits" (arXiv:2402.17764, Feb 2024)
2. Ma, S. et al., "BitNet b1.58 2B4T Technical Report" (arXiv:2504.12285, Apr 2025)
3. Microsoft Research, "bitnet.cpp: Efficient Edge Inference for Ternary LLMs" (arXiv:2502.11880, Feb 2025)
4. Microsoft, bitnet.cpp — https://github.com/microsoft/BitNet
5. Zhipu AI, GLM-4.7-Flash — https://huggingface.co/zai-org/GLM-4.7-Flash
6. Zhipu AI, "GLM-4.7: Advancing the Coding Capability" — https://z.ai/blog/glm-4.7
7. RuvLLM ADR-002: RuvLLM Integration with Ruvector
8. RuvLLM GGUF Quantization Module: `crates/ruvllm/src/gguf/quantization.rs`
9. Microsoft, bitnet-b1.58-2B-4T-gguf — https://huggingface.co/microsoft/bitnet-b1.58-2B-4T-gguf
10. RuvLLM GRPO Implementation: `crates/ruvllm/src/training/grpo.rs`
11. RuvLLM RealContrastiveTrainer: `crates/ruvllm/src/training/real_trainer.rs`
12. RuvLLM EWC++ Training Pipeline: `crates/ruvllm/src/lora/training.rs`
13. RuvLLM Memory Distillation: `crates/ruvllm/src/reasoning_bank/distillation.rs`
14. RuvLLM Policy Store: `crates/ruvllm/src/policy_store.rs`
15. RuvLLM Contrastive Training: `crates/ruvllm/src/training/contrastive.rs`
16. PT-BitNet: "Scaling up the 1-Bit large language model with post-training quantization" (2025) — https://www.sciencedirect.com/science/article/abs/pii/S089360802500735X
17. BitDistill: "BitNet Distillation" (arXiv:2510.13998, Oct 2025) — https://arxiv.org/html/2510.13998v1
18. bartowski, GLM-4.7-Flash-GGUF quantizations — https://huggingface.co/bartowski/zai-org_GLM-4.7-Flash-GGUF
19. unsloth, GLM-4.7-Flash-GGUF dynamic quantizations — https://huggingface.co/unsloth/GLM-4.7-Flash-GGUF
20. llama.cpp IQ1_S blind testing (Discussion #5962) — https://github.com/ggml-org/llama.cpp/discussions/5962
21. STBLLM: "Breaking the 1-bit Barrier" (ICLR 2025) — https://proceedings.iclr.cc/paper_files/paper/2025/file/ff997469ac66cf893c4183efeb22212a-Paper-Conference.pdf
22. Apple Mac Studio Technical Specifications (2025) — https://www.apple.com/mac-studio/specs/
23. RuvLLM Metal GEMV integration: `crates/ruvllm/src/kernels/matmul.rs:1444-1582`
24. RuvLLM MicroLoRA NEON SIMD forward: `crates/ruvllm/src/lora/micro_lora.rs:279-390` (forward_simd, forward_simd_neon_impl)
25. RuvLLM NEON SIMD kernels: `crates/ruvllm/src/kernels/` (matmul: gemm_neon/gemv_neon, activations: silu_neon/gelu_neon/relu_neon, norm: rms_norm_neon, rope: apply_rope_neon)
26. RuvLLM ContrastiveTrainer CPU fallback: `crates/ruvllm/src/training/contrastive.rs:171-175` (Metal → CPU fallback) and `contrastive.rs:475` (non-Candle pure CPU path)
27. R3-Engine: Pure Rust BitNet inference engine with WASM SIMD128 — https://github.com/r3-engine/r3-engine
28. bitnet.rs: Pure Rust BitNet toolkit (Apache 2.0) — https://github.com/ocentra/bitnet.rs
29. WASM SIMD128 specification: Fixed-width 128-bit SIMD for WebAssembly — https://v8.dev/features/simd
30. Rust `core::arch::wasm32` SIMD intrinsics — https://doc.rust-lang.org/beta/core/arch/wasm32/index.html
31. "The state of SIMD in Rust in 2025" (Sergey Davidoff) — https://shnatsel.medium.com/the-state-of-simd-in-rust-in-2025-32c263e5f53d
32. "Rust + WebAssembly 2025: WasmGC and SIMD" — https://dev.to/dataformathub/rust-webassembly-2025-why-wasmgc-and-simd-change-everything-3ldh
33. Bai, Y. et al., "Constitutional AI: Harmlessness from AI Feedback" (arXiv:2212.08073, Dec 2022) — https://arxiv.org/abs/2212.08073
34. Zheng, L. et al., "Judging LLM-as-a-Judge with MT-Bench and Chatbot Arena" (arXiv:2306.05685, Jun 2023) — https://arxiv.org/abs/2306.05685
35. Rafailov, R. et al., "Direct Preference Optimization: Your Language Model is Secretly a Reward Model" (arXiv:2305.18290, May 2023) — https://arxiv.org/abs/2305.18290
36. Min, S. et al., "FActScore: Fine-grained Atomic Evaluation of Factual Precision in Long Form Text Generation" (arXiv:2305.14251, May 2023) — https://arxiv.org/abs/2305.14251
37. RuvLLM BitNet Backend: `crates/ruvllm/src/bitnet/backend.rs` (MoE routing, TL1 GEMV, forward pass)
38. RuvLLM RLM Refiner: `crates/ruvllm/src/bitnet/rlm_refiner.rs` (Phase 0.5 refinement orchestrator)
