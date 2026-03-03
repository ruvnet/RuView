use serde::{Deserialize, Serialize};
use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionResult {
    pub emotions: Vec<DetectedEmotion>,
    pub dominant_emotion: Option<EmotionType>,
    pub emotional_intensity: f64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedEmotion {
    pub emotion_type: EmotionType,
    pub intensity: f64,
    pub confidence: f64,
    pub triggers: Vec<String>,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EmotionType {
    // Primary emotions (Plutchik's model)
    Joy,
    Sadness,
    Anger,
    Fear,
    Trust,
    Disgust,
    Surprise,
    Anticipation,

    // Secondary emotions
    Love,
    Hate,
    Guilt,
    Shame,
    Pride,
    Envy,
    Jealousy,
    Hope,
    Despair,
    Anxiety,
    Relief,
    Excitement,
    Boredom,
    Confusion,
    Curiosity,
    Contempt,
    Admiration,
    Gratitude,
    Resentment,
    Nostalgia,
}

#[derive(Debug)]
pub struct EmotionDetector {
    emotion_lexicon: HashMap<EmotionType, Vec<EmotionWord>>,
    intensity_modifiers: HashMap<String, f64>,
    contextual_patterns: Vec<ContextualPattern>,
}

#[derive(Debug, Clone)]
struct EmotionWord {
    word: String,
    base_intensity: f64,
    variants: Vec<String>,
}

#[derive(Debug)]
struct ContextualPattern {
    pattern: Regex,
    emotion: EmotionType,
    intensity_boost: f64,
    confidence_boost: f64,
}

impl EmotionDetector {
    pub fn new() -> Self {
        let mut detector = Self {
            emotion_lexicon: HashMap::new(),
            intensity_modifiers: HashMap::new(),
            contextual_patterns: Vec::new(),
        };

        detector.initialize_emotion_lexicon();
        detector.initialize_intensity_modifiers();
        detector.initialize_contextual_patterns();
        detector
    }

    fn initialize_emotion_lexicon(&mut self) {
        // Joy emotions
        self.add_emotion_words(EmotionType::Joy, vec![
            ("happy", 0.7, vec!["happiness", "happily", "happier", "happiest"]),
            ("joyful", 0.8, vec!["joy", "joyfully"]),
            ("delighted", 0.8, vec!["delight", "delightful"]),
            ("ecstatic", 0.9, vec!["ecstasy"]),
            ("elated", 0.8, vec!["elation"]),
            ("cheerful", 0.6, vec!["cheer", "cheery"]),
            ("pleased", 0.6, vec!["pleasure", "pleasant"]),
            ("content", 0.5, vec!["contentment"]),
            ("satisfied", 0.6, vec!["satisfaction"]),
            ("thrilled", 0.9, vec!["thrill", "thrilling"]),
        ]);

        // Sadness emotions
        self.add_emotion_words(EmotionType::Sadness, vec![
            ("sad", 0.7, vec!["sadness", "sadly", "sadder", "saddest"]),
            ("depressed", 0.8, vec!["depression", "depressing"]),
            ("melancholy", 0.7, vec!["melancholic"]),
            ("miserable", 0.8, vec!["misery"]),
            ("heartbroken", 0.9, vec!["heartbreak"]),
            ("sorrowful", 0.8, vec!["sorrow"]),
            ("grieving", 0.8, vec!["grief", "grieve"]),
            ("upset", 0.6, vec!["upsetting"]),
            ("disappointed", 0.6, vec!["disappointment", "disappointing"]),
            ("dejected", 0.7, vec!["dejection"]),
        ]);

        // Anger emotions
        self.add_emotion_words(EmotionType::Anger, vec![
            ("angry", 0.7, vec!["anger", "angrily", "angrier", "angriest"]),
            ("furious", 0.9, vec!["fury"]),
            ("enraged", 0.9, vec!["rage", "raging"]),
            ("mad", 0.6, vec!["madness"]),
            ("irritated", 0.5, vec!["irritation", "irritating"]),
            ("annoyed", 0.5, vec!["annoyance", "annoying"]),
            ("frustrated", 0.6, vec!["frustration", "frustrating"]),
            ("outraged", 0.8, vec!["outrage"]),
            ("indignant", 0.7, vec!["indignation"]),
            ("livid", 0.9, vec![]),
        ]);

        // Fear emotions
        self.add_emotion_words(EmotionType::Fear, vec![
            ("afraid", 0.7, vec!["fear", "fearful"]),
            ("scared", 0.7, vec!["scary", "scare"]),
            ("terrified", 0.9, vec!["terror", "terrifying"]),
            ("frightened", 0.8, vec!["fright", "frightening"]),
            ("nervous", 0.5, vec!["nervousness"]),
            ("worried", 0.6, vec!["worry", "worrying"]),
            ("anxious", 0.6, vec!["anxiety"]),
            ("panicked", 0.8, vec!["panic", "panicking"]),
            ("horrified", 0.9, vec!["horror", "horrifying"]),
            ("alarmed", 0.7, vec!["alarm", "alarming"]),
        ]);

        // Trust emotions
        self.add_emotion_words(EmotionType::Trust, vec![
            ("trusting", 0.7, vec!["trust", "trustworthy"]),
            ("confident", 0.6, vec!["confidence"]),
            ("secure", 0.6, vec!["security"]),
            ("assured", 0.6, vec!["assurance"]),
            ("certain", 0.5, vec!["certainty"]),
            ("believing", 0.5, vec!["belief", "believe"]),
            ("faithful", 0.7, vec!["faith"]),
            ("reliable", 0.5, vec!["reliability"]),
        ]);

        // Disgust emotions
        self.add_emotion_words(EmotionType::Disgust, vec![
            ("disgusted", 0.8, vec!["disgust", "disgusting"]),
            ("revolted", 0.8, vec!["revolt", "revolting"]),
            ("repulsed", 0.8, vec!["repulsion", "repulsive"]),
            ("nauseated", 0.7, vec!["nausea", "nauseating"]),
            ("sickened", 0.7, vec!["sick", "sickening"]),
            ("appalled", 0.8, vec!["appalling"]),
            ("offended", 0.6, vec!["offense", "offensive"]),
        ]);

        // Surprise emotions
        self.add_emotion_words(EmotionType::Surprise, vec![
            ("surprised", 0.6, vec!["surprise", "surprising"]),
            ("amazed", 0.7, vec!["amazement", "amazing"]),
            ("astonished", 0.8, vec!["astonishment", "astonishing"]),
            ("shocked", 0.8, vec!["shock", "shocking"]),
            ("stunned", 0.8, vec!["stunning"]),
            ("bewildered", 0.6, vec!["bewilderment"]),
            ("flabbergasted", 0.8, vec![]),
            ("startled", 0.6, vec!["startle", "startling"]),
        ]);

        // Anticipation emotions
        self.add_emotion_words(EmotionType::Anticipation, vec![
            ("excited", 0.7, vec!["excitement", "exciting"]),
            ("eager", 0.6, vec!["eagerness", "eagerly"]),
            ("anticipating", 0.6, vec!["anticipation"]),
            ("expectant", 0.5, vec!["expectation", "expecting"]),
            ("hopeful", 0.6, vec!["hope", "hoping"]),
            ("optimistic", 0.6, vec!["optimism"]),
            ("enthusiastic", 0.7, vec!["enthusiasm"]),
        ]);

        // Love emotions
        self.add_emotion_words(EmotionType::Love, vec![
            ("love", 0.8, vec!["loving", "loved", "lovely"]),
            ("adore", 0.8, vec!["adoration", "adoring"]),
            ("cherish", 0.7, vec!["cherishing"]),
            ("affectionate", 0.7, vec!["affection"]),
            ("devoted", 0.8, vec!["devotion"]),
            ("passionate", 0.8, vec!["passion"]),
            ("romantic", 0.7, vec!["romance"]),
        ]);

        // Additional secondary emotions...
        self.add_emotion_words(EmotionType::Guilt, vec![
            ("guilty", 0.7, vec!["guilt"]),
            ("ashamed", 0.7, vec!["shame", "shameful"]),
            ("remorseful", 0.8, vec!["remorse"]),
            ("regretful", 0.6, vec!["regret", "regretting"]),
        ]);

        self.add_emotion_words(EmotionType::Pride, vec![
            ("proud", 0.7, vec!["pride"]),
            ("accomplished", 0.6, vec!["accomplishment"]),
            ("triumphant", 0.8, vec!["triumph"]),
            ("victorious", 0.8, vec!["victory"]),
        ]);

        self.add_emotion_words(EmotionType::Anxiety, vec![
            ("anxious", 0.7, vec!["anxiety"]),
            ("stressed", 0.7, vec!["stress", "stressful"]),
            ("tense", 0.6, vec!["tension"]),
            ("uneasy", 0.6, vec!["uneasiness"]),
            ("restless", 0.6, vec!["restlessness"]),
        ]);
    }

    fn add_emotion_words(&mut self, emotion: EmotionType, words: Vec<(&str, f64, Vec<&str>)>) {
        let emotion_words = words.into_iter().map(|(word, intensity, variants)| {
            EmotionWord {
                word: word.to_string(),
                base_intensity: intensity,
                variants: variants.into_iter().map(|v| v.to_string()).collect(),
            }
        }).collect();

        self.emotion_lexicon.insert(emotion, emotion_words);
    }

    fn initialize_intensity_modifiers(&mut self) {
        let modifiers = [
            ("extremely", 2.0), ("incredibly", 2.0), ("absolutely", 2.0),
            ("completely", 1.8), ("totally", 1.8), ("utterly", 1.8),
            ("very", 1.5), ("really", 1.4), ("quite", 1.3), ("rather", 1.2),
            ("somewhat", 0.8), ("slightly", 0.7), ("a bit", 0.6), ("kind of", 0.5),
            ("barely", 0.3), ("hardly", 0.3), ("scarcely", 0.3),
        ];

        for (modifier, multiplier) in modifiers.iter() {
            self.intensity_modifiers.insert(modifier.to_string(), *multiplier);
        }
    }

    fn initialize_contextual_patterns(&mut self) {
        let patterns = [
            (r"(?i)\bI\s+feel\s+(.*?)\s+about", EmotionType::Sadness, 0.2, 0.1),
            (r"(?i)\bmaking\s+me\s+(.*)", EmotionType::Anger, 0.3, 0.2),
            (r"(?i)\bI\s+can't\s+believe", EmotionType::Surprise, 0.4, 0.2),
            (r"(?i)\bI'm\s+so\s+(.*?)\s+that", EmotionType::Joy, 0.3, 0.1),
            (r"(?i)\bwhat\s+if\s+(.*)", EmotionType::Fear, 0.2, 0.1),
        ];

        for (pattern_str, emotion, intensity_boost, confidence_boost) in patterns.iter() {
            if let Ok(pattern) = Regex::new(pattern_str) {
                self.contextual_patterns.push(ContextualPattern {
                    pattern,
                    emotion: emotion.clone(),
                    intensity_boost: *intensity_boost,
                    confidence_boost: *confidence_boost,
                });
            }
        }
    }

    pub fn detect(&self, text: &str) -> Vec<EmotionResult> {
        let sentences = self.split_into_sentences(text);
        let mut results = Vec::new();

        for sentence in sentences {
            let emotions = self.detect_emotions_in_sentence(&sentence);
            if !emotions.is_empty() {
                let dominant_emotion = self.find_dominant_emotion(&emotions);
                let emotional_intensity = self.calculate_emotional_intensity(&emotions);
                let confidence = self.calculate_confidence(&emotions);

                results.push(EmotionResult {
                    emotions,
                    dominant_emotion,
                    emotional_intensity,
                    confidence,
                });
            }
        }

        results
    }

    fn detect_emotions_in_sentence(&self, sentence: &str) -> Vec<DetectedEmotion> {
        let mut detected_emotions: HashMap<EmotionType, DetectedEmotion> = HashMap::new();
        let words = self.tokenize(sentence);

        // Detect emotions from lexicon
        for (i, word) in words.iter().enumerate() {
            let lower_word = word.to_lowercase();

            for (emotion_type, emotion_words) in &self.emotion_lexicon {
                for emotion_word in emotion_words {
                    if emotion_word.word == lower_word || emotion_word.variants.contains(&lower_word) {
                        let intensity = self.calculate_intensity(&words, i, emotion_word.base_intensity);
                        let confidence = 0.8; // Base confidence for lexicon matches

                        if let Some(existing) = detected_emotions.get_mut(emotion_type) {
                            // Combine intensities if emotion already detected
                            existing.intensity = (existing.intensity + intensity) / 2.0;
                            existing.confidence = (existing.confidence + confidence) / 2.0;
                            existing.triggers.push(word.clone());
                        } else {
                            detected_emotions.insert(emotion_type.clone(), DetectedEmotion {
                                emotion_type: emotion_type.clone(),
                                intensity,
                                confidence,
                                triggers: vec![word.clone()],
                                context: sentence.to_string(),
                            });
                        }
                    }
                }
            }
        }

        // Apply contextual patterns
        for pattern in &self.contextual_patterns {
            if pattern.pattern.is_match(sentence) {
                if let Some(existing) = detected_emotions.get_mut(&pattern.emotion) {
                    existing.intensity += pattern.intensity_boost;
                    existing.confidence += pattern.confidence_boost;
                } else {
                    detected_emotions.insert(pattern.emotion.clone(), DetectedEmotion {
                        emotion_type: pattern.emotion.clone(),
                        intensity: pattern.intensity_boost,
                        confidence: pattern.confidence_boost,
                        triggers: vec!["contextual_pattern".to_string()],
                        context: sentence.to_string(),
                    });
                }
            }
        }

        detected_emotions.into_values().collect()
    }

    fn calculate_intensity(&self, words: &[String], word_index: usize, base_intensity: f64) -> f64 {
        let mut intensity = base_intensity;

        // Check for intensity modifiers in the previous 3 words
        for i in 1..=3 {
            if word_index >= i {
                let prev_word = &words[word_index - i].to_lowercase();
                if let Some(&multiplier) = self.intensity_modifiers.get(prev_word) {
                    intensity *= multiplier;
                    break;
                }
            }
        }

        intensity.min(1.0)
    }

    fn find_dominant_emotion(&self, emotions: &[DetectedEmotion]) -> Option<EmotionType> {
        emotions
            .iter()
            .max_by(|a, b| {
                let a_score = a.intensity * a.confidence;
                let b_score = b.intensity * b.confidence;
                a_score.partial_cmp(&b_score).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|emotion| emotion.emotion_type.clone())
    }

    fn calculate_emotional_intensity(&self, emotions: &[DetectedEmotion]) -> f64 {
        if emotions.is_empty() {
            return 0.0;
        }

        let total_weighted_intensity: f64 = emotions
            .iter()
            .map(|emotion| emotion.intensity * emotion.confidence)
            .sum();

        let total_confidence: f64 = emotions.iter().map(|emotion| emotion.confidence).sum();

        if total_confidence > 0.0 {
            total_weighted_intensity / total_confidence
        } else {
            0.0
        }
    }

    fn calculate_confidence(&self, emotions: &[DetectedEmotion]) -> f64 {
        if emotions.is_empty() {
            return 0.0;
        }

        let average_confidence: f64 = emotions.iter().map(|emotion| emotion.confidence).sum::<f64>() / emotions.len() as f64;
        let emotion_diversity = emotions.len() as f64 / 10.0; // Normalize by max expected emotions

        (average_confidence + emotion_diversity).min(1.0)
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        text.split_whitespace()
            .map(|word| word.trim_matches(|c: char| c.is_ascii_punctuation()).to_string())
            .filter(|word| !word.is_empty())
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

    pub fn add_custom_emotion_word(&mut self, emotion: EmotionType, word: &str, intensity: f64) {
        let emotion_word = EmotionWord {
            word: word.to_string(),
            base_intensity: intensity,
            variants: Vec::new(),
        };

        self.emotion_lexicon
            .entry(emotion)
            .or_insert_with(Vec::new)
            .push(emotion_word);
    }

    pub fn get_emotion_statistics(&self) -> HashMap<EmotionType, usize> {
        self.emotion_lexicon
            .iter()
            .map(|(emotion, words)| (emotion.clone(), words.len()))
            .collect()
    }
}

impl Default for EmotionDetector {
    fn default() -> Self {
        Self::new()
    }
}