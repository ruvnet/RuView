// Metric calculation utilities
//
// Provides functions to calculate CER, WER, BLEU, and other quality metrics

/// Calculate Character Error Rate (CER)
pub fn calculate_cer(reference: &str, hypothesis: &str) -> f64 {
    let distance = levenshtein_distance(reference, hypothesis);
    let ref_len = reference.chars().count();

    if ref_len == 0 {
        return if hypothesis.is_empty() { 0.0 } else { 1.0 };
    }

    distance as f64 / ref_len as f64
}

/// Calculate Word Error Rate (WER)
pub fn calculate_wer(reference: &str, hypothesis: &str) -> f64 {
    let ref_words: Vec<&str> = reference.split_whitespace().collect();
    let hyp_words: Vec<&str> = hypothesis.split_whitespace().collect();

    let distance = word_levenshtein_distance(&ref_words, &hyp_words);
    let ref_len = ref_words.len();

    if ref_len == 0 {
        return if hyp_words.is_empty() { 0.0 } else { 1.0 };
    }

    distance as f64 / ref_len as f64
}

/// Calculate BLEU score
pub fn calculate_bleu(reference: &str, hypothesis: &str, max_n: usize) -> f64 {
    let ref_words: Vec<&str> = reference.split_whitespace().collect();
    let hyp_words: Vec<&str> = hypothesis.split_whitespace().collect();

    if hyp_words.is_empty() {
        return 0.0;
    }

    // Calculate n-gram precisions
    let mut precisions = Vec::new();
    for n in 1..=max_n {
        let precision = calculate_ngram_precision(&ref_words, &hyp_words, n);
        if precision == 0.0 {
            return 0.0; // BLEU is 0 if any n-gram precision is 0
        }
        precisions.push(precision);
    }

    // Geometric mean of precisions
    let geo_mean = precisions.iter().map(|p| p.ln()).sum::<f64>() / precisions.len() as f64;

    // Brevity penalty
    let bp = if hyp_words.len() >= ref_words.len() {
        1.0
    } else {
        (1.0 - (ref_words.len() as f64 / hyp_words.len() as f64)).exp()
    };

    bp * geo_mean.exp() * 100.0 // Return as percentage
}

/// Calculate precision for n-grams
fn calculate_ngram_precision(reference: &[&str], hypothesis: &[&str], n: usize) -> f64 {
    if hypothesis.len() < n {
        return 0.0;
    }

    let ref_ngrams = get_ngrams(reference, n);
    let hyp_ngrams = get_ngrams(hypothesis, n);

    if hyp_ngrams.is_empty() {
        return 0.0;
    }

    let mut matches = 0;
    for hyp_ngram in &hyp_ngrams {
        if ref_ngrams.contains(hyp_ngram) {
            matches += 1;
        }
    }

    matches as f64 / hyp_ngrams.len() as f64
}

/// Get n-grams from a sequence of words
fn get_ngrams(words: &[&str], n: usize) -> Vec<Vec<String>> {
    if words.len() < n {
        return vec![];
    }

    (0..=words.len() - n)
        .map(|i| words[i..i + n].iter().map(|s| s.to_string()).collect())
        .collect()
}

/// Calculate Levenshtein distance for characters
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };

            matrix[i][j] = *[
                matrix[i - 1][j] + 1,        // deletion
                matrix[i][j - 1] + 1,        // insertion
                matrix[i - 1][j - 1] + cost, // substitution
            ]
            .iter()
            .min()
            .unwrap();
        }
    }

    matrix[a_len][b_len]
}

/// Calculate Levenshtein distance for words
fn word_levenshtein_distance(a: &[&str], b: &[&str]) -> usize {
    let a_len = a.len();
    let b_len = b.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };

            matrix[i][j] = *[
                matrix[i - 1][j] + 1,        // deletion
                matrix[i][j - 1] + 1,        // insertion
                matrix[i - 1][j - 1] + cost, // substitution
            ]
            .iter()
            .min()
            .unwrap();
        }
    }

    matrix[a_len][b_len]
}

/// Calculate precision
pub fn calculate_precision(tp: usize, fp: usize) -> f64 {
    if tp + fp == 0 {
        return 0.0;
    }
    tp as f64 / (tp + fp) as f64
}

/// Calculate recall
pub fn calculate_recall(tp: usize, fn_count: usize) -> f64 {
    if tp + fn_count == 0 {
        return 0.0;
    }
    tp as f64 / (tp + fn_count) as f64
}

/// Calculate F1 score
pub fn calculate_f1(precision: f64, recall: f64) -> f64 {
    if precision + recall == 0.0 {
        return 0.0;
    }
    2.0 * (precision * recall) / (precision + recall)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cer() {
        assert_eq!(calculate_cer("abc", "abc"), 0.0);
        assert_eq!(calculate_cer("abc", "abd"), 1.0 / 3.0);
        assert_eq!(calculate_cer("abc", ""), 1.0);
    }

    #[test]
    fn test_wer() {
        assert_eq!(calculate_wer("hello world", "hello world"), 0.0);
        assert_eq!(calculate_wer("hello world", "hello earth"), 0.5);
    }

    #[test]
    fn test_bleu() {
        let bleu = calculate_bleu("the cat sat on the mat", "the cat sat on the mat", 4);
        assert!(bleu > 99.0);

        let bleu = calculate_bleu("the cat sat", "the dog sat", 2);
        assert!(bleu > 0.0 && bleu < 100.0);
    }

    #[test]
    fn test_precision_recall_f1() {
        let precision = calculate_precision(8, 2);
        assert_eq!(precision, 0.8);

        let recall = calculate_recall(8, 1);
        assert!((recall - 8.0 / 9.0).abs() < 0.001);

        let f1 = calculate_f1(precision, recall);
        assert!(f1 > 0.8);
    }
}
