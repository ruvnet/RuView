# RuVector Hooks User Guide

A comprehensive guide to setting up and using the RuVector hooks system for intelligent Claude Code automation.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Prerequisites](#prerequisites)
3. [Installation](#installation)
4. [Basic Usage](#basic-usage)
5. [Configuration](#configuration)
6. [Working with Hooks](#working-with-hooks)
7. [Intelligence Features](#intelligence-features)
8. [Best Practices](#best-practices)
9. [Examples](#examples)

---

## Quick Start

Get up and running in under 5 minutes:

```bash
# Step 1: Initialize hooks in your project
npx ruvector hooks init

# Step 2: Install hooks into Claude Code
npx ruvector hooks install

# Step 3: Verify installation
npx ruvector hooks stats

# Done! Hooks are now active
```

---

## Prerequisites

### Required

- **Node.js 18+**: Required for hook execution
- **Claude Code**: The hooks integrate with Claude Code's hook system
- **npm or pnpm**: Package manager for installation

### Optional

- **jq**: JSON processing for hook data (auto-installed on most systems)
- **Git**: For version control integration
- **claude-flow CLI**: For advanced swarm coordination

### Verify Prerequisites

```bash
# Check Node.js version
node --version  # Should be 18.x or higher

# Check npm
npm --version

# Check jq (optional)
which jq || echo "jq not installed (optional)"
```

---

## Installation

### Method 1: npx (Recommended)

The simplest way to install hooks:

```bash
# Initialize in any project
cd your-project
npx ruvector hooks init
npx ruvector hooks install
```

### Method 2: Global Installation

For frequent use across projects:

```bash
# Install globally
npm install -g @ruvector/cli

# Then use directly
ruvector hooks init
ruvector hooks install
```

### Method 3: With Claude Flow

If using claude-flow for swarm coordination:

```bash
# Initialize with full hook support
npx claude-flow init --hooks

# Hooks are automatically configured
```

### Verify Installation

```bash
# Check hooks are installed
npx ruvector hooks stats

# Expected output:
# RuVector Intelligence Statistics
# --------------------------------
# Patterns: 0 (new installation)
# Memories: 0
# Status: Ready
```

---

## Basic Usage

### How Hooks Work

Hooks automatically execute when you use Claude Code:

1. **Pre-hooks** run before tool execution
2. **Post-hooks** run after tool execution
3. **Session hooks** run at session boundaries

### Automatic Behavior

Once installed, hooks work automatically:

```bash
# When you edit a file in Claude Code:
# 1. pre-edit hook checks file type, assigns agent
# 2. Claude Code performs the edit
# 3. post-edit hook formats code, stores in memory

# When you run a command:
# 1. pre-bash hook validates safety
# 2. Command executes
# 3. post-bash hook logs result, updates metrics
```

### Manual Hook Execution

You can also run hooks manually:

```bash
# Test pre-edit hook
npx ruvector hooks pre-edit --file "src/app.ts"

# Test post-edit hook
npx ruvector hooks post-edit --file "src/app.ts" --success true

# Run session hooks
npx ruvector hooks session-start
npx ruvector hooks session-end --export-metrics
```

---

## Configuration

### Configuration File

After `hooks init`, a configuration file is created:

**`.ruvector/config.toml`**:

```toml
[intelligence]
enabled = true
learning_rate = 0.1
ab_test_group = "treatment"
use_hyperbolic_distance = true
curvature = 1.0

[memory]
backend = "rvdb"
max_memories = 50000
dimensions = 128

[patterns]
decay_half_life_days = 7
min_q_value = -0.5
max_q_value = 0.8

[hooks]
pre_command_enabled = true
post_command_enabled = true
pre_edit_enabled = true
post_edit_enabled = true
session_start_enabled = true
session_end_enabled = true
timeout_ms = 3000
```

### Claude Code Settings

Hooks are registered in `.claude/settings.json`:

```json
{
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
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [{
          "type": "command",
          "command": "npx ruvector hooks post-edit \"$FILE\" \"true\""
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
    ]
  }
}
```

### Customizing Hooks

#### Add Protected File Detection

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [{
          "type": "command",
          "command": "npx ruvector hooks check-protected \"$FILE\""
        }]
      }
    ]
  }
}
```

#### Add Auto-Testing

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write",
        "hooks": [{
          "type": "command",
          "command": "test -f \"${FILE%.ts}.test.ts\" && npm test \"${FILE%.ts}.test.ts\"",
          "continueOnError": true
        }]
      }
    ]
  }
}
```

---

## Working with Hooks

### Pre-Edit Hook

Runs before file modifications:

```bash
npx ruvector hooks pre-edit --file "src/auth/login.ts"
```

**What it does:**
- Detects file type and language
- Assigns appropriate agent (e.g., TypeScript files → typescript-developer)
- Checks for existing patterns
- Validates syntax if enabled
- Creates backup if configured

**Output example:**
```json
{
  "continue": true,
  "agent": "typescript-developer",
  "confidence": 0.85,
  "syntaxValid": true,
  "warnings": []
}
```

### Post-Edit Hook

Runs after file modifications:

```bash
npx ruvector hooks post-edit --file "src/auth/login.ts" --success true
```

**What it does:**
- Records successful edit in trajectory
- Updates Q-learning patterns
- Stores context in vector memory
- Optionally formats code
- Trains neural patterns

### Session Hooks

#### Starting a Session

```bash
npx ruvector hooks session-start --session-id "feature-dev"
```

**Output:**
```
RuVector Intelligence Layer Active

Patterns: 131 state-action pairs
Memories: 4,247 vectors
Status: Ready
```

#### Ending a Session

```bash
npx ruvector hooks session-end --export-metrics --generate-summary
```

**What it does:**
- Persists memory state
- Exports session metrics
- Generates work summary
- Cleans up temporary files

---

## Intelligence Features

### Q-Learning

The hooks system learns from your actions:

```bash
# View learned patterns
npx ruvector hooks stats --verbose

# Output:
# Top Patterns:
# 1. edit_ts_in_src → typescript-developer (Q=0.82)
# 2. edit_rs_in_crates → rust-developer (Q=0.79)
# 3. cargo_test → success (Q=0.91)
```

### Agent Routing

Automatically assigns the best agent for each file:

| File Type | Assigned Agent | Confidence |
|-----------|---------------|------------|
| `*.ts`, `*.tsx` | typescript-developer | 85% |
| `*.rs` | rust-developer | 80% |
| `*.py` | python-developer | 78% |
| `*.go` | go-developer | 75% |
| `*.sql` | database-specialist | 70% |

### Memory Persistence

Decisions are stored for future reference:

```bash
# Check memory usage
npx ruvector hooks stats

# Output:
# Memories: 4,247 vectors
# Dimensions: 128
# Storage: 2.4 MB
```

### A/B Testing

Compare learning effectiveness:

```bash
# Treatment group (learning enabled)
INTELLIGENCE_MODE=treatment npx ruvector hooks pre-edit --file "test.ts"

# Control group (random baseline)
INTELLIGENCE_MODE=control npx ruvector hooks pre-edit --file "test.ts"
```

---

## Best Practices

### 1. Initialize Early

Set up hooks at the start of a project:

```bash
# After creating a new project
npx ruvector hooks init
npx ruvector hooks install
```

### 2. Use Meaningful Session IDs

Track work with descriptive sessions:

```bash
# Good: Descriptive sessions
npx ruvector hooks session-start --session-id "feature-auth-oauth2"
npx ruvector hooks session-start --session-id "bugfix-memory-leak-123"

# Avoid: Generic sessions
npx ruvector hooks session-start --session-id "session1"
```

### 3. Export Metrics Regularly

Capture performance data:

```bash
# At end of work session
npx ruvector hooks session-end --export-metrics --generate-summary
```

### 4. Review Pattern Quality

Check learning effectiveness:

```bash
# Weekly review
npx ruvector hooks stats --verbose

# Check calibration
# Good: 0.04-0.06 calibration error
# Bad: >0.15 calibration error
```

### 5. Keep Hooks Lightweight

Follow performance guidelines:

- Hook execution: <50ms
- Total overhead: <100ms per operation
- Async heavy operations
- Cache repeated lookups

### 6. Use Version Control

Track hook configurations:

```bash
# Add to git
git add .claude/settings.json
git add .ruvector/config.toml

# Ignore learning data (optional)
echo ".ruvector/intelligence/" >> .gitignore
```

---

## Examples

### Example 1: Development Workflow

```bash
# Start development session
npx ruvector hooks session-start --session-id "feature-user-profile"

# Work on files (hooks run automatically via Claude Code)
# - Pre-edit assigns agents
# - Post-edit formats and stores

# End session
npx ruvector hooks session-end \
  --session-id "feature-user-profile" \
  --export-metrics \
  --generate-summary
```

### Example 2: Debugging Session

```bash
# Start debug session
npx ruvector hooks session-start --session-id "debug-api-timeout"

# Load previous context
npx ruvector hooks session-restore --session-id "debug-api-timeout"

# Work on debugging...

# Export findings
npx ruvector hooks session-end \
  --session-id "debug-api-timeout" \
  --store-decisions \
  --generate-report
```

### Example 3: Multi-Agent Task

```bash
# Pre-task with agent spawning
npx claude-flow hook pre-task \
  --description "Implement OAuth2 authentication" \
  --auto-spawn-agents \
  --load-memory

# Agents work on files (hooks coordinate)

# Post-task analysis
npx claude-flow hook post-task \
  --task-id "oauth2-impl" \
  --analyze-performance \
  --export-learnings
```

### Example 4: Custom Rust Workflow

For Rust projects, specialized hooks are available:

```bash
# Pre-edit for Rust files
.claude/hooks/rust-check.sh src/lib.rs

# Post-edit with benchmarks
.claude/hooks/post-rust-edit.sh src/lib.rs true
```

---

## Environment Setup

### Required Environment Variables

```bash
# Enable intelligence (default: true)
export RUVECTOR_INTELLIGENCE_ENABLED=true

# Set A/B test group
export INTELLIGENCE_MODE=treatment

# Optional: Custom data directory
export RUVECTOR_DATA_DIR=.ruvector
```

### Recommended Shell Configuration

Add to `~/.bashrc` or `~/.zshrc`:

```bash
# RuVector hooks
export RUVECTOR_INTELLIGENCE_ENABLED=true
export INTELLIGENCE_MODE=treatment

# Alias for convenience
alias rv='npx ruvector'
alias rvhooks='npx ruvector hooks'
```

---

## Getting Help

### Check Status

```bash
npx ruvector hooks stats --verbose
```

### Debug Mode

```bash
# Enable debug output
export CLAUDE_FLOW_DEBUG=true

# Run with debug
npx ruvector hooks pre-edit --file "test.ts" --debug
```

### View Logs

```bash
# Check hook execution logs
cat .ruvector/logs/hooks-$(date +%Y-%m-%d).log
```

### Validate Configuration

```bash
# Check JSON syntax
npx ruvector hooks validate-config
```

---

## Next Steps

- [CLI Reference](CLI_REFERENCE.md) - Full command documentation
- [Architecture](ARCHITECTURE.md) - Technical details
- [Migration Guide](MIGRATION.md) - Upgrade from other systems
- [Troubleshooting](TROUBLESHOOTING.md) - Common issues
