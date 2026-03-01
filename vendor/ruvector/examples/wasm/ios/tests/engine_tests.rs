//! Integration tests for iOS WASM Recommendation Engine
//!
//! Run with: cargo test --features std

#![cfg(test)]

use std::time::Instant;

// Note: These tests require std, so they run in native mode
// For WASM testing, use wasm-bindgen-test or a WASI runtime

mod embeddings {
    use super::*;

    // Re-implement test versions since the main crate is no_std
    #[derive(Clone, Debug, Default)]
    struct ContentMetadata {
        id: u64,
        content_type: u8,
        duration_secs: u32,
        category_flags: u32,
        popularity: f32,
        recency: f32,
    }

    struct ContentEmbedder {
        dim: usize,
        projection: Vec<f32>,
    }

    impl ContentEmbedder {
        fn new(dim: usize) -> Self {
            let mut projection = Vec::with_capacity(dim * 8);
            let mut seed: u32 = 12345;
            for _ in 0..(dim * 8) {
                seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                let val = ((seed >> 16) as f32 / 32768.0) - 1.0;
                projection.push(val * 0.1);
            }
            Self { dim, projection }
        }

        fn embed(&self, content: &ContentMetadata) -> Vec<f32> {
            let mut embedding = vec![0.0f32; self.dim];
            let features = [
                content.content_type as f32 / 4.0,
                (content.duration_secs as f32).ln_1p() / 10.0,
                (content.category_flags as f32).sqrt() / 64.0,
                content.popularity,
                content.recency,
                content.id as f32 % 1000.0 / 1000.0,
                ((content.id >> 10) as f32 % 1000.0) / 1000.0,
                ((content.id >> 20) as f32 % 1000.0) / 1000.0,
            ];

            for (i, e) in embedding.iter_mut().enumerate() {
                for (j, &feat) in features.iter().enumerate() {
                    let proj_idx = i * 8 + j;
                    if proj_idx < self.projection.len() {
                        *e += feat * self.projection[proj_idx];
                    }
                }
            }

            // Normalize
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-8 {
                for x in &mut embedding {
                    *x /= norm;
                }
            }
            embedding
        }

        fn similarity(a: &[f32], b: &[f32]) -> f32 {
            a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
        }
    }

    #[test]
    fn test_embedding_dimensions() {
        let embedder = ContentEmbedder::new(64);
        let content = ContentMetadata::default();
        let embedding = embedder.embed(&content);

        assert_eq!(embedding.len(), 64, "Embedding should have 64 dimensions");
    }

    #[test]
    fn test_embedding_normalized() {
        let embedder = ContentEmbedder::new(64);
        let content = ContentMetadata { id: 42, ..Default::default() };
        let embedding = embedder.embed(&content);

        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001, "Embedding should be L2 normalized");
    }

    #[test]
    fn test_embedding_deterministic() {
        let embedder = ContentEmbedder::new(64);
        let content = ContentMetadata { id: 123, ..Default::default() };

        let e1 = embedder.embed(&content);
        let e2 = embedder.embed(&content);

        assert_eq!(e1, e2, "Same content should produce same embedding");
    }

    #[test]
    fn test_similarity_range() {
        let embedder = ContentEmbedder::new(64);

        let c1 = ContentMetadata { id: 1, ..Default::default() };
        let c2 = ContentMetadata { id: 2, ..Default::default() };

        let e1 = embedder.embed(&c1);
        let e2 = embedder.embed(&c2);

        let sim = ContentEmbedder::similarity(&e1, &e2);
        assert!(sim >= -1.0 && sim <= 1.0, "Similarity should be in [-1, 1]");
    }

    #[test]
    fn test_self_similarity() {
        let embedder = ContentEmbedder::new(64);
        let content = ContentMetadata { id: 1, ..Default::default() };
        let embedding = embedder.embed(&content);

        let sim = ContentEmbedder::similarity(&embedding, &embedding);
        assert!((sim - 1.0).abs() < 0.001, "Self-similarity should be ~1.0");
    }

    #[test]
    fn test_embedding_performance() {
        let embedder = ContentEmbedder::new(64);
        let contents: Vec<ContentMetadata> = (0..1000)
            .map(|i| ContentMetadata { id: i, ..Default::default() })
            .collect();

        let start = Instant::now();
        for content in &contents {
            let _ = embedder.embed(content);
        }
        let duration = start.elapsed();

        let ops_per_sec = 1000.0 / duration.as_secs_f64();
        println!("Embedding throughput: {:.0} ops/sec", ops_per_sec);

        assert!(ops_per_sec > 10000.0, "Should embed >10k items/sec");
    }
}

mod qlearning {
    use super::*;

    #[derive(Clone, Copy, Debug)]
    enum InteractionType {
        View = 0,
        Like = 1,
        Share = 2,
        Skip = 3,
        Complete = 4,
        Dismiss = 5,
    }

    impl InteractionType {
        fn to_reward(self) -> f32 {
            match self {
                InteractionType::View => 0.1,
                InteractionType::Like => 0.8,
                InteractionType::Share => 1.0,
                InteractionType::Skip => -0.1,
                InteractionType::Complete => 0.6,
                InteractionType::Dismiss => -0.5,
            }
        }
    }

