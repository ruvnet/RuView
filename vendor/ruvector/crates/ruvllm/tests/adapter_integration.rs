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
//! Integration tests for task-specific LoRA adapters

#[cfg(test)]
mod tests {
    use ruvllm::lora::{
        AdaptFeedback, AdapterMerger, AdapterTrainer, AdapterTrainingConfig, HotSwapManager,
        MergeConfig, MergeStrategy, RuvLtraAdapters, SyntheticDataGenerator, TargetModule,
    };
    use std::collections::HashMap;

    #[test]
    fn test_adapter_creation_all() {
        let adapters = RuvLtraAdapters::new();

        // Test all 5 pre-defined adapters
        for name in &["coder", "researcher", "security", "architect", "reviewer"] {
            let lora = adapters.create_lora(name, 256).unwrap();
            assert!(lora.is_enabled());
            assert!(lora.param_count() > 0);
            println!("{}: {} params", name, lora.param_count());
        }
    }

    #[test]
    fn test_synthetic_data_generation() {
        let generator = SyntheticDataGenerator::new(256, 42);

        for task_type in &["coder", "researcher", "security", "architect", "reviewer"] {
            let dataset = generator.generate(task_type, 100);

            assert_eq!(dataset.feature_dim, 256);
            assert!(dataset.examples.len() > 0);
            assert!(dataset.validation.len() > 0);

            // Check quality scores are valid
            for example in &dataset.examples {
                assert!(example.quality >= 0.0 && example.quality <= 1.0);
            }

            let stats = dataset.stats();
            println!(
                "{}: train={}, val={}, avg_quality={:.2}",
                task_type, stats.train_size, stats.val_size, stats.avg_quality
            );
        }
    }

    #[test]
    fn test_adapter_training() {
        let adapters = RuvLtraAdapters::new();
        let lora = adapters.create_lora("coder", 256).unwrap();

        let generator = SyntheticDataGenerator::new(256, 42);
        let dataset = generator.generate("coder", 100);

        let config = AdapterTrainingConfig::quick();
        let mut trainer = AdapterTrainer::new(config);

        let result = trainer.train(&lora, &dataset).unwrap();

        assert!(result.epochs_completed > 0);
        assert!(result.total_steps > 0);
        assert!(result.final_loss >= 0.0);

        println!(
            "Training result: {} epochs, {} steps, loss={:.4}",
            result.epochs_completed, result.total_steps, result.final_loss
        );
    }

    #[test]
    fn test_adapter_inference() {
        let adapters = RuvLtraAdapters::new();
        let lora = adapters.create_lora("coder", 256).unwrap();

        let input = vec![0.5; 256];
        let output = lora.forward(&input, &TargetModule::QProj);

        assert_eq!(output.len(), 256);

        let mean = output.iter().sum::<f32>() / output.len() as f32;
        println!("Mean output: {:.4}", mean);
    }

    #[test]
    fn test_merge_average() {
        let adapters = RuvLtraAdapters::new();
        let lora1 = adapters.create_lora("coder", 256).unwrap();
        let lora2 = adapters.create_lora("researcher", 256).unwrap();

        let adapters_to_merge = vec![
            ("coder".to_string(), lora1),
            ("researcher".to_string(), lora2),
        ];

        let config = MergeConfig::average();
        let merger = AdapterMerger::new(config);

        let merged = merger
            .merge(&adapters_to_merge, &adapters.coder, 256)
            .unwrap();

        assert!(merged.is_enabled());
        assert!(merged.param_count() > 0);

        println!("Merged adapter: {} params", merged.param_count());
    }

    #[test]
    fn test_merge_weighted() {
        let adapters = RuvLtraAdapters::new();
        let lora1 = adapters.create_lora("coder", 256).unwrap();
        let lora2 = adapters.create_lora("security", 256).unwrap();

        let adapters_to_merge = vec![
            ("coder".to_string(), lora1),
            ("security".to_string(), lora2),
        ];

        let mut weights = HashMap::new();
        weights.insert("coder".to_string(), 0.7);
        weights.insert("security".to_string(), 0.3);

        let config = MergeConfig::weighted(weights);
        let merger = AdapterMerger::new(config);

        let merged = merger
            .merge(&adapters_to_merge, &adapters.coder, 256)
            .unwrap();

        assert!(merged.is_enabled());
    }

