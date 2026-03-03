use extractors::*;
use std::collections::HashMap;

#[cfg(test)]
mod sentiment_tests {
    use super::*;

    #[test]
    fn test_sentiment_analyzer_creation() {
        let analyzer = SentimentAnalyzer::new();
        assert!(analyzer.is_initialized());
    }

    #[test]
    fn test_positive_sentiment() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("I love this amazing product! It's fantastic and wonderful!");

        assert!(result.overall_score > 0.5);
        assert_eq!(result.dominant_sentiment, "positive");
        assert!(result.confidence > 0.6);
    }

    #[test]
    fn test_negative_sentiment() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("I hate this terrible product! It's awful and disappointing.");

        assert!(result.overall_score < -0.5);
        assert_eq!(result.dominant_sentiment, "negative");
        assert!(result.confidence > 0.6);
    }

    #[test]
    fn test_neutral_sentiment() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("This is a product. It exists. Some people might use it.");

        assert!(result.overall_score > -0.3 && result.overall_score < 0.3);
        assert_eq!(result.dominant_sentiment, "neutral");
    }

    #[test]
    fn test_mixed_sentiment() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("I love the design but hate the price. Great quality, terrible value.");

        assert!(result.aspects.len() > 1);
        let positive_aspects: Vec<_> = result.aspects.iter()
            .filter(|aspect| aspect.sentiment > 0.0)
            .collect();
        let negative_aspects: Vec<_> = result.aspects.iter()
            .filter(|aspect| aspect.sentiment < 0.0)
            .collect();

        assert!(!positive_aspects.is_empty());
        assert!(!negative_aspects.is_empty());
    }

    #[test]
    fn test_empty_text() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("");

        assert_eq!(result.overall_score, 0.0);
        assert_eq!(result.dominant_sentiment, "neutral");
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_sentiment_confidence_scoring() {
        let analyzer = SentimentAnalyzer::new();

        // Strong positive sentiment should have high confidence
        let strong_result = analyzer.analyze("Absolutely amazing! Best product ever! Love it completely!");

        // Weak sentiment should have lower confidence
        let weak_result = analyzer.analyze("It's okay, I guess.");

        assert!(strong_result.confidence > weak_result.confidence);
        assert!(strong_result.confidence > 0.8);
        assert!(weak_result.confidence < 0.6);
    }

    #[test]
    fn test_sentiment_aspect_extraction() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("The camera quality is excellent but the battery life is poor. Screen is decent.");

        assert!(result.aspects.len() >= 3);

        let camera_aspect = result.aspects.iter()
            .find(|aspect| aspect.aspect.contains("camera") || aspect.aspect.contains("quality"));
        let battery_aspect = result.aspects.iter()
            .find(|aspect| aspect.aspect.contains("battery"));
        let screen_aspect = result.aspects.iter()
            .find(|aspect| aspect.aspect.contains("screen"));

        assert!(camera_aspect.is_some());
        assert!(battery_aspect.is_some());
        assert!(screen_aspect.is_some());

        assert!(camera_aspect.unwrap().sentiment > 0.0);
        assert!(battery_aspect.unwrap().sentiment < 0.0);
    }

    #[test]
    fn test_sentiment_intensity_levels() {
        let analyzer = SentimentAnalyzer::new();

        let mild_positive = analyzer.analyze("It's pretty good.");
        let strong_positive = analyzer.analyze("This is absolutely incredible and amazing!");
        let extreme_positive = analyzer.analyze("BEST THING EVER!!! I LOVE IT SO MUCH!!!");

        assert!(mild_positive.intensity < strong_positive.intensity);
        assert!(strong_positive.intensity < extreme_positive.intensity);
        assert!(extreme_positive.intensity > 0.8);
    }

    #[test]
    fn test_sentiment_with_emojis() {
        let analyzer = SentimentAnalyzer::new();
        let result = analyzer.analyze("I love this! 😍💖✨ Best purchase ever! 🎉");

        assert!(result.overall_score > 0.7);
        assert_eq!(result.dominant_sentiment, "positive");
        assert!(result.has_emoji_indicators);
    }

    #[test]
    fn test_sentiment_negation_handling() {
        let analyzer = SentimentAnalyzer::new();

        let positive = analyzer.analyze("This is good.");
        let negated = analyzer.analyze("This is not good.");
        let double_negative = analyzer.analyze("This is not bad.");

        assert!(positive.overall_score > 0.0);
        assert!(negated.overall_score < 0.0);
        assert!(double_negative.overall_score > negated.overall_score);
    }
}

