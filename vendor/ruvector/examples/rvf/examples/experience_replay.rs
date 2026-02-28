//! RL Experience Replay Buffer — Agentic AI
//!
//! Demonstrates using an RVF store as an experience replay buffer for RL agents:
//! 1. Create a store as an experience replay buffer
//! 2. Insert "experience" vectors with metadata: episode_id, step, reward, priority
//! 3. Implement priority sampling: query sorted by reward metadata
//! 4. Query for high-reward experiences (Gt filter on reward)
//! 5. Show experience distribution across episodes
//! 6. Demonstrate temperature tiering concept: high-priority = Hot tier
//! 7. Print replay buffer stats and sampled experiences
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG (via RvfStore)
//!
//! Run with:
//!   cargo run --example experience_replay

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use tempfile::TempDir;

/// Simple pseudo-random number generator (LCG) for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn main() {
    println!("=== RVF RL Experience Replay Buffer Example ===\n");

    let dim = 64;
    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("replay_buffer.rvf");

    // -- Step 1: Create the replay buffer store --
    println!("--- 1. Creating Experience Replay Buffer ---");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");
    println!("  Replay buffer created ({} dims, L2 metric)", dim);

    // -- Step 2: Insert experiences --
    // Metadata fields:
    //   field_id 0: episode_id (U64)
    //   field_id 1: step       (U64: step within episode)
    //   field_id 2: reward     (U64: scaled reward, 0-10000 representing 0.0-100.0)
    //   field_id 3: priority   (U64: sampling priority, 1-100)
    println!("\n--- 2. Populating Replay Buffer ---");

    let num_episodes = 5;
    let steps_per_episode = 20;
    let total_experiences = num_episodes * steps_per_episode;
    let mut next_id: u64 = 0;

    // Track per-episode stats
    let mut episode_rewards: Vec<Vec<u64>> = Vec::new();

    for episode in 0..num_episodes {
        let mut ep_rewards = Vec::new();

        let vectors: Vec<Vec<f32>> = (0..steps_per_episode)
            .map(|step| {
                // State embedding varies by episode and step
                random_vector(dim, episode as u64 * 1000 + step as u64)
            })
            .collect();
        let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (next_id..next_id + steps_per_episode as u64).collect();

        let mut metadata = Vec::with_capacity(steps_per_episode * 4);
        for step in 0..steps_per_episode {
            // Episode ID
            metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::U64(episode as u64),
            });

            // Step number
            metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::U64(step as u64),
            });

            // Reward: deterministic, increases toward episode end (delayed reward)
            // Episode 0: low rewards, Episode 4: high rewards
            let base_reward = (episode as u64) * 1500;
            let step_bonus = (step as u64) * 100;
            let noise = ((step * 7 + episode * 13 + 42) % 500) as u64;
            let reward = base_reward + step_bonus + noise;
            metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::U64(reward),
            });
            ep_rewards.push(reward);

            // Priority: proportional to reward (high reward = high priority)
            let priority = (reward / 100).clamp(1, 100);
            metadata.push(MetadataEntry {
                field_id: 3,
                value: MetadataValue::U64(priority),
            });
        }

        store
            .ingest_batch(&vec_refs, &ids, Some(&metadata))
            .expect("failed to ingest experiences");

        let avg_reward = ep_rewards.iter().sum::<u64>() / ep_rewards.len() as u64;
        let max_reward = *ep_rewards.iter().max().unwrap();
        println!(
            "  Episode {}: {} steps, avg_reward={}, max_reward={}",
            episode, steps_per_episode, avg_reward, max_reward
        );

        episode_rewards.push(ep_rewards);
        next_id += steps_per_episode as u64;
    }

    println!("  Total experiences: {}", total_experiences);

    // -- Step 3: Priority sampling (query most similar + high reward) --
    println!("\n--- 3. Priority Sampling ---");

    // Query for experiences similar to a "target state"
    let target_state = random_vector(dim, 4010); // close to episode 4, step 10
    let k = 10;

    let all_results = store
        .query(&target_state, k, &QueryOptions::default())
        .expect("query failed");

    println!("  Similarity search (all priorities): top-{} results:", k);
    print_experience_results(&all_results, steps_per_episode, &episode_rewards);

    // -- Step 4: High-reward experience sampling --
    println!("\n--- 4. High-Reward Experiences (reward > 5000) ---");

    let filter_high_reward = FilterExpr::Gt(2, FilterValue::U64(5000));
    let opts_high_reward = QueryOptions {
        filter: Some(filter_high_reward),
        ..Default::default()
    };
    let high_reward_results = store
        .query(&target_state, k, &opts_high_reward)
        .expect("high reward query failed");

    println!(
        "  High-reward experiences: {} results",
        high_reward_results.len()
    );
    print_experience_results(&high_reward_results, steps_per_episode, &episode_rewards);

    // Verify all results have reward > 5000
    for r in &high_reward_results {
        let ep = (r.id as usize) / steps_per_episode;
        let step = (r.id as usize) % steps_per_episode;
        let reward = episode_rewards[ep][step];
        assert!(
            reward > 5000,
            "ID {} (ep={}, step={}) has reward {} which is not > 5000",
            r.id,
            ep,
            step,
            reward
        );
    }
    if !high_reward_results.is_empty() {
        println!("  All results verified: reward > 5000.");
    }

    // -- Step 5: Episode-specific sampling --
    println!("\n--- 5. Episode-Specific Sampling (Episode 3) ---");

    let filter_episode_3 = FilterExpr::Eq(0, FilterValue::U64(3));
    let opts_ep3 = QueryOptions {
        filter: Some(filter_episode_3),
        ..Default::default()
    };
    let ep3_results = store
        .query(&target_state, k, &opts_ep3)
        .expect("episode 3 query failed");

    println!("  Episode 3 experiences: {} results", ep3_results.len());
    print_experience_results(&ep3_results, steps_per_episode, &episode_rewards);

    // -- Step 6: Experience distribution --
    println!("\n--- 6. Experience Distribution ---\n");

    println!(
        "  {:>10}  {:>6}  {:>12}  {:>12}  {:>12}",
        "Episode", "Steps", "Avg Reward", "Max Reward", "Tier"
    );
    println!(
        "  {:->10}  {:->6}  {:->12}  {:->12}  {:->12}",
        "", "", "", "", ""
    );

    for (ep, rewards) in episode_rewards.iter().enumerate() {
        let avg = rewards.iter().sum::<u64>() / rewards.len() as u64;
        let max = *rewards.iter().max().unwrap();
        let tier = if avg > 5000 {
            "Hot"
        } else if avg > 2000 {
            "Warm"
        } else {
            "Cold"
        };
        println!(
            "  {:>10}  {:>6}  {:>12}  {:>12}  {:>12}",
            ep, rewards.len(), avg, max, tier
        );
    }

    // -- Step 7: Temperature tiering --
    println!("\n--- 7. Temperature Tiering (Access Priority) ---");

    // Hot tier: high-priority experiences (priority > 50)
    let filter_hot = FilterExpr::Gt(3, FilterValue::U64(50));
    let opts_hot = QueryOptions {
        filter: Some(filter_hot),
        ..Default::default()
    };
    let hot_results = store
        .query(&target_state, 20, &opts_hot)
        .expect("hot tier query failed");

    // Warm tier: medium priority (20 < priority <= 50)
    let filter_warm = FilterExpr::Range(3, FilterValue::U64(20), FilterValue::U64(51));
    let opts_warm = QueryOptions {
        filter: Some(filter_warm),
        ..Default::default()
    };
    let warm_results = store
        .query(&target_state, 20, &opts_warm)
        .expect("warm tier query failed");

    // Cold tier: low priority (priority <= 20)
    let filter_cold = FilterExpr::Le(3, FilterValue::U64(20));
    let opts_cold = QueryOptions {
        filter: Some(filter_cold),
        ..Default::default()
    };
    let cold_results = store
        .query(&target_state, 20, &opts_cold)
        .expect("cold tier query failed");

    // Count how many total exist in each tier (not just query results)
    let mut hot_count = 0u32;
    let mut warm_count = 0u32;
    let mut cold_count = 0u32;
    for rewards in &episode_rewards {
        for &reward in rewards {
            let priority = (reward / 100).clamp(1, 100);
            if priority > 50 {
                hot_count += 1;
            } else if priority > 20 {
                warm_count += 1;
            } else {
                cold_count += 1;
            }
        }
    }

    println!(
        "  {:>8}  {:>12}  {:>14}  {:>12}",
        "Tier", "Total Count", "Query Results", "Description"
    );
    println!(
        "  {:->8}  {:->12}  {:->14}  {:->12}",
        "", "", "", ""
    );
    println!(
        "  {:>8}  {:>12}  {:>14}  {:>12}",
        "Hot", hot_count, hot_results.len(), "priority > 50"
    );
    println!(
        "  {:>8}  {:>12}  {:>14}  {:>12}",
        "Warm", warm_count, warm_results.len(), "20 < p <= 50"
    );
    println!(
        "  {:>8}  {:>12}  {:>14}  {:>12}",
        "Cold", cold_count, cold_results.len(), "priority <= 20"
    );
    println!(
        "\n  Hot tier should be kept in fp32 (full precision)."
    );
    println!("  Warm tier could use scalar quantization (4x compression).");
    println!("  Cold tier could use binary quantization (32x compression).");

    // -- Step 8: Combined priority + episode filter --
    println!("\n--- 8. Combined Filter: Episode 4 + High Priority ---");

    let filter_combined = FilterExpr::And(vec![
        FilterExpr::Eq(0, FilterValue::U64(4)),
        FilterExpr::Gt(3, FilterValue::U64(50)),
    ]);
    let opts_combined = QueryOptions {
        filter: Some(filter_combined),
        ..Default::default()
    };
    let combined_results = store
        .query(&target_state, k, &opts_combined)
        .expect("combined query failed");

    println!(
        "  Episode 4 high-priority experiences: {} results",
        combined_results.len()
    );
    print_experience_results(&combined_results, steps_per_episode, &episode_rewards);

    store.close().expect("failed to close store");

    // -- Summary --
    println!("\n=== Experience Replay Buffer Summary ===\n");
    println!("  Total experiences: {}", total_experiences);
    println!("  Episodes:          {}", num_episodes);
    println!("  Steps per episode: {}", steps_per_episode);
    println!("  Hot tier (>50):    {} experiences", hot_count);
    println!("  Warm tier (20-50): {} experiences", warm_count);
    println!("  Cold tier (<=20):  {} experiences", cold_count);
    println!("  High-reward (>5k): {} query results", high_reward_results.len());
    println!("  Sampling:          similarity + metadata filters");

    println!("\nDone.");
}

fn print_experience_results(
    results: &[SearchResult],
    steps_per_episode: usize,
    episode_rewards: &[Vec<u64>],
) {
    println!(
        "  {:>6}  {:>12}  {:>8}  {:>6}  {:>8}  {:>8}  {:>6}",
        "ID", "Distance", "Episode", "Step", "Reward", "Priority", "Tier"
    );
    println!(
        "  {:->6}  {:->12}  {:->8}  {:->6}  {:->8}  {:->8}  {:->6}",
        "", "", "", "", "", "", ""
    );
    for r in results {
        let ep = (r.id as usize) / steps_per_episode;
        let step = (r.id as usize) % steps_per_episode;
        let reward = if ep < episode_rewards.len() && step < episode_rewards[ep].len() {
            episode_rewards[ep][step]
        } else {
            0
        };
        let priority = (reward / 100).clamp(1, 100);
        let tier = if priority > 50 {
            "Hot"
        } else if priority > 20 {
            "Warm"
        } else {
            "Cold"
        };
        println!(
            "  {:>6}  {:>12.6}  {:>8}  {:>6}  {:>8}  {:>8}  {:>6}",
            r.id, r.distance, ep, step, reward, priority, tier
        );
    }
}
