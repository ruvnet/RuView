# Hooks System MVP - Implementation Checklist

> **Related Documentation**: [README](README.md) | [Implementation Plan](IMPLEMENTATION_PLAN.md) | [Architecture](ARCHITECTURE.md)

**Target**: 3-4 weeks | **Status**: Ready for Development
**Feature Branch**: `feature/portable-hooks-mvp`

---

## Week 1: Foundation & CLI Scaffolding

### Day 1-2: Project Setup
- [ ] Create feature branch `feature/portable-hooks-mvp`
- [ ] Update `crates/ruvector-cli/Cargo.toml`:
  ```toml
  [dependencies]
  askama = "0.12"
  shell-escape = "0.1"
  ```
- [ ] Create directory structure:
  ```
  crates/ruvector-cli/
  ├── src/cli/hooks/
  │   ├── mod.rs
  │   ├── init.rs
  │   ├── install.rs
  │   ├── migrate.rs
  │   └── stats.rs
  └── templates/
      ├── hooks.json.j2
      ├── config.toml.j2
      └── gitignore.j2
  ```
- [ ] Write specification document (API contracts)
- [ ] Design template schema with variable placeholders

### Day 3-4: CLI Command Structure
- [ ] Add `Hooks` enum to `src/cli/commands.rs`:
  ```rust
  enum Commands {
      // ... existing commands
      Hooks {
          #[command(subcommand)]
          action: HooksCommands,
      },
  }

  enum HooksCommands {
      Init { path: Option<PathBuf> },
      Install { force: bool },
      Migrate { from: PathBuf },
      Stats { verbose: bool },
  }
  ```
- [ ] Implement command routing in `main.rs`
- [ ] Write unit tests for command parsing

### Day 5: Template Engine
- [ ] Create `HookTemplate` struct with `askama`:
  ```rust
  #[derive(Template)]
  #[template(path = "hooks.json.j2")]
  struct HookTemplate {
      shell: String,
      ruvector_cli: String,
      project_root: String,
  }
  ```
- [ ] Implement platform detection:
  ```rust
  fn get_shell_wrapper() -> &'static str {
      if cfg!(target_os = "windows") { "cmd /c" }
      else { "/bin/bash -c" }
  }
  ```
- [ ] Write template rendering tests

---

## Week 2: Core Commands Implementation

### Day 6-7: `ruvector hooks init`
- [ ] Implement `init.rs`:
  - [ ] Create `.ruvector/` directory
  - [ ] Generate `config.toml` from template
  - [ ] Create `intelligence/` subdirectories
  - [ ] Write `.gitignore`
- [ ] Add validation checks (prevent overwriting existing)
- [ ] Write integration test:
  ```rust
  #[test]
  fn test_init_creates_structure() {
      let temp = tempdir()?;
      run_cmd(&["ruvector", "hooks", "init"], &temp)?;
      assert!(temp.join(".ruvector/config.toml").exists());
  }
  ```

### Day 8-9: `ruvector hooks install`
- [ ] Implement `install.rs`:
  - [ ] Load hook template
  - [ ] Substitute variables with runtime values
  - [ ] Merge with existing `.claude/settings.json` or create new
  - [ ] Create backup (`.claude/settings.json.backup`)
- [ ] Add `--dry-run` flag
- [ ] Implement JSON validation
- [ ] Write tests for template substitution
- [ ] Write tests for merge logic

### Day 10: Hook Template Design
- [ ] Create `templates/hooks.json.j2`:
  ```json
  {
    "hooks": {
      "PreToolUse": [{
        "matcher": "Bash",
        "hooks": [{
          "command": "{{ shell }} 'RUVECTOR=$(which ruvector || echo npx ruvector); $RUVECTOR hooks pre-command \"$CMD\"'"
        }]
      }],
      "PostToolUse": [/* ... */],
      "SessionStart": [/* ... */]
    }
  }
  ```
- [ ] Test on Linux (bash)
- [ ] Test on macOS (bash)
- [ ] Test on Windows (cmd)

---

## Week 3: Intelligence Layer & Migration