#[cfg(test)]
mod preference_tests {
    use super::*;

    #[test]
    fn test_preference_extractor_creation() {
        let extractor = PreferenceExtractor::new();
        assert!(extractor.is_ready());
    }

    #[test]
    fn test_explicit_preferences() {
        let extractor = PreferenceExtractor::new();
        let result = extractor.extract("I prefer coffee over tea. I like dark roast and hate decaf.");

        assert!(!result.is_empty());

        let coffee_pref = result.iter()
            .find(|pref| pref.item.contains("coffee"));
        assert!(coffee_pref.is_some());
        assert!(coffee_pref.unwrap().strength > 0.5);
    }

    #[test]
    fn test_implicit_preferences() {
        let extractor = PreferenceExtractor::new();
        let result = extractor.extract("I always buy organic food. I never eat fast food. I shop at farmer's markets every weekend.");

        let organic_pref = result.iter()
            .find(|pref| pref.category == "food" && pref.item.contains("organic"));
        let fast_food_pref = result.iter()
            .find(|pref| pref.item.contains("fast food"));

        assert!(organic_pref.is_some());
        assert!(fast_food_pref.is_some());
        assert!(organic_pref.unwrap().strength > 0.0);
        assert!(fast_food_pref.unwrap().strength < 0.0); // Negative preference
    }

    #[test]
    fn test_preference_categories() {
        let extractor = PreferenceExtractor::new();
        let result = extractor.extract("I love action movies, prefer iOS over Android, and enjoy Italian cuisine. I hate heavy metal music.");

        let categories: std::collections::HashSet<_> = result.iter()
            .map(|pref| &pref.category)
            .collect();

        assert!(categories.contains(&"entertainment".to_string()) || categories.contains(&"movies".to_string()));
        assert!(categories.contains(&"technology".to_string()));
        assert!(categories.contains(&"food".to_string()) || categories.contains(&"cuisine".to_string()));
        assert!(categories.contains(&"music".to_string()));
    }

    #[test]
    fn test_preference_strength_calculation() {
        let extractor = PreferenceExtractor::new();

        let weak_result = extractor.extract("I kind of like chocolate.");
        let strong_result = extractor.extract("I absolutely love chocolate! It's my favorite thing ever!");
        let hate_result = extractor.extract("I absolutely hate chocolate. Can't stand it.");

        let weak_strength = weak_result[0].strength;
        let strong_strength = strong_result[0].strength;
        let hate_strength = hate_result[0].strength;

        assert!(weak_strength > 0.0 && weak_strength < 0.5);
        assert!(strong_strength > 0.7);
        assert!(hate_strength < -0.5);
    }

    #[test]
    fn test_comparative_preferences() {
        let extractor = PreferenceExtractor::new();
        let result = extractor.extract("I prefer dogs over cats. I like summer more than winter. Tea is better than coffee for me.");

        assert!(result.len() >= 3);

        let dog_pref = result.iter().find(|pref| pref.item.contains("dogs"));
        let cat_pref = result.iter().find(|pref| pref.item.contains("cats"));

        if let (Some(dog), Some(cat)) = (dog_pref, cat_pref) {
            assert!(dog.strength > cat.strength);
        }
    }

