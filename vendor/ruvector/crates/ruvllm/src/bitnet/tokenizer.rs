//! Minimal BPE Tokenizer for BitNet Inference
//!
//! Provides a byte-level BPE (Byte Pair Encoding) tokenizer that converts text
//! to token IDs and back. The tokenizer operates on UTF-8 byte sequences and
//! iteratively applies merge rules to produce a compact token representation.
//!
//! ## Algorithm
//!
//! 1. Convert input text to UTF-8 bytes
//! 2. Map each byte to a single-byte token string
//! 3. Iteratively apply BPE merge rules (highest-priority first)
//! 4. Map merged tokens to vocabulary IDs
//! 5. Prepend BOS token
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::bitnet::tokenizer::{BpeTokenizer, SpecialTokens};
//!
//! let vocab = (0..=255u8).map(|b| format!("<{:02X}>", b)).collect();
//! let merges = vec![("<48>".to_string(), "<65>".to_string())]; // "H" + "e"
//! let tokenizer = BpeTokenizer::from_vocab(vocab, merges, SpecialTokens::default());
//!
//! let ids = tokenizer.encode("Hello");
//! let text = tokenizer.decode(&ids);
//! ```

use std::collections::HashMap;

use crate::error::{Result, RuvLLMError};

// ============================================================================
// Special Tokens
// ============================================================================

/// Special token IDs used by the tokenizer.
///
/// These follow common conventions for transformer models:
/// - BOS (Beginning of Sequence) is prepended to every encoded sequence
/// - EOS (End of Sequence) signals generation should stop
/// - PAD is used for batch padding
/// - UNK replaces tokens not found in the vocabulary
pub struct SpecialTokens {
    /// Beginning-of-sequence token ID
    pub bos_id: u32,
    /// End-of-sequence token ID
    pub eos_id: u32,
    /// Padding token ID
    pub pad_id: u32,
    /// Unknown token ID
    pub unk_id: u32,
}

impl Default for SpecialTokens {
    fn default() -> Self {
        Self {
            bos_id: 1,
            eos_id: 2,
            pad_id: 0,
            unk_id: 3,
        }
    }
}

// ============================================================================
// BPE Tokenizer
// ============================================================================

/// Byte-level BPE tokenizer.
///
/// Encodes text by first splitting into UTF-8 bytes, then iteratively merging
/// adjacent token pairs according to a learned merge table. The merge table
/// is ordered by priority (index 0 = highest priority merge).
pub struct BpeTokenizer {
    /// Vocabulary: maps token ID to token string
    vocab: Vec<String>,
    /// Reverse mapping: token string to token ID
    token_to_id: HashMap<String, u32>,
    /// Ordered merge rules (pair of token strings to merge)
    merges: Vec<(String, String)>,
    /// Special token configuration
    special_tokens: SpecialTokens,
}

impl BpeTokenizer {
    /// Create a new BPE tokenizer from vocabulary and merge rules.
    ///
    /// The `tokens` vector defines the vocabulary (index = token ID).
    /// The `merges` vector defines BPE merge rules in priority order
    /// (index 0 = highest priority, applied first).
    ///
    /// # Arguments
    ///
    /// * `tokens` - Vocabulary tokens indexed by ID
    /// * `merges` - Ordered merge rules as (left, right) token string pairs
    /// * `special` - Special token ID configuration
    pub fn from_vocab(
        tokens: Vec<String>,
        merges: Vec<(String, String)>,
        special: SpecialTokens,
    ) -> Self {
        let mut token_to_id = HashMap::with_capacity(tokens.len());
        for (id, tok) in tokens.iter().enumerate() {
            token_to_id.insert(tok.clone(), id as u32);
        }
        Self {
            vocab: tokens,
            token_to_id,
            merges,
            special_tokens: special,
        }
    }

