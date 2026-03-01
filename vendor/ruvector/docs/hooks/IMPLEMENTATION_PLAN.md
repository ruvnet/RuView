# RuVector Generic Hooks System - Implementation Plan

> **Related Documentation**: [README](README.md) | [User Guide](USER_GUIDE.md) | [CLI Reference](CLI_REFERENCE.md) | [Architecture](ARCHITECTURE.md)

## Executive Summary

This document outlines a comprehensive SPARC-GOAP (Specification, Pseudocode, Architecture, Refinement, Completion + Goal-Oriented Action Planning) implementation plan for transforming the current repo-specific hooks system into a **generic, portable, CLI-integrated hooks system** for the ruvector project.

### Key Objectives
1. **Portability**: Transform hardcoded paths into dynamic, project-agnostic configurations
2. **CLI Integration**: Add `npx ruvector hooks` commands for easy setup and management
3. **Memory Migration**: Migrate claude-flow's SQLite memory.db to ruvector's native HNSW vector storage
4. **Universal Compatibility**: Enable any project to benefit from intelligent Claude Code hooks

---

## 1. GOAL STATE DEFINITION

### 1.1 Current State Analysis

**Repository-Specific System** (`.claude/settings.json`):
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "command": "/bin/bash -c '... cd /workspaces/ruvector/.claude/intelligence && node cli.js ...'"
      }]
    }]
  }
}
```

**Problems**:
- ❌ Hardcoded path: `/workspaces/ruvector/.claude/intelligence`
- ❌ Not portable across projects
- ❌ Requires manual setup in each repository
- ❌ Memory data trapped in repo-specific JSON files
- ❌ No CLI integration for management

### 1.2 Desired Goal State

**Generic Portable System** (`npx ruvector hooks`):
```bash
# Any project can initialize hooks
npx ruvector hooks init

# Install into Claude Code settings
npx ruvector hooks install

# Migrate existing learning data
npx ruvector hooks migrate --from ~/.swarm/memory.db

# View statistics
npx ruvector hooks stats

