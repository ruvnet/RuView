---
name: Pair Programming
description: AI-assisted pair programming with multiple modes (driver/navigator/switch), real-time verification, quality monitoring, and comprehensive testing. Supports TDD, debugging, refactoring, and learning sessions. Features automatic role switching, continuous code review, security scanning, and performance optimization with truth-score verification.
---

# Pair Programming

Collaborative AI pair programming with intelligent role management, real-time quality monitoring, and comprehensive development workflows.

## What This Skill Does

This skill provides professional pair programming capabilities with AI assistance, supporting multiple collaboration modes, continuous verification, and integrated testing. It manages driver/navigator roles, performs real-time code review, tracks quality metrics, and ensures high standards through truth-score verification.

**Key Capabilities:**
- **Multiple Modes**: Driver, Navigator, Switch, TDD, Review, Mentor, Debug
- **Real-Time Verification**: Automatic quality scoring with rollback on failures
- **Role Management**: Seamless switching between driver/navigator roles
- **Testing Integration**: Auto-generate tests, track coverage, continuous testing
- **Code Review**: Security scanning, performance analysis, best practice enforcement
- **Session Persistence**: Auto-save, recovery, export, and sharing

## Prerequisites

**Required:**
- Claude Flow CLI installed (`npm install -g claude-flow@alpha`)
- Git repository (optional but recommended)

**Recommended:**
- Testing framework (Jest, pytest, etc.)
- Linter configured (ESLint, pylint, etc.)
- Code formatter (Prettier, Black, etc.)

## Quick Start

### Basic Session
```bash
# Start simple pair programming
claude-flow pair --start
```

### TDD Session
```bash
# Test-driven development
claude-flow pair --start \
  --mode tdd \
  --test-first \
  --coverage 90
```

---

## Complete Guide

### Session Control Commands

#### Starting Sessions
```bash
# Basic start
claude-flow pair --start

# Expert refactoring session
claude-flow pair --start \
  --agent senior-dev \
  --focus refactor \
  --verify \
  --threshold 0.98

# Debugging session
claude-flow pair --start \
  --agent debugger-expert \
  --focus debug \
  --review

# Learning session
claude-flow pair --start \
  --mode mentor \
  --pace slow \
  --examples
```

#### Session Management
```bash
# Check status
claude-flow pair --status

# View history
claude-flow pair --history

# Pause session
/pause [--reason <reason>]

# Resume session
/resume

# End session
claude-flow pair --end [--save] [--report]
```

### Available Modes

#### Driver Mode
You write code while AI provides guidance.

```bash
claude-flow pair --start --mode driver
```

**Your Responsibilities:**
- Write actual code
- Implement solutions
- Make immediate decisions
- Handle syntax and structure

**AI Navigator:**
- Strategic guidance
- Spot potential issues
- Suggest improvements
- Real-time review
- Track overall direction

**Best For:**
- Learning new patterns
- Implementing familiar features
- Quick iterations
- Hands-on debugging

**Commands:**
```
/suggest     - Get implementation suggestions
/review      - Request code review
/explain     - Ask for explanations
/optimize    - Request optimization ideas
/patterns    - Get pattern recommendations
```

#### Navigator Mode
AI writes code while you provide direction.

```bash
claude-flow pair --start --mode navigator
```

**Your Responsibilities:**
- Provide high-level direction
- Review generated code
- Make architectural decisions
- Ensure business requirements

**AI Driver:**
- Write implementation code
- Handle syntax details
- Implement your guidance
- Manage boilerplate
- Execute refactoring

**Best For:**
- Rapid prototyping
- Boilerplate generation
- Learning from AI patterns
- Exploring solutions

**Commands:**
```
/implement   - Direct implementation
/refactor    - Request refactoring
/test        - Generate tests
/document    - Add documentation
/alternate   - See alternative approaches
```

#### Switch Mode
Automatically alternates roles at intervals.

```bash
# Default 10-minute intervals
claude-flow pair --start --mode switch

# 5-minute intervals (rapid)
claude-flow pair --start --mode switch --interval 5m

# 15-minute intervals (deep focus)
claude-flow pair --start --mode switch --interval 15m
```

