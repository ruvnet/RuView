use extractors::*;
use extractors::sentiment::{SentimentAnalyzer, SentimentLabel};
use extractors::preferences::{PreferenceExtractor, PreferenceType, PreferenceStrength};
use extractors::emotions::{EmotionDetector, EmotionType};
use extractors::patterns::{PatternMatcher};

#[test]
fn test_sentiment_analysis_positive() {
    let analyzer = SentimentAnalyzer::new();

    let text = "I absolutely love this amazing product! It's fantastic and wonderful.";
    let result = analyzer.analyze(text);

    assert!(matches!(result.label, SentimentLabel::VeryPositive | SentimentLabel::Positive));
    assert!(result.score > 0.5);
    assert!(!result.positive_words.is_empty());
    assert!(result.positive_words.contains(&"love".to_string()));
    assert!(result.positive_words.contains(&"amazing".to_string()));
}

#[test]
fn test_sentiment_analysis_negative() {
    let analyzer = SentimentAnalyzer::new();

    let text = "This is terrible and awful. I hate it completely. It's the worst thing ever.";
    let result = analyzer.analyze(text);

    assert!(matches!(result.label, SentimentLabel::VeryNegative | SentimentLabel::Negative));
    assert!(result.score < -0.5);
    assert!(!result.negative_words.is_empty());
    assert!(result.negative_words.contains(&"terrible".to_string()));
    assert!(result.negative_words.contains(&"hate".to_string()));
}

#[test]
fn test_sentiment_analysis_neutral() {
    let analyzer = SentimentAnalyzer::new();

    let text = "The weather today is cloudy. There are some birds in the tree.";
    let result = analyzer.analyze(text);

    assert!(matches!(result.label, SentimentLabel::Neutral));
    assert!(result.score.abs() < 0.3);
}

#[test]
fn test_sentiment_intensifiers() {
    let analyzer = SentimentAnalyzer::new();

    let text1 = "I like this product.";
    let text2 = "I really like this product.";
    let text3 = "I absolutely love this product.";

    let result1 = analyzer.analyze(text1);
    let result2 = analyzer.analyze(text2);
    let result3 = analyzer.analyze(text3);

    // More intense expressions should have higher scores
    assert!(result2.score > result1.score);
    assert!(result3.score > result2.score);
}

#[test]
fn test_sentiment_negation() {
    let analyzer = SentimentAnalyzer::new();

    let text1 = "This is good.";
    let text2 = "This is not good.";

    let result1 = analyzer.analyze(text1);
    let result2 = analyzer.analyze(text2);

    // Negated positive should be negative
    assert!(result1.score > 0.0);
    assert!(result2.score < 0.0);
}

#[test]
fn test_preference_extraction_simple() {
    let extractor = PreferenceExtractor::new();

    let text = "I like chocolate ice cream.";
    let results = extractor.extract(text);

    assert!(!results.is_empty());
    let preferences = &results[0].preferences;
    assert!(!preferences.is_empty());

    let pref = &preferences[0];
    assert!(matches!(pref.preference_type, PreferenceType::Like));
    assert!(pref.preferred_item.contains("chocolate") || pref.preferred_item.contains("ice cream"));
}

#[test]
fn test_preference_extraction_comparison() {
    let extractor = PreferenceExtractor::new();

    let text = "I prefer coffee over tea.";
    let results = extractor.extract(text);

    assert!(!results.is_empty());
    let preferences = &results[0].preferences;
    assert!(!preferences.is_empty());

    let pref = &preferences[0];
    assert!(matches!(pref.preference_type, PreferenceType::Prefer));
    assert!(pref.preferred_item.contains("coffee"));
    assert!(pref.alternative_item.is_some());
    assert!(pref.alternative_item.as_ref().unwrap().contains("tea"));
}