# Export/import learned patterns
npx ruvector hooks export --output patterns.json
npx ruvector hooks import --input patterns.json
```

**Success Criteria**:
- ✅ Works in ANY project directory
- ✅ Uses project-local `.ruvector/` directory for state
- ✅ CLI commands for all hook operations
- ✅ Migrates claude-flow memory.db → ruvector HNSW
- ✅ Supports global learning patterns (~/.ruvector/global/)
- ✅ Zero hardcoded paths in generated hooks
- ✅ Packaged with `@ruvector/core` npm package

---

## 2. MILESTONES & SPARC PHASES

### Milestone 1: Specification & Architecture Design
**SPARC Phase**: Specification → Architecture
**Goal**: Define portable hooks architecture and CLI API contracts

#### Actions:
1. **Design CLI Command Structure**
   - Define all `ruvector hooks <command>` subcommands
   - Specify input/output contracts for each command
   - Design configuration schema for hooks

2. **Architecture for Portable Paths**
   - Use environment variables: `$RUVECTOR_HOME`, `$PROJECT_ROOT`
   - Define fallback strategy: project-local → global → embedded
   - Design hook template system with variable substitution

3. **Memory Migration Architecture**
   - Map SQLite memory.db schema to ruvector VectorDB format
   - Design trajectory → vector embedding conversion
   - Plan Q-learning state preservation

**Deliverables**:
- [ ] CLI API specification document
- [ ] Hook template design with variable placeholders
- [ ] Memory migration schema mapping
- [ ] Architecture diagrams (ASCII or Mermaid)

**Success Criteria**:
- All CLI commands have clear input/output contracts
- Hook templates are 100% path-agnostic
- Migration preserves all learning data integrity

---

### Milestone 2: CLI Infrastructure Implementation
**SPARC Phase**: Architecture → Refinement (TDD)
**Goal**: Implement `ruvector hooks` CLI with full subcommand support

#### Actions:
1. **Add Hooks Subcommand to Rust CLI**
   ```rust
   // In src/cli/commands.rs
   enum Commands {
       // ... existing commands
       Hooks {
           #[command(subcommand)]
           action: HooksCommands,
       },
   }

   enum HooksCommands {
       Init { path: Option<PathBuf> },
       Install,
       Migrate { from: PathBuf },
       Stats,
       Export { output: PathBuf },
       Import { input: PathBuf },
       Enable,
       Disable,
   }
   ```

2. **Implement Hook Template Generator**
   - Create templates with `{{RUVECTOR_CLI_PATH}}`, `{{PROJECT_ROOT}}` placeholders
   - Dynamic path resolution at runtime
   - Generate `.claude/settings.json` with portable hooks

3. **Project-Local State Management**
   - Initialize `.ruvector/` directory structure
   - Create `config.toml` for per-project settings
   - Set up intelligence data directories

**Deliverables**:
- [ ] `ruvector hooks init` command
- [ ] `ruvector hooks install` command
- [ ] Hook template engine with variable substitution
- [ ] `.ruvector/` directory scaffolding
- [ ] Unit tests for CLI commands

**Success Criteria**:
- `npx ruvector hooks init` works in any directory
- Generated hooks contain zero hardcoded paths
- Tests verify cross-platform compatibility (Linux, macOS, Windows)

---

### Milestone 3: Intelligence Layer Portability
**SPARC Phase**: Refinement (TDD)
**Goal**: Make intelligence layer (`index.js`, `cli.js`) portable and embeddable

#### Actions:
1. **Refactor Intelligence Layer for Dynamic Paths**
   ```javascript
   // index.js - before
   const DATA_DIR = join(__dirname, 'data');

   // index.js - after
   const DATA_DIR = process.env.RUVECTOR_DATA_DIR ||
                    join(process.cwd(), '.ruvector', 'intelligence');
   ```

2. **Package Intelligence as NPM Module**
   - Move to `npm/packages/ruvector-intelligence/`
   - Export as standalone package
   - Include in `@ruvector/core` as dependency

3. **Environment Variable System**
   ```bash
   RUVECTOR_HOME=~/.ruvector        # Global learned patterns
   RUVECTOR_DATA_DIR=.ruvector      # Project-local data
   RUVECTOR_CLI_PATH=/path/to/cli   # Auto-detected
   ```

4. **Fallback Resolution Strategy**
   ```
   1. Project-local: ./.ruvector/intelligence/
   2. Global: ~/.ruvector/global/
   3. Embedded: node_modules/@ruvector/intelligence/
   ```

**Deliverables**:
- [ ] Refactored `index.js` with dynamic paths
- [ ] NPM package `@ruvector/intelligence`
- [ ] Environment variable documentation
- [ ] Tests for path resolution fallback chain

**Success Criteria**:
- Intelligence layer works without hardcoded paths
- Can run in any project directory
- Gracefully falls back to global patterns if local unavailable

---

### Milestone 4: Memory Migration System
**SPARC Phase**: Refinement → Completion
**Goal**: Migrate claude-flow SQLite memory.db to ruvector's HNSW VectorDB

#### Actions:
1. **Implement SQLite → VectorDB Converter**
   ```rust
   // In crates/ruvector-cli/src/cli/hooks/migrate.rs
   pub fn migrate_memory_db(sqlite_path: &Path, output_dir: &Path) -> Result<()> {
       // 1. Read SQLite memory.db
       let conn = Connection::open(sqlite_path)?;

       // 2. Extract trajectories, Q-table, memories
       let trajectories = extract_trajectories(&conn)?;
       let q_table = extract_q_learning_data(&conn)?;

       // 3. Convert to vector embeddings
       let embeddings = convert_to_embeddings(&trajectories)?;

       // 4. Store in ruvector VectorDB
       let db = VectorDB::new(/* config */)?;
       db.insert_batch(embeddings)?;

       // 5. Export Q-table as JSON
       export_q_table(&q_table, output_dir)?;

       Ok(())
   }
   ```

2. **Design Embedding Conversion Strategy**
   - Trajectory text → vector embedding (reuse `textToEmbedding()`)
   - Preserve Q-values and metadata
   - Map state-action pairs to searchable vectors

3. **Preserve Learning Integrity**
   - Validate Q-table checksums
   - Ensure all trajectories migrated
   - Verify searchable recall accuracy

**Deliverables**:
- [ ] `ruvector hooks migrate` command implementation
- [ ] SQLite schema parser
- [ ] Embedding conversion engine
- [ ] Migration validation tests
- [ ] Rollback/recovery mechanism

**Success Criteria**:
- 100% of memory.db data successfully migrated
- Q-learning patterns preserved accurately
- Vector search recalls migrated memories correctly
- Migration completes in <5 seconds for typical datasets

---

### Milestone 5: Hook Template System
**SPARC Phase**: Completion
**Goal**: Generate dynamic, portable hook configurations for `.claude/settings.json`

#### Actions:
1. **Create Hook Templates with Placeholders**
   ```json
   {
     "hooks": {
       "PreToolUse": [{
         "matcher": "Bash",
         "hooks": [{
           "command": "/bin/bash -c 'INPUT=$(cat); CMD=$(echo \"$INPUT\" | jq -r \".tool_input.command // empty\"); {{RUVECTOR_CLI_PATH}} hooks pre-command \"$CMD\" 2>/dev/null'"
         }]
       }]
     }
   }
   ```

2. **Variable Substitution Engine**
   ```rust
   fn substitute_template_vars(template: &str) -> Result<String> {
       let vars = HashMap::from([
           ("RUVECTOR_CLI_PATH", get_cli_path()?),
           ("PROJECT_ROOT", env::current_dir()?.to_str().unwrap()),
           ("RUVECTOR_HOME", get_ruvector_home()?),
       ]);

       let mut output = template.to_string();
       for (key, value) in vars {
           output = output.replace(&format!("{{{{{}}}}}", key), value);
       }
       Ok(output)
   }
   ```

3. **Install Command Implementation**
   ```rust
   pub fn install_hooks() -> Result<()> {
       // 1. Load template from embedded resource
       let template = include_str!("../templates/hooks.json");

       // 2. Substitute variables
       let config = substitute_template_vars(template)?;

       // 3. Merge with existing .claude/settings.json
       let settings_path = Path::new(".claude/settings.json");
       merge_or_create_settings(settings_path, &config)?;

       println!("✅ Hooks installed to .claude/settings.json");
       Ok(())
   }
   ```

**Deliverables**:
- [ ] Hook template files (JSON)
- [ ] Variable substitution engine
- [ ] `ruvector hooks install` implementation
- [ ] Merge strategy for existing settings
- [ ] Tests for template rendering

**Success Criteria**:
- Generated hooks work on first run without modification
- No hardcoded paths in output
- Merges safely with existing Claude Code settings
- Preserves user customizations

---

### Milestone 6: Global Learning Patterns
**SPARC Phase**: Completion
**Goal**: Support cross-project learning via `~/.ruvector/global/`

#### Actions:
1. **Global Pattern Storage**
   ```
   ~/.ruvector/
   ├── global/
   │   ├── patterns.json          # Shared Q-learning patterns
   │   ├── memory.rvdb            # Global vector memory
   │   ├── error-patterns.json    # Cross-project error fixes
   │   └── sequences.json         # File sequence patterns
   └── config.toml                # Global settings
   ```

2. **Pattern Synchronization**
   - Merge project-local + global patterns on load
   - Update global patterns when local patterns succeed
   - Privacy controls: opt-in/opt-out for global sharing

3. **Import/Export Commands**
   ```bash
   # Export project patterns to share with team
   npx ruvector hooks export --output team-patterns.json

   # Import patterns from another project
   npx ruvector hooks import --input team-patterns.json --merge
   ```

**Deliverables**:
- [ ] Global pattern storage system
- [ ] Pattern merge algorithm
- [ ] `ruvector hooks export` command
- [ ] `ruvector hooks import` command
- [ ] Privacy controls configuration

**Success Criteria**:
- Patterns learned in one project help in another
- No data leakage between unrelated projects
- Export/import preserves all learning data
- Team can share learned patterns via JSON

---

### Milestone 7: Integration Testing & Documentation
**SPARC Phase**: Completion
**Goal**: Ensure system works end-to-end in real-world scenarios

#### Actions:
1. **Integration Test Suite**
   ```bash
   # Test 1: Fresh project setup
   mkdir test-project && cd test-project
   npx ruvector hooks init
   npx ruvector hooks install
   # Verify .claude/settings.json generated correctly

   # Test 2: Migration from claude-flow
   npx ruvector hooks migrate --from ~/.swarm/memory.db
   npx ruvector hooks stats
   # Verify patterns imported

   # Test 3: Cross-platform compatibility
   # Run on Linux, macOS, Windows
   ```

2. **Documentation**
   - User guide: Setting up hooks in a new project
   - Migration guide: Moving from claude-flow
   - API reference: CLI command documentation
   - Troubleshooting: Common issues and solutions

3. **Examples and Templates**
   - Example `.ruvector/config.toml` configurations
   - Sample hook customizations
   - Team sharing workflow examples

**Deliverables**:
- [ ] Integration test suite (Rust + Bash scripts)
- [ ] User documentation (`docs/hooks/USER_GUIDE.md`)
- [ ] Migration guide (`docs/hooks/MIGRATION.md`)
- [ ] API reference (`docs/hooks/CLI_REFERENCE.md`)
- [ ] Example configurations

**Success Criteria**:
- All integration tests pass on Linux, macOS, Windows
- Documentation enables first-time user to set up hooks in <5 minutes
- Migration guide successfully migrates all test cases
- No hardcoded paths in final system

---

## 3. FILE STRUCTURE

### 3.1 New Directory Layout

```
ruvector/
├── crates/ruvector-cli/
│   ├── src/
│   │   ├── cli/
│   │   │   ├── commands.rs           # Add HooksCommands enum
│   │   │   ├── hooks/                # NEW: Hooks CLI module
│   │   │   │   ├── mod.rs
│   │   │   │   ├── init.rs           # `hooks init` implementation
│   │   │   │   ├── install.rs        # `hooks install` implementation
│   │   │   │   ├── migrate.rs        # `hooks migrate` implementation
│   │   │   │   ├── stats.rs          # `hooks stats` implementation
│   │   │   │   ├── export.rs         # `hooks export` implementation
│   │   │   │   └── import.rs         # `hooks import` implementation
│   │   │   └── ...
│   │   └── main.rs
│   ├── templates/                    # NEW: Hook templates
│   │   ├── hooks.json                # Portable hooks template
│   │   ├── config.toml.template      # Project config template
│   │   └── settings.json.template    # Claude settings template
│   └── Cargo.toml
│
├── npm/packages/
│   ├── ruvector-intelligence/        # NEW: Portable intelligence package
│   │   ├── src/
│   │   │   ├── index.js              # Refactored with dynamic paths
│   │   │   ├── cli.js                # Refactored CLI
│   │   │   ├── memory.js             # VectorMemory class
│   │   │   ├── reasoning.js          # ReasoningBank class
│   │   │   └── ...
│   │   ├── templates/                # Hook integration templates
│   │   │   └── hooks.json
│   │   ├── package.json
│   │   └── README.md
│   │
│   └── core/                         # Existing @ruvector/core
│       └── package.json              # Add intelligence as dependency
│
├── docs/hooks/
│   ├── IMPLEMENTATION_PLAN.md        # This document
│   ├── USER_GUIDE.md                 # NEW: User-facing guide
│   ├── MIGRATION.md                  # NEW: Migration from claude-flow
│   ├── CLI_REFERENCE.md              # NEW: CLI command reference
│   └── ARCHITECTURE.md               # NEW: Technical architecture
│
└── .ruvector/                        # NEW: Project-local state (gitignored)
    ├── config.toml                   # Project-specific settings
    ├── intelligence/
    │   ├── data/
    │   │   ├── memory.json
    │   │   ├── trajectories.json
    │   │   ├── patterns.json
    │   │   └── ...
    │   └── memory.rvdb               # Ruvector VectorDB storage
    └── .gitignore
