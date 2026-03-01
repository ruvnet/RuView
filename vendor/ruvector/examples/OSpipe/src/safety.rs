//! Safety gate for content filtering and PII redaction.
//!
//! The safety gate inspects captured content before it enters the
//! ingestion pipeline, detecting and optionally redacting sensitive
//! information such as credit card numbers, SSNs, and custom patterns.

use crate::config::SafetyConfig;

/// Decision made by the safety gate about a piece of content.
#[derive(Debug, Clone, PartialEq)]
pub enum SafetyDecision {
    /// Content is safe to store as-is.
    Allow,
    /// Content is safe after redaction; the redacted version is provided.
    AllowRedacted(String),
    /// Content must not be stored.
    Deny {
        /// Reason for denial.
        reason: String,
    },
}

/// Safety gate that checks content for sensitive information.
pub struct SafetyGate {
    config: SafetyConfig,
}

impl SafetyGate {
    /// Create a new safety gate with the given configuration.
    pub fn new(config: SafetyConfig) -> Self {
        Self { config }
    }

    /// Check content and return a safety decision.
    ///
    /// If PII is detected and redaction is enabled, the content is
    /// returned in redacted form. If custom patterns match and no
    /// redaction is possible, the content is denied.
    pub fn check(&self, content: &str) -> SafetyDecision {
        let mut redacted = content.to_string();
        let mut was_redacted = false;

        // Credit card redaction
        if self.config.credit_card_redaction {
            let (new_text, found) = redact_credit_cards(&redacted);
            if found {
                redacted = new_text;
                was_redacted = true;
            }
        }

        // SSN redaction
        if self.config.ssn_redaction {
            let (new_text, found) = redact_ssns(&redacted);
            if found {
                redacted = new_text;
                was_redacted = true;
            }
        }

        // PII detection (email addresses)
        if self.config.pii_detection {
            let (new_text, found) = redact_emails(&redacted);
            if found {
                redacted = new_text;
                was_redacted = true;
            }
        }

        // Custom patterns: deny if found (custom patterns indicate content
        // that should not be stored at all)
        for pattern in &self.config.custom_patterns {
            if content.contains(pattern.as_str()) {
                return SafetyDecision::Deny {
                    reason: format!("Custom pattern matched: {}", pattern),
                };
            }
        }

        if was_redacted {
            SafetyDecision::AllowRedacted(redacted)
        } else {
            SafetyDecision::Allow
        }
    }

    /// Redact all detected sensitive content and return the cleaned string.
    pub fn redact(&self, content: &str) -> String {
        match self.check(content) {
            SafetyDecision::Allow => content.to_string(),
            SafetyDecision::AllowRedacted(redacted) => redacted,
            SafetyDecision::Deny { .. } => "[REDACTED]".to_string(),
        }
    }
}

