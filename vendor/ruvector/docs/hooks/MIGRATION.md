# RuVector Hooks Migration Guide

Guide for migrating to RuVector's portable hooks system from legacy setups or other tools.

## Table of Contents

1. [Overview](#overview)
2. [Migration Paths](#migration-paths)
3. [From Legacy Intelligence](#from-legacy-intelligence)
4. [From Claude-Flow](#from-claude-flow)
5. [From Manual Setup](#from-manual-setup)
6. [Data Preservation](#data-preservation)
7. [Verification](#verification)
8. [Rollback](#rollback)

---

## Overview

### Why Migrate?

The new RuVector hooks system provides:

| Feature | Legacy | New System |
|---------|--------|------------|
| Portability | Hardcoded paths | Dynamic resolution |
| CLI Management | Manual JSON editing | Full CLI support |
| Cross-platform | Linux/macOS only | Linux, macOS, Windows |
| Global Patterns | Not available | Supported |
| Binary Updates | Hooks break | Survive reinstalls |

### Migration Safety

All migrations include:
- **Automatic backup** of existing data
- **Validation** of migrated data
- **Atomic operations** with rollback capability
- **Zero data loss** guarantee

---

## Migration Paths

### Quick Reference

| Source | Command | Time |
|--------|---------|------|
| Legacy `.claude/intelligence/` | `hooks migrate --from .claude/intelligence` | <5s |
| Claude-flow `memory.db` | `hooks migrate --from ~/.swarm/memory.db` | <10s |
| Exported JSON | `hooks import --input patterns.json` | <2s |
| Fresh start | `hooks init` | <1s |

### Prerequisites

Before migrating:

```bash
# 1. Backup existing data
cp -r .claude/intelligence .claude/intelligence.backup
cp -r ~/.swarm ~/.swarm.backup

# 2. Install latest RuVector CLI
npm install -g @ruvector/cli@latest

# 3. Verify installation
npx ruvector --version
```

---

## From Legacy Intelligence

Migrate from the repository-specific `.claude/intelligence/` system.

### Current Legacy Structure

```
.claude/
├── intelligence/
│   ├── data/
│   │   ├── memory.json       # Vector memories
│   │   ├── trajectories.json # Learning history
│   │   ├── patterns.json     # Q-learning patterns
│   │   └── feedback.json     # User feedback
│   ├── index.js              # Intelligence layer
│   └── cli.js                # CLI commands
└── settings.json             # Hardcoded hooks
```

### Migration Steps

#### Step 1: Initialize New System

```bash
npx ruvector hooks init
```

This creates:
```
.ruvector/
├── config.toml
├── intelligence/
│   └── (empty, ready for migration)
└── .gitignore
```

#### Step 2: Migrate Data

```bash
# Migrate with validation
npx ruvector hooks migrate \
  --from .claude/intelligence \
  --validate

# Expected output:
# Migrating from JSON files...
# ✓ Imported 1,247 trajectories
# ✓ Imported 89 Q-learning patterns
# ✓ Converted 543 memories to vectors
# ✓ Validation passed (100% integrity)
# ⏱ Completed in 3.2s
```

#### Step 3: Install New Hooks

```bash
# Install portable hooks
npx ruvector hooks install --force

# This replaces hardcoded paths with dynamic resolution
```

#### Step 4: Verify Migration

```bash
# Check statistics
npx ruvector hooks stats --verbose

# Should show migrated data:
# Patterns: 89
# Memories: 543
# Trajectories: 1,247
```

#### Step 5: Clean Up (Optional)

After confirming migration success:

```bash
# Remove legacy intelligence directory
rm -rf .claude/intelligence

# Keep backup for safety
# rm -rf .claude/intelligence.backup  # Only if confident
```

### Legacy Settings.json Update

**Before (hardcoded):**
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "command": "/bin/bash -c 'cd /workspaces/ruvector/.claude/intelligence && node cli.js pre-command'"
      }]
    }]
  }
}
```

**After (portable):**
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "command": "/bin/bash -c 'RUVECTOR=$(which ruvector || echo npx ruvector); $RUVECTOR hooks pre-command \"$CMD\"'"
      }]
    }]
  }
}
```

---

## From Claude-Flow

Migrate from Claude-Flow's SQLite memory database.

### Claude-Flow Structure

```
~/.swarm/
├── memory.db          # SQLite database
├── config.json        # Configuration
└── sessions/          # Session data
```

### Migration Steps

#### Step 1: Locate Memory Database

```bash
# Default location
ls ~/.swarm/memory.db

# Custom location (check config)
cat ~/.swarm/config.json | jq '.memoryPath'
```

#### Step 2: Initialize RuVector

```bash
cd your-project
npx ruvector hooks init
```

#### Step 3: Migrate SQLite Data

```bash
# Migrate from SQLite
npx ruvector hooks migrate \
  --from ~/.swarm/memory.db \
  --format sqlite \
  --validate

# Output:
# Migrating from SQLite database...
# ✓ Extracted 2,500 trajectories
# ✓ Converted 150 Q-learning patterns
# ✓ Migrated 1,200 memories to vectors
# ✓ Validation passed
```

**Note:** SQLite migration requires the `sqlite-migration` feature (v1.1+). For MVP, use JSON export:

```bash
# Alternative: Export from claude-flow first
npx claude-flow memory export --output memory-export.json

# Then import
npx ruvector hooks import --input memory-export.json
```

#### Step 4: Merge with Existing Data

If you have both legacy and claude-flow data:

```bash
# Merge with existing patterns
npx ruvector hooks migrate \
  --from ~/.swarm/memory.db \
  --merge \
  --strategy average
```

#### Step 5: Install Hooks

```bash
npx ruvector hooks install
```

---

## From Manual Setup

Migrate from manually configured hooks.

### Current Manual Setup

**`.claude/settings.json` (manual):**
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Write|Edit",
      "hooks": [{
        "type": "command",
        "command": "echo 'Pre-edit hook'"
      }]
    }]
  }
}
```

### Migration Steps

#### Step 1: Backup Existing Settings

```bash
cp .claude/settings.json .claude/settings.json.manual-backup
```

#### Step 2: Initialize RuVector

```bash
npx ruvector hooks init
```

#### Step 3: Install with Merge

```bash
# Merge RuVector hooks with existing
npx ruvector hooks install --merge

