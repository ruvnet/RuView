//! Financial coherence analysis using RuVector's min-cut

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Company, Filing, FilingAnalyzer, FinancialStatement, PeerNetwork, XbrlParser, xbrl::statement_to_embedding};
use crate::filings::{NarrativeExtractor, FilingAnalysis};

/// A coherence alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceAlert {
    /// Alert identifier
    pub id: String,

    /// Company CIK
    pub company_cik: String,

    /// Company name
    pub company_name: String,

    /// Alert timestamp
    pub timestamp: DateTime<Utc>,

    /// Alert severity
    pub severity: AlertSeverity,

    /// Divergence type
    pub divergence_type: DivergenceType,

    /// Coherence score before (0-1)
    pub coherence_before: f64,

    /// Coherence score after (0-1)
    pub coherence_after: f64,

    /// Magnitude of change
    pub magnitude: f64,

    /// Fundamental vector component
    pub fundamental_score: f64,

    /// Narrative vector component
    pub narrative_score: f64,

    /// Peer comparison (z-score)
    pub peer_z_score: f64,

    /// Related companies
    pub related_companies: Vec<String>,

    /// Interpretation
    pub interpretation: String,

    /// Evidence
    pub evidence: Vec<AlertEvidence>,
}

/// Alert severity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub enum AlertSeverity {
    /// Informational
    Info,
    /// Low concern
    Low,
    /// Moderate concern
    Medium,
    /// High concern
    High,
    /// Critical concern
    Critical,
}

impl AlertSeverity {
    /// From magnitude
    pub fn from_magnitude(magnitude: f64) -> Self {
        if magnitude < 0.1 {
            AlertSeverity::Info
        } else if magnitude < 0.2 {
            AlertSeverity::Low
        } else if magnitude < 0.3 {
            AlertSeverity::Medium
        } else if magnitude < 0.5 {
            AlertSeverity::High
        } else {
            AlertSeverity::Critical
        }
    }
}

/// Type of divergence detected
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DivergenceType {
    /// Fundamentals improving, narrative pessimistic
    FundamentalOutpacing,

    /// Narrative optimistic, fundamentals declining
    NarrativeLeading,

    /// Company diverging from peer group
    PeerDivergence,

    /// Sector-wide pattern change
    SectorShift,

    /// Unusual cross-metric divergence
    MetricAnomaly,

    /// Historical pattern break
    PatternBreak,
}

/// Evidence for an alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvidence {
    /// Evidence type
    pub evidence_type: String,

    /// Numeric value
    pub value: f64,

    /// Explanation
    pub explanation: String,
}

/// Coherence watch for financial monitoring
pub struct CoherenceWatch {
    /// Configuration
    config: WatchConfig,

    /// Peer network
    network: PeerNetwork,

    /// Historical coherence by company
    coherence_history: HashMap<String, Vec<(DateTime<Utc>, f64)>>,

    /// Detected alerts
    alerts: Vec<CoherenceAlert>,

    /// Filing analyzer
    filing_analyzer: FilingAnalyzer,

    /// XBRL parser
    xbrl_parser: XbrlParser,

    /// Narrative extractor
    narrative_extractor: NarrativeExtractor,
}

/// Watch configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    /// Weight for fundamental metrics
    pub fundamental_weight: f64,

    /// Weight for narrative analysis
    pub narrative_weight: f64,

    /// Weight for peer comparison
    pub peer_weight: f64,

    /// Minimum divergence to alert
    pub divergence_threshold: f64,

    /// Lookback quarters for trend analysis
    pub lookback_quarters: usize,

    /// Enable peer comparison
    pub compare_peers: bool,

    /// Alert on sector-wide shifts
    pub sector_alerts: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            fundamental_weight: 0.4,
            narrative_weight: 0.3,
            peer_weight: 0.3,
            divergence_threshold: 0.2,
            lookback_quarters: 8,
            compare_peers: true,
            sector_alerts: true,
        }
    }
}

impl CoherenceWatch {
    /// Create a new coherence watch
    pub fn new(network: PeerNetwork, config: WatchConfig) -> Self {
        Self {
            config,
            network,
            coherence_history: HashMap::new(),
            alerts: Vec::new(),
            filing_analyzer: FilingAnalyzer::new(Default::default()),
            xbrl_parser: XbrlParser::new(Default::default()),
            narrative_extractor: NarrativeExtractor::new(Default::default()),
        }
    }

