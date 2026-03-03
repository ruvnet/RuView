# Psycho-Symbolic Reasoner - Basic Usage Examples

## Quick Start

### 1. Setup and Health Check

First, verify the agent is properly configured and running:

```typescript
// Check system health
const health = await mcp__psycho_symbolic_reasoner__health_check({
  check_type: "basic",
  include_metrics: true
});

console.log("System Status:", health.status);
console.log("Available capabilities:", health.capabilities);
```

### 2. Simple Reasoning Query

Perform basic reasoning analysis:

```typescript
const result = await mcp__psycho_symbolic_reasoner__reason({
  query: "Why do people often make irrational decisions under stress?",
  reasoning_type: "causal",
  psychological_factors: ["stress", "decision_making", "cognitive_load"]
});

console.log("Reasoning result:", result.conclusion);
console.log("Confidence:", result.confidence);
console.log("Supporting evidence:", result.evidence);
```

### 3. Knowledge Graph Exploration

Query the knowledge graph for related concepts:

```typescript
const knowledge = await mcp__psycho_symbolic_reasoner__knowledge_graph_query({
  query: "stress decision making cognitive psychology",
  entity_types: ["concept", "theory", "phenomenon"],
  depth: 2,
  limit: 10
});

console.log("Related concepts:", knowledge.entities);
console.log("Relationships:", knowledge.relationships);
```

## Practical Scenarios

### Scenario 1: Analyzing Cognitive Bias in AI Systems

```typescript
// Step 1: Perform reasoning about AI bias
const bias_analysis = await mcp__psycho_symbolic_reasoner__reason({
  query: "How do human cognitive biases transfer to AI training data and model behavior?",
  reasoning_type: "causal",
  psychological_factors: ["confirmation_bias", "availability_heuristic", "anchoring_bias"],
  confidence_threshold: 0.8
});

// Step 2: Query for related research
const research = await mcp__psycho_symbolic_reasoner__knowledge_graph_query({
  query: "AI bias cognitive psychology machine learning",
  entity_types: ["study", "researcher", "methodology"],
  relationship_types: ["studied_by", "demonstrates", "mitigates"]
});

// Step 3: Analyze the reasoning path
const path_analysis = await mcp__psycho_symbolic_reasoner__analyze_reasoning_path({
  reasoning_id: bias_analysis.session_id,
  analysis_type: "bias_detection",
  include_suggestions: true
});

console.log("Bias analysis:", path_analysis.detected_biases);
console.log("Suggestions:", path_analysis.suggestions);
```

### Scenario 2: User Experience Design Psychology

```typescript
// Reasoning about UX psychology
const ux_reasoning = await mcp__psycho_symbolic_reasoner__reason({
  query: "What psychological principles should guide mobile app navigation design?",
  reasoning_type: "deductive",
  psychological_factors: ["cognitive_load", "mental_models", "visual_attention"],
  context: {
    domain: "user_experience",
    platform: "mobile",
    user_type: "general_population"
  }
});

// Add new knowledge from UX research
await mcp__psycho_symbolic_reasoner__add_knowledge({
  knowledge_type: "rule",
  content: {
    condition: "high_cognitive_load_interface",
    conclusion: "reduced_user_satisfaction",
    strength: "strong",
    domain: "ux_design"
  },
  source: "Nielsen, J. (1993) - Usability Engineering",
  confidence: 0.9,
  tags: ["ux", "cognitive_load", "usability"]
});

console.log("UX principles:", ux_reasoning.principles);
console.log("Design recommendations:", ux_reasoning.recommendations);
```

### Scenario 3: Learning and Memory Optimization

```typescript
// Analyze effective learning strategies
const learning_analysis = await mcp__psycho_symbolic_reasoner__reason({
  query: "How can spaced repetition and active recall be optimized based on individual cognitive profiles?",
  reasoning_type: "inductive",
  psychological_factors: ["memory_consolidation", "individual_differences", "metacognition"],
  context: {
    application: "educational_technology",
    target_audience: "students"
  }
});

// Query existing memory research
const memory_research = await mcp__psycho_symbolic_reasoner__knowledge_graph_query({
  query: "spaced repetition active recall memory consolidation",
  entity_types: ["technique", "study", "theory"],
  relationship_types: ["improves", "based_on", "validates"]
});

// Analyze for potential improvements
const optimization_analysis = await mcp__psycho_symbolic_reasoner__analyze_reasoning_path({
  reasoning_id: learning_analysis.session_id,
  analysis_type: "logical_validity",
  detailed_breakdown: true
});

console.log("Learning strategies:", learning_analysis.strategies);
console.log("Individual adaptation factors:", learning_analysis.adaptation_factors);
```

