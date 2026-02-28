# Tiny Dancer Routing Integration Plan

## Overview

Integrate AI agent routing capabilities from `ruvector-tiny-dancer` into PostgreSQL, enabling intelligent request routing, model selection, and cost optimization directly in SQL.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     PostgreSQL Extension                         │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                  Tiny Dancer Router                      │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │    │
│  │  │   FastGRNN   │  │    Route     │  │    Cost      │   │    │
│  │  │   Inference  │  │   Classifier │  │   Optimizer  │   │    │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘   │    │
│  └─────────┼─────────────────┼─────────────────┼───────────┘    │
│            └─────────────────┴─────────────────┘                │
│                              ▼                                   │
│              ┌───────────────────────────┐                       │
│              │   Agent Registry & Pool   │                       │
│              │   (LLMs, Tools, APIs)     │                       │
│              └───────────────────────────┘                       │
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── routing/
│   ├── mod.rs              # Module exports
│   ├── fastgrnn.rs         # FastGRNN neural inference
│   ├── router.rs           # Main routing engine
│   ├── classifier.rs       # Route classification
│   ├── cost_optimizer.rs   # Cost/latency optimization
│   ├── agents/
│   │   ├── registry.rs     # Agent registration
│   │   ├── pool.rs         # Agent pool management
│   │   └── capabilities.rs # Capability matching
│   ├── policies/
│   │   ├── cost.rs         # Cost-based routing
│   │   ├── latency.rs      # Latency-based routing
│   │   ├── quality.rs      # Quality-based routing
│   │   └── hybrid.rs       # Multi-objective routing
│   └── operators.rs        # SQL operators
```

## SQL Interface

### Agent Registration

```sql
-- Register AI agents/models
SELECT ruvector_register_agent(
    name := 'gpt-4',
    agent_type := 'llm',
    capabilities := ARRAY['reasoning', 'code', 'analysis', 'creative'],
    cost_per_1k_tokens := 0.03,
    avg_latency_ms := 2500,
    quality_score := 0.95,
    metadata := '{"provider": "openai", "context_window": 128000}'
);

SELECT ruvector_register_agent(
    name := 'claude-3-haiku',
    agent_type := 'llm',
    capabilities := ARRAY['fast-response', 'simple-tasks', 'classification'],
    cost_per_1k_tokens := 0.00025,
    avg_latency_ms := 400,
    quality_score := 0.80,
    metadata := '{"provider": "anthropic", "context_window": 200000}'
);

SELECT ruvector_register_agent(
    name := 'code-specialist',
    agent_type := 'tool',
    capabilities := ARRAY['code-execution', 'debugging', 'testing'],
    cost_per_call := 0.001,
    avg_latency_ms := 100,
    quality_score := 0.90
);

-- List registered agents
SELECT * FROM ruvector_list_agents();
```

### Basic Routing

```sql
-- Route a request to the best agent
SELECT * FROM ruvector_route(
    request := 'Write a Python function to calculate Fibonacci numbers',
    optimize_for := 'cost'  -- or 'latency', 'quality', 'balanced'
);

-- Result:
-- | agent_name | confidence | estimated_cost | estimated_latency |
-- |------------|------------|----------------|-------------------|
-- | claude-3-haiku | 0.85 | 0.001 | 400ms |

-- Route with constraints
SELECT * FROM ruvector_route(
    request := 'Analyze this complex legal document',
    required_capabilities := ARRAY['reasoning', 'analysis'],
    max_cost := 0.10,
    max_latency_ms := 5000,
    min_quality := 0.90
);

-- Multi-agent routing (for complex tasks)
SELECT * FROM ruvector_route_multi(
    request := 'Build and deploy a web application',
    num_agents := 3,
    strategy := 'pipeline'  -- or 'parallel', 'ensemble'
);
```

### Semantic Routing

```sql
-- Create semantic routes (like function calling)
SELECT ruvector_create_route(
    name := 'customer_support',
    description := 'Handle customer support inquiries, complaints, and feedback',
    embedding := ruvector_embed('Customer support and help requests'),
    target_agent := 'support-agent',
    priority := 1
);

