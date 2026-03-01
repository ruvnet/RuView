//! Speculative decoding with EAGLE-3 style draft trees.
//!
//! Uses mincut λ-stability as draft acceptance confidence signal.
//! Dynamic tree structure adapts to model confidence.
//!
//! # EAGLE-3 Algorithm
//!
//! EAGLE-3 (NeurIPS 2025) uses:
//! 1. **Draft tree generation**: Dynamic tree structure based on confidence
//! 2. **Multi-level feature fusion**: Uses λ-stability as confidence signal
//! 3. **Rejection sampling**: Verify drafts against target model
//! 4. **Tree attention**: Parallel verification of draft tokens
//!
//! # Example
//!
//! ```rust
//! use ruvector_mincut_gated_transformer::speculative::*;
//!
//! let config = SpeculativeConfig {
//!     max_draft_tokens: 5,
//!     tree_width: 3,
//!     acceptance_threshold: 0.7,
//!     use_lambda_guidance: true,
//! };
//!
//! let decoder = SpeculativeDecoder::new(config);
//!
//! // Generate draft tree using λ-guided confidence
//! let lambda = 100;
//! let lambda_prev = 95;
//! let draft_logits = vec![vec![0.0; 1000]; 5];
//! let tree = decoder.generate_draft_tree(lambda, lambda_prev, &draft_logits);
//!
//! // Verify against target model
//! let target_logits = vec![vec![0.0; 1000]; 5];
//! let result = decoder.verify_drafts(&tree, &target_logits, 1.0);
//! ```

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

/// Speculative decoding configuration
#[derive(Debug, Clone)]
pub struct SpeculativeConfig {
    /// Maximum number of tokens to draft per iteration (typically 5-8)
    pub max_draft_tokens: usize,

    /// Maximum number of branches per tree node (typically 2-4)
    pub tree_width: usize,

    /// Minimum confidence threshold for accepting drafts (0.0-1.0)
    pub acceptance_threshold: f32,

    /// Use mincut λ-stability as confidence guidance
    pub use_lambda_guidance: bool,
}

impl Default for SpeculativeConfig {
    fn default() -> Self {
        Self {
            max_draft_tokens: 5,
            tree_width: 3,
            acceptance_threshold: 0.7,
            use_lambda_guidance: true,
        }
    }
}

/// Draft token with confidence score
#[derive(Debug, Clone)]
pub struct DraftToken {
    /// Token ID from vocabulary
    pub token_id: u32,

    /// Confidence score (0.0-1.0) from draft model
    pub confidence: f32,

    /// Index of parent token in tree (None for root)
    pub parent_idx: Option<usize>,

    /// Depth in the tree (0 for root)
    pub depth: usize,
}

/// Draft tree for speculative decoding
#[derive(Debug, Clone)]
pub struct DraftTree {
    /// All tokens in the tree (breadth-first order)
    pub tokens: Vec<DraftToken>,

    /// Valid paths through the tree (sequences of token indices)
    pub paths: Vec<Vec<usize>>,
}

impl DraftTree {
    /// Create an empty draft tree
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
            paths: Vec::new(),
        }
    }

    /// Get maximum depth of the tree
    pub fn max_depth(&self) -> usize {
        self.tokens.iter().map(|t| t.depth).max().unwrap_or(0)
    }

    /// Get all tokens at a specific depth
    pub fn tokens_at_depth(&self, depth: usize) -> Vec<usize> {
        self.tokens
            .iter()
            .enumerate()
            .filter(|(_, t)| t.depth == depth)
            .map(|(i, _)| i)
            .collect()
    }

    /// Build all valid paths through the tree
    fn build_paths(&mut self) {
        self.paths.clear();

        if self.tokens.is_empty() {
            return;
        }

        // Find all leaf nodes
        let leaf_indices: Vec<usize> = self
            .tokens
            .iter()
            .enumerate()
            .filter(|(idx, _)| {
                // A node is a leaf if no other node has it as parent
                !self.tokens.iter().any(|t| t.parent_idx == Some(*idx))
            })
            .map(|(idx, _)| idx)
            .collect();

        // Build path from each leaf to root
        for leaf_idx in leaf_indices {
            let mut path = Vec::new();
            let mut current_idx = Some(leaf_idx);

            while let Some(idx) = current_idx {
                path.push(idx);
                current_idx = self.tokens[idx].parent_idx;
            }

            // Reverse to get root-to-leaf order
            path.reverse();
            self.paths.push(path);
        }
    }
}

