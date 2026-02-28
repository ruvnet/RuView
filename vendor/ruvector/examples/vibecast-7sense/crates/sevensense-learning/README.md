# sevensense-learning

[![Crate](https://img.shields.io/badge/crates.io-sevensense--learning-orange.svg)](https://crates.io/crates/sevensense-learning)
[![Docs](https://img.shields.io/badge/docs-sevensense--learning-blue.svg)](https://docs.rs/sevensense-learning)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

> Graph Neural Network (GNN) learning for bioacoustic pattern discovery.

**sevensense-learning** implements online learning algorithms that discover patterns in bird vocalizations over time. Using Graph Neural Networks with Elastic Weight Consolidation (EWC), it learns species-specific call patterns, dialect variations, and behavioral signatures without forgetting previously learned knowledge.

## Features

- **GNN Architecture**: Graph-based learning on similarity networks
- **EWC Regularization**: Prevents catastrophic forgetting in online learning
- **Online Updates**: Continuous learning from streaming data
- **Transition Graphs**: Model sequential call patterns
- **Fisher Information**: Importance-weighted parameter updates
- **Gradient Checkpointing**: Memory-efficient training

## Use Cases

| Use Case | Description | Key Functions |
|----------|-------------|---------------|
| Pattern Learning | Learn call patterns | `train()`, `learn_patterns()` |
| Online Updates | Incremental learning | `online_update()` |
| Transition Modeling | Sequential patterns | `TransitionGraph::learn()` |
| EWC Training | Continual learning | `ewc_train()` |
| Inference | Pattern prediction | `predict()`, `infer()` |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sevensense-learning = "0.1"
```

## Quick Start

```rust
use sevensense_learning::{GnnModel, GnnConfig, TransitionGraph};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create GNN model
    let config = GnnConfig {
        hidden_dim: 256,
        num_layers: 3,
        dropout: 0.1,
        ..Default::default()
    };
    let mut model = GnnModel::new(config);

    // Build transition graph from embeddings
    let graph = TransitionGraph::from_embeddings(&embeddings, 0.8)?;

    // Train the model
    model.train(&graph, &embeddings, 100)?;  // 100 epochs

    // Make predictions
    let prediction = model.predict(&query_embedding)?;
    println!("Predicted pattern: {:?}", prediction);

    Ok(())
}
```

---

<details>
<summary><b>Tutorial: Building Transition Graphs</b></summary>

### Creating from Embeddings

```rust
use sevensense_learning::{TransitionGraph, GraphConfig};

// Config for graph construction
let config = GraphConfig {
    similarity_threshold: 0.8,   // Edge threshold
    max_neighbors: 10,           // Max edges per node
    temporal_decay: 0.95,        // Time-based edge weighting
};

// Build graph from embeddings with timestamps
let graph = TransitionGraph::new(config);
for (id, embedding, timestamp) in recordings.iter() {
    graph.add_node(*id, embedding, *timestamp)?;
}

// Automatically computes edges based on similarity
graph.build_edges()?;

println!("Graph has {} nodes and {} edges",
    graph.node_count(),
    graph.edge_count());
```

### Analyzing Graph Structure

```rust
use sevensense_learning::TransitionGraph;

// Get neighbors for a node
let neighbors = graph.neighbors(node_id)?;
for (neighbor_id, weight) in neighbors {
    println!("Neighbor {}: weight {:.3}", neighbor_id, weight);
}

// Compute graph statistics
let stats = graph.statistics();
println!("Average degree: {:.2}", stats.avg_degree);
println!("Clustering coefficient: {:.3}", stats.clustering_coeff);
println!("Connected components: {}", stats.num_components);
```

### Sequential Patterns

```rust
// Analyze sequential call patterns
let sequences = graph.find_sequences(min_length: 3)?;

for seq in sequences {
    println!("Sequence: {:?}", seq.node_ids);
    println!("  Frequency: {}", seq.count);
    println!("  Avg interval: {:.2}s", seq.avg_interval);
}
```

</details>

<details>
<summary><b>Tutorial: GNN Training</b></summary>

### Basic Training

```rust
use sevensense_learning::{GnnModel, GnnConfig, TrainingConfig};

let model_config = GnnConfig {
    input_dim: 1536,           // Embedding dimension
    hidden_dim: 256,           // Hidden layer size
    output_dim: 64,            // Output embedding size
    num_layers: 3,             // GNN layers
    dropout: 0.1,
};

let mut model = GnnModel::new(model_config);

let train_config = TrainingConfig {
    epochs: 100,
    learning_rate: 0.001,
    batch_size: 32,
    early_stopping: Some(10),  // Stop if no improvement for 10 epochs
};

// Train on graph
let history = model.train(&graph, &features, train_config)?;

println!("Final loss: {:.4}", history.final_loss);
println!("Best epoch: {}", history.best_epoch);
```

### Training with Validation

```rust
let (train_graph, val_graph) = split_graph(&graph, 0.8)?;

let history = model.train_with_validation(
    &train_graph,
    &val_graph,
    &features,
    train_config,
)?;

// Plot training curves
for (epoch, train_loss, val_loss) in history.iter() {
    println!("Epoch {}: train={:.4}, val={:.4}", epoch, train_loss, val_loss);
}
```

### Custom Loss Functions

```rust
use sevensense_learning::{GnnModel, LossFunction};

// Contrastive loss for similarity learning
let loss_fn = LossFunction::Contrastive {
    margin: 0.5,
    positive_weight: 1.0,
    negative_weight: 0.5,
};

model.set_loss_function(loss_fn);
model.train(&graph, &features, config)?;
```

</details>

<details>
<summary><b>Tutorial: Elastic Weight Consolidation (EWC)</b></summary>

### Why EWC?

Standard neural networks suffer from "catastrophic forgetting"—learning new patterns erases old ones. EWC prevents this by protecting important parameters.

### EWC Training

```rust
use sevensense_learning::{GnnModel, EwcConfig};

let ewc_config = EwcConfig {
    lambda: 1000.0,            // Regularization strength
    fisher_samples: 200,       // Samples for Fisher estimation
    online: true,              // Online EWC variant
};

let mut model = GnnModel::new(model_config);

// Train on first dataset
model.train(&graph1, &features1, train_config)?;

// Compute Fisher information (importance weights)
model.compute_fisher(&graph1, &features1, ewc_config.fisher_samples)?;

// Train on second dataset with EWC
model.ewc_train(&graph2, &features2, train_config, ewc_config)?;

// Model remembers patterns from both datasets!
```

### Continual Learning Pipeline

```rust
use sevensense_learning::{ContinualLearner, EwcConfig};

let mut learner = ContinualLearner::new(model, EwcConfig::default());

// Learn from streaming data batches
for batch in data_stream {
    let graph = TransitionGraph::from_batch(&batch)?;
    learner.learn(&graph, &batch.features)?;

    println!("Learned batch {}, total patterns: {}",
        batch.id, learner.pattern_count());
}

// Test on all historical patterns
let recall = learner.evaluate_recall(&all_test_data)?;
println!("Recall on all patterns: {:.2}%", recall * 100.0);
```

</details>

<details>
<summary><b>Tutorial: Online Learning</b></summary>

### Incremental Updates

```rust
use sevensense_learning::{GnnModel, OnlineConfig};

let online_config = OnlineConfig {
    learning_rate: 0.0001,     // Lower LR for stability
    momentum: 0.9,
    max_updates_per_sample: 5,
    replay_buffer_size: 1000,
};

let mut model = GnnModel::new(model_config);
model.enable_online_learning(online_config);

// Process streaming data
for sample in stream {
    // Single-sample update
    model.online_update(&sample.embedding, &sample.label)?;

    if model.updates_count() % 100 == 0 {
        println!("Processed {} samples", model.updates_count());
    }
}
```

### Experience Replay

```rust
use sevensense_learning::{ReplayBuffer, GnnModel};

let mut buffer = ReplayBuffer::new(1000);  // Store 1000 samples
let mut model = GnnModel::new(config);

for sample in stream {
    // Add to replay buffer
    buffer.add(sample.clone());

    // Train on current sample + replay
    let replay_batch = buffer.sample(32)?;  // 32 random historical samples
    let batch = [vec![sample], replay_batch].concat();

    model.train_batch(&batch)?;
}
```

</details>

<details>
<summary><b>Tutorial: Pattern Prediction</b></summary>

### Predicting Similar Patterns

```rust
use sevensense_learning::GnnModel;

let model = GnnModel::load("trained_model.bin")?;

// Get learned representation
let embedding = model.encode(&query_features)?;

// Find similar learned patterns
let similar = model.find_similar(&embedding, 10)?;

for (pattern_id, similarity) in similar {
    println!("Pattern {}: {:.3} similarity", pattern_id, similarity);
}
```

### Predicting Next Call

```rust
// Given a sequence of calls, predict the next one
let sequence = vec![embedding1, embedding2, embedding3];
let prediction = model.predict_next(&sequence)?;

println!("Predicted next call embedding: {:?}", prediction.embedding);
println!("Confidence: {:.3}", prediction.confidence);
```

### Anomaly Detection

```rust
use sevensense_learning::{GnnModel, AnomalyDetector};

let detector = AnomalyDetector::new(&model);

for embedding in embeddings {
    let score = detector.anomaly_score(&embedding)?;

    if score > 0.95 {
        println!("Anomaly detected! Score: {:.3}", score);
    }
}
```

</details>

---

## Configuration

### GnnConfig Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `input_dim` | 1536 | Input embedding dimension |
| `hidden_dim` | 256 | Hidden layer dimension |
| `output_dim` | 64 | Output dimension |
| `num_layers` | 3 | Number of GNN layers |
| `dropout` | 0.1 | Dropout rate |

### EwcConfig Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `lambda` | 1000.0 | Regularization strength |
| `fisher_samples` | 200 | Samples for Fisher estimation |
| `online` | true | Use online EWC variant |

## Architecture

```
Input Embeddings (1536-dim)
         │
         ▼
    ┌─────────┐
    │  GNN    │ ◄── Graph structure (adjacency)
    │ Layer 1 │
    └────┬────┘
         │
    ┌────▼────┐
    │  GNN    │
    │ Layer 2 │
    └────┬────┘
         │
    ┌────▼────┐
    │  GNN    │
    │ Layer 3 │
    └────┬────┘
         │
         ▼
  Output Embeddings (64-dim)
```

## Links

- **Homepage**: [ruv.io](https://ruv.io)
- **Repository**: [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **Crates.io**: [crates.io/crates/sevensense-learning](https://crates.io/crates/sevensense-learning)
- **Documentation**: [docs.rs/sevensense-learning](https://docs.rs/sevensense-learning)

## License

MIT License - see [LICENSE](../../LICENSE) for details.

---

*Part of the [7sense Bioacoustic Intelligence Platform](https://ruv.io) by rUv*
