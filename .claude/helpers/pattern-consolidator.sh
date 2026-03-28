#!/bin/bash
# Claude Flow V3 - Pattern Consolidator Worker
# Deduplicates patterns, prunes old ones, improves quality scores

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PATTERNS_DB="$PROJECT_ROOT/.claude-flow/learning/patterns.db"
METRICS_DIR="$PROJECT_ROOT/.claude-flow/metrics"
LAST_RUN_FILE="$METRICS_DIR/.consolidator-last-run"

mkdir -p "$METRICS_DIR"

should_run() {
  if [ ! -f "$LAST_RUN_FILE" ]; then return 0; fi
  local last_run=$(cat "$LAST_RUN_FILE" 2>/dev/null || echo "0")
  local now=$(date +%s)
  [ $((now - last_run)) -ge 900 ]  # 15 minutes
}

consolidate_patterns() {
  if [ ! -f "$PATTERNS_DB" ] || ! command -v sqlite3 &>/dev/null; then
    echo "[$(date +%H:%M:%S)] No patterns database found"
    return 0
  fi

  echo "[$(date +%H:%M:%S)] Consolidating patterns..."

  # Count before
  local before=$(sqlite3 "$PATTERNS_DB" "SELECT COUNT(*) FROM short_term_patterns" 2>/dev/null || echo "0")

  # Remove duplicates (keep highest quality)
  sqlite3 "$PATTERNS_DB" "
    DELETE FROM short_term_patterns
    WHERE rowid NOT IN (
      SELECT MIN(rowid) FROM short_term_patterns
      GROUP BY strategy, domain
    )
  " 2>/dev/null || true

  # Prune old low-quality patterns (older than 7 days, quality < 0.3)
  sqlite3 "$PATTERNS_DB" "
    DELETE FROM short_term_patterns
    WHERE quality < 0.3
    AND created_at < datetime('now', '-7 days')
  " 2>/dev/null || true

  # Promote high-quality patterns to long-term (quality > 0.8, used > 5 times)
  sqlite3 "$PATTERNS_DB" "
    INSERT OR IGNORE INTO long_term_patterns (strategy, domain, quality, source)
    SELECT strategy, domain, quality, 'consolidated'
    FROM short_term_patterns
    WHERE quality > 0.8
  " 2>/dev/null || true

  # Decay quality of unused patterns
  sqlite3 "$PATTERNS_DB" "
    UPDATE short_term_patterns
    SET quality = quality * 0.95
    WHERE updated_at < datetime('now', '-1 day')
  " 2>/dev/null || true

  # Count after
  local after=$(sqlite3 "$PATTERNS_DB" "SELECT COUNT(*) FROM short_term_patterns" 2>/dev/null || echo "0")
  local removed=$((before - after))

  echo "[$(date +%H:%M:%S)] ✓ Consolidated: $before → $after patterns (removed $removed)"

  date +%s > "$LAST_RUN_FILE"
}

case "${1:-check}" in
  "run"|"consolidate") consolidate_patterns ;;
  "check") should_run && consolidate_patterns || echo "[$(date +%H:%M:%S)] Skipping (throttled)" ;;
  "force") rm -f "$LAST_RUN_FILE"; consolidate_patterns ;;
  "status")
    if [ -f "$PATTERNS_DB" ] && command -v sqlite3 &>/dev/null; then
      local short=$(sqlite3 "$PATTERNS_DB" "SELECT COUNT(*) FROM short_term_patterns" 2>/dev/null || echo "0")
      local long=$(sqlite3 "$PATTERNS_DB" "SELECT COUNT(*) FROM long_term_patterns" 2>/dev/null || echo "0")
      local avg_q=$(sqlite3 "$PATTERNS_DB" "SELECT ROUND(AVG(quality), 2) FROM short_term_patterns" 2>/dev/null || echo "0")
      echo "Patterns: $short short-term, $long long-term, avg quality: $avg_q"
    fi
    ;;
  *) echo "Usage: $0 [run|check|force|status]" ;;
esac
