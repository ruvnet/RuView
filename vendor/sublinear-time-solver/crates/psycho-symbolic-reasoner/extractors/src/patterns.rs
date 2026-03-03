use crate::preferences::PreferenceType;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TextPattern {
    pub id: String,
    pub name: String,
    pub regex: Regex,
    pub preference_type: Option<PreferenceType>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    pub pattern_id: String,
    pub pattern_name: String,
    pub matched_text: String,
    pub groups: Vec<String>,
    pub confidence: f64,
    pub start_pos: usize,
    pub end_pos: usize,
}

#[derive(Debug)]
pub struct PatternMatcher {
    patterns: Vec<TextPattern>,
    pattern_cache: HashMap<String, Vec<PatternMatch>>,
}

impl PatternMatcher {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            pattern_cache: HashMap::new(),
        }
    }

    pub fn add_pattern(&mut self, pattern: TextPattern) {
        self.patterns.push(pattern);
        self.clear_cache();
    }

    pub fn match_text(&mut self, text: &str) -> Vec<PatternMatch> {
        // Check cache first
        if let Some(cached_matches) = self.pattern_cache.get(text) {
            return cached_matches.clone();
        }

        let mut matches = Vec::new();

        for pattern in &self.patterns {
            for capture_match in pattern.regex.find_iter(text) {
                let matched_text = capture_match.as_str().to_string();
                let groups = if let Some(captures) = pattern.regex.captures(&matched_text) {
                    captures
                        .iter()
                        .skip(1) // Skip the full match
                        .filter_map(|m| m.map(|m| m.as_str().to_string()))
                        .collect()
                } else {
                    Vec::new()
                };

                matches.push(PatternMatch {
                    pattern_id: pattern.id.clone(),
                    pattern_name: pattern.name.clone(),
                    matched_text,
                    groups,
                    confidence: pattern.confidence,
                    start_pos: capture_match.start(),
                    end_pos: capture_match.end(),
                });
            }
        }

        // Sort matches by position
        matches.sort_by_key(|m| m.start_pos);

        // Cache the result
        self.pattern_cache.insert(text.to_string(), matches.clone());

        matches
    }

    pub fn find_pattern_by_id(&self, pattern_id: &str) -> Option<&TextPattern> {
        self.patterns.iter().find(|p| p.id == pattern_id)
    }

    pub fn get_patterns_by_type(&self, preference_type: &PreferenceType) -> Vec<&TextPattern> {
        self.patterns
            .iter()
            .filter(|p| p.preference_type.as_ref() == Some(preference_type))
            .collect()
    }

    pub fn remove_pattern(&mut self, pattern_id: &str) -> bool {
        if let Some(pos) = self.patterns.iter().position(|p| p.id == pattern_id) {
            self.patterns.remove(pos);
            self.clear_cache();
            true
        } else {
            false
        }
    }

    pub fn clear_cache(&mut self) {
        self.pattern_cache.clear();
    }

    pub fn get_pattern_statistics(&self) -> PatternStatistics {
        let total_patterns = self.patterns.len();
        let mut type_counts = HashMap::new();

        for pattern in &self.patterns {
            if let Some(ref pref_type) = pattern.preference_type {
                *type_counts.entry(format!("{:?}", pref_type)).or_insert(0) += 1;
            } else {
                *type_counts.entry("None".to_string()).or_insert(0) += 1;
            }
        }

        let average_confidence = if total_patterns > 0 {
            self.patterns.iter().map(|p| p.confidence).sum::<f64>() / total_patterns as f64
        } else {
            0.0
        };

        PatternStatistics {
            total_patterns,
            type_counts,
            average_confidence,
            cache_size: self.pattern_cache.len(),
        }
    }

    pub fn create_common_patterns() -> Vec<TextPattern> {
        let mut patterns = Vec::new();

        // Time patterns
        if let Ok(regex) = Regex::new(r"(?i)\b((?:yesterday|today|tomorrow|monday|tuesday|wednesday|thursday|friday|saturday|sunday|\d{1,2}(?:st|nd|rd|th)?\s+(?:january|february|march|april|may|june|july|august|september|october|november|december)|\d{1,2}/\d{1,2}/\d{2,4}|\d{4}-\d{2}-\d{2}))\b") {
            patterns.push(TextPattern {
                id: uuid::Uuid::new_v4().to_string(),
                name: "time_pattern".to_string(),
                regex,
                preference_type: None,
                confidence: 0.9,
            });
        }

        // Location patterns
        if let Ok(regex) = Regex::new(r"(?i)\b(at|in|on|near|around)\s+([A-Z][a-zA-Z\s]+(?:Street|St|Avenue|Ave|Road|Rd|Boulevard|Blvd|Drive|Dr|Lane|Ln|City|Town|State|Country)?)\b") {
            patterns.push(TextPattern {
                id: uuid::Uuid::new_v4().to_string(),
                name: "location_pattern".to_string(),
                regex,
                preference_type: None,
                confidence: 0.8,
            });
        }

        // Price patterns
        if let Ok(regex) = Regex::new(r"(?i)\$\d+(?:\.\d{2})?|\d+\s*(?:dollars?|cents?|bucks?)|\d+\.\d{2}") {
            patterns.push(TextPattern {
                id: uuid::Uuid::new_v4().to_string(),
                name: "price_pattern".to_string(),
                regex,
                preference_type: None,
                confidence: 0.95,
            });
        }

        // Email patterns
        if let Ok(regex) = Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b") {
            patterns.push(TextPattern {
                id: uuid::Uuid::new_v4().to_string(),
                name: "email_pattern".to_string(),
                regex,
                preference_type: None,
                confidence: 0.98,
            });
        }

        // Phone patterns
        if let Ok(regex) = Regex::new(r"(?:\+?1[-.\s]?)?\(?([0-9]{3})\)?[-.\s]?([0-9]{3})[-.\s]?([0-9]{4})") {
            patterns.push(TextPattern {
                id: uuid::Uuid::new_v4().to_string(),
                name: "phone_pattern".to_string(),
                regex,
                preference_type: None,
                confidence: 0.9,
            });
        }

        // URL patterns
        if let Ok(regex) = Regex::new(r"https?://(?:[-\w.])+(?:[:\d]+)?(?:/(?:[\w/_.])*(?:\?(?:[\w&=%.])*)?(?:#(?:\w*))?)?") {
            patterns.push(TextPattern {
                id: uuid::Uuid::new_v4().to_string(),
                name: "url_pattern".to_string(),
                regex,
                preference_type: None,
                confidence: 0.95,
            });
        }

        // Comparison patterns (for preferences)
        if let Ok(regex) = Regex::new(r"(?i)\b(.*?)\s+(?:is\s+)?(?:better|worse|superior|inferior|preferable)\s+(?:than|to)\s+(.*?)\b") {
            patterns.push(TextPattern {
                id: uuid::Uuid::new_v4().to_string(),
                name: "comparison_pattern".to_string(),
                regex,
                preference_type: Some(PreferenceType::Prefer),
                confidence: 0.8,
            });
        }

        // Quantity patterns
        if let Ok(regex) = Regex::new(r"(?i)\b(\d+(?:\.\d+)?)\s*(percent|%|pounds?|lbs?|kilograms?|kgs?|grams?|ounces?|oz|inches?|feet|ft|meters?|miles?|kilometers?|hours?|minutes?|seconds?|days?|weeks?|months?|years?)\b") {
            patterns.push(TextPattern {
                id: uuid::Uuid::new_v4().to_string(),
                name: "quantity_pattern".to_string(),
                regex,
                preference_type: None,
                confidence: 0.85,
            });
        }

        patterns
    }

    pub fn validate_pattern(pattern_str: &str) -> Result<(), String> {
        match Regex::new(pattern_str) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Invalid regex pattern: {}", e)),
        }
    }

    pub fn extract_named_entities(&self, text: &str) -> Vec<NamedEntity> {
        let mut entities = Vec::new();

        // Person names (simplified - looks for capitalized words that could be names)
        if let Ok(person_regex) = Regex::new(r"\b[A-Z][a-z]+\s+[A-Z][a-z]+\b") {
            for capture_match in person_regex.find_iter(text) {
                entities.push(NamedEntity {
                    entity_type: EntityType::Person,
                    text: capture_match.as_str().to_string(),
                    start_pos: capture_match.start(),
                    end_pos: capture_match.end(),
                    confidence: 0.6, // Lower confidence for simple pattern
                });
            }
        }

        // Organizations (simplified - looks for certain patterns)
        if let Ok(org_regex) = Regex::new(r"\b[A-Z][a-zA-Z\s]*(?:Inc|Corp|LLC|Ltd|Company|Corporation|Organization|University|College|School)\b") {
            for capture_match in org_regex.find_iter(text) {
                entities.push(NamedEntity {
                    entity_type: EntityType::Organization,
                    text: capture_match.as_str().to_string(),
                    start_pos: capture_match.start(),
                    end_pos: capture_match.end(),
                    confidence: 0.8,
                });
            }
        }

        entities
    }
}

