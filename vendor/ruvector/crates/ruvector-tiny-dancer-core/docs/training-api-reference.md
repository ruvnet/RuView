# Training API Reference

## Module: `ruvector_tiny_dancer_core::training`

Complete API reference for the FastGRNN training pipeline.

## Core Types

### TrainingConfig

Configuration for training hyperparameters.

```rust
pub struct TrainingConfig {
    pub learning_rate: f32,
    pub batch_size: usize,
    pub epochs: usize,
    pub validation_split: f32,
    pub early_stopping_patience: Option<usize>,
    pub lr_decay: f32,
    pub lr_decay_step: usize,
    pub grad_clip: f32,
    pub adam_beta1: f32,
    pub adam_beta2: f32,
    pub adam_epsilon: f32,
    pub l2_reg: f32,
    pub enable_distillation: bool,
    pub distillation_temperature: f32,
    pub distillation_alpha: f32,
}
```

**Default values:**
- `learning_rate`: 0.001
- `batch_size`: 32
- `epochs`: 100
- `validation_split`: 0.2
- `early_stopping_patience`: Some(10)
- `lr_decay`: 0.5
- `lr_decay_step`: 20
- `grad_clip`: 5.0
- `adam_beta1`: 0.9
- `adam_beta2`: 0.999
- `adam_epsilon`: 1e-8
- `l2_reg`: 1e-5
- `enable_distillation`: false
- `distillation_temperature`: 3.0
- `distillation_alpha`: 0.5

### TrainingDataset

Training dataset with features and labels.

```rust
pub struct TrainingDataset {
    pub features: Vec<Vec<f32>>,
    pub labels: Vec<f32>,
    pub soft_targets: Option<Vec<f32>>,
}
```

**Methods:**

#### `new`
```rust
pub fn new(features: Vec<Vec<f32>>, labels: Vec<f32>) -> Result<Self>
```
Create a new training dataset.

**Parameters:**
- `features`: Input features (N × input_dim)
- `labels`: Target labels (N)

**Returns:** Result<TrainingDataset>

**Errors:**
- Returns error if features and labels have different lengths
- Returns error if dataset is empty

**Example:**
```rust
let features = vec![
    vec![0.8, 0.9, 0.7, 0.85, 0.2],
    vec![0.3, 0.2, 0.4, 0.35, 0.9],
];
let labels = vec![1.0, 0.0];
let dataset = TrainingDataset::new(features, labels)?;
```

#### `with_soft_targets`
```rust
pub fn with_soft_targets(self, soft_targets: Vec<f32>) -> Result<Self>
```
Add soft targets from teacher model for knowledge distillation.

**Parameters:**
- `soft_targets`: Soft predictions from teacher model (N)

**Returns:** Result<TrainingDataset>

**Example:**
```rust
let soft_targets = generate_teacher_predictions(&teacher, &features, 3.0)?;
let dataset = dataset.with_soft_targets(soft_targets)?;
```

#### `split`
```rust
pub fn split(&self, val_ratio: f32) -> Result<(Self, Self)>
```
Split dataset into train and validation sets.

**Parameters:**
- `val_ratio`: Validation set ratio (0.0 to 1.0)

**Returns:** Result<(train_dataset, val_dataset)>

**Example:**
```rust
let (train, val) = dataset.split(0.2)?; // 80% train, 20% val
```

#### `normalize`
```rust
pub fn normalize(&mut self) -> Result<(Vec<f32>, Vec<f32>)>
```
Normalize features using z-score normalization.

**Returns:** Result<(means, stds)>

**Example:**
```rust
let (means, stds) = dataset.normalize()?;
// Save for inference
save_normalization_params("norm.json", &means, &stds)?;
```

#### `len`
```rust
pub fn len(&self) -> usize
```
Get number of samples in dataset.

#### `is_empty`
```rust
pub fn is_empty(&self) -> bool
```
Check if dataset is empty.

### BatchIterator

Iterator for mini-batch training.

```rust
pub struct BatchIterator<'a> {
    // Private fields
}
```

**Methods:**