**Handoff Process:**
1. 30-second warning before switch
2. Current driver completes thought
3. Context summary generated
4. Roles swap smoothly
5. New driver continues

**Best For:**
- Balanced collaboration
- Knowledge sharing
- Complex features
- Extended sessions

#### Specialized Modes

**TDD Mode** - Test-Driven Development:
```bash
claude-flow pair --start \
  --mode tdd \
  --test-first \
  --coverage 100
```
Workflow: Write failing test → Implement → Refactor → Repeat

**Review Mode** - Continuous code review:
```bash
claude-flow pair --start \
  --mode review \
  --strict \
  --security
```
Features: Real-time feedback, security scanning, performance analysis

**Mentor Mode** - Learning-focused:
```bash
claude-flow pair --start \
  --mode mentor \
  --explain-all \
  --pace slow
```
Features: Detailed explanations, step-by-step guidance, pattern teaching

**Debug Mode** - Problem-solving:
```bash
claude-flow pair --start \
  --mode debug \
  --verbose \
  --trace
```
Features: Issue identification, root cause analysis, fix suggestions

### In-Session Commands

#### Code Commands
```
/explain [--level basic|detailed|expert]
  Explain the current code or selection

/suggest [--type refactor|optimize|security|style]
  Get improvement suggestions

/implement <description>
  Request implementation (navigator mode)

/refactor [--pattern <pattern>] [--scope function|file|module]
  Refactor selected code

/optimize [--target speed|memory|both]
  Optimize code for performance

/document [--format jsdoc|markdown|inline]
  Add documentation to code

/comment [--verbose]
  Add inline comments

/pattern <pattern-name> [--example]
  Apply a design pattern
```

#### Testing Commands
```
/test [--watch] [--coverage] [--only <pattern>]
  Run test suite

/test-gen [--type unit|integration|e2e]
  Generate tests for current code

/coverage [--report html|json|terminal]
  Check test coverage

/mock <target> [--realistic]
  Generate mock data or functions

/test-watch [--on-save]
  Enable test watching

/snapshot [--update]
  Create test snapshots
```

#### Review Commands
```
/review [--scope current|file|changes] [--strict]
  Perform code review

/security [--deep] [--fix]
  Security analysis

/perf [--profile] [--suggestions]
  Performance analysis

/quality [--detailed]
  Check code quality metrics

/lint [--fix] [--config <config>]
  Run linters

/complexity [--threshold <value>]
  Analyze code complexity
```

#### Navigation Commands
```
/goto <file>[:line[:column]]
  Navigate to file or location

/find <pattern> [--regex] [--case-sensitive]
  Search in project

/recent [--limit <n>]
  Show recent files

/bookmark [add|list|goto|remove] [<name>]
  Manage bookmarks

/history [--limit <n>] [--filter <pattern>]
  Show command history

/tree [--depth <n>] [--filter <pattern>]
  Show project structure
```

#### Git Commands
```
/diff [--staged] [--file <file>]
  Show git diff

/commit [--message <msg>] [--amend]
  Commit with verification

/branch [create|switch|delete|list] [<name>]
  Branch operations

/stash [save|pop|list|apply] [<message>]
  Stash operations

/log [--oneline] [--limit <n>]
  View git log

/blame [<file>]
  Show git blame
```

#### AI Partner Commands
```
/agent [switch|info|config] [<agent-name>]
  Manage AI agent

/teach <preference>
  Teach the AI your preferences

/feedback [positive|negative] <message>
  Provide feedback to AI

/personality [professional|friendly|concise|verbose]
  Adjust AI personality

/expertise [add|remove|list] [<domain>]
  Set AI expertise focus
```

#### Metrics Commands
```
/metrics [--period today|session|week|all]
  Show session metrics

/score [--breakdown]
  Show quality scores

/productivity [--chart]
  Show productivity metrics

/leaderboard [--personal|team]
  Show improvement leaderboard
```

#### Role & Mode Commands
```
/switch [--immediate]
  Switch driver/navigator roles

/mode <type>
  Change mode (driver|navigator|switch|tdd|review|mentor|debug)

/role
  Show current role

/handoff
  Prepare role handoff
```

