//! Tool Orchestration Problems Domain
//!
//! Generates tasks requiring coordinating multiple tools/agents to achieve goals.
//! Task types include:
//!
//! - **PipelineConstruction**: Build a data processing pipeline from available tools
//! - **ErrorRecovery**: Handle failures in multi-step tool chains
//! - **ParallelCoordination**: Execute independent tool calls concurrently
//! - **ResourceNegotiation**: Manage shared resources across tool invocations
//! - **AdaptiveRouting**: Select tools dynamically based on intermediate results
//!
//! Cross-domain transfer is strongest here: planning decomposes goals,
//! Rust synthesis provides execution patterns, and orchestration combines them.

use crate::domain::{Domain, DomainEmbedding, DomainId, Evaluation, Solution, Task};
use rand::Rng;
use serde::{Deserialize, Serialize};

const EMBEDDING_DIM: usize = 64;

/// Categories of tool orchestration tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrchestrationCategory {
    /// Build a pipeline: chain tools to transform input to desired output.
    PipelineConstruction,
    /// Handle failure: detect errors and apply fallback strategies.
    ErrorRecovery,
    /// Coordinate parallel: dispatch independent calls and merge results.
    ParallelCoordination,
    /// Negotiate resources: manage rate limits, quotas, shared state.
    ResourceNegotiation,
    /// Adaptive routing: choose tool based on intermediate result properties.
    AdaptiveRouting,
}

/// A tool available in the orchestration environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// Input type signature (e.g., "text", "json", "binary").
    pub input_type: String,
    /// Output type signature.
    pub output_type: String,
    /// Average latency in milliseconds.
    pub latency_ms: u32,
    /// Failure rate [0.0, 1.0].
    pub failure_rate: f32,
    /// Cost per invocation.
    pub cost: f32,
    /// Rate limit (max calls per minute), 0 = unlimited.
    pub rate_limit: u32,
}

/// An orchestration task specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationTaskSpec {
    pub category: OrchestrationCategory,
    pub description: String,
    /// Available tools in the environment.
    pub available_tools: Vec<ToolSpec>,
    /// Input to the pipeline.
    pub input: serde_json::Value,
    /// Expected output type/shape.
    pub expected_output_type: String,
    /// Maximum total latency budget (ms).
    pub latency_budget_ms: u32,
    /// Maximum total cost budget.
    pub cost_budget: f32,
    /// Required reliability (min success rate).
    pub min_reliability: f32,
    /// Error scenarios that must be handled.
    pub error_scenarios: Vec<String>,
}

/// A tool call in an orchestration solution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_name: String,
    /// Input to this tool call (ref to previous output or literal).
    pub input_ref: String,
    /// Whether this can run in parallel with other calls.
    pub parallel_group: Option<u32>,
    /// Fallback tool if this one fails.
    pub fallback: Option<String>,
    /// Retry count on failure.
    pub retries: u32,
}

/// A parsed orchestration plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationPlan {
    pub calls: Vec<ToolCall>,
    /// Error handling strategy description.
    pub error_strategy: String,
}

/// Tool orchestration domain.
pub struct ToolOrchestrationDomain {
    id: DomainId,
}

impl ToolOrchestrationDomain {
    pub fn new() -> Self {
        Self {
            id: DomainId("tool_orchestration".to_string()),
        }
    }

