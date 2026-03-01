# RuVector Hooks CLI Reference

Complete command-line reference for the RuVector hooks system.

> **Implementation Status**: ✅ **FULLY IMPLEMENTED**
> - **Rust CLI**: `ruvector hooks <command>` (recommended)
> - **Node.js**: `.claude/intelligence/cli.js` (legacy)

## Synopsis

**Rust CLI (Recommended):**
```bash
# Direct execution
cargo run --bin ruvector -- hooks <command> [options]

# After installation
ruvector hooks <command> [options]
```

**Node.js (Legacy):**
```bash
node .claude/intelligence/cli.js <command> [args]
```

---

## Commands Overview

### Core Commands

| Command | Description |
|---------|-------------|
| `init` | Initialize hooks system in current project |
| `install` | Install hooks into Claude Code settings |
| `migrate` | Migrate learning data from other sources |
| `stats` | Display learning statistics |
| `export` | Export learned patterns |
| `import` | Import patterns from file |
| `enable` | Enable hooks system |
| `disable` | Disable hooks system |
| `validate-config` | Validate hook configuration |

### Hook Execution Commands

| Command | Description |
|---------|-------------|
| `pre-edit` | Pre-edit intelligence (agent assignment, validation) |
| `post-edit` | Post-edit learning (record outcome, suggest next) |
| `pre-command` | Pre-command intelligence (safety check) |
| `post-command` | Post-command learning (error patterns) |

### Session Commands

| Command | Description |
|---------|-------------|
| `session-start` | Start a new session |
| `session-end` | End current session |
| `session-restore` | Restore a previous session |

### Memory Commands

| Command | Description |
|---------|-------------|
| `remember` | Store content in vector memory |
| `recall` | Search memory semantically |
| `learn` | Record learning trajectory |
| `suggest` | Get best action suggestion |
| `route` | Route task to best agent |

### V3 Intelligence Features

| Command | Description |
|---------|-------------|
| `record-error` | Record error for pattern learning |
| `suggest-fix` | Get suggested fixes for error code |
| `suggest-next` | Suggest next files to edit |
| `should-test` | Check if tests should run |

### Swarm/Hive-Mind Commands

| Command | Description |
|---------|-------------|
| `swarm-register` | Register agent in swarm |
| `swarm-coordinate` | Record agent coordination |
| `swarm-optimize` | Optimize task distribution |
| `swarm-recommend` | Get best agent for task type |
| `swarm-heal` | Handle agent failure |
| `swarm-stats` | Show swarm statistics |

---

## Core Commands

### `hooks init`

Initialize the hooks system in the current project.

**Syntax:**
```bash
npx ruvector hooks init [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--path` | `PATH` | `./.ruvector` | Custom directory location |
| `--global` | flag | false | Initialize global patterns directory |
| `--template` | `NAME` | `default` | Template: `default`, `minimal`, `advanced` |
| `--force` | flag | false | Overwrite existing configuration |

**Examples:**

```bash
# Basic initialization
npx ruvector hooks init

# Custom directory
npx ruvector hooks init --path .config/ruvector

# Minimal configuration
npx ruvector hooks init --template minimal

# Force reinitialize
npx ruvector hooks init --force
```

**Output:**
```
Initialized ruvector hooks in ./.ruvector
Created: .ruvector/config.toml
Created: .ruvector/intelligence/
Next: Run `npx ruvector hooks install` to add hooks to Claude Code
```

---

### `hooks install`

Install hooks into `.claude/settings.json`.

**Syntax:**
```bash
npx ruvector hooks install [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--force` | flag | false | Overwrite existing hooks |
| `--dry-run` | flag | false | Show changes without applying |
| `--template` | `PATH` | built-in | Use custom hook template |
| `--merge` | flag | true | Merge with existing settings |

**Examples:**

```bash
# Standard installation
npx ruvector hooks install

# Preview changes
npx ruvector hooks install --dry-run

# Force overwrite
npx ruvector hooks install --force

# Custom template
npx ruvector hooks install --template ./my-hooks.json
```

**Output:**
```
Hooks installed to .claude/settings.json
Backup created: .claude/settings.json.backup
Intelligence layer ready
```

---

### `hooks migrate`

Migrate learning data from other sources.

**Syntax:**
```bash
npx ruvector hooks migrate --from <PATH> [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--from` | `PATH` | required | Source data path |
| `--format` | `FORMAT` | auto-detect | Source format: `json`, `sqlite`, `csv` |
| `--merge` | flag | false | Merge with existing patterns |
| `--validate` | flag | false | Validate migration integrity |
| `--dry-run` | flag | false | Show what would be migrated |

