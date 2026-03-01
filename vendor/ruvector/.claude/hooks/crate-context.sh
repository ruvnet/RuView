#!/bin/bash
# Load crate-specific context for intelligent code assistance
# Outputs relevant examples, tests, and documentation paths

set -e

FILE="$1"
if [ -z "$FILE" ]; then
    echo "Usage: $0 <file_path>"
    exit 1
fi

cd /workspaces/ruvector

# Detect crate from file path
CRATE_DIR=$(echo "$FILE" | grep -oP "crates/[^/]+" | head -1 || echo "")
CRATE_NAME=""

if [ -n "$CRATE_DIR" ]; then
    CRATE_NAME=$(basename "$CRATE_DIR")
fi

echo "{"
echo "  \"file\": \"$FILE\","
echo "  \"crate\": \"$CRATE_NAME\","

# Find related test files
echo "  \"tests\": ["
TESTS=$(find "$CRATE_DIR/tests" -name "*.rs" 2>/dev/null | head -5 | while read f; do echo "    \"$f\","; done | sed '$ s/,$//')
echo "$TESTS"
echo "  ],"

# Find related examples
echo "  \"examples\": ["
EXAMPLES=$(find "$CRATE_DIR/examples" -name "*.rs" 2>/dev/null | head -5 | while read f; do echo "    \"$f\","; done | sed '$ s/,$//')
if [ -z "$EXAMPLES" ]; then
    # Check examples/ directory at root
    case "$CRATE_NAME" in
        "ruvector-core"|"ruvector-wasm")
            EXAMPLES=$(find "examples/wasm" "examples/wasm-react" -name "*.ts" -o -name "*.tsx" 2>/dev/null | head -3 | while read f; do echo "    \"$f\","; done | sed '$ s/,$//')
            ;;
        "ruvector-graph"*)
            EXAMPLES=$(find "examples" -path "*graph*" -name "*.rs" 2>/dev/null | head -3 | while read f; do echo "    \"$f\","; done | sed '$ s/,$//')
            ;;
        "ruvector-mincut"*)
            EXAMPLES=$(find "examples/mincut" -name "*.rs" 2>/dev/null | head -3 | while read f; do echo "    \"$f\","; done | sed '$ s/,$//')
            ;;
    esac
fi
echo "$EXAMPLES"
echo "  ],"

# Find related documentation
echo "  \"docs\": ["
DOCS=$(find "$CRATE_DIR" -name "*.md" 2>/dev/null | head -5 | while read f; do echo "    \"$f\","; done | sed '$ s/,$//')
if [ -z "$DOCS" ]; then
    case "$CRATE_NAME" in
        "ruvector-postgres"*)
            DOCS=$(find "docs/postgres" -name "*.md" 2>/dev/null | head -5 | while read f; do echo "    \"$f\","; done | sed '$ s/,$//')
            ;;
        "rvlite")
            DOCS=$(find "crates/rvlite/docs" -name "*.md" 2>/dev/null | head -5 | while read f; do echo "    \"$f\","; done | sed '$ s/,$//')
            ;;
    esac
fi
echo "$DOCS"
echo "  ],"

# Key dependencies
echo "  \"key_deps\": ["
if [ -f "$CRATE_DIR/Cargo.toml" ]; then
    grep -E "^\[dependencies\]" -A 20 "$CRATE_DIR/Cargo.toml" 2>/dev/null | grep -E "^[a-z]" | head -5 | while read line; do
        DEP=$(echo "$line" | cut -d'=' -f1 | tr -d ' ')
        echo "    \"$DEP\","
    done | sed '$ s/,$//'
fi
echo "  ],"

# Suggest related commands
echo "  \"commands\": {"
case "$CRATE_NAME" in
    "ruvector-core"|"ruvector-bench")
        echo "    \"test\": \"cargo test -p $CRATE_NAME\","
        echo "    \"bench\": \"cargo bench -p ruvector-bench\","
        echo "    \"check\": \"cargo check -p $CRATE_NAME\""
        ;;
    "rvlite"|"ruvector-wasm"|"ruvector-graph-wasm"|"ruvector-gnn-wasm")
        echo "    \"build\": \"wasm-pack build --target web --release\","
        echo "    \"test\": \"wasm-pack test --headless --chrome\","
        echo "    \"size\": \".claude/hooks/wasm-size-check.sh $CRATE_NAME\""
        ;;
    "ruvector-postgres")
        echo "    \"build\": \"cargo pgrx package\","
        echo "    \"test\": \"cargo pgrx test\","
        echo "    \"run\": \"cargo pgrx run\""
        ;;
    *)
        echo "    \"test\": \"cargo test -p $CRATE_NAME\","
        echo "    \"check\": \"cargo check -p $CRATE_NAME\""
        ;;
esac
echo "  }"

echo "}"
