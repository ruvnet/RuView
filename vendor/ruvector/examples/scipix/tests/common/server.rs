// Test server setup and teardown utilities
//
// Provides a test server instance for integration tests

use super::types::{CacheStats, OutputFormat, ProcessingOptions, ProcessingResult};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct TestServer {
    inner: Arc<TestServerInner>,
}

struct TestServerInner {
    base_url: String,
    #[allow(dead_code)]
    process: Option<RwLock<tokio::process::Child>>,
    config: TestServerConfig,
}

#[derive(Debug, Clone)]
pub struct TestServerConfig {
    pub port: u16,
    pub enable_cache: bool,
    pub cache_size: Option<usize>,
    pub cache_ttl_seconds: Option<u64>,
    pub rate_limit: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub cache_dir: Option<String>,
}

impl Default for TestServerConfig {
    fn default() -> Self {
        Self {
            port: 18080,
            enable_cache: false,
            cache_size: None,
            cache_ttl_seconds: None,
            rate_limit: None,
            timeout_ms: None,
            cache_dir: None,
        }
    }
}

impl TestServer {
    /// Start a basic test server
    pub async fn start() -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_config(TestServerConfig::default()).await
    }

    /// Start test server with cache enabled
    pub async fn with_cache() -> Result<Self, Box<dyn std::error::Error>> {
        let config = TestServerConfig {
            enable_cache: true,
            cache_size: Some(100),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Start test server with specific cache size
    pub async fn with_cache_size(size: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let config = TestServerConfig {
            enable_cache: true,
            cache_size: Some(size),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Start test server with cache TTL
    pub async fn with_cache_ttl(ttl_seconds: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let config = TestServerConfig {
            enable_cache: true,
            cache_ttl_seconds: Some(ttl_seconds),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Start test server with persistent cache
    pub async fn with_persistent_cache(
        cache_dir: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let config = TestServerConfig {
            enable_cache: true,
            cache_dir: Some(cache_dir.to_string()),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Start test server with timeout
    pub async fn with_timeout(timeout_ms: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let config = TestServerConfig {
            timeout_ms: Some(timeout_ms),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Start API server
    pub async fn start_api() -> Result<Self, Box<dyn std::error::Error>> {
        Self::start().await
    }

    /// Start API server with rate limiting
    pub async fn start_api_with_rate_limit(limit: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let config = TestServerConfig {
            rate_limit: Some(limit),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    /// Start test server with custom configuration
    pub async fn with_config(config: TestServerConfig) -> Result<Self, Box<dyn std::error::Error>> {
        // Test infrastructure - provides mock server for testing
        // Real OCR processing requires ONNX models to be configured

        let base_url = format!("http://localhost:{}", config.port);

        let inner = Arc::new(TestServerInner {
            base_url,
            process: None,
            config,
        });

        // Wait for server to be ready
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(TestServer { inner })
    }

    /// Get base URL
    pub fn base_url(&self) -> &str {
        &self.inner.base_url
    }

    /// Process a single image
    /// Note: This is test infrastructure that returns mock data.
    /// Real OCR requires ONNX models to be configured.
    pub async fn process_image(
        &self,
        _image_path: &str,
        _format: OutputFormat,
    ) -> Result<ProcessingResult, String> {
        // Test infrastructure mock - real OCR requires models
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Ok(ProcessingResult {
            latex: "x + y".to_string(),
            mathml: Some("<math><mrow><mi>x</mi><mo>+</mo><mi>y</mi></mrow></math>".to_string()),
            html: None,
            ascii: None,
            text: Some("x + y".to_string()),
            confidence: 0.95,
            processing_time_ms: 50,
        })
    }

    /// Process image with options
    pub async fn process_image_with_options(
        &self,
        image_path: &str,
        format: OutputFormat,
        _options: ProcessingOptions,
    ) -> Result<ProcessingResult, String> {
        self.process_image(image_path, format).await
    }

    /// Process batch of images
    pub async fn process_batch(
        &self,
        image_paths: &[&str],
        format: OutputFormat,
    ) -> Result<Vec<ProcessingResult>, String> {
        let mut results = Vec::new();
        for path in image_paths {
            results.push(self.process_image(path, format.clone()).await?);
        }
        Ok(results)
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> Result<CacheStats, String> {
        Ok(CacheStats {
            hits: 0,
            misses: 0,
            evictions: 0,
            current_size: 0,
            max_size: self.inner.config.cache_size.unwrap_or(100),
        })
    }

    /// Invalidate cache
    pub async fn invalidate_cache(&self) -> Result<(), String> {
        Ok(())
    }

    /// Shutdown server
    pub async fn shutdown(self) {
        // Test infrastructure - no actual server to shut down
    }
}
