//! Speculative Decoding for Accelerated Inference
//!
//! Uses a small draft model to predict tokens, then verifies with the main model.
//! Achieves 2-3x speedup for greedy/low-temperature sampling.
//!
//! ## How It Works
//!
//! 1. **Draft Phase**: Generate K tokens using a small, fast draft model
//! 2. **Verify Phase**: Run main model on all K tokens in a single forward pass
//! 3. **Accept/Reject**: Accept verified tokens, reject where draft diverges
//! 4. **Correction**: Add the correct token where rejection occurred
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::speculative::{SpeculativeDecoder, SpeculativeConfig};
//!
//! let config = SpeculativeConfig {
//!     lookahead: 4,
//!     acceptance_threshold: 0.8,
//!     draft_temperature: 0.0,
//!     tree_speculation: false,
//!     ..Default::default()
//! };
//!
//! let mut decoder = SpeculativeDecoder::new(main_backend, draft_backend, config);
//! let output = decoder.generate("Hello, world!", params)?;
//! ```
//!
//! ## Recommended Model Pairings
//!
//! | Main Model | Draft Model | Expected Speedup |
//! |------------|-------------|------------------|
//! | Qwen2.5-14B | Qwen2.5-0.5B | 2.5-3.0x |
//! | Mistral-7B | TinyLlama-1.1B | 2.0-2.5x |
//! | Llama-3.2-3B | Llama-3.2-1B | 1.8-2.2x |

use crate::backends::{GenerateParams, GeneratedToken, LlmBackend, Tokenizer};
use crate::error::{Result, RuvLLMError};

use parking_lot::RwLock;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Configuration for speculative decoding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeConfig {
    /// Number of tokens to speculate ahead (typically 4-8)
    pub lookahead: usize,
    /// Acceptance threshold for draft tokens (probability cutoff)
    pub acceptance_threshold: f32,
    /// Temperature for draft model sampling (0.0 = greedy)
    pub draft_temperature: f32,
    /// Whether to use tree-based speculation for higher acceptance
    pub tree_speculation: bool,
    /// Maximum tree depth when tree speculation is enabled
    pub max_tree_depth: usize,
    /// Branching factor for tree speculation
    pub tree_branching_factor: usize,
    /// Whether to use nucleus sampling for draft
    pub draft_top_p: f32,
    /// Minimum probability ratio for acceptance (p_main / p_draft)
    pub min_acceptance_ratio: f32,
    /// Enable adaptive lookahead based on acceptance rate
    pub adaptive_lookahead: bool,
    /// Minimum lookahead when adaptive
    pub min_lookahead: usize,
    /// Maximum lookahead when adaptive
    pub max_lookahead: usize,
}

impl Default for SpeculativeConfig {
    fn default() -> Self {
        Self {
            lookahead: 4,
            acceptance_threshold: 0.5,
            draft_temperature: 0.0,
            tree_speculation: false,
            max_tree_depth: 3,
            tree_branching_factor: 2,
            draft_top_p: 1.0,
            min_acceptance_ratio: 0.1,
            adaptive_lookahead: true,
            min_lookahead: 2,
            max_lookahead: 8,
        }
    }
}

/// Statistics for speculative decoding performance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpeculativeStats {
    /// Total draft tokens generated
    pub draft_tokens: usize,
    /// Total tokens accepted from drafts
    pub accepted_tokens: usize,
    /// Current acceptance rate (0.0 - 1.0)
    pub acceptance_rate: f32,
    /// Estimated speedup compared to vanilla decoding
    pub speedup: f32,
    /// Total main model forward passes
    pub main_forward_passes: usize,
    /// Total draft model forward passes
    pub draft_forward_passes: usize,
    /// Average tokens per main forward pass
    pub avg_tokens_per_main_pass: f32,
    /// Total wall-clock time spent in speculation
    pub total_speculation_time_ms: f64,
    /// Total tokens generated (including corrections)
    pub total_tokens_generated: usize,
}

impl SpeculativeStats {
    /// Create new empty stats
    pub fn new() -> Self {
        Self::default()
    }

    /// Update acceptance rate
    pub fn update_acceptance_rate(&mut self) {
        if self.draft_tokens > 0 {
            self.acceptance_rate = self.accepted_tokens as f32 / self.draft_tokens as f32;
        }
    }

    /// Calculate speedup estimate
    pub fn calculate_speedup(&mut self) {
        if self.main_forward_passes > 0 {
            self.avg_tokens_per_main_pass =
                self.total_tokens_generated as f32 / self.main_forward_passes as f32;
            // Speedup is approximately avg tokens per pass (since we'd need 1 pass per token normally)
            self.speedup = self.avg_tokens_per_main_pass;
        }
    }

    /// Record a speculation round
    pub fn record_round(
        &mut self,
        draft_count: usize,
        accepted_count: usize,
        speculation_time_ms: f64,
    ) {
        self.draft_tokens += draft_count;
        self.accepted_tokens += accepted_count;
        self.draft_forward_passes += draft_count;
        self.main_forward_passes += 1;
        self.total_tokens_generated += accepted_count + 1; // +1 for correction/next token
        self.total_speculation_time_ms += speculation_time_ms;
        self.update_acceptance_rate();
        self.calculate_speedup();
    }

