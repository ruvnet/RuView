//! Heuristic named-entity recognition (NER) for extracting entities from text.
//!
//! This module performs lightweight, regex-free entity extraction suitable for
//! processing screen captures and transcriptions. It recognises:
//!
//! - **URLs** (`https://...` / `http://...`)
//! - **Email addresses** (`user@domain.tld`)
//! - **Mentions** (`@handle`)
//! - **Capitalized phrases** (two or more consecutive capitalized words -> proper nouns)

/// Extract `(label, name)` pairs from free-form `text`.
///
/// Labels returned:
/// - `"Url"` for HTTP(S) URLs
/// - `"Email"` for email-like patterns
/// - `"Mention"` for `@handle` patterns
/// - `"Person"` for capitalized multi-word phrases (heuristic proper noun)
pub fn extract_entities(text: &str) -> Vec<(String, String)> {
    let mut entities: Vec<(String, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // --- URL detection ---
    for word in text.split_whitespace() {
        let trimmed =
            word.trim_matches(|c: char| c == ',' || c == '.' || c == ')' || c == '(' || c == ';');
        if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
            && trimmed.len() > 10
            && seen.insert(("Url", trimmed.to_string()))
        {
            entities.push(("Url".to_string(), trimmed.to_string()));
        }
    }

    // --- Email detection ---
    for word in text.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| {
            c == ',' || c == '.' || c == ')' || c == '(' || c == ';' || c == '<' || c == '>'
        });
        if is_email_like(trimmed) && seen.insert(("Email", trimmed.to_string())) {
            entities.push(("Email".to_string(), trimmed.to_string()));
        }
    }

    // --- @mention detection ---
    for word in text.split_whitespace() {
        let trimmed =
            word.trim_matches(|c: char| c == ',' || c == '.' || c == ')' || c == '(' || c == ';');
        if trimmed.starts_with('@') && trimmed.len() > 1 {
            let handle = trimmed.to_string();
            if seen.insert(("Mention", handle.clone())) {
                entities.push(("Mention".to_string(), handle));
            }
        }
    }

    // --- Capitalized phrase detection (proper nouns) ---
    let cap_phrases = extract_capitalized_phrases(text);
    for phrase in cap_phrases {
        if seen.insert(("Person", phrase.clone())) {
            entities.push(("Person".to_string(), phrase));
        }
    }

    entities
}

/// Returns `true` if `s` looks like an email address (`local@domain.tld`).
fn is_email_like(s: &str) -> bool {
    // Must contain exactly one '@', with non-empty parts on both sides,
    // and the domain part must contain at least one '.'.
    if let Some(at_pos) = s.find('@') {
        let local = &s[..at_pos];
        let domain = &s[at_pos + 1..];
        !local.is_empty()
            && !domain.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
            && local
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '_' || c == '-' || c == '+')
            && domain
                .chars()
                .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
    } else {
        false
    }
}

/// Extract sequences of two or more consecutive capitalized words as likely
/// proper nouns. Filters out common sentence-starting words when they appear
/// alone at what looks like a sentence boundary.
fn extract_capitalized_phrases(text: &str) -> Vec<String> {
    let mut phrases = Vec::new();
    let words: Vec<&str> = text.split_whitespace().collect();

    let mut i = 0;
    while i < words.len() {
        // Skip words that start a sentence (preceded by nothing or a sentence-ending punctuation).
        let word = words[i].trim_matches(|c: char| !c.is_alphanumeric());
        if is_capitalized(word) && word.len() > 1 {
            // Accumulate consecutive capitalized words.
            let start = i;
            let mut parts = vec![word.to_string()];
            i += 1;
            while i < words.len() {
                let next = words[i].trim_matches(|c: char| !c.is_alphanumeric());
                if is_capitalized(next) && next.len() > 1 {
                    parts.push(next.to_string());
                    i += 1;
                } else {
                    break;
                }
            }
            // Only take phrases of 2+ words (single capitalized words are too noisy).
            if parts.len() >= 2 {
                // Skip if the first word is at position 0 or follows a sentence terminator
                // and is a common article/pronoun. We still keep it if part of a longer
                // multi-word phrase that itself is capitalized.
                let is_sentence_start = start == 0
                    || words.get(start.wrapping_sub(1)).is_some_and(|prev| {
                        prev.ends_with('.') || prev.ends_with('!') || prev.ends_with('?')
                    });

                if is_sentence_start && parts.len() == 2 && is_common_starter(&parts[0]) {
                    // Skip - likely just a sentence starting with "The Xyz" etc.
                } else {
                    let phrase = parts.join(" ");
                    phrases.push(phrase);
                }
            }
        } else {
            i += 1;
        }
    }

    phrases
}

/// Returns `true` if the first character of `word` is uppercase ASCII.
fn is_capitalized(word: &str) -> bool {
    word.chars().next().is_some_and(|c| c.is_uppercase())
}

/// Common sentence-starting words that are not proper nouns.
fn is_common_starter(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "the"
            | "a"
            | "an"
            | "this"
            | "that"
            | "these"
            | "those"
            | "it"
            | "i"
            | "we"
            | "they"
            | "he"
            | "she"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_urls() {
        let entities =
            extract_entities("Visit https://example.com/page and http://foo.bar/baz for info.");
        let urls: Vec<_> = entities.iter().filter(|(l, _)| l == "Url").collect();
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].1, "https://example.com/page");
        assert_eq!(urls[1].1, "http://foo.bar/baz");
    }

    #[test]
    fn test_extract_emails() {
        let entities = extract_entities("Email alice@example.com or bob@company.org for help.");
        let emails: Vec<_> = entities.iter().filter(|(l, _)| l == "Email").collect();
        assert_eq!(emails.len(), 2);
    }

    #[test]
    fn test_extract_mentions() {
        let entities = extract_entities("Hey @alice and @bob-dev, check this out.");
        let mentions: Vec<_> = entities.iter().filter(|(l, _)| l == "Mention").collect();
        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0].1, "@alice");
        assert_eq!(mentions[1].1, "@bob-dev");
    }

    #[test]
    fn test_extract_capitalized_phrases() {
        let entities = extract_entities("I met John Smith at the World Trade Center yesterday.");
        let persons: Vec<_> = entities.iter().filter(|(l, _)| l == "Person").collect();
        assert!(persons.iter().any(|(_, n)| n == "John Smith"));
        assert!(persons.iter().any(|(_, n)| n == "World Trade Center"));
    }

    #[test]
    fn test_no_false_positives_on_sentence_start() {
        let entities = extract_entities("The cat sat on the mat.");
        let persons: Vec<_> = entities.iter().filter(|(l, _)| l == "Person").collect();
        // "The cat" should not appear as a person (single cap word + lowercase).
        assert!(persons.is_empty());
    }

    #[test]
    fn test_deduplication() {
        let entities = extract_entities("Visit https://example.com and https://example.com again.");
        let urls: Vec<_> = entities.iter().filter(|(l, _)| l == "Url").collect();
        assert_eq!(urls.len(), 1);
    }
}
