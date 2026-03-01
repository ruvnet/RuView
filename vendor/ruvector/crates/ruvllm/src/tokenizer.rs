//! Tokenizer Integration for RuvLLM
//!
//! Provides HuggingFace tokenizer integration with support for:
//! - BPE, SentencePiece, Unigram tokenization
//! - Chat templates (Llama3, Mistral, Qwen, ChatML, Phi formats)
//! - Special token handling (BOS, EOS, PAD, etc.)
//! - Streaming decode with UTF-8 handling
//!
//! # Example
//!
//! ```rust,ignore
//! use ruvllm::tokenizer::{RuvTokenizer, ChatMessage, Role};
//!
//! let tokenizer = RuvTokenizer::from_pretrained("Qwen/Qwen2.5-0.5B-Instruct")?;
//!
//! // Basic encode/decode
//! let tokens = tokenizer.encode("Hello, world!")?;
//! let text = tokenizer.decode(&tokens)?;
//!
//! // Chat template
//! let messages = vec![
//!     ChatMessage::system("You are a helpful assistant."),
//!     ChatMessage::user("What is Rust?"),
//! ];
//! let prompt = tokenizer.apply_chat_template(&messages)?;
//! ```

use crate::error::{Result, RuvLLMError};
use std::path::Path;

#[cfg(feature = "candle")]
use hf_hub::{api::sync::Api, Repo, RepoType};
#[cfg(feature = "candle")]
use tokenizers::Tokenizer as HfTokenizer;

/// Chat message for instruction-tuned models
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// Role of the message sender
    pub role: Role,
    /// Content of the message
    pub content: String,
}

impl ChatMessage {
    /// Create a new chat message
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, content)
    }
}

/// Message role in a chat conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// System message (instructions)
    System,
    /// User message (human input)
    User,
    /// Assistant message (model output)
    Assistant,
}

impl Role {
    /// Get role name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
        }
    }
}

/// Chat template formats for different model families
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatTemplate {
    /// Llama 3 format: `<|begin_of_text|><|start_header_id|>role<|end_header_id|>\n\ncontent<|eot_id|>`
    Llama3,
    /// Llama 2 format: `[INST] <<SYS>>\nsystem\n<</SYS>>\n\nuser [/INST] assistant`
    Llama2,
    /// Mistral format: `[INST] system\n\nuser [/INST] assistant`
    Mistral,
    /// Qwen format: `<|im_start|>role\ncontent<|im_end|>\n`
    Qwen,
    /// ChatML format: `<|im_start|>role\ncontent<|im_end|>\n`
    ChatML,
    /// Phi format: `<|user|>\ncontent<|end|>\n<|assistant|>\n`
    Phi,
    /// Gemma format: `<start_of_turn>role\ncontent<end_of_turn>\n`
    Gemma,
    /// Custom template string with placeholders: `{system}`, `{user}`, `{assistant}`
    Custom(String),
}

impl Default for ChatTemplate {
    fn default() -> Self {
        Self::ChatML
    }
}

impl ChatTemplate {
    /// Detect chat template from model ID
    ///
    /// Supports automatic detection for:
    /// - Llama 2/3 variants
    /// - Mistral/Mixtral
    /// - Qwen (ChatML format)
    /// - Phi/Phi-3 (both use same template format)
    /// - Gemma/Gemma-2 (both use same template format)
    pub fn detect_from_model_id(model_id: &str) -> Self {
        let model_lower = model_id.to_lowercase();

        if model_lower.contains("llama-3") || model_lower.contains("llama3") {
            ChatTemplate::Llama3
        } else if model_lower.contains("llama-2") || model_lower.contains("llama2") {
            ChatTemplate::Llama2
        } else if model_lower.contains("mistral")
            || model_lower.contains("mixtral")
            || model_lower.contains("codestral")
        {
            ChatTemplate::Mistral
        } else if model_lower.contains("qwen") {
            ChatTemplate::Qwen
        } else if model_lower.contains("phi-3")
            || model_lower.contains("phi3")
            || model_lower.contains("phi")
        {
            // Phi-3 and Phi use the same template format
            ChatTemplate::Phi
        } else if model_lower.contains("gemma-2")
            || model_lower.contains("gemma2")
            || model_lower.contains("gemma")
        {
            // Gemma-2 and Gemma use the same template format
            ChatTemplate::Gemma
        } else {
            // Default to ChatML as it's widely supported
            ChatTemplate::ChatML
        }
    }

