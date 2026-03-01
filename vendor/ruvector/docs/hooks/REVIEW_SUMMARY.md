# Hooks Implementation Plan - Code Review Summary

> **Related Documentation**: [README](README.md) | [Full Review](REVIEW_REPORT.md) | [Implementation Plan](IMPLEMENTATION_PLAN.md)

**Status**: ✅ APPROVED WITH CRITICAL FIXES
**Timeline**: Optimized from 6-8 weeks → **3-4 weeks for MVP**
**Risk Level**: Low-Medium (major risks mitigated)

---

## 1. Critical Issues Found (Must Fix)

### 🔴 Issue #1: Windows Compatibility Broken
**Impact**: Complete failure on Windows
**Fix**: Use conditional shell detection
```rust
fn get_shell_wrapper() -> &'static str {
    if cfg!(target_os = "windows") { "cmd /c" }
    else { "/bin/bash -c" }
}
```

### 🔴 Issue #2: SQLite Migration Undefined Format
**Impact**: Data loss risk
**Fix**: Defer SQLite to v1.1, use JSON-only migration for MVP

### 🔴 Issue #3: Path Resolution Breaks After Reinstall
**Impact**: Hooks stop working after binary moves
**Fix**: Use runtime resolution instead of install-time substitution
```bash
RUVECTOR=$(which ruvector || echo npx ruvector); $RUVECTOR hooks pre-edit
```

---

## 2. Optimizations Applied

| Change | Time Saved | Rationale |
|--------|------------|-----------|
| Defer global patterns to v1.1 | 4-5 days | Adds complexity without MVP value |
| JSON migration only (defer SQLite) | 3-5 days | Most users have JSON data |
| Combine CLI + Templates milestones | 4-6 days | Reduce context switching |
| **Total Savings** | **~50%** | MVP: 3-4 weeks vs 6-8 weeks |

---

## 3. Missing Elements Added

✅ **Error handling**: Hooks never block Claude Code operations
✅ **Atomic migration**: Backup → Migrate → Validate → Swap with rollback
✅ **Security**: Command injection prevention with `shell-escape`
✅ **Windows testing**: PowerShell, CMD, WSL compatibility checklist

---

## 4. Code Quality Improvements

### Type-Safe Templates
**Before**: String-based templates (error-prone)
**After**: `askama` crate with compile-time validation

### Idiomatic Rust
**Before**: `fs::write(path, json)?`
**After**: `serde_json::to_writer_pretty()` + `file.sync_all()`

### Configuration
**Before**: Magic numbers (timeout: 3000)
**After**: Extract to `config.toml` for user customization

---

## 5. Leveraging Existing Crates

✅ **Already Available** (no work needed):
- `shellexpand = "3.1"` - Path expansion
- `clap` - CLI framework
- `ruvector-core` - Vector storage
- `tokio` - Async runtime

➕ **Add for MVP**:
- `askama = "0.12"` - Type-safe templates
- `shell-escape = "0.1"` - Security

➕ **Add for v1.1**:
- `rusqlite = "0.32"` - SQLite migration

---

## 6. MVP Definition (3-4 Weeks)

### Week 1-2: Foundation
- `ruvector hooks init` - Create `.ruvector/` structure
- `ruvector hooks install` - Generate portable hooks
- Template engine with runtime path resolution
- JSON-to-JSON migration

### Week 3: Intelligence Layer
- Refactor `index.js` for dynamic paths
- Zero hardcoded paths
- Test in fresh project

### Week 4: Polish
- Cross-platform testing (Linux, macOS, Windows)
- `ruvector hooks stats` command
- Error handling + rollback
- Documentation

### Deferred to v1.1
❌ SQLite migration (needs format detection)
❌ Global patterns system (sync complexity)
❌ Export/import commands (nice-to-have)

---

## 7. Concrete Edits Made

### Updated IMPLEMENTATION_PLAN.md
1. **Timeline table** (Section 8): Added MVP vs Full Release split
2. **Risk assessment** (Section 6.1): Added 2 new risks with mitigations
3. **Critical fixes** (NEW Section 11): Windows compatibility, rollback, security
4. **Dependencies** (NEW Section 12): Specific Cargo.toml additions
5. **Conclusion**: Updated with MVP achievements and timeline

