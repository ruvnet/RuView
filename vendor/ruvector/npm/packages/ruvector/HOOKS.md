# RuVector Hooks for Claude Code

Self-learning intelligence hooks that enhance Claude Code with Q-learning, vector memory, and automatic agent routing.

## Quick Start

```bash
# Full setup: hooks + pretrain + optimized agents
npx ruvector hooks init --pretrain --build-agents quality

# Or step by step:
npx ruvector hooks init          # Setup hooks
npx ruvector hooks pretrain      # Analyze repository
npx ruvector hooks build-agents  # Generate agent configs
```

## What It Does

RuVector hooks integrate with Claude Code to provide:

| Feature | Description |
|---------|-------------|
| **Agent Routing** | Suggests the best agent for each file type based on learned patterns |
| **Co-edit Patterns** | Predicts "likely next files" from git history |
| **Vector Memory** | Semantic recall of project context |
| **Command Analysis** | Risk assessment for bash commands |
| **Self-Learning** | Q-learning improves suggestions over time |

## Commands

### Initialization

```bash
# Full configuration
npx ruvector hooks init

# With pretrain and agent building
npx ruvector hooks init --pretrain --build-agents security

# Minimal (basic hooks only)
npx ruvector hooks init --minimal

# Options
--force           # Overwrite existing settings
--minimal         # Basic hooks only
--pretrain        # Run pretrain after init
--build-agents    # Generate optimized agents (quality|speed|security|testing|fullstack)
--no-claude-md    # Skip CLAUDE.md creation
--no-permissions  # Skip permissions config
--no-env          # Skip environment variables
--no-gitignore    # Skip .gitignore update
--no-mcp          # Skip MCP server config
--no-statusline   # Skip status line config
```

### Pretrain

Analyze your repository to bootstrap intelligence:

```bash
npx ruvector hooks pretrain

# Options
--depth <n>     # Git history depth (default: 100)
--verbose       # Show detailed progress
--skip-git      # Skip git history analysis
--skip-files    # Skip file structure analysis
```

**What it learns:**
- File type → Agent mapping (`.rs` → rust-developer)
- Co-edit patterns from git history
- Directory → Agent mapping
- Project context memories

### Build Agents

Generate optimized `.claude/agents/` configurations:

```bash
npx ruvector hooks build-agents --focus quality

# Focus modes
--focus quality   # Code quality, best practices (default)
--focus speed     # Rapid development, prototyping
--focus security  # OWASP, input validation, encryption
--focus testing   # TDD, comprehensive coverage
--focus fullstack # Balanced frontend/backend/database

# Options
--output <dir>    # Output directory (default: .claude/agents)
--format <fmt>    # yaml, json, or md (default: yaml)
--include-prompts # Include system prompts in agent configs
```

### Verification & Diagnostics

```bash
# Check if hooks are working
npx ruvector hooks verify

# Diagnose and fix issues
npx ruvector hooks doctor
npx ruvector hooks doctor --fix
```

### Data Management

```bash
# View statistics
npx ruvector hooks stats

# Export intelligence data
npx ruvector hooks export -o backup.json
npx ruvector hooks export --include-all

# Import intelligence data
npx ruvector hooks import backup.json
npx ruvector hooks import backup.json --merge
```

### Memory Operations

```bash
# Store context in vector memory
npx ruvector hooks remember "API uses JWT auth" -t project

# Semantic search memory
npx ruvector hooks recall "authentication"

# Route a task to best agent
npx ruvector hooks route "implement user login"
```

## Hook Events

| Event | Trigger | RuVector Action |
|-------|---------|-----------------|
| **PreToolUse** | Before Edit/Write/Bash | Agent routing, file analysis, command risk |
| **PostToolUse** | After Edit/Write/Bash | Q-learning update, pattern recording |
| **SessionStart** | Conversation begins | Load intelligence, display stats |
| **Stop** | Conversation ends | Save learning data |
| **UserPromptSubmit** | User sends message | Context suggestions |
| **PreCompact** | Before context compaction | Preserve important context |
| **Notification** | Any notification | Track events for learning |

## Generated Files

After running `hooks init`:

```
your-project/
├── .claude/
│   ├── settings.json      # Hooks configuration
│   ├── statusline.sh      # Status bar script
│   └── agents/            # Generated agents (with --build-agents)
│       ├── rust-specialist.yaml
│       ├── typescript-specialist.yaml
│       ├── test-architect.yaml
│       └── project-coordinator.yaml
├── .ruvector/
│   └── intelligence.json  # Learning data
├── CLAUDE.md              # Project documentation
└── .gitignore             # Updated with .ruvector/
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUVECTOR_INTELLIGENCE_ENABLED` | `true` | Enable/disable intelligence |
| `RUVECTOR_LEARNING_RATE` | `0.1` | Q-learning rate (0.0-1.0) |
| `RUVECTOR_MEMORY_BACKEND` | `rvlite` | Memory storage backend |
| `INTELLIGENCE_MODE` | `treatment` | A/B testing mode |

## Example Output

### Agent Routing
```
🧠 Intelligence Analysis:
   📁 src/api/routes.ts
   🤖 Recommended: typescript-developer (85% confidence)
      → learned from 127 .ts files in repo
   📎 Likely next files:
      - src/api/handlers.ts (12 co-edits)
      - src/types/api.ts (8 co-edits)
```

### Command Analysis
```
🧠 Command Analysis:
   📦 Category: rust
   🏷️  Type: test
   ✅ Risk: LOW
```

## Best Practices

1. **Run pretrain on existing repos** — Bootstrap intelligence before starting work
2. **Use focus modes** — Match agent generation to your current task
3. **Export before major changes** — Backup learning data
4. **Let it learn** — Intelligence improves with each edit

## Troubleshooting

```bash
# Check setup
npx ruvector hooks verify

# Fix common issues
npx ruvector hooks doctor --fix

# Reset and reinitialize
npx ruvector hooks init --force --pretrain
```

## Links

- [RuVector GitHub](https://github.com/ruvnet/ruvector)
- [npm Package](https://www.npmjs.com/package/ruvector)
- [Claude Code Documentation](https://docs.anthropic.com/claude-code)
