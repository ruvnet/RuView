// PostgreSQL Operators for Tiny Dancer Routing
//
// SQL functions for agent registration, routing, and management.

use pgrx::prelude::*;
use pgrx::JsonB;
use serde_json::json;
use std::sync::OnceLock;

use super::agents::{Agent, AgentRegistry, AgentType};
use super::router::{OptimizationTarget, Router, RoutingConstraints};

// Global agent registry and router
static AGENT_REGISTRY: OnceLock<AgentRegistry> = OnceLock::new();
static ROUTER: OnceLock<Router> = OnceLock::new();

/// Initialize the global registry and router
fn init_router() -> &'static Router {
    ROUTER.get_or_init(|| {
        let _registry = AGENT_REGISTRY.get_or_init(AgentRegistry::new);
        Router::with_registry(std::sync::Arc::new(AgentRegistry::new()))
    })
}

/// Get the global agent registry
fn get_registry() -> &'static AgentRegistry {
    AGENT_REGISTRY.get_or_init(AgentRegistry::new)
}

/// Register a new AI agent
///
/// # Arguments
/// * `name` - Unique agent identifier
/// * `agent_type` - Type of agent (llm, embedding, specialized, etc.)
/// * `capabilities` - Array of capability strings
/// * `cost_per_request` - Cost per request in dollars
/// * `avg_latency_ms` - Average latency in milliseconds
/// * `quality_score` - Quality score (0-1)
///
/// # Example
/// ```sql
/// SELECT ruvector_register_agent(
///     'gpt-4',
///     'llm',
///     ARRAY['code_generation', 'translation'],
///     0.03,
///     500.0,
///     0.95
/// );
/// ```
#[pg_extern]
fn ruvector_register_agent(
    name: String,
    agent_type: String,
    capabilities: Vec<String>,
    cost_per_request: f32,
    avg_latency_ms: f32,
    quality_score: f32,
) -> Result<bool, String> {
    let registry = get_registry();

    let mut agent = Agent::new(name.clone(), AgentType::from_str(&agent_type), capabilities);

    agent.cost_model.per_request = cost_per_request;
    agent.performance.avg_latency_ms = avg_latency_ms;
    agent.performance.quality_score = quality_score;

    registry.register(agent)?;
    Ok(true)
}

/// Register an agent with full configuration
///
/// # Arguments
/// * `config` - JSONB configuration with all agent properties
///
/// # Example
/// ```sql
/// SELECT ruvector_register_agent_full('{
///     "name": "gpt-4",
///     "agent_type": "llm",
///     "capabilities": ["code_generation", "translation"],
///     "cost_model": {
///         "per_request": 0.03,
///         "per_token": 0.00006
///     },
///     "performance": {
///         "avg_latency_ms": 500.0,
///         "quality_score": 0.95,
///         "success_rate": 0.99
///     }
/// }'::jsonb);
/// ```
#[pg_extern]
fn ruvector_register_agent_full(config: JsonB) -> Result<bool, String> {
    let registry = get_registry();

    let agent: Agent = serde_json::from_value(config.0)
        .map_err(|e| format!("Invalid agent configuration: {}", e))?;

    registry.register(agent)?;
    Ok(true)
}

/// Update an existing agent's performance metrics
///
/// # Arguments
/// * `name` - Agent name
/// * `latency_ms` - Observed latency
/// * `success` - Whether the request succeeded
/// * `quality` - Optional quality score for this request
///
/// # Example
/// ```sql
/// SELECT ruvector_update_agent_metrics('gpt-4', 450.0, true, 0.92);
/// ```
#[pg_extern]
fn ruvector_update_agent_metrics(
    name: String,
    latency_ms: f32,
    success: bool,
    quality: Option<f32>,
) -> Result<bool, String> {
    let registry = get_registry();

    let mut agent = registry
        .get(&name)
        .ok_or_else(|| format!("Agent '{}' not found", name))?;

    agent.update_metrics(latency_ms, success, quality);
    registry.update(agent)?;

    Ok(true)
}

/// Remove an agent from the registry
///
/// # Example
/// ```sql
/// SELECT ruvector_remove_agent('gpt-4');
/// ```
#[pg_extern]
fn ruvector_remove_agent(name: String) -> Result<bool, String> {
    let registry = get_registry();
    registry
        .remove(&name)
        .ok_or_else(|| format!("Agent '{}' not found", name))?;
    Ok(true)
}

/// Set an agent's active status
///
/// # Example
/// ```sql
/// SELECT ruvector_set_agent_active('gpt-4', false);
/// ```
#[pg_extern]
fn ruvector_set_agent_active(name: String, is_active: bool) -> Result<bool, String> {
    let registry = get_registry();

    let mut agent = registry
        .get(&name)
        .ok_or_else(|| format!("Agent '{}' not found", name))?;

    agent.is_active = is_active;
    registry.update(agent)?;

    Ok(true)
}

