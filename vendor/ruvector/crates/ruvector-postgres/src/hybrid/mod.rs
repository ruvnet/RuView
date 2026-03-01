//! Hybrid Search (BM25 + Vector) for RuVector Postgres
//!
//! Provides combined keyword and semantic vector search with multiple fusion strategies.
//!
//! ## Features
//!
//! - **BM25 Scoring**: Proper BM25 implementation with document length normalization
//! - **Fusion Algorithms**: RRF (default), Linear blend, Learned/adaptive
//! - **Parallel Execution**: Vector and keyword branches can run concurrently
//! - **Registry System**: Track hybrid-enabled collections with per-collection settings
//!
//! ## SQL Interface
//!
//! ```sql
//! -- Register a collection for hybrid search
//! SELECT ruvector_register_hybrid(
//!     collection := 'documents',
//!     vector_column := 'embedding',
//!     fts_column := 'fts',
//!     text_column := 'content'
//! );
//!
//! -- Perform hybrid search
//! SELECT * FROM ruvector_hybrid_search(
//!     'documents',
//!     query_text := 'database query optimization',
//!     query_vector := $embedding,
//!     k := 10,
//!     fusion := 'rrf'
//! );
//! ```

pub mod bm25;
pub mod executor;
pub mod fusion;
pub mod registry;

// Re-exports
pub use bm25::{tokenize_query, BM25Config, BM25Scorer, CorpusStats, Document, TermFrequencies};
pub use executor::{
    choose_strategy, BranchResults, ExecutionStats, HybridExecutor, HybridQuery, HybridResult,
    HybridStrategy,
};
pub use fusion::{
    fuse_results, learned_fusion, linear_fusion, rrf_fusion, DocId, FusedResult, FusionConfig,
    FusionMethod, FusionModel, DEFAULT_ALPHA, DEFAULT_RRF_K,
};
pub use registry::{
    get_registry, HybridCollectionConfig, HybridConfigUpdate, HybridRegistry, RegistryError,
    HYBRID_REGISTRY,
};

use pgrx::prelude::*;

// ============================================================================
// SQL Functions
// ============================================================================

