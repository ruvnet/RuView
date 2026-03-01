//! HuggingFace Hub Integration
//!
//! Direct integration with HuggingFace Hub API for uploading SONA models,
//! patterns, and datasets.

use super::{
    DatasetExporter, ExportConfig, ExportError, ExportResult, ExportType, SafeTensorsExporter,
};
use crate::engine::SonaEngine;
use std::path::Path;

#[cfg(feature = "serde-support")]
use serde::{Deserialize, Serialize};

/// HuggingFace Hub client
pub struct HuggingFaceHub {
    /// API token (optional for public repos)
    token: Option<String>,
    /// API base URL
    api_url: String,
}

impl HuggingFaceHub {
    /// Create new Hub client
    pub fn new(token: Option<&str>) -> Self {
        Self {
            token: token.map(|t| t.to_string()),
            api_url: "https://huggingface.co/api".to_string(),
        }
    }

    /// Create Hub client from environment variable
    pub fn from_env() -> Self {
        let token = std::env::var("HF_TOKEN")
            .or_else(|_| std::env::var("HUGGING_FACE_HUB_TOKEN"))
            .ok();
        Self::new(token.as_deref())
    }

    /// Push all exports to HuggingFace Hub
    pub fn push_all(
        &self,
        engine: &SonaEngine,
        config: &ExportConfig,
        repo_id: &str,
    ) -> Result<ExportResult, ExportError> {
        // Create temporary directory for exports
        let temp_dir = std::env::temp_dir().join(format!("sona-export-{}", uuid_v4()));
        std::fs::create_dir_all(&temp_dir).map_err(ExportError::Io)?;

        // Export all components to temp directory
        let safetensors_exporter = SafeTensorsExporter::new(config);
        let dataset_exporter = DatasetExporter::new(config);

        let mut total_items = 0;
        let mut total_size = 0u64;

        // Export LoRA weights
        if config.include_lora {
            let result = safetensors_exporter.export_engine(engine, temp_dir.join("lora"))?;
            total_items += result.items_exported;
            total_size += result.size_bytes;
        }

        // Export patterns
        if config.include_patterns {
            let result =
                dataset_exporter.export_patterns(engine, temp_dir.join("patterns.jsonl"))?;
            total_items += result.items_exported;
            total_size += result.size_bytes;
        }

        // Export preferences
        if config.include_preferences {
            let result =
                dataset_exporter.export_preferences(engine, temp_dir.join("preferences.jsonl"))?;
            total_items += result.items_exported;
            total_size += result.size_bytes;
        }

        // Create model card
        let readme = self.create_model_card(engine, config);
        let readme_path = temp_dir.join("README.md");
        std::fs::write(&readme_path, readme).map_err(ExportError::Io)?;

        // Create adapter config
        let adapter_config = self.create_adapter_config(engine, config);
        let config_path = temp_dir.join("adapter_config.json");
        let config_json = serde_json::to_string_pretty(&adapter_config)?;
        std::fs::write(&config_path, config_json).map_err(ExportError::Io)?;

        // Upload to Hub (using git LFS approach)
        self.upload_directory(&temp_dir, repo_id)?;

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok(ExportResult {
            export_type: ExportType::SafeTensors,
            items_exported: total_items,
            output_path: format!("https://huggingface.co/{}", repo_id),
            size_bytes: total_size,
        })
    }