    /// Format messages using this template
    pub fn format(&self, messages: &[ChatMessage]) -> String {
        match self {
            ChatTemplate::Llama3 => Self::format_llama3(messages),
            ChatTemplate::Llama2 => Self::format_llama2(messages),
            ChatTemplate::Mistral => Self::format_mistral(messages),
            ChatTemplate::Qwen | ChatTemplate::ChatML => Self::format_chatml(messages),
            ChatTemplate::Phi => Self::format_phi(messages),
            ChatTemplate::Gemma => Self::format_gemma(messages),
            ChatTemplate::Custom(template) => Self::format_custom(template, messages),
        }
    }

    /// Format messages in Llama 3 style
    fn format_llama3(messages: &[ChatMessage]) -> String {
        let mut result = String::from("<|begin_of_text|>");

        for msg in messages {
            let role = msg.role.as_str();
            result.push_str(&format!(
                "<|start_header_id|>{}<|end_header_id|>\n\n{}<|eot_id|>",
                role, msg.content
            ));
        }

        // Add assistant header for generation
        result.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
        result
    }

    /// Format messages in Llama 2 style
    fn format_llama2(messages: &[ChatMessage]) -> String {
        let mut result = String::new();
        let mut system_msg = String::new();
        let mut in_conversation = false;

        for msg in messages {
            match msg.role {
                Role::System => {
                    system_msg = msg.content.clone();
                }
                Role::User => {
                    if in_conversation {
                        result.push_str(" </s><s>");
                    }
                    result.push_str("[INST] ");
                    if !system_msg.is_empty() && !in_conversation {
                        result.push_str(&format!("<<SYS>>\n{}\n<</SYS>>\n\n", system_msg));
                    }
                    result.push_str(&msg.content);
                    result.push_str(" [/INST]");
                    in_conversation = true;
                }
                Role::Assistant => {
                    result.push(' ');
                    result.push_str(&msg.content);
                }
            }
        }

        result
    }

    /// Format messages in Mistral style
    fn format_mistral(messages: &[ChatMessage]) -> String {
        let mut result = String::new();
        let mut system_content = String::new();
        let mut awaiting_assistant = false;

        for msg in messages {
            match msg.role {
                Role::System => {
                    system_content = msg.content.clone();
                }
                Role::User => {
                    if awaiting_assistant {
                        result.push_str("</s>");
                    }
                    result.push_str("[INST] ");
                    if !system_content.is_empty() {
                        result.push_str(&system_content);
                        result.push_str("\n\n");
                        system_content.clear();
                    }
                    result.push_str(&msg.content);
                    result.push_str(" [/INST]");
                    awaiting_assistant = true;
                }
                Role::Assistant => {
                    result.push(' ');
                    result.push_str(&msg.content);
                }
            }
        }

        result
    }

    /// Format messages in ChatML/Qwen style
    fn format_chatml(messages: &[ChatMessage]) -> String {
        let mut result = String::new();

        for msg in messages {
            result.push_str(&format!(
                "<|im_start|>{}\n{}<|im_end|>\n",
                msg.role.as_str(),
                msg.content
            ));
        }

        // Add assistant start for generation
        result.push_str("<|im_start|>assistant\n");
        result
    }

    /// Format messages in Phi style
    fn format_phi(messages: &[ChatMessage]) -> String {
        let mut result = String::new();

        for msg in messages {
            let tag = match msg.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            result.push_str(&format!("<|{}|>\n{}<|end|>\n", tag, msg.content));
        }

        // Add assistant tag for generation
        result.push_str("<|assistant|>\n");
        result
    }

    /// Format messages in Gemma style
    fn format_gemma(messages: &[ChatMessage]) -> String {
        let mut result = String::new();

        for msg in messages {
            let role = match msg.role {
                Role::System => "system", // Gemma may not use system role
                Role::User => "user",
                Role::Assistant => "model",
            };
            result.push_str(&format!(
                "<start_of_turn>{}\n{}<end_of_turn>\n",
                role, msg.content
            ));
        }

        // Add model turn for generation
        result.push_str("<start_of_turn>model\n");
        result
    }

    /// Format messages using a custom template
    fn format_custom(template: &str, messages: &[ChatMessage]) -> String {
        let mut system_content = String::new();
        let mut user_content = String::new();
        let mut assistant_content = String::new();

        for msg in messages {
            match msg.role {
                Role::System => system_content.push_str(&msg.content),
                Role::User => user_content.push_str(&msg.content),
                Role::Assistant => assistant_content.push_str(&msg.content),
            }
        }

        template
            .replace("{system}", &system_content)
            .replace("{user}", &user_content)
            .replace("{assistant}", &assistant_content)
    }
}

/// Special tokens configuration
#[derive(Debug, Clone, Default)]
pub struct TokenizerSpecialTokens {
    /// End of sequence token ID
    pub eos_token_id: u32,
    /// Beginning of sequence token ID (optional)
    pub bos_token_id: Option<u32>,
    /// Padding token ID (optional)
    pub pad_token_id: Option<u32>,
    /// Unknown token ID (optional)
    pub unk_token_id: Option<u32>,
    /// End of text token (for some models)
    pub eot_token_id: Option<u32>,
    /// End of turn token (for chat models)
    pub end_turn_token_id: Option<u32>,
}

