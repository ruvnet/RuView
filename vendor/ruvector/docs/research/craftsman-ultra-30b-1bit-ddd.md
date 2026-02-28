# Domain-Driven Design: Craftsman Ultra 30b 1bit

**Version:** 2.4
**Date:** 2026-02-03
**Relates to:** ADR-017-craftsman-ultra-30b-1bit-bitnet-integration
**Status:** Research / Pre-Implementation

---

## 1. Strategic Domain Vision

Craftsman Ultra 30b 1bit is a CPU-native, 1-bit quantized coding/agentic LLM that merges BitNet b1.58 ternary inference with GLM-4.7-Flash's 30B-A3B MoE architecture. It operates within the RuvLLM serving runtime and leverages Ruvector for intelligent memory.

### Core Domain

**Ternary-Quantized Mixture-of-Experts Language Model Inference on CPU**

The domain encompasses:
- Loading and managing ternary-quantized model weights in GGUF format
- Routing tokens to sparse expert subsets via a gating network
- Executing forward passes using integer-addition-only GEMM kernels
- Managing mixed-precision compute across router (FP16), experts (ternary), and attention (FP16/ternary)
- Integrating with the SONA self-learning framework for per-session adaptation
- Serving inference results through the RuvLLM backend abstraction

### Subdomains

| Subdomain | Type | Description |
|-----------|------|-------------|
| Ternary Inference Engine | Core | BitNet kernel execution, GEMM, weight management |
| MoE Routing | Core | Expert gating, load balancing, capacity management |
| Model Lifecycle | Supporting | GGUF loading, weight initialization, memory mapping |
| Quantization Pipeline | Supporting | BitLinear training/distillation, ternary conversion |
| Kernel Dispatch | Supporting | Hardware detection, SIMD kernel selection |
| Adaptation Layer | Supporting | SONA MicroLoRA on ternary base, EWC++ consolidation |
| **RLM Training Orchestration** | **Supporting** | **GRPO rewards, contrastive validation, EWC++ stability, distillation quality tracking** |
| Serving Integration | Generic | Backend trait, NAPI bindings, session management |

---

## 2. Ubiquitous Language

The following terms have precise meaning within the Craftsman Ultra domain. All code, documentation, and communication must use these terms consistently.

| Term | Definition |
|------|-----------|
| **BitLinear** | A linear layer replacement where weights are ternary {-1, 0, +1} and activations are INT8. Forward pass uses integer addition only. |
| **Ternary Weight** | A model weight constrained to exactly three values: -1, 0, or +1. Encoded using 2 bits per weight. |
| **Absmean Quantization** | The method of converting FP16/BF16 weights to ternary: `W_t = RoundClip(W / mean(\|W\|), -1, 1)`. |
| **Absmax Activation** | Per-token INT8 quantization of activations: `X_q = round(X * 127 / max(\|X\|))`. |
| **Expert** | A sparse MLP sub-network within a MoE layer. Only K experts activate per token out of N total. |
| **Router / Gating Network** | FP16 linear layer that computes softmax scores to select which experts process each token. |
| **Active Parameters** | The ~3B parameters actually executing computation for any given token (selected experts + shared layers). |
| **Total Parameters** | The full ~30B parameter count across all experts and shared layers. |
| **TL1 Kernel** | Ternary Lookup Table kernel: packs 2 weights into a 4-bit LUT index. Balanced CPU performance. |
| **TL2 Kernel** | Ternary Lookup Table kernel: packs 3 weights into a 5-bit LUT index. Higher compression, lower bandwidth. |
| **I2_S Kernel** | Integer-2 with Scale kernel: stores ternary as 2-bit, unpacks to compute. Best for high-bandwidth hardware. |
| **Pack-and-Unpack** | Technique to maintain INT16 accumulation precision during LUT-based GEMM without lossy int8 requantization. |
| **Feature Filtering** | Zero-valued ternary weights effectively mask input features, providing implicit sparsity within dense layers. |
| **Shadow Weights** | FP16 weights maintained during training that are quantized to ternary for forward passes (dropped after training). |
| **Straight-Through Estimator (STE)** | Gradient approximation that passes gradients through the ternary rounding operation during backpropagation. |
| **Scale Factor** | Per-block FP16 value (the absmean) used to rescale ternary GEMM output back to float. |
| **Block** | A group of 256 contiguous weights sharing one scale factor. The fundamental unit of ternary storage. |
| **Mixed-Precision Forward** | A forward pass where different components use different precisions (FP16 router, ternary experts, Q8 activations). |
| **Capacity Factor** | MoE parameter controlling maximum tokens per expert to prevent routing collapse. |
| **Expert Parallelism** | Distributing different experts across different CPU cores for concurrent execution. |
| **GRPO** | Group Relative Policy Optimization. Critic-free RL algorithm that computes advantages within sample groups, used to scale distillation loss per-expert. |
| **SampleGroup** | A batch of teacher-vs-student comparisons for one expert, used by GRPO to compute relative advantages. |
| **Relative Advantage** | Per-sample reward normalized against group mean: `(reward - mean) / std`. Drives GRPO update direction. |
| **Adaptive KL** | Dynamic KL divergence penalty that increases when student diverges too far from teacher, decreases when converging. |
| **EWC++ (Elastic Weight Consolidation)** | Continual learning regularizer: `lambda/2 * Sigma F_i * (w_i - w*_i)^2`. Prevents catastrophic forgetting during sequential expert distillation. |
| **Fisher Diagonal** | Per-parameter importance weights computed from gradient magnitudes. Higher Fisher = more important to preserve. |
| **KeyLesson** | Extracted insight from distillation trajectories (e.g., "Expert 7 gate_proj converges fastest with lr=2e-6"). Persisted in ReasoningBank. |
| **TernaryScalePolicy** | Per-layer metadata (mean scale, sparsity, quality) persisted in PolicyStore to guide future distillation. |
| **Contrastive Router Validation** | Post-ternary-conversion check that MoE routing still selects correct experts, using triplet loss on expert embeddings. |
| **Knowledge Distillation Loss** | `alpha * KL(teacher/T, student/T) + (1-alpha) * CE(labels, student)`. Core training objective for ternary student. |
| **Distillation Trajectory** | Sequence of training steps for one expert, recorded as ReasoningBank `Trajectory` for quality analysis. |
| **PT-BitNet** | Post-Training BitNet quantization: applying absmean ternary conversion to pre-trained FP16 weights with optional calibration. No training loop — just quantize and export. |
| **Calibration Pass** | Forward pass of ~1000 samples through the teacher model to record activation statistics used to optimize ternary scale factors. |
| **IQ1_S** | llama.cpp's 1.56 bpw importance quantization format. Codebook-based, dequant-then-multiply — NOT multiplication-free like BitNet. |
| **BITNET_T158** | Proposed GGUF tensor type for native BitNet b1.58 ternary weights (2-bit packed + FP16 per-block absmean scale). Distinct from IQ1_S. |
| **Phase 0 Prototype** | PT-BitNet quantized model used for inference pipeline validation and kernel testing, not production quality. |
| **RLM Refinement** | Training only the FP16 components (LoRA, router, scales) of a PTQ model using the existing RLM stack, with ternary weights frozen. |
| **Frozen Ternary** | Expert FFN weights locked to their PTQ {-1,0,+1} values during Phase 0.5 refinement — not differentiable, not modified. |
| **LoRA Correction** | Small FP16 additive output from MicroLoRA that compensates for ternary quantization error: `Y = BitLinear(X) + LoRA(X)`. |
| **Router Repair** | Contrastive fine-tuning of FP16 router weights to correct misrouting caused by expert output distribution changes after PTQ. |
| **SIMD-Only Mode** | Phase 0.5 execution mode where all training runs on pure CPU SIMD (NEON on aarch64) without Metal GPU. All RLM components are GPU-agnostic except ContrastiveTrainer which has an explicit CPU fallback path. ~2-3x slower than Metal but extends platform support beyond macOS. |
| **NEON Intrinsics** | ARM SIMD instruction set used by MicroLoRA's `forward_simd_neon_impl()` for 8x-unrolled forward passes. Available on all Apple Silicon and ARM64 platforms. x86 platforms fall to scalar fallback. |
| **Scalar Fallback** | Platform-agnostic non-SIMD code path used when NEON (aarch64) is unavailable. Provides identical results at ~3-5x lower throughput. Enables Phase 0.5 on x86 Linux/Windows. |
| **WASM SIMD128** | WebAssembly's fixed-width 128-bit SIMD extension (v128 type). Enables ternary kernel execution in browsers at ~4-8x over scalar WASM. Supported in all major browsers. Maps TL1's 16-entry LUT to v128.swizzle. |
| **Dual-Target Compilation** | Cargo feature flag strategy where a single Rust codebase compiles to both native SIMD (NEON/AVX2/AVX512) and WASM SIMD128 via `#[cfg(target_arch)]` dispatch. |
| **Bit-Sliced Ternary Matrix** | R3-Engine's approach to ternary storage: weights packed into 64-byte cache-aligned lines, processed via bitwise AND + popcount instead of traditional LUT. Enables branchless integer math. |
| **VPOPCNTDQ** | AVX-512 vector population count instruction used by R3-Engine for ternary GEMM. Counts set bits in packed ternary representations to compute dot products via integer addition. |
| **Behavioral Gate** | A deterministic, non-LLM-judge evaluation checkpoint that tests a specific behavioral property (routing correctness, citation grounding, or refusal calibration). All gates must pass on the same evaluation run for the system to ship. |
| **Routing Agreement** | Fraction of tokens where the ternary student model selects the same top-K expert set as the FP16 teacher: `count(same_topk_experts) / total_tokens`. Measured per-token per-layer, order-invariant. Pass threshold: >= 0.85. |
| **Citation Precision** | Fraction of model-generated citations that are valid (cited chunk exists in corpus AND span matches or Jaccard > 0.6): `valid_citations / total_citations`. Pass threshold: >= 0.90. |
| **Citation Recall** | Fraction of relevant evidence in the corpus that the model actually cites: `cited_evidence / relevant_evidence`. Requires auto-labeled `resolved` prompts. Pass threshold: >= 0.70. |
| **Refusal F1** | Harmonic mean of refusal precision (fraction of refusals that are correct) and refusal recall (fraction of indeterminate prompts that are refused). Pass threshold: >= 0.85. |
| **Trace Schema** | JSONL format recording per-token routing decisions, per-response citation validity, and refusal correctness for every evaluation run. Each record includes `prompt_id`, `token_idx`, `layer_idx`, `routing`, `citations`, `refusal`, `coherence_score`, and `stop_reason`. |
| **Auto-Labeling** | Classification of evaluation prompts as `resolved` (evidence redundancy > 3), `contested` (cluster disagreement > 0.4), or `indeterminate` (mincut fragility > 0.7) using RuVector retrieval signals, without manual annotation. |
| **Go/No-Go Rule** | Shipping gate: all three behavioral gates (routing agreement >= 0.85, citation precision >= 0.90 AND recall >= 0.70, refusal F1 >= 0.85) must pass on the same evaluation suite run. Failure of any gate blocks release and triggers gate-specific remediation. |
| **Teacher Artifact** | Immutable, versioned output from a one-time FP16 teacher forward pass on a cloud GPU — includes routing traces (per-token expert selections and probabilities), sparse logits (answer spans, refusal boundaries, contradiction disclosure points), and preference labels (resolved/contested/indeterminate). Used for CPU-only refinement; not a runtime dependency. |
| **Behavioral Distillation** | Distilling task-relevant behavioral signals (expert routing, refusal decisions, citation patterns) rather than full sequence logits. Produces smaller artifacts, targets integrity-first objectives, and avoids training the student to imitate the teacher's general language behaviors. |
| **Router Repair** | Phase-1 CPU refinement step: match student top-k routing to teacher routing traces using contrastive training; penalize expert churn (frequent switching between experts across similar prompts) and margin collapse (routing probabilities converging toward uniform). |
| **Sparse Logits** | Teacher logits captured only at structurally important positions: answer spans, refusal boundaries, and contradiction disclosure points. Avoids the cost and noise of full-sequence logit distillation while providing targeted training signal for LoRA correction. |
| **Corpus Perturbation** | Stability test: remove 10% of the evidence corpus at random, re-run all three behavioral gates, and verify that results remain within threshold. A system that passes 200 prompts but fails under perturbation is overfitting to the specific corpus arrangement. |
| **RLM-Style Embedder** | An inference strategy (not architecture) that wraps a base sentence transformer in a 2-3 iteration loop: embed → retrieve neighbors → contextualize → re-embed → merge. Produces embeddings aware of their structural position in the evidence graph. |
| **Query-Conditioned Embedding** | Variant A: embedding a chunk conditioned on a specific query and its neighborhood, producing a vector optimized for retrieval under that query's intent. |
| **Corpus-Conditioned Embedding** | Variant B: embedding a chunk conditioned on stable neighbors and entity graph links, producing a vector that is stable over time and less sensitive to local phrasing changes. |
| **Contradiction-Aware Twin Embedding** | Variant C: when a chunk sits on a low-cut boundary, producing two embeddings — one aligned to each side of the disagreement — preserving bimodal structure in the embedding space. |
| **Merge Rule** | Auditable weighted combination of base, contextualized, and anti-cluster embeddings: `final = normalize(w0*base + w1*ctx + w2*anti)`. Weights are fixed or learned with minimal regression. |
| **Anti-Cluster Embedding** | The embedding of the strongest counter-cluster neighbor set for a chunk. Used in the merge rule to push the final embedding away from contradicting evidence, improving contradiction separation. |
| **Embedding Convergence** | Stop criterion for the recursive embedder: terminate when cosine similarity between iteration N and N-1 exceeds threshold (e.g., 0.98), indicating the embedding has stabilized. |

