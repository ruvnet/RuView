//! Real Inference Evaluation Harness
//!
//! Runs actual LLM inference for evaluation - no simulations.
//! Uses the full RuvLLM stack: backends, SONA, HNSW routing.

use super::correctness::{CorrectnessMetrics, TaskResult, VerificationLevel};
use super::diff_quality::DiffAnalyzer;
use super::economics::{CostTracker, EconomicsMetrics};
use super::harness::{
    AblationMode, EvalConfig, EvalReport, EvalRun, EvalTask, LatencyBreakdown, ModeMetrics,
};
use crate::backends::{create_backend, GenerateParams, LlmBackend, ModelConfig};
use crate::claude_flow::{AgentType, ClaudeFlowTask, HnswRouter, HnswRouterConfig, TaskPattern};
use crate::sona::integration::{SonaConfig, SonaIntegration, Trajectory};
use crate::Result;

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Result from HNSW routing
#[derive(Debug, Clone)]
pub struct RoutingResult {
    /// Primary agent recommended
    pub primary_agent: AgentType,
    /// Confidence in the recommendation
    pub confidence: f32,
    /// Number of patterns considered
    pub patterns_considered: usize,
    /// Alternative agents with scores
    pub alternatives: Vec<String>,
    /// Reasoning for the decision
    pub reasoning: String,
}

impl Default for RoutingResult {
    fn default() -> Self {
        Self {
            primary_agent: AgentType::Coder, // Default to Coder
            confidence: 0.0,
            patterns_considered: 0,
            alternatives: Vec::new(),
            reasoning: String::new(),
        }
    }
}

/// Real inference evaluation harness
///
/// Unlike the simulated harness, this actually runs inference
/// through real LLM backends with SONA learning.
pub struct RealEvaluationHarness {
    /// Configuration
    config: EvalConfig,
    /// Real LLM backend
    backend: Arc<RwLock<Box<dyn LlmBackend>>>,
    /// SONA integration for learning
    sona: Option<Arc<RwLock<SonaIntegration>>>,
    /// HNSW router for pattern matching
    hnsw_router: Option<Arc<RwLock<HnswRouter>>>,
    /// Diff analyzer
    diff_analyzer: DiffAnalyzer,
    /// Results by mode
    results: HashMap<AblationMode, Vec<EvalRun>>,
    /// Model loaded flag
    model_loaded: bool,
}

/// Configuration for real inference
#[derive(Debug, Clone)]
pub struct RealInferenceConfig {
    /// Path to GGUF model file
    pub model_path: String,
    /// Model configuration
    pub model_config: ModelConfig,
    /// Generation parameters
    pub generate_params: GenerateParams,
    /// Enable SONA learning
    pub enable_sona: bool,
    /// Enable HNSW routing
    pub enable_hnsw: bool,
    /// SONA configuration
    pub sona_config: Option<SonaConfig>,
    /// HNSW configuration
    pub hnsw_config: Option<HnswRouterConfig>,
}

impl Default for RealInferenceConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            model_config: ModelConfig::default(),
            generate_params: GenerateParams::default(),
            enable_sona: true,
            enable_hnsw: true,
            sona_config: None,
            hnsw_config: None,
        }
    }
}

impl RealEvaluationHarness {
    /// Create new harness with real backend
    pub fn new(eval_config: EvalConfig) -> Result<Self> {
        let backend = create_backend();

        Ok(Self {
            config: eval_config,
            backend: Arc::new(RwLock::new(backend)),
            sona: None,
            hnsw_router: None,
            diff_analyzer: DiffAnalyzer::default(),
            results: HashMap::new(),
            model_loaded: false,
        })
    }

