//! # RuVector SEC EDGAR Integration
//!
//! Integration with SEC EDGAR for financial intelligence, peer group coherence
//! analysis, and narrative drift detection.
//!
//! ## Core Capabilities
//!
//! - **Peer Network Graph**: Model company relationships via shared investors, sectors
//! - **Coherence Watch**: Detect when fundamentals diverge from narrative (10-K text)
//! - **Risk Signal Detection**: Use min-cut for structural discontinuities
//! - **Cross-Company Analysis**: Track contagion and sector-wide patterns
//!
//! ## Data Sources
//!
//! ### SEC EDGAR
//! - **XBRL Financial Statements**: Standardized accounting data (2009-present)
//! - **10-K/10-Q Filings**: Annual/quarterly reports with narrative
//! - **Form 4**: Insider trading disclosures
//! - **13F**: Institutional holdings
//! - **8-K**: Material events
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ruvector_data_edgar::{
//!     EdgarClient, PeerNetwork, CoherenceWatch, XbrlParser, FilingAnalyzer,
//! };
//!
//! // Build peer network from 13F holdings
//! let network = PeerNetwork::from_sector("technology")
//!     .with_min_market_cap(1_000_000_000)
//!     .build()
//!     .await?;
//!
//! // Create coherence watch
//! let watch = CoherenceWatch::new(network);
//!
//! // Analyze for divergence
//! let alerts = watch.detect_divergence(
//!     narrative_weight: 0.4,
//!     lookback_quarters: 8,
//! ).await?;
//!
//! for alert in alerts {
//!     println!("{}: {}", alert.company, alert.interpretation);
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod client;
pub mod xbrl;
pub mod filings;
pub mod coherence;
pub mod network;

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use client::EdgarClient;
pub use xbrl::{XbrlParser, FinancialStatement, XbrlFact, XbrlContext};
pub use filings::{Filing, FilingType, FilingAnalyzer, NarrativeExtractor};
pub use coherence::{CoherenceWatch, CoherenceAlert, AlertSeverity, DivergenceType};
pub use network::{PeerNetwork, PeerNetworkBuilder, CompanyNode, PeerEdge};

use ruvector_data_framework::{DataRecord, DataSource, FrameworkError, Relationship, Result};

/// EDGAR-specific error types
#[derive(Error, Debug)]
pub enum EdgarError {
    /// API request failed
    #[error("API error: {0}")]
    Api(String),

    /// Invalid CIK
    #[error("Invalid CIK: {0}")]
    InvalidCik(String),

    /// XBRL parsing failed
    #[error("XBRL parse error: {0}")]
    XbrlParse(String),

    /// Filing not found
    #[error("Filing not found: {0}")]
    FilingNotFound(String),

    /// Network error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Data format error
    #[error("Data format error: {0}")]
    DataFormat(String),
}

impl From<EdgarError> for FrameworkError {
    fn from(e: EdgarError) -> Self {
        FrameworkError::Ingestion(e.to_string())
    }
}

/// Configuration for EDGAR data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgarConfig {
    /// User agent (required by SEC)
    pub user_agent: String,

    /// Company name for user agent
    pub company_name: String,

    /// Contact email (required by SEC)
    pub contact_email: String,

    /// Rate limit (requests per second)
    pub rate_limit: u32,

    /// Include historical data
    pub include_historical: bool,

    /// Filing types to fetch
    pub filing_types: Vec<FilingType>,
}

impl Default for EdgarConfig {
    fn default() -> Self {
        Self {
            user_agent: "RuVector/0.1.0".to_string(),
            company_name: "Research Project".to_string(),
            contact_email: "contact@example.com".to_string(),
            rate_limit: 10, // SEC allows 10 requests/second
            include_historical: true,
            filing_types: vec![FilingType::TenK, FilingType::TenQ],
        }
    }
}

/// A company entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    /// CIK (Central Index Key)
    pub cik: String,

    /// Company name
    pub name: String,

    /// Ticker symbol
    pub ticker: Option<String>,

    /// SIC code (industry)
    pub sic_code: Option<String>,

    /// SIC description
    pub sic_description: Option<String>,

    /// State of incorporation
    pub state: Option<String>,

    /// Fiscal year end
    pub fiscal_year_end: Option<String>,

    /// Latest filing date
    pub latest_filing: Option<NaiveDate>,
}

/// A financial metric
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialMetric {
    /// Company CIK
    pub cik: String,

    /// Filing accession number
    pub accession: String,

    /// Report date
    pub report_date: NaiveDate,

    /// Metric name (XBRL tag)
    pub metric_name: String,

    /// Value
    pub value: f64,

    /// Unit
    pub unit: String,

    /// Is audited
    pub audited: bool,

    /// Context (annual, quarterly, etc.)
    pub context: String,
}

