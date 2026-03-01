//! Test Fixtures for RuvLTRA-Small
//!
//! This module provides test fixtures including sample prompts, expected patterns,
//! and perplexity baselines for validating the RuvLTRA-Small inference engine.

use std::collections::HashMap;

// ============================================================================
// Sample Prompts
// ============================================================================

/// Collection of test prompts organized by category
pub mod prompts {
    /// Simple text completion prompts
    pub mod completion {
        pub const QUICK_BROWN_FOX: &str = "The quick brown fox";
        pub const ONCE_UPON_A_TIME: &str = "Once upon a time";
        pub const IN_THE_BEGINNING: &str = "In the beginning";
        pub const IT_WAS_A_DARK: &str = "It was a dark and stormy night";
    }

    /// Instruction-following prompts
    pub mod instruction {
        pub const WRITE_HAIKU: &str = "Write a haiku about programming:";
        pub const EXPLAIN_GRAVITY: &str = "Explain gravity in simple terms:";
        pub const LIST_PLANETS: &str = "List the planets in our solar system:";
        pub const DESCRIBE_OCEAN: &str = "Describe the ocean in three sentences:";
    }

    /// Question-answering prompts
    pub mod qa {
        pub const CAPITAL_FRANCE: &str = "Q: What is the capital of France?\nA:";
        pub const TWO_PLUS_TWO: &str = "Q: What is 2 + 2?\nA:";
        pub const COLOR_SKY: &str = "Q: What color is the sky?\nA:";
        pub const LARGEST_PLANET: &str = "Q: What is the largest planet in our solar system?\nA:";
    }

    /// Code generation prompts
    pub mod code {
        pub const FIBONACCI: &str = "def fibonacci(n):\n    '''Return the nth Fibonacci number.'''\n";
        pub const HELLO_WORLD: &str = "# Python function to print hello world\ndef hello():";
        pub const FACTORIAL: &str = "def factorial(n):\n    '''Return n factorial.'''\n";
        pub const SORT_LIST: &str = "def sort_list(items):\n    '''Sort a list in ascending order.'''\n";
    }

    /// Conversation/chat prompts
    pub mod conversation {
        pub const GREETING: &str = "User: Hello!\nAssistant:";
        pub const TELL_JOKE: &str = "User: Tell me a joke.\nAssistant:";
        pub const WEATHER: &str = "User: What's the weather like today?\nAssistant:";
        pub const HELP: &str = "User: Can you help me?\nAssistant:";
    }

    /// Edge case prompts
    pub mod edge_cases {
        pub const EMPTY: &str = "";
        pub const SINGLE_CHAR: &str = "A";
        pub const SINGLE_WORD: &str = "Hello";
        pub const SPECIAL_CHARS: &str = "Translate: \"Hello, world!\" ->";
        pub const UNICODE: &str = "\u{4f60}\u{597d}\u{4e16}\u{754c}"; // 你好世界
        pub const NUMBERS_ONLY: &str = "1 2 3 4 5";
        pub const VERY_LONG: &str = "The quick brown fox jumps over the lazy dog. \
            The quick brown fox jumps over the lazy dog. \
            The quick brown fox jumps over the lazy dog. \
            The quick brown fox jumps over the lazy dog. \
            The quick brown fox jumps over the lazy dog. \
            Continue:";
    }
}

// ============================================================================
// Expected Output Patterns
// ============================================================================

/// Expected patterns in generated outputs
pub mod expected_patterns {
    /// Patterns expected after "The quick brown fox"
    pub const FOX_COMPLETION: &[&str] = &[
        "jumps", "jumped", "runs", "ran", "over", "the", "lazy", "dog"
    ];

    /// Patterns expected in haiku responses
    pub const HAIKU_PATTERNS: &[&str] = &[
        "code", "bug", "compile", "debug", "screen", "night", "lines", "function"
    ];

    /// Capital of France
    pub const FRANCE_CAPITAL: &str = "Paris";

    /// Answer to 2+2
    pub const TWO_PLUS_TWO: &str = "4";

    /// Patterns in Fibonacci code
    pub const FIBONACCI_PATTERNS: &[&str] = &[
        "return", "if", "else", "n", "<=", "1", "+", "fibonacci"
    ];

    /// Patterns in greeting responses
    pub const GREETING_PATTERNS: &[&str] = &[
        "hello", "hi", "hey", "how", "help", "assist", "welcome"
    ];