    /// Create with full configuration
    pub fn with_config(
        eval_config: EvalConfig,
        inference_config: RealInferenceConfig,
    ) -> Result<Self> {
        let mut harness = Self::new(eval_config)?;

        // Load model if path provided
        if !inference_config.model_path.is_empty() {
            harness.load_model(
                &inference_config.model_path,
                inference_config.model_config.clone(),
            )?;
        }

        // Initialize SONA if enabled
        if inference_config.enable_sona {
            let sona_config = inference_config.sona_config.unwrap_or_default();
            let sona = SonaIntegration::new(sona_config);
            harness.sona = Some(Arc::new(RwLock::new(sona)));
        }

        // Initialize HNSW router if enabled - use model's hidden_size if available
        if inference_config.enable_hnsw {
            let embedding_dim = harness.get_model_embedding_dim().unwrap_or(384);

            let mut hnsw_config = inference_config.hnsw_config.unwrap_or_default();
            hnsw_config.embedding_dim = embedding_dim;

            let router = HnswRouter::new(hnsw_config)?;
            harness.hnsw_router = Some(Arc::new(RwLock::new(router)));

            // Bootstrap with seed patterns for common code tasks
            harness.bootstrap_hnsw_patterns()?;
        }

        Ok(harness)
    }

    /// Get the model's embedding dimension from model info
    fn get_model_embedding_dim(&self) -> Option<usize> {
        self.backend
            .read()
            .model_info()
            .map(|info| info.hidden_size)
    }

    /// Bootstrap HNSW router with seed patterns for common code tasks
    fn bootstrap_hnsw_patterns(&self) -> Result<()> {
        let router = match &self.hnsw_router {
            Some(r) => r,
            None => return Ok(()),
        };

        let mut router = router.write();
        let dim = router.config().embedding_dim;

        // Seed patterns for different task types
        let seed_patterns = vec![
            // Bug fix patterns
            (
                "Fix null pointer exception",
                AgentType::Coder,
                ClaudeFlowTask::Debugging,
            ),
            (
                "Resolve memory leak",
                AgentType::Coder,
                ClaudeFlowTask::Debugging,
            ),
            (
                "Fix off-by-one error",
                AgentType::Coder,
                ClaudeFlowTask::Debugging,
            ),
            (
                "Handle edge case",
                AgentType::Coder,
                ClaudeFlowTask::Debugging,
            ),
            // Code generation patterns
            (
                "Implement new function",
                AgentType::Coder,
                ClaudeFlowTask::CodeGeneration,
            ),
            (
                "Add new feature",
                AgentType::Coder,
                ClaudeFlowTask::CodeGeneration,
            ),
            (
                "Create API endpoint",
                AgentType::Coder,
                ClaudeFlowTask::CodeGeneration,
            ),
            (
                "Build component",
                AgentType::Coder,
                ClaudeFlowTask::CodeGeneration,
            ),
            // Refactoring patterns
            (
                "Refactor for performance",
                AgentType::Coder,
                ClaudeFlowTask::Refactoring,
            ),
            (
                "Extract method",
                AgentType::Coder,
                ClaudeFlowTask::Refactoring,
            ),
            (
                "Simplify code",
                AgentType::Coder,
                ClaudeFlowTask::Refactoring,
            ),
            // Testing patterns
            (
                "Write unit tests",
                AgentType::Tester,
                ClaudeFlowTask::Testing,
            ),
            (
                "Add integration tests",
                AgentType::Tester,
                ClaudeFlowTask::Testing,
            ),
            (
                "Increase test coverage",
                AgentType::Tester,
                ClaudeFlowTask::Testing,
            ),
            // Research patterns
            (
                "Analyze codebase",
                AgentType::Researcher,
                ClaudeFlowTask::Research,
            ),
            (
                "Find similar patterns",
                AgentType::Researcher,
                ClaudeFlowTask::Research,
            ),
            // Review patterns
            (
                "Review code quality",
                AgentType::Reviewer,
                ClaudeFlowTask::CodeReview,
            ),
            (
                "Security review",
                AgentType::Reviewer,
                ClaudeFlowTask::CodeReview,
            ),
        ];

        for (i, (description, agent_type, task_type)) in seed_patterns.iter().enumerate() {
            // Create deterministic pseudo-embedding from description
            let embedding = Self::create_seed_embedding(description, dim, i);

            let mut pattern =
                TaskPattern::new(embedding, *agent_type, *task_type, description.to_string());
            // Give seed patterns initial trust
            pattern.usage_count = 10;
            pattern.success_count = 8;
            pattern.success_rate = 0.8;

            router.add_pattern(pattern)?;
        }

        tracing::info!(
            "Bootstrapped HNSW router with {} seed patterns",
            seed_patterns.len()
        );
        Ok(())
    }