/// Buffer for streaming UTF-8 decode
#[derive(Debug, Default)]
pub struct StreamingDecodeBuffer {
    /// Accumulated bytes for incomplete UTF-8 sequences
    bytes: Vec<u8>,
    /// Previously decoded text for skip_special handling
    prev_text: String,
}

impl StreamingDecodeBuffer {
    /// Create a new streaming decode buffer
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the buffer
    pub fn reset(&mut self) {
        self.bytes.clear();
        self.prev_text.clear();
    }
}

// ============================================================================
// Candle-enabled implementation
// ============================================================================

#[cfg(feature = "candle")]
mod candle_impl {
    use super::*;

    /// HuggingFace tokenizer wrapper with chat template support
    pub struct RuvTokenizer {
        /// Underlying HuggingFace tokenizer
        inner: HfTokenizer,
        /// Chat template for this model
        chat_template: Option<ChatTemplate>,
        /// Special tokens
        special_tokens: TokenizerSpecialTokens,
        /// Model ID (for detection)
        model_id: String,
        /// Streaming decode buffer
        stream_buffer: StreamingDecodeBuffer,
        /// Added tokens for decoding (tokens added beyond base vocab)
        added_tokens: Vec<(u32, String)>,
    }

    impl RuvTokenizer {
        /// Load tokenizer from HuggingFace Hub
        ///
        /// # Arguments
        ///
        /// * `model_id` - HuggingFace model ID (e.g., "Qwen/Qwen2.5-0.5B-Instruct")
        ///
        /// # Example
        ///
        /// ```rust,ignore
        /// let tokenizer = RuvTokenizer::from_pretrained("mistralai/Mistral-7B-Instruct-v0.3")?;
        /// ```
        pub fn from_pretrained(model_id: &str) -> Result<Self> {
            let api = Api::new().map_err(|e| {
                RuvLLMError::Storage(format!("Failed to initialize HuggingFace API: {}", e))
            })?;

            let repo = api.repo(Repo::new(model_id.to_string(), RepoType::Model));

            let tokenizer_path = repo.get("tokenizer.json").map_err(|e| {
                RuvLLMError::NotFound(format!("Tokenizer not found for {}: {}", model_id, e))
            })?;

            let mut tokenizer = Self::from_file(&tokenizer_path)?;
            tokenizer.model_id = model_id.to_string();
            tokenizer.chat_template = Some(ChatTemplate::detect_from_model_id(model_id));

            // Try to load tokenizer_config.json for special tokens
            if let Ok(config_path) = repo.get("tokenizer_config.json") {
                tokenizer.load_special_tokens_from_config(&config_path)?;
            }

            Ok(tokenizer)
        }

        /// Load tokenizer from a local file
        ///
        /// # Arguments
        ///
        /// * `path` - Path to tokenizer.json file
        pub fn from_file(path: &Path) -> Result<Self> {
            let inner = HfTokenizer::from_file(path).map_err(|e| {
                RuvLLMError::Tokenization(format!("Failed to load tokenizer: {}", e))
            })?;

            let special_tokens = Self::extract_special_tokens(&inner);
            let added_tokens = Self::extract_added_tokens(&inner);

            Ok(Self {
                inner,
                chat_template: None,
                special_tokens,
                model_id: String::new(),
                stream_buffer: StreamingDecodeBuffer::new(),
                added_tokens,
            })
        }

        /// Create tokenizer from HfTokenizer directly
        pub fn from_hf_tokenizer(tokenizer: HfTokenizer, model_id: Option<&str>) -> Self {
            let special_tokens = Self::extract_special_tokens(&tokenizer);
            let added_tokens = Self::extract_added_tokens(&tokenizer);
            let chat_template = model_id.map(ChatTemplate::detect_from_model_id);

            Self {
                inner: tokenizer,
                chat_template,
                special_tokens,
                model_id: model_id.unwrap_or_default().to_string(),
                stream_buffer: StreamingDecodeBuffer::new(),
                added_tokens,
            }
        }