    /// Analyze a company for coherence
    pub fn analyze_company(
        &mut self,
        company: &Company,
        filings: &[Filing],
        statements: &[FinancialStatement],
        filing_contents: &HashMap<String, String>,
    ) -> Option<CoherenceAlert> {
        if filings.is_empty() || statements.is_empty() {
            return None;
        }

        // Compute fundamental vector
        let latest_statement = statements.last()?;
        let fundamental_embedding = statement_to_embedding(latest_statement);

        // Compute narrative vector
        let latest_filing = filings.last()?;
        let content = filing_contents.get(&latest_filing.accession_number)?;
        let analysis = self.filing_analyzer.analyze(content, latest_filing);
        let narrative_embedding = self.narrative_extractor.extract_embedding(&analysis);

        // Compute coherence score
        let coherence = self.compute_coherence(&fundamental_embedding, &narrative_embedding);

        // Get historical coherence to check for significant change
        let cik = &company.cik;
        let should_alert = {
            let history = self.coherence_history.entry(cik.clone()).or_default();
            if !history.is_empty() {
                let prev_coherence = history.last()?.1;
                let delta = (coherence - prev_coherence).abs();
                if delta > self.config.divergence_threshold {
                    Some(prev_coherence)
                } else {
                    None
                }
            } else {
                None
            }
        };

        // Create alert if needed (outside the mutable borrow scope)
        let alert = should_alert.map(|prev_coherence| {
            self.create_alert(
                company,
                prev_coherence,
                coherence,
                &fundamental_embedding,
                &narrative_embedding,
                &analysis,
            )
        });

        // Update history
        self.coherence_history
            .entry(cik.clone())
            .or_default()
            .push((Utc::now(), coherence));

        alert
    }

