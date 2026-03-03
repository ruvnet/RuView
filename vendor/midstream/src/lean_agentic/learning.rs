//! Stream learning and online adaptation

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use async_trait::async_trait;

use super::agent::Action;

/// Stream learner for online adaptation
pub struct StreamLearner {
    /// Online model for learning
    model: OnlineModel,

    /// Learning rate
    learning_rate: f64,

    /// Experience buffer for replay
    experience_buffer: VecDeque<Experience>,

    /// Buffer size
    buffer_size: usize,

    /// Total iterations
    iterations: u64,

    /// Adaptation strategy
    strategy: AdaptationStrategy,
}

impl StreamLearner {
    pub fn new(learning_rate: f64) -> Self {
        Self {
            model: OnlineModel::new(),
            learning_rate,
            experience_buffer: VecDeque::new(),
            buffer_size: 1000,
            iterations: 0,
            strategy: AdaptationStrategy::default(),
        }
    }

    /// Update model with new experience
    pub async fn update(
        &mut self,
        action: &Action,
        reward: f64,
        context: &str,
    ) -> Result<(), String> {
        self.iterations += 1;

        // Create experience
        let experience = Experience {
            action: action.clone(),
            reward,
            context: context.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        // Add to buffer
        self.experience_buffer.push_back(experience.clone());
        if self.experience_buffer.len() > self.buffer_size {
            self.experience_buffer.pop_front();
        }

        // Update model based on strategy
        match &self.strategy {
            AdaptationStrategy::Immediate => {
                self.model.update_immediate(&experience, self.learning_rate).await?;
            }
            AdaptationStrategy::Batched { batch_size } => {
                if self.iterations % batch_size == 0 {
                    self.model.update_batch(&self.experience_buffer, self.learning_rate).await?;
                }
            }
            AdaptationStrategy::ExperienceReplay { replay_size } => {
                self.model.update_immediate(&experience, self.learning_rate).await?;

                // Replay random experiences
                let replay_samples = self.sample_experiences(*replay_size);
                for sample in replay_samples {
                    self.model.update_immediate(&sample, self.learning_rate * 0.5).await?;
                }
            }
        }

        Ok(())
    }

    /// Sample random experiences for replay (using simple deterministic sampling)
    fn sample_experiences(&self, n: usize) -> Vec<Experience> {
        // Simple deterministic sampling: take evenly spaced samples
        let experiences: Vec<_> = self.experience_buffer.iter().cloned().collect();
        let total = experiences.len();

        if total == 0 || n == 0 {
            return Vec::new();
        }

        let step = (total as f64 / n as f64).max(1.0) as usize;

        experiences.iter()
            .step_by(step)
            .take(n)
            .cloned()
            .collect()
    }

    /// Predict reward for an action
    pub async fn predict_reward(&self, action: &Action, context: &str) -> f64 {
        self.model.predict(action, context).await
    }

    /// Get learning statistics
    pub fn get_stats(&self) -> LearningStats {
        LearningStats {
            iterations: self.iterations,
            buffer_size: self.experience_buffer.len(),
            average_reward: self.compute_average_reward(),
            model_parameters: self.model.parameter_count(),
        }
    }

    fn compute_average_reward(&self) -> f64 {
        if self.experience_buffer.is_empty() {
            return 0.0;
        }

        let sum: f64 = self.experience_buffer.iter()
            .map(|e| e.reward)
            .sum();

        sum / self.experience_buffer.len() as f64
    }

    pub fn iteration_count(&self) -> u64 {
        self.iterations
    }
}

/// Online learning model
pub struct OnlineModel {
    /// Feature weights
    weights: HashMap<String, f64>,

    /// Bias term
    bias: f64,

    /// Feature statistics for normalization
    feature_stats: HashMap<String, FeatureStats>,
}

impl OnlineModel {
    pub fn new() -> Self {
        Self {
            weights: HashMap::new(),
            bias: 0.0,
            feature_stats: HashMap::new(),
        }
    }

