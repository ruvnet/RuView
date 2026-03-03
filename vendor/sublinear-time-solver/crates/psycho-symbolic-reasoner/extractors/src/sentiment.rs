use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use regex::Regex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentResult {
    pub score: f64,
    pub label: SentimentLabel,
    pub confidence: f64,
    pub positive_words: Vec<String>,
    pub negative_words: Vec<String>,
    pub neutral_words: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SentimentLabel {
    VeryPositive,
    Positive,
    Neutral,
    Negative,
    VeryNegative,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentScore {
    pub positive: f64,
    pub negative: f64,
    pub neutral: f64,
}

#[derive(Debug)]
pub struct SentimentAnalyzer {
    positive_lexicon: HashMap<String, f64>,
    negative_lexicon: HashMap<String, f64>,
    intensifiers: HashMap<String, f64>,
    negations: Vec<String>,
    text_processor: TextProcessor,
}

impl SentimentAnalyzer {
    pub fn new() -> Self {
        let mut analyzer = Self {
            positive_lexicon: HashMap::new(),
            negative_lexicon: HashMap::new(),
            intensifiers: HashMap::new(),
            negations: Vec::new(),
            text_processor: TextProcessor::new(),
        };

        analyzer.initialize_lexicons();
        analyzer
    }

    fn initialize_lexicons(&mut self) {
        // Positive words with scores
        let positive_words = [
            ("amazing", 2.0), ("awesome", 2.0), ("brilliant", 2.0), ("excellent", 2.0),
            ("fantastic", 2.0), ("great", 1.5), ("good", 1.0), ("wonderful", 2.0),
            ("love", 1.8), ("like", 1.0), ("enjoy", 1.2), ("happy", 1.5),
            ("pleased", 1.3), ("satisfied", 1.2), ("perfect", 2.0), ("outstanding", 2.0),
            ("superb", 1.8), ("nice", 1.0), ("beautiful", 1.5), ("magnificent", 2.0),
            ("marvelous", 1.8), ("incredible", 2.0), ("delightful", 1.5), ("charming", 1.3),
            ("exciting", 1.5), ("thrilling", 1.8), ("inspiring", 1.5), ("motivating", 1.3),
            ("refreshing", 1.2), ("relaxing", 1.2), ("comfortable", 1.1), ("convenient", 1.0),
            ("helpful", 1.2), ("useful", 1.1), ("valuable", 1.3), ("beneficial", 1.2),
            ("positive", 1.2), ("optimistic", 1.3), ("hopeful", 1.2), ("confident", 1.3),
        ];

        for (word, score) in positive_words.iter() {
            self.positive_lexicon.insert(word.to_string(), *score);
        }

        // Negative words with scores
        let negative_words = [
            ("terrible", -2.0), ("awful", -2.0), ("horrible", -2.0), ("disgusting", -2.0),
            ("hate", -1.8), ("dislike", -1.0), ("bad", -1.0), ("poor", -1.2),
            ("worse", -1.5), ("worst", -2.0), ("disappointing", -1.5), ("frustrating", -1.5),
            ("annoying", -1.3), ("irritating", -1.3), ("boring", -1.2), ("dull", -1.1),
            ("sad", -1.5), ("depressed", -1.8), ("angry", -1.6), ("furious", -2.0),
            ("upset", -1.4), ("worried", -1.2), ("anxious", -1.3), ("stressed", -1.4),
            ("confused", -1.1), ("lost", -1.2), ("broken", -1.5), ("damaged", -1.4),
            ("useless", -1.6), ("worthless", -1.8), ("pointless", -1.5), ("meaningless", -1.4),
            ("difficult", -1.1), ("hard", -1.0), ("impossible", -1.8), ("complicated", -1.2),
            ("expensive", -1.1), ("cheap", -1.0), ("slow", -1.1), ("fast", 0.5), // fast can be positive in some contexts
            ("negative", -1.2), ("pessimistic", -1.3), ("hopeless", -1.8), ("desperate", -1.6),
        ];

        for (word, score) in negative_words.iter() {
            self.negative_lexicon.insert(word.to_string(), *score);
        }

        // Intensifiers
        let intensifiers = [
            ("very", 1.5), ("extremely", 2.0), ("incredibly", 2.0), ("really", 1.3),
            ("quite", 1.2), ("rather", 1.1), ("somewhat", 0.8), ("slightly", 0.7),
            ("totally", 1.8), ("completely", 1.8), ("absolutely", 2.0), ("perfectly", 1.8),
            ("highly", 1.5), ("deeply", 1.4), ("truly", 1.4), ("genuinely", 1.3),
        ];

        for (word, multiplier) in intensifiers.iter() {
            self.intensifiers.insert(word.to_string(), *multiplier);
        }

        // Negations
        self.negations = vec![
            "not".to_string(), "no".to_string(), "never".to_string(), "nothing".to_string(),
            "nobody".to_string(), "nowhere".to_string(), "neither".to_string(), "none".to_string(),
            "without".to_string(), "lack".to_string(), "lacking".to_string(), "fail".to_string(),
            "cannot".to_string(), "can't".to_string(), "won't".to_string(), "wouldn't".to_string(),
            "shouldn't".to_string(), "couldn't".to_string(), "don't".to_string(), "doesn't".to_string(),
            "didn't".to_string(), "isn't".to_string(), "aren't".to_string(), "wasn't".to_string(),
            "weren't".to_string(), "haven't".to_string(), "hasn't".to_string(), "hadn't".to_string(),
        ];
    }

    pub fn analyze(&self, text: &str) -> SentimentResult {
        let processed_text = self.text_processor.process(text);
        let tokens = self.text_processor.tokenize(&processed_text);

        let mut total_score = 0.0;
        let mut positive_words = Vec::new();
        let mut negative_words = Vec::new();
        let mut neutral_words = Vec::new();
        let mut word_count = 0;

        let mut i = 0;
        while i < tokens.len() {
            let token = &tokens[i];
            let mut current_score = 0.0;
            let mut intensity_multiplier = 1.0;
            let mut is_negated = false;

            // Check for negation in the previous 3 words
            for j in 1..=3 {
                if i >= j {
                    let prev_token = &tokens[i - j];
                    if self.negations.contains(prev_token) {
                        is_negated = true;
                        break;
                    }
                }
            }

            // Check for intensifiers in the previous 2 words
            for j in 1..=2 {
                if i >= j {
                    let prev_token = &tokens[i - j];
                    if let Some(&multiplier) = self.intensifiers.get(prev_token) {
                        intensity_multiplier *= multiplier;
                    }
                }
            }

            // Get sentiment score
            if let Some(&score) = self.positive_lexicon.get(token) {
                current_score = score * intensity_multiplier;
                if is_negated {
                    current_score = -current_score;
                }
                if current_score > 0.0 {
                    positive_words.push(token.clone());
                } else {
                    negative_words.push(token.clone());
                }
            } else if let Some(&score) = self.negative_lexicon.get(token) {
                current_score = score * intensity_multiplier;
                if is_negated {
                    current_score = -current_score;
                }
                if current_score < 0.0 {
                    negative_words.push(token.clone());
                } else {
                    positive_words.push(token.clone());
                }
            } else if !self.is_stop_word(token) && !self.intensifiers.contains_key(token) {
                neutral_words.push(token.clone());
            }

            total_score += current_score;
            word_count += 1;
            i += 1;
        }

        // Normalize score
        let normalized_score = if word_count > 0 {
            total_score / word_count as f64
        } else {
            0.0
        };

        // Determine label and confidence
        let (label, confidence) = self.calculate_label_and_confidence(normalized_score, &positive_words, &negative_words);

        let _scores = SentimentScore {
            positive: positive_words.len() as f64 / word_count.max(1) as f64,
            negative: negative_words.len() as f64 / word_count.max(1) as f64,
            neutral: neutral_words.len() as f64 / word_count.max(1) as f64,
        };

        SentimentResult {
            score: normalized_score,
            label,
            confidence,
            positive_words,
            negative_words,
            neutral_words,
        }
    }

    fn calculate_label_and_confidence(&self, score: f64, positive_words: &[String], negative_words: &[String]) -> (SentimentLabel, f64) {
        let abs_score = score.abs();
        let word_ratio = (positive_words.len() + negative_words.len()) as f64 / (positive_words.len().max(1) + negative_words.len().max(1)) as f64;

        let confidence = (abs_score * word_ratio).min(1.0).max(0.0);

        let label = if score >= 1.0 {
            SentimentLabel::VeryPositive
        } else if score >= 0.3 {
            SentimentLabel::Positive
        } else if score <= -1.0 {
            SentimentLabel::VeryNegative
        } else if score <= -0.3 {
            SentimentLabel::Negative
        } else {
            SentimentLabel::Neutral
        };

        (label, confidence)
    }

    fn is_stop_word(&self, word: &str) -> bool {
        let stop_words = [
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "from", "as", "is", "was", "are", "were", "be", "been", "being", "have", "has",
            "had", "do", "does", "did", "will", "would", "could", "should", "may", "might", "must",
            "i", "you", "he", "she", "it", "we", "they", "me", "him", "her", "us", "them",
            "my", "your", "his", "her", "its", "our", "their", "this", "that", "these", "those",
        ];
        stop_words.contains(&word.to_lowercase().as_str())
    }

    pub fn add_positive_word(&mut self, word: &str, score: f64) {
        self.positive_lexicon.insert(word.to_string(), score);
    }

    pub fn add_negative_word(&mut self, word: &str, score: f64) {
        self.negative_lexicon.insert(word.to_string(), score);
    }

    pub fn remove_word(&mut self, word: &str) {
        self.positive_lexicon.remove(word);
        self.negative_lexicon.remove(word);
    }
}

impl Default for SentimentAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct TextProcessor {
    word_regex: Regex,
    punctuation_regex: Regex,
}

impl TextProcessor {
    fn new() -> Self {
        Self {
            word_regex: Regex::new(r"\b\w+\b").unwrap(),
            punctuation_regex: Regex::new(r"[^\w\s]").unwrap(),
        }
    }

    fn process(&self, text: &str) -> String {
        // Convert to lowercase and remove excessive punctuation
        let lowercase = text.to_lowercase();
        self.punctuation_regex.replace_all(&lowercase, " ").to_string()
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        self.word_regex
            .find_iter(text)
            .map(|mat| mat.as_str().to_string())
            .collect()
    }
}