#!/bin/bash
# Automatically sync package-lock.json with package.json changes
# Can be used as git hook, CI/CD step, or manual script

set -e

echo "🔍 Checking package-lock.json sync..."

# Change to npm directory (if it exists)
NPM_DIR="$(dirname "$0")/../npm"
if [ ! -d "$NPM_DIR" ]; then
    echo "✅ No npm directory found, skipping sync"
    exit 0
fi
cd "$NPM_DIR"

# Check if package.json or any workspace package.json changed
CHANGED_PACKAGES=$(git diff --cached --name-only | grep -E 'package\.json$' || true)

if [ -n "$CHANGED_PACKAGES" ]; then
    echo "📦 Package.json changes detected:"
    echo "$CHANGED_PACKAGES"
    echo ""
    echo "🔄 Running npm install to sync lock file..."

    # Run npm install to update lock file
    # Use --ignore-optional to skip platform-specific optional deps (darwin-arm64 on linux, etc.)
    npm install --ignore-optional || {
        echo "⚠️  npm install had warnings (likely platform-specific optional deps)"
        echo "   Continuing with lock file sync..."
    }

    # Check if lock file changed
    if git diff --name-only | grep -q 'package-lock.json'; then
        echo "✅ Lock file updated successfully"

        # If running as pre-commit hook, add the lock file
        if [ "${GIT_HOOK}" = "pre-commit" ]; then
            cd ..
            git add npm/package-lock.json
            echo "✅ Lock file staged for commit"
        else
            echo "⚠️  Lock file modified but not staged"
            echo "   Run: git add npm/package-lock.json"
        fi
    else
        echo "✅ Lock file already in sync"
    fi
else
    echo "✅ No package.json changes detected"
fi
