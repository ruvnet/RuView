//! Self-learning service for continuous improvement

use crate::config::LearningConfig;
use crate::error::{Error, Result};
use crate::memory::MemoryService;
use crate::router::FastGRNNRouter;
use crate::types::{Feedback, InteractionOutcome, RouterSample};

use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Learning service managing continuous improvement
pub struct LearningService {
    /// Configuration
    config: LearningConfig,
    /// Router reference
    router: Arc<RwLock<FastGRNNRouter>>,
    /// Memory reference
    memory: Arc<MemoryService>,
    /// Embedding dimension for creating new vectors
    embedding_dim: usize,
    /// Replay buffer
    replay_buffer: RwLock<ReplayBuffer>,
    /// EWC state
    ewc: RwLock<EWCState>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Background task handle
    task_handle: RwLock<Option<JoinHandle<()>>>,
}

/// Replay buffer with reservoir sampling
#[derive(Debug, Default)]
struct ReplayBuffer {
    entries: Vec<RouterSample>,
    capacity: usize,
    total_seen: u64,
}

/// Elastic Weight Consolidation state
#[derive(Debug, Default)]
struct EWCState {
    /// Fisher information diagonal
    fisher_info: Vec<f32>,
    /// Optimal weights from previous task
    optimal_weights: Vec<f32>,
    /// Lambda regularization strength
    lambda: f32,
}

impl LearningService {
    /// Create a new learning service
    pub fn new(
        config: &LearningConfig,
        router: Arc<RwLock<FastGRNNRouter>>,
        memory: Arc<MemoryService>,
        embedding_dim: usize,
    ) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            router,
            memory,
            embedding_dim,
            replay_buffer: RwLock::new(ReplayBuffer {
                entries: Vec::new(),
                capacity: config.replay_capacity,
                total_seen: 0,
            }),
            ewc: RwLock::new(EWCState {
                fisher_info: Vec::new(),
                optimal_weights: Vec::new(),
                lambda: config.ewc_lambda,
            }),
            shutdown_tx: None,
            task_handle: RwLock::new(None),
        })
    }

    /// Start background training loop
    pub async fn start_background_training(&self) {
        let (tx, mut rx) = mpsc::channel::<()>(1);

        let config = self.config.clone();
        let router = self.router.clone();
        let replay_buffer = Arc::new(RwLock::new(ReplayBuffer {
            entries: Vec::new(),
            capacity: config.replay_capacity,
            total_seen: 0,
        }));

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(
                config.training_interval_ms,
            ));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Check if enough samples
                        let buffer = replay_buffer.read();
                        if buffer.entries.len() < config.min_samples {
                            continue;
                        }
                        drop(buffer);

                        // Training step would go here
                        tracing::debug!("Background training tick");
                    }
                    _ = rx.recv() => {
                        tracing::info!("Learning service shutting down");
                        break;
                    }
                }
            }
        });

        *self.task_handle.write() = Some(handle);
    }

    /// Called on each interaction
    pub async fn on_interaction(
        &self,
        query: &str,
        response: &str,
        context: &[String],
    ) -> Result<InteractionOutcome> {
        // Skip if learning is disabled
        if !self.config.enabled {
            return Ok(InteractionOutcome {
                quality_score: 0.0,
                used_nodes: vec![],
                task_success: true,
                user_rating: None,
            });
        }

        // Evaluate quality (mock - in production use LLM judge)
        let quality_score = self.evaluate_quality(query, response, context);

        // Create outcome
        let outcome = InteractionOutcome {
            quality_score,
            used_nodes: vec![],
            task_success: quality_score > 0.5,
            user_rating: None,
        };

        // Maybe write to memory
        if quality_score >= self.config.quality_threshold {
            self.writeback(query, response, quality_score).await?;
        }

        Ok(outcome)
    }

    /// Record explicit feedback
    pub async fn record_feedback(&self, feedback: Feedback) -> Result<()> {
        tracing::info!(
            request_id = %feedback.request_id,
            rating = ?feedback.rating,
            "Recording feedback"
        );

        // Update memory edges based on feedback
        if let Some(rating) = feedback.rating {
            let delta = (rating as f32 - 3.0) / 10.0; // -0.2 to +0.2
                                                      // In production, look up the request and update edge weights
            tracing::debug!(delta = delta, "Would update edge weights");
        }

        Ok(())
    }

    /// Stop the learning service
    pub async fn stop(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }

        if let Some(handle) = self.task_handle.write().take() {
            let _ = handle.await;
        }
    }

    fn evaluate_quality(&self, query: &str, response: &str, _context: &[String]) -> f32 {
        // Simple heuristic quality evaluation (in production, use LLM judge)
        let mut score = 0.5;

        // Longer responses are typically better (up to a point)
        let word_count = response.split_whitespace().count();
        if word_count > 10 {
            score += 0.1;
        }
        if word_count > 50 {
            score += 0.1;
        }

        // Response should relate to query
        let query_lower = query.to_lowercase();
        let query_words: std::collections::HashSet<_> = query_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();
        let response_lower = response.to_lowercase();
        let response_words: std::collections::HashSet<_> = response_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();

        let overlap = query_words.intersection(&response_words).count();
        if overlap > 0 {
            score += 0.1 * (overlap as f32).min(3.0);
        }

        score.min(1.0)
    }

    async fn writeback(&self, query: &str, response: &str, quality: f32) -> Result<()> {
        use crate::types::{MemoryNode, NodeType};
        use std::collections::HashMap;
        use uuid::Uuid;

        // Create combined Q&A node
        let text = format!("Q: {}\nA: {}", query, response);

        // Mock embedding using configured dimension
        let vector = vec![0.0f32; self.embedding_dim];

        let node = MemoryNode {
            id: Uuid::new_v4().to_string(),
            vector,
            text,
            node_type: NodeType::QAPair,
            source: "self_learning".into(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("quality".into(), serde_json::json!(quality));
                m.insert(
                    "timestamp".into(),
                    serde_json::json!(chrono::Utc::now().timestamp()),
                );
                m
            },
        };

        self.memory.insert_node(node)?;
        tracing::debug!(quality = quality, "Wrote interaction to memory");

        Ok(())
    }
}