impl Default for DraftTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of draft verification
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Accepted token IDs
    pub accepted_tokens: Vec<u32>,

    /// Number of accepted tokens
    pub accepted_count: usize,

    /// Acceptance rate (accepted / total drafted)
    pub acceptance_rate: f32,
}

/// Speculative decoder using EAGLE-3 algorithm
pub struct SpeculativeDecoder {
    config: SpeculativeConfig,
}

impl SpeculativeDecoder {
    /// Create a new speculative decoder
    pub fn new(config: SpeculativeConfig) -> Self {
        Self { config }
    }

    /// Generate draft tree using λ-guided confidence
    ///
    /// # Arguments
    ///
    /// * `lambda` - Current mincut λ-stability value
    /// * `lambda_prev` - Previous λ-stability value
    /// * `draft_logits` - Draft model logits [draft_steps, vocab_size]
    ///
    /// # Returns
    ///
    /// Draft tree with tokens and valid paths
    pub fn generate_draft_tree(
        &self,
        lambda: u32,
        lambda_prev: u32,
        draft_logits: &[Vec<f32>],
    ) -> DraftTree {
        let mut tree = DraftTree::new();

        if draft_logits.is_empty() {
            return tree;
        }

        // Compute λ-based confidence scaling factor
        let lambda_confidence = if self.config.use_lambda_guidance {
            self.compute_lambda_confidence(lambda, lambda_prev)
        } else {
            1.0
        };

        // Generate root token (first draft step)
        let root_tokens = self.sample_top_k_tokens(
            &draft_logits[0],
            self.config.tree_width,
            lambda_confidence,
            None,
            0,
        );

        tree.tokens.extend(root_tokens);

        // Generate subsequent levels with branching
        for depth in 1..self.config.max_draft_tokens.min(draft_logits.len()) {
            let parent_indices = tree.tokens_at_depth(depth - 1);

            for parent_idx in parent_indices {
                let parent_confidence = tree.tokens[parent_idx].confidence;

                // Adjust tree width based on parent confidence
                let adaptive_width = self.compute_adaptive_width(parent_confidence);

                if adaptive_width > 0 {
                    let children = self.sample_top_k_tokens(
                        &draft_logits[depth],
                        adaptive_width,
                        lambda_confidence * parent_confidence,
                        Some(parent_idx),
                        depth,
                    );

                    tree.tokens.extend(children);
                }
            }
        }

        // Build all valid paths through the tree
        tree.build_paths();

        tree
    }

