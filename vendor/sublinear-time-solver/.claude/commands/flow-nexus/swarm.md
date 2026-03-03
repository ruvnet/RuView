---
name: flow-nexus-swarm
description: AI swarm deployment and coordination in cloud
---

# Flow Nexus Swarms

Deploy and manage AI agent swarms in the cloud.

## Initialize Swarm
```javascript
mcp__flow-nexus__swarm_init({
  topology: "hierarchical", // mesh, ring, star, hierarchical
  maxAgents: 8,
  strategy: "balanced" // balanced, specialized, adaptive
})
```

## Spawn Agents
```javascript
mcp__flow-nexus__agent_spawn({
  type: "researcher", // coder, analyst, optimizer, coordinator
  name: "Lead Researcher",
  capabilities: ["web_search", "analysis", "summarization"]
})
```

## Orchestrate Tasks
```javascript
mcp__flow-nexus__task_orchestrate({
  task: "Build a REST API with authentication",
  strategy: "parallel", // parallel, sequential, adaptive
  maxAgents: 5,
  priority: "high"
})
```

## Monitor Swarm
```javascript
// Get swarm status
mcp__flow-nexus__swarm_status()

// List active swarms
mcp__flow-nexus__swarm_list({ status: "active" })

// Scale swarm
mcp__flow-nexus__swarm_scale({ target_agents: 10 })

// Destroy swarm
mcp__flow-nexus__swarm_destroy({ swarm_id: "id" })
```

## Templates
```javascript
// Use pre-built swarm template
mcp__flow-nexus__swarm_create_from_template({
  template_name: "full-stack-dev",
  overrides: {
    maxAgents: 6,
    strategy: "specialized"
  }
})

// List available templates
mcp__flow-nexus__swarm_templates_list({
  category: "quickstart" // specialized, enterprise, custom
})
```

## Common Swarm Patterns

### Research Swarm
```javascript
mcp__flow-nexus__swarm_init({ topology: "mesh", maxAgents: 5 })
mcp__flow-nexus__agent_spawn({ type: "researcher", name: "Lead" })
mcp__flow-nexus__agent_spawn({ type: "analyst", name: "Data Analyst" })
mcp__flow-nexus__task_orchestrate({ task: "Research ML trends" })
```

### Development Swarm
```javascript
mcp__flow-nexus__swarm_init({ topology: "hierarchical", maxAgents: 8 })
mcp__flow-nexus__agent_spawn({ type: "coordinator", name: "PM" })
mcp__flow-nexus__agent_spawn({ type: "coder", name: "Backend Dev" })
mcp__flow-nexus__agent_spawn({ type: "coder", name: "Frontend Dev" })
mcp__flow-nexus__task_orchestrate({ task: "Build e-commerce platform" })
```