//! Comprehensive tests for the hybrid search module
//!
//! Tests cover:
//! - BM25 scoring correctness
//! - Fusion algorithm behavior
//! - Executor integration
//! - Registry operations

#[cfg(test)]
mod bm25_tests {
    use crate::hybrid::bm25::*;
    use std::collections::HashMap;

    /// Create a test scorer with known corpus statistics
    fn test_scorer() -> BM25Scorer {
        let stats = CorpusStats {
            avg_doc_length: 100.0,
            doc_count: 10000,
            total_terms: 1000000,
            last_update: 0,
        };
        BM25Scorer::new(stats)
    }

    #[test]
    fn test_bm25_idf_formula() {
        let scorer = test_scorer();

        // Set known document frequencies
        scorer.set_doc_freq("common", 5000);  // 50% of docs
        scorer.set_doc_freq("rare", 10);       // 0.1% of docs
        scorer.set_doc_freq("unique", 1);      // 0.01% of docs

        let idf_common = scorer.idf("common");
        let idf_rare = scorer.idf("rare");
        let idf_unique = scorer.idf("unique");

        // IDF should increase as term rarity increases
        assert!(idf_common < idf_rare, "Common term should have lower IDF");
        assert!(idf_rare < idf_unique, "Rare term should have lower IDF than unique");

        // Verify approximate values using BM25 formula
        // IDF = ln((N - df + 0.5) / (df + 0.5) + 1)
        // For common (df=5000, N=10000): ln((10000-5000+0.5)/(5000+0.5)+1) ~= ln(2) ~= 0.69
        assert!((idf_common - 0.69).abs() < 0.1, "IDF common: {}", idf_common);
    }

    #[test]
    fn test_bm25_score_single_term() {
        let scorer = test_scorer();
        scorer.set_doc_freq("test", 1000); // 10% of docs

        let mut freqs = HashMap::new();
        freqs.insert("test".to_string(), 5); // Term appears 5 times
        let term_freqs = TermFrequencies::new(freqs);
        let doc = Document::new(&term_freqs);

        let score = scorer.score(&doc, &["test".to_string()]);

        assert!(score > 0.0, "Score should be positive");
    }

    #[test]
    fn test_bm25_score_multiple_terms() {
        let scorer = test_scorer();
        scorer.set_doc_freq("database", 500);
        scorer.set_doc_freq("query", 300);
        scorer.set_doc_freq("optimization", 100);

        let mut freqs = HashMap::new();
        freqs.insert("database".to_string(), 3);
        freqs.insert("query".to_string(), 2);
        freqs.insert("optimization".to_string(), 1);
        let term_freqs = TermFrequencies::new(freqs);
        let doc = Document::new(&term_freqs);

        let query_terms = vec![
            "database".to_string(),
            "query".to_string(),
            "optimization".to_string(),
        ];

        let score = scorer.score(&doc, &query_terms);
        assert!(score > 0.0);

        // Score with subset should be lower
        let partial_score = scorer.score(&doc, &["database".to_string()]);
        assert!(partial_score < score, "Partial match should score lower");
    }

    #[test]
    fn test_bm25_length_normalization() {
        let scorer = test_scorer();
        scorer.set_doc_freq("keyword", 1000);

        // Create two documents with same TF but different lengths
        // Short doc: 50 terms, avg is 100
        let mut short_freqs = HashMap::new();
        short_freqs.insert("keyword".to_string(), 2);
        for i in 0..48 {
            short_freqs.insert(format!("other{}", i), 1);
        }
        let short_tf = TermFrequencies::new(short_freqs);
        let short_doc = Document::new(&short_tf);

        // Long doc: 200 terms, avg is 100
        let mut long_freqs = HashMap::new();
        long_freqs.insert("keyword".to_string(), 2);
        for i in 0..198 {
            long_freqs.insert(format!("other{}", i), 1);
        }
        let long_tf = TermFrequencies::new(long_freqs);
        let long_doc = Document::new(&long_tf);

        let query = vec!["keyword".to_string()];
        let short_score = scorer.score(&short_doc, &query);
        let long_score = scorer.score(&long_doc, &query);

        // Short doc should score higher due to length normalization
        assert!(short_score > long_score,
            "Short doc ({}) should score higher than long doc ({})",
            short_score, long_score
        );
    }

