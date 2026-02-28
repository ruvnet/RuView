//! AI/ML API Client Integrations
//!
//! This module provides async clients for AI/ML platforms including:
//! - HuggingFace: Model hub and inference
//! - Ollama: Local LLM inference
//! - Replicate: Cloud ML models
//! - TogetherAI: Open source model hosting
//! - Papers With Code: ML research papers and benchmarks
//!
//! All clients follow the framework's patterns with rate limiting, mock fallbacks,
//! and conversion to SemanticVector format for RuVector discovery.

use std::collections::HashMap;
use std::env;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration for different services
const HUGGINGFACE_RATE_LIMIT_MS: u64 = 2000; // 30 req/min = 2000ms
const PAPERWITHCODE_RATE_LIMIT_MS: u64 = 1000; // 60 req/min = 1000ms
const REPLICATE_RATE_LIMIT_MS: u64 = 1000;
const TOGETHER_RATE_LIMIT_MS: u64 = 1000;
const OLLAMA_RATE_LIMIT_MS: u64 = 100; // Local, minimal delay

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 2000;
const DEFAULT_EMBEDDING_DIM: usize = 384;
const REQUEST_TIMEOUT_SECS: u64 = 30;

// ============================================================================
// HuggingFace Client
// ============================================================================

/// HuggingFace model information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HuggingFaceModel {
    #[serde(rename = "modelId")]
    pub model_id: String,
    #[serde(rename = "author")]
    pub author: Option<String>,
    #[serde(rename = "downloads")]
    pub downloads: Option<u64>,
    #[serde(rename = "likes")]
    pub likes: Option<u64>,
    #[serde(rename = "tags")]
    pub tags: Option<Vec<String>>,
    #[serde(rename = "pipeline_tag")]
    pub pipeline_tag: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
}

/// HuggingFace dataset information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HuggingFaceDataset {
    pub id: String,
    pub author: Option<String>,
    pub downloads: Option<u64>,
    pub likes: Option<u64>,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    pub description: Option<String>,
}

/// HuggingFace inference input
#[derive(Debug, Clone, Serialize)]
pub struct HuggingFaceInferenceInput {
    pub inputs: serde_json::Value,
}