    /// Verify drafts against target model using rejection sampling
    ///
    /// # Arguments
    ///
    /// * `draft_tree` - Tree of draft tokens to verify
    /// * `target_logits` - Target model logits [steps, vocab_size]
    /// * `temperature` - Sampling temperature
    ///
    /// # Returns
    ///
    /// Verification result with accepted tokens
    pub fn verify_drafts(
        &self,
        draft_tree: &DraftTree,
        target_logits: &[Vec<f32>],
        temperature: f32,
    ) -> VerificationResult {
        let mut accepted_tokens = Vec::new();
        let total_paths = draft_tree.paths.len();

        if total_paths == 0 {
            return VerificationResult {
                accepted_tokens,
                accepted_count: 0,
                acceptance_rate: 0.0,
            };
        }

        // Find the best path through rejection sampling
        let mut best_path_idx = 0;
        let mut best_acceptance_score = 0.0;

        for (path_idx, path) in draft_tree.paths.iter().enumerate() {
            let mut path_score = 0.0;

            for (step, &token_idx) in path.iter().enumerate() {
                if step >= target_logits.len() {
                    break;
                }

                let draft_token = &draft_tree.tokens[token_idx];
                let target_probs = self.softmax_with_temperature(&target_logits[step], temperature);

                // Get draft and target probabilities
                let draft_prob = draft_token.confidence;
                let target_prob = target_probs
                    .get(draft_token.token_id as usize)
                    .copied()
                    .unwrap_or(0.0);

                // Compute acceptance probability using rejection sampling
                let accept_prob = Self::acceptance_probability(draft_prob, target_prob);

                if accept_prob >= self.config.acceptance_threshold {
                    path_score += accept_prob;
                } else {
                    // Rejection: stop this path
                    break;
                }
            }

            if path_score > best_acceptance_score {
                best_acceptance_score = path_score;
                best_path_idx = path_idx;
            }
        }

        // Extract accepted tokens from best path
        let best_path = &draft_tree.paths[best_path_idx];
        for (step, &token_idx) in best_path.iter().enumerate() {
            if step >= target_logits.len() {
                break;
            }

            let draft_token = &draft_tree.tokens[token_idx];
            let target_probs = self.softmax_with_temperature(&target_logits[step], temperature);

            let draft_prob = draft_token.confidence;
            let target_prob = target_probs
                .get(draft_token.token_id as usize)
                .copied()
                .unwrap_or(0.0);

            let accept_prob = Self::acceptance_probability(draft_prob, target_prob);

            if accept_prob >= self.config.acceptance_threshold {
                accepted_tokens.push(draft_token.token_id);
            } else {
                break;
            }
        }

        let accepted_count = accepted_tokens.len();
        let total_drafted = draft_tree.tokens.len();
        let acceptance_rate = if total_drafted > 0 {
            accepted_count as f32 / total_drafted as f32
        } else {
            0.0
        };

        VerificationResult {
            accepted_tokens,
            accepted_count,
            acceptance_rate,
        }
    }

    /// Compute acceptance probability using rejection sampling
    ///
    /// # Arguments
    ///
    /// * `draft_prob` - Probability from draft model
    /// * `target_prob` - Probability from target model
    ///
    /// # Returns
    ///
    /// Acceptance probability in [0, 1]
    fn acceptance_probability(draft_prob: f32, target_prob: f32) -> f32 {
        if draft_prob <= 0.0 {
            return 0.0;
        }

        // EAGLE-3 rejection sampling: min(1, target_prob / draft_prob)
        (target_prob / draft_prob).min(1.0)
    }

    /// Compute λ-based confidence scaling factor
    ///
    /// Higher λ-stability indicates more confident predictions
    fn compute_lambda_confidence(&self, lambda: u32, lambda_prev: u32) -> f32 {
        // Normalize to [0, 1] range (assuming λ <= 256)
        let lambda_norm = (lambda as f32 / 256.0).min(1.0);

        // Stability bonus: reward increasing λ
        let stability_bonus = if lambda >= lambda_prev {
            1.0 + 0.1 * ((lambda - lambda_prev) as f32 / 256.0)
        } else {
            1.0 - 0.1 * ((lambda_prev - lambda) as f32 / 256.0)
        };

        lambda_norm * stability_bonus
    }

    /// Compute adaptive tree width based on confidence
    fn compute_adaptive_width(&self, confidence: f32) -> usize {
        if confidence >= 0.9 {
            // High confidence: narrow tree but at least 1
            if self.config.tree_width == 1 {
                1 // Keep single path
            } else {
                (self.config.tree_width / 2).max(1)
            }
        } else if confidence >= 0.6 {
            // Medium confidence: normal width
            self.config.tree_width
        } else if confidence >= 0.3 {
            // Low confidence: wider tree
            (self.config.tree_width * 3 / 2).max(self.config.tree_width)
        } else {
            // Very low confidence: minimal branching
            (self.config.tree_width / 2).max(1)
        }
    }