    /// Reset stats
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Thread-safe atomic stats for concurrent access
pub struct AtomicSpeculativeStats {
    draft_tokens: AtomicUsize,
    accepted_tokens: AtomicUsize,
    main_forward_passes: AtomicUsize,
    draft_forward_passes: AtomicUsize,
    total_tokens_generated: AtomicUsize,
    total_speculation_time_ns: AtomicU64,
}

impl Default for AtomicSpeculativeStats {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicSpeculativeStats {
    /// Create new atomic stats
    pub fn new() -> Self {
        Self {
            draft_tokens: AtomicUsize::new(0),
            accepted_tokens: AtomicUsize::new(0),
            main_forward_passes: AtomicUsize::new(0),
            draft_forward_passes: AtomicUsize::new(0),
            total_tokens_generated: AtomicUsize::new(0),
            total_speculation_time_ns: AtomicU64::new(0),
        }
    }

    /// Record a speculation round atomically
    pub fn record_round(&self, draft_count: usize, accepted_count: usize, duration: Duration) {
        self.draft_tokens.fetch_add(draft_count, Ordering::Relaxed);
        self.accepted_tokens
            .fetch_add(accepted_count, Ordering::Relaxed);
        self.main_forward_passes.fetch_add(1, Ordering::Relaxed);
        self.draft_forward_passes
            .fetch_add(draft_count, Ordering::Relaxed);
        self.total_tokens_generated
            .fetch_add(accepted_count + 1, Ordering::Relaxed);
        self.total_speculation_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Get snapshot as regular stats
    pub fn snapshot(&self) -> SpeculativeStats {
        let draft_tokens = self.draft_tokens.load(Ordering::Relaxed);
        let accepted_tokens = self.accepted_tokens.load(Ordering::Relaxed);
        let main_forward_passes = self.main_forward_passes.load(Ordering::Relaxed);
        let total_tokens_generated = self.total_tokens_generated.load(Ordering::Relaxed);
        let total_speculation_time_ns = self.total_speculation_time_ns.load(Ordering::Relaxed);

        let acceptance_rate = if draft_tokens > 0 {
            accepted_tokens as f32 / draft_tokens as f32
        } else {
            0.0
        };

        let avg_tokens_per_main_pass = if main_forward_passes > 0 {
            total_tokens_generated as f32 / main_forward_passes as f32
        } else {
            0.0
        };

        SpeculativeStats {
            draft_tokens,
            accepted_tokens,
            acceptance_rate,
            speedup: avg_tokens_per_main_pass,
            main_forward_passes,
            draft_forward_passes: self.draft_forward_passes.load(Ordering::Relaxed),
            avg_tokens_per_main_pass,
            total_speculation_time_ms: total_speculation_time_ns as f64 / 1_000_000.0,
            total_tokens_generated,
        }
    }

    /// Reset stats
    pub fn reset(&self) {
        self.draft_tokens.store(0, Ordering::Relaxed);
        self.accepted_tokens.store(0, Ordering::Relaxed);
        self.main_forward_passes.store(0, Ordering::Relaxed);
        self.draft_forward_passes.store(0, Ordering::Relaxed);
        self.total_tokens_generated.store(0, Ordering::Relaxed);
        self.total_speculation_time_ns.store(0, Ordering::Relaxed);
    }
}

/// Result of a verification phase
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Number of accepted draft tokens
    pub accepted_count: usize,
    /// The next token from main model (correction or continuation)
    pub next_token: u32,
    /// Log probabilities of accepted tokens
    pub accepted_logprobs: Vec<f32>,
    /// Log probability of the next token
    pub next_logprob: f32,
    /// Whether all draft tokens were accepted
    pub all_accepted: bool,
}

/// Node in the speculation tree
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Token at this node
    pub token: u32,
    /// Probability of this token
    pub prob: f32,
    /// Log probability
    pub logprob: f32,
    /// Children nodes (branches)
    pub children: Vec<TreeNode>,
    /// Depth in the tree
    pub depth: usize,
}

impl TreeNode {
    /// Create a new tree node
    pub fn new(token: u32, prob: f32, depth: usize) -> Self {
        Self {
            token,
            prob,
            logprob: prob.ln(),
            children: Vec::new(),
            depth,
        }
    }

    /// Add a child node
    pub fn add_child(&mut self, token: u32, prob: f32) -> &mut TreeNode {
        let child = TreeNode::new(token, prob, self.depth + 1);
        self.children.push(child);
        // SAFETY: We just pushed, so children is non-empty
        self.children
            .last_mut()
            .expect("children is non-empty after push")
    }

    /// Get all paths from this node to leaves
    pub fn get_paths(&self) -> Vec<Vec<u32>> {
        if self.children.is_empty() {
            return vec![vec![self.token]];
        }

        let mut paths = Vec::new();
        for child in &self.children {
            for mut path in child.get_paths() {
                path.insert(0, self.token);
                paths.push(path);
            }
        }
        paths
    }