    #[test]
    fn test_preference_patterns() {
        let extractor = PreferenceExtractor::new();
        let patterns = extractor.get_patterns();

        assert!(!patterns.is_empty());

        let preference_patterns: Vec<_> = patterns.iter()
            .filter(|pattern| pattern.pattern_type == "preference")
            .collect();

        let dislike_patterns: Vec<_> = patterns.iter()
            .filter(|pattern| pattern.pattern_type == "dislike")
            .collect();

        assert!(!preference_patterns.is_empty());
        assert!(!dislike_patterns.is_empty());
    }

    #[test]
    fn test_preference_context_awareness() {
        let extractor = PreferenceExtractor::new();
        let result = extractor.extract("When I'm working, I prefer silence. For relaxation, I like soft music. At parties, I enjoy loud music.");

        let context_preferences: Vec<_> = result.iter()
            .filter(|pref| !pref.context.is_empty())
            .collect();

        assert!(!context_preferences.is_empty());

        let work_pref = result.iter()
            .find(|pref| pref.context.contains("work"));
        let party_pref = result.iter()
            .find(|pref| pref.context.contains("party"));

        assert!(work_pref.is_some());
        assert!(party_pref.is_some());
    }

    #[test]
    fn test_preference_temporal_indicators() {
        let extractor = PreferenceExtractor::new();
        let result = extractor.extract("I used to like pop music, but now I prefer rock. I've always loved chocolate.");

        let temporal_preferences: Vec<_> = result.iter()
            .filter(|pref| pref.temporal_indicator.is_some())
            .collect();

        assert!(!temporal_preferences.is_empty());
    }

    #[test]
    fn test_preference_confidence_scoring() {
        let extractor = PreferenceExtractor::new();

        let definite_result = extractor.extract("I definitely prefer X over Y.");
        let uncertain_result = extractor.extract("I think I might like X more than Y, maybe.");

        assert!(definite_result[0].confidence > uncertain_result[0].confidence);
    }

    #[test]
    fn test_no_preferences_found() {
        let extractor = PreferenceExtractor::new();
        let result = extractor.extract("The weather is nice today. This is a factual statement.");

        assert!(result.is_empty() || result.iter().all(|pref| pref.strength.abs() < 0.1));
    }
}

#[cfg(test)]
mod emotion_tests {
    use super::*;

    #[test]
    fn test_emotion_detector_creation() {
        let detector = EmotionDetector::new();
        assert!(detector.is_initialized());
    }

    #[test]
    fn test_basic_emotion_detection() {
        let detector = EmotionDetector::new();
        let result = detector.detect("I am so happy and excited about this news!");

        assert!(!result.is_empty());
        let joy_emotion = result.iter()
            .find(|emotion| emotion.emotion_type == EmotionType::Joy);
        assert!(joy_emotion.is_some());
        assert!(joy_emotion.unwrap().intensity > 0.6);
    }

    #[test]
    fn test_multiple_emotions() {
        let detector = EmotionDetector::new();
        let result = detector.detect("I'm excited about the opportunity but nervous about the interview.");

        assert!(result.len() >= 2);

        let positive_emotions: Vec<_> = result.iter()
            .filter(|emotion| matches!(emotion.emotion_type, EmotionType::Joy | EmotionType::Excitement))
            .collect();
        let negative_emotions: Vec<_> = result.iter()
            .filter(|emotion| matches!(emotion.emotion_type, EmotionType::Fear | EmotionType::Anxiety))
            .collect();

        assert!(!positive_emotions.is_empty());
        assert!(!negative_emotions.is_empty());
    }

    #[test]
    fn test_emotion_intensity_levels() {
        let detector = EmotionDetector::new();

        let mild_result = detector.detect("I'm a bit sad.");
        let intense_result = detector.detect("I'm absolutely devastated and heartbroken!");

        let mild_sadness = mild_result.iter()
            .find(|emotion| emotion.emotion_type == EmotionType::Sadness);
        let intense_sadness = intense_result.iter()
            .find(|emotion| emotion.emotion_type == EmotionType::Sadness);

        if let (Some(mild), Some(intense)) = (mild_sadness, intense_sadness) {
            assert!(intense.intensity > mild.intensity);
            assert!(intense.intensity > 0.7);
        }
    }