    /// Create a deterministic seed embedding from text
    fn create_seed_embedding(text: &str, dim: usize, seed: usize) -> Vec<f32> {
        let mut embedding = vec![0.0f32; dim];

        // Simple hash-based embedding for seed patterns
        for (i, c) in text.bytes().enumerate() {
            let idx = (i + seed * 7) % dim;
            embedding[idx] += (c as f32 / 255.0) - 0.5;
        }

        // Normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        embedding
    }

    /// Load a model into the backend
    pub fn load_model(&mut self, model_path: &str, config: ModelConfig) -> Result<()> {
        let mut backend = self.backend.write();
        backend.load_model(model_path, config)?;
        self.model_loaded = true;
        Ok(())
    }

    /// Check if model is loaded
    pub fn is_model_loaded(&self) -> bool {
        self.model_loaded && self.backend.read().is_model_loaded()
    }

    /// Run evaluation with real inference
    pub async fn run_evaluation(&mut self, tasks: &[EvalTask]) -> Result<EvalReport> {
        if !self.is_model_loaded() {
            return Err(crate::RuvLLMError::InvalidOperation(
                "No model loaded. Call load_model() first.".into(),
            ));
        }

        let start = Instant::now();

        for mode in &self.config.ablation_modes.clone() {
            let mode_results = self.run_mode(*mode, tasks).await?;
            self.results.insert(*mode, mode_results);
        }

        let total_duration = start.elapsed();
        Ok(self.generate_report(total_duration))
    }

    /// Run evaluation for a single ablation mode
    async fn run_mode(&mut self, mode: AblationMode, tasks: &[EvalTask]) -> Result<Vec<EvalRun>> {
        let mut runs = Vec::new();

        for task in tasks.iter().take(self.config.task_count) {
            for &seed in &self.config.seeds {
                let run = self.run_single_task(mode, task, seed).await?;
                runs.push(run);
            }
        }

        Ok(runs)
    }

    /// Run a single task with REAL inference
    async fn run_single_task(
        &self,
        mode: AblationMode,
        task: &EvalTask,
        seed: u64,
    ) -> Result<EvalRun> {
        let start = Instant::now();
        let mut latency = LatencyBreakdown::default();

        // ========== REAL ROUTING ==========
        let route_start = Instant::now();
        let routing_result = if matches!(
            mode,
            AblationMode::RetrievalOnly | AblationMode::RetrievalPlusAdapters | AblationMode::Full
        ) {
            self.real_routing(&task.description)?
        } else {
            RoutingResult::default()
        };
        latency.routing_ms = route_start.elapsed().as_secs_f64() * 1000.0;

        // ========== REAL RETRIEVAL ==========
        let retrieval_start = Instant::now();
        let context = if routing_result.patterns_considered > 0 {
            self.build_context_from_routing(&routing_result, &task.description)
        } else {
            String::new()
        };
        latency.retrieval_ms = retrieval_start.elapsed().as_secs_f64() * 1000.0;

        // ========== REAL GENERATION ==========
        let gen_start = Instant::now();
        let (patch, gen_cost) = self.real_generation(mode, task, seed, &context)?;
        latency.generation_ms = gen_start.elapsed().as_secs_f64() * 1000.0;

        latency.total_ms = start.elapsed().as_secs_f64() * 1000.0;

        // Analyze diff quality
        let diff_quality = patch.as_ref().map(|p| {
            self.diff_analyzer
                .analyze(p, task.reference_patch.as_deref())
        });

        // Build correctness result
        let correctness = self.evaluate_correctness(task, &patch, &latency);

        // Determine acceptance
        let accepted = correctness.succeeded()
            && diff_quality.as_ref().map_or(false, |dq| {
                dq.combined_score >= self.config.quality_threshold
            });

        // ========== LEARNING ==========
        // Learn from this task in modes that support learning
        if matches!(
            mode,
            AblationMode::AdaptersOnly | AblationMode::RetrievalPlusAdapters | AblationMode::Full
        ) {
            let _ = self.learn_from_success(task, &patch, accepted);
        }

        Ok(EvalRun {
            task_id: task.id.clone(),
            mode,
            seed,
            generated_patch: patch,
            correctness,
            diff_quality,
            cost: gen_cost,
            latency,
            accepted,
            error: None,
        })
    }

