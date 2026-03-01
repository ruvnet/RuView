#!/bin/bash
# Install git hooks for automatic lock file syncing

set -e

echo "🔧 Installing git hooks..."

# Create .git/hooks directory if it doesn't exist
mkdir -p .git/hooks

# Install pre-commit hook
if [ -f ".githooks/pre-commit" ]; then
    ln -sf ../../.githooks/pre-commit .git/hooks/pre-commit
    chmod +x .git/hooks/pre-commit
    chmod +x .githooks/pre-commit
    echo "✅ Pre-commit hook installed"
else
    echo "❌ Pre-commit hook file not found"
    exit 1
fi

echo ""
echo "✨ Git hooks installed successfully!"
echo ""
echo "The following hooks are now active:"
echo "  • pre-commit: Automatically syncs package-lock.json when package.json changes"
echo ""
echo "To disable, run: rm .git/hooks/pre-commit"
