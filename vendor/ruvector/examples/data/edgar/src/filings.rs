//! SEC filing types and analysis

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SEC filing types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FilingType {
    /// Annual report
    TenK,
    /// Quarterly report
    TenQ,
    /// Current report (material events)
    EightK,
    /// Proxy statement
    DefFourteen,
    /// Insider trading
    FormFour,
    /// Institutional holdings
    ThirteenF,
    /// Registration statement
    S1,
    /// Other filing type
    Other,
}

impl FilingType {
    /// Parse from SEC form name
    pub fn from_form(form: &str) -> Self {
        match form.to_uppercase().as_str() {
            "10-K" | "10-K/A" => FilingType::TenK,
            "10-Q" | "10-Q/A" => FilingType::TenQ,
            "8-K" | "8-K/A" => FilingType::EightK,
            "DEF 14A" | "DEFA14A" => FilingType::DefFourteen,
            "4" | "4/A" => FilingType::FormFour,
            "13F-HR" | "13F-HR/A" => FilingType::ThirteenF,
            "S-1" | "S-1/A" => FilingType::S1,
            _ => FilingType::Other,
        }
    }

    /// Get SEC form name
    pub fn form_name(&self) -> &str {
        match self {
            FilingType::TenK => "10-K",
            FilingType::TenQ => "10-Q",
            FilingType::EightK => "8-K",
            FilingType::DefFourteen => "DEF 14A",
            FilingType::FormFour => "4",
            FilingType::ThirteenF => "13F-HR",
            FilingType::S1 => "S-1",
            FilingType::Other => "Other",
        }
    }
}

/// A SEC filing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filing {
    /// Accession number (unique identifier)
    pub accession_number: String,

    /// Company CIK
    pub cik: String,

    /// Filing type
    pub filing_type: FilingType,

    /// Date filed
    pub filed_date: NaiveDate,

    /// Primary document URL
    pub document_url: String,

    /// Description
    pub description: Option<String>,
}

/// Filing analyzer for extracting insights
pub struct FilingAnalyzer {
    /// Configuration
    config: AnalyzerConfig,
}

/// Analyzer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzerConfig {
    /// Extract key phrases
    pub extract_phrases: bool,

    /// Sentiment analysis
    pub analyze_sentiment: bool,

    /// Risk factor extraction
    pub extract_risks: bool,

    /// Forward-looking statement extraction
    pub extract_fls: bool,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            extract_phrases: true,
            analyze_sentiment: true,
            extract_risks: true,
            extract_fls: true,
        }
    }
}

impl FilingAnalyzer {
    /// Create a new analyzer
    pub fn new(config: AnalyzerConfig) -> Self {
        Self { config }
    }

    /// Analyze a filing document
    pub fn analyze(&self, content: &str, filing: &Filing) -> FilingAnalysis {
        let sections = self.extract_sections(content, &filing.filing_type);
        let sentiment = if self.config.analyze_sentiment {
            Some(self.compute_sentiment(content))
        } else {
            None
        };

        let risk_factors = if self.config.extract_risks {
            self.extract_risk_factors(content)
        } else {
            vec![]
        };

        let forward_looking = if self.config.extract_fls {
            self.extract_forward_looking(content)
        } else {
            vec![]
        };

        let key_phrases = if self.config.extract_phrases {
            self.extract_key_phrases(content)
        } else {
            vec![]
        };

        FilingAnalysis {
            accession_number: filing.accession_number.clone(),
            sections,
            sentiment,
            risk_factors,
            forward_looking,
            key_phrases,
            word_count: content.split_whitespace().count(),
        }
    }

    /// Extract standard sections from filing
    fn extract_sections(&self, content: &str, filing_type: &FilingType) -> HashMap<String, String> {
        let mut sections = HashMap::new();

        // Section patterns vary by filing type
        let section_patterns = match filing_type {
            FilingType::TenK => vec![
                ("Business", "Item 1"),
                ("RiskFactors", "Item 1A"),
                ("Properties", "Item 2"),
                ("Legal", "Item 3"),
                ("MDA", "Item 7"),
                ("Financials", "Item 8"),
            ],
            FilingType::TenQ => vec![
                ("Financials", "Part I"),
                ("MDA", "Item 2"),
                ("Controls", "Item 4"),
            ],
            FilingType::EightK => vec![
                ("Item", "Item"),
            ],
            _ => vec![],
        };

        // Simplified extraction - would use better text segmentation
        for (name, marker) in section_patterns {
            if let Some(idx) = content.find(marker) {
                let section_text = &content[idx..];
                let end_idx = section_text.len().min(5000);
                sections.insert(name.to_string(), section_text[..end_idx].to_string());
            }
        }

        sections
    }