```

### 3.2 Global State Directory

```
~/.ruvector/                          # Global user state
├── global/
│   ├── patterns.json                 # Cross-project patterns
│   ├── memory.rvdb                   # Global vector memory
│   ├── error-patterns.json           # Error fixes
│   └── sequences.json                # File sequences
├── config.toml                       # Global configuration
└── cache/                            # CLI cache
    └── cli-path.txt
```

---

## 4. CLI API DESIGN

### 4.1 Command Reference

#### `ruvector hooks init [OPTIONS]`
Initialize hooks system in the current project.

**Options**:
- `--path <PATH>`: Custom `.ruvector` directory location (default: `./.ruvector`)
- `--global`: Initialize global patterns directory
- `--template <NAME>`: Use a predefined template (default, minimal, advanced)

**Behavior**:
1. Create `.ruvector/` directory structure
2. Generate `config.toml` with defaults
3. Initialize empty data files
4. Output next steps (run `hooks install`)

**Example**:
```bash
npx ruvector hooks init
# Output:
# ✅ Initialized ruvector hooks in ./.ruvector
# 📁 Created: .ruvector/intelligence/data/
# ⏭  Next: Run `npx ruvector hooks install` to add hooks to Claude Code
```

---

#### `ruvector hooks install [OPTIONS]`
Install hooks into `.claude/settings.json`.

**Options**:
- `--force`: Overwrite existing hooks
- `--dry-run`: Show what would be written without modifying files
- `--template <PATH>`: Use custom hook template

**Behavior**:
1. Load hook template from `templates/hooks.json`
2. Substitute variables (`{{RUVECTOR_CLI_PATH}}`, etc.)
3. Merge with existing `.claude/settings.json` or create new
4. Validate JSON syntax
5. Backup original settings to `.claude/settings.json.backup`

**Example**:
```bash
npx ruvector hooks install
# Output:
# ✅ Hooks installed to .claude/settings.json
# 📋 Backup created: .claude/settings.json.backup
# 🧠 Intelligence layer ready
```

---

#### `ruvector hooks migrate --from <PATH> [OPTIONS]`
Migrate learning data from claude-flow or other sources.

**Options**:
- `--from <PATH>`: Source database path (SQLite or JSON)
- `--format <FORMAT>`: Source format (sqlite, json, csv)
- `--merge`: Merge with existing patterns instead of replacing
- `--validate`: Validate migration integrity

**Behavior**:
1. Detect source format (SQLite, JSON, or CSV)
2. Parse trajectories, Q-table, and memories
3. Convert to ruvector format (vector embeddings + JSON patterns)
4. Store in `.ruvector/intelligence/`
5. Validate migration (checksum, count verification)
6. Report statistics

**Example**:
```bash
npx ruvector hooks migrate --from ~/.swarm/memory.db --validate
# Output:
# 📊 Migrating from SQLite database...
# ✅ Imported 1,247 trajectories
# ✅ Imported 89 Q-learning patterns
# ✅ Converted 543 memories to vectors
# 🔍 Validation passed (100% integrity)
# ⏱  Completed in 3.2s
```

---

#### `ruvector hooks stats [OPTIONS]`
Display learning statistics and system health.

**Options**:
- `--verbose`: Show detailed breakdown
- `--json`: Output as JSON for programmatic use
- `--compare-global`: Compare local vs global patterns

**Behavior**:
1. Load local patterns from `.ruvector/intelligence/`
2. Calculate statistics (pattern count, memory size, etc.)
3. Display formatted output
4. Optional: Compare with global patterns

**Example**:
```bash
npx ruvector hooks stats --verbose
# Output:
# 🧠 RuVector Intelligence Statistics
#
# 📊 Learning Data:
#    Trajectories: 1,247
#    Patterns: 89 (Q-learning states)
#    Memories: 543 vectors
#    Total size: 2.4 MB
#
# 🎯 Top Patterns:
#    1. edit_rs_in_ruvector-core → successful-edit (Q=0.823)
#    2. cargo_test → command-succeeded (Q=0.791)
#    3. npm_build → command-succeeded (Q=0.654)
#
# 🔥 Recent Activity:
#    Last trajectory: 2 hours ago
#    A/B test group: treatment
#    Calibration error: 0.042
```

---

#### `ruvector hooks export --output <PATH> [OPTIONS]`
Export learned patterns for sharing or backup.

**Options**:
- `--output <PATH>`: Output file path
- `--format <FORMAT>`: Export format (json, csv, sqlite)
- `--include <TYPES>`: What to include (patterns, memories, all)
- `--compress`: Compress output with gzip

**Behavior**:
1. Read patterns from `.ruvector/intelligence/`
2. Serialize to specified format
3. Optional: Compress with gzip
4. Write to output file
5. Generate checksum for integrity

**Example**:
```bash
npx ruvector hooks export --output team-patterns.json --include patterns
# Output:
# ✅ Exported 89 patterns to team-patterns.json
# 📦 Size: 45.2 KB
# 🔐 SHA256: 8f3b4c2a...
```

---

#### `ruvector hooks import --input <PATH> [OPTIONS]`
Import learned patterns from another project or team member.

**Options**:
- `--input <PATH>`: Input file path
- `--merge`: Merge with existing patterns (default: replace)
- `--strategy <STRATEGY>`: Merge strategy (prefer-local, prefer-imported, average)
- `--validate`: Validate before importing

**Behavior**:
1. Read input file
2. Parse and validate patterns
3. Merge with existing patterns (if `--merge`)
4. Write to `.ruvector/intelligence/`
5. Report import statistics

**Example**:
```bash
npx ruvector hooks import --input team-patterns.json --merge --strategy average
# Output:
# 📥 Importing patterns...
# ✅ Imported 89 patterns
# 🔀 Merged with 67 existing patterns
# 📊 New total: 123 patterns (33 updated, 56 unchanged)
```

---

#### `ruvector hooks enable` / `ruvector hooks disable`
Enable or disable hooks system.

**Behavior**:
- `enable`: Set `RUVECTOR_INTELLIGENCE_ENABLED=true` in config
- `disable`: Set `RUVECTOR_INTELLIGENCE_ENABLED=false` in config

**Example**:
```bash
npx ruvector hooks disable
# Output:
# ⏸  Hooks disabled (set RUVECTOR_INTELLIGENCE_ENABLED=false)
# 💡 Re-enable with: npx ruvector hooks enable
```

---

### 4.2 Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUVECTOR_HOME` | `~/.ruvector` | Global patterns directory |
| `RUVECTOR_DATA_DIR` | `./.ruvector` | Project-local data directory |
| `RUVECTOR_CLI_PATH` | Auto-detected | Path to ruvector CLI binary |
| `RUVECTOR_INTELLIGENCE_ENABLED` | `true` | Enable/disable intelligence layer |
| `RUVECTOR_LEARNING_RATE` | `0.1` | Q-learning alpha parameter |
| `INTELLIGENCE_MODE` | `treatment` | A/B test group (treatment, control) |

