//! Example: Training a FastGRNN model for routing decisions
//!
//! This example demonstrates:
//! - Synthetic data generation for routing tasks
//! - Training a FastGRNN model with validation
//! - Knowledge distillation from a teacher model
//! - Early stopping and learning rate scheduling
//! - Model evaluation and saving

use rand::Rng;
use ruvector_tiny_dancer_core::{
    model::{FastGRNN, FastGRNNConfig},
    training::{generate_teacher_predictions, Trainer, TrainingConfig, TrainingDataset},
    Result,
};
use std::path::PathBuf;

fn main() -> Result<()> {
    println!("=== FastGRNN Training Example ===\n");

    // 1. Generate synthetic training data
    println!("Generating synthetic training data...");
    let (features, labels) = generate_synthetic_data(1000);
    let mut dataset = TrainingDataset::new(features, labels)?;

    // Normalize features
    println!("Normalizing features...");
    let (means, stds) = dataset.normalize()?;
    println!("Feature means: {:?}", means);
    println!("Feature stds: {:?}\n", stds);

    // 2. Create model configuration
    let model_config = FastGRNNConfig {
        input_dim: 5,
        hidden_dim: 16,
        output_dim: 1,
        nu: 0.8,
        zeta: 1.2,
        rank: Some(8),
    };

    // 3. Create and initialize model
    println!("Creating FastGRNN model...");
    let mut model = FastGRNN::new(model_config.clone())?;
    println!("Model size: {} bytes\n", model.size_bytes());

    // 4. Optional: Knowledge distillation setup
    println!("Setting up knowledge distillation...");
    let teacher_model = create_pretrained_teacher(&model_config)?;
    let temperature = 3.0;
    let soft_targets =
        generate_teacher_predictions(&teacher_model, &dataset.features, temperature)?;
    dataset = dataset.with_soft_targets(soft_targets)?;
    println!("Generated soft targets from teacher model\n");

    // 5. Configure training
    let training_config = TrainingConfig {
        learning_rate: 0.01,
        batch_size: 32,
        epochs: 50,
        validation_split: 0.2,
        early_stopping_patience: Some(5),
        lr_decay: 0.8,
        lr_decay_step: 10,
        grad_clip: 5.0,
        adam_beta1: 0.9,
        adam_beta2: 0.999,
        adam_epsilon: 1e-8,
        l2_reg: 1e-4,
        enable_distillation: true,
        distillation_temperature: temperature,
        distillation_alpha: 0.7,
    };

    // 6. Create trainer and train model
    println!("Starting training...\n");
    let mut trainer = Trainer::new(&model_config, training_config);
    let metrics = trainer.train(&mut model, &dataset)?;

    // 7. Print training summary
    println!("\n=== Training Summary ===");
    println!("Total epochs: {}", metrics.len());
    if let Some(last_metrics) = metrics.last() {
        println!("Final train loss: {:.4}", last_metrics.train_loss);
        println!("Final val loss: {:.4}", last_metrics.val_loss);
        println!(
            "Final train accuracy: {:.2}%",
            last_metrics.train_accuracy * 100.0
        );
        println!(
            "Final val accuracy: {:.2}%",
            last_metrics.val_accuracy * 100.0
        );
    }

    // 8. Find best epoch
    if let Some(best) = metrics
        .iter()
        .min_by(|a, b| a.val_loss.partial_cmp(&b.val_loss).unwrap())
    {
        println!(
            "\nBest validation loss: {:.4} at epoch {}",
            best.val_loss,
            best.epoch + 1
        );
        println!(
            "Best validation accuracy: {:.2}%",
            best.val_accuracy * 100.0
        );
    }

    // 9. Test inference on sample data
    println!("\n=== Testing Inference ===");
    test_inference(&model)?;

    // 10. Save model and metrics
    println!("\n=== Saving Model ===");
    let model_path = PathBuf::from("models/fastgrnn_trained.safetensors");
    let metrics_path = PathBuf::from("models/training_metrics.json");

    // Create models directory if it doesn't exist
    std::fs::create_dir_all("models").ok();

    model.save(&model_path)?;
    trainer.save_metrics(&metrics_path)?;

    println!("Model saved to: {:?}", model_path);
    println!("Metrics saved to: {:?}", metrics_path);

    // 11. Demonstrate model optimization
    println!("\n=== Model Optimization ===");
    let original_size = model.size_bytes();
    println!("Original model size: {} bytes", original_size);

    model.quantize()?;
    let quantized_size = model.size_bytes();
    println!("Quantized model size: {} bytes", quantized_size);
    println!(
        "Size reduction: {:.1}%",
        (1.0 - quantized_size as f32 / original_size as f32) * 100.0
    );

    println!("\n=== Training Complete ===");

    Ok(())
}

