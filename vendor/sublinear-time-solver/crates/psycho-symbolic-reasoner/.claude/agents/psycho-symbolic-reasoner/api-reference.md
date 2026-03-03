# Psycho-Symbolic Reasoner API Reference

## Overview

The Psycho-Symbolic Reasoner provides five main tools for advanced reasoning, knowledge management, and analysis. All tools use the MCP (Model Context Protocol) interface and return structured JSON responses.

## Tool Reference

### 1. `mcp__psycho-symbolic-reasoner__reason`

Performs comprehensive psycho-symbolic reasoning analysis on provided inputs.

#### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | ✅ | - | The reasoning query or problem to analyze |
| `context` | object | ❌ | `{}` | Additional context for reasoning |
| `reasoning_type` | enum | ❌ | `"deductive"` | Type of reasoning to perform |
| `psychological_factors` | array | ❌ | `[]` | Psychological aspects to consider |
| `confidence_threshold` | number | ❌ | `0.7` | Minimum confidence level (0-1) |

#### Reasoning Types

- **`deductive`**: Logical deduction from premises to conclusions
- **`inductive`**: Pattern recognition and generalization from examples
- **`abductive`**: Best explanation inference (hypothesis formation)
- **`analogical`**: Reasoning by analogy and similarity
- **`causal`**: Cause-and-effect reasoning chains

#### Response Schema

```typescript
{
  session_id: string;           // Unique session identifier
  query: string;                // Original query
  reasoning_type: string;       // Type of reasoning used
  conclusion: string;           // Main reasoning conclusion
  confidence: number;           // Confidence score (0-1)
  evidence: Array<{             // Supporting evidence
    type: string;
    description: string;
    strength: number;
    source?: string;
  }>;
  reasoning_chain: Array<{      // Step-by-step reasoning
    step: number;
    operation: string;
    input: string;
    output: string;
    confidence: number;
  }>;
  psychological_insights: Array<{
    factor: string;
    impact: string;
    relevance: number;
  }>;
  limitations: string[];        // Known limitations of the analysis
  recommendations?: string[];   // Action recommendations (if applicable)
  metadata: {
    processing_time: number;
    knowledge_sources_used: number;
    complexity_score: number;
  };
}
```

#### Example Usage

```typescript
const result = await mcp__psycho_symbolic_reasoner__reason({
  query: "Why do people procrastinate on important tasks?",
  reasoning_type: "causal",
  psychological_factors: ["temporal_discounting", "anxiety", "perfectionism"],
  confidence_threshold: 0.8,
  context: {
    domain: "behavioral_psychology",
    target_audience: "general_population"
  }
});
```

### 2. `mcp__psycho-symbolic-reasoner__knowledge_graph_query`

Queries the internal knowledge graph for relevant information and relationships.

#### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `query` | string | ✅ | - | Query to search in the knowledge graph |
| `entity_types` | array | ❌ | `[]` | Specific entity types to filter |
| `relationship_types` | array | ❌ | `[]` | Relationship types to include |
| `depth` | number | ❌ | `3` | Maximum traversal depth |
| `limit` | number | ❌ | `50` | Maximum results to return |

#### Entity Types

- `concept`: Abstract concepts and ideas
- `theory`: Psychological theories and frameworks
- `researcher`: People who contributed to the field
- `study`: Research studies and experiments
- `phenomenon`: Observable psychological phenomena
- `technique`: Methods and techniques
- `principle`: Fundamental principles and laws

#### Relationship Types

- `relates_to`: General relationship
- `influences`: Causal influence
- `developed_by`: Creator/developer relationship
- `applies_to`: Application relationship
- `contradicts`: Contradictory relationship
- `supports`: Supporting relationship
- `is_example_of`: Instance relationship

#### Response Schema

```typescript
{
  query: string;                // Original query
  entities: Array<{             // Found entities
    id: string;
    name: string;
    type: string;
    properties: object;
    relevance_score: number;
    description?: string;
  }>;
  relationships: Array<{        // Found relationships
    id: string;
    source_id: string;
    target_id: string;
    type: string;
    properties: object;
    strength: number;
  }>;
  graph_statistics: {
    total_entities: number;
    total_relationships: number;
    query_coverage: number;
    average_relevance: number;
  };
  missing_relationships?: Array<{
    expected_source: string;
    expected_target: string;
    expected_type: string;
    confidence: number;
  }>;
  suggestions: string[];        // Query improvement suggestions
}
```

#### Example Usage

```typescript
const knowledge = await mcp__psycho_symbolic_reasoner__knowledge_graph_query({
  query: "cognitive bias decision making",
  entity_types: ["concept", "theory", "phenomenon"],
  relationship_types: ["influences", "applies_to"],
  depth: 2,
  limit: 20
});
```

### 3. `mcp__psycho-symbolic-reasoner__add_knowledge`

Adds new facts, relationships, or entities to the knowledge base.

#### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `knowledge_type` | enum | ✅ | - | Type of knowledge to add |
| `content` | object | ✅ | - | Knowledge content structure |
| `source` | string | ❌ | `null` | Source of the knowledge |
| `confidence` | number | ❌ | `0.8` | Confidence in the knowledge (0-1) |
| `tags` | array | ❌ | `[]` | Tags for categorization |

#### Knowledge Types

- **`fact`**: Simple factual statement
- **`relationship`**: Connection between entities
- **`rule`**: Logical rule or principle
- **`concept`**: New concept definition
- **`entity`**: New entity (person, theory, etc.)

#### Content Schemas

**Fact Content:**
```typescript
{
  statement: string;            // The factual statement
  domain: string;               // Domain of knowledge
  evidence_level: string;       // "strong" | "moderate" | "weak"
  context?: object;             // Additional context
}
```

**Relationship Content:**
```typescript
{
  subject: string;              // Source entity
  predicate: string;            // Relationship type
  object: string;               // Target entity
  properties?: object;          // Additional properties
}
```

**Rule Content:**
```typescript
{
  condition: string;            // Rule condition
  conclusion: string;           // Rule conclusion
  strength: string;             // "strong" | "moderate" | "weak"
  domain: string;               // Application domain
  exceptions?: string[];        // Known exceptions
}
```

**Concept Content:**
```typescript
{
  name: string;                 // Concept name
  definition: string;           // Concept definition
  domain: string;               // Field/domain
  properties?: object;          // Additional properties
  related_concepts?: string[];  // Related concepts
}
```

**Entity Content:**
```typescript
{
  name: string;                 // Entity name
  type: string;                 // Entity type
  properties: object;           // Entity properties
  description?: string;         // Description
}
```

#### Response Schema

```typescript
{
  id: string;                   // Assigned knowledge ID
  knowledge_type: string;       // Type of knowledge added
  status: "added" | "updated" | "merged";
  confidence: number;           // Final confidence score
  validation_results: {
    consistency_check: boolean;
    conflict_detection: string[];
    integration_quality: number;
  };
  related_knowledge: string[];  // IDs of related knowledge items
  suggestions?: string[];       // Improvement suggestions
}
```

#### Example Usage

```typescript
const result = await mcp__psycho_symbolic_reasoner__add_knowledge({
  knowledge_type: "concept",
  content: {
    name: "cognitive_load_theory",
    definition: "Theory explaining how the brain processes information in working memory",
    domain: "educational_psychology",
    properties: {
      author: "John Sweller",
      year: 1988,
      components: ["intrinsic", "extraneous", "germane"]
    }
  },
  source: "Sweller, J. (1988). Cognitive load during problem solving",
  confidence: 0.95,
  tags: ["learning", "memory", "education"]
});
```

### 4. `mcp__psycho-symbolic-reasoner__analyze_reasoning_path`

Analyzes the reasoning chain and provides insights into the reasoning process.

#### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `reasoning_id` | string | ✅ | - | ID of the reasoning session to analyze |
| `analysis_type` | enum | ❌ | `"logical_validity"` | Type of analysis to perform |
| `include_suggestions` | boolean | ❌ | `false` | Include improvement suggestions |
| `detailed_breakdown` | boolean | ❌ | `false` | Provide step-by-step analysis |

#### Analysis Types

- **`logical_validity`**: Check logical soundness of reasoning
- **`psychological_plausibility`**: Assess psychological realism
- **`bias_detection`**: Identify potential cognitive biases
- **`confidence_calibration`**: Evaluate confidence accuracy
- **`knowledge_gaps`**: Identify missing information
- **`alternative_paths`**: Explore alternative reasoning routes

#### Response Schema

```typescript
{
  reasoning_id: string;         // Original reasoning session ID
  analysis_type: string;        // Type of analysis performed
  overall_score: number;        // Overall quality score (0-1)

  validity_assessment?: {       // For logical_validity analysis
    logical_consistency: number;
    premise_validity: number;
    conclusion_support: number;
    fallacies_detected: string[];
  };

  psychological_assessment?: {  // For psychological_plausibility analysis
    cognitive_realism: number;
    behavioral_accuracy: number;
    empirical_support: number;
    psychological_mechanisms: string[];
  };

  bias_assessment?: {          // For bias_detection analysis
    detected_biases: Array<{
      type: string;
      confidence: number;
      evidence: string;
      impact: string;
    }>;
    bias_risk_score: number;
    mitigation_strategies: string[];
  };

  confidence_assessment?: {    // For confidence_calibration analysis
    calibration_score: number;
    overconfidence_indicators: string[];
    underconfidence_indicators: string[];
    reliability_estimate: number;
  };

  knowledge_assessment?: {     // For knowledge_gaps analysis
    identified_gaps: Array<{
      area: string;
      importance: number;
      suggested_sources: string[];
    }>;
    completeness_score: number;
    critical_missing_info: string[];
  };

  alternative_paths?: {        // For alternative_paths analysis
    paths: Array<{
      path_id: string;
      reasoning_type: string;
      confidence: number;
      key_differences: string[];
    }>;
    comparison_matrix: object;
    recommended_path: string;
  };

  suggestions?: string[];      // Improvement suggestions
  detailed_breakdown?: Array<{  // Step-by-step analysis
    step: number;
    analysis: string;
    score: number;
    issues: string[];
    recommendations: string[];
  }>;
}
```