        /// Load special tokens from tokenizer_config.json
        fn load_special_tokens_from_config(&mut self, path: &Path) -> Result<()> {
            let config_str = std::fs::read_to_string(path).map_err(|e| {
                RuvLLMError::Storage(format!("Failed to read tokenizer config: {}", e))
            })?;

            let config: serde_json::Value = serde_json::from_str(&config_str)?;

            // Extract special token IDs
            if let Some(eos_id) = config.get("eos_token_id").and_then(|v| v.as_u64()) {
                self.special_tokens.eos_token_id = eos_id as u32;
            }

            if let Some(bos_id) = config.get("bos_token_id").and_then(|v| v.as_u64()) {
                self.special_tokens.bos_token_id = Some(bos_id as u32);
            }

            if let Some(pad_id) = config.get("pad_token_id").and_then(|v| v.as_u64()) {
                self.special_tokens.pad_token_id = Some(pad_id as u32);
            }

            if let Some(unk_id) = config.get("unk_token_id").and_then(|v| v.as_u64()) {
                self.special_tokens.unk_token_id = Some(unk_id as u32);
            }

            Ok(())
        }

        /// Extract special tokens from the tokenizer
        fn extract_special_tokens(tokenizer: &HfTokenizer) -> TokenizerSpecialTokens {
            // Common special token patterns across models
            let eos_candidates = [
                "</s>",
                "<|endoftext|>",
                "<|end_of_text|>",
                "<|im_end|>",
                "<|eot_id|>",
                "<eos>",
            ];

            let bos_candidates = [
                "<s>",
                "<|begin_of_text|>",
                "<|startoftext|>",
                "<|im_start|>",
                "<bos>",
            ];

            let pad_candidates = ["<pad>", "<|pad|>", "[PAD]"];
            let unk_candidates = ["<unk>", "<|unk|>", "[UNK]"];

            let find_token = |candidates: &[&str]| -> Option<u32> {
                for candidate in candidates {
                    if let Some(id) = tokenizer.token_to_id(candidate) {
                        return Some(id);
                    }
                }
                None
            };

            let eos_token_id = find_token(&eos_candidates).unwrap_or(0);

            TokenizerSpecialTokens {
                eos_token_id,
                bos_token_id: find_token(&bos_candidates),
                pad_token_id: find_token(&pad_candidates),
                unk_token_id: find_token(&unk_candidates),
                eot_token_id: tokenizer.token_to_id("<|eot_id|>"),
                end_turn_token_id: tokenizer
                    .token_to_id("<end_of_turn>")
                    .or_else(|| tokenizer.token_to_id("<|im_end|>")),
            }
        }

        /// Extract added tokens for proper decoding
        fn extract_added_tokens(tokenizer: &HfTokenizer) -> Vec<(u32, String)> {
            let mut added = Vec::new();

            // Try to get added tokens from the tokenizer
            // Note: This depends on tokenizers crate version and API
            let vocab = tokenizer.get_vocab(true);
            let base_vocab_size = tokenizer.get_vocab_size(false);

            for (token, id) in vocab {
                if id >= base_vocab_size as u32 {
                    added.push((id, token));
                }
            }

            added.sort_by_key(|(id, _)| *id);
            added
        }

        /// Set the chat template
        pub fn with_chat_template(mut self, template: ChatTemplate) -> Self {
            self.chat_template = Some(template);
            self
        }

        /// Set EOS token ID
        pub fn with_eos_token_id(mut self, eos_token_id: u32) -> Self {
            self.special_tokens.eos_token_id = eos_token_id;
            self
        }

        /// Set BOS token ID
        pub fn with_bos_token_id(mut self, bos_token_id: u32) -> Self {
            self.special_tokens.bos_token_id = Some(bos_token_id);
            self
        }

        /// Set padding token ID
        pub fn with_pad_token_id(mut self, pad_token_id: u32) -> Self {
            self.special_tokens.pad_token_id = Some(pad_token_id);
            self
        }

        /// Encode text to token IDs
        ///
        /// # Arguments
        ///
        /// * `text` - Input text to tokenize
        ///
        /// # Returns
        ///
        /// Vector of token IDs
        pub fn encode(&self, text: &str) -> Result<Vec<u32>> {
            let encoding = self
                .inner
                .encode(text, false)
                .map_err(|e| RuvLLMError::Tokenization(format!("Encoding failed: {}", e)))?;
            Ok(encoding.get_ids().to_vec())
        }

        /// Encode text with special tokens
        pub fn encode_with_special_tokens(&self, text: &str) -> Result<Vec<u32>> {
            let encoding = self
                .inner
                .encode(text, true)
                .map_err(|e| RuvLLMError::Tokenization(format!("Encoding failed: {}", e)))?;
            Ok(encoding.get_ids().to_vec())
        }

        /// Decode token IDs to text
        ///
        /// # Arguments
        ///
        /// * `tokens` - Slice of token IDs
        ///
        /// # Returns
        ///
        /// Decoded text string
        pub fn decode(&self, tokens: &[u32]) -> Result<String> {
            self.inner
                .decode(tokens, true)
                .map_err(|e| RuvLLMError::Tokenization(format!("Decoding failed: {}", e)))
        }