    #[test]
    fn test_bm25_tf_saturation() {
        let scorer = test_scorer();
        scorer.set_doc_freq("term", 500);

        // Document with low TF
        let mut low_tf_freqs = HashMap::new();
        low_tf_freqs.insert("term".to_string(), 1);
        let low_tf = TermFrequencies::new(low_tf_freqs);
        let low_doc = Document::new(&low_tf);

        // Document with high TF
        let mut high_tf_freqs = HashMap::new();
        high_tf_freqs.insert("term".to_string(), 100);
        let high_tf = TermFrequencies::new(high_tf_freqs);
        let high_doc = Document::new(&high_tf);

        let query = vec!["term".to_string()];
        let low_score = scorer.score(&low_doc, &query);
        let high_score = scorer.score(&high_doc, &query);

        // High TF should score higher, but not 100x higher (saturation)
        assert!(high_score > low_score);
        assert!(high_score < low_score * 10.0,
            "TF saturation should prevent linear scaling: {} vs {}",
            high_score, low_score
        );
    }

    #[test]
    fn test_bm25_config_params() {
        let stats = CorpusStats {
            avg_doc_length: 100.0,
            doc_count: 1000,
            total_terms: 100000,
            last_update: 0,
        };

        // High k1 = more weight to term frequency
        let high_k1 = BM25Config::new(2.0, 0.75);
        let scorer_high_k1 = BM25Scorer::with_config(stats.clone(), high_k1);

        // Low k1 = less weight to term frequency
        let low_k1 = BM25Config::new(0.5, 0.75);
        let scorer_low_k1 = BM25Scorer::with_config(stats, low_k1);

        scorer_high_k1.set_doc_freq("test", 100);
        scorer_low_k1.set_doc_freq("test", 100);

        let mut freqs = HashMap::new();
        freqs.insert("test".to_string(), 10);
        let tf = TermFrequencies::new(freqs);
        let doc = Document::new(&tf);
        let query = vec!["test".to_string()];

        let score_high = scorer_high_k1.score(&doc, &query);
        let score_low = scorer_low_k1.score(&doc, &query);

        // Different k1 should produce different scores
        assert!((score_high - score_low).abs() > 0.1,
            "k1 should affect scoring: {} vs {}", score_high, score_low
        );
    }

    #[test]
    fn test_tokenize_query() {
        let tokens = tokenize_query("Hello, World! This is a TEST.");
        assert_eq!(tokens, vec!["hello", "world", "this", "is", "test"]);

        let tokens2 = tokenize_query("database-query optimization");
        assert!(tokens2.contains(&"database-query".to_string()) || tokens2.contains(&"database".to_string()));
    }

    #[test]
    fn test_parse_tsvector() {
        let tsvector = "'databas':1,4,7 'queri':2,5 'optim':3";
        let freqs = parse_tsvector(tsvector);

        assert_eq!(freqs.get("databas"), Some(&3));
        assert_eq!(freqs.get("queri"), Some(&2));
        assert_eq!(freqs.get("optim"), Some(&1));
    }
}

#[cfg(test)]
mod fusion_tests {
    use crate::hybrid::fusion::*;

    fn sample_vector_results() -> Vec<(DocId, f32)> {
        // Lower distance = better
        vec![
            (1, 0.1),
            (2, 0.15),
            (3, 0.25),
            (4, 0.4),
            (5, 0.6),
        ]
    }

    fn sample_keyword_results() -> Vec<(DocId, f32)> {
        // Higher BM25 = better
        vec![
            (3, 9.5),
            (6, 8.0),
            (1, 7.2),
            (7, 5.0),
            (2, 3.5),
        ]
    }

    #[test]
    fn test_rrf_basic() {
        let vector = sample_vector_results();
        let keyword = sample_keyword_results();

        let results = rrf_fusion(&vector, &keyword, 60, 10);

        assert!(!results.is_empty());
        // Doc 1 and 3 appear in both, should rank highly
        let top_3_ids: Vec<DocId> = results.iter().take(3).map(|r| r.doc_id).collect();
        assert!(top_3_ids.contains(&1) || top_3_ids.contains(&3));
    }

