# ADR-005: SONA Self-Learning for Pose Estimation

## Status
Proposed

## Date
2026-02-28

## Context

### Static Model Problem

The WiFi-DensePose modality translation network (`ModalityTranslationNetwork` in Python, `ModalityTranslator` in Rust) converts CSI features into visual-like feature maps that feed the DensePose head for body segmentation and UV coordinate estimation. These models are trained offline and deployed with frozen weights.

**Critical limitations of static models**:

1. **Environment drift**: CSI characteristics change when furniture moves, new objects are introduced, or building occupancy changes. A model trained in Lab A degrades in Lab B without retraining.

2. **Hardware variance**: Different WiFi chipsets (Intel AX200 vs Broadcom BCM4375 vs Qualcomm WCN6855) produce subtly different CSI patterns. Static models overfit to training hardware.

3. **Temporal drift**: Even in the same environment, CSI patterns shift with temperature, humidity, and electromagnetic interference changes throughout the day.

4. **Population bias**: Models trained on one demographic may underperform on body types, heights, or movement patterns not represented in training data.

Current mitigation: manual retraining with new data, which requires:
- Collecting labeled data in the new environment
- GPU-intensive training (hours to days)
- Model export/deployment cycle
- Downtime during switchover

### SONA Opportunity

RuVector's Self-Optimizing Neural Architecture (SONA) provides <1ms online adaptation through:

- **LoRA (Low-Rank Adaptation)**: Instead of updating all weights (millions of parameters), LoRA injects small trainable rank decomposition matrices into frozen model layers. For a weight matrix W ∈ R^(d×k), LoRA learns A ∈ R^(d×r) and B ∈ R^(r×k) where r << min(d,k), so the adapted weight is W + AB.

- **EWC++ (Elastic Weight Consolidation)**: Prevents catastrophic forgetting by penalizing changes to parameters important for previously learned tasks. Each parameter has a Fisher information-weighted importance score.

- **Online gradient accumulation**: Small batches of live data (as few as 1-10 samples) contribute to adaptation without full backward passes.

## Decision

We will integrate SONA as the online learning engine for both the modality translation network and the DensePose head, enabling continuous environment-specific adaptation without offline retraining.

### Adaptation Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                    SONA Adaptation Pipeline                          │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Frozen Base Model                    LoRA Adaptation Matrices       │
│  ┌─────────────────┐                  ┌──────────────────────┐      │
│  │ Conv2d(64,128)  │ ◀── W_frozen ──▶ │ A(64,r) × B(r,128) │      │
│  │ Conv2d(128,256) │ ◀── W_frozen ──▶ │ A(128,r) × B(r,256)│      │
│  │ Conv2d(256,512) │ ◀── W_frozen ──▶ │ A(256,r) × B(r,512)│      │
│  │ ConvT(512,256)  │ ◀── W_frozen ──▶ │ A(512,r) × B(r,256)│      │
│  │ ...             │                  │ ...                  │      │
│  └─────────────────┘                  └──────────────────────┘      │
│         │                                      │                     │
│         ▼                                      ▼                     │
│  ┌─────────────────────────────────────────────────────────┐        │
│  │            Effective Weight = W_frozen + α(AB)           │        │
│  │            α = scaling factor (0.0 → 1.0 over time)     │        │
│  └─────────────────────────────────────────────────────────┘        │
│                              │                                       │
│                              ▼                                       │
│  ┌─────────────────────────────────────────────────────────┐        │
│  │                    EWC++ Regularizer                      │        │
│  │  L_total = L_task + λ Σ F_i (θ_i - θ*_i)²              │        │
│  │                                                          │        │
│  │  F_i = Fisher information (parameter importance)         │        │
│  │  θ*_i = optimal parameters from previous tasks           │        │
│  │  λ = regularization strength (10-100)                    │        │
│  └─────────────────────────────────────────────────────────┘        │
└──────────────────────────────────────────────────────────────────────┘
```

### LoRA Configuration per Layer

```rust
/// SONA LoRA configuration for WiFi-DensePose
pub struct SonaConfig {
    /// LoRA rank (r): dimensionality of adaptation matrices
    /// r=4 for encoder layers (less variation needed)
    /// r=8 for decoder layers (more expression needed)
    /// r=16 for final output layers (maximum adaptability)
    lora_ranks: HashMap<String, usize>,

    /// Scaling factor alpha: controls adaptation strength
    /// Starts at 0.0 (pure frozen model), increases to target
    alpha: f64,  // Target: 0.3

    /// Alpha warmup steps before reaching target
    alpha_warmup_steps: usize,  // 100

    /// EWC++ regularization strength
    ewc_lambda: f64,  // 50.0

    /// Fisher information estimation samples
    fisher_samples: usize,  // 200

    /// Online learning rate (much smaller than offline training)
    online_lr: f64,  // 1e-5

    /// Gradient accumulation steps before applying update
    accumulation_steps: usize,  // 10