        /// Decode without skipping special tokens
        pub fn decode_with_special_tokens(&self, tokens: &[u32]) -> Result<String> {
            self.inner
                .decode(tokens, false)
                .map_err(|e| RuvLLMError::Tokenization(format!("Decoding failed: {}", e)))
        }

        /// Decode a single token for streaming output
        ///
        /// Handles multi-byte UTF-8 sequences gracefully by buffering
        /// incomplete sequences and returning them when complete.
        ///
        /// # Arguments
        ///
        /// * `token` - Single token ID to decode
        ///
        /// # Returns
        ///
        /// - `Ok(Some(text))` - Complete text to output
        /// - `Ok(None)` - Waiting for more bytes (incomplete UTF-8)
        ///
        /// # Example
        ///
        /// ```rust,ignore
        /// let mut tokenizer = RuvTokenizer::from_pretrained("...")?;
        /// for token in generated_tokens {
        ///     if let Some(text) = tokenizer.decode_stream(token)? {
        ///         print!("{}", text);
        ///     }
        /// }
        /// tokenizer.flush_stream()?; // Get any remaining bytes
        /// ```
        pub fn decode_stream(&mut self, token: u32) -> Result<Option<String>> {
            // Check if this is a special token we should skip
            if self.is_special_token(token) {
                return Ok(None);
            }

            // Get the raw bytes for this token
            let token_text = self
                .inner
                .decode(&[token], false)
                .map_err(|e| RuvLLMError::Tokenization(format!("Stream decode failed: {}", e)))?;

            // Check for replacement character (invalid UTF-8 indicator)
            if token_text.contains('\u{FFFD}') {
                // This token might be part of a multi-byte sequence
                // Try to decode with accumulated tokens
                let token_bytes = token_text.as_bytes();
                self.stream_buffer.bytes.extend_from_slice(token_bytes);

                // Try to decode accumulated bytes
                match std::str::from_utf8(&self.stream_buffer.bytes) {
                    Ok(s) => {
                        let result = s.to_string();
                        self.stream_buffer.bytes.clear();
                        Ok(Some(result))
                    }
                    Err(e) => {
                        // Check if this is a valid but incomplete sequence
                        let valid_up_to = e.valid_up_to();
                        if valid_up_to > 0 {
                            let valid_str =
                                std::str::from_utf8(&self.stream_buffer.bytes[..valid_up_to])
                                    .unwrap()
                                    .to_string();
                            self.stream_buffer.bytes =
                                self.stream_buffer.bytes[valid_up_to..].to_vec();
                            Ok(Some(valid_str))
                        } else {
                            // Still accumulating bytes
                            Ok(None)
                        }
                    }
                }
            } else {
                // Clean text, output directly
                // But first check if we have buffered bytes
                if !self.stream_buffer.bytes.is_empty() {
                    self.stream_buffer
                        .bytes
                        .extend_from_slice(token_text.as_bytes());
                    match std::str::from_utf8(&self.stream_buffer.bytes) {
                        Ok(s) => {
                            let result = s.to_string();
                            self.stream_buffer.bytes.clear();
                            Ok(Some(result))
                        }
                        Err(_) => {
                            // Something went wrong, output what we have
                            let lossy =
                                String::from_utf8_lossy(&self.stream_buffer.bytes).to_string();
                            self.stream_buffer.bytes.clear();
                            Ok(Some(lossy))
                        }
                    }
                } else {
                    Ok(Some(token_text))
                }
            }
        }

        /// Flush any remaining bytes in the streaming buffer
        ///
        /// Call this after streaming generation is complete to get any
        /// remaining buffered content.
        pub fn flush_stream(&mut self) -> Result<Option<String>> {
            if self.stream_buffer.bytes.is_empty() {
                return Ok(None);
            }

            let result = String::from_utf8_lossy(&self.stream_buffer.bytes).to_string();
            self.stream_buffer.bytes.clear();
            Ok(Some(result))
        }

        /// Reset the streaming buffer
        pub fn reset_stream(&mut self) {
            self.stream_buffer.reset();
        }

        /// Check if a token is a special token
        pub fn is_special_token(&self, token: u32) -> bool {
            token == self.special_tokens.eos_token_id
                || self.special_tokens.bos_token_id == Some(token)
                || self.special_tokens.pad_token_id == Some(token)
                || self.special_tokens.eot_token_id == Some(token)
                || self.special_tokens.end_turn_token_id == Some(token)
        }

        /// Apply chat template to messages
        ///
        /// # Arguments
        ///
        /// * `messages` - Slice of chat messages
        ///
        /// # Returns
        ///
        /// Formatted prompt string ready for tokenization
        pub fn apply_chat_template(&self, messages: &[ChatMessage]) -> Result<String> {
            let template = self
                .chat_template
                .as_ref()
                .ok_or_else(|| RuvLLMError::Config("No chat template configured".to_string()))?;

            Ok(template.format(messages))
        }