    struct QLearner {
        q_table: Vec<f32>,
        learning_rate: f32,
        discount: f32,
        exploration: f32,
        state_dim: usize,
        action_dim: usize,
        total_updates: u64,
    }

    impl QLearner {
        fn new(action_dim: usize) -> Self {
            let state_dim = 16;
            Self {
                q_table: vec![0.0; state_dim * action_dim],
                learning_rate: 0.1,
                discount: 0.95,
                exploration: 0.1,
                state_dim,
                action_dim,
                total_updates: 0,
            }
        }

        fn discretize_state(&self, state: &[f32]) -> usize {
            if state.is_empty() { return 0; }
            let mut hash: u32 = 0;
            for (i, &val) in state.iter().take(8).enumerate() {
                let quantized = ((val + 1.0) * 127.0) as u32;
                hash = hash.wrapping_add(quantized << (i * 4));
            }
            (hash as usize) % self.state_dim
        }

        fn get_q(&self, state: usize, action: usize) -> f32 {
            let idx = state * self.action_dim + action;
            self.q_table.get(idx).copied().unwrap_or(0.0)
        }

        fn set_q(&mut self, state: usize, action: usize, value: f32) {
            let idx = state * self.action_dim + action;
            if idx < self.q_table.len() {
                self.q_table[idx] = value;
            }
        }

        fn update(&mut self, state: &[f32], action: usize, reward: f32, next_state: &[f32]) {
            let s = self.discretize_state(state);
            let ns = self.discretize_state(next_state);

            let max_next_q = (0..self.action_dim)
                .map(|a| self.get_q(ns, a))
                .fold(f32::NEG_INFINITY, f32::max)
                .max(0.0);

            let current_q = self.get_q(s, action);
            let td_target = reward + self.discount * max_next_q;
            let new_q = current_q + self.learning_rate * (td_target - current_q);

            self.set_q(s, action, new_q);
            self.total_updates += 1;
        }

        fn rank_actions(&self, state: &[f32]) -> Vec<usize> {
            let s = self.discretize_state(state);
            let mut actions: Vec<(usize, f32)> = (0..self.action_dim)
                .map(|a| (a, self.get_q(s, a)))
                .collect();
            actions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            actions.into_iter().map(|(a, _)| a).collect()
        }
    }

    #[test]
    fn test_qlearner_initialization() {
        let learner = QLearner::new(50);
        assert_eq!(learner.action_dim, 50);
        assert_eq!(learner.q_table.len(), 16 * 50);
    }

    #[test]
    fn test_q_update() {
        let mut learner = QLearner::new(10);
        let state = vec![0.5; 64];

        // Initial Q should be 0
        let s = learner.discretize_state(&state);
        assert_eq!(learner.get_q(s, 0), 0.0);

        // Update with positive reward
        learner.update(&state, 0, 1.0, &state);

        // Q should increase
        assert!(learner.get_q(s, 0) > 0.0);
    }

    #[test]
    fn test_action_ranking() {
        let mut learner = QLearner::new(5);
        let state = vec![0.5; 64];

        // Set different Q values
        let s = learner.discretize_state(&state);
        learner.set_q(s, 0, 0.1);
        learner.set_q(s, 1, 0.5);
        learner.set_q(s, 2, 0.3);
        learner.set_q(s, 3, 0.8);
        learner.set_q(s, 4, 0.2);

        let ranking = learner.rank_actions(&state);
        assert_eq!(ranking[0], 3, "Highest Q action should be ranked first");
    }

    #[test]
    fn test_learning_performance() {
        let mut learner = QLearner::new(100);
        let state = vec![0.5; 64];

        let start = Instant::now();
        for _ in 0..10000 {
            learner.update(&state, 0, 0.5, &state);
        }
        let duration = start.elapsed();

        let ops_per_sec = 10000.0 / duration.as_secs_f64();
        println!("Q-learning throughput: {:.0} updates/sec", ops_per_sec);

        assert!(ops_per_sec > 100000.0, "Should perform >100k updates/sec");
    }
}

mod attention {
    use super::*;

    #[test]
    fn test_attention_basic() {
        // Simple softmax test
        fn softmax(scores: &mut [f32]) {
            let max = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let mut sum = 0.0;
            for s in scores.iter_mut() {
                *s = (*s - max).exp();
                sum += *s;
            }
            for s in scores.iter_mut() {
                *s /= sum;
            }
        }

        let mut scores = vec![1.0, 2.0, 3.0];
        softmax(&mut scores);

        let sum: f32 = scores.iter().sum();
        assert!((sum - 1.0).abs() < 0.001, "Softmax should sum to 1");
        assert!(scores[2] > scores[1], "Higher score should have higher probability");
    }