#[test]
fn test_preference_extraction_dislike() {
    let extractor = PreferenceExtractor::new();

    let text = "I really dislike spicy food.";
    let results = extractor.extract(text);

    assert!(!results.is_empty());
    let preferences = &results[0].preferences;
    assert!(!preferences.is_empty());

    let pref = &preferences[0];
    assert!(matches!(pref.preference_type, PreferenceType::Dislike));
    assert!(pref.preferred_item.contains("spicy food"));
    assert!(matches!(pref.strength, PreferenceStrength::Strong));
}

#[test]
fn test_preference_extraction_want_need() {
    let extractor = PreferenceExtractor::new();

    let text1 = "I want a new laptop.";
    let text2 = "I desperately need more sleep.";

    let results1 = extractor.extract(text1);
    let results2 = extractor.extract(text2);

    assert!(!results1.is_empty());
    assert!(!results2.is_empty());

    let pref1 = &results1[0].preferences[0];
    let pref2 = &results2[0].preferences[0];

    assert!(matches!(pref1.preference_type, PreferenceType::Want));
    assert!(matches!(pref2.preference_type, PreferenceType::Need));
    assert!(matches!(pref2.strength, PreferenceStrength::VeryStrong));
}

#[test]
fn test_emotion_detection_joy() {
    let detector = EmotionDetector::new();

    let text = "I'm so happy and excited about this wonderful news!";
    let results = detector.detect(text);

    assert!(!results.is_empty());
    let emotions = &results[0].emotions;
    assert!(!emotions.is_empty());

    let has_joy = emotions.iter().any(|e| matches!(e.emotion_type, EmotionType::Joy));
    let has_excitement = emotions.iter().any(|e| matches!(e.emotion_type, EmotionType::Excitement));

    assert!(has_joy || has_excitement);
}

#[test]
fn test_emotion_detection_anger() {
    let detector = EmotionDetector::new();

    let text = "I'm absolutely furious and outraged about this situation!";
    let results = detector.detect(text);

    assert!(!results.is_empty());
    let emotions = &results[0].emotions;
    assert!(!emotions.is_empty());

    let has_anger = emotions.iter().any(|e| matches!(e.emotion_type, EmotionType::Anger));
    assert!(has_anger);

    // Check for high intensity
    let anger_emotion = emotions.iter().find(|e| matches!(e.emotion_type, EmotionType::Anger));
    if let Some(emotion) = anger_emotion {
        assert!(emotion.intensity > 0.7);
    }
}

#[test]
fn test_emotion_detection_fear() {
    let detector = EmotionDetector::new();

    let text = "I'm terrified and really scared about what might happen.";
    let results = detector.detect(text);

    assert!(!results.is_empty());
    let emotions = &results[0].emotions;
    assert!(!emotions.is_empty());

    let has_fear = emotions.iter().any(|e| matches!(e.emotion_type, EmotionType::Fear));
    assert!(has_fear);
}

#[test]
fn test_emotion_detection_multiple_emotions() {
    let detector = EmotionDetector::new();

    let text = "I'm excited but also nervous about starting this new job tomorrow.";
    let results = detector.detect(text);

    assert!(!results.is_empty());
    let emotions = &results[0].emotions;

    // Should detect both positive (excited) and negative (nervous) emotions
    assert!(emotions.len() >= 2);

    let has_positive = emotions.iter().any(|e|
        matches!(e.emotion_type, EmotionType::Joy | EmotionType::Excitement | EmotionType::Anticipation)
    );
    let has_negative = emotions.iter().any(|e|
        matches!(e.emotion_type, EmotionType::Fear | EmotionType::Anxiety)
    );

    assert!(has_positive);
    assert!(has_negative);
}

#[test]
fn test_emotion_intensity_modifiers() {
    let detector = EmotionDetector::new();

    let text1 = "I'm happy.";
    let text2 = "I'm extremely happy.";

    let results1 = detector.detect(text1);
    let results2 = detector.detect(text2);

    assert!(!results1.is_empty() && !results2.is_empty());

    let intensity1 = results1[0].emotional_intensity;
    let intensity2 = results2[0].emotional_intensity;

    assert!(intensity2 > intensity1);
}

