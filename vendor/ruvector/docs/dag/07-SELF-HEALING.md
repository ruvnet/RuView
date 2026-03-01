# Self-Healing System Specification

## Overview

The self-healing system automatically detects, diagnoses, and repairs issues in the Neural DAG system, including index degradation, learning drift, and performance bottlenecks.

## Self-Healing Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          SELF-HEALING ENGINE                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                       DETECTION LAYER                                │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │   │
│  │  │  Anomaly    │  │ Performance │  │   Index     │  │  Learning  │  │   │
│  │  │  Detector   │  │  Monitor    │  │  Health     │  │   Drift    │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┴───────────────────────────────────┐   │
│  │                      DIAGNOSIS LAYER                                 │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │    Root     │  │   Impact    │  │  Priority   │                  │   │
│  │  │   Cause     │  │  Analysis   │  │  Scoring    │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┴───────────────────────────────────┐   │
│  │                        REPAIR LAYER                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌────────────┐  │   │
│  │  │   Index     │  │   Pattern   │  │  Parameter  │  │  Topology  │  │   │
│  │  │  Rebalance  │  │   Reset     │  │   Tuning    │  │   Repair   │  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│  ┌─────────────────────────────────┴───────────────────────────────────┐   │
│  │                      VERIFICATION LAYER                              │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │   │
│  │  │   Repair    │  │  Rollback   │  │   Metrics   │                  │   │
│  │  │ Validation  │  │  Mechanism  │  │  Reporting  │                  │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Self-Healing Engine

### Core Structure

```rust
pub struct SelfHealingEngine {
    /// Detection components
    anomaly_detector: AnomalyDetector,
    performance_monitor: PerformanceMonitor,
    index_health_checker: IndexHealthChecker,
    learning_drift_detector: LearningDriftDetector,

    /// Diagnosis components
    root_cause_analyzer: RootCauseAnalyzer,

    /// Repair strategies
    repair_strategies: Vec<Box<dyn RepairStrategy>>,

    /// Configuration
    config: HealingConfig,

    /// State tracking
    active_repairs: DashMap<RepairId, ActiveRepair>,
    repair_history: Vec<RepairRecord>,

    /// Metrics
    metrics: HealingMetrics,
}

#[derive(Clone, Debug)]
pub struct HealingConfig {
    /// Enable automatic healing
    pub auto_heal_enabled: bool,

    /// Check interval (milliseconds)
    pub check_interval_ms: u64,

    /// Anomaly detection sensitivity (0.0 - 1.0)
    pub anomaly_sensitivity: f32,

    /// Performance degradation threshold
    pub performance_threshold: f32,

    /// Maximum concurrent repairs
    pub max_concurrent_repairs: usize,

    /// Repair timeout (seconds)
    pub repair_timeout_secs: u64,

    /// Enable rollback on failure
    pub rollback_enabled: bool,

    /// History retention (days)
    pub history_retention_days: u32,
}

impl Default for HealingConfig {
    fn default() -> Self {
        Self {
            auto_heal_enabled: true,
            check_interval_ms: 300000,  // 5 minutes
            anomaly_sensitivity: 0.7,
            performance_threshold: 0.3,  // 30% degradation
            max_concurrent_repairs: 3,
            repair_timeout_secs: 300,
            rollback_enabled: true,
            history_retention_days: 30,
        }
    }
}
```

## Detection Layer

### Anomaly Detection