#### Example Usage

```typescript
const analysis = await mcp__psycho_symbolic_reasoner__analyze_reasoning_path({
  reasoning_id: "session_12345",
  analysis_type: "bias_detection",
  include_suggestions: true,
  detailed_breakdown: true
});
```

### 5. `mcp__psycho-symbolic-reasoner__health_check`

Performs system health checks and diagnostics.

#### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `check_type` | enum | ❌ | `"basic"` | Type of health check |
| `include_metrics` | boolean | ❌ | `false` | Include performance metrics |
| `verbose` | boolean | ❌ | `false` | Detailed output |

#### Check Types

- **`basic`**: Basic system status
- **`comprehensive`**: Full system diagnostic
- **`performance`**: Performance metrics only
- **`knowledge_base`**: Knowledge base integrity
- **`reasoning_engine`**: Reasoning engine status

#### Response Schema

```typescript
{
  status: "healthy" | "warning" | "error";
  timestamp: string;
  check_type: string;

  system_info: {
    version: string;
    uptime: number;
    memory_usage: number;
    cpu_usage: number;
  };

  knowledge_base_status: {
    total_entities: number;
    total_relationships: number;
    integrity_score: number;
    last_updated: string;
    inconsistencies?: string[];
  };

  reasoning_engine_status: {
    active_sessions: number;
    average_response_time: number;
    success_rate: number;
    error_rate: number;
    last_error?: string;
  };

  performance_metrics?: {       // If include_metrics = true
    queries_per_minute: number;
    average_reasoning_time: number;
    knowledge_retrieval_time: number;
    cache_hit_rate: number;
    confidence_accuracy: number;
  };

  capabilities: string[];       // Available capabilities
  warnings?: string[];          // System warnings
  errors?: string[];            // System errors
  recommendations?: string[];   // System optimization suggestions
}
```

#### Example Usage

```typescript
const health = await mcp__psycho_symbolic_reasoner__health_check({
  check_type: "comprehensive",
  include_metrics: true,
  verbose: true
});
```

## Error Handling

### Common Error Codes

| Code | Description | Resolution |
|------|-------------|------------|
| `INSUFFICIENT_KNOWLEDGE` | Not enough knowledge for reliable reasoning | Add relevant knowledge or lower confidence threshold |
| `REASONING_TIMEOUT` | Reasoning process timed out | Simplify query or increase timeout |
| `INVALID_QUERY` | Query format is invalid | Check query syntax and parameters |
| `KNOWLEDGE_CONFLICT` | Conflicting information in knowledge base | Resolve conflicts or specify preferences |
| `SYSTEM_OVERLOAD` | System is at capacity | Retry later or reduce query complexity |
| `VALIDATION_FAILED` | Input validation failed | Check parameter types and ranges |

### Error Response Schema

```typescript
{
  error: {
    code: string;               // Error code
    message: string;            // Human-readable message
    details?: object;           // Additional error details
    suggestions?: string[];     // Resolution suggestions
    retry_after?: number;       // Seconds to wait before retry
  };
  request_id: string;          // Request identifier for debugging
  timestamp: string;           // Error timestamp
}
```

## Rate Limits and Performance

### Rate Limits

| Operation | Limit | Window |
|-----------|-------|--------|
| Reasoning queries | 100 | 1 hour |
| Knowledge additions | 500 | 1 hour |
| Graph queries | 200 | 1 hour |
| Health checks | 20 | 1 minute |

### Performance Guidelines

- **Query optimization**: Use specific entity/relationship types for faster graph queries
- **Batch operations**: Group related knowledge additions
- **Caching**: Repeated queries with same parameters are cached for 5 minutes
- **Timeout limits**: Complex reasoning operations timeout after 30 seconds by default

## Authentication and Security

The psycho-symbolic-reasoner uses session-based authentication through Claude's MCP protocol. All requests are automatically authenticated when using the Claude interface.

### Security Features

- Input sanitization and validation
- Knowledge source verification
- Access logging and audit trails
- Secure knowledge isolation between sessions

## Versioning

The API follows semantic versioning. Current version: `1.0.0`

### Version Compatibility

- `1.x.x`: Backward compatible feature additions
- `2.x.x`: Breaking changes (with migration guide)
- `x.x.1`: Bug fixes and patches

For detailed changelog and migration guides, see the project documentation.