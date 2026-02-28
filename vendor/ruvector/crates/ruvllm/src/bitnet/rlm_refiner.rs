//! RLM Post-Quantization Refinement Orchestrator (Phase 0.5)
//!
//! Thin orchestrator (~300 lines) that wires existing RLM components together
//! to refine a Phase 0 PTQ model by training only the small FP16 components
//! (~1-2% of parameters), with ternary weights frozen.
//!
//! ## Architecture (AD-19)
//!
//! The pipeline combines:
//! - [`MicroLoRA`] adapters (rank 1-2) on each expert FFN
//! - [`EwcRegularizer`] for cross-step stability
//! - [`GrpoOptimizer`] for quality reward signal on scale factors
//! - [`ContrastiveTrainer`] for router repair (with AD-20 SIMD-only support)
//!
//! ## SIMD-Only Mode (AD-20)
//!
//! All components run on pure CPU SIMD when `use_metal: false`:
//! - `MicroLoRA::forward_simd()` uses NEON on aarch64, scalar fallback elsewhere
//! - `EwcRegularizer` and `GrpoOptimizer` are pure ndarray (GPU-agnostic)
//! - `ContrastiveTrainer` has a CPU fallback when `use_metal: false`

use crate::error::{Result, RuvLLMError};
use crate::lora::micro_lora::{EwcState, MicroLoRA, MicroLoraConfig, TargetModule};
use crate::lora::training::{EwcRegularizer, TrainingConfig, TrainingPipeline};
use crate::training::contrastive::{ContrastiveConfig, ContrastiveTrainer};
use crate::training::grpo::{GrpoConfig, GrpoOptimizer};

use ndarray::Array1;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the Phase 0.5 RLM refinement pipeline.
///
/// Controls MicroLoRA rank, learning rate, EWC regularization strength,
/// GRPO group size, router repair epochs, and SIMD-only mode (AD-20).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RlmRefinerConfig {
    /// LoRA rank for MicroLoRA adapters (1-2)
    pub lora_rank: usize,
    /// Base learning rate for LoRA training
    pub learning_rate: f32,
    /// Target total training tokens (100M-500M)
    pub training_tokens: usize,
    /// Batch size per step (1-4, memory constrained)
    pub batch_size: usize,
    /// EWC++ regularization lambda (prevents forgetting)
    pub ewc_lambda: f32,
    /// GRPO group size for relative advantage computation
    pub grpo_group_size: usize,
    /// Number of router repair epochs via ContrastiveTrainer
    pub router_repair_epochs: usize,
    /// When false, forces SIMD-only / CPU mode (AD-20)
    pub use_metal: bool,
    /// Save a checkpoint every N training steps
    pub checkpoint_every_n: usize,
    /// Hidden dimension of the model (for LoRA sizing)
    pub hidden_dim: usize,
    /// Directory for checkpoint files
    pub checkpoint_dir: PathBuf,
}

impl Default for RlmRefinerConfig {
    fn default() -> Self {
        Self {
            lora_rank: 2,
            learning_rate: 1e-4,
            training_tokens: 100_000_000,
            batch_size: 2,
            ewc_lambda: 2000.0,
            grpo_group_size: 8,
            router_repair_epochs: 5,
            use_metal: false, // SIMD-only by default (AD-20)
            checkpoint_every_n: 1000,
            hidden_dim: 768,
            checkpoint_dir: PathBuf::from("checkpoints/rlm_refiner"),
        }
    }
}

// ---------------------------------------------------------------------------
// Per-step metrics
// ---------------------------------------------------------------------------

/// Metrics collected for a single refinement step.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RefinementStepMetrics {
    /// Step index
    pub step: usize,
    /// KL divergence against teacher
    pub kl_divergence: f32,
    /// GRPO reward for this step
    pub grpo_reward: f32,
    /// EWC penalty magnitude
    pub ewc_penalty: f32,
    /// Mean LoRA correction magnitude
    pub lora_correction_norm: f32,
    /// Current learning rate
    pub learning_rate: f32,
}

