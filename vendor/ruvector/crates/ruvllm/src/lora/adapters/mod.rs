//! Task-Specific LoRA Adapters for RuvLTRA
//!
//! This module provides pre-defined adapter configurations optimized for
//! different agent types in the Claude Flow ecosystem:
//! - Coder: Code generation and refactoring
//! - Researcher: Information analysis and synthesis
//! - Security: Vulnerability detection and secure coding
//! - Architect: System design and architecture
//! - Reviewer: Code review and quality assessment
//!
//! Each adapter is tuned with specific rank and alpha values for optimal
//! performance in its domain.

use crate::error::{Result, RuvLLMError};
use crate::lora::micro_lora::{MicroLoRA, MicroLoraConfig, TargetModule};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod merge;
pub mod trainer;

/// Pre-defined task-specific adapter configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuvLtraAdapters {
    /// Coder adapter: Optimized for code generation and refactoring
    /// - rank=16: High capacity for code patterns
    /// - alpha=32: Strong adaptation signal
    /// - targets: All attention modules for context understanding
    pub coder: LoraConfig,

    /// Researcher adapter: Optimized for information analysis
    /// - rank=8: Moderate capacity for analysis patterns
    /// - alpha=16: Balanced adaptation
    /// - targets: Q/K/V for attention to relevant information
    pub researcher: LoraConfig,

    /// Security adapter: Optimized for vulnerability detection
    /// - rank=16: High capacity for security patterns
    /// - alpha=32: Strong signal for critical issues
    /// - targets: All modules for comprehensive analysis
    pub security: LoraConfig,

    /// Architect adapter: Optimized for system design
    /// - rank=12: Good capacity for architectural patterns
    /// - alpha=24: Strong but balanced adaptation
    /// - targets: Attention + MLP for reasoning
    pub architect: LoraConfig,

    /// Reviewer adapter: Optimized for code review
    /// - rank=8: Focused capacity for review patterns
    /// - alpha=16: Balanced adaptation
    /// - targets: Q/V for attention to code quality
    pub reviewer: LoraConfig,
}

impl RuvLtraAdapters {
    /// Create default adapter configurations
    pub fn new() -> Self {
        Self {
            coder: LoraConfig {
                name: "coder".to_string(),
                rank: 16,
                alpha: 32.0,
                dropout: 0.05,
                target_modules: TargetModule::attention(),
                description: "Code generation and refactoring adapter".to_string(),
                domain_tags: vec![
                    "code-gen".to_string(),
                    "refactoring".to_string(),
                    "syntax".to_string(),
                ],
            },
            researcher: LoraConfig {
                name: "researcher".to_string(),
                rank: 8,
                alpha: 16.0,
                dropout: 0.1,
                target_modules: vec![
                    TargetModule::QProj,
                    TargetModule::KProj,
                    TargetModule::VProj,
                ],
                description: "Information analysis and synthesis adapter".to_string(),
                domain_tags: vec![
                    "analysis".to_string(),
                    "research".to_string(),
                    "synthesis".to_string(),
                ],
            },
            security: LoraConfig {
                name: "security".to_string(),
                rank: 16,
                alpha: 32.0,
                dropout: 0.05,
                target_modules: {
                    let mut modules = TargetModule::attention();
                    modules.extend(TargetModule::mlp());
                    modules
                },
                description: "Vulnerability detection and secure coding adapter".to_string(),
                domain_tags: vec![
                    "security".to_string(),
                    "vulnerabilities".to_string(),
                    "audit".to_string(),
                ],
            },
            architect: LoraConfig {
                name: "architect".to_string(),
                rank: 12,
                alpha: 24.0,
                dropout: 0.05,
                target_modules: vec![
                    TargetModule::QProj,
                    TargetModule::VProj,
                    TargetModule::GateProj,
                    TargetModule::UpProj,
                ],
                description: "System design and architecture adapter".to_string(),
                domain_tags: vec![
                    "architecture".to_string(),
                    "design".to_string(),
                    "patterns".to_string(),
                ],
            },
            reviewer: LoraConfig {
                name: "reviewer".to_string(),
                rank: 8,
                alpha: 16.0,
                dropout: 0.1,
                target_modules: vec![TargetModule::QProj, TargetModule::VProj],
                description: "Code review and quality assessment adapter".to_string(),
                domain_tags: vec![
                    "review".to_string(),
                    "quality".to_string(),
                    "best-practices".to_string(),
                ],
            },
        }
    }