## Error Handling Examples

### Handling Insufficient Knowledge

```typescript
try {
  const result = await mcp__psycho_symbolic_reasoner__reason({
    query: "How does quantum consciousness affect decision-making?",
    reasoning_type: "deductive"
  });
} catch (error) {
  if (error.code === 'INSUFFICIENT_KNOWLEDGE') {
    console.log("Adding foundational knowledge...");

    // Add basic concepts
    await mcp__psycho_symbolic_reasoner__add_knowledge({
      knowledge_type: "concept",
      content: {
        name: "quantum_consciousness",
        definition: "Hypothetical quantum mechanical phenomena in consciousness",
        status: "theoretical",
        controversy_level: "high"
      },
      confidence: 0.3  // Low confidence due to theoretical nature
    });

    // Retry with lower confidence threshold
    const result = await mcp__psycho_symbolic_reasoner__reason({
      query: "How does quantum consciousness affect decision-making?",
      reasoning_type: "abductive",  // Use best-explanation reasoning
      confidence_threshold: 0.3
    });
  }
}
```

### Handling Low Confidence Results

```typescript
const result = await mcp__psycho_symbolic_reasoner__reason({
  query: "Complex interdisciplinary question",
  reasoning_type: "analogical"
});

if (result.confidence < 0.6) {
  console.log("Low confidence result, seeking additional context...");

  // Try different reasoning approach
  const alternative = await mcp__psycho_symbolic_reasoner__reason({
    query: result.original_query,
    reasoning_type: "inductive",
    context: {
      require_multiple_perspectives: true,
      evidence_threshold: "high"
    }
  });

  // Compare results
  console.log("Original confidence:", result.confidence);
  console.log("Alternative confidence:", alternative.confidence);
  console.log("Recommendation:", alternative.confidence > result.confidence ?
    "Use alternative approach" : "Gather more information");
}
```

## Integration with Claude Code Workflows

### SPARC Methodology Integration

```typescript
// Specification Phase
const spec_reasoning = await mcp__psycho_symbolic_reasoner__reason({
  query: "What psychological factors must be considered when designing an AI tutoring system?",
  reasoning_type: "deductive",
  psychological_factors: ["learning_styles", "motivation", "feedback_processing"],
  context: { phase: "specification", domain: "educational_ai" }
});

// Pseudocode Phase - Use reasoning insights
const design_principles = spec_reasoning.principles;
console.log("Design constraints from psychological analysis:", design_principles);

// Architecture Phase - Apply psychological insights to system design
const arch_reasoning = await mcp__psycho_symbolic_reasoner__reason({
  query: "How should system architecture adapt to different learning psychology principles?",
  reasoning_type: "analogical",
  context: {
    previous_insights: design_principles,
    phase: "architecture"
  }
});
```

### Swarm Coordination Integration

```typescript
// Coordinate multiple agents using psychological insights
const coordination_strategy = await mcp__psycho_symbolic_reasoner__reason({
  query: "What are optimal team coordination patterns for diverse cognitive styles?",
  reasoning_type: "inductive",
  psychological_factors: ["cognitive_diversity", "team_dynamics", "communication_styles"]
});

// Use insights to configure agent swarm
const swarm_config = {
  topology: coordination_strategy.recommended_topology,
  communication_protocol: coordination_strategy.communication_style,
  task_distribution: coordination_strategy.task_allocation_strategy
};

console.log("Psychologically-informed swarm configuration:", swarm_config);
```

## Performance Monitoring

### Tracking Reasoning Quality

```typescript
// Monitor reasoning performance over time
const performance_check = await mcp__psycho_symbolic_reasoner__health_check({
  check_type: "performance",
  include_metrics: true,
  verbose: true
});

console.log("Average reasoning time:", performance_check.metrics.avg_reasoning_time);
console.log("Confidence calibration:", performance_check.metrics.confidence_accuracy);
console.log("Knowledge graph size:", performance_check.metrics.knowledge_base_size);

// Analyze reasoning patterns
const pattern_analysis = await mcp__psycho_symbolic_reasoner__analyze_reasoning_path({
  reasoning_id: "recent_sessions",
  analysis_type: "confidence_calibration",
  include_suggestions: true
});

console.log("Calibration accuracy:", pattern_analysis.calibration_score);
console.log("Improvement suggestions:", pattern_analysis.suggestions);
```

This examples file provides practical, copy-paste ready code for common usage patterns and integration scenarios with the Psycho-Symbolic Reasoner agent.