        /// Get vocabulary size
        pub fn vocab_size(&self) -> usize {
            self.inner.get_vocab_size(true)
        }

        /// Get EOS token ID
        pub fn eos_token_id(&self) -> u32 {
            self.special_tokens.eos_token_id
        }

        /// Get BOS token ID
        pub fn bos_token_id(&self) -> Option<u32> {
            self.special_tokens.bos_token_id
        }

        /// Get PAD token ID
        pub fn pad_token_id(&self) -> Option<u32> {
            self.special_tokens.pad_token_id
        }

        /// Get special tokens configuration
        pub fn special_tokens(&self) -> &TokenizerSpecialTokens {
            &self.special_tokens
        }

        /// Get the chat template
        pub fn chat_template(&self) -> Option<&ChatTemplate> {
            self.chat_template.as_ref()
        }

        /// Get model ID
        pub fn model_id(&self) -> &str {
            &self.model_id
        }

        /// Get the underlying HuggingFace tokenizer
        pub fn inner(&self) -> &HfTokenizer {
            &self.inner
        }

        /// Token to string (if in vocabulary)
        pub fn id_to_token(&self, id: u32) -> Option<String> {
            self.inner.id_to_token(id)
        }

        /// String to token (if in vocabulary)
        pub fn token_to_id(&self, token: &str) -> Option<u32> {
            self.inner.token_to_id(token)
        }

        /// Batch encode multiple texts
        pub fn encode_batch(&self, texts: &[&str]) -> Result<Vec<Vec<u32>>> {
            let encodings = self
                .inner
                .encode_batch(texts.to_vec(), false)
                .map_err(|e| RuvLLMError::Tokenization(format!("Batch encoding failed: {}", e)))?;

            Ok(encodings.iter().map(|e| e.get_ids().to_vec()).collect())
        }

        /// Batch decode multiple token sequences
        pub fn decode_batch(&self, token_sequences: &[Vec<u32>]) -> Result<Vec<String>> {
            token_sequences
                .iter()
                .map(|tokens| self.decode(tokens))
                .collect()
        }
    }

    impl std::fmt::Debug for RuvTokenizer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("RuvTokenizer")
                .field("model_id", &self.model_id)
                .field("vocab_size", &self.vocab_size())
                .field("chat_template", &self.chat_template)
                .field("special_tokens", &self.special_tokens)
                .finish()
        }
    }
}

// ============================================================================
// Stub implementation when candle feature is disabled
// ============================================================================

#[cfg(not(feature = "candle"))]
mod stub_impl {
    use super::*;

    /// Stub tokenizer for when candle feature is disabled
    #[derive(Debug)]
    pub struct RuvTokenizer {
        chat_template: Option<ChatTemplate>,
        special_tokens: TokenizerSpecialTokens,
    }

    impl Default for RuvTokenizer {
        fn default() -> Self {
            Self {
                chat_template: Some(ChatTemplate::default()),
                special_tokens: TokenizerSpecialTokens {
                    eos_token_id: 2,
                    bos_token_id: Some(1),
                    pad_token_id: Some(0),
                    unk_token_id: Some(3),
                    eot_token_id: None,
                    end_turn_token_id: None,
                },
            }
        }
    }

    impl RuvTokenizer {
        pub fn from_pretrained(_model_id: &str) -> Result<Self> {
            Err(RuvLLMError::Config(
                "Tokenizer requires 'candle' feature to be enabled".to_string(),
            ))
        }

        pub fn from_file(_path: &Path) -> Result<Self> {
            Err(RuvLLMError::Config(
                "Tokenizer requires 'candle' feature to be enabled".to_string(),
            ))
        }

        pub fn with_chat_template(mut self, template: ChatTemplate) -> Self {
            self.chat_template = Some(template);
            self
        }

        pub fn encode(&self, _text: &str) -> Result<Vec<u32>> {
            Err(RuvLLMError::Config(
                "Tokenizer requires 'candle' feature".to_string(),
            ))
        }

        pub fn decode(&self, _tokens: &[u32]) -> Result<String> {
            Err(RuvLLMError::Config(
                "Tokenizer requires 'candle' feature".to_string(),
            ))
        }

        pub fn decode_stream(&mut self, _token: u32) -> Result<Option<String>> {
            Err(RuvLLMError::Config(
                "Tokenizer requires 'candle' feature".to_string(),
            ))
        }

        pub fn flush_stream(&mut self) -> Result<Option<String>> {
            Ok(None)
        }

        pub fn reset_stream(&mut self) {}

