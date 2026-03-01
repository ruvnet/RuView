//! OpenAlex API client

use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::Deserialize;

use crate::{OpenAlexError, Work};

/// OpenAlex API client
pub struct OpenAlexClient {
    client: Client,
    base_url: String,
    email: Option<String>,
}

/// API response wrapper
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    /// Metadata
    pub meta: ApiMeta,

    /// Results
    pub results: Vec<T>,
}

/// API metadata
#[derive(Debug, Deserialize)]
pub struct ApiMeta {
    /// Total count
    pub count: u64,

    /// Current page
    pub page: Option<u32>,

    /// Results per page
    pub per_page: Option<u32>,

    /// Next cursor (for cursor-based pagination)
    pub next_cursor: Option<String>,
}

impl OpenAlexClient {
    /// Create a new OpenAlex client
    ///
    /// Providing an email enables the "polite pool" with higher rate limits.
    pub fn new(email: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("RuVector/0.1.0")
            .gzip(true)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            base_url: "https://api.openalex.org".to_string(),
            email,
        }
    }

    /// Set custom base URL (for testing)
    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }

    /// Build URL with email parameter
    fn build_url(&self, endpoint: &str, params: &str) -> String {
        let mut url = format!("{}/{}?{}", self.base_url, endpoint, params);

        if let Some(ref email) = self.email {
            if !params.is_empty() {
                url.push('&');
            }
            url.push_str(&format!("mailto={}", email));
        }

        url
    }

    /// Health check - verify API is accessible
    pub async fn health_check(&self) -> Result<bool, OpenAlexError> {
        let url = format!("{}/works?per_page=1", self.base_url);
        let response = self.client.get(&url).send().await?;
        Ok(response.status().is_success())
    }

    /// Fetch a page of works with pagination
    pub async fn fetch_works_page(
        &self,
        filter: &str,
        cursor: Option<String>,
        per_page: usize,
    ) -> Result<(Vec<Work>, Option<String>), OpenAlexError> {
        let mut params = format!("per_page={}", per_page);

        if !filter.is_empty() {
            params.push_str(&format!("&{}", filter));
        }

        if let Some(c) = cursor {
            params.push_str(&format!("&cursor={}", c));
        } else {
            // Use cursor-based pagination for bulk
            params.push_str("&cursor=*");
        }

        let url = self.build_url("works", &params);
        let response = self.client.get(&url).send().await?;

        match response.status() {
            StatusCode::OK => {
                let api_response: ApiResponse<Work> = response.json().await?;
                Ok((api_response.results, api_response.meta.next_cursor))
            }
            StatusCode::TOO_MANY_REQUESTS => {
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(60);
                Err(OpenAlexError::RateLimited(retry_after))
            }
            status => Err(OpenAlexError::Api(format!(
                "Unexpected status: {}",
                status
            ))),
        }
    }

    /// Fetch a single work by ID
    pub async fn get_work(&self, id: &str) -> Result<Work, OpenAlexError> {
        // Normalize ID format
        let normalized_id = if id.starts_with("https://") {
            id.to_string()
        } else if id.starts_with("W") {
            format!("https://openalex.org/{}", id)
        } else {
            return Err(OpenAlexError::InvalidId(id.to_string()));
        };

        let url = self.build_url(&format!("works/{}", normalized_id), "");
        let response = self.client.get(&url).send().await?;

        match response.status() {
            StatusCode::OK => Ok(response.json().await?),
            StatusCode::NOT_FOUND => Err(OpenAlexError::InvalidId(id.to_string())),
            status => Err(OpenAlexError::Api(format!(
                "Unexpected status: {}",
                status
            ))),
        }
    }

    /// Search works by query
    pub async fn search_works(
        &self,
        query: &str,
        per_page: usize,
    ) -> Result<Vec<Work>, OpenAlexError> {
        let params = format!("search={}&per_page={}", urlencoding::encode(query), per_page);
        let url = self.build_url("works", &params);
        let response = self.client.get(&url).send().await?;

        match response.status() {
            StatusCode::OK => {
                let api_response: ApiResponse<Work> = response.json().await?;
                Ok(api_response.results)
            }
            status => Err(OpenAlexError::Api(format!(
                "Unexpected status: {}",
                status
            ))),
        }
    }

    /// Fetch works by topic
    pub async fn works_by_topic(
        &self,
        topic_id: &str,
        per_page: usize,
    ) -> Result<Vec<Work>, OpenAlexError> {
        let filter = format!("filter=primary_topic.id:{}", topic_id);
        let (works, _) = self.fetch_works_page(&filter, None, per_page).await?;
        Ok(works)
    }

    /// Fetch works by author
    pub async fn works_by_author(
        &self,
        author_id: &str,
        per_page: usize,
    ) -> Result<Vec<Work>, OpenAlexError> {
        let filter = format!("filter=authorships.author.id:{}", author_id);
        let (works, _) = self.fetch_works_page(&filter, None, per_page).await?;
        Ok(works)
    }

    /// Fetch works by institution
    pub async fn works_by_institution(
        &self,
        institution_id: &str,
        per_page: usize,
    ) -> Result<Vec<Work>, OpenAlexError> {
        let filter = format!(
            "filter=authorships.institutions.id:{}",
            institution_id
        );
        let (works, _) = self.fetch_works_page(&filter, None, per_page).await?;
        Ok(works)
    }

    /// Fetch works citing a specific work
    pub async fn citing_works(
        &self,
        work_id: &str,
        per_page: usize,
    ) -> Result<Vec<Work>, OpenAlexError> {
        let filter = format!("filter=cites:{}", work_id);
        let (works, _) = self.fetch_works_page(&filter, None, per_page).await?;
        Ok(works)
    }

    /// Fetch works cited by a specific work
    pub async fn cited_by_work(&self, work_id: &str) -> Result<Vec<Work>, OpenAlexError> {
        let work = self.get_work(work_id).await?;

        // Fetch referenced works
        let mut cited_works = Vec::new();
        for ref_id in work.referenced_works.iter().take(100) {
            // Limit to avoid too many requests
            if let Ok(cited) = self.get_work(ref_id).await {
                cited_works.push(cited);
            }
        }

        Ok(cited_works)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OpenAlexClient::new(None);
        assert_eq!(client.base_url, "https://api.openalex.org");
    }

    #[test]
    fn test_client_with_email() {
        let client = OpenAlexClient::new(Some("test@example.com".to_string()));
        let url = client.build_url("works", "per_page=10");
        assert!(url.contains("mailto=test@example.com"));
    }

    #[test]
    fn test_url_building() {
        let client = OpenAlexClient::new(None);
        let url = client.build_url("works", "filter=publication_year:2023");
        assert!(url.starts_with("https://api.openalex.org/works"));
        assert!(url.contains("filter=publication_year:2023"));
    }
}
