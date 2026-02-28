#!/bin/bash
# Learning Scenarios Setup Script
# This teaches the hooks system about shell script handling

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

echo "🧠 RuVector Learning Scenarios Setup"
echo "====================================="

# Function to initialize learning patterns
init_patterns() {
    echo "📊 Initializing learning patterns..."

    # Check if intelligence file exists
    if [[ -f "$PROJECT_ROOT/.ruvector/intelligence.json" ]]; then
        local pattern_count=$(jq '.patterns | length' "$PROJECT_ROOT/.ruvector/intelligence.json" 2>/dev/null || echo "0")
        echo "   Found $pattern_count existing patterns"
    else
        echo "   No existing patterns, starting fresh"
    fi
}

# Function to record a learning event
record_event() {
    local event_type="$1"
    local file_path="$2"
    local outcome="${3:-success}"

    echo "📝 Recording: $event_type on $file_path ($outcome)"

    # Use ruvector-cli if available
    if command -v ruvector-cli &>/dev/null; then
        ruvector-cli hooks remember "$event_type: $file_path" -t "$event_type" 2>/dev/null || true
    fi
}

# Function to simulate diverse file operations
simulate_diversity() {
    echo "🔄 Simulating diverse file operations..."

    local file_types=(
        "rs:rust-developer"
        "ts:typescript-developer"
        "yaml:config-specialist"
        "json:data-analyst"
        "sh:devops-engineer"
        "md:documentation-writer"
    )

    for entry in "${file_types[@]}"; do
        IFS=':' read -r ext agent <<< "$entry"
        echo "   .$ext -> $agent"
    done
}

# Main execution
main() {
    init_patterns
    simulate_diversity

    echo ""
    echo "✅ Learning scenarios initialized"
    echo "   Run 'ruvector hooks stats' to see current patterns"
}

main "$@"
