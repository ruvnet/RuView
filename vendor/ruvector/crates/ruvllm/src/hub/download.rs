//! Model download functionality with progress tracking and resume support

use super::progress::{ProgressBar, ProgressStyle};
use super::registry::ModelInfo;
use super::{default_cache_dir, get_hf_token, HubError, Result};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

// ============================================================================
// Security: URL and Input Validation (H-001)
// ============================================================================

/// Allowed domains for HuggingFace downloads
const ALLOWED_DOMAINS: &[&str] = &["huggingface.co", "hf.co", "cdn-lfs.huggingface.co"];

/// Validate URL is from allowed HuggingFace domains
fn validate_url(url: &str) -> Result<()> {
    // Parse the URL to extract the host
    let url_lower = url.to_lowercase();

    // Check for valid HTTPS scheme
    if !url_lower.starts_with("https://") {
        return Err(HubError::InvalidFormat(
            "Only HTTPS URLs are allowed for downloads".to_string(),
        ));
    }

    // Extract host from URL
    let without_scheme = &url[8..]; // Skip "https://"
    let host_end = without_scheme.find('/').unwrap_or(without_scheme.len());
    let host = &without_scheme[..host_end];

    // Remove port if present
    let host = host.split(':').next().unwrap_or(host);

    // Check against allowlist
    let is_allowed = ALLOWED_DOMAINS
        .iter()
        .any(|&domain| host == domain || host.ends_with(&format!(".{}", domain)));

    if !is_allowed {
        return Err(HubError::InvalidFormat(format!(
            "URL host '{}' is not in the allowed domains: {:?}",
            host, ALLOWED_DOMAINS
        )));
    }

    Ok(())
}

/// Validate repo_id format (prevents CLI injection)
/// Only allows: alphanumeric, /, -, _, .
fn validate_repo_id(repo_id: &str) -> Result<()> {
    // Must contain exactly one slash (user/repo format)
    let slash_count = repo_id.chars().filter(|&c| c == '/').count();
    if slash_count != 1 {
        return Err(HubError::InvalidFormat(
            "Repository ID must be in format 'username/repo-name'".to_string(),
        ));
    }

    // Regex: only allow safe characters
    let valid_pattern = Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9._-]*/[a-zA-Z0-9][a-zA-Z0-9._-]*$")
        .expect("Invalid regex pattern");

    if !valid_pattern.is_match(repo_id) {
        return Err(HubError::InvalidFormat(format!(
            "Repository ID '{}' contains invalid characters. Only alphanumeric, /, -, _, . are allowed",
            repo_id
        )));
    }

    // Prevent path traversal
    if repo_id.contains("..") {
        return Err(HubError::InvalidFormat(
            "Repository ID cannot contain '..' (path traversal)".to_string(),
        ));
    }

    Ok(())
}

/// Canonicalize and validate file path to prevent path traversal
fn validate_and_canonicalize_path(path: &Path, base_dir: &Path) -> Result<PathBuf> {
    // Canonicalize both paths
    let canonical_base = base_dir
        .canonicalize()
        .map_err(|e| HubError::Config(format!("Failed to canonicalize base directory: {}", e)))?;

    // Create parent directories if needed, then canonicalize
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // For new files, canonicalize the parent and append filename
    let canonical_path = if path.exists() {
        path.canonicalize()
            .map_err(|e| HubError::Config(format!("Failed to canonicalize path: {}", e)))?
    } else if let Some(parent) = path.parent() {
        let canonical_parent = parent
            .canonicalize()
            .map_err(|e| HubError::Config(format!("Failed to canonicalize parent path: {}", e)))?;
        canonical_parent.join(
            path.file_name()
                .ok_or_else(|| HubError::InvalidFormat("Invalid file path".to_string()))?,
        )
    } else {
        return Err(HubError::InvalidFormat("Invalid file path".to_string()));
    };

    // Ensure the path is within the base directory
    if !canonical_path.starts_with(&canonical_base) {
        return Err(HubError::InvalidFormat(format!(
            "Path '{}' is outside allowed directory '{}'",
            canonical_path.display(),
            canonical_base.display()
        )));
    }

    Ok(canonical_path)
}

/// Download configuration
#[derive(Debug, Clone)]
pub struct DownloadConfig {
    /// Target directory for downloads
    pub cache_dir: PathBuf,
    /// HuggingFace token for authentication
    pub hf_token: Option<String>,
    /// Enable resume for interrupted downloads
    pub resume: bool,
    /// Show progress bar
    pub show_progress: bool,
    /// Verify checksum after download
    pub verify_checksum: bool,
    /// Maximum retry attempts
    pub max_retries: u32,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            cache_dir: default_cache_dir(),
            hf_token: get_hf_token(),
            resume: true,
            show_progress: true,
            verify_checksum: true,
            max_retries: 3,
        }
    }
}

