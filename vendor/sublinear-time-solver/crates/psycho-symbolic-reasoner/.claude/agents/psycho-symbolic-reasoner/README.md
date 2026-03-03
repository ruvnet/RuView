# Psycho-Symbolic Reasoner Claude Agent

A comprehensive Claude agent configuration for advanced psycho-symbolic reasoning that combines symbolic logic with psychological context understanding.

## Quick Start

### 1. Install the MCP Server

First, add the psycho-symbolic-reasoner MCP server to your Claude configuration:

```bash
# Add the MCP server
claude mcp add psycho-symbolic-reasoner npx psycho-symbolic-reasoner mcp start

# Verify installation
claude mcp list
```

### 2. Basic Usage

Once configured, you can use the agent through Claude with natural language or direct tool calls:

```typescript
// Perform reasoning analysis
const result = await mcp__psycho_symbolic_reasoner__reason({
  query: "How does cognitive bias affect decision-making in AI-assisted environments?",
  reasoning_type: "causal",
  psychological_factors: ["confirmation_bias", "automation_bias", "anchoring"]
});
```

## Features

### 🧠 Advanced Reasoning Capabilities
- **Multiple reasoning types**: Deductive, inductive, abductive, analogical, and causal reasoning
- **Psychological integration**: Incorporates cognitive psychology principles
- **Confidence tracking**: Provides calibrated confidence scores
- **Bias detection**: Identifies and mitigates reasoning biases

### 📊 Knowledge Graph Management
- **Dynamic knowledge base**: Add and query psychological concepts, theories, and relationships
- **Graph exploration**: Navigate complex knowledge relationships
- **Source attribution**: Track knowledge provenance and reliability
- **Consistency validation**: Maintain knowledge base integrity

### 🔍 Analysis and Validation
- **Reasoning path analysis**: Examine step-by-step reasoning processes
- **Quality assessment**: Evaluate logical validity and psychological plausibility
- **Alternative exploration**: Consider multiple reasoning approaches
- **Performance optimization**: Improve reasoning through feedback

### ⚡ System Integration
- **Claude Code compatible**: Works seamlessly with SPARC methodology
- **Swarm coordination**: Supports multi-agent reasoning scenarios
- **Real-time diagnostics**: Monitor system health and performance
- **Error handling**: Robust error detection and recovery

## Available Tools

| Tool | Purpose | Use Case |
|------|---------|----------|
| `reason` | Perform psycho-symbolic analysis | Complex reasoning tasks requiring psychological insight |
| `knowledge_graph_query` | Search knowledge relationships | Finding related concepts and theories |
| `add_knowledge` | Expand knowledge base | Adding domain expertise and research findings |
| `analyze_reasoning_path` | Evaluate reasoning quality | Validating and improving reasoning processes |
| `health_check` | System diagnostics | Monitoring performance and troubleshooting |

## Documentation

### Core Documentation
- **[Agent Configuration](./agent.md)** - Complete agent setup and configuration guide
- **[API Reference](./api-reference.md)** - Detailed tool documentation with schemas
- **[Basic Usage Examples](./examples/basic-usage.md)** - Getting started with common scenarios
- **[Advanced Scenarios](./examples/advanced-scenarios.md)** - Complex multi-domain analysis examples

### Key Sections
- **Tool Parameters**: Detailed parameter descriptions and validation rules
- **Response Schemas**: Complete response format documentation
- **Error Handling**: Common errors and resolution strategies
- **Best Practices**: Optimization tips and usage patterns

## Configuration Files

```
.claude/agents/psycho-symbolic-reasoner/
├── agent.md              # Main agent configuration
├── config.json           # Agent metadata and settings
├── api-reference.md      # Complete API documentation
├── README.md             # This file
└── examples/
    ├── basic-usage.md    # Basic usage examples
    └── advanced-scenarios.md  # Advanced multi-domain examples
```

## Integration Examples

### With SPARC Methodology

```typescript
// Specification Phase - Psychological Requirements Analysis
const spec_analysis = await mcp__psycho_symbolic_reasoner__reason({
  query: "What psychological factors should be considered in user interface design?",
  reasoning_type: "deductive",
  psychological_factors: ["cognitive_load", "attention", "usability"]
});

// Use insights in subsequent SPARC phases
console.log("Design constraints:", spec_analysis.recommendations);
```