    #[test]
    fn test_rrf_k_parameter() {
        let vector = sample_vector_results();
        let keyword = sample_keyword_results();

        // Low k = top ranks matter more
        let results_low_k = rrf_fusion(&vector, &keyword, 10, 5);

        // High k = ranks matter less
        let results_high_k = rrf_fusion(&vector, &keyword, 100, 5);

        // Both should produce results
        assert!(!results_low_k.is_empty());
        assert!(!results_high_k.is_empty());

        // Order might differ due to k
        let order_low: Vec<DocId> = results_low_k.iter().map(|r| r.doc_id).collect();
        let order_high: Vec<DocId> = results_high_k.iter().map(|r| r.doc_id).collect();

        // At least verify both have same elements (possibly different order)
        for id in &order_low {
            assert!(order_high.contains(id) || order_low.len() > order_high.len());
        }
    }

    #[test]
    fn test_linear_fusion_alpha() {
        let vector = sample_vector_results();
        let keyword = sample_keyword_results();

        // Alpha = 1.0 means only vector
        let results_vector_only = linear_fusion(&vector, &keyword, 1.0, 5);
        // Alpha = 0.0 means only keyword
        let results_keyword_only = linear_fusion(&vector, &keyword, 0.0, 5);

        // With alpha=1, best vector result (id=1) should be top
        assert_eq!(results_vector_only[0].doc_id, 1);

        // With alpha=0, best keyword result (id=3) should be top
        assert_eq!(results_keyword_only[0].doc_id, 3);
    }

    #[test]
    fn test_linear_fusion_balanced() {
        let vector = sample_vector_results();
        let keyword = sample_keyword_results();

        let results = linear_fusion(&vector, &keyword, 0.5, 5);

        // All results should have both scores if they appeared in both
        for r in &results {
            assert!(r.hybrid_score > 0.0);
        }
    }

    #[test]
    fn test_fusion_preserves_scores() {
        let vector = vec![(1, 0.1), (2, 0.2)];
        let keyword = vec![(1, 5.0), (3, 4.0)];

        let results = rrf_fusion(&vector, &keyword, 60, 10);

        let doc1 = results.iter().find(|r| r.doc_id == 1).unwrap();
        assert!(doc1.vector_score.is_some());
        assert!(doc1.keyword_score.is_some());

        let doc2 = results.iter().find(|r| r.doc_id == 2).unwrap();
        assert!(doc2.vector_score.is_some());
        assert!(doc2.keyword_score.is_none());

        let doc3 = results.iter().find(|r| r.doc_id == 3).unwrap();
        assert!(doc3.vector_score.is_none());
        assert!(doc3.keyword_score.is_some());
    }

    #[test]
    fn test_fusion_method_parse() {
        assert_eq!("rrf".parse::<FusionMethod>().unwrap(), FusionMethod::Rrf);
        assert_eq!("linear".parse::<FusionMethod>().unwrap(), FusionMethod::Linear);
        assert_eq!("learned".parse::<FusionMethod>().unwrap(), FusionMethod::Learned);
        assert!("invalid".parse::<FusionMethod>().is_err());
    }

    #[test]
    fn test_query_type_classification() {
        let nav = classify_query_type(&["github".into(), "login".into()]);
        assert_eq!(nav, QueryType::Navigational);

        let info = classify_query_type(&["how".into(), "to".into(), "build".into()]);
        assert_eq!(info, QueryType::Informational);

        let trans = classify_query_type(&["buy".into(), "cheap".into(), "laptop".into()]);
        assert_eq!(trans, QueryType::Transactional);
    }

    #[test]
    fn test_fusion_model() {
        let model = FusionModel::default();

        // Test navigational query (should favor keyword)
        let nav_features = QueryFeatures {
            embedding_norm: 1.0,
            term_count: 2,
            avg_term_idf: 2.0,
            has_exact_match: true,
            query_type: QueryType::Navigational,
        };
        let nav_alpha = model.predict_alpha(&nav_features);
        assert!(nav_alpha < 0.5, "Nav query should favor keyword");

        // Test informational query (should favor vector)
        let info_features = QueryFeatures {
            embedding_norm: 1.2,
            term_count: 5,
            avg_term_idf: 4.5,
            has_exact_match: false,
            query_type: QueryType::Informational,
        };
        let info_alpha = model.predict_alpha(&info_features);
        assert!(info_alpha > 0.4, "Info query should favor vector");
    }
}

#[cfg(test)]
mod executor_tests {
    use crate::hybrid::executor::*;
    use crate::hybrid::fusion::*;
    use crate::hybrid::bm25::CorpusStats;

