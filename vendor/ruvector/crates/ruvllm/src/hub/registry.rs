//! RuvLTRA model registry with pre-configured models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Model size category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSize {
    /// Tiny models (< 1B parameters)
    Tiny,
    /// Small models (0.5B - 1B parameters)
    Small,
    /// Medium models (1B - 5B parameters)
    Medium,
    /// Large models (5B - 10B parameters)
    Large,
}

/// Quantization level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantizationLevel {
    /// 4-bit quantization (smallest, ~662MB for 0.5B model)
    Q4,
    /// 5-bit quantization (balanced)
    Q5,
    /// 8-bit quantization (highest quality)
    Q8,
    /// FP16 (no quantization)
    FP16,
}

impl QuantizationLevel {
    /// Get file size multiplier relative to FP16
    pub fn size_multiplier(&self) -> f32 {
        match self {
            Self::Q4 => 0.25,
            Self::Q5 => 0.3125,
            Self::Q8 => 0.5,
            Self::FP16 => 1.0,
        }
    }

    /// Get expected memory reduction
    pub fn memory_reduction(&self) -> f32 {
        match self {
            Self::Q4 => 0.75,  // 75% reduction
            Self::Q5 => 0.69,  // 69% reduction
            Self::Q8 => 0.50,  // 50% reduction
            Self::FP16 => 0.0, // No reduction
        }
    }
}

/// Hardware requirements for model execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareRequirements {
    /// Minimum RAM in GB
    pub min_ram_gb: f32,
    /// Recommended RAM in GB
    pub recommended_ram_gb: f32,
    /// Supports Apple Neural Engine
    pub supports_ane: bool,
    /// Supports Metal GPU acceleration
    pub supports_metal: bool,
    /// Supports CUDA
    pub supports_cuda: bool,
    /// Minimum GPU VRAM in GB (if using GPU)
    pub min_vram_gb: Option<f32>,
}

/// Model information in the registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model identifier (e.g., "ruvltra-small")
    pub id: String,
    /// Display name
    pub name: String,
    /// HuggingFace repository (e.g., "ruvnet/ruvltra-small")
    pub repo: String,
    /// Model filename on HF Hub
    pub filename: String,
    /// Model size category
    pub size: ModelSize,
    /// Quantization level
    pub quantization: QuantizationLevel,
    /// File size in bytes
    pub size_bytes: u64,
    /// SHA256 checksum
    pub checksum: Option<String>,
    /// Number of parameters (in billions)
    pub params_b: f32,
    /// Context window size
    pub context_length: usize,
    /// Hardware requirements
    pub hardware: HardwareRequirements,
    /// Model description
    pub description: String,
    /// Whether this is a LoRA adapter
    pub is_adapter: bool,
    /// Base model required (for adapters)
    pub base_model: Option<String>,
    /// Includes SONA pre-trained weights
    pub has_sona_weights: bool,
}

impl ModelInfo {
    /// Get download URL for this model
    pub fn download_url(&self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/main/{}",
            self.repo, self.filename
        )
    }

    /// Get HuggingFace Hub page URL
    pub fn hub_url(&self) -> String {
        format!("https://huggingface.co/{}", self.repo)
    }

    /// Estimate download time in seconds at given speed (MB/s)
    pub fn estimate_download_time(&self, speed_mbps: f32) -> f32 {
        let size_mb = self.size_bytes as f32 / (1024.0 * 1024.0);
        size_mb / speed_mbps
    }

    /// Check if model fits in available RAM
    pub fn fits_in_ram(&self, available_gb: f32) -> bool {
        available_gb >= self.hardware.min_ram_gb
    }
}

/// RuvLTRA model registry
pub struct RuvLtraRegistry {
    models: HashMap<String, ModelInfo>,
}