impl ReplayBuffer {
    fn add(&mut self, sample: RouterSample) {
        self.total_seen += 1;

        if self.entries.len() < self.capacity {
            self.entries.push(sample);
        } else {
            // Reservoir sampling
            use rand::Rng;
            let idx = rand::thread_rng().gen_range(0..self.total_seen) as usize;
            if idx < self.capacity {
                self.entries[idx] = sample;
            }
        }
    }

    fn sample(&self, batch_size: usize) -> Vec<&RouterSample> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        self.entries.choose_multiple(&mut rng, batch_size).collect()
    }
}

impl EWCState {
    fn regularization_loss(&self, current_weights: &[f32]) -> f32 {
        if self.fisher_info.is_empty() || self.optimal_weights.is_empty() {
            return 0.0;
        }

        self.fisher_info
            .iter()
            .zip(current_weights.iter())
            .zip(self.optimal_weights.iter())
            .map(|((f, w), w_star)| f * (w - w_star).powi(2))
            .sum::<f32>()
            * self.lambda
            / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_buffer() {
        let mut buffer = ReplayBuffer {
            entries: Vec::new(),
            capacity: 10,
            total_seen: 0,
        };

        for i in 0..20 {
            buffer.add(RouterSample {
                features: vec![i as f32],
                label_model: 0,
                label_context: 0,
                label_temperature: 0.7,
                label_top_p: 0.9,
                quality: 0.8,
                latency_ms: 100.0,
            });
        }

        // Buffer should be at capacity
        assert_eq!(buffer.entries.len(), 10);
        assert_eq!(buffer.total_seen, 20);
    }

    #[test]
    fn test_ewc_regularization() {
        let ewc = EWCState {
            fisher_info: vec![1.0, 1.0, 1.0],
            optimal_weights: vec![0.0, 0.0, 0.0],
            lambda: 1.0,
        };

        let current = vec![1.0, 1.0, 1.0];
        let loss = ewc.regularization_loss(&current);

        // Should penalize deviation from optimal
        assert!(loss > 0.0);
    }
}