### Day 11-12: Intelligence Layer Refactoring
- [ ] Update `.claude/intelligence/index.js`:
  - [ ] Change `DATA_DIR` to use `process.env.RUVECTOR_DATA_DIR`
  - [ ] Add fallback chain: env → project-local → global → legacy
  ```javascript
  const DATA_DIR = process.env.RUVECTOR_DATA_DIR ||
                   join(process.cwd(), '.ruvector', 'intelligence') ||
                   join(process.env.HOME || process.env.USERPROFILE, '.ruvector', 'global') ||
                   join(__dirname, 'data');  // Legacy fallback
  ```
- [ ] Update `cli.js` with same path logic
- [ ] Test intelligence layer works in new location
- [ ] Verify backward compatibility (legacy paths still work)

### Day 13-14: JSON Migration
- [ ] Implement `migrate.rs`:
  - [ ] Copy function with validation:
    ```rust
    fn copy_with_validation(from: &Path, to: &Path) -> Result<()> {
        let data = fs::read_to_string(from)?;
        let json: serde_json::Value = serde_json::from_str(&data)?;
        let mut file = File::create(to)?;
        serde_json::to_writer_pretty(&mut file, &json)?;
        file.sync_all()?;
        Ok(())
    }
    ```
  - [ ] Implement atomic migration with rollback:
    ```rust
    pub fn migrate_with_safety(from: &Path, to: &Path) -> Result<MigrationStats> {
        let backup = to.with_extension("backup");
        let temp = to.with_extension("tmp");

        // Backup → Migrate to temp → Validate → Atomic swap
    }
    ```
- [ ] Add checksum validation
- [ ] Write migration tests with sample data

### Day 15: `ruvector hooks stats`
- [ ] Implement `stats.rs`:
  - [ ] Load patterns from `.ruvector/intelligence/patterns.json`
  - [ ] Calculate statistics (count, Q-values, top patterns)
  - [ ] Format output (pretty-print or JSON)
- [ ] Add `--verbose` and `--json` flags
- [ ] Test with empty/populated data

---

## Week 4: Testing, Documentation & Polish

### Day 16-17: Cross-Platform Testing
- [ ] Set up GitHub Actions CI:
  ```yaml
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
  ```
- [ ] Linux tests:
  - [ ] Fresh project init
  - [ ] Migration from `.claude/intelligence/`
  - [ ] Hooks trigger correctly
