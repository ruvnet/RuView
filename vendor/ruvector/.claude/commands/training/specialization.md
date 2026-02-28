# Agent Specialization Training

## Purpose
Train agents to become experts in specific domains for better performance.

## Specialization Areas

### 1. By File Type
Agents automatically specialize based on file extensions:
- **.js/.ts**: Modern JavaScript patterns
- **.py**: Pythonic idioms
- **.go**: Go best practices
- **.rs**: Rust safety patterns

### 2. By Task Type
```
Tool: mcp__claude-flow__agent_spawn
Parameters: {
  "type": "coder",
  "capabilities": ["react", "typescript", "testing"],
  "name": "React Specialist"
}
```

### 3. Training Process
The system trains through:
- Successful edit operations
- Code review patterns
- Error fix approaches
- Performance optimizations

### 4. Specialization Benefits
```
# Check agent specializations
Tool: mcp__claude-flow__agent_list
Parameters: {"swarmId": "current"}

Result shows expertise levels:
{
  "agents": [
    {
      "id": "coder-123",
      "specializations": {
        "javascript": 0.95,
        "react": 0.88,
        "testing": 0.82
      }
    }
  ]
}
```

## Continuous Improvement
Agents share learnings across sessions for cumulative expertise!

## CLI Usage
```bash
# Train agent specialization via CLI
npx claude-flow train agent --type coder --capabilities "react,typescript"

# Check specializations
npx claude-flow agent list --specializations
```