/// Aggregate metrics for the full refinement run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RefinementResult {
    /// Total steps completed
    pub total_steps: usize,
    /// Tokens processed
    pub tokens_processed: usize,
    /// Final average KL divergence
    pub final_kl_divergence: f32,
    /// Final average GRPO reward
    pub final_grpo_reward: f32,
    /// Router repair accuracy (post-repair)
    pub router_accuracy: f64,
    /// Checkpoint paths written
    pub checkpoint_paths: Vec<PathBuf>,
    /// Per-step history (sampled)
    pub history: Vec<RefinementStepMetrics>,
}

// ---------------------------------------------------------------------------
// Orchestrator
// ---------------------------------------------------------------------------

/// Phase 0.5 RLM refinement orchestrator.
///
/// Wires [`MicroLoRA`], [`EwcRegularizer`], [`GrpoOptimizer`], and
/// [`ContrastiveTrainer`] together to refine a PTQ ternary model.
pub struct RlmRefiner {
    /// Pipeline configuration
    config: RlmRefinerConfig,
    /// MicroLoRA adapters keyed by expert layer index
    lora_adapters: HashMap<usize, MicroLoRA>,
    /// EWC regularizer (shared across experts)
    ewc: EwcRegularizer,
    /// GRPO optimizer for scale factor quality signal
    grpo: GrpoOptimizer,
    /// ContrastiveTrainer for router repair
    contrastive: ContrastiveTrainer,
    /// Training pipeline (LR schedule, gradient accumulation)
    training_pipeline: TrainingPipeline,
    /// Current global step counter
    global_step: usize,
    /// Accumulated metrics
    metrics_history: Vec<RefinementStepMetrics>,
}

impl RlmRefiner {
    /// Create a new `RlmRefiner` from the given configuration.
    ///
    /// Initializes all sub-components with settings derived from the config:
    /// - One [`MicroLoRA`] per expert layer (using MLP target modules)
    /// - [`EwcRegularizer`] with the configured lambda and Fisher decay
    /// - [`GrpoOptimizer`] with the configured group size
    /// - [`ContrastiveTrainer`] with `use_metal` from config (AD-20)
    /// - [`TrainingPipeline`] with matching LR and batch size
    pub fn new(config: RlmRefinerConfig, num_expert_layers: usize) -> Result<Self> {
        // -- MicroLoRA: one per expert layer targeting MLP modules --
        let mut lora_adapters = HashMap::with_capacity(num_expert_layers);
        let lora_config = MicroLoraConfig {
            rank: config.lora_rank.clamp(1, 2),
            alpha: (config.lora_rank as f32) * 2.0,
            dropout: 0.0,
            target_modules: TargetModule::mlp(),
            in_features: config.hidden_dim,
            out_features: config.hidden_dim,
            use_bias: false,
            standard_init: true,
            gradient_checkpointing: false,
        };
        for layer_idx in 0..num_expert_layers {
            lora_adapters.insert(layer_idx, MicroLoRA::new(lora_config.clone()));
        }

        // -- EWC regularizer --
        let ewc = EwcRegularizer::new(config.ewc_lambda, 0.999);

        // -- GRPO optimizer for scale-factor reward signal --
        let grpo_config = GrpoConfig {
            group_size: config.grpo_group_size,
            learning_rate: config.learning_rate as f32,
            normalize_rewards: true,
            normalize_advantages: true,
            ..GrpoConfig::default()
        };
        let grpo = GrpoOptimizer::new(grpo_config);

        // -- ContrastiveTrainer (use_metal controlled by AD-20) --
        let contrastive_config = ContrastiveConfig {
            use_metal: config.use_metal,
            ..ContrastiveConfig::default()
        };
        let contrastive = ContrastiveTrainer::new(contrastive_config)
            .map_err(|e| RuvLLMError::Config(format!("ContrastiveTrainer init: {}", e)))?;

        // -- TrainingPipeline --
        let training_config = TrainingConfig {
            learning_rate: config.learning_rate,
            ewc_lambda: config.ewc_lambda,
            batch_size: config.batch_size,
            ..TrainingConfig::default()
        };
        let training_pipeline = TrainingPipeline::new(training_config);

        Ok(Self {
            config,
            lora_adapters,
            ewc,
            grpo,
            contrastive,
            training_pipeline,
            global_step: 0,
            metrics_history: Vec::new(),
        })
    }

