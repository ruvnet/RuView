---
name: Hooks Automation
description: Automated coordination, formatting, and learning from Claude Code operations using intelligent hooks with MCP integration. Includes pre/post task hooks, session management, Git integration, memory coordination, and neural pattern training for enhanced development workflows.
---

# Hooks Automation

Intelligent automation system that coordinates, validates, and learns from Claude Code operations through hooks integrated with MCP tools and neural pattern training.

## What This Skill Does

This skill provides a comprehensive hook system that automatically manages development operations, coordinates swarm agents, maintains session state, and continuously learns from coding patterns. It enables automated agent assignment, code formatting, performance tracking, and cross-session memory persistence.

**Key Capabilities:**
- **Pre-Operation Hooks**: Validate, prepare, and auto-assign agents before operations
- **Post-Operation Hooks**: Format, analyze, and train patterns after operations
- **Session Management**: Persist state, restore context, generate summaries
- **Memory Coordination**: Synchronize knowledge across swarm agents
- **Git Integration**: Automated commit hooks with quality verification
- **Neural Training**: Continuous learning from successful patterns
- **MCP Integration**: Seamless coordination with swarm tools

## Prerequisites

**Required:**
- Claude Flow CLI installed (`npm install -g claude-flow@alpha`)
- Claude Code with hooks enabled
- `.claude/settings.json` with hook configurations

**Optional:**
- MCP servers configured (claude-flow, ruv-swarm, flow-nexus)
- Git repository for version control
- Testing framework for quality verification

## Quick Start

### Initialize Hooks System

```bash
# Initialize with default hooks configuration
npx claude-flow init --hooks
```

This creates:
- `.claude/settings.json` with pre-configured hooks
- Hook command documentation in `.claude/commands/hooks/`
- Default hook handlers for common operations

### Basic Hook Usage

```bash
# Pre-task hook (auto-spawns agents)
npx claude-flow hook pre-task --description "Implement authentication"

# Post-edit hook (auto-formats and stores in memory)
npx claude-flow hook post-edit --file "src/auth.js" --memory-key "auth/login"

# Session end hook (saves state and metrics)
npx claude-flow hook session-end --session-id "dev-session" --export-metrics
```

---

## Complete Guide

### Available Hooks

#### Pre-Operation Hooks

Hooks that execute BEFORE operations to prepare and validate:

**pre-edit** - Validate and assign agents before file modifications
```bash
npx claude-flow hook pre-edit [options]

Options:
  --file, -f <path>         File path to be edited
  --auto-assign-agent       Automatically assign best agent (default: true)
  --validate-syntax         Pre-validate syntax before edit
  --check-conflicts         Check for merge conflicts
  --backup-file             Create backup before editing

Examples:
  npx claude-flow hook pre-edit --file "src/auth/login.js"
  npx claude-flow hook pre-edit -f "config/db.js" --validate-syntax
  npx claude-flow hook pre-edit -f "production.env" --backup-file --check-conflicts
```

**Features:**
- Auto agent assignment based on file type
- Syntax validation to prevent broken code
- Conflict detection for concurrent edits
- Automatic file backups for safety

**pre-bash** - Check command safety and resource requirements
```bash
npx claude-flow hook pre-bash --command <cmd>

Options:
  --command, -c <cmd>       Command to validate
  --check-safety            Verify command safety (default: true)
  --estimate-resources      Estimate resource usage
  --require-confirmation    Request user confirmation for risky commands

Examples:
  npx claude-flow hook pre-bash -c "rm -rf /tmp/cache"
  npx claude-flow hook pre-bash --command "docker build ." --estimate-resources
```

**Features:**
- Command safety validation
- Resource requirement estimation
- Destructive command confirmation
- Permission checks

**pre-task** - Auto-spawn agents and prepare for complex tasks
```bash
npx claude-flow hook pre-task [options]

Options:
  --description, -d <text>  Task description for context
  --auto-spawn-agents       Automatically spawn required agents (default: true)
  --load-memory             Load relevant memory from previous sessions
  --optimize-topology       Select optimal swarm topology
  --estimate-complexity     Analyze task complexity

Examples:
  npx claude-flow hook pre-task --description "Implement user authentication"
  npx claude-flow hook pre-task -d "Continue API dev" --load-memory
  npx claude-flow hook pre-task -d "Refactor codebase" --optimize-topology
```

