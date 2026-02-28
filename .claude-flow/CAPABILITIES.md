# Claude Flow V3 - Complete Capabilities Reference
> Generated: 2026-02-28T16:04:10.839Z
> Full documentation: https://github.com/ruvnet/claude-flow

## 📋 Table of Contents

1. [Overview](#overview)
2. [Swarm Orchestration](#swarm-orchestration)
3. [Available Agents (60+)](#available-agents)
4. [CLI Commands (26 Commands, 140+ Subcommands)](#cli-commands)
5. [Hooks System (27 Hooks + 12 Workers)](#hooks-system)
6. [Memory & Intelligence (RuVector)](#memory--intelligence)
7. [Hive-Mind Consensus](#hive-mind-consensus)
8. [Performance Targets](#performance-targets)
9. [Integration Ecosystem](#integration-ecosystem)

---

## Overview

Claude Flow V3 is a domain-driven design architecture for multi-agent AI coordination with:

- **15-Agent Swarm Coordination** with hierarchical and mesh topologies
- **HNSW Vector Search** - 150x-12,500x faster pattern retrieval
- **SONA Neural Learning** - Self-optimizing with <0.05ms adaptation
- **Byzantine Fault Tolerance** - Queen-led consensus mechanisms
- **MCP Server Integration** - Model Context Protocol support

### Current Configuration
| Setting | Value |
|---------|-------|
| Topology | hierarchical-mesh |
| Max Agents | 15 |
| Memory Backend | hybrid |
| HNSW Indexing | Enabled |
| Neural Learning | Enabled |
| LearningBridge | Enabled (SONA + ReasoningBank) |
| Knowledge Graph | Enabled (PageRank + Communities) |
| Agent Scopes | Enabled (project/local/user) |

---

## Swarm Orchestration

### Topologies
| Topology | Description | Best For |
|----------|-------------|----------|
| `hierarchical` | Queen controls workers directly | Anti-drift, tight control |
| `mesh` | Fully connected peer network | Distributed tasks |
| `hierarchical-mesh` | V3 hybrid (recommended) | 10+ agents |
| `ring` | Circular communication | Sequential workflows |
| `star` | Central coordinator | Simple coordination |
| `adaptive` | Dynamic based on load | Variable workloads |

### Strategies
- `balanced` - Even distribution across agents
- `specialized` - Clear roles, no overlap (anti-drift)
- `adaptive` - Dynamic task routing

### Quick Commands
```bash
# Initialize swarm
npx @claude-flow/cli@latest swarm init --topology hierarchical --max-agents 8 --strategy specialized

# Check status
npx @claude-flow/cli@latest swarm status

# Monitor activity
npx @claude-flow/cli@latest swarm monitor
```

---

## Available Agents

### Core Development (5)
`coder`, `reviewer`, `tester`, `planner`, `researcher`

### V3 Specialized (4)
`security-architect`, `security-auditor`, `memory-specialist`, `performance-engineer`

### Swarm Coordination (5)
`hierarchical-coordinator`, `mesh-coordinator`, `adaptive-coordinator`, `collective-intelligence-coordinator`, `swarm-memory-manager`

### Consensus & Distributed (7)
`byzantine-coordinator`, `raft-manager`, `gossip-coordinator`, `consensus-builder`, `crdt-synchronizer`, `quorum-manager`, `security-manager`

### Performance & Optimization (5)
`perf-analyzer`, `performance-benchmarker`, `task-orchestrator`, `memory-coordinator`, `smart-agent`

### GitHub & Repository (9)
`github-modes`, `pr-manager`, `code-review-swarm`, `issue-tracker`, `release-manager`, `workflow-automation`, `project-board-sync`, `repo-architect`, `multi-repo-swarm`

### SPARC Methodology (6)
`sparc-coord`, `sparc-coder`, `specification`, `pseudocode`, `architecture`, `refinement`

### Specialized Development (8)
`backend-dev`, `mobile-dev`, `ml-developer`, `cicd-engineer`, `api-docs`, `system-architect`, `code-analyzer`, `base-template-generator`

### Testing & Validation (2)
`tdd-london-swarm`, `production-validator`

### Agent Routing by Task
| Task Type | Recommended Agents | Topology |
|-----------|-------------------|----------|
| Bug Fix | researcher, coder, tester | mesh |
| New Feature | coordinator, architect, coder, tester, reviewer | hierarchical |
| Refactoring | architect, coder, reviewer | mesh |
| Performance | researcher, perf-engineer, coder | hierarchical |
| Security | security-architect, auditor, reviewer | hierarchical |
| Docs | researcher, api-docs | mesh |

---

## CLI Commands

### Core Commands (12)
| Command | Subcommands | Description |
|---------|-------------|-------------|
| `init` | 4 | Project initialization |
| `agent` | 8 | Agent lifecycle management |
| `swarm` | 6 | Multi-agent coordination |
| `memory` | 11 | AgentDB with HNSW search |
| `mcp` | 9 | MCP server management |
| `task` | 6 | Task assignment |
| `session` | 7 | Session persistence |
| `config` | 7 | Configuration |
| `status` | 3 | System monitoring |
| `workflow` | 6 | Workflow templates |
| `hooks` | 17 | Self-learning hooks |
| `hive-mind` | 6 | Consensus coordination |

### Advanced Commands (14)
| Command | Subcommands | Description |
|---------|-------------|-------------|
| `daemon` | 5 | Background workers |
| `neural` | 5 | Pattern training |
| `security` | 6 | Security scanning |
| `performance` | 5 | Profiling & benchmarks |
| `providers` | 5 | AI provider config |
| `plugins` | 5 | Plugin management |
| `deployment` | 5 | Deploy management |
| `embeddings` | 4 | Vector embeddings |
| `claims` | 4 | Authorization |
| `migrate` | 5 | V2→V3 migration |
| `process` | 4 | Process management |
| `doctor` | 1 | Health diagnostics |
| `completions` | 4 | Shell completions |

### Example Commands
```bash
# Initialize
npx @claude-flow/cli@latest init --wizard

# Spawn agent
npx @claude-flow/cli@latest agent spawn -t coder --name my-coder

# Memory operations
npx @claude-flow/cli@latest memory store --key "pattern" --value "data" --namespace patterns
npx @claude-flow/cli@latest memory search --query "authentication"

# Diagnostics
npx @claude-flow/cli@latest doctor --fix
```

---

## Hooks System

### 27 Available Hooks

#### Core Hooks (6)
| Hook | Description |
|------|-------------|
| `pre-edit` | Context before file edits |
| `post-edit` | Record edit outcomes |
| `pre-command` | Risk assessment |
| `post-command` | Command metrics |
| `pre-task` | Task start + agent suggestions |
| `post-task` | Task completion learning |

#### Session Hooks (4)
| Hook | Description |
|------|-------------|
| `session-start` | Start/restore session |
| `session-end` | Persist state |
| `session-restore` | Restore previous |
| `notify` | Cross-agent notifications |

#### Intelligence Hooks (5)
| Hook | Description |
|------|-------------|
| `route` | Optimal agent routing |
| `explain` | Routing decisions |
| `pretrain` | Bootstrap intelligence |
| `build-agents` | Generate configs |
| `transfer` | Pattern transfer |

#### Coverage Hooks (3)
| Hook | Description |
|------|-------------|
| `coverage-route` | Coverage-based routing |
| `coverage-suggest` | Improvement suggestions |
| `coverage-gaps` | Gap analysis |

### 12 Background Workers
| Worker | Priority | Purpose |
|--------|----------|---------|
| `ultralearn` | normal | Deep knowledge |
| `optimize` | high | Performance |
| `consolidate` | low | Memory consolidation |
| `predict` | normal | Predictive preload |
| `audit` | critical | Security |
| `map` | normal | Codebase mapping |
| `preload` | low | Resource preload |
| `deepdive` | normal | Deep analysis |
| `document` | normal | Auto-docs |
| `refactor` | normal | Suggestions |
| `benchmark` | normal | Benchmarking |
| `testgaps` | normal | Coverage gaps |

---

## Memory & Intelligence

### RuVector Intelligence System
- **SONA**: Self-Optimizing Neural Architecture (<0.05ms)
- **MoE**: Mixture of Experts routing
- **HNSW**: 150x-12,500x faster search
- **EWC++**: Prevents catastrophic forgetting
- **Flash Attention**: 2.49x-7.47x speedup
- **Int8 Quantization**: 3.92x memory reduction

### 4-Step Intelligence Pipeline
1. **RETRIEVE** - HNSW pattern search
2. **JUDGE** - Success/failure verdicts
3. **DISTILL** - LoRA learning extraction
4. **CONSOLIDATE** - EWC++ preservation

### Self-Learning Memory (ADR-049)

| Component | Status | Description |
|-----------|--------|-------------|
| **LearningBridge** | ✅ Enabled | Connects insights to SONA/ReasoningBank neural pipeline |
| **MemoryGraph** | ✅ Enabled | PageRank knowledge graph + community detection |
| **AgentMemoryScope** | ✅ Enabled | 3-scope agent memory (project/local/user) |

**LearningBridge** - Insights trigger learning trajectories. Confidence evolves: +0.03 on access, -0.005/hour decay. Consolidation runs the JUDGE/DISTILL/CONSOLIDATE pipeline.

**MemoryGraph** - Builds a knowledge graph from entry references. PageRank identifies influential insights. Communities group related knowledge. Graph-aware ranking blends vector + structural scores.

**AgentMemoryScope** - Maps Claude Code 3-scope directories:
- `project`: `<gitRoot>/.claude/agent-memory/<agent>/`
- `local`: `<gitRoot>/.claude/agent-memory-local/<agent>/`
- `user`: `~/.claude/agent-memory/<agent>/`

High-confidence insights (>0.8) can transfer between agents.

### Memory Commands
```bash
# Store pattern
npx @claude-flow/cli@latest memory store --key "name" --value "data" --namespace patterns

# Semantic search
npx @claude-flow/cli@latest memory search --query "authentication"

# List entries
npx @claude-flow/cli@latest memory list --namespace patterns

# Initialize database
npx @claude-flow/cli@latest memory init --force
```

---

## Hive-Mind Consensus

### Queen Types
| Type | Role |
|------|------|
| Strategic Queen | Long-term planning |
| Tactical Queen | Execution coordination |
| Adaptive Queen | Dynamic optimization |

### Worker Types (8)
`researcher`, `coder`, `analyst`, `tester`, `architect`, `reviewer`, `optimizer`, `documenter`

### Consensus Mechanisms
| Mechanism | Fault Tolerance | Use Case |
|-----------|-----------------|----------|
| `byzantine` | f < n/3 faulty | Adversarial |
| `raft` | f < n/2 failed | Leader-based |
| `gossip` | Eventually consistent | Large scale |
| `crdt` | Conflict-free | Distributed |
| `quorum` | Configurable | Flexible |

### Hive-Mind Commands
```bash
# Initialize
npx @claude-flow/cli@latest hive-mind init --queen-type strategic

# Status
npx @claude-flow/cli@latest hive-mind status

# Spawn workers
npx @claude-flow/cli@latest hive-mind spawn --count 5 --type worker

# Consensus
npx @claude-flow/cli@latest hive-mind consensus --propose "task"
```

---

## Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| HNSW Search | 150x-12,500x faster | ✅ Implemented |
| Memory Reduction | 50-75% | ✅ Implemented (3.92x) |
| SONA Integration | Pattern learning | ✅ Implemented |
| Flash Attention | 2.49x-7.47x | 🔄 In Progress |
| MCP Response | <100ms | ✅ Achieved |
| CLI Startup | <500ms | ✅ Achieved |
| SONA Adaptation | <0.05ms | 🔄 In Progress |
| Graph Build (1k) | <200ms | ✅ 2.78ms (71.9x headroom) |
| PageRank (1k) | <100ms | ✅ 12.21ms (8.2x headroom) |
| Insight Recording | <5ms/each | ✅ 0.12ms (41x headroom) |
| Consolidation | <500ms | ✅ 0.26ms (1,955x headroom) |
| Knowledge Transfer | <100ms | ✅ 1.25ms (80x headroom) |

---

## Integration Ecosystem

### Integrated Packages
| Package | Version | Purpose |
|---------|---------|---------|
| agentic-flow | 3.0.0-alpha.1 | Core coordination + ReasoningBank + Router |
| agentdb | 3.0.0-alpha.10 | Vector database + 8 controllers |
| @ruvector/attention | 0.1.3 | Flash attention |
| @ruvector/sona | 0.1.5 | Neural learning |

### Optional Integrations
| Package | Command |
|---------|---------|
| ruv-swarm | `npx ruv-swarm mcp start` |
| flow-nexus | `npx flow-nexus@latest mcp start` |
| agentic-jujutsu | `npx agentic-jujutsu@latest` |

### MCP Server Setup
```bash
# Add Claude Flow MCP
claude mcp add claude-flow -- npx -y @claude-flow/cli@latest

# Optional servers
claude mcp add ruv-swarm -- npx -y ruv-swarm mcp start
claude mcp add flow-nexus -- npx -y flow-nexus@latest mcp start
```

---

## Quick Reference

### Essential Commands
```bash
# Setup
npx @claude-flow/cli@latest init --wizard
npx @claude-flow/cli@latest daemon start
npx @claude-flow/cli@latest doctor --fix

# Swarm
npx @claude-flow/cli@latest swarm init --topology hierarchical --max-agents 8
npx @claude-flow/cli@latest swarm status

# Agents
npx @claude-flow/cli@latest agent spawn -t coder
npx @claude-flow/cli@latest agent list

# Memory
npx @claude-flow/cli@latest memory search --query "patterns"

# Hooks
npx @claude-flow/cli@latest hooks pre-task --description "task"
npx @claude-flow/cli@latest hooks worker dispatch --trigger optimize
```

### File Structure
```
.claude-flow/
├── config.yaml      # Runtime configuration
├── CAPABILITIES.md  # This file
├── data/            # Memory storage
├── logs/            # Operation logs
├── sessions/        # Session state
├── hooks/           # Custom hooks
├── agents/          # Agent configs
└── workflows/       # Workflow templates
```

---

**Full Documentation**: https://github.com/ruvnet/claude-flow
**Issues**: https://github.com/ruvnet/claude-flow/issues