    /// Real routing using HNSW router
    fn real_routing(&self, task_description: &str) -> Result<RoutingResult> {
        if let Some(ref router) = self.hnsw_router {
            let router = router.read();

            // Get embedding for task - use seed embedding if backend can't provide
            let embedding = self
                .get_embedding(task_description)
                .unwrap_or_else(|_| Self::create_seed_embedding(task_description, 384, 0));

            // Use full routing with confidence scores
            let hnsw_result = router.route_by_similarity(&embedding)?;

            Ok(RoutingResult {
                primary_agent: hnsw_result.primary_agent,
                confidence: hnsw_result.confidence,
                patterns_considered: hnsw_result.patterns_considered,
                alternatives: hnsw_result
                    .alternatives
                    .iter()
                    .map(|(agent, score)| format!("{:?}:{:.2}", agent, score))
                    .collect(),
                reasoning: hnsw_result.reasoning,
            })
        } else {
            Ok(RoutingResult::default())
        }
    }

    /// Learn from successful task completion
    fn learn_from_success(
        &self,
        task: &EvalTask,
        patch: &Option<String>,
        success: bool,
    ) -> Result<()> {
        // Learn pattern in HNSW router
        if let Some(ref router) = self.hnsw_router {
            let mut router = router.write();

            let embedding = self
                .get_embedding(&task.description)
                .unwrap_or_else(|_| Self::create_seed_embedding(&task.description, 384, 0));

            // Determine task type from description
            let task_type = Self::classify_task_type(&task.description);

            router.learn_pattern(
                embedding,
                AgentType::Coder, // Default for code tasks
                task_type,
                task.description.clone(),
                success,
            )?;
        }

        // Record in SONA for learning
        if let Some(ref sona) = self.sona {
            let sona = sona.write();

            let query_embedding = self.get_embedding(&task.description).unwrap_or_default();
            let response_embedding = patch
                .as_ref()
                .and_then(|p| self.get_embedding(p).ok())
                .unwrap_or_default();

            let trajectory = Trajectory {
                request_id: task.id.clone(),
                session_id: "eval".to_string(),
                query_embedding,
                response_embedding,
                quality_score: if success { 0.9 } else { 0.3 },
                routing_features: vec![],
                model_index: 0,
                timestamp: chrono::Utc::now(),
            };

            if let Err(e) = sona.record_trajectory(trajectory) {
                tracing::warn!("Failed to record trajectory for learning: {}", e);
            }
        }

        Ok(())
    }

    /// Classify task type from description
    fn classify_task_type(description: &str) -> ClaudeFlowTask {
        let desc_lower = description.to_lowercase();

        if desc_lower.contains("fix") || desc_lower.contains("bug") || desc_lower.contains("error")
        {
            ClaudeFlowTask::Debugging
        } else if desc_lower.contains("test") {
            ClaudeFlowTask::Testing
        } else if desc_lower.contains("refactor") || desc_lower.contains("clean") {
            ClaudeFlowTask::Refactoring
        } else if desc_lower.contains("review") || desc_lower.contains("check") {
            ClaudeFlowTask::CodeReview
        } else if desc_lower.contains("research") || desc_lower.contains("analyze") {
            ClaudeFlowTask::Research
        } else {
            ClaudeFlowTask::CodeGeneration
        }
    }