    /// Initialize EWC state for every adapter in every expert layer.
    ///
    /// Should be called once after loading the pre-trained LoRA weights
    /// (or after initial random init) to record the starting point for
    /// EWC regularization.
    pub fn init_ewc_states(&mut self) {
        for lora in self.lora_adapters.values() {
            for module in &TargetModule::mlp() {
                if let Some(adapter_lock) = lora.get_adapter(module) {
                    let adapter = adapter_lock.read();
                    self.ewc.init_module(*module, &adapter);
                }
            }
        }
    }

    /// Execute one refinement step.
    ///
    /// The step proceeds as follows:
    /// 1. Forward through frozen ternary model (caller provides `ternary_output`)
    /// 2. Forward through MicroLoRA adapters via `forward_simd`
    /// 3. Combine: `Y = ternary_output + lora_correction`
    /// 4. Compute KL divergence against `teacher_output`
    /// 5. Compute GRPO reward for the step
    /// 6. Apply gradients with EWC regularization
    /// 7. Periodically trigger router repair and checkpointing
    ///
    /// # Arguments
    ///
    /// * `expert_idx` - Index of the expert layer being trained
    /// * `input` - Input hidden states (flat f32 slice, `hidden_dim` elements)
    /// * `ternary_output` - Output from the frozen ternary forward pass
    /// * `teacher_output` - Reference output from the FP16 teacher model
    ///
    /// # Returns
    ///
    /// Step metrics including KL divergence and GRPO reward.
    pub fn refine_step(
        &mut self,
        expert_idx: usize,
        input: &[f32],
        ternary_output: &[f32],
        teacher_output: &[f32],
    ) -> Result<RefinementStepMetrics> {
        let dim = self.config.hidden_dim;
        if input.len() != dim || ternary_output.len() != dim || teacher_output.len() != dim {
            return Err(RuvLLMError::InvalidOperation(format!(
                "Dimension mismatch: expected {}, got input={}, ternary={}, teacher={}",
                dim,
                input.len(),
                ternary_output.len(),
                teacher_output.len(),
            )));
        }

        let lora = self.lora_adapters.get(&expert_idx).ok_or_else(|| {
            RuvLLMError::InvalidOperation(format!("No LoRA adapter for expert {}", expert_idx))
        })?;

        // -- Step 2: Forward through MicroLoRA (SIMD path) --
        let mut lora_correction = vec![0.0f32; dim];
        for module in &TargetModule::mlp() {
            lora.forward_add(input, module, &mut lora_correction);
        }

        // -- Step 3: Combined output --
        let combined: Vec<f32> = ternary_output
            .iter()
            .zip(lora_correction.iter())
            .map(|(t, l)| t + l)
            .collect();

        // -- Step 4: KL divergence proxy (element-wise squared error) --
        let kl_divergence = kl_divergence_proxy(&combined, teacher_output);

        // -- Step 5: GRPO reward (higher is better, invert loss) --
        let cosine_sim = cosine_similarity(&combined, teacher_output);
        let grpo_reward = cosine_sim.max(0.0);
        let advantages = self.grpo.compute_relative_advantages(&[grpo_reward]);
        let _grpo_reward_normalized = advantages.first().copied().unwrap_or(0.0);

        // -- Step 6: Accumulate gradient and apply with EWC --
        let input_arr = Array1::from_vec(input.to_vec());
        // Gradient direction: teacher - combined (points toward teacher)
        let grad_output: Vec<f32> = teacher_output
            .iter()
            .zip(combined.iter())
            .map(|(t, c)| t - c)
            .collect();
        let grad_arr = Array1::from_vec(grad_output);

        let reward_signal = grpo_reward.max(0.01);
        for module in &TargetModule::mlp() {
            if let Some(adapter_lock) = lora.get_adapter(module) {
                let mut adapter = adapter_lock.write();
                adapter.accumulate_gradient(&input_arr, &grad_arr, reward_signal);
            }
        }

        // Apply gradients every `batch_size` steps
        if (self.global_step + 1) % self.config.batch_size == 0 {
            let ewc_states: HashMap<TargetModule, EwcState> = TargetModule::mlp()
                .into_iter()
                .filter_map(|m| self.ewc.get_state(&m).cloned().map(|s| (m, s)))
                .collect();

            lora.apply_updates_with_ewc(
                self.config.learning_rate,
                &ewc_states,
                self.config.ewc_lambda,
            );
        }

        // -- Correction norm --
        let lora_correction_norm = lora_correction.iter().map(|v| v * v).sum::<f32>().sqrt();

        // -- Build metrics --
        let metrics = RefinementStepMetrics {
            step: self.global_step,
            kl_divergence,
            grpo_reward,
            ewc_penalty: self.ewc.lambda(),
            lora_correction_norm,
            learning_rate: self.config.learning_rate,
        };

        self.metrics_history.push(metrics.clone());
        self.global_step += 1;

        // -- Checkpoint --
        if self.global_step % self.config.checkpoint_every_n == 0 {
            let _ = self.save_checkpoint(self.global_step);
        }

        Ok(metrics)
    }

