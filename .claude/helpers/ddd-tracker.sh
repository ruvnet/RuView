#!/bin/bash
# Claude Flow V3 - DDD Progress Tracker Worker
# Tracks Domain-Driven Design implementation progress

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
METRICS_DIR="$PROJECT_ROOT/.claude-flow/metrics"
DDD_FILE="$METRICS_DIR/ddd-progress.json"
V3_PROGRESS="$METRICS_DIR/v3-progress.json"
LAST_RUN_FILE="$METRICS_DIR/.ddd-last-run"

mkdir -p "$METRICS_DIR"

# V3 Target Domains
DOMAINS=("agent-lifecycle" "task-execution" "memory-management" "coordination" "shared-kernel")

should_run() {
  if [ ! -f "$LAST_RUN_FILE" ]; then return 0; fi
  local last_run=$(cat "$LAST_RUN_FILE" 2>/dev/null || echo "0")
  local now=$(date +%s)
  [ $((now - last_run)) -ge 600 ]  # 10 minutes
}

check_domain() {
  local domain="$1"
  local domain_path="$PROJECT_ROOT/v3/@claude-flow/$domain"
  local alt_path="$PROJECT_ROOT/src/domains/$domain"

  local score=0
  local max_score=100

  # Check if domain directory exists (20 points)
  if [ -d "$domain_path" ] || [ -d "$alt_path" ]; then
    score=$((score + 20))
    local path="${domain_path:-$alt_path}"
    [ -d "$domain_path" ] && path="$domain_path" || path="$alt_path"

    # Check for domain layer (15 points)
    [ -d "$path/domain" ] || [ -d "$path/src/domain" ] && score=$((score + 15))

    # Check for application layer (15 points)
    [ -d "$path/application" ] || [ -d "$path/src/application" ] && score=$((score + 15))

    # Check for infrastructure layer (15 points)
    [ -d "$path/infrastructure" ] || [ -d "$path/src/infrastructure" ] && score=$((score + 15))

    # Check for API/interface layer (10 points)
    [ -d "$path/api" ] || [ -d "$path/src/api" ] && score=$((score + 10))

    # Check for tests (15 points)
    local test_count=$(find "$path" -name "*.test.ts" -o -name "*.spec.ts" 2>/dev/null | wc -l)
    [ "$test_count" -gt 0 ] && score=$((score + 15))

    # Check for index/exports (10 points)
    [ -f "$path/index.ts" ] || [ -f "$path/src/index.ts" ] && score=$((score + 10))
  fi

  echo "$score"
}

count_entities() {
  local type="$1"
  local pattern="$2"

  find "$PROJECT_ROOT/v3" "$PROJECT_ROOT/src" -name "*.ts" 2>/dev/null | \
    xargs grep -l "$pattern" 2>/dev/null | \
    grep -v node_modules | grep -v ".test." | wc -l || echo "0"
}

track_ddd() {
  echo "[$(date +%H:%M:%S)] Tracking DDD progress..."

  local total_score=0
  local domain_scores=""
  local completed_domains=0

  for domain in "${DOMAINS[@]}"; do
    local score=$(check_domain "$domain")
    total_score=$((total_score + score))
    domain_scores="$domain_scores\"$domain\": $score, "

    [ "$score" -ge 50 ] && completed_domains=$((completed_domains + 1))
  done

  # Calculate overall progress
  local max_total=$((${#DOMAINS[@]} * 100))
  local progress=$((total_score * 100 / max_total))

  # Count DDD artifacts
  local entities=$(count_entities "entities" "class.*Entity\|interface.*Entity")
  local value_objects=$(count_entities "value-objects" "class.*VO\|ValueObject")
  local aggregates=$(count_entities "aggregates" "class.*Aggregate\|AggregateRoot")
  local repositories=$(count_entities "repositories" "interface.*Repository\|Repository")
  local services=$(count_entities "services" "class.*Service\|Service")
  local events=$(count_entities "events" "class.*Event\|DomainEvent")

  # Write DDD metrics
  cat > "$DDD_FILE" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "progress": $progress,
  "domains": {
    ${domain_scores%,*}
  },
  "completed": $completed_domains,
  "total": ${#DOMAINS[@]},
  "artifacts": {
    "entities": $entities,
    "valueObjects": $value_objects,
    "aggregates": $aggregates,
    "repositories": $repositories,
    "services": $services,
    "domainEvents": $events
  }
}
EOF

  # Update v3-progress.json
  if [ -f "$V3_PROGRESS" ] && command -v jq &>/dev/null; then
    jq --argjson progress "$progress" --argjson completed "$completed_domains" \
      '.ddd.progress = $progress | .domains.completed = $completed' \
      "$V3_PROGRESS" > "$V3_PROGRESS.tmp" && mv "$V3_PROGRESS.tmp" "$V3_PROGRESS"
  fi

  echo "[$(date +%H:%M:%S)] ✓ DDD: ${progress}% | Domains: $completed_domains/${#DOMAINS[@]} | Entities: $entities | Services: $services"

  date +%s > "$LAST_RUN_FILE"
}

case "${1:-check}" in
  "run"|"track") track_ddd ;;
  "check") should_run && track_ddd || echo "[$(date +%H:%M:%S)] Skipping (throttled)" ;;
  "force") rm -f "$LAST_RUN_FILE"; track_ddd ;;
  "status")
    if [ -f "$DDD_FILE" ]; then
      jq -r '"Progress: \(.progress)% | Domains: \(.completed)/\(.total) | Entities: \(.artifacts.entities) | Services: \(.artifacts.services)"' "$DDD_FILE"
    else
      echo "No DDD data available"
    fi
    ;;
  *) echo "Usage: $0 [run|check|force|status]" ;;
esac