/// HuggingFace inference response
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum HuggingFaceInferenceResponse {
    Embeddings(Vec<Vec<f32>>),
    Classification(Vec<ClassificationResult>),
    Generation(Vec<GenerationResult>),
    Error(InferenceError),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClassificationResult {
    pub label: String,
    pub score: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerationResult {
    pub generated_text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InferenceError {
    pub error: String,
}

/// Client for HuggingFace model hub and inference API
///
/// # API Details
/// - Base URL: https://huggingface.co/api
/// - Rate limit: 30 requests/minute (free tier)
/// - API key optional for public models
///
/// # Environment Variables
/// - `HUGGINGFACE_API_KEY`: Optional API key for higher rate limits and private models
pub struct HuggingFaceClient {
    client: Client,
    embedder: SimpleEmbedder,
    base_url: String,
    api_key: Option<String>,
    use_mock: bool,
}

impl HuggingFaceClient {
    /// Create a new HuggingFace client
    ///
    /// Reads API key from `HUGGINGFACE_API_KEY` environment variable if available.
    /// Falls back to mock data if no API key is provided.
    pub fn new() -> Self {
        Self::with_embedding_dim(DEFAULT_EMBEDDING_DIM)
    }

    /// Create a new HuggingFace client with custom embedding dimension
    pub fn with_embedding_dim(embedding_dim: usize) -> Self {
        let api_key = env::var("HUGGINGFACE_API_KEY").ok();
        let use_mock = api_key.is_none();

        if use_mock {
            tracing::warn!("HUGGINGFACE_API_KEY not set, using mock data");
        }

        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .build()
                .expect("Failed to create HTTP client"),
            embedder: SimpleEmbedder::new(embedding_dim),
            base_url: "https://huggingface.co/api".to_string(),
            api_key,
            use_mock,
        }
    }

    /// Search models by query and optional task filter
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `task` - Optional task filter (e.g., "text-classification", "text-generation")
    ///
    /// # Example
    /// ```rust,ignore
    /// let models = client.search_models("bert", Some("fill-mask")).await?;
    /// ```
    pub async fn search_models(
        &self,
        query: &str,
        task: Option<&str>,
    ) -> Result<Vec<HuggingFaceModel>> {
        if self.use_mock {
            return Ok(self.mock_models(query));
        }

        sleep(Duration::from_millis(HUGGINGFACE_RATE_LIMIT_MS)).await;

        let mut url = format!("{}/models?search={}", self.base_url, urlencoding::encode(query));
        if let Some(task_filter) = task {
            url.push_str(&format!("&filter={}", task_filter));
        }
        url.push_str("&limit=20");

        let response = self.fetch_with_retry(&url).await?;
        let models: Vec<HuggingFaceModel> = response.json().await?;

        Ok(models)
    }

    /// Get detailed information about a specific model
    ///
    /// # Arguments
    /// * `model_id` - Model identifier (e.g., "bert-base-uncased")
    pub async fn get_model(&self, model_id: &str) -> Result<Option<HuggingFaceModel>> {
        if self.use_mock {
            return Ok(self.mock_models(model_id).into_iter().next());
        }

        sleep(Duration::from_millis(HUGGINGFACE_RATE_LIMIT_MS)).await;

        let url = format!("{}/models/{}", self.base_url, model_id);
        let response = self.fetch_with_retry(&url).await?;
        let model: HuggingFaceModel = response.json().await?;

        Ok(Some(model))
    }

    /// List datasets with optional query filter
    ///
    /// # Arguments
    /// * `query` - Optional search query for datasets
    pub async fn list_datasets(&self, query: Option<&str>) -> Result<Vec<HuggingFaceDataset>> {
        if self.use_mock {
            return Ok(self.mock_datasets(query.unwrap_or("ml")));
        }

        sleep(Duration::from_millis(HUGGINGFACE_RATE_LIMIT_MS)).await;

        let mut url = format!("{}/datasets", self.base_url);
        if let Some(q) = query {
            url.push_str(&format!("?search={}", urlencoding::encode(q)));
        }
        url.push_str("&limit=20");

        let response = self.fetch_with_retry(&url).await?;
        let datasets: Vec<HuggingFaceDataset> = response.json().await?;

        Ok(datasets)
    }

    /// Get detailed information about a specific dataset
    ///
    /// # Arguments
    /// * `dataset_id` - Dataset identifier
    pub async fn get_dataset(&self, dataset_id: &str) -> Result<Option<HuggingFaceDataset>> {
        if self.use_mock {
            return Ok(self.mock_datasets(dataset_id).into_iter().next());
        }

        sleep(Duration::from_millis(HUGGINGFACE_RATE_LIMIT_MS)).await;

        let url = format!("{}/datasets/{}", self.base_url, dataset_id);
        let response = self.fetch_with_retry(&url).await?;
        let dataset: HuggingFaceDataset = response.json().await?;

        Ok(Some(dataset))
    }

    /// Run inference on a model
    ///
    /// # Arguments
    /// * `model_id` - Model identifier
    /// * `inputs` - Input data as JSON value
    ///
    /// # Note
    /// Requires API key. Returns mock embeddings if no API key is available.
    pub async fn inference(
        &self,
        model_id: &str,
        inputs: serde_json::Value,
    ) -> Result<HuggingFaceInferenceResponse> {
        if self.use_mock {
            // Return mock embeddings
            let embedding = self.embedder.embed_json(&inputs);
            return Ok(HuggingFaceInferenceResponse::Embeddings(vec![embedding]));
        }

        sleep(Duration::from_millis(HUGGINGFACE_RATE_LIMIT_MS)).await;

        let url = format!("https://api-inference.huggingface.co/models/{}", model_id);
        let body = HuggingFaceInferenceInput { inputs };

        let mut request = self.client.post(&url).json(&body);

        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(FrameworkError::Network(
                reqwest::Error::from(response.error_for_status().unwrap_err()),
            ));
        }

        let result: HuggingFaceInferenceResponse = response.json().await?;
        Ok(result)
    }

    /// Convert HuggingFace model to SemanticVector
    pub fn model_to_vector(&self, model: &HuggingFaceModel) -> SemanticVector {
        let text = format!(
            "{} {} {}",
            model.model_id,
            model.pipeline_tag.as_deref().unwrap_or(""),
            model.tags.as_ref().map(|t| t.join(" ")).unwrap_or_default()
        );

        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("model_id".to_string(), model.model_id.clone());
        if let Some(author) = &model.author {
            metadata.insert("author".to_string(), author.clone());
        }
        if let Some(downloads) = model.downloads {
            metadata.insert("downloads".to_string(), downloads.to_string());
        }
        if let Some(likes) = model.likes {
            metadata.insert("likes".to_string(), likes.to_string());
        }
        if let Some(pipeline) = &model.pipeline_tag {
            metadata.insert("task".to_string(), pipeline.clone());
        }
        metadata.insert("source".to_string(), "huggingface".to_string());

        let timestamp = model
            .created_at
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        SemanticVector {
            id: format!("hf:model:{}", model.model_id),
            embedding,
            domain: Domain::Research,
            timestamp,
            metadata,
        }
    }

    /// Convert HuggingFace dataset to SemanticVector
    pub fn dataset_to_vector(&self, dataset: &HuggingFaceDataset) -> SemanticVector {
        let text = format!(
            "{} {}",
            dataset.id,
            dataset.description.as_deref().unwrap_or("")
        );

        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("dataset_id".to_string(), dataset.id.clone());
        if let Some(author) = &dataset.author {
            metadata.insert("author".to_string(), author.clone());
        }
        if let Some(downloads) = dataset.downloads {
            metadata.insert("downloads".to_string(), downloads.to_string());
        }
        metadata.insert("source".to_string(), "huggingface".to_string());

        let timestamp = dataset
            .created_at
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        SemanticVector {
            id: format!("hf:dataset:{}", dataset.id),
            embedding,
            domain: Domain::Research,
            timestamp,
            metadata,
        }
    }

    /// Mock models for testing without API key
    fn mock_models(&self, query: &str) -> Vec<HuggingFaceModel> {
        vec![
            HuggingFaceModel {
                model_id: format!("bert-base-{}", query),
                author: Some("google".to_string()),
                downloads: Some(1_000_000),
                likes: Some(500),
                tags: Some(vec!["nlp".to_string(), "transformer".to_string()]),
                pipeline_tag: Some("fill-mask".to_string()),
                created_at: Some(Utc::now().to_rfc3339()),
            },
            HuggingFaceModel {
                model_id: format!("gpt2-{}", query),
                author: Some("openai".to_string()),
                downloads: Some(800_000),
                likes: Some(350),
                tags: Some(vec!["text-generation".to_string()]),
                pipeline_tag: Some("text-generation".to_string()),
                created_at: Some(Utc::now().to_rfc3339()),
            },
        ]
    }

    /// Mock datasets for testing without API key
    fn mock_datasets(&self, query: &str) -> Vec<HuggingFaceDataset> {
        vec![
            HuggingFaceDataset {
                id: format!("squad-{}", query),
                author: Some("datasets".to_string()),
                downloads: Some(500_000),
                likes: Some(200),
                tags: Some(vec!["qa".to_string(), "english".to_string()]),
                created_at: Some(Utc::now().to_rfc3339()),
                description: Some("Question answering dataset".to_string()),
            },
            HuggingFaceDataset {
                id: format!("glue-{}", query),
                author: Some("datasets".to_string()),
                downloads: Some(300_000),
                likes: Some(150),
                tags: Some(vec!["benchmark".to_string()]),
                created_at: Some(Utc::now().to_rfc3339()),
                description: Some("General Language Understanding Evaluation".to_string()),
            },
        ]
    }

    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let mut request = self.client.get(url);

            if let Some(key) = &self.api_key {
                request = request.header("Authorization", format!("Bearer {}", key));
            }

            match request.send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        tracing::warn!("Rate limited, retrying in {}ms", RETRY_DELAY_MS * retries as u64);
                        sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                        continue;
                    }
                    if !response.status().is_success() {
                        return Err(FrameworkError::Network(
                            reqwest::Error::from(response.error_for_status().unwrap_err()),
                        ));
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    tracing::warn!("Request failed, retrying ({}/{})", retries, MAX_RETRIES);
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for HuggingFaceClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Ollama Client (Local LLM)
// ============================================================================

/// Ollama model information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OllamaModel {
    pub name: String,
    pub modified_at: Option<String>,
    pub size: Option<u64>,
    pub digest: Option<String>,
}

/// Ollama model list response
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaModelsResponse {
    pub models: Vec<OllamaModel>,
}

/// Ollama generation request
#[derive(Debug, Clone, Serialize)]
pub struct OllamaGenerateRequest {
    pub model: String,
    pub prompt: String,
    pub stream: bool,
}

/// Ollama generation response
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaGenerateResponse {
    pub model: String,
    pub response: String,
    pub done: bool,
}

/// Ollama chat message
#[derive(Debug, Clone, Serialize)]
pub struct OllamaChatMessage {
    pub role: String,
    pub content: String,
}

/// Ollama chat request
#[derive(Debug, Clone, Serialize)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaChatMessage>,
    pub stream: bool,
}