/// Register a collection for hybrid search
///
/// Creates the necessary metadata and computes initial corpus statistics.
///
/// # Arguments
/// * `collection` - Table name (optionally schema-qualified)
/// * `vector_column` - Name of the vector column
/// * `fts_column` - Name of the tsvector column
/// * `text_column` - Name of the original text column (for BM25 stats)
///
/// # Returns
/// JSON object with registration details
#[pg_extern]
fn ruvector_register_hybrid(
    collection: &str,
    vector_column: &str,
    fts_column: &str,
    text_column: &str,
) -> pgrx::JsonB {
    // Parse collection name
    let (schema, table) = parse_collection_name(collection);

    // For now, use a simple hash as collection ID
    // In production, this would query ruvector.collections table
    let collection_id = collection
        .bytes()
        .fold(0i32, |acc, b| acc.wrapping_add(b as i32));

    // Check if already registered
    let registry = get_registry();
    if registry.is_registered(collection_id) {
        return pgrx::JsonB(serde_json::json!({
            "success": false,
            "error": format!("Collection '{}' is already registered for hybrid search", collection),
            "collection_id": collection_id
        }));
    }

    // Create configuration
    let mut config = HybridCollectionConfig::new(
        collection_id,
        table.to_string(),
        vector_column.to_string(),
        fts_column.to_string(),
        text_column.to_string(),
    );
    config.schema_name = schema.to_string();

    // Register
    match registry.register(config) {
        Ok(_) => pgrx::JsonB(serde_json::json!({
            "success": true,
            "collection_id": collection_id,
            "collection": collection,
            "vector_column": vector_column,
            "fts_column": fts_column,
            "text_column": text_column,
            "message": "Collection registered for hybrid search. Run ruvector_hybrid_update_stats() to compute corpus statistics."
        })),
        Err(e) => pgrx::JsonB(serde_json::json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

/// Update BM25 corpus statistics for a hybrid collection
///
/// Computes average document length, document count, and term frequencies.
/// Should be run periodically or after bulk inserts.
#[pg_extern]
fn ruvector_hybrid_update_stats(collection: &str) -> pgrx::JsonB {
    let (schema, table) = parse_collection_name(collection);
    let qualified_name = format!("{}.{}", schema, table);

    let registry = get_registry();
    let config = match registry.get_by_name(&qualified_name) {
        Some(c) => c,
        None => {
            return pgrx::JsonB(serde_json::json!({
                "success": false,
                "error": format!("Collection '{}' is not registered for hybrid search", collection)
            }));
        }
    };

    // In the actual extension, we would run SQL to compute stats:
    // SELECT AVG(LENGTH(text_column)), COUNT(*) FROM table
    // For now, we return a placeholder indicating the function works

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let stats = CorpusStats {
        avg_doc_length: config.corpus_stats.avg_doc_length,
        doc_count: config.corpus_stats.doc_count,
        total_terms: config.corpus_stats.total_terms,
        last_update: now,
    };

    match registry.update_stats(config.collection_id, stats) {
        Ok(_) => pgrx::JsonB(serde_json::json!({
            "success": true,
            "collection": collection,
            "message": "Stats update initiated. In production, this would compute actual corpus statistics.",
            "note": "Use Spi::run to execute SQL for actual stats computation"
        })),
        Err(e) => pgrx::JsonB(serde_json::json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

/// Configure hybrid search settings for a collection
///
/// # Arguments
/// * `collection` - Collection name
/// * `config` - JSON configuration object
///
/// # Example Configuration
/// ```json
/// {
///     "default_fusion": "rrf",
///     "default_alpha": 0.5,
///     "rrf_k": 60,
///     "prefetch_k": 100,
///     "bm25_k1": 1.2,
///     "bm25_b": 0.75,
///     "stats_refresh_interval": "1 hour",
///     "parallel_enabled": true
/// }
/// ```
#[pg_extern]
fn ruvector_hybrid_configure(collection: &str, config: pgrx::JsonB) -> pgrx::JsonB {
    let (schema, table) = parse_collection_name(collection);
    let qualified_name = format!("{}.{}", schema, table);

    let registry = get_registry();
    let mut existing_config = match registry.get_by_name(&qualified_name) {
        Some(c) => c,
        None => {
            return pgrx::JsonB(serde_json::json!({
                "success": false,
                "error": format!("Collection '{}' is not registered for hybrid search", collection)
            }));
        }
    };

    // Parse and apply updates
    let update: HybridConfigUpdate = match serde_json::from_value(config.0.clone()) {
        Ok(u) => u,
        Err(e) => {
            return pgrx::JsonB(serde_json::json!({
                "success": false,
                "error": format!("Invalid configuration: {}", e)
            }));
        }
    };

    if let Err(e) = update.apply(&mut existing_config) {
        return pgrx::JsonB(serde_json::json!({
            "success": false,
            "error": e.to_string()
        }));
    }

    match registry.update(existing_config.clone()) {
        Ok(_) => pgrx::JsonB(serde_json::json!({
            "success": true,
            "collection": collection,
            "config": {
                "fusion_method": format!("{:?}", existing_config.fusion_config.method),
                "alpha": existing_config.fusion_config.alpha,
                "rrf_k": existing_config.fusion_config.rrf_k,
                "prefetch_k": existing_config.prefetch_k,
                "bm25_k1": existing_config.bm25_config.k1,
                "bm25_b": existing_config.bm25_config.b,
                "stats_refresh_interval": existing_config.stats_refresh_interval,
                "parallel_enabled": existing_config.parallel_enabled
            }
        })),
        Err(e) => pgrx::JsonB(serde_json::json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

/// Perform hybrid search combining vector and keyword search
///
/// # Arguments
/// * `collection` - Table name
/// * `query_text` - Text query for keyword search
/// * `query_vector` - Vector for semantic search
/// * `k` - Number of results to return
/// * `fusion` - Fusion method ("rrf", "linear", "learned")
/// * `alpha` - Alpha for linear fusion (0-1, higher favors vector)
///
/// # Returns
/// Table of results with id, content, vector_score, keyword_score, hybrid_score
#[pg_extern]
fn ruvector_hybrid_search(
    collection: &str,
    query_text: &str,
    query_vector: Vec<f32>,
    k: i32,
    fusion: default!(Option<&str>, "NULL"),
    alpha: default!(Option<f32>, "NULL"),
) -> pgrx::JsonB {
    let k = k.max(1) as usize;
    let (schema, table) = parse_collection_name(collection);
    let qualified_name = format!("{}.{}", schema, table);

    let registry = get_registry();
    let config = match registry.get_by_name(&qualified_name) {
        Some(c) => c,
        None => {
            return pgrx::JsonB(serde_json::json!({
                "success": false,
                "error": format!("Collection '{}' is not registered for hybrid search. Run ruvector_register_hybrid first.", collection)
            }));
        }
    };

    // Build fusion config
    let mut fusion_config = config.fusion_config.clone();
    if let Some(method) = fusion {
        if let Ok(m) = method.parse::<FusionMethod>() {
            fusion_config.method = m;
        }
    }
    if let Some(a) = alpha {
        fusion_config.alpha = a.clamp(0.0, 1.0);
    }

    // Build query
    let query = HybridQuery {
        text: query_text.to_string(),
        embedding: query_vector,
        k,
        prefetch_k: config.prefetch_k.max(k * 2),
        fusion_config,
        filter: None,
    };

    // Create executor
    let executor = HybridExecutor::new(config.corpus_stats.clone());

    // In the actual extension, these would execute real searches via SPI
    // For now, return a demonstration response
    let mock_vector_results: Vec<(DocId, f32)> = (1..=k.min(10) as i64)
        .map(|i| (i, 0.1 * i as f32))
        .collect();

    let mock_keyword_results: Vec<(DocId, f32)> = (1..=k.min(10) as i64)
        .map(|i| ((k as i64 + 1 - i), 10.0 / i as f32))
        .collect();

    // Execute fusion
    let (results, stats) = executor.execute(
        &query,
        |_, k| BranchResults {
            results: mock_vector_results.clone().into_iter().take(k).collect(),
            latency_ms: 1.0,
            candidates_evaluated: 100,
        },
        |_, k| BranchResults {
            results: mock_keyword_results.clone().into_iter().take(k).collect(),
            latency_ms: 0.5,
            candidates_evaluated: 50,
        },
    );

    // Format results
    let result_json: Vec<serde_json::Value> = results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            serde_json::json!({
                "rank": i + 1,
                "id": r.id,
                "hybrid_score": r.hybrid_score,
                "vector_score": r.vector_score,
                "keyword_score": r.keyword_score,
                "vector_rank": r.vector_rank,
                "keyword_rank": r.keyword_rank
            })
        })
        .collect();

    pgrx::JsonB(serde_json::json!({
        "success": true,
        "collection": collection,
        "query": {
            "text": query_text,
            "vector_dims": query.embedding.len(),
            "k": k,
            "fusion": format!("{:?}", query.fusion_config.method),
            "alpha": query.fusion_config.alpha
        },
        "results": result_json,
        "stats": {
            "total_latency_ms": stats.total_latency_ms,
            "vector_latency_ms": stats.vector_latency_ms,
            "keyword_latency_ms": stats.keyword_latency_ms,
            "fusion_latency_ms": stats.fusion_latency_ms,
            "result_count": stats.result_count
        },
        "note": "This is a demonstration. In production, actual vector/keyword searches would be executed via SPI."
    }))
}

/// Get hybrid search statistics for a collection
#[pg_extern]
fn ruvector_hybrid_stats(collection: &str) -> pgrx::JsonB {
    let (schema, table) = parse_collection_name(collection);
    let qualified_name = format!("{}.{}", schema, table);

    let registry = get_registry();
    match registry.get_by_name(&qualified_name) {
        Some(config) => pgrx::JsonB(serde_json::json!({
            "collection": collection,
            "corpus_stats": {
                "avg_doc_length": config.corpus_stats.avg_doc_length,
                "doc_count": config.corpus_stats.doc_count,
                "total_terms": config.corpus_stats.total_terms,
                "last_update": config.corpus_stats.last_update
            },
            "bm25_config": {
                "k1": config.bm25_config.k1,
                "b": config.bm25_config.b
            },
            "fusion_config": {
                "method": format!("{:?}", config.fusion_config.method),
                "alpha": config.fusion_config.alpha,
                "rrf_k": config.fusion_config.rrf_k
            },
            "settings": {
                "prefetch_k": config.prefetch_k,
                "parallel_enabled": config.parallel_enabled,
                "stats_refresh_interval": config.stats_refresh_interval
            },
            "metadata": {
                "vector_column": config.vector_column,
                "fts_column": config.fts_column,
                "text_column": config.text_column,
                "created_at": config.created_at,
                "updated_at": config.updated_at
            }
        })),
        None => pgrx::JsonB(serde_json::json!({
            "error": format!("Collection '{}' is not registered for hybrid search", collection)
        })),
    }
}

/// Compute hybrid score from vector distance and keyword score
///
/// Utility function for manual hybrid scoring in queries.
#[pg_extern(immutable, parallel_safe)]
fn ruvector_hybrid_score(
    vector_distance: f32,
    keyword_score: f32,
    alpha: default!(Option<f32>, "0.5"),
) -> f32 {
    let alpha = alpha.unwrap_or(0.5).clamp(0.0, 1.0);

    // Convert distance to similarity (assuming cosine distance in [0, 2])
    let vector_similarity = 1.0 - (vector_distance / 2.0).clamp(0.0, 1.0);

    // Simple linear blend (normalized keyword scores assumed)
    alpha * vector_similarity + (1.0 - alpha) * keyword_score
}

/// List all collections registered for hybrid search
#[pg_extern]
fn ruvector_hybrid_list() -> pgrx::JsonB {
    let registry = get_registry();
    let collections: Vec<serde_json::Value> = registry
        .list()
        .iter()
        .map(|c| {
            serde_json::json!({
                "collection_id": c.collection_id,
                "name": c.qualified_name(),
                "vector_column": c.vector_column,
                "fts_column": c.fts_column,
                "fusion_method": format!("{:?}", c.fusion_config.method),
                "doc_count": c.corpus_stats.doc_count,
                "needs_refresh": c.needs_stats_refresh()
            })
        })
        .collect();

    pgrx::JsonB(serde_json::json!({
        "count": collections.len(),
        "collections": collections
    }))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse collection name into schema and table
fn parse_collection_name(name: &str) -> (&str, &str) {
    if let Some((schema, table)) = name.split_once('.') {
        (schema, table)
    } else {
        ("public", name)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_collection_name() {
        let (schema, table) = parse_collection_name("documents");
        assert_eq!(schema, "public");
        assert_eq!(table, "documents");

        let (schema, table) = parse_collection_name("myschema.mytable");
        assert_eq!(schema, "myschema");
        assert_eq!(table, "mytable");
    }

    #[test]
    fn test_module_exports() {
        // Verify all expected types are accessible
        let _ = BM25Config::default();
        let _ = FusionConfig::default();
        let _ = CorpusStats::default();

        let stats = CorpusStats {
            avg_doc_length: 100.0,
            doc_count: 1000,
            total_terms: 100000,
            last_update: 0,
        };
        let _ = BM25Scorer::new(stats.clone());
        let _ = HybridExecutor::new(stats);
    }

    #[test]
    fn test_registry_workflow() {
        let registry = HybridRegistry::new();

        // Register
        let config = HybridCollectionConfig::new(
            1,
            "test_table".to_string(),
            "embedding".to_string(),
            "fts".to_string(),
            "content".to_string(),
        );
        registry.register(config).unwrap();

        // Get
        let retrieved = registry.get(1).unwrap();
        assert_eq!(retrieved.table_name, "test_table");

        // List
        let list = registry.list();
        assert_eq!(list.len(), 1);
    }
}
