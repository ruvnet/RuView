//! SEC EDGAR API client

use std::time::Duration;

use chrono::NaiveDate;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use crate::{Company, EdgarError, Filing, FilingType, Sector};

/// SEC EDGAR API client
pub struct EdgarClient {
    client: Client,
    base_url: String,
    bulk_url: String,
}

/// Company tickers response
#[derive(Debug, Deserialize)]
struct CompanyTickersResponse {
    #[serde(flatten)]
    companies: std::collections::HashMap<String, CompanyEntry>,
}

/// Company entry
#[derive(Debug, Deserialize)]
struct CompanyEntry {
    cik_str: String,
    ticker: String,
    title: String,
}

/// Company facts response
#[derive(Debug, Deserialize)]
struct CompanyFactsResponse {
    cik: u64,
    #[serde(rename = "entityName")]
    entity_name: String,
    facts: Option<Facts>,
}

/// XBRL facts
#[derive(Debug, Deserialize)]
struct Facts {
    #[serde(rename = "us-gaap")]
    us_gaap: Option<std::collections::HashMap<String, Concept>>,
}

/// XBRL concept
#[derive(Debug, Deserialize)]
struct Concept {
    label: String,
    description: Option<String>,
    units: std::collections::HashMap<String, Vec<UnitValue>>,
}

/// Unit value
#[derive(Debug, Deserialize)]
struct UnitValue {
    #[serde(rename = "end")]
    end_date: String,
    val: f64,
    accn: String,
    fy: Option<i32>,
    fp: Option<String>,
    form: String,
    filed: String,
}

/// Submissions response
#[derive(Debug, Deserialize)]
struct SubmissionsResponse {
    cik: String,
    name: String,
    sic: Option<String>,
    #[serde(rename = "sicDescription")]
    sic_description: Option<String>,
    #[serde(rename = "stateOfIncorporation")]
    state: Option<String>,
    #[serde(rename = "fiscalYearEnd")]
    fiscal_year_end: Option<String>,
    filings: FilingsData,
}

/// Filings data
#[derive(Debug, Deserialize)]
struct FilingsData {
    recent: RecentFilings,
}

/// Recent filings
#[derive(Debug, Deserialize)]
struct RecentFilings {
    #[serde(rename = "accessionNumber")]
    accession_numbers: Vec<String>,
    #[serde(rename = "filingDate")]
    filing_dates: Vec<String>,
    form: Vec<String>,
    #[serde(rename = "primaryDocument")]
    primary_documents: Vec<String>,
    #[serde(rename = "primaryDocDescription")]
    descriptions: Vec<String>,
}

impl EdgarClient {
    /// Create a new EDGAR client
    ///
    /// SEC requires user agent with company/contact info
    pub fn new(user_agent: &str, company: &str, email: &str) -> Self {
        let full_agent = format!("{} ({}, {})", user_agent, company, email);

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(full_agent)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            base_url: "https://data.sec.gov".to_string(),
            bulk_url: "https://www.sec.gov/cgi-bin/browse-edgar".to_string(),
        }
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool, EdgarError> {
        let url = format!("{}/submissions/CIK0000320193.json", self.base_url);
        let response = self.client.get(&url).send().await?;
        Ok(response.status().is_success())
    }

    /// Convert ticker to CIK
    pub async fn ticker_to_cik(&self, ticker: &str) -> Result<String, EdgarError> {
        let url = format!("{}/files/company_tickers.json", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(EdgarError::Api("Failed to fetch company tickers".to_string()));
        }

        let data: CompanyTickersResponse = response.json().await?;

        for entry in data.companies.values() {
            if entry.ticker.eq_ignore_ascii_case(ticker) {
                return Ok(entry.cik_str.clone());
            }
        }

        Err(EdgarError::InvalidCik(format!("Ticker not found: {}", ticker)))
    }

