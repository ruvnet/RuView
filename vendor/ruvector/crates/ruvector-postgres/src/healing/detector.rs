//! Problem Detection for Self-Healing Engine
//!
//! Implements continuous monitoring and problem classification:
//! - IndexDegradation: Index performance has degraded
//! - ReplicaLag: Replica is falling behind primary
//! - StorageExhaustion: Storage space is running low
//! - QueryTimeout: Queries are timing out excessively
//! - IntegrityViolation: Graph integrity has been compromised

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

// ============================================================================
// Problem Types
// ============================================================================

/// Types of problems that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProblemType {
    /// Index performance has degraded (fragmentation, poor connectivity)
    IndexDegradation,
    /// Replica is lagging behind primary
    ReplicaLag,
    /// Storage space is running low
    StorageExhaustion,
    /// Queries are timing out excessively
    QueryTimeout,
    /// Graph integrity has been violated (mincut below threshold)
    IntegrityViolation,
    /// Memory pressure is high
    MemoryPressure,
    /// Connection pool exhaustion
    ConnectionExhaustion,
    /// Hot partition detected (uneven load distribution)
    HotPartition,
}

impl ProblemType {
    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            ProblemType::IndexDegradation => "Index performance degradation detected",
            ProblemType::ReplicaLag => "Replica lag exceeds threshold",
            ProblemType::StorageExhaustion => "Storage space running low",
            ProblemType::QueryTimeout => "Excessive query timeouts",
            ProblemType::IntegrityViolation => "Graph integrity violation",
            ProblemType::MemoryPressure => "Memory pressure detected",
            ProblemType::ConnectionExhaustion => "Connection pool exhausted",
            ProblemType::HotPartition => "Hot partition detected",
        }
    }

    /// Get all problem types
    pub fn all() -> Vec<ProblemType> {
        vec![
            ProblemType::IndexDegradation,
            ProblemType::ReplicaLag,
            ProblemType::StorageExhaustion,
            ProblemType::QueryTimeout,
            ProblemType::IntegrityViolation,
            ProblemType::MemoryPressure,
            ProblemType::ConnectionExhaustion,
            ProblemType::HotPartition,
        ]
    }
}

impl std::fmt::Display for ProblemType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProblemType::IndexDegradation => write!(f, "index_degradation"),
            ProblemType::ReplicaLag => write!(f, "replica_lag"),
            ProblemType::StorageExhaustion => write!(f, "storage_exhaustion"),
            ProblemType::QueryTimeout => write!(f, "query_timeout"),
            ProblemType::IntegrityViolation => write!(f, "integrity_violation"),
            ProblemType::MemoryPressure => write!(f, "memory_pressure"),
            ProblemType::ConnectionExhaustion => write!(f, "connection_exhaustion"),
            ProblemType::HotPartition => write!(f, "hot_partition"),
        }
    }
}

impl std::str::FromStr for ProblemType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "index_degradation" | "indexdegradation" => Ok(ProblemType::IndexDegradation),
            "replica_lag" | "replicalag" => Ok(ProblemType::ReplicaLag),
            "storage_exhaustion" | "storageexhaustion" => Ok(ProblemType::StorageExhaustion),
            "query_timeout" | "querytimeout" => Ok(ProblemType::QueryTimeout),
            "integrity_violation" | "integrityviolation" => Ok(ProblemType::IntegrityViolation),
            "memory_pressure" | "memorypressure" => Ok(ProblemType::MemoryPressure),
            "connection_exhaustion" | "connectionexhaustion" => {
                Ok(ProblemType::ConnectionExhaustion)
            }
            "hot_partition" | "hotpartition" => Ok(ProblemType::HotPartition),
            _ => Err(format!("Unknown problem type: {}", s)),
        }
    }
}

// ============================================================================
// Severity Levels
// ============================================================================

/// Problem severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Informational, no action required
    Info,
    /// Low severity, can be addressed during maintenance
    Low,
    /// Medium severity, should be addressed soon
    Medium,
    /// High severity, requires prompt attention
    High,
    /// Critical severity, immediate action required
    Critical,
}

impl Severity {
    /// Get numeric value for comparison
    pub fn value(&self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Low => 1,
            Severity::Medium => 2,
            Severity::High => 3,
            Severity::Critical => 4,
        }
    }
}

// ============================================================================
// Problem Definition
// ============================================================================