**Features:**
- Automatic agent spawning based on task analysis
- Memory loading for context continuity
- Topology optimization for task structure
- Complexity estimation and time prediction

**pre-search** - Prepare and optimize search operations
```bash
npx claude-flow hook pre-search --query <query>

Options:
  --query, -q <text>        Search query
  --check-cache             Check cache first (default: true)
  --optimize-query          Optimize search pattern

Examples:
  npx claude-flow hook pre-search -q "authentication middleware"
```

**Features:**
- Cache checking for faster results
- Query optimization
- Search pattern improvement

#### Post-Operation Hooks

Hooks that execute AFTER operations to process and learn:

**post-edit** - Auto-format, validate, and update memory
```bash
npx claude-flow hook post-edit [options]

Options:
  --file, -f <path>         File path that was edited
  --auto-format             Automatically format code (default: true)
  --memory-key, -m <key>    Store edit context in memory
  --train-patterns          Train neural patterns from edit
  --validate-output         Validate edited file

Examples:
  npx claude-flow hook post-edit --file "src/components/Button.jsx"
  npx claude-flow hook post-edit -f "api/auth.js" --memory-key "auth/login"
  npx claude-flow hook post-edit -f "utils/helpers.ts" --train-patterns
```

**Features:**
- Language-specific auto-formatting (Prettier, Black, gofmt)
- Memory storage for edit context and decisions
- Neural pattern training for continuous improvement
- Output validation with linting

**post-bash** - Log execution and update metrics
```bash
npx claude-flow hook post-bash --command <cmd>

Options:
  --command, -c <cmd>       Command that was executed
  --log-output              Log command output (default: true)
  --update-metrics          Update performance metrics
  --store-result            Store result in memory

Examples:
  npx claude-flow hook post-bash -c "npm test" --update-metrics
```

**Features:**
- Command execution logging
- Performance metric tracking
- Result storage for analysis
- Error pattern detection

**post-task** - Performance analysis and decision storage
```bash
npx claude-flow hook post-task [options]

Options:
  --task-id, -t <id>        Task identifier for tracking
  --analyze-performance     Generate performance metrics (default: true)
  --store-decisions         Save task decisions to memory
  --export-learnings        Export neural pattern learnings
  --generate-report         Create task completion report

Examples:
  npx claude-flow hook post-task --task-id "auth-implementation"
  npx claude-flow hook post-task -t "api-refactor" --analyze-performance
  npx claude-flow hook post-task -t "bug-fix-123" --store-decisions
```

**Features:**
- Execution time and token usage measurement
- Decision and implementation choice recording
- Neural learning pattern export
- Completion report generation

**post-search** - Cache results and improve patterns
```bash
npx claude-flow hook post-search --query <query> --results <path>

Options:
  --query, -q <text>        Original search query
  --results, -r <path>      Results file path
  --cache-results           Cache for future use (default: true)
  --train-patterns          Improve search patterns

Examples:
  npx claude-flow hook post-search -q "auth" -r "results.json" --train-patterns
```

**Features:**
- Result caching for faster subsequent searches
- Search pattern improvement
- Relevance scoring

#### MCP Integration Hooks

Hooks that coordinate with MCP swarm tools:

**mcp-initialized** - Persist swarm configuration
```bash
npx claude-flow hook mcp-initialized --swarm-id <id>

Features:
- Save swarm topology and configuration
- Store agent roster in memory
- Initialize coordination namespace
```

**agent-spawned** - Update agent roster and memory
```bash
npx claude-flow hook agent-spawned --agent-id <id> --type <type>

Features:
- Register agent in coordination memory
- Update agent roster
- Initialize agent-specific memory namespace
```

**task-orchestrated** - Monitor task progress
```bash
npx claude-flow hook task-orchestrated --task-id <id>

Features:
- Track task progress through memory
- Monitor agent assignments
- Update coordination state
```

**neural-trained** - Save pattern improvements
```bash
npx claude-flow hook neural-trained --pattern <name>

Features:
- Export trained neural patterns
- Update coordination models
- Share learning across agents
```

#### Memory Coordination Hooks

**memory-write** - Triggered when agents write to coordination memory
```bash
Features:
- Validate memory key format
- Update cross-agent indexes
- Trigger dependent hooks
- Notify subscribed agents
```