SELECT ruvector_create_route(
    name := 'technical_docs',
    description := 'Answer questions about technical documentation and APIs',
    embedding := ruvector_embed('Technical documentation and API reference'),
    target_agent := 'docs-agent',
    priority := 2
);

-- Semantic route matching
SELECT * FROM ruvector_semantic_route(
    query := 'How do I reset my password?',
    top_k := 3
);

-- Result:
-- | route_name | similarity | target_agent | confidence |
-- |------------|------------|--------------|------------|
-- | customer_support | 0.92 | support-agent | 0.95 |
```

### Cost Optimization

```sql
-- Analyze routing costs
SELECT * FROM ruvector_routing_analytics(
    time_range := '7 days',
    group_by := 'agent'
);

-- Result:
-- | agent | total_requests | total_cost | avg_latency | success_rate |
-- |-------|----------------|------------|-------------|--------------|
-- | gpt-4 | 1000 | $30.00 | 2.5s | 99.2% |
-- | haiku | 5000 | $1.25 | 0.4s | 98.5% |

-- Optimize budget allocation
SELECT * FROM ruvector_optimize_budget(
    monthly_budget := 100.00,
    quality_threshold := 0.85,
    latency_threshold_ms := 2000
);

-- Auto-route with budget awareness
SELECT * FROM ruvector_route(
    request := 'Summarize this article',
    budget_remaining := 10.00,
    optimize_for := 'quality_per_dollar'
);
```

### Batch Routing

```sql
-- Route multiple requests efficiently
SELECT * FROM ruvector_batch_route(
    requests := ARRAY[
        'Simple question 1',
        'Complex analysis task',
        'Code generation request'
    ],
    optimize_for := 'total_cost'
);

-- Classify requests in batch (for preprocessing)
SELECT request_id, ruvector_classify_request(content) AS classification
FROM pending_requests;
```

## Implementation Phases

### Phase 1: FastGRNN Core (Week 1-3)

```rust
// src/routing/fastgrnn.rs

use simsimd::SpatialSimilarity;

/// FastGRNN (Fast Gated Recurrent Neural Network)
/// Lightweight neural network for fast inference
pub struct FastGRNN {
    // Gate weights
    w_gate: Vec<f32>,   // [hidden, input]
    u_gate: Vec<f32>,   // [hidden, hidden]
    b_gate: Vec<f32>,   // [hidden]

    // Update weights
    w_update: Vec<f32>, // [hidden, input]
    u_update: Vec<f32>, // [hidden, hidden]
    b_update: Vec<f32>, // [hidden]

    // Hyperparameters
    zeta: f32,          // Gate sparsity
    nu: f32,            // Update sparsity

    input_dim: usize,
    hidden_dim: usize,
}

impl FastGRNN {
    pub fn new(input_dim: usize, hidden_dim: usize) -> Self {
        Self {
            w_gate: Self::init_weights(hidden_dim, input_dim),
            u_gate: Self::init_weights(hidden_dim, hidden_dim),
            b_gate: vec![0.0; hidden_dim],
            w_update: Self::init_weights(hidden_dim, input_dim),
            u_update: Self::init_weights(hidden_dim, hidden_dim),
            b_update: vec![0.0; hidden_dim],
            zeta: 1.0,
            nu: 1.0,
            input_dim,
            hidden_dim,
        }
    }

    /// Single step forward pass
    /// h_t = (ζ * (1 - z_t) + ν) ⊙ tanh(Wx_t + Uh_{t-1} + b_h) + z_t ⊙ h_{t-1}
    pub fn step(&self, input: &[f32], hidden: &[f32]) -> Vec<f32> {
        // Gate: z = σ(W_z x + U_z h + b_z)
        let gate = self.sigmoid(&self.linear_combine(
            input, hidden,
            &self.w_gate, &self.u_gate, &self.b_gate
        ));

        // Update: h̃ = tanh(W_h x + U_h h + b_h)
        let update = self.tanh(&self.linear_combine(
            input, hidden,
            &self.w_update, &self.u_update, &self.b_update
        ));

        // New hidden: h = (ζ(1-z) + ν) ⊙ h̃ + z ⊙ h
        let mut new_hidden = vec![0.0; self.hidden_dim];
        for i in 0..self.hidden_dim {
            let gate_factor = self.zeta * (1.0 - gate[i]) + self.nu;
            new_hidden[i] = gate_factor * update[i] + gate[i] * hidden[i];
        }

        new_hidden
    }