    /// Compute sentiment score (-1 to 1)
    fn compute_sentiment(&self, content: &str) -> f64 {
        let positive_words = [
            "growth", "profit", "increased", "strong", "improved", "successful",
            "innovative", "opportunity", "favorable", "exceeded", "achieved",
        ];

        let negative_words = [
            "loss", "decline", "decreased", "weak", "challenging", "risk",
            "uncertain", "adverse", "impairment", "litigation", "default",
        ];

        let content_lower = content.to_lowercase();
        let words: Vec<&str> = content_lower.split_whitespace().collect();
        let total_words = words.len() as f64;

        let positive_count = positive_words
            .iter()
            .map(|w| words.iter().filter(|word| word.contains(w)).count())
            .sum::<usize>() as f64;

        let negative_count = negative_words
            .iter()
            .map(|w| words.iter().filter(|word| word.contains(w)).count())
            .sum::<usize>() as f64;

        if total_words > 0.0 {
            (positive_count - negative_count) / total_words.sqrt()
        } else {
            0.0
        }
    }

    /// Extract risk factors
    fn extract_risk_factors(&self, content: &str) -> Vec<RiskFactor> {
        let mut risks = Vec::new();

        let risk_patterns = [
            ("Regulatory", "regulatory", "regulation", "compliance"),
            ("Competition", "competitive", "competition", "competitors"),
            ("Cybersecurity", "cybersecurity", "data breach", "security"),
            ("Litigation", "litigation", "lawsuit", "legal proceedings"),
            ("Economic", "economic conditions", "recession", "downturn"),
            ("Supply Chain", "supply chain", "suppliers", "logistics"),
        ];

        let content_lower = content.to_lowercase();

        for (category, pattern1, pattern2, pattern3) in risk_patterns {
            let count = [pattern1, pattern2, pattern3]
                .iter()
                .map(|p| content_lower.matches(p).count())
                .sum::<usize>();

            if count > 0 {
                risks.push(RiskFactor {
                    category: category.to_string(),
                    severity: (count as f64 / 10.0).min(1.0),
                    mentions: count,
                    sample_text: None,
                });
            }
        }

        risks.sort_by(|a, b| b.severity.partial_cmp(&a.severity).unwrap_or(std::cmp::Ordering::Equal));
        risks
    }

    /// Extract forward-looking statements
    fn extract_forward_looking(&self, content: &str) -> Vec<ForwardLookingStatement> {
        let mut statements = Vec::new();

        let fls_patterns = [
            "expect", "anticipate", "believe", "estimate", "project",
            "forecast", "intend", "plan", "may", "will", "should",
        ];

        let sentences: Vec<&str> = content.split(&['.', '!', '?'][..]).collect();

        for sentence in sentences {
            let sentence_lower = sentence.to_lowercase();

            for pattern in fls_patterns {
                if sentence_lower.contains(pattern) {
                    // Check if it's truly forward-looking
                    if sentence_lower.contains("future") ||
                       sentence_lower.contains("expect") ||
                       sentence_lower.contains("anticipate") {
                        statements.push(ForwardLookingStatement {
                            text: sentence.trim().to_string(),
                            sentiment: self.compute_sentiment(sentence),
                            confidence: 0.7,
                        });
                        break;
                    }
                }
            }
        }

        // Limit to most significant
        statements.truncate(20);
        statements
    }

    /// Extract key phrases
    fn extract_key_phrases(&self, content: &str) -> Vec<KeyPhrase> {
        let mut phrases = HashMap::new();

        // Simple n-gram extraction
        let words: Vec<&str> = content
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();

        // Bigrams
        for window in words.windows(2) {
            let phrase = format!("{} {}", window[0].to_lowercase(), window[1].to_lowercase());
            if self.is_meaningful_phrase(&phrase) {
                *phrases.entry(phrase).or_insert(0) += 1;
            }
        }

        let mut result: Vec<KeyPhrase> = phrases
            .into_iter()
            .filter(|(_, count)| *count >= 3)
            .map(|(phrase, count)| KeyPhrase {
                phrase,
                frequency: count,
                importance: count as f64 / words.len() as f64,
            })
            .collect();

        result.sort_by(|a, b| b.frequency.cmp(&a.frequency));
        result.truncate(50);
        result
    }