/// A detected problem with full context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Problem {
    /// Type of problem
    pub problem_type: ProblemType,
    /// Severity level
    pub severity: Severity,
    /// When the problem was detected
    #[serde(with = "system_time_serde")]
    pub detected_at: SystemTime,
    /// Additional details about the problem
    pub details: serde_json::Value,
    /// Affected partition IDs (if applicable)
    pub affected_partitions: Vec<i64>,
}

impl Problem {
    /// Create a new problem
    pub fn new(problem_type: ProblemType, severity: Severity) -> Self {
        Self {
            problem_type,
            severity,
            detected_at: SystemTime::now(),
            details: serde_json::json!({}),
            affected_partitions: vec![],
        }
    }

    /// Add details to the problem
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    /// Add affected partitions
    pub fn with_partitions(mut self, partitions: Vec<i64>) -> Self {
        self.affected_partitions = partitions;
        self
    }

    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        let detected_ts = self
            .detected_at
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        serde_json::json!({
            "problem_type": self.problem_type.to_string(),
            "severity": format!("{:?}", self.severity).to_lowercase(),
            "detected_at": detected_ts,
            "details": self.details,
            "affected_partitions": self.affected_partitions,
        })
    }
}

// Custom serde for SystemTime
mod system_time_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH).unwrap();
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + Duration::from_secs(secs))
    }
}

// ============================================================================
// Detection Thresholds
// ============================================================================

/// Configurable thresholds for problem detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionThresholds {
    /// Index fragmentation percentage threshold (0-100)
    pub index_fragmentation_pct: f32,
    /// Replica lag in seconds threshold
    pub replica_lag_seconds: f32,
    /// Storage usage percentage threshold (0-100)
    pub storage_usage_pct: f32,
    /// Query timeout rate threshold (0-1)
    pub query_timeout_rate: f32,
    /// Minimum lambda (mincut) value for integrity
    pub min_integrity_lambda: f32,
    /// Memory usage percentage threshold (0-100)
    pub memory_usage_pct: f32,
    /// Connection pool usage percentage threshold (0-100)
    pub connection_usage_pct: f32,
    /// Partition load ratio threshold (vs average)
    pub partition_load_ratio: f32,
}

impl Default for DetectionThresholds {
    fn default() -> Self {
        Self {
            index_fragmentation_pct: 30.0,
            replica_lag_seconds: 5.0,
            storage_usage_pct: 85.0,
            query_timeout_rate: 0.05, // 5% timeout rate
            min_integrity_lambda: 0.5,
            memory_usage_pct: 85.0,
            connection_usage_pct: 90.0,
            partition_load_ratio: 3.0, // 3x average load
        }
    }
}

// ============================================================================
// System Metrics
// ============================================================================

/// System metrics collected for problem detection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Index fragmentation percentage per index
    pub index_fragmentation: HashMap<String, f32>,
    /// Replica lag in seconds per replica
    pub replica_lag: HashMap<String, f32>,
    /// Storage usage percentage
    pub storage_usage_pct: f32,
    /// Query timeout rate (0-1)
    pub query_timeout_rate: f32,
    /// Current integrity lambda value
    pub integrity_lambda: f32,
    /// Memory usage percentage
    pub memory_usage_pct: f32,
    /// Connection pool usage percentage
    pub connection_usage_pct: f32,
    /// Load per partition
    pub partition_loads: HashMap<i64, f64>,
    /// Witness edges from mincut computation
    pub witness_edges: Vec<WitnessEdge>,
    /// Maintenance queue depth
    pub maintenance_queue_depth: usize,
    /// Top memory consumers
    pub top_memory_consumers: Vec<(String, usize)>,
    /// Fragmented index IDs
    pub fragmented_indexes: Vec<i64>,
    /// Timestamp of metrics collection
    pub collected_at: u64,
}

impl SystemMetrics {
    /// Create new empty metrics
    pub fn new() -> Self {
        Self {
            collected_at: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ..Default::default()
        }
    }

    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "index_fragmentation": self.index_fragmentation,
            "replica_lag": self.replica_lag,
            "storage_usage_pct": self.storage_usage_pct,
            "query_timeout_rate": self.query_timeout_rate,
            "integrity_lambda": self.integrity_lambda,
            "memory_usage_pct": self.memory_usage_pct,
            "connection_usage_pct": self.connection_usage_pct,
            "partition_loads": self.partition_loads,
            "witness_edge_count": self.witness_edges.len(),
            "maintenance_queue_depth": self.maintenance_queue_depth,
            "collected_at": self.collected_at,
        })
    }
}

