//! SQL function implementations for embedding generation

use pgrx::prelude::*;

use super::cache::global_cache;
use super::models::{EmbeddingModel, ModelInfo};
use super::{MAX_BATCH_SIZE, MAX_TEXT_LENGTH};

// ============================================================================
// Core Embedding Functions
// ============================================================================

/// Generate an embedding vector from text
///
/// # Arguments
/// * `text` - The text to embed
/// * `model_name` - Optional model name (defaults to 'all-MiniLM-L6-v2')
///
/// # Returns
/// A vector of f32 values representing the text embedding
///
/// # Example
/// ```sql
/// SELECT ruvector_embed('Hello world');
/// SELECT ruvector_embed('Hello world', 'bge-small');
/// ```
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_embed(text: &str, model_name: default!(&str, "'all-MiniLM-L6-v2'")) -> Vec<f32> {
    // Validate text length
    if text.len() > MAX_TEXT_LENGTH {
        pgrx::error!(
            "Text length {} exceeds maximum {} characters",
            text.len(),
            MAX_TEXT_LENGTH
        );
    }

    // Parse model name
    let model = EmbeddingModel::from_name(model_name).unwrap_or_else(|| {
        pgrx::warning!("Unknown model '{}', using default", model_name);
        EmbeddingModel::default()
    });

    // Generate embedding using cached model
    let documents = vec![text];
    match global_cache().embed(model, documents) {
        Ok(embeddings) => {
            if let Some(embedding) = embeddings.into_iter().next() {
                embedding
            } else {
                pgrx::error!("No embedding generated");
            }
        }
        Err(e) => {
            pgrx::error!("Embedding generation failed: {}", e);
        }
    }
}

/// Generate embeddings for multiple texts in batch
///
/// # Arguments
/// * `texts` - Array of texts to embed
/// * `model_name` - Optional model name (defaults to 'all-MiniLM-L6-v2')
///
/// # Returns
/// A 2D array of embeddings (one per input text)
///
/// # Example
/// ```sql
/// SELECT ruvector_embed_batch(ARRAY['Hello', 'World', 'Test']);
/// ```
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_embed_batch(
    texts: Vec<String>,
    model_name: default!(&str, "'all-MiniLM-L6-v2'"),
) -> Vec<Vec<f32>> {
    // Validate batch size
    if texts.len() > MAX_BATCH_SIZE {
        pgrx::error!(
            "Batch size {} exceeds maximum {}",
            texts.len(),
            MAX_BATCH_SIZE
        );
    }

    // Validate text lengths
    for (i, text) in texts.iter().enumerate() {
        if text.len() > MAX_TEXT_LENGTH {
            pgrx::error!(
                "Text at index {} exceeds maximum {} characters",
                i,
                MAX_TEXT_LENGTH
            );
        }
    }

    // Parse model name
    let model = EmbeddingModel::from_name(model_name).unwrap_or_else(|| {
        pgrx::warning!("Unknown model '{}', using default", model_name);
        EmbeddingModel::default()
    });

    // Generate embeddings using cached model
    let documents: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
    match global_cache().embed(model, documents) {
        Ok(embeddings) => embeddings,
        Err(e) => {
            pgrx::error!("Batch embedding generation failed: {}", e);
        }
    }
}

// ============================================================================
// Model Management Functions
// ============================================================================

/// List all available embedding models
///
/// # Returns
/// Table with model name, dimensions, and description
///
/// # Example
/// ```sql
/// SELECT * FROM ruvector_embedding_models();
/// ```
#[pg_extern]
pub fn ruvector_embedding_models() -> TableIterator<
    'static,
    (
        name!(name, String),
        name!(dimensions, i32),
        name!(description, String),
        name!(speed, i32),
        name!(quality, i32),
        name!(memory_mb, i32),
        name!(loaded, bool),
    ),
> {
    let cache = global_cache();
    let rows: Vec<_> = EmbeddingModel::all()
        .iter()
        .map(|model| {
            (
                model.name().to_string(),
                model.dimensions() as i32,
                model.description().to_string(),
                model.speed_rating() as i32,
                model.quality_rating() as i32,
                model.memory_mb() as i32,
                cache.is_loaded(*model),
            )
        })
        .collect();

    TableIterator::new(rows)
}

/// Pre-load an embedding model into cache
///
/// # Arguments
/// * `model_name` - Name of the model to load
///
/// # Returns
/// true if loaded successfully
///
/// # Example
/// ```sql
/// SELECT ruvector_load_model('bge-small');
/// ```
#[pg_extern]
pub fn ruvector_load_model(model_name: &str) -> bool {
    let model = match EmbeddingModel::from_name(model_name) {
        Some(m) => m,
        None => {
            pgrx::warning!("Unknown model: {}", model_name);
            return false;
        }
    };

    match global_cache().preload(model) {
        Ok(_) => {
            pgrx::info!("Model '{}' loaded successfully", model.name());
            true
        }
        Err(e) => {
            pgrx::warning!("Failed to load model '{}': {}", model_name, e);
            false
        }
    }
}

