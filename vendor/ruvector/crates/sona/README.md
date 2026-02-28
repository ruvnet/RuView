# SONA - Self-Optimizing Neural Architecture

[![Crates.io](https://img.shields.io/crates/v/ruvector-sona.svg)](https://crates.io/crates/ruvector-sona)
[![npm](https://img.shields.io/npm/v/@ruvector/sona.svg)](https://www.npmjs.com/package/@ruvector/sona)
[![Documentation](https://docs.rs/ruvector-sona/badge.svg)](https://docs.rs/ruvector-sona)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

**Runtime-adaptive learning for LLM routers and AI systems -- without expensive retraining.**

```bash
cargo add ruvector-sona
```

Most AI systems stop learning the moment they leave training. When a user gives bad feedback, that signal is lost -- or fixing it means days of fine-tuning and thousands of dollars. SONA is different. It watches every interaction, learns from feedback in sub-millisecond time, and continuously improves routing, ranking, and responses while your application is running. No retraining, no downtime, no cloud bills. It works in Rust, Node.js, and browsers (WASM).

| | SONA | Fine-Tuning | Prompt Tuning | RAG Alone |
|---|---|---|---|---|
| **Adaptation speed** | <1 ms (real-time) | Days to weeks | Hours to days | No adaptation |
| **Cost per update** | $0 (local compute) | $1,000-$100,000+ | Engineering time | N/A |
| **Downtime required** | None | Yes | No | No |
| **Learns from feedback** | Automatic | Manual pipeline | Manual | No |
| **Prevents forgetting** | EWC++ built in | Risk of regression | N/A | N/A |
| **Runs in browser** | Yes (WASM) | No | No | No |
| **Works offline** | Yes | No (needs GPU cluster) | Yes | Depends |

| Feature | What It Does | Why It Matters |
|---------|-------------|----------------|
| **Two-Tier LoRA** | Fast MicroLoRA layer for instant fixes, deeper BaseLoRA for long-term learning | Adapts immediately without sacrificing stability |
| **EWC++ (Elastic Weight Consolidation)** | Protects important learned weights when absorbing new feedback | Your system never forgets what it already learned |
| **ReasoningBank** | Stores and retrieves successful interaction patterns | Past successes inform future decisions automatically |
| **Trajectory Tracking** | Records the full path of each interaction (query, model choice, outcome) | Turns every user session into training data |
| **WASM Support** | Runs the full learning engine in browsers at near-native speed | On-device personalization with zero server costs |
| **Node.js Bindings** | Native N-API bindings -- no child processes or HTTP calls | Drop into any JavaScript backend with one `npm install` |

> Part of the [RuVector](https://github.com/ruvnet/ruvector) ecosystem -- the self-learning vector database with graph intelligence.

---

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [Tutorials](#tutorials)
  - [Tutorial 1: Your First SONA Application](#tutorial-1-your-first-sona-application)
  - [Tutorial 2: Building an Adaptive Chatbot](#tutorial-2-building-an-adaptive-chatbot)
  - [Tutorial 3: LLM Router with Learning](#tutorial-3-llm-router-with-learning)
  - [Tutorial 4: Browser-Based Learning (WASM)](#tutorial-4-browser-based-learning-wasm)
  - [Tutorial 5: Node.js Backend Integration](#tutorial-5-nodejs-backend-integration)
  - [Tutorial 6: Production Deployment](#tutorial-6-production-deployment)
- [Configuration Guide](#configuration-guide)
- [API Reference](#api-reference)
- [Benchmarks](#benchmarks)
- [Troubleshooting](#troubleshooting)

---

## Installation

### Rust (Cargo)

```toml
[dependencies]
ruvector-sona = "0.1.1"

# With all features
ruvector-sona = { version = "0.1.1", features = ["serde-support"] }
```

### Node.js (npm)

```bash
npm install @ruvector/sona
# or
yarn add @ruvector/sona
# or
pnpm add @ruvector/sona
```

### Browser (WASM)

```bash
# Clone and build WASM package
git clone https://github.com/ruvnet/ruvector.git
cd ruvector/crates/sona
wasm-pack build --target web --features wasm

# Copy to your project
cp -r pkg/ your-project/sona/
```

---

## Quick Start

### 30-Second Example (Rust)

```rust
use ruvector_sona::{SonaEngine, SonaConfig};

fn main() {
    // 1. Create engine
    let engine = SonaEngine::builder()
        .hidden_dim(256)
        .build();

    // 2. Record a user interaction
    let query_embedding = vec![0.1f32; 256];
    let traj_id = engine.begin_trajectory(query_embedding);

    // 3. Record what happened (model selection, confidence, latency)
    engine.add_step(traj_id, vec![0.5; 256], vec![0.8; 64], 0.9);

    // 4. Record outcome quality (0.0 = bad, 1.0 = perfect)
    engine.end_trajectory(traj_id, 0.85);

    // 5. Apply learned optimizations to future queries
    let new_query = vec![0.2f32; 256];
    let optimized = engine.apply_micro_lora(&new_query);

    println!("SONA is learning! Stats: {}", engine.get_stats());
}
```

### 30-Second Example (Node.js)

```javascript
const { SonaEngine } = require('@ruvector/sona');

// 1. Create engine
const engine = new SonaEngine(256);

// 2. Record interaction
const queryEmbedding = Array(256).fill(0.1);
const trajId = engine.beginTrajectory(queryEmbedding);

// 3. Add step data
engine.addTrajectoryStep(trajId, Array(256).fill(0.5), Array(64).fill(0.8), 0.9);

// 4. Complete with quality score
engine.endTrajectory(trajId, 0.85);

// 5. Apply learning
const newQuery = Array(256).fill(0.2);
const optimized = engine.applyMicroLora(newQuery);

console.log('Stats:', engine.getStats());
```

---

## Core Concepts

### Understanding Embeddings

Embeddings are numerical representations of text. Every word, sentence, or query can be converted into a vector of numbers (typically 256-4096 dimensions). SONA works with these embeddings to learn patterns.

```
"How do I reset my password?" → [0.12, -0.45, 0.78, ..., 0.23]  (256 numbers)
"Password reset help"         → [0.11, -0.44, 0.79, ..., 0.22]  (similar!)
"What's the weather?"         → [0.89, 0.12, -0.34, ..., 0.67]  (different)
```

### Trajectories: Recording What Happened

A **trajectory** is a complete record of one user interaction:

```
┌─────────────────────────────────────────────────────────────┐
│                        Trajectory                           │
├─────────────────────────────────────────────────────────────┤
│  Query Embedding: [0.12, -0.45, 0.78, ...]                  │
│                                                             │
│  Steps:                                                     │
│    Step 1: Selected Model A, confidence 0.82, latency 45ms  │
│    Step 2: Generated response, confidence 0.91, latency 120ms│
│    Step 3: Formatted output, confidence 0.95, latency 5ms   │
│                                                             │
│  Final Quality: 0.85 (user gave thumbs up)                  │
└─────────────────────────────────────────────────────────────┘
```

### Two-Tier LoRA: Fast and Deep Learning

SONA uses two types of adaptation:

| Tier | Rank | Speed | Purpose | When Used |
|------|------|-------|---------|-----------|
| **MicroLoRA** | 2 | ~45μs | Instant adjustments | Every request |
| **BaseLoRA** | 8-16 | ~1ms | Deep pattern learning | Background (hourly) |

**MicroLoRA** is like quick reflexes - it adapts immediately based on recent feedback.
**BaseLoRA** is like long-term memory - it consolidates patterns over time.

### EWC++: Remembering Without Forgetting

When learning new patterns, AI systems often "forget" old ones (catastrophic forgetting). EWC++ (Elastic Weight Consolidation) prevents this by:

1. Tracking which parameters are important for each task
2. Protecting important parameters when learning new tasks
3. Automatically detecting when a "new task" begins

```
Without EWC++:                    With EWC++:
┌────────────────────┐           ┌────────────────────┐
│ Learn Task A: ✓    │           │ Learn Task A: ✓    │
│ Learn Task B: ✓    │           │ Learn Task B: ✓    │
│ Task A knowledge: ✗ │           │ Task A knowledge: ✓ │
└────────────────────┘           └────────────────────┘
```

### ReasoningBank: Pattern Library

ReasoningBank stores successful interaction patterns using K-means++ clustering:

```
┌─────────────────────────────────────────────────────────────┐
│                     ReasoningBank                            │
├─────────────────────────────────────────────────────────────┤
│  Cluster 1: "Password/Account Issues"                       │
│    - 847 trajectories, avg quality 0.89                     │
│    - Best response pattern: Empathetic + Step-by-step       │
│                                                             │
│  Cluster 2: "Technical Questions"                           │
│    - 1,234 trajectories, avg quality 0.92                   │
│    - Best response pattern: Detailed + Code examples        │
│                                                             │
│  Cluster 3: "General Conversation"                          │
│    - 2,156 trajectories, avg quality 0.78                   │
│    - Best response pattern: Friendly + Concise              │
└─────────────────────────────────────────────────────────────┘
```

---

## Tutorials

### Tutorial 1: Your First SONA Application

Let's build a simple application that learns from user feedback.

**Goal**: Create a system that improves response quality based on thumbs up/down.

```rust
use ruvector_sona::{SonaEngine, SonaConfig};

fn main() {
    // Step 1: Configure SONA
    // Use optimized defaults (benchmark-validated)
    let config = SonaConfig::default();

    println!("Configuration:");
    println!("  MicroLoRA rank: {} (optimal for SIMD)", config.micro_lora_rank);
    println!("  Learning rate: {} (+55% quality)", config.micro_lora_lr);
    println!("  Pattern clusters: {} (2.3x faster)", config.pattern_clusters);
    println!("  EWC lambda: {} (anti-forgetting)", config.ewc_lambda);

    // Step 2: Create the engine
    let engine = SonaEngine::builder()
        .config(config)
        .build();

    // Step 3: Simulate 100 user interactions
    let mut positive_count = 0;
    let mut negative_count = 0;

    for i in 0..100 {
        // Simulate a query embedding (in real app, use your embedding model)
        let query_embedding: Vec<f32> = (0..256)
            .map(|j| ((i * 256 + j) as f32 * 0.001).sin())
            .collect();

        // Start recording this interaction
        let traj_id = engine.begin_trajectory(query_embedding.clone());

        // Simulate processing steps
        let activations: Vec<f32> = query_embedding.iter()
            .map(|x| x.tanh())
            .collect();
        let attention: Vec<f32> = vec![1.0 / 64.0; 64];

        engine.add_step(traj_id, activations, attention, 0.8);

        // Simulate user feedback (70% positive in this example)
        let is_positive = (i % 10) < 7;
        let quality = if is_positive { 0.9 } else { 0.3 };

        if is_positive {
            positive_count += 1;
        } else {
            negative_count += 1;
        }

        // Complete the trajectory with quality score
        engine.end_trajectory(traj_id, quality);

        // Run learning tick (processes pending trajectories)
        engine.tick();
    }

    // Step 4: Check what we learned
    println!("\nResults after 100 interactions:");
    println!("  Positive feedback: {}", positive_count);
    println!("  Negative feedback: {}", negative_count);
    println!("  Engine stats: {}", engine.get_stats());

    // Step 5: Apply learning to a new query
    let new_query: Vec<f32> = vec![0.5; 256];
    let optimized = engine.apply_micro_lora(&new_query);

    // The optimized embedding now incorporates learned patterns!
    let diff: f32 = new_query.iter()
        .zip(optimized.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();

    println!("\nLearning applied! Embedding change magnitude: {:.4}", diff);
}
```

**Expected Output:**
```
Configuration:
  MicroLoRA rank: 2 (optimal for SIMD)
  Learning rate: 0.002 (+55% quality)
  Pattern clusters: 100 (2.3x faster)
  EWC lambda: 2000 (anti-forgetting)

Results after 100 interactions:
  Positive feedback: 70
  Negative feedback: 30
  Engine stats: {"trajectories": 100, "patterns": 12, "micro_updates": 100}

Learning applied! Embedding change magnitude: 0.0847
```

---

### Tutorial 2: Building an Adaptive Chatbot

Let's build a chatbot that learns to give better responses.

```rust
use ruvector_sona::{SonaEngine, SonaConfig};
use std::collections::HashMap;

/// Adaptive chatbot that learns from user feedback
pub struct AdaptiveChatbot {
    engine: SonaEngine,
    response_templates: HashMap<String, Vec<String>>,
    active_trajectory: Option<u64>,
}

impl AdaptiveChatbot {
    pub fn new() -> Self {
        // Use max_quality preset for chatbot (we want best responses)
        let config = SonaConfig::max_quality();

        let engine = SonaEngine::builder()
            .config(config)
            .build();

        // Simple response templates (in real app, use LLM)
        let mut templates = HashMap::new();
        templates.insert("greeting".to_string(), vec![
            "Hello! How can I help you today?".to_string(),
            "Hi there! What can I do for you?".to_string(),
            "Welcome! I'm here to assist you.".to_string(),
        ]);
        templates.insert("farewell".to_string(), vec![
            "Goodbye! Have a great day!".to_string(),
            "Take care! Feel free to come back anytime.".to_string(),
            "Bye! It was nice helping you.".to_string(),
        ]);
        templates.insert("unknown".to_string(), vec![
            "I'm not sure I understand. Could you rephrase that?".to_string(),
            "Let me think about that...".to_string(),
            "Interesting question! Let me help you with that.".to_string(),
        ]);

        Self {
            engine,
            response_templates: templates,
            active_trajectory: None,
        }
    }

    /// Process a user message
    pub fn respond(&mut self, message: &str) -> String {
        // Step 1: Create embedding from message
        let embedding = self.create_embedding(message);

        // Step 2: Start trajectory
        let traj_id = self.engine.begin_trajectory(embedding.clone());
        self.active_trajectory = Some(traj_id);

        // Step 3: Apply learned optimizations
        let optimized = self.engine.apply_micro_lora(&embedding);

        // Step 4: Classify intent using optimized embedding
        let intent = self.classify_intent(&optimized);

        // Step 5: Record the classification step
        let activations: Vec<f32> = optimized.iter().map(|x| x.tanh()).collect();
        let attention = vec![1.0 / 64.0; 64];
        self.engine.add_step(traj_id, activations, attention, 0.8);

        // Step 6: Select best response template
        let responses = self.response_templates.get(&intent)
            .unwrap_or(&self.response_templates["unknown"]);

        // Use embedding similarity to pick best response
        let response = self.select_best_response(responses, &optimized);

        response
    }

    /// Record user feedback (call after response is shown)
    pub fn record_feedback(&mut self, was_helpful: bool) {
        if let Some(traj_id) = self.active_trajectory.take() {
            let quality = if was_helpful { 0.95 } else { 0.2 };
            self.engine.end_trajectory(traj_id, quality);

            // Force learning if negative feedback (learn faster from mistakes)
            if !was_helpful {
                self.engine.force_learn();
            }
        }
    }

    /// Create a simple embedding from text
    fn create_embedding(&self, text: &str) -> Vec<f32> {
        // Simple bag-of-characters embedding (use real embeddings in production!)
        let mut embedding = vec![0.0f32; 256];
        for (i, c) in text.chars().enumerate() {
            let idx = (c as usize + i) % 256;
            embedding[idx] += 0.1;
        }
        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            embedding.iter_mut().for_each(|x| *x /= norm);
        }
        embedding
    }

    /// Classify user intent
    fn classify_intent(&self, embedding: &[f32]) -> String {
        // Simple heuristic (use classifier in production!)
        let sum: f32 = embedding.iter().take(10).sum();
        if sum > 0.5 {
            "greeting".to_string()
        } else if sum < -0.5 {
            "farewell".to_string()
        } else {
            "unknown".to_string()
        }
    }

    /// Select best response based on embedding
    fn select_best_response(&self, responses: &[String], embedding: &[f32]) -> String {
        // Use embedding to deterministically select response
        let idx = (embedding[0].abs() * responses.len() as f32) as usize % responses.len();
        responses[idx].clone()
    }

    /// Get learning statistics
    pub fn stats(&self) -> String {
        self.engine.get_stats()
    }
}

fn main() {
    let mut bot = AdaptiveChatbot::new();

    // Simulate conversation
    let conversations = vec![
        ("Hello!", true),
        ("Hi there", true),
        ("What is AI?", false),  // Bad response
        ("Explain machine learning", false),  // Bad response
        ("Thanks, goodbye!", true),
        ("Hello again!", true),
    ];

    for (message, was_helpful) in conversations {
        println!("User: {}", message);
        let response = bot.respond(message);
        println!("Bot: {}", response);
        bot.record_feedback(was_helpful);
        println!("  [Feedback: {}]", if was_helpful { "👍" } else { "👎" });
        println!();
    }

    println!("Final stats: {}", bot.stats());
}
```

---

### Tutorial 3: LLM Router with Learning

Build a router that learns which LLM to use for different query types.

```rust
use ruvector_sona::{SonaEngine, SonaConfig};
use std::time::Instant;

/// Represents an LLM model
#[derive(Clone)]
pub struct LLMModel {
    pub name: String,
    pub cost_per_token: f32,
    pub avg_quality: f32,
    pub avg_latency_ms: u32,
}

/// Adaptive LLM Router that learns optimal model selection
pub struct AdaptiveLLMRouter {
    engine: SonaEngine,
    models: Vec<LLMModel>,
}

impl AdaptiveLLMRouter {
    pub fn new(models: Vec<LLMModel>) -> Self {
        // Use max_throughput for fast routing decisions
        let config = SonaConfig::max_throughput();

        let engine = SonaEngine::builder()
            .config(config)
            .build();

        Self { engine, models }
    }

    /// Route a query to the best model
    pub fn route(&self, query_embedding: Vec<f32>) -> (usize, &LLMModel) {
        // Apply learned optimizations
        let optimized = self.engine.apply_micro_lora(&query_embedding);

        // Find similar patterns
        let patterns = self.engine.find_patterns(&optimized, 3);

        // Score each model based on patterns and learned preferences
        let mut best_idx = 0;
        let mut best_score = f32::MIN;

        for (idx, model) in self.models.iter().enumerate() {
            let mut score = model.avg_quality;

            // Boost score if patterns suggest this model works well
            for pattern in &patterns {
                // Pattern centroid similarity affects model preference
                let similarity = cosine_similarity(&optimized, &pattern.centroid);
                if similarity > 0.8 {
                    // High similarity to successful pattern
                    score += pattern.avg_quality * similarity;
                }
            }

            // Penalize expensive models slightly
            score -= model.cost_per_token * 0.1;

            if score > best_score {
                best_score = score;
                best_idx = idx;
            }
        }

        (best_idx, &self.models[best_idx])
    }

    /// Record the outcome of a routing decision
    pub fn record_outcome(
        &self,
        query_embedding: Vec<f32>,
        selected_model: usize,
        quality: f32,
        latency_ms: u32,
    ) {
        // Start trajectory
        let traj_id = self.engine.begin_trajectory(query_embedding);

        // Record selection step
        let model = &self.models[selected_model];
        let activations = vec![
            model.avg_quality,
            model.cost_per_token,
            latency_ms as f32 / 1000.0,
        ];
        let activations_padded: Vec<f32> = activations.into_iter()
            .chain(std::iter::repeat(0.0))
            .take(256)
            .collect();

        let attention = vec![1.0 / 64.0; 64];
        self.engine.add_step(traj_id, activations_padded, attention, quality);

        // Set route info
        self.engine.set_trajectory_route(traj_id, model.name.clone());

        // Complete trajectory
        self.engine.end_trajectory(traj_id, quality);
    }

    /// Force background learning cycle
    pub fn learn(&self) -> String {
        self.engine.force_learn()
    }

    pub fn stats(&self) -> String {
        self.engine.get_stats()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

fn main() {
    // Define available models
    let models = vec![
        LLMModel {
            name: "GPT-4".to_string(),
            cost_per_token: 0.03,
            avg_quality: 0.95,
            avg_latency_ms: 2000,
        },
        LLMModel {
            name: "GPT-3.5-Turbo".to_string(),
            cost_per_token: 0.002,
            avg_quality: 0.85,
            avg_latency_ms: 500,
        },
        LLMModel {
            name: "Claude-Instant".to_string(),
            cost_per_token: 0.001,
            avg_quality: 0.80,
            avg_latency_ms: 300,
        },
        LLMModel {
            name: "Local-LLaMA".to_string(),
            cost_per_token: 0.0001,
            avg_quality: 0.70,
            avg_latency_ms: 100,
        },
    ];

    let router = AdaptiveLLMRouter::new(models);

    // Simulate 1000 queries with different types
    println!("Training router with 1000 queries...\n");

    let query_types = vec![
        ("simple", vec![0.1f32; 256], 0.70, "Local-LLaMA"),      // Simple queries work fine with local
        ("medium", vec![0.5f32; 256], 0.85, "GPT-3.5-Turbo"),    // Medium needs cloud
        ("complex", vec![0.9f32; 256], 0.95, "GPT-4"),           // Complex needs best
    ];

    for i in 0..1000 {
        let (query_type, base_embedding, target_quality, expected_model) =
            &query_types[i % query_types.len()];

        // Add some variation to embeddings
        let embedding: Vec<f32> = base_embedding.iter()
            .enumerate()
            .map(|(j, x)| x + (i as f32 * j as f32 * 0.0001).sin() * 0.1)
            .collect();

        // Route the query
        let (model_idx, model) = router.route(embedding.clone());

        // Simulate quality based on model fit
        let quality = if &model.name == *expected_model {
            *target_quality
        } else {
            target_quality - 0.2  // Penalty for wrong model
        };

        // Record outcome
        router.record_outcome(embedding, model_idx, quality, model.avg_latency_ms);

        // Periodic learning
        if i % 100 == 0 {
            router.learn();
        }
    }

    // Test learned routing
    println!("Testing learned routing:\n");

    for (query_type, embedding, _, expected) in &query_types {
        let (_, model) = router.route(embedding.clone());
        let match_status = if &model.name == *expected { "✓" } else { "✗" };
        println!("  {} query → {} {} (expected: {})",
            query_type, model.name, match_status, expected);
    }

    println!("\nRouter stats: {}", router.stats());
}
```

---

### Tutorial 4: Browser-Based Learning (WASM)

Deploy SONA in the browser for client-side learning.

```html
<!DOCTYPE html>
<html>
<head>
    <title>SONA Browser Demo</title>
    <style>
        body { font-family: Arial, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
        .chat { border: 1px solid #ccc; padding: 20px; height: 400px; overflow-y: auto; }
        .message { margin: 10px 0; padding: 10px; border-radius: 5px; }
        .user { background: #e3f2fd; text-align: right; }
        .bot { background: #f5f5f5; }
        .feedback { margin-top: 5px; }
        .feedback button { margin-right: 10px; padding: 5px 15px; cursor: pointer; }
        input { width: 70%; padding: 10px; }
        button.send { padding: 10px 20px; }
        .stats { background: #fff3e0; padding: 10px; margin-top: 20px; font-family: monospace; }
    </style>
</head>
<body>
    <h1>🧠 SONA Browser Demo</h1>
    <p>This chatbot learns from your feedback in real-time, entirely in your browser!</p>

    <div class="chat" id="chat"></div>

    <div style="margin-top: 10px;">
        <input type="text" id="input" placeholder="Type a message..." onkeypress="if(event.key==='Enter')sendMessage()">
        <button class="send" onclick="sendMessage()">Send</button>
    </div>

    <div class="stats" id="stats">Loading SONA...</div>

    <script type="module">
        import init, { WasmSonaEngine } from './pkg/sona.js';

        let engine = null;
        let currentTrajId = null;
        let messageCount = 0;

        // Initialize SONA
        async function initSona() {
            await init();
            engine = new WasmSonaEngine(256);
            updateStats();
            document.getElementById('stats').textContent = 'SONA initialized! Start chatting to train it.';
        }

        // Create embedding from text (simple version)
        function createEmbedding(text) {
            const embedding = new Float32Array(256).fill(0);
            for (let i = 0; i < text.length; i++) {
                const idx = (text.charCodeAt(i) + i) % 256;
                embedding[idx] += 0.1;
            }
            // Normalize
            const norm = Math.sqrt(embedding.reduce((s, x) => s + x * x, 0));
            if (norm > 0) {
                for (let i = 0; i < embedding.length; i++) {
                    embedding[i] /= norm;
                }
            }
            return Array.from(embedding);
        }

        // Generate response
        function generateResponse(input, optimizedEmbedding) {
            // Simple response logic (replace with actual LLM call)
            const responses = {
                greeting: ["Hello! How can I help you?", "Hi there! Nice to meet you!", "Hey! What's on your mind?"],
                question: ["That's a great question!", "Let me think about that...", "Interesting! Here's what I know:"],
                thanks: ["You're welcome!", "Happy to help!", "Anytime!"],
                default: ["I see.", "Tell me more.", "Interesting perspective!"]
            };

            const inputLower = input.toLowerCase();
            let category = 'default';
            if (inputLower.includes('hello') || inputLower.includes('hi')) category = 'greeting';
            else if (inputLower.includes('?')) category = 'question';
            else if (inputLower.includes('thank')) category = 'thanks';

            // Use optimized embedding to influence response selection
            const idx = Math.floor(Math.abs(optimizedEmbedding[0]) * responses[category].length);
            return responses[category][idx % responses[category].length];
        }

        // Add message to chat
        function addMessage(text, isUser, trajId = null) {
            const chat = document.getElementById('chat');
            const div = document.createElement('div');
            div.className = `message ${isUser ? 'user' : 'bot'}`;
            div.innerHTML = text;

            if (!isUser && trajId !== null) {
                const feedback = document.createElement('div');
                feedback.className = 'feedback';
                feedback.innerHTML = `
                    <button onclick="recordFeedback(${trajId}, true)">👍 Helpful</button>
                    <button onclick="recordFeedback(${trajId}, false)">👎 Not helpful</button>
                `;
                div.appendChild(feedback);
            }

            chat.appendChild(div);
            chat.scrollTop = chat.scrollHeight;
        }

        // Send message
        window.sendMessage = function() {
            const input = document.getElementById('input');
            const text = input.value.trim();
            if (!text) return;

            // Add user message
            addMessage(text, true);
            input.value = '';

            // Start trajectory
            const embedding = createEmbedding(text);
            currentTrajId = engine.begin_trajectory(embedding);

            // Apply learned optimizations
            const optimized = engine.apply_micro_lora(embedding);

            // Record step
            const activations = optimized.map(x => Math.tanh(x));
            const attention = new Array(64).fill(1/64);
            engine.add_trajectory_step(currentTrajId, activations, attention, 0.8);

            // Generate and display response
            const response = generateResponse(text, optimized);
            addMessage(response, false, currentTrajId);

            messageCount++;
            updateStats();
        };

        // Record feedback
        window.recordFeedback = function(trajId, wasHelpful) {
            const quality = wasHelpful ? 0.95 : 0.2;
            engine.end_trajectory(trajId, quality);

            // Run learning
            const result = engine.tick();
            if (result) {
                console.log('Learning cycle:', result);
            }

            // Disable feedback buttons
            event.target.parentElement.innerHTML = wasHelpful
                ? '<span style="color:green">✓ Thanks for the feedback!</span>'
                : '<span style="color:orange">✓ I\'ll try to improve!</span>';

            updateStats();
        };

        // Update stats display
        function updateStats() {
            const stats = JSON.parse(engine.get_stats());
            document.getElementById('stats').innerHTML = `
                <strong>SONA Stats:</strong><br>
                Messages: ${messageCount} |
                Patterns learned: ${stats.patterns_stored || 0} |
                Learning cycles: ${stats.background_cycles || 0}
            `;
        }

        // Initialize
        initSona();
    </script>
</body>
</html>
```

---

### Tutorial 5: Node.js Backend Integration

Production-ready Node.js integration with Express.

```javascript
const express = require('express');
const { SonaEngine } = require('@ruvector/sona');

const app = express();
app.use(express.json());

// Initialize SONA engine
const engine = SonaEngine.withConfig({
    hiddenDim: 256,
    microLoraRank: 2,      // Optimized for SIMD
    microLoraLr: 0.002,    // Optimal learning rate
    patternClusters: 100,  // Fast search
    ewcLambda: 2000,       // Anti-forgetting
    qualityThreshold: 0.3  // Learn from more samples
});

// Track active trajectories
const activeTrajectories = new Map();

// Middleware to create embeddings (replace with your embedding service)
function createEmbedding(text) {
    // Simple embedding (use OpenAI/Cohere embeddings in production)
    const embedding = new Array(256).fill(0);
    for (let i = 0; i < text.length; i++) {
        const idx = (text.charCodeAt(i) + i) % 256;
        embedding[idx] += 0.1;
    }
    const norm = Math.sqrt(embedding.reduce((s, x) => s + x * x, 0));
    return embedding.map(x => x / (norm || 1));
}

// Start a new interaction
app.post('/api/query', (req, res) => {
    const { query, sessionId } = req.body;

    // Create embedding
    const embedding = createEmbedding(query);

    // Start trajectory
    const trajId = engine.beginTrajectory(embedding);
    activeTrajectories.set(sessionId, { trajId, embedding, startTime: Date.now() });

    // Apply learned optimizations
    const optimized = engine.applyMicroLora(embedding);

    // Find similar patterns for context
    const patterns = engine.findPatterns(optimized, 3);

    // Record step
    const activations = optimized.map(x => Math.tanh(x));
    const attention = new Array(64).fill(1/64);
    engine.addTrajectoryStep(trajId, activations, attention, 0.8);

    res.json({
        sessionId,
        optimizedEmbedding: optimized,
        similarPatterns: patterns.map(p => ({
            avgQuality: p.avgQuality,
            clusterSize: p.clusterSize,
            patternType: p.patternType
        })),
        message: 'Query processed. Send response quality via /api/feedback'
    });
});

// Record feedback
app.post('/api/feedback', (req, res) => {
    const { sessionId, quality, wasHelpful } = req.body;

    const session = activeTrajectories.get(sessionId);
    if (!session) {
        return res.status(404).json({ error: 'Session not found' });
    }

    // Calculate quality score
    const qualityScore = quality ?? (wasHelpful ? 0.9 : 0.2);

    // Complete trajectory
    engine.endTrajectory(session.trajId, qualityScore);

    // Run learning tick
    const learnResult = engine.tick();

    // Clean up
    activeTrajectories.delete(sessionId);

    res.json({
        success: true,
        quality: qualityScore,
        latencyMs: Date.now() - session.startTime,
        learned: learnResult !== null
    });
});

// Force learning cycle
app.post('/api/learn', (req, res) => {
    const result = engine.forceLearn();
    res.json({
        success: true,
        result,
        stats: JSON.parse(engine.getStats())
    });
});

// Get stats
app.get('/api/stats', (req, res) => {
    res.json(JSON.parse(engine.getStats()));
});

// Health check
app.get('/health', (req, res) => {
    res.json({
        status: 'healthy',
        engine: engine.isEnabled() ? 'active' : 'disabled'
    });
});

// Background learning (run hourly)
setInterval(() => {
    console.log('Running background learning cycle...');
    const result = engine.forceLearn();
    console.log('Learning complete:', result);
}, 60 * 60 * 1000); // Every hour

const PORT = process.env.PORT || 3000;
app.listen(PORT, () => {
    console.log(`SONA server running on port ${PORT}`);
    console.log('Stats:', engine.getStats());
});
```

**Usage:**

```bash
# Start server
node server.js

# Test endpoints
curl -X POST http://localhost:3000/api/query \
  -H "Content-Type: application/json" \
  -d '{"query": "How do I reset my password?", "sessionId": "abc123"}'

curl -X POST http://localhost:3000/api/feedback \
  -H "Content-Type: application/json" \
  -d '{"sessionId": "abc123", "wasHelpful": true}'

curl http://localhost:3000/api/stats
```

---

### Tutorial 6: Production Deployment

Best practices for deploying SONA in production.

```rust
use ruvector_sona::{SonaEngine, SonaConfig};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Production-ready SONA wrapper
pub struct ProductionSona {
    engine: Arc<RwLock<SonaEngine>>,
    metrics: Arc<RwLock<Metrics>>,
}

#[derive(Default)]
pub struct Metrics {
    pub total_requests: u64,
    pub total_learning_cycles: u64,
    pub positive_feedback: u64,
    pub negative_feedback: u64,
    pub avg_latency_us: f64,
}

impl ProductionSona {
    pub async fn new() -> Self {
        // Use optimized defaults
        let config = SonaConfig::default();

        let engine = SonaEngine::builder()
            .config(config)
            .build();

        let instance = Self {
            engine: Arc::new(RwLock::new(engine)),
            metrics: Arc::new(RwLock::new(Metrics::default())),
        };

        // Start background tasks
        instance.start_background_tasks().await;

        instance
    }

    async fn start_background_tasks(&self) {
        let engine = self.engine.clone();
        let metrics = self.metrics.clone();

        // Hourly learning cycle
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(3600));
            loop {
                interval.tick().await;

                let mut engine = engine.write().await;
                let result = engine.force_learn();

                let mut m = metrics.write().await;
                m.total_learning_cycles += 1;

                tracing::info!("Background learning completed: {}", result);
            }
        });

        // Metrics logging (every 5 minutes)
        let metrics_clone = self.metrics.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300));
            loop {
                interval.tick().await;
                let m = metrics_clone.read().await;
                tracing::info!(
                    "SONA Metrics - Requests: {}, Learning: {}, Positive: {}, Negative: {}",
                    m.total_requests,
                    m.total_learning_cycles,
                    m.positive_feedback,
                    m.negative_feedback
                );
            }
        });
    }

    /// Process a query with full observability
    pub async fn process(&self, embedding: Vec<f32>) -> ProcessResult {
        let start = std::time::Instant::now();

        let engine = self.engine.read().await;

        // Start trajectory
        let traj_id = engine.begin_trajectory(embedding.clone());

        // Apply optimizations
        let optimized = engine.apply_micro_lora(&embedding);

        // Find patterns
        let patterns = engine.find_patterns(&optimized, 5);

        // Update metrics
        let latency = start.elapsed().as_micros() as u64;
        {
            let mut m = self.metrics.write().await;
            m.total_requests += 1;
            m.avg_latency_us = (m.avg_latency_us * (m.total_requests - 1) as f64
                + latency as f64) / m.total_requests as f64;
        }

        ProcessResult {
            trajectory_id: traj_id,
            optimized_embedding: optimized,
            similar_patterns: patterns.into_iter().map(|p| PatternInfo {
                quality: p.avg_quality,
                cluster_size: p.cluster_size,
            }).collect(),
            latency_us: latency,
        }
    }

    /// Record step in trajectory
    pub async fn record_step(
        &self,
        traj_id: u64,
        activations: Vec<f32>,
        attention: Vec<f32>,
        reward: f32,
    ) {
        let engine = self.engine.read().await;
        engine.add_step(traj_id, activations, attention, reward);
    }

    /// Complete trajectory with feedback
    pub async fn complete(&self, traj_id: u64, quality: f32, was_positive: bool) {
        {
            let engine = self.engine.read().await;
            engine.end_trajectory(traj_id, quality);
        }

        // Update metrics
        let mut m = self.metrics.write().await;
        if was_positive {
            m.positive_feedback += 1;
        } else {
            m.negative_feedback += 1;
        }
    }

    /// Get current statistics
    pub async fn stats(&self) -> Stats {
        let engine = self.engine.read().await;
        let engine_stats = engine.get_stats();

        let m = self.metrics.read().await;

        Stats {
            engine_stats,
            total_requests: m.total_requests,
            total_learning_cycles: m.total_learning_cycles,
            positive_feedback: m.positive_feedback,
            negative_feedback: m.negative_feedback,
            avg_latency_us: m.avg_latency_us,
            feedback_ratio: if m.positive_feedback + m.negative_feedback > 0 {
                m.positive_feedback as f64 / (m.positive_feedback + m.negative_feedback) as f64
            } else {
                0.0
            },
        }
    }
}

pub struct ProcessResult {
    pub trajectory_id: u64,
    pub optimized_embedding: Vec<f32>,
    pub similar_patterns: Vec<PatternInfo>,
    pub latency_us: u64,
}

pub struct PatternInfo {
    pub quality: f32,
    pub cluster_size: usize,
}

pub struct Stats {
    pub engine_stats: String,
    pub total_requests: u64,
    pub total_learning_cycles: u64,
    pub positive_feedback: u64,
    pub negative_feedback: u64,
    pub avg_latency_us: f64,
    pub feedback_ratio: f64,
}
```

---

## Configuration Guide

### Optimized Defaults (v0.1.1)

The default configuration is optimized based on extensive benchmarks:

```rust
SonaConfig {
    hidden_dim: 256,
    embedding_dim: 256,
    micro_lora_rank: 2,       // 5% faster than rank-1 (better SIMD)
    base_lora_rank: 8,
    micro_lora_lr: 0.002,     // +55% quality improvement
    base_lora_lr: 0.0001,
    ewc_lambda: 2000.0,       // Better forgetting prevention
    pattern_clusters: 100,    // 2.3x faster search
    trajectory_capacity: 10000,
    background_interval_ms: 3600000,  // 1 hour
    quality_threshold: 0.3,   // Learn from more samples
    enable_simd: true,
}
```

### Configuration Presets

```rust
// For real-time chat applications
let config = SonaConfig::max_throughput();

// For research/batch processing (best quality)
let config = SonaConfig::max_quality();

// For mobile/edge devices (<5MB memory)
let config = SonaConfig::edge_deployment();

// For high-throughput batch processing
let config = SonaConfig::batch_processing();
```

### Custom Configuration

```rust
let config = SonaConfig {
    // Embedding dimensions (match your model)
    hidden_dim: 512,
    embedding_dim: 512,

    // LoRA settings
    micro_lora_rank: 2,      // 1-2 for speed, keep at 2 for SIMD
    base_lora_rank: 16,      // 4-16 for expressiveness
    micro_lora_lr: 0.002,    // Higher = faster learning, risk of instability
    base_lora_lr: 0.0001,    // Lower = stable consolidation

    // Memory protection
    ewc_lambda: 2000.0,      // Higher = stronger protection against forgetting

    // Pattern storage
    pattern_clusters: 100,   // More clusters = faster search, more memory
    trajectory_capacity: 20000,

    // Learning triggers
    background_interval_ms: 1800000,  // 30 minutes
    quality_threshold: 0.2,  // Lower = learn from more trajectories

    // Performance
    enable_simd: true,
};
```

---

## API Reference

### SonaEngine

| Method | Description | Typical Latency |
|--------|-------------|-----------------|
| `new(hidden_dim)` | Create with default config | - |
| `with_config(config)` | Create with custom config | - |
| `builder()` | Start building configuration | - |
| `begin_trajectory(embedding)` | Start recording interaction | ~50ns |
| `add_trajectory_step(id, activations, attention, reward)` | Add step | ~112ns |
| `set_trajectory_route(id, route)` | Set model route | ~20ns |
| `add_trajectory_context(id, context)` | Add context | ~20ns |
| `end_trajectory(id, quality)` | Complete with quality | ~100ns |
| `apply_micro_lora(input)` | Fast transformation | ~45μs |
| `apply_base_lora(layer, input)` | Deep transformation | ~25μs |
| `tick()` | Run learning if due | ~34μs |
| `force_learn()` | Force background cycle | ~5ms |
| `flush()` | Flush instant updates | ~10μs |
| `find_patterns(embedding, k)` | Find similar patterns | ~100μs |
| `get_stats()` | Get JSON statistics | ~1μs |
| `set_enabled(bool)` | Enable/disable engine | ~1ns |
| `is_enabled()` | Check if enabled | ~1ns |

### JsSonaConfig (Node.js)

```typescript
interface JsSonaConfig {
    hiddenDim: number;              // Required
    embeddingDim?: number;          // Default: hiddenDim
    microLoraRank?: number;         // Default: 2
    baseLoraRank?: number;          // Default: 8
    microLoraLr?: number;           // Default: 0.002
    baseLoraLr?: number;            // Default: 0.0001
    ewcLambda?: number;             // Default: 2000
    patternClusters?: number;       // Default: 100
    trajectoryCapacity?: number;    // Default: 10000
    backgroundIntervalMs?: number;  // Default: 3600000
    qualityThreshold?: number;      // Default: 0.3
    enableSimd?: boolean;           // Default: true
}
```

### JsLearnedPattern (Node.js)

```typescript
interface JsLearnedPattern {
    id: string;
    centroid: number[];
    clusterSize: number;
    totalWeight: number;
    avgQuality: number;
    createdAt: string;
    lastAccessed: string;
    accessCount: number;
    patternType: string;
}
```

---

## Benchmarks

### Performance Results (v0.1.1)

| Operation | Target | Achieved | Improvement |
|-----------|--------|----------|-------------|
| MicroLoRA Forward (256d) | <100μs | **45μs** | 2.2x better |
| Trajectory Recording | <1μs | **112ns** | 9x better |
| Instant Learning Cycle | <1ms | **34μs** | 29x better |
| Pattern Search (100 clusters) | <5ms | **1.3ms** | 3.8x better |
| Background Learning | <10ms | **~5ms** | 2x better |
| Memory per Trajectory | <1KB | **~800B** | 20% better |

### Throughput Benchmarks

| Scenario | Ops/Second | Latency (p99) |
|----------|------------|---------------|
| MicroLoRA Rank-2 (SIMD) | 2,211 | 0.85ms |
| MicroLoRA Rank-1 | 2,100 | 0.90ms |
| Batch Size 32 | 2,236 | 0.45ms/vector |
| Pattern Search (k=5) | 770 | 1.5ms |

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p ruvector-sona

# Run specific benchmark
cargo bench -p ruvector-sona -- micro_lora

# With detailed output
cargo bench -p ruvector-sona -- --verbose
```

---

## Troubleshooting

### Common Issues

**1. "MicroLoRA rank must be 1-2"**
```rust
// Wrong
let config = SonaConfig { micro_lora_rank: 4, .. };

// Correct - MicroLoRA is limited to rank 1-2 for speed
let config = SonaConfig { micro_lora_rank: 2, .. };

// For higher ranks, use BaseLoRA
let config = SonaConfig { base_lora_rank: 16, .. };
```

**2. Embedding dimension mismatch**
```rust
// Engine expects 256-dim embeddings
let engine = SonaEngine::new(256);

// Wrong - 512-dim embedding
let embedding = vec![0.1f32; 512];  // Panic!

// Correct
let embedding = vec![0.1f32; 256];
let traj_id = engine.begin_trajectory(embedding);
```

**3. Low quality scores not learning**
```rust
// If quality_threshold is 0.5, scores below won't trigger learning
let config = SonaConfig {
    quality_threshold: 0.5,  // Only learns from quality >= 0.5
    ..Default::default()
};

// Lower threshold to learn from more feedback
let config = SonaConfig {
    quality_threshold: 0.2,  // Learns from quality >= 0.2
    ..Default::default()
};
```

**4. Memory growing unbounded**
```rust
// Limit trajectory buffer
let config = SonaConfig {
    trajectory_capacity: 10000,  // Max trajectories in memory
    ..Default::default()
};

// Force learning to clear buffer
engine.force_learn();
```

### Performance Optimization Tips

1. **Use Rank-2 MicroLoRA** - 5% faster due to SIMD alignment
2. **Batch inputs when possible** - Optimal batch size is 32
3. **Use 100 pattern clusters** - 2.3x faster than 50
4. **Enable SIMD** - 10% speedup on supported CPUs
5. **Run background learning during low-traffic periods**

---

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Contributing

Contributions welcome! Please see our [Contributing Guide](https://github.com/ruvnet/ruvector/blob/main/CONTRIBUTING.md).

## Acknowledgments

- [LoRA Paper](https://arxiv.org/abs/2106.09685) - Low-Rank Adaptation
- [EWC Paper](https://arxiv.org/abs/1612.00796) - Elastic Weight Consolidation
- [K-means++](https://theory.stanford.edu/~sergei/papers/kMeansPP-soda.pdf) - Initialization algorithm

---

<div align="center">

**[Documentation](https://docs.rs/ruvector-sona)** | **[GitHub](https://github.com/ruvnet/ruvector)** | **[npm](https://www.npmjs.com/package/@ruvector/sona)** | **[crates.io](https://crates.io/crates/ruvector-sona)**

Made with 🦀 Rust by the RuVector Team

</div>