```rust
pub struct AnomalyDetector {
    /// Baseline statistics
    baseline: BaselineStats,

    /// Detection algorithm
    algorithm: AnomalyAlgorithm,

    /// Recent observations
    observations: RingBuffer<Observation>,

    /// Detected anomalies
    anomalies: Vec<DetectedAnomaly>,
}

#[derive(Clone, Debug)]
pub struct BaselineStats {
    pub latency_mean: f64,
    pub latency_std: f64,
    pub latency_p99: f64,

    pub throughput_mean: f64,
    pub throughput_std: f64,

    pub pattern_hit_rate_mean: f64,
    pub pattern_hit_rate_std: f64,

    pub memory_usage_mean: f64,
    pub memory_usage_std: f64,

    pub sample_count: usize,
    pub last_updated: SystemTime,
}

pub enum AnomalyAlgorithm {
    /// Z-score based detection
    ZScore { threshold: f32 },

    /// Isolation Forest
    IsolationForest { contamination: f32 },

    /// Moving average deviation
    MovingAverage { window: usize, threshold: f32 },
}

impl AnomalyDetector {
    /// Check for anomalies
    pub fn detect(&mut self, observation: &Observation) -> Vec<DetectedAnomaly> {
        self.observations.push(observation.clone());

        let mut anomalies = Vec::new();

        // Latency anomaly
        if let Some(anomaly) = self.check_latency_anomaly(observation) {
            anomalies.push(anomaly);
        }

        // Throughput anomaly
        if let Some(anomaly) = self.check_throughput_anomaly(observation) {
            anomalies.push(anomaly);
        }

        // Pattern hit rate anomaly
        if let Some(anomaly) = self.check_hit_rate_anomaly(observation) {
            anomalies.push(anomaly);
        }

        // Memory anomaly
        if let Some(anomaly) = self.check_memory_anomaly(observation) {
            anomalies.push(anomaly);
        }

        self.anomalies.extend(anomalies.clone());
        anomalies
    }

    fn check_latency_anomaly(&self, obs: &Observation) -> Option<DetectedAnomaly> {
        let z_score = (obs.latency_us as f64 - self.baseline.latency_mean)
            / self.baseline.latency_std;

        if z_score.abs() > 3.0 {
            Some(DetectedAnomaly {
                anomaly_type: AnomalyType::LatencySpike,
                severity: self.compute_severity(z_score),
                observed_value: obs.latency_us as f64,
                expected_value: self.baseline.latency_mean,
                z_score,
                timestamp: obs.timestamp,
            })
        } else {
            None
        }
    }

    fn compute_severity(&self, z_score: f64) -> AnomalySeverity {
        let abs_z = z_score.abs();
        if abs_z > 5.0 {
            AnomalySeverity::Critical
        } else if abs_z > 4.0 {
            AnomalySeverity::High
        } else if abs_z > 3.0 {
            AnomalySeverity::Medium
        } else {
            AnomalySeverity::Low
        }
    }

    /// Update baseline with new observations
    pub fn update_baseline(&mut self) {
        let observations: Vec<_> = self.observations.iter().collect();
        if observations.len() < 100 {
            return;  // Need minimum samples
        }

        // Compute new statistics
        let latencies: Vec<f64> = observations.iter()
            .map(|o| o.latency_us as f64)
            .collect();

        self.baseline.latency_mean = mean(&latencies);
        self.baseline.latency_std = std_dev(&latencies);
        self.baseline.latency_p99 = percentile(&latencies, 99.0);

        // Similar for other metrics...

        self.baseline.sample_count = observations.len();
        self.baseline.last_updated = SystemTime::now();
    }
}

#[derive(Clone, Debug)]
pub struct DetectedAnomaly {
    pub anomaly_type: AnomalyType,
    pub severity: AnomalySeverity,
    pub observed_value: f64,
    pub expected_value: f64,
    pub z_score: f64,
    pub timestamp: SystemTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnomalyType {
    LatencySpike,
    ThroughputDrop,
    HitRateDrop,
    MemorySpike,
    ErrorRateSpike,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}
```

### Index Health Checker