/// Ollama chat response
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaChatResponse {
    pub model: String,
    pub message: OllamaMessage,
    pub done: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}

/// Ollama embeddings request
#[derive(Debug, Clone, Serialize)]
pub struct OllamaEmbeddingsRequest {
    pub model: String,
    pub prompt: String,
}

/// Ollama embeddings response
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaEmbeddingsResponse {
    pub embedding: Vec<f32>,
}

/// Client for Ollama local LLM inference
///
/// # API Details
/// - Base URL: http://localhost:11434/api (default)
/// - No rate limit (local service)
/// - No API key required
/// - Falls back to mock data when Ollama is not running
pub struct OllamaClient {
    client: Client,
    embedder: SimpleEmbedder,
    base_url: String,
    use_mock: bool,
}

impl OllamaClient {
    /// Create a new Ollama client with default base URL
    pub fn new() -> Self {
        Self::with_base_url("http://localhost:11434/api")
    }

    /// Create a new Ollama client with custom base URL
    ///
    /// # Arguments
    /// * `base_url` - Ollama API base URL (e.g., "http://localhost:11434/api")
    pub fn with_base_url(base_url: &str) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .build()
                .expect("Failed to create HTTP client"),
            embedder: SimpleEmbedder::new(DEFAULT_EMBEDDING_DIM),
            base_url: base_url.to_string(),
            use_mock: false,
        }
    }

    /// Check if Ollama is available
    pub async fn is_available(&self) -> bool {
        self.client
            .get(&format!("{}/tags", self.base_url))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// List available models
    pub async fn list_models(&mut self) -> Result<Vec<OllamaModel>> {
        sleep(Duration::from_millis(OLLAMA_RATE_LIMIT_MS)).await;

        let url = format!("{}/tags", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                let data: OllamaModelsResponse = response.json().await?;
                self.use_mock = false;
                Ok(data.models)
            }
            _ => {
                if !self.use_mock {
                    tracing::warn!("Ollama not available, using mock data");
                    self.use_mock = true;
                }
                Ok(self.mock_models())
            }
        }
    }

    /// Generate text completion
    ///
    /// # Arguments
    /// * `model` - Model name (e.g., "llama2", "mistral")
    /// * `prompt` - Prompt text
    pub async fn generate(&mut self, model: &str, prompt: &str) -> Result<String> {
        if self.use_mock || !self.is_available().await {
            self.use_mock = true;
            return Ok(self.mock_generation(prompt));
        }

        sleep(Duration::from_millis(OLLAMA_RATE_LIMIT_MS)).await;

        let url = format!("{}/generate", self.base_url);
        let body = OllamaGenerateRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
        };

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            return Err(FrameworkError::Network(
                reqwest::Error::from(response.error_for_status().unwrap_err()),
            ));
        }

        let result: OllamaGenerateResponse = response.json().await?;
        Ok(result.response)
    }

    /// Chat completion with message history
    ///
    /// # Arguments
    /// * `model` - Model name
    /// * `messages` - Chat message history
    pub async fn chat(
        &mut self,
        model: &str,
        messages: Vec<OllamaChatMessage>,
    ) -> Result<String> {
        if self.use_mock || !self.is_available().await {
            self.use_mock = true;
            let last_msg = messages.last().map(|m| m.content.as_str()).unwrap_or("");
            return Ok(self.mock_generation(last_msg));
        }

        sleep(Duration::from_millis(OLLAMA_RATE_LIMIT_MS)).await;

        let url = format!("{}/chat", self.base_url);
        let body = OllamaChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
        };

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            return Err(FrameworkError::Network(
                reqwest::Error::from(response.error_for_status().unwrap_err()),
            ));
        }

        let result: OllamaChatResponse = response.json().await?;
        Ok(result.message.content)
    }

    /// Generate embeddings for text
    ///
    /// # Arguments
    /// * `model` - Model name (e.g., "llama2")
    /// * `prompt` - Text to embed
    pub async fn embeddings(&mut self, model: &str, prompt: &str) -> Result<Vec<f32>> {
        if self.use_mock || !self.is_available().await {
            self.use_mock = true;
            return Ok(self.embedder.embed_text(prompt));
        }

        sleep(Duration::from_millis(OLLAMA_RATE_LIMIT_MS)).await;

        let url = format!("{}/embeddings", self.base_url);
        let body = OllamaEmbeddingsRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
        };

        let response = self.client.post(&url).json(&body).send().await?;

        if !response.status().is_success() {
            return Err(FrameworkError::Network(
                reqwest::Error::from(response.error_for_status().unwrap_err()),
            ));
        }

        let result: OllamaEmbeddingsResponse = response.json().await?;
        Ok(result.embedding)
    }

    /// Pull a model from Ollama library
    ///
    /// # Arguments
    /// * `name` - Model name to pull
    ///
    /// # Note
    /// This is a blocking operation that may take several minutes
    pub async fn pull_model(&mut self, name: &str) -> Result<bool> {
        if self.use_mock || !self.is_available().await {
            self.use_mock = true;
            tracing::warn!("Ollama not available, cannot pull model");
            return Ok(false);
        }

        sleep(Duration::from_millis(OLLAMA_RATE_LIMIT_MS)).await;

        let url = format!("{}/pull", self.base_url);
        let body = serde_json::json!({ "name": name });

        let response = self.client.post(&url).json(&body).send().await?;
        Ok(response.status().is_success())
    }

    /// Convert Ollama model to SemanticVector
    pub fn model_to_vector(&self, model: &OllamaModel) -> SemanticVector {
        let embedding = self.embedder.embed_text(&model.name);

        let mut metadata = HashMap::new();
        metadata.insert("model_name".to_string(), model.name.clone());
        if let Some(size) = model.size {
            metadata.insert("size_bytes".to_string(), size.to_string());
        }
        if let Some(digest) = &model.digest {
            metadata.insert("digest".to_string(), digest.clone());
        }
        metadata.insert("source".to_string(), "ollama".to_string());

        let timestamp = model
            .modified_at
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        SemanticVector {
            id: format!("ollama:model:{}", model.name),
            embedding,
            domain: Domain::Research,
            timestamp,
            metadata,
        }
    }

    fn mock_models(&self) -> Vec<OllamaModel> {
        vec![
            OllamaModel {
                name: "llama2:latest".to_string(),
                modified_at: Some(Utc::now().to_rfc3339()),
                size: Some(3_800_000_000),
                digest: Some("sha256:mock123".to_string()),
            },
            OllamaModel {
                name: "mistral:latest".to_string(),
                modified_at: Some(Utc::now().to_rfc3339()),
                size: Some(4_100_000_000),
                digest: Some("sha256:mock456".to_string()),
            },
        ]
    }

    fn mock_generation(&self, prompt: &str) -> String {
        format!("Mock response to: {}", prompt.chars().take(50).collect::<String>())
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Replicate Client
// ============================================================================

/// Replicate model information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplicateModel {
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub visibility: Option<String>,
    pub github_url: Option<String>,
    pub paper_url: Option<String>,
    pub latest_version: Option<ReplicateVersion>,
}