        pub fn apply_chat_template(&self, messages: &[ChatMessage]) -> Result<String> {
            let template = self
                .chat_template
                .as_ref()
                .ok_or_else(|| RuvLLMError::Config("No chat template configured".to_string()))?;
            Ok(template.format(messages))
        }

        pub fn vocab_size(&self) -> usize {
            0
        }

        pub fn eos_token_id(&self) -> u32 {
            self.special_tokens.eos_token_id
        }

        pub fn bos_token_id(&self) -> Option<u32> {
            self.special_tokens.bos_token_id
        }

        pub fn pad_token_id(&self) -> Option<u32> {
            self.special_tokens.pad_token_id
        }

        pub fn special_tokens(&self) -> &TokenizerSpecialTokens {
            &self.special_tokens
        }

        pub fn chat_template(&self) -> Option<&ChatTemplate> {
            self.chat_template.as_ref()
        }
    }
}

// ============================================================================
// Public re-exports
// ============================================================================

#[cfg(feature = "candle")]
pub use candle_impl::RuvTokenizer;

#[cfg(not(feature = "candle"))]
pub use stub_impl::RuvTokenizer;

// ============================================================================
// Tokenizer Trait Implementation (for LlmBackend compatibility)
// ============================================================================

use crate::backends::{SpecialTokens, Tokenizer};

#[cfg(feature = "candle")]
impl Tokenizer for RuvTokenizer {
    fn encode(&self, text: &str) -> Result<Vec<u32>> {
        self.encode(text)
    }

    fn decode(&self, tokens: &[u32]) -> Result<String> {
        self.decode(tokens)
    }

    fn vocab_size(&self) -> usize {
        self.vocab_size()
    }

    fn special_tokens(&self) -> SpecialTokens {
        SpecialTokens {
            bos_token_id: self.bos_token_id(),
            eos_token_id: Some(self.eos_token_id()),
            pad_token_id: self.pad_token_id(),
            unk_token_id: None,
        }
    }
}

#[cfg(not(feature = "candle"))]
impl Tokenizer for RuvTokenizer {
    fn encode(&self, text: &str) -> Result<Vec<u32>> {
        self.encode(text)
    }

    fn decode(&self, tokens: &[u32]) -> Result<String> {
        self.decode(tokens)
    }

    fn vocab_size(&self) -> usize {
        0
    }

