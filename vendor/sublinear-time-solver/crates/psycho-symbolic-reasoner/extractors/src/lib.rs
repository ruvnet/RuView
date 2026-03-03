use wasm_bindgen::prelude::*;

pub mod sentiment;
pub mod preferences;
pub mod emotions;
pub mod patterns;

pub use sentiment::{SentimentAnalyzer, SentimentResult, SentimentScore};
pub use preferences::{PreferenceExtractor, PreferencePattern, PreferenceResult};
pub use emotions::{EmotionDetector, EmotionResult, EmotionType};
pub use patterns::{PatternMatcher, TextPattern};

// WASM bindings
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct TextExtractor {
    sentiment_analyzer: SentimentAnalyzer,
    preference_extractor: PreferenceExtractor,
    emotion_detector: EmotionDetector,
}

#[wasm_bindgen]
impl TextExtractor {
    #[wasm_bindgen(constructor)]
    pub fn new() -> TextExtractor {
        TextExtractor {
            sentiment_analyzer: SentimentAnalyzer::new(),
            preference_extractor: PreferenceExtractor::new(),
            emotion_detector: EmotionDetector::new(),
        }
    }

    #[wasm_bindgen]
    pub fn analyze_sentiment(&self, text: &str) -> String {
        let result = self.sentiment_analyzer.analyze(text);
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
    }

    #[wasm_bindgen]
    pub fn extract_preferences(&self, text: &str) -> String {
        let result = self.preference_extractor.extract(text);
        serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen]
    pub fn detect_emotions(&self, text: &str) -> String {
        let result = self.emotion_detector.detect(text);
        serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
    }

    #[wasm_bindgen]
    pub fn analyze_all(&self, text: &str) -> String {
        let sentiment = self.sentiment_analyzer.analyze(text);
        let preferences = self.preference_extractor.extract(text);
        let emotions = self.emotion_detector.detect(text);

        let result = serde_json::json!({
            "sentiment": sentiment,
            "preferences": preferences,
            "emotions": emotions
        });

        result.to_string()
    }
}

impl Default for TextExtractor {
    fn default() -> Self {
        Self::new()
    }
}