    /// Get company info
    pub async fn get_company(&self, cik: &str) -> Result<Company, EdgarError> {
        let padded_cik = format!("{:0>10}", cik.trim_start_matches('0'));
        let url = format!("{}/submissions/CIK{}.json", self.base_url, padded_cik);

        let response = self.client.get(&url).send().await?;

        match response.status() {
            StatusCode::OK => {
                let data: SubmissionsResponse = response.json().await?;

                Ok(Company {
                    cik: data.cik,
                    name: data.name,
                    ticker: None, // Would need to look up
                    sic_code: data.sic,
                    sic_description: data.sic_description,
                    state: data.state,
                    fiscal_year_end: data.fiscal_year_end,
                    latest_filing: data.filings.recent.filing_dates.first()
                        .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()),
                })
            }
            StatusCode::NOT_FOUND => Err(EdgarError::InvalidCik(cik.to_string())),
            status => Err(EdgarError::Api(format!("Unexpected status: {}", status))),
        }
    }

    /// Get filings for a company
    pub async fn get_filings(
        &self,
        cik: &str,
        filing_types: &[FilingType],
    ) -> Result<Vec<Filing>, EdgarError> {
        let padded_cik = format!("{:0>10}", cik.trim_start_matches('0'));
        let url = format!("{}/submissions/CIK{}.json", self.base_url, padded_cik);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(EdgarError::Api(format!(
                "Failed to fetch submissions: {}",
                response.status()
            )));
        }

        let data: SubmissionsResponse = response.json().await?;

        let mut filings = Vec::new();

        for i in 0..data.filings.recent.accession_numbers.len() {
            let form = &data.filings.recent.form[i];
            let filing_type = FilingType::from_form(form);

            if filing_types.contains(&filing_type) {
                let filed_date = NaiveDate::parse_from_str(
                    &data.filings.recent.filing_dates[i],
                    "%Y-%m-%d",
                )
                .unwrap_or(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());

                filings.push(Filing {
                    accession_number: data.filings.recent.accession_numbers[i].clone(),
                    cik: cik.to_string(),
                    filing_type,
                    filed_date,
                    document_url: format!(
                        "https://www.sec.gov/Archives/edgar/data/{}/{}/{}",
                        cik,
                        data.filings.recent.accession_numbers[i].replace("-", ""),
                        data.filings.recent.primary_documents[i]
                    ),
                    description: data.filings.recent.descriptions.get(i).cloned(),
                });
            }
        }

        Ok(filings)
    }

    /// Get company facts (XBRL financial data)
    pub async fn get_company_facts(&self, cik: &str) -> Result<CompanyFactsResponse, EdgarError> {
        let padded_cik = format!("{:0>10}", cik.trim_start_matches('0'));
        let url = format!(
            "{}/api/xbrl/companyfacts/CIK{}.json",
            self.base_url, padded_cik
        );

        let response = self.client.get(&url).send().await?;

        match response.status() {
            StatusCode::OK => Ok(response.json().await?),
            StatusCode::NOT_FOUND => Err(EdgarError::InvalidCik(cik.to_string())),
            status => Err(EdgarError::Api(format!("Unexpected status: {}", status))),
        }
    }

    /// Get companies by sector
    pub async fn get_companies_by_sector(&self, sector: &Sector) -> Result<Vec<Company>, EdgarError> {
        // Note: This is a simplified implementation
        // Real implementation would use bulk data or SIC code search
        let sic_prefix = match sector {
            Sector::Technology => "73",
            Sector::Healthcare => "80",
            Sector::Financials => "60",
            Sector::ConsumerDiscretionary => "57",
            Sector::ConsumerStaples => "20",
            Sector::Energy => "13",
            Sector::Materials => "28",
            Sector::Industrials => "35",
            Sector::Utilities => "49",
            Sector::RealEstate => "65",
            Sector::CommunicationServices => "48",
            Sector::Other => "99",
        };

        // Return placeholder - would implement full sector search
        Ok(vec![])
    }

    /// Get XBRL financial statement data
    pub async fn get_financial_data(
        &self,
        cik: &str,
        metrics: &[&str],
    ) -> Result<std::collections::HashMap<String, Vec<(NaiveDate, f64)>>, EdgarError> {
        let facts = self.get_company_facts(cik).await?;

        let mut result = std::collections::HashMap::new();

        if let Some(facts) = facts.facts {
            if let Some(us_gaap) = facts.us_gaap {
                for metric in metrics {
                    if let Some(concept) = us_gaap.get(*metric) {
                        let mut values = Vec::new();

                        for (_, unit_values) in &concept.units {
                            for uv in unit_values {
                                if let Ok(date) = NaiveDate::parse_from_str(&uv.end_date, "%Y-%m-%d") {
                                    values.push((date, uv.val));
                                }
                            }
                        }

                        values.sort_by_key(|(d, _)| *d);
                        result.insert(metric.to_string(), values);
                    }
                }
            }
        }

        Ok(result)
    }

    /// Download filing document
    pub async fn download_filing(&self, url: &str) -> Result<String, EdgarError> {
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(EdgarError::FilingNotFound(url.to_string()));
        }

        Ok(response.text().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = EdgarClient::new("TestAgent/1.0", "Test Corp", "test@example.com");
        assert!(client.base_url.contains("data.sec.gov"));
    }
}
