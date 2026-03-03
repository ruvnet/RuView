---
name: flow-nexus-swarm
description: Cloud-based AI swarm deployment and event-driven workflow automation with Flow Nexus platform
category: orchestration
tags: [swarm, workflow, cloud, agents, automation, message-queue]
version: 1.0.0
requires:
  - flow-nexus MCP server
  - Active Flow Nexus account (register at flow-nexus.ruv.io)
---

# Flow Nexus Swarm & Workflow Orchestration

Deploy and manage cloud-based AI agent swarms with event-driven workflow automation, message queue processing, and intelligent agent coordination.

## 📋 Table of Contents

1. [Overview](#overview)
2. [Swarm Management](#swarm-management)
3. [Workflow Automation](#workflow-automation)
4. [Agent Orchestration](#agent-orchestration)
5. [Templates & Patterns](#templates--patterns)
6. [Advanced Features](#advanced-features)
7. [Best Practices](#best-practices)

## Overview

Flow Nexus provides cloud-based orchestration for AI agent swarms with:

- **Multi-topology Support**: Hierarchical, mesh, ring, and star architectures
- **Event-driven Workflows**: Message queue processing with async execution
- **Template Library**: Pre-built swarm configurations for common use cases
- **Intelligent Agent Assignment**: Vector similarity matching for optimal agent selection
- **Real-time Monitoring**: Comprehensive metrics and audit trails
- **Scalable Infrastructure**: Cloud-based execution with auto-scaling

## Swarm Management

### Initialize Swarm

Create a new swarm with specified topology and configuration:

```javascript
mcp__flow-nexus__swarm_init({
  topology: "hierarchical", // Options: mesh, ring, star, hierarchical
  maxAgents: 8,
  strategy: "balanced" // Options: balanced, specialized, adaptive
})
```

**Topology Guide:**
- **Hierarchical**: Tree structure with coordinator nodes (best for complex projects)
- **Mesh**: Peer-to-peer collaboration (best for research and analysis)
- **Ring**: Circular coordination (best for sequential workflows)
- **Star**: Centralized hub (best for simple delegation)

**Strategy Guide:**
- **Balanced**: Equal distribution of workload across agents
- **Specialized**: Agents focus on specific expertise areas
- **Adaptive**: Dynamic adjustment based on task complexity

### Spawn Agents

Add specialized agents to the swarm:

```javascript
mcp__flow-nexus__agent_spawn({
  type: "researcher", // Options: researcher, coder, analyst, optimizer, coordinator
  name: "Lead Researcher",
  capabilities: ["web_search", "analysis", "summarization"]
})
```

**Agent Types:**
- **Researcher**: Information gathering, web search, analysis
- **Coder**: Code generation, refactoring, implementation
- **Analyst**: Data analysis, pattern recognition, insights
- **Optimizer**: Performance tuning, resource optimization
- **Coordinator**: Task delegation, progress tracking, integration

### Orchestrate Tasks

Distribute tasks across the swarm:

```javascript
mcp__flow-nexus__task_orchestrate({
  task: "Build a REST API with authentication and database integration",
  strategy: "parallel", // Options: parallel, sequential, adaptive
  maxAgents: 5,
  priority: "high" // Options: low, medium, high, critical
})
```

**Execution Strategies:**
- **Parallel**: Maximum concurrency for independent subtasks
- **Sequential**: Step-by-step execution with dependencies
- **Adaptive**: AI-powered strategy selection based on task analysis

### Monitor & Scale Swarms

```javascript
// Get detailed swarm status
mcp__flow-nexus__swarm_status({
  swarm_id: "optional-id" // Uses active swarm if not provided
})

// List all active swarms
mcp__flow-nexus__swarm_list({
  status: "active" // Options: active, destroyed, all
})

// Scale swarm up or down
mcp__flow-nexus__swarm_scale({
  target_agents: 10,
  swarm_id: "optional-id"
})

// Gracefully destroy swarm
mcp__flow-nexus__swarm_destroy({
  swarm_id: "optional-id"
})
```

## Workflow Automation

### Create Workflow

Define event-driven workflows with message queue processing:

```javascript
mcp__flow-nexus__workflow_create({
  name: "CI/CD Pipeline",
  description: "Automated testing, building, and deployment",
  steps: [
    {
      id: "test",
      action: "run_tests",
      agent: "tester",
      parallel: true
    },
    {
      id: "build",
      action: "build_app",
      agent: "builder",
      depends_on: ["test"]
    },
    {
      id: "deploy",
      action: "deploy_prod",
      agent: "deployer",
      depends_on: ["build"]
    }
  ],
  triggers: ["push_to_main", "manual_trigger"],
  metadata: {
    priority: 10,
    retry_policy: "exponential_backoff"
  }
})
```

**Workflow Features:**
- **Dependency Management**: Define step dependencies with `depends_on`
- **Parallel Execution**: Set `parallel: true` for concurrent steps
- **Event Triggers**: GitHub events, schedules, manual triggers
- **Retry Policies**: Automatic retry on transient failures
- **Priority Queuing**: High-priority workflows execute first

### Execute Workflow

Run workflows synchronously or asynchronously:

```javascript
mcp__flow-nexus__workflow_execute({
  workflow_id: "workflow_id",
  input_data: {
    branch: "main",
    commit: "abc123",
    environment: "production"
  },
  async: true // Queue-based execution for long-running workflows
})
```

**Execution Modes:**
- **Sync (async: false)**: Immediate execution, wait for completion
- **Async (async: true)**: Message queue processing, non-blocking

### Monitor Workflows

```javascript
// Get workflow status and metrics
mcp__flow-nexus__workflow_status({
  workflow_id: "id",
  execution_id: "specific-run-id", // Optional
  include_metrics: true
})

// List workflows with filters
mcp__flow-nexus__workflow_list({
  status: "running", // Options: running, completed, failed, pending
  limit: 10,
  offset: 0
})

// Get complete audit trail
mcp__flow-nexus__workflow_audit_trail({
  workflow_id: "id",
  limit: 50,
  start_time: "2025-01-01T00:00:00Z"
})
```

### Agent Assignment

Intelligently assign agents to workflow tasks:

```javascript
mcp__flow-nexus__workflow_agent_assign({
  task_id: "task_id",
  agent_type: "coder", // Preferred agent type
  use_vector_similarity: true // AI-powered capability matching
})
```

**Vector Similarity Matching:**
- Analyzes task requirements and agent capabilities
- Finds optimal agent based on past performance
- Considers workload and availability

### Queue Management

Monitor and manage message queues:

```javascript
mcp__flow-nexus__workflow_queue_status({
  queue_name: "optional-specific-queue",
  include_messages: true // Show pending messages
})
```

## Agent Orchestration

### Full-Stack Development Pattern

```javascript
// 1. Initialize swarm with hierarchical topology
mcp__flow-nexus__swarm_init({
  topology: "hierarchical",
  maxAgents: 8,
  strategy: "specialized"
})

// 2. Spawn specialized agents
mcp__flow-nexus__agent_spawn({ type: "coordinator", name: "Project Manager" })
mcp__flow-nexus__agent_spawn({ type: "coder", name: "Backend Developer" })
mcp__flow-nexus__agent_spawn({ type: "coder", name: "Frontend Developer" })
mcp__flow-nexus__agent_spawn({ type: "coder", name: "Database Architect" })
mcp__flow-nexus__agent_spawn({ type: "analyst", name: "QA Engineer" })

// 3. Create development workflow
mcp__flow-nexus__workflow_create({
  name: "Full-Stack Development",
  steps: [
    { id: "requirements", action: "analyze_requirements", agent: "coordinator" },
    { id: "db_design", action: "design_schema", agent: "Database Architect" },
    { id: "backend", action: "build_api", agent: "Backend Developer", depends_on: ["db_design"] },
    { id: "frontend", action: "build_ui", agent: "Frontend Developer", depends_on: ["requirements"] },
    { id: "integration", action: "integrate", agent: "Backend Developer", depends_on: ["backend", "frontend"] },
    { id: "testing", action: "qa_testing", agent: "QA Engineer", depends_on: ["integration"] }
  ]
})

// 4. Execute workflow
mcp__flow-nexus__workflow_execute({
  workflow_id: "workflow_id",
  input_data: {
    project: "E-commerce Platform",
    tech_stack: ["Node.js", "React", "PostgreSQL"]
  }
})
```

### Research & Analysis Pattern

```javascript
// 1. Initialize mesh topology for collaborative research
mcp__flow-nexus__swarm_init({
  topology: "mesh",
  maxAgents: 5,
  strategy: "balanced"
})

// 2. Spawn research agents
mcp__flow-nexus__agent_spawn({ type: "researcher", name: "Primary Researcher" })
mcp__flow-nexus__agent_spawn({ type: "researcher", name: "Secondary Researcher" })
mcp__flow-nexus__agent_spawn({ type: "analyst", name: "Data Analyst" })
mcp__flow-nexus__agent_spawn({ type: "analyst", name: "Insights Analyst" })

// 3. Orchestrate research task
mcp__flow-nexus__task_orchestrate({
  task: "Research machine learning trends for 2025 and analyze market opportunities",
  strategy: "parallel",
  maxAgents: 4,
  priority: "high"
})
```

### CI/CD Pipeline Pattern

```javascript
mcp__flow-nexus__workflow_create({
  name: "Deployment Pipeline",
  description: "Automated testing, building, and multi-environment deployment",
  steps: [
    { id: "lint", action: "lint_code", agent: "code_quality", parallel: true },
    { id: "unit_test", action: "unit_tests", agent: "test_runner", parallel: true },
    { id: "integration_test", action: "integration_tests", agent: "test_runner", parallel: true },
    { id: "build", action: "build_artifacts", agent: "builder", depends_on: ["lint", "unit_test", "integration_test"] },
    { id: "security_scan", action: "security_scan", agent: "security", depends_on: ["build"] },
    { id: "deploy_staging", action: "deploy", agent: "deployer", depends_on: ["security_scan"] },
    { id: "smoke_test", action: "smoke_tests", agent: "test_runner", depends_on: ["deploy_staging"] },
    { id: "deploy_prod", action: "deploy", agent: "deployer", depends_on: ["smoke_test"] }
  ],
  triggers: ["github_push", "github_pr_merged"],
  metadata: {
    priority: 10,
    auto_rollback: true
  }
})
```

### Data Processing Pipeline Pattern

```javascript
mcp__flow-nexus__workflow_create({
  name: "ETL Pipeline",
  description: "Extract, Transform, Load data processing",
  steps: [
    { id: "extract", action: "extract_data", agent: "data_extractor" },
    { id: "validate_raw", action: "validate_data", agent: "validator", depends_on: ["extract"] },
    { id: "transform", action: "transform_data", agent: "transformer", depends_on: ["validate_raw"] },
    { id: "enrich", action: "enrich_data", agent: "enricher", depends_on: ["transform"] },
    { id: "load", action: "load_data", agent: "loader", depends_on: ["enrich"] },
    { id: "validate_final", action: "validate_data", agent: "validator", depends_on: ["load"] }
  ],
  triggers: ["schedule:0 2 * * *"], // Daily at 2 AM
  metadata: {
    retry_policy: "exponential_backoff",
    max_retries: 3
  }
})
```

## Templates & Patterns

### Use Pre-built Templates

```javascript
// Create swarm from template
mcp__flow-nexus__swarm_create_from_template({
  template_name: "full-stack-dev",
  overrides: {
    maxAgents: 6,
    strategy: "specialized"
  }
})

// List available templates
mcp__flow-nexus__swarm_templates_list({
  category: "quickstart", // Options: quickstart, specialized, enterprise, custom, all
  includeStore: true
})
```

**Available Template Categories:**

**Quickstart Templates:**
- `full-stack-dev`: Complete web development swarm
- `research-team`: Research and analysis swarm
- `code-review`: Automated code review swarm
- `data-pipeline`: ETL and data processing

**Specialized Templates:**
- `ml-development`: Machine learning project swarm
- `mobile-dev`: Mobile app development
- `devops-automation`: Infrastructure and deployment
- `security-audit`: Security analysis and testing

**Enterprise Templates:**
- `enterprise-migration`: Large-scale system migration
- `multi-repo-sync`: Multi-repository coordination
- `compliance-review`: Regulatory compliance workflows
- `incident-response`: Automated incident management

### Custom Template Creation

Save successful swarm configurations as reusable templates for future projects.

## Advanced Features

### Real-time Monitoring

```javascript
// Subscribe to execution streams
mcp__flow-nexus__execution_stream_subscribe({
  stream_type: "claude-flow-swarm",
  deployment_id: "deployment_id"
})

// Get execution status
mcp__flow-nexus__execution_stream_status({
  stream_id: "stream_id"
})

// List files created during execution
mcp__flow-nexus__execution_files_list({
  stream_id: "stream_id",
  created_by: "claude-flow"
})
```

### Swarm Metrics & Analytics

```javascript
// Get swarm performance metrics
mcp__flow-nexus__swarm_status({
  swarm_id: "id"
})

// Analyze workflow efficiency
mcp__flow-nexus__workflow_status({
  workflow_id: "id",
  include_metrics: true
})
```

### Multi-Swarm Coordination

Coordinate multiple swarms for complex, multi-phase projects:

```javascript
// Phase 1: Research swarm
const researchSwarm = await mcp__flow-nexus__swarm_init({
  topology: "mesh",
  maxAgents: 4
})

// Phase 2: Development swarm
const devSwarm = await mcp__flow-nexus__swarm_init({
  topology: "hierarchical",
  maxAgents: 8
})

// Phase 3: Testing swarm
const testSwarm = await mcp__flow-nexus__swarm_init({
  topology: "star",
  maxAgents: 5
})
```

## Best Practices

### 1. Choose the Right Topology

```javascript
// Simple projects: Star
mcp__flow-nexus__swarm_init({ topology: "star", maxAgents: 3 })

// Collaborative work: Mesh
mcp__flow-nexus__swarm_init({ topology: "mesh", maxAgents: 5 })

// Complex projects: Hierarchical
mcp__flow-nexus__swarm_init({ topology: "hierarchical", maxAgents: 10 })

// Sequential workflows: Ring
mcp__flow-nexus__swarm_init({ topology: "ring", maxAgents: 4 })
```

### 2. Optimize Agent Assignment

```javascript
// Use vector similarity for optimal matching
mcp__flow-nexus__workflow_agent_assign({
  task_id: "complex-task",
  use_vector_similarity: true
})
```

### 3. Implement Proper Error Handling

```javascript
mcp__flow-nexus__workflow_create({
  name: "Resilient Workflow",
  steps: [...],
  metadata: {
    retry_policy: "exponential_backoff",
    max_retries: 3,
    timeout: 300000, // 5 minutes
    on_failure: "notify_and_rollback"
  }
})
```

### 4. Monitor and Scale

```javascript
// Regular monitoring
const status = await mcp__flow-nexus__swarm_status()

// Scale based on workload
if (status.workload > 0.8) {
  await mcp__flow-nexus__swarm_scale({ target_agents: status.agents + 2 })
}
```

### 5. Use Async Execution for Long-Running Workflows

```javascript
// Long-running workflows should use message queues
mcp__flow-nexus__workflow_execute({
  workflow_id: "data-pipeline",
  async: true // Non-blocking execution
})

// Monitor progress
mcp__flow-nexus__workflow_queue_status({ include_messages: true })
```

### 6. Clean Up Resources

```javascript
// Destroy swarm when complete
mcp__flow-nexus__swarm_destroy({ swarm_id: "id" })
```

### 7. Leverage Templates

```javascript
// Use proven templates instead of building from scratch
mcp__flow-nexus__swarm_create_from_template({
  template_name: "code-review",
  overrides: { maxAgents: 4 }
})
```

## Integration with Claude Flow

Flow Nexus swarms integrate seamlessly with Claude Flow hooks:

```bash
# Pre-task coordination setup
npx claude-flow@alpha hooks pre-task --description "Initialize swarm"

# Post-task metrics export
npx claude-flow@alpha hooks post-task --task-id "swarm-execution"
```

## Common Use Cases

### 1. Multi-Repo Development
- Coordinate development across multiple repositories
- Synchronized testing and deployment
- Cross-repo dependency management

### 2. Research Projects
- Distributed information gathering
- Parallel analysis of different data sources
- Collaborative synthesis and reporting

### 3. DevOps Automation
- Infrastructure as Code deployment
- Multi-environment testing
- Automated rollback and recovery

### 4. Code Quality Workflows
- Automated code review
- Security scanning
- Performance benchmarking

### 5. Data Processing
- Large-scale ETL pipelines
- Real-time data transformation
- Data validation and quality checks

## Authentication & Setup

```bash
# Install Flow Nexus
npm install -g flow-nexus@latest

# Register account
npx flow-nexus@latest register

# Login
npx flow-nexus@latest login

# Add MCP server to Claude Code
claude mcp add flow-nexus npx flow-nexus@latest mcp start
```

## Support & Resources

- **Platform**: https://flow-nexus.ruv.io
- **Documentation**: https://github.com/ruvnet/flow-nexus
- **Issues**: https://github.com/ruvnet/flow-nexus/issues

---

**Remember**: Flow Nexus provides cloud-based orchestration infrastructure. For local execution and coordination, use the core `claude-flow` MCP server alongside Flow Nexus for maximum flexibility.