#[test]
fn test_pattern_matcher_email() {
    let mut matcher = PatternMatcher::new();

    // Add common patterns
    let patterns = PatternMatcher::create_common_patterns();
    for pattern in patterns {
        matcher.add_pattern(pattern);
    }

    let text = "Please contact me at john.doe@example.com for more information.";
    let matches = matcher.match_text(text);

    let email_matches: Vec<_> = matches.iter()
        .filter(|m| m.pattern_name == "email_pattern")
        .collect();

    assert!(!email_matches.is_empty());
    assert!(email_matches[0].matched_text.contains("john.doe@example.com"));
}

#[test]
fn test_pattern_matcher_phone() {
    let mut matcher = PatternMatcher::new();

    let patterns = PatternMatcher::create_common_patterns();
    for pattern in patterns {
        matcher.add_pattern(pattern);
    }

    let text = "Call me at (555) 123-4567 or 555.987.6543.";
    let matches = matcher.match_text(text);

    let phone_matches: Vec<_> = matches.iter()
        .filter(|m| m.pattern_name == "phone_pattern")
        .collect();

    assert!(!phone_matches.is_empty());
}

#[test]
fn test_pattern_matcher_url() {
    let mut matcher = PatternMatcher::new();

    let patterns = PatternMatcher::create_common_patterns();
    for pattern in patterns {
        matcher.add_pattern(pattern);
    }

    let text = "Visit our website at https://www.example.com for more details.";
    let matches = matcher.match_text(text);

    let url_matches: Vec<_> = matches.iter()
        .filter(|m| m.pattern_name == "url_pattern")
        .collect();

    assert!(!url_matches.is_empty());
    assert!(url_matches[0].matched_text.contains("https://www.example.com"));
}

#[test]
fn test_text_extractor_integration() {
    let extractor = TextExtractor::new();

    let text = "I absolutely love this new restaurant! The food is amazing and the service is excellent. I'm so excited to go back soon.";

    // Test sentiment analysis
    let sentiment_json = extractor.analyze_sentiment(text);
    assert!(!sentiment_json.contains("error"));

    // Test preference extraction
    let preferences_json = extractor.extract_preferences(text);
    assert!(!preferences_json.contains("error"));

    // Test emotion detection
    let emotions_json = extractor.detect_emotions(text);
    assert!(!emotions_json.contains("error"));

    // Test combined analysis
    let all_analysis_json = extractor.analyze_all(text);
    assert!(!all_analysis_json.contains("error"));
    assert!(all_analysis_json.contains("sentiment"));
    assert!(all_analysis_json.contains("preferences"));
    assert!(all_analysis_json.contains("emotions"));
}

#[test]
fn test_custom_patterns() {
    let mut extractor = PreferenceExtractor::new();

    // Add custom pattern
    let custom_pattern = r"(?i)\b(I|we)\s+(adore|worship|treasure)\s+(.*?)(?:\.|,|;|!|\?|$)";
    let result = extractor.add_custom_pattern(custom_pattern, PreferenceType::Like, 0.9);
    assert!(result.is_ok());

    let text = "I absolutely adore dark chocolate.";
    let results = extractor.extract(text);

    assert!(!results.is_empty());
    let preferences = &results[0].preferences;
    assert!(!preferences.is_empty());

    let has_like = preferences.iter().any(|p| matches!(p.preference_type, PreferenceType::Like));
    assert!(has_like);
}

#[test]
fn test_confidence_scoring() {
    let analyzer = SentimentAnalyzer::new();

    let text1 = "This is good.";  // Simple positive
    let text2 = "This is absolutely amazing and fantastic!";  // Strong positive with multiple words

    let result1 = analyzer.analyze(text1);
    let result2 = analyzer.analyze(text2);

    // Result2 should have higher confidence due to multiple positive words
    assert!(result2.confidence >= result1.confidence);
}

// WASM tests would require wasm-bindgen-test crate
// These are commented out for basic functionality testing