    /// Encode text into a sequence of token IDs.
    ///
    /// The encoding process:
    /// 1. Convert text to UTF-8 bytes
    /// 2. Map each byte to its single-byte token string
    /// 3. Iteratively apply BPE merges (highest priority first)
    /// 4. Map merged token strings to vocabulary IDs
    /// 5. Prepend BOS token ID
    ///
    /// Unknown tokens (not in vocabulary) are mapped to `unk_id`.
    ///
    /// # Arguments
    ///
    /// * `text` - Input text to encode
    ///
    /// # Returns
    ///
    /// Vector of token IDs with BOS prepended
    pub fn encode(&self, text: &str) -> Vec<u32> {
        if text.is_empty() {
            return vec![self.special_tokens.bos_id];
        }

        // Step 1: Convert to UTF-8 bytes and map to single-byte token strings
        let bytes = text.as_bytes();
        let mut symbols: Vec<String> = bytes.iter().map(|&b| self.byte_to_token(b)).collect();

        // Step 2: Iteratively apply BPE merges
        // For each merge rule (in priority order), scan the sequence and merge
        // all adjacent occurrences of the pair.
        for (left, right) in &self.merges {
            let merged = format!("{}{}", left, right);
            // Only process if the merged token exists in our vocabulary
            if !self.token_to_id.contains_key(&merged) {
                continue;
            }
            let mut i = 0;
            while i + 1 < symbols.len() {
                if symbols[i] == *left && symbols[i + 1] == *right {
                    symbols[i] = merged.clone();
                    symbols.remove(i + 1);
                    // Don't increment i; the new merged token might merge with
                    // the next token via a later (lower priority) rule, but
                    // we handle that in the next pass of the outer loop.
                } else {
                    i += 1;
                }
            }
        }

        // Step 3: Map token strings to IDs, prepend BOS
        let mut ids = Vec::with_capacity(symbols.len() + 1);
        ids.push(self.special_tokens.bos_id);
        for sym in &symbols {
            let id = self
                .token_to_id
                .get(sym)
                .copied()
                .unwrap_or(self.special_tokens.unk_id);
            ids.push(id);
        }

        ids
    }

    /// Decode a sequence of token IDs back to a string.
    ///
    /// Maps each ID to its vocabulary string and concatenates. Special tokens
    /// (BOS, EOS, PAD) are skipped. The concatenated bytes are interpreted
    /// as UTF-8; invalid sequences are replaced with the Unicode replacement
    /// character.
    ///
    /// # Arguments
    ///
    /// * `ids` - Token IDs to decode
    ///
    /// # Returns
    ///
    /// Decoded string
    pub fn decode(&self, ids: &[u32]) -> String {
        let mut bytes = Vec::new();

        for &id in ids {
            // Skip special tokens
            if id == self.special_tokens.bos_id
                || id == self.special_tokens.eos_id
                || id == self.special_tokens.pad_id
            {
                continue;
            }

            if let Some(token_str) = self.vocab.get(id as usize) {
                // Convert token string back to bytes
                let token_bytes = self.token_to_bytes(token_str);
                bytes.extend_from_slice(&token_bytes);
            }
        }

        String::from_utf8(bytes)
            .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
    }

    /// Get the vocabulary size.
    pub fn vocab_size(&self) -> usize {
        self.vocab.len()
    }

    /// Convert a single byte to its token string representation.
    ///
    /// Uses a hex-encoded format: `<XX>` where XX is the uppercase hex
    /// value of the byte. If this token exists in the vocabulary, use it;
    /// otherwise fall back to a raw byte string.
    fn byte_to_token(&self, byte: u8) -> String {
        // Try hex format first (common in BPE vocabularies)
        let hex_token = format!("<{:02X}>", byte);
        if self.token_to_id.contains_key(&hex_token) {
            return hex_token;
        }

        // Try the raw single-character representation
        let char_token = String::from(byte as char);
        if self.token_to_id.contains_key(&char_token) {
            return char_token;
        }

        // Fall back to hex format even if not in vocab (will map to UNK)
        hex_token
    }

