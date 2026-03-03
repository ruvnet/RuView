# hook pre-task

Execute pre-task preparations and context loading.

## Usage

```bash
npx claude-flow hook pre-task [options]
```

## Options

- `--description, -d <text>` - Task description for context
- `--auto-spawn-agents` - Automatically spawn required agents (default: true)
- `--load-memory` - Load relevant memory from previous sessions
- `--optimize-topology` - Select optimal swarm topology
- `--estimate-complexity` - Analyze task complexity

## Examples

### Basic pre-task hook

```bash
npx claude-flow hook pre-task --description "Implement user authentication"
```

### With memory loading

```bash
npx claude-flow hook pre-task -d "Continue API development" --load-memory
```

### Manual agent control

```bash
npx claude-flow hook pre-task -d "Debug issue #123" --auto-spawn-agents false
```

### Full optimization

```bash
npx claude-flow hook pre-task -d "Refactor codebase" --optimize-topology --estimate-complexity
```

## Features

### Auto Agent Assignment

- Analyzes task requirements
- Determines needed agent types
- Spawns agents automatically
- Configures agent parameters

### Memory Loading

- Retrieves relevant past decisions
- Loads previous task contexts
- Restores agent configurations
- Maintains continuity

### Topology Optimization

- Analyzes task structure
- Selects best swarm topology
- Configures communication patterns
- Optimizes for performance

### Complexity Estimation

- Evaluates task difficulty
- Estimates time requirements
- Suggests agent count
- Identifies dependencies

## Integration

This hook is automatically called by Claude Code when:

- Starting a new task
- Resuming work after a break
- Switching between projects
- Beginning complex operations

Manual usage in agents:

```bash
# In agent coordination
npx claude-flow hook pre-task --description "Your task here"
```

## Output

Returns JSON with:

```json
{
  "continue": true,
  "topology": "hierarchical",
  "agentsSpawned": 5,
  "complexity": "medium",
  "estimatedMinutes": 30,
  "memoryLoaded": true
}
```

## See Also

- `hook post-task` - Post-task cleanup
- `agent spawn` - Manual agent creation
- `memory usage` - Memory management
- `swarm init` - Swarm initialization