    #[test]
    fn test_merge_slerp() {
        let adapters = RuvLtraAdapters::new();
        let lora1 = adapters.create_lora("coder", 256).unwrap();
        let lora2 = adapters.create_lora("reviewer", 256).unwrap();

        let adapters_to_merge = vec![
            ("coder".to_string(), lora1),
            ("reviewer".to_string(), lora2),
        ];

        let config = MergeConfig::slerp(0.5);
        let merger = AdapterMerger::new(config);

        let merged = merger
            .merge(&adapters_to_merge, &adapters.coder, 256)
            .unwrap();

        assert!(merged.is_enabled());
    }

    #[test]
    fn test_hot_swap() {
        let adapters = RuvLtraAdapters::new();
        let lora1 = adapters.create_lora("coder", 256).unwrap();
        let lora2 = adapters.create_lora("security", 256).unwrap();

        let mut manager = HotSwapManager::new();

        manager.set_active(lora1);
        assert!(manager.active().is_some());

        manager.prepare_standby(lora2);
        manager.swap().unwrap();

        assert!(manager.active().is_some());
        assert!(!manager.is_swapping());
    }

    #[test]
    fn test_per_request_adaptation() {
        let adapters = RuvLtraAdapters::new();
        let lora = adapters.create_lora("coder", 256).unwrap();

        let input = vec![0.5; 256];

        // Baseline
        let baseline = lora.forward(&input, &TargetModule::QProj);
        let baseline_mean = baseline.iter().sum::<f32>() / baseline.len() as f32;

        // Adapt
        let feedback = AdaptFeedback::from_quality(0.9);
        lora.adapt(&input, feedback).unwrap();
        lora.apply_updates(0.01);

        // After adaptation
        let adapted = lora.forward(&input, &TargetModule::QProj);
        let adapted_mean = adapted.iter().sum::<f32>() / adapted.len() as f32;

        println!(
            "Baseline mean: {:.4}, Adapted mean: {:.4}",
            baseline_mean, adapted_mean
        );

        assert_eq!(lora.adaptation_count(), 1);
    }

    #[test]
    fn test_persistence() {
        let adapters = RuvLtraAdapters::new();
        let lora = adapters.create_lora("coder", 256).unwrap();

        // Adapt the model
        let input = vec![0.5; 256];
        let feedback = AdaptFeedback::from_quality(0.9);
        lora.adapt(&input, feedback).unwrap();
        lora.apply_updates(0.01);

        // Save
        let path = "/tmp/test_adapter.bin";
        lora.save(path).unwrap();

        // Load
        let loaded = ruvllm::lora::MicroLoRA::load(path).unwrap();

        assert_eq!(loaded.param_count(), lora.param_count());
        assert_eq!(loaded.memory_bytes(), lora.memory_bytes());

        println!("Saved and loaded adapter: {} params", loaded.param_count());

        // Cleanup
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn test_adapter_memory_footprint() {
        let adapters = RuvLtraAdapters::new();

        for name in &["coder", "researcher", "security", "architect", "reviewer"] {
            let config = adapters.get(name).unwrap();
            let mem_256 = config.estimate_memory(256);
            let mem_768 = config.estimate_memory(768);
            let mem_4096 = config.estimate_memory(4096);

            println!(
                "{}: 256d={:.1}KB, 768d={:.1}KB, 4096d={:.1}KB",
                name,
                mem_256 as f32 / 1024.0,
                mem_768 as f32 / 1024.0,
                mem_4096 as f32 / 1024.0
            );
        }
    }

    #[test]
    fn test_adapter_composition() {
        let adapters = RuvLtraAdapters::new();
        let generator = SyntheticDataGenerator::new(256, 42);

        // Create and train 3 adapters
        let datasets = generator.generate_all(50);

        let mut trained_adapters = Vec::new();
        for (name, dataset) in datasets.into_iter().take(3) {
            let lora = adapters.create_lora(&name, 256).unwrap();
            let mut trainer = AdapterTrainer::new(AdapterTrainingConfig::quick());
            trainer.train(&lora, &dataset).unwrap();
            trained_adapters.push((name, lora));
        }

        // TIES merge
        let ties_config = MergeConfig::ties(0.6);
        let ties_merger = AdapterMerger::new(ties_config);
        let ties_merged = ties_merger
            .merge(&trained_adapters, &adapters.coder, 256)
            .unwrap();

        assert!(ties_merged.is_enabled());

        println!("TIES merged adapter: {} params", ties_merged.param_count());
    }
}