    /// Check if phrase is meaningful
    fn is_meaningful_phrase(&self, phrase: &str) -> bool {
        let stop_phrases = ["the", "and", "for", "this", "that", "with"];
        !stop_phrases.iter().any(|s| phrase.starts_with(s))
    }
}

/// Analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilingAnalysis {
    /// Filing accession number
    pub accession_number: String,

    /// Extracted sections
    pub sections: HashMap<String, String>,

    /// Overall sentiment score
    pub sentiment: Option<f64>,

    /// Risk factors
    pub risk_factors: Vec<RiskFactor>,

    /// Forward-looking statements
    pub forward_looking: Vec<ForwardLookingStatement>,

    /// Key phrases
    pub key_phrases: Vec<KeyPhrase>,

    /// Total word count
    pub word_count: usize,
}

/// A risk factor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    /// Risk category
    pub category: String,

    /// Severity score (0-1)
    pub severity: f64,

    /// Number of mentions
    pub mentions: usize,

    /// Sample text
    pub sample_text: Option<String>,
}

/// A forward-looking statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardLookingStatement {
    /// Statement text
    pub text: String,

    /// Sentiment score
    pub sentiment: f64,

    /// Confidence that this is FLS
    pub confidence: f64,
}

/// A key phrase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPhrase {
    /// Phrase text
    pub phrase: String,

    /// Frequency count
    pub frequency: usize,

    /// Importance score
    pub importance: f64,
}

/// Narrative extractor for text-to-vector
pub struct NarrativeExtractor {
    /// Configuration
    config: ExtractorConfig,
}

/// Extractor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractorConfig {
    /// Target embedding dimension
    pub embedding_dim: usize,

    /// Use TF-IDF weighting
    pub use_tfidf: bool,

    /// Normalize embeddings
    pub normalize: bool,
}

impl Default for ExtractorConfig {
    fn default() -> Self {
        Self {
            embedding_dim: 128,
            use_tfidf: true,
            normalize: true,
        }
    }
}

impl NarrativeExtractor {
    /// Create a new extractor
    pub fn new(config: ExtractorConfig) -> Self {
        Self { config }
    }

    /// Extract embedding from filing analysis
    pub fn extract_embedding(&self, analysis: &FilingAnalysis) -> Vec<f32> {
        let mut embedding = Vec::with_capacity(self.config.embedding_dim);

        // Sentiment feature
        embedding.push(analysis.sentiment.unwrap_or(0.0) as f32);

        // Word count (normalized)
        embedding.push((analysis.word_count as f64 / 100000.0).min(1.0) as f32);

        // Risk factor features
        let total_risk_severity: f64 = analysis.risk_factors.iter().map(|r| r.severity).sum();
        embedding.push((total_risk_severity / 5.0).min(1.0) as f32);

        // FLS sentiment
        let fls_sentiment: f64 = analysis.forward_looking
            .iter()
            .map(|f| f.sentiment)
            .sum::<f64>() / analysis.forward_looking.len().max(1) as f64;
        embedding.push(fls_sentiment as f32);

        // Key phrase diversity
        let phrase_diversity = analysis.key_phrases.len() as f64 / 100.0;
        embedding.push(phrase_diversity.min(1.0) as f32);

        // Pad to target dimension
        while embedding.len() < self.config.embedding_dim {
            embedding.push(0.0);
        }

        // Normalize
        if self.config.normalize {
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut embedding {
                    *x /= norm;
                }
            }
        }

        embedding
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filing_type_from_form() {
        assert_eq!(FilingType::from_form("10-K"), FilingType::TenK);
        assert_eq!(FilingType::from_form("10-Q"), FilingType::TenQ);
        assert_eq!(FilingType::from_form("8-K"), FilingType::EightK);
    }

    #[test]
    fn test_sentiment_analysis() {
        let config = AnalyzerConfig::default();
        let analyzer = FilingAnalyzer::new(config);

        let positive_text = "Growth and profit increased significantly. Strong performance exceeded expectations.";
        let sentiment = analyzer.compute_sentiment(positive_text);
        assert!(sentiment > 0.0);

        let negative_text = "Loss and decline due to challenging conditions. Risk of default increased.";
        let sentiment = analyzer.compute_sentiment(negative_text);
        assert!(sentiment < 0.0);
    }
}
