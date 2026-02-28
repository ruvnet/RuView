//! Test helper functions
//!
//! Provides utility functions for integration testing.

#![allow(dead_code)]

use std::time::Duration;
use tokio::time::timeout;

/// Run async test with timeout
pub async fn with_timeout<F, T>(duration: Duration, future: F) -> Result<T, String>
where
    F: std::future::Future<Output = T>,
{
    match timeout(duration, future).await {
        Ok(result) => Ok(result),
        Err(_) => Err(format!("Test timed out after {:?}", duration)),
    }
}

/// Initialize test logger
pub fn init_test_logger() {
    // Initialize tracing/logging for tests
    // Only initialize once
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Info)
        .try_init();
}

/// Generate deterministic random data for testing
pub fn deterministic_random_vec(seed: u64, len: usize) -> Vec<f32> {
    // Simple LCG for deterministic "random" numbers
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            ((state / 65536) % 32768) as f32 / 32768.0
        })
        .collect()
}

/// Measure execution time of a function
pub async fn measure_async<F, T>(f: F) -> (T, Duration)
where
    F: std::future::Future<Output = T>,
{
    let start = std::time::Instant::now();
    let result = f.await;
    let duration = start.elapsed();
    (result, duration)
}

/// Compare vectors with tolerance
pub fn vectors_approx_equal(a: &[f32], b: &[f32], tolerance: f32) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.iter()
        .zip(b.iter())
        .all(|(av, bv)| (av - bv).abs() < tolerance)
}

/// Cosine similarity
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(av, bv)| av * bv).sum();
    let norm_a: f32 = a.iter().map(|av| av * av).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|bv| bv * bv).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

/// Wait for async condition to become true
pub async fn wait_for_condition<F>(
    mut condition: F,
    timeout_duration: Duration,
    check_interval: Duration,
) -> Result<(), String>
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();

    while start.elapsed() < timeout_duration {
        if condition() {
            return Ok(());
        }
        tokio::time::sleep(check_interval).await;
    }

    Err(format!(
        "Condition not met within {:?}",
        timeout_duration
    ))
}

/// Create a temporary test directory
pub fn create_temp_test_dir() -> std::io::Result<std::path::PathBuf> {
    let temp_dir = std::env::temp_dir().join(format!("exo-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    Ok(temp_dir)
}

/// Clean up test resources
pub async fn cleanup_test_resources(path: &std::path::Path) -> std::io::Result<()> {
    if path.exists() {
        tokio::fs::remove_dir_all(path).await?;
    }
    Ok(())
}

// Mock UUID for tests (replace with actual uuid crate when available)
mod uuid {
    pub struct Uuid;
    impl Uuid {
        pub fn new_v4() -> String {
            format!("{:016x}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos())
        }
    }
}