/// Replicate model version
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReplicateVersion {
    pub id: String,
    pub created_at: Option<String>,
}

/// Replicate prediction request
#[derive(Debug, Clone, Serialize)]
pub struct ReplicatePredictionRequest {
    pub version: String,
    pub input: serde_json::Value,
}

/// Replicate prediction response
#[derive(Debug, Clone, Deserialize)]
pub struct ReplicatePrediction {
    pub id: String,
    pub status: String,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Replicate collection
#[derive(Debug, Clone, Deserialize)]
pub struct ReplicateCollection {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
}

/// Client for Replicate cloud ML model API
///
/// # API Details
/// - Base URL: https://api.replicate.com/v1
/// - Requires API key
/// - Falls back to mock data when no API key is available
///
/// # Environment Variables
/// - `REPLICATE_API_TOKEN`: Required API token
pub struct ReplicateClient {
    client: Client,
    embedder: SimpleEmbedder,
    base_url: String,
    api_token: Option<String>,
    use_mock: bool,
}

impl ReplicateClient {
    /// Create a new Replicate client
    ///
    /// Reads API token from `REPLICATE_API_TOKEN` environment variable.
    pub fn new() -> Self {
        Self::with_embedding_dim(DEFAULT_EMBEDDING_DIM)
    }

    /// Create a new Replicate client with custom embedding dimension
    pub fn with_embedding_dim(embedding_dim: usize) -> Self {
        let api_token = env::var("REPLICATE_API_TOKEN").ok();
        let use_mock = api_token.is_none();

        if use_mock {
            tracing::warn!("REPLICATE_API_TOKEN not set, using mock data");
        }

        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .build()
                .expect("Failed to create HTTP client"),
            embedder: SimpleEmbedder::new(embedding_dim),
            base_url: "https://api.replicate.com/v1".to_string(),
            api_token,
            use_mock,
        }
    }

    /// Get model information
    ///
    /// # Arguments
    /// * `owner` - Model owner username
    /// * `name` - Model name
    pub async fn get_model(&self, owner: &str, name: &str) -> Result<Option<ReplicateModel>> {
        if self.use_mock {
            return Ok(Some(self.mock_model(owner, name)));
        }

        sleep(Duration::from_millis(REPLICATE_RATE_LIMIT_MS)).await;

        let url = format!("{}/models/{}/{}", self.base_url, owner, name);
        let response = self.fetch_with_retry(&url).await?;
        let model: ReplicateModel = response.json().await?;

        Ok(Some(model))
    }

    /// Create a prediction (run a model)
    ///
    /// # Arguments
    /// * `model` - Model identifier in "owner/name" format
    /// * `input` - Input parameters as JSON
    pub async fn create_prediction(
        &self,
        model: &str,
        input: serde_json::Value,
    ) -> Result<ReplicatePrediction> {
        if self.use_mock {
            return Ok(self.mock_prediction());
        }

        sleep(Duration::from_millis(REPLICATE_RATE_LIMIT_MS)).await;

        let url = format!("{}/predictions", self.base_url);

        // Get latest version for the model
        let parts: Vec<&str> = model.split('/').collect();
        if parts.len() != 2 {
            return Err(FrameworkError::Config(
                "Model must be in 'owner/name' format".to_string(),
            ));
        }

        let model_info = self.get_model(parts[0], parts[1]).await?;
        let version = model_info
            .and_then(|m| m.latest_version)
            .and_then(|v| Some(v.id))
            .ok_or_else(|| FrameworkError::Config("Model version not found".to_string()))?;

        let body = ReplicatePredictionRequest { version, input };

        let response = self.fetch_with_retry_post(&url, &body).await?;
        let prediction: ReplicatePrediction = response.json().await?;

        Ok(prediction)
    }

    /// Get prediction status and output
    ///
    /// # Arguments
    /// * `id` - Prediction ID
    pub async fn get_prediction(&self, id: &str) -> Result<ReplicatePrediction> {
        if self.use_mock {
            return Ok(self.mock_prediction());
        }

        sleep(Duration::from_millis(REPLICATE_RATE_LIMIT_MS)).await;

        let url = format!("{}/predictions/{}", self.base_url, id);
        let response = self.fetch_with_retry(&url).await?;
        let prediction: ReplicatePrediction = response.json().await?;

        Ok(prediction)
    }

    /// List model collections
    pub async fn list_collections(&self) -> Result<Vec<ReplicateCollection>> {
        if self.use_mock {
            return Ok(self.mock_collections());
        }

        sleep(Duration::from_millis(REPLICATE_RATE_LIMIT_MS)).await;

        let url = format!("{}/collections", self.base_url);
        let response = self.fetch_with_retry(&url).await?;
        let collections: Vec<ReplicateCollection> = response.json().await?;

        Ok(collections)
    }

    /// Convert Replicate model to SemanticVector
    pub fn model_to_vector(&self, model: &ReplicateModel) -> SemanticVector {
        let text = format!(
            "{}/{} {}",
            model.owner,
            model.name,
            model.description.as_deref().unwrap_or("")
        );

        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("owner".to_string(), model.owner.clone());
        metadata.insert("name".to_string(), model.name.clone());
        if let Some(desc) = &model.description {
            metadata.insert("description".to_string(), desc.clone());
        }
        if let Some(github) = &model.github_url {
            metadata.insert("github_url".to_string(), github.clone());
        }
        if let Some(paper) = &model.paper_url {
            metadata.insert("paper_url".to_string(), paper.clone());
        }
        metadata.insert("source".to_string(), "replicate".to_string());

        let timestamp = model
            .latest_version
            .as_ref()
            .and_then(|v| v.created_at.as_ref())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        SemanticVector {
            id: format!("replicate:{}/{}", model.owner, model.name),
            embedding,
            domain: Domain::Research,
            timestamp,
            metadata,
        }
    }