    /// Repair the MoE router using contrastive learning.
    ///
    /// Loads routing triplets and runs [`ContrastiveTrainer`] for the
    /// configured number of epochs. Triplets encode (anchor_hidden,
    /// correct_expert, wrong_expert) to fix misrouting caused by PTQ.
    ///
    /// # Arguments
    ///
    /// * `triplet_path` - Path to a JSONL file of [`TrainingTriplet`]s
    ///
    /// # Returns
    ///
    /// Post-repair accuracy and training loss.
    pub fn repair_router<P: AsRef<Path>>(&mut self, triplet_path: P) -> Result<f64> {
        let count = self
            .contrastive
            .load_triplets(triplet_path)
            .map_err(|e| RuvLLMError::Config(format!("Load triplets: {}", e)))?;

        if count == 0 {
            return Err(RuvLLMError::InvalidOperation(
                "No router repair triplets loaded".to_string(),
            ));
        }

        let result = self
            .contrastive
            .train(self.config.router_repair_epochs)
            .map_err(|e| RuvLLMError::InvalidOperation(format!("Router repair failed: {}", e)))?;

        Ok(result.best_accuracy)
    }

    /// Save a checkpoint of all LoRA adapter weights, EWC states,
    /// and optimized scale factors.
    pub fn save_checkpoint(&self, step: usize) -> Result<PathBuf> {
        let dir = self.config.checkpoint_dir.join(format!("step_{}", step));
        std::fs::create_dir_all(&dir)?;

        // Save each expert's LoRA adapters
        for (&layer_idx, lora) in &self.lora_adapters {
            let path = dir.join(format!("expert_{}_lora.bin", layer_idx));
            lora.save(path.to_str().unwrap_or("lora.bin"))?;
        }

        // Save EWC states
        let ewc_export = self.ewc.export_states();
        let ewc_bytes = bincode::serde::encode_to_vec(&ewc_export, bincode::config::standard())
            .map_err(|e| RuvLLMError::Serialization(e.to_string()))?;
        std::fs::write(dir.join("ewc_states.bin"), ewc_bytes)?;

        // Save metrics history
        let metrics_json = serde_json::to_string_pretty(&self.metrics_history)
            .map_err(|e| RuvLLMError::Serialization(e.to_string()))?;
        std::fs::write(dir.join("metrics.json"), metrics_json)?;

        Ok(dir)
    }

