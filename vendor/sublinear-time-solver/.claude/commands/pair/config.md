# Pair Programming Configuration

Complete configuration guide for pair programming sessions.

## Configuration File

Main configuration file: `.claude-flow/pair-config.json`

## Basic Configuration

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

## Complete Configuration

```json
{
  "pair": {
    "general": {
      "enabled": true,
      "defaultMode": "switch",
      "defaultAgent": "senior-dev",
      "autoStart": false,
      "theme": "professional",
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
      },
      "watch": true,
      "parallel": true
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
      "metricsInterval": "1m",
      "recordSession": false,
      "shareByDefault": false
    },
    
    "ai": {
      "model": "advanced",
      "temperature": 0.7,
      "maxTokens": 4000,
      "contextWindow": 8000,
      "personality": "professional",
      "expertise": ["backend", "testing", "security"],
      "learningEnabled": true
    },
    
    "interface": {
      "theme": "dark",
      "fontSize": 14,
      "showMetrics": true,
      "notifications": true,
      "sounds": false,
      "shortcuts": {
        "switch": "ctrl+shift+s",
        "suggest": "ctrl+space",
        "review": "ctrl+r",
        "test": "ctrl+t"
      }
    },
    
    "quality": {
      "linting": {
        "enabled": true,
        "autoFix": true,
        "rules": "standard"
      },
      "formatting": {
        "enabled": true,
        "autoFormat": true,
        "style": "prettier"
      },
      "documentation": {
        "required": true,
        "format": "jsdoc",
        "checkCompleteness": true
      }
    },
    
    "advanced": {
      "parallelSessions": false,
      "multiAgent": false,
      "customAgents": [],
      "plugins": [],
      "webhooks": {
        "onStart": "",
        "onEnd": "",
        "onCommit": "",
        "onError": ""
      }
    }
  }
}
```

## Agent Configuration

### Built-in Agents

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

### Custom Agents

```json
{
  "customAgents": [
    {
      "id": "security-expert",
      "name": "Security Specialist",
      "expertise": ["security", "cryptography", "vulnerabilities"],
      "personality": "cautious",
      "prompts": {
        "review": "Focus on security vulnerabilities",
        "suggest": "Prioritize secure coding practices"
      }
    }
  ]
}
```

## Mode-Specific Configuration

### Driver Mode
```json
{
  "driver": {
    "autocomplete": {
      "enabled": false,
      "delay": 500,
      "minChars": 3
    },
    "suggestions": {
      "enabled": true,
      "frequency": "onChange",
      "inline": true
    },
    "review": {
      "realTime": true,
      "highlighting": true,
      "annotations": true
    }
  }
}
```

### Navigator Mode
```json
{
  "navigator": {
    "generation": {
      "style": "verbose",
      "comments": true,
      "tests": true
    },
    "explanation": {
      "level": "detailed",
      "examples": true,
      "alternatives": 3
    }
  }
}
```

### Switch Mode
```json
{
  "switch": {
    "intervals": {
      "default": "10m",
      "minimum": "5m",
      "maximum": "30m"
    },
    "handoff": {
      "summary": true,
      "context": true,
      "nextSteps": true
    }
  }
}
```

## Quality Thresholds

```json
{
  "thresholds": {
    "verification": {
      "error": 0.90,
      "warning": 0.95,
      "success": 0.98
    },
    "coverage": {
      "error": 70,
      "warning": 80,
      "success": 90
    },
    "complexity": {
      "error": 15,
      "warning": 10,
      "success": 5
    }
  }
}
```

## Language-Specific Settings

### JavaScript/TypeScript
```json
{
  "languages": {
    "javascript": {
      "framework": "react",
      "linter": "eslint",
      "formatter": "prettier",
      "testRunner": "jest",
      "transpiler": "babel"
    }
  }
}
```

### Python
```json
{
  "languages": {
    "python": {
      "version": "3.11",
      "linter": "pylint",
      "formatter": "black",
      "testRunner": "pytest",
      "typeChecker": "mypy"
    }
  }
}
```

## Environment Variables

```bash
# Override configuration via environment
export CLAUDE_PAIR_MODE=driver
export CLAUDE_PAIR_VERIFY=true
export CLAUDE_PAIR_THRESHOLD=0.98
export CLAUDE_PAIR_AGENT=senior-dev
export CLAUDE_PAIR_AUTO_TEST=true
```

## CLI Configuration

### Set Configuration
```bash
# Set single value
claude-flow pair config set defaultMode switch

# Set nested value
claude-flow pair config set verification.threshold 0.98

# Set from file
claude-flow pair config import config.json
```

### Get Configuration
```bash
# Get all configuration
claude-flow pair config get

# Get specific value
claude-flow pair config get defaultMode

# Export configuration
claude-flow pair config export > config.json
```

### Reset Configuration
```bash
# Reset to defaults
claude-flow pair config reset

# Reset specific section
claude-flow pair config reset verification
```

## Profile Management

### Create Profile
```bash
claude-flow pair profile create refactoring \
  --mode driver \
  --verify true \
  --threshold 0.98 \
  --focus refactor
```

### Use Profile
```bash
claude-flow pair --start --profile refactoring
```

### List Profiles
```bash
claude-flow pair profile list
```

### Profiles Configuration
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

## Workspace Configuration

### Project-Specific
`.claude-flow/pair-config.json` in project root

### User-Specific
`~/.claude-flow/pair-config.json`

### Global
`/etc/claude-flow/pair-config.json`

### Priority Order
1. Command-line arguments
2. Environment variables
3. Project configuration
4. User configuration
5. Global configuration
6. Built-in defaults

## Configuration Validation

```bash
# Validate configuration
claude-flow pair config validate

# Test configuration
claude-flow pair config test
```

## Migration

### From Version 1.x
```bash
claude-flow pair config migrate --from 1.x
```

### Export/Import
```bash
# Export current config
claude-flow pair config export > my-config.json

# Import config
claude-flow pair config import my-config.json
```

## Best Practices

1. **Start Simple** - Use defaults, customize as needed
2. **Version Control** - Commit project config
3. **Team Standards** - Share configurations
4. **Regular Review** - Update thresholds based on metrics
5. **Profile Usage** - Create profiles for common scenarios

## Troubleshooting

### Configuration Not Loading
- Check file syntax (JSON)
- Verify file permissions
- Check priority order
- Validate configuration

### Settings Not Applied
- Restart session
- Clear cache
- Check overrides
- Review logs

## Related Documentation

- [Getting Started](./README.md)
- [Session Management](./session.md)
- [Modes](./modes.md)
- [Templates](./templates.md)