    /// Get all adapters as a HashMap
    pub fn all(&self) -> HashMap<String, LoraConfig> {
        let mut map = HashMap::new();
        map.insert(self.coder.name.clone(), self.coder.clone());
        map.insert(self.researcher.name.clone(), self.researcher.clone());
        map.insert(self.security.name.clone(), self.security.clone());
        map.insert(self.architect.name.clone(), self.architect.clone());
        map.insert(self.reviewer.name.clone(), self.reviewer.clone());
        map
    }

    /// Get adapter configuration by name
    pub fn get(&self, name: &str) -> Option<&LoraConfig> {
        match name {
            "coder" => Some(&self.coder),
            "researcher" => Some(&self.researcher),
            "security" => Some(&self.security),
            "architect" => Some(&self.architect),
            "reviewer" => Some(&self.reviewer),
            _ => None,
        }
    }

    /// Get adapter configuration by domain tag
    pub fn by_domain(&self, domain: &str) -> Vec<&LoraConfig> {
        let domain = domain.to_lowercase();
        let mut configs = Vec::new();

        for config in [
            &self.coder,
            &self.researcher,
            &self.security,
            &self.architect,
            &self.reviewer,
        ] {
            if config
                .domain_tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&domain))
            {
                configs.push(config);
            }
        }

        configs
    }

    /// Create MicroLoRA instance from adapter name
    pub fn create_lora(&self, name: &str, hidden_dim: usize) -> Result<MicroLoRA> {
        let config = self
            .get(name)
            .ok_or_else(|| RuvLLMError::Config(format!("Unknown adapter: {}", name)))?;

        config.to_micro_lora_config(hidden_dim).map(MicroLoRA::new)
    }

    /// List all available adapter names
    pub fn list_names(&self) -> Vec<String> {
        vec![
            self.coder.name.clone(),
            self.researcher.name.clone(),
            self.security.name.clone(),
            self.architect.name.clone(),
            self.reviewer.name.clone(),
        ]
    }
}

impl Default for RuvLtraAdapters {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for a single LoRA adapter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoraConfig {
    /// Adapter name
    pub name: String,
    /// LoRA rank
    pub rank: usize,
    /// Alpha scaling factor
    pub alpha: f32,
    /// Dropout rate
    pub dropout: f32,
    /// Target modules to adapt
    pub target_modules: Vec<TargetModule>,
    /// Human-readable description
    pub description: String,
    /// Domain tags for categorization
    pub domain_tags: Vec<String>,
}

impl LoraConfig {
    /// Convert to MicroLoraConfig
    pub fn to_micro_lora_config(&self, hidden_dim: usize) -> Result<MicroLoraConfig> {
        Ok(MicroLoraConfig {
            rank: self.rank,
            alpha: self.alpha,
            dropout: self.dropout,
            target_modules: self.target_modules.clone(),
            in_features: hidden_dim,
            out_features: hidden_dim,
            use_bias: false,
            standard_init: true,
            gradient_checkpointing: false,
        })
    }

    /// Create builder for custom configuration
    pub fn builder(name: impl Into<String>) -> LoraConfigBuilder {
        LoraConfigBuilder::new(name)
    }

    /// Estimate memory usage for this adapter
    pub fn estimate_memory(&self, hidden_dim: usize) -> usize {
        let params_per_module = hidden_dim * self.rank + self.rank * hidden_dim;
        params_per_module * self.target_modules.len() * std::mem::size_of::<f32>()
    }

    /// Get parameter count
    pub fn param_count(&self, hidden_dim: usize) -> usize {
        let params_per_module = hidden_dim * self.rank + self.rank * hidden_dim;
        params_per_module * self.target_modules.len()
    }
}

/// Builder for custom LoRA configurations
pub struct LoraConfigBuilder {
    name: String,
    rank: usize,
    alpha: f32,
    dropout: f32,
    target_modules: Vec<TargetModule>,
    description: String,
    domain_tags: Vec<String>,
}

impl LoraConfigBuilder {
    /// Create a new builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rank: 8,
            alpha: 16.0,
            dropout: 0.05,
            target_modules: TargetModule::defaults(),
            description: String::new(),
            domain_tags: Vec::new(),
        }
    }

    /// Set rank
    pub fn rank(mut self, rank: usize) -> Self {
        self.rank = rank;
        self
    }

    /// Set alpha
    pub fn alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha;
        self
    }

    /// Set dropout
    pub fn dropout(mut self, dropout: f32) -> Self {
        self.dropout = dropout;
        self
    }

    /// Set target modules
    pub fn target_modules(mut self, modules: Vec<TargetModule>) -> Self {
        self.target_modules = modules;
        self
    }

    /// Set description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Add domain tag
    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.domain_tags.push(tag.into());
        self
    }

    /// Add multiple domain tags
    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.domain_tags = tags;
        self
    }

    /// Build the configuration
    pub fn build(self) -> LoraConfig {
        LoraConfig {
            name: self.name,
            rank: self.rank,
            alpha: self.alpha,
            dropout: self.dropout,
            target_modules: self.target_modules,
            description: self.description,
            domain_tags: self.domain_tags,
        }
    }
}

