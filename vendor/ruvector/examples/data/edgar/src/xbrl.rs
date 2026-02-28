//! XBRL parsing for financial statement extraction

use std::collections::HashMap;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::EdgarError;

/// XBRL parser
pub struct XbrlParser {
    /// Configuration
    config: ParserConfig,
}

/// Parser configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserConfig {
    /// Include all numeric facts
    pub include_all_facts: bool,

    /// Fact name filters (regex patterns)
    pub fact_filters: Vec<String>,

    /// Merge duplicate contexts
    pub merge_contexts: bool,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            include_all_facts: false,
            fact_filters: vec![
                "Revenue".to_string(),
                "NetIncome".to_string(),
                "Assets".to_string(),
                "Liabilities".to_string(),
                "StockholdersEquity".to_string(),
            ],
            merge_contexts: true,
        }
    }
}

/// Parsed financial statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialStatement {
    /// Company CIK
    pub cik: String,

    /// Filing accession number
    pub accession: String,

    /// Report type (10-K, 10-Q)
    pub report_type: String,

    /// Period end date
    pub period_end: NaiveDate,

    /// Is annual (vs quarterly)
    pub is_annual: bool,

    /// Balance sheet items
    pub balance_sheet: HashMap<String, f64>,

    /// Income statement items
    pub income_statement: HashMap<String, f64>,

    /// Cash flow items
    pub cash_flow: HashMap<String, f64>,

    /// All facts
    pub all_facts: Vec<XbrlFact>,

    /// Contexts
    pub contexts: Vec<XbrlContext>,
}

/// An XBRL fact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XbrlFact {
    /// Concept name
    pub name: String,

    /// Value
    pub value: f64,

    /// Unit
    pub unit: String,

    /// Context reference
    pub context_ref: String,

    /// Decimals precision
    pub decimals: Option<i32>,

    /// Is negated
    pub is_negated: bool,
}

/// An XBRL context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XbrlContext {
    /// Context ID
    pub id: String,

    /// Start date
    pub start_date: Option<NaiveDate>,

    /// End date / instant
    pub end_date: NaiveDate,

    /// Is instant (vs duration)
    pub is_instant: bool,

    /// Segment/scenario dimensions
    pub dimensions: HashMap<String, String>,
}

impl XbrlParser {
    /// Create a new parser
    pub fn new(config: ParserConfig) -> Self {
        Self { config }
    }

    /// Parse XBRL document from string
    pub fn parse(&self, content: &str, cik: &str, accession: &str) -> Result<FinancialStatement, EdgarError> {
        // This is a simplified parser
        // Real implementation would use quick-xml or similar

        let contexts = self.parse_contexts(content)?;
        let facts = self.parse_facts(content)?;

        // Determine period end and type
        let (period_end, is_annual) = self.determine_period(&contexts)?;

        // Categorize facts
        let mut balance_sheet = HashMap::new();
        let mut income_statement = HashMap::new();
        let mut cash_flow = HashMap::new();

        for fact in &facts {
            if self.is_balance_sheet_item(&fact.name) {
                balance_sheet.insert(fact.name.clone(), fact.value);
            } else if self.is_income_statement_item(&fact.name) {
                income_statement.insert(fact.name.clone(), fact.value);
            } else if self.is_cash_flow_item(&fact.name) {
                cash_flow.insert(fact.name.clone(), fact.value);
            }
        }

        Ok(FinancialStatement {
            cik: cik.to_string(),
            accession: accession.to_string(),
            report_type: if is_annual { "10-K".to_string() } else { "10-Q".to_string() },
            period_end,
            is_annual,
            balance_sheet,
            income_statement,
            cash_flow,
            all_facts: facts,
            contexts,
        })
    }

    /// Parse contexts from XBRL
    fn parse_contexts(&self, content: &str) -> Result<Vec<XbrlContext>, EdgarError> {
        // Simplified - would use proper XML parsing
        let mut contexts = Vec::new();

        // Add placeholder context
        contexts.push(XbrlContext {
            id: "FY2023".to_string(),
            start_date: Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            end_date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
            is_instant: false,
            dimensions: HashMap::new(),
        });

        Ok(contexts)
    }

    /// Parse facts from XBRL
    fn parse_facts(&self, content: &str) -> Result<Vec<XbrlFact>, EdgarError> {
        // Simplified - would use proper XML parsing
        let mut facts = Vec::new();

        // Extract numeric values using simple pattern matching
        // Real implementation would parse XML properly

        Ok(facts)
    }