impl RuvLtraRegistry {
    /// Create a new registry with pre-configured models
    pub fn new() -> Self {
        let mut models = HashMap::new();

        // RuvLTRA-Small (0.5B) - Q4 quantization
        models.insert(
            "ruvltra-small".to_string(),
            ModelInfo {
                id: "ruvltra-small".to_string(),
                name: "RuvLTRA Small (0.5B Q4)".to_string(),
                repo: "ruv/ruvltra".to_string(),
                filename: "ruvltra-small-0.5b-q4_k_m.gguf".to_string(),
                size: ModelSize::Small,
                quantization: QuantizationLevel::Q4,
                size_bytes: 662_000_000, // ~662MB
                checksum: None,          // Set after publishing
                params_b: 0.5,
                context_length: 4096,
                hardware: HardwareRequirements {
                    min_ram_gb: 1.0,
                    recommended_ram_gb: 2.0,
                    supports_ane: true,
                    supports_metal: true,
                    supports_cuda: true,
                    min_vram_gb: Some(1.0),
                },
                description: "Compact RuvLTRA model optimized for edge devices. \
                             Includes SONA pre-trained weights for adaptive learning."
                    .to_string(),
                is_adapter: false,
                base_model: None,
                has_sona_weights: true,
            },
        );

        // RuvLTRA-Small (0.5B) - Q8 quantization
        models.insert(
            "ruvltra-small-q8".to_string(),
            ModelInfo {
                id: "ruvltra-small-q8".to_string(),
                name: "RuvLTRA Small (0.5B Q8)".to_string(),
                repo: "ruv/ruvltra".to_string(),
                filename: "ruvltra-small-0.5b-q8_0.gguf".to_string(),
                size: ModelSize::Small,
                quantization: QuantizationLevel::Q8,
                size_bytes: 1_324_000_000, // ~1.3GB
                checksum: None,
                params_b: 0.5,
                context_length: 4096,
                hardware: HardwareRequirements {
                    min_ram_gb: 2.0,
                    recommended_ram_gb: 4.0,
                    supports_ane: true,
                    supports_metal: true,
                    supports_cuda: true,
                    min_vram_gb: Some(2.0),
                },
                description: "High-quality Q8 quantization for better accuracy.".to_string(),
                is_adapter: false,
                base_model: None,
                has_sona_weights: true,
            },
        );

        // RuvLTRA-Medium (3B) - Q4 quantization
        models.insert(
            "ruvltra-medium".to_string(),
            ModelInfo {
                id: "ruvltra-medium".to_string(),
                name: "RuvLTRA Medium (3B Q4)".to_string(),
                repo: "ruv/ruvltra".to_string(),
                filename: "ruvltra-medium-1.1b-q4_k_m.gguf".to_string(),
                size: ModelSize::Medium,
                quantization: QuantizationLevel::Q4,
                size_bytes: 2_100_000_000, // ~2.1GB
                checksum: None,
                params_b: 3.0,
                context_length: 8192,
                hardware: HardwareRequirements {
                    min_ram_gb: 4.0,
                    recommended_ram_gb: 8.0,
                    supports_ane: true,
                    supports_metal: true,
                    supports_cuda: true,
                    min_vram_gb: Some(4.0),
                },
                description: "Balanced RuvLTRA model for general-purpose tasks. \
                             Extended context window with SONA learning."
                    .to_string(),
                is_adapter: false,
                base_model: None,
                has_sona_weights: true,
            },
        );

        // RuvLTRA-Medium (3B) - Q8 quantization
        models.insert(
            "ruvltra-medium-q8".to_string(),
            ModelInfo {
                id: "ruvltra-medium-q8".to_string(),
                name: "RuvLTRA Medium (3B Q8)".to_string(),
                repo: "ruv/ruvltra".to_string(),
                filename: "ruvltra-medium-1.1b-q8_0.gguf".to_string(),
                size: ModelSize::Medium,
                quantization: QuantizationLevel::Q8,
                size_bytes: 4_200_000_000, // ~4.2GB
                checksum: None,
                params_b: 3.0,
                context_length: 8192,
                hardware: HardwareRequirements {
                    min_ram_gb: 6.0,
                    recommended_ram_gb: 12.0,
                    supports_ane: true,
                    supports_metal: true,
                    supports_cuda: true,
                    min_vram_gb: Some(6.0),
                },
                description: "High-quality Medium model with Q8 quantization.".to_string(),
                is_adapter: false,
                base_model: None,
                has_sona_weights: true,
            },
        );

        // RuvLTRA-Small-Coder (LoRA adapter)
        models.insert(
            "ruvltra-small-coder".to_string(),
            ModelInfo {
                id: "ruvltra-small-coder".to_string(),
                name: "RuvLTRA Small Coder (LoRA)".to_string(),
                repo: "ruv/ruvltra".to_string(),
                filename: "ruvltra-small-coder-lora.safetensors".to_string(),
                size: ModelSize::Tiny,
                quantization: QuantizationLevel::FP16,
                size_bytes: 50_000_000, // ~50MB (LoRA is small)
                checksum: None,
                params_b: 0.05, // Adapter parameters
                context_length: 4096,
                hardware: HardwareRequirements {
                    min_ram_gb: 0.1,
                    recommended_ram_gb: 0.5,
                    supports_ane: true,
                    supports_metal: true,
                    supports_cuda: true,
                    min_vram_gb: None,
                },
                description: "LoRA adapter for code completion. \
                             Requires ruvltra-small or ruvltra-small-q8 base model."
                    .to_string(),
                is_adapter: true,
                base_model: Some("ruvltra-small".to_string()),
                has_sona_weights: false,
            },
        );

        Self { models }
    }