### Command Shortcuts

| Alias | Full Command |
|-------|-------------|
| `/s` | `/suggest` |
| `/e` | `/explain` |
| `/t` | `/test` |
| `/r` | `/review` |
| `/c` | `/commit` |
| `/g` | `/goto` |
| `/f` | `/find` |
| `/h` | `/help` |
| `/sw` | `/switch` |
| `/st` | `/status` |

### Configuration

#### Basic Configuration
Create `.claude-flow/pair-config.json`:

```json
{
  "pair": {
    "enabled": true,
    "defaultMode": "switch",
    "defaultAgent": "auto",
    "autoStart": false,
    "theme": "professional"
  }
}
```

#### Complete Configuration

```json
{
  "pair": {
    "general": {
      "enabled": true,
      "defaultMode": "switch",
      "defaultAgent": "senior-dev",
      "language": "javascript",
      "timezone": "UTC"
    },

    "modes": {
      "driver": {
        "enabled": true,
        "suggestions": true,
        "realTimeReview": true,
        "autoComplete": false
      },
      "navigator": {
        "enabled": true,
        "codeGeneration": true,
        "explanations": true,
        "alternatives": true
      },
      "switch": {
        "enabled": true,
        "interval": "10m",
        "warning": "30s",
        "autoSwitch": true,
        "pauseOnIdle": true
      }
    },

    "verification": {
      "enabled": true,
      "threshold": 0.95,
      "autoRollback": true,
      "preCommitCheck": true,
      "continuousMonitoring": true,
      "blockOnFailure": true
    },

    "testing": {
      "enabled": true,
      "autoRun": true,
      "framework": "jest",
      "onSave": true,
      "coverage": {
        "enabled": true,
        "minimum": 80,
        "enforce": true,
        "reportFormat": "html"
      }
    },

    "review": {
      "enabled": true,
      "continuous": true,
      "preCommit": true,
      "security": true,
      "performance": true,
      "style": true,
      "complexity": {
        "maxComplexity": 10,
        "maxDepth": 4,
        "maxLines": 100
      }
    },

    "git": {
      "enabled": true,
      "autoCommit": false,
      "commitTemplate": "feat: {message}",
      "signCommits": false,
      "pushOnEnd": false,
      "branchProtection": true
    },

    "session": {
      "autoSave": true,
      "saveInterval": "5m",
      "maxDuration": "4h",
      "idleTimeout": "15m",
      "breakReminder": "45m",
      "metricsInterval": "1m"
    },

    "ai": {
      "model": "advanced",
      "temperature": 0.7,
      "maxTokens": 4000,
      "personality": "professional",
      "expertise": ["backend", "testing", "security"],
      "learningEnabled": true
    }
  }
}
```

#### Built-in Agents

```json
{
  "agents": {
    "senior-dev": {
      "expertise": ["architecture", "patterns", "optimization"],
      "style": "thorough",
      "reviewLevel": "strict"
    },
    "tdd-specialist": {
      "expertise": ["testing", "mocks", "coverage"],
      "style": "test-first",
      "reviewLevel": "comprehensive"
    },
    "debugger-expert": {
      "expertise": ["debugging", "profiling", "tracing"],
      "style": "analytical",
      "reviewLevel": "focused"
    },
    "junior-dev": {
      "expertise": ["learning", "basics", "documentation"],
      "style": "questioning",
      "reviewLevel": "educational"
    }
  }
}
```

#### CLI Configuration
```bash
# Set configuration
claude-flow pair config set defaultMode switch
claude-flow pair config set verification.threshold 0.98

# Get configuration
claude-flow pair config get
claude-flow pair config get defaultMode

# Export/Import
claude-flow pair config export > config.json
claude-flow pair config import config.json

# Reset
claude-flow pair config reset
```

#### Profile Management

Create reusable profiles:

```bash
# Create profile
claude-flow pair profile create refactoring \
  --mode driver \
  --verify true \
  --threshold 0.98 \
  --focus refactor

# Use profile
claude-flow pair --start --profile refactoring

# List profiles
claude-flow pair profile list
```

