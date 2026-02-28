//! BM25 (Best Matching 25) scoring implementation
//!
//! Provides proper BM25 scoring with:
//! - Document length normalization
//! - IDF weighting across corpus
//! - Term frequency saturation
//! - Configurable k1 and b parameters
//!
//! Unlike PostgreSQL's ts_rank, this is a proper BM25 implementation.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Default BM25 k1 parameter (term frequency saturation)
pub const DEFAULT_K1: f32 = 1.2;

/// Default BM25 b parameter (length normalization)
pub const DEFAULT_B: f32 = 0.75;

/// Corpus statistics for BM25 scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusStats {
    /// Average document length in the corpus
    pub avg_doc_length: f32,
    /// Total number of documents
    pub doc_count: u64,
    /// Total number of terms across all documents
    pub total_terms: u64,
    /// Last update timestamp (Unix epoch seconds)
    pub last_update: i64,
}

impl Default for CorpusStats {
    fn default() -> Self {
        Self {
            avg_doc_length: 0.0,
            doc_count: 0,
            total_terms: 0,
            last_update: 0,
        }
    }
}

/// BM25 configuration parameters
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BM25Config {
    /// Term frequency saturation parameter (default: 1.2)
    /// Higher values give more weight to term frequency
    pub k1: f32,
    /// Length normalization parameter (default: 0.75)
    /// 0 = no length normalization, 1 = full normalization
    pub b: f32,
}

impl Default for BM25Config {
    fn default() -> Self {
        Self {
            k1: DEFAULT_K1,
            b: DEFAULT_B,
        }
    }
}

impl BM25Config {
    /// Create a new BM25 configuration
    pub fn new(k1: f32, b: f32) -> Self {
        Self {
            k1: k1.max(0.0),
            b: b.clamp(0.0, 1.0),
        }
    }
}

/// Term frequency information for a document
#[derive(Debug, Clone)]
pub struct TermFrequencies {
    /// Term -> frequency map
    pub frequencies: HashMap<String, u32>,
    /// Total terms in document
    pub doc_length: u32,
}

impl TermFrequencies {
    /// Create from term frequency map
    pub fn new(frequencies: HashMap<String, u32>) -> Self {
        let doc_length = frequencies.values().sum();
        Self {
            frequencies,
            doc_length,
        }
    }

    /// Get term frequency for a specific term
    pub fn get(&self, term: &str) -> Option<u32> {
        self.frequencies.get(term).copied()
    }
}

/// Document information for BM25 scoring
pub struct Document<'a> {
    /// Term frequencies in the document
    pub term_freqs: &'a TermFrequencies,
}

impl<'a> Document<'a> {
    /// Create a new document wrapper
    pub fn new(term_freqs: &'a TermFrequencies) -> Self {
        Self { term_freqs }
    }

    /// Get term frequency for a term
    pub fn term_freq(&self, term: &str) -> Option<u32> {
        self.term_freqs.get(term)
    }

    /// Get document length (total terms)
    pub fn term_count(&self) -> u32 {
        self.term_freqs.doc_length
    }
}

/// BM25 scorer with corpus statistics and IDF caching
pub struct BM25Scorer {
    /// Configuration parameters
    config: BM25Config,
    /// Corpus statistics
    corpus_stats: CorpusStats,
    /// Cached IDF values (term -> IDF score)
    idf_cache: Arc<RwLock<HashMap<String, f32>>>,
    /// Document frequency cache (term -> doc count containing term)
    df_cache: Arc<RwLock<HashMap<String, u64>>>,
}

impl BM25Scorer {
    /// Create a new BM25 scorer with default config
    pub fn new(corpus_stats: CorpusStats) -> Self {
        Self::with_config(corpus_stats, BM25Config::default())
    }

