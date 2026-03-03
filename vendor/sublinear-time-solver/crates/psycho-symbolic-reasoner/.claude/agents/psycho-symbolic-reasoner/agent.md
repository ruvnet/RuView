# Psycho-Symbolic Reasoner Agent

## Overview

The **Psycho-Symbolic Reasoner** is an advanced AI agent that combines symbolic reasoning with psychological context understanding to perform sophisticated analysis and knowledge processing. This agent leverages structured knowledge graphs and psychological insights to provide nuanced reasoning capabilities for complex problem-solving scenarios.

## Agent Configuration

```yaml
name: "Psycho-Symbolic Reasoner"
description: "AI agent that combines symbolic reasoning with psychological context understanding"
version: "1.0.0"
capabilities:
  - symbolic_reasoning
  - psychological_analysis
  - knowledge_graph_processing
  - reasoning_chain_analysis
  - contextual_understanding
```

## MCP Server Configuration

To use this agent, you must first add the psycho-symbolic-reasoner MCP server to your Claude configuration:

```bash
# Add the MCP server
claude mcp add psycho-symbolic-reasoner npx psycho-symbolic-reasoner mcp start

# Verify the server is running
claude mcp list
```

### Server Details
- **Server Name**: `psycho-symbolic-reasoner`
- **Protocol**: MCP (Model Context Protocol)
- **Transport**: stdio
- **Capabilities**: Reasoning analysis, knowledge graph operations, psychological context processing

## Available Tools

### 1. `mcp__psycho-symbolic-reasoner__reason`
Performs comprehensive psycho-symbolic reasoning analysis on provided inputs.

**Parameters:**
- `query` (string, required): The reasoning query or problem to analyze
- `context` (object, optional): Additional context for reasoning
- `reasoning_type` (enum, optional): Type of reasoning to perform
  - `deductive`: Logical deduction from premises
  - `inductive`: Pattern recognition and generalization
  - `abductive`: Best explanation inference
  - `analogical`: Reasoning by analogy
  - `causal`: Cause-and-effect reasoning
- `psychological_factors` (array, optional): Psychological aspects to consider
- `confidence_threshold` (number, optional): Minimum confidence level (0-1)

**Usage Example:**
```typescript
await mcp__psycho_symbolic_reasoner__reason({
  query: "What are the implications of cognitive bias in decision-making processes?",
  reasoning_type: "causal",
  psychological_factors: ["cognitive_bias", "decision_making", "heuristics"],
  confidence_threshold: 0.8
});
```

### 2. `mcp__psycho-symbolic-reasoner__knowledge_graph_query`
Queries the internal knowledge graph for relevant information and relationships.

**Parameters:**
- `query` (string, required): Query to search in the knowledge graph
- `entity_types` (array, optional): Specific entity types to filter
- `relationship_types` (array, optional): Relationship types to include
- `depth` (number, optional): Maximum traversal depth (default: 3)
- `limit` (number, optional): Maximum results to return (default: 50)

**Usage Example:**
```typescript
await mcp__psycho_symbolic_reasoner__knowledge_graph_query({
  query: "psychological theories related to reasoning",
  entity_types: ["theory", "concept", "researcher"],
  relationship_types: ["relates_to", "developed_by", "applies_to"],
  depth: 2,
  limit: 20
});
```

### 3. `mcp__psycho-symbolic-reasoner__add_knowledge`
Adds new facts, relationships, or entities to the knowledge base.

**Parameters:**
- `knowledge_type` (enum, required): Type of knowledge to add
  - `fact`: Simple factual statement
  - `relationship`: Connection between entities
  - `rule`: Logical rule or principle
  - `concept`: New concept definition
- `content` (object, required): Knowledge content structure
- `source` (string, optional): Source of the knowledge
- `confidence` (number, optional): Confidence in the knowledge (0-1)
- `tags` (array, optional): Tags for categorization