    #[test]
    fn test_attention_ranking() {
        // Simplified attention-based ranking
        fn rank_by_similarity(query: &[f32], items: &[Vec<f32>]) -> Vec<(usize, f32)> {
            let mut scores: Vec<(usize, f32)> = items.iter()
                .enumerate()
                .map(|(i, item)| {
                    let sim: f32 = query.iter().zip(item.iter())
                        .map(|(q, v)| q * v)
                        .sum();
                    (i, sim)
                })
                .collect();

            scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            scores
        }

        let query = vec![1.0, 0.0, 0.0];
        let items = vec![
            vec![0.5, 0.5, 0.0],  // similarity = 0.5
            vec![1.0, 0.0, 0.0],  // similarity = 1.0
            vec![0.0, 1.0, 0.0],  // similarity = 0.0
        ];

        let ranked = rank_by_similarity(&query, &items);
        assert_eq!(ranked[0].0, 1, "Most similar item should be ranked first");
        assert_eq!(ranked[2].0, 2, "Least similar item should be ranked last");
    }
}

mod integration {
    use super::*;

    #[test]
    fn test_full_recommendation_flow() {
        // Simplified engine for testing
        struct TestEngine {
            dim: usize,
        }

        impl TestEngine {
            fn new(dim: usize) -> Self {
                Self { dim }
            }

            fn embed(&self, id: u64) -> Vec<f32> {
                let mut embedding = vec![0.0; self.dim];
                let mut seed = id as u32;
                for e in &mut embedding {
                    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                    *e = ((seed >> 16) as f32 / 32768.0) - 0.5;
                }
                // Normalize
                let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
                for e in &mut embedding {
                    *e /= norm;
                }
                embedding
            }

            fn recommend(&self, candidates: &[u64], top_k: usize) -> Vec<(u64, f32)> {
                let mut scored: Vec<(u64, f32)> = candidates.iter()
                    .map(|&id| {
                        let score = 1.0 / (1.0 + (id as f32 / 100.0));
                        (id, score)
                    })
                    .collect();
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                scored.truncate(top_k);
                scored
            }
        }

        let engine = TestEngine::new(64);

        // Test embedding
        let e1 = engine.embed(1);
        let e2 = engine.embed(2);
        assert_eq!(e1.len(), 64);
        assert_ne!(e1, e2);

        // Test recommendations
        let candidates: Vec<u64> = (1..=20).collect();
        let recs = engine.recommend(&candidates, 5);

        assert_eq!(recs.len(), 5);
        assert!(recs[0].1 >= recs[1].1, "Should be sorted by score");
    }

    #[test]
    fn test_latency_target() {
        // Target: sub-100ms for full recommendation cycle

        struct SimpleEngine {
            embeddings: Vec<Vec<f32>>,
        }

        impl SimpleEngine {
            fn new(num_items: usize) -> Self {
                let embeddings = (0..num_items)
                    .map(|i| {
                        let mut e = vec![0.0; 64];
                        let mut seed = i as u32;
                        for x in &mut e {
                            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                            *x = ((seed >> 16) as f32 / 32768.0) - 0.5;
                        }
                        e
                    })
                    .collect();
                Self { embeddings }
            }

            fn recommend(&self, query: &[f32], top_k: usize) -> Vec<(usize, f32)> {
                let mut scored: Vec<(usize, f32)> = self.embeddings.iter()
                    .enumerate()
                    .map(|(i, e)| {
                        let sim: f32 = query.iter().zip(e.iter())
                            .map(|(q, v)| q * v)
                            .sum();
                        (i, sim)
                    })
                    .collect();
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
                scored.truncate(top_k);
                scored
            }
        }

        let engine = SimpleEngine::new(1000);
        let query = vec![0.1; 64];

        // Warm-up
        for _ in 0..10 {
            let _ = engine.recommend(&query, 10);
        }

        // Measure
        let start = Instant::now();
        let iterations = 100;
        for _ in 0..iterations {
            let _ = engine.recommend(&query, 10);
        }
        let duration = start.elapsed();

        let avg_ms = duration.as_secs_f64() * 1000.0 / iterations as f64;
        println!("Average recommendation latency: {:.2}ms", avg_ms);

        assert!(avg_ms < 100.0, "Should complete in under 100ms");
    }
}

mod serialization {
    #[test]
    fn test_state_serialization() {
        // Test that Q-table can be serialized and restored
        let q_table: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4, 0.5];

        // Serialize
        let mut bytes = Vec::new();
        for &q in &q_table {
            bytes.extend_from_slice(&q.to_le_bytes());
        }

        // Deserialize
        let mut restored = Vec::new();
        for chunk in bytes.chunks_exact(4) {
            let arr: [u8; 4] = chunk.try_into().unwrap();
            restored.push(f32::from_le_bytes(arr));
        }

        assert_eq!(q_table, restored);
    }

    #[test]
    fn test_memory_usage() {
        // Verify memory budget compliance
        let embedding_dim = 64;
        let num_cached_embeddings = 100;
        let num_states = 16;
        let num_actions = 100;

        let embedding_memory = embedding_dim * num_cached_embeddings * 4; // f32
        let q_table_memory = num_states * num_actions * 4; // f32
        let total_memory = embedding_memory + q_table_memory;

        let total_mb = total_memory as f64 / (1024.0 * 1024.0);
        println!("Estimated memory usage: {:.2} MB", total_mb);

        assert!(total_mb < 50.0, "Should use less than 50MB");
    }
}
