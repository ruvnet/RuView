//! Pure helper functions used by the WASM bindings.
//!
//! These functions have no WASM dependencies and can be tested on any target.

/// Cosine similarity between two vectors.
///
/// Returns 0.0 when either vector has zero magnitude.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "vectors must be same length");

    let mut dot: f32 = 0.0;
    let mut mag_a: f32 = 0.0;
    let mut mag_b: f32 = 0.0;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        mag_a += a[i] * a[i];
        mag_b += b[i] * b[i];
    }

    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

/// Produce a deterministic pseudo-embedding from text using a simple hash.
///
/// The algorithm:
/// 1. Hash each character position into a seed.
/// 2. Use the seed to generate a float in [-1, 1].
/// 3. L2-normalise the resulting vector.
///
/// This is NOT a real embedding model -- it is only useful for demos and
/// testing that the WASM plumbing works end-to-end.
pub fn hash_embed(text: &str, dimension: usize) -> Vec<f32> {
    let mut vec = vec![0.0f32; dimension];
    let bytes = text.as_bytes();

    for (i, slot) in vec.iter_mut().enumerate() {
        // Mix byte values into the slot.
        let mut h: u64 = 0xcbf29ce484222325; // FNV-1a offset basis
        for (j, &b) in bytes.iter().enumerate() {
            h ^= (b as u64)
                .wrapping_add((i as u64).wrapping_mul(31))
                .wrapping_add(j as u64);
            h = h.wrapping_mul(0x100000001b3); // FNV-1a prime
        }
        // Map to [-1, 1].
        *slot = ((h as i64) as f64 / i64::MAX as f64) as f32;
    }

    // L2 normalise.
    let mag: f32 = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if mag > 0.0 {
        for v in &mut vec {
            *v /= mag;
        }
    }

    vec
}

/// Check for credit-card-like patterns: 4 groups of 4 digits separated by
/// spaces or dashes (or no separator).
pub fn has_credit_card_pattern(content: &str) -> bool {
    // Strategy: scan for sequences of 16 digits (possibly with separators).
    let digits_only: String = content.chars().filter(|c| c.is_ascii_digit()).collect();

    // Quick check: must have at least 16 digits somewhere.
    if digits_only.len() < 16 {
        return false;
    }

    // Look for the formatted pattern: DDDD[-/ ]DDDD[-/ ]DDDD[-/ ]DDDD
    // We do a simple windowed scan on the original string.
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if let Some(end) = try_parse_cc_at(&chars, i) {
            // Verify the group doesn't continue with more digits (avoid
            // matching longer numeric strings that aren't cards).
            if end >= len || !chars[end].is_ascii_digit() {
                // Also make sure it didn't start as part of a longer number.
                if i == 0 || !chars[i - 1].is_ascii_digit() {
                    return true;
                }
            }
            i = end;
        } else {
            i += 1;
        }
    }

    false
}

/// Try to parse a credit-card-like pattern starting at position `start`.
/// Returns the index past the last consumed character on success.
fn try_parse_cc_at(chars: &[char], start: usize) -> Option<usize> {
    let mut pos = start;
    for group in 0..4 {
        // Expect 4 digits.
        for _ in 0..4 {
            if pos >= chars.len() || !chars[pos].is_ascii_digit() {
                return None;
            }
            pos += 1;
        }
        // After the first 3 groups, allow an optional separator.
        if group < 3 && pos < chars.len() && (chars[pos] == '-' || chars[pos] == ' ') {
            pos += 1;
        }
    }
    Some(pos)
}