```rust
pub struct IndexHealthChecker {
    /// HNSW index health
    hnsw_health: HashMap<String, HnswHealth>,

    /// IVFFlat index health
    ivfflat_health: HashMap<String, IvfFlatHealth>,

    /// Check history
    history: Vec<IndexHealthCheck>,
}

#[derive(Clone, Debug)]
pub struct HnswHealth {
    pub index_name: String,
    pub table_name: String,

    /// Graph connectivity
    pub connectivity_score: f32,

    /// Layer distribution
    pub layer_distribution: Vec<usize>,

    /// Entry point quality
    pub entry_point_quality: f32,

    /// Orphan nodes
    pub orphan_count: usize,

    /// Fragmentation score (0.0 = none, 1.0 = severe)
    pub fragmentation: f32,

    /// Last check time
    pub last_checked: SystemTime,
}

#[derive(Clone, Debug)]
pub struct IvfFlatHealth {
    pub index_name: String,
    pub table_name: String,

    /// Cluster balance (std dev of cluster sizes)
    pub cluster_balance: f32,

    /// Empty clusters
    pub empty_clusters: usize,

    /// Centroid quality
    pub centroid_quality: f32,

    /// Training staleness (how old is training data)
    pub training_staleness_hours: f32,

    /// Last check time
    pub last_checked: SystemTime,
}

impl IndexHealthChecker {
    /// Check HNSW index health
    pub fn check_hnsw(&mut self, index_name: &str) -> HnswHealth {
        // Connect to PostgreSQL and analyze index

        let health = HnswHealth {
            index_name: index_name.to_string(),
            table_name: self.get_table_name(index_name),
            connectivity_score: self.analyze_hnsw_connectivity(index_name),
            layer_distribution: self.get_layer_distribution(index_name),
            entry_point_quality: self.analyze_entry_point(index_name),
            orphan_count: self.count_orphan_nodes(index_name),
            fragmentation: self.estimate_fragmentation(index_name),
            last_checked: SystemTime::now(),
        };

        self.hnsw_health.insert(index_name.to_string(), health.clone());
        health
    }

    /// Check IVFFlat index health
    pub fn check_ivfflat(&mut self, index_name: &str) -> IvfFlatHealth {
        let health = IvfFlatHealth {
            index_name: index_name.to_string(),
            table_name: self.get_table_name(index_name),
            cluster_balance: self.analyze_cluster_balance(index_name),
            empty_clusters: self.count_empty_clusters(index_name),
            centroid_quality: self.analyze_centroid_quality(index_name),
            training_staleness_hours: self.get_training_staleness(index_name),
            last_checked: SystemTime::now(),
        };

        self.ivfflat_health.insert(index_name.to_string(), health.clone());
        health
    }

    /// Determine if index needs repair
    pub fn needs_repair(&self, index_name: &str) -> Option<IndexIssue> {
        if let Some(health) = self.hnsw_health.get(index_name) {
            if health.fragmentation > 0.5 {
                return Some(IndexIssue::Fragmentation {
                    index: index_name.to_string(),
                    level: health.fragmentation,
                });
            }
            if health.orphan_count > 100 {
                return Some(IndexIssue::OrphanNodes {
                    index: index_name.to_string(),
                    count: health.orphan_count,
                });
            }
            if health.connectivity_score < 0.8 {
                return Some(IndexIssue::PoorConnectivity {
                    index: index_name.to_string(),
                    score: health.connectivity_score,
                });
            }
        }

        if let Some(health) = self.ivfflat_health.get(index_name) {
            if health.cluster_balance > 2.0 {  // High std dev
                return Some(IndexIssue::UnbalancedClusters {
                    index: index_name.to_string(),
                    balance: health.cluster_balance,
                });
            }
            if health.training_staleness_hours > 168.0 {  // 1 week
                return Some(IndexIssue::StaleTraining {
                    index: index_name.to_string(),
                    hours: health.training_staleness_hours,
                });
            }
        }

        None
    }
}

#[derive(Clone, Debug)]
pub enum IndexIssue {
    Fragmentation { index: String, level: f32 },
    OrphanNodes { index: String, count: usize },
    PoorConnectivity { index: String, score: f32 },
    UnbalancedClusters { index: String, balance: f32 },
    StaleTraining { index: String, hours: f32 },
}
```

### Learning Drift Detector