    /// Compute coherence between fundamental and narrative vectors
    fn compute_coherence(&self, fundamental: &[f32], narrative: &[f32]) -> f64 {
        // Cosine similarity
        let dot_product: f32 = fundamental.iter()
            .zip(narrative.iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_f: f32 = fundamental.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_n: f32 = narrative.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_f > 0.0 && norm_n > 0.0 {
            ((dot_product / (norm_f * norm_n) + 1.0) / 2.0) as f64 // Scale to 0-1
        } else {
            0.5
        }
    }

    /// Create an alert from analysis
    fn create_alert(
        &self,
        company: &Company,
        prev_coherence: f64,
        curr_coherence: f64,
        fundamental: &[f32],
        narrative: &[f32],
        analysis: &FilingAnalysis,
    ) -> CoherenceAlert {
        let magnitude = (curr_coherence - prev_coherence).abs();
        let severity = AlertSeverity::from_magnitude(magnitude);

        // Determine divergence type
        let fundamental_score: f64 = fundamental.iter().map(|x| *x as f64).sum::<f64>() / fundamental.len() as f64;
        let narrative_score = analysis.sentiment.unwrap_or(0.0);

        let divergence_type = if fundamental_score > 0.0 && narrative_score < 0.0 {
            DivergenceType::FundamentalOutpacing
        } else if narrative_score > 0.0 && fundamental_score < 0.0 {
            DivergenceType::NarrativeLeading
        } else {
            DivergenceType::PatternBreak
        };

        // Compute peer z-score (simplified)
        let peer_z_score = self.compute_peer_z_score(&company.cik, curr_coherence);

        // Build evidence
        let evidence = vec![
            AlertEvidence {
                evidence_type: "coherence_change".to_string(),
                value: magnitude,
                explanation: format!(
                    "Coherence {} by {:.1}%",
                    if curr_coherence > prev_coherence { "increased" } else { "decreased" },
                    magnitude * 100.0
                ),
            },
            AlertEvidence {
                evidence_type: "fundamental_score".to_string(),
                value: fundamental_score,
                explanation: format!("Fundamental metric score: {:.3}", fundamental_score),
            },
            AlertEvidence {
                evidence_type: "narrative_sentiment".to_string(),
                value: narrative_score,
                explanation: format!("Narrative sentiment: {:.3}", narrative_score),
            },
        ];

        let interpretation = self.interpret_divergence(divergence_type, severity, peer_z_score);

        CoherenceAlert {
            id: format!("alert_{}_{}", company.cik, Utc::now().timestamp()),
            company_cik: company.cik.clone(),
            company_name: company.name.clone(),
            timestamp: Utc::now(),
            severity,
            divergence_type,
            coherence_before: prev_coherence,
            coherence_after: curr_coherence,
            magnitude,
            fundamental_score,
            narrative_score,
            peer_z_score,
            related_companies: self.find_related_companies(&company.cik),
            interpretation,
            evidence,
        }
    }

    /// Compute peer group z-score
    fn compute_peer_z_score(&self, cik: &str, coherence: f64) -> f64 {
        let peer_coherences: Vec<f64> = self.coherence_history
            .iter()
            .filter(|(k, _)| *k != cik)
            .filter_map(|(_, history)| history.last().map(|(_, c)| *c))
            .collect();

        if peer_coherences.len() < 2 {
            return 0.0;
        }

        let mean: f64 = peer_coherences.iter().sum::<f64>() / peer_coherences.len() as f64;
        let variance: f64 = peer_coherences.iter().map(|c| (c - mean).powi(2)).sum::<f64>()
            / peer_coherences.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev > 0.0 {
            (coherence - mean) / std_dev
        } else {
            0.0
        }
    }

    /// Find related companies from network
    fn find_related_companies(&self, cik: &str) -> Vec<String> {
        self.network.get_peers(cik)
            .iter()
            .take(5)
            .map(|p| p.to_string())
            .collect()
    }

    /// Interpret divergence
    fn interpret_divergence(
        &self,
        divergence_type: DivergenceType,
        severity: AlertSeverity,
        peer_z_score: f64,
    ) -> String {
        let severity_str = match severity {
            AlertSeverity::Info => "Minor",
            AlertSeverity::Low => "Notable",
            AlertSeverity::Medium => "Significant",
            AlertSeverity::High => "Major",
            AlertSeverity::Critical => "Critical",
        };

        let divergence_str = match divergence_type {
            DivergenceType::FundamentalOutpacing =>
                "Fundamentals improving faster than narrative suggests",
            DivergenceType::NarrativeLeading =>
                "Narrative more optimistic than fundamentals support",
            DivergenceType::PeerDivergence =>
                "Company diverging from peer group pattern",
            DivergenceType::SectorShift =>
                "Sector-wide coherence shift detected",
            DivergenceType::MetricAnomaly =>
                "Unusual cross-metric relationship detected",
            DivergenceType::PatternBreak =>
                "Historical coherence pattern broken",
        };

        let peer_context = if peer_z_score.abs() > 2.0 {
            format!(". Company is {:.1} std devs from peer mean",  peer_z_score)
        } else {
            String::new()
        };

        format!("{} divergence: {}{}", severity_str, divergence_str, peer_context)
    }

    /// Detect sector-wide coherence shifts
    pub fn detect_sector_shifts(&self) -> Vec<CoherenceAlert> {
        // Would analyze all companies in sector using min-cut on peer network
        vec![]
    }

    /// Get all alerts
    pub fn alerts(&self) -> &[CoherenceAlert] {
        &self.alerts
    }

    /// Get alerts by severity
    pub fn alerts_by_severity(&self, min_severity: AlertSeverity) -> Vec<&CoherenceAlert> {
        self.alerts
            .iter()
            .filter(|a| a.severity >= min_severity)
            .collect()
    }

    /// Get company coherence history
    pub fn coherence_history(&self, cik: &str) -> Option<&Vec<(DateTime<Utc>, f64)>> {
        self.coherence_history.get(cik)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::PeerNetworkBuilder;

    #[test]
    fn test_alert_severity() {
        assert_eq!(AlertSeverity::from_magnitude(0.05), AlertSeverity::Info);
        assert_eq!(AlertSeverity::from_magnitude(0.15), AlertSeverity::Low);
        assert_eq!(AlertSeverity::from_magnitude(0.25), AlertSeverity::Medium);
        assert_eq!(AlertSeverity::from_magnitude(0.4), AlertSeverity::High);
        assert_eq!(AlertSeverity::from_magnitude(0.6), AlertSeverity::Critical);
    }

    #[test]
    fn test_coherence_computation() {
        let network = PeerNetworkBuilder::new().build();
        let config = WatchConfig::default();
        let watch = CoherenceWatch::new(network, config);

        let vec_a = vec![1.0, 0.0, 0.0];
        let vec_b = vec![1.0, 0.0, 0.0];
        let coherence = watch.compute_coherence(&vec_a, &vec_b);
        assert!((coherence - 1.0).abs() < 0.001);

        let vec_c = vec![-1.0, 0.0, 0.0];
        let coherence_neg = watch.compute_coherence(&vec_a, &vec_c);
        assert!((coherence_neg - 0.0).abs() < 0.001);
    }
}