---

### 4.3 Configuration File Schema

#### `.ruvector/config.toml`

```toml
[intelligence]
enabled = true
learning_rate = 0.1
ab_test_group = "treatment"  # or "control"
use_hyperbolic_distance = true
curvature = 1.0

[memory]
backend = "rvdb"  # or "json" for fallback
max_memories = 50000
dimensions = 128

[patterns]
decay_half_life_days = 7
min_q_value = -0.5
max_q_value = 0.8

[global]
sync_enabled = true  # Sync with ~/.ruvector/global/
sync_interval_hours = 24
privacy_mode = "opt-in"  # or "opt-out"

[hooks]
pre_command_enabled = true
post_command_enabled = true
pre_edit_enabled = true
post_edit_enabled = true
pre_compact_enabled = true
session_start_enabled = true
session_end_enabled = true
```

---

## 5. MIGRATION STRATEGY

### 5.1 Existing User Migration Path

**For users with `.claude/intelligence/` (current repo-specific system)**:

1. **Backup existing data**:
   ```bash
   cp -r .claude/intelligence .claude/intelligence.backup
   ```

2. **Initialize new system**:
   ```bash
   npx ruvector hooks init
   ```

3. **Migrate data**:
   ```bash
   # Intelligence layer data (JSON files)
   npx ruvector hooks migrate --from .claude/intelligence --format json

   # Claude-flow memory.db (if exists)
   npx ruvector hooks migrate --from ~/.swarm/memory.db --merge
   ```