    /// Sample top-k tokens from logits with confidence scaling
    fn sample_top_k_tokens(
        &self,
        logits: &[f32],
        k: usize,
        confidence_scale: f32,
        parent_idx: Option<usize>,
        depth: usize,
    ) -> Vec<DraftToken> {
        if logits.is_empty() || k == 0 {
            return Vec::new();
        }

        // Apply softmax to get probabilities
        let probs = self.softmax_with_temperature(logits, 1.0);

        // Get top-k indices with their probabilities
        let mut indexed_probs: Vec<(usize, f32)> =
            probs.iter().enumerate().map(|(i, &p)| (i, p)).collect();

        // Sort by probability (descending)
        indexed_probs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        // Take top-k and create draft tokens
        indexed_probs
            .into_iter()
            .take(k)
            .filter(|(_, prob)| *prob > 0.0) // Skip zero-probability tokens
            .map(|(token_id, prob)| DraftToken {
                token_id: token_id as u32, // Use original index as token_id
                confidence: prob * confidence_scale,
                parent_idx,
                depth,
            })
            .collect()
    }

    /// Apply softmax with temperature to logits
    fn softmax_with_temperature(&self, logits: &[f32], temperature: f32) -> Vec<f32> {
        if logits.is_empty() {
            return Vec::new();
        }

        let temperature = temperature.max(1e-6); // Avoid division by zero

        // Find max for numerical stability
        let max_logit = logits
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal))
            .unwrap_or(0.0);

        // Compute exp((logit - max) / temperature)
        let exps: Vec<f32> = logits
            .iter()
            .map(|&logit| ((logit - max_logit) / temperature).exp())
            .collect();

        let sum: f32 = exps.iter().sum();

        if sum <= 0.0 {
            // Fallback: uniform distribution
            vec![1.0 / logits.len() as f32; logits.len()]
        } else {
            exps.iter().map(|&exp| exp / sum).collect()
        }
    }
}

