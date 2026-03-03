use crate::mlp::Mlp;
use crate::mlp_optimized::OptimizedMlp;
use crate::mlp_ultra::UltraMlp;
use crate::mlp_classifier::ClassifierMlp;
use rayon::prelude::*;
use std::sync::Arc;

/// Ensemble model combining multiple predictors with weighted voting
pub struct EnsembleModel {
    models: Vec<ModelType>,
    weights: Vec<f32>,
    use_adaptive_weights: bool,
}

enum ModelType {
    Simple(Mlp),
    Optimized(OptimizedMlp),
    Ultra(UltraMlp),
    Classifier(ClassifierMlp),
}

impl EnsembleModel {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            weights: Vec::new(),
            use_adaptive_weights: true,
        }
    }

    pub fn add_model_simple(&mut self, input: usize, hidden: usize, output: usize) {
        self.models.push(ModelType::Simple(Mlp::new(input, hidden, output)));
        self.weights.push(1.0);
    }

    pub fn add_model_optimized(&mut self, input: usize, hidden: usize, output: usize) {
        self.models.push(ModelType::Optimized(OptimizedMlp::new(input, hidden, output)));
        self.weights.push(1.0);
    }

    pub fn add_model_ultra(&mut self, input: usize, hidden: usize, output: usize) {
        self.models.push(ModelType::Ultra(UltraMlp::new(input, hidden, output)));
        self.weights.push(1.0);
    }

    pub fn add_model_classifier(&mut self, input: usize, output: usize) {
        self.models.push(ModelType::Classifier(ClassifierMlp::new(input, output)));
        self.weights.push(1.0);
    }

    /// Train all models in parallel with different random initializations
    pub fn train_ensemble(&mut self, x: &Vec<Vec<f32>>, y: &Vec<f32>,
                         epochs: usize, lr: f32, val_x: &Vec<Vec<f32>>, val_y: &Vec<usize>) {
        use rand::seq::SliceRandom;
        use rand::thread_rng;

        // Train each model with different data shuffling for diversity
        let model_count = self.models.len();

        // Parallel training with thread-safe access
        let x_arc = Arc::new(x.clone());
        let y_arc = Arc::new(y.clone());

        // Sequential training (models contain mutable state)
        for (i, model) in self.models.iter_mut().enumerate() {
            let mut indices: Vec<usize> = (0..x.len()).collect();
            let mut rng = thread_rng();
            use rand::Rng;
            indices.shuffle(&mut rng);

            // Bootstrap sampling for diversity
            let bootstrap_size = (x.len() as f32 * 0.8) as usize;
            let bootstrap_indices: Vec<usize> = (0..bootstrap_size)
                .map(|_| indices[rng.gen::<usize>() % indices.len()])
                .collect();

            let x_bootstrap: Vec<Vec<f32>> = bootstrap_indices.iter()
                .map(|&i| x[i].clone())
                .collect();
            let y_bootstrap: Vec<f32> = bootstrap_indices.iter()
                .map(|&i| y[i])
                .collect();

            // Train based on model type
            match model {
                ModelType::Simple(ref mut m) => {
                    m.train_regression(&x_bootstrap, &y_bootstrap, epochs, lr);
                }
                ModelType::Optimized(ref mut m) => {
                    m.train_batch(&x_bootstrap, &y_bootstrap, epochs, lr, 32);
                }
                ModelType::Ultra(ref mut m) => {
                    m.train_batch_parallel(&x_bootstrap, &y_bootstrap, epochs, lr, 32);
                }
                ModelType::Classifier(ref mut m) => {
                    m.train_classification(&x_bootstrap, &y_bootstrap, epochs, 32);
                }
            }

            println!("Trained model {}/{}", i + 1, model_count);
        }

        // Update weights based on validation performance
        if self.use_adaptive_weights && !val_x.is_empty() {
            self.update_weights(val_x, val_y);
        }
    }

    /// Update ensemble weights based on validation accuracy
    fn update_weights(&mut self, val_x: &Vec<Vec<f32>>, val_y: &Vec<usize>) {
        let accuracies: Vec<f32> = self.models.iter_mut().map(|model| {
            let predictions = match model {
                ModelType::Simple(ref m) => m.predict_cls3(val_x),
                ModelType::Optimized(ref m) => m.predict_cls3(val_x),
                ModelType::Ultra(ref mut m) => m.predict_cls3_parallel(val_x),
                ModelType::Classifier(ref mut m) => m.predict_cls3(val_x),
            };

            let correct = predictions.iter().zip(val_y.iter())
                .filter(|(p, y)| p == y)
                .count();
            correct as f32 / val_y.len() as f32
        }).collect();

        // Convert accuracies to weights (squared for emphasis)
        let total_acc: f32 = accuracies.iter().map(|&a| a * a).sum();
        if total_acc > 0.0 {
            self.weights = accuracies.iter()
                .map(|&a| (a * a) / total_acc)
                .collect();
        }

        println!("Ensemble weights: {:?}", self.weights);
    }

    /// Predict using weighted voting
    pub fn predict_ensemble(&mut self, x: &[Vec<f32>]) -> Vec<usize> {
        let predictions: Vec<Vec<usize>> = self.models.iter_mut().map(|model| {
            match model {
                ModelType::Simple(ref m) => m.predict_cls3(x),
                ModelType::Optimized(ref m) => m.predict_cls3(x),
                ModelType::Ultra(ref mut m) => m.predict_cls3_parallel(x),
                ModelType::Classifier(ref mut m) => m.predict_cls3(x),
            }
        }).collect();

        // Weighted voting for each sample
        (0..x.len()).map(|i| {
            let mut votes = vec![0.0; 3]; // 3 classes

            for (model_idx, model_preds) in predictions.iter().enumerate() {
                let pred = model_preds[i];
                if pred < 3 {
                    votes[pred] += self.weights[model_idx];
                }
            }

            // Return class with highest weighted votes
            votes.iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(1) // Default to middle class
        }).collect()
    }

    /// Fast ensemble prediction with majority voting (no weights)
    pub fn predict_fast(&mut self, x: &[Vec<f32>]) -> Vec<usize> {
        let predictions: Vec<Vec<usize>> = self.models.par_iter_mut().map(|model| {
            match model {
                ModelType::Simple(ref m) => m.predict_cls3(x),
                ModelType::Optimized(ref m) => m.predict_cls3(x),
                ModelType::Ultra(ref mut m) => m.predict_cls3_parallel(x),
                ModelType::Classifier(ref mut m) => m.predict_cls3(x),
            }
        }).collect();

        // Simple majority voting
        (0..x.len()).into_par_iter().map(|i| {
            let mut votes = [0u32; 3];
            for model_preds in &predictions {
                if model_preds[i] < 3 {
                    votes[model_preds[i]] += 1;
                }
            }

            votes.iter()
                .enumerate()
                .max_by_key(|&(_, &v)| v)
                .map(|(idx, _)| idx)
                .unwrap_or(1)
        }).collect()
    }
}