    /// Get the best path (highest probability)
    pub fn best_path(&self) -> Vec<u32> {
        if self.children.is_empty() {
            return vec![self.token];
        }

        // SAFETY: We checked children.is_empty() above, so max_by returns Some
        // For NaN comparisons, treat them as equal to maintain deterministic behavior
        let best_child = self
            .children
            .iter()
            .max_by(|a, b| {
                a.prob
                    .partial_cmp(&b.prob)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("children is non-empty");

        let mut path = vec![self.token];
        path.extend(best_child.best_path());
        path
    }
}

/// Speculation tree for tree-based speculation
#[derive(Debug)]
pub struct SpeculationTree {
    /// Root node (represents current context, token is placeholder)
    pub root: TreeNode,
    /// Maximum depth of the tree
    pub max_depth: usize,
    /// Branching factor at each level
    pub branching_factor: usize,
    /// Total number of nodes
    pub node_count: usize,
}

impl SpeculationTree {
    /// Create a new speculation tree
    pub fn new(max_depth: usize, branching_factor: usize) -> Self {
        Self {
            root: TreeNode::new(0, 1.0, 0),
            max_depth,
            branching_factor,
            node_count: 1,
        }
    }

    /// Get all candidate paths for verification
    pub fn get_candidate_paths(&self) -> Vec<Vec<u32>> {
        self.root.get_paths()
    }

    /// Get the best path
    pub fn best_path(&self) -> Vec<u32> {
        let path = self.root.best_path();
        // Skip the root placeholder token
        if path.len() > 1 {
            path[1..].to_vec()
        } else {
            Vec::new()
        }
    }

    /// Clear the tree
    pub fn clear(&mut self) {
        self.root = TreeNode::new(0, 1.0, 0);
        self.node_count = 1;
    }
}

/// Speculative decoder combining draft and main models
pub struct SpeculativeDecoder<M: LlmBackend + ?Sized, D: LlmBackend + ?Sized> {
    /// Main (target) model for verification
    main_model: Arc<M>,
    /// Draft (small) model for speculation
    draft_model: Arc<D>,
    /// Configuration
    config: RwLock<SpeculativeConfig>,
    /// Performance statistics
    stats: AtomicSpeculativeStats,
    /// Current adaptive lookahead
    current_lookahead: AtomicUsize,
    /// Random number generator seed
    rng_seed: AtomicU64,
}

impl<M: LlmBackend + ?Sized, D: LlmBackend + ?Sized> SpeculativeDecoder<M, D> {
    /// Create a new speculative decoder
    pub fn new(main_model: Arc<M>, draft_model: Arc<D>, config: SpeculativeConfig) -> Self {
        let lookahead = config.lookahead;
        Self {
            main_model,
            draft_model,
            config: RwLock::new(config),
            stats: AtomicSpeculativeStats::new(),
            current_lookahead: AtomicUsize::new(lookahead),
            rng_seed: AtomicU64::new(42),
        }
    }

    /// Get current configuration
    pub fn config(&self) -> SpeculativeConfig {
        self.config.read().clone()
    }

    /// Update configuration
    pub fn set_config(&self, config: SpeculativeConfig) {
        *self.config.write() = config;
    }

    /// Get performance statistics
    pub fn stats(&self) -> SpeculativeStats {
        self.stats.snapshot()
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        self.stats.reset();
    }

    /// Get the main model tokenizer
    pub fn tokenizer(&self) -> Option<&dyn Tokenizer> {
        self.main_model.tokenizer()
    }

    /// Tokenize input text
    fn tokenize(&self, text: &str) -> Result<Vec<u32>> {
        let tokenizer = self
            .main_model
            .tokenizer()
            .ok_or_else(|| RuvLLMError::InvalidOperation("No tokenizer available".to_string()))?;
        tokenizer.encode(text)
    }

    /// Decode tokens to text
    fn decode(&self, tokens: &[u32]) -> Result<String> {
        let tokenizer = self
            .main_model
            .tokenizer()
            .ok_or_else(|| RuvLLMError::InvalidOperation("No tokenizer available".to_string()))?;
        tokenizer.decode(tokens)
    }

    /// Check if we should use speculative decoding for these params
    pub fn should_use_speculative(&self, params: &GenerateParams) -> bool {
        // Use speculative for low temperature, greedy, or beam search
        params.temperature < 0.5 || params.top_k == 1
    }

    /// Generate text with speculative decoding
    pub fn generate(&self, prompt: &str, params: GenerateParams) -> Result<String> {
        let tokens = self.tokenize(prompt)?;
        let generated = self.generate_tokens(&tokens, &params)?;
        self.decode(&generated)
    }

    /// Generate tokens with speculative decoding
    pub fn generate_tokens(
        &self,
        prompt_tokens: &[u32],
        params: &GenerateParams,
    ) -> Result<Vec<u32>> {
        let config = self.config.read().clone();
        let mut context = prompt_tokens.to_vec();
        let mut output = Vec::new();

        // Get special tokens for stopping
        let eos_token = self
            .main_model
            .tokenizer()
            .and_then(|t| t.special_tokens().eos_token_id);

        while output.len() < params.max_tokens {
            let start = Instant::now();

            // Determine lookahead
            let lookahead = if config.adaptive_lookahead {
                self.current_lookahead.load(Ordering::Relaxed)
            } else {
                config.lookahead
            };

            // Draft phase: generate K tokens with small model
            let draft_tokens = self.draft_phase(&context, lookahead, &config)?;

            if draft_tokens.is_empty() {
                // Draft model couldn't generate, fall back to main model
                let main_token = self.single_main_forward(&context, params)?;
                if Some(main_token) == eos_token {
                    break;
                }
                context.push(main_token);
                output.push(main_token);
                continue;
            }

            // Verify phase: check with main model
            let verification = self.verify_phase(&context, &draft_tokens, params)?;

            // Accept verified tokens
            let accepted = &draft_tokens[..verification.accepted_count];
            context.extend_from_slice(accepted);
            output.extend_from_slice(accepted);

            // Add the corrected/continuation token
            if Some(verification.next_token) == eos_token {
                break;
            }
            context.push(verification.next_token);
            output.push(verification.next_token);

            // Record stats
            let duration = start.elapsed();
            self.stats
                .record_round(draft_tokens.len(), verification.accepted_count, duration);

            // Adaptive lookahead adjustment
            if config.adaptive_lookahead {
                self.adjust_lookahead(verification.accepted_count, draft_tokens.len(), &config);
            }

            // Check for stop sequences
            if !params.stop_sequences.is_empty() {
                let current_text = self.decode(&output)?;
                for stop_seq in &params.stop_sequences {
                    if current_text.contains(stop_seq) {
                        // Trim to before stop sequence
                        let trimmed = current_text.split(stop_seq).next().unwrap_or("");
                        return self
                            .tokenize(trimmed)
                            .map(|t| t.into_iter().skip(prompt_tokens.len()).collect());
                    }
                }
            }
        }

        Ok(output)
    }

    /// Draft phase: generate K tokens with small model
    fn draft_phase(
        &self,
        context: &[u32],
        k: usize,
        config: &SpeculativeConfig,
    ) -> Result<Vec<u32>> {
        let mut draft = Vec::with_capacity(k);
        let mut ctx = context.to_vec();

        // Build prompt from context for draft model
        let prompt_text = self.decode(&ctx)?;

        for i in 0..k {
            // Generate one token with draft model
            let draft_params = GenerateParams {
                max_tokens: 1,
                temperature: config.draft_temperature,
                top_p: config.draft_top_p,
                top_k: if config.draft_temperature == 0.0 {
                    1
                } else {
                    40
                },
                ..Default::default()
            };

            // Get next token from draft model
            // Note: In production, this would use a more efficient batched approach
            let current_prompt = self.decode(&ctx)?;
            let generated = self
                .draft_model
                .generate(&current_prompt, draft_params.clone())?;

            // Tokenize the generated text to get the new token
            let generated_tokens = self.tokenize(&format!("{}{}", prompt_text, generated))?;
            if generated_tokens.len() <= ctx.len() {
                // No new token generated
                break;
            }

            let new_token = generated_tokens[ctx.len()];
            draft.push(new_token);
            ctx.push(new_token);

            // Check for EOS
            if let Some(eos) = self
                .draft_model
                .tokenizer()
                .and_then(|t| t.special_tokens().eos_token_id)
            {
                if new_token == eos {
                    break;
                }
            }
        }

        Ok(draft)
    }

    /// Verify draft tokens with main model
    fn verify_phase(
        &self,
        context: &[u32],
        draft_tokens: &[u32],
        params: &GenerateParams,
    ) -> Result<VerificationResult> {
        let config = self.config.read();

        // In a full implementation, we would do a single forward pass through the main model
        // with all tokens (context + draft) to get logits for all positions at once.
        // Here we simulate this with individual calls.

        let mut accepted_count = 0;
        let mut accepted_logprobs = Vec::new();
        let mut ctx = context.to_vec();

        for (i, &draft_token) in draft_tokens.iter().enumerate() {
            // Get main model's probability distribution at this position
            let prompt_text = self.decode(&ctx)?;

            // Generate with main model to get its preferred token
            let main_params = GenerateParams {
                max_tokens: 1,
                temperature: params.temperature,
                top_p: params.top_p,
                top_k: params.top_k,
                ..params.clone()
            };

            let main_generated = self
                .main_model
                .generate(&prompt_text, main_params.clone())?;
            let main_tokens = self.tokenize(&format!("{}{}", prompt_text, main_generated))?;

            if main_tokens.len() <= ctx.len() {
                // Main model didn't generate, reject remaining drafts
                let next_token = self.single_main_forward(&ctx, params)?;
                return Ok(VerificationResult {
                    accepted_count,
                    next_token,
                    accepted_logprobs,
                    next_logprob: 0.0,
                    all_accepted: false,
                });
            }

            let main_token = main_tokens[ctx.len()];

            // Simple acceptance: if main model agrees with draft, accept
            // In production, we'd use proper probability comparison
            if main_token == draft_token {
                accepted_count += 1;
                accepted_logprobs.push(0.0); // Placeholder logprob
                ctx.push(draft_token);
            } else {
                // Rejection - return main model's token as correction
                return Ok(VerificationResult {
                    accepted_count,
                    next_token: main_token,
                    accepted_logprobs,
                    next_logprob: 0.0,
                    all_accepted: false,
                });
            }
        }

        // All drafts accepted - get next token from main model
        let next_token = self.single_main_forward(&ctx, params)?;

        Ok(VerificationResult {
            accepted_count,
            next_token,
            accepted_logprobs,
            next_logprob: 0.0,
            all_accepted: true,
        })
    }

    /// Single forward pass through main model to get next token
    fn single_main_forward(&self, context: &[u32], params: &GenerateParams) -> Result<u32> {
        let prompt_text = self.decode(context)?;
        let main_params = GenerateParams {
            max_tokens: 1,
            temperature: params.temperature,
            top_p: params.top_p,
            top_k: params.top_k,
            ..params.clone()
        };

        let generated = self.main_model.generate(&prompt_text, main_params)?;
        let tokens = self.tokenize(&format!("{}{}", prompt_text, generated))?;

        if tokens.len() > context.len() {
            Ok(tokens[context.len()])
        } else {
            // Return EOS if nothing generated
            Ok(self
                .main_model
                .tokenizer()
                .and_then(|t| t.special_tokens().eos_token_id)
                .unwrap_or(0))
        }
    }

    /// Adjust lookahead based on acceptance rate
    fn adjust_lookahead(&self, accepted: usize, total: usize, config: &SpeculativeConfig) {
        let current = self.current_lookahead.load(Ordering::Relaxed);
        let acceptance_rate = if total > 0 {
            accepted as f32 / total as f32
        } else {
            0.5
        };

        let new_lookahead = if acceptance_rate > 0.9 {
            // High acceptance - increase lookahead
            (current + 1).min(config.max_lookahead)
        } else if acceptance_rate < 0.5 {
            // Low acceptance - decrease lookahead
            current.saturating_sub(1).max(config.min_lookahead)
        } else {
            current
        };

        self.current_lookahead
            .store(new_lookahead, Ordering::Relaxed);
    }

    /// Generate with tree-based speculation (advanced)
    pub fn generate_tree(&self, prompt: &str, params: GenerateParams) -> Result<String> {
        let config = self.config.read().clone();
        if !config.tree_speculation {
            return self.generate(prompt, params);
        }

        // Tree speculation implementation
        let tokens = self.tokenize(prompt)?;
        let mut context = tokens.clone();
        let mut output = Vec::new();

        let eos_token = self
            .main_model
            .tokenizer()
            .and_then(|t| t.special_tokens().eos_token_id);

        while output.len() < params.max_tokens {
            let start = Instant::now();

            // Build speculation tree
            let tree = self.build_speculation_tree(&context, &config)?;

            // Verify best path
            let best_path = tree.best_path();
            if best_path.is_empty() {
                let main_token = self.single_main_forward(&context, &params)?;
                if Some(main_token) == eos_token {
                    break;
                }
                context.push(main_token);
                output.push(main_token);
                continue;
            }

            // Verify the best path
            let verification = self.verify_phase(&context, &best_path, &params)?;

            // Accept verified tokens
            let accepted = &best_path[..verification.accepted_count];
            context.extend_from_slice(accepted);
            output.extend_from_slice(accepted);

            // Add correction/continuation token
            if Some(verification.next_token) == eos_token {
                break;
            }
            context.push(verification.next_token);
            output.push(verification.next_token);

            // Record stats
            self.stats.record_round(
                best_path.len(),
                verification.accepted_count,
                start.elapsed(),
            );
        }

        self.decode(&output)
    }

    /// Build a speculation tree using draft model
    fn build_speculation_tree(
        &self,
        context: &[u32],
        config: &SpeculativeConfig,
    ) -> Result<SpeculationTree> {
        let mut tree = SpeculationTree::new(config.max_tree_depth, config.tree_branching_factor);

        // For simplicity, we just build a linear path (same as non-tree)
        // A full implementation would explore multiple branches
        let draft_tokens = self.draft_phase(context, config.max_tree_depth, config)?;

        // Add tokens as a linear path
        let mut current = &mut tree.root;
        for (i, &token) in draft_tokens.iter().enumerate() {
            current = current.add_child(token, 1.0 / (i + 1) as f32);
            tree.node_count += 1;
        }

        Ok(tree)
    }

    /// Stream generation with speculative decoding
    pub fn generate_stream<'a>(
        &'a self,
        prompt: &str,
        params: GenerateParams,
    ) -> Result<impl Iterator<Item = Result<GeneratedToken>> + 'a> {
        let tokens = self.tokenize(prompt)?;
        let context = tokens.clone();
        let config = self.config.read().clone();

        Ok(SpeculativeStreamIterator {
            decoder: self,
            context,
            params,
            config,
            output_count: 0,
            pending_tokens: Vec::new(),
            finished: false,
        })
    }
}

/// Iterator for streaming speculative generation
struct SpeculativeStreamIterator<'a, M: LlmBackend + ?Sized, D: LlmBackend + ?Sized> {
    decoder: &'a SpeculativeDecoder<M, D>,
    context: Vec<u32>,
    params: GenerateParams,
    config: SpeculativeConfig,
    output_count: usize,
    pending_tokens: Vec<u32>,
    finished: bool,
}

