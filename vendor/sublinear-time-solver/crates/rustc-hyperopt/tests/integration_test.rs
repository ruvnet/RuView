//! Integration tests for rustc-hyperopt

use rustc_hyperopt::{ColdStartOptimizer, OptimizerConfig};
use tokio;

#[tokio::test]
async fn test_optimizer_creation() {
    let optimizer = ColdStartOptimizer::new().await;
    assert!(optimizer.is_ok());
}

#[tokio::test]
async fn test_optimizer_with_config() {
    let config = OptimizerConfig::default();
    let optimizer = ColdStartOptimizer::with_config(config).await;
    assert!(optimizer.is_ok());
}

#[tokio::test]
async fn test_optimization_run() {
    let optimizer = ColdStartOptimizer::new().await.unwrap();
    let result = optimizer.optimize_compilation().await;
    assert!(result.is_ok());

    let optimization_result = result.unwrap();
    assert!(optimization_result.speedup_factor >= 1.0);
    assert!(optimization_result.patterns_matched >= 0);
}

#[tokio::test]
async fn test_performance_metrics() {
    let optimizer = ColdStartOptimizer::new().await.unwrap();

    // Run optimization first
    let _ = optimizer.optimize_compilation().await.unwrap();

    let metrics = optimizer.get_performance_metrics().await;
    assert!(metrics.is_ok());

    let performance_metrics = metrics.unwrap();
    assert!(performance_metrics.total_optimizations >= 1);
}

#[tokio::test]
async fn test_cache_operations() {
    let optimizer = ColdStartOptimizer::new().await.unwrap();

    // Clear caches should work
    let result = optimizer.clear_caches().await;
    assert!(result.is_ok());
}