# RuvLLM Fine-Tuning Guide

This guide covers RuvLLM's fine-tuning capabilities, including MicroLoRA for real-time adaptation and EWC++ for preventing catastrophic forgetting.

## Overview

RuvLLM provides three levels of fine-tuning:

| Level | Technique | Latency | Use Case |
|-------|-----------|---------|----------|
| Instant | MicroLoRA | <1ms | Per-request adaptation |
| Background | Adapter Merge + EWC++ | ~100ms | Pattern consolidation |
| Deep | Full Training Pipeline | Minutes | Periodic optimization |

## MicroLoRA: Real-Time Adaptation

MicroLoRA enables per-request fine-tuning with minimal overhead.

### How It Works

```
User Request
     |
     v
+------------------+
| Compute Input    |
| Embedding        |
+------------------+
     |
     v
+------------------+    +------------------+
| Base Model       |--->| MicroLoRA Delta  |
| Forward Pass     |    | (rank 1-2)       |
+------------------+    +------------------+
     |                          |
     +----------+---------------+
                |
                v
+------------------+
| Combined Output  |
+------------------+
     |
     v
Response + Quality Feedback
     |
     v
+------------------+
| Update MicroLoRA |
| Weights          |
+------------------+
```

### Basic Usage

```rust
use ruvllm::lora::{MicroLoRA, MicroLoraConfig, AdaptFeedback, TargetModule};

// Create MicroLoRA for 4096-dim hidden states
let config = MicroLoraConfig::for_hidden_dim(4096);
let lora = MicroLoRA::new(config);

// During inference: apply LoRA delta
let base_output = model.forward(&input)?;
let lora_delta = lora.forward(&input, &TargetModule::QProj);

// Combine outputs
let output: Vec<f32> = base_output.iter()
    .zip(lora_delta.iter())
    .map(|(b, d)| b + d)
    .collect();

// After response: adapt based on feedback
let feedback = AdaptFeedback::from_quality(0.85);
lora.adapt(&input, feedback)?;

// Periodically apply accumulated gradients
lora.apply_updates(0.01); // learning rate
```

### Configuration Options

```rust
let config = MicroLoraConfig {
    // Input/output dimensions (typically hidden_dim)
    in_features: 4096,
    out_features: 4096,

    // LoRA rank: 1-2 for micro, 4-8 for standard
    rank: 2,

    // Scaling factor (effective_rank = alpha / rank)
    alpha: 4.0,

    // Dropout for regularization
    dropout: 0.0,

    // Which modules to adapt
    target_modules: vec![
        TargetModule::QProj,
        TargetModule::VProj,
    ],

    // Memory optimization
    gradient_checkpointing: false,
};
```

### Target Modules

Choose which transformer components to adapt:

| Module | Description | Memory | Impact |
|--------|-------------|--------|--------|
| `QProj` | Query projection | Low | High (attention focus) |
| `KProj` | Key projection | Low | Medium |
| `VProj` | Value projection | Low | High (content) |
| `OProj` | Output projection | Low | Medium |
| `GateProj` | FFN gate | Medium | High (routing) |
| `UpProj` | FFN up | High | Medium |
| `DownProj` | FFN down | High | Medium |

**Recommended combinations:**
- **Speed-focused**: `QProj` only
- **Quality-focused**: `QProj`, `VProj`
- **Full adaptation**: All attention projections

## EWC++ (Elastic Weight Consolidation)

EWC++ prevents catastrophic forgetting when adapting to new tasks.

### How It Works

```
Task 1 Training
     |
     v
+------------------+
| Compute Fisher   |
| Information      |
| F = E[grad^2]    |
+------------------+
     |
     v
+------------------+
| Store Optimal    |
| Weights θ*       |
+------------------+

...later...

Task 2 Training
     |
     v
+------------------+
| Regularized Loss |
| L = L_task +     |
| λ Σ F_i(θ-θ*)²   |
+------------------+
     |
     v
+------------------+
| Update with      |
| Importance       |
| Weights          |
+------------------+
```

### Using EWC++ with MicroLoRA