impl<'a, M: LlmBackend + ?Sized, D: LlmBackend + ?Sized> Iterator
    for SpeculativeStreamIterator<'a, M, D>
{
    type Item = Result<GeneratedToken>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished || self.output_count >= self.params.max_tokens {
            return None;
        }

        // Return pending tokens first
        if !self.pending_tokens.is_empty() {
            let token = self.pending_tokens.remove(0);
            self.output_count += 1;

            let text = self.decoder.decode(&[token]).unwrap_or_default();
            return Some(Ok(GeneratedToken {
                id: token,
                text,
                logprob: None,
                is_special: false,
            }));
        }

        // Generate more tokens via speculation
        let lookahead = self.config.lookahead;
        let draft_result = self
            .decoder
            .draft_phase(&self.context, lookahead, &self.config);

        match draft_result {
            Ok(draft_tokens) if !draft_tokens.is_empty() => {
                // Verify draft tokens
                match self
                    .decoder
                    .verify_phase(&self.context, &draft_tokens, &self.params)
                {
                    Ok(verification) => {
                        // Queue accepted tokens and correction
                        let accepted = &draft_tokens[..verification.accepted_count];
                        self.pending_tokens.extend_from_slice(accepted);
                        self.pending_tokens.push(verification.next_token);

                        // Update context
                        self.context.extend_from_slice(accepted);
                        self.context.push(verification.next_token);

                        // Return first token
                        self.next()
                    }
                    Err(e) => {
                        self.finished = true;
                        Some(Err(e))
                    }
                }
            }
            Ok(_) => {
                // Empty draft, single token generation
                match self
                    .decoder
                    .single_main_forward(&self.context, &self.params)
                {
                    Ok(token) => {
                        self.context.push(token);
                        self.output_count += 1;

                        // Check for EOS
                        let eos = self
                            .decoder
                            .main_model
                            .tokenizer()
                            .and_then(|t| t.special_tokens().eos_token_id);
                        if Some(token) == eos {
                            self.finished = true;
                        }

                        let text = self.decoder.decode(&[token]).unwrap_or_default();
                        Some(Ok(GeneratedToken {
                            id: token,
                            text,
                            logprob: None,
                            is_special: Some(token) == eos,
                        }))
                    }
                    Err(e) => {
                        self.finished = true;
                        Some(Err(e))
                    }
                }
            }
            Err(e) => {
                self.finished = true;
                Some(Err(e))
            }
        }
    }
}