**memory-read** - Triggered when agents read from coordination memory
```bash
Features:
- Log access patterns
- Update popularity metrics
- Preload related data
- Track usage statistics
```

**memory-sync** - Synchronize memory across swarm agents
```bash
npx claude-flow hook memory-sync --namespace <ns>

Features:
- Sync memory state across agents
- Resolve conflicts
- Propagate updates
- Maintain consistency
```

#### Session Hooks

**session-start** - Initialize new session
```bash
npx claude-flow hook session-start --session-id <id>

Options:
  --session-id, -s <id>     Session identifier
  --load-context            Load context from previous session
  --init-agents             Initialize required agents

Features:
- Create session directory
- Initialize metrics tracking
- Load previous context
- Set up coordination namespace
```

**session-restore** - Load previous session state
```bash
npx claude-flow hook session-restore --session-id <id>

Options:
  --session-id, -s <id>     Session to restore
  --restore-memory          Restore memory state (default: true)
  --restore-agents          Restore agent configurations

Examples:
  npx claude-flow hook session-restore --session-id "swarm-20241019"
  npx claude-flow hook session-restore -s "feature-auth" --restore-memory
```

**Features:**
- Load previous session context
- Restore memory state and decisions
- Reconfigure agents to previous state
- Resume in-progress tasks

**session-end** - Cleanup and persist session state
```bash
npx claude-flow hook session-end [options]

Options:
  --session-id, -s <id>     Session identifier to end
  --save-state              Save current session state (default: true)
  --export-metrics          Export session metrics
  --generate-summary        Create session summary
  --cleanup-temp            Remove temporary files

Examples:
  npx claude-flow hook session-end --session-id "dev-session-2024"
  npx claude-flow hook session-end -s "feature-auth" --export-metrics --generate-summary
  npx claude-flow hook session-end -s "quick-fix" --cleanup-temp
```

**Features:**
- Save current context and progress
- Export session metrics (duration, commands, tokens, files)
- Generate work summary with decisions and next steps
- Cleanup temporary files and optimize storage

**notify** - Custom notifications with swarm status
```bash
npx claude-flow hook notify --message <msg>

Options:
  --message, -m <text>      Notification message
  --level <level>           Notification level (info|warning|error)
  --swarm-status            Include swarm status (default: true)
  --broadcast               Send to all agents

Examples:
  npx claude-flow hook notify -m "Task completed" --level info
  npx claude-flow hook notify -m "Critical error" --level error --broadcast
```

**Features:**
- Send notifications to coordination system
- Include swarm status and metrics
- Broadcast to all agents
- Log important events

### Configuration

#### Basic Configuration

Edit `.claude/settings.json` to configure hooks:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^(Write|Edit|MultiEdit)$",
        "hooks": [{
          "type": "command",
          "command": "npx claude-flow hook pre-edit --file '${tool.params.file_path}' --memory-key 'swarm/editor/current'"
        }]
      },
      {
        "matcher": "^Bash$",
        "hooks": [{
          "type": "command",
          "command": "npx claude-flow hook pre-bash --command '${tool.params.command}'"
        }]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "^(Write|Edit|MultiEdit)$",
        "hooks": [{
          "type": "command",
          "command": "npx claude-flow hook post-edit --file '${tool.params.file_path}' --memory-key 'swarm/editor/complete' --auto-format --train-patterns"
        }]
      },
      {
        "matcher": "^Bash$",
        "hooks": [{
          "type": "command",
          "command": "npx claude-flow hook post-bash --command '${tool.params.command}' --update-metrics"
        }]
      }
    ]
  }
}
```

#### Advanced Configuration

Complete hook configuration with all features:

```json
{
  "hooks": {
    "enabled": true,
    "debug": false,
    "timeout": 5000,

    "PreToolUse": [
      {
        "matcher": "^(Write|Edit|MultiEdit)$",
        "hooks": [
          {
            "type": "command",
            "command": "npx claude-flow hook pre-edit --file '${tool.params.file_path}' --auto-assign-agent --validate-syntax",
            "timeout": 3000,
            "continueOnError": true
          }
        ]
      },
      {
        "matcher": "^Task$",
        "hooks": [
          {
            "type": "command",
            "command": "npx claude-flow hook pre-task --description '${tool.params.task}' --auto-spawn-agents --load-memory",
            "async": true
          }
        ]
      },
      {
        "matcher": "^Grep$",
        "hooks": [
          {
            "type": "command",
            "command": "npx claude-flow hook pre-search --query '${tool.params.pattern}' --check-cache"
          }
        ]
      }
    ],

    "PostToolUse": [
      {
        "matcher": "^(Write|Edit|MultiEdit)$",
        "hooks": [
          {
            "type": "command",
            "command": "npx claude-flow hook post-edit --file '${tool.params.file_path}' --memory-key 'edits/${tool.params.file_path}' --auto-format --train-patterns",
            "async": true
          }
        ]
      },
      {
        "matcher": "^Task$",
        "hooks": [
          {
            "type": "command",
            "command": "npx claude-flow hook post-task --task-id '${result.task_id}' --analyze-performance --store-decisions --export-learnings",
            "async": true
          }
        ]
      },
      {
        "matcher": "^Grep$",
        "hooks": [
          {
            "type": "command",
            "command": "npx claude-flow hook post-search --query '${tool.params.pattern}' --cache-results --train-patterns"
          }
        ]
      }
    ],

    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "npx claude-flow hook session-start --session-id '${session.id}' --load-context"
          }
        ]
      }
    ],

    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "npx claude-flow hook session-end --session-id '${session.id}' --export-metrics --generate-summary --cleanup-temp"
          }
        ]
      }
    ]
  }
}
```

#### Protected File Patterns

Add protection for sensitive files:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^(Write|Edit|MultiEdit)$",
        "hooks": [
          {
            "type": "command",
            "command": "npx claude-flow hook check-protected --file '${tool.params.file_path}'"
          }
        ]
      }
    ]
  }
}
```