### Created REVIEW_REPORT.md
- 14-section detailed technical review
- Platform-specific testing checklist
- Integration test examples
- Documentation requirements
- Success metrics

---

## 8. Next Steps

### Immediate (Before Coding)
1. ✅ Review and approve this document
2. ✅ Add dependencies to `crates/ruvector-cli/Cargo.toml`
3. ✅ Create `crates/ruvector-cli/templates/` directory
4. ✅ Set up CI for Windows/macOS/Linux testing

### Week 1
1. Create feature branch: `feature/portable-hooks-mvp`
2. Implement Milestone 1 (Specification)
3. Start CLI scaffolding (Milestone 2)

### Week 2-4
Follow MVP implementation order (see Section 6)

---

## 9. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Windows edge cases | Comprehensive testing on PowerShell, CMD, WSL |
| Data loss | Atomic migration with backup/rollback |
| Command injection | `shell-escape` crate for all user inputs |
| Hooks break after reinstall | Runtime path resolution via `which` |
| SQLite format errors | Deferred to v1.1 with format detection |

---

## 10. Files Modified

### 1. IMPLEMENTATION_PLAN.md
**Changes**:
- Updated timeline (Section 8)
- Added critical fixes (Section 11)
- Added dependency recommendations (Section 12)
- Updated conclusion with MVP scope

**Lines Modified**: ~100 lines added/changed

### 2. REVIEW_REPORT.md (New)
**Purpose**: Detailed technical review with testing strategy

### 3. REVIEW_SUMMARY.md (This File)
**Purpose**: Executive summary for quick review

---

## 11. Recommended File Changes

### Cargo.toml
**File**: `/home/user/ruvector/crates/ruvector-cli/Cargo.toml`
```toml
[dependencies]
askama = "0.12"           # MVP
shell-escape = "0.1"      # MVP
rusqlite = { version = "0.32", optional = true }  # v1.1

[features]
sqlite-migration = ["rusqlite"]
```

### index.js
**File**: `/home/user/ruvector/.claude/intelligence/index.js`
**Line 20**: Change from:
```javascript
const DATA_DIR = join(__dirname, 'data');
```
To:
```javascript
const DATA_DIR = process.env.RUVECTOR_DATA_DIR ||
                 join(process.cwd(), '.ruvector', 'intelligence') ||
                 join(__dirname, 'data');  // Legacy fallback
```

---

## 12. Success Metrics

### Technical
- ✅ Works on 3 platforms (Linux, macOS, Windows)
- ✅ Migration <5s for 1000 trajectories
- ✅ 100% data integrity (checksums)
- ✅ Test coverage >80%

### User Experience
- ✅ First-time setup <5 minutes
- ✅ Zero hardcoded paths
- ✅ Hooks survive reinstallation
- ✅ Clear error messages

---

## 13. Final Verdict

**Approval**: ✅ **APPROVED FOR IMPLEMENTATION**

**Confidence**: 8/10

**Timeline**: 3-4 weeks for MVP (realistic and achievable)

**Remaining Risks**: Low (10-20% chance of minor delays)

**Overall Assessment**: Plan is solid, well-researched, and implementable. Critical issues identified and fixed. Timeline optimized by 50% while maintaining quality.

---

**Reviewed By**: Code Review Agent (ruvector)
**Review Date**: 2025-12-25
**Plan Version**: v2.0 (Post-Review)
**Next Step**: Approve and begin implementation

---

## Appendix: Quick Reference

### Commands Being Added
```bash
npx ruvector hooks init          # Initialize .ruvector/
npx ruvector hooks install       # Generate Claude Code hooks
npx ruvector hooks migrate --from .claude/intelligence  # Migrate data
npx ruvector hooks stats         # Show learning statistics
```

### File Structure (MVP)
```
.ruvector/
├── config.toml                  # Project settings
├── intelligence/
│   ├── trajectories.json        # Learning data
│   ├── patterns.json            # Q-learning patterns
│   └── memory.rvdb              # Vector memory (rvlite)
└── .gitignore
```

### Dependencies Added
- `askama` - Type-safe templates
- `shell-escape` - Security
- `rusqlite` (v1.1) - SQLite migration

### Existing Dependencies Leveraged
- `shellexpand` - Path resolution ✅
- `clap` - CLI framework ✅
- `ruvector-core` - Vector storage ✅
- `tokio` - Async runtime ✅