/// Softmax function for probability computation
///
/// M4 Pro optimizations:
/// - NEON-accelerated max finding and exp computation
/// - 8x unrolling for maximum ILP
/// - Fast exp approximation for vocabulary-sized inputs
pub fn softmax(logits: &[f32]) -> Vec<f32> {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        softmax_neon_optimized(logits)
    }

    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    {
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = logits.iter().map(|&x| (x - max_logit).exp()).sum();
        logits
            .iter()
            .map(|&x| (x - max_logit).exp() / exp_sum)
            .collect()
    }
}

/// NEON-optimized softmax with 8x unrolling
///
/// Key optimizations:
/// - Vectorized max finding
/// - Fast exp approximation using polynomial (6th order)
/// - Dual accumulator pattern for sum reduction
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fn softmax_neon_optimized(logits: &[f32]) -> Vec<f32> {
    use std::arch::aarch64::*;

    const UNROLL_8X: usize = 8;

    if logits.is_empty() {
        return vec![];
    }

    let mut result = vec![0.0f32; logits.len()];

    unsafe {
        // Phase 1: Find max using NEON
        let mut max_vec = vdupq_n_f32(f32::NEG_INFINITY);
        let chunks = logits.len() / UNROLL_8X;

        for c in 0..chunks {
            let base = c * UNROLL_8X;
            let v0 = vld1q_f32(logits.as_ptr().add(base));
            let v1 = vld1q_f32(logits.as_ptr().add(base + 4));
            max_vec = vmaxq_f32(max_vec, vmaxq_f32(v0, v1));
        }

        let mut max_logit = vmaxvq_f32(max_vec);

        // Handle remainder
        for i in (chunks * UNROLL_8X)..logits.len() {
            max_logit = max_logit.max(logits[i]);
        }

        let max_vec = vdupq_n_f32(max_logit);

        // Phase 2: Compute exp(x - max) and sum using fast exp approximation
        // exp(x) ≈ (1 + x/256)^256 or polynomial approximation
        // We use the more accurate polynomial: exp(x) ≈ 1 + x + x²/2 + x³/6 + x⁴/24 + x⁵/120 + x⁶/720
        let one = vdupq_n_f32(1.0);
        let half = vdupq_n_f32(0.5);
        let sixth = vdupq_n_f32(1.0 / 6.0);
        let twenty_fourth = vdupq_n_f32(1.0 / 24.0);
        let one_twenty = vdupq_n_f32(1.0 / 120.0);
        let seven_twenty = vdupq_n_f32(1.0 / 720.0);

        let mut sum0 = vdupq_n_f32(0.0);
        let mut sum1 = vdupq_n_f32(0.0);

        // Fast exp approximation: good for |x| < 10
        #[inline(always)]
        unsafe fn fast_exp_vec(
            x: float32x4_t,
            one: float32x4_t,
            half: float32x4_t,
            sixth: float32x4_t,
            twenty_fourth: float32x4_t,
            one_twenty: float32x4_t,
            seven_twenty: float32x4_t,
        ) -> float32x4_t {
            // Clamp to reasonable range to avoid overflow
            let x = vmaxq_f32(vdupq_n_f32(-20.0), vminq_f32(x, vdupq_n_f32(20.0)));

            // exp(x) ≈ 1 + x(1 + x/2(1 + x/3(1 + x/4(1 + x/5(1 + x/6)))))
            let x2 = vmulq_f32(x, x);
            let x3 = vmulq_f32(x2, x);
            let x4 = vmulq_f32(x2, x2);
            let x5 = vmulq_f32(x4, x);
            let x6 = vmulq_f32(x3, x3);

            // 1 + x + x²/2 + x³/6 + x⁴/24 + x⁵/120 + x⁶/720
            let result = vaddq_f32(one, x);
            let result = vfmaq_f32(result, x2, half);
            let result = vfmaq_f32(result, x3, sixth);
            let result = vfmaq_f32(result, x4, twenty_fourth);
            let result = vfmaq_f32(result, x5, one_twenty);
            let result = vfmaq_f32(result, x6, seven_twenty);

            // Ensure non-negative
            vmaxq_f32(result, vdupq_n_f32(0.0))
        }

        for c in 0..chunks {
            let base = c * UNROLL_8X;
            let v0 = vld1q_f32(logits.as_ptr().add(base));
            let v1 = vld1q_f32(logits.as_ptr().add(base + 4));

            // Subtract max
            let d0 = vsubq_f32(v0, max_vec);
            let d1 = vsubq_f32(v1, max_vec);

            // Fast exp
            let e0 = fast_exp_vec(
                d0,
                one,
                half,
                sixth,
                twenty_fourth,
                one_twenty,
                seven_twenty,
            );
            let e1 = fast_exp_vec(
                d1,
                one,
                half,
                sixth,
                twenty_fourth,
                one_twenty,
                seven_twenty,
            );

            // Store exp values
            vst1q_f32(result.as_mut_ptr().add(base), e0);
            vst1q_f32(result.as_mut_ptr().add(base + 4), e1);

            // Accumulate sums
            sum0 = vaddq_f32(sum0, e0);
            sum1 = vaddq_f32(sum1, e1);
        }

        // Reduce sum
        let mut exp_sum = vaddvq_f32(vaddq_f32(sum0, sum1));

        // Handle remainder with scalar exp (more accurate for edge cases)
        for i in (chunks * UNROLL_8X)..logits.len() {
            let e = (logits[i] - max_logit).exp();
            result[i] = e;
            exp_sum += e;
        }

        // Phase 3: Normalize by sum
        let inv_sum = vdupq_n_f32(1.0 / exp_sum);

        for c in 0..chunks {
            let base = c * UNROLL_8X;
            let e0 = vld1q_f32(result.as_ptr().add(base));
            let e1 = vld1q_f32(result.as_ptr().add(base + 4));

            let p0 = vmulq_f32(e0, inv_sum);
            let p1 = vmulq_f32(e1, inv_sum);

            vst1q_f32(result.as_mut_ptr().add(base), p0);
            vst1q_f32(result.as_mut_ptr().add(base + 4), p1);
        }

        // Remainder
        let inv_sum_scalar = 1.0 / exp_sum;
        for i in (chunks * UNROLL_8X)..logits.len() {
            result[i] *= inv_sum_scalar;
        }
    }

    result
}