/// Check for SSN-like patterns: XXX-XX-XXXX
pub fn has_ssn_pattern(content: &str) -> bool {
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();

    // Pattern length: 3 + 1 + 2 + 1 + 4 = 11
    if len < 11 {
        return false;
    }

    for i in 0..=len - 11 {
        // Must not be preceded by a digit.
        if i > 0 && chars[i - 1].is_ascii_digit() {
            continue;
        }
        // Must not be followed by a digit.
        if i + 11 < len && chars[i + 11].is_ascii_digit() {
            continue;
        }

        if chars[i].is_ascii_digit()
            && chars[i + 1].is_ascii_digit()
            && chars[i + 2].is_ascii_digit()
            && chars[i + 3] == '-'
            && chars[i + 4].is_ascii_digit()
            && chars[i + 5].is_ascii_digit()
            && chars[i + 6] == '-'
            && chars[i + 7].is_ascii_digit()
            && chars[i + 8].is_ascii_digit()
            && chars[i + 9].is_ascii_digit()
            && chars[i + 10].is_ascii_digit()
        {
            return true;
        }
    }

    false
}

/// Simple safety classification for content.
///
/// Returns `"deny"`, `"redact"`, or `"allow"`.
///
/// Classification matches native `SafetyGate::check`:
/// - Credit card patterns -> "redact"
/// - SSN patterns -> "redact"
/// - Email patterns -> "redact"
/// - Custom sensitive keywords -> "deny"
pub fn safety_classify(content: &str) -> &'static str {
    // PII patterns are redacted (matching native SafetyGate behavior)
    if has_credit_card_pattern(content) {
        return "redact";
    }
    if has_ssn_pattern(content) {
        return "redact";
    }
    if has_email_pattern(content) {
        return "redact";
    }

    // Custom sensitive keywords are denied (matching native custom_patterns -> Deny)
    let lower = content.to_lowercase();
    let deny_keywords = [
        "password",
        "secret",
        "api_key",
        "api-key",
        "apikey",
        "token",
        "private_key",
        "private-key",
    ];
    for kw in &deny_keywords {
        if lower.contains(kw) {
            return "deny";
        }
    }

    "allow"
}

/// Check for email-like patterns: local@domain.tld
pub fn has_email_pattern(content: &str) -> bool {
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();

    for i in 0..len {
        if chars[i] == '@' {
            // Must have at least one local-part char before '@'
            if i == 0 || chars[i - 1].is_whitespace() {
                continue;
            }
            // Must have at least one domain char and a dot after '@'
            if i + 1 >= len || chars[i + 1].is_whitespace() {
                continue;
            }
            // Scan backwards to find start of local part
            let mut start = i;
            while start > 0 && is_email_char(chars[start - 1]) {
                start -= 1;
            }
            if start == i {
                continue;
            }
            // Scan forwards to find end of domain
            let mut end = i + 1;
            let mut has_dot = false;
            while end < len && is_domain_char(chars[end]) {
                if chars[end] == '.' {
                    has_dot = true;
                }
                end += 1;
            }
            if has_dot && end > i + 3 {
                return true;
            }
        }
    }
    false
}

fn is_email_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '+' || c == '-' || c == '_'
}

fn is_domain_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-'
}

