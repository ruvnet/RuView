# RuVector Hooks Troubleshooting Guide

Solutions for common issues with the RuVector hooks system.

## Table of Contents

1. [Quick Diagnostics](#quick-diagnostics)
2. [Installation Issues](#installation-issues)
3. [Hook Execution Issues](#hook-execution-issues)
4. [Intelligence Layer Issues](#intelligence-layer-issues)
5. [Performance Issues](#performance-issues)
6. [Platform-Specific Issues](#platform-specific-issues)
7. [Migration Issues](#migration-issues)
8. [Debug Mode](#debug-mode)

---

## Quick Diagnostics

### Run Full Diagnostic

```bash
# Check overall health
npx ruvector hooks stats --verbose

# Validate configuration
npx ruvector hooks validate-config

# Test hook execution
npx ruvector hooks pre-edit --file test.ts
npx ruvector hooks post-edit --file test.ts --success true
```

### Common Symptoms and Solutions

| Symptom | Likely Cause | Solution |
|---------|--------------|----------|
| Hooks not running | Missing settings.json | Run `hooks install` |
| "Command not found" | CLI not in PATH | Use `npx ruvector` |
| No agent assignment | Intelligence disabled | Set `RUVECTOR_INTELLIGENCE_ENABLED=true` |
| Slow hook execution | Large memory | Clean old trajectories |
| Windows errors | Shell mismatch | Check shell wrapper |

---

## Installation Issues

### Problem: `hooks init` fails

**Symptoms:**
```
Error: Failed to create .ruvector directory
Permission denied
```

**Solutions:**

1. Check directory permissions:
```bash
ls -la .
# Ensure you have write access
```

2. Create directory manually:
```bash
mkdir -p .ruvector/intelligence
npx ruvector hooks init
```

3. Use sudo (last resort):
```bash
sudo npx ruvector hooks init
sudo chown -R $USER:$USER .ruvector
```

---

### Problem: `hooks install` doesn't update settings

**Symptoms:**
- `.claude/settings.json` unchanged
- Old hooks still running

**Solutions:**

1. Use `--force` flag:
```bash
npx ruvector hooks install --force
```

2. Check backup and restore:
```bash
# View backup
cat .claude/settings.json.backup

# If needed, restore and try again
cp .claude/settings.json.backup .claude/settings.json
npx ruvector hooks install --force
```

3. Manually edit settings:
```bash
# Open and verify hook section
code .claude/settings.json
```

---

### Problem: "npx ruvector" command not found

**Symptoms:**
```
npm ERR! could not determine executable to run
```

**Solutions:**

1. Install globally:
```bash
npm install -g @ruvector/cli
ruvector hooks init
```

2. Check npm configuration:
```bash
npm config get prefix
# Ensure this is in your PATH
```

3. Use npx with package:
```bash
npx @ruvector/cli hooks init
```

---

## Hook Execution Issues

### Problem: Hooks not triggering

**Symptoms:**
- No output when editing files
- Session start message missing
- Intelligence not active

**Diagnosis:**

```bash
# Check settings.json has hooks
cat .claude/settings.json | jq '.hooks'

# Should show PreToolUse, PostToolUse, etc.
```

**Solutions:**

1. Reinstall hooks:
```bash
npx ruvector hooks install --force
```

2. Check matcher patterns:
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",  // Case-sensitive!
      "hooks": [...]
    }]
  }
}
```

3. Verify Claude Code is loading settings:
```bash
# Restart Claude Code to reload settings
```

---

### Problem: Hook timeout

**Symptoms:**
```
Warning: Hook timeout after 3000ms
```

**Solutions:**

1. Increase timeout in settings:
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "timeout": 5000,  // Increase to 5 seconds
        "command": "..."
      }]
    }]
  }
}
```

2. Check for slow operations:
```bash
# Time hook execution
time npx ruvector hooks pre-edit --file test.ts
```

3. Reduce hook complexity:
- Disable neural training in pre-hooks
- Use async for heavy operations
- Cache repeated lookups

---

### Problem: Hook blocks tool execution

**Symptoms:**
- Edit operations not completing
- "continue: false" in output

**Diagnosis:**

```bash
# Test hook directly
npx ruvector hooks pre-edit --file problematic-file.ts

# Check response
# { "continue": false, "reason": "..." }
```

**Solutions:**

1. Check protected files:
```bash
# If file is protected, you'll see:
# { "continue": false, "reason": "Protected file" }

# Add to exceptions in config.toml
[hooks]
protected_exceptions = [".env.local"]
```

2. Disable blocking:
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Write",
      "hooks": [{
        "command": "...",
        "continueOnError": true  // Never block on error
      }]
    }]
  }
}
```

---

## Intelligence Layer Issues

### Problem: No agent suggestions

**Symptoms:**
- `assignedAgent` always null
- No intelligence guidance

**Diagnosis:**

```bash
# Check intelligence status
npx ruvector hooks stats