/// Route a request to the best agent
///
/// # Arguments
/// * `request_embedding` - Request embedding vector (384-dim)
/// * `optimize_for` - Optimization target: 'cost', 'latency', 'quality', 'balanced'
/// * `constraints` - Optional JSONB constraints object
///
/// # Example
/// ```sql
/// SELECT ruvector_route(
///     embedding,
///     'balanced',
///     '{"max_cost": 0.1, "min_quality": 0.8}'::jsonb
/// )
/// FROM request_embeddings
/// WHERE id = 123;
/// ```
#[pg_extern]
fn ruvector_route(
    request_embedding: Vec<f32>,
    optimize_for: default!(String, "'balanced'"),
    constraints: default!(Option<JsonB>, "NULL"),
) -> Result<JsonB, String> {
    init_router(); // Ensure router is initialized

    let target = OptimizationTarget::from_str(&optimize_for);

    let routing_constraints = if let Some(JsonB(json_val)) = constraints {
        serde_json::from_value(json_val).map_err(|e| format!("Invalid constraints: {}", e))?
    } else {
        RoutingConstraints::default()
    };

    // Get router with proper registry
    let registry = get_registry();
    let router = Router::with_registry(std::sync::Arc::new(AgentRegistry::new()));

    // Copy agents from global registry to router's registry
    for agent in registry.list_all() {
        router.registry().register(agent).ok();
    }

    let decision = router.route(&request_embedding, &routing_constraints, target)?;

    let result = json!({
        "agent_name": decision.agent_name,
        "confidence": decision.confidence,
        "estimated_cost": decision.estimated_cost,
        "estimated_latency_ms": decision.estimated_latency_ms,
        "expected_quality": decision.expected_quality,
        "similarity_score": decision.similarity_score,
        "reasoning": decision.reasoning,
        "alternatives": decision.alternatives,
    });

    Ok(JsonB(result))
}

/// List all registered agents
///
/// # Example
/// ```sql
/// SELECT * FROM ruvector_list_agents();
/// ```
#[pg_extern]
fn ruvector_list_agents() -> TableIterator<
    'static,
    (
        name!(name, String),
        name!(agent_type, String),
        name!(capabilities, Vec<String>),
        name!(cost_per_request, f32),
        name!(avg_latency_ms, f32),
        name!(quality_score, f32),
        name!(success_rate, f32),
        name!(total_requests, i64),
        name!(is_active, bool),
    ),
> {
    let registry = get_registry();
    let agents = registry.list_all();

    TableIterator::new(
        agents
            .into_iter()
            .map(|agent| {
                (
                    agent.name,
                    agent.agent_type.as_str().to_string(),
                    agent.capabilities,
                    agent.cost_model.per_request,
                    agent.performance.avg_latency_ms,
                    agent.performance.quality_score,
                    agent.performance.success_rate,
                    agent.performance.total_requests as i64,
                    agent.is_active,
                )
            })
            .collect::<Vec<_>>(),
    )
}

/// Get detailed information about a specific agent
///
/// # Example
/// ```sql
/// SELECT ruvector_get_agent('gpt-4');
/// ```
#[pg_extern]
fn ruvector_get_agent(name: String) -> Result<JsonB, String> {
    let registry = get_registry();

    let agent = registry
        .get(&name)
        .ok_or_else(|| format!("Agent '{}' not found", name))?;

    let result = serde_json::to_value(&agent).map_err(|e| format!("Serialization error: {}", e))?;

    Ok(JsonB(result))
}

/// Find agents by capability
///
/// # Example
/// ```sql
/// SELECT * FROM ruvector_find_agents_by_capability('code_generation', 5);
/// ```
#[pg_extern]
fn ruvector_find_agents_by_capability(
    capability: String,
    limit: default!(i32, 10),
) -> TableIterator<
    'static,
    (
        name!(name, String),
        name!(quality_score, f32),
        name!(avg_latency_ms, f32),
        name!(cost_per_request, f32),
    ),
> {
    let registry = get_registry();
    let agents = registry.find_by_capability(&capability, limit as usize);

    TableIterator::new(
        agents
            .into_iter()
            .map(|agent| {
                (
                    agent.name,
                    agent.performance.quality_score,
                    agent.performance.avg_latency_ms,
                    agent.cost_model.per_request,
                )
            })
            .collect::<Vec<_>>(),
    )
}

/// Get routing statistics
///
/// # Example
/// ```sql
/// SELECT ruvector_routing_stats();
/// ```
#[pg_extern]
fn ruvector_routing_stats() -> JsonB {
    let registry = get_registry();

    let total_agents = registry.count();
    let active_agents = registry.count_active();

    let agents = registry.list_all();

    let total_requests: u64 = agents.iter().map(|a| a.performance.total_requests).sum();
    let avg_quality: f32 = if !agents.is_empty() {
        agents
            .iter()
            .map(|a| a.performance.quality_score)
            .sum::<f32>()
            / agents.len() as f32
    } else {
        0.0
    };

    let result = json!({
        "total_agents": total_agents,
        "active_agents": active_agents,
        "total_requests": total_requests,
        "average_quality": avg_quality,
    });

    JsonB(result)
}

