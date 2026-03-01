# Quick Start: Claude Task Dataset Generation

Generate fine-tuning datasets for RuvLTRA models in 5 minutes.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ruvllm = { version = "0.1.0", features = ["training"] }
```

## Basic Usage

### 1. Generate a Dataset

```rust
use ruvllm::training::{DatasetGenerator, DatasetConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create generator with default config
    let config = DatasetConfig::default();
    let mut generator = DatasetGenerator::new(config);

    // Generate dataset
    let dataset = generator.generate();

    println!("Generated {} examples", dataset.examples.len());

    Ok(())
}
```

### 2. Export to JSONL

```rust
// Export full dataset
dataset.export_jsonl("training.jsonl")?;

// Export statistics
dataset.export_stats("stats.json")?;
```

### 3. Create Train/Val/Test Splits

```rust
// 70% train, 15% validation, 15% test
let (train, val, test) = dataset.split(0.7, 0.15, 0.15, 42);

// Export each split
ClaudeTaskDataset::new(train).export_jsonl("train.jsonl")?;
ClaudeTaskDataset::new(val).export_jsonl("val.jsonl")?;
ClaudeTaskDataset::new(test).export_jsonl("test.jsonl")?;
```

## Run the Example

```bash
# Generate a complete dataset
cargo run --example generate_claude_dataset --release

# Output:
# - claude_training_full.jsonl (~2,700 examples)
# - claude_training_train.jsonl (70% split)
# - claude_training_val.jsonl (15% split)
# - claude_training_test.jsonl (15% split)
# - claude_training_stats.json (statistics)
```

## Custom Configuration

### Control Dataset Size

```rust
let config = DatasetConfig {
    examples_per_category: 200,  // 200 examples per category
    ..Default::default()
};
```

### Disable Augmentation

```rust
let config = DatasetConfig {
    examples_per_category: 100,
    enable_augmentation: false,  // No augmentation
    ..Default::default()
};
```

### Fine-Tune Augmentation

```rust
use ruvllm::training::AugmentationConfig;

let config = DatasetConfig {
    examples_per_category: 100,
    enable_augmentation: true,
    augmentation: AugmentationConfig {
        paraphrases_per_example: 3,      // 3 paraphrases
        complexity_variations: 2,         // 2 complexity levels
        enable_domain_transfer: true,    // Cross-domain transfer
    },
    seed: 42,  // For reproducibility
};
```

## Understanding the Data

### Dataset Structure

Each example contains:

```json
{
  "input": "Implement JWT authentication middleware in TypeScript",
  "context": "Should verify Bearer tokens, check expiration, validate RS256 signature",
  "output_agent": "coder",
  "metadata": {
    "category": "Coder",
    "complexity": "Moderate",
    "domain": "Web",
    "expected_model": "sonnet",
    "quality_score": 0.87,
    "tags": ["authentication", "middleware", "jwt"]
  }
}
```

### Task Categories

1. **Coder** (20%) - Code generation, debugging, refactoring
2. **Researcher** (20%) - Analysis, exploration, documentation
3. **Security** (20%) - Audits, vulnerabilities, compliance
4. **Architecture** (20%) - System design, planning
5. **Reviewer** (20%) - Code review, quality assessment

### Model Selection

The dataset includes intelligent routing:

- **Haiku**: Simple tasks (cheap, fast)
- **Sonnet**: Moderate complexity (balanced)
- **Opus**: Complex/security tasks (highest quality)

## Dataset Statistics

Default configuration generates:

```
Base examples:      500 (5 categories × 100)
Paraphrased:      1,000 (500 × 2)
Complexity varied:  800 (500 × 2, filtered)
Domain transfer:    400 (500 × 1, filtered)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Total:           ~2,700 examples
```

Category distribution:
```
Coder:        ~540 examples (20%)
Researcher:   ~540 examples (20%)
Security:     ~540 examples (20%)
Architecture: ~540 examples (20%)
Reviewer:     ~540 examples (20%)
```

Model distribution:
```
Haiku:   ~730 examples (27%) - Cost-effective
Sonnet: ~1,270 examples (47%) - Balanced
Opus:    ~700 examples (26%) - High-quality
```

## Inspect the Data

```rust
// Print first 5 examples
for (i, example) in dataset.examples.iter().take(5).enumerate() {
    println!("Example {}:", i + 1);
    println!("  Input: {}", example.input);
    println!("  Agent: {}", example.output_agent);
    println!("  Model: {}", example.metadata.expected_model);
    println!("  Quality: {:.2}\n", example.metadata.quality_score);
}
```

## Filter by Category

```rust
// Get all security tasks
let security_tasks: Vec<_> = dataset.examples
    .iter()
    .filter(|e| e.metadata.category == TaskCategory::Security)
    .collect();

println!("Security tasks: {}", security_tasks.len());
```

## Filter by Complexity

```rust
// Get all simple tasks
let simple_tasks: Vec<_> = dataset.examples
    .iter()
    .filter(|e| e.metadata.complexity == ComplexityLevel::Simple)
    .collect();

println!("Simple tasks: {}", simple_tasks.len());
```

## Next Steps

1. **Fine-tune a model**: Use the generated JSONL files with your favorite ML framework
2. **Customize templates**: Modify `claude_dataset.rs` to add domain-specific tasks
3. **Integrate with SONA**: Use RuvLLM's SONA learning for continuous improvement
4. **Deploy**: Use RuvLLM's serving engine for production inference

## Common Issues

### "Not enough examples"
Increase `examples_per_category`:
```rust
let config = DatasetConfig {
    examples_per_category: 500,  // Generate more
    ..Default::default()
};
```

### "Too much variation"
Disable augmentation:
```rust
let config = DatasetConfig {
    enable_augmentation: false,
    ..Default::default()
};
```

### "Need specific domain"
Filter after generation:
```rust
let web_tasks: Vec<_> = dataset.examples
    .iter()
    .filter(|e| e.metadata.domain == DomainType::Web)
    .cloned()
    .collect();

ClaudeTaskDataset::new(web_tasks).export_jsonl("web_tasks.jsonl")?;
```

## Resources

- **Full Documentation**: `../crates/ruvllm/src/training/README.md`
- **Format Spec**: `../docs/claude_dataset_format.md`
- **Example Code**: `../crates/ruvllm/examples/generate_claude_dataset.rs`
- **Tests**: `../crates/ruvllm/src/training/tests.rs`

## Support

- GitHub Issues: https://github.com/ruvector/issues
- Documentation: https://docs.ruvector.io
