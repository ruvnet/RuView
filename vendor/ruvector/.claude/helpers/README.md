# Claude Flow V3 Helpers

This directory contains helper scripts and utilities for V3 development.

## 🚀 Quick Start

```bash
# Initialize V3 development environment
.claude/helpers/v3.sh init

# Quick status check
.claude/helpers/v3.sh status

# Update progress metrics
.claude/helpers/v3.sh update domain 3
.claude/helpers/v3.sh update agent 8
.claude/helpers/v3.sh update security 2
```

## Available Helpers

### 🎛️ V3 Master Tool
- **`v3.sh`** - Main command-line interface for all V3 operations
  ```bash
  .claude/helpers/v3.sh help           # Show all commands
  .claude/helpers/v3.sh status         # Quick development status
  .claude/helpers/v3.sh update domain 3 # Update specific metrics
  .claude/helpers/v3.sh validate       # Validate configuration
  .claude/helpers/v3.sh full-status    # Complete status overview
  ```

### 📊 V3 Progress Management
- **`update-v3-progress.sh`** - Update V3 development metrics
  ```bash
  # Usage examples:
  .claude/helpers/update-v3-progress.sh domain 3      # Mark 3 domains complete
  .claude/helpers/update-v3-progress.sh agent 8       # 8 agents active
  .claude/helpers/update-v3-progress.sh security 2    # 2 CVEs fixed
  .claude/helpers/update-v3-progress.sh performance 2.5x # Performance boost
  .claude/helpers/update-v3-progress.sh status        # Show current status
  ```

### 🔍 Configuration Validation
- **`validate-v3-config.sh`** - Comprehensive environment validation
  - Checks all required directories and files
  - Validates JSON configuration files
  - Verifies Node.js and development tools
  - Confirms Git repository status
  - Validates file permissions

### ⚡ Quick Status
- **`v3-quick-status.sh`** - Compact development progress overview
  - Shows domain, agent, and DDD progress
  - Displays security and performance metrics
  - Color-coded status indicators
  - Current Git branch information

## Helper Script Standards

### File Naming
- Use kebab-case: `update-v3-progress.sh`
- Include version prefix: `v3-*` for V3-specific helpers
- Use descriptive names that indicate purpose

### Script Requirements
- Must be executable (`chmod +x`)
- Include proper error handling (`set -e`)
- Provide usage help when called without arguments
- Use consistent exit codes (0 = success, non-zero = error)

### Configuration Integration
Helpers are configured in `.claude/settings.json`:
```json
{
  "helpers": {
    "directory": ".claude/helpers",
    "enabled": true,
    "v3ProgressUpdater": ".claude/helpers/update-v3-progress.sh"
  }
}
```

## Development Guidelines

1. **Security First**: All helpers must validate inputs
2. **Idempotent**: Scripts should be safe to run multiple times
3. **Fast Execution**: Keep helper execution under 1 second when possible
4. **Clear Output**: Provide clear success/error messages
5. **JSON Safe**: When updating JSON files, use `jq` for safety

## Adding New Helpers

1. Create script in `.claude/helpers/`
2. Make executable: `chmod +x script-name.sh`
3. Add to settings.json helpers section
4. Test thoroughly before committing
5. Update this README with usage documentation