    /// Extract features from action and context
    fn extract_features(&self, action: &Action, context: &str) -> HashMap<String, f64> {
        let mut features = HashMap::new();

        // Action type feature
        features.insert(
            format!("action_{}", action.action_type),
            1.0,
        );

        // Number of parameters
        features.insert(
            "param_count".to_string(),
            action.parameters.len() as f64,
        );

        // Number of tool calls
        features.insert(
            "tool_count".to_string(),
            action.tool_calls.len() as f64,
        );

        // Context length
        features.insert(
            "context_length".to_string(),
            context.len() as f64 / 100.0, // Normalize
        );

        // Expected reward (from action)
        features.insert(
            "expected_reward".to_string(),
            action.expected_reward,
        );

        features
    }

    /// Predict reward for given features
    pub async fn predict(&self, action: &Action, context: &str) -> f64 {
        let features = self.extract_features(action, context);

        let mut prediction = self.bias;

        for (feature, value) in features {
            if let Some(weight) = self.weights.get(&feature) {
                prediction += weight * value;
            }
        }

        prediction
    }

    /// Update model immediately with single experience
    pub async fn update_immediate(
        &mut self,
        experience: &Experience,
        learning_rate: f64,
    ) -> Result<(), String> {
        let features = self.extract_features(&experience.action, &experience.context);
        let prediction = self.predict(&experience.action, &experience.context).await;

        // Gradient descent update
        let error = experience.reward - prediction;

        // Update bias
        self.bias += learning_rate * error;

        // Update weights
        for (feature, value) in features {
            let weight = self.weights.entry(feature.clone()).or_insert(0.0);
            *weight += learning_rate * error * value;

            // Update feature statistics
            let stats = self.feature_stats.entry(feature).or_insert(FeatureStats::default());
            stats.update(value);
        }

        Ok(())
    }

    /// Update model with batch of experiences
    pub async fn update_batch(
        &mut self,
        experiences: &VecDeque<Experience>,
        learning_rate: f64,
    ) -> Result<(), String> {
        for experience in experiences {
            self.update_immediate(experience, learning_rate).await?;
        }

        Ok(())
    }

    pub fn parameter_count(&self) -> usize {
        self.weights.len() + 1 // weights + bias
    }
}

/// Experience tuple for learning
#[derive(Debug, Clone)]
pub struct Experience {
    pub action: Action,
    pub reward: f64,
    pub context: String,
    pub timestamp: i64,
}

/// Adaptation strategy for online learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdaptationStrategy {
    /// Update immediately after each experience
    Immediate,

    /// Update in batches
    Batched { batch_size: u64 },

    /// Use experience replay
    ExperienceReplay { replay_size: usize },
}

impl Default for AdaptationStrategy {
    fn default() -> Self {
        AdaptationStrategy::Immediate
    }
}

/// Feature statistics for normalization
#[derive(Debug, Clone, Default)]
struct FeatureStats {
    count: u64,
    sum: f64,
    sum_squared: f64,
}

impl FeatureStats {
    fn update(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.sum_squared += value * value;
    }

    fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    fn variance(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            let mean = self.mean();
            (self.sum_squared / self.count as f64) - (mean * mean)
        }
    }
}

/// Learning statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningStats {
    pub iterations: u64,
    pub buffer_size: usize,
    pub average_reward: f64,
    pub model_parameters: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_stream_learner() {
        let mut learner = StreamLearner::new(0.01);

        let action = Action {
            action_type: "test".to_string(),
            description: "Test action".to_string(),
            parameters: HashMap::new(),
            tool_calls: vec![],
            expected_outcome: None,
            expected_reward: 0.5,
        };

        let result = learner.update(&action, 1.0, "test context").await;
        assert!(result.is_ok());

        let stats = learner.get_stats();
        assert_eq!(stats.iterations, 1);
    }
}