/// Witness edge from mincut computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessEdge {
    /// Source node ID
    pub from: i64,
    /// Target node ID
    pub to: i64,
    /// Edge type (e.g., "partition_link", "replication", "dependency")
    pub edge_type: String,
    /// Edge weight/capacity
    pub weight: f32,
}

// ============================================================================
// Problem Detector
// ============================================================================

/// Problem detector with configurable thresholds
pub struct ProblemDetector {
    /// Detection thresholds
    thresholds: RwLock<DetectionThresholds>,
    /// Number of problems detected
    problems_detected: AtomicU64,
    /// Last detection timestamp
    last_detection: AtomicU64,
}

impl ProblemDetector {
    /// Create a new problem detector with default thresholds
    pub fn new() -> Self {
        Self {
            thresholds: RwLock::new(DetectionThresholds::default()),
            problems_detected: AtomicU64::new(0),
            last_detection: AtomicU64::new(0),
        }
    }

    /// Create with custom thresholds
    pub fn with_thresholds(thresholds: DetectionThresholds) -> Self {
        Self {
            thresholds: RwLock::new(thresholds),
            problems_detected: AtomicU64::new(0),
            last_detection: AtomicU64::new(0),
        }
    }

    /// Update thresholds
    pub fn update_thresholds(&self, thresholds: DetectionThresholds) {
        *self.thresholds.write() = thresholds;
    }

    /// Get current thresholds
    pub fn get_thresholds(&self) -> DetectionThresholds {
        self.thresholds.read().clone()
    }

    /// Collect current system metrics
    pub fn collect_metrics(&self) -> SystemMetrics {
        let mut metrics = SystemMetrics::new();

        // In production, these would query PostgreSQL system catalogs
        // and index statistics. For now, we simulate with reasonable defaults.

        // Query pg_stat_user_indexes for fragmentation
        metrics.index_fragmentation = self.collect_index_fragmentation();

        // Query pg_stat_replication for replica lag
        metrics.replica_lag = self.collect_replica_lag();

        // Query pg_tablespace for storage usage
        metrics.storage_usage_pct = self.collect_storage_usage();

        // Query pg_stat_statements for timeout rate
        metrics.query_timeout_rate = self.collect_query_timeout_rate();

        // Get integrity lambda from mincut computation
        metrics.integrity_lambda = self.collect_integrity_lambda();

        // Query memory usage
        metrics.memory_usage_pct = self.collect_memory_usage();

        // Query connection pool usage
        metrics.connection_usage_pct = self.collect_connection_usage();

        // Query partition loads
        metrics.partition_loads = self.collect_partition_loads();

        // Get witness edges from mincut
        metrics.witness_edges = self.collect_witness_edges();

        metrics
    }

