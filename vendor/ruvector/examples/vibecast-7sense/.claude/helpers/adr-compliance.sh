#!/bin/bash
# 7sense - ADR Compliance Checker Worker
# Checks compliance with Architecture Decision Records for bioacoustics platform

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
METRICS_DIR="$PROJECT_ROOT/.claude-flow/metrics"
ADR_FILE="$METRICS_DIR/adr-compliance.json"
LAST_RUN_FILE="$METRICS_DIR/.adr-last-run"

mkdir -p "$METRICS_DIR"

# 7sense ADRs (from /docs/adr/)
declare -A ADRS=(
  ["ADR-001"]="System Architecture - Modular Monolith"
  ["ADR-002"]="DDD Bounded Contexts"
  ["ADR-003"]="Security Architecture"
  ["ADR-004"]="Performance Optimization"
  ["ADR-005"]="Self-Learning & Hooks"
  ["ADR-006"]="Data Architecture & Vector Storage"
  ["ADR-007"]="ML Inference Pipeline"
  ["ADR-008"]="API Design"
  ["ADR-009"]="Visualization & UI"
)

should_run() {
  if [ ! -f "$LAST_RUN_FILE" ]; then return 0; fi
  local last_run=$(cat "$LAST_RUN_FILE" 2>/dev/null || echo "0")
  local now=$(date +%s)
  [ $((now - last_run)) -ge 900 ]  # 15 minutes
}

check_adr_001() {
  # ADR-001: System Architecture - Modular Monolith (10 domain modules)
  local score=0

  # Check for modular crate structure
  [ -d "$PROJECT_ROOT/crates/sevensense-core" ] && score=$((score + 15))
  [ -d "$PROJECT_ROOT/crates/sevensense-audio" ] && score=$((score + 15))
  [ -d "$PROJECT_ROOT/crates/sevensense-embedding" ] && score=$((score + 15))
  [ -d "$PROJECT_ROOT/crates/sevensense-vector" ] && score=$((score + 15))
  [ -d "$PROJECT_ROOT/crates/sevensense-learning" ] && score=$((score + 10))
  [ -d "$PROJECT_ROOT/crates/sevensense-analysis" ] && score=$((score + 10))
  [ -d "$PROJECT_ROOT/crates/sevensense-interpretation" ] && score=$((score + 10))

  # Check for Cargo.toml workspace
  grep -q "sevensense" "$PROJECT_ROOT/Cargo.toml" 2>/dev/null && score=$((score + 10))

  echo "$score"
}

check_adr_002() {
  # ADR-002: DDD Bounded Contexts (6 contexts)
  local score=0
  local contexts_found=0

  # Check for bounded context implementations
  for ctx in audio embedding vector learning analysis interpretation; do
    if [ -d "$PROJECT_ROOT/crates/sevensense-$ctx" ] || \
       [ -d "$PROJECT_ROOT/src/domains/$ctx" ] || \
       grep -rq "mod ${ctx}" "$PROJECT_ROOT/src" 2>/dev/null; then
      contexts_found=$((contexts_found + 1))
    fi
  done

  # Score based on contexts found (6 total)
  score=$((contexts_found * 100 / 6))

  # Bonus for domain events
  grep -rq "DomainEvent\|Event\|EventBus" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 10))
  [ "$score" -gt 100 ] && score=100

  echo "$score"
}

check_adr_003() {
  # ADR-003: Security Architecture
  local score=0

  # Check for input validation
  grep -rq "validate\|InputValidator\|sanitize" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  # Check for path traversal protection
  grep -rq "PathValidator\|secure_path\|canonicalize" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  # Check for authentication
  grep -rq "auth\|jwt\|Argon2\|bcrypt" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  # Check for audit logging
  grep -rq "audit\|AuditLog\|provenance" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  # Check for rate limiting
  grep -rq "rate_limit\|RateLimiter\|throttle" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  echo "$score"
}

check_adr_004() {
  # ADR-004: Performance Optimization (HNSW, quantization, caching)
  local score=0

  # Check for HNSW implementation
  grep -rq "hnsw\|HNSW\|HierarchicalNSW" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 30))

  # Check for quantization
  grep -rq "quantize\|int8\|float16\|compression" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 25))

  # Check for caching
  grep -rq "cache\|Cache\|LruCache\|moka" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 25))

  # Check for batch processing
  grep -rq "batch\|Batch\|parallel\|rayon" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  echo "$score"
}

check_adr_005() {
  # ADR-005: Self-Learning & Hooks (claude-flow integration)
  local score=0

  # Check for hooks integration
  [ -f "$PROJECT_ROOT/.claude/settings.json" ] && score=$((score + 20))
  grep -q "hooks" "$PROJECT_ROOT/.claude/settings.json" 2>/dev/null && score=$((score + 20))

  # Check for learning patterns
  grep -rq "pattern\|learn\|train" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  # Check for memory namespaces
  grep -rq "namespace\|patterns\|motifs\|species" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  # Check for EWC/consolidation
  grep -rq "ewc\|consolidate\|forget" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  echo "$score"
}

check_adr_006() {
  # ADR-006: Data Architecture & Vector Storage (3-tier, hyperbolic)
  local score=0

  # Check for entity definitions
  grep -rq "Recording\|CallSegment\|Embedding\|Cluster" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 25))

  # Check for tiered storage
  grep -rq "hot\|warm\|cold\|tier" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 25))

  # Check for hyperbolic embeddings
  grep -rq "poincare\|hyperbolic\|Poincar" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 25))

  # Check for graph relationships
  grep -rq "SIMILAR\|NEXT\|HAS_SEGMENT\|edge\|graph" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 25))

  echo "$score"
}