    fn base_tools() -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: "text_extract".into(),
                description: "Extract text from documents".into(),
                input_type: "binary".into(),
                output_type: "text".into(),
                latency_ms: 50,
                failure_rate: 0.02,
                cost: 0.001,
                rate_limit: 100,
            },
            ToolSpec {
                name: "text_embed".into(),
                description: "Generate embeddings from text".into(),
                input_type: "text".into(),
                output_type: "vector".into(),
                latency_ms: 30,
                failure_rate: 0.01,
                cost: 0.002,
                rate_limit: 200,
            },
            ToolSpec {
                name: "vector_search".into(),
                description: "Search vector index for similar items".into(),
                input_type: "vector".into(),
                output_type: "json".into(),
                latency_ms: 10,
                failure_rate: 0.005,
                cost: 0.0005,
                rate_limit: 500,
            },
            ToolSpec {
                name: "llm_generate".into(),
                description: "Generate text using language model".into(),
                input_type: "text".into(),
                output_type: "text".into(),
                latency_ms: 2000,
                failure_rate: 0.05,
                cost: 0.01,
                rate_limit: 30,
            },
            ToolSpec {
                name: "json_transform".into(),
                description: "Apply JQ-like transformations to JSON".into(),
                input_type: "json".into(),
                output_type: "json".into(),
                latency_ms: 5,
                failure_rate: 0.001,
                cost: 0.0001,
                rate_limit: 0,
            },
            ToolSpec {
                name: "code_execute".into(),
                description: "Execute code in sandboxed environment".into(),
                input_type: "text".into(),
                output_type: "json".into(),
                latency_ms: 500,
                failure_rate: 0.1,
                cost: 0.005,
                rate_limit: 20,
            },
            ToolSpec {
                name: "http_fetch".into(),
                description: "Fetch data from external HTTP endpoint".into(),
                input_type: "text".into(),
                output_type: "json".into(),
                latency_ms: 300,
                failure_rate: 0.15,
                cost: 0.0,
                rate_limit: 60,
            },
            ToolSpec {
                name: "cache_lookup".into(),
                description: "Check local cache for previously computed results".into(),
                input_type: "text".into(),
                output_type: "json".into(),
                latency_ms: 1,
                failure_rate: 0.0,
                cost: 0.0,
                rate_limit: 0,
            },
            ToolSpec {
                name: "validator".into(),
                description: "Validate output against schema".into(),
                input_type: "json".into(),
                output_type: "json".into(),
                latency_ms: 2,
                failure_rate: 0.0,
                cost: 0.0,
                rate_limit: 0,
            },
            ToolSpec {
                name: "aggregator".into(),
                description: "Merge multiple results into one".into(),
                input_type: "json".into(),
                output_type: "json".into(),
                latency_ms: 5,
                failure_rate: 0.0,
                cost: 0.0001,
                rate_limit: 0,
            },
        ]
    }

    fn gen_pipeline(&self, difficulty: f32) -> OrchestrationTaskSpec {
        let tools = Self::base_tools();
        let num_tools = if difficulty < 0.3 {
            3
        } else if difficulty < 0.7 {
            6
        } else {
            10
        };

        OrchestrationTaskSpec {
            category: OrchestrationCategory::PipelineConstruction,
            description: format!(
                "Build a RAG pipeline using {} tools: extract, embed, search, generate.",
                num_tools
            ),
            available_tools: tools[..num_tools.min(tools.len())].to_vec(),
            input: serde_json::json!({"type": "binary", "format": "pdf"}),
            expected_output_type: "text".into(),
            latency_budget_ms: if difficulty < 0.5 { 5000 } else { 2000 },
            cost_budget: if difficulty < 0.5 { 0.1 } else { 0.02 },
            min_reliability: if difficulty < 0.5 { 0.9 } else { 0.99 },
            error_scenarios: Vec::new(),
        }
    }

    fn gen_error_recovery(&self, difficulty: f32) -> OrchestrationTaskSpec {
        let tools = Self::base_tools();
        let error_scenarios = if difficulty < 0.3 {
            vec!["timeout on llm_generate".into()]
        } else if difficulty < 0.7 {
            vec![
                "timeout on llm_generate".into(),
                "http_fetch returns 429".into(),
                "code_execute sandbox OOM".into(),
            ]
        } else {
            vec![
                "timeout on llm_generate".into(),
                "http_fetch returns 429".into(),
                "code_execute sandbox OOM".into(),
                "vector_search index corruption".into(),
                "cascading failure: embed + search both down".into(),
            ]
        };

        OrchestrationTaskSpec {
            category: OrchestrationCategory::ErrorRecovery,
            description: format!(
                "Handle {} error scenarios in a multi-tool pipeline with graceful degradation.",
                error_scenarios.len()
            ),
            available_tools: tools,
            input: serde_json::json!({"type": "text", "content": "query"}),
            expected_output_type: "json".into(),
            latency_budget_ms: 10000,
            cost_budget: 0.1,
            min_reliability: 0.95,
            error_scenarios,
        }
    }

    fn gen_parallel_coordination(&self, difficulty: f32) -> OrchestrationTaskSpec {
        let tools = Self::base_tools();
        let parallelism = if difficulty < 0.3 {
            2
        } else if difficulty < 0.7 {
            4
        } else {
            8
        };

        OrchestrationTaskSpec {
            category: OrchestrationCategory::ParallelCoordination,
            description: format!(
                "Execute {} independent tool chains in parallel, merge results within latency budget.",
                parallelism
            ),
            available_tools: tools,
            input: serde_json::json!({"queries": (0..parallelism).map(|i| format!("query_{}", i)).collect::<Vec<_>>()}),
            expected_output_type: "json".into(),
            latency_budget_ms: if difficulty < 0.5 { 3000 } else { 1000 },
            cost_budget: 0.05 * parallelism as f32,
            min_reliability: 0.95,
            error_scenarios: Vec::new(),
        }
    }

    fn extract_features(&self, solution: &Solution) -> Vec<f32> {
        let content = &solution.content;
        let mut features = vec![0.0f32; EMBEDDING_DIM];

        let plan: OrchestrationPlan = serde_json::from_str(&solution.data.to_string())
            .or_else(|_| serde_json::from_str(content))
            .unwrap_or(OrchestrationPlan {
                calls: Vec::new(),
                error_strategy: String::new(),
            });

        // Feature 0-7: Plan structure
        features[0] = plan.calls.len() as f32 / 20.0;
        let unique_tools: std::collections::HashSet<&str> =
            plan.calls.iter().map(|c| c.tool_name.as_str()).collect();
        features[1] = unique_tools.len() as f32 / 10.0;
        // Parallelism ratio
        let parallel_calls = plan
            .calls
            .iter()
            .filter(|c| c.parallel_group.is_some())
            .count();
        features[2] = parallel_calls as f32 / plan.calls.len().max(1) as f32;
        // Fallback coverage
        let fallback_calls = plan.calls.iter().filter(|c| c.fallback.is_some()).count();
        features[3] = fallback_calls as f32 / plan.calls.len().max(1) as f32;
        // Average retries
        let total_retries: u32 = plan.calls.iter().map(|c| c.retries).sum();
        features[4] = total_retries as f32 / plan.calls.len().max(1) as f32 / 5.0;

        // Feature 8-15: Tool type usage
        let tool_names = [
            "extract",
            "embed",
            "search",
            "generate",
            "transform",
            "execute",
            "fetch",
            "cache",
        ];
        for (i, name) in tool_names.iter().enumerate() {
            features[8 + i] = plan
                .calls
                .iter()
                .filter(|c| c.tool_name.contains(name))
                .count() as f32
                / plan.calls.len().max(1) as f32;
        }

        // Feature 16-23: Text pattern features
        features[16] = content.matches("pipeline").count() as f32 / 3.0;
        features[17] = content.matches("parallel").count() as f32 / 5.0;
        features[18] = content.matches("fallback").count() as f32 / 5.0;
        features[19] = content.matches("retry").count() as f32 / 5.0;
        features[20] = content.matches("cache").count() as f32 / 5.0;
        features[21] = content.matches("timeout").count() as f32 / 3.0;
        features[22] = content.matches("merge").count() as f32 / 3.0;
        features[23] = content.matches("validate").count() as f32 / 3.0;

        // Feature 32-39: Error handling patterns
        features[32] = content.matches("error").count() as f32 / 5.0;
        features[33] = content.matches("recover").count() as f32 / 3.0;
        features[34] = content.matches("degrade").count() as f32 / 3.0;
        features[35] = content.matches("circuit_break").count() as f32 / 2.0;
        features[36] = content.matches("rate_limit").count() as f32 / 3.0;
        features[37] = content.matches("backoff").count() as f32 / 3.0;
        features[38] = content.matches("health_check").count() as f32 / 2.0;
        features[39] = content.matches("monitor").count() as f32 / 3.0;

        // Feature 48-55: Coordination patterns
        features[48] = content.matches("scatter").count() as f32 / 2.0;
        features[49] = content.matches("gather").count() as f32 / 2.0;
        features[50] = content.matches("fan_out").count() as f32 / 2.0;
        features[51] = content.matches("aggregate").count() as f32 / 3.0;
        features[52] = content.matches("route").count() as f32 / 3.0;
        features[53] = content.matches("dispatch").count() as f32 / 3.0;
        features[54] = content.matches("await").count() as f32 / 5.0;
        features[55] = content.matches("join").count() as f32 / 3.0;

        // Normalize
        let norm: f32 = features.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-10 {
            for f in &mut features {
                *f /= norm;
            }
        }

        features
    }

    fn score_orchestration(&self, spec: &OrchestrationTaskSpec, solution: &Solution) -> Evaluation {
        let content = &solution.content;
        let mut correctness = 0.0f32;
        let mut efficiency = 0.5f32;
        let mut elegance = 0.5f32;
        let mut notes = Vec::new();

        let plan: Option<OrchestrationPlan> = serde_json::from_str(&solution.data.to_string())
            .ok()
            .or_else(|| serde_json::from_str(content).ok());

        let plan = match plan {
            Some(p) => p,
            None => {
                let has_tools = spec
                    .available_tools
                    .iter()
                    .any(|t| content.contains(&t.name));
                if has_tools {
                    correctness = 0.2;
                }
                return Evaluation {
                    score: correctness * 0.6,
                    correctness,
                    efficiency: 0.0,
                    elegance: 0.0,
                    constraint_results: Vec::new(),
                    notes: vec!["Could not parse orchestration plan".into()],
                };
            }
        };

        if plan.calls.is_empty() {
            return Evaluation::zero(vec!["Empty orchestration plan".into()]);
        }

        // Correctness: type chain validity
        let mut type_errors = 0;
        for window in plan.calls.windows(2) {
            let output_tool = spec
                .available_tools
                .iter()
                .find(|t| t.name == window[0].tool_name);
            let input_tool = spec
                .available_tools
                .iter()
                .find(|t| t.name == window[1].tool_name);

            if let (Some(out_t), Some(in_t)) = (output_tool, input_tool) {
                if window[1].parallel_group.is_none() && out_t.output_type != in_t.input_type {
                    type_errors += 1;
                    notes.push(format!(
                        "Type mismatch: {} outputs {} but {} expects {}",
                        out_t.name, out_t.output_type, in_t.name, in_t.input_type
                    ));
                }
            }
        }
        let chain_len = (plan.calls.len() - 1).max(1);
        correctness = 1.0 - (type_errors as f32 / chain_len as f32);

        // Tool coverage: do we use tools that produce the expected output?
        let produces_output = plan.calls.iter().any(|c| {
            spec.available_tools
                .iter()
                .any(|t| t.name == c.tool_name && t.output_type == spec.expected_output_type)
        });
        if !produces_output {
            correctness *= 0.5;
            notes.push("No tool produces the expected output type".into());
        }

        // Error handling coverage
        if !spec.error_scenarios.is_empty() {
            let handled = spec
                .error_scenarios
                .iter()
                .filter(|scenario| {
                    plan.calls
                        .iter()
                        .any(|c| c.fallback.is_some() || c.retries > 0)
                        || plan
                            .error_strategy
                            .contains(&scenario.as_str()[..scenario.len().min(10)])
                })
                .count() as f32
                / spec.error_scenarios.len() as f32;
            correctness = correctness * 0.7 + handled * 0.3;
        }

        // Efficiency: estimated latency and cost
        let est_latency: u32 = {
            let mut groups: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
            let mut sequential_latency = 0u32;
            for call in &plan.calls {
                let tool_latency = spec
                    .available_tools
                    .iter()
                    .find(|t| t.name == call.tool_name)
                    .map(|t| t.latency_ms)
                    .unwrap_or(100);

                if let Some(group) = call.parallel_group {
                    let entry = groups.entry(group).or_insert(0);
                    *entry = (*entry).max(tool_latency);
                } else {
                    sequential_latency += tool_latency;
                }
            }
            sequential_latency + groups.values().sum::<u32>()
        };

        if est_latency <= spec.latency_budget_ms {
            efficiency = 1.0 - (est_latency as f32 / spec.latency_budget_ms as f32 * 0.5);
        } else {
            efficiency = spec.latency_budget_ms as f32 / est_latency as f32 * 0.5;
            notes.push(format!(
                "Estimated latency {}ms exceeds budget {}ms",
                est_latency, spec.latency_budget_ms
            ));
        }

        let est_cost: f32 = plan
            .calls
            .iter()
            .filter_map(|c| {
                spec.available_tools
                    .iter()
                    .find(|t| t.name == c.tool_name)
                    .map(|t| t.cost * (1.0 + c.retries as f32))
            })
            .sum();

        if est_cost > spec.cost_budget {
            efficiency *= 0.7;
            notes.push(format!(
                "Cost {:.4} exceeds budget {:.4}",
                est_cost, spec.cost_budget
            ));
        }

        // Elegance: parallelism, caching, minimal redundancy
        let parallelism_used = plan.calls.iter().any(|c| c.parallel_group.is_some());
        if parallelism_used {
            elegance += 0.15;
        }

        let cache_used = plan.calls.iter().any(|c| c.tool_name.contains("cache"));
        if cache_used {
            elegance += 0.1;
        }

        let validation_used = plan.calls.iter().any(|c| c.tool_name.contains("validat"));
        if validation_used {
            elegance += 0.1;
        }

        // Penalize excessive retries
        let total_retries: u32 = plan.calls.iter().map(|c| c.retries).sum();
        if total_retries > plan.calls.len() as u32 * 2 {
            elegance -= 0.2;
            notes.push("Excessive retry configuration".into());
        }

        elegance = elegance.clamp(0.0, 1.0);

        let score = 0.6 * correctness + 0.25 * efficiency + 0.15 * elegance;
        Evaluation {
            score: score.clamp(0.0, 1.0),
            correctness,
            efficiency,
            elegance,
            constraint_results: Vec::new(),
            notes,
        }
    }
}