    /// Detect problems from collected metrics
    pub fn detect_problems(&self, metrics: &SystemMetrics) -> Vec<Problem> {
        let thresholds = self.thresholds.read();
        let mut problems = Vec::new();

        // Check index fragmentation
        for (index_name, frag_pct) in &metrics.index_fragmentation {
            if *frag_pct > thresholds.index_fragmentation_pct {
                let severity = if *frag_pct > 60.0 {
                    Severity::High
                } else if *frag_pct > 45.0 {
                    Severity::Medium
                } else {
                    Severity::Low
                };

                problems.push(
                    Problem::new(ProblemType::IndexDegradation, severity).with_details(
                        serde_json::json!({
                            "index_name": index_name,
                            "fragmentation_pct": frag_pct,
                            "threshold": thresholds.index_fragmentation_pct,
                        }),
                    ),
                );
            }
        }

        // Check replica lag
        for (replica_id, lag_seconds) in &metrics.replica_lag {
            if *lag_seconds > thresholds.replica_lag_seconds {
                let severity = if *lag_seconds > 30.0 {
                    Severity::Critical
                } else if *lag_seconds > 15.0 {
                    Severity::High
                } else if *lag_seconds > 10.0 {
                    Severity::Medium
                } else {
                    Severity::Low
                };

                problems.push(
                    Problem::new(ProblemType::ReplicaLag, severity).with_details(
                        serde_json::json!({
                            "replica_id": replica_id,
                            "lag_seconds": lag_seconds,
                            "threshold": thresholds.replica_lag_seconds,
                        }),
                    ),
                );
            }
        }

        // Check storage usage
        if metrics.storage_usage_pct > thresholds.storage_usage_pct {
            let severity = if metrics.storage_usage_pct > 95.0 {
                Severity::Critical
            } else if metrics.storage_usage_pct > 90.0 {
                Severity::High
            } else {
                Severity::Medium
            };

            problems.push(
                Problem::new(ProblemType::StorageExhaustion, severity).with_details(
                    serde_json::json!({
                        "usage_pct": metrics.storage_usage_pct,
                        "threshold": thresholds.storage_usage_pct,
                    }),
                ),
            );
        }

        // Check query timeout rate
        if metrics.query_timeout_rate > thresholds.query_timeout_rate {
            let severity = if metrics.query_timeout_rate > 0.20 {
                Severity::Critical
            } else if metrics.query_timeout_rate > 0.10 {
                Severity::High
            } else {
                Severity::Medium
            };

            problems.push(
                Problem::new(ProblemType::QueryTimeout, severity).with_details(serde_json::json!({
                    "timeout_rate": metrics.query_timeout_rate,
                    "threshold": thresholds.query_timeout_rate,
                })),
            );
        }

        // Check integrity lambda
        if metrics.integrity_lambda < thresholds.min_integrity_lambda
            && metrics.integrity_lambda > 0.0
        {
            let severity = if metrics.integrity_lambda < 0.2 {
                Severity::Critical
            } else if metrics.integrity_lambda < 0.35 {
                Severity::High
            } else {
                Severity::Medium
            };

            problems.push(
                Problem::new(ProblemType::IntegrityViolation, severity).with_details(
                    serde_json::json!({
                        "lambda": metrics.integrity_lambda,
                        "threshold": thresholds.min_integrity_lambda,
                        "witness_edges": metrics.witness_edges.len(),
                    }),
                ),
            );
        }

        // Check memory pressure
        if metrics.memory_usage_pct > thresholds.memory_usage_pct {
            let severity = if metrics.memory_usage_pct > 95.0 {
                Severity::Critical
            } else if metrics.memory_usage_pct > 90.0 {
                Severity::High
            } else {
                Severity::Medium
            };

            problems.push(
                Problem::new(ProblemType::MemoryPressure, severity).with_details(
                    serde_json::json!({
                        "usage_pct": metrics.memory_usage_pct,
                        "threshold": thresholds.memory_usage_pct,
                    }),
                ),
            );
        }

        // Check connection exhaustion
        if metrics.connection_usage_pct > thresholds.connection_usage_pct {
            let severity = if metrics.connection_usage_pct > 98.0 {
                Severity::Critical
            } else if metrics.connection_usage_pct > 95.0 {
                Severity::High
            } else {
                Severity::Medium
            };

            problems.push(
                Problem::new(ProblemType::ConnectionExhaustion, severity).with_details(
                    serde_json::json!({
                        "usage_pct": metrics.connection_usage_pct,
                        "threshold": thresholds.connection_usage_pct,
                    }),
                ),
            );
        }

        // Check for hot partitions
        if !metrics.partition_loads.is_empty() {
            let avg_load: f64 = metrics.partition_loads.values().sum::<f64>()
                / metrics.partition_loads.len() as f64;

            let hot_partitions: Vec<i64> = metrics
                .partition_loads
                .iter()
                .filter(|(_, load)| **load > avg_load * thresholds.partition_load_ratio as f64)
                .map(|(id, _)| *id)
                .collect();

            if !hot_partitions.is_empty() {
                let max_ratio = hot_partitions
                    .iter()
                    .filter_map(|id| metrics.partition_loads.get(id))
                    .map(|load| *load / avg_load)
                    .fold(0.0_f64, f64::max);

                let severity = if max_ratio > 10.0 {
                    Severity::High
                } else if max_ratio > 5.0 {
                    Severity::Medium
                } else {
                    Severity::Low
                };

                problems.push(
                    Problem::new(ProblemType::HotPartition, severity)
                        .with_details(serde_json::json!({
                            "avg_load": avg_load,
                            "max_ratio": max_ratio,
                            "threshold_ratio": thresholds.partition_load_ratio,
                        }))
                        .with_partitions(hot_partitions),
                );
            }
        }

        // Update statistics
        self.problems_detected
            .fetch_add(problems.len() as u64, Ordering::SeqCst);
        self.last_detection.store(
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::SeqCst,
        );

        problems
    }

    /// Get detection statistics
    pub fn get_stats(&self) -> DetectorStats {
        DetectorStats {
            problems_detected: self.problems_detected.load(Ordering::SeqCst),
            last_detection: self.last_detection.load(Ordering::SeqCst),
        }
    }

    // ========================================================================
    // Metric Collection Helpers (would use SPI in production)
    // ========================================================================