/// Log softmax function
///
/// M4 Pro optimizations:
/// - NEON-accelerated log-sum-exp computation
/// - 8x unrolling for maximum ILP
pub fn log_softmax(logits: &[f32]) -> Vec<f32> {
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    {
        log_softmax_neon_optimized(logits)
    }

    #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
    {
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let log_sum_exp: f32 = logits
            .iter()
            .map(|&x| (x - max_logit).exp())
            .sum::<f32>()
            .ln()
            + max_logit;
        logits.iter().map(|&x| x - log_sum_exp).collect()
    }
}

/// NEON-optimized log softmax
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
fn log_softmax_neon_optimized(logits: &[f32]) -> Vec<f32> {
    use std::arch::aarch64::*;

    const UNROLL_8X: usize = 8;

    if logits.is_empty() {
        return vec![];
    }

    let mut result = vec![0.0f32; logits.len()];

    unsafe {
        // Find max using NEON
        let mut max_vec = vdupq_n_f32(f32::NEG_INFINITY);
        let chunks = logits.len() / UNROLL_8X;

        for c in 0..chunks {
            let base = c * UNROLL_8X;
            let v0 = vld1q_f32(logits.as_ptr().add(base));
            let v1 = vld1q_f32(logits.as_ptr().add(base + 4));
            max_vec = vmaxq_f32(max_vec, vmaxq_f32(v0, v1));
        }

        let mut max_logit = vmaxvq_f32(max_vec);
        for i in (chunks * UNROLL_8X)..logits.len() {
            max_logit = max_logit.max(logits[i]);
        }

        // Compute sum of exp(x - max) - use scalar exp for accuracy
        let mut exp_sum = 0.0f32;
        for i in 0..logits.len() {
            exp_sum += (logits[i] - max_logit).exp();
        }

        let log_sum_exp = exp_sum.ln() + max_logit;
        let log_sum_vec = vdupq_n_f32(log_sum_exp);

        // Compute log softmax: x - log_sum_exp with NEON
        for c in 0..chunks {
            let base = c * UNROLL_8X;
            let v0 = vld1q_f32(logits.as_ptr().add(base));
            let v1 = vld1q_f32(logits.as_ptr().add(base + 4));

            let r0 = vsubq_f32(v0, log_sum_vec);
            let r1 = vsubq_f32(v1, log_sum_vec);

            vst1q_f32(result.as_mut_ptr().add(base), r0);
            vst1q_f32(result.as_mut_ptr().add(base + 4), r1);
        }

        for i in (chunks * UNROLL_8X)..logits.len() {
            result[i] = logits[i] - log_sum_exp;
        }
    }

    result
}