/// Financial ratio
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FinancialRatio {
    /// Current ratio (current assets / current liabilities)
    CurrentRatio,
    /// Quick ratio ((current assets - inventory) / current liabilities)
    QuickRatio,
    /// Debt to equity
    DebtToEquity,
    /// Return on equity
    ReturnOnEquity,
    /// Return on assets
    ReturnOnAssets,
    /// Gross margin
    GrossMargin,
    /// Operating margin
    OperatingMargin,
    /// Net margin
    NetMargin,
    /// Asset turnover
    AssetTurnover,
    /// Inventory turnover
    InventoryTurnover,
    /// Price to earnings
    PriceToEarnings,
    /// Price to book
    PriceToBook,
}

impl FinancialRatio {
    /// Compute ratio from financial data
    pub fn compute(&self, data: &HashMap<String, f64>) -> Option<f64> {
        match self {
            FinancialRatio::CurrentRatio => {
                let current_assets = data.get("Assets Current")?;
                let current_liabilities = data.get("Liabilities Current")?;
                if *current_liabilities != 0.0 {
                    Some(current_assets / current_liabilities)
                } else {
                    None
                }
            }
            FinancialRatio::DebtToEquity => {
                let total_debt = data.get("Debt")?;
                let equity = data.get("Stockholders Equity")?;
                if *equity != 0.0 {
                    Some(total_debt / equity)
                } else {
                    None
                }
            }
            FinancialRatio::NetMargin => {
                let net_income = data.get("Net Income")?;
                let revenue = data.get("Revenue")?;
                if *revenue != 0.0 {
                    Some(net_income / revenue)
                } else {
                    None
                }
            }
            FinancialRatio::ReturnOnEquity => {
                let net_income = data.get("Net Income")?;
                let equity = data.get("Stockholders Equity")?;
                if *equity != 0.0 {
                    Some(net_income / equity)
                } else {
                    None
                }
            }
            FinancialRatio::ReturnOnAssets => {
                let net_income = data.get("Net Income")?;
                let assets = data.get("Assets")?;
                if *assets != 0.0 {
                    Some(net_income / assets)
                } else {
                    None
                }
            }
            _ => None, // Add more implementations as needed
        }
    }
}

/// Sector classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Sector {
    /// Technology
    Technology,
    /// Healthcare
    Healthcare,
    /// Financial services
    Financials,
    /// Consumer discretionary
    ConsumerDiscretionary,
    /// Consumer staples
    ConsumerStaples,
    /// Energy
    Energy,
    /// Materials
    Materials,
    /// Industrials
    Industrials,
    /// Utilities
    Utilities,
    /// Real estate
    RealEstate,
    /// Communication services
    CommunicationServices,
    /// Other/Unknown
    Other,
}

impl Sector {
    /// Get sector from SIC code
    pub fn from_sic(sic: &str) -> Self {
        match sic.chars().next() {
            Some('7') => Sector::Technology,
            Some('8') => Sector::Healthcare,
            Some('6') => Sector::Financials,
            Some('5') => Sector::ConsumerDiscretionary,
            Some('2') => Sector::ConsumerStaples,
            Some('1') => Sector::Energy,
            Some('3') => Sector::Materials,
            Some('4') => Sector::Industrials,
            _ => Sector::Other,
        }
    }
}

/// EDGAR data source for the framework
pub struct EdgarSource {
    client: EdgarClient,
    config: EdgarConfig,
    ciks: Vec<String>,
}

impl EdgarSource {
    /// Create a new EDGAR data source
    pub fn new(config: EdgarConfig) -> Self {
        let client = EdgarClient::new(
            &config.user_agent,
            &config.company_name,
            &config.contact_email,
        );

        Self {
            client,
            config,
            ciks: Vec::new(),
        }
    }

    /// Add CIKs to fetch
    pub fn with_ciks(mut self, ciks: Vec<String>) -> Self {
        self.ciks = ciks;
        self
    }

    /// Add companies by ticker
    pub async fn with_tickers(mut self, tickers: &[&str]) -> Result<Self> {
        for ticker in tickers {
            if let Ok(cik) = self.client.ticker_to_cik(ticker).await {
                self.ciks.push(cik);
            }
        }
        Ok(self)
    }

    /// Add all companies in a sector
    pub async fn with_sector(mut self, sector: Sector) -> Result<Self> {
        let companies = self.client.get_companies_by_sector(&sector).await?;
        self.ciks.extend(companies.into_iter().map(|c| c.cik));
        Ok(self)
    }
}