    /// Create a new BM25 scorer with custom config
    pub fn with_config(corpus_stats: CorpusStats, config: BM25Config) -> Self {
        Self {
            config,
            corpus_stats,
            idf_cache: Arc::new(RwLock::new(HashMap::new())),
            df_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update corpus statistics
    pub fn update_corpus_stats(&mut self, stats: CorpusStats) {
        self.corpus_stats = stats;
        // Clear caches when stats change
        self.idf_cache.write().clear();
    }

    /// Set document frequency for a term (used during index building)
    pub fn set_doc_freq(&self, term: &str, doc_freq: u64) {
        self.df_cache.write().insert(term.to_string(), doc_freq);
        // Invalidate IDF cache for this term
        self.idf_cache.write().remove(term);
    }

    /// Compute IDF (Inverse Document Frequency) for a term
    ///
    /// Uses the BM25 IDF formula:
    /// IDF(t) = ln((N - df(t) + 0.5) / (df(t) + 0.5) + 1)
    ///
    /// where:
    /// - N = total documents in corpus
    /// - df(t) = number of documents containing term t
    pub fn idf(&self, term: &str) -> f32 {
        // Check cache first
        if let Some(&cached) = self.idf_cache.read().get(term) {
            return cached;
        }

        // Get document frequency
        let df = self.df_cache.read().get(term).copied().unwrap_or(0);

        // Compute IDF using BM25 formula
        let n = self.corpus_stats.doc_count as f32;
        let df_f = df as f32;

        // Prevent division by zero and handle edge cases
        let idf = if df == 0 {
            // Term not in corpus - use max IDF
            (n + 0.5).ln()
        } else {
            ((n - df_f + 0.5) / (df_f + 0.5) + 1.0).ln()
        };

        // Cache the result
        self.idf_cache.write().insert(term.to_string(), idf);

        idf
    }

    /// Compute IDF with known document frequency (bypasses cache lookup)
    pub fn idf_with_df(&self, doc_freq: u64) -> f32 {
        let n = self.corpus_stats.doc_count as f32;
        let df = doc_freq as f32;

        if doc_freq == 0 {
            (n + 0.5).ln()
        } else {
            ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
        }
    }

    /// Score a document for a query
    ///
    /// BM25 formula:
    /// score(D, Q) = sum over t in Q of: IDF(t) * (tf(t,D) * (k1 + 1)) / (tf(t,D) + k1 * (1 - b + b * |D|/avgdl))
    ///
    /// where:
    /// - tf(t,D) = term frequency of t in document D
    /// - |D| = document length
    /// - avgdl = average document length
    /// - k1 = term saturation parameter
    /// - b = length normalization parameter
    pub fn score(&self, doc: &Document, query_terms: &[String]) -> f32 {
        let doc_len = doc.term_count() as f32;
        let avg_doc_len = self.corpus_stats.avg_doc_length.max(1.0);

        // Length normalization factor
        let len_norm = 1.0 - self.config.b + self.config.b * (doc_len / avg_doc_len);

        query_terms
            .iter()
            .filter_map(|term| {
                let tf = doc.term_freq(term)? as f32;
                let idf = self.idf(term);

                // BM25 term score
                let numerator = tf * (self.config.k1 + 1.0);
                let denominator = tf + self.config.k1 * len_norm;

                Some(idf * numerator / denominator)
            })
            .sum()
    }

    /// Score a document with pre-computed term frequencies and document frequencies
    ///
    /// This is the optimized version for batch scoring where IDF values are known.
    pub fn score_with_freqs(
        &self,
        term_freqs: &[(String, u32, u64)], // (term, tf in doc, df in corpus)
        doc_length: u32,
    ) -> f32 {
        let doc_len = doc_length as f32;
        let avg_doc_len = self.corpus_stats.avg_doc_length.max(1.0);

        let len_norm = 1.0 - self.config.b + self.config.b * (doc_len / avg_doc_len);

        term_freqs
            .iter()
            .map(|(_, tf, df)| {
                let tf = *tf as f32;
                let idf = self.idf_with_df(*df);

                let numerator = tf * (self.config.k1 + 1.0);
                let denominator = tf + self.config.k1 * len_norm;

                idf * numerator / denominator
            })
            .sum()
    }

    /// Batch score multiple documents for the same query
    pub fn score_batch<'a>(
        &self,
        docs: impl Iterator<Item = &'a Document<'a>>,
        query_terms: &[String],
    ) -> Vec<f32> {
        docs.map(|doc| self.score(doc, query_terms)).collect()
    }

    /// Get current configuration
    pub fn config(&self) -> &BM25Config {
        &self.config
    }

    /// Get corpus statistics
    pub fn corpus_stats(&self) -> &CorpusStats {
        &self.corpus_stats
    }

    /// Clear IDF cache
    pub fn clear_cache(&self) {
        self.idf_cache.write().clear();
        self.df_cache.write().clear();
    }
}

/// Simple query tokenizer for BM25
///
/// Note: In production, this should use PostgreSQL's text search configuration
/// for proper stemming, stopword removal, etc.
pub fn tokenize_query(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split_whitespace()
        .filter(|s| s.len() > 1) // Skip single characters
        .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Parse tsvector-style string to term frequencies
pub fn parse_tsvector(tsvector_str: &str) -> HashMap<String, u32> {
    let mut frequencies = HashMap::new();

    // Format: 'term1':1,2,3 'term2':4,5
    for part in tsvector_str.split_whitespace() {
        if let Some(quote_end) = part.find("':") {
            if part.starts_with('\'') {
                let term = &part[1..quote_end];
                let positions = &part[quote_end + 2..];
                // Count positions as frequency
                let freq = positions.split(',').count() as u32;
                frequencies.insert(term.to_string(), freq.max(1));
            }
        } else if part.starts_with('\'') && part.ends_with('\'') {
            // Term without positions
            let term = &part[1..part.len() - 1];
            frequencies.insert(term.to_string(), 1);
        }
    }

    frequencies
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_scorer() -> BM25Scorer {
        let stats = CorpusStats {
            avg_doc_length: 100.0,
            doc_count: 1000,
            total_terms: 100000,
            last_update: 0,
        };
        BM25Scorer::new(stats)
    }

    #[test]
    fn test_idf_common_term() {
        let scorer = create_test_scorer();
        scorer.set_doc_freq("the", 900); // Very common term

        let idf = scorer.idf("the");
        assert!(idf > 0.0, "IDF should be positive");
        assert!(idf < 1.0, "IDF for common term should be low");
    }

    #[test]
    fn test_idf_rare_term() {
        let scorer = create_test_scorer();
        scorer.set_doc_freq("xyzzy", 5); // Rare term

        let idf = scorer.idf("xyzzy");
        assert!(idf > 4.0, "IDF for rare term should be high");
    }

    #[test]
    fn test_idf_unknown_term() {
        let scorer = create_test_scorer();

        let idf = scorer.idf("unknown_term_xyz");
        assert!(idf > 5.0, "IDF for unknown term should be maximum");
    }

    #[test]
    fn test_bm25_score() {
        let scorer = create_test_scorer();
        scorer.set_doc_freq("database", 100);
        scorer.set_doc_freq("query", 50);

        let mut freqs = HashMap::new();
        freqs.insert("database".to_string(), 3);
        freqs.insert("query".to_string(), 2);
        freqs.insert("other".to_string(), 5);

        let term_freqs = TermFrequencies::new(freqs);
        let doc = Document::new(&term_freqs);

        let query_terms = vec!["database".to_string(), "query".to_string()];
        let score = scorer.score(&doc, &query_terms);

        assert!(score > 0.0, "Score should be positive");
    }

    #[test]
    fn test_length_normalization() {
        let scorer = create_test_scorer();
        scorer.set_doc_freq("test", 100);

        // Short document (50 terms)
        let mut short_freqs = HashMap::new();
        short_freqs.insert("test".to_string(), 2);
        for i in 0..48 {
            short_freqs.insert(format!("filler{}", i), 1);
        }
        let short_tf = TermFrequencies::new(short_freqs);
        let short_doc = Document::new(&short_tf);

        // Long document (200 terms)
        let mut long_freqs = HashMap::new();
        long_freqs.insert("test".to_string(), 2);
        for i in 0..198 {
            long_freqs.insert(format!("filler{}", i), 1);
        }
        let long_tf = TermFrequencies::new(long_freqs);
        let long_doc = Document::new(&long_tf);

        let query_terms = vec!["test".to_string()];
        let short_score = scorer.score(&short_doc, &query_terms);
        let long_score = scorer.score(&long_doc, &query_terms);

        // Short doc should score higher (same tf, less length penalty)
        assert!(
            short_score > long_score,
            "Short doc should score higher than long doc with same TF"
        );
    }

    #[test]
    fn test_tokenize_query() {
        let tokens = tokenize_query("Hello World! Database Query.");
        assert_eq!(tokens, vec!["hello", "world", "database", "query"]);
    }

    #[test]
    fn test_parse_tsvector() {
        let tsvector = "'database':1,3,5 'query':2,4";
        let freqs = parse_tsvector(tsvector);

        assert_eq!(freqs.get("database"), Some(&3));
        assert_eq!(freqs.get("query"), Some(&2));
    }

    #[test]
    fn test_config_clamping() {
        let config = BM25Config::new(-1.0, 1.5);
        assert_eq!(config.k1, 0.0, "k1 should be clamped to 0");
        assert_eq!(config.b, 1.0, "b should be clamped to 1");
    }
}