#### Automatic Testing

Run tests after file modifications:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "^Write$",
        "hooks": [
          {
            "type": "command",
            "command": "test -f '${tool.params.file_path%.js}.test.js' && npm test '${tool.params.file_path%.js}.test.js'",
            "continueOnError": true
          }
        ]
      }
    ]
  }
}
```

### MCP Tool Integration

Hooks automatically integrate with MCP tools for coordination:

#### Pre-Task Hook with Agent Spawning

```javascript
// Hook command
npx claude-flow hook pre-task --description "Build REST API"

// Internally calls MCP tools:
mcp__claude-flow__agent_spawn {
  type: "backend-dev",
  capabilities: ["api", "database", "testing"]
}

mcp__claude-flow__memory_usage {
  action: "store",
  key: "swarm/task/api-build/context",
  namespace: "coordination",
  value: JSON.stringify({
    description: "Build REST API",
    agents: ["backend-dev"],
    started: Date.now()
  })
}
```

#### Post-Edit Hook with Memory Storage

```javascript
// Hook command
npx claude-flow hook post-edit --file "api/auth.js"

// Internally calls MCP tools:
mcp__claude-flow__memory_usage {
  action: "store",
  key: "swarm/edits/api/auth.js",
  namespace: "coordination",
  value: JSON.stringify({
    file: "api/auth.js",
    timestamp: Date.now(),
    changes: { added: 45, removed: 12 },
    formatted: true,
    linted: true
  })
}

mcp__claude-flow__neural_train {
  pattern_type: "coordination",
  training_data: { /* edit patterns */ }
}
```

#### Session End Hook with State Persistence

```javascript
// Hook command
npx claude-flow hook session-end --session-id "dev-2024"

// Internally calls MCP tools:
mcp__claude-flow__memory_persist {
  sessionId: "dev-2024"
}

mcp__claude-flow__swarm_status {
  swarmId: "current"
}