/// Adapter metadata for tracking and versioning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterMetadata {
    /// Adapter name
    pub name: String,
    /// Version string (semantic versioning)
    pub version: String,
    /// Training dataset description
    pub dataset: String,
    /// Number of training examples
    pub num_examples: usize,
    /// Training quality score
    pub quality_score: f32,
    /// Creation timestamp
    pub created_at: u64,
    /// Last modified timestamp
    pub modified_at: u64,
    /// Domain tags
    pub tags: Vec<String>,
    /// Additional custom metadata
    pub custom: HashMap<String, String>,
}

impl AdapterMetadata {
    /// Create new metadata
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            name: name.into(),
            version: version.into(),
            dataset: String::new(),
            num_examples: 0,
            quality_score: 0.0,
            created_at: now,
            modified_at: now,
            tags: Vec::new(),
            custom: HashMap::new(),
        }
    }

    /// Update modification timestamp
    pub fn touch(&mut self) {
        self.modified_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ruvltra_adapters_creation() {
        let adapters = RuvLtraAdapters::new();

        assert_eq!(adapters.coder.rank, 16);
        assert_eq!(adapters.coder.alpha, 32.0);

        assert_eq!(adapters.researcher.rank, 8);
        assert_eq!(adapters.researcher.alpha, 16.0);

        assert_eq!(adapters.security.rank, 16);
        assert_eq!(adapters.architect.rank, 12);
        assert_eq!(adapters.reviewer.rank, 8);
    }

    #[test]
    fn test_adapter_by_name() {
        let adapters = RuvLtraAdapters::new();

        let coder = adapters.get("coder").unwrap();
        assert_eq!(coder.name, "coder");

        assert!(adapters.get("nonexistent").is_none());
    }

    #[test]
    fn test_adapter_by_domain() {
        let adapters = RuvLtraAdapters::new();

        let security_adapters = adapters.by_domain("security");
        assert_eq!(security_adapters.len(), 1);
        assert_eq!(security_adapters[0].name, "security");

        let code_adapters = adapters.by_domain("code");
        assert!(!code_adapters.is_empty());
    }

    #[test]
    fn test_create_lora() {
        let adapters = RuvLtraAdapters::new();
        let lora = adapters.create_lora("coder", 768).unwrap();

        assert_eq!(lora.config().rank, 16);
        assert_eq!(lora.config().in_features, 768);
    }

    #[test]
    fn test_memory_estimation() {
        let adapters = RuvLtraAdapters::new();

        let coder_mem = adapters.coder.estimate_memory(768);
        let researcher_mem = adapters.researcher.estimate_memory(768);

        // Coder has rank=16, researcher has rank=8
        // With same target modules, coder should use ~2x memory
        assert!(coder_mem > researcher_mem);
    }

    #[test]
    fn test_config_builder() {
        let config = LoraConfig::builder("custom")
            .rank(4)
            .alpha(8.0)
            .dropout(0.2)
            .description("Custom adapter")
            .add_tag("test")
            .build();

        assert_eq!(config.name, "custom");
        assert_eq!(config.rank, 4);
        assert_eq!(config.alpha, 8.0);
        assert_eq!(config.dropout, 0.2);
        assert!(config.domain_tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_list_names() {
        let adapters = RuvLtraAdapters::new();
        let names = adapters.list_names();

        assert_eq!(names.len(), 5);
        assert!(names.contains(&"coder".to_string()));
        assert!(names.contains(&"researcher".to_string()));
        assert!(names.contains(&"security".to_string()));
        assert!(names.contains(&"architect".to_string()));
        assert!(names.contains(&"reviewer".to_string()));
    }

    #[test]
    fn test_adapter_metadata() {
        let mut metadata = AdapterMetadata::new("test-adapter", "1.0.0");

        assert_eq!(metadata.name, "test-adapter");
        assert_eq!(metadata.version, "1.0.0");

        let original_modified = metadata.modified_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        metadata.touch();

        assert!(metadata.modified_at > original_modified);
    }
}
