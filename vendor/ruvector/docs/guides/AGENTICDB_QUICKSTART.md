# AgenticDB Quick Start Guide

Get started with Ruvector's AgenticDB API in 5 minutes.

## Installation

```bash
# Add to Cargo.toml
[dependencies]
ruvector-core = "0.1"
```

## Basic Usage

```rust
use ruvector_core::{AgenticDB, DbOptions, Result};
use std::collections::HashMap;

fn main() -> Result<()> {
    // 1. Initialize database
    let db = AgenticDB::with_dimensions(128)?;

    // 2. Store a learning episode
    let episode_id = db.store_episode(
        "Learn to optimize code".to_string(),
        vec!["analyzed bottleneck".to_string(), "applied optimization".to_string()],
        vec!["code 2x faster".to_string()],
        "Profiling first helps identify real bottlenecks".to_string(),
    )?;
    println!("Stored episode: {}", episode_id);

    // 3. Retrieve similar past experiences
    let similar = db.retrieve_similar_episodes("code optimization", 5)?;
    println!("Found {} similar experiences", similar.len());

    // 4. Create a reusable skill
    let skill_id = db.create_skill(
        "Code Profiler".to_string(),
        "Profile code to find performance bottlenecks".to_string(),
        HashMap::new(),
        vec!["run profiler".to_string(), "analyze hotspots".to_string()],
    )?;
    println!("Created skill: {}", skill_id);

    // 5. Add causal knowledge
    db.add_causal_edge(
        vec!["inefficient loop".to_string()],
        vec!["slow performance".to_string()],
        0.9,
        "Performance analysis".to_string(),
    )?;

    // 6. Start RL training
    let session = db.start_session("Q-Learning".to_string(), 4, 2)?;
    db.add_experience(&session, vec![1.0; 4], vec![1.0; 2], 1.0, vec![0.0; 4], false)?;
    
    // 7. Get predictions
    let prediction = db.predict_with_confidence(&session, vec![1.0; 4])?;
    println!("Predicted action: {:?}", prediction.action);

    Ok(())
}
```

## Five Core APIs

### 1. Reflexion Memory
Learn from past mistakes:
```rust
// Store mistake
db.store_episode(task, actions, observations, critique)?;

// Learn from history
let similar = db.retrieve_similar_episodes("similar situation", 5)?;
```

### 2. Skill Library
Build reusable patterns:
```rust
// Create skill
db.create_skill(name, description, params, examples)?;

// Find relevant skills
let skills = db.search_skills("what I need to do", 5)?;
```

### 3. Causal Memory
Understand cause and effect:
```rust
// Add relationship (supports multiple causes → multiple effects)
db.add_causal_edge(
    vec!["cause1", "cause2"],
    vec!["effect1", "effect2"],
    confidence,
    context,
)?;

// Query with utility function
let results = db.query_with_utility(query, k, 0.7, 0.2, 0.1)?;
```

### 4. Learning Sessions
Train RL models:
```rust
// Start training
let session = db.start_session("DQN", state_dim, action_dim)?;

// Add experience
db.add_experience(&session, state, action, reward, next_state, done)?;

// Make predictions
let pred = db.predict_with_confidence(&session, current_state)?;
```

### 5. Vector Search
Fast similarity search:
```rust
// All text is automatically embedded and indexed
// Just use the high-level APIs above!
```

## Complete Example

See `examples/agenticdb_demo.rs` for a full demonstration.

## Documentation

- Full API reference: `docs/AGENTICDB_API.md`
- Implementation details: `docs/PHASE3_SUMMARY.md`

## Performance

- 10-100x faster than original agenticDB
- O(log n) search with HNSW index
- SIMD-optimized distance calculations
- Concurrent access with lock-free reads

## Next Steps

1. Try the example: `cargo run --example agenticdb_demo`
2. Read the API docs: `docs/AGENTICDB_API.md`
3. Run tests: `cargo test -p ruvector-core agenticdb`
4. Build your agentic AI system!
