# Intelligence System Improvements

## Current State
- 5 hook types, 16 CLI commands
- 4,023 memories, 117 Q-states, 8,520 calibration samples
- Learning: command-type + context (cargo_in_rvlite, etc.)

## Proposed Improvements

### 1. Error Pattern Learning (High Impact)
Learn from specific error types, not just success/failure.
```javascript
// Instead of just: learn(state, 'command-failed', -0.5)
// Learn specific error patterns:
learn('cargo_build_error:E0308', 'type-mismatch', -0.3)
learn('cargo_build_error:E0433', 'missing-import', -0.2)
```
**Benefit**: Suggest fixes based on error type

### 2. File Sequence Learning (High Impact)
Track which files are often edited together.
```javascript
// After editing lib.rs, user often edits:
sequences['crates/core/lib.rs'] = [
  { file: 'crates/core/tests/lib.rs', probability: 0.8 },
  { file: 'crates/core/Cargo.toml', probability: 0.3 }
]
```
**Benefit**: Proactively suggest related files

### 3. Crate Dependency Graph
Use the 42-crate structure for smarter suggestions.
```javascript
dependencies = {
  'rvlite': ['ruvector-core', 'ruvector-attention-wasm'],
  'sona': ['ruvector-core']
}
// If editing rvlite, warn about downstream effects
```
**Benefit**: Warn about breaking changes

### 4. Test Suggestion Triggers
Automatically suggest running tests after certain edits.
```javascript
// Post-edit hook detects:
if (file.match(/src\/.*\.rs$/) && !file.includes('test')) {
  suggest('Run tests: cargo test -p ' + crate);
}
```
**Benefit**: Reduce test-related bugs

### 5. Build Optimization
Learn minimal rebuild commands.
```javascript
// Instead of 'cargo build', suggest:
if (changedCrates.length === 1) {
  suggest(`cargo build -p ${changedCrates[0]}`);
}
```
**Benefit**: Faster iteration cycles

### 6. Session Context Memory
Track patterns within the current session.
```javascript
sessionContext = {
  filesEdited: ['lib.rs', 'mod.rs'],
  commandsRun: ['cargo check', 'cargo test'],
  errors: ['E0308 in line 45']
}
// Use for smarter in-session suggestions
```
**Benefit**: Context-aware suggestions

### 7. Git Branch Awareness
Different patterns for different branches.
```javascript
// On feature branch: suggest more tests
// On main: suggest careful review
branchPatterns = {
  'main': { requireTests: true, suggestReview: true },
  'feature/*': { suggestTests: true }
}
```
**Benefit**: Branch-appropriate workflows

### 8. Hook Performance Metrics
Track hook execution time.
```javascript
hookMetrics = {
  'pre-edit': { avgMs: 45, p99Ms: 120 },
  'post-command': { avgMs: 80, p99Ms: 200 }
}
// Alert if hooks become slow
```
**Benefit**: Prevent hook slowdowns

### 9. Predictive Prefetching
Pre-load likely-needed data.
```javascript
// When user opens a Rust file, prefetch:
// - Related test files
// - Crate's Cargo.toml
// - Recent memories for that crate
```
**Benefit**: Faster responses

### 10. Multi-Crate Coordination
Optimize cross-crate work patterns.
```javascript
// Detect multi-crate changes
if (editedCrates.length > 1) {
  suggest('Consider running: cargo build --workspace');
  recordPattern('multi-crate-edit', editedCrates);
}
```
**Benefit**: Better monorepo workflows

## Implementation Priority

| Improvement | Impact | Effort | Priority |
|------------|--------|--------|----------|
| Error Pattern Learning | High | Medium | 1 |
| File Sequence Learning | High | Medium | 2 |
| Test Suggestion | High | Low | 3 |
| Session Context | Medium | Medium | 4 |
| Build Optimization | Medium | Low | 5 |
| Crate Dependencies | Medium | Medium | 6 |
| Git Branch Awareness | Medium | Low | 7 |
| Hook Performance | Low | Low | 8 |
| Predictive Prefetch | Low | High | 9 |
| Multi-Crate Coord | Low | Medium | 10 |