    /// Process sequence
    pub fn forward(&self, sequence: &[Vec<f32>]) -> Vec<f32> {
        let mut hidden = vec![0.0; self.hidden_dim];

        for input in sequence {
            hidden = self.step(input, &hidden);
        }

        hidden
    }

    /// Process single input (common case for routing)
    pub fn forward_single(&self, input: &[f32]) -> Vec<f32> {
        let hidden = vec![0.0; self.hidden_dim];
        self.step(input, &hidden)
    }

    #[inline]
    fn linear_combine(
        &self,
        input: &[f32],
        hidden: &[f32],
        w: &[f32],
        u: &[f32],
        b: &[f32],
    ) -> Vec<f32> {
        let mut result = b.to_vec();

        // W @ x
        for i in 0..self.hidden_dim {
            for j in 0..self.input_dim {
                result[i] += w[i * self.input_dim + j] * input[j];
            }
        }

        // U @ h
        for i in 0..self.hidden_dim {
            for j in 0..self.hidden_dim {
                result[i] += u[i * self.hidden_dim + j] * hidden[j];
            }
        }

        result
    }

    #[inline]
    fn sigmoid(&self, x: &[f32]) -> Vec<f32> {
        x.iter().map(|&v| 1.0 / (1.0 + (-v).exp())).collect()
    }

    #[inline]
    fn tanh(&self, x: &[f32]) -> Vec<f32> {
        x.iter().map(|&v| v.tanh()).collect()
    }
}
```

### Phase 2: Route Classifier (Week 4-5)

```rust
// src/routing/classifier.rs

/// Route classifier using FastGRNN + linear head
pub struct RouteClassifier {
    fastgrnn: FastGRNN,
    classifier_head: Vec<f32>,  // [num_classes, hidden_dim]
    num_classes: usize,
    class_names: Vec<String>,
}

impl RouteClassifier {
    /// Classify request to route category
    pub fn classify(&self, embedding: &[f32]) -> Vec<(String, f32)> {
        // FastGRNN encoding
        let hidden = self.fastgrnn.forward_single(embedding);

        // Linear classifier
        let mut logits = vec![0.0; self.num_classes];
        for i in 0..self.num_classes {
            for j in 0..hidden.len() {
                logits[i] += self.classifier_head[i * hidden.len() + j] * hidden[j];
            }
        }

        // Softmax
        let probs = softmax(&logits);

        // Return sorted by probability
        let mut results: Vec<_> = self.class_names.iter()
            .zip(probs.iter())
            .map(|(name, &prob)| (name.clone(), prob))
            .collect();

        results.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        results
    }

    /// Multi-label classification (request may need multiple capabilities)
    pub fn classify_capabilities(&self, embedding: &[f32]) -> Vec<(String, f32)> {
        let hidden = self.fastgrnn.forward_single(embedding);

        // Sigmoid for multi-label
        let mut results = Vec::new();
        for i in 0..self.num_classes {
            let mut logit = 0.0;
            for j in 0..hidden.len() {
                logit += self.classifier_head[i * hidden.len() + j] * hidden[j];
            }
            let prob = 1.0 / (1.0 + (-logit).exp());

            if prob > 0.5 {
                results.push((self.class_names[i].clone(), prob));
            }
        }

        results.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());
        results
    }
}

#[pg_extern]
fn ruvector_classify_request(request: &str) -> pgrx::JsonB {
    let embedding = get_embedding(request);
    let classifier = get_route_classifier();

    let classifications = classifier.classify(&embedding);

    pgrx::JsonB(serde_json::json!({
        "classifications": classifications,
        "top_category": classifications.first().map(|(name, _)| name),
        "confidence": classifications.first().map(|(_, prob)| prob),
    }))
}
```

### Phase 3: Agent Registry (Week 6-7)

```rust
// src/routing/agents/registry.rs