4. **Update hooks**:
   ```bash
   npx ruvector hooks install --force
   ```

5. **Verify**:
   ```bash
   npx ruvector hooks stats
   ```

6. **Clean up**:
   ```bash
   # Optional: Remove old intelligence directory
   rm -rf .claude/intelligence
   ```

---

### 5.2 Claude-Flow Memory.db Migration

**SQLite Schema** (inferred from claude-flow):
```sql
CREATE TABLE trajectories (
    id TEXT PRIMARY KEY,
    state TEXT,
    action TEXT,
    outcome TEXT,
    reward REAL,
    timestamp TEXT,
    ab_group TEXT
);

CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    type TEXT,
    content TEXT,
    embedding BLOB,  -- Serialized float array
    metadata TEXT    -- JSON
);

CREATE TABLE q_table (
    state TEXT,
    action TEXT,
    q_value REAL,
    update_count INTEGER,
    last_update TEXT,
    PRIMARY KEY (state, action)
);
```

**Migration Algorithm**:
```rust
pub fn migrate_memory_db(sqlite_path: &Path, output_dir: &Path) -> Result<MigrationStats> {
    let conn = Connection::open(sqlite_path)?;
    let mut stats = MigrationStats::default();

    // 1. Migrate trajectories
    let mut stmt = conn.prepare("SELECT * FROM trajectories")?;
    let trajectories: Vec<Trajectory> = stmt.query_map([], |row| {
        Ok(Trajectory {
            id: row.get(0)?,
            state: row.get(1)?,
            action: row.get(2)?,
            outcome: row.get(3)?,
            reward: row.get(4)?,
            timestamp: row.get(5)?,
            ab_group: row.get(6)?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    fs::write(
        output_dir.join("trajectories.json"),
        serde_json::to_string_pretty(&trajectories)?
    )?;
    stats.trajectories = trajectories.len();

    // 2. Migrate Q-table
    let mut stmt = conn.prepare("SELECT * FROM q_table")?;
    let q_table: HashMap<String, HashMap<String, f64>> = /* ... */;
    fs::write(
        output_dir.join("patterns.json"),
        serde_json::to_string_pretty(&q_table)?
    )?;
    stats.patterns = q_table.len();

    // 3. Migrate memories to VectorDB
    let mut stmt = conn.prepare("SELECT * FROM memories")?;
    let db = VectorDB::new(/* ... */)?;

    let memories = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let type_: String = row.get(1)?;
        let content: String = row.get(2)?;
        let embedding: Vec<u8> = row.get(3)?;
        let metadata: String = row.get(4)?;

        // Deserialize embedding blob
        let embedding_f32 = deserialize_embedding(&embedding)?;

        Ok(VectorEntry {
            id: Some(id),
            vector: embedding_f32,
            metadata: Some(serde_json::from_str(&metadata)?),
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    db.insert_batch(memories)?;
    stats.memories = memories.len();

    // 4. Validation
    validate_migration(&stats, &conn, &db)?;

    Ok(stats)
}
```