```rust
use ruvllm::lora::{MicroLoRA, TrainingPipeline, TrainingConfig};

// Create training pipeline with EWC++
let training_config = TrainingConfig {
    learning_rate: 0.001,
    ewc_lambda: 0.1,  // Regularization strength
    ..Default::default()
};

let mut pipeline = TrainingPipeline::new(training_config);
pipeline.init_for_lora(&lora);

// Train on task 1
for sample in task1_samples {
    pipeline.train_step(&lora, &sample.input, sample.feedback)?;
}

// Mark end of task 1 (computes Fisher information)
pipeline.start_new_task(&lora);

// Train on task 2 (EWC++ regularization active)
for sample in task2_samples {
    pipeline.train_step(&lora, &sample.input, sample.feedback)?;
}
```

### EWC++ Configuration

```rust
let config = TrainingConfig {
    // Base learning rate
    learning_rate: 0.001,

    // EWC regularization strength
    // Higher = more preservation of old knowledge
    // Lower = more adaptation to new tasks
    ewc_lambda: 0.1,

    // Minimum quality for learning
    quality_threshold: 0.5,

    // Fisher information estimation samples
    fisher_samples: 100,

    // Online Fisher update rate
    online_ewc_gamma: 0.95,
};
```

## SONA Learning Loops

SONA provides automated multi-tier learning.

### Architecture

```
+-------------------+     +-------------------+
| Inference Request |---->| Instant Loop      |
| + feedback        |     | - MicroLoRA adapt |
+-------------------+     | - <1ms latency    |
                          +--------+----------+
                                   |
                                   v (async, 100ms)
                          +--------+----------+
                          | Background Loop   |
                          | - Pattern merge   |
                          | - Adapter compose |
                          | - EWC++ update    |
                          +--------+----------+
                                   |
                                   v (triggered)
                          +--------+----------+
                          | Deep Loop         |
                          | - Full fine-tune  |
                          | - Model distill   |
                          | - Pattern bank    |
                          +-------------------+
```

### Using SONA

```rust
use ruvllm::optimization::{SonaLlm, SonaLlmConfig};

// Create SONA integration
let config = SonaLlmConfig {
    instant_lr: 0.01,
    background_interval_ms: 100,
    background_min_samples: 10,
    deep_trigger_threshold: 100.0,
    consolidation_strategy: ConsolidationStrategy::EwcMerge,
    ..Default::default()
};

let sona = SonaLlm::new(config);

// During inference
let response = model.generate(&query)?;

// Record feedback (runs instant loop)
let result = sona.instant_adapt(&query, &response, 0.85);
println!("Instant adapt latency: {}μs", result.latency_us);

// Periodically check background loop
if let Some(bg_result) = sona.maybe_background() {
    println!("Background: {} samples, quality delta: {:.3}",
        bg_result.samples_used, bg_result.quality_delta);
}

// Check if deep loop should trigger
if sona.should_trigger_deep() {
    let samples = collect_training_samples();
    let deep_result = sona.deep_optimize(&samples);
    println!("Deep optimization complete");
}
```

### Consolidation Strategies

```rust
pub enum ConsolidationStrategy {
    /// EWC++ merge (default) - preserves important weights
    EwcMerge,

    /// Simple averaging - fast but may lose specialization
    Average,

    /// Quality-weighted - higher quality samples have more influence
    QualityWeighted,

    /// Best only - keep top 20% by quality
    BestOnly,

    /// Ensemble - maintain multiple adapters
    Ensemble,
}
```

**Recommendations:**
- `EwcMerge`: Best for multi-domain use
- `QualityWeighted`: Best for quality optimization
- `BestOnly`: Best for high-variance feedback
- `Ensemble`: Best when you have distinct use cases

## Training Data Format

### TrainingSample

```rust
pub struct TrainingSample {
    /// Input embedding
    pub input_embedding: Vec<f32>,

    /// Output embedding
    pub output_embedding: Vec<f32>,

    /// Query text (optional)
    pub query: Option<String>,

    /// Response text (optional)
    pub response: Option<String>,

    /// Quality score (0.0 - 1.0)
    pub quality: f32,

    /// Latency in milliseconds
    pub latency_ms: f32,

    /// Token count
    pub token_count: usize,

    /// Session identifier
    pub session_id: String,
}
```

### Creating Training Samples