/// Detect and redact sequences of 13-16 digits that look like credit card numbers.
///
/// This uses a simple pattern: sequences of digits (with optional spaces or dashes)
/// totaling 13-16 digits are replaced with [CC_REDACTED].
fn redact_credit_cards(text: &str) -> (String, bool) {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut found = false;

    while i < chars.len() {
        // Check if we are at the start of a digit sequence
        if chars[i].is_ascii_digit() {
            let start = i;
            let mut digit_count = 0;

            // Consume digits, spaces, and dashes
            while i < chars.len()
                && (chars[i].is_ascii_digit() || chars[i] == ' ' || chars[i] == '-')
            {
                if chars[i].is_ascii_digit() {
                    digit_count += 1;
                }
                i += 1;
            }

            if (13..=16).contains(&digit_count) {
                result.push_str("[CC_REDACTED]");
                found = true;
            } else {
                // Not a credit card, keep original text
                for c in &chars[start..i] {
                    result.push(*c);
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    (result, found)
}

/// Detect and redact SSN patterns (XXX-XX-XXXX).
fn redact_ssns(text: &str) -> (String, bool) {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut found = false;
    let mut i = 0;

    while i < chars.len() {
        // Check for SSN pattern: 3 digits, dash, 2 digits, dash, 4 digits
        if i + 10 < chars.len() && is_ssn_at(&chars, i) {
            result.push_str("[SSN_REDACTED]");
            found = true;
            i += 11; // Skip the SSN (XXX-XX-XXXX = 11 chars)
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    (result, found)
}

/// Check if an SSN pattern exists at the given position.
fn is_ssn_at(chars: &[char], pos: usize) -> bool {
    if pos + 10 >= chars.len() {
        return false;
    }
    // XXX-XX-XXXX
    chars[pos].is_ascii_digit()
        && chars[pos + 1].is_ascii_digit()
        && chars[pos + 2].is_ascii_digit()
        && chars[pos + 3] == '-'
        && chars[pos + 4].is_ascii_digit()
        && chars[pos + 5].is_ascii_digit()
        && chars[pos + 6] == '-'
        && chars[pos + 7].is_ascii_digit()
        && chars[pos + 8].is_ascii_digit()
        && chars[pos + 9].is_ascii_digit()
        && chars[pos + 10].is_ascii_digit()
}

/// Detect and redact email addresses while preserving surrounding whitespace.
///
/// Scans character-by-character for `@` signs, then expands outward to find
/// the full `local@domain.tld` span and replaces it in-place, keeping all
/// surrounding whitespace (tabs, newlines, multi-space runs) intact.
fn redact_emails(text: &str) -> (String, bool) {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut result = String::with_capacity(text.len());
    let mut found = false;
    let mut i = 0;

    while i < len {
        if chars[i] == '@' {
            // Try to identify an email around this '@'.
            // Scan backwards for the local part.
            let mut local_start = i;
            while local_start > 0 && is_email_local_char(chars[local_start - 1]) {
                local_start -= 1;
            }

            // Scan forwards for the domain part.
            let mut domain_end = i + 1;
            let mut has_dot = false;
            while domain_end < len && is_email_domain_char(chars[domain_end]) {
                if chars[domain_end] == '.' {
                    has_dot = true;
                }
                domain_end += 1;
            }
            // Trim trailing dots/hyphens from domain (not valid at end).
            while domain_end > i + 1
                && (chars[domain_end - 1] == '.' || chars[domain_end - 1] == '-')
            {
                if chars[domain_end - 1] == '.' {
                    // Re-check if we still have a dot in the trimmed domain.
                    has_dot = chars[i + 1..domain_end - 1].contains(&'.');
                }
                domain_end -= 1;
            }

            let local_len = i - local_start;
            let domain_len = domain_end - (i + 1);

            if local_len > 0 && domain_len >= 3 && has_dot {
                // Valid email: replace the span [local_start..domain_end]
                // We need to remove any characters already pushed for the local part.
                // They were pushed in the normal flow below, so truncate them.
                let already_pushed = i - local_start;
                let new_len = result.len() - already_pushed;
                result.truncate(new_len);
                result.push_str("[EMAIL_REDACTED]");
                found = true;
                i = domain_end;
            } else {
                // Not a valid email, keep the '@' as-is.
                result.push(chars[i]);
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    (result, found)
}

/// Characters valid in the local part of an email address.
fn is_email_local_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '+' || c == '-' || c == '_'
}

/// Characters valid in the domain part of an email address.
fn is_email_domain_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '-'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SafetyConfig;

    // ---------------------------------------------------------------
    // Email redaction whitespace preservation tests
    // ---------------------------------------------------------------

    #[test]
    fn test_email_redaction_preserves_tabs() {
        let (result, found) = redact_emails("contact\tuser@example.com\there");
        assert!(found);
        assert_eq!(result, "contact\t[EMAIL_REDACTED]\there");
    }

    #[test]
    fn test_email_redaction_preserves_newlines() {
        let (result, found) = redact_emails("contact\nuser@example.com\nhere");
        assert!(found);
        assert_eq!(result, "contact\n[EMAIL_REDACTED]\nhere");
    }

    #[test]
    fn test_email_redaction_preserves_multi_spaces() {
        let (result, found) = redact_emails("contact   user@example.com   here");
        assert!(found);
        assert_eq!(result, "contact   [EMAIL_REDACTED]   here");
    }

    #[test]
    fn test_email_redaction_preserves_mixed_whitespace() {
        let (result, found) = redact_emails("contact\t  user@example.com\n  here");
        assert!(found);
        assert_eq!(result, "contact\t  [EMAIL_REDACTED]\n  here");
    }

    #[test]
    fn test_email_redaction_basic() {
        let (result, found) = redact_emails("email user@example.com here");
        assert!(found);
        assert_eq!(result, "email [EMAIL_REDACTED] here");
    }

    #[test]
    fn test_email_redaction_no_email() {
        let (result, found) = redact_emails("no email here");
        assert!(!found);
        assert_eq!(result, "no email here");
    }

    #[test]
    fn test_email_redaction_multiple_emails() {
        let (result, found) = redact_emails("a@b.com and c@d.org");
        assert!(found);
        assert_eq!(result, "[EMAIL_REDACTED] and [EMAIL_REDACTED]");
    }

    #[test]
    fn test_email_redaction_at_start() {
        let (result, found) = redact_emails("user@example.com is the contact");
        assert!(found);
        assert_eq!(result, "[EMAIL_REDACTED] is the contact");
    }

    #[test]
    fn test_email_redaction_at_end() {
        let (result, found) = redact_emails("contact: user@example.com");
        assert!(found);
        assert_eq!(result, "contact: [EMAIL_REDACTED]");
    }

    // ---------------------------------------------------------------
    // Safety gate integration tests for consistency
    // ---------------------------------------------------------------

    #[test]
    fn test_safety_gate_email_preserves_whitespace() {
        let config = SafetyConfig::default();
        let gate = SafetyGate::new(config);
        let decision = gate.check("contact\tuser@example.com\nhere");
        match decision {
            SafetyDecision::AllowRedacted(redacted) => {
                assert_eq!(redacted, "contact\t[EMAIL_REDACTED]\nhere");
            }
            other => panic!("Expected AllowRedacted, got {:?}", other),
        }
    }

    // ---------------------------------------------------------------
    // Routing consistency tests (WASM vs native)
    // ---------------------------------------------------------------

    #[test]
    fn test_wasm_routing_matches_native_temporal() {
        use crate::search::router::QueryRoute;
        use crate::search::router::QueryRouter;
        use crate::wasm::helpers::route_query;

        let router = QueryRouter::new();
        let queries = [
            "what did I see yesterday",
            "show me last week",
            "results from today",
        ];
        for q in &queries {
            assert_eq!(
                router.route(q),
                QueryRoute::Temporal,
                "Native router failed for: {}",
                q
            );
            assert_eq!(route_query(q), "Temporal", "WASM router failed for: {}", q);
        }
    }

    #[test]
    fn test_wasm_routing_matches_native_graph() {
        use crate::search::router::QueryRoute;
        use crate::search::router::QueryRouter;
        use crate::wasm::helpers::route_query;

        let router = QueryRouter::new();
        let queries = [
            "documents related to authentication",
            "things connected to the API module",
        ];
        for q in &queries {
            assert_eq!(
                router.route(q),
                QueryRoute::Graph,
                "Native router failed for: {}",
                q
            );
            assert_eq!(route_query(q), "Graph", "WASM router failed for: {}", q);
        }
    }

    #[test]
    fn test_wasm_routing_matches_native_keyword_short() {
        use crate::search::router::QueryRoute;
        use crate::search::router::QueryRouter;
        use crate::wasm::helpers::route_query;

        let router = QueryRouter::new();
        let queries = ["hello", "rust programming"];
        for q in &queries {
            assert_eq!(
                router.route(q),
                QueryRoute::Keyword,
                "Native router failed for: {}",
                q
            );
            assert_eq!(route_query(q), "Keyword", "WASM router failed for: {}", q);
        }
    }

    #[test]
    fn test_wasm_routing_matches_native_keyword_quoted() {
        use crate::search::router::QueryRoute;
        use crate::search::router::QueryRouter;
        use crate::wasm::helpers::route_query;

        let router = QueryRouter::new();
        let q = "\"exact phrase search\"";
        assert_eq!(router.route(q), QueryRoute::Keyword);
        assert_eq!(route_query(q), "Keyword");
    }

    #[test]
    fn test_wasm_routing_matches_native_hybrid() {
        use crate::search::router::QueryRoute;
        use crate::search::router::QueryRouter;
        use crate::wasm::helpers::route_query;

        let router = QueryRouter::new();
        let queries = [
            "how to implement authentication in Rust",
            "explain how embeddings work",
            "something about machine learning",
        ];
        for q in &queries {
            assert_eq!(
                router.route(q),
                QueryRoute::Hybrid,
                "Native router failed for: {}",
                q
            );
            assert_eq!(route_query(q), "Hybrid", "WASM router failed for: {}", q);
        }
    }

    // ---------------------------------------------------------------
    // Safety consistency tests (WASM vs native)
    // ---------------------------------------------------------------

    #[test]
    fn test_wasm_safety_matches_native_cc() {
        use crate::wasm::helpers::safety_classify;

        // Native: CC -> AllowRedacted; WASM should return "redact"
        let config = SafetyConfig::default();
        let gate = SafetyGate::new(config);
        let content = "pay with 4111-1111-1111-1111";
        assert!(matches!(
            gate.check(content),
            SafetyDecision::AllowRedacted(_)
        ));
        assert_eq!(safety_classify(content), "redact");
    }

    #[test]
    fn test_wasm_safety_matches_native_ssn() {
        use crate::wasm::helpers::safety_classify;

        let config = SafetyConfig::default();
        let gate = SafetyGate::new(config);
        let content = "my ssn 123-45-6789";
        assert!(matches!(
            gate.check(content),
            SafetyDecision::AllowRedacted(_)
        ));
        assert_eq!(safety_classify(content), "redact");
    }

    #[test]
    fn test_wasm_safety_matches_native_email() {
        use crate::wasm::helpers::safety_classify;

        let config = SafetyConfig::default();
        let gate = SafetyGate::new(config);
        let content = "email user@example.com here";
        assert!(matches!(
            gate.check(content),
            SafetyDecision::AllowRedacted(_)
        ));
        assert_eq!(safety_classify(content), "redact");
    }

    #[test]
    fn test_wasm_safety_matches_native_custom_deny() {
        use crate::wasm::helpers::safety_classify;

        // Native: custom_patterns -> Deny; WASM: sensitive keywords -> "deny"
        let config = SafetyConfig {
            custom_patterns: vec!["password".to_string()],
            ..Default::default()
        };
        let gate = SafetyGate::new(config);
        let content = "my password is foo";
        assert!(matches!(gate.check(content), SafetyDecision::Deny { .. }));
        assert_eq!(safety_classify(content), "deny");
    }

    #[test]
    fn test_wasm_safety_matches_native_allow() {
        use crate::wasm::helpers::safety_classify;

        let config = SafetyConfig::default();
        let gate = SafetyGate::new(config);
        let content = "the weather is nice";
        assert_eq!(gate.check(content), SafetyDecision::Allow);
        assert_eq!(safety_classify(content), "allow");
    }

    // ---------------------------------------------------------------
    // MMR tests
    // ---------------------------------------------------------------

    #[test]
    fn test_mmr_produces_different_order_than_cosine() {
        use crate::search::mmr::MmrReranker;

        let mmr = MmrReranker::new(0.3);
        let query = vec![1.0, 0.0, 0.0, 0.0];
        let results = vec![
            ("a".to_string(), 0.95, vec![1.0, 0.0, 0.0, 0.0]),
            ("b".to_string(), 0.90, vec![0.99, 0.01, 0.0, 0.0]),
            ("c".to_string(), 0.60, vec![0.0, 1.0, 0.0, 0.0]),
        ];

        let ranked = mmr.rerank(&query, &results, 3);
        assert_eq!(ranked.len(), 3);

        // Pure cosine order: a, b, c
        // MMR with diversity: a, c, b (c is diverse, b is near-duplicate of a)
        assert_eq!(ranked[0].0, "a");
        assert_eq!(ranked[1].0, "c", "MMR should promote diverse result");
        assert_eq!(ranked[2].0, "b");
    }
}