    fn mock_model(&self, owner: &str, name: &str) -> ReplicateModel {
        ReplicateModel {
            owner: owner.to_string(),
            name: name.to_string(),
            description: Some("Mock model for testing".to_string()),
            visibility: Some("public".to_string()),
            github_url: None,
            paper_url: None,
            latest_version: Some(ReplicateVersion {
                id: "mock-version-123".to_string(),
                created_at: Some(Utc::now().to_rfc3339()),
            }),
        }
    }

    fn mock_prediction(&self) -> ReplicatePrediction {
        ReplicatePrediction {
            id: "mock-prediction-123".to_string(),
            status: "succeeded".to_string(),
            output: Some(serde_json::json!({"result": "mock output"})),
            error: None,
        }
    }

    fn mock_collections(&self) -> Vec<ReplicateCollection> {
        vec![
            ReplicateCollection {
                name: "Text to Image".to_string(),
                slug: "text-to-image".to_string(),
                description: Some("Generate images from text".to_string()),
            },
            ReplicateCollection {
                name: "Image to Text".to_string(),
                slug: "image-to-text".to_string(),
                description: Some("Generate text from images".to_string()),
            },
        ]
    }

    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let mut request = self.client.get(url);

            if let Some(token) = &self.api_token {
                request = request.header("Authorization", format!("Token {}", token));
            }