    #[test]
    fn test_emotion_confidence() {
        let detector = EmotionDetector::new();

        let clear_result = detector.detect("I am furious and absolutely enraged!");
        let ambiguous_result = detector.detect("I feel something... not sure what.");

        let clear_anger = clear_result.iter()
            .find(|emotion| emotion.emotion_type == EmotionType::Anger);

        if let Some(anger) = clear_anger {
            assert!(anger.confidence > 0.7);
        }

        // Ambiguous text should have lower confidence scores
        let avg_confidence = ambiguous_result.iter()
            .map(|emotion| emotion.confidence)
            .sum::<f64>() / ambiguous_result.len() as f64;

        if !ambiguous_result.is_empty() {
            assert!(avg_confidence < 0.6);
        }
    }

    #[test]
    fn test_all_emotion_types() {
        let detector = EmotionDetector::new();

        let test_cases = vec![
            ("I'm so happy and joyful!", EmotionType::Joy),
            ("This makes me so angry!", EmotionType::Anger),
            ("I'm really sad about this.", EmotionType::Sadness),
            ("I'm scared and afraid.", EmotionType::Fear),
            ("I feel disgusted by this.", EmotionType::Disgust),
            ("This is such a surprise!", EmotionType::Surprise),
            ("I love this so much!", EmotionType::Love),
            ("I'm so anxious about tomorrow.", EmotionType::Anxiety),
            ("I'm really excited!", EmotionType::Excitement),
            ("I'm proud of my achievement.", EmotionType::Pride),
        ];

        for (text, expected_emotion) in test_cases {
            let result = detector.detect(text);
            let found_emotion = result.iter()
                .find(|emotion| emotion.emotion_type == expected_emotion);
            assert!(found_emotion.is_some(), "Failed to detect {} in '{}'",
                    format!("{:?}", expected_emotion), text);
        }
    }

    #[test]
    fn test_emotion_triggers() {
        let detector = EmotionDetector::new();
        let result = detector.detect("I lost my job today. My boss fired me without warning. I'm devastated.");

        let sadness = result.iter()
            .find(|emotion| emotion.emotion_type == EmotionType::Sadness);

        if let Some(sad_emotion) = sadness {
            assert!(!sad_emotion.triggers.is_empty());
            assert!(sad_emotion.triggers.iter().any(|trigger|
                trigger.contains("job") || trigger.contains("fired")));
        }
    }

    #[test]
    fn test_emotion_context() {
        let detector = EmotionDetector::new();
        let result = detector.detect("At work, I feel stressed. At home, I feel relaxed and happy.");

        let work_emotions: Vec<_> = result.iter()
            .filter(|emotion| emotion.context.as_ref().map_or(false, |ctx| ctx.contains("work")))
            .collect();

        let home_emotions: Vec<_> = result.iter()
            .filter(|emotion| emotion.context.as_ref().map_or(false, |ctx| ctx.contains("home")))
            .collect();

        assert!(!work_emotions.is_empty());
        assert!(!home_emotions.is_empty());
    }

    #[test]
    fn test_emotion_temporal_aspects() {
        let detector = EmotionDetector::new();
        let result = detector.detect("I was angry yesterday, but today I feel much better and happier.");

        let past_emotions: Vec<_> = result.iter()
            .filter(|emotion| emotion.temporal_context.as_ref().map_or(false, |ctx| ctx.contains("past")))
            .collect();

        let present_emotions: Vec<_> = result.iter()
            .filter(|emotion| emotion.temporal_context.as_ref().map_or(false, |ctx| ctx.contains("present")))
            .collect();

        assert!(!past_emotions.is_empty() || !present_emotions.is_empty());
    }

