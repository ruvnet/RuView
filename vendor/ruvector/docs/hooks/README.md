# RuVector Hooks System Documentation

Intelligent hooks for Claude Code that provide automatic agent assignment, code formatting, neural pattern training, and cross-session memory persistence.

> **Implementation Status**: ✅ **FULLY IMPLEMENTED** - Both implementations are now functional:
> - **Rust CLI**: `ruvector hooks <command>` (portable, high-performance)
> - **Node.js**: `.claude/intelligence/cli.js` (legacy compatibility)

## Available Implementations

```bash
# Rust CLI (recommended - faster, portable)
cargo run --bin ruvector -- hooks stats
cargo run --bin ruvector -- hooks pre-edit <file>
cargo run --bin ruvector -- hooks post-edit <file> --success

# Node.js (legacy - still functional)
node .claude/intelligence/cli.js stats
node .claude/intelligence/cli.js pre-edit <file>
```

## Quick Navigation

| Document | Description |
|----------|-------------|
| [User Guide](USER_GUIDE.md) | Getting started, setup, and basic usage |
| [CLI Reference](CLI_REFERENCE.md) | Complete CLI command documentation |
| [Architecture](ARCHITECTURE.md) | Technical design and system internals |
| [Migration Guide](MIGRATION.md) | Upgrading from claude-flow or legacy systems |
| [Troubleshooting](TROUBLESHOOTING.md) | Common issues and solutions |

## Developer Documents

| Document | Description |
|----------|-------------|
| [Implementation Plan](IMPLEMENTATION_PLAN.md) | SPARC-GOAP implementation roadmap |
| [MVP Checklist](MVP_CHECKLIST.md) | Development checklist for MVP release |
| [Review Report](REVIEW_REPORT.md) | Detailed code review and recommendations |
| [Review Summary](REVIEW_SUMMARY.md) | Executive summary of review findings |

---

## What Are Hooks?

Hooks are automated actions that execute before or after Claude Code tool operations. They enable:

- **Intelligent Agent Assignment**: Automatically assign the best agent for each file type
- **Code Quality**: Format, lint, and validate code automatically
- **Memory Persistence**: Store decisions and context across sessions
- **Neural Learning**: Continuously improve from successful patterns
- **Swarm Coordination**: Synchronize knowledge across multi-agent workflows

## System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Claude Code Session                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐  │
│  │ PreTool  │───►│  Tool    │───►│ PostTool │───►│  Result  │  │
│  │   Hook   │    │ Execute  │    │   Hook   │    │          │  │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘  │
│       │                                │                         │
│       ▼                                ▼                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Intelligence Layer                          │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐    │   │
│  │  │Q-Learn  │  │ Vector  │  │ Pattern │  │  Agent  │    │   │
│  │  │ Table   │  │ Memory  │  │ Training│  │ Router  │    │   │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Quick Start

### 1. Initialize Hooks

```bash
# Using npx (recommended)
npx ruvector hooks init

# Or using claude-flow
npx claude-flow init --hooks
```

### 2. Install into Claude Code

```bash
npx ruvector hooks install
```

### 3. Verify Setup

```bash
npx ruvector hooks stats
```

## Hook Types

### Pre-Operation Hooks

Execute **before** Claude Code operations:

| Hook | Trigger | Purpose |
|------|---------|---------|
| `pre-edit` | Write, Edit, MultiEdit | Agent assignment, syntax validation |
| `pre-bash` | Bash commands | Safety checks, resource estimation |
| `pre-task` | Task spawning | Auto-spawn agents, load memory |
| `pre-search` | Grep, Glob | Cache checking, query optimization |

### Post-Operation Hooks

Execute **after** Claude Code operations:

| Hook | Trigger | Purpose |
|------|---------|---------|
| `post-edit` | Write, Edit, MultiEdit | Formatting, memory storage, training |
| `post-bash` | Bash commands | Logging, metrics, error detection |
| `post-task` | Task completion | Performance analysis, learning export |
| `post-search` | Grep, Glob | Caching, pattern improvement |

### Session Hooks

Manage session lifecycle:

| Hook | Trigger | Purpose |
|------|---------|---------|
| `session-start` | Session begins | Context loading, initialization |
| `session-restore` | Manual restore | Restore previous session state |
| `session-end` / `Stop` | Session ends | Persist state, export metrics |

### Compact Hooks

Execute during context compaction:

| Hook | Matcher | Purpose |
|------|---------|---------|
| `PreCompact` | `manual` | Display learned patterns during manual compact |
| `PreCompact` | `auto` | Show learning stats during auto-compact |

## Configuration