    /// Get model info by ID
    pub fn get(&self, id: &str) -> Option<&ModelInfo> {
        self.models.get(id)
    }

    /// Get all available models
    pub fn list_all(&self) -> Vec<&ModelInfo> {
        self.models.values().collect()
    }

    /// Get models by size
    pub fn list_by_size(&self, size: ModelSize) -> Vec<&ModelInfo> {
        self.models.values().filter(|m| m.size == size).collect()
    }

    /// Get base models (exclude adapters)
    pub fn list_base_models(&self) -> Vec<&ModelInfo> {
        self.models.values().filter(|m| !m.is_adapter).collect()
    }

    /// Get adapters for a specific base model
    pub fn list_adapters(&self, base_model: &str) -> Vec<&ModelInfo> {
        self.models
            .values()
            .filter(|m| {
                m.is_adapter
                    && m.base_model
                        .as_ref()
                        .map(|b| b == base_model)
                        .unwrap_or(false)
            })
            .collect()
    }

    /// Recommend model based on available RAM
    pub fn recommend_for_ram(&self, available_gb: f32) -> Option<&ModelInfo> {
        let mut candidates: Vec<_> = self
            .models
            .values()
            .filter(|m| !m.is_adapter && m.fits_in_ram(available_gb))
            .collect();

        // Sort by parameters (largest that fits)
        candidates.sort_by(|a, b| b.params_b.partial_cmp(&a.params_b).unwrap());

        candidates.first().copied()
    }

    /// Get model IDs
    pub fn model_ids(&self) -> Vec<String> {
        self.models.keys().cloned().collect()
    }
}

impl Default for RuvLtraRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Get model info by ID (convenience function)
pub fn get_model_info(id: &str) -> Option<ModelInfo> {
    RuvLtraRegistry::new().get(id).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_initialization() {
        let registry = RuvLtraRegistry::new();
        assert!(registry.get("ruvltra-small").is_some());
        assert!(registry.get("ruvltra-medium").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_model_info() {
        let registry = RuvLtraRegistry::new();
        let model = registry.get("ruvltra-small").unwrap();

        assert_eq!(model.params_b, 0.5);
        assert_eq!(model.quantization, QuantizationLevel::Q4);
        assert!(model.has_sona_weights);
        assert!(!model.is_adapter);
    }

    #[test]
    fn test_list_by_size() {
        let registry = RuvLtraRegistry::new();
        let small_models = registry.list_by_size(ModelSize::Small);
        assert!(!small_models.is_empty());
    }

    #[test]
    fn test_adapters() {
        let registry = RuvLtraRegistry::new();
        let adapters = registry.list_adapters("ruvltra-small");
        assert!(!adapters.is_empty());
        assert!(adapters[0].is_adapter);
    }

    #[test]
    fn test_ram_recommendation() {
        let registry = RuvLtraRegistry::new();

        // Should recommend small model for 2GB
        let model = registry.recommend_for_ram(2.0);
        assert!(model.is_some());
        assert!(model.unwrap().params_b <= 1.0);

        // Should recommend medium model for 8GB
        let model = registry.recommend_for_ram(8.0);
        assert!(model.is_some());
    }

    #[test]
    fn test_quantization_multipliers() {
        assert_eq!(QuantizationLevel::Q4.size_multiplier(), 0.25);
        assert_eq!(QuantizationLevel::Q8.size_multiplier(), 0.5);
        assert_eq!(QuantizationLevel::FP16.size_multiplier(), 1.0);
    }

    #[test]
    fn test_model_urls() {
        let registry = RuvLtraRegistry::new();
        let model = registry.get("ruvltra-small").unwrap();

        let url = model.download_url();
        assert!(url.contains("huggingface.co"));
        assert!(url.contains("ruv/ruvltra"));
        assert!(url.contains(".gguf"));

        let hub_url = model.hub_url();
        assert_eq!(hub_url, "https://huggingface.co/ruv/ruvltra");
    }

    #[test]
    fn test_download_time_estimation() {
        let registry = RuvLtraRegistry::new();
        let model = registry.get("ruvltra-small").unwrap();

        // At 10 MB/s, should take ~66 seconds
        let time = model.estimate_download_time(10.0);
        assert!(time > 60.0 && time < 70.0);
    }
}