```rust
pub struct LearningDriftDetector {
    /// Pattern quality history
    pattern_quality_history: RingBuffer<f32>,

    /// EWC task count
    ewc_task_history: Vec<usize>,

    /// Drift threshold
    drift_threshold: f32,

    /// Detected drifts
    detected_drifts: Vec<LearningDrift>,
}

impl LearningDriftDetector {
    /// Detect learning drift
    pub fn detect(&mut self, engine: &DagSonaEngine) -> Option<LearningDrift> {
        let metrics = engine.metrics.to_json();

        // Check pattern quality trend
        let current_quality = self.compute_average_pattern_quality(engine);
        self.pattern_quality_history.push(current_quality);

        if self.pattern_quality_history.len() >= 10 {
            let trend = self.compute_trend();

            if trend < -self.drift_threshold {
                return Some(LearningDrift {
                    drift_type: DriftType::QualityDegradation,
                    severity: self.trend_to_severity(trend),
                    trend_value: trend,
                    recommendation: DriftRecommendation::ResetPatterns,
                });
            }
        }

        // Check EWC task explosion
        let ewc_tasks = metrics["ewc_tasks"].as_u64().unwrap_or(0) as usize;
        self.ewc_task_history.push(ewc_tasks);

        if ewc_tasks > 20 {
            return Some(LearningDrift {
                drift_type: DriftType::TaskExplosion,
                severity: DriftSeverity::High,
                trend_value: ewc_tasks as f32,
                recommendation: DriftRecommendation::ConsolidateTasks,
            });
        }

        // Check pattern staleness
        let staleness = self.compute_pattern_staleness(engine);
        if staleness > 0.8 {
            return Some(LearningDrift {
                drift_type: DriftType::PatternStaleness,
                severity: DriftSeverity::Medium,
                trend_value: staleness,
                recommendation: DriftRecommendation::RefreshPatterns,
            });
        }

        None
    }

    fn compute_trend(&self) -> f32 {
        let values: Vec<f32> = self.pattern_quality_history.iter().cloned().collect();
        if values.len() < 5 {
            return 0.0;
        }

        // Simple linear regression slope
        let n = values.len() as f32;
        let x_sum: f32 = (0..values.len()).map(|i| i as f32).sum();
        let y_sum: f32 = values.iter().sum();
        let xy_sum: f32 = values.iter().enumerate()
            .map(|(i, &y)| i as f32 * y)
            .sum();
        let x2_sum: f32 = (0..values.len()).map(|i| (i as f32).powi(2)).sum();

        (n * xy_sum - x_sum * y_sum) / (n * x2_sum - x_sum.powi(2))
    }
}

#[derive(Clone, Debug)]
pub struct LearningDrift {
    pub drift_type: DriftType,
    pub severity: DriftSeverity,
    pub trend_value: f32,
    pub recommendation: DriftRecommendation,
}

#[derive(Clone, Debug)]
pub enum DriftType {
    QualityDegradation,
    TaskExplosion,
    PatternStaleness,
    DistributionShift,
}

#[derive(Clone, Debug)]
pub enum DriftSeverity {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug)]
pub enum DriftRecommendation {
    ResetPatterns,
    ConsolidateTasks,
    RefreshPatterns,
    IncreaseLearningRate,
    ReduceEwcLambda,
}
```

## Repair Strategies

### Repair Strategy Trait

```rust
pub trait RepairStrategy: Send + Sync {
    /// Strategy identifier
    fn name(&self) -> &str;

    /// Check if this strategy can handle the issue
    fn can_repair(&self, issue: &Issue) -> bool;

    /// Execute repair
    fn repair(&self, issue: &Issue, context: &RepairContext) -> Result<RepairResult, RepairError>;

    /// Validate repair success
    fn validate(&self, issue: &Issue, result: &RepairResult) -> bool;

    /// Rollback repair
    fn rollback(&self, result: &RepairResult) -> Result<(), RepairError>;

    /// Estimated repair time
    fn estimated_duration(&self, issue: &Issue) -> Duration;
}
```

### Index Rebalance Strategy