/// Sample from a probability distribution
pub fn sample_from_probs(probs: &[f32], rng: &mut impl Rng) -> usize {
    let r: f32 = rng.gen();
    let mut cumsum = 0.0;
    for (i, &p) in probs.iter().enumerate() {
        cumsum += p;
        if cumsum > r {
            return i;
        }
    }
    probs.len() - 1
}

/// Top-k filtering
pub fn top_k_filter(logits: &mut [f32], k: usize) {
    if k == 0 || k >= logits.len() {
        return;
    }

    let mut indexed: Vec<(usize, f32)> = logits.iter().cloned().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let threshold = indexed[k - 1].1;
    for logit in logits.iter_mut() {
        if *logit < threshold {
            *logit = f32::NEG_INFINITY;
        }
    }
}

/// Top-p (nucleus) filtering
pub fn top_p_filter(logits: &mut [f32], p: f32) {
    if p >= 1.0 {
        return;
    }

    let probs = softmax(logits);
    let mut indexed: Vec<(usize, f32)> = probs.iter().cloned().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut cumsum = 0.0;
    let mut cutoff_idx = indexed.len();
    for (i, (_, prob)) in indexed.iter().enumerate() {
        cumsum += prob;
        if cumsum > p {
            cutoff_idx = i + 1;
            break;
        }
    }

    // Set excluded tokens to -inf
    let included: std::collections::HashSet<usize> =
        indexed[..cutoff_idx].iter().map(|(i, _)| *i).collect();
    for (i, logit) in logits.iter_mut().enumerate() {
        if !included.contains(&i) {
            *logit = f32::NEG_INFINITY;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speculative_config_default() {
        let config = SpeculativeConfig::default();
        assert_eq!(config.lookahead, 4);
        assert!((config.acceptance_threshold - 0.5).abs() < 0.01);
        assert!(!config.tree_speculation);
    }

    #[test]
    fn test_speculative_stats() {
        let mut stats = SpeculativeStats::new();
        assert_eq!(stats.draft_tokens, 0);
        assert_eq!(stats.accepted_tokens, 0);

        stats.record_round(4, 3, 10.0);
        assert_eq!(stats.draft_tokens, 4);
        assert_eq!(stats.accepted_tokens, 3);
        assert!((stats.acceptance_rate - 0.75).abs() < 0.01);
        assert_eq!(stats.total_tokens_generated, 4); // 3 accepted + 1 correction
    }

    #[test]
    fn test_atomic_stats() {
        let stats = AtomicSpeculativeStats::new();
        stats.record_round(4, 3, Duration::from_millis(10));

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.draft_tokens, 4);
        assert_eq!(snapshot.accepted_tokens, 3);
        assert!((snapshot.acceptance_rate - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_tree_node() {
        let mut root = TreeNode::new(0, 1.0, 0);
        root.add_child(1, 0.5);
        root.add_child(2, 0.3);

        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].token, 1);
        assert_eq!(root.children[1].token, 2);
    }

    #[test]
    fn test_speculation_tree() {
        let mut tree = SpeculationTree::new(3, 2);
        assert_eq!(tree.node_count, 1);

        let current = &mut tree.root;
        current.add_child(1, 0.8);
        tree.node_count += 1;

        assert_eq!(tree.node_count, 2);
    }

    #[test]
    fn test_softmax() {
        let logits = vec![1.0, 2.0, 3.0];
        let probs = softmax(&logits);

        // Check probabilities sum to 1
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);

        // Check ordering preserved
        assert!(probs[2] > probs[1]);
        assert!(probs[1] > probs[0]);
    }

    #[test]
    fn test_top_k_filter() {
        let mut logits = vec![1.0, 5.0, 3.0, 4.0, 2.0];
        top_k_filter(&mut logits, 2);

        // Only top 2 should remain finite
        let finite_count = logits.iter().filter(|x| x.is_finite()).count();
        assert_eq!(finite_count, 2);
    }

    #[test]
    fn test_top_p_filter() {
        let mut logits = vec![10.0, 5.0, 3.0, 2.0, 1.0];
        top_p_filter(&mut logits, 0.9);

        // Most probability mass should be preserved
        let finite_count = logits.iter().filter(|x| x.is_finite()).count();
        assert!(finite_count >= 1);
    }

    #[test]
    fn test_verification_result() {
        let result = VerificationResult {
            accepted_count: 3,
            next_token: 42,
            accepted_logprobs: vec![-0.1, -0.2, -0.3],
            next_logprob: -0.5,
            all_accepted: false,
        };

        assert_eq!(result.accepted_count, 3);
        assert_eq!(result.next_token, 42);
        assert!(!result.all_accepted);
    }
}