Profile configuration:
```json
{
  "profiles": {
    "refactoring": {
      "mode": "driver",
      "verification": {
        "enabled": true,
        "threshold": 0.98
      },
      "focus": "refactor"
    },
    "debugging": {
      "mode": "navigator",
      "agent": "debugger-expert",
      "trace": true,
      "verbose": true
    },
    "learning": {
      "mode": "mentor",
      "pace": "slow",
      "explanations": "detailed",
      "examples": true
    }
  }
}
```

### Real-World Examples

#### Example 1: Feature Implementation

Implementing user authentication with JWT tokens:

```bash
# Session setup
claude-flow pair --start \
  --mode switch \
  --agent senior-dev \
  --focus implement \
  --verify \
  --test
```

**Session Flow:**
```
👥 Starting pair programming for authentication feature...

[DRIVER: You - 10 minutes]
/explain JWT authentication flow
> AI explains JWT concepts and best practices

/suggest implementation approach
> AI suggests using middleware pattern with refresh tokens

# You write the basic auth middleware structure

[SWITCH TO NAVIGATOR]

[NAVIGATOR: AI - 10 minutes]
/implement JWT token generation with refresh tokens
> AI generates secure token implementation

/test-gen
> AI creates comprehensive test suite

[SWITCH TO DRIVER]

[DRIVER: You - 10 minutes]
# You refine the implementation
/review --security
> AI performs security review, suggests improvements

/commit --message "feat: JWT authentication with refresh tokens"
✅ Truth Score: 0.98 - Committed successfully
```

#### Example 2: Bug Fixing

Debugging a memory leak in Node.js:

```bash
# Session setup
claude-flow pair --start \
  --mode navigator \
  --agent debugger-expert \
  --focus debug \
  --trace
```

**Session Flow:**
```
👥 Starting debugging session...

/status
> Analyzing application for memory issues...

/perf --profile
> Memory usage growing: 150MB → 450MB over 10 minutes

/find "new EventEmitter" --regex
> Found 3 instances of EventEmitter creation

/inspect eventEmitters --deep
> Discovering listeners not being removed

/suggest fix for memory leak
> AI suggests: "Add removeListener in cleanup functions"

/implement cleanup functions for all event emitters
> AI generates proper cleanup code

/test
> Memory stable at 150MB ✅

/commit --message "fix: memory leak in event emitters"
```

#### Example 3: TDD Session

Building shopping cart with test-driven development:

```bash
# Session setup
claude-flow pair --start \
  --mode tdd \
  --agent tdd-specialist \
  --test-first
```

**Session Flow:**
```
👥 TDD Session: Shopping Cart Feature

[RED PHASE]
/test-gen "add item to cart"
> AI writes failing test:
  ✗ should add item to cart
  ✗ should update quantity for existing item
  ✗ should calculate total price

[GREEN PHASE]
/implement minimal cart functionality
> You write just enough code to pass tests

/test
> Tests passing: 3/3 ✅

[REFACTOR PHASE]
/refactor --pattern repository
> AI refactors to repository pattern

/test
> Tests still passing: 3/3 ✅

[NEXT CYCLE]
/test-gen "remove item from cart"
> AI writes new failing tests...
```

#### Example 4: Code Refactoring

Modernizing legacy code:

```bash
# Session setup
claude-flow pair --start \
  --mode driver \
  --focus refactor \
  --verify \
  --threshold 0.98
```

**Session Flow:**
```
👥 Refactoring Session: Modernizing UserService

/analyze UserService.js
> AI identifies:
  - Callback hell (5 levels deep)
  - No error handling
  - Tight coupling
  - No tests

/suggest refactoring plan
> AI suggests:
  1. Convert callbacks to async/await
  2. Add error boundaries
  3. Extract dependencies
  4. Add unit tests

/test-gen --before-refactor
> AI generates tests for current behavior

/refactor callbacks to async/await
# You refactor with AI guidance

/test
> All tests passing ✅

/review --compare
> AI shows before/after comparison
> Code complexity: 35 → 12
> Truth score: 0.99 ✅

/commit --message "refactor: modernize UserService with async/await"
```

#### Example 5: Performance Optimization

Optimizing slow React application:

