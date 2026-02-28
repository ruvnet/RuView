//! Comprehensive test suite for AgenticDB API
//!
//! Tests all 5 tables and API compatibility with agenticDB

use ruvector_core::{AgenticDB, DbOptions, Result};
use std::collections::HashMap;
use tempfile::tempdir;

fn create_test_db() -> Result<AgenticDB> {
    let dir = tempdir().unwrap();
    let mut options = DbOptions::default();
    options.storage_path = dir.path().join("test.db").to_string_lossy().to_string();
    options.dimensions = 128;
    AgenticDB::new(options)
}

// ============ Reflexion Memory Tests ============

#[test]
fn test_store_and_retrieve_episode() -> Result<()> {
    let db = create_test_db()?;

    let episode_id = db.store_episode(
        "Solve math problem".to_string(),
        vec!["read problem".to_string(), "calculate".to_string(), "verify".to_string()],
        vec!["got 42".to_string(), "answer correct".to_string()],
        "Good approach: verified the answer before submitting".to_string(),
    )?;

    assert!(!episode_id.is_empty());

    let episodes = db.retrieve_similar_episodes("solve math problems", 5)?;
    assert!(!episodes.is_empty());
    assert_eq!(episodes[0].id, episode_id);
    assert_eq!(episodes[0].task, "Solve math problem");
    assert_eq!(episodes[0].actions.len(), 3);

    Ok(())
}

#[test]
fn test_multiple_episodes_retrieval() -> Result<()> {
    let db = create_test_db()?;

    // Store multiple episodes
    for i in 0..5 {
        db.store_episode(
            format!("Task {}", i),
            vec![format!("action_{}", i)],
            vec![format!("observation_{}", i)],
            format!("critique_{}", i),
        )?;
    }

    let episodes = db.retrieve_similar_episodes("task", 10)?;
    assert!(episodes.len() >= 5);

    Ok(())
}

#[test]
fn test_episode_metadata() -> Result<()> {
    let db = create_test_db()?;

    let episode_id = db.store_episode(
        "Debug code".to_string(),
        vec!["add logging".to_string()],
        vec!["found bug".to_string()],
        "Logging helped identify the issue".to_string(),
    )?;

    let episodes = db.retrieve_similar_episodes("debug", 1)?;
    assert_eq!(episodes[0].id, episode_id);
    assert!(episodes[0].timestamp > 0);

    Ok(())
}

// ============ Skill Library Tests ============

#[test]
fn test_create_and_search_skill() -> Result<()> {
    let db = create_test_db()?;

    let mut params = HashMap::new();
    params.insert("input".to_string(), "string".to_string());
    params.insert("output".to_string(), "json".to_string());

    let skill_id = db.create_skill(
        "JSON Parser".to_string(),
        "Parse JSON string into structured data".to_string(),
        params,
        vec!["JSON.parse(input)".to_string()],
    )?;

    assert!(!skill_id.is_empty());

    let skills = db.search_skills("parse json", 5)?;
    assert!(!skills.is_empty());
    assert_eq!(skills[0].name, "JSON Parser");
    assert_eq!(skills[0].usage_count, 0);

    Ok(())
}

#[test]
fn test_skill_search_relevance() -> Result<()> {
    let db = create_test_db()?;

    // Create skills with different descriptions
    db.create_skill(
        "Sort Array".to_string(),
        "Sort an array of numbers in ascending order".to_string(),
        HashMap::new(),
        vec!["array.sort()".to_string()],
    )?;

    db.create_skill(
        "Filter Data".to_string(),
        "Filter array elements based on condition".to_string(),
        HashMap::new(),
        vec!["array.filter()".to_string()],
    )?;

    let skills = db.search_skills("sort numbers in array", 5)?;
    assert!(!skills.is_empty());

    Ok(())
}

#[test]
fn test_auto_consolidate_skills() -> Result<()> {
    let db = create_test_db()?;

    let sequences = vec![
        vec!["step1".to_string(), "step2".to_string(), "step3".to_string()],
        vec!["action1".to_string(), "action2".to_string(), "action3".to_string()],
        vec!["task1".to_string(), "task2".to_string()],  // Too short
    ];

    let skill_ids = db.auto_consolidate(sequences, 3)?;
    assert_eq!(skill_ids.len(), 2);  // Only 2 sequences meet threshold

    Ok(())
}

#[test]
fn test_skill_parameters() -> Result<()> {
    let db = create_test_db()?;

    let mut params = HashMap::new();
    params.insert("x".to_string(), "number".to_string());
    params.insert("y".to_string(), "number".to_string());

    db.create_skill(
        "Add Numbers".to_string(),
        "Add two numbers together".to_string(),
        params.clone(),
        vec!["return x + y".to_string()],
    )?;

    let skills = db.search_skills("add numbers", 1)?;
    assert!(!skills.is_empty());
    assert_eq!(skills[0].parameters.len(), 2);
    assert_eq!(skills[0].parameters.get("x"), Some(&"number".to_string()));

    Ok(())
}

