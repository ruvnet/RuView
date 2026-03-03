use crate::patterns::{PatternMatcher, TextPattern};
use serde::{Deserialize, Serialize};
use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceResult {
    pub preferences: Vec<PreferencePattern>,
    pub confidence: f64,
    pub extracted_entities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferencePattern {
    pub id: String,
    pub preference_type: PreferenceType,
    pub subject: String,
    pub preferred_item: String,
    pub alternative_item: Option<String>,
    pub strength: PreferenceStrength,
    pub confidence: f64,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PreferenceType {
    Like,
    Dislike,
    Prefer,
    Avoid,
    Want,
    Need,
    Choose,
    Recommend,
    Oppose,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreferenceStrength {
    Weak,
    Moderate,
    Strong,
    VeryStrong,
}

#[derive(Debug)]
pub struct PreferenceExtractor {
    pattern_matcher: PatternMatcher,
    preference_patterns: Vec<TextPattern>,
    strength_indicators: HashMap<String, PreferenceStrength>,
    negation_words: Vec<String>,
}

impl PreferenceExtractor {
    pub fn new() -> Self {
        let mut extractor = Self {
            pattern_matcher: PatternMatcher::new(),
            preference_patterns: Vec::new(),
            strength_indicators: HashMap::new(),
            negation_words: Vec::new(),
        };

        extractor.initialize_patterns();
        extractor.initialize_strength_indicators();
        extractor.initialize_negations();
        extractor
    }

    fn initialize_patterns(&mut self) {
        // Preference patterns with regex
        let patterns = [
            // Like patterns
            (r"(?i)\b(I|we|you|they|he|she)\s+(really\s+|absolutely\s+|definitely\s+)?(like|love|enjoy|adore)\s+(.*?)(?:\.|,|;|!|\?|$)", PreferenceType::Like),

            // Dislike patterns
            (r"(?i)\b(I|we|you|they|he|she)\s+(really\s+|absolutely\s+|definitely\s+)?(dislike|hate|loathe|despise)\s+(.*?)(?:\.|,|;|!|\?|$)", PreferenceType::Dislike),

            // Prefer patterns
            (r"(?i)\b(I|we|you|they|he|she)\s+(would\s+)?(prefer|choose|pick|select)\s+(.*?)\s+(?:over|instead\s+of|rather\s+than)\s+(.*?)(?:\.|,|;|!|\?|$)", PreferenceType::Prefer),
            (r"(?i)\b(.*?)\s+is\s+(better|superior|preferable)\s+to\s+(.*?)(?:\.|,|;|!|\?|$)", PreferenceType::Prefer),

            // Want patterns
            (r"(?i)\b(I|we|you|they|he|she)\s+(really\s+|desperately\s+|badly\s+)?(want|desire|wish\s+for|crave)\s+(.*?)(?:\.|,|;|!|\?|$)", PreferenceType::Want),

            // Need patterns
            (r"(?i)\b(I|we|you|they|he|she)\s+(really\s+|desperately\s+|urgently\s+)?(need|require|must\s+have)\s+(.*?)(?:\.|,|;|!|\?|$)", PreferenceType::Need),

            // Recommend patterns
            (r"(?i)\b(I|we)\s+(would\s+)?(recommend|suggest|advise)\s+(.*?)(?:\.|,|;|!|\?|$)", PreferenceType::Recommend),
            (r"(?i)\b(.*?)\s+is\s+(recommended|suggested|advised)(?:\.|,|;|!|\?|$)", PreferenceType::Recommend),

            // Avoid patterns
            (r"(?i)\b(I|we|you|they|he|she)\s+(should\s+|would\s+|try\s+to\s+)?(avoid|stay\s+away\s+from|keep\s+away\s+from)\s+(.*?)(?:\.|,|;|!|\?|$)", PreferenceType::Avoid),

            // Choose patterns
            (r"(?i)\b(I|we|you|they|he|she)\s+(would\s+|will\s+)?(choose|go\s+with|opt\s+for|decide\s+on)\s+(.*?)(?:\.|,|;|!|\?|$)", PreferenceType::Choose),

            // Oppose patterns
            (r"(?i)\b(I|we|you|they|he|she)\s+(strongly\s+|firmly\s+)?(oppose|object\s+to|disagree\s+with|reject)\s+(.*?)(?:\.|,|;|!|\?|$)", PreferenceType::Oppose),
        ];

        for (pattern_str, pref_type) in patterns.iter() {
            if let Ok(regex) = Regex::new(pattern_str) {
                self.preference_patterns.push(TextPattern {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: format!("{:?}_pattern", pref_type),
                    regex,
                    preference_type: Some(pref_type.clone()),
                    confidence: 0.8,
                });
            }
        }
    }

    fn initialize_strength_indicators(&mut self) {
        let indicators = [
            // Very strong
            ("absolutely", PreferenceStrength::VeryStrong),
            ("definitely", PreferenceStrength::VeryStrong),
            ("certainly", PreferenceStrength::VeryStrong),
            ("undoubtedly", PreferenceStrength::VeryStrong),
            ("extremely", PreferenceStrength::VeryStrong),
            ("desperately", PreferenceStrength::VeryStrong),
            ("passionately", PreferenceStrength::VeryStrong),

            // Strong
            ("really", PreferenceStrength::Strong),
            ("very", PreferenceStrength::Strong),
            ("strongly", PreferenceStrength::Strong),
            ("greatly", PreferenceStrength::Strong),
            ("highly", PreferenceStrength::Strong),
            ("deeply", PreferenceStrength::Strong),
            ("truly", PreferenceStrength::Strong),
            ("genuinely", PreferenceStrength::Strong),

            // Moderate
            ("quite", PreferenceStrength::Moderate),
            ("fairly", PreferenceStrength::Moderate),
            ("rather", PreferenceStrength::Moderate),
            ("pretty", PreferenceStrength::Moderate),
            ("reasonably", PreferenceStrength::Moderate),

            // Weak
            ("somewhat", PreferenceStrength::Weak),
            ("slightly", PreferenceStrength::Weak),
            ("a bit", PreferenceStrength::Weak),
            ("kind of", PreferenceStrength::Weak),
            ("sort of", PreferenceStrength::Weak),
            ("maybe", PreferenceStrength::Weak),
            ("perhaps", PreferenceStrength::Weak),
        ];

        for (word, strength) in indicators.iter() {
            self.strength_indicators.insert(word.to_string(), strength.clone());
        }
    }

    fn initialize_negations(&mut self) {
        self.negation_words = vec![
            "not".to_string(), "no".to_string(), "never".to_string(), "don't".to_string(),
            "doesn't".to_string(), "didn't".to_string(), "won't".to_string(), "wouldn't".to_string(),
            "can't".to_string(), "cannot".to_string(), "shouldn't".to_string(), "couldn't".to_string(),
            "isn't".to_string(), "aren't".to_string(), "wasn't".to_string(), "weren't".to_string(),
        ];
    }

    pub fn extract(&self, text: &str) -> Vec<PreferenceResult> {
        let mut results = Vec::new();
        let sentences = self.split_into_sentences(text);

        for sentence in sentences {
            let preferences = self.extract_from_sentence(&sentence);
            if !preferences.is_empty() {
                let confidence = self.calculate_sentence_confidence(&preferences);
                let entities = self.extract_entities(&sentence);

                results.push(PreferenceResult {
                    preferences,
                    confidence,
                    extracted_entities: entities,
                });
            }
        }

        results
    }

    fn extract_from_sentence(&self, sentence: &str) -> Vec<PreferencePattern> {
        let mut preferences = Vec::new();

        for pattern in &self.preference_patterns {
            if let Some(captures) = pattern.regex.captures(sentence) {
                if let Some(preference) = self.create_preference_from_match(&pattern, &captures, sentence) {
                    preferences.push(preference);
                }
            }
        }

        preferences
    }

    fn create_preference_from_match(&self, pattern: &TextPattern, captures: &regex::Captures, context: &str) -> Option<PreferencePattern> {
        let subject = captures.get(1)?.as_str().trim().to_string();
        let preference_type = pattern.preference_type.as_ref()?.clone();

        // Extract the main preference item (usually the last significant capture group)
        let preferred_item = if captures.len() > 2 {
            captures.get(captures.len() - 1)?.as_str().trim().to_string()
        } else {
            return None;
        };

        // For "prefer X over Y" patterns, extract the alternative
        let alternative_item = if captures.len() > 4 && matches!(preference_type, PreferenceType::Prefer) {
            Some(captures.get(captures.len() - 2)?.as_str().trim().to_string())
        } else {
            None
        };

        // Determine strength based on modifiers in the text
        let strength = self.determine_strength(context);

        // Calculate confidence based on pattern match quality and context
        let confidence = self.calculate_pattern_confidence(pattern, context);

        Some(PreferencePattern {
            id: uuid::Uuid::new_v4().to_string(),
            preference_type,
            subject,
            preferred_item,
            alternative_item,
            strength,
            confidence,
            context: context.to_string(),
        })
    }

    fn determine_strength(&self, text: &str) -> PreferenceStrength {
        let lower_text = text.to_lowercase();

        for (indicator, strength) in &self.strength_indicators {
            if lower_text.contains(indicator) {
                return strength.clone();
            }
        }

        PreferenceStrength::Moderate
    }

    fn calculate_pattern_confidence(&self, pattern: &TextPattern, context: &str) -> f64 {
        let mut confidence = pattern.confidence;

        // Boost confidence for specific indicators
        let lower_context = context.to_lowercase();

        // Check for negations that might affect confidence
        for negation in &self.negation_words {
            if lower_context.contains(negation) {
                confidence *= 0.7; // Reduce confidence for negated statements
                break;
            }
        }

        // Boost confidence for explicit preference words
        let explicit_words = ["prefer", "like", "love", "hate", "dislike", "want", "need"];
        for word in explicit_words.iter() {
            if lower_context.contains(word) {
                confidence *= 1.2;
                break;
            }
        }

        confidence.min(1.0).max(0.0)
    }

    fn calculate_sentence_confidence(&self, preferences: &[PreferencePattern]) -> f64 {
        if preferences.is_empty() {
            return 0.0;
        }

        let total_confidence: f64 = preferences.iter().map(|p| p.confidence).sum();
        total_confidence / preferences.len() as f64
    }

    fn extract_entities(&self, text: &str) -> Vec<String> {
        // Simple entity extraction based on capitalized words and noun phrases
        let entity_regex = Regex::new(r"\b[A-Z][a-z]+(?:\s+[A-Z][a-z]+)*\b").unwrap();

        entity_regex
            .find_iter(text)
            .map(|mat| mat.as_str().to_string())
            .collect()
    }

    fn split_into_sentences(&self, text: &str) -> Vec<String> {
        let sentence_regex = Regex::new(r"[.!?]+\s*").unwrap();
        sentence_regex
            .split(text)
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect()
    }

    pub fn add_custom_pattern(&mut self, pattern: &str, preference_type: PreferenceType, confidence: f64) -> Result<(), String> {
        match Regex::new(pattern) {
            Ok(regex) => {
                self.preference_patterns.push(TextPattern {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: format!("custom_{:?}_pattern", preference_type),
                    regex,
                    preference_type: Some(preference_type),
                    confidence,
                });
                Ok(())
            }
            Err(e) => Err(format!("Invalid regex pattern: {}", e)),
        }
    }

    pub fn get_pattern_statistics(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        for pattern in &self.preference_patterns {
            let key = pattern.preference_type
                .as_ref()
                .map(|pt| format!("{:?}", pt))
                .unwrap_or_else(|| "Unknown".to_string());
            *stats.entry(key).or_insert(0) += 1;
        }
        stats
    }
}

impl Default for PreferenceExtractor {
    fn default() -> Self {
        Self::new()
    }
}