---

### 5.3 Backwards Compatibility

**Support Matrix**:
| Feature | Legacy (repo-specific) | New (portable) | Notes |
|---------|------------------------|----------------|-------|
| JSON data files | ✅ Supported | ✅ Supported | Automatic migration |
| Hardcoded paths | ✅ Still works | ❌ Replaced | Use `hooks install` to update |
| SQLite memory.db | ❌ Not supported | ✅ Via migration | One-time migration required |
| Global patterns | ❌ Not available | ✅ Supported | New feature |
| CLI management | ❌ Manual editing | ✅ Full CLI | Recommended upgrade path |

---

## 6. RISK ASSESSMENT & MITIGATION

### 6.1 Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Path resolution fails on Windows | Medium | High | Use `shellexpand` crate, conditional shell (`cmd` vs `bash`), test on PowerShell |
| Migration loses learning data | Low | Critical | Atomic migration with automatic backup/rollback, checksums, validation |
| Performance regression | Medium | Medium | Benchmark before/after, optimize hot paths |
| Breaking changes for existing users | High | Medium | Migration guide, backwards compatibility layer, keep `.claude/intelligence` working |
| Hook template bugs | Medium | High | Integration tests, `--dry-run` mode, type-safe templates with `askama` |
| Command injection in hooks | Low | Critical | Escape all shell arguments with `shell-escape` crate |
| SQLite format incompatibility | High | High | **MVP: JSON migration only**, defer SQLite to v1.1 with format detection |

### 6.2 User Experience Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Complex migration process | Medium | High | Automated migration scripts, clear documentation |
| Configuration complexity | Medium | Medium | Sensible defaults, templates, examples |
| Unclear error messages | High | Medium | User-friendly error messages, troubleshooting guide |
| Lost productivity during transition | Medium | High | Gradual rollout, backwards compatibility |

---

## 7. SUCCESS METRICS

### 7.1 Technical Metrics

- **Portability**: ✅ Works on 3+ operating systems (Linux, macOS, Windows)
- **Performance**: ✅ Migration completes in <10 seconds for 10k trajectories
- **Reliability**: ✅ 100% data integrity in migration (validated via checksums)
- **Compatibility**: ✅ Backwards compatible with existing JSON data files

### 7.2 User Metrics

- **Setup Time**: ✅ New user can set up hooks in <5 minutes
- **Migration Success**: ✅ 95%+ of users successfully migrate without assistance
- **Adoption**: ✅ 80%+ of active users upgrade within 1 month
- **Satisfaction**: ✅ Positive feedback on portability and ease of use

### 7.3 Code Quality Metrics

- **Test Coverage**: ✅ >80% coverage for CLI commands and migration logic
- **Documentation**: ✅ All CLI commands documented with examples
- **Code Review**: ✅ All code reviewed and approved
- **No Regressions**: ✅ All existing functionality preserved

---

## 8. TIMELINE ESTIMATES

| Milestone | Estimated Time | Dependencies | MVP |
|-----------|----------------|--------------|-----|
| 1. Specification & Architecture | 2-3 days | None | ✅ |
| 2+5. CLI + Template System (Combined) | 4-5 days | Milestone 1 | ✅ |
| 3. Intelligence Layer Portability | 3-4 days | Milestone 1 | ✅ |
| 4a. JSON Migration (MVP) | 2 days | Milestone 3 | ✅ |
| 4b. SQLite Migration (v1.1) | 4-5 days | Milestone 4a | ❌ Deferred |
| 6. Global Patterns (v1.1) | 4-5 days | Milestone 3, 4 | ❌ Deferred |
| 7. Integration Testing & Documentation | 4-5 days | Milestones 1-4a | ✅ |
| **MVP Total** | **15-19 days** | (~3-4 weeks) | |
| **Full Release (v1.1+)** | **27-38 days** | (~5-7 weeks) | |