Hooks are configured in `.claude/settings.json`:

```json
{
  "env": {
    "RUVECTOR_INTELLIGENCE_ENABLED": "true",
    "INTELLIGENCE_MODE": "treatment",
    "RUVECTOR_MEMORY_BACKEND": "rvlite"
  },
  "permissions": {
    "allow": ["Bash(cargo:*)", "Bash(git:*)", "Bash(.claude/hooks:*)"],
    "deny": ["Bash(rm -rf /)", "Bash(cargo publish:*)"]
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{
          "type": "command",
          "timeout": 3000,
          "command": "npx ruvector hooks pre-command \"$CMD\""
        }]
      },
      {
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [{
          "type": "command",
          "timeout": 3000,
          "command": "npx ruvector hooks pre-edit \"$FILE\""
        }]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{
          "type": "command",
          "command": "npx ruvector hooks post-command \"$CMD\" \"$SUCCESS\""
        }]
      },
      {
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [{
          "type": "command",
          "command": "npx ruvector hooks post-edit \"$FILE\""
        }]
      }
    ],
    "PreCompact": [
      {
        "matcher": "manual",
        "hooks": [{
          "type": "command",
          "command": "npx ruvector hooks compact-context --mode manual"
        }]
      },
      {
        "matcher": "auto",
        "hooks": [{
          "type": "command",
          "command": "npx ruvector hooks compact-context --mode auto"
        }]
      }
    ],
    "SessionStart": [
      {
        "hooks": [{
          "type": "command",
          "timeout": 5000,
          "command": "npx ruvector hooks session-start"
        }]
      }
    ],
    "Stop": [
      {
        "hooks": [{
          "type": "command",
          "command": "npx ruvector hooks session-end --persist-state"
        }]
      }
    ]
  },
  "includeCoAuthoredBy": true,
  "enabledMcpjsonServers": ["claude-flow", "ruv-swarm"],
  "statusLine": {
    "type": "command",
    "command": ".claude/statusline-command.sh"
  }
}
```

### Configuration Sections

| Section | Description |
|---------|-------------|
| `env` | Environment variables for hooks |
| `permissions` | Allow/deny rules for commands |
| `hooks` | Hook definitions by event type |
| `includeCoAuthoredBy` | Add co-author attribution |
| `enabledMcpjsonServers` | MCP servers to enable |
| `statusLine` | Custom status line command |

## Intelligence Layer

The hooks system includes a self-learning intelligence layer:

### Q-Learning Patterns

Learns optimal actions from experience:

- State-action pair tracking
- Reward-based learning
- Decay over time for relevance

### Vector Memory

Semantic search over past decisions:

- 128-dimensional embeddings
- HNSW indexing for fast retrieval
- Persistent storage with rvlite

### Agent Routing

Intelligent agent assignment:

- File type → agent type mapping
- Confidence scoring
- Fallback strategies

## Directory Structure

After initialization:

```
.ruvector/
├── config.toml              # Project configuration
├── intelligence/
│   ├── memory.json          # Vector memory entries
│   ├── patterns.json        # Q-learning patterns
│   ├── trajectories.json    # Learning trajectories
│   ├── feedback.json        # User feedback tracking
│   └── memory.rvdb          # RvLite vector database
└── .gitignore
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUVECTOR_HOME` | `~/.ruvector` | Global patterns directory |
| `RUVECTOR_DATA_DIR` | `./.ruvector` | Project-local data |
| `RUVECTOR_INTELLIGENCE_ENABLED` | `true` | Enable/disable intelligence |
| `INTELLIGENCE_MODE` | `treatment` | A/B test group |

## Performance

The hooks system is designed for minimal overhead:

- **Hook execution**: <50ms typical
- **Memory lookup**: <10ms with HNSW
- **Pattern training**: Async, non-blocking
- **Total overhead**: <100ms per operation

## Integration

Hooks integrate with:

- **Claude Code**: Native tool hooks
- **Claude Flow**: MCP swarm coordination
- **Git**: Pre-commit and post-commit hooks
- **RvLite**: Vector database storage
- **Neural Training**: Pattern improvement

## Next Steps

1. Read the [User Guide](USER_GUIDE.md) for detailed setup
2. Explore the [CLI Reference](CLI_REFERENCE.md) for all commands
3. Check [Architecture](ARCHITECTURE.md) for internals
4. See [Migration Guide](MIGRATION.md) if upgrading

---

## Support

- **Issues**: [GitHub Issues](https://github.com/ruvnet/ruvector/issues)
- **Documentation**: This directory
- **Examples**: `.claude/commands/hooks/`