check_adr_007() {
  # ADR-007: ML Inference Pipeline (Perch 2.0, ONNX)
  local score=0

  # Check for ONNX integration
  grep -rq "onnx\|ONNX\|onnxruntime" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" "$PROJECT_ROOT/Cargo.toml" 2>/dev/null && score=$((score + 30))

  # Check for audio preprocessing
  grep -rq "mel\|spectrogram\|resample\|32000\|32kHz" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 25))

  # Check for embedding normalization
  grep -rq "normalize\|L2\|norm" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 25))

  # Check for model management
  grep -rq "ModelManager\|model_version\|model_registry" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  echo "$score"
}

check_adr_008() {
  # ADR-008: API Design (REST, GraphQL, WebSocket)
  local score=0

  # Check for REST/HTTP
  grep -rq "axum\|actix\|rocket\|warp\|http" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" "$PROJECT_ROOT/Cargo.toml" 2>/dev/null && score=$((score + 30))

  # Check for GraphQL
  grep -rq "graphql\|async-graphql\|juniper" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" "$PROJECT_ROOT/Cargo.toml" 2>/dev/null && score=$((score + 25))

  # Check for WebSocket
  grep -rq "websocket\|ws\|tungstenite" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" "$PROJECT_ROOT/Cargo.toml" 2>/dev/null && score=$((score + 25))

  # Check for OpenAPI/schema
  grep -rq "openapi\|swagger\|schema" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  echo "$score"
}

check_adr_009() {
  # ADR-009: Visualization & UI (UMAP, WASM, D3)
  local score=0

  # Check for UMAP/dimensionality reduction
  grep -rq "umap\|tsne\|reduction\|project" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" "$PROJECT_ROOT/Cargo.toml" 2>/dev/null && score=$((score + 30))

  # Check for WASM support
  grep -rq "wasm\|wasm-bindgen\|web-sys" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" "$PROJECT_ROOT/Cargo.toml" 2>/dev/null && score=$((score + 30))

  # Check for visualization (plotly, D3, etc)
  grep -rq "plotly\|d3\|chart\|viz" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" "$PROJECT_ROOT/apps" 2>/dev/null && score=$((score + 20))

  # Check for evidence pack display
  grep -rq "EvidencePack\|evidence\|citation" "$PROJECT_ROOT/src" "$PROJECT_ROOT/crates" 2>/dev/null && score=$((score + 20))

  echo "$score"
}

check_compliance() {
  echo "[$(date +%H:%M:%S)] Checking 7sense ADR compliance..."

  local total_score=0
  local compliant_count=0

  # Check each ADR
  local adr_001=$(check_adr_001)
  local adr_002=$(check_adr_002)
  local adr_003=$(check_adr_003)
  local adr_004=$(check_adr_004)
  local adr_005=$(check_adr_005)
  local adr_006=$(check_adr_006)
  local adr_007=$(check_adr_007)
  local adr_008=$(check_adr_008)
  local adr_009=$(check_adr_009)

  # Calculate totals
  for score in $adr_001 $adr_002 $adr_003 $adr_004 $adr_005 $adr_006 $adr_007 $adr_008 $adr_009; do
    total_score=$((total_score + score))
    [ "$score" -ge 50 ] && compliant_count=$((compliant_count + 1))
  done

  local avg_score=$((total_score / 9))

  # Write ADR compliance metrics
  cat > "$ADR_FILE" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "project": "7sense",
  "overallCompliance": $avg_score,
  "compliantCount": $compliant_count,
  "totalADRs": 9,
  "adrs": {
    "ADR-001": {"score": $adr_001, "title": "System Architecture - Modular Monolith"},
    "ADR-002": {"score": $adr_002, "title": "DDD Bounded Contexts"},
    "ADR-003": {"score": $adr_003, "title": "Security Architecture"},
    "ADR-004": {"score": $adr_004, "title": "Performance Optimization"},
    "ADR-005": {"score": $adr_005, "title": "Self-Learning & Hooks"},
    "ADR-006": {"score": $adr_006, "title": "Data Architecture & Vector Storage"},
    "ADR-007": {"score": $adr_007, "title": "ML Inference Pipeline"},
    "ADR-008": {"score": $adr_008, "title": "API Design"},
    "ADR-009": {"score": $adr_009, "title": "Visualization & UI"}
  }
}
EOF

  echo "[$(date +%H:%M:%S)] ✓ 7sense ADR Compliance: ${avg_score}% | Compliant: $compliant_count/9"

  date +%s > "$LAST_RUN_FILE"
}

case "${1:-check}" in
  "run") check_compliance ;;
  "check") should_run && check_compliance || echo "[$(date +%H:%M:%S)] Skipping (throttled)" ;;
  "force") rm -f "$LAST_RUN_FILE"; check_compliance ;;
  "status")
    if [ -f "$ADR_FILE" ]; then
      jq -r '"7sense Compliance: \(.overallCompliance)% | Compliant: \(.compliantCount)/\(.totalADRs)"' "$ADR_FILE"
    else
      echo "No ADR data available"
    fi
    ;;
  "details")
    if [ -f "$ADR_FILE" ]; then
      jq -r '.adrs | to_entries[] | "\(.key): \(.value.score)% - \(.value.title)"' "$ADR_FILE"
    fi
    ;;
  *) echo "Usage: $0 [run|check|force|status|details]" ;;
esac