/// Clear all agents (for testing)
#[pg_extern]
fn ruvector_clear_agents() -> bool {
    let registry = get_registry();
    registry.clear();
    true
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_register_agent() {
        ruvector_clear_agents();

        let result = ruvector_register_agent(
            "test-agent".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.05,
            200.0,
            0.85,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), true);

        // Verify agent was registered
        let agent = ruvector_get_agent("test-agent".to_string());
        assert!(agent.is_ok());
    }

    #[pg_test]
    fn test_register_duplicate_agent() {
        ruvector_clear_agents();

        ruvector_register_agent(
            "test-agent".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.05,
            200.0,
            0.85,
        )
        .unwrap();

        // Try to register again
        let result = ruvector_register_agent(
            "test-agent".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.05,
            200.0,
            0.85,
        );

        assert!(result.is_err());
    }

    #[pg_test]
    fn test_update_agent_metrics() {
        ruvector_clear_agents();

        ruvector_register_agent(
            "test-agent".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.05,
            200.0,
            0.85,
        )
        .unwrap();

        let result =
            ruvector_update_agent_metrics("test-agent".to_string(), 150.0, true, Some(0.9));

        assert!(result.is_ok());
    }

    #[pg_test]
    fn test_remove_agent() {
        ruvector_clear_agents();

        ruvector_register_agent(
            "test-agent".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.05,
            200.0,
            0.85,
        )
        .unwrap();

        let result = ruvector_remove_agent("test-agent".to_string());
        assert!(result.is_ok());

        // Verify agent was removed
        let agent = ruvector_get_agent("test-agent".to_string());
        assert!(agent.is_err());
    }

    #[pg_test]
    fn test_set_agent_active() {
        ruvector_clear_agents();

        ruvector_register_agent(
            "test-agent".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.05,
            200.0,
            0.85,
        )
        .unwrap();

        let result = ruvector_set_agent_active("test-agent".to_string(), false);
        assert!(result.is_ok());

        let agent_json = ruvector_get_agent("test-agent".to_string()).unwrap();
        let agent: Agent = serde_json::from_value(agent_json.0).unwrap();
        assert_eq!(agent.is_active, false);
    }

    #[pg_test]
    fn test_list_agents() {
        ruvector_clear_agents();

        ruvector_register_agent(
            "agent1".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.05,
            200.0,
            0.85,
        )
        .unwrap();

        ruvector_register_agent(
            "agent2".to_string(),
            "embedding".to_string(),
            vec!["similarity".to_string()],
            0.01,
            50.0,
            0.90,
        )
        .unwrap();

        let agents: Vec<_> = ruvector_list_agents().collect();
        assert_eq!(agents.len(), 2);
    }

    #[pg_test]
    fn test_find_agents_by_capability() {
        ruvector_clear_agents();

        ruvector_register_agent(
            "coder1".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.05,
            200.0,
            0.85,
        )
        .unwrap();

        ruvector_register_agent(
            "coder2".to_string(),
            "llm".to_string(),
            vec!["coding".to_string(), "translation".to_string()],
            0.08,
            250.0,
            0.90,
        )
        .unwrap();

        ruvector_register_agent(
            "translator".to_string(),
            "llm".to_string(),
            vec!["translation".to_string()],
            0.03,
            150.0,
            0.80,
        )
        .unwrap();

        let coders: Vec<_> = ruvector_find_agents_by_capability("coding".to_string(), 10).collect();
        assert_eq!(coders.len(), 2);
    }

    #[pg_test]
    fn test_routing_stats() {
        ruvector_clear_agents();

        ruvector_register_agent(
            "agent1".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.05,
            200.0,
            0.85,
        )
        .unwrap();

        let stats = ruvector_routing_stats();
        let stats_obj: serde_json::Value = stats.0;

        assert_eq!(stats_obj["total_agents"], 1);
        assert_eq!(stats_obj["active_agents"], 1);
    }

    #[pg_test]
    fn test_route_basic() {
        ruvector_clear_agents();

        ruvector_register_agent(
            "cheap-agent".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.01,
            200.0,
            0.70,
        )
        .unwrap();

        ruvector_register_agent(
            "expensive-agent".to_string(),
            "llm".to_string(),
            vec!["coding".to_string()],
            0.10,
            200.0,
            0.95,
        )
        .unwrap();

        let embedding = vec![0.1; 384];

        // Route optimizing for cost
        let result = ruvector_route(embedding.clone(), "cost".to_string(), None);
        assert!(result.is_ok());

        let decision = result.unwrap().0;
        assert_eq!(decision["agent_name"], "cheap-agent");
    }
}