/// Download progress information
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    /// Total bytes to download
    pub total_bytes: u64,
    /// Bytes downloaded so far
    pub downloaded_bytes: u64,
    /// Download speed in bytes/sec
    pub speed_bps: f64,
    /// Estimated time remaining in seconds
    pub eta_seconds: f64,
    /// Current stage
    pub stage: DownloadStage,
}

/// Download stages
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStage {
    /// Preparing download
    Preparing,
    /// Downloading file
    Downloading,
    /// Verifying checksum
    Verifying,
    /// Complete
    Complete,
    /// Failed
    Failed(String),
}

impl DownloadProgress {
    /// Calculate progress percentage
    pub fn percentage(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.downloaded_bytes as f64 / self.total_bytes as f64 * 100.0) as f32
        }
    }

    /// Format speed as human-readable string
    pub fn speed_str(&self) -> String {
        format_bytes_per_sec(self.speed_bps)
    }

    /// Format ETA as human-readable string
    pub fn eta_str(&self) -> String {
        format_duration(self.eta_seconds as u64)
    }
}

/// Checksum verifier
pub struct ChecksumVerifier {
    hasher: Sha256,
    bytes_hashed: u64,
}

impl ChecksumVerifier {
    /// Create a new checksum verifier
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
            bytes_hashed: 0,
        }
    }

    /// Update with new data
    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
        self.bytes_hashed += data.len() as u64;
    }

    /// Finalize and get checksum
    pub fn finalize(self) -> String {
        format!("{:x}", self.hasher.finalize())
    }

    /// Verify against expected checksum
    pub fn verify(self, expected: &str) -> Result<()> {
        let actual = self.finalize();
        if actual == expected {
            Ok(())
        } else {
            Err(HubError::ChecksumMismatch {
                expected: expected.to_string(),
                actual,
            })
        }
    }
}

impl Default for ChecksumVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Model downloader
pub struct ModelDownloader {
    config: DownloadConfig,
}

impl ModelDownloader {
    /// Create a new downloader with default config
    pub fn new() -> Self {
        Self {
            config: DownloadConfig::default(),
        }
    }

    /// Create a downloader with custom config
    pub fn with_config(config: DownloadConfig) -> Self {
        Self { config }
    }

    /// Download a model by ID from the registry
    pub fn download_by_id(&self, model_id: &str) -> Result<PathBuf> {
        let registry = super::registry::RuvLtraRegistry::new();
        let model_info = registry
            .get(model_id)
            .ok_or_else(|| HubError::NotFound(model_id.to_string()))?;

        self.download(model_info, None)
    }

    /// Download a model from ModelInfo
    pub fn download(&self, model_info: &ModelInfo, target_path: Option<&Path>) -> Result<PathBuf> {
        // Determine target path
        let path = if let Some(p) = target_path {
            p.to_path_buf()
        } else {
            self.config.cache_dir.join(&model_info.filename)
        };

        // SECURITY: Validate and canonicalize path to prevent path traversal
        let path = validate_and_canonicalize_path(&path, &self.config.cache_dir)?;

        // Check if already downloaded
        if path.exists() && !self.config.resume {
            if self.config.verify_checksum {
                if let Some(checksum) = &model_info.checksum {
                    self.verify_file(&path, checksum)?;
                }
            }
            return Ok(path);
        }

        // Download the file
        let url = model_info.download_url();

        // SECURITY: Validate URL is from allowed domains
        validate_url(&url)?;

        self.download_file(
            &url,
            &path,
            model_info.size_bytes,
            model_info.checksum.as_deref(),
        )?;

        Ok(path)
    }

    /// Download a file from URL
    fn download_file(
        &self,
        url: &str,
        path: &Path,
        expected_size: u64,
        expected_checksum: Option<&str>,
    ) -> Result<()> {
        // Use curl/wget if available, otherwise fail with helpful message
        if self.has_curl() {
            self.download_with_curl(url, path, expected_size, expected_checksum)
        } else if self.has_wget() {
            self.download_with_wget(url, path, expected_size, expected_checksum)
        } else {
            Err(HubError::Config(
                "Download requires curl or wget. Please install: brew install curl (macOS) or apt install curl (Linux)"
                    .to_string(),
            ))
        }
    }