// Generates metrics and summary
```

### Memory Coordination Protocol

All hooks follow a standardized memory coordination pattern:

#### Three-Phase Memory Protocol

**Phase 1: STATUS** - Hook starts
```javascript
mcp__claude-flow__memory_usage {
  action: "store",
  key: "swarm/hooks/pre-edit/status",
  namespace: "coordination",
  value: JSON.stringify({
    status: "running",
    hook: "pre-edit",
    file: "src/auth.js",
    timestamp: Date.now()
  })
}
```

**Phase 2: PROGRESS** - Hook processes
```javascript
mcp__claude-flow__memory_usage {
  action: "store",
  key: "swarm/hooks/pre-edit/progress",
  namespace: "coordination",
  value: JSON.stringify({
    progress: 50,
    action: "validating syntax",
    file: "src/auth.js"
  })
}
```

**Phase 3: COMPLETE** - Hook finishes
```javascript
mcp__claude-flow__memory_usage {
  action: "store",
  key: "swarm/hooks/pre-edit/complete",
  namespace: "coordination",
  value: JSON.stringify({
    status: "complete",
    result: "success",
    agent_assigned: "backend-dev",
    syntax_valid: true,
    backup_created: true
  })
}
```

### Hook Response Format

Hooks return JSON responses to control operation flow:

#### Continue Response
```json
{
  "continue": true,
  "reason": "All validations passed",
  "metadata": {
    "agent_assigned": "backend-dev",
    "syntax_valid": true,
    "file": "src/auth.js"
  }
}
```

#### Block Response
```json
{
  "continue": false,
  "reason": "Protected file - manual review required",
  "metadata": {
    "file": ".env.production",
    "protection_level": "high",
    "requires": "manual_approval"
  }
}
```

#### Warning Response
```json
{
  "continue": true,
  "reason": "Syntax valid but complexity high",
  "warnings": [
    "Cyclomatic complexity: 15 (threshold: 10)",
    "Consider refactoring for better maintainability"
  ],
  "metadata": {
    "complexity": 15,
    "threshold": 10
  }
}
```

### Git Integration

Hooks can integrate with Git operations for quality control:

#### Pre-Commit Hook
```bash
# Add to .git/hooks/pre-commit or use husky

#!/bin/bash
# Run quality checks before commit

# Get staged files
FILES=$(git diff --cached --name-only --diff-filter=ACM)

for FILE in $FILES; do
  # Run pre-edit hook for validation
  npx claude-flow hook pre-edit --file "$FILE" --validate-syntax

  if [ $? -ne 0 ]; then
    echo "Validation failed for $FILE"
    exit 1
  fi

  # Run post-edit hook for formatting
  npx claude-flow hook post-edit --file "$FILE" --auto-format
done

# Run tests
npm test

exit $?
```

#### Post-Commit Hook
```bash
# Add to .git/hooks/post-commit

#!/bin/bash
# Track commit metrics

COMMIT_HASH=$(git rev-parse HEAD)
COMMIT_MSG=$(git log -1 --pretty=%B)

npx claude-flow hook notify \
  --message "Commit completed: $COMMIT_MSG" \
  --level info \
  --swarm-status
```

#### Pre-Push Hook
```bash
# Add to .git/hooks/pre-push

#!/bin/bash
# Quality gate before push

# Run full test suite
npm run test:all

# Run quality checks
npx claude-flow hook session-end \
  --generate-report \
  --export-metrics

# Verify quality thresholds
TRUTH_SCORE=$(npx claude-flow metrics score --format json | jq -r '.truth_score')

if (( $(echo "$TRUTH_SCORE < 0.95" | bc -l) )); then
  echo "Truth score below threshold: $TRUTH_SCORE < 0.95"
  exit 1
fi

exit 0
```

### Agent Coordination Workflow

How agents use hooks for coordination:

#### Agent Workflow Example

```bash
# Agent 1: Backend Developer
# STEP 1: Pre-task preparation
npx claude-flow hook pre-task \
  --description "Implement user authentication API" \
  --auto-spawn-agents \
  --load-memory

# STEP 2: Work begins - pre-edit validation
npx claude-flow hook pre-edit \
  --file "api/auth.js" \
  --auto-assign-agent \
  --validate-syntax

# STEP 3: Edit file (via Claude Code Edit tool)
# ... code changes ...

# STEP 4: Post-edit processing
npx claude-flow hook post-edit \
  --file "api/auth.js" \
  --memory-key "swarm/backend/auth-api" \
  --auto-format \
  --train-patterns

# STEP 5: Notify coordination system
npx claude-flow hook notify \
  --message "Auth API implementation complete" \
  --swarm-status \
  --broadcast

# STEP 6: Task completion
npx claude-flow hook post-task \
  --task-id "auth-api" \
  --analyze-performance \
  --store-decisions \
  --export-learnings
```

```bash
# Agent 2: Test Engineer (receives notification)
# STEP 1: Check memory for API details
npx claude-flow hook session-restore \
  --session-id "swarm-current" \
  --restore-memory

# Memory contains: swarm/backend/auth-api with implementation details

