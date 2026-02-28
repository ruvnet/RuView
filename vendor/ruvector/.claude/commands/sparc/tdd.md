# SPARC TDD Mode

## Purpose
Test-driven development with TodoWrite planning and comprehensive testing.

## Activation

### Option 1: Using MCP Tools (Preferred in Claude Code)
```javascript
mcp__claude-flow__sparc_mode {
  mode: "tdd",
  task_description: "shopping cart feature",
  options: {
    coverage_target: 90,
    test_framework: "jest"
  }
}
```

### Option 2: Using NPX CLI (Fallback when MCP not available)
```bash
# Use when running from terminal or MCP tools unavailable
npx claude-flow sparc run tdd "shopping cart feature"

# For alpha features
npx claude-flow@alpha sparc run tdd "shopping cart feature"
```

### Option 3: Local Installation
```bash
# If claude-flow is installed locally
./claude-flow sparc run tdd "shopping cart feature"
```

## Core Capabilities
- Test-first development
- Red-green-refactor cycle
- Test suite design
- Coverage optimization
- Continuous testing

## TDD Workflow
1. Write failing tests
2. Implement minimum code
3. Make tests pass
4. Refactor code
5. Repeat cycle

## Testing Strategies
- Unit testing
- Integration testing
- End-to-end testing
- Performance testing
- Security testing