    /// Determine period end and whether annual
    fn determine_period(&self, contexts: &[XbrlContext]) -> Result<(NaiveDate, bool), EdgarError> {
        // Find the main reporting context
        for ctx in contexts {
            if !ctx.is_instant {
                let duration_days = ctx.start_date
                    .map(|s| (ctx.end_date - s).num_days())
                    .unwrap_or(0);

                let is_annual = duration_days > 300;
                return Ok((ctx.end_date, is_annual));
            }
        }

        // Default to latest instant context
        if let Some(ctx) = contexts.last() {
            return Ok((ctx.end_date, true));
        }

        Err(EdgarError::XbrlParse("No valid context found".to_string()))
    }

    /// Check if concept is balance sheet item
    fn is_balance_sheet_item(&self, name: &str) -> bool {
        let balance_sheet_patterns = [
            "Assets",
            "Liabilities",
            "Equity",
            "Cash",
            "Inventory",
            "Receivable",
            "Payable",
            "Debt",
            "Property",
            "Goodwill",
        ];

        balance_sheet_patterns.iter().any(|p| name.contains(p))
    }

    /// Check if concept is income statement item
    fn is_income_statement_item(&self, name: &str) -> bool {
        let income_patterns = [
            "Revenue",
            "Sales",
            "Cost",
            "Expense",
            "Income",
            "Profit",
            "Loss",
            "Earnings",
            "EBITDA",
            "Margin",
        ];

        income_patterns.iter().any(|p| name.contains(p))
    }

    /// Check if concept is cash flow item
    fn is_cash_flow_item(&self, name: &str) -> bool {
        let cash_flow_patterns = [
            "CashFlow",
            "Operating",
            "Investing",
            "Financing",
            "Depreciation",
            "Amortization",
            "CapitalExpenditure",
        ];

        cash_flow_patterns.iter().any(|p| name.contains(p))
    }
}

/// Convert financial statement to vector embedding
pub fn statement_to_embedding(statement: &FinancialStatement) -> Vec<f32> {
    let mut embedding = Vec::with_capacity(64);

    // Balance sheet ratios
    let total_assets = statement.balance_sheet.get("Assets").copied().unwrap_or(1.0);
    let total_liabilities = statement.balance_sheet.get("Liabilities").copied().unwrap_or(0.0);
    let equity = statement.balance_sheet.get("StockholdersEquity").copied().unwrap_or(1.0);
    let cash = statement.balance_sheet.get("Cash").copied().unwrap_or(0.0);

    embedding.push((total_liabilities / total_assets) as f32); // Debt ratio
    embedding.push((cash / total_assets) as f32); // Cash ratio
    embedding.push((equity / total_assets) as f32); // Equity ratio

    // Income statement ratios
    let revenue = statement.income_statement.get("Revenue").copied().unwrap_or(1.0);
    let net_income = statement.income_statement.get("NetIncome").copied().unwrap_or(0.0);
    let operating_income = statement.income_statement.get("OperatingIncome").copied().unwrap_or(0.0);

    embedding.push((net_income / revenue) as f32); // Net margin
    embedding.push((operating_income / revenue) as f32); // Operating margin
    embedding.push((net_income / equity) as f32); // ROE
    embedding.push((net_income / total_assets) as f32); // ROA

    // Pad to fixed size
    while embedding.len() < 64 {
        embedding.push(0.0);
    }

    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut embedding {
            *x /= norm;
        }
    }

    embedding
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let config = ParserConfig::default();
        let parser = XbrlParser::new(config);
        assert!(!parser.config.include_all_facts);
    }

    #[test]
    fn test_balance_sheet_detection() {
        let config = ParserConfig::default();
        let parser = XbrlParser::new(config);

        assert!(parser.is_balance_sheet_item("TotalAssets"));
        assert!(parser.is_balance_sheet_item("CashAndCashEquivalents"));
        assert!(!parser.is_balance_sheet_item("Revenue"));
    }

    #[test]
    fn test_income_statement_detection() {
        let config = ParserConfig::default();
        let parser = XbrlParser::new(config);

        assert!(parser.is_income_statement_item("Revenue"));
        assert!(parser.is_income_statement_item("NetIncome"));
        assert!(!parser.is_income_statement_item("TotalAssets"));
    }
}