# STEP 2: Generate tests
npx claude-flow hook pre-task \
  --description "Write tests for auth API" \
  --load-memory

# STEP 3: Create test file
npx claude-flow hook post-edit \
  --file "api/auth.test.js" \
  --memory-key "swarm/testing/auth-api-tests" \
  --train-patterns

# STEP 4: Share test results
npx claude-flow hook notify \
  --message "Auth API tests complete - 100% coverage" \
  --broadcast
```

### Custom Hook Creation

Create custom hooks for specific workflows:

#### Custom Hook Template

```javascript
// .claude/hooks/custom-quality-check.js

module.exports = {
  name: 'custom-quality-check',
  type: 'pre',
  matcher: /\.(ts|js)$/,

  async execute(context) {
    const { file, content } = context;

    // Custom validation logic
    const complexity = await analyzeComplexity(content);
    const securityIssues = await scanSecurity(content);

    // Store in memory
    await storeInMemory({
      key: `quality/${file}`,
      value: { complexity, securityIssues }
    });

    // Return decision
    if (complexity > 15 || securityIssues.length > 0) {
      return {
        continue: false,
        reason: 'Quality checks failed',
        warnings: [
          `Complexity: ${complexity} (max: 15)`,
          `Security issues: ${securityIssues.length}`
        ]
      };
    }

    return {
      continue: true,
      reason: 'Quality checks passed',
      metadata: { complexity, securityIssues: 0 }
    };
  }
};
```

#### Register Custom Hook

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "^(Write|Edit)$",
        "hooks": [
          {
            "type": "script",
            "script": ".claude/hooks/custom-quality-check.js"
          }
        ]
      }
    ]
  }
}
```

### Real-World Examples

#### Example 1: Full-Stack Development Workflow

```bash
# Session start - initialize coordination
npx claude-flow hook session-start --session-id "fullstack-feature"

# Pre-task planning
npx claude-flow hook pre-task \
  --description "Build user profile feature - frontend + backend + tests" \
  --auto-spawn-agents \
  --optimize-topology

# Backend work
npx claude-flow hook pre-edit --file "api/profile.js"
# ... implement backend ...
npx claude-flow hook post-edit \
  --file "api/profile.js" \
  --memory-key "profile/backend" \
  --train-patterns

# Frontend work (reads backend details from memory)
npx claude-flow hook pre-edit --file "components/Profile.jsx"
# ... implement frontend ...
npx claude-flow hook post-edit \
  --file "components/Profile.jsx" \
  --memory-key "profile/frontend" \
  --train-patterns

# Testing (reads both backend and frontend from memory)
npx claude-flow hook pre-task \
  --description "Test profile feature" \
  --load-memory

# Session end - export everything
npx claude-flow hook session-end \
  --session-id "fullstack-feature" \
  --export-metrics \
  --generate-summary
```

#### Example 2: Debugging with Hooks

```bash
# Start debugging session
npx claude-flow hook session-start --session-id "debug-memory-leak"

# Pre-task: analyze issue
npx claude-flow hook pre-task \
  --description "Debug memory leak in event handlers" \
  --load-memory \
  --estimate-complexity

# Search for event emitters
npx claude-flow hook pre-search --query "EventEmitter"
# ... search executes ...
npx claude-flow hook post-search \
  --query "EventEmitter" \
  --cache-results

# Fix the issue
npx claude-flow hook pre-edit \
  --file "services/events.js" \
  --backup-file
# ... fix code ...
npx claude-flow hook post-edit \
  --file "services/events.js" \
  --memory-key "debug/memory-leak-fix" \
  --validate-output

# Verify fix
npx claude-flow hook post-task \
  --task-id "memory-leak-fix" \
  --analyze-performance \
  --generate-report

# End session
npx claude-flow hook session-end \
  --session-id "debug-memory-leak" \
  --export-metrics
```

#### Example 3: Multi-Agent Refactoring

