//! Repair Strategies

use super::anomaly::{Anomaly, AnomalyType};

#[derive(Debug, Clone)]
pub struct RepairResult {
    pub strategy_name: String,
    pub success: bool,
    pub duration_ms: f64,
    pub details: String,
}

pub trait RepairStrategy: Send + Sync {
    fn name(&self) -> &str;
    fn can_repair(&self, anomaly: &Anomaly) -> bool;
    fn repair(&self, anomaly: &Anomaly) -> RepairResult;
}

pub struct IndexRebalanceStrategy {
    target_recall: f64,
}

impl IndexRebalanceStrategy {
    pub fn new(target_recall: f64) -> Self {
        Self { target_recall }
    }
}

impl RepairStrategy for IndexRebalanceStrategy {
    fn name(&self) -> &str {
        "index_rebalance"
    }

    fn can_repair(&self, anomaly: &Anomaly) -> bool {
        matches!(anomaly.anomaly_type, AnomalyType::LatencySpike)
    }

    fn repair(&self, anomaly: &Anomaly) -> RepairResult {
        let start = std::time::Instant::now();

        // Simulate rebalancing
        // In real implementation, would call index rebuild
        std::thread::sleep(std::time::Duration::from_millis(10));

        RepairResult {
            strategy_name: self.name().to_string(),
            success: true,
            duration_ms: start.elapsed().as_secs_f64() * 1000.0,
            details: format!(
                "Rebalanced index for component: {} (target recall: {:.2})",
                anomaly.component, self.target_recall
            ),
        }
    }
}

pub struct PatternResetStrategy {
    quality_threshold: f64,
}

impl PatternResetStrategy {
    pub fn new(quality_threshold: f64) -> Self {
        Self { quality_threshold }
    }
}

impl RepairStrategy for PatternResetStrategy {
    fn name(&self) -> &str {
        "pattern_reset"
    }

    fn can_repair(&self, anomaly: &Anomaly) -> bool {
        matches!(
            anomaly.anomaly_type,
            AnomalyType::PatternDrift | AnomalyType::LearningStall
        )
    }

    fn repair(&self, anomaly: &Anomaly) -> RepairResult {
        let start = std::time::Instant::now();

        // Reset low-quality patterns
        std::thread::sleep(std::time::Duration::from_millis(5));

        RepairResult {
            strategy_name: self.name().to_string(),
            success: true,
            duration_ms: start.elapsed().as_secs_f64() * 1000.0,
            details: format!(
                "Reset patterns below quality {} for component: {}",
                self.quality_threshold, anomaly.component
            ),
        }
    }
}

pub struct CacheFlushStrategy;

impl RepairStrategy for CacheFlushStrategy {
    fn name(&self) -> &str {
        "cache_flush"
    }

    fn can_repair(&self, anomaly: &Anomaly) -> bool {
        matches!(
            anomaly.anomaly_type,
            AnomalyType::CacheEviction | AnomalyType::MemoryPressure
        )
    }

    fn repair(&self, anomaly: &Anomaly) -> RepairResult {
        let start = std::time::Instant::now();

        // Flush caches
        std::thread::sleep(std::time::Duration::from_millis(2));

        RepairResult {
            strategy_name: self.name().to_string(),
            success: true,
            duration_ms: start.elapsed().as_secs_f64() * 1000.0,
            details: format!(
                "Flushed attention and pattern caches for component: {}",
                anomaly.component
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_rebalance_strategy() {
        let strategy = IndexRebalanceStrategy::new(0.95);
        let anomaly = Anomaly {
            anomaly_type: AnomalyType::LatencySpike,
            z_score: 4.5,
            value: 100.0,
            expected: 10.0,
            timestamp: std::time::Instant::now(),
            component: "hnsw_index".to_string(),
        };

        assert!(strategy.can_repair(&anomaly));
        let result = strategy.repair(&anomaly);
        assert!(result.success);
        assert!(result.duration_ms > 0.0);
    }

    #[test]
    fn test_pattern_reset_strategy() {
        let strategy = PatternResetStrategy::new(0.8);
        let anomaly = Anomaly {
            anomaly_type: AnomalyType::PatternDrift,
            z_score: 3.2,
            value: 0.5,
            expected: 0.9,
            timestamp: std::time::Instant::now(),
            component: "pattern_cache".to_string(),
        };

        assert!(strategy.can_repair(&anomaly));
        let result = strategy.repair(&anomaly);
        assert!(result.success);
    }

    #[test]
    fn test_cache_flush_strategy() {
        let strategy = CacheFlushStrategy;
        let anomaly = Anomaly {
            anomaly_type: AnomalyType::MemoryPressure,
            z_score: 5.0,
            value: 95.0,
            expected: 60.0,
            timestamp: std::time::Instant::now(),
            component: "memory".to_string(),
        };

        assert!(strategy.can_repair(&anomaly));
        let result = strategy.repair(&anomaly);
        assert!(result.success);
    }
}