#[async_trait]
impl DataSource for EdgarSource {
    fn source_id(&self) -> &str {
        "edgar"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        let start_idx: usize = cursor.as_ref().and_then(|c| c.parse().ok()).unwrap_or(0);

        let end_idx = (start_idx + batch_size).min(self.ciks.len());

        let mut records = Vec::new();

        for cik in &self.ciks[start_idx..end_idx] {
            // Fetch filings for this CIK
            match self.client.get_filings(cik, &self.config.filing_types).await {
                Ok(filings) => {
                    for filing in filings {
                        records.push(filing_to_record(filing));
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch filings for CIK {}: {}", cik, e);
                }
            }

            // Rate limiting
            if self.config.rate_limit > 0 {
                let delay = 1000 / self.config.rate_limit as u64;
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }
        }

        let next_cursor = if end_idx < self.ciks.len() {
            Some(end_idx.to_string())
        } else {
            None
        };

        Ok((records, next_cursor))
    }

    async fn total_count(&self) -> Result<Option<u64>> {
        Ok(Some(self.ciks.len() as u64))
    }

    async fn health_check(&self) -> Result<bool> {
        self.client.health_check().await.map_err(|e| e.into())
    }
}

/// Convert a filing to a data record
fn filing_to_record(filing: Filing) -> DataRecord {
    let mut relationships = Vec::new();

    // Company relationship
    relationships.push(Relationship {
        target_id: filing.cik.clone(),
        rel_type: "filed_by".to_string(),
        weight: 1.0,
        properties: HashMap::new(),
    });

    DataRecord {
        id: filing.accession_number.clone(),
        source: "edgar".to_string(),
        record_type: format!("{:?}", filing.filing_type).to_lowercase(),
        timestamp: filing.filed_date.and_hms_opt(0, 0, 0)
            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
            .unwrap_or_else(Utc::now),
        data: serde_json::to_value(&filing).unwrap_or_default(),
        embedding: None,
        relationships,
    }
}

/// Fundamental vs Narrative analyzer
///
/// Detects divergence between quantitative financial data
/// and qualitative narrative in filings.
pub struct FundamentalNarrativeAnalyzer {
    /// Configuration
    config: AnalyzerConfig,
}

/// Analyzer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerConfig {
    /// Weight for fundamental metrics
    pub fundamental_weight: f64,

    /// Weight for narrative sentiment
    pub narrative_weight: f64,

    /// Minimum divergence to flag
    pub divergence_threshold: f64,

    /// Lookback periods
    pub lookback_periods: usize,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            fundamental_weight: 0.6,
            narrative_weight: 0.4,
            divergence_threshold: 0.3,
            lookback_periods: 4,
        }
    }
}

impl FundamentalNarrativeAnalyzer {
    /// Create a new analyzer
    pub fn new(config: AnalyzerConfig) -> Self {
        Self { config }
    }

    /// Analyze a company for fundamental vs narrative divergence
    pub fn analyze(&self, company: &Company, filings: &[Filing]) -> Option<DivergenceResult> {
        if filings.len() < 2 {
            return None;
        }

        // Extract fundamental changes
        let fundamental_trend = self.compute_fundamental_trend(filings);

        // Extract narrative sentiment changes
        let narrative_trend = self.compute_narrative_trend(filings);

        // Detect divergence
        let divergence = (fundamental_trend - narrative_trend).abs();

        if divergence > self.config.divergence_threshold {
            Some(DivergenceResult {
                company_cik: company.cik.clone(),
                company_name: company.name.clone(),
                fundamental_trend,
                narrative_trend,
                divergence_score: divergence,
                interpretation: self.interpret_divergence(fundamental_trend, narrative_trend),
            })
        } else {
            None
        }
    }

    /// Compute fundamental trend
    fn compute_fundamental_trend(&self, filings: &[Filing]) -> f64 {
        // Simplified: would compute from actual XBRL data
        // Positive = improving financials, negative = declining
        0.0
    }

    /// Compute narrative sentiment trend
    fn compute_narrative_trend(&self, filings: &[Filing]) -> f64 {
        // Simplified: would analyze text sentiment
        // Positive = optimistic narrative, negative = pessimistic
        0.0
    }

    /// Interpret the divergence
    fn interpret_divergence(&self, fundamental: f64, narrative: f64) -> String {
        if fundamental > 0.0 && narrative < 0.0 {
            "Fundamentals improving but narrative pessimistic - potential undervaluation".to_string()
        } else if fundamental < 0.0 && narrative > 0.0 {
            "Fundamentals declining but narrative optimistic - potential risk".to_string()
        } else if fundamental > narrative {
            "Narrative lagging behind fundamental improvement".to_string()
        } else {
            "Narrative ahead of fundamental reality".to_string()
        }
    }
}

/// Result of divergence analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivergenceResult {
    /// Company CIK
    pub company_cik: String,

    /// Company name
    pub company_name: String,

    /// Fundamental trend (-1 to 1)
    pub fundamental_trend: f64,

    /// Narrative trend (-1 to 1)
    pub narrative_trend: f64,

    /// Divergence score (0 to 2)
    pub divergence_score: f64,

    /// Human-readable interpretation
    pub interpretation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sector_from_sic() {
        assert_eq!(Sector::from_sic("7370"), Sector::Technology);
        assert_eq!(Sector::from_sic("6000"), Sector::Financials);
    }

    #[test]
    fn test_default_config() {
        let config = EdgarConfig::default();
        assert_eq!(config.rate_limit, 10);
    }

    #[test]
    fn test_financial_ratio_compute() {
        let mut data = HashMap::new();
        data.insert("Assets Current".to_string(), 100.0);
        data.insert("Liabilities Current".to_string(), 50.0);

        let ratio = FinancialRatio::CurrentRatio.compute(&data);
        assert!(ratio.is_some());
        assert!((ratio.unwrap() - 2.0).abs() < 0.001);
    }
}