```rust
pub struct IndexRebalanceStrategy;

impl RepairStrategy for IndexRebalanceStrategy {
    fn name(&self) -> &str {
        "index_rebalance"
    }

    fn can_repair(&self, issue: &Issue) -> bool {
        matches!(issue,
            Issue::Index(IndexIssue::Fragmentation { .. }) |
            Issue::Index(IndexIssue::OrphanNodes { .. }) |
            Issue::Index(IndexIssue::UnbalancedClusters { .. })
        )
    }

    fn repair(&self, issue: &Issue, ctx: &RepairContext) -> Result<RepairResult, RepairError> {
        match issue {
            Issue::Index(IndexIssue::Fragmentation { index, level }) => {
                if *level > 0.8 {
                    // Full rebuild required
                    self.rebuild_index(index, ctx)
                } else {
                    // Partial rebalance
                    self.partial_rebalance(index, ctx)
                }
            }

            Issue::Index(IndexIssue::OrphanNodes { index, count }) => {
                // Reconnect orphan nodes
                self.reconnect_orphans(index, *count, ctx)
            }

            Issue::Index(IndexIssue::UnbalancedClusters { index, .. }) => {
                // Retrain clusters
                self.retrain_clusters(index, ctx)
            }

            _ => Err(RepairError::UnsupportedIssue),
        }
    }

    fn validate(&self, issue: &Issue, result: &RepairResult) -> bool {
        // Re-check health after repair
        match issue {
            Issue::Index(idx_issue) => {
                let health_checker = IndexHealthChecker::new();
                match idx_issue {
                    IndexIssue::Fragmentation { index, .. } => {
                        let health = health_checker.check_hnsw(index);
                        health.fragmentation < 0.3
                    }
                    IndexIssue::OrphanNodes { index, .. } => {
                        let health = health_checker.check_hnsw(index);
                        health.orphan_count < 10
                    }
                    _ => true,
                }
            }
            _ => true,
        }
    }

    fn rollback(&self, result: &RepairResult) -> Result<(), RepairError> {
        if let Some(backup) = &result.backup_data {
            // Restore from backup
            self.restore_backup(backup)
        } else {
            Ok(())  // No rollback needed
        }
    }

    fn estimated_duration(&self, issue: &Issue) -> Duration {
        match issue {
            Issue::Index(IndexIssue::Fragmentation { .. }) => Duration::from_secs(300),
            Issue::Index(IndexIssue::OrphanNodes { count, .. }) => {
                Duration::from_secs((*count / 100) as u64 + 10)
            }
            Issue::Index(IndexIssue::UnbalancedClusters { .. }) => Duration::from_secs(120),
            _ => Duration::from_secs(60),
        }
    }
}

impl IndexRebalanceStrategy {
    fn rebuild_index(&self, index: &str, ctx: &RepairContext) -> Result<RepairResult, RepairError> {
        // Create backup
        let backup = self.backup_index(index)?;

        // Drop and recreate
        ctx.execute_sql(&format!("REINDEX INDEX CONCURRENTLY {}", index))?;

        Ok(RepairResult {
            success: true,
            repair_type: "index_rebuild".to_string(),
            duration: ctx.elapsed(),
            backup_data: Some(backup),
            details: json!({
                "index": index,
                "method": "concurrent_reindex",
            }),
        })
    }

    fn partial_rebalance(&self, index: &str, ctx: &RepairContext) -> Result<RepairResult, RepairError> {
        // Identify fragmented regions
        let regions = self.identify_fragmented_regions(index)?;

        // Rebalance each region
        for region in regions {
            self.rebalance_region(index, &region, ctx)?;
        }

        Ok(RepairResult {
            success: true,
            repair_type: "partial_rebalance".to_string(),
            duration: ctx.elapsed(),
            backup_data: None,
            details: json!({
                "index": index,
                "regions_rebalanced": regions.len(),
            }),
        })
    }

    fn reconnect_orphans(&self, index: &str, count: usize, ctx: &RepairContext) -> Result<RepairResult, RepairError> {
        // Find orphan nodes
        let orphans = self.find_orphan_nodes(index)?;

        // Reconnect each to nearest neighbors
        let mut reconnected = 0;
        for orphan in orphans {
            if self.reconnect_node(index, orphan)? {
                reconnected += 1;
            }
        }

        Ok(RepairResult {
            success: reconnected > 0,
            repair_type: "orphan_reconnection".to_string(),
            duration: ctx.elapsed(),
            backup_data: None,
            details: json!({
                "index": index,
                "total_orphans": count,
                "reconnected": reconnected,
            }),
        })
    }
}
```

### Pattern Reset Strategy