#### `new`
```rust
pub fn new(dataset: &'a TrainingDataset, batch_size: usize, shuffle: bool) -> Self
```
Create a new batch iterator.

**Parameters:**
- `dataset`: Reference to training dataset
- `batch_size`: Size of each batch
- `shuffle`: Whether to shuffle data

**Example:**
```rust
let batch_iter = BatchIterator::new(&dataset, 32, true);
for (features, labels, soft_targets) in batch_iter {
    // Train on batch
}
```

### TrainingMetrics

Metrics recorded during training.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingMetrics {
    pub epoch: usize,
    pub train_loss: f32,
    pub val_loss: f32,
    pub train_accuracy: f32,
    pub val_accuracy: f32,
    pub learning_rate: f32,
}
```

### Trainer

Main trainer for FastGRNN models.

```rust
pub struct Trainer {
    // Private fields
}
```

**Methods:**

#### `new`
```rust
pub fn new(model_config: &FastGRNNConfig, config: TrainingConfig) -> Self
```
Create a new trainer.

**Parameters:**
- `model_config`: Model configuration
- `config`: Training configuration

**Example:**
```rust
let trainer = Trainer::new(&model_config, training_config);
```

#### `train`
```rust
pub fn train(
    &mut self,
    model: &mut FastGRNN,
    dataset: &TrainingDataset,
) -> Result<Vec<TrainingMetrics>>
```
Train the model on the dataset.

**Parameters:**
- `model`: Mutable reference to the model
- `dataset`: Training dataset

**Returns:** Result<Vec<TrainingMetrics>> - Metrics for each epoch

**Example:**
```rust
let metrics = trainer.train(&mut model, &dataset)?;

// Print results
for m in &metrics {
    println!("Epoch {}: val_loss={:.4}, val_acc={:.2}%",
             m.epoch, m.val_loss, m.val_accuracy * 100.0);
}
```

#### `metrics_history`
```rust
pub fn metrics_history(&self) -> &[TrainingMetrics]
```
Get training metrics history.

**Returns:** Slice of training metrics

#### `save_metrics`
```rust
pub fn save_metrics<P: AsRef<Path>>(&self, path: P) -> Result<()>
```
Save training metrics to JSON file.

**Parameters:**
- `path`: Output file path

**Example:**
```rust
trainer.save_metrics("models/metrics.json")?;
```

## Functions

### binary_cross_entropy
```rust
fn binary_cross_entropy(prediction: f32, target: f32) -> f32
```
Compute binary cross-entropy loss.

**Formula:**
```
BCE = -target * log(pred) - (1 - target) * log(1 - pred)
```

**Parameters:**
- `prediction`: Model prediction (0.0 to 1.0)
- `target`: True label (0.0 or 1.0)

**Returns:** Loss value

### temperature_softmax
```rust
pub fn temperature_softmax(logit: f32, temperature: f32) -> f32
```
Temperature-scaled sigmoid for knowledge distillation.

**Parameters:**
- `logit`: Model output logit
- `temperature`: Temperature scaling factor (> 1.0 = softer)

**Returns:** Temperature-scaled probability

**Example:**
```rust
let soft_pred = temperature_softmax(logit, 3.0);
```

### generate_teacher_predictions
```rust
pub fn generate_teacher_predictions(
    teacher: &FastGRNN,
    features: &[Vec<f32>],
    temperature: f32,
) -> Result<Vec<f32>>
```
Generate soft predictions from teacher model.

**Parameters:**
- `teacher`: Teacher model
- `features`: Input features
- `temperature`: Temperature for softening

**Returns:** Result<Vec<f32>> - Soft predictions

**Example:**
```rust
let teacher = FastGRNN::load("teacher.safetensors")?;
let soft_targets = generate_teacher_predictions(&teacher, &features, 3.0)?;
```

## Usage Examples

### Basic Training

```rust
use ruvector_tiny_dancer_core::{
    model::{FastGRNN, FastGRNNConfig},
    training::{TrainingConfig, TrainingDataset, Trainer},
};

