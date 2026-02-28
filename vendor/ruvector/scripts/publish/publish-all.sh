#!/bin/bash
# RuVector - Publish All Packages Script
# Triggers GitHub Actions workflow to build and publish for all platforms

set -e

VERSION="${1:-0.1.31}"
DRY_RUN="${2:-false}"

echo "🚀 RuVector Publish All Packages"
echo "================================"
echo "Version: $VERSION"
echo "Dry Run: $DRY_RUN"
echo ""

# Check if gh CLI is available
if ! command -v gh &> /dev/null; then
    echo "❌ GitHub CLI (gh) is required. Install with: brew install gh"
    exit 1
fi

# Check if logged in
if ! gh auth status &> /dev/null; then
    echo "❌ Not logged into GitHub. Run: gh auth login"
    exit 1
fi

echo "📦 Packages to publish:"
echo "  crates.io:"
echo "    - ruvector-math v$VERSION"
echo "    - ruvector-attention v$VERSION"
echo "    - ruvector-math-wasm v$VERSION"
echo "    - ruvector-attention-wasm v$VERSION"
echo ""
echo "  npm:"
echo "    - ruvector-math-wasm v$VERSION"
echo "    - @ruvector/attention v$VERSION"
echo "    - @ruvector/attention-wasm v$VERSION"
echo "    - @ruvector/attention-linux-x64-gnu v$VERSION"
echo "    - @ruvector/attention-linux-arm64-gnu v$VERSION"
echo "    - @ruvector/attention-darwin-x64 v$VERSION"
echo "    - @ruvector/attention-darwin-arm64 v$VERSION"
echo "    - @ruvector/attention-win32-x64-msvc v$VERSION"
echo ""

read -p "Continue? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 0
fi

echo ""
echo "🔄 Triggering GitHub Actions workflow..."

gh workflow run publish-all.yml \
    --field version="$VERSION" \
    --field publish_crates=true \
    --field publish_npm=true \
    --field dry_run="$DRY_RUN"

echo ""
echo "✅ Workflow triggered!"
echo ""
echo "📊 Monitor progress at:"
echo "   https://github.com/ruvnet/ruvector/actions/workflows/publish-all.yml"
echo ""
echo "Or run: gh run list --workflow=publish-all.yml"
