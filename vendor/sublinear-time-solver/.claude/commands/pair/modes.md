# Pair Programming Modes

Detailed guide to pair programming modes and their optimal use cases.

## Driver Mode

In driver mode, you write the code while the AI acts as navigator.

### Usage
```bash
claude-flow pair --start --mode driver
```

### Responsibilities

**You (Driver):**
- Write the actual code
- Implement solutions
- Make immediate decisions
- Handle syntax and structure

**AI (Navigator):**
- Provide strategic guidance
- Spot potential issues
- Suggest improvements
- Review in real-time
- Track overall direction

### Best For
- Learning new patterns
- Implementing familiar features
- Quick iterations
- Hands-on debugging

### Example Session
```bash
claude-flow pair --start \
  --mode driver \
  --agent senior-navigator \
  --review \
  --verify
```

### Commands in Driver Mode
```
/suggest     - Get implementation suggestions
/review      - Request code review
/explain     - Ask for explanations
/optimize    - Request optimization ideas
/patterns    - Get pattern recommendations
```

## Navigator Mode

In navigator mode, the AI writes code while you provide guidance.

### Usage
```bash
claude-flow pair --start --mode navigator
```

### Responsibilities

**You (Navigator):**
- Provide high-level direction
- Review generated code
- Make architectural decisions
- Ensure business requirements

**AI (Driver):**
- Write implementation code
- Handle syntax details
- Implement your guidance
- Manage boilerplate
- Execute refactoring

### Best For
- Rapid prototyping
- Boilerplate generation
- Learning from AI patterns
- Exploring solutions

### Example Session
```bash
claude-flow pair --start \
  --mode navigator \
  --agent expert-coder \
  --test \
  --language python
```

### Commands in Navigator Mode
```
/implement   - Direct implementation
/refactor    - Request refactoring
/test        - Generate tests
/document    - Add documentation
/alternate   - See alternative approaches
```

## Switch Mode

Automatically alternates between driver and navigator roles.

### Usage
```bash
claude-flow pair --start --mode switch [--interval <time>]
```

### Default Intervals
- **10 minutes** - Standard switching
- **5 minutes** - Rapid collaboration
- **15 minutes** - Deep focus periods
- **Custom** - Set your preference

### Configuration
```bash
# 5-minute intervals
claude-flow pair --start --mode switch --interval 5m

# 15-minute intervals
claude-flow pair --start --mode switch --interval 15m

# Hour-long intervals
claude-flow pair --start --mode switch --interval 1h
```

### Role Transitions

**Handoff Process:**
1. 30-second warning before switch
2. Current driver completes thought
3. Context summary generated
4. Roles swap smoothly
5. New driver continues

### Best For
- Balanced collaboration
- Knowledge sharing
- Complex features
- Extended sessions

### Example Session
```bash
claude-flow pair --start \
  --mode switch \
  --interval 10m \
  --verify \
  --test
```

## Specialized Modes

### TDD Mode
Test-Driven Development focus.

```bash
claude-flow pair --start \
  --mode tdd \
  --test-first \
  --coverage 100
```

**Workflow:**
1. Write failing test (Red)
2. Implement minimal code (Green)
3. Refactor (Refactor)
4. Repeat cycle

### Review Mode
Continuous code review focus.

```bash
claude-flow pair --start \
  --mode review \
  --strict \
  --security
```

**Features:**
- Real-time feedback
- Security scanning
- Performance analysis
- Best practice enforcement

### Mentor Mode
Learning-focused collaboration.

```bash
claude-flow pair --start \
  --mode mentor \
  --explain-all \
  --pace slow
```

**Features:**
- Detailed explanations
- Step-by-step guidance
- Pattern teaching
- Best practice examples

### Debug Mode
Problem-solving focus.

```bash
claude-flow pair --start \
  --mode debug \
  --verbose \
  --trace
```

**Features:**
- Issue identification
- Root cause analysis
- Fix suggestions
- Prevention strategies

## Mode Selection Guide

### Choose Driver Mode When:
- You want hands-on practice
- Learning new concepts
- Implementing your ideas
- Prefer writing code yourself

### Choose Navigator Mode When:
- Need rapid implementation
- Generating boilerplate
- Exploring AI suggestions
- Learning from examples

### Choose Switch Mode When:
- Long sessions planned
- Balanced collaboration needed
- Complex features
- Team simulation

### Choose Specialized Modes When:
- **TDD**: Building with tests
- **Review**: Quality focus
- **Mentor**: Learning priority
- **Debug**: Fixing issues

## Mode Comparison

| Mode | You Write | AI Writes | Best For | Switch Time |
|------|-----------|-----------|----------|-------------|
| Driver | ✅ | ❌ | Learning, Control | N/A |
| Navigator | ❌ | ✅ | Speed, Generation | N/A |
| Switch | ✅/❌ | ✅/❌ | Balance, Long Sessions | 5-60min |
| TDD | ✅/❌ | ✅/❌ | Test-First | Per cycle |
| Review | ✅ | ❌ | Quality | N/A |
| Mentor | ✅ | ❌ | Learning | N/A |
| Debug | ✅/❌ | ✅/❌ | Fixing | N/A |

## Mode Combinations

### Quality-Focused
```bash
claude-flow pair --start \
  --mode switch \
  --verify \
  --test \
  --review \
  --threshold 0.98
```

### Learning-Focused
```bash
claude-flow pair --start \
  --mode mentor \
  --explain-all \
  --examples \
  --pace slow
```

### Speed-Focused
```bash
claude-flow pair --start \
  --mode navigator \
  --quick \
  --templates \
  --no-review
```

### Debug-Focused
```bash
claude-flow pair --start \
  --mode debug \
  --trace \
  --verbose \
  --breakpoints
```

## Switching Modes Mid-Session

During any session, you can switch modes:

```
/mode driver     - Switch to driver mode
/mode navigator  - Switch to navigator mode
/mode switch     - Enable auto-switching
/mode tdd        - Switch to TDD mode
/mode review     - Switch to review mode
```

## Mode Persistence

Save mode preferences:

```json
// .claude-flow/config.json
{
  "pair": {
    "defaultMode": "switch",
    "switchInterval": "10m",
    "preferredRole": "driver",
    "autoSwitchOnIdle": true
  }
}
```

## Best Practices by Mode

### Driver Mode
1. Ask questions frequently
2. Request reviews often
3. Use suggestions wisely
4. Learn from feedback

### Navigator Mode
1. Provide clear direction
2. Review thoroughly
3. Test generated code
4. Understand implementations

### Switch Mode
1. Prepare for handoffs
2. Maintain context
3. Document decisions
4. Stay synchronized

## Related Documentation

- [Pair Programming Overview](./README.md)
- [Starting Sessions](./start.md)
- [Session Management](./session.md)
- [Configuration](./config.md)