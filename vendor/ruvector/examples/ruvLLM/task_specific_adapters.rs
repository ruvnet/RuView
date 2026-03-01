//! Task-Specific LoRA Adapters Example
//!
//! This example demonstrates:
//! 1. Using pre-defined adapters for different agent types
//! 2. Training adapters from synthetic datasets
//! 3. Merging multiple adapters
//! 4. Hot-swapping adapters at runtime
//!
//! Run with:
//! ```bash
//! cargo run --example task_specific_adapters --features ruvllm
//! ```

use ruvllm::lora::{
    RuvLtraAdapters, AdapterTrainer, AdapterTrainingConfig, SyntheticDataGenerator,
    AdapterMerger, MergeConfig, MergeStrategy, HotSwapManager, AdaptFeedback,
};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Task-Specific LoRA Adapters Demo\n");

    // 1. Explore available adapters
    println!("📋 Available Adapters:");
    println!("═══════════════════════\n");

    let adapters = RuvLtraAdapters::new();
    for name in adapters.list_names() {
        if let Some(config) = adapters.get(&name) {
            println!("  🔧 {}", name);
            println!("     Description: {}", config.description);
            println!("     Rank: {}, Alpha: {}", config.rank, config.alpha);
            println!("     Target modules: {} modules", config.target_modules.len());
            println!("     Memory (768d): {:.2} KB", config.estimate_memory(768) as f32 / 1024.0);
            println!("     Tags: {}", config.domain_tags.join(", "));
            println!();
        }
    }

    // 2. Create and train adapters
    println!("\n🎓 Training Adapters");
    println!("═══════════════════════\n");

    let hidden_dim = 768;
    let generator = SyntheticDataGenerator::new(hidden_dim, 42);

    // Train coder adapter
    println!("  Training 'coder' adapter...");
    let coder_dataset = generator.generate("coder", 1000);
    println!("    Dataset: {} train, {} val examples",
             coder_dataset.examples.len(),
             coder_dataset.validation.len());

    let coder_lora = adapters.create_lora("coder", hidden_dim)?;
    let mut coder_trainer = AdapterTrainer::new(AdapterTrainingConfig::quick());

    let coder_result = coder_trainer.train(&coder_lora, &coder_dataset)?;
    println!("    ✓ Completed {} epochs in {} steps",
             coder_result.epochs_completed,
             coder_result.total_steps);
    println!("    Final loss: {:.4}", coder_result.final_loss);

    // Train security adapter
    println!("\n  Training 'security' adapter...");
    let security_dataset = generator.generate("security", 1000);
    let security_lora = adapters.create_lora("security", hidden_dim)?;
    let mut security_trainer = AdapterTrainer::new(AdapterTrainingConfig::quick());

    let security_result = security_trainer.train(&security_lora, &security_dataset)?;
    println!("    ✓ Completed {} epochs in {} steps",
             security_result.epochs_completed,
             security_result.total_steps);

    // 3. Use adapters for inference
    println!("\n\n🔮 Adapter Inference");
    println!("═══════════════════════\n");

    let test_input = vec![0.5; hidden_dim];

    println!("  Coder adapter output:");
    let coder_output = coder_lora.forward(&test_input, &ruvllm::lora::TargetModule::QProj);
    println!("    Output dim: {}", coder_output.len());
    println!("    Mean activation: {:.4}", coder_output.iter().sum::<f32>() / coder_output.len() as f32);

    println!("\n  Security adapter output:");
    let security_output = security_lora.forward(&test_input, &ruvllm::lora::TargetModule::QProj);
    println!("    Output dim: {}", security_output.len());
    println!("    Mean activation: {:.4}", security_output.iter().sum::<f32>() / security_output.len() as f32);

    // 4. Merge adapters
    println!("\n\n🔀 Adapter Merging");
    println!("═══════════════════════\n");

    // Average merge
    println!("  Average merge (coder + security):");
    let merge_config = MergeConfig::average();
    let merger = AdapterMerger::new(merge_config);

    let adapters_to_merge = vec![
        ("coder".to_string(), coder_lora.clone()),
        ("security".to_string(), security_lora.clone()),
    ];

    let merged = merger.merge(&adapters_to_merge, &adapters.coder, hidden_dim)?;
    let merged_output = merged.forward(&test_input, &ruvllm::lora::TargetModule::QProj);
    println!("    Mean activation: {:.4}", merged_output.iter().sum::<f32>() / merged_output.len() as f32);

    // Weighted merge
    println!("\n  Weighted merge (70% coder, 30% security):");
    let mut weights = HashMap::new();
    weights.insert("coder".to_string(), 0.7);
    weights.insert("security".to_string(), 0.3);

    let weighted_config = MergeConfig::weighted(weights);
    let weighted_merger = AdapterMerger::new(weighted_config);
    let weighted_merged = weighted_merger.merge(&adapters_to_merge, &adapters.coder, hidden_dim)?;
    let weighted_output = weighted_merged.forward(&test_input, &ruvllm::lora::TargetModule::QProj);
    println!("    Mean activation: {:.4}", weighted_output.iter().sum::<f32>() / weighted_output.len() as f32);

    // SLERP interpolation
    println!("\n  SLERP interpolation (t=0.5):");
    let slerp_config = MergeConfig::slerp(0.5);
    let slerp_merger = AdapterMerger::new(slerp_config);
    let slerp_merged = slerp_merger.merge(&adapters_to_merge, &adapters.coder, hidden_dim)?;
    let slerp_output = slerp_merged.forward(&test_input, &ruvllm::lora::TargetModule::QProj);
    println!("    Mean activation: {:.4}", slerp_output.iter().sum::<f32>() / slerp_output.len() as f32);

    // 5. Hot-swapping demonstration
    println!("\n\n🔄 Hot-Swap Demo");
    println!("═══════════════════════\n");

    let mut swap_manager = HotSwapManager::new();

    println!("  Setting coder as active adapter...");
    swap_manager.set_active(coder_lora.clone());

    if let Some(active) = swap_manager.active() {
        let output = active.forward(&test_input, &ruvllm::lora::TargetModule::QProj);
        println!("    Active adapter mean: {:.4}", output.iter().sum::<f32>() / output.len() as f32);
    }

    println!("\n  Preparing security adapter in standby...");
    swap_manager.prepare_standby(security_lora.clone());

    println!("  Performing hot-swap...");
    swap_manager.swap()?;

    if let Some(active) = swap_manager.active() {
        let output = active.forward(&test_input, &ruvllm::lora::TargetModule::QProj);
        println!("    New active adapter mean: {:.4}", output.iter().sum::<f32>() / output.len() as f32);
    }

    // 6. Adapter composition (multi-task)
    println!("\n\n🧩 Multi-Task Composition");
    println!("═══════════════════════\n");

    println!("  Creating researcher adapter...");
    let researcher_dataset = generator.generate("researcher", 1000);
    let researcher_lora = adapters.create_lora("researcher", hidden_dim)?;
    let mut researcher_trainer = AdapterTrainer::new(AdapterTrainingConfig::quick());
    researcher_trainer.train(&researcher_lora, &researcher_dataset)?;

    println!("\n  TIES merge (coder + security + researcher):");
    let ties_adapters = vec![
        ("coder".to_string(), coder_lora.clone()),
        ("security".to_string(), security_lora.clone()),
        ("researcher".to_string(), researcher_lora.clone()),
    ];

    let ties_config = MergeConfig::ties(0.6);
    let ties_merger = AdapterMerger::new(ties_config);
    let ties_merged = ties_merger.merge(&ties_adapters, &adapters.coder, hidden_dim)?;
    let ties_output = ties_merged.forward(&test_input, &ruvllm::lora::TargetModule::QProj);
    println!("    Mean activation: {:.4}", ties_output.iter().sum::<f32>() / ties_output.len() as f32);

    // 7. Per-request adaptation
    println!("\n\n⚡ Per-Request Adaptation");
    println!("═══════════════════════\n");

    println!("  Baseline output:");
    let baseline = coder_lora.forward(&test_input, &ruvllm::lora::TargetModule::QProj);
    println!("    Mean: {:.4}", baseline.iter().sum::<f32>() / baseline.len() as f32);

    println!("\n  Adapting with high-quality feedback...");
    let feedback = AdaptFeedback::from_quality(0.95);
    coder_lora.adapt(&test_input, feedback)?;
    coder_lora.apply_updates(0.01);

    let adapted = coder_lora.forward(&test_input, &ruvllm::lora::TargetModule::QProj);
    println!("    Mean after adaptation: {:.4}", adapted.iter().sum::<f32>() / adapted.len() as f32);
    println!("    Change: {:.4}",
             (adapted.iter().sum::<f32>() - baseline.iter().sum::<f32>()) / baseline.len() as f32);

    // 8. Save and load adapters
    println!("\n\n💾 Persistence");
    println!("═══════════════════════\n");

    let save_path = "/tmp/coder_adapter.bin";
    println!("  Saving coder adapter to {}...", save_path);
    coder_lora.save(save_path)?;
    println!("    ✓ Saved");

    println!("\n  Loading adapter...");
    let loaded_lora = ruvllm::lora::MicroLoRA::load(save_path)?;
    println!("    ✓ Loaded");
    println!("    Params: {}", loaded_lora.param_count());
    println!("    Memory: {:.2} KB", loaded_lora.memory_bytes() as f32 / 1024.0);

    // 9. Performance summary
    println!("\n\n📊 Performance Summary");
    println!("═══════════════════════\n");

    println!("  Coder Adapter:");
    println!("    Rank: {}", adapters.coder.rank);
    println!("    Parameters: {}", coder_lora.param_count());
    println!("    Memory: {:.2} KB", coder_lora.memory_bytes() as f32 / 1024.0);
    println!("    Forward passes: {}", coder_lora.forward_count());
    println!("    Adaptations: {}", coder_lora.adaptation_count());

    println!("\n  Security Adapter:");
    println!("    Rank: {}", adapters.security.rank);
    println!("    Parameters: {}", security_lora.param_count());
    println!("    Memory: {:.2} KB", security_lora.memory_bytes() as f32 / 1024.0);

    println!("\n✨ Demo Complete!\n");

    Ok(())
}