**Usage Example:**
```typescript
await mcp__psycho_symbolic_reasoner__add_knowledge({
  knowledge_type: "relationship",
  content: {
    subject: "cognitive_load_theory",
    predicate: "influences",
    object: "learning_effectiveness",
    properties: {
      strength: "strong",
      context: "educational_psychology"
    }
  },
  source: "Sweller, J. (1988)",
  confidence: 0.9,
  tags: ["psychology", "learning", "cognition"]
});
```

### 4. `mcp__psycho-symbolic-reasoner__analyze_reasoning_path`
Analyzes the reasoning chain and provides insights into the reasoning process.

**Parameters:**
- `reasoning_id` (string, required): ID of the reasoning session to analyze
- `analysis_type` (enum, optional): Type of analysis to perform
  - `logical_validity`: Check logical soundness
  - `psychological_plausibility`: Assess psychological realism
  - `bias_detection`: Identify potential biases
  - `confidence_calibration`: Evaluate confidence levels
- `include_suggestions` (boolean, optional): Include improvement suggestions
- `detailed_breakdown` (boolean, optional): Provide step-by-step analysis

**Usage Example:**
```typescript
await mcp__psycho_symbolic_reasoner__analyze_reasoning_path({
  reasoning_id: "reasoning_session_12345",
  analysis_type: "bias_detection",
  include_suggestions: true,
  detailed_breakdown: true
});
```

### 5. `mcp__psycho-symbolic-reasoner__health_check`
Performs system health checks and diagnostics.

**Parameters:**
- `check_type` (enum, optional): Type of health check
  - `basic`: Basic system status
  - `comprehensive`: Full system diagnostic
  - `performance`: Performance metrics
- `include_metrics` (boolean, optional): Include performance metrics
- `verbose` (boolean, optional): Detailed output

**Usage Example:**
```typescript
await mcp__psycho_symbolic_reasoner__health_check({
  check_type: "comprehensive",
  include_metrics: true,
  verbose: true
});
```

## Usage Patterns

### Basic Reasoning Flow
```typescript
// 1. Perform initial reasoning
const reasoning = await mcp__psycho_symbolic_reasoner__reason({
  query: "How does confirmation bias affect scientific research?",
  reasoning_type: "causal",
  psychological_factors: ["confirmation_bias", "scientific_method"]
});

// 2. Query related knowledge
const knowledge = await mcp__psycho_symbolic_reasoner__knowledge_graph_query({
  query: "confirmation bias research methodology",
  entity_types: ["bias", "methodology", "study"]
});

// 3. Analyze the reasoning process
const analysis = await mcp__psycho_symbolic_reasoner__analyze_reasoning_path({
  reasoning_id: reasoning.session_id,
  analysis_type: "bias_detection",
  include_suggestions: true
});
```

### Knowledge Building Flow
```typescript
// 1. Add domain knowledge
await mcp__psycho_symbolic_reasoner__add_knowledge({
  knowledge_type: "concept",
  content: {
    name: "metacognitive_awareness",
    definition: "Knowledge and understanding of one's own thought processes",
    domain: "cognitive_psychology"
  },
  source: "Flavell, J. H. (1976)",
  confidence: 0.95
});

// 2. Establish relationships
await mcp__psycho_symbolic_reasoner__add_knowledge({
  knowledge_type: "relationship",
  content: {
    subject: "metacognitive_awareness",
    predicate: "improves",
    object: "problem_solving_accuracy"
  }
});

// 3. Apply knowledge in reasoning
const result = await mcp__psycho_symbolic_reasoner__reason({
  query: "How can metacognitive training improve decision-making?",
  reasoning_type: "deductive"
});
```

## Best Practices

### 1. Reasoning Query Formulation
- **Be specific**: Clearly define the problem or question
- **Provide context**: Include relevant background information
- **Specify reasoning type**: Choose the most appropriate reasoning approach
- **Set confidence thresholds**: Define acceptable confidence levels

### 2. Knowledge Graph Management
- **Structured queries**: Use specific entity and relationship types
- **Incremental building**: Add knowledge systematically
- **Source attribution**: Always provide sources for added knowledge
- **Regular validation**: Check knowledge consistency

