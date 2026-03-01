//! Three-stage PII stripping pipeline.
//!
//! **Stage 1 — Detection**: Scan string fields for PII patterns.
//! **Stage 2 — Redaction**: Replace PII with deterministic pseudonyms.
//! **Stage 3 — Attestation**: Generate a `RedactionLog` segment.

use std::collections::HashMap;
use regex::Regex;
use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};

use crate::types::{RedactionLog, RedactionEntry};

/// PII category with its detection regex and replacement template.
struct PiiRule {
    category: &'static str,
    rule_id: &'static str,
    pattern: Regex,
    prefix: &'static str,
}

/// Three-stage PII stripping pipeline.
pub struct PiiStripper {
    rules: Vec<PiiRule>,
    /// Custom regex rules added by the user.
    custom_rules: Vec<PiiRule>,
    /// Pseudonym counter per category (for deterministic replacement).
    counters: HashMap<String, u32>,
    /// Map from original value to pseudonym (preserves structural relationships).
    pseudonym_map: HashMap<String, String>,
}

impl PiiStripper {
    /// Create a new stripper with default detection rules.
    pub fn new() -> Self {
        let rules = vec![
            PiiRule {
                category: "path",
                rule_id: "rule_path_unix",
                pattern: Regex::new(r#"(?:/(?:home|Users|var|tmp|opt|etc)/[^\s,;:"'\]}>)]+)"#).unwrap(),
                prefix: "PATH",
            },
            PiiRule {
                category: "path",
                rule_id: "rule_path_windows",
                pattern: Regex::new(r#"(?i:[A-Z]:\\(?:Users|Documents|Program Files)[^\s,;:"'\]}>)]+)"#).unwrap(),
                prefix: "PATH",
            },
            PiiRule {
                category: "ip",
                rule_id: "rule_ipv4",
                pattern: Regex::new(r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\b").unwrap(),
                prefix: "IP",
            },
            PiiRule {
                category: "ip",
                rule_id: "rule_ipv6",
                pattern: Regex::new(r"\b(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b").unwrap(),
                prefix: "IP",
            },
            PiiRule {
                category: "email",
                rule_id: "rule_email",
                pattern: Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b").unwrap(),
                prefix: "EMAIL",
            },
            PiiRule {
                category: "api_key",
                rule_id: "rule_api_key_sk",
                pattern: Regex::new(r"\bsk-[A-Za-z0-9]{20,}\b").unwrap(),
                prefix: "REDACTED_KEY",
            },
            PiiRule {
                category: "api_key",
                rule_id: "rule_api_key_aws",
                pattern: Regex::new(r"\bAKIA[A-Z0-9]{16}\b").unwrap(),
                prefix: "REDACTED_KEY",
            },
            PiiRule {
                category: "api_key",
                rule_id: "rule_api_key_github",
                pattern: Regex::new(r"\bghp_[A-Za-z0-9]{36}\b").unwrap(),
                prefix: "REDACTED_KEY",
            },
            PiiRule {
                category: "api_key",
                rule_id: "rule_bearer_token",
                pattern: Regex::new(r"\bBearer\s+[A-Za-z0-9._~+/=-]{20,}\b").unwrap(),
                prefix: "REDACTED_KEY",
            },
            PiiRule {
                category: "env_var",
                rule_id: "rule_env_unix",
                pattern: Regex::new(r"\$(?:HOME|USER|USERNAME|USERPROFILE|PATH|TMPDIR)\b").unwrap(),
                prefix: "ENV",
            },
            PiiRule {
                category: "env_var",
                rule_id: "rule_env_windows",
                pattern: Regex::new(r"%(?:HOME|USER|USERNAME|USERPROFILE|PATH|TEMP)%").unwrap(),
                prefix: "ENV",
            },
            PiiRule {
                category: "username",
                rule_id: "rule_username_at",
                pattern: Regex::new(r"@[A-Za-z][A-Za-z0-9_-]{2,30}\b").unwrap(),
                prefix: "USER",
            },
        ];

        Self {
            rules,
            custom_rules: Vec::new(),
            counters: HashMap::new(),
            pseudonym_map: HashMap::new(),
        }
    }

    /// Add a custom detection rule.
    pub fn add_rule(&mut self, category: &'static str, rule_id: &'static str, pattern: &str, prefix: &'static str) -> Result<(), regex::Error> {
        self.custom_rules.push(PiiRule {
            category,
            rule_id,
            pattern: Regex::new(pattern)?,
            prefix,
        });
        Ok(())
    }

    /// Reset the pseudonym map and counters (call between exports).
    pub fn reset(&mut self) {
        self.counters.clear();
        self.pseudonym_map.clear();
    }

    /// Get or create a deterministic pseudonym for a matched value.
    fn pseudonym(&mut self, original: &str, prefix: &str) -> String {
        if let Some(existing) = self.pseudonym_map.get(original) {
            return existing.clone();
        }
        let counter = self.counters.entry(prefix.to_string()).or_insert(0);
        *counter += 1;
        let pseudo = format!("<{}_{}>", prefix, counter);
        self.pseudonym_map.insert(original.to_string(), pseudo.clone());
        pseudo
    }

    /// Stage 1+2: Detect and redact PII in a single string.
    /// Returns (redacted_string, list of (category, rule_id, count) tuples).
    fn strip_string(&mut self, input: &str) -> (String, Vec<(String, String, u32)>) {
        let mut result = input.to_string();
        let mut detections: Vec<(String, String, u32)> = Vec::new();

        let num_builtin = self.rules.len();
        let num_custom = self.custom_rules.len();

        for i in 0..(num_builtin + num_custom) {
            let (pattern, prefix, category, rule_id) = if i < num_builtin {
                let r = &self.rules[i];
                (&r.pattern as &Regex, r.prefix, r.category, r.rule_id)
            } else {
                let r = &self.custom_rules[i - num_builtin];
                (&r.pattern as &Regex, r.prefix, r.category, r.rule_id)
            };
            let matches: Vec<String> = pattern.find_iter(&result).map(|m| m.as_str().to_string()).collect();
            if matches.is_empty() {
                continue;
            }
            let count = matches.len() as u32;
            // Build pseudonyms and perform replacements
            let mut replacements: Vec<(String, String)> = Vec::new();
            for m in &matches {
                let pseudo = self.pseudonym(m, prefix);
                replacements.push((m.clone(), pseudo));
            }
            for (original, pseudo) in &replacements {
                result = result.replace(original.as_str(), pseudo.as_str());
            }
            detections.push((category.to_string(), rule_id.to_string(), count));
        }

        (result, detections)
    }

    /// Strip PII from a collection of named string fields.
    ///
    /// Returns the redacted fields and a `RedactionLog` attestation.
    pub fn strip_fields(&mut self, fields: &[(&str, &str)]) -> (Vec<(String, String)>, RedactionLog) {
        // Stage 1+2: Detect and redact
        let mut redacted_fields = Vec::new();
        let mut all_detections: HashMap<(String, String), u32> = HashMap::new();

        // Compute pre-redaction hash (Stage 3 prep)
        let mut hasher = Shake256::default();
        for (name, value) in fields {
            hasher.update(name.as_bytes());
            hasher.update(value.as_bytes());
        }
        let mut pre_hash = [0u8; 32];
        hasher.finalize_xof().read(&mut pre_hash);

        for (name, value) in fields {
            let (redacted, detections) = self.strip_string(value);
            redacted_fields.push((name.to_string(), redacted));
            for (cat, rule, count) in detections {
                *all_detections.entry((cat, rule)).or_insert(0) += count;
            }
        }

        // Stage 3: Build attestation
        let mut log = RedactionLog {
            entries: Vec::new(),
            pre_redaction_hash: pre_hash,
            fields_scanned: fields.len() as u64,
            total_redactions: 0,
            timestamp_s: 0, // caller should set this
        };

        for ((category, rule_id), count) in &all_detections {
            log.entries.push(RedactionEntry {
                category: category.clone(),
                count: *count,
                rule_id: rule_id.clone(),
            });
            log.total_redactions += *count as u64;
        }

        (redacted_fields, log)
    }

    /// Strip PII from a single string value.
    pub fn strip_value(&mut self, input: &str) -> String {
        let (result, _) = self.strip_string(input);
        result
    }

    /// Check if a string contains any detectable PII.
    pub fn contains_pii(&self, input: &str) -> bool {
        let all_rules: Vec<&PiiRule> = self.rules.iter().chain(self.custom_rules.iter()).collect();
        for rule in all_rules {
            if rule.pattern.is_match(input) {
                return true;
            }
        }
        false
    }

    /// Return the current pseudonym map (for debugging/auditing).
    pub fn pseudonym_map(&self) -> &HashMap<String, String> {
        &self.pseudonym_map
    }
}

impl Default for PiiStripper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_unix_paths() {
        let stripper = PiiStripper::new();
        assert!(stripper.contains_pii("/home/user/project/src/main.rs"));
        assert!(stripper.contains_pii("/Users/alice/.ssh/id_rsa"));
    }

    #[test]
    fn detect_ipv4() {
        let stripper = PiiStripper::new();
        assert!(stripper.contains_pii("connecting to 192.168.1.100:8080"));
        assert!(stripper.contains_pii("server at 10.0.0.1"));
    }

    #[test]
    fn detect_emails() {
        let stripper = PiiStripper::new();
        assert!(stripper.contains_pii("contact user@example.com for help"));
    }

    #[test]
    fn detect_api_keys() {
        let stripper = PiiStripper::new();
        assert!(stripper.contains_pii("key: sk-abcdefghijklmnopqrstuv"));
        assert!(stripper.contains_pii("aws: AKIAIOSFODNN7EXAMPLE"));
        assert!(stripper.contains_pii("token: ghp_abcdefghijklmnopqrstuvwxyz0123456789"));
    }

    #[test]
    fn detect_env_vars() {
        let stripper = PiiStripper::new();
        assert!(stripper.contains_pii("path is $HOME/.config"));
        assert!(stripper.contains_pii("dir is %USERPROFILE%\\Desktop"));
    }

    #[test]
    fn redact_preserves_structure() {
        let mut stripper = PiiStripper::new();
        let input1 = "file at /home/alice/project/a.rs";
        let input2 = "also at /home/alice/project/b.rs";
        let r1 = stripper.strip_value(input1);
        let r2 = stripper.strip_value(input2);
        // Same path prefix should get same pseudonym
        assert!(r1.contains("<PATH_"));
        assert!(r2.contains("<PATH_"));
        assert!(!r1.contains("/home/alice"));
        assert!(!r2.contains("/home/alice"));
    }

    #[test]
    fn strip_fields_produces_redaction_log() {
        let mut stripper = PiiStripper::new();
        let fields = vec![
            ("path_field", "/home/user/data.csv"),
            ("ip_field", "connecting to 10.0.0.1"),
            ("clean_field", "no pii here"),
        ];
        let (redacted, log) = stripper.strip_fields(&fields);
        assert_eq!(redacted.len(), 3);
        assert_eq!(log.fields_scanned, 3);
        assert!(log.total_redactions >= 2);
        assert!(log.pre_redaction_hash != [0u8; 32]);
        // clean field should be unchanged
        assert_eq!(redacted[2].1, "no pii here");
    }

    #[test]
    fn no_pii_returns_clean() {
        let stripper = PiiStripper::new();
        assert!(!stripper.contains_pii("just a normal string"));
        assert!(!stripper.contains_pii("alpha = 10.5, beta = 3.2"));
    }

    #[test]
    fn reset_clears_state() {
        let mut stripper = PiiStripper::new();
        stripper.strip_value("/home/user/test");
        assert!(!stripper.pseudonym_map().is_empty());
        stripper.reset();
        assert!(stripper.pseudonym_map().is_empty());
    }

    #[test]
    fn custom_rule() {
        let mut stripper = PiiStripper::new();
        stripper.add_rule("ssn", "rule_ssn", r"\b\d{3}-\d{2}-\d{4}\b", "SSN").unwrap();
        assert!(stripper.contains_pii("ssn: 123-45-6789"));
        let redacted = stripper.strip_value("ssn: 123-45-6789");
        assert!(redacted.contains("<SSN_"));
        assert!(!redacted.contains("123-45-6789"));
    }
}
