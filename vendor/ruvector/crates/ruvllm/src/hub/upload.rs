//! Model upload functionality for publishing to HuggingFace Hub

use super::model_card::{ModelCard, ModelCardBuilder};
use super::{get_hf_token, HubError, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

// ============================================================================
// Security: Input Validation (H-002)
// ============================================================================

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

    // Prevent shell metacharacters that could be used for injection
    let dangerous_chars = [
        '`', '$', '(', ')', ';', '&', '|', '<', '>', '\n', '\r', '"', '\'', '\\',
    ];
    for c in dangerous_chars {
        if repo_id.contains(c) {
            return Err(HubError::InvalidFormat(format!(
                "Repository ID cannot contain shell metacharacter '{}'",
                c
            )));
        }
    }

    Ok(())
}

/// Validate file path for upload (prevents path traversal)
fn validate_upload_path(path: &Path) -> Result<()> {
    let path_str = path.to_string_lossy();

    // Prevent path traversal
    if path_str.contains("..") {
        return Err(HubError::InvalidFormat(
            "File path cannot contain '..' (path traversal)".to_string(),
        ));
    }

    // Canonicalize to resolve any symlinks and verify it exists
    let canonical = path.canonicalize().map_err(|e| {
        HubError::NotFound(format!("Cannot resolve path '{}': {}", path.display(), e))
    })?;

    // Verify the file exists and is a regular file
    if !canonical.is_file() {
        return Err(HubError::NotFound(format!(
            "Path '{}' is not a regular file",
            path.display()
        )));
    }

    Ok(())
}

/// Upload configuration
#[derive(Debug, Clone)]
pub struct UploadConfig {
    /// HuggingFace token for authentication (required)
    pub hf_token: String,
    /// Make repository private
    pub private: bool,
    /// Create repository if it doesn't exist
    pub create_repo: bool,
    /// Upload SONA weights separately
    pub include_sona_weights: bool,
    /// Generate model card automatically
    pub auto_model_card: bool,
    /// Commit message
    pub commit_message: String,
}

impl UploadConfig {
    /// Create upload config with token
    pub fn new(hf_token: String) -> Self {
        Self {
            hf_token,
            private: false,
            create_repo: true,
            include_sona_weights: true,
            auto_model_card: true,
            commit_message: "Upload RuvLTRA model".to_string(),
        }
    }

    /// Set repository visibility
    pub fn private(mut self, private: bool) -> Self {
        self.private = private;
        self
    }

    /// Set commit message
    pub fn commit_message(mut self, message: impl Into<String>) -> Self {
        self.commit_message = message.into();
        self
    }
}

/// Upload progress information
#[derive(Debug, Clone)]
pub struct UploadProgress {
    /// Total bytes to upload
    pub total_bytes: u64,
    /// Bytes uploaded so far
    pub uploaded_bytes: u64,
    /// Upload speed in bytes/sec
    pub speed_bps: f64,
    /// Current file being uploaded
    pub current_file: String,
    /// Upload stage
    pub stage: UploadStage,
}

/// Upload stages
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadStage {
    /// Preparing upload
    Preparing,
    /// Creating repository
    CreatingRepo,
    /// Uploading model file
    UploadingModel,
    /// Uploading SONA weights
    UploadingSona,
    /// Uploading model card
    UploadingCard,
    /// Complete
    Complete,
    /// Failed
    Failed(String),
}

/// Model metadata for upload
#[derive(Debug, Clone)]
pub struct ModelMetadata {
    /// Model name
    pub name: String,
    /// Model description
    pub description: Option<String>,
    /// Model architecture
    pub architecture: String,
    /// Number of parameters
    pub params_b: f32,
    /// Context length
    pub context_length: usize,
    /// Quantization type
    pub quantization: Option<String>,
    /// License
    pub license: Option<String>,
    /// Training datasets
    pub datasets: Vec<String>,
    /// Tags for discovery
    pub tags: Vec<String>,
}

/// Model uploader
pub struct ModelUploader {
    config: UploadConfig,
}

impl ModelUploader {
    /// Create a new uploader with HF token
    pub fn new(hf_token: impl Into<String>) -> Self {
        Self {
            config: UploadConfig::new(hf_token.into()),
        }
    }

    /// Create uploader with custom config
    pub fn with_config(config: UploadConfig) -> Self {
        Self { config }
    }

    /// Upload a model file to HuggingFace Hub
    ///
    /// # Arguments
    ///
    /// * `model_path` - Path to the model file (.gguf)
    /// * `repo_id` - HuggingFace repository (e.g., "username/model-name")
    /// * `metadata` - Optional model metadata
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let uploader = ModelUploader::new("hf_token");
    /// uploader.upload(
    ///     "./ruvltra-custom.gguf",
    ///     "username/ruvltra-custom",
    ///     Some(metadata),
    /// )?;
    /// ```
    pub fn upload(
        &self,
        model_path: impl AsRef<Path>,
        repo_id: &str,
        metadata: Option<ModelMetadata>,
    ) -> Result<String> {
        let model_path = model_path.as_ref();

        // SECURITY: Validate repository ID format (prevents CLI injection)
        validate_repo_id(repo_id)?;

        // SECURITY: Validate and canonicalize file path (prevents path traversal)
        validate_upload_path(model_path)?;

        // For now, use git-based upload via huggingface-cli
        // In production, this would use the HF API
        self.upload_via_cli(model_path, repo_id, metadata)
    }