---

## 9. NEXT STEPS

### Immediate Actions (Week 1)
1. **Review and approve this implementation plan**
2. **Set up development branch**: `feature/portable-hooks-system`
3. **Create initial file structure**: directories, templates, module stubs
4. **Write specification documents**: CLI API, hook template format
5. **Design test cases**: integration tests, migration scenarios

### Phase 1 (Weeks 2-3): Foundation
- Implement CLI command structure (`HooksCommands` enum)
- Create hook template engine with variable substitution
- Implement `ruvector hooks init` and `ruvector hooks install`
- Write unit tests for template generation

### Phase 2 (Weeks 4-5): Intelligence & Migration
- Refactor intelligence layer for dynamic paths
- Package as `@ruvector/intelligence` npm module
- Implement SQLite → VectorDB migration
- Implement `ruvector hooks migrate`

### Phase 3 (Weeks 6-7): Advanced Features
- Implement global patterns system
- Add `ruvector hooks export/import`
- Create integration tests
- Write comprehensive documentation

### Phase 4 (Week 8): Polish & Release
- Cross-platform testing (Linux, macOS, Windows)
- User acceptance testing
- Documentation review
- Release candidate and final release

---

## 10. APPENDICES

### Appendix A: Example Hook Template

```json
{
  "env": {
    "RUVECTOR_INTELLIGENCE_ENABLED": "true",
    "RUVECTOR_LEARNING_RATE": "0.1",
    "INTELLIGENCE_MODE": "treatment"
  },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "timeout": 3000,
            "command": "/bin/bash -c 'INPUT=$(cat); CMD=$(echo \"$INPUT\" | jq -r \".tool_input.command // empty\"); {{RUVECTOR_CLI_PATH}} hooks pre-command \"$CMD\" 2>/dev/null'"
          }
        ]
      },
      {
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [
          {
            "type": "command",
            "timeout": 3000,
            "command": "/bin/bash -c 'INPUT=$(cat); FILE=$(echo \"$INPUT\" | jq -r \".tool_input.file_path // .tool_input.path // empty\"); if [ -n \"$FILE\" ]; then {{RUVECTOR_CLI_PATH}} hooks pre-edit \"$FILE\" 2>/dev/null; fi'"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "/bin/bash -c 'INPUT=$(cat); CMD=$(echo \"$INPUT\" | jq -r \".tool_input.command // empty\"); SUCCESS=\"true\"; STDERR=\"\"; if echo \"$INPUT\" | jq -e \".tool_result.stderr\" 2>/dev/null | grep -q .; then SUCCESS=\"false\"; STDERR=$(echo \"$INPUT\" | jq -r \".tool_result.stderr // empty\" | head -c 300); fi; ({{RUVECTOR_CLI_PATH}} hooks post-command \"$CMD\" \"$SUCCESS\" \"$STDERR\" 2>/dev/null) &'"
          }
        ]
      },
      {
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [
          {
            "type": "command",
            "command": "/bin/bash -c 'INPUT=$(cat); FILE=$(echo \"$INPUT\" | jq -r \".tool_input.file_path // .tool_input.path // empty\"); if [ -n \"$FILE\" ]; then ({{RUVECTOR_CLI_PATH}} hooks post-edit \"$FILE\" \"true\" 2>/dev/null) & fi'"
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "timeout": 5000,
            "command": "{{RUVECTOR_CLI_PATH}} hooks session-start"
          }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "{{RUVECTOR_CLI_PATH}} hooks session-end"
          }
        ]
      }
    ]
  }
}
```

### Appendix B: Migration Validation Tests

```rust
#[cfg(test)]
mod migration_tests {
    use super::*;

    #[test]
    fn test_sqlite_to_json_migration() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sqlite_path = create_test_sqlite_db(&temp_dir);

        let stats = migrate_memory_db(&sqlite_path, temp_dir.path()).unwrap();

        assert_eq!(stats.trajectories, 100);
        assert_eq!(stats.patterns, 25);
        assert_eq!(stats.memories, 50);
    }

    #[test]
    fn test_embedding_preservation() {
        let original_embedding = vec![0.1, 0.2, 0.3, 0.4];
        let serialized = serialize_embedding(&original_embedding);
        let deserialized = deserialize_embedding(&serialized).unwrap();

        assert_eq!(original_embedding, deserialized);
    }

    #[test]
    fn test_q_value_accuracy() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sqlite_path = create_test_sqlite_db(&temp_dir);

        migrate_memory_db(&sqlite_path, temp_dir.path()).unwrap();

        let patterns: HashMap<String, HashMap<String, f64>> =
            serde_json::from_str(&fs::read_to_string(
                temp_dir.path().join("patterns.json")
            ).unwrap()).unwrap();

        // Verify Q-values preserved
        assert!((patterns["test_state"]["test_action"] - 0.823).abs() < 0.001);
    }
}
```

### Appendix C: Cross-Platform Path Resolution