    /// Patterns in factorial code
    pub const FACTORIAL_PATTERNS: &[&str] = &[
        "return", "if", "n", "<=", "1", "*", "factorial"
    ];
}

// ============================================================================
// Perplexity Baselines
// ============================================================================

/// Perplexity baseline values for quality validation
pub mod perplexity {
    /// Maximum acceptable perplexity for coherent output
    pub const MAX_ACCEPTABLE: f32 = 50.0;

    /// Warning threshold for elevated perplexity
    pub const WARNING_THRESHOLD: f32 = 30.0;

    /// Excellent perplexity (high-quality output)
    pub const EXCELLENT: f32 = 15.0;

    /// Expected perplexity ranges by task type
    pub mod task_ranges {
        /// Simple completion: low perplexity expected
        pub const COMPLETION: (f32, f32) = (5.0, 20.0);

        /// Code generation: moderate perplexity
        pub const CODE: (f32, f32) = (8.0, 30.0);

        /// Creative writing: higher perplexity acceptable
        pub const CREATIVE: (f32, f32) = (15.0, 45.0);

        /// Factual QA: low perplexity (confident answers)
        pub const FACTUAL: (f32, f32) = (3.0, 15.0);
    }

    /// Quantization degradation limits
    pub mod degradation {
        /// Max perplexity increase from quantization (%)
        pub const MAX_INCREASE_PCT: f32 = 20.0;

        /// Q4_K expected degradation from F16 (%)
        pub const Q4K_EXPECTED: f32 = 15.0;

        /// Q8_0 expected degradation from F16 (%)
        pub const Q8_EXPECTED: f32 = 3.0;
    }
}

// ============================================================================
// Token Probability Thresholds
// ============================================================================

/// Thresholds for token probability validation
pub mod probability_thresholds {
    /// Minimum probability for top-1 token
    pub const MIN_TOP1: f32 = 0.01;

    /// Minimum cumulative probability for top-5 tokens
    pub const MIN_TOP5_CUMULATIVE: f32 = 0.1;

    /// Maximum entropy for non-degenerate output
    pub const MAX_ENTROPY: f32 = 10.0;

    /// Minimum confidence for factual answers
    pub const MIN_FACTUAL_CONFIDENCE: f32 = 0.5;
}

// ============================================================================
// Coherence Metrics
// ============================================================================

/// Coherence validation thresholds
pub mod coherence {
    /// Maximum consecutive word repetitions
    pub const MAX_CONSECUTIVE_REPEATS: usize = 3;

    /// Maximum n-gram repetition ratio
    pub const MAX_NGRAM_REPETITION: f32 = 0.3;

    /// Minimum alphanumeric ratio for valid text
    pub const MIN_ALPHANUMERIC_RATIO: f32 = 0.7;

    /// Maximum special character ratio
    pub const MAX_SPECIAL_CHAR_RATIO: f32 = 0.2;

    /// Sentence length bounds
    pub const MIN_SENTENCE_LENGTH: usize = 3;
    pub const MAX_SENTENCE_LENGTH: usize = 200;
}

// ============================================================================
// Performance Baselines
// ============================================================================

/// Performance baseline values
pub mod performance {
    /// Tokens per second baselines by device
    pub mod tokens_per_second {
        /// M4 Pro with ANE
        pub const M4_PRO_ANE: f32 = 60.0;

        /// M4 Pro NEON only
        pub const M4_PRO_NEON: f32 = 45.0;

        /// M1 with ANE
        pub const M1_ANE: f32 = 40.0;

        /// x86 CPU (AVX2)
        pub const X86_AVX2: f32 = 15.0;
    }

    /// Latency thresholds (milliseconds)
    pub mod latency_ms {
        /// Maximum time to first token
        pub const MAX_FIRST_TOKEN: u64 = 500;

        /// Maximum inter-token latency
        pub const MAX_INTER_TOKEN: u64 = 100;

        /// Target inter-token latency
        pub const TARGET_INTER_TOKEN: u64 = 20;
    }

    /// Memory thresholds (bytes)
    pub mod memory {
        /// Maximum model memory (Q4_K)
        pub const MAX_MODEL_Q4K: usize = 1_500_000_000;

        /// Maximum KV cache memory
        pub const MAX_KV_CACHE: usize = 500_000_000;

        /// Maximum working memory
        pub const MAX_WORKING: usize = 200_000_000;
    }
}

// ============================================================================
// Test Data Generators
// ============================================================================