            match request.send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                        continue;
                    }
                    if !response.status().is_success() {
                        return Err(FrameworkError::Network(
                            reqwest::Error::from(response.error_for_status().unwrap_err()),
                        ));
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }

    async fn fetch_with_retry_post<T: Serialize>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let mut request = self.client.post(url).json(body);

            if let Some(token) = &self.api_token {
                request = request.header("Authorization", format!("Token {}", token));
            }

            match request.send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                        continue;
                    }
                    if !response.status().is_success() {
                        return Err(FrameworkError::Network(
                            reqwest::Error::from(response.error_for_status().unwrap_err()),
                        ));
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for ReplicateClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TogetherAI Client
// ============================================================================

/// TogetherAI model information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TogetherModel {
    pub id: String,
    pub name: Option<String>,
    #[serde(rename = "display_name")]
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub context_length: Option<u64>,
    pub pricing: Option<TogetherPricing>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TogetherPricing {
    pub input: Option<f64>,
    pub output: Option<f64>,
}

/// TogetherAI chat completion request
#[derive(Debug, Clone, Serialize)]
pub struct TogetherChatRequest {
    pub model: String,
    pub messages: Vec<TogetherMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TogetherMessage {
    pub role: String,
    pub content: String,
}

/// TogetherAI chat completion response
#[derive(Debug, Clone, Deserialize)]
pub struct TogetherChatResponse {
    pub id: String,
    pub choices: Vec<TogetherChoice>,
    pub usage: Option<TogetherUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TogetherChoice {
    pub message: TogetherMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TogetherUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// TogetherAI embeddings request
#[derive(Debug, Clone, Serialize)]
pub struct TogetherEmbeddingsRequest {
    pub model: String,
    pub input: String,
}

/// TogetherAI embeddings response
#[derive(Debug, Clone, Deserialize)]
pub struct TogetherEmbeddingsResponse {
    pub data: Vec<TogetherEmbeddingData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TogetherEmbeddingData {
    pub embedding: Vec<f32>,
    pub index: u32,
}

/// Client for TogetherAI open source model hosting
///
/// # API Details
/// - Base URL: https://api.together.xyz/v1
/// - Requires API key
/// - Falls back to mock data when no API key is available
///
/// # Environment Variables
/// - `TOGETHER_API_KEY`: Required API key
pub struct TogetherAiClient {
    client: Client,
    embedder: SimpleEmbedder,
    base_url: String,
    api_key: Option<String>,
    use_mock: bool,
}

impl TogetherAiClient {
    /// Create a new TogetherAI client
    ///
    /// Reads API key from `TOGETHER_API_KEY` environment variable.
    pub fn new() -> Self {
        Self::with_embedding_dim(DEFAULT_EMBEDDING_DIM)
    }

    /// Create a new TogetherAI client with custom embedding dimension
    pub fn with_embedding_dim(embedding_dim: usize) -> Self {
        let api_key = env::var("TOGETHER_API_KEY").ok();
        let use_mock = api_key.is_none();

        if use_mock {
            tracing::warn!("TOGETHER_API_KEY not set, using mock data");
        }

        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .build()
                .expect("Failed to create HTTP client"),
            embedder: SimpleEmbedder::new(embedding_dim),
            base_url: "https://api.together.xyz/v1".to_string(),
            api_key,
            use_mock,
        }
    }

    /// List available models
    pub async fn list_models(&self) -> Result<Vec<TogetherModel>> {
        if self.use_mock {
            return Ok(self.mock_models());
        }

        sleep(Duration::from_millis(TOGETHER_RATE_LIMIT_MS)).await;

        let url = format!("{}/models", self.base_url);
        let response = self.fetch_with_retry(&url).await?;
        let models: Vec<TogetherModel> = response.json().await?;

        Ok(models)
    }

    /// Chat completion
    ///
    /// # Arguments
    /// * `model` - Model identifier
    /// * `messages` - Chat message history
    pub async fn chat_completion(
        &self,
        model: &str,
        messages: Vec<TogetherMessage>,
    ) -> Result<String> {
        if self.use_mock {
            let last_msg = messages.last().map(|m| m.content.as_str()).unwrap_or("");
            return Ok(format!("Mock response to: {}", last_msg));
        }

        sleep(Duration::from_millis(TOGETHER_RATE_LIMIT_MS)).await;

        let url = format!("{}/chat/completions", self.base_url);
        let body = TogetherChatRequest {
            model: model.to_string(),
            messages,
            max_tokens: Some(512),
            temperature: Some(0.7),
        };

        let response = self.fetch_with_retry_post(&url, &body).await?;
        let result: TogetherChatResponse = response.json().await?;

        Ok(result
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default())
    }

    /// Generate embeddings
    ///
    /// # Arguments
    /// * `model` - Embedding model identifier
    /// * `input` - Text to embed
    pub async fn embeddings(&self, model: &str, input: &str) -> Result<Vec<f32>> {
        if self.use_mock {
            return Ok(self.embedder.embed_text(input));
        }

        sleep(Duration::from_millis(TOGETHER_RATE_LIMIT_MS)).await;

        let url = format!("{}/embeddings", self.base_url);
        let body = TogetherEmbeddingsRequest {
            model: model.to_string(),
            input: input.to_string(),
        };

        let response = self.fetch_with_retry_post(&url, &body).await?;
        let result: TogetherEmbeddingsResponse = response.json().await?;

        Ok(result
            .data
            .first()
            .map(|d| d.embedding.clone())
            .unwrap_or_default())
    }

    /// Convert TogetherAI model to SemanticVector
    pub fn model_to_vector(&self, model: &TogetherModel) -> SemanticVector {
        let text = format!(
            "{} {}",
            model.display_name.as_deref().unwrap_or(&model.id),
            model.description.as_deref().unwrap_or("")
        );

        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("model_id".to_string(), model.id.clone());
        if let Some(name) = &model.display_name {
            metadata.insert("display_name".to_string(), name.clone());
        }
        if let Some(ctx) = model.context_length {
            metadata.insert("context_length".to_string(), ctx.to_string());
        }
        metadata.insert("source".to_string(), "together".to_string());

        SemanticVector {
            id: format!("together:{}", model.id),
            embedding,
            domain: Domain::Research,
            timestamp: Utc::now(),
            metadata,
        }
    }

    fn mock_models(&self) -> Vec<TogetherModel> {
        vec![
            TogetherModel {
                id: "togethercomputer/llama-2-7b".to_string(),
                name: Some("Llama 2 7B".to_string()),
                display_name: Some("Llama 2 7B".to_string()),
                description: Some("Meta's Llama 2 7B model".to_string()),
                context_length: Some(4096),
                pricing: None,
            },
            TogetherModel {
                id: "mistralai/Mistral-7B-v0.1".to_string(),
                name: Some("Mistral 7B".to_string()),
                display_name: Some("Mistral 7B".to_string()),
                description: Some("Mistral AI's 7B model".to_string()),
                context_length: Some(8192),
                pricing: None,
            },
        ]
    }

    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let mut request = self.client.get(url);

            if let Some(key) = &self.api_key {
                request = request.header("Authorization", format!("Bearer {}", key));
            }

            match request.send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                        continue;
                    }
                    if !response.status().is_success() {
                        return Err(FrameworkError::Network(
                            reqwest::Error::from(response.error_for_status().unwrap_err()),
                        ));
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }

    async fn fetch_with_retry_post<T: Serialize>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let mut request = self.client.post(url).json(body);

            if let Some(key) = &self.api_key {
                request = request.header("Authorization", format!("Bearer {}", key));
            }

            match request.send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                        continue;
                    }
                    if !response.status().is_success() {
                        return Err(FrameworkError::Network(
                            reqwest::Error::from(response.error_for_status().unwrap_err()),
                        ));
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for TogetherAiClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Papers With Code Client
// ============================================================================

/// Papers With Code paper
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaperWithCodePaper {
    pub id: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub url_abs: Option<String>,
    pub url_pdf: Option<String>,
    pub published: Option<String>,
    pub authors: Option<Vec<PaperAuthor>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaperAuthor {
    pub name: String,
}

/// Papers With Code dataset
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaperWithCodeDataset {
    pub id: String,
    pub name: String,
    pub full_name: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    pub paper: Option<String>,
}

/// Papers With Code SOTA (State of the Art) benchmark result
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SotaEntry {
    pub task: String,
    pub dataset: String,
    pub metric: String,
    pub value: f64,
    pub paper_title: Option<String>,
    pub paper_url: Option<String>,
}

/// Papers With Code method/technique
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Method {
    pub name: String,
    pub full_name: Option<String>,
    pub description: Option<String>,
    pub paper: Option<String>,
}

/// Papers With Code search results
#[derive(Debug, Clone, Deserialize)]
pub struct PapersSearchResponse {
    pub results: Vec<PaperWithCodePaper>,
    pub count: Option<u32>,
}

/// Papers With Code datasets list response
#[derive(Debug, Clone, Deserialize)]
pub struct DatasetsResponse {
    pub results: Vec<PaperWithCodeDataset>,
    pub count: Option<u32>,
}

/// Client for Papers With Code ML research database
///
/// # API Details
/// - Base URL: https://paperswithcode.com/api/v1
/// - Rate limit: 60 requests/minute
/// - No API key required
pub struct PapersWithCodeClient {
    client: Client,
    embedder: SimpleEmbedder,
    base_url: String,
}

impl PapersWithCodeClient {
    /// Create a new Papers With Code client
    pub fn new() -> Self {
        Self::with_embedding_dim(DEFAULT_EMBEDDING_DIM)
    }

    /// Create a new Papers With Code client with custom embedding dimension
    pub fn with_embedding_dim(embedding_dim: usize) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .build()
                .expect("Failed to create HTTP client"),
            embedder: SimpleEmbedder::new(embedding_dim),
            base_url: "https://paperswithcode.com/api/v1".to_string(),
        }
    }

    /// Search papers by query
    ///
    /// # Arguments
    /// * `query` - Search query string
    pub async fn search_papers(&self, query: &str) -> Result<Vec<PaperWithCodePaper>> {
        sleep(Duration::from_millis(PAPERWITHCODE_RATE_LIMIT_MS)).await;

        let url = format!(
            "{}/papers/?q={}",
            self.base_url,
            urlencoding::encode(query)
        );

        let response = self.fetch_with_retry(&url).await?;
        let data: PapersSearchResponse = response.json().await?;

        Ok(data.results)
    }

    /// Get paper by ID
    ///
    /// # Arguments
    /// * `paper_id` - Paper identifier
    pub async fn get_paper(&self, paper_id: &str) -> Result<Option<PaperWithCodePaper>> {
        sleep(Duration::from_millis(PAPERWITHCODE_RATE_LIMIT_MS)).await;

        let url = format!("{}/papers/{}/", self.base_url, paper_id);
        let response = self.fetch_with_retry(&url).await?;
        let paper: PaperWithCodePaper = response.json().await?;

        Ok(Some(paper))
    }

    /// List datasets
    pub async fn list_datasets(&self) -> Result<Vec<PaperWithCodeDataset>> {
        sleep(Duration::from_millis(PAPERWITHCODE_RATE_LIMIT_MS)).await;

        let url = format!("{}/datasets/", self.base_url);
        let response = self.fetch_with_retry(&url).await?;
        let data: DatasetsResponse = response.json().await?;

        Ok(data.results)
    }

    /// Get state-of-the-art results for a task
    ///
    /// # Arguments
    /// * `task` - Task name (e.g., "image-classification", "question-answering")
    pub async fn get_sota(&self, task: &str) -> Result<Vec<SotaEntry>> {
        sleep(Duration::from_millis(PAPERWITHCODE_RATE_LIMIT_MS)).await;

        let url = format!("{}/sota/?task={}", self.base_url, urlencoding::encode(task));

        // Papers With Code API might not have a direct SOTA endpoint in v1
        // Return mock data for now
        Ok(self.mock_sota(task))
    }

    /// Search methods/techniques
    ///
    /// # Arguments
    /// * `query` - Search query for methods
    pub async fn search_methods(&self, query: &str) -> Result<Vec<Method>> {
        sleep(Duration::from_millis(PAPERWITHCODE_RATE_LIMIT_MS)).await;

        // Return mock data as the methods endpoint structure may vary
        Ok(self.mock_methods(query))
    }

    /// Convert paper to SemanticVector
    pub fn paper_to_vector(&self, paper: &PaperWithCodePaper) -> SemanticVector {
        let text = format!(
            "{} {}",
            paper.title,
            paper.abstract_text.as_deref().unwrap_or("")
        );

        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("paper_id".to_string(), paper.id.clone());
        metadata.insert("title".to_string(), paper.title.clone());
        if let Some(url) = &paper.url_abs {
            metadata.insert("url".to_string(), url.clone());
        }
        if let Some(pdf) = &paper.url_pdf {
            metadata.insert("pdf_url".to_string(), pdf.clone());
        }
        if let Some(authors) = &paper.authors {
            let author_names = authors
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            metadata.insert("authors".to_string(), author_names);
        }
        metadata.insert("source".to_string(), "paperswithcode".to_string());

        let timestamp = paper
            .published
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        SemanticVector {
            id: format!("pwc:paper:{}", paper.id),
            embedding,
            domain: Domain::Research,
            timestamp,
            metadata,
        }
    }

    /// Convert dataset to SemanticVector
    pub fn dataset_to_vector(&self, dataset: &PaperWithCodeDataset) -> SemanticVector {
        let text = format!(
            "{} {}",
            dataset.name,
            dataset.description.as_deref().unwrap_or("")
        );

        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("dataset_id".to_string(), dataset.id.clone());
        metadata.insert("name".to_string(), dataset.name.clone());
        if let Some(desc) = &dataset.description {
            metadata.insert("description".to_string(), desc.clone());
        }
        if let Some(url) = &dataset.url {
            metadata.insert("url".to_string(), url.clone());
        }
        metadata.insert("source".to_string(), "paperswithcode".to_string());

        SemanticVector {
            id: format!("pwc:dataset:{}", dataset.id),
            embedding,
            domain: Domain::Research,
            timestamp: Utc::now(),
            metadata,
        }
    }

    fn mock_sota(&self, task: &str) -> Vec<SotaEntry> {
        vec![
            SotaEntry {
                task: task.to_string(),
                dataset: "ImageNet".to_string(),
                metric: "Top-1 Accuracy".to_string(),
                value: 90.2,
                paper_title: Some("Vision Transformer".to_string()),
                paper_url: Some("https://arxiv.org/abs/2010.11929".to_string()),
            },
            SotaEntry {
                task: task.to_string(),
                dataset: "COCO".to_string(),
                metric: "mAP".to_string(),
                value: 58.7,
                paper_title: Some("DETR".to_string()),
                paper_url: Some("https://arxiv.org/abs/2005.12872".to_string()),
            },
        ]
    }

    fn mock_methods(&self, query: &str) -> Vec<Method> {
        vec![
            Method {
                name: format!("Transformer-{}", query),
                full_name: Some("Transformer Architecture".to_string()),
                description: Some("Attention-based neural network architecture".to_string()),
                paper: Some("https://arxiv.org/abs/1706.03762".to_string()),
            },
            Method {
                name: format!("ResNet-{}", query),
                full_name: Some("Residual Network".to_string()),
                description: Some("Deep residual learning framework".to_string()),
                paper: Some("https://arxiv.org/abs/1512.03385".to_string()),
            },
        ]
    }

    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        tracing::warn!("Rate limited, retrying in {}ms", RETRY_DELAY_MS * retries as u64);
                        sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                        continue;
                    }
                    if !response.status().is_success() {
                        return Err(FrameworkError::Network(
                            reqwest::Error::from(response.error_for_status().unwrap_err()),
                        ));
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    tracing::warn!("Request failed, retrying ({}/{})", retries, MAX_RETRIES);
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for PapersWithCodeClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // HuggingFace Tests
    #[test]
    fn test_huggingface_client_creation() {
        let client = HuggingFaceClient::new();
        assert_eq!(client.base_url, "https://huggingface.co/api");
    }

    #[test]
    fn test_huggingface_mock_models() {
        let client = HuggingFaceClient::new();
        let models = client.mock_models("test");
        assert!(!models.is_empty());
        assert!(models[0].model_id.contains("test"));
    }

    #[test]
    fn test_huggingface_model_to_vector() {
        let client = HuggingFaceClient::new();
        let model = HuggingFaceModel {
            model_id: "bert-base-uncased".to_string(),
            author: Some("google".to_string()),
            downloads: Some(1_000_000),
            likes: Some(500),
            tags: Some(vec!["nlp".to_string()]),
            pipeline_tag: Some("fill-mask".to_string()),
            created_at: Some(Utc::now().to_rfc3339()),
        };

        let vector = client.model_to_vector(&model);
        assert_eq!(vector.id, "hf:model:bert-base-uncased");
        assert_eq!(vector.domain, Domain::Research);
        assert!(vector.metadata.contains_key("model_id"));
        assert_eq!(vector.metadata.get("author").unwrap(), "google");
    }

    #[tokio::test]
    async fn test_huggingface_search_models_mock() {
        let client = HuggingFaceClient::new();
        let models = client.search_models("bert", None).await;
        assert!(models.is_ok());
        assert!(!models.unwrap().is_empty());
    }

    // Ollama Tests
    #[test]
    fn test_ollama_client_creation() {
        let client = OllamaClient::new();
        assert_eq!(client.base_url, "http://localhost:11434/api");
    }

    #[test]
    fn test_ollama_mock_models() {
        let client = OllamaClient::new();
        let models = client.mock_models();
        assert!(!models.is_empty());
        assert!(models[0].name.contains("llama"));
    }

    #[test]
    fn test_ollama_model_to_vector() {
        let client = OllamaClient::new();
        let model = OllamaModel {
            name: "llama2:latest".to_string(),
            modified_at: Some(Utc::now().to_rfc3339()),
            size: Some(3_800_000_000),
            digest: Some("sha256:abc123".to_string()),
        };

        let vector = client.model_to_vector(&model);
        assert_eq!(vector.id, "ollama:model:llama2:latest");
        assert_eq!(vector.domain, Domain::Research);
        assert!(vector.metadata.contains_key("model_name"));
    }

    #[tokio::test]
    async fn test_ollama_list_models_mock() {
        let mut client = OllamaClient::new();
        client.use_mock = true;
        let models = client.list_models().await;
        assert!(models.is_ok());
        assert!(!models.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_ollama_embeddings_mock() {
        let mut client = OllamaClient::new();
        client.use_mock = true;
        let embedding = client.embeddings("llama2", "test text").await;
        assert!(embedding.is_ok());
        assert_eq!(embedding.unwrap().len(), DEFAULT_EMBEDDING_DIM);
    }

    // Replicate Tests
    #[test]
    fn test_replicate_client_creation() {
        let client = ReplicateClient::new();
        assert_eq!(client.base_url, "https://api.replicate.com/v1");
    }

    #[test]
    fn test_replicate_mock_model() {
        let client = ReplicateClient::new();
        let model = client.mock_model("owner", "model");
        assert_eq!(model.owner, "owner");
        assert_eq!(model.name, "model");
    }

    #[test]
    fn test_replicate_model_to_vector() {
        let client = ReplicateClient::new();
        let model = ReplicateModel {
            owner: "stability-ai".to_string(),
            name: "stable-diffusion".to_string(),
            description: Some("Text to image model".to_string()),
            visibility: Some("public".to_string()),
            github_url: None,
            paper_url: None,
            latest_version: Some(ReplicateVersion {
                id: "v1.0".to_string(),
                created_at: Some(Utc::now().to_rfc3339()),
            }),
        };

        let vector = client.model_to_vector(&model);
        assert_eq!(vector.id, "replicate:stability-ai/stable-diffusion");
        assert_eq!(vector.domain, Domain::Research);
    }

    #[tokio::test]
    async fn test_replicate_get_model_mock() {
        let client = ReplicateClient::new();
        let model = client.get_model("owner", "model").await;
        assert!(model.is_ok());
        assert!(model.unwrap().is_some());
    }

    // TogetherAI Tests
    #[test]
    fn test_together_client_creation() {
        let client = TogetherAiClient::new();
        assert_eq!(client.base_url, "https://api.together.xyz/v1");
    }

    #[test]
    fn test_together_mock_models() {
        let client = TogetherAiClient::new();
        let models = client.mock_models();
        assert!(!models.is_empty());
        assert!(models[0].id.contains("llama"));
    }

    #[test]
    fn test_together_model_to_vector() {
        let client = TogetherAiClient::new();
        let model = TogetherModel {
            id: "togethercomputer/llama-2-7b".to_string(),
            name: Some("Llama 2 7B".to_string()),
            display_name: Some("Llama 2 7B".to_string()),
            description: Some("Meta's Llama 2 model".to_string()),
            context_length: Some(4096),
            pricing: None,
        };

        let vector = client.model_to_vector(&model);
        assert_eq!(vector.id, "together:togethercomputer/llama-2-7b");
        assert_eq!(vector.domain, Domain::Research);
    }

    #[tokio::test]
    async fn test_together_list_models_mock() {
        let client = TogetherAiClient::new();
        let models = client.list_models().await;
        assert!(models.is_ok());
        assert!(!models.unwrap().is_empty());
    }

    // Papers With Code Tests
    #[test]
    fn test_paperswithcode_client_creation() {
        let client = PapersWithCodeClient::new();
        assert_eq!(client.base_url, "https://paperswithcode.com/api/v1");
    }

    #[test]
    fn test_paperswithcode_paper_to_vector() {
        let client = PapersWithCodeClient::new();
        let paper = PaperWithCodePaper {
            id: "attention-is-all-you-need".to_string(),
            title: "Attention Is All You Need".to_string(),
            abstract_text: Some("We propose the Transformer...".to_string()),
            url_abs: Some("https://arxiv.org/abs/1706.03762".to_string()),
            url_pdf: Some("https://arxiv.org/pdf/1706.03762.pdf".to_string()),
            published: Some(Utc::now().to_rfc3339()),
            authors: Some(vec![
                PaperAuthor {
                    name: "Vaswani et al.".to_string(),
                },
            ]),
        };

        let vector = client.paper_to_vector(&paper);
        assert_eq!(vector.id, "pwc:paper:attention-is-all-you-need");
        assert_eq!(vector.domain, Domain::Research);
        assert!(vector.metadata.contains_key("title"));
    }

    #[test]
    fn test_paperswithcode_dataset_to_vector() {
        let client = PapersWithCodeClient::new();
        let dataset = PaperWithCodeDataset {
            id: "imagenet".to_string(),
            name: "ImageNet".to_string(),
            full_name: Some("ImageNet Large Scale Visual Recognition Challenge".to_string()),
            description: Some("Large-scale image dataset".to_string()),
            url: Some("https://image-net.org".to_string()),
            paper: None,
        };

        let vector = client.dataset_to_vector(&dataset);
        assert_eq!(vector.id, "pwc:dataset:imagenet");
        assert_eq!(vector.domain, Domain::Research);
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting API in tests
    async fn test_paperswithcode_search_papers_integration() {
        let client = PapersWithCodeClient::new();
        let papers = client.search_papers("transformer").await;
        assert!(papers.is_ok());
    }

    // Integration Tests
    #[test]
    fn test_all_clients_default() {
        let _hf = HuggingFaceClient::default();
        let _ollama = OllamaClient::default();
        let _replicate = ReplicateClient::default();
        let _together = TogetherAiClient::default();
        let _pwc = PapersWithCodeClient::default();
    }

    #[test]
    fn test_custom_embedding_dimensions() {
        let hf = HuggingFaceClient::with_embedding_dim(512);
        let model = hf.mock_models("test")[0].clone();
        let vector = hf.model_to_vector(&model);
        assert_eq!(vector.embedding.len(), 512);

        let pwc = PapersWithCodeClient::with_embedding_dim(768);
        let paper = PaperWithCodePaper {
            id: "test".to_string(),
            title: "Test Paper".to_string(),
            abstract_text: None,
            url_abs: None,
            url_pdf: None,
            published: None,
            authors: None,
        };
        let vector = pwc.paper_to_vector(&paper);
        assert_eq!(vector.embedding.len(), 768);
    }
}