# This preserves your custom hooks and adds RuVector hooks
```

#### Step 4: Review Merged Settings

```bash
# View the merged settings
cat .claude/settings.json

# Verify your custom hooks are preserved
```

### Preserving Custom Hooks

If you have custom hooks to preserve:

**Before install:**
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "CustomTool",
      "hooks": [{
        "command": "my-custom-hook.sh"
      }]
    }]
  }
}
```

**After install (merged):**
```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{
          "command": "npx ruvector hooks pre-command"
        }]
      },
      {
        "matcher": "CustomTool",
        "hooks": [{
          "command": "my-custom-hook.sh"
        }]
      }
    ]
  }
}
```

---

## Data Preservation

### What Gets Migrated

| Data Type | Source | Destination |
|-----------|--------|-------------|
| Q-learning patterns | `patterns.json` | `.ruvector/intelligence/patterns.json` |
| Trajectories | `trajectories.json` | `.ruvector/intelligence/trajectories.json` |
| Vector memories | `memory.json` | `.ruvector/intelligence/memory.rvdb` |
| Feedback data | `feedback.json` | `.ruvector/intelligence/feedback.json` |
| Configuration | settings.json | `.ruvector/config.toml` |

### Data Integrity Checks

The migration process includes:

1. **Checksum validation**: Verify data wasn't corrupted
2. **Count verification**: Ensure all records migrated
3. **Q-value preservation**: Maintain learned values
4. **Vector accuracy**: Preserve embedding precision

### Backup Locations

Automatic backups are created:

```
.ruvector/
├── intelligence/
│   └── backup-YYYYMMDD-HHMMSS/
│       ├── patterns.json
│       ├── trajectories.json
│       └── memory.json
```

---

## Verification

### Verify Migration Success