**Examples:**

```bash
# Migrate from existing intelligence
npx ruvector hooks migrate --from .claude/intelligence

# Migrate from claude-flow memory
npx ruvector hooks migrate --from ~/.swarm/memory.db --format sqlite

# Merge with validation
npx ruvector hooks migrate --from ./patterns.json --merge --validate

# Preview migration
npx ruvector hooks migrate --from ./old-data --dry-run
```

**Output:**
```
Migrating from JSON files...
Imported 1,247 trajectories
Imported 89 Q-learning patterns
Converted 543 memories to vectors
Validation passed (100% integrity)
Completed in 3.2s
```

---

### `hooks stats`

Display learning statistics and system health.

**Syntax:**
```bash
npx ruvector hooks stats [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--verbose` | flag | false | Show detailed breakdown |
| `--json` | flag | false | Output as JSON |
| `--compare-global` | flag | false | Compare local vs global patterns |

**Examples:**

```bash
# Basic stats
npx ruvector hooks stats

# Detailed view
npx ruvector hooks stats --verbose

# JSON output for scripting
npx ruvector hooks stats --json

# Compare with global
npx ruvector hooks stats --compare-global
```

**Output (verbose):**
```
RuVector Intelligence Statistics
================================

Learning Data:
   Trajectories: 1,247
   Patterns: 89 (Q-learning states)
   Memories: 543 vectors
   Total size: 2.4 MB

Top Patterns:
   1. edit_rs_in_ruvector-core → successful-edit (Q=0.823)
   2. cargo_test → command-succeeded (Q=0.791)
   3. npm_build → command-succeeded (Q=0.654)

Recent Activity:
   Last trajectory: 2 hours ago
   A/B test group: treatment
   Calibration error: 0.042
```

---

### `hooks export`

Export learned patterns for sharing or backup.

**Syntax:**
```bash
npx ruvector hooks export --output <PATH> [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--output` | `PATH` | required | Output file path |
| `--format` | `FORMAT` | `json` | Format: `json`, `csv`, `sqlite` |
| `--include` | `TYPES` | `all` | Include: `patterns`, `memories`, `all` |
| `--compress` | flag | false | Compress with gzip |

**Examples:**

```bash
# Export all data
npx ruvector hooks export --output backup.json

# Export patterns only
npx ruvector hooks export --output patterns.json --include patterns

# Compressed export
npx ruvector hooks export --output backup.json.gz --compress

# CSV format
npx ruvector hooks export --output data.csv --format csv
```

**Output:**
```
Exported 89 patterns to team-patterns.json
Size: 45.2 KB
SHA256: 8f3b4c2a...
```

---

### `hooks import`

Import learned patterns from file.

**Syntax:**
```bash
npx ruvector hooks import --input <PATH> [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--input` | `PATH` | required | Input file path |
| `--merge` | flag | false | Merge with existing patterns |
| `--strategy` | `STRATEGY` | `prefer-local` | Merge strategy: `prefer-local`, `prefer-imported`, `average` |
| `--validate` | flag | false | Validate before importing |

**Examples:**

```bash
# Import patterns (replace)
npx ruvector hooks import --input patterns.json

# Merge with existing
npx ruvector hooks import --input team-patterns.json --merge

# Merge with strategy
npx ruvector hooks import --input patterns.json --merge --strategy average

# Validate first
npx ruvector hooks import --input data.json --validate
```

**Output:**
```
Importing patterns...
Imported 89 patterns
Merged with 67 existing patterns
New total: 123 patterns (33 updated, 56 unchanged)
```

---

### `hooks enable` / `hooks disable`

Enable or disable the hooks system.

**Syntax:**
```bash
npx ruvector hooks enable
npx ruvector hooks disable
```

**Examples:**

```bash
# Disable temporarily
npx ruvector hooks disable
# Output: Hooks disabled (set RUVECTOR_INTELLIGENCE_ENABLED=false)

# Re-enable
npx ruvector hooks enable
# Output: Hooks enabled (set RUVECTOR_INTELLIGENCE_ENABLED=true)
```

---

## Hook Execution Commands

### `hooks pre-edit`

Execute pre-edit validation and agent assignment.