```bash
# Initialize swarm for refactoring
npx claude-flow hook pre-task \
  --description "Refactor legacy codebase to modern patterns" \
  --auto-spawn-agents \
  --optimize-topology

# Agent 1: Code Analyzer
npx claude-flow hook pre-task --description "Analyze code complexity"
# ... analysis ...
npx claude-flow hook post-task \
  --task-id "analysis" \
  --store-decisions

# Agent 2: Refactoring (reads analysis from memory)
npx claude-flow hook session-restore \
  --session-id "swarm-refactor" \
  --restore-memory

for file in src/**/*.js; do
  npx claude-flow hook pre-edit --file "$file" --backup-file
  # ... refactor ...
  npx claude-flow hook post-edit \
    --file "$file" \
    --memory-key "refactor/$file" \
    --auto-format \
    --train-patterns
done

# Agent 3: Testing (reads refactored code from memory)
npx claude-flow hook pre-task \
  --description "Generate tests for refactored code" \
  --load-memory

# Broadcast completion
npx claude-flow hook notify \
  --message "Refactoring complete - all tests passing" \
  --broadcast
```

### Performance Tips

1. **Keep Hooks Lightweight** - Target < 100ms execution time
2. **Use Async for Heavy Operations** - Don't block the main flow
3. **Cache Aggressively** - Store frequently accessed data
4. **Batch Related Operations** - Combine multiple actions
5. **Use Memory Wisely** - Set appropriate TTLs
6. **Monitor Hook Performance** - Track execution times
7. **Parallelize When Possible** - Run independent hooks concurrently

### Debugging Hooks

Enable debug mode for troubleshooting:

```bash
# Enable debug output
export CLAUDE_FLOW_DEBUG=true

# Test specific hook with verbose output
npx claude-flow hook pre-edit --file "test.js" --debug

# Check hook execution logs
cat .claude-flow/logs/hooks-$(date +%Y-%m-%d).log

# Validate configuration
npx claude-flow hook validate-config
```

### Benefits

- **Automatic Agent Assignment**: Right agent for every file type
- **Consistent Code Formatting**: Language-specific formatters
- **Continuous Learning**: Neural patterns improve over time
- **Cross-Session Memory**: Context persists between sessions
- **Performance Tracking**: Comprehensive metrics and analytics
- **Automatic Coordination**: Agents sync via memory
- **Smart Agent Spawning**: Task-based agent selection
- **Quality Gates**: Pre-commit validation and verification
- **Error Prevention**: Syntax validation before edits
- **Knowledge Sharing**: Decisions stored and shared
- **Reduced Manual Work**: Automation of repetitive tasks
- **Better Collaboration**: Seamless multi-agent coordination

### Best Practices

1. **Configure Hooks Early** - Set up during project initialization
2. **Use Memory Keys Strategically** - Organize with clear namespaces
3. **Enable Auto-Formatting** - Maintain code consistency
4. **Train Patterns Continuously** - Learn from successful operations
5. **Monitor Performance** - Track hook execution times
6. **Validate Configuration** - Test hooks before production use
7. **Document Custom Hooks** - Maintain hook documentation
8. **Set Appropriate Timeouts** - Prevent hanging operations
9. **Handle Errors Gracefully** - Use continueOnError when appropriate
10. **Review Metrics Regularly** - Optimize based on usage patterns

### Troubleshooting

#### Hooks Not Executing
- Verify `.claude/settings.json` syntax
- Check hook matcher patterns
- Enable debug mode
- Review permission settings
- Ensure claude-flow CLI is in PATH

#### Hook Timeouts
- Increase timeout values in configuration
- Make hooks asynchronous for heavy operations
- Optimize hook logic
- Check network connectivity for MCP tools

#### Memory Issues
- Set appropriate TTLs for memory keys
- Clean up old memory entries
- Use memory namespaces effectively
- Monitor memory usage

#### Performance Problems
- Profile hook execution times
- Use caching for repeated operations
- Batch operations when possible
- Reduce hook complexity

### Related Commands

- `npx claude-flow init --hooks` - Initialize hooks system
- `npx claude-flow hook --list` - List available hooks
- `npx claude-flow hook --test <hook>` - Test specific hook
- `npx claude-flow memory usage` - Manage memory
- `npx claude-flow agent spawn` - Spawn agents
- `npx claude-flow swarm init` - Initialize swarm

### Integration with Other Skills

This skill works seamlessly with:
- **SPARC Methodology** - Hooks enhance SPARC workflows
- **Pair Programming** - Automated quality in pairing sessions
- **Verification Quality** - Truth-score validation in hooks
- **GitHub Workflows** - Git integration for commits/PRs
- **Performance Analysis** - Metrics collection in hooks
- **Swarm Advanced** - Multi-agent coordination via hooks
