//! Discovery engine for detecting novel patterns from coherence signals

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{CoherenceSignal, Result};

/// Configuration for discovery engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Minimum signal strength to consider
    pub min_signal_strength: f64,

    /// Lookback window for trend analysis
    pub lookback_windows: usize,

    /// Threshold for detecting emergence
    pub emergence_threshold: f64,

    /// Threshold for detecting splits
    pub split_threshold: f64,

    /// Threshold for detecting bridges
    pub bridge_threshold: f64,

    /// Enable anomaly detection
    pub detect_anomalies: bool,

    /// Anomaly sensitivity (standard deviations)
    pub anomaly_sigma: f64,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            min_signal_strength: 0.01,
            lookback_windows: 10,
            emergence_threshold: 0.2,
            split_threshold: 0.5,
            bridge_threshold: 0.3,
            detect_anomalies: true,
            anomaly_sigma: 2.5,
        }
    }
}

/// Categories of discoverable patterns
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PatternCategory {
    /// New cluster/community emerging
    Emergence,

    /// Existing structure splitting
    Split,

    /// Two structures merging
    Merge,

    /// Cross-domain connection forming
    Bridge,

    /// Unusual coherence pattern
    Anomaly,

    /// Gradual strengthening
    Consolidation,

    /// Gradual weakening
    Dissolution,

    /// Cyclical pattern detected
    Cyclical,
}

/// Strength of discovered pattern
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub enum PatternStrength {
    /// Weak signal, might be noise
    Weak,

    /// Moderate signal, worth monitoring
    Moderate,

    /// Strong signal, likely real
    Strong,

    /// Very strong signal, high confidence
    VeryStrong,
}

impl PatternStrength {
    /// Convert from numeric score
    pub fn from_score(score: f64) -> Self {
        if score < 0.25 {
            PatternStrength::Weak
        } else if score < 0.5 {
            PatternStrength::Moderate
        } else if score < 0.75 {
            PatternStrength::Strong
        } else {
            PatternStrength::VeryStrong
        }
    }
}

/// A discovered pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryPattern {
    /// Unique pattern identifier
    pub id: String,

    /// Pattern category
    pub category: PatternCategory,

    /// Pattern strength
    pub strength: PatternStrength,

    /// Numeric confidence score (0-1)
    pub confidence: f64,

    /// When pattern was first detected
    pub detected_at: DateTime<Utc>,

    /// Time range pattern spans
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,

    /// Related nodes/entities
    pub entities: Vec<String>,

    /// Description of pattern
    pub description: String,

    /// Supporting evidence
    pub evidence: Vec<PatternEvidence>,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Evidence supporting a pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternEvidence {
    /// Evidence type
    pub evidence_type: String,

    /// Numeric value
    pub value: f64,

    /// Reference to source signal/data
    pub source_ref: String,

    /// Human-readable explanation
    pub explanation: String,
}

/// Discovery engine for pattern detection
pub struct DiscoveryEngine {
    config: DiscoveryConfig,
    patterns: Vec<DiscoveryPattern>,
    signal_history: Vec<CoherenceSignal>,
}

impl DiscoveryEngine {
    /// Create a new discovery engine
    pub fn new(config: DiscoveryConfig) -> Self {
        Self {
            config,
            patterns: Vec::new(),
            signal_history: Vec::new(),
        }
    }

    /// Detect patterns from coherence signals
    pub fn detect(&mut self, signals: &[CoherenceSignal]) -> Result<Vec<DiscoveryPattern>> {
        self.signal_history.extend(signals.iter().cloned());

        let mut patterns = Vec::new();

        // Need at least 2 signals to detect patterns
        if self.signal_history.len() < 2 {
            return Ok(patterns);
        }

        // Detect different pattern types
        patterns.extend(self.detect_emergence()?);
        patterns.extend(self.detect_splits()?);
        patterns.extend(self.detect_bridges()?);
        patterns.extend(self.detect_trends()?);

        if self.config.detect_anomalies {
            patterns.extend(self.detect_anomalies()?);
        }

        self.patterns.extend(patterns.clone());
        Ok(patterns)
    }

    /// Detect emerging structures
    fn detect_emergence(&self) -> Result<Vec<DiscoveryPattern>> {
        let mut patterns = Vec::new();

        if self.signal_history.len() < self.config.lookback_windows {
            return Ok(patterns);
        }

        let recent = &self.signal_history[self.signal_history.len() - self.config.lookback_windows..];

        // Look for sustained growth in node/edge count with increasing coherence
        let node_growth: Vec<i64> = recent
            .windows(2)
            .map(|w| w[1].node_count as i64 - w[0].node_count as i64)
            .collect();

        let avg_growth = node_growth.iter().sum::<i64>() as f64 / node_growth.len() as f64;

        if avg_growth > self.config.emergence_threshold * recent[0].node_count as f64 {
            let latest = recent.last().unwrap();

            patterns.push(DiscoveryPattern {
                id: format!("emergence_{}", self.patterns.len()),
                category: PatternCategory::Emergence,
                strength: PatternStrength::from_score(avg_growth / 10.0),
                confidence: (avg_growth / 10.0).min(1.0),
                detected_at: Utc::now(),
                time_range: Some((recent[0].window.start, latest.window.end)),
                entities: latest.cut_nodes.clone(),
                description: format!(
                    "Emerging structure detected: {} new nodes over {} windows",
                    (avg_growth * recent.len() as f64) as i64,
                    recent.len()
                ),
                evidence: vec![PatternEvidence {
                    evidence_type: "node_growth".to_string(),
                    value: avg_growth,
                    source_ref: latest.id.clone(),
                    explanation: "Sustained node count growth".to_string(),
                }],
                metadata: HashMap::new(),
            });
        }

        Ok(patterns)
    }