    fn mock_corpus_stats() -> CorpusStats {
        CorpusStats {
            avg_doc_length: 150.0,
            doc_count: 5000,
            total_terms: 750000,
            last_update: 0,
        }
    }

    fn mock_vector_search(_embedding: &[f32], k: usize) -> BranchResults {
        BranchResults {
            results: (1..=k.min(10) as i64)
                .map(|i| (i, 0.05 * i as f32))
                .collect(),
            latency_ms: 2.5,
            candidates_evaluated: 500,
        }
    }

    fn mock_keyword_search(_text: &str, k: usize) -> BranchResults {
        BranchResults {
            results: (1..=k.min(10) as i64)
                .map(|i| (10 - i + 1, 12.0 - i as f32))
                .collect(),
            latency_ms: 1.2,
            candidates_evaluated: 200,
        }
    }

    #[test]
    fn test_hybrid_query_builder() {
        let query = HybridQuery::new("test query".into(), vec![0.1; 128], 10)
            .with_fusion(FusionMethod::Linear)
            .with_alpha(0.7)
            .with_prefetch(200)
            .with_rrf_k(40);

        assert_eq!(query.k, 10);
        assert_eq!(query.prefetch_k, 200);
        assert_eq!(query.fusion_config.method, FusionMethod::Linear);
        assert!((query.fusion_config.alpha - 0.7).abs() < 0.01);
        assert_eq!(query.fusion_config.rrf_k, 40);
    }

    #[test]
    fn test_executor_execute() {
        let executor = HybridExecutor::new(mock_corpus_stats());

        let query = HybridQuery::new(
            "database optimization".into(),
            vec![0.1; 64],
            5,
        );

        let (results, stats) = executor.execute(&query, mock_vector_search, mock_keyword_search);

        assert!(!results.is_empty());
        assert!(results.len() <= 5);

        // Check stats
        assert!(stats.total_latency_ms > 0.0);
        assert!(stats.vector_latency_ms > 0.0);
        assert!(stats.keyword_latency_ms > 0.0);
        assert!(stats.result_count <= 5);
    }

    #[test]
    fn test_executor_with_different_fusion() {
        let executor = HybridExecutor::new(mock_corpus_stats());

        // RRF
        let query_rrf = HybridQuery::new("test".into(), vec![0.1; 32], 5)
            .with_fusion(FusionMethod::Rrf);
        let (results_rrf, _) = executor.execute(&query_rrf, mock_vector_search, mock_keyword_search);

        // Linear
        let query_linear = HybridQuery::new("test".into(), vec![0.1; 32], 5)
            .with_fusion(FusionMethod::Linear)
            .with_alpha(0.5);
        let (results_linear, _) = executor.execute(&query_linear, mock_vector_search, mock_keyword_search);

        assert!(!results_rrf.is_empty());
        assert!(!results_linear.is_empty());
    }

    #[test]
    fn test_strategy_selection() {
        // No filter
        assert_eq!(choose_strategy(None, 10000, false), HybridStrategy::Full);

        // Very selective filter
        assert_eq!(choose_strategy(Some(0.005), 1000000, true), HybridStrategy::PreFilter);

        // Moderate selectivity, large corpus
        assert_eq!(choose_strategy(Some(0.05), 5000000, true), HybridStrategy::PostFilter);

        // Low selectivity
        assert_eq!(choose_strategy(Some(0.5), 10000, true), HybridStrategy::Full);
    }

