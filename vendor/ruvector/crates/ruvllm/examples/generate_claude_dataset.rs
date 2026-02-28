#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! # Claude Task Dataset Generation Example
//!
//! This example demonstrates how to generate a comprehensive fine-tuning dataset
//! for RuvLTRA models trained on Claude Flow agent tasks.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --example generate_claude_dataset --release
//! ```
//!
//! This will generate:
//! - `claude_training_full.jsonl` - Full dataset in JSONL format
//! - `claude_training_train.jsonl` - Training split (70%)
//! - `claude_training_val.jsonl` - Validation split (15%)
//! - `claude_training_test.jsonl` - Test split (15%)
//! - `claude_training_stats.json` - Dataset statistics

use ruvllm::training::{
    AugmentationConfig, ClaudeTaskDataset, DatasetConfig, DatasetGenerator, TaskCategory,
};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    println!("🚀 Claude Task Dataset Generator");
    println!("═══════════════════════════════════════════════════\n");

    // Configure dataset generation
    let config = DatasetConfig {
        examples_per_category: 100,
        enable_augmentation: true,
        augmentation: AugmentationConfig {
            paraphrases_per_example: 2,
            complexity_variations: 2,
            enable_domain_transfer: true,
        },
        seed: 42,
    };

    println!("📋 Configuration:");
    println!(
        "  • Examples per category: {}",
        config.examples_per_category
    );
    println!("  • Augmentation enabled: {}", config.enable_augmentation);
    println!(
        "  • Paraphrases per example: {}",
        config.augmentation.paraphrases_per_example
    );
    println!(
        "  • Complexity variations: {}",
        config.augmentation.complexity_variations
    );
    println!(
        "  • Domain transfer: {}\n",
        config.augmentation.enable_domain_transfer
    );

    // Generate dataset
    println!("⚙️  Generating dataset...");
    let mut generator = DatasetGenerator::new(config);
    let dataset = generator.generate();

    println!("✅ Dataset generated!\n");

    // Print statistics
    print_statistics(&dataset);

    // Export full dataset
    println!("\n💾 Exporting datasets...");

    dataset.export_jsonl("claude_training_full.jsonl")?;
    println!(
        "  ✓ Full dataset: claude_training_full.jsonl ({} examples)",
        dataset.examples.len()
    );

    dataset.export_json("claude_training_full.json")?;
    println!("  ✓ Full dataset JSON: claude_training_full.json");

    // Split and export
    let (train, val, test) = dataset.split(0.7, 0.15, 0.15, 42);

    let train_dataset = ClaudeTaskDataset::new(train);
    train_dataset.export_jsonl("claude_training_train.jsonl")?;
    println!(
        "  ✓ Training set: claude_training_train.jsonl ({} examples)",
        train_dataset.examples.len()
    );

    let val_dataset = ClaudeTaskDataset::new(val);
    val_dataset.export_jsonl("claude_training_val.jsonl")?;
    println!(
        "  ✓ Validation set: claude_training_val.jsonl ({} examples)",
        val_dataset.examples.len()
    );

    let test_dataset = ClaudeTaskDataset::new(test);
    test_dataset.export_jsonl("claude_training_test.jsonl")?;
    println!(
        "  ✓ Test set: claude_training_test.jsonl ({} examples)",
        test_dataset.examples.len()
    );

    // Export statistics
    dataset.export_stats("claude_training_stats.json")?;
    println!("  ✓ Statistics: claude_training_stats.json\n");

    // Print sample examples
    print_sample_examples(&dataset);

    // Print model routing analysis
    print_model_routing_analysis(&dataset);

    println!("\n✨ Dataset generation complete!");
    println!("   Total examples: {}", dataset.examples.len());
    println!("   Ready for fine-tuning RuvLTRA models\n");

    Ok(())
}

fn print_statistics(dataset: &ClaudeTaskDataset) {
    println!("📊 Dataset Statistics:");
    println!("  ═══════════════════════════════════════════════════");
    println!("  Total examples: {}", dataset.stats.total_examples);
    println!(
        "  Average quality score: {:.2}",
        dataset.stats.avg_quality_score
    );

    println!("\n  📂 Examples by Category:");
    for category in TaskCategory::all() {
        let count = dataset
            .stats
            .examples_per_category
            .get(category.name())
            .unwrap_or(&0);
        let percentage = (*count as f32 / dataset.stats.total_examples as f32) * 100.0;
        println!(
            "    • {:12} {:4} ({:5.1}%)",
            category.name(),
            count,
            percentage
        );
    }

    println!("\n  📈 Examples by Complexity:");
    for (complexity, count) in &dataset.stats.examples_per_complexity {
        let percentage = (*count as f32 / dataset.stats.total_examples as f32) * 100.0;
        println!("    • {:12} {:4} ({:5.1}%)", complexity, count, percentage);
    }

    println!("\n  🏷️  Examples by Domain:");
    for (domain, count) in &dataset.stats.examples_per_domain {
        let percentage = (*count as f32 / dataset.stats.total_examples as f32) * 100.0;
        println!("    • {:12} {:4} ({:5.1}%)", domain, count, percentage);
    }
}

fn print_sample_examples(dataset: &ClaudeTaskDataset) {
    println!("📝 Sample Examples:");
    println!("  ═══════════════════════════════════════════════════");

    for category in TaskCategory::all() {
        let sample = dataset
            .examples
            .iter()
            .find(|e| e.metadata.category == category);

        if let Some(example) = sample {
            println!(
                "\n  🔹 {} ({})",
                category.name(),
                example.metadata.expected_model
            );
            println!(
                "     Complexity: {:?}, Domain: {:?}",
                example.metadata.complexity, example.metadata.domain
            );
            println!("     Input: {}", truncate(&example.input, 80));
            println!("     Context: {}", truncate(&example.context, 80));
            println!("     Quality: {:.2}", example.metadata.quality_score);
        }
    }
}

fn print_model_routing_analysis(dataset: &ClaudeTaskDataset) {
    println!("\n🎯 Model Routing Analysis:");
    println!("  ═══════════════════════════════════════════════════");

    let mut model_counts = std::collections::HashMap::new();
    for example in &dataset.examples {
        *model_counts
            .entry(&example.metadata.expected_model)
            .or_insert(0) += 1;
    }

    for (model, count) in model_counts.iter() {
        let percentage = (*count as f32 / dataset.stats.total_examples as f32) * 100.0;
        let cost_indicator = match model.as_str() {
            "haiku" => "💰 (cheapest)",
            "sonnet" => "💰💰 (balanced)",
            "opus" => "💰💰💰 (most capable)",
            _ => "",
        };
        println!(
            "  • {:8} {:4} ({:5.1}%) {}",
            model, count, percentage, cost_indicator
        );
    }

    println!("\n  ℹ️  Model Selection Guide:");
    println!("     • Haiku:  Simple tasks, fast responses, low cost");
    println!("     • Sonnet: Balanced complexity, moderate cost");
    println!("     • Opus:   Complex reasoning, highest quality");
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