**Syntax:**
```bash
npx ruvector hooks pre-edit --file <PATH> [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--file`, `-f` | `PATH` | required | File path to be edited |
| `--auto-assign-agent` | flag | true | Assign best agent |
| `--validate-syntax` | flag | false | Validate syntax |
| `--check-conflicts` | flag | false | Check for conflicts |
| `--backup-file` | flag | false | Create backup |

**Examples:**

```bash
# Basic pre-edit
npx ruvector hooks pre-edit --file src/auth/login.ts

# With validation
npx ruvector hooks pre-edit -f src/api.ts --validate-syntax

# Safe edit with backup
npx ruvector hooks pre-edit -f config.json --backup-file
```

**Output (JSON):**
```json
{
  "continue": true,
  "file": "src/auth/login.ts",
  "assignedAgent": "typescript-developer",
  "confidence": 0.85,
  "syntaxValid": true,
  "warnings": []
}
```

---

### `hooks post-edit`

Execute post-edit processing.

**Syntax:**
```bash
npx ruvector hooks post-edit --file <PATH> [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--file`, `-f` | `PATH` | required | File that was edited |
| `--success` | `BOOL` | true | Whether edit succeeded |
| `--auto-format` | flag | true | Format code |
| `--memory-key`, `-m` | `KEY` | auto | Memory storage key |
| `--train-patterns` | flag | false | Train neural patterns |
| `--validate-output` | flag | false | Validate result |

**Examples:**

```bash
# Basic post-edit
npx ruvector hooks post-edit --file src/app.ts

# With memory key
npx ruvector hooks post-edit -f src/auth.ts -m "auth/login-impl"

# Full processing
npx ruvector hooks post-edit -f src/utils.ts --train-patterns --validate-output
```

**Output (JSON):**
```json
{
  "file": "src/app.ts",
  "formatted": true,
  "formatterUsed": "prettier",
  "memorySaved": "edits/src/app.ts",
  "patternsTrained": 3,
  "success": true
}
```

---

### `hooks pre-command`

Execute pre-command safety check.

**Syntax:**
```bash
npx ruvector hooks pre-command <COMMAND> [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--check-safety` | flag | true | Verify command safety |
| `--estimate-resources` | flag | false | Estimate resource usage |
| `--require-confirmation` | flag | false | Require confirmation |

**Examples:**

```bash
# Basic check
npx ruvector hooks pre-command "npm install"

# With resource estimation
npx ruvector hooks pre-command "docker build ." --estimate-resources

# Dangerous command
npx ruvector hooks pre-command "rm -rf /tmp/*" --require-confirmation
```

---

### `hooks post-command`

Execute post-command logging.

**Syntax:**
```bash
npx ruvector hooks post-command <COMMAND> <SUCCESS> [STDERR]
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `COMMAND` | string | Command that was executed |
| `SUCCESS` | boolean | Whether command succeeded |
| `STDERR` | string | Error output (optional) |

**Examples:**

```bash
# Successful command
npx ruvector hooks post-command "npm test" true

