# FastGRNN Training Pipeline Guide

This guide covers the complete training pipeline for the FastGRNN model used in Tiny Dancer's neural routing system.

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Quick Start](#quick-start)
4. [Training Configuration](#training-configuration)
5. [Data Preparation](#data-preparation)
6. [Training Loop](#training-loop)
7. [Knowledge Distillation](#knowledge-distillation)
8. [Advanced Features](#advanced-features)
9. [Production Deployment](#production-deployment)

## Overview

The FastGRNN training pipeline provides a complete solution for training lightweight recurrent neural networks for AI agent routing decisions. Key features include:

- **Adam Optimizer**: State-of-the-art adaptive learning rate optimization
- **Mini-batch Training**: Efficient batch processing with configurable batch sizes
- **Early Stopping**: Automatic stopping when validation loss stops improving
- **Learning Rate Scheduling**: Exponential decay for better convergence
- **Knowledge Distillation**: Learn from larger teacher models
- **Gradient Clipping**: Prevent exploding gradients
- **L2 Regularization**: Prevent overfitting

## Architecture

### FastGRNN Cell

The FastGRNN (Fast Gated Recurrent Neural Network) uses a simplified gating mechanism:

```
r_t = σ(W_r × x_t + b_r)                    [Reset gate]
u_t = σ(W_u × x_t + b_u)                    [Update gate]
c_t = tanh(W_c × x_t + W × (r_t ⊙ h_t-1))  [Candidate state]
h_t = u_t ⊙ h_t-1 + (1 - u_t) ⊙ c_t         [Hidden state]
y_t = σ(W_out × h_t + b_out)                [Output]
```

Where:
- `σ` is the sigmoid activation with scaling parameter `nu`
- `tanh` is the hyperbolic tangent with scaling parameter `zeta`
- `⊙` denotes element-wise multiplication

### Training Pipeline

```
┌─────────────────┐
│  Raw Features   │
│  + Labels       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Normalization  │
│  (z-score)      │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Train/Val      │
│  Split          │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Mini-batch     │
│  Training       │
│  (BPTT)         │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Adam Update    │
│  + Grad Clip    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Validation     │
│  + Early Stop   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Trained Model  │
└─────────────────┘
```

## Quick Start

### Basic Training

```rust
use ruvector_tiny_dancer_core::{
    model::{FastGRNN, FastGRNNConfig},
    training::{TrainingConfig, TrainingDataset, Trainer},
};

// 1. Prepare your data
let features = vec![
    vec![0.8, 0.9, 0.7, 0.85, 0.2], // High confidence case
    vec![0.3, 0.2, 0.4, 0.35, 0.9], // Low confidence case
    // ... more samples
];
let labels = vec![1.0, 0.0, /* ... */]; // 1.0 = lightweight, 0.0 = powerful

let mut dataset = TrainingDataset::new(features, labels)?;

// 2. Normalize features
let (means, stds) = dataset.normalize()?;

// 3. Create model
let model_config = FastGRNNConfig {
    input_dim: 5,
    hidden_dim: 16,
    output_dim: 1,
    nu: 0.8,
    zeta: 1.2,
    rank: Some(8),
};
let mut model = FastGRNN::new(model_config.clone())?;

// 4. Configure training
let training_config = TrainingConfig {
    learning_rate: 0.01,
    batch_size: 32,
    epochs: 50,
    validation_split: 0.2,
    early_stopping_patience: Some(5),
    ..Default::default()
};

// 5. Train
let mut trainer = Trainer::new(&model_config, training_config);
let metrics = trainer.train(&mut model, &dataset)?;

// 6. Save model
model.save("models/fastgrnn.safetensors")?;
```

### Run the Example

```bash
cd crates/ruvector-tiny-dancer-core
cargo run --example train-model
```

## Training Configuration

### Hyperparameters

```rust
pub struct TrainingConfig {
    /// Learning rate (default: 0.001)
    pub learning_rate: f32,

    /// Batch size (default: 32)
    pub batch_size: usize,

    /// Number of epochs (default: 100)
    pub epochs: usize,

    /// Validation split ratio (default: 0.2)
    pub validation_split: f32,

    /// Early stopping patience (default: Some(10))
    pub early_stopping_patience: Option<usize>,

    /// Learning rate decay factor (default: 0.5)
    pub lr_decay: f32,

    /// Learning rate decay step in epochs (default: 20)
    pub lr_decay_step: usize,

    /// Gradient clipping threshold (default: 5.0)
    pub grad_clip: f32,

    /// Adam beta1 parameter (default: 0.9)
    pub adam_beta1: f32,

    /// Adam beta2 parameter (default: 0.999)
    pub adam_beta2: f32,

    /// Adam epsilon (default: 1e-8)
    pub adam_epsilon: f32,

    /// L2 regularization strength (default: 1e-5)
    pub l2_reg: f32,
}
```

### Recommended Settings

#### Small Datasets (< 1,000 samples)
```rust
TrainingConfig {
    learning_rate: 0.01,
    batch_size: 16,
    epochs: 100,
    validation_split: 0.2,
    early_stopping_patience: Some(10),
    lr_decay: 0.8,
    lr_decay_step: 20,
    l2_reg: 1e-4,
    ..Default::default()
}
```

#### Medium Datasets (1,000 - 10,000 samples)
```rust
TrainingConfig {
    learning_rate: 0.005,
    batch_size: 32,
    epochs: 50,
    validation_split: 0.15,
    early_stopping_patience: Some(5),
    lr_decay: 0.7,
    lr_decay_step: 10,
    l2_reg: 1e-5,
    ..Default::default()
}
```

#### Large Datasets (> 10,000 samples)
```rust
TrainingConfig {
    learning_rate: 0.001,
    batch_size: 64,
    epochs: 30,
    validation_split: 0.1,
    early_stopping_patience: Some(3),
    lr_decay: 0.5,
    lr_decay_step: 5,
    l2_reg: 1e-6,
    ..Default::default()
}
```

## Data Preparation

### Feature Engineering

For routing decisions, typical features include:

```rust
pub struct RoutingFeatures {
    /// Semantic similarity between query and candidate (0.0 to 1.0)
    pub similarity: f32,

    /// Recency score - how recently was this candidate accessed (0.0 to 1.0)
    pub recency: f32,

    /// Popularity score - how often is this candidate used (0.0 to 1.0)
    pub popularity: f32,

    /// Historical success rate for this candidate (0.0 to 1.0)
    pub success_rate: f32,

    /// Query complexity estimate (0.0 to 1.0)
    pub complexity: f32,
}

impl RoutingFeatures {
    fn to_vector(&self) -> Vec<f32> {
        vec![
            self.similarity,
            self.recency,
            self.popularity,
            self.success_rate,
            self.complexity,
        ]
    }
}
```

### Data Collection

```rust
// Collect training data from production logs
fn collect_training_data(logs: &[RoutingLog]) -> (Vec<Vec<f32>>, Vec<f32>) {
    let mut features = Vec::new();
    let mut labels = Vec::new();

    for log in logs {
        // Extract features
        let feature_vec = vec![
            log.similarity_score,
            log.recency_score,
            log.popularity_score,
            log.success_rate,
            log.complexity_score,
        ];

        // Label based on actual outcome
        // 1.0 if lightweight model was sufficient
        // 0.0 if powerful model was needed
        let label = if log.lightweight_successful { 1.0 } else { 0.0 };

        features.push(feature_vec);
        labels.push(label);
    }

    (features, labels)
}
```

### Data Normalization

Always normalize your features before training:

```rust
let mut dataset = TrainingDataset::new(features, labels)?;
let (means, stds) = dataset.normalize()?;

// Save normalization parameters for inference
save_normalization_params("models/normalization.json", &means, &stds)?;
```

During inference, apply the same normalization:

```rust
fn normalize_features(features: &mut [f32], means: &[f32], stds: &[f32]) {
    for (i, feat) in features.iter_mut().enumerate() {
        *feat = (*feat - means[i]) / stds[i];
    }
}
```

## Training Loop

### Basic Training

```rust
let mut trainer = Trainer::new(&model_config, training_config);
let metrics = trainer.train(&mut model, &dataset)?;

// Print final results
if let Some(last) = metrics.last() {
    println!("Final validation accuracy: {:.2}%", last.val_accuracy * 100.0);
}
```

### Custom Training Loop

For more control, implement your own training loop:

```rust
use ruvector_tiny_dancer_core::training::BatchIterator;

for epoch in 0..config.epochs {
    let mut epoch_loss = 0.0;
    let mut n_batches = 0;

    // Training phase
    let batch_iter = BatchIterator::new(&train_dataset, config.batch_size, true);
    for (features, labels, _) in batch_iter {
        // Forward pass
        let predictions: Vec<f32> = features
            .iter()
            .map(|f| model.forward(f, None).unwrap())
            .collect();

        // Compute loss
        let batch_loss: f32 = predictions
            .iter()
            .zip(&labels)
            .map(|(&pred, &target)| binary_cross_entropy(pred, target))
            .sum::<f32>() / predictions.len() as f32;

        epoch_loss += batch_loss;
        n_batches += 1;

        // Backward pass (simplified - real implementation needs BPTT)
        // ...
    }

    println!("Epoch {}: loss = {:.4}", epoch, epoch_loss / n_batches as f32);
}
```

## Knowledge Distillation

Knowledge distillation allows a smaller "student" model to learn from a larger "teacher" model.

### Setup

```rust
use ruvector_tiny_dancer_core::training::{
    generate_teacher_predictions,
    temperature_softmax,
};

// 1. Create/load teacher model (larger, pre-trained)
let teacher_config = FastGRNNConfig {
    input_dim: 5,
    hidden_dim: 32,  // Larger than student
    output_dim: 1,
    ..Default::default()
};
let teacher = FastGRNN::load("models/teacher.safetensors")?;

// 2. Generate soft targets
let temperature = 3.0;  // Higher = softer probabilities
let soft_targets = generate_teacher_predictions(
    &teacher,
    &dataset.features,
    temperature
)?;

// 3. Add soft targets to dataset
let dataset = dataset.with_soft_targets(soft_targets)?;

// 4. Enable distillation in training config
let training_config = TrainingConfig {
    enable_distillation: true,
    distillation_temperature: temperature,
    distillation_alpha: 0.7,  // 70% soft targets, 30% hard targets
    ..Default::default()
};
```

### Distillation Loss

The total loss combines hard and soft targets:

```
L_total = α × L_soft + (1 - α) × L_hard

where:
- L_soft = BCE(student_logit / T, teacher_logit / T)
- L_hard = BCE(student_logit, true_label)
- α = distillation_alpha (typically 0.5 to 0.9)
- T = temperature (typically 2.0 to 5.0)
```

### Benefits

- **Faster Inference**: Student model is smaller and faster
- **Better Accuracy**: Student learns from teacher's knowledge
- **Compression**: 2-4x smaller models with minimal accuracy loss
- **Transfer Learning**: Transfer knowledge across architectures

## Advanced Features

### Learning Rate Scheduling

Exponential decay schedule:

```rust
TrainingConfig {
    learning_rate: 0.01,      // Initial LR
    lr_decay: 0.8,            // Multiply by 0.8 every lr_decay_step epochs
    lr_decay_step: 10,        // Decay every 10 epochs
    ..Default::default()
}

// Schedule:
// Epochs 0-9:   LR = 0.01
// Epochs 10-19: LR = 0.008
// Epochs 20-29: LR = 0.0064
// Epochs 30-39: LR = 0.00512
// ...
```

### Early Stopping

Prevent overfitting by stopping when validation loss stops improving:

```rust
TrainingConfig {
    early_stopping_patience: Some(5),  // Stop after 5 epochs without improvement
    ..Default::default()
}
```

### Gradient Clipping

Prevent exploding gradients in RNNs:

```rust
TrainingConfig {
    grad_clip: 5.0,  // Clip gradients to [-5.0, 5.0]
    ..Default::default()
}
```

### Regularization

L2 weight decay to prevent overfitting:

```rust
TrainingConfig {
    l2_reg: 1e-5,  // Add L2 penalty to loss
    ..Default::default()
}
```

## Production Deployment

### Training Pipeline

1. **Data Collection**
   ```rust
   // Collect production logs
   let logs = collect_routing_logs_from_db(db_path)?;
   let (features, labels) = extract_features_and_labels(&logs);
   ```

2. **Data Validation**
   ```rust
   // Check data quality
   assert!(features.len() >= 1000, "Need at least 1000 samples");
   assert!(labels.iter().filter(|&&l| l > 0.5).count() > 100,
           "Need balanced dataset");
   ```

3. **Training**
   ```rust
   let mut dataset = TrainingDataset::new(features, labels)?;
   let (means, stds) = dataset.normalize()?;

   let mut trainer = Trainer::new(&model_config, training_config);
   let metrics = trainer.train(&mut model, &dataset)?;
   ```

4. **Validation**
   ```rust
   // Test on holdout set
   let (_, test_dataset) = dataset.split(0.2)?;
   let (test_loss, test_accuracy) = evaluate_model(&model, &test_dataset)?;

   assert!(test_accuracy > 0.85, "Model accuracy too low");
   ```

5. **Save Artifacts**
   ```rust
   // Save model
   model.save("models/fastgrnn_v1.safetensors")?;

   // Save normalization params
   save_normalization("models/normalization_v1.json", &means, &stds)?;

   // Save metrics
   trainer.save_metrics("models/metrics_v1.json")?;
   ```

6. **Optimization**
   ```rust
   // Quantize for production
   model.quantize()?;

   // Optional: Prune weights
   model.prune(0.3)?;  // 30% sparsity
   ```

### Continual Learning

Update the model with new data:

```rust
// Load existing model
let mut model = FastGRNN::load("models/current.safetensors")?;

// Collect new data
let new_logs = collect_recent_logs(since_timestamp)?;
let (new_features, new_labels) = extract_features_and_labels(&new_logs);

// Create dataset
let new_dataset = TrainingDataset::new(new_features, new_labels)?;

// Fine-tune with lower learning rate
let training_config = TrainingConfig {
    learning_rate: 0.0001,  // Lower LR for fine-tuning
    epochs: 10,
    ..Default::default()
};

let mut trainer = Trainer::new(model.config(), training_config);
trainer.train(&mut model, &new_dataset)?;

// Save updated model
model.save("models/current_v2.safetensors")?;
```

### Model Versioning

```rust
use chrono::Utc;

pub struct ModelVersion {
    pub version: String,
    pub timestamp: i64,
    pub model_path: String,
    pub metrics_path: String,
    pub normalization_path: String,
    pub test_accuracy: f32,
    pub model_size_bytes: usize,
}

impl ModelVersion {
    pub fn create_new(model: &FastGRNN, metrics: &[TrainingMetrics]) -> Self {
        let timestamp = Utc::now().timestamp();
        let version = format!("v{}", timestamp);

        Self {
            version: version.clone(),
            timestamp,
            model_path: format!("models/fastgrnn_{}.safetensors", version),
            metrics_path: format!("models/metrics_{}.json", version),
            normalization_path: format!("models/norm_{}.json", version),
            test_accuracy: metrics.last().unwrap().val_accuracy,
            model_size_bytes: model.size_bytes(),
        }
    }
}
```

## Performance Benchmarks

### Training Speed

| Dataset Size | Batch Size | Epoch Time | Total Time (50 epochs) |
|--------------|------------|------------|------------------------|
| 1,000        | 32         | 0.2s       | 10s                    |
| 10,000       | 64         | 1.5s       | 75s                    |
| 100,000      | 128        | 12s        | 600s (10 min)          |

### Model Size

| Configuration      | Parameters | FP32 Size | INT8 Size | Compression |
|--------------------|------------|-----------|-----------|-------------|
| Tiny (8 hidden)    | ~250       | 1 KB      | 256 B     | 4x          |
| Small (16 hidden)  | ~850       | 3.4 KB    | 850 B     | 4x          |
| Medium (32 hidden) | ~3,200     | 12.8 KB   | 3.2 KB    | 4x          |

### Inference Speed

After training and quantization:

- **Inference time**: < 100 μs per sample
- **Batch inference** (32 samples): < 1 ms
- **Memory footprint**: < 5 KB

## Troubleshooting

### Common Issues

#### 1. Loss Not Decreasing

**Symptoms**: Training loss stays high or increases

**Solutions**:
- Reduce learning rate (try 0.001 or lower)
- Increase batch size
- Check data normalization
- Verify labels are correct (0.0 or 1.0)
- Add more training data

#### 2. Overfitting

**Symptoms**: Training accuracy high, validation accuracy low

**Solutions**:
- Increase L2 regularization (try 1e-4)
- Reduce model size (fewer hidden units)
- Use early stopping
- Add more training data
- Increase validation split

#### 3. Slow Convergence

**Symptoms**: Training takes too many epochs

**Solutions**:
- Increase learning rate (try 0.01 or 0.1)
- Use knowledge distillation
- Better feature engineering
- Use larger batch sizes

#### 4. Gradient Explosion

**Symptoms**: Loss becomes NaN, training crashes

**Solutions**:
- Enable gradient clipping (grad_clip: 1.0 or 5.0)
- Reduce learning rate
- Check for invalid data (NaN, Inf values)

## Next Steps

1. **Run the example**: `cargo run --example train-model`
2. **Collect your own data**: Integrate with production logs
3. **Experiment with hyperparameters**: Find optimal settings
4. **Deploy to production**: Integrate with the Router
5. **Monitor performance**: Track accuracy and latency
6. **Iterate**: Collect more data and retrain regularly

## References

- FastGRNN Paper: [Resource-efficient Machine Learning in 2 KB RAM for the Internet of Things](https://arxiv.org/abs/1901.02358)
- Knowledge Distillation: [Distilling the Knowledge in a Neural Network](https://arxiv.org/abs/1503.02531)
- Adam Optimizer: [Adam: A Method for Stochastic Optimization](https://arxiv.org/abs/1412.6980)