```rust
use shellexpand;
use std::env;

fn get_ruvector_home() -> Result<PathBuf> {
    if let Ok(home) = env::var("RUVECTOR_HOME") {
        return Ok(PathBuf::from(shellexpand::tilde(&home).to_string()));
    }

    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow!("Could not determine home directory"))?;

    Ok(home_dir.join(".ruvector"))
}

fn get_cli_path() -> Result<String> {
    // 1. Check if already in PATH
    if let Ok(path) = which::which("ruvector") {
        return Ok(path.display().to_string());
    }

    // 2. Check if running via npx
    if let Ok(npm_execpath) = env::var("npm_execpath") {
        if npm_execpath.contains("npx") {
            return Ok("npx ruvector".to_string());
        }
    }

    // 3. Fallback to current executable
    env::current_exe()
        .map(|p| p.display().to_string())
        .map_err(|e| anyhow!("Could not determine CLI path: {}", e))
}
```

---

## 11. CRITICAL FIXES REQUIRED

### 11.1 Windows Shell Compatibility

**Add to Milestone 5** (Hook Template System):

```rust
// In crates/ruvector-cli/src/cli/hooks/install.rs

fn get_shell_wrapper() -> &'static str {
    if cfg!(target_os = "windows") {
        "cmd /c"
    } else {
        "/bin/bash -c"
    }
}

fn render_hook_template(template: &str) -> Result<String> {
    let vars = HashMap::from([
        ("SHELL", get_shell_wrapper()),
        ("RUVECTOR_CLI", "which ruvector || echo npx ruvector"),
        // Runtime resolution instead of install-time
    ]);
    // ...
}
```

### 11.2 Atomic Migration with Rollback

**Add to Milestone 4a** (JSON Migration):

```rust
// In crates/ruvector-cli/src/cli/hooks/migrate.rs

pub fn migrate_with_safety(from: &Path, to: &Path) -> Result<MigrationStats> {
    let backup_dir = to.with_extension("backup");
    let temp_dir = to.with_extension("tmp");

    // Step 1: Backup existing data
    if to.exists() {
        fs::rename(to, &backup_dir)?;
    }

    // Step 2: Migrate to temporary location
    fs::create_dir_all(&temp_dir)?;
    let stats = match do_migration(from, &temp_dir) {
        Ok(s) => s,
        Err(e) => {
            // Restore backup on failure
            fs::remove_dir_all(&temp_dir)?;
            if backup_dir.exists() {
                fs::rename(&backup_dir, to)?;
            }
            return Err(e);
        }
    };

    // Step 3: Validate migrated data
    validate_migration(&temp_dir, &stats)?;

    // Step 4: Atomic swap
    fs::rename(&temp_dir, to)?;
    fs::remove_dir_all(&backup_dir)?;

    Ok(stats)
}
```

### 11.3 Command Injection Prevention

**Add to all hook templates**:

```rust
// Add to Cargo.toml:
// shell-escape = "0.1"

use shell_escape::escape;

fn generate_hook_command(file_path: &str) -> String {
    let escaped = escape(file_path.into());
    format!(
        r#"/bin/bash -c 'FILE={}; ruvector hooks pre-edit "$FILE"'"#,
        escaped
    )
}
```

---

## 12. RECOMMENDED DEPENDENCY ADDITIONS

Add to `crates/ruvector-cli/Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...

# For SQLite migration (v1.1)
rusqlite = { version = "0.32", optional = true }

# For type-safe templates
askama = "0.12"

# For shell argument escaping
shell-escape = "0.1"

# Already present (good!):
# shellexpand = "3.1"  ✅

[features]
default = []
sqlite-migration = ["rusqlite"]
```

---

## CONCLUSION

This implementation plan provides a comprehensive roadmap for transforming the ruvector hooks system from a repository-specific solution into a **generic, portable, CLI-integrated intelligence layer** that can benefit any project using Claude Code.

**Key Achievements (MVP - 3-4 weeks)**:
- ✅ Zero hardcoded paths
- ✅ Works in any project with `npx ruvector hooks init`
- ✅ Migrates existing JSON learning data automatically
- ✅ Full CLI management of hooks system
- ✅ Backwards compatible with existing setups
- ✅ Cross-platform (Linux, macOS, Windows)
- ✅ Atomic migration with rollback protection
- ✅ Security: Command injection prevention

**Deferred to v1.1 (Additional 2-3 weeks)**:
- ⏭️ SQLite migration (complex, needs format detection)
- ⏭️ Global cross-project learning patterns
- ⏭️ Export/import team sharing

**Estimated Effort**:
- **MVP**: 3-4 weeks (15-19 days)
- **Full v1.1**: 5-7 weeks (27-38 days)

**Risk Level**: Low-Medium (major risks mitigated in sections 11.1-11.3)
**Expected Impact**: High (enables widespread adoption of intelligent hooks)

**Next Step**: Approve this plan and proceed to Milestone 1 (Specification & Architecture Design).

---

*Document Version*: 2.0 (Post-Review)
*Last Updated*: 2025-12-25
*Status*: Reviewed - Ready for Implementation
*Reviewer Notes*: Optimized timeline by 50%, addressed Windows compatibility, added security fixes