    fn collect_index_fragmentation(&self) -> HashMap<String, f32> {
        // In production: Query pg_stat_user_indexes and compute fragmentation
        // For now, return empty (healthy state)
        HashMap::new()
    }

    fn collect_replica_lag(&self) -> HashMap<String, f32> {
        // In production: Query pg_stat_replication
        HashMap::new()
    }

    fn collect_storage_usage(&self) -> f32 {
        // In production: Query pg_tablespace sizes
        0.0
    }

    fn collect_query_timeout_rate(&self) -> f32 {
        // In production: Query pg_stat_statements for timeout metrics
        0.0
    }

    fn collect_integrity_lambda(&self) -> f32 {
        // In production: Get from integrity control plane
        1.0 // Healthy default
    }

    fn collect_memory_usage(&self) -> f32 {
        // In production: Query pg_shmem_allocations or OS metrics
        0.0
    }

    fn collect_connection_usage(&self) -> f32 {
        // In production: Query pg_stat_activity vs max_connections
        0.0
    }

    fn collect_partition_loads(&self) -> HashMap<i64, f64> {
        // In production: Query partition statistics
        HashMap::new()
    }

    fn collect_witness_edges(&self) -> Vec<WitnessEdge> {
        // In production: Get from mincut computation
        Vec::new()
    }
}

impl Default for ProblemDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Detector statistics
#[derive(Debug, Clone)]
pub struct DetectorStats {
    pub problems_detected: u64,
    pub last_detection: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_problem_type_display() {
        assert_eq!(
            ProblemType::IndexDegradation.to_string(),
            "index_degradation"
        );
        assert_eq!(ProblemType::ReplicaLag.to_string(), "replica_lag");
        assert_eq!(
            ProblemType::IntegrityViolation.to_string(),
            "integrity_violation"
        );
    }

    #[test]
    fn test_problem_type_parse() {
        assert_eq!(
            "index_degradation".parse::<ProblemType>().unwrap(),
            ProblemType::IndexDegradation
        );
        assert_eq!(
            "replica_lag".parse::<ProblemType>().unwrap(),
            ProblemType::ReplicaLag
        );
    }

    #[test]
    fn test_detect_index_degradation() {
        let detector = ProblemDetector::new();

        let mut metrics = SystemMetrics::new();
        metrics
            .index_fragmentation
            .insert("test_idx".to_string(), 50.0);

        let problems = detector.detect_problems(&metrics);

        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].problem_type, ProblemType::IndexDegradation);
        assert_eq!(problems[0].severity, Severity::Medium);
    }

    #[test]
    fn test_detect_storage_exhaustion() {
        let detector = ProblemDetector::new();

        let mut metrics = SystemMetrics::new();
        metrics.storage_usage_pct = 92.0;

        let problems = detector.detect_problems(&metrics);

        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].problem_type, ProblemType::StorageExhaustion);
        assert_eq!(problems[0].severity, Severity::High);
    }

    #[test]
    fn test_detect_integrity_violation() {
        let detector = ProblemDetector::new();

        let mut metrics = SystemMetrics::new();
        metrics.integrity_lambda = 0.3;

        let problems = detector.detect_problems(&metrics);

        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].problem_type, ProblemType::IntegrityViolation);
        assert_eq!(problems[0].severity, Severity::High);
    }

    #[test]
    fn test_detect_hot_partition() {
        let detector = ProblemDetector::new();

        let mut metrics = SystemMetrics::new();
        metrics.partition_loads.insert(1, 100.0);
        metrics.partition_loads.insert(2, 100.0);
        metrics.partition_loads.insert(3, 500.0); // Hot partition

        let problems = detector.detect_problems(&metrics);

        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].problem_type, ProblemType::HotPartition);
        assert!(problems[0].affected_partitions.contains(&3));
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
        assert!(Severity::Low > Severity::Info);
    }

    #[test]
    fn test_healthy_metrics_no_problems() {
        let detector = ProblemDetector::new();
        let metrics = SystemMetrics::new();

        let problems = detector.detect_problems(&metrics);

        assert!(problems.is_empty());
    }

    #[test]
    fn test_custom_thresholds() {
        let thresholds = DetectionThresholds {
            index_fragmentation_pct: 10.0, // More sensitive
            ..Default::default()
        };
        let detector = ProblemDetector::with_thresholds(thresholds);

        let mut metrics = SystemMetrics::new();
        metrics
            .index_fragmentation
            .insert("test_idx".to_string(), 15.0);

        let problems = detector.detect_problems(&metrics);

        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].problem_type, ProblemType::IndexDegradation);
    }
}