impl Default for ToolOrchestrationDomain {
    fn default() -> Self {
        Self::new()
    }
}

impl Domain for ToolOrchestrationDomain {
    fn id(&self) -> &DomainId {
        &self.id
    }

    fn name(&self) -> &str {
        "Tool Orchestration"
    }

    fn generate_tasks(&self, count: usize, difficulty: f32) -> Vec<Task> {
        let mut rng = rand::thread_rng();
        let difficulty = difficulty.clamp(0.0, 1.0);

        (0..count)
            .map(|i| {
                let roll: f32 = rng.gen();
                let spec = if roll < 0.4 {
                    self.gen_pipeline(difficulty)
                } else if roll < 0.7 {
                    self.gen_error_recovery(difficulty)
                } else {
                    self.gen_parallel_coordination(difficulty)
                };

                Task {
                    id: format!("orch_{}_d{:.0}", i, difficulty * 100.0),
                    domain_id: self.id.clone(),
                    difficulty,
                    spec: serde_json::to_value(&spec).unwrap_or_default(),
                    constraints: Vec::new(),
                }
            })
            .collect()
    }

    fn evaluate(&self, task: &Task, solution: &Solution) -> Evaluation {
        let spec: OrchestrationTaskSpec = match serde_json::from_value(task.spec.clone()) {
            Ok(s) => s,
            Err(e) => return Evaluation::zero(vec![format!("Invalid task spec: {}", e)]),
        };
        self.score_orchestration(&spec, solution)
    }