---

## 3. Bounded Contexts

### 3.1 Ternary Inference Context (Core)

**Responsibility**: Execute BitNet forward passes using ternary GEMM kernels.

**Owns:**
- BitLinear layer implementation
- TL1/TL2/I2_S kernel dispatch and execution
- Lookup table generation and caching
- INT8 activation quantization/dequantization
- Per-block scale factor management
- Pack-and-unpack accumulation

**Key Entities:**
- `TernaryTensor` — Packed 2-bit weight storage with per-block FP16 scales
- `BitLinearLayer` — Forward pass implementation using ternary GEMM
- `LookupTable` — Pre-computed activation sums for TL1/TL2 kernels
- `ActivationBuffer` — INT8 per-token quantized activation storage

**Invariants:**
- Ternary weights are immutable after model load (no in-place modification)
- GEMM output must be bit-exact with reference float implementation
- Accumulation uses INT16 minimum (no INT8 intermediate quantization)
- Scale factors are always FP16 (never quantized further)

**Interfaces:**
- **Inbound**: Receives FP16 activations from attention/router, quantizes to INT8
- **Outbound**: Produces FP16 output after dequantization with scale factors
- **Anti-corruption layer**: Validates tensor shapes match expected block alignment (mod 256)

```
┌─────────────────────────────────────────────┐
│         Ternary Inference Context            │
│                                             │
│  ┌──────────────┐    ┌──────────────────┐   │
│  │ TernaryTensor │───▶│  BitLinearLayer  │   │
│  │  (2-bit pack) │    │  (ternary GEMM)  │   │
│  └──────────────┘    └────────┬─────────┘   │
│                               │              │
│  ┌──────────────┐    ┌───────▼──────────┐   │
│  │ LookupTable  │───▶│ KernelDispatcher │   │
│  │ (TL1/TL2)    │    │ (SIMD selection) │   │
│  └──────────────┘    └──────────────────┘   │
│                                             │
│  ┌──────────────────────────────────────┐   │
│  │        ActivationBuffer              │   │
│  │  (INT8 per-token, absmax scaling)    │   │
│  └──────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

---

### 3.2 MoE Routing Context (Core)

**Responsibility**: Select which experts process each token and manage load balancing.

**Owns:**
- Gating network (FP16 linear + softmax)
- Top-K expert selection per token
- Capacity factor enforcement
- Load balancing loss computation (for training/distillation)
- Expert output aggregation (weighted sum)

**Key Entities:**
- `MoERouter` — Gating network computing expert selection scores
- `ExpertSelector` — Top-K selection with capacity constraints
- `ExpertPool` — Registry of available expert BitLinear layers
- `RoutingDecision` — Per-token mapping of token → selected experts + weights

**Invariants:**
- Router weights are always FP16 (never quantized to ternary)
- Exactly K experts are selected per token (no fallback to fewer)
- Expert output weights sum to 1.0 after normalization
- Capacity factor prevents any single expert from processing >CF× its fair share

**Interfaces:**
- **Inbound**: Receives hidden states from attention output (FP16)
- **Outbound**: Dispatches tokens to selected expert BitLinear layers, receives expert outputs, produces weighted sum
- **Upstream**: Consumes `BitLinearLayer` from Ternary Inference Context

```
┌─────────────────────────────────────────────┐
│           MoE Routing Context               │
│                                             │
│  ┌──────────────┐    ┌──────────────────┐   │
│  │  MoERouter   │───▶│ ExpertSelector   │   │
│  │ (FP16 gate)  │    │ (top-K + cap)    │   │
│  └──────────────┘    └────────┬─────────┘   │
│                               │              │
│            ┌──────────────────┼──────┐       │
│            ▼                  ▼      ▼       │
│  ┌─────────────┐  ┌──────────┐  ┌────────┐  │
│  │  Expert 0   │  │ Expert 1 │  │Expert N│  │
│  │(BitLinear)  │  │(BitLinear│  │(BitLin)│  │
│  └──────┬──────┘  └────┬─────┘  └───┬────┘  │
│         │              │             │       │
│         └──────────┬───┘─────────────┘       │
│                    ▼                         │
│           ┌────────────────┐                 │
│           │ WeightedSum    │                 │
│           │ (expert agg)   │                 │
│           └────────────────┘                 │
└─────────────────────────────────────────────┘
```

---

### 3.3 Model Lifecycle Context (Supporting)

**Responsibility**: Load, validate, and manage model artifacts in GGUF format.

**Owns:**
- GGUF file parsing and validation
- Tensor extraction and type detection (ternary vs FP16)
- Memory-mapped file management for large models
- Model metadata extraction (architecture config, BitNet version)
- Weight conversion between formats (distillation export)

**Key Entities:**
- `CraftsmanModel` — Root aggregate for the loaded model
- `GGUFModelFile` — Parsed GGUF container with tensor access
- `TensorMap` — Name → TernaryTensor/FP16Tensor mapping
- `ModelConfig` — Deserialized architecture configuration
- `MemoryMapper` — Memory-mapped tensor access for demand paging

**Invariants:**
- Model file must pass GGUF v3 magic/version validation
- All expected tensors must be present (fail-fast on missing layers)
- Ternary tensors must have correct block alignment (256 elements)
- FP16 tensors (router, embed, head) must not be loaded as ternary

**Interfaces:**
- **Inbound**: File path or HuggingFace model ID
- **Outbound**: Hydrated `CraftsmanModel` ready for inference
- **Downstream**: Provides tensors to Ternary Inference and MoE Routing contexts

```
┌─────────────────────────────────────────────┐
│        Model Lifecycle Context              │
│                                             │
│  ┌──────────────┐    ┌──────────────────┐   │
│  │ GGUFParser   │───▶│  TensorLoader    │   │
│  │ (validate)   │    │ (mmap + extract) │   │
│  └──────────────┘    └────────┬─────────┘   │
│                               │              │
│  ┌──────────────┐    ┌───────▼──────────┐   │
│  │ ModelConfig  │◀───│ CraftsmanModel   │   │
│  │ (metadata)   │    │ (root aggregate) │   │
│  └──────────────┘    └──────────────────┘   │
│                                             │
│  ┌──────────────────────────────────────┐   │
│  │        MemoryMapper                  │   │
│  │  (demand-page inactive experts)      │   │
│  └──────────────────────────────────────┘   │
└─────────────────────────────────────────────┘
```

---

### 3.4 Quantization Pipeline Context (Supporting)

**Responsibility**: Convert full-precision weights to ternary format. Supports two modes:
1. **Phase 0 (PTQ)**: Direct absmean ternary quantization with optional calibration — no training loop
2. **Phase 1+ (Distillation)**: Full training pipeline with STE, shadow weights, and RLM orchestration

**Delegates training orchestration to the RLM Training Orchestration Context** (3.8) for Phase 1+ distillation, which provides GRPO rewards, EWC++ stability, and quality tracking.

**Owns:**
- Absmean quantization implementation (shared by Phase 0 and Phase 1+)
- PT-BitNet quantizer for Phase 0 rapid prototype (no training loop)
- Straight-through estimator for backpropagation (Phase 1+ only)
- Shadow weight management (FP16 ↔ ternary, Phase 1+ only)
- Calibration pass for scale factor optimization (Phase 0)
- GGUF export with ternary tensor metadata (BITNET_T158 type)
- Calibration dataset management

**Delegates to RLM Training (3.8) — Phase 1+ only:**
- Distillation loss computation with GRPO reward scaling
- Cross-expert stability via EWC++ regularization
- Router validation via contrastive training
- Distillation quality tracking via MemoryDistiller
- Per-layer policy persistence via PolicyStore

**Key Entities:**
- `PtBitnetQuantizer` — Phase 0: direct FP16 → ternary conversion with calibration (NEW, ~200-300 lines)
- `AbsmeanQuantizer` — Converts FP16 block → ternary + scale (NEW, shared by Phase 0 and 1+)
- `CalibrationRunner` — Phase 0: runs calibration samples to optimize scale factors (NEW, ~100 lines)
- `BitLinearTrainer` — Phase 1+: BitLinear layer with shadow weights and STE (NEW)
- `TeacherModel` — FP16 GLM-4.7-Flash reference model (NEW)
- `CalibrationDataset` — Token sequences for quantization calibration (NEW)
- `GrpoOptimizer` — Per-expert reward scaling, Phase 1+ only (REUSED from `training/grpo.rs`)
- `EwcRegularizer` — Cross-expert forgetting prevention, Phase 1+ only (REUSED from `lora/training.rs`)

**Invariants:**
- Quantization is deterministic: same FP16 input → same ternary output
- Phase 0: No shadow weights — direct one-shot quantization
- Phase 1+: Shadow weights are FP16 throughout training (never accumulated in ternary)
- Phase 1+: Teacher model is frozen during distillation (no gradient updates)
- Phase 1+: Distillation loss = KD_base * GRPO_scale + EWC_penalty (see ADR-017 AD-11, AD-13)

**Interfaces:**
- **Inbound**: Teacher model weights (FP16/BF16) + calibration or training dataset
- **Outbound**: Ternary weights exported as GGUF with BITNET_T158 tensor type
- **Downstream**: Feeds Model Lifecycle Context with final artifacts

```
┌──────────────────────────────────────────────────────────┐
│           Quantization Pipeline Context                  │
│                                                          │
│  Phase 0 (PTQ):                                          │
│  ┌──────────────┐    ┌──────────────────┐                │
│  │ FP16 Weights │───▶│PtBitnetQuantizer │                │
│  │(GLM-4.7-Flash│    │(absmean + calib) │                │
│  └──────────────┘    └────────┬─────────┘                │
│                               │                          │
│  Phase 1+ (Distillation):    │                          │
│  ┌──────────────┐    ┌───────┼──────────┐                │
│  │TeacherModel  │───▶│DistillPipeline   │                │
│  │(GLM-4.7-Flash│    │(KD loss + STE)   │                │
│  └──────────────┘    └────────┬─────────┘                │
│                               │                          │
│  ┌──────────────┐    ┌───────▼──────────┐                │
│  │AbsmeanQuant  │◀───│BitLinearTrainer  │                │
│  │(FP16→ternary)│    │(shadow weights)  │                │
│  └──────┬───────┘    └──────────────────┘                │
│         │                                                │
│  ┌──────▼───────────────────────────────┐   Both paths:  │
│  │         GGUFExporter                 │◀──────────┘    │
│  │  (BITNET_T158 tensors + metadata)    │                │
│  └──────────────────────────────────────┘                │
└──────────────────────────────────────────────────────────┘
```

---

### 3.5 Kernel Dispatch Context (Supporting)

**Responsibility**: Detect hardware capabilities and select optimal ternary GEMM kernels.

**Owns:**
- CPU feature detection (AVX512, AVX2, NEON, SSE4.1, SVE)
- Cache hierarchy analysis (L1/L2/L3 sizes)
- Kernel selection heuristics
- Kernel code generation (optional, for runtime specialization)
- Benchmark-based kernel tuning

**Key Entities:**
- `HardwareCaps` — Detected CPU features and cache topology
- `KernelRegistry` — Available kernel implementations per platform
- `KernelSelector` — Decision logic for kernel choice
- `KernelConfig` — Tile sizes, unroll factors, prefetch distances

**Invariants:**
- Kernel selection happens once at model load time (not per-token)
- Selected kernel must be validated against reference implementation
- Fallback to scalar kernel must always exist
- Kernel config is immutable after selection

**Interfaces:**
- **Inbound**: System hardware information (CPUID, /proc/cpuinfo)
- **Outbound**: Configured kernel function pointers to Ternary Inference Context

```
┌─────────────────────────────────────────────┐
│        Kernel Dispatch Context              │
│                                             │
│  ┌──────────────┐    ┌──────────────────┐   │
│  │HardwareCaps  │───▶│ KernelSelector   │   │
│  │(CPUID/NEON)  │    │ (heuristics)     │   │
│  └──────────────┘    └────────┬─────────┘   │
│                               │              │
│  ┌──────────────┐    ┌───────▼──────────┐   │
│  │KernelRegistry│◀───│ KernelConfig     │   │
│  │(impl table)  │    │ (tile/unroll)    │   │
│  └──────────────┘    └──────────────────┘   │
└─────────────────────────────────────────────┘
```

---

### 3.6 Adaptation Layer Context (Supporting)

**Responsibility**: Apply SONA MicroLoRA corrections on top of ternary base weights.

**Owns:**
- MicroLoRA adapter creation and management
- FP16 delta computation (LoRA_B @ LoRA_A @ X)
- EWC++ Fisher information for catastrophic forgetting prevention
- Adapter composition (merging multiple adapters)
- Adapter hot-swap without model reload

**Key Entities:**
- `TernaryAdapter` — MicroLoRA adapter for a specific BitLinear layer
- `AdaptationManager` — Coordinates adapter lifecycle across layers
- `FisherDiagonal` — EWC++ regularization weights per adapter
- `AdaptFeedback` — Quality signal from inference results driving adaptation

**Invariants:**
- Adapters never modify base ternary weights (additive only)
- Adapter rank is 1-2 maximum (memory constraint: <1MB per module)
- EWC++ prevents adapter weights from drifting too far from initial values
- Hot-swap is atomic (no partially-loaded adapter state)

**Interfaces:**
- **Inbound**: Inference quality feedback (SONA instant loop)
- **Outbound**: FP16 corrections added to ternary GEMM output
- **Upstream**: Interacts with Ternary Inference Context at BitLinear output

```
┌─────────────────────────────────────────────┐
│        Adaptation Layer Context             │
│                                             │
│  ┌──────────────┐    ┌──────────────────┐   │
│  │AdaptManager  │───▶│TernaryAdapter    │   │
│  │(lifecycle)   │    │(MicroLoRA FP16)  │   │
│  └──────────────┘    └────────┬─────────┘   │
│                               │              │
│  ┌──────────────┐    ┌───────▼──────────┐   │
│  │FisherDiag   │◀───│ AdaptFeedback    │   │
│  │(EWC++ reg)   │    │ (quality signal) │   │
│  └──────────────┘    └──────────────────┘   │
└─────────────────────────────────────────────┘
```

---

### 3.7 Serving Integration Context (Generic)

**Responsibility**: Expose Craftsman Ultra as a standard RuvLLM backend.

**Owns:**
- `BitNetBackend` implementation of `InferenceBackend` trait
- Session management (multi-turn conversation state)
- KV cache allocation and management
- Token streaming and generation parameters
- NAPI bindings for Node.js access

**Key Entities:**
- `BitNetBackend` — Backend trait implementation
- `InferenceSession` — Per-conversation state including KV cache
- `GenerationConfig` — Temperature, top-k, top-p, repetition penalty
- `TokenStream` — Async iterator for streaming token output

**Invariants:**
- Backend must satisfy all `InferenceBackend` trait methods
- Sessions are isolated (no cross-session state leakage)
- KV cache eviction follows LRU policy when memory pressure detected
- Token generation is deterministic given same seed + config

**Interfaces:**
- **Inbound**: RuvLLM backend dispatcher, NAPI calls from Node.js
- **Outbound**: Generated tokens, embeddings, model metadata
- **Downstream**: Orchestrates all other contexts for end-to-end inference

---

### 3.8 RLM Training Orchestration Context (Supporting — Reused)

**Responsibility**: Orchestrate GRPO-guided distillation, contrastive router validation, EWC++ cross-expert stability, and distillation quality tracking using the existing RuvLLM RLM stack.

**This context is ~70% composed of existing production-tested code.** Only the `CraftsmanDistiller` orchestrator and `BitLinearTrainer` are net-new.

**Owns:**
- GRPO per-expert reward computation during distillation
- Contrastive router validation after ternary expert conversion
- EWC++ Fisher diagonal management across sequential expert phases
- Distillation trajectory recording in ReasoningBank
- Per-layer TernaryScale policy persistence in PolicyStore
- Expert-parallel distillation scheduling

**Key Entities (REUSED from existing crates):**

| Entity | Source File | Role in Craftsman Ultra |
|--------|-----------|------------------------|
| `GrpoOptimizer` | `training/grpo.rs` | Compute per-expert reward scaling during KD |
| `GrpoConfig` | `training/grpo.rs` | Configure adaptive KL, clip range, group size |
| `SampleGroup` | `training/grpo.rs` | Map one expert's teacher-vs-student outputs |
| `GrpoEvaluator` | `training/real_trainer.rs` | Score ternary student against FP16 teacher |
| `EwcRegularizer` | `lora/training.rs` | Prevent cross-expert weight interference |
| `TrainingPipeline` | `lora/training.rs` | LR scheduling, gradient accumulation |
| `ContrastiveTrainer` | `training/contrastive.rs` | Validate MoE routing post-ternary conversion |
| `TrainingTriplet` | `training/contrastive.rs` | Expert routing triplets (anchor/pos/neg) |
| `MemoryDistiller` | `reasoning_bank/distillation.rs` | Extract KeyLessons from distillation runs |
| `KeyLesson` | `reasoning_bank/distillation.rs` | Persist distillation insights |
| `PolicyStore` | `policy_store.rs` | Persist TernaryScale policies per layer |
| `RealTrainingConfig` | `training/real_trainer.rs` | Training hyperparameters + GGUF export config |

**Key Entities (NEW):**

| Entity | Role |
|--------|------|
| `CraftsmanDistiller` | Top-level orchestrator wiring GRPO + EWC + Contrastive + KD |
| `BitLinearTrainer` | BitLinear layer with shadow weights + straight-through estimator |
| `ExpertTripletGenerator` | Produces contrastive triplets from MoE routing decisions |
| `DistillationTrajectoryRecorder` | Adapts training steps to ReasoningBank `Trajectory` format |
| `TernaryScalePolicy` | Per-layer ternary metadata for PolicyStore |
| `SequentialExpertDistiller` | EWC-regularized sequential expert distillation loop |

**Invariants:**
- GRPO reward never overrides KD loss — it scales the loss multiplicatively (1 + reward * 0.1)
- EWC Fisher diagonals are accumulated, not replaced, across expert phases
- Contrastive router validation runs after each expert batch, not after each step
- PolicyStore entries are immutable once written (append-only per distillation run)
- Teacher model weights are frozen throughout (no gradient updates to teacher)

**Interfaces:**
- **Inbound**: Teacher model (GLM-4.7-Flash), training dataset, target architecture config
- **Outbound**: Trained ternary GGUF weights, TernaryScale policies, KeyLessons
- **Upstream**: Consumes from Quantization Pipeline (BitLinear training) and feeds Model Lifecycle (GGUF export)

```
┌──────────────────────────────────────────────────────────────┐
│          RLM Training Orchestration Context                  │
│                                                              │
│  ┌───────────────────────────────────────────────────┐       │
│  │         CraftsmanDistiller (NEW orchestrator)     │       │
│  └───────────┬────────────┬──────────────┬───────────┘       │
│              │            │              │                    │
│    ┌─────────▼───┐  ┌────▼────────┐  ┌──▼─────────────┐     │
│    │GrpoOptimizer│  │EwcRegularizer│  │ContrastiveTrainer│    │
│    │(REUSED)     │  │(REUSED)     │  │(REUSED)        │     │
│    │Per-expert   │  │Cross-expert │  │Router          │     │
│    │rewards      │  │stability    │  │validation      │     │
│    └──────┬──────┘  └──────┬──────┘  └────────┬───────┘     │
│           │                │                   │             │
│    ┌──────▼────────────────▼───────────────────▼──────┐      │
│    │         BitLinearTrainer (NEW)                    │      │
│    │    Shadow weights + STE + KD loss + GRPO scale   │      │
│    └──────────────────────┬───────────────────────────┘      │
│                           │                                  │
│    ┌──────────────┐  ┌────▼──────────┐  ┌──────────────┐    │
│    │MemoryDistiller│  │ PolicyStore  │  │ GGUFExporter │    │
│    │(REUSED)      │  │(REUSED)      │  │(REUSED)      │    │
│    │KeyLessons    │  │TernaryScale  │  │Ternary GGUF  │    │
│    └──────────────┘  └──────────────┘  └──────────────┘    │
│                                                              │
│  Legend:  (REUSED) = existing production code, no changes    │
│           (NEW)    = net-new code for Craftsman Ultra        │
└──────────────────────────────────────────────────────────────┘
```

**Reuse ratio**: ~70% existing / ~30% new (Phase 1+ distillation)

### 3.8.1 Phase 0.5: RLM Post-Quantization Refinement Mode

The RLM Training Orchestration Context operates in a **lightweight refinement mode** during Phase 0.5, where ternary weights are frozen and only FP16 components are trained. This requires zero new training code — all components are wired directly from existing production code.

**Phase 0.5 operational differences from Phase 1+:**

| Aspect | Phase 1+ (Distillation) | Phase 0.5 (RLM Refinement) |
|--------|------------------------|---------------------------|
| Ternary weights | Trained (shadow + STE) | **Frozen** |
| Trainable params | ~28B | ~200-400M (1-2%) |
| Training tokens | 200B | 100-500M (400x less) |
| `BitLinearTrainer` | Yes (NEW code) | **Not needed** |
| `MicroLoRA` | Post-training LoRA | **Training-time LoRA corrections** |
| `ContrastiveTrainer` | Router validation | **Router repair** |
| `GrpoOptimizer` | Per-expert distillation reward | **Scale factor optimization reward** |
| `EwcRegularizer` | Cross-expert stability | **Cross-step stability** |
| Platform | Cloud GPU (4× A100) | **Mac Studio (Metal or SIMD-only)** |
| Cost | $1,300+ | **$0** |
| New code | ~30% new | **~0% new** (only thin orchestrator) |

**Key entities in Phase 0.5 mode:**
- `RlmRefiner` — Thin orchestrator (~200-300 lines) that wires existing RLM components for post-quantization refinement (NEW)
- `MicroLoRA` — Rank 1-2 FP16 adapters per expert FFN (REUSED from `lora/micro_lora.rs`)
- `TrainingPipeline` — Single-example + batch gradient training with EWC++ (REUSED from `lora/training.rs`)
- `ContrastiveTrainer` — Triplet + InfoNCE for router repair (REUSED from `training/contrastive.rs`)
- `GrpoOptimizer` — Quality reward signal for scale optimization (REUSED from `training/grpo.rs`)
- `EwcRegularizer` — Prevents regression during multi-step refinement (REUSED from `lora/training.rs`)
- `MemoryDistiller` — Tracks which experts benefit most from LoRA corrections (REUSED)
- `PolicyStore` — Persists optimized scale factors and LoRA configs (REUSED)

**Reuse ratio (Phase 0.5)**: **100% existing / 0% new training code** (only a thin orchestrator wrapper)

---

## 4. Aggregates and Entities

### 4.1 CraftsmanModel (Root Aggregate)

The `CraftsmanModel` is the root aggregate that owns the entire loaded model state.

```
CraftsmanModel
├── config: ModelConfig
│   ├── num_layers: u32 (transformer depth)
│   ├── hidden_size: u32
│   ├── num_experts: u32 (total experts per MoE layer)
│   ├── active_experts: u32 (K experts selected per token)
│   ├── num_attention_heads: u32
│   ├── num_kv_heads: u32
│   ├── vocab_size: u32
│   ├── max_context: u32 (200K)
│   ├── rope_theta: f32
│   └── bitnet_version: u8 (1 = b1.58)
│
├── embedding: EmbeddingTable (FP16)
│   ├── weights: Tensor<f16> [vocab_size × hidden_size]
│   └── position_encoding: RoPEConfig
│
├── layers: Vec<TransformerLayer>
│   └── TransformerLayer
│       ├── attention: AttentionBlock
│       │   ├── q_proj: BitLinearLayer | FP16Linear (phase-dependent)
│       │   ├── k_proj: BitLinearLayer | FP16Linear
│       │   ├── v_proj: BitLinearLayer | FP16Linear
│       │   ├── o_proj: BitLinearLayer | FP16Linear
│       │   └── norm: RMSNorm (FP16 params)
│       │
│       ├── moe: MoEBlock
│       │   ├── router: MoERouter
│       │   │   ├── gate: FP16Linear [hidden_size × num_experts]
│       │   │   └── top_k: u32
│       │   ├── experts: Vec<Expert>
│       │   │   └── Expert
│       │   │       ├── gate_proj: BitLinearLayer
│       │   │       ├── up_proj: BitLinearLayer
│       │   │       └── down_proj: BitLinearLayer
│       │   └── norm: RMSNorm (FP16 params)
│       │
│       └── adapter: Option<TernaryAdapter> (SONA MicroLoRA)
│
├── lm_head: FP16Linear [hidden_size × vocab_size]
│
├── kernel: SelectedKernel
│   ├── variant: KernelType (TL1/TL2/I2_S)
│   ├── lookup_tables: Vec<LookupTable>
│   └── config: KernelConfig
│
└── memory_map: Option<MemoryMapper>
    └── file_handle: MmapFile
```

### 4.2 BitLinearLayer (Entity)

Core compute entity representing a single ternary linear layer.

```
BitLinearLayer
├── ternary_weights: TernaryTensor
│   ├── packed_data: Vec<u8>  (2 bits per weight, packed)
│   ├── scales: Vec<f16>      (one per 256-element block)
│   ├── shape: [out_features, in_features]
│   └── num_blocks: u32
│
├── kernel_fn: fn(&TernaryTensor, &[i8]) -> Vec<f16>
│   └── (function pointer to selected SIMD kernel)
│
└── stats: LayerStats
    ├── sparsity: f32          (fraction of zero weights)
    ├── mean_abs_scale: f32    (average block scale)
    └── compute_flops: u64     (additions per forward)
```

**Forward pass pseudocode:**
```
fn forward(input: &[f16]) -> Vec<f16> {
    let x_int8 = absmax_quantize(input);       // FP16 → INT8
    let y_int = (self.kernel_fn)(&self.ternary_weights, &x_int8);  // Ternary GEMM (addition only)
    let y_fp16 = dequantize_with_scales(y_int, &self.scales);      // INT → FP16
    y_fp16
}
```

### 4.3 TernaryTensor (Value Object)

Immutable packed ternary weight storage.

```
TernaryTensor
├── encoding: TernaryEncoding
│   ├── I2S  — 2 bits per weight: 00=0, 01=+1, 10=-1, 11=reserved
│   ├── TL1  — 4 bits per 2 weights (lookup index)
│   └── TL2  — 5 bits per 3 weights (lookup index)
│
├── packed_bytes: &[u8]  (immutable, potentially memory-mapped)
├── scales: &[f16]       (per-block absmean values)
├── shape: (usize, usize)
├── block_size: usize    (256 default)
└── total_weights: u64
```

**Storage calculation:**
- I2_S: `ceil(total_weights / 4)` bytes for weights + `ceil(total_weights / 256) * 2` bytes for scales
- TL1: `ceil(total_weights / 2) * 0.5` bytes + scales
- TL2: `ceil(total_weights / 3) * 0.625` bytes + scales

### 4.4 MoERouter (Entity)

Expert selection mechanism. Always FP16.

```
MoERouter
├── gate_weights: Tensor<f16> [hidden_size × num_experts]
├── gate_bias: Option<Tensor<f16>> [num_experts]
├── top_k: u32
├── capacity_factor: f32
├── balance_loss_weight: f32
│
└── fn route(hidden: &[f16]) -> RoutingDecision
    RoutingDecision
    ├── selected_experts: Vec<(usize, f32)>  // (expert_idx, weight)
    ├── expert_mask: BitVec                   // which experts are active
    └── balance_loss: f32                     // for training feedback
```

### 4.5 LookupTable (Value Object)

Pre-computed activation sums for TL1/TL2 kernels.

```
LookupTable
├── variant: LutVariant
│   ├── TL1 — 16 entries per table (2^4 for 2-weight combinations)
│   └── TL2 — 32 entries per table (2^5 for 3-weight combinations)
│
├── tables: Vec<Vec<i16>>  (one table per activation group)
├── num_tables: usize
└── activation_group_size: usize
```

**Generation (TL1 example):**
For each pair of ternary weights (w0, w1) and each possible pair of INT8 activations (a0, a1):
```
table[index(w0, w1)] = w0*a0 + w1*a1
```
Since w ∈ {-1, 0, +1}, this becomes addition/subtraction only.

---

## 5. Context Map (Inter-Context Relationships)

```
┌──────────────────────────────────────────────────────────────┐
│                                                              │
│    ┌──────────────┐         ┌──────────────────┐             │
│    │   Kernel     │────────▶│    Ternary       │             │
│    │   Dispatch   │ kernel  │    Inference      │             │
│    │   Context    │ config  │    Engine         │             │
│    └──────────────┘         └────────┬─────────┘             │
│                                      │                        │
│    ┌──────────────┐         ┌───────▼──────────┐             │
│    │  Model       │────────▶│     MoE          │             │
│    │  Lifecycle   │ tensors │     Routing       │             │
│    │  Context     │         │     Context       │             │
│    └──────┬───────┘         └────────┬─────────┘             │
│           │                          │                        │
│    ┌──────▼───────┐         ┌───────▼──────────┐             │
│    │Quantization  │         │   Adaptation     │             │
│    │  Pipeline    │────────▶│     Layer         │             │
│    │  Context     │ weights │   (SONA)          │             │
│    └──────┬───────┘         └────────┬─────────┘             │
│           │                          │                        │
│    ┌──────▼───────────────┐ ┌───────▼──────────┐             │
│    │  RLM Training        │ │    Serving        │             │
│    │  Orchestration       │ │    Integration    │             │
│    │  Context             │ │    Context        │             │
│    │                      │ └──────────────────┘             │
│    │ ┌──────────────────┐ │                                  │
│    │ │ GRPO    EWC++    │ │  ─── Reuse Boundary ───          │
│    │ │ Contrastive      │ │  Components above the line       │
│    │ │ MemoryDistiller  │ │  are ~70% REUSED from existing   │
│    │ │ PolicyStore      │ │  RuvLLM RLM training stack       │
│    │ └──────────────────┘ │                                  │
│    └──────────────────────┘                                  │
│                                                              │
│  ──── Relationship Types ────                                │
│  ────▶  Conformist (downstream conforms to upstream)         │
│  ─ ─ ▶  Anti-Corruption Layer (translates at boundary)       │
│  ══════  Shared Kernel (common types/interfaces)             │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### Relationship Details

| Upstream | Downstream | Type | Interface |
|----------|-----------|------|-----------|
| Kernel Dispatch | Ternary Inference | Conformist | `KernelConfig` + function pointers |
| Model Lifecycle | Ternary Inference | Conformist | `TernaryTensor`, `FP16Tensor` |
| Model Lifecycle | MoE Routing | Conformist | `MoERouter` weights, `ExpertPool` |
| Ternary Inference | MoE Routing | Shared Kernel | `BitLinearLayer` entity shared |
| MoE Routing | Serving Integration | Conformist | Forward pass API |
| Adaptation Layer | Ternary Inference | ACL | FP16 deltas translated to output corrections |
| Quantization Pipeline | Model Lifecycle | Conformist | GGUF export format |
| **RLM Training** | **Quantization Pipeline** | **Shared Kernel** | **`BitLinearTrainer` drives `AbsmeanQuantizer`** |
| **RLM Training** | **MoE Routing** | **ACL** | **`ContrastiveTrainer` validates router post-ternary** |
| **RLM Training** | **Model Lifecycle** | **Conformist** | **GGUF export via `GgufExportResult`** |
| **RLM Training** | **Adaptation Layer** | **Shared Kernel** | **`EwcRegularizer` shared for training + inference** |

### External System Integrations

| External System | Integration Point | Pattern |
|----------------|-------------------|---------|
| RuvLLM Backends | Serving Integration | `InferenceBackend` trait (published language) |
| SONA Learning Loops | Adaptation Layer | Event-driven (quality feedback signals) |
| Ruvector HNSW | Serving Integration, RLM Training | Pattern retrieval for routing optimization + policy search |
| HuggingFace Hub | Model Lifecycle | Model download/upload API |
| Claude Flow | Serving Integration | Agent routing task delegation |
| NAPI/Node.js | Serving Integration | FFI boundary (NAPI-RS bindings) |
| **ReasoningBank** | **RLM Training** | **`Trajectory` recording + `KeyLesson` extraction** |
| **PolicyStore** | **RLM Training** | **`TernaryScalePolicy` persistence + semantic retrieval** |

---

## 6. Domain Events

Events drive communication between bounded contexts without tight coupling.

| Event | Producer | Consumers | Payload |
|-------|----------|-----------|---------|
| `ModelLoaded` | Model Lifecycle | Kernel Dispatch, Serving | model_id, config, tensor_count |
| `KernelSelected` | Kernel Dispatch | Ternary Inference | kernel_type, config, lut_size |
| `ExpertRouted` | MoE Routing | Ternary Inference | token_id, expert_ids[], weights[] |
| `InferenceCompleted` | Serving Integration | Adaptation Layer | session_id, quality_score, latency_ms |
| `AdapterUpdated` | Adaptation Layer | Ternary Inference | layer_id, adapter_version |
| `DistillationCheckpoint` | Quantization Pipeline | Model Lifecycle | epoch, loss, checkpoint_path |
| `MemoryPressure` | Serving Integration | MoE Routing, Model Lifecycle | available_mb, action (evict/compact) |
| `ExpertDistilled` | RLM Training | Model Lifecycle, PolicyStore | expert_idx, final_loss, fisher_diag, ternary_scale_stats |
| `GrpoRewardComputed` | RLM Training | MemoryDistiller | sample_group_id, mean_reward, kl_divergence |
| `RouterValidated` | RLM Training | MoE Routing | routing_accuracy, misrouted_expert_pairs[], triplet_loss |
| `EwcFisherUpdated` | RLM Training | Adaptation Layer | expert_idx, fisher_top_k_indices, fisher_magnitude |
| `KeyLessonExtracted` | RLM Training | PolicyStore | lesson_content, embedding, source_expert, quality_score |
| `TernaryPolicyStored` | RLM Training | PolicyStore | layer_idx, module, mean_scale, sparsity, quality |
| `DistillationPhaseComplete` | RLM Training | Model Lifecycle | phase (1/2/3), experts_distilled, total_loss, elapsed_hours |

---

## 7. Module Structure (Proposed Crate Layout)

```
crates/ruvllm/src/
├── bitnet/                          # NEW: Ternary Inference Context
│   ├── mod.rs                       # Module exports
│   ├── bit_linear.rs                # BitLinearLayer implementation
│   ├── ternary_tensor.rs            # TernaryTensor value object
│   ├── quantizer.rs                 # Absmean + absmax quantization
│   ├── kernels/                     # Platform-specific GEMM kernels
│   │   ├── mod.rs
│   │   ├── tl1_avx2.rs             # TL1 kernel for x86 AVX2
│   │   ├── tl1_avx512.rs           # TL1 kernel for x86 AVX512
│   │   ├── tl1_neon.rs             # TL1 kernel for ARM NEON
│   │   ├── tl2_neon.rs             # TL2 kernel for memory-constrained ARM
│   │   ├── i2s_avx512.rs           # I2_S kernel for high-bandwidth x86
│   │   ├── i2s_scalar.rs           # Scalar fallback
│   │   └── lookup_table.rs         # LUT generation for TL1/TL2
│   └── tests/
│       ├── kernel_correctness.rs    # Bit-exact validation vs reference
│       ├── gemm_benchmark.rs        # Performance regression tests
│       └── quantizer_roundtrip.rs   # FP16 → ternary → verify
│
├── moe/                             # NEW: MoE Routing Context
│   ├── mod.rs
│   ├── router.rs                    # MoERouter gating network
│   ├── expert_pool.rs               # Expert registry and dispatch
│   ├── load_balancer.rs             # Capacity factor enforcement
│   └── tests/
│       └── routing_tests.rs
│
├── craftsman/                       # NEW: Craftsman Ultra integration
│   ├── mod.rs
│   ├── model.rs                     # CraftsmanModel root aggregate
│   ├── config.rs                    # ModelConfig deserialization
│   ├── forward.rs                   # End-to-end forward pass pipeline
│   └── tests/
│       └── integration_tests.rs
│
├── backends/
│   ├── bitnet_backend.rs            # NEW: BitNetBackend implementation
│   └── ... (existing backends)
│
├── distillation/                    # NEW: Quantization Pipeline Context
│   ├── mod.rs
│   ├── pipeline.rs                  # CraftsmanDistiller orchestrator (NEW)
│   ├── teacher.rs                   # TeacherModel wrapper (NEW)
│   ├── bit_linear_trainer.rs        # Shadow weights + STE (NEW)
│   ├── expert_triplet_gen.rs        # Expert routing triplets (NEW)
│   ├── trajectory_recorder.rs       # ReasoningBank adapter (NEW)
│   ├── sequential_expert.rs         # EWC-regularized sequential loop (NEW)
│   └── gguf_export.rs              # GGUF ternary export (extends REUSED GgufExportResult)
│
├── training/                        # EXISTING: RLM Training Stack (REUSED)
│   ├── grpo.rs                      # REUSED: GrpoOptimizer, SampleGroup, GrpoConfig
│   ├── contrastive.rs               # REUSED: ContrastiveTrainer, TrainingTriplet
│   ├── real_trainer.rs              # REUSED: RealContrastiveTrainer, GrpoEvaluator
│   ├── claude_dataset.rs            # REUSED: DatasetConfig, DatasetGenerator
│   └── mod.rs                       # REUSED: module exports
│
├── lora/
│   ├── training.rs                  # REUSED: EwcRegularizer, TrainingPipeline, LR schedules
│   └── micro_lora.rs               # REUSED: MicroLoRA, AdaptFeedback
│
├── reasoning_bank/
│   ├── distillation.rs              # REUSED: MemoryDistiller, KeyLesson, CompressedTrajectory
│   └── ...
│
├── policy_store.rs                  # REUSED: PolicyStore + NEW PolicyType::TernaryScale
│
├── gguf/
│   ├── quantization.rs              # EXISTING: Add BITNET_T158 type
│   └── ... (existing files)
│
├── autodetect.rs                    # EXISTING: Add ternary kernel detection
├── kernels/                         # EXISTING: Add bitnet kernel dispatch
└── ...
```

---

## 8. Performance Model

### Compute Analysis (Per Token, Phase 1)

Assuming GLM-4.7-Flash architecture with ~3B active parameters per token:

| Component | Precision | Operations | Estimated Latency |
|-----------|-----------|-----------|-------------------|
| Embedding lookup | FP16 | 1 lookup | <0.01 ms |
| Attention Q/K/V/O (FP16) | FP16 | ~1.2B FP multiply-add | ~30 ms (CPU) |
| RMSNorm (per layer) | FP16 | Negligible | <0.1 ms |
| MoE Router (per layer) | FP16 | ~1M FP multiply-add | <0.5 ms |
| Expert FFN (ternary) | INT8/ternary | ~1.8B INT additions | ~15 ms (TL1 AVX2) |
| LM Head | FP16 | ~vocab_size FP multiply-add | ~2 ms |
| **Total per token** | — | — | **~50 ms → ~20 tok/s** |

### Phase 2 (Full Ternary) Projection

| Component | Precision | Estimated Latency |
|-----------|-----------|-------------------|
| Attention (ternary) | INT8/ternary | ~12 ms |
| Expert FFN (ternary) | INT8/ternary | ~15 ms |
| Router + norms | FP16 | ~1 ms |
| **Total per token** | — | **~30 ms → ~33 tok/s** |

### Memory Budget

| Component | Phase 1 | Phase 2 |
|-----------|---------|---------|
| Expert weights (ternary) | 5.5 GB | 5.5 GB |
| Attention weights | 2.0 GB (FP16) | 0.7 GB (ternary) |
| Shared (embed/head/router/norm) | 1.5 GB | 1.5 GB |
| Lookup tables | 0.2 GB | 0.3 GB |
| KV cache (4K context) | 1.5 GB | 1.5 GB |
| **Total** | **~10.7 GB** | **~9.5 GB** |

---

## 8.5 Training Infrastructure Model

### Why Not Local CPU/SIMD (for Phase 1+)

The existing RuvLLM SIMD kernels (`crates/ruvllm/src/kernels/`) are **inference-only** — no backward pass, no gradient computation, no training support. The training code paths are:

- `RealContrastiveTrainer`: Candle tensors on `Device::Metal` or `Device::Cpu` (no CUDA)
- `EwcRegularizer` / LoRA training: Pure CPU via `ndarray` (no GPU acceleration)
- SIMD kernels: Forward-pass optimizations only (flash attention, matmul, activations)

At ~50-100 training tok/s on CPU, 200B tokens would require ~65 years. Not viable for Phase 1+.

### Why SIMD-Only Works (for Phase 0.5)

Phase 0.5 is fundamentally different from Phase 1+: it trains only ~200-400M FP16 parameters (1-2% of 30B) using existing RLM components that are already pure ndarray/CPU. The SIMD kernels are used for the forward pass through the frozen model to compute training loss, not for gradient computation.

**GPU dependency analysis of Phase 0.5 components:**

| Component | GPU Required? | SIMD Benefit |
|-----------|--------------|-------------|
| MicroLoRA forward pass | No — `forward_simd()` uses NEON intrinsics directly | ~3-4x over scalar |
| MicroLoRA gradient computation | No — pure ndarray `apply_gradients()` | None (ndarray handles) |
| TrainingPipeline | No — pure ndarray | None |
| EwcRegularizer | No — pure ndarray | None |
| GrpoOptimizer | No — pure ndarray | None |
| ContrastiveTrainer | Optional — `use_metal: false` forces CPU | Candle CPU tensors |
| Frozen model forward (loss computation) | No — SIMD inference kernels | NEON GEMM/GEMV ~3x |

**Effective training throughput (SIMD-only, 100M-500M tokens):**

| Platform | SIMD | tok/s | 100M tokens | Feasible? |
|----------|------|-------|-------------|-----------|
| Mac Studio M4 Max | NEON | ~100-300 | 4-12 days | **Yes** |
| Mac Studio M3 Ultra | NEON | ~150-400 | 3-8 days | **Yes** |
| Linux ARM64 (Graviton3) | NEON | ~80-200 | 6-14 days | **Yes** |
| Linux x86 (Ryzen 9) | Scalar | ~30-80 | 14-39 days | **Marginal** |

**Platform gap**: No AVX2/AVX512 SIMD kernels exist in `kernels/matmul.rs` — only `target_arch = "aarch64"` (NEON) vs scalar dispatch. x86 therefore falls to scalar, making it ~3-5x slower than NEON. Adding AVX2 kernels is an identified future improvement (see ADR-017 AD-20).

### Cloud GPU Distillation Strategy

**Per-expert distillation fits in a single A100 80GB:**

```
Expert FFN (~1B params):
  Shadow weights (FP16):    2 GB
  Gradients (FP32):         4 GB
  AdamW state (2×FP32):     8 GB
  Teacher activations:      1 GB
  EWC++ Fisher:             0.5 GB
  ────────────────────────────────
  Total per expert:         ~15.5 GB  ✓ Fits A100 40GB
```

**Expert-parallel: 4 experts distill concurrently on 4× A100/H100:**

```
┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   GPU 0      │  │   GPU 1      │  │   GPU 2      │  │   GPU 3      │
│  Expert 0    │  │  Expert 1    │  │  Expert 2    │  │  Expert 3    │
│  BitLinear   │  │  BitLinear   │  │  BitLinear   │  │  BitLinear   │
│  + EWC       │  │  + EWC       │  │  + EWC       │  │  + EWC       │
│  + GRPO      │  │  + GRPO      │  │  + GRPO      │  │  + GRPO      │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │                 │
       └─────────────────┴─────────────────┴─────────────────┘
                                 │
                    ┌────────────▼───────────┐
                    │   Fisher Accumulation  │
                    │   (cross-expert EWC)   │
                    └────────────────────────┘
```

### What Runs Where

| Task | Location | Device | Duration |
|------|----------|--------|----------|
| **Phase 0.5 RLM refinement (Metal)** | **Mac Studio** | **Metal GPU + CPU ndarray** | **3-14 days** |
| **Phase 0.5 RLM refinement (SIMD-only)** | **Mac Studio or Linux ARM64** | **NEON SIMD + CPU ndarray** | **4-24 days** |
| Expert distillation (Phase 1) | GCP 4×A100 spot | CUDA | ~46 days |
| Router contrastive validation | GCP 1×A100 or local Mac | CUDA/Metal/CPU | Hours |
| Inference benchmark (TL1/TL2) | Local workstation | CPU SIMD (AVX2/NEON) | Minutes |
| MicroLoRA adaptation | Local / edge | CPU (ndarray + NEON SIMD) | <1ms/update |
| GGUF export | Local | CPU | Minutes |
| Kernel correctness tests | Local | CPU SIMD | Seconds |

### Required Code Change

Add CUDA device dispatch to `RealContrastiveTrainer` (`training/real_trainer.rs:178-184`):
- New config field: `use_cuda: bool`, `cuda_device_id: usize`
- Device selection: CUDA → Metal → CPU fallback chain
- Existing `candle` + `cuda` Cargo features already available in `Cargo.toml`

---

## 9. Testing Strategy

### Unit Tests (Per Context)

| Context | Test Focus | Examples |
|---------|-----------|---------|
| Ternary Inference | Kernel correctness | Bit-exact GEMM vs reference float impl |
| Ternary Inference | Quantizer roundtrip | FP16 → ternary → verify scale preservation |
| MoE Routing | Router selection | Top-K selection, capacity enforcement |
| MoE Routing | Load balancing | No expert starvation under varied inputs |
| Model Lifecycle | GGUF parsing | Valid/invalid/corrupt file handling |
| Kernel Dispatch | Hardware detection | Mock CPUID, verify kernel selection |
| Adaptation Layer | LoRA correctness | Adapter output matches FP16 reference |

### Integration Tests

| Test | Contexts Involved | Validation |
|------|------------------|-----------|
| End-to-end generation | All | Generate coherent text from prompt |
| Mixed-precision forward | Ternary + MoE + Serving | Output matches reference within tolerance |
| Model load + inference | Lifecycle + Inference | Cold-start to first token <5s |
| Adapter hot-swap | Adaptation + Inference | Zero downtime, correct output switch |
| GRPO reward convergence | RLM Training + Quant Pipeline | Mean reward > 0.8 after 1000 steps per expert |
| EWC cross-expert stability | RLM Training | Expert N+1 distillation doesn't increase expert N loss by > 5% |
| Contrastive router validation | RLM Training + MoE Routing | Router accuracy >= 95% post-ternary conversion |
| PolicyStore roundtrip | RLM Training + Model Lifecycle | TernaryScale policies stored and retrievable via semantic search |
| KeyLesson extraction | RLM Training | >= 5 meaningful lessons extracted per distillation phase |
| Full distillation pipeline | RLM Training + Quant + Lifecycle | End-to-end: teacher weights → ternary GGUF with policies |

### Benchmark Tests

| Benchmark | Target | Pass Criteria |
|-----------|--------|--------------|
| HumanEval pass@1 | >=50% (Phase 1), >=58% (Phase 2) | >= threshold |
| MBPP pass@1 | >=55% | >= threshold |
| Decode tok/s (AVX2) | >=10 (Phase 1), >=20 (Phase 2) | >= threshold |
| Memory peak (4K ctx) | <=12 GB (Phase 1), <=10 GB (Phase 2) | <= threshold |
| Kernel GEMM (1024x1024) | <=2ms (TL1 AVX2) | <= threshold |

---

## 10. Migration Path from Existing RuvLLM

### Compatibility Matrix

| Existing Feature | Impact | Phase 0 | Phase 1+ |
|-----------------|--------|---------|----------|
| GGUF parser | Low | Add BITNET_T158 type to `GgufQuantType` enum | Same |
| `dequantize_tensor` | **Medium** | **Implement IQ1_S/BITNET_T158 dequant** (currently returns error at line 358) | Same |
| `InferenceBackend` trait | None | New `BitNetBackend` implements existing trait | Same |
| KV cache (`kv_cache.rs`) | None | Reused as-is | Reused as-is |
| Autodetect (`autodetect.rs`) | Low | Add ternary kernel capability flags | Same |
| SIMD kernels (`kernels/`) | **Medium** | TL1 kernel minimum viable for validation | Full TL1/TL2/I2_S suite |
| MicroLoRA (`lora/`) | None (Phase 0) | Not needed for PTQ | Adapter applied to BitLinear output |
| SONA (`sona/`) | None | Not needed for PTQ | Instant loop drives adapter feedback |
| Claude Flow (`claude_flow/`) | Low | Add `BitNetModel` to model router | Same |
| NAPI bindings | Low | Expose `BitNetBackend` via existing pattern | Same |
| tokenizer | None | Reused (GLM-4 tokenizer, 151K vocab) | Same |

### Non-Breaking Changes

All changes are additive. No existing backend, model, or API is modified. The `BitNetBackend` is a new backend option that coexists with Candle, mistral-rs, and CoreML.

---

## 11. Open Questions

| # | Question | Impact | Status | Notes |
|---|----------|--------|--------|-------|
| 1 | Exact expert count in GLM-4.7-Flash? | Architecture config | Open | Need to inspect `config.json` from HF or wait for technical report |
| 2 | MLA (Multi-head Latent Attention) compatibility with ternary? | Phase 2 design | Open | MLA's compressed KV may conflict with ternary attention |
| 3 | GLM-4.7-Flash tokenizer reuse or custom? | Model Lifecycle | Open | Likely reuse GLM-4 tokenizer (151K vocab) |
| 4 | Distillation compute budget? | Phase 1 timeline | **Reduced** | RLM reuse reduces framework dev cost; compute still 800-1600 A100-hours but engineering effort ~70% less |
| 5 | WASM target for ternary kernels? | Portability | **Resolved (AD-21)** | Yes — WASM SIMD128 viable. TL1 LUT maps to v128.swizzle; R3-Engine proves dual-target Rust→WASM. ~20-40 tok/s browser. |
| 6 | HuggingFace model name reservation? | Distribution | Open | Reserve `ruv/craftsman-ultra-30b-1bit` |
| 7 | BitNet patent/license status? | Legal | Open | MIT license for bitnet.cpp; research papers are open |
| 8 | Multi-Token Prediction (MTP) compat? | Speculative decoding | Open | GLM-4.7-Flash uses MTP; unclear if ternary draft model works |
| 9 | EWC++ Fisher OOM at 30B scale? | RLM Training | Open | May need sparse Fisher (top-k diagonal entries per expert) |
| 10 | GRPO group_size = num_experts or per-layer? | RLM Training | Open | Per-layer groups provide finer reward signal but more compute |
| 11 | Expert-parallel distillation rayon thread count? | RLM Training | Open | Balance CPU cores between rayon parallelism and ternary GEMM |
| 12 | Phase 0 PTQ calibration corpus choice? | Phase 0 quality | Open | WikiText-2 vs code-specific corpus (e.g., The Stack) — code corpus may preserve coding ability better |
| 13 | IQ1_S vs BITNET_T158 GGUF type for Phase 0? | GGUF compatibility | Open | IQ1_S (type 19) exists but block format may differ from absmean; custom BITNET_T158 avoids confusion but breaks llama.cpp compat |
| 14 | Phase 0 → Phase 1 weight migration path? | Efficiency | Open | Can Phase 0 PTQ weights serve as initialization for Phase 1 distillation shadow weights? |
| 15 | Optimal MicroLoRA rank for Phase 0.5? | Quality vs speed | Open | Rank-1 is faster, rank-2 is 5% faster due to SIMD but has 2× params. Empirical testing needed. |
| 16 | LoRA adapter persistence in GGUF? | Export format | Open | Store LoRA A/B matrices as separate tensors in GGUF, or merge into ternary+FP16 hybrid format? |
| 17 | Phase 0.5 LoRA → Phase 1 distillation init? | Continuity | Open | Can Phase 0.5 LoRA corrections inform Phase 1 shadow weight initialization for faster convergence? |
| 18 | Add AVX2/AVX512 SIMD kernels to `matmul.rs`? | x86 SIMD-only performance | Open | Current kernels only have NEON (aarch64) + scalar fallback. Adding AVX2 would make x86 SIMD-only Phase 0.5 ~3-5x faster. Is it worth the effort vs just using ARM? |
| 19 | SIMD-only vs Metal quality equivalence? | Phase 0.5 validation | Open | Does ContrastiveTrainer produce identical router accuracy on CPU vs Metal? Need empirical comparison to confirm no numerical divergence. |
| 20 | Cloud ARM64 instances for SIMD-only Phase 0.5? | Platform portability | Open | AWS Graviton3/4 or Ampere Altra instances with 128+ GB RAM could run SIMD-only Phase 0.5 without Mac Studio. Cost-competitive? |
| 21 | R3-Engine license compatibility? | Legal | Open | R3-Engine has no explicit license in README. Need to verify before referencing their bit-slicing approach in production code. bitnet.rs is Apache 2.0 (clear). |
| 22 | WASM model size for browser deployment? | Feasibility | Open | 30B model is ~5.5GB ternary — too large for most browsers. Need streaming/chunked loading or deploy 2B-4T model for browser demo. |
| 23 | SharedArrayBuffer for WASM multi-threading? | Performance | Open | WASM SIMD128 is single-threaded without SharedArrayBuffer + Web Workers. COOP/COEP headers required. Deployment complexity vs throughput gain? |
| 24 | Auto-label threshold sensitivity for eval suite? | Eval quality (AD-22) | Open | Evidence redundancy > 3, cluster disagreement > 0.4, and mincut fragility > 0.7 are initial thresholds. Need ablation study: how many prompts change label when thresholds shift by +/- 0.1? High flip rate suggests thresholds need tightening or a "borderline" fourth category. |
| 25 | Eval suite expansion cadence and adversarial prompt sourcing? | Eval coverage (AD-22) | Open | The initial 200-prompt suite covers known domains. How often should adversarial / distribution-shifted prompts be added? Potential sources: red-team exercises, production failure logs, community-submitted edge cases. Need a governance process for suite versioning. |
| 26 | Citation recall ground truth for multi-hop reasoning? | Eval accuracy (AD-22) | Open | Gate 2 citation recall assumes a flat list of relevant evidence chunks. For multi-hop questions requiring evidence chains (chunk A implies B, B implies answer), the `relevant_evidence` denominator is ambiguous — include intermediate chunks or only the final supporting evidence? Impacts recall threshold calibration. |
| 27 | Optimal GPU instance for teacher artifact generation? | Phase-1 cost (AD-23) | Open | Single A100 (80GB) vs 4×A10G vs spot instance with preemption risk? FP16 30B forward pass on 200 prompts needs ~60GB VRAM. Spot pricing could reduce the one-time cost from ~$50-200 to ~$15-60. |
| 28 | Teacher artifact format and versioning scheme? | Phase-1 operability (AD-23) | Open | Store routing traces as JSONL, Parquet, or binary protobuf? Versioning: hash of (teacher_model_revision + prompt_suite_hash + generation_config). Need deterministic teacher sampling (temperature=0, greedy) for reproducible artifacts. |
| 29 | Sparse logit selection strategy for Phase-1? | Phase-1 quality (AD-23) | Open | Which token positions get full logits? Options: (a) all tokens in answer spans, (b) only first/last token of each span, (c) positions where teacher top-1 vs top-2 logit margin < threshold. Strategy (c) focuses on uncertain positions but requires an extra teacher pass to compute margins. |
| 30 | Corpus perturbation protocol for stability testing? | Phase-1 eval (AD-23) | Open | "Remove 10% of corpus" — random subset? Stratified by source? Targeted removal of high-fragility chunks? Different strategies test different failure modes. Need a defined protocol before the perturbation test is meaningful. |
| 31 | Base embedder model selection for RLM embedder? | Embedding quality (AD-24) | Open | Candidates: all-MiniLM-L6-v2 (22M, 384-dim, fast), BGE-small (33M, 384-dim), nomic-embed-text (137M, 768-dim). Smaller models benefit more from recursive contextualization but have lower baseline quality. Need empirical comparison on target corpus. |
| 32 | Optimal iteration count for RLM embedder? | Latency vs quality (AD-24) | Open | 2 iterations is the minimum for context-aware re-embedding. 3 adds contradiction detection but ~50% more latency. Convergence threshold (cosine > 0.98) may terminate early. Need latency profiling on target hardware (Pi 5, Mac Studio, browser WASM). |
| 33 | Merge weight learning strategy? | Embedding quality (AD-24) | Open | Fixed weights (w0=0.6, w1=0.3, w2=0.1) vs grid search vs small regression on eval set. Grid search is simple but doesn't generalize across domains. Regression requires labeled retrieval pairs. Can we use RuVector's own retrieval accuracy as the training signal? |
| 34 | Ternary quantization of the base embedder? | Performance (AD-24) | Open | Can the base sentence transformer be ternary-quantized using Phase 0 PTQ? This would make the RLM embedder fully ternary — multiplication-free embedding. Quality impact on embeddings is unknown; may need separate evaluation. |

---

## 12. References

- ADR-017: Craftsman Ultra 30b 1bit — BitNet Integration with RuvLLM (v2, with RLM integration)
- ADR-002: RuvLLM Integration with Ruvector
- Microsoft Research, "The Era of 1-bit LLMs" (arXiv:2402.17764)
- Microsoft Research, "bitnet.cpp: Efficient Edge Inference for Ternary LLMs" (arXiv:2502.11880)
- Zhipu AI, GLM-4.7-Flash (https://huggingface.co/zai-org/GLM-4.7-Flash)
- Evans, Eric. "Domain-Driven Design: Tackling Complexity in the Heart of Software" (2003)
- Vernon, Vaughn. "Implementing Domain-Driven Design" (2013)
- RuvLLM GRPO Implementation: `crates/ruvllm/src/training/grpo.rs`
- RuvLLM RealContrastiveTrainer: `crates/ruvllm/src/training/real_trainer.rs`
- RuvLLM EWC++ Training Pipeline: `crates/ruvllm/src/lora/training.rs`
- RuvLLM Memory Distillation: `crates/ruvllm/src/reasoning_bank/distillation.rs`
- RuvLLM Policy Store: `crates/ruvllm/src/policy_store.rs`
- RuvLLM Contrastive Training: `crates/ruvllm/src/training/contrastive.rs`
- PT-BitNet: "Scaling up the 1-Bit large language model with post-training quantization" (2025)
- BitDistill: "BitNet Distillation" (arXiv:2510.13998, Oct 2025)
- bartowski, GLM-4.7-Flash-GGUF quantizations: https://huggingface.co/bartowski/zai-org_GLM-4.7-Flash-GGUF
- llama.cpp IQ1_S blind testing: https://github.com/ggml-org/llama.cpp/discussions/5962
- RuvLLM MicroLoRA NEON SIMD: `crates/ruvllm/src/lora/micro_lora.rs:279-390`
- RuvLLM NEON SIMD kernels: `crates/ruvllm/src/kernels/` (gemm_neon, gemv_neon, silu_neon, gelu_neon, relu_neon, rms_norm_neon, apply_rope_neon)
- RuvLLM ContrastiveTrainer CPU fallback: `crates/ruvllm/src/training/contrastive.rs:171-175`
- R3-Engine: Pure Rust BitNet inference with WASM SIMD128: https://github.com/r3-engine/r3-engine
- bitnet.rs: Pure Rust BitNet toolkit (Apache 2.0): https://github.com/ocentra/bitnet.rs
- WASM SIMD128 specification (V8): https://v8.dev/features/simd