    /// Detect structure splits
    fn detect_splits(&self) -> Result<Vec<DiscoveryPattern>> {
        let mut patterns = Vec::new();

        if self.signal_history.len() < 2 {
            return Ok(patterns);
        }

        // Look for sudden drops in min-cut value
        for i in 1..self.signal_history.len() {
            let prev = &self.signal_history[i - 1];
            let curr = &self.signal_history[i];

            if prev.min_cut_value > 0.0 {
                let drop_ratio = (prev.min_cut_value - curr.min_cut_value) / prev.min_cut_value;

                if drop_ratio > self.config.split_threshold {
                    patterns.push(DiscoveryPattern {
                        id: format!("split_{}", self.patterns.len()),
                        category: PatternCategory::Split,
                        strength: PatternStrength::from_score(drop_ratio),
                        confidence: drop_ratio.min(1.0),
                        detected_at: curr.window.start,
                        time_range: Some((prev.window.start, curr.window.end)),
                        entities: curr.cut_nodes.clone(),
                        description: format!(
                            "Structure split detected: {:.1}% coherence drop",
                            drop_ratio * 100.0
                        ),
                        evidence: vec![PatternEvidence {
                            evidence_type: "mincut_drop".to_string(),
                            value: drop_ratio,
                            source_ref: curr.id.clone(),
                            explanation: format!(
                                "Min-cut dropped from {:.3} to {:.3}",
                                prev.min_cut_value, curr.min_cut_value
                            ),
                        }],
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        Ok(patterns)
    }

    /// Detect cross-domain bridges
    fn detect_bridges(&self) -> Result<Vec<DiscoveryPattern>> {
        let mut patterns = Vec::new();

        if self.signal_history.is_empty() {
            return Ok(patterns);
        }

        // Look for nodes that appear in cut boundaries frequently
        let mut boundary_counts: HashMap<String, usize> = HashMap::new();

        for signal in &self.signal_history {
            for node in &signal.cut_nodes {
                *boundary_counts.entry(node.clone()).or_default() += 1;
            }
        }

        let threshold = (self.signal_history.len() as f64 * self.config.bridge_threshold) as usize;

        let bridge_nodes: Vec<_> = boundary_counts
            .iter()
            .filter(|(_, &count)| count >= threshold)
            .map(|(node, &count)| (node.clone(), count))
            .collect();

        if !bridge_nodes.is_empty() {
            let latest = self.signal_history.last().unwrap();

            patterns.push(DiscoveryPattern {
                id: format!("bridge_{}", self.patterns.len()),
                category: PatternCategory::Bridge,
                strength: PatternStrength::Moderate,
                confidence: 0.6,
                detected_at: Utc::now(),
                time_range: Some((
                    self.signal_history[0].window.start,
                    latest.window.end,
                )),
                entities: bridge_nodes.iter().map(|(n, _)| n.clone()).collect(),
                description: format!(
                    "Bridge nodes detected: {} nodes consistently on boundaries",
                    bridge_nodes.len()
                ),
                evidence: bridge_nodes
                    .iter()
                    .map(|(node, count)| PatternEvidence {
                        evidence_type: "boundary_frequency".to_string(),
                        value: *count as f64,
                        source_ref: node.clone(),
                        explanation: format!("{} appeared in {} cut boundaries", node, count),
                    })
                    .collect(),
                metadata: HashMap::new(),
            });
        }

        Ok(patterns)
    }

    /// Detect trends (consolidation/dissolution)
    fn detect_trends(&self) -> Result<Vec<DiscoveryPattern>> {
        let mut patterns = Vec::new();

        if self.signal_history.len() < self.config.lookback_windows {
            return Ok(patterns);
        }

        let recent = &self.signal_history[self.signal_history.len() - self.config.lookback_windows..];

        // Calculate trend in min-cut values
        let values: Vec<f64> = recent.iter().map(|s| s.min_cut_value).collect();

        let (slope, _) = self.linear_regression(&values);

        if slope.abs() > 0.1 {
            let latest = recent.last().unwrap();
            let category = if slope > 0.0 {
                PatternCategory::Consolidation
            } else {
                PatternCategory::Dissolution
            };

            patterns.push(DiscoveryPattern {
                id: format!("trend_{}", self.patterns.len()),
                category,
                strength: PatternStrength::from_score(slope.abs()),
                confidence: slope.abs().min(1.0),
                detected_at: Utc::now(),
                time_range: Some((recent[0].window.start, latest.window.end)),
                entities: vec![],
                description: format!(
                    "{} trend detected: {:.2}% per window",
                    if slope > 0.0 {
                        "Strengthening"
                    } else {
                        "Weakening"
                    },
                    slope * 100.0
                ),
                evidence: vec![PatternEvidence {
                    evidence_type: "trend_slope".to_string(),
                    value: slope,
                    source_ref: latest.id.clone(),
                    explanation: format!(
                        "Linear trend slope: {:.4} over {} windows",
                        slope,
                        recent.len()
                    ),
                }],
                metadata: HashMap::new(),
            });
        }

        Ok(patterns)
    }

    /// Detect anomalies
    fn detect_anomalies(&self) -> Result<Vec<DiscoveryPattern>> {
        let mut patterns = Vec::new();

        if self.signal_history.len() < 5 {
            return Ok(patterns);
        }

        // Calculate mean and std dev of min-cut values
        let values: Vec<f64> = self.signal_history.iter().map(|s| s.min_cut_value).collect();

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance =
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        // Find anomalies
        for (i, signal) in self.signal_history.iter().enumerate() {
            let z_score = if std_dev > 0.0 {
                (signal.min_cut_value - mean) / std_dev
            } else {
                0.0
            };

            if z_score.abs() > self.config.anomaly_sigma {
                patterns.push(DiscoveryPattern {
                    id: format!("anomaly_{}", i),
                    category: PatternCategory::Anomaly,
                    strength: PatternStrength::from_score(z_score.abs() / 5.0),
                    confidence: (z_score.abs() / 5.0).min(1.0),
                    detected_at: signal.window.start,
                    time_range: Some((signal.window.start, signal.window.end)),
                    entities: signal.cut_nodes.clone(),
                    description: format!(
                        "Anomalous coherence: {:.2}σ from mean",
                        z_score
                    ),
                    evidence: vec![PatternEvidence {
                        evidence_type: "z_score".to_string(),
                        value: z_score,
                        source_ref: signal.id.clone(),
                        explanation: format!(
                            "Value {:.4} vs mean {:.4} (σ={:.4})",
                            signal.min_cut_value, mean, std_dev
                        ),
                    }],
                    metadata: HashMap::new(),
                });
            }
        }

        Ok(patterns)
    }

    /// Simple linear regression
    fn linear_regression(&self, values: &[f64]) -> (f64, f64) {
        let n = values.len() as f64;
        let x_mean = (n - 1.0) / 2.0;
        let y_mean = values.iter().sum::<f64>() / n;

        let mut num = 0.0;
        let mut denom = 0.0;

        for (i, &y) in values.iter().enumerate() {
            let x = i as f64;
            num += (x - x_mean) * (y - y_mean);
            denom += (x - x_mean).powi(2);
        }

        let slope = if denom > 0.0 { num / denom } else { 0.0 };
        let intercept = y_mean - slope * x_mean;

        (slope, intercept)
    }

    /// Get all discovered patterns
    pub fn patterns(&self) -> &[DiscoveryPattern] {
        &self.patterns
    }

    /// Get patterns by category
    pub fn patterns_by_category(&self, category: PatternCategory) -> Vec<&DiscoveryPattern> {
        self.patterns
            .iter()
            .filter(|p| p.category == category)
            .collect()
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.patterns.clear();
        self.signal_history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TemporalWindow;

    fn make_signal(id: &str, min_cut: f64, nodes: usize) -> CoherenceSignal {
        CoherenceSignal {
            id: id.to_string(),
            window: TemporalWindow::new(Utc::now(), Utc::now(), 0),
            min_cut_value: min_cut,
            node_count: nodes,
            edge_count: nodes * 2,
            partition_sizes: Some((nodes / 2, nodes - nodes / 2)),
            is_exact: true,
            cut_nodes: vec![],
            delta: None,
        }
    }

    #[test]
    fn test_discovery_engine_creation() {
        let config = DiscoveryConfig::default();
        let engine = DiscoveryEngine::new(config);
        assert!(engine.patterns().is_empty());
    }

    #[test]
    fn test_pattern_strength() {
        assert_eq!(PatternStrength::from_score(0.1), PatternStrength::Weak);
        assert_eq!(PatternStrength::from_score(0.3), PatternStrength::Moderate);
        assert_eq!(PatternStrength::from_score(0.6), PatternStrength::Strong);
        assert_eq!(
            PatternStrength::from_score(0.9),
            PatternStrength::VeryStrong
        );
    }

    #[test]
    fn test_empty_signals() {
        let config = DiscoveryConfig::default();
        let mut engine = DiscoveryEngine::new(config);

        let patterns = engine.detect(&[]).unwrap();
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_linear_regression() {
        let config = DiscoveryConfig::default();
        let engine = DiscoveryEngine::new(config);

        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let (slope, intercept) = engine.linear_regression(&values);

        assert!((slope - 1.0).abs() < 0.001);
        assert!((intercept - 1.0).abs() < 0.001);
    }
}