- [ ] macOS tests (same as Linux)
- [ ] Windows tests:
  - [ ] PowerShell environment
  - [ ] CMD environment
  - [ ] WSL environment
  - [ ] Path separator handling (`\` vs `/`)

### Day 18: Error Handling & Edge Cases
- [ ] Implement graceful hook failures:
  ```rust
  pub fn execute_hook_safely(hook: &HookCommand) -> Result<()> {
      match hook.execute().timeout(Duration::from_secs(3)) {
          Ok(Ok(_)) => Ok(()),
          Ok(Err(e)) => {
              eprintln!("⚠️ Hook failed: {}", e);
              Ok(())  // Non-fatal
          }
          Err(_) => {
              eprintln!("⚠️ Hook timeout");
              Ok(())
          }
      }
  }
  ```
- [ ] Test with missing dependencies (`jq` not installed)
- [ ] Test with corrupted JSON files
- [ ] Test with permission errors

### Day 19: Documentation
- [ ] Write `docs/hooks/USER_GUIDE.md`:
  - [ ] Quick start (5-minute setup)
  - [ ] Migration guide
  - [ ] Troubleshooting
  - [ ] Configuration reference
- [ ] Write `docs/hooks/CLI_REFERENCE.md`:
  - [ ] Each command with examples
  - [ ] All flags documented
  - [ ] Return codes and errors
- [ ] Update `README.md` with hooks section
- [ ] Add examples to `examples/hooks/`

### Day 20: Final Polish
- [ ] Run `cargo clippy` and fix warnings
- [ ] Run `cargo fmt`
- [ ] Verify test coverage >80%:
  ```bash
  cargo tarpaulin --out Html
  ```
- [ ] Write release notes
- [ ] Tag version `v0.2.0-mvp`

---

## Post-MVP: v1.1 Planning (Future)

### SQLite Migration (v1.1.0)
- [ ] Add `rusqlite = { version = "0.32", optional = true }`
- [ ] Implement format detection:
  ```rust
  fn detect_embedding_format(blob: &[u8]) -> EmbeddingFormat {
      if blob.starts_with(b"\x93NUMPY") { Numpy }
      else if blob.len() % 4 == 0 { RawF32 }
      else { Unknown }
  }
  ```
- [ ] Write SQLite → rvlite converter
- [ ] Add `--format` flag to `hooks migrate`

### Global Patterns System (v1.1.0)
- [ ] Design sync protocol
- [ ] Implement pattern merge algorithm
- [ ] Add privacy controls (opt-in/opt-out)
- [ ] Implement `hooks export` and `hooks import`

---

## Testing Checklist

### Unit Tests
- [ ] Template rendering (all platforms)
- [ ] Path resolution (Windows vs Unix)
- [ ] Command parsing (clap)
- [ ] JSON validation
- [ ] Migration rollback logic

### Integration Tests
- [ ] End-to-end: init → install → test hook triggers
- [ ] Migration preserves data integrity
- [ ] Hooks work after binary reinstallation
- [ ] Backward compatibility with legacy paths

### Manual Testing
- [ ] Fresh Ubuntu 22.04 VM
- [ ] Fresh macOS 14 VM
- [ ] Fresh Windows 11 VM (PowerShell)
- [ ] Codespaces environment
- [ ] GitHub Codespaces with Claude Code

---

## Definition of Done

### MVP Release Criteria
- ✅ All tests pass on Linux, macOS, Windows
- ✅ Test coverage >80%
- ✅ Documentation complete (USER_GUIDE + CLI_REFERENCE)
- ✅ Zero clippy warnings
- ✅ Manual testing on 3 platforms
- ✅ Migration tested with sample data (no data loss)
- ✅ Hooks survive binary reinstallation
- ✅ First-time setup completes in <5 minutes

### Performance Targets
- ✅ `hooks init` completes in <1 second
- ✅ `hooks install` completes in <2 seconds
- ✅ `hooks migrate` processes 1000 trajectories in <5 seconds
- ✅ Hook execution adds <50ms overhead

---

## Known Limitations (Document in Release Notes)

1. **SQLite migration not supported in MVP** - JSON-only migration available, SQLite coming in v1.1
2. **Global patterns deferred to v1.1** - Project-local patterns only in MVP
3. **Windows requires `jq` binary** - Not auto-installed, manual setup required
4. **No `hooks export/import` in MVP** - Manual file copying as workaround

---

## Risk Mitigation Checklist

- [ ] ✅ Windows testing (PowerShell, CMD, WSL)
- [ ] ✅ Atomic migration with backup/rollback
- [ ] ✅ Command injection prevention (`shell-escape`)
- [ ] ✅ Runtime path resolution (no hardcoded paths)
- [ ] ✅ Graceful failure (hooks never block Claude Code)
- [ ] ✅ Backward compatibility (legacy paths still work)

---

## Dependencies Required

### MVP
```toml
askama = "0.12"           # Type-safe templates
shell-escape = "0.1"      # Security
```

### Already Available
```toml
shellexpand = "3.1"       # Path expansion ✅
clap = { ... }            # CLI framework ✅
tokio = { ... }           # Async runtime ✅
serde_json = { ... }      # JSON parsing ✅
```

### v1.1
```toml
rusqlite = { version = "0.32", optional = true }
```

---

## Success Metrics (Track During Development)

| Metric | Target | Actual |
|--------|--------|--------|
| Test coverage | >80% | ___ |
| Platforms passing | 3/3 | ___ |
| Migration speed (1000 traj) | <5s | ___ |
| First-time setup | <5min | ___ |
| Hook overhead | <50ms | ___ |
| Data loss during migration | 0% | ___ |

---

## Quick Commands Reference

```bash
# Create branch
git checkout -b feature/portable-hooks-mvp

# Run tests
cargo test --package ruvector-cli --lib hooks

# Run integration tests
cargo test --package ruvector-cli --test hooks_integration

# Check coverage
cargo tarpaulin --out Html --package ruvector-cli

# Lint
cargo clippy --package ruvector-cli -- -D warnings

# Format
cargo fmt --package ruvector-cli

# Build for all platforms
cargo build --release --package ruvector-cli

# Test CLI commands
cargo run --package ruvector-cli -- hooks init
cargo run --package ruvector-cli -- hooks install --dry-run
```

---

**Last Updated**: 2025-12-25
**Status**: Ready for Development
**Estimated Completion**: 3-4 weeks from start