    /// Check if curl is available
    fn has_curl(&self) -> bool {
        std::process::Command::new("which")
            .arg("curl")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if wget is available
    fn has_wget(&self) -> bool {
        std::process::Command::new("which")
            .arg("wget")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Download using curl
    fn download_with_curl(
        &self,
        url: &str,
        path: &Path,
        _expected_size: u64,
        expected_checksum: Option<&str>,
    ) -> Result<()> {
        let mut args = vec![
            "-L".to_string(),     // Follow redirects
            "-#".to_string(),     // Progress bar
            "--fail".to_string(), // Fail on HTTP errors
        ];

        // Add resume flag if enabled
        if self.config.resume && path.exists() {
            args.push("-C".to_string());
            args.push("-".to_string()); // Auto-resume
        }

        // Add auth token if provided
        if let Some(token) = &self.config.hf_token {
            args.push("-H".to_string());
            args.push(format!("Authorization: Bearer {}", token));
        }

        args.push("-o".to_string());
        args.push(path.to_str().unwrap().to_string());
        args.push(url.to_string());

        let status = std::process::Command::new("curl")
            .args(&args)
            .status()
            .map_err(|e| HubError::Network(e.to_string()))?;

        if !status.success() {
            return Err(HubError::Network(format!(
                "curl failed with status: {}",
                status
            )));
        }

        // Verify checksum if provided
        if self.config.verify_checksum {
            if let Some(checksum) = expected_checksum {
                self.verify_file(path, checksum)?;
            }
        }

        Ok(())
    }

    /// Download using wget
    fn download_with_wget(
        &self,
        url: &str,
        path: &Path,
        _expected_size: u64,
        expected_checksum: Option<&str>,
    ) -> Result<()> {
        let mut args = vec![
            "-q".to_string(),              // Quiet
            "--show-progress".to_string(), // But show progress
        ];

        // Add resume flag if enabled
        if self.config.resume && path.exists() {
            args.push("-c".to_string()); // Continue
        }

        // Add auth token if provided
        if let Some(token) = &self.config.hf_token {
            args.push("--header".to_string());
            args.push(format!("Authorization: Bearer {}", token));
        }

        args.push("-O".to_string());
        args.push(path.to_str().unwrap().to_string());
        args.push(url.to_string());

        let status = std::process::Command::new("wget")
            .args(&args)
            .status()
            .map_err(|e| HubError::Network(e.to_string()))?;

        if !status.success() {
            return Err(HubError::Network(format!(
                "wget failed with status: {}",
                status
            )));
        }

        // Verify checksum if provided
        if self.config.verify_checksum {
            if let Some(checksum) = expected_checksum {
                self.verify_file(path, checksum)?;
            }
        }

        Ok(())
    }

    /// Verify file checksum
    fn verify_file(&self, path: &Path, expected_checksum: &str) -> Result<()> {
        use std::io::Read;

        let mut file = File::open(path)?;
        let mut verifier = ChecksumVerifier::new();
        let mut buffer = [0u8; 8192];

        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            verifier.update(&buffer[..n]);
        }

        verifier.verify(expected_checksum)
    }
}

impl Default for ModelDownloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Download error type
#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(String),
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    /// Checksum mismatch
    #[error("Checksum verification failed")]
    ChecksumMismatch,
}

/// Format bytes per second
fn format_bytes_per_sec(bps: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bps >= GB {
        format!("{:.2} GB/s", bps / GB)
    } else if bps >= MB {
        format!("{:.2} MB/s", bps / MB)
    } else if bps >= KB {
        format!("{:.2} KB/s", bps / KB)
    } else {
        format!("{:.0} B/s", bps)
    }
}

/// Format duration in seconds
fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_config_default() {
        let config = DownloadConfig::default();
        assert!(config.resume);
        assert!(config.show_progress);
        assert!(config.verify_checksum);
    }

    #[test]
    fn test_download_progress() {
        let progress = DownloadProgress {
            total_bytes: 1000,
            downloaded_bytes: 500,
            speed_bps: 1024.0 * 1024.0,
            eta_seconds: 30.0,
            stage: DownloadStage::Downloading,
        };

        assert_eq!(progress.percentage(), 50.0);
        assert!(progress.speed_str().contains("MB/s"));
    }

    #[test]
    fn test_checksum_verifier() {
        let mut verifier = ChecksumVerifier::new();
        verifier.update(b"hello world");
        let checksum = verifier.finalize();
        assert!(!checksum.is_empty());
        assert_eq!(checksum.len(), 64); // SHA256 hex is 64 chars
    }

    #[test]
    fn test_format_bytes_per_sec() {
        assert_eq!(format_bytes_per_sec(500.0), "500 B/s");
        assert_eq!(format_bytes_per_sec(1024.0 * 10.0), "10.00 KB/s");
        assert_eq!(format_bytes_per_sec(1024.0 * 1024.0 * 5.0), "5.00 MB/s");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m 30s");
        assert_eq!(format_duration(3700), "1h 1m");
    }
}