```rust
pub struct PatternResetStrategy;

impl RepairStrategy for PatternResetStrategy {
    fn name(&self) -> &str {
        "pattern_reset"
    }

    fn can_repair(&self, issue: &Issue) -> bool {
        matches!(issue,
            Issue::Learning(LearningDrift { drift_type: DriftType::QualityDegradation, .. }) |
            Issue::Learning(LearningDrift { drift_type: DriftType::PatternStaleness, .. })
        )
    }

    fn repair(&self, issue: &Issue, ctx: &RepairContext) -> Result<RepairResult, RepairError> {
        let engine = ctx.get_dag_engine()?;

        match issue {
            Issue::Learning(drift) => {
                match drift.recommendation {
                    DriftRecommendation::ResetPatterns => {
                        // Backup current patterns
                        let backup = self.backup_patterns(&engine)?;

                        // Clear patterns but keep EWC state
                        {
                            let mut bank = engine.dag_reasoning_bank.write();
                            bank.clear_patterns();
                        }

                        // Force immediate learning cycle
                        engine.run_background_cycle()?;

                        Ok(RepairResult {
                            success: true,
                            repair_type: "pattern_reset".to_string(),
                            duration: ctx.elapsed(),
                            backup_data: Some(backup),
                            details: json!({
                                "action": "reset_and_relearn",
                            }),
                        })
                    }

                    DriftRecommendation::RefreshPatterns => {
                        // Keep existing patterns, but force refresh
                        engine.run_background_cycle()?;

                        // Consolidate similar patterns
                        {
                            let mut bank = engine.dag_reasoning_bank.write();
                            bank.consolidate(0.9);
                        }

                        Ok(RepairResult {
                            success: true,
                            repair_type: "pattern_refresh".to_string(),
                            duration: ctx.elapsed(),
                            backup_data: None,
                            details: json!({
                                "action": "refresh_and_consolidate",
                            }),
                        })
                    }

                    _ => Err(RepairError::UnsupportedIssue),
                }
            }
            _ => Err(RepairError::UnsupportedIssue),
        }
    }

    fn validate(&self, _issue: &Issue, _result: &RepairResult) -> bool {
        // Validation will happen over time as new patterns are learned
        true
    }

    fn rollback(&self, result: &RepairResult) -> Result<(), RepairError> {
        if let Some(backup) = &result.backup_data {
            self.restore_patterns(backup)
        } else {
            Ok(())
        }
    }

    fn estimated_duration(&self, _issue: &Issue) -> Duration {
        Duration::from_secs(10)
    }
}
```

## Healing Orchestration

### Main Healing Loop

```rust
impl SelfHealingEngine {
    /// Run healing check cycle
    pub fn run_check_cycle(&mut self) -> HealingCycleResult {
        let start = Instant::now();
        let mut detected_issues = Vec::new();
        let mut repairs_initiated = Vec::new();

        // 1. Anomaly detection
        if let Some(obs) = self.collect_observation() {
            let anomalies = self.anomaly_detector.detect(&obs);
            for anomaly in anomalies {
                detected_issues.push(Issue::Anomaly(anomaly));
            }
        }

        // 2. Index health check
        for index in self.get_monitored_indexes() {
            if let Some(issue) = self.index_health_checker.needs_repair(&index) {
                detected_issues.push(Issue::Index(issue));
            }
        }

        // 3. Learning drift detection
        for engine in self.get_dag_engines() {
            if let Some(drift) = self.learning_drift_detector.detect(&engine) {
                detected_issues.push(Issue::Learning(drift));
            }
        }

        // 4. MinCut bottleneck check
        for engine in self.get_dag_engines() {
            if let Some(mincut) = &engine.mincut_engine {
                let health = mincut.run_health_check(&engine.current_plan);
                for alert in health.alerts {
                    if alert.severity >= AlertSeverity::Warning {
                        detected_issues.push(Issue::Bottleneck(alert));
                    }
                }
            }
        }

        // 5. Prioritize and diagnose
        let prioritized = self.prioritize_issues(&detected_issues);

        // 6. Initiate repairs (if auto-heal enabled)
        if self.config.auto_heal_enabled {
            for issue in &prioritized {
                if self.active_repairs.len() < self.config.max_concurrent_repairs {
                    if let Some(repair) = self.initiate_repair(issue) {
                        repairs_initiated.push(repair);
                    }
                }
            }
        }

        // 7. Check active repairs
        let completed_repairs = self.check_active_repairs();

        HealingCycleResult {
            detected_issues: detected_issues.len(),
            repairs_initiated: repairs_initiated.len(),
            repairs_completed: completed_repairs.len(),
            active_repairs: self.active_repairs.len(),
            duration: start.elapsed(),
        }
    }

    fn prioritize_issues(&self, issues: &[Issue]) -> Vec<Issue> {
        let mut prioritized = issues.to_vec();

        prioritized.sort_by(|a, b| {
            let a_priority = self.compute_priority(a);
            let b_priority = self.compute_priority(b);
            b_priority.cmp(&a_priority)
        });

        prioritized
    }

    fn compute_priority(&self, issue: &Issue) -> u32 {
        match issue {
            Issue::Anomaly(a) => match a.severity {
                AnomalySeverity::Critical => 100,
                AnomalySeverity::High => 80,
                AnomalySeverity::Medium => 50,
                AnomalySeverity::Low => 20,
            },
            Issue::Index(i) => match i {
                IndexIssue::Fragmentation { level, .. } => (level * 100.0) as u32,
                IndexIssue::OrphanNodes { count, .. } => (*count as u32).min(90),
                _ => 50,
            },
            Issue::Learning(d) => match d.severity {
                DriftSeverity::High => 70,
                DriftSeverity::Medium => 40,
                DriftSeverity::Low => 20,
            },
            Issue::Bottleneck(a) => match a.severity {
                AlertSeverity::Critical => 90,
                AlertSeverity::Warning => 60,
                AlertSeverity::Info => 30,
            },
        }
    }

    fn initiate_repair(&mut self, issue: &Issue) -> Option<RepairId> {
        // Find suitable strategy
        let strategy = self.repair_strategies.iter()
            .find(|s| s.can_repair(issue))?;

        let repair_id = self.generate_repair_id();

        // Create repair context
        let context = RepairContext::new();

        // Start repair in background
        let active_repair = ActiveRepair {
            id: repair_id,
            issue: issue.clone(),
            strategy_name: strategy.name().to_string(),
            started_at: Instant::now(),
            status: RepairStatus::InProgress,
        };

        self.active_repairs.insert(repair_id, active_repair);

        // Execute repair
        let result = strategy.repair(issue, &context);

        // Update status
        if let Some(mut repair) = self.active_repairs.get_mut(&repair_id) {
            repair.status = match result {
                Ok(r) if r.success => RepairStatus::Completed(r),
                Ok(r) => RepairStatus::Failed(RepairError::ValidationFailed),
                Err(e) => RepairStatus::Failed(e),
            };
        }

        Some(repair_id)
    }
}
```