# Expected output:
# Patterns: N
# Memories: N
# Status: Ready
```

**Solutions:**

1. Enable intelligence:
```bash
export RUVECTOR_INTELLIGENCE_ENABLED=true
```

2. Check data files exist:
```bash
ls -la .ruvector/intelligence/
# Should show patterns.json, memory.json, etc.
```

3. Initialize fresh data:
```bash
npx ruvector hooks init --force
```

---

### Problem: Poor agent suggestions

**Symptoms:**
- Wrong agent assigned to file types
- Low confidence scores

**Diagnosis:**

```bash
# Check patterns
npx ruvector hooks stats --verbose

# Look for:
# Top Patterns:
#   1. edit_rs_in_xxx → rust-developer (Q=0.82)
```

**Solutions:**

1. Reset learning data:
```bash
rm .ruvector/intelligence/patterns.json
rm .ruvector/intelligence/trajectories.json
# Will rebuild from scratch
```

2. Import team patterns:
```bash
npx ruvector hooks import --input team-patterns.json
```

3. Wait for learning:
- Patterns improve with use
- 50+ edits needed for good suggestions

---

### Problem: Memory search slow or failing

**Symptoms:**
- Memory search timeout
- "Error: Failed to load memory"

**Diagnosis:**

```bash
# Check memory size
ls -la .ruvector/intelligence/memory.json

# If >10MB, consider cleanup
```

**Solutions:**

1. Clean old memories:
```bash
# Backup first
cp .ruvector/intelligence/memory.json memory-backup.json

# Keep only recent
node -e "
const fs = require('fs');
const data = JSON.parse(fs.readFileSync('.ruvector/intelligence/memory.json'));
const recent = data.slice(-1000);  // Keep last 1000
fs.writeFileSync('.ruvector/intelligence/memory.json', JSON.stringify(recent));
"
```

2. Rebuild HNSW index:
```bash
rm .ruvector/intelligence/memory.rvdb
# Will rebuild on next use
```

---

## Performance Issues

### Problem: High hook overhead

**Symptoms:**
- Slow file operations
- Noticeable delay on every edit

**Diagnosis:**

```bash
# Time individual hooks
time npx ruvector hooks pre-edit --file test.ts
time npx ruvector hooks post-edit --file test.ts --success true

# Target: <50ms each
```

**Solutions:**

1. Disable neural training:
```bash
# In config.toml
[intelligence]
neural_training = false
```

2. Reduce memory operations:
```toml
[hooks]
store_memory = false  # Disable memory storage
```

3. Use async post-hooks:
```json
{
  "hooks": {
    "PostToolUse": [{
      "matcher": "Write",
      "hooks": [{
        "command": "...",
        "async": true  // Don't wait for completion
      }]
    }]
  }
}
```

---

### Problem: Large intelligence data files

**Symptoms:**
- `.ruvector/intelligence/` >100MB
- Slow startup

**Solutions:**

1. Set retention limits:
```toml
# In config.toml
[intelligence]
max_trajectories = 1000
max_memories = 10000
```

2. Clean old data:
```bash
# Export current patterns
npx ruvector hooks export --output patterns-backup.json --include patterns

