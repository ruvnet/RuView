# stream-chain run

Execute a custom stream chain with your own prompts.

## Usage

```bash
claude-flow stream-chain run <prompt1> <prompt2> [...] [options]
```

Minimum 2 prompts required for chaining.

## Options

- `--verbose` - Show detailed execution information
- `--timeout <seconds>` - Timeout per step (default: 30)
- `--debug` - Enable debug mode

## How It Works

Each prompt in the chain receives the complete output from the previous step as context, enabling complex multi-step workflows.

## Examples

### Basic Chain
```bash
claude-flow stream-chain run \
  "Write a function" \
  "Add tests for it"
```

### Complex Workflow
```bash
claude-flow stream-chain run \
  "Analyze the authentication system" \
  "Identify security vulnerabilities" \
  "Propose fixes with priority levels" \
  "Implement the critical fixes" \
  "Generate tests for the fixes"
```

### With Options
```bash
claude-flow stream-chain run \
  "Complex analysis task" \
  "Detailed implementation" \
  --timeout 60 \
  --verbose
```

## Context Preservation

The output from each step is injected into the next prompt:

```
Step 1: "Write a sorting function"
Output: [function code]

Step 2 receives: "Previous step output:
[function code]

Next step: Optimize for performance"
```

## Best Practices

1. **Clear Instructions**: Make each prompt specific
2. **Logical Flow**: Order prompts in logical sequence
3. **Appropriate Timeouts**: Increase for complex tasks
4. **Verification**: Add verification steps in chain