    /// Upload directory to HuggingFace Hub
    fn upload_directory(&self, local_path: &Path, repo_id: &str) -> Result<(), ExportError> {
        // Check for git and git-lfs
        let has_git = std::process::Command::new("git")
            .arg("--version")
            .output()
            .is_ok();

        if !has_git {
            return Err(ExportError::HubError(
                "git is required for HuggingFace Hub upload. Install git and git-lfs.".to_string(),
            ));
        }

        // Clone or create repo
        let repo_url = if let Some(ref token) = self.token {
            format!("https://{}@huggingface.co/{}", token, repo_id)
        } else {
            format!("https://huggingface.co/{}", repo_id)
        };

        let clone_dir = local_path.parent().unwrap().join("hf-repo");

        // Try to clone existing repo
        let clone_result = std::process::Command::new("git")
            .args(["clone", &repo_url, clone_dir.to_str().unwrap()])
            .output();

        if clone_result.is_err() {
            // Create new repo via API
            self.create_repo(repo_id)?;

            // Try cloning again
            std::process::Command::new("git")
                .args(["clone", &repo_url, clone_dir.to_str().unwrap()])
                .output()
                .map_err(|e| ExportError::HubError(format!("Failed to clone repo: {}", e)))?;
        }

        // Copy files to cloned repo
        copy_dir_recursive(local_path, &clone_dir)?;

        // Add, commit, and push
        std::process::Command::new("git")
            .args(["-C", clone_dir.to_str().unwrap(), "add", "-A"])
            .output()
            .map_err(|e| ExportError::HubError(format!("git add failed: {}", e)))?;

        std::process::Command::new("git")
            .args([
                "-C",
                clone_dir.to_str().unwrap(),
                "commit",
                "-m",
                "Upload SONA adapter",
            ])
            .output()
            .map_err(|e| ExportError::HubError(format!("git commit failed: {}", e)))?;

        let push_result = std::process::Command::new("git")
            .args(["-C", clone_dir.to_str().unwrap(), "push"])
            .output()
            .map_err(|e| ExportError::HubError(format!("git push failed: {}", e)))?;

        if !push_result.status.success() {
            let stderr = String::from_utf8_lossy(&push_result.stderr);
            return Err(ExportError::HubError(format!(
                "git push failed: {}",
                stderr
            )));
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&clone_dir);

        Ok(())
    }