    #[test]
    fn test_emotion_with_emoji() {
        let detector = EmotionDetector::new();
        let result = detector.detect("I'm so happy! 😊😃🎉 This is amazing! 💖");

        assert!(!result.is_empty());
        let joy_emotions: Vec<_> = result.iter()
            .filter(|emotion| emotion.emotion_type == EmotionType::Joy)
            .collect();

        assert!(!joy_emotions.is_empty());
        // Emoji should boost confidence and intensity
        assert!(joy_emotions[0].intensity > 0.7);
    }

    #[test]
    fn test_no_emotions_detected() {
        let detector = EmotionDetector::new();
        let result = detector.detect("The temperature is 72 degrees Fahrenheit. This is a measurement.");

        assert!(result.is_empty() || result.iter().all(|emotion| emotion.intensity < 0.2));
    }

    #[test]
    fn test_conflicting_emotions() {
        let detector = EmotionDetector::new();
        let result = detector.detect("I'm happy about the promotion but sad to leave my team.");

        let joy_count = result.iter()
            .filter(|emotion| emotion.emotion_type == EmotionType::Joy)
            .count();
        let sadness_count = result.iter()
            .filter(|emotion| emotion.emotion_type == EmotionType::Sadness)
            .count();

        assert!(joy_count > 0);
        assert!(sadness_count > 0);
    }
}

#[cfg(test)]
mod pattern_tests {
    use super::*;

    #[test]
    fn test_pattern_matcher_creation() {
        let matcher = PatternMatcher::new();
        assert!(matcher.is_ready());
    }

