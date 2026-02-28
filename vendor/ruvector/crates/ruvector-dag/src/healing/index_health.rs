//! Index Health Monitoring for HNSW and IVFFlat

#[derive(Debug, Clone)]
pub struct IndexHealth {
    pub index_name: String,
    pub index_type: IndexType,
    pub fragmentation: f64,
    pub recall_estimate: f64,
    pub node_count: usize,
    pub last_rebalanced: Option<std::time::Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndexType {
    Hnsw,
    IvfFlat,
    BTree,
    Other,
}

pub struct IndexHealthChecker {
    thresholds: IndexThresholds,
}

#[derive(Debug, Clone)]
pub struct IndexThresholds {
    pub max_fragmentation: f64,
    pub min_recall: f64,
    pub rebalance_interval_secs: u64,
}

impl Default for IndexThresholds {
    fn default() -> Self {
        Self {
            max_fragmentation: 0.3,
            min_recall: 0.95,
            rebalance_interval_secs: 3600,
        }
    }
}

impl IndexHealthChecker {
    pub fn new(thresholds: IndexThresholds) -> Self {
        Self { thresholds }
    }

    pub fn check_health(&self, health: &IndexHealth) -> IndexCheckResult {
        let mut issues = Vec::new();
        let mut recommendations = Vec::new();

        // Check fragmentation
        if health.fragmentation > self.thresholds.max_fragmentation {
            issues.push(format!(
                "High fragmentation: {:.1}% (threshold: {:.1}%)",
                health.fragmentation * 100.0,
                self.thresholds.max_fragmentation * 100.0
            ));
            recommendations.push("Run REINDEX or vacuum".to_string());
        }

        // Check recall
        if health.recall_estimate < self.thresholds.min_recall {
            issues.push(format!(
                "Low recall estimate: {:.1}% (threshold: {:.1}%)",
                health.recall_estimate * 100.0,
                self.thresholds.min_recall * 100.0
            ));

            match health.index_type {
                IndexType::Hnsw => {
                    recommendations.push("Increase ef_construction or M parameter".to_string());
                }
                IndexType::IvfFlat => {
                    recommendations.push("Increase nprobe or rebuild with more lists".to_string());
                }
                _ => {
                    recommendations.push("Consider rebuilding index".to_string());
                }
            }
        }

        // Check rebalance interval
        if let Some(last_rebalanced) = health.last_rebalanced {
            let elapsed = last_rebalanced.elapsed().as_secs();
            if elapsed > self.thresholds.rebalance_interval_secs {
                issues.push(format!(
                    "Index not rebalanced for {} seconds (threshold: {})",
                    elapsed, self.thresholds.rebalance_interval_secs
                ));
                recommendations.push("Schedule index rebalance".to_string());
            }
        }

        let status = if issues.is_empty() {
            HealthStatus::Healthy
        } else if issues.len() == 1 {
            HealthStatus::Warning
        } else {
            HealthStatus::Critical
        };

        IndexCheckResult {
            status,
            issues,
            recommendations,
            needs_rebalance: health.fragmentation > self.thresholds.max_fragmentation,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexCheckResult {
    pub status: HealthStatus,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
    pub needs_rebalance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_healthy_index() {
        let checker = IndexHealthChecker::new(IndexThresholds::default());
        let health = IndexHealth {
            index_name: "test_index".to_string(),
            index_type: IndexType::Hnsw,
            fragmentation: 0.1,
            recall_estimate: 0.98,
            node_count: 1000,
            last_rebalanced: Some(std::time::Instant::now()),
        };

        let result = checker.check_health(&health);
        assert_eq!(result.status, HealthStatus::Healthy);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_fragmented_index() {
        let checker = IndexHealthChecker::new(IndexThresholds::default());
        let health = IndexHealth {
            index_name: "test_index".to_string(),
            index_type: IndexType::Hnsw,
            fragmentation: 0.5,
            recall_estimate: 0.98,
            node_count: 1000,
            last_rebalanced: Some(std::time::Instant::now()),
        };

        let result = checker.check_health(&health);
        assert_eq!(result.status, HealthStatus::Warning);
        assert!(!result.issues.is_empty());
        assert!(result.needs_rebalance);
    }

    #[test]
    fn test_low_recall_index() {
        let checker = IndexHealthChecker::new(IndexThresholds::default());
        let health = IndexHealth {
            index_name: "test_index".to_string(),
            index_type: IndexType::IvfFlat,
            fragmentation: 0.1,
            recall_estimate: 0.85,
            node_count: 1000,
            last_rebalanced: Some(std::time::Instant::now()),
        };

        let result = checker.check_health(&health);
        assert_eq!(result.status, HealthStatus::Warning);
        assert!(!result.recommendations.is_empty());
    }
}