```bash
# Session setup
claude-flow pair --start \
  --mode switch \
  --agent performance-expert \
  --focus optimize \
  --profile
```

**Session Flow:**
```
👥 Performance Optimization Session

/perf --profile
> React DevTools Profiler Results:
  - ProductList: 450ms render
  - CartSummary: 200ms render
  - Unnecessary re-renders: 15

/suggest optimizations for ProductList
> AI suggests:
  1. Add React.memo
  2. Use useMemo for expensive calculations
  3. Implement virtualization for long lists

/implement React.memo and useMemo
# You implement with AI guidance

/perf --profile
> ProductList: 45ms render (90% improvement!) ✅

/implement virtualization with react-window
> AI implements virtual scrolling

/perf --profile
> ProductList: 12ms render (97% improvement!) ✅
> FPS: 60 stable ✅

/commit --message "perf: optimize ProductList with memoization and virtualization"
```

#### Example 6: API Development

Building RESTful API with Express:

```bash
# Session setup
claude-flow pair --start \
  --mode navigator \
  --agent backend-expert \
  --focus implement \
  --test
```

**Session Flow:**
```
👥 API Development Session

/design REST API for blog platform
> AI designs endpoints:
  POST   /api/posts
  GET    /api/posts
  GET    /api/posts/:id
  PUT    /api/posts/:id
  DELETE /api/posts/:id

/implement CRUD endpoints with validation
> AI implements with Express + Joi validation

/test-gen --integration
> AI generates integration tests

/security --api
> AI adds:
  - Rate limiting
  - Input sanitization
  - JWT authentication
  - CORS configuration

/document --openapi
> AI generates OpenAPI documentation

/test --integration
> All endpoints tested: 15/15 ✅
```

### Session Templates

#### Quick Start Templates

```bash
# Refactoring template
claude-flow pair --template refactor
# Focus: Code improvement
# Verification: High (0.98)
# Testing: After each change
# Review: Continuous

# Feature template
claude-flow pair --template feature
# Focus: Implementation
# Verification: Standard (0.95)
# Testing: On completion
# Review: Pre-commit

# Debug template
claude-flow pair --template debug
# Focus: Problem solving
# Verification: Moderate (0.90)
# Testing: Regression tests
# Review: Root cause

# Learning template
claude-flow pair --template learn
# Mode: Mentor
# Pace: Slow
# Explanations: Detailed
# Examples: Many
```

### Session Management

#### Session Status

```bash
claude-flow pair --status
```

**Output:**
```
👥 Pair Programming Session
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Session ID: pair_1755021234567
Duration: 45 minutes
Status: Active

Partner: senior-dev
Current Role: DRIVER (you)
Mode: Switch (10m intervals)
Next Switch: in 3 minutes

📊 Metrics:
├── Truth Score: 0.982 ✅
├── Lines Changed: 234
├── Files Modified: 5
├── Tests Added: 12
├── Coverage: 87% ↑3%
└── Commits: 3

🎯 Focus: Implementation
📝 Current File: src/auth/login.js
```

#### Session History

```bash
claude-flow pair --history
```

**Output:**
```
📚 Session History
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

1. 2024-01-15 14:30 - 16:45 (2h 15m)
   Partner: expert-coder
   Focus: Refactoring
   Truth Score: 0.975
   Changes: +340 -125 lines

2. 2024-01-14 10:00 - 11:30 (1h 30m)
   Partner: tdd-specialist
   Focus: Testing
   Truth Score: 0.991
   Tests Added: 24

3. 2024-01-13 15:00 - 17:00 (2h)
   Partner: debugger-expert
   Focus: Bug Fixing
   Truth Score: 0.968
   Issues Fixed: 5
```

#### Session Persistence

```bash
# Save session
claude-flow pair --save [--name <name>]

# Load session
claude-flow pair --load <session-id>

# Export session
claude-flow pair --export <session-id> [--format json|md]

# Generate report
claude-flow pair --report <session-id>
```

#### Background Sessions

```bash
# Start in background
claude-flow pair --start --background

# Monitor background session
claude-flow pair --monitor

# Attach to background session
claude-flow pair --attach <session-id>

# End background session
claude-flow pair --end <session-id>
```

