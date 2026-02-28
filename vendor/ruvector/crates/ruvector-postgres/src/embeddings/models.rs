//! Embedding model definitions and metadata

use serde::{Deserialize, Serialize};

/// Supported embedding models
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmbeddingModel {
    /// all-MiniLM-L6-v2: Fast, good quality, 384 dimensions
    AllMiniLmL6V2,
    /// BAAI/bge-small-en-v1.5: Fast, high quality, 384 dimensions
    BgeSmallEnV15,
    /// BAAI/bge-base-en-v1.5: Medium speed, higher quality, 768 dimensions
    BgeBaseEnV15,
    /// sentence-transformers/all-mpnet-base-v2: Medium speed, high quality, 768 dimensions
    AllMpnetBaseV2,
    /// nomic-ai/nomic-embed-text-v1.5: Good quality, 768 dimensions
    NomicEmbedTextV15,
    /// BAAI/bge-large-en-v1.5: Slower, highest quality, 1024 dimensions
    BgeLargeEnV15,
}

impl EmbeddingModel {
    /// Parse model name string to enum
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "all-minilm-l6-v2" | "minilm" | "default" => Some(Self::AllMiniLmL6V2),
            "bge-small-en-v1.5" | "bge-small" | "baai/bge-small-en-v1.5" => {
                Some(Self::BgeSmallEnV15)
            }
            "bge-base-en-v1.5" | "bge-base" | "baai/bge-base-en-v1.5" => Some(Self::BgeBaseEnV15),
            "bge-large-en-v1.5" | "bge-large" | "baai/bge-large-en-v1.5" => {
                Some(Self::BgeLargeEnV15)
            }
            "all-mpnet-base-v2" | "mpnet" | "sentence-transformers/all-mpnet-base-v2" => {
                Some(Self::AllMpnetBaseV2)
            }
            "nomic-embed-text-v1.5" | "nomic" | "nomic-ai/nomic-embed-text-v1.5" => {
                Some(Self::NomicEmbedTextV15)
            }
            _ => None,
        }
    }

    /// Get the canonical name for this model
    pub fn name(&self) -> &'static str {
        match self {
            Self::AllMiniLmL6V2 => "all-MiniLM-L6-v2",
            Self::BgeSmallEnV15 => "BAAI/bge-small-en-v1.5",
            Self::BgeBaseEnV15 => "BAAI/bge-base-en-v1.5",
            Self::BgeLargeEnV15 => "BAAI/bge-large-en-v1.5",
            Self::AllMpnetBaseV2 => "sentence-transformers/all-mpnet-base-v2",
            Self::NomicEmbedTextV15 => "nomic-ai/nomic-embed-text-v1.5",
        }
    }

    /// Get the embedding dimensions for this model
    pub fn dimensions(&self) -> usize {
        match self {
            Self::AllMiniLmL6V2 => 384,
            Self::BgeSmallEnV15 => 384,
            Self::BgeBaseEnV15 => 768,
            Self::BgeLargeEnV15 => 1024,
            Self::AllMpnetBaseV2 => 768,
            Self::NomicEmbedTextV15 => 768,
        }
    }

    /// Get a description of this model
    pub fn description(&self) -> &'static str {
        match self {
            Self::AllMiniLmL6V2 => "Fast general-purpose model, good for most use cases",
            Self::BgeSmallEnV15 => "High quality small model from BAAI, great for semantic search",
            Self::BgeBaseEnV15 => "Higher quality base model from BAAI, better accuracy",
            Self::BgeLargeEnV15 => "Highest quality large model from BAAI, best accuracy",
            Self::AllMpnetBaseV2 => "High quality model from sentence-transformers",
            Self::NomicEmbedTextV15 => "Modern model with good quality from Nomic AI",
        }
    }

    /// Get model speed rating (1-5, higher is faster)
    pub fn speed_rating(&self) -> u8 {
        match self {
            Self::AllMiniLmL6V2 => 5,
            Self::BgeSmallEnV15 => 5,
            Self::BgeBaseEnV15 => 3,
            Self::BgeLargeEnV15 => 1,
            Self::AllMpnetBaseV2 => 3,
            Self::NomicEmbedTextV15 => 3,
        }
    }

    /// Get model quality rating (1-5, higher is better)
    pub fn quality_rating(&self) -> u8 {
        match self {
            Self::AllMiniLmL6V2 => 3,
            Self::BgeSmallEnV15 => 4,
            Self::BgeBaseEnV15 => 4,
            Self::BgeLargeEnV15 => 5,
            Self::AllMpnetBaseV2 => 4,
            Self::NomicEmbedTextV15 => 4,
        }
    }

    /// Get approximate memory usage in MB
    pub fn memory_mb(&self) -> usize {
        match self {
            Self::AllMiniLmL6V2 => 90,
            Self::BgeSmallEnV15 => 130,
            Self::BgeBaseEnV15 => 440,
            Self::BgeLargeEnV15 => 1340,
            Self::AllMpnetBaseV2 => 440,
            Self::NomicEmbedTextV15 => 550,
        }
    }

    /// Get all supported models
    pub fn all() -> &'static [EmbeddingModel] {
        &[
            Self::AllMiniLmL6V2,
            Self::BgeSmallEnV15,
            Self::BgeBaseEnV15,
            Self::BgeLargeEnV15,
            Self::AllMpnetBaseV2,
            Self::NomicEmbedTextV15,
        ]
    }

    /// Get the default model
    pub fn default_model() -> Self {
        Self::AllMiniLmL6V2
    }
}

impl Default for EmbeddingModel {
    fn default() -> Self {
        Self::default_model()
    }
}

/// Model information for SQL queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub dimensions: i32,
    pub description: String,
    pub speed_rating: i32,
    pub quality_rating: i32,
    pub memory_mb: i32,
    pub loaded: bool,
}

impl From<EmbeddingModel> for ModelInfo {
    fn from(model: EmbeddingModel) -> Self {
        Self {
            name: model.name().to_string(),
            dimensions: model.dimensions() as i32,
            description: model.description().to_string(),
            speed_rating: model.speed_rating() as i32,
            quality_rating: model.quality_rating() as i32,
            memory_mb: model.memory_mb() as i32,
            loaded: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_parsing() {
        assert_eq!(
            EmbeddingModel::from_name("all-minilm-l6-v2"),
            Some(EmbeddingModel::AllMiniLmL6V2)
        );
        assert_eq!(
            EmbeddingModel::from_name("minilm"),
            Some(EmbeddingModel::AllMiniLmL6V2)
        );
        assert_eq!(
            EmbeddingModel::from_name("default"),
            Some(EmbeddingModel::AllMiniLmL6V2)
        );
        assert_eq!(
            EmbeddingModel::from_name("bge-small"),
            Some(EmbeddingModel::BgeSmallEnV15)
        );
        assert_eq!(EmbeddingModel::from_name("unknown"), None);
    }

    #[test]
    fn test_model_dimensions() {
        assert_eq!(EmbeddingModel::AllMiniLmL6V2.dimensions(), 384);
        assert_eq!(EmbeddingModel::BgeBaseEnV15.dimensions(), 768);
        assert_eq!(EmbeddingModel::BgeLargeEnV15.dimensions(), 1024);
    }

    #[test]
    fn test_all_models() {
        let models = EmbeddingModel::all();
        assert!(models.len() >= 4);
        for model in models {
            assert!(!model.name().is_empty());
            assert!(model.dimensions() > 0);
        }
    }
}