    /// Upload using huggingface-cli (requires huggingface-cli to be installed)
    fn upload_via_cli(
        &self,
        model_path: &Path,
        repo_id: &str,
        metadata: Option<ModelMetadata>,
    ) -> Result<String> {
        // Check if huggingface-cli is available
        if !self.has_hf_cli() {
            return Err(HubError::Config(
                "huggingface-cli not found. Install with: pip install huggingface_hub[cli]"
                    .to_string(),
            ));
        }

        // Create repository if needed
        if self.config.create_repo {
            self.create_repo_cli(repo_id)?;
        }

        // Upload model file
        self.upload_file_cli(model_path, repo_id)?;

        // Generate and upload model card if enabled
        if self.config.auto_model_card {
            if let Some(meta) = metadata {
                let card = self.generate_model_card(&meta);
                self.upload_model_card_cli(&card, repo_id)?;
            }
        }

        Ok(format!("https://huggingface.co/{}", repo_id))
    }

    /// Check if huggingface-cli is available
    fn has_hf_cli(&self) -> bool {
        std::process::Command::new("huggingface-cli")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Create repository using huggingface-cli
    fn create_repo_cli(&self, repo_id: &str) -> Result<()> {
        let mut args = vec![
            "repo".to_string(),
            "create".to_string(),
            repo_id.to_string(),
        ];

        if self.config.private {
            args.push("--private".to_string());
        }

        let status = std::process::Command::new("huggingface-cli")
            .args(&args)
            .env("HF_TOKEN", &self.config.hf_token)
            .status()
            .map_err(|e| HubError::Network(e.to_string()))?;

        if !status.success() && status.code() != Some(1) {
            // Exit code 1 might mean repo already exists
            return Err(HubError::Network("Failed to create repository".to_string()));
        }

        Ok(())
    }

    /// Upload file using huggingface-cli
    fn upload_file_cli(&self, file_path: &Path, repo_id: &str) -> Result<()> {
        let args = vec![
            "upload".to_string(),
            repo_id.to_string(),
            file_path.to_str().unwrap().to_string(),
            "--commit-message".to_string(),
            self.config.commit_message.clone(),
        ];

        let status = std::process::Command::new("huggingface-cli")
            .args(&args)
            .env("HF_TOKEN", &self.config.hf_token)
            .status()
            .map_err(|e| HubError::Network(e.to_string()))?;

        if !status.success() {
            return Err(HubError::Network("Failed to upload file".to_string()));
        }

        Ok(())
    }

    /// Generate model card from metadata
    fn generate_model_card(&self, metadata: &ModelMetadata) -> ModelCard {
        use super::model_card::{Framework, License, TaskType};

        let mut builder = ModelCardBuilder::new(&metadata.name);

        if let Some(desc) = &metadata.description {
            builder = builder.description(desc);
        }

        builder = builder
            .task(TaskType::TextGeneration)
            .framework(Framework::Gguf)
            .architecture(&metadata.architecture)
            .parameters((metadata.params_b * 1e9) as u64)
            .context_length(metadata.context_length);

        if let Some(quant) = &metadata.quantization {
            builder = builder.add_tag(quant);
        }

        if let Some(license) = &metadata.license {
            if let Ok(lic) = license.parse() {
                builder = builder.license(lic);
            }
        }

        for dataset in &metadata.datasets {
            builder = builder.add_dataset(dataset, None);
        }

        for tag in &metadata.tags {
            builder = builder.add_tag(tag);
        }

        builder.build()
    }

    /// Upload model card
    fn upload_model_card_cli(&self, card: &ModelCard, repo_id: &str) -> Result<()> {
        // Write card to temporary file
        let temp_dir = std::env::temp_dir();
        let card_path = temp_dir.join("README.md");
        fs::write(&card_path, card.to_markdown())?;

        // Upload README.md
        self.upload_file_cli(&card_path, repo_id)?;

        // Clean up
        let _ = fs::remove_file(&card_path);

        Ok(())
    }
}

/// Upload error type
#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    /// Authentication error
    #[error("Authentication failed: {0}")]
    Auth(String),
    /// Network error
    #[error("Network error: {0}")]
    Network(String),
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_config() {
        let config = UploadConfig::new("test_token".to_string());
        assert!(!config.private);
        assert!(config.create_repo);
        assert!(config.include_sona_weights);
    }

    #[test]
    fn test_upload_config_builder() {
        let config = UploadConfig::new("token".to_string())
            .private(true)
            .commit_message("Custom message");

        assert!(config.private);
        assert_eq!(config.commit_message, "Custom message");
    }

    #[test]
    fn test_model_metadata() {
        let metadata = ModelMetadata {
            name: "RuvLTRA Test".to_string(),
            description: Some("Test model".to_string()),
            architecture: "llama".to_string(),
            params_b: 0.5,
            context_length: 4096,
            quantization: Some("Q4_K_M".to_string()),
            license: Some("MIT".to_string()),
            datasets: vec!["dataset1".to_string()],
            tags: vec!["test".to_string()],
        };

        assert_eq!(metadata.params_b, 0.5);
        assert!(metadata.description.is_some());
    }
}