/// Generate tree attention mask for parallel verification
///
/// Creates a causal attention mask that allows each draft token
/// to attend to its ancestors in the tree.
///
/// # Arguments
///
/// * `tree` - Draft tree structure
/// * `seq_len` - Sequence length for attention mask
///
/// # Returns
///
/// Flattened boolean mask [seq_len, seq_len] where true = allowed attention
pub fn generate_tree_attention_mask(tree: &DraftTree, seq_len: usize) -> Vec<bool> {
    let mut mask = vec![false; seq_len * seq_len];

    if tree.tokens.is_empty() {
        return mask;
    }

    // For each path in the tree
    for path in &tree.paths {
        for (i, _) in path.iter().enumerate() {
            if i >= seq_len {
                break;
            }

            // Token can attend to all ancestors in its path
            for (j, _) in path.iter().enumerate() {
                if j >= seq_len || j > i {
                    break;
                }

                // Allow attention from position i to position j
                mask[i * seq_len + j] = true;
            }
        }
    }

    mask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_path_speculation() {
        let config = SpeculativeConfig {
            max_draft_tokens: 3,
            tree_width: 1, // Single path
            acceptance_threshold: 0.6,
            use_lambda_guidance: false,
        };

        let decoder = SpeculativeDecoder::new(config);

        // Create simple draft logits (3 steps, vocab size 10)
        let draft_logits = vec![
            vec![0.0, 1.0, 0.5, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0, 0.0],
            vec![0.5, 0.0, 1.0, 0.4, 0.3, 0.2, 0.1, 0.0, 0.0, 0.0],
            vec![0.3, 0.2, 0.1, 1.0, 0.5, 0.4, 0.2, 0.1, 0.0, 0.0],
        ];

        let tree = decoder.generate_draft_tree(100, 100, &draft_logits);

        // Should have 3 tokens (one per step)
        assert_eq!(tree.tokens.len(), 3);

        // Should have 1 path
        assert_eq!(tree.paths.len(), 1);
        assert_eq!(tree.paths[0].len(), 3);

        // Tokens should be highest logits (1, 2, 3)
        assert_eq!(tree.tokens[0].token_id, 1);
        assert_eq!(tree.tokens[1].token_id, 2);
        assert_eq!(tree.tokens[2].token_id, 3);
    }

    #[test]
    fn test_tree_speculation_with_branches() {
        let config = SpeculativeConfig {
            max_draft_tokens: 3,
            tree_width: 2, // Two branches
            acceptance_threshold: 0.6,
            use_lambda_guidance: false,
        };

        let decoder = SpeculativeDecoder::new(config);

        let draft_logits = vec![
            vec![1.0, 0.9, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.8, 0.7, 0.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.5, 0.4, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ];

        let tree = decoder.generate_draft_tree(100, 100, &draft_logits);

        // Should have root (2) + level 1 (2*2=4) + level 2 (4*2=8) = 14 tokens
        // But adaptive width may reduce this
        assert!(tree.tokens.len() >= 6); // At least root + 2 children + their children

        // Should have multiple paths
        assert!(tree.paths.len() >= 2);

        // Each path should start at root
        for path in &tree.paths {
            assert!(path.len() >= 1);
            assert_eq!(tree.tokens[path[0]].parent_idx, None);
        }
    }

    #[test]
    fn test_rejection_sampling_correctness() {
        // Test acceptance probability calculation

        // Case 1: draft_prob == target_prob -> accept_prob = 1.0
        let accept_prob = SpeculativeDecoder::acceptance_probability(0.5, 0.5);
        assert!((accept_prob - 1.0).abs() < 1e-6);

        // Case 2: target_prob > draft_prob -> accept_prob = 1.0
        let accept_prob = SpeculativeDecoder::acceptance_probability(0.3, 0.7);
        assert!((accept_prob - 1.0).abs() < 1e-6);

        // Case 3: target_prob < draft_prob -> accept_prob < 1.0
        let accept_prob = SpeculativeDecoder::acceptance_probability(0.7, 0.3);
        assert!((accept_prob - 0.428571).abs() < 1e-4);

        // Case 4: draft_prob = 0 -> accept_prob = 0
        let accept_prob = SpeculativeDecoder::acceptance_probability(0.0, 0.5);
        assert_eq!(accept_prob, 0.0);
    }

    #[test]
    fn test_lambda_guided_confidence_scaling() {
        let config = SpeculativeConfig {
            max_draft_tokens: 2,
            tree_width: 2,
            acceptance_threshold: 0.5,
            use_lambda_guidance: true,
        };

        let decoder = SpeculativeDecoder::new(config);

        let draft_logits = vec![vec![1.0, 0.8, 0.0, 0.0, 0.0], vec![0.9, 0.7, 0.0, 0.0, 0.0]];

        // High λ should give higher confidence
        let tree_high = decoder.generate_draft_tree(250, 240, &draft_logits);

        // Low λ should give lower confidence
        let tree_low = decoder.generate_draft_tree(50, 60, &draft_logits);

        // High λ tokens should have higher confidence
        let avg_conf_high: f32 = tree_high.tokens.iter().map(|t| t.confidence).sum::<f32>()
            / tree_high.tokens.len() as f32;

        let avg_conf_low: f32 = tree_low.tokens.iter().map(|t| t.confidence).sum::<f32>()
            / tree_low.tokens.len() as f32;

        assert!(avg_conf_high > avg_conf_low);
    }

    #[test]
    fn test_draft_verification() {
        let config = SpeculativeConfig {
            max_draft_tokens: 3,
            tree_width: 1,
            acceptance_threshold: 0.7,
            use_lambda_guidance: false,
        };

        let decoder = SpeculativeDecoder::new(config);

        // Draft logits
        let draft_logits = vec![
            vec![0.0, 1.0, 0.5, 0.0],
            vec![0.5, 0.0, 1.0, 0.0],
            vec![0.3, 0.2, 0.1, 1.0],
        ];

        let tree = decoder.generate_draft_tree(100, 100, &draft_logits);

        // Target logits (similar to draft -> should accept)
        let target_logits = vec![
            vec![0.0, 1.0, 0.6, 0.0],
            vec![0.4, 0.0, 1.0, 0.0],
            vec![0.2, 0.1, 0.0, 1.0],
        ];

        let result = decoder.verify_drafts(&tree, &target_logits, 1.0);

        // Should accept all tokens
        assert_eq!(result.accepted_count, 3);
        assert_eq!(result.accepted_tokens, vec![1, 2, 3]);
        assert!(result.acceptance_rate > 0.9);
    }

    #[test]
    fn test_tree_attention_mask() {
        let mut tree = DraftTree::new();

        // Build a simple tree:
        //     0
        //    / \
        //   1   2
        //   |
        //   3
        tree.tokens.push(DraftToken {
            token_id: 100,
            confidence: 0.9,
            parent_idx: None,
            depth: 0,
        });
        tree.tokens.push(DraftToken {
            token_id: 101,
            confidence: 0.8,
            parent_idx: Some(0),
            depth: 1,
        });
        tree.tokens.push(DraftToken {
            token_id: 102,
            confidence: 0.7,
            parent_idx: Some(0),
            depth: 1,
        });
        tree.tokens.push(DraftToken {
            token_id: 103,
            confidence: 0.6,
            parent_idx: Some(1),
            depth: 2,
        });

        tree.build_paths();

        let mask = generate_tree_attention_mask(&tree, 4);

        // Mask should be 4x4 = 16 elements
        assert_eq!(mask.len(), 16);

        // Check causal structure: each token can attend to ancestors
        // Path 1: 0 -> 1 -> 3
        // Path 2: 0 -> 2

        // Token 0 can only attend to itself
        assert!(mask[0 * 4 + 0]); // 0 -> 0

        // Token 1 can attend to 0 and 1
        assert!(mask[1 * 4 + 0]); // 1 -> 0
        assert!(mask[1 * 4 + 1]); // 1 -> 1
    }

    #[test]
    fn test_adaptive_tree_width() {
        let config = SpeculativeConfig {
            max_draft_tokens: 3,
            tree_width: 4,
            acceptance_threshold: 0.5,
            use_lambda_guidance: false,
        };

        let decoder = SpeculativeDecoder::new(config);

        // Test different confidence levels
        assert_eq!(decoder.compute_adaptive_width(0.95), 2); // High: narrow (tree_width / 2)
        assert_eq!(decoder.compute_adaptive_width(0.75), 4); // Medium: normal (tree_width)
        assert_eq!(decoder.compute_adaptive_width(0.55), 6); // Low: wider (tree_width * 3 / 2)
        assert_eq!(decoder.compute_adaptive_width(0.25), 2); // Very low: minimal (tree_width / 2)

        // Test single-path configuration
        let config_single = SpeculativeConfig {
            max_draft_tokens: 3,
            tree_width: 1,
            acceptance_threshold: 0.5,
            use_lambda_guidance: false,
        };
        let decoder_single = SpeculativeDecoder::new(config_single);
        assert_eq!(decoder_single.compute_adaptive_width(0.95), 1); // Always 1 for single path
        assert_eq!(decoder_single.compute_adaptive_width(0.25), 1); // Always at least 1
    }

    #[test]
    fn test_empty_inputs() {
        let config = SpeculativeConfig::default();
        let decoder = SpeculativeDecoder::new(config);

        // Empty draft logits
        let tree = decoder.generate_draft_tree(100, 100, &[]);
        assert_eq!(tree.tokens.len(), 0);
        assert_eq!(tree.paths.len(), 0);

        // Empty target logits
        let result = decoder.verify_drafts(&tree, &[], 1.0);
        assert_eq!(result.accepted_count, 0);
        assert_eq!(result.acceptance_rate, 0.0);
    }
}