use dashmap::DashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub agent_type: AgentType,
    pub capabilities: Vec<String>,
    pub capability_embedding: Vec<f32>,  // Embedding of capabilities for semantic matching
    pub cost_model: CostModel,
    pub performance: AgentPerformance,
    pub metadata: serde_json::Value,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentType {
    LLM,
    Tool,
    API,
    Human,
    Ensemble,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostModel {
    pub cost_per_1k_tokens: Option<f64>,
    pub cost_per_call: Option<f64>,
    pub cost_per_second: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPerformance {
    pub avg_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub quality_score: f64,
    pub success_rate: f64,
    pub total_requests: u64,
}

/// Global agent registry
pub struct AgentRegistry {
    agents: DashMap<String, Agent>,
    capability_index: HnswIndex,  // For semantic capability matching
}

impl AgentRegistry {
    pub fn register(&self, agent: Agent) -> Result<(), RegistryError> {
        // Index capability embedding
        let embedding = &agent.capability_embedding;
        self.capability_index.insert(&agent.name, embedding);

        self.agents.insert(agent.name.clone(), agent);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Agent> {
        self.agents.get(name).map(|a| a.clone())
    }

    pub fn find_by_capability(&self, capability: &str, k: usize) -> Vec<&Agent> {
        let embedding = get_embedding(capability);
        let results = self.capability_index.search(&embedding, k);

        results.iter()
            .filter_map(|(name, _)| self.agents.get(name.as_str()).map(|a| a.value()))
            .collect()
    }

    pub fn list_active(&self) -> Vec<Agent> {
        self.agents.iter()
            .filter(|a| a.active)
            .map(|a| a.clone())
            .collect()
    }
}

#[pg_extern]
fn ruvector_register_agent(
    name: &str,
    agent_type: &str,
    capabilities: Vec<String>,
    cost_per_1k_tokens: default!(Option<f64>, "NULL"),
    cost_per_call: default!(Option<f64>, "NULL"),
    avg_latency_ms: f64,
    quality_score: f64,
    metadata: default!(Option<pgrx::JsonB>, "NULL"),
) -> bool {
    let registry = get_agent_registry();

    // Create capability embedding
    let capability_text = capabilities.join(", ");
    let capability_embedding = get_embedding(&capability_text);

    let agent = Agent {
        name: name.to_string(),
        agent_type: agent_type.parse().unwrap_or(AgentType::LLM),
        capabilities,
        capability_embedding,
        cost_model: CostModel {
            cost_per_1k_tokens,
            cost_per_call,
            cost_per_second: None,
        },
        performance: AgentPerformance {
            avg_latency_ms,
            p99_latency_ms: avg_latency_ms * 2.0,
            quality_score,
            success_rate: 1.0,
            total_requests: 0,
        },
        metadata: metadata.map(|m| m.0).unwrap_or(serde_json::json!({})),
        active: true,
    };

    registry.register(agent).is_ok()
}
```

### Phase 4: Routing Engine (Week 8-9)

```rust
// src/routing/router.rs

pub struct Router {
    registry: Arc<AgentRegistry>,
    classifier: Arc<RouteClassifier>,
    optimizer: Arc<CostOptimizer>,
    semantic_routes: Arc<SemanticRoutes>,
}

#[derive(Debug, Clone)]
pub struct RoutingDecision {
    pub agent: Agent,
    pub confidence: f64,
    pub estimated_cost: f64,
    pub estimated_latency_ms: f64,
    pub reasoning: String,
}

#[derive(Debug, Clone)]
pub struct RoutingConstraints {
    pub required_capabilities: Option<Vec<String>>,
    pub max_cost: Option<f64>,
    pub max_latency_ms: Option<f64>,
    pub min_quality: Option<f64>,
    pub excluded_agents: Option<Vec<String>>,
}

impl Router {
    /// Route request to best agent
    pub fn route(
        &self,
        request: &str,
        constraints: &RoutingConstraints,
        optimize_for: OptimizationTarget,
    ) -> Result<RoutingDecision, RoutingError> {
        let embedding = get_embedding(request);

        // Get candidate agents
        let mut candidates = self.get_candidates(&embedding, constraints)?;

        if candidates.is_empty() {
            return Err(RoutingError::NoSuitableAgent);
        }

        // Score candidates
        let scored: Vec<_> = candidates.iter()
            .map(|agent| {
                let score = self.score_agent(agent, &embedding, optimize_for);
                (agent, score)
            })
            .collect();

        // Select best
        let (best_agent, confidence) = scored.into_iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        Ok(RoutingDecision {
            agent: best_agent.clone(),
            confidence,
            estimated_cost: self.estimate_cost(best_agent, request),
            estimated_latency_ms: best_agent.performance.avg_latency_ms,
            reasoning: format!("Selected {} based on {:?} optimization", best_agent.name, optimize_for),
        })
    }

    fn get_candidates(
        &self,
        embedding: &[f32],
        constraints: &RoutingConstraints,
    ) -> Result<Vec<Agent>, RoutingError> {
        let mut candidates: Vec<_> = self.registry.list_active();

        // Filter by required capabilities
        if let Some(required) = &constraints.required_capabilities {
            candidates.retain(|a| {
                required.iter().all(|cap| a.capabilities.contains(cap))
            });
        }

        // Filter by cost
        if let Some(max_cost) = constraints.max_cost {
            candidates.retain(|a| {
                a.cost_model.cost_per_1k_tokens.unwrap_or(0.0) <= max_cost ||
                a.cost_model.cost_per_call.unwrap_or(0.0) <= max_cost
            });
        }

        // Filter by latency
        if let Some(max_latency) = constraints.max_latency_ms {
            candidates.retain(|a| a.performance.avg_latency_ms <= max_latency);
        }

        // Filter by quality
        if let Some(min_quality) = constraints.min_quality {
            candidates.retain(|a| a.performance.quality_score >= min_quality);
        }

        // Filter excluded
        if let Some(excluded) = &constraints.excluded_agents {
            candidates.retain(|a| !excluded.contains(&a.name));
        }

        Ok(candidates)
    }

    fn score_agent(
        &self,
        agent: &Agent,
        request_embedding: &[f32],
        optimize_for: OptimizationTarget,
    ) -> f64 {
        // Capability match score
        let capability_sim = cosine_similarity(request_embedding, &agent.capability_embedding);

        match optimize_for {
            OptimizationTarget::Cost => {
                let cost = agent.cost_model.cost_per_1k_tokens.unwrap_or(0.01);
                capability_sim * (1.0 / (1.0 + cost))
            }
            OptimizationTarget::Latency => {
                let latency_factor = 1.0 / (1.0 + agent.performance.avg_latency_ms / 1000.0);
                capability_sim * latency_factor
            }
            OptimizationTarget::Quality => {
                capability_sim * agent.performance.quality_score
            }
            OptimizationTarget::Balanced => {
                let cost = agent.cost_model.cost_per_1k_tokens.unwrap_or(0.01);
                let cost_factor = 1.0 / (1.0 + cost);
                let latency_factor = 1.0 / (1.0 + agent.performance.avg_latency_ms / 1000.0);
                let quality = agent.performance.quality_score;

                capability_sim * (0.3 * cost_factor + 0.3 * latency_factor + 0.4 * quality)
            }
            OptimizationTarget::QualityPerDollar => {
                let cost = agent.cost_model.cost_per_1k_tokens.unwrap_or(0.01);
                capability_sim * agent.performance.quality_score / (cost + 0.001)
            }
        }
    }

    fn estimate_cost(&self, agent: &Agent, request: &str) -> f64 {
        let estimated_tokens = (request.len() / 4) as f64;  // Rough estimate

        if let Some(cost_per_1k) = agent.cost_model.cost_per_1k_tokens {
            cost_per_1k * estimated_tokens / 1000.0
        } else if let Some(cost_per_call) = agent.cost_model.cost_per_call {
            cost_per_call
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OptimizationTarget {
    Cost,
    Latency,
    Quality,
    Balanced,
    QualityPerDollar,
}

#[pg_extern]
fn ruvector_route(
    request: &str,
    optimize_for: default!(&str, "'balanced'"),
    required_capabilities: default!(Option<Vec<String>>, "NULL"),
    max_cost: default!(Option<f64>, "NULL"),
    max_latency_ms: default!(Option<f64>, "NULL"),
    min_quality: default!(Option<f64>, "NULL"),
) -> pgrx::JsonB {
    let router = get_router();

    let constraints = RoutingConstraints {
        required_capabilities,
        max_cost,
        max_latency_ms,
        min_quality,
        excluded_agents: None,
    };

    let target = match optimize_for {
        "cost" => OptimizationTarget::Cost,
        "latency" => OptimizationTarget::Latency,
        "quality" => OptimizationTarget::Quality,
        "quality_per_dollar" => OptimizationTarget::QualityPerDollar,
        _ => OptimizationTarget::Balanced,
    };

    match router.route(request, &constraints, target) {
        Ok(decision) => pgrx::JsonB(serde_json::json!({
            "agent_name": decision.agent.name,
            "confidence": decision.confidence,
            "estimated_cost": decision.estimated_cost,
            "estimated_latency_ms": decision.estimated_latency_ms,
            "reasoning": decision.reasoning,
        })),
        Err(e) => pgrx::JsonB(serde_json::json!({
            "error": format!("{:?}", e),
        })),
    }
}
```

### Phase 5: Semantic Routes (Week 10-11)

```rust
// src/routing/semantic_routes.rs

pub struct SemanticRoutes {
    routes: DashMap<String, SemanticRoute>,
    index: HnswIndex,
}

#[derive(Debug, Clone)]
pub struct SemanticRoute {
    pub name: String,
    pub description: String,
    pub embedding: Vec<f32>,
    pub target_agent: String,
    pub priority: i32,
    pub conditions: Option<RouteConditions>,
}

#[derive(Debug, Clone)]
pub struct RouteConditions {
    pub time_range: Option<(chrono::NaiveTime, chrono::NaiveTime)>,
    pub user_tier: Option<Vec<String>>,
    pub rate_limit: Option<u32>,
}

impl SemanticRoutes {
    pub fn add_route(&self, route: SemanticRoute) {
        self.index.insert(&route.name, &route.embedding);
        self.routes.insert(route.name.clone(), route);
    }

    pub fn match_route(&self, query_embedding: &[f32], k: usize) -> Vec<(SemanticRoute, f32)> {
        let results = self.index.search(query_embedding, k);

        results.iter()
            .filter_map(|(name, score)| {
                self.routes.get(name.as_str())
                    .map(|r| (r.clone(), *score))
            })
            .collect()
    }
}

#[pg_extern]
fn ruvector_create_route(
    name: &str,
    description: &str,
    target_agent: &str,
    priority: default!(i32, 0),
    embedding: default!(Option<Vec<f32>>, "NULL"),
) -> bool {
    let routes = get_semantic_routes();

    let embedding = embedding.unwrap_or_else(|| get_embedding(description));

    let route = SemanticRoute {
        name: name.to_string(),
        description: description.to_string(),
        embedding,
        target_agent: target_agent.to_string(),
        priority,
        conditions: None,
    };

    routes.add_route(route);
    true
}

#[pg_extern]
fn ruvector_semantic_route(
    query: &str,
    top_k: default!(i32, 3),
) -> TableIterator<'static, (
    name!(route_name, String),
    name!(similarity, f32),
    name!(target_agent, String),
    name!(confidence, f32),
)> {
    let routes = get_semantic_routes();
    let embedding = get_embedding(query);

    let matches = routes.match_route(&embedding, top_k as usize);

    let results: Vec<_> = matches.into_iter()
        .map(|(route, similarity)| {
            let confidence = similarity * (route.priority as f32 + 1.0) / 10.0;
            (route.name, similarity, route.target_agent, confidence.min(1.0))
        })
        .collect();

    TableIterator::new(results)
}
```

### Phase 6: Cost Optimizer (Week 12)

```rust
// src/routing/cost_optimizer.rs

pub struct CostOptimizer {
    budget_tracker: BudgetTracker,
    usage_history: UsageHistory,
}

#[derive(Debug, Clone)]
pub struct BudgetAllocation {
    pub agent_budgets: HashMap<String, f64>,
    pub total_budget: f64,
    pub period: chrono::Duration,
}

impl CostOptimizer {
    /// Optimize budget allocation across agents
    pub fn optimize_budget(
        &self,
        total_budget: f64,
        quality_threshold: f64,
        latency_threshold: f64,
        period_days: i64,
    ) -> BudgetAllocation {
        let agents = get_agent_registry().list_active();
        let history = self.usage_history.get_period(period_days);

        // Calculate value score for each agent
        let agent_values: HashMap<String, f64> = agents.iter()
            .filter(|a| {
                a.performance.quality_score >= quality_threshold &&
                a.performance.avg_latency_ms <= latency_threshold
            })
            .map(|a| {
                let historical_usage = history.get(&a.name).map(|h| h.request_count).unwrap_or(1);
                let quality = a.performance.quality_score;
                let cost_efficiency = 1.0 / (a.cost_model.cost_per_1k_tokens.unwrap_or(0.01) + 0.001);

                let value = quality * cost_efficiency * (historical_usage as f64).ln();
                (a.name.clone(), value)
            })
            .collect();

        // Allocate budget proportionally to value
        let total_value: f64 = agent_values.values().sum();
        let agent_budgets: HashMap<String, f64> = agent_values.iter()
            .map(|(name, value)| {
                let allocation = (value / total_value) * total_budget;
                (name.clone(), allocation)
            })
            .collect();

        BudgetAllocation {
            agent_budgets,
            total_budget,
            period: chrono::Duration::days(period_days),
        }
    }

    /// Check if request fits within budget
    pub fn check_budget(&self, agent: &str, estimated_cost: f64) -> bool {
        self.budget_tracker.remaining(agent) >= estimated_cost
    }

    /// Record usage
    pub fn record_usage(&self, agent: &str, actual_cost: f64, success: bool, latency_ms: f64) {
        self.budget_tracker.deduct(agent, actual_cost);
        self.usage_history.record(agent, actual_cost, success, latency_ms);
    }
}

#[pg_extern]
fn ruvector_optimize_budget(
    monthly_budget: f64,
    quality_threshold: default!(f64, 0.8),
    latency_threshold_ms: default!(f64, 5000.0),
) -> pgrx::JsonB {
    let optimizer = get_cost_optimizer();

    let allocation = optimizer.optimize_budget(
        monthly_budget,
        quality_threshold,
        latency_threshold_ms,
        30,
    );

    pgrx::JsonB(serde_json::json!({
        "allocations": allocation.agent_budgets,
        "total_budget": allocation.total_budget,
        "period_days": 30,
    }))
}

#[pg_extern]
fn ruvector_routing_analytics(
    time_range: default!(&str, "'7 days'"),
    group_by: default!(&str, "'agent'"),
) -> TableIterator<'static, (
    name!(agent, String),
    name!(total_requests, i64),
    name!(total_cost, f64),
    name!(avg_latency_ms, f64),
    name!(success_rate, f64),
)> {
    let optimizer = get_cost_optimizer();
    let days = parse_time_range(time_range);

    let stats = optimizer.usage_history.aggregate(days, group_by);

    TableIterator::new(stats)
}
```

## Benchmarks

| Operation | Input Size | Time (μs) | Memory |
|-----------|------------|-----------|--------|
| FastGRNN step | 768-dim | 45 | 1KB |
| Route classification | 768-dim | 120 | 4KB |
| Semantic route match (1K routes) | 768-dim | 250 | 8KB |
| Full routing decision | 768-dim | 500 | 16KB |

## Dependencies

```toml
[dependencies]
# Link to ruvector-tiny-dancer
ruvector-tiny-dancer-core = { path = "../ruvector-tiny-dancer-core", optional = true }

# SIMD
simsimd = "5.9"

# Time handling
chrono = "0.4"

# Concurrent collections
dashmap = "6.0"
```

## Feature Flags

```toml
[features]
routing = []
routing-fastgrnn = ["routing"]
routing-semantic = ["routing", "index-hnsw"]
routing-optimizer = ["routing"]
routing-all = ["routing-fastgrnn", "routing-semantic", "routing-optimizer"]
```