/// Generate synthetic training data for routing decisions
///
/// Features represent:
/// - [0]: Semantic similarity (0.0 to 1.0)
/// - [1]: Recency score (0.0 to 1.0)
/// - [2]: Popularity score (0.0 to 1.0)
/// - [3]: Historical success rate (0.0 to 1.0)
/// - [4]: Query complexity (0.0 to 1.0)
///
/// Label: 1.0 = route to lightweight model, 0.0 = route to powerful model
fn generate_synthetic_data(n_samples: usize) -> (Vec<Vec<f32>>, Vec<f32>) {
    let mut rng = rand::thread_rng();
    let mut features = Vec::with_capacity(n_samples);
    let mut labels = Vec::with_capacity(n_samples);

    for _ in 0..n_samples {
        // Generate random features
        let similarity: f32 = rng.gen();
        let recency: f32 = rng.gen();
        let popularity: f32 = rng.gen();
        let success_rate: f32 = rng.gen();
        let complexity: f32 = rng.gen();

        let feature_vec = vec![similarity, recency, popularity, success_rate, complexity];

        // Generate label based on heuristic rules
        // High similarity + high success rate + low complexity -> lightweight (1.0)
        // Low similarity + low success rate + high complexity -> powerful (0.0)
        let lightweight_score = similarity * 0.4 + success_rate * 0.3 + (1.0 - complexity) * 0.3;

        // Add some noise and threshold
        let noise: f32 = rng.gen_range(-0.1..0.1);
        let label = if lightweight_score + noise > 0.6 {
            1.0
        } else {
            0.0
        };

        features.push(feature_vec);
        labels.push(label);
    }

    (features, labels)
}

/// Create a pretrained teacher model (simulated)
///
/// In practice, this would be a larger, more accurate model
/// For this example, we create a model with similar architecture
/// but pretend it's been trained to high accuracy
fn create_pretrained_teacher(config: &FastGRNNConfig) -> Result<FastGRNN> {
    // Create a teacher model with larger capacity
    let teacher_config = FastGRNNConfig {
        input_dim: config.input_dim,
        hidden_dim: config.hidden_dim * 2, // Larger model
        output_dim: config.output_dim,
        nu: config.nu,
        zeta: config.zeta,
        rank: config.rank.map(|r| r * 2),
    };

    let teacher = FastGRNN::new(teacher_config)?;
    // In practice, you would load pretrained weights here:
    // teacher.load("path/to/teacher/model.safetensors")?;

    Ok(teacher)
}

/// Test model inference on sample inputs
fn test_inference(model: &FastGRNN) -> Result<()> {
    // Test case 1: High confidence -> lightweight
    let high_confidence = vec![0.9, 0.8, 0.7, 0.9, 0.2]; // high sim, low complexity
    let pred1 = model.forward(&high_confidence, None)?;
    println!("High confidence case: prediction = {:.4}", pred1);

    // Test case 2: Low confidence -> powerful
    let low_confidence = vec![0.3, 0.2, 0.1, 0.4, 0.9]; // low sim, high complexity
    let pred2 = model.forward(&low_confidence, None)?;
    println!("Low confidence case: prediction = {:.4}", pred2);

    // Test case 3: Medium confidence
    let medium_confidence = vec![0.5, 0.5, 0.5, 0.5, 0.5];
    let pred3 = model.forward(&medium_confidence, None)?;
    println!("Medium confidence case: prediction = {:.4}", pred3);

    // Batch inference
    let batch = vec![high_confidence, low_confidence, medium_confidence];
    let batch_preds = model.forward_batch(&batch)?;
    println!("\nBatch predictions: {:?}", batch_preds);

    Ok(())
}

/// Example: Custom training loop with manual control
#[allow(dead_code)]
fn example_custom_training_loop() -> Result<()> {
    println!("=== Custom Training Loop Example ===\n");

    // Setup
    let (features, labels) = generate_synthetic_data(500);
    let dataset = TrainingDataset::new(features, labels)?;
    let (train_dataset, val_dataset) = dataset.split(0.2)?;

    let config = FastGRNNConfig::default();
    let mut model = FastGRNN::new(config.clone())?;

    let training_config = TrainingConfig {
        batch_size: 16,
        learning_rate: 0.005,
        epochs: 20,
        ..Default::default()
    };

    let mut trainer = Trainer::new(&config, training_config);

    // Custom training with per-epoch callbacks
    println!("Training with custom callbacks...");
    for epoch in 0..10 {
        // You could implement custom logic here
        // For example: dynamic batch size, custom metrics, etc.

        println!("Epoch {}: Custom preprocessing...", epoch + 1);

        // Train for one epoch
        // In practice, you'd call trainer.train_epoch() here
        // This is just to demonstrate the pattern
    }

    println!("Custom training complete!");

    Ok(())
}

/// Example: Continual learning scenario
#[allow(dead_code)]
fn example_continual_learning() -> Result<()> {
    println!("=== Continual Learning Example ===\n");

    let config = FastGRNNConfig::default();
    let mut model = FastGRNN::new(config.clone())?;

    // Train on initial dataset
    println!("Phase 1: Training on initial data...");
    let (features1, labels1) = generate_synthetic_data(500);
    let dataset1 = TrainingDataset::new(features1, labels1)?;

    let training_config = TrainingConfig {
        epochs: 20,
        ..Default::default()
    };

    let mut trainer = Trainer::new(&config, training_config.clone());
    trainer.train(&mut model, &dataset1)?;

    // Continue training on new data
    println!("\nPhase 2: Continual learning on new data...");
    let (features2, labels2) = generate_synthetic_data(300);
    let dataset2 = TrainingDataset::new(features2, labels2)?;

    let mut trainer2 = Trainer::new(&config, training_config);
    trainer2.train(&mut model, &dataset2)?;

    println!("\nContinual learning complete!");

    Ok(())
}