### With Claude Code Swarm

```typescript
// Coordinate multiple agents using psychological insights
const coordination_strategy = await mcp__psycho_symbolic_reasoner__reason({
  query: "Optimal team coordination patterns for cognitive diversity?",
  reasoning_type: "inductive",
  psychological_factors: ["team_dynamics", "cognitive_styles", "communication"]
});

// Apply to swarm configuration
const swarm_config = {
  topology: coordination_strategy.recommended_topology,
  communication: coordination_strategy.communication_protocol
};
```

## Use Cases

### 🎯 AI Ethics and Safety
- Analyze psychological implications of AI decisions
- Evaluate bias in AI training and deployment
- Design human-centered AI interaction patterns
- Assess psychological safety of AI systems

### 🎨 User Experience Design
- Apply cognitive psychology to interface design
- Optimize information architecture for human cognition
- Design for accessibility and cognitive diversity
- Evaluate user mental models and expectations

### 🤝 Human-AI Collaboration
- Design effective human-AI team structures
- Optimize AI assistance for human cognitive patterns
- Mitigate automation bias and over-reliance
- Enhance trust calibration in AI systems

### 📚 Educational Technology
- Apply learning psychology to AI tutoring systems
- Design adaptive learning experiences
- Optimize cognitive load in educational interfaces
- Personalize learning based on cognitive profiles

### 🧪 Research and Analysis
- Conduct literature reviews in psychology and AI
- Synthesize cross-disciplinary research findings
- Generate research hypotheses based on psychological theory
- Validate psychological models with AI behavior data

## Requirements

### System Requirements
- **Claude**: Version 3.0.0 or higher
- **MCP**: Version 1.0.0 or higher
- **Node.js**: Version 18.0.0 or higher

### Optional Integrations
- **Claude Code**: For SPARC methodology integration
- **Claude Flow**: For swarm coordination capabilities
- **Flow-Nexus**: For cloud-based advanced features

## Performance and Limits

### Rate Limits
- **Reasoning queries**: 100 per hour
- **Knowledge operations**: 500 per hour
- **Graph queries**: 200 per hour
- **Health checks**: 20 per minute

### Performance Optimization
- Use specific entity types for faster graph queries
- Batch related knowledge additions
- Cache frequently used reasoning patterns
- Monitor system health regularly

## Troubleshooting

### Common Issues

**Low Confidence Results**
```typescript
// Solution: Add domain knowledge or adjust reasoning approach
if (result.confidence < 0.7) {
  await mcp__psycho_symbolic_reasoner__add_knowledge({
    // Add relevant domain knowledge
  });

  // Retry with different reasoning type
  const retry = await mcp__psycho_symbolic_reasoner__reason({
    query: original_query,
    reasoning_type: "abductive"  // Try best-explanation reasoning
  });
}
```

**Knowledge Gaps**
```typescript
// Solution: Use knowledge gap analysis
const gaps = await mcp__psycho_symbolic_reasoner__analyze_reasoning_path({
  reasoning_id: session_id,
  analysis_type: "knowledge_gaps"
});

// Add missing knowledge based on identified gaps
for (const gap of gaps.identified_gaps) {
  // Add knowledge to fill gaps
}
```

**System Performance Issues**
```typescript
// Solution: Check system health and optimize
const health = await mcp__psycho_symbolic_reasoner__health_check({
  check_type: "performance",
  include_metrics: true
});

if (health.performance_metrics.average_reasoning_time > 10000) {
  // Optimize queries or increase resources
}
```

## Support and Contribution

### Getting Help
- Check the [API Reference](./api-reference.md) for detailed documentation
- Review [Examples](./examples/) for common usage patterns
- Use the `health_check` tool for system diagnostics

### Best Practices
- Always provide specific psychological factors for better reasoning
- Use appropriate reasoning types for different problem domains
- Validate reasoning paths with analysis tools
- Build knowledge base incrementally with reliable sources

### Error Reporting
When reporting issues, include:
- Agent configuration details
- Tool parameters used
- Error messages and codes
- System health check results
- Reproducible example queries

---

This Claude agent configuration provides a robust foundation for advanced psycho-symbolic reasoning tasks, combining the power of symbolic logic with deep psychological understanding to solve complex problems across multiple domains.