/// Boosted ensemble using AdaBoost-style weighting
pub struct BoostedEnsemble {
    weak_learners: Vec<Mlp>,
    alphas: Vec<f32>,
    input_dim: usize,
    hidden_dim: usize,
}

impl BoostedEnsemble {
    pub fn new(input: usize, hidden: usize) -> Self {
        Self {
            weak_learners: Vec::new(),
            alphas: Vec::new(),
            input_dim: input,
            hidden_dim: hidden,
        }
    }

    pub fn train_boosted(&mut self, x: &Vec<Vec<f32>>, y: &Vec<usize>,
                        n_estimators: usize, epochs: usize) {
        let n_samples = x.len();
        let mut weights = vec![1.0 / n_samples as f32; n_samples];

        for t in 0..n_estimators {
            // Train weak learner on weighted data
            let mut learner = Mlp::new(self.input_dim, self.hidden_dim, 3);

            // Convert classes to continuous for regression
            let y_cont: Vec<f32> = y.iter().map(|&c| c as f32 - 1.0).collect();

            // Weight samples by resampling
            let mut weighted_x = Vec::new();
            let mut weighted_y = Vec::new();

            use rand::thread_rng;
            use rand::distributions::{Distribution, WeightedIndex};
            let mut rng = thread_rng();
            let dist = WeightedIndex::new(&weights).unwrap();

            for _ in 0..n_samples {
                let idx = dist.sample(&mut rng);
                weighted_x.push(x[idx].clone());
                weighted_y.push(y_cont[idx]);
            }

            learner.train_regression(&weighted_x, &weighted_y, epochs, 0.01);

            // Calculate error
            let predictions = learner.predict_cls3(x);
            let mut error = 0.0;
            for i in 0..n_samples {
                if predictions[i] != y[i] {
                    error += weights[i];
                }
            }

            // Avoid division by zero
            if error >= 0.5 {
                break; // Stop if not better than random
            }

            // Calculate alpha
            let alpha = 0.5 * ((1.0 - error) / error.max(1e-10)).ln();

            // Update weights
            for i in 0..n_samples {
                if predictions[i] != y[i] {
                    weights[i] *= (alpha.exp());
                } else {
                    weights[i] *= ((-alpha).exp());
                }
            }

            // Normalize weights
            let sum: f32 = weights.iter().sum();
            weights.iter_mut().for_each(|w| *w /= sum);

            self.weak_learners.push(learner);
            self.alphas.push(alpha);

            println!("Boosting round {}/{}, error: {:.4}", t + 1, n_estimators, error);
        }
    }

    pub fn predict_boosted(&self, x: &[Vec<f32>]) -> Vec<usize> {
        let n_classes = 3;

        x.iter().map(|xi| {
            let mut class_scores = vec![0.0; n_classes];

            for (learner, &alpha) in self.weak_learners.iter().zip(&self.alphas) {
                let pred = learner.predict_cls3(&[xi.clone()])[0];
                if pred < n_classes {
                    class_scores[pred] += alpha;
                }
            }

            class_scores.iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(idx, _)| idx)
                .unwrap_or(1)
        }).collect()
    }
}