// Prepare data
let features = vec![/* ... */];
let labels = vec![/* ... */];
let mut dataset = TrainingDataset::new(features, labels)?;
dataset.normalize()?;

// Configure
let model_config = FastGRNNConfig::default();
let training_config = TrainingConfig::default();

// Train
let mut model = FastGRNN::new(model_config.clone())?;
let mut trainer = Trainer::new(&model_config, training_config);
let metrics = trainer.train(&mut model, &dataset)?;

// Save
model.save("model.safetensors")?;
```

### Knowledge Distillation

```rust
use ruvector_tiny_dancer_core::training::generate_teacher_predictions;

// Load teacher
let teacher = FastGRNN::load("teacher.safetensors")?;

// Generate soft targets
let temperature = 3.0;
let soft_targets = generate_teacher_predictions(&teacher, &features, temperature)?;

// Add to dataset
let dataset = dataset.with_soft_targets(soft_targets)?;

// Configure distillation
let training_config = TrainingConfig {
    enable_distillation: true,
    distillation_temperature: temperature,
    distillation_alpha: 0.7,
    ..Default::default()
};

// Train with distillation
let mut trainer = Trainer::new(&model_config, training_config);
trainer.train(&mut model, &dataset)?;
```

### Custom Training Loop

```rust
use ruvector_tiny_dancer_core::training::BatchIterator;

for epoch in 0..50 {
    let mut epoch_loss = 0.0;
    let mut n_batches = 0;

    let batch_iter = BatchIterator::new(&train_dataset, 32, true);
    for (features, labels, soft_targets) in batch_iter {
        // Your training logic here
        epoch_loss += train_batch(&mut model, &features, &labels);
        n_batches += 1;
    }

    let avg_loss = epoch_loss / n_batches as f32;
    println!("Epoch {}: loss={:.4}", epoch, avg_loss);
}
```

### Progressive Training

```rust
// Start with high LR
let mut config = TrainingConfig {
    learning_rate: 0.1,
    epochs: 20,
    ..Default::default()
};

let mut trainer = Trainer::new(&model_config, config.clone());
trainer.train(&mut model, &dataset)?;

// Continue with lower LR
config.learning_rate = 0.01;
config.epochs = 30;

let mut trainer2 = Trainer::new(&model_config, config);
trainer2.train(&mut model, &dataset)?;
```

## Error Handling

All training functions return `Result<T>` with `TinyDancerError`:

```rust
match trainer.train(&mut model, &dataset) {
    Ok(metrics) => {
        println!("Training successful!");
        println!("Final accuracy: {:.2}%",
                 metrics.last().unwrap().val_accuracy * 100.0);
    }
    Err(e) => {
        eprintln!("Training failed: {}", e);
        // Handle error appropriately
    }
}
```

Common errors:
- `InvalidInput`: Invalid dataset, configuration, or parameters
- `SerializationError`: Failed to save/load files
- `IoError`: File I/O errors

## Performance Considerations

### Memory Usage

- **Dataset**: O(N × input_dim) floats
- **Model**: ~850 parameters for default config (16 hidden units)
- **Optimizer**: 2× model size (Adam momentum)

For large datasets (>100K samples), consider:
- Batch processing
- Data streaming
- Memory-mapped files

### Training Speed

Typical training times (CPU):
- Small dataset (1K samples): ~10 seconds
- Medium dataset (10K samples): ~1-2 minutes
- Large dataset (100K samples): ~10-20 minutes

Optimization tips:
- Use larger batch sizes (32-128)
- Enable early stopping
- Use knowledge distillation for faster convergence

### Reproducibility

For reproducible results:
1. Set random seed before training
2. Use deterministic operations
3. Save normalization parameters
4. Version control all hyperparameters

```rust
// Set seed (note: full reproducibility requires more work)
use rand::SeedableRng;
let mut rng = rand::rngs::StdRng::seed_from_u64(42);
```

## See Also

- [Training Guide](./training-guide.md) - Complete training walkthrough
- [Model API](../src/model.rs) - FastGRNN model implementation
- [Examples](../examples/train-model.rs) - Working code examples
