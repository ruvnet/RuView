//! Comprehensive tests for Claude task dataset generation

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_basic_dataset_generation() {
        let config = DatasetConfig {
            examples_per_category: 5,
            enable_augmentation: false,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        // 5 categories * 5 examples = 25 total
        assert_eq!(dataset.examples.len(), 25);
        assert_eq!(dataset.stats.total_examples, 25);
    }

    #[test]
    fn test_category_distribution() {
        let config = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        // Check each category has exactly 10 examples
        for category in TaskCategory::all() {
            let count = dataset
                .stats
                .examples_per_category
                .get(category.name())
                .unwrap_or(&0);
            assert_eq!(
                *count,
                10,
                "Category {} should have 10 examples",
                category.name()
            );
        }
    }

    #[test]
    fn test_augmentation_increases_dataset() {
        let config_no_aug = DatasetConfig {
            examples_per_category: 5,
            enable_augmentation: false,
            ..Default::default()
        };

        let config_with_aug = DatasetConfig {
            examples_per_category: 5,
            enable_augmentation: true,
            augmentation: AugmentationConfig {
                paraphrases_per_example: 1,
                complexity_variations: 1,
                enable_domain_transfer: true,
            },
            ..Default::default()
        };

        let mut gen_no_aug = DatasetGenerator::new(config_no_aug);
        let dataset_no_aug = gen_no_aug.generate();

        let mut gen_with_aug = DatasetGenerator::new(config_with_aug);
        let dataset_with_aug = gen_with_aug.generate();

        // Augmented dataset should be larger
        assert!(
            dataset_with_aug.examples.len() > dataset_no_aug.examples.len(),
            "Augmented dataset should be larger: {} vs {}",
            dataset_with_aug.examples.len(),
            dataset_no_aug.examples.len()
        );
    }

    #[test]
    fn test_model_recommendation_logic() {
        // Coder category
        assert_eq!(
            TaskCategory::Coder.recommended_model(ComplexityLevel::Simple),
            "haiku"
        );
        assert_eq!(
            TaskCategory::Coder.recommended_model(ComplexityLevel::Moderate),
            "sonnet"
        );
        assert_eq!(
            TaskCategory::Coder.recommended_model(ComplexityLevel::Complex),
            "opus"
        );

        // Security category (always opus)
        assert_eq!(
            TaskCategory::Security.recommended_model(ComplexityLevel::Simple),
            "opus"
        );
        assert_eq!(
            TaskCategory::Security.recommended_model(ComplexityLevel::Moderate),
            "opus"
        );
        assert_eq!(
            TaskCategory::Security.recommended_model(ComplexityLevel::Complex),
            "opus"
        );

        // Architecture category
        assert_eq!(
            TaskCategory::Architecture.recommended_model(ComplexityLevel::Simple),
            "sonnet"
        );
        assert_eq!(
            TaskCategory::Architecture.recommended_model(ComplexityLevel::Moderate),
            "opus"
        );
    }

    #[test]
    fn test_quality_scores_in_range() {
        let config = DatasetConfig {
            examples_per_category: 20,
            enable_augmentation: true,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        for example in &dataset.examples {
            assert!(
                example.metadata.quality_score >= 0.0 && example.metadata.quality_score <= 1.0,
                "Quality score must be in [0, 1]: {}",
                example.metadata.quality_score
            );
        }

        // Average quality should be reasonable
        assert!(
            dataset.stats.avg_quality_score >= 0.7 && dataset.stats.avg_quality_score <= 1.0,
            "Average quality should be good: {}",
            dataset.stats.avg_quality_score
        );
    }

    #[test]
    fn test_dataset_split_ratios() {
        let config = DatasetConfig {
            examples_per_category: 20,
            enable_augmentation: false,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        let (train, val, test) = dataset.split(0.7, 0.15, 0.15, 42);

        let total = train.len() + val.len() + test.len();
        assert_eq!(total, dataset.examples.len());

        // Check approximate ratios (allow small rounding errors)
        let train_ratio = train.len() as f32 / total as f32;
        let val_ratio = val.len() as f32 / total as f32;
        let test_ratio = test.len() as f32 / total as f32;

        assert!(
            (train_ratio - 0.7).abs() < 0.05,
            "Train ratio should be ~0.7: {}",
            train_ratio
        );
        assert!(
            (val_ratio - 0.15).abs() < 0.05,
            "Val ratio should be ~0.15: {}",
            val_ratio
        );
        assert!(
            (test_ratio - 0.15).abs() < 0.05,
            "Test ratio should be ~0.15: {}",
            test_ratio
        );
    }

    #[test]
    fn test_dataset_split_deterministic() {
        let config = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            seed: 42,
            ..Default::default()
        };

        let mut gen1 = DatasetGenerator::new(config.clone());
        let dataset1 = gen1.generate();
        let (train1, _, _) = dataset1.split(0.7, 0.15, 0.15, 42);

        let mut gen2 = DatasetGenerator::new(config);
        let dataset2 = gen2.generate();
        let (train2, _, _) = dataset2.split(0.7, 0.15, 0.15, 42);

        // Same seed should produce same split
        assert_eq!(train1.len(), train2.len());
        for (ex1, ex2) in train1.iter().zip(train2.iter()) {
            assert_eq!(ex1.input, ex2.input);
        }
    }

    #[test]
    fn test_all_categories_present() {
        let config = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        let mut categories_seen = std::collections::HashSet::new();
        for example in &dataset.examples {
            categories_seen.insert(example.metadata.category);
        }

        // Should see all 5 categories
        assert_eq!(categories_seen.len(), 5);
        assert!(categories_seen.contains(&TaskCategory::Coder));
        assert!(categories_seen.contains(&TaskCategory::Researcher));
        assert!(categories_seen.contains(&TaskCategory::Security));
        assert!(categories_seen.contains(&TaskCategory::Architecture));
        assert!(categories_seen.contains(&TaskCategory::Reviewer));
    }

    #[test]
    fn test_complexity_levels_present() {
        let config = DatasetConfig {
            examples_per_category: 20,
            enable_augmentation: true,
            augmentation: AugmentationConfig {
                paraphrases_per_example: 0,
                complexity_variations: 2,
                enable_domain_transfer: false,
            },
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        let mut complexities_seen = std::collections::HashSet::new();
        for example in &dataset.examples {
            complexities_seen.insert(example.metadata.complexity);
        }

        // Should see all 3 complexity levels due to variations
        assert!(complexities_seen.contains(&ComplexityLevel::Simple));
        assert!(complexities_seen.contains(&ComplexityLevel::Moderate));
        assert!(complexities_seen.contains(&ComplexityLevel::Complex));
    }

    #[test]
    fn test_domain_diversity() {
        let config = DatasetConfig {
            examples_per_category: 30,
            enable_augmentation: false,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        let mut domains_seen = std::collections::HashSet::new();
        for example in &dataset.examples {
            domains_seen.insert(example.metadata.domain);
        }

        // Should see multiple domains
        assert!(
            domains_seen.len() >= 3,
            "Should have at least 3 different domains"
        );
    }

    #[test]
    fn test_tags_not_empty() {
        let config = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        for example in &dataset.examples {
            assert!(
                !example.metadata.tags.is_empty(),
                "Examples should have tags"
            );
        }
    }

    #[test]
    fn test_output_agent_matches_category() {
        let config = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        for example in &dataset.examples {
            assert_eq!(
                example.output_agent,
                example.metadata.category.name(),
                "Output agent should match category"
            );
        }
    }

    #[test]
    fn test_expected_model_is_valid() {
        let config = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            ..Default::default()
        };

        let mut generator = DatasetGenerator::new(config);
        let dataset = generator.generate();

        for example in &dataset.examples {
            let model = &example.metadata.expected_model;
            assert!(
                model == "haiku" || model == "sonnet" || model == "opus",
                "Expected model should be haiku, sonnet, or opus: {}",
                model
            );
        }
    }

    #[test]
    fn test_reproducibility_with_seed() {
        let config1 = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            seed: 12345,
            ..Default::default()
        };

        let config2 = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            seed: 12345,
            ..Default::default()
        };

        let mut gen1 = DatasetGenerator::new(config1);
        let dataset1 = gen1.generate();

        let mut gen2 = DatasetGenerator::new(config2);
        let dataset2 = gen2.generate();

        // Same seed should produce same examples
        assert_eq!(dataset1.examples.len(), dataset2.examples.len());
        for (ex1, ex2) in dataset1.examples.iter().zip(dataset2.examples.iter()) {
            assert_eq!(ex1.input, ex2.input);
            assert_eq!(ex1.output_agent, ex2.output_agent);
        }
    }

    #[test]
    fn test_different_seeds_produce_different_data() {
        let config1 = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            seed: 111,
            ..Default::default()
        };

        let config2 = DatasetConfig {
            examples_per_category: 10,
            enable_augmentation: false,
            seed: 222,
            ..Default::default()
        };

        let mut gen1 = DatasetGenerator::new(config1);
        let dataset1 = gen1.generate();

        let mut gen2 = DatasetGenerator::new(config2);
        let dataset2 = gen2.generate();

        // Different seeds should produce different examples
        let mut different_count = 0;
        for (ex1, ex2) in dataset1.examples.iter().zip(dataset2.examples.iter()) {
            if ex1.input != ex2.input {
                different_count += 1;
            }
        }

        assert!(
            different_count > 0,
            "Different seeds should produce at least some different examples"
        );
    }
}