    fn embed(&self, solution: &Solution) -> DomainEmbedding {
        let features = self.extract_features(solution);
        DomainEmbedding::new(features, self.id.clone())
    }

    fn embedding_dim(&self) -> usize {
        EMBEDDING_DIM
    }

    fn reference_solution(&self, task: &Task) -> Option<Solution> {
        let spec: OrchestrationTaskSpec = serde_json::from_value(task.spec.clone()).ok()?;

        // Build a sequential pipeline through available tools
        let calls: Vec<ToolCall> = spec
            .available_tools
            .iter()
            .map(|t| ToolCall {
                tool_name: t.name.clone(),
                input_ref: "previous".into(),
                parallel_group: None,
                fallback: None,
                retries: if t.failure_rate > 0.05 { 2 } else { 0 },
            })
            .collect();

        let plan = OrchestrationPlan {
            calls,
            error_strategy: "retry with exponential backoff".into(),
        };

        let content = serde_json::to_string_pretty(&plan).ok()?;
        Some(Solution {
            task_id: task.id.clone(),
            content,
            data: serde_json::to_value(&plan).ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_orchestration_tasks() {
        let domain = ToolOrchestrationDomain::new();
        let tasks = domain.generate_tasks(5, 0.5);
        assert_eq!(tasks.len(), 5);
        for task in &tasks {
            assert_eq!(task.domain_id, domain.id);
        }
    }

    #[test]
    fn test_reference_solution() {
        let domain = ToolOrchestrationDomain::new();
        let tasks = domain.generate_tasks(3, 0.3);
        for task in &tasks {
            let ref_sol = domain.reference_solution(task);
            assert!(ref_sol.is_some());
        }
    }

    #[test]
    fn test_evaluate_reference() {
        let domain = ToolOrchestrationDomain::new();
        let tasks = domain.generate_tasks(3, 0.3);
        for task in &tasks {
            if let Some(solution) = domain.reference_solution(task) {
                let eval = domain.evaluate(task, &solution);
                assert!(eval.score >= 0.0 && eval.score <= 1.0);
            }
        }
    }

    #[test]
    fn test_embed_orchestration() {
        let domain = ToolOrchestrationDomain::new();
        let solution = Solution {
            task_id: "test".into(),
            content: "pipeline: extract -> embed -> search with fallback and retry".into(),
            data: serde_json::json!({
                "calls": [
                    {"tool_name": "text_extract", "input_ref": "input", "retries": 1}
                ],
                "error_strategy": "retry"
            }),
        };
        let embedding = domain.embed(&solution);
        assert_eq!(embedding.dim, EMBEDDING_DIM);
    }

    #[test]
    fn test_difficulty_affects_error_scenarios() {
        let domain = ToolOrchestrationDomain::new();
        // Generate many tasks at high difficulty to get error recovery tasks
        let hard = domain.generate_tasks(20, 0.9);
        let has_error_tasks = hard.iter().any(|t| {
            let spec: OrchestrationTaskSpec = serde_json::from_value(t.spec.clone()).unwrap();
            !spec.error_scenarios.is_empty()
        });
        assert!(
            has_error_tasks,
            "High difficulty should produce error scenarios"
        );
    }
}