# Failed command
npx ruvector hooks post-command "cargo build" false "error[E0308]"
```

---

## Session Commands

### `hooks session-start`

Initialize a new session.

**Syntax:**
```bash
npx ruvector hooks session-start [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--session-id`, `-s` | `ID` | auto-generated | Session identifier |
| `--load-context` | flag | false | Load previous context |
| `--init-agents` | flag | false | Initialize agents |

**Examples:**

```bash
# Auto-generated session
npx ruvector hooks session-start

# Named session
npx ruvector hooks session-start --session-id "feature-auth"

# With context loading
npx ruvector hooks session-start -s "debug-123" --load-context
```

**Output:**
```
RuVector Intelligence Layer Active

Session: feature-auth
Patterns: 131 state-action pairs
Memories: 4,247 vectors
Status: Ready
```

---

### `hooks session-end`

End and persist session state.

**Syntax:**
```bash
npx ruvector hooks session-end [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--session-id`, `-s` | `ID` | current | Session to end |
| `--save-state` | flag | true | Save session state |
| `--export-metrics` | flag | false | Export metrics |
| `--generate-summary` | flag | false | Generate summary |
| `--cleanup-temp` | flag | false | Remove temp files |

**Examples:**

```bash
# Basic end
npx ruvector hooks session-end

# With metrics and summary
npx ruvector hooks session-end --export-metrics --generate-summary

# Full cleanup
npx ruvector hooks session-end -s "debug-session" --cleanup-temp
```

**Output (JSON):**
```json
{
  "sessionId": "feature-auth",
  "duration": 7200000,
  "saved": true,
  "metrics": {
    "commandsRun": 145,
    "filesModified": 23,
    "tokensUsed": 85000
  },
  "summaryPath": "./sessions/feature-auth-summary.md"
}
```

---

### `hooks session-restore`

Restore a previous session.

**Syntax:**
```bash
npx ruvector hooks session-restore --session-id <ID> [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--session-id`, `-s` | `ID` | required | Session to restore |
| `--restore-memory` | flag | true | Restore memory state |
| `--restore-agents` | flag | false | Restore agent configs |

**Examples:**

```bash
# Restore session
npx ruvector hooks session-restore --session-id "feature-auth"

# Full restore
npx ruvector hooks session-restore -s "debug-123" --restore-agents
```

---

## Utility Commands

### `hooks validate-config`

Validate hook configuration.

**Syntax:**
```bash
npx ruvector hooks validate-config [OPTIONS]
```

**Options:**

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--file` | `PATH` | `.claude/settings.json` | Config file to validate |
| `--fix` | flag | false | Auto-fix issues |

**Examples:**

```bash
# Validate default config
npx ruvector hooks validate-config

# Validate custom file
npx ruvector hooks validate-config --file .claude/settings.json

# Auto-fix issues
npx ruvector hooks validate-config --fix
```

---

## Memory Commands

### `remember`

Store content in vector memory for semantic search.

**Syntax:**
```bash
node .claude/intelligence/cli.js remember <type> <content>
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `type` | string | Memory category: `edit`, `command`, `decision`, `error` |
| `content` | string | Content to store |

**Examples:**

```bash
# Store an edit memory
node .claude/intelligence/cli.js remember edit "implemented OAuth2 in auth.ts"

# Store a decision
node .claude/intelligence/cli.js remember decision "chose JWT over sessions for auth"
```

---

### `recall`

Search memory semantically.

**Syntax:**
```bash
node .claude/intelligence/cli.js recall <query>
```

**Examples:**

```bash
# Find related memories
node .claude/intelligence/cli.js recall "authentication implementation"

# Search for error patterns
node .claude/intelligence/cli.js recall "E0308 type mismatch"
```

**Output (JSON):**
```json
{
  "query": "authentication",
  "results": [
    {
      "type": "edit",
      "content": "implemented OAuth2 in auth.ts",
      "score": "0.85",
      "timestamp": "2025-12-27T10:30:00Z"
    }
  ]
}
```

---

### `route`

Route a task to the best agent based on learned patterns.

**Syntax:**
```bash
node .claude/intelligence/cli.js route <task> [OPTIONS]
```

**Options:**

| Option | Type | Description |
|--------|------|-------------|
| `--file` | `PATH` | File being worked on |
| `--crate` | `NAME` | Crate name for Rust projects |
| `--op` | `TYPE` | Operation type: `edit`, `test`, `review` |

**Examples:**

```bash
# Route based on file type
node .claude/intelligence/cli.js route "fix bug" --file src/auth.rs --crate ruvector-core

# Route a testing task
node .claude/intelligence/cli.js route "add tests" --file lib.rs --op test
```

**Output (JSON):**
```json
{
  "recommended": "rust-developer",
  "confidence": 0.82,
  "reasoning": "Learned from 47 similar edits"
}
```

---

## V3 Intelligence Features

### `record-error`

Record an error for pattern learning.

**Syntax:**
```bash
node .claude/intelligence/cli.js record-error <command> <stderr>
```

**Examples:**

```bash
# Record a Rust compilation error
node .claude/intelligence/cli.js record-error "cargo build" "error[E0308]: mismatched types"
```

**Output:**
```json
{
  "recorded": 1,
  "errors": [{"type": "rust", "code": "E0308"}]
}
```

---

### `suggest-fix`

Get suggested fixes for an error code based on past solutions.

**Syntax:**
```bash
node .claude/intelligence/cli.js suggest-fix <error-code>
```

**Examples:**

```bash
# Get fix suggestions for Rust error
node .claude/intelligence/cli.js suggest-fix "rust:E0308"

# Get fix for npm error
node .claude/intelligence/cli.js suggest-fix "npm:ERESOLVE"
```

**Output:**
```json
{
  "errorCode": "rust:E0308",
  "recentFixes": [
    "Add explicit type annotation",
    "Use .into() for conversion"
  ],
  "confidence": 0.75
}
```

---

### `suggest-next`

Suggest next files to edit based on edit sequence patterns.

**Syntax:**
```bash
node .claude/intelligence/cli.js suggest-next <file>
```

**Examples:**

```bash
# Get suggestions after editing lib.rs
node .claude/intelligence/cli.js suggest-next "crates/ruvector-core/src/lib.rs"
```

**Output:**
```json
[
  {"file": "mod.rs", "confidence": 0.85},
  {"file": "tests.rs", "confidence": 0.72}
]
```

---

### `should-test`

Check if tests should be run after editing a file.

**Syntax:**
```bash
node .claude/intelligence/cli.js should-test <file>
```

**Examples:**

```bash
node .claude/intelligence/cli.js should-test "src/lib.rs"
```

**Output:**
```json
{
  "suggest": true,
  "command": "cargo test -p ruvector-core",
  "reason": "Core library modified"
}
```

---

## Swarm/Hive-Mind Commands

### `swarm-register`

Register an agent in the swarm.

**Syntax:**
```bash
node .claude/intelligence/cli.js swarm-register <id> <type> [capabilities...]
```

**Examples:**

```bash
# Register a Rust developer agent
node .claude/intelligence/cli.js swarm-register agent-1 rust-developer testing optimization
```

---

### `swarm-coordinate`

Record coordination between agents.

**Syntax:**
```bash
node .claude/intelligence/cli.js swarm-coordinate <source> <destination> [weight]
```

**Examples:**

```bash
# Record coordination from coder to reviewer
node .claude/intelligence/cli.js swarm-coordinate coder-1 reviewer-1 1.5
```

---

### `swarm-optimize`

Optimize task distribution across agents.

**Syntax:**
```bash
node .claude/intelligence/cli.js swarm-optimize <task1> <task2> ...
```

**Examples:**

```bash
node .claude/intelligence/cli.js swarm-optimize "implement auth" "write tests" "review code"
```

---

### `swarm-recommend`

Get the best agent for a task type.

**Syntax:**
```bash
node .claude/intelligence/cli.js swarm-recommend <task-type> [capabilities...]
```

**Examples:**

```bash
node .claude/intelligence/cli.js swarm-recommend rust-development testing
```

---

### `swarm-heal`

Handle agent failure and recover.

**Syntax:**
```bash
node .claude/intelligence/cli.js swarm-heal <agent-id>
```

**Examples:**

```bash
node .claude/intelligence/cli.js swarm-heal agent-3
```

---

### `swarm-stats`

Show swarm statistics.

**Syntax:**
```bash
node .claude/intelligence/cli.js swarm-stats
```

**Output:**
```json
{
  "agents": 5,
  "activeAgents": 4,
  "totalCoordinations": 127,
  "avgCoordinationWeight": 1.2
}
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Migration error |
| 4 | Validation failed |
| 5 | Timeout |

---

## Environment Variables

### RuVector Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUVECTOR_HOME` | `~/.ruvector` | Global patterns directory |
| `RUVECTOR_DATA_DIR` | `./.ruvector` | Project-local data directory |
| `RUVECTOR_CLI_PATH` | auto-detected | Path to CLI binary |
| `RUVECTOR_INTELLIGENCE_ENABLED` | `true` | Enable/disable intelligence |
| `RUVECTOR_LEARNING_RATE` | `0.1` | Q-learning alpha parameter |
| `RUVECTOR_MEMORY_BACKEND` | `rvlite` | Memory backend: `rvlite`, `json` |
| `RUVECTOR_WASM_SIZE_LIMIT_KB` | `3072` | WASM size limit for rvlite |
| `INTELLIGENCE_MODE` | `treatment` | A/B test group: `treatment`, `control` |

### Claude Flow Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CLAUDE_FLOW_HOOKS_ENABLED` | `true` | Enable/disable all hooks |
| `CLAUDE_FLOW_AUTO_COMMIT` | `false` | Auto-commit after changes |
| `CLAUDE_FLOW_AUTO_PUSH` | `false` | Auto-push after commits |
| `CLAUDE_FLOW_TELEMETRY_ENABLED` | `true` | Enable telemetry |
| `CLAUDE_FLOW_REMOTE_EXECUTION` | `true` | Allow remote execution |
| `CLAUDE_FLOW_CHECKPOINTS_ENABLED` | `true` | Enable session checkpoints |
| `CLAUDE_FLOW_DEBUG` | `false` | Enable debug output |

---

## See Also

- [User Guide](USER_GUIDE.md) - Getting started guide
- [Architecture](ARCHITECTURE.md) - Technical details
- [Migration Guide](MIGRATION.md) - Upgrade from other systems
- [Troubleshooting](TROUBLESHOOTING.md) - Common issues