/// Generate a long prompt of specified length
pub fn generate_long_prompt(word_count: usize) -> String {
    let words = [
        "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog",
        "and", "then", "runs", "around", "park", "with", "great", "joy"
    ];

    (0..word_count)
        .map(|i| words[i % words.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

/// Generate a sequence of numbers for pattern completion tests
pub fn generate_number_sequence(start: i32, count: usize) -> String {
    (start..start + count as i32)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Generate a repeated pattern prompt
pub fn generate_repetition_prompt(word: &str, count: usize) -> String {
    vec![word; count].join(" ")
}

// ============================================================================
// Validation Helpers
// ============================================================================

/// Check if output contains any of the expected patterns
pub fn contains_expected_pattern(output: &str, patterns: &[&str]) -> bool {
    let output_lower = output.to_lowercase();
    patterns.iter().any(|p| output_lower.contains(&p.to_lowercase()))
}

/// Calculate repetition ratio for n-grams
pub fn calculate_ngram_repetition(text: &str, n: usize) -> f32 {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < n {
        return 0.0;
    }

    let total_ngrams = words.len() - n + 1;
    let mut ngram_counts: HashMap<Vec<&str>, usize> = HashMap::new();

    for window in words.windows(n) {
        *ngram_counts.entry(window.to_vec()).or_insert(0) += 1;
    }

    let repeated = ngram_counts.values().filter(|&&c| c > 1).sum::<usize>();
    repeated as f32 / total_ngrams as f32
}

/// Count consecutive word repetitions
pub fn count_consecutive_repeats(text: &str) -> usize {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut max_repeats = 0;
    let mut current_repeats = 0;

    for i in 1..words.len() {
        if words[i] == words[i - 1] {
            current_repeats += 1;
            max_repeats = max_repeats.max(current_repeats);
        } else {
            current_repeats = 0;
        }
    }

    max_repeats
}

/// Calculate alphanumeric ratio
pub fn alphanumeric_ratio(text: &str) -> f32 {
    if text.is_empty() {
        return 0.0;
    }

    let alphanumeric = text.chars()
        .filter(|c| c.is_alphanumeric())
        .count();

    alphanumeric as f32 / text.len() as f32
}

/// Check if text passes basic coherence checks
pub fn is_coherent(text: &str) -> bool {
    // Check alphanumeric ratio
    if alphanumeric_ratio(text) < coherence::MIN_ALPHANUMERIC_RATIO {
        return false;
    }

    // Check repetition
    if count_consecutive_repeats(text) > coherence::MAX_CONSECUTIVE_REPEATS {
        return false;
    }

    // Check n-gram repetition
    if calculate_ngram_repetition(text, 3) > coherence::MAX_NGRAM_REPETITION {
        return false;
    }

    true
}

// ============================================================================
// Tests for Fixtures Module
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_long_prompt() {
        let prompt = generate_long_prompt(100);
        let word_count = prompt.split_whitespace().count();
        assert_eq!(word_count, 100);
    }

    #[test]
    fn test_generate_number_sequence() {
        let seq = generate_number_sequence(1, 5);
        assert_eq!(seq, "1, 2, 3, 4, 5");
    }

    #[test]
    fn test_contains_expected_pattern() {
        let output = "The fox jumps over the lazy dog";
        assert!(contains_expected_pattern(output, expected_patterns::FOX_COMPLETION));
    }

    #[test]
    fn test_ngram_repetition() {
        let no_repeat = "the quick brown fox jumps over";
        assert!(calculate_ngram_repetition(no_repeat, 2) < 0.1);

        let high_repeat = "the the the the the the";
        assert!(calculate_ngram_repetition(high_repeat, 2) > 0.5);
    }

    #[test]
    fn test_consecutive_repeats() {
        assert_eq!(count_consecutive_repeats("hello world"), 0);
        assert_eq!(count_consecutive_repeats("hello hello world"), 1);
        assert_eq!(count_consecutive_repeats("hello hello hello"), 2);
    }

    #[test]
    fn test_alphanumeric_ratio() {
        assert!(alphanumeric_ratio("Hello World") > 0.8);
        assert!(alphanumeric_ratio("!@#$%^&*()") < 0.1);
    }

    #[test]
    fn test_coherence_check() {
        assert!(is_coherent("The quick brown fox jumps over the lazy dog."));
        assert!(!is_coherent("!@#$%^&*()!@#$%^&*()!@#$%^&*()"));
        assert!(!is_coherent("the the the the the the the"));
    }
}