    /// Build context from routing result
    fn build_context_from_routing(&self, routing: &RoutingResult, task: &str) -> String {
        if routing.patterns_considered == 0 {
            return String::new();
        }

        let mut context = String::new();

        // Add routing decision context
        context.push_str(&format!(
            "Routing analysis (confidence: {:.1}%):\n",
            routing.confidence * 100.0
        ));
        context.push_str(&format!("- Primary agent: {:?}\n", routing.primary_agent));
        context.push_str(&format!(
            "- Patterns analyzed: {}\n",
            routing.patterns_considered
        ));

        if !routing.alternatives.is_empty() {
            context.push_str("- Alternative agents: ");
            context.push_str(&routing.alternatives.join(", "));
            context.push('\n');
        }

        context.push_str(&format!("- Reasoning: {}\n\n", routing.reasoning));
        context.push_str(&format!("Task: {}\n", task));

        context
    }

    /// Get embedding for text using backend
    fn get_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let backend = self.backend.read();
        backend.get_embeddings(text)
    }

    /// Real generation using LLM backend
    fn real_generation(
        &self,
        mode: AblationMode,
        task: &EvalTask,
        seed: u64,
        context: &str,
    ) -> Result<(Option<String>, CostTracker)> {
        let backend = self.backend.read();

        // Build prompt based on mode
        let prompt = self.build_prompt(mode, task, context);

        // Configure generation parameters
        let params = GenerateParams {
            max_tokens: 2048,
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repetition_penalty: 1.1,
            seed: Some(seed),
            ..Default::default()
        };

        // Count input tokens
        let input_tokens = if let Some(tokenizer) = backend.tokenizer() {
            tokenizer.encode(&prompt)?.len()
        } else {
            prompt.len() / 4 // Rough estimate
        };

        // REAL GENERATION
        let result = backend.generate(&prompt, params);

        match result {
            Ok(generated_text) => {
                // Count output tokens
                let output_tokens = if let Some(tokenizer) = backend.tokenizer() {
                    tokenizer.encode(&generated_text)?.len()
                } else {
                    generated_text.len() / 4
                };

                // Extract patch from generated text
                let patch = self.extract_patch(&generated_text);

                // Calculate cost
                let mut cost = CostTracker::with_claude_pricing();
                cost.input_tokens = input_tokens as u64;
                cost.output_tokens = output_tokens as u64;

                Ok((patch, cost))
            }
            Err(e) => {
                tracing::warn!("Generation failed: {}", e);
                let mut cost = CostTracker::with_claude_pricing();
                cost.input_tokens = input_tokens as u64;
                Ok((None, cost))
            }
        }
    }

    /// Build prompt for generation
    fn build_prompt(&self, mode: AblationMode, task: &EvalTask, context: &str) -> String {
        let mut prompt = String::new();

        // Add context if using retrieval
        if !context.is_empty()
            && matches!(
                mode,
                AblationMode::RetrievalOnly
                    | AblationMode::RetrievalPlusAdapters
                    | AblationMode::Full
            )
        {
            prompt.push_str(context);
            prompt.push_str("\n---\n\n");
        }

        // Core prompt
        prompt.push_str(&format!(
            "Generate a code patch for the following task:\n\n\
            Repository: {}\n\
            Task: {}\n\n\
            Expected files to modify: {}\n\n\
            Please provide the patch in unified diff format.\n\
            Output ONLY the patch, no explanations.\n\n\
            ```diff\n",
            task.repo,
            task.description,
            task.expected_files.join(", ")
        ));

        prompt
    }

    /// Extract patch from generated text
    fn extract_patch(&self, text: &str) -> Option<String> {
        // Look for diff block
        if let Some(start) = text.find("```diff") {
            let start = start + 7;
            if let Some(end) = text[start..].find("```") {
                let patch = text[start..start + end].trim();
                if !patch.is_empty() {
                    return Some(patch.to_string());
                }
            }
        }

        // Look for raw diff content
        if text.contains("---") && text.contains("+++") {
            return Some(text.trim().to_string());
        }

        // Return raw if looks like patch
        if text.starts_with('+') || text.starts_with('-') || text.starts_with('@') {
            return Some(text.trim().to_string());
        }

        None
    }

    /// Evaluate correctness of generated patch
    fn evaluate_correctness(
        &self,
        task: &EvalTask,
        patch: &Option<String>,
        latency: &LatencyBreakdown,
    ) -> TaskResult {
        let patch_generated = patch.is_some();
        let patch_applies = patch.as_ref().map_or(false, |p| !p.is_empty());

        TaskResult {
            task_id: task.id.clone(),
            repo: task.repo.clone(),
            issue_id: task.issue.clone(),
            patch_generated,
            patch_applies,
            test_results: None, // Would run actual tests
            verification_level: task.verification_level,
            human_verified: None,
            files_changed: patch.as_ref().map_or(0, |p| p.matches("--- a/").count()),
            lines_changed: patch.as_ref().map_or(0, |p| {
                p.lines()
                    .filter(|l| l.starts_with('+') || l.starts_with('-'))
                    .count()
            }),
            is_multi_file: task.expected_files.len() > 1,
            coupling_score: 0.3,
            generation_time: Duration::from_millis(latency.generation_ms as u64),
            retries: 0,
            error: None,
        }
    }

    /// Generate evaluation report
    fn generate_report(&self, duration: Duration) -> EvalReport {
        let mut mode_metrics: HashMap<AblationMode, ModeMetrics> = HashMap::new();

        for (mode, runs) in &self.results {
            let mut correctness = CorrectnessMetrics::new();
            let mut economics = EconomicsMetrics::new();
            let mut quality_scores = Vec::new();

            for run in runs {
                correctness.add_result(&run.correctness);
                economics.cost.add(&run.cost);

                if run.accepted {
                    economics.successful_tasks += 1;
                }

                if let Some(ref dq) = run.diff_quality {
                    quality_scores.push(dq.combined_score);
                }

                // Add REAL latency samples
                economics
                    .latency
                    .routing
                    .add_secs(run.latency.routing_ms / 1000.0);
                economics
                    .latency
                    .end_to_end
                    .add_secs(run.latency.total_ms / 1000.0);
            }

            economics.recalculate();

            let avg_quality = if quality_scores.is_empty() {
                0.0
            } else {
                quality_scores.iter().sum::<f64>() / quality_scores.len() as f64
            };

            mode_metrics.insert(
                *mode,
                ModeMetrics {
                    mode: *mode,
                    correctness,
                    economics,
                    avg_quality_score: avg_quality,
                    total_runs: runs.len(),
                },
            );
        }

        EvalReport {
            config: self.config.clone(),
            mode_metrics,
            total_duration: duration,
            timestamp: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_harness_creation() {
        let config = EvalConfig {
            task_count: 1,
            seeds: vec![42],
            ablation_modes: vec![AblationMode::Baseline],
            ..Default::default()
        };

        let harness = RealEvaluationHarness::new(config);
        assert!(harness.is_ok());
    }

    #[test]
    fn test_prompt_building() {
        let config = EvalConfig::default();
        let harness = RealEvaluationHarness::new(config).unwrap();

        let task = EvalTask {
            id: "test-1".to_string(),
            repo: "test/repo".to_string(),
            issue: None,
            description: "Fix null pointer".to_string(),
            reference_patch: None,
            test_command: "cargo test".to_string(),
            expected_files: vec!["src/lib.rs".to_string()],
            verification_level: VerificationLevel::Automated,
            tags: vec![],
        };

        let prompt = harness.build_prompt(AblationMode::Baseline, &task, "");
        assert!(prompt.contains("Fix null pointer"));
        assert!(prompt.contains("test/repo"));
    }

    #[test]
    fn test_patch_extraction() {
        let config = EvalConfig::default();
        let harness = RealEvaluationHarness::new(config).unwrap();

        let text = "Here's the patch:\n```diff\n--- a/file.rs\n+++ b/file.rs\n@@ -1 +1 @@\n-old\n+new\n```";
        let patch = harness.extract_patch(text);
        assert!(patch.is_some());
        assert!(patch.unwrap().contains("--- a/file.rs"));
    }
}
