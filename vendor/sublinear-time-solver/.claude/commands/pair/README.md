# 👥 Pair Programming Command

Collaborative development with real-time verification and AI assistance.

## Overview

The `pair` command enables collaborative pair programming between you and AI agents, with real-time verification, code review, and quality enforcement.

## Usage

```bash
claude-flow pair [--start] [options]
```

## Quick Start

```bash
# Start pair programming session
claude-flow pair --start

# Start with specific agent
claude-flow pair --start --agent senior-dev

# Start with verification
claude-flow pair --start --verify --threshold 0.98
```

## Options

- `--start` - Start new pair programming session
- `--agent <name>` - Specify AI pair partner (default: auto-select)
- `--verify` - Enable real-time verification
- `--threshold <0-1>` - Verification threshold (default: 0.95)
- `--mode <type>` - Programming mode: driver, navigator, switch
- `--focus <area>` - Focus area: refactor, test, debug, implement
- `--language <lang>` - Primary language for session
- `--review` - Enable continuous code review
- `--test` - Run tests after each change

## Modes

### Driver Mode
You write code, AI provides suggestions and reviews.
```bash
claude-flow pair --start --mode driver
```

### Navigator Mode
AI writes code, you provide guidance and review.
```bash
claude-flow pair --start --mode navigator
```

### Switch Mode (Default)
Alternate between driver and navigator roles.
```bash
claude-flow pair --start --mode switch --interval 10m
```

## Features

### Real-Time Verification
- Continuous truth checking (0.95 threshold)
- Automatic rollback on verification failure
- Quality gates before commits

### Code Review
- Instant feedback on code changes
- Best practice suggestions
- Security vulnerability detection
- Performance optimization tips

### Test Integration
- Automatic test generation
- Test-driven development support
- Coverage monitoring
- Integration test suggestions

### Collaboration Tools
- Shared context between you and AI
- Session history and replay
- Code explanation on demand
- Learning from your preferences

## Session Management

### Start Session
```bash
# Basic start
claude-flow pair --start

# With configuration
claude-flow pair --start \
  --agent expert-coder \
  --verify \
  --test \
  --focus refactor
```

### During Session
```commands
/help          - Show available commands
/explain       - Explain current code
/suggest       - Get improvement suggestions
/test          - Run tests
/verify        - Check verification score
/switch        - Switch driver/navigator roles
/focus <area>  - Change focus area
/commit        - Commit with verification
/pause         - Pause session
/resume        - Resume session
/end           - End session
```

### End Session
```bash
# End and save session
claude-flow pair --end --save

# End and generate report
claude-flow pair --end --report
```

## Examples

### Refactoring Session
```bash
claude-flow pair --start \
  --focus refactor \
  --verify \
  --threshold 0.98
```

### Test-Driven Development
```bash
claude-flow pair --start \
  --focus test \
  --mode tdd \
  --language javascript
```

### Bug Fixing
```bash
claude-flow pair --start \
  --focus debug \
  --agent debugger-expert \
  --test
```

### Code Review Session
```bash
claude-flow pair --start \
  --review \
  --verify \
  --agent senior-reviewer
```

## Integration

### With Git
```bash
# Auto-commit with verification
claude-flow pair --start --git --auto-commit

# Review before commit
claude-flow pair --start --git --review-commit
```

### With Testing Frameworks
```bash
# Jest integration
claude-flow pair --start --test-framework jest

# Pytest integration
claude-flow pair --start --test-framework pytest
```

### With CI/CD
```bash
# CI-friendly mode
claude-flow pair --start --ci --non-interactive
```

## Session Output

```
👥 Pair Programming Session Started
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Partner: expert-coder
Mode: Switch (10m intervals)
Focus: Implementation
Verification: ✅ Enabled (0.95)
Testing: ✅ Auto-run

Current Role: DRIVER (you)
Navigator: expert-coder is reviewing...

📝 Working on: src/auth/login.js
Truth Score: 0.972 ✅
Test Coverage: 84% 📈

💡 Suggestion: Consider adding input validation for email field
🔍 Review: Line 23 - Potential SQL injection vulnerability

Type /help for commands or start coding...
```

## Quality Metrics

- **Session Duration**: Total pair programming time
- **Code Quality**: Average truth score during session
- **Productivity**: Lines changed, features completed
- **Learning**: Patterns learned from collaboration
- **Test Coverage**: Coverage improvement during session

## Configuration

Configure pair programming in `.claude-flow/config.json`:

```json
{
  "pair": {
    "defaultAgent": "expert-coder",
    "defaultMode": "switch",
    "switchInterval": "10m",
    "verification": {
      "enabled": true,
      "threshold": 0.95,
      "autoRollback": true
    },
    "testing": {
      "autoRun": true,
      "framework": "jest",
      "coverage": {
        "minimum": 80,
        "enforce": true
      }
    },
    "review": {
      "continuous": true,
      "preCommit": true
    }
  }
}
```

## Best Practices

1. **Start with Clear Goals**: Define what you want to accomplish
2. **Use Verification**: Enable verification for critical code
3. **Test Frequently**: Run tests after significant changes
4. **Review Together**: Use review features for learning
5. **Document Decisions**: AI will help document why choices were made

## Related Commands

- `verify` - Standalone verification
- `truth` - View quality metrics
- `test` - Run test suites
- `review` - Code review tools