# Reset
rm -rf .ruvector/intelligence/*

# Re-import patterns
npx ruvector hooks import --input patterns-backup.json
```

---

## Platform-Specific Issues

### Windows Issues

#### Problem: "/bin/bash not found"

**Symptoms:**
```
'/bin/bash' is not recognized as an internal or external command
```

**Solution:**

Check that hooks use Windows-compatible shell:

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "command": "cmd /c 'npx ruvector hooks pre-command'"
      }]
    }]
  }
}
```

Or reinstall hooks (auto-detects platform):
```bash
npx ruvector hooks install --force
```

#### Problem: Path separator issues

**Symptoms:**
- File paths not recognized
- "File not found" errors

**Solution:**

Ensure paths use forward slashes or escaped backslashes:

```bash
# Good
npx ruvector hooks pre-edit --file "src/app.ts"

# Bad on Windows
npx ruvector hooks pre-edit --file "src\app.ts"
```

#### Problem: jq not found

**Symptoms:**
```
'jq' is not recognized as an internal or external command
```

**Solutions:**

1. Install jq:
```bash
# Using chocolatey
choco install jq

# Using scoop
scoop install jq
```

2. Or use jq-free hooks:
```bash
npx ruvector hooks install --template minimal
```

---

### macOS Issues

#### Problem: Permission denied

**Symptoms:**
```
Error: EACCES: permission denied
```

**Solutions:**

1. Fix npm permissions:
```bash
sudo chown -R $(whoami) ~/.npm
```

2. Use nvm:
```bash
# Install nvm and use it for npm
nvm install node
nvm use node
```

---

### Linux Issues

#### Problem: Node.js version too old

**Symptoms:**
```
SyntaxError: Unexpected token '.'
```

**Solution:**

Update Node.js:
```bash
# Using nvm
nvm install 18
nvm use 18

# Or using package manager
curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash -
sudo apt-get install -y nodejs
```

---

## Migration Issues

### Problem: Migration data loss

**Symptoms:**
- Fewer patterns after migration
- Missing memories

**Diagnosis:**

```bash
# Compare counts
echo "Before: $(jq '.length' old-patterns.json)"
echo "After: $(npx ruvector hooks stats --json | jq '.patterns')"
```

**Solutions:**

1. Use validation:
```bash
npx ruvector hooks migrate --from old-data --validate
```

2. Merge instead of replace:
```bash
npx ruvector hooks migrate --from old-data --merge
```

3. Restore from backup:
```bash
cp .ruvector/intelligence/backup-*/* .ruvector/intelligence/
```

---

### Problem: SQLite migration format error

**Symptoms:**
```
Error: Unknown embedding format in memory.db
```

**Solution:**

SQLite migration requires format detection. For MVP, use JSON export:

```bash
# Export from source as JSON first
npx claude-flow memory export --output memory.json

# Then import
npx ruvector hooks import --input memory.json
```

---

## Debug Mode

### Enable Debug Output

```bash
# Set environment variable
export CLAUDE_FLOW_DEBUG=true
export RUVECTOR_DEBUG=true

# Run with debug
npx ruvector hooks pre-edit --file test.ts --debug
```

### Debug Output Interpretation

```
DEBUG: Loading config from .ruvector/config.toml
DEBUG: Intelligence enabled: true
DEBUG: Q-table loaded: 89 patterns
DEBUG: Memory loaded: 543 vectors
DEBUG: Encoding state for test.ts
DEBUG: State key: edit_ts_in_project
DEBUG: Q-values: { "typescript-developer": 0.82, "coder": 0.45 }
DEBUG: Selected agent: typescript-developer (confidence: 0.82)
```

### View Hook Logs

```bash
# Today's logs
cat .ruvector/logs/hooks-$(date +%Y-%m-%d).log

# Tail logs
tail -f .ruvector/logs/hooks-*.log
```

### Test Hooks Manually

```bash
# Test pre-edit
echo '{"tool_input":{"file_path":"test.ts"}}' | npx ruvector hooks pre-edit --stdin

# Test post-edit
echo '{"tool_input":{"file_path":"test.ts"},"tool_result":{"success":true}}' | npx ruvector hooks post-edit --stdin
```

---

## Getting Help

### Gather Diagnostic Info

```bash
# Create diagnostic report
{
  echo "=== RuVector Version ==="
  npx ruvector --version

  echo -e "\n=== Node Version ==="
  node --version

  echo -e "\n=== Platform ==="
  uname -a

  echo -e "\n=== Hooks Stats ==="
  npx ruvector hooks stats --json

  echo -e "\n=== Config ==="
  cat .ruvector/config.toml

  echo -e "\n=== Settings ==="
  cat .claude/settings.json | jq '.hooks'
} > ruvector-diagnostic.txt

echo "Diagnostic saved to ruvector-diagnostic.txt"
```

### Report Issues

1. Create diagnostic report (above)
2. Open issue: https://github.com/ruvnet/ruvector/issues
3. Include:
   - Diagnostic report
   - Steps to reproduce
   - Expected vs actual behavior

---

## See Also

- [User Guide](USER_GUIDE.md) - Getting started
- [CLI Reference](CLI_REFERENCE.md) - Command documentation
- [Architecture](ARCHITECTURE.md) - Technical details
- [Migration Guide](MIGRATION.md) - Upgrade from other systems