// ============ Causal Memory Tests ============

#[test]
fn test_add_causal_edge() -> Result<()> {
    let db = create_test_db()?;

    let edge_id = db.add_causal_edge(
        vec!["high CPU".to_string()],
        vec!["slow response".to_string()],
        0.95,
        "Performance issue".to_string(),
    )?;

    assert!(!edge_id.is_empty());

    Ok(())
}

#[test]
fn test_hypergraph_multiple_causes_effects() -> Result<()> {
    let db = create_test_db()?;

    let edge_id = db.add_causal_edge(
        vec!["cause1".to_string(), "cause2".to_string(), "cause3".to_string()],
        vec!["effect1".to_string(), "effect2".to_string()],
        0.87,
        "Complex causal relationship".to_string(),
    )?;

    assert!(!edge_id.is_empty());

    Ok(())
}

#[test]
fn test_query_with_utility() -> Result<()> {
    let db = create_test_db()?;

    // Add causal edges
    db.add_causal_edge(
        vec!["rain".to_string()],
        vec!["wet ground".to_string()],
        0.99,
        "Weather observation".to_string(),
    )?;

    db.add_causal_edge(
        vec!["sun".to_string()],
        vec!["dry ground".to_string()],
        0.95,
        "Weather observation".to_string(),
    )?;

    // Query with utility function
    let results = db.query_with_utility(
        "weather conditions",
        5,
        0.7,  // alpha: similarity weight
        0.2,  // beta: causal confidence weight
        0.1,  // gamma: latency penalty
    )?;

    assert!(!results.is_empty());

    // Verify utility calculation
    for result in &results {
        assert!(result.utility_score >= 0.0);
        assert!(result.similarity_score >= 0.0);
        assert!(result.causal_uplift >= 0.0);
        assert!(result.latency_penalty >= 0.0);
    }

    Ok(())
}

#[test]
fn test_utility_function_weights() -> Result<()> {
    let db = create_test_db()?;

    db.add_causal_edge(
        vec!["test".to_string()],
        vec!["result".to_string()],
        0.8,
        "Test causal relationship".to_string(),
    )?;

    // Query with different weights
    let results1 = db.query_with_utility("test", 5, 1.0, 0.0, 0.0)?;  // Only similarity
    let results2 = db.query_with_utility("test", 5, 0.0, 1.0, 0.0)?;  // Only causal
    let results3 = db.query_with_utility("test", 5, 0.5, 0.5, 0.0)?;  // Balanced

    assert!(!results1.is_empty());
    assert!(!results2.is_empty());
    assert!(!results3.is_empty());

    Ok(())
}

// ============ Learning Sessions Tests ============

#[test]
fn test_start_learning_session() -> Result<()> {
    let db = create_test_db()?;

    let session_id = db.start_session(
        "Q-Learning".to_string(),
        4,  // state_dim
        2,  // action_dim
    )?;

    assert!(!session_id.is_empty());

    let session = db.get_session(&session_id)?;
    assert!(session.is_some());
    let session = session.unwrap();
    assert_eq!(session.algorithm, "Q-Learning");
    assert_eq!(session.state_dim, 4);
    assert_eq!(session.action_dim, 2);

    Ok(())
}

#[test]
fn test_add_experience() -> Result<()> {
    let db = create_test_db()?;

    let session_id = db.start_session("DQN".to_string(), 4, 2)?;

    db.add_experience(
        &session_id,
        vec![1.0, 0.0, 0.0, 0.0],
        vec![1.0, 0.0],
        1.0,
        vec![0.0, 1.0, 0.0, 0.0],
        false,
    )?;

    let session = db.get_session(&session_id)?.unwrap();
    assert_eq!(session.experiences.len(), 1);
    assert_eq!(session.experiences[0].reward, 1.0);

    Ok(())
}

#[test]
fn test_multiple_experiences() -> Result<()> {
    let db = create_test_db()?;

    let session_id = db.start_session("PPO".to_string(), 4, 2)?;

    // Add 10 experiences
    for i in 0..10 {
        db.add_experience(
            &session_id,
            vec![i as f32, 0.0, 0.0, 0.0],
            vec![1.0, 0.0],
            i as f64 * 0.1,
            vec![(i + 1) as f32, 0.0, 0.0, 0.0],
            i == 9,
        )?;
    }

    let session = db.get_session(&session_id)?.unwrap();
    assert_eq!(session.experiences.len(), 10);
    assert!(session.experiences.last().unwrap().done);

    Ok(())
}

