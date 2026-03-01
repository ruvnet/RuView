#!/bin/bash
# 7sense - DDD Progress Tracker Worker
# Tracks Domain-Driven Design implementation progress for bioacoustics platform

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
METRICS_DIR="$PROJECT_ROOT/.claude-flow/metrics"
DDD_FILE="$METRICS_DIR/ddd-progress.json"
V3_PROGRESS="$METRICS_DIR/v3-progress.json"
LAST_RUN_FILE="$METRICS_DIR/.ddd-last-run"

mkdir -p "$METRICS_DIR"

# 7sense Bounded Contexts (from ADR-002)
DOMAINS=("audio-ingestion" "embedding" "vector-space" "learning" "analysis" "interpretation")

# Domain descriptions for display
declare -A DOMAIN_NAMES=(
  ["audio-ingestion"]="Audio Ingestion"
  ["embedding"]="Embedding"
  ["vector-space"]="Vector Space"
  ["learning"]="Learning"
  ["analysis"]="Analysis"
  ["interpretation"]="Interpretation"
)

should_run() {
  if [ ! -f "$LAST_RUN_FILE" ]; then return 0; fi
  local last_run=$(cat "$LAST_RUN_FILE" 2>/dev/null || echo "0")
  local now=$(date +%s)
  [ $((now - last_run)) -ge 600 ]  # 10 minutes
}

check_domain() {
  local domain="$1"
  local domain_short="${domain//-/}"

  # Possible paths for domain implementation
  local crate_path="$PROJECT_ROOT/crates/sevensense-${domain//-/}"
  local alt_crate_path="$PROJECT_ROOT/crates/sevensense-${domain}"
  local domain_path="$PROJECT_ROOT/src/domains/$domain"
  local alt_domain_path="$PROJECT_ROOT/src/domains/${domain//-/_}"

  local score=0

  # Check if domain directory exists (20 points)
  local path=""
  if [ -d "$crate_path" ]; then
    path="$crate_path"
    score=$((score + 20))
  elif [ -d "$alt_crate_path" ]; then
    path="$alt_crate_path"
    score=$((score + 20))
  elif [ -d "$domain_path" ]; then
    path="$domain_path"
    score=$((score + 20))
  elif [ -d "$alt_domain_path" ]; then
    path="$alt_domain_path"
    score=$((score + 20))
  fi

  if [ -n "$path" ]; then
    # Check for domain layer (15 points) - entities, aggregates, value objects
    [ -d "$path/domain" ] || [ -d "$path/src/domain" ] || \
    [ -f "$path/entities.rs" ] || [ -f "$path/src/entities.rs" ] && score=$((score + 15))

    # Check for application layer (15 points) - use cases, services
    [ -d "$path/application" ] || [ -d "$path/src/application" ] || \
    [ -f "$path/services.rs" ] || [ -f "$path/src/services.rs" ] && score=$((score + 15))

    # Check for infrastructure layer (15 points) - repositories, adapters
    [ -d "$path/infrastructure" ] || [ -d "$path/src/infrastructure" ] || \
    [ -f "$path/repository.rs" ] || [ -f "$path/src/repository.rs" ] && score=$((score + 15))

    # Check for API/interface layer (10 points)
    [ -d "$path/api" ] || [ -d "$path/src/api" ] || \
    [ -f "$path/handlers.rs" ] || [ -f "$path/src/handlers.rs" ] && score=$((score + 10))

    # Check for tests (15 points)
    local test_count=$(find "$path" -name "*_test.rs" -o -name "*_tests.rs" -o -name "test_*.rs" 2>/dev/null | wc -l)
    [ "$test_count" -gt 0 ] && score=$((score + 15))

    # Check for module exports (10 points)
    [ -f "$path/lib.rs" ] || [ -f "$path/mod.rs" ] || [ -f "$path/src/lib.rs" ] && score=$((score + 10))
  fi

  echo "$score"
}

count_artifacts() {
  local pattern="$1"

  find "$PROJECT_ROOT/crates" "$PROJECT_ROOT/src" -name "*.rs" 2>/dev/null | \
    xargs grep -l "$pattern" 2>/dev/null | \
    grep -v target | wc -l || echo "0"
}

track_ddd() {
  echo "[$(date +%H:%M:%S)] Tracking 7sense DDD progress..."

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

  # Count 7sense DDD artifacts
  local entities=$(count_artifacts "struct.*Recording\|struct.*CallSegment\|struct.*Embedding\|struct.*Cluster\|struct.*Taxon")
  local value_objects=$(count_artifacts "struct.*Id\|struct.*Timestamp\|struct.*Metadata")
  local aggregates=$(count_artifacts "impl.*Recording\|impl.*EvidencePack\|impl.*AnalysisSession")
  local repositories=$(count_artifacts "trait.*Repository\|impl.*Repository")
  local services=$(count_artifacts "struct.*Service\|impl.*Service")
  local events=$(count_artifacts "enum.*Event\|struct.*Event\|DomainEvent")

  # Write DDD metrics
  cat > "$DDD_FILE" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "project": "7sense",
  "progress": $progress,
  "domains": {
    ${domain_scores%,*}
  },
  "completed": $completed_domains,
  "total": ${#DOMAINS[@]},
  "boundedContexts": {
    "audio-ingestion": "Recording capture, segmentation, preprocessing",
    "embedding": "Perch 2.0 inference, vector normalization",
    "vector-space": "HNSW indexing, similarity search",
    "learning": "GNN training, pattern discovery",
    "analysis": "Clustering, motif detection, sequences",
    "interpretation": "RAB evidence packs, constrained generation"
  },
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

  # Update v3-progress.json if it exists
  if [ -f "$V3_PROGRESS" ] && command -v jq &>/dev/null; then
    jq --argjson progress "$progress" --argjson completed "$completed_domains" \
      '.ddd.progress = $progress | .domains.completed = $completed' \
      "$V3_PROGRESS" > "$V3_PROGRESS.tmp" 2>/dev/null && mv "$V3_PROGRESS.tmp" "$V3_PROGRESS"
  fi

  echo "[$(date +%H:%M:%S)] ✓ 7sense DDD: ${progress}% | Contexts: $completed_domains/${#DOMAINS[@]} | Entities: $entities | Services: $services"

  date +%s > "$LAST_RUN_FILE"
}

case "${1:-check}" in
  "run"|"track") track_ddd ;;
  "check") should_run && track_ddd || echo "[$(date +%H:%M:%S)] Skipping (throttled)" ;;
  "force") rm -f "$LAST_RUN_FILE"; track_ddd ;;
  "status")
    if [ -f "$DDD_FILE" ]; then
      jq -r '"7sense DDD: \(.progress)% | Contexts: \(.completed)/\(.total) | Entities: \(.artifacts.entities) | Services: \(.artifacts.services)"' "$DDD_FILE"
    else
      echo "No DDD data available"
    fi
    ;;
  "contexts")
    if [ -f "$DDD_FILE" ]; then
      echo "7sense Bounded Contexts:"
      jq -r '.boundedContexts | to_entries[] | "  \(.key): \(.value)"' "$DDD_FILE"
    fi
    ;;
  "details")
    if [ -f "$DDD_FILE" ]; then
      echo "Domain Progress:"
      jq -r '.domains | to_entries[] | "  \(.key): \(.value)%"' "$DDD_FILE"
    fi
    ;;
  *) echo "Usage: $0 [run|check|force|status|contexts|details]" ;;
esac