    fn special_tokens(&self) -> SpecialTokens {
        SpecialTokens {
            bos_token_id: self.bos_token_id(),
            eos_token_id: Some(self.eos_token_id()),
            pad_token_id: self.pad_token_id(),
            unk_token_id: None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_creation() {
        let system = ChatMessage::system("You are helpful.");
        assert_eq!(system.role, Role::System);
        assert_eq!(system.content, "You are helpful.");

        let user = ChatMessage::user("Hello!");
        assert_eq!(user.role, Role::User);

        let assistant = ChatMessage::assistant("Hi there!");
        assert_eq!(assistant.role, Role::Assistant);
    }

    #[test]
    fn test_role_as_str() {
        assert_eq!(Role::System.as_str(), "system");
        assert_eq!(Role::User.as_str(), "user");
        assert_eq!(Role::Assistant.as_str(), "assistant");
    }

    #[test]
    fn test_chat_template_detection() {
        assert_eq!(
            ChatTemplate::detect_from_model_id("meta-llama/Llama-3-8B-Instruct"),
            ChatTemplate::Llama3
        );
        assert_eq!(
            ChatTemplate::detect_from_model_id("meta-llama/Llama-2-7b-chat-hf"),
            ChatTemplate::Llama2
        );
        assert_eq!(
            ChatTemplate::detect_from_model_id("mistralai/Mistral-7B-Instruct-v0.3"),
            ChatTemplate::Mistral
        );
        assert_eq!(
            ChatTemplate::detect_from_model_id("Qwen/Qwen2.5-0.5B-Instruct"),
            ChatTemplate::Qwen
        );
        assert_eq!(
            ChatTemplate::detect_from_model_id("microsoft/Phi-3-mini-4k-instruct"),
            ChatTemplate::Phi
        );
        assert_eq!(
            ChatTemplate::detect_from_model_id("google/gemma-2b-it"),
            ChatTemplate::Gemma
        );
        assert_eq!(
            ChatTemplate::detect_from_model_id("unknown-model"),
            ChatTemplate::ChatML
        );
    }

    #[test]
    fn test_llama3_template() {
        let messages = vec![
            ChatMessage::system("You are helpful."),
            ChatMessage::user("What is Rust?"),
        ];

        let formatted = ChatTemplate::Llama3.format(&messages);

        assert!(formatted.contains("<|begin_of_text|>"));
        assert!(formatted.contains("<|start_header_id|>system<|end_header_id|>"));
        assert!(formatted.contains("You are helpful."));
        assert!(formatted.contains("<|start_header_id|>user<|end_header_id|>"));
        assert!(formatted.contains("What is Rust?"));
        assert!(formatted.contains("<|start_header_id|>assistant<|end_header_id|>"));
    }

    #[test]
    fn test_mistral_template() {
        let messages = vec![ChatMessage::system("Be concise."), ChatMessage::user("Hi")];

        let formatted = ChatTemplate::Mistral.format(&messages);

        assert!(formatted.contains("[INST]"));
        assert!(formatted.contains("Be concise."));
        assert!(formatted.contains("Hi"));
        assert!(formatted.contains("[/INST]"));
    }

    #[test]
    fn test_chatml_template() {
        let messages = vec![
            ChatMessage::system("You are an AI."),
            ChatMessage::user("Hello"),
        ];

        let formatted = ChatTemplate::ChatML.format(&messages);

        assert!(formatted.contains("<|im_start|>system"));
        assert!(formatted.contains("You are an AI."));
        assert!(formatted.contains("<|im_end|>"));
        assert!(formatted.contains("<|im_start|>user"));
        assert!(formatted.contains("<|im_start|>assistant"));
    }

    #[test]
    fn test_phi_template() {
        let messages = vec![ChatMessage::user("Hello"), ChatMessage::assistant("Hi!")];

        let formatted = ChatTemplate::Phi.format(&messages);

        assert!(formatted.contains("<|user|>"));
        assert!(formatted.contains("Hello"));
        assert!(formatted.contains("<|end|>"));
        assert!(formatted.contains("<|assistant|>"));
    }

    #[test]
    fn test_gemma_template() {
        let messages = vec![ChatMessage::user("Hi")];

        let formatted = ChatTemplate::Gemma.format(&messages);

        assert!(formatted.contains("<start_of_turn>user"));
        assert!(formatted.contains("Hi"));
        assert!(formatted.contains("<end_of_turn>"));
        assert!(formatted.contains("<start_of_turn>model"));
    }

    #[test]
    fn test_custom_template() {
        let template =
            ChatTemplate::Custom("System: {system}\nUser: {user}\nAssistant:".to_string());

        let messages = vec![ChatMessage::system("Be brief."), ChatMessage::user("Hello")];

        let formatted = template.format(&messages);

        assert!(formatted.contains("System: Be brief."));
        assert!(formatted.contains("User: Hello"));
        assert!(formatted.contains("Assistant:"));
    }

    #[test]
    fn test_special_tokens_default() {
        let tokens = TokenizerSpecialTokens::default();
        assert_eq!(tokens.eos_token_id, 0);
        assert!(tokens.bos_token_id.is_none());
    }

    #[test]
    fn test_streaming_buffer() {
        let mut buffer = StreamingDecodeBuffer::new();
        assert!(buffer.bytes.is_empty());

        buffer.bytes.push(0xE2);
        buffer.bytes.push(0x9C);
        buffer.bytes.push(0x93);

        buffer.reset();
        assert!(buffer.bytes.is_empty());
    }

    #[test]
    fn test_llama2_template() {
        let messages = vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("Hello"),
            ChatMessage::assistant("Hi there!"),
            ChatMessage::user("How are you?"),
        ];

        let formatted = ChatTemplate::Llama2.format(&messages);

        assert!(formatted.contains("<<SYS>>"));
        assert!(formatted.contains("You are a helpful assistant."));
        assert!(formatted.contains("<</SYS>>"));
        assert!(formatted.contains("[INST]"));
        assert!(formatted.contains("[/INST]"));
        assert!(formatted.contains("Hi there!"));
    }

    #[test]
    fn test_multi_turn_conversation() {
        let messages = vec![
            ChatMessage::system("Be helpful."),
            ChatMessage::user("What is 2+2?"),
            ChatMessage::assistant("4"),
            ChatMessage::user("And 3+3?"),
        ];

        // Test with ChatML
        let chatml = ChatTemplate::ChatML.format(&messages);
        assert!(chatml.contains("<|im_start|>user\nWhat is 2+2?"));
        assert!(chatml.contains("<|im_start|>assistant\n4"));
        assert!(chatml.contains("<|im_start|>user\nAnd 3+3?"));
    }

    #[cfg(not(feature = "candle"))]
    #[test]
    fn test_stub_tokenizer() {
        let tokenizer = RuvTokenizer::default();

        assert!(tokenizer.encode("test").is_err());
        assert!(tokenizer.decode(&[1, 2, 3]).is_err());

        // Chat template should work even without candle
        let messages = vec![ChatMessage::user("Hi")];
        let result = tokenizer.apply_chat_template(&messages);
        assert!(result.is_ok());
    }
}