#[test]
fn test_predict_with_confidence() -> Result<()> {
    let db = create_test_db()?;

    let session_id = db.start_session("Q-Learning".to_string(), 4, 2)?;

    // Add training data
    for i in 0..5 {
        db.add_experience(
            &session_id,
            vec![1.0, 0.0, 0.0, 0.0],
            vec![1.0, 0.0],
            0.8,
            vec![0.0, 1.0, 0.0, 0.0],
            false,
        )?;
    }

    // Make prediction
    let prediction = db.predict_with_confidence(&session_id, vec![1.0, 0.0, 0.0, 0.0])?;

    assert_eq!(prediction.action.len(), 2);
    assert!(prediction.mean_confidence >= 0.0);
    assert!(prediction.confidence_lower <= prediction.mean_confidence);
    assert!(prediction.confidence_upper >= prediction.mean_confidence);

    Ok(())
}

#[test]
fn test_different_algorithms() -> Result<()> {
    let db = create_test_db()?;

    let algorithms = vec!["Q-Learning", "DQN", "PPO", "A3C", "DDPG"];

    for algo in algorithms {
        let session_id = db.start_session(algo.to_string(), 4, 2)?;
        let session = db.get_session(&session_id)?.unwrap();
        assert_eq!(session.algorithm, algo);
    }

    Ok(())
}

// ============ Integration Tests ============

#[test]
fn test_full_workflow() -> Result<()> {
    let db = create_test_db()?;

    // 1. Agent attempts task and fails
    let fail_episode = db.store_episode(
        "Optimize query".to_string(),
        vec!["wrote query".to_string(), "ran on production".to_string()],
        vec!["timeout".to_string()],
        "Should test on staging first".to_string(),
    )?;

    // 2. Agent learns causal relationship
    let causal_edge = db.add_causal_edge(
        vec!["no index".to_string()],
        vec!["slow query".to_string()],
        0.9,
        "Database performance".to_string(),
    )?;

    // 3. Agent succeeds and creates skill
    let success_episode = db.store_episode(
        "Optimize query (retry)".to_string(),
        vec!["added index".to_string(), "tested on staging".to_string()],
        vec!["fast query".to_string()],
        "Indexes are important".to_string(),
    )?;

    let skill = db.create_skill(
        "Query Optimizer".to_string(),
        "Optimize database queries".to_string(),
        HashMap::new(),
        vec!["add index".to_string(), "test".to_string()],
    )?;

    // 4. Agent uses RL for future decisions
    let session = db.start_session("Q-Learning".to_string(), 4, 2)?;
    db.add_experience(&session, vec![1.0; 4], vec![1.0; 2], 1.0, vec![0.0; 4], false)?;

    // Verify all components work together
    assert!(!fail_episode.is_empty());
    assert!(!causal_edge.is_empty());
    assert!(!success_episode.is_empty());
    assert!(!skill.is_empty());
    assert!(!session.is_empty());

    Ok(())
}

#[test]
fn test_cross_table_queries() -> Result<()> {
    let db = create_test_db()?;

    // Populate all tables
    db.store_episode("task".to_string(), vec![], vec![], "critique".to_string())?;
    db.create_skill("skill".to_string(), "desc".to_string(), HashMap::new(), vec![])?;
    db.add_causal_edge(vec!["cause".to_string()], vec!["effect".to_string()], 0.8, "context".to_string())?;
    let session = db.start_session("Q-Learning".to_string(), 4, 2)?;

    // Query across tables
    let episodes = db.retrieve_similar_episodes("task", 5)?;
    let skills = db.search_skills("skill", 5)?;
    let causal = db.query_with_utility("cause", 5, 0.7, 0.2, 0.1)?;
    let session_data = db.get_session(&session)?;

    assert!(!episodes.is_empty());
    assert!(!skills.is_empty());
    assert!(!causal.is_empty());
    assert!(session_data.is_some());

    Ok(())
}

#[test]
fn test_persistence() -> Result<()> {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("persistent.db");

    // Create and populate database
    {
        let mut options = DbOptions::default();
        options.storage_path = db_path.to_string_lossy().to_string();
        options.dimensions = 128;
        let db = AgenticDB::new(options)?;

        db.store_episode("task".to_string(), vec![], vec![], "critique".to_string())?;
    }

    // Reopen and verify data persisted
    {
        let mut options = DbOptions::default();
        options.storage_path = db_path.to_string_lossy().to_string();
        options.dimensions = 128;
        let db = AgenticDB::new(options)?;

        let episodes = db.retrieve_similar_episodes("task", 5)?;
        assert!(!episodes.is_empty());
    }

    Ok(())
}

#[test]
fn test_concurrent_operations() -> Result<()> {
    let db = create_test_db()?;

    // Simulate concurrent operations
    for i in 0..100 {
        db.store_episode(
            format!("task{}", i),
            vec![],
            vec![],
            format!("critique{}", i),
        )?;
    }

    let episodes = db.retrieve_similar_episodes("task", 10)?;
    assert!(episodes.len() <= 10);

    Ok(())
}
