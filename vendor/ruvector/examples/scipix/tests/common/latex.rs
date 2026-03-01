// LaTeX comparison and manipulation utilities
//
// Provides functions to normalize, compare, and analyze LaTeX strings

use std::collections::HashSet;

/// Normalize LaTeX string for comparison
pub fn normalize(latex: &str) -> String {
    latex
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

/// Check if two LaTeX expressions match semantically
pub fn expressions_match(a: &str, b: &str) -> bool {
    let norm_a = normalize(a);
    let norm_b = normalize(b);

    // Direct match
    if norm_a == norm_b {
        return true;
    }

    // Try alternative representations
    // e.g., \frac{1}{2} vs 0.5, x^{2} vs x^2, etc.

    // For now, use normalized comparison
    norm_a == norm_b
}

/// Calculate similarity between two LaTeX strings (0.0 to 1.0)
pub fn calculate_similarity(a: &str, b: &str) -> f64 {
    let norm_a = normalize(a);
    let norm_b = normalize(b);

    // Use Levenshtein distance ratio
    let distance = levenshtein_distance(&norm_a, &norm_b);
    let max_len = norm_a.len().max(norm_b.len()) as f64;

    if max_len == 0.0 {
        return 1.0;
    }

    1.0 - (distance as f64 / max_len)
}

/// Calculate Levenshtein distance between two strings
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

/// Extract LaTeX commands from string
pub fn extract_commands(latex: &str) -> HashSet<String> {
    let mut commands = HashSet::new();
    let mut chars = latex.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            let mut command = String::from("\\");
            while let Some(&next_ch) = chars.peek() {
                if next_ch.is_alphabetic() {
                    command.push(next_ch);
                    chars.next();
                } else {
                    break;
                }
            }
            if command.len() > 1 {
                commands.insert(command);
            }
        }
    }

    commands
}

/// Count LaTeX elements (fractions, superscripts, etc.)
pub fn count_elements(latex: &str) -> ElementCounts {
    let mut counts = ElementCounts::default();

    if latex.contains(r"\frac") {
        counts.fractions = latex.matches(r"\frac").count();
    }
    if latex.contains(r"\int") {
        counts.integrals = latex.matches(r"\int").count();
    }
    if latex.contains(r"\sum") {
        counts.sums = latex.matches(r"\sum").count();
    }
    if latex.contains("^") {
        counts.superscripts = latex.matches("^").count();
    }
    if latex.contains("_") {
        counts.subscripts = latex.matches("_").count();
    }
    if latex.contains(r"\begin{matrix}") || latex.contains(r"\begin{bmatrix}") {
        counts.matrices = 1;
    }

    counts
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ElementCounts {
    pub fractions: usize,
    pub integrals: usize,
    pub sums: usize,
    pub superscripts: usize,
    pub subscripts: usize,
    pub matrices: usize,
}

/// Validate LaTeX syntax (basic check)
pub fn validate_syntax(latex: &str) -> Result<(), String> {
    let mut brace_count = 0;
    let mut bracket_count = 0;

    for ch in latex.chars() {
        match ch {
            '{' => brace_count += 1,
            '}' => {
                brace_count -= 1;
                if brace_count < 0 {
                    return Err("Unmatched closing brace".to_string());
                }
            }
            '[' => bracket_count += 1,
            ']' => {
                bracket_count -= 1;
                if bracket_count < 0 {
                    return Err("Unmatched closing bracket".to_string());
                }
            }
            _ => {}
        }
    }

    if brace_count != 0 {
        return Err(format!("Unmatched braces: {} unclosed", brace_count));
    }
    if bracket_count != 0 {
        return Err(format!("Unmatched brackets: {} unclosed", bracket_count));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("x + y"), "x+y");
        assert_eq!(normalize("  a  b  "), "ab");
        assert_eq!(normalize(r"\frac{1}{2}"), r"\frac{1}{2}");
    }

    #[test]
    fn test_expressions_match() {
        assert!(expressions_match("x+y", "x + y"));
        assert!(expressions_match(r"\frac{1}{2}", r"\frac{1}{2}"));
        assert!(!expressions_match("x+y", "x-y"));
    }

    #[test]
    fn test_calculate_similarity() {
        assert!(calculate_similarity("abc", "abc") == 1.0);
        assert!(calculate_similarity("abc", "abd") > 0.6);
        assert!(calculate_similarity("abc", "xyz") < 0.5);
    }

    #[test]
    fn test_extract_commands() {
        let latex = r"\frac{1}{2} + \sqrt{x}";
        let commands = extract_commands(latex);
        assert!(commands.contains(r"\frac"));
        assert!(commands.contains(r"\sqrt"));
    }

    #[test]
    fn test_validate_syntax() {
        assert!(validate_syntax(r"\frac{1}{2}").is_ok());
        assert!(validate_syntax(r"\frac{1}{2").is_err());
        assert!(validate_syntax(r"\frac{1}2}").is_err());
    }
}