### Advanced Features

#### Custom Commands

Define in configuration:

```json
{
  "customCommands": {
    "tdd": "/test-gen && /test --watch",
    "full-review": "/lint --fix && /test && /review --strict",
    "quick-fix": "/suggest --type fix && /implement && /test"
  }
}
```

Use custom commands:
```
/custom tdd
/custom full-review
```

#### Command Chaining

```
/test && /commit && /push
/lint --fix && /test && /review --strict
```

#### Session Recording

```bash
# Start with recording
claude-flow pair --start --record

# Replay session
claude-flow pair --replay <session-id>

# Session analytics
claude-flow pair --analytics <session-id>
```

#### Integration Options

**With Git:**
```bash
claude-flow pair --start --git --auto-commit
```

**With CI/CD:**
```bash
claude-flow pair --start --ci --non-interactive
```

**With IDE:**
```bash
claude-flow pair --start --ide vscode
```

### Best Practices

#### Session Practices
1. **Clear Goals** - Define session objectives upfront
2. **Appropriate Mode** - Choose based on task type
3. **Enable Verification** - For critical code paths
4. **Regular Testing** - Maintain quality continuously
5. **Session Notes** - Document important decisions
6. **Regular Breaks** - Take breaks every 45-60 minutes

#### Code Practices
1. **Test Early** - Run tests after each change
2. **Verify Before Commit** - Check truth scores
3. **Review Security** - Always for sensitive code
4. **Profile Performance** - Use `/perf` for optimization
5. **Save Sessions** - For complex work
6. **Learn from AI** - Ask questions frequently

#### Mode Selection
- **Driver Mode**: When learning, controlling implementation
- **Navigator Mode**: For rapid prototyping, generation
- **Switch Mode**: Long sessions, balanced collaboration
- **TDD Mode**: Building with tests
- **Review Mode**: Quality focus
- **Mentor Mode**: Learning priority
- **Debug Mode**: Fixing issues

### Troubleshooting

#### Session Won't Start
- Check agent availability
- Verify configuration file syntax
- Ensure clean workspace
- Review log files

#### Session Disconnected
- Use `--recover` to restore
- Check network connection
- Verify background processes
- Review auto-save files

#### Poor Performance
- Reduce verification threshold
- Disable continuous testing
- Check system resources
- Use lighter AI model

#### Configuration Issues
- Validate JSON syntax
- Check file permissions
- Review priority order (CLI > env > project > user > global)
- Run `claude-flow pair config validate`

### Quality Metrics

#### Truth Score Thresholds
```
Error:   < 0.90 ❌
Warning: 0.90 - 0.95 ⚠️
Good:    0.95 - 0.98 ✅
Excellent: > 0.98 🌟
```

#### Coverage Thresholds
```
Error:   < 70% ❌
Warning: 70% - 80% ⚠️
Good:    80% - 90% ✅
Excellent: > 90% 🌟
```

#### Complexity Thresholds
```
Error:   > 15 ❌
Warning: 10 - 15 ⚠️
Good:    5 - 10 ✅
Excellent: < 5 🌟
```

### Environment Variables

Override configuration via environment:

```bash
export CLAUDE_PAIR_MODE=driver
export CLAUDE_PAIR_VERIFY=true
export CLAUDE_PAIR_THRESHOLD=0.98
export CLAUDE_PAIR_AGENT=senior-dev
export CLAUDE_PAIR_AUTO_TEST=true
```

### Command History

Navigate history:
- `↑/↓` - Navigate through command history
- `Ctrl+R` - Search command history
- `!!` - Repeat last command
- `!<n>` - Run command n from history

### Keyboard Shortcuts (Configurable)

Default shortcuts:
```json
{
  "shortcuts": {
    "switch": "ctrl+shift+s",
    "suggest": "ctrl+space",
    "review": "ctrl+r",
    "test": "ctrl+t"
  }
}
```

### Related Commands

- `claude-flow pair --help` - Show help
- `claude-flow pair config` - Manage configuration
- `claude-flow pair profile` - Manage profiles
- `claude-flow pair templates` - List templates
- `claude-flow pair agents` - List available agents
