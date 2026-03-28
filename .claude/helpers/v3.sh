#!/bin/bash
# V3 Helper Alias Script - Quick access to all V3 development tools

set -e

HELPERS_DIR=".claude/helpers"

case "$1" in
  "status"|"st")
    "$HELPERS_DIR/v3-quick-status.sh"
    ;;

  "progress"|"prog")
    shift
    "$HELPERS_DIR/update-v3-progress.sh" "$@"
    ;;

  "validate"|"check")
    "$HELPERS_DIR/validate-v3-config.sh"
    ;;

  "statusline"|"sl")
    ".claude/statusline.sh"
    ;;

  "update")
    if [ -z "$2" ] || [ -z "$3" ]; then
      echo "Usage: v3 update <metric> <value>"
      echo "Examples:"
      echo "  v3 update domain 3"
      echo "  v3 update agent 8"
      echo "  v3 update security 2"
      echo "  v3 update performance 2.5x"
      echo "  v3 update memory 45%"
      echo "  v3 update ddd 75"
      exit 1
    fi
    "$HELPERS_DIR/update-v3-progress.sh" "$2" "$3"
    ;;

  "full-status"|"fs")
    echo "🔍 V3 Development Environment Status"
    echo "====================================="
    echo ""
    echo "📊 Quick Status:"
    "$HELPERS_DIR/v3-quick-status.sh"
    echo ""
    echo "📺 Full Statusline:"
    ".claude/statusline.sh"
    ;;

  "init")
    echo "🚀 Initializing V3 Development Environment..."

    # Run validation first
    echo ""
    echo "1️⃣ Validating configuration..."
    if "$HELPERS_DIR/validate-v3-config.sh"; then
      echo ""
      echo "2️⃣ Showing current status..."
      "$HELPERS_DIR/v3-quick-status.sh"
      echo ""
      echo "✅ V3 development environment is ready!"
      echo ""
      echo "🔧 Quick commands:"
      echo "  v3 status        - Show quick status"
      echo "  v3 update        - Update progress metrics"
      echo "  v3 statusline    - Show full statusline"
      echo "  v3 validate      - Validate configuration"
    else
      echo ""
      echo "❌ Configuration validation failed. Please fix issues before proceeding."
      exit 1
    fi
    ;;

  "help"|"--help"|"-h"|"")
    echo "Claude Flow V3 Helper Tool"
    echo "=========================="
    echo ""
    echo "Usage: v3 <command> [options]"
    echo ""
    echo "Commands:"
    echo "  status, st              Show quick development status"
    echo "  progress, prog [args]   Update progress metrics"
    echo "  validate, check         Validate V3 configuration"
    echo "  statusline, sl          Show full statusline"
    echo "  full-status, fs         Show both quick status and statusline"
    echo "  update <metric> <value> Update specific metric"
    echo "  init                    Initialize and validate environment"
    echo "  help                    Show this help message"
    echo ""
    echo "Update Examples:"
    echo "  v3 update domain 3      # Mark 3 domains complete"
    echo "  v3 update agent 8       # Set 8 agents active"
    echo "  v3 update security 2    # Mark 2 CVEs fixed"
    echo "  v3 update performance 2.5x # Set performance to 2.5x"
    echo "  v3 update memory 45%    # Set memory reduction to 45%"
    echo "  v3 update ddd 75        # Set DDD progress to 75%"
    echo ""
    echo "Quick Start:"
    echo "  v3 init                 # Initialize environment"
    echo "  v3 status               # Check current progress"
    ;;

  *)
    echo "Unknown command: $1"
    echo "Run 'v3 help' for usage information"
    exit 1
    ;;
esac