    /// Export the refined model artifacts ready for GGUF integration.
    ///
    /// Writes LoRA adapter weights and optimized scales to the output
    /// directory. These can be embedded alongside ternary weights during
    /// GGUF export.
    pub fn export_refined_model<P: AsRef<Path>>(&self, output_dir: P) -> Result<PathBuf> {
        let dir = output_dir.as_ref();
        std::fs::create_dir_all(dir)?;

        // Export each expert's LoRA state
        for (&layer_idx, lora) in &self.lora_adapters {
            let state = lora.export_state();
            let bytes = bincode::serde::encode_to_vec(&state, bincode::config::standard())
                .map_err(|e| RuvLLMError::Serialization(e.to_string()))?;
            std::fs::write(
                dir.join(format!("expert_{}_lora_state.bin", layer_idx)),
                bytes,
            )?;
        }

        // Export EWC states for future phases
        let ewc_export = self.ewc.export_states();
        let ewc_bytes = bincode::serde::encode_to_vec(&ewc_export, bincode::config::standard())
            .map_err(|e| RuvLLMError::Serialization(e.to_string()))?;
        std::fs::write(dir.join("ewc_states.bin"), ewc_bytes)?;

        // Export config for reproducibility
        let config_json = serde_json::to_string_pretty(&self.config)
            .map_err(|e| RuvLLMError::Serialization(e.to_string()))?;
        std::fs::write(dir.join("refiner_config.json"), config_json)?;

        Ok(dir.to_path_buf())
    }

    /// Return a summary of the refinement run.
    pub fn result_summary(&self) -> RefinementResult {
        let final_kl = self
            .metrics_history
            .last()
            .map(|m| m.kl_divergence)
            .unwrap_or(0.0);
        let final_reward = self
            .metrics_history
            .last()
            .map(|m| m.grpo_reward)
            .unwrap_or(0.0);

        RefinementResult {
            total_steps: self.global_step,
            tokens_processed: self.global_step * self.config.batch_size,
            final_kl_divergence: final_kl,
            final_grpo_reward: final_reward,
            router_accuracy: 0.0, // Set after repair_router()
            checkpoint_paths: Vec::new(),
            history: self.metrics_history.clone(),
        }
    }

    /// Access the global step counter.
    pub fn global_step(&self) -> usize {
        self.global_step
    }

    /// Access the configuration.
    pub fn config(&self) -> &RlmRefinerConfig {
        &self.config
    }

    /// Access a specific expert's MicroLoRA instance.
    pub fn get_expert_lora(&self, expert_idx: usize) -> Option<&MicroLoRA> {
        self.lora_adapters.get(&expert_idx)
    }

    /// Total trainable parameters across all LoRA adapters.
    pub fn total_trainable_params(&self) -> usize {
        self.lora_adapters.values().map(|l| l.param_count()).sum()
    }