/// Route a query string to the optimal search backend.
///
/// Returns `"Temporal"`, `"Graph"`, `"Keyword"`, or `"Hybrid"`.
///
/// Routing heuristics (matching native `QueryRouter::route`):
/// - Temporal keywords ("yesterday", "last week", etc.) -> Temporal
/// - Graph keywords ("related to", "connected to", etc.) -> Graph
/// - Quoted exact phrases -> Keyword
/// - Short queries (1-2 words) -> Keyword
/// - Everything else -> Hybrid
pub fn route_query(query: &str) -> &'static str {
    let lower = query.to_lowercase();
    let word_count = lower.split_whitespace().count();

    // Temporal patterns (checked first, matching native router order)
    let temporal_keywords = [
        "yesterday",
        "last week",
        "last month",
        "today",
        "this morning",
        "this afternoon",
        "hours ago",
        "minutes ago",
        "days ago",
        "between",
        "before",
        "after",
    ];
    for kw in &temporal_keywords {
        if lower.contains(kw) {
            return "Temporal";
        }
    }

    // Graph patterns
    let graph_keywords = [
        "related to",
        "connected to",
        "linked with",
        "associated with",
        "relationship between",
    ];
    for kw in &graph_keywords {
        if lower.contains(kw) {
            return "Graph";
        }
    }

    // Exact phrase (quoted)
    if query.starts_with('"') && query.ends_with('"') {
        return "Keyword";
    }

    // Very short queries are better served by keyword
    if word_count <= 2 {
        return "Keyword";
    }

    // Default: hybrid combines the best of both
    "Hybrid"
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 2.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_hash_embed_deterministic() {
        let v1 = hash_embed("hello world", 128);
        let v2 = hash_embed("hello world", 128);
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_hash_embed_normalized() {
        let v = hash_embed("test text", 64);
        let mag: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (mag - 1.0).abs() < 1e-4,
            "magnitude should be ~1.0, got {mag}"
        );
    }

    #[test]
    fn test_hash_embed_different_texts_differ() {
        let v1 = hash_embed("hello", 64);
        let v2 = hash_embed("world", 64);
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_has_credit_card_pattern() {
        assert!(has_credit_card_pattern("my card is 1234 5678 9012 3456"));
        assert!(has_credit_card_pattern("cc: 1234-5678-9012-3456"));
        assert!(has_credit_card_pattern("number 1234567890123456 here"));
        assert!(!has_credit_card_pattern("short 123456"));
        assert!(!has_credit_card_pattern("no cards here"));
    }

    #[test]
    fn test_has_ssn_pattern() {
        assert!(has_ssn_pattern("ssn is 123-45-6789"));
        assert!(has_ssn_pattern("start 999-99-9999 end"));
        assert!(!has_ssn_pattern("not a ssn 12-345-6789"));
        assert!(!has_ssn_pattern("1234-56-7890")); // preceded by extra digit
        assert!(!has_ssn_pattern("no ssn here"));
    }

    #[test]
    fn test_safety_classify_redact_cc() {
        assert_eq!(safety_classify("pay with 4111-1111-1111-1111"), "redact");
    }

    #[test]
    fn test_safety_classify_redact_ssn() {
        assert_eq!(safety_classify("my ssn 123-45-6789"), "redact");
    }

    #[test]
    fn test_safety_classify_redact_email() {
        assert_eq!(safety_classify("contact user@example.com"), "redact");
    }

    #[test]
    fn test_safety_classify_deny_password() {
        assert_eq!(safety_classify("my password is foo"), "deny");
    }

    #[test]
    fn test_safety_classify_deny_api_key() {
        assert_eq!(safety_classify("api_key: sk-abc123"), "deny");
    }

    #[test]
    fn test_safety_classify_allow() {
        assert_eq!(safety_classify("the weather is nice"), "allow");
    }

    #[test]
    fn test_has_email_pattern() {
        assert!(has_email_pattern("contact user@example.com please"));
        assert!(has_email_pattern("email: alice@test.org"));
        assert!(!has_email_pattern("not an email"));
        assert!(!has_email_pattern("@ alone"));
        assert!(!has_email_pattern("no@d"));
    }

    #[test]
    fn test_route_query_temporal() {
        assert_eq!(route_query("what happened yesterday"), "Temporal");
        assert_eq!(route_query("show me events from last week"), "Temporal");
    }

    #[test]
    fn test_route_query_graph() {
        assert_eq!(route_query("documents related to authentication"), "Graph");
        assert_eq!(route_query("things connected to the API module"), "Graph");
    }

    #[test]
    fn test_route_query_keyword_quoted() {
        assert_eq!(route_query("\"exact phrase search\""), "Keyword");
    }

    #[test]
    fn test_route_query_keyword_short() {
        assert_eq!(route_query("rust programming"), "Keyword");
        assert_eq!(route_query("hello"), "Keyword");
    }

    #[test]
    fn test_route_query_hybrid() {
        assert_eq!(route_query("something about machine learning"), "Hybrid");
        assert_eq!(route_query("explain how embeddings work"), "Hybrid");
    }
}