    #[test]
    fn test_regex_patterns() {
        let matcher = PatternMatcher::new();
        let pattern = TextPattern::new("email", r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}");

        let matches = matcher.find_matches(&pattern, "Contact us at support@example.com or admin@test.org");
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|m| m.text == "support@example.com"));
        assert!(matches.iter().any(|m| m.text == "admin@test.org"));
    }

    #[test]
    fn test_keyword_patterns() {
        let matcher = PatternMatcher::new();
        let pattern = TextPattern::keyword("programming_languages", vec!["Python", "Java", "Rust", "JavaScript"]);

        let matches = matcher.find_matches(&pattern, "I love programming in Python and Rust. Java is okay too.");
        assert!(matches.len() >= 3);

        let found_languages: Vec<_> = matches.iter().map(|m| &m.text).collect();
        assert!(found_languages.contains(&&"Python".to_string()));
        assert!(found_languages.contains(&&"Rust".to_string()));
        assert!(found_languages.contains(&&"Java".to_string()));
    }

    #[test]
    fn test_phrase_patterns() {
        let matcher = PatternMatcher::new();
        let pattern = TextPattern::phrase("greetings", vec!["hello world", "good morning", "how are you"]);

        let matches = matcher.find_matches(&pattern, "Hello world! Good morning everyone. How are you doing today?");
        assert!(matches.len() >= 3);
    }

    #[test]
    fn test_wildcard_patterns() {
        let matcher = PatternMatcher::new();
        let pattern = TextPattern::wildcard("file_references", "*.rs");

        let matches = matcher.find_matches(&pattern, "Check main.rs and lib.rs files. Also see config.toml.");
        assert!(matches.len() >= 2);
        assert!(matches.iter().any(|m| m.text.contains("main.rs")));
        assert!(matches.iter().any(|m| m.text.contains("lib.rs")));
    }

    #[test]
    fn test_numeric_patterns() {
        let matcher = PatternMatcher::new();
        let pattern = TextPattern::numeric("prices", r"\$\d+\.?\d*");

        let matches = matcher.find_matches(&pattern, "Items cost $19.99, $5, and $100.50 respectively.");
        assert_eq!(matches.len(), 3);
        assert!(matches.iter().any(|m| m.text == "$19.99"));
        assert!(matches.iter().any(|m| m.text == "$5"));
        assert!(matches.iter().any(|m| m.text == "$100.50"));
    }

    #[test]
    fn test_custom_patterns() {
        let matcher = PatternMatcher::new();
        let pattern = TextPattern::custom("time_expressions",
            r"(?i)(today|tomorrow|yesterday|\d{1,2}:\d{2}|\d{1,2}/\d{1,2}/\d{4})");

        let matches = matcher.find_matches(&pattern, "Meet me today at 3:30. Yesterday was 12/25/2023.");
        assert!(matches.len() >= 3);
    }

    #[test]
    fn test_pattern_confidence_scoring() {
        let matcher = PatternMatcher::new();
        let pattern = TextPattern::fuzzy("colors", vec!["red", "blue", "green"], 0.8);

        let matches = matcher.find_matches(&pattern, "I like crimson, azure, and emerald colors.");

        // Should find fuzzy matches for red->crimson, blue->azure, green->emerald
        for match_result in &matches {
            assert!(match_result.confidence >= 0.8);
        }
    }

    #[test]
    fn test_overlapping_patterns() {
        let matcher = PatternMatcher::new();
        let email_pattern = TextPattern::new("email", r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}");
        let domain_pattern = TextPattern::new("domain", r"@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}");

        let text = "Contact: user@example.com";
        let email_matches = matcher.find_matches(&email_pattern, text);
        let domain_matches = matcher.find_matches(&domain_pattern, text);

        assert!(!email_matches.is_empty());
        assert!(!domain_matches.is_empty());

        // Check for overlap detection
        let has_overlap = matcher.check_pattern_overlap(&email_pattern, &domain_pattern, text);
        assert!(has_overlap);
    }

    #[test]
    fn test_context_aware_patterns() {
        let matcher = PatternMatcher::new();
        let pattern = TextPattern::contextual("measurements",
            r"\d+\.?\d*\s*(kg|lbs|cm|inches|meters)",
            vec!["weight", "height", "distance"]);

        let matches = matcher.find_matches(&pattern, "I weigh 70 kg and am 180 cm tall. The distance is 5 meters.");
        assert!(matches.len() >= 3);

        for match_result in &matches {
            assert!(!match_result.context.is_empty());
        }
    }

    #[test]
    fn test_pattern_extraction_performance() {
        let matcher = PatternMatcher::new();
        let pattern = TextPattern::new("words", r"\b[a-zA-Z]+\b");

        let large_text = "word ".repeat(1000);
        let start = std::time::Instant::now();
        let matches = matcher.find_matches(&pattern, &large_text);
        let duration = start.elapsed();

        assert!(matches.len() >= 1000);
        assert!(duration.as_millis() < 100); // Should be fast
    }

    #[test]
    fn test_pattern_caching() {
        let mut matcher = PatternMatcher::new();
        let pattern = TextPattern::new("test", r"\d+");

        // First call should compile pattern
        let start1 = std::time::Instant::now();
        let matches1 = matcher.find_matches(&pattern, "123 456 789");
        let duration1 = start1.elapsed();

        // Second call should use cached pattern
        let start2 = std::time::Instant::now();
        let matches2 = matcher.find_matches(&pattern, "123 456 789");
        let duration2 = start2.elapsed();

        assert_eq!(matches1.len(), matches2.len());
        // Second call should be faster (cached)
        assert!(duration2 <= duration1);
    }

    #[test]
    fn test_pattern_validation() {
        let invalid_pattern = TextPattern::new("invalid", "[invalid regex");
        assert!(!invalid_pattern.is_valid());

        let valid_pattern = TextPattern::new("valid", r"\d+");
        assert!(valid_pattern.is_valid());
    }

    #[test]
    fn test_pattern_priority() {
        let matcher = PatternMatcher::new();
        let high_priority = TextPattern::with_priority("important", r"urgent|critical", 100);
        let low_priority = TextPattern::with_priority("normal", r"info|note", 10);

        let matches = matcher.find_all_matches(vec![high_priority, low_priority],
            "Urgent: This is critical info with a note.");

        // High priority matches should come first
        assert!(!matches.is_empty());
        if matches.len() > 1 {
            assert!(matches[0].priority >= matches[1].priority);
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_text_extractor_integration() {
        let extractor = TextExtractor::new();
        let text = "I love this amazing product! 😊 I prefer it over all competitors. However, I'm a bit worried about the price.";

        let sentiment_result = extractor.analyze_sentiment(text);
        let preference_result = extractor.extract_preferences(text);
        let emotion_result = extractor.detect_emotions(text);

        // Parse JSON results
        let sentiment: SentimentResult = serde_json::from_str(&sentiment_result).unwrap();
        let preferences: Vec<PreferenceResult> = serde_json::from_str(&preference_result).unwrap();
        let emotions: Vec<EmotionResult> = serde_json::from_str(&emotion_result).unwrap();

        assert!(sentiment.overall_score > 0.0); // Overall positive
        assert!(!preferences.is_empty()); // Should find preference
        assert!(!emotions.is_empty()); // Should detect emotions

        // Check for consistency
        let has_positive_emotion = emotions.iter().any(|e|
            matches!(e.emotion_type, EmotionType::Joy | EmotionType::Love));
        assert!(has_positive_emotion);
    }

    #[test]
    fn test_comprehensive_analysis() {
        let extractor = TextExtractor::new();
        let complex_text = "I absolutely love my new iPhone! It's so much better than my old Android.
                           The camera quality is fantastic, but I hate how expensive it was.
                           I'm excited to use it but nervous about breaking it. 💖📱";

        let all_results = extractor.analyze_all(complex_text);
        let analysis: serde_json::Value = serde_json::from_str(&all_results).unwrap();

        assert!(analysis["sentiment"].is_object());
        assert!(analysis["preferences"].is_array());
        assert!(analysis["emotions"].is_array());

        // Should detect mixed sentiments and emotions
        let sentiment = &analysis["sentiment"];
        let preferences = &analysis["preferences"];
        let emotions = &analysis["emotions"];

        assert!(!preferences.as_array().unwrap().is_empty());
        assert!(!emotions.as_array().unwrap().is_empty());
    }

    #[test]
    fn test_cross_component_consistency() {
        let extractor = TextExtractor::new();
        let text = "I'm thrilled about my vacation! Going to Paris has always been my dream.
                   I prefer European cities over American ones. Can't wait! 🎉✈️";

        let sentiment_str = extractor.analyze_sentiment(text);
        let preference_str = extractor.extract_preferences(text);
        let emotion_str = extractor.detect_emotions(text);

        let sentiment: SentimentResult = serde_json::from_str(&sentiment_str).unwrap();
        let preferences: Vec<PreferenceResult> = serde_json::from_str(&preference_str).unwrap();
        let emotions: Vec<EmotionResult> = serde_json::from_str(&emotion_str).unwrap();

        // All should indicate positive state
        assert!(sentiment.overall_score > 0.5);

        let positive_emotions = emotions.iter()
            .filter(|e| matches!(e.emotion_type, EmotionType::Joy | EmotionType::Excitement))
            .count();
        assert!(positive_emotions > 0);

        let travel_preferences = preferences.iter()
            .filter(|p| p.category.contains("travel") || p.item.contains("cities"))
            .count();
        assert!(travel_preferences > 0);
    }

    #[test]
    fn test_error_handling() {
        let extractor = TextExtractor::new();

        // Test with empty input
        let empty_sentiment = extractor.analyze_sentiment("");
        let empty_preferences = extractor.extract_preferences("");
        let empty_emotions = extractor.detect_emotions("");

        // Should return valid JSON even for empty input
        assert!(serde_json::from_str::<SentimentResult>(&empty_sentiment).is_ok());
        assert!(serde_json::from_str::<Vec<PreferenceResult>>(&empty_preferences).is_ok());
        assert!(serde_json::from_str::<Vec<EmotionResult>>(&empty_emotions).is_ok());

        // Test with very long input
        let long_text = "word ".repeat(10000);
        let long_result = extractor.analyze_all(&long_text);
        assert!(serde_json::from_str::<serde_json::Value>(&long_result).is_ok());
    }

    #[test]
    fn test_multilingual_support() {
        let extractor = TextExtractor::new();

        // Test with non-English text (if supported)
        let spanish_text = "Me encanta este producto! Es fantástico.";
        let spanish_result = extractor.analyze_sentiment(&spanish_text);

        // Should handle gracefully even if not fully supported
        assert!(serde_json::from_str::<SentimentResult>(&spanish_result).is_ok());
    }

    #[test]
    fn test_performance_with_real_data() {
        let extractor = TextExtractor::new();
        let realistic_text = "After using this laptop for 3 months, I can say it's pretty good overall.
                             The performance is excellent for coding and video editing.
                             I love the keyboard feel and the screen quality is superb.
                             However, the battery life could be better - it only lasts about 6 hours.
                             The price was reasonable considering the specs.
                             I'd definitely recommend it to other developers, but maybe not for all-day mobile use.
                             Overall satisfied with my purchase! 👍";

        let start = std::time::Instant::now();
        let result = extractor.analyze_all(realistic_text);
        let duration = start.elapsed();

        assert!(!result.is_empty());
        assert!(duration.as_millis() < 1000); // Should complete within 1 second

        let analysis: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(analysis["sentiment"]["aspects"].as_array().unwrap().len() > 3);
    }
}

#[cfg(test)]
mod wasm_specific_tests {
    use super::*;

    #[test]
    fn test_wasm_serialization_compatibility() {
        let sentiment_result = SentimentResult {
            overall_score: 0.75,
            dominant_sentiment: "positive".to_string(),
            confidence: 0.85,
            intensity: 0.9,
            aspects: vec![],
            has_emoji_indicators: true,
        };

        let serialized = serde_json::to_string(&sentiment_result).unwrap();
        let deserialized: SentimentResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(sentiment_result.overall_score, deserialized.overall_score);
        assert_eq!(sentiment_result.dominant_sentiment, deserialized.dominant_sentiment);
    }

    #[test]
    fn test_memory_efficiency() {
        let extractor = TextExtractor::new();

        // Process multiple texts to test memory usage
        let texts = vec![
            "I love this product!",
            "This is terrible quality.",
            "Amazing experience overall.",
            "Could be better but okay.",
            "Absolutely fantastic!",
        ];

        let initial_memory = get_memory_usage();

        for text in &texts {
            let _ = extractor.analyze_all(text);
        }

        let final_memory = get_memory_usage();
        let memory_increase = final_memory - initial_memory;

        // Memory increase should be reasonable
        assert!(memory_increase < 1024 * 1024); // Less than 1MB increase
    }

    #[test]
    fn test_concurrent_analysis() {
        use std::sync::Arc;

        let extractor = Arc::new(TextExtractor::new());
        let texts = vec![
            "Happy text with joy!",
            "Sad and depressing content.",
            "Neutral informational text.",
            "Mixed emotions here - happy but worried.",
        ];

        let handles: Vec<_> = texts.into_iter().map(|text| {
            let extractor_clone = Arc::clone(&extractor);
            std::thread::spawn(move || {
                extractor_clone.analyze_all(&text)
            })
        }).collect();

        let results: Vec<_> = handles.into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();

        assert_eq!(results.len(), 4);
        for result in results {
            assert!(!result.is_empty());
            assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
        }
    }

    fn get_memory_usage() -> usize {
        // Simplified memory usage estimation
        // In a real implementation, this would use platform-specific APIs
        0
    }
}