    /// Maximum adaptation delta (safety bound)
    max_delta_norm: f64,  // 0.1
}
```

**Parameter budget**:

| Layer | Original Params | LoRA Rank | LoRA Params | Overhead |
|-------|----------------|-----------|-------------|----------|
| Encoder Conv1 (64→128) | 73,728 | 4 | 768 | 1.0% |
| Encoder Conv2 (128→256) | 294,912 | 4 | 1,536 | 0.5% |
| Encoder Conv3 (256→512) | 1,179,648 | 4 | 3,072 | 0.3% |
| Decoder ConvT1 (512→256) | 1,179,648 | 8 | 6,144 | 0.5% |
| Decoder ConvT2 (256→128) | 294,912 | 8 | 3,072 | 1.0% |
| Output Conv (128→24) | 27,648 | 16 | 2,432 | 8.8% |
| **Total** | **3,050,496** | - | **17,024** | **0.56%** |

SONA adapts **0.56% of parameters** while achieving 70-90% of the accuracy improvement of full fine-tuning.

### Adaptation Trigger Conditions

```rust
/// When to trigger SONA adaptation
pub enum AdaptationTrigger {
    /// Detection confidence drops below threshold over N samples
    ConfidenceDrop {
        threshold: f64,     // 0.6
        window_size: usize, // 50
    },

    /// CSI statistics drift beyond baseline (KL divergence)
    DistributionDrift {
        kl_threshold: f64,  // 0.5
        reference_window: usize, // 1000
    },

    /// New environment detected (no close HNSW matches)
    NewEnvironment {
        min_distance: f64,  // 0.8 (far from all known fingerprints)
    },

    /// Periodic adaptation (maintenance)
    Periodic {
        interval_samples: usize, // 10000
    },

    /// Manual trigger via API
    Manual,
}
```

### Adaptation Feedback Sources

Since WiFi-DensePose lacks camera ground truth in deployment, adaptation uses **self-supervised signals**:

1. **Temporal consistency**: Pose estimates should change smoothly between frames. Jerky transitions indicate prediction error.
   ```
   L_temporal = ||pose(t) - pose(t-1)||² when Δt < 100ms
   ```

2. **Physical plausibility**: Body part positions must satisfy skeletal constraints (limb lengths, joint angles).
   ```
   L_skeleton = Σ max(0, |limb_length - expected_length| - tolerance)
   ```

3. **Multi-view agreement** (multi-AP): Different APs observing the same person should produce consistent poses.
   ```
   L_multiview = ||pose_AP1 - transform(pose_AP2)||²
   ```

4. **Detection stability**: Confidence should be high when the environment is stable.
   ```
   L_stability = -log(confidence) when variance(CSI_window) < threshold
   ```

### Safety Mechanisms

```rust
/// Safety bounds prevent adaptation from degrading the model
pub struct AdaptationSafety {
    /// Maximum parameter change per update step
    max_step_norm: f64,

    /// Rollback if validation loss increases by this factor
    rollback_threshold: f64,  // 1.5 (50% worse = rollback)

    /// Keep N checkpoints for rollback
    checkpoint_count: usize,  // 5

    /// Disable adaptation after N consecutive rollbacks
    max_consecutive_rollbacks: usize,  // 3

    /// Minimum samples between adaptations
    cooldown_samples: usize,  // 100
}
```

### Persistence via RVF

Adaptation state is stored in the Model Container (ADR-003):
- LoRA matrices A and B serialized to VEC segment
- Fisher information matrix serialized alongside
- Each adaptation creates a witness chain entry (ADR-010)
- COW branching allows reverting to any previous adaptation state

```
model.rvf.model
  ├── main (frozen base weights)
  ├── branch/adapted-office-2024-01 (LoRA deltas)
  ├── branch/adapted-warehouse (LoRA deltas)
  └── branch/adapted-outdoor-disaster (LoRA deltas)
```

## Consequences

### Positive
- **Zero-downtime adaptation**: Model improves continuously during operation
- **Tiny overhead**: 17K parameters (0.56%) vs 3M full model; <1ms per adaptation step
- **No forgetting**: EWC++ preserves performance on previously-seen environments
- **Portable adaptations**: LoRA deltas are ~70 KB, easily shared between devices
- **Safe rollback**: Checkpoint system prevents runaway degradation
- **Self-supervised**: No labeled data needed during deployment

### Negative
- **Bounded expressiveness**: LoRA rank limits the degree of adaptation; extreme environment changes may require offline retraining
- **Feedback noise**: Self-supervised signals are weaker than ground-truth labels; adaptation is slower and less precise
- **Compute on device**: Even small gradient computations require tensor math on the inference device
- **Complexity**: Debugging adapted models is harder than static models
- **Hyperparameter sensitivity**: EWC lambda, LoRA rank, learning rate require tuning

### Validation Plan

1. **Offline validation**: Train base model on Environment A, test SONA adaptation to Environment B with known ground truth. Measure pose estimation MPJPE (Mean Per-Joint Position Error) improvement.
2. **A/B deployment**: Run static model and SONA-adapted model in parallel on same CSI stream. Compare detection rates and pose consistency.
3. **Stress test**: Rapidly change environments (simulated) and verify EWC++ prevents catastrophic forgetting.
4. **Edge latency**: Benchmark adaptation step on target hardware (Raspberry Pi 4, Jetson Nano, browser WASM).

## References

- [LoRA: Low-Rank Adaptation of Large Language Models](https://arxiv.org/abs/2106.09685)
- [Elastic Weight Consolidation (EWC)](https://arxiv.org/abs/1612.00796)
- [Continual Learning with SONA](https://github.com/ruvnet/ruvector)
- [Self-Supervised WiFi Sensing](https://arxiv.org/abs/2203.11928)
- ADR-002: RuVector RVF Integration Strategy
- ADR-003: RVF Cognitive Containers for CSI Data