    #[test]
    fn test_result_has_ranks() {
        let executor = HybridExecutor::new(mock_corpus_stats());

        let query = HybridQuery::new("test".into(), vec![0.1; 16], 10);
        let (results, _) = executor.execute(&query, mock_vector_search, mock_keyword_search);

        // Check that rank information is populated
        for r in &results {
            // At least one rank should be present (doc appeared in at least one branch)
            assert!(r.vector_rank.is_some() || r.keyword_rank.is_some());
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use crate::hybrid::registry::*;
    use crate::hybrid::bm25::CorpusStats;
    use crate::hybrid::fusion::FusionMethod;

    #[test]
    fn test_registry_lifecycle() {
        let registry = HybridRegistry::new();

        // Register
        let config = HybridCollectionConfig::new(
            1,
            "test_collection".to_string(),
            "embedding".to_string(),
            "fts".to_string(),
            "content".to_string(),
        );
        assert!(registry.register(config).is_ok());

        // Get by ID
        let retrieved = registry.get(1).unwrap();
        assert_eq!(retrieved.table_name, "test_collection");

        // Get by name
        let by_name = registry.get_by_name("public.test_collection").unwrap();
        assert_eq!(by_name.collection_id, 1);

        // List
        let list = registry.list();
        assert_eq!(list.len(), 1);

        // Unregister
        assert!(registry.unregister(1).is_ok());
        assert!(registry.get(1).is_none());
    }

    #[test]
    fn test_registry_duplicate_prevention() {
        let registry = HybridRegistry::new();

        let config = HybridCollectionConfig::new(
            1,
            "unique_table".to_string(),
            "vec".to_string(),
            "fts".to_string(),
            "text".to_string(),
        );

        registry.register(config.clone()).unwrap();
        let result = registry.register(config);

        assert!(matches!(result, Err(RegistryError::AlreadyRegistered(_))));
    }

    #[test]
    fn test_registry_stats_update() {
        let registry = HybridRegistry::new();

        let config = HybridCollectionConfig::new(
            42,
            "stats_test".to_string(),
            "v".to_string(),
            "f".to_string(),
            "t".to_string(),
        );
        registry.register(config).unwrap();

        let new_stats = CorpusStats {
            avg_doc_length: 200.0,
            doc_count: 10000,
            total_terms: 2000000,
            last_update: 12345,
        };

        registry.update_stats(42, new_stats).unwrap();

        let updated = registry.get(42).unwrap();
        assert!((updated.corpus_stats.avg_doc_length - 200.0).abs() < 0.1);
        assert_eq!(updated.corpus_stats.doc_count, 10000);
    }

    #[test]
    fn test_config_update_apply() {
        let mut config = HybridCollectionConfig::new(
            1,
            "test".to_string(),
            "v".to_string(),
            "f".to_string(),
            "t".to_string(),
        );

        let update = HybridConfigUpdate {
            default_fusion: Some("linear".to_string()),
            default_alpha: Some(0.8),
            rrf_k: Some(50),
            prefetch_k: Some(150),
            bm25_k1: Some(1.4),
            bm25_b: Some(0.7),
            stats_refresh_interval: Some("30 minutes".to_string()),
            parallel_enabled: Some(false),
        };

        update.apply(&mut config).unwrap();

        assert_eq!(config.fusion_config.method, FusionMethod::Linear);
        assert!((config.fusion_config.alpha - 0.8).abs() < 0.01);
        assert_eq!(config.fusion_config.rrf_k, 50);
        assert_eq!(config.prefetch_k, 150);
        assert!((config.bm25_config.k1 - 1.4).abs() < 0.01);
        assert!((config.bm25_config.b - 0.7).abs() < 0.01);
        assert_eq!(config.stats_refresh_interval, 1800);
        assert!(!config.parallel_enabled);
    }

    #[test]
    fn test_config_update_validation() {
        let mut config = HybridCollectionConfig::new(
            1,
            "test".to_string(),
            "v".to_string(),
            "f".to_string(),
            "t".to_string(),
        );

        // Invalid alpha
        let invalid_alpha = HybridConfigUpdate {
            default_alpha: Some(1.5),
            ..Default::default()
        };
        assert!(invalid_alpha.apply(&mut config).is_err());

        // Invalid rrf_k
        let invalid_rrf = HybridConfigUpdate {
            rrf_k: Some(0),
            ..Default::default()
        };
        assert!(invalid_rrf.apply(&mut config).is_err());
    }

    #[test]
    fn test_idf_caching() {
        let registry = HybridRegistry::new();

        let mut config = HybridCollectionConfig::new(
            1,
            "idf_test".to_string(),
            "v".to_string(),
            "f".to_string(),
            "t".to_string(),
        );
        config.corpus_stats.doc_count = 1000;
        registry.register(config).unwrap();

        // Set doc freq
        registry.set_doc_freq(1, "test_term", 100).unwrap();

        // Get IDF (should compute and cache)
        let idf1 = registry.get_idf(1, "test_term").unwrap();
        assert!(idf1 > 0.0);

        // Get again (should use cache)
        let idf2 = registry.get_idf(1, "test_term").unwrap();
        assert!((idf1 - idf2).abs() < 0.001);
    }

    #[test]
    fn test_needs_refresh() {
        let mut config = HybridCollectionConfig::new(
            1,
            "refresh_test".to_string(),
            "v".to_string(),
            "f".to_string(),
            "t".to_string(),
        );

        // Set refresh interval to 1 hour
        config.stats_refresh_interval = 3600;

        // Fresh stats
        config.corpus_stats.last_update = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        assert!(!config.needs_stats_refresh());

        // Stale stats (2 hours old)
        config.corpus_stats.last_update -= 7200;
        assert!(config.needs_stats_refresh());
    }
}

#[cfg(test)]
mod integration_tests {
    use crate::hybrid::*;

    #[test]
    fn test_end_to_end_workflow() {
        // 1. Setup registry
        let registry = HybridRegistry::new();

        // 2. Register collection
        let config = HybridCollectionConfig::new(
            100,
            "documents".to_string(),
            "embedding".to_string(),
            "fts".to_string(),
            "content".to_string(),
        );
        registry.register(config).unwrap();

        // 3. Update with corpus stats
        let stats = CorpusStats {
            avg_doc_length: 250.0,
            doc_count: 50000,
            total_terms: 12500000,
            last_update: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        };
        registry.update_stats(100, stats).unwrap();

        // 4. Configure hybrid settings
        let config_update = HybridConfigUpdate {
            default_fusion: Some("rrf".to_string()),
            rrf_k: Some(60),
            prefetch_k: Some(200),
            ..Default::default()
        };

        let mut updated_config = registry.get(100).unwrap();
        config_update.apply(&mut updated_config).unwrap();
        registry.update(updated_config.clone()).unwrap();

        // 5. Create executor with updated config
        let executor = HybridExecutor::new(updated_config.corpus_stats);

        // 6. Execute query
        let query = HybridQuery::new(
            "machine learning optimization".to_string(),
            vec![0.1; 768],
            10,
        )
        .with_fusion(FusionMethod::Rrf)
        .with_prefetch(200);

        let mock_vector = |_: &[f32], k: usize| BranchResults {
            results: (1..=k.min(20) as i64).map(|i| (i, i as f32 * 0.02)).collect(),
            latency_ms: 3.0,
            candidates_evaluated: 1000,
        };

        let mock_keyword = |_: &str, k: usize| BranchResults {
            results: (1..=k.min(20) as i64).map(|i| (25 - i, 15.0 - i as f32 * 0.5)).collect(),
            latency_ms: 1.5,
            candidates_evaluated: 500,
        };

        let (results, stats) = executor.execute(&query, mock_vector, mock_keyword);

        // 7. Verify results
        assert!(!results.is_empty());
        assert!(results.len() <= 10);
        assert!(stats.total_latency_ms > 0.0);

        // Top result should have high hybrid score
        assert!(results[0].hybrid_score > 0.0);
    }

    #[test]
    fn test_bm25_scorer_integration() {
        let corpus_stats = CorpusStats {
            avg_doc_length: 100.0,
            doc_count: 1000,
            total_terms: 100000,
            last_update: 0,
        };

        let scorer = BM25Scorer::new(corpus_stats);

        // Set up document frequencies
        scorer.set_doc_freq("machine", 200);
        scorer.set_doc_freq("learning", 150);
        scorer.set_doc_freq("deep", 50);

        // Create test document
        let mut doc_freqs = std::collections::HashMap::new();
        doc_freqs.insert("machine".to_string(), 3);
        doc_freqs.insert("learning".to_string(), 2);
        doc_freqs.insert("deep".to_string(), 1);
        doc_freqs.insert("neural".to_string(), 2);
        doc_freqs.insert("networks".to_string(), 2);

        let term_freqs = TermFrequencies::new(doc_freqs);
        let doc = Document::new(&term_freqs);

        // Score with query
        let query_terms = vec![
            "machine".to_string(),
            "learning".to_string(),
            "deep".to_string(),
        ];

        let score = scorer.score(&doc, &query_terms);
        assert!(score > 0.0);

        // "deep" is rarer, so query with just "deep" should have higher IDF contribution
        let deep_idf = scorer.idf("deep");
        let machine_idf = scorer.idf("machine");
        assert!(deep_idf > machine_idf, "Rare term should have higher IDF");
    }
}