/// Unload an embedding model from cache
///
/// # Arguments
/// * `model_name` - Name of the model to unload
///
/// # Returns
/// true if the model was unloaded
///
/// # Example
/// ```sql
/// SELECT ruvector_unload_model('bge-small');
/// ```
#[pg_extern]
pub fn ruvector_unload_model(model_name: &str) -> bool {
    let model = match EmbeddingModel::from_name(model_name) {
        Some(m) => m,
        None => {
            pgrx::warning!("Unknown model: {}", model_name);
            return false;
        }
    };

    global_cache().unload(model)
}

/// Get information about a specific model
///
/// # Arguments
/// * `model_name` - Name of the model
///
/// # Returns
/// JSON object with model information
///
/// # Example
/// ```sql
/// SELECT ruvector_model_info('all-MiniLM-L6-v2');
/// ```
#[pg_extern]
pub fn ruvector_model_info(model_name: &str) -> pgrx::JsonB {
    let model = match EmbeddingModel::from_name(model_name) {
        Some(m) => m,
        None => {
            return pgrx::JsonB(serde_json::json!({
                "error": format!("Unknown model: {}", model_name),
                "available_models": EmbeddingModel::all().iter().map(|m| m.name()).collect::<Vec<_>>()
            }));
        }
    };

    let cache = global_cache();
    let mut info = ModelInfo::from(model);
    info.loaded = cache.is_loaded(model);

    pgrx::JsonB(serde_json::to_value(info).unwrap_or_default())
}

/// Set the default embedding model
///
/// # Arguments
/// * `model_name` - Name of the model to set as default
///
/// # Returns
/// true if set successfully
///
/// # Example
/// ```sql
/// SELECT ruvector_set_default_model('bge-small');
/// ```
#[pg_extern]
pub fn ruvector_set_default_model(model_name: &str) -> bool {
    let model = match EmbeddingModel::from_name(model_name) {
        Some(m) => m,
        None => {
            pgrx::warning!("Unknown model: {}", model_name);
            return false;
        }
    };

    global_cache().set_default_model(model);
    pgrx::info!("Default model set to '{}'", model.name());
    true
}

/// Get the current default embedding model name
///
/// # Returns
/// Name of the default model
///
/// # Example
/// ```sql
/// SELECT ruvector_default_model();
/// ```
#[pg_extern]
pub fn ruvector_default_model() -> String {
    global_cache().default_model().name().to_string()
}

/// Get embedding cache statistics
///
/// # Returns
/// JSON object with cache statistics
///
/// # Example
/// ```sql
/// SELECT ruvector_embedding_stats();
/// ```
#[pg_extern]
pub fn ruvector_embedding_stats() -> pgrx::JsonB {
    let cache = global_cache();
    let loaded_models = cache.loaded_models();

    pgrx::JsonB(serde_json::json!({
        "loaded_model_count": loaded_models.len(),
        "loaded_models": loaded_models.iter().map(|m| m.name()).collect::<Vec<_>>(),
        "estimated_memory_mb": cache.estimated_memory_usage() / (1024 * 1024),
        "default_model": cache.default_model().name(),
        "available_model_count": EmbeddingModel::all().len(),
    }))
}

/// Get embedding dimensions for a model
///
/// # Arguments
/// * `model_name` - Name of the model
///
/// # Returns
/// Number of dimensions, or -1 if model unknown
///
/// # Example
/// ```sql
/// SELECT ruvector_embedding_dims('all-MiniLM-L6-v2');  -- Returns 384
/// ```
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_embedding_dims(model_name: &str) -> i32 {
    match EmbeddingModel::from_name(model_name) {
        Some(m) => m.dimensions() as i32,
        None => -1,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_embedding_models_list() {
        let models: Vec<_> = ruvector_embedding_models().collect();
        assert!(!models.is_empty());
    }

    #[pg_test]
    fn test_model_info() {
        let info = ruvector_model_info("all-MiniLM-L6-v2");
        let json = info.0;
        assert!(json.get("name").is_some());
        assert!(json.get("dimensions").is_some());
    }

    #[pg_test]
    fn test_default_model() {
        let name = ruvector_default_model();
        assert!(!name.is_empty());
    }

    #[pg_test]
    fn test_embedding_dims() {
        assert_eq!(ruvector_embedding_dims("all-MiniLM-L6-v2"), 384);
        assert_eq!(ruvector_embedding_dims("bge-base"), 768);
        assert_eq!(ruvector_embedding_dims("unknown"), -1);
    }
}