### 3. Analysis and Validation
- **Multi-perspective analysis**: Use different analysis types
- **Bias awareness**: Regularly check for reasoning biases
- **Confidence calibration**: Validate confidence levels
- **Iterative refinement**: Use analysis feedback for improvement

### 4. Performance Optimization
- **Limit query scope**: Use appropriate depth and result limits
- **Batch operations**: Group related knowledge additions
- **Cache frequently used knowledge**: Store common reasoning patterns
- **Monitor system health**: Regular health checks

## Error Handling

```typescript
try {
  const result = await mcp__psycho_symbolic_reasoner__reason({
    query: "complex reasoning query",
    reasoning_type: "deductive"
  });

  if (result.confidence < 0.7) {
    console.warn("Low confidence result, consider additional context");
  }
} catch (error) {
  if (error.code === 'INSUFFICIENT_KNOWLEDGE') {
    // Add relevant knowledge before retrying
    await mcp__psycho_symbolic_reasoner__add_knowledge({
      // ... knowledge addition
    });
  } else if (error.code === 'REASONING_TIMEOUT') {
    // Simplify query or increase timeout
    console.log("Reasoning timeout, consider simplifying the query");
  }
}
```

## Integration Examples

### With Claude Code SPARC Methodology
```typescript
// Specification phase
const spec_analysis = await mcp__psycho_symbolic_reasoner__reason({
  query: "What are the psychological factors in user interface design?",
  reasoning_type: "inductive",
  psychological_factors: ["cognitive_load", "attention", "memory"]
});

// Architecture phase
const arch_reasoning = await mcp__psycho_symbolic_reasoner__reason({
  query: "How should system architecture account for cognitive limitations?",
  reasoning_type: "deductive",
  context: { domain: "software_architecture" }
});
```

### With Swarm Coordination
```typescript
// Coordinate with other agents using reasoning insights
const coordination_analysis = await mcp__psycho_symbolic_reasoner__reason({
  query: "Optimal agent coordination patterns for complex reasoning tasks",
  reasoning_type: "analogical",
  psychological_factors: ["team_dynamics", "cognitive_diversity"]
});
```

## Monitoring and Metrics

### Performance Tracking
- **Reasoning accuracy**: Track correctness of reasoning outputs
- **Response time**: Monitor query processing speed
- **Knowledge growth**: Track knowledge base expansion
- **Confidence calibration**: Measure prediction accuracy

### Health Monitoring
- **System status**: Regular health checks
- **Resource usage**: Monitor memory and processing
- **Error rates**: Track and analyze failures
- **Knowledge consistency**: Validate graph integrity

## Advanced Features

### Custom Reasoning Types
The agent supports custom reasoning type definitions for domain-specific applications:

```typescript
await mcp__psycho_symbolic_reasoner__reason({
  query: "domain-specific question",
  reasoning_type: "custom",
  custom_reasoning_config: {
    type: "psychological_profile_analysis",
    parameters: {
      factors: ["personality", "motivation", "cognitive_style"],
      weights: { personality: 0.4, motivation: 0.3, cognitive_style: 0.3 }
    }
  }
});
```

### Knowledge Graph Visualization
Query results can include graph visualization data for external rendering:

```typescript
const graph_data = await mcp__psycho_symbolic_reasoner__knowledge_graph_query({
  query: "concept relationships",
  include_visualization: true,
  visualization_format: "d3_json"
});
```

## Security Considerations

- **Input validation**: All inputs are validated and sanitized
- **Knowledge source verification**: Sources are tracked and validated
- **Access control**: Sensitive knowledge can be access-controlled
- **Audit logging**: All operations are logged for review

## Version History

- **v1.0.0**: Initial release with core reasoning capabilities
- **Future versions**: Enhanced psychological modeling, advanced graph algorithms

---

*This agent configuration follows Claude agent best practices and is designed to integrate seamlessly with the Claude Code environment and SPARC methodology.*