```rust
let sample = TrainingSample::new(
    input_embedding,
    output_embedding,
    0.9,  // quality
)
.with_query("What is machine learning?".to_string())
.with_response("Machine learning is...".to_string())
.with_latency(150.0)  // ms
.with_session("session-123".to_string());
```

## Adapter Management

### Saving and Loading Adapters

```rust
// Save adapter state
let adapter_bytes = lora.export_weights()?;
std::fs::write("adapter.bin", &adapter_bytes)?;

// Load adapter state
let adapter_bytes = std::fs::read("adapter.bin")?;
lora.import_weights(&adapter_bytes)?;
```

### Merging Adapters

```rust
// Merge multiple adapters with weights
let adapters = vec![
    (adapter1, 0.6),  // 60% weight
    (adapter2, 0.4),  // 40% weight
];

let merged = MicroLoRA::merge_adapters(&adapters)?;
```

### Adapter Composition

```rust
// Sequential composition: adapter1 -> adapter2
let composed = MicroLoRA::compose_sequential(&[adapter1, adapter2])?;

// Parallel composition: average outputs
let composed = MicroLoRA::compose_parallel(&[adapter1, adapter2])?;
```

## Best Practices

### 1. Quality Threshold Selection

```rust
let config = TrainingConfig {
    // Too low: learns from poor examples
    // Too high: learns very slowly
    // Recommended: 0.5 - 0.7
    quality_threshold: 0.6,
    ..Default::default()
};
```

### 2. Learning Rate Scheduling

```rust
// Start high for quick adaptation
let initial_lr = 0.01;

// Reduce over time for stability
let decay_lr = |epoch: usize| -> f32 {
    initial_lr * 0.95_f32.powi(epoch as i32)
};
```

### 3. Memory Management

```rust
// For memory-constrained environments
let config = MicroLoraConfig {
    rank: 1,  // Minimum rank
    target_modules: vec![TargetModule::QProj],  // Single module
    gradient_checkpointing: true,
    ..Default::default()
};
```

### 4. Preventing Overfitting

```rust
let config = MicroLoraConfig {
    dropout: 0.1,  // Add regularization
    ..Default::default()
};

let training_config = TrainingConfig {
    ewc_lambda: 0.5,  // Strong regularization
    ..Default::default()
};
```

## Monitoring and Debugging

### Statistics

```rust
let stats = sona.stats();
println!("Learning Statistics:");
println!("  Instant updates: {}", stats.instant_count);
println!("  Avg instant latency: {:.2}μs", stats.instant_avg_latency_us);
println!("  Background updates: {}", stats.background_count);
println!("  Pending samples: {}", stats.pending_samples);
println!("  Accumulated quality: {:.2}", stats.accumulated_quality);
```

### Debugging Adaptation

```rust
// Enable debug logging
std::env::set_var("RUST_LOG", "ruvllm::lora=debug");

// Check adaptation result
let result = sona.instant_adapt(&query, &response, feedback);
if !result.applied {
    println!("Adaptation skipped: {:?}", result.notes);
}
```

## Performance Tuning

### Latency Optimization

| Setting | Low Latency | Balanced | High Quality |
|---------|-------------|----------|--------------|
| LoRA rank | 1 | 2 | 4 |
| Target modules | 1 | 2 | 4 |
| Background interval | 200ms | 100ms | 50ms |
| EWC lambda | 0.0 | 0.1 | 0.5 |

### Memory Optimization

```rust
// Minimal memory footprint
let config = SonaLlmConfig {
    max_pending_samples: 100,  // Reduce buffer
    micro_lora: MicroLoraConfig {
        rank: 1,
        target_modules: vec![TargetModule::QProj],
        ..Default::default()
    },
    ..Default::default()
};
```

## Troubleshooting

### Adaptation Not Improving

1. Check quality threshold isn't too high
2. Verify feedback is meaningful (not always same value)
3. Increase learning rate
4. Try different target modules

### Catastrophic Forgetting

1. Increase EWC lambda
2. Use `EwcMerge` consolidation strategy
3. Reduce learning rate
4. Add more diverse training data

### High Latency

1. Reduce LoRA rank to 1
2. Reduce target modules
3. Increase background interval
4. Use `gradient_checkpointing`