## SQL Interface

```sql
-- Get healing status
SELECT ruvector_dag_healing_status();

-- Force health check
SELECT ruvector_dag_health_check('documents');

-- Get detected issues
SELECT * FROM ruvector_dag_detected_issues('documents');

-- Trigger manual repair
SELECT ruvector_dag_repair('documents', 'issue_id');

-- Get repair history
SELECT * FROM ruvector_dag_repair_history('documents', 7);  -- Last 7 days

-- Configure healing
SET ruvector.dag_healing_enabled = true;
SET ruvector.dag_healing_interval_ms = 300000;
SET ruvector.dag_healing_threshold = 0.3;
```

## Metrics and Monitoring

```rust
#[derive(Clone, Debug, Default)]
pub struct HealingMetrics {
    pub checks_performed: AtomicU64,
    pub issues_detected: AtomicU64,
    pub repairs_initiated: AtomicU64,
    pub repairs_successful: AtomicU64,
    pub repairs_failed: AtomicU64,
    pub repairs_rolled_back: AtomicU64,
    pub total_repair_time_ms: AtomicU64,
    pub last_check_time: AtomicU64,
}

impl HealingMetrics {
    pub fn to_json(&self) -> serde_json::Value {
        json!({
            "checks_performed": self.checks_performed.load(Ordering::Relaxed),
            "issues_detected": self.issues_detected.load(Ordering::Relaxed),
            "repairs_initiated": self.repairs_initiated.load(Ordering::Relaxed),
            "repairs_successful": self.repairs_successful.load(Ordering::Relaxed),
            "repairs_failed": self.repairs_failed.load(Ordering::Relaxed),
            "repairs_rolled_back": self.repairs_rolled_back.load(Ordering::Relaxed),
            "success_rate": self.success_rate(),
            "avg_repair_time_ms": self.avg_repair_time(),
        })
    }

    fn success_rate(&self) -> f64 {
        let initiated = self.repairs_initiated.load(Ordering::Relaxed);
        let successful = self.repairs_successful.load(Ordering::Relaxed);
        if initiated > 0 {
            successful as f64 / initiated as f64
        } else {
            1.0
        }
    }
}
```