impl Default for PatternMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternStatistics {
    pub total_patterns: usize,
    pub type_counts: HashMap<String, usize>,
    pub average_confidence: f64,
    pub cache_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedEntity {
    pub entity_type: EntityType,
    pub text: String,
    pub start_pos: usize,
    pub end_pos: usize,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType {
    Person,
    Organization,
    Location,
    Date,
    Time,
    Money,
    Percentage,
    Miscellaneous,
}

// Builder pattern for creating complex patterns
#[derive(Debug)]
pub struct PatternBuilder {
    name: String,
    pattern_parts: Vec<String>,
    preference_type: Option<PreferenceType>,
    confidence: f64,
}

impl PatternBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            pattern_parts: Vec::new(),
            preference_type: None,
            confidence: 0.8,
        }
    }

    pub fn add_literal(mut self, text: &str) -> Self {
        self.pattern_parts.push(regex::escape(text));
        self
    }

    pub fn add_word_boundary(mut self) -> Self {
        self.pattern_parts.push(r"\b".to_string());
        self
    }

    pub fn add_capture_group(mut self, pattern: &str) -> Self {
        self.pattern_parts.push(format!("({})", pattern));
        self
    }

    pub fn add_optional_group(mut self, pattern: &str) -> Self {
        self.pattern_parts.push(format!("(?:{})?", pattern));
        self
    }

    pub fn add_any_word(mut self) -> Self {
        self.pattern_parts.push(r"\w+".to_string());
        self
    }

    pub fn add_whitespace(mut self) -> Self {
        self.pattern_parts.push(r"\s+".to_string());
        self
    }

    pub fn set_preference_type(mut self, pref_type: PreferenceType) -> Self {
        self.preference_type = Some(pref_type);
        self
    }

    pub fn set_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn case_insensitive(mut self) -> Self {
        if !self.pattern_parts.is_empty() {
            self.pattern_parts.insert(0, "(?i)".to_string());
        }
        self
    }

    pub fn build(self) -> Result<TextPattern, String> {
        if self.pattern_parts.is_empty() {
            return Err("Pattern cannot be empty".to_string());
        }

        let pattern_str = self.pattern_parts.join("");
        match Regex::new(&pattern_str) {
            Ok(regex) => Ok(TextPattern {
                id: uuid::Uuid::new_v4().to_string(),
                name: self.name,
                regex,
                preference_type: self.preference_type,
                confidence: self.confidence,
            }),
            Err(e) => Err(format!("Failed to compile regex: {}", e)),
        }
    }
}