    /// Convert a token string back to its byte representation.
    ///
    /// Handles both hex-encoded (`<XX>`) and raw character tokens,
    /// as well as merged multi-byte tokens.
    fn token_to_bytes(&self, token: &str) -> Vec<u8> {
        let mut result = Vec::new();
        let mut chars = token.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '<' {
                // Try to parse hex byte: <XX>
                let mut hex = String::new();
                let mut found_close = false;
                for c in chars.by_ref() {
                    if c == '>' {
                        found_close = true;
                        break;
                    }
                    hex.push(c);
                }
                if found_close && hex.len() == 2 {
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte);
                        continue;
                    }
                }
                // Not a valid hex escape; emit the raw characters
                result.push(b'<');
                result.extend_from_slice(hex.as_bytes());
                if found_close {
                    result.push(b'>');
                }
            } else {
                // Raw character: emit its UTF-8 bytes
                let mut buf = [0u8; 4];
                let encoded = ch.encode_utf8(&mut buf);
                result.extend_from_slice(encoded.as_bytes());
            }
        }

        result
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a test tokenizer with hex-encoded byte tokens and optional merges.
    fn test_tokenizer(merges: Vec<(String, String)>, extra_tokens: Vec<String>) -> BpeTokenizer {
        // Base vocabulary: special tokens + 256 byte tokens
        let mut vocab = vec![
            "<PAD>".to_string(), // 0 = PAD
            "<BOS>".to_string(), // 1 = BOS
            "<EOS>".to_string(), // 2 = EOS
            "<UNK>".to_string(), // 3 = UNK
        ];
        for b in 0..=255u8 {
            vocab.push(format!("<{:02X}>", b));
        }
        // Add merged tokens
        for tok in extra_tokens {
            vocab.push(tok);
        }

        BpeTokenizer::from_vocab(vocab, merges, SpecialTokens::default())
    }

    #[test]
    fn test_roundtrip_ascii() {
        let tok = test_tokenizer(vec![], vec![]);
        let text = "Hello, world!";
        let ids = tok.encode(text);
        let decoded = tok.decode(&ids);
        assert_eq!(decoded, text, "ASCII roundtrip failed");
    }

    #[test]
    fn test_roundtrip_utf8() {
        let tok = test_tokenizer(vec![], vec![]);
        let text = "cafe\u{0301}"; // cafe with combining accent
        let ids = tok.encode(text);
        let decoded = tok.decode(&ids);
        assert_eq!(decoded, text, "UTF-8 roundtrip failed");
    }

    #[test]
    fn test_bos_prepended() {
        let tok = test_tokenizer(vec![], vec![]);
        let ids = tok.encode("A");
        assert_eq!(ids[0], 1, "First token should be BOS (id=1)");
        assert!(ids.len() >= 2, "Should have at least BOS + one token");
    }

    #[test]
    fn test_eos_handling() {
        let tok = test_tokenizer(vec![], vec![]);
        // Decoding a sequence with EOS should skip the EOS token
        let ids = vec![1, 4 + b'H' as u32, 4 + b'i' as u32, 2]; // BOS, H, i, EOS
        let decoded = tok.decode(&ids);
        assert_eq!(decoded, "Hi", "EOS should be skipped in decode");
    }

    #[test]
    fn test_unknown_token() {
        // Token ID beyond vocab should not appear in normal encode,
        // but decode should handle gracefully
        let tok = test_tokenizer(vec![], vec![]);
        let ids = vec![99999]; // Way beyond vocab
        let decoded = tok.decode(&ids);
        assert_eq!(decoded, "", "Unknown ID should produce empty output");
    }

    #[test]
    fn test_empty_string() {
        let tok = test_tokenizer(vec![], vec![]);
        let ids = tok.encode("");
        assert_eq!(ids, vec![1], "Empty string should encode to just BOS");
        let decoded = tok.decode(&ids);
        assert_eq!(decoded, "", "Decoding just BOS should give empty string");
    }

    #[test]
    fn test_single_char() {
        let tok = test_tokenizer(vec![], vec![]);
        let ids = tok.encode("A");
        assert_eq!(ids.len(), 2, "Single char should give BOS + 1 token");
        assert_eq!(ids[0], 1, "First should be BOS");
        let decoded = tok.decode(&ids);
        assert_eq!(decoded, "A");
    }

    #[test]
    fn test_bpe_merge_application() {
        // Create a merge rule: <48> + <65> -> <48><65> (i.e., "H" + "e")
        let merged_token = "<48><65>".to_string();
        let merges = vec![("<48>".to_string(), "<65>".to_string())];
        let tok = test_tokenizer(merges, vec![merged_token.clone()]);

        let ids = tok.encode("He");
        // BOS + merged token. The merged token should be one ID.
        // Without merge: BOS, <48>, <65> = 3 tokens
        // With merge: BOS, <48><65> = 2 tokens
        assert_eq!(
            ids.len(),
            2,
            "Merge should reduce 'He' to BOS + 1 merged token"
        );
    }

    #[test]
    fn test_bpe_merge_multiple_occurrences() {
        // Merge rule applied to multiple occurrences in one string
        let merged_token = "<61><62>".to_string(); // "a" + "b"
        let merges = vec![("<61>".to_string(), "<62>".to_string())];
        let tok = test_tokenizer(merges, vec![merged_token]);

        let ids = tok.encode("ababab");
        // "ababab" = 6 bytes. Without merge: BOS + 6 tokens = 7.
        // With merge "ab": BOS + 3 merged tokens = 4.
        assert_eq!(ids.len(), 4, "Should merge all 'ab' pairs");
    }

    #[test]
    fn test_vocab_size() {
        let tok = test_tokenizer(vec![], vec![]);
        assert_eq!(
            tok.vocab_size(),
            4 + 256,
            "Should have 4 special + 256 byte tokens"
        );
    }

    #[test]
    fn test_decode_skips_pad() {
        let tok = test_tokenizer(vec![], vec![]);
        let ids = vec![0, 1, 4 + b'X' as u32, 0, 0]; // PAD, BOS, X, PAD, PAD
        let decoded = tok.decode(&ids);
        assert_eq!(decoded, "X", "PAD and BOS should be skipped");
    }
}