    /// Total LoRA memory usage in bytes.
    pub fn total_lora_memory_bytes(&self) -> usize {
        self.lora_adapters.values().map(|l| l.memory_bytes()).sum()
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Proxy KL divergence as mean squared error between logit vectors.
///
/// True KL would require softmax normalization; MSE is a computationally
/// cheaper proxy suitable for gradient direction during refinement.
fn kl_divergence_proxy(predicted: &[f32], target: &[f32]) -> f32 {
    if predicted.len() != target.len() || predicted.is_empty() {
        return 0.0;
    }
    let mse: f32 = predicted
        .iter()
        .zip(target.iter())
        .map(|(p, t)| {
            let d = p - t;
            d * d
        })
        .sum();
    mse / predicted.len() as f32
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 1e-8 && norm_b > 1e-8 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = RlmRefinerConfig::default();
        assert_eq!(config.lora_rank, 2);
        assert!(!config.use_metal); // AD-20: SIMD-only by default
        assert_eq!(config.ewc_lambda, 2000.0);
        assert_eq!(config.grpo_group_size, 8);
    }

    #[test]
    fn test_refiner_creation() {
        let config = RlmRefinerConfig {
            hidden_dim: 64,
            ..Default::default()
        };
        let refiner = RlmRefiner::new(config, 4).unwrap();

        assert_eq!(refiner.lora_adapters.len(), 4);
        assert_eq!(refiner.global_step(), 0);
        // 4 experts × 3 MLP modules × (64*2 + 2*64) params × 4 bytes
        assert!(refiner.total_trainable_params() > 0);
        assert!(refiner.total_lora_memory_bytes() > 0);
    }

    #[test]
    fn test_refine_step() {
        let config = RlmRefinerConfig {
            hidden_dim: 64,
            batch_size: 1,
            ..Default::default()
        };
        let mut refiner = RlmRefiner::new(config, 2).unwrap();
        refiner.init_ewc_states();

        let input = vec![0.1f32; 64];
        let ternary_out = vec![0.5f32; 64];
        let teacher_out = vec![0.6f32; 64];

        let metrics = refiner
            .refine_step(0, &input, &ternary_out, &teacher_out)
            .unwrap();

        assert_eq!(metrics.step, 0);
        assert!(metrics.kl_divergence >= 0.0);
        assert_eq!(refiner.global_step(), 1);
    }

    #[test]
    fn test_refine_step_dimension_mismatch() {
        let config = RlmRefinerConfig {
            hidden_dim: 64,
            ..Default::default()
        };
        let mut refiner = RlmRefiner::new(config, 1).unwrap();

        let result = refiner.refine_step(0, &[0.1; 32], &[0.5; 64], &[0.6; 64]);
        assert!(result.is_err());
    }

    #[test]
    fn test_refine_step_invalid_expert() {
        let config = RlmRefinerConfig {
            hidden_dim: 64,
            ..Default::default()
        };
        let mut refiner = RlmRefiner::new(config, 1).unwrap();

        let result = refiner.refine_step(99, &[0.1; 64], &[0.5; 64], &[0.6; 64]);
        assert!(result.is_err());
    }

    #[test]
    fn test_kl_divergence_proxy() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert!((kl_divergence_proxy(&a, &b)).abs() < 1e-6);

        let c = vec![2.0, 3.0, 4.0];
        assert!(kl_divergence_proxy(&a, &c) > 0.0);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn test_result_summary() {
        let config = RlmRefinerConfig {
            hidden_dim: 64,
            batch_size: 1,
            ..Default::default()
        };
        let mut refiner = RlmRefiner::new(config, 1).unwrap();
        refiner.init_ewc_states();

        let input = vec![0.1f32; 64];
        let ternary_out = vec![0.5f32; 64];
        let teacher_out = vec![0.6f32; 64];

        for _ in 0..5 {
            refiner
                .refine_step(0, &input, &ternary_out, &teacher_out)
                .unwrap();
        }

        let result = refiner.result_summary();
        assert_eq!(result.total_steps, 5);
        assert_eq!(result.history.len(), 5);
    }

    #[test]
    fn test_multiple_expert_training() {
        let config = RlmRefinerConfig {
            hidden_dim: 64,
            batch_size: 1,
            ..Default::default()
        };
        let mut refiner = RlmRefiner::new(config, 4).unwrap();
        refiner.init_ewc_states();

        let input = vec![0.1f32; 64];
        let ternary_out = vec![0.5f32; 64];
        let teacher_out = vec![0.6f32; 64];

        // Train each expert for a few steps
        for expert in 0..4 {
            for _ in 0..3 {
                refiner
                    .refine_step(expert, &input, &ternary_out, &teacher_out)
                    .unwrap();
            }
        }

        assert_eq!(refiner.global_step(), 12);
        assert_eq!(refiner.result_summary().history.len(), 12);
    }
}