```bash
# 1. Check statistics
npx ruvector hooks stats --verbose

# 2. Compare counts
echo "Legacy patterns: $(jq '.patterns | length' .claude/intelligence.backup/data/patterns.json 2>/dev/null || echo 0)"
echo "Migrated patterns: $(npx ruvector hooks stats --json | jq '.patterns')"

# 3. Test hook execution
npx ruvector hooks pre-edit --file test.ts
npx ruvector hooks post-edit --file test.ts --success true

# 4. Verify session hooks
npx ruvector hooks session-start --session-id "migration-test"
npx ruvector hooks session-end --session-id "migration-test"
```

### Expected Verification Output

```bash
$ npx ruvector hooks stats --verbose

RuVector Intelligence Statistics
================================

Data Migration Status: SUCCESS

Learning Data:
   Trajectories: 1,247 (migrated: 1,247)
   Patterns: 89 (migrated: 89)
   Memories: 543 vectors (migrated: 543)
   Integrity: 100%

Configuration:
   Hooks installed: Yes
   Portable paths: Yes
   Intelligence enabled: Yes
```

### Test in Claude Code

1. Open Claude Code in your project
2. Verify session start message appears
3. Make an edit to a file
4. Confirm agent assignment message
5. Check post-edit formatting

---

## Rollback

### Automatic Rollback

If migration fails, automatic rollback occurs:

```bash
$ npx ruvector hooks migrate --from .claude/intelligence

Migrating from JSON files...
✓ Imported 1,247 trajectories
✗ Error during pattern migration: Invalid Q-value format
⟲ Rolling back migration...
✓ Restored from backup
Migration failed, original data preserved
```

### Manual Rollback

To manually rollback:

#### Step 1: Restore Backup

```bash
# Restore intelligence data
rm -rf .ruvector/intelligence
cp -r .ruvector/intelligence/backup-YYYYMMDD-HHMMSS/* .ruvector/intelligence/

# Or restore legacy location
rm -rf .ruvector
mv .claude/intelligence.backup .claude/intelligence
```

#### Step 2: Restore Settings

```bash
# Restore Claude settings
cp .claude/settings.json.backup .claude/settings.json
```

#### Step 3: Verify Restoration

```bash
# For legacy
node .claude/intelligence/cli.js stats

# For new system
npx ruvector hooks stats
```

### Complete Reset

To completely reset and start fresh:

```bash
# Remove all RuVector data
rm -rf .ruvector

# Remove from Claude settings
# Edit .claude/settings.json to remove hooks section

# Reinitialize
npx ruvector hooks init
npx ruvector hooks install
```

---

## Migration FAQ

### Q: Will I lose my learned patterns?

**A:** No. All migrations include automatic backup and validation. Q-values, trajectories, and memories are preserved with 100% integrity.

### Q: Can I migrate incrementally?

**A:** Yes. Use the `--merge` flag to add new data without replacing existing:

```bash
npx ruvector hooks migrate --from new-data.json --merge
```

### Q: What about Windows compatibility?

**A:** The new system uses conditional shell detection:

```bash
# Windows
cmd /c 'npx ruvector hooks ...'

# Linux/macOS
/bin/bash -c 'npx ruvector hooks ...'
```

### Q: How do I migrate a team project?

**A:** Export and share patterns:

```bash
# Team member 1: Export
npx ruvector hooks export --output team-patterns.json

# Team member 2: Import and merge
npx ruvector hooks import --input team-patterns.json --merge
```

### Q: Is the migration reversible?

**A:** Yes. Backups are automatically created and manual rollback is always possible.

---

## Post-Migration Checklist

- [ ] `npx ruvector hooks stats` shows expected counts
- [ ] Session hooks trigger on Claude Code start
- [ ] Pre-edit hooks assign agents correctly
- [ ] Post-edit hooks format code
- [ ] No hardcoded paths in `.claude/settings.json`
- [ ] Backup data stored safely
- [ ] Team notified of migration (if applicable)

---

## See Also

- [User Guide](USER_GUIDE.md) - Getting started
- [CLI Reference](CLI_REFERENCE.md) - Command documentation
- [Architecture](ARCHITECTURE.md) - Technical details
- [Troubleshooting](TROUBLESHOOTING.md) - Common issues