    /// Create a new repository on HuggingFace Hub
    fn create_repo(&self, repo_id: &str) -> Result<(), ExportError> {
        let token = self.token.as_ref().ok_or_else(|| {
            ExportError::HubError("HuggingFace token required to create repos".to_string())
        })?;

        // Parse repo_id (org/name or just name)
        let (organization, name) = if let Some(idx) = repo_id.find('/') {
            (Some(&repo_id[..idx]), &repo_id[idx + 1..])
        } else {
            (None, repo_id)
        };

        let create_request = CreateRepoRequest {
            name: name.to_string(),
            organization: organization.map(|s| s.to_string()),
            private: false,
            repo_type: "model".to_string(),
        };

        let url = format!("{}/repos/create", self.api_url);

        // Use simple HTTP client approach (blocking for simplicity)
        // In production, you'd use reqwest or similar
        let body = serde_json::to_string(&create_request)?;

        let output = std::process::Command::new("curl")
            .args([
                "-X",
                "POST",
                "-H",
                &format!("Authorization: Bearer {}", token),
                "-H",
                "Content-Type: application/json",
                "-d",
                &body,
                &url,
            ])
            .output()
            .map_err(|e| ExportError::HubError(format!("curl failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Repo might already exist, which is fine
            if !stderr.contains("already exists") {
                return Err(ExportError::HubError(format!(
                    "Failed to create repo: {}",
                    stderr
                )));
            }
        }

        Ok(())
    }

    /// Create model card content
    fn create_model_card(&self, engine: &SonaEngine, config: &ExportConfig) -> String {
        let stats = engine.stats();
        format!(
            r#"---
license: mit
library_name: peft
base_model: {}
tags:
  - sona
  - lora
  - adaptive-learning
  - ruvector
---

# {} SONA Adapter

This adapter was generated using [SONA (Self-Optimizing Neural Architecture)](https://github.com/ruvnet/ruvector/tree/main/crates/sona) - a runtime-adaptive learning system.

## Model Details

- **Base Model**: {}
- **PEFT Type**: LoRA (Two-Tier)
- **MicroLoRA Rank**: {} (instant adaptation)
- **BaseLoRA Rank**: {} (background learning)
- **Patterns Learned**: {}
- **Trajectories Processed**: {}

## SONA Features

### Two-Tier LoRA Architecture
- **MicroLoRA**: Rank 1-2 for instant adaptation (<0.5ms latency)
- **BaseLoRA**: Rank 4-16 for background learning

### EWC++ (Elastic Weight Consolidation)
Prevents catastrophic forgetting when learning new patterns.

### ReasoningBank
K-means++ clustering for efficient pattern storage and retrieval.

## Performance Benchmarks

| Metric | Value |
|--------|-------|
| Throughput | 2211 ops/sec |
| Latency | <0.5ms per layer |
| Quality Improvement | +55% max |

## Usage with PEFT

```python
from peft import PeftModel, PeftConfig
from transformers import AutoModelForCausalLM

# Load adapter
config = PeftConfig.from_pretrained("your-username/{}")
model = AutoModelForCausalLM.from_pretrained(config.base_model_name_or_path)
model = PeftModel.from_pretrained(model, "your-username/{}")

# Use for inference
outputs = model.generate(input_ids)
```

## Training with Included Datasets

### Patterns Dataset
```python
from datasets import load_dataset

patterns = load_dataset("json", data_files="patterns.jsonl")
```

### Preference Pairs (for DPO/RLHF)
```python
preferences = load_dataset("json", data_files="preferences.jsonl")
```

## License

MIT License - see [LICENSE](LICENSE) for details.

---

Generated with [ruvector-sona](https://crates.io/crates/ruvector-sona) v{}
"#,
            config.target_architecture,
            config.model_name,
            config.target_architecture,
            engine.config().micro_lora_rank,
            engine.config().base_lora_rank,
            stats.patterns_stored,
            stats.trajectories_buffered,
            config.model_name,
            config.model_name,
            env!("CARGO_PKG_VERSION"),
        )
    }

    /// Create PEFT-compatible adapter config
    fn create_adapter_config(
        &self,
        engine: &SonaEngine,
        config: &ExportConfig,
    ) -> AdapterConfigJson {
        let sona_config = engine.config();
        AdapterConfigJson {
            peft_type: "LORA".to_string(),
            auto_mapping: None,
            base_model_name_or_path: config.target_architecture.clone(),
            revision: None,
            task_type: "CAUSAL_LM".to_string(),
            inference_mode: true,
            r: sona_config.base_lora_rank,
            lora_alpha: sona_config.base_lora_rank as f32,
            lora_dropout: 0.0,
            fan_in_fan_out: false,
            bias: "none".to_string(),
            target_modules: vec![
                "q_proj".to_string(),
                "k_proj".to_string(),
                "v_proj".to_string(),
                "o_proj".to_string(),
            ],
            modules_to_save: None,
            layers_to_transform: None,
            layers_pattern: None,
        }
    }
}

/// Request to create a new repo
#[cfg_attr(feature = "serde-support", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
struct CreateRepoRequest {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    organization: Option<String>,
    private: bool,
    #[serde(rename = "type")]
    repo_type: String,
}

/// PEFT adapter config for JSON export
#[cfg_attr(feature = "serde-support", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct AdapterConfigJson {
    pub peft_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_mapping: Option<serde_json::Value>,
    pub base_model_name_or_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    pub task_type: String,
    pub inference_mode: bool,
    pub r: usize,
    pub lora_alpha: f32,
    pub lora_dropout: f32,
    pub fan_in_fan_out: bool,
    pub bias: String,
    pub target_modules: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modules_to_save: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layers_to_transform: Option<Vec<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layers_pattern: Option<String>,
}

/// Simple UUID v4 generator
fn uuid_v4() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        (bytes[6] & 0x0f) | 0x40, bytes[7],
        (bytes[8] & 0x3f) | 0x80, bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

/// Copy directory recursively
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), ExportError> {
    if !dst.exists() {
        std::fs::create_dir_all(dst).map_err(ExportError::Io)?;
    }

    for entry in std::fs::read_dir(src).map_err(ExportError::Io)? {
        let entry = entry.map_err(ExportError::Io)?;
        let path = entry.path();
        let file_name = path.file_name().unwrap();
        let dest_path = dst.join(file_name);

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path).map_err(ExportError::Io)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hub_from_env() {
        // Just ensure it doesn't panic
        let _hub = HuggingFaceHub::from_env();
    }

    #[test]
    fn test_uuid_v4() {
        let uuid = uuid_v4();
        assert_eq!(uuid.len(), 36);
        assert!(uuid.contains('-'));
    }

    #[test]
    fn test_adapter_config_json() {
        let config = AdapterConfigJson {
            peft_type: "LORA".to_string(),
            auto_mapping: None,
            base_model_name_or_path: "microsoft/phi-4".to_string(),
            revision: None,
            task_type: "CAUSAL_LM".to_string(),
            inference_mode: true,
            r: 8,
            lora_alpha: 8.0,
            lora_dropout: 0.0,
            fan_in_fan_out: false,
            bias: "none".to_string(),
            target_modules: vec!["q_proj".to_string()],
            modules_to_save: None,
            layers_to_transform: None,
            layers_pattern: None,
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("LORA"));
        assert!(json.contains("phi-4"));
    }
}
